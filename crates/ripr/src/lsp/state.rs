use super::component_outcome::ComponentOutcome;
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
    Uri, WorkspaceFolder,
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

/// One canonical workspace-folder entry (#2036, RIPR-SPEC-0139): the
/// client-sent URI spelling beside its normalized lexical path identity.
/// Identity is lexical normalization only — never symlink canonicalization —
/// matching the root-identity invariant in `uri.rs`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct WorkspaceFolderEntry {
    pub(super) uri: String,
    pub(super) path: PathBuf,
}

/// Why the stored workspace-folder set resolves to its current authority
/// state (#2036, RIPR-SPEC-0139). More than one entry is always ambiguous:
/// the server never falls back to the first folder.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum WorkspaceFolderSelection {
    NoFolders,
    SingleFolder,
    AmbiguousFolders,
}

/// Typed rejection kind for one `workspace/didChangeWorkspaceFolders` event
/// or full-list reconciliation (#2036, RIPR-SPEC-0139). A rejected update
/// leaves the stored set untouched; the handler surfaces a typed bounded
/// status instead of a silent fallback.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum WorkspaceFolderEventRejectionKind {
    InvalidFileUri,
    DuplicateAddition,
    UnknownRemoval,
    ContradictoryEvent,
}

impl WorkspaceFolderEventRejectionKind {
    pub(super) fn as_str(self) -> &'static str {
        match self {
            Self::InvalidFileUri => "invalid_file_uri",
            Self::DuplicateAddition => "duplicate_addition",
            Self::UnknownRemoval => "unknown_removal",
            Self::ContradictoryEvent => "contradictory_event",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct WorkspaceFolderEventRejection {
    pub(super) kind: WorkspaceFolderEventRejectionKind,
    pub(super) detail: String,
}

/// Outcome of applying one accepted event delta to the stored set.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct WorkspaceFolderEventOutcome {
    /// Sequence identity of the applied event (`last applied event
    /// identity` in the folder-set identity model).
    pub(super) event_id: u64,
    /// Whether the delta changed the stored entries.
    pub(super) changed: bool,
    /// The folder-set epoch after the event, captured under the same lock
    /// so a later reconciliation round-trip binds to this exact set state.
    pub(super) folder_set_epoch: u64,
}

/// The stored workspace-folder-set identity (#2036, RIPR-SPEC-0139):
/// ordered canonical entries (sorted by normalized path, deduplicated), a
/// folder-set epoch bumped on every content change, and the identity of the
/// last applied event. `didChangeWorkspaceFolders` deltas apply to this set;
/// a full client list is a separately versioned confirmation step bound to
/// the folder-set epoch. An equivalent set in a different order is
/// byte-identical and produces no epoch bump and no transition.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(super) struct WorkspaceFolderSet {
    entries: Vec<WorkspaceFolderEntry>,
    folder_set_epoch: u64,
    event_sequence: u64,
    last_applied_event: Option<u64>,
}

fn parse_folder_entries(
    folders: &[WorkspaceFolder],
) -> Result<Vec<WorkspaceFolderEntry>, WorkspaceFolderEventRejection> {
    folders
        .iter()
        .map(|folder| {
            let Some(path) = path_from_file_uri(&folder.uri) else {
                return Err(WorkspaceFolderEventRejection {
                    kind: WorkspaceFolderEventRejectionKind::InvalidFileUri,
                    detail: format!(
                        "workspace folder URI is not a valid file URI: {}",
                        folder.uri.as_str()
                    ),
                });
            };
            Ok(WorkspaceFolderEntry {
                uri: folder.uri.as_str().to_string(),
                path,
            })
        })
        .collect()
}

fn canonical_folder_entries(
    folders: &[WorkspaceFolder],
) -> Result<Vec<WorkspaceFolderEntry>, WorkspaceFolderEventRejection> {
    let mut entries: Vec<WorkspaceFolderEntry> = Vec::new();
    for entry in parse_folder_entries(folders)? {
        if entries
            .iter()
            .any(|stored: &WorkspaceFolderEntry| stored.path == entry.path)
        {
            continue;
        }
        entries.push(entry);
    }
    entries.sort_by(|left, right| left.path.cmp(&right.path));
    Ok(entries)
}

impl WorkspaceFolderSet {
    /// Build a canonical set from a complete folder list (used at
    /// `initialize`): duplicates collapse and entries sort by normalized
    /// path so an equivalent list in any order yields the same set.
    pub(super) fn from_folder_list(
        folders: &[WorkspaceFolder],
    ) -> Result<Self, WorkspaceFolderEventRejection> {
        Ok(Self {
            entries: canonical_folder_entries(folders)?,
            ..Self::default()
        })
    }

    /// Replace the set from a complete folder list returned by the client
    /// reconciliation query. Returns whether the entries changed. Identity
    /// is the normalized path: an equivalent list (any order, duplicates,
    /// or a byte-different but path-equivalent URI spelling) is a no-op and
    /// the stored URI spelling is kept for display.
    pub(super) fn replace_from_folder_list(
        &mut self,
        folders: &[WorkspaceFolder],
    ) -> Result<bool, WorkspaceFolderEventRejection> {
        let entries = canonical_folder_entries(folders)?;
        if self.same_paths(&entries) {
            return Ok(false);
        }
        self.entries = entries;
        self.folder_set_epoch = self.folder_set_epoch.saturating_add(1);
        Ok(true)
    }

    /// Whether a complete folder list from the client reconciliation query
    /// is consistent with the stored set as a path set (order- and
    /// duplicate-insensitive, path identity only). An unparseable list is
    /// never consistent. Used to consistency-check a reconciliation answer
    /// against an accepted delta: a lagging contradictory answer must be
    /// dropped, never installed over the delta.
    pub(super) fn matches_folder_list_paths(&self, folders: &[WorkspaceFolder]) -> bool {
        let Ok(entries) = canonical_folder_entries(folders) else {
            return false;
        };
        self.same_paths(&entries)
    }

    /// Path-identity comparison against canonical (sorted, deduplicated)
    /// entries. The URI spelling is display-only and never compared.
    fn same_paths(&self, entries: &[WorkspaceFolderEntry]) -> bool {
        self.entries.len() == entries.len()
            && self
                .entries
                .iter()
                .zip(entries.iter())
                .all(|(stored, candidate)| stored.path == candidate.path)
    }

    /// Apply one `didChangeWorkspaceFolders` delta. Every entry is
    /// validated before any mutation, so a rejected event leaves the set
    /// untouched. The event sequence identity is recorded for every
    /// accepted event; the folder-set epoch advances only when the entries
    /// change.
    pub(super) fn apply_event(
        &mut self,
        added: &[WorkspaceFolder],
        removed: &[WorkspaceFolder],
    ) -> Result<WorkspaceFolderEventOutcome, WorkspaceFolderEventRejection> {
        self.event_sequence = self.event_sequence.saturating_add(1);
        let event_id = self.event_sequence;
        let added_entries = parse_folder_entries(added)?;
        let removed_entries = parse_folder_entries(removed)?;
        for (index, entry) in added_entries.iter().enumerate() {
            if added_entries[..index]
                .iter()
                .any(|earlier| earlier.path == entry.path)
            {
                return Err(WorkspaceFolderEventRejection {
                    kind: WorkspaceFolderEventRejectionKind::DuplicateAddition,
                    detail: format!("the same folder is added more than once: {}", entry.uri),
                });
            }
        }
        for entry in &added_entries {
            if removed_entries
                .iter()
                .any(|removed| removed.path == entry.path)
            {
                return Err(WorkspaceFolderEventRejection {
                    kind: WorkspaceFolderEventRejectionKind::ContradictoryEvent,
                    detail: format!(
                        "the same folder appears in both added and removed: {}",
                        entry.uri
                    ),
                });
            }
            if self.entries.iter().any(|stored| stored.path == entry.path) {
                return Err(WorkspaceFolderEventRejection {
                    kind: WorkspaceFolderEventRejectionKind::DuplicateAddition,
                    detail: format!(
                        "added folder is already in the workspace folder set: {}",
                        entry.uri
                    ),
                });
            }
        }
        for (index, entry) in removed_entries.iter().enumerate() {
            let known = self.entries.iter().any(|stored| stored.path == entry.path);
            let removed_twice = removed_entries[..index]
                .iter()
                .any(|earlier| earlier.path == entry.path);
            if !known || removed_twice {
                return Err(WorkspaceFolderEventRejection {
                    kind: WorkspaceFolderEventRejectionKind::UnknownRemoval,
                    detail: format!(
                        "removed folder is not in the workspace folder set: {}",
                        entry.uri
                    ),
                });
            }
        }
        self.last_applied_event = Some(event_id);
        let changed = !added_entries.is_empty() || !removed_entries.is_empty();
        if changed {
            self.entries.retain(|stored| {
                !removed_entries
                    .iter()
                    .any(|removed| removed.path == stored.path)
            });
            self.entries.extend(added_entries);
            self.entries
                .sort_by(|left, right| left.path.cmp(&right.path));
            self.folder_set_epoch = self.folder_set_epoch.saturating_add(1);
        }
        Ok(WorkspaceFolderEventOutcome {
            event_id,
            changed,
            folder_set_epoch: self.folder_set_epoch,
        })
    }

    pub(super) fn entries(&self) -> &[WorkspaceFolderEntry] {
        &self.entries
    }

    pub(super) fn selection(&self) -> WorkspaceFolderSelection {
        match self.entries.len() {
            0 => WorkspaceFolderSelection::NoFolders,
            1 => WorkspaceFolderSelection::SingleFolder,
            _ => WorkspaceFolderSelection::AmbiguousFolders,
        }
    }

    pub(super) fn folder_set_epoch(&self) -> u64 {
        self.folder_set_epoch
    }

    pub(super) fn last_applied_event(&self) -> Option<u64> {
        self.last_applied_event
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

/// Startup-window and lifecycle state of the server-originated
/// `workspace/configuration` pull (#2031, RIPR-SPEC-0136). Disclosed in the
/// analysis status payload so defaults never masquerade as accepted requested
/// settings: until the first pull resolves the state is `pending`, and a
/// failed or malformed pull is a typed state with a recovery route while the
/// last-known-good pulled layer is retained as stale.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum ConfigPullState {
    /// The negotiated configuration mode is not pull; no pull is attempted.
    NotApplicable,
    /// Pull mode is active but no single workspace root is selected, so the
    /// pull is deferred (analysis is already blocked in that state).
    Deferred,
    /// A pull has been requested and has not resolved yet.
    Pending,
    /// The last pull resolved and its validated settings were applied.
    Applied,
    /// The last pull failed at the transport level (`config_pull_failed`) or
    /// failed validation (`config_pull_invalid`); the retained pulled layer
    /// is last-known-good and stale.
    Failed(AnalysisFailure),
}

impl ConfigPullState {
    pub(super) fn as_str(&self) -> &'static str {
        match self {
            Self::NotApplicable => "not_applicable",
            Self::Deferred => "deferred",
            Self::Pending => "pending",
            Self::Applied => "applied",
            Self::Failed(_) => "failed",
        }
    }

    pub(super) fn failure(&self) -> Option<&AnalysisFailure> {
        match self {
            Self::Failed(failure) => Some(failure),
            _ => None,
        }
    }

    /// Recovery route disclosed next to a failed pull, mirroring the
    /// `root_recovery_route` style.
    pub(super) fn recovery_route(&self) -> Option<&'static str> {
        match self {
            Self::Failed(_) => Some("retry_via_did_change_configuration"),
            Self::Deferred => Some("select_single_workspace_root"),
            _ => None,
        }
    }
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
    /// Typed bounded per-component outcomes for this snapshot (#1997,
    /// RIPR-SPEC-0141): the single typed authority for optional-component
    /// degradation (seam inventory, gap ledger, causal projection, cache).
    /// The shared run status, `ripr/analysisStatus`, workspace status,
    /// progress ends, and the deduplicated `window/logMessage` degradation
    /// warning all derive from these records — no degradation is reported
    /// only through process stderr.
    pub(super) component_outcomes: Vec<ComponentOutcome>,
    /// Count of diff-analysis findings the projection dropped because their
    /// anchor is a Rust path outside the production scope (the shared
    /// `workspace::is_production_rust_path` classifier: `tests/`,
    /// `examples/`, `benches/`, `tests.rs`, and other non-production trees).
    /// The LSP scope must match the CLI review surface, which scopes to
    /// changed production files; an editor that pins line-local gap
    /// diagnostics in a test-only file inverts that signal. The findings are
    /// removed before snapshot construction so diagnostics, hover, actions,
    /// and status counts agree; this count is the disclosure that the
    /// suppression happened — it is never silent.
    pub(super) out_of_scope_test_file_findings: usize,
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

/// Stable content identity for saved workspace bytes. Only a digest is
/// retained: unsaved buffer text is hashed on demand for comparison and is
/// never stored outside the in-memory document store, so it cannot leak into
/// caches, artifacts, receipts, or producer evidence.
pub(super) fn content_digest(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    format!("{:x}", Sha256::digest(bytes))
}

/// Why a document's line-local diagnostics are currently withdrawn.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum DocumentStalenessReason {
    /// The open buffer diverges from the saved content the committed
    /// snapshot analyzed, so saved-state line identity no longer matches
    /// the client's buffer.
    BufferDivergesFromAnalyzedSavedContent,
    /// No refresh has analyzed this document's saved content in the current
    /// session, so there is no analyzed baseline to serve against.
    NoAnalyzedSavedContent,
}

impl DocumentStalenessReason {
    pub(super) fn as_str(self) -> &'static str {
        match self {
            Self::BufferDivergesFromAnalyzedSavedContent => {
                "buffer_diverges_from_analyzed_saved_content"
            }
            Self::NoAnalyzedSavedContent => "no_analyzed_saved_content",
        }
    }

    pub(super) fn description(self) -> &'static str {
        match self {
            Self::BufferDivergesFromAnalyzedSavedContent => {
                "the open buffer diverges from the last analyzed saved content"
            }
            Self::NoAnalyzedSavedContent => {
                "the document's saved content has not been analyzed in this session"
            }
        }
    }
}

/// Quarantine marker for one open document. While present, line-local
/// diagnostics for the document are withdrawn because only the saved
/// workspace state is a valid diagnostics authority and the buffer no
/// longer matches the analyzed saved state.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct DocumentQuarantine {
    pub(super) reason: DocumentStalenessReason,
    /// True once the withdrawal has been disclosed to the client, so one
    /// quarantine episode emits at most one disclosure.
    pub(super) withdrawal_disclosed: bool,
}

/// Quarantine edges produced by applying a committed snapshot's analyzed
/// identities (#1970): documents that entered quarantine, and documents
/// that exited it with each exit carrying its disclosed marker.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(super) struct QuarantineEdges {
    pub(super) entered: Vec<Uri>,
    pub(super) exited: Vec<(Uri, bool)>,
}

/// Quarantine edge produced by recomputing a document's dirty state.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum QuarantineTransition {
    Entered,
    Exited {
        /// Whether the ended episode had disclosed its withdrawal; the
        /// caller restores only what it disclosed.
        was_disclosed: bool,
    },
    Unchanged,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct DocumentState {
    pub(super) uri: Uri,
    pub(super) path: PathBuf,
    pub(super) version: Option<i32>,
    pub(super) text: String,
    /// SHA-256 of the last known saved content: seeded from persisted bytes
    /// at open (never from the didOpen text, which may carry unsaved or
    /// recovered buffer content) and updated from the didSave digest (#2129).
    /// `None` when the saved bytes could not be established.
    pub(super) saved_digest: Option<String>,
    /// SHA-256 of the saved content the committed snapshot analyzed for
    /// this document, recorded only when that snapshot commits (#1970).
    pub(super) analyzed_saved_digest: Option<String>,
    /// Input identity of the snapshot that last analyzed this document.
    pub(super) analyzed_input_identity: Option<String>,
    pub(super) quarantine: Option<DocumentQuarantine>,
}

impl DocumentState {
    fn new(uri: Uri, version: Option<i32>, text: String) -> Self {
        let path = document_path(&uri);
        // Saved-workspace authority: the saved-content identity comes from
        // the persisted bytes, not the client-sent text (#2129 rationale).
        let saved_digest = std::fs::read(&path)
            .ok()
            .map(|bytes| content_digest(&bytes));
        Self {
            uri,
            path,
            version,
            text,
            saved_digest,
            analyzed_saved_digest: None,
            analyzed_input_identity: None,
            quarantine: None,
        }
    }

    fn buffer_digest(&self) -> String {
        content_digest(self.text.as_bytes())
    }

    pub(super) fn is_quarantined(&self) -> bool {
        self.quarantine.is_some()
    }

    /// The staleness of the current buffer against one candidate analyzed
    /// saved-content identity: `None` when the buffer matches it (clean),
    /// otherwise the reason the document must be quarantined. Used both for
    /// the committed identity and for a refresh transaction's pending
    /// identity before that transaction commits (#1970).
    pub(super) fn staleness_for_analyzed(
        &self,
        analyzed: Option<&String>,
    ) -> Option<DocumentStalenessReason> {
        match analyzed {
            None => Some(DocumentStalenessReason::NoAnalyzedSavedContent),
            Some(analyzed) if *analyzed != self.buffer_digest() => {
                Some(DocumentStalenessReason::BufferDivergesFromAnalyzedSavedContent)
            }
            Some(_) => None,
        }
    }

    /// Recompute the quarantine state from the current buffer and the
    /// analyzed/saved content identities. A document is quarantined while
    /// its buffer digest differs from the analyzed saved digest.
    pub(super) fn refresh_quarantine(&mut self) -> QuarantineTransition {
        let mut next = self
            .staleness_for_analyzed(self.analyzed_saved_digest.as_ref())
            .map(|reason| DocumentQuarantine {
                reason,
                withdrawal_disclosed: false,
            });
        let was_disclosed = self
            .quarantine
            .as_ref()
            .is_some_and(|quarantine| quarantine.withdrawal_disclosed);
        // Staying quarantined is one episode even when the reason changes;
        // keep the disclosed marker so the withdrawal is disclosed at most
        // once per episode.
        if let (Some(_), Some(next_quarantine)) = (&self.quarantine, &mut next) {
            next_quarantine.withdrawal_disclosed = was_disclosed;
        }
        let was = self.quarantine.is_some();
        self.quarantine = next;
        match (was, self.quarantine.is_some()) {
            (false, true) => QuarantineTransition::Entered,
            (true, false) => QuarantineTransition::Exited { was_disclosed },
            _ => QuarantineTransition::Unchanged,
        }
    }
}

#[derive(Default)]
pub(super) struct DocumentStore {
    pub(super) documents: BTreeMap<Uri, DocumentState>,
}

impl DocumentStore {
    pub(super) fn open(&mut self, params: DidOpenTextDocumentParams) -> QuarantineTransition {
        let uri = params.text_document.uri;
        let mut state = DocumentState::new(
            uri.clone(),
            Some(params.text_document.version),
            params.text_document.text,
        );
        let transition = state.refresh_quarantine();
        self.documents.insert(uri, state);
        transition
    }

    pub(super) fn change(&mut self, params: DidChangeTextDocumentParams) -> QuarantineTransition {
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
            return state.refresh_quarantine();
        }
        let Some(text) = text else {
            return QuarantineTransition::Unchanged;
        };
        let mut state = DocumentState::new(uri.clone(), version, text);
        let transition = state.refresh_quarantine();
        self.documents.insert(uri, state);
        transition
    }

    /// Record a save: the didSave digest is the new saved-content identity
    /// (#2129), and the buffer now holds the persisted text when the client
    /// included it. The quarantine recomputation may lift the withdrawal
    /// when the buffer matches the analyzed saved content again.
    pub(super) fn save(
        &mut self,
        uri: &Uri,
        saved_digest: Option<String>,
        text: Option<String>,
    ) -> QuarantineTransition {
        let Some(state) = self.documents.get_mut(uri) else {
            return QuarantineTransition::Unchanged;
        };
        if let Some(digest) = saved_digest {
            state.saved_digest = Some(digest);
        }
        if let Some(text) = text {
            state.text = text;
        }
        state.refresh_quarantine()
    }

    /// Compute the analyzed saved-content identity a refresh transaction
    /// would record for every open document, without mutating any state
    /// (#1970). The identity comes from the persisted bytes on disk — the
    /// same bytes the refresh's analysis read — not from the didSave-tracked
    /// digest, so a file changed on disk outside didSave (git checkout,
    /// formatter, external editor) cannot be recorded as analyzed against
    /// its stale pre-change digest. Falls back to the didSave-tracked digest
    /// only when the persisted bytes cannot be read. Also returns the URIs
    /// that are currently clean but would enter quarantine under the pending
    /// identity, so publication can withdraw them before commit.
    pub(super) fn pending_analyzed_digests(&self) -> (BTreeMap<Uri, Option<String>>, Vec<Uri>) {
        let mut digests = BTreeMap::new();
        let mut entered = Vec::new();
        for (uri, state) in &self.documents {
            let analyzed = std::fs::read(&state.path)
                .ok()
                .map(|bytes| content_digest(&bytes))
                .or_else(|| state.saved_digest.clone());
            if !state.is_quarantined() && state.staleness_for_analyzed(analyzed.as_ref()).is_some()
            {
                entered.push(uri.clone());
            }
            digests.insert(uri.clone(), analyzed);
        }
        (digests, entered)
    }

    /// Record that the committed refresh snapshot analyzed the given saved
    /// content of every open document (#1970). Called only from the snapshot
    /// commit path, so document identities advance exclusively with the
    /// committed snapshot — a superseded or failed transaction leaves them
    /// untouched. `analyzed` carries the pending identities computed at
    /// prepare time from the persisted bytes (see
    /// `pending_analyzed_digests`); `pre_disclosed` lists URIs whose pending
    /// withdrawal was already disclosed during publication, so the new
    /// episode does not disclose a second time.
    pub(super) fn note_refresh_analyzed(
        &mut self,
        input_identity: Option<String>,
        analyzed: &BTreeMap<Uri, Option<String>>,
        pre_disclosed: &[Uri],
    ) -> QuarantineEdges {
        let mut edges = QuarantineEdges::default();
        for (uri, state) in &mut self.documents {
            if let Some(digest) = analyzed.get(uri) {
                state.analyzed_saved_digest = digest.clone();
            }
            state.analyzed_input_identity = input_identity.clone();
            match state.refresh_quarantine() {
                QuarantineTransition::Entered => {
                    if pre_disclosed.iter().any(|disclosed| disclosed == uri)
                        && let Some(quarantine) = state.quarantine.as_mut()
                    {
                        quarantine.withdrawal_disclosed = true;
                    }
                    edges.entered.push(uri.clone());
                }
                QuarantineTransition::Exited { was_disclosed } => {
                    edges.exited.push((uri.clone(), was_disclosed));
                }
                QuarantineTransition::Unchanged => {}
            }
        }
        edges
    }

    pub(super) fn close(&mut self, params: DidCloseTextDocumentParams) {
        self.documents.remove(&params.text_document.uri);
    }

    /// Look up a document by exact URI, tolerating spelling differences the
    /// same way the snapshot diagnostic lookups do.
    pub(super) fn state_for_uri(&self, uri: &Uri) -> Option<&DocumentState> {
        self.documents.get(uri).or_else(|| {
            self.documents
                .iter()
                .find(|(stored_uri, _)| file_uris_match(stored_uri, uri))
                .map(|(_, state)| state)
        })
    }

    pub(super) fn state_for_uri_mut(&mut self, uri: &Uri) -> Option<&mut DocumentState> {
        if self.documents.contains_key(uri) {
            return self.documents.get_mut(uri);
        }
        let stored_uri = self
            .documents
            .keys()
            .find(|stored_uri| file_uris_match(stored_uri, uri))
            .cloned()?;
        self.documents.get_mut(&stored_uri)
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

    fn digest_of(text: &str) -> String {
        content_digest(text.as_bytes())
    }

    fn clean_document_state(uri: &Uri, saved_text: &str) -> DocumentState {
        DocumentState {
            uri: uri.clone(),
            path: PathBuf::from("/workspace/src/lib.rs"),
            version: Some(1),
            text: saved_text.to_string(),
            saved_digest: Some(digest_of(saved_text)),
            analyzed_saved_digest: Some(digest_of(saved_text)),
            analyzed_input_identity: Some("input:test".to_string()),
            quarantine: None,
        }
    }

    #[test]
    fn quarantine_follows_buffer_against_analyzed_saved_content() -> Result<(), String> {
        let uri = test_uri("file:///workspace/src/lib.rs")?;
        let mut state = clean_document_state(&uri, "fn saved() {}");
        if state.refresh_quarantine() != QuarantineTransition::Unchanged {
            return Err("clean buffer must not enter quarantine".to_string());
        }
        state.text = "fn dirty() {}".to_string();
        if state.refresh_quarantine() != QuarantineTransition::Entered {
            return Err("diverging buffer must enter quarantine".to_string());
        }
        let Some(quarantine) = state.quarantine.as_mut() else {
            return Err("expected quarantine marker".to_string());
        };
        if quarantine.reason != DocumentStalenessReason::BufferDivergesFromAnalyzedSavedContent {
            return Err("wrong staleness reason for a diverging buffer".to_string());
        }
        quarantine.withdrawal_disclosed = true;
        // Further edits keep one episode and its disclosed marker.
        state.text = "fn dirtier() {}".to_string();
        if state.refresh_quarantine() != QuarantineTransition::Unchanged {
            return Err("still-dirty buffer must stay in the same episode".to_string());
        }
        if !state
            .quarantine
            .as_ref()
            .is_some_and(|quarantine| quarantine.withdrawal_disclosed)
        {
            return Err("disclosed marker must survive within an episode".to_string());
        }
        // Returning to the analyzed saved content lifts the quarantine.
        state.text = "fn saved() {}".to_string();
        if state.refresh_quarantine()
            != (QuarantineTransition::Exited {
                was_disclosed: true,
            })
        {
            return Err("matching buffer must lift the quarantine".to_string());
        }
        if state.is_quarantined() {
            return Err("quarantine marker must be cleared on lift".to_string());
        }
        Ok(())
    }

    #[test]
    fn quarantine_without_analyzed_saved_content_names_missing_baseline() -> Result<(), String> {
        let uri = test_uri("file:///workspace/src/lib.rs")?;
        let mut state = clean_document_state(&uri, "fn saved() {}");
        state.analyzed_saved_digest = None;
        if state.refresh_quarantine() != QuarantineTransition::Entered {
            return Err("a missing analyzed baseline must quarantine".to_string());
        }
        let Some(quarantine) = &state.quarantine else {
            return Err("expected quarantine marker".to_string());
        };
        if quarantine.reason != DocumentStalenessReason::NoAnalyzedSavedContent {
            return Err("expected the no_analyzed_saved_content reason".to_string());
        }
        Ok(())
    }

    #[test]
    fn save_lifts_quarantine_when_buffer_matches_analyzed_saved_content() -> Result<(), String> {
        let uri = test_uri("file:///workspace/src/lib.rs")?;
        let mut store = DocumentStore::default();
        store
            .documents
            .insert(uri.clone(), clean_document_state(&uri, "fn saved() {}"));
        let transition = store.save(
            &uri,
            Some(digest_of("fn dirty() {}")),
            Some("fn dirty() {}".to_string()),
        );
        if transition != QuarantineTransition::Entered {
            return Err("saving divergent text must enter quarantine".to_string());
        }
        // Dedup-style save: the recorded saved digest is unchanged and the
        // buffer again matches the analyzed saved content, so the quarantine
        // lifts even though no refresh will run.
        let transition = store.save(
            &uri,
            Some(digest_of("fn saved() {}")),
            Some("fn saved() {}".to_string()),
        );
        if transition
            != (QuarantineTransition::Exited {
                was_disclosed: false,
            })
        {
            return Err("a buffer matching the analyzed saved content must lift".to_string());
        }
        Ok(())
    }

    #[test]
    fn note_refresh_analyzed_records_identities_and_edges() -> Result<(), String> {
        let clean_uri = test_uri("file:///workspace/src/clean.rs")?;
        let dirty_uri = test_uri("file:///workspace/src/dirty.rs")?;
        let lifted_uri = test_uri("file:///workspace/src/lifted.rs")?;
        let mut store = DocumentStore::default();
        let mut clean = clean_document_state(&clean_uri, "fn clean() {}");
        clean.analyzed_saved_digest = None;
        let mut dirty = clean_document_state(&dirty_uri, "fn saved() {}");
        dirty.text = "fn dirty() {}".to_string();
        let mut lifted = clean_document_state(&lifted_uri, "fn saved() {}");
        lifted.quarantine = Some(DocumentQuarantine {
            reason: DocumentStalenessReason::BufferDivergesFromAnalyzedSavedContent,
            withdrawal_disclosed: true,
        });
        store.documents.insert(clean_uri.clone(), clean);
        store.documents.insert(dirty_uri.clone(), dirty);
        store.documents.insert(lifted_uri.clone(), lifted);

        let analyzed = store
            .documents
            .iter()
            .map(|(uri, state)| (uri.clone(), state.saved_digest.clone()))
            .collect::<BTreeMap<_, _>>();
        let edges = store.note_refresh_analyzed(Some("input:42".to_string()), &analyzed, &[]);
        if edges.entered != vec![dirty_uri.clone()] {
            return Err(format!(
                "expected only the dirty document to enter quarantine: {:?}",
                edges.entered
            ));
        }
        if edges.exited != vec![(lifted_uri.clone(), true)] {
            return Err(format!(
                "expected the lifted document to exit with its disclosed marker: {:?}",
                edges.exited
            ));
        }
        let Some(dirty_state) = store.state_for_uri(&dirty_uri) else {
            return Err("missing dirty document state".to_string());
        };
        if dirty_state.analyzed_saved_digest != dirty_state.saved_digest {
            return Err("analyzed identity must track the saved identity".to_string());
        }
        if dirty_state.analyzed_input_identity.as_deref() != Some("input:42") {
            return Err("analyzed input identity must be recorded".to_string());
        }
        if store
            .state_for_uri(&lifted_uri)
            .is_some_and(|state| state.is_quarantined())
        {
            return Err("lifted document must be clean".to_string());
        }
        Ok(())
    }

    #[test]
    fn note_refresh_analyzed_marks_pre_disclosed_entries() -> Result<(), String> {
        let uri = test_uri("file:///workspace/src/dirty.rs")?;
        let mut store = DocumentStore::default();
        let mut state = clean_document_state(&uri, "fn saved() {}");
        state.analyzed_saved_digest = None;
        state.quarantine = None;
        state.text = "fn dirty() {}".to_string();
        let analyzed_digest = state.saved_digest.clone();
        store.documents.insert(uri.clone(), state);
        let analyzed = BTreeMap::from([(uri.clone(), analyzed_digest)]);

        // The pending withdrawal was disclosed during publication, so the
        // episode created at commit must not disclose a second time.
        let edges = store.note_refresh_analyzed(None, &analyzed, std::slice::from_ref(&uri));
        if edges.entered != vec![uri.clone()] {
            return Err(format!(
                "expected the document to enter quarantine: {:?}",
                edges.entered
            ));
        }
        let Some(state) = store.state_for_uri(&uri) else {
            return Err("missing document state".to_string());
        };
        if !state
            .quarantine
            .as_ref()
            .is_some_and(|quarantine| quarantine.withdrawal_disclosed)
        {
            return Err("pre-disclosed episode must keep the disclosed marker".to_string());
        }
        Ok(())
    }

    #[test]
    fn open_seeds_saved_identity_from_persisted_bytes() -> Result<(), String> {
        let stamp = SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|duration| duration.as_nanos())
            .unwrap_or(0);
        let dir = std::env::temp_dir().join(format!(
            "ripr-state-open-seed-{}-{stamp}",
            std::process::id()
        ));
        std::fs::create_dir_all(&dir).map_err(|err| format!("create temp dir failed: {err}"))?;
        let path = dir.join("lib.rs");
        std::fs::write(&path, "fn on_disk() {}").map_err(|err| format!("write failed: {err}"))?;
        let uri = crate::lsp::uri::file_uri_for_path(&path)
            .map_err(|err| format!("file URI failed: {err}"))?;
        let mut store = DocumentStore::default();
        let transition = store.open(DidOpenTextDocumentParams {
            text_document: tower_lsp_server::ls_types::TextDocumentItem::new(
                uri.clone(),
                "rust".to_string(),
                1,
                "fn unsaved_buffer() {}".to_string(),
            ),
        });
        let result = (|| {
            if transition != QuarantineTransition::Entered {
                return Err("a divergent fresh buffer must enter quarantine".to_string());
            }
            let Some(state) = store.documents.get(&uri) else {
                return Err("missing opened document".to_string());
            };
            if state.saved_digest.as_deref() != Some(digest_of("fn on_disk() {}").as_str()) {
                return Err(
                    "saved identity must come from persisted bytes, not the didOpen text"
                        .to_string(),
                );
            }
            if state.saved_digest.as_deref() == Some(digest_of("fn unsaved_buffer() {}").as_str()) {
                return Err("saved identity must not be seeded from the didOpen text".to_string());
            }
            Ok(())
        })();
        let _ = std::fs::remove_dir_all(&dir);
        result
    }

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
            component_outcomes: Vec::new(),
            out_of_scope_test_file_findings: 0,
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
            component_outcomes: Vec::new(),
            out_of_scope_test_file_findings: 0,
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

    fn folder(uri: &str) -> Result<WorkspaceFolder, String> {
        Ok(WorkspaceFolder {
            uri: test_uri(uri)?,
            name: "test".to_string(),
        })
    }

    fn entry_paths(set: &WorkspaceFolderSet) -> Vec<PathBuf> {
        set.entries()
            .iter()
            .map(|entry| entry.path.clone())
            .collect()
    }

    #[test]
    fn workspace_folder_set_canonicalizes_order_and_duplicates() -> Result<(), String> {
        let first = WorkspaceFolderSet::from_folder_list(&[
            folder("file:///workspace/b")?,
            folder("file:///workspace/a")?,
            folder("file:///workspace/b")?,
        ])
        .map_err(|rejection| format!("unexpected rejection: {rejection:?}"))?;
        let second = WorkspaceFolderSet::from_folder_list(&[
            folder("file:///workspace/a")?,
            folder("file:///workspace/b")?,
        ])
        .map_err(|rejection| format!("unexpected rejection: {rejection:?}"))?;
        if first != second {
            return Err(
                "an equivalent folder list in any order must canonicalize equal".to_string(),
            );
        }
        if entry_paths(&first) != vec![PathBuf::from("/workspace/a"), PathBuf::from("/workspace/b")]
        {
            return Err("entries must sort by normalized path".to_string());
        }
        if first.selection() != WorkspaceFolderSelection::AmbiguousFolders {
            return Err("two folders must read as ambiguous".to_string());
        }
        let empty = WorkspaceFolderSet::default();
        if empty.selection() != WorkspaceFolderSelection::NoFolders {
            return Err("an empty set must read as no_folders".to_string());
        }
        Ok(())
    }

    #[test]
    fn workspace_folder_set_rejects_invalid_file_uri() -> Result<(), String> {
        let rejection = WorkspaceFolderSet::from_folder_list(&[folder("https://example.test/x")?])
            .err()
            .ok_or_else(|| "a non-file URI must be rejected".to_string())?;
        if rejection.kind != WorkspaceFolderEventRejectionKind::InvalidFileUri {
            return Err("expected the invalid_file_uri rejection kind".to_string());
        }
        if rejection.kind.as_str() != "invalid_file_uri" {
            return Err("unexpected rejection kind string".to_string());
        }
        Ok(())
    }

    #[test]
    fn workspace_folder_set_apply_event_validates_before_mutating() -> Result<(), String> {
        let mut set = WorkspaceFolderSet::from_folder_list(&[folder("file:///workspace/a")?])
            .map_err(|rejection| format!("unexpected rejection: {rejection:?}"))?;
        let epoch = set.folder_set_epoch();

        // Duplicate addition: already stored.
        let rejection = set
            .apply_event(&[folder("file:///workspace/a")?], &[])
            .err()
            .ok_or_else(|| "adding a stored folder must be rejected".to_string())?;
        if rejection.kind != WorkspaceFolderEventRejectionKind::DuplicateAddition {
            return Err("expected duplicate_addition for a stored folder".to_string());
        }
        // Duplicate addition within one event.
        let rejection = set
            .apply_event(
                &[
                    folder("file:///workspace/b")?,
                    folder("file:///workspace/b")?,
                ],
                &[],
            )
            .err()
            .ok_or_else(|| "adding the same folder twice must be rejected".to_string())?;
        if rejection.kind != WorkspaceFolderEventRejectionKind::DuplicateAddition {
            return Err("expected duplicate_addition within one event".to_string());
        }
        // Contradictory event.
        let rejection = set
            .apply_event(
                &[folder("file:///workspace/b")?],
                &[folder("file:///workspace/b")?],
            )
            .err()
            .ok_or_else(|| "add+remove of one folder must be rejected".to_string())?;
        if rejection.kind != WorkspaceFolderEventRejectionKind::ContradictoryEvent {
            return Err("expected contradictory_event".to_string());
        }
        // Removal of an unknown folder.
        let rejection = set
            .apply_event(&[], &[folder("file:///workspace/b")?])
            .err()
            .ok_or_else(|| "removing an unknown folder must be rejected".to_string())?;
        if rejection.kind != WorkspaceFolderEventRejectionKind::UnknownRemoval {
            return Err("expected unknown_removal".to_string());
        }
        // Every rejection left the set untouched: entries, epoch, and the
        // last applied event identity are unchanged.
        if set.entries().len() != 1
            || set.folder_set_epoch() != epoch
            || set.last_applied_event().is_some()
        {
            return Err("a rejected event must not mutate the stored set".to_string());
        }
        Ok(())
    }

    #[test]
    fn workspace_folder_set_apply_event_tracks_epoch_and_event_identity() -> Result<(), String> {
        let mut set = WorkspaceFolderSet::default();
        // First folder after none: one content change, one applied event.
        let outcome = set
            .apply_event(&[folder("file:///workspace/a")?], &[])
            .map_err(|rejection| format!("unexpected rejection: {rejection:?}"))?;
        if !outcome.changed || outcome.folder_set_epoch != 1 || outcome.event_id != 1 {
            return Err("the first addition must change the set exactly once".to_string());
        }
        if set.selection() != WorkspaceFolderSelection::SingleFolder
            || set.last_applied_event() != Some(1)
        {
            return Err("one stored folder must select as single_folder".to_string());
        }
        // An empty delta is accepted but changes nothing and bumps no epoch.
        let outcome = set
            .apply_event(&[], &[])
            .map_err(|rejection| format!("unexpected rejection: {rejection:?}"))?;
        if outcome.changed || outcome.folder_set_epoch != 1 || outcome.event_id != 2 {
            return Err("an empty delta must be a recorded no-op".to_string());
        }
        if set.last_applied_event() != Some(2) {
            return Err("an accepted no-op event still advances the event identity".to_string());
        }
        // A direct switch in one event replaces the entries canonically.
        let outcome = set
            .apply_event(
                &[folder("file:///workspace/b")?],
                &[folder("file:///workspace/a")?],
            )
            .map_err(|rejection| format!("unexpected rejection: {rejection:?}"))?;
        if !outcome.changed
            || outcome.folder_set_epoch != 2
            || entry_paths(&set) != vec![PathBuf::from("/workspace/b")]
        {
            return Err("a one-event switch must replace the stored entries".to_string());
        }
        // A full-list reconciliation with an equivalent set is a no-op.
        let replaced = set
            .replace_from_folder_list(&[folder("file:///workspace/b")?])
            .map_err(|rejection| format!("unexpected rejection: {rejection:?}"))?;
        if replaced || set.folder_set_epoch() != 2 {
            return Err("an equivalent reconciliation must not bump the epoch".to_string());
        }
        // A different reconciliation list replaces and bumps.
        let replaced = set
            .replace_from_folder_list(&[
                folder("file:///workspace/c")?,
                folder("file:///workspace/b")?,
            ])
            .map_err(|rejection| format!("unexpected rejection: {rejection:?}"))?;
        if !replaced
            || set.folder_set_epoch() != 3
            || entry_paths(&set)
                != vec![PathBuf::from("/workspace/b"), PathBuf::from("/workspace/c")]
        {
            return Err("a changed reconciliation must replace canonically".to_string());
        }
        Ok(())
    }

    #[test]
    fn workspace_folder_set_reconciliation_with_equivalent_uri_spelling_is_noop()
    -> Result<(), String> {
        // Identity is the normalized path, not the URI spelling
        // (RIPR-SPEC-0139): a reconciliation answer that re-sends the same
        // folder with a byte-different but path-equivalent URI spelling
        // (here: percent-encoded characters) is a no-op — no epoch bump, no
        // entry change, and the stored spelling is kept for display.
        let mut set = WorkspaceFolderSet::from_folder_list(&[folder("file:///workspace/ab")?])
            .map_err(|rejection| format!("unexpected rejection: {rejection:?}"))?;
        if set
            .apply_event(&[folder("file:///workspace/cd")?], &[])
            .map_err(|rejection| format!("unexpected rejection: {rejection:?}"))?
            .folder_set_epoch
            != 1
        {
            return Err("setup: the addition must bump the epoch once".to_string());
        }
        let replaced = set
            .replace_from_folder_list(&[
                folder("file:///workspace/%61%62")?,
                folder("file:///workspace/%63d")?,
            ])
            .map_err(|rejection| format!("unexpected rejection: {rejection:?}"))?;
        if replaced || set.folder_set_epoch() != 1 {
            return Err("a path-equivalent respelling must not bump the epoch".to_string());
        }
        if entry_paths(&set)
            != vec![
                PathBuf::from("/workspace/ab"),
                PathBuf::from("/workspace/cd"),
            ]
        {
            return Err("a path-equivalent respelling must not change entries".to_string());
        }
        if set.entries().iter().any(|entry| entry.uri.contains('%')) {
            return Err("the stored URI spelling must be kept, not rewritten".to_string());
        }
        // The consistency check agrees: both spellings match the stored set.
        if !set.matches_folder_list_paths(&[
            folder("file:///workspace/%61%62")?,
            folder("file:///workspace/cd")?,
        ]) {
            return Err("path-equivalent spellings must be consistent".to_string());
        }
        if set.matches_folder_list_paths(&[folder("file:///workspace/ab")?]) {
            return Err("a smaller list must not be consistent".to_string());
        }
        if set.matches_folder_list_paths(&[folder("https://example.test/ab")?]) {
            return Err("an unparseable list must not be consistent".to_string());
        }
        Ok(())
    }
}
