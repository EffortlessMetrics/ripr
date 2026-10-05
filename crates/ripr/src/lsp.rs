mod action_contract;
mod actions;
pub(crate) mod agent_protocol;
mod analysis_thread;
mod backend;
mod capabilities;
mod client_features;
mod component_outcome;
mod config;
pub mod diagnostic_budget;
mod diagnostic_catalog;
mod diagnostics;
mod dollar_requests;
mod gap_artifacts;
mod git_inputs;
mod hover;
mod input_identity;
mod lens;
mod payload_bounds;
mod position;
mod progress;
mod progress_stages;
mod refresh_scheduler;
mod repair_card;
#[cfg(test)]
mod saved_edit_sequence;
#[cfg(test)]
mod source_origin_tests;
mod state;
#[cfg(test)]
mod tests;
mod transport_bounds;
mod uri;

use backend::Backend;
pub use diagnostics::{DiagnosticBatch, workspace_diagnostic_batches};
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::task::{Context, Poll};
use tower::Service;
use tower_lsp_server::jsonrpc::{Request, Response};
use tower_lsp_server::ls_types::{LSPAny, notification::Notification};
use tower_lsp_server::{ClientSocket, LspService, Server};

pub(super) struct AnalysisStatusNotification;

impl Notification for AnalysisStatusNotification {
    type Params = LSPAny;
    const METHOD: &'static str = "ripr/analysisStatus";
}

const COPY_CONTEXT_COMMAND: &str = "ripr.copyContext";
const COPY_AGENT_REPAIR_COMMAND: &str = "ripr.copyAgentRepairCommand";
const COPY_AGENT_PACKET_COMMAND: &str = "ripr.copyAgentPacketCommand";
const COPY_AGENT_BRIEF_COMMAND: &str = "ripr.copyAgentBriefCommand";
const COPY_AFTER_SNAPSHOT_COMMAND: &str = "ripr.copyAfterSnapshotCommand";
const COPY_AGENT_VERIFY_COMMAND: &str = "ripr.copyAgentVerifyCommand";
const COPY_AGENT_RECEIPT_COMMAND: &str = "ripr.copyAgentReceiptCommand";
const COPY_SUGGESTED_ASSERTION_COMMAND: &str = "ripr.copySuggestedAssertion";
const COPY_TARGETED_TEST_BRIEF_COMMAND: &str = "ripr.copyTargetedTestBrief";
const COLLECT_CONTEXT_COMMAND: &str = "ripr.collectContext";
const COLLECT_EVIDENCE_CONTEXT_COMMAND: &str = "ripr.collectEvidenceContext";
const COLLECT_WORKSPACE_STATUS_COMMAND: &str = "ripr.collectWorkspaceStatus";
const COLLECT_REPAIR_PACKET_COMMAND: &str = "ripr.collectRepairPacket";
const COLLECT_TOP_LIMITATION_COMMAND: &str = "ripr.collectTopLimitation";
const COLLECT_RECEIPT_STATUS_COMMAND: &str = "ripr.collectReceiptStatus";
const OPEN_RELATED_TEST_COMMAND: &str = "ripr.openRelatedTest";
const REFRESH_COMMAND: &str = "ripr.refresh";
const HOVER_TEXT: &str = "ripr estimates static RIPR exposure for changed Rust behavior. Run `ripr check --format json` for current findings.";

pub fn serve() -> Result<(), String> {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .map_err(|err| format!("failed to start LSP runtime: {err}"))?;
    runtime.block_on(serve_stdio())
}

async fn serve_stdio() -> Result<(), String> {
    let root =
        std::env::current_dir().map_err(|err| format!("failed to get current dir: {err}"))?;
    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();
    serve_streams(
        stdin,
        stdout,
        root,
        &transport_bounds::TransportBounds::default(),
    )
    .await
}

/// Serves one LSP session over the given transport pair with the reviewed
/// ingress/concurrency/egress bounds from `lsp/transport_bounds.rs` (issue
/// #2034). Separated from `serve_stdio` so the bounded composition is
/// exercised in-process by tests, not only by spawned binaries.
async fn serve_streams<I, O>(
    stdin: I,
    stdout: O,
    root: std::path::PathBuf,
    bounds: &transport_bounds::TransportBounds,
) -> Result<(), String>
where
    I: tokio::io::AsyncRead + Unpin,
    O: tokio::io::AsyncWrite + Unpin,
{
    let (stdin, stdout) = bounds.wrap(stdin, stdout);
    let (service, socket) = build_service(root.clone(), bounds.client_request_timeout);
    let order = ShutdownExitOrder::default();

    Server::new(stdin, stdout, socket)
        .concurrency_level(bounds.request_concurrency)
        .serve(dollar_requests::AnswerDollarRequests(
            RecordShutdownExit::new(service, order.clone()),
        ))
        .await;
    if order.exit_without_shutdown() {
        return Err(
            "lsp: received `exit` without a prior `shutdown` request; LSP section exit requires a nonzero exit"
                .to_string(),
        );
    }
    Ok(())
}

/// Shutdown/exit order observed on the wire (#5249).
///
/// LSP section exit says a server that receives `exit` without a prior
/// `shutdown` "should exit with an error code". tower-lsp-server stops
/// unconditionally, so the order is recorded here and `serve_streams`
/// reports the violation as a failure (exit 2 through the CLI failure
/// mapping). EOF and malformed-frame termination never set the exit flag,
/// so their exit-0 contract is unchanged.
#[derive(Clone, Default)]
struct ShutdownExitOrder {
    shutdown_received: Arc<AtomicBool>,
    exit_received: Arc<AtomicBool>,
}

impl ShutdownExitOrder {
    fn record(&self, method: &str) {
        match method {
            "shutdown" => self.shutdown_received.store(true, Ordering::SeqCst),
            "exit" => self.exit_received.store(true, Ordering::SeqCst),
            _ => {}
        }
    }

    fn exit_without_shutdown(&self) -> bool {
        self.exit_received.load(Ordering::SeqCst) && !self.shutdown_received.load(Ordering::SeqCst)
    }
}

/// Records `shutdown`/`exit` order so `serve_streams` can exit nonzero when
/// `exit` arrives without a prior `shutdown` (#5249). `exit` is a
/// notification, so its receipt alone counts; `shutdown` is a request, so
/// only a processed request authorizes a clean exit — an ID-less `shutdown`
/// notification never does (#6253 review). tower-lsp-server 0.23 exposes no
/// `id()` getter (only the consuming `into_parts`), so `shutdown` is
/// recorded on the response path: the inner service answers a real request
/// with `Some` and drops a notification to `None`. Observes only; answers
/// pass through untouched, and the existing `$/`-request layer keeps its
/// position outside this one.
struct RecordShutdownExit<S> {
    inner: S,
    order: ShutdownExitOrder,
}

impl<S> RecordShutdownExit<S> {
    fn new(inner: S, order: ShutdownExitOrder) -> Self {
        Self { inner, order }
    }
}

impl<S> Service<Request> for RecordShutdownExit<S>
where
    S: Service<Request, Response = Option<Response>>,
    S::Future: Send + 'static,
{
    type Response = Option<Response>;
    type Error = S::Error;
    type Future = Pin<Box<dyn Future<Output = Result<Option<Response>, S::Error>> + Send>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, request: Request) -> Self::Future {
        let method = request.method().to_owned();
        // `exit` arrives as a notification, so its receipt alone counts:
        // even if the inner service errors or the call is cancelled, the
        // message arrived. `shutdown` records on the response path below.
        if method == "exit" {
            self.order.record("exit");
        }
        let future = self.inner.call(request);
        let order = self.order.clone();
        Box::pin(async move {
            let response = future.await?;
            if method == "shutdown" && response.is_some() {
                order.record("shutdown");
            }
            Ok(response)
        })
    }
}

/// Builds the LSP service with the standard trace lifecycle registered
/// (`$/setTrace`, #2035, RIPR-SPEC-0137). tower-lsp-server has no native
/// `$/setTrace` handler — unregistered notifications are silently dropped —
/// so the notification is registered as a custom method whose handler takes
/// untyped params and validates them manually (a typed-params parse failure
/// would be dropped silently). The framed duplex tests use this same
/// constructor so the trace contract is exercised through the wire harness.
/// `client_request_timeout` is the backend's server→client request liveness
/// bound (#5278); the reviewed default lives in `transport_bounds`.
fn build_service(
    root: std::path::PathBuf,
    client_request_timeout: std::time::Duration,
) -> (LspService<Backend>, ClientSocket) {
    LspService::build(|client| {
        Backend::new(client, root).with_client_request_timeout(client_request_timeout)
    })
    .custom_method("$/setTrace", Backend::set_trace)
    .custom_method(
        "ripr/listActionableItems",
        Backend::ripr_list_actionable_items,
    )
    .finish()
}
