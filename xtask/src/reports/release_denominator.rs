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
    snapshot: Snapshot,
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
    for (index, record) in snapshot.records.iter().enumerate() {
        if !seen.insert(record.commit_sha.as_str()) {
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
            report.record_set_digest.starts_with("sha256:"),
            "record digest missing",
        )?;
        let again = normalize_snapshot(fixture()?, None)?;
        require(
            report.record_set_digest == again.record_set_digest,
            "record digest is not stable",
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
