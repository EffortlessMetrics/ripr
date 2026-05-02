use crate::app::{CheckInput, OutputFormat, check_workspace};
use crate::domain::Finding;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use tower_lsp_server::jsonrpc::Result as LspResult;
use tower_lsp_server::ls_types::{
    CodeAction, CodeActionKind, CodeActionOrCommand, CodeActionProviderCapability,
    CodeActionResponse, Command, Diagnostic, DiagnosticSeverity, DidChangeTextDocumentParams,
    DidOpenTextDocumentParams, DidSaveTextDocumentParams, ExecuteCommandOptions,
    ExecuteCommandParams, Hover, HoverContents, HoverParams, HoverProviderCapability,
    InitializeParams, InitializeResult, LSPAny, MarkupContent, MarkupKind, NumberOrString,
    Position, Range, ServerCapabilities, ServerInfo, TextDocumentSyncCapability,
    TextDocumentSyncKind, Uri,
};
use tower_lsp_server::{Client, LanguageServer, LspService, Server};

const COLLECT_CONTEXT_COMMAND: &str = "ripr.collectContext";
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

struct Backend {
    client: Client,
    root: PathBuf,
}

impl Backend {
    fn new(client: Client, root: PathBuf) -> Self {
        Self { client, root }
    }

    async fn refresh_diagnostics(&self) {
        let root = self.root.clone();
        let Ok(Ok(batches)) =
            tokio::task::spawn_blocking(move || workspace_diagnostic_batches(&root)).await
        else {
            return;
        };
        for batch in batches {
            self.client
                .publish_diagnostics(batch.uri, batch.diagnostics, None)
                .await;
        }
    }
}

impl LanguageServer for Backend {
    async fn initialize(&self, _: InitializeParams) -> LspResult<InitializeResult> {
        Ok(initialize_result())
    }

    async fn shutdown(&self) -> LspResult<()> {
        Ok(())
    }

    async fn did_open(&self, _: DidOpenTextDocumentParams) {
        self.refresh_diagnostics().await;
    }

    async fn did_change(&self, _: DidChangeTextDocumentParams) {
        self.refresh_diagnostics().await;
    }

    async fn did_save(&self, _: DidSaveTextDocumentParams) {
        self.refresh_diagnostics().await;
    }

    async fn hover(&self, _: HoverParams) -> LspResult<Option<Hover>> {
        Ok(Some(hover_response()))
    }

    async fn code_action(
        &self,
        _: tower_lsp_server::ls_types::CodeActionParams,
    ) -> LspResult<Option<CodeActionResponse>> {
        Ok(Some(code_action_response()))
    }

    async fn execute_command(&self, params: ExecuteCommandParams) -> LspResult<Option<LSPAny>> {
        if params.command == REFRESH_COMMAND {
            self.refresh_diagnostics().await;
        }
        Ok(None)
    }
}

pub struct DiagnosticBatch {
    pub uri: Uri,
    pub diagnostics: Vec<Diagnostic>,
}

pub fn workspace_diagnostic_batches(root: &Path) -> Result<Vec<DiagnosticBatch>, String> {
    let input = CheckInput {
        root: root.to_path_buf(),
        format: OutputFormat::Json,
        ..CheckInput::default()
    };
    let output = match check_workspace(input) {
        Ok(output) => output,
        Err(_) => return Ok(Vec::new()),
    };
    let mut grouped = BTreeMap::<Uri, Vec<Diagnostic>>::new();
    for finding in &output.findings {
        let path = absolute_finding_path(&output.root, finding);
        let uri = file_uri_for_path(&path)?;
        grouped
            .entry(uri)
            .or_default()
            .push(diagnostic_for_finding(finding));
    }
    Ok(grouped
        .into_iter()
        .map(|(uri, diagnostics)| DiagnosticBatch { uri, diagnostics })
        .collect())
}

fn initialize_result() -> InitializeResult {
    InitializeResult {
        capabilities: ServerCapabilities {
            text_document_sync: Some(TextDocumentSyncCapability::Kind(TextDocumentSyncKind::FULL)),
            hover_provider: Some(HoverProviderCapability::Simple(true)),
            code_action_provider: Some(CodeActionProviderCapability::Simple(true)),
            execute_command_provider: Some(ExecuteCommandOptions {
                commands: vec![
                    COLLECT_CONTEXT_COMMAND.to_string(),
                    REFRESH_COMMAND.to_string(),
                ],
                ..ExecuteCommandOptions::default()
            }),
            ..ServerCapabilities::default()
        },
        server_info: Some(ServerInfo {
            name: "ripr".to_string(),
            version: Some(env!("CARGO_PKG_VERSION").to_string()),
        }),
        offset_encoding: None,
    }
}

fn hover_response() -> Hover {
    Hover {
        contents: HoverContents::Markup(MarkupContent {
            kind: MarkupKind::Markdown,
            value: HOVER_TEXT.to_string(),
        }),
        range: None,
    }
}

fn code_action_response() -> CodeActionResponse {
    vec![
        CodeActionOrCommand::CodeAction(CodeAction {
            title: "Copy ripr context packet".to_string(),
            kind: Some(CodeActionKind::QUICKFIX),
            command: Some(Command {
                title: "Collect ripr context".to_string(),
                command: COLLECT_CONTEXT_COMMAND.to_string(),
                arguments: Some(Vec::new()),
            }),
            ..CodeAction::default()
        }),
        CodeActionOrCommand::CodeAction(CodeAction {
            title: "Run ripr check".to_string(),
            kind: Some(CodeActionKind::SOURCE),
            command: Some(Command {
                title: "Refresh ripr analysis".to_string(),
                command: REFRESH_COMMAND.to_string(),
                arguments: Some(Vec::new()),
            }),
            ..CodeAction::default()
        }),
    ]
}

fn diagnostic_for_finding(finding: &Finding) -> Diagnostic {
    let line = finding.probe.location.line.saturating_sub(1) as u32;
    Diagnostic {
        range: Range {
            start: Position { line, character: 0 },
            end: Position {
                line,
                character: 120,
            },
        },
        severity: Some(DiagnosticSeverity::WARNING),
        code: Some(NumberOrString::String(finding.class.as_str().to_string())),
        code_description: None,
        source: Some("ripr".to_string()),
        message: lsp_message(finding),
        related_information: None,
        tags: None,
        data: Some(serde_json::json!({
            "probeId": finding.id,
            "class": finding.class.as_str(),
            "family": finding.probe.family.as_str(),
            "confidence": finding.confidence,
        })),
    }
}

fn lsp_message(finding: &Finding) -> String {
    finding
        .recommended_next_step
        .clone()
        .unwrap_or_else(|| format!("{} static RIPR exposure", finding.class.as_str()))
}

fn absolute_finding_path(root: &Path, finding: &Finding) -> PathBuf {
    if finding.probe.location.file.is_absolute() {
        finding.probe.location.file.clone()
    } else {
        root.join(&finding.probe.location.file)
    }
}

fn file_uri_for_path(path: &Path) -> Result<Uri, String> {
    let normalized = path.to_string_lossy().replace('\\', "/");
    let encoded = encode_uri_path(&normalized);
    let uri = if encoded.starts_with('/') {
        format!("file://{encoded}")
    } else {
        format!("file:///{encoded}")
    };
    uri.parse()
        .map_err(|err| format!("failed to build LSP file URI for {}: {err}", path.display()))
}

fn encode_uri_path(path: &str) -> String {
    let mut encoded = String::new();
    for byte in path.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' | b'/' | b':' => {
                encoded.push(byte as char)
            }
            _ => encoded.push_str(&format!("%{byte:02X}")),
        }
    }
    encoded
}

#[cfg(test)]
mod tests {
    use super::{
        COLLECT_CONTEXT_COMMAND, HOVER_TEXT, REFRESH_COMMAND, code_action_response,
        diagnostic_for_finding, encode_uri_path, file_uri_for_path, hover_response,
        initialize_result,
    };
    use crate::domain::{
        Confidence, DeltaKind, ExposureClass, Finding, Probe, ProbeFamily, ProbeId, RevealEvidence,
        RiprEvidence, SourceLocation, StageEvidence, StageState,
    };
    use std::path::PathBuf;
    use tower_lsp_server::ls_types::{
        CodeActionOrCommand, DiagnosticSeverity, HoverContents, HoverProviderCapability,
        NumberOrString, TextDocumentSyncCapability, TextDocumentSyncKind,
    };

    #[test]
    fn initialize_result_exposes_existing_lsp_capabilities() -> Result<(), String> {
        let result = initialize_result();

        assert_eq!(
            result.capabilities.text_document_sync,
            Some(TextDocumentSyncCapability::Kind(TextDocumentSyncKind::FULL))
        );
        assert_eq!(
            result.capabilities.hover_provider,
            Some(HoverProviderCapability::Simple(true))
        );
        let Some(provider) = result.capabilities.execute_command_provider else {
            return Err("expected execute command provider".to_string());
        };
        let commands = provider.commands;
        assert_eq!(commands, vec![COLLECT_CONTEXT_COMMAND, REFRESH_COMMAND]);
        Ok(())
    }

    #[test]
    fn hover_response_keeps_current_guidance_text() -> Result<(), String> {
        let hover = hover_response();

        match hover.contents {
            HoverContents::Markup(markup) => {
                assert_eq!(markup.value, HOVER_TEXT);
                Ok(())
            }
            _ => Err("expected markup hover".to_string()),
        }
    }

    #[test]
    fn code_action_response_keeps_current_commands() -> Result<(), String> {
        let actions = code_action_response();

        let mut titles_kinds_and_commands = Vec::new();
        for action in &actions {
            match action {
                CodeActionOrCommand::CodeAction(action) => {
                    let Some(command) = &action.command else {
                        return Err("expected code action command".to_string());
                    };
                    let Some(kind) = &action.kind else {
                        return Err("expected code action kind".to_string());
                    };
                    titles_kinds_and_commands.push((
                        action.title.as_str(),
                        kind.as_str(),
                        command.title.as_str(),
                        command.command.as_str(),
                    ));
                }
                CodeActionOrCommand::Command(_) => {
                    return Err("expected code action".to_string());
                }
            }
        }

        assert_eq!(
            titles_kinds_and_commands,
            vec![
                (
                    "Copy ripr context packet",
                    "quickfix",
                    "Collect ripr context",
                    COLLECT_CONTEXT_COMMAND,
                ),
                (
                    "Run ripr check",
                    "source",
                    "Refresh ripr analysis",
                    REFRESH_COMMAND,
                ),
            ]
        );
        Ok(())
    }

    #[test]
    fn diagnostic_for_finding_preserves_lsp_payload_shape() -> Result<(), String> {
        let finding = sample_finding();
        let diagnostic = diagnostic_for_finding(&finding);

        assert_eq!(diagnostic.range.start.line, 87);
        assert_eq!(diagnostic.severity, Some(DiagnosticSeverity::WARNING));
        assert_eq!(
            diagnostic.code,
            Some(NumberOrString::String("weakly_exposed".to_string()))
        );
        assert_eq!(diagnostic.source.as_deref(), Some("ripr"));
        assert_eq!(diagnostic.message, "Add an exact boundary assertion.");
        let Some(data) = diagnostic.data else {
            return Err("expected diagnostic data".to_string());
        };
        assert_eq!(data["probeId"], "probe:pricing:88:predicate");
        assert_eq!(data["class"], "weakly_exposed");
        assert_eq!(data["family"], "predicate");
        assert_eq!(data["confidence"], 0.75);
        Ok(())
    }

    #[test]
    fn file_uri_for_path_uses_valid_encoded_file_uri() -> Result<(), String> {
        let uri = file_uri_for_path(&PathBuf::from("src lib.rs"))?;

        assert_eq!(uri.as_str(), "file:///src%20lib.rs");
        Ok(())
    }

    #[test]
    fn uri_path_encoding_preserves_path_syntax_and_escapes_spaces() {
        assert_eq!(
            encode_uri_path("workspace/src lib.rs"),
            "workspace/src%20lib.rs"
        );
    }

    fn sample_finding() -> Finding {
        Finding {
            id: "probe:pricing:88:predicate".to_string(),
            probe: Probe {
                id: ProbeId("probe:pricing:88:predicate".to_string()),
                location: SourceLocation {
                    file: PathBuf::from("src/pricing.rs"),
                    line: 88,
                    column: 1,
                },
                owner: None,
                family: ProbeFamily::Predicate,
                delta: DeltaKind::Control,
                before: None,
                after: None,
                expression: "amount >= threshold".to_string(),
                expected_sinks: Vec::new(),
                required_oracles: Vec::new(),
            },
            class: ExposureClass::WeaklyExposed,
            ripr: RiprEvidence {
                reach: StageEvidence::new(StageState::Yes, Confidence::High, "related tests found"),
                infect: StageEvidence::new(
                    StageState::Yes,
                    Confidence::High,
                    "predicate can alter branch behavior",
                ),
                propagate: StageEvidence::new(
                    StageState::Yes,
                    Confidence::Medium,
                    "branch influences return value",
                ),
                reveal: RevealEvidence {
                    observe: StageEvidence::new(
                        StageState::Weak,
                        Confidence::Medium,
                        "return value asserted",
                    ),
                    discriminate: StageEvidence::new(
                        StageState::Weak,
                        Confidence::Medium,
                        "boundary value missing",
                    ),
                },
            },
            confidence: 0.75,
            evidence: Vec::new(),
            missing: Vec::new(),
            stop_reasons: Vec::new(),
            related_tests: Vec::new(),
            recommended_next_step: Some("Add an exact boundary assertion.".to_string()),
        }
    }
}
