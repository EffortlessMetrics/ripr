//! Issue lifecycle attempt extension contract (#4929, RIPR-SPEC-0218).
//!
//! Typed, versioned DTOs for one governed issue-lifecycle attempt:
//! `IssueLifecycleAttemptV1`, `IssueLifecycleCorpusV1`,
//! `IssueLifecycleDispositionV1` and the fail-closed row validator that
//! decides whether one committed lifecycle row may be counted. The
//! deterministic `IssueLifecycleScorecardV1` projection lives in
//! `crate::reports::issue_lifecycle` and derives from these row assessments,
//! so JSON and Markdown can never strengthen machine state.
//!
//! This contract is an extension of the #4925 base orchestration attempt
//! authority (RIPR-SPEC-0213), not a parallel task store: context,
//! execution, verification and cleanup facts stay in the referenced
//! `OrchestrationAttemptV1` rows, and each lifecycle row binds only the
//! attempt identities it extends plus `shared_facts` naming which fact
//! families are referenced. Nothing the base attempt already owns is copied.
//!
//! Counting law enforced here: synthetic mechanics evidence never enters
//! real denominators (the scorecard separates the two populations); issue
//! state is a projection over evidence, so a claimed disposition is
//! deterministically downgraded — never upgraded — when the retained
//! evidence does not support it: open intake questions downgrade everything
//! beyond `needs_evidence`, a spec-required decision blocks `qualified_one_pr`
//! qualification, an unchanged progress update that was not suppressed
//! blocks completion-shaped dispositions, and a merge is never a completed
//! closeout — a claimed `completed` without a completed closeout state and
//! current-head verification downgrades to `merged_pending_closeout` when a
//! merge identity is bound, to `partially_landed` when acceptance rows stay
//! uncovered, to `stale` when the verification/current-main receipt does not
//! match the observed current main, and to `blocked` otherwise.
//! `duplicate`, `already_satisfied`, `closed_not_planned`, `blocked` and
//! `needs_evidence` rows stay first-class and never count as implementation
//! success.
//!
//! The validator runs no triage, selects no work, mutates nothing and
//! decides no root disposition; a counted row claims only that this
//! contract's identity, disposition, burn-down and closeout evidence is
//! complete and internally consistent.

use serde::{Deserialize, Serialize};

pub(crate) const ISSUE_LIFECYCLE_ATTEMPT_SCHEMA_VERSION: &str = "issue_lifecycle_attempt.v1";
pub(crate) const ISSUE_LIFECYCLE_CORPUS_SCHEMA_VERSION: &str = "issue_lifecycle_corpus.v1";
pub(crate) const ISSUE_LIFECYCLE_FIXTURE_CORPUS_SCHEMA_VERSION: &str =
    "issue_lifecycle_fixture_corpus.v1";
pub(crate) const ISSUE_LIFECYCLE_SCORECARD_SCHEMA_VERSION: &str = "issue_lifecycle_scorecard.v1";
/// Closed vocabulary for the shared-fact families a lifecycle row references
/// from its base orchestration attempts (#4925); anything else rejects the
/// row as an unknown fact family.
pub(crate) const ISSUE_LIFECYCLE_SHARED_FACT_FAMILIES: [&str; 4] =
    ["context", "execution", "verification", "cleanup"];
pub(crate) const ISSUE_LIFECYCLE_CLAIM_BOUNDARY: &str = "Static issue lifecycle attempt receipt \
 contract: a counted row defines countable, auditable issue-lifecycle evidence only; it claims no \
 intake, planning, implementation or closeout quality on a real issue, no triage correctness and \
 no parent acceptance.";

/// The closed issue-lifecycle disposition vocabulary (#4929). Only
/// `Completed` is the positive terminal and counts as implementation
/// success; every other disposition stays visible in projections and no
/// aggregate rate converts them into success.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum IssueLifecycleDispositionV1 {
    NeedsEvidence,
    QualifiedOnePr,
    QualifiedSpecRequired,
    RootDecisionRequired,
    Duplicate,
    AlreadySatisfied,
    Superseded,
    Blocked,
    PartiallyLanded,
    VerificationFailed,
    MergedPendingCloseout,
    Completed,
    ClosedNotPlanned,
    Stale,
    NotRun,
}

impl IssueLifecycleDispositionV1 {
    /// Every disposition, in canonical declaration order, so projections keep
    /// a stable row even for dispositions with zero lifecycles.
    pub(crate) fn all() -> [Self; 15] {
        [
            Self::NeedsEvidence,
            Self::QualifiedOnePr,
            Self::QualifiedSpecRequired,
            Self::RootDecisionRequired,
            Self::Duplicate,
            Self::AlreadySatisfied,
            Self::Superseded,
            Self::Blocked,
            Self::PartiallyLanded,
            Self::VerificationFailed,
            Self::MergedPendingCloseout,
            Self::Completed,
            Self::ClosedNotPlanned,
            Self::Stale,
            Self::NotRun,
        ]
    }

    /// Only this disposition counts as implementation success in the
    /// scorecard; qualification, partial landing and every closed negative
    /// disposition remain visible without becoming success.
    pub(crate) fn is_implementation_success(self) -> bool {
        matches!(self, Self::Completed)
    }
}

/// Initial information completeness of the issue snapshot at intake.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum IssueLifecycleInformationCompletenessV1 {
    Complete,
    Partial,
    MissingEvidence,
}

/// Burn-down board state for the lifecycle row.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum IssueLifecycleBurnDownStateV1 {
    Open,
    InProgress,
    Closed,
}

/// Closeout stage of the lifecycle row. `Pending` covers a merged change
/// whose current-head proof or acceptance coverage is still outstanding.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum IssueLifecycleCloseoutStateV1 {
    NotStarted,
    Pending,
    Completed,
}

/// One claim lifecycle event observed during the issue attempt.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum IssueLifecycleClaimEventKindV1 {
    Claimed,
    Collision,
    Expired,
    Takeover,
}

/// One retained evidence reference: exact identity plus retained byte count.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct IssueLifecycleEvidenceRefV1 {
    pub identity: String,
    pub bytes: u64,
}

/// Reference to one base orchestration attempt (#4925). `shared_facts` names
/// the fact families this lifecycle row draws from that attempt and must be
/// drawn from the closed vocabulary `context | execution | verification |
/// cleanup`; the facts themselves are never copied into this row.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct IssueLifecycleBaseAttemptRefV1 {
    pub attempt_id: String,
    pub shared_facts: Vec<String>,
}

/// GitHub issue snapshot identity: the immutable snapshot plus the identity
/// of its comments, labels, assignees and milestone. Absent assignees or
/// milestone stay `None`; they are never invented.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct IssueLifecycleIssueSnapshotV1 {
    pub snapshot_id: String,
    pub issue_ref: String,
    pub comments_ref: String,
    pub labels_ref: String,
    pub assignees_ref: Option<String>,
    pub milestone_ref: Option<String>,
}

/// Initial intake assessment of the issue snapshot.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct IssueLifecycleInitialV1 {
    pub information_completeness: IssueLifecycleInformationCompletenessV1,
    /// Opaque issue family label (narrow bug, public contract, architecture
    /// ambiguity, ...); the family stays a producer observation, never a
    /// validator verdict.
    pub issue_family: String,
}

/// Shared context identities. Execution, verification and cleanup facts
/// themselves live in the referenced base attempts; this row binds only the
/// identity of the current main, the relevant PRs, the portfolio, the
/// selected work and the claim identities observed for this issue.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct IssueLifecycleContextV1 {
    pub current_main: String,
    pub relevant_prs: Vec<String>,
    pub portfolio: String,
    pub selected_work: String,
    pub claim_ids: Vec<String>,
}

/// Intake evidence: retained references plus the exact questions intake
/// could not answer. Open questions cap every claimed disposition at
/// `needs_evidence`; a question is never a guessed implementation.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct IssueLifecycleIntakeV1 {
    pub evidence: Vec<IssueLifecycleEvidenceRefV1>,
    pub missing_evidence_questions: Vec<String>,
}

/// The spec-required decision and its root disposition. `spec_required`
/// stays a recorded decision with its rationale, never an execution queue.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct IssueLifecycleContractDecisionV1 {
    pub spec_required: bool,
    pub decision_rationale: String,
    pub root_disposition: Option<String>,
}

/// Proposal/spec/ADR draft, challenge, amendment and acceptance identities.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct IssueLifecycleContractArtifactsV1 {
    pub proposal: Option<String>,
    pub spec: Option<String>,
    pub adr: Option<String>,
    pub challenge: Option<String>,
    pub amendments: Vec<String>,
    pub acceptance: Option<String>,
}

/// Plan identities: the plan, its work items, dependency edges and the
/// acceptance-coverage rows the plan claims to cover.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct IssueLifecyclePlanV1 {
    pub plan_id: String,
    pub work_items: Vec<String>,
    pub dependencies: Vec<String>,
    pub acceptance_coverage: Vec<String>,
}

/// One claim lifecycle event: claim, collision, expiry or takeover.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct IssueLifecycleClaimEventV1 {
    pub claim_id: String,
    pub event: IssueLifecycleClaimEventKindV1,
    pub detail: String,
}

/// Execution fact references into the base attempts: PR/review/check and
/// verification identities plus the current main the verification receipt
/// was bound against. These are identities, never copied facts.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct IssueLifecycleExecutionRefsV1 {
    pub pr: Option<String>,
    pub review: Option<String>,
    pub checks: Vec<String>,
    pub verification: Option<String>,
    pub merge: Option<String>,
    /// The main identity the verification/current-main receipt was bound
    /// against; closeout honesty compares it with `context.current_main`.
    pub current_main: String,
}

/// Progress mutation evidence: the mutation plan, the before/after digests
/// and whether an unchanged update was suppressed. An update whose before
/// and after digests are equal must be recorded as suppressed.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct IssueLifecycleProgressV1 {
    pub mutation_plan: Option<String>,
    pub before_digest: Option<String>,
    pub after_digest: Option<String>,
    pub suppressed_unchanged: bool,
}

/// Burn-down board: the state plus the uncovered, contradicted and deferred
/// acceptance rows. Deferred rows are disclosed, not hidden; uncovered and
/// contradicted rows block a completed closeout.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct IssueLifecycleBurnDownV1 {
    pub state: IssueLifecycleBurnDownStateV1,
    pub uncovered_rows: Vec<String>,
    pub contradicted_rows: Vec<String>,
    pub deferred_rows: Vec<String>,
}

/// Closeout state: the stage, the recorded reason, the remaining limitations
/// and the identity of the current-head verification receipt. A merge alone
/// never populates `current_head_verification`.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct IssueLifecycleCloseoutV1 {
    pub state: IssueLifecycleCloseoutStateV1,
    pub reason: Option<String>,
    pub remaining_limitations: Vec<String>,
    pub current_head_verification: Option<String>,
}

/// One governed issue-lifecycle attempt row. Every real row requires exact
/// issue snapshot, context, intake, plan and closeout identities; execution,
/// verification and cleanup facts are referenced from the base orchestration
/// attempts through `base_attempts` instead of being restated.
/// `row_digest` binds the retained surface and `observation_key` is the
/// deduplication key for repeated observations of one lifecycle.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct IssueLifecycleAttemptV1 {
    pub schema_version: String,
    pub lifecycle_id: String,
    pub observation_key: String,
    /// Synthetic mechanics evidence (fixtures) never enters real denominators.
    pub synthetic: bool,
    /// The producer-claimed disposition; the validator may downgrade it and
    /// records every downgrade reason, but never upgrades it.
    pub disposition: IssueLifecycleDispositionV1,
    pub base_attempts: Vec<IssueLifecycleBaseAttemptRefV1>,
    pub issue: IssueLifecycleIssueSnapshotV1,
    pub initial: IssueLifecycleInitialV1,
    pub context: IssueLifecycleContextV1,
    pub intake: IssueLifecycleIntakeV1,
    pub contract_decision: IssueLifecycleContractDecisionV1,
    pub contract_artifacts: IssueLifecycleContractArtifactsV1,
    pub plan: IssueLifecyclePlanV1,
    pub claim_events: Vec<IssueLifecycleClaimEventV1>,
    pub execution_refs: IssueLifecycleExecutionRefsV1,
    pub progress: IssueLifecycleProgressV1,
    pub burn_down: IssueLifecycleBurnDownV1,
    pub closeout: IssueLifecycleCloseoutV1,
    pub limitations: Vec<String>,
    pub non_claims: Vec<String>,
    /// SHA-256 hex over the canonical retained surface; recomputed by the
    /// validator and rejected on mismatch.
    pub row_digest: String,
}

/// A plain corpus of lifecycle rows (the real-use input shape).
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct IssueLifecycleCorpusV1 {
    pub schema_version: String,
    pub rows: Vec<IssueLifecycleAttemptV1>,
}

/// The validator's decision for one row. `counted` rows carry the
/// disposition after deterministic downgrades; rejected rows stay visible
/// through `reasons` and can never enter any denominator.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct IssueLifecycleRowAssessmentV1 {
    pub lifecycle_id: String,
    pub observation_key: String,
    pub synthetic: bool,
    pub identity: String,
    pub counted: bool,
    pub disposition: Option<IssueLifecycleDispositionV1>,
    pub reasons: Vec<String>,
}

/// The retained row digest surface: every row field except `row_digest`
/// itself. Field order is the canonical digest identity. The lifecycle
/// retained surface contains no host-local spellings (no worktree roots,
/// durations, PIDs or scratch paths), so the retained digest is already
/// portable: equivalent inputs at equivalent roots share one identity.
#[derive(Serialize)]
struct IssueLifecycleAttemptDigestInput<'a> {
    schema_version: &'a str,
    lifecycle_id: &'a str,
    observation_key: &'a str,
    synthetic: bool,
    disposition: IssueLifecycleDispositionV1,
    base_attempts: &'a [IssueLifecycleBaseAttemptRefV1],
    issue: &'a IssueLifecycleIssueSnapshotV1,
    initial: &'a IssueLifecycleInitialV1,
    context: &'a IssueLifecycleContextV1,
    intake: &'a IssueLifecycleIntakeV1,
    contract_decision: &'a IssueLifecycleContractDecisionV1,
    contract_artifacts: &'a IssueLifecycleContractArtifactsV1,
    plan: &'a IssueLifecyclePlanV1,
    claim_events: &'a [IssueLifecycleClaimEventV1],
    execution_refs: &'a IssueLifecycleExecutionRefsV1,
    progress: &'a IssueLifecycleProgressV1,
    burn_down: &'a IssueLifecycleBurnDownV1,
    closeout: &'a IssueLifecycleCloseoutV1,
    limitations: &'a [String],
    non_claims: &'a [String],
}

fn canonical_json<T: Serialize>(value: &T) -> Result<String, String> {
    serde_json::to_string(value).map_err(|error| format!("canonical serialization failed: {error}"))
}

fn digest_input(row: &IssueLifecycleAttemptV1) -> IssueLifecycleAttemptDigestInput<'_> {
    IssueLifecycleAttemptDigestInput {
        schema_version: &row.schema_version,
        lifecycle_id: &row.lifecycle_id,
        observation_key: &row.observation_key,
        synthetic: row.synthetic,
        disposition: row.disposition,
        base_attempts: &row.base_attempts,
        issue: &row.issue,
        initial: &row.initial,
        context: &row.context,
        intake: &row.intake,
        contract_decision: &row.contract_decision,
        contract_artifacts: &row.contract_artifacts,
        plan: &row.plan,
        claim_events: &row.claim_events,
        execution_refs: &row.execution_refs,
        progress: &row.progress,
        burn_down: &row.burn_down,
        closeout: &row.closeout,
        limitations: &row.limitations,
        non_claims: &row.non_claims,
    }
}

/// SHA-256 hex over the canonical retained row surface. The surface is
/// host-local-free by design, so this digest is also the portable identity.
pub(crate) fn issue_lifecycle_row_digest(row: &IssueLifecycleAttemptV1) -> Result<String, String> {
    Ok(crate::blind_journey::sha256_hex(
        canonical_json(&digest_input(row))?.as_bytes(),
    ))
}

/// Fail-closed counting-law assessment of one committed row. The producer's
/// claimed disposition is never upgraded; every downgrade reason is retained.
pub(crate) fn assess_issue_lifecycle_attempt(
    row: &IssueLifecycleAttemptV1,
) -> IssueLifecycleRowAssessmentV1 {
    // Serialization of this contract's own DTOs cannot fail in practice; keep
    // a named state instead of inventing an identity.
    let identity = match issue_lifecycle_row_digest(row) {
        Ok(digest) => digest,
        Err(error) => format!("identity_error:{error}"),
    };
    let base =
        |counted: bool, disposition: Option<IssueLifecycleDispositionV1>, reasons: Vec<String>| {
            IssueLifecycleRowAssessmentV1 {
                lifecycle_id: row.lifecycle_id.clone(),
                observation_key: row.observation_key.clone(),
                synthetic: row.synthetic,
                identity: identity.clone(),
                counted,
                disposition,
                reasons,
            }
        };
    if row.schema_version != ISSUE_LIFECYCLE_ATTEMPT_SCHEMA_VERSION {
        return base(
            false,
            None,
            vec![format!(
                "schema_version `{}` is not supported",
                row.schema_version
            )],
        );
    }
    match issue_lifecycle_row_digest(row) {
        Err(error) => return base(false, None, vec![error]),
        Ok(recomputed) if recomputed != row.row_digest => {
            return base(
                false,
                None,
                vec![format!(
                    "row_digest mismatch: recorded `{}`, recomputed `{recomputed}`",
                    row.row_digest
                )],
            );
        }
        Ok(_recomputed) => {}
    }
    let mut missing = Vec::new();
    if row.lifecycle_id.trim().is_empty() {
        missing.push("lifecycle_id");
    }
    if row.observation_key.trim().is_empty() {
        missing.push("observation_key");
    }
    if row.base_attempts.is_empty() {
        missing.push("base_attempts");
    }
    for (index, reference) in row.base_attempts.iter().enumerate() {
        if reference.attempt_id.trim().is_empty() {
            missing.push("base_attempts.attempt_id");
        }
        if reference.shared_facts.is_empty() {
            missing.push("base_attempts.shared_facts");
        }
        for fact in &reference.shared_facts {
            if !ISSUE_LIFECYCLE_SHARED_FACT_FAMILIES.contains(&fact.as_str()) {
                return base(
                    false,
                    None,
                    vec![format!(
                        "base_attempts entry {index} references unknown shared fact family `{fact}`"
                    )],
                );
            }
        }
    }
    if row.issue.snapshot_id.trim().is_empty() {
        missing.push("issue.snapshot_id");
    }
    if row.issue.issue_ref.trim().is_empty() {
        missing.push("issue.issue_ref");
    }
    if row.issue.comments_ref.trim().is_empty() {
        missing.push("issue.comments_ref");
    }
    if row.issue.labels_ref.trim().is_empty() {
        missing.push("issue.labels_ref");
    }
    if row.initial.issue_family.trim().is_empty() {
        missing.push("initial.issue_family");
    }
    if row.context.current_main.trim().is_empty() {
        missing.push("context.current_main");
    }
    if row.context.portfolio.trim().is_empty() {
        missing.push("context.portfolio");
    }
    if row.context.selected_work.trim().is_empty() {
        missing.push("context.selected_work");
    }
    if row.contract_decision.decision_rationale.trim().is_empty() {
        missing.push("contract_decision.decision_rationale");
    }
    if row.plan.plan_id.trim().is_empty() {
        missing.push("plan.plan_id");
    }
    if row.execution_refs.current_main.trim().is_empty() {
        missing.push("execution_refs.current_main");
    }
    if row
        .claim_events
        .iter()
        .any(|event| event.claim_id.trim().is_empty())
    {
        missing.push("claim_events.claim_id");
    }
    if row.closeout.state == IssueLifecycleCloseoutStateV1::Completed
        && row
            .closeout
            .reason
            .as_ref()
            .is_none_or(|reason| reason.trim().is_empty())
    {
        missing.push("closeout.reason");
    }
    for (index, evidence) in row.intake.evidence.iter().enumerate() {
        if evidence.identity.trim().is_empty() {
            missing.push("intake.evidence.identity");
        }
        if evidence.bytes == 0 {
            return base(
                false,
                None,
                vec![format!("intake evidence {index} is malformed: zero bytes")],
            );
        }
    }
    if !missing.is_empty() {
        return base(
            false,
            None,
            vec![format!("missing required identities: {missing:?}")],
        );
    }

    let claimed = row.disposition;
    let mut disposition = claimed;
    let mut reasons: Vec<String> = Vec::new();

    // Intake evidence gate: open questions cap every claimed disposition at
    // needs_evidence; a label, comment or confident summary alone is never
    // authority to claim more.
    if claimed != IssueLifecycleDispositionV1::NeedsEvidence
        && claimed != IssueLifecycleDispositionV1::NotRun
        && !row.intake.missing_evidence_questions.is_empty()
    {
        reasons.push(format!(
            "open intake questions {:?} cap the disposition at needs_evidence",
            row.intake.missing_evidence_questions
        ));
        disposition = IssueLifecycleDispositionV1::NeedsEvidence;
    }

    // Qualification coherence: a spec-required decision can never qualify as
    // direct one-PR work, whatever the producer claimed.
    if disposition == IssueLifecycleDispositionV1::QualifiedOnePr
        && row.contract_decision.spec_required
    {
        reasons.push("spec_required decision blocks a qualified_one_pr qualification".to_string());
        disposition = IssueLifecycleDispositionV1::QualifiedSpecRequired;
    }

    // Unchanged suppression: a recorded progress mutation whose before and
    // after digests are equal must have been suppressed; an unsuppressed
    // no-op update blocks completion-shaped dispositions.
    let unchanged_unsuppressed = row.progress.mutation_plan.is_some()
        && row.progress.before_digest.is_some()
        && row.progress.before_digest == row.progress.after_digest
        && !row.progress.suppressed_unchanged;
    if unchanged_unsuppressed
        && matches!(
            disposition,
            IssueLifecycleDispositionV1::Completed
                | IssueLifecycleDispositionV1::PartiallyLanded
                | IssueLifecycleDispositionV1::MergedPendingCloseout
        )
    {
        reasons.push("unchanged progress update must be suppressed".to_string());
        disposition = IssueLifecycleDispositionV1::Blocked;
    }

    // Merge is not closeout: a claimed completed closeout must be backed by
    // a completed closeout state, a current-head verification receipt and a
    // clean burn-down against the observed current main. The first
    // established downgrade wins; every blocking condition is retained.
    if disposition == IssueLifecycleDispositionV1::Completed {
        let merged = row.execution_refs.merge.is_some();
        if row.closeout.state != IssueLifecycleCloseoutStateV1::Completed {
            record_closeout_downgrade(
                &mut disposition,
                if merged {
                    IssueLifecycleDispositionV1::MergedPendingCloseout
                } else {
                    IssueLifecycleDispositionV1::Blocked
                },
                "completed disposition requires a completed closeout state".to_string(),
                &mut reasons,
            );
        }
        if row
            .closeout
            .current_head_verification
            .as_ref()
            .is_none_or(|verification| verification.trim().is_empty())
        {
            record_closeout_downgrade(
                &mut disposition,
                if merged {
                    IssueLifecycleDispositionV1::MergedPendingCloseout
                } else {
                    IssueLifecycleDispositionV1::Blocked
                },
                "merge without current-head proof remains pending closeout".to_string(),
                &mut reasons,
            );
        }
        if row.execution_refs.current_main != row.context.current_main {
            record_closeout_downgrade(
                &mut disposition,
                IssueLifecycleDispositionV1::Stale,
                "verification/current-main receipt is stale against the observed current main"
                    .to_string(),
                &mut reasons,
            );
        }
        if !row.burn_down.uncovered_rows.is_empty() || !row.burn_down.contradicted_rows.is_empty() {
            record_closeout_downgrade(
                &mut disposition,
                if merged {
                    IssueLifecycleDispositionV1::PartiallyLanded
                } else {
                    IssueLifecycleDispositionV1::Blocked
                },
                format!(
                    "uncovered burn-down rows {:?} or contradicted burn-down rows {:?} block a completed closeout",
                    row.burn_down.uncovered_rows, row.burn_down.contradicted_rows
                ),
                &mut reasons,
            );
        }
        if row.contract_decision.spec_required && row.contract_artifacts.acceptance.is_none() {
            record_closeout_downgrade(
                &mut disposition,
                IssueLifecycleDispositionV1::Blocked,
                "spec-required decision without acceptance evidence blocks a completed closeout"
                    .to_string(),
                &mut reasons,
            );
        }
    }

    base(true, Some(disposition), reasons)
}

/// Push one closeout downgrade reason and move a still-`completed`
/// disposition to `to`. The first established downgrade wins: once a row is
/// no longer `completed`, later gates retain their reasons without
/// rewriting the disposition.
fn record_closeout_downgrade(
    disposition: &mut IssueLifecycleDispositionV1,
    to: IssueLifecycleDispositionV1,
    reason: String,
    reasons: &mut Vec<String>,
) {
    reasons.push(reason);
    if *disposition == IssueLifecycleDispositionV1::Completed {
        *disposition = to;
    }
}

/// Scenario expectation recorded in the committed fixture corpus.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
pub(crate) struct IssueLifecycleFixtureExpectationV1 {
    pub countable: bool,
    pub disposition: Option<IssueLifecycleDispositionV1>,
    /// Every entry must appear verbatim in the assessed row reasons.
    pub reason_contains: Vec<String>,
}

/// One committed fixture scenario: an expected counting outcome plus one
/// full lifecycle row.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct IssueLifecycleFixtureScenarioV1 {
    pub id: String,
    pub expected: IssueLifecycleFixtureExpectationV1,
    pub attempt: IssueLifecycleAttemptV1,
}

/// The committed issue-lifecycle fixture corpus (RIPR-SPEC-0218).
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct IssueLifecycleFixtureCorpusV1 {
    pub schema_version: String,
    pub scenarios: Vec<IssueLifecycleFixtureScenarioV1>,
}

/// The fixture scenarios #4929 requires; the committed corpus must cover all
/// of them and the live validator decides each outcome independently.
pub(crate) const REQUIRED_ISSUE_LIFECYCLE_SCENARIO_IDS: [&str; 14] = [
    "one_pr_bug_no_new_spec",
    "public_contract_requires_independent_challenge",
    "architecture_ambiguity_returns_root_decision",
    "duplicate_case",
    "already_satisfied_case",
    "overlapping_open_pr_blocks_pickup",
    "needs_evidence_case",
    "partially_landed_umbrella_uncovered_rows",
    "verification_failed_after_merge_candidate",
    "merged_pending_current_head_closeout",
    "stale_main_verification_receipt",
    "unchanged_progress_update_suppressed",
    "closed_not_planned_without_implementation_success",
    "malformed_zero_byte_intake_evidence",
];

/// Required scenario ids absent from one committed corpus id set.
pub(crate) fn missing_issue_lifecycle_required_scenarios<'a>(
    present: impl IntoIterator<Item = &'a str>,
) -> Vec<&'static str> {
    let present: std::collections::BTreeSet<&str> = present.into_iter().collect();
    REQUIRED_ISSUE_LIFECYCLE_SCENARIO_IDS
        .iter()
        .filter(|required| !present.contains(**required))
        .copied()
        .collect()
}

/// Load and parse one committed fixture corpus.
pub(crate) fn load_issue_lifecycle_fixture_corpus(
    body: &str,
) -> Result<IssueLifecycleFixtureCorpusV1, String> {
    let corpus: IssueLifecycleFixtureCorpusV1 = serde_json::from_str(body)
        .map_err(|error| format!("parse issue lifecycle fixture corpus: {error}"))?;
    if corpus.schema_version != ISSUE_LIFECYCLE_FIXTURE_CORPUS_SCHEMA_VERSION {
        return Err(format!(
            "unsupported issue lifecycle fixture corpus schema `{}`",
            corpus.schema_version
        ));
    }
    let mut ids = std::collections::BTreeSet::new();
    for scenario in &corpus.scenarios {
        if !ids.insert(scenario.id.clone()) {
            return Err(format!(
                "duplicate issue lifecycle fixture scenario id `{}`",
                scenario.id
            ));
        }
    }
    Ok(corpus)
}

/// Load and parse one plain issue-lifecycle corpus.
pub(crate) fn load_issue_lifecycle_corpus(body: &str) -> Result<IssueLifecycleCorpusV1, String> {
    let corpus: IssueLifecycleCorpusV1 = serde_json::from_str(body)
        .map_err(|error| format!("parse issue lifecycle corpus: {error}"))?;
    if corpus.schema_version != ISSUE_LIFECYCLE_CORPUS_SCHEMA_VERSION {
        return Err(format!(
            "unsupported issue lifecycle corpus schema `{}`",
            corpus.schema_version
        ));
    }
    Ok(corpus)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hex64(fill: char) -> String {
        fill.to_string().repeat(64)
    }

    fn sample_lifecycle() -> IssueLifecycleAttemptV1 {
        IssueLifecycleAttemptV1 {
            schema_version: ISSUE_LIFECYCLE_ATTEMPT_SCHEMA_VERSION.to_string(),
            lifecycle_id: "lifecycle-sample".to_string(),
            observation_key: "observation-sample".to_string(),
            synthetic: false,
            disposition: IssueLifecycleDispositionV1::Completed,
            base_attempts: vec![IssueLifecycleBaseAttemptRefV1 {
                attempt_id: "attempt-sample".to_string(),
                shared_facts: ["context", "execution", "verification", "cleanup"]
                    .iter()
                    .map(|fact| (*fact).to_string())
                    .collect(),
            }],
            issue: IssueLifecycleIssueSnapshotV1 {
                snapshot_id: "issue-snapshot:sha256:".to_string() + &hex64('a'),
                issue_ref: "operator/target#1234".to_string(),
                comments_ref: "issue-comments:sha256:".to_string() + &hex64('b'),
                labels_ref: "issue-labels:sha256:".to_string() + &hex64('c'),
                assignees_ref: Some("issue-assignees:sha256:".to_string() + &hex64('d')),
                milestone_ref: None,
            },
            initial: IssueLifecycleInitialV1 {
                information_completeness: IssueLifecycleInformationCompletenessV1::Complete,
                issue_family: "narrow_bug".to_string(),
            },
            context: IssueLifecycleContextV1 {
                current_main: "main-sha".to_string(),
                relevant_prs: vec!["pr-sample".to_string()],
                portfolio: "campaign-sample".to_string(),
                selected_work: "issue-sample".to_string(),
                claim_ids: vec!["claim-sample".to_string()],
            },
            intake: IssueLifecycleIntakeV1 {
                evidence: vec![IssueLifecycleEvidenceRefV1 {
                    identity: "intake-packet:sha256:".to_string() + &hex64('e'),
                    bytes: 1024,
                }],
                missing_evidence_questions: Vec::new(),
            },
            contract_decision: IssueLifecycleContractDecisionV1 {
                spec_required: false,
                decision_rationale: "narrow accepted-contract bug; no new spec".to_string(),
                root_disposition: Some("root-accepted".to_string()),
            },
            contract_artifacts: IssueLifecycleContractArtifactsV1 {
                proposal: None,
                spec: None,
                adr: None,
                challenge: None,
                amendments: Vec::new(),
                acceptance: Some("contract-acceptance:sha256:".to_string() + &hex64('f')),
            },
            plan: IssueLifecyclePlanV1 {
                plan_id: "plan-sample".to_string(),
                work_items: vec!["work-item-sample".to_string()],
                dependencies: Vec::new(),
                acceptance_coverage: vec!["acceptance-row-sample".to_string()],
            },
            claim_events: vec![IssueLifecycleClaimEventV1 {
                claim_id: "claim-sample".to_string(),
                event: IssueLifecycleClaimEventKindV1::Claimed,
                detail: "durable exclusive writer claim".to_string(),
            }],
            execution_refs: IssueLifecycleExecutionRefsV1 {
                pr: Some("pr-sample".to_string()),
                review: Some("review-sample".to_string()),
                checks: vec!["ci-sample".to_string()],
                verification: Some("verification-sample".to_string()),
                merge: Some("merge-sample".to_string()),
                current_main: "main-sha".to_string(),
            },
            progress: IssueLifecycleProgressV1 {
                mutation_plan: None,
                before_digest: None,
                after_digest: None,
                suppressed_unchanged: false,
            },
            burn_down: IssueLifecycleBurnDownV1 {
                state: IssueLifecycleBurnDownStateV1::Closed,
                uncovered_rows: Vec::new(),
                contradicted_rows: Vec::new(),
                deferred_rows: Vec::new(),
            },
            closeout: IssueLifecycleCloseoutV1 {
                state: IssueLifecycleCloseoutStateV1::Completed,
                reason: Some("acceptance covered at current main".to_string()),
                remaining_limitations: vec!["sample limitation".to_string()],
                current_head_verification: Some(
                    "current-head-verification:sha256:".to_string() + &hex64('1'),
                ),
            },
            limitations: vec!["sample limitation".to_string()],
            non_claims: vec!["sample non-claim".to_string()],
            row_digest: String::new(),
        }
    }

    fn stamped_sample() -> Result<IssueLifecycleAttemptV1, String> {
        let mut row = sample_lifecycle();
        row.row_digest = issue_lifecycle_row_digest(&row)?;
        Ok(row)
    }

    fn reason_contains(assessment: &IssueLifecycleRowAssessmentV1, needle: &str) -> bool {
        assessment
            .reasons
            .iter()
            .any(|reason| reason.contains(needle))
    }

    #[test]
    fn complete_lifecycle_row_counts_as_completed() -> Result<(), String> {
        let row = stamped_sample()?;
        let assessment = assess_issue_lifecycle_attempt(&row);
        if !assessment.counted {
            return Err(format!(
                "valid row was not counted: {:?}",
                assessment.reasons
            ));
        }
        if assessment.disposition != Some(IssueLifecycleDispositionV1::Completed) {
            return Err(format!(
                "valid row downgraded unexpectedly: {:?} reasons={:?}",
                assessment.disposition, assessment.reasons
            ));
        }
        Ok(())
    }

    #[test]
    fn schema_drift_rejects_the_row() -> Result<(), String> {
        let mut row = stamped_sample()?;
        row.schema_version = "issue_lifecycle_attempt.v2".to_string();
        row.row_digest = issue_lifecycle_row_digest(&row)?;
        let assessment = assess_issue_lifecycle_attempt(&row);
        if assessment.counted || !reason_contains(&assessment, "schema_version") {
            return Err(format!(
                "a drifted schema must reject, got counted={} reasons={:?}",
                assessment.counted, assessment.reasons
            ));
        }
        Ok(())
    }

    #[test]
    fn altered_row_bytes_reject_the_digest_binding() -> Result<(), String> {
        let mut row = stamped_sample()?;
        row.plan.work_items.push("work-item-extra".to_string());
        // row_digest intentionally not recomputed: the binding must catch it.
        let assessment = assess_issue_lifecycle_attempt(&row);
        if assessment.counted || !reason_contains(&assessment, "row_digest mismatch") {
            return Err(format!(
                "an altered row must reject on its digest binding, got counted={} reasons={:?}",
                assessment.counted, assessment.reasons
            ));
        }
        Ok(())
    }

    #[test]
    fn blank_base_attempt_identity_rejects_the_row() -> Result<(), String> {
        let mut row = stamped_sample()?;
        row.base_attempts[0].attempt_id = String::new();
        row.row_digest = issue_lifecycle_row_digest(&row)?;
        let assessment = assess_issue_lifecycle_attempt(&row);
        if assessment.counted || !reason_contains(&assessment, "base_attempts.attempt_id") {
            return Err(format!(
                "a blank base attempt identity must reject, got counted={} reasons={:?}",
                assessment.counted, assessment.reasons
            ));
        }
        Ok(())
    }

    #[test]
    fn empty_shared_facts_reject_the_row() -> Result<(), String> {
        let mut row = stamped_sample()?;
        row.base_attempts[0].shared_facts = Vec::new();
        row.row_digest = issue_lifecycle_row_digest(&row)?;
        let assessment = assess_issue_lifecycle_attempt(&row);
        if assessment.counted || !reason_contains(&assessment, "shared_facts") {
            return Err(format!(
                "empty shared_facts must reject, got counted={} reasons={:?}",
                assessment.counted, assessment.reasons
            ));
        }
        Ok(())
    }

    #[test]
    fn unknown_shared_fact_family_rejects_the_row() -> Result<(), String> {
        let mut row = stamped_sample()?;
        row.base_attempts[0]
            .shared_facts
            .push("telemetry".to_string());
        row.row_digest = issue_lifecycle_row_digest(&row)?;
        let assessment = assess_issue_lifecycle_attempt(&row);
        if assessment.counted || !reason_contains(&assessment, "unknown shared fact") {
            return Err(format!(
                "an unknown shared fact family must reject, got counted={} reasons={:?}",
                assessment.counted, assessment.reasons
            ));
        }
        Ok(())
    }

    #[test]
    fn open_intake_questions_cap_at_needs_evidence() -> Result<(), String> {
        let mut row = stamped_sample()?;
        row.intake.missing_evidence_questions =
            vec!["what is the exact failing command?".to_string()];
        row.row_digest = issue_lifecycle_row_digest(&row)?;
        let assessment = assess_issue_lifecycle_attempt(&row);
        if assessment.disposition != Some(IssueLifecycleDispositionV1::NeedsEvidence)
            || !reason_contains(&assessment, "needs_evidence")
        {
            return Err(format!(
                "open intake questions must cap at needs_evidence, got {:?} reasons={:?}",
                assessment.disposition, assessment.reasons
            ));
        }
        Ok(())
    }

    #[test]
    fn spec_required_blocks_one_pr_qualification() -> Result<(), String> {
        let mut row = stamped_sample()?;
        row.disposition = IssueLifecycleDispositionV1::QualifiedOnePr;
        row.contract_decision.spec_required = true;
        row.closeout.state = IssueLifecycleCloseoutStateV1::NotStarted;
        row.closeout.reason = None;
        row.closeout.current_head_verification = None;
        row.contract_artifacts.acceptance = None;
        row.execution_refs.merge = None;
        row.execution_refs.pr = None;
        row.row_digest = issue_lifecycle_row_digest(&row)?;
        let assessment = assess_issue_lifecycle_attempt(&row);
        if assessment.disposition != Some(IssueLifecycleDispositionV1::QualifiedSpecRequired)
            || !reason_contains(&assessment, "qualified_one_pr")
        {
            return Err(format!(
                "a spec-required decision must block one-pr qualification, got {:?} reasons={:?}",
                assessment.disposition, assessment.reasons
            ));
        }
        Ok(())
    }

    #[test]
    fn unsuppressed_unchanged_progress_blocks_completion() -> Result<(), String> {
        let mut row = stamped_sample()?;
        row.progress.mutation_plan = Some("progress-plan:sha256:sample".to_string());
        row.progress.before_digest = Some("digest-before".to_string());
        row.progress.after_digest = Some("digest-before".to_string());
        row.progress.suppressed_unchanged = false;
        row.row_digest = issue_lifecycle_row_digest(&row)?;
        let assessment = assess_issue_lifecycle_attempt(&row);
        if assessment.disposition != Some(IssueLifecycleDispositionV1::Blocked)
            || !reason_contains(&assessment, "suppressed")
        {
            return Err(format!(
                "an unsuppressed unchanged update must block, got {:?} reasons={:?}",
                assessment.disposition, assessment.reasons
            ));
        }
        Ok(())
    }

    #[test]
    fn merge_without_current_head_proof_stays_pending_closeout() -> Result<(), String> {
        let mut row = stamped_sample()?;
        row.closeout.current_head_verification = None;
        row.row_digest = issue_lifecycle_row_digest(&row)?;
        let assessment = assess_issue_lifecycle_attempt(&row);
        if assessment.disposition != Some(IssueLifecycleDispositionV1::MergedPendingCloseout)
            || !reason_contains(&assessment, "current-head")
        {
            return Err(format!(
                "merge without current-head proof must stay pending closeout, got {:?} reasons={:?}",
                assessment.disposition, assessment.reasons
            ));
        }
        Ok(())
    }

    #[test]
    fn incomplete_closeout_state_blocks_completed_without_merge() -> Result<(), String> {
        let mut row = stamped_sample()?;
        row.closeout.state = IssueLifecycleCloseoutStateV1::Pending;
        row.execution_refs.merge = None;
        row.row_digest = issue_lifecycle_row_digest(&row)?;
        let assessment = assess_issue_lifecycle_attempt(&row);
        if assessment.disposition != Some(IssueLifecycleDispositionV1::Blocked)
            || !reason_contains(&assessment, "closeout state")
        {
            return Err(format!(
                "a pending closeout without merge must block, got {:?} reasons={:?}",
                assessment.disposition, assessment.reasons
            ));
        }
        Ok(())
    }

    #[test]
    fn uncovered_burn_down_rows_keep_a_merge_partially_landed() -> Result<(), String> {
        let mut row = stamped_sample()?;
        row.burn_down.uncovered_rows = vec!["acceptance row: cli flag wiring".to_string()];
        row.row_digest = issue_lifecycle_row_digest(&row)?;
        let assessment = assess_issue_lifecycle_attempt(&row);
        if assessment.disposition != Some(IssueLifecycleDispositionV1::PartiallyLanded)
            || !reason_contains(&assessment, "uncovered")
        {
            return Err(format!(
                "uncovered acceptance rows must keep the lifecycle partially landed, got {:?} reasons={:?}",
                assessment.disposition, assessment.reasons
            ));
        }
        Ok(())
    }

    #[test]
    fn uncovered_burn_down_rows_block_completed_without_merge() -> Result<(), String> {
        let mut row = stamped_sample()?;
        row.burn_down.uncovered_rows = vec!["acceptance row: cli flag wiring".to_string()];
        row.execution_refs.merge = None;
        row.row_digest = issue_lifecycle_row_digest(&row)?;
        let assessment = assess_issue_lifecycle_attempt(&row);
        if assessment.disposition != Some(IssueLifecycleDispositionV1::Blocked) {
            return Err(format!(
                "uncovered acceptance rows without a merge must block, got {:?}",
                assessment.disposition
            ));
        }
        Ok(())
    }

    #[test]
    fn stale_current_main_receipt_forces_stale() -> Result<(), String> {
        let mut row = stamped_sample()?;
        row.execution_refs.current_main = "older-main-sha".to_string();
        row.row_digest = issue_lifecycle_row_digest(&row)?;
        let assessment = assess_issue_lifecycle_attempt(&row);
        if assessment.disposition != Some(IssueLifecycleDispositionV1::Stale)
            || !reason_contains(&assessment, "stale")
        {
            return Err(format!(
                "a stale current-main receipt must force stale, got {:?} reasons={:?}",
                assessment.disposition, assessment.reasons
            ));
        }
        Ok(())
    }

    #[test]
    fn first_established_downgrade_wins_across_closeout_gates() -> Result<(), String> {
        let mut row = stamped_sample()?;
        row.closeout.current_head_verification = None;
        row.burn_down.uncovered_rows = vec!["acceptance row: cli flag wiring".to_string()];
        row.row_digest = issue_lifecycle_row_digest(&row)?;
        let assessment = assess_issue_lifecycle_attempt(&row);
        if assessment.disposition != Some(IssueLifecycleDispositionV1::MergedPendingCloseout)
            || !reason_contains(&assessment, "current-head")
            || !reason_contains(&assessment, "uncovered")
        {
            return Err(format!(
                "the first downgrade must win with every reason retained, got {:?} reasons={:?}",
                assessment.disposition, assessment.reasons
            ));
        }
        Ok(())
    }

    #[test]
    fn spec_required_without_acceptance_blocks_completed() -> Result<(), String> {
        let mut row = stamped_sample()?;
        row.contract_decision.spec_required = true;
        row.contract_artifacts.acceptance = None;
        row.row_digest = issue_lifecycle_row_digest(&row)?;
        let assessment = assess_issue_lifecycle_attempt(&row);
        if assessment.disposition != Some(IssueLifecycleDispositionV1::Blocked)
            || !reason_contains(&assessment, "acceptance evidence")
        {
            return Err(format!(
                "a spec-required decision without acceptance must block, got {:?} reasons={:?}",
                assessment.disposition, assessment.reasons
            ));
        }
        Ok(())
    }

    #[test]
    fn negative_dispositions_pass_through_and_stay_visible() -> Result<(), String> {
        for disposition in [
            IssueLifecycleDispositionV1::Duplicate,
            IssueLifecycleDispositionV1::AlreadySatisfied,
            IssueLifecycleDispositionV1::ClosedNotPlanned,
            IssueLifecycleDispositionV1::Blocked,
            IssueLifecycleDispositionV1::VerificationFailed,
            IssueLifecycleDispositionV1::Stale,
            IssueLifecycleDispositionV1::MergedPendingCloseout,
            IssueLifecycleDispositionV1::NotRun,
        ] {
            let mut row = sample_lifecycle();
            row.disposition = disposition;
            row.closeout.state = IssueLifecycleCloseoutStateV1::NotStarted;
            row.closeout.reason = None;
            row.closeout.current_head_verification = None;
            row.contract_artifacts.acceptance = None;
            row.execution_refs.merge = None;
            row.execution_refs.pr = None;
            row.row_digest = issue_lifecycle_row_digest(&row)?;
            let assessment = assess_issue_lifecycle_attempt(&row);
            if !assessment.counted || assessment.disposition != Some(disposition) {
                return Err(format!(
                    "disposition {disposition:?} must pass through counted and visible, got counted={} disposition={:?} reasons={:?}",
                    assessment.counted, assessment.disposition, assessment.reasons
                ));
            }
        }
        Ok(())
    }

    #[test]
    fn completed_closeout_requires_a_recorded_reason() -> Result<(), String> {
        let mut row = stamped_sample()?;
        row.closeout.reason = None;
        row.row_digest = issue_lifecycle_row_digest(&row)?;
        let assessment = assess_issue_lifecycle_attempt(&row);
        if assessment.counted || !reason_contains(&assessment, "closeout.reason") {
            return Err(format!(
                "a completed closeout without a reason must reject, got counted={} reasons={:?}",
                assessment.counted, assessment.reasons
            ));
        }
        Ok(())
    }

    #[test]
    fn zero_byte_intake_evidence_rejects_the_row() -> Result<(), String> {
        let mut row = stamped_sample()?;
        row.intake.evidence[0].bytes = 0;
        row.row_digest = issue_lifecycle_row_digest(&row)?;
        let assessment = assess_issue_lifecycle_attempt(&row);
        if assessment.counted || !reason_contains(&assessment, "malformed") {
            return Err(format!(
                "zero-byte intake evidence must reject, got counted={} reasons={:?}",
                assessment.counted, assessment.reasons
            ));
        }
        Ok(())
    }

    #[test]
    fn synthetic_flag_does_not_change_row_assessment() -> Result<(), String> {
        let mut row = stamped_sample()?;
        row.synthetic = true;
        row.row_digest = issue_lifecycle_row_digest(&row)?;
        let assessment = assess_issue_lifecycle_attempt(&row);
        if !assessment.counted
            || assessment.disposition != Some(IssueLifecycleDispositionV1::Completed)
        {
            return Err(
                "synthetic rows must assess by the same law; only scorecard denominators separate them"
                    .to_string(),
            );
        }
        Ok(())
    }

    #[test]
    fn fixture_corpus_loader_rejects_duplicates_and_drifted_schemas() -> Result<(), String> {
        let row = stamped_sample()?;
        let scenario = serde_json::json!({
            "id": "sample",
            "expected": { "countable": true, "disposition": "completed",
                "reason_contains": [] },
            "attempt": serde_json::to_value(&row).map_err(|error| error.to_string())?
        });
        let corpus = serde_json::json!({
            "schema_version": ISSUE_LIFECYCLE_FIXTURE_CORPUS_SCHEMA_VERSION,
            "scenarios": [scenario.clone(), scenario]
        });
        let body = serde_json::to_string(&corpus).map_err(|error| error.to_string())?;
        match load_issue_lifecycle_fixture_corpus(&body) {
            Err(message) if message.contains("duplicate") => Ok(()),
            Err(message) => Err(format!("expected a duplicate-id error, got: {message}")),
            Ok(_corpus) => Err("a corpus with duplicate scenario ids must be rejected".to_string()),
        }
    }

    #[test]
    fn missing_required_scenarios_are_reported() -> Result<(), String> {
        let missing = missing_issue_lifecycle_required_scenarios(["one_pr_bug_no_new_spec"]);
        if missing.len() != REQUIRED_ISSUE_LIFECYCLE_SCENARIO_IDS.len() - 1 {
            return Err(format!(
                "expected every other scenario to be reported missing, got {missing:?}"
            ));
        }
        let none =
            missing_issue_lifecycle_required_scenarios(REQUIRED_ISSUE_LIFECYCLE_SCENARIO_IDS);
        if !none.is_empty() {
            return Err(format!(
                "a complete id set must report nothing missing, got {none:?}"
            ));
        }
        Ok(())
    }
}
