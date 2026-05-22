mod io;
mod model;
mod render;
mod util;

use super::write_parented_file;
use io::load_json;
use render::{SummaryRenderInput, render_pr_evidence_summary};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

const PR_EVIDENCE_JSON: &str = "target/ripr/pr/repo-exposure.json";
const PR_EVIDENCE_MD: &str = "target/ripr/pr/repo-exposure.md";
const REVIEW_COMMENTS_JSON: &str = "target/ripr/review/comments.json";
const REVIEW_COMMENTS_MD: &str = "target/ripr/review/comments.md";
const START_HERE_JSON: &str = "target/ripr/reports/start-here.json";
const START_HERE_MD: &str = "target/ripr/reports/start-here.md";
const PR_SUMMARY_MD: &str = "target/ripr/pr/summary.md";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct SummaryOptions {
    check: bool,
}

pub(crate) fn ripr_pr_summary(args: &[String]) -> Result<(), String> {
    if args.iter().any(|arg| arg == "--help" || arg == "-h") {
        print_help();
        return Ok(());
    }
    let options = parse_options(args)?;
    let repo = repo_root()?;
    let summary = summary_text(&repo);
    let path = repo.join(PR_SUMMARY_MD);
    if options.check {
        check_summary(&path, &summary)
    } else {
        write_summary(&path, &summary)
    }
}

fn summary_text(repo: &Path) -> String {
    let pr_evidence = load_json(repo, PR_EVIDENCE_JSON);
    let review_comments = load_json(repo, REVIEW_COMMENTS_JSON);
    let start_here = load_json(repo, START_HERE_JSON);
    render_pr_evidence_summary(&SummaryRenderInput {
        repo,
        pr_evidence_json: PR_EVIDENCE_JSON,
        review_comments_json: REVIEW_COMMENTS_JSON,
        start_here_json: START_HERE_JSON,
        pr_evidence_md: PR_EVIDENCE_MD,
        review_comments_md: REVIEW_COMMENTS_MD,
        start_here_md: START_HERE_MD,
        pr_summary_md: PR_SUMMARY_MD,
        pr_evidence: &pr_evidence,
        review_comments: &review_comments,
        start_here: &start_here,
    })
}

fn parse_options(args: &[String]) -> Result<SummaryOptions, String> {
    let mut check = false;
    for arg in args {
        match arg.as_str() {
            "--check" => check = true,
            other => return Err(format!("unknown ripr-pr-summary argument {other:?}")),
        }
    }
    Ok(SummaryOptions { check })
}

fn print_help() {
    println!("usage: cargo xtask ripr-pr-summary [--check]");
}

fn check_summary(path: &Path, expected: &str) -> Result<(), String> {
    let actual = fs::read_to_string(path)
        .map_err(|err| format!("missing or unreadable {PR_SUMMARY_MD}: {err}"))?;
    if actual == expected {
        println!("PR evidence summary contract ok: {PR_SUMMARY_MD}");
        Ok(())
    } else {
        Err(format!(
            "{PR_SUMMARY_MD} is stale; run `cargo xtask ripr-pr-summary`"
        ))
    }
}

fn write_summary(path: &Path, summary: &str) -> Result<(), String> {
    write_parented_file(path, PR_SUMMARY_MD, summary)?;
    println!("Wrote {PR_SUMMARY_MD}");
    Ok(())
}

fn repo_root() -> Result<PathBuf, String> {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    manifest_dir.parent().map(Path::to_path_buf).ok_or_else(|| {
        format!(
            "failed to resolve repo root from {}",
            manifest_dir.display()
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;
    use serde_json::json;

    #[test]
    fn parse_accepts_check_only() {
        assert_eq!(
            parse_options(&["--check".to_string()]),
            Ok(SummaryOptions { check: true })
        );
        assert_eq!(
            parse_options(&["--bad".to_string()]),
            Err("unknown ripr-pr-summary argument \"--bad\"".to_string())
        );
    }

    #[test]
    fn summary_renders_from_machine_readable_artifacts() -> Result<(), String> {
        let repo = temp_repo("ripr-pr-summary-valid")?;
        write_json(
            &repo,
            PR_EVIDENCE_JSON,
            &json!({
                "status": "advisory",
                "base": "origin/main",
                "head": "HEAD",
                "summary": {
                    "changed_files": 2,
                    "weakly_exposed": 1,
                    "reachable_unrevealed": 0,
                    "no_static_path": 0,
                    "severe_gaps": 1,
                    "requires_targeted_mutation": true,
                    "ripr_severe_gap": true,
                    "routing_reason": "ripr severe gap"
                }
            }),
        )?;
        write_json(
            &repo,
            START_HERE_JSON,
            &json!({
                "status": "actionable",
                "posture": "advisory",
                "selected": {
                    "state": "top_gap",
                    "gap_id": "gap:pr:pricing",
                    "canonical_gap_id": "gap:rust:pricing:discount:threshold-boundary",
                    "language": "rust",
                    "language_status": "stable",
                    "kind": "MissingBoundaryAssertion",
                    "changed_behavior": "amount == threshold",
                    "missing_discriminator": "Add an exact assertion for amount == threshold.",
                    "focused_proof_intent": "Add a boundary assertion in tests/pricing.rs.",
                    "repair": {
                        "route": "AddBoundaryAssertion",
                        "target_file": "tests/pricing.rs",
                        "related_test": "tests/pricing.rs::discount_boundary"
                    },
                    "verify_command": "cargo xtask fixtures boundary_gap",
                    "receipt_command": "ripr agent receipt --gap gap:pr:pricing",
                    "receipt_state": "receipt_missing",
                    "static_limit_kind": null
                },
                "limits": [
                    "Composes explicit RIPR artifacts only.",
                    "Does not run mutation testing."
                ]
            }),
        )?;
        write_json(
            &repo,
            REVIEW_COMMENTS_JSON,
            &json!({
                "status": "advisory",
                "summary": {
                    "comments": 1,
                    "summary_only": 2,
                    "suppressed": 3
                }
            }),
        )?;
        fs::write(repo.join(PR_EVIDENCE_MD), "# PR Evidence\n")
            .map_err(|err| format!("write PR md: {err}"))?;
        fs::write(repo.join(REVIEW_COMMENTS_MD), "# Guidance\n")
            .map_err(|err| format!("write review md: {err}"))?;
        fs::write(repo.join(START_HERE_MD), "# Start Here\n")
            .map_err(|err| format!("write start-here md: {err}"))?;

        let summary = summary_text(&repo);
        assert!(summary.contains("# PR Evidence Summary"));
        assert!(summary.contains("## Start Here"));
        assert!(
            summary.contains("- canonical gap: `gap:rust:pricing:discount:threshold-boundary`")
        );
        assert!(summary.contains("- repair route: `AddBoundaryAssertion`"));
        assert!(summary.contains("- verify: `cargo xtask fixtures boundary_gap`"));
        assert!(summary.contains("- receipt: `ripr agent receipt --gap gap:pr:pricing`"));
        assert!(summary.contains("- receipt state: `receipt_missing`"));
        assert!(summary.contains("- boundary: static advisory evidence only; gate decision remains separate pass/fail authority when configured."));
        assert!(summary.contains("## Fast Gate"));
        assert!(summary.contains("## RIPR"));
        assert!(summary.contains("## Targeted Mutation"));
        assert!(summary.contains("- changed-line comments: 1"));
        assert!(summary.contains("- routing_reason: `ripr severe gap`"));
        assert!(summary.contains("target/ripr/pr/repo-exposure.json"));
        assert!(summary.contains("target/ripr/review/comments.json"));

        fs::remove_dir_all(&repo).map_err(|err| format!("cleanup {}: {err}", repo.display()))
    }

    #[test]
    fn summary_makes_missing_artifacts_explicit() -> Result<(), String> {
        let repo = temp_repo("ripr-pr-summary-missing")?;
        let summary = summary_text(&repo);
        assert!(summary.contains("- PR evidence JSON: missing"));
        assert!(summary.contains("- review guidance JSON: missing"));
        assert!(summary.contains("- changed files: not_available"));
        assert!(
            summary.contains(
                "| Review guidance Markdown | `target/ripr/review/comments.md` | missing |"
            )
        );
        fs::remove_dir_all(&repo).map_err(|err| format!("cleanup {}: {err}", repo.display()))
    }

    #[test]
    fn summary_handles_error_packets_and_invalid_json() -> Result<(), String> {
        let repo = temp_repo("ripr-pr-summary-error")?;
        write_json(
            &repo,
            PR_EVIDENCE_JSON,
            &json!({
                "status": "error",
                "base": "main",
                "head": "HEAD",
                "summary": {
                    "changed_files": 0,
                    "requires_targeted_mutation": false,
                    "ripr_severe_gap": false,
                    "routing_reason": null
                }
            }),
        )?;
        write_file(&repo, REVIEW_COMMENTS_JSON, "{not json")?;
        let summary = summary_text(&repo);
        assert!(summary.contains("- PR evidence status: `error`"));
        assert!(summary.contains("- routing_reason: `none`"));
        assert!(summary.contains("- review guidance JSON: invalid:"));
        fs::remove_dir_all(&repo).map_err(|err| format!("cleanup {}: {err}", repo.display()))
    }

    #[test]
    fn check_rejects_stale_summary() -> Result<(), String> {
        let repo = temp_repo("ripr-pr-summary-stale")?;
        let path = repo.join(PR_SUMMARY_MD);
        fs::create_dir_all(path.parent().ok_or_else(|| "summary parent".to_string())?)
            .map_err(|err| format!("create summary parent: {err}"))?;
        fs::write(&path, "stale\n").map_err(|err| format!("write stale summary: {err}"))?;
        let expected = summary_text(&repo);
        let err = match check_summary(&path, &expected) {
            Ok(()) => return Err("stale summary should fail".to_string()),
            Err(err) => err,
        };
        assert!(err.contains("target/ripr/pr/summary.md is stale"));
        fs::remove_dir_all(&repo).map_err(|err| format!("cleanup {}: {err}", repo.display()))
    }

    fn temp_repo(name: &str) -> Result<PathBuf, String> {
        let unique = format!(
            "{}-{}-{}",
            name,
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_err(|err| format!("system clock before epoch: {err}"))?
                .as_nanos()
        );
        let path = env::temp_dir().join(unique);
        fs::create_dir_all(&path).map_err(|err| format!("create {}: {err}", path.display()))?;
        Ok(path)
    }

    fn write_json(repo: &Path, relative: &str, value: &Value) -> Result<(), String> {
        let text =
            serde_json::to_string_pretty(value).map_err(|err| format!("serialize: {err}"))?;
        write_file(repo, relative, &text)
    }

    fn write_file(repo: &Path, relative: &str, text: &str) -> Result<(), String> {
        let path = repo.join(relative);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .map_err(|err| format!("create {}: {err}", parent.display()))?;
        }
        fs::write(&path, text).map_err(|err| format!("write {}: {err}", path.display()))
    }
}
