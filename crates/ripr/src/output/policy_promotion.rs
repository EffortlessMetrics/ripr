use serde_json::{Value, json};
use std::collections::BTreeSet;
use std::path::Path;

const SCHEMA_VERSION: &str = "0.1";
const REPORT_KIND: &str = "policy_promotion_packet";
const LIMITS_NOTE: &str = "Read-only advisory promotion packet. It supports manual review only and never mutates policy configuration, baselines, suppressions, workflows, branch protection, CI defaults, history, or preview-language eligibility.";

const TARGET_MODES: [&str; 4] = [
    "visible-only",
    "acknowledgeable",
    "baseline-check",
    "calibrated-gate",
];

const NON_GOALS: [&str; 15] = [
    "No automatic config mutation.",
    "No automatic baseline adoption.",
    "No baseline mutation.",
    "No suppression creation.",
    "No workflow mutation.",
    "No branch-protection mutation.",
    "No generated CI mutation.",
    "No default CI blocking.",
    "No gate decision.",
    "No analyzer behavior changes.",
    "No evidence identity rewrites.",
    "No generated tests.",
    "No provider calls.",
    "No mutation execution.",
    "No preview-language promotion.",
];

pub(crate) fn default_policy_promotion_out(target_mode: &str) -> String {
    format!("target/ripr/reports/policy-promotion-{target_mode}.json")
}

pub(crate) fn default_policy_promotion_md_out(target_mode: &str) -> String {
    format!("target/ripr/reports/policy-promotion-{target_mode}.md")
}

pub(crate) fn is_supported_target_mode(target_mode: &str) -> bool {
    TARGET_MODES.contains(&target_mode)
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PolicyPromotionInput {
    pub(crate) root: String,
    pub(crate) generated_at: String,
    pub(crate) target_mode: String,
    pub(crate) operations_path: String,
    pub(crate) history_path: Option<String>,
    pub(crate) operations_json: Result<String, String>,
    pub(crate) history_json: Option<Result<String, String>>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PolicyPromotionReport {
    root: String,
    generated_at: String,
    target_mode: String,
    allowed_now: bool,
    why_or_why_not: String,
    required_repairs: Vec<String>,
    required_receipts: Vec<String>,
    rollback_path: Vec<String>,
    example_config_change: ExampleConfigChange,
    input_artifacts: Vec<InputArtifact>,
    warnings: Vec<Notice>,
    unknowns: Vec<Notice>,
    non_goals: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ExampleConfigChange {
    file: String,
    change: String,
    manual_only: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct InputArtifact {
    kind: String,
    path: Option<String>,
    status: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct Notice {
    kind: String,
    message: String,
    source_artifact: Option<String>,
}

#[derive(Clone, Debug)]
struct ParsedArtifact {
    kind: &'static str,
    path: Option<String>,
    status: &'static str,
    value: Option<Value>,
}

pub(crate) fn build_policy_promotion_report(input: PolicyPromotionInput) -> PolicyPromotionReport {
    let operations = parse_required_json(
        "policy_operations",
        input.operations_path.clone(),
        input.operations_json,
    );
    let history = parse_optional_json(
        "policy_history",
        input.history_path.clone(),
        input.history_json,
    );

    let mut warnings = Vec::new();
    let mut unknowns = Vec::new();
    collect_artifact_notices(&operations, true, &mut warnings, &mut unknowns);
    collect_artifact_notices(&history, false, &mut warnings, &mut unknowns);

    let operations_value = operations.value.as_ref();
    copy_notices(
        operations_value,
        "warnings",
        "policy_operations_warning",
        &mut warnings,
    );
    copy_notices(
        operations_value,
        "unknowns",
        "policy_operations_unknown",
        &mut unknowns,
    );

    if input.target_mode == "calibrated-gate" {
        warnings.push(Notice {
            kind: "calibrated_gate_stable_rust_only".to_string(),
            message: "Manual calibrated-gate promotion is limited to eligible stable Rust classes with same-class calibration.".to_string(),
            source_artifact: operations.path.clone(),
        });
    }

    let allowed_now = operations_value
        .and_then(|value| assessment_for(value, "safe_to_promote_to", &input.target_mode))
        .is_some();
    let blockers = operations_value
        .map(|value| blockers_for_target(value, &input.target_mode))
        .unwrap_or_default();
    let blocked_assessment = operations_value
        .and_then(|value| assessment_for(value, "not_safe_to_promote_to", &input.target_mode));
    let safe_assessment = operations_value
        .and_then(|value| assessment_for(value, "safe_to_promote_to", &input.target_mode));

    let why_or_why_not = explain_promotion(
        &input.target_mode,
        allowed_now,
        &operations,
        safe_assessment,
        blocked_assessment,
        &blockers,
    );
    let required_repairs = required_repairs(
        &input.target_mode,
        allowed_now,
        operations_value,
        blocked_assessment,
        &blockers,
    );
    let required_receipts = required_receipts(
        &input.target_mode,
        allowed_now,
        &operations,
        history.status == "read",
    );

    PolicyPromotionReport {
        root: input.root,
        generated_at: input.generated_at,
        target_mode: input.target_mode.clone(),
        allowed_now,
        why_or_why_not,
        required_repairs,
        required_receipts,
        rollback_path: rollback_path(&input.target_mode),
        example_config_change: ExampleConfigChange {
            file: "ripr.toml".to_string(),
            change: format!(
                "Set the reviewed policy gate mode to {}.",
                input.target_mode
            ),
            manual_only: true,
        },
        input_artifacts: vec![input_artifact(&operations), input_artifact(&history)],
        warnings,
        unknowns,
        non_goals: NON_GOALS.iter().map(|value| (*value).to_string()).collect(),
    }
}

pub(crate) fn render_policy_promotion_json(
    report: &PolicyPromotionReport,
) -> Result<String, String> {
    serde_json::to_string_pretty(&json!({
        "schema_version": SCHEMA_VERSION,
        "tool": "ripr",
        "kind": REPORT_KIND,
        "root": report.root,
        "generated_at": report.generated_at,
        "target_mode": report.target_mode,
        "allowed_now": report.allowed_now,
        "why_or_why_not": report.why_or_why_not,
        "required_repairs": report.required_repairs,
        "required_receipts": report.required_receipts,
        "rollback_path": report.rollback_path,
        "example_config_change": {
            "file": report.example_config_change.file,
            "change": report.example_config_change.change,
            "manual_only": report.example_config_change.manual_only,
        },
        "input_artifacts": report.input_artifacts.iter().map(input_artifact_json).collect::<Vec<_>>(),
        "warnings": report.warnings.iter().map(notice_json).collect::<Vec<_>>(),
        "unknowns": report.unknowns.iter().map(notice_json).collect::<Vec<_>>(),
        "non_goals": report.non_goals,
        "limits_note": LIMITS_NOTE,
    }))
    .map_err(|err| format!("failed to render policy promotion JSON: {err}"))
}

pub(crate) fn render_policy_promotion_markdown(report: &PolicyPromotionReport) -> String {
    let mut out = String::new();
    out.push_str("# RIPR Policy Promotion Packet\n\n");
    out.push_str(&format!("Target mode: {}\n", report.target_mode));
    out.push_str(&format!(
        "Allowed now: {}\n",
        if report.allowed_now { "yes" } else { "no" }
    ));
    out.push_str(&format!("Why: {}\n", markdown_text(&report.why_or_why_not)));

    render_string_section(&mut out, "Required Repairs", &report.required_repairs);
    render_string_section(&mut out, "Required Receipts", &report.required_receipts);
    render_string_section(&mut out, "Rollback", &report.rollback_path);

    out.push_str("\n## Example Config Change\n\n");
    out.push_str(&format!(
        "- File: {}\n",
        markdown_text(&report.example_config_change.file)
    ));
    out.push_str(&format!(
        "- Change: {}\n",
        markdown_text(&report.example_config_change.change)
    ));
    out.push_str("- Manual review only. This command does not edit ripr.toml.\n");

    if !report.warnings.is_empty() {
        out.push_str("\n## Warnings\n\n");
        for warning in &report.warnings {
            out.push_str(&format!(
                "- {}: {}\n",
                warning.kind,
                markdown_text(&warning.message)
            ));
        }
    }

    if !report.unknowns.is_empty() {
        out.push_str("\n## Unknowns\n\n");
        for unknown in &report.unknowns {
            out.push_str(&format!(
                "- {}: {}\n",
                unknown.kind,
                markdown_text(&unknown.message)
            ));
        }
    }

    out.push_str("\n## Input Artifacts\n\n");
    for artifact in &report.input_artifacts {
        out.push_str(&format!("- {}: {}", artifact.kind, artifact.status));
        if let Some(path) = artifact.path.as_deref() {
            out.push_str(&format!(" ({})", markdown_text(path)));
        }
        out.push('\n');
    }

    out.push_str("\n## Non-Goals\n\n");
    for non_goal in &report.non_goals {
        out.push_str(&format!("- {}\n", markdown_text(non_goal)));
    }

    out.push_str("\nLimits:\n");
    out.push_str(LIMITS_NOTE);
    out.push('\n');
    out
}

pub(crate) fn policy_promotion_allowed_now(report: &PolicyPromotionReport) -> bool {
    report.allowed_now
}

pub(crate) fn display_path(path: &Path) -> String {
    path.display().to_string().replace('\\', "/")
}

fn parse_required_json(
    kind: &'static str,
    path: String,
    text: Result<String, String>,
) -> ParsedArtifact {
    parse_json(kind, Some(path), Some(text))
}

fn parse_optional_json(
    kind: &'static str,
    path: Option<String>,
    text: Option<Result<String, String>>,
) -> ParsedArtifact {
    parse_json(kind, path, text)
}

fn parse_json(
    kind: &'static str,
    path: Option<String>,
    text: Option<Result<String, String>>,
) -> ParsedArtifact {
    if path.is_none() {
        return ParsedArtifact {
            kind,
            path,
            status: "omitted",
            value: None,
        };
    }
    let Some(text) = text else {
        return ParsedArtifact {
            kind,
            path,
            status: "missing",
            value: None,
        };
    };
    let text = match text {
        Ok(text) => text,
        Err(error) => {
            return ParsedArtifact {
                kind,
                path,
                status: if looks_like_missing_file(&error) {
                    "missing"
                } else {
                    "malformed"
                },
                value: None,
            };
        }
    };
    match serde_json::from_str::<Value>(&text) {
        Ok(value) => ParsedArtifact {
            kind,
            path,
            status: "read",
            value: Some(value),
        },
        Err(_) => ParsedArtifact {
            kind,
            path,
            status: "malformed",
            value: None,
        },
    }
}

fn looks_like_missing_file(error: &str) -> bool {
    error.contains("os error 2")
        || error.contains("No such file")
        || error.contains("cannot find the file")
}

fn collect_artifact_notices(
    artifact: &ParsedArtifact,
    required: bool,
    warnings: &mut Vec<Notice>,
    unknowns: &mut Vec<Notice>,
) {
    match artifact.status {
        "read" => {}
        "omitted" if !required => unknowns.push(Notice {
            kind: format!("{}_not_supplied", artifact.kind),
            message: format!(
                "{} was not supplied; trend and rollback context remain unknown.",
                artifact.kind.replace('_', " ")
            ),
            source_artifact: None,
        }),
        "missing" if required => warnings.push(Notice {
            kind: format!("{}_missing", artifact.kind),
            message: format!(
                "{} is required before promotion can be reviewed.",
                artifact.kind.replace('_', " ")
            ),
            source_artifact: artifact.path.clone(),
        }),
        "missing" => unknowns.push(Notice {
            kind: format!("{}_missing", artifact.kind),
            message: format!(
                "{} was supplied but could not be read; trend and rollback context remain unknown.",
                artifact.kind.replace('_', " ")
            ),
            source_artifact: artifact.path.clone(),
        }),
        "malformed" => warnings.push(Notice {
            kind: format!("{}_malformed", artifact.kind),
            message: format!(
                "{} input is not valid JSON for this packet.",
                artifact.kind.replace('_', " ")
            ),
            source_artifact: artifact.path.clone(),
        }),
        _ => {}
    }
}

fn explain_promotion(
    target_mode: &str,
    allowed_now: bool,
    operations: &ParsedArtifact,
    safe_assessment: Option<&Value>,
    blocked_assessment: Option<&Value>,
    blockers: &[&Value],
) -> String {
    if operations.status != "read" {
        return "Policy operations input is missing or malformed; promotion cannot be reviewed."
            .to_string();
    }
    if allowed_now {
        return safe_assessment
            .and_then(|value| string_path(value, &["reason"]))
            .unwrap_or_else(|| {
                format!("Policy operations lists {target_mode} in safe_to_promote_to.")
            });
    }
    if let Some(reason) = blocked_assessment.and_then(|value| string_path(value, &["reason"])) {
        return reason;
    }
    let messages = blockers
        .iter()
        .filter_map(|blocker| string_path(blocker, &["message"]))
        .collect::<Vec<_>>();
    if !messages.is_empty() {
        return messages.join(" ");
    }
    format!("Policy operations does not list {target_mode} in safe_to_promote_to.")
}

fn required_repairs(
    target_mode: &str,
    allowed_now: bool,
    operations: Option<&Value>,
    blocked_assessment: Option<&Value>,
    blockers: &[&Value],
) -> Vec<String> {
    if allowed_now {
        return Vec::new();
    }
    let mut repairs = Vec::new();
    for blocker in blockers {
        if let Some(action) = string_path(blocker, &["repair_action"]) {
            push_unique(&mut repairs, action);
        }
    }
    if let Some(assessment) = blocked_assessment {
        for blocker in string_array_path(assessment, &["blockers"]) {
            push_unique(&mut repairs, blocker);
        }
    }
    if let Some(value) = operations {
        match target_mode {
            "acknowledgeable" => {
                push_actions(&mut repairs, value, "waiver_actions");
                push_actions(&mut repairs, value, "suppression_actions");
            }
            "baseline-check" => {
                push_actions(&mut repairs, value, "baseline_actions");
                push_actions(&mut repairs, value, "waiver_actions");
                push_actions(&mut repairs, value, "suppression_actions");
                push_actions(&mut repairs, value, "preview_boundary_actions");
            }
            "calibrated-gate" => {
                push_actions(&mut repairs, value, "baseline_actions");
                push_actions(&mut repairs, value, "suppression_actions");
                push_actions(&mut repairs, value, "calibration_actions");
                push_actions(&mut repairs, value, "preview_boundary_actions");
            }
            _ => {}
        }
    }
    if repairs.is_empty() {
        push_unique(
            &mut repairs,
            "Generate policy-operations.json and repair blockers before manual promotion review.",
        );
    }
    repairs
}

fn required_receipts(
    target_mode: &str,
    allowed_now: bool,
    operations: &ParsedArtifact,
    history_read: bool,
) -> Vec<String> {
    let mut receipts = Vec::new();
    if operations.status == "read" && allowed_now {
        push_unique(
            &mut receipts,
            format!("policy-operations.json showing {target_mode} in safe_to_promote_to"),
        );
    } else {
        push_unique(
            &mut receipts,
            format!("policy-operations.json showing why {target_mode} is blocked"),
        );
    }
    match target_mode {
        "visible-only" => {
            push_unique(
                &mut receipts,
                "policy-readiness.json supporting ready_for_visible_only or stricter ceiling",
            );
        }
        "acknowledgeable" => {
            push_unique(
                &mut receipts,
                "waiver-aging.json showing PR-time acknowledgement pressure is reviewed",
            );
            push_unique(
                &mut receipts,
                "suppression-health.json showing durable exception metadata is healthy",
            );
        }
        "baseline-check" => {
            push_unique(
                &mut receipts,
                "baseline-debt-delta.json showing reviewed shrink-only movement",
            );
            push_unique(
                &mut receipts,
                "suppression-health.json showing durable exception metadata is healthy",
            );
        }
        "calibrated-gate" => {
            push_unique(
                &mut receipts,
                "recommendation-calibration.json showing same-class stable Rust calibration",
            );
            push_unique(
                &mut receipts,
                "mutation-calibration.json when explicitly supplied for the same stable Rust class",
            );
        }
        _ => {}
    }
    if history_read {
        push_unique(
            &mut receipts,
            "policy-history.json showing readiness trend and rollback context",
        );
    } else {
        push_unique(
            &mut receipts,
            "policy-history.json is recommended for trend review before manual promotion",
        );
    }
    receipts
}

fn rollback_path(target_mode: &str) -> Vec<String> {
    let fallback = match target_mode {
        "calibrated-gate" => "baseline-check",
        "baseline-check" => "acknowledgeable",
        "acknowledgeable" => "visible-only",
        _ => "advisory-only",
    };
    vec![
        "Revert the manual gate-mode config change.".to_string(),
        format!("Return to {fallback} policy mode."),
        "Keep policy operations and history artifacts for audit.".to_string(),
    ]
}

fn assessment_for<'a>(value: &'a Value, field: &str, target_mode: &str) -> Option<&'a Value> {
    value
        .get(field)
        .and_then(Value::as_array)?
        .iter()
        .find(|assessment| string_path(assessment, &["mode"]).as_deref() == Some(target_mode))
}

fn blockers_for_target<'a>(value: &'a Value, target_mode: &str) -> Vec<&'a Value> {
    value
        .get("promotion_blockers")
        .and_then(Value::as_array)
        .map(|blockers| {
            blockers
                .iter()
                .filter(|blocker| {
                    string_array_path(blocker, &["target_modes"])
                        .iter()
                        .any(|mode| mode == target_mode)
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default()
}

fn push_actions(repairs: &mut Vec<String>, value: &Value, field: &str) {
    for action in string_array_path(value, &[field]) {
        push_unique(repairs, action);
    }
}

fn copy_notices(value: Option<&Value>, field: &str, prefix: &str, notices: &mut Vec<Notice>) {
    let Some(values) = value
        .and_then(|value| value.get(field))
        .and_then(Value::as_array)
    else {
        return;
    };
    for notice in values {
        let kind = string_path(notice, &["kind"]).unwrap_or_else(|| "unknown".to_string());
        let message = string_path(notice, &["message"]).unwrap_or_else(|| {
            format!("policy operations emitted a {field} entry without a message")
        });
        notices.push(Notice {
            kind: format!("{prefix}_{kind}"),
            message,
            source_artifact: string_path(notice, &["source_artifact"]),
        });
    }
}

fn input_artifact(artifact: &ParsedArtifact) -> InputArtifact {
    InputArtifact {
        kind: artifact.kind.to_string(),
        path: artifact.path.clone(),
        status: artifact.status.to_string(),
    }
}

fn input_artifact_json(artifact: &InputArtifact) -> Value {
    json!({
        "kind": artifact.kind,
        "path": artifact.path,
        "status": artifact.status,
    })
}

fn notice_json(notice: &Notice) -> Value {
    json!({
        "kind": notice.kind,
        "message": notice.message,
        "source_artifact": notice.source_artifact,
    })
}

fn render_string_section(out: &mut String, title: &str, values: &[String]) {
    out.push_str(&format!("\n## {title}\n\n"));
    if values.is_empty() {
        out.push_str("- none\n");
    } else {
        for value in values {
            out.push_str(&format!("- {}\n", markdown_text(value)));
        }
    }
}

fn push_unique(values: &mut Vec<String>, value: impl Into<String>) {
    let value = value.into();
    if value.trim().is_empty() || values.iter().any(|existing| existing == &value) {
        return;
    }
    values.push(value);
}

fn string_array_path(value: &Value, path: &[&str]) -> Vec<String> {
    path_value(value, path)
        .and_then(Value::as_array)
        .map(|values| {
            values
                .iter()
                .filter_map(Value::as_str)
                .map(ToOwned::to_owned)
                .collect::<BTreeSet<_>>()
                .into_iter()
                .collect()
        })
        .unwrap_or_default()
}

fn string_path(value: &Value, path: &[&str]) -> Option<String> {
    path_value(value, path)
        .and_then(Value::as_str)
        .map(ToOwned::to_owned)
}

fn path_value<'a>(value: &'a Value, path: &[&str]) -> Option<&'a Value> {
    let mut current = value;
    for key in path {
        current = current.get(*key)?;
    }
    Some(current)
}

fn markdown_text(value: &str) -> String {
    value.replace('\\', "\\\\")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn input(target_mode: &str, operations_json: Result<String, String>) -> PolicyPromotionInput {
        PolicyPromotionInput {
            root: ".".to_string(),
            generated_at: "unix_ms:1".to_string(),
            target_mode: target_mode.to_string(),
            operations_path: "policy-operations.json".to_string(),
            history_path: None,
            operations_json,
            history_json: None,
        }
    }

    fn operations_json(safe_modes: &[&str], blocked_modes: &[&str]) -> String {
        let safe = safe_modes
            .iter()
            .map(|mode| {
                format!(
                    r#"{{
                      "mode": "{mode}",
                      "allowed_now": true,
                      "reason": "Policy operations allows {mode}.",
                      "source_artifacts": ["policy_readiness"]
                    }}"#
                )
            })
            .collect::<Vec<_>>()
            .join(",");
        let blocked = blocked_modes
            .iter()
            .map(|mode| {
                format!(
                    r#"{{
                      "mode": "{mode}",
                      "allowed_now": false,
                      "reason": "{mode} is blocked by policy health.",
                      "blockers": ["Repair {mode} blockers."],
                      "source_artifacts": ["policy_readiness"]
                    }}"#
                )
            })
            .collect::<Vec<_>>()
            .join(",");
        format!(
            r#"{{
              "schema_version": "0.1",
              "kind": "policy_operations",
              "current_policy_ceiling": "ready_for_acknowledgeable",
              "recommended_next_action": "Repair blockers.",
              "safe_to_promote_to": [{safe}],
              "not_safe_to_promote_to": [{blocked}],
              "promotion_blockers": [
                {{
                  "kind": "suppression_health",
                  "severity": "warning",
                  "message": "Suppression health has warnings.",
                  "target_modes": ["acknowledgeable", "baseline-check"],
                  "source_artifact": "suppression-health.json",
                  "repair_action": "Repair suppression-health warnings before tightening policy."
                }}
              ],
              "baseline_actions": ["Run shrink-only baseline review."],
              "waiver_actions": ["Review repeated PR-time acknowledgements."],
              "suppression_actions": ["Repair suppression-health warnings before tightening policy."],
              "calibration_actions": ["Collect same-class calibration receipts."],
              "preview_boundary_actions": ["Keep preview evidence advisory."],
              "warnings": [],
              "unknowns": [],
              "input_artifacts": []
            }}"#
        )
    }

    fn history_json() -> String {
        r#"{
          "schema_version": "0.1",
          "kind": "policy_history",
          "current": {
            "current_policy_ceiling": "ready_for_acknowledgeable"
          },
          "history_summary": {
            "entries": 2
          }
        }"#
        .to_string()
    }

    #[test]
    fn promotion_visible_only_allowed_has_empty_repairs() {
        let mut input = input("visible-only", Ok(operations_json(&["visible-only"], &[])));
        input.history_path = Some("policy-history.json".to_string());
        input.history_json = Some(Ok(history_json()));
        let report = build_policy_promotion_report(input);

        assert!(report.allowed_now);
        assert!(report.required_repairs.is_empty());
        assert!(
            report
                .required_receipts
                .iter()
                .any(|receipt| receipt.contains("safe_to_promote_to"))
        );
        assert!(report.unknowns.is_empty());
    }

    #[test]
    fn promotion_acknowledgeable_blocked_uses_blocker_repairs() {
        let report = build_policy_promotion_report(input(
            "acknowledgeable",
            Ok(operations_json(&["visible-only"], &["acknowledgeable"])),
        ));

        assert!(!report.allowed_now);
        assert!(report.why_or_why_not.contains("blocked by policy health"));
        assert!(
            report
                .required_repairs
                .iter()
                .any(|repair| repair.contains("suppression-health warnings"))
        );
    }

    #[test]
    fn promotion_baseline_check_blocked_uses_baseline_repairs() {
        let report = build_policy_promotion_report(input(
            "baseline-check",
            Ok(operations_json(&["visible-only"], &["baseline-check"])),
        ));

        assert!(!report.allowed_now);
        assert!(
            report
                .required_repairs
                .iter()
                .any(|repair| repair.contains("shrink-only baseline review"))
        );
        assert!(
            report
                .required_receipts
                .iter()
                .any(|receipt| receipt.contains("baseline-debt-delta.json"))
        );
    }

    #[test]
    fn promotion_calibrated_gate_allowed_keeps_stable_rust_limit() {
        let report = build_policy_promotion_report(input(
            "calibrated-gate",
            Ok(operations_json(&["calibrated-gate"], &[])),
        ));

        assert!(report.allowed_now);
        assert!(
            report
                .warnings
                .iter()
                .any(|warning| warning.kind == "calibrated_gate_stable_rust_only")
        );
        assert!(
            report
                .required_receipts
                .iter()
                .any(|receipt| receipt.contains("stable Rust calibration"))
        );
    }

    #[test]
    fn promotion_missing_history_records_unknown() {
        let report = build_policy_promotion_report(input(
            "visible-only",
            Ok(operations_json(&["visible-only"], &[])),
        ));

        assert!(
            report
                .unknowns
                .iter()
                .any(|unknown| unknown.kind == "policy_history_not_supplied")
        );
        assert!(
            report
                .required_receipts
                .iter()
                .any(|receipt| receipt.contains("recommended for trend review"))
        );
    }

    #[test]
    fn promotion_malformed_operations_blocks_all_modes() {
        let report = build_policy_promotion_report(input("visible-only", Ok("{".to_string())));

        assert!(!report.allowed_now);
        assert!(
            report
                .warnings
                .iter()
                .any(|warning| warning.kind == "policy_operations_malformed")
        );
        assert!(report.why_or_why_not.contains("missing or malformed"));
    }

    #[test]
    fn promotion_json_and_markdown_are_structured() -> Result<(), String> {
        let report = build_policy_promotion_report(input(
            "baseline-check",
            Ok(operations_json(&["visible-only"], &["baseline-check"])),
        ));
        let json = render_policy_promotion_json(&report)?;
        let parsed =
            serde_json::from_str::<Value>(&json).map_err(|err| format!("json parse: {err}"))?;
        assert_eq!(
            string_path(&parsed, &["kind"]),
            Some("policy_promotion_packet".to_string())
        );
        assert_eq!(
            string_path(&parsed, &["target_mode"]),
            Some("baseline-check".to_string())
        );
        assert_eq!(
            path_value(&parsed, &["example_config_change", "manual_only"]).and_then(Value::as_bool),
            Some(true)
        );
        let markdown = render_policy_promotion_markdown(&report);
        assert!(markdown.contains("# RIPR Policy Promotion Packet"));
        assert!(markdown.contains("Target mode: baseline-check"));
        assert!(markdown.contains("Manual review only"));
        Ok(())
    }
}
