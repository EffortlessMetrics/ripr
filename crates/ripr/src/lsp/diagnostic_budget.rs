//! Deterministic passive diagnostic delivery budgeting.
//!
//! This module is a presentation projection over already-canonical evidence.
//! It does not discover, merge, rank, or otherwise reinterpret findings. The
//! producer supplies eligibility and evidence-owned ordering ranks; this module
//! only applies finite delivery limits and records every omitted identity.

use std::collections::{BTreeMap, BTreeSet};

use sha2::{Digest, Sha256};

pub const DIAGNOSTIC_BUDGET_SCHEMA_VERSION: &str = "lsp-diagnostic-budget-v1";
pub const DIAGNOSTIC_BUDGET_SELECTION_VERSION: &str = "evidence-order-v1";
pub const DEFAULT_MAX_ITEMS_PER_DOCUMENT: usize = 50;
pub const DEFAULT_MAX_ITEMS_PER_WORKSPACE_RESPONSE: usize = 500;
pub const DEFAULT_MAX_SERIALIZED_BYTES: usize = 64 * 1024;
pub const DEFAULT_MAX_INLINE_DETAIL_BYTES: usize = 4 * 1024;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DiagnosticBudget {
    pub max_items_per_document: usize,
    pub max_items_per_workspace_response: usize,
    pub max_serialized_bytes: usize,
    pub max_inline_detail_bytes: usize,
}

impl Default for DiagnosticBudget {
    fn default() -> Self {
        Self {
            max_items_per_document: DEFAULT_MAX_ITEMS_PER_DOCUMENT,
            max_items_per_workspace_response: DEFAULT_MAX_ITEMS_PER_WORKSPACE_RESPONSE,
            max_serialized_bytes: DEFAULT_MAX_SERIALIZED_BYTES,
            max_inline_detail_bytes: DEFAULT_MAX_INLINE_DETAIL_BYTES,
        }
    }
}

impl DiagnosticBudget {
    pub fn validate(&self) -> Result<(), DiagnosticBudgetError> {
        if self.max_items_per_document == 0 {
            return Err(DiagnosticBudgetError::ZeroLimit("max_items_per_document"));
        }
        if self.max_items_per_workspace_response == 0 {
            return Err(DiagnosticBudgetError::ZeroLimit(
                "max_items_per_workspace_response",
            ));
        }
        if self.max_serialized_bytes == 0 {
            return Err(DiagnosticBudgetError::ZeroLimit("max_serialized_bytes"));
        }
        if self.max_inline_detail_bytes == 0 {
            return Err(DiagnosticBudgetError::ZeroLimit("max_inline_detail_bytes"));
        }
        if self.max_items_per_document > self.max_items_per_workspace_response {
            return Err(DiagnosticBudgetError::ContradictoryLimits(
                "max_items_per_document exceeds max_items_per_workspace_response",
            ));
        }
        Ok(())
    }

    fn identity_fragment(&self) -> String {
        format!(
            "{}:{}:{}:{}",
            self.max_items_per_document,
            self.max_items_per_workspace_response,
            self.max_serialized_bytes,
            self.max_inline_detail_bytes
        )
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DiagnosticBudgetError {
    ZeroLimit(&'static str),
    ContradictoryLimits(&'static str),
}

impl std::fmt::Display for DiagnosticBudgetError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ZeroLimit(name) => write!(formatter, "{name} must be greater than zero"),
            Self::ContradictoryLimits(message) => formatter.write_str(message),
        }
    }
}

impl std::error::Error for DiagnosticBudgetError {}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum DiagnosticBudgetEligibility {
    Actionable,
    ProfileFiltered,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct DiagnosticSelectionKey {
    /// Lower values mean a more complete producer-owned repair route.
    pub repair_route_rank: u8,
    /// Lower values mean a more directly change-caused producer-owned state.
    pub causal_rank: u8,
    /// Lower values mean stronger supported producer-owned evidence.
    pub evidence_rank: u8,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DiagnosticBudgetItem {
    pub canonical_id: String,
    pub document: String,
    /// UTF-8 bytes of the normalized passive diagnostic payload.
    pub payload_bytes: usize,
    /// Bytes of optional witness/detail content that can be retrieved lazily.
    pub inline_detail_bytes: usize,
    pub eligibility: DiagnosticBudgetEligibility,
    pub selection_key: DiagnosticSelectionKey,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum DiagnosticOverflowReason {
    DocumentItemLimit,
    WorkspaceItemLimit,
    SerializedByteLimit,
    InlineDetailLimit,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SelectedDiagnosticItem {
    pub canonical_id: String,
    pub document: String,
    pub payload_bytes: usize,
    pub inline_detail_omitted: bool,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum OmittedDiagnosticReason {
    ProfileFiltered,
    DocumentItemLimit,
    WorkspaceItemLimit,
    SerializedByteLimit,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OmittedDiagnosticItem {
    pub canonical_id: String,
    pub reason: OmittedDiagnosticReason,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DiagnosticBudgetResult {
    pub schema_version: &'static str,
    pub snapshot_profile_budget_identity: String,
    pub complete_evidence_identity: String,
    pub continuation_or_inspect_route: String,
    pub selection_basis_version: &'static str,
    pub total_canonical_items: usize,
    pub eligible_items: usize,
    pub selected: Vec<SelectedDiagnosticItem>,
    pub omitted: Vec<OmittedDiagnosticItem>,
    pub selected_bytes: usize,
    pub complete_bytes: usize,
    pub overflowed: bool,
    pub overflow_reasons: BTreeSet<DiagnosticOverflowReason>,
}

impl DiagnosticBudgetResult {
    pub fn selected_ids(&self) -> impl Iterator<Item = &str> {
        self.selected.iter().map(|item| item.canonical_id.as_str())
    }

    pub fn omitted_ids(&self) -> impl Iterator<Item = &str> {
        self.omitted.iter().map(|item| item.canonical_id.as_str())
    }
}

pub fn evaluate_diagnostic_budget(
    items: impl IntoIterator<Item = DiagnosticBudgetItem>,
    budget: &DiagnosticBudget,
    snapshot_profile_identity: &str,
    complete_evidence_identity: &str,
) -> Result<DiagnosticBudgetResult, DiagnosticBudgetError> {
    budget.validate()?;

    let mut items = items.into_iter().collect::<Vec<_>>();
    items.sort_by(|left, right| {
        left.selection_key
            .cmp(&right.selection_key)
            .then_with(|| left.canonical_id.cmp(&right.canonical_id))
            .then_with(|| left.document.cmp(&right.document))
    });

    let total_canonical_items = items.len();
    let eligible_items = items
        .iter()
        .filter(|item| item.eligibility == DiagnosticBudgetEligibility::Actionable)
        .count();
    let complete_bytes = items
        .iter()
        .filter(|item| item.eligibility == DiagnosticBudgetEligibility::Actionable)
        .map(|item| item.payload_bytes)
        .fold(0usize, usize::saturating_add);
    let mut selected = Vec::new();
    let mut omitted = Vec::new();
    let mut overflow_reasons = BTreeSet::new();
    let mut selected_per_document = std::collections::BTreeMap::<String, usize>::new();
    let mut selected_bytes = 0usize;

    for item in items {
        if item.eligibility == DiagnosticBudgetEligibility::ProfileFiltered {
            omitted.push(OmittedDiagnosticItem {
                canonical_id: item.canonical_id,
                reason: OmittedDiagnosticReason::ProfileFiltered,
            });
            continue;
        }

        let document_count = selected_per_document
            .get(&item.document)
            .copied()
            .unwrap_or(0);
        if document_count >= budget.max_items_per_document {
            overflow_reasons.insert(DiagnosticOverflowReason::DocumentItemLimit);
            omitted.push(OmittedDiagnosticItem {
                canonical_id: item.canonical_id,
                reason: OmittedDiagnosticReason::DocumentItemLimit,
            });
            continue;
        }
        if selected.len() >= budget.max_items_per_workspace_response {
            overflow_reasons.insert(DiagnosticOverflowReason::WorkspaceItemLimit);
            omitted.push(OmittedDiagnosticItem {
                canonical_id: item.canonical_id,
                reason: OmittedDiagnosticReason::WorkspaceItemLimit,
            });
            continue;
        }
        if selected_bytes.saturating_add(item.payload_bytes) > budget.max_serialized_bytes {
            overflow_reasons.insert(DiagnosticOverflowReason::SerializedByteLimit);
            if !selected.is_empty() {
                omitted.push(OmittedDiagnosticItem {
                    canonical_id: item.canonical_id,
                    reason: OmittedDiagnosticReason::SerializedByteLimit,
                });
                continue;
            }
        }

        let inline_detail_omitted = item.inline_detail_bytes > budget.max_inline_detail_bytes;
        if inline_detail_omitted {
            overflow_reasons.insert(DiagnosticOverflowReason::InlineDetailLimit);
        }
        selected_bytes = selected_bytes.saturating_add(item.payload_bytes);
        *selected_per_document
            .entry(item.document.clone())
            .or_insert(0) += 1;
        selected.push(SelectedDiagnosticItem {
            canonical_id: item.canonical_id,
            document: item.document,
            payload_bytes: item.payload_bytes,
            inline_detail_omitted,
        });
    }

    let budget_identity = format!(
        "{DIAGNOSTIC_BUDGET_SCHEMA_VERSION}:{snapshot_profile_identity}:{}",
        budget.identity_fragment()
    );
    let overflowed = !overflow_reasons.is_empty()
        || omitted
            .iter()
            .any(|item| item.reason != OmittedDiagnosticReason::ProfileFiltered);
    Ok(DiagnosticBudgetResult {
        schema_version: DIAGNOSTIC_BUDGET_SCHEMA_VERSION,
        snapshot_profile_budget_identity: budget_identity,
        complete_evidence_identity: complete_evidence_identity.to_string(),
        continuation_or_inspect_route: "ripr/listActionableItems".to_string(),
        selection_basis_version: DIAGNOSTIC_BUDGET_SELECTION_VERSION,
        total_canonical_items,
        eligible_items,
        selected,
        omitted,
        selected_bytes,
        complete_bytes,
        overflowed,
        overflow_reasons,
    })
}

/// Outcome of computing the one delivery selection for a snapshot. The full
/// [`DiagnosticBudgetResult`] is retained so the omission evidence the budget
/// already computes is disclosed rather than dropped (#1969).
#[derive(Clone, Debug)]
pub(crate) enum DiagnosticDeliveryOutcome {
    Applied {
        result: Box<DiagnosticBudgetResult>,
        /// Canonical identity to publishing document, recovered from the
        /// budget items so per-document omitted counts can be disclosed
        /// without changing the shared budget result shape.
        document_by_canonical_id: BTreeMap<String, String>,
        /// Publishing document to ordered canonical identities, in the same
        /// order the budget builder walked the documents, so the delivery
        /// filter does not re-serialize every diagnostic.
        ids_by_document: BTreeMap<String, Vec<String>>,
        /// Publishing document to the selected canonical identities it
        /// serves. This is the exact per-document membership every transport
        /// must agree on (#1973).
        selected_ids_by_document: BTreeMap<String, Vec<String>>,
    },
    /// The budget could not be built or evaluated. Delivery falls back to
    /// unfiltered serving; the reason class names the failure family and the
    /// detail carries the human-readable cause. Consumers match on the class,
    /// never on the detail string.
    Unavailable {
        reason: DiagnosticDeliveryUnavailableReason,
        detail: String,
    },
}

/// The failure family of an unavailable delivery selection.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum DiagnosticDeliveryUnavailableReason {
    /// Serializing diagnostics into budget items failed.
    ItemSerialization,
    /// The budget evaluation itself rejected its inputs.
    Evaluation,
}

impl DiagnosticDeliveryUnavailableReason {
    pub(crate) fn as_status_reason(self) -> &'static str {
        match self {
            Self::ItemSerialization => "serialization_failure",
            Self::Evaluation => "evaluation_failure",
        }
    }
}

/// The one immutable diagnostic delivery selection shared by push publication
/// and both pull handlers (#1973).
///
/// This is computed once per snapshot (at refresh-transaction prepare time,
/// before any publication) and stored on the committed snapshot. Neither
/// transport may rerun ranking or re-evaluate the budget with a different
/// result: push and pull for the same snapshot, profile, and budget read this
/// stored outcome and therefore agree on selected items, per-document
/// membership, omitted identities and reasons, overflow state, and the
/// retrieval route.
#[derive(Clone, Debug)]
pub(crate) struct DiagnosticDeliverySelection {
    pub(crate) budget: DiagnosticBudget,
    pub(crate) outcome: DiagnosticDeliveryOutcome,
}

impl DiagnosticDeliverySelection {
    /// Compute the delivery selection for one snapshot's complete diagnostics.
    ///
    /// `snapshot_profile_identity` binds the selection to the snapshot input
    /// and diagnostic profile; `complete_evidence_identity` binds it to the
    /// complete (unfiltered) evidence so a budget/profile/input change
    /// produces a new selection identity without renumbering canonical
    /// evidence. Computation failure is recorded as
    /// [`DiagnosticDeliveryOutcome::Unavailable`] rather than propagated:
    /// delivery then falls back to unfiltered serving with a disclosed
    /// partial state.
    pub(crate) fn evaluate(
        diagnostics_by_uri: &BTreeMap<
            tower_lsp_server::ls_types::Uri,
            Vec<tower_lsp_server::ls_types::Diagnostic>,
        >,
        budget: &DiagnosticBudget,
        snapshot_profile_identity: &str,
        complete_evidence_identity: &str,
    ) -> Self {
        let outcome = Self::evaluate_outcome(
            diagnostics_by_uri,
            budget,
            snapshot_profile_identity,
            complete_evidence_identity,
        );
        Self {
            budget: budget.clone(),
            outcome,
        }
    }

    fn evaluate_outcome(
        diagnostics_by_uri: &BTreeMap<
            tower_lsp_server::ls_types::Uri,
            Vec<tower_lsp_server::ls_types::Diagnostic>,
        >,
        budget: &DiagnosticBudget,
        snapshot_profile_identity: &str,
        complete_evidence_identity: &str,
    ) -> DiagnosticDeliveryOutcome {
        let items = match build_budget_items_from_diagnostics(diagnostics_by_uri) {
            Ok(items) => items,
            Err(error) => {
                return DiagnosticDeliveryOutcome::Unavailable {
                    reason: DiagnosticDeliveryUnavailableReason::ItemSerialization,
                    detail: format!("budget item serialization failed: {error}"),
                };
            }
        };
        let document_by_canonical_id = items
            .iter()
            .map(|item| (item.canonical_id.clone(), item.document.clone()))
            .collect();
        let mut ids_by_document: BTreeMap<String, Vec<String>> = BTreeMap::new();
        for item in &items {
            ids_by_document
                .entry(item.document.clone())
                .or_default()
                .push(item.canonical_id.clone());
        }
        match evaluate_diagnostic_budget(
            items,
            budget,
            snapshot_profile_identity,
            complete_evidence_identity,
        ) {
            Ok(result) => {
                let mut selected_ids_by_document: BTreeMap<String, Vec<String>> = BTreeMap::new();
                for item in &result.selected {
                    selected_ids_by_document
                        .entry(item.document.clone())
                        .or_default()
                        .push(item.canonical_id.clone());
                }
                DiagnosticDeliveryOutcome::Applied {
                    result: Box::new(result),
                    document_by_canonical_id,
                    ids_by_document,
                    selected_ids_by_document,
                }
            }
            Err(error) => DiagnosticDeliveryOutcome::Unavailable {
                reason: DiagnosticDeliveryUnavailableReason::Evaluation,
                detail: format!("budget evaluation failed: {error}"),
            },
        }
    }

    /// The diagnostics one document serves under this selection. This is the
    /// single membership authority for both transports: an unavailable budget
    /// serves the document unfiltered (named by the fallback disclosure); a
    /// computed selection is applied strictly, so zero selected means zero
    /// served. Membership is checked against the stored per-document
    /// selected set, not a re-evaluated budget.
    pub(crate) fn diagnostics_for_document(
        &self,
        document: &str,
        diagnostics: &[tower_lsp_server::ls_types::Diagnostic],
    ) -> Vec<tower_lsp_server::ls_types::Diagnostic> {
        let DiagnosticDeliveryOutcome::Applied {
            ids_by_document,
            selected_ids_by_document,
            ..
        } = &self.outcome
        else {
            return diagnostics.to_vec();
        };
        let selected_for_document = selected_ids_by_document
            .get(document)
            .map(|ids| ids.iter().map(String::as_str).collect::<BTreeSet<_>>())
            .unwrap_or_default();
        // Ids were computed by the budget builder in this exact order; reuse
        // them instead of serializing every diagnostic again.
        if let Some(ordered_ids) = ids_by_document.get(document)
            && ordered_ids.len() == diagnostics.len()
        {
            return diagnostics
                .iter()
                .zip(ordered_ids.iter())
                .filter(|(_, id)| selected_for_document.contains(id.as_str()))
                .map(|(diagnostic, _)| diagnostic.clone())
                .collect();
        }
        diagnostics
            .iter()
            .filter(|diagnostic| {
                let payload = serde_json::to_vec(diagnostic).unwrap_or_default();
                let id = diagnostic_canonical_id(diagnostic, document, &payload);
                selected_for_document.contains(id.as_str())
            })
            .cloned()
            .collect()
    }
}

/// Build [`DiagnosticBudgetItem`]s from a snapshot's diagnostics.
///
/// This is the integration bridge between the LSP diagnostics (tower-lsp
/// `Diagnostic` values keyed by URI) and the budget evaluator. Each
/// diagnostic is measured for payload bytes (serialized JSON size) and
/// assigned a default `DiagnosticSelectionKey` (all ranks equal — the
/// producer-owned ranks from the finding/seam are not yet plumbed through
/// to the diagnostic payload; a follow-up PR will enrich the selection
/// key from the classified seam evidence).
///
/// The `canonical_id` is extracted from the diagnostic's producer-owned
/// `data` field. The location/hash fallback is retained for legacy diagnostics
/// that predate the explicit identity fields, but it is deterministic and is
/// never used as an actionability signal.
pub fn build_budget_items_from_diagnostics(
    diagnostics_by_uri: &std::collections::BTreeMap<
        tower_lsp_server::ls_types::Uri,
        Vec<tower_lsp_server::ls_types::Diagnostic>,
    >,
) -> Result<Vec<DiagnosticBudgetItem>, serde_json::Error> {
    let mut items = Vec::new();
    for (uri, diagnostics) in diagnostics_by_uri {
        let document = uri.as_str().to_string();
        for diagnostic in diagnostics {
            let payload = serde_json::to_vec(diagnostic)?;
            let payload_bytes = payload.len();
            let canonical_id = diagnostic_canonical_id(diagnostic, &document, &payload);
            items.push(DiagnosticBudgetItem {
                canonical_id,
                document: document.clone(),
                payload_bytes,
                inline_detail_bytes: 0,
                eligibility: if diagnostic_is_actionable(diagnostic) {
                    DiagnosticBudgetEligibility::Actionable
                } else {
                    DiagnosticBudgetEligibility::ProfileFiltered
                },
                selection_key: DiagnosticSelectionKey {
                    repair_route_rank: 128,
                    causal_rank: 128,
                    evidence_rank: 128,
                },
            });
        }
    }
    Ok(items)
}

pub(crate) fn diagnostic_canonical_id(
    diagnostic: &tower_lsp_server::ls_types::Diagnostic,
    document: &str,
    payload: &[u8],
) -> String {
    if let Some(data) = &diagnostic.data
        && let Some(obj) = data.as_object()
    {
        for key in [
            "diagnostic_id",
            "canonical_gap_id",
            "finding_id",
            "gap_id",
            "seam_id",
        ] {
            if let Some(id) = obj
                .get(key)
                .and_then(|value| value.as_str())
                .filter(|value| !value.trim().is_empty())
            {
                return id.to_string();
            }
        }
    }
    let line = diagnostic.range.start.line;
    let character = diagnostic.range.start.character;
    let payload_hash = Sha256::digest(payload);
    format!("location:{document}:{line}:{character}:{payload_hash:x}")
}

fn diagnostic_is_actionable(diagnostic: &tower_lsp_server::ls_types::Diagnostic) -> bool {
    let Some(data) = diagnostic.data.as_ref() else {
        return false;
    };

    // Gap diagnostics are emitted only after the producer has validated the
    // LSP projection route. The budget still needs the producer's semantic
    // actionability predicate, rather than treating identity as eligibility.
    if data.get("source").and_then(|value| value.as_str()) == Some("gap_decision_ledger") {
        return data.get("gap_state").and_then(|value| value.as_str()) == Some("actionable")
            && data.get("repairability").and_then(|value| value.as_str()) == Some("repairable");
    }

    // Classified seams publish their producer-owned headline decision. Preview
    // findings publish the shared validator's packet decision. These signals
    // are already settled before this delivery projection runs.
    if let Some(eligible) = data
        .get("headline_eligible")
        .and_then(|value| value.as_bool())
    {
        return eligible;
    }
    data.get("preview_actionability")
        .and_then(|value| value.get("repair_packet_ready"))
        .and_then(|value| value.as_bool())
        .or_else(|| {
            data.get("delivery_eligible")
                .and_then(|value| value.as_bool())
        })
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(
        id: &str,
        document: &str,
        key: DiagnosticSelectionKey,
        payload_bytes: usize,
    ) -> DiagnosticBudgetItem {
        DiagnosticBudgetItem {
            canonical_id: id.to_string(),
            document: document.to_string(),
            payload_bytes,
            inline_detail_bytes: 0,
            eligibility: DiagnosticBudgetEligibility::Actionable,
            selection_key: key,
        }
    }

    fn key(route: u8, causal: u8, evidence: u8) -> DiagnosticSelectionKey {
        DiagnosticSelectionKey {
            repair_route_rank: route,
            causal_rank: causal,
            evidence_rank: evidence,
        }
    }

    #[test]
    fn diagnostic_bridge_preserves_identity_and_actionability() -> Result<(), String> {
        let uri = "file:///workspace/src/lib.rs"
            .parse::<tower_lsp_server::ls_types::Uri>()
            .map_err(|err| format!("parse test URI: {err}"))?;
        let first = tower_lsp_server::ls_types::Diagnostic {
            data: Some(serde_json::json!({
                "diagnostic_id": "finding:first",
                "source": "gap_decision_ledger",
                "canonical_gap_id": "gap:first",
                "gap_state": "baseline_only",
                "repairability": "no_action",
            })),
            ..Default::default()
        };
        let second = tower_lsp_server::ls_types::Diagnostic {
            data: Some(serde_json::json!({
                "diagnostic_id": "finding:second",
                "source": "gap_decision_ledger",
                "canonical_gap_id": "gap:second",
                "gap_state": "actionable",
                "repairability": "repairable",
            })),
            ..Default::default()
        };
        let diagnostics = std::collections::BTreeMap::from([(uri, vec![first, second])]);

        let items = build_budget_items_from_diagnostics(&diagnostics)
            .map_err(|err| format!("build budget items: {err}"))?;
        if items.len() != 2 {
            return Err(format!("expected two budget items, got {items:?}"));
        }
        if items[0].canonical_id != "finding:first" || items[1].canonical_id != "finding:second" {
            return Err(format!("producer identities were not preserved: {items:?}"));
        }
        if items[0].eligibility != DiagnosticBudgetEligibility::ProfileFiltered {
            return Err("non-actionable canonical gap consumed the budget".to_string());
        }
        if items[1].eligibility != DiagnosticBudgetEligibility::Actionable {
            return Err("actionable canonical gap was profile-filtered".to_string());
        }
        Ok(())
    }

    #[test]
    fn diagnostic_bridge_uses_explicit_seam_and_preview_eligibility() -> Result<(), String> {
        let uri = "file:///workspace/src/lib.rs"
            .parse::<tower_lsp_server::ls_types::Uri>()
            .map_err(|err| format!("parse test URI: {err}"))?;
        let seam = tower_lsp_server::ls_types::Diagnostic {
            data: Some(serde_json::json!({
                "diagnostic_id": "seam:headline",
                "seam_id": "seam:headline",
                "headline_eligible": true,
            })),
            ..Default::default()
        };
        let preview = tower_lsp_server::ls_types::Diagnostic {
            data: Some(serde_json::json!({
                "diagnostic_id": "finding:preview",
                "finding_id": "finding:preview",
                "canonical_gap_id": "gap:preview",
                "preview_actionability": {"repair_packet_ready": true},
            })),
            ..Default::default()
        };
        let diagnostics = std::collections::BTreeMap::from([(uri, vec![seam, preview])]);

        let items = build_budget_items_from_diagnostics(&diagnostics)
            .map_err(|err| format!("build budget items: {err}"))?;
        if !items
            .iter()
            .all(|item| item.eligibility == DiagnosticBudgetEligibility::Actionable)
        {
            return Err(format!(
                "explicit producer eligibility was not preserved: {items:?}"
            ));
        }
        Ok(())
    }

    #[test]
    fn legacy_fallback_identity_is_stable_when_diagnostics_are_reordered() -> Result<(), String> {
        let uri = "file:///workspace/src/lib.rs"
            .parse::<tower_lsp_server::ls_types::Uri>()
            .map_err(|err| format!("parse test URI: {err}"))?;
        let first = tower_lsp_server::ls_types::Diagnostic {
            message: "first legacy diagnostic".to_string(),
            ..Default::default()
        };
        let second = tower_lsp_server::ls_types::Diagnostic {
            message: "second legacy diagnostic".to_string(),
            ..Default::default()
        };
        let forward =
            std::collections::BTreeMap::from([(uri.clone(), vec![first.clone(), second.clone()])]);
        let reversed = std::collections::BTreeMap::from([(uri, vec![second, first])]);
        let forward_ids = build_budget_items_from_diagnostics(&forward)
            .map_err(|err| format!("build forward budget items: {err}"))?
            .into_iter()
            .map(|item| item.canonical_id)
            .collect::<std::collections::BTreeSet<_>>();
        let reversed_ids = build_budget_items_from_diagnostics(&reversed)
            .map_err(|err| format!("build reversed budget items: {err}"))?
            .into_iter()
            .map(|item| item.canonical_id)
            .collect::<std::collections::BTreeSet<_>>();
        if forward_ids != reversed_ids {
            return Err(format!(
                "legacy fallback identities changed after reordering: forward={forward_ids:?}, reversed={reversed_ids:?}"
            ));
        }
        Ok(())
    }

    #[test]
    fn diagnostic_bridge_fails_closed_for_missing_or_negative_eligibility() -> Result<(), String> {
        let uri = "file:///workspace/src/lib.rs"
            .parse::<tower_lsp_server::ls_types::Uri>()
            .map_err(|err| format!("parse test URI: {err}"))?;
        let no_metadata = tower_lsp_server::ls_types::Diagnostic::default();
        let non_headline_seam = tower_lsp_server::ls_types::Diagnostic {
            data: Some(serde_json::json!({
                "diagnostic_id": "seam:advisory",
                "headline_eligible": false,
            })),
            ..Default::default()
        };
        let incomplete_preview = tower_lsp_server::ls_types::Diagnostic {
            data: Some(serde_json::json!({
                "diagnostic_id": "finding:incomplete-preview",
                "preview_actionability": {"repair_packet_ready": false},
            })),
            ..Default::default()
        };
        let diagnostics = std::collections::BTreeMap::from([(
            uri,
            vec![no_metadata, non_headline_seam, incomplete_preview],
        )]);

        let items = build_budget_items_from_diagnostics(&diagnostics)
            .map_err(|err| format!("build budget items: {err}"))?;
        if items.len() != 3
            || items
                .iter()
                .any(|item| item.eligibility != DiagnosticBudgetEligibility::ProfileFiltered)
        {
            return Err(format!(
                "missing or negative producer eligibility was over-credited: {items:?}"
            ));
        }
        if !items[0]
            .canonical_id
            .starts_with("location:file:///workspace/src/lib.rs:")
        {
            return Err(format!(
                "legacy diagnostic identity was not deterministic: {items:?}"
            ));
        }
        Ok(())
    }

    #[test]
    fn diagnostic_bridge_reads_ordinary_eligibility_after_family_authority() -> Result<(), String> {
        let uri = "file:///workspace/src/lib.rs"
            .parse::<tower_lsp_server::ls_types::Uri>()
            .map_err(|err| format!("parse test URI: {err}"))?;
        let ordinary = tower_lsp_server::ls_types::Diagnostic {
            data: Some(serde_json::json!({
                "diagnostic_id": "finding:ordinary",
                "delivery_eligible": true,
            })),
            ..Default::default()
        };
        let family_override = tower_lsp_server::ls_types::Diagnostic {
            data: Some(serde_json::json!({
                "diagnostic_id": "finding:family-override",
                "headline_eligible": false,
                "delivery_eligible": true,
            })),
            ..Default::default()
        };
        let diagnostics =
            std::collections::BTreeMap::from([(uri, vec![ordinary, family_override])]);

        let items = build_budget_items_from_diagnostics(&diagnostics)
            .map_err(|err| format!("build budget items: {err}"))?;
        if items[0].eligibility != DiagnosticBudgetEligibility::Actionable
            || items[1].eligibility != DiagnosticBudgetEligibility::ProfileFiltered
        {
            return Err(format!(
                "ordinary eligibility or family precedence regressed: {items:?}"
            ));
        }
        Ok(())
    }

    #[test]
    fn selection_is_deterministic_and_uses_evidence_owned_order() -> Result<(), String> {
        let budget = DiagnosticBudget {
            max_items_per_document: 2,
            max_items_per_workspace_response: 2,
            max_serialized_bytes: 100,
            max_inline_detail_bytes: 10,
        };
        let first = evaluate_diagnostic_budget(
            [
                item("gap:b", "src/b.rs", key(1, 0, 0), 10),
                item("gap:a", "src/a.rs", key(0, 1, 0), 10),
                item("gap:c", "src/c.rs", key(0, 1, 0), 10),
            ],
            &budget,
            "snapshot:s1:profile:actionable",
            "evidence:e1",
        )
        .map_err(|error| error.to_string())?;
        let second = evaluate_diagnostic_budget(
            [
                item("gap:c", "src/c.rs", key(0, 1, 0), 10),
                item("gap:a", "src/a.rs", key(0, 1, 0), 10),
                item("gap:b", "src/b.rs", key(1, 0, 0), 10),
            ],
            &budget,
            "snapshot:s1:profile:actionable",
            "evidence:e1",
        )
        .map_err(|error| error.to_string())?;
        assert_eq!(first, second);
        assert_eq!(
            first.selected_ids().collect::<Vec<_>>(),
            vec!["gap:a", "gap:c"]
        );
        assert_eq!(first.omitted_ids().collect::<Vec<_>>(), vec!["gap:b"]);
        assert!(first.overflowed);
        assert!(
            first
                .overflow_reasons
                .contains(&DiagnosticOverflowReason::WorkspaceItemLimit)
        );
        Ok(())
    }

    #[test]
    fn document_and_byte_limits_record_each_omission() -> Result<(), String> {
        let budget = DiagnosticBudget {
            max_items_per_document: 1,
            max_items_per_workspace_response: 10,
            max_serialized_bytes: 15,
            max_inline_detail_bytes: 10,
        };
        let result = evaluate_diagnostic_budget(
            [
                item("gap:a", "src/a.rs", key(0, 0, 0), 10),
                item("gap:b", "src/a.rs", key(0, 0, 1), 10),
                item("gap:c", "src/b.rs", key(0, 0, 2), 10),
            ],
            &budget,
            "snapshot:s1:profile:actionable",
            "evidence:e1",
        )
        .map_err(|error| error.to_string())?;
        assert_eq!(result.selected_ids().collect::<Vec<_>>(), vec!["gap:a"]);
        assert_eq!(
            result.omitted,
            vec![
                OmittedDiagnosticItem {
                    canonical_id: "gap:b".to_string(),
                    reason: OmittedDiagnosticReason::DocumentItemLimit,
                },
                OmittedDiagnosticItem {
                    canonical_id: "gap:c".to_string(),
                    reason: OmittedDiagnosticReason::SerializedByteLimit,
                },
            ]
        );
        assert!(
            result
                .overflow_reasons
                .contains(&DiagnosticOverflowReason::DocumentItemLimit)
        );
        assert!(
            result
                .overflow_reasons
                .contains(&DiagnosticOverflowReason::SerializedByteLimit)
        );
        Ok(())
    }

    #[test]
    fn first_item_over_byte_budget_stays_selected_and_marks_overflow() -> Result<(), String> {
        let result = evaluate_diagnostic_budget(
            [item("gap:oversized", "src/a.rs", key(0, 0, 0), 10)],
            &DiagnosticBudget {
                max_serialized_bytes: 5,
                ..DiagnosticBudget::default()
            },
            "snapshot:s1:profile:actionable",
            "evidence:e1",
        )
        .map_err(|error| error.to_string())?;
        assert_eq!(
            result.selected_ids().collect::<Vec<_>>(),
            vec!["gap:oversized"]
        );
        assert!(result.overflowed);
        assert!(
            result
                .overflow_reasons
                .contains(&DiagnosticOverflowReason::SerializedByteLimit)
        );
        Ok(())
    }

    #[test]
    fn oversized_inline_detail_stays_selected_with_explicit_omission() -> Result<(), String> {
        let mut candidate = item("gap:a", "src/a.rs", key(0, 0, 0), 10);
        candidate.inline_detail_bytes = 100;
        let result = evaluate_diagnostic_budget(
            [candidate],
            &DiagnosticBudget {
                max_inline_detail_bytes: 10,
                ..DiagnosticBudget::default()
            },
            "snapshot:s1:profile:actionable",
            "evidence:e1",
        )
        .map_err(|error| error.to_string())?;
        assert_eq!(result.selected.len(), 1);
        assert!(result.selected[0].inline_detail_omitted);
        assert!(
            result
                .overflow_reasons
                .contains(&DiagnosticOverflowReason::InlineDetailLimit)
        );
        Ok(())
    }

    #[test]
    fn profile_filtered_items_never_consume_the_budget() -> Result<(), String> {
        let mut filtered = item("gap:filtered", "src/a.rs", key(0, 0, 0), 10);
        filtered.eligibility = DiagnosticBudgetEligibility::ProfileFiltered;
        let result = evaluate_diagnostic_budget(
            [
                filtered,
                item("gap:actionable", "src/b.rs", key(1, 1, 1), 10),
            ],
            &DiagnosticBudget {
                max_items_per_document: 1,
                max_items_per_workspace_response: 1,
                max_serialized_bytes: 10,
                max_inline_detail_bytes: 10,
            },
            "snapshot:s1:profile:actionable",
            "evidence:e1",
        )
        .map_err(|error| error.to_string())?;
        assert_eq!(
            result.selected_ids().collect::<Vec<_>>(),
            vec!["gap:actionable"]
        );
        assert!(!result.overflowed);
        assert_eq!(result.total_canonical_items, 2);
        assert_eq!(result.eligible_items, 1);
        Ok(())
    }

    #[test]
    fn invalid_budget_is_fail_closed() {
        let error = DiagnosticBudget {
            max_items_per_document: 0,
            ..DiagnosticBudget::default()
        }
        .validate();
        assert_eq!(
            error,
            Err(DiagnosticBudgetError::ZeroLimit("max_items_per_document"))
        );
    }
}
