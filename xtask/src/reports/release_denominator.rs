//! Deterministic historical supplemental-denominator ledger for the former
//! 0.11 C/T release model. It is not active live-head selection authority.
//!
//! Captured ledgers are replayable without GitHub. The optional live mode
//! compares the captured range and candidate-tree identities with bounded
//! first-parent Git observations. Neither mode selects a candidate or mutates
//! repository state.

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;
use std::time::Duration;

use super::candidate_control::{CandidateSelection, evaluate as evaluate_candidate_selection};

const REPORT_NAME: &str = "release-denominator";
const SCHEMA_VERSION: &str = "0.1";
const INPUT_KIND: &str = "release_denominator_snapshot";
const JSON_FILE: &str = "release-denominator.json";
const MARKDOWN_FILE: &str = "release-denominator.md";
const LIVE_TIMEOUT: Duration = Duration::from_secs(30);
const GITHUB_CAPTURE_TIMEOUT: Duration = Duration::from_secs(45);
const ACCEPTED_EXECUTION_EXCLUSION_ID: &str = "exclusion:2767:verification-execution";
const EXECUTION_EXCLUSION_GRANULARITY: &str = "hunk_or_symbol";
const ADJUDICATION_REVIEW_PREFIXES: &[&str] = &["review:2832:", "review:2825:"];

const DISPOSITIONS: &[&str] = &[
    "include_product",
    "include_release_infrastructure",
    "include_control_or_honesty",
    "structural_no_semantic_delta",
    "candidate_only_exclusion",
    "source_only_followup",
    "safe_defer_post_0_11",
    "operator_decision_required",
];

const TREE_STATES: &[&str] = &[
    "present_in_candidate",
    "absent_by_candidate_only_exclusion",
    "replaced_by_later_commit",
    "source_only_not_in_swarm_candidate",
    "candidate_tree_state_pending",
];

const CAPTURE_STATUSES: &[&str] = &[
    "not_captured",
    "captured",
    "no_linked_authority",
    "ambiguous",
    "unavailable",
];

const REFERENCE_KINDS: &[&str] = &[
    "merge_pr",
    "issue",
    "pull_request",
    "reviewed_manual_mapping",
];

const REFERENCE_SOURCES: &[&str] = &[
    "associated_pull_request",
    "closing_reference",
    "body_reference",
    "explicit_review",
];

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
struct Snapshot {
    schema_version: String,
    kind: String,
    captured_at: String,
    source: SnapshotSource,
    #[serde(default)]
    candidate_selection: Option<CandidateSelection>,
    records: Vec<CommitRecord>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
struct SnapshotSource {
    mode: String,
    stage: String,
    freshness: String,
    historical_base_sha: String,
    #[serde(default)]
    provisional_review_cutoff_sha: Option<String>,
    #[serde(default)]
    github_repository: Option<String>,
    candidate_ref: String,
    candidate_sha: String,
    range_commits: Vec<String>,
    candidate_tree_commits: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
struct CommitRecord {
    commit_sha: String,
    first_parent_position: usize,
    subject: String,
    #[serde(default)]
    pr_refs: Vec<u64>,
    #[serde(default)]
    issue_refs: Vec<u64>,
    #[serde(default)]
    references: Vec<ReferenceEvidence>,
    #[serde(default)]
    claim_refs: Vec<String>,
    #[serde(default = "default_capture_status")]
    reference_capture_status: String,
    #[serde(default)]
    reference_capture_limitation: String,
    theme_or_surface: String,
    release_disposition: String,
    acceptance_owner: String,
    candidate_tree_state: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    candidate_only_excluded_paths: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    candidate_only_exclusion_granularity: Option<String>,
    #[serde(default)]
    replacement_commit_sha: Option<String>,
    #[serde(default)]
    review_refs: Vec<String>,
    #[serde(default)]
    proof_refs: Vec<String>,
    source_survivor_or_swarm_exclusion_effect: String,
    limitation_or_operator_decision: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
struct ReferenceEvidence {
    kind: String,
    number: u64,
    source: String,
    #[serde(default)]
    evidence_url: Option<String>,
    #[serde(default)]
    github_identity: Option<String>,
    observed_for_commit_sha: String,
    reviewed: bool,
    limitation: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
struct GithubCapture {
    schema_version: String,
    kind: String,
    repository: String,
    observed_for_candidate_sha: String,
    observed_range_digest: String,
    records: Vec<GithubCaptureRecord>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
struct GithubCaptureRecord {
    commit_sha: String,
    status: String,
    #[serde(default)]
    limitation: String,
    #[serde(default)]
    references: Vec<ReferenceEvidence>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct AdjudicationManifest {
    schema_version: String,
    kind: String,
    cutoff_sha: String,
    batches: Vec<AdjudicationBatch>,
    #[serde(default)]
    overrides: Vec<AdjudicationOverride>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct AdjudicationBatch {
    id: String,
    positions: Vec<usize>,
    disposition: String,
    candidate_tree_state: String,
    review_ref: String,
    rationale: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct AdjudicationOverride {
    position: usize,
    batch_id: String,
    #[serde(default)]
    exclusion_id: Option<String>,
    disposition: String,
    candidate_tree_state: String,
    review_ref: String,
    delivered_delta: String,
    challenged_disposition: String,
    candidate_effect: String,
    residual_acceptance: String,
    #[serde(default)]
    candidate_only_excluded_paths: Vec<String>,
    #[serde(default)]
    candidate_only_exclusion_granularity: Option<String>,
    #[serde(default)]
    review_evidence: Vec<String>,
}

#[derive(Clone, Debug, Deserialize)]
struct GithubPullRequest {
    number: u64,
    url: String,
    #[serde(default)]
    body: Option<String>,
    #[serde(rename = "mergeCommit")]
    merge_commit: Option<GithubCommitRef>,
}

#[derive(Clone, Debug, Deserialize)]
struct GithubCommitRef {
    oid: String,
}

#[derive(Clone, Debug)]
enum DenominatorCommand<'a> {
    Normalize {
        live: bool,
        path: &'a str,
    },
    Capture {
        path: &'a str,
        output: &'a str,
    },
    Import {
        path: &'a str,
        capture: &'a str,
        output: &'a str,
    },
    Adjudicate {
        path: &'a str,
        decisions: &'a str,
        output: &'a str,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct LiveFacts {
    candidate_sha: Option<String>,
    range_commits: Vec<String>,
    candidate_tree_commits: Vec<String>,
    errors: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct NormalizedReport {
    status: String,
    captured_at: String,
    source: SnapshotSource,
    candidate_selection: Option<CandidateSelection>,
    records: Vec<CommitRecord>,
    counts_by_disposition: BTreeMap<String, usize>,
    counts_by_tree_state: BTreeMap<String, usize>,
    range_digest: String,
    candidate_tree_digest: String,
    record_set_digest: String,
    reconciliation_reasons: Vec<String>,
    next_action: String,
}

pub(crate) fn release_denominator(args: &[String]) -> Result<(), String> {
    match parse_args(args)? {
        DenominatorCommand::Normalize { live, path } => {
            let snapshot = read_snapshot(path)?;
            let live_facts = live.then(|| collect_live_facts(&snapshot.source));
            let report = normalize_snapshot(snapshot, live_facts.as_ref())?;
            let json_text =
                serde_json::to_string_pretty(&report_json(&report)).map_err(|error| {
                    format!("failed to serialize release-denominator JSON: {error}")
                })?;
            crate::write_report(JSON_FILE, &format!("{json_text}\n"))?;
            crate::write_report(MARKDOWN_FILE, &report_markdown(&report))?;
            println!("Wrote target/ripr/reports/{JSON_FILE}");
            println!("Wrote target/ripr/reports/{MARKDOWN_FILE}");
        }
        DenominatorCommand::Capture { path, output } => capture_github(path, output)?,
        DenominatorCommand::Import {
            path,
            capture,
            output,
        } => import_github(path, capture, output)?,
        DenominatorCommand::Adjudicate {
            path,
            decisions,
            output,
        } => apply_adjudication(path, decisions, output)?,
    }
    Ok(())
}

fn parse_args<'a>(args: &'a [String]) -> Result<DenominatorCommand<'a>, String> {
    let usage = "usage: cargo xtask release-denominator [--live] --input <ledger.json> | --capture-github --input <ledger.json> --output <capture.json> | --import-github --input <ledger.json> --capture <capture.json> --output <ledger.json> | --apply-adjudication --input <ledger.json> --decisions <adjudication.json> --output <ledger.json>";
    match args {
        [input, path] if input == "--input" && !path.trim().is_empty() => {
            Ok(DenominatorCommand::Normalize { live: false, path })
        }
        [live, input, path]
            if live == "--live" && input == "--input" && !path.trim().is_empty() =>
        {
            Ok(DenominatorCommand::Normalize { live: true, path })
        }
        [mode, input, path, output_flag, output]
            if mode == "--capture-github"
                && input == "--input"
                && output_flag == "--output"
                && !path.trim().is_empty()
                && !output.trim().is_empty() =>
        {
            Ok(DenominatorCommand::Capture { path, output })
        }
        [
            mode,
            input,
            path,
            capture_flag,
            capture,
            output_flag,
            output,
        ] if mode == "--import-github"
            && input == "--input"
            && capture_flag == "--capture"
            && output_flag == "--output"
            && !path.trim().is_empty()
            && !capture.trim().is_empty()
            && !output.trim().is_empty() =>
        {
            Ok(DenominatorCommand::Import {
                path,
                capture,
                output,
            })
        }
        [
            mode,
            input,
            path,
            decisions_flag,
            decisions,
            output_flag,
            output,
        ] if mode == "--apply-adjudication"
            && input == "--input"
            && decisions_flag == "--decisions"
            && output_flag == "--output"
            && !path.trim().is_empty()
            && !decisions.trim().is_empty()
            && !output.trim().is_empty() =>
        {
            Ok(DenominatorCommand::Adjudicate {
                path,
                decisions,
                output,
            })
        }
        [help] if help == "--help" || help == "-h" => Err(usage.to_string()),
        _ => Err(usage.to_string()),
    }
}

fn default_capture_status() -> String {
    "not_captured".to_string()
}

fn read_snapshot(path: &str) -> Result<Snapshot, String> {
    let text = fs::read_to_string(path)
        .map_err(|error| format!("failed to read release-denominator input {path}: {error}"))?;
    serde_json::from_str(&text)
        .map_err(|error| format!("failed to parse release-denominator input {path}: {error}"))
}

fn capture_github(input: &str, output: &str) -> Result<(), String> {
    let snapshot = read_snapshot(input)?;
    let repository = gh_repository_name()?;
    validate_capture_repository(snapshot.source.github_repository.as_deref(), &repository)?;
    let all_prs = gh_pull_requests(&repository)?;
    let mut by_merge_sha: BTreeMap<String, Vec<&GithubPullRequest>> = BTreeMap::new();
    let mut known_prs = BTreeMap::new();
    for pull_request in &all_prs {
        known_prs.insert(pull_request.number, pull_request);
        if let Some(merge_commit) = &pull_request.merge_commit {
            by_merge_sha
                .entry(merge_commit.oid.clone())
                .or_default()
                .push(pull_request);
        }
    }

    let mut records = Vec::with_capacity(snapshot.records.len());
    for record in &snapshot.records {
        let Some(merge_prs) = by_merge_sha.get(&record.commit_sha) else {
            records.push(GithubCaptureRecord {
                commit_sha: record.commit_sha.clone(),
                status: "no_linked_authority".to_string(),
                limitation: "No merged PR or linked PR/issue reference was found in retained GitHub metadata.".to_string(),
                references: Vec::new(),
            });
            continue;
        };
        if merge_prs.len() != 1 {
            records.push(GithubCaptureRecord {
                commit_sha: record.commit_sha.clone(),
                status: "ambiguous".to_string(),
                limitation: format!(
                    "Multiple merged PRs claim commit {}: {}.",
                    record.commit_sha,
                    merge_prs
                        .iter()
                        .map(|pull_request| format!("#{}", pull_request.number))
                        .collect::<Vec<_>>()
                        .join(", ")
                ),
                references: Vec::new(),
            });
            continue;
        }

        let pull_request = merge_prs[0];
        let mut references = Vec::new();
        references.push(ReferenceEvidence {
            kind: "merge_pr".to_string(),
            number: pull_request.number,
            source: "associated_pull_request".to_string(),
            evidence_url: Some(pull_request.url.clone()),
            github_identity: None,
            observed_for_commit_sha: record.commit_sha.clone(),
            reviewed: false,
            limitation:
                "Captured from GitHub merge metadata; operator adjudication remains required."
                    .to_string(),
        });
        let body = pull_request.body.as_deref().unwrap_or_default();
        for (number, source) in body_issue_or_pr_references(body) {
            if number == pull_request.number {
                continue;
            }
            let kind = if known_prs.contains_key(&number) {
                "pull_request"
            } else {
                "issue"
            };
            references.push(ReferenceEvidence {
                kind: kind.to_string(),
                number,
                source: source.to_string(),
                evidence_url: None,
                github_identity: Some(format!(
                    "pr-body:{}:{}:{}",
                    pull_request.number, source, number
                )),
                observed_for_commit_sha: record.commit_sha.clone(),
                reviewed: false,
                limitation: "Captured from a retained GitHub PR body; operator adjudication remains required.".to_string(),
            });
        }
        references.sort_by_key(|reference| {
            (
                reference.kind.clone(),
                reference.number,
                reference.source.clone(),
            )
        });
        references.dedup_by_key(|reference| {
            (
                reference.kind.clone(),
                reference.number,
                reference.source.clone(),
            )
        });
        records.push(GithubCaptureRecord {
            commit_sha: record.commit_sha.clone(),
            status: "captured".to_string(),
            limitation: "References are captured authority, not yet operator-reviewed.".to_string(),
            references,
        });
    }

    let range_digest = digest_json(&snapshot.source.range_commits)?;
    let capture = GithubCapture {
        schema_version: "0.1".to_string(),
        kind: "release_denominator_github_capture".to_string(),
        repository,
        observed_for_candidate_sha: snapshot.source.candidate_sha,
        observed_range_digest: range_digest,
        records,
    };
    let text = serde_json::to_string_pretty(&capture)
        .map_err(|error| format!("failed to serialize GitHub denominator capture: {error}"))?;
    fs::write(output, format!("{text}\n"))
        .map_err(|error| format!("failed to write GitHub denominator capture {output}: {error}"))?;
    println!("Wrote GitHub denominator capture {output}");
    Ok(())
}

fn import_github(input: &str, capture_path: &str, output: &str) -> Result<(), String> {
    let mut snapshot = read_snapshot(input)?;
    let capture_text = fs::read_to_string(capture_path).map_err(|error| {
        format!("failed to read GitHub denominator capture {capture_path}: {error}")
    })?;
    let capture: GithubCapture = serde_json::from_str(&capture_text).map_err(|error| {
        format!("failed to parse GitHub denominator capture {capture_path}: {error}")
    })?;
    if capture.schema_version != "0.1" || capture.kind != "release_denominator_github_capture" {
        return Err("GitHub denominator capture has an unsupported schema or kind".to_string());
    }
    validate_capture_repository(
        snapshot.source.github_repository.as_deref(),
        &capture.repository,
    )?;
    if capture.observed_for_candidate_sha != snapshot.source.candidate_sha {
        return Err(
            "GitHub denominator capture candidate SHA does not match input ledger".to_string(),
        );
    }
    let expected_range_digest = digest_json(&snapshot.source.range_commits)?;
    if capture.observed_range_digest != expected_range_digest {
        return Err(
            "GitHub denominator capture range digest does not match input ledger".to_string(),
        );
    }
    let mut by_commit = BTreeMap::new();
    for record in capture.records {
        if by_commit
            .insert(record.commit_sha.clone(), record)
            .is_some()
        {
            return Err("GitHub denominator capture contains a duplicate commit".to_string());
        }
    }
    for record in &mut snapshot.records {
        let captured = by_commit.get(&record.commit_sha).ok_or_else(|| {
            format!(
                "GitHub denominator capture is missing commit {}",
                record.commit_sha
            )
        })?;
        if !CAPTURE_STATUSES.contains(&captured.status.as_str()) {
            return Err(format!(
                "GitHub denominator capture has invalid status {} for {}",
                captured.status, record.commit_sha
            ));
        }
        record.references = captured.references.clone();
        record.pr_refs.clear();
        record.issue_refs.clear();
        record.reference_capture_status = captured.status.clone();
        record.reference_capture_limitation = captured.limitation.clone();
    }
    apply_provisional_cutoff_boundary(&mut snapshot)?;
    let text = serde_json::to_string_pretty(&snapshot)
        .map_err(|error| format!("failed to serialize imported denominator ledger: {error}"))?;
    fs::write(output, format!("{text}\n")).map_err(|error| {
        format!("failed to write imported denominator ledger {output}: {error}")
    })?;
    println!("Wrote imported denominator ledger {output}");
    Ok(())
}

fn validate_capture_repository(expected: Option<&str>, observed: &str) -> Result<(), String> {
    let expected = expected.ok_or_else(|| {
        "GitHub denominator input must pin source.github_repository before capture or import"
            .to_string()
    })?;
    if expected != observed {
        return Err(format!(
            "GitHub denominator capture repository {observed} does not match expected {expected}"
        ));
    }
    Ok(())
}

fn apply_adjudication(input: &str, decisions_path: &str, output: &str) -> Result<(), String> {
    let mut snapshot = read_snapshot(input)?;
    let decisions_text = fs::read_to_string(decisions_path).map_err(|error| {
        format!("failed to read denominator adjudication {decisions_path}: {error}")
    })?;
    let manifest: AdjudicationManifest =
        serde_json::from_str(&decisions_text).map_err(|error| {
            format!("failed to parse denominator adjudication {decisions_path}: {error}")
        })?;
    if manifest.schema_version != "0.1" || manifest.kind != "release_denominator_adjudication" {
        return Err("denominator adjudication has an unsupported schema or kind".to_string());
    }
    let cutoff = snapshot
        .source
        .provisional_review_cutoff_sha
        .as_deref()
        .ok_or_else(|| "input ledger has no provisional review cutoff".to_string())?;
    if manifest.cutoff_sha != cutoff {
        return Err("denominator adjudication cutoff does not match input ledger".to_string());
    }
    let cutoff_position = snapshot
        .records
        .iter()
        .position(|record| record.commit_sha == cutoff)
        .ok_or_else(|| format!("adjudication cutoff {cutoff} is not in the input ledger"))?
        + 1;
    let mut coverage = BTreeMap::new();
    let mut batch_ids = BTreeSet::new();
    for batch in &manifest.batches {
        if batch.id.trim().is_empty()
            || !batch_ids.insert(batch.id.clone())
            || batch.review_ref.trim().is_empty()
            || !is_adjudication_review_ref(&batch.review_ref)
            || batch.rationale.trim().is_empty()
        {
            return Err(
                "adjudication batches require unique id, a well-formed review:<issue>:<slug> review_ref, and rationale"
                    .to_string(),
            );
        }
        if batch.positions.is_empty()
            || batch
                .positions
                .iter()
                .any(|position| *position == 0 || *position > cutoff_position)
        {
            return Err(format!(
                "adjudication batch {} has an invalid first-parent position list",
                batch.id
            ));
        }
        if !DISPOSITIONS.contains(&batch.disposition.as_str())
            || batch.disposition == "operator_decision_required"
        {
            return Err(format!(
                "adjudication batch {} has a non-closed disposition {}",
                batch.id, batch.disposition
            ));
        }
        if !TREE_STATES.contains(&batch.candidate_tree_state.as_str())
            || batch.candidate_tree_state == "candidate_tree_state_pending"
        {
            return Err(format!(
                "adjudication batch {} has a non-closed candidate-tree state {}",
                batch.id, batch.candidate_tree_state
            ));
        }
        for position in &batch.positions {
            if coverage.insert(*position, batch.id.clone()).is_some() {
                return Err(format!(
                    "adjudication batches overlap at first-parent position {position}"
                ));
            }
        }
    }
    let mut overrides_by_position = BTreeMap::new();
    for override_record in &manifest.overrides {
        if override_record.position == 0
            || override_record.position > cutoff_position
            || override_record.batch_id.trim().is_empty()
            || !batch_ids.contains(&override_record.batch_id)
            || !is_adjudication_review_ref(&override_record.review_ref)
            || override_record.delivered_delta.trim().is_empty()
            || override_record.challenged_disposition.trim().is_empty()
            || override_record.candidate_effect.trim().is_empty()
            || override_record.residual_acceptance.trim().is_empty()
            || override_record
                .review_evidence
                .iter()
                .any(|evidence| evidence.trim().is_empty())
            || !DISPOSITIONS.contains(&override_record.disposition.as_str())
            || override_record.disposition == "operator_decision_required"
            || !TREE_STATES.contains(&override_record.candidate_tree_state.as_str())
            || override_record.candidate_tree_state == "candidate_tree_state_pending"
        {
            return Err(format!(
                "adjudication override at position {} is malformed or not closed",
                override_record.position
            ));
        }
        validate_override_exclusion_paths(override_record)?;
        if overrides_by_position
            .insert(override_record.position, override_record)
            .is_some()
        {
            return Err(format!(
                "adjudication overrides overlap at first-parent position {}",
                override_record.position
            ));
        }
        if coverage.get(&override_record.position) != Some(&override_record.batch_id) {
            return Err(format!(
                "adjudication override at position {} does not belong to batch {}",
                override_record.position, override_record.batch_id
            ));
        }
    }
    for position in 1..=cutoff_position {
        let batch_id = coverage.get(&position).ok_or_else(|| {
            format!("adjudication has no decision for first-parent position {position}")
        })?;
        let batch = manifest
            .batches
            .iter()
            .find(|batch| &batch.id == batch_id)
            .ok_or_else(|| format!("adjudication batch {batch_id} disappeared"))?;
        let record = snapshot
            .records
            .get_mut(position - 1)
            .ok_or_else(|| format!("ledger is missing first-parent position {position}"))?;
        if record.first_parent_position != position {
            return Err(format!(
                "ledger record at position {position} has first_parent_position {}",
                record.first_parent_position
            ));
        }
        let override_record = overrides_by_position.get(&position).copied();
        let disposition = override_record.map_or(batch.disposition.as_str(), |record| {
            record.disposition.as_str()
        });
        let candidate_tree_state = override_record
            .map_or(batch.candidate_tree_state.as_str(), |record| {
                record.candidate_tree_state.as_str()
            });
        record.release_disposition = disposition.to_string();
        record.candidate_tree_state = candidate_tree_state.to_string();
        apply_candidate_only_override(record, override_record)?;
        record.review_refs = vec![batch.review_ref.clone(), format!("batch:{}", batch.id)];
        if let Some(override_record) = override_record {
            record.review_refs.push(override_record.review_ref.clone());
            record
                .review_refs
                .extend(override_record.review_evidence.iter().cloned());
            record.review_refs.push(format!("override:{}", position));
            record.source_survivor_or_swarm_exclusion_effect = format!(
                "#{} {} reviewed commit {} at first-parent position {}: delivered delta: {}; challenged disposition: {}; accepted disposition: {}; candidate effect: {}.",
                adjudication_issue_tag(&batch.review_ref),
                batch.id,
                record.commit_sha,
                position,
                override_record.delivered_delta,
                override_record.challenged_disposition,
                override_record.disposition,
                override_record.candidate_effect,
            );
            record.limitation_or_operator_decision = format!(
                "{} Release non-claim: this provisional disposition does not close the owning issue, prove candidate qualification, or claim source promotion.",
                override_record.residual_acceptance
            );
        } else {
            record.source_survivor_or_swarm_exclusion_effect = format!(
                "#{} {} reviewed commit {} at first-parent position {}: {}",
                adjudication_issue_tag(&batch.review_ref),
                batch.id,
                record.commit_sha,
                position,
                batch.rationale
            );
            record.limitation_or_operator_decision = format!(
                "This provisional disposition does not close the owning issue, prove candidate qualification, or claim source promotion. Residual acceptance remains with {}.",
                record.acceptance_owner
            );
        }
    }
    snapshot.source.candidate_tree_commits =
        project_candidate_tree(&snapshot.records, &snapshot.source.range_commits);
    let normalized = normalize_snapshot(snapshot.clone(), None)?;
    if normalized.status != "ready" {
        return Err(format!(
            "adjudication produced a non-reconciling ledger: {}",
            normalized.reconciliation_reasons.join("; ")
        ));
    }
    let text = serde_json::to_string_pretty(&snapshot)
        .map_err(|error| format!("failed to serialize adjudicated denominator ledger: {error}"))?;
    fs::write(output, format!("{text}\n")).map_err(|error| {
        format!("failed to write adjudicated denominator ledger {output}: {error}")
    })?;
    println!("Wrote adjudicated denominator ledger {output}");
    Ok(())
}

fn project_candidate_tree(records: &[CommitRecord], range_commits: &[String]) -> Vec<String> {
    let candidate_tree = records
        .iter()
        .filter(|record| record.candidate_tree_state == "present_in_candidate")
        .map(|record| record.commit_sha.as_str())
        .collect::<BTreeSet<_>>();
    range_commits
        .iter()
        .filter(|commit_sha| candidate_tree.contains(commit_sha.as_str()))
        .cloned()
        .collect()
}

fn validate_override_exclusion_paths(override_record: &AdjudicationOverride) -> Result<(), String> {
    let supplied = normalized_exclusion_paths(override_record)?;
    if override_record.disposition != "candidate_only_exclusion" {
        if !supplied.is_empty()
            || override_record.exclusion_id.is_some()
            || override_record
                .candidate_only_exclusion_granularity
                .is_some()
        {
            return Err(format!(
                "adjudication override at position {} carries exclusion paths without candidate_only_exclusion disposition",
                override_record.position
            ));
        }
        return Ok(());
    }
    if override_record.exclusion_id.as_deref() != Some(ACCEPTED_EXECUTION_EXCLUSION_ID) {
        return Err(format!(
            "adjudication override at position {} is not bound to the accepted #2767 exclusion",
            override_record.position
        ));
    }
    if override_record.candidate_tree_state == "present_in_candidate" && supplied.is_empty() {
        return Err(format!(
            "adjudication override at position {} has no path-level exclusion",
            override_record.position
        ));
    }
    if override_record
        .candidate_only_exclusion_granularity
        .as_deref()
        != Some(EXECUTION_EXCLUSION_GRANULARITY)
    {
        return Err(format!(
            "adjudication override at position {} does not declare hunk-or-symbol exclusion granularity",
            override_record.position
        ));
    }
    let accepted = accepted_execution_excluded_paths()?;
    if supplied != accepted {
        return Err(format!(
            "adjudication override at position {} does not match the accepted #2767 path set",
            override_record.position
        ));
    }
    Ok(())
}

fn accepted_execution_excluded_paths() -> Result<BTreeSet<String>, String> {
    let scope: Value = serde_json::from_str(include_str!(
        "../../../fixtures/release_scope/accepted-outcome-a.json"
    ))
    .map_err(|error| format!("failed to parse accepted execution scope fixture: {error}"))?;
    scope
        .get("candidate_excluded_paths")
        .and_then(Value::as_array)
        .ok_or_else(|| "accepted execution scope has no candidate_excluded_paths".to_string())?
        .iter()
        .map(|path| {
            path.as_str()
                .filter(|path| !path.trim().is_empty())
                .map(|path| path.trim().replace('\\', "/"))
                .ok_or_else(|| "accepted execution scope contains a non-string path".to_string())
        })
        .collect()
}

fn apply_candidate_only_override(
    record: &mut CommitRecord,
    override_record: Option<&AdjudicationOverride>,
) -> Result<(), String> {
    record.candidate_only_excluded_paths = override_record
        .map(normalized_exclusion_paths)
        .transpose()?
        .unwrap_or_default()
        .into_iter()
        .collect();
    record.candidate_only_exclusion_granularity =
        override_record.and_then(|record| record.candidate_only_exclusion_granularity.clone());
    Ok(())
}

fn normalized_exclusion_paths(
    override_record: &AdjudicationOverride,
) -> Result<BTreeSet<String>, String> {
    let mut supplied = BTreeSet::new();
    for path in &override_record.candidate_only_excluded_paths {
        let normalized = path.trim().replace('\\', "/");
        if normalized.is_empty()
            || normalized == "."
            || normalized == ".."
            || normalized
                .split('/')
                .any(|part| part.is_empty() || part == "." || part == "..")
            || !supplied.insert(normalized)
        {
            return Err(format!(
                "adjudication override at position {} has blank, unsafe, or duplicate exclusion paths",
                override_record.position
            ));
        }
    }
    Ok(supplied)
}

fn apply_provisional_cutoff_boundary(snapshot: &mut Snapshot) -> Result<(), String> {
    let Some(cutoff) = snapshot.source.provisional_review_cutoff_sha.as_deref() else {
        return Ok(());
    };
    let cutoff_position = snapshot
        .records
        .iter()
        .position(|record| record.commit_sha == cutoff)
        .ok_or_else(|| {
            format!("provisional review cutoff {cutoff} is not in the captured range")
        })?;
    for (index, record) in snapshot.records.iter_mut().enumerate() {
        if index > cutoff_position || record.release_disposition == "safe_defer_post_0_11" {
            record.release_disposition = "operator_decision_required".to_string();
            record.candidate_tree_state = "candidate_tree_state_pending".to_string();
            record.source_survivor_or_swarm_exclusion_effect =
                "Observed in the provisional denominator; candidate-tree inclusion or exclusion awaits record-level adjudication in #2832."
                    .to_string();
            record.limitation_or_operator_decision =
                "No blanket post-cutoff exclusion is accepted. Review the exact commit, claim mapping, proof, and candidate-tree effect before candidate selection."
                    .to_string();
        }
    }
    Ok(())
}

fn gh_repository_name() -> Result<String, String> {
    let output = crate::run::run_output_owned(
        "gh",
        &[
            "repo".to_string(),
            "view".to_string(),
            "--json".to_string(),
            "nameWithOwner".to_string(),
        ],
    )?;
    let value: Value = serde_json::from_str(&output)
        .map_err(|error| format!("failed to parse gh repository JSON: {error}"))?;
    value
        .get("nameWithOwner")
        .and_then(Value::as_str)
        .map(str::to_string)
        .ok_or_else(|| "gh repository JSON is missing nameWithOwner".to_string())
}

fn gh_pull_requests(repository: &str) -> Result<Vec<GithubPullRequest>, String> {
    let args = vec![
        "pr".to_string(),
        "list".to_string(),
        "--repo".to_string(),
        repository.to_string(),
        "--state".to_string(),
        "all".to_string(),
        "--limit".to_string(),
        "10000".to_string(),
        "--json".to_string(),
        "number,url,body,mergeCommit".to_string(),
    ];
    let output = crate::run::capture_output_with_timeout(
        "gh",
        &args,
        &[],
        GITHUB_CAPTURE_TIMEOUT,
        "GitHub denominator PR capture",
    )?;
    if output.timed_out {
        return Err("GitHub denominator PR capture timed out".to_string());
    }
    let status = output
        .status
        .ok_or_else(|| "GitHub denominator PR capture did not report a status".to_string())?;
    if !status.success() {
        return Err(format!(
            "GitHub denominator PR capture failed with {status}: {}",
            output.stderr.trim()
        ));
    }
    serde_json::from_str(&output.stdout)
        .map_err(|error| format!("failed to parse GitHub PR capture: {error}"))
}

fn body_issue_or_pr_references(body: &str) -> Vec<(u64, &'static str)> {
    let bytes = body.as_bytes();
    let mut references = Vec::new();
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] != b'#'
            || (index > 0 && (bytes[index - 1].is_ascii_alphanumeric() || bytes[index - 1] == b'_'))
        {
            index += 1;
            continue;
        }
        let start = index + 1;
        let mut end = start;
        while end < bytes.len() && bytes[end].is_ascii_digit() {
            end += 1;
        }
        if end == start {
            index += 1;
            continue;
        }
        let Ok(number) = body[start..end].parse::<u64>() else {
            index = end;
            continue;
        };
        let source = if has_explicit_closing_keyword(&body[..index]) {
            "closing_reference"
        } else {
            "body_reference"
        };
        references.push((number, source));
        index = end;
    }
    references
}

fn has_explicit_closing_keyword(prefix: &str) -> bool {
    let words = prefix
        .split_whitespace()
        .rev()
        .take(3)
        .map(|word| {
            word.trim_matches(|character: char| !character.is_ascii_alphanumeric())
                .to_ascii_lowercase()
        })
        .collect::<Vec<_>>();
    let Some(keyword) = words.first().map(String::as_str) else {
        return false;
    };
    if !matches!(
        keyword,
        "close"
            | "closes"
            | "closed"
            | "fix"
            | "fixes"
            | "fixed"
            | "resolve"
            | "resolves"
            | "resolved"
    ) {
        return false;
    }
    !matches!(words.get(1).map(String::as_str), Some("not" | "never"))
}

fn normalize_snapshot(
    mut snapshot: Snapshot,
    live_facts: Option<&LiveFacts>,
) -> Result<NormalizedReport, String> {
    let mut reasons = Vec::new();
    validate_source(&snapshot, &mut reasons);
    if let Some(facts) = live_facts {
        reasons.extend(facts.errors.iter().cloned());
        if facts.candidate_sha.as_deref() != Some(snapshot.source.candidate_sha.as_str()) {
            reasons
                .push("live candidate ref does not resolve to captured candidate_sha".to_string());
        }
        if facts.range_commits != snapshot.source.range_commits {
            reasons.push("live first-parent range differs from captured range_commits".to_string());
        }
        if facts.candidate_tree_commits != snapshot.source.candidate_tree_commits {
            reasons.push(
                "live candidate tree differs from captured candidate_tree_commits".to_string(),
            );
        }
    }

    let expected = snapshot
        .source
        .range_commits
        .iter()
        .map(String::as_str)
        .collect::<BTreeSet<_>>();
    let mut seen = BTreeSet::new();
    let mut counts_by_disposition = BTreeMap::new();
    let mut counts_by_tree_state = BTreeMap::new();
    for (index, record) in snapshot.records.iter_mut().enumerate() {
        if !seen.insert(record.commit_sha.clone()) {
            reasons.push(format!(
                "commit {} appears more than once",
                record.commit_sha
            ));
        }
        if !expected.contains(record.commit_sha.as_str()) {
            reasons.push(format!(
                "commit {} is outside the captured range",
                record.commit_sha
            ));
        }
        if snapshot
            .source
            .range_commits
            .get(index)
            .is_none_or(|expected_sha| expected_sha != &record.commit_sha)
        {
            reasons.push(format!(
                "record at position {} does not match first-parent range order",
                index + 1
            ));
        }
        if record.first_parent_position != index + 1 {
            reasons.push(format!(
                "commit {} has first_parent_position {}, expected {}",
                record.commit_sha,
                record.first_parent_position,
                index + 1
            ));
        }
        normalize_reference_projections(record, &mut reasons);
        validate_record(record, &snapshot.source, &mut reasons);
        validate_capture_metadata(record, &mut reasons);
        validate_claim_refs(record, snapshot.candidate_selection.as_ref(), &mut reasons);
        *counts_by_disposition
            .entry(record.release_disposition.clone())
            .or_insert(0) += 1;
        *counts_by_tree_state
            .entry(record.candidate_tree_state.clone())
            .or_insert(0) += 1;
    }
    if snapshot.records.len() != snapshot.source.range_commits.len() {
        reasons.push(format!(
            "record count {} does not reconcile with range count {}",
            snapshot.records.len(),
            snapshot.source.range_commits.len()
        ));
    }
    reconcile_provisional_decision_count(&mut snapshot, &mut reasons);
    let record_set_digest = digest_json(&snapshot.records)?;
    validate_final_cut_authority(&snapshot, &record_set_digest, &mut reasons);
    if snapshot.source.stage == "final"
        && snapshot
            .records
            .iter()
            .any(|record| record.release_disposition == "operator_decision_required")
    {
        reasons.push("final ledger retains operator_decision_required".to_string());
    }
    let range_digest = digest_json(&snapshot.source.range_commits)?;
    let candidate_tree_digest = digest_json(&snapshot.source.candidate_tree_commits)?;
    let status = if reasons.is_empty() {
        "ready"
    } else {
        "reconcile_required"
    };
    let next_action = if status == "ready" {
        if snapshot.source.stage == "final" {
            "Bind this record-set digest unchanged into the immutable candidate manifest and qualification bundle.".to_string()
        } else {
            "Review every provisional disposition before promoting this ledger to the final candidate.".to_string()
        }
    } else {
        "Refresh the exact first-parent range, candidate tree, and reviewed record dispositions; do not consume this ledger as complete.".to_string()
    };
    Ok(NormalizedReport {
        status: status.to_string(),
        captured_at: snapshot.captured_at,
        source: snapshot.source,
        candidate_selection: snapshot.candidate_selection,
        records: snapshot.records,
        counts_by_disposition,
        counts_by_tree_state,
        range_digest,
        candidate_tree_digest,
        record_set_digest,
        reconciliation_reasons: reasons,
        next_action,
    })
}

fn validate_final_cut_authority(
    snapshot: &Snapshot,
    record_set_digest: &str,
    reasons: &mut Vec<String>,
) {
    let Some(selection) = snapshot.candidate_selection.as_ref() else {
        return;
    };
    let Some(authority) = selection.final_cut_authority.as_ref() else {
        if selection.selected_cut_sha.is_some() {
            reasons.push("selected cut has no final-cut authority ledger".to_string());
        }
        return;
    };
    if selection.selected_cut_sha.as_deref() != Some(authority.cut_sha.as_str()) {
        reasons.push("final-cut authority is not bound to selected_cut_sha".to_string());
    }
    if authority.record_set_digest.as_deref() != Some(record_set_digest) {
        reasons.push(
            "final-cut authority is not bound to the normalized denominator record-set digest"
                .to_string(),
        );
    }
    let Some(cut_position) = snapshot
        .records
        .iter()
        .position(|record| record.commit_sha == authority.cut_sha)
    else {
        reasons.push(format!(
            "final-cut authority cut {} is absent from the denominator records",
            authority.cut_sha
        ));
        return;
    };
    let Some(provisional_cutoff) = snapshot.source.provisional_review_cutoff_sha.as_deref() else {
        reasons.push("final-cut authority requires a provisional review cutoff".to_string());
        return;
    };
    let Some(provisional_position) = snapshot
        .records
        .iter()
        .position(|record| record.commit_sha == provisional_cutoff)
    else {
        reasons.push(format!(
            "provisional review cutoff {provisional_cutoff} is absent from the denominator records"
        ));
        return;
    };
    if cut_position < provisional_position {
        reasons.push("selected cut precedes the provisional review cutoff".to_string());
        return;
    }
    let provisional_decisions_remaining = snapshot.records[..=provisional_position]
        .iter()
        .filter(|record| record.release_disposition == "operator_decision_required")
        .count() as u64;
    let post_provisional_records = if cut_position > provisional_position {
        &snapshot.records[provisional_position + 1..=cut_position]
    } else {
        &snapshot.records[0..0]
    };
    let unreviewed_post_provisional_records_through_cut = post_provisional_records
        .iter()
        .filter(|record| {
            !record
                .review_refs
                .iter()
                .any(|review_ref| is_adjudication_review_ref(review_ref))
        })
        .count() as u64;
    for record in post_provisional_records {
        if !record
            .review_refs
            .iter()
            .any(|review_ref| is_adjudication_review_ref(review_ref))
            && !record.review_refs.is_empty()
        {
            reasons.push(format!(
                "post-provisional record {} has no valid adjudication review authority",
                record.commit_sha
            ));
        }
    }
    let final_cut_decisions_remaining = snapshot.records[..=cut_position]
        .iter()
        .filter(|record| record.release_disposition == "operator_decision_required")
        .count() as u64;
    if authority.provisional_decisions_remaining != provisional_decisions_remaining {
        reasons.push(format!(
            "final-cut authority provisional decision count disagrees with records: supplied {}, derived {}",
            authority.provisional_decisions_remaining, provisional_decisions_remaining
        ));
    }
    if authority.unreviewed_post_provisional_records_through_cut
        != unreviewed_post_provisional_records_through_cut
    {
        reasons.push(format!(
            "final-cut authority reviewed-record count disagrees with records: supplied {}, derived {}",
            authority.unreviewed_post_provisional_records_through_cut,
            unreviewed_post_provisional_records_through_cut
        ));
    }
    if authority.final_cut_decisions_remaining != final_cut_decisions_remaining {
        reasons.push(format!(
            "final-cut authority decision count disagrees with records: supplied {}, derived {}",
            authority.final_cut_decisions_remaining, final_cut_decisions_remaining
        ));
    }
    if authority.reviewed_through_selected_cut
        != (unreviewed_post_provisional_records_through_cut == 0)
    {
        reasons.push(
            "final-cut authority reviewed_through_selected_cut disagrees with records".to_string(),
        );
    }
}

fn is_adjudication_review_ref(value: &str) -> bool {
    let Some(prefix) = adjudication_review_prefix(value) else {
        return false;
    };
    is_adjudication_review_slug(&value[prefix.len()..])
}

fn is_adjudication_review_slug(slug: &str) -> bool {
    // Established shape: hyphen-joined non-empty ASCII-alphanumeric segments
    // (review:2832:batch-A-product, review:2825:batch-J-defer), bounded length.
    !slug.is_empty()
        && slug.len() <= 64
        && slug.split('-').all(|segment| {
            !segment.is_empty() && segment.chars().all(|c| c.is_ascii_alphanumeric())
        })
}

fn adjudication_review_prefix(value: &str) -> Option<&'static str> {
    ADJUDICATION_REVIEW_PREFIXES
        .iter()
        .copied()
        .find(|prefix| value.starts_with(prefix))
}

fn adjudication_issue_tag(review_ref: &str) -> &str {
    review_ref
        .strip_prefix("review:")
        .and_then(|rest| rest.split(':').next())
        .unwrap_or("unknown")
}

fn reconcile_provisional_decision_count(snapshot: &mut Snapshot, reasons: &mut Vec<String>) {
    let Some(cutoff_sha) = snapshot.source.provisional_review_cutoff_sha.as_deref() else {
        return;
    };
    let Some(cutoff_position) = snapshot
        .source
        .range_commits
        .iter()
        .position(|commit_sha| commit_sha == cutoff_sha)
    else {
        return;
    };
    let derived = snapshot
        .records
        .iter()
        .take(cutoff_position + 1)
        .filter(|record| record.release_disposition == "operator_decision_required")
        .count() as u64;
    if let Some(selection) = snapshot.candidate_selection.as_mut() {
        if let Some(supplied) = selection.denominator_decisions_remaining_through_provisional_cutoff
            && supplied != derived
        {
            reasons.push(format!(
                "denominator decisions through provisional cutoff disagree with ledger: supplied {supplied}, derived {derived}"
            ));
        }
        selection.denominator_decisions_remaining_through_provisional_cutoff = Some(derived);
    }
}

fn validate_source(snapshot: &Snapshot, reasons: &mut Vec<String>) {
    if snapshot.schema_version != SCHEMA_VERSION {
        reasons.push(format!(
            "snapshot schema_version must be {SCHEMA_VERSION}, got {}",
            snapshot.schema_version
        ));
    }
    if snapshot.kind != INPUT_KIND {
        reasons.push(format!(
            "snapshot kind must be {INPUT_KIND}, got {}",
            snapshot.kind
        ));
    }
    if snapshot.captured_at.trim().is_empty() {
        reasons.push("snapshot captured_at is missing".to_string());
    }
    let source = &snapshot.source;
    if source.mode != "captured" && source.mode != "live" {
        reasons.push(format!(
            "source mode must be captured or live, got {}",
            source.mode
        ));
    }
    if source.stage != "provisional" && source.stage != "final" {
        reasons.push(format!(
            "source stage must be provisional or final, got {}",
            source.stage
        ));
    }
    if source.freshness != "current" {
        reasons.push(format!(
            "source freshness must be current, got {}",
            source.freshness
        ));
    }
    if !is_sha(&source.historical_base_sha) {
        reasons.push("historical_base_sha must be a 40-character hexadecimal SHA".to_string());
    }
    if let Some(cutoff) = &source.provisional_review_cutoff_sha {
        if !is_sha(cutoff) {
            reasons.push(
                "provisional_review_cutoff_sha must be a 40-character hexadecimal SHA".to_string(),
            );
        } else if !source.range_commits.iter().any(|sha| sha == cutoff) {
            reasons.push(format!(
                "provisional review cutoff {cutoff} is not in range_commits"
            ));
        }
    }
    if source.candidate_ref.trim().is_empty() {
        reasons.push("candidate_ref is missing".to_string());
    }
    if !is_sha(&source.candidate_sha) {
        reasons.push("candidate_sha must be a 40-character hexadecimal SHA".to_string());
    }
    validate_sha_list("range_commits", &source.range_commits, reasons);
    validate_sha_list(
        "candidate_tree_commits",
        &source.candidate_tree_commits,
        reasons,
    );
    let range_commits = source
        .range_commits
        .iter()
        .map(String::as_str)
        .collect::<BTreeSet<_>>();
    for commit_sha in &source.candidate_tree_commits {
        if !range_commits.contains(commit_sha.as_str()) {
            reasons.push(format!(
                "candidate_tree_commits contains commit {commit_sha} outside range_commits"
            ));
        }
    }
}

fn validate_sha_list(name: &str, values: &[String], reasons: &mut Vec<String>) {
    if values.is_empty() {
        reasons.push(format!("{name} must not be empty"));
    }
    let mut seen = BTreeSet::new();
    for value in values {
        if !is_sha(value) {
            reasons.push(format!("{name} contains invalid SHA {value}"));
        }
        if !seen.insert(value) {
            reasons.push(format!("{name} contains duplicate SHA {value}"));
        }
    }
}

fn validate_record(record: &CommitRecord, source: &SnapshotSource, reasons: &mut Vec<String>) {
    if !is_sha(&record.commit_sha) {
        reasons.push(format!(
            "record {} has an invalid commit SHA",
            record.commit_sha
        ));
    }
    for (field, value) in [
        ("subject", &record.subject),
        ("theme_or_surface", &record.theme_or_surface),
        ("acceptance_owner", &record.acceptance_owner),
        (
            "source_survivor_or_swarm_exclusion_effect",
            &record.source_survivor_or_swarm_exclusion_effect,
        ),
        (
            "limitation_or_operator_decision",
            &record.limitation_or_operator_decision,
        ),
    ] {
        if value.trim().is_empty() {
            reasons.push(format!("commit {} has empty {field}", record.commit_sha));
        }
    }
    if !DISPOSITIONS.contains(&record.release_disposition.as_str()) {
        reasons.push(format!(
            "commit {} has invalid release_disposition {}",
            record.commit_sha, record.release_disposition
        ));
    }
    if !TREE_STATES.contains(&record.candidate_tree_state.as_str()) {
        reasons.push(format!(
            "commit {} has invalid candidate_tree_state {}",
            record.commit_sha, record.candidate_tree_state
        ));
        return;
    }
    let in_candidate = source
        .candidate_tree_commits
        .iter()
        .any(|sha| sha == &record.commit_sha);
    match record.candidate_tree_state.as_str() {
        "present_in_candidate" if !in_candidate => reasons.push(format!(
            "commit {} is marked present_in_candidate but is absent from candidate tree",
            record.commit_sha
        )),
        "absent_by_candidate_only_exclusion" if in_candidate => reasons.push(format!(
            "commit {} is marked candidate-only excluded but remains in candidate tree",
            record.commit_sha
        )),
        "source_only_not_in_swarm_candidate" if in_candidate => reasons.push(format!(
            "commit {} is marked source-only but remains in candidate tree",
            record.commit_sha
        )),
        "replaced_by_later_commit" => {
            if in_candidate {
                reasons.push(format!(
                    "commit {} is marked replaced but remains in candidate tree",
                    record.commit_sha
                ));
            }
            if record
                .replacement_commit_sha
                .as_deref()
                .is_none_or(|sha| !source.candidate_tree_commits.iter().any(|item| item == sha))
            {
                reasons.push(format!(
                    "commit {} replacement is missing from candidate tree",
                    record.commit_sha
                ));
            }
        }
        _ => {}
    }
    let valid_candidate_only_exclusion = match record.candidate_tree_state.as_str() {
        "absent_by_candidate_only_exclusion" => true,
        "present_in_candidate" => {
            !record.candidate_only_excluded_paths.is_empty()
                && record.candidate_only_exclusion_granularity.as_deref()
                    == Some(EXECUTION_EXCLUSION_GRANULARITY)
        }
        _ => false,
    };
    if record.release_disposition != "candidate_only_exclusion"
        && (!record.candidate_only_excluded_paths.is_empty()
            || record.candidate_only_exclusion_granularity.is_some())
    {
        reasons.push(format!(
            "commit {} has candidate_only_excluded_paths without candidate_only_exclusion disposition",
            record.commit_sha
        ));
    }
    if record.release_disposition == "candidate_only_exclusion" && !valid_candidate_only_exclusion {
        reasons.push(format!(
            "commit {} candidate_only_exclusion disposition has the wrong tree state",
            record.commit_sha
        ));
    }
    if record.release_disposition == "source_only_followup"
        && record.candidate_tree_state != "source_only_not_in_swarm_candidate"
    {
        reasons.push(format!(
            "commit {} source_only_followup disposition has the wrong tree state",
            record.commit_sha
        ));
    }
    if record.candidate_tree_state == "candidate_tree_state_pending"
        && record.release_disposition != "operator_decision_required"
    {
        reasons.push(format!(
            "commit {} pending candidate-tree state requires operator_decision_required",
            record.commit_sha
        ));
    }
    validate_reference_evidence(record, source, reasons);
}

fn validate_capture_metadata(record: &CommitRecord, reasons: &mut Vec<String>) {
    if !CAPTURE_STATUSES.contains(&record.reference_capture_status.as_str()) {
        reasons.push(format!(
            "commit {} has invalid reference_capture_status {}",
            record.commit_sha, record.reference_capture_status
        ));
    }
    match record.reference_capture_status.as_str() {
        "captured" if record.references.is_empty() => reasons.push(format!(
            "commit {} is marked captured without typed reference evidence",
            record.commit_sha
        )),
        "no_linked_authority" if !record.references.is_empty() => reasons.push(format!(
            "commit {} has references despite no_linked_authority capture status",
            record.commit_sha
        )),
        "ambiguous" | "unavailable" if record.reference_capture_limitation.trim().is_empty() => {
            reasons.push(format!(
                "commit {} {} capture status must state its limitation",
                record.commit_sha, record.reference_capture_status
            ));
        }
        _ => {}
    }
}

fn validate_claim_refs(
    record: &CommitRecord,
    selection: Option<&CandidateSelection>,
    reasons: &mut Vec<String>,
) {
    let mut seen = BTreeSet::new();
    for claim_ref in &record.claim_refs {
        if claim_ref.trim().is_empty() {
            reasons.push(format!(
                "commit {} has a blank claim_ref",
                record.commit_sha
            ));
        }
        if !seen.insert(claim_ref) {
            reasons.push(format!(
                "commit {} repeats claim_ref {}",
                record.commit_sha, claim_ref
            ));
        }
    }
    let Some(selection) = selection else {
        if !record.claim_refs.is_empty() {
            reasons.push(format!(
                "commit {} has claim_refs but the selected claim authority is absent",
                record.commit_sha
            ));
        }
        return;
    };
    let authority = evaluate_candidate_selection(Some(selection));
    if authority.status == "scope_pending" && !record.claim_refs.is_empty() {
        reasons.push(format!(
            "commit {} claim_refs require structurally valid selected claim authority: {}",
            record.commit_sha,
            authority.reasons.join("; ")
        ));
        return;
    }
    let selected_claim_ids = selection
        .selected_claims
        .iter()
        .map(|claim| claim.claim_id.as_str())
        .collect::<BTreeSet<_>>();
    for claim_ref in &record.claim_refs {
        if !selected_claim_ids.contains(claim_ref.as_str()) {
            reasons.push(format!(
                "commit {} claim_ref {} is absent from selected claim authority",
                record.commit_sha, claim_ref
            ));
        }
    }
}

fn normalize_reference_projections(record: &mut CommitRecord, reasons: &mut Vec<String>) {
    if record.references.is_empty() {
        return;
    }
    record.references.sort_by(|left, right| {
        (
            left.kind.as_str(),
            left.number,
            left.source.as_str(),
            left.evidence_url.as_deref().unwrap_or_default(),
            left.github_identity.as_deref().unwrap_or_default(),
            left.observed_for_commit_sha.as_str(),
            left.reviewed,
            left.limitation.as_str(),
        )
            .cmp(&(
                right.kind.as_str(),
                right.number,
                right.source.as_str(),
                right.evidence_url.as_deref().unwrap_or_default(),
                right.github_identity.as_deref().unwrap_or_default(),
                right.observed_for_commit_sha.as_str(),
                right.reviewed,
                right.limitation.as_str(),
            ))
    });
    let mut projected_pr_refs = record
        .references
        .iter()
        .filter(|reference| reference.kind == "merge_pr" || reference.kind == "pull_request")
        .map(|reference| reference.number)
        .collect::<Vec<_>>();
    projected_pr_refs.sort();
    let mut projected_issue_refs = record
        .references
        .iter()
        .filter(|reference| reference.kind == "issue")
        .map(|reference| reference.number)
        .collect::<Vec<_>>();
    projected_issue_refs.sort();
    if record.pr_refs.is_empty() {
        record.pr_refs = projected_pr_refs;
    } else if record.pr_refs != projected_pr_refs {
        reasons.push(format!(
            "commit {} pr_refs compatibility projection disagrees with references",
            record.commit_sha
        ));
    }
    if record.issue_refs.is_empty() {
        record.issue_refs = projected_issue_refs;
    } else if record.issue_refs != projected_issue_refs {
        reasons.push(format!(
            "commit {} issue_refs compatibility projection disagrees with references",
            record.commit_sha
        ));
    }
}

fn validate_reference_evidence(
    record: &CommitRecord,
    source: &SnapshotSource,
    reasons: &mut Vec<String>,
) {
    let mut seen_authorities = BTreeSet::new();
    let mut evidence_identities = BTreeMap::new();
    for reference in &record.references {
        if !REFERENCE_KINDS.contains(&reference.kind.as_str()) {
            reasons.push(format!(
                "commit {} has invalid reference kind {}",
                record.commit_sha, reference.kind
            ));
        }
        if !REFERENCE_SOURCES.contains(&reference.source.as_str()) {
            reasons.push(format!(
                "commit {} has invalid reference source {}",
                record.commit_sha, reference.source
            ));
        }
        if reference.number == 0 {
            reasons.push(format!(
                "commit {} has a zero reference number",
                record.commit_sha
            ));
        }
        if reference.observed_for_commit_sha != record.commit_sha {
            reasons.push(format!(
                "commit {} reference observation binds to {}",
                record.commit_sha, reference.observed_for_commit_sha
            ));
        }
        let evidence_url = reference
            .evidence_url
            .as_deref()
            .filter(|value| !value.trim().is_empty());
        let github_identity = reference
            .github_identity
            .as_deref()
            .filter(|value| !value.trim().is_empty());
        if usize::from(evidence_url.is_some()) + usize::from(github_identity.is_some()) != 1 {
            reasons.push(format!(
                "commit {} reference {} must carry exactly one evidence_url or github_identity",
                record.commit_sha, reference.number
            ));
        }
        if !reference.reviewed && reference.limitation.trim().is_empty() {
            reasons.push(format!(
                "commit {} unreviewed reference {} must state its limitation",
                record.commit_sha, reference.number
            ));
        }
        if reference.kind == "reviewed_manual_mapping" && reference.limitation.trim().is_empty() {
            reasons.push(format!(
                "commit {} reviewed_manual_mapping {} must state its reason",
                record.commit_sha, reference.number
            ));
        }
        if source.stage == "final" && !reference.reviewed {
            reasons.push(format!(
                "final ledger retains an unreviewed reference {} for commit {}",
                reference.number, record.commit_sha
            ));
        }
        if reference.kind == "merge_pr" && reference.source != "associated_pull_request" {
            reasons.push(format!(
                "commit {} merge_pr reference {} must use associated_pull_request source",
                record.commit_sha, reference.number
            ));
        }
        if reference.kind == "reviewed_manual_mapping" && reference.source != "explicit_review" {
            reasons.push(format!(
                "commit {} reviewed_manual_mapping {} must use explicit_review source",
                record.commit_sha, reference.number
            ));
        }
        if !seen_authorities.insert((
            reference.kind.clone(),
            reference.number,
            reference.source.clone(),
        )) {
            reasons.push(format!(
                "commit {} contains duplicate reference authority {}:{}:{}",
                record.commit_sha, reference.kind, reference.number, reference.source
            ));
        }
        if let Some(identity) = evidence_url.or(github_identity) {
            let authority = (
                reference.kind.clone(),
                reference.number,
                reference.source.clone(),
            );
            if let Some(previous) =
                evidence_identities.insert(identity.to_string(), authority.clone())
                && previous != authority
            {
                reasons.push(format!(
                    "commit {} evidence identity has contradictory reference claims",
                    record.commit_sha
                ));
            }
        }
    }
    if source.stage == "final"
        && record.references.is_empty()
        && (!record.pr_refs.is_empty() || !record.issue_refs.is_empty())
    {
        reasons.push(format!(
            "final ledger commit {} relies on legacy reference projections without authority evidence",
            record.commit_sha
        ));
    }
}

fn collect_live_facts(source: &SnapshotSource) -> LiveFacts {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| Path::new(".").to_path_buf());
    let mut errors = Vec::new();
    let candidate_sha = match live_output(
        &root,
        &[
            "rev-parse",
            "--verify",
            &format!("{}^{{commit}}", source.candidate_ref),
        ],
        "candidate ref collection",
    ) {
        Ok(value) => Some(value.trim().to_string()),
        Err(error) => {
            errors.push(error);
            None
        }
    };
    let range_commits = match live_output(
        &root,
        &[
            "rev-list",
            "--first-parent",
            "--reverse",
            &format!("{}..{}", source.historical_base_sha, source.candidate_ref),
        ],
        "candidate range collection",
    ) {
        Ok(value) => value
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty())
            .map(str::to_string)
            .collect(),
        Err(error) => {
            errors.push(error);
            Vec::new()
        }
    };
    let candidate_tree_commits = match candidate_sha.as_deref() {
        Some(candidate_sha) => match live_output(
            &root,
            &[
                "rev-list",
                "--first-parent",
                "--reverse",
                &format!("{}..{candidate_sha}", source.historical_base_sha),
            ],
            "candidate tree collection",
        ) {
            Ok(value) => value
                .lines()
                .map(str::trim)
                .filter(|line| !line.is_empty())
                .map(str::to_string)
                .collect(),
            Err(error) => {
                errors.push(error);
                Vec::new()
            }
        },
        None => Vec::new(),
    };
    LiveFacts {
        candidate_sha,
        candidate_tree_commits,
        range_commits,
        errors,
    }
}

fn live_output(root: &Path, args: &[&str], context: &str) -> Result<String, String> {
    let mut owned_args = vec!["-C".to_string(), root.display().to_string()];
    owned_args.extend(args.iter().map(|arg| (*arg).to_string()));
    let output =
        crate::run::capture_output_with_timeout("git", &owned_args, &[], LIVE_TIMEOUT, context)?;
    if output.timed_out {
        return Err(format!(
            "{context} timed out after {} seconds",
            LIVE_TIMEOUT.as_secs()
        ));
    }
    let status = output
        .status
        .ok_or_else(|| format!("{context} did not report a process status"))?;
    if status.success() {
        Ok(output.stdout)
    } else {
        Err(format!(
            "{context} failed with {status}: {}",
            output.stderr.trim()
        ))
    }
}

fn report_json(report: &NormalizedReport) -> Value {
    json!({
        "report": REPORT_NAME,
        "schema_version": SCHEMA_VERSION,
        "status": report.status,
        "captured_at": report.captured_at,
        "source": report.source,
        "candidate_selection": report.candidate_selection,
        "range_digest": report.range_digest,
        "candidate_tree_digest": report.candidate_tree_digest,
        "record_set_digest": report.record_set_digest,
        "counts_by_disposition": report.counts_by_disposition,
        "counts_by_tree_state": report.counts_by_tree_state,
        "records": report.records,
        "reconciliation_reasons": report.reconciliation_reasons,
        "next_action": report.next_action,
        "authority_boundary": "supplemental_denominator_only",
        "must_not_claim": [
            "candidate qualification",
            "merge approval",
            "release publication",
            "source integration",
        ],
    })
}

fn report_markdown(report: &NormalizedReport) -> String {
    let mut markdown = format!(
        "# Supplemental release denominator\n\n- Status: `{}`\n- Stage: `{}`\n- Historical base: `{}`\n- Provisional review cutoff: `{}`\n- Candidate: `{}` (`{}`)\n- Range digest: `{}`\n- Candidate-tree digest: `{}`\n- Record-set digest: `{}`\n\n",
        report.status,
        report.source.stage,
        report.source.historical_base_sha,
        report
            .source
            .provisional_review_cutoff_sha
            .as_deref()
            .unwrap_or("not selected"),
        report.source.candidate_ref,
        report.source.candidate_sha,
        report.range_digest,
        report.candidate_tree_digest,
        report.record_set_digest,
    );
    markdown.push_str("## Counts\n\n| Disposition | Count |\n| --- | ---: |\n");
    for (name, count) in &report.counts_by_disposition {
        markdown.push_str(&format!("| `{}` | {} |\n", escape_cell(name), count));
    }
    markdown.push_str("\n## Records\n\n| Position | Commit | Disposition | Candidate tree | Subject |\n| ---: | --- | --- | --- | --- |\n");
    for record in &report.records {
        markdown.push_str(&format!(
            "| {} | `{}` | `{}` | `{}` | {} |\n",
            record.first_parent_position,
            record.commit_sha,
            escape_cell(&record.release_disposition),
            escape_cell(&record.candidate_tree_state),
            escape_cell(&record.subject),
        ));
    }
    markdown.push_str("\n## Reconciliation\n\n");
    if report.reconciliation_reasons.is_empty() {
        markdown.push_str(
            "The captured range, record order, dispositions, and candidate tree reconcile.\n\n",
        );
    } else {
        for reason in &report.reconciliation_reasons {
            markdown.push_str(&format!("- {}\n", escape_cell(reason)));
        }
        markdown.push('\n');
    }
    markdown.push_str("## Next action\n\n");
    markdown.push_str(&report.next_action);
    markdown.push_str("\n\n## Must not claim\n\n- candidate qualification\n- merge approval\n- release publication\n- source integration\n");
    markdown
}

fn escape_cell(value: &str) -> String {
    value.replace('|', "\\|").replace(['\n', '\r'], " ")
}

fn digest_json<T: Serialize>(value: &T) -> Result<String, String> {
    let bytes =
        serde_json::to_vec(value).map_err(|error| format!("failed to hash JSON: {error}"))?;
    let digest = Sha256::digest(bytes);
    Ok(format!(
        "sha256:{}",
        digest
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>()
    ))
}

fn is_sha(value: &str) -> bool {
    value.len() == 40 && value.chars().all(|character| character.is_ascii_hexdigit())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn fixture() -> Result<Snapshot, String> {
        serde_json::from_str(include_str!(
            "../../../fixtures/release_denominator/complete.json"
        ))
        .map_err(|error| error.to_string())
    }

    fn require(condition: bool, message: impl Into<String>) -> Result<(), String> {
        condition.then_some(()).ok_or(message.into())
    }

    #[test]
    fn complete_provisional_ledger_is_ready_and_digest_stable() -> Result<(), String> {
        let report = normalize_snapshot(fixture()?, None)?;
        require(
            report.status == "ready",
            format!("unexpected reasons: {:?}", report.reconciliation_reasons),
        )?;
        require(
            report.records.len() == 4,
            "fixture denominator is incomplete",
        )?;
        require(
            report.source.historical_base_sha == "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            "historical base identity changed",
        )?;
        require(
            report.source.candidate_sha == "eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee",
            "candidate identity changed",
        )?;
        require(
            report.source.range_commits.first().map(String::as_str)
                == Some("bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb")
                && report.source.range_commits.last().map(String::as_str)
                    == Some("eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee"),
            "ordered range identity changed",
        )?;
        require(
            report.range_digest
                == "sha256:e596e47e5225954058523ad415e398252211b02e99522c614e8dc413fae713a8",
            "ordered range digest changed",
        )?;
        require(
            report.candidate_tree_digest
                == "sha256:e596e47e5225954058523ad415e398252211b02e99522c614e8dc413fae713a8",
            "candidate-tree digest changed",
        )?;
        require(
            report.record_set_digest
                == "sha256:10aad6896d106a06d6e89053adb8bfb095ee94d0ce4c8b4cfa7d66007b3cd31c",
            "normalized record-set digest changed",
        )?;
        let again = normalize_snapshot(fixture()?, None)?;
        require(
            report.record_set_digest == again.record_set_digest,
            "record digest is not stable",
        )
    }

    #[test]
    fn reference_authority_pins_known_merge_and_issue_pair() -> Result<(), String> {
        let report = normalize_snapshot(fixture()?, None)?;
        let first = report
            .records
            .first()
            .ok_or_else(|| "complete fixture has no first record".to_string())?;
        require(
            first.references.iter().any(|reference| {
                reference.kind == "merge_pr"
                    && reference.number == 2788
                    && reference.source == "associated_pull_request"
            }),
            "known merge PR authority is missing",
        )?;
        require(
            first.references.iter().any(|reference| {
                reference.kind == "issue"
                    && reference.number == 2767
                    && reference.source == "closing_reference"
            }),
            "known issue authority is missing",
        )?;
        require(
            first.pr_refs == [2788] && first.issue_refs == [2767],
            "compatibility projections do not match retained references",
        )
    }

    #[test]
    fn body_reference_remains_distinct_from_merge_pr_authority() -> Result<(), String> {
        let report = normalize_snapshot(fixture()?, None)?;
        let second = report
            .records
            .get(1)
            .ok_or_else(|| "complete fixture has no second record".to_string())?;
        require(
            second.references.iter().any(|reference| {
                reference.kind == "pull_request"
                    && reference.number == 2791
                    && reference.source == "body_reference"
            }),
            "non-merge body PR authority is missing",
        )?;
        require(
            second.references.iter().any(|reference| {
                reference.kind == "merge_pr"
                    && reference.number == 2790
                    && reference.source == "associated_pull_request"
            }),
            "second known merge PR authority is missing",
        )?;
        require(
            second.references.iter().any(|reference| {
                reference.kind == "issue"
                    && reference.number == 2768
                    && reference.source == "closing_reference"
            }),
            "second known issue authority is missing",
        )?;
        require(
            second.pr_refs == [2790, 2791] && second.issue_refs == [2768],
            "PR or issue compatibility projection lost retained references",
        )
    }

    #[test]
    fn no_linked_issue_record_can_remain_explicitly_empty() -> Result<(), String> {
        let report = normalize_snapshot(fixture()?, None)?;
        let third = report
            .records
            .get(2)
            .ok_or_else(|| "complete fixture has no third record".to_string())?;
        require(
            third.references.is_empty() && third.pr_refs.is_empty() && third.issue_refs.is_empty(),
            "no-linked-issue record gained fabricated reference authority",
        )
    }

    #[test]
    fn reviewed_manual_mapping_is_explicit_and_replayable() -> Result<(), String> {
        let mut snapshot = fixture()?;
        let observed_for_commit_sha = snapshot.records[2].commit_sha.clone();
        snapshot.records[2].references.push(ReferenceEvidence {
            kind: "reviewed_manual_mapping".to_string(),
            number: 1704,
            source: "explicit_review".to_string(),
            evidence_url: None,
            github_identity: Some("review:release-authority-1704".to_string()),
            observed_for_commit_sha,
            reviewed: true,
            limitation: "manual mapping retained for portfolio authority".to_string(),
        });
        let report = normalize_snapshot(snapshot, None)?;
        require(
            report.status == "ready"
                && report.records[2].references[0].kind == "reviewed_manual_mapping",
            "reviewed manual mapping was not retained",
        )
    }

    #[test]
    fn reference_order_is_normalized_and_mapping_changes_digest() -> Result<(), String> {
        let fixture_snapshot = fixture()?;
        let baseline = normalize_snapshot(fixture_snapshot.clone(), None)?;
        let mut reordered = fixture_snapshot.clone();
        reordered.records[0].references.reverse();
        reordered.records[0].pr_refs.clear();
        reordered.records[0].issue_refs.clear();
        let reordered_report = normalize_snapshot(reordered, None)?;
        require(
            baseline.record_set_digest == reordered_report.record_set_digest,
            "reference order changed the normalized digest",
        )?;

        let mut changed = fixture_snapshot;
        changed.records[0].references[0].number = 2789;
        changed.records[0].pr_refs.clear();
        let changed_report = normalize_snapshot(changed, None)?;
        require(
            baseline.record_set_digest != changed_report.record_set_digest,
            "changed reference mapping did not change the record-set digest",
        )
    }

    #[test]
    fn final_ledger_rejects_unreviewed_reference() -> Result<(), String> {
        let mut snapshot = fixture()?;
        snapshot.source.stage = "final".to_string();
        snapshot.records[0].references[0].reviewed = false;
        snapshot.records[0].references[0].limitation = "provider unavailable".to_string();
        let report = normalize_snapshot(snapshot, None)?;
        require(
            report
                .reconciliation_reasons
                .iter()
                .any(|reason| reason.contains("unreviewed reference")),
            "unreviewed final reference was accepted",
        )
    }

    #[test]
    fn final_ledger_rejects_ambiguous_manual_mapping() -> Result<(), String> {
        let mut snapshot = fixture()?;
        snapshot.source.stage = "final".to_string();
        let observed_for_commit_sha = snapshot.records[2].commit_sha.clone();
        snapshot.records[2].references.push(ReferenceEvidence {
            kind: "reviewed_manual_mapping".to_string(),
            number: 1704,
            source: "explicit_review".to_string(),
            evidence_url: None,
            github_identity: Some("review:ambiguous-1704".to_string()),
            observed_for_commit_sha,
            reviewed: false,
            limitation: "ambiguous mapping requires operator review".to_string(),
        });
        let report = normalize_snapshot(snapshot, None)?;
        require(
            report
                .reconciliation_reasons
                .iter()
                .any(|reason| reason.contains("unreviewed reference")),
            "ambiguous final manual mapping was accepted",
        )
    }

    #[test]
    fn final_ledger_rejects_legacy_only_reference_projection() -> Result<(), String> {
        let mut snapshot = fixture()?;
        snapshot.source.stage = "final".to_string();
        snapshot.records[0].references.clear();
        let report = normalize_snapshot(snapshot, None)?;
        require(
            report.reconciliation_reasons.iter().any(|reason| {
                reason.contains("legacy reference projections without authority evidence")
            }),
            "final legacy-only reference projection was accepted",
        )
    }

    #[test]
    fn reference_projection_mismatch_fails_closed() -> Result<(), String> {
        let mut snapshot = fixture()?;
        snapshot.records[0].pr_refs = vec![9999];
        let report = normalize_snapshot(snapshot, None)?;
        require(
            report
                .reconciliation_reasons
                .iter()
                .any(|reason| reason.contains("pr_refs compatibility projection")),
            "contradictory compatibility projection was accepted",
        )
    }

    #[test]
    fn contradictory_reference_identity_fails_closed() -> Result<(), String> {
        let mut snapshot = fixture()?;
        let mut contradictory = snapshot.records[0].references[0].clone();
        contradictory.kind = "issue".to_string();
        contradictory.source = "closing_reference".to_string();
        snapshot.records[0].references.push(contradictory);
        snapshot.records[0].pr_refs.clear();
        snapshot.records[0].issue_refs.clear();
        let report = normalize_snapshot(snapshot, None)?;
        require(
            report
                .reconciliation_reasons
                .iter()
                .any(|reason| reason.contains("contradictory reference claims")),
            "contradictory reference identity was accepted",
        )
    }

    #[test]
    fn reused_reference_identity_for_different_claim_fails_closed() -> Result<(), String> {
        let mut snapshot = fixture()?;
        snapshot.source.stage = "final".to_string();
        let mut reused = snapshot.records[0].references[0].clone();
        reused.number = 2789;
        snapshot.records[0].references.push(reused);
        snapshot.records[0].pr_refs.clear();
        let report = normalize_snapshot(snapshot, None)?;
        require(
            report
                .reconciliation_reasons
                .iter()
                .any(|reason| reason.contains("contradictory reference claims")),
            "reused reference identity was accepted for a different claim",
        )
    }

    #[test]
    fn malformed_reference_evidence_fails_closed() -> Result<(), String> {
        let mut snapshot = fixture()?;
        snapshot.source.stage = "final".to_string();
        let observed_for_commit_sha = snapshot.records[0].commit_sha.clone();
        let malformed = ReferenceEvidence {
            kind: "unknown".to_string(),
            number: 0,
            source: "unknown".to_string(),
            evidence_url: None,
            github_identity: None,
            observed_for_commit_sha: "ffffffffffffffffffffffffffffffffffffffff".to_string(),
            reviewed: false,
            limitation: String::new(),
        };
        let mut merge_with_wrong_source = malformed.clone();
        merge_with_wrong_source.kind = "merge_pr".to_string();
        merge_with_wrong_source.number = 1801;
        merge_with_wrong_source.source = "body_reference".to_string();
        merge_with_wrong_source.evidence_url =
            Some("https://github.com/EffortlessMetrics/ripr-swarm/pull/1801".to_string());
        merge_with_wrong_source.observed_for_commit_sha = observed_for_commit_sha.clone();
        merge_with_wrong_source.reviewed = true;
        let mut manual_with_wrong_source = merge_with_wrong_source.clone();
        manual_with_wrong_source.kind = "reviewed_manual_mapping".to_string();
        manual_with_wrong_source.number = 1802;
        manual_with_wrong_source.evidence_url =
            Some("https://github.com/EffortlessMetrics/ripr-swarm/pull/1802".to_string());
        manual_with_wrong_source.source = "body_reference".to_string();
        let duplicate = manual_with_wrong_source.clone();
        let mut same_identity_same_kind = manual_with_wrong_source.clone();
        same_identity_same_kind.number = 1803;
        same_identity_same_kind.source = "explicit_review".to_string();
        same_identity_same_kind.evidence_url = duplicate.evidence_url.clone();
        snapshot.records[0].references = vec![
            malformed,
            merge_with_wrong_source,
            manual_with_wrong_source,
            duplicate,
            same_identity_same_kind,
        ];
        snapshot.records[0].pr_refs.clear();
        snapshot.records[0].issue_refs = vec![9999];
        let report = normalize_snapshot(snapshot, None)?;
        let reasons = report.reconciliation_reasons.join("\n");
        for expected in [
            "invalid reference kind",
            "invalid reference source",
            "zero reference number",
            "reference observation binds",
            "must carry exactly one evidence_url or github_identity",
            "unreviewed reference",
            "merge_pr reference",
            "reviewed_manual_mapping",
            "duplicate reference authority",
            "issue_refs compatibility projection",
        ] {
            require(
                reasons.contains(expected),
                format!("malformed reference evidence missed {expected}"),
            )?;
        }
        require(
            report.status == "reconcile_required",
            "malformed reference evidence was accepted",
        )
    }

    #[test]
    fn compatibility_projections_sort_reference_numbers() -> Result<(), String> {
        let mut snapshot = fixture()?;
        snapshot.records[1].references[1].number = 1000;
        snapshot.records[1].references[1].evidence_url =
            Some("https://github.com/EffortlessMetrics/ripr-swarm/pull/1000".to_string());
        let mut lower_issue = snapshot.records[1].references[2].clone();
        lower_issue.number = 1000;
        lower_issue.evidence_url =
            Some("https://github.com/EffortlessMetrics/ripr-swarm/issues/1000".to_string());
        snapshot.records[1].references.push(lower_issue);
        snapshot.records[1].pr_refs.clear();
        snapshot.records[1].issue_refs.clear();
        let report = normalize_snapshot(snapshot, None)?;
        require(
            report.records[1].pr_refs == [1000, 2790],
            "compatibility PR projection was not sorted numerically",
        )?;
        require(
            report.records[1].issue_refs == [1000, 2768],
            "compatibility issue projection was not sorted numerically",
        )
    }

    #[test]
    fn reviewed_manual_mapping_requires_reason() -> Result<(), String> {
        let mut snapshot = fixture()?;
        let observed_for_commit_sha = snapshot.records[2].commit_sha.clone();
        snapshot.records[2].references.push(ReferenceEvidence {
            kind: "reviewed_manual_mapping".to_string(),
            number: 1704,
            source: "explicit_review".to_string(),
            evidence_url: None,
            github_identity: Some("review:release-authority-1704".to_string()),
            observed_for_commit_sha,
            reviewed: true,
            limitation: String::new(),
        });
        let report = normalize_snapshot(snapshot, None)?;
        require(
            report.reconciliation_reasons.iter().any(|reason| {
                reason.contains("reviewed_manual_mapping") && reason.contains("reason")
            }),
            "reviewed manual mapping without a reason was accepted",
        )
    }

    #[test]
    fn capture_status_requires_typed_reference_evidence() -> Result<(), String> {
        let mut snapshot = fixture()?;
        snapshot.records[0].reference_capture_status = "captured".to_string();
        snapshot.records[0].references.clear();
        let report = normalize_snapshot(snapshot, None)?;
        require(
            report
                .reconciliation_reasons
                .iter()
                .any(|reason| reason.contains("marked captured without typed reference evidence")),
            "captured status without references was accepted",
        )
    }

    #[test]
    fn pending_cutoff_rows_are_not_candidate_exclusions() -> Result<(), String> {
        let mut snapshot = fixture()?;
        snapshot.source.provisional_review_cutoff_sha =
            Some(snapshot.records[1].commit_sha.clone());
        apply_provisional_cutoff_boundary(&mut snapshot)?;
        require(
            snapshot.records[0].candidate_tree_state == "present_in_candidate",
            "cutoff predecessor changed unexpectedly",
        )?;
        require(
            snapshot.records[2].release_disposition == "operator_decision_required"
                && snapshot.records[2].candidate_tree_state == "candidate_tree_state_pending",
            "post-cutoff row remained implicitly excluded",
        )
    }

    #[test]
    fn claim_refs_require_selected_claim_authority() -> Result<(), String> {
        let mut snapshot = fixture()?;
        snapshot.records[0].claim_refs = vec!["claim:lifecycle".to_string()];
        let report = normalize_snapshot(snapshot, None)?;
        require(
            report.reconciliation_reasons.iter().any(|reason| {
                reason.contains("claim_refs but the selected claim authority is absent")
            }),
            "claim reference was accepted without selected claim authority",
        )
    }

    #[test]
    fn claim_refs_reject_structurally_invalid_selected_authority() -> Result<(), String> {
        let mut snapshot = fixture()?;
        snapshot.candidate_selection = Some(
            serde_json::from_value(json!({
                "schema_version": "0.1",
                "selected_claims": [{
                    "claim_id": "claim:lifecycle",
                    "owner_issue": 0,
                    "required_for_candidate": true,
                    "resolution": "pending",
                    "candidate_effect": "",
                    "reviewed": false
                }]
            }))
            .map_err(|error| error.to_string())?,
        );
        snapshot.records[0].claim_refs = vec!["claim:lifecycle".to_string()];
        let report = normalize_snapshot(snapshot, None)?;
        require(
            report.reconciliation_reasons.iter().any(|reason| {
                reason.contains("claim_refs require structurally valid selected claim authority")
            }),
            "structurally invalid selected claim authority was accepted",
        )
    }

    #[test]
    fn github_body_reference_parser_handles_unicode_context() -> Result<(), String> {
        require(
            body_issue_or_pr_references("Decision — Closes #2393.")
                == vec![(2393, "closing_reference")],
            "Unicode PR body context was not parsed safely",
        )
    }

    #[test]
    fn github_body_reference_parser_requires_explicit_closing_syntax() -> Result<(), String> {
        require(
            body_issue_or_pr_references("Add fixture #123") == vec![(123, "body_reference")],
            "ordinary fixture reference was misclassified as closing",
        )?;
        require(
            body_issue_or_pr_references("Does not close #123") == vec![(123, "body_reference")],
            "negated closing reference was misclassified",
        )?;
        require(
            body_issue_or_pr_references("Fixes #123") == vec![(123, "closing_reference")],
            "explicit closing reference was not recognized",
        )
    }

    #[test]
    fn github_capture_repository_must_match_pinned_source() -> Result<(), String> {
        require(
            validate_capture_repository(Some("EffortlessMetrics/ripr-swarm"), "attacker/fork")
                .is_err(),
            "capture from a different repository was accepted",
        )?;
        require(
            validate_capture_repository(
                Some("EffortlessMetrics/ripr-swarm"),
                "EffortlessMetrics/ripr-swarm",
            )
            .is_ok(),
            "matching capture repository was rejected",
        )
    }

    #[test]
    fn current_main_provisional_census_is_ready_and_pinned() -> Result<(), String> {
        let snapshot: Snapshot = serde_json::from_str(include_str!(
            "../../../fixtures/release_denominator/current-main-provisional.json"
        ))
        .map_err(|error| error.to_string())?;
        let report = normalize_snapshot(snapshot, None)?;
        require(
            report.status == "ready",
            format!(
                "current-main census did not reconcile: {:?}",
                report.reconciliation_reasons
            ),
        )?;
        require(
            report.records.len() == 333,
            "current-main census record count changed",
        )?;
        require(
            report.source.range_commits.len() == 333,
            "current-main census range count changed",
        )?;
        require(
            report.source.candidate_tree_commits.len() == 325,
            "current-main census candidate-tree count changed",
        )?;
        require(
            report.source.historical_base_sha == "c86807ecdbf359594ef88c0ff38b10b446139dca"
                && report.source.candidate_sha == "b8b1c9ec78b013dfac6dcf929447839132835971"
                && report.source.range_commits.first().map(String::as_str)
                    == Some("fd1eec2ad8145678f0fb494a50bd181d6857b0c7")
                && report.source.range_commits.last().map(String::as_str)
                    == Some("b8b1c9ec78b013dfac6dcf929447839132835971"),
            "current-main identity changed",
        )?;
        require(
            report.range_digest
                == "sha256:857911c214cd10a011ffc54fcf3226b81811a699a6e2364f10c03343c6a969c4"
                && report.candidate_tree_digest
                    == "sha256:025963fa597c4f2c068be06cea7576a43589ac49b34014c68119987f5bb59825",
            "current-main range or candidate-tree digest changed",
        )?;
        require(
            report.record_set_digest
                == "sha256:4d8977b4a50e7697a5632f9e2e127bc83fde05640fddc09356dfc532bb72956a",
            "current-main record-set digest changed",
        )?;
        require(
            !report
                .counts_by_tree_state
                .contains_key("candidate_tree_state_pending")
                && report.counts_by_tree_state.get("present_in_candidate") == Some(&325)
                && report
                    .counts_by_tree_state
                    .get("absent_by_candidate_only_exclusion")
                    == Some(&8)
                && !report
                    .counts_by_disposition
                    .contains_key("operator_decision_required")
                && report.counts_by_disposition.get("safe_defer_post_0_11") == Some(&3),
            "current-main denominator counts changed",
        )?;
        let execution_record = report
            .records
            .iter()
            .find(|record| record.commit_sha == "365f7d61e27fd441e997e6e17e3f3e28859a3964")
            .ok_or_else(|| "current-main census is missing the #2396 record".to_string())?;
        require(
            execution_record.first_parent_position == 81
                && execution_record.release_disposition == "candidate_only_exclusion"
                && execution_record.candidate_tree_state == "present_in_candidate"
                && execution_record
                    .candidate_only_excluded_paths
                    .iter()
                    .any(|path| path == "crates/ripr/src/app/verification_execution.rs"),
            "accepted #2767 path-level exclusion was not retained on the execution record",
        )?;
        require(
            report.counts_by_disposition.get("candidate_only_exclusion") == Some(&6),
            "current-main census does not retain the accepted execution exclusion and the five whole-commit dependency exclusions",
        )?;
        let golden_defect = report
            .candidate_selection
            .as_ref()
            .and_then(|selection| {
                selection.known_candidate_defects.iter().find(|defect| {
                    defect.defect_id == "defect:goldens-check-misses-editor-agent-loop"
                })
            })
            .ok_or_else(|| {
                "current-main census lost the golden-check defect authority".to_string()
            })?;
        require(
            golden_defect.resolved
                && golden_defect.description.contains("present_in_candidate")
                && report.records.iter().any(|record| {
                    record.commit_sha == "c1fbf43274e187edbb8a1b2cd8ba2b6b3620ebcd"
                        && record.candidate_tree_state == "present_in_candidate"
                }),
            "golden-check defect was not closed with its adjudicated fixing commit",
        )?;
        require(
            !report.records.iter().any(|record| {
                record.candidate_tree_state == "candidate_tree_state_pending"
                    || record.release_disposition == "operator_decision_required"
            }),
            "current-main census retains an unresolved row through the development cut",
        )?;
        Ok(())
    }

    #[test]
    fn current_main_provisional_range_matches_record_order() -> Result<(), String> {
        let snapshot: Snapshot = serde_json::from_str(include_str!(
            "../../../fixtures/release_denominator/current-main-provisional.json"
        ))
        .map_err(|error| error.to_string())?;
        let record_range = snapshot
            .records
            .iter()
            .map(|record| record.commit_sha.clone())
            .collect::<Vec<_>>();
        require(
            record_range == snapshot.source.range_commits
                && snapshot.source.range_commits.len() == 333
                && snapshot
                    .source
                    .provisional_review_cutoff_sha
                    .as_deref()
                    .and_then(|cutoff| {
                        snapshot
                            .source
                            .range_commits
                            .iter()
                            .position(|commit| commit == cutoff)
                    })
                    == Some(229)
                && snapshot.source.range_commits.last().map(String::as_str)
                    == Some("b8b1c9ec78b013dfac6dcf929447839132835971"),
            "pinned 333-entry first-parent census does not match record order and cutoff",
        )
    }

    #[test]
    fn final_cut_authority_counts_are_derived_from_commit_sha_records() -> Result<(), String> {
        let mut snapshot: Snapshot = serde_json::from_str(include_str!(
            "../../../fixtures/release_denominator/current-main-provisional.json"
        ))
        .map_err(|error| error.to_string())?;
        {
            let selection = snapshot
                .candidate_selection
                .as_mut()
                .ok_or_else(|| "current-main fixture has no candidate selection".to_string())?;
            selection.selected_cut_sha =
                Some("b8b1c9ec78b013dfac6dcf929447839132835971".to_string());
            selection.final_cut_authority =
                Some(crate::reports::candidate_control::FinalCutAuthority {
                    cut_sha: "b8b1c9ec78b013dfac6dcf929447839132835971".to_string(),
                    record_set_digest: None,
                    provisional_decisions_remaining: 0,
                    unreviewed_post_provisional_records_through_cut: 0,
                    final_cut_decisions_remaining: 0,
                    reviewed_through_selected_cut: true,
                });
        }
        let first_report = normalize_snapshot(snapshot.clone(), None)?;
        snapshot
            .candidate_selection
            .as_mut()
            .ok_or_else(|| "current-main fixture lost candidate selection".to_string())?
            .final_cut_authority
            .as_mut()
            .ok_or_else(|| "current-main fixture lost final-cut authority".to_string())?
            .record_set_digest = Some(first_report.record_set_digest.clone());
        let report = normalize_snapshot(snapshot.clone(), None)?;
        require(
            report.status == "ready",
            format!(
                "record-derived final-cut authority did not reconcile: {:?}",
                report.reconciliation_reasons
            ),
        )?;
        snapshot
            .candidate_selection
            .as_mut()
            .ok_or_else(|| "current-main fixture lost candidate selection".to_string())?
            .final_cut_authority
            .as_mut()
            .ok_or_else(|| "current-main fixture lost final-cut authority".to_string())?
            .final_cut_decisions_remaining = 1;
        let report = normalize_snapshot(snapshot, None)?;
        require(
            report
                .reconciliation_reasons
                .iter()
                .any(|reason| reason.contains("decision count disagrees with records")),
            "hand-authored final-cut zero was not reconciled against records",
        )
    }

    #[test]
    fn final_cut_authority_requires_valid_review_reference() -> Result<(), String> {
        let mut snapshot: Snapshot = serde_json::from_str(include_str!(
            "../../../fixtures/release_denominator/current-main-provisional.json"
        ))
        .map_err(|error| error.to_string())?;
        let cut_sha = snapshot
            .records
            .last()
            .map(|record| record.commit_sha.clone())
            .ok_or_else(|| "current-main fixture has no records".to_string())?;
        // The provisional cutoff stays at P, so the final record is a
        // post-provisional row whose review authority is under test.
        let post_cutoff = snapshot
            .records
            .last_mut()
            .ok_or_else(|| "current-main fixture has no last record".to_string())?;
        post_cutoff.review_refs = vec!["fabricated-review-token".to_string()];
        {
            let selection = snapshot
                .candidate_selection
                .as_mut()
                .ok_or_else(|| "current-main fixture has no candidate selection".to_string())?;
            selection.selected_cut_sha = Some(cut_sha.clone());
            selection.final_cut_authority =
                Some(crate::reports::candidate_control::FinalCutAuthority {
                    cut_sha,
                    record_set_digest: None,
                    provisional_decisions_remaining: 0,
                    unreviewed_post_provisional_records_through_cut: 0,
                    final_cut_decisions_remaining: 0,
                    reviewed_through_selected_cut: true,
                });
        }
        let first_report = normalize_snapshot(snapshot.clone(), None)?;
        snapshot
            .candidate_selection
            .as_mut()
            .ok_or_else(|| "current-main fixture lost candidate selection".to_string())?
            .final_cut_authority
            .as_mut()
            .ok_or_else(|| "current-main fixture lost final-cut authority".to_string())?
            .record_set_digest = Some(first_report.record_set_digest);
        let report = normalize_snapshot(snapshot, None)?;
        require(
            report
                .reconciliation_reasons
                .iter()
                .any(|reason| reason.contains("no valid adjudication review authority")),
            "fabricated review reference was credited as final-cut review",
        )
    }

    #[test]
    fn adjudication_manifest_covers_every_record_through_cutoff() -> Result<(), String> {
        let manifest: AdjudicationManifest = serde_json::from_str(include_str!(
            "../../../fixtures/release_denominator/2832-adjudication.json"
        ))
        .map_err(|error| error.to_string())?;
        let positions = manifest
            .batches
            .iter()
            .flat_map(|batch| batch.positions.iter().copied())
            .collect::<Vec<_>>();
        let unique = positions.iter().copied().collect::<BTreeSet<_>>();
        require(
            manifest.cutoff_sha == "fcbb30a7cf6a37027fa377abafb617632b2e6f57"
                && positions.len() == 230
                && unique.len() == 230
                && unique.first() == Some(&1)
                && unique.last() == Some(&230),
            "#2832 adjudication manifest does not cover the fixed cutoff exactly",
        )?;
        let manifest: AdjudicationManifest = serde_json::from_str(include_str!(
            "../../../fixtures/release_denominator/2825-adjudication.json"
        ))
        .map_err(|error| error.to_string())?;
        let positions = manifest
            .batches
            .iter()
            .flat_map(|batch| batch.positions.iter().copied())
            .collect::<Vec<_>>();
        let unique = positions.iter().copied().collect::<BTreeSet<_>>();
        require(
            manifest.cutoff_sha == "b8b1c9ec78b013dfac6dcf929447839132835971"
                && positions.len() == 333
                && unique.len() == 333
                && unique.first() == Some(&1)
                && unique.last() == Some(&333),
            "#2825 adjudication manifest does not cover the selected development cut exactly",
        )
    }

    #[test]
    fn candidate_tree_projection_preserves_reviewed_post_cutoff_rows() -> Result<(), String> {
        let mut snapshot: Snapshot = serde_json::from_str(include_str!(
            "../../../fixtures/release_denominator/current-main-provisional.json"
        ))
        .map_err(|error| error.to_string())?;
        // The adjudicated census pins the provisional cutoff at P (index 229)
        // and has no pending rows; park two post-provisional rows as pending
        // to reproduce the post-cutoff projection boundary.
        for index in [230, 231] {
            let record = snapshot
                .records
                .get_mut(index)
                .ok_or_else(|| "provisional fixture is missing a post-cutoff row".to_string())?;
            record.release_disposition = "operator_decision_required".to_string();
            record.candidate_tree_state = "candidate_tree_state_pending".to_string();
        }
        let baseline = project_candidate_tree(&snapshot.records, &snapshot.source.range_commits);
        let still_pending = snapshot
            .records
            .get(231)
            .ok_or_else(|| "provisional fixture is missing a second post-cutoff row".to_string())?
            .commit_sha
            .clone();
        let post_cutoff = snapshot
            .records
            .get_mut(230)
            .ok_or_else(|| "provisional fixture is missing post-cutoff row".to_string())?;
        let post_cutoff_sha = post_cutoff.commit_sha.clone();
        post_cutoff.release_disposition = "include_product".to_string();
        post_cutoff.candidate_tree_state = "present_in_candidate".to_string();
        post_cutoff.review_refs = vec!["review:2825:post-cutoff-row".to_string()];

        let projected = project_candidate_tree(&snapshot.records, &snapshot.source.range_commits);
        require(
            projected
                .iter()
                .any(|commit_sha| commit_sha == &post_cutoff_sha),
            "a reviewed post-cutoff candidate-tree row was discarded during projection",
        )?;
        require(
            projected.len() == baseline.len() + 1,
            "projection admitted more than the single reviewed post-cutoff row",
        )?;
        require(
            !projected
                .iter()
                .any(|commit_sha| commit_sha == &still_pending),
            "projection admitted a still-pending post-cutoff row",
        )
    }

    #[test]
    fn adjudication_review_refs_require_a_complete_slug_shape() -> Result<(), String> {
        for accepted in [
            "review:2832:batch-A-product",
            "review:2832:exception-2767",
            "review:2825:batch-E-product",
            "review:2825:batch-J-defer",
        ] {
            require(
                is_adjudication_review_ref(accepted),
                format!("established review ref {accepted} was rejected"),
            )?;
        }
        for rejected in [
            "review:2825:",
            "review:2825:bad value",
            "review:2825:trailing ",
            "review:2825:snake_case",
            "review:2825:dot.slug",
            "review:2825:path/slug",
            "review:2825:-leading-hyphen",
            "review:2825:trailing-hyphen-",
            "review:2825:double--hyphen",
            "review:9999:unknown-issue",
        ] {
            require(
                !is_adjudication_review_ref(rejected),
                format!("malformed review ref {rejected} was accepted"),
            )?;
        }
        let overlong = format!("review:2825:{}", "a".repeat(65));
        require(
            !is_adjudication_review_ref(&overlong),
            "overlong review-ref slug was accepted",
        )
    }

    #[test]
    fn accepted_execution_exclusion_paths_are_pinned() -> Result<(), String> {
        let manifest: AdjudicationManifest = serde_json::from_str(include_str!(
            "../../../fixtures/release_denominator/2832-adjudication.json"
        ))
        .map_err(|error| error.to_string())?;
        let mut override_record =
            manifest.overrides.into_iter().next().ok_or_else(|| {
                "adjudication fixture is missing its exception override".to_string()
            })?;
        override_record.candidate_only_excluded_paths[0] = "unrelated/path".to_string();
        require(
            validate_override_exclusion_paths(&override_record).is_err(),
            "mutated accepted execution exclusion paths were accepted",
        )?;
        override_record.candidate_only_excluded_paths =
            accepted_execution_excluded_paths()?.into_iter().collect();
        override_record.candidate_only_exclusion_granularity = Some("whole_file".to_string());
        require(
            validate_override_exclusion_paths(&override_record).is_err(),
            "whole-file exclusion granularity was accepted",
        )?;
        override_record.candidate_only_excluded_paths.clear();
        override_record.disposition = "include_product".to_string();
        require(
            validate_override_exclusion_paths(&override_record).is_err(),
            "non-exclusion disposition retained exclusion authority",
        )
    }

    #[test]
    fn exclusion_manifest_rejects_unknown_keys_and_blank_paths() -> Result<(), String> {
        let mut value: Value = serde_json::from_str(include_str!(
            "../../../fixtures/release_denominator/2832-adjudication.json"
        ))
        .map_err(|error| error.to_string())?;
        value["unknown_override_key"] = json!(true);
        require(
            serde_json::from_value::<AdjudicationManifest>(value).is_err(),
            "unknown adjudication manifest key was accepted",
        )?;

        let manifest: AdjudicationManifest = serde_json::from_str(include_str!(
            "../../../fixtures/release_denominator/2832-adjudication.json"
        ))
        .map_err(|error| error.to_string())?;
        let mut override_record = manifest
            .overrides
            .into_iter()
            .next()
            .ok_or_else(|| "adjudication fixture has no override".to_string())?;
        override_record.candidate_only_excluded_paths[0] = "  ".to_string();
        require(
            validate_override_exclusion_paths(&override_record).is_err(),
            "blank exclusion path was accepted",
        )
    }

    #[test]
    fn exclusion_override_does_not_leave_replay_residue() -> Result<(), String> {
        let snapshot = fixture()?;
        let manifest: AdjudicationManifest = serde_json::from_str(include_str!(
            "../../../fixtures/release_denominator/2832-adjudication.json"
        ))
        .map_err(|error| error.to_string())?;
        let override_record = manifest
            .overrides
            .first()
            .ok_or_else(|| "adjudication fixture has no override".to_string())?;
        let mut record = snapshot
            .records
            .first()
            .ok_or_else(|| "complete fixture has no record".to_string())?
            .clone();
        apply_candidate_only_override(&mut record, Some(override_record))?;
        require(
            !record.candidate_only_excluded_paths.is_empty()
                && record.candidate_only_exclusion_granularity.as_deref() == Some("hunk_or_symbol"),
            "candidate-only override was not applied",
        )?;
        apply_candidate_only_override(&mut record, None)?;
        require(
            record.candidate_only_excluded_paths.is_empty()
                && record.candidate_only_exclusion_granularity.is_none(),
            "removed candidate-only override left replay residue",
        )
    }

    #[test]
    fn provisional_decision_count_is_derived_from_cutoff_records() -> Result<(), String> {
        let mut snapshot: Snapshot = serde_json::from_str(include_str!(
            "../../../fixtures/release_denominator/current-main-provisional.json"
        ))
        .map_err(|error| error.to_string())?;
        snapshot
            .records
            .first_mut()
            .ok_or_else(|| "provisional fixture has no records".to_string())?
            .release_disposition = "operator_decision_required".to_string();
        let report = normalize_snapshot(snapshot, None)?;
        require(
            report
                .reconciliation_reasons
                .iter()
                .any(|reason| reason.contains("supplied 0, derived 1")),
            "a hand-authored provisional decision count was not reconciled",
        )?;
        require(
            report.candidate_selection.as_ref().and_then(|selection| {
                selection.denominator_decisions_remaining_through_provisional_cutoff
            }) == Some(1),
            "the validated provisional decision count did not use the derived value",
        )
    }

    #[test]
    fn missing_record_fails_closed() -> Result<(), String> {
        let mut snapshot = fixture()?;
        snapshot.records.pop();
        let report = normalize_snapshot(snapshot, None)?;
        require(
            report.status == "reconcile_required",
            "missing record was accepted",
        )?;
        require(
            report
                .reconciliation_reasons
                .iter()
                .any(|reason| reason.contains("record count")),
            "missing count reason absent",
        )
    }

    #[test]
    fn duplicate_record_fails_closed() -> Result<(), String> {
        let mut snapshot = fixture()?;
        let duplicate = snapshot.records[1].clone();
        snapshot.records[2] = duplicate;
        let report = normalize_snapshot(snapshot, None)?;
        require(
            report
                .reconciliation_reasons
                .iter()
                .any(|reason| reason.contains("appears more than once")),
            "duplicate reason absent",
        )
    }

    #[test]
    fn out_of_range_record_fails_closed() -> Result<(), String> {
        let mut snapshot = fixture()?;
        snapshot.records[0].commit_sha = "ffffffffffffffffffffffffffffffffffffffff".to_string();
        let report = normalize_snapshot(snapshot, None)?;
        require(
            report
                .reconciliation_reasons
                .iter()
                .any(|reason| reason.contains("outside the captured range")),
            "out-of-range reason absent",
        )
    }

    #[test]
    fn wrong_order_fails_closed() -> Result<(), String> {
        let mut snapshot = fixture()?;
        snapshot.records.swap(0, 1);
        let report = normalize_snapshot(snapshot, None)?;
        require(
            report
                .reconciliation_reasons
                .iter()
                .any(|reason| reason.contains("first-parent range order")),
            "wrong-order reason absent",
        )
    }

    #[test]
    fn wrong_tree_fails_closed() -> Result<(), String> {
        let mut snapshot = fixture()?;
        snapshot.source.candidate_tree_commits.pop();
        let report = normalize_snapshot(snapshot, None)?;
        require(
            report
                .reconciliation_reasons
                .iter()
                .any(|reason| reason.contains("present_in_candidate")),
            "wrong-tree reason absent",
        )
    }

    #[test]
    fn candidate_tree_extra_commit_fails_closed() -> Result<(), String> {
        let mut snapshot = fixture()?;
        snapshot
            .source
            .candidate_tree_commits
            .push("ffffffffffffffffffffffffffffffffffffffff".to_string());
        let report = normalize_snapshot(snapshot, None)?;
        require(
            report.status == "reconcile_required",
            "candidate tree accepted a commit outside the captured range",
        )?;
        require(
            report
                .reconciliation_reasons
                .iter()
                .any(|reason| reason.contains("outside range_commits")),
            "candidate-tree range disagreement reason absent",
        )
    }

    #[test]
    fn final_operator_decision_fails_closed() -> Result<(), String> {
        let mut snapshot = fixture()?;
        snapshot.source.stage = "final".to_string();
        snapshot.records[0].release_disposition = "operator_decision_required".to_string();
        let report = normalize_snapshot(snapshot, None)?;
        require(
            report
                .reconciliation_reasons
                .iter()
                .any(|reason| reason.contains("final ledger retains")),
            "final operator reason absent",
        )
    }

    #[test]
    fn live_observation_mismatch_fails_closed() -> Result<(), String> {
        let snapshot = fixture()?;
        let facts = LiveFacts {
            candidate_sha: Some("0000000000000000000000000000000000000000".to_string()),
            range_commits: Vec::new(),
            candidate_tree_commits: Vec::new(),
            errors: Vec::new(),
        };
        let report = normalize_snapshot(snapshot, Some(&facts))?;
        require(
            report.status == "reconcile_required",
            "live mismatch was accepted",
        )
    }

    #[test]
    fn report_json_and_markdown_share_the_claim_boundary() -> Result<(), String> {
        let report = normalize_snapshot(fixture()?, None)?;
        let json = report_json(&report);
        require(
            json["authority_boundary"] == "supplemental_denominator_only",
            "JSON boundary changed",
        )?;
        let markdown = report_markdown(&report);
        require(
            markdown.contains("## Must not claim") && markdown.contains("candidate qualification"),
            "Markdown boundary missing",
        )?;
        let _ = json!(report.counts_by_tree_state);
        Ok(())
    }
}
