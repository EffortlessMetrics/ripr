use serde_json::{Value, json};
use std::collections::BTreeSet;

const SCHEMA_VERSION: &str = "0.1";
const REPORT_KIND: &str = "policy_readiness";
const LIMITS_NOTE: &str = "Read-only advisory readiness over explicit artifacts; gate-decision remains the only pass/fail authority when configured.";
const PREVIEW_LIMITS_NOTE: &str = "Preview-language evidence is visible and advisory by default; it is not gate-eligible, RIPR Zero blocking debt, or calibrated confidence without explicit promotion.";

pub(crate) const DEFAULT_POLICY_READINESS_OUT: &str = "target/ripr/reports/policy-readiness.json";
pub(crate) const DEFAULT_POLICY_READINESS_MD_OUT: &str = "target/ripr/reports/policy-readiness.md";

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PolicyReadinessInput {
    pub(crate) root: String,
    pub(crate) generated_at: String,
    pub(crate) gate_decision_path: Option<String>,
    pub(crate) baseline_delta_path: Option<String>,
    pub(crate) recommendation_calibration_path: Option<String>,
    pub(crate) mutation_calibration_path: Option<String>,
    pub(crate) waiver_aging_path: Option<String>,
    pub(crate) suppression_health_path: Option<String>,
    pub(crate) repo_config_path: Option<String>,
    pub(crate) previous_readiness_path: Option<String>,
    pub(crate) gate_decision_json: Option<Result<String, String>>,
    pub(crate) baseline_delta_json: Option<Result<String, String>>,
    pub(crate) recommendation_calibration_json: Option<Result<String, String>>,
    pub(crate) mutation_calibration_json: Option<Result<String, String>>,
    pub(crate) waiver_aging_json: Option<Result<String, String>>,
    pub(crate) suppression_health_json: Option<Result<String, String>>,
    pub(crate) repo_config_json: Option<Result<String, String>>,
    pub(crate) previous_readiness_json: Option<Result<String, String>>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PolicyReadinessReport {
    root: String,
    generated_at: String,
    status: String,
    recommended_mode: String,
    inputs: PolicyReadinessInputs,
    summary: PolicyReadinessSummary,
    blocking_readiness: Axis,
    baseline_health: Axis,
    waiver_health: Axis,
    suppression_health: Axis,
    calibration_health: Axis,
    preview_evidence_boundary: PreviewEvidenceBoundary,
    unknowns: Vec<Notice>,
    warnings: Vec<Notice>,
    next_policy_action: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct PolicyReadinessInputs {
    gate_decision: Option<String>,
    baseline_delta: Option<String>,
    recommendation_calibration: Option<String>,
    mutation_calibration: Option<String>,
    waiver_aging: Option<String>,
    suppression_health: Option<String>,
    repo_config: Option<String>,
    previous_readiness: Option<String>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct PolicyReadinessSummary {
    blocking_ready: bool,
    visible_only_ready: bool,
    acknowledgeable_ready: bool,
    baseline_check_ready: bool,
    calibrated_gate_ready: bool,
    preview_candidates: usize,
    preview_candidates_gate_eligible: usize,
    warnings: usize,
    unknowns: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct Axis {
    state: String,
    evidence: Vec<String>,
    warnings: Vec<Notice>,
    next_action: String,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct PreviewEvidenceBoundary {
    state: String,
    preview_languages: Vec<String>,
    preview_findings_visible: usize,
    preview_findings_acknowledgeable: usize,
    preview_findings_suppressible: usize,
    preview_findings_baseline_advisory: usize,
    preview_findings_gate_eligible: usize,
    preview_findings_ripr_zero_blocking: usize,
    preview_findings_calibrated_confidence: usize,
    missing_language_status: usize,
    static_limits_seen: usize,
    static_limits_required: bool,
    promotion_policy: Option<String>,
    warnings: Vec<Notice>,
    next_action: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct Notice {
    kind: String,
    message: String,
    source_artifact: Option<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ArtifactStatus {
    Missing,
    Loaded,
    Invalid,
}

#[derive(Clone, Debug)]
struct ArtifactParse {
    label: &'static str,
    path: Option<String>,
    status: ArtifactStatus,
    value: Option<Value>,
    notice: Option<Notice>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct GateFacts {
    mode: Option<String>,
    status: Option<String>,
    blocking: usize,
    acknowledged: usize,
    advisory: usize,
    suppressed: usize,
    not_applicable: usize,
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
struct PreviewScan {
    languages: BTreeSet<String>,
    preview_findings: usize,
    static_limits_seen: usize,
    missing_language_status: usize,
}

pub(crate) fn build_policy_readiness_report(input: PolicyReadinessInput) -> PolicyReadinessReport {
    let gate = parse_optional_json(
        "gate_decision",
        input.gate_decision_path.clone(),
        input.gate_decision_json,
    );
    let baseline = parse_optional_json(
        "baseline_delta",
        input.baseline_delta_path.clone(),
        input.baseline_delta_json,
    );
    let recommendation = parse_optional_json(
        "recommendation_calibration",
        input.recommendation_calibration_path.clone(),
        input.recommendation_calibration_json,
    );
    let mutation = parse_optional_json(
        "mutation_calibration",
        input.mutation_calibration_path.clone(),
        input.mutation_calibration_json,
    );
    let waiver = parse_optional_json(
        "waiver_aging",
        input.waiver_aging_path.clone(),
        input.waiver_aging_json,
    );
    let suppression = parse_optional_json(
        "suppression_health",
        input.suppression_health_path.clone(),
        input.suppression_health_json,
    );
    let repo_config = parse_optional_json(
        "repo_config",
        input.repo_config_path.clone(),
        input.repo_config_json,
    );
    let previous = parse_optional_json(
        "previous_readiness",
        input.previous_readiness_path.clone(),
        input.previous_readiness_json,
    );

    let artifacts = [
        &gate,
        &baseline,
        &recommendation,
        &mutation,
        &waiver,
        &suppression,
        &repo_config,
        &previous,
    ];

    let mut warnings = artifacts
        .iter()
        .filter_map(|artifact| artifact.notice.clone())
        .collect::<Vec<_>>();
    let unknowns = artifacts
        .iter()
        .filter(|artifact| artifact.status == ArtifactStatus::Missing)
        .map(|artifact| Notice {
            kind: "missing_input".to_string(),
            message: format!("{} input not supplied.", artifact.label),
            source_artifact: artifact.path.clone(),
        })
        .collect::<Vec<_>>();

    let mut preview = PreviewScan::default();
    for artifact in artifacts {
        if let Some(value) = artifact.value.as_ref() {
            scan_preview(value, &mut preview);
        }
    }

    let gate_facts = gate.value.as_ref().map(gate_facts).unwrap_or_default();
    let baseline_facts = baseline
        .value
        .as_ref()
        .map(baseline_facts)
        .unwrap_or_default();

    let blocking_readiness = blocking_axis(&gate, &gate_facts);
    let baseline_health = baseline_axis(&baseline, &baseline_facts);
    let waiver_health = generic_axis(
        &waiver,
        "Waiver aging is available; repeated waivers stay visible as signals.",
        "Add waiver-aging input before requiring acknowledgement.",
    );
    let suppression_health = suppression_health_axis(&suppression);
    let calibration_health = calibration_axis(&recommendation, &mutation);
    let preview_evidence_boundary = preview_axis(preview);
    warnings.extend(suppression_health.warnings.clone());
    warnings.extend(preview_evidence_boundary.warnings.clone());

    let has_config_error = artifacts
        .iter()
        .any(|artifact| artifact.status == ArtifactStatus::Invalid)
        || suppression_health.state == "config_error";
    let preview_boundary_healthy = preview_evidence_boundary.missing_language_status == 0;
    let baseline_delta_healthy = baseline_facts.stale == 0
        && baseline_facts.invalid == 0
        && baseline_facts.missing_input == 0;
    let suppression_health_ready = suppression_health.state == "healthy";
    let visible_only_ready = gate.status == ArtifactStatus::Loaded && preview_boundary_healthy;
    let acknowledgeable_ready = visible_only_ready
        && waiver.status == ArtifactStatus::Loaded
        && suppression.status == ArtifactStatus::Loaded
        && suppression_health_ready;
    let baseline_check_ready = visible_only_ready
        && baseline.status == ArtifactStatus::Loaded
        && baseline_delta_healthy
        && !baseline_facts.auto_adopt_new
        && preview_evidence_boundary.preview_findings_gate_eligible == 0;
    let calibrated_gate_ready = baseline_check_ready
        && recommendation.status == ArtifactStatus::Loaded
        && mutation.status != ArtifactStatus::Invalid
        && preview_evidence_boundary.preview_findings_calibrated_confidence == 0;

    let status = readiness_status(
        has_config_error,
        calibrated_gate_ready,
        baseline_check_ready,
        acknowledgeable_ready,
        visible_only_ready,
    );
    let recommended_mode = recommended_mode_for_status(&status).to_string();
    let next_policy_action = next_policy_action_for_status(&status).to_string();

    PolicyReadinessReport {
        root: input.root,
        generated_at: input.generated_at,
        status,
        recommended_mode,
        inputs: PolicyReadinessInputs {
            gate_decision: input.gate_decision_path,
            baseline_delta: input.baseline_delta_path,
            recommendation_calibration: input.recommendation_calibration_path,
            mutation_calibration: input.mutation_calibration_path,
            waiver_aging: input.waiver_aging_path,
            suppression_health: input.suppression_health_path,
            repo_config: input.repo_config_path,
            previous_readiness: input.previous_readiness_path,
        },
        summary: PolicyReadinessSummary {
            blocking_ready: baseline_check_ready || calibrated_gate_ready,
            visible_only_ready,
            acknowledgeable_ready,
            baseline_check_ready,
            calibrated_gate_ready,
            preview_candidates: preview_evidence_boundary.preview_findings_visible,
            preview_candidates_gate_eligible: 0,
            warnings: warnings.len(),
            unknowns: unknowns.len(),
        },
        blocking_readiness,
        baseline_health,
        waiver_health,
        suppression_health,
        calibration_health,
        preview_evidence_boundary,
        unknowns,
        warnings,
        next_policy_action,
    }
}

pub(crate) fn render_policy_readiness_json(
    report: &PolicyReadinessReport,
) -> Result<String, String> {
    serde_json::to_string_pretty(&json!({
        "schema_version": SCHEMA_VERSION,
        "tool": "ripr",
        "kind": REPORT_KIND,
        "status": report.status,
        "recommended_mode": report.recommended_mode,
        "root": report.root,
        "generated_at": report.generated_at,
        "inputs": inputs_json(&report.inputs),
        "summary": summary_json(&report.summary),
        "blocking_readiness": axis_json(&report.blocking_readiness),
        "baseline_health": axis_json(&report.baseline_health),
        "waiver_health": axis_json(&report.waiver_health),
        "suppression_health": axis_json(&report.suppression_health),
        "calibration_health": axis_json(&report.calibration_health),
        "preview_evidence_boundary": preview_boundary_json(&report.preview_evidence_boundary),
        "unknowns": report.unknowns.iter().map(notice_json).collect::<Vec<_>>(),
        "warnings": report.warnings.iter().map(notice_json).collect::<Vec<_>>(),
        "next_policy_action": report.next_policy_action,
        "limits_note": LIMITS_NOTE,
        "preview_limits_note": PREVIEW_LIMITS_NOTE,
    }))
    .map_err(|err| format!("failed to render policy readiness JSON: {err}"))
}

pub(crate) fn render_policy_readiness_markdown(report: &PolicyReadinessReport) -> String {
    let mut out = String::new();
    out.push_str("# RIPR Policy Readiness\n\n");
    out.push_str(&format!("Status: {}\n", report.status));
    out.push_str(&format!(
        "Recommended mode: {}\n\n",
        report.recommended_mode
    ));
    out.push_str("| Axis | State | Next action |\n");
    out.push_str("| --- | --- | --- |\n");
    for (name, axis) in [
        ("Blocking readiness", &report.blocking_readiness),
        ("Baseline health", &report.baseline_health),
        ("Waiver health", &report.waiver_health),
        ("Suppression health", &report.suppression_health),
        ("Calibration health", &report.calibration_health),
    ] {
        out.push_str(&format!(
            "| {name} | {} | {} |\n",
            axis.state, axis.next_action
        ));
    }
    out.push_str(&format!(
        "| Preview evidence boundary | {} | {} |\n",
        report.preview_evidence_boundary.state, report.preview_evidence_boundary.next_action
    ));

    out.push_str("\nPreview evidence:\n");
    out.push_str(&format!(
        "- visible: {}\n",
        report.preview_evidence_boundary.preview_findings_visible
    ));
    out.push_str(&format!(
        "- gate_eligible: {}\n",
        report
            .preview_evidence_boundary
            .preview_findings_gate_eligible
    ));
    out.push_str(&format!(
        "- ripr_zero_blocking: {}\n",
        report
            .preview_evidence_boundary
            .preview_findings_ripr_zero_blocking
    ));
    out.push_str(&format!(
        "- calibrated_confidence: {}\n",
        report
            .preview_evidence_boundary
            .preview_findings_calibrated_confidence
    ));

    if !report.unknowns.is_empty() {
        out.push_str("\nUnknowns:\n");
        for unknown in &report.unknowns {
            out.push_str(&format!("- {}\n", unknown.message));
        }
    }
    if !report.warnings.is_empty() {
        out.push_str("\nWarnings:\n");
        for warning in &report.warnings {
            out.push_str(&format!("- {}\n", warning.message));
        }
    }

    out.push_str("\nNext policy action:\n");
    out.push_str(&format!("- {}\n", report.next_policy_action));
    out.push_str("\nLimits:\n");
    out.push_str(LIMITS_NOTE);
    out.push('\n');
    out.push_str(PREVIEW_LIMITS_NOTE);
    out.push('\n');
    out
}

pub(crate) fn policy_readiness_status(report: &PolicyReadinessReport) -> &str {
    &report.status
}

pub(crate) fn policy_readiness_recommended_mode(report: &PolicyReadinessReport) -> &str {
    &report.recommended_mode
}

pub(crate) use crate::output::path::display_path;

fn parse_optional_json(
    label: &'static str,
    path: Option<String>,
    text: Option<Result<String, String>>,
) -> ArtifactParse {
    let Some(path_value) = path.clone() else {
        return ArtifactParse {
            label,
            path,
            status: ArtifactStatus::Missing,
            value: None,
            notice: None,
        };
    };
    let Some(text) = text else {
        return ArtifactParse {
            label,
            path,
            status: ArtifactStatus::Invalid,
            value: None,
            notice: Some(Notice {
                kind: "missing_supplied_input".to_string(),
                message: format!("supplied {label} input {path_value} was not read"),
                source_artifact: Some(path_value),
            }),
        };
    };
    let text = match text {
        Ok(text) => text,
        Err(error) => {
            return ArtifactParse {
                label,
                path,
                status: ArtifactStatus::Invalid,
                value: None,
                notice: Some(Notice {
                    kind: "invalid_input".to_string(),
                    message: format!("{label} input {path_value} is invalid: {error}"),
                    source_artifact: Some(path_value),
                }),
            };
        }
    };
    match serde_json::from_str::<Value>(&text) {
        Ok(value) => ArtifactParse {
            label,
            path,
            status: ArtifactStatus::Loaded,
            value: Some(value),
            notice: None,
        },
        Err(error) => ArtifactParse {
            label,
            path,
            status: ArtifactStatus::Invalid,
            value: None,
            notice: Some(Notice {
                kind: "invalid_json".to_string(),
                message: format!("{label} input {path_value} is invalid JSON: {error}"),
                source_artifact: Some(path_value),
            }),
        },
    }
}

fn blocking_axis(gate: &ArtifactParse, facts: &GateFacts) -> Axis {
    match gate.status {
        ArtifactStatus::Loaded => Axis {
            state: "healthy".to_string(),
            evidence: vec![
                format!(
                    "gate_status={}",
                    facts.status.as_deref().unwrap_or("unknown")
                ),
                format!(
                    "current_gate_mode={}",
                    facts.mode.as_deref().unwrap_or("unknown")
                ),
                format!("blocking_candidates={}", facts.blocking),
                format!("acknowledged={}", facts.acknowledged),
                format!("advisory={}", facts.advisory),
                format!("suppressed={}", facts.suppressed),
                format!("not_applicable={}", facts.not_applicable),
            ],
            warnings: Vec::new(),
            next_action:
                "Keep generated CI advisory unless RIPR_GATE_MODE is explicitly configured."
                    .to_string(),
        },
        ArtifactStatus::Invalid => Axis {
            state: "config_error".to_string(),
            evidence: Vec::new(),
            warnings: gate.notice.clone().into_iter().collect(),
            next_action: "Repair the supplied gate-decision input before tightening policy."
                .to_string(),
        },
        ArtifactStatus::Missing => Axis {
            state: "missing".to_string(),
            evidence: Vec::new(),
            warnings: Vec::new(),
            next_action: "Run or supply gate-decision before visible-only readiness.".to_string(),
        },
    }
}

fn baseline_axis(baseline: &ArtifactParse, facts: &BaselineFacts) -> Axis {
    match baseline.status {
        ArtifactStatus::Loaded => {
            let state = if facts.auto_adopt_new {
                "not_ready"
            } else if facts.stale > 0 || facts.invalid > 0 || facts.missing_input > 0 {
                "warning"
            } else {
                "healthy"
            };
            Axis {
                state: state.to_string(),
                evidence: vec![
                    format!("still_present={}", facts.still_present),
                    format!("resolved={}", facts.resolved),
                    format!("new_policy_eligible={}", facts.new_policy_eligible),
                    format!("acknowledged={}", facts.acknowledged),
                    format!("suppressed={}", facts.suppressed),
                    format!("stale={}", facts.stale),
                    format!("invalid={}", facts.invalid),
                    format!("missing_input={}", facts.missing_input),
                    format!("auto_adopt_new={}", facts.auto_adopt_new),
                ],
                warnings: Vec::new(),
                next_action: if facts.auto_adopt_new {
                    "Disable auto-adopt-new; baseline refresh must be shrink-only."
                } else {
                    "Use baseline-check only with the reviewed baseline path supplied."
                }
                .to_string(),
            }
        }
        ArtifactStatus::Invalid => Axis {
            state: "config_error".to_string(),
            evidence: Vec::new(),
            warnings: baseline.notice.clone().into_iter().collect(),
            next_action: "Repair the supplied baseline-delta input before baseline-check."
                .to_string(),
        },
        ArtifactStatus::Missing => Axis {
            state: "missing".to_string(),
            evidence: Vec::new(),
            warnings: Vec::new(),
            next_action: "Create a reviewed baseline debt delta before baseline-check.".to_string(),
        },
    }
}

fn generic_axis(artifact: &ArtifactParse, loaded_evidence: &str, missing_action: &str) -> Axis {
    match artifact.status {
        ArtifactStatus::Loaded => Axis {
            state: "healthy".to_string(),
            evidence: vec![loaded_evidence.to_string()],
            warnings: Vec::new(),
            next_action: "Keep this evidence visible in policy reports.".to_string(),
        },
        ArtifactStatus::Invalid => Axis {
            state: "config_error".to_string(),
            evidence: Vec::new(),
            warnings: artifact.notice.clone().into_iter().collect(),
            next_action: format!("Repair the supplied {} input.", artifact.label),
        },
        ArtifactStatus::Missing => Axis {
            state: "missing".to_string(),
            evidence: Vec::new(),
            warnings: Vec::new(),
            next_action: missing_action.to_string(),
        },
    }
}

fn suppression_health_axis(artifact: &ArtifactParse) -> Axis {
    match artifact.status {
        ArtifactStatus::Loaded => {
            let value = artifact.value.as_ref();
            let status = value
                .and_then(|value| string_path(value, &["status"]))
                .unwrap_or_else(|| "unknown".to_string());
            let suppressions = value
                .map(|value| usize_path(value, &["summary", "suppressions"]))
                .unwrap_or(0);
            let missing_owner = value
                .map(|value| usize_path(value, &["summary", "missing_owner"]))
                .unwrap_or(0);
            let missing_reason = value
                .map(|value| usize_path(value, &["summary", "missing_reason"]))
                .unwrap_or(0);
            let stale = value
                .map(|value| usize_path(value, &["summary", "stale"]))
                .unwrap_or(0);
            let overbroad_scope = value
                .map(|value| usize_path(value, &["summary", "overbroad_scope"]))
                .unwrap_or(0);
            let unknown_selector = value
                .map(|value| usize_path(value, &["summary", "unknown_selector"]))
                .unwrap_or(0);
            let preview_without_preview_label = value
                .map(|value| usize_path(value, &["summary", "preview_without_preview_label"]))
                .unwrap_or(0);
            let config_errors = value
                .map(|value| usize_path(value, &["summary", "config_errors"]))
                .unwrap_or(0);
            let warnings = value
                .map(|value| usize_path(value, &["summary", "warnings"]))
                .unwrap_or(0);

            let state = match status.as_str() {
                "healthy" | "no_suppressions" => "healthy",
                "config_error" => "config_error",
                "warning" => "warning",
                _ => "warning",
            };
            let mut notices = Vec::new();
            if state == "warning" {
                notices.push(Notice {
                    kind: "suppression_health_warning".to_string(),
                    message: "suppression-health reports durable exception metadata warnings."
                        .to_string(),
                    source_artifact: artifact.path.clone(),
                });
            } else if state == "config_error" {
                notices.push(Notice {
                    kind: "suppression_health_config_error".to_string(),
                    message: "suppression-health reports malformed durable exception metadata."
                        .to_string(),
                    source_artifact: artifact.path.clone(),
                });
            }
            Axis {
                state: state.to_string(),
                evidence: vec![
                    format!("suppression_health_status={status}"),
                    format!("suppressions={suppressions}"),
                    format!("missing_owner={missing_owner}"),
                    format!("missing_reason={missing_reason}"),
                    format!("stale={stale}"),
                    format!("overbroad_scope={overbroad_scope}"),
                    format!("unknown_selector={unknown_selector}"),
                    format!("preview_without_preview_label={preview_without_preview_label}"),
                    format!("warnings={warnings}"),
                    format!("config_errors={config_errors}"),
                ],
                warnings: notices,
                next_action: match state {
                    "healthy" => "Keep suppressions visible with owner and reason.",
                    "config_error" => {
                        "Repair malformed suppression metadata before tightening policy."
                    }
                    _ => "Review suppression-health warnings before tightening policy.",
                }
                .to_string(),
            }
        }
        ArtifactStatus::Invalid => Axis {
            state: "config_error".to_string(),
            evidence: Vec::new(),
            warnings: artifact.notice.clone().into_iter().collect(),
            next_action: "Repair the supplied suppression-health input.".to_string(),
        },
        ArtifactStatus::Missing => Axis {
            state: "missing".to_string(),
            evidence: Vec::new(),
            warnings: Vec::new(),
            next_action: "Add suppression-health input before tightening policy.".to_string(),
        },
    }
}

fn calibration_axis(recommendation: &ArtifactParse, mutation: &ArtifactParse) -> Axis {
    if recommendation.status == ArtifactStatus::Invalid
        || mutation.status == ArtifactStatus::Invalid
    {
        let mut warnings = Vec::new();
        warnings.extend(recommendation.notice.clone());
        warnings.extend(mutation.notice.clone());
        return Axis {
            state: "config_error".to_string(),
            evidence: Vec::new(),
            warnings,
            next_action: "Repair supplied calibration inputs before calibrated-gate.".to_string(),
        };
    }
    if recommendation.status == ArtifactStatus::Loaded {
        let mutation_state = if mutation.status == ArtifactStatus::Loaded {
            "supplied"
        } else {
            "missing"
        };
        Axis {
            state: if mutation.status == ArtifactStatus::Loaded {
                "healthy"
            } else {
                "warning"
            }
            .to_string(),
            evidence: vec![
                "recommendation_calibration=supplied".to_string(),
                format!("mutation_calibration={mutation_state}"),
            ],
            warnings: Vec::new(),
            next_action: "Use calibrated-gate only for same-class stable evidence.".to_string(),
        }
    } else {
        Axis {
            state: "not_ready".to_string(),
            evidence: Vec::new(),
            warnings: Vec::new(),
            next_action: "Collect same-class recommendation calibration before calibrated-gate."
                .to_string(),
        }
    }
}

fn preview_axis(scan: PreviewScan) -> PreviewEvidenceBoundary {
    let warnings = if scan.missing_language_status > 0 {
        vec![Notice {
            kind: "preview_language_status_missing".to_string(),
            message: format!(
                "{} non-Rust language findings are missing language_status=preview.",
                scan.missing_language_status
            ),
            source_artifact: None,
        }]
    } else {
        Vec::new()
    };
    let state = if scan.missing_language_status > 0 {
        "warning"
    } else {
        "healthy"
    };
    PreviewEvidenceBoundary {
        state: state.to_string(),
        preview_languages: scan.languages.into_iter().collect(),
        preview_findings_visible: scan.preview_findings,
        preview_findings_acknowledgeable: scan.preview_findings,
        preview_findings_suppressible: scan.preview_findings,
        preview_findings_baseline_advisory: scan.preview_findings,
        preview_findings_gate_eligible: 0,
        preview_findings_ripr_zero_blocking: 0,
        preview_findings_calibrated_confidence: 0,
        missing_language_status: scan.missing_language_status,
        static_limits_seen: scan.static_limits_seen,
        static_limits_required: scan.preview_findings > 0,
        promotion_policy: None,
        warnings,
        next_action: if scan.preview_findings > 0 {
            "Keep preview evidence advisory until an explicit promotion policy exists."
        } else {
            "No preview evidence is present; preserve the stable Rust policy boundary."
        }
        .to_string(),
    }
}

fn readiness_status(
    has_config_error: bool,
    calibrated_gate_ready: bool,
    baseline_check_ready: bool,
    acknowledgeable_ready: bool,
    visible_only_ready: bool,
) -> String {
    if has_config_error {
        "config_error"
    } else if calibrated_gate_ready {
        "ready_for_calibrated_gate"
    } else if baseline_check_ready {
        "ready_for_baseline_check"
    } else if acknowledgeable_ready {
        "ready_for_acknowledgeable"
    } else if visible_only_ready {
        "ready_for_visible_only"
    } else {
        "advisory_only"
    }
    .to_string()
}

fn recommended_mode_for_status(status: &str) -> &'static str {
    match status {
        "ready_for_calibrated_gate" => "calibrated-gate",
        "ready_for_baseline_check" => "baseline-check",
        "ready_for_acknowledgeable" => "acknowledgeable",
        "ready_for_visible_only" => "visible-only",
        _ => "advisory-only",
    }
}

fn next_policy_action_for_status(status: &str) -> &'static str {
    match status {
        "config_error" => "Repair invalid supplied inputs before using readiness recommendations.",
        "ready_for_calibrated_gate" => {
            "Consider an explicit calibrated-gate only for narrow stable Rust evidence."
        }
        "ready_for_baseline_check" => {
            "Enable baseline-check for stable Rust evidence only; keep preview evidence advisory."
        }
        "ready_for_acknowledgeable" => {
            "Use acknowledgement only with visible waiver and suppression evidence."
        }
        "ready_for_visible_only" => {
            "Render policy decisions without pass/fail authority while collecting baseline evidence."
        }
        _ => {
            "Stay advisory while collecting gate, baseline, waiver, suppression, and calibration inputs."
        }
    }
}

fn gate_facts(value: &Value) -> GateFacts {
    GateFacts {
        mode: string_field(value.get("mode")).or_else(|| string_path(value, &["policy", "mode"])),
        status: string_field(value.get("status")),
        blocking: usize_path(value, &["summary", "blocking"]),
        acknowledged: usize_path(value, &["summary", "acknowledged"]),
        advisory: usize_path(value, &["summary", "advisory"]),
        suppressed: usize_path(value, &["summary", "suppressed"]),
        not_applicable: usize_path(value, &["summary", "not_applicable"]),
    }
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

fn scan_preview(value: &Value, scan: &mut PreviewScan) {
    match value {
        Value::Object(map) => {
            let language = map.get("language").and_then(Value::as_str);
            let language_status = map.get("language_status").and_then(Value::as_str);
            if language_status == Some("preview") {
                scan.preview_findings += 1;
                if let Some(language) = language {
                    scan.languages.insert(language.to_string());
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
                    scan.languages.insert(language.to_string());
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

fn inputs_json(inputs: &PolicyReadinessInputs) -> Value {
    json!({
        "gate_decision": inputs.gate_decision,
        "baseline_delta": inputs.baseline_delta,
        "recommendation_calibration": inputs.recommendation_calibration,
        "mutation_calibration": inputs.mutation_calibration,
        "waiver_aging": inputs.waiver_aging,
        "suppression_health": inputs.suppression_health,
        "repo_config": inputs.repo_config,
        "previous_readiness": inputs.previous_readiness,
    })
}

fn summary_json(summary: &PolicyReadinessSummary) -> Value {
    json!({
        "blocking_ready": summary.blocking_ready,
        "visible_only_ready": summary.visible_only_ready,
        "acknowledgeable_ready": summary.acknowledgeable_ready,
        "baseline_check_ready": summary.baseline_check_ready,
        "calibrated_gate_ready": summary.calibrated_gate_ready,
        "preview_candidates": summary.preview_candidates,
        "preview_candidates_gate_eligible": summary.preview_candidates_gate_eligible,
        "warnings": summary.warnings,
        "unknowns": summary.unknowns,
    })
}

fn axis_json(axis: &Axis) -> Value {
    json!({
        "state": axis.state,
        "evidence": axis.evidence,
        "warnings": axis.warnings.iter().map(notice_json).collect::<Vec<_>>(),
        "next_action": axis.next_action,
    })
}

fn preview_boundary_json(boundary: &PreviewEvidenceBoundary) -> Value {
    json!({
        "state": boundary.state,
        "preview_languages": boundary.preview_languages,
        "preview_findings_visible": boundary.preview_findings_visible,
        "preview_findings_acknowledgeable": boundary.preview_findings_acknowledgeable,
        "preview_findings_suppressible": boundary.preview_findings_suppressible,
        "preview_findings_baseline_advisory": boundary.preview_findings_baseline_advisory,
        "preview_findings_gate_eligible": boundary.preview_findings_gate_eligible,
        "preview_findings_ripr_zero_blocking": boundary.preview_findings_ripr_zero_blocking,
        "preview_findings_calibrated_confidence": boundary.preview_findings_calibrated_confidence,
        "missing_language_status": boundary.missing_language_status,
        "static_limits_seen": boundary.static_limits_seen,
        "static_limits_required": boundary.static_limits_required,
        "promotion_policy": boundary.promotion_policy,
        "warnings": boundary.warnings.iter().map(notice_json).collect::<Vec<_>>(),
        "next_action": boundary.next_action,
    })
}

fn notice_json(notice: &Notice) -> Value {
    json!({
        "kind": notice.kind,
        "message": notice.message,
        "source_artifact": notice.source_artifact,
    })
}

fn string_path(value: &Value, path: &[&str]) -> Option<String> {
    path_value(value, path).and_then(string_value)
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

fn string_field(value: Option<&Value>) -> Option<String> {
    value.and_then(string_value)
}

fn string_value(value: &Value) -> Option<String> {
    value.as_str().map(ToString::to_string)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn input() -> PolicyReadinessInput {
        PolicyReadinessInput {
            root: ".".to_string(),
            generated_at: "unix_ms:1".to_string(),
            gate_decision_path: None,
            baseline_delta_path: None,
            recommendation_calibration_path: None,
            mutation_calibration_path: None,
            waiver_aging_path: None,
            suppression_health_path: None,
            repo_config_path: None,
            previous_readiness_path: None,
            gate_decision_json: None,
            baseline_delta_json: None,
            recommendation_calibration_json: None,
            mutation_calibration_json: None,
            waiver_aging_json: None,
            suppression_health_json: None,
            repo_config_json: None,
            previous_readiness_json: None,
        }
    }

    fn supply_gate(input: &mut PolicyReadinessInput, body: &str) {
        input.gate_decision_path = Some("target/ripr/reports/gate-decision.json".to_string());
        input.gate_decision_json = Some(Ok(body.to_string()));
    }

    fn supply_baseline(input: &mut PolicyReadinessInput, body: &str) {
        input.baseline_delta_path =
            Some("target/ripr/reports/baseline-debt-delta.json".to_string());
        input.baseline_delta_json = Some(Ok(body.to_string()));
    }

    fn gate_body(decisions: &str) -> String {
        format!(
            r#"{{
              "schema_version": "0.1",
              "status": "advisory",
              "policy": {{"mode": "visible-only"}},
              "summary": {{"blocking": 1, "acknowledged": 2, "advisory": 3, "suppressed": 4, "not_applicable": 5}},
              "decisions": {decisions}
            }}"#
        )
    }

    fn clean_baseline_body() -> &'static str {
        r#"{
          "schema_version": "0.1",
          "kind": "baseline_debt_delta",
          "delta": {"still_present": 2, "resolved": 1, "new_policy_eligible": 0, "acknowledged": 0, "suppressed": 0, "stale_baseline_entry": 0, "invalid_baseline_entry": 0, "missing_current_input": 0}
        }"#
    }

    fn healthy_suppression_health_body() -> &'static str {
        r#"{
          "schema_version": "0.1",
          "kind": "suppression_health",
          "status": "healthy",
          "summary": {"suppressions": 1, "missing_owner": 0, "missing_reason": 0, "stale": 0, "overbroad_scope": 0, "unknown_selector": 0, "preview_without_preview_label": 0, "warnings": 0, "config_errors": 0}
        }"#
    }

    #[test]
    fn missing_inputs_keep_report_advisory() -> Result<(), String> {
        let report = build_policy_readiness_report(input());
        assert_eq!(report.status, "advisory_only");
        assert_eq!(report.recommended_mode, "advisory-only");
        assert_eq!(report.unknowns.len(), 8);
        let rendered = render_policy_readiness_json(&report)?;
        assert!(rendered.contains("\"kind\": \"policy_readiness\""));
        assert!(rendered.contains("\"recommended_mode\": \"advisory-only\""));
        Ok(())
    }

    #[test]
    fn gate_and_baseline_inputs_enable_baseline_check_readiness() -> Result<(), String> {
        let mut input = input();
        supply_gate(
            &mut input,
            r#"{
              "schema_version": "0.1",
              "status": "advisory",
              "mode": "visible-only",
              "summary": {"blocking": 0, "acknowledged": 0, "advisory": 1, "suppressed": 0, "not_applicable": 0},
              "decisions": [{
                "decision": "advisory",
                "language": "typescript",
                "language_status": "preview",
                "static_limit_kind": "dynamic_dispatch"
              }]
            }"#,
        );
        supply_baseline(&mut input, clean_baseline_body());

        let report = build_policy_readiness_report(input);
        assert_eq!(report.status, "ready_for_baseline_check");
        assert_eq!(report.recommended_mode, "baseline-check");
        assert_eq!(report.preview_evidence_boundary.preview_findings_visible, 1);
        assert_eq!(report.preview_evidence_boundary.static_limits_seen, 1);
        assert_eq!(
            report
                .preview_evidence_boundary
                .preview_findings_gate_eligible,
            0
        );
        assert_eq!(
            report
                .preview_evidence_boundary
                .preview_findings_ripr_zero_blocking,
            0
        );
        assert_eq!(
            report
                .preview_evidence_boundary
                .preview_findings_calibrated_confidence,
            0
        );
        let rendered = render_policy_readiness_markdown(&report);
        assert!(rendered.contains("Recommended mode: baseline-check"));
        assert!(rendered.contains("gate_eligible: 0"));
        Ok(())
    }

    #[test]
    fn gate_only_is_visible_only_ready_and_preserves_gate_facts() -> Result<(), String> {
        let mut input = input();
        supply_gate(&mut input, &gate_body("[]"));

        let report = build_policy_readiness_report(input);

        assert_eq!(report.status, "ready_for_visible_only");
        assert_eq!(report.recommended_mode, "visible-only");
        assert_eq!(report.blocking_readiness.state, "healthy");
        assert!(report.summary.visible_only_ready);
        assert!(!report.summary.baseline_check_ready);
        assert!(
            report
                .blocking_readiness
                .evidence
                .contains(&"gate_status=advisory".to_string())
        );
        assert!(
            report
                .blocking_readiness
                .evidence
                .contains(&"current_gate_mode=visible-only".to_string())
        );
        assert!(
            report
                .blocking_readiness
                .evidence
                .contains(&"blocking_candidates=1".to_string())
        );
        let rendered = render_policy_readiness_json(&report)?;
        assert!(rendered.contains("\"status\": \"ready_for_visible_only\""));
        Ok(())
    }

    #[test]
    fn waiver_and_suppression_inputs_enable_acknowledgeable_readiness() {
        let mut input = input();
        supply_gate(&mut input, &gate_body("[]"));
        input.waiver_aging_path = Some("target/ripr/reports/waiver-aging.json".to_string());
        input.waiver_aging_json = Some(Ok(r#"{"schema_version":"0.1"}"#.to_string()));
        input.suppression_health_path =
            Some("target/ripr/reports/suppression-health.json".to_string());
        input.suppression_health_json = Some(Ok(healthy_suppression_health_body().to_string()));

        let report = build_policy_readiness_report(input);

        assert_eq!(report.status, "ready_for_acknowledgeable");
        assert_eq!(report.recommended_mode, "acknowledgeable");
        assert!(report.summary.acknowledgeable_ready);
        assert_eq!(report.waiver_health.state, "healthy");
        assert_eq!(report.suppression_health.state, "healthy");
        assert_eq!(
            report.next_policy_action,
            "Use acknowledgement only with visible waiver and suppression evidence."
        );
    }

    #[test]
    fn suppression_health_warning_blocks_acknowledgeable_readiness() {
        let mut input = input();
        supply_gate(&mut input, &gate_body("[]"));
        input.waiver_aging_path = Some("target/ripr/reports/waiver-aging.json".to_string());
        input.waiver_aging_json = Some(Ok(r#"{"schema_version":"0.1"}"#.to_string()));
        input.suppression_health_path =
            Some("target/ripr/reports/suppression-health.json".to_string());
        input.suppression_health_json = Some(Ok(
            r#"{
              "schema_version": "0.1",
              "kind": "suppression_health",
              "status": "warning",
              "summary": {"suppressions": 1, "missing_owner": 0, "missing_reason": 0, "stale": 1, "overbroad_scope": 0, "unknown_selector": 0, "preview_without_preview_label": 0, "warnings": 1, "config_errors": 0}
            }"#
            .to_string(),
        ));

        let report = build_policy_readiness_report(input);

        assert_eq!(report.status, "ready_for_visible_only");
        assert!(!report.summary.acknowledgeable_ready);
        assert_eq!(report.suppression_health.state, "warning");
        assert_eq!(report.warnings.len(), 1);
    }

    #[test]
    fn calibration_input_promotes_baseline_check_to_calibrated_gate() {
        let mut input = input();
        supply_gate(&mut input, &gate_body("[]"));
        supply_baseline(&mut input, clean_baseline_body());
        input.recommendation_calibration_path =
            Some("target/ripr/reports/recommendation-calibration.json".to_string());
        input.recommendation_calibration_json = Some(Ok(r#"{"schema_version":"0.1"}"#.to_string()));
        input.mutation_calibration_path =
            Some("target/ripr/reports/mutation-calibration.json".to_string());
        input.mutation_calibration_json = Some(Ok(r#"{"schema_version":"0.1"}"#.to_string()));

        let report = build_policy_readiness_report(input);

        assert_eq!(report.status, "ready_for_calibrated_gate");
        assert_eq!(report.recommended_mode, "calibrated-gate");
        assert!(report.summary.calibrated_gate_ready);
        assert_eq!(report.calibration_health.state, "healthy");
        assert!(
            report
                .calibration_health
                .evidence
                .contains(&"mutation_calibration=supplied".to_string())
        );
    }

    #[test]
    fn auto_adopt_new_keeps_baseline_check_not_ready() {
        let mut input = input();
        supply_gate(&mut input, &gate_body("[]"));
        supply_baseline(
            &mut input,
            r#"{
              "schema_version": "0.1",
              "policy": {"auto_adopt_new": true},
              "delta": {"new_policy_eligible": 0}
            }"#,
        );

        let report = build_policy_readiness_report(input);

        assert_eq!(report.status, "ready_for_visible_only");
        assert!(!report.summary.baseline_check_ready);
        assert_eq!(report.baseline_health.state, "not_ready");
        assert_eq!(
            report.baseline_health.next_action,
            "Disable auto-adopt-new; baseline refresh must be shrink-only."
        );
    }

    #[test]
    fn preview_language_without_status_is_warning_not_gate_eligibility() {
        let mut input = input();
        supply_gate(
            &mut input,
            &gate_body(
                r#"[{
                  "decision": "advisory",
                  "language": "python",
                  "static_limit_kind": "dynamic_dispatch"
                }]"#,
            ),
        );

        let report = build_policy_readiness_report(input);

        assert_eq!(report.status, "advisory_only");
        assert_eq!(report.recommended_mode, "advisory-only");
        assert_eq!(report.preview_evidence_boundary.state, "warning");
        assert_eq!(report.preview_evidence_boundary.missing_language_status, 1);
        assert_eq!(
            report
                .preview_evidence_boundary
                .preview_findings_gate_eligible,
            0
        );
        assert_eq!(report.warnings.len(), 1);
        assert_eq!(report.warnings[0].kind, "preview_language_status_missing");
    }

    #[test]
    fn supplied_input_read_failures_are_config_errors() {
        let mut input = input();
        input.gate_decision_path = Some("target/ripr/reports/gate-decision.json".to_string());
        input.gate_decision_json = Some(Err("read failed".to_string()));
        input.baseline_delta_path =
            Some("target/ripr/reports/baseline-debt-delta.json".to_string());

        let report = build_policy_readiness_report(input);

        assert_eq!(report.status, "config_error");
        assert_eq!(report.blocking_readiness.state, "config_error");
        assert_eq!(report.baseline_health.state, "config_error");
        assert_eq!(report.warnings.len(), 2);
        assert!(
            report
                .warnings
                .iter()
                .any(|warning| warning.kind == "invalid_input")
        );
        assert!(
            report
                .warnings
                .iter()
                .any(|warning| warning.kind == "missing_supplied_input")
        );
    }

    #[test]
    fn baseline_alias_counters_surface_warning_state() {
        let mut input = input();
        supply_gate(&mut input, &gate_body("[]"));
        supply_baseline(
            &mut input,
            r#"{
              "schema_version": "0.1",
              "baseline": {"auto_adopt_new": false},
              "debt_delta": {
                "still_present": 4,
                "resolved": 3,
                "new_policy_eligible": 2,
                "acknowledged": 1,
                "suppressed": 1,
                "stale": 1,
                "invalid": 1,
                "missing_input": 1
              }
            }"#,
        );

        let report = build_policy_readiness_report(input);

        assert_eq!(report.status, "ready_for_visible_only");
        assert!(!report.summary.baseline_check_ready);
        assert_eq!(report.baseline_health.state, "warning");
        for evidence in [
            "still_present=4",
            "resolved=3",
            "new_policy_eligible=2",
            "acknowledged=1",
            "suppressed=1",
            "stale=1",
            "invalid=1",
            "missing_input=1",
            "auto_adopt_new=false",
        ] {
            assert!(
                report
                    .baseline_health
                    .evidence
                    .contains(&evidence.to_string())
            );
        }
    }

    #[test]
    fn invalid_calibration_input_is_config_error() {
        let mut input = input();
        input.recommendation_calibration_path =
            Some("target/ripr/reports/recommendation-calibration.json".to_string());
        input.recommendation_calibration_json = Some(Ok(r#"{"schema_version":"0.1"}"#.to_string()));
        input.mutation_calibration_path =
            Some("target/ripr/reports/mutation-calibration.json".to_string());
        input.mutation_calibration_json = Some(Ok("{".to_string()));

        let report = build_policy_readiness_report(input);

        assert_eq!(report.status, "config_error");
        assert_eq!(report.calibration_health.state, "config_error");
        assert_eq!(report.calibration_health.warnings.len(), 1);
        assert!(
            report.calibration_health.warnings[0]
                .message
                .contains("invalid JSON")
        );
    }

    #[test]
    fn invalid_supplied_input_is_config_error() -> Result<(), String> {
        let mut input = input();
        input.gate_decision_path = Some("target/ripr/reports/gate-decision.json".to_string());
        input.gate_decision_json = Some(Ok("{".to_string()));
        let report = build_policy_readiness_report(input);
        assert_eq!(report.status, "config_error");
        assert_eq!(report.recommended_mode, "advisory-only");
        assert_eq!(report.warnings.len(), 1);
        let rendered = render_policy_readiness_json(&report)?;
        assert!(rendered.contains("\"status\": \"config_error\""));
        assert!(rendered.contains("invalid JSON"));
        Ok(())
    }
}
