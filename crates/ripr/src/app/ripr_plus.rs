//! `ripr plus` — binary-first RIPR+ repo receipt (composition-only).
//!
//! Ports `cargo xtask ripr-plus` into the `ripr` binary so downstream
//! consumers can produce the repo-wide RIPR+ quality-gate receipt without
//! compiling their own xtask.
//!
//! # Binary-first scope
//!
//! Unlike the xtask (which ran an expensive full-repo scan via subprocess for
//! its no-args and `--gap-ledger` paths), the binary-first `ripr plus` is
//! **artifact-composition-only**: it never runs an in-process full-repo
//! exposure scan inside a subcommand. It accepts one of two pre-computed
//! artifacts:
//!
//! 1. `--repo-exposure-summary <path>` — pure composition: reads a
//!    `repo-exposure-summary-json` artifact and composes the RIPR+ receipt
//!    from its `metrics`, `top_files`, and `reason_breakdown`.
//! 2. `--gap-ledger <path>` — composes a `repo-badge-json` from the gap
//!    decision ledger (cheap ledger-only composition, no repo scan) and then
//!    builds the receipt from the badge counts.
//!
//! When neither input is provided, the command prints a clear error telling
//! the user to supply one of the two artifacts. The xtask's third path
//! (no-args full-repo scan) is intentionally out of scope: that scan belongs
//! in `ripr check --format repo-exposure-summary-json`, not inside a
//! receipt-composition subcommand.
//!
//! Output: `target/ripr/reports/ripr-plus.{json,md}`.

use crate::config::RiprConfig;
use crate::output;
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

const RIPR_PLUS_JSON: &str = "target/ripr/reports/ripr-plus.json";
const RIPR_PLUS_MD: &str = "target/ripr/reports/ripr-plus.md";

#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct RiprPlusOptions {
    repo_exposure_summary: Option<PathBuf>,
    gap_ledger: Option<PathBuf>,
}

/// Entry point for `ripr plus`. Composes a RIPR+ repo receipt from a
/// pre-computed `repo-exposure-summary-json` or `--gap-ledger` artifact and
/// writes `ripr-plus.{json,md}`. Never runs an in-process full-repo scan.
pub(crate) fn run_ripr_plus(args: &[String]) -> Result<(), String> {
    if args.iter().any(|arg| arg == "--help" || arg == "-h") {
        print_help();
        return Ok(());
    }
    let options = parse_options(args)?;
    let repo = repo_root()?;
    let head = git_head(&repo);
    match ripr_plus_receipt_from_options(&options, &head) {
        Ok(receipt) => write_receipt(&repo, &receipt),
        Err(err) => {
            let receipt = error_ripr_plus_receipt(&head, &err);
            write_receipt(&repo, &receipt)
        }
    }
}

fn parse_options(args: &[String]) -> Result<RiprPlusOptions, String> {
    let mut options = RiprPlusOptions::default();
    let mut index = 0usize;
    while index < args.len() {
        match args[index].as_str() {
            "--check" => {}
            "--gap-ledger" => {
                index += 1;
                options.gap_ledger =
                    Some(PathBuf::from(non_empty_arg(args, index, "--gap-ledger")?));
            }
            "--repo-exposure-summary" => {
                index += 1;
                options.repo_exposure_summary = Some(PathBuf::from(non_empty_arg(
                    args,
                    index,
                    "--repo-exposure-summary",
                )?));
            }
            other => return Err(format!("unknown plus argument `{other}`")),
        }
        index += 1;
    }
    if options.gap_ledger.is_some() && options.repo_exposure_summary.is_some() {
        return Err(
            "plus accepts either --gap-ledger or --repo-exposure-summary, not both".to_string(),
        );
    }
    if options.gap_ledger.is_none() && options.repo_exposure_summary.is_none() {
        return Err(format!(
            "plus requires either --repo-exposure-summary <path> or --gap-ledger <path>; \
             the binary-first `ripr plus` composes a receipt from a pre-computed artifact and \
             does not run an in-process full-repo scan. \
             Run `ripr check --root . --format repo-exposure-summary-json` first, \
             then `ripr plus --repo-exposure-summary {}`.",
            RIPR_PLUS_JSON
        ));
    }
    Ok(options)
}

fn non_empty_arg<'a>(args: &'a [String], index: usize, flag: &str) -> Result<&'a str, String> {
    let Some(value) = args.get(index) else {
        return Err(format!("plus {flag} requires a path"));
    };
    if value.trim().is_empty() {
        return Err(format!("plus {flag} requires a non-empty path"));
    }
    Ok(value)
}

fn print_help() {
    println!("usage: ripr plus --repo-exposure-summary <path> | --gap-ledger <path> [--check]");
    println!();
    println!("Options:");
    println!(
        "  --repo-exposure-summary <path>  Compose the receipt from a repo-exposure-summary-json artifact (pure composition)."
    );
    println!(
        "  --gap-ledger <path>             Compose the receipt from a gap decision ledger (ledger-only composition; no repo scan)."
    );
    println!("  --check                         Accepted for xtask parity (no-op).");
    println!();
    println!("Outputs:");
    println!("  {RIPR_PLUS_JSON}");
    println!("  {RIPR_PLUS_MD}");
    println!();
    println!("This receipt is the repo-wide RIPR+ quality-gate input. It uses the");
    println!("public canonical actionable gap basis and does not count raw seam");
    println!("inventory as unresolved debt. The binary-first `ripr plus` is");
    println!("artifact-composition-only: it does not run an in-process full-repo scan.");
}

fn ripr_plus_receipt_from_options(options: &RiprPlusOptions, head: &str) -> Result<Value, String> {
    if let Some(summary_path) = options.repo_exposure_summary.as_deref() {
        let repo_summary_json = read_repo_exposure_summary_artifact(summary_path)?;
        return ripr_plus_receipt_from_repo_exposure_summary_json_with_source(
            &repo_summary_json,
            head,
            &format!(
                "ripr plus --repo-exposure-summary {}",
                normalize_path(summary_path)
            ),
            Some(summary_path),
        );
    }

    if let Some(gap_ledger) = options.gap_ledger.as_deref() {
        let repo_badge_json = render_gap_ledger_repo_badge_json(gap_ledger)?;
        return ripr_plus_receipt_from_repo_badge_json(&repo_badge_json, head, Some(gap_ledger));
    }

    // Unreachable: parse_options rejects the no-input case. Kept for clarity.
    Err("plus requires either --repo-exposure-summary or --gap-ledger".to_string())
}

/// Composes a `repo-badge-json` string from a gap decision ledger using the
/// crate-private badge model. This mirrors `ripr check --gap-ledger
/// --format repo-badge-json` but stays in-process and ledger-only (no repo
/// scan). The resulting basis is `gap_decision_ledger`, which is a valid
/// RIPR+ basis.
fn render_gap_ledger_repo_badge_json(gap_ledger: &Path) -> Result<String, String> {
    let text = fs::read_to_string(gap_ledger)
        .map_err(|err| format!("failed to read gap ledger {}: {err}", gap_ledger.display()))?;
    let policy = output::badge::BadgePolicy {
        suppressions_path: RiprConfig::default().suppressions().display_path(),
        ..output::badge::BadgePolicy::default()
    };
    let mut summary = output::badge::repo_gap_ledger_badge_summary_from_json(
        &text,
        output::badge::BadgeKind::Ripr,
        policy,
    )?;
    output::badge::attach_public_projection(&mut summary, &gap_ledger.display().to_string());
    Ok(output::badge::render_native_json(&summary))
}

fn read_repo_exposure_summary_artifact(path: &Path) -> Result<String, String> {
    let json = fs::read_to_string(path)
        .map_err(|err| format!("failed to read {}: {err}", normalize_path(path)))?;
    let value: Value = serde_json::from_str(&json).map_err(|err| {
        format!(
            "failed to parse repo exposure summary artifact {}: {err}",
            normalize_path(path)
        )
    })?;
    if audit_bool(&value, &["runtime_status", "downstream_consumable"]) == Some(false) {
        return Err(format!(
            "repo exposure summary artifact {} is not downstream consumable (basis {:?}, run_status {:?}); \
             rerun `ripr check --root . --format repo-exposure-summary-json` with enough time to produce \
             canonical_actionable_gap data, or pass --gap-ledger to use an existing gap decision ledger",
            normalize_path(path),
            value.get("basis").and_then(Value::as_str),
            value.get("run_status").and_then(Value::as_str)
        ));
    }
    Ok(json)
}

fn ripr_plus_receipt_from_repo_badge_json(
    json: &str,
    head: &str,
    gap_ledger: Option<&Path>,
) -> Result<Value, String> {
    let badge = badge_native_audit_snapshot(json)?;
    ripr_plus_receipt_from_badge(&badge, head, gap_ledger)
}

fn ripr_plus_receipt_from_badge(
    badge: &BadgeNativeAuditSnapshot,
    head: &str,
    gap_ledger: Option<&Path>,
) -> Result<Value, String> {
    if !ripr_plus_accepts_badge_basis(&badge.basis) {
        return Err(format!(
            "ripr plus requires repo-badge-json with canonical actionable or gap decision ledger basis, got {:?}",
            badge.basis
        ));
    }
    let unresolved = badge
        .counts
        .get("unsuppressed_exposure_gaps")
        .copied()
        .ok_or_else(|| {
            "repo-badge-json counts are missing unsuppressed_exposure_gaps".to_string()
        })?;
    let suppressed = badge
        .counts
        .get("suppressed_exposure_gaps")
        .copied()
        .unwrap_or(0)
        + badge
            .counts
            .get("suppressed_test_efficiency_findings")
            .copied()
            .unwrap_or(0);
    let raw_seams = badge
        .counts
        .get("raw_seams")
        .or_else(|| badge.counts.get("analyzed_seams"))
        .copied();
    let status = if unresolved == 0 { "pass" } else { "warn" };

    let source_command = if let Some(gap_ledger) = gap_ledger {
        format!(
            "ripr check --root . --format repo-badge-json --gap-ledger {}",
            normalize_path(gap_ledger)
        )
    } else {
        "ripr check --root . --format repo-badge-json".to_string()
    };

    Ok(json!({
        "schema_version": "0.1",
        "status": status,
        "basis": &badge.basis,
        "source_format": "repo-badge-json",
        "source_command": source_command,
        "unresolved": unresolved,
        "top_files": [],
        "suppressed": suppressed,
        "head": head,
        "counts": &badge.counts,
        "reason_counts": &badge.reason_counts,
        "raw_inventory": {
            "raw_seams": raw_seams,
            "debt_basis": false,
            "note": "Raw seam inventory is supporting analyzer pressure only; it is not the RIPR+ unresolved debt counter."
        },
        "basis_note": "RIPR+ unresolved is sourced from counts.unsuppressed_exposure_gaps in repo-badge-json when the badge basis is canonical_actionable_gap or gap_decision_ledger. top_files is intentionally empty because bounded repo-badge-json does not expose per-file debt and this command must not run full repo-exposure-json.",
        "warnings": &badge.warnings,
        "non_claims": [
            "not raw seam inventory",
            "not runtime mutation confirmation",
            "not coverage",
            "not badge endpoint regeneration"
        ]
    }))
}

fn ripr_plus_accepts_badge_basis(basis: &str) -> bool {
    matches!(basis, "canonical_actionable_gap" | "gap_decision_ledger")
}

fn ripr_plus_receipt_from_repo_exposure_summary_json_with_source(
    json: &str,
    head: &str,
    source_command: &str,
    source_artifact: Option<&Path>,
) -> Result<Value, String> {
    let summary: Value = serde_json::from_str(json)
        .map_err(|err| format!("failed to parse repo-exposure-summary-json: {err}"))?;
    let summary_format = json_string_field_value(&summary, "format");
    if summary_format != "repo-exposure-summary-json" {
        return Err(format!(
            "ripr plus requires repo-exposure-summary-json, got {summary_format:?}"
        ));
    }
    let basis = json_string_field_value(&summary, "basis");
    if basis != "canonical_actionable_gap" {
        return Err(format!(
            "ripr plus requires repo-exposure-summary-json with canonical_actionable_gap basis, got {basis:?}"
        ));
    }

    let unresolved =
        json_path_usize_required(&summary, &["metrics", "unsuppressed_exposure_gaps"])?;
    let suppressed =
        json_path_usize_optional(&summary, &["metrics", "suppressed_exposure_gaps"]).unwrap_or(0);
    let raw_seams = json_path_usize_optional(&summary, &["metrics", "raw_seams"]);
    let headline_eligible_seams =
        json_path_usize_optional(&summary, &["metrics", "headline_eligible_seams"]);
    let canonical_gap_records =
        json_path_usize_optional(&summary, &["metrics", "canonical_gap_records"]);
    let raw_actionable_seam_records =
        json_path_usize_optional(&summary, &["metrics", "raw_actionable_seam_records"]);
    let reason_counts =
        json_path_object_usize_map(&summary, &["reason_breakdown", "actionability"]);
    let counts = json_path_object_usize_map(&summary, &["metrics"]);
    let top_files = summary
        .get("top_files")
        .and_then(Value::as_array)
        .cloned()
        .ok_or_else(|| "repo-exposure-summary-json is missing top_files array".to_string())?;
    let status = if unresolved == 0 { "pass" } else { "warn" };

    Ok(json!({
        "schema_version": "0.1",
        "status": status,
        "basis": basis,
        "source_format": "repo-exposure-summary-json",
        "source_command": source_command,
        "source_artifact": source_artifact.map(normalize_path),
        "unresolved": unresolved,
        "top_files": top_files,
        "suppressed": suppressed,
        "head": head,
        "counts": counts,
        "metrics": &summary["metrics"],
        "reason_counts": reason_counts,
        "reason_breakdown": &summary["reason_breakdown"],
        "limits": &summary["limits"],
        "raw_inventory": {
            "raw_seams": raw_seams,
            "headline_eligible_seams": headline_eligible_seams,
            "canonical_gap_records": canonical_gap_records,
            "raw_actionable_seam_records": raw_actionable_seam_records,
            "debt_basis": false,
            "note": "Raw seam inventory is supporting analyzer pressure only; it is not the RIPR+ unresolved debt counter."
        },
        "basis_note": "RIPR+ unresolved is sourced from metrics.unsuppressed_exposure_gaps in repo-exposure-summary-json. top_files is bounded by the summary format and this command does not run full repo-exposure-json.",
        "warnings": [],
        "non_claims": [
            "not raw seam inventory",
            "not runtime mutation confirmation",
            "not coverage",
            "not badge endpoint regeneration"
        ]
    }))
}

fn error_ripr_plus_receipt(head: &str, err: &str) -> Value {
    let machine_readable_cause = if err.contains("timed out") {
        "evaluation_timeout"
    } else {
        "evaluation_error"
    };
    let first_warning = err.lines().next().unwrap_or(err).trim().to_string();
    json!({
        "schema_version": "0.1",
        "status": "indeterminate",
        "basis": null,
        "source_format": null,
        "source_command": null,
        "source_artifact": null,
        "unresolved": null,
        "top_files": [],
        "suppressed": null,
        "head": head,
        "counts": {},
        "reason_counts": {},
        "machine_readable_cause": machine_readable_cause,
        "raw_inventory": {
            "raw_seams": null,
            "debt_basis": false,
            "note": "Raw seam inventory is supporting analyzer pressure only; it is not the RIPR+ unresolved debt counter."
        },
        "basis_note": "Evaluation did not complete; no basis or debt count is available.",
        "warnings": [first_warning],
        "non_claims": [
            "not raw seam inventory",
            "not runtime mutation confirmation",
            "not coverage",
            "not badge endpoint regeneration"
        ]
    })
}

fn ripr_plus_receipt_markdown(receipt: &Value) -> String {
    let mut body = String::from("# ripr+ Repo Receipt\n\n");
    body.push_str("This report is the repo-wide RIPR+ quality-gate input. It uses the public canonical actionable gap basis and does not count raw seam inventory as unresolved debt.\n\n");
    body.push_str("## Basis\n\n");
    body.push_str("| Field | Value |\n");
    body.push_str("| --- | --- |\n");
    body.push_str(&format!(
        "| Status | `{}` |\n",
        markdown_cell(&json_string_field_value(receipt, "status"))
    ));
    body.push_str(&format!(
        "| Basis | `{}` |\n",
        markdown_cell(&json_string_field_value(receipt, "basis"))
    ));
    body.push_str(&format!(
        "| Source format | `{}` |\n",
        markdown_cell(&json_string_field_value(receipt, "source_format"))
    ));
    let is_indeterminate = receipt
        .get("status")
        .and_then(Value::as_str)
        .is_some_and(|s| s == "indeterminate");
    let unresolved_is_null = receipt
        .get("unresolved")
        .is_some_and(serde_json::Value::is_null);
    if is_indeterminate || unresolved_is_null {
        body.push_str("| Unresolved | N/A — unknown (evaluation did not complete) |\n");
    } else {
        body.push_str(&format!(
            "| Unresolved | {} |\n",
            receipt
                .get("unresolved")
                .and_then(Value::as_u64)
                .unwrap_or(0)
        ));
    }
    body.push_str(&format!(
        "| Suppressed | {} |\n",
        receipt
            .get("suppressed")
            .and_then(Value::as_u64)
            .unwrap_or(0)
    ));
    body.push_str(&format!(
        "| Head | `{}` |\n",
        markdown_cell(&json_string_field_value(receipt, "head"))
    ));
    if is_indeterminate || unresolved_is_null {
        body.push_str("\n## Evaluation Status\n\n");
        body.push_str("> **Indeterminate** — the evaluation did not complete. No gap count is available from this run. This receipt must not be treated as evidence of zero unresolved gaps.\n\n");
        if let Some(warnings) = receipt
            .get("warnings")
            .and_then(Value::as_array)
            .filter(|w| !w.is_empty())
        {
            body.push_str("### Warnings\n\n");
            for w in warnings {
                let msg = w.as_str().unwrap_or_default();
                if !msg.is_empty() {
                    body.push_str(&format!("- {}\n", msg.replace('|', "\\|")));
                }
            }
            body.push('\n');
        }
        return body;
    }
    body.push_str("\n## Reason Counts\n\n");
    let reason_counts = json_object_usize_map(receipt, "reason_counts");
    append_count_table(&mut body, &reason_counts);
    body.push_str("\n## Top Files\n\n");
    append_ripr_plus_top_files_table(&mut body, receipt);
    body
}

fn append_ripr_plus_top_files_table(body: &mut String, receipt: &Value) {
    let Some(top_files) = receipt.get("top_files").and_then(Value::as_array) else {
        body.push_str("No bounded top-file summary is available.\n");
        return;
    };
    if top_files.is_empty() {
        body.push_str("No bounded top-file summary is available from this source; this command intentionally does not run full `repo-exposure-json`.\n");
        return;
    }

    body.push_str("| File | Unresolved | Canonical gaps | Headline-eligible seams | Raw seams |\n");
    body.push_str("| --- | ---: | ---: | ---: | ---: |\n");
    for file in top_files {
        body.push_str(&format!(
            "| `{}` | {} | {} | {} | {} |\n",
            markdown_cell(&json_string_field_value(file, "file")),
            file.get("unsuppressed_exposure_gaps")
                .and_then(Value::as_u64)
                .unwrap_or(0),
            file.get("canonical_gap_records")
                .and_then(Value::as_u64)
                .unwrap_or(0),
            file.get("headline_eligible_seams")
                .and_then(Value::as_u64)
                .unwrap_or(0),
            file.get("raw_seams").and_then(Value::as_u64).unwrap_or(0)
        ));
    }
    body.push_str(
        "\nTop files are bounded by `repo-exposure-summary-json`; this command does not run full `repo-exposure-json`.\n",
    );
}

fn write_receipt(repo: &Path, receipt: &Value) -> Result<(), String> {
    let json = serde_json::to_string_pretty(receipt)
        .map_err(|err| format!("failed to serialize ripr-plus receipt: {err}"))?;
    let markdown = ripr_plus_receipt_markdown(receipt);

    write_parented_file(
        &repo.join(RIPR_PLUS_JSON),
        RIPR_PLUS_JSON,
        format!("{json}\n"),
    )?;
    write_parented_file(&repo.join(RIPR_PLUS_MD), RIPR_PLUS_MD, markdown)?;
    println!("Wrote {RIPR_PLUS_JSON}");
    println!("Wrote {RIPR_PLUS_MD}");
    Ok(())
}

// ── JSON helpers (ported from xtask, kept local to this module) ──

#[derive(Clone, Debug, Eq, PartialEq)]
struct BadgeNativeAuditSnapshot {
    label: String,
    kind: String,
    scope: String,
    basis: String,
    message: String,
    status: String,
    color: String,
    counts: BTreeMap<String, usize>,
    reason_counts: BTreeMap<String, usize>,
    warnings: Vec<String>,
}

fn badge_native_audit_snapshot(json: &str) -> Result<BadgeNativeAuditSnapshot, String> {
    let value: Value = serde_json::from_str(json)
        .map_err(|err| format!("failed to parse native badge JSON: {err}"))?;
    Ok(BadgeNativeAuditSnapshot {
        label: json_string_field_value(&value, "label"),
        kind: json_string_field_value(&value, "kind"),
        scope: json_string_field_value(&value, "scope"),
        basis: json_string_field_value(&value, "basis"),
        message: json_string_field_value(&value, "message"),
        status: json_string_field_value(&value, "status"),
        color: json_string_field_value(&value, "color"),
        counts: json_object_usize_map(&value, "counts"),
        reason_counts: json_object_usize_map(&value, "reason_counts"),
        warnings: json_array_string_values(&value, "warnings"),
    })
}

fn json_string_field_value(value: &Value, key: &str) -> String {
    value
        .get(key)
        .and_then(Value::as_str)
        .unwrap_or("unknown")
        .to_string()
}

fn json_object_usize_map(value: &Value, key: &str) -> BTreeMap<String, usize> {
    value
        .get(key)
        .map(json_value_object_usize_map)
        .unwrap_or_default()
}

fn json_path_object_usize_map(value: &Value, path: &[&str]) -> BTreeMap<String, usize> {
    audit_get(value, path)
        .map(json_value_object_usize_map)
        .unwrap_or_default()
}

fn json_value_object_usize_map(value: &Value) -> BTreeMap<String, usize> {
    value
        .as_object()
        .map(|object| {
            object
                .iter()
                .filter_map(|(key, value)| value.as_u64().map(|count| (key.clone(), count)))
                .filter_map(|(key, count)| usize::try_from(count).ok().map(|count| (key, count)))
                .collect()
        })
        .unwrap_or_default()
}

fn json_path_usize_optional(value: &Value, path: &[&str]) -> Option<usize> {
    audit_get(value, path)
        .and_then(Value::as_u64)
        .and_then(|count| usize::try_from(count).ok())
}

fn json_path_usize_required(value: &Value, path: &[&str]) -> Result<usize, String> {
    json_path_usize_optional(value, path)
        .ok_or_else(|| format!("JSON is missing numerical field {}", path.join(".")))
}

fn json_array_string_values(value: &Value, key: &str) -> Vec<String> {
    value
        .get(key)
        .and_then(Value::as_array)
        .map(|values| {
            values
                .iter()
                .filter_map(Value::as_str)
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default()
}

fn audit_get<'a>(value: &'a Value, path: &[&str]) -> Option<&'a Value> {
    let mut current = value;
    for segment in path {
        current = current.get(*segment)?;
    }
    Some(current)
}

fn audit_bool(value: &Value, path: &[&str]) -> Option<bool> {
    audit_get(value, path).and_then(Value::as_bool)
}

fn markdown_cell(value: &str) -> String {
    value.replace('|', "\\|")
}

fn append_count_table(body: &mut String, counts: &BTreeMap<String, usize>) {
    if counts.is_empty() {
        body.push_str("No count breakdown is available.\n");
        return;
    }
    body.push_str("| Class | Count |\n| --- | ---: |\n");
    for (key, value) in counts {
        body.push_str(&format!("| `{}` | {} |\n", markdown_cell(key), value));
    }
}

fn normalize_path(path: &Path) -> String {
    path.to_string_lossy()
        .replace('\\', "/")
        .trim_start_matches("./")
        .to_string()
}

fn git_head(repo: &Path) -> String {
    std::process::Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(["rev-parse", "HEAD"])
        .output()
        .ok()
        .and_then(|output| {
            if output.status.success() {
                String::from_utf8(output.stdout).ok()
            } else {
                None
            }
        })
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "unknown".to_string())
}

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
    fn parse_rejects_both_inputs() -> Result<(), String> {
        match parse_options(&[
            "--gap-ledger".to_string(),
            "ledger.json".to_string(),
            "--repo-exposure-summary".to_string(),
            "summary.json".to_string(),
        ]) {
            Err(msg) if msg.contains("not both") => Ok(()),
            other => Err(format!("expected `not both` error, got {other:?}")),
        }
    }

    #[test]
    fn parse_requires_an_input() -> Result<(), String> {
        match parse_options(&[]) {
            Err(msg) if msg.contains("requires either") => Ok(()),
            other => Err(format!("expected `requires either` error, got {other:?}")),
        }
    }

    #[test]
    fn parse_accepts_repo_exposure_summary() -> Result<(), String> {
        let options = parse_options(&[
            "--repo-exposure-summary".to_string(),
            "summary.json".to_string(),
        ])?;
        assert_eq!(
            options.repo_exposure_summary,
            Some(PathBuf::from("summary.json"))
        );
        assert!(options.gap_ledger.is_none());
        Ok(())
    }

    #[test]
    fn parse_rejects_empty_gap_ledger() -> Result<(), String> {
        match parse_options(&["--gap-ledger".to_string(), "   ".to_string()]) {
            Err(msg) if msg.contains("non-empty path") => Ok(()),
            other => Err(format!("expected `non-empty path` error, got {other:?}")),
        }
    }

    #[test]
    fn parse_rejects_unknown_arg() -> Result<(), String> {
        match parse_options(&["--bogus".to_string()]) {
            Err(msg) if msg.contains("--bogus") => Ok(()),
            other => Err(format!("expected unknown-arg error, got {other:?}")),
        }
    }

    #[test]
    fn receipt_from_repo_exposure_summary_extracts_counts() -> Result<(), String> {
        let summary = json!({
            "format": "repo-exposure-summary-json",
            "basis": "canonical_actionable_gap",
            "metrics": {
                "unsuppressed_exposure_gaps": 3,
                "suppressed_exposure_gaps": 1,
                "raw_seams": 9,
                "headline_eligible_seams": 7
            },
            "reason_breakdown": {
                "actionability": { "missing_discriminator": 2, "no_static_path": 1 }
            },
            "top_files": [
                {
                    "file": "src/pricing.rs",
                    "unsuppressed_exposure_gaps": 2,
                    "canonical_gap_records": 2,
                    "headline_eligible_seams": 4,
                    "raw_seams": 5
                }
            ],
            "limits": { "max_top_files": 25 }
        });
        let receipt = ripr_plus_receipt_from_repo_exposure_summary_json_with_source(
            &summary.to_string(),
            "deadbeef",
            "ripr plus --repo-exposure-summary summary.json",
            Some(Path::new("summary.json")),
        )?;
        assert_eq!(receipt["status"], "warn");
        assert_eq!(receipt["basis"], "canonical_actionable_gap");
        assert_eq!(receipt["unresolved"], 3);
        assert_eq!(receipt["suppressed"], 1);
        assert_eq!(receipt["source_format"], "repo-exposure-summary-json");
        assert_eq!(receipt["source_artifact"], "summary.json");
        assert_eq!(receipt["reason_counts"]["missing_discriminator"], 2);
        assert_eq!(receipt["top_files"][0]["file"], "src/pricing.rs");
        Ok(())
    }

    #[test]
    fn receipt_from_repo_exposure_summary_rejects_wrong_basis() -> Result<(), String> {
        let summary = json!({
            "format": "repo-exposure-summary-json",
            "basis": "seam_native",
            "metrics": { "unsuppressed_exposure_gaps": 0 },
            "top_files": []
        });
        match ripr_plus_receipt_from_repo_exposure_summary_json_with_source(
            &summary.to_string(),
            "deadbeef",
            "src",
            None,
        ) {
            Err(msg) if msg.contains("canonical_actionable_gap basis") => Ok(()),
            other => Err(format!("expected basis error, got {other:?}")),
        }
    }

    #[test]
    fn receipt_from_badge_composes_unresolved_and_suppressed() -> Result<(), String> {
        let badge = BadgeNativeAuditSnapshot {
            label: "ripr".to_string(),
            kind: "ripr".to_string(),
            scope: "repo".to_string(),
            basis: "gap_decision_ledger".to_string(),
            message: "2".to_string(),
            status: "warn".to_string(),
            color: "yellow".to_string(),
            counts: [
                ("unsuppressed_exposure_gaps".to_string(), 2),
                ("suppressed_exposure_gaps".to_string(), 1),
                ("suppressed_test_efficiency_findings".to_string(), 1),
                ("analyzed_seams".to_string(), 4),
            ]
            .into_iter()
            .collect(),
            reason_counts: BTreeMap::new(),
            warnings: Vec::new(),
        };
        let receipt =
            ripr_plus_receipt_from_badge(&badge, "deadbeef", Some(Path::new("ledger.json")))?;
        assert_eq!(receipt["unresolved"], 2);
        assert_eq!(receipt["suppressed"], 2);
        assert_eq!(receipt["raw_inventory"]["raw_seams"], 4);
        assert_eq!(receipt["basis"], "gap_decision_ledger");
        assert_eq!(receipt["top_files"].as_array().map(Vec::len), Some(0));
        assert!(
            receipt["source_command"]
                .as_str()
                .map(|s| s.contains("--gap-ledger"))
                .unwrap_or(false)
        );
        Ok(())
    }

    #[test]
    fn receipt_from_badge_rejects_seam_native_basis() -> Result<(), String> {
        let badge = BadgeNativeAuditSnapshot {
            label: "ripr".to_string(),
            kind: "ripr".to_string(),
            scope: "repo".to_string(),
            basis: "seam_native".to_string(),
            message: "0".to_string(),
            status: "pass".to_string(),
            color: "brightgreen".to_string(),
            counts: BTreeMap::new(),
            reason_counts: BTreeMap::new(),
            warnings: Vec::new(),
        };
        match ripr_plus_receipt_from_badge(&badge, "deadbeef", None) {
            Err(msg) if msg.contains("canonical actionable or gap decision ledger basis") => Ok(()),
            other => Err(format!("expected basis error, got {other:?}")),
        }
    }

    #[test]
    fn error_receipt_is_indeterminate() -> Result<(), String> {
        let receipt = error_ripr_plus_receipt("unknown", "the scan timed out after 1000 ms");
        assert_eq!(receipt["status"], "indeterminate");
        assert_eq!(receipt["machine_readable_cause"], "evaluation_timeout");
        assert_eq!(receipt["unresolved"], serde_json::Value::Null);
        assert_eq!(receipt["basis"], serde_json::Value::Null);
        assert!(
            receipt["warnings"][0]
                .as_str()
                .unwrap_or_default()
                .contains("timed out")
        );
        Ok(())
    }

    #[test]
    fn markdown_renders_basis_and_top_files() {
        let receipt = json!({
            "status": "warn",
            "basis": "canonical_actionable_gap",
            "source_format": "repo-exposure-summary-json",
            "unresolved": 3,
            "suppressed": 1,
            "head": "deadbeef",
            "reason_counts": { "missing_discriminator": 2 },
            "top_files": [
                {
                    "file": "src/pricing.rs",
                    "unsuppressed_exposure_gaps": 2,
                    "canonical_gap_records": 2,
                    "headline_eligible_seams": 4,
                    "raw_seams": 5
                }
            ]
        });
        let markdown = ripr_plus_receipt_markdown(&receipt);
        assert!(markdown.contains("# ripr+ Repo Receipt"));
        assert!(markdown.contains("## Basis"));
        assert!(markdown.contains("`warn`"));
        assert!(markdown.contains("## Reason Counts"));
        assert!(markdown.contains("## Top Files"));
        assert!(markdown.contains("src/pricing.rs"));
        assert!(!markdown.contains("## Evaluation Status"));
    }

    #[test]
    fn markdown_renders_indeterminate_evaluation_status() {
        let receipt = error_ripr_plus_receipt("unknown", "evaluation failed");
        let markdown = ripr_plus_receipt_markdown(&receipt);
        assert!(markdown.contains("## Evaluation Status"));
        assert!(markdown.contains("Indeterminate"));
        assert!(markdown.contains("N/A"));
    }
}
