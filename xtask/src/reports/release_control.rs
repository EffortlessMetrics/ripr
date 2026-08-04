//! Read-only release-selection lens for the temporary 0.11 convergence window.
//!
//! The command deliberately consumes an explicit snapshot. Live collection is
//! kept separate from normalization so a captured board can be replayed in CI
//! without depending on GitHub availability. Missing or contradictory
//! authority input always produces a non-mergeable report.

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
use std::fs;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use super::candidate_control::{
    CandidateSelection, CandidateState, evaluate as evaluate_candidate_state,
};

const REPORT_NAME: &str = "release-control";
const SCHEMA_VERSION: &str = "0.1";
const INPUT_KIND: &str = "release_control_snapshot";
const AUTHORITY_ISSUE: u64 = 2379;
const JSON_FILE: &str = "release-control.json";
const MARKDOWN_FILE: &str = "release-control.md";
const LIVE_COLLECTION_TIMEOUT: Duration = Duration::from_secs(30);
const LIVE_OPEN_PR_LIMIT: usize = 100;

const DISPOSITIONS: &[&str] = &[
    "release_required",
    "release_optional_pending_decision",
    "hold_post_release",
    "blocked_on_named_authority",
];

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
struct Snapshot {
    schema_version: String,
    kind: String,
    captured_at: String,
    source: SnapshotSource,
    #[serde(default)]
    prs: Vec<SnapshotPr>,
    #[serde(default)]
    candidate_selection: Option<CandidateSelection>,
    #[serde(default)]
    collector_errors: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
struct SnapshotSource {
    mode: String,
    freshness: String,
    main_sha: String,
    authority_issue: u64,
    authority_state: String,
    authority_main_sha: String,
    portfolio_state: String,
    open_prs_complete: bool,
    active_claims_complete: bool,
    #[serde(default)]
    worktree_inventory_complete: bool,
    #[serde(default)]
    worktree_count: u64,
    graph_digest: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
struct SnapshotPr {
    number: u64,
    title: String,
    state: String,
    is_draft: bool,
    head_sha: String,
    base_ref: String,
    #[serde(default)]
    linked_issue_refs: Vec<u64>,
    release_disposition: Option<String>,
    disposition_reason: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct NormalizedReport {
    status: String,
    captured_at: String,
    source: SnapshotSource,
    prs: Vec<NormalizedPr>,
    candidate_state: CandidateState,
    reconciliation_reasons: Vec<String>,
    next_action: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct NormalizedPr {
    number: u64,
    title: String,
    is_draft: bool,
    head_sha: String,
    base_ref: String,
    linked_issue_refs: Vec<u64>,
    disposition: String,
    reason: String,
    merge_eligible: bool,
}

pub(crate) fn release_control(args: &[String]) -> Result<(), String> {
    let input = parse_args(args)?;
    let live_collected = matches!(input, ReleaseControlInput::Live);
    let snapshot = match input {
        ReleaseControlInput::Captured(path) => read_snapshot(path)?,
        ReleaseControlInput::Live => capture_live_snapshot(),
    };
    let report = if live_collected {
        normalize_snapshot_with_origin(snapshot, true)
    } else {
        normalize_snapshot(snapshot)
    };
    let json_text = serde_json::to_string_pretty(&report_json(&report))
        .map_err(|err| format!("failed to serialize release-control JSON: {err}"))?;
    crate::write_report(JSON_FILE, &format!("{json_text}\n"))?;
    crate::write_report(MARKDOWN_FILE, &report_markdown(&report))?;
    println!("Wrote target/ripr/reports/{JSON_FILE}");
    println!("Wrote target/ripr/reports/{MARKDOWN_FILE}");
    Ok(())
}

#[derive(Clone, Copy)]
enum ReleaseControlInput<'a> {
    Captured(&'a str),
    Live,
}

fn parse_args(args: &[String]) -> Result<ReleaseControlInput<'_>, String> {
    if args.len() == 1 && matches!(args.first().map(String::as_str), Some("--help" | "-h")) {
        return Err(
            "usage: cargo xtask release-control --input <captured-snapshot.json> | --live"
                .to_string(),
        );
    }
    if args.len() == 1 && args.first().map(String::as_str) == Some("--live") {
        return Ok(ReleaseControlInput::Live);
    }
    if args.len() != 2
        || args.first().map(String::as_str) != Some("--input")
        || args.get(1).is_none_or(|value| value.trim().is_empty())
    {
        return Err(
            "usage: cargo xtask release-control --input <captured-snapshot.json> | --live"
                .to_string(),
        );
    }
    args.get(1)
        .map(String::as_str)
        .map(ReleaseControlInput::Captured)
        .ok_or_else(|| "release-control input path is missing".to_string())
}

fn read_snapshot(path: &str) -> Result<Snapshot, String> {
    let text = fs::read_to_string(path)
        .map_err(|err| format!("failed to read release-control input {path}: {err}"))?;
    serde_json::from_str(&text)
        .map_err(|err| format!("failed to parse release-control input {path} as JSON: {err}"))
}

#[derive(Debug, Deserialize)]
struct LivePr {
    number: u64,
    title: String,
    state: String,
    #[serde(rename = "isDraft")]
    is_draft: bool,
    #[serde(rename = "headRefOid")]
    head_sha: String,
    #[serde(rename = "baseRefName")]
    base_ref: String,
}

fn capture_live_snapshot() -> Snapshot {
    capture_live_snapshot_with(live_output)
}

fn capture_live_snapshot_with<F>(mut output: F) -> Snapshot
where
    F: FnMut(&str, &[&str], &str) -> Result<String, String>,
{
    let mut collector_errors = Vec::new();
    let main_sha = match output("git", &["rev-parse", "origin/main"], "live main collection") {
        Ok(value) => value.trim().to_string(),
        Err(error) => {
            collector_errors.push(format!("live main collection failed: {error}"));
            String::new()
        }
    };
    let mut open_prs_complete = true;
    let prs = match output(
        "gh",
        &[
            "pr",
            "list",
            "--repo",
            "EffortlessMetrics/ripr-swarm",
            "--state",
            "open",
            "--limit",
            "101",
            "--json",
            "number,title,state,isDraft,headRefOid,baseRefName",
        ],
        "live open-PR collection",
    ) {
        Ok(value) => match serde_json::from_str::<Vec<LivePr>>(&value) {
            Ok(rows) => {
                if rows.len() > LIVE_OPEN_PR_LIMIT {
                    open_prs_complete = false;
                    collector_errors.push(format!(
                        "live open-PR collection exceeded the bounded {LIVE_OPEN_PR_LIMIT}-row inventory"
                    ));
                }
                rows.into_iter()
                    .take(LIVE_OPEN_PR_LIMIT)
                    .map(|pr| SnapshotPr {
                        number: pr.number,
                        title: pr.title,
                        state: pr.state.to_ascii_lowercase(),
                        is_draft: pr.is_draft,
                        head_sha: pr.head_sha,
                        base_ref: pr.base_ref,
                        linked_issue_refs: Vec::new(),
                        release_disposition: None,
                        disposition_reason: None,
                    })
                    .collect()
            }
            Err(error) => {
                open_prs_complete = false;
                collector_errors.push(format!(
                    "live open-PR collection was not valid JSON: {error}"
                ));
                Vec::new()
            }
        },
        Err(error) => {
            open_prs_complete = false;
            collector_errors.push(format!("live open-PR collection failed: {error}"));
            Vec::new()
        }
    };
    let authority = match output(
        "gh",
        &[
            "issue",
            "view",
            "2379",
            "--repo",
            "EffortlessMetrics/ripr-swarm",
            "--json",
            "state,body",
        ],
        "live #2379 collection",
    ) {
        Ok(value) => match serde_json::from_str::<Value>(&value) {
            Ok(value) => value,
            Err(error) => {
                collector_errors.push(format!("live #2379 collection was not valid JSON: {error}"));
                Value::Null
            }
        },
        Err(error) => {
            collector_errors.push(format!("live #2379 collection failed: {error}"));
            Value::Null
        }
    };
    let authority_state = authority
        .get("state")
        .and_then(Value::as_str)
        .map(str::to_ascii_lowercase)
        .unwrap_or_else(|| "unknown".to_string());
    let graph_digest = authority
        .get("body")
        .and_then(Value::as_str)
        .map(hash_text)
        .unwrap_or_default();
    let (worktree_inventory_complete, worktree_count) =
        collect_worktree_inventory(&mut collector_errors, &mut output);
    let freshness = if collector_errors.is_empty() {
        "current"
    } else {
        "unknown"
    };
    Snapshot {
        schema_version: SCHEMA_VERSION.to_string(),
        kind: INPUT_KIND.to_string(),
        captured_at: live_timestamp(),
        source: SnapshotSource {
            mode: "live".to_string(),
            freshness: freshness.to_string(),
            main_sha,
            authority_issue: AUTHORITY_ISSUE,
            authority_state,
            authority_main_sha: String::new(),
            portfolio_state: "unknown".to_string(),
            open_prs_complete,
            active_claims_complete: false,
            worktree_inventory_complete,
            worktree_count,
            graph_digest,
        },
        prs,
        candidate_selection: None,
        collector_errors,
    }
}

fn collect_worktree_inventory<F>(errors: &mut Vec<String>, output: &mut F) -> (bool, u64)
where
    F: FnMut(&str, &[&str], &str) -> Result<String, String>,
{
    match output(
        "git",
        &["worktree", "list", "--porcelain"],
        "live worktree collection",
    ) {
        Ok(value) => {
            let count = value
                .lines()
                .filter(|line| line.starts_with("worktree "))
                .count() as u64;
            (true, count)
        }
        Err(error) => {
            errors.push(format!("live worktree collection failed: {error}"));
            (false, 0)
        }
    }
}

fn live_output(program: &str, args: &[&str], context: &str) -> Result<String, String> {
    let owned_args = args
        .iter()
        .map(|arg| (*arg).to_string())
        .collect::<Vec<_>>();
    let output = crate::run::capture_output_with_timeout(
        program,
        &owned_args,
        &[("GH_PAGER", "cat"), ("PAGER", "cat"), ("NO_COLOR", "1")],
        LIVE_COLLECTION_TIMEOUT,
        context,
    )?;
    interpret_live_output(program, args, context, output)
}

fn interpret_live_output(
    program: &str,
    args: &[&str],
    context: &str,
    output: crate::run::TimedOutput,
) -> Result<String, String> {
    if output.timed_out {
        return Err(format!(
            "{context} timed out after {} seconds; the child process tree was terminated",
            LIVE_COLLECTION_TIMEOUT.as_secs()
        ));
    }
    let status = output
        .status
        .ok_or_else(|| format!("{context} did not report a process status"))?;
    if status.success() {
        Ok(output.stdout)
    } else {
        Err(format!(
            "{program} {} failed with {status}\nstdout:\n{}\nstderr:\n{}",
            args.join(" "),
            output.stdout.trim(),
            output.stderr.trim()
        ))
    }
}

fn live_timestamp() -> String {
    let seconds = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_secs());
    format!("unix:{seconds}")
}

fn hash_text(value: &str) -> String {
    let digest = Sha256::digest(value.as_bytes());
    let hex = digest
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    format!("sha256:{hex}")
}

fn normalize_snapshot(snapshot: Snapshot) -> NormalizedReport {
    normalize_snapshot_with_origin(snapshot, false)
}

fn normalize_snapshot_with_origin(snapshot: Snapshot, live_collected: bool) -> NormalizedReport {
    let mut reasons = Vec::new();
    reasons.extend(snapshot.collector_errors.iter().cloned());
    validate_source(&snapshot, live_collected, &mut reasons);
    let candidate_state = evaluate_candidate_state(snapshot.candidate_selection.as_ref());

    let mut seen = BTreeSet::new();
    let mut prs = snapshot
        .prs
        .into_iter()
        .map(|pr| {
            let duplicate = !seen.insert(pr.number);
            normalize_pr(pr, duplicate, &mut reasons)
        })
        .collect::<Vec<_>>();
    prs.sort_by_key(|pr| pr.number);

    let status = if reasons.is_empty() {
        "ready".to_string()
    } else {
        "reconcile_required".to_string()
    };
    let merge_allowed = status == "ready";
    for pr in &mut prs {
        pr.merge_eligible = merge_allowed
            && pr.disposition == "release_required"
            && !pr.is_draft
            && !pr.title.trim().is_empty();
    }
    let next_action = if status == "ready" {
        "Use only release_required rows as merge candidates; keep every other disposition visible and open.".to_string()
    } else {
        "Refresh current main, #2379, portfolio/claim inputs, and every open PR disposition before treating any row as merge-eligible.".to_string()
    };

    NormalizedReport {
        status,
        captured_at: snapshot.captured_at,
        source: snapshot.source,
        prs,
        candidate_state,
        reconciliation_reasons: reasons,
        next_action,
    }
}

fn validate_source(snapshot: &Snapshot, live_collected: bool, reasons: &mut Vec<String>) {
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
    } else if source.mode == "live" && !live_collected {
        reasons.push("live snapshots require the --live collector".to_string());
    }
    if source.freshness != "current" {
        reasons.push(format!(
            "source freshness must be current, got {}",
            source.freshness
        ));
    }
    if !is_sha(&source.main_sha) {
        reasons.push("source main_sha must be a 40-character hexadecimal SHA".to_string());
    }
    if source.authority_issue != AUTHORITY_ISSUE {
        reasons.push(format!(
            "release authority must be issue #{AUTHORITY_ISSUE}, got #{}",
            source.authority_issue
        ));
    }
    if source.authority_state != "open" {
        reasons.push(format!(
            "release authority #{} must remain open, got {}",
            source.authority_issue, source.authority_state
        ));
    }
    if source.authority_main_sha != source.main_sha {
        reasons.push("authority main SHA does not match observed main SHA".to_string());
    }
    if source.portfolio_state != "complete" {
        reasons.push(format!(
            "portfolio state must be complete, got {}",
            source.portfolio_state
        ));
    }
    if !source.open_prs_complete {
        reasons.push("open PR inventory is incomplete".to_string());
    }
    if !source.active_claims_complete {
        reasons.push("active claim/worktree inventory is incomplete".to_string());
    }
    if !source.worktree_inventory_complete {
        reasons.push("worktree inventory is incomplete".to_string());
    }
    if source.graph_digest.trim().is_empty() {
        reasons.push("release graph digest is missing".to_string());
    }
}

fn normalize_pr(pr: SnapshotPr, duplicate: bool, reasons: &mut Vec<String>) -> NormalizedPr {
    let disposition = pr
        .release_disposition
        .filter(|value| DISPOSITIONS.contains(&value.as_str()))
        .unwrap_or_else(|| {
            reasons.push(format!(
                "PR #{} is missing a valid release disposition",
                pr.number
            ));
            "blocked_on_named_authority".to_string()
        });
    if duplicate {
        reasons.push(format!("PR #{} appears more than once", pr.number));
    }
    if pr.number == 0 {
        reasons.push("PR number must be non-zero".to_string());
    }
    if !is_sha(&pr.head_sha) {
        reasons.push(format!(
            "PR #{} head_sha must be a 40-character hexadecimal SHA",
            pr.number
        ));
    }
    if pr.state != "open" {
        reasons.push(format!("PR #{} is not open", pr.number));
    }
    if pr.base_ref.trim().is_empty() {
        reasons.push(format!("PR #{} base_ref is missing", pr.number));
    } else if pr.base_ref != "main" {
        reasons.push(format!(
            "PR #{} targets `{}` instead of release base `main`",
            pr.number, pr.base_ref
        ));
    }
    if pr.title.trim().is_empty() {
        reasons.push(format!("PR #{} title is missing", pr.number));
    }
    let reason = pr
        .disposition_reason
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| {
            reasons.push(format!("PR #{} is missing a disposition reason", pr.number));
            "missing disposition reason".to_string()
        });
    let mut linked_issue_refs = pr.linked_issue_refs;
    linked_issue_refs.sort_unstable();
    NormalizedPr {
        number: pr.number,
        title: pr.title,
        is_draft: pr.is_draft,
        head_sha: pr.head_sha,
        base_ref: pr.base_ref,
        linked_issue_refs,
        disposition,
        reason,
        merge_eligible: false,
    }
}

fn is_sha(value: &str) -> bool {
    value.len() == 40 && value.chars().all(|character| character.is_ascii_hexdigit())
}

fn report_json(report: &NormalizedReport) -> Value {
    json!({
        "report": REPORT_NAME,
        "schema_version": SCHEMA_VERSION,
        "status": report.status,
        "captured_at": report.captured_at,
        "source": {
            "mode": report.source.mode,
            "freshness": report.source.freshness,
            "main_sha": report.source.main_sha,
            "authority_issue": report.source.authority_issue,
            "authority_state": report.source.authority_state,
            "authority_main_sha": report.source.authority_main_sha,
            "portfolio_state": report.source.portfolio_state,
            "open_prs_complete": report.source.open_prs_complete,
            "active_claims_complete": report.source.active_claims_complete,
            "worktree_inventory_complete": report.source.worktree_inventory_complete,
            "worktree_count": report.source.worktree_count,
            "graph_digest": report.source.graph_digest,
        },
        "reconciliation_reasons": report.reconciliation_reasons,
        "prs": report.prs.iter().map(|pr| json!({
            "number": pr.number,
            "title": pr.title,
            "is_draft": pr.is_draft,
            "head_sha": pr.head_sha,
            "base_ref": pr.base_ref,
            "linked_issue_refs": pr.linked_issue_refs,
            "release_disposition": pr.disposition,
            "disposition_reason": pr.reason,
            "merge_eligible": pr.merge_eligible,
        })).collect::<Vec<_>>(),
        "candidate_state": &report.candidate_state,
        "next_action": report.next_action,
        "authority_boundary": "temporary_release_lens_only",
        "must_not_claim": [
            "candidate qualification",
            "package readiness",
            "merge approval",
            "release publication",
        ],
    })
}

fn report_markdown(report: &NormalizedReport) -> String {
    let mut body = format!(
        "# ripr release control lens\n\nStatus: `{}`\n\nCaptured: `{}`\n\n",
        report.status, report.captured_at
    );
    body.push_str("## Authority snapshot\n\n");
    body.push_str(&format!(
        "- main: `{}`\n- release authority: `#{} ({})`\n- portfolio: `{}`\n- worktree inventory: `{}` (count: `{}`)\n- graph digest: `{}`\n\n",
        report.source.main_sha,
        report.source.authority_issue,
        report.source.authority_state,
        report.source.portfolio_state,
        report.source.worktree_inventory_complete,
        report.source.worktree_count,
        report.source.graph_digest
    ));
    body.push_str("## Candidate state\n\n");
    body.push_str(&format!(
        "- status: `{}`\n- selected claims: `{}`\n- required claims pending: `{}`\n- unresolved candidate defects: `{}`\n- denominator decisions remaining through provisional cutoff: `{}`\n- denominator decisions remaining through selected cut: `{}`\n- cut selected: `{}`\n- projection reproducible: `{}`\n- candidate tree present: `{}`\n- immutable candidate ref created: `{}`\n\n",
        report.candidate_state.status,
        report.candidate_state.selected_candidate_claims,
        report.candidate_state.candidate_required_claims_pending,
        report.candidate_state.candidate_defects_unresolved,
        report
            .candidate_state
            .denominator_decisions_remaining_through_provisional_cutoff
            .map_or_else(|| "unknown".to_string(), |value| value.to_string()),
        report
            .candidate_state
            .denominator_decisions_remaining_through_selected_cut
            .map_or_else(|| "unknown".to_string(), |value| value.to_string()),
        report.candidate_state.candidate_cut_selected,
        report.candidate_state.projection_reproducible,
        report.candidate_state.candidate_tree_present,
        report.candidate_state.candidate_ref_created,
    ));
    if !report.candidate_state.reasons.is_empty() {
        body.push_str("Candidate-state reasons:\n");
        for reason in &report.candidate_state.reasons {
            body.push_str(&format!("- {}\n", markdown_cell(reason)));
        }
        body.push('\n');
    }
    if !report.reconciliation_reasons.is_empty() {
        body.push_str("## Reconciliation required\n\n");
        for reason in &report.reconciliation_reasons {
            body.push_str(&format!("- {}\n", markdown_cell(reason)));
        }
        body.push('\n');
    }
    body.push_str("## PR dispositions\n\n| PR | Title | Disposition | Merge eligible | Reason |\n| ---: | --- | --- | --- | --- |\n");
    for pr in &report.prs {
        body.push_str(&format!(
            "| #{} | {} | `{}` | `{}` | {} |\n",
            pr.number,
            markdown_cell(&pr.title),
            pr.disposition,
            pr.merge_eligible,
            markdown_cell(&pr.reason)
        ));
    }
    body.push_str(&format!("\n## Next action\n\n{}\n\n", report.next_action));
    body.push_str("This is a temporary, read-only release-selection lens. It does not close issues, merge PRs, create branches, qualify a candidate, publish, or change development `main`.\n");
    body
}

fn markdown_cell(value: &str) -> String {
    value.replace('|', "\\|").replace('\n', " ")
}

#[cfg(test)]
mod tests {
    use super::{
        LIVE_OPEN_PR_LIMIT, ReleaseControlInput, Snapshot, capture_live_snapshot_with,
        interpret_live_output, live_output, normalize_snapshot, normalize_snapshot_with_origin,
        parse_args, read_snapshot, report_json, report_markdown,
    };
    use serde_json::{Value, json};
    use std::collections::VecDeque;
    use std::time::Duration;

    fn require(condition: bool, message: impl Into<String>) -> Result<(), String> {
        condition.then_some(()).ok_or(message.into())
    }

    fn snapshot() -> Result<Snapshot, String> {
        serde_json::from_value(json!({
            "schema_version": "0.1",
            "kind": "release_control_snapshot",
            "captured_at": "2026-07-29T20:30:00Z",
            "source": {
                "mode": "captured",
                "freshness": "current",
                "main_sha": "19849177ce9418d0024e4cc839eb8382e4f716a8",
                "authority_issue": 2379,
                "authority_state": "open",
                "authority_main_sha": "19849177ce9418d0024e4cc839eb8382e4f716a8",
                "portfolio_state": "complete",
                "open_prs_complete": true,
                "active_claims_complete": true,
                "worktree_inventory_complete": true,
                "worktree_count": 1,
                "graph_digest": "sha256:release-graph-fixture"
            },
            "prs": [
                {
                    "number": 2765,
                    "title": "held unrelated PR",
                    "state": "open",
                    "is_draft": false,
                    "head_sha": "54437cdc7591395070c68e4b759aa01b0761d5f0",
                    "base_ref": "main",
                    "linked_issue_refs": [2764],
                    "release_disposition": "hold_post_release",
                    "disposition_reason": "outside the accepted 0.11 release graph"
                },
                {
                    "number": 2528,
                    "title": "release editor qualification",
                    "state": "open",
                    "is_draft": false,
                    "head_sha": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
                    "base_ref": "main",
                    "linked_issue_refs": [2528],
                    "release_disposition": "release_required",
                    "disposition_reason": "named by the accepted #2379 release graph"
                }
            ]
        }))
        .map_err(|err| format!("failed to build test snapshot: {err}"))
    }

    #[test]
    fn complete_snapshot_is_ready_and_only_required_rows_are_merge_eligible() -> Result<(), String>
    {
        let report = normalize_snapshot(snapshot()?);
        if report.status != "ready" {
            return Err(format!("expected ready, got {}", report.status));
        }
        let required = report
            .prs
            .iter()
            .find(|pr| pr.number == 2528)
            .ok_or_else(|| "missing required PR".to_string())?;
        if !required.merge_eligible {
            return Err("release-required PR should be merge eligible".to_string());
        }
        let held = report
            .prs
            .iter()
            .find(|pr| pr.number == 2765)
            .ok_or_else(|| "missing held PR".to_string())?;
        if held.merge_eligible {
            return Err("post-release PR must remain held".to_string());
        }
        Ok(())
    }

    #[test]
    fn missing_candidate_selection_is_exposed_as_scope_pending() -> Result<(), String> {
        let report = normalize_snapshot(snapshot()?);
        require(
            report.status == "ready",
            "candidate scope_pending must not change the reconciliation status",
        )?;
        let report_json = report_json(&report);
        let state = report_json
            .get("candidate_state")
            .and_then(Value::as_object)
            .ok_or_else(|| "candidate_state was missing from JSON output".to_string())?;
        require(
            state.get("status") == Some(&Value::String("scope_pending".to_string())),
            "missing candidate selection must remain scope_pending in JSON",
        )?;
        require(
            state.get("candidate_cut_selected") == Some(&Value::Bool(false)),
            "missing candidate selection must not select a cut",
        )
    }

    #[test]
    fn missing_disposition_fails_closed() -> Result<(), String> {
        let mut value = serde_json::to_value(snapshot()?)
            .map_err(|err| format!("failed to serialize snapshot: {err}"))?;
        let pr = value
            .get_mut("prs")
            .and_then(Value::as_array_mut)
            .and_then(|prs| prs.first_mut())
            .and_then(Value::as_object_mut)
            .ok_or_else(|| "expected PR object".to_string())?;
        pr.remove("release_disposition");
        pr.remove("disposition_reason");
        let report: Snapshot = serde_json::from_value(value)
            .map_err(|err| format!("failed to parse changed snapshot: {err}"))?;
        let normalized = normalize_snapshot(report);
        if normalized.status != "reconcile_required" {
            return Err("missing disposition should require reconciliation".to_string());
        }
        if normalized.prs.iter().any(|pr| pr.merge_eligible) {
            return Err(
                "reconciliation-required report cannot expose merge eligibility".to_string(),
            );
        }
        Ok(())
    }

    #[test]
    fn input_order_does_not_change_normalized_output() -> Result<(), String> {
        let first = normalize_snapshot(snapshot()?);
        let mut reversed = snapshot()?;
        reversed.prs.reverse();
        let second = normalize_snapshot(reversed);
        if report_json(&first) != report_json(&second) {
            return Err("normalized JSON changed with input order".to_string());
        }
        if report_markdown(&first) != report_markdown(&second) {
            return Err("normalized Markdown changed with input order".to_string());
        }
        Ok(())
    }

    #[test]
    fn stale_authority_cannot_be_merge_eligible() -> Result<(), String> {
        let mut stale = snapshot()?;
        stale.source.authority_main_sha = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb".to_string();
        let report = normalize_snapshot(stale);
        if report.status != "reconcile_required" {
            return Err("stale authority should require reconciliation".to_string());
        }
        if report.prs.iter().any(|pr| pr.merge_eligible) {
            return Err("stale authority must not permit merge eligibility".to_string());
        }
        Ok(())
    }

    #[test]
    fn unsupported_live_mode_cannot_be_merge_eligible() -> Result<(), String> {
        let mut live = snapshot()?;
        live.source.mode = "live".to_string();
        let report = normalize_snapshot(live);
        if report.status != "reconcile_required" {
            return Err("unsupported live mode should require reconciliation".to_string());
        }
        if report.prs.iter().any(|pr| pr.merge_eligible) {
            return Err("unsupported live mode must not permit merge eligibility".to_string());
        }
        Ok(())
    }

    #[test]
    fn live_origin_is_required_before_live_snapshot_can_be_ready() -> Result<(), String> {
        let mut live = snapshot()?;
        live.source.mode = "live".to_string();
        if normalize_snapshot(live.clone()).status != "reconcile_required" {
            return Err("input-supplied live snapshot must require the collector".to_string());
        }
        let collected = normalize_snapshot_with_origin(live, true);
        if collected.status != "ready" {
            return Err(format!(
                "collector-owned complete live snapshot should be ready, got {}",
                collected.status
            ));
        }
        Ok(())
    }

    #[test]
    fn collector_error_fails_closed_and_clears_eligibility() -> Result<(), String> {
        let mut live = snapshot()?;
        live.source.mode = "live".to_string();
        live.collector_errors
            .push("open PR inventory unavailable".to_string());
        let report = normalize_snapshot_with_origin(live, true);
        if report.status != "reconcile_required" {
            return Err("collector errors must require reconciliation".to_string());
        }
        if report.prs.iter().any(|pr| pr.merge_eligible) {
            return Err("collector errors must clear merge eligibility".to_string());
        }
        Ok(())
    }

    #[test]
    fn non_main_base_cannot_be_merge_eligible() -> Result<(), String> {
        let mut non_main = snapshot()?;
        let pr = non_main
            .prs
            .get_mut(1)
            .ok_or_else(|| "expected second PR".to_string())?;
        pr.base_ref = "release/0.11".to_string();
        let report = normalize_snapshot(non_main);
        if report.status != "reconcile_required" {
            return Err("non-main base should require reconciliation".to_string());
        }
        if report.prs.iter().any(|pr| pr.merge_eligible) {
            return Err("non-main base must not permit merge eligibility".to_string());
        }
        Ok(())
    }

    #[test]
    fn argument_and_snapshot_loading_paths_are_explicit() -> Result<(), String> {
        let live_args = ["--live".to_string()];
        let live = parse_args(&live_args)?;
        require(
            matches!(live, ReleaseControlInput::Live),
            "--live should select the live collector",
        )?;
        let input_args = ["--input".to_string(), "snapshot.json".to_string()];
        let input = parse_args(&input_args)?;
        require(
            matches!(input, ReleaseControlInput::Captured("snapshot.json")),
            "--input should retain its path",
        )?;
        for args in [
            vec!["--help".to_string()],
            vec!["--input".to_string()],
            vec!["--input".to_string(), " ".to_string()],
            vec!["--unexpected".to_string()],
        ] {
            require(
                parse_args(&args).is_err(),
                format!("invalid arguments were accepted: {args:?}"),
            )?;
        }

        let fixture_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("fixtures")
            .join("release_control")
            .join("complete.json");
        let fixture_text = fixture_path
            .to_str()
            .ok_or_else(|| "fixture path was not UTF-8".to_string())?;
        let loaded = read_snapshot(fixture_text)?;
        require(
            loaded.kind == "release_control_snapshot",
            "fixture did not load as a release-control snapshot",
        )?;
        require(
            read_snapshot("fixtures/release_control/does-not-exist.json").is_err(),
            "missing snapshot should return an error",
        )?;
        Ok(())
    }

    #[test]
    fn malformed_source_and_pr_fields_all_fail_closed() -> Result<(), String> {
        let mut malformed = snapshot()?;
        malformed.schema_version = "wrong".to_string();
        malformed.kind = "wrong_kind".to_string();
        malformed.captured_at.clear();
        malformed.source.mode = "unknown".to_string();
        malformed.source.freshness = "stale".to_string();
        malformed.source.main_sha = "bad".to_string();
        malformed.source.authority_issue = 1;
        malformed.source.authority_state = "closed".to_string();
        malformed.source.authority_main_sha = "different".to_string();
        malformed.source.portfolio_state = "unknown".to_string();
        malformed.source.open_prs_complete = false;
        malformed.source.active_claims_complete = false;
        malformed.source.worktree_inventory_complete = false;
        malformed.source.graph_digest.clear();
        let first = malformed
            .prs
            .first_mut()
            .ok_or_else(|| "expected fixture PR".to_string())?;
        first.number = 0;
        first.title.clear();
        first.state = "closed".to_string();
        first.head_sha = "bad".to_string();
        first.base_ref.clear();
        first.release_disposition = Some("not-a-disposition".to_string());
        first.disposition_reason = Some(" ".to_string());
        let duplicate = malformed
            .prs
            .first()
            .cloned()
            .ok_or_else(|| "expected malformed PR".to_string())?;
        malformed.prs.push(duplicate);

        let report = normalize_snapshot(malformed);
        require(
            report.status == "reconcile_required" && !report.prs.iter().any(|pr| pr.merge_eligible),
            "malformed input must be fully non-eligible",
        )?;
        let _ = report_markdown(&report);
        for expected in [
            "snapshot schema_version",
            "snapshot kind",
            "snapshot captured_at is missing",
            "source mode must be captured or live",
            "source freshness must be current",
            "source main_sha must be",
            "release authority must be issue #2379",
            "release authority #1 must remain open",
            "authority main SHA does not match",
            "portfolio state must be complete",
            "open PR inventory is incomplete",
            "active claim/worktree inventory is incomplete",
            "worktree inventory is incomplete",
            "release graph digest is missing",
            "PR #0 is missing a valid release disposition",
            "PR #0 appears more than once",
            "PR number must be non-zero",
            "PR #0 head_sha must be",
            "PR #0 is not open",
            "PR #0 base_ref is missing",
            "PR #0 title is missing",
            "PR #0 is missing a disposition reason",
        ] {
            require(
                report
                    .reconciliation_reasons
                    .iter()
                    .any(|reason| reason.contains(expected)),
                format!("missing malformed-input reason: {expected}"),
            )?;
        }
        Ok(())
    }

    #[test]
    fn markdown_and_json_escape_and_expose_contract_boundaries() -> Result<(), String> {
        let mut escaped = snapshot()?;
        let held = escaped
            .prs
            .first_mut()
            .ok_or_else(|| "expected held PR".to_string())?;
        held.title = "held | title\ncontinued".to_string();
        held.disposition_reason = Some("reason | text\ncontinued".to_string());
        let report = normalize_snapshot(escaped);
        let markdown = report_markdown(&report);
        require(
            markdown.contains("held \\| title continued")
                && markdown.contains("reason \\| text continued"),
            "Markdown cells were not escaped and normalized",
        )?;
        let json = report_json(&report);
        require(
            json.get("authority_boundary")
                == Some(&Value::String("temporary_release_lens_only".to_string())),
            "JSON authority boundary changed unexpectedly",
        )?;
        require(
            json.get("must_not_claim")
                .and_then(Value::as_array)
                .is_some_and(|claims| claims.len() == 4),
            "JSON must-not-claim boundary is incomplete",
        )?;
        Ok(())
    }

    #[test]
    fn bounded_live_collector_normalizes_success_and_failure_inputs() -> Result<(), String> {
        let main_sha = "cccccccccccccccccccccccccccccccccccccccc";
        let mut successful_outputs = VecDeque::from(vec![
            Ok(format!("{main_sha}\n")),
            Ok(r#"[{"number":7,"title":"live PR","state":"OPEN","isDraft":false,"headRefOid":"dddddddddddddddddddddddddddddddddddddddd","baseRefName":"main"}]"#.to_string()),
            Ok(r#"{"state":"OPEN","body":"release graph"}"#.to_string()),
            Ok("worktree repo-root\n\nworktree other-root\n".to_string()),
        ]);
        let live = capture_live_snapshot_with(|_, _, _| {
            successful_outputs
                .pop_front()
                .ok_or_else(|| "test output queue exhausted".to_string())?
        });
        require(
            live.source.freshness == "current"
                && live.source.main_sha == main_sha
                && live.source.authority_state == "open"
                && live.source.worktree_count == 2
                && live.collector_errors.is_empty(),
            "successful live collection was not normalized",
        )?;
        require(
            live.prs.first().map(|pr| pr.state.as_str()) == Some("open"),
            "live PR state was not normalized",
        )?;
        require(
            normalize_snapshot_with_origin(live, true).status == "reconcile_required",
            "live collection must remain fail-closed without claim inputs",
        )?;

        let mut failed_outputs = VecDeque::from(vec![
            Err("main unavailable".to_string()),
            Ok("not-json".to_string()),
            Ok("also-not-json".to_string()),
            Err("worktrees unavailable".to_string()),
        ]);
        let failed = capture_live_snapshot_with(|_, _, _| {
            failed_outputs
                .pop_front()
                .ok_or_else(|| "test output queue exhausted".to_string())?
        });
        require(
            failed.source.freshness == "unknown"
                && !failed.source.open_prs_complete
                && !failed.source.worktree_inventory_complete
                && failed.source.worktree_count == 0
                && failed.collector_errors.len() == 4,
            "failed live collection did not remain visibly incomplete",
        )?;

        let mut command_failures = VecDeque::from(vec![
            Ok(format!("{main_sha}\n")),
            Err("PR inventory unavailable".to_string()),
            Err("release authority unavailable".to_string()),
            Ok("worktree repo-root\n".to_string()),
        ]);
        let command_failed = capture_live_snapshot_with(|_, _, _| {
            command_failures
                .pop_front()
                .ok_or_else(|| "test output queue exhausted".to_string())?
        });
        require(
            command_failed.collector_errors.len() == 2
                && !command_failed.source.open_prs_complete
                && command_failed.source.worktree_inventory_complete,
            "collector command failures were not recorded",
        )?;
        Ok(())
    }

    #[test]
    fn live_open_pr_bound_is_disclosed_and_fails_closed() -> Result<(), String> {
        let main_sha = "cccccccccccccccccccccccccccccccccccccccc";
        let rows = (1..=101)
            .map(|number| {
                format!(
                    r#"{{"number":{number},"title":"live PR {number}","state":"OPEN","isDraft":false,"headRefOid":"{number:040x}","baseRefName":"main"}}"#
                )
            })
            .collect::<Vec<_>>()
            .join(",");
        let mut outputs = VecDeque::from(vec![
            Ok(format!("{main_sha}\n")),
            Ok(format!("[{rows}]")),
            Ok(r#"{"state":"OPEN","body":"release graph"}"#.to_string()),
            Ok("worktree repo-root\n".to_string()),
        ]);
        let mut requested_sentinel_limit = false;
        let live = capture_live_snapshot_with(|program, args, _| {
            if program == "gh"
                && args
                    .iter()
                    .position(|arg| *arg == "--limit")
                    .is_some_and(|index| args.get(index + 1).copied() == Some("101"))
            {
                requested_sentinel_limit = true;
            }
            outputs
                .pop_front()
                .ok_or_else(|| "test output queue exhausted".to_string())?
        });
        if !requested_sentinel_limit {
            return Err("live collector did not request the sentinel PR row".to_string());
        }
        if live.source.open_prs_complete || live.prs.len() != LIVE_OPEN_PR_LIMIT {
            return Err("bounded live PR inventory was not marked incomplete".to_string());
        }
        if !live
            .collector_errors
            .iter()
            .any(|error| error.contains("exceeded the bounded 100-row inventory"))
        {
            return Err("live PR truncation lost its reconciliation reason".to_string());
        }
        if normalize_snapshot_with_origin(live, true).status != "reconcile_required" {
            return Err("truncated live PR inventory must remain fail-closed".to_string());
        }
        Ok(())
    }

    #[test]
    fn live_command_and_result_interpretation_are_bounded() -> Result<(), String> {
        let stdout = live_output("git", &["--version"], "test success")?;
        require(
            stdout.contains("git version"),
            "successful live command did not return stdout",
        )?;
        require(
            live_output(
                "git",
                &["--definitely-invalid-ripr-argument"],
                "test failure",
            )
            .is_err(),
            "failed live command should return an error",
        )?;
        require(
            live_output(
                "ripr-release-control-command-that-does-not-exist",
                &[],
                "missing program",
            )
            .is_err(),
            "missing live program should return an error",
        )?;

        let timed_out = crate::run::TimedOutput {
            status: None,
            stdout: String::new(),
            stderr: String::new(),
            duration: Duration::ZERO,
            timed_out: true,
        };
        require(
            interpret_live_output("cmd", &[], "timed out", timed_out)
                .err()
                .is_some_and(|error| error.contains("timed out")),
            "timed-out live command lost its diagnostic",
        )?;
        let missing_status = crate::run::TimedOutput {
            status: None,
            stdout: String::new(),
            stderr: String::new(),
            duration: Duration::ZERO,
            timed_out: false,
        };
        require(
            interpret_live_output("cmd", &[], "missing status", missing_status)
                .err()
                .is_some_and(|error| error.contains("did not report")),
            "missing live process status lost its diagnostic",
        )?;
        Ok(())
    }
}
