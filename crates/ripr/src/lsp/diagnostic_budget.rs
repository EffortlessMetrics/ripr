//! Deterministic passive diagnostic delivery budgeting.
//!
//! This module is a presentation projection over already-canonical evidence.
//! It does not discover, merge, rank, or otherwise reinterpret findings. The
//! producer supplies eligibility and evidence-owned ordering ranks; this module
//! only applies finite delivery limits and records every omitted identity.

use std::collections::BTreeSet;

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
