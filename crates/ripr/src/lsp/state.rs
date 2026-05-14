use super::gap_artifacts::ValidatedGapArtifact;
use super::uri::{file_uris_match, path_from_file_uri};
use crate::analysis::ClassifiedSeam;
use crate::app::Mode;
use crate::domain::Finding;
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::time::{Duration, SystemTime};
use tower_lsp_server::ls_types::{
    Diagnostic, DidChangeTextDocumentParams, DidCloseTextDocumentParams, DidOpenTextDocumentParams,
    Uri,
};

#[derive(Clone, Debug)]
pub(super) struct RefreshMetadata {
    pub(super) generated_at: SystemTime,
    pub(super) duration: Option<Duration>,
}

impl RefreshMetadata {
    pub(super) fn generated_now() -> Self {
        Self {
            generated_at: SystemTime::now(),
            duration: None,
        }
    }

    pub(super) fn record_duration(&mut self, duration: Duration) {
        self.duration = Some(duration);
    }

    pub(super) fn age(&self) -> Option<Duration> {
        SystemTime::now().duration_since(self.generated_at).ok()
    }
}

impl Default for RefreshMetadata {
    fn default() -> Self {
        Self::generated_now()
    }
}

#[derive(Clone, Debug)]
pub(super) struct AnalysisSnapshot {
    pub(super) root: PathBuf,
    pub(super) base: Option<String>,
    pub(super) mode: Mode,
    pub(super) refresh: RefreshMetadata,
    pub(super) findings: Vec<Finding>,
    /// Classified seam evidence. Empty when `seamDiagnostics` is off
    /// (the default). Populated lazily on workspace refresh when the
    /// flag is enabled.
    pub(super) classified_seams: Vec<ClassifiedSeam>,
    pub(super) gap_artifacts: Vec<ValidatedGapArtifact>,
    pub(super) diagnostics_by_uri: BTreeMap<Uri, Vec<Diagnostic>>,
}

impl AnalysisSnapshot {
    pub(super) fn is_consistent(&self) -> bool {
        let diagnostic_count = self
            .diagnostics_by_uri
            .values()
            .map(Vec::len)
            .sum::<usize>();
        let surfacable_seams = self
            .classified_seams
            .iter()
            .filter(|entry| {
                super::diagnostics::diagnostic_severity_for_grip_class(entry.class).is_some()
            })
            .count();
        let gap_diagnostics = self
            .diagnostics_by_uri
            .values()
            .flatten()
            .filter(|diagnostic| diagnostic_has_string_data(diagnostic, "gap_id"))
            .count();
        !self.root.as_os_str().is_empty()
            && self
                .base
                .as_ref()
                .is_none_or(|base| !base.trim().is_empty())
            && !self.mode.as_str().is_empty()
            && self.findings.len() + surfacable_seams + gap_diagnostics == diagnostic_count
            && self
                .gap_artifacts
                .iter()
                .all(ValidatedGapArtifact::is_safe_projection_input)
    }

    pub(super) fn diagnostics_for_uri(&self, uri: &Uri) -> Option<&[Diagnostic]> {
        self.diagnostics_by_uri
            .get(uri)
            .or_else(|| {
                self.diagnostics_by_uri
                    .iter()
                    .find(|(stored_uri, _)| file_uris_match(stored_uri, uri))
                    .map(|(_, diagnostics)| diagnostics)
            })
            .map(Vec::as_slice)
    }

    pub(super) fn diagnostic_count(&self) -> usize {
        self.diagnostics_by_uri
            .values()
            .map(Vec::len)
            .sum::<usize>()
    }

    pub(super) fn diagnostic_uri_count(&self) -> usize {
        self.diagnostics_by_uri.len()
    }

    pub(super) fn finding_count(&self) -> usize {
        self.findings.len()
    }

    pub(super) fn seam_diagnostic_count(&self) -> usize {
        self.classified_seams.len()
    }

    pub(super) fn finding_by_id(&self, finding_id: &str) -> Option<&Finding> {
        self.findings
            .iter()
            .find(|finding| finding.id == finding_id)
    }

    pub(super) fn finding_for_diagnostic(&self, diagnostic: &Diagnostic) -> Option<&Finding> {
        let finding_id = diagnostic
            .data
            .as_ref()
            .and_then(|data| data.get("finding_id"))
            .and_then(|value| value.as_str())?;
        self.finding_by_id(finding_id)
    }

    /// Look up the classified seam matching a diagnostic's
    /// `data.seam_id` field, if present. Mirrors
    /// `finding_for_diagnostic` for the seam evidence diagnostics
    /// introduced by `lsp/repo-seam-diagnostics-v1`. Returns `None`
    /// when the diagnostic carries a `finding_id` instead, or when
    /// the snapshot was built without seam diagnostics enabled.
    pub(super) fn classified_seam_for_diagnostic(
        &self,
        diagnostic: &Diagnostic,
    ) -> Option<&ClassifiedSeam> {
        let seam_id = diagnostic
            .data
            .as_ref()
            .and_then(|data| data.get("seam_id"))
            .and_then(|value| value.as_str())?;
        self.classified_seams
            .iter()
            .find(|entry| entry.seam.id().as_str() == seam_id)
    }

    pub(super) fn classified_seam_by_id(&self, seam_id: &str) -> Option<&ClassifiedSeam> {
        self.classified_seams
            .iter()
            .find(|entry| entry.seam.id().as_str() == seam_id)
    }
}

fn diagnostic_has_string_data(diagnostic: &Diagnostic, key: &str) -> bool {
    diagnostic
        .data
        .as_ref()
        .and_then(|data| data.get(key))
        .and_then(|value| value.as_str())
        .is_some()
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct DocumentState {
    pub(super) uri: Uri,
    pub(super) path: PathBuf,
    pub(super) version: Option<i32>,
    pub(super) text: String,
}

#[derive(Default)]
pub(super) struct DocumentStore {
    pub(super) documents: BTreeMap<Uri, DocumentState>,
}

impl DocumentStore {
    pub(super) fn open(&mut self, params: DidOpenTextDocumentParams) {
        let uri = params.text_document.uri;
        let state = DocumentState {
            path: document_path(&uri),
            uri: uri.clone(),
            version: Some(params.text_document.version),
            text: params.text_document.text,
        };
        self.documents.insert(uri, state);
    }

    pub(super) fn change(&mut self, params: DidChangeTextDocumentParams) {
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

    pub(super) fn close(&mut self, params: DidCloseTextDocumentParams) {
        self.documents.remove(&params.text_document.uri);
    }
}

fn document_path(uri: &Uri) -> PathBuf {
    path_from_file_uri(uri).unwrap_or_else(|| PathBuf::from(uri.as_str()))
}

pub(super) fn format_duration(duration: Duration) -> String {
    if duration.as_secs() == 0 {
        return format!("{} ms", duration.as_millis());
    }
    if duration.as_secs() == 1 {
        return "1 second".to_string();
    }
    format!("{} seconds", duration.as_secs())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tower_lsp_server::ls_types::{Position, Range};

    #[test]
    fn snapshot_consistency_counts_gap_record_diagnostics() -> Result<(), String> {
        let uri = test_uri("file:///workspace/src/pricing.rs")?;
        let mut diagnostics_by_uri = BTreeMap::new();
        diagnostics_by_uri.insert(uri, vec![gap_diagnostic()]);
        let snapshot = AnalysisSnapshot {
            root: PathBuf::from("/workspace"),
            base: None,
            mode: Mode::Draft,
            refresh: RefreshMetadata::default(),
            findings: Vec::new(),
            classified_seams: Vec::new(),
            gap_artifacts: Vec::new(),
            diagnostics_by_uri,
        };

        if !snapshot.is_consistent() {
            return Err("gap diagnostics should count as explicit diagnostics".to_string());
        }
        Ok(())
    }

    #[test]
    fn snapshot_consistency_rejects_unknown_extra_diagnostic() -> Result<(), String> {
        let uri = test_uri("file:///workspace/src/pricing.rs")?;
        let mut diagnostics_by_uri = BTreeMap::new();
        diagnostics_by_uri.insert(uri, vec![plain_diagnostic()]);
        let snapshot = AnalysisSnapshot {
            root: PathBuf::from("/workspace"),
            base: None,
            mode: Mode::Draft,
            refresh: RefreshMetadata::default(),
            findings: Vec::new(),
            classified_seams: Vec::new(),
            gap_artifacts: Vec::new(),
            diagnostics_by_uri,
        };

        if snapshot.is_consistent() {
            return Err(
                "plain diagnostics should still require matching source evidence".to_string(),
            );
        }
        Ok(())
    }

    fn gap_diagnostic() -> Diagnostic {
        let mut diagnostic = plain_diagnostic();
        diagnostic.data = Some(serde_json::json!({
            "source": "gap_decision_ledger",
            "gap_id": "gap:pr:pricing:threshold-boundary"
        }));
        diagnostic
    }

    fn plain_diagnostic() -> Diagnostic {
        Diagnostic {
            range: Range {
                start: Position {
                    line: 10,
                    character: 0,
                },
                end: Position {
                    line: 10,
                    character: 120,
                },
            },
            severity: None,
            code: None,
            code_description: None,
            source: Some("ripr".to_string()),
            message: "test diagnostic".to_string(),
            related_information: None,
            tags: None,
            data: None,
        }
    }

    fn test_uri(uri: &str) -> Result<Uri, String> {
        uri.parse::<Uri>()
            .map_err(|err| format!("failed to parse test URI: {err}"))
    }
}
