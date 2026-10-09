//! Argument parsing, command entry, and report publication for
//! `python-repair-trust check-driver`.
//!
//! Validation, aggregation, and rendering stay in their owning modules. This
//! adapter parses flags, prints the human summary, writes the JSON/Markdown
//! pair, and maps an `inconsistent` verdict to a nonzero exit.

use super::super::python_repair_trust::DEFAULT_MANIFEST;
use super::aggregate::check_driver_artifacts;
use super::render::{render_check_driver_json, render_check_driver_markdown};
use super::schema::RERUN_COMMAND;

const CHECK_REPORT_JSON: &str = "python-repair-driver-check.json";
const CHECK_REPORT_MD: &str = "python-repair-driver-check.md";

fn parse_check_driver_args(args: &[String]) -> Result<(String, String), String> {
    let mut manifest = DEFAULT_MANIFEST.to_string();
    let mut bindings: Option<String> = None;
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--manifest" => {
                index += 1;
                manifest = args.get(index).cloned().ok_or_else(|| {
                    format!(
                        "python-repair-trust check-driver --manifest requires a value\nrerun: {RERUN_COMMAND}"
                    )
                })?;
            }
            "--bindings" => {
                index += 1;
                bindings = Some(args.get(index).cloned().ok_or_else(|| {
                    format!(
                        "python-repair-trust check-driver --bindings requires a value\nrerun: {RERUN_COMMAND}"
                    )
                })?);
            }
            other => {
                return Err(format!(
                    "unknown python-repair-trust check-driver argument: {other}\nusage: cargo xtask python-repair-trust check-driver [--manifest <path>] --bindings <dir-or-file>\nrerun: {RERUN_COMMAND}"
                ));
            }
        }
        index += 1;
    }
    let bindings = bindings.ok_or_else(|| {
        format!(
            "python-repair-trust check-driver requires --bindings <dir-or-file>\nrerun: {RERUN_COMMAND}"
        )
    })?;
    Ok((manifest, bindings))
}

/// The command entry: `python-repair-trust check-driver`, dispatched from the
/// `python-repair-trust` subcommand router.
pub(crate) fn run_check_driver(args: &[String]) -> Result<(), String> {
    let (manifest_path, bindings_input) = parse_check_driver_args(args)?;
    let outcome = check_driver_artifacts(&manifest_path, &bindings_input)?;
    let verdict = outcome.verdict();

    match &outcome.manifest {
        None => println!(
            "python-repair-trust check-driver: manifest={manifest_path} selections=<none> (no accepted selection manifest; binding records cannot bind without one)"
        ),
        Some(manifest) => println!(
            "python-repair-trust check-driver: manifest={manifest_path} selections={} sha256={}",
            manifest.selections.len(),
            manifest.sha256
        ),
    }
    match &outcome.bindings_input {
        None => println!(
            "python-repair-trust check-driver: bindings=<none> verdict={verdict} (no records supplied; not_run is not a pass)"
        ),
        Some(input) => println!(
            "python-repair-trust check-driver: bindings={input} records={} violations={} verdict={verdict}",
            outcome.records.len(),
            outcome.violations.len()
        ),
    }
    for violation in &outcome.violations {
        println!("  violation: {violation}");
    }
    println!(
        "python-repair-trust check-driver verdict: {verdict} — binding structural validation only; no completed or correct repair is established and no verification phase is claimed"
    );
    println!("rerun: {RERUN_COMMAND}");

    let report = render_check_driver_json(&outcome)?;
    crate::write_report(CHECK_REPORT_JSON, &format!("{report}\n"))?;
    let markdown = render_check_driver_markdown(&outcome);
    crate::write_report(CHECK_REPORT_MD, &markdown)?;
    if verdict == "inconsistent" {
        return Err(format!(
            "python-repair-trust check-driver found {} violation(s); see target/ripr/reports/{CHECK_REPORT_JSON}",
            outcome.violations.len()
        ));
    }
    Ok(())
}
