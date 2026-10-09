//! The `eval-sweep check` command surface: argument parsing (`--manifest`,
//! `--runs`) and report writing. `run_check` runs the offline artifact
//! check, prints the verdict and the full typed incomplete disclosures, and
//! writes the JSON/Markdown reports through the render layer. It writes no
//! accepted state; a fail-closed violation returns the named error.

use super::render::{render_check_json, render_check_markdown};
use super::{RERUN_COMMAND, check_artifacts};

const DEFAULT_MANIFEST: &str = "fixtures/python-eval-sweep/manifest.json";
const CHECK_REPORT_JSON: &str = "eval-sweep-check.json";
const CHECK_REPORT_MD: &str = "eval-sweep-check.md";
// ---------------------------------------------------------------------------
// Args
// ---------------------------------------------------------------------------

struct CheckArgs {
    manifest: String,
    runs: Option<String>,
}

fn parse_check_args(args: &[String]) -> Result<CheckArgs, String> {
    let mut parsed = CheckArgs {
        manifest: DEFAULT_MANIFEST.to_string(),
        runs: None,
    };
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--manifest" => {
                index += 1;
                parsed.manifest = args.get(index).cloned().ok_or_else(|| {
                    format!("eval-sweep check --manifest requires a value\nrerun: {RERUN_COMMAND}")
                })?;
            }
            "--runs" => {
                index += 1;
                parsed.runs = Some(args.get(index).cloned().ok_or_else(|| {
                    format!("eval-sweep check --runs requires a value\nrerun: {RERUN_COMMAND}")
                })?);
            }
            other => {
                return Err(format!(
                    "unknown eval-sweep check argument: {other}\nusage: cargo xtask eval-sweep check [--manifest <path>] [--runs <path>]\nrerun: {RERUN_COMMAND}"
                ));
            }
        }
        index += 1;
    }
    Ok(parsed)
}
pub(crate) fn run_check(args: &[String]) -> Result<(), String> {
    let parsed = parse_check_args(args)?;
    let outcome = check_artifacts(&parsed.manifest, parsed.runs.as_deref())?;
    let verdict = outcome.verdict();

    println!(
        "eval-sweep check: manifest={} subjects={} sha256={}",
        outcome.manifest_path,
        outcome.accepted.subjects.len(),
        outcome.accepted.sha256
    );
    match &outcome.receipt {
        None => println!(
            "eval-sweep check: receipt=<none> verdict={} (no retained receipt supplied; not_run is not a pass)",
            verdict.as_str()
        ),
        Some(receipt) => println!(
            "eval-sweep check: receipt={} schema={} denominator selected={} run={} verdict={}",
            receipt.path,
            receipt.schema_version,
            receipt.denominator_selected,
            receipt.denominator_run,
            verdict.as_str()
        ),
    }
    let disclosures = outcome.incomplete();
    println!(
        "eval-sweep check: incomplete identities disclosed: {}",
        disclosures.len()
    );
    for diagnostic in &disclosures {
        println!("  incomplete: {}", diagnostic.render());
    }
    println!(
        "eval-sweep check verdict: {} — structural validation only; not a currentness, robustness, or adequacy claim",
        verdict.as_str()
    );
    println!("rerun: {RERUN_COMMAND}");

    let json = render_check_json(&outcome)?;
    crate::write_report(CHECK_REPORT_JSON, &format!("{json}\n"))?;
    let markdown = render_check_markdown(&outcome, verdict);
    crate::write_report(CHECK_REPORT_MD, &markdown)?;
    Ok(())
}
