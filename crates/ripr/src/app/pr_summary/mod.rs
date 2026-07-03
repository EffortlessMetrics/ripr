//! `ripr pr-summary` — binary-first PR readiness summary (Campaign 31 item 8).
//!
//! Ports the `cargo xtask ripr-pr-summary` report into the `ripr` binary so
//! downstream consumers (e.g. perl-lsp-swarm) can generate their PR readiness
//! packet without compiling their own `xtask`. The xtask wrapper remains as a
//! compatibility shim until downstream consumers migrate.
//!
//! This command composes existing RIPR artifacts (start-here, gap-decision-
//! ledger, repo-exposure, diff-report) into a PR evidence summary. It does NOT
//! run analysis, invoke Cargo, or change gate semantics.

mod io;
mod json;
mod model;
mod render;
mod util;

use io::load_json;
use json::{build_pr_evidence_summary, render_pr_evidence_summary_json};
use render::{SummaryRenderInput, render_pr_evidence_summary};
use std::fs;
use std::path::{Path, PathBuf};

const PR_EVIDENCE_JSON: &str = "target/ripr/pr/repo-exposure.json";
const PR_EVIDENCE_MD: &str = "target/ripr/pr/repo-exposure.md";
const REVIEW_COMMENTS_JSON: &str = "target/ripr/review/comments.json";
const REVIEW_COMMENTS_MD: &str = "target/ripr/review/comments.md";
const START_HERE_JSON: &str = "target/ripr/reports/start-here.json";
const START_HERE_MD: &str = "target/ripr/reports/start-here.md";
const PR_SUMMARY_MD: &str = "target/ripr/pr/summary.md";
const PR_EVIDENCE_SUMMARY_JSON: &str = "target/ripr/reports/pr-evidence-summary.json";
const PR_EVIDENCE_SUMMARY_MD: &str = "target/ripr/reports/pr-evidence-summary.md";
const GAP_DECISION_LEDGER_JSON: &str = "target/ripr/reports/gap-decision-ledger.json";
const REPO_EXPOSURE_JSON: &str = "target/ripr/reports/repo-exposure.json";
const DIFF_REPORT_JSON: &str = "target/ripr/reports/diff-report.json";
const ATTEMPT_LEDGER_JSON: &str = "target/ripr/reports/swarm-attempt-ledger.json";

#[derive(Clone, Debug, Eq, PartialEq)]
struct SummaryOptions {
    check: bool,
    baseline: Option<String>,
}

/// Entry point for `ripr pr-summary`. Composes existing artifacts into a PR
/// readiness summary. Writes three outputs:
/// - `target/ripr/pr/summary.md` (legacy PR evidence summary)
/// - `target/ripr/reports/pr-evidence-summary.json` (v1 JSON)
/// - `target/ripr/reports/pr-evidence-summary.md` (v1 Markdown panel)
pub(crate) fn run_pr_summary(args: &[String]) -> Result<(), String> {
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
    write_evidence_summary_pair(&repo, &options)?;
    Ok(())
}

fn write_evidence_summary_pair(repo: &Path, options: &SummaryOptions) -> Result<(), String> {
    let start_here = load_json(repo, START_HERE_JSON);
    let gap_ledger = load_json(repo, GAP_DECISION_LEDGER_JSON);
    let repo_exposure = load_json(repo, REPO_EXPOSURE_JSON);
    let diff_report = load_json(repo, DIFF_REPORT_JSON);
    let attempt_ledger = load_json(repo, ATTEMPT_LEDGER_JSON);
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
        attempt_ledger.value.as_ref(),
    );

    let json_text = render_pr_evidence_summary_json(&summary_struct);
    let json_path = repo.join(PR_EVIDENCE_SUMMARY_JSON);
    write_parented_file(&json_path, PR_EVIDENCE_SUMMARY_JSON, json_text.as_bytes())?;
    println!("Wrote {PR_EVIDENCE_SUMMARY_JSON}");

    let md_text = render_evidence_summary_md(&summary_struct);
    let md_path = repo.join(PR_EVIDENCE_SUMMARY_MD);
    write_parented_file(&md_path, PR_EVIDENCE_SUMMARY_MD, md_text.as_bytes())?;
    println!("Wrote {PR_EVIDENCE_SUMMARY_MD}");
    Ok(())
}

fn render_evidence_summary_md(s: &model::PrEvidenceSummaryJson) -> String {
    render::render_evidence_summary_md(s)
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
            other => return Err(format!("unknown pr-summary argument `{other}`")),
        }
    }
    Ok(SummaryOptions { check, baseline })
}

fn print_help() {
    println!("usage: ripr pr-summary [--check] [--baseline <before.json>]");
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
        Err(format!("{PR_SUMMARY_MD} is stale; run `ripr pr-summary`"))
    }
}

fn write_summary(path: &Path, summary: &str) -> Result<(), String> {
    write_parented_file(path, PR_SUMMARY_MD, summary)?;
    println!("Wrote {PR_SUMMARY_MD}");
    Ok(())
}

/// Resolve the repo root. In the ripr binary, this is the current working
/// directory (the user runs `ripr pr-summary` from the repo root). The xtask
/// used `CARGO_MANIFEST_DIR` but the binary should not assume a build-system
/// location.
fn repo_root() -> Result<PathBuf, String> {
    std::env::current_dir().map_err(|err| format!("failed to determine working directory: {err}"))
}

fn write_parented_file(path: &Path, label: &str, contents: impl AsRef<[u8]>) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|err| format!("failed to create parent dir for {label}: {err}"))?;
    }
    fs::write(path, contents).map_err(|err| format!("failed to write {label}: {err}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_accepts_check_only() {
        assert_eq!(
            parse_options(&["--check".to_string()]),
            Ok(SummaryOptions {
                check: true,
                baseline: None
            })
        );
    }

    #[test]
    fn parse_accepts_baseline() {
        assert_eq!(
            parse_options(&["--baseline".to_string(), "before.json".to_string()]),
            Ok(SummaryOptions {
                check: false,
                baseline: Some("before.json".to_string())
            })
        );
    }

    #[test]
    fn parse_rejects_unknown_arg() -> Result<(), String> {
        match parse_options(&["--bogus".to_string()]) {
            Err(msg) => {
                if msg.contains("--bogus") {
                    Ok(())
                } else {
                    Err(format!("error must name the arg: {msg}"))
                }
            }
            Ok(_) => Err("unknown arg must be rejected".to_string()),
        }
    }
}
