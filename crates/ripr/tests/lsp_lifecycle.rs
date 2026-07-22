//! LSP lifecycle conformance tests against the real `ripr lsp --stdio`
//! binary (issue #2030).
//!
//! These tests spawn the actual server process and speak JSON-RPC over
//! stdio with `Content-Length` framing, pinning the standard LSP lifecycle:
//!
//! 1. requests before `initialize` fail with `-32002` (server not initialized);
//! 2. `initialize` is accepted exactly once (a duplicate fails `-32600`);
//! 3. the `initialized` notification completes the handshake (and cannot
//!    substitute for `initialize`);
//! 4. normal requests are answered after the handshake;
//! 5. `shutdown` returns a `null` result and transitions state;
//! 6. requests after `shutdown` fail `-32600` (InvalidRequest per LSP);
//! 7. the `exit` notification terminates the process;
//! 8. stdin EOF / malformed frames do not hang the server.
//!
//! Named limitations (verified against tower-lsp-server 0.23, the transport
//! stack in `crates/ripr/src/lsp.rs`):
//!
//! - `InitializeParams.processId` monitoring is NOT implemented: neither
//!   tower-lsp-server 0.23 nor `lsp/backend.rs` watches the client process,
//!   so a crashed editor leaves the server running until stdin closes.
//!   `initialize_accepts_client_process_id_without_monitoring` documents the
//!   accepted-but-unmonitored behavior; detecting a dead-but-pipe-holding
//!   parent is out of scope.
//! - LSP §exit says a server that receives `exit` without a prior `shutdown`
//!   "should exit with an error code". tower-lsp-server treats `exit` as an
//!   unconditional stop and `ripr lsp` returns success either way, so the
//!   pinned exit code is 0 in both orders. This diverges from the letter of
//!   the spec; `exit_without_shutdown_still_exits_zero` pins the actual
//!   behavior so the divergence is explicit, not accidental.
//!
//! Every spawned server is terminated and reaped by `LspSession::drop`, so a
//! failing test cannot orphan a process that would hold a file lock on the
//! binary and break later builds.

use std::io::Read;
use std::io::Write;
use std::process::{Child, ChildStdin, Command, ExitStatus, Stdio};
use std::sync::mpsc::{Receiver, RecvTimeoutError, sync_channel};
use std::time::{Duration, Instant};

/// Generous per-read budget so a hung server fails fast with a clear
/// message instead of blocking CI; far above healthy response latency.
const RESPONSE_TIMEOUT: Duration = Duration::from_secs(15);
/// Budget for the process to terminate after `exit`/EOF.
const EXIT_TIMEOUT: Duration = Duration::from_secs(15);

const SERVER_NOT_INITIALIZED: i64 = -32002;
const INVALID_REQUEST: i64 = -32600;
const PARSE_ERROR: i64 = -32700;

/// Events forwarded by the stdout reader thread. `Failed` carries a framing
/// or JSON error observed on the wire; channel disconnect signals EOF.
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
        if let Some(value) = line
            .get(..NAME.len())
            .filter(|name| name.eq_ignore_ascii_case(NAME))
            .and_then(|_| line.get(NAME.len()..))
        {
            return value
                .trim()
                .parse::<usize>()
                .map_err(|err| format!("invalid Content-Length in {headers:?}: {err}"));
        }
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
/// Dropping kills and reaps the child so tests never orphan a server.
struct LspSession {
    child: Child,
    stdin: Option<ChildStdin>,
    events: Receiver<WireEvent>,
    next_id: u64,
}

impl LspSession {
    fn spawn() -> Result<Self, String> {
        let binary = env!("CARGO_BIN_EXE_ripr");
        let mut child = Command::new(binary)
            .args(["lsp", "--stdio"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            // Tracing output is not part of the wire contract; dropping
            // stderr also removes any chance of a full pipe stalling the
            // server under test.
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
        let id = self.next_id;
        self.next_id += 1;
        let message = serde_json::json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": method,
            "params": params,
        });
        self.send_frame(message.to_string().as_bytes())?;
        self.await_response(id, RESPONSE_TIMEOUT)
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

    /// Returns the response correlated to `id`, skipping server-to-client
    /// notifications. Any other response id is a protocol violation and
    /// fails the test with the offending message captured.
    fn await_response(&mut self, id: u64, timeout: Duration) -> Result<serde_json::Value, String> {
        let deadline = Instant::now() + timeout;
        loop {
            let message = self.await_message(deadline, &format!("response id {id}"))?;
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

    /// Returns the next message of any kind before `deadline`.
    fn await_message(
        &mut self,
        deadline: Instant,
        waiting_for: &str,
    ) -> Result<serde_json::Value, String> {
        let remaining = deadline
            .checked_duration_since(Instant::now())
            .ok_or_else(|| format!("timed out waiting {RESPONSE_TIMEOUT:?} for {waiting_for}"))?;
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

    fn close_stdin(&mut self) {
        self.stdin.take();
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

impl Drop for LspSession {
    fn drop(&mut self) {
        // An orphaned `ripr lsp --stdio` server holds file locks and breaks
        // later builds; always kill and reap, ignoring "already exited".
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn initialize_params() -> serde_json::Value {
    serde_json::json!({
        "processId": null,
        "rootUri": null,
        "capabilities": {},
    })
}

fn hover_params() -> serde_json::Value {
    serde_json::json!({
        "textDocument": { "uri": "file:///ripr-lsp-lifecycle/nonexistent.rs" },
        "position": { "line": 0, "character": 0 },
    })
}

fn expect_result<'a>(
    response: &'a serde_json::Value,
    method: &str,
) -> Result<&'a serde_json::Value, String> {
    response
        .get("result")
        .ok_or_else(|| format!("expected a result for `{method}`, got: {response}"))
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

/// Full handshake: `initialize` request + `initialized` notification.
fn handshake(session: &mut LspSession) -> Result<(), String> {
    let response = session.request("initialize", initialize_params())?;
    expect_result(&response, "initialize")?;
    session.notify("initialized", Some(serde_json::json!({})))
}

/// Sends a request frame without awaiting its response (load generation).
/// Allocates the id from the session sequence so it cannot collide with a
/// later `request()` call; returns the id used.
fn fire(session: &mut LspSession, method: &str, params: serde_json::Value) -> Result<u64, String> {
    let id = session.next_id;
    session.next_id += 1;
    let message = serde_json::json!({
        "jsonrpc": "2.0",
        "id": id,
        "method": method,
        "params": params,
    });
    session.send_frame(message.to_string().as_bytes())?;
    Ok(id)
}

/// Collects the responses for `ids` in any arrival order, failing on a stray
/// response id (protocol violation) or timeout.
fn collect_responses(
    session: &mut LspSession,
    ids: &[u64],
    timeout: Duration,
) -> Result<Vec<serde_json::Value>, String> {
    let deadline = Instant::now() + timeout;
    let mut collected: Vec<serde_json::Value> = Vec::new();
    while collected.len() < ids.len() {
        let message = session.await_message(deadline, "queued responses")?;
        let is_response = message.get("result").is_some() || message.get("error").is_some();
        if !is_response {
            continue;
        }
        let id = message.get("id").and_then(serde_json::Value::as_u64);
        match id {
            Some(id) if ids.contains(&id) => collected.push(message),
            other => {
                return Err(format!(
                    "protocol violation: stray response id {other:?} while collecting {} responses",
                    ids.len()
                ));
            }
        }
    }
    Ok(collected)
}

/// Ask the server to stop and require exit code 0 within the budget.
fn exit_and_wait(session: &mut LspSession) -> Result<(), String> {
    session.notify("exit", None)?;
    let status = session.wait_exit(EXIT_TIMEOUT)?;
    if status.success() {
        return Ok(());
    }
    Err(format!(
        "expected exit code 0 after `exit` notification, got: {status}"
    ))
}

// ── 1. Pre-initialize request handling ──

#[test]
fn request_before_initialize_fails_server_not_initialized() -> Result<(), String> {
    let mut session = LspSession::spawn()?;
    let response = session.request("textDocument/hover", hover_params())?;
    expect_error(&response, "textDocument/hover", SERVER_NOT_INITIALIZED)?;
    // The rejection must not poison the server: a proper handshake still works.
    handshake(&mut session)?;
    exit_and_wait(&mut session)
}

#[test]
fn shutdown_before_initialize_fails_server_not_initialized() -> Result<(), String> {
    let mut session = LspSession::spawn()?;
    let response = session.request("shutdown", serde_json::Value::Null)?;
    expect_error(&response, "shutdown", SERVER_NOT_INITIALIZED)?;
    handshake(&mut session)?;
    exit_and_wait(&mut session)
}

// ── 2. `initialize` is accepted exactly once ──

#[test]
fn initialize_is_accepted_exactly_once() -> Result<(), String> {
    let mut session = LspSession::spawn()?;
    let first = session.request("initialize", initialize_params())?;
    let result = expect_result(&first, "initialize")?;
    if result.get("capabilities").is_none() {
        return Err(format!(
            "initialize result must carry server capabilities: {first}"
        ));
    }
    let second = session.request("initialize", initialize_params())?;
    expect_error(&second, "duplicate initialize", INVALID_REQUEST)?;
    // The server stays in the initialized state after rejecting the duplicate.
    let hover = session.request("textDocument/hover", hover_params())?;
    expect_result(&hover, "textDocument/hover")?;
    exit_and_wait(&mut session)
}

// ── 3. `initialized` notification transition ──

#[test]
fn initialized_notification_completes_handshake_without_a_response() -> Result<(), String> {
    let mut session = LspSession::spawn()?;
    let initialize = session.request("initialize", initialize_params())?;
    expect_result(&initialize, "initialize")?;
    session.notify("initialized", Some(serde_json::json!({})))?;
    // Notifications have no id, so the next response must correlate to the
    // next request's id; `await_response` fails on any stray response.
    let hover = session.request("textDocument/hover", hover_params())?;
    expect_result(&hover, "textDocument/hover")?;
    exit_and_wait(&mut session)
}

#[test]
fn initialized_notification_before_initialize_does_not_initialize() -> Result<(), String> {
    let mut session = LspSession::spawn()?;
    session.notify("initialized", Some(serde_json::json!({})))?;
    let response = session.request("textDocument/hover", hover_params())?;
    expect_error(&response, "textDocument/hover", SERVER_NOT_INITIALIZED)?;
    handshake(&mut session)?;
    exit_and_wait(&mut session)
}

// ── 4. Normal request handling after initialize ──

#[test]
fn normal_request_after_initialize_returns_result() -> Result<(), String> {
    let mut session = LspSession::spawn()?;
    handshake(&mut session)?;
    let hover = session.request("textDocument/hover", hover_params())?;
    let result = expect_result(&hover, "textDocument/hover")?;
    let text = result
        .pointer("/contents/value")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| format!("hover result must carry markup contents: {hover}"))?;
    if !text.contains("ripr") {
        return Err(format!("hover contents should describe ripr, got: {text}"));
    }
    exit_and_wait(&mut session)
}

// ── 5/6. `shutdown` transition and request-after-shutdown rejection ──

#[test]
fn shutdown_returns_null_result_and_transitions() -> Result<(), String> {
    let mut session = LspSession::spawn()?;
    handshake(&mut session)?;
    let shutdown = session.request("shutdown", serde_json::Value::Null)?;
    let result = expect_result(&shutdown, "shutdown")?;
    if !result.is_null() {
        return Err(format!(
            "LSP requires a null result for `shutdown`, got: {shutdown}"
        ));
    }
    exit_and_wait(&mut session)
}

#[test]
fn requests_after_shutdown_are_rejected_invalid_request() -> Result<(), String> {
    let mut session = LspSession::spawn()?;
    handshake(&mut session)?;
    let shutdown = session.request("shutdown", serde_json::Value::Null)?;
    expect_result(&shutdown, "shutdown")?;
    // LSP §shutdown: after shutdown, any request except `exit` must fail
    // with InvalidRequest.
    let hover = session.request("textDocument/hover", hover_params())?;
    expect_error(&hover, "textDocument/hover", INVALID_REQUEST)?;
    let second_shutdown = session.request("shutdown", serde_json::Value::Null)?;
    expect_error(&second_shutdown, "duplicate shutdown", INVALID_REQUEST)?;
    exit_and_wait(&mut session)
}

// ── 7. `exit` notification terminates the process ──

#[test]
fn exit_after_shutdown_exits_zero() -> Result<(), String> {
    let mut session = LspSession::spawn()?;
    handshake(&mut session)?;
    let shutdown = session.request("shutdown", serde_json::Value::Null)?;
    expect_result(&shutdown, "shutdown")?;
    exit_and_wait(&mut session)
}

#[test]
fn exit_without_shutdown_still_exits_zero() -> Result<(), String> {
    // Divergence from LSP §exit (spec wants a non-zero code when no
    // `shutdown` was received): tower-lsp-server stops unconditionally and
    // `ripr lsp` reports success. Pinned here so the divergence is explicit;
    // see the module header for the full rationale.
    let mut session = LspSession::spawn()?;
    handshake(&mut session)?;
    session.notify("exit", None)?;
    let status = session.wait_exit(EXIT_TIMEOUT)?;
    if status.success() {
        return Ok(());
    }
    Err(format!(
        "pinned behavior: exit without shutdown currently exits 0, got: {status}"
    ))
}

// ── 8. EOF / malformed transport cleanup ──

#[test]
fn stdin_eof_lets_the_process_exit_on_its_own() -> Result<(), String> {
    let mut session = LspSession::spawn()?;
    handshake(&mut session)?;
    session.close_stdin();
    let status = session.wait_exit(EXIT_TIMEOUT)?;
    if status.success() {
        return Ok(());
    }
    Err(format!(
        "expected exit code 0 after stdin EOF, got: {status}"
    ))
}

#[test]
fn stdin_eof_before_initialize_still_exits() -> Result<(), String> {
    let mut session = LspSession::spawn()?;
    session.close_stdin();
    let status = session.wait_exit(EXIT_TIMEOUT)?;
    if status.success() {
        return Ok(());
    }
    Err(format!(
        "expected exit code 0 when stdin closes before initialize, got: {status}"
    ))
}

#[test]
fn malformed_frame_yields_parse_error_then_the_server_exits() -> Result<(), String> {
    let mut session = LspSession::spawn()?;
    // `Content-Length` is mandatory and must be an integer; this frame is
    // malformed at the transport layer, before JSON-RPC dispatch.
    session.send_raw(b"Content-Length: nope\r\n\r\n")?;
    let message =
        session.await_message(Instant::now() + RESPONSE_TIMEOUT, "parse-error response")?;
    if message
        .pointer("/error/code")
        .and_then(serde_json::Value::as_i64)
        != Some(PARSE_ERROR)
    {
        return Err(format!(
            "expected a -32700 parse error for a malformed frame, got: {message}"
        ));
    }
    if !message.get("id").is_some_and(serde_json::Value::is_null) {
        return Err(format!(
            "parse-error response must carry a null id (no request to correlate), got: {message}"
        ));
    }
    // The server must not hang after a malformed frame: tokio-util's
    // FramedRead fuses the stream after the first decode error
    // (`has_errored` in framed_impl.rs), so tower-lsp-server's read loop
    // ends and the process exits on its own with code 0 even though this
    // test still holds stdin open. Frame-level recovery therefore does NOT
    // happen on the wire (the codec's own recover-after-error unit test is
    // decoder-level only); the pinned contract is "respond -32700, then
    // exit promptly", which is the no-hang behavior issue #2030 requires.
    let status = session.wait_exit(EXIT_TIMEOUT)?;
    if status.success() {
        return Ok(());
    }
    Err(format!(
        "expected exit code 0 after a malformed frame, got: {status}"
    ))
}

// ── `InitializeParams.processId` (documented limitation) ──

#[test]
fn initialize_accepts_client_process_id_without_monitoring() -> Result<(), String> {
    // Named limitation (see module header): the server accepts a live
    // `processId` but neither tower-lsp-server 0.23 nor `lsp/backend.rs`
    // monitors it, so a dead client that holds stdin open is never noticed.
    // This test pins only that a real pid is accepted and the lifecycle is
    // unchanged; it deliberately does NOT fabricate monitoring evidence.
    let mut session = LspSession::spawn()?;
    let mut params = initialize_params();
    params["processId"] = serde_json::Value::from(std::process::id());
    let response = session.request("initialize", params)?;
    expect_result(&response, "initialize with processId")?;
    session.notify("initialized", Some(serde_json::json!({})))?;
    exit_and_wait(&mut session)
}

// ── 9. Transport and typed-payload bounds (issue #2034) ──
//
// Adversarial framing/payload cases against the same real binary. Every
// negative asserts a bounded wire response (or bounded rejection), prompt
// clean termination where framing cannot recover, and a server that starts
// no analysis, Git, or filesystem work for the rejected input: the rejected
// paths run before dispatch, and the follow-up healthy request in each test
// proves no poisoned or half-applied state. Peak-memory classes are pinned
// by construction (rejection happens before the declared body is read or
// the typed payload is iterated), not by one-host RSS measurement.

const INVALID_PARAMS: i64 = -32602;

/// Awaits one bounded framing-level rejection (null id, bounded message),
/// then requires the process to exit on its own: tokio-util's FramedRead
/// fuses after the first decode error, so an ingress bound trip ends the
/// session exactly like a malformed frame.
fn expect_bounded_frame_rejection(session: &mut LspSession, case: &str) -> Result<(), String> {
    let message = session.await_message(Instant::now() + RESPONSE_TIMEOUT, case)?;
    if !message.get("id").is_some_and(serde_json::Value::is_null) {
        return Err(format!(
            "{case}: framing rejection must carry a null id, got: {message}"
        ));
    }
    let code = message
        .pointer("/error/code")
        .and_then(serde_json::Value::as_i64);
    if code != Some(PARSE_ERROR) && code != Some(INVALID_REQUEST) {
        return Err(format!("{case}: expected -32700 or -32600, got: {message}"));
    }
    let rendered = message.to_string();
    if rendered.len() > 4096 {
        return Err(format!(
            "{case}: rejection response must be bounded, got {} bytes",
            rendered.len()
        ));
    }
    let status = session.wait_exit(EXIT_TIMEOUT)?;
    if status.success() {
        return Ok(());
    }
    Err(format!(
        "{case}: expected exit code 0 after rejection, got: {status}"
    ))
}

/// Awaits a typed -32602 rejection for the in-flight request id, asserting
/// the message is bounded and carries no attacker-controlled payload.
fn expect_bounded_invalid_params(
    session: &mut LspSession,
    id: u64,
    case: &str,
    attacker_marker: &str,
) -> Result<(), String> {
    let response = session.await_response(id, RESPONSE_TIMEOUT)?;
    expect_error(&response, case, INVALID_PARAMS)?;
    let message = response
        .pointer("/error/message")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| format!("{case}: rejection must carry a message: {response}"))?;
    if message.len() > 256 {
        return Err(format!(
            "{case}: rejection message must be bounded, got {} bytes",
            message.len()
        ));
    }
    if !attacker_marker.is_empty() && message.contains(attacker_marker) {
        return Err(format!(
            "{case}: rejection message must not echo attacker input: {message}"
        ));
    }
    Ok(())
}

#[test]
fn oversized_declared_content_length_is_rejected_before_body() -> Result<(), String> {
    // 16 MiB + 1 exceeds the transport message cap; no body is ever sent.
    // If the server waited for the declared body this test would time out
    // instead of receiving the bounded rejection.
    let mut session = LspSession::spawn()?;
    session.send_raw(b"Content-Length: 16777217\r\n\r\n")?;
    expect_bounded_frame_rejection(&mut session, "oversized Content-Length")
}

#[test]
fn oversized_header_block_is_rejected() -> Result<(), String> {
    // Well past the 8 KiB header-block cap with no terminator in sight.
    let mut session = LspSession::spawn()?;
    let mut frame = b"Content-Length: 2\r\nX-Pad: ".to_vec();
    frame.extend_from_slice(&[b'x'; 16 * 1024]);
    session.send_raw(&frame)?;
    expect_bounded_frame_rejection(&mut session, "oversized header block")
}

#[test]
fn integer_overflow_content_length_is_rejected() -> Result<(), String> {
    let mut session = LspSession::spawn()?;
    session.send_raw(b"Content-Length: 99999999999999999999\r\n\r\n")?;
    expect_bounded_frame_rejection(&mut session, "overflowing Content-Length")
}

#[test]
fn negative_content_length_is_rejected() -> Result<(), String> {
    let mut session = LspSession::spawn()?;
    session.send_raw(b"Content-Length: -1\r\n\r\n")?;
    expect_bounded_frame_rejection(&mut session, "negative Content-Length")
}

#[test]
fn deeply_nested_json_is_rejected_bounded() -> Result<(), String> {
    // serde_json's default recursion limit (128) must reject the body as a
    // bounded codec error; the nesting must not produce allocation or a
    // response proportional to depth.
    let mut session = LspSession::spawn()?;
    let depth = 5000;
    let mut params = String::with_capacity(depth * 2 + 2);
    params.extend(std::iter::repeat_n('[', depth));
    params.extend(std::iter::repeat_n(']', depth));
    let body = format!(
        "{{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"textDocument/hover\",\"params\":{params}}}"
    );
    session.send_frame(body.as_bytes())?;
    let message =
        session.await_message(Instant::now() + RESPONSE_TIMEOUT, "nested-JSON rejection")?;
    let code = message
        .pointer("/error/code")
        .and_then(serde_json::Value::as_i64);
    if code != Some(PARSE_ERROR) && code != Some(INVALID_REQUEST) {
        return Err(format!(
            "deeply nested JSON: expected -32700 or -32600, got: {message}"
        ));
    }
    let rendered = message.to_string();
    if rendered.len() > 4096 {
        return Err(format!(
            "deeply nested JSON: rejection must be bounded, got {} bytes",
            rendered.len()
        ));
    }
    if rendered.contains("[[[[") {
        return Err("deeply nested JSON: rejection must not echo the payload".to_string());
    }
    let status = session.wait_exit(EXIT_TIMEOUT)?;
    if status.success() {
        return Ok(());
    }
    Err(format!(
        "deeply nested JSON: expected exit code 0 after rejection, got: {status}"
    ))
}

#[test]
fn giant_previous_result_ids_are_rejected_and_server_stays_healthy() -> Result<(), String> {
    let mut session = LspSession::spawn()?;
    handshake(&mut session)?;
    // 5000 entries (with duplicates) exceed the 4096-entry typed bound.
    let ids: Vec<serde_json::Value> = (0..5000)
        .map(|index| {
            serde_json::json!({
                "uri": format!("file:///ripr-bounds/f{}.rs", index % 2500),
                "value": format!("attacker-marker-result-{index}")
            })
        })
        .collect();
    let response = session.request(
        "workspace/diagnostic",
        serde_json::json!({ "previousResultIds": ids }),
    )?;
    expect_error(&response, "oversized previousResultIds", INVALID_PARAMS)?;
    let message = response
        .pointer("/error/message")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| format!("rejection must carry a message: {response}"))?;
    if message.contains("attacker-marker") {
        return Err(format!("rejection must not echo attacker input: {message}"));
    }
    // A legitimately sized set is accepted, and the rejection left no
    // half-applied state: the server answers normally afterwards.
    let legit = session.request(
        "workspace/diagnostic",
        serde_json::json!({
            "previousResultIds": [
                {"uri": "file:///ripr-bounds/a.rs", "value": "digest-1"},
                {"uri": "file:///ripr-bounds/a.rs", "value": "digest-1"}
            ]
        }),
    )?;
    expect_result(&legit, "legit previousResultIds")?;
    let hover = session.request("textDocument/hover", hover_params())?;
    expect_result(&hover, "textDocument/hover")?;
    exit_and_wait(&mut session)
}

#[test]
fn previous_result_ids_oversized_value_is_rejected() -> Result<(), String> {
    let mut session = LspSession::spawn()?;
    handshake(&mut session)?;
    let id = fire(
        &mut session,
        "workspace/diagnostic",
        serde_json::json!({
            "previousResultIds": [
                {"uri": "file:///ripr-bounds/a.rs", "value": "v".repeat(2048)}
            ]
        }),
    )?;
    expect_bounded_invalid_params(&mut session, id, "oversized result-id value", "")?;
    exit_and_wait(&mut session)
}

#[test]
fn oversized_execute_command_arguments_are_rejected() -> Result<(), String> {
    let mut session = LspSession::spawn()?;
    handshake(&mut session)?;
    // One argument object carrying a 200 KiB identifier exceeds the 64 KiB
    // typed arguments bound.
    let id = fire(
        &mut session,
        "workspace/executeCommand",
        serde_json::json!({
            "command": "ripr.collectContext",
            "arguments": [{"gap_id": "g".repeat(200 * 1024)}]
        }),
    )?;
    expect_bounded_invalid_params(
        &mut session,
        id,
        "oversized executeCommand arguments",
        "gggg",
    )?;
    // Too many argument entries is rejected on the count bound alone.
    let id = fire(
        &mut session,
        "workspace/executeCommand",
        serde_json::json!({
            "command": "ripr.collectContext",
            "arguments": [{}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {}]
        }),
    )?;
    expect_bounded_invalid_params(&mut session, id, "too many executeCommand arguments", "")?;
    // A legitimate command still runs; rejection did not poison the session.
    let status = session.request(
        "workspace/executeCommand",
        serde_json::json!({"command": "ripr.collectWorkspaceStatus", "arguments": []}),
    )?;
    expect_result(&status, "ripr.collectWorkspaceStatus")?;
    exit_and_wait(&mut session)
}

#[test]
fn oversized_initialization_options_are_rejected_without_poisoning() -> Result<(), String> {
    let mut session = LspSession::spawn()?;
    let mut params = initialize_params();
    params["initializationOptions"] = serde_json::json!({"pad": "p".repeat(1024 * 1024)});
    let rejected = session.request("initialize", params)?;
    expect_error(
        &rejected,
        "oversized initialization_options",
        INVALID_PARAMS,
    )?;
    // The rejection happened before any state mutation: a clean initialize
    // on the same connection still completes the handshake.
    handshake(&mut session)?;
    let hover = session.request("textDocument/hover", hover_params())?;
    expect_result(&hover, "textDocument/hover")?;
    exit_and_wait(&mut session)
}

#[test]
fn concurrent_requests_and_cancel_stay_bounded_and_serviceable() -> Result<(), String> {
    let mut session = LspSession::spawn()?;
    handshake(&mut session)?;
    // Fire a burst beyond the in-flight concurrency bound whose aggregate
    // bytes also exceed the 8 KiB framing-header cap (~15 KiB across 96
    // frames): the transport queue (bounded, backpressured) and the framing
    // observer must absorb it without dropping, corrupting, or false-tripping
    // on legitimate coalesced reads.
    let mut ids = Vec::new();
    for _ in 0..96 {
        ids.push(fire(&mut session, "textDocument/hover", hover_params())?);
    }
    // A cancellation for an unknown/queued id is a lightweight lifecycle
    // notification; it must stay serviceable under load and must not break
    // the session.
    session.notify("$/cancelRequest", Some(serde_json::json!({"id": 9999})))?;
    let responses = collect_responses(&mut session, &ids, RESPONSE_TIMEOUT)?;
    for response in &responses {
        if response.get("result").is_none() && response.get("error").is_none() {
            return Err(format!(
                "burst response must be a result or error: {response}"
            ));
        }
    }
    // The server is still healthy after the burst.
    let hover = session.request("textDocument/hover", hover_params())?;
    expect_result(&hover, "textDocument/hover after burst")?;
    exit_and_wait(&mut session)
}

#[test]
fn shutdown_and_eof_during_request_load_exits_zero() -> Result<(), String> {
    let mut session = LspSession::spawn()?;
    handshake(&mut session)?;
    for _ in 0..32 {
        fire(&mut session, "textDocument/hover", hover_params())?;
    }
    // Shutdown + exit while requests are still queued: lifecycle messages
    // must stay serviceable and the process must terminate cleanly.
    session.notify("exit", None)?;
    let status = session.wait_exit(EXIT_TIMEOUT)?;
    if status.success() {
        return Ok(());
    }
    Err(format!(
        "expected exit code 0 for exit during request load, got: {status}"
    ))
}
