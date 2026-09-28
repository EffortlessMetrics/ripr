use serde_json::{Value, json};

const SCHEMA_VERSION: &str = "0.1";
const REPORT_KIND: &str = "policy_history";
const LIMITS_NOTE: &str = "Read-only advisory policy history report. It reads explicit history inputs and never appends, mutates policy, or changes gate authority.";

pub(crate) const DEFAULT_POLICY_HISTORY_OUT: &str = "target/ripr/reports/policy-history.json";
pub(crate) const DEFAULT_POLICY_HISTORY_MD_OUT: &str = "target/ripr/reports/policy-history.md";

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PolicyHistoryInput {
    pub(crate) root: String,
    pub(crate) generated_at: String,
    pub(crate) current_path: String,
    pub(crate) history_path: Option<String>,
    pub(crate) commit: Option<String>,
    pub(crate) pr_number: Option<String>,
    pub(crate) current_json: Result<String, String>,
    pub(crate) history_jsonl: Option<Result<String, String>>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PolicyHistoryReport {
    root: String,
    generated_at: String,
    current: PolicySnapshot,
    history_summary: HistorySummary,
    trend: PolicyTrend,
    example_append_record: PolicySnapshot,
    warnings: Vec<Notice>,
    unknowns: Vec<Notice>,
    input_artifacts: Vec<InputArtifact>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct PolicySnapshot {
    commit: Option<String>,
    pr_number: Option<String>,
    generated_at: String,
    recommended_mode: String,
    current_policy_ceiling: String,
    baseline_health: String,
    waiver_health: String,
    suppression_health: String,
    calibration_health: String,
    preview_boundary_state: String,
    new_policy_eligible_count: usize,
    waiver_count: usize,
    stale_suppression_count: usize,
    baseline_still_present: usize,
    baseline_resolved: usize,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct SnapshotAvailability {
    new_policy_eligible_count: bool,
    waiver_count: bool,
    stale_suppression_count: bool,
    baseline_still_present: bool,
    baseline_resolved: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct SnapshotRecord {
    snapshot: PolicySnapshot,
    availability: SnapshotAvailability,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct HistorySummary {
    entries: usize,
    oldest_generated_at: Option<String>,
    newest_generated_at: Option<String>,
    readiness_improved: bool,
    waiver_pressure_increased: bool,
    suppression_health_regressed: bool,
    baseline_shrank: bool,
    preview_remained_advisory: bool,
    calibration_changed_ceiling: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct PolicyTrend {
    ceiling: TrendValue,
    waiver_count: TrendValue,
    stale_suppression_count: TrendValue,
    baseline_still_present: TrendValue,
    baseline_resolved: TrendValue,
    preview_boundary_state: TrendValue,
    calibration_health: TrendValue,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct TrendValue {
    previous: Option<String>,
    current: Option<String>,
    direction: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct Notice {
    kind: String,
    message: String,
    source_artifact: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct InputArtifact {
    kind: String,
    path: Option<String>,
    status: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ArtifactStatus {
    Read,
    Missing,
    Malformed,
    Omitted,
}

#[derive(Clone, Debug)]
struct ParsedJson {
    status: ArtifactStatus,
    value: Option<Value>,
}

pub(crate) fn build_policy_history_report(input: PolicyHistoryInput) -> PolicyHistoryReport {
    let mut warnings = Vec::new();
    let mut unknowns = Vec::new();

    let current_artifact = parse_required_json(
        "policy_operations",
        &input.current_path,
        input.current_json.clone(),
        &mut warnings,
    );
    let (history_status, history_values) = parse_history_jsonl(
        input.history_path.as_deref(),
        &input.history_jsonl,
        &mut warnings,
        &mut unknowns,
    );

    if input.commit.is_none() {
        unknowns.push(Notice {
            kind: "commit_not_supplied".to_string(),
            message: "No commit identity was supplied for the current policy snapshot.".to_string(),
            source_artifact: None,
        });
    }
    if input.pr_number.is_none() {
        unknowns.push(Notice {
            kind: "pr_number_not_supplied".to_string(),
            message: "No PR number was supplied for the current policy snapshot.".to_string(),
            source_artifact: None,
        });
    }

    let mut current_record = current_artifact
        .value
        .as_ref()
        .map(|value| snapshot_from_operations(value, &input))
        .unwrap_or_else(|| {
            unknowns.push(Notice {
                kind: "current_policy_operations_unavailable".to_string(),
                message: "Current policy operations input is unavailable.".to_string(),
                source_artifact: Some(input.current_path.clone()),
            });
            SnapshotRecord {
                snapshot: PolicySnapshot {
                    commit: input.commit.clone(),
                    pr_number: input.pr_number.clone(),
                    generated_at: input.generated_at.clone(),
                    recommended_mode: "advisory-only".to_string(),
                    current_policy_ceiling: "config_error".to_string(),
                    baseline_health: "unknown".to_string(),
                    waiver_health: "unknown".to_string(),
                    suppression_health: "unknown".to_string(),
                    calibration_health: "unknown".to_string(),
                    preview_boundary_state: "unknown".to_string(),
                    ..PolicySnapshot::default()
                },
                availability: SnapshotAvailability::default(),
            }
        });
    current_record.snapshot.commit = input.commit.clone();
    current_record.snapshot.pr_number = input.pr_number.clone();
    if current_record.snapshot.generated_at.trim().is_empty() {
        current_record.snapshot.generated_at = input.generated_at.clone();
    }

    add_count_unknowns(
        &mut unknowns,
        &current_record.availability,
        Some(input.current_path.clone()),
    );

    let history_records = history_values
        .iter()
        .enumerate()
        .filter_map(|(index, value)| {
            let source = value.get("current").unwrap_or(value);
            if string_path(source, &["current_policy_ceiling"]).is_none()
                && string_path(source, &["recommended_mode"]).is_none()
            {
                warnings.push(Notice {
                    kind: "history_shape_unsupported".to_string(),
                    message: format!(
                        "Policy history line {} does not contain a supported snapshot shape.",
                        index + 1
                    ),
                    source_artifact: input.history_path.clone(),
                });
                None
            } else {
                Some(snapshot_from_history_value(value))
            }
        })
        .collect::<Vec<_>>();
    let trend = build_trend(history_records.last(), &current_record);
    let history_summary = build_history_summary(&history_records, &current_record, &trend);

    let input_artifacts = vec![
        InputArtifact {
            kind: "policy_operations".to_string(),
            path: Some(input.current_path.clone()),
            status: artifact_status_label(current_artifact.status).to_string(),
        },
        InputArtifact {
            kind: "policy_history_jsonl".to_string(),
            path: input.history_path.clone(),
            status: artifact_status_label(history_status).to_string(),
        },
    ];

    PolicyHistoryReport {
        root: input.root,
        generated_at: input.generated_at,
        current: current_record.snapshot.clone(),
        history_summary,
        trend,
        example_append_record: current_record.snapshot,
        warnings,
        unknowns,
        input_artifacts,
    }
}

pub(crate) fn render_policy_history_json(report: &PolicyHistoryReport) -> Result<String, String> {
    serde_json::to_string_pretty(&json!({
        "schema_version": SCHEMA_VERSION,
        "tool": "ripr",
        "kind": REPORT_KIND,
        "root": report.root,
        "generated_at": report.generated_at,
        "current": snapshot_json(&report.current),
        "history_summary": history_summary_json(&report.history_summary),
        "trend": trend_json(&report.trend),
        "example_append_record": snapshot_json(&report.example_append_record),
        "warnings": report.warnings.iter().map(notice_json).collect::<Vec<_>>(),
        "unknowns": report.unknowns.iter().map(notice_json).collect::<Vec<_>>(),
        "input_artifacts": report.input_artifacts.iter().map(input_artifact_json).collect::<Vec<_>>(),
        "limits_note": LIMITS_NOTE,
    }))
    .map_err(|err| format!("failed to render policy history JSON: {err}"))
}

pub(crate) fn render_policy_history_markdown(report: &PolicyHistoryReport) -> String {
    let mut out = String::new();
    out.push_str("# RIPR Policy History\n\n");
    out.push_str(&format!(
        "Current ceiling: {}\n",
        report.current.current_policy_ceiling
    ));
    out.push_str(&format!(
        "Recommended mode: {}\n",
        report.current.recommended_mode
    ));
    out.push_str(&format!(
        "History entries: {}\n",
        report.history_summary.entries
    ));

    out.push_str("\n## Trend\n\n");
    out.push_str(&format!(
        "- Readiness: {}\n",
        report.trend.ceiling.direction
    ));
    out.push_str(&format!(
        "- Waiver pressure: {}\n",
        report.trend.waiver_count.direction
    ));
    out.push_str(&format!(
        "- Suppression health: {}\n",
        report.trend.stale_suppression_count.direction
    ));
    out.push_str(&format!(
        "- Baseline debt: {}\n",
        baseline_debt_direction(&report.trend)
    ));
    out.push_str(&format!(
        "- Preview boundary: {}\n",
        report.trend.preview_boundary_state.direction
    ));
    out.push_str(&format!(
        "- Calibration ceiling effect: {}\n",
        yes_no(report.history_summary.calibration_changed_ceiling)
    ));

    out.push_str("\n## Current Snapshot\n\n");
    out.push_str(&format!(
        "- commit: {}\n",
        report.current.commit.as_deref().unwrap_or("unknown")
    ));
    out.push_str(&format!(
        "- PR: {}\n",
        report.current.pr_number.as_deref().unwrap_or("unknown")
    ));
    out.push_str(&format!(
        "- new policy-eligible: {}\n",
        report.current.new_policy_eligible_count
    ));
    out.push_str(&format!("- waivers: {}\n", report.current.waiver_count));
    out.push_str(&format!(
        "- stale suppressions: {}\n",
        report.current.stale_suppression_count
    ));
    out.push_str(&format!(
        "- baseline still present: {}\n",
        report.current.baseline_still_present
    ));
    out.push_str(&format!(
        "- baseline resolved: {}\n",
        report.current.baseline_resolved
    ));

    out.push_str("\n## Input Artifacts\n\n");
    for artifact in &report.input_artifacts {
        out.push_str(&format!(
            "- {}: {}",
            artifact.kind.replace('_', "-"),
            artifact.status
        ));
        if let Some(path) = artifact.path.as_deref() {
            out.push_str(&format!(" ({path})"));
        }
        out.push('\n');
    }

    if !report.warnings.is_empty() {
        out.push_str("\n## Warnings\n\n");
        for warning in &report.warnings {
            out.push_str(&format!("- {}: {}\n", warning.kind, warning.message));
        }
    }
    if !report.unknowns.is_empty() {
        out.push_str("\n## Unknowns\n\n");
        for unknown in &report.unknowns {
            out.push_str(&format!("- {}: {}\n", unknown.kind, unknown.message));
        }
    }

    out.push_str("\n## Append Record\n\n");
    out.push_str(
        "The command may show this record for manual review, but it does not write history automatically.\n",
    );
    out.push_str("\nLimits:\n");
    out.push_str(LIMITS_NOTE);
    out.push('\n');
    out
}

pub(crate) fn policy_history_current_ceiling(report: &PolicyHistoryReport) -> &str {
    &report.current.current_policy_ceiling
}

pub(crate) fn policy_history_trend_direction(report: &PolicyHistoryReport) -> &str {
    &report.trend.ceiling.direction
}

pub(crate) use crate::output::path::display_path;

fn parse_required_json(
    kind: &str,
    path: &str,
    text: Result<String, String>,
    warnings: &mut Vec<Notice>,
) -> ParsedJson {
    let text = match text {
        Ok(text) => text,
        Err(error) => {
            let status = if looks_like_missing_file(&error) {
                ArtifactStatus::Missing
            } else {
                ArtifactStatus::Malformed
            };
            warnings.push(Notice {
                kind: format!("{kind}_unreadable"),
                message: format!("{kind} input {path} could not be read: {error}"),
                source_artifact: Some(path.to_string()),
            });
            return ParsedJson {
                status,
                value: None,
            };
        }
    };
    match serde_json::from_str::<Value>(&text) {
        Ok(value) => ParsedJson {
            status: ArtifactStatus::Read,
            value: Some(value),
        },
        Err(error) => {
            warnings.push(Notice {
                kind: format!("{kind}_malformed"),
                message: format!("{kind} input {path} is invalid JSON: {error}"),
                source_artifact: Some(path.to_string()),
            });
            ParsedJson {
                status: ArtifactStatus::Malformed,
                value: None,
            }
        }
    }
}

fn parse_history_jsonl(
    path: Option<&str>,
    text: &Option<Result<String, String>>,
    warnings: &mut Vec<Notice>,
    unknowns: &mut Vec<Notice>,
) -> (ArtifactStatus, Vec<Value>) {
    let Some(path) = path else {
        unknowns.push(Notice {
            kind: "history_not_supplied".to_string(),
            message:
                "No policy history JSONL was supplied; trend is limited to the current snapshot."
                    .to_string(),
            source_artifact: None,
        });
        return (ArtifactStatus::Omitted, Vec::new());
    };
    let Some(text) = text else {
        unknowns.push(Notice {
            kind: "history_not_loaded".to_string(),
            message: "Policy history path was supplied but no text was loaded.".to_string(),
            source_artifact: Some(path.to_string()),
        });
        return (ArtifactStatus::Missing, Vec::new());
    };
    let text = match text {
        Ok(text) => text,
        Err(error) => {
            let status = if looks_like_missing_file(error) {
                ArtifactStatus::Missing
            } else {
                ArtifactStatus::Malformed
            };
            if status == ArtifactStatus::Missing {
                unknowns.push(Notice {
                    kind: "history_not_supplied".to_string(),
                    message: "Policy history JSONL was not found; trend is limited to the current snapshot.".to_string(),
                    source_artifact: Some(path.to_string()),
                });
            } else {
                warnings.push(Notice {
                    kind: "history_unreadable".to_string(),
                    message: format!("Policy history input {path} could not be read: {error}"),
                    source_artifact: Some(path.to_string()),
                });
            }
            return (status, Vec::new());
        }
    };

    let mut values = Vec::new();
    let mut malformed = 0usize;
    for (index, line) in text.lines().enumerate() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        match serde_json::from_str::<Value>(trimmed) {
            Ok(value) => values.push(value),
            Err(error) => {
                malformed += 1;
                warnings.push(Notice {
                    kind: "history_line_malformed".to_string(),
                    message: format!("Policy history line {} is invalid JSON: {error}", index + 1),
                    source_artifact: Some(path.to_string()),
                });
            }
        }
    }
    if values.is_empty() && malformed > 0 {
        (ArtifactStatus::Malformed, values)
    } else {
        (ArtifactStatus::Read, values)
    }
}

fn looks_like_missing_file(error: &str) -> bool {
    error.contains("os error 2")
        || error.contains("No such file")
        || error.contains("cannot find the file")
}

fn snapshot_from_operations(value: &Value, input: &PolicyHistoryInput) -> SnapshotRecord {
    let ceiling = string_path(value, &["current_policy_ceiling"])
        .unwrap_or_else(|| "config_error".to_string());
    let recommended_mode = highest_safe_mode(value).unwrap_or_else(|| mode_for_ceiling(&ceiling));
    let blockers = value
        .get("promotion_blockers")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let input_artifacts = value
        .get("input_artifacts")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let current = value.get("current").unwrap_or(value);
    let snapshot = PolicySnapshot {
        commit: input.commit.clone(),
        pr_number: input.pr_number.clone(),
        generated_at: string_path(value, &["generated_at"])
            .unwrap_or_else(|| input.generated_at.clone()),
        recommended_mode,
        current_policy_ceiling: ceiling,
        baseline_health: health_from_inputs_and_blockers(
            &input_artifacts,
            &blockers,
            "baseline_delta",
            "baseline",
        ),
        waiver_health: health_from_inputs_and_blockers(
            &input_artifacts,
            &blockers,
            "waiver_aging",
            "waiver",
        ),
        suppression_health: health_from_inputs_and_blockers(
            &input_artifacts,
            &blockers,
            "suppression_health",
            "suppression",
        ),
        calibration_health: calibration_health(value, &input_artifacts, &blockers),
        preview_boundary_state: preview_boundary_state(&blockers),
        new_policy_eligible_count: usize_path(current, &["new_policy_eligible_count"]),
        waiver_count: usize_path(current, &["waiver_count"]),
        stale_suppression_count: usize_path(current, &["stale_suppression_count"]),
        baseline_still_present: usize_path(current, &["baseline_still_present"]),
        baseline_resolved: usize_path(current, &["baseline_resolved"]),
    };
    let availability = SnapshotAvailability {
        new_policy_eligible_count: path_value(current, &["new_policy_eligible_count"]).is_some(),
        waiver_count: path_value(current, &["waiver_count"]).is_some(),
        stale_suppression_count: path_value(current, &["stale_suppression_count"]).is_some(),
        baseline_still_present: path_value(current, &["baseline_still_present"]).is_some(),
        baseline_resolved: path_value(current, &["baseline_resolved"]).is_some(),
    };
    SnapshotRecord {
        snapshot,
        availability,
    }
}

fn snapshot_from_history_value(value: &Value) -> SnapshotRecord {
    let source = value.get("current").unwrap_or(value);
    let snapshot = PolicySnapshot {
        commit: string_path(source, &["commit"]),
        pr_number: string_path(source, &["pr_number"]),
        generated_at: string_path(source, &["generated_at"]).unwrap_or_default(),
        recommended_mode: string_path(source, &["recommended_mode"]).unwrap_or_else(|| {
            string_path(source, &["current_policy_ceiling"])
                .map(|ceiling| mode_for_ceiling(&ceiling))
                .unwrap_or_else(|| "advisory-only".to_string())
        }),
        current_policy_ceiling: string_path(source, &["current_policy_ceiling"])
            .unwrap_or_else(|| "advisory_only".to_string()),
        baseline_health: string_path(source, &["baseline_health"])
            .unwrap_or_else(|| "unknown".to_string()),
        waiver_health: string_path(source, &["waiver_health"])
            .unwrap_or_else(|| "unknown".to_string()),
        suppression_health: string_path(source, &["suppression_health"])
            .unwrap_or_else(|| "unknown".to_string()),
        calibration_health: string_path(source, &["calibration_health"])
            .unwrap_or_else(|| "unknown".to_string()),
        preview_boundary_state: string_path(source, &["preview_boundary_state"])
            .unwrap_or_else(|| "unknown".to_string()),
        new_policy_eligible_count: usize_path(source, &["new_policy_eligible_count"]),
        waiver_count: usize_path(source, &["waiver_count"]),
        stale_suppression_count: usize_path(source, &["stale_suppression_count"]),
        baseline_still_present: usize_path(source, &["baseline_still_present"]),
        baseline_resolved: usize_path(source, &["baseline_resolved"]),
    };
    let availability = SnapshotAvailability {
        new_policy_eligible_count: path_value(source, &["new_policy_eligible_count"]).is_some(),
        waiver_count: path_value(source, &["waiver_count"]).is_some(),
        stale_suppression_count: path_value(source, &["stale_suppression_count"]).is_some(),
        baseline_still_present: path_value(source, &["baseline_still_present"]).is_some(),
        baseline_resolved: path_value(source, &["baseline_resolved"]).is_some(),
    };
    SnapshotRecord {
        snapshot,
        availability,
    }
}

fn highest_safe_mode(value: &Value) -> Option<String> {
    value
        .get("safe_to_promote_to")
        .and_then(Value::as_array)
        .and_then(|items| {
            items
                .iter()
                .filter_map(|item| item.get("mode").and_then(Value::as_str))
                .max_by_key(|mode| mode_rank(mode))
                .map(ToString::to_string)
        })
}

fn health_from_inputs_and_blockers(
    input_artifacts: &[Value],
    blockers: &[Value],
    artifact_kind: &str,
    blocker_prefix: &str,
) -> String {
    if blockers
        .iter()
        .filter_map(|blocker| blocker.get("kind").and_then(Value::as_str))
        .any(|kind| kind.contains(blocker_prefix) && kind.contains("malformed"))
    {
        return "config_error".to_string();
    }
    if blockers
        .iter()
        .filter_map(|blocker| blocker.get("kind").and_then(Value::as_str))
        .any(|kind| kind.contains(blocker_prefix))
    {
        return "warning".to_string();
    }
    match artifact_status(input_artifacts, artifact_kind) {
        Some("read") => "healthy",
        Some("malformed") => "config_error",
        Some("missing" | "omitted") => "missing",
        _ => "unknown",
    }
    .to_string()
}

fn calibration_health(value: &Value, input_artifacts: &[Value], blockers: &[Value]) -> String {
    if blockers
        .iter()
        .filter_map(|blocker| blocker.get("kind").and_then(Value::as_str))
        .any(|kind| kind.contains("calibration") && kind.contains("malformed"))
    {
        return "config_error".to_string();
    }
    if value
        .get("safe_to_promote_to")
        .and_then(Value::as_array)
        .is_some_and(|items| {
            items
                .iter()
                .any(|item| item.get("mode").and_then(Value::as_str) == Some("calibrated-gate"))
        })
    {
        return "healthy".to_string();
    }
    match artifact_status(input_artifacts, "recommendation_calibration") {
        Some("read") => "warning",
        Some("malformed") => "config_error",
        Some("missing" | "omitted") => "not_ready",
        _ => "not_ready",
    }
    .to_string()
}

fn preview_boundary_state(blockers: &[Value]) -> String {
    let has_violation = blockers
        .iter()
        .filter_map(|blocker| blocker.get("kind").and_then(Value::as_str))
        .any(|kind| kind == "preview_boundary_violation");
    if has_violation {
        return "config_error".to_string();
    }
    let has_warning = blockers
        .iter()
        .filter_map(|blocker| blocker.get("kind").and_then(Value::as_str))
        .any(|kind| kind.starts_with("preview_"));
    if has_warning { "warning" } else { "healthy" }.to_string()
}

fn artifact_status<'a>(input_artifacts: &'a [Value], kind: &str) -> Option<&'a str> {
    input_artifacts
        .iter()
        .find(|artifact| artifact.get("kind").and_then(Value::as_str) == Some(kind))
        .and_then(|artifact| artifact.get("status").and_then(Value::as_str))
}

fn add_count_unknowns(
    unknowns: &mut Vec<Notice>,
    availability: &SnapshotAvailability,
    source_artifact: Option<String>,
) {
    for (available, kind, message) in [
        (
            availability.new_policy_eligible_count,
            "new_policy_eligible_count_unavailable",
            "New policy-eligible count was not available in policy operations.",
        ),
        (
            availability.waiver_count,
            "waiver_count_unavailable",
            "Waiver count was not available in policy operations.",
        ),
        (
            availability.stale_suppression_count,
            "stale_suppression_count_unavailable",
            "Stale suppression count was not available in policy operations.",
        ),
        (
            availability.baseline_still_present,
            "baseline_still_present_unavailable",
            "Baseline still-present count was not available in policy operations.",
        ),
        (
            availability.baseline_resolved,
            "baseline_resolved_unavailable",
            "Baseline resolved count was not available in policy operations.",
        ),
    ] {
        if !available {
            unknowns.push(Notice {
                kind: kind.to_string(),
                message: message.to_string(),
                source_artifact: source_artifact.clone(),
            });
        }
    }
}

fn build_history_summary(
    history: &[SnapshotRecord],
    current: &SnapshotRecord,
    trend: &PolicyTrend,
) -> HistorySummary {
    HistorySummary {
        entries: history.len() + 1,
        oldest_generated_at: history
            .first()
            .map(|record| record.snapshot.generated_at.clone())
            .filter(|value| !value.is_empty())
            .or_else(|| Some(current.snapshot.generated_at.clone())),
        newest_generated_at: Some(current.snapshot.generated_at.clone()),
        readiness_improved: trend.ceiling.direction == "improved",
        waiver_pressure_increased: trend.waiver_count.direction == "regressed",
        suppression_health_regressed: trend.stale_suppression_count.direction == "regressed",
        baseline_shrank: trend.baseline_still_present.direction == "improved"
            || trend.baseline_resolved.direction == "improved",
        preview_remained_advisory: current.snapshot.preview_boundary_state != "config_error"
            && history
                .iter()
                .all(|record| record.snapshot.preview_boundary_state != "config_error"),
        calibration_changed_ceiling: trend.calibration_health.direction == "improved"
            && trend.ceiling.direction == "improved",
    }
}

fn build_trend(previous: Option<&SnapshotRecord>, current: &SnapshotRecord) -> PolicyTrend {
    let previous = previous.unwrap_or(current);
    let single_snapshot = std::ptr::eq(previous, current);
    PolicyTrend {
        ceiling: trend_ranked_text(
            (!single_snapshot).then_some(previous.snapshot.current_policy_ceiling.as_str()),
            current.snapshot.current_policy_ceiling.as_str(),
            ceiling_rank,
        ),
        waiver_count: trend_count(
            (!single_snapshot && previous.availability.waiver_count)
                .then_some(previous.snapshot.waiver_count),
            current
                .availability
                .waiver_count
                .then_some(current.snapshot.waiver_count),
            CountDirection::LowerIsBetter,
        ),
        stale_suppression_count: trend_count(
            (!single_snapshot && previous.availability.stale_suppression_count)
                .then_some(previous.snapshot.stale_suppression_count),
            current
                .availability
                .stale_suppression_count
                .then_some(current.snapshot.stale_suppression_count),
            CountDirection::LowerIsBetter,
        ),
        baseline_still_present: trend_count(
            (!single_snapshot && previous.availability.baseline_still_present)
                .then_some(previous.snapshot.baseline_still_present),
            current
                .availability
                .baseline_still_present
                .then_some(current.snapshot.baseline_still_present),
            CountDirection::LowerIsBetter,
        ),
        baseline_resolved: trend_count(
            (!single_snapshot && previous.availability.baseline_resolved)
                .then_some(previous.snapshot.baseline_resolved),
            current
                .availability
                .baseline_resolved
                .then_some(current.snapshot.baseline_resolved),
            CountDirection::HigherIsBetter,
        ),
        preview_boundary_state: trend_ranked_text(
            (!single_snapshot).then_some(previous.snapshot.preview_boundary_state.as_str()),
            current.snapshot.preview_boundary_state.as_str(),
            health_rank,
        ),
        calibration_health: trend_ranked_text(
            (!single_snapshot).then_some(previous.snapshot.calibration_health.as_str()),
            current.snapshot.calibration_health.as_str(),
            health_rank,
        ),
    }
}

#[derive(Clone, Copy)]
enum CountDirection {
    LowerIsBetter,
    HigherIsBetter,
}

fn trend_count(
    previous: Option<usize>,
    current: Option<usize>,
    direction: CountDirection,
) -> TrendValue {
    let direction_label = match (previous, current) {
        (Some(previous), Some(current)) if previous == current => "unchanged",
        (Some(previous), Some(current)) => {
            let improved = match direction {
                CountDirection::LowerIsBetter => current < previous,
                CountDirection::HigherIsBetter => current > previous,
            };
            if improved { "improved" } else { "regressed" }
        }
        _ => "unknown",
    };
    TrendValue {
        previous: previous.map(|value| value.to_string()),
        current: current.map(|value| value.to_string()),
        direction: direction_label.to_string(),
    }
}

fn trend_ranked_text(previous: Option<&str>, current: &str, rank: fn(&str) -> usize) -> TrendValue {
    let direction = match previous {
        Some(previous) if previous == current => "unchanged",
        Some(previous) if rank(current) > rank(previous) => "improved",
        Some(previous) if rank(current) < rank(previous) => "regressed",
        Some(_) => "unchanged",
        None => "unknown",
    };
    TrendValue {
        previous: previous.map(ToString::to_string),
        current: Some(current.to_string()),
        direction: direction.to_string(),
    }
}

fn ceiling_rank(value: &str) -> usize {
    match value {
        "ready_for_calibrated_gate" => 4,
        "ready_for_baseline_check" => 3,
        "ready_for_acknowledgeable" => 2,
        "ready_for_visible_only" => 1,
        _ => 0,
    }
}

fn health_rank(value: &str) -> usize {
    match value {
        "healthy" => 4,
        "advisory" => 3,
        "warning" => 2,
        "not_ready" | "missing" | "unknown" => 1,
        "config_error" => 0,
        _ => 1,
    }
}

fn mode_rank(mode: &str) -> usize {
    match mode {
        "calibrated-gate" => 4,
        "baseline-check" => 3,
        "acknowledgeable" => 2,
        "visible-only" => 1,
        _ => 0,
    }
}

fn mode_for_ceiling(ceiling: &str) -> String {
    match ceiling {
        "ready_for_calibrated_gate" => "calibrated-gate",
        "ready_for_baseline_check" => "baseline-check",
        "ready_for_acknowledgeable" => "acknowledgeable",
        "ready_for_visible_only" => "visible-only",
        _ => "advisory-only",
    }
    .to_string()
}

fn baseline_debt_direction(trend: &PolicyTrend) -> &str {
    if trend.baseline_still_present.direction == "improved"
        || trend.baseline_resolved.direction == "improved"
    {
        "improved"
    } else if trend.baseline_still_present.direction == "regressed"
        || trend.baseline_resolved.direction == "regressed"
    {
        "regressed"
    } else if trend.baseline_still_present.direction == "unknown"
        || trend.baseline_resolved.direction == "unknown"
    {
        "unknown"
    } else {
        "unchanged"
    }
}

fn yes_no(value: bool) -> &'static str {
    if value { "yes" } else { "no" }
}

fn snapshot_json(snapshot: &PolicySnapshot) -> Value {
    json!({
        "commit": snapshot.commit,
        "pr_number": snapshot.pr_number,
        "generated_at": snapshot.generated_at,
        "recommended_mode": snapshot.recommended_mode,
        "current_policy_ceiling": snapshot.current_policy_ceiling,
        "baseline_health": snapshot.baseline_health,
        "waiver_health": snapshot.waiver_health,
        "suppression_health": snapshot.suppression_health,
        "calibration_health": snapshot.calibration_health,
        "preview_boundary_state": snapshot.preview_boundary_state,
        "new_policy_eligible_count": snapshot.new_policy_eligible_count,
        "waiver_count": snapshot.waiver_count,
        "stale_suppression_count": snapshot.stale_suppression_count,
        "baseline_still_present": snapshot.baseline_still_present,
        "baseline_resolved": snapshot.baseline_resolved,
    })
}

fn history_summary_json(summary: &HistorySummary) -> Value {
    json!({
        "entries": summary.entries,
        "oldest_generated_at": summary.oldest_generated_at,
        "newest_generated_at": summary.newest_generated_at,
        "readiness_improved": summary.readiness_improved,
        "waiver_pressure_increased": summary.waiver_pressure_increased,
        "suppression_health_regressed": summary.suppression_health_regressed,
        "baseline_shrank": summary.baseline_shrank,
        "preview_remained_advisory": summary.preview_remained_advisory,
        "calibration_changed_ceiling": summary.calibration_changed_ceiling,
    })
}

fn trend_json(trend: &PolicyTrend) -> Value {
    json!({
        "ceiling": trend_value_json(&trend.ceiling),
        "waiver_count": trend_value_json(&trend.waiver_count),
        "stale_suppression_count": trend_value_json(&trend.stale_suppression_count),
        "baseline_still_present": trend_value_json(&trend.baseline_still_present),
        "baseline_resolved": trend_value_json(&trend.baseline_resolved),
        "preview_boundary_state": trend_value_json(&trend.preview_boundary_state),
        "calibration_health": trend_value_json(&trend.calibration_health),
    })
}

fn trend_value_json(trend: &TrendValue) -> Value {
    json!({
        "previous": trend.previous,
        "current": trend.current,
        "direction": trend.direction,
    })
}

fn notice_json(notice: &Notice) -> Value {
    json!({
        "kind": notice.kind,
        "message": notice.message,
        "source_artifact": notice.source_artifact,
    })
}

fn input_artifact_json(artifact: &InputArtifact) -> Value {
    json!({
        "kind": artifact.kind,
        "path": artifact.path,
        "status": artifact.status,
    })
}

fn artifact_status_label(status: ArtifactStatus) -> &'static str {
    match status {
        ArtifactStatus::Read => "read",
        ArtifactStatus::Missing => "missing",
        ArtifactStatus::Malformed => "malformed",
        ArtifactStatus::Omitted => "omitted",
    }
}

fn string_path(value: &Value, path: &[&str]) -> Option<String> {
    path_value(value, path).and_then(|value| value.as_str().map(ToString::to_string))
}

fn usize_path(value: &Value, path: &[&str]) -> usize {
    path_value(value, path)
        .and_then(Value::as_u64)
        .and_then(|value| usize::try_from(value).ok())
        .unwrap_or(0)
}

fn path_value<'a>(value: &'a Value, path: &[&str]) -> Option<&'a Value> {
    let mut current = value;
    for key in path {
        current = current.get(*key)?;
    }
    Some(current)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn input(current: &str) -> PolicyHistoryInput {
        PolicyHistoryInput {
            root: ".".to_string(),
            generated_at: "unix_ms:10".to_string(),
            current_path: "policy-operations.json".to_string(),
            history_path: None,
            commit: Some("HEAD".to_string()),
            pr_number: Some("123".to_string()),
            current_json: Ok(current.to_string()),
            history_jsonl: None,
        }
    }

    fn operations(ceiling: &str, safe_modes: &[&str]) -> String {
        operations_with_current(
            ceiling,
            safe_modes,
            r#""current": {
                "new_policy_eligible_count": 1,
                "waiver_count": 2,
                "stale_suppression_count": 0,
                "baseline_still_present": 4,
                "baseline_resolved": 1
              }"#,
        )
    }

    fn operations_with_current(ceiling: &str, safe_modes: &[&str], current: &str) -> String {
        let safe = safe_modes
            .iter()
            .map(|mode| {
                format!(
                    r#"{{"mode":"{mode}","allowed_now":true,"reason":"ok","source_artifacts":["policy-readiness.json"]}}"#
                )
            })
            .collect::<Vec<_>>()
            .join(",");
        format!(
            r#"{{
              "schema_version": "0.1",
              "kind": "policy_operations",
              "generated_at": "unix_ms:10",
              "current_policy_ceiling": "{ceiling}",
              "safe_to_promote_to": [{safe}],
              "not_safe_to_promote_to": [],
              "promotion_blockers": [],
              "input_artifacts": [
                {{"kind":"baseline_delta","path":"baseline.json","status":"read"}},
                {{"kind":"waiver_aging","path":"waiver.json","status":"read"}},
                {{"kind":"suppression_health","path":"suppression.json","status":"read"}},
                {{"kind":"recommendation_calibration","path":"recommendation.json","status":"omitted"}}
              ],
              {current}
            }}"#
        )
    }

    #[test]
    fn policy_history_single_snapshot_marks_trends_unknown() {
        let report = build_policy_history_report(input(&operations(
            "ready_for_acknowledgeable",
            &["visible-only", "acknowledgeable"],
        )));

        assert_eq!(
            report.current.current_policy_ceiling,
            "ready_for_acknowledgeable"
        );
        assert_eq!(report.current.recommended_mode, "acknowledgeable");
        assert_eq!(report.history_summary.entries, 1);
        assert_eq!(report.trend.ceiling.direction, "unknown");
        assert!(
            report
                .unknowns
                .iter()
                .any(|unknown| unknown.kind == "history_not_supplied")
        );
    }

    #[test]
    fn policy_history_detects_readiness_improvement() {
        let mut input = input(&operations(
            "ready_for_acknowledgeable",
            &["visible-only", "acknowledgeable"],
        ));
        input.history_path = Some(".ripr/policy-history.jsonl".to_string());
        input.history_jsonl = Some(Ok(
            r#"{"generated_at":"unix_ms:1","current_policy_ceiling":"ready_for_visible_only","recommended_mode":"visible-only","baseline_health":"healthy","waiver_health":"healthy","suppression_health":"healthy","calibration_health":"not_ready","preview_boundary_state":"healthy","new_policy_eligible_count":1,"waiver_count":2,"stale_suppression_count":0,"baseline_still_present":5,"baseline_resolved":0}"#
                .to_string(),
        ));

        let report = build_policy_history_report(input);

        assert!(report.history_summary.readiness_improved);
        assert_eq!(
            report.trend.ceiling.previous.as_deref(),
            Some("ready_for_visible_only")
        );
        assert_eq!(report.trend.ceiling.direction, "improved");
    }

    #[test]
    fn policy_history_detects_waiver_pressure_regression() {
        let mut input = input(&operations(
            "ready_for_acknowledgeable",
            &["visible-only", "acknowledgeable"],
        ));
        input.history_path = Some(".ripr/policy-history.jsonl".to_string());
        input.history_jsonl = Some(Ok(
            r#"{"generated_at":"unix_ms:1","current_policy_ceiling":"ready_for_acknowledgeable","recommended_mode":"acknowledgeable","baseline_health":"healthy","waiver_health":"healthy","suppression_health":"healthy","calibration_health":"not_ready","preview_boundary_state":"healthy","new_policy_eligible_count":1,"waiver_count":1,"stale_suppression_count":0,"baseline_still_present":4,"baseline_resolved":1}"#
                .to_string(),
        ));

        let report = build_policy_history_report(input);

        assert!(report.history_summary.waiver_pressure_increased);
        assert_eq!(report.trend.waiver_count.direction, "regressed");
    }

    #[test]
    fn policy_history_detects_baseline_shrink() {
        let mut input = input(&operations(
            "ready_for_acknowledgeable",
            &["visible-only", "acknowledgeable"],
        ));
        input.history_path = Some(".ripr/policy-history.jsonl".to_string());
        input.history_jsonl = Some(Ok(
            r#"{"generated_at":"unix_ms:1","current_policy_ceiling":"ready_for_acknowledgeable","recommended_mode":"acknowledgeable","baseline_health":"healthy","waiver_health":"healthy","suppression_health":"healthy","calibration_health":"not_ready","preview_boundary_state":"healthy","new_policy_eligible_count":1,"waiver_count":2,"stale_suppression_count":0,"baseline_still_present":5,"baseline_resolved":0}"#
                .to_string(),
        ));

        let report = build_policy_history_report(input);

        assert!(report.history_summary.baseline_shrank);
        assert_eq!(report.trend.baseline_still_present.direction, "improved");
        assert_eq!(report.trend.baseline_resolved.direction, "improved");
    }

    #[test]
    fn policy_history_marks_preview_boundary_still_advisory() {
        let mut input = input(&operations(
            "ready_for_acknowledgeable",
            &["visible-only", "acknowledgeable"],
        ));
        input.history_path = Some(".ripr/policy-history.jsonl".to_string());
        input.history_jsonl = Some(Ok(
            r#"{"generated_at":"unix_ms:1","current_policy_ceiling":"ready_for_acknowledgeable","recommended_mode":"acknowledgeable","baseline_health":"healthy","waiver_health":"healthy","suppression_health":"healthy","calibration_health":"not_ready","preview_boundary_state":"healthy","new_policy_eligible_count":1,"waiver_count":2,"stale_suppression_count":0,"baseline_still_present":4,"baseline_resolved":1}"#
                .to_string(),
        ));

        let report = build_policy_history_report(input);

        assert!(report.history_summary.preview_remained_advisory);
        assert_eq!(report.trend.preview_boundary_state.direction, "unchanged");
    }

    #[test]
    fn policy_history_keeps_usable_history_with_malformed_line_warning() {
        let mut input = input(&operations("ready_for_visible_only", &["visible-only"]));
        input.history_path = Some(".ripr/policy-history.jsonl".to_string());
        input.history_jsonl = Some(Ok(format!(
            "{}\nnot-json\n",
            r#"{"generated_at":"unix_ms:1","current_policy_ceiling":"advisory_only","recommended_mode":"advisory-only","new_policy_eligible_count":1,"waiver_count":2,"stale_suppression_count":0,"baseline_still_present":5,"baseline_resolved":0}"#
        )));

        let report = build_policy_history_report(input);

        assert_eq!(report.input_artifacts[1].status, "read");
        assert!(
            report
                .warnings
                .iter()
                .any(|warning| warning.kind == "history_line_malformed")
        );
        assert_eq!(report.trend.ceiling.direction, "improved");
    }

    #[test]
    fn policy_history_marks_preview_boundary_violation_not_advisory() {
        let mut current = operations("ready_for_baseline_check", &["visible-only"]);
        current = current.replace(
            r#""promotion_blockers": []"#,
            r#""promotion_blockers": [{"kind":"preview_boundary_violation","severity":"config_error","message":"preview promoted","target_modes":["baseline-check"],"source_artifact":"policy-readiness.json","repair_action":"keep advisory"}]"#,
        );

        let report = build_policy_history_report(input(&current));

        assert_eq!(report.current.preview_boundary_state, "config_error");
        assert!(!report.history_summary.preview_remained_advisory);
    }

    #[test]
    fn policy_history_malformed_current_is_config_error_snapshot() {
        let report = build_policy_history_report(PolicyHistoryInput {
            root: ".".to_string(),
            generated_at: "unix_ms:10".to_string(),
            current_path: "policy-operations.json".to_string(),
            history_path: None,
            commit: None,
            pr_number: None,
            current_json: Ok("not-json".to_string()),
            history_jsonl: None,
        });

        assert_eq!(report.current.current_policy_ceiling, "config_error");
        assert_eq!(report.input_artifacts[0].status, "malformed");
        assert!(
            report
                .warnings
                .iter()
                .any(|warning| warning.kind == "policy_operations_malformed")
        );
    }

    #[test]
    fn policy_history_reports_unavailable_counts_as_unknowns() {
        let report = build_policy_history_report(input(&operations_with_current(
            "ready_for_acknowledgeable",
            &["visible-only", "acknowledgeable"],
            r#""current": {}"#,
        )));

        assert!(
            report
                .unknowns
                .iter()
                .any(|unknown| unknown.kind == "waiver_count_unavailable")
        );
        assert_eq!(report.trend.waiver_count.direction, "unknown");
    }

    #[test]
    fn policy_history_json_and_markdown_are_structured() -> Result<(), String> {
        let report = build_policy_history_report(input(&operations(
            "ready_for_acknowledgeable",
            &["visible-only", "acknowledgeable"],
        )));

        let json = render_policy_history_json(&report)?;
        let markdown = render_policy_history_markdown(&report);

        assert!(json.contains("\"kind\": \"policy_history\""));
        assert!(json.contains("\"current_policy_ceiling\": \"ready_for_acknowledgeable\""));
        assert!(json.contains("\"example_append_record\""));
        assert!(markdown.contains("# RIPR Policy History"));
        assert!(markdown.contains("The command may show this record for manual review"));
        Ok(())
    }

    #[test]
    fn policy_history_marks_missing_history_jsonl_as_missing_artifact() {
        // History path supplied + read error that looks like
        // "No such file": the artifact is reported as `missing`, and
        // an `history_not_supplied` unknown notice is appended.
        let mut input = input(&operations("ready_for_visible_only", &["visible-only"]));
        input.history_path = Some(".ripr/policy-history.jsonl".to_string());
        input.history_jsonl = Some(Err("No such file or directory (os error 2)".to_string()));

        let report = build_policy_history_report(input);

        let history_artifact = report
            .input_artifacts
            .iter()
            .find(|artifact| artifact.kind == "policy_history_jsonl");
        assert!(
            history_artifact.is_some(),
            "expected policy_history_jsonl artifact"
        );
        assert_eq!(history_artifact.map(|a| a.status.as_str()), Some("missing"));
        assert!(
            report
                .unknowns
                .iter()
                .any(|notice| notice.kind == "history_not_supplied"),
            "expected history_not_supplied notice, got {:?}",
            report.unknowns
        );
    }

    #[test]
    fn policy_history_records_warning_for_non_missing_history_read_error() {
        // History path supplied + read error that does not look like
        // missing-file: the artifact is reported as `malformed` and
        // a `history_unreadable` warning is appended.
        let mut input = input(&operations("ready_for_visible_only", &["visible-only"]));
        input.history_path = Some(".ripr/policy-history.jsonl".to_string());
        input.history_jsonl = Some(Err("permission denied".to_string()));

        let report = build_policy_history_report(input);

        let history_artifact = report
            .input_artifacts
            .iter()
            .find(|artifact| artifact.kind == "policy_history_jsonl");
        assert_eq!(
            history_artifact.map(|a| a.status.as_str()),
            Some("malformed")
        );
        assert!(
            report
                .warnings
                .iter()
                .any(|notice| notice.kind == "history_unreadable"),
            "expected history_unreadable warning, got {:?}",
            report.warnings
        );
    }

    #[test]
    fn policy_history_records_unknown_when_history_supplied_without_text() {
        // History path supplied but `history_jsonl` is `None`: this
        // hits the "supplied but not loaded" branch in
        // `parse_history_jsonl` and routes to a `history_not_loaded`
        // unknown notice with the artifact marked `missing`.
        let mut input = input(&operations("ready_for_visible_only", &["visible-only"]));
        input.history_path = Some(".ripr/policy-history.jsonl".to_string());
        input.history_jsonl = None;

        let report = build_policy_history_report(input);

        let history_artifact = report
            .input_artifacts
            .iter()
            .find(|artifact| artifact.kind == "policy_history_jsonl");
        assert_eq!(history_artifact.map(|a| a.status.as_str()), Some("missing"));
        assert!(
            report
                .unknowns
                .iter()
                .any(|notice| notice.kind == "history_not_loaded"),
            "expected history_not_loaded notice, got {:?}",
            report.unknowns
        );
    }

    #[test]
    fn policy_history_records_warning_for_unreadable_current_operations() {
        // `current_json` is an `Err`: parse_required_json appends a
        // `policy_operations_unreadable` warning and the snapshot
        // falls back to `config_error` ceiling.
        let report = build_policy_history_report(PolicyHistoryInput {
            root: ".".to_string(),
            generated_at: "unix_ms:10".to_string(),
            current_path: "policy-operations.json".to_string(),
            history_path: None,
            commit: Some("HEAD".to_string()),
            pr_number: Some("123".to_string()),
            current_json: Err("permission denied".to_string()),
            history_jsonl: None,
        });

        assert!(
            report
                .warnings
                .iter()
                .any(|warning| warning.kind == "policy_operations_unreadable"),
            "expected policy_operations_unreadable warning, got {:?}",
            report.warnings
        );
        assert_eq!(report.current.current_policy_ceiling, "config_error");
        assert_eq!(report.input_artifacts[0].status, "malformed");
    }

    #[test]
    fn policy_history_baseline_health_flips_to_config_error_on_malformed_blocker() {
        // `promotion_blockers` carries a `*_malformed` kind matching
        // the baseline prefix: `health_from_inputs_and_blockers`
        // returns `config_error` for the baseline health field.
        let mut operations = operations("ready_for_visible_only", &["visible-only"]);
        operations = operations.replace(
            r#""promotion_blockers": []"#,
            r#""promotion_blockers": [{"kind":"baseline_delta_malformed","severity":"config_error","message":"baseline malformed","target_modes":["baseline-check"],"source_artifact":"baseline.json","repair_action":"fix it"}]"#,
        );

        let report = build_policy_history_report(input(&operations));

        assert_eq!(report.current.baseline_health, "config_error");
    }

    #[test]
    fn policy_history_waiver_health_flips_to_warning_on_matching_blocker() {
        // A non-malformed waiver_aging blocker should map to `warning`.
        let mut operations = operations("ready_for_visible_only", &["visible-only"]);
        operations = operations.replace(
            r#""promotion_blockers": []"#,
            r#""promotion_blockers": [{"kind":"waiver_aging_threshold","severity":"warning","message":"too many old waivers","target_modes":["baseline-check"],"source_artifact":"waiver.json","repair_action":"resolve waivers"}]"#,
        );

        let report = build_policy_history_report(input(&operations));

        assert_eq!(report.current.waiver_health, "warning");
    }

    #[test]
    fn policy_history_preview_boundary_state_warns_on_non_violation_preview_blocker() {
        // A `preview_*` blocker that is not the exact
        // `preview_boundary_violation` kind maps to `warning` rather
        // than `config_error`.
        let mut operations = operations("ready_for_visible_only", &["visible-only"]);
        operations = operations.replace(
            r#""promotion_blockers": []"#,
            r#""promotion_blockers": [{"kind":"preview_pending_calibration","severity":"warning","message":"awaiting calibration","target_modes":["calibrated-gate"],"source_artifact":"policy-readiness.json","repair_action":"wait"}]"#,
        );

        let report = build_policy_history_report(input(&operations));

        assert_eq!(report.current.preview_boundary_state, "warning");
    }

    #[test]
    fn mode_for_ceiling_maps_known_ceilings_and_falls_back_to_advisory_only() {
        assert_eq!(
            mode_for_ceiling("ready_for_calibrated_gate"),
            "calibrated-gate"
        );
        assert_eq!(
            mode_for_ceiling("ready_for_baseline_check"),
            "baseline-check"
        );
        assert_eq!(
            mode_for_ceiling("ready_for_acknowledgeable"),
            "acknowledgeable"
        );
        assert_eq!(mode_for_ceiling("ready_for_visible_only"), "visible-only");
        assert_eq!(mode_for_ceiling("anything_else"), "advisory-only");
    }

    #[test]
    fn policy_history_recommended_mode_falls_back_from_ceiling_on_history_record_without_mode() {
        // The history record omits `recommended_mode` but supplies
        // `current_policy_ceiling`. `snapshot_from_history_value`
        // routes through `mode_for_ceiling` for the previous trend.
        //
        // The direct snapshot assertion below pins the fallback output
        // itself (no `recommended_mode` -> `mode_for_ceiling(ceiling)`),
        // so the branch cannot regress silently behind the trend assertions.
        let history_value = json!({
            "generated_at": "unix_ms:1",
            "current_policy_ceiling": "ready_for_baseline_check",
        });
        let snapshot = snapshot_from_history_value(&history_value);
        assert_eq!(snapshot.snapshot.recommended_mode, "baseline-check");

        let mut input = input(&operations(
            "ready_for_calibrated_gate",
            &[
                "visible-only",
                "acknowledgeable",
                "baseline-check",
                "calibrated-gate",
            ],
        ));
        input.history_path = Some(".ripr/policy-history.jsonl".to_string());
        input.history_jsonl = Some(Ok(
            r#"{"generated_at":"unix_ms:1","current_policy_ceiling":"ready_for_baseline_check","new_policy_eligible_count":0,"waiver_count":0,"stale_suppression_count":0,"baseline_still_present":0,"baseline_resolved":0}"#
                .to_string(),
        ));

        let report = build_policy_history_report(input);

        assert_eq!(
            report.trend.ceiling.previous.as_deref(),
            Some("ready_for_baseline_check")
        );
        assert_eq!(report.trend.ceiling.direction, "improved");
    }

    #[test]
    fn policy_history_markdown_includes_warnings_section_when_warnings_exist() {
        // Use the malformed-history-line case to seed a warning, then
        // confirm the markdown renders the `## Warnings` section that
        // is otherwise omitted when `report.warnings.is_empty()`.
        let mut input = input(&operations("ready_for_visible_only", &["visible-only"]));
        input.history_path = Some(".ripr/policy-history.jsonl".to_string());
        input.history_jsonl = Some(Ok(format!(
            "{}\nnot-json\n",
            r#"{"generated_at":"unix_ms:1","current_policy_ceiling":"advisory_only","recommended_mode":"advisory-only","new_policy_eligible_count":1,"waiver_count":2,"stale_suppression_count":0,"baseline_still_present":5,"baseline_resolved":0}"#
        )));

        let report = build_policy_history_report(input);
        let markdown = render_policy_history_markdown(&report);

        assert!(
            markdown.contains("\n## Warnings\n"),
            "expected `## Warnings` section in markdown, got:\n{markdown}"
        );
        assert!(
            markdown.contains("history_line_malformed"),
            "expected the warning kind to appear in the section, got:\n{markdown}"
        );
    }
}
