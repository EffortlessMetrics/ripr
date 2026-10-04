//! B4 LSP agent-protocol benchmark.
//!
//! Drives the real `ripr lsp --stdio` binary over JSON-RPC with
//! `Content-Length` framing through the agent-protocol wire contract:
//! `initialize` (with the `riprAgent` experimental block),
//! `ripr/listActionableItems`, the `workspace/executeCommand` collect
//! surface, `ripr.refresh`, `shutdown`, and `exit`. The script lives in
//! `benchmarks/agentic/lsp-protocol/script.json`; every step lands in a
//! bounded receipt under `target/ripr/reports/` (git-ignored).
//!
//! Oracle: pre-init `-32002`, duplicate `initialize` `-32600`, bad
//! collect arguments `-32602` naming the accepted shape, reserved agent
//! requests rejected, an unsupported client `riprAgent` major rejected
//! fail-closed (the server keeps advertising major 0), a bounded
//! receipt, and no hang on malformed frames.
//!
//! Every spawned server is terminated and reaped by `BenchSession::drop`.

use std::fs;
use std::io::Read;
use std::io::Write;
use std::path::PathBuf;
use std::process::{Child, ChildStdin, Command, ExitStatus, Stdio};
use std::sync::mpsc::{Receiver, RecvTimeoutError, sync_channel};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

/// Per-read budget so a hung server fails fast instead of blocking CI.
const RESPONSE_TIMEOUT: Duration = Duration::from_secs(15);
/// Budget for `ripr.refresh`, which runs workspace analysis.
const ANALYSIS_TIMEOUT: Duration = Duration::from_secs(90);
/// Budget for the process to terminate after `exit`/EOF.
const EXIT_TIMEOUT: Duration = Duration::from_secs(15);

const RECEIPT_PATH: &str = "target/ripr/reports/agentic-bench-lsp-receipt.json";

/// Events forwarded by the stdout reader thread.
enum WireEvent {
    Message(serde_json::Value),
    Failed(String),
}

/// Incremental `Content-Length` frame decoder for the server's stdout.
struct FrameReader<R> {
    inner: R,
    buffer: Vec<u8>,
}

fn find_subslice(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}

fn parse_content_length(headers: &str) -> Result<usize, String> {
    const NAME: &str = "Content-Length:";
    for line in headers.split("\r\n") {
        let Some(name) = line.get(..NAME.len()) else {
            continue;
        };
        if !name.eq_ignore_ascii_case(NAME) {
            continue;
        }
        let Some(value) = line.get(NAME.len()..) else {
            continue;
        };
        return value
            .trim()
            .parse::<usize>()
            .map_err(|err| format!("invalid Content-Length in {headers:?}: {err}"));
    }
    Err(format!("missing Content-Length header in {headers:?}"))
}

impl<R: Read> FrameReader<R> {
    fn read_message(&mut self) -> Result<Option<serde_json::Value>, String> {
        loop {
            if let Some(header_end) = find_subslice(&self.buffer, b"\r\n\r\n") {
                let headers = std::str::from_utf8(&self.buffer[..header_end])
                    .map_err(|err| format!("frame headers are not UTF-8: {err}"))?;
                let content_length = parse_content_length(headers)?;
                let frame_len = header_end + 4 + content_length;
                if self.buffer.len() >= frame_len {
                    let body = self.buffer[header_end + 4..frame_len].to_vec();
                    self.buffer.drain(..frame_len);
                    let text = String::from_utf8(body)
                        .map_err(|err| format!("frame body is not UTF-8: {err}"))?;
                    let value = serde_json::from_str(&text)
                        .map_err(|err| format!("frame body is not JSON: {err}; body: {text}"))?;
                    return Ok(Some(value));
                }
            }
            let mut chunk = [0_u8; 4096];
            let read = self
                .inner
                .read(&mut chunk)
                .map_err(|err| format!("reading server stdout: {err}"))?;
            if read == 0 {
                if self.buffer.is_empty() {
                    return Ok(None);
                }
                return Err(format!(
                    "server stdout hit EOF mid-frame with {} buffered byte(s)",
                    self.buffer.len()
                ));
            }
            self.buffer.extend_from_slice(&chunk[..read]);
        }
    }
}

/// A live `ripr lsp --stdio` process plus its framed stdout stream.
struct BenchSession {
    child: Child,
    stdin: Option<ChildStdin>,
    events: Receiver<WireEvent>,
    next_id: u64,
}

impl BenchSession {
    fn spawn() -> Result<Self, String> {
        let binary = env!("CARGO_BIN_EXE_ripr");
        let mut child = Command::new(binary)
            .args(["lsp", "--stdio"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|err| format!("failed to spawn `{binary} lsp --stdio`: {err}"))?;
        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| "spawned server is missing a stdin pipe".to_string())?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| "spawned server is missing a stdout pipe".to_string())?;
        let (sender, events) = sync_channel(64);
        std::thread::spawn(move || {
            let mut reader = FrameReader {
                inner: stdout,
                buffer: Vec::new(),
            };
            loop {
                match reader.read_message() {
                    Ok(Some(value)) => {
                        if sender.send(WireEvent::Message(value)).is_err() {
                            break;
                        }
                    }
                    Ok(None) => break,
                    Err(err) => {
                        let _ = sender.send(WireEvent::Failed(err));
                        break;
                    }
                }
            }
        });
        Ok(Self {
            child,
            stdin: Some(stdin),
            events,
            next_id: 1,
        })
    }

    fn send_frame(&mut self, body: &[u8]) -> Result<(), String> {
        let stdin = self
            .stdin
            .as_mut()
            .ok_or_else(|| "stdin is already closed".to_string())?;
        let header = format!("Content-Length: {}\r\n\r\n", body.len());
        stdin
            .write_all(header.as_bytes())
            .and_then(|()| stdin.write_all(body))
            .and_then(|()| stdin.flush())
            .map_err(|err| format!("writing frame to server stdin: {err}"))
    }

    fn send_raw(&mut self, bytes: &[u8]) -> Result<(), String> {
        let stdin = self
            .stdin
            .as_mut()
            .ok_or_else(|| "stdin is already closed".to_string())?;
        stdin
            .write_all(bytes)
            .and_then(|()| stdin.flush())
            .map_err(|err| format!("writing raw bytes to server stdin: {err}"))
    }

    fn request(
        &mut self,
        method: &str,
        params: serde_json::Value,
    ) -> Result<serde_json::Value, String> {
        self.request_with_timeout(method, params, RESPONSE_TIMEOUT)
    }

    fn request_with_timeout(
        &mut self,
        method: &str,
        params: serde_json::Value,
        timeout: Duration,
    ) -> Result<serde_json::Value, String> {
        let id = self.next_id;
        self.next_id += 1;
        let message = serde_json::json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": method,
            "params": params,
        });
        self.send_frame(message.to_string().as_bytes())?;
        self.await_response(id, timeout)
    }

    fn notify(&mut self, method: &str, params: Option<serde_json::Value>) -> Result<(), String> {
        let mut message = serde_json::json!({
            "jsonrpc": "2.0",
            "method": method,
        });
        if let Some(params) = params {
            message["params"] = params;
        }
        self.send_frame(message.to_string().as_bytes())
    }

    fn await_response(&mut self, id: u64, timeout: Duration) -> Result<serde_json::Value, String> {
        let deadline = Instant::now() + timeout;
        loop {
            let waiting_for = format!("response id {id}");
            let message = self.await_message(deadline, &waiting_for)?;
            let is_response = message.get("result").is_some() || message.get("error").is_some();
            if !is_response {
                continue;
            }
            if message.get("id").and_then(serde_json::Value::as_u64) == Some(id) {
                return Ok(message);
            }
            return Err(format!(
                "protocol violation: received response with id {:?} while awaiting id {id}: {message}",
                message.get("id")
            ));
        }
    }

    fn await_message(
        &mut self,
        deadline: Instant,
        waiting_for: &str,
    ) -> Result<serde_json::Value, String> {
        let remaining = deadline
            .checked_duration_since(Instant::now())
            .ok_or_else(|| format!("timed out waiting for {waiting_for}"))?;
        match self.events.recv_timeout(remaining) {
            Ok(WireEvent::Message(message)) => Ok(message),
            Ok(WireEvent::Failed(err)) => Err(format!(
                "stdout framing failed while awaiting {waiting_for}: {err}"
            )),
            Err(RecvTimeoutError::Timeout) => Err(format!("timed out waiting for {waiting_for}")),
            Err(RecvTimeoutError::Disconnected) => Err(format!(
                "server closed stdout (EOF or exit) while awaiting {waiting_for}"
            )),
        }
    }

    fn wait_exit(&mut self, timeout: Duration) -> Result<ExitStatus, String> {
        let deadline = Instant::now() + timeout;
        loop {
            match self.child.try_wait() {
                Ok(Some(status)) => return Ok(status),
                Ok(None) => {
                    if Instant::now() >= deadline {
                        return Err(format!(
                            "ripr lsp did not exit within {timeout:?} (terminated by Drop)"
                        ));
                    }
                    std::thread::sleep(Duration::from_millis(25));
                }
                Err(err) => return Err(format!("try_wait on server process failed: {err}")),
            }
        }
    }
}

impl Drop for BenchSession {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// One recorded wire step: outcome, serialized size, and latency.
struct StepRecord {
    step: String,
    method: String,
    outcome: String,
    bytes: usize,
    elapsed_ms: u64,
}

impl StepRecord {
    fn render(&self) -> serde_json::Value {
        serde_json::json!({
            "step": self.step,
            "method": self.method,
            "outcome": self.outcome,
            "bytes": self.bytes,
            "elapsed_ms": self.elapsed_ms,
        })
    }
}

fn outcome_of(response: &serde_json::Value) -> String {
    if let Some(code) = response
        .pointer("/error/code")
        .and_then(serde_json::Value::as_i64)
    {
        return format!("error:{code}");
    }
    if response.get("result").is_some() {
        return "result".to_string();
    }
    "malformed".to_string()
}

fn record(step: &str, method: &str, response: &serde_json::Value, started: Instant) -> StepRecord {
    let bytes = response.to_string().len();
    let elapsed_ms = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);
    StepRecord {
        step: step.to_string(),
        method: method.to_string(),
        outcome: outcome_of(response),
        bytes,
        elapsed_ms,
    }
}

fn script_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../benchmarks/agentic/lsp-protocol")
}

fn load_script() -> Result<serde_json::Value, String> {
    let path = script_root().join("script.json");
    let bytes = fs::read(&path).map_err(|err| format!("read {} failed: {err}", path.display()))?;
    let script: serde_json::Value = serde_json::from_slice(&bytes)
        .map_err(|err| format!("parse {} failed: {err}", path.display()))?;
    if script
        .get("schema_version")
        .and_then(serde_json::Value::as_str)
        != Some("agentic_lsp_bench.v1")
    {
        return Err("lsp-protocol script schema drifted".to_string());
    }
    Ok(script)
}

fn script_code(script: &serde_json::Value, name: &str) -> Result<i64, String> {
    script
        .pointer(&format!("/expected_codes/{name}"))
        .and_then(serde_json::Value::as_i64)
        .ok_or_else(|| format!("script lacks expected code {name}"))
}

fn script_text<'a>(script: &'a serde_json::Value, pointer: &str) -> Result<&'a str, String> {
    script
        .pointer(pointer)
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| format!("script lacks text at {pointer}"))
}

fn script_bound(script: &serde_json::Value, name: &str) -> Result<usize, String> {
    script
        .pointer(&format!("/bounds/{name}"))
        .and_then(serde_json::Value::as_u64)
        .and_then(|value| usize::try_from(value).ok())
        .ok_or_else(|| format!("script lacks bound {name}"))
}

fn expect_error(response: &serde_json::Value, method: &str, code: i64) -> Result<(), String> {
    let actual = response
        .pointer("/error/code")
        .and_then(serde_json::Value::as_i64);
    if actual == Some(code) {
        return Ok(());
    }
    Err(format!(
        "expected `{method}` to fail with error code {code}, got: {response}"
    ))
}

fn expect_result<'a>(
    response: &'a serde_json::Value,
    method: &str,
) -> Result<&'a serde_json::Value, String> {
    response
        .get("result")
        .ok_or_else(|| format!("expected a result for `{method}`, got: {response}"))
}

/// A typed `-32602` rejection: a JSON-RPC error without `result` whose
/// message names every expected fragment.
fn expect_typed_invalid_params(
    response: &serde_json::Value,
    case: &str,
    invalid_params: i64,
    fragments: &[String],
) -> Result<(), String> {
    if response.get("result").is_some() {
        return Err(format!(
            "{case}: must be a JSON-RPC error without `result`: {response}"
        ));
    }
    expect_error(response, case, invalid_params)?;
    let message = response
        .pointer("/error/message")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| format!("{case}: InvalidParams must carry a message: {response}"))?;
    for fragment in fragments {
        if !message.contains(fragment) {
            return Err(format!(
                "{case}: error message must contain `{fragment}`: {message}"
            ));
        }
    }
    Ok(())
}

/// A unique temp root for the benchmark session, removed on drop.
struct BenchFixtureRoot {
    path: PathBuf,
}

impl Drop for BenchFixtureRoot {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

fn bench_fixture_root() -> Result<BenchFixtureRoot, String> {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or(0);
    let path =
        std::env::temp_dir().join(format!("ripr-agentic-bench-{}-{stamp}", std::process::id()));
    fs::create_dir_all(&path).map_err(|err| format!("create bench fixture root failed: {err}"))?;
    Ok(BenchFixtureRoot { path })
}

/// Minimal `file://` URI for an absolute fixture path.
fn bench_file_uri(path: &std::path::Path) -> Result<String, String> {
    if !path.is_absolute() {
        return Err(format!("fixture root must be absolute: {}", path.display()));
    }
    let text = path
        .to_str()
        .ok_or_else(|| format!("fixture root is not UTF-8: {}", path.display()))?;
    let text = text.replace('\\', "/");
    let absolute = if text.starts_with('/') {
        text
    } else {
        format!("/{text}")
    };
    Ok(format!("file://{absolute}"))
}

fn write_bench_receipt(receipt: &serde_json::Value) -> Result<PathBuf, String> {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(RECEIPT_PATH);
    let parent = path
        .parent()
        .ok_or_else(|| format!("receipt path has no parent: {}", path.display()))?;
    fs::create_dir_all(parent).map_err(|err| format!("create receipt dir failed: {err}"))?;
    let rendered = serde_json::to_string_pretty(receipt)
        .map_err(|err| format!("render receipt failed: {err}"))?;
    fs::write(&path, format!("{rendered}\n"))
        .map_err(|err| format!("write receipt {} failed: {err}", path.display()))?;
    Ok(path)
}

#[test]
fn lsp_agent_protocol_benchmark_sequence() -> Result<(), String> {
    let script = load_script()?;
    let not_initialized = script_code(&script, "server_not_initialized")?;
    let invalid_request = script_code(&script, "invalid_request")?;
    let method_not_found = script_code(&script, "method_not_found")?;
    let invalid_params = script_code(&script, "invalid_params")?;
    let protocol_version = script_text(&script, "/protocol/protocol_version")?.to_string();
    let schema_version = script_text(&script, "/protocol/schema_version")?.to_string();
    let live_request = script_text(&script, "/protocol/live_request")?.to_string();
    let reserved_request = script_text(&script, "/protocol/reserved_request")?.to_string();
    let major_probe = script_text(&script, "/major_probe_protocol")?.to_string();
    let max_response = script_bound(&script, "max_response_bytes")?;
    let max_total = script_bound(&script, "max_total_bytes")?;
    let max_receipt = script_bound(&script, "max_receipt_bytes")?;
    let collect_commands: Vec<String> = script
        .pointer("/collect_commands")
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| "script lacks collect_commands".to_string())?
        .iter()
        .filter_map(serde_json::Value::as_str)
        .map(str::to_string)
        .collect();

    let fixture = bench_fixture_root()?;
    let root_uri = bench_file_uri(&fixture.path)?;
    let mut initialize_params = script
        .get("initialize")
        .cloned()
        .ok_or_else(|| "script lacks initialize params".to_string())?;
    initialize_params["rootUri"] = serde_json::Value::String(root_uri);

    let mut steps: Vec<StepRecord> = Vec::new();
    let mut session = BenchSession::spawn()?;

    // 1. Pre-init requests fail `-32002` without poisoning the server.
    let started = Instant::now();
    let pre_init = session.request(
        "textDocument/hover",
        serde_json::json!({
            "textDocument": { "uri": "file:///ripr-agentic-bench/nonexistent.rs" },
            "position": { "line": 0, "character": 0 },
        }),
    )?;
    steps.push(record(
        "pre_init_hover",
        "textDocument/hover",
        &pre_init,
        started,
    ));
    expect_error(&pre_init, "textDocument/hover", not_initialized)?;

    // 2. `initialize` advertises the agent-protocol surface exactly once.
    let started = Instant::now();
    let initialize = session.request("initialize", initialize_params.clone())?;
    steps.push(record("initialize", "initialize", &initialize, started));
    let initialize_result = expect_result(&initialize, "initialize")?;
    let advertised = initialize_result
        .pointer("/capabilities/executeCommandProvider/commands")
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| format!("initialize must advertise executeCommandProvider: {initialize}"))?;
    for command in &collect_commands {
        if !advertised
            .iter()
            .any(|entry| entry.as_str() == Some(command))
        {
            return Err(format!(
                "initialize must advertise `{command}`: {initialize_result}"
            ));
        }
    }
    check_agent_identity(initialize_result, &protocol_version, &schema_version)?;
    let started = Instant::now();
    let duplicate = session.request("initialize", initialize_params.clone())?;
    steps.push(record(
        "duplicate_initialize",
        "initialize",
        &duplicate,
        started,
    ));
    expect_error(&duplicate, "initialize", invalid_request)?;
    session.notify("initialized", Some(serde_json::json!({})))?;

    // 3. The one live agent request answers; reserved requests are rejected.
    let started = Instant::now();
    let live = session.request(&live_request, serde_json::json!({}))?;
    steps.push(record("agent_live_request", &live_request, &live, started));
    let live_result = expect_result(&live, &live_request)?;
    if live_result
        .pointer("/error/kind")
        .and_then(serde_json::Value::as_str)
        != Some("no_snapshot")
    {
        return Err(format!(
            "pre-refresh {live_request} must disclose the honest no_snapshot shape: {live}"
        ));
    }
    let started = Instant::now();
    let reserved = session.request(&reserved_request, serde_json::json!({}))?;
    steps.push(record(
        "agent_reserved_request",
        &reserved_request,
        &reserved,
        started,
    ));
    expect_error(&reserved, &reserved_request, method_not_found)?;

    // 4. Bad collect arguments fail typed `-32602`, never silent null.
    let bad_cases = script
        .get("bad_argument_cases")
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| "script lacks bad_argument_cases".to_string())?;
    for case in bad_cases {
        let label = case
            .get("label")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| format!("bad-argument case lacks a label: {case}"))?;
        let params = case
            .get("params")
            .cloned()
            .ok_or_else(|| format!("{label} lacks params"))?;
        let fragments: Vec<String> = case
            .get("fragments")
            .and_then(serde_json::Value::as_array)
            .ok_or_else(|| format!("{label} lacks fragments"))?
            .iter()
            .filter_map(serde_json::Value::as_str)
            .map(str::to_string)
            .collect();
        let started = Instant::now();
        let response = session.request("workspace/executeCommand", params)?;
        steps.push(record(
            label,
            "workspace/executeCommand",
            &response,
            started,
        ));
        expect_typed_invalid_params(&response, label, invalid_params, &fragments)?;
    }

    // 5. The collect journey runs without hanging; outcomes are recorded.
    let journey = script
        .get("journey_arguments")
        .and_then(serde_json::Value::as_object)
        .ok_or_else(|| "script lacks journey_arguments".to_string())?;
    for (command, arguments) in journey {
        if command == "refresh" {
            continue;
        }
        let started = Instant::now();
        let full_command = format!("ripr.{command}");
        let response = session.request(
            "workspace/executeCommand",
            serde_json::json!({"command": full_command, "arguments": arguments}),
        )?;
        let label = format!("journey_{command}");
        steps.push(record(
            &label,
            "workspace/executeCommand",
            &response,
            started,
        ));
        if outcome_of(&response) == "malformed" {
            return Err(format!(
                "{label}: server sent a malformed response: {response}"
            ));
        }
    }

    // 6. `refresh` answers within the analysis budget.
    let started = Instant::now();
    let refresh = session.request_with_timeout(
        "workspace/executeCommand",
        serde_json::json!({"command": "ripr.refresh", "arguments": []}),
        ANALYSIS_TIMEOUT,
    )?;
    steps.push(record(
        "refresh",
        "workspace/executeCommand",
        &refresh,
        started,
    ));
    if outcome_of(&refresh) == "malformed" {
        return Err(format!(
            "refresh: server sent a malformed response: {refresh}"
        ));
    }

    // 7. Clean shutdown terminates the process with code 0.
    let started = Instant::now();
    let shutdown = session.request("shutdown", serde_json::Value::Null)?;
    steps.push(record("shutdown", "shutdown", &shutdown, started));
    if !expect_result(&shutdown, "shutdown")?.is_null() {
        return Err(format!(
            "shutdown must return a null result, got: {shutdown}"
        ));
    }
    session.notify("exit", None)?;
    let status = session.wait_exit(EXIT_TIMEOUT)?;
    if !status.success() {
        return Err(format!("expected exit code 0 after `exit`, got: {status}"));
    }

    // 8. Major-gated version reject: an unsupported client `riprAgent`
    // major is ignored fail-closed — `initialize` still succeeds and the
    // server keeps advertising the supported major, never the bad one.
    let mut probe_params = initialize_params.clone();
    probe_params["capabilities"]["experimental"]["riprAgent"]["protocol"] =
        serde_json::Value::String(major_probe.clone());
    let mut probe = BenchSession::spawn()?;
    let started = Instant::now();
    let probe_initialize = probe.request("initialize", probe_params)?;
    steps.push(record(
        "major_probe_initialize",
        "initialize",
        &probe_initialize,
        started,
    ));
    let probe_result = expect_result(&probe_initialize, "initialize")?;
    check_agent_identity(probe_result, &protocol_version, &schema_version)?;
    probe.notify("initialized", Some(serde_json::json!({})))?;
    let started = Instant::now();
    let probe_live = probe.request(&live_request, serde_json::json!({}))?;
    steps.push(record(
        "major_probe_live_request",
        &live_request,
        &probe_live,
        started,
    ));
    expect_result(&probe_live, &live_request)?;
    // The unsupported major must fail closed in the parsed profile, not
    // just in the advertised identity: the receipt status projection must
    // carry a null ripr_agent block.
    let started = Instant::now();
    let probe_status = probe.request(
        "workspace/executeCommand",
        serde_json::json!({"command": "ripr.collectReceiptStatus", "arguments": []}),
    )?;
    steps.push(record(
        "major_probe_client_features",
        "workspace/executeCommand",
        &probe_status,
        started,
    ));
    let probe_status_result = expect_result(&probe_status, "workspace/executeCommand")?;
    if !matches!(
        probe_status_result.pointer("/client_features/ripr_agent"),
        Some(value) if value.is_null()
    ) {
        return Err(format!(
            "unsupported client riprAgent major must fail closed: {probe_status}"
        ));
    }
    let started = Instant::now();
    let probe_shutdown = probe.request("shutdown", serde_json::Value::Null)?;
    steps.push(record(
        "major_probe_shutdown",
        "shutdown",
        &probe_shutdown,
        started,
    ));
    expect_result(&probe_shutdown, "shutdown")?;
    probe.notify("exit", None)?;
    let probe_status = probe.wait_exit(EXIT_TIMEOUT)?;
    if !probe_status.success() {
        return Err(format!(
            "major probe: expected exit code 0, got: {probe_status}"
        ));
    }

    // 9. Bounded receipt: every response and the receipt itself fit the
    // script bounds.
    let mut total_bytes: usize = 0;
    for step in &steps {
        if step.bytes > max_response {
            return Err(format!(
                "step `{}` exceeded the response bound: {} > {max_response} bytes",
                step.step, step.bytes
            ));
        }
        total_bytes += step.bytes;
    }
    if total_bytes > max_total {
        return Err(format!(
            "wire total exceeded the bound: {total_bytes} > {max_total} bytes"
        ));
    }
    let receipt = serde_json::json!({
        "schema_version": "agentic_lsp_bench_receipt.v1",
        "binary": env!("CARGO_BIN_EXE_ripr"),
        "script": "benchmarks/agentic/lsp-protocol/script.json",
        "bounds": {"max_response_bytes": max_response, "max_total_bytes": max_total, "max_receipt_bytes": max_receipt},
        "total_bytes": total_bytes,
        "steps": steps.iter().map(StepRecord::render).collect::<Vec<_>>(),
        "shutdown": "ok",
        "exit_code": 0,
    });
    let rendered = serde_json::to_string_pretty(&receipt)
        .map_err(|err| format!("render receipt failed: {err}"))?;
    if rendered.len() > max_receipt {
        return Err(format!(
            "receipt exceeded the bound: {} > {max_receipt} bytes",
            rendered.len()
        ));
    }
    let path = write_bench_receipt(&receipt)?;
    if !path.is_file() {
        return Err(format!("receipt was not written: {}", path.display()));
    }
    Ok(())
}

/// The server's advertised `riprAgent` identity must equal the script's
/// supported versions — the major gate never adopts a client major.
fn check_agent_identity(
    initialize_result: &serde_json::Value,
    protocol_version: &str,
    schema_version: &str,
) -> Result<(), String> {
    let agent = initialize_result
        .pointer("/capabilities/experimental/riprAgent")
        .ok_or_else(|| {
            format!("initialize must advertise the riprAgent block: {initialize_result}")
        })?;
    for (key, expected) in [
        ("protocol_version", protocol_version),
        ("schema_version", schema_version),
    ] {
        let actual = agent.get(key).and_then(serde_json::Value::as_str);
        if actual != Some(expected) {
            return Err(format!(
                "riprAgent `{key}` must be {expected:?}, got: {agent}"
            ));
        }
    }
    Ok(())
}

#[test]
fn lsp_agent_protocol_malformed_frame_does_not_hang() -> Result<(), String> {
    let script = load_script()?;
    let parse_error = script_code(&script, "parse_error")?;
    let frames = script
        .get("malformed_frames")
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| "script lacks malformed_frames".to_string())?;
    if frames.is_empty() {
        return Err("script has no malformed frames".to_string());
    }
    for frame in frames {
        let bytes = frame
            .as_str()
            .ok_or_else(|| format!("malformed frame must be a string: {frame}"))?;
        let mut session = BenchSession::spawn()?;
        session.send_raw(bytes.as_bytes())?;
        let deadline = Instant::now() + RESPONSE_TIMEOUT;
        let message = session.await_message(deadline, "malformed-frame rejection")?;
        expect_error(&message, "malformed frame", parse_error)?;
        if !message.get("id").is_some_and(serde_json::Value::is_null) {
            return Err(format!(
                "parse-error response must carry a null id (no request to correlate), got: {message}"
            ));
        }
        // The transport fuses after the first decode error, so the server
        // exits on its own even though this test still holds stdin open.
        let status = session.wait_exit(EXIT_TIMEOUT)?;
        if !status.success() {
            return Err(format!(
                "expected exit code 0 after a malformed frame, got: {status}"
            ));
        }
    }
    Ok(())
}
