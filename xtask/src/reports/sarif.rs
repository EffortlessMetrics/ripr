//! SARIF policy command (`sarif-policy`): compares a current ripr SARIF log
//! against an optional baseline and reports new configured-threshold results
//! in advisory, baseline-check, or fail-on-new-warning mode, plus its
//! exclusive argument parsing, SARIF result extraction, fingerprinting, and
//! JSON/markdown rendering helpers.
//!
//! Extracted verbatim from `main.rs` as a behavior-preserving decomposition
//! slice of #2119. Items are `pub(crate)` where `tests.rs` or `dispatch.rs`
//! need them so existing call sites compile unchanged.

use crate::{
    json_string_field, json_usize_field, md_escape, normalize_path, read_text_lossy, write_report,
};
use serde_json::Value;
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug)]
pub(crate) struct SarifPolicyArgs {
    pub(crate) current: PathBuf,
    pub(crate) baseline: Option<PathBuf>,
    pub(crate) mode: SarifPolicyMode,
    pub(crate) threshold: SarifPolicyThreshold,
    pub(crate) missing_baseline: SarifMissingBaseline,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum SarifPolicyMode {
    Advisory,
    BaselineCheck,
    FailOnNewWarning,
}

impl SarifPolicyMode {
    fn as_str(self) -> &'static str {
        match self {
            Self::Advisory => "advisory",
            Self::BaselineCheck => "baseline-check",
            Self::FailOnNewWarning => "fail-on-new-warning",
        }
    }

    fn from_str(value: &str) -> Option<Self> {
        match value {
            "advisory" => Some(Self::Advisory),
            "baseline-check" => Some(Self::BaselineCheck),
            "fail-on-new-warning" => Some(Self::FailOnNewWarning),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum SarifPolicyThreshold {
    Warning,
    Note,
}

impl SarifPolicyThreshold {
    fn as_str(self) -> &'static str {
        match self {
            Self::Warning => "warning",
            Self::Note => "note",
        }
    }

    fn from_str(value: &str) -> Option<Self> {
        match value {
            "warning" => Some(Self::Warning),
            "note" => Some(Self::Note),
            _ => None,
        }
    }

    fn includes(self, level: &str) -> bool {
        match self {
            Self::Warning => level == "warning",
            Self::Note => matches!(level, "warning" | "note"),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum SarifMissingBaseline {
    Advisory,
    Error,
}

impl SarifMissingBaseline {
    fn from_str(value: &str) -> Option<Self> {
        match value {
            "advisory" => Some(Self::Advisory),
            "error" => Some(Self::Error),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct SarifPolicyResult {
    pub(crate) key: String,
    pub(crate) rule_id: String,
    pub(crate) level: String,
    pub(crate) fingerprint: String,
    pub(crate) uri: String,
    pub(crate) line: Option<usize>,
    pub(crate) message: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct SarifPolicyReport {
    pub(crate) mode: SarifPolicyMode,
    pub(crate) threshold: SarifPolicyThreshold,
    pub(crate) status: String,
    pub(crate) current_path: String,
    pub(crate) baseline_path: Option<String>,
    pub(crate) baseline_missing: bool,
    pub(crate) current_results_total: usize,
    pub(crate) current_compared_results: usize,
    pub(crate) baseline_results_total: usize,
    pub(crate) baseline_compared_results: usize,
    pub(crate) new_results: Vec<SarifPolicyResult>,
}

pub(crate) fn sarif_policy_impl(args: &[String]) -> Result<(), String> {
    let parsed = parse_sarif_policy_args(args)?;
    let current_text = read_text_lossy(&parsed.current)?;
    let current_results = parse_sarif_policy_results(&current_text, "current SARIF")?;

    let (baseline_results, baseline_missing) = match parsed.baseline.as_ref() {
        Some(path) if path.exists() => {
            let baseline_text = read_text_lossy(path)?;
            (
                Some(parse_sarif_policy_results(
                    &baseline_text,
                    "baseline SARIF",
                )?),
                false,
            )
        }
        Some(_) | None => (None, true),
    };

    let report = build_sarif_policy_report(
        parsed.mode,
        parsed.threshold,
        normalize_path(&parsed.current),
        parsed.baseline.as_ref().map(|path| normalize_path(path)),
        &current_results,
        baseline_results.as_deref(),
        baseline_missing,
    );

    write_report("sarif-policy.json", &sarif_policy_report_json(&report)?)?;
    write_report("sarif-policy.md", &sarif_policy_report_markdown(&report))?;

    if report.baseline_missing && parsed.missing_baseline == SarifMissingBaseline::Error {
        return Err("SARIF policy baseline is missing".to_string());
    }
    if parsed.mode == SarifPolicyMode::FailOnNewWarning && !report.new_results.is_empty() {
        return Err(format!(
            "SARIF policy found {} new {} result(s)",
            report.new_results.len(),
            parsed.threshold.as_str()
        ));
    }
    Ok(())
}

pub(crate) fn parse_sarif_policy_args(args: &[String]) -> Result<SarifPolicyArgs, String> {
    let mut current: Option<PathBuf> = None;
    let mut baseline: Option<PathBuf> = None;
    let mut mode = SarifPolicyMode::Advisory;
    let mut threshold = SarifPolicyThreshold::Warning;
    let mut missing_baseline = SarifMissingBaseline::Advisory;
    let mut index = 0;

    while index < args.len() {
        let arg = args[index].as_str();
        match arg {
            "--current" | "--sarif" => {
                index += 1;
                let Some(path) = args.get(index) else {
                    return Err(format!(
                        "missing value for `{arg}`\n{}",
                        sarif_policy_usage()
                    ));
                };
                current = Some(PathBuf::from(path));
            }
            "--baseline" => {
                index += 1;
                let Some(path) = args.get(index) else {
                    return Err(format!(
                        "missing value for `{arg}`\n{}",
                        sarif_policy_usage()
                    ));
                };
                baseline = Some(PathBuf::from(path));
            }
            "--mode" => {
                index += 1;
                let Some(value) = args.get(index) else {
                    return Err(format!(
                        "missing value for `{arg}`\n{}",
                        sarif_policy_usage()
                    ));
                };
                let Some(parsed) = SarifPolicyMode::from_str(value) else {
                    return Err(format!(
                        "unsupported SARIF policy mode `{value}`; expected advisory, baseline-check, or fail-on-new-warning"
                    ));
                };
                mode = parsed;
            }
            "--threshold" => {
                index += 1;
                let Some(value) = args.get(index) else {
                    return Err(format!(
                        "missing value for `{arg}`\n{}",
                        sarif_policy_usage()
                    ));
                };
                let Some(parsed) = SarifPolicyThreshold::from_str(value) else {
                    return Err(
                        "unsupported SARIF policy threshold; expected warning or note".to_string(),
                    );
                };
                threshold = parsed;
            }
            "--missing-baseline" => {
                index += 1;
                let Some(value) = args.get(index) else {
                    return Err(format!(
                        "missing value for `{arg}`\n{}",
                        sarif_policy_usage()
                    ));
                };
                let Some(parsed) = SarifMissingBaseline::from_str(value) else {
                    return Err(
                        "unsupported missing-baseline behavior; expected advisory or error"
                            .to_string(),
                    );
                };
                missing_baseline = parsed;
            }
            "--help" | "-h" => return Err(sarif_policy_usage()),
            flag if flag.starts_with('-') => {
                return Err(format!(
                    "unknown sarif-policy option `{flag}`\n{}",
                    sarif_policy_usage()
                ));
            }
            other => {
                return Err(format!(
                    "unexpected positional argument `{other}`\n{}",
                    sarif_policy_usage()
                ));
            }
        }
        index += 1;
    }

    let Some(current) = current else {
        return Err(format!(
            "sarif-policy requires `--current <path>`\n{}",
            sarif_policy_usage()
        ));
    };

    Ok(SarifPolicyArgs {
        current,
        baseline,
        mode,
        threshold,
        missing_baseline,
    })
}

fn sarif_policy_usage() -> String {
    "usage: cargo xtask sarif-policy --current <path> [--baseline <path>] [--mode advisory|baseline-check|fail-on-new-warning] [--threshold warning|note] [--missing-baseline advisory|error]"
        .to_string()
}

pub(crate) fn parse_sarif_policy_results(
    text: &str,
    label: &str,
) -> Result<Vec<SarifPolicyResult>, String> {
    let value: Value =
        serde_json::from_str(text).map_err(|err| format!("failed to parse {label}: {err}"))?;
    let runs = value
        .get("runs")
        .and_then(Value::as_array)
        .ok_or_else(|| format!("{label} is missing SARIF `runs` array"))?;
    let mut out = Vec::new();
    for run in runs {
        let Some(results) = run.get("results").and_then(Value::as_array) else {
            continue;
        };
        for result in results {
            if result_has_suppression(result) {
                continue;
            }
            let rule_id = json_string_field(result, "ruleId").unwrap_or_else(|| "unknown".into());
            let level = json_string_field(result, "level").unwrap_or_else(|| "warning".into());
            let fingerprint = sarif_policy_fingerprint(result);
            let location = sarif_policy_location(result);
            let message = result
                .get("message")
                .and_then(|message| json_string_field(message, "text"))
                .unwrap_or_else(|| "ripr SARIF result".to_string());
            let key = format!("{rule_id}|{fingerprint}");
            out.push(SarifPolicyResult {
                key,
                rule_id,
                level,
                fingerprint,
                uri: location.0,
                line: location.1,
                message,
            });
        }
    }
    out.sort_by(|a, b| a.key.cmp(&b.key));
    out.dedup_by(|a, b| a.key == b.key);
    Ok(out)
}

pub(crate) fn build_sarif_policy_report(
    mode: SarifPolicyMode,
    threshold: SarifPolicyThreshold,
    current_path: String,
    baseline_path: Option<String>,
    current_results: &[SarifPolicyResult],
    baseline_results: Option<&[SarifPolicyResult]>,
    baseline_missing: bool,
) -> SarifPolicyReport {
    let current_compared = filtered_sarif_policy_results(current_results, threshold);
    let baseline_compared = baseline_results
        .map(|results| filtered_sarif_policy_results(results, threshold))
        .unwrap_or_default();
    let baseline_keys = baseline_compared
        .iter()
        .map(|result| result.key.as_str())
        .collect::<BTreeSet<_>>();
    let new_results = if baseline_missing {
        Vec::new()
    } else {
        current_compared
            .iter()
            .filter(|result| !baseline_keys.contains(result.key.as_str()))
            .map(|result| (*result).clone())
            .collect::<Vec<_>>()
    };
    let status = if baseline_missing {
        "advisory_missing_baseline"
    } else if new_results.is_empty() {
        "pass"
    } else if mode == SarifPolicyMode::FailOnNewWarning {
        "fail"
    } else {
        "new_results"
    };

    SarifPolicyReport {
        mode,
        threshold,
        status: status.to_string(),
        current_path,
        baseline_path,
        baseline_missing,
        current_results_total: current_results.len(),
        current_compared_results: current_compared.len(),
        baseline_results_total: baseline_results.map_or(0, <[SarifPolicyResult]>::len),
        baseline_compared_results: baseline_compared.len(),
        new_results,
    }
}

fn filtered_sarif_policy_results(
    results: &[SarifPolicyResult],
    threshold: SarifPolicyThreshold,
) -> Vec<&SarifPolicyResult> {
    results
        .iter()
        .filter(|result| threshold.includes(&result.level))
        .collect()
}

pub(crate) fn sarif_policy_report_json(report: &SarifPolicyReport) -> Result<String, String> {
    let value = serde_json::json!({
        "schema_version": "0.1",
        "tool": "ripr",
        "status": report.status,
        "mode": report.mode.as_str(),
        "threshold": report.threshold.as_str(),
        "current": {
            "path": report.current_path,
            "results_total": report.current_results_total,
            "compared_results": report.current_compared_results
        },
        "baseline": {
            "path": report.baseline_path,
            "missing": report.baseline_missing,
            "results_total": report.baseline_results_total,
            "compared_results": report.baseline_compared_results
        },
        "new_results_total": report.new_results.len(),
        "new_results": report.new_results.iter().map(|result| {
            serde_json::json!({
                "rule_id": result.rule_id,
                "level": result.level,
                "fingerprint": result.fingerprint,
                "uri": result.uri,
                "line": result.line,
                "message": result.message
            })
        }).collect::<Vec<_>>()
    });
    serde_json::to_string_pretty(&value)
        .map(|mut rendered| {
            rendered.push('\n');
            rendered
        })
        .map_err(|err| format!("failed to render SARIF policy JSON: {err}"))
}

pub(crate) fn sarif_policy_report_markdown(report: &SarifPolicyReport) -> String {
    let mut out = String::new();
    out.push_str("# ripr SARIF policy report\n\n");
    out.push_str(&format!("Status: {}\n\n", report.status));
    out.push_str(&format!("- mode: `{}`\n", report.mode.as_str()));
    out.push_str(&format!("- threshold: `{}`\n", report.threshold.as_str()));
    out.push_str(&format!(
        "- current: `{}`\n",
        md_escape(&report.current_path)
    ));
    match &report.baseline_path {
        Some(path) => out.push_str(&format!("- baseline: `{}`\n", md_escape(path))),
        None => out.push_str("- baseline: not provided\n"),
    }
    out.push_str(&format!(
        "- current compared results: {}\n",
        report.current_compared_results
    ));
    out.push_str(&format!(
        "- baseline compared results: {}\n",
        report.baseline_compared_results
    ));
    if report.baseline_missing {
        out.push_str(
            "\nBaseline is missing; this is advisory unless `--missing-baseline error` is set.\n",
        );
        return out;
    }
    if report.new_results.is_empty() {
        out.push_str("\nNo new configured-threshold SARIF results were detected.\n");
        return out;
    }
    out.push_str("\n## New results\n\n");
    for result in &report.new_results {
        out.push_str(&format!(
            "- `{}` `{}` {}:{} — {}\n",
            result.rule_id,
            result.level,
            md_escape(&result.uri),
            result.line.map_or("?".to_string(), |line| line.to_string()),
            md_escape(&result.message)
        ));
    }
    out
}

fn result_has_suppression(result: &Value) -> bool {
    result
        .get("suppressions")
        .and_then(Value::as_array)
        .is_some_and(|suppressions| !suppressions.is_empty())
}

fn sarif_policy_fingerprint(result: &Value) -> String {
    if let Some(fingerprint) = result
        .get("partialFingerprints")
        .and_then(|fingerprints| json_string_field(fingerprints, "riprFingerprintV1"))
    {
        return fingerprint;
    }
    if let Some(fingerprints) = result.get("partialFingerprints").and_then(Value::as_object) {
        for value in fingerprints.values() {
            if let Some(fingerprint) = value.as_str() {
                return fingerprint.to_string();
            }
        }
    }
    let (uri, line) = sarif_policy_location(result);
    let message = result
        .get("message")
        .and_then(|message| json_string_field(message, "text"))
        .unwrap_or_default();
    format!(
        "{}|{}|{}",
        normalize_path(Path::new(&uri)),
        line.map_or(0, |line| line),
        message
    )
}

fn sarif_policy_location(result: &Value) -> (String, Option<usize>) {
    let Some(location) = result
        .get("locations")
        .and_then(Value::as_array)
        .and_then(|locations| locations.first())
    else {
        return ("unknown".to_string(), None);
    };
    let physical = location.get("physicalLocation");
    let uri = physical
        .and_then(|physical| physical.get("artifactLocation"))
        .and_then(|artifact| json_string_field(artifact, "uri"))
        .unwrap_or_else(|| "unknown".to_string());
    let line = physical
        .and_then(|physical| physical.get("region"))
        .and_then(|region| json_usize_field(region, "startLine"));
    (uri, line)
}

pub(crate) use self::sarif_policy_impl as sarif_policy;
