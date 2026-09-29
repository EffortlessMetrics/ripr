use super::*;
use serde_json::{Value, json};
use std::{
    pin::Pin,
    sync::atomic::{AtomicBool, AtomicUsize, Ordering},
    task::{Context, Poll, Waker},
    time::Duration,
};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

#[derive(Default)]
struct OutputGate {
    held: AtomicBool,
    blocked: Notify,
    waker: StdMutex<Option<Waker>>,
}
impl OutputGate {
    fn release(&self) -> Result<(), String> {
        self.held.store(false, Ordering::SeqCst);
        if let Some(waker) = self
            .waker
            .lock()
            .map_err(|_error| "output gate poisoned")?
            .take()
        {
            waker.wake();
        }
        Ok(())
    }
}
struct GatedWriter<W> {
    inner: W,
    gate: Arc<OutputGate>,
}
impl<W: AsyncWrite + Unpin> AsyncWrite for GatedWriter<W> {
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        bytes: &[u8],
    ) -> Poll<std::io::Result<usize>> {
        let this = self.get_mut();
        if this.gate.held.load(Ordering::SeqCst) {
            let Ok(mut waker) = this.gate.waker.lock() else {
                return Poll::Ready(Err(Error::other("test output gate poisoned")));
            };
            *waker = Some(cx.waker().clone());
            if this.gate.held.load(Ordering::SeqCst) {
                this.gate.blocked.notify_one();
                return Poll::Pending;
            }
        }
        Pin::new(&mut this.inner).poll_write(cx, bytes)
    }
    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        Pin::new(&mut self.get_mut().inner).poll_flush(cx)
    }
    fn poll_shutdown(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        Pin::new(&mut self.get_mut().inner).poll_shutdown(cx)
    }
}

/// Count actual transport admissions to the real SDK, not prefetched bytes.
struct ObservedTransport<R, W> {
    inner: BoundedTransport<R, W>,
    requests: Arc<AtomicUsize>,
    admitted: Arc<Notify>,
}
impl<R: AsyncRead + Unpin + Send, W: AsyncWrite + Unpin + Send + 'static> Transport<RoleServer>
    for ObservedTransport<R, W>
{
    type Error = Error;
    fn send(
        &mut self,
        item: ServerJsonRpcMessage,
    ) -> impl Future<Output = Result<(), Error>> + Send + 'static {
        self.inner.send(item)
    }
    async fn receive(&mut self) -> Option<ClientJsonRpcMessage> {
        let message = self.inner.receive().await;
        if matches!(message, Some(ClientJsonRpcMessage::Request(_))) {
            self.requests.fetch_add(1, Ordering::SeqCst);
            self.admitted.notify_one();
        }
        message
    }
    async fn close(&mut self) -> Result<(), Error> {
        self.inner.close().await
    }
}
fn current_meta() -> Value {
    json!({
        "io.modelcontextprotocol/protocolVersion": "2026-07-28",
        "io.modelcontextprotocol/clientCapabilities": {},
        "io.modelcontextprotocol/clientInfo": {"name":"backpressure-test","version":"1"}
    })
}
fn frame(value: Value) -> Result<Vec<u8>, String> {
    let mut bytes = serde_json::to_vec(&value).map_err(|error| error.to_string())?;
    bytes.push(b'\n');
    Ok(bytes)
}
async fn next_reply<R: AsyncRead + Unpin>(reader: &mut BufReader<R>) -> Result<Value, String> {
    let mut bytes = Vec::new();
    let count = tokio::time::timeout(Duration::from_secs(5), reader.read_until(b'\n', &mut bytes))
        .await
        .map_err(|_error| "actual SDK reply deadline")?
        .map_err(|error| error.to_string())?;
    if count == 0 {
        return Err("actual SDK reply stream ended early".into());
    }
    serde_json::from_slice(&bytes).map_err(|error| error.to_string())
}

#[tokio::test]
async fn actual_sdk_held_stdout_preserves_serial_request_admission() -> Result<(), String> {
    const BURST: u64 = 32;
    let (mut input, source) = tokio::io::duplex(64 * 1024);
    let (sink, output) = tokio::io::duplex(64 * 1024);
    let gate = Arc::new(OutputGate::default());
    let requests = Arc::new(AtomicUsize::new(0));
    let admitted = Arc::new(Notify::new());
    let transport = ObservedTransport {
        inner: BoundedTransport {
            reader: FrameReader::new(source),
            writer: Arc::new(Mutex::new(FrameWriter::new(GatedWriter {
                inner: sink,
                gate: gate.clone(),
            }))),
            failure: Arc::new(TransportFailure::default()),
            admission: Arc::new(Admission::default()),
            pending_protocol_error: None,
            writer_needs_drain: false,
        },
        requests: requests.clone(),
        admitted: admitted.clone(),
    };
    let server =
        McpServer::new(WorkspaceStatus::resolve(None)).map_err(|error| error.to_string())?;
    let service_task = tokio::spawn(async move {
        let service = server
            .serve(transport)
            .await
            .map_err(|error| error.to_string())?;
        service.waiting().await.map_err(|error| error.to_string())?;
        Ok::<_, String>(())
    });
    input.write_all(&frame(json!({"jsonrpc":"2.0","id":"discover","method":"server/discover","params":{"_meta":current_meta()}}))?)
        .await.map_err(|error| error.to_string())?;
    let mut output = BufReader::new(output);
    let discovery = next_reply(&mut output).await?;
    if discovery.get("id").and_then(Value::as_str) != Some("discover")
        || discovery.get("result").is_none()
        || requests.load(Ordering::SeqCst) != 1
    {
        service_task.abort();
        return Err("actual SDK discovery did not establish one admitted request".into());
    }
    gate.held.store(true, Ordering::SeqCst);
    let mut burst = Vec::new();
    for id in 0..BURST {
        burst.extend(frame(json!({"jsonrpc":"2.0","id":id,"method":"tools/call","params":{"_meta":current_meta(),"name":"ripr_workspace_status","arguments":{}}}))?);
    }
    input
        .write_all(&burst)
        .await
        .map_err(|error| error.to_string())?;
    tokio::time::timeout(Duration::from_secs(5), gate.blocked.notified())
        .await
        .map_err(|_error| "actual response writer never reached Pending")?;
    // Give queued, nonempty input a bounded opportunity to expose admission
    // beyond the one response whose stdout write has actually suspended.
    let exceeded = tokio::time::timeout(Duration::from_millis(200), async {
        loop {
            if requests.load(Ordering::SeqCst) > 2 {
                break;
            }
            admitted.notified().await;
        }
    })
    .await
    .is_ok();
    let held_admissions = requests.load(Ordering::SeqCst);
    gate.release()?;
    let mut ids = std::collections::BTreeSet::new();
    for _ in 0..BURST {
        let reply = next_reply(&mut output).await?;
        let id = reply
            .get("id")
            .and_then(Value::as_u64)
            .ok_or_else(|| "actual status response omitted its numeric ID".to_string())?;
        if reply.get("result").is_none() || !ids.insert(id) {
            service_task.abort();
            return Err("actual SDK lost or duplicated a status response".into());
        }
    }
    drop(input);
    tokio::time::timeout(Duration::from_secs(5), service_task)
        .await
        .map_err(|_error| "SDK service did not terminate after actual responses and EOF")?
        .map_err(|error| error.to_string())??;
    if ids != (0..BURST).collect() {
        return Err("actual SDK status response IDs differ from the finite input burst".into());
    }
    if exceeded || held_admissions != 2 {
        return Err(format!(
            "held stdout admitted {held_admissions} requests; expected discovery plus one pending response"
        ));
    }
    Ok(())
}

const TWO_REQUESTS: &[u8] = b"{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"tools/list\",\"params\":{}}\n{\"jsonrpc\":\"2.0\",\"id\":2,\"method\":\"tools/list\",\"params\":{}}\n";

fn direct_transport<W: AsyncWrite + Unpin>(writer: W) -> BoundedTransport<&'static [u8], W> {
    BoundedTransport {
        reader: FrameReader::new(TWO_REQUESTS),
        writer: Arc::new(Mutex::new(FrameWriter::new(writer))),
        failure: Arc::new(TransportFailure::default()),
        admission: Arc::new(Admission::default()),
        pending_protocol_error: None,
        writer_needs_drain: false,
    }
}
async fn require_request<R, W>(
    transport: &mut BoundedTransport<R, W>,
    id: i64,
) -> Result<(), String>
where
    R: AsyncRead + Unpin + Send,
    W: AsyncWrite + Unpin + Send + 'static,
{
    match transport.receive().await {
        Some(ClientJsonRpcMessage::Request(request))
            if request.id == rmcp::model::RequestId::Number(id) =>
        {
            Ok(())
        }
        _ => Err(format!("actual transport did not admit request {id}")),
    }
}
async fn require_pending<R, W>(transport: &mut BoundedTransport<R, W>) -> Result<(), String>
where
    R: AsyncRead + Unpin + Send,
    W: AsyncWrite + Unpin + Send + 'static,
{
    use std::{future::Future, future::poll_fn};
    let mut receiving = Box::pin(transport.receive());
    poll_fn(|cx| match receiving.as_mut().poll(cx) {
        Poll::Pending => Poll::Ready(Ok(())),
        Poll::Ready(_) => Poll::Ready(Err("unflushed request admitted additional input".into())),
    })
    .await
}
fn reply(id: i64) -> ServerJsonRpcMessage {
    ServerJsonRpcMessage::error(
        ErrorData::internal_error("admission witness", None),
        Some(rmcp::model::RequestId::Number(id)),
    )
}

#[tokio::test]
async fn cancelled_receive_and_wrong_reply_id_cannot_release_admission() -> Result<(), String> {
    let mut transport = direct_transport(Vec::<u8>::new());
    require_request(&mut transport, 1).await?;
    require_pending(&mut transport).await?;
    let notification: ServerJsonRpcMessage = serde_json::from_value(serde_json::json!({
        "jsonrpc": "2.0",
        "method": "notifications/tools/list_changed"
    }))
    .map_err(|error| error.to_string())?;
    if !matches!(notification, ServerJsonRpcMessage::Notification(_)) {
        return Err("notification admission control did not construct a typed notification".into());
    }
    transport
        .send(notification)
        .await
        .map_err(|error| error.to_string())?;
    require_pending(&mut transport).await?;
    transport
        .send(reply(99))
        .await
        .map_err(|error| error.to_string())?;
    require_pending(&mut transport).await?;
    transport
        .send(reply(1))
        .await
        .map_err(|error| error.to_string())?;
    require_request(&mut transport, 2).await
}

#[tokio::test]
async fn unknown_id_protocol_error_cannot_release_an_admitted_request() -> Result<(), String> {
    let mut transport = direct_transport(Vec::<u8>::new());
    require_request(&mut transport, 1).await?;
    transport
        .send(ServerJsonRpcMessage::error(
            ErrorData::invalid_request("unknown-ID admission witness", None),
            None,
        ))
        .await
        .map_err(|error| error.to_string())?;
    require_pending(&mut transport).await?;
    transport
        .send(reply(1))
        .await
        .map_err(|error| error.to_string())?;
    require_request(&mut transport, 2).await
}

#[tokio::test]
async fn cancelled_partial_response_retains_admission_until_actual_flush() -> Result<(), String> {
    use std::{future::Future, future::poll_fn};
    let (sink, mut output) = tokio::io::duplex(2);
    let mut transport = direct_transport(sink);
    require_request(&mut transport, 1).await?;
    {
        let mut sending = Box::pin(transport.send(reply(1)));
        poll_fn(|cx| match sending.as_mut().poll(cx) {
            Poll::Pending => Poll::Ready(Ok(())),
            Poll::Ready(_) => {
                Poll::Ready(Err("partial response never reached Pending".to_string()))
            }
        })
        .await?;
    }
    require_pending(&mut transport).await?;
    let collector = tokio::spawn(async move {
        use tokio::io::AsyncReadExt;
        let mut bytes = Vec::new();
        output
            .read_to_end(&mut bytes)
            .await
            .map_err(|error| error.to_string())?;
        Ok::<_, String>(bytes)
    });
    tokio::time::timeout(Duration::from_secs(5), require_request(&mut transport, 2))
        .await
        .map_err(|_error| "cancelled partial response did not resume before admission")??;
    transport.close().await.map_err(|error| error.to_string())?;
    let bytes = tokio::time::timeout(Duration::from_secs(5), collector)
        .await
        .map_err(|_error| "partial reply collector exceeded its deadline")?
        .map_err(|error| error.to_string())??;
    let response: Value = serde_json::from_slice(&bytes).map_err(|error| error.to_string())?;
    if response.get("id").and_then(Value::as_i64) != Some(1)
        || bytes.iter().filter(|byte| **byte == b'\n').count() != 1
    {
        return Err("resumed actual reply was duplicated or lost its original ID".into());
    }
    Ok(())
}

#[tokio::test]
async fn cancelled_flush_retains_admission_after_all_reply_bytes_are_written() -> Result<(), String>
{
    use std::{future::Future, future::poll_fn};
    struct FlushHeldWriter {
        bytes: Vec<u8>,
        gate: Arc<OutputGate>,
    }
    impl AsyncWrite for FlushHeldWriter {
        fn poll_write(
            mut self: Pin<&mut Self>,
            _cx: &mut Context<'_>,
            bytes: &[u8],
        ) -> Poll<std::io::Result<usize>> {
            self.bytes.extend_from_slice(bytes);
            Poll::Ready(Ok(bytes.len()))
        }
        fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
            if !self.bytes.is_empty() && self.gate.held.load(Ordering::SeqCst) {
                let Ok(mut waker) = self.gate.waker.lock() else {
                    return Poll::Ready(Err(Error::other("test flush gate poisoned")));
                };
                *waker = Some(cx.waker().clone());
                return Poll::Pending;
            }
            Poll::Ready(Ok(()))
        }
        fn poll_shutdown(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
            self.poll_flush(cx)
        }
    }
    let gate = Arc::new(OutputGate::default());
    gate.held.store(true, Ordering::SeqCst);
    let mut transport = direct_transport(FlushHeldWriter {
        bytes: Vec::new(),
        gate: gate.clone(),
    });
    require_request(&mut transport, 1).await?;
    {
        let mut sending = Box::pin(transport.send(reply(1)));
        poll_fn(|cx| match sending.as_mut().poll(cx) {
            Poll::Pending => Poll::Ready(Ok(())),
            Poll::Ready(_) => Poll::Ready(Err("reply flush never reached Pending".to_string())),
        })
        .await?;
    }
    let expected =
        super::super::writer::encode_message(&reply(1)).map_err(|error| error.to_string())?;
    {
        let writer = transport.writer.lock().await;
        if writer.output().bytes != expected {
            return Err(
                "flush control did not write the entire actual reply before cancellation".into(),
            );
        }
    }
    require_pending(&mut transport).await?;
    gate.release()?;
    tokio::time::timeout(Duration::from_secs(5), require_request(&mut transport, 2))
        .await
        .map_err(|_error| "cancelled flush did not resume before admission")??;
    let writer = transport.writer.lock().await;
    if writer.output().bytes != expected {
        return Err("cancelled flush duplicated or changed the completed reply frame".into());
    }
    Ok(())
}

#[tokio::test]
async fn fatal_send_wakes_receive_with_an_admitted_request() -> Result<(), String> {
    let mut transport = direct_transport(Vec::<u8>::new());
    require_request(&mut transport, 1).await?;
    let giant = ServerJsonRpcMessage::error(
        ErrorData::internal_error("fatal admission witness", None),
        Some(rmcp::model::RequestId::String(
            "\n".repeat(70 * 1024).into(),
        )),
    );
    let reason = transport
        .send(giant)
        .await
        .err()
        .ok_or_else(|| "giant correlated response unexpectedly fit".to_string())?;
    if reason.to_string() != "MCP output limit" {
        return Err("fatal admitted response lost its bounded reason".into());
    }
    if tokio::time::timeout(Duration::from_secs(5), transport.receive())
        .await
        .map_err(|_error| "fatal output did not release blocked receive for termination")?
        .is_some()
    {
        return Err("fatal output admitted another request".into());
    }
    Ok(())
}
