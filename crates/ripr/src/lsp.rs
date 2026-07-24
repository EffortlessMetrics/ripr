mod action_contract;
mod actions;
mod agent_protocol;
mod backend;
mod capabilities;
mod client_features;
mod component_outcome;
mod config;
pub mod diagnostic_budget;
mod diagnostic_catalog;
mod diagnostics;
mod gap_artifacts;
mod git_inputs;
mod hover;
mod input_identity;
mod lens;
mod payload_bounds;
mod position;
mod progress;
mod refresh_scheduler;
mod state;
#[cfg(test)]
mod tests;
mod transport_bounds;
mod uri;

use backend::Backend;
pub use diagnostics::{DiagnosticBatch, workspace_diagnostic_batches};
use tower_lsp_server::ls_types::{LSPAny, notification::Notification};
use tower_lsp_server::{ClientSocket, LspService, Server};

pub(super) struct AnalysisStatusNotification;

impl Notification for AnalysisStatusNotification {
    type Params = LSPAny;
    const METHOD: &'static str = "ripr/analysisStatus";
}

const COPY_CONTEXT_COMMAND: &str = "ripr.copyContext";
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
    let (service, socket) = build_service(root.clone());

    Server::new(stdin, stdout, socket)
        .concurrency_level(bounds.request_concurrency)
        .serve(service)
        .await;
    Ok(())
}

/// Builds the LSP service with the standard trace lifecycle registered
/// (`$/setTrace`, #2035, RIPR-SPEC-0137). tower-lsp-server has no native
/// `$/setTrace` handler — unregistered notifications are silently dropped —
/// so the notification is registered as a custom method whose handler takes
/// untyped params and validates them manually (a typed-params parse failure
/// would be dropped silently). The framed duplex tests use this same
/// constructor so the trace contract is exercised through the wire harness.
fn build_service(root: std::path::PathBuf) -> (LspService<Backend>, ClientSocket) {
    LspService::build(|client| Backend::new(client, root))
        .custom_method("$/setTrace", Backend::set_trace)
        .finish()
}
