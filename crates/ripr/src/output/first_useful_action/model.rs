use super::current_evidence_strength_for_selection;
use serde::Serialize;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct FirstUsefulActionInput {
    pub(crate) root: String,
    pub(crate) generated_at: String,
    pub(crate) pr_guidance_path: Option<String>,
    pub(crate) assistant_proof_path: Option<String>,
    pub(crate) gap_ledger_path: Option<String>,
    pub(crate) ledger_path: Option<String>,
    pub(crate) baseline_delta_path: Option<String>,
    pub(crate) receipt_path: Option<String>,
    pub(crate) gate_decision_path: Option<String>,
    pub(crate) coverage_frontier_path: Option<String>,
    pub(crate) editor_context_path: Option<String>,
    pub(crate) pr_guidance_json: Option<Result<String, String>>,
    pub(crate) assistant_proof_json: Option<Result<String, String>>,
    pub(crate) gap_ledger_json: Option<Result<String, String>>,
    pub(crate) ledger_json: Option<Result<String, String>>,
    pub(crate) baseline_delta_json: Option<Result<String, String>>,
    pub(crate) receipt_json: Option<Result<String, String>>,
    pub(crate) gate_decision_json: Option<Result<String, String>>,
    pub(crate) coverage_frontier_json: Option<Result<String, String>>,
    pub(crate) editor_context_json: Option<Result<String, String>>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct FirstUsefulActionReport {
    pub(super) status: String,
    pub(super) audience: String,
    pub(super) action_kind: String,
    pub(super) root: String,
    pub(super) generated_at: String,
    pub(super) inputs: ActionInputs,
    pub(super) selected: Option<ActionSelected>,
    pub(super) title: String,
    pub(super) why: String,
    pub(super) why_first: Vec<String>,
    pub(super) target: Option<ActionTarget>,
    pub(super) commands: ActionCommands,
    pub(super) evidence: ActionEvidence,
    pub(super) fallback: Option<ActionFallback>,
    pub(super) warnings: Vec<String>,
    pub(super) limits: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub(super) struct ActionInputs {
    pub(super) pr_guidance: Option<String>,
    pub(super) assistant_proof: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) gap_ledger: Option<String>,
    pub(super) ledger: Option<String>,
    pub(super) baseline_delta: Option<String>,
    pub(super) receipt: Option<String>,
    pub(super) gate_decision: Option<String>,
    pub(super) coverage_frontier: Option<String>,
    pub(super) editor_context: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub(super) struct ActionSelected {
    pub(super) source: String,
    pub(super) source_artifact: String,
    pub(super) seam_id: Option<String>,
    pub(super) seam_kind: Option<String>,
    pub(super) path: Option<String>,
    pub(super) line: Option<u64>,
    pub(super) classification: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) current_evidence_strength: Option<String>,
    pub(super) missing_discriminator: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) gap_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) canonical_gap_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) repair_route: Option<String>,
}

impl ActionSelected {
    pub(super) fn with_inferred_current_evidence_strength(mut self) -> Self {
        if self.current_evidence_strength.is_none() {
            self.current_evidence_strength = current_evidence_strength_for_selection(
                self.repair_route.as_deref(),
                self.classification.as_deref(),
                self.seam_kind.as_deref(),
            );
        }
        self
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub(super) struct ActionTarget {
    pub(super) file: Option<String>,
    pub(super) related_test: Option<String>,
    pub(super) suggested_test_name: Option<String>,
    pub(super) suggested_assertion: Option<String>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
pub(super) struct ActionCommands {
    pub(super) context_packet: Option<String>,
    pub(super) after_snapshot: Option<String>,
    pub(super) verify: Option<String>,
    pub(super) receipt: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) assistant_proof: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) status: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub(super) struct ActionEvidence {
    pub(super) pr_guidance: Option<String>,
    pub(super) assistant_proof: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) gap_ledger: Option<String>,
    pub(super) receipt: Option<String>,
    pub(super) ledger: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) baseline_delta: Option<String>,
    pub(super) static_movement: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub(super) struct ActionFallback {
    pub(super) kind: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) summary: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) missing: Option<String>,
}
