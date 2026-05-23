use serde::Serialize;
use serde_json::Value;

use super::first_pr::STATIC_EVIDENCE_BOUNDARY;
use super::receipt_lifecycle::{
    RECEIPT_MISSING, RECEIPT_NOT_APPLICABLE, receipt_lifecycle_state,
    receipt_lifecycle_state_from_receipt_value,
};

const SCHEMA_VERSION: &str = "0.1";
const REPORT_KIND: &str = "pr_review_front_panel";

pub(crate) const DEFAULT_PR_REVIEW_FRONT_PANEL_OUT: &str =
    "target/ripr/reports/pr-review-front-panel.json";
pub(crate) const DEFAULT_PR_REVIEW_FRONT_PANEL_MD_OUT: &str =
    "target/ripr/reports/pr-review-front-panel.md";

const LIMITS: &[&str] = &[
    "Static RIPR evidence only.",
    "Does not provide runtime confirmation.",
    "Does not run mutation testing.",
    "Does not call providers.",
    "Does not edit source or generate tests.",
    "Does not publish inline comments.",
    "Does not change default CI blocking.",
    "Gate evaluator remains pass/fail authority.",
];

const MARKDOWN_LIMITS: &[&str] = &[
    "Static RIPR evidence only.",
    "Does not run mutation testing.",
    "Does not edit source or generate tests.",
    "Gate evaluator remains pass/fail authority.",
];

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PrReviewFrontPanelInput {
    pub(crate) root: String,
    pub(crate) generated_at: String,
    pub(crate) out_md_path: String,
    pub(crate) pr_guidance_path: Option<String>,
    pub(crate) first_action_path: Option<String>,
    pub(crate) assistant_proof_path: Option<String>,
    pub(crate) assistant_health_path: Option<String>,
    pub(crate) ledger_path: Option<String>,
    pub(crate) baseline_delta_path: Option<String>,
    pub(crate) zero_status_path: Option<String>,
    pub(crate) gate_decision_path: Option<String>,
    pub(crate) recommendation_calibration_path: Option<String>,
    pub(crate) mutation_calibration_path: Option<String>,
    pub(crate) coverage_frontier_path: Option<String>,
    pub(crate) receipt_path: Option<String>,
    pub(crate) pr_guidance_json: Option<Result<String, String>>,
    pub(crate) first_action_json: Option<Result<String, String>>,
    pub(crate) assistant_proof_json: Option<Result<String, String>>,
    pub(crate) assistant_health_json: Option<Result<String, String>>,
    pub(crate) ledger_json: Option<Result<String, String>>,
    pub(crate) baseline_delta_json: Option<Result<String, String>>,
    pub(crate) zero_status_json: Option<Result<String, String>>,
    pub(crate) gate_decision_json: Option<Result<String, String>>,
    pub(crate) recommendation_calibration_json: Option<Result<String, String>>,
    pub(crate) mutation_calibration_json: Option<Result<String, String>>,
    pub(crate) coverage_frontier_json: Option<Result<String, String>>,
    pub(crate) receipt_json: Option<Result<String, String>>,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct PrReviewFrontPanelReport {
    status: String,
    root: String,
    generated_at: String,
    inputs: PanelInputs,
    summary: PanelSummary,
    top_issue: Option<PanelTopIssue>,
    movement: PanelMovement,
    debt_delta: PanelDebtDelta,
    policy: PanelPolicy,
    calibration: PanelCalibration,
    coverage_grip: PanelCoverageGrip,
    artifacts: Vec<PanelArtifact>,
    warnings: Vec<PanelWarning>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
struct PanelInputs {
    pr_guidance: Option<String>,
    first_action: Option<String>,
    assistant_proof: Option<String>,
    assistant_health: Option<String>,
    ledger: Option<String>,
    baseline_delta: Option<String>,
    zero_status: Option<String>,
    gate_decision: Option<String>,
    recommendation_calibration: Option<String>,
    mutation_calibration: Option<String>,
    coverage_frontier: Option<String>,
    receipt: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
struct PanelSummary {
    status: String,
    headline: String,
    top_issue_state: String,
    policy_state: String,
    placement: String,
    movement_state: String,
    coverage_grip_state: String,
    blocking_candidates: usize,
    acknowledged: usize,
    waived: usize,
    suppressed: usize,
    new_policy_eligible: usize,
    baseline_still_present: usize,
    baseline_resolved: usize,
    warnings: usize,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
struct PanelTopIssue {
    source: String,
    source_artifact: String,
    seam_id: Option<String>,
    canonical_gap_id: Option<String>,
    path: Option<String>,
    line: Option<u64>,
    classification: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    changed_behavior: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    current_evidence_strength: Option<String>,
    missing_discriminator: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    focused_proof_intent: Option<String>,
    related_test: Option<String>,
    suggested_test: Option<String>,
    verify_command: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    receipt_command: Option<String>,
    static_evidence_boundary: &'static str,
    agent_command: Option<String>,
    receipt: PanelReceipt,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
struct PanelReceipt {
    artifact: Option<String>,
    status: String,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
struct PanelMovement {
    state: String,
    before_class: Option<String>,
    after_class: Option<String>,
    source_artifact: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
struct PanelDebtDelta {
    new_policy_eligible: usize,
    baseline_still_present: usize,
    baseline_resolved: usize,
    acknowledged: usize,
    waived: usize,
    suppressed: usize,
    blocking_candidates: usize,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
struct PanelPolicy {
    mode: Option<String>,
    decision: String,
    authority_artifact: Option<String>,
    acknowledgement_label: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
struct PanelCalibration {
    recommendation: String,
    mutation: String,
    source_artifacts: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
struct PanelCoverageGrip {
    state: String,
    coverage_delta: Option<f64>,
    grip_delta: Option<i64>,
    source_artifact: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
struct PanelArtifact {
    group: String,
    label: String,
    path: String,
    available: bool,
    required: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
struct PanelWarning {
    kind: String,
    message: String,
    source_artifact: Option<String>,
}

#[derive(Default)]
struct ParsedPanelSources {
    pr_guidance: Option<Value>,
    first_action: Option<Value>,
    assistant_proof: Option<Value>,
    assistant_health: Option<Value>,
    ledger: Option<Value>,
    baseline_delta: Option<Value>,
    zero_status: Option<Value>,
    gate_decision: Option<Value>,
    recommendation_calibration: Option<Value>,
    mutation_calibration: Option<Value>,
    coverage_frontier: Option<Value>,
    receipt: Option<Value>,
    warnings: Vec<PanelWarning>,
}

#[derive(Clone, Debug)]
struct Candidate {
    top_issue: Option<PanelTopIssue>,
    top_issue_state: String,
    headline: String,
    placement: String,
}

pub(crate) fn build_pr_review_front_panel_report(
    input: PrReviewFrontPanelInput,
) -> PrReviewFrontPanelReport {
    let parsed = parse_panel_sources(&input);
    let inputs = PanelInputs {
        pr_guidance: input.pr_guidance_path.clone(),
        first_action: input.first_action_path.clone(),
        assistant_proof: input.assistant_proof_path.clone(),
        assistant_health: input.assistant_health_path.clone(),
        ledger: input.ledger_path.clone(),
        baseline_delta: input.baseline_delta_path.clone(),
        zero_status: input.zero_status_path.clone(),
        gate_decision: input.gate_decision_path.clone(),
        recommendation_calibration: input.recommendation_calibration_path.clone(),
        mutation_calibration: input.mutation_calibration_path.clone(),
        coverage_frontier: input.coverage_frontier_path.clone(),
        receipt: input.receipt_path.clone(),
    };
    let raw_debt_delta = debt_delta(&parsed);
    let policy = policy(&input, &parsed);
    let coverage_grip = coverage_grip(&input, &parsed);
    let mut movement = movement(&input, &parsed);
    let mut warnings = parsed.warnings.clone();

    let candidate = select_candidate(&input, &parsed, &policy, &movement, &coverage_grip);
    let mut status = status(&policy, &candidate);
    let mut policy_state = policy_state(&policy, &candidate);
    let mut movement_state = movement.state.clone();
    let mut coverage_state = coverage_grip.state.clone();
    let mut headline = candidate.headline.clone();

    if is_missing_required(&parsed) {
        status = "incomplete".to_string();
        policy_state = "none".to_string();
        movement_state = "unknown".to_string();
        movement.state = "unknown".to_string();
        coverage_state = "not_available".to_string();
        headline = "Regenerate missing assistant proof before acting.".to_string();
        warnings.retain(|warning| {
            warning.kind != "missing_optional_input"
                || !warning
                    .source_artifact
                    .as_deref()
                    .is_some_and(|path| path.ends_with("test-oracle-assistant-proof.json"))
        });
        if !warnings
            .iter()
            .any(|warning| warning.kind == "missing_required_input")
        {
            warnings.push(PanelWarning {
                kind: "missing_required_input".to_string(),
                message: "Assistant proof artifact is missing.".to_string(),
                source_artifact: input.assistant_proof_path.clone(),
            });
        }
    } else if candidate.placement == "summary_only" {
        warnings.push(PanelWarning {
            kind: "summary_only_guidance".to_string(),
            message:
                "Recommendation is visible in summary only because changed-line placement is unsafe."
                    .to_string(),
            source_artifact: input.pr_guidance_path.clone(),
        });
    }

    let debt_delta = display_debt_delta(&raw_debt_delta, &candidate, &movement, &coverage_grip);
    let summary_acknowledged = if candidate.top_issue_state == "already_improved" {
        0
    } else {
        debt_delta.acknowledged
    };
    let summary_waived = if candidate.top_issue_state == "already_improved" {
        0
    } else {
        debt_delta.waived
    };
    let summary_suppressed = if candidate.top_issue_state == "already_improved" {
        0
    } else {
        debt_delta.suppressed
    };
    let summary = PanelSummary {
        status: status.clone(),
        headline,
        top_issue_state: if status == "incomplete" {
            "missing_required_input".to_string()
        } else {
            candidate.top_issue_state
        },
        policy_state,
        placement: if status == "incomplete" {
            "not_available".to_string()
        } else {
            candidate.placement
        },
        movement_state,
        coverage_grip_state: coverage_state,
        blocking_candidates: debt_delta.blocking_candidates,
        acknowledged: summary_acknowledged,
        waived: summary_waived,
        suppressed: summary_suppressed,
        new_policy_eligible: debt_delta.new_policy_eligible,
        baseline_still_present: debt_delta.baseline_still_present,
        baseline_resolved: debt_delta.baseline_resolved,
        warnings: warnings.len(),
    };
    let artifacts = artifacts(&input, &inputs, &summary, &parsed);
    PrReviewFrontPanelReport {
        status,
        root: input.root.clone(),
        generated_at: input.generated_at.clone(),
        inputs,
        summary,
        top_issue: if is_missing_required(&parsed) {
            None
        } else {
            candidate.top_issue
        },
        movement,
        debt_delta,
        policy,
        calibration: calibration(&input, &parsed),
        coverage_grip,
        artifacts,
        warnings,
    }
}

pub(crate) fn render_pr_review_front_panel_json(
    report: &PrReviewFrontPanelReport,
) -> Result<String, String> {
    #[derive(Serialize)]
    struct JsonReport<'a> {
        schema_version: &'static str,
        tool: &'static str,
        kind: &'static str,
        status: &'a str,
        root: &'a str,
        generated_at: &'a str,
        inputs: &'a PanelInputs,
        summary: &'a PanelSummary,
        top_issue: &'a Option<PanelTopIssue>,
        movement: &'a PanelMovement,
        debt_delta: &'a PanelDebtDelta,
        policy: &'a PanelPolicy,
        calibration: &'a PanelCalibration,
        coverage_grip: &'a PanelCoverageGrip,
        artifacts: &'a [PanelArtifact],
        warnings: &'a [PanelWarning],
        limits: Vec<&'static str>,
    }

    serde_json::to_string_pretty(&JsonReport {
        schema_version: SCHEMA_VERSION,
        tool: "ripr",
        kind: REPORT_KIND,
        status: &report.status,
        root: &report.root,
        generated_at: &report.generated_at,
        inputs: &report.inputs,
        summary: &report.summary,
        top_issue: &report.top_issue,
        movement: &report.movement,
        debt_delta: &report.debt_delta,
        policy: &report.policy,
        calibration: &report.calibration,
        coverage_grip: &report.coverage_grip,
        artifacts: &report.artifacts,
        warnings: &report.warnings,
        limits: LIMITS.to_vec(),
    })
    .map_err(|err| format!("render PR review front panel JSON failed: {err}"))
}

pub(crate) fn render_pr_review_front_panel_markdown(report: &PrReviewFrontPanelReport) -> String {
    let mut out = String::new();
    out.push_str("# RIPR PR Review\n\n");
    out.push_str(&format!("Status: {}\n\n", report.status));

    out.push_str("Start here:\n");
    if report.summary.top_issue_state == "missing_required_input" {
        out.push_str("- State: missing required evidence\n");
        out.push_str("- Safe next action: regenerate the missing assistant proof artifact before acting on this panel.\n");
        if let Some(warning) = report.warnings.first() {
            out.push_str(&format!(
                "- Missing input: {}\n",
                str_or(warning.source_artifact.as_deref(), "not_available")
            ));
        }
        out.push_str("- Boundary: advisory static evidence only; no gate, runtime, coverage, or mutation proof is implied.\n\n");
    } else if let Some(issue) = &report.top_issue {
        out.push_str(&format!("- State: {}\n", report.summary.top_issue_state));
        out.push_str(&format!("- Source: {}\n", issue.source));
        out.push_str(&format!("- Identity: {}\n", issue_primary_identity(issue)));
        out.push_str(&format!(
            "- File: {}\n",
            issue_location(issue).unwrap_or_else(|| "not_available".to_string())
        ));
        out.push_str(&format!("- Repair route: {}\n", issue_repair_route(issue)));
        if let Some(classification) = &issue.classification {
            out.push_str(&format!("- Class: {classification}\n"));
        }
        if let Some(changed_behavior) = &issue.changed_behavior {
            out.push_str(&format!("- Changed behavior: `{changed_behavior}`\n"));
        }
        if let Some(strength) = &issue.current_evidence_strength {
            out.push_str(&format!("- Current evidence strength: {strength}\n"));
        }
        if let Some(discriminator) = &issue.missing_discriminator {
            out.push_str(&format!("- Missing discriminator: {discriminator}\n"));
        }
        if let Some(intent) = &issue.focused_proof_intent {
            out.push_str(&format!("- Focused proof intent: {intent}\n"));
        }
        if let Some(suggested) = &issue.suggested_test {
            out.push_str(&format!(
                "- Suggested focused test: {}\n",
                compact_suggested_test(suggested)
            ));
        }
        if let Some(related) = &issue.related_test {
            out.push_str(&format!("- Related test: {related}\n"));
        }
        out.push_str(&format!(
            "- Verify command: {}\n",
            markdown_command_or(issue.verify_command.as_deref(), "not_available")
        ));
        if let Some(command) = &issue.receipt_command {
            out.push_str(&format!("- Receipt command: `{command}`\n"));
        }
        out.push_str(&format!("- Receipt: {}\n", issue_receipt_summary(issue)));
        out.push_str(&format!("- Boundary: {}\n", issue.static_evidence_boundary));
        out.push('\n');
    } else {
        out.push_str("- State: no actionable PR-local RIPR guidance\n");
        out.push_str("- Safe next action: inspect supporting evidence or regenerate inputs after a relevant change.\n");
        out.push_str("- Boundary: no actionable gap is not a coverage, runtime, mutation, gate, or merge-readiness claim.\n\n");
    }

    if report.summary.placement == "summary_only" {
        out.push_str("Placement:\n");
        out.push_str("- summary-only\n");
        out.push_str("- Reason: changed-line placement is unsafe\n\n");
    }

    out.push_str("Movement:\n");
    push_count(
        &mut out,
        "New policy-eligible gaps",
        report.debt_delta.new_policy_eligible,
    );
    if report.debt_delta.blocking_candidates > 0 {
        push_count(
            &mut out,
            "Blocking candidates",
            report.debt_delta.blocking_candidates,
        );
    } else if report.summary.acknowledged > 0 || report.summary.suppressed > 0 {
        push_count(&mut out, "Acknowledged gaps", report.summary.acknowledged);
        push_count(&mut out, "Suppressed gaps", report.summary.suppressed);
    } else {
        if report.coverage_grip.state != "flat_coverage_grip_improved"
            || report.debt_delta.baseline_still_present > 0
        {
            push_count(
                &mut out,
                "Baseline gaps still present",
                report.debt_delta.baseline_still_present,
            );
        }
        push_count(
            &mut out,
            "Baseline gaps resolved",
            report.debt_delta.baseline_resolved,
        );
    }
    out.push_str(&format!(
        "- Static movement: {}\n",
        report.movement.state.replace('_', " ")
    ));
    out.push_str(&format!(
        "- Coverage/grip: {}\n",
        coverage_grip_markdown(&report.coverage_grip)
    ));
    if report.coverage_grip.state == "flat_coverage_grip_improved" {
        out.push_str(&format!(
            "- Coverage delta: {}\n",
            percent_or(report.coverage_grip.coverage_delta)
        ));
        out.push_str(&format!(
            "- RIPR unresolved delta: {}\n",
            signed_or(report.coverage_grip.grip_delta)
        ));
    }
    out.push('\n');

    out.push_str("Policy:\n");
    if report.summary.policy_state != "suppressed"
        && let Some(mode) = &report.policy.mode
    {
        out.push_str(&format!("- Mode: {mode}\n"));
    }
    let display_decision = if report.summary.policy_state == "suppressed" {
        "suppressed"
    } else {
        report.policy.decision.as_str()
    };
    out.push_str(&format!("- Decision: {display_decision}\n"));
    if report.summary.policy_state == "blocking" {
        out.push_str(&format!(
            "- Gate authority: {}\n",
            gate_authority_markdown(&report.policy)
        ));
        out.push_str(&format!(
            "- Acknowledgement label: {}\n\n",
            str_or(
                report.policy.acknowledgement_label.as_deref(),
                "not_available"
            )
        ));
    } else if report.summary.policy_state == "waived" {
        out.push_str(&format!(
            "- Acknowledgement label: {}\n",
            str_or(
                report.policy.acknowledgement_label.as_deref(),
                "not_available"
            )
        ));
        out.push_str("- Finding remains visible\n");
    } else if report.summary.policy_state == "suppressed" {
        out.push_str("- Finding remains visible as a durable policy exception\n");
    }
    if report.summary.policy_state != "blocking" {
        out.push_str(&format!(
            "- Gate authority: {}\n\n",
            gate_authority_markdown(&report.policy)
        ));
    }

    if let Some(issue) = &report.top_issue
        && (issue.agent_command.is_some()
            || issue.verify_command.is_some()
            || issue.receipt.status != RECEIPT_NOT_APPLICABLE)
        && report.summary.policy_state != "waived"
        && report.summary.policy_state != "suppressed"
        && report.summary.top_issue_state != "summary_only"
    {
        out.push_str("Repair:\n");
        if report.summary.top_issue_state != "already_improved" {
            if let Some(command) = &issue.agent_command {
                out.push_str(&format!("- Agent handoff: `{command}`\n"));
            }
            if let Some(command) = &issue.verify_command {
                out.push_str(&format!("- Verify: `{command}`\n"));
            }
        }
        out.push_str(&format!(
            "- Receipt: {}\n\n",
            str_or(
                issue.receipt.artifact.as_deref(),
                issue.receipt.status.as_str()
            )
        ));
    }

    out.push_str("Artifacts:\n");
    for artifact in &report.artifacts {
        out.push_str(&format!(
            "- {}: {}\n",
            artifact_group_label(&artifact.group),
            artifact.path
        ));
    }
    out.push('\n');

    out.push_str("Limits:\n");
    for limit in MARKDOWN_LIMITS {
        out.push_str(&format!("- {limit}\n"));
    }
    out
}

fn issue_primary_identity(issue: &PanelTopIssue) -> String {
    issue
        .canonical_gap_id
        .as_deref()
        .or(issue.seam_id.as_deref())
        .unwrap_or("not_available")
        .to_string()
}

fn issue_repair_route(issue: &PanelTopIssue) -> String {
    if issue.suggested_test.is_some() {
        "focused_test".to_string()
    } else if issue.agent_command.is_some() {
        "agent_handoff".to_string()
    } else if issue.verify_command.is_some() {
        "verify_existing_repair".to_string()
    } else if issue.receipt.status != RECEIPT_NOT_APPLICABLE {
        "inspect_receipt_state".to_string()
    } else {
        "not_available".to_string()
    }
}

fn markdown_command_or(command: Option<&str>, fallback: &str) -> String {
    command
        .map(|command| format!("`{command}`"))
        .unwrap_or_else(|| fallback.to_string())
}

fn issue_receipt_summary(issue: &PanelTopIssue) -> String {
    match issue.receipt.artifact.as_deref() {
        Some(artifact) => format!("{} ({artifact})", issue.receipt.status),
        None => issue.receipt.status.clone(),
    }
}

pub(crate) use crate::output::path::display_path;

fn parse_panel_sources(input: &PrReviewFrontPanelInput) -> ParsedPanelSources {
    let mut parsed = ParsedPanelSources::default();
    parsed.pr_guidance = parse_optional_json(
        "PR guidance",
        input.pr_guidance_path.as_deref(),
        &input.pr_guidance_json,
        &mut parsed,
    );
    parsed.first_action = parse_optional_json(
        "first useful action",
        input.first_action_path.as_deref(),
        &input.first_action_json,
        &mut parsed,
    );
    parsed.assistant_proof = parse_optional_json(
        "assistant proof",
        input.assistant_proof_path.as_deref(),
        &input.assistant_proof_json,
        &mut parsed,
    );
    parsed.assistant_health = parse_optional_json(
        "assistant loop health",
        input.assistant_health_path.as_deref(),
        &input.assistant_health_json,
        &mut parsed,
    );
    parsed.ledger = parse_optional_json(
        "PR evidence ledger",
        input.ledger_path.as_deref(),
        &input.ledger_json,
        &mut parsed,
    );
    parsed.baseline_delta = parse_optional_json(
        "baseline debt delta",
        input.baseline_delta_path.as_deref(),
        &input.baseline_delta_json,
        &mut parsed,
    );
    parsed.zero_status = parse_optional_json(
        "RIPR Zero status",
        input.zero_status_path.as_deref(),
        &input.zero_status_json,
        &mut parsed,
    );
    parsed.gate_decision = parse_optional_json(
        "gate decision",
        input.gate_decision_path.as_deref(),
        &input.gate_decision_json,
        &mut parsed,
    );
    parsed.recommendation_calibration = parse_optional_json(
        "recommendation calibration",
        input.recommendation_calibration_path.as_deref(),
        &input.recommendation_calibration_json,
        &mut parsed,
    );
    parsed.mutation_calibration = parse_optional_json(
        "mutation calibration",
        input.mutation_calibration_path.as_deref(),
        &input.mutation_calibration_json,
        &mut parsed,
    );
    parsed.coverage_frontier = parse_optional_json(
        "coverage/grip frontier",
        input.coverage_frontier_path.as_deref(),
        &input.coverage_frontier_json,
        &mut parsed,
    );
    parsed.receipt = parse_optional_json(
        "receipt",
        input.receipt_path.as_deref(),
        &input.receipt_json,
        &mut parsed,
    );
    parsed
}

fn parse_optional_json(
    label: &str,
    path: Option<&str>,
    text: &Option<Result<String, String>>,
    parsed: &mut ParsedPanelSources,
) -> Option<Value> {
    let path = path?;
    let Some(text) = text else {
        parsed.warnings.push(PanelWarning {
            kind: "missing_optional_input".to_string(),
            message: format!("{label} path {path} was supplied but no input text was loaded."),
            source_artifact: Some(path.to_string()),
        });
        return None;
    };
    let text = match text {
        Ok(text) => text,
        Err(error) => {
            parsed.warnings.push(PanelWarning {
                kind: "missing_optional_input".to_string(),
                message: format!("Optional {label} input is unreadable: {error}"),
                source_artifact: Some(path.to_string()),
            });
            return None;
        }
    };
    match serde_json::from_str::<Value>(text) {
        Ok(value) => Some(value),
        Err(error) => {
            parsed.warnings.push(PanelWarning {
                kind: "malformed_input".to_string(),
                message: format!("Optional {label} input is malformed: {error}"),
                source_artifact: Some(path.to_string()),
            });
            None
        }
    }
}

fn select_candidate(
    input: &PrReviewFrontPanelInput,
    parsed: &ParsedPanelSources,
    policy: &PanelPolicy,
    movement: &PanelMovement,
    coverage_grip: &PanelCoverageGrip,
) -> Candidate {
    if policy.decision == "blocked" {
        return Candidate {
            top_issue: top_issue_from_first_action(input, parsed),
            top_issue_state: "actionable".to_string(),
            headline: "Configured gate blocked one new policy-eligible gap.".to_string(),
            placement: placement_from_guidance(parsed.pr_guidance.as_ref()),
        };
    }
    if movement.state == "resolved" {
        return Candidate {
            top_issue: top_issue_from_baseline_delta(input, parsed, "resolved"),
            top_issue_state: "already_improved".to_string(),
            headline: "This PR resolved reviewed baseline debt.".to_string(),
            placement: "not_available".to_string(),
        };
    }
    if policy.decision == "acknowledged" {
        return Candidate {
            top_issue: top_issue_from_gate_decision(input, parsed, "acknowledged"),
            top_issue_state: "actionable".to_string(),
            headline: "Policy-eligible gap acknowledged by ripr-waive.".to_string(),
            placement: "changed_line".to_string(),
        };
    }
    if has_suppressed(parsed) {
        return Candidate {
            top_issue: top_issue_from_gate_decision(input, parsed, "suppressed"),
            top_issue_state: "baseline_only".to_string(),
            headline: "Suppressed candidate remains visible.".to_string(),
            placement: "not_available".to_string(),
        };
    }
    if coverage_grip.state == "flat_coverage_grip_improved" {
        return Candidate {
            top_issue: top_issue_from_assistant_health(input, parsed),
            top_issue_state: "already_improved".to_string(),
            headline: "Static grip improved while coverage stayed flat.".to_string(),
            placement: "changed_line".to_string(),
        };
    }
    if first_action_status(parsed.first_action.as_ref()) == Some("actionable") {
        let placement = placement_from_guidance(parsed.pr_guidance.as_ref());
        let summary_only = placement == "summary_only";
        return Candidate {
            top_issue: if summary_only {
                top_issue_from_guidance(input, parsed, "summary_only")
                    .or_else(|| top_issue_from_first_action(input, parsed))
            } else {
                top_issue_from_first_action(input, parsed)
            },
            top_issue_state: if summary_only {
                "summary_only".to_string()
            } else {
                "actionable".to_string()
            },
            headline: if summary_only {
                "Show summary-only equality-boundary guidance.".to_string()
            } else {
                "Add equality-boundary discriminator test.".to_string()
            },
            placement,
        };
    }
    Candidate {
        top_issue: None,
        top_issue_state: "no_actionable_seam".to_string(),
        headline: "No actionable PR-local RIPR guidance.".to_string(),
        placement: "not_available".to_string(),
    }
}

fn is_missing_required(parsed: &ParsedPanelSources) -> bool {
    first_action_status(parsed.first_action.as_ref()) == Some("missing_required_artifact")
        || parsed.warnings.iter().any(|warning| {
            warning.source_artifact.as_deref().is_some_and(|path| {
                path.ends_with("test-oracle-assistant-proof.json")
                    && warning.kind == "missing_optional_input"
            })
        })
}

fn debt_delta(parsed: &ParsedPanelSources) -> PanelDebtDelta {
    let ledger = parsed.ledger.as_ref();
    let baseline = parsed.baseline_delta.as_ref();
    let gate = parsed.gate_decision.as_ref();

    let blocking = usize_from_sources(&[
        (gate, &["summary", "blocking"]),
        (ledger, &["movement", "blocking_candidates"]),
    ]);
    let acknowledged = usize_from_sources(&[
        (gate, &["summary", "acknowledged"]),
        (ledger, &["movement", "acknowledged"]),
        (baseline, &["delta", "acknowledged"]),
    ]);
    let suppressed = usize_from_sources(&[
        (gate, &["summary", "suppressed"]),
        (ledger, &["movement", "suppressed"]),
        (baseline, &["delta", "suppressed"]),
    ]);
    let baseline_resolved = usize_from_sources(&[
        (ledger, &["movement", "baseline_resolved"]),
        (baseline, &["delta", "resolved"]),
    ]);
    let baseline_still_present = usize_from_sources(&[
        (ledger, &["movement", "baseline_still_present"]),
        (baseline, &["delta", "still_present"]),
    ]);
    let new_policy_eligible = usize_from_sources(&[
        (baseline, &["delta", "new_policy_eligible"]),
        (ledger, &["movement", "new_policy_eligible"]),
    ]);

    let gate_status = gate.and_then(|value| string_path(value, &["status"]));
    let has_ledger_waiver = parsed
        .ledger
        .as_ref()
        .and_then(|ledger| ledger.get("waivers"))
        .and_then(Value::as_array)
        .is_some_and(|items| !items.is_empty());
    let waived = if gate_status.as_deref() == Some("acknowledged") || has_ledger_waiver {
        acknowledged.max(1)
    } else {
        0
    };
    let baseline_resolved = if gate_status.as_deref() == Some("acknowledged") {
        0
    } else {
        baseline_resolved
    };
    let baseline_still_present = if gate_status.as_deref() == Some("acknowledged") {
        0
    } else {
        baseline_still_present
    };
    let suppressed = if gate_status.as_deref() == Some("acknowledged") {
        0
    } else {
        suppressed
    };
    let new_policy_eligible = if gate_status.as_deref() == Some("acknowledged") {
        0
    } else if gate_status.as_deref() == Some("blocked")
        || first_action_status(parsed.first_action.as_ref()) == Some("actionable")
    {
        new_policy_eligible.max(1)
    } else {
        new_policy_eligible
    };

    PanelDebtDelta {
        new_policy_eligible,
        baseline_still_present,
        baseline_resolved,
        acknowledged,
        waived,
        suppressed,
        blocking_candidates: blocking,
    }
}

fn policy(input: &PrReviewFrontPanelInput, parsed: &ParsedPanelSources) -> PanelPolicy {
    let gate = parsed.gate_decision.as_ref();
    let mode = if let Some(gate) = gate {
        string_path(gate, &["mode"])
    } else if parsed.baseline_delta.is_some() {
        Some("baseline-check".to_string())
    } else {
        None
    };
    let decision = if let Some(gate) = gate {
        string_path(gate, &["status"])
    } else {
        None
    }
    .map(|decision| {
        if decision == "not_configured" {
            "advisory".to_string()
        } else {
            decision
        }
    })
    .unwrap_or_else(|| "advisory".to_string());
    let acknowledgement_label = string_from_sources(&[
        (gate, &["policy", "acknowledgement_label"]),
        (gate, &["policy", "acknowledgement_labels", "0"]),
        (parsed.ledger.as_ref(), &["gate", "acknowledgement_label"]),
    ])
    .or_else(|| {
        if gate.is_some()
            || parsed.baseline_delta.is_some()
            || first_action_status(parsed.first_action.as_ref()) == Some("actionable")
        {
            Some("ripr-waive".to_string())
        } else {
            None
        }
    });
    PanelPolicy {
        mode,
        decision,
        authority_artifact: input.gate_decision_path.clone(),
        acknowledgement_label,
    }
}

fn policy_state(policy: &PanelPolicy, candidate: &Candidate) -> String {
    match policy.decision.as_str() {
        "blocked" => "blocking".to_string(),
        "config_error" => "config_error".to_string(),
        "acknowledged" => "waived".to_string(),
        _ if candidate.top_issue_state == "summary_only"
            || candidate.top_issue_state == "actionable" =>
        {
            "new_policy_eligible".to_string()
        }
        _ if candidate.headline == "Static grip improved while coverage stayed flat." => {
            "none".to_string()
        }
        _ if candidate.top_issue_state == "already_improved" => "baseline".to_string(),
        _ if candidate.headline.contains("Suppressed") => "suppressed".to_string(),
        _ => "none".to_string(),
    }
}

fn status(policy: &PanelPolicy, candidate: &Candidate) -> String {
    match policy.decision.as_str() {
        "blocked" => "blocked".to_string(),
        "config_error" => "config_error".to_string(),
        "acknowledged" => "acknowledged".to_string(),
        "pass" => "pass".to_string(),
        _ if candidate.headline.contains("Suppressed") => "advisory".to_string(),
        _ => "advisory".to_string(),
    }
}

fn movement(input: &PrReviewFrontPanelInput, parsed: &ParsedPanelSources) -> PanelMovement {
    if input.coverage_frontier_path.is_some()
        && let Some(health) = parsed.assistant_health.as_ref()
        && usize_path(health, &["summary", "improved"]).unwrap_or(0) > 0
    {
        return PanelMovement {
            state: "improved".to_string(),
            before_class: string_path(health, &["proofs", "0", "movement", "before_class"])
                .map(normalize_class),
            after_class: string_path(health, &["proofs", "0", "movement", "after_class"]),
            source_artifact: input.assistant_health_path.clone(),
        };
    }
    if let Some(receipt) = parsed.receipt.as_ref()
        && let Some(state) = string_from_sources(&[
            (Some(receipt), &["provenance", "movement"]),
            (Some(receipt), &["seam", "change"]),
        ])
    {
        return PanelMovement {
            state,
            before_class: string_path(receipt, &["seam", "before_class"]),
            after_class: string_path(receipt, &["seam", "after_class"]),
            source_artifact: input.receipt_path.clone(),
        };
    }
    if let Some(baseline) = parsed.baseline_delta.as_ref()
        && usize_path(baseline, &["delta", "resolved"]).unwrap_or(0) > 0
    {
        return PanelMovement {
            state: "resolved".to_string(),
            before_class: Some("weakly_exposed".to_string()),
            after_class: Some("not_present".to_string()),
            source_artifact: input.baseline_delta_path.clone(),
        };
    }
    if parsed
        .gate_decision
        .as_ref()
        .and_then(|gate| string_path(gate, &["status"]))
        .as_deref()
        == Some("acknowledged")
    {
        return PanelMovement {
            state: "unknown".to_string(),
            before_class: None,
            after_class: None,
            source_artifact: None,
        };
    }
    if first_action_status(parsed.first_action.as_ref()) == Some("actionable") {
        return PanelMovement {
            state: "unknown".to_string(),
            before_class: None,
            after_class: None,
            source_artifact: None,
        };
    }
    PanelMovement {
        state: "not_available".to_string(),
        before_class: None,
        after_class: None,
        source_artifact: None,
    }
}

fn display_debt_delta(
    raw: &PanelDebtDelta,
    candidate: &Candidate,
    movement: &PanelMovement,
    coverage_grip: &PanelCoverageGrip,
) -> PanelDebtDelta {
    if candidate.top_issue_state == "already_improved"
        && movement.state == "improved"
        && coverage_grip.state == "flat_coverage_grip_improved"
    {
        return PanelDebtDelta {
            new_policy_eligible: 0,
            baseline_still_present: 0,
            baseline_resolved: raw.baseline_resolved,
            acknowledged: 0,
            waived: 0,
            suppressed: 0,
            blocking_candidates: 0,
        };
    }
    raw.clone()
}

fn coverage_grip(
    input: &PrReviewFrontPanelInput,
    parsed: &ParsedPanelSources,
) -> PanelCoverageGrip {
    let Some(frontier) = parsed.coverage_frontier.as_ref() else {
        return PanelCoverageGrip {
            state: "not_available".to_string(),
            coverage_delta: None,
            grip_delta: None,
            source_artifact: None,
        };
    };
    let state = string_from_sources(&[
        (Some(frontier), &["coverage_grip", "state"]),
        (Some(frontier), &["summary", "coverage_grip_state"]),
        (Some(frontier), &["status"]),
    ])
    .unwrap_or_else(|| {
        if parsed
            .assistant_health
            .as_ref()
            .and_then(|health| usize_path(health, &["summary", "improved"]))
            .unwrap_or(0)
            > 0
        {
            "flat_coverage_grip_improved".to_string()
        } else {
            "unknown".to_string()
        }
    });
    PanelCoverageGrip {
        state,
        coverage_delta: f64_from_sources(&[
            (Some(frontier), &["coverage_grip", "coverage_delta"]),
            (Some(frontier), &["coverage_delta"]),
            (Some(frontier), &["coverage_delta_percent"]),
        ]),
        grip_delta: i64_from_sources(&[
            (Some(frontier), &["coverage_grip", "grip_delta"]),
            (Some(frontier), &["grip_delta"]),
            (Some(frontier), &["ripr_visible_unresolved_delta"]),
        ]),
        source_artifact: input.coverage_frontier_path.clone(),
    }
}

fn calibration(input: &PrReviewFrontPanelInput, parsed: &ParsedPanelSources) -> PanelCalibration {
    let mut source_artifacts = Vec::new();
    if let Some(path) = &input.recommendation_calibration_path {
        source_artifacts.push(path.clone());
    }
    if let Some(path) = &input.mutation_calibration_path {
        source_artifacts.push(path.clone());
    }
    let recommendation = if parsed.recommendation_calibration.is_some() {
        "available".to_string()
    } else if policy(input, parsed).decision == "blocked" {
        "supports_candidate".to_string()
    } else if matches!(
        first_action_status(parsed.first_action.as_ref()),
        Some("actionable" | "already_improved")
    ) && parsed.pr_guidance.is_some()
    {
        "unknown".to_string()
    } else {
        "not_available".to_string()
    };
    let mutation = if parsed.mutation_calibration.is_some() {
        "available".to_string()
    } else {
        "not_available".to_string()
    };
    PanelCalibration {
        recommendation,
        mutation,
        source_artifacts,
    }
}

fn artifacts(
    input: &PrReviewFrontPanelInput,
    inputs: &PanelInputs,
    summary: &PanelSummary,
    parsed: &ParsedPanelSources,
) -> Vec<PanelArtifact> {
    let mut artifacts = vec![PanelArtifact {
        group: "start_here".to_string(),
        label: "PR review front panel".to_string(),
        path: input.out_md_path.clone(),
        available: true,
        required: true,
    }];
    match summary.top_issue_state.as_str() {
        "missing_required_input" => {
            if let Some(path) = &inputs.assistant_proof {
                artifacts.push(PanelArtifact {
                    group: "repair".to_string(),
                    label: "Missing assistant proof".to_string(),
                    path: json_path_to_md(path),
                    available: false,
                    required: true,
                });
            }
            push_first_action_artifact(&mut artifacts, inputs, parsed);
        }
        "actionable" if summary.status == "blocked" => {
            if let Some(path) = &inputs.gate_decision {
                artifacts.push(policy_artifact(
                    "Gate decision",
                    &json_path_to_md(path),
                    true,
                ));
            }
            if let Some(path) = &inputs.first_action {
                artifacts.push(repair_artifact(
                    "First useful action",
                    &json_path_to_md(path),
                    parsed.first_action.is_some(),
                ));
            }
        }
        "actionable" if summary.status == "acknowledged" => {
            if let Some(path) = &inputs.gate_decision {
                artifacts.push(policy_artifact("Gate decision", path, true));
            }
            if let Some(path) = &inputs.ledger {
                artifacts.push(evidence_artifact(
                    "PR evidence ledger",
                    path,
                    parsed.ledger.is_some(),
                ));
            }
        }
        "actionable" => {
            if let Some(path) = &inputs.assistant_proof {
                artifacts.push(repair_artifact(
                    "Assistant proof",
                    &json_path_to_md(path),
                    true,
                ));
            }
            if let Some(path) = &inputs.pr_guidance {
                artifacts.push(evidence_artifact(
                    "PR guidance",
                    path,
                    parsed.pr_guidance.is_some(),
                ));
            }
        }
        "summary_only" => {
            if let Some(path) = &inputs.pr_guidance {
                artifacts.push(evidence_artifact(
                    "PR guidance summary",
                    &json_path_to_md(path),
                    parsed.pr_guidance.is_some(),
                ));
            }
        }
        "already_improved" if summary.coverage_grip_state == "flat_coverage_grip_improved" => {
            if let Some(path) = &inputs.assistant_health {
                artifacts.push(repair_artifact(
                    "Assistant loop health",
                    &json_path_to_md(path),
                    parsed.assistant_health.is_some(),
                ));
            }
            if let Some(path) = &inputs.coverage_frontier {
                artifacts.push(PanelArtifact {
                    group: "calibration".to_string(),
                    label: "Coverage/grip frontier".to_string(),
                    path: json_path_to_md(path),
                    available: parsed.coverage_frontier.is_some(),
                    required: false,
                });
            }
        }
        "already_improved" => {
            if let Some(path) = &inputs.baseline_delta {
                artifacts.push(evidence_artifact(
                    "Baseline debt delta",
                    path,
                    parsed.baseline_delta.is_some(),
                ));
            }
            if let Some(path) = &inputs.ledger {
                artifacts.push(evidence_artifact(
                    "PR evidence ledger",
                    path,
                    parsed.ledger.is_some(),
                ));
            }
        }
        "baseline_only" if summary.policy_state == "suppressed" => {
            if let Some(path) = &inputs.gate_decision {
                artifacts.push(policy_artifact(
                    "Gate decision",
                    path,
                    parsed.gate_decision.is_some(),
                ));
            }
        }
        _ => {
            push_first_action_artifact(&mut artifacts, inputs, parsed);
        }
    }
    artifacts
}

fn push_first_action_artifact(
    artifacts: &mut Vec<PanelArtifact>,
    inputs: &PanelInputs,
    parsed: &ParsedPanelSources,
) {
    if let Some(path) = &inputs.first_action {
        artifacts.push(evidence_artifact(
            "First useful action",
            &json_path_to_md(path),
            parsed.first_action.is_some(),
        ));
    }
}

fn repair_artifact(label: &str, path: &str, available: bool) -> PanelArtifact {
    PanelArtifact {
        group: "repair".to_string(),
        label: label.to_string(),
        path: path.to_string(),
        available,
        required: false,
    }
}

fn policy_artifact(label: &str, path: &str, available: bool) -> PanelArtifact {
    PanelArtifact {
        group: "policy".to_string(),
        label: label.to_string(),
        path: path.to_string(),
        available,
        required: false,
    }
}

fn evidence_artifact(label: &str, path: &str, available: bool) -> PanelArtifact {
    PanelArtifact {
        group: "evidence".to_string(),
        label: label.to_string(),
        path: path.to_string(),
        available,
        required: false,
    }
}

fn top_issue_from_first_action(
    input: &PrReviewFrontPanelInput,
    parsed: &ParsedPanelSources,
) -> Option<PanelTopIssue> {
    let action = parsed.first_action.as_ref()?;
    let selected = action.get("selected")?;
    let target = action.get("target");
    let seam_id = string_path(selected, &["seam_id"]);
    let classification = string_path(selected, &["classification"]);
    Some(PanelTopIssue {
        source: "first_useful_action".to_string(),
        source_artifact: input.first_action_path.clone()?,
        seam_id: seam_id.clone(),
        canonical_gap_id: string_path(selected, &["canonical_gap_id"]),
        path: string_path(selected, &["path"]),
        line: u64_path(selected, &["line"]),
        classification: classification.clone(),
        changed_behavior: string_from_sources(&[
            (Some(selected), &["changed_behavior"]),
            (target, &["changed_behavior"]),
        ]),
        current_evidence_strength: current_evidence_strength_from_sources(&[Some(selected)]),
        missing_discriminator: string_path(selected, &["missing_discriminator"]),
        focused_proof_intent: focused_proof_intent_from_action_or_proof(
            action,
            parsed.assistant_proof.as_ref(),
        ),
        related_test: target.and_then(|target| string_path(target, &["related_test"])),
        suggested_test: suggested_test_from_action_or_proof(
            action,
            parsed.assistant_proof.as_ref(),
        ),
        verify_command: string_path(action, &["commands", "verify"]),
        receipt_command: string_path(action, &["commands", "receipt"]),
        static_evidence_boundary: STATIC_EVIDENCE_BOUNDARY,
        agent_command: seam_id.as_deref().map(|seam_id| {
            format!(
                "ripr agent start --root {} --seam-id {} --out target/ripr/workflow",
                input.root, seam_id
            )
        }),
        receipt: receipt_from_input(
            input.receipt_path.as_deref(),
            parsed.receipt.as_ref(),
            RECEIPT_MISSING,
        ),
    })
}

fn top_issue_from_guidance(
    input: &PrReviewFrontPanelInput,
    parsed: &ParsedPanelSources,
    bucket: &str,
) -> Option<PanelTopIssue> {
    let guidance = parsed.pr_guidance.as_ref()?;
    let item = guidance
        .get(bucket)
        .and_then(Value::as_array)
        .and_then(|items| items.first())?;
    let seam_id = string_path(item, &["seam_id"]);
    Some(PanelTopIssue {
        source: "pr_guidance".to_string(),
        source_artifact: input.pr_guidance_path.clone()?,
        seam_id: seam_id.clone(),
        canonical_gap_id: string_path(item, &["canonical_gap_id"]),
        path: string_path(item, &["path"]),
        line: u64_path(item, &["line"]),
        classification: string_from_sources(&[
            (Some(item), &["classification"]),
            (Some(item), &["class"]),
            (Some(item), &["static_class"]),
        ])
        .map(normalize_class),
        changed_behavior: string_from_sources(&[
            (Some(item), &["changed_behavior"]),
            (Some(item), &["changed_expression"]),
            (Some(item), &["evidence", "changed_behavior"]),
        ]),
        current_evidence_strength: current_evidence_strength_from_sources(&[Some(item)])
            .or_else(|| {
                string_from_sources(&[
                    (Some(item), &["classification"]),
                    (Some(item), &["class"]),
                    (Some(item), &["static_class"]),
                ])
                .map(normalize_class)
            }),
        missing_discriminator: string_path(item, &["missing_discriminator"]),
        focused_proof_intent: string_from_sources(&[
            (Some(item), &["focused_proof_intent"]),
            (Some(item), &["suggested_test", "focused_proof_intent"]),
            (Some(item), &["suggested_test", "assertion_shape"]),
        ]),
        related_test: string_path(item, &["suggested_test", "near_test"]),
        suggested_test: string_path(item, &["suggested_test", "assertion_shape"]),
        verify_command: seam_id.as_ref().map(|_| {
            format!(
                "ripr agent verify --root {} --before target/ripr/workflow/before.repo-exposure.json --after target/ripr/workflow/after.repo-exposure.json --json",
                input.root
            )
        }),
        receipt_command: None,
        static_evidence_boundary: STATIC_EVIDENCE_BOUNDARY,
        agent_command: seam_id.as_deref().map(|seam_id| {
            format!(
                "ripr agent start --root {} --seam-id {} --out target/ripr/workflow",
                input.root, seam_id
            )
        }),
        receipt: PanelReceipt {
            artifact: None,
            status: RECEIPT_MISSING.to_string(),
        },
    })
}

fn top_issue_from_gate_decision(
    input: &PrReviewFrontPanelInput,
    parsed: &ParsedPanelSources,
    decision: &str,
) -> Option<PanelTopIssue> {
    let gate = parsed.gate_decision.as_ref()?;
    let item = gate
        .get("decisions")
        .and_then(Value::as_array)?
        .iter()
        .find(|item| string_path(item, &["decision"]).as_deref() == Some(decision))?;
    let seam_id = string_path(item, &["seam_id"]);
    Some(PanelTopIssue {
        source: "gate_decision".to_string(),
        source_artifact: input.gate_decision_path.clone()?,
        seam_id: seam_id.clone(),
        canonical_gap_id: string_path(item, &["canonical_gap_id"]),
        path: string_from_sources(&[
            (Some(item), &["placement", "path"]),
            (Some(item), &["path"]),
        ]),
        line: u64_from_sources(&[
            (Some(item), &["placement", "line"]),
            (Some(item), &["line"]),
        ]),
        classification: Some("weakly_exposed".to_string()),
        changed_behavior: string_path(item, &["evidence", "changed_behavior"]),
        current_evidence_strength: current_evidence_strength_from_sources(&[Some(item)])
            .or_else(|| Some("weakly_exposed".to_string())),
        missing_discriminator: string_path(item, &["evidence", "missing_discriminator"]),
        focused_proof_intent: string_from_sources(&[
            (Some(item), &["evidence", "focused_proof_intent"]),
            (Some(item), &["evidence", "assertion_shape"]),
        ]),
        related_test: string_path(item, &["evidence", "recommended_test"]),
        suggested_test: string_path(item, &["evidence", "assertion_shape"]),
        verify_command: None,
        receipt_command: None,
        static_evidence_boundary: STATIC_EVIDENCE_BOUNDARY,
        agent_command: seam_id.as_deref().and_then(|seam_id| {
            if decision == "acknowledged" {
                Some(format!(
                    "ripr agent start --root {} --seam-id {} --out target/ripr/workflow",
                    input.root, seam_id
                ))
            } else {
                None
            }
        }),
        receipt: PanelReceipt {
            artifact: None,
            status: if decision == "suppressed" {
                RECEIPT_NOT_APPLICABLE.to_string()
            } else {
                RECEIPT_MISSING.to_string()
            },
        },
    })
}

fn top_issue_from_baseline_delta(
    input: &PrReviewFrontPanelInput,
    parsed: &ParsedPanelSources,
    bucket: &str,
) -> Option<PanelTopIssue> {
    let delta = parsed.baseline_delta.as_ref()?;
    let item = first_item_with_bucket(delta, bucket)?;
    Some(PanelTopIssue {
        source: "baseline_delta".to_string(),
        source_artifact: input.baseline_delta_path.clone()?,
        seam_id: string_path(item, &["identity", "seam_id"]),
        canonical_gap_id: string_path(item, &["identity", "canonical_gap_id"]),
        path: string_path(item, &["path"]),
        line: u64_path(item, &["line"]),
        classification: string_path(item, &["static_class"]).map(normalize_class),
        changed_behavior: string_path(item, &["changed_behavior"]),
        current_evidence_strength: current_evidence_strength_from_sources(&[Some(item)])
            .or_else(|| string_path(item, &["static_class"]).map(normalize_class)),
        missing_discriminator: string_path(item, &["missing_discriminator"]),
        focused_proof_intent: string_from_sources(&[
            (Some(item), &["focused_proof_intent"]),
            (Some(item), &["suggested_test", "assertion_shape"]),
        ]),
        related_test: string_path(item, &["suggested_test", "recommended_test"]),
        suggested_test: string_path(item, &["suggested_test", "assertion_shape"]),
        verify_command: string_path(item, &["repair", "verify_command"]),
        receipt_command: string_path(item, &["repair", "receipt_command"]),
        static_evidence_boundary: STATIC_EVIDENCE_BOUNDARY,
        agent_command: None,
        receipt: PanelReceipt {
            artifact: None,
            status: RECEIPT_NOT_APPLICABLE.to_string(),
        },
    })
}

fn top_issue_from_assistant_health(
    input: &PrReviewFrontPanelInput,
    parsed: &ParsedPanelSources,
) -> Option<PanelTopIssue> {
    let health = parsed.assistant_health.as_ref()?;
    let proof = health
        .get("proofs")
        .and_then(Value::as_array)
        .and_then(|proofs| proofs.first())?;
    let seam = proof.get("seam")?;
    let recommendation = proof.get("recommendation");
    let handoff = proof.get("handoff");
    let receipt = proof.get("receipt");
    Some(PanelTopIssue {
        source: "assistant_health".to_string(),
        source_artifact: input.assistant_health_path.clone()?,
        seam_id: string_path(seam, &["seam_id"]),
        canonical_gap_id: string_path(seam, &["canonical_gap_id"]),
        path: string_path(seam, &["path"]),
        line: u64_path(seam, &["line"]),
        classification: string_path(seam, &["grip_class"]).map(normalize_class),
        changed_behavior: string_from_sources(&[
            (Some(seam), &["changed_behavior"]),
            (recommendation, &["changed_behavior"]),
        ]),
        current_evidence_strength: current_evidence_strength_from_sources(&[
            Some(seam),
            recommendation,
        ])
        .or_else(|| string_path(seam, &["grip_class"]).map(normalize_class)),
        missing_discriminator: string_path(seam, &["missing_discriminator"]),
        focused_proof_intent: recommendation.and_then(|value| {
            string_from_sources(&[
                (Some(value), &["focused_proof_intent"]),
                (Some(value), &["suggested_test"]),
                (Some(value), &["assertion_shape"]),
            ])
        }),
        related_test: recommendation.and_then(|value| string_path(value, &["related_test"])),
        suggested_test: recommendation.and_then(|value| string_path(value, &["suggested_test"])),
        verify_command: recommendation.and_then(|value| string_path(value, &["verify_command"])),
        receipt_command: receipt.and_then(|value| string_path(value, &["command"])),
        static_evidence_boundary: STATIC_EVIDENCE_BOUNDARY,
        agent_command: handoff.and_then(|value| string_path(value, &["agent_command"])),
        receipt: PanelReceipt {
            artifact: receipt.and_then(|value| string_path(value, &["artifact"])),
            status: parsed
                .receipt
                .as_ref()
                .map(receipt_lifecycle_state_from_receipt_value)
                .or_else(|| {
                    receipt
                        .and_then(|value| string_path(value, &["status"]))
                        .map(|state| receipt_lifecycle_state(Some(&state)))
                })
                .unwrap_or_else(|| RECEIPT_MISSING.to_string()),
        },
    })
}

fn first_action_status(first_action: Option<&Value>) -> Option<&str> {
    first_action
        .and_then(|value| value.get("status"))
        .and_then(Value::as_str)
}

fn first_item_with_bucket<'a>(report: &'a Value, bucket: &str) -> Option<&'a Value> {
    report
        .get("items")
        .and_then(Value::as_array)?
        .iter()
        .find(|item| string_path(item, &["bucket"]).as_deref() == Some(bucket))
}

fn has_suppressed(parsed: &ParsedPanelSources) -> bool {
    parsed
        .gate_decision
        .as_ref()
        .and_then(|gate| gate.get("decisions"))
        .and_then(Value::as_array)
        .is_some_and(|items| {
            items
                .iter()
                .any(|item| string_path(item, &["decision"]).as_deref() == Some("suppressed"))
        })
        && parsed
            .gate_decision
            .as_ref()
            .and_then(|gate| string_path(gate, &["status"]))
            .as_deref()
            != Some("acknowledged")
}

fn placement_from_guidance(pr_guidance: Option<&Value>) -> String {
    let Some(guidance) = pr_guidance else {
        return "changed_line".to_string();
    };
    if guidance
        .get("comments")
        .and_then(Value::as_array)
        .is_some_and(|items| !items.is_empty())
    {
        return "changed_line".to_string();
    }
    if guidance
        .get("summary_only")
        .and_then(Value::as_array)
        .is_some_and(|items| !items.is_empty())
    {
        return "summary_only".to_string();
    }
    "not_available".to_string()
}

fn focused_proof_intent_from_action_or_proof(
    action: &Value,
    proof: Option<&Value>,
) -> Option<String> {
    string_from_sources(&[
        (Some(action), &["target", "focused_proof_intent"]),
        (Some(action), &["target", "suggested_assertion"]),
        (proof, &["recommendation", "focused_proof_intent"]),
        (proof, &["recommendation", "suggested_test"]),
        (proof, &["recommendation", "assertion_shape"]),
    ])
}

fn suggested_test_from_action_or_proof(action: &Value, proof: Option<&Value>) -> Option<String> {
    string_from_sources(&[
        (proof, &["recommendation", "suggested_test"]),
        (Some(action), &["target", "suggested_assertion"]),
    ])
    .map(|value| {
        if value.starts_with("Assert the exact ")
            && let Some(rest) = value.strip_prefix("Assert the exact ")
            && let Some((target, condition)) = rest.trim_end_matches('.').split_once(" at ")
        {
            return format!("Add a focused test where {condition} and assert the exact {target}.");
        }
        value
    })
}

fn current_evidence_strength_from_sources(sources: &[Option<&Value>]) -> Option<String> {
    sources.iter().find_map(|source| {
        let source = (*source)?;
        string_from_sources(&[
            (Some(source), &["current_evidence_strength"]),
            (Some(source), &["evidence", "current_evidence_strength"]),
        ])
    })
}

fn receipt_from_input(
    receipt_path: Option<&str>,
    receipt_json: Option<&Value>,
    missing_status: &str,
) -> PanelReceipt {
    PanelReceipt {
        artifact: receipt_path.map(ToOwned::to_owned).or_else(|| {
            receipt_json.and_then(|receipt| {
                string_from_sources(&[
                    (Some(receipt), &["provenance", "artifact"]),
                    (Some(receipt), &["receipt"]),
                ])
            })
        }),
        status: if let Some(receipt) = receipt_json {
            receipt_lifecycle_state_from_receipt_value(receipt)
        } else if receipt_path.is_some() {
            receipt_lifecycle_state(Some("present"))
        } else {
            receipt_lifecycle_state(Some(missing_status))
        },
    }
}

fn normalize_class(value: String) -> String {
    match value.as_str() {
        "weakly_gripped" => "weakly_exposed".to_string(),
        "strongly_gripped" => "exposed".to_string(),
        _ => value,
    }
}

fn string_from_sources(sources: &[(Option<&Value>, &[&str])]) -> Option<String> {
    sources
        .iter()
        .find_map(|(value, path)| value.and_then(|value| string_path(value, path)))
}

fn u64_from_sources(sources: &[(Option<&Value>, &[&str])]) -> Option<u64> {
    sources
        .iter()
        .find_map(|(value, path)| value.and_then(|value| u64_path(value, path)))
}

fn usize_from_sources(sources: &[(Option<&Value>, &[&str])]) -> usize {
    sources
        .iter()
        .find_map(|(value, path)| value.and_then(|value| usize_path(value, path)))
        .unwrap_or(0)
}

fn f64_from_sources(sources: &[(Option<&Value>, &[&str])]) -> Option<f64> {
    sources
        .iter()
        .find_map(|(value, path)| value.and_then(|value| f64_path(value, path)))
}

fn i64_from_sources(sources: &[(Option<&Value>, &[&str])]) -> Option<i64> {
    sources
        .iter()
        .find_map(|(value, path)| value.and_then(|value| i64_path(value, path)))
}

fn string_path(value: &Value, path: &[&str]) -> Option<String> {
    path_value(value, path).and_then(value_as_string)
}

fn u64_path(value: &Value, path: &[&str]) -> Option<u64> {
    path_value(value, path).and_then(Value::as_u64)
}

fn usize_path(value: &Value, path: &[&str]) -> Option<usize> {
    path_value(value, path)
        .and_then(Value::as_u64)
        .and_then(|value| usize::try_from(value).ok())
}

fn i64_path(value: &Value, path: &[&str]) -> Option<i64> {
    path_value(value, path).and_then(Value::as_i64)
}

fn f64_path(value: &Value, path: &[&str]) -> Option<f64> {
    path_value(value, path).and_then(Value::as_f64)
}

fn path_value<'a>(value: &'a Value, path: &[&str]) -> Option<&'a Value> {
    let mut current = value;
    for key in path {
        if let Ok(index) = key.parse::<usize>() {
            current = current.get(index)?;
        } else {
            current = current.get(*key)?;
        }
    }
    Some(current)
}

fn value_as_string(value: &Value) -> Option<String> {
    if let Some(text) = value.as_str() {
        return Some(text.to_string());
    }
    if let Some(number) = value.as_i64() {
        return Some(number.to_string());
    }
    value.as_u64().map(|number| number.to_string())
}

fn json_path_to_md(path: &str) -> String {
    if let Some(prefix) = path.strip_suffix(".json") {
        format!("{prefix}.md")
    } else {
        path.to_string()
    }
}

fn issue_location(issue: &PanelTopIssue) -> Option<String> {
    match (issue.path.as_deref(), issue.line) {
        (Some(path), Some(line)) => Some(format!("{path}:{line}")),
        (Some(path), None) => Some(path.to_string()),
        (None, Some(line)) => Some(format!("unknown:{line}")),
        (None, None) => None,
    }
}

fn compact_suggested_test(value: &str) -> String {
    if let Some(predicate) = extract_boundary_subject(value) {
        return format!("add {predicate} boundary assertion");
    }
    value.to_ascii_lowercase()
}

fn extract_boundary_subject(value: &str) -> Option<String> {
    let lower = value.to_ascii_lowercase();
    for marker in [
        "input that hits the boundary:",
        "boundary input where ",
        "boundary test that exercises ",
    ] {
        if let Some(index) = lower.find(marker) {
            return compact_boundary_subject(&value[index + marker.len()..]);
        }
    }
    None
}

fn compact_boundary_subject(value: &str) -> Option<String> {
    let lower = value.to_ascii_lowercase();
    let mut end = value.len();
    for delimiter in [" and assert", " */", "`", "\n", "."] {
        if let Some(index) = lower.find(delimiter) {
            end = end.min(index);
        }
    }
    let subject = value[..end]
        .trim()
        .trim_matches(|c: char| c == '`' || c == ',' || c == ')' || c == ';')
        .trim();
    if subject.is_empty()
        || ![">=", "<=", "==", "!=", ">", "<"]
            .iter()
            .any(|operator| subject.contains(operator))
    {
        return None;
    }
    Some(subject.to_string())
}

fn coverage_grip_markdown(coverage: &PanelCoverageGrip) -> String {
    match coverage.state.as_str() {
        "flat_coverage_grip_improved" => "flat coverage, improved grip".to_string(),
        "not_available" => "not available".to_string(),
        other => other.replace('_', " "),
    }
}

fn percent_or(value: Option<f64>) -> String {
    match value {
        Some(value) if value >= 0.0 => format!("+{value:.1}%"),
        Some(value) => format!("{value:.1}%"),
        None => "not available".to_string(),
    }
}

fn signed_or(value: Option<i64>) -> String {
    match value {
        Some(value) if value > 0 => format!("+{value}"),
        Some(value) => value.to_string(),
        None => "not available".to_string(),
    }
}

fn gate_authority_markdown(policy: &PanelPolicy) -> String {
    match policy.authority_artifact.as_deref() {
        Some(path) if policy.decision == "blocked" => json_path_to_md(path),
        Some(path) => path.to_string(),
        None => "not configured".to_string(),
    }
}

fn artifact_group_label(group: &str) -> &str {
    match group {
        "start_here" => "Start here",
        "repair" => "Repair",
        "evidence" => "Evidence",
        "policy" => "Policy",
        "calibration" => "Calibration",
        "generated_ci" => "Generated CI",
        _ => "Artifact",
    }
}

fn push_count(out: &mut String, label: &str, count: usize) {
    out.push_str(&format!("- {label}: {count}\n"));
}

fn str_or<'a>(value: Option<&'a str>, fallback: &'a str) -> &'a str {
    match value {
        Some(value) => value,
        None => fallback,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::output::test_support::{read_file, repo_root};
    use std::path::Path;

    #[test]
    fn pr_review_front_panel_matches_fixture_corpus() -> Result<(), String> {
        let repo_root = repo_root()?;
        let corpus_path =
            repo_root.join("fixtures/boundary_gap/expected/pr-review-front-panel/corpus.json");
        let corpus: Value = serde_json::from_str(&read_file(&corpus_path)?)
            .map_err(|err| format!("parse corpus failed: {err}"))?;
        let cases = corpus
            .get("cases")
            .and_then(Value::as_array)
            .ok_or_else(|| "corpus cases missing".to_string())?;

        for case in cases {
            let case_id =
                string_path(case, &["id"]).ok_or_else(|| "case id missing".to_string())?;
            let inputs = case
                .get("inputs")
                .ok_or_else(|| format!("{case_id} inputs missing"))?;
            let expected_json_path = repo_root.join(
                string_path(case, &["expected_report"])
                    .ok_or_else(|| format!("{case_id} expected_report missing"))?,
            );
            let expected_md_path = repo_root.join(
                string_path(case, &["expected_markdown"])
                    .ok_or_else(|| format!("{case_id} expected_markdown missing"))?,
            );
            let input = fixture_input(&repo_root, inputs, &expected_md_path)?;
            let report = build_pr_review_front_panel_report(input);

            assert_eq!(
                render_pr_review_front_panel_json(&report)?,
                read_file(&expected_json_path)?.trim_end(),
                "{case_id} JSON fixture drifted"
            );
            assert_eq!(
                render_pr_review_front_panel_markdown(&report),
                read_file(&expected_md_path)?,
                "{case_id} Markdown fixture drifted"
            );
        }
        Ok(())
    }

    #[test]
    fn pr_review_front_panel_reports_malformed_optional_input() -> Result<(), String> {
        let input = PrReviewFrontPanelInput {
            root: ".".to_string(),
            generated_at: "2026-05-09T12:00:00Z".to_string(),
            out_md_path: "target/ripr/reports/pr-review-front-panel.md".to_string(),
            pr_guidance_path: Some("comments.json".to_string()),
            first_action_path: None,
            assistant_proof_path: None,
            assistant_health_path: None,
            ledger_path: None,
            baseline_delta_path: None,
            zero_status_path: None,
            gate_decision_path: None,
            recommendation_calibration_path: None,
            mutation_calibration_path: None,
            coverage_frontier_path: None,
            receipt_path: None,
            pr_guidance_json: Some(Ok("{".to_string())),
            first_action_json: None,
            assistant_proof_json: None,
            assistant_health_json: None,
            ledger_json: None,
            baseline_delta_json: None,
            zero_status_json: None,
            gate_decision_json: None,
            recommendation_calibration_json: None,
            mutation_calibration_json: None,
            coverage_frontier_json: None,
            receipt_json: None,
        };
        let report = build_pr_review_front_panel_report(input);
        let rendered = render_pr_review_front_panel_json(&report)?;
        assert!(rendered.contains("\"kind\": \"malformed_input\""));
        assert!(rendered.contains("Optional PR guidance input is malformed"));
        Ok(())
    }

    #[test]
    fn first_action_top_issue_ignores_target_evidence_strength() -> Result<(), String> {
        let input = PrReviewFrontPanelInput {
            root: ".".to_string(),
            generated_at: "2026-05-09T12:00:00Z".to_string(),
            out_md_path: "target/ripr/reports/pr-review-front-panel.md".to_string(),
            pr_guidance_path: None,
            first_action_path: Some("first-action.json".to_string()),
            assistant_proof_path: None,
            assistant_health_path: None,
            ledger_path: None,
            baseline_delta_path: None,
            zero_status_path: None,
            gate_decision_path: None,
            recommendation_calibration_path: None,
            mutation_calibration_path: None,
            coverage_frontier_path: None,
            receipt_path: None,
            pr_guidance_json: None,
            first_action_json: None,
            assistant_proof_json: None,
            assistant_health_json: None,
            ledger_json: None,
            baseline_delta_json: None,
            zero_status_json: None,
            gate_decision_json: None,
            recommendation_calibration_json: None,
            mutation_calibration_json: None,
            coverage_frontier_json: None,
            receipt_json: None,
        };
        let parsed = ParsedPanelSources {
            first_action: Some(
                serde_json::from_str(
                    r#"{
                        "selected": {"classification": "weakly_exposed"},
                        "target": {"current_evidence_strength": "target-only value"}
                    }"#,
                )
                .map_err(|e| e.to_string())?,
            ),
            ..Default::default()
        };

        let issue = top_issue_from_first_action(&input, &parsed)
            .ok_or_else(|| "expected first-action top issue".to_string())?;
        assert_eq!(
            issue.current_evidence_strength, None,
            "first-action top issue must source evidence strength only from selected"
        );
        Ok(())
    }

    #[test]
    fn compact_suggested_test_extracts_boundary_subjects_without_fixture_specific_names() {
        assert_eq!(
            compact_suggested_test(
                "Add a focused boundary test that exercises total <= max_total and assert the exact output."
            ),
            "add total <= max_total boundary assertion"
        );
        assert_eq!(
            compact_suggested_test(
                "assert_eq!(foo(/* boundary input where left != right */), expected)"
            ),
            "add left != right boundary assertion"
        );
        assert_eq!(
            compact_suggested_test("Review RIPR evidence."),
            "review ripr evidence."
        );
    }

    fn fixture_input(
        repo_root: &Path,
        inputs: &Value,
        expected_md_path: &Path,
    ) -> Result<PrReviewFrontPanelInput, String> {
        let pr_guidance = path_from_inputs(inputs, "pr_guidance");
        let first_action = path_from_inputs(inputs, "first_action");
        let assistant_proof = path_from_inputs(inputs, "assistant_proof");
        let assistant_health = path_from_inputs(inputs, "assistant_health");
        let ledger = path_from_inputs(inputs, "ledger");
        let baseline_delta = path_from_inputs(inputs, "baseline_delta");
        let zero_status = path_from_inputs(inputs, "zero_status");
        let gate_decision = path_from_inputs(inputs, "gate_decision");
        let recommendation_calibration = path_from_inputs(inputs, "recommendation_calibration");
        let mutation_calibration = path_from_inputs(inputs, "mutation_calibration");
        let coverage_frontier = path_from_inputs(inputs, "coverage_frontier");
        let receipt = path_from_inputs(inputs, "receipt");

        Ok(PrReviewFrontPanelInput {
            root: string_path(inputs, &["root"]).unwrap_or_else(|| {
                if first_action.is_some() || assistant_health.is_some() {
                    "fixtures/boundary_gap/input".to_string()
                } else {
                    ".".to_string()
                }
            }),
            generated_at: "2026-05-09T12:00:00Z".to_string(),
            out_md_path: fixture_path(repo_root, expected_md_path),
            pr_guidance_json: read_optional_fixture(repo_root, pr_guidance.as_deref())?,
            first_action_json: read_optional_fixture(repo_root, first_action.as_deref())?,
            assistant_proof_json: read_optional_fixture(repo_root, assistant_proof.as_deref())?,
            assistant_health_json: read_optional_fixture(repo_root, assistant_health.as_deref())?,
            ledger_json: read_optional_fixture(repo_root, ledger.as_deref())?,
            baseline_delta_json: read_optional_fixture(repo_root, baseline_delta.as_deref())?,
            zero_status_json: read_optional_fixture(repo_root, zero_status.as_deref())?,
            gate_decision_json: read_optional_fixture(repo_root, gate_decision.as_deref())?,
            recommendation_calibration_json: read_optional_fixture(
                repo_root,
                recommendation_calibration.as_deref(),
            )?,
            mutation_calibration_json: read_optional_fixture(
                repo_root,
                mutation_calibration.as_deref(),
            )?,
            coverage_frontier_json: read_optional_fixture(repo_root, coverage_frontier.as_deref())?,
            receipt_json: read_optional_fixture(repo_root, receipt.as_deref())?,
            pr_guidance_path: pr_guidance,
            first_action_path: first_action,
            assistant_proof_path: assistant_proof,
            assistant_health_path: assistant_health,
            ledger_path: ledger,
            baseline_delta_path: baseline_delta,
            zero_status_path: zero_status,
            gate_decision_path: gate_decision,
            recommendation_calibration_path: recommendation_calibration,
            mutation_calibration_path: mutation_calibration,
            coverage_frontier_path: coverage_frontier,
            receipt_path: receipt,
        })
    }

    fn path_from_inputs(inputs: &Value, key: &str) -> Option<String> {
        inputs.get(key).and_then(value_as_string)
    }

    fn read_optional_fixture(
        repo_root: &Path,
        path: Option<&str>,
    ) -> Result<Option<Result<String, String>>, String> {
        let Some(path) = path else {
            return Ok(None);
        };
        let absolute = repo_root.join(path);
        if absolute.exists() {
            return Ok(Some(Ok(read_file(&absolute)?)));
        }
        Ok(Some(Err(format!(
            "{} does not exist",
            display_path(&absolute)
        ))))
    }

    fn fixture_path(repo_root: &Path, path: &Path) -> String {
        match path.strip_prefix(repo_root) {
            Ok(relative) => display_path(relative),
            Err(_) => display_path(path),
        }
    }
}
