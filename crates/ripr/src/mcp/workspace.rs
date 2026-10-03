//! MCP workspace session: the bounded analysis lifecycle adapter.
//!
//! The session is the MCP-side owner of "which snapshot is current". It does
//! not analyze by itself: [`run_check`] delegates to the shared
//! [`crate::app::check_workspace`] authority, and bounded selection delegates
//! to the shared [`crate::lsp::diagnostic_budget`] evaluator. The session only
//! records attempt state, keeps the last completed (last-known-good) snapshot,
//! and projects bounded, progressively disclosed documents for the tools and
//! resources.
//!
//! Read-only boundary (ADR 0022): analysis here is the same static analysis
//! the CLI and LSP run in-process. The session never edits source, never
//! executes verification or mutation commands, never loads project-local
//! provider configuration, and never reports a repair-ready state.

use super::gaps::{self, GAP_LIST_SCHEMA_VERSION, GapItem};
use crate::analysis_outcome::{AnalysisOutcome, AnalysisOutcomeKind};
use crate::lsp::diagnostic_budget::{
    self, DiagnosticBudget, DiagnosticBudgetResult, DiagnosticOverflowReason,
    OmittedDiagnosticReason,
};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::Mutex;

pub(crate) const SESSION_SCHEMA_VERSION: &str = "ripr-mcp-session-v1";
pub(crate) const REFRESH_SCHEMA_VERSION: &str = "ripr-mcp-refresh-v1";
pub(crate) const SNAPSHOT_SCHEMA_VERSION: &str = "ripr-mcp-snapshot-v1";

pub(crate) const CODE_WORKSPACE_UNAVAILABLE: &str = "workspace_unavailable";
pub(crate) const CODE_ANALYSIS_FAILED: &str = "analysis_failed";
pub(crate) const CODE_UNSUPPORTED_PROFILE: &str = "unsupported_profile";
pub(crate) const CODE_NO_SNAPSHOT: &str = "no_snapshot";
pub(crate) const CODE_ANALYSIS_IN_FLIGHT: &str = "analysis_in_flight";
pub(crate) const CODE_STALE_SNAPSHOT: &str = "stale_snapshot";
pub(crate) const CODE_ITEM_NOT_FOUND: &str = "item_not_found";
pub(crate) const CODE_RESULT_TOO_LARGE: &str = "result_too_large";
/// Reserved typed-failure vocabulary for later slices. This slice fails
/// closed before any of these states can occur; they are named so the wire
/// contract stays stable when the owning slice lands.
pub(crate) const RESERVED_FAILURE_CODES: &[&str] = &[
    "config_invalid",
    "workspace_ambiguous",
    "static_limitation",
    "cancelled",
    "superseded",
];

const MAX_FAILURE_DETAIL_CHARS: usize = 512;

/// One typed failure. `detail` is producer wording bounded to
/// [`MAX_FAILURE_DETAIL_CHARS`]; `recovery` names the action that can change
/// the state; `data` carries small structured context (for example the
/// current snapshot identity on a stale request).
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct AttemptFailure {
    pub(crate) code: &'static str,
    pub(crate) detail: String,
    pub(crate) recovery: &'static str,
    pub(crate) data: Value,
}

impl AttemptFailure {
    pub(crate) fn new(
        code: &'static str,
        detail: impl Into<String>,
        recovery: &'static str,
    ) -> Self {
        Self {
            code,
            detail: bounded_detail(detail),
            recovery,
            data: json!({}),
        }
    }

    pub(crate) fn with_data(mut self, data: Value) -> Self {
        self.data = data;
        self
    }

    /// The structured failure block shared by tool results and documents.
    pub(crate) fn value(&self) -> Value {
        json!({
            "code": self.code,
            "detail": self.detail,
            "recovery": self.recovery,
            "data": self.data,
        })
    }

    pub(crate) fn document(&self, schema_version: &str) -> Value {
        json!({
            "schema_version": schema_version,
            "failure": self.value(),
        })
    }
}

fn bounded_detail(detail: impl Into<String>) -> String {
    let detail = detail.into();
    let mut chars = detail.chars();
    let bounded: String = chars.by_ref().take(MAX_FAILURE_DETAIL_CHARS).collect();
    if chars.next().is_some() {
        return format!("{bounded}…");
    }
    bounded
}

/// The completed-snapshot authority for one MCP session. Immutable once
/// committed: refresh replaces the whole snapshot, never mutates it.
pub(crate) struct Snapshot {
    pub(crate) snapshot_id: String,
    pub(crate) outcome: AnalysisOutcome,
    pub(crate) items: Vec<GapItem>,
    pub(crate) budget: DiagnosticBudget,
    pub(crate) selection: DiagnosticBudgetResult,
}

impl Snapshot {
    /// Bind one completed analysis output into a snapshot. Closed incomplete
    /// outcomes (`unsupported_input`, `analysis_failed`) stay failures and
    /// never become snapshots; partial outcomes commit with their typed
    /// limitations so complete-zero and incomplete-zero stay distinct.
    pub(crate) fn from_output(
        output: &crate::app::CheckOutput,
        root_identity: Option<&str>,
    ) -> Result<Self, AttemptFailure> {
        let outcome = output
            .analysis_outcome
            .as_ref()
            .ok_or_else(|| {
                AttemptFailure::new(
                    CODE_ANALYSIS_FAILED,
                    "the producer returned no typed analysis outcome",
                    "retry with ripr_refresh; if the failure persists, run `ripr check --format json` in the repository",
                )
            })?
            .clone();
        match outcome.kind {
            AnalysisOutcomeKind::UnsupportedInput => {
                return Err(AttemptFailure::new(
                    CODE_UNSUPPORTED_PROFILE,
                    format!(
                        "the workspace input is unsupported: {}",
                        limitation_summary(&outcome)
                    ),
                    "narrow the diff (for example `ripr check --base <revision> --format json`) and retry ripr_refresh",
                ));
            }
            AnalysisOutcomeKind::AnalysisFailed => {
                return Err(AttemptFailure::new(
                    CODE_ANALYSIS_FAILED,
                    format!("the analysis failed: {}", limitation_summary(&outcome)),
                    "retry with ripr_refresh; if the failure persists, run `ripr check --format json` in the repository",
                ));
            }
            AnalysisOutcomeKind::NoScope
            | AnalysisOutcomeKind::NoChangedLines
            | AnalysisOutcomeKind::NoBehavioralCandidates
            | AnalysisOutcomeKind::CompleteNoFindings
            | AnalysisOutcomeKind::CompleteWithFindings
            | AnalysisOutcomeKind::PartialWithLimitations => {}
        }

        let mut items = output
            .findings
            .iter()
            .map(GapItem::from_finding)
            .collect::<Result<Vec<_>, String>>()
            .map_err(|error| {
                AttemptFailure::new(
                    CODE_ANALYSIS_FAILED,
                    format!("project canonical items: {error}"),
                    "retry with ripr_refresh",
                )
            })?;
        items.sort_by(|left, right| left.canonical_id.cmp(&right.canonical_id));

        let snapshot_id = snapshot_identity(&outcome, &items).map_err(|error| {
            AttemptFailure::new(CODE_ANALYSIS_FAILED, error, "retry with ripr_refresh")
        })?;
        let profile_identity = format!(
            "mcp:{}:profile:diff:draft",
            root_identity.unwrap_or("root:unavailable")
        );
        let evidence_identity = evidence_identity(&items).map_err(|error| {
            AttemptFailure::new(CODE_ANALYSIS_FAILED, error, "retry with ripr_refresh")
        })?;
        let budget = DiagnosticBudget::default();
        let selection = diagnostic_budget::evaluate_diagnostic_budget(
            gaps::budget_items(&items),
            &budget,
            &profile_identity,
            &evidence_identity,
        )
        .map_err(|error| {
            AttemptFailure::new(
                CODE_ANALYSIS_FAILED,
                format!("bounded selection failed: {error}"),
                "retry with ripr_refresh",
            )
        })?;

        Ok(Self {
            snapshot_id,
            outcome,
            items,
            budget,
            selection,
        })
    }

    pub(crate) fn item(&self, id: &str) -> Option<&GapItem> {
        self.items
            .iter()
            .find(|item| item.canonical_id == id || item.finding_id == id)
    }
}

fn limitation_summary(outcome: &AnalysisOutcome) -> String {
    outcome
        .limitations
        .iter()
        .map(|limitation| limitation.kind.as_str())
        .collect::<Vec<_>>()
        .join(", ")
}

fn snapshot_identity(outcome: &AnalysisOutcome, items: &[GapItem]) -> Result<String, String> {
    let outcome_digest = outcome.semantic_digest()?;
    let item_ids = items
        .iter()
        .map(|item| item.canonical_id.as_str())
        .collect::<Vec<_>>();
    let evidence_digests = items
        .iter()
        .map(|item| item.evidence_sha256.as_str())
        .collect::<Vec<_>>();
    let payload = json!({
        "outcome_digest": outcome_digest,
        "items": item_ids,
        "evidence": evidence_digests,
    });
    let bytes = serde_json::to_vec(&payload)
        .map_err(|error| format!("serialize snapshot identity: {error}"))?;
    Ok(format!("snapshot:sha256:{}", sha256_hex(&bytes)))
}

fn evidence_identity(items: &[GapItem]) -> Result<String, String> {
    let item_ids = items
        .iter()
        .map(|item| item.canonical_id.as_str())
        .collect::<Vec<_>>();
    let payload = json!({ "items": item_ids });
    let bytes = serde_json::to_vec(&payload)
        .map_err(|error| format!("serialize evidence identity: {error}"))?;
    Ok(format!("evidence:sha256:{}", sha256_hex(&bytes)))
}

fn sha256_hex(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

/// Freshness is relative to the last completed snapshot. A later failed
/// attempt leaves the retained snapshot unverified for that attempt: it is
/// never labeled current at a refresh that produced no snapshot.
fn freshness_state(session: &WorkspaceSession) -> &'static str {
    match (&session.last_good, &session.last_failure) {
        (Some(_), Some(_)) => "stale_after_failed_attempt",
        (Some(_), None) => "current_at_last_refresh",
        (None, _) => "none",
    }
}

/// Mutable session state. `in_flight` marks a running attempt; `last_good`
/// is the last-known-good completed snapshot; `last_failure` is the most
/// recent terminal failure. A failure never replaces `last_good`.
#[derive(Default)]
pub(crate) struct WorkspaceSession {
    pub(crate) in_flight: bool,
    pub(crate) last_good: Option<Arc<Snapshot>>,
    pub(crate) last_failure: Option<AttemptFailure>,
    /// Session repair transactions created by `ripr_prepare_repair`, keyed by
    /// deterministic attempt identity. In-memory like the snapshot: a restart
    /// drops it, and every transaction binds the snapshot identity it was
    /// prepared against.
    pub(crate) repairs: std::collections::BTreeMap<String, super::repair::RepairTransaction>,
    /// Tombstones for session transactions evicted when a newer snapshot
    /// supersedes them (`attempt_id` → the snapshot identity it was bound
    /// to). Keeps the typed `superseded` failure reachable after the full
    /// packet is pruned, without holding superseded packets in memory for
    /// the lifetime of the session.
    pub(crate) superseded_attempts: std::collections::BTreeMap<String, String>,
}

impl WorkspaceSession {
    pub(crate) fn attempt_state(&self) -> &'static str {
        if self.in_flight {
            "in_flight"
        } else if self.last_failure.is_some() {
            "failed"
        } else if self.last_good.is_some() {
            "completed"
        } else {
            "no_snapshot"
        }
    }

    pub(crate) fn active_snapshot(
        &self,
        requested: Option<&str>,
    ) -> Result<&Arc<Snapshot>, AttemptFailure> {
        if self.in_flight {
            return Err(AttemptFailure::new(
                CODE_ANALYSIS_IN_FLIGHT,
                "an analysis attempt is running",
                "poll ripr_workspace_status until attempt_state leaves in_flight, then retry",
            ));
        }
        let Some(snapshot) = &self.last_good else {
            return Err(AttemptFailure::new(
                CODE_NO_SNAPSHOT,
                "no completed snapshot exists in this session",
                "call ripr_refresh to run one bounded analysis, then retry",
            ));
        };
        if let Some(requested) = requested
            && requested != snapshot.snapshot_id
        {
            return Err(AttemptFailure::new(
                CODE_STALE_SNAPSHOT,
                format!("snapshot {requested} is not the current completed snapshot"),
                "re-read ripr_workspace_status for the current snapshot identity, then retry",
            )
            .with_data(json!({ "current_snapshot_id": snapshot.snapshot_id })));
        }
        Ok(snapshot)
    }

    /// Bounded working set for the current (or named) snapshot. Selection is
    /// the snapshot's stored shared-budget result; this function never
    /// re-runs ranking.
    pub(crate) fn list_gaps(&self, requested: Option<&str>) -> Result<Value, AttemptFailure> {
        let snapshot = self.active_snapshot(requested)?;
        let selection = &snapshot.selection;
        let selected_ids = selection
            .selected_ids()
            .collect::<std::collections::BTreeSet<&str>>();
        let items = snapshot
            .items
            .iter()
            .filter(|item| selected_ids.contains(item.canonical_id.as_str()))
            .map(|item| item.list_summary.clone())
            .collect::<Vec<_>>();
        let document = json!({
            "schema_version": GAP_LIST_SCHEMA_VERSION,
            "snapshot_id": snapshot.snapshot_id,
            "requested_snapshot_id": requested,
            "snapshot_profile_budget_identity": selection.snapshot_profile_budget_identity,
            "selection_basis_version": selection.selection_basis_version,
            "budget": {
                "max_items_per_workspace_response": snapshot.budget.max_items_per_workspace_response,
                "max_items_per_document": snapshot.budget.max_items_per_document,
                "max_serialized_bytes": snapshot.budget.max_serialized_bytes,
                "max_inline_detail_bytes": snapshot.budget.max_inline_detail_bytes,
            },
            "total": selection.total_canonical_items,
            "eligible": selection.eligible_items,
            "selected": selection.selected.len(),
            "omitted": selection.omitted.len(),
            "selected_bytes": selection.selected_bytes,
            "complete_bytes": selection.complete_bytes,
            "overflowed": selection.overflowed,
            "overflow_reasons": selection
                .overflow_reasons
                .iter()
                .map(|reason| overflow_reason_as_str(*reason))
                .collect::<Vec<_>>(),
            "items": items,
            "omitted_items": selection
                .omitted
                .iter()
                .map(|item| json!({
                    "canonical_id": item.canonical_id,
                    "reason": omitted_reason_as_str(item.reason),
                }))
                .collect::<Vec<_>>(),
            "continuation": {
                "tool": "ripr_get_gap",
                "resource_template": "ripr://gap/{canonical_item_id}",
            },
            "claim_boundary": "Bounded working-set projection over one completed snapshot. Selection is the shared CLI/LSP budget authority; omitted identities and reasons are disclosed, never silently truncated, and no business-risk ranking is inferred.",
            "limitations": [
                "summaries do not contain evidence detail; read one item with ripr_get_gap or ripr://gap/{canonical_item_id}",
                "the list is deterministic for its snapshot identity; a refresh replaces the snapshot and its identities",
            ],
        });
        bounded_document(document)
    }

    /// One canonical item's complete bounded evidence.
    pub(crate) fn get_gap(
        &self,
        gap_id: &str,
        requested: Option<&str>,
    ) -> Result<Value, AttemptFailure> {
        let snapshot = self.active_snapshot(requested)?;
        let Some(item) = snapshot.item(gap_id) else {
            return Err(AttemptFailure::new(
                CODE_ITEM_NOT_FOUND,
                format!("no canonical item {gap_id} exists in the current snapshot"),
                "list the current canonical ids with ripr_list_gaps, then retry",
            ));
        };
        let mut document = item.document(&snapshot.snapshot_id);
        // A live session transaction binds the reserved repair-attempt link;
        // without one the link stays the explicit null the projection sets.
        if let Some(attempt_id) =
            self.live_repair_attempt(&snapshot.snapshot_id, &item.canonical_id)
            && let Some(links) = document
                .pointer_mut("/item/links")
                .and_then(Value::as_object_mut)
        {
            links.insert(
                "repair_attempt".to_string(),
                Value::from(format!("ripr://repair-attempt/{attempt_id}")),
            );
            links.remove("repair_attempt_note");
        }
        bounded_document(document)
    }

    /// The `ripr://snapshot/{snapshot_id}` evidence resource: snapshot
    /// identity, typed outcome, the full item index, and the stored selection
    /// summary — not the full evidence graph.
    pub(crate) fn snapshot_document(&self, requested: &str) -> Result<Value, AttemptFailure> {
        let snapshot = self.active_snapshot(Some(requested))?;
        let selection = &snapshot.selection;
        let outcome = serde_json::to_value(&snapshot.outcome).map_err(|error| {
            AttemptFailure::new(
                CODE_ANALYSIS_FAILED,
                format!("serialize analysis outcome: {error}"),
                "retry with ripr_refresh",
            )
        })?;
        let document = json!({
            "schema_version": SNAPSHOT_SCHEMA_VERSION,
            "snapshot_id": snapshot.snapshot_id,
            "outcome": outcome,
            "items": snapshot
                .items
                .iter()
                .map(|item| item.list_summary.clone())
                .collect::<Vec<_>>(),
            "selection": {
                "snapshot_profile_budget_identity": selection.snapshot_profile_budget_identity,
                "selection_basis_version": selection.selection_basis_version,
                "total": selection.total_canonical_items,
                "eligible": selection.eligible_items,
                "selected": selection.selected.len(),
                "omitted": selection.omitted.len(),
                "selected_bytes": selection.selected_bytes,
                "complete_bytes": selection.complete_bytes,
                "overflowed": selection.overflowed,
                "overflow_reasons": selection
                    .overflow_reasons
                    .iter()
                    .map(|reason| overflow_reason_as_str(*reason))
                    .collect::<Vec<_>>(),
            },
            "continuation": {
                "list_tool": "ripr_list_gaps",
                "item_resource_template": "ripr://gap/{canonical_item_id}",
            },
            "claim_boundary": "Bounded snapshot evidence for one completed analysis. The outcome is typed producer state; a later refresh supersedes this snapshot and its identity.",
            "limitations": [
                "item entries are identities and locations, not evidence; read one item through ripr_get_gap or ripr://gap/{canonical_item_id}",
            ],
        });
        bounded_document(document)
    }

    /// Session block for `ripr_workspace_status`. Facts only: no evidence
    /// detail, per progressive disclosure.
    pub(crate) fn session_document(&self, profile: &SessionProfile) -> Value {
        let last_completed = self.last_good.as_ref().map(|snapshot| {
            json!({
                "snapshot_id": snapshot.snapshot_id,
                "outcome_kind": snapshot.outcome.kind.as_str(),
                "finding_count": snapshot.outcome.counts.finding_count,
                "total_items": snapshot.selection.total_canonical_items,
            })
        });
        let analysis_outcome = self
            .last_good
            .as_ref()
            .and_then(|snapshot| serde_json::to_value(&snapshot.outcome).ok())
            .unwrap_or(Value::Null);
        let last_known_good = self
            .last_good
            .as_ref()
            .map(|snapshot| json!({ "snapshot_id": snapshot.snapshot_id }));
        let last_failure = self
            .last_failure
            .as_ref()
            .map(|failure| failure.value())
            .unwrap_or(Value::Null);
        json!({
            "schema_version": SESSION_SCHEMA_VERSION,
            "attempt_state": self.attempt_state(),
            "current_attempt": if self.in_flight {
                Value::from("in_flight")
            } else {
                Value::Null
            },
            "current_desired_input": {
                "kind": "workspace_diff",
                "base": "default_branch",
                "mode": "draft",
                "scope": "diff",
            },
            "last_completed_snapshot": last_completed,
            "last_known_good": last_known_good,
            "last_failure": last_failure,
            "freshness": {
                "state": freshness_state(self),
                "note": "the server does not watch the worktree; the snapshot is current as of its last completed ripr_refresh, a later failed attempt leaves it unverified for that attempt, so refresh again after edits",
            },
            "analysis_outcome": analysis_outcome,
            "profile": profile.document(),
            "limitations": [
                "the session is in-memory: restarting the server drops the snapshot unless a new ripr_refresh commits one",
                "project-local ripr.toml stays detected-not-loaded; refresh runs with built-in defaults",
            ],
        })
    }
}

/// Guard that clears [`WorkspaceSession::in_flight`] on every terminal path.
/// rmcp awaits a handler future to completion on request cancellation, but
/// transport teardown can still drop the task while the analysis worker
/// runs; without the guard a dropped handler would leave `in_flight` set
/// and fail every later refresh closed with `analysis_in_flight`.
pub(crate) struct InFlightAttempt {
    session: Arc<Mutex<WorkspaceSession>>,
    committed: bool,
}

impl InFlightAttempt {
    pub(crate) fn new(session: Arc<Mutex<WorkspaceSession>>) -> Self {
        Self {
            session,
            committed: false,
        }
    }

    /// The handler committed the attempt outcome itself; `Drop` becomes a
    /// no-op and the normal path owns the state transition.
    pub(crate) fn disarm(&mut self) {
        self.committed = true;
    }
}

impl Drop for InFlightAttempt {
    fn drop(&mut self) {
        if self.committed {
            return;
        }
        if let Ok(mut session) = self.session.try_lock() {
            session.in_flight = false;
            return;
        }
        // The lock is momentarily held by a concurrent projection; reset on
        // the runtime instead of leaving the session stuck in flight.
        if let Ok(runtime) = tokio::runtime::Handle::try_current() {
            let session = self.session.clone();
            let _reset = runtime.spawn(async move {
                let mut session = session.lock().await;
                session.in_flight = false;
            });
        }
    }
}

/// The refresh-time attempt document returned by `ripr_refresh`.
pub(crate) fn refresh_document(session: &WorkspaceSession) -> Value {
    let snapshot = session.last_good.as_ref().map(|snapshot| {
        json!({
            "snapshot_id": snapshot.snapshot_id,
            "outcome_kind": snapshot.outcome.kind.as_str(),
            "finding_count": snapshot.outcome.counts.finding_count,
            "total_items": snapshot.selection.total_canonical_items,
            "list_route": "ripr_list_gaps",
            "snapshot_resource": format!("ripr://snapshot/{}", snapshot.snapshot_id),
        })
    });
    json!({
        "schema_version": REFRESH_SCHEMA_VERSION,
        "attempt": {
            "state": session.attempt_state(),
            "failure": session
                .last_failure
                .as_ref()
                .map(|failure| failure.value())
                .unwrap_or(Value::Null),
        },
        "snapshot": snapshot,
        "last_known_good": session
            .last_good
            .as_ref()
            .map(|snapshot| json!({ "snapshot_id": snapshot.snapshot_id })),
        "claim_boundary": "One bounded static analysis attempt over the workspace diff. The server never edits source, executes verification or mutation commands, or loads project-local provider configuration.",
        "limitations": [
            "an attempt runs to a terminal state; MCP cancellation of the request does not roll back a running attempt and never manufactures a snapshot",
            "a cancelled or superseded attempt is never committed as a completed snapshot",
        ],
    })
}

/// Built-in-default analysis profile facts for the session block.
pub(crate) struct SessionProfile {
    mode: &'static str,
    languages: Vec<String>,
}

impl SessionProfile {
    pub(crate) fn built_in() -> Self {
        let languages = crate::config::RiprConfig::default()
            .languages()
            .enabled()
            .iter()
            .map(|language| language.as_str().to_string())
            .collect();
        Self {
            mode: "draft",
            languages,
        }
    }

    fn document(&self) -> Value {
        json!({
            "mode": self.mode,
            "languages": self.languages,
            "project_config": "detected_not_loaded",
            "support": "built_in_defaults",
        })
    }
}

/// Run one bounded analysis through the shared check authority and bind it
/// into a snapshot. This is the only bridge from the session to the
/// producer: read-only static analysis, identical to what `ripr check` and
/// the LSP run in-process.
pub(crate) fn run_check(
    root: &Path,
    root_identity: Option<&str>,
) -> Result<Snapshot, AttemptFailure> {
    let input = crate::app::CheckInput {
        root: PathBuf::from(root),
        git_timeout: Some(crate::app::default_cli_git_timeout()),
        ..Default::default()
    };
    let output = crate::app::check_workspace(input).map_err(|error| {
        AttemptFailure::new(
            CODE_ANALYSIS_FAILED,
            format!("shared check authority failed: {error}"),
            "retry with ripr_refresh; if the failure persists, run `ripr check --format json` in the repository for the full diagnostic",
        )
    })?;
    Snapshot::from_output(&output, root_identity)
}

fn overflow_reason_as_str(reason: DiagnosticOverflowReason) -> &'static str {
    match reason {
        DiagnosticOverflowReason::DocumentItemLimit => "document_item_limit",
        DiagnosticOverflowReason::WorkspaceItemLimit => "workspace_item_limit",
        DiagnosticOverflowReason::SerializedByteLimit => "serialized_byte_limit",
        DiagnosticOverflowReason::InlineDetailLimit => "inline_detail_limit",
    }
}

fn omitted_reason_as_str(reason: OmittedDiagnosticReason) -> &'static str {
    match reason {
        OmittedDiagnosticReason::ProfileFiltered => "profile_filtered",
        OmittedDiagnosticReason::DocumentItemLimit => "document_item_limit",
        OmittedDiagnosticReason::WorkspaceItemLimit => "workspace_item_limit",
        OmittedDiagnosticReason::SerializedByteLimit => "serialized_byte_limit",
    }
}

/// Fail closed when even one bounded document cannot fit the wire cap: an
/// over-cap response would otherwise terminate the transport.
pub(crate) fn bounded_document(document: Value) -> Result<Value, AttemptFailure> {
    struct LengthWriter(usize);

    impl std::io::Write for LengthWriter {
        fn write(&mut self, buffer: &[u8]) -> std::io::Result<usize> {
            self.0 += buffer.len();
            Ok(buffer.len())
        }

        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    let mut writer = LengthWriter(0);
    serde_json::to_writer(&mut writer, &document).map_err(|error| {
        AttemptFailure::new(
            CODE_ANALYSIS_FAILED,
            format!("serialize document: {error}"),
            "retry with ripr_refresh",
        )
    })?;
    if writer.0 > super::MAX_RESPONSE_BYTES {
        return Err(AttemptFailure::new(
            CODE_RESULT_TOO_LARGE,
            format!(
                "document is {} bytes; the MCP response bound is {}",
                writer.0,
                super::MAX_RESPONSE_BYTES
            ),
            "read narrower evidence (one item through ripr_get_gap) instead of widening the response",
        ));
    }
    Ok(document)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis_outcome::{
        AnalysisLimitation, AnalysisLimitationKind, AnalysisOutcomeCounts, AnalysisRecovery,
        AnalysisRecoveryKind, AnalysisStage,
    };
    use std::path::PathBuf;

    fn limitation(kind: AnalysisLimitationKind) -> Result<AnalysisLimitation, String> {
        Ok(AnalysisLimitation::new(
            kind,
            AnalysisStage::DiffParse,
            AnalysisRecovery::new(
                AnalysisRecoveryKind::Retry,
                "inspect the typed limitation before retrying",
            )?,
        ))
    }

    fn outcome(
        kind: AnalysisOutcomeKind,
        findings: u64,
        limitations: Vec<AnalysisLimitation>,
    ) -> Result<AnalysisOutcome, String> {
        AnalysisOutcome::new(
            kind,
            Default::default(),
            AnalysisOutcomeCounts {
                changed_file_count: 1,
                changed_line_count: 2,
                candidate_line_count: 2,
                probe_count: 2,
                finding_count: findings,
            },
            limitations,
        )
    }

    /// A CheckOutput shell carrying only what snapshot binding reads. No
    /// analysis runs in these tests: the session contract is exercised
    /// against typed producer outcomes directly.
    fn output(
        kind: AnalysisOutcomeKind,
        findings: u64,
        limitations: Vec<AnalysisLimitation>,
    ) -> Result<crate::app::CheckOutput, String> {
        Ok(crate::app::CheckOutput {
            schema_version: crate::app::CHECK_OUTPUT_SCHEMA_VERSION.to_string(),
            harness_projections: Vec::new(),
            tool: "ripr".to_string(),
            mode: crate::app::Mode::Draft,
            root: PathBuf::from("."),
            base: None,
            analysis_outcome: Some(outcome(kind, findings, limitations)?),
            summary: crate::domain::Summary::default(),
            findings: Vec::new(),
            preview_language_advisories: Vec::new(),
            language_runs: Vec::new(),
            no_scope_provided: false,
            unanalyzed_working_tree: false,
            suppression: None,
            partial_scope: None,
        })
    }

    fn session_with(kind: AnalysisOutcomeKind, findings: u64) -> Result<WorkspaceSession, String> {
        let output = output(kind, findings, Vec::new())?;
        let snapshot = Snapshot::from_output(&output, Some("root:sha256:test"))
            .map_err(|failure| failure.detail)?;
        Ok(WorkspaceSession {
            in_flight: false,
            last_good: Some(Arc::new(snapshot)),
            last_failure: None,
            repairs: std::collections::BTreeMap::new(),
            superseded_attempts: std::collections::BTreeMap::new(),
        })
    }

    fn expect_code(result: Result<Value, AttemptFailure>, code: &str) -> Result<(), String> {
        match result {
            Ok(value) => Err(format!("expected {code}, got success: {value}")),
            Err(failure) if failure.code == code => Ok(()),
            Err(failure) => Err(format!("expected {code}, got {}", failure.code)),
        }
    }

    #[test]
    fn complete_zero_and_incomplete_zero_stay_distinct() -> Result<(), String> {
        let complete = session_with(AnalysisOutcomeKind::CompleteNoFindings, 0)?;
        let limited = output(
            AnalysisOutcomeKind::PartialWithLimitations,
            0,
            vec![limitation(AnalysisLimitationKind::ProducerFailure)?],
        )?;
        let limited_snapshot = Snapshot::from_output(&limited, Some("root:sha256:test"))
            .map_err(|failure| failure.detail)?;
        let incomplete = WorkspaceSession {
            in_flight: false,
            last_good: Some(Arc::new(limited_snapshot)),
            last_failure: None,
            repairs: std::collections::BTreeMap::new(),
            superseded_attempts: std::collections::BTreeMap::new(),
        };
        let _ = complete.list_gaps(None).map_err(|failure| failure.detail)?;
        let incomplete_doc = incomplete
            .list_gaps(None)
            .map_err(|failure| failure.detail)?;
        let complete_snapshot = complete
            .last_good
            .as_ref()
            .ok_or_else(|| "complete session lost its snapshot".to_string())?;
        let incomplete_snapshot = incomplete
            .last_good
            .as_ref()
            .ok_or_else(|| "incomplete session lost its snapshot".to_string())?;
        if complete_snapshot.snapshot_id == incomplete_snapshot.snapshot_id {
            return Err(
                "complete-zero and incomplete-zero must not share a snapshot identity".to_string(),
            );
        }
        if complete_snapshot.outcome.kind == incomplete_snapshot.outcome.kind {
            return Err(
                "complete-zero and incomplete-zero must keep distinct outcome kinds".to_string(),
            );
        }
        let incomplete_list = incomplete_doc
            .pointer("/items")
            .and_then(Value::as_array)
            .ok_or_else(|| "incomplete-zero list lost its items array".to_string())?;
        if !incomplete_list.is_empty() {
            return Err("incomplete-zero must not project phantom items".to_string());
        }
        Ok(())
    }

    #[test]
    fn unsupported_input_never_becomes_a_snapshot() -> Result<(), String> {
        let unsupported = output(
            AnalysisOutcomeKind::UnsupportedInput,
            0,
            vec![limitation(AnalysisLimitationKind::CombinedHunkUnsupported)?],
        )?;
        let failure = Snapshot::from_output(&unsupported, None)
            .err()
            .ok_or_else(|| "unsupported input must fail closed".to_string())?;
        if failure.code != CODE_UNSUPPORTED_PROFILE {
            return Err(format!("unexpected failure code: {}", failure.code));
        }
        Ok(())
    }

    #[test]
    fn list_get_snapshot_before_any_refresh_fail_closed() -> Result<(), String> {
        let session = WorkspaceSession::default();
        expect_code(session.list_gaps(None), CODE_NO_SNAPSHOT)?;
        expect_code(session.get_gap("gap:any", None), CODE_NO_SNAPSHOT)?;
        expect_code(
            session.snapshot_document("snapshot:sha256:none"),
            CODE_NO_SNAPSHOT,
        )?;
        let document = session.session_document(&SessionProfile::built_in());
        if document.pointer("/attempt_state").and_then(Value::as_str) != Some("no_snapshot") {
            return Err(format!("fresh session must be no_snapshot: {document}"));
        }
        Ok(())
    }

    #[test]
    fn in_flight_session_reports_analysis_in_flight() -> Result<(), String> {
        let session = WorkspaceSession {
            in_flight: true,
            ..Default::default()
        };
        expect_code(session.list_gaps(None), CODE_ANALYSIS_IN_FLIGHT)?;
        expect_code(session.get_gap("gap:any", None), CODE_ANALYSIS_IN_FLIGHT)?;
        expect_code(
            session.snapshot_document("snapshot:sha256:any"),
            CODE_ANALYSIS_IN_FLIGHT,
        )?;
        let document = session.session_document(&SessionProfile::built_in());
        if document.pointer("/attempt_state").and_then(Value::as_str) != Some("in_flight") {
            return Err(format!("in-flight session state drifted: {document}"));
        }
        Ok(())
    }

    #[test]
    fn stale_snapshot_identity_fails_closed_with_current_identity() -> Result<(), String> {
        let session = session_with(AnalysisOutcomeKind::CompleteNoFindings, 0)?;
        let current = session
            .last_good
            .as_ref()
            .ok_or_else(|| "missing snapshot".to_string())?
            .snapshot_id
            .clone();
        match session.list_gaps(Some("snapshot:sha256:old")) {
            Ok(value) => Err(format!("stale snapshot must fail closed: {value}")),
            Err(failure) if failure.code != CODE_STALE_SNAPSHOT => {
                Err(format!("unexpected failure code: {}", failure.code))
            }
            Err(failure) => {
                if failure
                    .data
                    .pointer("/current_snapshot_id")
                    .and_then(Value::as_str)
                    != Some(current.as_str())
                {
                    return Err(format!(
                        "stale failure lost the current identity: {}",
                        failure.data
                    ));
                }
                Ok(())
            }
        }
    }

    #[test]
    fn snapshot_resource_binds_identity_and_typed_outcome() -> Result<(), String> {
        let session = session_with(AnalysisOutcomeKind::CompleteNoFindings, 0)?;
        let current = session
            .last_good
            .as_ref()
            .ok_or_else(|| "missing snapshot".to_string())?
            .snapshot_id
            .clone();
        let document = session
            .snapshot_document(&current)
            .map_err(|failure| failure.detail)?;
        if document.pointer("/schema_version").and_then(Value::as_str)
            != Some(SNAPSHOT_SCHEMA_VERSION)
        {
            return Err(format!("snapshot resource lost its schema: {document}"));
        }
        if document.pointer("/snapshot_id").and_then(Value::as_str) != Some(current.as_str()) {
            return Err("snapshot resource lost its identity".to_string());
        }
        if document.pointer("/outcome/kind").and_then(Value::as_str) != Some("complete_no_findings")
        {
            return Err(format!(
                "snapshot resource lost the typed outcome: {document}"
            ));
        }
        Ok(())
    }

    #[test]
    fn unknown_item_fails_closed_and_never_fabricates_evidence() -> Result<(), String> {
        let session = session_with(AnalysisOutcomeKind::CompleteNoFindings, 0)?;
        expect_code(session.get_gap("gap:missing", None), CODE_ITEM_NOT_FOUND)?;
        Ok(())
    }

    #[test]
    fn session_document_projects_attempt_state_and_built_in_profile() -> Result<(), String> {
        let session = session_with(AnalysisOutcomeKind::CompleteNoFindings, 0)?;
        let document = session.session_document(&SessionProfile::built_in());
        if document.pointer("/attempt_state").and_then(Value::as_str) != Some("completed") {
            return Err(format!("completed session state drifted: {document}"));
        }
        if document
            .pointer("/profile/project_config")
            .and_then(Value::as_str)
            != Some("detected_not_loaded")
        {
            return Err("session profile must stay detected-not-loaded".to_string());
        }
        if document
            .pointer("/last_completed_snapshot/snapshot_id")
            .is_none()
        {
            return Err("session lost its last completed snapshot identity".to_string());
        }
        if document.pointer("/analysis_outcome/kind").is_none() {
            return Err("session lost its typed analysis outcome".to_string());
        }
        let profile = SessionProfile::built_in();
        if profile.languages.is_empty() {
            return Err("built-in profile must name at least one language".to_string());
        }
        Ok(())
    }

    #[test]
    fn failure_detail_is_bounded_and_documents_carry_schema() -> Result<(), String> {
        let long = "x".repeat(MAX_FAILURE_DETAIL_CHARS * 2);
        let failure = AttemptFailure::new(CODE_ANALYSIS_FAILED, long, "retry");
        if failure.detail.chars().count() > MAX_FAILURE_DETAIL_CHARS + 1 {
            return Err(format!(
                "failure detail was not bounded: {}",
                failure.detail.len()
            ));
        }
        let document = failure.document(REFRESH_SCHEMA_VERSION);
        if document.pointer("/schema_version").and_then(Value::as_str)
            != Some(REFRESH_SCHEMA_VERSION)
            || document.pointer("/failure/code").and_then(Value::as_str)
                != Some(CODE_ANALYSIS_FAILED)
        {
            return Err(format!("failure document lost its contract: {document}"));
        }
        Ok(())
    }

    #[test]
    fn refresh_document_reports_failure_without_dropping_last_known_good() -> Result<(), String> {
        let mut session = session_with(AnalysisOutcomeKind::CompleteNoFindings, 0)?;
        let failure = AttemptFailure::new(CODE_ANALYSIS_FAILED, "producer hiccup", "retry");
        session.last_failure = Some(failure);
        let document = refresh_document(&session);
        if document.pointer("/attempt/state").and_then(Value::as_str) != Some("failed") {
            return Err(format!("failed attempt state drifted: {document}"));
        }
        if document
            .pointer("/attempt/failure/code")
            .and_then(Value::as_str)
            != Some(CODE_ANALYSIS_FAILED)
        {
            return Err("failed attempt lost its typed failure".to_string());
        }
        if document.pointer("/last_known_good/snapshot_id").is_none() {
            return Err("a failed refresh must not drop the last-known-good snapshot".to_string());
        }
        if document.pointer("/snapshot/snapshot_id").is_none() {
            return Err("the committed snapshot summary must survive a later failure".to_string());
        }
        Ok(())
    }

    #[test]
    fn portable_snapshot_identity_ignores_the_concrete_root() -> Result<(), String> {
        let first = output(AnalysisOutcomeKind::CompleteNoFindings, 0, Vec::new())?;
        let second = output(AnalysisOutcomeKind::CompleteNoFindings, 0, Vec::new())?;
        let one = Snapshot::from_output(&first, Some("root:sha256:one")).map_err(|f| f.detail)?;
        let two = Snapshot::from_output(&second, Some("root:sha256:two")).map_err(|f| f.detail)?;
        if one.snapshot_id != two.snapshot_id {
            return Err(
                "equivalent roots must preserve one portable snapshot identity".to_string(),
            );
        }
        Ok(())
    }

    #[test]
    fn reserved_failure_vocabulary_stays_named() -> Result<(), String> {
        for code in RESERVED_FAILURE_CODES {
            if code.is_empty() || code.contains(' ') {
                return Err(format!("reserved code is not a wire token: {code}"));
            }
        }
        Ok(())
    }
}
