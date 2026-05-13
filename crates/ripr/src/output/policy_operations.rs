use serde_json::{Map, Value, json};
use std::collections::BTreeSet;
use std::path::Path;

const SCHEMA_VERSION: &str = "0.1";
const REPORT_KIND: &str = "policy_operations";
const LIMITS_NOTE: &str = "Read-only advisory policy operations report over explicit existing artifacts. Promotion requires separate manual review and configuration changes; this report never mutates config, baselines, suppressions, workflows, CI defaults, or preview-language eligibility.";

pub(crate) const DEFAULT_POLICY_OPERATIONS_OUT: &str = "target/ripr/reports/policy-operations.json";
pub(crate) const DEFAULT_POLICY_OPERATIONS_MD_OUT: &str =
    "target/ripr/reports/policy-operations.md";

const TARGET_MODES: [&str; 4] = [
    "visible-only",
    "acknowledgeable",
    "baseline-check",
    "calibrated-gate",
];

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PolicyOperationsInput {
    pub(crate) root: String,
    pub(crate) generated_at: String,
    pub(crate) policy_readiness_path: Option<String>,
    pub(crate) waiver_aging_path: Option<String>,
    pub(crate) suppression_health_path: Option<String>,
    pub(crate) baseline_delta_path: Option<String>,
    pub(crate) gate_decision_path: Option<String>,
    pub(crate) recommendation_calibration_path: Option<String>,
    pub(crate) mutation_calibration_path: Option<String>,
    pub(crate) preview_boundary_path: Option<String>,
    pub(crate) policy_readiness_json: Option<Result<String, String>>,
    pub(crate) waiver_aging_json: Option<Result<String, String>>,
    pub(crate) suppression_health_json: Option<Result<String, String>>,
    pub(crate) baseline_delta_json: Option<Result<String, String>>,
    pub(crate) gate_decision_json: Option<Result<String, String>>,
    pub(crate) recommendation_calibration_json: Option<Result<String, String>>,
    pub(crate) mutation_calibration_json: Option<Result<String, String>>,
    pub(crate) preview_boundary_json: Option<Result<String, String>>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PolicyOperationsReport {
    root: String,
    generated_at: String,
    current_policy_ceiling: String,
    recommended_next_action: String,
    safe_to_promote_to: Vec<PromotionAssessment>,
    not_safe_to_promote_to: Vec<PromotionAssessment>,
    promotion_blockers: Vec<PromotionBlocker>,
    baseline_actions: Vec<String>,
    waiver_actions: Vec<String>,
    suppression_actions: Vec<String>,
    calibration_actions: Vec<String>,
    preview_boundary_actions: Vec<String>,
    warnings: Vec<Notice>,
    unknowns: Vec<Notice>,
    input_artifacts: Vec<InputArtifact>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct PromotionAssessment {
    mode: String,
    allowed_now: bool,
    reason: String,
    blockers: Vec<String>,
    source_artifacts: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct PromotionBlocker {
    kind: String,
    severity: String,
    message: String,
    target_modes: Vec<String>,
    source_artifact: Option<String>,
    repair_action: String,
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
    Omitted,
    Read,
    Missing,
    Malformed,
}

#[derive(Clone, Debug)]
struct ParsedArtifact {
    kind: &'static str,
    path: Option<String>,
    status: ArtifactStatus,
    value: Option<Value>,
    notice: Option<Notice>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct BaselineFacts {
    still_present: usize,
    resolved: usize,
    new_policy_eligible: usize,
    acknowledged: usize,
    suppressed: usize,
    stale: usize,
    invalid: usize,
    missing_input: usize,
    auto_adopt_new: bool,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct PreviewFacts {
    languages: Vec<String>,
    visible: usize,
    gate_eligible: usize,
    ripr_zero_blocking: usize,
    calibrated_confidence: usize,
    missing_language_status: usize,
    static_limits_seen: usize,
}

pub(crate) fn build_policy_operations_report(
    input: PolicyOperationsInput,
) -> PolicyOperationsReport {
    let policy_readiness = parse_artifact(
        "policy_readiness",
        input.policy_readiness_path.clone(),
        input.policy_readiness_json,
    );
    let waiver_aging = parse_artifact(
        "waiver_aging",
        input.waiver_aging_path.clone(),
        input.waiver_aging_json,
    );
    let suppression_health = parse_artifact(
        "suppression_health",
        input.suppression_health_path.clone(),
        input.suppression_health_json,
    );
    let baseline_delta = parse_artifact(
        "baseline_delta",
        input.baseline_delta_path.clone(),
        input.baseline_delta_json,
    );
    let gate_decision = parse_artifact(
        "gate_decision",
        input.gate_decision_path.clone(),
        input.gate_decision_json,
    );
    let recommendation_calibration = parse_artifact(
        "recommendation_calibration",
        input.recommendation_calibration_path.clone(),
        input.recommendation_calibration_json,
    );
    let mutation_calibration = parse_artifact(
        "mutation_calibration",
        input.mutation_calibration_path.clone(),
        input.mutation_calibration_json,
    );
    let preview_boundary = parse_artifact(
        "preview_boundary",
        input.preview_boundary_path.clone(),
        input.preview_boundary_json,
    );

    let artifacts = [
        &policy_readiness,
        &waiver_aging,
        &suppression_health,
        &baseline_delta,
        &gate_decision,
        &recommendation_calibration,
        &mutation_calibration,
        &preview_boundary,
    ];

    let mut warnings = artifacts
        .iter()
        .filter_map(|artifact| artifact.notice.clone())
        .filter(|notice| {
            !matches!(
                notice.kind.as_str(),
                "omitted_policy_readiness" | "omitted_preview_boundary"
            )
        })
        .collect::<Vec<_>>();
    let mut unknowns = Vec::new();

    let current_policy_ceiling = policy_readiness
        .value
        .as_ref()
        .and_then(|value| string_path(value, &["status"]))
        .or_else(|| {
            policy_readiness
                .value
                .as_ref()
                .and_then(|value| string_path(value, &["current_policy_ceiling"]))
        })
        .unwrap_or_else(|| "config_error".to_string());
    let readiness_next_action = policy_readiness
        .value
        .as_ref()
        .and_then(|value| string_path(value, &["next_policy_action"]))
        .or_else(|| {
            policy_readiness
                .value
                .as_ref()
                .and_then(|value| string_path(value, &["recommended_next_action"]))
        });

    add_input_unknowns(
        &mut unknowns,
        &policy_readiness,
        "policy_readiness",
        "Policy readiness input is required before policy operations can recommend promotion.",
    );
    add_recommended_unknown(
        &mut unknowns,
        &waiver_aging,
        "Waiver-aging input not supplied; acknowledgement confidence is limited.",
    );
    add_recommended_unknown(
        &mut unknowns,
        &suppression_health,
        "Suppression-health input not supplied; durable exception health is unknown.",
    );
    add_recommended_unknown(
        &mut unknowns,
        &baseline_delta,
        "Baseline-delta input not supplied; baseline-check is blocked.",
    );
    add_recommended_unknown(
        &mut unknowns,
        &gate_decision,
        "Gate-decision input not supplied; visible policy state is limited to readiness.",
    );
    add_recommended_unknown(
        &mut unknowns,
        &recommendation_calibration,
        "Recommendation calibration input not supplied; calibrated-gate is blocked.",
    );
    if mutation_calibration.status == ArtifactStatus::Omitted {
        warnings.push(Notice {
            kind: "missing_optional_input".to_string(),
            message: "No mutation calibration input was supplied.".to_string(),
            source_artifact: None,
        });
    }
    if preview_boundary.status == ArtifactStatus::Omitted {
        unknowns.push(Notice {
            kind: "preview_boundary_not_supplied".to_string(),
            message: "Preview boundary details came only from policy readiness when available."
                .to_string(),
            source_artifact: policy_readiness.path.clone(),
        });
    }

    let baseline_facts = baseline_delta
        .value
        .as_ref()
        .map(baseline_facts)
        .unwrap_or_default();
    let preview_facts = preview_facts(
        preview_boundary
            .value
            .as_ref()
            .or(policy_readiness.value.as_ref()),
    );

    let mut promotion_blockers = Vec::new();
    collect_input_blockers(
        &mut promotion_blockers,
        &policy_readiness,
        &[
            "visible-only",
            "acknowledgeable",
            "baseline-check",
            "calibrated-gate",
        ],
        "policy_readiness_unavailable",
        "Policy readiness must be readable before promotion review.",
        "Run `ripr policy readiness` and supply the generated JSON.",
    );
    collect_input_blockers(
        &mut promotion_blockers,
        &waiver_aging,
        &["acknowledgeable", "baseline-check", "calibrated-gate"],
        "waiver_aging_unavailable",
        "Waiver aging must be readable before acknowledgement policy.",
        "Run `ripr policy waiver-aging` and review repeated PR-time acknowledgements.",
    );
    collect_input_blockers(
        &mut promotion_blockers,
        &suppression_health,
        &["acknowledgeable", "baseline-check", "calibrated-gate"],
        "suppression_health_unavailable",
        "Suppression health must be readable before stricter policy.",
        "Run `ripr policy suppression-health` and repair metadata gaps.",
    );
    collect_input_blockers(
        &mut promotion_blockers,
        &baseline_delta,
        &["baseline-check", "calibrated-gate"],
        "baseline_delta_unavailable",
        "Baseline debt delta must be readable before baseline-check.",
        "Run `ripr baseline diff` against the reviewed baseline.",
    );
    collect_input_blockers(
        &mut promotion_blockers,
        &recommendation_calibration,
        &["calibrated-gate"],
        "recommendation_calibration_unavailable",
        "Same-class recommendation calibration is required before calibrated-gate.",
        "Collect same-class recommendation calibration receipts for the target class.",
    );
    collect_malformed_optional_blocker(
        &mut promotion_blockers,
        &gate_decision,
        &[
            "visible-only",
            "acknowledgeable",
            "baseline-check",
            "calibrated-gate",
        ],
        "gate_decision_malformed",
        "Supplied gate-decision input is malformed.",
        "Repair or omit the gate-decision input before promotion review.",
    );
    collect_malformed_optional_blocker(
        &mut promotion_blockers,
        &mutation_calibration,
        &["calibrated-gate"],
        "mutation_calibration_malformed",
        "Supplied mutation calibration input is malformed.",
        "Repair or omit mutation calibration before calibrated-gate review.",
    );
    collect_malformed_optional_blocker(
        &mut promotion_blockers,
        &preview_boundary,
        &["baseline-check", "calibrated-gate"],
        "preview_boundary_malformed",
        "Supplied preview-boundary input is malformed.",
        "Repair preview-boundary input or use policy-readiness preview metadata.",
    );

    collect_ceiling_blockers(&mut promotion_blockers, &current_policy_ceiling);
    collect_status_blockers(
        &mut promotion_blockers,
        &waiver_aging,
        "waiver_aging_warning",
        "Waiver-aging status requires review before acknowledgement policy.",
        &["acknowledgeable", "baseline-check", "calibrated-gate"],
        "Review repeated PR-time acknowledgements before tightening policy.",
    );
    collect_status_blockers(
        &mut promotion_blockers,
        &suppression_health,
        "suppression_health_warnings",
        "Suppression health has warnings or config errors.",
        &["acknowledgeable", "baseline-check", "calibrated-gate"],
        "Repair durable suppression metadata before tightening policy.",
    );
    collect_baseline_blockers(&mut promotion_blockers, &baseline_delta, &baseline_facts);
    collect_preview_blockers(&mut promotion_blockers, &policy_readiness, &preview_facts);

    let baseline_actions = baseline_actions(&baseline_delta, &baseline_facts);
    let waiver_actions = waiver_actions(&waiver_aging);
    let suppression_actions = suppression_actions(&suppression_health);
    let calibration_actions =
        calibration_actions(&recommendation_calibration, &mutation_calibration);
    let preview_boundary_actions = preview_boundary_actions(&preview_facts);

    let input_artifacts = artifacts
        .iter()
        .map(|artifact| InputArtifact {
            kind: artifact.kind.to_string(),
            path: artifact.path.clone(),
            status: artifact_status_label(artifact.status).to_string(),
        })
        .collect::<Vec<_>>();

    let mut safe_to_promote_to = Vec::new();
    let mut not_safe_to_promote_to = Vec::new();
    for mode in TARGET_MODES {
        let mode_blockers = promotion_blockers
            .iter()
            .filter(|blocker| blocker.target_modes.iter().any(|target| target == mode))
            .collect::<Vec<_>>();
        let allowed_now = mode_blockers.is_empty();
        let assessment = if allowed_now {
            PromotionAssessment {
                mode: mode.to_string(),
                allowed_now,
                reason: safe_reason(mode).to_string(),
                blockers: Vec::new(),
                source_artifacts: safe_sources(mode, &artifacts),
            }
        } else {
            PromotionAssessment {
                mode: mode.to_string(),
                allowed_now,
                reason: not_safe_reason(&mode_blockers),
                blockers: mode_blockers
                    .iter()
                    .map(|blocker| blocker.kind.clone())
                    .collect(),
                source_artifacts: blocker_sources(&mode_blockers),
            }
        };
        if assessment.allowed_now {
            safe_to_promote_to.push(assessment);
        } else {
            not_safe_to_promote_to.push(assessment);
        }
    }

    let recommended_next_action = promotion_blockers
        .first()
        .map(|blocker| blocker.repair_action.clone())
        .or(readiness_next_action)
        .unwrap_or_else(|| {
            "Keep policy advisory until a promotion packet is reviewed.".to_string()
        });

    PolicyOperationsReport {
        root: input.root,
        generated_at: input.generated_at,
        current_policy_ceiling,
        recommended_next_action,
        safe_to_promote_to,
        not_safe_to_promote_to,
        promotion_blockers,
        baseline_actions,
        waiver_actions,
        suppression_actions,
        calibration_actions,
        preview_boundary_actions,
        warnings,
        unknowns,
        input_artifacts,
    }
}

pub(crate) fn render_policy_operations_json(
    report: &PolicyOperationsReport,
) -> Result<String, String> {
    serde_json::to_string_pretty(&json!({
        "schema_version": SCHEMA_VERSION,
        "tool": "ripr",
        "kind": REPORT_KIND,
        "root": report.root,
        "generated_at": report.generated_at,
        "current_policy_ceiling": report.current_policy_ceiling,
        "recommended_next_action": report.recommended_next_action,
        "safe_to_promote_to": report.safe_to_promote_to.iter().map(assessment_json).collect::<Vec<_>>(),
        "not_safe_to_promote_to": report.not_safe_to_promote_to.iter().map(assessment_json).collect::<Vec<_>>(),
        "promotion_blockers": report.promotion_blockers.iter().map(blocker_json).collect::<Vec<_>>(),
        "baseline_actions": report.baseline_actions,
        "waiver_actions": report.waiver_actions,
        "suppression_actions": report.suppression_actions,
        "calibration_actions": report.calibration_actions,
        "preview_boundary_actions": report.preview_boundary_actions,
        "warnings": report.warnings.iter().map(notice_json).collect::<Vec<_>>(),
        "unknowns": report.unknowns.iter().map(notice_json).collect::<Vec<_>>(),
        "input_artifacts": report.input_artifacts.iter().map(input_artifact_json).collect::<Vec<_>>(),
        "limits_note": LIMITS_NOTE,
    }))
    .map_err(|err| format!("failed to render policy operations JSON: {err}"))
}

pub(crate) fn render_policy_operations_markdown(report: &PolicyOperationsReport) -> String {
    let mut out = String::new();
    out.push_str("# RIPR Policy Operations\n\n");
    out.push_str(&format!(
        "Current ceiling: {}\n",
        report.current_policy_ceiling
    ));
    out.push_str(&format!(
        "Next safe action: {}\n",
        report.recommended_next_action
    ));

    out.push_str("\n## Can Promote To\n\n");
    if report.safe_to_promote_to.is_empty() {
        out.push_str("- none: no\n");
    } else {
        for assessment in &report.safe_to_promote_to {
            out.push_str(&format!("- {}: yes\n", assessment.mode));
        }
    }

    out.push_str("\n## Cannot Promote To\n\n");
    if report.not_safe_to_promote_to.is_empty() {
        out.push_str("- none\n");
    } else {
        for assessment in &report.not_safe_to_promote_to {
            out.push_str(&format!(
                "- {}: no, {}\n",
                assessment.mode, assessment.reason
            ));
        }
    }

    out.push_str("\n## Top Blockers\n\n");
    if report.promotion_blockers.is_empty() {
        out.push_str("- none\n");
    } else {
        for blocker in report.promotion_blockers.iter().take(5) {
            out.push_str(&format!("- {}: {}\n", blocker.kind, blocker.repair_action));
        }
    }

    out.push_str("\n## Actions\n\n");
    render_action_group(&mut out, "Baseline actions", &report.baseline_actions);
    render_action_group(&mut out, "Waiver actions", &report.waiver_actions);
    render_action_group(&mut out, "Suppression actions", &report.suppression_actions);
    render_action_group(&mut out, "Calibration actions", &report.calibration_actions);
    render_action_group(
        &mut out,
        "Preview evidence boundary",
        &report.preview_boundary_actions,
    );

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

    out.push_str("\n## Input Artifacts\n\n");
    for artifact in &report.input_artifacts {
        out.push_str(&format!(
            "- {}: {}",
            artifact.kind.replace('_', "-"),
            artifact.status
        ));
        if let Some(path) = artifact.path.as_deref() {
            out.push_str(&format!(" ({})", markdown_text(path)));
        }
        out.push('\n');
    }

    out.push_str("\nLimits:\n");
    out.push_str(LIMITS_NOTE);
    out.push('\n');
    out
}

pub(crate) fn policy_operations_current_ceiling(report: &PolicyOperationsReport) -> &str {
    &report.current_policy_ceiling
}

pub(crate) fn policy_operations_next_action(report: &PolicyOperationsReport) -> &str {
    &report.recommended_next_action
}

pub(crate) fn display_path(path: &Path) -> String {
    path.display().to_string().replace('\\', "/")
}

fn parse_artifact(
    kind: &'static str,
    path: Option<String>,
    text: Option<Result<String, String>>,
) -> ParsedArtifact {
    let Some(path_value) = path.clone() else {
        return ParsedArtifact {
            kind,
            path,
            status: ArtifactStatus::Omitted,
            value: None,
            notice: None,
        };
    };
    let Some(text) = text else {
        return ParsedArtifact {
            kind,
            path,
            status: ArtifactStatus::Missing,
            value: None,
            notice: Some(Notice {
                kind: format!("{kind}_missing"),
                message: format!("{kind} path {path_value} was supplied but no text was loaded."),
                source_artifact: Some(path_value),
            }),
        };
    };
    let text = match text {
        Ok(text) => text,
        Err(error) => {
            let status = if looks_like_missing_file(&error) {
                ArtifactStatus::Missing
            } else {
                ArtifactStatus::Malformed
            };
            return ParsedArtifact {
                kind,
                path,
                status,
                value: None,
                notice: Some(Notice {
                    kind: if status == ArtifactStatus::Missing {
                        format!("{kind}_missing")
                    } else {
                        format!("{kind}_unreadable")
                    },
                    message: format!("{kind} input {path_value} could not be read: {error}"),
                    source_artifact: Some(path_value),
                }),
            };
        }
    };
    match serde_json::from_str::<Value>(&text) {
        Ok(value) => ParsedArtifact {
            kind,
            path,
            status: ArtifactStatus::Read,
            value: Some(value),
            notice: None,
        },
        Err(error) => ParsedArtifact {
            kind,
            path,
            status: ArtifactStatus::Malformed,
            value: None,
            notice: Some(Notice {
                kind: format!("{kind}_malformed"),
                message: format!("{kind} input {path_value} is invalid JSON: {error}"),
                source_artifact: Some(path_value),
            }),
        },
    }
}

fn looks_like_missing_file(error: &str) -> bool {
    error.contains("os error 2")
        || error.contains("No such file")
        || error.contains("cannot find the file")
}

fn add_input_unknowns(
    unknowns: &mut Vec<Notice>,
    artifact: &ParsedArtifact,
    kind: &str,
    message: &str,
) {
    if artifact.status == ArtifactStatus::Omitted || artifact.status == ArtifactStatus::Missing {
        unknowns.push(Notice {
            kind: format!("{kind}_not_supplied"),
            message: message.to_string(),
            source_artifact: artifact.path.clone(),
        });
    }
}

fn add_recommended_unknown(unknowns: &mut Vec<Notice>, artifact: &ParsedArtifact, message: &str) {
    if artifact.status == ArtifactStatus::Omitted || artifact.status == ArtifactStatus::Missing {
        unknowns.push(Notice {
            kind: format!("{}_not_supplied", artifact.kind),
            message: message.to_string(),
            source_artifact: artifact.path.clone(),
        });
    }
}

fn collect_input_blockers(
    blockers: &mut Vec<PromotionBlocker>,
    artifact: &ParsedArtifact,
    target_modes: &[&str],
    kind: &str,
    message: &str,
    repair_action: &str,
) {
    match artifact.status {
        ArtifactStatus::Read => {}
        ArtifactStatus::Omitted | ArtifactStatus::Missing | ArtifactStatus::Malformed => {
            let severity = if artifact.status == ArtifactStatus::Malformed {
                "config_error"
            } else {
                "warning"
            };
            blockers.push(PromotionBlocker {
                kind: kind.to_string(),
                severity: severity.to_string(),
                message: message.to_string(),
                target_modes: target_modes
                    .iter()
                    .map(|mode| (*mode).to_string())
                    .collect(),
                source_artifact: artifact.path.clone(),
                repair_action: repair_action.to_string(),
            });
        }
    }
}

fn collect_malformed_optional_blocker(
    blockers: &mut Vec<PromotionBlocker>,
    artifact: &ParsedArtifact,
    target_modes: &[&str],
    kind: &str,
    message: &str,
    repair_action: &str,
) {
    if artifact.status == ArtifactStatus::Malformed {
        blockers.push(PromotionBlocker {
            kind: kind.to_string(),
            severity: "config_error".to_string(),
            message: message.to_string(),
            target_modes: target_modes
                .iter()
                .map(|mode| (*mode).to_string())
                .collect(),
            source_artifact: artifact.path.clone(),
            repair_action: repair_action.to_string(),
        });
    }
}

fn collect_ceiling_blockers(blockers: &mut Vec<PromotionBlocker>, ceiling: &str) {
    for mode in TARGET_MODES {
        if ceiling_rank(ceiling) < mode_rank(mode) {
            blockers.push(PromotionBlocker {
                kind: format!("current_ceiling_below_{}", mode.replace('-', "_")),
                severity: "warning".to_string(),
                message: format!("Current policy ceiling {ceiling} does not allow {mode}."),
                target_modes: vec![mode.to_string()],
                source_artifact: None,
                repair_action: format!("Fix lower-mode blockers before reviewing {mode}."),
            });
        }
    }
}

fn collect_status_blockers(
    blockers: &mut Vec<PromotionBlocker>,
    artifact: &ParsedArtifact,
    kind: &str,
    message: &str,
    target_modes: &[&str],
    repair_action: &str,
) {
    let Some(value) = artifact.value.as_ref() else {
        return;
    };
    let status = string_path(value, &["status"]).unwrap_or_else(|| "unknown".to_string());
    let healthy = match artifact.kind {
        "waiver_aging" => matches!(status.as_str(), "advisory" | "no_waivers"),
        "suppression_health" => matches!(status.as_str(), "healthy" | "no_suppressions"),
        _ => true,
    };
    if !healthy {
        blockers.push(PromotionBlocker {
            kind: kind.to_string(),
            severity: if status == "config_error" {
                "config_error"
            } else {
                "warning"
            }
            .to_string(),
            message: format!("{message} Status: {status}."),
            target_modes: target_modes
                .iter()
                .map(|mode| (*mode).to_string())
                .collect(),
            source_artifact: artifact.path.clone(),
            repair_action: repair_action.to_string(),
        });
    }
}

fn collect_baseline_blockers(
    blockers: &mut Vec<PromotionBlocker>,
    artifact: &ParsedArtifact,
    facts: &BaselineFacts,
) {
    if artifact.status != ArtifactStatus::Read {
        return;
    }
    if facts.auto_adopt_new {
        blockers.push(PromotionBlocker {
            kind: "baseline_auto_adopt_enabled".to_string(),
            severity: "config_error".to_string(),
            message: "Baseline auto-adopt is enabled; promotion requires reviewed shrink-only baseline behavior.".to_string(),
            target_modes: vec!["baseline-check".to_string(), "calibrated-gate".to_string()],
            source_artifact: artifact.path.clone(),
            repair_action: "Disable auto-adopt-new and review baseline movement manually.".to_string(),
        });
    }
    if facts.stale > 0 {
        blockers.push(PromotionBlocker {
            kind: "baseline_stale_entries".to_string(),
            severity: "warning".to_string(),
            message: format!("Baseline contains {} stale entries.", facts.stale),
            target_modes: vec!["baseline-check".to_string(), "calibrated-gate".to_string()],
            source_artifact: artifact.path.clone(),
            repair_action: "Run shrink-only baseline review and remove resolved entries."
                .to_string(),
        });
    }
    if facts.invalid > 0 || facts.missing_input > 0 {
        blockers.push(PromotionBlocker {
            kind: "baseline_invalid_or_missing_input".to_string(),
            severity: "config_error".to_string(),
            message: format!(
                "Baseline delta has invalid entries={} and missing inputs={}.",
                facts.invalid, facts.missing_input
            ),
            target_modes: vec!["baseline-check".to_string(), "calibrated-gate".to_string()],
            source_artifact: artifact.path.clone(),
            repair_action: "Repair baseline inputs before baseline-check.".to_string(),
        });
    }
}

fn collect_preview_blockers(
    blockers: &mut Vec<PromotionBlocker>,
    policy_readiness: &ParsedArtifact,
    facts: &PreviewFacts,
) {
    if facts.missing_language_status > 0 {
        blockers.push(PromotionBlocker {
            kind: "preview_language_status_missing".to_string(),
            severity: "warning".to_string(),
            message: format!(
                "{} preview-language findings are missing language_status metadata.",
                facts.missing_language_status
            ),
            target_modes: vec![
                "visible-only".to_string(),
                "acknowledgeable".to_string(),
                "baseline-check".to_string(),
                "calibrated-gate".to_string(),
            ],
            source_artifact: policy_readiness.path.clone(),
            repair_action: "Add preview status and static-limit metadata before promotion review."
                .to_string(),
        });
    }
    if facts.gate_eligible > 0 || facts.ripr_zero_blocking > 0 || facts.calibrated_confidence > 0 {
        blockers.push(PromotionBlocker {
            kind: "preview_boundary_violation".to_string(),
            severity: "config_error".to_string(),
            message: format!(
                "Preview evidence reports gate_eligible={}, ripr_zero_blocking={}, calibrated_confidence={}.",
                facts.gate_eligible, facts.ripr_zero_blocking, facts.calibrated_confidence
            ),
            target_modes: vec!["baseline-check".to_string(), "calibrated-gate".to_string()],
            source_artifact: policy_readiness.path.clone(),
            repair_action: "Keep preview-language evidence advisory until a separate promotion packet is reviewed.".to_string(),
        });
    }
}

fn ceiling_rank(ceiling: &str) -> usize {
    match ceiling {
        "ready_for_calibrated_gate" => 4,
        "ready_for_baseline_check" => 3,
        "ready_for_acknowledgeable" => 2,
        "ready_for_visible_only" => 1,
        _ => 0,
    }
}

fn mode_rank(mode: &str) -> usize {
    match mode {
        "visible-only" => 1,
        "acknowledgeable" => 2,
        "baseline-check" => 3,
        "calibrated-gate" => 4,
        _ => usize::MAX,
    }
}

fn baseline_actions(artifact: &ParsedArtifact, facts: &BaselineFacts) -> Vec<String> {
    if artifact.status != ArtifactStatus::Read {
        return vec!["Supply baseline debt delta before baseline-check.".to_string()];
    }
    let mut actions = Vec::new();
    if facts.stale > 0 {
        actions.push("Review stale baseline entries.".to_string());
    }
    if facts.invalid > 0 || facts.missing_input > 0 {
        actions.push("Repair invalid or missing baseline inputs.".to_string());
    }
    if facts.auto_adopt_new {
        actions.push("Disable automatic baseline adoption before promotion.".to_string());
    }
    actions.push("Use shrink-only refresh for resolved debt.".to_string());
    actions
}

fn waiver_actions(artifact: &ParsedArtifact) -> Vec<String> {
    if artifact.status != ArtifactStatus::Read {
        return vec![
            "Supply waiver-aging report before acknowledgement policy.".to_string(),
            "Keep PR-time acknowledgements separate from durable suppressions.".to_string(),
        ];
    }
    let status = artifact
        .value
        .as_ref()
        .and_then(|value| string_path(value, &["status"]))
        .unwrap_or_else(|| "unknown".to_string());
    if status == "config_error" || status == "incomplete" {
        vec!["Repair waiver-aging input before acknowledgement policy.".to_string()]
    } else {
        vec![
            "Review repeated PR-time acknowledgements before requiring acknowledgement."
                .to_string(),
            "Keep waivers visible and do not convert them to suppressions automatically."
                .to_string(),
        ]
    }
}

fn suppression_actions(artifact: &ParsedArtifact) -> Vec<String> {
    if artifact.status != ArtifactStatus::Read {
        return vec!["Supply suppression-health report before stricter policy.".to_string()];
    }
    let status = artifact
        .value
        .as_ref()
        .and_then(|value| string_path(value, &["status"]))
        .unwrap_or_else(|| "unknown".to_string());
    if status == "healthy" || status == "no_suppressions" {
        vec![
            "Keep durable suppressions visible with owner, reason, scope, and review metadata."
                .to_string(),
        ]
    } else {
        vec!["Repair suppression-health warnings before tightening policy.".to_string()]
    }
}

fn calibration_actions(recommendation: &ParsedArtifact, mutation: &ParsedArtifact) -> Vec<String> {
    let mut actions = Vec::new();
    if recommendation.status == ArtifactStatus::Read {
        actions.push(
            "Use calibrated-gate only for eligible stable Rust classes with same-class calibration."
                .to_string(),
        );
    } else {
        actions.push(
            "Collect same-class recommendation calibration before calibrated-gate.".to_string(),
        );
    }
    if mutation.status == ArtifactStatus::Omitted {
        actions.push(
            "Optional mutation calibration was not supplied; keep runtime confirmation separate from static evidence.".to_string(),
        );
    }
    actions
}

fn preview_boundary_actions(facts: &PreviewFacts) -> Vec<String> {
    if facts.visible > 0 || !facts.languages.is_empty() {
        let languages = if facts.languages.is_empty() {
            "preview-language".to_string()
        } else {
            facts.languages.join(", ")
        };
        vec![format!(
            "Keep {languages} preview evidence visible/advisory and excluded from gate eligibility, RIPR Zero blocking debt, and calibrated confidence."
        )]
    } else {
        vec![
            "Keep preview-language evidence advisory until an explicit promotion packet exists."
                .to_string(),
        ]
    }
}

fn safe_reason(mode: &str) -> &'static str {
    match mode {
        "visible-only" => {
            "Policy readiness and supplied inputs allow visible-only advisory display."
        }
        "acknowledgeable" => {
            "Waivers are visible PR-time acknowledgements and suppression health is readable."
        }
        "baseline-check" => {
            "Baseline debt delta is readable, reviewed, and free of stale or invalid entries."
        }
        "calibrated-gate" => {
            "Same-class stable Rust calibration inputs support calibrated-gate review."
        }
        _ => "Policy operations found no blockers for this mode.",
    }
}

fn not_safe_reason(blockers: &[&PromotionBlocker]) -> String {
    let mut parts = blockers
        .iter()
        .take(3)
        .map(|blocker| blocker.message.clone())
        .collect::<Vec<_>>();
    if parts.is_empty() {
        parts.push("Promotion is blocked by unavailable policy evidence.".to_string());
    }
    parts.join(" ")
}

fn safe_sources(mode: &str, artifacts: &[&ParsedArtifact]) -> Vec<String> {
    let wanted = match mode {
        "visible-only" => &["policy_readiness", "gate_decision"][..],
        "acknowledgeable" => &["policy_readiness", "waiver_aging", "suppression_health"][..],
        "baseline-check" => &[
            "policy_readiness",
            "waiver_aging",
            "suppression_health",
            "baseline_delta",
            "gate_decision",
        ][..],
        "calibrated-gate" => &[
            "policy_readiness",
            "waiver_aging",
            "suppression_health",
            "baseline_delta",
            "gate_decision",
            "recommendation_calibration",
            "mutation_calibration",
        ][..],
        _ => &["policy_readiness"][..],
    };
    artifacts
        .iter()
        .filter(|artifact| wanted.contains(&artifact.kind))
        .filter(|artifact| artifact.status == ArtifactStatus::Read)
        .filter_map(|artifact| artifact.path.clone())
        .collect()
}

fn blocker_sources(blockers: &[&PromotionBlocker]) -> Vec<String> {
    let mut sources = BTreeSet::new();
    for blocker in blockers {
        if let Some(source) = blocker.source_artifact.as_deref() {
            sources.insert(source.to_string());
        }
    }
    sources.into_iter().collect()
}

fn baseline_facts(value: &Value) -> BaselineFacts {
    BaselineFacts {
        still_present: usize_path(value, &["delta", "still_present"])
            + usize_path(value, &["debt_delta", "still_present"]),
        resolved: usize_path(value, &["delta", "resolved"])
            + usize_path(value, &["debt_delta", "resolved"]),
        new_policy_eligible: usize_path(value, &["delta", "new_policy_eligible"])
            + usize_path(value, &["debt_delta", "new_policy_eligible"]),
        acknowledged: usize_path(value, &["delta", "acknowledged"])
            + usize_path(value, &["debt_delta", "acknowledged"]),
        suppressed: usize_path(value, &["delta", "suppressed"])
            + usize_path(value, &["debt_delta", "suppressed"]),
        stale: usize_path(value, &["delta", "stale_baseline_entry"])
            + usize_path(value, &["debt_delta", "stale"]),
        invalid: usize_path(value, &["delta", "invalid_baseline_entry"])
            + usize_path(value, &["debt_delta", "invalid"]),
        missing_input: usize_path(value, &["delta", "missing_current_input"])
            + usize_path(value, &["debt_delta", "missing_input"]),
        auto_adopt_new: bool_path(value, &["baseline", "auto_adopt_new"])
            || bool_path(value, &["policy", "auto_adopt_new"]),
    }
}

fn preview_facts(value: Option<&Value>) -> PreviewFacts {
    let Some(value) = value else {
        return PreviewFacts::default();
    };
    let boundary = value.get("preview_evidence_boundary").unwrap_or(value);
    let mut facts = PreviewFacts {
        languages: string_array_path(boundary, &["preview_languages"]),
        visible: usize_path(boundary, &["preview_findings_visible"]),
        gate_eligible: usize_path(boundary, &["preview_findings_gate_eligible"]),
        ripr_zero_blocking: usize_path(boundary, &["preview_findings_ripr_zero_blocking"]),
        calibrated_confidence: usize_path(boundary, &["preview_findings_calibrated_confidence"]),
        missing_language_status: usize_path(boundary, &["missing_language_status"]),
        static_limits_seen: usize_path(boundary, &["static_limits_seen"]),
    };
    let mut scan = PreviewScan::default();
    scan_preview(value, &mut scan);
    for language in scan.languages {
        if !facts.languages.iter().any(|existing| existing == &language) {
            facts.languages.push(language);
        }
    }
    facts.languages.sort();
    facts.languages.dedup();
    facts.visible = facts.visible.max(scan.preview_findings);
    facts.missing_language_status = facts
        .missing_language_status
        .max(scan.missing_language_status);
    facts.static_limits_seen = facts.static_limits_seen.max(scan.static_limits_seen);
    facts
}

#[derive(Default)]
struct PreviewScan {
    languages: Vec<String>,
    preview_findings: usize,
    missing_language_status: usize,
    static_limits_seen: usize,
}

fn scan_preview(value: &Value, scan: &mut PreviewScan) {
    match value {
        Value::Object(map) => {
            let language = map.get("language").and_then(Value::as_str);
            let language_status = map.get("language_status").and_then(Value::as_str);
            if language_status == Some("preview") {
                scan.preview_findings += 1;
                if let Some(language) = language {
                    scan.languages.push(language.to_string());
                }
                if map
                    .get("static_limit_kind")
                    .and_then(Value::as_str)
                    .is_some()
                {
                    scan.static_limits_seen += 1;
                }
            } else if matches!(language, Some("typescript" | "javascript" | "python")) {
                scan.missing_language_status += 1;
                if let Some(language) = language {
                    scan.languages.push(language.to_string());
                }
            }
            for child in map.values() {
                scan_preview(child, scan);
            }
        }
        Value::Array(items) => {
            for item in items {
                scan_preview(item, scan);
            }
        }
        _ => {}
    }
}

fn assessment_json(assessment: &PromotionAssessment) -> Value {
    let mut object = Map::new();
    object.insert("mode".to_string(), json!(assessment.mode));
    object.insert("allowed_now".to_string(), json!(assessment.allowed_now));
    object.insert("reason".to_string(), json!(assessment.reason));
    if !assessment.blockers.is_empty() {
        object.insert("blockers".to_string(), json!(assessment.blockers));
    }
    object.insert(
        "source_artifacts".to_string(),
        json!(assessment.source_artifacts),
    );
    Value::Object(object)
}

fn blocker_json(blocker: &PromotionBlocker) -> Value {
    json!({
        "kind": blocker.kind,
        "severity": blocker.severity,
        "message": blocker.message,
        "target_modes": blocker.target_modes,
        "source_artifact": blocker.source_artifact,
        "repair_action": blocker.repair_action,
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
        ArtifactStatus::Omitted => "omitted",
        ArtifactStatus::Read => "read",
        ArtifactStatus::Missing => "missing",
        ArtifactStatus::Malformed => "malformed",
    }
}

fn render_action_group(out: &mut String, title: &str, actions: &[String]) {
    out.push_str(&format!("{title}:\n"));
    if actions.is_empty() {
        out.push_str("- none\n\n");
    } else {
        for action in actions {
            out.push_str(&format!("- {action}\n"));
        }
        out.push('\n');
    }
}

fn markdown_text(value: &str) -> String {
    value.replace('|', "\\|").replace('\n', " ")
}

fn string_path(value: &Value, path: &[&str]) -> Option<String> {
    path_value(value, path).and_then(|value| value.as_str().map(ToString::to_string))
}

fn string_array_path(value: &Value, path: &[&str]) -> Vec<String> {
    path_value(value, path)
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .map(ToString::to_string)
                .collect()
        })
        .unwrap_or_default()
}

fn usize_path(value: &Value, path: &[&str]) -> usize {
    path_value(value, path)
        .and_then(Value::as_u64)
        .and_then(|value| usize::try_from(value).ok())
        .unwrap_or(0)
}

fn bool_path(value: &Value, path: &[&str]) -> bool {
    path_value(value, path)
        .and_then(Value::as_bool)
        .unwrap_or(false)
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

    fn input(readiness: Option<Result<String, String>>) -> PolicyOperationsInput {
        PolicyOperationsInput {
            root: ".".to_string(),
            generated_at: "unix_ms:1".to_string(),
            policy_readiness_path: readiness
                .as_ref()
                .map(|_| "policy-readiness.json".to_string()),
            waiver_aging_path: None,
            suppression_health_path: None,
            baseline_delta_path: None,
            gate_decision_path: None,
            recommendation_calibration_path: None,
            mutation_calibration_path: None,
            preview_boundary_path: None,
            policy_readiness_json: readiness,
            waiver_aging_json: None,
            suppression_health_json: None,
            baseline_delta_json: None,
            gate_decision_json: None,
            recommendation_calibration_json: None,
            mutation_calibration_json: None,
            preview_boundary_json: None,
        }
    }

    fn readiness(status: &str) -> String {
        format!(
            r#"{{
              "schema_version": "0.1",
              "kind": "policy_readiness",
              "status": "{status}",
              "next_policy_action": "Review the next policy mode.",
              "preview_evidence_boundary": {{
                "state": "healthy",
                "preview_languages": ["typescript"],
                "preview_findings_visible": 1,
                "preview_findings_gate_eligible": 0,
                "preview_findings_ripr_zero_blocking": 0,
                "preview_findings_calibrated_confidence": 0,
                "missing_language_status": 0,
                "static_limits_seen": 1
              }}
            }}"#
        )
    }

    fn healthy_waiver() -> Result<String, String> {
        Ok(r#"{"schema_version":"0.1","kind":"waiver_aging","status":"advisory","summary":{"waiver_count":1}}"#.to_string())
    }

    fn healthy_suppression() -> Result<String, String> {
        Ok(r#"{"schema_version":"0.1","kind":"suppression_health","status":"healthy","summary":{"warnings":0,"config_errors":0}}"#.to_string())
    }

    fn baseline(stale: usize) -> Result<String, String> {
        Ok(format!(
            r#"{{
              "schema_version": "0.1",
              "kind": "baseline_debt_delta",
              "delta": {{
                "still_present": 1,
                "resolved": 0,
                "new_policy_eligible": 0,
                "acknowledged": 0,
                "suppressed": 0,
                "stale_baseline_entry": {stale},
                "invalid_baseline_entry": 0,
                "missing_current_input": 0
              }}
            }}"#
        ))
    }

    fn with_core_inputs(mut input: PolicyOperationsInput) -> PolicyOperationsInput {
        input.waiver_aging_path = Some("waiver-aging.json".to_string());
        input.waiver_aging_json = Some(healthy_waiver());
        input.suppression_health_path = Some("suppression-health.json".to_string());
        input.suppression_health_json = Some(healthy_suppression());
        input.baseline_delta_path = Some("baseline-debt-delta.json".to_string());
        input.baseline_delta_json = Some(baseline(0));
        input.gate_decision_path = Some("gate-decision.json".to_string());
        input.gate_decision_json = Some(Ok(
            r#"{"schema_version":"0.1","kind":"gate_decision","status":"advisory"}"#.to_string(),
        ));
        input
    }

    #[test]
    fn policy_operations_acknowledgeable_ceiling_blocks_baseline() {
        let mut input = with_core_inputs(input(Some(Ok(readiness("ready_for_acknowledgeable")))));
        input.baseline_delta_json = Some(baseline(1));

        let report = build_policy_operations_report(input);

        assert_eq!(report.current_policy_ceiling, "ready_for_acknowledgeable");
        assert!(
            report
                .safe_to_promote_to
                .iter()
                .any(|assessment| assessment.mode == "visible-only")
        );
        assert!(
            report
                .safe_to_promote_to
                .iter()
                .any(|assessment| assessment.mode == "acknowledgeable")
        );
        assert!(
            report
                .not_safe_to_promote_to
                .iter()
                .any(|assessment| assessment.mode == "baseline-check")
        );
        assert!(
            report
                .promotion_blockers
                .iter()
                .any(|blocker| blocker.kind == "baseline_stale_entries")
        );
    }

    #[test]
    fn policy_operations_baseline_check_ready_allows_baseline() {
        let report = build_policy_operations_report(with_core_inputs(input(Some(Ok(readiness(
            "ready_for_baseline_check",
        ))))));

        assert!(
            report
                .safe_to_promote_to
                .iter()
                .any(|assessment| assessment.mode == "baseline-check")
        );
        assert!(
            report
                .not_safe_to_promote_to
                .iter()
                .any(|assessment| assessment.mode == "calibrated-gate")
        );
    }

    #[test]
    fn policy_operations_calibrated_gate_accepts_missing_optional_mutation() {
        let mut input = with_core_inputs(input(Some(Ok(readiness("ready_for_calibrated_gate")))));
        input.recommendation_calibration_path = Some("recommendation-calibration.json".to_string());
        input.recommendation_calibration_json = Some(Ok(
            r#"{"schema_version":"0.1","kind":"recommendation_calibration","status":"complete"}"#
                .to_string(),
        ));

        let report = build_policy_operations_report(input);

        assert!(
            report
                .safe_to_promote_to
                .iter()
                .any(|assessment| assessment.mode == "calibrated-gate")
        );
        assert!(
            report
                .warnings
                .iter()
                .any(|warning| warning.kind == "missing_optional_input")
        );
    }

    #[test]
    fn policy_operations_missing_readiness_is_config_error() {
        let report = build_policy_operations_report(input(None));

        assert_eq!(report.current_policy_ceiling, "config_error");
        assert!(report.safe_to_promote_to.is_empty());
        assert!(
            report
                .promotion_blockers
                .iter()
                .any(|blocker| blocker.kind == "policy_readiness_unavailable")
        );
    }

    #[test]
    fn policy_operations_malformed_readiness_blocks_all_modes() {
        let report = build_policy_operations_report(input(Some(Ok("not json".to_string()))));

        assert!(report.safe_to_promote_to.is_empty());
        assert_eq!(
            report.input_artifacts[0],
            InputArtifact {
                kind: "policy_readiness".to_string(),
                path: Some("policy-readiness.json".to_string()),
                status: "malformed".to_string(),
            }
        );
        assert!(
            report
                .warnings
                .iter()
                .any(|warning| warning.kind == "policy_readiness_malformed")
        );
    }

    #[test]
    fn policy_operations_preview_boundary_violation_blocks_stricter_modes() {
        let mut input = with_core_inputs(input(Some(Ok(readiness("ready_for_calibrated_gate")))));
        input.recommendation_calibration_path = Some("recommendation-calibration.json".to_string());
        input.recommendation_calibration_json = Some(Ok(
            r#"{"schema_version":"0.1","kind":"recommendation_calibration","status":"complete"}"#
                .to_string(),
        ));
        input.preview_boundary_path = Some("preview-boundary.json".to_string());
        input.preview_boundary_json = Some(Ok(r#"{
              "schema_version": "0.1",
              "kind": "preview_boundary",
              "preview_languages": ["typescript"],
              "preview_findings_visible": 1,
              "preview_findings_gate_eligible": 1,
              "preview_findings_ripr_zero_blocking": 0,
              "preview_findings_calibrated_confidence": 0
            }"#
        .to_string()));

        let report = build_policy_operations_report(input);

        assert!(
            report
                .safe_to_promote_to
                .iter()
                .any(|assessment| assessment.mode == "acknowledgeable")
        );
        assert!(
            report
                .not_safe_to_promote_to
                .iter()
                .any(|assessment| assessment.mode == "baseline-check")
        );
        assert!(
            report
                .promotion_blockers
                .iter()
                .any(|blocker| blocker.kind == "preview_boundary_violation")
        );
    }

    #[test]
    fn policy_operations_supplied_but_unloaded_readiness_is_missing() {
        let mut input = input(None);
        input.policy_readiness_path = Some("policy-readiness.json".to_string());

        let report = build_policy_operations_report(input);

        assert_eq!(report.current_policy_ceiling, "config_error");
        assert_eq!(
            report.input_artifacts[0],
            InputArtifact {
                kind: "policy_readiness".to_string(),
                path: Some("policy-readiness.json".to_string()),
                status: "missing".to_string(),
            }
        );
        assert!(
            report
                .warnings
                .iter()
                .any(|warning| warning.kind == "policy_readiness_missing")
        );
        assert!(
            report
                .promotion_blockers
                .iter()
                .any(|blocker| blocker.kind == "policy_readiness_unavailable")
        );
    }

    #[test]
    fn policy_operations_read_failures_and_optional_malformed_inputs_are_reported() {
        let mut input = with_core_inputs(input(Some(Err(
            "No such file or directory (os error 2)".to_string(),
        ))));
        input.policy_readiness_path = Some("policy-readiness.json".to_string());
        input.waiver_aging_json = Some(Err("permission denied".to_string()));
        input.gate_decision_json = Some(Ok("not json".to_string()));
        input.mutation_calibration_path = Some("mutation-calibration.json".to_string());
        input.mutation_calibration_json = Some(Ok("not json".to_string()));
        input.preview_boundary_path = Some("preview-boundary.json".to_string());
        input.preview_boundary_json = Some(Ok("not json".to_string()));

        let report = build_policy_operations_report(input);

        assert_eq!(report.input_artifacts[0].status, "missing");
        assert!(
            report
                .warnings
                .iter()
                .any(|warning| warning.kind == "policy_readiness_missing")
        );
        assert!(
            report
                .warnings
                .iter()
                .any(|warning| warning.kind == "waiver_aging_unreadable")
        );
        for kind in [
            "gate_decision_malformed",
            "mutation_calibration_malformed",
            "preview_boundary_malformed",
        ] {
            assert!(
                report
                    .promotion_blockers
                    .iter()
                    .any(|blocker| blocker.kind == kind),
                "missing blocker {kind}"
            );
        }
    }

    #[test]
    fn policy_operations_unhealthy_policy_artifacts_block_stricter_modes() {
        let mut input = with_core_inputs(input(Some(Ok(readiness("ready_for_baseline_check")))));
        input.waiver_aging_json = Some(Ok(
            r#"{"schema_version":"0.1","kind":"waiver_aging","status":"config_error"}"#.to_string(),
        ));
        input.suppression_health_json = Some(Ok(
            r#"{"schema_version":"0.1","kind":"suppression_health","status":"warnings"}"#
                .to_string(),
        ));
        input.baseline_delta_json = Some(Ok(r#"{
              "schema_version": "0.1",
              "kind": "baseline_debt_delta",
              "baseline": {"auto_adopt_new": true},
              "delta": {
                "still_present": 1,
                "resolved": 0,
                "new_policy_eligible": 0,
                "acknowledged": 0,
                "suppressed": 0,
                "stale_baseline_entry": 0,
                "invalid_baseline_entry": 2,
                "missing_current_input": 1
              }
            }"#
        .to_string()));

        let report = build_policy_operations_report(input);

        for kind in [
            "waiver_aging_warning",
            "suppression_health_warnings",
            "baseline_auto_adopt_enabled",
            "baseline_invalid_or_missing_input",
        ] {
            assert!(
                report
                    .promotion_blockers
                    .iter()
                    .any(|blocker| blocker.kind == kind),
                "missing blocker {kind}"
            );
        }
        assert!(
            report
                .baseline_actions
                .iter()
                .any(|action| action == "Repair invalid or missing baseline inputs.")
        );
        assert!(
            report
                .baseline_actions
                .iter()
                .any(|action| action == "Disable automatic baseline adoption before promotion.")
        );
        assert_eq!(
            report.waiver_actions,
            vec!["Repair waiver-aging input before acknowledgement policy.".to_string()]
        );
        assert_eq!(
            report.suppression_actions,
            vec!["Repair suppression-health warnings before tightening policy.".to_string()]
        );
    }

    #[test]
    fn policy_operations_scans_nested_preview_records() {
        let input = with_core_inputs(input(Some(Ok(r#"{
              "schema_version": "0.1",
              "kind": "policy_readiness",
              "status": "ready_for_acknowledgeable",
              "findings": [
                {
                  "language": "typescript",
                  "language_status": "preview",
                  "static_limit_kind": "syntax_only"
                },
                {
                  "language": "python"
                }
              ]
            }"#
        .to_string()))));

        let report = build_policy_operations_report(input);

        assert!(
            report
                .promotion_blockers
                .iter()
                .any(|blocker| blocker.kind == "preview_language_status_missing")
        );
        assert!(
            report
                .preview_boundary_actions
                .iter()
                .any(|action| action.contains("python, typescript"))
        );
    }

    #[test]
    fn policy_operations_no_blockers_uses_default_next_action() {
        let mut input = with_core_inputs(input(Some(Ok(r#"{
              "schema_version": "0.1",
              "kind": "policy_readiness",
              "status": "ready_for_calibrated_gate",
              "preview_evidence_boundary": {
                "state": "healthy",
                "preview_findings_visible": 0,
                "preview_findings_gate_eligible": 0,
                "preview_findings_ripr_zero_blocking": 0,
                "preview_findings_calibrated_confidence": 0,
                "missing_language_status": 0,
                "static_limits_seen": 0
              }
            }"#
        .to_string()))));
        input.recommendation_calibration_path = Some("recommendation-calibration.json".to_string());
        input.recommendation_calibration_json = Some(Ok(
            r#"{"schema_version":"0.1","kind":"recommendation_calibration","status":"complete"}"#
                .to_string(),
        ));

        let report = build_policy_operations_report(input);
        let markdown = render_policy_operations_markdown(&report);

        assert_eq!(
            report.recommended_next_action,
            "Keep policy advisory until a promotion packet is reviewed."
        );
        assert!(report.promotion_blockers.is_empty());
        assert!(markdown.contains("## Cannot Promote To\n\n- none"));
        assert!(markdown.contains("## Top Blockers\n\n- none"));
    }

    #[test]
    fn policy_operations_json_and_markdown_are_structured() -> Result<(), String> {
        let report = build_policy_operations_report(with_core_inputs(input(Some(Ok(readiness(
            "ready_for_baseline_check",
        ))))));

        let json = render_policy_operations_json(&report)?;
        let markdown = render_policy_operations_markdown(&report);

        assert!(json.contains("\"kind\": \"policy_operations\""));
        assert!(json.contains("\"current_policy_ceiling\": \"ready_for_baseline_check\""));
        assert!(json.contains("\"input_artifacts\""));
        assert!(markdown.contains("# RIPR Policy Operations"));
        assert!(markdown.contains("Current ceiling: ready_for_baseline_check"));
        assert!(markdown.contains("## Input Artifacts"));
        Ok(())
    }
}
