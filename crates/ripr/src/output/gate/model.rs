use super::super::gap_decision_ledger::GapRepairRoute;
use crate::domain::{CanonicalDelta, DeltaAttribution};
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
            other => Err(format!(
                "unknown gate mode `{other}`; expected `visible-only`, `acknowledgeable`, `baseline-check`, or `calibrated-gate`"
            )),
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
    /// Optional `--exception-policy` TOML ledger (#1442). Relative paths
    /// resolve against `root`. Fail-closed: a missing or malformed ledger is
    /// a `config_error`.
    pub(crate) exception_policy: Option<PathBuf>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct GateDecisionReport {
    pub(super) status: String,
    pub(super) mode: GateMode,
    pub(super) root: String,
    pub(super) inputs: GateDecisionInputs,
    /// Subject identity of the evaluation (#5263): which build produced the
    /// decision and which input bytes it consumed, so a stale gate receipt is
    /// distinguishable from a fresh one by the artifact itself.
    pub(super) subject: GateSubject,
    pub(super) policy: GatePolicy,
    pub(super) summary: GateSummary,
    pub(super) new_unsuppressed: NewUnsuppressed,
    pub(super) decisions: Vec<GateDecision>,
    pub(super) warnings: Vec<String>,
    pub(super) config_errors: Vec<String>,
    pub(super) causal_delta: Option<super::causal::CausalDeltaAuthority>,
    pub(super) causal_projection: Option<crate::app::causal_projection::CausalDeltaArtifact>,
    /// Exception-ledger evaluation (#1442). `Some` only when the caller
    /// passed `--exception-policy`; absent otherwise so existing
    /// gate-decision consumers and goldens see identical output.
    pub(super) exception_policy: Option<super::exception_policy::ExceptionPolicyReport>,
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
    /// Present only when `--exception-policy` was supplied (#1442), keeping
    /// existing gate-decision JSON byte-identical without the flag.
    pub(super) exception_policy: Option<String>,
}

/// Subject identity of one gate evaluation (#5263). Every field is either
/// measured here (build identity, input content hashes) or copied verbatim
/// from the input document's own producer receipt — never inferred and never
/// re-resolved, so the artifact records what this evaluation actually saw.
///
/// There is deliberately no timestamp: two identical evaluations stay
/// byte-identical, and staleness is carried by the content hashes and the
/// producer SHAs instead. There is deliberately no absolute checkout path:
/// the caller-relative `root` field already names the evaluated root, and
/// portable identity fields carry no machine-specific spelling.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct GateSubject {
    /// Build identity of the writing binary (version plus commit or source
    /// digest; `build_identity::cache_identity`), the same analyzer stamp
    /// `check --write-artifact` records.
    pub(super) analyzer_version: String,
    /// One entry per consumed input, keyed by input name — every
    /// CLI-supplied input that can change the decision, not only the
    /// candidate sources (review round 1, #5263).
    pub(super) inputs: BTreeMap<String, GateSubjectInput>,
    /// `sha256` over the caller-supplied `--labels` strings (joined with NUL
    /// separators), present only when labels were supplied on the command
    /// line rather than through `--labels-json`.
    pub(super) labels_sha256: Option<String>,
}

/// Identity of one consumed gate input document (#5263).
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct GateSubjectInput {
    /// `sha256:<hex>` of the exact bytes the evaluation consumed; `None` only
    /// when the bytes could not be read for hashing (the read failure itself
    /// already surfaces as a `config_error` or warning).
    pub(super) content_hash: Option<String>,
    /// The input document's own producer receipt, copied verbatim when the
    /// producer recorded one (`run_receipt` on a review-comments guidance
    /// document): the resolved base/head SHAs and root identity that producer
    /// evaluated, so a `pr-ledger`'s asserted base/head can be cross-checked
    /// against what the gate actually consumed.
    pub(super) producer_subject: Option<GateProducerSubject>,
}

/// The producer identity carried by an input document's `run_receipt`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct GateProducerSubject {
    pub(super) root_identity: Option<String>,
    pub(super) base_sha: String,
    pub(super) head_sha: String,
    pub(super) reusable_cache_identity: Option<String>,
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

/// Decision-payload value recorded when a baseline match succeeded only via
/// the legacy `path:line:static_class` fallback selector (issue #1934,
/// RIPR-SPEC-0014 § Baseline Comparison).
pub(super) const BASELINE_MATCH_KIND_LEGACY_PATH_LINE_CLASS: &str = "legacy_path_line_class";

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
    pub(super) gap_state: Option<String>,
    pub(super) source_id: String,
    pub(super) static_class: Option<String>,
    pub(super) severity: Option<String>,
    pub(super) placement: GatePlacement,
    pub(super) policy: GateDecisionPolicy,
    pub(super) evidence: GateEvidence,
    pub(super) repair_route: GateRepairRoute,
    /// `false` only for PR-guidance summary items the review producer could
    /// not place on a changed line (`no_safe_changed_line_placement`): the
    /// seam's own line and owner span sit outside the diff as far as the
    /// producer knows. Markdown then names the owner and behavior without
    /// calling them changed. Not serialized; JSON keeps its field names.
    pub(super) changed_line_anchored: bool,
    /// Whether the candidate was absent from the baseline at decision time.
    /// Always `true` for diff-scoped modes (no baseline).
    /// Used when computing `new_unsuppressed.count` in baseline mode.
    pub(super) is_baseline_new: bool,
    /// `Some("legacy_path_line_class")` when the baseline matched only via
    /// the legacy path/line/static_class fallback selector; `None` for
    /// canonical identity matches and for baseline-new candidates. Rendered
    /// only when `Some`, so non-fallback decisions stay byte-identical.
    pub(super) baseline_match_kind: Option<String>,
    pub(super) delta_attribution: Option<DeltaAttribution>,
    pub(super) causal_delta: Option<CanonicalDelta>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct GateRepairRoute {
    pub(super) canonical_gap_id: Option<String>,
    pub(super) seam_id: Option<String>,
    pub(super) classification: Option<String>,
    pub(super) changed_owner: Option<String>,
    pub(super) changed_behavior: Option<String>,
    pub(super) missing_discriminator: Option<String>,
    pub(super) repair_target: Option<GateRepairTarget>,
    pub(super) test_intent: Option<String>,
    /// The repair transaction's start (#3906). Present only when the
    /// upstream card carries it, which it does only past the fail-closed
    /// repair-packet flip; the gate never derives it.
    pub(super) repair_command: Option<String>,
    /// Optional producer-owned completeness step; never derived by the gate.
    pub(super) analysis_outcome_command: Option<String>,
    pub(super) verify_command: Option<String>,
    pub(super) receipt_command: Option<String>,
    pub(super) inspection_command: Option<String>,
    pub(super) authority_boundary: String,
    pub(super) limitation: Option<GateRepairRouteLimitation>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum GateRepairTarget {
    RelatedTest {
        name: String,
        file: String,
        line: u64,
    },
    ProductionCaller {
        owner: String,
        file: Option<String>,
        line: Option<u64>,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct GateRepairRouteLimitation {
    pub(super) kind: &'static str,
    pub(super) missing_fields: Vec<String>,
    pub(super) detail: &'static str,
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
    pub(super) gap_state: Option<String>,
    pub(super) static_class: Option<String>,
    pub(super) severity: Option<String>,
    pub(super) placement: GatePlacement,
    pub(super) missing_discriminator: Option<String>,
    pub(super) route_facts: GateRouteFacts,
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
    /// Producer-owned reason a review card with `gap_state=static_limitation`
    /// is not actionable (the card's `why_not_actionable`). `None` for any
    /// other card and for gap-ledger records.
    pub(super) why_not_actionable: Option<String>,
    pub(super) gap_ledger_gate_candidate: bool,
    pub(super) gap_ledger_gate_reason: Option<String>,
    pub(super) gap_ledger_safe_gate_predicate: bool,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(super) struct GateRouteFacts {
    pub(super) canonical_gap_id: Option<String>,
    pub(super) seam_id: Option<String>,
    pub(super) gap_state: Option<String>,
    pub(super) classification: Option<String>,
    pub(super) changed_owner: Option<String>,
    pub(super) changed_behavior: Option<String>,
    pub(super) missing_discriminator: Option<String>,
    pub(super) repair_target: Option<GateRepairTarget>,
    pub(super) test_intent: Option<String>,
    /// The repair transaction's start (#3906). Present only when the
    /// upstream card carries it, which it does only past the fail-closed
    /// repair-packet flip; the gate never derives it.
    pub(super) repair_command: Option<String>,
    pub(super) analysis_outcome_command: Option<String>,
    pub(super) verify_command: Option<String>,
    pub(super) receipt_command: Option<String>,
    pub(super) inspection_command: Option<String>,
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
