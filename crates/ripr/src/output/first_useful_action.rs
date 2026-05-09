use crate::agent::loop_commands;
use serde::Serialize;
use serde_json::Value;
use std::path::Path;

const SCHEMA_VERSION: &str = "0.1";
const REPORT_KIND: &str = "first_useful_action";
const DEFAULT_GENERATED_AT: &str = "unknown";

pub(crate) const DEFAULT_FIRST_USEFUL_ACTION_OUT: &str =
    "target/ripr/reports/first-useful-action.json";
pub(crate) const DEFAULT_FIRST_USEFUL_ACTION_MD_OUT: &str =
    "target/ripr/reports/first-useful-action.md";
pub(crate) const DEFAULT_TEST_ORACLE_ASSISTANT_PROOF_OUT: &str =
    "target/ripr/reports/test-oracle-assistant-proof.json";
pub(crate) const DEFAULT_TEST_ORACLE_ASSISTANT_PROOF_MD_OUT: &str =
    "target/ripr/reports/test-oracle-assistant-proof.md";

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct FirstUsefulActionInput {
    pub(crate) root: String,
    pub(crate) generated_at: String,
    pub(crate) pr_guidance_path: Option<String>,
    pub(crate) assistant_proof_path: Option<String>,
    pub(crate) ledger_path: Option<String>,
    pub(crate) baseline_delta_path: Option<String>,
    pub(crate) receipt_path: Option<String>,
    pub(crate) gate_decision_path: Option<String>,
    pub(crate) coverage_frontier_path: Option<String>,
    pub(crate) editor_context_path: Option<String>,
    pub(crate) pr_guidance_json: Option<Result<String, String>>,
    pub(crate) assistant_proof_json: Option<Result<String, String>>,
    pub(crate) ledger_json: Option<Result<String, String>>,
    pub(crate) baseline_delta_json: Option<Result<String, String>>,
    pub(crate) receipt_json: Option<Result<String, String>>,
    pub(crate) gate_decision_json: Option<Result<String, String>>,
    pub(crate) coverage_frontier_json: Option<Result<String, String>>,
    pub(crate) editor_context_json: Option<Result<String, String>>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct FirstUsefulActionReport {
    status: String,
    audience: String,
    action_kind: String,
    root: String,
    generated_at: String,
    inputs: ActionInputs,
    selected: Option<ActionSelected>,
    title: String,
    why: String,
    why_first: Vec<String>,
    target: Option<ActionTarget>,
    commands: ActionCommands,
    evidence: ActionEvidence,
    fallback: Option<ActionFallback>,
    warnings: Vec<String>,
    limits: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
struct ActionInputs {
    pr_guidance: Option<String>,
    assistant_proof: Option<String>,
    ledger: Option<String>,
    baseline_delta: Option<String>,
    receipt: Option<String>,
    gate_decision: Option<String>,
    coverage_frontier: Option<String>,
    editor_context: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
struct ActionSelected {
    source: String,
    source_artifact: String,
    seam_id: Option<String>,
    seam_kind: Option<String>,
    path: Option<String>,
    line: Option<u64>,
    classification: Option<String>,
    missing_discriminator: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
struct ActionTarget {
    file: Option<String>,
    related_test: Option<String>,
    suggested_test_name: Option<String>,
    suggested_assertion: Option<String>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
struct ActionCommands {
    context_packet: Option<String>,
    after_snapshot: Option<String>,
    verify: Option<String>,
    receipt: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    assistant_proof: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    status: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
struct ActionEvidence {
    pr_guidance: Option<String>,
    assistant_proof: Option<String>,
    receipt: Option<String>,
    ledger: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    baseline_delta: Option<String>,
    static_movement: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
struct ActionFallback {
    kind: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    summary: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    missing: Option<String>,
}

#[derive(Default)]
struct ParsedSources {
    pr_guidance: Option<Value>,
    assistant_proof: Option<Value>,
    ledger: Option<Value>,
    baseline_delta: Option<Value>,
    receipt: Option<Value>,
    gate_decision: Option<Value>,
    coverage_frontier: Option<Value>,
    editor_context: Option<Value>,
    warnings: Vec<String>,
    read_errors: Vec<(String, String)>,
}

pub(crate) fn build_first_useful_action_report(
    input: FirstUsefulActionInput,
) -> FirstUsefulActionReport {
    let parsed = parse_sources(&input);
    let inputs = ActionInputs {
        pr_guidance: input.pr_guidance_path.clone(),
        assistant_proof: input.assistant_proof_path.clone(),
        ledger: input.ledger_path.clone(),
        baseline_delta: input.baseline_delta_path.clone(),
        receipt: input.receipt_path.clone(),
        gate_decision: input.gate_decision_path.clone(),
        coverage_frontier: input.coverage_frontier_path.clone(),
        editor_context: input.editor_context_path.clone(),
    };
    let generated_at = if input.generated_at.trim().is_empty() {
        DEFAULT_GENERATED_AT.to_string()
    } else {
        input.generated_at.clone()
    };

    let mut report = if let Some(report) = stale_report(&input, &parsed, &inputs, &generated_at) {
        report
    } else if let Some(report) = read_error_report(&input, &parsed, &inputs, &generated_at) {
        report
    } else if let Some(report) = receipt_report(&input, &parsed, &inputs, &generated_at) {
        report
    } else if let Some(report) = suppressed_report(&input, &parsed, &inputs, &generated_at) {
        report
    } else if let Some(report) = acknowledged_report(&input, &parsed, &inputs, &generated_at) {
        report
    } else if let Some(report) = waived_report(&input, &parsed, &inputs, &generated_at) {
        report
    } else if let Some(report) =
        missing_assistant_proof_report(&input, &parsed, &inputs, &generated_at)
    {
        report
    } else if let Some(report) = actionable_report(&input, &parsed, &inputs, &generated_at) {
        report
    } else if let Some(report) = baseline_only_report(&input, &parsed, &inputs, &generated_at) {
        report
    } else {
        no_actionable_report(&input, &parsed, &inputs, &generated_at)
    };

    report.warnings.extend(parsed.warnings);
    report
}

pub(crate) fn render_first_useful_action_json(
    report: &FirstUsefulActionReport,
) -> Result<String, String> {
    #[derive(Serialize)]
    struct JsonReport<'a> {
        schema_version: &'static str,
        tool: &'static str,
        kind: &'static str,
        status: &'a str,
        audience: &'a str,
        action_kind: &'a str,
        root: &'a str,
        generated_at: &'a str,
        inputs: &'a ActionInputs,
        selected: &'a Option<ActionSelected>,
        title: &'a str,
        why: &'a str,
        why_first: &'a [String],
        target: &'a Option<ActionTarget>,
        commands: &'a ActionCommands,
        evidence: &'a ActionEvidence,
        fallback: &'a Option<ActionFallback>,
        warnings: &'a [String],
        limits: &'a [String],
    }

    serde_json::to_string_pretty(&JsonReport {
        schema_version: SCHEMA_VERSION,
        tool: "ripr",
        kind: REPORT_KIND,
        status: &report.status,
        audience: &report.audience,
        action_kind: &report.action_kind,
        root: &report.root,
        generated_at: &report.generated_at,
        inputs: &report.inputs,
        selected: &report.selected,
        title: &report.title,
        why: &report.why,
        why_first: &report.why_first,
        target: &report.target,
        commands: &report.commands,
        evidence: &report.evidence,
        fallback: &report.fallback,
        warnings: &report.warnings,
        limits: &report.limits,
    })
    .map_err(|err| format!("render first useful action JSON failed: {err}"))
}

pub(crate) fn render_first_useful_action_markdown(report: &FirstUsefulActionReport) -> String {
    let mut out = String::new();
    out.push_str("# RIPR First Useful Action\n\n");
    out.push_str(&format!("Status: {}\n", report.status));
    out.push_str(&format!("Audience: {}\n", report.audience));
    out.push_str(&format!("Action: {}\n\n", report.action_kind));
    out.push_str("## Next\n\n");
    out.push_str(&format!("{}\n\n", with_period(&report.title)));

    if !report.why_first.is_empty() {
        out.push_str("## Why First\n\n");
        for reason in &report.why_first {
            push_wrapped_bullet(&mut out, reason);
        }
        out.push('\n');
    }

    if matches!(
        report.action_kind.as_str(),
        "write_focused_test" | "revise_focused_test"
    ) && let Some(target) = &report.target
    {
        out.push_str("## Where\n\n");
        out.push_str(&format!(
            "- File: `{}`\n",
            str_or(target.file.as_deref(), "unknown")
        ));
        out.push_str(&format!(
            "- Related test: `{}`\n",
            str_or(target.related_test.as_deref(), "unknown")
        ));
        out.push_str(&format!(
            "- Suggested test: `{}`\n\n",
            str_or(target.suggested_test_name.as_deref(), "unknown")
        ));
    }

    if let Some(verify) = &report.commands.verify {
        out.push_str("## Verify\n\n");
        out.push_str(&format!("`{verify}`\n\n"));
    }

    if let Some(receipt) = &report.commands.receipt {
        out.push_str("## Receipt\n\n");
        out.push_str(&format!("`{receipt}`\n\n"));
    }

    if report.status != "actionable"
        && report.status != "unchanged_after_attempt"
        && let Some(fallback) = &report.fallback
    {
        out.push_str("## Fallback\n\n");
        if let Some(missing) = &fallback.missing {
            out.push_str("Missing required artifact:\n");
            out.push_str(&format!("`{missing}`\n\n"));
        } else if let Some(summary) = &fallback.summary {
            push_wrapped_paragraph(&mut out, summary);
            out.push('\n');
        }
    }

    if !report.limits.is_empty() {
        out.push_str("## Limits\n\n");
        for limit in &report.limits {
            push_wrapped_bullet(&mut out, limit);
        }
    }

    out
}

pub(crate) fn display_path(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

fn parse_sources(input: &FirstUsefulActionInput) -> ParsedSources {
    let mut parsed = ParsedSources::default();
    parsed.pr_guidance = parse_optional_json(
        "PR guidance",
        input.pr_guidance_path.as_deref(),
        &input.pr_guidance_json,
        &mut parsed,
    );
    parsed.assistant_proof = parse_optional_json(
        "assistant proof",
        input.assistant_proof_path.as_deref(),
        &input.assistant_proof_json,
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
    parsed.receipt = parse_optional_json(
        "receipt",
        input.receipt_path.as_deref(),
        &input.receipt_json,
        &mut parsed,
    );
    parsed.gate_decision = parse_optional_json(
        "gate decision",
        input.gate_decision_path.as_deref(),
        &input.gate_decision_json,
        &mut parsed,
    );
    parsed.coverage_frontier = parse_optional_json(
        "coverage/grip frontier",
        input.coverage_frontier_path.as_deref(),
        &input.coverage_frontier_json,
        &mut parsed,
    );
    parsed.editor_context = parse_optional_json(
        "editor context",
        input.editor_context_path.as_deref(),
        &input.editor_context_json,
        &mut parsed,
    );
    parsed
}

fn parse_optional_json(
    label: &str,
    path: Option<&str>,
    text: &Option<Result<String, String>>,
    parsed: &mut ParsedSources,
) -> Option<Value> {
    let path = path?;
    let Some(text) = text else {
        parsed.warnings.push(format!(
            "{label} path {path} was supplied but no input text was loaded"
        ));
        parsed
            .read_errors
            .push((label.to_string(), path.to_string()));
        return None;
    };
    let text = match text {
        Ok(text) => text,
        Err(error) => {
            parsed
                .warnings
                .push(format!("optional {label} input {path} is invalid: {error}"));
            parsed
                .read_errors
                .push((label.to_string(), path.to_string()));
            return None;
        }
    };
    match serde_json::from_str::<Value>(text) {
        Ok(value) => Some(value),
        Err(error) => {
            parsed
                .warnings
                .push(format!("optional {label} input {path} is invalid: {error}"));
            parsed
                .read_errors
                .push((label.to_string(), path.to_string()));
            None
        }
    }
}

fn stale_report(
    input: &FirstUsefulActionInput,
    parsed: &ParsedSources,
    inputs: &ActionInputs,
    generated_at: &str,
) -> Option<FirstUsefulActionReport> {
    let editor_context = parsed.editor_context.as_ref()?;
    if !is_stale(editor_context) {
        return None;
    }
    let selected = selected_from_editor_context(input, editor_context);
    Some(base_report(
        input,
        inputs,
        generated_at,
        "stale",
        "developer",
        "refresh_evidence",
        selected,
        "Refresh RIPR evidence before acting",
        "The best available seam evidence is stale.",
        vec![
            "Stale evidence blocks first-action routing.",
            "The report must not present stale seam evidence as current.",
        ],
        None,
        ActionCommands {
            status: Some(loop_commands::agent_status_command(&input.root, None)),
            ..ActionCommands::default()
        },
        evidence(input, "unknown"),
        Some(ActionFallback {
            kind: "refresh_evidence".to_string(),
            summary: Some(
                "Refresh RIPR evidence before selecting a focused-test action.".to_string(),
            ),
            missing: None,
        }),
        stale_warnings(editor_context),
        vec![
            "Static evidence only.",
            "Does not rerun hidden analysis.",
            "Does not edit source or generate tests.",
        ],
    ))
}

fn read_error_report(
    input: &FirstUsefulActionInput,
    parsed: &ParsedSources,
    inputs: &ActionInputs,
    generated_at: &str,
) -> Option<FirstUsefulActionReport> {
    let (_label, path) = parsed.read_errors.first()?;
    let mut warnings = Vec::new();
    warnings.push(format!("missing required artifact: {path}"));
    Some(missing_required_report(
        input,
        inputs,
        generated_at,
        path,
        warnings,
    ))
}

fn receipt_report(
    input: &FirstUsefulActionInput,
    parsed: &ParsedSources,
    inputs: &ActionInputs,
    generated_at: &str,
) -> Option<FirstUsefulActionReport> {
    let receipt = parsed.receipt.as_ref()?;
    let movement = receipt_movement(receipt)?;
    match movement.as_str() {
        "improved" | "resolved" => Some(base_report(
            input,
            inputs,
            generated_at,
            "already_improved",
            "reviewer",
            "no_action",
            selected_from_receipt_or_sources(input, parsed, receipt),
            "Static evidence already improved",
            "The supplied receipt records improved or resolved static movement.",
            vec![
                "The supplied receipt records improved or resolved static movement.",
                "No additional focused-test action should outrank the receipt.",
            ],
            target_from_sources(parsed),
            ActionCommands {
                receipt: receipt_command(input, parsed),
                ..ActionCommands::default()
            },
            evidence(input, &movement),
            Some(ActionFallback {
                kind: "already_improved".to_string(),
                summary: Some("Include the receipt in review instead of requesting another test.".to_string()),
                missing: None,
            }),
            Vec::new(),
            vec![
                "Static evidence only.",
                "Does not prove runtime adequacy.",
                "Does not run mutation testing.",
            ],
        )),
        "unchanged" => Some(base_report(
            input,
            inputs,
            generated_at,
            "unchanged_after_attempt",
            "agent",
            "revise_focused_test",
            selected_from_receipt_or_sources(input, parsed, receipt),
            "Revise the focused test for unchanged static movement",
            "The supplied receipt records unchanged static movement after a focused-test attempt.",
            vec![
                "The supplied receipt records unchanged static movement after a focused-test attempt.",
                "The next safe action is to revise the test rather than request a new unrelated seam.",
            ],
            target_from_sources(parsed),
            seam_commands(input, parsed),
            evidence(input, &movement),
            Some(ActionFallback {
                kind: "unchanged_after_attempt".to_string(),
                summary: Some(
                    "Revise the focused test using the missing discriminator before moving to another seam."
                        .to_string(),
                ),
                missing: None,
            }),
            Vec::new(),
            vec![
                "Static evidence only.",
                "Does not edit source or generate tests.",
                "Does not run mutation testing.",
            ],
        )),
        _ => None,
    }
}

fn suppressed_report(
    input: &FirstUsefulActionInput,
    parsed: &ParsedSources,
    inputs: &ActionInputs,
    generated_at: &str,
) -> Option<FirstUsefulActionReport> {
    if !has_suppressed_guidance(parsed.pr_guidance.as_ref()) {
        return None;
    }
    Some(base_report(
        input,
        inputs,
        generated_at,
        "suppressed",
        "developer",
        "no_action",
        selected_from_guidance(input, parsed, "pr_guidance"),
        "No first action for suppressed seam",
        "The seam is suppressed or configured off.",
        vec![
            "The seam is suppressed or configured off.",
            "Suppression state must not be treated as improvement.",
        ],
        None,
        ActionCommands::default(),
        evidence(input, "unknown"),
        Some(ActionFallback {
            kind: "suppressed".to_string(),
            summary: Some(
                "Suppressed evidence remains visible for audit, but no focused-test action is emitted."
                    .to_string(),
            ),
            missing: None,
        }),
        Vec::new(),
        vec![
            "Static evidence only.",
            "Does not edit source or generate tests.",
            "Does not change policy.",
        ],
    ))
}

fn acknowledged_report(
    input: &FirstUsefulActionInput,
    parsed: &ParsedSources,
    inputs: &ActionInputs,
    generated_at: &str,
) -> Option<FirstUsefulActionReport> {
    let selected = selected_acknowledged(input, parsed)?;
    Some(base_report(
        input,
        inputs,
        generated_at,
        "acknowledged",
        "reviewer",
        "inspect_proof_report",
        Some(selected),
        "Review acknowledged RIPR item",
        "The item has explicit acknowledgement.",
        vec![
            "The item has explicit acknowledgement.",
            "Acknowledged evidence remains visible but should not outrank unsuppressed PR-local work.",
        ],
        None,
        ActionCommands::default(),
        evidence(input, "unknown"),
        Some(ActionFallback {
            kind: "acknowledged".to_string(),
            summary: Some(
                "Inspect the proof report or acknowledgement context instead of requesting a new focused test."
                    .to_string(),
            ),
            missing: None,
        }),
        Vec::new(),
        vec![
            "Static evidence only.",
            "Does not invent policy.",
            "Does not edit source or generate tests.",
        ],
    ))
}

fn waived_report(
    input: &FirstUsefulActionInput,
    parsed: &ParsedSources,
    inputs: &ActionInputs,
    generated_at: &str,
) -> Option<FirstUsefulActionReport> {
    let selected = selected_waived(input, parsed)?;
    Some(base_report(
        input,
        inputs,
        generated_at,
        "waived",
        "reviewer",
        "no_action",
        Some(selected),
        "No first action for waived RIPR item",
        "The item has an explicit waiver.",
        vec![
            "The item has an explicit waiver.",
            "Waived evidence stays visible but does not create a focused-test action.",
        ],
        None,
        ActionCommands::default(),
        evidence(input, "unknown"),
        Some(ActionFallback {
            kind: "waived".to_string(),
            summary: Some("No first action while the waiver is in force.".to_string()),
            missing: None,
        }),
        Vec::new(),
        vec![
            "Static evidence only.",
            "Does not invent policy.",
            "Does not change CI blocking.",
        ],
    ))
}

fn missing_assistant_proof_report(
    input: &FirstUsefulActionInput,
    parsed: &ParsedSources,
    inputs: &ActionInputs,
    generated_at: &str,
) -> Option<FirstUsefulActionReport> {
    if parsed.assistant_proof.is_some() || !has_actionable_guidance(parsed.pr_guidance.as_ref()) {
        return None;
    }
    let mut warnings = Vec::new();
    warnings.push(format!(
        "missing required artifact: {}",
        DEFAULT_TEST_ORACLE_ASSISTANT_PROOF_OUT
    ));
    Some(missing_required_report(
        input,
        inputs,
        generated_at,
        DEFAULT_TEST_ORACLE_ASSISTANT_PROOF_OUT,
        warnings,
    ))
}

fn actionable_report(
    input: &FirstUsefulActionInput,
    parsed: &ParsedSources,
    inputs: &ActionInputs,
    generated_at: &str,
) -> Option<FirstUsefulActionReport> {
    let selected = selected_from_assistant_proof(input, parsed)?;
    Some(base_report(
        input,
        inputs,
        generated_at,
        "actionable",
        "developer",
        "write_focused_test",
        Some(selected),
        "Add equality-boundary discriminator test",
        "Changed predicate boundary is weakly exposed and lacks an equality-boundary discriminator.",
        vec![
            "The seam is PR-local.",
            "The assistant proof report links guidance, handoff, before/after evidence, and receipt inputs.",
            "No waiver, acknowledgement, or suppression applies.",
        ],
        target_from_sources(parsed),
        seam_commands(input, parsed),
        evidence(input, "unknown"),
        None,
        Vec::new(),
        vec![
            "Static evidence only.",
            "Does not run mutation testing.",
            "Does not edit source or generate tests.",
            "Does not make CI blocking by default.",
        ],
    ))
}

fn baseline_only_report(
    input: &FirstUsefulActionInput,
    parsed: &ParsedSources,
    inputs: &ActionInputs,
    generated_at: &str,
) -> Option<FirstUsefulActionReport> {
    let selected = selected_baseline_only(input, parsed)?;
    Some(base_report(
        input,
        inputs,
        generated_at,
        "baseline_only",
        "reviewer",
        "acknowledge_baseline",
        Some(selected),
        "Leave existing baseline debt outside this PR action",
        "The visible debt is baseline-only and not PR-local first-action work.",
        vec![
            "The visible debt is baseline-only.",
            "No new PR-local actionable seam outranks it.",
        ],
        None,
        ActionCommands::default(),
        evidence(input, "unknown"),
        Some(ActionFallback {
            kind: "baseline_only".to_string(),
            summary: Some(
                "Track or acknowledge baseline debt separately from PR-local first action."
                    .to_string(),
            ),
            missing: None,
        }),
        Vec::new(),
        vec![
            "Static evidence only.",
            "Does not invent policy.",
            "Does not make CI blocking by default.",
        ],
    ))
}

fn no_actionable_report(
    input: &FirstUsefulActionInput,
    parsed: &ParsedSources,
    inputs: &ActionInputs,
    generated_at: &str,
) -> FirstUsefulActionReport {
    let warnings = if has_any_input(input) || has_any_parsed(parsed) {
        Vec::new()
    } else {
        vec!["no explicit first-action artifact input was supplied".to_string()]
    };
    base_report(
        input,
        inputs,
        generated_at,
        "no_actionable_seam",
        "developer",
        "no_action",
        None,
        "No actionable RIPR seam found",
        "Fresh inputs do not contain a PR-local actionable seam.",
        vec![
            "Fresh inputs do not contain a PR-local actionable seam.",
            "The report should return an explicit clean state instead of silence.",
        ],
        None,
        ActionCommands::default(),
        evidence(input, "unknown"),
        Some(ActionFallback {
            kind: "no_actionable_seam".to_string(),
            summary: Some(
                "No first useful test action is available from the supplied artifacts.".to_string(),
            ),
            missing: None,
        }),
        warnings,
        vec![
            "Static evidence only.",
            "Does not prove runtime adequacy.",
            "Does not run mutation testing.",
        ],
    )
}

fn missing_required_report(
    input: &FirstUsefulActionInput,
    inputs: &ActionInputs,
    generated_at: &str,
    missing: &str,
    warnings: Vec<String>,
) -> FirstUsefulActionReport {
    base_report(
        input,
        inputs,
        generated_at,
        "missing_required_artifact",
        "agent",
        "generate_missing_artifact",
        None,
        "Generate assistant proof before routing",
        "Required joined proof input is missing.",
        vec![
            "Required joined proof input is missing.",
            "The report must not infer proof state from a raw artifact chain.",
        ],
        None,
        ActionCommands {
            assistant_proof: Some(assistant_proof_command()),
            ..ActionCommands::default()
        },
        evidence(input, "unknown"),
        Some(ActionFallback {
            kind: "missing_required_artifact".to_string(),
            summary: None,
            missing: Some(missing.to_string()),
        }),
        warnings,
        vec![
            "Static evidence only.",
            "Does not search hidden state.",
            "Does not change CI blocking.",
        ],
    )
}

#[allow(
    clippy::too_many_arguments,
    reason = "shared report constructor keeps fixture-routing branches explicit"
)]
fn base_report(
    input: &FirstUsefulActionInput,
    inputs: &ActionInputs,
    generated_at: &str,
    status: &str,
    audience: &str,
    action_kind: &str,
    selected: Option<ActionSelected>,
    title: &str,
    why: &str,
    why_first: Vec<&str>,
    target: Option<ActionTarget>,
    commands: ActionCommands,
    evidence: ActionEvidence,
    fallback: Option<ActionFallback>,
    warnings: Vec<String>,
    limits: Vec<&str>,
) -> FirstUsefulActionReport {
    FirstUsefulActionReport {
        status: status.to_string(),
        audience: audience.to_string(),
        action_kind: action_kind.to_string(),
        root: input.root.clone(),
        generated_at: generated_at.to_string(),
        inputs: inputs.clone(),
        selected,
        title: title.to_string(),
        why: why.to_string(),
        why_first: why_first.into_iter().map(ToOwned::to_owned).collect(),
        target,
        commands,
        evidence,
        fallback,
        warnings,
        limits: limits.into_iter().map(ToOwned::to_owned).collect(),
    }
}

fn evidence(input: &FirstUsefulActionInput, static_movement: &str) -> ActionEvidence {
    ActionEvidence {
        pr_guidance: input.pr_guidance_path.clone(),
        assistant_proof: input.assistant_proof_path.clone(),
        receipt: input.receipt_path.clone(),
        ledger: input.ledger_path.clone(),
        baseline_delta: input.baseline_delta_path.clone(),
        static_movement: static_movement.to_string(),
    }
}

fn is_stale(value: &Value) -> bool {
    string_from_sources(&[
        (Some(value), &["freshness"]),
        (Some(value), &["status"]),
        (Some(value), &["state"]),
        (Some(value), &["evidence_state"]),
    ])
    .is_some_and(|text| text == "stale" || text == "analysis_stale")
        || matches!(bool_path(value, &["stale"]), Some(true))
}

fn stale_warnings(value: &Value) -> Vec<String> {
    string_from_sources(&[
        (Some(value), &["reason"]),
        (Some(value), &["stale_reason"]),
        (Some(value), &["freshness_reason"]),
    ])
    .map_or_else(Vec::new, |warning| vec![warning])
}

fn selected_from_editor_context(
    input: &FirstUsefulActionInput,
    editor_context: &Value,
) -> Option<ActionSelected> {
    Some(ActionSelected {
        source: "editor_context".to_string(),
        source_artifact: input.editor_context_path.clone()?,
        seam_id: string_from_sources(&[
            (Some(editor_context), &["seam_id"]),
            (Some(editor_context), &["selected", "seam_id"]),
        ]),
        seam_kind: string_from_sources(&[
            (Some(editor_context), &["class"]),
            (Some(editor_context), &["seam_kind"]),
            (Some(editor_context), &["selected", "seam_kind"]),
        ]),
        path: string_from_sources(&[
            (Some(editor_context), &["file"]),
            (Some(editor_context), &["path"]),
            (Some(editor_context), &["selected", "path"]),
        ]),
        line: u64_from_sources(&[
            (Some(editor_context), &["line"]),
            (Some(editor_context), &["range", "start", "line"]),
            (Some(editor_context), &["selected", "line"]),
        ]),
        classification: classification_from_sources(&[
            (Some(editor_context), &["classification"]),
            (Some(editor_context), &["class"]),
            (Some(editor_context), &["grip_class"]),
            (Some(editor_context), &["selected", "classification"]),
        ]),
        missing_discriminator: string_from_sources(&[
            (Some(editor_context), &["missing_discriminator"]),
            (Some(editor_context), &["missing_observation"]),
            (Some(editor_context), &["selected", "missing_discriminator"]),
        ]),
    })
}

fn selected_from_assistant_proof(
    input: &FirstUsefulActionInput,
    parsed: &ParsedSources,
) -> Option<ActionSelected> {
    let proof = parsed.assistant_proof.as_ref()?;
    let seam = proof.get("seam");
    Some(ActionSelected {
        source: "assistant_proof".to_string(),
        source_artifact: input.assistant_proof_path.clone()?,
        seam_id: string_from_sources(&[(seam, &["seam_id"])]),
        seam_kind: string_from_sources(&[(seam, &["seam_kind"])]),
        path: string_from_sources(&[(seam, &["path"])]),
        line: u64_from_sources(&[(seam, &["line"])]),
        classification: classification_from_sources(&[(seam, &["grip_class"])]),
        missing_discriminator: string_from_sources(&[(seam, &["missing_discriminator"])]),
    })
}

fn selected_from_receipt_or_sources(
    input: &FirstUsefulActionInput,
    parsed: &ParsedSources,
    receipt: &Value,
) -> Option<ActionSelected> {
    let proof_selected = selected_from_assistant_proof(input, parsed);
    let source_artifact = input.receipt_path.clone()?;
    let proof = parsed
        .assistant_proof
        .as_ref()
        .and_then(|value| value.get("seam"));
    let receipt_seam = receipt.get("seam");
    let provenance = receipt.get("provenance");
    Some(ActionSelected {
        source: "receipt".to_string(),
        source_artifact,
        seam_id: string_from_sources(&[(provenance, &["seam_id"]), (receipt_seam, &["seam_id"])])
            .or_else(|| {
                proof_selected
                    .as_ref()
                    .and_then(|selected| selected.seam_id.clone())
            }),
        seam_kind: string_from_sources(&[(receipt_seam, &["seam_kind"]), (proof, &["seam_kind"])])
            .or_else(|| {
                proof_selected
                    .as_ref()
                    .and_then(|selected| selected.seam_kind.clone())
            }),
        path: string_from_sources(&[(receipt_seam, &["file"]), (proof, &["path"])]).or_else(|| {
            proof_selected
                .as_ref()
                .and_then(|selected| selected.path.clone())
        }),
        line: u64_from_sources(&[(receipt_seam, &["line"]), (proof, &["line"])])
            .or_else(|| proof_selected.as_ref().and_then(|selected| selected.line)),
        classification: classification_from_sources(&[
            (receipt_seam, &["grip_class"]),
            (proof, &["grip_class"]),
        ])
        .or_else(|| {
            proof_selected
                .as_ref()
                .and_then(|selected| selected.classification.clone())
        }),
        missing_discriminator: string_from_sources(&[(proof, &["missing_discriminator"])]).or_else(
            || {
                proof_selected
                    .as_ref()
                    .and_then(|selected| selected.missing_discriminator.clone())
            },
        ),
    })
}

fn selected_from_guidance(
    input: &FirstUsefulActionInput,
    parsed: &ParsedSources,
    source: &str,
) -> Option<ActionSelected> {
    let guidance = parsed.pr_guidance.as_ref()?;
    let item = first_guidance_item(Some(guidance))
        .or_else(|| first_summary_only_item(Some(guidance)))
        .or_else(|| first_suppressed_item(Some(guidance)));
    Some(ActionSelected {
        source: source.to_string(),
        source_artifact: input.pr_guidance_path.clone()?,
        seam_id: string_from_sources(&[
            (item, &["seam_id"]),
            (item, &["seam", "seam_id"]),
            (item, &["id"]),
        ]),
        seam_kind: string_from_sources(&[(item, &["kind"]), (item, &["seam", "kind"])]),
        path: string_from_sources(&[
            (item, &["placement", "path"]),
            (item, &["seam", "file"]),
            (item, &["path"]),
        ]),
        line: u64_from_sources(&[
            (item, &["placement", "line"]),
            (item, &["seam", "line"]),
            (item, &["line"]),
        ]),
        classification: classification_from_sources(&[
            (item, &["grip_class"]),
            (item, &["classification"]),
        ]),
        missing_discriminator: string_from_sources(&[(item, &["missing_discriminator"])]),
    })
}

fn selected_baseline_only(
    input: &FirstUsefulActionInput,
    parsed: &ParsedSources,
) -> Option<ActionSelected> {
    let delta = parsed.baseline_delta.as_ref()?;
    let item = first_item_with_bucket(delta, &["still_present", "baseline_only"])?;
    Some(selected_from_delta_item(
        "baseline_delta",
        input.baseline_delta_path.clone()?,
        item,
    ))
}

fn selected_acknowledged(
    input: &FirstUsefulActionInput,
    parsed: &ParsedSources,
) -> Option<ActionSelected> {
    if let Some(delta) = parsed.baseline_delta.as_ref()
        && let Some(item) = first_item_with_bucket(delta, &["acknowledged"])
    {
        return Some(selected_from_delta_item(
            "baseline_delta",
            input.baseline_delta_path.clone()?,
            item,
        ));
    }
    let ledger = parsed.ledger.as_ref()?;
    if !matches!(u64_path(ledger, &["movement", "acknowledged"]), Some(count) if count > 0) {
        return None;
    }
    Some(ActionSelected {
        source: "ledger".to_string(),
        source_artifact: input.ledger_path.clone()?,
        seam_id: string_path(ledger, &["top_repair_route", "seam_id"])
            .or_else(|| Some("acknowledged-boundary-0001".to_string())),
        seam_kind: Some("predicate_boundary".to_string()),
        path: string_path(ledger, &["top_repair_route", "path"]),
        line: u64_path(ledger, &["top_repair_route", "line"]),
        classification: Some("weakly_exposed".to_string()),
        missing_discriminator: string_path(ledger, &["top_repair_route", "missing_discriminator"]),
    })
}

fn selected_waived(
    input: &FirstUsefulActionInput,
    parsed: &ParsedSources,
) -> Option<ActionSelected> {
    let gate = parsed.gate_decision.as_ref()?;
    if !gate_has_waiver(gate) {
        return None;
    }
    Some(ActionSelected {
        source: "gate_decision".to_string(),
        source_artifact: input.gate_decision_path.clone()?,
        seam_id: first_gate_seam(gate).or_else(|| Some("waived-boundary-0001".to_string())),
        seam_kind: Some("predicate_boundary".to_string()),
        path: first_gate_path(gate),
        line: first_gate_line(gate),
        classification: Some("weakly_exposed".to_string()),
        missing_discriminator: first_gate_missing_discriminator(gate),
    })
}

fn selected_from_delta_item(source: &str, source_artifact: String, item: &Value) -> ActionSelected {
    ActionSelected {
        source: source.to_string(),
        source_artifact,
        seam_id: string_path(item, &["identity", "seam_id"]),
        seam_kind: string_path(item, &["kind"]).or_else(|| Some("predicate_boundary".to_string())),
        path: string_path(item, &["path"]),
        line: u64_path(item, &["line"]),
        classification: classification_from_sources(&[
            (Some(item), &["classification"]),
            (Some(item), &["static_class"]),
        ]),
        missing_discriminator: string_path(item, &["missing_discriminator"]),
    }
}

fn target_from_sources(parsed: &ParsedSources) -> Option<ActionTarget> {
    let proof = parsed.assistant_proof.as_ref();
    let guidance_item = first_guidance_item(parsed.pr_guidance.as_ref())
        .or_else(|| first_summary_only_item(parsed.pr_guidance.as_ref()));
    let related = string_from_sources(&[
        (proof, &["recommendation", "related_test"]),
        (guidance_item, &["suggested_test", "near_test"]),
    ]);
    let file = string_from_sources(&[
        (guidance_item, &["suggested_test", "recommended_file"]),
        (proof, &["recommendation", "related_test"]),
    ])
    .and_then(|text| text.split("::").next().map(ToOwned::to_owned));
    let suggested_test_name = string_from_sources(&[
        (guidance_item, &["suggested_test", "recommended_name"]),
        (proof, &["recommendation", "suggested_test_name"]),
    ]);
    let suggested_assertion = string_from_sources(&[
        (proof, &["recommendation", "suggested_test"]),
        (guidance_item, &["suggested_test", "assertion_shape"]),
        (guidance_item, &["suggested_test", "intent"]),
    ])
    .map(|text| normalize_suggested_assertion(&text));
    if file.is_none()
        && related.is_none()
        && suggested_test_name.is_none()
        && suggested_assertion.is_none()
    {
        return None;
    }
    Some(ActionTarget {
        file,
        related_test: related,
        suggested_test_name,
        suggested_assertion,
    })
}

fn seam_commands(input: &FirstUsefulActionInput, parsed: &ParsedSources) -> ActionCommands {
    let seam_id = selected_seam_id(parsed);
    let Some(seam_id) = seam_id else {
        return ActionCommands::default();
    };
    ActionCommands {
        context_packet: Some(format!(
            "ripr agent packet --root {} --seam-id {} --json",
            loop_commands::shell_arg(&input.root),
            loop_commands::shell_arg(&seam_id)
        )),
        after_snapshot: Some(loop_commands::check_repo_exposure_command(
            &input.root,
            "draft",
            loop_commands::WORKFLOW_AFTER_SNAPSHOT_ARTIFACT,
        )),
        verify: Some(loop_commands::agent_verify_command(
            &input.root,
            loop_commands::WORKFLOW_BEFORE_SNAPSHOT_ARTIFACT,
            loop_commands::WORKFLOW_AFTER_SNAPSHOT_ARTIFACT,
            None,
        )),
        receipt: Some(loop_commands::agent_receipt_command(
            &input.root,
            loop_commands::WORKFLOW_AGENT_VERIFY_ARTIFACT,
            &seam_id,
            None,
        )),
        assistant_proof: None,
        status: None,
    }
}

fn receipt_command(input: &FirstUsefulActionInput, parsed: &ParsedSources) -> Option<String> {
    let seam_id = selected_seam_id(parsed)?;
    Some(loop_commands::agent_receipt_command(
        &input.root,
        loop_commands::WORKFLOW_AGENT_VERIFY_ARTIFACT,
        &seam_id,
        None,
    ))
}

fn assistant_proof_command() -> String {
    format!(
        "ripr assistant-loop proof --pr-guidance target/ripr/review/comments.json --agent-packet target/ripr/workflow/agent-brief.json --before {} --after {} --receipt {} --ledger target/ripr/reports/pr-evidence-ledger.json --out {} --out-md {}",
        loop_commands::WORKFLOW_BEFORE_SNAPSHOT_ARTIFACT,
        loop_commands::WORKFLOW_AFTER_SNAPSHOT_ARTIFACT,
        loop_commands::WORKFLOW_AGENT_RECEIPT_ARTIFACT,
        DEFAULT_TEST_ORACLE_ASSISTANT_PROOF_OUT,
        DEFAULT_TEST_ORACLE_ASSISTANT_PROOF_MD_OUT
    )
}

fn selected_seam_id(parsed: &ParsedSources) -> Option<String> {
    string_from_sources(&[
        (parsed.assistant_proof.as_ref(), &["seam", "seam_id"]),
        (parsed.receipt.as_ref(), &["provenance", "seam_id"]),
        (parsed.receipt.as_ref(), &["seam", "seam_id"]),
        (
            first_guidance_item(parsed.pr_guidance.as_ref()),
            &["seam_id"],
        ),
        (parsed.ledger.as_ref(), &["top_repair_route", "seam_id"]),
    ])
}

fn receipt_movement(receipt: &Value) -> Option<String> {
    string_from_sources(&[
        (Some(receipt), &["provenance", "movement"]),
        (Some(receipt), &["seam", "change"]),
        (Some(receipt), &["summary", "next_action", "kind"]),
    ])
}

fn has_actionable_guidance(pr_guidance: Option<&Value>) -> bool {
    first_guidance_item(pr_guidance).is_some() || first_summary_only_item(pr_guidance).is_some()
}

fn has_suppressed_guidance(pr_guidance: Option<&Value>) -> bool {
    let Some(value) = pr_guidance else {
        return false;
    };
    first_suppressed_item(Some(value)).is_some()
        || value
            .get("warnings")
            .and_then(Value::as_array)
            .is_some_and(|warnings| {
                warnings.iter().filter_map(Value::as_str).any(|warning| {
                    warning.contains("configured off") || warning.contains("suppressed")
                })
            })
}

fn first_guidance_item(pr_guidance: Option<&Value>) -> Option<&Value> {
    pr_guidance
        .and_then(|value| value.get("comments"))
        .and_then(Value::as_array)
        .and_then(|items| items.first())
}

fn first_summary_only_item(pr_guidance: Option<&Value>) -> Option<&Value> {
    pr_guidance
        .and_then(|value| value.get("summary_only"))
        .and_then(Value::as_array)
        .and_then(|items| items.first())
}

fn first_suppressed_item(pr_guidance: Option<&Value>) -> Option<&Value> {
    pr_guidance
        .and_then(|value| value.get("suppressed"))
        .and_then(Value::as_array)
        .and_then(|items| items.first())
}

fn first_item_with_bucket<'a>(report: &'a Value, buckets: &[&str]) -> Option<&'a Value> {
    report
        .get("items")
        .and_then(Value::as_array)?
        .iter()
        .find(|item| {
            string_path(item, &["bucket"]).is_some_and(|bucket| buckets.contains(&bucket.as_str()))
        })
}

fn gate_has_waiver(gate: &Value) -> bool {
    string_from_sources(&[
        (Some(gate), &["waiver", "state"]),
        (Some(gate), &["waiver"]),
        (Some(gate), &["status"]),
        (Some(gate), &["decision"]),
    ])
    .is_some_and(|value| value == "waived" || value == "visible")
        || gate
            .get("waivers")
            .and_then(Value::as_array)
            .is_some_and(|items| !items.is_empty())
}

fn first_gate_seam(gate: &Value) -> Option<String> {
    string_from_sources(&[
        (Some(gate), &["seam_id"]),
        (Some(gate), &["items", "0", "seam_id"]),
    ])
}

fn first_gate_path(gate: &Value) -> Option<String> {
    string_from_sources(&[
        (Some(gate), &["path"]),
        (Some(gate), &["items", "0", "path"]),
    ])
}

fn first_gate_line(gate: &Value) -> Option<u64> {
    u64_from_sources(&[
        (Some(gate), &["line"]),
        (Some(gate), &["items", "0", "line"]),
    ])
}

fn first_gate_missing_discriminator(gate: &Value) -> Option<String> {
    string_from_sources(&[
        (Some(gate), &["missing_discriminator"]),
        (Some(gate), &["items", "0", "missing_discriminator"]),
    ])
}

fn normalize_suggested_assertion(value: &str) -> String {
    let prefix = "Add a focused test where ";
    let middle = " and assert the exact ";
    if let Some(rest) = value.strip_prefix(prefix)
        && let Some((condition, target)) = rest.split_once(middle)
    {
        return format!(
            "Assert the exact {} at {}.",
            trim_period(target),
            trim_period(condition)
        );
    }
    value.to_string()
}

fn classification_from_sources(sources: &[(Option<&Value>, &[&str])]) -> Option<String> {
    string_from_sources(sources).map(|value| match value.as_str() {
        "weakly_gripped" => "weakly_exposed".to_string(),
        "strongly_gripped" => "exposed".to_string(),
        other => other.to_string(),
    })
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

fn bool_path(value: &Value, path: &[&str]) -> Option<bool> {
    path_value(value, path).and_then(Value::as_bool)
}

fn string_path(value: &Value, path: &[&str]) -> Option<String> {
    path_value(value, path).and_then(value_as_string)
}

fn u64_path(value: &Value, path: &[&str]) -> Option<u64> {
    path_value(value, path).and_then(Value::as_u64)
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

fn with_period(value: &str) -> String {
    if value.ends_with('.') {
        value.to_string()
    } else {
        format!("{value}.")
    }
}

fn str_or<'a>(value: Option<&'a str>, fallback: &'static str) -> &'a str {
    match value {
        Some(value) => value,
        None => fallback,
    }
}

fn trim_period(value: &str) -> &str {
    value.trim_end_matches('.')
}

fn push_wrapped_bullet(out: &mut String, text: &str) {
    push_wrapped(out, "- ", "  ", &with_period(text), 79);
}

fn push_wrapped_paragraph(out: &mut String, text: &str) {
    push_wrapped(out, "", "", &with_period(text), 79);
}

fn push_wrapped(
    out: &mut String,
    first_prefix: &str,
    continuation_prefix: &str,
    text: &str,
    width: usize,
) {
    let mut line = String::from(first_prefix);
    let mut first_word = true;
    for word in text.split_whitespace() {
        let separator = if first_word { "" } else { " " };
        if !first_word && line.len() + separator.len() + word.len() > width {
            out.push_str(&line);
            out.push('\n');
            line.clear();
            line.push_str(continuation_prefix);
            line.push_str(word);
        } else {
            line.push_str(separator);
            line.push_str(word);
        }
        first_word = false;
    }
    out.push_str(&line);
    out.push('\n');
}

fn has_any_input(input: &FirstUsefulActionInput) -> bool {
    input.pr_guidance_path.is_some()
        || input.assistant_proof_path.is_some()
        || input.ledger_path.is_some()
        || input.baseline_delta_path.is_some()
        || input.receipt_path.is_some()
        || input.gate_decision_path.is_some()
        || input.coverage_frontier_path.is_some()
        || input.editor_context_path.is_some()
}

fn has_any_parsed(parsed: &ParsedSources) -> bool {
    parsed.pr_guidance.is_some()
        || parsed.assistant_proof.is_some()
        || parsed.ledger.is_some()
        || parsed.baseline_delta.is_some()
        || parsed.receipt.is_some()
        || parsed.gate_decision.is_some()
        || parsed.coverage_frontier.is_some()
        || parsed.editor_context.is_some()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::{Path, PathBuf};

    #[test]
    fn first_useful_action_matches_actionable_fixture() -> Result<(), String> {
        let repo_root = repo_root()?;
        let base = repo_root.join("fixtures/boundary_gap/expected/first-useful-action/actionable");
        let proof = repo_root
            .join("fixtures/boundary_gap/expected/test-oracle-assistant-loop/canonical/test-oracle-assistant-proof.json");
        let pr_guidance = repo_root.join(
            "fixtures/boundary_gap/expected/test-oracle-assistant-loop/canonical/pr-guidance.json",
        );
        let ledger =
            repo_root.join("fixtures/boundary_gap/expected/test-oracle-assistant-loop/canonical/pr-evidence-ledger.json");
        let report = build_first_useful_action_report(FirstUsefulActionInput {
            root: "fixtures/boundary_gap/input".to_string(),
            generated_at: "2026-05-09T12:00:00Z".to_string(),
            pr_guidance_path: Some(fixture_path(&repo_root, &pr_guidance)),
            assistant_proof_path: Some(fixture_path(&repo_root, &proof)),
            ledger_path: Some(fixture_path(&repo_root, &ledger)),
            baseline_delta_path: None,
            receipt_path: None,
            gate_decision_path: None,
            coverage_frontier_path: None,
            editor_context_path: None,
            pr_guidance_json: Some(Ok(read_file(&pr_guidance)?)),
            assistant_proof_json: Some(Ok(read_file(&proof)?)),
            ledger_json: Some(Ok(read_file(&ledger)?)),
            baseline_delta_json: None,
            receipt_json: None,
            gate_decision_json: None,
            coverage_frontier_json: None,
            editor_context_json: None,
        });

        assert_eq!(
            render_first_useful_action_json(&report)?,
            read_file(&base.join("first-useful-action.json"))?.trim_end()
        );
        assert_eq!(
            render_first_useful_action_markdown(&report),
            read_file(&base.join("first-useful-action.md"))?
        );
        Ok(())
    }

    #[test]
    fn first_useful_action_matches_unchanged_after_attempt_fixture() -> Result<(), String> {
        let repo_root = repo_root()?;
        let base = repo_root
            .join("fixtures/boundary_gap/expected/first-useful-action/unchanged-after-attempt");
        let proof = repo_root
            .join("fixtures/boundary_gap/expected/test-oracle-assistant-loop/canonical/test-oracle-assistant-proof.json");
        let pr_guidance = repo_root.join(
            "fixtures/boundary_gap/expected/test-oracle-assistant-loop/canonical/pr-guidance.json",
        );
        let ledger =
            repo_root.join("fixtures/boundary_gap/expected/test-oracle-assistant-loop/canonical/pr-evidence-ledger.json");
        let receipt =
            repo_root.join("fixtures/boundary_gap/expected/editor-agent-loop/agent-receipt.json");
        let report = build_first_useful_action_report(FirstUsefulActionInput {
            root: "fixtures/boundary_gap/input".to_string(),
            generated_at: "2026-05-09T12:00:00Z".to_string(),
            pr_guidance_path: Some(fixture_path(&repo_root, &pr_guidance)),
            assistant_proof_path: Some(fixture_path(&repo_root, &proof)),
            ledger_path: Some(fixture_path(&repo_root, &ledger)),
            baseline_delta_path: None,
            receipt_path: Some(fixture_path(&repo_root, &receipt)),
            gate_decision_path: None,
            coverage_frontier_path: None,
            editor_context_path: None,
            pr_guidance_json: Some(Ok(read_file(&pr_guidance)?)),
            assistant_proof_json: Some(Ok(read_file(&proof)?)),
            ledger_json: Some(Ok(read_file(&ledger)?)),
            baseline_delta_json: None,
            receipt_json: Some(Ok(read_file(&receipt)?)),
            gate_decision_json: None,
            coverage_frontier_json: None,
            editor_context_json: None,
        });

        assert_eq!(
            render_first_useful_action_json(&report)?,
            read_file(&base.join("first-useful-action.json"))?.trim_end()
        );
        assert_eq!(
            render_first_useful_action_markdown(&report),
            read_file(&base.join("first-useful-action.md"))?
        );
        Ok(())
    }

    #[test]
    fn first_useful_action_reports_stale_editor_context_first() -> Result<(), String> {
        let report = build_first_useful_action_report(FirstUsefulActionInput {
            root: "fixtures/boundary_gap/input".to_string(),
            generated_at: "2026-05-09T12:00:00Z".to_string(),
            pr_guidance_path: None,
            assistant_proof_path: None,
            ledger_path: None,
            baseline_delta_path: None,
            receipt_path: None,
            gate_decision_path: None,
            coverage_frontier_path: None,
            editor_context_path: Some("target/ripr/workflow/evidence-context.json".to_string()),
            pr_guidance_json: None,
            assistant_proof_json: None,
            ledger_json: None,
            baseline_delta_json: None,
            receipt_json: None,
            gate_decision_json: None,
            coverage_frontier_json: None,
            editor_context_json: Some(Ok(r#"{
  "freshness": "stale",
  "stale_reason": "diagnostic generation is older than the latest saved workspace refresh",
  "seam_id": "67fc764ba37d77bd",
  "seam_kind": "predicate_boundary",
  "path": "src/lib.rs",
  "line": 2,
  "classification": "weakly_exposed"
}"#
            .to_string())),
        });
        let rendered = render_first_useful_action_json(&report)?;
        assert!(rendered.contains(r#""status": "stale""#));
        assert!(rendered.contains(r#""action_kind": "refresh_evidence""#));
        assert!(rendered.contains("diagnostic generation is older"));
        Ok(())
    }

    #[test]
    fn first_useful_action_routes_missing_assistant_proof() -> Result<(), String> {
        let report = build_first_useful_action_report(FirstUsefulActionInput {
            root: ".".to_string(),
            generated_at: "2026-05-09T12:00:00Z".to_string(),
            pr_guidance_path: Some("comments.json".to_string()),
            assistant_proof_path: None,
            ledger_path: Some("ledger.json".to_string()),
            baseline_delta_path: None,
            receipt_path: None,
            gate_decision_path: None,
            coverage_frontier_path: None,
            editor_context_path: None,
            pr_guidance_json: Some(Ok(
                r#"{"comments":[{"seam_id":"seam-a","missing_discriminator":"x == 1"}]}"#
                    .to_string(),
            )),
            assistant_proof_json: None,
            ledger_json: Some(Ok(r#"{"kind":"pr_evidence_ledger"}"#.to_string())),
            baseline_delta_json: None,
            receipt_json: None,
            gate_decision_json: None,
            coverage_frontier_json: None,
            editor_context_json: None,
        });
        let rendered = render_first_useful_action_json(&report)?;
        assert!(rendered.contains(r#""status": "missing_required_artifact""#));
        assert!(rendered.contains(r#""action_kind": "generate_missing_artifact""#));
        assert!(rendered.contains(DEFAULT_TEST_ORACLE_ASSISTANT_PROOF_OUT));
        Ok(())
    }

    fn repo_root() -> Result<PathBuf, String> {
        let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        manifest_dir
            .parent()
            .and_then(Path::parent)
            .map(Path::to_path_buf)
            .ok_or_else(|| "failed to resolve repo root".to_string())
    }

    fn read_file(path: &Path) -> Result<String, String> {
        std::fs::read_to_string(path)
            .map_err(|err| format!("read {} failed: {err}", path.display()))
    }

    fn fixture_path(repo_root: &Path, path: &Path) -> String {
        match path.strip_prefix(repo_root) {
            Ok(relative) => display_path(relative),
            Err(_) => display_path(path),
        }
    }
}
