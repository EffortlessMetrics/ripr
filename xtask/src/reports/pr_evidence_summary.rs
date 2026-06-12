mod io;
mod json;
mod model;
mod render;
mod util;

use super::write_parented_file;
use io::load_json;
use json::{build_pr_evidence_summary, render_pr_evidence_summary_json};
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
/// v1 JSON twin written alongside the Markdown summary.
const PR_EVIDENCE_SUMMARY_JSON: &str = "target/ripr/reports/pr-evidence-summary.json";
/// v1 Markdown panel (separate from the legacy `target/ripr/pr/summary.md`).
const PR_EVIDENCE_SUMMARY_MD: &str = "target/ripr/reports/pr-evidence-summary.md";
/// Gap-decision-ledger artifact (advisory gap counts and receipt state).
const GAP_DECISION_LEDGER_JSON: &str = "target/ripr/reports/gap-decision-ledger.json";
/// Repo-exposure artifact (run_status and limitations[]).
const REPO_EXPOSURE_JSON: &str = "target/ripr/reports/repo-exposure.json";
/// Diff-report artifact (run_status and changed_files from DiffReport).
const DIFF_REPORT_JSON: &str = "target/ripr/reports/diff-report.json";

#[derive(Clone, Debug, Eq, PartialEq)]
struct SummaryOptions {
    check: bool,
    baseline: Option<String>,
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
        check_summary(&path, &summary)?;
    } else {
        write_summary(&path, &summary)?;
    }

    // Also write the v1 JSON + MD pair.
    write_evidence_summary_pair(&repo, &options)?;

    Ok(())
}

fn write_evidence_summary_pair(repo: &Path, options: &SummaryOptions) -> Result<(), String> {
    let start_here = load_json(repo, START_HERE_JSON);
    let gap_ledger = load_json(repo, GAP_DECISION_LEDGER_JSON);
    let repo_exposure = load_json(repo, REPO_EXPOSURE_JSON);
    let diff_report = load_json(repo, DIFF_REPORT_JSON);
    let baseline_loaded;
    let baseline_value = if let Some(path) = options.baseline.as_deref() {
        baseline_loaded = load_json(repo, path);
        baseline_loaded.value.as_ref()
    } else {
        None
    };

    let summary_struct = build_pr_evidence_summary(
        start_here.value.as_ref(),
        gap_ledger.value.as_ref(),
        repo_exposure.value.as_ref(),
        diff_report.value.as_ref(),
        baseline_value,
    );

    let json_text = render_pr_evidence_summary_json(&summary_struct);
    let json_path = repo.join(PR_EVIDENCE_SUMMARY_JSON);
    write_parented_file(&json_path, PR_EVIDENCE_SUMMARY_JSON, &json_text)?;
    println!("Wrote {PR_EVIDENCE_SUMMARY_JSON}");

    let md_text = render_evidence_summary_md(&summary_struct, &json_text);
    let md_path = repo.join(PR_EVIDENCE_SUMMARY_MD);
    write_parented_file(&md_path, PR_EVIDENCE_SUMMARY_MD, md_text.as_bytes())?;
    println!("Wrote {PR_EVIDENCE_SUMMARY_MD}");

    Ok(())
}

/// Render the compact Markdown panel from the already-serialized JSON struct.
fn render_evidence_summary_md(s: &model::PrEvidenceSummaryJson, _json_text: &str) -> String {
    let mut out = String::new();
    out.push_str("# PR Evidence Summary v1\n\n");
    out.push_str(&format!("**Run Status**: `{}`\n\n", s.run_status));

    // Changed surfaces
    let surfaces = match &s.changed_surfaces {
        model::U64OrNotAvailable::Value(n) => n.to_string(),
        model::U64OrNotAvailable::NotAvailable => "not_available".to_string(),
    };
    out.push_str(&format!("**Changed Surfaces**: {surfaces}\n\n"));

    // Gaps
    out.push_str("## Gaps\n\n");
    let total_act = match &s.gaps.total_actionable {
        model::U64OrNotAvailable::Value(n) => n.to_string(),
        model::U64OrNotAvailable::NotAvailable => "not_available".to_string(),
    };
    let total_lim = match &s.gaps.total_static_limitation {
        model::U64OrNotAvailable::Value(n) => n.to_string(),
        model::U64OrNotAvailable::NotAvailable => "not_available".to_string(),
    };
    out.push_str(&format!("- total actionable: {total_act}\n"));
    out.push_str(&format!("- total static limitations: {total_lim}\n"));
    match &s.gaps.new_actionable {
        model::NullableU64::Value(n) => {
            out.push_str(&format!("- new actionable: {n}\n"));
        }
        model::NullableU64::Null => {
            out.push_str("- new actionable: null\n");
        }
    }
    match &s.gaps.resolved {
        model::NullableU64::Value(n) => {
            out.push_str(&format!("- resolved: {n}\n"));
        }
        model::NullableU64::Null => {
            out.push_str("- resolved: null\n");
        }
    }
    match &s.gaps.regressed {
        model::NullableU64::Value(n) => {
            out.push_str(&format!("- regressed: {n}\n"));
        }
        model::NullableU64::Null => {
            out.push_str("- regressed: null\n");
        }
    }
    if let Some(note) = &s.gaps.gap_delta_note {
        out.push_str(&format!("- delta note: {note}\n"));
    }
    out.push('\n');

    // Limitations
    out.push_str("## Limitations\n\n");
    if s.limitations.is_empty() {
        out.push_str("- none\n");
    } else {
        for lim in &s.limitations {
            out.push_str(&format!("- `{}`: {}\n", lim.category, lim.repair_route));
        }
    }
    out.push('\n');

    // Missing receipts
    let missing_receipts = match &s.missing_receipts {
        model::U64OrNotAvailable::Value(n) => n.to_string(),
        model::U64OrNotAvailable::NotAvailable => "not_available".to_string(),
    };
    out.push_str(&format!("**Missing Receipts**: {missing_receipts}\n\n"));

    // Top repair
    out.push_str("## Top Repair\n\n");
    if let Some(repair) = &s.top_repair {
        out.push_str(&format!("- canonical gap: `{}`\n", repair.canonical_gap_id));
        out.push_str(&format!("- language: `{}`\n", repair.language));
        out.push_str(&format!("- repair kind: `{}`\n", repair.repair_kind));
        out.push_str(&format!("- target: `{}`\n", repair.target));
        out.push_str(&format!("- verify: `{}`\n", repair.verify_command));
        out.push_str(&format!("- receipt: `{}`\n", repair.receipt_command));
        out.push_str(&format!("- receipt state: `{}`\n", repair.receipt_state));
    } else {
        let state = s.top_repair_state.as_deref().unwrap_or("missing_artifact");
        out.push_str(&format!("- state: `{state}`\n"));
    }
    out.push('\n');

    // Top limitation
    out.push_str("## Top Limitation\n\n");
    if let Some(lim) = &s.top_limitation {
        out.push_str(&format!("- category: `{}`\n", lim.category));
        out.push_str(&format!("- repair route: `{}`\n", lim.repair_route));
        out.push_str(&format!(
            "- why not actionable: {}\n",
            lim.why_not_actionable
        ));
    } else {
        out.push_str("- none\n");
    }
    out.push('\n');

    // Repro commands
    out.push_str("## Local Reproduction Commands\n\n");
    for cmd in &s.local_reproduction_commands {
        out.push_str(&format!("```\n{cmd}\n```\n\n"));
    }

    out.push_str(
        "_This summary composes existing RIPR artifacts. \
        It is static advisory evidence only; \
        run status, gap counts, and receipt state are read from repo artifacts, \
        not computed by this command._\n",
    );

    out
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
    let mut baseline: Option<String> = None;
    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--check" => check = true,
            "--baseline" => {
                let path = iter
                    .next()
                    .ok_or_else(|| "--baseline requires a path argument".to_string())?;
                baseline = Some(path.clone());
            }
            other => return Err(format!("unknown ripr-pr-summary argument {other:?}")),
        }
    }
    Ok(SummaryOptions { check, baseline })
}

fn print_help() {
    println!("usage: cargo xtask ripr-pr-summary [--check] [--baseline <before.json>]");
    println!();
    println!("Options:");
    println!("  --check              Verify the existing summary is up to date.");
    println!("  --baseline <path>    Provide a before-snapshot JSON for gap delta counts.");
    println!();
    println!("Outputs:");
    println!("  {PR_SUMMARY_MD}  — legacy PR evidence summary (Markdown)");
    println!("  {PR_EVIDENCE_SUMMARY_JSON}  — v1 evidence summary (JSON)");
    println!("  {PR_EVIDENCE_SUMMARY_MD}  — v1 evidence summary (Markdown panel)");
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
            Ok(SummaryOptions {
                check: true,
                baseline: None
            })
        );
        assert_eq!(
            parse_options(&["--bad".to_string()]),
            Err("unknown ripr-pr-summary argument \"--bad\"".to_string())
        );
    }

    #[test]
    fn parse_accepts_baseline_path() {
        let result = parse_options(&["--baseline".to_string(), "target/before.json".to_string()]);
        assert_eq!(
            result,
            Ok(SummaryOptions {
                check: false,
                baseline: Some("target/before.json".to_string()),
            })
        );
    }

    #[test]
    fn parse_rejects_baseline_without_path() {
        let result = parse_options(&["--baseline".to_string()]);
        assert!(result.is_err());
        if let Err(msg) = result {
            assert!(
                msg.contains("--baseline requires a path argument"),
                "unexpected error: {msg}"
            );
        }
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

    /// Missing-all-artifacts case: all JSON fields show explicit "not_available"
    /// or null-with-note, never fake zeros.
    #[test]
    fn evidence_summary_pair_missing_all_shows_explicit_states() -> Result<(), String> {
        let repo = temp_repo("pr-evidence-summary-missing-all")?;
        let options = SummaryOptions {
            check: false,
            baseline: None,
        };
        write_evidence_summary_pair(&repo, &options)?;

        let json_text = fs::read_to_string(repo.join(PR_EVIDENCE_SUMMARY_JSON))
            .map_err(|err| format!("read JSON: {err}"))?;
        let md_text = fs::read_to_string(repo.join(PR_EVIDENCE_SUMMARY_MD))
            .map_err(|err| format!("read MD: {err}"))?;

        // JSON: run_status unknown, not_available for counts, null for deltas.
        assert!(
            json_text.contains("\"run_status\": \"unknown\""),
            "run_status must be unknown: {json_text}"
        );
        assert!(
            json_text.contains("\"changed_surfaces\": \"not_available\""),
            "changed_surfaces must be not_available: {json_text}"
        );
        assert!(
            json_text.contains("\"total_actionable\": \"not_available\""),
            "total_actionable must be not_available: {json_text}"
        );
        assert!(
            json_text.contains("\"new_actionable\": null"),
            "new_actionable must be null: {json_text}"
        );
        assert!(
            json_text.contains("no baseline snapshot provided"),
            "gap_delta_note must be present: {json_text}"
        );
        assert!(
            json_text.contains("\"missing_artifact\""),
            "top_repair_state missing_artifact must be present: {json_text}"
        );

        // MD: explicit states surfaced.
        assert!(
            md_text.contains("not_available"),
            "MD must show not_available: {md_text}"
        );
        assert!(
            md_text.contains("missing_artifact"),
            "MD must show missing_artifact: {md_text}"
        );

        fs::remove_dir_all(&repo).map_err(|err| format!("cleanup {}: {err}", repo.display()))
    }

    /// Present-top-gap case: top_repair is populated and verify/receipt are
    /// copyable from the JSON output.
    #[test]
    fn evidence_summary_pair_present_top_gap_is_copyable() -> Result<(), String> {
        let repo = temp_repo("pr-evidence-summary-top-gap")?;

        write_json(
            &repo,
            START_HERE_JSON,
            &json!({
                "status": "actionable",
                "selected": {
                    "state": "top_gap",
                    "canonical_gap_id": "gap:rust:pricing:discount:boundary",
                    "language": "rust",
                    "repair": {
                        "route": "AddBoundaryAssertion",
                        "target_file": "tests/pricing.rs"
                    },
                    "verify_command": "cargo test discount_boundary",
                    "receipt_command": "ripr agent receipt --gap gap:pr:pricing:discount",
                    "receipt_state": "receipt_missing"
                }
            }),
        )?;
        write_json(
            &repo,
            DIFF_REPORT_JSON,
            &json!({
                "run_status": "diff_complete_full_repo_limited",
                "base": "origin/main",
                "head": "HEAD",
                "summary": { "changed_files": 2 }
            }),
        )?;
        write_json(
            &repo,
            GAP_DECISION_LEDGER_JSON,
            &json!({
                "summary": {
                    "repairable_total": 1,
                    "static_limitation_total": 0,
                    "receipt_improved_total": 0
                }
            }),
        )?;

        let options = SummaryOptions {
            check: false,
            baseline: None,
        };
        write_evidence_summary_pair(&repo, &options)?;

        let json_text = fs::read_to_string(repo.join(PR_EVIDENCE_SUMMARY_JSON))
            .map_err(|err| format!("read JSON: {err}"))?;
        let md_text = fs::read_to_string(repo.join(PR_EVIDENCE_SUMMARY_MD))
            .map_err(|err| format!("read MD: {err}"))?;

        // JSON top_repair present and copyable.
        assert!(
            json_text.contains("\"canonical_gap_id\": \"gap:rust:pricing:discount:boundary\""),
            "canonical_gap_id missing: {json_text}"
        );
        assert!(
            json_text.contains("\"verify_command\": \"cargo test discount_boundary\""),
            "verify_command missing: {json_text}"
        );
        assert!(
            json_text.contains("\"receipt_state\": \"receipt_missing\""),
            "receipt_state missing: {json_text}"
        );
        // top_repair_state must be absent when top_repair is present.
        assert!(
            !json_text.contains("\"top_repair_state\""),
            "top_repair_state must be absent when repair is present: {json_text}"
        );
        // run_status from diff-report.
        assert!(
            json_text.contains("\"run_status\": \"diff_complete_full_repo_limited\""),
            "run_status missing: {json_text}"
        );
        // gap counts.
        assert!(
            json_text.contains("\"total_actionable\": 1"),
            "total_actionable missing: {json_text}"
        );
        assert!(
            json_text.contains("\"missing_receipts\": 1"),
            "missing_receipts missing: {json_text}"
        );

        // MD must contain the verify command and receipt state.
        assert!(
            md_text.contains("cargo test discount_boundary"),
            "MD missing verify command: {md_text}"
        );
        assert!(
            md_text.contains("receipt_missing"),
            "MD missing receipt_state: {md_text}"
        );

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
