//! Transport resource bounds for `ripr lsp --stdio` (issue #2034).
//!
//! tower-lsp-server 0.23 constructs its `LanguageServerCodec` internally in
//! `Server::serve` and exposes no max-payload knob, so the framing cap cannot
//! be installed by configuring the transport. The sanctioned generic seam is
//! `Server::new(stdin, stdout, socket)`, which accepts any
//! `AsyncRead + Unpin` / `AsyncWrite` pair. The bounds below are therefore
//! enforced by byte-transparent adapters around those handles:
//!
//! - [`BoundedStdinReader`] observes the `Content-Length` framing as bytes
//!   pass through (never modifying them) and trips on a header block larger
//!   than [`MAX_HEADER_BYTES`] or a declared `Content-Length` above
//!   [`MAX_MESSAGE_BYTES`]. Tripping returns one `io::Error`; tokio-util's
//!   `FramedRead` fuses after the first decode error, so tower-lsp emits one
//!   bounded `-32700` JSON-RPC response with a null id and the read loop ends
//!   — the same pinned behavior class as a malformed frame
//!   (`crates/ripr/tests/lsp_lifecycle.rs`).
//! - [`BoundedStdoutWriter`] enforces [`WRITE_STALL_TIMEOUT`] of zero write
//!   progress. tower-lsp's egress is a rendezvous channel, so a client that
//!   stops reading wedges the write half with *bounded* memory but the
//!   process would otherwise wait for stdin EOF forever. On expiry the
//!   writer errors and trips the shared [`SessionTrip`], which wakes the
//!   stdin wrapper to return EOF so the server terminates cleanly instead of
//!   wedging on a dead reader.
//! - `Server::concurrency_level` is set explicitly to
//!   [`REQUEST_CONCURRENCY_LIMIT`] (the transport's own implicit default) so
//!   the in-flight request bound is owned and reviewed here. A value above 1
//!   keeps the built-in `$/cancelRequest` notification serviceable under
//!   load. tower-lsp additionally bounds queued request futures at an
//!   internal 100 with read-loop backpressure; expensive analysis work is
//!   already funneled through the refresh scheduler (one active attempt plus
//!   coalesced pending), so no second queue authority is introduced here.
//!
//! Named residuals (documented, not silently absent):
//!
//! - JSON nesting depth is enforced by serde_json's default recursion limit
//!   (128; the `unbounded_depth` feature is not enabled anywhere in this
//!   workspace), not by these adapters; an over-deep body is a bounded codec
//!   `Body` error.
//! - A duplicate `Content-Length` header is last-wins in the tower-lsp
//!   codec; [`BoundedStdinReader`] mirrors that when applying the cap so the
//!   two layers always agree on which value governs.
//! - Unknown extra headers are warn-only inside the vendored codec; the
//!   header-block byte cap bounds their cost.
//! - `initialize.processId` is accepted but not monitored (#2030); a dead
//!   client holding stdin open is now still bounded by the write-stall trip
//!   once output blocks, but a fully idle half-open session is only
//!   terminated by client EOF.
//! - The internal queued-future bound (100) is a tower-lsp constant and not
//!   configurable without forking the transport; it is bounded, which is the
//!   property this module relies on.

use std::io;
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll, Waker};
use std::time::Duration;

use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};
use tokio::time::{Instant as TokioInstant, Sleep};

/// Maximum decoded JSON-RPC message body accepted on the stdio transport.
///
/// Chosen from the largest legitimate ingress class: `textDocument/didOpen`
/// and `didSave` carry full document text, and multi-MiB generated Rust
/// sources are plausible. Egress is independently bounded by the diagnostic
/// budget (500 items and 64 KiB serialized caps in
/// `lsp/diagnostic_budget.rs`), so ingress dominates; 16 MiB gives wide
/// headroom over any real source file while capping the bytes a single frame
/// can force the codec's read buffer to hold.
pub(crate) const MAX_MESSAGE_BYTES: usize = 16 * 1024 * 1024;

/// Maximum bytes accepted for one framing header block (everything before
/// the `\r\n\r\n` terminator). Legitimate headers are under ~100 bytes
/// (`Content-Length` plus an optional `Content-Type`); 8 KiB is generous
/// headroom and bounds a client that streams header bytes forever.
pub(crate) const MAX_HEADER_BYTES: usize = 8 * 1024;

/// Maximum duration of zero write progress on stdout before the session is
/// considered wedged and terminated. Any successful write resets the clock,
/// so only a reader that has completely stopped draining trips this.
pub(crate) const WRITE_STALL_TIMEOUT: Duration = Duration::from_mins(2);

/// In-flight LSP request concurrency, passed to
/// `Server::concurrency_level`. Matches the tower-lsp implicit default (4);
/// declared here so the bound is owned and reviewed. Must stay above 1 so
/// `$/cancelRequest` and lifecycle notifications remain serviceable while
/// requests execute.
pub(crate) const REQUEST_CONCURRENCY_LIMIT: usize = 4;

/// Shared session-liveness flag between the stdin and stdout adapters.
///
/// The writer sets it when the write-stall deadline expires; the reader
/// observes it (its task is woken via a registered waker) and returns EOF so
/// tower-lsp's read loop ends and the process exits instead of waiting on a
/// dead client's stdin forever.
#[derive(Clone, Default)]
pub(crate) struct SessionTrip {
    inner: Arc<SessionTripInner>,
}

#[derive(Default)]
struct SessionTripInner {
    tripped: AtomicBool,
    wakers: Mutex<Vec<Waker>>,
}

impl SessionTrip {
    fn trip(&self) {
        self.inner.tripped.store(true, Ordering::SeqCst);
        if let Ok(mut wakers) = self.inner.wakers.lock() {
            for waker in wakers.drain(..) {
                waker.wake();
            }
        }
    }

    fn is_tripped(&self) -> bool {
        self.inner.tripped.load(Ordering::SeqCst)
    }

    fn register_waker(&self, waker: &Waker) {
        let Ok(mut wakers) = self.inner.wakers.lock() else {
            return;
        };
        // A trip that landed between the reader's `is_tripped` check and
        // this registration must not strand the new waker (#2185 review):
        // `trip` stores the flag before draining, so checking under the same
        // lock makes the two orderings safe — either the flag is already set
        // (wake now) or the pending trip will drain us next.
        if self.is_tripped() {
            drop(wakers);
            waker.wake_by_ref();
            return;
        }
        if !wakers.iter().any(|registered| registered.will_wake(waker)) {
            wakers.push(waker.clone());
        }
    }
}

/// Framing-observing `AsyncRead` adapter; see module docs.
pub(crate) struct BoundedStdinReader<R> {
    inner: R,
    session: SessionTrip,
    max_header_bytes: usize,
    max_message_bytes: usize,
    state: IngressState,
    tripped: bool,
    error_reported: bool,
    trip_reason: &'static str,
}

enum IngressState {
    /// Accumulating one header block until its `\r\n\r\n` terminator.
    Header(Vec<u8>),
    /// Inside a message body with this many bytes still expected.
    Body(usize),
}

const HEADER_TERMINATOR: &[u8] = b"\r\n\r\n";

impl<R> BoundedStdinReader<R> {
    fn with_limits(
        inner: R,
        session: SessionTrip,
        max_header_bytes: usize,
        max_message_bytes: usize,
    ) -> Self {
        Self {
            inner,
            session,
            max_header_bytes,
            max_message_bytes,
            state: IngressState::Header(Vec::new()),
            tripped: false,
            error_reported: false,
            trip_reason: "ripr lsp ingress bound exceeded",
        }
    }

    /// Scans `bytes` (pass-through, unmodified) and returns a static bounded
    /// reason when a framing bound is exceeded.
    fn observe(&mut self, bytes: &[u8]) -> Result<(), &'static str> {
        let mut offset = 0;
        while offset < bytes.len() {
            match std::mem::replace(&mut self.state, IngressState::Header(Vec::new())) {
                IngressState::Header(mut buffered) => {
                    let room = self.max_header_bytes.saturating_sub(buffered.len());
                    if room == 0 {
                        return Err(
                            "ripr lsp ingress bound: framing header block exceeds the byte cap",
                        );
                    }
                    let take = room.min(bytes.len() - offset);
                    buffered.extend_from_slice(&bytes[offset..offset + take]);
                    offset += take;
                    // Consume every complete frame already in the buffer: a
                    // burst can carry several frames in one read, and
                    // deferring a carried frame's scan to the next read would
                    // desynchronize the accounting and false-trip the header
                    // cap on legitimate load.
                    let mut body_remaining = 0_usize;
                    while let Some(position) = find_subslice(&buffered, HEADER_TERMINATOR) {
                        let declared = declared_content_length(&buffered[..position]);
                        let mut rest = buffered.split_off(position + 4);
                        buffered.clear();
                        match declared {
                            Some(length) if length > self.max_message_bytes => {
                                return Err(
                                    "ripr lsp ingress bound: declared Content-Length exceeds the message cap",
                                );
                            }
                            Some(length) => {
                                let body_in_buffer = rest.len().min(length);
                                let leftover = rest.split_off(body_in_buffer);
                                buffered = leftover;
                                if body_in_buffer < length {
                                    body_remaining = length - body_in_buffer;
                                    break;
                                }
                            }
                            // No parseable Content-Length: leave rejection to
                            // the tower-lsp codec, which fails
                            // deterministically (missing/invalid length ->
                            // one bounded parse error, then the stream
                            // fuses).
                            None => {
                                buffered = rest;
                            }
                        }
                    }
                    self.state = if body_remaining > 0 {
                        IngressState::Body(body_remaining)
                    } else {
                        IngressState::Header(buffered)
                    };
                }
                IngressState::Body(mut remaining) => {
                    let advance = remaining.min(bytes.len() - offset);
                    remaining -= advance;
                    offset += advance;
                    self.state = if remaining == 0 {
                        IngressState::Header(Vec::new())
                    } else {
                        IngressState::Body(remaining)
                    };
                }
            }
        }
        Ok(())
    }
}

impl<R: AsyncRead + Unpin> AsyncRead for BoundedStdinReader<R> {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        if self.session.is_tripped() && !self.tripped {
            // A write-stall trip ends the session: report EOF so tower-lsp's
            // read loop finishes and the process terminates cleanly instead
            // of waiting on a dead client's stdin forever.
            self.tripped = true;
            self.error_reported = true;
        }
        if self.tripped {
            if self.error_reported {
                // EOF after the single bounded error; tokio-util has fused
                // the framed stream by now anyway.
                return Poll::Ready(Ok(()));
            }
            self.error_reported = true;
            return Poll::Ready(Err(io::Error::new(
                io::ErrorKind::InvalidData,
                self.trip_reason,
            )));
        }
        let before = buf.filled().len();
        match Pin::new(&mut self.inner).poll_read(cx, buf) {
            Poll::Ready(Ok(())) => {
                let fresh = buf.filled()[before..].to_vec();
                if let Err(reason) = self.observe(&fresh) {
                    self.tripped = true;
                    self.trip_reason = reason;
                    self.session.trip();
                    // The freshly read bytes are already delivered in `buf`;
                    // the bounded error surfaces on the next poll, because an
                    // `AsyncRead` error must not accompany newly filled bytes.
                }
                Poll::Ready(Ok(()))
            }
            Poll::Ready(Err(err)) => Poll::Ready(Err(err)),
            Poll::Pending => {
                // Park with a registered waker so a write-stall trip can wake
                // this task even when the client never sends another byte.
                self.session.register_waker(cx.waker());
                Poll::Pending
            }
        }
    }
}

/// Write-stall-enforcing `AsyncWrite` adapter; see module docs.
pub(crate) struct BoundedStdoutWriter<W> {
    inner: W,
    session: SessionTrip,
    stall_timeout: Duration,
    stall_deadline: Option<Pin<Box<Sleep>>>,
    failed: bool,
}

impl<W> BoundedStdoutWriter<W> {
    fn with_stall_timeout(inner: W, session: SessionTrip, stall_timeout: Duration) -> Self {
        Self {
            inner,
            session,
            stall_timeout,
            stall_deadline: None,
            failed: false,
        }
    }

    fn poll_with_deadline<T>(
        &mut self,
        cx: &mut Context<'_>,
        attempt: impl FnOnce(Pin<&mut W>, &mut Context<'_>) -> Poll<io::Result<T>>,
    ) -> Poll<io::Result<T>>
    where
        W: AsyncWrite + Unpin,
    {
        if self.failed {
            return Poll::Ready(Err(stall_error()));
        }
        match attempt(Pin::new(&mut self.inner), cx) {
            Poll::Ready(result) => {
                self.stall_deadline = None;
                Poll::Ready(result)
            }
            Poll::Pending => {
                if self.stall_deadline.is_none() {
                    self.stall_deadline = Some(Box::pin(tokio::time::sleep_until(
                        TokioInstant::now() + self.stall_timeout,
                    )));
                }
                let deadline = self.stall_deadline.as_mut().map(|sleep| sleep.as_mut());
                if let Some(deadline) = deadline
                    && deadline.poll(cx).is_ready()
                {
                    self.failed = true;
                    self.session.trip();
                    return Poll::Ready(Err(stall_error()));
                }
                Poll::Pending
            }
        }
    }
}

fn stall_error() -> io::Error {
    io::Error::new(
        io::ErrorKind::TimedOut,
        "ripr lsp egress bound: client stopped reading stdout past the write-stall deadline",
    )
}

impl<W: AsyncWrite + Unpin> AsyncWrite for BoundedStdoutWriter<W> {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        self.poll_with_deadline(cx, |inner, cx| inner.poll_write(cx, buf))
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        self.poll_with_deadline(cx, |inner, cx| inner.poll_flush(cx))
    }

    fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        // Shutdown is a best-effort terminal transition; do not arm the stall
        // deadline for it.
        Pin::new(&mut self.inner).poll_shutdown(cx)
    }
}

/// The framing adapters plus the concurrency bound for one stdio session.
pub(crate) struct TransportBounds {
    pub(crate) max_header_bytes: usize,
    pub(crate) max_message_bytes: usize,
    pub(crate) write_stall_timeout: Duration,
    pub(crate) request_concurrency: usize,
}

impl Default for TransportBounds {
    fn default() -> Self {
        Self {
            max_header_bytes: MAX_HEADER_BYTES,
            max_message_bytes: MAX_MESSAGE_BYTES,
            write_stall_timeout: WRITE_STALL_TIMEOUT,
            request_concurrency: REQUEST_CONCURRENCY_LIMIT,
        }
    }
}

impl TransportBounds {
    /// Wraps a transport pair in the bounded adapters. Bytes pass through
    /// unmodified until a bound trips.
    pub(crate) fn wrap<R, W>(
        &self,
        reader: R,
        writer: W,
    ) -> (BoundedStdinReader<R>, BoundedStdoutWriter<W>) {
        let session = SessionTrip::default();
        (
            BoundedStdinReader::with_limits(
                reader,
                session.clone(),
                self.max_header_bytes,
                self.max_message_bytes,
            ),
            BoundedStdoutWriter::with_stall_timeout(writer, session, self.write_stall_timeout),
        )
    }
}

fn find_subslice(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}

/// Extracts the governing `Content-Length` from a raw header block, mirroring
/// the tower-lsp codec: ASCII-case-insensitive header name, last occurrence
/// wins. Returns `None` when absent or unparseable (the codec rejects those
/// deterministically on its own).
fn declared_content_length(header_block: &[u8]) -> Option<usize> {
    const NAME: &[u8] = b"content-length:";
    let mut declared = None;
    for line in header_block.split(|byte| *byte == b'\n') {
        let line = strip_trailing_carriage_return(line);
        if line.len() >= NAME.len() && line[..NAME.len()].eq_ignore_ascii_case(NAME) {
            let value = std::str::from_utf8(&line[NAME.len()..]).ok()?;
            declared = Some(value.trim().parse::<usize>().ok()?);
        }
    }
    declared
}

fn strip_trailing_carriage_return(line: &[u8]) -> &[u8] {
    line.strip_suffix(b"\r").unwrap_or(line)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    fn bounds() -> TransportBounds {
        TransportBounds {
            max_header_bytes: 64,
            max_message_bytes: 256,
            write_stall_timeout: Duration::from_millis(50),
            request_concurrency: REQUEST_CONCURRENCY_LIMIT,
        }
    }

    async fn read_all<R: AsyncRead + Unpin>(mut reader: R) -> io::Result<Vec<u8>> {
        let mut out = Vec::new();
        reader.read_to_end(&mut out).await?;
        Ok(out)
    }

    struct CountWake(std::sync::atomic::AtomicUsize);

    impl std::task::Wake for CountWake {
        fn wake(self: Arc<Self>) {
            self.0.fetch_add(1, Ordering::SeqCst);
        }
        fn wake_by_ref(self: &Arc<Self>) {
            self.0.fetch_add(1, Ordering::SeqCst);
        }
    }

    fn counting_waker() -> (Arc<CountWake>, Waker) {
        let counter = Arc::new(CountWake(std::sync::atomic::AtomicUsize::new(0)));
        let waker = std::task::Waker::from(Arc::clone(&counter));
        (counter, waker)
    }

    #[test]
    fn waker_registered_after_a_trip_is_woken_immediately() {
        // #2185 review: a reader that registers after trip() drained the
        // queue must not hang.
        let session = SessionTrip::default();
        session.trip();
        let (counter, waker) = counting_waker();
        session.register_waker(&waker);
        assert_eq!(
            counter.0.load(Ordering::SeqCst),
            1,
            "a waker registered after the trip must wake immediately"
        );
    }

    #[test]
    fn waker_registered_before_a_trip_is_woken_by_it() {
        let session = SessionTrip::default();
        let (counter, waker) = counting_waker();
        session.register_waker(&waker);
        assert_eq!(counter.0.load(Ordering::SeqCst), 0);
        session.trip();
        assert_eq!(counter.0.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn legitimate_frames_pass_through_unmodified() -> Result<(), String> {
        let payload = b"Content-Length: 4\r\n\r\n{\"a\":1}Content-Length: 2\r\n\r\n{}";
        let (reader, _writer) = bounds().wrap(&payload[..], tokio::io::sink());
        let out = read_all(reader)
            .await
            .map_err(|err| format!("legit stream must pass: {err}"))?;
        assert_eq!(out, payload, "bounded reader must not alter bytes");
        Ok(())
    }

    #[tokio::test]
    async fn frame_split_across_reads_passes() -> Result<(), String> {
        let payload = b"Content-Length: 11\r\n\r\n{\"a\":12345}";
        let (mut reader, _writer) = bounds().wrap(&payload[..], tokio::io::sink());
        let mut out = Vec::new();
        let mut chunk = [0_u8; 3];
        loop {
            let read = reader
                .read(&mut chunk)
                .await
                .map_err(|err| format!("split read failed: {err}"))?;
            if read == 0 {
                break;
            }
            out.extend_from_slice(&chunk[..read]);
        }
        assert_eq!(out, payload);
        Ok(())
    }

    #[tokio::test]
    async fn burst_of_small_frames_in_one_read_passes() -> Result<(), String> {
        // Many complete frames coalesced into a single read: aggregate bytes
        // far exceed the header cap, but each header block does not. The
        // accounting must consume every frame in place instead of letting
        // carried bytes accumulate against the cap.
        let mut payload = Vec::new();
        for _ in 0..200 {
            payload.extend_from_slice(b"Content-Length: 2\r\n\r\n{}");
        }
        let (reader, _writer) = bounds().wrap(&payload[..], tokio::io::sink());
        let out = read_all(reader)
            .await
            .map_err(|err| format!("legit burst must pass: {err}"))?;
        assert_eq!(out, payload);
        Ok(())
    }

    #[tokio::test]
    async fn body_bytes_that_look_like_headers_do_not_desync() -> Result<(), String> {
        // A body containing the header terminator bytes must be consumed as
        // body, never scanned as a header.
        let body = "{\"a\": \"x\r\n\r\nContent-Length: 999\"}";
        let payload = format!("Content-Length: {}\r\n\r\n{}", body.len(), body);
        let mut stream = payload.into_bytes();
        stream.extend_from_slice(b"Content-Length: 2\r\n\r\n{}");
        let (reader, _writer) = bounds().wrap(&stream[..], tokio::io::sink());
        let out = read_all(reader)
            .await
            .map_err(|err| format!("header-looking body must pass: {err}"))?;
        assert_eq!(out, stream);
        Ok(())
    }

    #[tokio::test]
    async fn oversized_declared_content_length_trips_before_body() -> Result<(), String> {
        let payload = b"Content-Length: 257\r\n\r\n";
        let (reader, _writer) = bounds().wrap(&payload[..], tokio::io::sink());
        match read_all(reader).await {
            Err(err) if err.kind() == io::ErrorKind::InvalidData => Ok(()),
            other => Err(format!("expected InvalidData trip, got: {other:?}")),
        }
    }

    #[tokio::test]
    async fn last_wins_duplicate_content_length_governs_the_cap() -> Result<(), String> {
        // The first value exceeds the cap, the governing (last) value does
        // not; the stream must pass because the codec consumes the last one.
        let payload = b"Content-Length: 999\r\nContent-Length: 2\r\n\r\n{}";
        let (reader, _writer) = bounds().wrap(&payload[..], tokio::io::sink());
        let out = read_all(reader)
            .await
            .map_err(|err| format!("last-wins header must pass: {err}"))?;
        assert_eq!(out, payload);
        Ok(())
    }

    #[tokio::test]
    async fn oversized_header_block_trips() -> Result<(), String> {
        let mut payload = b"Content-Length: 2\r\nX-Pad: ".to_vec();
        payload.extend_from_slice(&[b'x'; 128]);
        payload.extend_from_slice(b"\r\n\r\n{}");
        let (reader, _writer) = bounds().wrap(&payload[..], tokio::io::sink());
        match read_all(reader).await {
            Err(err) if err.kind() == io::ErrorKind::InvalidData => Ok(()),
            other => Err(format!("expected header-cap trip, got: {other:?}")),
        }
    }

    #[tokio::test]
    async fn missing_content_length_is_left_to_the_codec() -> Result<(), String> {
        // Not a bound violation: the tower-lsp codec rejects a missing
        // Content-Length itself; the reader must stay transparent.
        let payload = b"Content-Type: application/vscode-jsonrpc\r\n\r\n";
        let (reader, _writer) = bounds().wrap(&payload[..], tokio::io::sink());
        let out = read_all(reader)
            .await
            .map_err(|err| format!("header without length must pass through: {err}"))?;
        assert_eq!(out, payload);
        Ok(())
    }

    #[tokio::test]
    async fn writer_passes_through_and_reports_bytes() -> Result<(), String> {
        let (_reader, mut writer) = bounds().wrap(tokio::io::empty(), Vec::<u8>::new());
        writer
            .write_all(b"hello")
            .await
            .map_err(|err| format!("write must pass: {err}"))?;
        writer
            .flush()
            .await
            .map_err(|err| format!("flush: {err}"))?;
        assert_eq!(writer.inner, b"hello");
        Ok(())
    }

    #[tokio::test]
    async fn stalled_writer_trips_after_deadline_and_wakes_reader() -> Result<(), String> {
        let session = SessionTrip::default();
        let mut writer = BoundedStdoutWriter::with_stall_timeout(
            NeverReadyWriter,
            session.clone(),
            Duration::from_millis(20),
        );
        // Park a read on a reader whose inner stream never yields bytes (the
        // write half stays open but silent); only the session trip may wake it.
        let (_silent_tx, silent_rx) = tokio::io::duplex(8);
        let mut reader = BoundedStdinReader::with_limits(silent_rx, session.clone(), 64, 256);
        let parked = tokio::spawn(async move {
            let mut buf = [0_u8; 8];
            reader.read(&mut buf).await
        });
        tokio::task::yield_now().await;
        let write = writer.write_all(b"x");
        let outcome = tokio::time::timeout(Duration::from_secs(5), write).await;
        match outcome {
            Ok(Err(err)) if err.kind() == io::ErrorKind::TimedOut => {}
            other => return Err(format!("expected write-stall timeout, got: {other:?}")),
        }
        assert!(session.is_tripped(), "stall must trip the session flag");
        let read = tokio::time::timeout(Duration::from_secs(5), parked)
            .await
            .map_err(|err| format!("tripped session must wake the parked reader: {err}"))?
            .map_err(|err| format!("reader task failed: {err}"))?
            .map_err(|err| format!("tripped reader must report EOF, not error: {err}"))?;
        assert_eq!(read, 0, "tripped session must read as EOF");
        Ok(())
    }

    /// An `AsyncWrite` that never makes progress, simulating a client that
    /// stopped reading.
    struct NeverReadyWriter;

    impl AsyncWrite for NeverReadyWriter {
        fn poll_write(
            self: Pin<&mut Self>,
            _cx: &mut Context<'_>,
            _buf: &[u8],
        ) -> Poll<io::Result<usize>> {
            Poll::Pending
        }

        fn poll_flush(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<io::Result<()>> {
            Poll::Pending
        }

        fn poll_shutdown(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<io::Result<()>> {
            Poll::Ready(Ok(()))
        }
    }

    #[test]
    fn declared_content_length_parses_case_insensitively() -> Result<(), String> {
        assert_eq!(declared_content_length(b"Content-Length: 42"), Some(42));
        assert_eq!(declared_content_length(b"content-length:  7"), Some(7));
        assert_eq!(declared_content_length(b"Content-Length: nope"), None);
        assert_eq!(declared_content_length(b"X-Other: 1"), None);
        assert_eq!(
            declared_content_length(b"Content-Length: 184467440737095516159"),
            None,
            "overflowing values must defer to the codec's own rejection"
        );
        Ok(())
    }
    // ── In-process end-to-end composition tests (issue #2034) ──
    //
    // These drive `lsp::serve_streams` (the same composition `ripr lsp
    // --stdio` runs) over in-memory duplex streams, so the bounded adapters
    // plus tower-lsp are exercised together without a process spawn.

    fn frame(body: &str) -> Vec<u8> {
        format!("Content-Length: {}\r\n\r\n{}", body.len(), body).into_bytes()
    }

    fn initialize_frame() -> Vec<u8> {
        frame(
            r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"processId":null,"rootUri":null,"capabilities":{}}}"#,
        )
    }

    fn hover_frame(id: u64) -> Vec<u8> {
        frame(&format!(
            r#"{{"jsonrpc":"2.0","id":{id},"method":"textDocument/hover","params":{{"textDocument":{{"uri":"file:///nonexistent.rs"}},"position":{{"line":0,"character":0}}}}}}"#
        ))
    }

    fn e2e_bounds() -> TransportBounds {
        TransportBounds {
            max_header_bytes: MAX_HEADER_BYTES,
            max_message_bytes: 1024,
            write_stall_timeout: Duration::from_millis(200),
            request_concurrency: REQUEST_CONCURRENCY_LIMIT,
        }
    }

    /// Reads one framed response from the client side of the duplex.
    async fn read_frame<R: AsyncRead + Unpin>(reader: &mut R) -> Result<String, String> {
        let mut buffered = Vec::new();
        let deadline = std::time::Instant::now() + Duration::from_secs(10);
        loop {
            if buffered.windows(4).position(|w| w == b"\r\n\r\n").is_some()
                && String::from_utf8_lossy(&buffered).contains('}')
            {
                return String::from_utf8(buffered)
                    .map_err(|err| format!("response is not UTF-8: {err}"));
            }
            if std::time::Instant::now() >= deadline {
                return Err("timed out waiting for a framed response".to_string());
            }
            let mut chunk = [0_u8; 1024];
            let read = reader
                .read(&mut chunk)
                .await
                .map_err(|err| format!("reading response: {err}"))?;
            if read == 0 {
                return Err(format!(
                    "server closed the stream with {buffered:?} buffered"
                ));
            }
            buffered.extend_from_slice(&chunk[..read]);
        }
    }

    #[tokio::test]
    async fn oversized_frame_yields_one_bounded_parse_error_then_clean_stop() -> Result<(), String>
    {
        let (client_io, server_io) = tokio::io::duplex(64 * 1024);
        let (server_read, server_write) = tokio::io::split(server_io);
        let (mut client_read, mut client_write) = tokio::io::split(client_io);
        let root = std::env::temp_dir();
        let server = tokio::spawn(async move {
            super::super::serve_streams(server_read, server_write, root, &e2e_bounds()).await
        });
        // Declared length 1025 exceeds the 1024-byte test cap; no body ever
        // arrives. The bound must trip at the header, before any body wait.
        client_write
            .write_all(b"Content-Length: 1025\r\n\r\n")
            .await
            .map_err(|err| format!("writing oversized frame: {err}"))?;
        let response = read_frame(&mut client_read).await?;
        assert!(
            response.contains("-32700"),
            "expected one bounded -32700 parse error, got: {response}"
        );
        assert!(
            response.len() < 4096,
            "error response must be bounded, got {} bytes",
            response.len()
        );
        drop(client_write);
        drop(client_read);
        let outcome = tokio::time::timeout(Duration::from_secs(10), server)
            .await
            .map_err(|err| format!("server must stop after an ingress bound trip: {err}"))?
            .map_err(|err| format!("server task failed: {err}"))?;
        outcome.map_err(|err| format!("serve_streams returned an error: {err}"))
    }

    #[tokio::test]
    async fn client_that_stops_reading_causes_bounded_clean_stop() -> Result<(), String> {
        // Small duplex so unread egress wedges the writer quickly.
        let (client_io, server_io) = tokio::io::duplex(2048);
        let (server_read, server_write) = tokio::io::split(server_io);
        let (mut client_read, mut client_write) = tokio::io::split(client_io);
        let root = std::env::temp_dir();
        let server = tokio::spawn(async move {
            super::super::serve_streams(server_read, server_write, root, &e2e_bounds()).await
        });
        client_write
            .write_all(&initialize_frame())
            .await
            .map_err(|err| format!("writing initialize: {err}"))?;
        let initialize_response = read_frame(&mut client_read).await?;
        assert!(
            initialize_response.contains("\"capabilities\""),
            "expected an initialize result, got: {initialize_response}"
        );
        // Fire enough requests that the unread responses far exceed the
        // duplex capacity, then stop reading entirely: the write-stall
        // deadline must end the session instead of wedging it forever.
        for id in 2..130_u64 {
            client_write
                .write_all(&hover_frame(id))
                .await
                .map_err(|err| format!("writing hover {id}: {err}"))?;
        }
        let outcome = tokio::time::timeout(Duration::from_secs(10), server)
            .await
            .map_err(|err| {
                format!("server wedged: a stopped reader must trip the write-stall deadline: {err}")
            })?
            .map_err(|err| format!("server task failed: {err}"))?;
        drop(client_write);
        drop(client_read);
        outcome.map_err(|err| format!("serve_streams returned an error: {err}"))
    }
}
