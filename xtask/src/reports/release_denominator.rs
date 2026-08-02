//! Deterministic supplemental-denominator ledger for the temporary 0.11 release.
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

const REPORT_NAME: &str = "release-denominator";
const SCHEMA_VERSION: &str = "0.1";
const INPUT_KIND: &str = "release_denominator_snapshot";
const JSON_FILE: &str = "release-denominator.json";
const MARKDOWN_FILE: &str = "release-denominator.md";
const LIVE_TIMEOUT: Duration = Duration::from_secs(30);

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
    records: Vec<CommitRecord>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
struct SnapshotSource {
    mode: String,
    stage: String,
    freshness: String,
    historical_base_sha: String,
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
    theme_or_surface: String,
    release_disposition: String,
    acceptance_owner: String,
    candidate_tree_state: String,
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
    let (live, path) = parse_args(args)?;
    let snapshot = read_snapshot(path)?;
    let live_facts = live.then(|| collect_live_facts(&snapshot.source));
    let report = normalize_snapshot(snapshot, live_facts.as_ref())?;
    let json_text = serde_json::to_string_pretty(&report_json(&report))
        .map_err(|error| format!("failed to serialize release-denominator JSON: {error}"))?;
    crate::write_report(JSON_FILE, &format!("{json_text}\n"))?;
    crate::write_report(MARKDOWN_FILE, &report_markdown(&report))?;
    println!("Wrote target/ripr/reports/{JSON_FILE}");
    println!("Wrote target/ripr/reports/{MARKDOWN_FILE}");
    Ok(())
}

fn parse_args(args: &[String]) -> Result<(bool, &str), String> {
    let usage = "usage: cargo xtask release-denominator [--live] --input <ledger.json>";
    match args {
        [input, path] if input == "--input" && !path.trim().is_empty() => Ok((false, path)),
        [live, input, path]
            if live == "--live" && input == "--input" && !path.trim().is_empty() =>
        {
            Ok((true, path))
        }
        [help] if help == "--help" || help == "-h" => Err(usage.to_string()),
        _ => Err(usage.to_string()),
    }
}

fn read_snapshot(path: &str) -> Result<Snapshot, String> {
    let text = fs::read_to_string(path)
        .map_err(|error| format!("failed to read release-denominator input {path}: {error}"))?;
    serde_json::from_str(&text)
        .map_err(|error| format!("failed to parse release-denominator input {path}: {error}"))
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
    let record_set_digest = digest_json(&snapshot.records)?;
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
    if record.release_disposition == "candidate_only_exclusion"
        && record.candidate_tree_state != "absent_by_candidate_only_exclusion"
    {
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
    validate_reference_evidence(record, source, reasons);
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
        "# Supplemental release denominator\n\n- Status: `{}`\n- Stage: `{}`\n- Historical base: `{}`\n- Candidate: `{}` (`{}`)\n- Range digest: `{}`\n- Candidate-tree digest: `{}`\n- Record-set digest: `{}`\n\n",
        report.status,
        report.source.stage,
        report.source.historical_base_sha,
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
                == "sha256:5e49670e6b0f8da180ac36538bb8267d02849795595d56b9b29cba0e679ea9d4",
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
            report.records.len() == 224,
            "current-main census record count changed",
        )?;
        require(
            report.source.range_commits.len() == 224,
            "current-main census range count changed",
        )?;
        require(
            report.source.candidate_tree_commits.len() == 219,
            "current-main census candidate-tree count changed",
        )?;
        require(
            report.source.historical_base_sha == "c86807ecdbf359594ef88c0ff38b10b446139dca"
                && report.source.candidate_sha == "5576c5331580413840c5958be4bb2d4e07b197dc"
                && report.source.range_commits.first().map(String::as_str)
                    == Some("fd1eec2ad8145678f0fb494a50bd181d6857b0c7")
                && report.source.range_commits.last().map(String::as_str)
                    == Some("5576c5331580413840c5958be4bb2d4e07b197dc"),
            "current-main identity changed",
        )?;
        require(
            report.range_digest
                == "sha256:e2d1bb7679cb1c554e707907160a657b80ff95f999cbf704fbff8ca7c0e4d75d"
                && report.candidate_tree_digest
                    == "sha256:c1b3675b6b98f609343f35711898e805a6ad27577c8f9b351ae53718b91082ae",
            "current-main range or candidate-tree digest changed",
        )?;
        Ok(())
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
