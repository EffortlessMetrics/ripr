//! Issue lifecycle contract/plan decision-boundary pilot (#4931, RIPR-SPEC-0236).
//!
//! The second real-corpus consumer of the frozen RIPR-SPEC-0218 attempt
//! contract (#4929), extending the sibling intake machinery (RIPR-SPEC-0234):
//! exactly two REAL current issue snapshots, captured immutably and read-only,
//! traverse the decision boundary between contract-required work and direct
//! one-PR work. The contract-required row retains the full contract-case
//! evidence (source-of-truth identity, spec-required rationale, draft spec
//! identity, author role/config/result identity, independent adversary result
//! with inspected scope, missing failure/limited findings or bounded
//! `none_found`, the root role/config/result identity as the acceptance
//! authority, preserved open decisions, the root disposition in the closed
//! RIPR-SPEC-0218 vocabulary and the accepted/amended/rejected/provisional
//! contract state, the latter frozen into the embedded attempt's contract
//! decision). Both rows retain the planning evidence (one-PR versus
//! campaign decision, work-item identities with dependency edges, acceptance
//! rows covered or explicitly omitted, edit cages and semantic conflict
//! resources, proof commands with denominators, stop conditions, non-goals,
//! portfolio placement without a tracked selection/current-work mutation, and
//! root corrections/re-splits). The author/adversary/root roles are distinct
//! fixture identities with distinct results: the author result never carries
//! an acceptance state and the root disposition is the only acceptance
//! authority, expressed in the RIPR-SPEC-0218 disposition set only.
//!
//! Selection law: this module selects no work. It reads only committed corpus
//! fields; no `active.toml`, label, issue age or title is consulted, and the
//! packet projection is a pure function of the corpus row. Mechanics controls
//! (the ten required decision-boundary controls) live in a separate synthetic
//! control corpus and never enter the two real rows or any real denominator.

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::issue_lifecycle_attempt::{
    IssueLifecycleAttemptV1, IssueLifecycleDispositionV1, IssueLifecycleRowAssessmentV1,
    assess_issue_lifecycle_attempt,
};
use crate::issue_lifecycle_intake::{
    IssueLifecycleIntakeRetrievalStepV1, IssueLifecycleIntakeSnapshotV1,
};

pub(crate) const ISSUE_LIFECYCLE_CONTRACT_PLAN_CORPUS_SCHEMA_VERSION: &str =
    "issue_lifecycle_contract_plan_corpus.v1";
pub(crate) const ISSUE_LIFECYCLE_CONTRACT_PLAN_CONTROL_CORPUS_SCHEMA_VERSION: &str =
    "issue_lifecycle_contract_plan_control_corpus.v1";
pub(crate) const ISSUE_LIFECYCLE_CONTRACT_PLAN_PROVENANCE_SCHEMA_VERSION: &str =
    "issue_lifecycle_contract_plan_provenance.v1";
pub(crate) const DEFAULT_CONTRACT_PLAN_CORPUS_DIR: &str = "fixtures/issue_lifecycle_contract_plan";

/// The closed two-category vocabulary #4931 requires, one row per category.
pub(crate) const REQUIRED_ISSUE_LIFECYCLE_CONTRACT_PLAN_CATEGORIES: [&str; 2] =
    ["contract_required", "narrow_accepted_contract_bug"];

/// The ten mechanics controls the committed control corpus must carry: each
/// names one decision-boundary law the two real rows can never exercise
/// alone, so an emptied or gutted control corpus fails closed instead of
/// passing vacuously.
pub(crate) const REQUIRED_ISSUE_LIFECYCLE_CONTRACT_PLAN_CONTROLS: [&str; 10] = [
    "adversary_catches_missing_failure_state_or_bounded_none_found",
    "author_cannot_accept_own_contract",
    "unresolved_decision_blocks_implementation",
    "narrow_bug_gets_no_unnecessary_spec",
    "campaign_rejected_when_one_vertical_slice_suffices",
    "one_pr_rejected_when_acceptance_cannot_be_covered_coherently",
    "specs_contain_behavior_not_execution_queues",
    "plans_contain_work_order_not_new_behavior_authority",
    "no_tracked_selection_or_current_work_file_changed",
    "cold_start_root_reconstructs_decisions_from_artifacts",
];

pub(crate) const ISSUE_LIFECYCLE_CONTRACT_PLAN_CLAIM_BOUNDARY: &str = "Read-only contract/plan pilot receipt: two exact real issue snapshots \
 traverse the contract/plan decision boundary with distinct author, adversary \
 and root fixture identities; it claims no decision correctness beyond these \
 two rows, no implementation, no plan execution and no parent acceptance.";

/// The closed contract-state vocabulary a root disposition may carry.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum IssueLifecycleContractStateV1 {
    Accepted,
    Amended,
    Rejected,
    Provisional,
}

/// One role result: identity, config and produced result artifact. The author
/// result must never carry an acceptance state; only the root disposition
/// records acceptance authority.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct IssueLifecycleContractRoleResultV1 {
    pub role: String,
    pub config_identity: String,
    pub result_identity: String,
    pub carries_acceptance: bool,
}

/// The independent adversary result: what it inspected and what it found. A
/// bounded `none_found` (empty findings) is honest; findings and `none_found`
/// are mutually exclusive.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct IssueLifecycleContractAdversaryResultV1 {
    pub result_identity: String,
    pub inspected_scope: String,
    pub findings: Vec<String>,
    pub none_found: bool,
}

/// The full contract-case evidence for one contract-required row.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct IssueLifecycleContractCaseEvidenceV1 {
    pub source_of_truth: String,
    pub spec_required_rationale: String,
    pub draft_spec_identity: String,
    pub author: IssueLifecycleContractRoleResultV1,
    pub adversary: IssueLifecycleContractAdversaryResultV1,
    /// The root role result: the acceptance authority itself. Like the author
    /// and adversary results it is a typed identity, so a corpus in which the
    /// root reuses the author's result — or no root actor ran at all — fails
    /// closed instead of passing as three distinct fixture identities.
    pub root: IssueLifecycleContractRoleResultV1,
    pub open_decisions: Vec<String>,
    pub root_disposition: IssueLifecycleDispositionV1,
    pub contract_state: IssueLifecycleContractStateV1,
}

/// The bounded plan shape decision.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum IssueLifecyclePlanShapeV1 {
    OnePr,
    Campaign,
}

/// One planned work item with its dependency edges.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct IssueLifecyclePlanWorkItemV1 {
    pub id: String,
    pub depends_on: Vec<String>,
}

/// One proof command with the exact denominator it must run against.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct IssueLifecycleProofCommandV1 {
    pub command: String,
    pub denominator: String,
}

/// The planning evidence retained for one row.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct IssueLifecyclePlanEvidenceV1 {
    pub shape_decision: IssueLifecyclePlanShapeV1,
    pub shape_rationale: String,
    pub work_items: Vec<IssueLifecyclePlanWorkItemV1>,
    pub acceptance_covered: Vec<String>,
    pub acceptance_omitted: Vec<String>,
    pub edit_cages: Vec<String>,
    pub conflict_resources: Vec<String>,
    pub proof_commands: Vec<IssueLifecycleProofCommandV1>,
    pub stop_conditions: Vec<String>,
    pub non_goals: Vec<String>,
    pub portfolio_placement: String,
    pub root_corrections: Vec<String>,
    pub re_splits: Vec<String>,
}

/// One contract/plan row: the #4931 contract-case and planning evidence plus
/// the embedded RIPR-SPEC-0218 attempt the scorecard counts.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct IssueLifecycleContractPlanRowV1 {
    pub id: String,
    pub category: String,
    pub snapshot: IssueLifecycleIntakeSnapshotV1,
    pub current_main: String,
    pub contract: Option<IssueLifecycleContractCaseEvidenceV1>,
    pub planning: IssueLifecyclePlanEvidenceV1,
    pub retrieval_steps: Vec<IssueLifecycleIntakeRetrievalStepV1>,
    pub limitations: Vec<String>,
    pub non_claims: Vec<String>,
    pub attempt: IssueLifecycleAttemptV1,
}

/// The committed read-only contract/plan corpus: exactly two real rows.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct IssueLifecycleContractPlanCorpusV1 {
    pub schema_version: String,
    pub captured_at: String,
    pub base_main: String,
    pub rows: Vec<IssueLifecycleContractPlanRowV1>,
}

/// One synthetic mechanics control row (outside the two real rows).
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct IssueLifecycleContractPlanControlRowV1 {
    pub id: String,
    pub control: String,
    pub scenario: String,
    pub row: IssueLifecycleContractPlanRowV1,
}

/// The committed mechanics control corpus; every embedded attempt is
/// synthetic and never enters a real denominator.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct IssueLifecycleContractPlanControlCorpusV1 {
    pub schema_version: String,
    pub rows: Vec<IssueLifecycleContractPlanControlRowV1>,
}

/// One provenance entry per real row.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct IssueLifecycleContractPlanProvenanceRowV1 {
    pub issue: u64,
    pub category: String,
    pub justification: String,
}

/// Capture provenance: when, against which main, and why each row matches its
/// category.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct IssueLifecycleContractPlanProvenanceV1 {
    pub schema_version: String,
    pub repository: String,
    pub captured_at: String,
    pub base_main: String,
    pub capture_method: String,
    pub rows: Vec<IssueLifecycleContractPlanProvenanceRowV1>,
}

fn workspace_path(relative: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join(relative)
}

/// Load and shape-check the committed contract/plan corpus: exactly two rows,
/// the closed category set covered once each, unique issue numbers, real (not
/// synthetic) attempts.
pub(crate) fn load_issue_lifecycle_contract_plan_corpus(
    body: &str,
) -> Result<IssueLifecycleContractPlanCorpusV1, String> {
    let corpus: IssueLifecycleContractPlanCorpusV1 = serde_json::from_str(body)
        .map_err(|error| format!("parse issue lifecycle contract plan corpus: {error}"))?;
    if corpus.schema_version != ISSUE_LIFECYCLE_CONTRACT_PLAN_CORPUS_SCHEMA_VERSION {
        return Err(format!(
            "unsupported issue lifecycle contract plan corpus schema `{}`",
            corpus.schema_version
        ));
    }
    if corpus.captured_at.trim().is_empty() || corpus.base_main.trim().is_empty() {
        return Err(
            "issue lifecycle contract plan corpus must record captured_at and base_main"
                .to_string(),
        );
    }
    if corpus.rows.len() != REQUIRED_ISSUE_LIFECYCLE_CONTRACT_PLAN_CATEGORIES.len() {
        return Err(format!(
            "issue lifecycle contract plan corpus must carry exactly {} rows, got {}",
            REQUIRED_ISSUE_LIFECYCLE_CONTRACT_PLAN_CATEGORIES.len(),
            corpus.rows.len()
        ));
    }
    let mut categories = BTreeSet::new();
    let mut issues = BTreeSet::new();
    let mut observation_keys = BTreeSet::new();
    let mut lifecycle_ids = BTreeSet::new();
    for row in &corpus.rows {
        if !categories.insert(row.category.clone()) {
            return Err(format!(
                "issue lifecycle contract plan corpus duplicates category `{}`",
                row.category
            ));
        }
        if !issues.insert(row.snapshot.issue_number) {
            return Err(format!(
                "issue lifecycle contract plan corpus duplicates issue `{}`",
                row.snapshot.issue_ref
            ));
        }
        // Aliased observation keys would collapse both rows into conflicting
        // duplicates at scorecard construction and emit zero real lifecycles
        // while this command still exited successfully; reject the alias at
        // load so a deduplicated lifecycle can never disappear silently.
        if !observation_keys.insert(row.attempt.observation_key.clone()) {
            return Err(format!(
                "issue lifecycle contract plan corpus duplicates observation key `{}`",
                row.attempt.observation_key
            ));
        }
        if !lifecycle_ids.insert(row.attempt.lifecycle_id.clone()) {
            return Err(format!(
                "issue lifecycle contract plan corpus duplicates lifecycle id `{}`",
                row.attempt.lifecycle_id
            ));
        }
        if row.attempt.synthetic {
            return Err(format!(
                "issue lifecycle contract plan row `{}` must be a real row, not synthetic",
                row.id
            ));
        }
    }
    for required in REQUIRED_ISSUE_LIFECYCLE_CONTRACT_PLAN_CATEGORIES {
        if !categories.contains(required) {
            return Err(format!(
                "issue lifecycle contract plan corpus is missing category `{required}`"
            ));
        }
    }
    Ok(corpus)
}

/// Load and shape-check the mechanics control corpus: unique ids, the
/// `control_` id prefix, synthetic attempts only, and the full set of
/// required mechanics controls present — an empty or gutted control corpus
/// must fail closed rather than pass vacuously.
pub(crate) fn load_issue_lifecycle_contract_plan_control_corpus(
    body: &str,
) -> Result<IssueLifecycleContractPlanControlCorpusV1, String> {
    let corpus: IssueLifecycleContractPlanControlCorpusV1 = serde_json::from_str(body)
        .map_err(|error| format!("parse issue lifecycle contract plan control corpus: {error}"))?;
    if corpus.schema_version != ISSUE_LIFECYCLE_CONTRACT_PLAN_CONTROL_CORPUS_SCHEMA_VERSION {
        return Err(format!(
            "unsupported issue lifecycle contract plan control corpus schema `{}`",
            corpus.schema_version
        ));
    }
    if corpus.rows.is_empty() {
        return Err("issue lifecycle contract plan control corpus must not be empty".to_string());
    }
    let present: BTreeSet<&str> = corpus
        .rows
        .iter()
        .map(|control| control.control.as_str())
        .collect();
    for required in REQUIRED_ISSUE_LIFECYCLE_CONTRACT_PLAN_CONTROLS {
        if !present.contains(required) {
            return Err(format!(
                "issue lifecycle contract plan control corpus is missing required control `{required}`"
            ));
        }
    }
    let mut ids = BTreeSet::new();
    for control in &corpus.rows {
        if !control.id.starts_with("control_") {
            return Err(format!(
                "issue lifecycle contract plan control row `{}` must use the `control_` id prefix",
                control.id
            ));
        }
        if !ids.insert(control.id.clone()) {
            return Err(format!(
                "issue lifecycle contract plan control corpus duplicates id `{}`",
                control.id
            ));
        }
        if control.control.trim().is_empty() || control.scenario.trim().is_empty() {
            return Err(format!(
                "issue lifecycle contract plan control row `{}` must name its control and scenario",
                control.id
            ));
        }
        if !control.row.attempt.synthetic {
            return Err(format!(
                "issue lifecycle contract plan control row `{}` must be synthetic",
                control.id
            ));
        }
    }
    Ok(corpus)
}

/// Load and parse the committed provenance file.
pub(crate) fn load_issue_lifecycle_contract_plan_provenance(
    body: &str,
) -> Result<IssueLifecycleContractPlanProvenanceV1, String> {
    let provenance: IssueLifecycleContractPlanProvenanceV1 = serde_json::from_str(body)
        .map_err(|error| format!("parse issue lifecycle contract plan provenance: {error}"))?;
    if provenance.schema_version != ISSUE_LIFECYCLE_CONTRACT_PLAN_PROVENANCE_SCHEMA_VERSION {
        return Err(format!(
            "unsupported issue lifecycle contract plan provenance schema `{}`",
            provenance.schema_version
        ));
    }
    if provenance.base_main.trim().is_empty() || provenance.captured_at.trim().is_empty() {
        return Err(
            "issue lifecycle contract plan provenance must record captured_at and base_main"
                .to_string(),
        );
    }
    if provenance.repository.trim().is_empty() || provenance.capture_method.trim().is_empty() {
        return Err(
            "issue lifecycle contract plan provenance must record repository and capture_method"
                .to_string(),
        );
    }
    Ok(provenance)
}

fn identity_matches(identity: &str, scheme: &str, computed_hex: &str) -> bool {
    identity == format!("{scheme}:sha256:{computed_hex}")
}

/// Normalize the frozen attempt's root contract state string into the closed
/// contract-state vocabulary; an unknown value is a rejection reason, never a
/// silent parse drop.
fn normalized_contract_state(
    raw: &Option<String>,
) -> Result<Option<IssueLifecycleContractStateV1>, String> {
    match raw {
        None => Ok(None),
        Some(value) => serde_json::from_value(serde_json::Value::String(value.clone()))
            .map(Some)
            .map_err(|_error| format!("`{value}` is not a closed contract state")),
    }
}

/// Fail-closed snapshot binding: recompute the snapshot and comment digests
/// from the committed bytes and reject any drift. A row whose snapshot file
/// is missing or altered can never be counted. Real rows must capture the
/// full issue/comments/timeline triple, the embedded attempt must name the
/// verified snapshot identities, the issue payload must agree with the named
/// GitHub issue (including the `#number` suffix of the issue ref), the
/// timeline payload must bind to the row repository, and retrieval-step byte
/// claims for the three capture commands are bound to the committed file
/// sizes and the full row-repository endpoint.
pub(crate) fn verify_contract_plan_row_snapshot(
    row: &IssueLifecycleContractPlanRowV1,
    root: &Path,
) -> Result<(), String> {
    let snapshot_path = row
        .snapshot
        .snapshot_path
        .as_ref()
        .ok_or_else(|| format!("contract plan row `{}` has no snapshot path", row.id))?;
    let comments_path = row
        .snapshot
        .comments_path
        .as_ref()
        .ok_or_else(|| format!("contract plan row `{}` has no comments path", row.id))?;
    if row.snapshot.captured_at.trim().is_empty() {
        return Err(format!(
            "contract plan row `{}` snapshot records no captured_at",
            row.id
        ));
    }
    // The embedded attempt must name the exact snapshot this verification
    // authenticates; swapping the embedded issue identities and recomputing
    // the row digests must not leave a packet naming one issue while the
    // scorecard counts an attempt bound to another.
    if row.attempt.issue.issue_ref != row.snapshot.issue_ref {
        return Err(format!(
            "contract plan row `{}` embedded attempt names issue `{}` but the row snapshot names `{}`",
            row.id, row.attempt.issue.issue_ref, row.snapshot.issue_ref
        ));
    }
    if row.attempt.issue.snapshot_id != row.snapshot.issue_snapshot_id {
        return Err(format!(
            "contract plan row `{}` embedded attempt issue digest disagrees with the verified snapshot digest",
            row.id
        ));
    }
    if row.attempt.issue.comments_ref != row.snapshot.comments_snapshot_id {
        return Err(format!(
            "contract plan row `{}` embedded attempt comments digest disagrees with the verified comments digest",
            row.id
        ));
    }
    let snapshot_body = fs::read(root.join(snapshot_path)).map_err(|error| {
        format!(
            "contract plan row `{}` snapshot {} is unreadable: {error}",
            row.id, snapshot_path
        )
    })?;
    let computed = crate::blind_journey::sha256_hex(&snapshot_body);
    if !identity_matches(
        &row.snapshot.issue_snapshot_id,
        "gh-issue-snapshot",
        &computed,
    ) {
        return Err(format!(
            "contract plan row `{}` snapshot digest drifted: recorded `{}`, recomputed `gh-issue-snapshot:sha256:{computed}`",
            row.id, row.snapshot.issue_snapshot_id
        ));
    }
    verify_issue_payload_binding(row, &snapshot_body)?;
    let comments_body = fs::read(root.join(comments_path)).map_err(|error| {
        format!(
            "contract plan row `{}` comments {} are unreadable: {error}",
            row.id, comments_path
        )
    })?;
    let computed = crate::blind_journey::sha256_hex(&comments_body);
    if !identity_matches(
        &row.snapshot.comments_snapshot_id,
        "gh-issue-comments",
        &computed,
    ) {
        return Err(format!(
            "contract plan row `{}` comments digest drifted: recorded `{}`, recomputed `gh-issue-comments:sha256:{computed}`",
            row.id, row.snapshot.comments_snapshot_id
        ));
    }
    // A real row must capture the issue/comments/timeline triple: deleting the
    // timeline surface fails closed instead of skipping one third of the
    // snapshot evidence. (Synthetic control rows never enter this verifier.)
    let timeline_path = row
        .snapshot
        .timeline_path
        .as_ref()
        .ok_or_else(|| {
            format!(
                "contract plan row `{}` has no timeline path; a real row must capture the issue/comments/timeline triple",
                row.id
            )
        })?;
    let timeline_body = fs::read(root.join(timeline_path)).map_err(|error| {
        format!(
            "contract plan row `{}` timeline {} is unreadable: {error}",
            row.id, timeline_path
        )
    })?;
    let computed = crate::blind_journey::sha256_hex(&timeline_body);
    let recorded = row
        .snapshot
        .timeline_snapshot_id
        .as_deref()
        .ok_or_else(|| format!("contract plan row `{}` timeline records no digest", row.id))?;
    if !identity_matches(recorded, "gh-issue-timeline", &computed) {
        return Err(format!(
            "contract plan row `{}` timeline digest drifted: recorded `{recorded}`, recomputed `gh-issue-timeline:sha256:{computed}`",
            row.id
        ));
    }
    verify_timeline_payload_binding(row, &timeline_body)?;
    verify_retrieval_step_bytes(row, "issue", snapshot_body.len())?;
    verify_retrieval_step_bytes(row, "comments", comments_body.len())?;
    verify_retrieval_step_bytes(row, "timeline", timeline_body.len())?;
    Ok(())
}

/// Bind the committed issue snapshot bytes to the named GitHub issue: the
/// payload's number and repository must agree with `issue_number`/`issue_ref`,
/// so digest agreement cannot pass while the bytes came from another issue.
fn verify_issue_payload_binding(
    row: &IssueLifecycleContractPlanRowV1,
    snapshot_body: &[u8],
) -> Result<(), String> {
    let payload: serde_json::Value = serde_json::from_slice(snapshot_body).map_err(|error| {
        format!(
            "contract plan row `{}` issue snapshot is not a GitHub issue payload: {error}",
            row.id
        )
    })?;
    let number = payload
        .get("number")
        .and_then(serde_json::Value::as_u64)
        .ok_or_else(|| {
            format!(
                "contract plan row `{}` issue snapshot payload records no issue number",
                row.id
            )
        })?;
    if number != row.snapshot.issue_number {
        return Err(format!(
            "contract plan row `{}` names issue {} but its snapshot payload is issue {number}",
            row.id, row.snapshot.issue_number
        ));
    }
    let payload_repository = payload
        .get("repository_url")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| {
            format!(
                "contract plan row `{}` issue snapshot payload records no repository_url",
                row.id
            )
        })?;
    let (repository, suffix) = row.snapshot.issue_ref.split_once('#').ok_or_else(|| {
        format!(
            "contract plan row `{}` issue ref `{}` is not an `owner/repo#number` reference",
            row.id, row.snapshot.issue_ref
        )
    })?;
    let expected_repository = format!("https://api.github.com/repos/{repository}");
    let referenced_number: u64 = suffix.parse().map_err(|_error| {
        format!(
            "contract plan row `{}` issue ref `{}` carries a non-numeric issue suffix `{suffix}`",
            row.id, row.snapshot.issue_ref
        )
    })?;
    if referenced_number != row.snapshot.issue_number {
        return Err(format!(
            "contract plan row `{}` issue ref `{}` names issue {referenced_number} but the row authenticates issue {}",
            row.id, row.snapshot.issue_ref, row.snapshot.issue_number
        ));
    }
    if payload_repository != expected_repository {
        return Err(format!(
            "contract plan row `{}` issue ref `{}` disagrees with the snapshot repository `{payload_repository}`",
            row.id, row.snapshot.issue_ref
        ));
    }
    Ok(())
}

/// Bind the committed timeline bytes to the row repository: the payload must
/// be a non-empty event array, every event must carry a nonblank event name,
/// and every cross-referenced source issue must name the row repository with a
/// url that agrees with its own issue number. The GitHub timeline payload
/// names the referrer issue, not the host issue, so this law authenticates
/// repository membership and internal number/url consistency; a timeline
/// harvested from another repository, or with spliced source issues, fails
/// closed. The committed digest chain remains the authority for host-swap
/// drift of otherwise-consistent captures.
fn verify_timeline_payload_binding(
    row: &IssueLifecycleContractPlanRowV1,
    timeline_body: &[u8],
) -> Result<(), String> {
    let payload: serde_json::Value = serde_json::from_slice(timeline_body).map_err(|error| {
        format!(
            "contract plan row `{}` timeline is not a GitHub timeline payload: {error}",
            row.id
        )
    })?;
    let events = payload.as_array().ok_or_else(|| {
        format!(
            "contract plan row `{}` timeline payload is not an event array",
            row.id
        )
    })?;
    if events.is_empty() {
        return Err(format!(
            "contract plan row `{}` timeline records no events; a captured timeline is never empty",
            row.id
        ));
    }
    let repository = row
        .snapshot
        .issue_ref
        .split_once('#')
        .map(|(repository, _)| repository)
        .ok_or_else(|| {
            format!(
                "contract plan row `{}` issue ref `{}` is not an `owner/repo#number` reference",
                row.id, row.snapshot.issue_ref
            )
        })?;
    for (index, event) in events.iter().enumerate() {
        let name = event
            .get("event")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| {
                format!(
                    "contract plan row `{}` timeline event {index} records no event name",
                    row.id
                )
            })?;
        if name.trim().is_empty() {
            return Err(format!(
                "contract plan row `{}` timeline event {index} records a blank event name",
                row.id
            ));
        }
        let Some(issue) = event.get("source").and_then(|source| source.get("issue")) else {
            continue;
        };
        let number = issue
            .get("number")
            .and_then(serde_json::Value::as_u64)
            .ok_or_else(|| {
                format!(
                    "contract plan row `{}` timeline event {index} source issue records no issue number",
                    row.id
                )
            })?;
        let source_repository = issue
            .get("repository")
            .and_then(|repository| repository.get("full_name"))
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| {
                format!(
                    "contract plan row `{}` timeline event {index} source issue records no repository full_name",
                    row.id
                )
            })?;
        if source_repository != repository {
            return Err(format!(
                "contract plan row `{}` timeline event {index} source issue names repository `{source_repository}` but the row authenticates repository `{repository}`",
                row.id
            ));
        }
        let url = issue
            .get("url")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| {
                format!(
                    "contract plan row `{}` timeline event {index} source issue records no url",
                    row.id
                )
            })?;
        if url != format!("https://api.github.com/repos/{repository}/issues/{number}") {
            return Err(format!(
                "contract plan row `{}` timeline event {index} source issue url `{url}` does not name its own issue number {number} in the row repository",
                row.id
            ));
        }
    }
    Ok(())
}

/// Bind recorded retrieval-step byte claims to the committed snapshot bytes:
/// exactly one step must name the capture command for each present snapshot
/// surface, and it must carry a measured byte count equal to the committed
/// file it produced. A missing step, a `NotMeasured` step or a fabricated
/// count fails closed. The command must name the full row-repository endpoint
/// (`repos/<owner>/<repo>/issues/<N>` derived from the issue ref), not merely
/// the issue-number suffix: a command harvested from another repository fails
/// closed even when the number matches.
fn verify_retrieval_step_bytes(
    row: &IssueLifecycleContractPlanRowV1,
    surface: &str,
    committed_len: usize,
) -> Result<(), String> {
    let repository = row
        .snapshot
        .issue_ref
        .split_once('#')
        .map(|(repository, _)| repository)
        .ok_or_else(|| {
            format!(
                "contract plan row `{}` issue ref `{}` is not an `owner/repo#number` reference",
                row.id, row.snapshot.issue_ref
            )
        })?;
    let suffix = match surface {
        "issue" => format!("repos/{repository}/issues/{}", row.snapshot.issue_number),
        "comments" => format!(
            "repos/{repository}/issues/{}/comments",
            row.snapshot.issue_number
        ),
        "timeline" => format!(
            "repos/{repository}/issues/{}/timeline",
            row.snapshot.issue_number
        ),
        other => return Err(format!("unknown retrieval surface `{other}`")),
    };
    let foreign_suffix = format!("/issues/{}", row.snapshot.issue_number);
    let mut matching = row
        .retrieval_steps
        .iter()
        .filter(|step| step.command.ends_with(&suffix));
    let step = match matching.next() {
        Some(step) => step,
        None => {
            if let Some(step) = row
                .retrieval_steps
                .iter()
                .find(|step| step.command.ends_with(&foreign_suffix))
            {
                return Err(format!(
                    "contract plan row `{}` retrieval step `{}` names another repository; the capture endpoint must be `repos/{repository}/issues/{}` derived from the row issue ref",
                    row.id, step.command, row.snapshot.issue_number
                ));
            }
            return Err(format!(
                "contract plan row `{}` records no retrieval step for surface `{surface}`; the captured bytes must be measured evidence, not a missing step",
                row.id
            ));
        }
    };
    if matching.next().is_some() {
        return Err(format!(
            "contract plan row `{}` records duplicate retrieval steps for surface `{surface}`",
            row.id
        ));
    }
    match step.bytes {
        crate::issue_lifecycle_intake::IssueLifecycleIntakeBytesV1::Measured(bytes)
            if bytes as usize == committed_len => {}
        crate::issue_lifecycle_intake::IssueLifecycleIntakeBytesV1::Measured(bytes) => {
            return Err(format!(
                "contract plan row `{}` retrieval step `{}` claims {bytes} bytes, committed bytes measure {committed_len}",
                row.id, step.command
            ));
        }
        crate::issue_lifecycle_intake::IssueLifecycleIntakeBytesV1::NotMeasured => {
            return Err(format!(
                "contract plan row `{}` retrieval step `{}` for surface `{surface}` is not measured; fail-closed capture evidence requires the committed byte count",
                row.id, step.command
            ));
        }
    }
    Ok(())
}

/// One deterministic packet projection: everything a cold-start root needs
/// to reconstruct the contract/plan decision from the committed corpus alone,
/// with no chat, no live GitHub read and no selection signal. The projection
/// retains the source-of-truth identity, the spec-required rationale, the
/// draft identity, the distinct author/adversary/root result identities, the
/// inspected scope, the shape rationale, the work-item nodes and their
/// dependency edges, the edit cages, the conflict resources, the non-goals,
/// the row limitations and the row non-claims alongside the decisions
/// themselves; snapshot-only signals (title, labels, age) never enter it.
#[derive(Clone, Debug, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct IssueLifecycleContractPlanProjectionV1 {
    pub schema_version: String,
    pub id: String,
    pub issue_ref: String,
    pub category: String,
    pub current_main: String,
    pub contract_state: Option<IssueLifecycleContractStateV1>,
    pub root_disposition: Option<IssueLifecycleDispositionV1>,
    pub source_of_truth: Option<String>,
    pub spec_required_rationale: Option<String>,
    pub draft_spec_identity: Option<String>,
    pub author_result_identity: Option<String>,
    pub adversary_result_identity: Option<String>,
    pub root_result_identity: Option<String>,
    pub adversary_inspected_scope: Option<String>,
    pub open_decisions: Vec<String>,
    pub adversary_findings: Vec<String>,
    pub adversary_none_found: Option<bool>,
    pub shape_decision: IssueLifecyclePlanShapeV1,
    pub shape_rationale: String,
    pub work_item_edges: Vec<String>,
    pub work_item_nodes: Vec<String>,
    pub acceptance_covered: Vec<String>,
    pub acceptance_omitted: Vec<String>,
    pub proof_commands: Vec<IssueLifecycleProofCommandV1>,
    pub stop_conditions: Vec<String>,
    pub edit_cages: Vec<String>,
    pub conflict_resources: Vec<String>,
    pub non_goals: Vec<String>,
    pub portfolio_placement: String,
    pub root_corrections: Vec<String>,
    pub re_splits: Vec<String>,
    pub limitations: Vec<String>,
    pub non_claims: Vec<String>,
}

pub(crate) fn build_contract_plan_projection(
    row: &IssueLifecycleContractPlanRowV1,
) -> IssueLifecycleContractPlanProjectionV1 {
    let mut work_item_edges: Vec<String> = row
        .planning
        .work_items
        .iter()
        .flat_map(|item| {
            item.depends_on
                .iter()
                .map(move |dependency| format!("{}->{}", item.id, dependency))
        })
        .collect();
    work_item_edges.sort();
    let mut work_item_nodes: Vec<String> = row
        .planning
        .work_items
        .iter()
        .map(|item| item.id.clone())
        .collect();
    work_item_nodes.sort();
    IssueLifecycleContractPlanProjectionV1 {
        schema_version: "issue_lifecycle_contract_plan_packet.v1".to_string(),
        id: row.id.clone(),
        issue_ref: row.snapshot.issue_ref.clone(),
        category: row.category.clone(),
        current_main: row.current_main.clone(),
        contract_state: row
            .contract
            .as_ref()
            .map(|contract| contract.contract_state),
        root_disposition: row
            .contract
            .as_ref()
            .map(|contract| contract.root_disposition),
        source_of_truth: row
            .contract
            .as_ref()
            .map(|contract| contract.source_of_truth.clone()),
        spec_required_rationale: row
            .contract
            .as_ref()
            .map(|contract| contract.spec_required_rationale.clone()),
        draft_spec_identity: row
            .contract
            .as_ref()
            .map(|contract| contract.draft_spec_identity.clone()),
        author_result_identity: row
            .contract
            .as_ref()
            .map(|contract| contract.author.result_identity.clone()),
        adversary_result_identity: row
            .contract
            .as_ref()
            .map(|contract| contract.adversary.result_identity.clone()),
        root_result_identity: row
            .contract
            .as_ref()
            .map(|contract| contract.root.result_identity.clone()),
        adversary_inspected_scope: row
            .contract
            .as_ref()
            .map(|contract| contract.adversary.inspected_scope.clone()),
        open_decisions: row
            .contract
            .as_ref()
            .map_or_else(Vec::new, |contract| contract.open_decisions.clone()),
        adversary_findings: row
            .contract
            .as_ref()
            .map_or_else(Vec::new, |contract| contract.adversary.findings.clone()),
        adversary_none_found: row
            .contract
            .as_ref()
            .map(|contract| contract.adversary.none_found),
        shape_decision: row.planning.shape_decision,
        shape_rationale: row.planning.shape_rationale.clone(),
        work_item_edges,
        work_item_nodes,
        acceptance_covered: row.planning.acceptance_covered.clone(),
        acceptance_omitted: row.planning.acceptance_omitted.clone(),
        proof_commands: row.planning.proof_commands.clone(),
        stop_conditions: row.planning.stop_conditions.clone(),
        edit_cages: row.planning.edit_cages.clone(),
        conflict_resources: row.planning.conflict_resources.clone(),
        non_goals: row.planning.non_goals.clone(),
        portfolio_placement: row.planning.portfolio_placement.clone(),
        root_corrections: row.planning.root_corrections.clone(),
        re_splits: row.planning.re_splits.clone(),
        limitations: row.limitations.clone(),
        non_claims: row.non_claims.clone(),
    }
}

/// Every planning string surface the behavior-authority and selection-file
/// laws scan; plans carry work order, never new behavior authority, and never
/// reference a tracked selection or current-work file. Proof commands scan
/// too: a proof command or denominator minting behavior authority or naming
/// a selection file fails closed like any other planning surface.
fn planning_strings(planning: &IssueLifecyclePlanEvidenceV1) -> Vec<String> {
    let mut strings = vec![
        planning.shape_rationale.clone(),
        planning.portfolio_placement.clone(),
    ];
    strings.extend(planning.acceptance_covered.iter().cloned());
    strings.extend(planning.acceptance_omitted.iter().cloned());
    strings.extend(planning.edit_cages.iter().cloned());
    strings.extend(planning.conflict_resources.iter().cloned());
    strings.extend(planning.stop_conditions.iter().cloned());
    strings.extend(planning.non_goals.iter().cloned());
    strings.extend(planning.root_corrections.iter().cloned());
    strings.extend(planning.re_splits.iter().cloned());
    strings.extend(planning.work_items.iter().map(|item| item.id.clone()));
    for proof in &planning.proof_commands {
        strings.push(proof.command.clone());
        strings.push(proof.denominator.clone());
    }
    strings
}

/// Per-row decision-boundary law over the contract-case and planning
/// evidence plus the embedded RIPR-SPEC-0218 attempt. Returns every failure;
/// an empty vector means the row is internally consistent, the roles are
/// separated and the root disposition is genuinely independent.
pub(crate) fn assess_contract_plan_row(row: &IssueLifecycleContractPlanRowV1) -> Vec<String> {
    let mut failures = Vec::new();
    match row.category.as_str() {
        "contract_required" => {
            let contract = match &row.contract {
                Some(contract) => contract,
                None => {
                    failures.push(format!(
                        "row `{}` is contract-required but records no contract evidence",
                        row.id
                    ));
                    return failures;
                }
            };
            if !row.attempt.contract_decision.spec_required {
                failures.push(format!(
                    "row `{}` is contract-required but the attempt records no spec-required decision",
                    row.id
                ));
            }
            if contract.source_of_truth.trim().is_empty() {
                failures.push(format!(
                    "row `{}` contract records no source-of-truth identity; the retained contract authority must name what governs the row",
                    row.id
                ));
            }
            if contract.spec_required_rationale.trim().is_empty() {
                failures.push(format!(
                    "row `{}` contract records no spec-required rationale; the retained authority must justify why a spec is required",
                    row.id
                ));
            }
            if row
                .attempt
                .contract_decision
                .decision_rationale
                .trim()
                .is_empty()
            {
                failures.push(format!(
                    "row `{}` embedded attempt records no contract decision rationale; the frozen decision must justify the root contract state",
                    row.id
                ));
            }
            if contract.spec_required_rationale != row.attempt.contract_decision.decision_rationale
            {
                failures.push(format!(
                    "row `{}` contract spec-required rationale drifted from the embedded attempt decision rationale; the retained authority and the frozen decision must state the same justification",
                    row.id
                ));
            }
            if contract.root_disposition != row.attempt.disposition {
                failures.push(format!(
                    "row `{}` contract root disposition {:?} disagrees with the attempt disposition {:?}",
                    row.id, contract.root_disposition, row.attempt.disposition
                ));
            }
            // State binding: the frozen attempt's root contract state is the
            // outer contract state; mutating the committed state from amended
            // to rejected without re-freezing the attempt fails closed instead
            // of emitting a packet that contradicts the scorecard attempt.
            match normalized_contract_state(&row.attempt.contract_decision.root_disposition) {
                Ok(Some(state)) if state == contract.contract_state => {}
                Ok(Some(state)) => failures.push(format!(
                    "row `{}` contract state {:?} disagrees with the embedded attempt root contract state {:?}",
                    row.id, contract.contract_state, state
                )),
                Ok(None) => failures.push(format!(
                    "row `{}` embeds no root contract state; the frozen attempt must record the root's state decision",
                    row.id
                )),
                Err(reason) => failures.push(format!(
                    "row `{}` embedded attempt records an unknown root contract state: {reason}",
                    row.id
                )),
            }
            // Amendment law: an amended root contract state must retain what
            // the root corrected; the field lives outside the embedded
            // attempt digest, so without this binding an amended packet can
            // emit no correction evidence at all.
            if contract.contract_state == IssueLifecycleContractStateV1::Amended {
                if row.planning.root_corrections.is_empty() {
                    failures.push(format!(
                        "row `{}` contract state is amended but records no root corrections; the cold-start packet must retain what the root corrected",
                        row.id
                    ));
                }
                if row
                    .planning
                    .root_corrections
                    .iter()
                    .any(|correction| correction.trim().is_empty())
                {
                    failures.push(format!(
                        "row `{}` records a blank root correction; an amendment that names nothing is not retained evidence",
                        row.id
                    ));
                }
            }
            if contract.author.carries_acceptance {
                failures.push(format!(
                    "row `{}` author result `{}` carries an acceptance state; the author cannot accept its own contract",
                    row.id, contract.author.result_identity
                ));
            }
            if contract.author.result_identity == contract.adversary.result_identity {
                failures.push(format!(
                    "row `{}` author and adversary results must be distinct identities",
                    row.id
                ));
            }
            if contract.root.role.trim().is_empty()
                || contract.root.config_identity.trim().is_empty()
                || contract.root.result_identity.trim().is_empty()
            {
                failures.push(format!(
                    "row `{}` root role/config/result identity must be complete; a corpus with no root actor cannot claim an independent root decision",
                    row.id
                ));
            }
            if contract.root.role == contract.author.role {
                failures.push(format!(
                    "row `{}` root role `{}` duplicates the author role; the acceptance authority must be identified independently of the author",
                    row.id, contract.root.role
                ));
            }
            if contract.root.config_identity == contract.author.config_identity {
                failures.push(format!(
                    "row `{}` root config identity `{}` duplicates the author config identity; the acceptance authority must be configured independently of the author",
                    row.id, contract.root.config_identity
                ));
            }
            if contract.root.result_identity == contract.author.result_identity
                || contract.root.result_identity == contract.adversary.result_identity
                || contract.author.result_identity == contract.adversary.result_identity
            {
                failures.push(format!(
                    "row `{}` author, adversary and root results must be pairwise distinct identities",
                    row.id
                ));
            }
            if !contract.root.carries_acceptance {
                failures.push(format!(
                    "row `{}` root result must carry the acceptance state; the root disposition is the only acceptance authority",
                    row.id
                ));
            }
            if contract.author.role.trim().is_empty()
                || contract.author.config_identity.trim().is_empty()
                || contract.author.result_identity.trim().is_empty()
            {
                failures.push(format!(
                    "row `{}` author role/config/result identity must be complete",
                    row.id
                ));
            }
            if contract.adversary.result_identity.trim().is_empty() {
                failures.push(format!(
                    "row `{}` adversary records no result identity",
                    row.id
                ));
            }
            if contract.adversary.inspected_scope.trim().is_empty() {
                failures.push(format!(
                    "row `{}` adversary records no inspected scope",
                    row.id
                ));
            }
            if contract
                .adversary
                .findings
                .iter()
                .any(|finding| finding.trim().is_empty())
            {
                failures.push(format!(
                    "row `{}` adversary findings must be nonblank; a blank finding carries no missing-failure evidence",
                    row.id
                ));
            }
            let findings_reported = !contract.adversary.findings.is_empty();
            if !(contract.adversary.none_found ^ findings_reported) {
                failures.push(format!(
                    "row `{}` adversary findings and bounded none_found must be mutually exclusive",
                    row.id
                ));
            }
            if !contract.draft_spec_identity.starts_with("RIPR-SPEC-draft-") {
                failures.push(format!(
                    "row `{}` draft identity `{}` is not a behavior draft; specs contain behavior, not execution queues",
                    row.id, contract.draft_spec_identity
                ));
            }
            if row.attempt.contract_artifacts.proposal.as_deref()
                != Some(contract.draft_spec_identity.as_str())
            {
                failures.push(format!(
                    "row `{}` draft identity `{}` drifted from the embedded proposal identity {:?}; the packet must name the artifact the frozen attempt authenticates",
                    row.id, contract.draft_spec_identity, row.attempt.contract_artifacts.proposal
                ));
            }
            if row.attempt.contract_artifacts.challenge.as_deref()
                != Some(contract.adversary.result_identity.as_str())
            {
                failures.push(format!(
                    "row `{}` adversary result identity `{}` drifted from the embedded challenge identity {:?}; the packet must name the artifact the frozen attempt authenticates",
                    row.id, contract.adversary.result_identity, row.attempt.contract_artifacts.challenge
                ));
            }
            if row.attempt.disposition == IssueLifecycleDispositionV1::QualifiedOnePr {
                failures.push(format!(
                    "row `{}` is contract-required and can never qualify as direct one-PR work",
                    row.id
                ));
            }
            // Preservation law: a real contract-required row must retain its
            // open decisions; the field lives outside the embedded attempt
            // digest, so clearing it would silently skip every
            // unresolved-decision guard below. Synthetic control rows may
            // legitimately carry no open decisions.
            if !row.attempt.synthetic && contract.open_decisions.is_empty() {
                failures.push(format!(
                    "row `{}` is a real contract-required row but retains no open decisions; clearing the preserved decisions must fail closed instead of skipping the unresolved-decision guards",
                    row.id
                ));
            }
            if !contract.open_decisions.is_empty() {
                if contract.contract_state == IssueLifecycleContractStateV1::Accepted {
                    failures.push(format!(
                        "row `{}` claims an accepted contract state while open decisions remain; acceptance cannot outrun unresolved decisions",
                        row.id
                    ));
                }
                if row.planning.stop_conditions.is_empty() {
                    failures.push(format!(
                        "row `{}` preserves open decisions but records no stop condition; unresolved decisions stop the plan rather than being guessed",
                        row.id
                    ));
                }
                if row.planning.stop_conditions.len() < contract.open_decisions.len() {
                    failures.push(format!(
                        "row `{}` preserves {} open decisions but records only {} stop conditions; every open decision must stop the plan",
                        row.id,
                        contract.open_decisions.len(),
                        row.planning.stop_conditions.len()
                    ));
                }
                if contract
                    .open_decisions
                    .iter()
                    .any(|decision| decision.trim().is_empty())
                {
                    failures.push(format!(
                        "row `{}` records a blank open decision; a decision that names nothing cannot bind stop conditions",
                        row.id
                    ));
                }
                if row
                    .planning
                    .stop_conditions
                    .iter()
                    .any(|condition| condition.trim().is_empty())
                {
                    failures.push(format!(
                        "row `{}` records a blank stop condition; an empty condition stops nothing",
                        row.id
                    ));
                }
                if matches!(
                    row.attempt.disposition,
                    IssueLifecycleDispositionV1::Completed
                        | IssueLifecycleDispositionV1::QualifiedOnePr
                        | IssueLifecycleDispositionV1::PartiallyLanded
                        | IssueLifecycleDispositionV1::VerificationFailed
                        | IssueLifecycleDispositionV1::MergedPendingCloseout
                ) {
                    failures.push(format!(
                        "row `{}` binds open decisions but claims an implementation-ready disposition {:?}",
                        row.id, row.attempt.disposition
                    ));
                }
                if row.attempt.contract_artifacts.acceptance.is_some() {
                    failures.push(format!(
                        "row `{}` records contract acceptance evidence while open decisions remain; acceptance cannot outrun the root disposition",
                        row.id
                    ));
                }
            }
        }
        "narrow_accepted_contract_bug" => {
            if row.contract.is_some() {
                failures.push(format!(
                    "row `{}` is a narrow bug but records contract evidence; no unnecessary spec",
                    row.id
                ));
            }
            if row.attempt.contract_decision.spec_required {
                failures.push(format!(
                    "row `{}` narrow bug must not require a spec",
                    row.id
                ));
            }
            if row.attempt.contract_decision.root_disposition.is_some() {
                failures.push(format!(
                    "row `{}` narrow bug must not record a root contract state; no contract decision exists",
                    row.id
                ));
            }
            let artifacts = &row.attempt.contract_artifacts;
            if artifacts.proposal.is_some()
                || artifacts.spec.is_some()
                || artifacts.adr.is_some()
                || artifacts.challenge.is_some()
                || !artifacts.amendments.is_empty()
                || artifacts.acceptance.is_some()
            {
                failures.push(format!(
                    "row `{}` narrow bug must carry no contract artifacts",
                    row.id
                ));
            }
            if row.attempt.disposition != IssueLifecycleDispositionV1::QualifiedOnePr {
                failures.push(format!(
                    "row `{}` narrow bug must qualify as one-PR work, got {:?}",
                    row.id, row.attempt.disposition
                ));
            }
            if row.planning.shape_decision != IssueLifecyclePlanShapeV1::OnePr {
                failures.push(format!(
                    "row `{}` narrow bug must plan as one PR, got {:?}",
                    row.id, row.planning.shape_decision
                ));
            }
        }
        other => failures.push(format!("row `{}` has unknown category `{other}`", row.id)),
    }
    // Identity law: the outer row id and the embedded lifecycle id must be
    // one identity; the row id lives outside the attempt digest, so a changed
    // outer id would otherwise emit two conflicting identities for one
    // lifecycle.
    if row.id != row.attempt.lifecycle_id {
        failures.push(format!(
            "row `{}` outer id disagrees with the embedded lifecycle id `{}`; one lifecycle must present one identity",
            row.id, row.attempt.lifecycle_id
        ));
    }
    // Planning law: work order, not behavior authority. No planning surface
    // may mint a draft spec identity; draft specs live on the contract
    // evidence only.
    for string in planning_strings(&row.planning) {
        if string.contains("RIPR-SPEC-draft") {
            failures.push(format!(
                "row `{}` planning surface mints behavior authority (`{string}`); plans contain work order, not new behavior authority",
                row.id
            ));
        }
        if string.contains("active.toml") || string.contains("selected_work") {
            failures.push(format!(
                "row `{}` planning surface references a tracked selection or current-work file (`{string}`)",
                row.id
            ));
        }
    }
    // Work-item identity law: ids must be nonblank and unique before edges
    // are resolved; a duplicate collapses two items into one ambiguous order.
    let mut work_item_ids: BTreeSet<&str> = BTreeSet::new();
    for item in &row.planning.work_items {
        if item.id.trim().is_empty() {
            failures.push(format!(
                "row `{}` records a blank work item id; an unnamed item cannot be ordered or depended on",
                row.id
            ));
        } else if !work_item_ids.insert(item.id.as_str()) {
            failures.push(format!(
                "row `{}` duplicates work item id `{}`; dependency edges over duplicated ids are ambiguous",
                row.id, item.id
            ));
        }
    }
    // Dependency-edge law: every depends_on target must name another work
    // item in the same plan; a dangling edge is a guessed dependency.
    for item in &row.planning.work_items {
        for dependency in &item.depends_on {
            if !work_item_ids.contains(dependency.as_str()) {
                failures.push(format!(
                    "row `{}` work item `{}` depends on unknown work item `{dependency}`",
                    row.id, item.id
                ));
            }
        }
    }
    // Cycle law: a self-edge or a dependency cycle can never become ready,
    // so the claimed work order is one the packet must not emit.
    if let Some(cycle) = work_item_dependency_cycle(&row.planning.work_items) {
        failures.push(format!(
            "row `{}` work item dependencies form a cycle involving `{cycle}`; cyclic items can never become ready",
            row.id
        ));
    }
    // Drift law: the outer planning evidence and the embedded RIPR-SPEC-0218
    // attempt plan must describe the same work; a plan that drifted from the
    // attempt it claims to plan fails closed.
    let mut planned_ids: Vec<&str> = row
        .planning
        .work_items
        .iter()
        .map(|item| item.id.as_str())
        .collect();
    let mut attempt_ids: Vec<&str> = row
        .attempt
        .plan
        .work_items
        .iter()
        .map(String::as_str)
        .collect();
    planned_ids.sort_unstable();
    attempt_ids.sort_unstable();
    if planned_ids != attempt_ids {
        failures.push(format!(
            "row `{}` planning work items drifted from the embedded attempt plan work items",
            row.id
        ));
    }
    let mut planned_edges: Vec<String> = row
        .planning
        .work_items
        .iter()
        .flat_map(|item| {
            item.depends_on
                .iter()
                .map(move |dependency| format!("{}->{dependency}", item.id))
        })
        .collect();
    let mut attempt_edges = row.attempt.plan.dependencies.clone();
    planned_edges.sort();
    attempt_edges.sort();
    if planned_edges != attempt_edges {
        failures.push(format!(
            "row `{}` planning dependency edges drifted from the embedded attempt plan dependencies",
            row.id
        ));
    }
    let mut planned_coverage = row.planning.acceptance_covered.clone();
    let mut attempt_coverage = row.attempt.plan.acceptance_coverage.clone();
    planned_coverage.sort();
    attempt_coverage.sort();
    if planned_coverage != attempt_coverage {
        failures.push(format!(
            "row `{}` planning acceptance coverage drifted from the embedded attempt plan acceptance coverage",
            row.id
        ));
    }
    // Acceptance law: explicit omissions must carry a reason; an omitted row
    // is disclosed, never hidden.
    for omitted in &row.planning.acceptance_omitted {
        if omitted.trim().is_empty() || !omitted.contains(':') {
            failures.push(format!(
                "row `{}` acceptance omission `{omitted}` must name the omitted row and its reason after a colon",
                row.id
            ));
        }
    }
    // Acceptance binding: every covered or omitted entry must reference the
    // captured issue, no entry may be blank or repeated, and covered plus
    // omitted rows must partition disjointly; self-consistent unrelated text
    // is not coverage evidence. Synthetic control rows name no captured
    // issue, so the issue marker binds only real snapshot-numbered rows.
    if row.snapshot.issue_number > 0 {
        let marker = format!("#{}", row.snapshot.issue_number);
        for entry in row
            .planning
            .acceptance_covered
            .iter()
            .chain(row.planning.acceptance_omitted.iter())
        {
            if !entry.contains(&marker) {
                failures.push(format!(
                    "row `{}` acceptance entry `{entry}` does not reference the captured issue {marker}; coverage must bind the issue it claims to cover",
                    row.id
                ));
            }
        }
    }
    if row
        .planning
        .acceptance_covered
        .iter()
        .chain(row.planning.acceptance_omitted.iter())
        .any(|entry| entry.trim().is_empty())
    {
        failures.push(format!(
            "row `{}` records a blank acceptance entry; an entry that names nothing covers nothing",
            row.id
        ));
    }
    let mut acceptance_entries = row.planning.acceptance_covered.clone();
    acceptance_entries.extend(row.planning.acceptance_omitted.iter().cloned());
    let mut unique_entries = acceptance_entries.clone();
    unique_entries.sort();
    unique_entries.dedup();
    if unique_entries.len() != acceptance_entries.len() {
        failures.push(format!(
            "row `{}` duplicates an acceptance entry across covered and omitted rows; each acceptance row may be claimed once",
            row.id
        ));
    }
    // Proof law: every proof command must name its denominator.
    for proof in &row.planning.proof_commands {
        if proof.command.trim().is_empty() || proof.denominator.trim().is_empty() {
            failures.push(format!(
                "row `{}` proof commands must name the command and its denominator",
                row.id
            ));
        }
    }
    // Completeness law: a counted row carries a bounded plan; clearing a
    // mandatory section vacates the claim that the plan bounds the work, and
    // acceptance must be covered or explicitly omitted with reasons.
    if row.planning.shape_rationale.trim().is_empty() {
        failures.push(format!(
            "row `{}` plan records no shape rationale; the shape decision must be justified on the record",
            row.id
        ));
    }
    if row.planning.portfolio_placement.trim().is_empty() {
        failures.push(format!(
            "row `{}` plan records no portfolio placement; the row must name its portfolio lane without writing a selection file",
            row.id
        ));
    }
    for (section, entries) in [
        ("work items", row.planning.work_items.len()),
        ("edit cages", row.planning.edit_cages.len()),
        (
            "semantic conflict resources",
            row.planning.conflict_resources.len(),
        ),
        ("proof commands", row.planning.proof_commands.len()),
        ("stop conditions", row.planning.stop_conditions.len()),
        ("non-goals", row.planning.non_goals.len()),
    ] {
        if entries == 0 {
            failures.push(format!(
                "row `{}` plan clears its mandatory {section}; an emptied plan cannot bound the work",
                row.id
            ));
        }
    }
    for (section, entries) in [
        ("edit cages", &row.planning.edit_cages),
        (
            "semantic conflict resources",
            &row.planning.conflict_resources,
        ),
        ("stop conditions", &row.planning.stop_conditions),
        ("non-goals", &row.planning.non_goals),
    ] {
        if entries.iter().any(|entry| entry.trim().is_empty()) {
            failures.push(format!(
                "row `{}` plan records a blank {section} entry; an entry that names nothing cannot bound the work",
                row.id
            ));
        }
    }
    if row.planning.acceptance_covered.is_empty() && row.planning.acceptance_omitted.is_empty() {
        failures.push(format!(
            "row `{}` plan covers no acceptance row and omits none; acceptance must be covered or explicitly omitted with reasons",
            row.id
        ));
    }
    // Claim-boundary law: the outer limitations and non_claims duplicate the
    // digest-protected attempt fields and must state the same boundary;
    // clearing the outer lists would emit a packet that overstates the
    // counted attempt.
    if row.limitations != row.attempt.limitations {
        failures.push(format!(
            "row `{}` outer limitations drifted from the embedded attempt limitations; the packet must not overstate the counted claim boundary",
            row.id
        ));
    }
    if row.non_claims != row.attempt.non_claims {
        failures.push(format!(
            "row `{}` outer non_claims drifted from the embedded attempt non_claims; the packet must not overstate the counted claim boundary",
            row.id
        ));
    }
    failures
}

/// Detect a self-edge or a dependency cycle in the work-item dependency
/// graph, returning one involved id. Depth-first with gray/black coloring;
/// committed plans are small, so recursion stays shallow.
fn work_item_dependency_cycle(items: &[IssueLifecyclePlanWorkItemV1]) -> Option<String> {
    use std::collections::BTreeMap;
    let adjacency: BTreeMap<&str, Vec<&str>> = items
        .iter()
        .map(|item| {
            (
                item.id.as_str(),
                item.depends_on.iter().map(String::as_str).collect(),
            )
        })
        .collect();
    fn visit<'a>(
        node: &'a str,
        adjacency: &BTreeMap<&'a str, Vec<&'a str>>,
        gray: &mut BTreeSet<&'a str>,
        black: &mut BTreeSet<&'a str>,
    ) -> Option<&'a str> {
        gray.insert(node);
        for next in adjacency.get(node).into_iter().flatten().copied() {
            if gray.contains(next) {
                return Some(next);
            }
            if black.contains(next) {
                continue;
            }
            if let Some(cycle) = visit(next, adjacency, gray, black) {
                return Some(cycle);
            }
        }
        gray.remove(node);
        black.insert(node);
        None
    }
    let mut gray = BTreeSet::new();
    let mut black = BTreeSet::new();
    for node in adjacency.keys().copied() {
        if black.contains(node) {
            continue;
        }
        if let Some(cycle) = visit(node, &adjacency, &mut gray, &mut black) {
            return Some(cycle.to_string());
        }
    }
    None
}

/// Assess every row of the committed corpus plus every synthetic control row
/// by the same decision-boundary law.
pub(crate) fn assess_contract_plan_corpus(
    corpus: &IssueLifecycleContractPlanCorpusV1,
) -> Vec<String> {
    let mut failures = Vec::new();
    for row in &corpus.rows {
        failures.extend(assess_contract_plan_row(row));
    }
    failures
}

/// Every named mechanics control binds one decision-boundary law, and the row
/// carrying that name must demonstrate that law's discriminating behavior:
/// dispatching every control through the same generic row assessor would let
/// a corpus attach all ten required control names to copies of an unrelated
/// clean row and still pass. Each closed control name selects its own
/// behavior-specific expectation here.
fn control_expectation_failures(control: &IssueLifecycleContractPlanControlRowV1) -> Vec<String> {
    let mut failures = Vec::new();
    let row = &control.row;
    let post_implementation = |disposition: IssueLifecycleDispositionV1| {
        matches!(
            disposition,
            IssueLifecycleDispositionV1::Completed
                | IssueLifecycleDispositionV1::QualifiedOnePr
                | IssueLifecycleDispositionV1::PartiallyLanded
                | IssueLifecycleDispositionV1::VerificationFailed
                | IssueLifecycleDispositionV1::MergedPendingCloseout
        )
    };
    match control.control.as_str() {
        "adversary_catches_missing_failure_state_or_bounded_none_found" => {
            if row.contract.is_none() {
                failures.push(format!(
                    "control `{}` must carry contract evidence so the adversary result exists",
                    control.id
                ));
            }
        }
        "author_cannot_accept_own_contract" => {
            let contract = row.contract.as_ref();
            if contract.is_none_or(|contract| {
                contract.author.carries_acceptance || contract.author.role != "contract_author"
            }) {
                failures.push(format!(
                    "control `{}` must keep the author free of acceptance with role contract_author",
                    control.id
                ));
            }
        }
        "unresolved_decision_blocks_implementation" => {
            let contract = row.contract.as_ref();
            if contract.is_none_or(|contract| contract.open_decisions.is_empty())
                || row.planning.stop_conditions.is_empty()
                || post_implementation(row.attempt.disposition)
            {
                failures.push(format!(
                    "control `{}` must preserve an open decision, bind stop conditions and stay clear of implementation-ready dispositions",
                    control.id
                ));
            }
        }
        "narrow_bug_gets_no_unnecessary_spec" => {
            let artifacts = &row.attempt.contract_artifacts;
            if row.category != "narrow_accepted_contract_bug"
                || row.contract.is_some()
                || row.attempt.contract_decision.spec_required
                || row.attempt.disposition != IssueLifecycleDispositionV1::QualifiedOnePr
                || row.planning.shape_decision != IssueLifecyclePlanShapeV1::OnePr
                || artifacts.proposal.is_some()
                || artifacts.spec.is_some()
                || artifacts.adr.is_some()
                || artifacts.challenge.is_some()
                || !artifacts.amendments.is_empty()
                || artifacts.acceptance.is_some()
            {
                failures.push(format!(
                    "control `{}` must route a narrow accepted-contract bug directly to one PR with no contract evidence and no contract artifacts",
                    control.id
                ));
            }
        }
        "campaign_rejected_when_one_vertical_slice_suffices" => {
            if row.planning.shape_decision != IssueLifecyclePlanShapeV1::OnePr
                || !row.planning.shape_rationale.contains("rejected")
            {
                failures.push(format!(
                    "control `{}` must land on one_pr with the campaign rejection on the record",
                    control.id
                ));
            }
        }
        "one_pr_rejected_when_acceptance_cannot_be_covered_coherently" => {
            if row.planning.shape_decision != IssueLifecyclePlanShapeV1::Campaign
                || !row.planning.shape_rationale.contains("rejected")
            {
                failures.push(format!(
                    "control `{}` must land on campaign with the one-PR rejection on the record",
                    control.id
                ));
            }
        }
        "specs_contain_behavior_not_execution_queues" => {
            if row
                .contract
                .as_ref()
                .is_none_or(|contract| contract.draft_spec_identity.trim().is_empty())
            {
                failures.push(format!(
                    "control `{}` must carry a behavior draft identity, never an execution queue",
                    control.id
                ));
            }
        }
        "plans_contain_work_order_not_new_behavior_authority" => {
            if row.category != "contract_required"
                || row.planning.work_items.is_empty()
                || row.planning.stop_conditions.is_empty()
            {
                failures.push(format!(
                    "control `{}` must exercise the plan law on a spec-required plan carrying work items and stop conditions, never behavior authority",
                    control.id
                ));
            }
        }
        "no_tracked_selection_or_current_work_file_changed" => {
            if row.category != "contract_required"
                || row.planning.portfolio_placement.trim().is_empty()
            {
                failures.push(format!(
                    "control `{}` must record a portfolio placement label on a spec-required plan without touching a tracked selection or current-work file",
                    control.id
                ));
            }
        }
        "cold_start_root_reconstructs_decisions_from_artifacts" => {
            if row.contract.is_none() {
                failures.push(format!(
                    "control `{}` must carry the full contract evidence a cold-start root reconstructs from",
                    control.id
                ));
            }
        }
        other => failures.push(format!(
            "control `{}` names unknown control `{other}`",
            control.id
        )),
    }
    failures
}

pub(crate) fn assess_contract_plan_control_corpus(
    corpus: &IssueLifecycleContractPlanControlCorpusV1,
) -> Vec<String> {
    let mut failures = Vec::new();
    for control in &corpus.rows {
        failures.extend(assess_contract_plan_row(&control.row));
        failures.extend(control_expectation_failures(control));
    }
    // The adversary control covers both branches: one row where the adversary
    // returns concrete findings and one where it returns a bounded none_found.
    let adversary_rows = corpus
        .rows
        .iter()
        .filter(|control| {
            control.control == "adversary_catches_missing_failure_state_or_bounded_none_found"
        })
        .collect::<Vec<_>>();
    let findings_branch = adversary_rows.iter().any(|control| {
        control.row.contract.as_ref().is_some_and(|contract| {
            !contract.adversary.findings.is_empty() && !contract.adversary.none_found
        })
    });
    let none_found_branch = adversary_rows.iter().any(|control| {
        control.row.contract.as_ref().is_some_and(|contract| {
            contract.adversary.findings.is_empty() && contract.adversary.none_found
        })
    });
    if !findings_branch || !none_found_branch {
        failures.push(
            "control corpus must exercise both the findings and the bounded none_found branches of adversary_catches_missing_failure_state_or_bounded_none_found"
                .to_string(),
        );
    }
    failures
}

fn parse_corpus_dir_arg(args: &[String]) -> Result<String, String> {
    const USAGE: &str =
        "usage: cargo xtask issue-lifecycle-contract-plan-scorecard [--corpus <dir>]";
    let mut corpus = None;
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--corpus" | "--captured" => {
                let value = args
                    .get(index + 1)
                    .ok_or_else(|| format!("missing value for {}\n{USAGE}", args[index]))?;
                if corpus.replace(value.clone()).is_some() {
                    return Err(format!("duplicate corpus path\n{USAGE}"));
                }
                index += 2;
            }
            "--help" | "-h" => return Err(USAGE.to_string()),
            other => return Err(format!("unknown argument `{other}`\n{USAGE}")),
        }
    }
    Ok(corpus.unwrap_or_else(|| DEFAULT_CONTRACT_PLAN_CORPUS_DIR.to_string()))
}

/// Fail-closed provenance shape check: the provenance must name exactly the
/// corpus rows — no missing entry, no duplicate issue, and no extra row the
/// corpus does not contain — and must record the same capture instant and
/// base main as the corpus it describes.
pub(crate) fn check_provenance_against_corpus(
    corpus: &IssueLifecycleContractPlanCorpusV1,
    provenance: &IssueLifecycleContractPlanProvenanceV1,
) -> Result<(), String> {
    if provenance.base_main != corpus.base_main {
        return Err(format!(
            "contract plan provenance base main `{}` disagrees with the corpus base main `{}`",
            provenance.base_main, corpus.base_main
        ));
    }
    if provenance.repository.trim().is_empty() {
        return Err("contract plan provenance records no repository".to_string());
    }
    if provenance.captured_at != corpus.captured_at {
        return Err(format!(
            "contract plan provenance captured at `{}` disagrees with the corpus captured at `{}`",
            provenance.captured_at, corpus.captured_at
        ));
    }
    if provenance.rows.len() != corpus.rows.len() {
        return Err(format!(
            "contract plan provenance must name {} rows, got {}",
            corpus.rows.len(),
            provenance.rows.len()
        ));
    }
    let mut provenance_issues = BTreeSet::new();
    for entry in &provenance.rows {
        if entry.justification.trim().is_empty() {
            return Err(format!(
                "contract plan provenance for issue `{}` records no justification; the retained evidence must say why the issue belongs on its decision-boundary side",
                entry.issue
            ));
        }
        if !provenance_issues.insert(entry.issue) {
            return Err(format!(
                "contract plan provenance duplicates issue `{}`",
                entry.issue
            ));
        }
    }
    for row in &corpus.rows {
        if !provenance_issues.contains(&row.snapshot.issue_number) {
            return Err(format!(
                "contract plan row `{}` has no provenance entry for issue `{}`",
                row.id, row.snapshot.issue_ref
            ));
        }
        let entry = provenance
            .rows
            .iter()
            .find(|entry| entry.issue == row.snapshot.issue_number)
            .map(|entry| entry.category.clone())
            .unwrap_or_default();
        if entry != row.category {
            return Err(format!(
                "contract plan row `{}` category `{}` disagrees with its provenance category `{entry}`",
                row.id, row.category
            ));
        }
        // Repository binding: the provenance repository must be the one
        // common repository every row issue ref names; a provenance drift to
        // an unrelated repository must not pass while the captured rows
        // still authenticate another one.
        let row_repository = row
            .snapshot
            .issue_ref
            .split_once('#')
            .map(|(repository, _)| repository)
            .ok_or_else(|| {
                format!(
                    "contract plan row `{}` issue ref `{}` is not an `owner/repo#number` reference",
                    row.id, row.snapshot.issue_ref
                )
            })?;
        if row_repository != provenance.repository {
            return Err(format!(
                "contract plan row `{}` issue ref `{}` names repository `{row_repository}` but the provenance records repository `{}`",
                row.id, row.snapshot.issue_ref, provenance.repository
            ));
        }
        // Currentness binding: every duplicated capture-time field must agree
        // with the authoritative corpus values, so a row cannot pass while its
        // emitted projection claims a different main or capture instant.
        if row.current_main != corpus.base_main {
            return Err(format!(
                "contract plan row `{}` current main `{}` disagrees with the corpus base main `{}`",
                row.id, row.current_main, corpus.base_main
            ));
        }
        if row.snapshot.captured_at != corpus.captured_at {
            return Err(format!(
                "contract plan row `{}` snapshot captured at `{}` disagrees with the corpus captured at `{}`",
                row.id, row.snapshot.captured_at, corpus.captured_at
            ));
        }
        if row.attempt.context.current_main != corpus.base_main {
            return Err(format!(
                "contract plan row `{}` embedded attempt context main `{}` disagrees with the corpus base main `{}`",
                row.id, row.attempt.context.current_main, corpus.base_main
            ));
        }
        if row.attempt.execution_refs.current_main != corpus.base_main {
            return Err(format!(
                "contract plan row `{}` embedded attempt execution main `{}` disagrees with the corpus base main `{}`",
                row.id, row.attempt.execution_refs.current_main, corpus.base_main
            ));
        }
    }
    Ok(())
}

/// Read one committed contract/plan corpus directory end to end: corpus,
/// controls, provenance and snapshot bindings, all fail-closed. Shared by the
/// scorecard command and the fixture-contract gate.
pub(crate) fn load_contract_plan_corpus_dir(
    dir: &Path,
) -> Result<
    (
        IssueLifecycleContractPlanCorpusV1,
        IssueLifecycleContractPlanControlCorpusV1,
        IssueLifecycleContractPlanProvenanceV1,
    ),
    String,
> {
    let corpus_body = fs::read_to_string(dir.join("corpus.json"))
        .map_err(|error| format!("read issue lifecycle contract plan corpus: {error}"))?;
    let corpus = load_issue_lifecycle_contract_plan_corpus(&corpus_body)?;
    let controls_body = fs::read_to_string(dir.join("controls.json"))
        .map_err(|error| format!("read issue lifecycle contract plan controls: {error}"))?;
    let controls = load_issue_lifecycle_contract_plan_control_corpus(&controls_body)?;
    let provenance_body = fs::read_to_string(dir.join("provenance.json"))
        .map_err(|error| format!("read issue lifecycle contract plan provenance: {error}"))?;
    let provenance = load_issue_lifecycle_contract_plan_provenance(&provenance_body)?;
    check_provenance_against_corpus(&corpus, &provenance)?;
    for row in &corpus.rows {
        verify_contract_plan_row_snapshot(row, dir)?;
    }
    Ok((corpus, controls, provenance))
}

/// Fail-closed RIPR-SPEC-0218 assessment of the embedded real attempt rows:
/// a rejected row fails the run, and a counted row whose deterministically
/// downgraded assessed disposition disagrees with the row's independent root
/// disposition fails too — the command never emits a scorecard stronger than
/// the retained evidence. For contract-required rows the root disposition
/// lives on the contract evidence; narrow rows bind no separate root.
pub(crate) fn assess_real_rows_against_counting_law(
    corpus: &IssueLifecycleContractPlanCorpusV1,
) -> (Vec<String>, Vec<IssueLifecycleRowAssessmentV1>) {
    let attempts: Vec<IssueLifecycleAttemptV1> =
        corpus.rows.iter().map(|row| row.attempt.clone()).collect();
    let assessed: Vec<IssueLifecycleRowAssessmentV1> = attempts
        .iter()
        .map(assess_issue_lifecycle_attempt)
        .collect();
    let mut failures = Vec::new();
    for (row, assessment) in corpus.rows.iter().zip(assessed.iter()) {
        if !assessment.counted {
            failures.push(format!(
                "contract plan row `{}` attempt was rejected by the RIPR-SPEC-0218 counting law: {:?}",
                row.id, assessment.reasons
            ));
        }
        let root = row
            .contract
            .as_ref()
            .map(|contract| contract.root_disposition)
            .unwrap_or(row.attempt.disposition);
        if assessment.disposition != Some(root) {
            failures.push(format!(
                "contract plan row `{}` root disposition {:?} disagrees with the assessed disposition {:?}",
                row.id, root, assessment.disposition
            ));
        }
    }
    (failures, assessed)
}

/// Fail-closed RIPR-SPEC-0218 assessment of the synthetic control rows:
/// a rejected control fails the run, and a counted control whose
/// deterministically downgraded assessed disposition disagrees with its
/// independent root disposition fails too — the shared counting law may keep
/// a downgraded row counted, so counted alone is not agreement. Shared by the
/// scorecard command and the fixture-contract gate so both surfaces enforce
/// the same control counting law.
pub(crate) fn assess_control_rows_against_counting_law(
    corpus: &IssueLifecycleContractPlanControlCorpusV1,
) -> Vec<String> {
    let mut failures = Vec::new();
    for control in &corpus.rows {
        let assessment = assess_issue_lifecycle_attempt(&control.row.attempt);
        if !assessment.counted {
            failures.push(format!(
                "contract plan control `{}` was rejected by the RIPR-SPEC-0218 counting law: {:?}",
                control.id, assessment.reasons
            ));
        }
        let root = control
            .row
            .contract
            .as_ref()
            .map(|contract| contract.root_disposition)
            .unwrap_or(control.row.attempt.disposition);
        if assessment.disposition != Some(root) {
            failures.push(format!(
                "contract plan control `{}` assessed disposition {:?} disagrees with its root disposition {:?}",
                control.id, assessment.disposition, root
            ));
        }
    }
    failures
}

/// `cargo xtask issue-lifecycle-contract-plan-scorecard [--corpus <dir>]`
/// (#4931, RIPR-SPEC-0236): validate the committed read-only contract/plan
/// corpus fail-closed (shape, provenance, snapshot digest bindings,
/// decision-boundary law), then project the embedded real attempt rows
/// through the unchanged RIPR-SPEC-0218 validator and scorecard builder,
/// writing the standard issue-lifecycle scorecard reports — results flow
/// through #4929 without a parallel contract/plan report. The deterministic
/// per-row contract/plan projections are printed to stdout.
pub(crate) fn issue_lifecycle_contract_plan_scorecard(args: &[String]) -> Result<(), String> {
    let corpus_dir = parse_corpus_dir_arg(args)?;
    let root = workspace_path(&corpus_dir);
    let (corpus, controls, _provenance) = load_contract_plan_corpus_dir(&root)?;
    let mut failures = assess_contract_plan_corpus(&corpus);
    failures.extend(assess_contract_plan_control_corpus(&controls));
    let (law_failures, assessed) = assess_real_rows_against_counting_law(&corpus);
    failures.extend(law_failures);
    let attempts: Vec<IssueLifecycleAttemptV1> =
        corpus.rows.iter().map(|row| row.attempt.clone()).collect();
    failures.extend(assess_control_rows_against_counting_law(&controls));
    if !failures.is_empty() {
        return Err(format!(
            "issue lifecycle contract plan corpus failed assessment: {failures:?}"
        ));
    }
    let corpus_identity = crate::reports::issue_lifecycle_corpus_identity(&attempts)?;
    let scorecard =
        crate::reports::build_issue_lifecycle_scorecard(&assessed, corpus_identity, &corpus_dir);
    let json_body = crate::reports::issue_lifecycle_scorecard_json(&scorecard)?;
    crate::write_report("issue-lifecycle-scorecard.json", &json_body)?;
    crate::write_report(
        "issue-lifecycle-scorecard.md",
        &crate::reports::issue_lifecycle_scorecard_markdown(&scorecard),
    )?;
    println!("{json_body}");
    println!("{}", ISSUE_LIFECYCLE_CONTRACT_PLAN_CLAIM_BOUNDARY);
    let mut projections: Vec<IssueLifecycleContractPlanProjectionV1> = corpus
        .rows
        .iter()
        .map(build_contract_plan_projection)
        .collect();
    projections.sort_by(|left, right| left.id.cmp(&right.id));
    let packet_body = serde_json::to_string_pretty(&projections)
        .map_err(|error| format!("serialize contract plan projections: {error}"))?;
    println!("{packet_body}");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::issue_lifecycle_intake::IssueLifecycleIntakeBytesV1;

    fn contract_plan_root() -> PathBuf {
        workspace_path(DEFAULT_CONTRACT_PLAN_CORPUS_DIR)
    }

    fn load_committed() -> Result<
        (
            IssueLifecycleContractPlanCorpusV1,
            IssueLifecycleContractPlanControlCorpusV1,
            IssueLifecycleContractPlanProvenanceV1,
        ),
        String,
    > {
        load_contract_plan_corpus_dir(&contract_plan_root())
    }

    fn row_by_category<'a>(
        corpus: &'a IssueLifecycleContractPlanCorpusV1,
        category: &str,
    ) -> Result<&'a IssueLifecycleContractPlanRowV1, String> {
        corpus
            .rows
            .iter()
            .find(|row| row.category == category)
            .ok_or_else(|| format!("committed corpus is missing category `{category}`"))
    }

    fn control_by_id<'a>(
        controls: &'a IssueLifecycleContractPlanControlCorpusV1,
        id: &str,
    ) -> Result<&'a IssueLifecycleContractPlanControlRowV1, String> {
        controls
            .rows
            .iter()
            .find(|control| control.id == id)
            .ok_or_else(|| format!("committed control corpus is missing control `{id}`"))
    }

    #[test]
    fn issue_lifecycle_contract_plan_pilot_committed_corpus_loads_with_two_categories()
    -> Result<(), String> {
        let (corpus, controls, provenance) = load_committed()?;
        if corpus.rows.len() != 2 {
            return Err(format!("expected two real rows, got {}", corpus.rows.len()));
        }
        if provenance.rows.len() != 2 {
            return Err(format!(
                "expected two provenance rows, got {}",
                provenance.rows.len()
            ));
        }
        for control in &controls.rows {
            if !control.row.attempt.synthetic {
                return Err(format!("control row `{}` must be synthetic", control.id));
            }
        }
        for row in &corpus.rows {
            if row.attempt.synthetic {
                return Err(format!("real row `{}` must not be synthetic", row.id));
            }
        }
        if !assess_contract_plan_corpus(&corpus).is_empty() {
            return Err("the committed real corpus must assess clean".to_string());
        }
        if !assess_contract_plan_control_corpus(&controls).is_empty() {
            return Err("the committed control corpus must assess clean".to_string());
        }
        Ok(())
    }

    #[test]
    fn issue_lifecycle_contract_plan_pilot_contract_case_runs_author_adversary_and_root()
    -> Result<(), String> {
        let (corpus, _controls, _provenance) = load_committed()?;
        let row = row_by_category(&corpus, "contract_required")?;
        if row.snapshot.issue_number != 6225 {
            return Err(format!(
                "the contract case must bind issue 6225, got {}",
                row.snapshot.issue_number
            ));
        }
        let contract = row
            .contract
            .as_ref()
            .ok_or_else(|| "the contract case must carry contract evidence".to_string())?;
        if contract.author.carries_acceptance {
            return Err("the author result must not carry an acceptance state".to_string());
        }
        if contract.author.result_identity == contract.adversary.result_identity {
            return Err("author and adversary results must be distinct identities".to_string());
        }
        if contract.adversary.inspected_scope.trim().is_empty() {
            return Err("the adversary must record its inspected scope".to_string());
        }
        if contract.adversary.findings.is_empty() {
            return Err(
                "the adversary must return concrete missing failure/limited findings for this case"
                    .to_string(),
            );
        }
        if contract.adversary.none_found {
            return Err("a findings-carrying adversary must not claim none_found".to_string());
        }
        if contract.open_decisions.is_empty() {
            return Err("the contract case must preserve its open decisions".to_string());
        }
        if contract.root_disposition != IssueLifecycleDispositionV1::QualifiedSpecRequired {
            return Err(format!(
                "the root disposition must stay qualified_spec_required, got {:?}",
                contract.root_disposition
            ));
        }
        if contract.contract_state != IssueLifecycleContractStateV1::Amended {
            return Err(format!(
                "the root amended the draft per the adversary findings, got {:?}",
                contract.contract_state
            ));
        }
        if row.attempt.contract_artifacts.acceptance.is_some() {
            return Err("no acceptance artifact may exist while open decisions remain".to_string());
        }
        if row.planning.acceptance_omitted.is_empty() {
            return Err("the contract plan must disclose its omitted acceptance rows".to_string());
        }
        let projection = build_contract_plan_projection(row);
        if projection.draft_spec_identity.as_deref() != Some(contract.draft_spec_identity.as_str())
        {
            return Err("the projection must retain the draft spec identity".to_string());
        }
        if projection.author_result_identity.as_deref() != Some("draft-result-6225-author-v1") {
            return Err("the projection must retain the author result identity".to_string());
        }
        if projection.adversary_result_identity.as_deref()
            != Some("challenge-result-6225-adversary-v1")
        {
            return Err("the projection must retain the adversary result identity".to_string());
        }
        if projection.adversary_inspected_scope.as_deref()
            != Some(contract.adversary.inspected_scope.as_str())
        {
            return Err("the projection must retain the adversary inspected scope".to_string());
        }
        Ok(())
    }

    #[test]
    fn issue_lifecycle_contract_plan_pilot_narrow_case_routes_directly_to_one_pr()
    -> Result<(), String> {
        let (corpus, _controls, _provenance) = load_committed()?;
        let row = row_by_category(&corpus, "narrow_accepted_contract_bug")?;
        if row.snapshot.issue_number != 6180 {
            return Err(format!(
                "the narrow case must bind issue 6180, got {}",
                row.snapshot.issue_number
            ));
        }
        if row.contract.is_some() {
            return Err("the narrow bug must carry no contract evidence".to_string());
        }
        if row.attempt.contract_decision.spec_required {
            return Err("the narrow bug must not require a spec".to_string());
        }
        let artifacts = &row.attempt.contract_artifacts;
        if artifacts.proposal.is_some()
            || artifacts.spec.is_some()
            || artifacts.adr.is_some()
            || artifacts.challenge.is_some()
            || !artifacts.amendments.is_empty()
            || artifacts.acceptance.is_some()
        {
            return Err("the narrow bug must carry no contract artifacts".to_string());
        }
        if row.attempt.disposition != IssueLifecycleDispositionV1::QualifiedOnePr {
            return Err(format!(
                "the narrow bug must qualify as one-PR work, got {:?}",
                row.attempt.disposition
            ));
        }
        if row.planning.shape_decision != IssueLifecyclePlanShapeV1::OnePr {
            return Err("the narrow bug must plan as one PR".to_string());
        }
        if !row.planning.acceptance_omitted.is_empty() {
            return Err("the narrow bug must omit no acceptance rows".to_string());
        }
        Ok(())
    }

    #[test]
    fn issue_lifecycle_contract_plan_pilot_adversary_findings_and_bounded_none_found_exclusive()
    -> Result<(), String> {
        let (_corpus, controls, _provenance) = load_committed()?;
        let caught = control_by_id(&controls, "control_adversary_catches_missing_failure_state")?;
        if caught.row.contract.is_none() {
            return Err("the catch control must carry contract evidence".to_string());
        }
        let contract = caught
            .row
            .contract
            .as_ref()
            .ok_or_else(|| "the catch control must carry contract evidence".to_string())?;
        if contract.adversary.findings.is_empty() || contract.adversary.none_found {
            return Err(
                "the catch control must carry concrete findings without none_found".to_string(),
            );
        }
        let none = control_by_id(&controls, "control_adversary_bounded_none_found")?;
        let none_contract = none
            .row
            .contract
            .as_ref()
            .ok_or_else(|| "the none-found control must carry contract evidence".to_string())?;
        if !none_contract.adversary.findings.is_empty() || !none_contract.adversary.none_found {
            return Err(
                "the none-found control must return bounded none_found with an empty finding list"
                    .to_string(),
            );
        }
        if none_contract.contract_state != IssueLifecycleContractStateV1::Accepted {
            return Err("a clean challenge supports an accepted root state".to_string());
        }
        Ok(())
    }

    #[test]
    fn issue_lifecycle_contract_plan_pilot_author_cannot_accept_own_contract() -> Result<(), String>
    {
        let (corpus, controls, _provenance) = load_committed()?;
        for row in corpus
            .rows
            .iter()
            .chain(controls.rows.iter().map(|control| &control.row))
        {
            if let Some(contract) = &row.contract {
                if contract.author.carries_acceptance {
                    return Err(format!(
                        "row `{}` author result carries an acceptance state",
                        row.id
                    ));
                }
                if contract.author.role != "contract_author" {
                    return Err(format!(
                        "row `{}` author role must be contract_author, got {}",
                        row.id, contract.author.role
                    ));
                }
            }
        }
        let mut row = row_by_category(&corpus, "contract_required")?.clone();
        let contract = row
            .contract
            .as_mut()
            .ok_or_else(|| "the contract case must carry contract evidence".to_string())?;
        contract.author.carries_acceptance = true;
        let failures = assess_contract_plan_row(&row);
        if !failures
            .iter()
            .any(|failure| failure.contains("cannot accept its own contract"))
        {
            return Err(format!(
                "an author-carrying-acceptance row must fail the law, got {failures:?}"
            ));
        }
        Ok(())
    }

    #[test]
    fn issue_lifecycle_contract_plan_pilot_unresolved_decision_blocks_implementation()
    -> Result<(), String> {
        let (corpus, _controls, _provenance) = load_committed()?;
        let row = row_by_category(&corpus, "contract_required")?;
        let contract = row
            .contract
            .as_ref()
            .ok_or_else(|| "the contract case must carry contract evidence".to_string())?;
        if contract.open_decisions.is_empty() {
            return Err("the committed contract row must preserve open decisions".to_string());
        }
        if matches!(
            row.attempt.disposition,
            IssueLifecycleDispositionV1::Completed | IssueLifecycleDispositionV1::QualifiedOnePr
        ) {
            return Err("open decisions must block implementation-ready dispositions".to_string());
        }
        if row.planning.stop_conditions.is_empty() {
            return Err("open decisions must bind stop conditions".to_string());
        }
        // A spec-required row qualified as one-PR work fails even with the
        // open decisions preserved: the disposition cap binds by category,
        // not by the open-decision gate alone.
        let mut qualified = row.clone();
        qualified.attempt.disposition = IssueLifecycleDispositionV1::QualifiedOnePr;
        let failures = assess_contract_plan_row(&qualified);
        if !failures
            .iter()
            .any(|failure| failure.contains("can never qualify as direct one-PR work"))
        {
            return Err(format!(
                "a spec-required row qualified as one-PR work must fail the law, got {failures:?}"
            ));
        }
        // Each open-decision guard must fire on its own mutation.
        let mut no_stop = row.clone();
        no_stop.planning.stop_conditions.clear();
        let failures = assess_contract_plan_row(&no_stop);
        if !failures
            .iter()
            .any(|failure| failure.contains("records no stop condition"))
        {
            return Err(format!(
                "a row preserving open decisions without stop conditions must fail the law, got {failures:?}"
            ));
        }
        let mut completed = row.clone();
        completed.attempt.disposition = IssueLifecycleDispositionV1::Completed;
        let failures = assess_contract_plan_row(&completed);
        if !failures
            .iter()
            .any(|failure| failure.contains("claims an implementation-ready disposition"))
        {
            return Err(format!(
                "a row preserving open decisions with a completed disposition must fail the law, got {failures:?}"
            ));
        }
        let mut acceptance = row.clone();
        acceptance.attempt.contract_artifacts.acceptance = Some("acceptance-x".to_string());
        let failures = assess_contract_plan_row(&acceptance);
        if !failures
            .iter()
            .any(|failure| failure.contains("acceptance cannot outrun the root disposition"))
        {
            return Err(format!(
                "a row preserving open decisions with acceptance evidence must fail the law, got {failures:?}"
            ));
        }
        Ok(())
    }

    #[test]
    fn issue_lifecycle_contract_plan_pilot_campaign_and_one_pr_shapes_rejected_when_incoherent()
    -> Result<(), String> {
        let (_corpus, controls, _provenance) = load_committed()?;
        let campaign_rejected =
            control_by_id(&controls, "control_campaign_rejected_when_slice_suffices")?;
        if campaign_rejected.row.planning.shape_decision != IssueLifecyclePlanShapeV1::OnePr {
            return Err(
                "the rejected-campaign control must land on one_pr with its rationale".to_string(),
            );
        }
        if !campaign_rejected
            .row
            .planning
            .shape_rationale
            .contains("rejected")
        {
            return Err(
                "the rejected-campaign control must record the rejection rationale".to_string(),
            );
        }
        let one_pr_rejected = control_by_id(&controls, "control_one_pr_rejected_when_incoherent")?;
        if one_pr_rejected.row.planning.shape_decision != IssueLifecyclePlanShapeV1::Campaign {
            return Err(
                "the rejected-one-PR control must land on campaign with its rationale".to_string(),
            );
        }
        if !one_pr_rejected
            .row
            .planning
            .shape_rationale
            .contains("rejected")
        {
            return Err(
                "the rejected-one-PR control must record the rejection rationale".to_string(),
            );
        }
        // The real contract row considered and rejected a campaign on the
        // record: its shape rationale must name the rejection.
        let (corpus, _controls, _provenance) = load_committed()?;
        let contract_row = row_by_category(&corpus, "contract_required")?;
        if contract_row.planning.shape_decision != IssueLifecyclePlanShapeV1::OnePr {
            return Err("the real contract row must land on one_pr".to_string());
        }
        if !contract_row.planning.shape_rationale.contains("rejected") {
            return Err("the real contract row must record the campaign rejection".to_string());
        }
        Ok(())
    }

    #[test]
    fn issue_lifecycle_contract_plan_pilot_specs_behavior_and_plans_work_order_laws()
    -> Result<(), String> {
        let (corpus, _controls, _provenance) = load_committed()?;
        let row = row_by_category(&corpus, "contract_required")?;
        let contract = row
            .contract
            .as_ref()
            .ok_or_else(|| "the contract case must carry contract evidence".to_string())?;
        if !contract.draft_spec_identity.starts_with("RIPR-SPEC-draft-") {
            return Err("the draft spec identity must be a behavior draft".to_string());
        }
        // Spec law: a queue-shaped draft identity fails closed.
        let mut mutated = row.clone();
        {
            let contract = mutated
                .contract
                .as_mut()
                .ok_or_else(|| "the contract case must carry contract evidence".to_string())?;
            contract.draft_spec_identity = "work-order-queue-1".to_string();
        }
        let failures = assess_contract_plan_row(&mutated);
        if !failures
            .iter()
            .any(|failure| failure.contains("specs contain behavior, not execution queues"))
        {
            return Err(format!(
                "a queue-shaped draft identity must fail the spec law, got {failures:?}"
            ));
        }
        // Plan law: a plan minting behavior authority fails closed. The
        // rationale must name the real behavior draft identity so the
        // planning-surface scan sees the minted authority.
        let mut mutated_plan = row.clone();
        mutated_plan.planning.shape_rationale = format!(
            "implements {} as the behavior authority",
            contract.draft_spec_identity
        );
        let failures = assess_contract_plan_row(&mutated_plan);
        if !failures
            .iter()
            .any(|failure| failure.contains("plans contain work order, not new behavior authority"))
        {
            return Err(format!(
                "a plan minting a draft spec identity must fail the plan law, got {failures:?}"
            ));
        }
        Ok(())
    }

    #[test]
    fn issue_lifecycle_contract_plan_pilot_no_tracked_selection_file_changed() -> Result<(), String>
    {
        let (corpus, controls, _provenance) = load_committed()?;
        for row in corpus
            .rows
            .iter()
            .chain(controls.rows.iter().map(|control| &control.row))
        {
            for string in planning_strings(&row.planning) {
                if string.contains("active.toml") || string.contains("selected_work") {
                    return Err(format!(
                        "row `{}` planning surface references a tracked selection file: {string}",
                        row.id
                    ));
                }
            }
        }
        let mut mutated = row_by_category(&corpus, "narrow_accepted_contract_bug")?.clone();
        mutated.planning.portfolio_placement = "writes active.toml to select this work".to_string();
        let failures = assess_contract_plan_row(&mutated);
        if !failures
            .iter()
            .any(|failure| failure.contains("tracked selection or current-work file"))
        {
            return Err(format!(
                "a selection-file mutation must fail the law, got {failures:?}"
            ));
        }
        Ok(())
    }

    #[test]
    fn issue_lifecycle_contract_plan_pilot_cold_start_root_reconstructs_decisions_without_chat()
    -> Result<(), String> {
        let (corpus, _controls, _provenance) = load_committed()?;
        let first: Vec<String> = corpus
            .rows
            .iter()
            .map(|row| {
                serde_json::to_string(&build_contract_plan_projection(row))
                    .map_err(|error| format!("serialize contract plan projection: {error}"))
            })
            .collect::<Result<_, _>>()?;
        let reloaded = load_contract_plan_corpus_dir(&contract_plan_root())?;
        let second: Vec<String> = reloaded
            .0
            .rows
            .iter()
            .map(|row| {
                serde_json::to_string(&build_contract_plan_projection(row))
                    .map_err(|error| format!("serialize contract plan projection: {error}"))
            })
            .collect::<Result<_, _>>()?;
        if first != second {
            return Err(
                "a cold-start reload from the committed corpus alone must reconstruct byte-identical contract/plan decisions"
                    .to_string(),
            );
        }
        Ok(())
    }

    #[test]
    fn issue_lifecycle_contract_plan_pilot_fabricated_retrieval_bytes_fail_closed()
    -> Result<(), String> {
        let (corpus, _controls, _provenance) = load_committed()?;
        let mut row = corpus.rows[0].clone();
        row.retrieval_steps[0].bytes = IssueLifecycleIntakeBytesV1::Measured(1);
        let error = verify_contract_plan_row_snapshot(&row, &contract_plan_root())
            .err()
            .ok_or_else(|| "a fabricated retrieval byte count must fail closed".to_string())?;
        if !error.contains("claims 1 bytes") {
            return Err(format!("unexpected verification error: {error}"));
        }
        Ok(())
    }

    #[test]
    fn issue_lifecycle_contract_plan_pilot_root_disposition_downgrade_rejected()
    -> Result<(), String> {
        let (mut corpus, _controls, _provenance) = load_committed()?;
        let lifecycle_id = {
            let row = corpus
                .rows
                .iter_mut()
                .find(|row| row.category == "contract_required")
                .ok_or_else(|| "missing contract-required row".to_string())?;
            row.attempt.disposition = IssueLifecycleDispositionV1::RootDecisionRequired;
            row.attempt.row_digest =
                crate::issue_lifecycle_attempt::issue_lifecycle_row_digest(&row.attempt)?;
            row.attempt.lifecycle_id.clone()
        };
        let (failures, assessed) = assess_real_rows_against_counting_law(&corpus);
        let assessment = assessed
            .iter()
            .find(|assessment| assessment.lifecycle_id == lifecycle_id)
            .ok_or_else(|| "downgraded row was not assessed".to_string())?;
        if !assessment.counted {
            return Err(
                "the downgrade keeps the row counted, so only the root-disposition disagreement may reject it"
                    .to_string(),
            );
        }
        if !failures
            .iter()
            .any(|failure| failure.contains("disagrees with the assessed disposition"))
        {
            return Err(format!(
                "a counted but downgraded row must be rejected, got failures {failures:?}"
            ));
        }
        Ok(())
    }

    #[test]
    fn issue_lifecycle_contract_plan_pilot_extra_provenance_row_rejected() -> Result<(), String> {
        let (corpus, _controls, mut provenance) = load_committed()?;
        check_provenance_against_corpus(&corpus, &provenance)?;
        provenance
            .rows
            .push(IssueLifecycleContractPlanProvenanceRowV1 {
                issue: 9_999_999,
                category: "narrow_accepted_contract_bug".to_string(),
                justification: "synthetic extra provenance entry".to_string(),
            });
        let error = check_provenance_against_corpus(&corpus, &provenance)
            .err()
            .ok_or_else(|| "an extra provenance row must fail closed".to_string())?;
        if !error.contains("must name 2 rows, got 3") {
            return Err(format!("unexpected provenance error: {error}"));
        }
        Ok(())
    }

    #[test]
    fn issue_lifecycle_contract_plan_pilot_empty_control_corpus_rejected() -> Result<(), String> {
        let body = format!(
            "{{\"schema_version\":\"{ISSUE_LIFECYCLE_CONTRACT_PLAN_CONTROL_CORPUS_SCHEMA_VERSION}\",\"rows\":[]}}"
        );
        let error = load_issue_lifecycle_contract_plan_control_corpus(&body)
            .err()
            .ok_or_else(|| "an empty control corpus must fail closed".to_string())?;
        if !error.contains("must not be empty") {
            return Err(format!("unexpected control corpus error: {error}"));
        }
        Ok(())
    }

    #[test]
    fn issue_lifecycle_contract_plan_pilot_missing_required_control_rejected() -> Result<(), String>
    {
        let path = contract_plan_root().join("controls.json");
        let body = fs::read_to_string(&path)
            .map_err(|error| format!("read committed controls: {error}"))?;
        let mut value: serde_json::Value = serde_json::from_str(&body)
            .map_err(|error| format!("parse committed controls: {error}"))?;
        let rows = value
            .get_mut("rows")
            .and_then(serde_json::Value::as_array_mut)
            .ok_or_else(|| "committed controls carry no rows array".to_string())?;
        rows.retain(|row| {
            row.get("control").and_then(serde_json::Value::as_str)
                != Some("cold_start_root_reconstructs_decisions_from_artifacts")
        });
        let trimmed = serde_json::to_string(&value)
            .map_err(|error| format!("serialize trimmed controls: {error}"))?;
        let error = load_issue_lifecycle_contract_plan_control_corpus(&trimmed)
            .err()
            .ok_or_else(|| {
                "a control corpus missing a required control must fail closed".to_string()
            })?;
        if !error.contains("missing required control") {
            return Err(format!("unexpected control corpus error: {error}"));
        }
        Ok(())
    }

    #[test]
    fn issue_lifecycle_contract_plan_pilot_author_and_adversary_results_must_be_distinct()
    -> Result<(), String> {
        let (corpus, _controls, _provenance) = load_committed()?;
        let mut row = row_by_category(&corpus, "contract_required")?.clone();
        let contract = row
            .contract
            .as_mut()
            .ok_or_else(|| "the contract case must carry contract evidence".to_string())?;
        contract.adversary.result_identity = contract.author.result_identity.clone();
        let failures = assess_contract_plan_row(&row);
        if !failures
            .iter()
            .any(|failure| failure.contains("must be distinct identities"))
        {
            return Err(format!(
                "a shared author/adversary result identity must fail the law, got {failures:?}"
            ));
        }
        Ok(())
    }

    #[test]
    fn issue_lifecycle_contract_plan_pilot_planning_drift_from_embedded_attempt_rejected()
    -> Result<(), String> {
        let (corpus, _controls, _provenance) = load_committed()?;
        let contract_row = row_by_category(&corpus, "contract_required")?;
        // Work-item identity drift.
        let mut drifted = contract_row.clone();
        drifted.planning.work_items[0].id = "wi-drifted".to_string();
        let failures = assess_contract_plan_row(&drifted);
        if !failures
            .iter()
            .any(|failure| failure.contains("planning work items drifted"))
        {
            return Err(format!(
                "a planning work-item drift must fail the law, got {failures:?}"
            ));
        }
        // Dependency-edge drift.
        let mut edge_drift = contract_row.clone();
        edge_drift.planning.work_items[1].depends_on.clear();
        let failures = assess_contract_plan_row(&edge_drift);
        if !failures
            .iter()
            .any(|failure| failure.contains("planning dependency edges drifted"))
        {
            return Err(format!(
                "a planning dependency-edge drift must fail the law, got {failures:?}"
            ));
        }
        // Acceptance-coverage drift.
        let mut coverage_drift = contract_row.clone();
        coverage_drift
            .planning
            .acceptance_covered
            .push("synthetic row absent from the attempt plan".to_string());
        let failures = assess_contract_plan_row(&coverage_drift);
        if !failures
            .iter()
            .any(|failure| failure.contains("planning acceptance coverage drifted"))
        {
            return Err(format!(
                "a planning acceptance-coverage drift must fail the law, got {failures:?}"
            ));
        }
        Ok(())
    }

    #[test]
    fn issue_lifecycle_contract_plan_pilot_provenance_captured_at_mismatch_rejected()
    -> Result<(), String> {
        let (corpus, _controls, mut provenance) = load_committed()?;
        provenance.captured_at = "2026-01-01T00:00:00Z".to_string();
        let error = check_provenance_against_corpus(&corpus, &provenance)
            .err()
            .ok_or_else(|| "a captured_at mismatch must fail closed".to_string())?;
        if !error.contains("captured at") {
            return Err(format!("unexpected provenance error: {error}"));
        }
        Ok(())
    }

    #[test]
    fn issue_lifecycle_contract_plan_pilot_root_result_identity_must_be_distinct_and_complete()
    -> Result<(), String> {
        let (corpus, _controls, _provenance) = load_committed()?;
        let mut aliased = row_by_category(&corpus, "contract_required")?.clone();
        let contract = aliased
            .contract
            .as_mut()
            .ok_or_else(|| "the contract case must carry contract evidence".to_string())?;
        contract.root.result_identity = contract.author.result_identity.clone();
        let failures = assess_contract_plan_row(&aliased);
        if !failures
            .iter()
            .any(|failure| failure.contains("pairwise distinct identities"))
        {
            return Err(format!(
                "a root result aliased to the author result must fail the law, got {failures:?}"
            ));
        }
        let mut emptied = row_by_category(&corpus, "contract_required")?.clone();
        let contract = emptied
            .contract
            .as_mut()
            .ok_or_else(|| "the contract case must carry contract evidence".to_string())?;
        contract.root.result_identity.clear();
        let failures = assess_contract_plan_row(&emptied);
        if !failures
            .iter()
            .any(|failure| failure.contains("root role/config/result identity must be complete"))
        {
            return Err(format!(
                "an emptied root result identity must fail the law, got {failures:?}"
            ));
        }
        let projection =
            build_contract_plan_projection(row_by_category(&corpus, "contract_required")?);
        if projection
            .root_result_identity
            .as_deref()
            .is_none_or(str::is_empty)
        {
            return Err("the projection must retain the root result identity".to_string());
        }
        Ok(())
    }

    #[test]
    fn issue_lifecycle_contract_plan_pilot_contract_state_bound_to_embedded_attempt()
    -> Result<(), String> {
        let (corpus, _controls, _provenance) = load_committed()?;
        let row = row_by_category(&corpus, "contract_required")?;
        // Mutating only the outer committed state must fail.
        let mut outer = row.clone();
        outer
            .contract
            .as_mut()
            .ok_or_else(|| "the contract case must carry contract evidence".to_string())?
            .contract_state = IssueLifecycleContractStateV1::Rejected;
        let failures = assess_contract_plan_row(&outer);
        if !failures.iter().any(|failure| {
            failure.contains("disagrees with the embedded attempt root contract state")
        }) {
            return Err(format!(
                "an outer contract state drift must fail the law, got {failures:?}"
            ));
        }
        // Drifting the frozen attempt state must fail too.
        let mut inner = row.clone();
        inner.attempt.contract_decision.root_disposition = Some("rejected".to_string());
        let failures = assess_contract_plan_row(&inner);
        if !failures.iter().any(|failure| {
            failure.contains("disagrees with the embedded attempt root contract state")
        }) {
            return Err(format!(
                "an embedded attempt state drift must fail the law, got {failures:?}"
            ));
        }
        // An unknown state string must fail closed, never parse silently.
        let mut unknown = row.clone();
        unknown.attempt.contract_decision.root_disposition = Some("spec-accepted".to_string());
        let failures = assess_contract_plan_row(&unknown);
        if !failures
            .iter()
            .any(|failure| failure.contains("unknown root contract state"))
        {
            return Err(format!(
                "an unknown embedded contract state must fail the law, got {failures:?}"
            ));
        }
        // A narrow bug must not record a root contract state at all.
        let mut narrow = row_by_category(&corpus, "narrow_accepted_contract_bug")?.clone();
        narrow.attempt.contract_decision.root_disposition = Some("accepted".to_string());
        let failures = assess_contract_plan_row(&narrow);
        if !failures
            .iter()
            .any(|failure| failure.contains("must not record a root contract state"))
        {
            return Err(format!(
                "a narrow bug recording a root contract state must fail the law, got {failures:?}"
            ));
        }
        Ok(())
    }

    #[test]
    fn issue_lifecycle_contract_plan_pilot_control_names_bound_to_named_behavior()
    -> Result<(), String> {
        let (_corpus, controls, _provenance) = load_committed()?;
        let narrow_clean = control_by_id(&controls, "control_narrow_bug_gets_no_spec")?.clone();
        let contract_clean =
            control_by_id(&controls, "control_author_cannot_accept_own_contract")?.clone();
        // Attaching every control name to copies of an unrelated narrow clean
        // row must fail for every name except the narrow-bug law itself.
        let mut faked = controls.clone();
        for control in faked.rows.iter_mut() {
            let mut replacement = narrow_clean.clone();
            replacement.id = control.id.clone();
            replacement.control = control.control.clone();
            replacement.scenario = control.scenario.clone();
            *control = replacement;
        }
        let failures = assess_contract_plan_control_corpus(&faked);
        for control in &controls.rows {
            if control.control == "narrow_bug_gets_no_unnecessary_spec" {
                continue;
            }
            if !failures.iter().any(|failure| failure.contains(&control.id)) {
                return Err(format!(
                    "faking control `{}` with an unrelated clean row must fail, got {failures:?}",
                    control.id
                ));
            }
        }
        // The narrow-bug law itself rejects an unrelated contract-shaped row.
        let mut faked = controls.clone();
        for control in faked.rows.iter_mut() {
            if control.control == "narrow_bug_gets_no_unnecessary_spec" {
                let mut replacement = contract_clean.clone();
                replacement.id = control.id.clone();
                replacement.control = control.control.clone();
                replacement.scenario = control.scenario.clone();
                *control = replacement;
            }
        }
        let failures = assess_contract_plan_control_corpus(&faked);
        if !failures
            .iter()
            .any(|failure| failure.contains("control_narrow_bug_gets_no_spec"))
        {
            return Err(format!(
                "faking the narrow-bug control with a contract row must fail, got {failures:?}"
            ));
        }
        // A corpus that only exercises the findings branch of the adversary
        // control must fail the both-branches requirement.
        let mut findings_only = controls.clone();
        findings_only
            .rows
            .retain(|control| control.id != "control_adversary_bounded_none_found");
        let failures = assess_contract_plan_control_corpus(&findings_only);
        if !failures
            .iter()
            .any(|failure| failure.contains("bounded none_found branches"))
        {
            return Err(format!(
                "dropping the bounded none_found branch must fail, got {failures:?}"
            ));
        }
        Ok(())
    }

    #[test]
    fn issue_lifecycle_contract_plan_pilot_unmeasured_or_missing_retrieval_step_fails_closed()
    -> Result<(), String> {
        let (corpus, _controls, _provenance) = load_committed()?;
        let mut not_measured = corpus.rows[0].clone();
        not_measured.retrieval_steps[0].bytes = IssueLifecycleIntakeBytesV1::NotMeasured;
        let error = verify_contract_plan_row_snapshot(&not_measured, &contract_plan_root())
            .err()
            .ok_or_else(|| "a NotMeasured retrieval step must fail closed".to_string())?;
        if !error.contains("is not measured") {
            return Err(format!("unexpected verification error: {error}"));
        }
        let mut missing = corpus.rows[0].clone();
        missing.retrieval_steps.remove(0);
        let error = verify_contract_plan_row_snapshot(&missing, &contract_plan_root())
            .err()
            .ok_or_else(|| "a missing retrieval step must fail closed".to_string())?;
        if !error.contains("records no retrieval step") {
            return Err(format!("unexpected verification error: {error}"));
        }
        Ok(())
    }

    #[test]
    fn issue_lifecycle_contract_plan_pilot_row_currentness_bound_to_capture_provenance()
    -> Result<(), String> {
        let (corpus, _controls, provenance) = load_committed()?;
        check_provenance_against_corpus(&corpus, &provenance)?;
        let drifted_main = "f1d2c3b4a5968778695a4b3c2d1e0f9a8b7c6d5e";
        let mut drifted = corpus.clone();
        drifted.rows[0].current_main = drifted_main.to_string();
        let error = check_provenance_against_corpus(&drifted, &provenance)
            .err()
            .ok_or_else(|| "a drifted row current main must fail closed".to_string())?;
        if !error.contains("current main") {
            return Err(format!("unexpected provenance error: {error}"));
        }
        let mut drifted = corpus.clone();
        drifted.rows[0].snapshot.captured_at = "2026-01-01T00:00:00Z".to_string();
        let error = check_provenance_against_corpus(&drifted, &provenance)
            .err()
            .ok_or_else(|| "a drifted row captured_at must fail closed".to_string())?;
        if !error.contains("captured at") {
            return Err(format!("unexpected provenance error: {error}"));
        }
        let mut drifted = corpus.clone();
        drifted.rows[0].attempt.context.current_main = drifted_main.to_string();
        let error = check_provenance_against_corpus(&drifted, &provenance)
            .err()
            .ok_or_else(|| "a drifted attempt context main must fail closed".to_string())?;
        if !error.contains("context main") {
            return Err(format!("unexpected provenance error: {error}"));
        }
        let mut drifted = corpus.clone();
        drifted.rows[0].attempt.execution_refs.current_main = drifted_main.to_string();
        let error = check_provenance_against_corpus(&drifted, &provenance)
            .err()
            .ok_or_else(|| "a drifted attempt execution main must fail closed".to_string())?;
        if !error.contains("execution main") {
            return Err(format!("unexpected provenance error: {error}"));
        }
        Ok(())
    }

    #[test]
    fn issue_lifecycle_contract_plan_pilot_embedded_attempt_bound_to_verified_snapshot()
    -> Result<(), String> {
        let (corpus, _controls, _provenance) = load_committed()?;
        let contract_row = row_by_category(&corpus, "contract_required")?;
        let narrow_row = row_by_category(&corpus, "narrow_accepted_contract_bug")?;
        let mut swapped = contract_row.clone();
        swapped.attempt.issue = narrow_row.attempt.issue.clone();
        swapped.attempt.row_digest =
            crate::issue_lifecycle_attempt::issue_lifecycle_row_digest(&swapped.attempt)?;
        let error = verify_contract_plan_row_snapshot(&swapped, &contract_plan_root())
            .err()
            .ok_or_else(|| {
                "an embedded attempt bound to another issue must fail closed".to_string()
            })?;
        if !error.contains("names issue") {
            return Err(format!("unexpected verification error: {error}"));
        }
        Ok(())
    }

    #[test]
    fn issue_lifecycle_contract_plan_pilot_open_decisions_block_post_implementation_states()
    -> Result<(), String> {
        let (corpus, _controls, _provenance) = load_committed()?;
        let row = row_by_category(&corpus, "contract_required")?;
        for disposition in [
            IssueLifecycleDispositionV1::PartiallyLanded,
            IssueLifecycleDispositionV1::VerificationFailed,
            IssueLifecycleDispositionV1::MergedPendingCloseout,
            IssueLifecycleDispositionV1::Completed,
        ] {
            let mut mutated = row.clone();
            mutated.attempt.disposition = disposition;
            let failures = assess_contract_plan_row(&mutated);
            if !failures
                .iter()
                .any(|failure| failure.contains("claims an implementation-ready disposition"))
            {
                return Err(format!(
                    "open decisions with disposition {disposition:?} must fail the law, got {failures:?}"
                ));
            }
        }
        Ok(())
    }

    #[test]
    fn issue_lifecycle_contract_plan_pilot_emptied_plan_sections_fail_closed() -> Result<(), String>
    {
        let (corpus, _controls, _provenance) = load_committed()?;
        let row = row_by_category(&corpus, "narrow_accepted_contract_bug")?.clone();
        let sections = [
            "work items",
            "edit cages",
            "semantic conflict resources",
            "proof commands",
            "stop conditions",
            "non-goals",
            "acceptance",
            "shape rationale",
            "portfolio placement",
        ];
        let expected = |section: &str| match section {
            "acceptance" => "covers no acceptance row".to_string(),
            "shape rationale" => "records no shape rationale".to_string(),
            "portfolio placement" => "records no portfolio placement".to_string(),
            other => format!("clears its mandatory {other}"),
        };
        for section in sections {
            let mut mutated = row.clone();
            match section {
                "work items" => mutated.planning.work_items.clear(),
                "edit cages" => mutated.planning.edit_cages.clear(),
                "semantic conflict resources" => mutated.planning.conflict_resources.clear(),
                "proof commands" => mutated.planning.proof_commands.clear(),
                "stop conditions" => mutated.planning.stop_conditions.clear(),
                "non-goals" => mutated.planning.non_goals.clear(),
                "acceptance" => mutated.planning.acceptance_covered.clear(),
                "shape rationale" => mutated.planning.shape_rationale.clear(),
                "portfolio placement" => mutated.planning.portfolio_placement.clear(),
                other => return Err(format!("unknown plan section `{other}`")),
            }
            let failures = assess_contract_plan_row(&mutated);
            let needle = expected(section);
            if !failures.iter().any(|failure| failure.contains(&needle)) {
                return Err(format!(
                    "emptying the plan {section} must fail the law, got {failures:?}"
                ));
            }
        }
        Ok(())
    }

    #[test]
    fn issue_lifecycle_contract_plan_pilot_duplicate_observation_key_rejected() -> Result<(), String>
    {
        let path = contract_plan_root().join("corpus.json");
        let body =
            fs::read_to_string(&path).map_err(|error| format!("read committed corpus: {error}"))?;
        let mut value: serde_json::Value = serde_json::from_str(&body)
            .map_err(|error| format!("parse committed corpus: {error}"))?;
        let rows = value
            .get_mut("rows")
            .and_then(serde_json::Value::as_array_mut)
            .ok_or_else(|| "committed corpus carries no rows array".to_string())?;
        if rows.len() < 2 {
            return Err("the committed corpus must carry two rows".to_string());
        }
        let alias = rows[0]
            .pointer("/attempt/observation_key")
            .cloned()
            .ok_or_else(|| "the first row carries no observation key".to_string())?;
        let attempt_slot = rows[1]
            .pointer_mut("/attempt/observation_key")
            .ok_or_else(|| "the second row carries no observation key".to_string())?;
        *attempt_slot = alias;
        // Recompute the aliased row digest so only the load-time shape law can
        // reject the alias.
        let mut attempt: IssueLifecycleAttemptV1 = serde_json::from_value(
            rows[1]
                .get("attempt")
                .cloned()
                .ok_or_else(|| "the second row carries no attempt".to_string())?,
        )
        .map_err(|error| format!("parse the aliased attempt: {error}"))?;
        attempt.row_digest = crate::issue_lifecycle_attempt::issue_lifecycle_row_digest(&attempt)?;
        rows[1]["attempt"] = serde_json::to_value(&attempt)
            .map_err(|error| format!("serialize the aliased attempt: {error}"))?;
        let mutated = serde_json::to_string(&value)
            .map_err(|error| format!("serialize the aliased corpus: {error}"))?;
        let error = load_issue_lifecycle_contract_plan_corpus(&mutated)
            .err()
            .ok_or_else(|| "an aliased observation key must fail closed".to_string())?;
        if !error.contains("duplicates observation key") {
            return Err(format!("unexpected corpus error: {error}"));
        }
        Ok(())
    }

    #[test]
    fn issue_lifecycle_contract_plan_pilot_real_row_timeline_snapshot_required()
    -> Result<(), String> {
        let (corpus, _controls, _provenance) = load_committed()?;
        let mut no_path = corpus.rows[0].clone();
        no_path.snapshot.timeline_path = None;
        let error = verify_contract_plan_row_snapshot(&no_path, &contract_plan_root())
            .err()
            .ok_or_else(|| "a real row without a timeline path must fail closed".to_string())?;
        if !error.contains("no timeline path") {
            return Err(format!("unexpected verification error: {error}"));
        }
        let mut no_digest = corpus.rows[0].clone();
        no_digest.snapshot.timeline_snapshot_id = None;
        let error = verify_contract_plan_row_snapshot(&no_digest, &contract_plan_root())
            .err()
            .ok_or_else(|| "a real row without a timeline digest must fail closed".to_string())?;
        if !error.contains("records no digest") {
            return Err(format!("unexpected verification error: {error}"));
        }
        Ok(())
    }

    #[test]
    fn issue_lifecycle_contract_plan_pilot_snapshot_bytes_bound_to_named_issue()
    -> Result<(), String> {
        let (corpus, _controls, _provenance) = load_committed()?;
        let row_6225 = corpus
            .rows
            .iter()
            .find(|row| row.snapshot.issue_number == 6225)
            .ok_or_else(|| "missing the issue 6225 row".to_string())?;
        // Point the 6225 row at 6180's committed bytes, update the recorded
        // digest and mirror it into the embedded attempt, so only the payload
        // binding can reject the swap.
        let mut swapped = row_6225.clone();
        swapped.snapshot.snapshot_path = Some("snapshots/issue-6180.json".to_string());
        let bytes = fs::read(contract_plan_root().join("snapshots/issue-6180.json"))
            .map_err(|error| format!("read the swapped snapshot: {error}"))?;
        swapped.snapshot.issue_snapshot_id = format!(
            "gh-issue-snapshot:sha256:{}",
            crate::blind_journey::sha256_hex(&bytes)
        );
        swapped.attempt.issue.snapshot_id = swapped.snapshot.issue_snapshot_id.clone();
        let error = verify_contract_plan_row_snapshot(&swapped, &contract_plan_root())
            .err()
            .ok_or_else(|| "snapshot bytes from another issue must fail closed".to_string())?;
        if !error.contains("names issue 6225 but its snapshot payload is issue 6180") {
            return Err(format!("unexpected verification error: {error}"));
        }
        Ok(())
    }

    #[test]
    fn issue_lifecycle_contract_plan_pilot_contract_authority_and_rationale_enforced()
    -> Result<(), String> {
        let (corpus, _controls, _provenance) = load_committed()?;
        let row = row_by_category(&corpus, "contract_required")?;
        let contract = row
            .contract
            .as_ref()
            .ok_or_else(|| "the contract case must carry contract evidence".to_string())?;
        if contract.source_of_truth.trim().is_empty() {
            return Err("the committed contract must retain its source of truth".to_string());
        }
        let projection = build_contract_plan_projection(row);
        if projection.source_of_truth.as_deref() != Some(contract.source_of_truth.as_str()) {
            return Err("the projection must retain the source-of-truth identity".to_string());
        }
        if projection.spec_required_rationale.as_deref()
            != Some(contract.spec_required_rationale.as_str())
        {
            return Err("the projection must retain the spec-required rationale".to_string());
        }
        let mut cleared = row.clone();
        cleared
            .contract
            .as_mut()
            .ok_or_else(|| "the contract case must carry contract evidence".to_string())?
            .source_of_truth
            .clear();
        let failures = assess_contract_plan_row(&cleared);
        if !failures
            .iter()
            .any(|failure| failure.contains("records no source-of-truth identity"))
        {
            return Err(format!(
                "a cleared source of truth must fail the law, got {failures:?}"
            ));
        }
        let mut drifted = row.clone();
        drifted
            .contract
            .as_mut()
            .ok_or_else(|| "the contract case must carry contract evidence".to_string())?
            .spec_required_rationale
            .clear();
        let failures = assess_contract_plan_row(&drifted);
        if !failures
            .iter()
            .any(|failure| failure.contains("records no spec-required rationale"))
        {
            return Err(format!(
                "a cleared spec-required rationale must fail the law, got {failures:?}"
            ));
        }
        let mut no_decision_rationale = row.clone();
        no_decision_rationale
            .attempt
            .contract_decision
            .decision_rationale
            .clear();
        let failures = assess_contract_plan_row(&no_decision_rationale);
        if !failures
            .iter()
            .any(|failure| failure.contains("records no contract decision rationale"))
        {
            return Err(format!(
                "an emptied embedded decision rationale must fail the law, got {failures:?}"
            ));
        }
        // Equality law: drifting either side to unrelated nonempty text must
        // fail; self-consistency with blank checks alone is not a binding.
        let mut outer_drift = row.clone();
        outer_drift
            .contract
            .as_mut()
            .ok_or_else(|| "the contract case must carry contract evidence".to_string())?
            .spec_required_rationale = "unrelated text".to_string();
        let failures = assess_contract_plan_row(&outer_drift);
        if !failures
            .iter()
            .any(|failure| failure.contains("spec-required rationale drifted"))
        {
            return Err(format!(
                "an outer rationale drift must fail the law, got {failures:?}"
            ));
        }
        let mut inner_drift = row.clone();
        inner_drift.attempt.contract_decision.decision_rationale = "unrelated text".to_string();
        let failures = assess_contract_plan_row(&inner_drift);
        if !failures
            .iter()
            .any(|failure| failure.contains("spec-required rationale drifted"))
        {
            return Err(format!(
                "an embedded rationale drift must fail the law, got {failures:?}"
            ));
        }
        Ok(())
    }

    #[test]
    fn issue_lifecycle_contract_plan_pilot_issue_ref_suffix_bound_to_issue_number()
    -> Result<(), String> {
        let (corpus, _controls, _provenance) = load_committed()?;
        let row = corpus
            .rows
            .iter()
            .find(|row| row.snapshot.issue_number == 6225)
            .ok_or_else(|| "missing the issue 6225 row".to_string())?;
        // The embedded attempt mirrors the same incorrect ref so the payload
        // binding, not the identity equality, must reject the drift.
        let mut swapped = row.clone();
        swapped.snapshot.issue_ref = "EffortlessMetrics/ripr-swarm#6180".to_string();
        swapped.attempt.issue.issue_ref = "EffortlessMetrics/ripr-swarm#6180".to_string();
        let error = verify_contract_plan_row_snapshot(&swapped, &contract_plan_root())
            .err()
            .ok_or_else(|| "a mismatched issue suffix must fail closed".to_string())?;
        if !error.contains("names issue 6180 but the row authenticates issue 6225") {
            return Err(format!("unexpected verification error: {error}"));
        }
        let mut malformed = row.clone();
        malformed.snapshot.issue_ref = "EffortlessMetrics/ripr-swarm#abc".to_string();
        malformed.attempt.issue.issue_ref = "EffortlessMetrics/ripr-swarm#abc".to_string();
        let error = verify_contract_plan_row_snapshot(&malformed, &contract_plan_root())
            .err()
            .ok_or_else(|| "a non-numeric issue suffix must fail closed".to_string())?;
        if !error.contains("non-numeric issue suffix") {
            return Err(format!("unexpected verification error: {error}"));
        }
        Ok(())
    }

    #[test]
    fn issue_lifecycle_contract_plan_pilot_projection_preserves_dependency_free_work_items()
    -> Result<(), String> {
        let (corpus, _controls, _provenance) = load_committed()?;
        let row = row_by_category(&corpus, "contract_required")?;
        let mut standalone = row.clone();
        standalone
            .planning
            .work_items
            .push(IssueLifecyclePlanWorkItemV1 {
                id: "wi-standalone-projection-check".to_string(),
                depends_on: Vec::new(),
            });
        standalone
            .attempt
            .plan
            .work_items
            .push("wi-standalone-projection-check".to_string());
        let projection = build_contract_plan_projection(&standalone);
        if !projection
            .work_item_nodes
            .iter()
            .any(|node| node == "wi-standalone-projection-check")
        {
            return Err(
                "a dependency-free work item must survive in the projected nodes".to_string(),
            );
        }
        if projection
            .work_item_edges
            .iter()
            .any(|edge| edge.starts_with("wi-standalone-projection-check->"))
        {
            return Err("a dependency-free work item must not emit a dependency edge".to_string());
        }
        Ok(())
    }

    #[test]
    fn issue_lifecycle_contract_plan_pilot_timeline_payload_binds_row_repository()
    -> Result<(), String> {
        let (corpus, _controls, _provenance) = load_committed()?;
        let row = corpus
            .rows
            .iter()
            .find(|row| row.snapshot.issue_number == 6225)
            .ok_or_else(|| "missing the issue 6225 row".to_string())?;
        let committed = fs::read(contract_plan_root().join("snapshots/issue-6225-timeline.json"))
            .map_err(|error| format!("read the committed timeline: {error}"))?;
        verify_timeline_payload_binding(row, &committed)
            .map_err(|error| format!("the committed timeline must verify: {error}"))?;
        let foreign = br#"[{"event":"cross-referenced","source":{"issue":{"number":6227,"url":"https://api.github.com/repos/Other/Repo/issues/6227","repository":{"full_name":"Other/Repo"}}}}]"#;
        let error = verify_timeline_payload_binding(row, foreign)
            .err()
            .ok_or_else(|| "a foreign-repository timeline must fail closed".to_string())?;
        if !error.contains("names repository `Other/Repo`") {
            return Err(format!("unexpected timeline error: {error}"));
        }
        let disagreeing_url = br#"[{"event":"cross-referenced","source":{"issue":{"number":6227,"url":"https://api.github.com/repos/EffortlessMetrics/ripr-swarm/issues/9999","repository":{"full_name":"EffortlessMetrics/ripr-swarm"}}}}]"#;
        let error = verify_timeline_payload_binding(row, disagreeing_url)
            .err()
            .ok_or_else(|| "a number/url disagreement must fail closed".to_string())?;
        if !error.contains("does not name its own issue number") {
            return Err(format!("unexpected timeline error: {error}"));
        }
        let empty = b"[]";
        let error = verify_timeline_payload_binding(row, empty)
            .err()
            .ok_or_else(|| "an empty timeline must fail closed".to_string())?;
        if !error.contains("records no events") {
            return Err(format!("unexpected timeline error: {error}"));
        }
        let unnamed = br#"[{"source":{}}]"#;
        let error = verify_timeline_payload_binding(row, unnamed)
            .err()
            .ok_or_else(|| "an unnamed event must fail closed".to_string())?;
        if !error.contains("records no event name") {
            return Err(format!("unexpected timeline error: {error}"));
        }
        let blank_name = br#"[{"event":"   "}]"#;
        let error = verify_timeline_payload_binding(row, blank_name)
            .err()
            .ok_or_else(|| "a blank event name must fail closed".to_string())?;
        if !error.contains("blank event name") {
            return Err(format!("unexpected timeline error: {error}"));
        }
        Ok(())
    }

    #[test]
    fn issue_lifecycle_contract_plan_pilot_stop_conditions_cover_every_open_decision()
    -> Result<(), String> {
        let (corpus, _controls, _provenance) = load_committed()?;
        let row = row_by_category(&corpus, "contract_required")?;
        let decisions = row
            .contract
            .as_ref()
            .ok_or_else(|| "the contract case must carry contract evidence".to_string())?
            .open_decisions
            .len();
        let mut short = row.clone();
        short.planning.stop_conditions = vec!["unrelated stop".to_string()];
        let failures = assess_contract_plan_row(&short);
        if !failures
            .iter()
            .any(|failure| failure.contains("records only 1 stop conditions"))
        {
            return Err(format!(
                "fewer stop conditions than open decisions must fail the law, got {failures:?}"
            ));
        }
        let mut blank = row.clone();
        blank.planning.stop_conditions = vec!["x".to_string(); decisions];
        blank.planning.stop_conditions[0] = "   ".to_string();
        let failures = assess_contract_plan_row(&blank);
        if !failures
            .iter()
            .any(|failure| failure.contains("blank stop condition"))
        {
            return Err(format!(
                "a blank stop condition must fail the law, got {failures:?}"
            ));
        }
        let mut blank_decision = row.clone();
        blank_decision
            .contract
            .as_mut()
            .ok_or_else(|| "the contract case must carry contract evidence".to_string())?
            .open_decisions[0] = "  ".to_string();
        let failures = assess_contract_plan_row(&blank_decision);
        if !failures
            .iter()
            .any(|failure| failure.contains("blank open decision"))
        {
            return Err(format!(
                "a blank open decision must fail the law, got {failures:?}"
            ));
        }
        Ok(())
    }

    #[test]
    fn issue_lifecycle_contract_plan_pilot_provenance_justification_and_repository_enforced()
    -> Result<(), String> {
        let (corpus, _controls, mut provenance) = load_committed()?;
        check_provenance_against_corpus(&corpus, &provenance)?;
        provenance.rows[0].justification = "   ".to_string();
        let error = check_provenance_against_corpus(&corpus, &provenance)
            .err()
            .ok_or_else(|| "a blank justification must fail closed".to_string())?;
        if !error.contains("records no justification") {
            return Err(format!("unexpected provenance error: {error}"));
        }
        let (corpus, _controls, mut provenance) = load_committed()?;
        provenance.repository = "other/repo".to_string();
        let error = check_provenance_against_corpus(&corpus, &provenance)
            .err()
            .ok_or_else(|| "a drifted provenance repository must fail closed".to_string())?;
        if !error.contains("names repository `EffortlessMetrics/ripr-swarm`") {
            return Err(format!("unexpected provenance error: {error}"));
        }
        Ok(())
    }

    #[test]
    fn issue_lifecycle_contract_plan_pilot_real_contract_row_retains_open_decisions()
    -> Result<(), String> {
        let (mut corpus, _controls, _provenance) = load_committed()?;
        let row = corpus
            .rows
            .iter_mut()
            .find(|row| row.category == "contract_required")
            .ok_or_else(|| "missing contract-required row".to_string())?;
        row.contract
            .as_mut()
            .ok_or_else(|| "the contract case must carry contract evidence".to_string())?
            .open_decisions
            .clear();
        let failures = assess_contract_plan_corpus(&corpus);
        if !failures
            .iter()
            .any(|failure| failure.contains("retains no open decisions"))
        {
            return Err(format!(
                "a real contract row without open decisions must fail the law, got {failures:?}"
            ));
        }
        Ok(())
    }

    #[test]
    fn issue_lifecycle_contract_plan_pilot_blank_plan_entries_fail_closed() -> Result<(), String> {
        let (corpus, _controls, _provenance) = load_committed()?;
        let row = row_by_category(&corpus, "narrow_accepted_contract_bug")?;
        let blank_cases = [
            ("edit cages", "edit_cages"),
            ("semantic conflict resources", "conflict_resources"),
            ("stop conditions", "stop_conditions"),
            ("non-goals", "non_goals"),
        ];
        for (section, field) in blank_cases {
            let mut mutated = row.clone();
            match field {
                "edit_cages" => mutated.planning.edit_cages = vec!["   ".to_string()],
                "conflict_resources" => {
                    mutated.planning.conflict_resources = vec![String::new()];
                }
                "stop_conditions" => mutated.planning.stop_conditions = vec![" ".to_string()],
                "non_goals" => mutated.planning.non_goals = vec!["  ".to_string()],
                other => return Err(format!("unknown plan section `{other}`")),
            }
            let failures = assess_contract_plan_row(&mutated);
            if !failures
                .iter()
                .any(|failure| failure.contains(&format!("blank {section} entry")))
            {
                return Err(format!(
                    "a blank {section} entry must fail the law, got {failures:?}"
                ));
            }
        }
        Ok(())
    }

    #[test]
    fn issue_lifecycle_contract_plan_pilot_duplicate_work_item_ids_fail_closed()
    -> Result<(), String> {
        let (corpus, _controls, _provenance) = load_committed()?;
        let row = row_by_category(&corpus, "contract_required")?;
        let mut duplicated = row.clone();
        let alias = duplicated.planning.work_items[0].id.clone();
        duplicated.planning.work_items[1].id = alias;
        let failures = assess_contract_plan_row(&duplicated);
        if !failures
            .iter()
            .any(|failure| failure.contains("duplicates work item id"))
        {
            return Err(format!(
                "duplicated work item ids must fail the law, got {failures:?}"
            ));
        }
        let mut blank = row.clone();
        blank.planning.work_items[0].id = "  ".to_string();
        let failures = assess_contract_plan_row(&blank);
        if !failures
            .iter()
            .any(|failure| failure.contains("blank work item id"))
        {
            return Err(format!(
                "a blank work item id must fail the law, got {failures:?}"
            ));
        }
        Ok(())
    }

    #[test]
    fn issue_lifecycle_contract_plan_pilot_control_assessed_disposition_bound_to_root()
    -> Result<(), String> {
        let (_corpus, mut controls, _provenance) = load_committed()?;
        let control = controls
            .rows
            .iter_mut()
            .find(|control| control.id == "control_author_cannot_accept_own_contract")
            .ok_or_else(|| "missing the author control".to_string())?;
        // An open intake question caps the assessed disposition at
        // needs_evidence while the counting law keeps the row counted.
        control
            .row
            .attempt
            .intake
            .missing_evidence_questions
            .push("what evidence separates this control?".to_string());
        control.row.attempt.row_digest =
            crate::issue_lifecycle_attempt::issue_lifecycle_row_digest(&control.row.attempt)?;
        let failures = assess_control_rows_against_counting_law(&controls);
        if !failures
            .iter()
            .any(|failure| failure.contains("disagrees with its root disposition"))
        {
            return Err(format!(
                "a counted but downgraded control must fail the law, got {failures:?}"
            ));
        }
        Ok(())
    }

    #[test]
    fn issue_lifecycle_contract_plan_pilot_outer_row_id_bound_to_lifecycle_id() -> Result<(), String>
    {
        let (corpus, _controls, _provenance) = load_committed()?;
        let mut drifted = row_by_category(&corpus, "contract_required")?.clone();
        drifted.id = "drifted-outer-id".to_string();
        let failures = assess_contract_plan_row(&drifted);
        if !failures
            .iter()
            .any(|failure| failure.contains("outer id disagrees with the embedded lifecycle id"))
        {
            return Err(format!(
                "an outer id drift must fail the law, got {failures:?}"
            ));
        }
        Ok(())
    }

    #[test]
    fn issue_lifecycle_contract_plan_pilot_contract_identities_bound_to_embedded_artifacts()
    -> Result<(), String> {
        let (corpus, _controls, _provenance) = load_committed()?;
        let row = row_by_category(&corpus, "contract_required")?;
        let mut draft_drift = row.clone();
        draft_drift
            .contract
            .as_mut()
            .ok_or_else(|| "the contract case must carry contract evidence".to_string())?
            .draft_spec_identity = "RIPR-SPEC-draft-unrelated-drift-v9".to_string();
        let failures = assess_contract_plan_row(&draft_drift);
        if !failures
            .iter()
            .any(|failure| failure.contains("drifted from the embedded proposal identity"))
        {
            return Err(format!(
                "a draft identity drift must fail the law, got {failures:?}"
            ));
        }
        let mut challenge_drift = row.clone();
        challenge_drift
            .contract
            .as_mut()
            .ok_or_else(|| "the contract case must carry contract evidence".to_string())?
            .adversary
            .result_identity = "challenge-result-unrelated-drift".to_string();
        let failures = assess_contract_plan_row(&challenge_drift);
        if !failures
            .iter()
            .any(|failure| failure.contains("drifted from the embedded challenge identity"))
        {
            return Err(format!(
                "an adversary result drift must fail the law, got {failures:?}"
            ));
        }
        Ok(())
    }

    #[test]
    fn issue_lifecycle_contract_plan_pilot_claim_boundaries_bound_to_embedded_attempt()
    -> Result<(), String> {
        let (corpus, _controls, _provenance) = load_committed()?;
        let row = row_by_category(&corpus, "contract_required")?;
        let projection = build_contract_plan_projection(row);
        if projection.non_claims != row.non_claims {
            return Err("the projection must retain the row non_claims".to_string());
        }
        let mut cleared_limitations = row.clone();
        cleared_limitations.limitations.clear();
        let failures = assess_contract_plan_row(&cleared_limitations);
        if !failures
            .iter()
            .any(|failure| failure.contains("limitations drifted from the embedded attempt"))
        {
            return Err(format!(
                "cleared outer limitations must fail the law, got {failures:?}"
            ));
        }
        let mut drifted_non_claims = row.clone();
        drifted_non_claims.non_claims = vec!["unrelated boundary".to_string()];
        let failures = assess_contract_plan_row(&drifted_non_claims);
        if !failures
            .iter()
            .any(|failure| failure.contains("non_claims drifted from the embedded attempt"))
        {
            return Err(format!(
                "drifted outer non_claims must fail the law, got {failures:?}"
            ));
        }
        Ok(())
    }

    #[test]
    fn issue_lifecycle_contract_plan_pilot_cyclic_work_item_dependencies_fail_closed()
    -> Result<(), String> {
        let (corpus, _controls, _provenance) = load_committed()?;
        let row = row_by_category(&corpus, "contract_required")?;
        let mut self_edge = row.clone();
        let self_id = self_edge.planning.work_items[0].id.clone();
        self_edge.planning.work_items[0].depends_on = vec![self_id];
        let failures = assess_contract_plan_row(&self_edge);
        if !failures
            .iter()
            .any(|failure| failure.contains("work item dependencies form a cycle"))
        {
            return Err(format!("a self-edge must fail the law, got {failures:?}"));
        }
        let mut two_cycle = row.clone();
        let first = two_cycle.planning.work_items[0].id.clone();
        let second = two_cycle.planning.work_items[1].id.clone();
        two_cycle.planning.work_items[0].depends_on = vec![second.clone()];
        two_cycle.planning.work_items[1].depends_on = vec![first];
        let failures = assess_contract_plan_row(&two_cycle);
        if !failures
            .iter()
            .any(|failure| failure.contains("work item dependencies form a cycle"))
        {
            return Err(format!(
                "a two-item cycle must fail the law, got {failures:?}"
            ));
        }
        Ok(())
    }

    #[test]
    fn issue_lifecycle_contract_plan_pilot_amended_contract_retains_correction_evidence()
    -> Result<(), String> {
        let (corpus, _controls, _provenance) = load_committed()?;
        let row = row_by_category(&corpus, "contract_required")?;
        let mut cleared = row.clone();
        cleared.planning.root_corrections.clear();
        let failures = assess_contract_plan_row(&cleared);
        if !failures
            .iter()
            .any(|failure| failure.contains("records no root corrections"))
        {
            return Err(format!(
                "an amended contract without corrections must fail the law, got {failures:?}"
            ));
        }
        let mut blank = row.clone();
        blank.planning.root_corrections = vec!["  ".to_string()];
        let failures = assess_contract_plan_row(&blank);
        if !failures
            .iter()
            .any(|failure| failure.contains("blank root correction"))
        {
            return Err(format!(
                "a blank root correction must fail the law, got {failures:?}"
            ));
        }
        Ok(())
    }

    #[test]
    fn issue_lifecycle_contract_plan_pilot_accepted_state_rejected_while_decisions_open()
    -> Result<(), String> {
        let (corpus, _controls, _provenance) = load_committed()?;
        let mut accepted = row_by_category(&corpus, "contract_required")?.clone();
        {
            let contract = accepted
                .contract
                .as_mut()
                .ok_or_else(|| "the contract case must carry contract evidence".to_string())?;
            contract.contract_state = IssueLifecycleContractStateV1::Accepted;
        }
        accepted.attempt.contract_decision.root_disposition = Some("accepted".to_string());
        let failures = assess_contract_plan_row(&accepted);
        if !failures
            .iter()
            .any(|failure| failure.contains("accepted contract state while open decisions remain"))
        {
            return Err(format!(
                "an accepted state with open decisions must fail the law, got {failures:?}"
            ));
        }
        Ok(())
    }

    #[test]
    fn issue_lifecycle_contract_plan_pilot_root_role_and_config_independent_of_author()
    -> Result<(), String> {
        let (corpus, _controls, _provenance) = load_committed()?;
        let row = row_by_category(&corpus, "contract_required")?;
        let mut role_alias = row.clone();
        {
            let contract = role_alias
                .contract
                .as_mut()
                .ok_or_else(|| "the contract case must carry contract evidence".to_string())?;
            contract.root.role = contract.author.role.clone();
        }
        let failures = assess_contract_plan_row(&role_alias);
        if !failures.iter().any(|failure| failure.contains("root role")) {
            return Err(format!(
                "a root role aliased to the author must fail the law, got {failures:?}"
            ));
        }
        let mut config_alias = row.clone();
        {
            let contract = config_alias
                .contract
                .as_mut()
                .ok_or_else(|| "the contract case must carry contract evidence".to_string())?;
            contract.root.config_identity = contract.author.config_identity.clone();
        }
        let failures = assess_contract_plan_row(&config_alias);
        if !failures
            .iter()
            .any(|failure| failure.contains("root config identity"))
        {
            return Err(format!(
                "a root config aliased to the author must fail the law, got {failures:?}"
            ));
        }
        Ok(())
    }

    #[test]
    fn issue_lifecycle_contract_plan_pilot_blank_adversary_findings_fail_closed()
    -> Result<(), String> {
        let (corpus, _controls, _provenance) = load_committed()?;
        let mut blank = row_by_category(&corpus, "contract_required")?.clone();
        blank
            .contract
            .as_mut()
            .ok_or_else(|| "the contract case must carry contract evidence".to_string())?
            .adversary
            .findings = vec!["   ".to_string()];
        let failures = assess_contract_plan_row(&blank);
        if !failures
            .iter()
            .any(|failure| failure.contains("adversary findings must be nonblank"))
        {
            return Err(format!(
                "a blank adversary finding must fail the law, got {failures:?}"
            ));
        }
        Ok(())
    }

    #[test]
    fn issue_lifecycle_contract_plan_pilot_acceptance_coverage_binds_captured_issue()
    -> Result<(), String> {
        let (corpus, _controls, _provenance) = load_committed()?;
        let row = row_by_category(&corpus, "contract_required")?;
        // Mutate both sides identically and recompute the digest so only the
        // captured-issue binding can reject the drift.
        let mut drifted = row.clone();
        let unrelated = vec!["unrelated acceptance row".to_string()];
        drifted.planning.acceptance_covered = unrelated.clone();
        drifted.attempt.plan.acceptance_coverage = unrelated;
        drifted.attempt.row_digest =
            crate::issue_lifecycle_attempt::issue_lifecycle_row_digest(&drifted.attempt)?;
        let failures = assess_contract_plan_row(&drifted);
        if !failures
            .iter()
            .any(|failure| failure.contains("does not reference the captured issue"))
        {
            return Err(format!(
                "unrelated coverage text must fail the law, got {failures:?}"
            ));
        }
        let mut overlapped = row.clone();
        let shared = overlapped.planning.acceptance_covered[0].clone();
        overlapped.planning.acceptance_omitted.push(shared);
        let failures = assess_contract_plan_row(&overlapped);
        if !failures
            .iter()
            .any(|failure| failure.contains("duplicates an acceptance entry"))
        {
            return Err(format!(
                "an overlapping covered/omitted entry must fail the law, got {failures:?}"
            ));
        }
        Ok(())
    }

    #[test]
    fn issue_lifecycle_contract_plan_pilot_retrieval_command_bound_to_row_repository()
    -> Result<(), String> {
        let (corpus, _controls, _provenance) = load_committed()?;
        let mut foreign = corpus.rows[0].clone();
        foreign.retrieval_steps[0].command = "gh api repos/other/repo/issues/6225".to_string();
        let error = verify_contract_plan_row_snapshot(&foreign, &contract_plan_root())
            .err()
            .ok_or_else(|| {
                "a retrieval step naming another repository must fail closed".to_string()
            })?;
        if !error.contains("names another repository") {
            return Err(format!("unexpected verification error: {error}"));
        }
        Ok(())
    }
}
