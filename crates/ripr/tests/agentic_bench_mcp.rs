//! B2/B3 agentic benchmarks for the MCP journey.
//!
//! Executable oracle for `benchmarks/agentic/mcp-journey` (`spec.json` v1).
//! The harness spawns `ripr mcp --stdio` against a two-branch temp fixture
//! (`main` holds the open predicate, `journey` closes it) and drives NDJSON
//! `tools/call` sequences over the legacy lifecycle. B2 walks the full
//! journey (status, refresh, list_gaps, get_gap, prepare_repair twice,
//! get_repair_attempt, get_receipt_status) and pins replay identity; B3
//! pins the typed-failure vocabulary (`no_snapshot`, `stale_snapshot`,
//! `item_not_found`, `attempt_not_found`), the never-silent-null envelope
//! rule, pipelined coherence, and the no-write/no-artifact boundary.
//! `analysis_in_flight` and `result_too_large` are excitation-limited over
//! stdio (single-request admission; 64 KiB budget under the 128 KiB wire
//! bound) and are guarded by contract audit instead of direct excitation.
//! Every test returns `Result<(), String>`; failures name the step and the
//! wire bytes that broke the contract.

use serde_json::{Value, json};
use std::collections::{BTreeMap, HashMap};
use std::io::{BufRead, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

#[path = "common/mod.rs"]
mod common;

use common::fixture_git::{fixture_git_ok, fixture_git_output};

const PROTOCOL_VERSION: &str = "2025-11-25";
const EXPECTED_MAX_RESPONSE_BYTES: u64 = 128 * 1024;
const EXPECTED_MAX_MESSAGE_BYTES: u64 = 256 * 1024;
const REFRESH_TIMEOUT: Duration = Duration::from_mins(3);
const REPLY_TIMEOUT: Duration = Duration::from_mins(1);
const REAP_TIMEOUT: Duration = Duration::from_secs(15);
const UNKNOWN_ATTEMPT_ID: &str = "repair-attempt-000000000000000000000000";
const UNKNOWN_CANONICAL_ID: &str = "gap:does-not-exist";
const STALE_SNAPSHOT_ID: &str = "snapshot:stale-probe";
const OVERSIZE_ARGUMENT_LEN: usize = 200_000;

static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

fn at<'a>(value: &'a Value, pointer: &str, context: &str) -> Result<&'a Value, String> {
    value
        .pointer(pointer)
        .ok_or_else(|| format!("{context}: response omitted `{pointer}`: {value}"))
}

fn as_str<'a>(value: &'a Value, pointer: &str, context: &str) -> Result<&'a str, String> {
    at(value, pointer, context)?
        .as_str()
        .ok_or_else(|| format!("{context}: `{pointer}` is not a string: {value}"))
}

fn as_u64(value: &Value, pointer: &str, context: &str) -> Result<u64, String> {
    at(value, pointer, context)?
        .as_u64()
        .ok_or_else(|| format!("{context}: `{pointer}` is not a u64: {value}"))
}

fn as_bool(value: &Value, pointer: &str, context: &str) -> Result<bool, String> {
    at(value, pointer, context)?
        .as_bool()
        .ok_or_else(|| format!("{context}: `{pointer}` is not a bool: {value}"))
}

fn nonempty_str<'a>(value: &'a Value, pointer: &str, context: &str) -> Result<&'a str, String> {
    let text = as_str(value, pointer, context)?;
    if text.is_empty() {
        return Err(format!("{context}: `{pointer}` is empty: {value}"));
    }
    Ok(text)
}

/// Resolve a tool name through `tools/list`: the driver must use a name
/// the server advertised, never a literal it guessed (#5209).
fn tool_named(tools: &Value, name: &str) -> Result<String, String> {
    let entries = at(tools, "/result/tools", "tools/list")?
        .as_array()
        .ok_or_else(|| format!("tools/list: `/result/tools` is not an array: {tools}"))?;
    for entry in entries {
        if entry.pointer("/name").and_then(Value::as_str) == Some(name) {
            return Ok(name.to_string());
        }
    }
    Err(format!(
        "tools/list: server did not advertise {name:?}: {tools}"
    ))
}

/// One stdout line. A line that is not JSON is a protocol violation,
/// never a skippable frame. Replies correlate by their `id` member.
struct WireLine {
    parsed: Result<Value, String>,
    raw_len: usize,
}

fn spawn_stdout_reader(
    stdout: std::process::ChildStdout,
    sender: mpsc::Sender<WireLine>,
) -> JoinHandle<()> {
    thread::spawn(move || {
        let mut reader = std::io::BufReader::new(stdout);
        loop {
            let mut line = Vec::new();
            match reader.read_until(b'\n', &mut line) {
                Ok(0) => break,
                Ok(_) => {
                    let parsed = serde_json::from_slice::<Value>(&line)
                        .map_err(|error| format!("MCP stdout line is not JSON: {error}"));
                    let delivered = sender.send(WireLine {
                        parsed,
                        raw_len: line.len(),
                    });
                    if delivered.is_err() {
                        break;
                    }
                }
                Err(_) => break,
            }
        }
    })
}

/// Owned stdio session: one writer (this thread), one stdout reader, one
/// stderr drainer. Out-of-order replies are buffered by id so pipelined
/// batches (the B3 in-flight probe) correlate correctly.
struct McpSession {
    child: Child,
    stdin: Option<std::process::ChildStdin>,
    inbox: Receiver<WireLine>,
    reader: Option<JoinHandle<()>>,
    stderr_join: Option<JoinHandle<Vec<u8>>>,
    pending: HashMap<String, Value>,
    frame_sizes: Vec<usize>,
    sent: usize,
}

impl Drop for McpSession {
    fn drop(&mut self) {
        let _ = self.child.kill();
    }
}

struct SessionAudit {
    frames: usize,
    max_frame_bytes: usize,
}

impl McpSession {
    fn spawn(root: &Path) -> Result<Self, String> {
        let mut child = Command::new(env!("CARGO_BIN_EXE_ripr"))
            .args(["mcp", "--stdio", "--root"])
            .arg(root)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|error| format!("spawn ripr mcp: {error}"))?;
        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| "MCP child exposed no stdin".to_string())?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| "MCP child exposed no stdout".to_string())?;
        let stderr = child
            .stderr
            .take()
            .ok_or_else(|| "MCP child exposed no stderr".to_string())?;
        let (sender, inbox) = mpsc::channel();
        let reader = spawn_stdout_reader(stdout, sender);
        let stderr_join = thread::spawn(move || {
            let mut pipe = stderr;
            let mut buffer = Vec::new();
            let _ = std::io::Read::read_to_end(&mut pipe, &mut buffer);
            buffer
        });
        Ok(Self {
            child,
            stdin: Some(stdin),
            inbox,
            reader: Some(reader),
            stderr_join: Some(stderr_join),
            pending: HashMap::new(),
            frame_sizes: Vec::new(),
            sent: 0,
        })
    }

    fn send_value(&mut self, request: &Value) -> Result<(), String> {
        let mut encoded =
            serde_json::to_vec(request).map_err(|error| format!("encode request: {error}"))?;
        encoded.push(b'\n');
        let Some(stdin) = self.stdin.as_mut() else {
            return Err("MCP stdin is already closed".to_string());
        };
        stdin
            .write_all(&encoded)
            .map_err(|error| format!("write MCP request: {error}"))?;
        stdin
            .flush()
            .map_err(|error| format!("flush MCP request: {error}"))?;
        self.sent += 1;
        Ok(())
    }

    fn await_reply(&mut self, id: &str, timeout: Duration) -> Result<Value, String> {
        if let Some(hit) = self.pending.remove(id) {
            return Ok(hit);
        }
        let deadline = Instant::now() + timeout;
        loop {
            let now = Instant::now();
            if now >= deadline {
                return Err(format!(
                    "timed out after {timeout:?} waiting for MCP reply id {id:?}; \
                     {} other replies buffered",
                    self.pending.len()
                ));
            }
            match self.inbox.recv_timeout(deadline - now) {
                Ok(line) => {
                    self.frame_sizes.push(line.raw_len);
                    let value = line.parsed?;
                    let got = value
                        .get("id")
                        .and_then(Value::as_str)
                        .unwrap_or_default()
                        .to_string();
                    if got == id {
                        return Ok(value);
                    }
                    self.pending.insert(got, value);
                }
                Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                    return Err(format!(
                        "timed out after {timeout:?} waiting for MCP reply id {id:?}; \
                         {} other replies buffered",
                        self.pending.len()
                    ));
                }
                Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                    return Err(format!(
                        "MCP reply stream ended while waiting for id {id:?}"
                    ));
                }
            }
        }
    }

    fn initialize(&mut self) -> Result<Value, String> {
        self.send_value(&json!({
            "jsonrpc": "2.0",
            "id": "init",
            "method": "initialize",
            "params": {
                "protocolVersion": PROTOCOL_VERSION,
                "capabilities": {},
                "clientInfo": { "name": "ripr-agentic-bench", "version": "1" }
            }
        }))?;
        let reply = self.await_reply("init", REPLY_TIMEOUT)?;
        let negotiated = as_str(&reply, "/result/protocolVersion", "initialize")?;
        if negotiated != PROTOCOL_VERSION {
            return Err(format!(
                "initialize negotiated {negotiated:?}, want {PROTOCOL_VERSION:?}: {reply}"
            ));
        }
        self.send_value(&json!({
            "jsonrpc": "2.0",
            "method": "notifications/initialized"
        }))?;
        // A notification carries no id, so it is not a request: the reply
        // audit must not expect a response line for it.
        self.sent -= 1;
        Ok(reply)
    }

    fn send_call(&mut self, id: &str, tool: &str, arguments: &Value) -> Result<(), String> {
        self.send_value(&json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": "tools/call",
            "params": { "name": tool, "arguments": arguments }
        }))
    }

    fn list_tools(&mut self, id: &str) -> Result<Value, String> {
        self.send_value(&json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": "tools/list",
            "params": {}
        }))?;
        self.await_reply(id, REPLY_TIMEOUT)
    }

    fn call(&mut self, id: &str, tool: &str, arguments: Value) -> Result<Value, String> {
        self.send_call(id, tool, &arguments)?;
        let timeout = if tool == "ripr_refresh" {
            REFRESH_TIMEOUT
        } else {
            REPLY_TIMEOUT
        };
        self.await_reply(id, timeout)
    }

    /// Write a batch in immediate succession without awaiting between
    /// lines, then collect every reply. The transport admits one request
    /// at a time, so this proves pipelined bytes stay coherent rather
    /// than overlapping.
    fn call_batch(&mut self, batch: &[(&str, &str, Value)]) -> Result<Vec<Value>, String> {
        for (id, tool, arguments) in batch {
            self.send_call(id, tool, arguments)?;
        }
        let mut replies = Vec::new();
        for (id, tool, _) in batch {
            let timeout = if *tool == "ripr_refresh" {
                REFRESH_TIMEOUT
            } else {
                REPLY_TIMEOUT
            };
            replies.push(self.await_reply(id, timeout)?);
        }
        Ok(replies)
    }

    fn finish(mut self, bound: u64) -> Result<SessionAudit, String> {
        drop(self.stdin.take());
        let deadline = Instant::now() + REAP_TIMEOUT;
        loop {
            match self
                .child
                .try_wait()
                .map_err(|error| format!("poll ripr mcp: {error}"))?
            {
                Some(status) => {
                    if let Some(handle) = self.reader.take() {
                        handle
                            .join()
                            .map_err(|error| format!("stdout reader panicked: {error:?}"))?;
                    }
                    while let Ok(line) = self.inbox.try_recv() {
                        self.frame_sizes.push(line.raw_len);
                        line.parsed?;
                    }
                    let stderr = match self.stderr_join.take() {
                        Some(handle) => handle
                            .join()
                            .map_err(|error| format!("stderr reader panicked: {error:?}"))?,
                        None => Vec::new(),
                    };
                    if !status.success() {
                        return Err(format!(
                            "ripr mcp exited with status {:?}; stderr: {}",
                            status.code(),
                            String::from_utf8_lossy(&stderr)
                        ));
                    }
                    if !stderr.is_empty() {
                        return Err(format!(
                            "successful MCP session contaminated stderr: {}",
                            String::from_utf8_lossy(&stderr)
                        ));
                    }
                    let mut max_frame_bytes = 0_usize;
                    for size in &self.frame_sizes {
                        if *size as u64 > bound {
                            return Err(format!(
                                "MCP frame of {size} bytes exceeds the {bound}-byte response bound"
                            ));
                        }
                        if *size > max_frame_bytes {
                            max_frame_bytes = *size;
                        }
                    }
                    if self.frame_sizes.len() != self.sent {
                        return Err(format!(
                            "reply count {} does not match request count {}",
                            self.frame_sizes.len(),
                            self.sent
                        ));
                    }
                    return Ok(SessionAudit {
                        frames: self.frame_sizes.len(),
                        max_frame_bytes,
                    });
                }
                None if Instant::now() < deadline => {
                    thread::sleep(Duration::from_millis(25));
                }
                None => {
                    let _ = self.child.kill();
                    let _ = self.child.wait();
                    return Err("ripr mcp did not exit after stdin close".to_string());
                }
            }
        }
    }
}

/// A successful tool envelope: no protocol error, `isError: false`, and a
/// structured document. Returns the structured document.
fn tool_success<'a>(reply: &'a Value, context: &str) -> Result<&'a Value, String> {
    if let Some(error) = reply.get("error") {
        return Err(format!("{context}: expected a tool result, got {error}"));
    }
    if as_bool(reply, "/result/isError", context)? {
        return Err(format!(
            "{context}: expected success, got a typed failure: {reply}"
        ));
    }
    at(reply, "/result/structuredContent", context)
}

/// A typed tool failure: `isError: true` with a code/detail/recovery
/// failure block. Returns the failure code.
fn tool_failure(reply: &Value, context: &str) -> Result<String, String> {
    if let Some(error) = reply.get("error") {
        return Err(format!("{context}: expected a typed failure, got {error}"));
    }
    if !as_bool(reply, "/result/isError", context)? {
        return Err(format!(
            "{context}: expected a typed failure, got success: {reply}"
        ));
    }
    let document = at(reply, "/result/structuredContent", context)?;
    let code = nonempty_str(document, "/failure/code", context)?.to_string();
    nonempty_str(document, "/failure/detail", context)?;
    nonempty_str(document, "/failure/recovery", context)?;
    let text = as_str(reply, "/result/content/0/text", context)?;
    if !text.contains(&code) {
        return Err(format!(
            "{context}: failure text does not name its code {code:?}: {reply}"
        ));
    }
    Ok(code)
}

fn protocol_error_code(reply: &Value, context: &str) -> Result<i64, String> {
    at(reply, "/error/code", context)?
        .as_i64()
        .ok_or_else(|| format!("{context}: expected a protocol error code: {reply}"))
}

/// The advertised wire bounds, pinned to the documented constants. The
/// response bound drives every frame audit in this file.
fn response_bound(status: &Value) -> Result<u64, String> {
    let bound = as_u64(status, "/mcp/bounds/max_response_bytes", "status bounds")?;
    if bound != EXPECTED_MAX_RESPONSE_BYTES {
        return Err(format!(
            "status advertised max_response_bytes {bound}, want {EXPECTED_MAX_RESPONSE_BYTES}: {status}"
        ));
    }
    let cap = as_u64(status, "/mcp/bounds/max_message_bytes", "status bounds")?;
    if cap != EXPECTED_MAX_MESSAGE_BYTES {
        return Err(format!(
            "status advertised max_message_bytes {cap}, want {EXPECTED_MAX_MESSAGE_BYTES}: {status}"
        ));
    }
    Ok(bound)
}

/// Temp fixture root that removes itself on every exit path.
struct Fixture {
    root: PathBuf,
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

fn benchmark_input_dir() -> Result<PathBuf, String> {
    let dir =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../benchmarks/agentic/mcp-journey/input");
    if !dir.is_dir() {
        return Err(format!(
            "benchmark input directory {} is missing",
            dir.display()
        ));
    }
    Ok(dir)
}

fn copy_tree(source: &Path, destination: &Path) -> Result<(), String> {
    let mut entries = std::fs::read_dir(source)
        .map_err(|error| format!("read {}: {error}", source.display()))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("read {}: {error}", source.display()))?;
    entries.sort_by_key(|entry| entry.file_name());
    for entry in entries {
        let from = entry.path();
        let to = destination.join(entry.file_name());
        let file_type = entry
            .file_type()
            .map_err(|error| format!("stat {}: {error}", from.display()))?;
        if file_type.is_dir() {
            std::fs::create_dir_all(&to)
                .map_err(|error| format!("create {}: {error}", to.display()))?;
            copy_tree(&from, &to)?;
        } else if file_type.is_file() {
            if let Some(parent) = to.parent() {
                std::fs::create_dir_all(parent)
                    .map_err(|error| format!("create {}: {error}", parent.display()))?;
            }
            std::fs::copy(&from, &to)
                .map_err(|error| format!("copy {} to {}: {error}", from.display(), to.display()))?;
        } else {
            return Err(format!("fixture source {} is not a file", from.display()));
        }
    }
    Ok(())
}

fn commit_fixture(root: &Path, message: &str) -> Result<(), String> {
    fixture_git_ok(root, &["add", "."])?;
    fixture_git_ok(
        root,
        &[
            "-c",
            "user.name=ripr fixture",
            "-c",
            "user.email=fixture@ripr.invalid",
            "commit",
            "-qm",
            message,
        ],
    )
}

/// Two-branch fixture: `main` holds the open predicate, `journey` (checked
/// out) closes it. Default-base resolution picks `main`, so refresh
/// analyzes exactly the predicate change.
fn install_fixture(label: &str) -> Result<Fixture, String> {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|error| format!("clock before Unix epoch: {error}"))?
        .as_nanos();
    let root = std::env::temp_dir().join(format!(
        "ripr-agentic-bench-mcp-{label}-{}-{stamp}-{}",
        std::process::id(),
        TEMP_COUNTER.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&root)
        .map_err(|error| format!("create {}: {error}", root.display()))?;
    let input = benchmark_input_dir()?;
    copy_tree(&input.join("base"), &root)?;
    fixture_git_ok(&root, &["-c", "init.defaultBranch=main", "init", "-q"])
        .map_err(|error| format!("fixture git init: {error}"))?;
    fixture_git_ok(&root, &["config", "user.name", "ripr fixture"])
        .map_err(|error| format!("fixture git config: {error}"))?;
    fixture_git_ok(&root, &["config", "user.email", "fixture@ripr.invalid"])
        .map_err(|error| format!("fixture git config: {error}"))?;
    fixture_git_ok(&root, &["config", "core.autocrlf", "false"])
        .map_err(|error| format!("fixture git config: {error}"))?;
    commit_fixture(&root, "open predicate").map_err(|error| format!("base commit: {error}"))?;
    fixture_git_ok(&root, &["checkout", "-q", "-b", "journey"])
        .map_err(|error| format!("fixture git checkout: {error}"))?;
    std::fs::copy(input.join("journey/src/lib.rs"), root.join("src/lib.rs"))
        .map_err(|error| format!("apply journey production change: {error}"))?;
    commit_fixture(&root, "closed predicate")
        .map_err(|error| format!("journey commit: {error}"))?;
    let head = fixture_git_output(&root, &["rev-parse", "--abbrev-ref", "HEAD"])?;
    if head.trim() != "journey" {
        return Err(format!("fixture HEAD is {head:?}, want journey"));
    }
    let base = fixture_git_output(&root, &["rev-parse", "--verify", "main"])?;
    if base.trim().is_empty() {
        return Err("fixture lost its main branch".to_string());
    }
    Ok(Fixture { root })
}

fn sha256_hex(bytes: &[u8]) -> String {
    use sha2::Digest;
    let digest = sha2::Sha256::digest(bytes);
    let mut text = String::with_capacity(digest.len() * 2);
    for byte in digest {
        text.push_str(&format!("{byte:02x}"));
    }
    text
}

/// Full content snapshot: relative path to `len:sha256`. Mtimes are
/// ignored; only bytes matter.
fn snapshot_tree(root: &Path) -> Result<BTreeMap<String, String>, String> {
    fn walk(
        root: &Path,
        directory: &Path,
        output: &mut BTreeMap<String, String>,
    ) -> Result<(), String> {
        let mut entries = std::fs::read_dir(directory)
            .map_err(|error| format!("read {}: {error}", directory.display()))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| format!("read {}: {error}", directory.display()))?;
        entries.sort_by_key(|entry| entry.file_name());
        for entry in entries {
            let path = entry.path();
            let relative = path
                .strip_prefix(root)
                .map_err(|error| format!("relativize {}: {error}", path.display()))?;
            let key = relative.to_string_lossy().replace('\\', "/");
            let file_type = entry
                .file_type()
                .map_err(|error| format!("stat {}: {error}", path.display()))?;
            if file_type.is_dir() {
                walk(root, &path, output)?;
            } else if file_type.is_file() {
                let bytes = std::fs::read(&path)
                    .map_err(|error| format!("read {}: {error}", path.display()))?;
                output.insert(key, format!("{}:{}", bytes.len(), sha256_hex(&bytes)));
            } else {
                output.insert(key, "non_file".to_string());
            }
        }
        Ok(())
    }

    let mut output = BTreeMap::new();
    walk(root, root, &mut output)?;
    Ok(output)
}

fn require_tree_equal(
    before: &BTreeMap<String, String>,
    after: &BTreeMap<String, String>,
    context: &str,
) -> Result<(), String> {
    if before == after {
        return Ok(());
    }
    let mut drift = Vec::new();
    for (path, hash) in after {
        match before.get(path) {
            Some(expected) if expected == hash => {}
            Some(_) => drift.push(format!("modified {path}")),
            None => drift.push(format!("added {path}")),
        }
    }
    for path in before.keys() {
        if !after.contains_key(path) {
            drift.push(format!("deleted {path}"));
        }
    }
    drift.sort();
    drift.truncate(8);
    Err(format!(
        "{context}: fixture tree drifted: {}",
        drift.join(", ")
    ))
}

/// The status authority boundary: every capability stays `none`.
fn require_authority_none(status: &Value) -> Result<(), String> {
    for pointer in [
        "/workspace/authority/source_edit_capability",
        "/workspace/authority/verification_execution_capability",
        "/workspace/authority/mutation_execution_capability",
        "/workspace/authority/model_provider",
    ] {
        let capability = as_str(status, pointer, "status authority")?;
        if capability != "none" {
            return Err(format!(
                "status authority {pointer} is {capability:?}, want none: {status}"
            ));
        }
    }
    Ok(())
}

/// One completed refresh: terminal `completed` state plus a non-empty
/// snapshot id. Returns `(snapshot_id, finding_count, total_items)`.
fn require_completed_refresh(refresh: &Value) -> Result<(String, u64, u64), String> {
    let state = as_str(refresh, "/attempt/state", "refresh")?;
    if state != "completed" {
        return Err(format!(
            "refresh state is {state:?}, want completed: {refresh}"
        ));
    }
    let snapshot = nonempty_str(refresh, "/snapshot/snapshot_id", "refresh")?.to_string();
    if !snapshot.starts_with("snapshot:") {
        return Err(format!(
            "refresh snapshot id {snapshot:?} lost its grammar: {refresh}"
        ));
    }
    let findings = as_u64(refresh, "/snapshot/finding_count", "refresh")?;
    let total = as_u64(refresh, "/snapshot/total_items", "refresh")?;
    if findings != total {
        return Err(format!(
            "refresh finding_count {findings} != total_items {total}: {refresh}"
        ));
    }
    let known_good = as_str(refresh, "/last_known_good/snapshot_id", "refresh")?;
    if known_good != snapshot {
        return Err(format!(
            "refresh last_known_good {known_good:?} != snapshot {snapshot:?}: {refresh}"
        ));
    }
    Ok((snapshot, findings, total))
}

/// Gap-count consistency: array lengths match their counts, selection
/// partitions the total, and byte counts nest.
fn require_consistent_counts(list: &Value, snapshot: &str, total_items: u64) -> Result<(), String> {
    let context = "list_gaps counts";
    let bound = as_str(list, "/snapshot_id", context)?;
    if bound != snapshot {
        return Err(format!(
            "list_gaps binds {bound:?}, want {snapshot:?}: {list}"
        ));
    }
    let total = as_u64(list, "/total", context)?;
    let eligible = as_u64(list, "/eligible", context)?;
    let selected = as_u64(list, "/selected", context)?;
    let omitted = as_u64(list, "/omitted", context)?;
    let items = at(list, "/items", context)?
        .as_array()
        .ok_or_else(|| format!("{context}: `/items` is not an array: {list}"))?;
    let omitted_items = at(list, "/omitted_items", context)?
        .as_array()
        .ok_or_else(|| format!("{context}: `/omitted_items` is not an array: {list}"))?;
    if items.len() as u64 != selected {
        return Err(format!(
            "{context}: items.len() {} != selected {selected}: {list}",
            items.len()
        ));
    }
    if omitted_items.len() as u64 != omitted {
        return Err(format!(
            "{context}: omitted_items.len() {} != omitted {omitted}: {list}",
            omitted_items.len()
        ));
    }
    if selected + omitted != total {
        return Err(format!(
            "{context}: selected {selected} + omitted {omitted} != total {total}: {list}"
        ));
    }
    if selected > eligible || eligible > total {
        return Err(format!(
            "{context}: counts do not nest (selected {selected} <= eligible {eligible} <= total {total}): {list}"
        ));
    }
    if total != total_items {
        return Err(format!(
            "{context}: total {total} != refresh total_items {total_items}: {list}"
        ));
    }
    let selected_bytes = as_u64(list, "/selected_bytes", context)?;
    let complete_bytes = as_u64(list, "/complete_bytes", context)?;
    if selected_bytes > complete_bytes {
        return Err(format!(
            "{context}: selected_bytes {selected_bytes} > complete_bytes {complete_bytes}: {list}"
        ));
    }
    Ok(())
}

fn require_failure_code(reply: &Value, context: &str, expected: &str) -> Result<Value, String> {
    let code = tool_failure(reply, context)?;
    if code != expected {
        return Err(format!(
            "{context}: failure code is {code:?}, want {expected:?}: {reply}"
        ));
    }
    let document = at(reply, "/result/structuredContent", context)?.clone();
    Ok(document)
}

#[test]
fn b2_mcp_happy_path_journey() -> Result<(), String> {
    let fixture = install_fixture("b2")?;
    let tree_before = snapshot_tree(&fixture.root)?;
    let mut session = McpSession::spawn(&fixture.root)?;
    session.initialize()?;

    // #5209: the B2 driver selects every tool from `tools/list` — no
    // literal tool name below may reach the wire undiscovered.
    let tools = session.list_tools("b2-tools")?;
    let status_tool = tool_named(&tools, "ripr_workspace_status")?;
    let refresh_tool = tool_named(&tools, "ripr_refresh")?;
    let list_tool = tool_named(&tools, "ripr_list_gaps")?;
    let gap_tool = tool_named(&tools, "ripr_get_gap")?;
    let prepare_tool = tool_named(&tools, "ripr_prepare_repair")?;
    let attempt_tool = tool_named(&tools, "ripr_get_repair_attempt")?;
    let receipt_tool = tool_named(&tools, "ripr_get_receipt_status")?;

    let reply = session.call("b2-status", &status_tool, json!({}))?;
    let status = tool_success(&reply, "b2 status")?.clone();
    if as_str(&status, "/workspace/workspace_state", "b2 status")? != "ready" {
        return Err(format!("b2 status: fixture root is not ready: {status}"));
    }
    require_authority_none(&status)?;
    if as_str(&status, "/session/attempt_state", "b2 status")? != "no_snapshot" {
        return Err(format!(
            "b2 status: fresh session already has a snapshot: {status}"
        ));
    }
    let bound = response_bound(&status)?;

    let reply = session.call("b2-refresh", &refresh_tool, json!({}))?;
    let refresh = tool_success(&reply, "b2 refresh")?.clone();
    let (snapshot, findings, total) = require_completed_refresh(&refresh)?;
    if total == 0 || findings == 0 {
        return Err(format!(
            "b2 refresh: the journey fixture must yield at least one gap: {refresh}"
        ));
    }

    let reply = session.call("b2-status-after", &status_tool, json!({}))?;
    let status = tool_success(&reply, "b2 status after refresh")?.clone();
    if as_str(&status, "/session/attempt_state", "b2 status after refresh")? != "completed" {
        return Err(format!("b2 status after refresh: not completed: {status}"));
    }
    let rebound = as_str(
        &status,
        "/session/last_completed_snapshot/snapshot_id",
        "b2 status after refresh",
    )?;
    if rebound != snapshot {
        return Err(format!(
            "b2 status after refresh: rebound {rebound:?}, want {snapshot:?}: {status}"
        ));
    }

    let reply = session.call("b2-list", &list_tool, json!({}))?;
    let list = tool_success(&reply, "b2 list_gaps")?.clone();
    require_consistent_counts(&list, &snapshot, total)?;
    let selected = as_u64(&list, "/selected", "b2 list_gaps")?;
    if selected == 0 {
        return Err(format!("b2 list_gaps: no gap selected: {list}"));
    }
    let canonical = nonempty_str(&list, "/items/0/canonical_id", "b2 list_gaps")?.to_string();
    let file = as_str(&list, "/items/0/file", "b2 list_gaps")?;
    if !file.ends_with("src/lib.rs") {
        return Err(format!(
            "b2 list_gaps: first item is outside src/lib.rs: {list}"
        ));
    }
    if as_u64(&list, "/items/0/line", "b2 list_gaps")? != 2 {
        return Err(format!(
            "b2 list_gaps: boundary item moved off line 2: {list}"
        ));
    }

    let reply = session.call("b2-gap", &gap_tool, json!({ "canonical_id": canonical }))?;
    let gap = tool_success(&reply, "b2 get_gap")?.clone();
    if as_str(&gap, "/snapshot_id", "b2 get_gap")? != snapshot {
        return Err(format!("b2 get_gap: snapshot drifted: {gap}"));
    }
    if as_str(&gap, "/item/canonical_id", "b2 get_gap")? != canonical {
        return Err(format!("b2 get_gap: item drifted: {gap}"));
    }
    let expression = as_str(&gap, "/item/changed_behavior/expression", "b2 get_gap")?;
    if !expression.contains("discount_threshold") {
        return Err(format!("b2 get_gap: wrong changed behavior: {gap}"));
    }
    if at(&gap, "/item/links/repair_attempt", "b2 get_gap")? != &Value::Null {
        return Err(format!(
            "b2 get_gap: repair link binds before prepare: {gap}"
        ));
    }
    nonempty_str(&gap, "/item/links/repair_attempt_note", "b2 get_gap")?;

    let reply = session.call(
        "b2-prepare",
        &prepare_tool,
        json!({ "canonical_id": canonical }),
    )?;
    let first = tool_success(&reply, "b2 prepare_repair")?.clone();
    if as_str(&first, "/snapshot_id", "b2 prepare_repair")? != snapshot {
        return Err(format!("b2 prepare_repair: snapshot drifted: {first}"));
    }
    let first_text = as_str(&reply, "/result/content/0/text", "b2 prepare_repair")?.to_string();
    let reply = session.call(
        "b2-prepare-replay",
        &prepare_tool,
        json!({ "canonical_id": canonical }),
    )?;
    let second = tool_success(&reply, "b2 prepare_repair replay")?.clone();
    if second != first {
        return Err(
            "b2 prepare_repair: replay document differs from the first prepare".to_string(),
        );
    }
    let second_text = as_str(&reply, "/result/content/0/text", "b2 prepare_repair replay")?;
    if second_text != first_text {
        return Err("b2 prepare_repair: replay text differs from the first prepare".to_string());
    }

    // Replay twin: a different item must not replay the same document.
    let reply = session.call(
        "b2-prepare-twin",
        &prepare_tool,
        json!({ "canonical_id": UNKNOWN_CANONICAL_ID }),
    )?;
    require_failure_code(&reply, "b2 prepare_repair twin", "item_not_found")?;
    let reply = session.call(
        "b2-prepare-stale",
        &prepare_tool,
        json!({ "canonical_id": canonical, "snapshot_id": STALE_SNAPSHOT_ID }),
    )?;
    let stale = require_failure_code(&reply, "b2 prepare_repair stale", "stale_snapshot")?;
    if as_str(
        &stale,
        "/failure/data/current_snapshot_id",
        "b2 prepare_repair stale",
    )? != snapshot
    {
        return Err(format!(
            "b2 prepare_repair stale: current id not echoed: {stale}"
        ));
    }

    if as_bool(&first, "/repair_packet_ready", "b2 prepare_repair")? {
        let attempt = nonempty_str(&first, "/attempt/attempt_id", "b2 prepare_repair")?.to_string();
        if !attempt.starts_with("repair-attempt-") {
            return Err(format!(
                "b2 prepare_repair: attempt id {attempt:?} lost its grammar: {first}"
            ));
        }
        let reply = session.call(
            "b2-attempt",
            &attempt_tool,
            json!({ "attempt_id": attempt }),
        )?;
        let attempt_doc = tool_success(&reply, "b2 get_repair_attempt")?.clone();
        if as_str(&attempt_doc, "/attempt_id", "b2 get_repair_attempt")? != attempt {
            return Err(format!(
                "b2 get_repair_attempt: attempt drifted: {attempt_doc}"
            ));
        }
        if as_str(&attempt_doc, "/snapshot_id", "b2 get_repair_attempt")? != snapshot {
            return Err(format!(
                "b2 get_repair_attempt: snapshot drifted: {attempt_doc}"
            ));
        }
        if as_str(&attempt_doc, "/state", "b2 get_repair_attempt")? != "awaiting_edit" {
            return Err(format!("b2 get_repair_attempt: wrong state: {attempt_doc}"));
        }
        if at(&attempt_doc, "/packet", "b2 get_repair_attempt")? != &first {
            return Err(
                "b2 get_repair_attempt: packet differs from the prepare document".to_string(),
            );
        }
        let reply = session.call(
            "b2-receipt",
            &receipt_tool,
            json!({ "receipt_id": attempt }),
        )?;
        let receipt = tool_success(&reply, "b2 get_receipt_status")?.clone();
        if as_str(&receipt, "/status", "b2 get_receipt_status")? != "awaiting_edit" {
            return Err(format!("b2 get_receipt_status: wrong status: {receipt}"));
        }
        if as_str(&receipt, "/receipt_id", "b2 get_receipt_status")? != attempt {
            return Err(format!("b2 get_receipt_status: receipt drifted: {receipt}"));
        }
        if at(&receipt, "/receipt", "b2 get_receipt_status")? != &Value::Null {
            return Err(format!(
                "b2 get_receipt_status: session receipt is not null: {receipt}"
            ));
        }
        let reply = session.call(
            "b2-gap-linked",
            &gap_tool,
            json!({ "canonical_id": canonical }),
        )?;
        let linked = tool_success(&reply, "b2 get_gap after prepare")?.clone();
        let expected = format!("ripr://repair-attempt/{attempt}");
        if as_str(
            &linked,
            "/item/links/repair_attempt",
            "b2 get_gap after prepare",
        )? != expected
        {
            return Err(format!(
                "b2 get_gap after prepare: repair link did not bind: {linked}"
            ));
        }
    } else {
        nonempty_str(&first, "/ineligibility/reason", "b2 prepare_repair")?;
        if at(&first, "/attempt", "b2 prepare_repair")? != &Value::Null {
            return Err(format!(
                "b2 prepare_repair: unready packet carries an attempt: {first}"
            ));
        }
        let reply = session.call(
            "b2-attempt-unknown",
            &attempt_tool,
            json!({ "attempt_id": UNKNOWN_ATTEMPT_ID }),
        )?;
        require_failure_code(&reply, "b2 get_repair_attempt unknown", "attempt_not_found")?;
        let reply = session.call(
            "b2-receipt-unknown",
            &receipt_tool,
            json!({ "receipt_id": UNKNOWN_ATTEMPT_ID }),
        )?;
        require_failure_code(&reply, "b2 get_receipt_status unknown", "attempt_not_found")?;
    }

    let audit = session.finish(bound)?;
    if audit.frames == 0 || audit.max_frame_bytes == 0 {
        return Err("b2: session audit is empty".to_string());
    }

    // The journey may warm the producer fact cache under target/ and
    // nothing else; repair or receipt artifacts anywhere are a violation.
    let tree_after = snapshot_tree(&fixture.root)?;
    for (path, hash) in &tree_after {
        if path.contains("repair-attempt") {
            return Err(format!(
                "b2: journey created a repair-attempt artifact at {path}"
            ));
        }
        match tree_before.get(path) {
            Some(expected) if expected == hash => {}
            _ if path.starts_with("target/") => {}
            Some(_) => return Err(format!("b2: journey modified {path} outside target/")),
            None => return Err(format!("b2: journey added {path} outside target/")),
        }
    }
    for path in tree_before.keys() {
        if !tree_after.contains_key(path) {
            return Err(format!("b2: journey deleted {path}"));
        }
    }
    Ok(())
}

#[test]
fn b3a_mcp_negative_authority_pre_refresh() -> Result<(), String> {
    let fixture = install_fixture("b3a")?;
    let tree_before = snapshot_tree(&fixture.root)?;
    let mut session = McpSession::spawn(&fixture.root)?;
    session.initialize()?;

    let reply = session.call("b3a-status", "ripr_workspace_status", json!({}))?;
    let status = tool_success(&reply, "b3a status")?.clone();
    if as_str(&status, "/workspace/workspace_state", "b3a status")? != "ready" {
        return Err(format!("b3a status: fixture root is not ready: {status}"));
    }
    require_authority_none(&status)?;
    if as_str(&status, "/session/attempt_state", "b3a status")? != "no_snapshot" {
        return Err(format!(
            "b3a status: fresh session already has a snapshot: {status}"
        ));
    }
    let bound = response_bound(&status)?;

    let reply = session.call("b3a-list", "ripr_list_gaps", json!({}))?;
    require_failure_code(&reply, "b3a list_gaps", "no_snapshot")?;
    let reply = session.call(
        "b3a-gap",
        "ripr_get_gap",
        json!({ "canonical_id": UNKNOWN_CANONICAL_ID }),
    )?;
    require_failure_code(&reply, "b3a get_gap", "no_snapshot")?;
    let reply = session.call(
        "b3a-prepare",
        "ripr_prepare_repair",
        json!({ "canonical_id": UNKNOWN_CANONICAL_ID }),
    )?;
    require_failure_code(&reply, "b3a prepare_repair", "no_snapshot")?;
    let reply = session.call(
        "b3a-attempt",
        "ripr_get_repair_attempt",
        json!({ "attempt_id": UNKNOWN_ATTEMPT_ID }),
    )?;
    require_failure_code(&reply, "b3a get_repair_attempt", "attempt_not_found")?;
    let reply = session.call(
        "b3a-receipt",
        "ripr_get_receipt_status",
        json!({ "receipt_id": UNKNOWN_ATTEMPT_ID }),
    )?;
    require_failure_code(&reply, "b3a get_receipt_status", "attempt_not_found")?;

    // Dispatch edge: unknown tools and unknown arguments are protocol
    // errors, never silent nulls.
    let reply = session.call("b3a-unknown-tool", "ripr_everything", json!({}))?;
    if protocol_error_code(&reply, "b3a unknown tool")? != -32601 {
        return Err(format!("b3a unknown tool: wrong protocol code: {reply}"));
    }
    let reply = session.call(
        "b3a-unknown-argument",
        "ripr_workspace_status",
        json!({ "verbose": true }),
    )?;
    if protocol_error_code(&reply, "b3a unknown argument")? != -32602 {
        return Err(format!(
            "b3a unknown argument: wrong protocol code: {reply}"
        ));
    }

    // Oversize twin: a large-but-under-input-cap argument fails closed
    // typed, with a bounded reply, and the session keeps serving.
    let oversize = "g".repeat(OVERSIZE_ARGUMENT_LEN);
    let reply = session.call(
        "b3a-oversize",
        "ripr_get_gap",
        json!({ "canonical_id": oversize }),
    )?;
    require_failure_code(&reply, "b3a oversize canonical_id", "no_snapshot")?;
    let reply = session.call("b3a-survivor", "ripr_workspace_status", json!({}))?;
    let survivor = tool_success(&reply, "b3a survivor status")?.clone();
    if as_str(&survivor, "/session/attempt_state", "b3a survivor status")? != "no_snapshot" {
        return Err(format!(
            "b3a survivor status: session state moved: {survivor}"
        ));
    }
    if at(&survivor, "/session/last_failure", "b3a survivor status")? != &Value::Null {
        return Err(format!(
            "b3a survivor status: negative probes recorded a failure: {survivor}"
        ));
    }

    let _audit = session.finish(bound)?;
    require_tree_equal(&tree_before, &snapshot_tree(&fixture.root)?, "b3a")?;
    Ok(())
}

#[test]
fn b3b_mcp_negative_authority_post_refresh() -> Result<(), String> {
    let fixture = install_fixture("b3b")?;
    let mut session = McpSession::spawn(&fixture.root)?;
    session.initialize()?;

    let reply = session.call("b3b-status", "ripr_workspace_status", json!({}))?;
    let status = tool_success(&reply, "b3b status")?.clone();
    let bound = response_bound(&status)?;

    let reply = session.call("b3b-refresh", "ripr_refresh", json!({}))?;
    let refresh = tool_success(&reply, "b3b refresh")?.clone();
    let (snapshot, _, _) = require_completed_refresh(&refresh)?;

    let reply = session.call("b3b-list", "ripr_list_gaps", json!({}))?;
    let list = tool_success(&reply, "b3b list_gaps")?.clone();
    let canonical = nonempty_str(&list, "/items/0/canonical_id", "b3b list_gaps")?.to_string();

    // The refresh legitimately warms the fact cache; the negative probes
    // after this point must not move a single byte.
    let tree_after_refresh = snapshot_tree(&fixture.root)?;

    let reply = session.call(
        "b3b-list-stale",
        "ripr_list_gaps",
        json!({ "snapshot_id": STALE_SNAPSHOT_ID }),
    )?;
    let stale = require_failure_code(&reply, "b3b list_gaps stale", "stale_snapshot")?;
    if as_str(
        &stale,
        "/failure/data/current_snapshot_id",
        "b3b list_gaps stale",
    )? != snapshot
    {
        return Err(format!(
            "b3b list_gaps stale: current id not echoed: {stale}"
        ));
    }
    let reply = session.call(
        "b3b-gap-stale",
        "ripr_get_gap",
        json!({ "canonical_id": canonical, "snapshot_id": STALE_SNAPSHOT_ID }),
    )?;
    let stale = require_failure_code(&reply, "b3b get_gap stale", "stale_snapshot")?;
    if as_str(
        &stale,
        "/failure/data/current_snapshot_id",
        "b3b get_gap stale",
    )? != snapshot
    {
        return Err(format!("b3b get_gap stale: current id not echoed: {stale}"));
    }
    let reply = session.call(
        "b3b-prepare-stale",
        "ripr_prepare_repair",
        json!({ "canonical_id": canonical, "snapshot_id": STALE_SNAPSHOT_ID }),
    )?;
    let stale = require_failure_code(&reply, "b3b prepare_repair stale", "stale_snapshot")?;
    if as_str(
        &stale,
        "/failure/data/current_snapshot_id",
        "b3b prepare_repair stale",
    )? != snapshot
    {
        return Err(format!(
            "b3b prepare_repair stale: current id not echoed: {stale}"
        ));
    }

    let reply = session.call(
        "b3b-gap-unknown",
        "ripr_get_gap",
        json!({ "canonical_id": UNKNOWN_CANONICAL_ID }),
    )?;
    require_failure_code(&reply, "b3b get_gap unknown", "item_not_found")?;
    let reply = session.call(
        "b3b-prepare-unknown",
        "ripr_prepare_repair",
        json!({ "canonical_id": UNKNOWN_CANONICAL_ID }),
    )?;
    require_failure_code(&reply, "b3b prepare_repair unknown", "item_not_found")?;
    let reply = session.call(
        "b3b-attempt-unknown",
        "ripr_get_repair_attempt",
        json!({ "attempt_id": UNKNOWN_ATTEMPT_ID }),
    )?;
    require_failure_code(
        &reply,
        "b3b get_repair_attempt unknown",
        "attempt_not_found",
    )?;
    let reply = session.call(
        "b3b-receipt-unknown",
        "ripr_get_receipt_status",
        json!({ "receipt_id": UNKNOWN_ATTEMPT_ID }),
    )?;
    require_failure_code(
        &reply,
        "b3b get_receipt_status unknown",
        "attempt_not_found",
    )?;

    // Oversize twin after a snapshot exists: the item lookup fails
    // closed typed instead of served or dropped.
    let oversize = "g".repeat(OVERSIZE_ARGUMENT_LEN);
    let reply = session.call(
        "b3b-oversize",
        "ripr_get_gap",
        json!({ "canonical_id": oversize }),
    )?;
    require_failure_code(&reply, "b3b oversize canonical_id", "item_not_found")?;
    let reply = session.call("b3b-survivor", "ripr_workspace_status", json!({}))?;
    let survivor = tool_success(&reply, "b3b survivor status")?.clone();
    if as_str(&survivor, "/session/attempt_state", "b3b survivor status")? != "completed" {
        return Err(format!(
            "b3b survivor status: session left completed: {survivor}"
        ));
    }

    let _audit = session.finish(bound)?;
    require_tree_equal(&tree_after_refresh, &snapshot_tree(&fixture.root)?, "b3b")?;
    Ok(())
}

#[test]
fn b3c_mcp_pipelined_coherence() -> Result<(), String> {
    let fixture = install_fixture("b3c")?;
    let mut session = McpSession::spawn(&fixture.root)?;
    session.initialize()?;

    let reply = session.call("b3c-status", "ripr_workspace_status", json!({}))?;
    let status = tool_success(&reply, "b3c status")?.clone();
    let bound = response_bound(&status)?;

    // One immediate batch with no delay between lines. The transport
    // admits a single request at a time (`Admission`: one typed request
    // stays admitted until its reply frame flushes), so `analysis_in_flight`
    // is unreachable over one stdio connection by design; the executable
    // property is that pipelined bytes stay sequential and coherent:
    // refresh completes and every follower binds that snapshot.
    let replies = session.call_batch(&[
        ("b3c-refresh", "ripr_refresh", json!({})),
        ("b3c-list", "ripr_list_gaps", json!({})),
        (
            "b3c-attempt",
            "ripr_get_repair_attempt",
            json!({ "attempt_id": UNKNOWN_ATTEMPT_ID }),
        ),
        (
            "b3c-receipt",
            "ripr_get_receipt_status",
            json!({ "receipt_id": UNKNOWN_ATTEMPT_ID }),
        ),
        ("b3c-status-after", "ripr_workspace_status", json!({})),
    ])?;
    if replies.len() != 5 {
        return Err("b3c pipeline: batch lost replies".to_string());
    }

    let refresh = tool_success(&replies[0], "b3c refresh")?.clone();
    let (snapshot, _, total) = require_completed_refresh(&refresh)?;

    let list = tool_success(&replies[1], "b3c pipelined list_gaps")?.clone();
    require_consistent_counts(&list, &snapshot, total)?;
    require_failure_code(
        &replies[2],
        "b3c pipelined get_repair_attempt",
        "attempt_not_found",
    )?;
    require_failure_code(
        &replies[3],
        "b3c pipelined get_receipt_status",
        "attempt_not_found",
    )?;
    let after = tool_success(&replies[4], "b3c pipelined status")?.clone();
    if as_str(&after, "/session/attempt_state", "b3c pipelined status")? != "completed" {
        return Err(format!("b3c pipelined status: not completed: {after}"));
    }
    let rebound = as_str(
        &after,
        "/session/last_completed_snapshot/snapshot_id",
        "b3c pipelined status",
    )?;
    if rebound != snapshot {
        return Err(format!(
            "b3c pipelined status: rebound {rebound:?}, want {snapshot:?}: {after}"
        ));
    }

    let _audit = session.finish(bound)?;
    Ok(())
}
