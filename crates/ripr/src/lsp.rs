mod actions;
mod backend;
mod capabilities;
mod config;
mod diagnostics;
mod gap_artifacts;
mod hover;
mod state;
#[cfg(test)]
mod tests;
mod uri;

use backend::Backend;
pub use diagnostics::{DiagnosticBatch, workspace_diagnostic_batches};
use tower_lsp_server::{LspService, Server};

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
    let (service, socket) = LspService::new(|client| Backend::new(client, root.clone()));

    Server::new(stdin, stdout, socket).serve(service).await;
    Ok(())
}
