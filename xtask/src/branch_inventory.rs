//! Remote-branch inventory and reviewed cleanup planning (#2024).
//!
//! `cargo xtask branch-inventory` regenerates the remote-branch inventory from
//! current GitHub/Git data and writes deterministic review artifacts plus a
//! separate deletion plan under `target/ripr/reports/`. The default mode is
//! read-only: it never deletes branches.
//!
//! `cargo xtask branch-inventory apply --plan <path> --digest <digest>` is the
//! only mutating path. It refuses a regenerated or changed plan (sha256 digest
//! over the canonical plan content), rechecks open PR heads and branch SHAs
//! immediately before each deletion, uses non-force ref deletion bound to
//! the rechecked SHA (`git push --force-with-lease=<ref>:<sha> origin
//! --delete`, never plain `--force`), refuses to run under CI
//! (`CI` / `GITHUB_ACTIONS` environment), and writes a cleanup receipt that
//! records deleted, skipped, changed, and failed branches with reasons.
//! Nothing wires the apply path into CI, hooks, or any other xtask command;
//! it is an explicit operator action on a reviewed plan.
//!
//! Classification goes through the all-state PR lookup by head branch name,
//! never through Git ancestry: squash merges mean a merged PR's branch
//! commits are not ancestors of `main`, so `git branch --merged` reachability
//! is recorded for context but is not the merged discriminator (#2024
//! grounding, 2026-07-22). The PR lookup paginates the full pull list
//! (`--paginate`), so nothing falls out of a fixed list window into a false
//! "no PR" verdict. Unknown always classifies `manual-review`, never a
//! deletion candidate.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::run::{run_output, run_output_owned};
use crate::{json_escape, write_report};

const CLASSIFICATION_PROTECTED: &str = "protected";
const CLASSIFICATION_ACTIVE: &str = "active";
const CLASSIFICATION_PARKED: &str = "parked";
const CLASSIFICATION_MERGED_PR_LEFTOVER: &str = "merged-pr-leftover";
const CLASSIFICATION_CLOSED_PR_LEFTOVER: &str = "closed-pr-leftover";
const CLASSIFICATION_UNOWNED: &str = "unowned";
const CLASSIFICATION_MANUAL_REVIEW: &str = "manual-review";

const DISPOSITION_KEEP: &str = "keep";
const DISPOSITION_DELETE_CANDIDATE: &str = "delete-candidate";
const DISPOSITION_MANUAL_REVIEW: &str = "manual-review";

const APPLY_DELETED: &str = "deleted";
const APPLY_SKIPPED: &str = "skipped";
const APPLY_CHANGED: &str = "changed";
const APPLY_FAILED: &str = "failed";

const CLAIMS_ISSUE: u64 = 2022;
const INPUT_SCHEMA_VERSION: &str = "1.0";
const PLAN_SCHEMA_VERSION: &str = "1.0";
const RECEIPT_SCHEMA_VERSION: &str = "1.0";
const MAX_GRAPHQL_PAGES: usize = 100;

#[derive(Clone, Debug)]
pub(crate) struct BranchFacts {
    name: String,
    head_sha: String,
    committed_date: Option<String>,
    author: Option<String>,
    committer: Option<String>,
    protected: bool,
    reachable_from_main: Option<bool>,
    lookup_error: Option<String>,
}

#[derive(Clone, Debug)]
pub(crate) struct PullRequestFacts {
    number: u64,
    state: String,
    merged: bool,
    head_ref: String,
    head_sha: String,
    title: String,
    issue_refs: Vec<u64>,
}

#[derive(Clone, Debug)]
pub(crate) struct ClaimFacts {
    source: String,
    branch: String,
    state: String,
}

#[derive(Clone, Debug)]
pub(crate) struct InventoryInput {
    repository: String,
    claims_available: bool,
    branches: Vec<BranchFacts>,
    pull_requests: Vec<PullRequestFacts>,
    claims: Vec<ClaimFacts>,
    warnings: Vec<String>,
}

#[derive(Clone, Debug)]
struct InventoryEntry {
    branch: BranchFacts,
    age_days: Option<i64>,
    matching_prs: Vec<u64>,
    issue_refs: Vec<u64>,
    active_pr_head: bool,
    claim_source: Option<String>,
    /// The specific merged PR whose head SHA matched this branch head at
    /// classification time. This is the identity recorded on a deletion plan;
    /// it must never be re-derived from the broader matching-PR set (a later
    /// closed-unmerged PR can share the head branch and must not be named on
    /// a destructive artifact).
    merged_pr_number: Option<u64>,
    classification: &'static str,
    disposition: &'static str,
    reason: String,
}

#[derive(Clone, Debug)]
struct PlanDeletion {
    branch: String,
    head_sha: String,
    merged_pr: u64,
    reason: String,
}

#[derive(Clone, Debug)]
struct LiveBranchState {
    lookup_error: Option<String>,
    exists: bool,
    head_sha: Option<String>,
    protected: bool,
    open_pr_head: bool,
}

#[derive(Clone, Debug)]
struct ApplyDecision {
    outcome: &'static str,
    reason: String,
}

#[derive(Clone, Debug)]
struct ApplyOutcome {
    branch: String,
    outcome: String,
    reason: String,
}

pub(crate) fn run(args: &[String]) -> Result<(), String> {
    if args.first().map(String::as_str) == Some("apply") {
        return apply_plan(&args[1..]);
    }
    inventory_report(args)
}

// ---------------------------------------------------------------------------
// Inventory mode (read-only)
// ---------------------------------------------------------------------------

struct InventoryArgs {
    input: Option<PathBuf>,
}

fn inventory_report(args: &[String]) -> Result<(), String> {
    let parsed = parse_inventory_args(args)?;
    let reference_epoch = now_epoch_seconds()?;
    let input = match &parsed.input {
        Some(path) => {
            let text = std::fs::read_to_string(path).map_err(|err| {
                format!(
                    "failed to read branch-inventory input {}: {err}",
                    path.display()
                )
            })?;
            parse_input_packet(&text)?
        }
        None => collect_live_input()?,
    };
    let mut entries = classify_all(&input, reference_epoch);
    entries.sort_by(|left, right| left.branch.name.cmp(&right.branch.name));

    let capture = input_packet_json(&input, reference_epoch);
    write_report("branch-inventory-input.json", &capture)?;

    let deletions = plan_deletions(&entries);
    let digest = plan_digest(&input.repository, &deletions);
    let inventory_json = inventory_json(&input, &entries, reference_epoch);
    write_report("branch-inventory.json", &inventory_json)?;
    let inventory_md = inventory_markdown(&input, &entries, &digest, reference_epoch);
    write_report("branch-inventory.md", &inventory_md)?;
    let plan_json = plan_json(&input, &deletions, &digest, reference_epoch);
    write_report("branch-inventory-plan.json", &plan_json)?;

    let counts = inventory_counts(&entries);
    println!(
        "branch-inventory: {} remote branches, {} delete-candidate(s), {} manual-review, plan digest {digest}",
        entries.len(),
        counts
            .by_disposition
            .get(DISPOSITION_DELETE_CANDIDATE)
            .copied()
            .unwrap_or(0),
        counts
            .by_disposition
            .get(DISPOSITION_MANUAL_REVIEW)
            .copied()
            .unwrap_or(0),
    );
    println!(
        "review target/ripr/reports/branch-inventory.md and branch-inventory-plan.json before any apply"
    );
    Ok(())
}

fn parse_inventory_args(args: &[String]) -> Result<InventoryArgs, String> {
    let mut input = None;
    let mut index = 0;
    while index < args.len() {
        let arg = &args[index];
        if arg == "--input" {
            let Some(value) = args.get(index + 1) else {
                return Err(
                    "cargo xtask branch-inventory requires a path after `--input`".to_string(),
                );
            };
            input = Some(PathBuf::from(value));
            index += 2;
            continue;
        }
        if arg == "--dry-run" {
            // Dry-run is the default and only behavior of inventory mode; the
            // flag is accepted so runbooks can state it explicitly.
            index += 1;
            continue;
        }
        return Err(format!(
            "unknown branch-inventory argument `{arg}`; use `cargo xtask branch-inventory [--input <path>] [--dry-run]` or `cargo xtask branch-inventory apply --plan <path> --digest <digest>`"
        ));
    }
    Ok(InventoryArgs { input })
}

// ---------------------------------------------------------------------------
// Classification (pure; unit-tested against fabricated fixtures)
// ---------------------------------------------------------------------------

fn classify_all(input: &InventoryInput, reference_epoch: i64) -> Vec<InventoryEntry> {
    input
        .branches
        .iter()
        .map(|branch| {
            let matching: Vec<&PullRequestFacts> = input
                .pull_requests
                .iter()
                .filter(|pr| pr.head_ref == branch.name)
                .collect();
            let claim = input
                .claims
                .iter()
                .find(|claim| claim.branch == branch.name);
            classify_branch(
                branch,
                &matching,
                claim,
                input.claims_available,
                reference_epoch,
            )
        })
        .collect()
}

fn classify_branch(
    facts: &BranchFacts,
    matching: &[&PullRequestFacts],
    claim: Option<&ClaimFacts>,
    claims_available: bool,
    reference_epoch: i64,
) -> InventoryEntry {
    let age_days = facts
        .committed_date
        .as_deref()
        .and_then(|date| parse_rfc3339_epoch_seconds(date).ok())
        .map(|committed| (reference_epoch - committed) / 86_400);
    let mut issue_refs: Vec<u64> = matching
        .iter()
        .flat_map(|pr| pr.issue_refs.iter().copied())
        .collect();
    issue_refs.sort_unstable();
    issue_refs.dedup();
    let matching_numbers: Vec<u64> = matching.iter().map(|pr| pr.number).collect();
    let open_pr = matching.iter().find(|pr| pr.state == "open");
    let merged_pr = matching
        .iter()
        .filter(|pr| pr.merged)
        .max_by_key(|pr| pr.number);
    let closed_pr = matching
        .iter()
        .filter(|pr| pr.state == "closed" && !pr.merged)
        .max_by_key(|pr| pr.number);

    let base = |classification: &'static str,
                disposition: &'static str,
                reason: String,
                merged_pr_number: Option<u64>| InventoryEntry {
        branch: facts.clone(),
        age_days,
        matching_prs: matching_numbers.clone(),
        issue_refs: issue_refs.clone(),
        active_pr_head: open_pr.is_some(),
        claim_source: claim.map(|claim| claim.source.clone()),
        merged_pr_number,
        classification,
        disposition,
        reason,
    };

    if is_authority_branch(&facts.name) || facts.protected {
        return base(
            CLASSIFICATION_PROTECTED,
            DISPOSITION_KEEP,
            "protected or release/source-promotion authority branch; never a deletion candidate"
                .to_string(),
            None,
        );
    }
    if let Some(error) = &facts.lookup_error {
        return base(
            CLASSIFICATION_MANUAL_REVIEW,
            DISPOSITION_MANUAL_REVIEW,
            format!(
                "GitHub lookup unavailable or ambiguous ({error}); unknown means manual-review"
            ),
            None,
        );
    }
    if let Some(pr) = open_pr {
        return base(
            CLASSIFICATION_ACTIVE,
            DISPOSITION_KEEP,
            format!("branch is the head of open PR #{}", pr.number),
            None,
        );
    }
    if let Some(claim) = claim {
        return base(
            CLASSIFICATION_PARKED,
            DISPOSITION_KEEP,
            format!(
                "branch is named by a #{CLAIMS_ISSUE} claim ({}, state {}); active/parked claims are never deletion candidates",
                claim.source, claim.state
            ),
            None,
        );
    }
    if let Some(pr) = merged_pr {
        if pr.head_sha == facts.head_sha {
            if !claims_available {
                return base(
                    CLASSIFICATION_MERGED_PR_LEFTOVER,
                    DISPOSITION_MANUAL_REVIEW,
                    format!(
                        "merged PR #{} matches this head branch and SHA, but the #{CLAIMS_ISSUE} claim lookup was unavailable; fail-closed to manual-review",
                        pr.number
                    ),
                    Some(pr.number),
                );
            }
            return base(
                CLASSIFICATION_MERGED_PR_LEFTOVER,
                DISPOSITION_DELETE_CANDIDATE,
                format!(
                    "merged PR #{} shares this head branch name and head SHA; squash merges leave the branch unreachable from main, so the merged-PR record — not Git ancestry — is the merged discriminator",
                    pr.number
                ),
                Some(pr.number),
            );
        }
        return base(
            CLASSIFICATION_MERGED_PR_LEFTOVER,
            DISPOSITION_MANUAL_REVIEW,
            format!(
                "merged PR #{} shares this head branch name, but the branch head SHA differs from the PR head SHA; unique commits may exist",
                pr.number
            ),
            Some(pr.number),
        );
    }
    if let Some(pr) = closed_pr {
        return base(
            CLASSIFICATION_CLOSED_PR_LEFTOVER,
            DISPOSITION_MANUAL_REVIEW,
            format!(
                "closed without merge (PR #{}); unique unmerged work requires manual review",
                pr.number
            ),
            None,
        );
    }
    base(
        CLASSIFICATION_UNOWNED,
        DISPOSITION_MANUAL_REVIEW,
        "no matching PR in the full all-state lookup and no claim; unknown means manual-review"
            .to_string(),
        None,
    )
}

fn is_authority_branch(name: &str) -> bool {
    name == "main"
        || name == "master"
        || name.starts_with("freeze/")
        || name.starts_with("release/")
}

fn branch_prefix(name: &str) -> String {
    match name.split_once('/') {
        Some((prefix, _)) => prefix.to_string(),
        None => "(no-prefix)".to_string(),
    }
}

fn age_bucket(age_days: Option<i64>) -> &'static str {
    match age_days {
        None => "unknown",
        Some(days) if days <= 30 => "0-30",
        Some(days) if days <= 90 => "31-90",
        Some(days) if days <= 180 => "91-180",
        Some(days) if days <= 365 => "181-365",
        Some(_) => "over-365",
    }
}

struct InventoryCounts {
    by_classification: BTreeMap<String, usize>,
    by_disposition: BTreeMap<String, usize>,
    by_prefix: BTreeMap<String, usize>,
    by_age_bucket: BTreeMap<String, usize>,
}

fn inventory_counts(entries: &[InventoryEntry]) -> InventoryCounts {
    let mut counts = InventoryCounts {
        by_classification: BTreeMap::new(),
        by_disposition: BTreeMap::new(),
        by_prefix: BTreeMap::new(),
        by_age_bucket: BTreeMap::new(),
    };
    for entry in entries {
        *counts
            .by_classification
            .entry(entry.classification.to_string())
            .or_insert(0) += 1;
        *counts
            .by_disposition
            .entry(entry.disposition.to_string())
            .or_insert(0) += 1;
        *counts
            .by_prefix
            .entry(branch_prefix(&entry.branch.name))
            .or_insert(0) += 1;
        *counts
            .by_age_bucket
            .entry(age_bucket(entry.age_days).to_string())
            .or_insert(0) += 1;
    }
    counts
}

// ---------------------------------------------------------------------------
// Deletion plan and digest
// ---------------------------------------------------------------------------

fn plan_deletions(entries: &[InventoryEntry]) -> Vec<PlanDeletion> {
    let mut deletions: Vec<PlanDeletion> = entries
        .iter()
        .filter(|entry| entry.disposition == DISPOSITION_DELETE_CANDIDATE)
        .filter_map(|entry| {
            // The plan must name the specific merged PR that matched at
            // classification time, never a re-derived max over all matching
            // PRs (a later closed-unmerged PR on the same head branch would
            // otherwise be recorded as the merge evidence). A delete
            // candidate always carries this number; if it is somehow absent,
            // fail closed and leave the branch out of the plan.
            entry.merged_pr_number.map(|merged_pr| PlanDeletion {
                branch: entry.branch.name.clone(),
                head_sha: entry.branch.head_sha.clone(),
                merged_pr,
                reason: entry.reason.clone(),
            })
        })
        .collect();
    deletions.sort_by(|left, right| left.branch.cmp(&right.branch));
    deletions
}

/// Canonical plan content: one header line, the repository line, then one
/// `branch|head_sha|merged_pr|reason` line per deletion, sorted by branch.
/// Sorting happens here (not only at plan construction) so the canonical form
/// is order-independent at the digest layer itself. The timestamp is
/// deliberately excluded so the digest is stable across regeneration of
/// identical review content.
fn plan_canonical_content(repository: &str, deletions: &[PlanDeletion]) -> String {
    let mut sorted: Vec<&PlanDeletion> = deletions.iter().collect();
    sorted.sort_by(|left, right| left.branch.cmp(&right.branch));
    let mut lines = vec![
        "branch-inventory-deletion-plan v1".to_string(),
        format!("repository: {repository}"),
    ];
    for deletion in sorted {
        lines.push(format!(
            "{}|{}|{}|{}",
            deletion.branch, deletion.head_sha, deletion.merged_pr, deletion.reason
        ));
    }
    lines.join("\n")
}

fn plan_digest(repository: &str, deletions: &[PlanDeletion]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(plan_canonical_content(repository, deletions).as_bytes());
    format!("sha256:{:x}", hasher.finalize())
}

fn parse_plan_deletions(plan: &Value) -> Result<Vec<PlanDeletion>, String> {
    let items = plan
        .get("deletions")
        .and_then(Value::as_array)
        .ok_or_else(|| "deletion plan is missing a `deletions` array".to_string())?;
    let mut deletions = Vec::new();
    for item in items {
        let branch = json_string(item, "branch", "plan deletion")?;
        let head_sha = json_string(item, "head_sha", "plan deletion")?;
        let merged_pr = item
            .get("merged_pr")
            .and_then(Value::as_u64)
            .ok_or_else(|| "plan deletion is missing numeric `merged_pr`".to_string())?;
        let reason = json_string(item, "reason", "plan deletion")?;
        deletions.push(PlanDeletion {
            branch,
            head_sha,
            merged_pr,
            reason,
        });
    }
    deletions.sort_by(|left, right| left.branch.cmp(&right.branch));
    Ok(deletions)
}

// ---------------------------------------------------------------------------
// Apply mode (explicit operator action; never wired into CI)
// ---------------------------------------------------------------------------

struct ApplyArgs {
    plan: PathBuf,
    digest: String,
}

fn apply_plan(args: &[String]) -> Result<(), String> {
    // Structural CI guard: branch deletion is an explicit operator action on
    // a reviewed plan, so the apply path refuses to run under CI even if a
    // future change wires the command into a workflow by mistake.
    if std::env::var_os("CI").is_some() || std::env::var_os("GITHUB_ACTIONS").is_some() {
        return Err(
            "branch-inventory apply refuses to run under CI (CI/GITHUB_ACTIONS is set); branch deletion is an explicit operator action on a reviewed plan"
                .to_string(),
        );
    }
    let parsed = parse_apply_args(args)?;
    let text = std::fs::read_to_string(&parsed.plan).map_err(|err| {
        format!(
            "failed to read deletion plan {}: {err}",
            parsed.plan.display()
        )
    })?;
    let plan: Value = serde_json::from_str(&text)
        .map_err(|err| format!("failed to parse deletion plan JSON: {err}"))?;
    let repository = json_string(&plan, "repository", "deletion plan")?;
    let deletions = parse_plan_deletions(&plan)?;
    let computed = plan_digest(&repository, &deletions);
    let embedded = json_string(&plan, "digest", "deletion plan")?;
    if embedded != computed {
        return Err(format!(
            "deletion plan digest mismatch: plan embeds {embedded} but its content recomputes to {computed}; regenerate and re-review the plan"
        ));
    }
    if parsed.digest != computed {
        return Err(format!(
            "deletion plan digest mismatch: `--digest {}` does not match recomputed {computed}; apply refuses a regenerated or changed plan",
            parsed.digest
        ));
    }
    let live_repository = gh_repository()?;
    if live_repository != repository {
        return Err(format!(
            "deletion plan targets {repository} but gh resolves {live_repository}; refusing to apply a plan to a different repository"
        ));
    }

    let mut outcomes = Vec::new();
    for deletion in &deletions {
        let state = recheck_branch(&repository, &deletion.branch);
        let decision = decide_apply(deletion, &state);
        let outcome = if decision.outcome == APPLY_DELETED {
            match delete_remote_branch(&deletion.branch, &deletion.head_sha) {
                Ok(()) => ApplyOutcome {
                    branch: deletion.branch.clone(),
                    outcome: APPLY_DELETED.to_string(),
                    reason: decision.reason.clone(),
                },
                Err(err) => ApplyOutcome {
                    branch: deletion.branch.clone(),
                    outcome: APPLY_FAILED.to_string(),
                    reason: format!("SHA-bound non-force ref deletion failed: {err}"),
                },
            }
        } else {
            ApplyOutcome {
                branch: deletion.branch.clone(),
                outcome: decision.outcome.to_string(),
                reason: decision.reason.clone(),
            }
        };
        outcomes.push(outcome);
    }

    let receipt_json = receipt_json(&repository, &computed, &outcomes);
    write_report("branch-inventory-cleanup.json", &receipt_json)?;
    let receipt_md = receipt_markdown(&repository, &computed, &outcomes);
    write_report("branch-inventory-cleanup.md", &receipt_md)?;

    let failed = outcomes
        .iter()
        .filter(|outcome| outcome.outcome == APPLY_FAILED)
        .count();
    println!(
        "branch-inventory apply: {} deleted, {} skipped, {} changed, {} failed (receipt: target/ripr/reports/branch-inventory-cleanup.md)",
        outcomes
            .iter()
            .filter(|outcome| outcome.outcome == APPLY_DELETED)
            .count(),
        outcomes
            .iter()
            .filter(|outcome| outcome.outcome == APPLY_SKIPPED)
            .count(),
        outcomes
            .iter()
            .filter(|outcome| outcome.outcome == APPLY_CHANGED)
            .count(),
        failed,
    );
    if failed > 0 {
        return Err(format!(
            "branch-inventory apply recorded {failed} failed deletion(s); see the cleanup receipt"
        ));
    }
    Ok(())
}

fn parse_apply_args(args: &[String]) -> Result<ApplyArgs, String> {
    let mut plan = None;
    let mut digest = None;
    let mut index = 0;
    while index < args.len() {
        let arg = &args[index];
        if arg == "--plan" {
            let Some(value) = args.get(index + 1) else {
                return Err(
                    "cargo xtask branch-inventory apply requires a path after `--plan`".to_string(),
                );
            };
            plan = Some(PathBuf::from(value));
            index += 2;
            continue;
        }
        if arg == "--digest" {
            let Some(value) = args.get(index + 1) else {
                return Err(
                    "cargo xtask branch-inventory apply requires a digest after `--digest`"
                        .to_string(),
                );
            };
            digest = Some(value.clone());
            index += 2;
            continue;
        }
        return Err(format!(
            "unknown branch-inventory apply argument `{arg}`; use `cargo xtask branch-inventory apply --plan <path> --digest <digest>`"
        ));
    }
    match (plan, digest) {
        (Some(plan), Some(digest)) => Ok(ApplyArgs { plan, digest }),
        _ => Err(
            "cargo xtask branch-inventory apply requires both `--plan <path>` and `--digest <digest>`"
                .to_string(),
        ),
    }
}

/// Decide one plan entry against freshly rechecked state. Fail-closed: any
/// lookup ambiguity, drift, or new protection means no deletion.
fn decide_apply(deletion: &PlanDeletion, state: &LiveBranchState) -> ApplyDecision {
    if let Some(error) = &state.lookup_error {
        return ApplyDecision {
            outcome: APPLY_FAILED,
            reason: format!(
                "pre-deletion recheck failed ({error}); refusing to delete on ambiguous state"
            ),
        };
    }
    if !state.exists {
        return ApplyDecision {
            outcome: APPLY_CHANGED,
            reason: "branch no longer exists on the remote (renamed or already deleted since plan generation)"
                .to_string(),
        };
    }
    if state.protected {
        return ApplyDecision {
            outcome: APPLY_SKIPPED,
            reason: "branch became protected since plan generation".to_string(),
        };
    }
    if state.open_pr_head {
        return ApplyDecision {
            outcome: APPLY_SKIPPED,
            reason: "branch is now the head of an open PR".to_string(),
        };
    }
    match &state.head_sha {
        Some(sha) if sha == &deletion.head_sha => ApplyDecision {
            outcome: APPLY_DELETED,
            reason: format!(
                "rechecked head SHA {sha} matches the reviewed plan; non-force ref deletion bound to that SHA via --force-with-lease"
            ),
        },
        Some(sha) => ApplyDecision {
            outcome: APPLY_CHANGED,
            reason: format!(
                "branch head SHA moved since plan generation ({} -> {sha}); the reviewed plan no longer matches",
                deletion.head_sha
            ),
        },
        None => ApplyDecision {
            outcome: APPLY_FAILED,
            reason:
                "pre-deletion recheck returned no head SHA; refusing to delete on ambiguous state"
                    .to_string(),
        },
    }
}

/// Non-force remote ref deletion bound to the rechecked head SHA.
/// `--force-with-lease=<ref>:<expect>` with an explicit expected value works
/// with `--delete` (verified against git 2.43.0: a matching SHA deletes, a
/// stale SHA is rejected with "stale info" and the ref survives), so the
/// deletion is rejected if the ref moved between the pre-deletion recheck
/// and the push. Plain `--force` is never used on this path.
fn delete_branch_argv(branch: &str, expected_sha: &str) -> Vec<String> {
    vec![
        "push".to_string(),
        format!("--force-with-lease=refs/heads/{branch}:{expected_sha}"),
        "origin".to_string(),
        "--delete".to_string(),
        branch.to_string(),
    ]
}

fn delete_remote_branch(branch: &str, expected_sha: &str) -> Result<(), String> {
    let args = delete_branch_argv(branch, expected_sha);
    let argv: Vec<&str> = args.iter().map(String::as_str).collect();
    crate::run::run("git", &argv).map(|_| ())
}

// ---------------------------------------------------------------------------
// Live collection (gh CLI; read-only except apply's explicit deletion)
// ---------------------------------------------------------------------------

fn collect_live_input() -> Result<InventoryInput, String> {
    let repository = gh_repository()?;
    let branch_flags = gh_rest_branch_flags(&repository)?;
    let ref_metadata = gh_graphql_ref_metadata(&repository)?;
    let pull_requests = gh_all_pull_requests(&repository)?;
    let (reachability, reachability_available) = local_reachability();
    let mut warnings = Vec::new();
    if !reachability_available {
        warnings.push(
            "local origin/* refs were unavailable; reachable_from_main is recorded as unknown (reachability is contextual only, never the merged discriminator)"
                .to_string(),
        );
    }

    let mut branches = Vec::new();
    for (name, (head_sha, protected)) in &branch_flags {
        let (committed_date, author, committer, lookup_error) = match ref_metadata.get(name) {
            Some(metadata) => (
                metadata.committed_date.clone(),
                metadata.author.clone(),
                metadata.committer.clone(),
                None,
            ),
            None => (
                None,
                None,
                None,
                Some("branch missing from the GraphQL refs lookup".to_string()),
            ),
        };
        let reachable_from_main = if reachability_available {
            Some(reachability.contains_key(name))
        } else {
            None
        };
        branches.push(BranchFacts {
            name: name.clone(),
            head_sha: head_sha.clone(),
            committed_date,
            author,
            committer,
            protected: *protected,
            reachable_from_main,
            lookup_error,
        });
    }
    branches.sort_by(|left, right| left.name.cmp(&right.name));

    let branch_names: Vec<String> = branches.iter().map(|branch| branch.name.clone()).collect();
    let (claims, claims_available) = match collect_claims(&branch_names) {
        Ok(claims) => (claims, true),
        Err(err) => {
            warnings.push(format!(
                "#{CLAIMS_ISSUE} claim lookup failed ({err}); classification fails closed: merged-PR leftovers degrade to manual-review"
            ));
            (Vec::new(), false)
        }
    };

    Ok(InventoryInput {
        repository,
        claims_available,
        branches,
        pull_requests,
        claims,
        warnings,
    })
}

fn gh_repository() -> Result<String, String> {
    let output = run_output("gh", &["repo", "view", "--json", "nameWithOwner"])?;
    let value: Value = serde_json::from_str(&output)
        .map_err(|err| format!("failed to parse gh repo JSON: {err}"))?;
    value
        .get("nameWithOwner")
        .and_then(Value::as_str)
        .map(str::to_string)
        .ok_or_else(|| format!("gh repo JSON is missing nameWithOwner: {value}"))
}

/// REST branch list: name -> (head sha, protected). `--paginate` retrieves
/// every page, so no branch falls out of a fixed list window; `--jq .[]`
/// flattens each page to one compact JSON object per line.
fn gh_rest_branch_flags(repository: &str) -> Result<BTreeMap<String, (String, bool)>, String> {
    let endpoint = format!("repos/{repository}/branches?per_page=100");
    let output = run_output_owned(
        "gh",
        &[
            "api".to_string(),
            endpoint,
            "--paginate".to_string(),
            "--jq".to_string(),
            ".[]".to_string(),
        ],
    )?;
    let mut flags = BTreeMap::new();
    for item in parse_paginated_items(&output, "branches")? {
        let name = json_string(&item, "name", "REST branch")?;
        let head_sha = item
            .get("commit")
            .and_then(|commit| commit.get("sha"))
            .and_then(Value::as_str)
            .ok_or_else(|| "REST branch is missing commit.sha".to_string())?
            .to_string();
        let protected = item
            .get("protected")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        flags.insert(name, (head_sha, protected));
    }
    Ok(flags)
}

struct RefCommitMetadata {
    committed_date: Option<String>,
    author: Option<String>,
    committer: Option<String>,
}

const REFS_QUERY: &str = "query($owner: String!, $name: String!, $after: String) { repository(owner: $owner, name: $name) { refs(refPrefix: \"refs/heads/\", first: 100, after: $after) { nodes { name target { ... on Commit { oid committedDate author { name email user { login } } committer { name email user { login } } } } } pageInfo { hasNextPage endCursor } } } }";

/// GraphQL refs walk with an explicit cursor loop (bounded), so commit
/// metadata covers every head ref rather than a fixed window.
fn gh_graphql_ref_metadata(
    repository: &str,
) -> Result<BTreeMap<String, RefCommitMetadata>, String> {
    let (owner, name) = repository.split_once('/').ok_or_else(|| {
        format!("repository `{repository}` must be in owner/name form for the refs lookup")
    })?;
    let mut metadata = BTreeMap::new();
    let mut after: Option<String> = None;
    for _ in 0..MAX_GRAPHQL_PAGES {
        let mut args = vec![
            "api".to_string(),
            "graphql".to_string(),
            "-f".to_string(),
            format!("query={REFS_QUERY}"),
            "-f".to_string(),
            format!("owner={owner}"),
            "-f".to_string(),
            format!("name={name}"),
        ];
        if let Some(cursor) = &after {
            args.push("-f".to_string());
            args.push(format!("after={cursor}"));
        }
        let output = run_output_owned("gh", &args)?;
        let value: Value = serde_json::from_str(&output)
            .map_err(|err| format!("failed to parse gh refs GraphQL JSON: {err}"))?;
        let refs = value
            .get("data")
            .and_then(|data| data.get("repository"))
            .and_then(|repo| repo.get("refs"))
            .ok_or_else(|| {
                format!("gh refs GraphQL JSON is missing data.repository.refs: {value}")
            })?;
        let nodes = refs
            .get("nodes")
            .and_then(Value::as_array)
            .ok_or_else(|| "gh refs GraphQL JSON is missing refs.nodes".to_string())?;
        for node in nodes {
            let branch_name = node
                .get("name")
                .and_then(Value::as_str)
                .ok_or_else(|| "gh refs node is missing name".to_string())?
                .to_string();
            let target = node.get("target").cloned().unwrap_or(Value::Null);
            let committed_date = target
                .get("committedDate")
                .and_then(Value::as_str)
                .map(str::to_string);
            let author = target.get("author").and_then(graphql_actor_label);
            let committer = target.get("committer").and_then(graphql_actor_label);
            metadata.insert(
                branch_name,
                RefCommitMetadata {
                    committed_date,
                    author,
                    committer,
                },
            );
        }
        let page_info = refs
            .get("pageInfo")
            .ok_or_else(|| "gh refs GraphQL JSON is missing refs.pageInfo".to_string())?;
        let has_next = page_info
            .get("hasNextPage")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        if !has_next {
            return Ok(metadata);
        }
        after = Some(
            page_info
                .get("endCursor")
                .and_then(Value::as_str)
                .ok_or_else(|| "gh refs GraphQL pageInfo is missing endCursor".to_string())?
                .to_string(),
        );
    }
    Err(format!(
        "gh refs GraphQL pagination exceeded {MAX_GRAPHQL_PAGES} pages; refusing a partial inventory"
    ))
}

fn graphql_actor_label(actor: &Value) -> Option<String> {
    actor
        .get("user")
        .and_then(|user| user.get("login"))
        .and_then(Value::as_str)
        .or_else(|| actor.get("name").and_then(Value::as_str))
        .or_else(|| actor.get("email").and_then(Value::as_str))
        .map(str::to_string)
}

/// All-state PR list via the REST pulls endpoint with full pagination. This
/// is the merged discriminator: matching goes by head branch name, and only
/// same-repository heads count (fork heads never alias an origin branch).
fn gh_all_pull_requests(repository: &str) -> Result<Vec<PullRequestFacts>, String> {
    let endpoint = format!("repos/{repository}/pulls?state=all&per_page=100");
    let output = run_output_owned(
        "gh",
        &[
            "api".to_string(),
            endpoint,
            "--paginate".to_string(),
            "--jq".to_string(),
            ".[]".to_string(),
        ],
    )?;
    let items = parse_paginated_items(&output, "pulls")?;
    pull_request_facts_from_items(&items, repository)
}

/// Build PR facts from raw pulls items. Only same-repository heads count:
/// GitHub sets `head.repo.full_name` to the repository the head branch
/// belongs to, so a value equal to `repository` always names a branch in
/// this repository. Fork heads and deleted forks (`head.repo: null`) are
/// excluded — the failure direction is conservative (an origin branch whose
/// only PR history is a fork PR classifies unowned/manual-review, never a
/// deletion candidate).
fn pull_request_facts_from_items(
    items: &[Value],
    repository: &str,
) -> Result<Vec<PullRequestFacts>, String> {
    let mut prs = Vec::new();
    for item in items {
        let head = item.get("head").cloned().unwrap_or(Value::Null);
        let same_repository = head
            .get("repo")
            .and_then(|repo| repo.get("full_name"))
            .and_then(Value::as_str)
            == Some(repository);
        if !same_repository {
            continue;
        }
        let number = item
            .get("number")
            .and_then(Value::as_u64)
            .ok_or_else(|| "pulls item is missing numeric `number`".to_string())?;
        let state = json_string(item, "state", "pulls item")?;
        let merged = item.get("merged_at").is_some_and(|value| !value.is_null());
        let head_ref = json_string(&head, "ref", "pulls item head")?;
        let head_sha = json_string(&head, "sha", "pulls item head")?;
        let title = item
            .get("title")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        let body = item
            .get("body")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        let mut issue_refs = extract_issue_refs(&title);
        issue_refs.extend(extract_issue_refs(&body));
        issue_refs.sort_unstable();
        issue_refs.dedup();
        prs.push(PullRequestFacts {
            number,
            state,
            merged,
            head_ref,
            head_sha,
            title,
            issue_refs,
        });
    }
    prs.sort_by_key(|pr| pr.number);
    Ok(prs)
}

/// Parse the `gh api --paginate --jq .[]` output shape: one compact JSON
/// object per line, across every page. Full pagination means the item count
/// here is the whole list, not a fixed window.
fn parse_paginated_items(text: &str, label: &str) -> Result<Vec<Value>, String> {
    let mut items = Vec::new();
    for (index, line) in text.lines().enumerate() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let value: Value = serde_json::from_str(trimmed)
            .map_err(|err| format!("failed to parse gh {label} JSON line {}: {err}", index + 1))?;
        items.push(value);
    }
    Ok(items)
}

/// Conservative #2022 claim lookup: a comment that names a branch as a whole
/// token counts as a claim. Over-protection is intentional — a stale mention
/// parks a branch; it never marks one for deletion.
///
/// Matching rule (see #2022's claim fields: "branch and worktree when
/// created", and the repo convention that working branches carry a
/// `prefix/`): a claim match requires either a backtick-quoted occurrence of
/// the exact branch name (`` `fix` ``), or — for slash-prefixed names only —
/// an unquoted occurrence bounded on both sides by characters outside the
/// branch-name alphabet (`[A-Za-z0-9-_+/]`). A bare unquoted single-segment
/// name never matches: prose like "fix the bug" must not park a branch named
/// `fix`, while realistic claim blocks name branches backtick-quoted or with
/// their prefix.
fn collect_claims(branch_names: &[String]) -> Result<Vec<ClaimFacts>, String> {
    let output = run_output(
        "gh",
        &[
            "issue",
            "view",
            &CLAIMS_ISSUE.to_string(),
            "--json",
            "comments",
        ],
    )?;
    let value: Value = serde_json::from_str(&output)
        .map_err(|err| format!("failed to parse gh issue comments JSON: {err}"))?;
    let comments = value
        .get("comments")
        .and_then(Value::as_array)
        .ok_or_else(|| "gh issue comments JSON is missing `comments`".to_string())?;
    let mut claims: Vec<ClaimFacts> = Vec::new();
    for comment in comments {
        let body = comment.get("body").and_then(Value::as_str).unwrap_or("");
        let id = comment
            .get("id")
            .and_then(Value::as_str)
            .unwrap_or("unknown");
        for name in branch_names {
            if name == "main" || claims.iter().any(|claim| claim.branch == *name) {
                continue;
            }
            if comment_claims_branch(body, name) {
                claims.push(ClaimFacts {
                    source: format!("issue:{CLAIMS_ISSUE} comment {id}"),
                    branch: name.clone(),
                    state: "active".to_string(),
                });
            }
        }
    }
    claims.sort_by(|left, right| left.branch.cmp(&right.branch));
    Ok(claims)
}

/// Characters that can extend a branch reference: a neighbor in this alphabet
/// means the occurrence is part of a longer token (e.g. `hotfix`, `xcodex/a`,
/// `codex/a/b`), not a standalone branch mention. `.` is deliberately outside
/// the alphabet so sentence-final periods still count as boundaries; the
/// resulting over-match parks (fail-safe) and never deletes.
fn is_branch_token_char(ch: char) -> bool {
    ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '+' | '/')
}

/// Whole-token claim match: backtick-quoted exact name always matches;
/// unquoted matches require a slash-prefixed name bounded by non-branch
/// characters on both sides. Bare unquoted single-segment names never match.
fn comment_claims_branch(body: &str, name: &str) -> bool {
    if name.is_empty() {
        return false;
    }
    if body.contains(&format!("`{name}`")) {
        return true;
    }
    if !name.contains('/') {
        return false;
    }
    body.match_indices(name).any(|(start, _)| {
        let before = body[..start].chars().next_back();
        let after = body[start + name.len()..].chars().next();
        before.is_none_or(|ch| !is_branch_token_char(ch))
            && after.is_none_or(|ch| !is_branch_token_char(ch))
    })
}

/// Reachability of each origin branch from origin/main, for context only.
/// Squash merges make this useless as a merged discriminator, so it is
/// recorded but never consulted by classification.
fn local_reachability() -> (BTreeMap<String, bool>, bool) {
    let Ok(refs) = run_output(
        "git",
        &[
            "for-each-ref",
            "--format=%(refname:short)",
            "refs/remotes/origin",
        ],
    ) else {
        return (BTreeMap::new(), false);
    };
    let Ok(merged) = run_output(
        "git",
        &[
            "branch",
            "-r",
            "--merged",
            "origin/main",
            "--format=%(refname:short)",
        ],
    ) else {
        return (BTreeMap::new(), false);
    };
    let merged_names: std::collections::BTreeSet<&str> = merged
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect();
    let mut reachability = BTreeMap::new();
    for line in refs.lines() {
        let short = line.trim();
        let Some(name) = short.strip_prefix("origin/") else {
            continue;
        };
        if name == "HEAD" {
            continue;
        }
        reachability.insert(name.to_string(), merged_names.contains(short));
    }
    (reachability, true)
}

fn recheck_branch(repository: &str, branch: &str) -> LiveBranchState {
    let endpoint = format!("repos/{repository}/branches/{branch}");
    let branch_result = run_output_owned("gh", &["api".to_string(), endpoint]);
    let (exists, head_sha, protected, lookup_error) = match branch_result {
        Ok(output) => match serde_json::from_str::<Value>(&output) {
            Ok(value) => {
                let sha = value
                    .get("commit")
                    .and_then(|commit| commit.get("sha"))
                    .and_then(Value::as_str)
                    .map(str::to_string);
                let protected = value
                    .get("protected")
                    .and_then(Value::as_bool)
                    .unwrap_or(false);
                (true, sha, protected, None)
            }
            Err(err) => (
                false,
                None,
                false,
                Some(format!("unparseable branch recheck JSON: {err}")),
            ),
        },
        Err(err) if err.contains("404") => (false, None, false, None),
        Err(err) => (false, None, false, Some(err)),
    };
    let open_pr_head = match run_output_owned(
        "gh",
        &[
            "pr".to_string(),
            "list".to_string(),
            "--repo".to_string(),
            repository.to_string(),
            "--state".to_string(),
            "open".to_string(),
            "--head".to_string(),
            branch.to_string(),
            "--json".to_string(),
            "number".to_string(),
            "--limit".to_string(),
            "10".to_string(),
        ],
    ) {
        Ok(output) => serde_json::from_str::<Value>(&output)
            .ok()
            .and_then(|value| value.as_array().map(|items| !items.is_empty()))
            .unwrap_or(false),
        Err(_) => false,
    };
    LiveBranchState {
        lookup_error,
        exists,
        head_sha,
        protected,
        open_pr_head,
    }
}

// ---------------------------------------------------------------------------
// Input packet (--input mode and the live-run capture)
// ---------------------------------------------------------------------------

fn parse_input_packet(text: &str) -> Result<InventoryInput, String> {
    let value: Value = serde_json::from_str(text)
        .map_err(|err| format!("failed to parse branch-inventory input JSON: {err}"))?;
    let schema_version = json_string(&value, "schema_version", "input packet")?;
    if schema_version != INPUT_SCHEMA_VERSION {
        return Err(format!(
            "branch-inventory input schema_version `{schema_version}` is not supported (expected `{INPUT_SCHEMA_VERSION}`)"
        ));
    }
    let repository = json_string(&value, "repository", "input packet")?;
    let claims_available = value
        .get("claims_available")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let mut branches = Vec::new();
    for item in json_array(&value, "branches", "input packet")? {
        branches.push(BranchFacts {
            name: json_string(item, "name", "input branch")?,
            head_sha: json_string(item, "head_sha", "input branch")?,
            committed_date: json_optional_string(item, "committed_date"),
            author: json_optional_string(item, "author"),
            committer: json_optional_string(item, "committer"),
            protected: item
                .get("protected")
                .and_then(Value::as_bool)
                .unwrap_or(false),
            reachable_from_main: item.get("reachable_from_main").and_then(Value::as_bool),
            lookup_error: json_optional_string(item, "lookup_error"),
        });
    }
    let mut pull_requests = Vec::new();
    for item in json_array(&value, "pull_requests", "input packet")? {
        let number = item
            .get("number")
            .and_then(Value::as_u64)
            .ok_or_else(|| "input pull_request is missing numeric `number`".to_string())?;
        let issue_refs = item
            .get("issue_refs")
            .and_then(Value::as_array)
            .map(|refs| refs.iter().filter_map(Value::as_u64).collect())
            .unwrap_or_default();
        pull_requests.push(PullRequestFacts {
            number,
            state: json_string(item, "state", "input pull_request")?,
            merged: item.get("merged").and_then(Value::as_bool).unwrap_or(false),
            head_ref: json_string(item, "head_ref", "input pull_request")?,
            head_sha: json_string(item, "head_sha", "input pull_request")?,
            title: item
                .get("title")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string(),
            issue_refs,
        });
    }
    let mut claims = Vec::new();
    for item in json_array(&value, "claims", "input packet")? {
        claims.push(ClaimFacts {
            source: json_string(item, "source", "input claim")?,
            branch: json_string(item, "branch", "input claim")?,
            state: item
                .get("state")
                .and_then(Value::as_str)
                .unwrap_or("active")
                .to_string(),
        });
    }
    let warnings = value
        .get("warnings")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(|item| item.as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default();
    branches.sort_by(|left, right| left.name.cmp(&right.name));
    Ok(InventoryInput {
        repository,
        claims_available,
        branches,
        pull_requests,
        claims,
        warnings,
    })
}

fn json_string(value: &Value, key: &str, label: &str) -> Result<String, String> {
    value
        .get(key)
        .and_then(Value::as_str)
        .map(str::to_string)
        .ok_or_else(|| format!("{label} is missing string `{key}`"))
}

fn json_optional_string(value: &Value, key: &str) -> Option<String> {
    value.get(key).and_then(Value::as_str).map(str::to_string)
}

fn json_array<'a>(value: &'a Value, key: &str, label: &str) -> Result<&'a [Value], String> {
    value
        .get(key)
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .ok_or_else(|| format!("{label} is missing array `{key}`"))
}

// ---------------------------------------------------------------------------
// Rendering (deterministic JSON + Markdown review artifacts)
// ---------------------------------------------------------------------------

fn json_opt_string(value: Option<&str>) -> String {
    match value {
        Some(text) => format!("\"{}\"", json_escape(text)),
        None => "null".to_string(),
    }
}

fn json_opt_bool(value: Option<bool>) -> String {
    match value {
        Some(flag) => flag.to_string(),
        None => "null".to_string(),
    }
}

fn json_opt_i64(value: Option<i64>) -> String {
    match value {
        Some(number) => number.to_string(),
        None => "null".to_string(),
    }
}

fn json_u64_array(values: &[u64]) -> String {
    let inner: Vec<String> = values.iter().map(u64::to_string).collect();
    format!("[{}]", inner.join(", "))
}

fn counts_json(counts: &BTreeMap<String, usize>, indent: &str) -> String {
    if counts.is_empty() {
        return "{}".to_string();
    }
    let inner: Vec<String> = counts
        .iter()
        .map(|(key, count)| format!("{indent}  \"{}\": {count}", json_escape(key)))
        .collect();
    format!("{{\n{}\n{indent}}}", inner.join(",\n"))
}

fn input_packet_json(input: &InventoryInput, reference_epoch: i64) -> String {
    let mut body = String::new();
    body.push_str("{\n");
    body.push_str(&format!(
        "  \"schema_version\": \"{INPUT_SCHEMA_VERSION}\",\n  \"kind\": \"branch-inventory-input\",\n  \"repository\": \"{}\",\n  \"captured_at_epoch_seconds\": {reference_epoch},\n  \"captured_at\": \"{}\",\n  \"claims_available\": {},\n",
        json_escape(&input.repository),
        rfc3339_from_epoch_seconds(reference_epoch),
        input.claims_available,
    ));
    body.push_str("  \"warnings\": [");
    let warnings: Vec<String> = input
        .warnings
        .iter()
        .map(|warning| format!("\"{}\"", json_escape(warning)))
        .collect();
    body.push_str(&warnings.join(", "));
    body.push_str("],\n  \"branches\": [\n");
    let branches: Vec<String> = input
        .branches
        .iter()
        .map(|branch| {
            format!(
                "    {{\n      \"name\": \"{}\",\n      \"head_sha\": \"{}\",\n      \"committed_date\": {},\n      \"author\": {},\n      \"committer\": {},\n      \"protected\": {},\n      \"reachable_from_main\": {},\n      \"lookup_error\": {}\n    }}",
                json_escape(&branch.name),
                json_escape(&branch.head_sha),
                json_opt_string(branch.committed_date.as_deref()),
                json_opt_string(branch.author.as_deref()),
                json_opt_string(branch.committer.as_deref()),
                branch.protected,
                json_opt_bool(branch.reachable_from_main),
                json_opt_string(branch.lookup_error.as_deref()),
            )
        })
        .collect();
    body.push_str(&branches.join(",\n"));
    body.push_str("\n  ],\n  \"pull_requests\": [\n");
    let prs: Vec<String> = input
        .pull_requests
        .iter()
        .map(|pr| {
            format!(
                "    {{\n      \"number\": {},\n      \"state\": \"{}\",\n      \"merged\": {},\n      \"head_ref\": \"{}\",\n      \"head_sha\": \"{}\",\n      \"title\": \"{}\",\n      \"issue_refs\": {}\n    }}",
                pr.number,
                json_escape(&pr.state),
                pr.merged,
                json_escape(&pr.head_ref),
                json_escape(&pr.head_sha),
                json_escape(&pr.title),
                json_u64_array(&pr.issue_refs),
            )
        })
        .collect();
    body.push_str(&prs.join(",\n"));
    body.push_str("\n  ],\n  \"claims\": [\n");
    let claims: Vec<String> = input
        .claims
        .iter()
        .map(|claim| {
            format!(
                "    {{\n      \"source\": \"{}\",\n      \"branch\": \"{}\",\n      \"state\": \"{}\"\n    }}",
                json_escape(&claim.source),
                json_escape(&claim.branch),
                json_escape(&claim.state),
            )
        })
        .collect();
    body.push_str(&claims.join(",\n"));
    body.push_str("\n  ]\n}\n");
    body
}

fn inventory_json(
    input: &InventoryInput,
    entries: &[InventoryEntry],
    reference_epoch: i64,
) -> String {
    let counts = inventory_counts(entries);
    let mut body = String::new();
    body.push_str("{\n");
    body.push_str(&format!(
        "  \"schema_version\": \"{INPUT_SCHEMA_VERSION}\",\n  \"kind\": \"branch-inventory\",\n  \"repository\": \"{}\",\n  \"generated_at_epoch_seconds\": {reference_epoch},\n  \"generated_at\": \"{}\",\n  \"mode\": \"read-only inventory; nothing was deleted\",\n",
        json_escape(&input.repository),
        rfc3339_from_epoch_seconds(reference_epoch),
    ));
    body.push_str("  \"methodology\": [\n");
    body.push_str("    \"classification goes through the all-state PR lookup by head branch name, never Git ancestry: squash merges leave merged branch SHAs unreachable from main\",\n");
    body.push_str("    \"the PR and branch lookups paginate in full; nothing falls out of a fixed list window into a false no-PR verdict\",\n");
    body.push_str("    \"reachable_from_main is recorded for context only and is never the merged discriminator\",\n");
    body.push_str("    \"unknown always classifies manual-review, never a deletion candidate\"\n");
    body.push_str("  ],\n");
    body.push_str(&format!("  \"total_branches\": {},\n", entries.len()));
    body.push_str("  \"counts\": {\n");
    body.push_str(&format!(
        "    \"by_classification\": {},\n    \"by_disposition\": {},\n    \"by_prefix\": {},\n    \"by_age_bucket\": {}\n  }},\n",
        counts_json(&counts.by_classification, "    "),
        counts_json(&counts.by_disposition, "    "),
        counts_json(&counts.by_prefix, "    "),
        counts_json(&counts.by_age_bucket, "    "),
    ));
    body.push_str("  \"branches\": [\n");
    let rendered: Vec<String> = entries.iter().map(entry_json).collect();
    body.push_str(&rendered.join(",\n"));
    body.push_str("\n  ]\n}\n");
    body
}

fn entry_json(entry: &InventoryEntry) -> String {
    format!(
        "    {{\n      \"name\": \"{}\",\n      \"head_sha\": \"{}\",\n      \"committed_date\": {},\n      \"author\": {},\n      \"committer\": {},\n      \"age_days\": {},\n      \"matching_prs\": {},\n      \"merged_pr\": {},\n      \"issue_refs\": {},\n      \"active_pr_head\": {},\n      \"claim\": {},\n      \"protected\": {},\n      \"reachable_from_main\": {},\n      \"lookup_error\": {},\n      \"classification\": \"{}\",\n      \"disposition\": \"{}\",\n      \"reason\": \"{}\"\n    }}",
        json_escape(&entry.branch.name),
        json_escape(&entry.branch.head_sha),
        json_opt_string(entry.branch.committed_date.as_deref()),
        json_opt_string(entry.branch.author.as_deref()),
        json_opt_string(entry.branch.committer.as_deref()),
        json_opt_i64(entry.age_days),
        json_u64_array(&entry.matching_prs),
        entry
            .merged_pr_number
            .map(|number| number.to_string())
            .unwrap_or_else(|| "null".to_string()),
        json_u64_array(&entry.issue_refs),
        entry.active_pr_head,
        json_opt_string(entry.claim_source.as_deref()),
        entry.branch.protected,
        json_opt_bool(entry.branch.reachable_from_main),
        json_opt_string(entry.branch.lookup_error.as_deref()),
        entry.classification,
        entry.disposition,
        json_escape(&entry.reason),
    )
}

fn inventory_markdown(
    input: &InventoryInput,
    entries: &[InventoryEntry],
    digest: &str,
    reference_epoch: i64,
) -> String {
    let counts = inventory_counts(entries);
    let mut body = String::new();
    body.push_str("# ripr remote-branch inventory (#2024)\n\n");
    body.push_str("Status: read-only inventory; nothing was deleted.\n\n");
    body.push_str(&format!("- Repository: `{}`\n", input.repository));
    body.push_str(&format!(
        "- Generated: {} (epoch {reference_epoch})\n",
        rfc3339_from_epoch_seconds(reference_epoch)
    ));
    body.push_str(&format!("- Remote branches: {}\n", entries.len()));
    body.push_str(&format!("- Deletion plan digest: `{digest}`\n"));
    body.push_str("- Companion artifacts: `branch-inventory.json`, `branch-inventory-input.json`, `branch-inventory-plan.json`\n");
    if !input.warnings.is_empty() {
        body.push_str("\n## Warnings\n\n");
        for warning in &input.warnings {
            body.push_str(&format!("- {warning}\n"));
        }
    }
    body.push_str("\n## Methodology\n\n");
    body.push_str("- Classification goes through the all-state PR lookup by head branch name, never Git ancestry. Squash merges leave a merged PR's branch SHAs unreachable from `main`, so `git branch --merged` would misclassify nearly every merged leftover; `reachable_from_main` is recorded for context only.\n");
    body.push_str("- The branch and PR lookups paginate in full (`--paginate` / an explicit GraphQL cursor loop), so nothing below a fixed list window silently classifies as having no PR.\n");
    body.push_str("- Unknown always classifies `manual-review`, never a deletion candidate. Only `merged-pr-leftover` entries whose branch head SHA equals the merged PR head SHA are `delete-candidate`.\n");
    body.push_str("- Deletion is a separate explicit operator action: `cargo xtask branch-inventory apply --plan <path> --digest <digest>`. It refuses a regenerated or changed plan, rechecks open PR heads and branch SHAs immediately before each deletion, uses non-force ref deletion bound to the rechecked SHA (`--force-with-lease=<ref>:<sha>`, never plain `--force`), refuses to run under CI, and writes a cleanup receipt.\n");

    body.push_str(
        "\n## Counts by classification\n\n| Classification | Branches |\n| --- | ---: |\n",
    );
    for (key, count) in &counts.by_classification {
        body.push_str(&format!("| {key} | {count} |\n"));
    }
    body.push_str("\n## Counts by disposition\n\n| Disposition | Branches |\n| --- | ---: |\n");
    for (key, count) in &counts.by_disposition {
        body.push_str(&format!("| {key} | {count} |\n"));
    }
    body.push_str("\n## Counts by prefix\n\n| Prefix | Branches |\n| --- | ---: |\n");
    for (key, count) in &counts.by_prefix {
        body.push_str(&format!("| {key} | {count} |\n"));
    }
    body.push_str("\n## Counts by age (days)\n\n| Age bucket | Branches |\n| --- | ---: |\n");
    for (key, count) in &counts.by_age_bucket {
        body.push_str(&format!("| {key} | {count} |\n"));
    }

    let candidates: Vec<&InventoryEntry> = entries
        .iter()
        .filter(|entry| entry.disposition == DISPOSITION_DELETE_CANDIDATE)
        .collect();
    body.push_str("\n## Delete candidates (review the plan before any apply)\n\n");
    if candidates.is_empty() {
        body.push_str("None.\n");
    } else {
        body.push_str(
            "| Branch | Head SHA | Merged PR | Age (days) |\n| --- | --- | --- | ---: |\n",
        );
        for entry in candidates {
            body.push_str(&format!(
                "| `{}` | `{}` | {} | {} |\n",
                entry.branch.name,
                short_sha(&entry.branch.head_sha),
                entry
                    .merged_pr_number
                    .map(|number| format!("#{number}"))
                    .unwrap_or_else(|| "unknown".to_string()),
                entry
                    .age_days
                    .map(|days| days.to_string())
                    .unwrap_or_else(|| "unknown".to_string()),
            ));
        }
    }

    body.push_str("\n## All branches\n\n");
    body.push_str("| Branch | Classification | Disposition | Age (days) | Reachable from main | PRs | Reason |\n");
    body.push_str("| --- | --- | --- | ---: | --- | --- | --- |\n");
    for entry in entries {
        body.push_str(&format!(
            "| `{}` | {} | {} | {} | {} | {} | {} |\n",
            entry.branch.name,
            entry.classification,
            entry.disposition,
            entry
                .age_days
                .map(|days| days.to_string())
                .unwrap_or_else(|| "unknown".to_string()),
            entry
                .branch
                .reachable_from_main
                .map(|flag| if flag { "yes" } else { "no" })
                .unwrap_or("unknown"),
            entry
                .matching_prs
                .iter()
                .map(|number| format!("#{number}"))
                .collect::<Vec<_>>()
                .join(", "),
            entry.reason,
        ));
    }
    body
}

fn short_sha(sha: &str) -> String {
    sha.chars().take(8).collect()
}

fn plan_json(
    input: &InventoryInput,
    deletions: &[PlanDeletion],
    digest: &str,
    reference_epoch: i64,
) -> String {
    let mut body = String::new();
    body.push_str("{\n");
    body.push_str(&format!(
        "  \"schema_version\": \"{PLAN_SCHEMA_VERSION}\",\n  \"kind\": \"branch-inventory-deletion-plan\",\n  \"repository\": \"{}\",\n  \"generated_at_epoch_seconds\": {reference_epoch},\n  \"generated_at\": \"{}\",\n",
        json_escape(&input.repository),
        rfc3339_from_epoch_seconds(reference_epoch),
    ));
    body.push_str("  \"digest_algorithm\": \"sha256 over the canonical plan content: a `branch-inventory-deletion-plan v1` header line, the repository line, then one `branch|head_sha|merged_pr|reason` line per deletion sorted by branch (the timestamp is excluded so identical review content has a stable digest)\",\n");
    body.push_str(&format!("  \"digest\": \"{digest}\",\n"));
    body.push_str("  \"apply_command\": \"cargo xtask branch-inventory apply --plan target/ripr/reports/branch-inventory-plan.json --digest <digest>\",\n");
    body.push_str("  \"apply_guards\": [\n");
    body.push_str(
        "    \"refuses a regenerated or changed plan (digest recomputed over plan content)\",\n",
    );
    body.push_str("    \"rechecks open PR heads, branch SHAs, and protection immediately before each deletion\",\n");
    body.push_str(
        "    \"uses non-force ref deletion bound to the rechecked SHA (git push --force-with-lease=<ref>:<sha> origin --delete, never plain --force)\",\n",
    );
    body.push_str("    \"refuses to run under CI (CI/GITHUB_ACTIONS environment); no workflow or hook wires this path\",\n");
    body.push_str("    \"does not touch local worktrees, caches, tags, releases, or source-repository refs (#1034/#1635 govern those)\"\n");
    body.push_str("  ],\n");
    body.push_str("  \"deletions\": [\n");
    let rendered: Vec<String> = deletions
        .iter()
        .map(|deletion| {
            format!(
                "    {{\n      \"branch\": \"{}\",\n      \"head_sha\": \"{}\",\n      \"merged_pr\": {},\n      \"reason\": \"{}\"\n    }}",
                json_escape(&deletion.branch),
                json_escape(&deletion.head_sha),
                deletion.merged_pr,
                json_escape(&deletion.reason),
            )
        })
        .collect();
    body.push_str(&rendered.join(",\n"));
    body.push_str("\n  ]\n}\n");
    body
}

fn receipt_json(repository: &str, digest: &str, outcomes: &[ApplyOutcome]) -> String {
    let mut body = String::new();
    body.push_str("{\n");
    body.push_str(&format!(
        "  \"schema_version\": \"{RECEIPT_SCHEMA_VERSION}\",\n  \"kind\": \"branch-inventory-cleanup-receipt\",\n  \"repository\": \"{}\",\n  \"plan_digest\": \"{digest}\",\n",
        json_escape(repository),
    ));
    body.push_str(&format!(
        "  \"counts\": {{ \"deleted\": {}, \"skipped\": {}, \"changed\": {}, \"failed\": {} }},\n",
        outcome_count(outcomes, APPLY_DELETED),
        outcome_count(outcomes, APPLY_SKIPPED),
        outcome_count(outcomes, APPLY_CHANGED),
        outcome_count(outcomes, APPLY_FAILED),
    ));
    body.push_str("  \"outcomes\": [\n");
    let rendered: Vec<String> = outcomes
        .iter()
        .map(|outcome| {
            format!(
                "    {{\n      \"branch\": \"{}\",\n      \"outcome\": \"{}\",\n      \"reason\": \"{}\"\n    }}",
                json_escape(&outcome.branch),
                json_escape(&outcome.outcome),
                json_escape(&outcome.reason),
            )
        })
        .collect();
    body.push_str(&rendered.join(",\n"));
    body.push_str("\n  ]\n}\n");
    body
}

fn receipt_markdown(repository: &str, digest: &str, outcomes: &[ApplyOutcome]) -> String {
    let mut body = String::new();
    body.push_str("# ripr remote-branch cleanup receipt (#2024)\n\n");
    body.push_str(&format!("- Repository: `{repository}`\n"));
    body.push_str(&format!("- Plan digest: `{digest}`\n"));
    body.push_str(&format!(
        "- Outcomes: {} deleted, {} skipped, {} changed, {} failed\n",
        outcome_count(outcomes, APPLY_DELETED),
        outcome_count(outcomes, APPLY_SKIPPED),
        outcome_count(outcomes, APPLY_CHANGED),
        outcome_count(outcomes, APPLY_FAILED),
    ));
    body.push_str("\n| Branch | Outcome | Reason |\n| --- | --- | --- |\n");
    for outcome in outcomes {
        body.push_str(&format!(
            "| `{}` | {} | {} |\n",
            outcome.branch, outcome.outcome, outcome.reason
        ));
    }
    body
}

fn outcome_count(outcomes: &[ApplyOutcome], kind: &str) -> usize {
    outcomes
        .iter()
        .filter(|outcome| outcome.outcome == kind)
        .count()
}

// ---------------------------------------------------------------------------
// Issue references, time helpers
// ---------------------------------------------------------------------------

fn extract_issue_refs(text: &str) -> Vec<u64> {
    let mut refs = Vec::new();
    let bytes = text.as_bytes();
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'#' {
            let start = index + 1;
            let mut end = start;
            while end < bytes.len() && bytes[end].is_ascii_digit() {
                end += 1;
            }
            if end > start
                && let Ok(number) = text[start..end].parse::<u64>()
            {
                refs.push(number);
            }
            index = end.max(start);
        } else {
            index += 1;
        }
    }
    refs.sort_unstable();
    refs.dedup();
    refs
}

fn now_epoch_seconds() -> Result<i64, String> {
    let duration = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|err| format!("system clock is before the unix epoch: {err}"))?;
    i64::try_from(duration.as_secs())
        .map_err(|err| format!("current epoch seconds do not fit in i64: {err}"))
}

fn days_from_civil(year: i64, month: i64, day: i64) -> i64 {
    let year = if month <= 2 { year - 1 } else { year };
    let era = if year >= 0 { year } else { year - 399 } / 400;
    let year_of_era = year - era * 400;
    let month_prime = if month > 2 { month - 3 } else { month + 9 };
    let day_of_year = (153 * month_prime + 2) / 5 + day - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    era * 146_097 + day_of_era - 719_468
}

fn civil_from_days(days: i64) -> (i64, i64, i64) {
    let shifted = days + 719_468;
    let era = if shifted >= 0 {
        shifted
    } else {
        shifted - 146_096
    } / 146_097;
    let day_of_era = shifted - era * 146_097;
    let year_of_era =
        (day_of_era - day_of_era / 1460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let year = year_of_era + era * 400;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_prime = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_prime + 2) / 5 + 1;
    let month = if month_prime < 10 {
        month_prime + 3
    } else {
        month_prime - 9
    };
    (if month <= 2 { year + 1 } else { year }, month, day)
}

fn parse_rfc3339_epoch_seconds(text: &str) -> Result<i64, String> {
    let bytes = text.as_bytes();
    if bytes.len() < 20 {
        return Err(format!("RFC3339 timestamp `{text}` is too short"));
    }
    let number = |range: std::ops::Range<usize>, label: &str| -> Result<i64, String> {
        text.get(range.clone())
            .ok_or_else(|| format!("RFC3339 timestamp `{text}` is missing {label}"))?
            .parse::<i64>()
            .map_err(|err| format!("RFC3339 timestamp `{text}` has invalid {label}: {err}"))
    };
    let separators = [(4, b'-'), (7, b'-'), (10, b'T'), (13, b':'), (16, b':')];
    for (position, expected) in separators {
        if bytes.get(position) != Some(&expected) {
            return Err(format!(
                "RFC3339 timestamp `{text}` has an unexpected byte at position {position}"
            ));
        }
    }
    let year = number(0..4, "year")?;
    let month = number(5..7, "month")?;
    let day = number(8..10, "day")?;
    let hour = number(11..13, "hour")?;
    let minute = number(14..16, "minute")?;
    let second = number(17..19, "second")?;
    let tail = &text[19..];
    let offset_seconds: i64 = if tail == "Z" {
        0
    } else if tail.len() == 6 && (tail.starts_with('+') || tail.starts_with('-')) {
        let sign: i64 = if tail.starts_with('-') { -1 } else { 1 };
        let offset_hours = tail
            .get(1..3)
            .and_then(|part| part.parse::<i64>().ok())
            .ok_or_else(|| format!("RFC3339 timestamp `{text}` has invalid offset hours"))?;
        let offset_minutes = tail
            .get(4..6)
            .and_then(|part| part.parse::<i64>().ok())
            .ok_or_else(|| format!("RFC3339 timestamp `{text}` has invalid offset minutes"))?;
        sign * (offset_hours * 3600 + offset_minutes * 60)
    } else {
        return Err(format!(
            "RFC3339 timestamp `{text}` must end in `Z` or a `+hh:mm` offset"
        ));
    };
    Ok(
        days_from_civil(year, month, day) * 86_400 + hour * 3600 + minute * 60 + second
            - offset_seconds,
    )
}

fn rfc3339_from_epoch_seconds(epoch: i64) -> String {
    let days = epoch.div_euclid(86_400);
    let seconds_of_day = epoch.rem_euclid(86_400);
    let (year, month, day) = civil_from_days(days);
    let hour = seconds_of_day / 3600;
    let minute = (seconds_of_day % 3600) / 60;
    let second = seconds_of_day % 60;
    format!("{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:{second:02}Z")
}

// ---------------------------------------------------------------------------
// Tests: pure-function classification, plan digest, and apply decisions over
// fabricated git/GitHub data. No network, no process spawns.
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn ensure(condition: bool, message: &str) -> Result<(), String> {
        if condition {
            Ok(())
        } else {
            Err(format!("branch-inventory test failed: {message}"))
        }
    }

    fn branch(name: &str, sha: &str) -> BranchFacts {
        BranchFacts {
            name: name.to_string(),
            head_sha: sha.to_string(),
            committed_date: Some("2026-01-01T00:00:00Z".to_string()),
            author: Some("octocat".to_string()),
            committer: Some("octocat".to_string()),
            protected: false,
            reachable_from_main: None,
            lookup_error: None,
        }
    }

    fn pr(
        number: u64,
        state: &str,
        merged: bool,
        head_ref: &str,
        head_sha: &str,
    ) -> PullRequestFacts {
        PullRequestFacts {
            number,
            state: state.to_string(),
            merged,
            head_ref: head_ref.to_string(),
            head_sha: head_sha.to_string(),
            title: format!("PR {number}"),
            issue_refs: Vec::new(),
        }
    }

    fn input(
        branches: Vec<BranchFacts>,
        prs: Vec<PullRequestFacts>,
        claims: Vec<ClaimFacts>,
    ) -> InventoryInput {
        InventoryInput {
            repository: "owner/repo".to_string(),
            claims_available: true,
            branches,
            pull_requests: prs,
            claims,
            warnings: Vec::new(),
        }
    }

    fn claim(branch_name: &str) -> ClaimFacts {
        ClaimFacts {
            source: "issue:2022 comment IC_1".to_string(),
            branch: branch_name.to_string(),
            state: "active".to_string(),
        }
    }

    /// 2026-07-22T00:00:00Z; 2026-01-01 is exactly 202 days earlier.
    const REFERENCE_EPOCH: i64 = 1_784_678_400;

    fn classify_one(
        branch_facts: &BranchFacts,
        prs: Vec<PullRequestFacts>,
        claims: Vec<ClaimFacts>,
    ) -> Result<InventoryEntry, String> {
        let input = input(vec![branch_facts.clone()], prs, claims);
        classify_all(&input, REFERENCE_EPOCH)
            .into_iter()
            .next()
            .ok_or_else(|| "classification returned no entry".to_string())
    }

    // Fixture 1: merged PR branch still present, head SHA reachable from main
    // (classic merge commit shape).
    #[test]
    fn branch_inventory_merged_pr_branch_still_present_is_delete_candidate() -> Result<(), String> {
        let mut facts = branch("codex/done-work", "aaaa1111");
        facts.reachable_from_main = Some(true);
        let entry = classify_one(
            &facts,
            vec![pr(100, "closed", true, "codex/done-work", "aaaa1111")],
            Vec::new(),
        )?;
        ensure(
            entry.classification == CLASSIFICATION_MERGED_PR_LEFTOVER,
            "expected merged-pr-leftover",
        )?;
        ensure(
            entry.disposition == DISPOSITION_DELETE_CANDIDATE,
            "expected delete-candidate disposition",
        )?;
        ensure(entry.matching_prs == vec![100], "expected matching PR #100")?;
        Ok(())
    }

    // Fixture (required addition): squash-merged PR branch is NOT reachable
    // from main; classification must still go through the merged-PR record.
    #[test]
    fn branch_inventory_squash_merged_branch_not_reachable_is_delete_candidate()
    -> Result<(), String> {
        let mut facts = branch("analysis/squash-merged", "bbbb2222");
        facts.reachable_from_main = Some(false);
        let entry = classify_one(
            &facts,
            vec![pr(
                759,
                "closed",
                true,
                "analysis/squash-merged",
                "bbbb2222",
            )],
            Vec::new(),
        )?;
        ensure(
            entry.classification == CLASSIFICATION_MERGED_PR_LEFTOVER,
            "squash-merged branch must classify merged-pr-leftover despite unreachable SHA",
        )?;
        ensure(
            entry.disposition == DISPOSITION_DELETE_CANDIDATE,
            "squash-merged branch with matching head SHA must be a delete candidate",
        )?;
        ensure(
            entry.reason.contains("squash"),
            "reason must name the squash-merge discriminator",
        )?;
        Ok(())
    }

    // Fixture 2: closed-unmerged branch with unique commits.
    #[test]
    fn branch_inventory_closed_unmerged_branch_requires_manual_review() -> Result<(), String> {
        let entry = classify_one(
            &branch("fix/abandoned", "cccc3333"),
            vec![pr(200, "closed", false, "fix/abandoned", "cccc3333")],
            Vec::new(),
        )?;
        ensure(
            entry.classification == CLASSIFICATION_CLOSED_PR_LEFTOVER,
            "expected closed-pr-leftover",
        )?;
        ensure(
            entry.disposition == DISPOSITION_MANUAL_REVIEW,
            "closed unmerged work must require manual review",
        )?;
        Ok(())
    }

    // Fixture 3: open PR head is structurally excluded.
    #[test]
    fn branch_inventory_open_pr_head_is_active_and_kept() -> Result<(), String> {
        let entry = classify_one(
            &branch("feat/open-work", "dddd4444"),
            vec![pr(300, "open", false, "feat/open-work", "dddd4444")],
            Vec::new(),
        )?;
        ensure(
            entry.classification == CLASSIFICATION_ACTIVE,
            "expected active",
        )?;
        ensure(
            entry.disposition == DISPOSITION_KEEP,
            "open PR head must be kept",
        )?;
        ensure(entry.active_pr_head, "active_pr_head flag must be set")?;
        Ok(())
    }

    // Fixture 4: branch renamed after plan generation; the apply recheck finds
    // the planned branch missing and records `changed`, never a deletion.
    #[test]
    fn branch_inventory_apply_renamed_branch_records_changed() -> Result<(), String> {
        let deletion = PlanDeletion {
            branch: "codex/renamed-away".to_string(),
            head_sha: "eeee5555".to_string(),
            merged_pr: 400,
            reason: "merged PR #400".to_string(),
        };
        let state = LiveBranchState {
            lookup_error: None,
            exists: false,
            head_sha: None,
            protected: false,
            open_pr_head: false,
        };
        let decision = decide_apply(&deletion, &state);
        ensure(
            decision.outcome == APPLY_CHANGED,
            "renamed branch must record changed",
        )?;
        ensure(
            decision.reason.contains("no longer exists"),
            "changed reason must name the missing branch",
        )?;
        Ok(())
    }

    // Fixture 5: branch SHA moved after plan generation; apply records
    // `changed` and refuses the stale plan entry.
    #[test]
    fn branch_inventory_apply_moved_sha_records_changed() -> Result<(), String> {
        let deletion = PlanDeletion {
            branch: "codex/moved".to_string(),
            head_sha: "ffff6666".to_string(),
            merged_pr: 500,
            reason: "merged PR #500".to_string(),
        };
        let state = LiveBranchState {
            lookup_error: None,
            exists: true,
            head_sha: Some("99990000".to_string()),
            protected: false,
            open_pr_head: false,
        };
        let decision = decide_apply(&deletion, &state);
        ensure(
            decision.outcome == APPLY_CHANGED,
            "moved SHA must record changed",
        )?;
        ensure(
            decision.reason.contains("moved since plan generation"),
            "changed reason must name the SHA drift",
        )?;
        Ok(())
    }

    // Fixture 6: active #2022 claim without a PR.
    #[test]
    fn branch_inventory_active_claim_without_pr_is_parked() -> Result<(), String> {
        let entry = classify_one(
            &branch("codex/claimed", "a1a1a1a1"),
            Vec::new(),
            vec![claim("codex/claimed")],
        )?;
        ensure(
            entry.classification == CLASSIFICATION_PARKED,
            "expected parked",
        )?;
        ensure(
            entry.disposition == DISPOSITION_KEEP,
            "claimed branch must be kept",
        )?;
        ensure(
            entry.claim_source.as_deref() == Some("issue:2022 comment IC_1"),
            "claim source must be recorded",
        )?;
        Ok(())
    }

    // Fixture 7: branch with no matching issue/PR.
    #[test]
    fn branch_inventory_no_matching_pr_is_unowned_manual_review() -> Result<(), String> {
        let entry = classify_one(
            &branch("legacy/surfaces-x", "b2b2b2b2"),
            Vec::new(),
            Vec::new(),
        )?;
        ensure(
            entry.classification == CLASSIFICATION_UNOWNED,
            "expected unowned",
        )?;
        ensure(
            entry.disposition == DISPOSITION_MANUAL_REVIEW,
            "unowned branch must require manual review, never a deletion candidate",
        )?;
        Ok(())
    }

    // Fixture 8: duplicate PR history — a closed PR followed by a merged PR on
    // the same head branch; the merged record wins.
    #[test]
    fn branch_inventory_duplicate_pr_history_merged_wins() -> Result<(), String> {
        let entry = classify_one(
            &branch("codex/retried", "c3c3c3c3"),
            vec![
                pr(775, "closed", false, "codex/retried", "c3c3c3c3"),
                pr(789, "closed", true, "codex/retried", "c3c3c3c3"),
            ],
            Vec::new(),
        )?;
        ensure(
            entry.classification == CLASSIFICATION_MERGED_PR_LEFTOVER,
            "merged record must win over the closed duplicate",
        )?;
        ensure(
            entry.disposition == DISPOSITION_DELETE_CANDIDATE,
            "expected delete-candidate disposition",
        )?;
        ensure(
            entry.matching_prs == vec![775, 789],
            "both PRs must be recorded",
        )?;
        Ok(())
    }

    // Fixture 9: GitHub lookup unavailable for the branch.
    #[test]
    fn branch_inventory_lookup_unavailable_is_manual_review() -> Result<(), String> {
        let mut facts = branch("analysis/ambiguous", "d4d4d4d4");
        facts.lookup_error = Some("branch missing from the GraphQL refs lookup".to_string());
        let entry = classify_one(
            &facts,
            vec![pr(900, "closed", true, "analysis/ambiguous", "d4d4d4d4")],
            Vec::new(),
        )?;
        ensure(
            entry.classification == CLASSIFICATION_MANUAL_REVIEW,
            "ambiguous lookup must classify manual-review",
        )?;
        ensure(
            entry.disposition == DISPOSITION_MANUAL_REVIEW,
            "ambiguous lookup must require manual review",
        )?;
        Ok(())
    }

    // Fixture 10: source-repo promotion branch appearing in this repository's
    // remote listing (freeze/* authority) is never a deletion candidate.
    #[test]
    fn branch_inventory_freeze_authority_branch_is_protected() -> Result<(), String> {
        let entry = classify_one(
            &branch("freeze/source-sync-2026-07-21", "e5e5e5e5"),
            Vec::new(),
            Vec::new(),
        )?;
        ensure(
            entry.classification == CLASSIFICATION_PROTECTED,
            "expected protected",
        )?;
        ensure(
            entry.disposition == DISPOSITION_KEEP,
            "authority branch must be kept",
        )?;
        Ok(())
    }

    #[test]
    fn branch_inventory_main_is_protected() -> Result<(), String> {
        let entry = classify_one(&branch("main", "00000000"), Vec::new(), Vec::new())?;
        ensure(
            entry.classification == CLASSIFICATION_PROTECTED,
            "main must be protected",
        )?;
        ensure(entry.disposition == DISPOSITION_KEEP, "main must be kept")?;
        Ok(())
    }

    #[test]
    fn branch_inventory_protected_flag_is_kept() -> Result<(), String> {
        let mut facts = branch("release/2026-07", "f6f6f6f6");
        facts.protected = true;
        let entry = classify_one(&facts, Vec::new(), Vec::new())?;
        ensure(
            entry.classification == CLASSIFICATION_PROTECTED,
            "expected protected",
        )?;
        ensure(
            entry.disposition == DISPOSITION_KEEP,
            "protected flag must be kept",
        )?;
        Ok(())
    }

    // Merged PR but the branch head SHA differs from the PR head SHA: unique
    // commits may exist, so this is manual-review, not a deletion candidate.
    #[test]
    fn branch_inventory_merged_pr_with_sha_drift_requires_manual_review() -> Result<(), String> {
        let entry = classify_one(
            &branch("codex/drifted", "0a0a0a0a"),
            vec![pr(600, "closed", true, "codex/drifted", "1b1b1b1b")],
            Vec::new(),
        )?;
        ensure(
            entry.classification == CLASSIFICATION_MERGED_PR_LEFTOVER,
            "expected merged-pr-leftover",
        )?;
        ensure(
            entry.disposition == DISPOSITION_MANUAL_REVIEW,
            "SHA drift after merge must require manual review",
        )?;
        Ok(())
    }

    // Fail-closed: when the #2022 claim lookup was unavailable, even a clean
    // merged-PR match degrades to manual-review.
    #[test]
    fn branch_inventory_unavailable_claims_fail_closed() -> Result<(), String> {
        let facts = branch("codex/done-work", "aaaa1111");
        let mut packet = input(
            vec![facts],
            vec![pr(100, "closed", true, "codex/done-work", "aaaa1111")],
            Vec::new(),
        );
        packet.claims_available = false;
        let entries = classify_all(&packet, REFERENCE_EPOCH);
        let entry = entries
            .first()
            .ok_or_else(|| "classification returned no entry".to_string())?;
        ensure(
            entry.disposition == DISPOSITION_MANUAL_REVIEW,
            "unavailable claim lookup must fail closed to manual-review",
        )?;
        Ok(())
    }

    #[test]
    fn branch_inventory_age_days_and_buckets() -> Result<(), String> {
        let entry = classify_one(&branch("docs/old", "12345678"), Vec::new(), Vec::new())?;
        ensure(entry.age_days == Some(202), "age must be 202 days")?;
        ensure(
            age_bucket(entry.age_days) == "181-365",
            "expected 181-365 bucket",
        )?;
        ensure(age_bucket(Some(10)) == "0-30", "expected 0-30 bucket")?;
        ensure(age_bucket(Some(45)) == "31-90", "expected 31-90 bucket")?;
        ensure(age_bucket(Some(120)) == "91-180", "expected 91-180 bucket")?;
        ensure(
            age_bucket(Some(900)) == "over-365",
            "expected over-365 bucket",
        )?;
        ensure(age_bucket(None) == "unknown", "expected unknown bucket")?;
        ensure(branch_prefix("codex/x") == "codex", "prefix before slash")?;
        ensure(branch_prefix("goals-x") == "(no-prefix)", "no-slash prefix")?;
        Ok(())
    }

    #[test]
    fn branch_inventory_plan_digest_is_stable_and_detects_tampering() -> Result<(), String> {
        let deletions = vec![
            PlanDeletion {
                branch: "codex/a".to_string(),
                head_sha: "aaaa1111".to_string(),
                merged_pr: 100,
                reason: "merged PR #100".to_string(),
            },
            PlanDeletion {
                branch: "codex/b".to_string(),
                head_sha: "bbbb2222".to_string(),
                merged_pr: 200,
                reason: "merged PR #200".to_string(),
            },
        ];
        let digest = plan_digest("owner/repo", &deletions);
        ensure(
            digest.starts_with("sha256:"),
            "digest must carry the algorithm",
        )?;
        ensure(
            digest == plan_digest("owner/repo", &deletions),
            "digest must be stable over identical content",
        )?;
        let mut tampered = deletions.clone();
        if let Some(first) = tampered.first_mut() {
            first.head_sha = "deadbeef".to_string();
        }
        ensure(
            digest != plan_digest("owner/repo", &tampered),
            "a changed plan must recompute to a different digest",
        )?;
        ensure(
            digest != plan_digest("owner/other", &deletions),
            "a different repository must recompute to a different digest",
        )?;
        Ok(())
    }

    #[test]
    fn branch_inventory_plan_digest_covers_every_field_and_canonicalizes_order()
    -> Result<(), String> {
        let deletions = vec![
            PlanDeletion {
                branch: "codex/a".to_string(),
                head_sha: "aaaa1111".to_string(),
                merged_pr: 100,
                reason: "merged PR #100".to_string(),
            },
            PlanDeletion {
                branch: "codex/b".to_string(),
                head_sha: "bbbb2222".to_string(),
                merged_pr: 200,
                reason: "merged PR #200".to_string(),
            },
        ];
        let digest = plan_digest("owner/repo", &deletions);

        let mut merged_pr_tampered = deletions.clone();
        if let Some(first) = merged_pr_tampered.first_mut() {
            first.merged_pr = 101;
        }
        ensure(
            digest != plan_digest("owner/repo", &merged_pr_tampered),
            "a tampered merged_pr field must change the digest",
        )?;

        let mut reason_tampered = deletions.clone();
        if let Some(first) = reason_tampered.first_mut() {
            first.reason = "merged PR #100 (edited after review)".to_string();
        }
        ensure(
            digest != plan_digest("owner/repo", &reason_tampered),
            "a tampered reason field must change the digest",
        )?;

        let reordered: Vec<PlanDeletion> = deletions.iter().rev().cloned().collect();
        ensure(
            digest == plan_digest("owner/repo", &reordered),
            "entry order must not change the digest (sorted-branch canonicalization)",
        )?;
        Ok(())
    }

    #[test]
    fn branch_inventory_plan_records_the_matched_merged_pr_not_the_latest_pr() -> Result<(), String>
    {
        // Merged #700 and a later closed-unmerged #800 share the head branch;
        // the deletion plan must name the merged PR that established the
        // delete-candidate status, never the max PR number.
        let entry = classify_one(
            &branch("codex/reopened", "aaaa7000"),
            vec![
                pr(700, "closed", true, "codex/reopened", "aaaa7000"),
                pr(800, "closed", false, "codex/reopened", "aaaa7000"),
            ],
            Vec::new(),
        )?;
        ensure(
            entry.disposition == DISPOSITION_DELETE_CANDIDATE,
            "exact SHA match with merged #700 must be a delete candidate",
        )?;
        ensure(
            entry.merged_pr_number == Some(700),
            "the entry must record the matched merged PR #700",
        )?;
        ensure(
            entry.matching_prs == vec![700, 800],
            "both PRs stay visible in the inventory",
        )?;
        let packet = input(
            vec![branch("codex/reopened", "aaaa7000")],
            vec![
                pr(700, "closed", true, "codex/reopened", "aaaa7000"),
                pr(800, "closed", false, "codex/reopened", "aaaa7000"),
            ],
            Vec::new(),
        );
        let entries = classify_all(&packet, REFERENCE_EPOCH);
        let deletions = plan_deletions(&entries);
        ensure(deletions.len() == 1, "exactly one plan deletion expected")?;
        let deletion = deletions
            .first()
            .ok_or_else(|| "plan deletion missing".to_string())?;
        ensure(
            deletion.merged_pr == 700,
            "the plan must name merged PR #700, not the later closed PR #800",
        )?;
        Ok(())
    }

    #[test]
    fn branch_inventory_delete_argv_binds_rechecked_sha_without_plain_force() -> Result<(), String>
    {
        let argv = delete_branch_argv("codex/x", "aaaa1111");
        let expected = vec![
            "push".to_string(),
            "--force-with-lease=refs/heads/codex/x:aaaa1111".to_string(),
            "origin".to_string(),
            "--delete".to_string(),
            "codex/x".to_string(),
        ];
        ensure(
            argv == expected,
            "delete argv must match the documented lease form",
        )?;
        ensure(
            argv.iter()
                .any(|arg| arg == "--force-with-lease=refs/heads/codex/x:aaaa1111"),
            "the lease must bind the exact rechecked SHA refspec",
        )?;
        ensure(
            !argv.iter().any(|arg| arg == "--force" || arg == "-f"),
            "plain force flags must never appear on the delete path",
        )?;
        Ok(())
    }

    #[test]
    fn branch_inventory_pull_requests_aggregate_across_pages_and_exclude_forks()
    -> Result<(), String> {
        // Two `--paginate --jq .[]` pages: a closed PR and a merged PR share
        // head branch `codex/multi` across the page boundary; a fork head and
        // a deleted-fork (null repo) head with the same branch name must be
        // excluded, never aliasing the origin branch.
        let page_one = concat!(
            "{\"number\": 775, \"state\": \"closed\", \"merged_at\": null,",
            " \"title\": \"attempt one\", \"body\": \"refs #100\",",
            " \"head\": {\"ref\": \"codex/multi\", \"sha\": \"cccc3333\",",
            " \"repo\": {\"full_name\": \"owner/repo\"}}}\n",
            "{\"number\": 800, \"state\": \"closed\", \"merged_at\": \"2026-06-01T00:00:00Z\",",
            " \"title\": \"fork attempt\", \"body\": null,",
            " \"head\": {\"ref\": \"codex/multi\", \"sha\": \"cccc3333\",",
            " \"repo\": {\"full_name\": \"someone/fork\"}}}\n",
        );
        let page_two = concat!(
            "{\"number\": 801, \"state\": \"closed\", \"merged_at\": \"2026-06-02T00:00:00Z\",",
            " \"title\": \"deleted fork\", \"body\": null,",
            " \"head\": {\"ref\": \"codex/multi\", \"sha\": \"cccc3333\", \"repo\": null}}\n",
            "{\"number\": 789, \"state\": \"closed\", \"merged_at\": \"2026-06-03T00:00:00Z\",",
            " \"title\": \"attempt two\", \"body\": null,",
            " \"head\": {\"ref\": \"codex/multi\", \"sha\": \"cccc3333\",",
            " \"repo\": {\"full_name\": \"owner/repo\"}}}\n",
        );
        let text = format!("{page_one}{page_two}");
        let items = parse_paginated_items(&text, "pulls")?;
        ensure(items.len() == 4, "both pages must be aggregated")?;
        let prs = pull_request_facts_from_items(&items, "owner/repo")?;
        ensure(prs.len() == 2, "fork and null-repo heads must be excluded")?;
        ensure(
            prs.iter().map(|pr| pr.number).collect::<Vec<_>>() == vec![775, 789],
            "surviving PRs must be ordered by number across pages",
        )?;
        ensure(
            prs.iter().all(|pr| pr.head_ref == "codex/multi"),
            "matching by head branch name must span the page boundary",
        )?;
        let merged = prs.iter().filter(|pr| pr.merged).count();
        ensure(merged == 1, "exactly the same-repo merged PR survives")?;

        // Classification over the aggregated pages: the merged record from
        // page two wins, and the fork/null-repo collisions never contributed.
        let facts = branch("codex/multi", "cccc3333");
        let packet = InventoryInput {
            repository: "owner/repo".to_string(),
            claims_available: true,
            branches: vec![facts],
            pull_requests: prs,
            claims: Vec::new(),
            warnings: Vec::new(),
        };
        let entries = classify_all(&packet, REFERENCE_EPOCH);
        let entry = entries
            .first()
            .ok_or_else(|| "classification returned no entry".to_string())?;
        ensure(
            entry.classification == CLASSIFICATION_MERGED_PR_LEFTOVER,
            "cross-page merged record must classify merged-pr-leftover",
        )?;
        ensure(
            entry.disposition == DISPOSITION_DELETE_CANDIDATE,
            "exact SHA match across pages must be a delete candidate",
        )?;
        ensure(
            entry.matching_prs == vec![775, 789],
            "both same-repo PRs must be recorded, sorted",
        )?;
        ensure(
            entry.issue_refs == vec![100],
            "issue refs must survive parsing",
        )?;
        Ok(())
    }

    #[test]
    fn branch_inventory_claims_match_whole_tokens_only() -> Result<(), String> {
        // Near-collision prose must NOT park a branch.
        ensure(
            !comment_claims_branch("please fix the bug before Friday", "fix"),
            "prose `fix the bug` must not park branch `fix`",
        )?;
        ensure(
            !comment_claims_branch("see the release notes", "release"),
            "prose `release notes` must not park branch `release`",
        )?;
        ensure(
            !comment_claims_branch("hotfix landed on main", "fix"),
            "a longer token containing the name must not match",
        )?;
        ensure(
            !comment_claims_branch("work continues in xcodex/foo today", "codex/foo"),
            "a branch-name prefix glued to prose must not match",
        )?;
        ensure(
            !comment_claims_branch("see codex/foo/bar for the follow-up", "codex/foo"),
            "a longer path continuing the name must not match",
        )?;
        // Realistic claim shapes MUST park the branch.
        ensure(
            comment_claims_branch("claim: branch `fix`, worktree wt/fix", "fix"),
            "a backtick-quoted name must park branch `fix`",
        )?;
        ensure(
            comment_claims_branch("claim: branch codex/foo, slice alpha", "codex/foo"),
            "an unquoted slash-prefixed whole token must match",
        )?;
        ensure(
            comment_claims_branch("branch: codex/foo.", "codex/foo"),
            "a sentence-final period still counts as a boundary",
        )?;
        ensure(
            comment_claims_branch("codex/foo", "codex/foo"),
            "a comment consisting of just the branch name must match",
        )?;
        ensure(
            !comment_claims_branch("", "codex/foo"),
            "an empty comment never matches",
        )?;
        ensure(
            !comment_claims_branch("anything", ""),
            "an empty branch name never matches",
        )?;
        Ok(())
    }

    #[test]
    fn branch_inventory_apply_happy_path_and_rechecks() -> Result<(), String> {
        let deletion = PlanDeletion {
            branch: "codex/clean".to_string(),
            head_sha: "aaaa1111".to_string(),
            merged_pr: 100,
            reason: "merged PR #100".to_string(),
        };
        let matching = LiveBranchState {
            lookup_error: None,
            exists: true,
            head_sha: Some("aaaa1111".to_string()),
            protected: false,
            open_pr_head: false,
        };
        ensure(
            decide_apply(&deletion, &matching).outcome == APPLY_DELETED,
            "matching recheck must allow the deletion",
        )?;
        let now_open = LiveBranchState {
            open_pr_head: true,
            ..matching.clone()
        };
        let decision = decide_apply(&deletion, &now_open);
        ensure(
            decision.outcome == APPLY_SKIPPED,
            "new open PR head must skip",
        )?;
        let now_protected = LiveBranchState {
            protected: true,
            ..matching.clone()
        };
        let decision = decide_apply(&deletion, &now_protected);
        ensure(
            decision.outcome == APPLY_SKIPPED,
            "new protection must skip",
        )?;
        let ambiguous = LiveBranchState {
            lookup_error: Some("rate limited".to_string()),
            exists: false,
            head_sha: None,
            protected: false,
            open_pr_head: false,
        };
        let decision = decide_apply(&deletion, &ambiguous);
        ensure(
            decision.outcome == APPLY_FAILED,
            "ambiguous recheck must fail closed",
        )?;
        Ok(())
    }

    #[test]
    fn branch_inventory_parse_paginated_items_flattens_all_pages() -> Result<(), String> {
        // `gh api --paginate --jq .[]` emits one compact object per line
        // across every page; blank lines between pages are tolerated.
        let two_pages = "{\"number\": 1}\n{\"number\": 2}\n\n{\"number\": 3}\n";
        let items = parse_paginated_items(two_pages, "test")?;
        ensure(
            items.len() == 3,
            "all pages must be flattened, got a partial window",
        )?;
        let single_page = "{\"number\": 1}\n";
        let items = parse_paginated_items(single_page, "test")?;
        ensure(items.len() == 1, "single page must parse")?;
        ensure(
            parse_paginated_items("not json\n", "test").is_err(),
            "malformed lines must fail the parse",
        )?;
        Ok(())
    }

    #[test]
    fn branch_inventory_input_packet_round_trip() -> Result<(), String> {
        let packet = input(
            vec![branch("codex/x", "aaaa1111")],
            vec![pr(7, "closed", true, "codex/x", "aaaa1111")],
            vec![claim("codex/y")],
        );
        let text = input_packet_json(&packet, REFERENCE_EPOCH);
        let parsed = parse_input_packet(&text)?;
        ensure(
            parsed.repository == "owner/repo",
            "repository must round-trip",
        )?;
        ensure(parsed.branches.len() == 1, "branches must round-trip")?;
        ensure(
            parsed.pull_requests.len() == 1,
            "pull requests must round-trip",
        )?;
        ensure(parsed.claims.len() == 1, "claims must round-trip")?;
        ensure(parsed.claims_available, "claims_available must round-trip")?;
        let reparsed = parse_input_packet(&input_packet_json(&parsed, REFERENCE_EPOCH))?;
        ensure(
            reparsed.branches.len() == parsed.branches.len(),
            "input capture must be byte-stable under re-parse",
        )?;
        Ok(())
    }

    #[test]
    fn branch_inventory_input_packet_rejects_unknown_schema() -> Result<(), String> {
        let text = r#"{"schema_version": "9.9", "repository": "o/n", "branches": [], "pull_requests": [], "claims": []}"#;
        ensure(
            parse_input_packet(text).is_err(),
            "an unsupported schema_version must be rejected",
        )?;
        Ok(())
    }

    #[test]
    fn branch_inventory_extract_issue_refs() -> Result<(), String> {
        let refs =
            extract_issue_refs("fixes #2024, relates to #1034 and #2024 again; #abc ignored");
        ensure(
            refs == vec![1034, 2024],
            "issue refs must be extracted, sorted, deduped",
        )?;
        ensure(
            extract_issue_refs("no refs here").is_empty(),
            "no refs expected",
        )?;
        Ok(())
    }

    #[test]
    fn branch_inventory_rfc3339_round_trip() -> Result<(), String> {
        let epoch = parse_rfc3339_epoch_seconds("2026-07-22T00:00:00Z")?;
        ensure(
            epoch == REFERENCE_EPOCH,
            "2026-07-22 epoch must match the reference",
        )?;
        ensure(
            rfc3339_from_epoch_seconds(epoch) == "2026-07-22T00:00:00Z",
            "epoch must format back to the same timestamp",
        )?;
        let jan = parse_rfc3339_epoch_seconds("2026-01-01T00:00:00Z")?;
        ensure(jan == 1_767_225_600, "2026-01-01 epoch anchor")?;
        let offset = parse_rfc3339_epoch_seconds("2026-07-22T02:30:00+02:30")?;
        ensure(offset == epoch, "positive offsets must normalize to Z")?;
        ensure(
            parse_rfc3339_epoch_seconds("not a date").is_err(),
            "invalid timestamps must be rejected",
        )?;
        Ok(())
    }

    #[test]
    fn branch_inventory_rendered_reports_are_deterministic() -> Result<(), String> {
        let packet = input(
            vec![
                branch("codex/a", "aaaa1111"),
                branch("freeze/source-sync-2026-07-21", "e5e5e5e5"),
            ],
            vec![pr(100, "closed", true, "codex/a", "aaaa1111")],
            Vec::new(),
        );
        let mut entries = classify_all(&packet, REFERENCE_EPOCH);
        entries.sort_by(|left, right| left.branch.name.cmp(&right.branch.name));
        let first = inventory_json(&packet, &entries, REFERENCE_EPOCH);
        let second = inventory_json(&packet, &entries, REFERENCE_EPOCH);
        ensure(first == second, "inventory JSON must be deterministic")?;
        let parsed: Value = serde_json::from_str(&first)
            .map_err(|err| format!("inventory JSON must parse: {err}"))?;
        ensure(
            parsed.get("counts").is_some(),
            "inventory JSON must carry prefix/age/disposition counts",
        )?;
        let deletions = plan_deletions(&entries);
        ensure(
            deletions.len() == 1,
            "exactly one delete candidate expected",
        )?;
        let digest = plan_digest(&packet.repository, &deletions);
        let plan_text = plan_json(&packet, &deletions, &digest, REFERENCE_EPOCH);
        let plan: Value = serde_json::from_str(&plan_text)
            .map_err(|err| format!("plan JSON must parse: {err}"))?;
        let parsed_deletions = parse_plan_deletions(&plan)?;
        ensure(
            plan_digest("owner/repo", &parsed_deletions) == digest,
            "plan digest must survive a write/parse round-trip",
        )?;
        let first_md = inventory_markdown(&packet, &entries, &digest, REFERENCE_EPOCH);
        ensure(
            first_md == inventory_markdown(&packet, &entries, &digest, REFERENCE_EPOCH),
            "inventory Markdown must be deterministic",
        )?;
        ensure(
            first_md.contains("manual-review"),
            "markdown must carry the manual-review vocabulary",
        )?;
        Ok(())
    }
}
