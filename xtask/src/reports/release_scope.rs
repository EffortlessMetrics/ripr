//! Candidate-only scope reconciliation for the accepted 0.11 release boundary.
//!
//! This report does not construct, merge, or publish a candidate. It verifies a
//! captured scope decision against the exact execution commit and the current
//! candidate-parent ref so a later candidate builder cannot silently remove
//! only part of the `verify-execute` surface.

use std::collections::BTreeSet;
use std::fs;
use std::path::Path;
use std::process::Command;

use serde::{Deserialize, Serialize};
#[cfg(test)]
use serde_json::Value;
use sha2::{Digest, Sha256};

const REPORT_NAME: &str = "release-scope";
const JSON_FILE: &str = "release-scope.json";
const MARKDOWN_FILE: &str = "release-scope.md";
const SCHEMA_VERSION: &str = "0.1";
const EXPECTED_OUTCOME: &str = "preserve_accepted_0_11";
const EXPECTED_ISSUE_STATE: &str = "open";
const EXPECTED_NON_CLAIM: &str =
    "RIPR 0.11.0 does not execute verification commands and does not issue RepairReceiptV2.";

#[derive(Clone, Debug, Deserialize, Serialize)]
struct ScopeInput {
    schema_version: String,
    kind: String,
    outcome: String,
    candidate_parent_ref: String,
    candidate_parent_sha: String,
    execution_commit: String,
    strictly_dependent_commits: Vec<String>,
    execution_only_paths: Vec<String>,
    candidate_excluded_paths: Vec<String>,
    preserved_paths: Vec<String>,
    release_non_claim: String,
    issue_2332_state: String,
    candidate_tree: CandidateTreeInput,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct CandidateTreeInput {
    state: String,
    operation: String,
    source_history_unchanged: bool,
}

#[derive(Clone, Debug, Serialize)]
struct ScopeReport {
    schema_version: &'static str,
    kind: &'static str,
    status: &'static str,
    decision: DecisionReport,
    source: SourceReport,
    candidate_tree_delta: CandidateTreeReport,
    checks: ChecksReport,
    reconciliation_reasons: Vec<String>,
    authority_boundary: &'static str,
    must_not_claim: Vec<&'static str>,
}

#[derive(Clone, Debug, Serialize)]
struct DecisionReport {
    outcome: String,
    issue_2332_state: String,
    release_non_claim: String,
    decision_digest: String,
}

#[derive(Clone, Debug, Serialize)]
struct SourceReport {
    candidate_parent_ref: String,
    candidate_parent_sha: String,
    observed_candidate_parent_sha: Option<String>,
    execution_commit: String,
    execution_commit_parents: Vec<String>,
    strictly_dependent_commits: Vec<String>,
    execution_only_paths: Vec<String>,
    observed_execution_paths: Vec<String>,
    preserved_paths: Vec<String>,
}

#[derive(Clone, Debug, Serialize)]
struct CandidateTreeReport {
    state: String,
    operation: String,
    source_history_unchanged: bool,
    excluded_commit: String,
    excluded_paths: Vec<String>,
    candidate_tree_constructed: bool,
}

#[derive(Clone, Debug, Serialize)]
struct ChecksReport {
    outcome_is_explicit: bool,
    execution_commit_is_present: bool,
    execution_paths_match_commit: bool,
    exclusion_is_complete: bool,
    preserved_paths_are_outside_exclusion: bool,
    candidate_parent_is_current: bool,
    issue_2332_remains_open: bool,
    candidate_tree_is_not_claimed: bool,
}

pub(crate) fn release_scope(args: &[String]) -> Result<(), String> {
    let input_path = parse_input_path(args)?;
    let text = fs::read_to_string(&input_path)
        .map_err(|error| format!("failed to read release-scope input {input_path}: {error}"))?;
    let input: ScopeInput = serde_json::from_str(&text)
        .map_err(|error| format!("failed to parse release-scope input {input_path}: {error}"))?;
    let root = repository_root()?;
    let report = build_report(&input, &root)?;
    let json = serde_json::to_string_pretty(&report)
        .map_err(|error| format!("failed to serialize release-scope JSON: {error}"))?;
    let markdown = report_markdown(&report);
    let report_dir = Path::new("target/ripr/reports");
    fs::create_dir_all(report_dir)
        .map_err(|error| format!("failed to create {}: {error}", report_dir.display()))?;
    fs::write(report_dir.join(JSON_FILE), format!("{json}\n"))
        .map_err(|error| format!("failed to write {JSON_FILE}: {error}"))?;
    fs::write(report_dir.join(MARKDOWN_FILE), markdown)
        .map_err(|error| format!("failed to write {MARKDOWN_FILE}: {error}"))?;
    println!("{REPORT_NAME}: {}", report.status);
    if report.status == "reconcile_required" {
        Err(format!(
            "release-scope requires reconciliation; see target/ripr/reports/{JSON_FILE}"
        ))
    } else {
        Ok(())
    }
}

fn parse_input_path(args: &[String]) -> Result<String, String> {
    if args.len() != 2 || args[0] != "--input" || args[1].trim().is_empty() {
        return Err("usage: cargo xtask release-scope --input <scope.json>".to_string());
    }
    Ok(args[1].clone())
}

fn repository_root() -> Result<std::path::PathBuf, String> {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .map(Path::to_path_buf)
        .ok_or_else(|| "xtask manifest has no repository parent".to_string())
}

#[derive(Clone, Debug)]
struct GitFacts {
    observed_candidate_parent_sha: Option<String>,
    execution_commit_is_present: bool,
    observed_execution_paths: Vec<String>,
    preserved_paths_present: bool,
    execution_commit_parents: Vec<String>,
    missing_dependent_commits: Vec<String>,
}

fn build_report(input: &ScopeInput, root: &Path) -> Result<ScopeReport, String> {
    build_report_with_facts(input, collect_git_facts(input, root))
}

fn build_report_with_facts(input: &ScopeInput, facts: GitFacts) -> Result<ScopeReport, String> {
    let mut reasons = Vec::new();
    if input.schema_version != SCHEMA_VERSION {
        reasons.push(format!(
            "unsupported scope schema `{}`; expected `{SCHEMA_VERSION}`",
            input.schema_version
        ));
    }
    if input.kind != "release_execution_scope" {
        reasons.push("input kind is not release_execution_scope".to_string());
    }
    if input.outcome != EXPECTED_OUTCOME {
        reasons.push(format!(
            "outcome `{}` is not the accepted 0.11 preservation decision",
            input.outcome
        ));
    }
    if input.issue_2332_state != EXPECTED_ISSUE_STATE {
        reasons.push("#2332 must remain open for undelivered execution acceptance".to_string());
    }
    if input.release_non_claim != EXPECTED_NON_CLAIM {
        reasons.push("release non-claim does not match the accepted authority".to_string());
    }
    if input.candidate_tree.state != "candidate_only_exclusion" {
        reasons.push("candidate tree state must be candidate_only_exclusion".to_string());
    }
    if input.candidate_tree.operation != "exclude_commit_paths" {
        reasons.push("candidate tree operation must be exclude_commit_paths".to_string());
    }
    if !input.candidate_tree.source_history_unchanged {
        reasons.push("candidate-only scope must preserve development history".to_string());
    }

    if !facts.execution_commit_is_present {
        reasons.push(format!(
            "execution commit `{}` is not present",
            input.execution_commit
        ));
    }
    for dependent in &facts.missing_dependent_commits {
        reasons.push(format!("dependent commit `{dependent}` is not present"));
    }
    let candidate_parent_is_current = is_sha(&input.candidate_parent_sha)
        && facts
            .observed_candidate_parent_sha
            .as_deref()
            .is_some_and(|sha| sha == input.candidate_parent_sha);
    if !candidate_parent_is_current {
        reasons.push(format!(
            "candidate parent `{}` is stale or unavailable; expected {}",
            input.candidate_parent_ref, input.candidate_parent_sha
        ));
    }

    let expected_paths = sorted_unique(&input.execution_only_paths);
    let observed_paths = sorted_unique(&facts.observed_execution_paths);
    if has_duplicates(&input.execution_only_paths) {
        reasons.push("execution-only path inventory contains duplicates".to_string());
    }
    if has_duplicates(&input.candidate_excluded_paths) {
        reasons.push("candidate exclusion path inventory contains duplicates".to_string());
    }
    if has_duplicates(&input.preserved_paths) {
        reasons.push("preserved path inventory contains duplicates".to_string());
    }
    if has_duplicates(&input.strictly_dependent_commits) {
        reasons.push("strictly dependent commits inventory contains duplicates".to_string());
    }
    let execution_paths_match_commit =
        expected_paths == observed_paths && !expected_paths.is_empty();
    if !execution_paths_match_commit {
        reasons.push("execution-only path inventory does not match the named commit".to_string());
    }

    let exclusion_paths = sorted_unique(&input.candidate_excluded_paths);
    let exclusion_is_complete = exclusion_paths == expected_paths && !exclusion_paths.is_empty();
    if !exclusion_is_complete {
        reasons
            .push("candidate exclusion does not cover the complete execution surface".to_string());
    }
    let preserved_paths_are_outside_exclusion = facts.preserved_paths_present
        && input
            .preserved_paths
            .iter()
            .all(|path| !exclusion_paths.contains(path));
    if !preserved_paths_are_outside_exclusion {
        reasons.push(
            "preserved provenance/static-assurance paths are missing or excluded".to_string(),
        );
    }
    let candidate_tree_is_not_claimed = true;
    let decision_digest = sha256_json(input)?;
    let status = if reasons.is_empty() {
        "ready"
    } else {
        "reconcile_required"
    };

    Ok(ScopeReport {
        schema_version: SCHEMA_VERSION,
        kind: REPORT_NAME,
        status,
        decision: DecisionReport {
            outcome: input.outcome.clone(),
            issue_2332_state: input.issue_2332_state.clone(),
            release_non_claim: input.release_non_claim.clone(),
            decision_digest,
        },
        source: SourceReport {
            candidate_parent_ref: input.candidate_parent_ref.clone(),
            candidate_parent_sha: input.candidate_parent_sha.clone(),
            observed_candidate_parent_sha: facts.observed_candidate_parent_sha,
            execution_commit: input.execution_commit.clone(),
            execution_commit_parents: facts.execution_commit_parents,
            strictly_dependent_commits: sorted_unique(&input.strictly_dependent_commits),
            execution_only_paths: expected_paths.clone(),
            observed_execution_paths: observed_paths,
            preserved_paths: sorted_unique(&input.preserved_paths),
        },
        candidate_tree_delta: CandidateTreeReport {
            state: input.candidate_tree.state.clone(),
            operation: input.candidate_tree.operation.clone(),
            source_history_unchanged: input.candidate_tree.source_history_unchanged,
            excluded_commit: input.execution_commit.clone(),
            excluded_paths: exclusion_paths,
            candidate_tree_constructed: false,
        },
        checks: ChecksReport {
            outcome_is_explicit: input.outcome == EXPECTED_OUTCOME,
            execution_commit_is_present: facts.execution_commit_is_present,
            execution_paths_match_commit,
            exclusion_is_complete,
            preserved_paths_are_outside_exclusion,
            candidate_parent_is_current,
            issue_2332_remains_open: input.issue_2332_state == EXPECTED_ISSUE_STATE,
            candidate_tree_is_not_claimed,
        },
        reconciliation_reasons: reasons,
        authority_boundary: "candidate_scope_decision_only",
        must_not_claim: vec![
            "candidate qualification",
            "verification command execution",
            "RepairReceiptV2 issuance",
            "merge approval",
            "release publication",
        ],
    })
}

fn collect_git_facts(input: &ScopeInput, root: &Path) -> GitFacts {
    let execution_commit_is_present = is_sha(&input.execution_commit)
        && git_succeeds(
            root,
            &[
                "cat-file",
                "-e",
                &format!("{}^{{commit}}", input.execution_commit),
            ],
        );
    let missing_dependent_commits = input
        .strictly_dependent_commits
        .iter()
        .filter(|dependent| {
            !is_sha(dependent)
                || !git_succeeds(
                    root,
                    &["cat-file", "-e", &format!("{}^{{commit}}", dependent)],
                )
        })
        .cloned()
        .collect();
    let observed_candidate_parent_sha = git_output(
        root,
        &[
            "rev-parse",
            "--verify",
            &format!("{}^{{commit}}", input.candidate_parent_ref),
        ],
    );
    let observed_execution_paths = git_output_lines(
        root,
        &[
            "show",
            "--format=",
            "--name-only",
            "--no-renames",
            &input.execution_commit,
        ],
    );
    let preserved_paths_present = input.preserved_paths.iter().all(|path| {
        git_succeeds(
            root,
            &[
                "cat-file",
                "-e",
                &format!("{}:{path}", input.candidate_parent_sha),
            ],
        )
    });
    let execution_commit_parents = git_output(
        root,
        &["show", "-s", "--format=%P", &input.execution_commit],
    )
    .unwrap_or_default()
    .split_whitespace()
    .map(str::to_string)
    .collect();
    GitFacts {
        observed_candidate_parent_sha,
        execution_commit_is_present,
        observed_execution_paths,
        preserved_paths_present,
        execution_commit_parents,
        missing_dependent_commits,
    }
}

fn sorted_unique(values: &[String]) -> Vec<String> {
    values
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

fn has_duplicates(values: &[String]) -> bool {
    values.len() != values.iter().collect::<BTreeSet<_>>().len()
}

fn is_sha(value: &str) -> bool {
    value.len() == 40 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn git_succeeds(root: &Path, args: &[&str]) -> bool {
    Command::new("git")
        .args(args)
        .current_dir(root)
        .output()
        .is_ok_and(|output| output.status.success())
}

fn git_output(root: &Path, args: &[&str]) -> Option<String> {
    let output = Command::new("git")
        .args(args)
        .current_dir(root)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    Some(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

fn git_output_lines(root: &Path, args: &[&str]) -> Vec<String> {
    git_output(root, args)
        .unwrap_or_default()
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(str::to_string)
        .collect()
}

fn sha256_json(input: &ScopeInput) -> Result<String, String> {
    let mut canonical = input.clone();
    canonical.execution_only_paths.sort();
    canonical.candidate_excluded_paths.sort();
    canonical.preserved_paths.sort();
    canonical.strictly_dependent_commits.sort();
    let bytes = serde_json::to_vec(&canonical)
        .map_err(|error| format!("failed to serialize scope decision for digest: {error}"))?;
    let digest = Sha256::digest(bytes);
    Ok(format!("sha256:{digest:x}"))
}

fn report_markdown(report: &ScopeReport) -> String {
    let mut markdown = format!(
        "# Release execution scope\n\n- Status: `{}`\n- Outcome: `{}`\n- Candidate parent: `{}` (`{}`)\n- Execution commit: `{}`\n- Decision digest: `{}`\n\n",
        report.status,
        report.decision.outcome,
        report.source.candidate_parent_ref,
        report.source.candidate_parent_sha,
        report.source.execution_commit,
        report.decision.decision_digest
    );
    markdown.push_str("## Candidate-only decision\n\n");
    markdown.push_str("The accepted 0.11 verification non-claim remains in force. ");
    markdown.push_str(&report.decision.release_non_claim);
    markdown.push_str("\n\n");
    markdown.push_str("## Exclusion\n\n");
    markdown.push_str(&format!(
        "- State: `{}`\n- Operation: `{}`\n- Commit: `{}`\n- Constructed here: `{}`\n\n",
        report.candidate_tree_delta.state,
        report.candidate_tree_delta.operation,
        report.candidate_tree_delta.excluded_commit,
        report.candidate_tree_delta.candidate_tree_constructed
    ));
    markdown.push_str("## Reconciliation\n\n");
    if report.reconciliation_reasons.is_empty() {
        markdown.push_str("All captured scope checks passed.\n\n");
    } else {
        for reason in &report.reconciliation_reasons {
            markdown.push_str(&format!("- {reason}\n"));
        }
        markdown.push('\n');
    }
    markdown.push_str("## Must not claim\n\n");
    for claim in &report.must_not_claim {
        markdown.push_str(&format!("- {claim}\n"));
    }
    markdown
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> Result<ScopeInput, String> {
        let text = include_str!("../../../fixtures/release_scope/accepted-outcome-a.json");
        serde_json::from_str(text).map_err(|error| error.to_string())
    }

    fn root() -> Result<std::path::PathBuf, String> {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .map(Path::to_path_buf)
            .ok_or_else(|| "xtask has no repository parent".to_string())
    }

    fn captured_facts(input: &ScopeInput) -> GitFacts {
        GitFacts {
            observed_candidate_parent_sha: Some(input.candidate_parent_sha.clone()),
            execution_commit_is_present: true,
            observed_execution_paths: input.execution_only_paths.clone(),
            preserved_paths_present: true,
            execution_commit_parents: vec!["captured-parent".to_string()],
            missing_dependent_commits: Vec::new(),
        }
    }

    #[test]
    fn accepted_scope_normalizes_captured_git_observation() -> Result<(), String> {
        let input = fixture()?;
        let report = build_report_with_facts(&input, captured_facts(&input))?;
        if report.status != "ready" {
            return Err(format!(
                "expected ready report: {:?}",
                report.reconciliation_reasons
            ));
        }
        if !report.checks.execution_paths_match_commit || !report.checks.exclusion_is_complete {
            return Err("accepted scope did not verify its complete path inventory".to_string());
        }
        Ok(())
    }

    #[test]
    fn a_missing_excluded_path_fails_closed() -> Result<(), String> {
        let mut input = fixture()?;
        input.candidate_excluded_paths.pop();
        let report = build_report(&input, &root()?)?;
        if report.status != "reconcile_required"
            || report.checks.exclusion_is_complete
            || !report
                .reconciliation_reasons
                .iter()
                .any(|reason| reason.contains("complete execution surface"))
        {
            return Err("partial candidate exclusion was not rejected".to_string());
        }
        Ok(())
    }

    #[test]
    fn a_changed_commit_path_fails_closed() -> Result<(), String> {
        let mut input = fixture()?;
        input
            .execution_only_paths
            .push("invented/path.rs".to_string());
        let report = build_report(&input, &root()?)?;
        if report.status != "reconcile_required" || report.checks.execution_paths_match_commit {
            return Err("invented execution path was accepted".to_string());
        }
        Ok(())
    }

    #[test]
    fn closed_execution_issue_fails_closed() -> Result<(), String> {
        let mut input = fixture()?;
        input.issue_2332_state = "closed".to_string();
        let report = build_report(&input, &root()?)?;
        if report.status != "reconcile_required" || report.checks.issue_2332_remains_open {
            return Err("closed #2332 was treated as an accepted scope".to_string());
        }
        Ok(())
    }

    #[test]
    fn an_unapproved_non_claim_fails_closed() -> Result<(), String> {
        let mut input = fixture()?;
        input.release_non_claim = "verification is probably not used".to_string();
        let report = build_report(&input, &root()?)?;
        if report.status != "reconcile_required" {
            return Err("an unapproved release claim was accepted".to_string());
        }
        Ok(())
    }

    #[test]
    fn duplicate_exclusion_paths_fail_closed() -> Result<(), String> {
        let mut input = fixture()?;
        let duplicate = input
            .candidate_excluded_paths
            .first()
            .cloned()
            .ok_or_else(|| "fixture has no exclusion paths".to_string())?;
        input.candidate_excluded_paths.push(duplicate);
        let report = build_report(&input, &root()?)?;
        if report.status != "reconcile_required" {
            return Err("duplicate exclusion path was accepted".to_string());
        }
        Ok(())
    }

    #[test]
    fn duplicate_strictly_dependent_commits_fail_closed() -> Result<(), String> {
        let mut input = fixture()?;
        input.strictly_dependent_commits = vec![
            input.execution_commit.clone(),
            input.execution_commit.clone(),
        ];
        let report = build_report(&input, &root()?)?;
        if report.status != "reconcile_required"
            || !report.reconciliation_reasons.iter().any(|reason| {
                reason.contains("strictly dependent commits inventory contains duplicates")
            })
        {
            return Err("duplicate strictly dependent commit was accepted".to_string());
        }
        Ok(())
    }

    #[test]
    fn markdown_is_derived_from_the_normalized_report() -> Result<(), String> {
        let input = fixture()?;
        let report = build_report_with_facts(&input, captured_facts(&input))?;
        let markdown = report_markdown(&report);
        for required in [
            "ready",
            EXPECTED_OUTCOME,
            "candidate_only_exclusion",
            "Must not claim",
        ] {
            if !markdown.contains(required) {
                return Err(format!("markdown is missing `{required}`"));
            }
        }
        Ok(())
    }

    #[test]
    fn input_order_does_not_change_the_decision_digest() -> Result<(), String> {
        let mut first = fixture()?;
        let mut second = fixture()?;
        second.execution_only_paths.reverse();
        second.candidate_excluded_paths.reverse();
        first.execution_only_paths.sort();
        first.candidate_excluded_paths.sort();
        let first_digest = sha256_json(&first)?;
        let second_digest = sha256_json(&second)?;
        if first_digest != second_digest {
            return Err("canonical decision digest changed with path ordering".to_string());
        }
        Ok(())
    }

    #[test]
    fn report_json_has_an_explicit_authority_boundary() -> Result<(), String> {
        let input = fixture()?;
        let report = build_report(&input, &root()?)?;
        let value: Value = serde_json::to_value(report).map_err(|error| error.to_string())?;
        let authority = value
            .get("authority_boundary")
            .and_then(Value::as_str)
            .unwrap_or_default();
        let constructed = value
            .get("candidate_tree_delta")
            .and_then(Value::as_object)
            .and_then(|delta| delta.get("candidate_tree_constructed"))
            .and_then(Value::as_bool);
        if authority != "candidate_scope_decision_only" || constructed != Some(false) {
            return Err("report widened the candidate-scope authority".to_string());
        }
        Ok(())
    }
}
