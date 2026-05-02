use crate::app::{CheckInput, OutputFormat, check_workspace};
use crate::domain::{ExposureClass, Finding, RelatedTest};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use tower_lsp_server::jsonrpc::Result as LspResult;
use tower_lsp_server::ls_types::{
    CodeAction, CodeActionKind, CodeActionOrCommand, CodeActionProviderCapability,
    CodeActionResponse, Command, Diagnostic, DiagnosticRelatedInformation, DiagnosticSeverity,
    DidChangeTextDocumentParams, DidCloseTextDocumentParams, DidOpenTextDocumentParams,
    DidSaveTextDocumentParams, ExecuteCommandOptions, ExecuteCommandParams, Hover, HoverContents,
    HoverParams, HoverProviderCapability, InitializeParams, InitializeResult, LSPAny, Location,
    MarkupContent, MarkupKind, MessageType, NumberOrString, Position, Range, ServerCapabilities,
    ServerInfo, TextDocumentSyncCapability, TextDocumentSyncKind, Uri,
};
use tower_lsp_server::{Client, LanguageServer, LspService, Server};

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

struct Backend {
    client: Client,
    root: Mutex<PathBuf>,
    documents: Mutex<DocumentStore>,
    last_diagnostic_uris: Mutex<BTreeSet<Uri>>,
}

impl Backend {
    fn new(client: Client, root: PathBuf) -> Self {
        Self {
            client,
            root: Mutex::new(root),
            documents: Mutex::new(DocumentStore::default()),
            last_diagnostic_uris: Mutex::new(BTreeSet::new()),
        }
    }

    async fn refresh_diagnostics(&self) {
        let Some(root) = self.root() else {
            return;
        };
        let batches =
            match tokio::task::spawn_blocking(move || workspace_diagnostic_batches(&root)).await {
                Ok(Ok(batches)) => batches,
                Ok(Err(err)) => {
                    self.report_refresh_failure(err).await;
                    return;
                }
                Err(err) => {
                    self.report_refresh_failure(format!("analysis task failed: {err}"))
                        .await;
                    return;
                }
            };
        let Some(refresh) = self.refresh_plan(batches) else {
            return;
        };
        for batch in refresh.publish_batches {
            self.client
                .publish_diagnostics(batch.uri, batch.diagnostics, None)
                .await;
        }
        for uri in refresh.clear_uris {
            self.client.publish_diagnostics(uri, Vec::new(), None).await;
        }
    }

    async fn report_refresh_failure(&self, message: String) {
        self.client
            .log_message(
                MessageType::WARNING,
                format!("ripr analysis refresh failed: {message}"),
            )
            .await;
        for uri in self.clear_all_diagnostic_uris() {
            self.client.publish_diagnostics(uri, Vec::new(), None).await;
        }
    }

    fn refresh_plan(&self, batches: Vec<DiagnosticBatch>) -> Option<DiagnosticRefreshPlan> {
        let Ok(mut last_diagnostic_uris) = self.last_diagnostic_uris.lock() else {
            return None;
        };
        let refresh = diagnostic_refresh_plan(&last_diagnostic_uris, batches);
        *last_diagnostic_uris = refresh.current_uris.clone();
        Some(refresh)
    }

    fn clear_all_diagnostic_uris(&self) -> Vec<Uri> {
        let Ok(mut last_diagnostic_uris) = self.last_diagnostic_uris.lock() else {
            return Vec::new();
        };
        take_all_uris(&mut last_diagnostic_uris)
    }

    fn root(&self) -> Option<PathBuf> {
        let Ok(root) = self.root.lock() else {
            return None;
        };
        Some(root.clone())
    }

    fn set_root(&self, root: PathBuf) {
        let Ok(mut current_root) = self.root.lock() else {
            return;
        };
        *current_root = root;
    }

    fn open_document(&self, params: DidOpenTextDocumentParams) {
        let Ok(mut documents) = self.documents.lock() else {
            return;
        };
        documents.open(params);
    }

    fn change_document(&self, params: DidChangeTextDocumentParams) {
        let Ok(mut documents) = self.documents.lock() else {
            return;
        };
        documents.change(params);
    }

    fn close_document(&self, params: DidCloseTextDocumentParams) {
        let Ok(mut documents) = self.documents.lock() else {
            return;
        };
        documents.close(params);
    }
}

impl LanguageServer for Backend {
    async fn initialize(&self, params: InitializeParams) -> LspResult<InitializeResult> {
        let fallback_root = self
            .root()
            .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));
        self.set_root(root_from_initialize_params(&params, &fallback_root));
        Ok(initialize_result())
    }

    async fn shutdown(&self) -> LspResult<()> {
        Ok(())
    }

    async fn did_open(&self, params: DidOpenTextDocumentParams) {
        self.open_document(params);
        self.refresh_diagnostics().await;
    }

    async fn did_change(&self, params: DidChangeTextDocumentParams) {
        self.change_document(params);
        self.refresh_diagnostics().await;
    }

    async fn did_close(&self, params: DidCloseTextDocumentParams) {
        self.close_document(params);
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

#[derive(Clone, Debug, PartialEq, Eq)]
struct DocumentState {
    uri: Uri,
    path: PathBuf,
    version: Option<i32>,
    text: String,
}

#[derive(Default)]
struct DocumentStore {
    documents: BTreeMap<Uri, DocumentState>,
}

impl DocumentStore {
    fn open(&mut self, params: DidOpenTextDocumentParams) {
        let uri = params.text_document.uri;
        let state = DocumentState {
            path: document_path(&uri),
            uri: uri.clone(),
            version: Some(params.text_document.version),
            text: params.text_document.text,
        };
        self.documents.insert(uri, state);
    }

    fn change(&mut self, params: DidChangeTextDocumentParams) {
        let uri = params.text_document.uri;
        let version = Some(params.text_document.version);
        let text = params
            .content_changes
            .into_iter()
            .last()
            .map(|change| change.text);
        if let Some(state) = self.documents.get_mut(&uri) {
            state.version = version;
            if let Some(text) = text {
                state.text = text;
            }
            return;
        }
        let Some(text) = text else {
            return;
        };
        let state = DocumentState {
            path: document_path(&uri),
            uri: uri.clone(),
            version,
            text,
        };
        self.documents.insert(uri, state);
    }

    fn close(&mut self, params: DidCloseTextDocumentParams) {
        self.documents.remove(&params.text_document.uri);
    }
}

fn document_path(uri: &Uri) -> PathBuf {
    path_from_file_uri(uri).unwrap_or_else(|| PathBuf::from(uri.as_str()))
}

struct DiagnosticRefreshPlan {
    publish_batches: Vec<DiagnosticBatch>,
    clear_uris: Vec<Uri>,
    current_uris: BTreeSet<Uri>,
}

fn diagnostic_refresh_plan(
    previous_uris: &BTreeSet<Uri>,
    batches: Vec<DiagnosticBatch>,
) -> DiagnosticRefreshPlan {
    let current_uris = batches
        .iter()
        .map(|batch| batch.uri.clone())
        .collect::<BTreeSet<_>>();
    let clear_uris = previous_uris
        .difference(&current_uris)
        .cloned()
        .collect::<Vec<_>>();
    DiagnosticRefreshPlan {
        publish_batches: batches,
        clear_uris,
        current_uris,
    }
}

fn take_all_uris(uris: &mut BTreeSet<Uri>) -> Vec<Uri> {
    let cleared = uris.iter().cloned().collect::<Vec<_>>();
    uris.clear();
    cleared
}

pub fn workspace_diagnostic_batches(root: &Path) -> Result<Vec<DiagnosticBatch>, String> {
    let input = CheckInput {
        root: root.to_path_buf(),
        format: OutputFormat::Json,
        ..CheckInput::default()
    };
    let output =
        check_workspace(input).map_err(|err| format!("workspace analysis failed: {err}"))?;
    let mut grouped = BTreeMap::<Uri, Vec<Diagnostic>>::new();
    for finding in &output.findings {
        let path = absolute_finding_path(&output.root, finding);
        let uri = file_uri_for_path(&path)?;
        grouped
            .entry(uri)
            .or_default()
            .push(diagnostic_for_finding(&output.root, finding));
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
                commands: vec![REFRESH_COMMAND.to_string()],
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

#[allow(deprecated)]
fn root_from_initialize_params(params: &InitializeParams, fallback_root: &Path) -> PathBuf {
    params
        .workspace_folders
        .as_ref()
        .and_then(|folders| folders.first())
        .and_then(|folder| path_from_file_uri(&folder.uri))
        .or_else(|| params.root_uri.as_ref().and_then(path_from_file_uri))
        .unwrap_or_else(|| fallback_root.to_path_buf())
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
                title: "Copy ripr context".to_string(),
                command: COPY_CONTEXT_COMMAND.to_string(),
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

fn diagnostic_for_finding(root: &Path, finding: &Finding) -> Diagnostic {
    let line = finding.probe.location.line.saturating_sub(1) as u32;
    Diagnostic {
        range: Range {
            start: Position { line, character: 0 },
            end: Position {
                line,
                character: 120,
            },
        },
        severity: Some(diagnostic_severity_for_class(&finding.class)),
        code: Some(NumberOrString::String(finding.class.as_str().to_string())),
        code_description: None,
        source: Some("ripr".to_string()),
        message: lsp_message(finding),
        related_information: related_information_for_finding(root, finding),
        tags: None,
        data: Some(serde_json::json!({
            "schema_version": "0.1",
            "finding_id": finding.id.as_str(),
            "probe_id": finding.probe.id.to_string(),
            "classification": finding.class.as_str(),
            "probe_family": finding.probe.family.as_str(),
            "confidence": finding.confidence,
            "source_range": {
                "file": finding.probe.location.file.display().to_string(),
                "line": finding.probe.location.line,
                "column": finding.probe.location.column,
            },
        })),
    }
}

fn related_information_for_finding(
    root: &Path,
    finding: &Finding,
) -> Option<Vec<DiagnosticRelatedInformation>> {
    let related = finding
        .related_tests
        .iter()
        .filter_map(|test| related_information_for_test(root, test))
        .collect::<Vec<_>>();
    if related.is_empty() {
        None
    } else {
        Some(related)
    }
}

fn related_information_for_test(
    root: &Path,
    test: &RelatedTest,
) -> Option<DiagnosticRelatedInformation> {
    let path = absolute_related_test_path(root, test);
    let uri = file_uri_for_path(&path).ok()?;
    let line = test.line.saturating_sub(1) as u32;
    Some(DiagnosticRelatedInformation {
        location: Location {
            uri,
            range: Range {
                start: Position { line, character: 0 },
                end: Position {
                    line,
                    character: 120,
                },
            },
        },
        message: related_test_message(test),
    })
}

fn related_test_message(test: &RelatedTest) -> String {
    let strength = test.oracle_strength.as_str();
    match &test.oracle {
        Some(oracle) => format!(
            "Related test `{}` has {strength} oracle: {oracle}",
            test.name
        ),
        None => format!("Related test `{}` has {strength} oracle", test.name),
    }
}

fn diagnostic_severity_for_class(class: &ExposureClass) -> DiagnosticSeverity {
    match class {
        ExposureClass::Exposed
        | ExposureClass::PropagationUnknown
        | ExposureClass::StaticUnknown => DiagnosticSeverity::INFORMATION,
        ExposureClass::WeaklyExposed
        | ExposureClass::ReachableUnrevealed
        | ExposureClass::NoStaticPath
        | ExposureClass::InfectionUnknown => DiagnosticSeverity::WARNING,
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

fn absolute_related_test_path(root: &Path, test: &RelatedTest) -> PathBuf {
    if test.file.is_absolute() {
        test.file.clone()
    } else {
        root.join(&test.file)
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

fn path_from_file_uri(uri: &Uri) -> Option<PathBuf> {
    let raw = uri.as_str();
    let path = raw.strip_prefix("file://")?;
    let decoded = percent_decode_uri_path(path)?;
    let path = if is_windows_drive_uri_path(&decoded) {
        decoded[1..].to_string()
    } else {
        decoded
    };
    Some(PathBuf::from(path))
}

fn is_windows_drive_uri_path(path: &str) -> bool {
    let bytes = path.as_bytes();
    bytes.len() >= 3 && bytes[0] == b'/' && bytes[2] == b':' && bytes[1].is_ascii_alphabetic()
}

fn percent_decode_uri_path(path: &str) -> Option<String> {
    let bytes = path.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%' {
            let high = hex_value(*bytes.get(index + 1)?)?;
            let low = hex_value(*bytes.get(index + 2)?)?;
            decoded.push((high << 4) | low);
            index += 3;
        } else {
            decoded.push(bytes[index]);
            index += 1;
        }
    }
    String::from_utf8(decoded).ok()
}

fn hex_value(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
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
        Backend, COPY_CONTEXT_COMMAND, DocumentStore, HOVER_TEXT, REFRESH_COMMAND,
        code_action_response, diagnostic_for_finding, diagnostic_refresh_plan,
        diagnostic_severity_for_class, encode_uri_path, file_uri_for_path, hover_response,
        initialize_result, path_from_file_uri, root_from_initialize_params, take_all_uris,
    };
    use crate::domain::{
        Confidence, DeltaKind, ExposureClass, Finding, OracleStrength, Probe, ProbeFamily, ProbeId,
        RelatedTest, RevealEvidence, RiprEvidence, SourceLocation, StageEvidence, StageState,
    };
    use std::collections::BTreeSet;
    use std::path::{Path, PathBuf};
    use tower_lsp_server::LspService;
    use tower_lsp_server::ls_types::{
        CodeActionOrCommand, DiagnosticSeverity, DidChangeTextDocumentParams,
        DidCloseTextDocumentParams, DidOpenTextDocumentParams, HoverContents,
        HoverProviderCapability, InitializeParams, NumberOrString, TextDocumentContentChangeEvent,
        TextDocumentIdentifier, TextDocumentItem, TextDocumentSyncCapability, TextDocumentSyncKind,
        VersionedTextDocumentIdentifier, WorkspaceFolder,
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
        assert_eq!(commands, vec![REFRESH_COMMAND]);
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
                    "Copy ripr context",
                    COPY_CONTEXT_COMMAND,
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
        let diagnostic = diagnostic_for_finding(Path::new("/workspace"), &finding);

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
        assert_eq!(data["schema_version"], "0.1");
        assert_eq!(data["finding_id"], "probe:pricing:88:predicate");
        assert_eq!(data["probe_id"], "probe:pricing:88:predicate");
        assert_eq!(data["classification"], "weakly_exposed");
        assert_eq!(data["probe_family"], "predicate");
        assert_eq!(data["confidence"], 0.75);
        assert_eq!(data["source_range"]["file"], "src/pricing.rs");
        assert_eq!(data["source_range"]["line"], 88);
        assert_eq!(data["source_range"]["column"], 1);
        Ok(())
    }

    #[test]
    fn diagnostic_for_finding_attaches_related_test_information() -> Result<(), String> {
        let mut finding = sample_finding();
        finding.related_tests.push(RelatedTest {
            name: "discount_boundary_is_exact".to_string(),
            file: PathBuf::from("tests/pricing.rs"),
            line: 12,
            oracle: Some("assert_eq!(total, expected)".to_string()),
            oracle_strength: OracleStrength::Strong,
        });

        let diagnostic = diagnostic_for_finding(Path::new("/workspace"), &finding);
        let Some(related) = diagnostic.related_information else {
            return Err("expected related diagnostic information".to_string());
        };

        assert_eq!(related.len(), 1);
        assert_eq!(
            related[0].location.uri.as_str(),
            "file:///workspace/tests/pricing.rs"
        );
        assert_eq!(related[0].location.range.start.line, 11);
        assert_eq!(
            related[0].message,
            "Related test `discount_boundary_is_exact` has strong oracle: assert_eq!(total, expected)"
        );
        Ok(())
    }

    #[test]
    fn diagnostic_severity_tracks_static_exposure_class() {
        let cases = [
            (ExposureClass::Exposed, DiagnosticSeverity::INFORMATION),
            (ExposureClass::WeaklyExposed, DiagnosticSeverity::WARNING),
            (
                ExposureClass::ReachableUnrevealed,
                DiagnosticSeverity::WARNING,
            ),
            (ExposureClass::NoStaticPath, DiagnosticSeverity::WARNING),
            (ExposureClass::InfectionUnknown, DiagnosticSeverity::WARNING),
            (
                ExposureClass::PropagationUnknown,
                DiagnosticSeverity::INFORMATION,
            ),
            (
                ExposureClass::StaticUnknown,
                DiagnosticSeverity::INFORMATION,
            ),
        ];

        for (class, expected) in cases {
            assert_eq!(diagnostic_severity_for_class(&class), expected);
        }
    }

    #[test]
    fn diagnostic_refresh_plan_clears_stale_previous_uris() -> Result<(), String> {
        let stale_uri = test_uri("file:///workspace/src/stale.rs")?;
        let current_uri = test_uri("file:///workspace/src/current.rs")?;
        let mut previous_uris = BTreeSet::new();
        previous_uris.insert(stale_uri.clone());
        previous_uris.insert(current_uri.clone());

        let plan = diagnostic_refresh_plan(
            &previous_uris,
            vec![super::DiagnosticBatch {
                uri: current_uri.clone(),
                diagnostics: Vec::new(),
            }],
        );

        assert_eq!(plan.publish_batches.len(), 1);
        assert_eq!(plan.publish_batches[0].uri, current_uri);
        assert_eq!(plan.clear_uris, vec![stale_uri]);
        assert_eq!(plan.current_uris.len(), 1);
        Ok(())
    }

    #[test]
    fn take_all_uris_returns_and_clears_previous_diagnostic_uris() -> Result<(), String> {
        let first_uri = test_uri("file:///workspace/src/first.rs")?;
        let second_uri = test_uri("file:///workspace/src/second.rs")?;
        let mut uris = BTreeSet::new();
        uris.insert(first_uri.clone());
        uris.insert(second_uri.clone());

        let cleared = take_all_uris(&mut uris);

        assert_eq!(cleared, vec![first_uri, second_uri]);
        assert!(uris.is_empty());
        Ok(())
    }

    #[test]
    fn refresh_failure_reports_and_clears_tracked_diagnostics() -> Result<(), String> {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|err| format!("failed to start test runtime: {err}"))?;
        runtime.block_on(async {
            let (service, _socket) =
                LspService::new(|client| Backend::new(client, PathBuf::from(".")));
            let backend = service.inner();
            let tracked_uri = test_uri("file:///workspace/src/stale.rs")?;
            let Some(_) = backend.refresh_plan(vec![super::DiagnosticBatch {
                uri: tracked_uri.clone(),
                diagnostics: Vec::new(),
            }]) else {
                return Err("expected refresh plan".to_string());
            };

            backend
                .report_refresh_failure("simulated analysis failure".to_string())
                .await;

            assert!(backend.clear_all_diagnostic_uris().is_empty());
            Ok(())
        })
    }

    #[test]
    fn document_store_tracks_open_change_and_close() -> Result<(), String> {
        let uri = test_uri("file:///workspace/src/lib.rs")?;
        let mut store = DocumentStore::default();

        store.open(DidOpenTextDocumentParams {
            text_document: TextDocumentItem::new(
                uri.clone(),
                "rust".to_string(),
                1,
                "fn old() {}".to_string(),
            ),
        });

        let Some(opened) = store.documents.get(&uri) else {
            return Err("expected opened document".to_string());
        };
        assert_eq!(opened.path, PathBuf::from("/workspace/src/lib.rs"));
        assert_eq!(opened.version, Some(1));
        assert_eq!(opened.text, "fn old() {}");

        store.change(DidChangeTextDocumentParams {
            text_document: VersionedTextDocumentIdentifier::new(uri.clone(), 2),
            content_changes: vec![TextDocumentContentChangeEvent {
                range: None,
                range_length: None,
                text: "fn new() {}".to_string(),
            }],
        });

        let Some(changed) = store.documents.get(&uri) else {
            return Err("expected changed document".to_string());
        };
        assert_eq!(changed.version, Some(2));
        assert_eq!(changed.text, "fn new() {}");

        store.close(DidCloseTextDocumentParams {
            text_document: TextDocumentIdentifier::new(uri.clone()),
        });

        assert!(!store.documents.contains_key(&uri));
        Ok(())
    }

    #[test]
    fn document_store_creates_document_from_full_change_when_missing() -> Result<(), String> {
        let uri = test_uri("file:///workspace/src/lib.rs")?;
        let mut store = DocumentStore::default();

        store.change(DidChangeTextDocumentParams {
            text_document: VersionedTextDocumentIdentifier::new(uri.clone(), 7),
            content_changes: vec![TextDocumentContentChangeEvent {
                range: None,
                range_length: None,
                text: "fn discovered() {}".to_string(),
            }],
        });

        let Some(document) = store.documents.get(&uri) else {
            return Err("expected document from full change".to_string());
        };
        assert_eq!(document.version, Some(7));
        assert_eq!(document.text, "fn discovered() {}");
        Ok(())
    }

    #[test]
    fn initialize_root_prefers_first_workspace_folder() -> Result<(), String> {
        let fallback = PathBuf::from("/fallback");
        let params = initialize_params(
            Some(vec![
                WorkspaceFolder {
                    uri: test_uri("file:///workspace/main")?,
                    name: "main".to_string(),
                },
                WorkspaceFolder {
                    uri: test_uri("file:///workspace/other")?,
                    name: "other".to_string(),
                },
            ]),
            Some(test_uri("file:///workspace/root-uri")?),
        );

        let root = root_from_initialize_params(&params, &fallback);

        assert_eq!(root, PathBuf::from("/workspace/main"));
        Ok(())
    }

    #[test]
    fn initialize_root_uses_root_uri_when_workspace_folders_are_missing() -> Result<(), String> {
        let fallback = PathBuf::from("/fallback");
        let params = initialize_params(None, Some(test_uri("file:///workspace/root-uri")?));

        let root = root_from_initialize_params(&params, &fallback);

        assert_eq!(root, PathBuf::from("/workspace/root-uri"));
        Ok(())
    }

    #[test]
    fn initialize_root_falls_back_to_process_cwd_when_no_lsp_root_exists() {
        let fallback = PathBuf::from("/fallback");
        let params = initialize_params(None, None);

        let root = root_from_initialize_params(&params, &fallback);

        assert_eq!(root, fallback);
    }

    #[test]
    fn file_uri_to_path_decodes_spaces_and_windows_drive_prefix() -> Result<(), String> {
        let uri = test_uri(&format!("file:///{}{}", "C%3A", "/path/to/ripr%20repo"))?;

        let Some(path) = path_from_file_uri(&uri) else {
            return Err("expected path from file URI".to_string());
        };

        assert_eq!(
            path,
            PathBuf::from(format!("{}{}", "C:", "/path/to/ripr repo"))
        );
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

    fn test_uri(uri: &str) -> Result<tower_lsp_server::ls_types::Uri, String> {
        uri.parse::<tower_lsp_server::ls_types::Uri>()
            .map_err(|err| format!("failed to parse test URI: {err}"))
    }

    #[allow(deprecated)]
    fn initialize_params(
        workspace_folders: Option<Vec<WorkspaceFolder>>,
        root_uri: Option<tower_lsp_server::ls_types::Uri>,
    ) -> InitializeParams {
        InitializeParams {
            workspace_folders,
            root_uri,
            ..InitializeParams::default()
        }
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
