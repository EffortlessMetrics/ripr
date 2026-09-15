//! `adjudicate`/`report` CLI parsing and entry points for the Python judged
//! panel (RIPR-SPEC-0092).

use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::branch_inventory::rfc3339_from_epoch_seconds;
use crate::python_judged_panel::INVENTORY_PATHS;
use crate::python_judged_panel_replay::RECORDS_DIR;

use super::adjudication::adjudicate_case_at;
use super::publish::{print_report_summary, verify_stored_bytes, write_report_generation};
use super::report::build_report_at;
use super::{ADJUDICATE_RERUN, ADJUDICATIONS_DIR, REPORT_OUT_DIR, REPORT_RERUN, REVIEWER_ENV};

// ---------------------------------------------------------------------------
// CLI entry points
// ---------------------------------------------------------------------------

pub(crate) fn run_report(args: &[String]) -> Result<(), String> {
    let mut records = RECORDS_DIR.to_string();
    let mut adjudications = ADJUDICATIONS_DIR.to_string();
    let mut out_dir = REPORT_OUT_DIR.to_string();
    let mut policy: Option<String> = None;
    let mut check = false;
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--records" => records = take_value(args, &mut index, "--records <dir>", REPORT_RERUN)?,
            "--adjudications" => {
                adjudications = take_value(args, &mut index, "--adjudications <dir>", REPORT_RERUN)?
            }
            "--threshold-policy" => {
                policy = Some(take_value(
                    args,
                    &mut index,
                    "--threshold-policy <path>",
                    REPORT_RERUN,
                )?)
            }
            "--out" => out_dir = take_value(args, &mut index, "--out <dir>", REPORT_RERUN)?,
            "--check" => check = true,
            other => {
                return Err(format!(
                    "unknown python-judged-panel report argument `{other}`; expected `report [--records <dir>] [--adjudications <dir>] [--threshold-policy <path>] [--out <dir>] [--check]`\nrerun: {REPORT_RERUN}"
                ));
            }
        }
        index += 1;
    }
    let report = build_report_at(
        Path::new("."),
        &INVENTORY_PATHS,
        Path::new(&records),
        &records,
        Path::new(&adjudications),
        &adjudications,
        policy.as_deref(),
    )?;
    let json_path = Path::new(&out_dir).join("report.json");
    let markdown_path = Path::new(&out_dir).join("report.md");
    if check {
        verify_stored_bytes(&json_path, &report.json)?;
        verify_stored_bytes(&markdown_path, &report.markdown)?;
        println!("verified against fresh render: {}", json_path.display());
        println!("verified against fresh render: {}", markdown_path.display());
    } else {
        write_report_generation(&json_path, &markdown_path, &report.json, &report.markdown)?;
        println!("wrote: {}", json_path.display());
        println!("wrote: {}", markdown_path.display());
    }
    print_report_summary(&report);
    println!("rerun: {REPORT_RERUN}");
    Ok(())
}

pub(crate) fn run_adjudicate(args: &[String]) -> Result<(), String> {
    let mut request = AdjudicationRequest {
        case_id: String::new(),
        verdict: String::new(),
        role: String::new(),
        identity: String::new(),
        evidence: Vec::new(),
        false_actionable: None,
        false_exposed: None,
        wrong_target: None,
        invalid_command: None,
        limitation_quality: None,
        notes: None,
        recorded_at: utc_now_rfc3339()?,
    };
    let mut adjudications = ADJUDICATIONS_DIR.to_string();
    let mut records = RECORDS_DIR.to_string();
    let mut identity_from_env = false;
    let mut index = 0;
    while index < args.len() {
        // The four assessment axes share one parse shape: a tri-state bool
        // whose undecided token is `undecided` for the error lattice (null is
        // an explicit undecided there) and `not-assessed` for the quality
        // axes.
        let flag = args[index].as_str();
        match flag {
            "--case" => {
                request.case_id = take_value(args, &mut index, "--case <id>", ADJUDICATE_RERUN)?
            }
            "--verdict" => {
                request.verdict = take_value(
                    args,
                    &mut index,
                    "--verdict <classification>",
                    ADJUDICATE_RERUN,
                )?
            }
            "--role" => {
                request.role = take_value(args, &mut index, "--role <role>", ADJUDICATE_RERUN)?
            }
            "--reviewer" => {
                request.identity =
                    take_value(args, &mut index, "--reviewer <identity>", ADJUDICATE_RERUN)?
            }
            "--evidence" => request.evidence.push(take_value(
                args,
                &mut index,
                "--evidence <ref>",
                ADJUDICATE_RERUN,
            )?),
            "--false-actionable" | "--false-exposed" | "--wrong-target" | "--invalid-command" => {
                let undecided = if flag.starts_with("--false") {
                    "undecided"
                } else {
                    "not-assessed"
                };
                let value = parse_bool_flag(
                    &take_value(
                        args,
                        &mut index,
                        &format!("{flag} true|false|{undecided}"),
                        ADJUDICATE_RERUN,
                    )?,
                    flag,
                    undecided,
                )?;
                match flag {
                    "--false-actionable" => request.false_actionable = value,
                    "--false-exposed" => request.false_exposed = value,
                    "--wrong-target" => request.wrong_target = value,
                    _ => request.invalid_command = value,
                }
            }
            "--limitation-quality" => {
                let value = take_value(
                    args,
                    &mut index,
                    "--limitation-quality precise|imprecise|wrong_kind|over_limited|not-assessed",
                    ADJUDICATE_RERUN,
                )?;
                if value != "not-assessed" {
                    request.limitation_quality = Some(value);
                }
            }
            "--notes" => {
                request.notes = Some(take_value(
                    args,
                    &mut index,
                    "--notes <text>",
                    ADJUDICATE_RERUN,
                )?)
            }
            "--adjudications" => {
                adjudications =
                    take_value(args, &mut index, "--adjudications <dir>", ADJUDICATE_RERUN)?
            }
            "--records" => {
                records = take_value(args, &mut index, "--records <dir>", ADJUDICATE_RERUN)?
            }
            other => {
                return Err(format!(
                    "unknown python-judged-panel adjudicate argument `{other}`; expected `adjudicate --case <id> --verdict <classification> --role <role> (--reviewer <identity> | env {REVIEWER_ENV}) --evidence <ref>`\nrerun: {ADJUDICATE_RERUN}"
                ));
            }
        }
        index += 1;
    }
    if request.identity.is_empty()
        && let Ok(from_env) = std::env::var(REVIEWER_ENV)
    {
        request.identity = from_env;
        identity_from_env = true;
    }
    let outcome = adjudicate_case_at(
        Path::new("."),
        &INVENTORY_PATHS,
        &adjudications,
        &records,
        &request,
    )?;
    println!(
        "adjudication recorded: case `{}` role `{}`{} — {}",
        request.case_id,
        request.role,
        if identity_from_env {
            format!(" (identity from {REVIEWER_ENV})")
        } else {
            String::new()
        },
        outcome
    );
    Ok(())
}

fn take_value(
    args: &[String],
    index: &mut usize,
    what: &str,
    rerun: &str,
) -> Result<String, String> {
    let value = args
        .get(*index + 1)
        .ok_or_else(|| format!("{what} requires a value\nrerun: {rerun}"))?;
    if value.starts_with("--") {
        return Err(format!(
            "{what} requires a value, found flag `{value}`\nrerun: {rerun}"
        ));
    }
    *index += 1;
    Ok(value.trim().to_string())
}

fn parse_bool_flag(value: &str, flag: &str, undecided: &str) -> Result<Option<bool>, String> {
    match value {
        "true" => Ok(Some(true)),
        "false" => Ok(Some(false)),
        token if token == undecided => Ok(None),
        other => Err(format!(
            "{flag} must be `true`, `false`, or `{undecided}`, found `{other}`\nrerun: {ADJUDICATE_RERUN}"
        )),
    }
}

fn utc_now_rfc3339() -> Result<String, String> {
    // FIX fqP (devin round 2): a pre-epoch system clock is a broken
    // environment. Fabricating `1970-01-01T00:00:00Z` would stamp valid-looking
    // provenance over an environment the run cannot trust, so the error is
    // named and the command fails closed instead.
    let epoch = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| {
            format!(
                "system clock is before the Unix epoch ({error}); refusing to stamp adjudication provenance with a fabricated timestamp"
            )
        })?
        .as_secs() as i64;
    Ok(rfc3339_from_epoch_seconds(epoch))
}
/// One adjudication request: the reviewer's own current judgment for a case.
/// Tests inject `recorded_at`; the CLI stamps UTC now.
pub(crate) struct AdjudicationRequest {
    pub(crate) case_id: String,
    pub(crate) verdict: String,
    pub(crate) role: String,
    pub(crate) identity: String,
    pub(crate) evidence: Vec<String>,
    pub(crate) false_actionable: Option<bool>,
    pub(crate) false_exposed: Option<bool>,
    pub(crate) wrong_target: Option<bool>,
    pub(crate) invalid_command: Option<bool>,
    pub(crate) limitation_quality: Option<String>,
    pub(crate) notes: Option<String>,
    pub(crate) recorded_at: String,
}
