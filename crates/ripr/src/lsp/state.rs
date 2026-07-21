use super::gap_artifacts::{GapArtifactRejection, ValidatedGapArtifact};
use super::input_identity::LspAnalysisInputIdentity;
use super::uri::{file_uris_match, path_from_file_uri};
use crate::analysis::ClassifiedSeam;
use crate::app::Mode;
use crate::config::LspDiagnosticProfile;
use crate::domain::Finding;
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::time::{Duration, SystemTime};
use tower_lsp_server::ls_types::{
    Diagnostic, DidChangeTextDocumentParams, DidCloseTextDocumentParams, DidOpenTextDocumentParams,
    Uri,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum WorkspaceRootState {
    SelectedSingleRoot,
    WorkspaceAmbiguous,
    RootUnavailable,
    RootRemoved,
    RootChanged,
}

impl WorkspaceRootState {
    pub(super) fn as_str(&self) -> &'static str {
        match self {
            Self::SelectedSingleRoot => "selected_single_root",
            Self::WorkspaceAmbiguous => "workspace_ambiguous",
            Self::RootUnavailable => "root_unavailable",
            Self::RootRemoved => "root_removed",
            Self::RootChanged => "root_changed",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct WorkspaceRootAuthority {
    pub(super) state: WorkspaceRootState,
    pub(super) effective_root: Option<PathBuf>,
    pub(super) candidate_roots: Vec<PathBuf>,
    pub(super) detail: Option<String>,
}

impl WorkspaceRootAuthority {
    pub(super) fn selected(root: PathBuf) -> Self {
        Self {
            state: WorkspaceRootState::SelectedSingleRoot,
            effective_root: Some(root),
            candidate_roots: Vec::new(),
            detail: None,
        }
    }

    pub(super) fn unavailable(detail: impl Into<String>) -> Self {
        Self {
            state: WorkspaceRootState::RootUnavailable,
            effective_root: None,
            candidate_roots: Vec::new(),
            detail: Some(detail.into()),
        }
    }

    pub(super) fn ambiguous(candidate_roots: Vec<PathBuf>) -> Self {
        Self {
            state: WorkspaceRootState::WorkspaceAmbiguous,
            effective_root: None,
            candidate_roots,
            detail: Some("select one workspace folder, then restart or refresh the session".into()),
        }
    }

    pub(super) fn changed(previous: Option<PathBuf>, current: Option<PathBuf>) -> Self {
        Self {
            state: WorkspaceRootState::RootChanged,
            effective_root: current,
            candidate_roots: previous.into_iter().collect(),
            detail: Some("workspace root changed; refresh to obtain current evidence".into()),
        }
    }

    pub(super) fn removed(previous: Option<PathBuf>) -> Self {
        Self {
            state: WorkspaceRootState::RootRemoved,
            effective_root: None,
            candidate_roots: previous.into_iter().collect(),
            detail: Some(
                "the selected workspace root was removed; select a root and restart".into(),
            ),
        }
    }

    pub(super) fn input_identity(&self) -> Option<String> {
        self.effective_root
            .as_ref()
            .map(|root| format!("root:{}", root.display()))
    }

    pub(super) fn allows_analysis(&self) -> bool {
        matches!(self.state, WorkspaceRootState::SelectedSingleRoot)
            && self.effective_root.is_some()
    }
}

#[derive(Clone, Debug)]
pub(super) struct RefreshMetadata {
    pub(super) generated_at: SystemTime,
    pub(super) duration: Option<Duration>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum AnalysisAttemptState {
    Queued,
    Running,
    Succeeded,
    Failed,
    Cancelled,
    Superseded,
    Stopped,
}

impl AnalysisAttemptState {
    pub(super) fn as_str(self) -> &'static str {
        match self {
            Self::Queued => "queued",
            Self::Running => "running",
            Self::Succeeded => "succeeded",
            Self::Failed => "failed",
            Self::Cancelled => "cancelled",
            Self::Superseded => "superseded",
            Self::Stopped => "stopped",
        }
    }

    pub(super) fn allows_current_repairs(self) -> bool {
        matches!(self, Self::Succeeded)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct AnalysisFailure {
    pub(super) kind: String,
    pub(super) message: String,
}

#[derive(Clone, Debug)]
pub(super) struct AnalysisHealth {
    pub(super) attempt_id: Option<u64>,
    pub(super) state: AnalysisAttemptState,
    pub(super) reason: Option<String>,
    pub(super) requested_scope: Option<String>,
    pub(super) snapshot_id: Option<String>,
    pub(super) last_success_snapshot_id: Option<String>,
    pub(super) last_success_at: Option<SystemTime>,
    pub(super) snapshot_run_status: Option<String>,
    pub(super) current_input_identity: Option<String>,
    pub(super) last_success_input_identity: Option<String>,
    pub(super) failure: Option<AnalysisFailure>,
    pub(super) pending_attempt_id: Option<u64>,
    pub(super) pending_reason: Option<String>,
    pub(super) pending_scope: Option<String>,
}

impl Default for AnalysisHealth {
    fn default() -> Self {
        Self {
            attempt_id: None,
            state: AnalysisAttemptState::Stopped,
            reason: None,
            requested_scope: None,
            snapshot_id: None,
            last_success_snapshot_id: None,
            last_success_at: None,
            snapshot_run_status: None,
            current_input_identity: None,
            last_success_input_identity: None,
            failure: None,
            pending_attempt_id: None,
            pending_reason: None,
            pending_scope: None,
        }
    }
}

impl AnalysisHealth {
    pub(super) fn run_status(&self) -> &'static str {
        if self.snapshot_id.is_none() {
            return "no_snapshot";
        }
        if !matches!(self.state, AnalysisAttemptState::Succeeded) {
            return "stale";
        }
        match self.snapshot_run_status.as_deref() {
            Some("seams_deferred") => "seams_deferred",
            Some("cache_limited") => "cache_limited",
            Some("limited") => "limited",
            // RIPR-PROP-0019 (#1999): a partial partition is a limited run
            // state, never "full" — falling through here would present a
            // partial denominator as complete (#2142 review).
            Some(crate::analysis::PartialDiffScope::RUN_STATUS) => {
                crate::analysis::PartialDiffScope::RUN_STATUS
            }
            Some("stale") => "stale",
            _ => "full",
        }
    }

    pub(super) fn allows_current_repairs(&self) -> bool {
        self.snapshot_id.is_some()
            && self.state.allows_current_repairs()
            && self.run_status() != "stale"
    }

    pub(super) fn pending(&self) -> bool {
        self.pending_attempt_id.is_some()
    }
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
    /// The exact input identity that produced this completed snapshot. This
    /// is producer-owned provenance, not a renderer-derived summary.
    pub(super) input_identity: Option<LspAnalysisInputIdentity>,
    pub(super) base: Option<String>,
    pub(super) mode: Mode,
    pub(super) refresh: RefreshMetadata,
    pub(super) findings: Vec<Finding>,
    /// Profile used to derive the published finding diagnostics. Keeping this
    /// on the snapshot lets consistency validate the same bounded projection
    /// that the refresh actually published.
    pub(super) diagnostic_profile: LspDiagnosticProfile,
    /// Classified seam evidence. Empty when `seamDiagnostics` is off
    /// (the default), or when the seam inventory was deferred on an
    /// interactive open/save refresh (see RIPR-SPEC-0105). Use
    /// `seams_deferred` to distinguish "deferred" from "disabled".
    pub(super) classified_seams: Vec<ClassifiedSeam>,
    pub(super) gap_artifacts: Vec<ValidatedGapArtifact>,
    pub(super) gap_artifact_rejections: Vec<GapArtifactRejection>,
    pub(super) diagnostics_by_uri: BTreeMap<Uri, Vec<Diagnostic>>,
    /// The one immutable delivery selection shared by push publication and
    /// both pull handlers (#1973). Computed once at refresh-transaction
    /// prepare time (before any publication) and retained with the committed
    /// snapshot. `None` only for snapshots constructed outside the refresh
    /// transaction (unit-test fixtures); `commit_refresh_snapshot` fills it
    /// before the snapshot becomes the committed authority, so pull handlers
    /// always read a stored selection rather than re-evaluating the budget.
    pub(super) delivery_selection:
        Option<std::sync::Arc<super::diagnostic_budget::DiagnosticDeliverySelection>>,
    /// True when the seam inventory pass was intentionally skipped on an
    /// interactive open/save refresh to avoid the 336s cold-start cost.
    /// The snapshot carries complete diff-scoped findings but no seam
    /// evidence. `run_status` will be `"seams_deferred"` in this case.
    /// Invoking `ripr.refreshDiagnostics` produces a new snapshot with
    /// `seams_deferred = false` and the full seam inventory.
    pub(super) seams_deferred: bool,
    /// Partial diff-scope run state (RIPR-PROP-0019, #1999). `Some` only when
    /// the diff exceeded the partial-selection budget and the run analyzed a
    /// deterministic bounded partition (`limited_partial_scope`). The run
    /// status surfaces in workspace status and downgrades/suppresses
    /// diagnostics like the other limited-family states; the result is never
    /// a gate, baseline, badge, or RIPR Zero input.
    pub(super) partial_scope: Option<crate::analysis::PartialDiffScope>,
}

impl AnalysisSnapshot {
    pub(super) fn input_identity_id(&self) -> Option<String> {
        self.input_identity
            .as_ref()
            .map(LspAnalysisInputIdentity::stable_id)
    }

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
        let published_finding_count = super::diagnostics::canonical_finding_groups(&self.findings)
            .into_iter()
            .filter(|(primary, _)| {
                super::diagnostics::finding_is_visible_in_profile(self.diagnostic_profile, primary)
            })
            .count();
        !self.root.as_os_str().is_empty()
            && self
                .base
                .as_ref()
                .is_none_or(|base| !base.trim().is_empty())
            && !self.mode.as_str().is_empty()
            && published_finding_count + surfacable_seams + gap_diagnostics == diagnostic_count
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

    /// The stored document key and complete diagnostics for one URI. The
    /// stored key is the document identity the delivery selection was
    /// computed against, so transports must look the selection up by this
    /// key rather than the request URI's spelling.
    pub(super) fn diagnostics_entry_for_uri(&self, uri: &Uri) -> Option<(&Uri, &[Diagnostic])> {
        self.diagnostics_by_uri
            .get_key_value(uri)
            .map(|(stored_uri, diagnostics)| (stored_uri, diagnostics.as_slice()))
            .or_else(|| {
                self.diagnostics_by_uri
                    .iter()
                    .find(|(stored_uri, _)| file_uris_match(stored_uri, uri))
                    .map(|(stored_uri, diagnostics)| (stored_uri, diagnostics.as_slice()))
            })
    }

    /// The diagnostics one URI serves under the stored delivery selection
    /// (#1973). A committed snapshot always carries a selection, so this is
    /// the selection-filtered set push and pull agree on. A snapshot without
    /// a committed selection (unit-test fixture) serves its complete
    /// diagnostics, matching the pre-selection-authority behavior.
    pub(super) fn served_diagnostics_for_uri(&self, uri: &Uri) -> Vec<Diagnostic> {
        let Some((stored_uri, diagnostics)) = self.diagnostics_entry_for_uri(uri) else {
            return Vec::new();
        };
        match &self.delivery_selection {
            Some(selection) => selection.diagnostics_for_document(stored_uri.as_str(), diagnostics),
            None => diagnostics.to_vec(),
        }
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

    pub(super) fn canonical_finding_count(&self) -> usize {
        super::diagnostics::canonical_finding_groups(&self.findings).len()
    }

    pub(super) fn actionable_diagnostic_count(&self) -> usize {
        self.diagnostics_by_uri
            .values()
            .flatten()
            .filter(|diagnostic| diagnostic_has_string_data(diagnostic, "canonical_gap_id"))
            .count()
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
    fn analysis_health_run_status_preserves_limited_partial_scope() {
        // RIPR-PROP-0019 (#1999): a partial partition is a limited run
        // state; the DTO must not fall through to "full" (#2142 review).
        let health = AnalysisHealth {
            snapshot_id: Some("snap-1".to_string()),
            state: AnalysisAttemptState::Succeeded,
            snapshot_run_status: Some(crate::analysis::PartialDiffScope::RUN_STATUS.to_string()),
            ..AnalysisHealth::default()
        };
        assert_eq!(health.run_status(), "limited_partial_scope");
    }

    #[test]
    fn snapshot_consistency_counts_gap_record_diagnostics() -> Result<(), String> {
        let uri = test_uri("file:///workspace/src/pricing.rs")?;
        let mut diagnostics_by_uri = BTreeMap::new();
        diagnostics_by_uri.insert(uri, vec![gap_diagnostic()]);
        let snapshot = AnalysisSnapshot {
            root: PathBuf::from("/workspace"),
            input_identity: None,
            base: None,
            mode: Mode::Draft,
            refresh: RefreshMetadata::default(),
            findings: Vec::new(),
            diagnostic_profile: LspDiagnosticProfile::Full,
            classified_seams: Vec::new(),
            gap_artifacts: Vec::new(),
            gap_artifact_rejections: Vec::new(),
            diagnostics_by_uri,
            delivery_selection: None,
            seams_deferred: false,
            partial_scope: None,
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
            input_identity: None,
            base: None,
            mode: Mode::Draft,
            refresh: RefreshMetadata::default(),
            findings: Vec::new(),
            diagnostic_profile: LspDiagnosticProfile::Full,
            classified_seams: Vec::new(),
            gap_artifacts: Vec::new(),
            gap_artifact_rejections: Vec::new(),
            diagnostics_by_uri,
            delivery_selection: None,
            seams_deferred: false,
            partial_scope: None,
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
