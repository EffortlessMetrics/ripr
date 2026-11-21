//! Badge command cluster: the diff-scoped badge artifact generator
//! (`badge-artifacts`), the repo-scoped badge artifact generator
//! (`repo-badge-artifacts`), the badge basis audit (`badge-basis`), the ripr+
//! receipt (`ripr-plus`), the public badge endpoint sync/check commands
//! (`update-badge-endpoints`, `check-badge-endpoints`), and the
//! `check-badge-diff-policy` gate, plus their exclusive JSON/Shields/markdown
//! helpers.
//!
//! Extracted verbatim from `main.rs` as a behavior-preserving decomposition
//! slice of #2119. Items are `pub(crate)` and re-exported so existing call
//! sites (`dispatch.rs`, `main.rs` precommit/check-pr wiring, and `tests.rs`)
//! compile unchanged.

use crate::run::{TimedOutput, capture_output_with_timeout, run_output_optional, run_output_owned};
use crate::{
    FixKind, PolicyReportSpec, audit_bool, audit_get, audit_markdown_cell,
    badge_diff_policy_violations, badge_refresh_context, collect_pr_changes, finish_policy_report,
    git_value, markdown_cell, normalize_path, normalize_report_path, read_json_value, repo_root,
    repo_seam_inventory_command_args, reports_dir, ripr_debug_binary, test_efficiency_report_impl,
    write_report,
};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

pub(crate) fn badge_artifacts_impl() -> Result<(), String> {
    badge_artifacts_impl_with_runners(
        read_badge_artifact_diff,
        build_badge_artifact_binary,
        run_badge_artifact_command,
    )
}

pub(crate) fn badge_artifacts_impl_with_runners<DiffRunner, BuildRunner, Runner>(
    mut read_diff: DiffRunner,
    build_ripr: BuildRunner,
    run_artifact: Runner,
) -> Result<(), String>
where
    DiffRunner: FnMut() -> Result<String, String>,
    BuildRunner: FnMut(Duration) -> Result<TimedOutput, String>,
    Runner: FnMut(&Path, &str, &[String], Duration) -> Result<TimedOutput, String>,
{
    let badge_dir = Path::new("target").join("ripr");
    fs::create_dir_all(&badge_dir).map_err(|err| {
        format!(
            "failed to create badge directory {}: {err}",
            normalize_path(&badge_dir)
        )
    })?;

    let badge_input_path = badge_dir.join("badge-input.diff");
    let diff_output = read_diff()?;
    fs::write(&badge_input_path, &diff_output).map_err(|err| {
        format!(
            "failed to write badge input diff {}: {err}",
            normalize_path(&badge_input_path)
        )
    })?;

    let timeout = Duration::from_millis(badge_artifact_timeout_ms());
    let binary = ripr_debug_binary();

    write_badge_artifacts_after_build(&diff_output, &binary, timeout, build_ripr, run_artifact)
}

fn read_badge_artifact_diff() -> Result<String, String> {
    run_output_optional("git", &["diff", "origin/main...HEAD"])
}

pub(crate) fn write_badge_artifacts_after_build<BuildRunner, Runner>(
    diff_output: &str,
    binary: &Path,
    timeout: Duration,
    mut build_ripr: BuildRunner,
    run_artifact: Runner,
) -> Result<(), String>
where
    BuildRunner: FnMut(Duration) -> Result<TimedOutput, String>,
    Runner: FnMut(&Path, &str, &[String], Duration) -> Result<TimedOutput, String>,
{
    let build_output = build_ripr(timeout)?;
    match build_output.status {
        Some(status) if status.success() && !build_output.timed_out => {
            write_badge_artifacts_from_diff(diff_output, binary, timeout, run_artifact)
        }
        Some(_) | None => write_limited_badge_artifact_reports(
            "badge-build",
            &badge_artifact_build_command_label(),
            timeout,
            diff_output.len(),
            &build_output,
        ),
    }
}

pub(crate) fn write_badge_artifacts_from_diff<Runner>(
    diff_output: &str,
    binary: &Path,
    timeout: Duration,
    mut run_artifact: Runner,
) -> Result<(), String>
where
    Runner: FnMut(&Path, &str, &[String], Duration) -> Result<TimedOutput, String>,
{
    clear_badge_artifact_limitation();
    let mut ripr_native_json = String::new();
    let mut ripr_plus_native_json = String::new();

    for job in badge_artifact_jobs() {
        let args = badge_artifact_command_args(job.format);
        let command = badge_artifact_command_label(binary, &args);
        let output = run_artifact(binary, job.format, &args, timeout)?;
        if output.timed_out {
            return write_limited_badge_artifact_reports(
                job.format,
                &command,
                timeout,
                diff_output.len(),
                &output,
            );
        }
        let Some(status) = output.status else {
            return write_limited_badge_artifact_reports(
                job.format,
                &command,
                timeout,
                diff_output.len(),
                &output,
            );
        };
        if !status.success() {
            return write_limited_badge_artifact_reports(
                job.format,
                &command,
                timeout,
                diff_output.len(),
                &output,
            );
        }
        let output = output.stdout;
        write_report(job.output_file, &output)?;
        match badge_artifact_native_slot(job.format) {
            Some(BadgeNativeSlot::Ripr) => ripr_native_json = output,
            Some(BadgeNativeSlot::RiprPlus) => ripr_plus_native_json = output,
            None => {}
        }
    }

    let summary = badge_artifacts_summary_markdown(&ripr_native_json, &ripr_plus_native_json);
    write_report("ripr-badges.md", &summary)
}

const BADGE_ARTIFACT_TIMEOUT_ENV: &str = "RIPR_BADGE_ARTIFACT_TIMEOUT_MS";
const BADGE_ARTIFACT_DEFAULT_TIMEOUT_MS: u64 = 90_000;

#[derive(Debug, PartialEq, Eq)]
pub(crate) struct BadgeArtifactJob {
    pub(crate) format: &'static str,
    pub(crate) output_file: &'static str,
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum BadgeNativeSlot {
    Ripr,
    RiprPlus,
}

pub(crate) fn badge_artifact_jobs() -> Vec<BadgeArtifactJob> {
    vec![
        BadgeArtifactJob {
            format: "badge-json",
            output_file: "ripr-badge.json",
        },
        BadgeArtifactJob {
            format: "badge-shields",
            output_file: "ripr-badge-shields.json",
        },
        BadgeArtifactJob {
            format: "badge-plus-json",
            output_file: "ripr-plus-badge.json",
        },
        BadgeArtifactJob {
            format: "badge-plus-shields",
            output_file: "ripr-plus-badge-shields.json",
        },
    ]
}

pub(crate) fn badge_artifact_command_args(format: &str) -> Vec<String> {
    vec![
        "check".to_string(),
        "--root".to_string(),
        ".".to_string(),
        "--diff".to_string(),
        "target/ripr/badge-input.diff".to_string(),
        "--format".to_string(),
        format.to_string(),
    ]
}

fn badge_artifact_build_command_args() -> Vec<String> {
    vec!["build".to_string(), "-p".to_string(), "ripr".to_string()]
}

fn badge_artifact_timeout_ms() -> u64 {
    std::env::var(BADGE_ARTIFACT_TIMEOUT_ENV)
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .filter(|value| *value > 0)
        .unwrap_or(BADGE_ARTIFACT_DEFAULT_TIMEOUT_MS)
}

fn build_badge_artifact_binary(timeout: Duration) -> Result<TimedOutput, String> {
    let args = badge_artifact_build_command_args();
    capture_output_with_timeout("cargo", &args, &[], timeout, "badge artifact binary build")
}

fn badge_artifact_build_command_label() -> String {
    format!("cargo {}", badge_artifact_build_command_args().join(" "))
}

fn run_badge_artifact_command(
    binary: &Path,
    format: &str,
    args: &[String],
    timeout: Duration,
) -> Result<TimedOutput, String> {
    let binary_text = binary.display().to_string();
    capture_output_with_timeout(
        &binary_text,
        args,
        &[],
        timeout,
        &format!("badge artifact generation for {format}"),
    )
}

fn write_limited_badge_artifact_reports(
    format: &str,
    command: &str,
    timeout: Duration,
    diff_bytes: usize,
    output: &TimedOutput,
) -> Result<(), String> {
    clear_diff_badge_artifact_outputs();
    write_report(
        "badge-artifacts-limitation.json",
        &limited_badge_artifacts_json(format, command, timeout, diff_bytes, output)?,
    )?;
    write_report(
        "ripr-badges.md",
        &limited_badge_artifacts_markdown(format, command, timeout, diff_bytes, output),
    )
}

pub(crate) fn badge_artifact_command_label(binary: &Path, args: &[String]) -> String {
    normalize_report_path(&format!("{} {}", binary.display(), args.join(" ")))
}

fn clear_badge_artifact_limitation() {
    let _ = fs::remove_file(reports_dir().join("badge-artifacts-limitation.json"));
}

fn clear_diff_badge_artifact_outputs() {
    for job in badge_artifact_jobs() {
        let _ = fs::remove_file(reports_dir().join(job.output_file));
    }
    let _ = fs::remove_file(reports_dir().join("ripr-badges.md"));
}

pub(crate) fn limited_badge_artifacts_json(
    format: &str,
    command: &str,
    timeout: Duration,
    diff_bytes: usize,
    output: &TimedOutput,
) -> Result<String, String> {
    let limitation = badge_artifacts_limited_kind(format, output);
    let summary = badge_artifacts_limited_summary(limitation);
    let repair_route = badge_artifacts_limited_repair_route(limitation);
    let value = serde_json::json!({
        "schema_version": "0.1",
        "status": "warn",
        "phase": "badge_artifacts",
        "format": format,
        "limitation": {
            "category": limitation,
            "summary": summary,
            "repair_route": repair_route,
        },
        "input": {
            "diff_path": "target/ripr/badge-input.diff",
            "diff_bytes": diff_bytes,
        },
        "generation": {
            "command": command,
            "timeout_ms": timeout.as_millis(),
            "duration_ms": output.duration.as_millis(),
            "timed_out": output.timed_out,
            "exit_code": output.status.and_then(|status| status.code()),
            "stdout_bytes": output.stdout.len(),
            "stderr_bytes": output.stderr.len(),
        },
        "non_claims": [
            "no badge count claimed from this limited run",
            "not runtime mutation confirmation",
            "not merge approval",
            "not user test debt"
        ],
    });
    serde_json::to_string_pretty(&value)
        .map(|json| format!("{json}\n"))
        .map_err(|err| format!("failed to render badge artifact limitation JSON: {err}"))
}

fn badge_artifacts_limited_kind(format: &str, output: &TimedOutput) -> &'static str {
    if format == "badge-build" {
        return if output.timed_out {
            "badge_artifacts_build_timeout"
        } else {
            "badge_artifacts_build_incomplete"
        };
    }
    if output.timed_out {
        "badge_artifacts_diff_analysis_timeout"
    } else {
        "badge_artifacts_generation_incomplete"
    }
}

fn badge_artifacts_limited_summary(kind: &str) -> &'static str {
    match kind {
        "badge_artifacts_diff_analysis_timeout" => {
            "Badge artifact generation timed out before a complete diff-scoped badge was available."
        }
        "badge_artifacts_build_timeout" => {
            "Badge artifact generation timed out while building the ripr binary before diff analysis could run."
        }
        "badge_artifacts_build_incomplete" => {
            "Badge artifact generation could not build the ripr binary before diff analysis could run."
        }
        "badge_artifacts_generation_incomplete" => {
            "Badge artifact generation ended before producing a complete diff-scoped badge."
        }
        _ => "Badge artifact generation did not produce a complete diff-scoped badge.",
    }
}

fn badge_artifacts_limited_repair_route(kind: &str) -> &'static str {
    match kind {
        "badge_artifacts_diff_analysis_timeout" => {
            "inspect the diff-scoped badge runtime, narrow the diff input, or rerun with RIPR_BADGE_ARTIFACT_TIMEOUT_MS on a machine that can complete the analysis"
        }
        "badge_artifacts_build_timeout" => {
            "inspect the cargo build output or rerun with RIPR_BADGE_ARTIFACT_TIMEOUT_MS on a machine that can build ripr before claiming badge counts from this run"
        }
        "badge_artifacts_build_incomplete" => {
            "inspect the cargo build exit status and stdout/stderr before claiming badge counts from this run"
        }
        "badge_artifacts_generation_incomplete" => {
            "inspect the badge artifact command exit status, stdout/stderr, and diff input before claiming badge counts from this run"
        }
        _ => "inspect badge artifact generation and rerun with bounded diagnostics",
    }
}

pub(crate) fn limited_badge_artifacts_markdown(
    format: &str,
    command: &str,
    timeout: Duration,
    diff_bytes: usize,
    output: &TimedOutput,
) -> String {
    let limitation = badge_artifacts_limited_kind(format, output);
    let summary = badge_artifacts_limited_summary(limitation);
    let repair_route = badge_artifacts_limited_repair_route(limitation);
    let exit = output
        .status
        .and_then(|status| status.code())
        .map(|code| code.to_string())
        .unwrap_or_else(|| "n/a".to_string());
    let mut out = String::new();
    out.push_str("# ripr badges\n\n");
    out.push_str("Status: warn\n\n");
    out.push_str(summary);
    out.push_str(" No badge count is claimed from this limited artifact.\n\n");
    out.push_str("## Run Limitation\n\n");
    out.push_str("| Field | Value |\n");
    out.push_str("| --- | --- |\n");
    out.push_str(&format!("| Category | `{limitation}` |\n"));
    out.push_str(&format!("| Format | `{}` |\n", audit_markdown_cell(format)));
    out.push_str(&format!("| Diff bytes | {diff_bytes} |\n"));
    out.push_str(&format!("| Timeout | {} ms |\n", timeout.as_millis()));
    out.push_str(&format!(
        "| Duration | {} ms |\n",
        output.duration.as_millis()
    ));
    out.push_str(&format!("| Exit code | {exit} |\n"));
    out.push_str(&format!(
        "| Command | `{}` |\n",
        audit_markdown_cell(command)
    ));
    out.push_str(&format!("| Repair route | {repair_route} |\n\n"));
    if !output.stderr.trim().is_empty() {
        out.push_str("## Stderr Tail\n\n```text\n");
        for line in output
            .stderr
            .lines()
            .rev()
            .take(8)
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
        {
            out.push_str(line);
            out.push('\n');
        }
        out.push_str("```\n");
    }
    out
}

pub(crate) fn badge_artifact_native_slot(format: &str) -> Option<BadgeNativeSlot> {
    match format {
        "badge-json" | "repo-badge-json" => Some(BadgeNativeSlot::Ripr),
        "badge-plus-json" | "repo-badge-plus-json" => Some(BadgeNativeSlot::RiprPlus),
        _ => None,
    }
}

pub(crate) fn badge_artifacts_summary_markdown(
    ripr_native_json: &str,
    ripr_plus_native_json: &str,
) -> String {
    let mut markdown = String::from("# ripr badges\n\n");
    append_badge_section(&mut markdown, "ripr", ripr_native_json);
    append_badge_section(&mut markdown, "ripr+", ripr_plus_native_json);
    markdown.push_str("## Artifacts\n\n");
    markdown.push_str("- `ripr-badge.json` — native ripr badge\n");
    markdown.push_str("- `ripr-badge-shields.json` — Shields projection of ripr badge\n");
    markdown.push_str("- `ripr-plus-badge.json` — native ripr+ badge\n");
    markdown.push_str("- `ripr-plus-badge-shields.json` — Shields projection of ripr+ badge\n");
    markdown
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct RepoBadgeArtifactOptions {
    pub(crate) gap_ledger: Option<PathBuf>,
    pub(crate) repo_exposure_summary: Option<PathBuf>,
    pub(crate) include_seam_classes: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct BadgeEndpointSnapshot {
    pub(crate) path: String,
    pub(crate) label: String,
    pub(crate) message: String,
    pub(crate) color: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct BadgeNativeAuditSnapshot {
    pub(crate) label: String,
    pub(crate) kind: String,
    pub(crate) scope: String,
    pub(crate) basis: String,
    pub(crate) message: String,
    pub(crate) status: String,
    pub(crate) color: String,
    pub(crate) counts: BTreeMap<String, usize>,
    pub(crate) reason_counts: BTreeMap<String, usize>,
    pub(crate) warnings: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct BadgeCountBreakdown {
    pub(crate) status: String,
    pub(crate) source: String,
    pub(crate) counts: BTreeMap<String, usize>,
    pub(crate) note: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct BadgeCanonicalProjection {
    pub(crate) status: String,
    pub(crate) source: String,
    pub(crate) ripr_count: Option<usize>,
    pub(crate) ripr_plus_count: Option<usize>,
    pub(crate) detail: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct BadgeBasisSignal {
    pub(crate) status: String,
    pub(crate) source: String,
    pub(crate) count: Option<usize>,
    pub(crate) detail: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct BadgeBasisReport {
    pub(crate) status: String,
    pub(crate) current_public_endpoints: Vec<BadgeEndpointSnapshot>,
    pub(crate) current_repo_badges: Vec<BadgeNativeAuditSnapshot>,
    pub(crate) seam_native_counts: BadgeCountBreakdown,
    pub(crate) test_efficiency_counts: BadgeCountBreakdown,
    pub(crate) canonical_actionable_gap: BadgeCanonicalProjection,
    pub(crate) raw_alignment_signals: BadgeBasisSignal,
    pub(crate) canonical_evidence_items: BadgeBasisSignal,
    pub(crate) static_limitations: BadgeBasisSignal,
    pub(crate) suppressed_or_intentional_items: BadgeBasisSignal,
    pub(crate) no_action_items: BadgeBasisSignal,
    pub(crate) recommended_public_projection: String,
    pub(crate) warnings: Vec<String>,
}

pub(crate) fn repo_badge_artifacts_impl(args: &[String]) -> Result<(), String> {
    let options = parse_repo_badge_artifact_options(args, "repo-badge-artifacts")?;
    run_with_repo_root_cwd(|| write_repo_badge_artifacts(&options))
}

pub(crate) fn badge_basis_impl(args: &[String]) -> Result<(), String> {
    let options = parse_repo_badge_artifact_options(args, "badge-basis")?;
    run_with_repo_root_cwd(|| {
        let report = build_badge_basis_report(&options)?;
        write_report("badge-basis.json", &badge_basis_report_json(&report)?)?;
        write_report("badge-basis.md", &badge_basis_report_markdown(&report))
    })
}

fn build_badge_basis_report(
    options: &RepoBadgeArtifactOptions,
) -> Result<BadgeBasisReport, String> {
    let current_public_endpoints = badge_basis_endpoint_snapshots()?;
    test_efficiency_report_impl()?;

    let ripr_native_json =
        run_repo_badge_artifact_job("repo-badge-json", options.gap_ledger.as_deref())?;
    let ripr_snapshot = badge_native_audit_snapshot(&ripr_native_json)?;
    let test_efficiency_value = badge_basis_test_efficiency_value();
    let test_efficiency_counts = badge_basis_test_efficiency_counts(&test_efficiency_value);
    let ripr_plus_snapshot = if badge_basis_needs_repo_badge_plus_job(options) {
        let ripr_plus_native_json =
            run_repo_badge_artifact_job("repo-badge-plus-json", options.gap_ledger.as_deref())?;
        badge_native_audit_snapshot(&ripr_plus_native_json)?
    } else {
        badge_basis_derived_ripr_plus_snapshot(&ripr_snapshot, test_efficiency_value.as_ref().ok())
    };
    let current_repo_badges = vec![ripr_snapshot.clone(), ripr_plus_snapshot.clone()];

    let seam_native_counts =
        badge_basis_seam_native_counts(options.include_seam_classes, &ripr_snapshot);
    let canonical_actionable_gap =
        badge_basis_canonical_projection(options, &ripr_snapshot, &ripr_plus_snapshot);
    let static_limitations = badge_basis_static_limitations(&seam_native_counts);
    let suppressed_or_intentional_items =
        badge_basis_suppressed_or_intentional_items(&ripr_snapshot, &ripr_plus_snapshot);

    let mut warnings = Vec::new();
    let current_basis = current_repo_badges
        .iter()
        .map(|badge| badge.basis.as_str())
        .collect::<BTreeSet<_>>();
    if current_basis.contains("seam_native") && canonical_actionable_gap.ripr_count.is_none() {
        warnings.push(
            "Current public endpoint values are decomposed as seam-native inventory; canonical actionable public projection is not implemented in this PR."
                .to_string(),
        );
    }
    if seam_native_counts.status == "warn" {
        warnings.push(seam_native_counts.note.clone());
    }

    let status = if warnings.is_empty() { "pass" } else { "warn" }.to_string();

    Ok(BadgeBasisReport {
        status,
        current_public_endpoints,
        current_repo_badges,
        seam_native_counts,
        test_efficiency_counts,
        canonical_actionable_gap,
        raw_alignment_signals: BadgeBasisSignal {
            status: "not_in_current_badge_generator".to_string(),
            source: "finding-alignment dogfood receipts".to_string(),
            count: None,
            detail:
                "Raw alignment signals remain supporting evidence; they are not counted by the current public endpoint generator."
                    .to_string(),
        },
        canonical_evidence_items: BadgeBasisSignal {
            status: "not_in_current_badge_generator".to_string(),
            source: "repo exposure / gap ledger artifacts".to_string(),
            count: None,
            detail:
                "Canonical evidence identity is available to downstream reports, but the current public endpoint generator does not count it as the headline unit."
                    .to_string(),
        },
        static_limitations,
        suppressed_or_intentional_items,
        no_action_items: BadgeBasisSignal {
            status: "requires_gap_decision_ledger".to_string(),
            source: options
                .gap_ledger
                .as_ref()
                .map(|path| normalize_path(path))
                .unwrap_or_else(|| "no gap decision ledger supplied".to_string()),
            count: None,
            detail:
                "No-action, already-observed, suppressed, and intentional gap states require explicit gap records; seam-native inventory cannot infer them safely."
                    .to_string(),
        },
        recommended_public_projection: "canonical_actionable_gap".to_string(),
        warnings,
    })
}

pub(crate) fn ripr_plus_impl(args: &[String]) -> Result<(), String> {
    let options = parse_repo_badge_artifact_options(args, "ripr-plus")?;
    run_with_repo_root_cwd(|| {
        let head = git_value(&["rev-parse", "HEAD"]);
        let receipt = match ripr_plus_receipt_from_options(&options, &head) {
            Ok(r) => r,
            Err(err) => {
                write_error_ripr_plus_receipt(&head, &err)?;
                return Ok(());
            }
        };
        let json = serde_json::to_string_pretty(&receipt)
            .map_err(|err| format!("failed to serialize ripr-plus receipt: {err}"))?;
        write_report("ripr-plus.json", &format!("{json}\n"))?;
        write_report("ripr-plus.md", &ripr_plus_receipt_markdown(&receipt))
    })
}

fn write_error_ripr_plus_receipt(head: &str, err: &str) -> Result<(), String> {
    let receipt = error_ripr_plus_receipt(head, err);
    let json = serde_json::to_string_pretty(&receipt)
        .map_err(|e| format!("failed to serialize error ripr-plus receipt: {e}"))?;
    write_report("ripr-plus.json", &format!("{json}\n"))?;
    write_report("ripr-plus.md", &ripr_plus_receipt_markdown(&receipt))
}

pub(crate) fn error_ripr_plus_receipt(head: &str, err: &str) -> Value {
    let machine_readable_cause = if err.contains("timed out") {
        "evaluation_timeout"
    } else {
        "evaluation_error"
    };
    let first_warning = err.lines().next().unwrap_or(err).trim().to_string();
    serde_json::json!({
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

pub(crate) fn ripr_plus_receipt_from_options(
    options: &RepoBadgeArtifactOptions,
    head: &str,
) -> Result<Value, String> {
    if let Some(summary_path) = options.repo_exposure_summary.as_deref() {
        let repo_summary_json = read_repo_exposure_summary_artifact(summary_path)?;
        return ripr_plus_receipt_from_repo_exposure_summary_json_with_source(
            &repo_summary_json,
            head,
            &format!(
                "cargo xtask ripr-plus --repo-exposure-summary {}",
                normalize_path(summary_path)
            ),
            Some(summary_path),
        );
    }

    if options.gap_ledger.is_some() {
        let repo_badge_json =
            run_repo_badge_artifact_job("repo-badge-json", options.gap_ledger.as_deref())?;
        return ripr_plus_receipt_from_repo_badge_json(
            &repo_badge_json,
            head,
            options.gap_ledger.as_deref(),
        );
    }

    let repo_summary_json = run_repo_exposure_summary_job()?;
    ripr_plus_receipt_from_repo_exposure_summary_json(&repo_summary_json, head)
}

pub(crate) fn read_repo_exposure_summary_artifact(path: &Path) -> Result<String, String> {
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
            "repo exposure summary artifact {} is not downstream consumable (basis {:?}, run_status {:?}); rerun `cargo xtask repo-exposure-summary-report` with enough time to produce canonical_actionable_gap data, or pass --gap-ledger to use an existing gap decision ledger",
            normalize_path(path),
            value.get("basis").and_then(Value::as_str),
            value.get("run_status").and_then(Value::as_str)
        ));
    }
    Ok(json)
}

fn run_repo_exposure_summary_job() -> Result<String, String> {
    let timeout = Duration::from_millis(repo_badge_artifact_timeout_ms());
    if let Ok(ripr_bin) = std::env::var("RIPR_BIN") {
        let repo_root = repo_root()?;
        let args = vec![
            "check".to_string(),
            "--root".to_string(),
            normalize_path(&repo_root),
            "--format".to_string(),
            "repo-exposure-summary-json".to_string(),
        ];
        return run_repo_exposure_summary_command(&ripr_bin, &args, timeout);
    }

    let args = repo_seam_inventory_command_args("repo-exposure-summary-json");
    run_repo_exposure_summary_command("cargo", &args, timeout)
}

fn run_repo_exposure_summary_command(
    program: &str,
    args: &[String],
    timeout: Duration,
) -> Result<String, String> {
    let output = capture_output_with_timeout(
        program,
        args,
        &[],
        timeout,
        "repo exposure summary generation for ripr-plus",
    )?;
    let command = format!("{} {}", program, args.join(" "));
    if output.timed_out {
        return Err(format!(
            "{command} timed out after {} ms while generating repo-exposure-summary-json for ripr-plus; no RIPR+ receipt is claimed. Rerun with {REPO_BADGE_ARTIFACT_TIMEOUT_ENV}=<milliseconds> only in an explicit large-repo refresh, or pass --gap-ledger to use an existing gap decision ledger.",
            timeout.as_millis()
        ));
    }
    let Some(status) = output.status else {
        return Err(format!(
            "{command} finished without an exit status while generating repo-exposure-summary-json for ripr-plus"
        ));
    };
    if !status.success() {
        return Err(format!(
            "{command} failed with {status}\nstdout:\n{}\nstderr:\n{}",
            output.stdout.trim(),
            output.stderr.trim()
        ));
    }
    Ok(output.stdout)
}

pub(crate) fn ripr_plus_receipt_from_repo_badge_json(
    json: &str,
    head: &str,
    gap_ledger: Option<&Path>,
) -> Result<Value, String> {
    let badge = badge_native_audit_snapshot(json)?;
    ripr_plus_receipt_from_badge(&badge, head, gap_ledger)
}

pub(crate) fn ripr_plus_receipt_from_badge(
    badge: &BadgeNativeAuditSnapshot,
    head: &str,
    gap_ledger: Option<&Path>,
) -> Result<Value, String> {
    if !ripr_plus_accepts_badge_basis(&badge.basis) {
        return Err(format!(
            "ripr-plus requires repo-badge-json with canonical actionable or gap decision ledger basis, got {:?}",
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

    Ok(serde_json::json!({
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

pub(crate) fn ripr_plus_receipt_from_repo_exposure_summary_json(
    json: &str,
    head: &str,
) -> Result<Value, String> {
    ripr_plus_receipt_from_repo_exposure_summary_json_with_source(
        json,
        head,
        "ripr check --root . --format repo-exposure-summary-json",
        None,
    )
}

pub(crate) fn ripr_plus_receipt_from_repo_exposure_summary_json_with_source(
    json: &str,
    head: &str,
    source_command: &str,
    source_artifact: Option<&Path>,
) -> Result<Value, String> {
    let summary: Value = serde_json::from_str(json)
        .map_err(|err| format!("failed to parse repo-exposure-summary-json: {err}"))?;
    let format = json_string_field_value(&summary, "format");
    if format != "repo-exposure-summary-json" {
        return Err(format!(
            "ripr-plus requires repo-exposure-summary-json, got {format:?}"
        ));
    }
    let basis = json_string_field_value(&summary, "basis");
    if basis != "canonical_actionable_gap" {
        return Err(format!(
            "ripr-plus requires repo-exposure-summary-json with canonical_actionable_gap basis, got {basis:?}"
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

    Ok(serde_json::json!({
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

pub(crate) fn ripr_plus_receipt_markdown(receipt: &Value) -> String {
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

fn write_repo_badge_artifacts(options: &RepoBadgeArtifactOptions) -> Result<(), String> {
    test_efficiency_report_impl()?;

    let badge_dir = Path::new("target").join("ripr");
    fs::create_dir_all(&badge_dir).map_err(|err| {
        format!(
            "failed to create badge directory {}: {err}",
            normalize_path(&badge_dir)
        )
    })?;

    // Repo scope is intentionally diff-free: the badge formats render from
    // classified repo seams or from an explicit gap decision ledger rather
    // than `git diff origin/main...HEAD`. Capturing a diff would silently make
    // the artifact dependent on branch state.
    let mut ripr_native_json = String::new();
    let mut ripr_plus_native_json = String::new();

    for job in repo_badge_artifact_jobs() {
        let output = run_repo_badge_artifact_job(job.format, options.gap_ledger.as_deref())?;
        write_report(job.output_file, &output)?;
        match badge_artifact_native_slot(job.format) {
            Some(BadgeNativeSlot::Ripr) => ripr_native_json = output,
            Some(BadgeNativeSlot::RiprPlus) => ripr_plus_native_json = output,
            None => {}
        }
    }

    let summary = repo_badge_artifacts_summary_markdown(
        &ripr_native_json,
        &ripr_plus_native_json,
        options.gap_ledger.as_deref(),
    );
    write_report("repo-ripr-badges.md", &summary)
}

pub(crate) fn parse_repo_badge_artifact_options(
    args: &[String],
    command_name: &str,
) -> Result<RepoBadgeArtifactOptions, String> {
    let mut options = RepoBadgeArtifactOptions::default();
    let mut index = 0usize;
    while index < args.len() {
        match args[index].as_str() {
            "--check" => {}
            "--include-seam-classes" if command_name == "badge-basis" => {
                options.include_seam_classes = true;
            }
            "--include-seam-classes" => {
                return Err(format!(
                    "{command_name} does not support --include-seam-classes"
                ));
            }
            "--gap-ledger" => {
                index += 1;
                let Some(value) = args.get(index) else {
                    return Err(format!("{command_name} --gap-ledger requires a path"));
                };
                if value.trim().is_empty() {
                    return Err(format!(
                        "{command_name} --gap-ledger requires a non-empty path"
                    ));
                }
                options.gap_ledger = Some(PathBuf::from(value));
            }
            "--repo-exposure-summary" if command_name == "ripr-plus" => {
                index += 1;
                let Some(value) = args.get(index) else {
                    return Err(format!(
                        "{command_name} --repo-exposure-summary requires a path"
                    ));
                };
                if value.trim().is_empty() {
                    return Err(format!(
                        "{command_name} --repo-exposure-summary requires a non-empty path"
                    ));
                }
                options.repo_exposure_summary = Some(PathBuf::from(value));
            }
            "--repo-exposure-summary" => {
                return Err(format!(
                    "{command_name} does not support --repo-exposure-summary"
                ));
            }
            other => return Err(format!("unknown {command_name} argument {other:?}")),
        }
        index += 1;
    }
    if options.gap_ledger.is_some() && options.repo_exposure_summary.is_some() {
        return Err(format!(
            "{command_name} accepts either --gap-ledger or --repo-exposure-summary, not both"
        ));
    }
    Ok(options)
}

fn badge_basis_endpoint_snapshots() -> Result<Vec<BadgeEndpointSnapshot>, String> {
    let mut snapshots = Vec::new();
    for (path, _) in BADGE_ENDPOINT_FILES {
        let value = read_json_value(Path::new(path))?;
        snapshots.push(BadgeEndpointSnapshot {
            path: (*path).to_string(),
            label: json_string_field_value(&value, "label"),
            message: json_string_field_value(&value, "message"),
            color: json_string_field_value(&value, "color"),
        });
    }
    Ok(snapshots)
}

pub(crate) fn badge_native_audit_snapshot(json: &str) -> Result<BadgeNativeAuditSnapshot, String> {
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
        .ok_or_else(|| format!("JSON is missing numeric field {}", path.join(".")))
}

pub(crate) fn badge_basis_needs_repo_badge_plus_job(options: &RepoBadgeArtifactOptions) -> bool {
    options.gap_ledger.is_some()
}

pub(crate) fn badge_basis_derived_ripr_plus_snapshot(
    ripr: &BadgeNativeAuditSnapshot,
    test_efficiency: Option<&Value>,
) -> BadgeNativeAuditSnapshot {
    let mut counts = ripr.counts.clone();
    counts.insert("unsuppressed_test_efficiency_findings".to_string(), 0);
    counts.insert("intentional_test_efficiency_findings".to_string(), 0);
    counts.insert("suppressed_test_efficiency_findings".to_string(), 0);
    counts.insert("unknowns_test_efficiency".to_string(), 0);
    if let Some(tests_scanned) = test_efficiency
        .and_then(|value| value.get("metrics"))
        .and_then(|metrics| metrics.get("tests_scanned"))
        .and_then(Value::as_u64)
        .and_then(|count| usize::try_from(count).ok())
    {
        counts.insert("analyzed_tests".to_string(), tests_scanned);
    }

    BadgeNativeAuditSnapshot {
        label: "ripr+".to_string(),
        kind: "ripr_plus".to_string(),
        scope: ripr.scope.clone(),
        basis: ripr.basis.clone(),
        message: ripr.message.clone(),
        status: ripr.status.clone(),
        color: ripr.color.clone(),
        counts,
        reason_counts: test_efficiency
            .map(|value| json_object_usize_map(value, "reason_counts"))
            .unwrap_or_default(),
        warnings: ripr.warnings.clone(),
    }
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

pub(crate) fn badge_basis_seam_native_counts(
    include_seam_classes: bool,
    ripr: &BadgeNativeAuditSnapshot,
) -> BadgeCountBreakdown {
    if !include_seam_classes {
        if ripr.basis != "seam_native" {
            return BadgeCountBreakdown {
                status: "not_collected".to_string(),
                source: "cargo xtask badge-basis --include-seam-classes".to_string(),
                counts: BTreeMap::new(),
                note: "Public repo badge counts now use canonical_actionable_gap; rerun with `cargo xtask badge-basis --include-seam-classes` when seam-native inventory detail is needed.".to_string(),
            };
        }
        let mut counts = BTreeMap::new();
        for key in [
            "analyzed_seams",
            "unsuppressed_exposure_gaps",
            "suppressed_exposure_gaps",
            "unknowns",
        ] {
            if let Some(value) = ripr.counts.get(key) {
                counts.insert(key.to_string(), *value);
            }
        }
        return BadgeCountBreakdown {
            status: "partial".to_string(),
            source: "native repo badge counts".to_string(),
            counts,
            note: "Compact badge counts are available. Per-class seam-native breakdown is intentionally skipped by default because repo-exposure can be expensive; rerun with `cargo xtask badge-basis --include-seam-classes` when that detail is needed.".to_string(),
        };
    }

    match run_repo_exposure_markdown_for_badge_basis()
        .and_then(|markdown| parse_repo_exposure_summary_counts(&markdown))
    {
        Ok(counts) => BadgeCountBreakdown {
            status: "pass".to_string(),
            source: "ripr check --root . --format repo-exposure-md".to_string(),
            counts,
            note: "Repo exposure summary class counts are available.".to_string(),
        },
        Err(err) => BadgeCountBreakdown {
            status: "warn".to_string(),
            source: "ripr check --root . --format repo-exposure-md".to_string(),
            counts: BTreeMap::new(),
            note: format!("Could not collect seam-native class counts: {err}"),
        },
    }
}

fn run_repo_exposure_markdown_for_badge_basis() -> Result<String, String> {
    if let Ok(ripr_bin) = std::env::var("RIPR_BIN") {
        let repo_root = repo_root()?;
        let args = vec![
            "check".to_string(),
            "--root".to_string(),
            normalize_path(&repo_root),
            "--format".to_string(),
            "repo-exposure-md".to_string(),
        ];
        return run_output_owned(&ripr_bin, &args);
    }
    let args = repo_seam_inventory_command_args("repo-exposure-md");
    run_output_owned("cargo", &args)
}

pub(crate) fn parse_repo_exposure_summary_counts(
    markdown: &str,
) -> Result<BTreeMap<String, usize>, String> {
    let mut counts = BTreeMap::new();
    for line in markdown.lines() {
        let trimmed = line.trim();
        if !trimmed.starts_with('|') || !trimmed.ends_with('|') {
            continue;
        }
        let cells = trimmed.split('|').map(str::trim).collect::<Vec<_>>();
        if cells.len() != 4 {
            continue;
        }
        let key = cells[1];
        let value = cells[2];
        if key.is_empty()
            || key == "Class"
            || key.chars().all(|ch| ch == '-')
            || value.chars().all(|ch| ch == '-' || ch == ':')
        {
            continue;
        }
        if let Ok(count) = value.parse::<usize>() {
            counts.insert(key.to_string(), count);
        }
    }
    if counts.is_empty() {
        return Err("repo exposure Markdown summary did not contain count rows".to_string());
    }
    Ok(counts)
}

fn badge_basis_test_efficiency_value() -> Result<Value, String> {
    let path = Path::new("target/ripr/reports/test-efficiency.json");
    read_json_value(path)
}

fn badge_basis_test_efficiency_counts(
    test_efficiency: &Result<Value, String>,
) -> BadgeCountBreakdown {
    let path = Path::new("target/ripr/reports/test-efficiency.json");
    match test_efficiency {
        Ok(value) => BadgeCountBreakdown {
            status: "pass".to_string(),
            source: normalize_path(path),
            counts: json_object_usize_map(value, "counts"),
            note: "Test-efficiency class counts are available.".to_string(),
        },
        Err(err) => BadgeCountBreakdown {
            status: "warn".to_string(),
            source: normalize_path(path),
            counts: BTreeMap::new(),
            note: format!("Could not collect test-efficiency counts: {err}"),
        },
    }
}

pub(crate) fn badge_basis_canonical_projection(
    options: &RepoBadgeArtifactOptions,
    ripr: &BadgeNativeAuditSnapshot,
    ripr_plus: &BadgeNativeAuditSnapshot,
) -> BadgeCanonicalProjection {
    if ripr.basis == "canonical_actionable_gap" && ripr_plus.basis == "canonical_actionable_gap" {
        return BadgeCanonicalProjection {
            status: "available".to_string(),
            source: "repo-badge-artifacts".to_string(),
            ripr_count: ripr.message.parse::<usize>().ok(),
            ripr_plus_count: ripr_plus.message.parse::<usize>().ok(),
            detail:
                "The current repo badge generator uses canonical_actionable_gap to count unresolved actionable static repair gaps."
                    .to_string(),
        };
    }

    if options.gap_ledger.is_some()
        && ripr.basis == "gap_decision_ledger"
        && ripr_plus.basis == "gap_decision_ledger"
    {
        return BadgeCanonicalProjection {
            status: "available_via_gap_decision_ledger".to_string(),
            source: options
                .gap_ledger
                .as_ref()
                .map(|path| normalize_path(path))
                .unwrap_or_else(|| "gap decision ledger".to_string()),
            ripr_count: ripr.message.parse::<usize>().ok(),
            ripr_plus_count: ripr_plus.message.parse::<usize>().ok(),
            detail:
                "An explicit gap decision ledger supplied projection targets for the current audit run."
                    .to_string(),
        };
    }

    BadgeCanonicalProjection {
        status: "not_available".to_string(),
        source: "canonical_actionable_gap generator path".to_string(),
        ripr_count: None,
        ripr_plus_count: None,
        detail:
            "This audit PR decomposes the current seam-native public basis; a later PR will implement canonical actionable public badge generation."
                .to_string(),
    }
}

fn badge_basis_static_limitations(seam_counts: &BadgeCountBreakdown) -> BadgeBasisSignal {
    let limit_keys = [
        "activation_unknown",
        "propagation_unknown",
        "observation_unknown",
        "discrimination_unknown",
        "opaque",
    ];
    let count = if seam_counts.counts.is_empty() {
        None
    } else if let Some(unknowns) = seam_counts.counts.get("unknowns") {
        Some(*unknowns)
    } else {
        Some(
            limit_keys
                .iter()
                .map(|key| seam_counts.counts.get(*key).copied().unwrap_or(0))
                .sum(),
        )
    };
    BadgeBasisSignal {
        status: if count.is_some() {
            "available"
        } else {
            "not_available"
        }
        .to_string(),
        source: seam_counts.source.clone(),
        count,
        detail:
            "Static-limit pressure is supporting inventory evidence; it should not become a public repair counter without actionability."
                .to_string(),
    }
}

fn badge_basis_suppressed_or_intentional_items(
    ripr: &BadgeNativeAuditSnapshot,
    ripr_plus: &BadgeNativeAuditSnapshot,
) -> BadgeBasisSignal {
    let count = ripr
        .counts
        .get("suppressed_exposure_gaps")
        .copied()
        .unwrap_or(0)
        + ripr_plus
            .counts
            .get("suppressed_test_efficiency_findings")
            .copied()
            .unwrap_or(0)
        + ripr_plus
            .counts
            .get("intentional_test_efficiency_findings")
            .copied()
            .unwrap_or(0);
    BadgeBasisSignal {
        status: "available_from_badge_counts".to_string(),
        source: "native repo badge counts".to_string(),
        count: Some(count),
        detail:
            "Suppressed and intentional items stay out of the public repair counter; they remain visible in detailed reports."
                .to_string(),
    }
}

pub(crate) fn badge_basis_report_json(report: &BadgeBasisReport) -> Result<String, String> {
    let endpoints = report
        .current_public_endpoints
        .iter()
        .map(|endpoint| {
            serde_json::json!({
                "path": &endpoint.path,
                "label": &endpoint.label,
                "message": &endpoint.message,
                "color": &endpoint.color,
            })
        })
        .collect::<Vec<_>>();
    let repo_badges = report
        .current_repo_badges
        .iter()
        .map(|badge| {
            serde_json::json!({
                "label": &badge.label,
                "kind": &badge.kind,
                "scope": &badge.scope,
                "basis": &badge.basis,
                "message": &badge.message,
                "status": &badge.status,
                "color": &badge.color,
                "counts": &badge.counts,
                "reason_counts": &badge.reason_counts,
                "warnings": &badge.warnings,
            })
        })
        .collect::<Vec<_>>();
    let value = serde_json::json!({
        "schema_version": "0.1",
        "status": &report.status,
        "mode": "advisory",
        "current_public_endpoints": endpoints,
        "current_repo_badges": repo_badges,
        "seam_native": {
            "status": &report.seam_native_counts.status,
            "source": &report.seam_native_counts.source,
            "counts_by_class": &report.seam_native_counts.counts,
            "note": &report.seam_native_counts.note,
        },
        "test_efficiency": {
            "status": &report.test_efficiency_counts.status,
            "source": &report.test_efficiency_counts.source,
            "counts_by_class": &report.test_efficiency_counts.counts,
            "note": &report.test_efficiency_counts.note,
        },
        "canonical_actionable_gap": {
            "status": &report.canonical_actionable_gap.status,
            "source": &report.canonical_actionable_gap.source,
            "ripr_count": report.canonical_actionable_gap.ripr_count,
            "ripr_plus_count": report.canonical_actionable_gap.ripr_plus_count,
            "detail": &report.canonical_actionable_gap.detail,
        },
        "supporting_signals": {
            "raw_alignment_signals": badge_basis_signal_json(&report.raw_alignment_signals),
            "canonical_evidence_items": badge_basis_signal_json(&report.canonical_evidence_items),
            "static_limitations": badge_basis_signal_json(&report.static_limitations),
            "suppressed_or_intentional_items": badge_basis_signal_json(&report.suppressed_or_intentional_items),
            "no_action_items": badge_basis_signal_json(&report.no_action_items),
        },
        "recommended_public_projection": {
            "basis": &report.recommended_public_projection,
            "rule": "README/store badges should count unresolved actionable static repair gaps using canonical_actionable_gap; ripr+ adds only items projected into the same repair, verify, and receipt model; seam-native inventory stays supporting/internal.",
        },
        "warnings": &report.warnings,
        "non_claims": [
            "not coverage",
            "not runtime mutation confirmation",
            "not merge approval",
            "not a complete seam inventory"
        ],
    });
    serde_json::to_string_pretty(&value)
        .map_err(|err| format!("failed to serialize badge-basis report: {err}"))
}

fn badge_basis_signal_json(signal: &BadgeBasisSignal) -> Value {
    serde_json::json!({
        "status": &signal.status,
        "source": &signal.source,
        "count": signal.count,
        "detail": &signal.detail,
    })
}

pub(crate) fn badge_basis_report_markdown(report: &BadgeBasisReport) -> String {
    let mut body = format!(
        "# ripr Public Badge Basis Audit\n\nStatus: `{}`\nMode: advisory\n\n",
        report.status
    );
    body.push_str(
        "This report decomposes the committed public badge endpoint counts before \
changing badge semantics. It does not edit `badges/*.json`.\n\n",
    );
    body.push_str("## Public Headline Meaning\n\n");
    body.push_str("- `ripr` headline count: unresolved actionable static repair gaps.\n");
    body.push_str("- Public basis: `canonical_actionable_gap`.\n");
    body.push_str("- `ripr+` only adds items that project into the same ");
    body.push_str("repair, verify, and receipt model.\n");
    body.push_str("- Seam-native inventory and raw findings are supporting/internal ");
    body.push_str("diagnostics, not the public headline counter.\n\n");

    body.push_str("## Current Public Endpoints\n\n");
    body.push_str("| Path | Label | Message | Color |\n");
    body.push_str("| --- | --- | ---: | --- |\n");
    for endpoint in &report.current_public_endpoints {
        body.push_str(&format!(
            "| `{}` | `{}` | {} | `{}` |\n",
            markdown_cell(&endpoint.path),
            markdown_cell(&endpoint.label),
            markdown_cell(&endpoint.message),
            markdown_cell(&endpoint.color)
        ));
    }

    body.push_str("\n## Current Repo Badge Basis\n\n");
    body.push_str("| Badge | Scope | Basis | Message | Status | Color |\n");
    body.push_str("| --- | --- | --- | ---: | --- | --- |\n");
    for badge in &report.current_repo_badges {
        body.push_str(&format!(
            "| `{}` | `{}` | `{}` | {} | `{}` | `{}` |\n",
            markdown_cell(&badge.label),
            markdown_cell(&badge.scope),
            markdown_cell(&badge.basis),
            markdown_cell(&badge.message),
            markdown_cell(&badge.status),
            markdown_cell(&badge.color)
        ));
    }

    body.push_str("\n## Supporting/Internal Seam-Native Inventory\n\n");
    body.push_str(&format!(
        "Status: `{}`\n\nSource: `{}`\n\n{}\n\n",
        report.seam_native_counts.status,
        markdown_cell(&report.seam_native_counts.source),
        markdown_cell(&report.seam_native_counts.note)
    ));
    append_count_table(&mut body, &report.seam_native_counts.counts);

    body.push_str("\n## Test-Efficiency Inventory\n\n");
    body.push_str(&format!(
        "Status: `{}`\n\nSource: `{}`\n\n{}\n\n",
        report.test_efficiency_counts.status,
        markdown_cell(&report.test_efficiency_counts.source),
        markdown_cell(&report.test_efficiency_counts.note)
    ));
    append_count_table(&mut body, &report.test_efficiency_counts.counts);

    body.push_str("\n## Canonical Actionable Projection\n\n");
    body.push_str("| Field | Value |\n| --- | --- |\n");
    body.push_str(&format!(
        "| Status | `{}` |\n",
        markdown_cell(&report.canonical_actionable_gap.status)
    ));
    body.push_str(&format!(
        "| Source | `{}` |\n",
        markdown_cell(&report.canonical_actionable_gap.source)
    ));
    body.push_str(&format!(
        "| ripr count | {} |\n",
        optional_count_label(report.canonical_actionable_gap.ripr_count)
    ));
    body.push_str(&format!(
        "| ripr+ count | {} |\n",
        optional_count_label(report.canonical_actionable_gap.ripr_plus_count)
    ));
    body.push_str(&format!(
        "| Detail | {} |\n",
        markdown_cell(&report.canonical_actionable_gap.detail)
    ));

    body.push_str("\n## Supporting Signals\n\n");
    append_signal_row_table(
        &mut body,
        &[
            ("Raw alignment signals", &report.raw_alignment_signals),
            ("Canonical evidence items", &report.canonical_evidence_items),
            ("Static limitations", &report.static_limitations),
            (
                "Suppressed or intentional items",
                &report.suppressed_or_intentional_items,
            ),
            ("No-action items", &report.no_action_items),
        ],
    );

    body.push_str("\n## Recommended Public Projection\n\n");
    body.push_str(&format!(
        "- Basis: `{}`\n",
        markdown_cell(&report.recommended_public_projection)
    ));
    body.push_str("- Rule: README/store badges should count unresolved actionable static repair ");
    body.push_str("gaps using `canonical_actionable_gap`; `ripr+` only adds items projected ");
    body.push_str("into the same repair, verify, and receipt model; seam-native counts stay ");
    body.push_str("supporting/internal.\n");

    body.push_str("\n## Warnings\n\n");
    if report.warnings.is_empty() {
        body.push_str("- none\n");
    } else {
        for warning in &report.warnings {
            body.push_str(&format!("- {}\n", markdown_cell(warning)));
        }
    }

    body.push_str("\n## Non-Claims\n\n");
    body.push_str("- not coverage\n");
    body.push_str("- not runtime mutation confirmation\n");
    body.push_str("- not merge approval\n");
    body.push_str("- not a complete seam inventory\n");
    body
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

fn append_signal_row_table(body: &mut String, rows: &[(&str, &BadgeBasisSignal)]) {
    body.push_str("| Signal | Status | Count | Source | Detail |\n");
    body.push_str("| --- | --- | ---: | --- | --- |\n");
    for (name, signal) in rows {
        body.push_str(&format!(
            "| {} | `{}` | {} | `{}` | {} |\n",
            markdown_cell(name),
            markdown_cell(&signal.status),
            optional_count_label(signal.count),
            markdown_cell(&signal.source),
            markdown_cell(&signal.detail)
        ));
    }
}

fn optional_count_label(count: Option<usize>) -> String {
    count
        .map(|value| value.to_string())
        .unwrap_or_else(|| "n/a".to_string())
}

fn run_repo_badge_artifact_job(format: &str, gap_ledger: Option<&Path>) -> Result<String, String> {
    let timeout = Duration::from_millis(repo_badge_artifact_timeout_ms());
    if let Ok(ripr_bin) = std::env::var("RIPR_BIN") {
        let repo_root = repo_root()?;
        let mut args = vec![
            "check".to_string(),
            "--root".to_string(),
            normalize_path(&repo_root),
            "--format".to_string(),
            format.to_string(),
        ];
        if let Some(gap_ledger) = gap_ledger {
            args.push("--gap-ledger".to_string());
            args.push(normalize_path(gap_ledger));
        }
        return run_repo_badge_artifact_command(&ripr_bin, &args, format, timeout);
    }

    let args = repo_badge_artifact_command_args(format, gap_ledger);
    run_repo_badge_artifact_command("cargo", &args, format, timeout)
}

pub(crate) fn run_repo_badge_artifact_command(
    program: &str,
    args: &[String],
    format: &str,
    timeout: Duration,
) -> Result<String, String> {
    let output = capture_output_with_timeout(
        program,
        args,
        &[],
        timeout,
        &format!("repo badge artifact generation for {format}"),
    )?;
    repo_badge_artifact_stdout_from_output(program, args, format, timeout, output)
}

pub(crate) fn repo_badge_artifact_stdout_from_output(
    program: &str,
    args: &[String],
    format: &str,
    timeout: Duration,
    output: TimedOutput,
) -> Result<String, String> {
    let command = format!("{} {}", program, args.join(" "));
    if output.timed_out {
        return Err(format!(
            "{command} timed out after {} ms while generating repo badge artifact `{format}`; no public badge endpoint was refreshed and no public badge count is claimed. Rerun with {REPO_BADGE_ARTIFACT_TIMEOUT_ENV}=<milliseconds> only in an explicit badge refresh PR if the machine can afford the cost, or pass --gap-ledger to use an existing gap decision ledger.",
            timeout.as_millis()
        ));
    }
    let Some(status) = output.status else {
        return Err(format!(
            "{command} finished without an exit status while generating repo badge artifact `{format}`"
        ));
    };
    if !status.success() {
        return Err(format!(
            "{command} failed with {status}\nstdout:\n{}\nstderr:\n{}",
            output.stdout.trim(),
            output.stderr.trim()
        ));
    }
    Ok(output.stdout)
}

fn run_with_repo_root_cwd<T>(f: impl FnOnce() -> Result<T, String>) -> Result<T, String> {
    let old = std::env::current_dir().map_err(|err| format!("failed to capture cwd: {err}"))?;
    let root = repo_root()?;
    std::env::set_current_dir(&root)
        .map_err(|err| format!("failed to set cwd to {}: {err}", root.display()))?;
    let result = f();
    let restore = std::env::set_current_dir(&old)
        .map_err(|err| format!("failed to restore cwd to {}: {err}", old.display()));
    restore?;
    result
}

pub(crate) fn repo_badge_artifact_jobs() -> Vec<BadgeArtifactJob> {
    vec![
        BadgeArtifactJob {
            format: "repo-badge-json",
            output_file: "repo-ripr-badge.json",
        },
        BadgeArtifactJob {
            format: "repo-badge-shields",
            output_file: "repo-ripr-badge-shields.json",
        },
        BadgeArtifactJob {
            format: "repo-badge-plus-json",
            output_file: "repo-ripr-plus-badge.json",
        },
        BadgeArtifactJob {
            format: "repo-badge-plus-shields",
            output_file: "repo-ripr-plus-badge-shields.json",
        },
    ]
}

pub(crate) fn repo_badge_artifact_command_args(
    format: &str,
    gap_ledger: Option<&Path>,
) -> Vec<String> {
    // Intentionally omits any `--diff` / `--base` argument: repo scope must
    // not consult `git diff origin/main...HEAD`. The regression test
    // `repo_badge_artifact_command_args_does_not_use_git_diff` pins this
    // contract.
    let mut args = vec![
        "run".to_string(),
        "-p".to_string(),
        "ripr".to_string(),
        "--quiet".to_string(),
        "--".to_string(),
        "check".to_string(),
        "--root".to_string(),
        ".".to_string(),
        "--format".to_string(),
        format.to_string(),
    ];
    if let Some(gap_ledger) = gap_ledger {
        args.push("--gap-ledger".to_string());
        args.push(normalize_path(gap_ledger));
    }
    args
}

pub(crate) const REPO_BADGE_ARTIFACT_TIMEOUT_ENV: &str = "RIPR_REPO_BADGE_ARTIFACT_TIMEOUT_MS";
pub(crate) const REPO_BADGE_ARTIFACT_DEFAULT_TIMEOUT_MS: u64 = 90_000;

fn repo_badge_artifact_timeout_ms() -> u64 {
    repo_badge_artifact_timeout_ms_from_env(std::env::var(REPO_BADGE_ARTIFACT_TIMEOUT_ENV).ok())
}

pub(crate) fn repo_badge_artifact_timeout_ms_from_env(value: Option<String>) -> u64 {
    value
        .and_then(|value| value.parse::<u64>().ok())
        .filter(|value| *value > 0)
        .unwrap_or(REPO_BADGE_ARTIFACT_DEFAULT_TIMEOUT_MS)
}

pub(crate) fn repo_badge_artifacts_summary_markdown(
    ripr_native_json: &str,
    ripr_plus_native_json: &str,
    gap_ledger: Option<&Path>,
) -> String {
    let mut markdown = String::from("# ripr repo badges\n\n");
    if let Some(gap_ledger) = gap_ledger {
        markdown.push_str(&format!(
            "Repo-scoped artifacts: rendered from explicit gap decision ledger \
`{}`. Counts reflect policy-targeted `GapRecord` projection eligibility, not \
`git diff origin/main...HEAD`. They are not runtime mutation confirmation.\n\n",
            normalize_path(gap_ledger)
        ));
    } else {
        markdown.push_str(
            "Repo-scoped artifacts: rendered against classified repo seams, not \
against `git diff origin/main...HEAD`. Counts reflect seam-native unresolved \
exposure gaps and unsuppressed actionable test-efficiency findings under the \
configured policy. They are not runtime mutation confirmation.\n\n",
        );
    }
    append_badge_section(&mut markdown, "ripr", ripr_native_json);
    append_badge_section(&mut markdown, "ripr+", ripr_plus_native_json);
    markdown.push_str("## Artifacts\n\n");
    markdown.push_str("- `repo-ripr-badge.json` — native repo-scoped ripr badge\n");
    markdown.push_str(
        "- `repo-ripr-badge-shields.json` — Shields projection of repo-scoped ripr badge\n",
    );
    markdown.push_str("- `repo-ripr-plus-badge.json` — native repo-scoped ripr+ badge\n");
    markdown.push_str(
        "- `repo-ripr-plus-badge-shields.json` — Shields projection of repo-scoped ripr+ badge\n",
    );
    markdown
}

/// Names of the two committed badge endpoint files served via
/// `raw.githubusercontent.com/.../main/badges/<file>`. The `ripr`
/// product contract is "ripr emits Shields-compatible JSON"; this is
/// just the v1 self-hosted dogfood path that copies the latest
/// repo-scoped Shields JSON into a stable repo-relative location.
/// See `docs/BADGE_POLICY.md` and `deferred/hosted-badge-service`.
pub(crate) const BADGE_ENDPOINT_FILES: &[(&str, &str)] = &[
    ("badges/ripr.json", "repo-ripr-badge-shields.json"),
    ("badges/ripr-plus.json", "repo-ripr-plus-badge-shields.json"),
];

/// Regenerates `target/ripr/reports/repo-ripr-{badge,plus-badge}-shields.json`
/// via `repo_badge_artifacts()` and copies the two Shields projections
/// into the committed `badges/` directory so the README endpoint URLs
/// reflect the latest repo-scoped state.
pub(crate) fn update_badge_endpoints_impl(args: &[String]) -> Result<(), String> {
    let options = parse_repo_badge_artifact_options(args, "badges")?;
    run_with_repo_root_cwd(|| {
        write_repo_badge_artifacts(&options)?;
        copy_badge_endpoints_from_reports(Path::new("target/ripr/reports"), Path::new("."))
    })
}

/// Pure file-copy half of `update_badge_endpoints` — separated so the
/// path arithmetic and per-file error wrapping can be unit-tested
/// against tempdirs without invoking `cargo`.
pub(crate) fn copy_badge_endpoints_from_reports(
    reports_dir: &Path,
    repo_root: &Path,
) -> Result<(), String> {
    let badges_dir = repo_root.join("badges");
    fs::create_dir_all(&badges_dir).map_err(|err| {
        format!(
            "failed to create badges directory {}: {err}",
            normalize_path(&badges_dir)
        )
    })?;
    for (committed, source_name) in BADGE_ENDPOINT_FILES {
        let source = reports_dir.join(source_name);
        let bytes = fs::read(&source).map_err(|err| {
            format!(
                "failed to read {} (run `cargo xtask repo-badge-artifacts` first): {err}",
                normalize_path(&source)
            )
        })?;
        validate_shields_endpoint_bytes(&bytes, expected_badge_label(committed)?)?;
        let dest = repo_root.join(committed);
        fs::write(&dest, &bytes)
            .map_err(|err| format!("failed to write {}: {err}", normalize_path(&dest)))?;
    }
    Ok(())
}

/// File-reading wrapper around `badge_endpoint_violation`. Walks
/// `BADGE_ENDPOINT_FILES`, reads each source from `reports_dir` and
/// each committed file from `repo_root`, and collects violations.
/// Splitting this out from `check_badge_endpoints` lets tests exercise
/// the file walk against tempdirs without invoking `cargo`.
pub(crate) fn compute_badge_endpoint_violations(
    reports_dir: &Path,
    repo_root: &Path,
) -> Result<Vec<String>, String> {
    let mut violations = Vec::new();
    for (committed, source_name) in BADGE_ENDPOINT_FILES {
        let source = reports_dir.join(source_name);
        let source_display = normalize_path(&source);
        let want =
            fs::read(&source).map_err(|err| format!("failed to read {source_display}: {err}"))?;
        validate_shields_endpoint_bytes(&want, expected_badge_label(committed)?)?;
        let committed_path = repo_root.join(committed);
        let actual = fs::read(&committed_path).ok();
        if let Some(violation) =
            badge_endpoint_violation(committed, &source_display, &want, actual.as_deref())
        {
            violations.push(violation);
        }
    }
    Ok(violations)
}

/// Pure comparison helper for `check_badge_endpoints` — separated so
/// the violation-string contract is unit-testable without touching
/// the file system. Returns `None` when the committed file is in
/// sync, otherwise an actionable violation message.
pub(crate) fn badge_endpoint_violation(
    committed_path: &str,
    source_display: &str,
    expected_bytes: &[u8],
    actual_bytes: Option<&[u8]>,
) -> Option<String> {
    match actual_bytes {
        None => Some(format!(
            "missing badge endpoint file {committed_path}; run `cargo xtask update-badge-endpoints`"
        )),
        Some(actual) if actual != expected_bytes => Some(format!(
            "badge endpoint file {committed_path} is stale relative to {source_display}; run `cargo xtask update-badge-endpoints` and commit the diff"
        )),
        _ => None,
    }
}

/// Verifies that the committed `badges/*.json` files match the latest
/// `cargo xtask repo-badge-artifacts` output. Fails with an actionable
/// message pointing at `cargo xtask update-badge-endpoints` when stale.
/// Intentionally not added to the default CI gate set in v1 — the
/// endpoint count drifts whenever production code or tests change, and
/// requiring every PR to also update `badges/` is too much friction
/// before the headline stabilizes. Use locally before campaign
/// closeouts and after material analyzer changes.
pub(crate) fn check_badge_endpoints_impl(args: &[String]) -> Result<(), String> {
    let options = parse_repo_badge_artifact_options(args, "badges")?;
    run_with_repo_root_cwd(|| {
        write_repo_badge_artifacts(&options)?;
        let violations =
            compute_badge_endpoint_violations(Path::new("target/ripr/reports"), Path::new("."))?;
        finish_policy_report(
            PolicyReportSpec {
                report_file: "badge-endpoints.md",
                check: "check-badge-endpoints",
                why_it_matters: "The committed badges/*.json files are the public Shields endpoint surfaces; stale files cause the README badge to lie about repo state.",
                fix_kind: FixKind::AuthorDecisionRequired,
                recommended_fixes: &[
                    "Run `cargo xtask update-badge-endpoints` and commit the resulting badges/*.json diff.",
                    "If the drift is from an unrelated PR, run `cargo xtask update-badge-endpoints` on `main` and commit on its own scoped PR.",
                    "Skip running this check on PRs that do not change the repo headline (it is not yet a hard CI gate).",
                ],
                rerun_command: "cargo xtask check-badge-endpoints",
                exception_template: None,
            },
            &violations,
        )
    })
}

fn expected_badge_label(committed_path: &str) -> Result<&'static str, String> {
    match committed_path {
        "badges/ripr.json" => Ok("ripr"),
        "badges/ripr-plus.json" => Ok("ripr+"),
        other => Err(format!("unknown badge endpoint mapping for {other}")),
    }
}

pub(crate) fn validate_shields_endpoint_bytes(
    bytes: &[u8],
    expected_label: &str,
) -> Result<(), String> {
    let value: serde_json::Value = serde_json::from_slice(bytes)
        .map_err(|err| format!("badge endpoint for `{expected_label}` is not valid JSON: {err}"))?;
    let object = value
        .as_object()
        .ok_or_else(|| format!("badge endpoint for `{expected_label}` must be a JSON object"))?;
    let keys: BTreeSet<&str> = object.keys().map(String::as_str).collect();
    let expected_keys = BTreeSet::from(["schemaVersion", "label", "message", "color"]);
    if keys != expected_keys {
        return Err(format!(
            "badge endpoint for `{expected_label}` must contain only schemaVersion, label, message, and color"
        ));
    }
    if object
        .get("schemaVersion")
        .and_then(serde_json::Value::as_u64)
        != Some(1)
    {
        return Err(format!(
            "badge endpoint for `{expected_label}` has unsupported schemaVersion"
        ));
    }
    if object.get("label").and_then(serde_json::Value::as_str) != Some(expected_label) {
        return Err(format!(
            "badge endpoint label drifted: expected `{expected_label}`"
        ));
    }
    for field in ["message", "color"] {
        let Some(text) = object.get(field).and_then(serde_json::Value::as_str) else {
            return Err(format!(
                "badge endpoint for `{expected_label}` field `{field}` must be a string"
            ));
        };
        if text.trim().is_empty() {
            return Err(format!(
                "badge endpoint for `{expected_label}` field `{field}` must not be empty"
            ));
        }
    }
    Ok(())
}

fn append_badge_section(markdown: &mut String, heading: &str, native_json: &str) {
    let message = extract_json_string(native_json, "\"message\":").unwrap_or_default();
    let color = extract_json_string(native_json, "\"color\":").unwrap_or_default();
    let counts = extract_json_object_usize_map(native_json, "\"counts\":");
    let reason_counts = extract_json_object_usize_map(native_json, "\"reason_counts\":");
    let warnings = extract_json_warnings(native_json);

    markdown.push_str(&format!("## {heading}\n\n"));
    markdown.push_str(&format!("- message: {message}\n"));
    markdown.push_str(&format!("- color: {color}\n"));
    markdown.push_str("- counts:\n");
    for (key, value) in &counts {
        markdown.push_str(&format!("  - {key}: {value}\n"));
    }
    markdown.push_str("- reason_counts:\n");
    for (key, value) in &reason_counts {
        markdown.push_str(&format!("  - {key}: {value}\n"));
    }
    if warnings.is_empty() {
        markdown.push_str("- warnings: none\n\n");
    } else {
        markdown.push_str("- warnings:\n");
        for warning in &warnings {
            markdown.push_str(&format!("  - {warning}\n"));
        }
        markdown.push('\n');
    }
}

pub(crate) fn extract_json_string(json: &str, key: &str) -> Option<String> {
    let start = json.find(key)? + key.len();
    let remaining = &json[start..];
    let quote_start = remaining.find('"')?;
    let quote_end = remaining[quote_start + 1..].find('"')?;
    Some(remaining[quote_start + 1..quote_start + 1 + quote_end].to_string())
}

pub(crate) fn extract_json_object_usize_map(json: &str, key: &str) -> BTreeMap<String, usize> {
    let mut entries = BTreeMap::new();
    let object_start = match json.find(key) {
        Some(pos) => {
            let after_key = pos + key.len();
            let remaining = &json[after_key..];
            let brace_pos = remaining.find('{').unwrap_or(0);
            after_key + brace_pos + 1
        }
        None => return entries,
    };

    let object_slice = &json[object_start..];
    let object_end = match object_slice.find('}') {
        Some(pos) => pos,
        None => return entries,
    };

    let object_text = &object_slice[..object_end];
    for part in object_text.split(',') {
        if let Some(colon_pos) = part.find(':') {
            let key_part = part[..colon_pos].trim();
            let value_part = part[colon_pos + 1..].trim();

            if key_part.starts_with('"') && key_part.ends_with('"') {
                let entry_key = key_part[1..key_part.len() - 1].to_string();
                if let Ok(value) = value_part.parse::<usize>() {
                    entries.insert(entry_key, value);
                }
            }
        }
    }
    entries
}

pub(crate) fn extract_json_warnings(json: &str) -> Vec<String> {
    let mut warnings = Vec::new();
    let needle = "\"warnings\":";
    let warnings_start = match json.find(needle) {
        Some(pos) => {
            let after_colon = pos + needle.len();
            let remaining = &json[after_colon..];
            let bracket_pos = remaining.find('[').unwrap_or(0);
            after_colon + bracket_pos + 1
        }
        None => return warnings,
    };

    let remaining = &json[warnings_start..];
    let end_bracket_pos = match remaining.find(']') {
        Some(pos) => pos,
        None => return warnings,
    };

    let warnings_content = &remaining[..end_bracket_pos];

    let mut i = 0;
    let chars: Vec<char> = warnings_content.chars().collect();

    while i < chars.len() {
        if chars[i] == '"' {
            i += 1;
            let mut warning_chars = Vec::new();

            while i < chars.len() && chars[i] != '"' {
                if chars[i] == '\\' && i + 1 < chars.len() {
                    i += 1;
                }
                warning_chars.push(chars[i]);
                i += 1;
            }

            if i < chars.len() && chars[i] == '"' {
                let warning: String = warning_chars.into_iter().collect();
                warnings.push(warning);
            }
        }
        i += 1;
    }

    warnings
}

pub(crate) fn check_badge_diff_policy() -> Result<(), String> {
    check_badge_diff_policy_with_context(badge_refresh_context())
}

pub(crate) fn check_badge_diff_policy_with_context(
    badge_refresh_context: bool,
) -> Result<(), String> {
    let changes = collect_pr_changes()?;
    let violations = badge_diff_policy_violations(&changes, badge_refresh_context);

    finish_policy_report(
        PolicyReportSpec {
            report_file: "badge-diff-policy.md",
            check: "check-badge-diff-policy",
            why_it_matters: "Public RIPR badge endpoint counts are generated trust markers. Ordinary docs, README, and implementation PRs may edit badge links or layout, but must not hand-author badges/*.json endpoint numbers.",
            fix_kind: FixKind::AuthorDecisionRequired,
            recommended_fixes: &[
                "Remove badges/*.json diffs from ordinary PRs.",
                "For repo-scoped badge count refreshes, use `cargo xtask badges` or the Badge Endpoints workflow.",
                "Carry generated endpoint JSON only in an explicit `badge: refresh public endpoints` PR or automation/badge-endpoints branch.",
            ],
            rerun_command: "cargo xtask check-badge-diff-policy",
            exception_template: None,
        },
        &violations,
    )
}

pub(crate) use self::badge_artifacts_impl as badge_artifacts;
pub(crate) use self::badge_basis_impl as badge_basis;
pub(crate) use self::check_badge_endpoints_impl as check_badge_endpoints;
pub(crate) use self::repo_badge_artifacts_impl as repo_badge_artifacts;
pub(crate) use self::ripr_plus_impl as ripr_plus;
pub(crate) use self::update_badge_endpoints_impl as update_badge_endpoints;
