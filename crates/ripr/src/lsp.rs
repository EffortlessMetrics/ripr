mod actions;
mod backend;
mod capabilities;
mod diagnostics;
mod hover;
mod state;
#[cfg(test)]
mod tests;
mod uri;

use backend::Backend;
pub use diagnostics::{DiagnosticBatch, workspace_diagnostic_batches};
use tower_lsp_server::{LspService, Server};

const COPY_CONTEXT_COMMAND: &str = "ripr.copyContext";
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
