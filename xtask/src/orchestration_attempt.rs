//! Orchestration attempt receipts contract (#4925, RIPR-SPEC-0213).
//!
//! Typed, versioned DTOs for one governed orchestration attempt:
//! `OrchestrationAttemptV1`, `OrchestrationCorpusV1`, `AttemptStrategyV1`,
//! `AttemptDispositionV1` and `AttemptComparisonV1`, plus the fail-closed row
//! validator that decides whether one committed attempt row may be counted.
//! The deterministic `OrchestrationScorecardV1` projection lives in
//! `crate::reports::orchestration` and derives from these row assessments, so
//! JSON and Markdown can never strengthen machine state.
//!
//! Counting law enforced here: synthetic mechanics evidence never enters real
//! denominators (the scorecard separates the two populations); one work item
//! observed through several roles remains one attempt (deduplication is the
//! scorecard's, keyed on `observation_key` with portable-identity agreement);
//! blocked, contradicted, stale, malformed, over-budget, verification-failed
//! and single-agent-preferred rows remain visible as rows or rejected rows; a
//! passing command with zero subjects is not verification; a builder claim
//! cannot become `verified_fact` without a matching independent receipt;
//! missing required overflow makes the row incomplete; and a claimed
//! `completed` disposition is downgraded deterministically when synthesis,
//! verification, cleanup, boundary, contradiction or rejected-claim evidence
//! does not support it.
//!
//! Volatile durations, PIDs, scratch roots and API request IDs stay outside
//! this contract entirely: they are host-local telemetry, never portable
//! semantic identity. The one retained host-local spelling is each claim's
//! `worktree_root`, which binds the row digest (retained evidence) but never
//! the portable identity, so equivalent roots at equivalent inputs share one
//! portable identity.
//!
//! The validator runs no agent, selects no work and decides no strategy
//! verdict; a counted row claims only that this contract's identity,
//! disposition, denominator, verification and cleanup evidence is complete
//! and internally consistent.

use serde::{Deserialize, Serialize};

pub(crate) const ORCHESTRATION_ATTEMPT_SCHEMA_VERSION: &str = "orchestration_attempt.v1";
pub(crate) const ORCHESTRATION_CORPUS_SCHEMA_VERSION: &str = "orchestration_corpus.v1";
pub(crate) const ORCHESTRATION_FIXTURE_CORPUS_SCHEMA_VERSION: &str =
    "orchestration_fixture_corpus.v1";
/// One attempt result is rejected as over-budget above this byte bound. The
/// bound governs the receipt row, not the underlying artifact: a producer with
/// a legitimately larger result retains it by reference instead of bytes.
pub(crate) const ORCHESTRATION_RESULT_BYTE_BUDGET: u64 = 1_073_741_824;
pub(crate) const ORCHESTRATION_CLAIM_BOUNDARY: &str = "Static orchestration attempt receipt \
 contract: a counted row defines countable, auditable orchestration evidence only; it claims no \
 orchestration usefulness, no speed or safety advantage, no strategy preference and no parent \
 acceptance.";

/// The closed strategy vocabulary (#4925). `SingleAgent` is a first-class
/// decision: a row that preferred one agent after rejected fan-out remains
/// visible in the scorecard instead of disappearing from denominators.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum AttemptStrategyV1 {
    SingleAgent,
    ReadOnlyFanout,
    ScoutsPlusAdversary,
    BuilderPlusVerifier,
    ValidatedParallelWriters,
}

impl AttemptStrategyV1 {
    /// Every strategy, in canonical declaration order, so projections keep a
    /// stable row even for strategies with zero attempts.
    pub(crate) fn all() -> [Self; 5] {
        [
            Self::SingleAgent,
            Self::ReadOnlyFanout,
            Self::ScoutsPlusAdversary,
            Self::BuilderPlusVerifier,
            Self::ValidatedParallelWriters,
        ]
    }
}

/// The closed disposition vocabulary (#4925). Only `Completed` is the
/// positive terminal; every other disposition stays visible in projections
/// and no aggregate rate converts them into success.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum AttemptDispositionV1 {
    Completed,
    Partial,
    Blocked,
    Contradicted,
    VerificationFailed,
    Stale,
    BoundaryViolation,
    InstrumentFailure,
    NotRun,
}

/// How an independent verification compared the builder result against the
/// accepted contract expectation.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum AttemptComparisonV1 {
    Matched,
    NearMatched,
    NonComparative,
    Incomparable,
}

/// Stable snake_case label used in human-facing reason text.
pub(crate) fn comparison_label(comparison: AttemptComparisonV1) -> &'static str {
    match comparison {
        AttemptComparisonV1::Matched => "matched",
        AttemptComparisonV1::NearMatched => "near_matched",
        AttemptComparisonV1::NonComparative => "non_comparative",
        AttemptComparisonV1::Incomparable => "incomparable",
    }
}

/// One role in the orchestration cast. Roles observe; the scorecard decides
/// counting, so one work item seen through several roles stays one attempt.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum OrchestrationRoleV1 {
    Scout,
    Adversary,
    Builder,
    Verifier,
    Synthesizer,
}

/// Claim lifecycle state. `VerifiedFact` is gated by the matching independent
/// receipt law; a builder claim can never promote itself.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum OrchestrationClaimStateV1 {
    Open,
    VerifiedFact,
    Rejected,
    Interrupted,
}

/// Boundary status of one changed path against the claim edit cage and the
/// repository boundary.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum OrchestrationBoundaryStatusV1 {
    WithinCage,
    OutsideCage,
    ForbiddenPath,
}

/// Repository / source / selected-work / portfolio / base / head identity. The
/// opaque `source` names where the selected work came from (issue, campaign
/// directive, manual intake) and stays distinct from `selected_work`, so two
/// attempts on identical work from different sources remain distinct. This is
/// the shared context identity later extensions (the issue-lifecycle family)
/// reference instead of restating.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct OrchestrationWorkRefV1 {
    pub repository: String,
    pub source: String,
    pub selected_work: String,
    pub portfolio: String,
    pub base: String,
    pub head: String,
}

/// Agent client and role configuration retained for audit.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct OrchestrationClientRefV1 {
    pub agent_client: String,
    pub role_configuration: String,
}

/// One retained evidence reference: exact identity plus retained byte count.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct OrchestrationEvidenceRefV1 {
    pub identity: String,
    pub bytes: u64,
}

/// Overflow disclosure: a required overflow that was not retrieved makes the
/// row incomplete; a non-required omitted overflow is disclosed, not hidden.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct OrchestrationOverflowRefV1 {
    pub required: bool,
    pub evidence: Option<OrchestrationEvidenceRefV1>,
}

/// One claim with its worktree spelling (host-local retained evidence),
/// edit cage and resource bindings.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct OrchestrationClaimRefV1 {
    pub claim_id: String,
    pub role: OrchestrationRoleV1,
    pub state: OrchestrationClaimStateV1,
    /// Host-local concrete root spelling. Retained evidence only: it binds
    /// the row digest but never the portable identity.
    pub worktree_root: String,
    pub edit_cage: Vec<String>,
    pub resources: Vec<String>,
}

/// One command denominator. `subject_count` is the load-bearing denominator:
/// a passing command with zero subjects is not verification.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct OrchestrationCommandDenominatorV1 {
    pub command: String,
    pub subject_count: u64,
    pub passed: bool,
}

/// Independent verification binding. The bound base/head/result identities
/// must match the attempt row or the row is stale, never completed.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct OrchestrationVerificationV1 {
    pub verification_id: String,
    pub bound_base: String,
    pub bound_head: String,
    pub bound_result_identity: String,
    pub commands: Vec<OrchestrationCommandDenominatorV1>,
    /// Whether a matching independent receipt exists; the builder's own
    /// re-run is never independent.
    pub independent_receipt: bool,
    pub comparison: AttemptComparisonV1,
}

/// One changed path and its boundary status.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct OrchestrationChangedPathV1 {
    pub path: String,
    pub boundary_status: OrchestrationBoundaryStatusV1,
}

/// PR / review / CI / merge state where applicable. All four stay optional:
/// a read-only fan-out legitimately has none.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct OrchestrationDeliveryV1 {
    pub pr: Option<String>,
    pub review: Option<String>,
    pub ci: Option<String>,
    pub merge: Option<String>,
}

/// Cleanup evidence. Residue with a completed claim downgrades the row to
/// `partial`; residue is retained, never erased.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct OrchestrationCleanupV1 {
    pub cleaned: bool,
    pub residue: Vec<String>,
}

/// One governed orchestration attempt row. Every real row requires exact
/// selected-work, source, role, result, verification and cleanup identities;
/// `row_digest` binds the retained surface (including host-local worktree
/// spellings) and `observation_key` is the deduplication key for repeated
/// observations of one attempt.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct OrchestrationAttemptV1 {
    pub schema_version: String,
    pub attempt_id: String,
    pub work: OrchestrationWorkRefV1,
    pub task_family: String,
    pub accepted_contract: String,
    pub client: OrchestrationClientRefV1,
    pub strategy: AttemptStrategyV1,
    /// The producer-claimed disposition; the validator may downgrade it and
    /// records every downgrade reason, but never upgrades it.
    pub disposition: AttemptDispositionV1,
    pub planned_waves: u32,
    pub actual_waves: u32,
    pub packet: OrchestrationEvidenceRefV1,
    pub result: OrchestrationEvidenceRefV1,
    pub synthesis: Option<OrchestrationEvidenceRefV1>,
    pub overflow: Vec<OrchestrationOverflowRefV1>,
    pub claims: Vec<OrchestrationClaimRefV1>,
    pub commands: Vec<OrchestrationCommandDenominatorV1>,
    pub independent_verification: Option<OrchestrationVerificationV1>,
    pub contradictions: Vec<String>,
    pub rejected_claims: Vec<String>,
    pub changed_paths: Vec<OrchestrationChangedPathV1>,
    pub delivery: Option<OrchestrationDeliveryV1>,
    pub cleanup: OrchestrationCleanupV1,
    pub limitations: Vec<String>,
    pub non_claims: Vec<String>,
    /// Synthetic mechanics evidence (fixtures) never enters real denominators.
    pub synthetic: bool,
    pub observation_key: String,
    /// SHA-256 hex over the canonical retained surface; recomputed by the
    /// validator and rejected on mismatch.
    pub row_digest: String,
}

/// A plain corpus of attempt rows (the real-use input shape).
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct OrchestrationCorpusV1 {
    pub schema_version: String,
    pub rows: Vec<OrchestrationAttemptV1>,
}

/// The validator's decision for one row. `counted` rows carry the disposition
/// after deterministic downgrades; rejected rows stay visible through
/// `reasons` and can never enter any denominator.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct OrchestrationRowAssessmentV1 {
    pub attempt_id: String,
    pub observation_key: String,
    pub synthetic: bool,
    pub portable_identity: String,
    pub counted: bool,
    pub disposition: Option<AttemptDispositionV1>,
    pub strategy: AttemptStrategyV1,
    pub reasons: Vec<String>,
}

/// The retained-claim digest surface: every claim field, including the
/// host-local worktree spelling. Field order is the canonical digest identity.
#[derive(Serialize)]
struct OrchestrationClaimDigestInput<'a> {
    claim_id: &'a str,
    role: OrchestrationRoleV1,
    state: OrchestrationClaimStateV1,
    worktree_root: &'a str,
    edit_cage: &'a [String],
    resources: &'a [String],
}

/// The portable claim surface: the worktree spelling is excluded so
/// equivalent roots share one portable identity.
#[derive(Serialize)]
struct OrchestrationClaimPortableInput<'a> {
    claim_id: &'a str,
    role: OrchestrationRoleV1,
    state: OrchestrationClaimStateV1,
    edit_cage: &'a [String],
    resources: &'a [String],
}

/// The retained row digest surface: every row field except `row_digest`
/// itself. Field order is the canonical digest identity.
#[derive(Serialize)]
struct OrchestrationAttemptDigestInput<'a> {
    schema_version: &'a str,
    attempt_id: &'a str,
    work: &'a OrchestrationWorkRefV1,
    task_family: &'a str,
    accepted_contract: &'a str,
    client: &'a OrchestrationClientRefV1,
    strategy: AttemptStrategyV1,
    disposition: AttemptDispositionV1,
    planned_waves: u32,
    actual_waves: u32,
    packet: &'a OrchestrationEvidenceRefV1,
    result: &'a OrchestrationEvidenceRefV1,
    synthesis: Option<&'a OrchestrationEvidenceRefV1>,
    overflow: &'a [OrchestrationOverflowRefV1],
    claims: Vec<OrchestrationClaimDigestInput<'a>>,
    commands: &'a [OrchestrationCommandDenominatorV1],
    independent_verification: Option<&'a OrchestrationVerificationV1>,
    contradictions: &'a [String],
    rejected_claims: &'a [String],
    changed_paths: &'a [OrchestrationChangedPathV1],
    delivery: Option<&'a OrchestrationDeliveryV1>,
    cleanup: &'a OrchestrationCleanupV1,
    limitations: &'a [String],
    non_claims: &'a [String],
    synthetic: bool,
    observation_key: &'a str,
}

/// The portable row surface: every digest field except each claim's
/// `worktree_root`, so equivalent roots at equivalent inputs share one
/// portable identity while concrete root evidence stays retained.
#[derive(Serialize)]
struct OrchestrationAttemptPortableInput<'a> {
    schema_version: &'a str,
    attempt_id: &'a str,
    work: &'a OrchestrationWorkRefV1,
    task_family: &'a str,
    accepted_contract: &'a str,
    client: &'a OrchestrationClientRefV1,
    strategy: AttemptStrategyV1,
    disposition: AttemptDispositionV1,
    planned_waves: u32,
    actual_waves: u32,
    packet: &'a OrchestrationEvidenceRefV1,
    result: &'a OrchestrationEvidenceRefV1,
    synthesis: Option<&'a OrchestrationEvidenceRefV1>,
    overflow: &'a [OrchestrationOverflowRefV1],
    claims: Vec<OrchestrationClaimPortableInput<'a>>,
    commands: &'a [OrchestrationCommandDenominatorV1],
    independent_verification: Option<&'a OrchestrationVerificationV1>,
    contradictions: &'a [String],
    rejected_claims: &'a [String],
    changed_paths: &'a [OrchestrationChangedPathV1],
    delivery: Option<&'a OrchestrationDeliveryV1>,
    cleanup: &'a OrchestrationCleanupV1,
    limitations: &'a [String],
    non_claims: &'a [String],
    synthetic: bool,
    observation_key: &'a str,
}

fn canonical_json<T: Serialize>(value: &T) -> Result<String, String> {
    serde_json::to_string(value).map_err(|error| format!("canonical serialization failed: {error}"))
}

fn claim_digest_inputs(row: &OrchestrationAttemptV1) -> Vec<OrchestrationClaimDigestInput<'_>> {
    row.claims
        .iter()
        .map(|claim| OrchestrationClaimDigestInput {
            claim_id: &claim.claim_id,
            role: claim.role,
            state: claim.state,
            worktree_root: &claim.worktree_root,
            edit_cage: &claim.edit_cage,
            resources: &claim.resources,
        })
        .collect()
}

fn claim_portable_inputs(row: &OrchestrationAttemptV1) -> Vec<OrchestrationClaimPortableInput<'_>> {
    row.claims
        .iter()
        .map(|claim| OrchestrationClaimPortableInput {
            claim_id: &claim.claim_id,
            role: claim.role,
            state: claim.state,
            edit_cage: &claim.edit_cage,
            resources: &claim.resources,
        })
        .collect()
}

/// SHA-256 hex over the canonical retained row surface.
pub(crate) fn orchestration_row_digest(row: &OrchestrationAttemptV1) -> Result<String, String> {
    let input = OrchestrationAttemptDigestInput {
        schema_version: &row.schema_version,
        attempt_id: &row.attempt_id,
        work: &row.work,
        task_family: &row.task_family,
        accepted_contract: &row.accepted_contract,
        client: &row.client,
        strategy: row.strategy,
        disposition: row.disposition,
        planned_waves: row.planned_waves,
        actual_waves: row.actual_waves,
        packet: &row.packet,
        result: &row.result,
        synthesis: row.synthesis.as_ref(),
        overflow: &row.overflow,
        claims: claim_digest_inputs(row),
        commands: &row.commands,
        independent_verification: row.independent_verification.as_ref(),
        contradictions: &row.contradictions,
        rejected_claims: &row.rejected_claims,
        changed_paths: &row.changed_paths,
        delivery: row.delivery.as_ref(),
        cleanup: &row.cleanup,
        limitations: &row.limitations,
        non_claims: &row.non_claims,
        synthetic: row.synthetic,
        observation_key: &row.observation_key,
    };
    Ok(crate::blind_journey::sha256_hex(
        canonical_json(&input)?.as_bytes(),
    ))
}

/// Portable semantic identity of one row: the retained surface minus each
/// claim's concrete worktree spelling. Equivalent roots therefore share one
/// portable identity while concrete root evidence remains retained.
pub(crate) fn orchestration_portable_identity(
    row: &OrchestrationAttemptV1,
) -> Result<String, String> {
    let input = OrchestrationAttemptPortableInput {
        schema_version: &row.schema_version,
        attempt_id: &row.attempt_id,
        work: &row.work,
        task_family: &row.task_family,
        accepted_contract: &row.accepted_contract,
        client: &row.client,
        strategy: row.strategy,
        disposition: row.disposition,
        planned_waves: row.planned_waves,
        actual_waves: row.actual_waves,
        packet: &row.packet,
        result: &row.result,
        synthesis: row.synthesis.as_ref(),
        overflow: &row.overflow,
        claims: claim_portable_inputs(row),
        commands: &row.commands,
        independent_verification: row.independent_verification.as_ref(),
        contradictions: &row.contradictions,
        rejected_claims: &row.rejected_claims,
        changed_paths: &row.changed_paths,
        delivery: row.delivery.as_ref(),
        cleanup: &row.cleanup,
        limitations: &row.limitations,
        non_claims: &row.non_claims,
        synthetic: row.synthetic,
        observation_key: &row.observation_key,
    };
    Ok(crate::blind_journey::sha256_hex(
        canonical_json(&input)?.as_bytes(),
    ))
}

/// Fail-closed counting-law assessment of one committed row. The producer's
/// claimed disposition is never upgraded; every downgrade reason is retained.
pub(crate) fn assess_orchestration_attempt(
    row: &OrchestrationAttemptV1,
) -> OrchestrationRowAssessmentV1 {
    // Serialization of this contract's own DTOs cannot fail in practice; keep
    // a named state instead of inventing an identity.
    let portable_identity = match orchestration_portable_identity(row) {
        Ok(identity) => identity,
        Err(error) => format!("portable_identity_error:{error}"),
    };
    let base = |counted: bool, disposition: Option<AttemptDispositionV1>, reasons: Vec<String>| {
        OrchestrationRowAssessmentV1 {
            attempt_id: row.attempt_id.clone(),
            observation_key: row.observation_key.clone(),
            synthetic: row.synthetic,
            portable_identity: portable_identity.clone(),
            counted,
            disposition,
            strategy: row.strategy,
            reasons,
        }
    };
    if row.schema_version != ORCHESTRATION_ATTEMPT_SCHEMA_VERSION {
        return base(
            false,
            None,
            vec![format!(
                "schema_version `{}` is not supported",
                row.schema_version
            )],
        );
    }
    match orchestration_row_digest(row) {
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
    if row.attempt_id.trim().is_empty() {
        missing.push("attempt_id");
    }
    if row.work.repository.trim().is_empty() {
        missing.push("work.repository");
    }
    if row.work.source.trim().is_empty() {
        missing.push("work.source");
    }
    if row.work.selected_work.trim().is_empty() {
        missing.push("work.selected_work");
    }
    if row.work.portfolio.trim().is_empty() {
        missing.push("work.portfolio");
    }
    if row.work.base.trim().is_empty() {
        missing.push("work.base");
    }
    if row.work.head.trim().is_empty() {
        missing.push("work.head");
    }
    if row.task_family.trim().is_empty() {
        missing.push("task_family");
    }
    if row.accepted_contract.trim().is_empty() {
        missing.push("accepted_contract");
    }
    if row.client.agent_client.trim().is_empty() {
        missing.push("client.agent_client");
    }
    if row.client.role_configuration.trim().is_empty() {
        missing.push("client.role_configuration");
    }
    if row.observation_key.trim().is_empty() {
        missing.push("observation_key");
    }
    if row.packet.identity.trim().is_empty() {
        missing.push("packet.identity");
    }
    if row.result.identity.trim().is_empty() {
        missing.push("result.identity");
    }
    if row
        .synthesis
        .as_ref()
        .is_some_and(|evidence| evidence.identity.trim().is_empty())
    {
        missing.push("synthesis.identity");
    }
    if row
        .independent_verification
        .as_ref()
        .is_some_and(|verification| verification.verification_id.trim().is_empty())
    {
        missing.push("independent_verification.verification_id");
    }
    if row
        .claims
        .iter()
        .any(|claim| claim.claim_id.trim().is_empty())
    {
        missing.push("claims.claim_id");
    }
    if !missing.is_empty() {
        return base(
            false,
            None,
            vec![format!("missing required identities: {missing:?}")],
        );
    }
    if row.packet.bytes == 0 {
        return base(
            false,
            None,
            vec!["packet evidence is malformed: zero bytes".to_string()],
        );
    }
    if row.result.bytes == 0 {
        return base(
            false,
            None,
            vec!["result evidence is malformed: zero bytes".to_string()],
        );
    }
    if row.result.bytes > ORCHESTRATION_RESULT_BYTE_BUDGET {
        return base(
            false,
            None,
            vec![format!(
                "result evidence is over budget: {} bytes exceed the {} byte bound",
                row.result.bytes, ORCHESTRATION_RESULT_BYTE_BUDGET
            )],
        );
    }
    for (index, overflow) in row.overflow.iter().enumerate() {
        if let Some(evidence) = &overflow.evidence {
            if evidence.identity.trim().is_empty() {
                return base(
                    false,
                    None,
                    vec![format!("overflow evidence {index} lacks an exact identity")],
                );
            }
            if evidence.bytes == 0 {
                return base(
                    false,
                    None,
                    vec![format!(
                        "overflow evidence {index} is malformed: zero bytes"
                    )],
                );
            }
        }
    }

    let claimed = row.disposition;
    let completion_claimed = matches!(
        claimed,
        AttemptDispositionV1::Completed | AttemptDispositionV1::Partial
    );
    let mut reasons: Vec<String> = Vec::new();
    let mut disposition = claimed;

    if completion_claimed {
        if row
            .changed_paths
            .iter()
            .any(|path| path.boundary_status == OrchestrationBoundaryStatusV1::ForbiddenPath)
        {
            let forbidden: Vec<&str> = row
                .changed_paths
                .iter()
                .filter(|path| path.boundary_status == OrchestrationBoundaryStatusV1::ForbiddenPath)
                .map(|path| path.path.as_str())
                .collect();
            reasons.push(format!(
                "forbidden path change {forbidden:?} blocks a completed disposition"
            ));
            disposition = AttemptDispositionV1::BoundaryViolation;
        } else if !row.contradictions.is_empty() {
            reasons.push(format!(
                "unresolved contradiction {:?} blocks a completed disposition",
                row.contradictions
            ));
            disposition = AttemptDispositionV1::Contradicted;
        }
    }

    // The verified_fact law is universal: no claim state may assert a fact
    // without a matching independent receipt, whatever the claimed disposition.
    let verified_fact_receipted =
        row.independent_verification
            .as_ref()
            .is_some_and(|verification| {
                verification.independent_receipt
                    && verification.comparison == AttemptComparisonV1::Matched
            });
    if !verified_fact_receipted {
        for claim in &row.claims {
            if claim.state == OrchestrationClaimStateV1::VerifiedFact {
                reasons.push(format!(
                    "verified_fact claim `{}` lacks a matching independent receipt",
                    claim.claim_id
                ));
                if disposition == AttemptDispositionV1::Completed {
                    disposition = AttemptDispositionV1::VerificationFailed;
                }
            }
        }
    }

    if claimed == AttemptDispositionV1::Completed {
        disposition = match (&row.independent_verification, disposition) {
            (None, current) => {
                if current == AttemptDispositionV1::Completed {
                    reasons.push(
                        "completed disposition requires independent verification".to_string(),
                    );
                    AttemptDispositionV1::Blocked
                } else {
                    current
                }
            }
            (Some(verification), current) => {
                // The first established downgrade wins: a boundary violation
                // or contradiction recorded above is never overwritten by a
                // later verification signal, so combined failures keep their
                // strongest disposition regardless of check order.
                let mut next = current;
                let downgrade = |next: &mut AttemptDispositionV1, to: AttemptDispositionV1| {
                    if *next == AttemptDispositionV1::Completed {
                        *next = to;
                    }
                };
                if verification.comparison != AttemptComparisonV1::Matched {
                    reasons.push(format!(
                        "verification comparison `{}` is not matched",
                        comparison_label(verification.comparison)
                    ));
                    downgrade(&mut next, AttemptDispositionV1::VerificationFailed);
                }
                if verification.commands.is_empty() {
                    reasons.push("no command denominators recorded for verification".to_string());
                    downgrade(&mut next, AttemptDispositionV1::VerificationFailed);
                }
                for command in &verification.commands {
                    if !command.passed {
                        reasons.push(format!("verification command `{}` failed", command.command));
                        downgrade(&mut next, AttemptDispositionV1::VerificationFailed);
                    } else if command.subject_count == 0 {
                        reasons.push(format!(
                            "passing command `{}` has zero subjects and is not verification",
                            command.command
                        ));
                        downgrade(&mut next, AttemptDispositionV1::VerificationFailed);
                    }
                }
                if verification.bound_base != row.work.base
                    || verification.bound_head != row.work.head
                    || verification.bound_result_identity != row.result.identity
                {
                    reasons.push(
                        "verification binds a stale base, head or result identity".to_string(),
                    );
                    downgrade(&mut next, AttemptDispositionV1::Stale);
                }
                if !verification.independent_receipt {
                    reasons.push("verification is not independently receipted".to_string());
                    downgrade(&mut next, AttemptDispositionV1::VerificationFailed);
                }
                // Completion gating. Reasons record every blocking condition;
                // the downgrade helper only moves a still-Completed row.
                if row.synthesis.is_none() {
                    reasons.push("completed disposition requires synthesis evidence".to_string());
                    downgrade(&mut next, AttemptDispositionV1::Blocked);
                } else if row
                    .overflow
                    .iter()
                    .any(|overflow| overflow.required && overflow.evidence.is_none())
                {
                    reasons.push("required overflow is missing".to_string());
                    downgrade(&mut next, AttemptDispositionV1::Blocked);
                }
                if !row.rejected_claims.is_empty() {
                    reasons.push(format!(
                        "rejected claim {:?} blocks a completed disposition",
                        row.rejected_claims
                    ));
                    downgrade(&mut next, AttemptDispositionV1::VerificationFailed);
                }
                if !row.cleanup.cleaned || !row.cleanup.residue.is_empty() {
                    reasons.push(
                        "cleanup residue remains; completed downgraded to partial".to_string(),
                    );
                    downgrade(&mut next, AttemptDispositionV1::Partial);
                }
                next
            }
        };
    }

    base(true, Some(disposition), reasons)
}

/// Scenario expectation recorded in the committed fixture corpus.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
pub(crate) struct OrchestrationFixtureExpectationV1 {
    pub countable: bool,
    pub disposition: Option<AttemptDispositionV1>,
    /// Every entry must appear verbatim in the assessed row reasons.
    pub reason_contains: Vec<String>,
    pub same_portable_identity_as: Option<String>,
}

/// One committed fixture scenario: an expected counting outcome plus one
/// full attempt row.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct OrchestrationFixtureScenarioV1 {
    pub id: String,
    pub expected: OrchestrationFixtureExpectationV1,
    pub attempt: OrchestrationAttemptV1,
}

/// The committed orchestration fixture corpus (RIPR-SPEC-0213).
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct OrchestrationFixtureCorpusV1 {
    pub schema_version: String,
    pub scenarios: Vec<OrchestrationFixtureScenarioV1>,
}

/// The fixture scenarios #4925 requires; the committed corpus must cover all
/// of them and the live validator decides each outcome independently.
pub(crate) const REQUIRED_ORCHESTRATION_SCENARIO_IDS: [&str; 13] = [
    "single_agent_preferred_narrow_task",
    "read_only_fanout_bounded_overflow",
    "adversary_contradiction_blocks_dependent_result",
    "builder_claim_rejected_by_verifier",
    "parallel_writers_semantic_conflict_disjoint_files",
    "stale_portfolio_base_result_identity",
    "root_worktree_contamination_forbidden_path",
    "malformed_over_budget_result",
    "interrupted_claim_worktree_cleanup_residue",
    "duplicate_observation_a",
    "duplicate_observation_b",
    "equivalent_root_a",
    "equivalent_root_b",
];

/// Required scenario ids absent from one committed corpus id set.
pub(crate) fn missing_orchestration_required_scenarios<'a>(
    present: impl IntoIterator<Item = &'a str>,
) -> Vec<&'static str> {
    let present: std::collections::BTreeSet<&str> = present.into_iter().collect();
    REQUIRED_ORCHESTRATION_SCENARIO_IDS
        .iter()
        .filter(|required| !present.contains(**required))
        .copied()
        .collect()
}

/// Load and parse one committed fixture corpus.
pub(crate) fn load_orchestration_fixture_corpus(
    body: &str,
) -> Result<OrchestrationFixtureCorpusV1, String> {
    let corpus: OrchestrationFixtureCorpusV1 = serde_json::from_str(body)
        .map_err(|error| format!("parse orchestration fixture corpus: {error}"))?;
    if corpus.schema_version != ORCHESTRATION_FIXTURE_CORPUS_SCHEMA_VERSION {
        return Err(format!(
            "unsupported orchestration fixture corpus schema `{}`",
            corpus.schema_version
        ));
    }
    let mut ids = std::collections::BTreeSet::new();
    for scenario in &corpus.scenarios {
        if !ids.insert(scenario.id.clone()) {
            return Err(format!(
                "duplicate orchestration fixture scenario id `{}`",
                scenario.id
            ));
        }
    }
    Ok(corpus)
}

/// Load and parse one plain orchestration corpus.
pub(crate) fn load_orchestration_corpus(body: &str) -> Result<OrchestrationCorpusV1, String> {
    let corpus: OrchestrationCorpusV1 = serde_json::from_str(body)
        .map_err(|error| format!("parse orchestration corpus: {error}"))?;
    if corpus.schema_version != ORCHESTRATION_CORPUS_SCHEMA_VERSION {
        return Err(format!(
            "unsupported orchestration corpus schema `{}`",
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

    fn sample_attempt() -> OrchestrationAttemptV1 {
        OrchestrationAttemptV1 {
            schema_version: ORCHESTRATION_ATTEMPT_SCHEMA_VERSION.to_string(),
            attempt_id: "attempt-sample".to_string(),
            work: OrchestrationWorkRefV1 {
                repository: "https://example.invalid/operator/target".to_string(),
                source: "source-sample".to_string(),
                selected_work: "issue-sample".to_string(),
                portfolio: "campaign-sample".to_string(),
                base: "base-sha".to_string(),
                head: "head-sha".to_string(),
            },
            task_family: "narrow_bug".to_string(),
            accepted_contract: "accepted-contract-sample".to_string(),
            client: OrchestrationClientRefV1 {
                agent_client: "agent-cli-1.0".to_string(),
                role_configuration: "single_builder".to_string(),
            },
            strategy: AttemptStrategyV1::SingleAgent,
            disposition: AttemptDispositionV1::Completed,
            planned_waves: 1,
            actual_waves: 1,
            packet: OrchestrationEvidenceRefV1 {
                identity: format!("packet:sha256:{}", hex64('a')),
                bytes: 2048,
            },
            result: OrchestrationEvidenceRefV1 {
                identity: format!("result:sha256:{}", hex64('b')),
                bytes: 4096,
            },
            synthesis: Some(OrchestrationEvidenceRefV1 {
                identity: format!("synthesis:sha256:{}", hex64('c')),
                bytes: 512,
            }),
            overflow: vec![
                OrchestrationOverflowRefV1 {
                    required: true,
                    evidence: Some(OrchestrationEvidenceRefV1 {
                        identity: format!("overflow:sha256:{}", hex64('d')),
                        bytes: 256,
                    }),
                },
                OrchestrationOverflowRefV1 {
                    required: false,
                    evidence: None,
                },
            ],
            claims: vec![OrchestrationClaimRefV1 {
                claim_id: "claim-sample".to_string(),
                role: OrchestrationRoleV1::Builder,
                state: OrchestrationClaimStateV1::Open,
                worktree_root: "/srv/orchestration/worktree-sample".to_string(),
                edit_cage: vec!["src/lib.rs".to_string()],
                resources: vec!["cpu:1".to_string()],
            }],
            commands: vec![OrchestrationCommandDenominatorV1 {
                command: "cargo test -p sample".to_string(),
                subject_count: 3,
                passed: true,
            }],
            independent_verification: Some(OrchestrationVerificationV1 {
                verification_id: "verification-sample".to_string(),
                bound_base: "base-sha".to_string(),
                bound_head: "head-sha".to_string(),
                bound_result_identity: format!("result:sha256:{}", hex64('b')),
                commands: vec![OrchestrationCommandDenominatorV1 {
                    command: "cargo test -p sample".to_string(),
                    subject_count: 3,
                    passed: true,
                }],
                independent_receipt: true,
                comparison: AttemptComparisonV1::Matched,
            }),
            contradictions: Vec::new(),
            rejected_claims: Vec::new(),
            changed_paths: vec![OrchestrationChangedPathV1 {
                path: "src/lib.rs".to_string(),
                boundary_status: OrchestrationBoundaryStatusV1::WithinCage,
            }],
            delivery: Some(OrchestrationDeliveryV1 {
                pr: Some("pr-sample".to_string()),
                review: Some("review-sample".to_string()),
                ci: Some("ci-sample".to_string()),
                merge: Some("merge-sample".to_string()),
            }),
            cleanup: OrchestrationCleanupV1 {
                cleaned: true,
                residue: Vec::new(),
            },
            limitations: vec!["sample limitation".to_string()],
            non_claims: vec!["sample non-claim".to_string()],
            synthetic: false,
            observation_key: "observation-sample".to_string(),
            row_digest: String::new(),
        }
    }

    fn stamped_sample() -> Result<OrchestrationAttemptV1, String> {
        let mut row = sample_attempt();
        row.row_digest = orchestration_row_digest(&row)?;
        Ok(row)
    }

    fn reason_contains(assessment: &OrchestrationRowAssessmentV1, needle: &str) -> bool {
        assessment
            .reasons
            .iter()
            .any(|reason| reason.contains(needle))
    }

    #[test]
    fn complete_single_agent_row_counts_as_completed() -> Result<(), String> {
        let row = stamped_sample()?;
        let assessment = assess_orchestration_attempt(&row);
        if !assessment.counted {
            return Err(format!(
                "valid row was not counted: {:?}",
                assessment.reasons
            ));
        }
        if assessment.disposition != Some(AttemptDispositionV1::Completed) {
            return Err(format!(
                "valid row downgraded unexpectedly: {:?}",
                assessment.disposition
            ));
        }
        if assessment.strategy != AttemptStrategyV1::SingleAgent {
            return Err("the single-agent strategy must remain first-class".to_string());
        }
        Ok(())
    }

    #[test]
    fn schema_drift_rejects_the_row() -> Result<(), String> {
        let mut row = stamped_sample()?;
        row.schema_version = "orchestration_attempt.v2".to_string();
        row.row_digest = orchestration_row_digest(&row)?;
        let assessment = assess_orchestration_attempt(&row);
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
        row.actual_waves = 7;
        // row_digest intentionally not recomputed: the binding must catch it.
        let assessment = assess_orchestration_attempt(&row);
        if assessment.counted || !reason_contains(&assessment, "row_digest mismatch") {
            return Err(format!(
                "an altered row must reject on its digest binding, got counted={} reasons={:?}",
                assessment.counted, assessment.reasons
            ));
        }
        Ok(())
    }

    #[test]
    fn equivalent_worktree_roots_share_portable_identity() -> Result<(), String> {
        let mut other = stamped_sample()?;
        other.claims[0].worktree_root = "/var/orchestration/worktree-sample".to_string();
        other.row_digest = orchestration_row_digest(&other)?;
        let first = orchestration_portable_identity(&stamped_sample()?)?;
        let second = orchestration_portable_identity(&other)?;
        if first != second {
            return Err(
                "equivalent roots must share one portable identity while row digests differ"
                    .to_string(),
            );
        }
        if orchestration_row_digest(&stamped_sample()?) == orchestration_row_digest(&other) {
            return Err("retained root evidence must keep distinct row digests".to_string());
        }
        Ok(())
    }

    #[test]
    fn zero_subject_passing_command_is_not_verification() -> Result<(), String> {
        let mut row = stamped_sample()?;
        let verification = row
            .independent_verification
            .as_mut()
            .ok_or_else(|| "sample verification missing".to_string())?;
        verification.commands[0].subject_count = 0;
        row.commands[0].subject_count = 0;
        row.row_digest = orchestration_row_digest(&row)?;
        let assessment = assess_orchestration_attempt(&row);
        if assessment.disposition != Some(AttemptDispositionV1::VerificationFailed)
            || !reason_contains(&assessment, "zero subjects")
        {
            return Err(format!(
                "zero-subject verification must fail closed, got {:?} reasons={:?}",
                assessment.disposition, assessment.reasons
            ));
        }
        Ok(())
    }

    #[test]
    fn failed_verification_command_is_not_completed() -> Result<(), String> {
        let mut row = stamped_sample()?;
        let verification = row
            .independent_verification
            .as_mut()
            .ok_or_else(|| "sample verification missing".to_string())?;
        verification.commands[0].passed = false;
        row.row_digest = orchestration_row_digest(&row)?;
        let assessment = assess_orchestration_attempt(&row);
        if assessment.disposition != Some(AttemptDispositionV1::VerificationFailed) {
            return Err(format!(
                "a failed verification command must not complete, got {:?}",
                assessment.disposition
            ));
        }
        Ok(())
    }

    #[test]
    fn builder_claim_cannot_become_verified_fact_without_independent_receipt() -> Result<(), String>
    {
        let mut row = stamped_sample()?;
        row.claims[0].state = OrchestrationClaimStateV1::VerifiedFact;
        let verification = row
            .independent_verification
            .as_mut()
            .ok_or_else(|| "sample verification missing".to_string())?;
        verification.independent_receipt = false;
        row.row_digest = orchestration_row_digest(&row)?;
        let assessment = assess_orchestration_attempt(&row);
        if assessment.disposition != Some(AttemptDispositionV1::VerificationFailed)
            || !reason_contains(&assessment, "independent receipt")
        {
            return Err(format!(
                "a self-receipted verified_fact claim must fail closed, got {:?} reasons={:?}",
                assessment.disposition, assessment.reasons
            ));
        }
        Ok(())
    }

    #[test]
    fn missing_required_overflow_blocks_completion() -> Result<(), String> {
        let mut row = stamped_sample()?;
        row.overflow[0].evidence = None;
        row.row_digest = orchestration_row_digest(&row)?;
        let assessment = assess_orchestration_attempt(&row);
        if assessment.disposition != Some(AttemptDispositionV1::Blocked)
            || !reason_contains(&assessment, "required overflow")
        {
            return Err(format!(
                "missing required overflow must block completion, got {:?} reasons={:?}",
                assessment.disposition, assessment.reasons
            ));
        }
        Ok(())
    }

    #[test]
    fn forbidden_path_change_forces_boundary_violation() -> Result<(), String> {
        let mut row = stamped_sample()?;
        row.changed_paths.push(OrchestrationChangedPathV1 {
            path: "Cargo.lock".to_string(),
            boundary_status: OrchestrationBoundaryStatusV1::ForbiddenPath,
        });
        row.row_digest = orchestration_row_digest(&row)?;
        let assessment = assess_orchestration_attempt(&row);
        if assessment.disposition != Some(AttemptDispositionV1::BoundaryViolation)
            || !reason_contains(&assessment, "forbidden path")
        {
            return Err(format!(
                "a forbidden path change must force boundary_violation, got {:?} reasons={:?}",
                assessment.disposition, assessment.reasons
            ));
        }
        Ok(())
    }

    #[test]
    fn unresolved_contradiction_forces_contradicted() -> Result<(), String> {
        let mut row = stamped_sample()?;
        row.contradictions
            .push("adversary disputes the dependent result".to_string());
        row.row_digest = orchestration_row_digest(&row)?;
        let assessment = assess_orchestration_attempt(&row);
        if assessment.disposition != Some(AttemptDispositionV1::Contradicted)
            || !reason_contains(&assessment, "contradiction")
        {
            return Err(format!(
                "an unresolved contradiction must force contradicted, got {:?} reasons={:?}",
                assessment.disposition, assessment.reasons
            ));
        }
        Ok(())
    }

    #[test]
    fn stale_verification_binding_forces_stale() -> Result<(), String> {
        let mut row = stamped_sample()?;
        let verification = row
            .independent_verification
            .as_mut()
            .ok_or_else(|| "sample verification missing".to_string())?;
        verification.bound_base = "older-base-sha".to_string();
        row.row_digest = orchestration_row_digest(&row)?;
        let assessment = assess_orchestration_attempt(&row);
        if assessment.disposition != Some(AttemptDispositionV1::Stale)
            || !reason_contains(&assessment, "stale")
        {
            return Err(format!(
                "a stale verification binding must force stale, got {:?} reasons={:?}",
                assessment.disposition, assessment.reasons
            ));
        }
        Ok(())
    }

    #[test]
    fn rejected_claims_block_completed() -> Result<(), String> {
        let mut row = stamped_sample()?;
        row.claims[0].state = OrchestrationClaimStateV1::Rejected;
        row.rejected_claims.push("claim-sample".to_string());
        row.row_digest = orchestration_row_digest(&row)?;
        let assessment = assess_orchestration_attempt(&row);
        if assessment.disposition != Some(AttemptDispositionV1::VerificationFailed)
            || !reason_contains(&assessment, "rejected claim")
        {
            return Err(format!(
                "a rejected builder claim must block completion, got {:?} reasons={:?}",
                assessment.disposition, assessment.reasons
            ));
        }
        Ok(())
    }

    #[test]
    fn cleanup_residue_downgrades_completed_to_partial() -> Result<(), String> {
        let mut row = stamped_sample()?;
        row.cleanup
            .residue
            .push("target/debug/leftover".to_string());
        row.row_digest = orchestration_row_digest(&row)?;
        let assessment = assess_orchestration_attempt(&row);
        if assessment.disposition != Some(AttemptDispositionV1::Partial)
            || !reason_contains(&assessment, "residue")
        {
            return Err(format!(
                "cleanup residue must downgrade completed to partial, got {:?} reasons={:?}",
                assessment.disposition, assessment.reasons
            ));
        }
        Ok(())
    }

    #[test]
    fn over_budget_result_rejects_the_row() -> Result<(), String> {
        let mut row = stamped_sample()?;
        row.result.bytes = ORCHESTRATION_RESULT_BYTE_BUDGET + 1;
        row.row_digest = orchestration_row_digest(&row)?;
        let assessment = assess_orchestration_attempt(&row);
        if assessment.counted || !reason_contains(&assessment, "over budget") {
            return Err(format!(
                "an over-budget result must reject, got counted={} reasons={:?}",
                assessment.counted, assessment.reasons
            ));
        }
        Ok(())
    }

    #[test]
    fn malformed_zero_byte_result_rejects_the_row() -> Result<(), String> {
        let mut row = stamped_sample()?;
        row.result.bytes = 0;
        row.row_digest = orchestration_row_digest(&row)?;
        let assessment = assess_orchestration_attempt(&row);
        if assessment.counted || !reason_contains(&assessment, "malformed") {
            return Err(format!(
                "a zero-byte result must reject as malformed, got counted={} reasons={:?}",
                assessment.counted, assessment.reasons
            ));
        }
        Ok(())
    }

    #[test]
    fn missing_independent_verification_blocks_completed() -> Result<(), String> {
        let mut row = stamped_sample()?;
        row.independent_verification = None;
        row.row_digest = orchestration_row_digest(&row)?;
        let assessment = assess_orchestration_attempt(&row);
        if assessment.disposition != Some(AttemptDispositionV1::Blocked) {
            return Err(format!(
                "completed without independent verification must block, got {:?}",
                assessment.disposition
            ));
        }
        Ok(())
    }

    #[test]
    fn non_completed_dispositions_pass_through_and_stay_visible() -> Result<(), String> {
        let mut row = stamped_sample()?;
        row.disposition = AttemptDispositionV1::Blocked;
        row.independent_verification = None;
        row.row_digest = orchestration_row_digest(&row)?;
        let assessment = assess_orchestration_attempt(&row);
        if !assessment.counted || assessment.disposition != Some(AttemptDispositionV1::Blocked) {
            return Err(format!(
                "a blocked row must stay counted and visible, got counted={} disposition={:?}",
                assessment.counted, assessment.disposition
            ));
        }
        Ok(())
    }

    #[test]
    fn synthetic_flag_does_not_change_row_assessment() -> Result<(), String> {
        let mut row = stamped_sample()?;
        row.synthetic = true;
        row.row_digest = orchestration_row_digest(&row)?;
        let assessment = assess_orchestration_attempt(&row);
        if !assessment.counted || assessment.disposition != Some(AttemptDispositionV1::Completed) {
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
                "reason_contains": [], "same_portable_identity_as": null },
            "attempt": serde_json::to_value(&row).map_err(|error| error.to_string())?
        });
        let corpus = serde_json::json!({
            "schema_version": ORCHESTRATION_FIXTURE_CORPUS_SCHEMA_VERSION,
            "scenarios": [scenario.clone(), scenario]
        });
        let body = serde_json::to_string(&corpus).map_err(|error| error.to_string())?;
        match load_orchestration_fixture_corpus(&body) {
            Err(message) if message.contains("duplicate") => Ok(()),
            Err(message) => Err(format!("expected a duplicate-id error, got: {message}")),
            Ok(_corpus) => Err("a corpus with duplicate scenario ids must be rejected".to_string()),
        }
    }

    #[test]
    fn missing_required_scenarios_are_reported() -> Result<(), String> {
        let missing =
            missing_orchestration_required_scenarios(["single_agent_preferred_narrow_task"]);
        if missing.len() != REQUIRED_ORCHESTRATION_SCENARIO_IDS.len() - 1 {
            return Err(format!(
                "expected every other scenario to be reported missing, got {missing:?}"
            ));
        }
        let none = missing_orchestration_required_scenarios(REQUIRED_ORCHESTRATION_SCENARIO_IDS);
        if !none.is_empty() {
            return Err(format!(
                "a complete id set must report nothing missing, got {none:?}"
            ));
        }
        Ok(())
    }

    #[test]
    fn blank_result_identity_rejects_the_row() -> Result<(), String> {
        let mut row = stamped_sample()?;
        row.result.identity = String::new();
        row.row_digest = orchestration_row_digest(&row)?;
        let assessment = assess_orchestration_attempt(&row);
        if assessment.counted || !reason_contains(&assessment, "result.identity") {
            return Err(format!(
                "a blank result identity must reject the row, got counted={} reasons={:?}",
                assessment.counted, assessment.reasons
            ));
        }
        Ok(())
    }

    #[test]
    fn blank_synthesis_identity_rejects_the_row() -> Result<(), String> {
        let mut row = stamped_sample()?;
        row.synthesis = Some(OrchestrationEvidenceRefV1 {
            identity: String::new(),
            bytes: 512,
        });
        row.row_digest = orchestration_row_digest(&row)?;
        let assessment = assess_orchestration_attempt(&row);
        if assessment.counted || !reason_contains(&assessment, "synthesis.identity") {
            return Err(format!(
                "a blank synthesis identity must reject the row, got counted={} reasons={:?}",
                assessment.counted, assessment.reasons
            ));
        }
        Ok(())
    }

    #[test]
    fn blank_claim_id_rejects_the_row() -> Result<(), String> {
        let mut row = stamped_sample()?;
        row.claims[0].claim_id = String::new();
        row.row_digest = orchestration_row_digest(&row)?;
        let assessment = assess_orchestration_attempt(&row);
        if assessment.counted || !reason_contains(&assessment, "claims.claim_id") {
            return Err(format!(
                "a blank claim id must reject the row, got counted={} reasons={:?}",
                assessment.counted, assessment.reasons
            ));
        }
        Ok(())
    }

    #[test]
    fn blank_source_identity_rejects_the_row() -> Result<(), String> {
        let mut row = stamped_sample()?;
        row.work.source = String::new();
        row.row_digest = orchestration_row_digest(&row)?;
        let assessment = assess_orchestration_attempt(&row);
        if assessment.counted || !reason_contains(&assessment, "work.source") {
            return Err(format!(
                "a blank source identity must reject the row, got counted={} reasons={:?}",
                assessment.counted, assessment.reasons
            ));
        }
        Ok(())
    }

    #[test]
    fn non_matched_comparison_blocks_completed() -> Result<(), String> {
        let mut row = stamped_sample()?;
        let verification = row
            .independent_verification
            .as_mut()
            .ok_or_else(|| "sample verification missing".to_string())?;
        verification.comparison = AttemptComparisonV1::NearMatched;
        row.row_digest = orchestration_row_digest(&row)?;
        let assessment = assess_orchestration_attempt(&row);
        if assessment.disposition != Some(AttemptDispositionV1::VerificationFailed)
            || !reason_contains(&assessment, "not matched")
        {
            return Err(format!(
                "a non-matched comparison must block completion, got {:?} reasons={:?}",
                assessment.disposition, assessment.reasons
            ));
        }
        Ok(())
    }

    #[test]
    fn forbidden_path_survives_a_failed_verification_command() -> Result<(), String> {
        let mut row = stamped_sample()?;
        row.changed_paths.push(OrchestrationChangedPathV1 {
            path: "Cargo.lock".to_string(),
            boundary_status: OrchestrationBoundaryStatusV1::ForbiddenPath,
        });
        let verification = row
            .independent_verification
            .as_mut()
            .ok_or_else(|| "sample verification missing".to_string())?;
        verification.commands[0].passed = false;
        row.row_digest = orchestration_row_digest(&row)?;
        let assessment = assess_orchestration_attempt(&row);
        if assessment.disposition != Some(AttemptDispositionV1::BoundaryViolation)
            || !reason_contains(&assessment, "forbidden path")
            || !reason_contains(&assessment, "failed")
        {
            return Err(format!(
                "the first downgrade must win, got {:?} reasons={:?}",
                assessment.disposition, assessment.reasons
            ));
        }
        Ok(())
    }

    #[test]
    fn contradiction_survives_a_stale_verification_binding() -> Result<(), String> {
        let mut row = stamped_sample()?;
        row.contradictions
            .push("adversary disputes the dependent result".to_string());
        let verification = row
            .independent_verification
            .as_mut()
            .ok_or_else(|| "sample verification missing".to_string())?;
        verification.bound_base = "older-base-sha".to_string();
        row.row_digest = orchestration_row_digest(&row)?;
        let assessment = assess_orchestration_attempt(&row);
        if assessment.disposition != Some(AttemptDispositionV1::Contradicted)
            || !reason_contains(&assessment, "contradiction")
            || !reason_contains(&assessment, "stale")
        {
            return Err(format!(
                "a contradiction must not be overwritten by stale, got {:?} reasons={:?}",
                assessment.disposition, assessment.reasons
            ));
        }
        Ok(())
    }
}
