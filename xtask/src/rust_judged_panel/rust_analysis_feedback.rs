//! Governed analyzer-feedback ledger for independently judged Rust cases
//! (#4796 / RIPR-SPEC-0199).
//!
//! This adapter turns the frozen #3806 judgment packet into one checked
//! feedback row per terminal case. It does not repair the analyzer, rewrite
//! judgments, run #4795 calibration, mutate GitHub, or close parent #3164.

use std::fs;
use std::path::Path;

use serde::Deserialize;
use sha2::{Digest, Sha256};

use super::parse_json_without_duplicate_keys;
use super::release_judgments::{RELEASE_JUDGMENTS_PATH, check_release_judgments_at};

mod lifecycle;
mod owners;
mod report;
mod schema;

#[cfg(test)]
mod tests;

pub(super) const LEDGER_PATH: &str = "metrics/rust-judged-behavior-panel/feedback-ledger.json";
const KIND: &str = "rust_judged_panel_feedback_ledger";
const AUTHORITY: &str = "EffortlessMetrics/ripr-swarm#4796";
const JUDGMENTS_AUTHORITY: &str = "EffortlessMetrics/ripr-swarm#3806";
const CALIBRATION_AUTHORITY: &str = "EffortlessMetrics/ripr-swarm#4795";
const RERUN_COMMAND: &str = "cargo xtask rust-judged-panel check";
const FEEDBACK_RERUN: &str = "cargo xtask rust-judged-panel feedback";
const DEFAULT_OUT_DIR: &str = "target/ripr/rust-judged-panel/feedback";

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct FeedbackLedger {
    schema_version: String,
    kind: String,
    authority: String,
    inherited_authorities: Vec<String>,
    excluded_authorities: Vec<String>,
    release_judgments_path: String,
    release_judgments_sha256: String,
    calibration: schema::CalibrationMeta,
    limits: Vec<String>,
    rows: Vec<schema::FeedbackRow>,
}

struct FeedbackCli {
    out_dir: String,
    check: bool,
}

pub(crate) fn run_feedback(args: &[String]) -> Result<(), String> {
    let cli = parse_feedback_args(args)?;
    let root = Path::new(".");
    let bundle = load_and_validate(root)?;
    let rendered = report::render(&bundle)?;
    let out_path = Path::new(&cli.out_dir);
    if cli.check {
        verify_staged(out_path, &rendered)?;
        println!(
            "rust-judged-panel feedback --check: staged reports match (rows={}, defects={})\nrerun: {FEEDBACK_RERUN}",
            rendered.row_count, rendered.analyzer_defect_count
        );
        return Ok(());
    }
    write_staging(out_path, &rendered)?;
    println!(
        "rust-judged-panel feedback: wrote {} and {} (rows={}, defects={})\nledger never mutates GitHub, judgments, calibration, or analyzer behavior\nrerun: {FEEDBACK_RERUN}",
        out_path.join("feedback.json").display(),
        out_path.join("feedback.md").display(),
        rendered.row_count,
        rendered.analyzer_defect_count
    );
    Ok(())
}

fn parse_feedback_args(args: &[String]) -> Result<FeedbackCli, String> {
    let mut out_dir = DEFAULT_OUT_DIR.to_string();
    let mut check = false;
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--out" => out_dir = take_value(args, &mut index, "--out <dir>")?,
            "--check" => check = true,
            other => {
                return Err(format!(
                    "unknown rust-judged-panel feedback argument `{other}`; expected `feedback [--out <dir>] [--check]`\nrerun: {FEEDBACK_RERUN}"
                ));
            }
        }
        index += 1;
    }
    Ok(FeedbackCli { out_dir, check })
}

pub(super) fn validate_at(root: &Path) -> Result<(), String> {
    load_and_validate(root).map(|_| ())
}

struct ValidatedBundle {
    ledger: FeedbackLedger,
    facts: Vec<schema::JudgmentFact>,
    judgments_sha256: String,
}

fn load_and_validate(root: &Path) -> Result<ValidatedBundle, String> {
    // The frozen #3806 packet is validated by its own semantic owner first:
    // selection binding, typed rows, and one judgment per case are enforced
    // there, so this ledger never derives facts from a state the judgment
    // authority would reject.
    check_release_judgments_at(root)?;
    let judgments_path = root.join(RELEASE_JUDGMENTS_PATH);
    let judgments_bytes = fs::read(&judgments_path)
        .map_err(|error| format!("read `{RELEASE_JUDGMENTS_PATH}`: {error}"))?;
    let judgments_sha256 = sha256_bytes(&judgments_bytes);
    let judgments_body = String::from_utf8(judgments_bytes.clone()).map_err(|error| {
        format!("release judgments `{RELEASE_JUDGMENTS_PATH}` are not UTF-8: {error}")
    })?;
    let judgments_value = parse_json_without_duplicate_keys(&judgments_body)
        .map_err(|error| format!("parse `{RELEASE_JUDGMENTS_PATH}`: {error}"))?;
    let facts = schema::judgment_facts(&judgments_value)?;

    let ledger_path = root.join(LEDGER_PATH);
    let ledger_body = fs::read_to_string(&ledger_path).map_err(|error| {
        format!("read feedback ledger `{LEDGER_PATH}`: {error}\nrerun: {RERUN_COMMAND}")
    })?;
    let ledger_value = parse_json_without_duplicate_keys(&ledger_body)
        .map_err(|error| format!("parse `{LEDGER_PATH}`: {error}"))?;
    let ledger: FeedbackLedger = serde_json::from_value(ledger_value)
        .map_err(|error| format!("parse `{LEDGER_PATH}`: {error}"))?;

    let mut violations = validate_bundle(root, &ledger, &facts, &judgments_sha256);
    violations.sort();
    violations.dedup();
    if violations.is_empty() {
        Ok(ValidatedBundle {
            ledger,
            facts,
            judgments_sha256,
        })
    } else {
        Err(format!(
            "Rust judged-panel feedback ledger `{LEDGER_PATH}` has {} violation(s):\n- {}\nrerun: {RERUN_COMMAND}",
            violations.len(),
            violations.join("\n- ")
        ))
    }
}

fn validate_bundle(
    root: &Path,
    ledger: &FeedbackLedger,
    facts: &[schema::JudgmentFact],
    judgments_sha256: &str,
) -> Vec<String> {
    let mut violations = Vec::new();
    if ledger.schema_version != "0.1" {
        violations.push(format!(
            "schema_version: expected `0.1`, found `{}`",
            ledger.schema_version
        ));
    }
    if ledger.kind != KIND {
        violations.push(format!("kind: expected `{KIND}`"));
    }
    if ledger.authority != AUTHORITY {
        violations.push(format!("authority: expected `{AUTHORITY}`"));
    }
    if !ledger
        .inherited_authorities
        .iter()
        .any(|item| item == JUDGMENTS_AUTHORITY)
    {
        violations.push(format!(
            "inherited_authorities: must retain `{JUDGMENTS_AUTHORITY}`"
        ));
    }
    if !ledger
        .excluded_authorities
        .iter()
        .any(|item| item == CALIBRATION_AUTHORITY)
    {
        violations.push(format!(
            "excluded_authorities: must exclude `{CALIBRATION_AUTHORITY}`; runtime calibration is owned there, not in this ledger"
        ));
    }
    if ledger
        .inherited_authorities
        .iter()
        .any(|item| item == CALIBRATION_AUTHORITY)
    {
        violations.push(format!(
            "inherited_authorities: must not absorb `{CALIBRATION_AUTHORITY}`; the calibration boundary stays excluded"
        ));
    }
    if ledger.release_judgments_path != RELEASE_JUDGMENTS_PATH {
        violations.push(format!(
            "release_judgments_path: expected `{RELEASE_JUDGMENTS_PATH}`"
        ));
    }
    if ledger.release_judgments_sha256 != judgments_sha256 {
        violations.push(format!(
            "release_judgments_sha256: ledger binds `{}` but `{RELEASE_JUDGMENTS_PATH}` is `{judgments_sha256}`; stale closure evidence is invalid",
            ledger.release_judgments_sha256
        ));
    }
    if ledger.limits.is_empty() || ledger.limits.iter().all(|item| item.trim().is_empty()) {
        violations.push("limits: at least one explicit limitation is required".into());
    }
    violations.extend(schema::validate_calibration(&ledger.calibration));
    violations.extend(schema::validate_coverage(&ledger.rows, facts));
    for row in &ledger.rows {
        let Some(fact) = facts.iter().find(|fact| fact.case_id == row.case_id) else {
            continue;
        };
        if row.runtime_calibration.status != ledger.calibration.status {
            violations.push(format!(
                "{}: runtime_calibration.status `{}` contradicts the ledger-level calibration status `{}`",
                row.case_id, row.runtime_calibration.status, ledger.calibration.status
            ));
        }
        violations.extend(schema::validate_row(row, fact, root));
        violations.extend(lifecycle::validate_row(row));
        violations.extend(owners::validate_row(row));
    }
    violations.extend(owners::validate_duplicates(&ledger.rows));
    violations.extend(schema::validate_order(&ledger.rows));
    violations
}

fn write_staging(out_path: &Path, rendered: &report::RenderedFeedback) -> Result<(), String> {
    fs::create_dir_all(out_path)
        .map_err(|error| format!("create feedback staging `{}`: {error}", out_path.display()))?;
    for (name, bytes) in [
        ("feedback.json", rendered.json.as_bytes()),
        ("feedback.md", rendered.markdown.as_bytes()),
    ] {
        let path = out_path.join(name);
        if let Ok(metadata) = fs::symlink_metadata(&path)
            && metadata.file_type().is_symlink()
        {
            return Err(format!(
                "write `{}`: refusing to follow an existing symlink; remove it and rerun",
                path.display()
            ));
        }
        fs::write(&path, bytes).map_err(|error| format!("write `{}`: {error}", path.display()))?;
    }
    Ok(())
}

fn verify_staged(out_path: &Path, rendered: &report::RenderedFeedback) -> Result<(), String> {
    if !out_path.is_dir() {
        return Err(format!(
            "feedback staging directory `{}` does not exist; run `{FEEDBACK_RERUN}` first\nrerun: {FEEDBACK_RERUN}",
            out_path.display()
        ));
    }
    for (name, expected) in [
        ("feedback.json", rendered.json.as_bytes()),
        ("feedback.md", rendered.markdown.as_bytes()),
    ] {
        let path = out_path.join(name);
        let actual = fs::read(&path).map_err(|error| {
            format!(
                "read staged feedback `{}`: {error}; run `{FEEDBACK_RERUN}` first\nrerun: {FEEDBACK_RERUN}",
                path.display()
            )
        })?;
        if actual != expected {
            return Err(format!(
                "staged feedback `{}` does not match a fresh derivation; re-run `{FEEDBACK_RERUN}` to restage\nrerun: {FEEDBACK_RERUN}",
                path.display()
            ));
        }
    }
    Ok(())
}

fn take_value(args: &[String], index: &mut usize, flag: &str) -> Result<String, String> {
    let value = args.get(*index + 1).ok_or_else(|| {
        format!("rust-judged-panel feedback missing value for {flag}\nrerun: {FEEDBACK_RERUN}")
    })?;
    *index += 1;
    Ok(value.clone())
}

fn sha256_bytes(bytes: &[u8]) -> String {
    format!("sha256:{:x}", Sha256::digest(bytes))
}

#[cfg(test)]
impl FeedbackLedger {
    fn from_json(value: serde_json::Value) -> Result<Self, String> {
        serde_json::from_value(value).map_err(|error| error.to_string())
    }
}

#[cfg(test)]
fn validate_bundle_for_test(
    ledger: &FeedbackLedger,
    facts: &[schema::JudgmentFact],
    judgments_sha256: &str,
) -> Vec<String> {
    validate_bundle(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap_or(Path::new(".")),
        ledger,
        facts,
        judgments_sha256,
    )
}

#[cfg(test)]
fn write_staging_for_test(
    out_path: &Path,
    rendered: &report::RenderedFeedback,
) -> Result<(), String> {
    write_staging(out_path, rendered)
}

#[cfg(test)]
fn verify_staged_for_test(
    out_path: &Path,
    rendered: &report::RenderedFeedback,
) -> Result<(), String> {
    verify_staged(out_path, rendered)
}

#[cfg(test)]
fn parse_feedback_args_for_test(args: &[String]) -> Result<(String, bool), String> {
    parse_feedback_args(args).map(|cli| (cli.out_dir, cli.check))
}
