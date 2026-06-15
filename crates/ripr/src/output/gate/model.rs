use super::super::gap_decision_ledger::GapRepairRoute;
use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum GateMode {
    VisibleOnly,
    Acknowledgeable,
    BaselineCheck,
    CalibratedGate,
}

impl GateMode {
    pub(crate) fn parse(value: &str) -> Result<Self, String> {
        match value {
            "visible-only" => Ok(Self::VisibleOnly),
            "acknowledgeable" => Ok(Self::Acknowledgeable),
            "baseline-check" => Ok(Self::BaselineCheck),
            "calibrated-gate" => Ok(Self::CalibratedGate),
            other => Err(format!("unknown gate mode `{other}`")),
        }
    }

    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::VisibleOnly => "visible-only",
            Self::Acknowledgeable => "acknowledgeable",
            Self::BaselineCheck => "baseline-check",
            Self::CalibratedGate => "calibrated-gate",
        }
    }

    pub(super) fn requires_baseline(self) -> bool {
        matches!(self, Self::BaselineCheck | Self::CalibratedGate)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct GateEvaluateInput {
    pub(crate) root: PathBuf,
    pub(crate) repo_exposure: Option<PathBuf>,
    pub(crate) pr_guidance: Option<PathBuf>,
    pub(crate) gap_ledger: Option<PathBuf>,
    pub(crate) sarif_policy: Option<PathBuf>,
    pub(crate) labels_json: Option<PathBuf>,
    pub(crate) labels: Vec<String>,
    pub(crate) agent_verify: Option<PathBuf>,
    pub(crate) agent_receipt: Option<PathBuf>,
    pub(crate) recommendation_calibration: Option<PathBuf>,
    pub(crate) mutation_calibration: Option<PathBuf>,
    pub(crate) baseline: Option<PathBuf>,
    pub(crate) mode: GateMode,
    pub(crate) acknowledgement_labels: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct GateDecisionReport {
    pub(super) status: String,
    pub(super) mode: GateMode,
    pub(super) root: String,
    pub(super) inputs: GateDecisionInputs,
    pub(super) policy: GatePolicy,
    pub(super) summary: GateSummary,
    pub(super) new_unsuppressed: NewUnsuppressed,
    pub(super) decisions: Vec<GateDecision>,
    pub(super) warnings: Vec<String>,
    pub(super) config_errors: Vec<String>,
}

/// Canonical downstream-thresholding receipt field.
///
/// `count` is the number of policy-eligible, non-suppressed, non-acknowledged,
/// non-not_applicable decisions (i.e. `decision ∈ {"blocking","advisory"}`)
/// after filtering by `candidate_class_is_policy_eligible`.  In baseline mode
/// only decisions where `is_baseline_new` is true are counted.
///
/// `basis` is `Some("diff")` for diff-scoped runs, `Some("baseline")` for
/// baseline-aware runs where a baseline was actually read, or `None` when
/// `config_errors` is non-empty (analysis did not run — fail-closed sentinel).
/// When `basis` is `None`, `count` is `0` and `reason` discloses why.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct NewUnsuppressed {
    pub(crate) basis: Option<String>,
    pub(crate) count: u64,
    pub(crate) reason: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct GateDecisionInputs {
    pub(super) repo_exposure: Option<String>,
    pub(super) pr_guidance: Option<String>,
    pub(super) gap_ledger: Option<String>,
    pub(super) sarif_policy: Option<String>,
    pub(super) labels_json: Option<String>,
    pub(super) labels: Vec<String>,
    pub(super) agent_verify: Option<String>,
    pub(super) agent_receipt: Option<String>,
    pub(super) recommendation_calibration: Option<String>,
    pub(super) mutation_calibration: Option<String>,
    pub(super) baseline: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct GatePolicy {
    pub(super) mode: GateMode,
    pub(super) threshold: String,
    pub(super) acknowledgement_labels: Vec<String>,
    pub(super) default_workflow_posture: String,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(super) struct GateSummary {
    pub(super) evaluated: usize,
    pub(super) blocking: usize,
    pub(super) acknowledged: usize,
    pub(super) advisory: usize,
    pub(super) suppressed: usize,
    pub(super) not_applicable: usize,
    pub(super) unknown_confidence: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct GateDecision {
    pub(super) id: String,
    pub(super) source: String,
    pub(super) decision: String,
    pub(super) gate_reason: String,
    pub(super) gap_id: Option<String>,
    pub(super) gap_kind: Option<String>,
    pub(super) canonical_gap_id: Option<String>,
    pub(super) seam_id: Option<String>,
    pub(super) source_id: String,
    pub(super) static_class: Option<String>,
    pub(super) severity: Option<String>,
    pub(super) placement: GatePlacement,
    pub(super) policy: GateDecisionPolicy,
    pub(super) evidence: GateEvidence,
    /// Whether the candidate was absent from the baseline at decision time.
    /// Always `true` for diff-scoped modes (no baseline).
    /// Used when computing `new_unsuppressed.count` in baseline mode.
    pub(super) is_baseline_new: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct GatePlacement {
    pub(super) path: Option<String>,
    pub(super) line: Option<u64>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct GateDecisionPolicy {
    pub(super) mode: GateMode,
    pub(super) threshold: String,
    pub(super) acknowledgement_label: Option<String>,
    pub(super) baseline_identity: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct GateEvidence {
    pub(super) missing_discriminator: Option<String>,
    pub(super) assertion_shape: Option<String>,
    pub(super) candidate_values: Vec<String>,
    pub(super) recommended_test: Option<String>,
    pub(super) repair_route: Option<GapRepairRoute>,
    pub(super) verification_commands: Vec<String>,
    pub(super) nearby_test_changed: bool,
    pub(super) suppressed: bool,
    pub(super) configured_off: bool,
    pub(super) recommendation_calibration: CalibrationEvidence,
    pub(super) mutation_calibration: CalibrationEvidence,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct CalibrationEvidence {
    pub(super) available: bool,
    pub(super) outcome: Option<String>,
    pub(super) confidence_effect: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct GateCandidate {
    pub(super) source: String,
    pub(super) source_id: String,
    pub(super) gap_id: Option<String>,
    pub(super) gap_kind: Option<String>,
    pub(super) canonical_gap_id: Option<String>,
    pub(super) seam_id: Option<String>,
    pub(super) static_class: Option<String>,
    pub(super) severity: Option<String>,
    pub(super) placement: GatePlacement,
    pub(super) missing_discriminator: Option<String>,
    pub(super) assertion_shape: Option<String>,
    pub(super) candidate_values: Vec<String>,
    pub(super) recommended_test: Option<String>,
    pub(super) repair_route: Option<GapRepairRoute>,
    pub(super) verification_commands: Vec<String>,
    pub(super) nearby_test_changed: bool,
    pub(super) suppressed: bool,
    pub(super) configured_off: bool,
    pub(super) suppression_reason: Option<String>,
    /// Producer-assigned reason why this item was placed in `summary_only` instead
    /// of an inline comment slot.  Closed vocabulary: `inline_comment_cap_reached`,
    /// `no_safe_changed_line_placement`, `navigation_only_cross_language_target`.
    pub(super) summary_reason: Option<String>,
    pub(super) gap_ledger_gate_candidate: bool,
    pub(super) gap_ledger_gate_reason: Option<String>,
    pub(super) gap_ledger_safe_gate_predicate: bool,
}

#[derive(Clone, Copy, Debug)]
pub(super) struct GateReasonContext<'a> {
    pub(super) mode: GateMode,
    pub(super) decision: &'a str,
    pub(super) eligible: bool,
    pub(super) is_baseline_new: bool,
    pub(super) recommendation_calibration: &'a CalibrationEvidence,
    pub(super) mutation_calibration: &'a CalibrationEvidence,
    pub(super) acknowledgement_label: Option<&'a str>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(super) struct CalibrationIndex {
    pub(super) by_source_id: BTreeMap<String, CalibrationEvidence>,
    pub(super) by_seam_id: BTreeMap<String, CalibrationEvidence>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(super) struct BaselineIndex {
    pub(super) identities: BTreeSet<String>,
}
