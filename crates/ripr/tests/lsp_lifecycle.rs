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
