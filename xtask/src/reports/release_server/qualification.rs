use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::policy::distribution::load_distribution_contract;

const ROW_SCHEMA_VERSION: u32 = 1;
const ROW_KIND: &str = "ripr_server_archive_target_execution";
const RECEIPT_SCHEMA_VERSION: u32 = 1;
const RECEIPT_KIND: &str = "ripr_server_archive_terminal_qualification";
pub(crate) const TERMINAL_RECEIPT_JSON: &str = "server-archive-terminal-qualification-receipt.json";
pub(crate) const TERMINAL_RECEIPT_MARKDOWN: &str =
    "server-archive-terminal-qualification-receipt.md";

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum TargetExecutionOutcome {
    Success,
    Failure,
    Cancelled,
    NotRun,
}

impl TargetExecutionOutcome {
    fn as_str(self) -> &'static str {
        match self {
            Self::Success => "success",
            Self::Failure => "failure",
            Self::Cancelled => "cancelled",
            Self::NotRun => "not_run",
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct TargetExecutionRow {
    pub(crate) schema_version: u32,
    pub(crate) kind: String,
    pub(crate) target: String,
    pub(crate) outcome: TargetExecutionOutcome,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub(crate) struct TerminalTargetState {
    pub(crate) target: String,
    pub(crate) outcome: TargetExecutionOutcome,
    pub(crate) observed: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub(crate) struct TerminalExecutionCounts {
    pub(crate) selected: usize,
    pub(crate) executed: usize,
    pub(crate) succeeded: usize,
    pub(crate) failed: usize,
    pub(crate) cancelled: usize,
    pub(crate) not_run: usize,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub(crate) struct TerminalUpstreamResults {
    pub(crate) verify_candidate: String,
    pub(crate) build: String,
    pub(crate) manifest: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub(crate) struct TerminalQualificationReceipt {
    pub(crate) schema_version: u32,
    pub(crate) kind: String,
    pub(crate) status: String,
    pub(crate) candidate_sha: String,
    pub(crate) version: String,
    pub(crate) upstream: TerminalUpstreamResults,
    pub(crate) execution: TerminalExecutionCounts,
    pub(crate) targets: Vec<TerminalTargetState>,
    pub(crate) qualification_claims_allowed: bool,
    pub(crate) errors: Vec<String>,
}

pub(crate) fn write_terminal_receipt(
    rows_dir: &Path,
    out_dir: &Path,
    candidate_sha: &str,
    version: &str,
    verify_result: &str,
    build_result: &str,
    manifest_result: &str,
) -> Result<TerminalQualificationReceipt, String> {
    let contract = load_distribution_contract()?;
    let mut expected_targets = contract
        .target
        .iter()
        .map(|target| target.rust_target.clone())
        .collect::<Vec<_>>();
    expected_targets.sort();
    expected_targets.dedup();
    let (rows, row_errors) = read_target_rows(rows_dir)?;
    let receipt = classify_terminal_receipt(
        &expected_targets,
        rows,
        row_errors,
        candidate_sha,
        version,
        verify_result,
        build_result,
        manifest_result,
    );
    fs::create_dir_all(out_dir)
        .map_err(|err| format!("failed to create {}: {err}", out_dir.display()))?;
    let json_path = out_dir.join(TERMINAL_RECEIPT_JSON);
    let markdown_path = out_dir.join(TERMINAL_RECEIPT_MARKDOWN);
    let rendered = serde_json::to_string_pretty(&receipt)
        .map_err(|err| format!("failed to render terminal qualification receipt: {err}"))?;
    fs::write(&json_path, format!("{rendered}\n"))
        .map_err(|err| format!("failed to write {}: {err}", json_path.display()))?;
    fs::write(&markdown_path, render_terminal_receipt_markdown(&receipt))
        .map_err(|err| format!("failed to write {}: {err}", markdown_path.display()))?;
    Ok(receipt)
}

fn read_target_rows(rows_dir: &Path) -> Result<(Vec<TargetExecutionRow>, Vec<String>), String> {
    if !rows_dir.exists() {
        return Ok((Vec::new(), Vec::new()));
    }
    let mut paths = Vec::new();
    collect_json_files(rows_dir, &mut paths)?;
    paths.sort();
    let mut rows = Vec::new();
    let mut errors = Vec::new();
    for path in paths {
        let text = match fs::read_to_string(&path) {
            Ok(text) => text,
            Err(err) => {
                errors.push(format!("failed to read {}: {err}", path.display()));
                continue;
            }
        };
        match serde_json::from_str::<TargetExecutionRow>(&text) {
            Ok(row) => rows.push(row),
            Err(err) => errors.push(format!("failed to parse {}: {err}", path.display())),
        }
    }
    Ok((rows, errors))
}

fn collect_json_files(directory: &Path, output: &mut Vec<PathBuf>) -> Result<(), String> {
    for entry in fs::read_dir(directory)
        .map_err(|err| format!("failed to read {}: {err}", directory.display()))?
    {
        let path = entry
            .map_err(|err| format!("failed to read entry under {}: {err}", directory.display()))?
            .path();
        let metadata = fs::symlink_metadata(&path)
            .map_err(|err| format!("failed to stat {}: {err}", path.display()))?;
        if metadata.file_type().is_symlink() {
            return Err(format!(
                "target execution receipt directory must not contain symlinks: {}",
                path.display()
            ));
        }
        if metadata.is_dir() {
            collect_json_files(&path, output)?;
        } else if metadata.is_file()
            && path.extension().and_then(|value| value.to_str()) == Some("json")
        {
            output.push(path);
        }
    }
    Ok(())
}

#[allow(
    clippy::too_many_arguments,
    reason = "terminal receipt classification takes the full fail-closed receipt input set in one call"
)]
fn classify_terminal_receipt(
    expected_targets: &[String],
    rows: Vec<TargetExecutionRow>,
    mut errors: Vec<String>,
    candidate_sha: &str,
    version: &str,
    verify_result: &str,
    build_result: &str,
    manifest_result: &str,
) -> TerminalQualificationReceipt {
    let expected = expected_targets.iter().cloned().collect::<BTreeSet<_>>();
    let mut observed = BTreeMap::new();
    for row in rows {
        if row.schema_version != ROW_SCHEMA_VERSION {
            errors.push(format!(
                "target `{}` row schema must be {ROW_SCHEMA_VERSION}, got {}",
                row.target, row.schema_version
            ));
            continue;
        }
        if row.kind != ROW_KIND {
            errors.push(format!(
                "target `{}` row kind must be `{ROW_KIND}`, got `{}`",
                row.target, row.kind
            ));
            continue;
        }
        if row.outcome == TargetExecutionOutcome::NotRun {
            errors.push(format!(
                "target `{}` must be absent, not observed as `not_run`",
                row.target
            ));
            continue;
        }
        if !expected.contains(&row.target) {
            errors.push(format!("unexpected target execution row `{}`", row.target));
            continue;
        }
        if observed.insert(row.target.clone(), row.outcome).is_some() {
            errors.push(format!("duplicate target execution row `{}`", row.target));
        }
    }

    let mut targets = Vec::new();
    for target in expected_targets {
        let outcome = observed
            .get(target)
            .copied()
            .unwrap_or(TargetExecutionOutcome::NotRun);
        targets.push(TerminalTargetState {
            target: target.clone(),
            outcome,
            observed: outcome != TargetExecutionOutcome::NotRun,
        });
    }
    let counts = TerminalExecutionCounts {
        selected: targets.len(),
        executed: targets.iter().filter(|row| row.observed).count(),
        succeeded: targets
            .iter()
            .filter(|row| row.outcome == TargetExecutionOutcome::Success)
            .count(),
        failed: targets
            .iter()
            .filter(|row| row.outcome == TargetExecutionOutcome::Failure)
            .count(),
        cancelled: targets
            .iter()
            .filter(|row| row.outcome == TargetExecutionOutcome::Cancelled)
            .count(),
        not_run: targets
            .iter()
            .filter(|row| row.outcome == TargetExecutionOutcome::NotRun)
            .count(),
    };
    let upstream = TerminalUpstreamResults {
        verify_candidate: normalize_upstream_result("verify-candidate", verify_result, &mut errors),
        build: normalize_upstream_result("build", build_result, &mut errors),
        manifest: normalize_upstream_result("manifest", manifest_result, &mut errors),
    };
    let upstream_success = upstream.verify_candidate == "success"
        && upstream.build == "success"
        && upstream.manifest == "success";
    let candidate_sha = candidate_sha.trim().to_ascii_lowercase();
    if candidate_sha.len() != 40
        || !candidate_sha
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        errors.push(format!(
            "candidate_sha must be 40 hexadecimal characters, got `{candidate_sha}`"
        ));
    }
    let version = version.trim().trim_start_matches('v').to_string();
    if version.is_empty() || version.contains(['/', '\\']) {
        errors.push(format!(
            "version must be a non-empty release version, got `{version}`"
        ));
    }
    let qualification_claims_allowed = upstream_success
        && errors.is_empty()
        && counts.selected == expected_targets.len()
        && counts.executed == counts.selected
        && counts.succeeded == counts.selected
        && counts.failed == 0
        && counts.cancelled == 0
        && counts.not_run == 0;
    TerminalQualificationReceipt {
        schema_version: RECEIPT_SCHEMA_VERSION,
        kind: RECEIPT_KIND.to_string(),
        status: if qualification_claims_allowed {
            "qualified".to_string()
        } else {
            "not_qualified".to_string()
        },
        candidate_sha,
        version,
        upstream,
        execution: counts,
        targets,
        qualification_claims_allowed,
        errors,
    }
}

fn normalize_upstream_result(label: &str, value: &str, errors: &mut Vec<String>) -> String {
    let value = value.trim().to_ascii_lowercase();
    if matches!(
        value.as_str(),
        "success" | "failure" | "cancelled" | "skipped"
    ) {
        value
    } else {
        errors.push(format!(
            "upstream result `{label}` has unsupported value `{value}`"
        ));
        "unknown".to_string()
    }
}

fn render_terminal_receipt_markdown(receipt: &TerminalQualificationReceipt) -> String {
    let mut output = format!(
        "# Server archive terminal qualification receipt\n\n- status: `{}`\n- qualification claims allowed: `{}`\n- candidate SHA: `{}`\n- version: `{}`\n- upstream verify/build/manifest: `{}` / `{}` / `{}`\n- selected/executed/succeeded/failed/cancelled/not run: `{}` / `{}` / `{}` / `{}` / `{}` / `{}`\n\n## Target outcomes\n\n| Target | Outcome | Observed |\n| --- | --- | --- |\n",
        receipt.status,
        receipt.qualification_claims_allowed,
        receipt.candidate_sha,
        receipt.version,
        receipt.upstream.verify_candidate,
        receipt.upstream.build,
        receipt.upstream.manifest,
        receipt.execution.selected,
        receipt.execution.executed,
        receipt.execution.succeeded,
        receipt.execution.failed,
        receipt.execution.cancelled,
        receipt.execution.not_run,
    );
    for target in &receipt.targets {
        output.push_str(&format!(
            "| `{}` | `{}` | `{}` |\n",
            target.target,
            target.outcome.as_str(),
            target.observed
        ));
    }
    if !receipt.errors.is_empty() {
        output.push_str("\n## Errors\n\n");
        for error in &receipt.errors {
            output.push_str(&format!("- {error}\n"));
        }
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;

    fn targets() -> Vec<String> {
        [
            "linux-aarch64",
            "linux-x64",
            "macos-aarch64",
            "macos-x64",
            "windows-x64",
        ]
        .map(str::to_string)
        .to_vec()
    }

    fn success_rows() -> Vec<TargetExecutionRow> {
        targets()
            .into_iter()
            .map(|target| TargetExecutionRow {
                schema_version: ROW_SCHEMA_VERSION,
                kind: ROW_KIND.to_string(),
                target,
                outcome: TargetExecutionOutcome::Success,
            })
            .collect()
    }

    #[test]
    fn complete_success_allows_qualification_claims() {
        let receipt = classify_terminal_receipt(
            &targets(),
            success_rows(),
            Vec::new(),
            &"a".repeat(40),
            "0.11.0",
            "success",
            "success",
            "success",
        );
        assert!(receipt.qualification_claims_allowed);
        assert_eq!(receipt.status, "qualified");
        assert_eq!(receipt.execution.executed, 5);
        assert_eq!(receipt.execution.not_run, 0);
    }

    #[test]
    fn missing_target_retains_nonpass_receipt() {
        let mut rows = success_rows();
        rows.pop();
        let receipt = classify_terminal_receipt(
            &targets(),
            rows,
            Vec::new(),
            &"a".repeat(40),
            "0.11.0",
            "success",
            "failure",
            "skipped",
        );
        assert!(!receipt.qualification_claims_allowed);
        assert_eq!(receipt.status, "not_qualified");
        assert_eq!(receipt.execution.executed, 4);
        assert_eq!(receipt.execution.not_run, 1);
    }

    #[test]
    fn failed_target_retains_failed_denominator() {
        let mut rows = success_rows();
        rows[2].outcome = TargetExecutionOutcome::Failure;
        let receipt = classify_terminal_receipt(
            &targets(),
            rows,
            Vec::new(),
            &"a".repeat(40),
            "0.11.0",
            "success",
            "failure",
            "skipped",
        );
        assert!(!receipt.qualification_claims_allowed);
        assert_eq!(receipt.execution.executed, 5);
        assert_eq!(receipt.execution.failed, 1);
        assert_eq!(receipt.execution.not_run, 0);
    }

    #[test]
    fn duplicate_and_unexpected_rows_fail_closed() {
        let mut rows = success_rows();
        rows.push(rows[0].clone());
        rows.push(TargetExecutionRow {
            schema_version: ROW_SCHEMA_VERSION,
            kind: ROW_KIND.to_string(),
            target: "other".to_string(),
            outcome: TargetExecutionOutcome::Success,
        });
        let receipt = classify_terminal_receipt(
            &targets(),
            rows,
            Vec::new(),
            &"a".repeat(40),
            "0.11.0",
            "success",
            "success",
            "success",
        );
        assert!(!receipt.qualification_claims_allowed);
        assert!(
            receipt
                .errors
                .iter()
                .any(|error| error.contains("duplicate"))
        );
        assert!(
            receipt
                .errors
                .iter()
                .any(|error| error.contains("unexpected"))
        );
    }

    #[test]
    fn malformed_candidate_identity_fails_closed() {
        let receipt = classify_terminal_receipt(
            &targets(),
            success_rows(),
            Vec::new(),
            "not-a-sha",
            "v",
            "success",
            "success",
            "success",
        );
        assert!(!receipt.qualification_claims_allowed);
        assert_eq!(receipt.status, "not_qualified");
        assert_eq!(receipt.version, "");
        assert!(
            receipt
                .errors
                .iter()
                .any(|error| error.contains("candidate_sha must be 40 hexadecimal"))
        );
        assert!(
            receipt
                .errors
                .iter()
                .any(|error| error.contains("version must be a non-empty release version"))
        );
    }

    #[test]
    fn trimmed_and_normalized_identity_is_retained() {
        let receipt = classify_terminal_receipt(
            &targets(),
            success_rows(),
            Vec::new(),
            " ABCDEF0123456789ABCDEF0123456789ABCDEF01 ",
            " v0.11.0 ",
            "success",
            "success",
            "success",
        );
        assert!(receipt.qualification_claims_allowed);
        assert_eq!(receipt.status, "qualified");
        assert_eq!(
            receipt.candidate_sha,
            "abcdef0123456789abcdef0123456789abcdef01"
        );
        assert_eq!(receipt.version, "0.11.0");
    }
}
