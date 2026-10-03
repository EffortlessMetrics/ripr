use super::{
    framing::{FrameRead, FrameReader},
    server::McpServer,
    writer::FrameWriter,
};
use crate::workspace_status::WorkspaceStatus;
use rmcp::{
    RoleServer, ServiceExt,
    model::{ClientJsonRpcMessage, ErrorData, RequestId, ServerJsonRpcMessage},
    transport::{
        Transport,
        async_rw::{JsonRpcMessageCodec, JsonRpcMessageCodecError},
    },
};
use std::{
    io::Error,
    path::PathBuf,
    sync::{Arc, Mutex as StdMutex},
};
use tokio::{
    io::{AsyncRead, AsyncWrite},
    sync::{Mutex, Notify},
};
use tokio_util::{bytes::BytesMut, codec::Decoder};

#[derive(Default)]
struct TransportFailure {
    reason: StdMutex<Option<&'static str>>,
    wake: Notify,
}
type Failure = Arc<TransportFailure>;
/// One typed request remains admitted until its actual reply frame is flushed.
#[derive(Default)]
struct Admission {
    pending: StdMutex<Option<RequestId>>,
    wake: Notify,
}
impl Admission {
    fn is_pending(&self) -> Result<bool, Error> {
        self.pending
            .lock()
            .map(|id| id.is_some())
            .map_err(|_error| Error::other("MCP admission unavailable"))
    }
    fn admit(&self, id: RequestId) -> Result<(), Error> {
        let mut pending = self
            .pending
            .lock()
            .map_err(|_error| Error::other("MCP admission unavailable"))?;
        if pending.is_some() {
            return Err(Error::other("MCP admission already pending"));
        }
        *pending = Some(id);
        Ok(())
    }
    fn complete(&self, id: Option<RequestId>) -> Result<(), Error> {
        let mut pending = self
            .pending
            .lock()
            .map_err(|_error| Error::other("MCP admission unavailable"))?;
        if id.is_some() && *pending == id {
            *pending = None;
            self.wake.notify_one();
        }
        Ok(())
    }
}
fn record_failure(failure: &Failure, reason: &'static str) {
    if let Ok(mut stored) = failure.reason.lock()
        && stored.is_none()
    {
        *stored = Some(reason);
    }
    // A response send task can fail while the SDK is awaiting receive.
    // Notify retains a permit even if that future is between polls.
    failure.wake.notify_one();
}
struct BoundedTransport<R, W> {
    reader: FrameReader<R>,
    writer: Arc<Mutex<FrameWriter<W>>>,
    failure: Failure,
    admission: Arc<Admission>,
    pending_protocol_error: Option<ServerJsonRpcMessage>,
    writer_needs_drain: bool,
}
impl<R: AsyncRead + Unpin + Send, W: AsyncWrite + Unpin + Send + 'static> Transport<RoleServer>
    for BoundedTransport<R, W>
{
    type Error = Error;
    fn send(
        &mut self,
        item: ServerJsonRpcMessage,
    ) -> impl Future<Output = Result<(), Error>> + Send + 'static {
        let writer = self.writer.clone();
        let failure = self.failure.clone();
        let admission = self.admission.clone();
        async move {
            let result = async {
                let mut writer = writer.lock().await;
                admission.complete(writer.finish_pending().await?)?;
                writer.queue(&item)?;
                // Retained notification and frame identity survive a dropped send.
                admission.wake.notify_one();
                admission.complete(writer.finish_pending().await?)
            }
            .await;
            if let Err(error) = &result {
                record_failure(&failure, super::writer::failure_reason(error));
            }
            result
        }
    }
    async fn receive(&mut self) -> Option<ClientJsonRpcMessage> {
        loop {
            if self
                .failure
                .reason
                .lock()
                .map_or(true, |reason| reason.is_some())
            {
                return None;
            }
            match self.admission.is_pending() {
                Err(_) => {
                    record_failure(&self.failure, "MCP admission unavailable");
                    return None;
                }
                Ok(true) => {
                    // Never hold the admission mutex while acquiring the writer.
                    let mut writer = tokio::select! {
                        _ = self.failure.wake.notified() => return None,
                        writer = self.writer.lock() => writer,
                    };
                    if writer.has_pending() {
                        let flushed = tokio::select! {
                            _ = self.failure.wake.notified() => return None,
                            flushed = writer.finish_pending() => flushed,
                        };
                        match flushed.and_then(|id| self.admission.complete(id)) {
                            Ok(()) => continue,
                            Err(error) => {
                                record_failure(
                                    &self.failure,
                                    super::writer::failure_reason(&error),
                                );
                                return None;
                            }
                        }
                    }
                    drop(writer);
                    if self.admission.is_pending().ok() != Some(true) {
                        continue;
                    }
                    tokio::select! {
                        _ = self.failure.wake.notified() => return None,
                        _ = self.admission.wake.notified() => continue,
                    }
                }
                Ok(false) => {}
            }
            if self.pending_protocol_error.is_some() || self.writer_needs_drain {
                let mut writer = self.writer.lock().await;
                if let Err(error) = writer
                    .finish_pending()
                    .await
                    .and_then(|id| self.admission.complete(id))
                {
                    record_failure(&self.failure, super::writer::failure_reason(&error));
                    return None;
                }
                self.writer_needs_drain = false;
                if let Some(error) = self.pending_protocol_error.as_ref() {
                    if let Err(error) = writer.queue(error) {
                        record_failure(&self.failure, super::writer::failure_reason(&error));
                        return None;
                    }
                    self.pending_protocol_error = None;
                    self.writer_needs_drain = true;
                    if let Err(error) = writer
                        .finish_pending()
                        .await
                        .and_then(|id| self.admission.complete(id))
                    {
                        record_failure(&self.failure, super::writer::failure_reason(&error));
                        return None;
                    }
                    self.writer_needs_drain = false;
                }
            }
            let read = tokio::select! {
                _ = self.failure.wake.notified() => return None,
                read = self.reader.read_frame() => read,
            };
            let frame = match read {
                Ok(FrameRead::Eof) => return None,
                Ok(FrameRead::Empty) => continue,
                Ok(FrameRead::Oversized) => {
                    let error = ServerJsonRpcMessage::error(
                        ErrorData::invalid_request(
                            "MCP message exceeds the configured byte limit",
                            None,
                        ),
                        None,
                    );
                    self.pending_protocol_error = Some(error);
                    continue;
                }
                Ok(FrameRead::Frame(frame)) => frame,
                Err(_) => {
                    record_failure(&self.failure, "MCP input IO failed");
                    return None;
                }
            };
            // The SDK codec owns typed parsing and compatibility. Framing
            // already enforced the product input bound, including at EOF.
            let mut bytes = BytesMut::from(frame.as_slice());
            match JsonRpcMessageCodec::<ClientJsonRpcMessage>::default().decode_eof(&mut bytes) {
                Ok(Some(message)) => {
                    if let ClientJsonRpcMessage::Request(request) = &message
                        && self.admission.admit(request.id.clone()).is_err()
                    {
                        record_failure(&self.failure, "MCP admission unavailable");
                        return None;
                    }
                    return Some(message);
                }
                Ok(None) => continue,
                Err(JsonRpcMessageCodecError::Serde(error))
                    if matches!(
                        error.classify(),
                        serde_json::error::Category::Syntax | serde_json::error::Category::Eof
                    ) =>
                {
                    continue;
                }
                Err(_) => {
                    let error = ServerJsonRpcMessage::error(
                        ErrorData::invalid_request("Invalid request", None),
                        None,
                    );
                    self.pending_protocol_error = Some(error);
                }
            }
        }
    }
    async fn close(&mut self) -> Result<(), Error> {
        let result = self.writer.lock().await.close().await;
        if let Err(error) = &result {
            record_failure(&self.failure, super::writer::failure_reason(error));
        }
        result
    }
}
pub(super) async fn serve_stdio(explicit_root: Option<PathBuf>) -> Result<(), String> {
    let (status, analysis_root) = WorkspaceStatus::resolve_with_root(explicit_root);
    serve(
        tokio::io::stdin(),
        tokio::io::stdout(),
        status,
        analysis_root,
    )
    .await
}
async fn serve<R, W>(
    reader: R,
    writer: W,
    status: WorkspaceStatus,
    analysis_root: Option<PathBuf>,
) -> Result<(), String>
where
    R: AsyncRead + Unpin + Send + 'static,
    W: AsyncWrite + Unpin + Send + 'static,
{
    let failure: Failure = Arc::new(TransportFailure::default());
    let transport = BoundedTransport {
        reader: FrameReader::new(reader),
        writer: Arc::new(Mutex::new(FrameWriter::new(writer))),
        failure: failure.clone(),
        admission: Arc::new(Admission::default()),
        pending_protocol_error: None,
        writer_needs_drain: false,
    };
    let server = McpServer::new(status, analysis_root)
        .map_err(|_error| "MCP status projection failed".to_owned())?;
    let service = match server.serve(transport).await {
        Ok(service) => service,
        Err(error) => {
            let reason = failure
                .reason
                .lock()
                .map_err(|_error| "MCP transport status unavailable".to_string())?
                .take();
            return match reason {
                Some(reason) => Err(reason.to_string()),
                None if matches!(
                    error,
                    rmcp::service::ServerInitializeError::ConnectionClosed(_)
                ) =>
                {
                    Ok(())
                }
                None => Err("MCP SDK startup failed".to_string()),
            };
        }
    };
    service
        .waiting()
        .await
        .map_err(|_error| "MCP SDK service failed".to_string())?;
    let error = failure
        .reason
        .lock()
        .map_err(|_error| "MCP transport status unavailable".to_string())?
        .take();
    match error {
        Some(error) => Err(error.to_string()),
        None => Ok(()),
    }
}

#[cfg(test)]
#[path = "transport_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "transport_backpressure_tests.rs"]
mod backpressure_tests;
