//! Matched RIPR intervention-study preregistration (RIPR-SPEC-0216 / #4649).
//!
//! This module owns the frozen protocol object and the study-law validator.
//! It does not execute agents, grade repairs, or emit an intervention-value
//! conclusion. JSON and Markdown projections live in `output`.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

/// Wire schema identity for a preregistered matched intervention study.
pub(crate) const RIPR_INTERVENTION_STUDY_SCHEMA_VERSION: &str = "ripr_intervention_study.v1";
pub(crate) const RIPR_INTERVENTION_STUDY_KIND: &str = "ripr_intervention_study";
pub(crate) const RIPR_INTERVENTION_STUDY_IMPLEMENTATION_STATE: &str = "preregistration_only";

const SHA256_PREFIX: &str = "sha256:";
const SHA256_HEX_LENGTH: usize = 64;
const GIT_SHA_LENGTH: usize = 40;

/// Stable rejection codes for the ten issue #4649 falsifiers plus digest/shape.
pub(crate) mod codes {
    pub(crate) const UNSUPPORTED_SCHEMA: &str = "unsupported_schema";
    pub(crate) const MISSING_FIELD: &str = "missing_field";
    pub(crate) const MALFORMED_IDENTITY: &str = "malformed_identity";
    pub(crate) const ASSIGNMENT_AFTER_OUTCOME: &str = "assignment_after_outcome";
    pub(crate) const UNEQUAL_CONDITION_BUDGETS: &str = "unequal_condition_budgets";
    pub(crate) const CONTROL_CAN_READ_RIPR_OUTPUTS: &str = "control_can_read_ripr_outputs";
    pub(crate) const ASSISTED_UNDECLARED_EXTRA_CONTEXT: &str = "assisted_undeclared_extra_context";
    pub(crate) const TASK_REPLACEMENT_AFTER_FAILURE: &str = "task_replacement_after_failure";
    pub(crate) const RETRY_ONLY_IN_WEAKER_CONDITION: &str = "retry_only_in_weaker_condition";
    pub(crate) const DROP_INVALID_FROM_DENOMINATOR: &str =
        "drop_timeouts_or_invalid_from_denominator";
    pub(crate) const STOPPING_ON_FAVORABLE_INTERIM: &str = "stopping_on_favorable_interim";
    pub(crate) const GRADER_OR_RUBRIC_ABSENT: &str = "grader_or_rubric_absent";
    pub(crate) const PROTOCOL_MUTATION_REQUIRES_NEW_STUDY_ID: &str =
        "protocol_mutation_requires_new_study_id";
    pub(crate) const COMPENSATING_OUTCOME_SCORE: &str = "compensating_outcome_score";
    pub(crate) const PROTOCOL_DIGEST_MISMATCH: &str = "protocol_digest_mismatch";
}

/// Typed failure from the preregistration validator.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct InterventionStudyError {
    pub code: &'static str,
    pub message: String,
}

impl fmt::Display for InterventionStudyError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}: {}", self.code, self.message)
    }
}

fn error(code: &'static str, message: impl Into<String>) -> InterventionStudyError {
    InterventionStudyError {
        code,
        message: message.into(),
    }
}

/// Frozen preregistration for one matched RIPR intervention study.
#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct RiprInterventionStudyV1 {
    pub schema_version: String,
    pub kind: String,
    pub implementation_state: String,
    pub study_id: String,
    pub protocol_version: String,
    pub protocol_digest: String,
    pub parent_issue: String,
    pub sequence: String,
    pub study_phase: StudyPhase,
    pub repository: RepositoryIdentity,
    pub task_series: TaskSeries,
    pub eligibility: EligibilityRules,
    pub strata: Vec<MatchedStratum>,
    pub tasks: Vec<StudyTask>,
    pub conditions: Vec<StudyCondition>,
    pub assignment: AssignmentLaw,
    pub shared_budget: ResourceBudget,
    pub condition_budget_overlays: BTreeMap<String, BudgetOverlay>,
    pub intervention_surface: InterventionSurface,
    pub leakage_controls: LeakageControls,
    pub attempt_policy: AttemptPolicy,
    pub outcome_axes: Vec<OutcomeAxis>,
    pub non_authoritative_summary: Option<NonAuthoritativeSummary>,
    pub adjudication: AdjudicationPlan,
    pub invalid_attempt_rules: InvalidAttemptRules,
    pub stopping_rule: StoppingRule,
    pub planned_analysis: PlannedAnalysis,
    pub claim_ceiling: ClaimCeiling,
    pub protocol_lock: ProtocolLock,
    pub non_claims: Vec<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum StudyPhase {
    Preregistered,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct RepositoryIdentity {
    pub repository_id: String,
    pub clone_url: String,
    pub analyzed_head_sha: String,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct TaskSeries {
    pub task_series_id: String,
    pub description: String,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct EligibilityRules {
    pub include: Vec<String>,
    pub exclude: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct MatchedStratum {
    pub stratum_id: String,
    pub behavior_family: String,
    pub existing_proof_state: String,
    pub route_completeness: String,
    pub difficulty_class: String,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct StudyTask {
    pub task_id: String,
    pub stratum_id: String,
    pub canonical_behavior: String,
    pub existing_proof_state: String,
    pub fixture_id: String,
}

#[derive(
    Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, serde::Serialize, serde::Deserialize,
)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ConditionId {
    Control,
    RiprAssisted,
}

impl ConditionId {
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::Control => "control",
            Self::RiprAssisted => "ripr_assisted",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct StudyCondition {
    pub condition_id: ConditionId,
    pub description: String,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct AssignmentLaw {
    pub selected_before_outcomes: bool,
    pub freeze_rule: AssignmentFreezeRule,
    pub may_change_after_outcome_or_grader_signal: bool,
    pub pairing: PairingLaw,
    pub pairs: Vec<AssignmentPair>,
    pub assignment_seed: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum AssignmentFreezeRule {
    BeforeFirstOutcomeOrGraderSignal,
    AfterOutcomeVisible,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum PairingLaw {
    MatchedStratumCounterbalanced,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct AssignmentPair {
    pub pair_id: String,
    pub task_id: String,
    pub first_condition: ConditionId,
    pub second_condition: ConditionId,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ResourceBudget {
    pub model_id: String,
    pub operator_profile: String,
    pub runtime_id: String,
    pub tool_ids: Vec<String>,
    pub wall_clock_ms: u64,
    pub token_budget: u64,
    pub retry_limit: u32,
    pub command_authority: String,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct BudgetOverlay {
    pub additional_wall_clock_ms: u64,
    pub additional_token_budget: u64,
    pub additional_retries: u32,
    pub additional_tools: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct InterventionSurface {
    pub control: ConditionEvidenceSurface,
    pub ripr_assisted: ConditionEvidenceSurface,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ConditionEvidenceSurface {
    pub allowed_ripr_receipts: Vec<String>,
    pub allowed_ripr_views: Vec<String>,
    pub allowed_ripr_commands: Vec<String>,
    pub may_read_ripr_outputs: bool,
    pub undeclared_repository_context: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct LeakageControls {
    pub prior_exposure: IsolationPolicy,
    pub branch_reuse: IsolationPolicy,
    pub cross_condition_artifacts: IsolationPolicy,
    pub shared_scratch_state: IsolationPolicy,
    pub operator_leakage: IsolationPolicy,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum IsolationPolicy {
    Forbidden,
    DiscloseAndExclude,
    IndependentOperators,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct AttemptPolicy {
    pub task_replacement_after_failure: TaskReplacementPolicy,
    pub retry_policy_equal_across_conditions: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum TaskReplacementPolicy {
    Forbidden,
    AllowedAfterDifficultFailure,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct OutcomeAxis {
    pub axis_id: String,
    pub description: String,
    pub compensating: bool,
    pub independently_observable: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct NonAuthoritativeSummary {
    pub name: String,
    pub authoritative: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct AdjudicationPlan {
    pub grader_identities: Vec<String>,
    pub rubric_id: String,
    pub rubric_version: String,
    pub blinded_to_condition: bool,
    pub min_independent_graders: u32,
    pub disagreement_policy: DisagreementPolicy,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum DisagreementPolicy {
    RetainPerAxisDoNotAverageCorrectness,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct InvalidAttemptRules {
    pub outcome_cells: Vec<OutcomeCell>,
    pub retain_timeouts: bool,
    pub retain_cancellations: bool,
    pub retain_invalid_attempts: bool,
    pub retain_missing_evidence: bool,
    pub retain_instrument_failures: bool,
    pub drop_timeouts: bool,
    pub drop_invalid_attempts: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum OutcomeCell {
    Pass,
    Fail,
    Partial,
    NotProven,
    Invalid,
    InstrumentFailure,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct StoppingRule {
    pub kind: StoppingKind,
    pub planned_pair_count: u32,
    pub may_depend_on_interim_estimate: bool,
    pub favorable_interim_stop: FavorableInterimStop,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum StoppingKind {
    FixedSample,
    StopOnFavorableInterim,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum FavorableInterimStop {
    Forbidden,
    Allowed,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct PlannedAnalysis {
    pub unit: String,
    pub compares: Vec<String>,
    pub reports_exact_numerators_and_denominators: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ClaimCeiling {
    pub bounded_to_task_series: bool,
    pub bounded_to_model_operator_profile: bool,
    pub bounded_to_intervention_form: bool,
    pub preregistration_proves_intervention_value: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ProtocolLock {
    pub immutable_after_first_attempt: bool,
    pub first_attempt_recorded: bool,
    pub mutation_without_new_study_id: MutationPolicy,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum MutationPolicy {
    Forbidden,
    Allowed,
}

impl RiprInterventionStudyV1 {
    /// Validate study laws for one protocol document. Digest matching against
    /// rendered JSON is owned by the output projection.
    pub(crate) fn validate(&self) -> Result<(), InterventionStudyError> {
        self.validate_identity()?;
        self.validate_tasks_and_assignment()?;
        self.validate_budgets_and_retries()?;
        self.validate_intervention_and_leakage()?;
        self.validate_attempts_and_denominator()?;
        self.validate_outcomes_and_adjudication()?;
        self.validate_stopping_and_claims()?;
        Ok(())
    }

    /// Reject a successor that mutates a started study without a new identity.
    pub(crate) fn validate_successor(
        &self,
        successor: &Self,
    ) -> Result<(), InterventionStudyError> {
        self.validate()?;
        successor.validate()?;
        if self.study_id != successor.study_id {
            return Ok(());
        }
        // The recorded-attempt lock is monotonic under one study identity:
        // clearing it would reopen the protocol for later same-ID mutation.
        if self.protocol_lock.first_attempt_recorded
            && !successor.protocol_lock.first_attempt_recorded
        {
            return Err(error(
                codes::PROTOCOL_MUTATION_REQUIRES_NEW_STUDY_ID,
                "a recorded first attempt cannot be cleared under the same study_id",
            ));
        }
        // Either side carrying a recorded attempt freezes the payload, so a
        // mutation cannot ride along with the transition that records the
        // first attempt.
        if (self.protocol_lock.first_attempt_recorded
            || successor.protocol_lock.first_attempt_recorded)
            && self.canonical_protocol_payload() != successor.canonical_protocol_payload()
        {
            return Err(error(
                codes::PROTOCOL_MUTATION_REQUIRES_NEW_STUDY_ID,
                "protocol mutation after the first attempt requires a new study_id",
            ));
        }
        Ok(())
    }

    /// Semantic payload used for digest and mutation identity.
    ///
    /// `protocol_digest` is derived from this payload. `first_attempt_recorded`
    /// is observed lock state, not a protocol amendment.
    pub(crate) fn canonical_protocol_payload(&self) -> Self {
        let mut payload = self.clone();
        payload.protocol_digest.clear();
        payload.protocol_lock.first_attempt_recorded = false;
        payload
    }

    fn validate_identity(&self) -> Result<(), InterventionStudyError> {
        if self.schema_version != RIPR_INTERVENTION_STUDY_SCHEMA_VERSION {
            return Err(error(
                codes::UNSUPPORTED_SCHEMA,
                format!("schema_version must be {RIPR_INTERVENTION_STUDY_SCHEMA_VERSION}"),
            ));
        }
        if self.kind != RIPR_INTERVENTION_STUDY_KIND {
            return Err(error(
                codes::UNSUPPORTED_SCHEMA,
                format!("kind must be {RIPR_INTERVENTION_STUDY_KIND}"),
            ));
        }
        if self.implementation_state != RIPR_INTERVENTION_STUDY_IMPLEMENTATION_STATE {
            return Err(error(
                codes::UNSUPPORTED_SCHEMA,
                format!(
                    "implementation_state must be {RIPR_INTERVENTION_STUDY_IMPLEMENTATION_STATE}"
                ),
            ));
        }
        if self.study_phase != StudyPhase::Preregistered {
            return Err(error(
                codes::UNSUPPORTED_SCHEMA,
                "study_phase must remain preregistered on this protocol document",
            ));
        }
        require_non_empty("study_id", &self.study_id)?;
        require_non_empty("protocol_version", &self.protocol_version)?;
        require_sha256("protocol_digest", &self.protocol_digest)?;
        require_non_empty("parent_issue", &self.parent_issue)?;
        require_non_empty("sequence", &self.sequence)?;
        require_non_empty("repository_id", &self.repository.repository_id)?;
        require_non_empty("clone_url", &self.repository.clone_url)?;
        require_git_sha("analyzed_head_sha", &self.repository.analyzed_head_sha)?;
        require_non_empty("task_series_id", &self.task_series.task_series_id)?;
        if self.non_claims.is_empty() {
            return Err(error(
                codes::MISSING_FIELD,
                "non_claims must name at least one excluded claim",
            ));
        }
        Ok(())
    }

    fn validate_tasks_and_assignment(&self) -> Result<(), InterventionStudyError> {
        if self.strata.is_empty() || self.tasks.is_empty() {
            return Err(error(
                codes::MISSING_FIELD,
                "matched strata and tasks must be selected before outcomes",
            ));
        }
        let stratum_ids: BTreeSet<&str> = self
            .strata
            .iter()
            .map(|stratum| stratum.stratum_id.as_str())
            .collect();
        if stratum_ids.len() != self.strata.len() {
            return Err(error(
                codes::MALFORMED_IDENTITY,
                "stratum_id values must be unique",
            ));
        }
        let mut task_ids = BTreeSet::new();
        for task in &self.tasks {
            require_non_empty("task_id", &task.task_id)?;
            if !task_ids.insert(task.task_id.as_str()) {
                return Err(error(
                    codes::MALFORMED_IDENTITY,
                    "task_id values must be unique",
                ));
            }
            if !stratum_ids.contains(task.stratum_id.as_str()) {
                return Err(error(
                    codes::MALFORMED_IDENTITY,
                    format!(
                        "task {} names unknown stratum {}",
                        task.task_id, task.stratum_id
                    ),
                ));
            }
        }
        if !self.assignment.selected_before_outcomes
            || self.assignment.may_change_after_outcome_or_grader_signal
            || self.assignment.freeze_rule != AssignmentFreezeRule::BeforeFirstOutcomeOrGraderSignal
        {
            return Err(error(
                codes::ASSIGNMENT_AFTER_OUTCOME,
                "assignment cannot be chosen or changed after an attempt outcome or grader signal is visible",
            ));
        }
        if self.assignment.pairs.len() != self.tasks.len() {
            return Err(error(
                codes::MISSING_FIELD,
                "every selected task must appear in exactly one assignment pair",
            ));
        }
        let mut assigned = BTreeSet::new();
        let mut first_conditions = BTreeSet::new();
        for pair in &self.assignment.pairs {
            require_non_empty("pair_id", &pair.pair_id)?;
            if !task_ids.contains(pair.task_id.as_str()) {
                return Err(error(
                    codes::MALFORMED_IDENTITY,
                    format!(
                        "assignment pair {} names unknown task {}",
                        pair.pair_id, pair.task_id
                    ),
                ));
            }
            if !assigned.insert(pair.task_id.as_str()) {
                return Err(error(
                    codes::MALFORMED_IDENTITY,
                    format!("task {} is assigned more than once", pair.task_id),
                ));
            }
            if pair.first_condition == pair.second_condition
                || pair.first_condition != ConditionId::Control
                    && pair.second_condition != ConditionId::Control
                || pair.first_condition != ConditionId::RiprAssisted
                    && pair.second_condition != ConditionId::RiprAssisted
            {
                return Err(error(
                    codes::MALFORMED_IDENTITY,
                    format!(
                        "pair {} must counterbalance control and ripr_assisted",
                        pair.pair_id
                    ),
                ));
            }
            first_conditions.insert(pair.first_condition);
        }
        if first_conditions.len() < 2 {
            return Err(error(
                codes::MALFORMED_IDENTITY,
                "matched pairs must counterbalance condition order",
            ));
        }
        Ok(())
    }

    fn validate_budgets_and_retries(&self) -> Result<(), InterventionStudyError> {
        require_non_empty("model_id", &self.shared_budget.model_id)?;
        require_non_empty("operator_profile", &self.shared_budget.operator_profile)?;
        require_non_empty("runtime_id", &self.shared_budget.runtime_id)?;
        if self.shared_budget.tool_ids.is_empty() {
            return Err(error(codes::MISSING_FIELD, "shared tool_ids must be named"));
        }
        let required = [
            ConditionId::Control.as_str(),
            ConditionId::RiprAssisted.as_str(),
        ];
        if self.conditions.len() != 2
            || required.iter().any(|id| {
                !self
                    .conditions
                    .iter()
                    .any(|condition| condition.condition_id.as_str() == *id)
            })
        {
            return Err(error(
                codes::MISSING_FIELD,
                "protocol must declare exactly control and ripr_assisted",
            ));
        }
        for condition in &self.conditions {
            let overlay = self
                .condition_budget_overlays
                .get(condition.condition_id.as_str())
                .ok_or_else(|| {
                    error(
                        codes::MISSING_FIELD,
                        format!(
                            "condition {} is missing a budget overlay",
                            condition.condition_id.as_str()
                        ),
                    )
                })?;
            if overlay.additional_wall_clock_ms != 0
                || overlay.additional_token_budget != 0
                || overlay.additional_retries != 0
                || !overlay.additional_tools.is_empty()
            {
                return Err(error(
                    codes::UNEQUAL_CONDITION_BUDGETS,
                    "control and ripr_assisted must share model, operator, runtime, tool, time, token, and retry budgets except for the declared intervention",
                ));
            }
        }
        if !self.attempt_policy.retry_policy_equal_across_conditions {
            return Err(error(
                codes::RETRY_ONLY_IN_WEAKER_CONDITION,
                "retries cannot be granted only in the weaker condition",
            ));
        }
        Ok(())
    }

    fn validate_intervention_and_leakage(&self) -> Result<(), InterventionStudyError> {
        let control = &self.intervention_surface.control;
        if control.may_read_ripr_outputs
            || !control.allowed_ripr_receipts.is_empty()
            || !control.allowed_ripr_views.is_empty()
            || !control.allowed_ripr_commands.is_empty()
        {
            return Err(error(
                codes::CONTROL_CAN_READ_RIPR_OUTPUTS,
                "control attempts must not read RIPR receipts, views, or commands",
            ));
        }
        let assisted = &self.intervention_surface.ripr_assisted;
        if assisted.allowed_ripr_receipts.is_empty()
            && assisted.allowed_ripr_views.is_empty()
            && assisted.allowed_ripr_commands.is_empty()
        {
            return Err(error(
                codes::MISSING_FIELD,
                "ripr_assisted must name exact receipts, views, or commands rather than summarizing use RIPR",
            ));
        }
        if !assisted.undeclared_repository_context.is_empty()
            || !control.undeclared_repository_context.is_empty()
        {
            return Err(error(
                codes::ASSISTED_UNDECLARED_EXTRA_CONTEXT,
                "a RIPR-assisted attempt cannot receive undeclared extra repository context",
            ));
        }
        Ok(())
    }

    fn validate_attempts_and_denominator(&self) -> Result<(), InterventionStudyError> {
        if self.attempt_policy.task_replacement_after_failure != TaskReplacementPolicy::Forbidden {
            return Err(error(
                codes::TASK_REPLACEMENT_AFTER_FAILURE,
                "tasks cannot be replaced after a difficult failure",
            ));
        }
        let rules = &self.invalid_attempt_rules;
        let required_cells = [
            OutcomeCell::Pass,
            OutcomeCell::Fail,
            OutcomeCell::Partial,
            OutcomeCell::NotProven,
            OutcomeCell::Invalid,
            OutcomeCell::InstrumentFailure,
        ];
        for cell in required_cells {
            if !rules.outcome_cells.contains(&cell) {
                return Err(error(
                    codes::MISSING_FIELD,
                    "invalid and incomplete attempts need typed denominator-preserving outcome cells",
                ));
            }
        }
        if rules.drop_timeouts
            || rules.drop_invalid_attempts
            || !rules.retain_timeouts
            || !rules.retain_cancellations
            || !rules.retain_invalid_attempts
            || !rules.retain_missing_evidence
            || !rules.retain_instrument_failures
        {
            return Err(error(
                codes::DROP_INVALID_FROM_DENOMINATOR,
                "timeouts, cancellations, invalid attempts, missing evidence, and instrument failures remain in the denominator",
            ));
        }
        Ok(())
    }

    fn validate_outcomes_and_adjudication(&self) -> Result<(), InterventionStudyError> {
        let required_axes = [
            "behavior_alignment",
            "discriminating_proof_quality",
            "change_scope_and_churn",
            "review_findings_and_repair_burden",
            "completion_terminal_state",
            "time_and_bounded_resource_use",
            "maintainability_readability",
            "false_confidence_or_unsupported_claim_events",
        ];
        let present: BTreeSet<&str> = self
            .outcome_axes
            .iter()
            .map(|axis| axis.axis_id.as_str())
            .collect();
        for axis_id in required_axes {
            if !present.contains(axis_id) {
                return Err(error(
                    codes::MISSING_FIELD,
                    format!("outcome axis {axis_id} must be preregistered"),
                ));
            }
        }
        for axis in &self.outcome_axes {
            if axis.compensating || !axis.independently_observable {
                return Err(error(
                    codes::COMPENSATING_OUTCOME_SCORE,
                    "outcome axes must remain non-compensating and independently observable",
                ));
            }
        }
        if let Some(summary) = &self.non_authoritative_summary
            && summary.authoritative
        {
            return Err(error(
                codes::COMPENSATING_OUTCOME_SCORE,
                "a combined score is allowed only when explicitly declared non-authoritative",
            ));
        }
        if self.adjudication.grader_identities.is_empty()
            || self.adjudication.rubric_id.trim().is_empty()
            || self.adjudication.rubric_version.trim().is_empty()
            || self.adjudication.min_independent_graders == 0
            || !self.adjudication.blinded_to_condition
        {
            return Err(error(
                codes::GRADER_OR_RUBRIC_ABSENT,
                "independent adjudication requires grader identity, a rubric version, and condition blinding",
            ));
        }
        Ok(())
    }

    fn validate_stopping_and_claims(&self) -> Result<(), InterventionStudyError> {
        if self.stopping_rule.kind != StoppingKind::FixedSample
            || self.stopping_rule.may_depend_on_interim_estimate
            || self.stopping_rule.favorable_interim_stop != FavorableInterimStop::Forbidden
        {
            return Err(error(
                codes::STOPPING_ON_FAVORABLE_INTERIM,
                "the stopping rule cannot depend on a favorable interim estimate",
            ));
        }
        // A count mismatch is a malformed fixed sample, not an interim stop;
        // runners and adjudicators key on the code, so keep them distinct.
        if self.stopping_rule.planned_pair_count == 0
            || u32::try_from(self.assignment.pairs.len()).ok()
                != Some(self.stopping_rule.planned_pair_count)
        {
            return Err(error(
                codes::MALFORMED_IDENTITY,
                "planned_pair_count must equal the number of preregistered assignment pairs",
            ));
        }
        if !self.protocol_lock.immutable_after_first_attempt
            || self.protocol_lock.mutation_without_new_study_id != MutationPolicy::Forbidden
        {
            return Err(error(
                codes::PROTOCOL_MUTATION_REQUIRES_NEW_STUDY_ID,
                "protocol mutation after the first attempt requires a new study_id",
            ));
        }
        if self.claim_ceiling.preregistration_proves_intervention_value
            || !self.claim_ceiling.bounded_to_task_series
            || !self.claim_ceiling.bounded_to_model_operator_profile
            || !self.claim_ceiling.bounded_to_intervention_form
            || !self
                .planned_analysis
                .reports_exact_numerators_and_denominators
        {
            return Err(error(
                codes::MISSING_FIELD,
                "public conclusions are bounded to the exact task series, model/operator profile, and intervention form",
            ));
        }
        Ok(())
    }
}

fn require_non_empty(field: &str, value: &str) -> Result<(), InterventionStudyError> {
    if value.trim().is_empty() {
        Err(error(
            codes::MISSING_FIELD,
            format!("{field} must be non-empty"),
        ))
    } else {
        Ok(())
    }
}

fn require_sha256(field: &str, value: &str) -> Result<(), InterventionStudyError> {
    let hex = value.strip_prefix(SHA256_PREFIX).ok_or_else(|| {
        error(
            codes::MALFORMED_IDENTITY,
            format!("{field} must use {SHA256_PREFIX}<64-hex>"),
        )
    })?;
    if hex.len() != SHA256_HEX_LENGTH
        || !hex.chars().all(|ch| ch.is_ascii_hexdigit())
        || hex != hex.to_ascii_lowercase()
    {
        return Err(error(
            codes::MALFORMED_IDENTITY,
            format!("{field} must be 64 lowercase hexadecimal characters"),
        ));
    }
    Ok(())
}

fn require_git_sha(field: &str, value: &str) -> Result<(), InterventionStudyError> {
    if value.len() != GIT_SHA_LENGTH
        || !value.chars().all(|ch| ch.is_ascii_hexdigit())
        || value != value.to_ascii_lowercase()
    {
        return Err(error(
            codes::MALFORMED_IDENTITY,
            format!("{field} must be a 40-character lowercase git SHA"),
        ));
    }
    Ok(())
}

/// Example IV01 preregistration used by tests and the committed fixture.
#[cfg(test)]
pub(crate) fn example_preregistered_study() -> RiprInterventionStudyV1 {
    let dummy_digest = format!("{SHA256_PREFIX}{}", "0".repeat(SHA256_HEX_LENGTH));
    RiprInterventionStudyV1 {
        schema_version: RIPR_INTERVENTION_STUDY_SCHEMA_VERSION.to_string(),
        kind: RIPR_INTERVENTION_STUDY_KIND.to_string(),
        implementation_state: RIPR_INTERVENTION_STUDY_IMPLEMENTATION_STATE.to_string(),
        study_id: "study:ripr-intervention:iv01:matched-rust-boundary".to_string(),
        protocol_version: "1".to_string(),
        protocol_digest: dummy_digest,
        parent_issue: "#3751".to_string(),
        sequence: "IV01".to_string(),
        study_phase: StudyPhase::Preregistered,
        repository: RepositoryIdentity {
            repository_id: "EffortlessMetrics/ripr-swarm".to_string(),
            clone_url: "https://github.com/EffortlessMetrics/ripr-swarm".to_string(),
            analyzed_head_sha: "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_string(),
        },
        task_series: TaskSeries {
            task_series_id: "series:rust-boundary-missing-discriminator".to_string(),
            description: "Matched ordinary Rust missing-discriminator repair tasks".to_string(),
        },
        eligibility: EligibilityRules {
            include: vec![
                "producer-owned Rust route with retained exact identity".to_string(),
                "test-only repair contract".to_string(),
            ],
            exclude: vec![
                "unsafe or unresolved target".to_string(),
                "production-code edit".to_string(),
            ],
        },
        strata: vec![MatchedStratum {
            stratum_id: "stratum:rust-boundary-missing-discriminator".to_string(),
            behavior_family: "predicate_boundary".to_string(),
            existing_proof_state: "missing_discriminator".to_string(),
            route_completeness: "complete_repair_route".to_string(),
            difficulty_class: "ordinary".to_string(),
        }],
        tasks: vec![
            StudyTask {
                task_id: "task:boundary-oracle-a".to_string(),
                stratum_id: "stratum:rust-boundary-missing-discriminator".to_string(),
                canonical_behavior: "equality boundary on parse_limit".to_string(),
                existing_proof_state: "missing_discriminator".to_string(),
                fixture_id: "fixtures/boundary_gap".to_string(),
            },
            StudyTask {
                task_id: "task:boundary-oracle-b".to_string(),
                stratum_id: "stratum:rust-boundary-missing-discriminator".to_string(),
                canonical_behavior: "inclusive upper-bound on retry_budget".to_string(),
                existing_proof_state: "missing_discriminator".to_string(),
                fixture_id: "fixtures/strong_boundary_oracle".to_string(),
            },
        ],
        conditions: vec![
            StudyCondition {
                condition_id: ConditionId::Control,
                description: "repository change plus ordinary source, tests, and tooling"
                    .to_string(),
            },
            StudyCondition {
                condition_id: ConditionId::RiprAssisted,
                description: "same task context plus the named RIPR evidence surface".to_string(),
            },
        ],
        assignment: AssignmentLaw {
            selected_before_outcomes: true,
            freeze_rule: AssignmentFreezeRule::BeforeFirstOutcomeOrGraderSignal,
            may_change_after_outcome_or_grader_signal: false,
            pairing: PairingLaw::MatchedStratumCounterbalanced,
            pairs: vec![
                AssignmentPair {
                    pair_id: "pair:boundary-oracle-a".to_string(),
                    task_id: "task:boundary-oracle-a".to_string(),
                    first_condition: ConditionId::Control,
                    second_condition: ConditionId::RiprAssisted,
                },
                AssignmentPair {
                    pair_id: "pair:boundary-oracle-b".to_string(),
                    task_id: "task:boundary-oracle-b".to_string(),
                    first_condition: ConditionId::RiprAssisted,
                    second_condition: ConditionId::Control,
                },
            ],
            assignment_seed: "preregistered:iv01:counterbalanced".to_string(),
        },
        shared_budget: ResourceBudget {
            model_id: "model:study-operator-profile-v1".to_string(),
            operator_profile: "operator:independent-study-agent".to_string(),
            runtime_id: "runtime:isolated-worktree".to_string(),
            tool_ids: vec!["cargo".to_string(), "rustc".to_string(), "git".to_string()],
            wall_clock_ms: 1_800_000,
            token_budget: 50_000,
            retry_limit: 0,
            command_authority: "test_only_repair".to_string(),
        },
        condition_budget_overlays: BTreeMap::from([
            (ConditionId::Control.as_str().to_string(), empty_overlay()),
            (
                ConditionId::RiprAssisted.as_str().to_string(),
                empty_overlay(),
            ),
        ]),
        intervention_surface: InterventionSurface {
            control: ConditionEvidenceSurface {
                allowed_ripr_receipts: Vec::new(),
                allowed_ripr_views: Vec::new(),
                allowed_ripr_commands: Vec::new(),
                may_read_ripr_outputs: false,
                undeclared_repository_context: Vec::new(),
            },
            ripr_assisted: ConditionEvidenceSurface {
                allowed_ripr_receipts: vec!["agent_receipt.v0.5".to_string()],
                allowed_ripr_views: vec!["agent_seam_packet.v0.4".to_string()],
                allowed_ripr_commands: vec![
                    "ripr check --format json".to_string(),
                    "ripr agent packet --json".to_string(),
                ],
                may_read_ripr_outputs: true,
                undeclared_repository_context: Vec::new(),
            },
        },
        leakage_controls: LeakageControls {
            prior_exposure: IsolationPolicy::DiscloseAndExclude,
            branch_reuse: IsolationPolicy::Forbidden,
            cross_condition_artifacts: IsolationPolicy::Forbidden,
            shared_scratch_state: IsolationPolicy::Forbidden,
            operator_leakage: IsolationPolicy::IndependentOperators,
        },
        attempt_policy: AttemptPolicy {
            task_replacement_after_failure: TaskReplacementPolicy::Forbidden,
            retry_policy_equal_across_conditions: true,
        },
        outcome_axes: required_outcome_axes(),
        non_authoritative_summary: None,
        adjudication: AdjudicationPlan {
            grader_identities: vec!["grader:independent-a".to_string()],
            rubric_id: "ripr-intervention-rubric.v1".to_string(),
            rubric_version: "1".to_string(),
            blinded_to_condition: true,
            min_independent_graders: 1,
            disagreement_policy: DisagreementPolicy::RetainPerAxisDoNotAverageCorrectness,
        },
        invalid_attempt_rules: InvalidAttemptRules {
            outcome_cells: vec![
                OutcomeCell::Pass,
                OutcomeCell::Fail,
                OutcomeCell::Partial,
                OutcomeCell::NotProven,
                OutcomeCell::Invalid,
                OutcomeCell::InstrumentFailure,
            ],
            retain_timeouts: true,
            retain_cancellations: true,
            retain_invalid_attempts: true,
            retain_missing_evidence: true,
            retain_instrument_failures: true,
            drop_timeouts: false,
            drop_invalid_attempts: false,
        },
        stopping_rule: StoppingRule {
            kind: StoppingKind::FixedSample,
            planned_pair_count: 2,
            may_depend_on_interim_estimate: false,
            favorable_interim_stop: FavorableInterimStop::Forbidden,
        },
        planned_analysis: PlannedAnalysis {
            unit: "exact repository/head/canonical behavior task".to_string(),
            compares: vec!["control".to_string(), "ripr_assisted".to_string()],
            reports_exact_numerators_and_denominators: true,
        },
        claim_ceiling: ClaimCeiling {
            bounded_to_task_series: true,
            bounded_to_model_operator_profile: true,
            bounded_to_intervention_form: true,
            preregistration_proves_intervention_value: false,
        },
        protocol_lock: ProtocolLock {
            immutable_after_first_attempt: true,
            first_attempt_recorded: false,
            mutation_without_new_study_id: MutationPolicy::Forbidden,
        },
        non_claims: vec![
            "no_agent_execution".to_string(),
            "no_repair_adjudication".to_string(),
            "no_pilot_result".to_string(),
            "no_intervention_value".to_string(),
            "no_generic_ab_platform".to_string(),
        ],
    }
}

#[cfg(test)]
fn empty_overlay() -> BudgetOverlay {
    BudgetOverlay {
        additional_wall_clock_ms: 0,
        additional_token_budget: 0,
        additional_retries: 0,
        additional_tools: Vec::new(),
    }
}

#[cfg(test)]
fn required_outcome_axes() -> Vec<OutcomeAxis> {
    [
        (
            "behavior_alignment",
            "test targets the exact changed behavior, owner, and sink",
        ),
        (
            "discriminating_proof_quality",
            "test would notice if the selected behavior were wrong",
        ),
        (
            "change_scope_and_churn",
            "edit stays inside the allowed test surface",
        ),
        (
            "review_findings_and_repair_burden",
            "independent review findings and follow-up repairs",
        ),
        (
            "completion_terminal_state",
            "typed terminal completeness of the attempt",
        ),
        (
            "time_and_bounded_resource_use",
            "elapsed effort and budget consumption",
        ),
        (
            "maintainability_readability",
            "independent readability and maintenance quality",
        ),
        (
            "false_confidence_or_unsupported_claim_events",
            "unsupported or false-success claims",
        ),
    ]
    .into_iter()
    .map(|(axis_id, description)| OutcomeAxis {
        axis_id: axis_id.to_string(),
        description: description.to_string(),
        compensating: false,
        independently_observable: true,
    })
    .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_code(result: Result<(), InterventionStudyError>, code: &str) -> Result<(), String> {
        match result {
            Ok(()) => Err(format!("expected rejection {code}")),
            Err(error) if error.code == code => Ok(()),
            Err(error) => Err(format!(
                "expected {code}, got {}: {}",
                error.code, error.message
            )),
        }
    }

    #[test]
    fn example_preregistration_satisfies_study_laws() -> Result<(), String> {
        example_preregistered_study()
            .validate()
            .map_err(|error| error.to_string())
    }

    #[test]
    fn assignment_after_outcome_is_rejected() -> Result<(), String> {
        let mut study = example_preregistered_study();
        study.assignment.may_change_after_outcome_or_grader_signal = true;
        assert_code(study.validate(), codes::ASSIGNMENT_AFTER_OUTCOME)?;
        study.assignment.may_change_after_outcome_or_grader_signal = false;
        study.assignment.freeze_rule = AssignmentFreezeRule::AfterOutcomeVisible;
        assert_code(study.validate(), codes::ASSIGNMENT_AFTER_OUTCOME)
    }

    #[test]
    fn unequal_condition_budgets_are_rejected() -> Result<(), String> {
        let mut study = example_preregistered_study();
        let overlay = study
            .condition_budget_overlays
            .get_mut(ConditionId::RiprAssisted.as_str())
            .ok_or("assisted overlay")?;
        overlay.additional_token_budget = 8_000;
        assert_code(study.validate(), codes::UNEQUAL_CONDITION_BUDGETS)
    }

    #[test]
    fn control_reading_ripr_outputs_is_rejected() -> Result<(), String> {
        let mut study = example_preregistered_study();
        study.intervention_surface.control.may_read_ripr_outputs = true;
        assert_code(study.validate(), codes::CONTROL_CAN_READ_RIPR_OUTPUTS)?;
        study.intervention_surface.control.may_read_ripr_outputs = false;
        study
            .intervention_surface
            .control
            .allowed_ripr_commands
            .push("ripr check --format json".to_string());
        assert_code(study.validate(), codes::CONTROL_CAN_READ_RIPR_OUTPUTS)
    }

    #[test]
    fn assisted_undeclared_context_is_rejected() -> Result<(), String> {
        let mut study = example_preregistered_study();
        study
            .intervention_surface
            .ripr_assisted
            .undeclared_repository_context
            .push("issue analysis not named by the protocol".to_string());
        assert_code(study.validate(), codes::ASSISTED_UNDECLARED_EXTRA_CONTEXT)
    }

    #[test]
    fn task_replacement_after_failure_is_rejected() -> Result<(), String> {
        let mut study = example_preregistered_study();
        study.attempt_policy.task_replacement_after_failure =
            TaskReplacementPolicy::AllowedAfterDifficultFailure;
        assert_code(study.validate(), codes::TASK_REPLACEMENT_AFTER_FAILURE)
    }

    #[test]
    fn retry_only_in_weaker_condition_is_rejected() -> Result<(), String> {
        let mut study = example_preregistered_study();
        study.attempt_policy.retry_policy_equal_across_conditions = false;
        assert_code(study.validate(), codes::RETRY_ONLY_IN_WEAKER_CONDITION)
    }

    #[test]
    fn dropping_timeouts_or_invalid_attempts_is_rejected() -> Result<(), String> {
        let mut study = example_preregistered_study();
        study.invalid_attempt_rules.drop_timeouts = true;
        assert_code(study.validate(), codes::DROP_INVALID_FROM_DENOMINATOR)?;
        study.invalid_attempt_rules.drop_timeouts = false;
        study.invalid_attempt_rules.retain_invalid_attempts = false;
        assert_code(study.validate(), codes::DROP_INVALID_FROM_DENOMINATOR)
    }

    #[test]
    fn stopping_on_favorable_interim_is_rejected() -> Result<(), String> {
        let mut study = example_preregistered_study();
        study.stopping_rule.kind = StoppingKind::StopOnFavorableInterim;
        study.stopping_rule.may_depend_on_interim_estimate = true;
        study.stopping_rule.favorable_interim_stop = FavorableInterimStop::Allowed;
        assert_code(study.validate(), codes::STOPPING_ON_FAVORABLE_INTERIM)
    }

    #[test]
    fn missing_grader_or_rubric_is_rejected() -> Result<(), String> {
        let mut study = example_preregistered_study();
        study.adjudication.grader_identities.clear();
        assert_code(study.validate(), codes::GRADER_OR_RUBRIC_ABSENT)?;
        study.adjudication.grader_identities = vec!["grader:independent-a".to_string()];
        study.adjudication.rubric_version.clear();
        assert_code(study.validate(), codes::GRADER_OR_RUBRIC_ABSENT)
    }

    #[test]
    fn protocol_mutation_after_first_attempt_requires_new_study_id() -> Result<(), String> {
        let mut predecessor = example_preregistered_study();
        predecessor.protocol_lock.first_attempt_recorded = true;
        let mut successor = predecessor.clone();
        successor.shared_budget.token_budget = 40_000;
        assert_code(
            predecessor.validate_successor(&successor),
            codes::PROTOCOL_MUTATION_REQUIRES_NEW_STUDY_ID,
        )?;
        successor.study_id = "study:ripr-intervention:iv01:successor".to_string();
        predecessor
            .validate_successor(&successor)
            .map_err(|error| error.to_string())
    }

    #[test]
    fn each_missing_required_outcome_cell_is_rejected() -> Result<(), String> {
        for cell in [
            OutcomeCell::Pass,
            OutcomeCell::Fail,
            OutcomeCell::Partial,
            OutcomeCell::NotProven,
            OutcomeCell::Invalid,
            OutcomeCell::InstrumentFailure,
        ] {
            let mut study = example_preregistered_study();
            let before = study.invalid_attempt_rules.outcome_cells.len();
            study
                .invalid_attempt_rules
                .outcome_cells
                .retain(|present| *present != cell);
            if study.invalid_attempt_rules.outcome_cells.len() == before {
                return Err(format!("example study did not carry {cell:?}"));
            }
            assert_code(study.validate(), codes::MISSING_FIELD)?;
        }
        Ok(())
    }

    #[test]
    fn planned_pair_count_mismatch_is_malformed_identity_not_an_interim_stop() -> Result<(), String>
    {
        for planned in [0, 3] {
            let mut study = example_preregistered_study();
            study.stopping_rule.planned_pair_count = planned;
            assert_code(study.validate(), codes::MALFORMED_IDENTITY)?;
        }
        let mut interim = example_preregistered_study();
        interim.stopping_rule.may_depend_on_interim_estimate = true;
        assert_code(interim.validate(), codes::STOPPING_ON_FAVORABLE_INTERIM)
    }

    #[test]
    fn recorded_attempt_lock_cannot_be_cleared_under_the_same_study_id() -> Result<(), String> {
        let mut predecessor = example_preregistered_study();
        predecessor.protocol_lock.first_attempt_recorded = true;
        let mut reset = predecessor.clone();
        reset.protocol_lock.first_attempt_recorded = false;
        assert_code(
            predecessor.validate_successor(&reset),
            codes::PROTOCOL_MUTATION_REQUIRES_NEW_STUDY_ID,
        )?;
        // An unchanged recorded successor remains a valid transition.
        predecessor
            .validate_successor(&predecessor.clone())
            .map_err(|error| error.to_string())
    }

    #[test]
    fn mutation_cannot_ride_along_with_recording_the_first_attempt() -> Result<(), String> {
        let predecessor = example_preregistered_study();
        let mut successor = predecessor.clone();
        successor.protocol_lock.first_attempt_recorded = true;
        // Recording the first attempt without mutation is the ordinary start.
        predecessor
            .validate_successor(&successor)
            .map_err(|error| error.to_string())?;
        successor.shared_budget.token_budget = 40_000;
        assert_code(
            predecessor.validate_successor(&successor),
            codes::PROTOCOL_MUTATION_REQUIRES_NEW_STUDY_ID,
        )?;
        // Pre-start amendment without a recorded attempt stays permitted.
        successor.protocol_lock.first_attempt_recorded = false;
        predecessor
            .validate_successor(&successor)
            .map_err(|error| error.to_string())
    }

    #[test]
    fn required_axes_stay_non_compensating() -> Result<(), String> {
        let mut study = example_preregistered_study();
        let axis = study
            .outcome_axes
            .get_mut(0)
            .ok_or("missing first outcome axis")?;
        axis.compensating = true;
        assert_code(study.validate(), codes::COMPENSATING_OUTCOME_SCORE)
    }
}
