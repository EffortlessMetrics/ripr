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
pub use json::{build_pr_evidence_summary, render_pr_evidence_summary_json};
pub use model::PrEvidenceSummaryJson;
pub use render::render_evidence_summary_md;
use render::{SummaryRenderInput, render_pr_evidence_summary};
use std::fs;
use std::path::{Path, PathBuf};

use crate::cli::{expect_value, unknown_argument};

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
    root: PathBuf,
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
    let repo = repo_root(&options.root)?;
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
    let mut root = PathBuf::from(".");
    let mut check = false;
    let mut baseline: Option<String> = None;
    let mut index = 0;
    while let Some(arg) = args.get(index) {
        match arg.as_str() {
            "--check" => check = true,
            "--root" | "--baseline" => {
                index += 1;
                let path = expect_value(args, index, arg)?;
                if path.trim().is_empty() || path.starts_with('-') {
                    return Err(format!("pr-summary {arg} requires a non-empty path value"));
                }
                #[cfg(windows)]
                {
                    let parsed = Path::new(path);
                    if !parsed.is_absolute()
                        && (parsed.has_root()
                            || matches!(
                                parsed.components().next(),
                                Some(std::path::Component::Prefix(_))
                            ))
                    {
                        return Err(format!(
                            "pr-summary {arg} requires a fully qualified or ordinary relative path; \
                             Windows partially qualified paths are not supported"
                        ));
                    }
                }
                if arg == "--root" {
                    root = PathBuf::from(path);
                } else {
                    baseline = Some(path.to_string());
                }
            }
            other => return Err(unknown_argument("pr-summary", other)),
        }
        index += 1;
    }
    Ok(SummaryOptions {
        root,
        check,
        baseline,
    })
}

fn print_help() {
    println!("{PR_SUMMARY_HELP}");
}

/// Help body for `ripr pr-summary`. Also the flag source for unknown-argument
/// suggestions; keep accepted flags on option-list lines.
pub(crate) const PR_SUMMARY_HELP: &str = "\
usage: ripr pr-summary [--root <path>] [--check] [--baseline <before.json>]

Options:
  --root <path>        Select the artifact repository (default: current directory).
  --check              Verify the existing summary is up to date.
  --baseline <path>    Before-snapshot JSON for gap delta counts, relative to the selected root.

Relative artifact inputs and all outputs are anchored under --root.
On Windows, drive-relative and root-relative paths (C:repo or \\repo) are rejected.

Outputs:
  target/ripr/pr/summary.md  — legacy PR evidence summary (Markdown)
  target/ripr/reports/pr-evidence-summary.json  — v1 evidence summary (JSON)
  target/ripr/reports/pr-evidence-summary.md  — v1 evidence summary (Markdown panel)
";

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

/// Resolve a relative selected root once against the invocation directory.
/// Absolute selections do not depend on the process working directory.
fn repo_root(root: &Path) -> Result<PathBuf, String> {
    if root.is_absolute() {
        return Ok(root.to_path_buf());
    }
    std::env::current_dir()
        .map(|cwd| cwd.join(root))
        .map_err(|err| format!("failed to determine working directory: {err}"))
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
                root: PathBuf::from("."),
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
                root: PathBuf::from("."),
                check: false,
                baseline: Some("before.json".to_string())
            })
        );
    }

    #[test]
    fn parse_accepts_root_check_and_baseline() -> Result<(), String> {
        let args = [
            "--root",
            "selected répo",
            "--check",
            "--baseline",
            "before.json",
        ]
        .map(str::to_string);
        let options = parse_options(&args)?;
        if options.root.as_path() != Path::new("selected répo")
            || !options.check
            || options.baseline.as_deref() != Some("before.json")
        {
            return Err(format!("unexpected summary options: {options:?}"));
        }
        Ok(())
    }

    #[test]
    fn parse_rejects_missing_blank_and_flag_like_paths() -> Result<(), String> {
        for flag in ["--root", "--baseline"] {
            for value in [None, Some(""), Some("   "), Some("--check")] {
                let mut args = vec![flag.to_string()];
                if let Some(value) = value {
                    args.push(value.to_string());
                }
                match parse_options(&args) {
                    Err(error) if error.contains(flag) => {}
                    other => return Err(format!("malformed {args:?} accepted: {other:?}")),
                }
            }
        }
        Ok(())
    }

    #[test]
    fn parse_rejects_unknown_arg() -> Result<(), String> {
        match parse_options(&["--baselin".to_string()]) {
            Err(msg)
                if msg.contains("Did you mean `--baseline`?")
                    && msg.contains("Run `ripr pr-summary --help`.") =>
            {
                Ok(())
            }
            other => Err(format!(
                "expected scoped pr-summary suggestion, got {other:?}"
            )),
        }
    }

    #[cfg(windows)]
    #[test]
    fn parse_rejects_windows_partially_qualified_paths() -> Result<(), String> {
        // These relative paths replace rather than extend the selected root.
        // Assemble generic drive fixtures at runtime, following the repository
        // local-context convention; no host-machine path is committed.
        let drive = 'C';
        let selected = PathBuf::from(format!(r"{drive}:\selected"));
        let before = format!(r"{drive}:\before.json");
        for (relative, expected) in [("C:repo", "C:repo"), (r"\before.json", before.as_str())] {
            let relative = Path::new(relative);
            if selected.join(relative) != Path::new(expected) || relative.is_absolute() {
                return Err(format!(
                    "Windows path replacement premise changed: {relative:?}"
                ));
            }
        }
        for flag in ["--root", "--baseline"] {
            for path in ["C:repo", "C:", r"\repo", "/repo"] {
                let args = [flag.to_string(), path.to_string()];
                match parse_options(&args) {
                    Err(error) if error.contains(flag) => {}
                    other => {
                        return Err(format!("partially qualified {args:?} accepted: {other:?}"));
                    }
                }
            }
            let absolute = format!(r"{drive}:\repo");
            let verbatim = format!(r"\\?\{drive}:\repo");
            for path in [
                absolute.as_str(),
                r"\\server\share\repo",
                verbatim.as_str(),
                r"\\?\UNC\server\share\repo",
            ] {
                if !Path::new(path).is_absolute() {
                    return Err(format!(
                        "Windows absolute fixture is not absolute: {path:?}"
                    ));
                }
                parse_options(&[flag.to_string(), path.to_string()])?;
            }
            for path in ["repo", "../repo"] {
                parse_options(&[flag.to_string(), path.to_string()])?;
            }
        }
        Ok(())
    }

    #[cfg(not(windows))]
    #[test]
    fn parse_accepts_colons_in_relative_paths() -> Result<(), String> {
        for flag in ["--root", "--baseline"] {
            parse_options(&[flag.to_string(), "C:repo".to_string()])?;
        }
        Ok(())
    }
}
