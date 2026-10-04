//! Issue lifecycle read-only intake pilot (#4930, RIPR-SPEC-0223).
//!
//! The first real-corpus consumer of the frozen RIPR-SPEC-0218 attempt
//! contract (#4929): exactly six REAL current issue snapshots, captured
//! immutably and read-only, classified through the closed fifteen-value
//! disposition vocabulary with an independent root disposition per row.
//! The intake rows add packet observations (selected/omitted/overflow bytes,
//! triager findings, duplicate/already-satisfied/stale/spec-needed
//! candidates, exact missing-evidence questions, retrieval steps,
//! limitations/false candidates/non-claims) around the unchanged
//! `IssueLifecycleAttemptV1` rows, and the scorecard command projects the
//! embedded attempts through the unchanged #4929 validator and scorecard
//! builder, so results flow through #4929 without a parallel intake report.
//!
//! Selection law: this module selects no work. It reads only committed
//! corpus fields; no `active.toml`, label, issue age or title is consulted,
//! and the packet projection is a pure function of the corpus row. Mechanics
//! controls (false-duplicate correction, overlapping-PR pickup block,
//! explicit `not_measured` bytes) live in a separate synthetic control
//! corpus and never enter the six real rows or any real denominator.

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::issue_lifecycle_attempt::{
    IssueLifecycleAttemptV1, IssueLifecycleDispositionV1, IssueLifecycleRowAssessmentV1,
    assess_issue_lifecycle_attempt,
};

pub(crate) const ISSUE_LIFECYCLE_INTAKE_CORPUS_SCHEMA_VERSION: &str =
    "issue_lifecycle_intake_corpus.v1";
pub(crate) const ISSUE_LIFECYCLE_INTAKE_CONTROL_CORPUS_SCHEMA_VERSION: &str =
    "issue_lifecycle_intake_control_corpus.v1";
pub(crate) const ISSUE_LIFECYCLE_INTAKE_PROVENANCE_SCHEMA_VERSION: &str =
    "issue_lifecycle_intake_provenance.v1";
pub(crate) const DEFAULT_INTAKE_CORPUS_DIR: &str = "fixtures/issue_lifecycle_intake";

/// The closed six-category vocabulary #4930 requires, one row per category.
pub(crate) const REQUIRED_ISSUE_LIFECYCLE_INTAKE_CATEGORIES: [&str; 6] = [
    "narrow_accepted_contract_bug",
    "duplicate_or_already_satisfied",
    "overlapping_open_pr",
    "needs_evidence",
    "partially_landed_umbrella",
    "root_decision_required",
];

pub(crate) const ISSUE_LIFECYCLE_INTAKE_CLAIM_BOUNDARY: &str = "Read-only intake pilot receipt: \
 six exact real issue snapshots are classified at intake time through the RIPR-SPEC-0218 \
 vocabulary with independent root dispositions; it claims no triage universality, no \
 duplicate-detection accuracy beyond these rows, no implementation, no merge judgment and \
 no parent acceptance.";

/// A packet byte count: measured, or explicitly `not_measured` — a number is
/// never fabricated to fill a gap.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum IssueLifecycleIntakeBytesV1 {
    NotMeasured,
    Measured(u64),
}

/// One packet byte surface. Every surface is present on every row.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct IssueLifecycleIntakePacketBytesV1 {
    pub selected: IssueLifecycleIntakeBytesV1,
    pub omitted: IssueLifecycleIntakeBytesV1,
    pub overflow: IssueLifecycleIntakeBytesV1,
}

/// Closed candidate vocabulary a triager may raise at intake.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum IssueLifecycleIntakeCandidateKindV1 {
    Duplicate,
    AlreadySatisfied,
    Stale,
    SpecNeeded,
    OpenPrOverlap,
}

/// One triager candidate with the independent root review recorded on it. A
/// corrected candidate stays visible (`root_corrected: true` plus the root
/// note); a confirmed candidate keeps `root_corrected: false`.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct IssueLifecycleIntakeCandidateV1 {
    pub kind: IssueLifecycleIntakeCandidateKindV1,
    pub identity: String,
    pub evidence: String,
    pub root_corrected: bool,
    pub root_note: Option<String>,
}

/// The independent root disposition for one row, expressed in the closed
/// RIPR-SPEC-0218 vocabulary and bound to the embedded attempt disposition.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct IssueLifecycleIntakeRootDispositionV1 {
    pub disposition: IssueLifecycleDispositionV1,
    pub rationale: String,
    pub corrected_candidates: Vec<String>,
}

/// One artifact-archaeology/retrieval step with its measured byte count.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct IssueLifecycleIntakeRetrievalStepV1 {
    pub step: String,
    pub command: String,
    pub bytes: IssueLifecycleIntakeBytesV1,
}

/// Immutable snapshot identity for one row. Synthetic control rows carry
/// `None` paths and never touch the snapshot files.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct IssueLifecycleIntakeSnapshotV1 {
    pub issue_number: u64,
    pub issue_ref: String,
    pub snapshot_path: Option<String>,
    pub comments_path: Option<String>,
    pub timeline_path: Option<String>,
    pub timeline_snapshot_id: Option<String>,
    pub captured_at: String,
    pub issue_snapshot_id: String,
    pub comments_snapshot_id: String,
}

/// One intake row: the #4930 packet observations plus the embedded
/// RIPR-SPEC-0218 attempt the scorecard counts.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct IssueLifecycleIntakeRowV1 {
    pub id: String,
    pub category: String,
    pub snapshot: IssueLifecycleIntakeSnapshotV1,
    pub current_main: String,
    pub packet_bytes: IssueLifecycleIntakePacketBytesV1,
    pub open_candidates: Vec<IssueLifecycleIntakeCandidateV1>,
    pub triager_findings: Vec<String>,
    pub missing_evidence_questions: Vec<String>,
    pub root_disposition: IssueLifecycleIntakeRootDispositionV1,
    pub retrieval_steps: Vec<IssueLifecycleIntakeRetrievalStepV1>,
    pub limitations: Vec<String>,
    pub false_candidates: Vec<String>,
    pub non_claims: Vec<String>,
    pub attempt: IssueLifecycleAttemptV1,
}

/// The committed read-only intake corpus: exactly six real rows.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct IssueLifecycleIntakeCorpusV1 {
    pub schema_version: String,
    pub captured_at: String,
    pub base_main: String,
    pub rows: Vec<IssueLifecycleIntakeRowV1>,
}

/// One synthetic mechanics control row (outside the six real rows). The
/// intake row nests under `row` so the control metadata stays explicit.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct IssueLifecycleIntakeControlRowV1 {
    pub id: String,
    pub control: String,
    pub scenario: String,
    pub row: IssueLifecycleIntakeRowV1,
}

/// The committed mechanics control corpus; every embedded attempt is
/// synthetic and never enters a real denominator.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct IssueLifecycleIntakeControlCorpusV1 {
    pub schema_version: String,
    pub rows: Vec<IssueLifecycleIntakeControlRowV1>,
}

/// One provenance entry per real row.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct IssueLifecycleIntakeProvenanceRowV1 {
    pub issue: u64,
    pub category: String,
    pub justification: String,
}

/// Capture provenance: when, against which main, and why each row matches
/// its category.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct IssueLifecycleIntakeProvenanceV1 {
    pub schema_version: String,
    pub repository: String,
    pub captured_at: String,
    pub base_main: String,
    pub capture_method: String,
    pub rows: Vec<IssueLifecycleIntakeProvenanceRowV1>,
}

fn workspace_path(relative: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join(relative)
}

/// Load and shape-check the committed intake corpus: exactly six rows, the
/// closed category set covered once each, unique issue numbers, real (not
/// synthetic) attempts, and a non-empty root rationale per row.
pub(crate) fn load_issue_lifecycle_intake_corpus(
    body: &str,
) -> Result<IssueLifecycleIntakeCorpusV1, String> {
    let corpus: IssueLifecycleIntakeCorpusV1 = serde_json::from_str(body)
        .map_err(|error| format!("parse issue lifecycle intake corpus: {error}"))?;
    if corpus.schema_version != ISSUE_LIFECYCLE_INTAKE_CORPUS_SCHEMA_VERSION {
        return Err(format!(
            "unsupported issue lifecycle intake corpus schema `{}`",
            corpus.schema_version
        ));
    }
    if corpus.captured_at.trim().is_empty() || corpus.base_main.trim().is_empty() {
        return Err(
            "issue lifecycle intake corpus must record captured_at and base_main".to_string(),
        );
    }
    if corpus.rows.len() != REQUIRED_ISSUE_LIFECYCLE_INTAKE_CATEGORIES.len() {
        return Err(format!(
            "issue lifecycle intake corpus must carry exactly {} rows, got {}",
            REQUIRED_ISSUE_LIFECYCLE_INTAKE_CATEGORIES.len(),
            corpus.rows.len()
        ));
    }
    let mut categories = BTreeSet::new();
    let mut issues = BTreeSet::new();
    for row in &corpus.rows {
        if !categories.insert(row.category.clone()) {
            return Err(format!(
                "issue lifecycle intake corpus duplicates category `{}`",
                row.category
            ));
        }
        if !issues.insert(row.snapshot.issue_number) {
            return Err(format!(
                "issue lifecycle intake corpus duplicates issue `{}`",
                row.snapshot.issue_ref
            ));
        }
        if row.attempt.synthetic {
            return Err(format!(
                "issue lifecycle intake row `{}` must be a real row, not synthetic",
                row.id
            ));
        }
        if row.root_disposition.rationale.trim().is_empty() {
            return Err(format!(
                "issue lifecycle intake row `{}` has a blank root rationale",
                row.id
            ));
        }
    }
    for required in REQUIRED_ISSUE_LIFECYCLE_INTAKE_CATEGORIES {
        if !categories.contains(required) {
            return Err(format!(
                "issue lifecycle intake corpus is missing category `{required}`"
            ));
        }
    }
    Ok(corpus)
}

/// Load and shape-check the mechanics control corpus: unique ids, the
/// `control_` id prefix, and synthetic attempts only.
pub(crate) fn load_issue_lifecycle_intake_control_corpus(
    body: &str,
) -> Result<IssueLifecycleIntakeControlCorpusV1, String> {
    let corpus: IssueLifecycleIntakeControlCorpusV1 = serde_json::from_str(body)
        .map_err(|error| format!("parse issue lifecycle intake control corpus: {error}"))?;
    if corpus.schema_version != ISSUE_LIFECYCLE_INTAKE_CONTROL_CORPUS_SCHEMA_VERSION {
        return Err(format!(
            "unsupported issue lifecycle intake control corpus schema `{}`",
            corpus.schema_version
        ));
    }
    let mut ids = BTreeSet::new();
    for control in &corpus.rows {
        if !control.id.starts_with("control_") {
            return Err(format!(
                "issue lifecycle intake control row `{}` must use the `control_` id prefix",
                control.id
            ));
        }
        if !ids.insert(control.id.clone()) {
            return Err(format!(
                "issue lifecycle intake control corpus duplicates id `{}`",
                control.id
            ));
        }
        if control.control.trim().is_empty() || control.scenario.trim().is_empty() {
            return Err(format!(
                "issue lifecycle intake control row `{}` must name its control and scenario",
                control.id
            ));
        }
        if !control.row.attempt.synthetic {
            return Err(format!(
                "issue lifecycle intake control row `{}` must be synthetic",
                control.id
            ));
        }
    }
    Ok(corpus)
}

/// Load and parse the committed provenance file.
pub(crate) fn load_issue_lifecycle_intake_provenance(
    body: &str,
) -> Result<IssueLifecycleIntakeProvenanceV1, String> {
    let provenance: IssueLifecycleIntakeProvenanceV1 = serde_json::from_str(body)
        .map_err(|error| format!("parse issue lifecycle intake provenance: {error}"))?;
    if provenance.schema_version != ISSUE_LIFECYCLE_INTAKE_PROVENANCE_SCHEMA_VERSION {
        return Err(format!(
            "unsupported issue lifecycle intake provenance schema `{}`",
            provenance.schema_version
        ));
    }
    if provenance.base_main.trim().is_empty() || provenance.captured_at.trim().is_empty() {
        return Err(
            "issue lifecycle intake provenance must record captured_at and base_main".to_string(),
        );
    }
    if provenance.repository.trim().is_empty() || provenance.capture_method.trim().is_empty() {
        return Err(
            "issue lifecycle intake provenance must record repository and capture_method"
                .to_string(),
        );
    }
    Ok(provenance)
}

fn identity_matches(identity: &str, scheme: &str, computed_hex: &str) -> bool {
    identity == format!("{scheme}:sha256:{computed_hex}")
}

/// Fail-closed snapshot binding: recompute the snapshot and comment digests
/// from the committed bytes and reject any drift. A row whose snapshot file
/// is missing or altered can never be counted.
pub(crate) fn verify_intake_row_snapshot(
    row: &IssueLifecycleIntakeRowV1,
    root: &Path,
) -> Result<(), String> {
    let snapshot_path = row
        .snapshot
        .snapshot_path
        .as_ref()
        .ok_or_else(|| format!("intake row `{}` has no snapshot path", row.id))?;
    let comments_path = row
        .snapshot
        .comments_path
        .as_ref()
        .ok_or_else(|| format!("intake row `{}` has no comments path", row.id))?;
    if row.snapshot.captured_at.trim().is_empty() {
        return Err(format!(
            "intake row `{}` snapshot records no captured_at",
            row.id
        ));
    }
    let snapshot_body = fs::read(root.join(snapshot_path)).map_err(|error| {
        format!(
            "intake row `{}` snapshot {} is unreadable: {error}",
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
            "intake row `{}` snapshot digest drifted: recorded `{}`, recomputed `gh-issue-snapshot:sha256:{computed}`",
            row.id, row.snapshot.issue_snapshot_id
        ));
    }
    let comments_body = fs::read(root.join(comments_path)).map_err(|error| {
        format!(
            "intake row `{}` comments {} are unreadable: {error}",
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
            "intake row `{}` comments digest drifted: recorded `{}`, recomputed `gh-issue-comments:sha256:{computed}`",
            row.id, row.snapshot.comments_snapshot_id
        ));
    }
    let timeline_body = match &row.snapshot.timeline_path {
        Some(timeline_path) => Some(fs::read(root.join(timeline_path)).map_err(|error| {
            format!(
                "intake row `{}` timeline {} is unreadable: {error}",
                row.id, timeline_path
            )
        })?),
        None => None,
    };
    if let Some(timeline_body) = &timeline_body {
        let computed = crate::blind_journey::sha256_hex(timeline_body);
        let recorded = row
            .snapshot
            .timeline_snapshot_id
            .as_deref()
            .ok_or_else(|| format!("intake row `{}` timeline records no digest", row.id))?;
        if !identity_matches(recorded, "gh-issue-timeline", &computed) {
            return Err(format!(
                "intake row `{}` timeline digest drifted: recorded `{recorded}`, recomputed `gh-issue-timeline:sha256:{computed}`",
                row.id
            ));
        }
    }
    let retained_len =
        snapshot_body.len() + comments_body.len() + timeline_body.as_ref().map_or(0, Vec::len);
    verify_selected_packet_bytes(row, retained_len)?;
    if let Some(timeline_body) = &timeline_body {
        verify_retrieval_step_bytes(row, "issue", snapshot_body.len())?;
        verify_retrieval_step_bytes(row, "comments", comments_body.len())?;
        verify_retrieval_step_bytes(row, "timeline", timeline_body.len())?;
    }
    Ok(())
}

/// Bind the recorded selected-packet byte claim to the committed snapshot
/// bytes: a measured count must equal what the retained snapshots actually
/// measure. A fabricated or stale count fails closed instead of entering a
/// counted scorecard.
fn verify_selected_packet_bytes(
    row: &IssueLifecycleIntakeRowV1,
    retained_len: usize,
) -> Result<(), String> {
    if let IssueLifecycleIntakeBytesV1::Measured(selected) = row.packet_bytes.selected
        && selected as usize != retained_len
    {
        return Err(format!(
            "intake row `{}` packet selected bytes claim {selected}, committed snapshots measure {retained_len}",
            row.id
        ));
    }
    Ok(())
}

/// Bind recorded retrieval-step byte claims to the committed snapshot bytes:
/// a step that names one of the three capture commands must carry a measured
/// byte count equal to the committed file it produced. A fabricated or stale
/// count fails closed.
fn verify_retrieval_step_bytes(
    row: &IssueLifecycleIntakeRowV1,
    surface: &str,
    committed_len: usize,
) -> Result<(), String> {
    let suffix = match surface {
        "issue" => format!("/issues/{}", row.snapshot.issue_number),
        "comments" => format!("/issues/{}/comments", row.snapshot.issue_number),
        "timeline" => format!("/issues/{}/timeline", row.snapshot.issue_number),
        other => return Err(format!("unknown retrieval surface `{other}`")),
    };
    for step in &row.retrieval_steps {
        if !step.command.ends_with(&suffix) {
            continue;
        }
        if let IssueLifecycleIntakeBytesV1::Measured(bytes) = step.bytes
            && bytes as usize != committed_len
        {
            return Err(format!(
                "intake row `{}` retrieval step `{}` claims {bytes} bytes, committed bytes measure {committed_len}",
                row.id, step.command
            ));
        }
    }
    Ok(())
}

/// Every identity one row binds: snapshot identities, the issue ref, the
/// observed current main, PR/claim candidates and merged PR references.
/// Movement of any bound identity invalidates the row; movement outside the
/// set is unrelated and leaves the row intact.
#[cfg(test)]
pub(crate) fn intake_row_identity_set(row: &IssueLifecycleIntakeRowV1) -> BTreeSet<String> {
    let mut identities = BTreeSet::new();
    identities.insert(row.current_main.clone());
    identities.insert(row.snapshot.issue_snapshot_id.clone());
    identities.insert(row.snapshot.comments_snapshot_id.clone());
    identities.insert(row.snapshot.issue_ref.clone());
    for candidate in &row.open_candidates {
        identities.insert(candidate.identity.clone());
    }
    for pr in &row.attempt.context.relevant_prs {
        identities.insert(pr.clone());
    }
    for claim in &row.attempt.context.claim_ids {
        identities.insert(claim.clone());
    }
    if let Some(pr) = &row.attempt.execution_refs.pr {
        identities.insert(pr.clone());
    }
    if let Some(merge) = &row.attempt.execution_refs.merge {
        identities.insert(merge.clone());
    }
    identities
}

/// The portfolio-movement law: a movement whose identity the row binds
/// (current main, snapshot, overlapping PR, claim, merge) invalidates the
/// row; any other movement is unrelated and does not.
#[cfg(test)]
pub(crate) fn movement_invalidates_row(
    row: &IssueLifecycleIntakeRowV1,
    movement_identity: &str,
) -> bool {
    intake_row_identity_set(row).contains(movement_identity)
}

/// One deterministic packet projection: everything a cold-start root needs
/// to reconstruct the intake packet from the committed corpus alone, with
/// no chat, no live GitHub read and no selection signal. The projection is
/// a pure function of the corpus row; snapshot-only signals (title, labels,
/// age) never enter it.
#[derive(Clone, Debug, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct IssueLifecycleIntakePacketProjectionV1 {
    pub schema_version: String,
    pub id: String,
    pub issue_ref: String,
    pub category: String,
    pub disposition: IssueLifecycleDispositionV1,
    pub current_main: String,
    pub selected_bytes: IssueLifecycleIntakeBytesV1,
    pub omitted_bytes: IssueLifecycleIntakeBytesV1,
    pub overflow_bytes: IssueLifecycleIntakeBytesV1,
    pub candidates: Vec<IssueLifecycleIntakeCandidateV1>,
    pub triager_findings: Vec<String>,
    pub missing_evidence_questions: Vec<String>,
    pub root_rationale: String,
    pub retrieval_steps: Vec<IssueLifecycleIntakeRetrievalStepV1>,
    pub limitations: Vec<String>,
    pub false_candidates: Vec<String>,
    pub non_claims: Vec<String>,
}

pub(crate) fn build_intake_packet_projection(
    row: &IssueLifecycleIntakeRowV1,
) -> IssueLifecycleIntakePacketProjectionV1 {
    let mut candidates = row.open_candidates.clone();
    candidates.sort_by(|left, right| left.identity.cmp(&right.identity));
    IssueLifecycleIntakePacketProjectionV1 {
        schema_version: "issue_lifecycle_intake_packet.v1".to_string(),
        id: row.id.clone(),
        issue_ref: row.snapshot.issue_ref.clone(),
        category: row.category.clone(),
        disposition: row.root_disposition.disposition,
        current_main: row.current_main.clone(),
        selected_bytes: row.packet_bytes.selected,
        omitted_bytes: row.packet_bytes.omitted,
        overflow_bytes: row.packet_bytes.overflow,
        candidates,
        triager_findings: row.triager_findings.clone(),
        missing_evidence_questions: row.missing_evidence_questions.clone(),
        root_rationale: row.root_disposition.rationale.clone(),
        retrieval_steps: row.retrieval_steps.clone(),
        limitations: row.limitations.clone(),
        false_candidates: row.false_candidates.clone(),
        non_claims: row.non_claims.clone(),
    }
}

/// Per-row consistency assessment over the packet observations and the
/// embedded RIPR-SPEC-0218 attempt. Returns every failure; an empty vector
/// means the row is internally consistent and its root disposition is
/// genuinely independent (it agrees with the assessed attempt evidence).
pub(crate) fn assess_intake_row(row: &IssueLifecycleIntakeRowV1) -> Vec<String> {
    let mut failures = Vec::new();
    if row.root_disposition.disposition != row.attempt.disposition {
        failures.push(format!(
            "row `{}` root disposition {:?} disagrees with the attempt disposition {:?}",
            row.id, row.root_disposition.disposition, row.attempt.disposition
        ));
    }
    if row.missing_evidence_questions != row.attempt.intake.missing_evidence_questions {
        failures.push(format!(
            "row `{}` packet questions disagree with the attempt intake questions",
            row.id
        ));
    }
    let expects_questions = row.category == "needs_evidence";
    if expects_questions && row.missing_evidence_questions.is_empty() {
        failures.push(format!(
            "row `{}` is a needs-evidence row but records no exact questions",
            row.id
        ));
    }
    if !expects_questions && !row.missing_evidence_questions.is_empty() {
        failures.push(format!(
            "row `{}` records questions but is not a needs-evidence row",
            row.id
        ));
    }
    if row.category == "duplicate_or_already_satisfied"
        && !row
            .open_candidates
            .iter()
            .any(|candidate| candidate.kind == IssueLifecycleIntakeCandidateKindV1::Duplicate)
    {
        failures.push(format!(
            "row `{}` claims a duplicate disposition without a duplicate candidate identity",
            row.id
        ));
    }
    let has_open_overlap = row
        .open_candidates
        .iter()
        .any(|candidate| candidate.kind == IssueLifecycleIntakeCandidateKindV1::OpenPrOverlap);
    if has_open_overlap && row.attempt.disposition != IssueLifecycleDispositionV1::Blocked {
        failures.push(format!(
            "row `{}` binds an overlapping open PR but is not blocked",
            row.id
        ));
    }
    for candidate in &row.open_candidates {
        if candidate.root_corrected
            && (!row.false_candidates.contains(&candidate.identity)
                || !row
                    .root_disposition
                    .corrected_candidates
                    .contains(&candidate.identity))
        {
            failures.push(format!(
                "row `{}` corrected candidate `{}` must stay visible in false_candidates and corrected_candidates",
                row.id, candidate.identity
            ));
        }
        if candidate.root_corrected && candidate.root_note.is_none() {
            failures.push(format!(
                "row `{}` corrected candidate `{}` records no root note",
                row.id, candidate.identity
            ));
        }
    }
    for corrected in &row.root_disposition.corrected_candidates {
        if !row
            .open_candidates
            .iter()
            .any(|candidate| candidate.identity == *corrected && candidate.root_corrected)
        {
            failures.push(format!(
                "row `{}` lists corrected candidate `{corrected}` without a matching corrected candidate",
                row.id
            ));
        }
    }
    failures
}

/// Assess every row of the committed corpus plus every synthetic control
/// row by the same packet law.
pub(crate) fn assess_intake_corpus(corpus: &IssueLifecycleIntakeCorpusV1) -> Vec<String> {
    let mut failures = Vec::new();
    for row in &corpus.rows {
        failures.extend(assess_intake_row(row));
    }
    failures
}

pub(crate) fn assess_intake_control_corpus(
    corpus: &IssueLifecycleIntakeControlCorpusV1,
) -> Vec<String> {
    let mut failures = Vec::new();
    for control in &corpus.rows {
        failures.extend(assess_intake_row(&control.row));
    }
    failures
}

fn parse_corpus_dir_arg(args: &[String]) -> Result<String, String> {
    const USAGE: &str = "usage: cargo xtask issue-lifecycle-intake-scorecard [--corpus <dir>]";
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
    Ok(corpus.unwrap_or_else(|| DEFAULT_INTAKE_CORPUS_DIR.to_string()))
}

/// Read one committed intake corpus directory end to end: corpus, controls,
/// provenance and snapshot bindings, all fail-closed. Shared by the
/// scorecard command and the fixture-contract gate.
pub(crate) fn load_intake_corpus_dir(
    dir: &Path,
) -> Result<
    (
        IssueLifecycleIntakeCorpusV1,
        IssueLifecycleIntakeControlCorpusV1,
        IssueLifecycleIntakeProvenanceV1,
    ),
    String,
> {
    let corpus_body = fs::read_to_string(dir.join("corpus.json"))
        .map_err(|error| format!("read issue lifecycle intake corpus: {error}"))?;
    let corpus = load_issue_lifecycle_intake_corpus(&corpus_body)?;
    let controls_body = fs::read_to_string(dir.join("controls.json"))
        .map_err(|error| format!("read issue lifecycle intake controls: {error}"))?;
    let controls = load_issue_lifecycle_intake_control_corpus(&controls_body)?;
    let provenance_body = fs::read_to_string(dir.join("provenance.json"))
        .map_err(|error| format!("read issue lifecycle intake provenance: {error}"))?;
    let provenance = load_issue_lifecycle_intake_provenance(&provenance_body)?;
    check_provenance_against_corpus(&corpus, &provenance)?;
    for row in &corpus.rows {
        verify_intake_row_snapshot(row, dir)?;
    }
    Ok((corpus, controls, provenance))
}

/// Fail-closed provenance shape check shared by the scorecard command path:
/// the provenance must name exactly the corpus rows — no missing entry, no
/// duplicate issue, and no extra row the corpus does not contain.
fn check_provenance_against_corpus(
    corpus: &IssueLifecycleIntakeCorpusV1,
    provenance: &IssueLifecycleIntakeProvenanceV1,
) -> Result<(), String> {
    if provenance.base_main != corpus.base_main {
        return Err(format!(
            "intake provenance base main `{}` disagrees with the corpus base main `{}`",
            provenance.base_main, corpus.base_main
        ));
    }
    if provenance.rows.len() != corpus.rows.len() {
        return Err(format!(
            "intake provenance must name {} rows, got {}",
            corpus.rows.len(),
            provenance.rows.len()
        ));
    }
    let mut provenance_issues = BTreeSet::new();
    for entry in &provenance.rows {
        if !provenance_issues.insert(entry.issue) {
            return Err(format!(
                "intake provenance duplicates issue `{}`",
                entry.issue
            ));
        }
    }
    for row in &corpus.rows {
        if !provenance_issues.contains(&row.snapshot.issue_number) {
            return Err(format!(
                "intake row `{}` has no provenance entry for issue `{}`",
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
                "intake row `{}` category `{}` disagrees with its provenance category `{entry}`",
                row.id, row.category
            ));
        }
    }
    Ok(())
}

/// Fail-closed RIPR-SPEC-0218 assessment of the embedded real attempt rows:
/// a rejected row fails the run, and a counted row whose deterministically
/// downgraded assessed disposition disagrees with the row's independent root
/// disposition fails too — the command never emits a scorecard stronger than
/// the retained evidence.
fn assess_real_rows_against_counting_law(
    corpus: &IssueLifecycleIntakeCorpusV1,
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
                "intake row `{}` attempt was rejected by the RIPR-SPEC-0218 counting law: {:?}",
                row.id, assessment.reasons
            ));
        }
        if assessment.disposition != Some(row.root_disposition.disposition) {
            failures.push(format!(
                "intake row `{}` root disposition {:?} disagrees with the assessed disposition {:?}",
                row.id,
                row.root_disposition.disposition,
                assessment.disposition
            ));
        }
    }
    (failures, assessed)
}

/// `cargo xtask issue-lifecycle-intake-scorecard [--corpus <dir>]` (#4930,
/// RIPR-SPEC-0223): validate the committed read-only intake corpus
/// fail-closed (shape, provenance, snapshot digest bindings, packet law),
/// then project the embedded real attempt rows through the unchanged
/// RIPR-SPEC-0218 validator and scorecard builder, writing the standard
/// issue-lifecycle scorecard reports — results flow through #4929 without a
/// parallel intake report. The deterministic per-row packet projections are
/// printed to stdout.
pub(crate) fn issue_lifecycle_intake_scorecard(args: &[String]) -> Result<(), String> {
    let corpus_dir = parse_corpus_dir_arg(args)?;
    let root = workspace_path(&corpus_dir);
    let (corpus, controls, _provenance) = load_intake_corpus_dir(&root)?;
    let mut failures = assess_intake_corpus(&corpus);
    failures.extend(assess_intake_control_corpus(&controls));
    let (law_failures, assessed) = assess_real_rows_against_counting_law(&corpus);
    failures.extend(law_failures);
    let attempts: Vec<IssueLifecycleAttemptV1> =
        corpus.rows.iter().map(|row| row.attempt.clone()).collect();
    for control in &controls.rows {
        let assessment = assess_issue_lifecycle_attempt(&control.row.attempt);
        if !assessment.counted {
            failures.push(format!(
                "intake control `{}` was rejected by the RIPR-SPEC-0218 counting law: {:?}",
                control.id, assessment.reasons
            ));
        }
    }
    if !failures.is_empty() {
        return Err(format!(
            "issue lifecycle intake corpus failed assessment: {failures:?}"
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
    println!("{}", ISSUE_LIFECYCLE_INTAKE_CLAIM_BOUNDARY);
    let mut projections: Vec<IssueLifecycleIntakePacketProjectionV1> = corpus
        .rows
        .iter()
        .map(build_intake_packet_projection)
        .collect();
    projections.sort_by(|left, right| left.id.cmp(&right.id));
    let packet_body = serde_json::to_string_pretty(&projections)
        .map_err(|error| format!("serialize intake packet projections: {error}"))?;
    println!("{packet_body}");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::issue_lifecycle_attempt::IssueLifecycleCloseoutStateV1;

    fn intake_root() -> PathBuf {
        workspace_path(DEFAULT_INTAKE_CORPUS_DIR)
    }

    fn load_committed() -> Result<
        (
            IssueLifecycleIntakeCorpusV1,
            IssueLifecycleIntakeControlCorpusV1,
            IssueLifecycleIntakeProvenanceV1,
        ),
        String,
    > {
        load_intake_corpus_dir(&intake_root())
    }

    fn row_by_category<'a>(
        corpus: &'a IssueLifecycleIntakeCorpusV1,
        category: &str,
    ) -> Result<&'a IssueLifecycleIntakeRowV1, String> {
        corpus
            .rows
            .iter()
            .find(|row| row.category == category)
            .ok_or_else(|| format!("committed corpus is missing category `{category}`"))
    }

    #[test]
    fn issue_lifecycle_intake_pilot_committed_corpus_loads_with_six_categories()
    -> Result<(), String> {
        let (corpus, controls, provenance) = load_committed()?;
        if corpus.rows.len() != 6 {
            return Err(format!("expected six real rows, got {}", corpus.rows.len()));
        }
        if provenance.rows.len() != 6 {
            return Err(format!(
                "expected six provenance rows, got {}",
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
        Ok(())
    }

    #[test]
    fn issue_lifecycle_intake_pilot_false_duplicate_candidate_corrected_by_root_review()
    -> Result<(), String> {
        let (_corpus, controls, _provenance) = load_committed()?;
        let control = controls
            .rows
            .iter()
            .find(|control| control.control == "false_duplicate_candidate_corrected_by_root_review")
            .ok_or_else(|| {
                "control corpus is missing the false-duplicate correction control".to_string()
            })?;
        let candidate = control
            .row
            .open_candidates
            .first()
            .ok_or_else(|| "the correction control must carry a candidate".to_string())?;
        if !candidate.root_corrected {
            return Err("the false duplicate candidate must be marked root-corrected".to_string());
        }
        if candidate.root_note.is_none() {
            return Err("the correction must retain the root note".to_string());
        }
        if control.row.attempt.disposition != IssueLifecycleDispositionV1::QualifiedOnePr {
            return Err(format!(
                "the corrected row must stay qualified, got {:?}",
                control.row.attempt.disposition
            ));
        }
        if !control.row.false_candidates.contains(&candidate.identity) {
            return Err(
                "the corrected candidate must stay visible in false_candidates".to_string(),
            );
        }
        if !control
            .row
            .root_disposition
            .corrected_candidates
            .contains(&candidate.identity)
        {
            return Err(
                "the corrected candidate must stay visible in corrected_candidates".to_string(),
            );
        }
        if !assess_intake_control_corpus(&controls).is_empty() {
            return Err("the committed control corpus must assess clean".to_string());
        }
        Ok(())
    }

    #[test]
    fn issue_lifecycle_intake_pilot_real_overlapping_pr_blocks_duplicate_pickup()
    -> Result<(), String> {
        let (_corpus, controls, _provenance) = load_committed()?;
        let control = controls
            .rows
            .iter()
            .find(|control| control.control == "real_overlapping_pr_blocks_duplicate_pickup")
            .ok_or_else(|| {
                "control corpus is missing the overlapping-PR pickup control".to_string()
            })?;
        let has_duplicate = control
            .row
            .open_candidates
            .iter()
            .any(|candidate| candidate.kind == IssueLifecycleIntakeCandidateKindV1::Duplicate);
        let has_overlap =
            control.row.open_candidates.iter().any(|candidate| {
                candidate.kind == IssueLifecycleIntakeCandidateKindV1::OpenPrOverlap
            });
        if !has_duplicate || !has_overlap {
            return Err(
                "the control must carry both a duplicate candidate and the overlapping PR"
                    .to_string(),
            );
        }
        if control.row.attempt.disposition != IssueLifecycleDispositionV1::Blocked {
            return Err(format!(
                "a real overlapping PR must block pickup, got {:?}",
                control.row.attempt.disposition
            ));
        }
        Ok(())
    }

    #[test]
    fn issue_lifecycle_intake_pilot_narrow_issue_skips_spec_bureaucracy() -> Result<(), String> {
        let (corpus, _controls, _provenance) = load_committed()?;
        let row = row_by_category(&corpus, "narrow_accepted_contract_bug")?;
        let artifacts = &row.attempt.contract_artifacts;
        if row.attempt.contract_decision.spec_required {
            return Err("the narrow bug must not require a spec".to_string());
        }
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
        Ok(())
    }

    #[test]
    fn issue_lifecycle_intake_pilot_architecture_ambiguity_stops_at_root_decision_required()
    -> Result<(), String> {
        let (corpus, _controls, _provenance) = load_committed()?;
        let row = row_by_category(&corpus, "root_decision_required")?;
        if row.attempt.disposition != IssueLifecycleDispositionV1::RootDecisionRequired {
            return Err(format!(
                "the architecture ambiguity must stop at root_decision_required, got {:?}",
                row.attempt.disposition
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
            return Err("no contract artifact may be drafted before the root decision".to_string());
        }
        if row.attempt.contract_decision.root_disposition.is_some() {
            return Err("the root decision must not be pre-recorded on the attempt".to_string());
        }
        if row.root_disposition.rationale.is_empty() {
            return Err("the row must retain the root rationale".to_string());
        }
        Ok(())
    }

    #[test]
    fn issue_lifecycle_intake_pilot_missing_evidence_yields_exact_questions() -> Result<(), String>
    {
        let (corpus, _controls, _provenance) = load_committed()?;
        let row = row_by_category(&corpus, "needs_evidence")?;
        if row.attempt.disposition != IssueLifecycleDispositionV1::NeedsEvidence {
            return Err(format!(
                "the row must stay needs_evidence, got {:?}",
                row.attempt.disposition
            ));
        }
        if row.missing_evidence_questions.len() < 3 {
            return Err(format!(
                "the row must retain its exact questions, got {}",
                row.missing_evidence_questions.len()
            ));
        }
        if row.missing_evidence_questions != row.attempt.intake.missing_evidence_questions {
            return Err("packet questions and attempt questions must agree".to_string());
        }
        for question in &row.missing_evidence_questions {
            if !question.ends_with('?') {
                return Err(format!("an exact question must be a question: {question}"));
            }
        }
        if !row.attempt.plan.work_items.is_empty() {
            return Err(
                "a needs-evidence row must not guess implementation work items".to_string(),
            );
        }
        Ok(())
    }

    #[test]
    fn issue_lifecycle_intake_pilot_partial_umbrella_retains_uncovered_acceptance()
    -> Result<(), String> {
        let (corpus, _controls, _provenance) = load_committed()?;
        let row = row_by_category(&corpus, "partially_landed_umbrella")?;
        if row.attempt.disposition != IssueLifecycleDispositionV1::PartiallyLanded {
            return Err(format!(
                "the umbrella must stay partially_landed, got {:?}",
                row.attempt.disposition
            ));
        }
        if row.attempt.burn_down.uncovered_rows.is_empty() {
            return Err("the umbrella must retain its uncovered acceptance rows".to_string());
        }
        if row.attempt.execution_refs.merge.is_none() {
            return Err("the umbrella must bind the merged child identity".to_string());
        }
        if row.attempt.closeout.state != IssueLifecycleCloseoutStateV1::NotStarted {
            return Err("a merged child must not advance the umbrella closeout".to_string());
        }
        if row.attempt.closeout.current_head_verification.is_some() {
            return Err(
                "a merged child must not populate current-head closeout verification".to_string(),
            );
        }
        Ok(())
    }

    #[test]
    fn issue_lifecycle_intake_pilot_duplicate_row_names_evidence_and_stays_first_class()
    -> Result<(), String> {
        let (corpus, _controls, _provenance) = load_committed()?;
        let row = row_by_category(&corpus, "duplicate_or_already_satisfied")?;
        if row.attempt.disposition != IssueLifecycleDispositionV1::Duplicate {
            return Err(format!(
                "the row must be a duplicate, got {:?}",
                row.attempt.disposition
            ));
        }
        let candidate = row
            .open_candidates
            .iter()
            .find(|candidate| candidate.kind == IssueLifecycleIntakeCandidateKindV1::Duplicate)
            .ok_or_else(|| "the duplicate row must name its duplicate-of identity".to_string())?;
        if !candidate.identity.contains("#5270") {
            return Err(format!(
                "the duplicate-of identity must name issue 5270, got {}",
                candidate.identity
            ));
        }
        if candidate.evidence.trim().is_empty() {
            return Err("the duplicate candidate must retain its evidence".to_string());
        }
        let assessment = assess_issue_lifecycle_attempt(&row.attempt);
        if !assessment.counted {
            return Err(format!(
                "the duplicate row must stay a first-class counted row, got {:?}",
                assessment.reasons
            ));
        }
        Ok(())
    }

    #[test]
    fn issue_lifecycle_intake_pilot_relevant_movement_invalidates_row_unrelated_does_not()
    -> Result<(), String> {
        let (corpus, controls, _provenance) = load_committed()?;
        for row in corpus
            .rows
            .iter()
            .chain(controls.rows.iter().map(|control| &control.row))
        {
            if !movement_invalidates_row(row, &row.current_main) {
                return Err(format!(
                    "movement of the bound current main must invalidate row `{}`",
                    row.id
                ));
            }
            if movement_invalidates_row(row, "unrelated-movement-identity-0000000000000000") {
                return Err(format!(
                    "unrelated movement must not invalidate row `{}`",
                    row.id
                ));
            }
        }
        let overlap = row_by_category(&corpus, "overlapping_open_pr")?;
        if !movement_invalidates_row(overlap, "pr-5408") {
            return Err(
                "movement of the overlapping PR identity must invalidate the row".to_string(),
            );
        }
        let umbrella = row_by_category(&corpus, "partially_landed_umbrella")?;
        if !movement_invalidates_row(umbrella, "pr-5366") {
            return Err(
                "movement of the merged child identity must invalidate the umbrella row"
                    .to_string(),
            );
        }
        if movement_invalidates_row(umbrella, &format!("{}-unrelated", umbrella.current_main)) {
            return Err("a similar-but-unbound movement must not invalidate the row".to_string());
        }
        Ok(())
    }

    #[test]
    fn issue_lifecycle_intake_pilot_no_active_toml_label_age_or_title_selects_work()
    -> Result<(), String> {
        let (corpus, _controls, _provenance) = load_committed()?;
        let projections: Vec<String> = corpus
            .rows
            .iter()
            .map(|row| {
                serde_json::to_string(&build_intake_packet_projection(row))
                    .map_err(|error| format!("serialize packet projection: {error}"))
            })
            .collect::<Result<_, _>>()?;
        for (row, projection) in corpus.rows.iter().zip(projections.iter()) {
            let snapshot_body = fs::read_to_string(
                intake_root().join(
                    row.snapshot
                        .snapshot_path
                        .clone()
                        .ok_or_else(|| format!("row `{}` has no snapshot path", row.id))?,
                ),
            )
            .map_err(|error| format!("read snapshot for selection-signal check: {error}"))?;
            let snapshot: serde_json::Value = serde_json::from_str(&snapshot_body)
                .map_err(|error| format!("parse snapshot for selection-signal check: {error}"))?;
            let title = snapshot
                .get("title")
                .and_then(serde_json::Value::as_str)
                .unwrap_or_default();
            if !title.is_empty() && projection.contains(title) {
                return Err(format!(
                    "row `{}` packet projection leaked the issue title; no title signal may select work",
                    row.id
                ));
            }
            let labels = snapshot
                .get("labels")
                .and_then(serde_json::Value::as_array)
                .cloned()
                .unwrap_or_default();
            for label in labels {
                let name = label
                    .get("name")
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or_default();
                let quoted = format!("\"{name}\"");
                if !name.is_empty() && projection.contains(&quoted) {
                    return Err(format!(
                        "row `{}` packet projection leaked the label `{name}`; no label signal may select work",
                        row.id
                    ));
                }
            }
        }
        Ok(())
    }

    #[test]
    fn issue_lifecycle_intake_pilot_cold_start_root_reconstructs_packet_without_chat()
    -> Result<(), String> {
        let (corpus, _controls, _provenance) = load_committed()?;
        let first: Vec<String> = corpus
            .rows
            .iter()
            .map(|row| {
                serde_json::to_string(&build_intake_packet_projection(row))
                    .map_err(|error| format!("serialize packet projection: {error}"))
            })
            .collect::<Result<_, _>>()?;
        let reloaded = load_intake_corpus_dir(&intake_root())?;
        let second: Vec<String> = reloaded
            .0
            .rows
            .iter()
            .map(|row| {
                serde_json::to_string(&build_intake_packet_projection(row))
                    .map_err(|error| format!("serialize packet projection: {error}"))
            })
            .collect::<Result<_, _>>()?;
        if first != second {
            return Err(
                "a cold-start reload from the committed corpus alone must reconstruct byte-identical packets".to_string(),
            );
        }
        Ok(())
    }

    #[test]
    fn issue_lifecycle_intake_pilot_packet_bytes_complete_or_explicitly_not_measured()
    -> Result<(), String> {
        let (corpus, controls, _provenance) = load_committed()?;
        let mut checked = 0usize;
        for row in corpus
            .rows
            .iter()
            .chain(controls.rows.iter().map(|control| &control.row))
        {
            if let IssueLifecycleIntakeBytesV1::Measured(selected) = row.packet_bytes.selected
                && row.snapshot.snapshot_path.is_some()
            {
                let committed = committed_snapshot_bytes(row)?;
                if selected as usize != committed {
                    return Err(format!(
                        "row `{}` measured selected bytes {selected}, committed snapshots measure {committed}",
                        row.id
                    ));
                }
            }
            let surfaces = [
                &row.packet_bytes.selected,
                &row.packet_bytes.omitted,
                &row.packet_bytes.overflow,
            ];
            for surface in surfaces {
                match surface {
                    IssueLifecycleIntakeBytesV1::Measured(_bytes) => {}
                    IssueLifecycleIntakeBytesV1::NotMeasured => {}
                }
                checked += 1;
            }
        }
        if checked != (corpus.rows.len() + controls.rows.len()) * 3 {
            return Err(format!(
                "expected {} byte surfaces, checked {checked}",
                (corpus.rows.len() + controls.rows.len()) * 3
            ));
        }
        let control = controls
            .rows
            .iter()
            .find(|control| control.control == "packet_bytes_not_measured_is_explicit")
            .ok_or_else(|| {
                "control corpus is missing the not-measured bytes control".to_string()
            })?;
        if control.row.packet_bytes.selected != IssueLifecycleIntakeBytesV1::NotMeasured {
            return Err("the control row must say not_measured explicitly".to_string());
        }
        Ok(())
    }

    fn committed_snapshot_bytes(row: &IssueLifecycleIntakeRowV1) -> Result<usize, String> {
        let mut total = 0usize;
        for path in [
            row.snapshot.snapshot_path.as_deref(),
            row.snapshot.comments_path.as_deref(),
            row.snapshot.timeline_path.as_deref(),
        ]
        .into_iter()
        .flatten()
        {
            let bytes = fs::read(intake_root().join(path))
                .map_err(|error| format!("read committed snapshot {path}: {error}"))?;
            total += bytes.len();
        }
        Ok(total)
    }

    #[test]
    fn issue_lifecycle_intake_pilot_fabricated_selected_bytes_fail_closed() -> Result<(), String> {
        let (corpus, _controls, _provenance) = load_committed()?;
        let mut row = corpus.rows[0].clone();
        row.packet_bytes.selected = IssueLifecycleIntakeBytesV1::Measured(1);
        let error = verify_intake_row_snapshot(&row, &intake_root())
            .err()
            .ok_or_else(|| "a fabricated selected byte count must fail closed".to_string())?;
        if !error.contains("packet selected bytes claim") {
            return Err(format!("unexpected verification error: {error}"));
        }
        Ok(())
    }

    #[test]
    fn issue_lifecycle_intake_pilot_downgraded_root_disposition_rejected() -> Result<(), String> {
        let (mut corpus, _controls, _provenance) = load_committed()?;
        let lifecycle_id = {
            let row = corpus
                .rows
                .iter_mut()
                .find(|row| row.category == "narrow_accepted_contract_bug")
                .ok_or_else(|| "missing narrow accepted-contract bug row".to_string())?;
            row.attempt.contract_decision.spec_required = true;
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
                "the spec-required downgrade keeps the row counted, so only the root-disposition disagreement may reject it".to_string(),
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
    fn issue_lifecycle_intake_pilot_extra_provenance_row_rejected() -> Result<(), String> {
        let (corpus, _controls, mut provenance) = load_committed()?;
        check_provenance_against_corpus(&corpus, &provenance)?;
        provenance.rows.push(IssueLifecycleIntakeProvenanceRowV1 {
            issue: 9_999_999,
            category: "narrow_accepted_contract_bug".to_string(),
            justification: "synthetic extra provenance entry".to_string(),
        });
        let error = check_provenance_against_corpus(&corpus, &provenance)
            .err()
            .ok_or_else(|| "an extra provenance row must fail closed".to_string())?;
        if !error.contains("must name 6 rows, got 7") {
            return Err(format!("unexpected provenance error: {error}"));
        }
        Ok(())
    }
}
