use crate::agent::loop_commands;
use serde::Serialize;
use serde_json::Value;

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
    pub(crate) gap_ledger_path: Option<String>,
    pub(crate) ledger_path: Option<String>,
    pub(crate) baseline_delta_path: Option<String>,
    pub(crate) receipt_path: Option<String>,
    pub(crate) gate_decision_path: Option<String>,
    pub(crate) coverage_frontier_path: Option<String>,
    pub(crate) editor_context_path: Option<String>,
    pub(crate) pr_guidance_json: Option<Result<String, String>>,
    pub(crate) assistant_proof_json: Option<Result<String, String>>,
    pub(crate) gap_ledger_json: Option<Result<String, String>>,
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
    #[serde(skip_serializing_if = "Option::is_none")]
    gap_ledger: Option<String>,
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
    #[serde(skip_serializing_if = "Option::is_none")]
    gap_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    canonical_gap_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    repair_route: Option<String>,
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
    #[serde(skip_serializing_if = "Option::is_none")]
    gap_ledger: Option<String>,
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
    gap_ledger: Option<Value>,
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
        gap_ledger: input.gap_ledger_path.clone(),
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
    } else if let Some(report) = gap_record_report(&input, &parsed, &inputs, &generated_at) {
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

    if should_render_one_screen_recommendation(report) {
        render_one_screen_recommendation_markdown(report, &mut out);
    }

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

fn should_render_one_screen_recommendation(report: &FirstUsefulActionReport) -> bool {
    report.selected.is_some()
        || matches!(
            report.action_kind.as_str(),
            "write_focused_test" | "revise_focused_test" | "generate_missing_artifact"
        )
}

fn render_one_screen_recommendation_markdown(report: &FirstUsefulActionReport, out: &mut String) {
    let changed_behavior = if report.why.trim().is_empty() {
        "changed behavior unavailable"
    } else {
        report.why.trim()
    };
    let evidence_strength = report
        .selected
        .as_ref()
        .and_then(|selected| selected.classification.as_deref())
        .unwrap_or(report.status.as_str());
    let missing_discriminator = report
        .selected
        .as_ref()
        .and_then(|selected| selected.missing_discriminator.as_deref())
        .or_else(|| {
            report
                .target
                .as_ref()
                .and_then(|target| target.suggested_assertion.as_deref())
        })
        .unwrap_or("missing discriminator unavailable");
    let focused_proof_intent = report
        .target
        .as_ref()
        .and_then(|target| target.suggested_assertion.as_deref())
        .unwrap_or(report.title.as_str());
    let verify_command = report.commands.verify.as_deref().unwrap_or("not_available");
    let receipt_command = report
        .commands
        .receipt
        .as_deref()
        .unwrap_or("not_available");
    let artifacts = one_screen_artifacts(report);

    out.push_str("## One-Screen Recommendation\n\n");
    out.push_str(&format!("- Changed behavior: {changed_behavior}\n"));
    out.push_str(&format!(
        "- Current evidence strength: `{evidence_strength}`\n"
    ));
    out.push_str(&format!(
        "- Missing discriminator: {missing_discriminator}\n"
    ));
    out.push_str(&format!("- Focused proof intent: {focused_proof_intent}\n"));
    out.push_str(&format!("- Verify command: `{verify_command}`\n"));
    out.push_str(&format!("- Receipt command: `{receipt_command}`\n"));
    if !artifacts.is_empty() {
        let joined = artifacts
            .into_iter()
            .map(|artifact| format!("`{artifact}`"))
            .collect::<Vec<_>>()
            .join(", ");
        out.push_str(&format!("- Artifacts: {joined}\n"));
    }
    out.push_str(
        "- Boundary: static advisory evidence only; not runtime, coverage, mutation, or gate proof.\n\n",
    );
}

fn one_screen_artifacts(report: &FirstUsefulActionReport) -> Vec<&str> {
    let mut artifacts = Vec::new();
    if let Some(selected) = report.selected.as_ref() {
        push_unique_str(&mut artifacts, selected.source_artifact.as_str());
    }
    if let Some(path) = report.evidence.pr_guidance.as_deref() {
        push_unique_str(&mut artifacts, path);
    }
    if let Some(path) = report.evidence.assistant_proof.as_deref() {
        push_unique_str(&mut artifacts, path);
    }
    if let Some(path) = report.evidence.gap_ledger.as_deref() {
        push_unique_str(&mut artifacts, path);
    }
    if let Some(path) = report.evidence.ledger.as_deref() {
        push_unique_str(&mut artifacts, path);
    }
    if let Some(path) = report.evidence.receipt.as_deref() {
        push_unique_str(&mut artifacts, path);
    }
    artifacts
}

fn push_unique_str<'a>(items: &mut Vec<&'a str>, value: &'a str) {
    if !items.contains(&value) {
        items.push(value);
    }
}

pub(crate) use crate::output::path::display_path;

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
    parsed.gap_ledger = parse_optional_json(
        "gap decision ledger",
        input.gap_ledger_path.as_deref(),
        &input.gap_ledger_json,
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

fn gap_record_report(
    input: &FirstUsefulActionInput,
    parsed: &ParsedSources,
    inputs: &ActionInputs,
    generated_at: &str,
) -> Option<FirstUsefulActionReport> {
    let record = first_actionable_gap_record(parsed.gap_ledger.as_ref()?)?;
    let repair_route = record.get("repair_route");
    let route_kind = string_from_sources(&[(repair_route, &["route_kind"])])
        .unwrap_or_else(|| "RepairGap".to_string());
    let gap_kind = string_path(record, &["kind"]).unwrap_or_else(|| "Gap".to_string());
    let gap_id = string_path(record, &["gap_id"]).unwrap_or_else(|| "unknown".to_string());
    let verify_command = first_string_array_item(record, &["verification_commands"]);
    Some(base_report(
        input,
        inputs,
        generated_at,
        "actionable",
        "developer",
        action_kind_for_gap_route(&route_kind),
        Some(selected_from_gap_record(input, record)?),
        &format!("Repair {gap_kind} via {route_kind}"),
        &format!(
            "Gap decision {gap_id} is a new PR-local Rust gap with repair route {route_kind}."
        ),
        vec![
            "The gap decision is PR-local stable Rust evidence.",
            "It is unresolved, repairable, and policy-targeted.",
            "The repair route and verification command are supplied by the gap ledger.",
        ],
        target_from_gap_record(record),
        ActionCommands {
            verify: verify_command,
            ..ActionCommands::default()
        },
        evidence(input, "unknown"),
        None,
        Vec::new(),
        vec![
            "Static evidence only.",
            "Uses explicit gap decision input.",
            "Does not run mutation testing.",
            "Does not edit source or generate tests.",
            "Does not make CI blocking by default.",
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
        gap_ledger: input.gap_ledger_path.clone(),
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

fn first_actionable_gap_record(gap_ledger: &Value) -> Option<&Value> {
    gap_records(gap_ledger).into_iter().find(|record| {
        string_path(record, &["language"]).is_some_and(|value| value == "rust")
            && string_path(record, &["language_status"]).is_some_and(|value| value == "stable")
            && string_path(record, &["scope"]).is_some_and(|value| value == "pr_local")
            && string_path(record, &["gap_state"]).is_some_and(|value| value == "actionable")
            && string_path(record, &["repairability"]).is_some_and(|value| value == "repairable")
            && string_path(record, &["policy_state"])
                .is_some_and(|value| value == "new" || value == "reintroduced")
            && record.get("repair_route").is_some()
            && record
                .get("verification_commands")
                .and_then(Value::as_array)
                .is_some_and(|commands| {
                    commands
                        .iter()
                        .filter_map(Value::as_str)
                        .any(|command| !command.trim().is_empty())
                })
    })
}

fn gap_records(value: &Value) -> Vec<&Value> {
    if let Some(records) = value.as_array() {
        return records.iter().collect();
    }
    if let Some(records) = value.get("records").and_then(Value::as_array) {
        return records.iter().collect();
    }
    if let Some(records) = value.get("gap_records").and_then(Value::as_array) {
        return records.iter().collect();
    }
    value
        .get("cases")
        .and_then(Value::as_array)
        .map(|cases| {
            cases
                .iter()
                .filter_map(|case| case.get("expected_gap_record"))
                .collect()
        })
        .unwrap_or_default()
}

fn selected_from_gap_record(
    input: &FirstUsefulActionInput,
    record: &Value,
) -> Option<ActionSelected> {
    let repair_route = record.get("repair_route");
    let anchor = record.get("anchor");
    Some(ActionSelected {
        source: "gap_ledger".to_string(),
        source_artifact: input.gap_ledger_path.clone()?,
        seam_id: None,
        seam_kind: string_path(record, &["evidence_class"]),
        path: string_from_sources(&[(anchor, &["file"]), (repair_route, &["target_file"])]),
        line: u64_from_sources(&[(anchor, &["line"]), (repair_route, &["target_line"])]),
        classification: string_path(record, &["gap_state"]),
        missing_discriminator: string_path(repair_route?, &["assertion_shape"]),
        gap_id: string_path(record, &["gap_id"]),
        canonical_gap_id: string_path(record, &["canonical_gap_id"]),
        repair_route: string_path(repair_route?, &["route_kind"]),
    })
}

fn target_from_gap_record(record: &Value) -> Option<ActionTarget> {
    let repair_route = record.get("repair_route")?;
    Some(ActionTarget {
        file: string_path(repair_route, &["target_file"]),
        related_test: string_path(repair_route, &["related_test"]),
        suggested_test_name: None,
        suggested_assertion: string_path(repair_route, &["assertion_shape"]),
    })
    .filter(|target| {
        target.file.is_some()
            || target.related_test.is_some()
            || target.suggested_assertion.is_some()
    })
}

fn action_kind_for_gap_route(route_kind: &str) -> &'static str {
    if route_kind == "AddOutputGolden" || route_kind == "RegenerateArtifact" {
        "generate_missing_artifact"
    } else {
        "write_focused_test"
    }
}

fn first_string_array_item(value: &Value, path: &[&str]) -> Option<String> {
    path_value(value, path)
        .and_then(Value::as_array)?
        .iter()
        .filter_map(Value::as_str)
        .find(|item| !item.trim().is_empty())
        .map(ToOwned::to_owned)
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
        gap_id: None,
        canonical_gap_id: None,
        repair_route: None,
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
        gap_id: None,
        canonical_gap_id: None,
        repair_route: None,
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
        gap_id: None,
        canonical_gap_id: None,
        repair_route: None,
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
        gap_id: None,
        canonical_gap_id: None,
        repair_route: None,
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
        gap_id: None,
        canonical_gap_id: None,
        repair_route: None,
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
        gap_id: None,
        canonical_gap_id: None,
        repair_route: None,
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
        gap_id: None,
        canonical_gap_id: None,
        repair_route: None,
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
    ])
    .is_some_and(|value| value == "waived" || value == "visible")
        || string_from_sources(&[(Some(gate), &["status"]), (Some(gate), &["decision"])])
            .is_some_and(|value| value == "waived")
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
        || input.gap_ledger_path.is_some()
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
        || parsed.gap_ledger.is_some()
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
    use crate::output::test_support::{read_file, repo_root};
    use std::path::Path;

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
            gap_ledger_path: None,
            ledger_path: Some(fixture_path(&repo_root, &ledger)),
            baseline_delta_path: None,
            receipt_path: None,
            gate_decision_path: None,
            coverage_frontier_path: None,
            editor_context_path: None,
            pr_guidance_json: Some(Ok(read_file(&pr_guidance)?)),
            assistant_proof_json: Some(Ok(read_file(&proof)?)),
            gap_ledger_json: None,
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
            gap_ledger_path: None,
            ledger_path: Some(fixture_path(&repo_root, &ledger)),
            baseline_delta_path: None,
            receipt_path: Some(fixture_path(&repo_root, &receipt)),
            gate_decision_path: None,
            coverage_frontier_path: None,
            editor_context_path: None,
            pr_guidance_json: Some(Ok(read_file(&pr_guidance)?)),
            assistant_proof_json: Some(Ok(read_file(&proof)?)),
            gap_ledger_json: None,
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
            gap_ledger_path: None,
            ledger_path: None,
            baseline_delta_path: None,
            receipt_path: None,
            gate_decision_path: None,
            coverage_frontier_path: None,
            editor_context_path: Some("target/ripr/workflow/evidence-context.json".to_string()),
            pr_guidance_json: None,
            assistant_proof_json: None,
            gap_ledger_json: None,
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
            gap_ledger_path: None,
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
            gap_ledger_json: None,
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

    #[test]
    fn first_useful_action_routes_gap_record_without_assistant_proof() -> Result<(), String> {
        let report = build_first_useful_action_report(FirstUsefulActionInput {
            root: ".".to_string(),
            generated_at: "2026-05-09T12:00:00Z".to_string(),
            pr_guidance_path: Some("comments.json".to_string()),
            assistant_proof_path: None,
            gap_ledger_path: Some("gap-decision-ledger.json".to_string()),
            ledger_path: None,
            baseline_delta_path: None,
            receipt_path: None,
            gate_decision_path: None,
            coverage_frontier_path: None,
            editor_context_path: None,
            pr_guidance_json: Some(Ok(
                r#"{"comments":[{"seam_id":"raw-a","classification":"static_unknown"}]}"#
                    .to_string(),
            )),
            assistant_proof_json: None,
            gap_ledger_json: Some(Ok(r#"{
  "kind": "gap_decision_ledger",
  "records": [
    {
      "gap_id": "gap:pr:pricing:threshold-boundary",
      "canonical_gap_id": "gap:rust:pricing:discount:threshold-boundary",
      "kind": "MissingBoundaryAssertion",
      "language": "rust",
      "language_status": "stable",
      "scope": "pr_local",
      "evidence_class": "predicate_boundary",
      "gap_state": "actionable",
      "policy_state": "new",
      "repairability": "repairable",
      "anchor": {
        "file": "src/pricing.rs",
        "line": 42,
        "dedupe_fingerprint": "gap:rust:pricing:discount:threshold-boundary"
      },
      "repair_route": {
        "route_kind": "AddBoundaryAssertion",
        "target_file": "tests/pricing.rs",
        "related_test": "tests/pricing.rs::below_threshold_has_no_discount",
        "assertion_shape": "assert_eq!(discount(100, 100), 90)"
      },
      "verification_commands": [
        "cargo xtask fixtures boundary_gap"
      ]
    }
  ]
}"#
            .to_string())),
            ledger_json: None,
            baseline_delta_json: None,
            receipt_json: None,
            gate_decision_json: None,
            coverage_frontier_json: None,
            editor_context_json: None,
        });
        let rendered = render_first_useful_action_json(&report)?;
        assert!(rendered.contains(r#""source": "gap_ledger""#));
        assert!(rendered.contains(r#""gap_id": "gap:pr:pricing:threshold-boundary""#));
        assert!(rendered.contains(r#""repair_route": "AddBoundaryAssertion""#));
        assert!(rendered.contains(r#""verify": "cargo xtask fixtures boundary_gap""#));
        assert!(!rendered.contains(DEFAULT_TEST_ORACLE_ASSISTANT_PROOF_OUT));
        assert!(!rendered.contains("Confidence"));
        let markdown = render_first_useful_action_markdown(&report);
        assert!(markdown.contains("Repair MissingBoundaryAssertion via AddBoundaryAssertion"));
        assert!(markdown.contains("`cargo xtask fixtures boundary_gap`"));
        Ok(())
    }

    #[test]
    fn first_useful_action_supports_gap_record_shapes_and_output_routes() -> Result<(), String> {
        let raw_array: Value = serde_json::from_str(&format!("[{}]", output_contract_gap_record()))
            .map_err(|err| format!("parse raw gap records: {err}"))?;
        assert_eq!(gap_records(&raw_array).len(), 1);

        let wrapped_gap_records: Value = serde_json::from_str(&format!(
            r#"{{"gap_records":[{}]}}"#,
            output_contract_gap_record()
        ))
        .map_err(|err| format!("parse wrapped gap_records: {err}"))?;
        assert!(first_actionable_gap_record(&wrapped_gap_records).is_some());

        let fixture_cases: Value = serde_json::from_str(&format!(
            r#"{{"cases":[{{"expected_gap_record":{}}}]}}"#,
            output_contract_gap_record()
        ))
        .map_err(|err| format!("parse fixture-style gap records: {err}"))?;
        assert!(first_actionable_gap_record(&fixture_cases).is_some());

        let report = build_first_useful_action_report(FirstUsefulActionInput {
            root: ".".to_string(),
            generated_at: "2026-05-09T12:00:00Z".to_string(),
            pr_guidance_path: None,
            assistant_proof_path: None,
            gap_ledger_path: Some("gap-decision-ledger.json".to_string()),
            ledger_path: None,
            baseline_delta_path: None,
            receipt_path: None,
            gate_decision_path: None,
            coverage_frontier_path: None,
            editor_context_path: None,
            pr_guidance_json: None,
            assistant_proof_json: None,
            gap_ledger_json: Some(Ok(format!(
                r#"{{"gap_records":[{}]}}"#,
                output_contract_gap_record()
            ))),
            ledger_json: None,
            baseline_delta_json: None,
            receipt_json: None,
            gate_decision_json: None,
            coverage_frontier_json: None,
            editor_context_json: None,
        });

        let rendered = render_first_useful_action_json(&report)?;
        assert!(rendered.contains(r#""action_kind": "generate_missing_artifact""#));
        assert!(rendered.contains(r#""repair_route": "AddOutputGolden""#));
        assert!(rendered.contains(r#""verify": "cargo xtask goldens check""#));
        assert!(rendered.contains(r#""file": "fixtures/boundary_gap/expected/output.json""#));
        Ok(())
    }

    fn output_contract_gap_record() -> &'static str {
        r#"{
  "gap_id": "gap:pr:output:first-action-golden",
  "canonical_gap_id": "gap:rust:output:first-action-golden",
  "kind": "MissingOutputContract",
  "language": "rust",
  "language_status": "stable",
  "scope": "pr_local",
  "evidence_class": "presentation_text",
  "gap_state": "actionable",
  "policy_state": "reintroduced",
  "repairability": "repairable",
  "anchor": {
    "file": "crates/ripr/src/output/human.rs",
    "line": 17,
    "dedupe_fingerprint": "gap:rust:output:first-action-golden"
  },
  "repair_route": {
    "route_kind": "AddOutputGolden",
    "target_file": "fixtures/boundary_gap/expected/output.json",
    "target_line": 1,
    "assertion_shape": "refresh output golden"
  },
  "verification_commands": [
    "cargo xtask goldens check"
  ]
}"#
    }

    fn fixture_path(repo_root: &Path, path: &Path) -> String {
        match path.strip_prefix(repo_root) {
            Ok(relative) => display_path(relative),
            Err(_) => display_path(path),
        }
    }

    // ── helper: build a bare-minimum FirstUsefulActionInput with all Nones ──

    fn bare_input() -> FirstUsefulActionInput {
        FirstUsefulActionInput {
            root: ".".to_string(),
            generated_at: "2026-01-01T00:00:00Z".to_string(),
            pr_guidance_path: None,
            assistant_proof_path: None,
            gap_ledger_path: None,
            ledger_path: None,
            baseline_delta_path: None,
            receipt_path: None,
            gate_decision_path: None,
            coverage_frontier_path: None,
            editor_context_path: None,
            pr_guidance_json: None,
            assistant_proof_json: None,
            gap_ledger_json: None,
            ledger_json: None,
            baseline_delta_json: None,
            receipt_json: None,
            gate_decision_json: None,
            coverage_frontier_json: None,
            editor_context_json: None,
        }
    }

    // ── parse_optional_json branches ─────────────────────────────────────────

    #[test]
    fn parse_optional_json_no_path_returns_none() -> Result<(), String> {
        // When path is None the function immediately returns None without touching
        // ParsedSources – zero warnings, zero read_errors.
        let mut parsed = ParsedSources::default();
        let result = parse_optional_json("label", None, &Some(Ok("{}".to_string())), &mut parsed);
        assert!(result.is_none(), "expected None when path is None");
        assert!(
            parsed.warnings.is_empty(),
            "expected no warnings but got {:?}",
            parsed.warnings
        );
        Ok(())
    }

    #[test]
    fn parse_optional_json_path_but_no_text_records_warning() -> Result<(), String> {
        // path is Some, but text (Option<Result>) is None → warning + read_error
        let mut parsed = ParsedSources::default();
        let result = parse_optional_json("my-label", Some("some/path.json"), &None, &mut parsed);
        assert!(result.is_none(), "expected None when text is absent");
        assert!(
            !(parsed.warnings.is_empty()),
            "expected a warning when text is absent"
        );
        assert!(
            parsed.warnings[0].contains("some/path.json")
                && parsed.warnings[0].contains("my-label"),
            "unexpected warning text: {}",
            parsed.warnings[0]
        );
        assert!(
            !(parsed.read_errors.is_empty()),
            "expected a read_error entry"
        );
        Ok(())
    }

    #[test]
    fn parse_optional_json_io_error_records_warning() -> Result<(), String> {
        // text is Some(Err(...)) → warning + read_error
        let mut parsed = ParsedSources::default();
        let result = parse_optional_json(
            "my-label",
            Some("broken.json"),
            &Some(Err("permission denied".to_string())),
            &mut parsed,
        );
        assert!(result.is_none(), "expected None on Err text");
        let warning = parsed.warnings.first().ok_or("expected a warning")?.clone();
        assert!(
            warning.contains("permission denied"),
            "expected error text in warning, got: {warning}"
        );
        Ok(())
    }

    #[test]
    fn parse_optional_json_invalid_json_records_warning() -> Result<(), String> {
        // text is Some(Ok(...)) but not valid JSON → warning + read_error
        let mut parsed = ParsedSources::default();
        let result = parse_optional_json(
            "my-label",
            Some("bad.json"),
            &Some(Ok("not json {{{".to_string())),
            &mut parsed,
        );
        assert!(result.is_none(), "expected None on invalid JSON");
        assert!(
            !(parsed.warnings.is_empty()),
            "expected warning on invalid JSON"
        );
        assert!(
            !(parsed.read_errors.is_empty()),
            "expected read_error on invalid JSON"
        );
        Ok(())
    }

    #[test]
    fn parse_optional_json_valid_json_returns_value() -> Result<(), String> {
        let mut parsed = ParsedSources::default();
        let result = parse_optional_json(
            "my-label",
            Some("ok.json"),
            &Some(Ok(r#"{"key": "value"}"#.to_string())),
            &mut parsed,
        );
        let val = result.ok_or("expected Some(Value) on valid JSON")?;
        assert!(
            (val.get("key").and_then(|v| v.as_str()) == Some("value")),
            "unexpected parsed value"
        );
        assert!(
            parsed.warnings.is_empty(),
            "expected no warnings on valid JSON"
        );
        Ok(())
    }

    // ── generated_at empty / whitespace → DEFAULT_GENERATED_AT ─────────────

    #[test]
    fn empty_generated_at_uses_default() -> Result<(), String> {
        let mut input = bare_input();
        input.generated_at = "  ".to_string();
        let report = build_first_useful_action_report(input);
        let rendered = render_first_useful_action_json(&report)?;
        assert!(
            rendered.contains(&format!(r#""generated_at": "{DEFAULT_GENERATED_AT}""#)),
            "expected generated_at to be '{DEFAULT_GENERATED_AT}' in: {rendered}"
        );
        Ok(())
    }

    // ── no_actionable_report – both warning and no-warning branches ──────────

    #[test]
    fn no_actionable_report_with_no_inputs_warns() -> Result<(), String> {
        // All inputs None → warning injected
        let report = build_first_useful_action_report(bare_input());
        let rendered = render_first_useful_action_json(&report)?;
        assert!(
            rendered.contains(r#""status": "no_actionable_seam""#),
            "expected no_actionable_seam status but got: {rendered}"
        );
        assert!(
            rendered.contains("no explicit first-action artifact input was supplied"),
            "expected warning about no inputs"
        );
        Ok(())
    }

    #[test]
    fn no_actionable_report_with_inputs_no_warning() -> Result<(), String> {
        // Some input paths provided but all JSON parses fine to non-actionable content
        let mut input = bare_input();
        input.pr_guidance_path = Some("guidance.json".to_string());
        input.pr_guidance_json = Some(Ok(r#"{"comments":[]}"#.to_string()));
        let report = build_first_useful_action_report(input);
        let rendered = render_first_useful_action_json(&report)?;
        assert!(
            rendered.contains(r#""status": "no_actionable_seam""#),
            "expected no_actionable_seam"
        );
        assert!(
            !(rendered.contains("no explicit first-action artifact input was supplied")),
            "should not warn when inputs are present"
        );
        Ok(())
    }

    // ── read_error_report ────────────────────────────────────────────────────

    #[test]
    fn read_error_triggers_missing_required_report() -> Result<(), String> {
        // Providing a path with no JSON text creates a read_error
        let mut input = bare_input();
        input.pr_guidance_path = Some("guidance.json".to_string());
        input.pr_guidance_json = None; // path given but text not loaded
        let report = build_first_useful_action_report(input);
        let rendered = render_first_useful_action_json(&report)?;
        assert!(
            rendered.contains(r#""status": "missing_required_artifact""#),
            "expected missing_required_artifact but got: {rendered}"
        );
        assert!(
            rendered.contains("guidance.json"),
            "expected missing path in report"
        );
        Ok(())
    }

    // ── receipt_report: improved/resolved ────────────────────────────────────

    #[test]
    fn receipt_improved_routes_already_improved() -> Result<(), String> {
        let receipt_json = r#"{
            "provenance": { "movement": "improved", "seam_id": "seam-abc" }
        }"#;
        let mut input = bare_input();
        input.receipt_path = Some("receipt.json".to_string());
        input.receipt_json = Some(Ok(receipt_json.to_string()));
        let report = build_first_useful_action_report(input);
        let rendered = render_first_useful_action_json(&report)?;
        assert!(
            rendered.contains(r#""status": "already_improved""#),
            "expected already_improved status but got: {rendered}"
        );
        Ok(())
    }

    #[test]
    fn receipt_resolved_routes_already_improved() -> Result<(), String> {
        let receipt_json = r#"{
            "provenance": { "movement": "resolved", "seam_id": "seam-xyz" }
        }"#;
        let mut input = bare_input();
        input.receipt_path = Some("receipt.json".to_string());
        input.receipt_json = Some(Ok(receipt_json.to_string()));
        let report = build_first_useful_action_report(input);
        let rendered = render_first_useful_action_json(&report)?;
        assert!(
            rendered.contains(r#""status": "already_improved""#),
            "expected already_improved but got: {rendered}"
        );
        Ok(())
    }

    #[test]
    fn receipt_unchanged_routes_unchanged_after_attempt() -> Result<(), String> {
        let receipt_json = r#"{
            "provenance": { "movement": "unchanged", "seam_id": "seam-u" }
        }"#;
        let mut input = bare_input();
        input.receipt_path = Some("receipt.json".to_string());
        input.receipt_json = Some(Ok(receipt_json.to_string()));
        let report = build_first_useful_action_report(input);
        let rendered = render_first_useful_action_json(&report)?;
        assert!(
            rendered.contains(r#""status": "unchanged_after_attempt""#),
            "expected unchanged_after_attempt but got: {rendered}"
        );
        Ok(())
    }

    #[test]
    fn receipt_unknown_movement_does_not_route_receipt() -> Result<(), String> {
        // movement = "other" → receipt_report returns None → falls through
        let receipt_json = r#"{"provenance": {"movement": "other"}}"#;
        let mut input = bare_input();
        input.receipt_path = Some("receipt.json".to_string());
        input.receipt_json = Some(Ok(receipt_json.to_string()));
        let report = build_first_useful_action_report(input);
        let rendered = render_first_useful_action_json(&report)?;
        assert!(
            !(rendered.contains(r#""status": "already_improved""#)
                || rendered.contains(r#""status": "unchanged_after_attempt""#)),
            "should not route as receipt for unknown movement: {rendered}"
        );
        Ok(())
    }

    #[test]
    fn receipt_movement_from_seam_change_field() -> Result<(), String> {
        // receipt_movement also reads ["seam"]["change"]
        let receipt_json = r#"{"seam": {"change": "improved"}}"#;
        let mut input = bare_input();
        input.receipt_path = Some("receipt.json".to_string());
        input.receipt_json = Some(Ok(receipt_json.to_string()));
        let report = build_first_useful_action_report(input);
        let rendered = render_first_useful_action_json(&report)?;
        assert!(
            rendered.contains(r#""status": "already_improved""#),
            "expected already_improved but got: {rendered}"
        );
        Ok(())
    }

    // ── suppressed_report ───────────────────────────────────────────────────

    #[test]
    fn suppressed_guidance_routes_suppressed() -> Result<(), String> {
        let pr_guidance_json = r#"{
            "suppressed": [{"seam_id": "seam-s", "kind": "predicate_boundary"}]
        }"#;
        let mut input = bare_input();
        input.pr_guidance_path = Some("guidance.json".to_string());
        input.pr_guidance_json = Some(Ok(pr_guidance_json.to_string()));
        let report = build_first_useful_action_report(input);
        let rendered = render_first_useful_action_json(&report)?;
        assert!(
            rendered.contains(r#""status": "suppressed""#),
            "expected suppressed status but got: {rendered}"
        );
        assert!(
            rendered.contains(r#""action_kind": "no_action""#),
            "expected no_action for suppressed"
        );
        Ok(())
    }

    #[test]
    fn suppressed_guidance_via_warning_text() -> Result<(), String> {
        // has_suppressed_guidance also checks warnings array containing "configured off"
        let pr_guidance_json = r#"{
            "warnings": ["seam configured off by policy"]
        }"#;
        let mut input = bare_input();
        input.pr_guidance_path = Some("guidance.json".to_string());
        input.pr_guidance_json = Some(Ok(pr_guidance_json.to_string()));
        let report = build_first_useful_action_report(input);
        let rendered = render_first_useful_action_json(&report)?;
        assert!(
            rendered.contains(r#""status": "suppressed""#),
            "expected suppressed from warning text, got: {rendered}"
        );
        Ok(())
    }

    #[test]
    fn suppressed_guidance_via_warning_text_suppressed_keyword() -> Result<(), String> {
        let pr_guidance_json = r#"{
            "warnings": ["seam is suppressed by ripr policy"]
        }"#;
        let mut input = bare_input();
        input.pr_guidance_path = Some("guidance.json".to_string());
        input.pr_guidance_json = Some(Ok(pr_guidance_json.to_string()));
        let report = build_first_useful_action_report(input);
        let rendered = render_first_useful_action_json(&report)?;
        assert!(
            rendered.contains(r#""status": "suppressed""#),
            "expected suppressed from warning text, got: {rendered}"
        );
        Ok(())
    }

    // ── acknowledged_report via baseline_delta ───────────────────────────────

    #[test]
    fn acknowledged_bucket_in_baseline_delta_routes_acknowledged() -> Result<(), String> {
        let delta_json = r#"{
            "items": [
                {"bucket": "acknowledged", "path": "src/lib.rs", "line": 10, "kind": "predicate_boundary"}
            ]
        }"#;
        let mut input = bare_input();
        input.baseline_delta_path = Some("delta.json".to_string());
        input.baseline_delta_json = Some(Ok(delta_json.to_string()));
        let report = build_first_useful_action_report(input);
        let rendered = render_first_useful_action_json(&report)?;
        assert!(
            rendered.contains(r#""status": "acknowledged""#),
            "expected acknowledged but got: {rendered}"
        );
        Ok(())
    }

    #[test]
    fn acknowledged_via_ledger_movement_count() -> Result<(), String> {
        // No baseline_delta, but ledger has acknowledged count > 0
        let ledger_json = r#"{
            "movement": {"acknowledged": 1},
            "top_repair_route": {"seam_id": "seam-ack"}
        }"#;
        let mut input = bare_input();
        input.ledger_path = Some("ledger.json".to_string());
        input.ledger_json = Some(Ok(ledger_json.to_string()));
        let report = build_first_useful_action_report(input);
        let rendered = render_first_useful_action_json(&report)?;
        assert!(
            rendered.contains(r#""status": "acknowledged""#),
            "expected acknowledged via ledger but got: {rendered}"
        );
        Ok(())
    }

    #[test]
    fn zero_acknowledged_in_ledger_does_not_route_acknowledged() -> Result<(), String> {
        let ledger_json = r#"{"movement": {"acknowledged": 0}}"#;
        let mut input = bare_input();
        input.ledger_path = Some("ledger.json".to_string());
        input.ledger_json = Some(Ok(ledger_json.to_string()));
        let report = build_first_useful_action_report(input);
        let rendered = render_first_useful_action_json(&report)?;
        assert!(
            !(rendered.contains(r#""status": "acknowledged""#)),
            "should not route acknowledged when count is 0"
        );
        Ok(())
    }

    // ── waived_report ────────────────────────────────────────────────────────

    #[test]
    fn gate_decision_with_waiver_routes_waived() -> Result<(), String> {
        let gate_json = r#"{
            "waiver": {"state": "waived"},
            "seam_id": "seam-w",
            "path": "src/main.rs",
            "line": 5
        }"#;
        let mut input = bare_input();
        input.gate_decision_path = Some("gate.json".to_string());
        input.gate_decision_json = Some(Ok(gate_json.to_string()));
        let report = build_first_useful_action_report(input);
        let rendered = render_first_useful_action_json(&report)?;
        assert!(
            rendered.contains(r#""status": "waived""#),
            "expected waived status but got: {rendered}"
        );
        Ok(())
    }

    #[test]
    fn gate_decision_waivers_array_non_empty_routes_waived() -> Result<(), String> {
        let gate_json = r#"{
            "waivers": [{"id": "w1"}],
            "seam_id": "seam-ww"
        }"#;
        let mut input = bare_input();
        input.gate_decision_path = Some("gate.json".to_string());
        input.gate_decision_json = Some(Ok(gate_json.to_string()));
        let report = build_first_useful_action_report(input);
        let rendered = render_first_useful_action_json(&report)?;
        assert!(
            rendered.contains(r#""status": "waived""#),
            "expected waived via waivers array but got: {rendered}"
        );
        Ok(())
    }

    #[test]
    fn gate_decision_visible_waiver_routes_waived() -> Result<(), String> {
        let gate_json = r#"{"waiver": "visible"}"#;
        let mut input = bare_input();
        input.gate_decision_path = Some("gate.json".to_string());
        input.gate_decision_json = Some(Ok(gate_json.to_string()));
        let report = build_first_useful_action_report(input);
        let rendered = render_first_useful_action_json(&report)?;
        assert!(
            rendered.contains(r#""status": "waived""#),
            "expected waived for visible waiver but got: {rendered}"
        );
        Ok(())
    }

    #[test]
    fn gate_decision_visible_status_does_not_route_waived() -> Result<(), String> {
        let gate_json = r#"{"status": "visible"}"#;
        let mut input = bare_input();
        input.gate_decision_path = Some("gate.json".to_string());
        input.gate_decision_json = Some(Ok(gate_json.to_string()));
        let report = build_first_useful_action_report(input);
        let rendered = render_first_useful_action_json(&report)?;
        assert!(
            !(rendered.contains(r#""status": "waived""#)),
            "visible status should not route waived: {rendered}"
        );
        Ok(())
    }

    #[test]
    fn gate_decision_without_waiver_does_not_route_waived() -> Result<(), String> {
        let gate_json = r#"{"status": "blocking"}"#;
        let mut input = bare_input();
        input.gate_decision_path = Some("gate.json".to_string());
        input.gate_decision_json = Some(Ok(gate_json.to_string()));
        let report = build_first_useful_action_report(input);
        let rendered = render_first_useful_action_json(&report)?;
        assert!(
            !(rendered.contains(r#""status": "waived""#)),
            "should not route waived for non-waived gate"
        );
        Ok(())
    }

    // ── baseline_only_report ─────────────────────────────────────────────────

    #[test]
    fn baseline_delta_still_present_routes_baseline_only() -> Result<(), String> {
        let delta_json = r#"{
            "items": [
                {"bucket": "still_present", "path": "src/lib.rs", "line": 20}
            ]
        }"#;
        let mut input = bare_input();
        input.baseline_delta_path = Some("delta.json".to_string());
        input.baseline_delta_json = Some(Ok(delta_json.to_string()));
        let report = build_first_useful_action_report(input);
        let rendered = render_first_useful_action_json(&report)?;
        assert!(
            rendered.contains(r#""status": "baseline_only""#),
            "expected baseline_only status but got: {rendered}"
        );
        assert!(
            rendered.contains(r#""action_kind": "acknowledge_baseline""#),
            "expected acknowledge_baseline action"
        );
        Ok(())
    }

    #[test]
    fn baseline_delta_only_bucket_routes_baseline_only() -> Result<(), String> {
        let delta_json = r#"{
            "items": [
                {"bucket": "baseline_only", "path": "src/other.rs", "line": 5}
            ]
        }"#;
        let mut input = bare_input();
        input.baseline_delta_path = Some("delta.json".to_string());
        input.baseline_delta_json = Some(Ok(delta_json.to_string()));
        let report = build_first_useful_action_report(input);
        let rendered = render_first_useful_action_json(&report)?;
        assert!(
            rendered.contains(r#""status": "baseline_only""#),
            "expected baseline_only but got: {rendered}"
        );
        Ok(())
    }

    // ── is_stale variants ────────────────────────────────────────────────────

    #[test]
    fn is_stale_detects_analysis_stale_status() -> Result<(), String> {
        let ctx_json = r#"{"status": "analysis_stale", "seam_id": "seam-s"}"#;
        let mut input = bare_input();
        input.editor_context_path = Some("ctx.json".to_string());
        input.editor_context_json = Some(Ok(ctx_json.to_string()));
        let report = build_first_useful_action_report(input);
        let rendered = render_first_useful_action_json(&report)?;
        assert!(
            rendered.contains(r#""status": "stale""#),
            "expected stale for analysis_stale status but got: {rendered}"
        );
        Ok(())
    }

    #[test]
    fn is_stale_detects_stale_state_field() -> Result<(), String> {
        let ctx_json = r#"{"state": "stale", "seam_id": "seam-s"}"#;
        let mut input = bare_input();
        input.editor_context_path = Some("ctx.json".to_string());
        input.editor_context_json = Some(Ok(ctx_json.to_string()));
        let report = build_first_useful_action_report(input);
        let rendered = render_first_useful_action_json(&report)?;
        assert!(
            rendered.contains(r#""status": "stale""#),
            "expected stale from state field but got: {rendered}"
        );
        Ok(())
    }

    #[test]
    fn is_stale_detects_stale_evidence_state_field() -> Result<(), String> {
        let ctx_json = r#"{"evidence_state": "stale", "seam_id": "seam-s"}"#;
        let mut input = bare_input();
        input.editor_context_path = Some("ctx.json".to_string());
        input.editor_context_json = Some(Ok(ctx_json.to_string()));
        let report = build_first_useful_action_report(input);
        let rendered = render_first_useful_action_json(&report)?;
        assert!(
            rendered.contains(r#""status": "stale""#),
            "expected stale from evidence_state but got: {rendered}"
        );
        Ok(())
    }

    #[test]
    fn is_stale_detects_bool_stale_field() -> Result<(), String> {
        let ctx_json = r#"{"stale": true, "seam_id": "seam-s"}"#;
        let mut input = bare_input();
        input.editor_context_path = Some("ctx.json".to_string());
        input.editor_context_json = Some(Ok(ctx_json.to_string()));
        let report = build_first_useful_action_report(input);
        let rendered = render_first_useful_action_json(&report)?;
        assert!(
            rendered.contains(r#""status": "stale""#),
            "expected stale from bool stale field but got: {rendered}"
        );
        Ok(())
    }

    #[test]
    fn is_stale_with_reason_included_in_warnings() -> Result<(), String> {
        // stale_warnings picks up "reason", "stale_reason", "freshness_reason"
        let ctx_json = r#"{"freshness": "stale", "reason": "outdated cache"}"#;
        let mut input = bare_input();
        input.editor_context_path = Some("ctx.json".to_string());
        input.editor_context_json = Some(Ok(ctx_json.to_string()));
        let report = build_first_useful_action_report(input);
        let rendered = render_first_useful_action_json(&report)?;
        assert!(
            rendered.contains("outdated cache"),
            "expected stale reason in warnings but got: {rendered}"
        );
        Ok(())
    }

    #[test]
    fn is_stale_with_freshness_reason_included_in_warnings() -> Result<(), String> {
        let ctx_json = r#"{"freshness": "stale", "freshness_reason": "file changed"}"#;
        let mut input = bare_input();
        input.editor_context_path = Some("ctx.json".to_string());
        input.editor_context_json = Some(Ok(ctx_json.to_string()));
        let report = build_first_useful_action_report(input);
        let rendered = render_first_useful_action_json(&report)?;
        assert!(
            rendered.contains("file changed"),
            "expected freshness_reason in warnings: {rendered}"
        );
        Ok(())
    }

    #[test]
    fn non_stale_editor_context_does_not_route_stale() -> Result<(), String> {
        let ctx_json = r#"{"freshness": "current", "seam_id": "seam-s"}"#;
        let mut input = bare_input();
        input.editor_context_path = Some("ctx.json".to_string());
        input.editor_context_json = Some(Ok(ctx_json.to_string()));
        let report = build_first_useful_action_report(input);
        let rendered = render_first_useful_action_json(&report)?;
        assert!(
            !(rendered.contains(r#""status": "stale""#)),
            "should not route stale for current freshness"
        );
        Ok(())
    }

    // ── render_first_useful_action_markdown branches ─────────────────────────

    #[test]
    fn markdown_includes_why_first_when_non_empty() -> Result<(), String> {
        // The existing actionable fixture exercises why_first – build a simple
        // actionable report from inline JSON so we don't need file I/O.
        let proof_json = r#"{
            "seam": {
                "seam_id": "seam-md",
                "seam_kind": "predicate_boundary",
                "path": "src/lib.rs",
                "line": 5,
                "grip_class": "weakly_gripped",
                "missing_discriminator": "assert boundary"
            },
            "recommendation": {
                "related_test": "tests/lib.rs::test_me",
                "suggested_test_name": "test_boundary"
            }
        }"#;
        let pr_guidance_json = r#"{"comments":[{"seam_id":"seam-md"}]}"#;
        let mut input = bare_input();
        input.pr_guidance_path = Some("g.json".to_string());
        input.assistant_proof_path = Some("proof.json".to_string());
        input.pr_guidance_json = Some(Ok(pr_guidance_json.to_string()));
        input.assistant_proof_json = Some(Ok(proof_json.to_string()));

        let report = build_first_useful_action_report(input);
        let md = render_first_useful_action_markdown(&report);

        // The actionable report has why_first bullets
        assert!(
            md.contains("## Why First"),
            "expected Why First section in markdown: {md}"
        );
        Ok(())
    }

    #[test]
    fn markdown_includes_where_section_for_write_focused_test() -> Result<(), String> {
        let proof_json = r#"{
            "seam": {
                "seam_id": "seam-where",
                "seam_kind": "predicate_boundary",
                "path": "src/lib.rs",
                "line": 7,
                "grip_class": "weakly_gripped"
            },
            "recommendation": {
                "related_test": "tests/lib.rs::test_boundary",
                "recommended_file": "tests/lib.rs",
                "suggested_test_name": "test_boundary_at_seven"
            }
        }"#;
        let pr_guidance_json = r#"{"comments":[{"seam_id":"seam-where","suggested_test":{"recommended_file":"tests/lib.rs","near_test":"tests/lib.rs::test_boundary","recommended_name":"test_boundary_at_seven"}}]}"#;
        let mut input = bare_input();
        input.pr_guidance_path = Some("g.json".to_string());
        input.assistant_proof_path = Some("proof.json".to_string());
        input.pr_guidance_json = Some(Ok(pr_guidance_json.to_string()));
        input.assistant_proof_json = Some(Ok(proof_json.to_string()));

        let report = build_first_useful_action_report(input);
        let md = render_first_useful_action_markdown(&report);

        assert!(
            md.contains("## Where"),
            "expected Where section in markdown: {md}"
        );
        Ok(())
    }

    #[test]
    fn markdown_fallback_with_missing_artifact() -> Result<(), String> {
        // missing_required_report includes fallback with missing field
        let mut input = bare_input();
        input.pr_guidance_path = Some("g.json".to_string());
        input.pr_guidance_json = None; // forces read_error → missing_required_report
        let report = build_first_useful_action_report(input);
        let md = render_first_useful_action_markdown(&report);
        assert!(
            md.contains("## Fallback"),
            "expected Fallback section: {md}"
        );
        assert!(
            md.contains("Missing required artifact"),
            "expected 'Missing required artifact' text: {md}"
        );
        Ok(())
    }

    #[test]
    fn markdown_fallback_with_summary_text() -> Result<(), String> {
        // suppressed report uses fallback with summary (not missing)
        let pr_guidance_json = r#"{"suppressed":[{"seam_id":"seam-s"}]}"#;
        let mut input = bare_input();
        input.pr_guidance_path = Some("g.json".to_string());
        input.pr_guidance_json = Some(Ok(pr_guidance_json.to_string()));
        let report = build_first_useful_action_report(input);
        let md = render_first_useful_action_markdown(&report);
        assert!(
            md.contains("## Fallback"),
            "expected Fallback section in markdown: {md}"
        );
        Ok(())
    }

    #[test]
    fn markdown_no_fallback_for_actionable_status() -> Result<(), String> {
        // actionable report has no fallback – markdown should not show Fallback section
        let proof_json = r#"{
            "seam": {"seam_id": "seam-a", "seam_kind": "predicate_boundary", "grip_class": "weakly_gripped"},
            "recommendation": {}
        }"#;
        let pr_guidance_json = r#"{"comments":[{"seam_id":"seam-a"}]}"#;
        let mut input = bare_input();
        input.pr_guidance_path = Some("g.json".to_string());
        input.assistant_proof_path = Some("proof.json".to_string());
        input.pr_guidance_json = Some(Ok(pr_guidance_json.to_string()));
        input.assistant_proof_json = Some(Ok(proof_json.to_string()));
        let report = build_first_useful_action_report(input);
        let md = render_first_useful_action_markdown(&report);
        // actionable status → fallback section suppressed
        assert!(
            !(md.contains("## Fallback")),
            "should not show Fallback for actionable status: {md}"
        );
        Ok(())
    }

    #[test]
    fn markdown_limits_section_present() -> Result<(), String> {
        // All routing branches produce limits; verify they appear in markdown
        let report = build_first_useful_action_report(bare_input());
        let md = render_first_useful_action_markdown(&report);
        assert!(
            md.contains("## Limits"),
            "expected Limits section in markdown: {md}"
        );
        Ok(())
    }

    // ── with_period / trim_period / str_or ───────────────────────────────────

    #[test]
    fn with_period_appends_when_missing() -> Result<(), String> {
        let result = with_period("hello");
        assert!((result == "hello."), "expected 'hello.' but got '{result}'");
        Ok(())
    }

    #[test]
    fn with_period_does_not_double_period() -> Result<(), String> {
        let result = with_period("done.");
        assert!((result == "done."), "expected 'done.' but got '{result}'");
        Ok(())
    }

    #[test]
    fn trim_period_removes_trailing_dot() -> Result<(), String> {
        let result = trim_period("word.");
        assert!((result == "word"), "expected 'word' but got '{result}'");
        Ok(())
    }

    #[test]
    fn trim_period_leaves_no_dot_unchanged() -> Result<(), String> {
        let result = trim_period("word");
        assert!((result == "word"), "expected 'word' but got '{result}'");
        Ok(())
    }

    #[test]
    fn str_or_returns_value_when_some() -> Result<(), String> {
        let result = str_or(Some("actual"), "fallback");
        assert!((result == "actual"), "expected 'actual' but got '{result}'");
        Ok(())
    }

    #[test]
    fn str_or_returns_fallback_when_none() -> Result<(), String> {
        let result = str_or(None, "fallback");
        assert!(
            (result == "fallback"),
            "expected 'fallback' but got '{result}'"
        );
        Ok(())
    }

    // ── normalize_suggested_assertion ────────────────────────────────────────

    #[test]
    fn normalize_suggested_assertion_reformats_add_focused_test_pattern() -> Result<(), String> {
        let input = "Add a focused test where x > 0 and assert the exact output is 1.";
        let result = normalize_suggested_assertion(input);
        assert!(
            result.starts_with("Assert the exact"),
            "expected reformatted assertion but got: {result}"
        );
        assert!(
            !(result.contains("Add a focused test where")),
            "should not contain original prefix after normalization: {result}"
        );
        Ok(())
    }

    #[test]
    fn normalize_suggested_assertion_passes_through_unmatched() -> Result<(), String> {
        let input = "assert_eq!(f(x), 42)";
        let result = normalize_suggested_assertion(input);
        assert!((result == input), "expected pass-through but got: {result}");
        Ok(())
    }

    // ── classification_from_sources alias mapping ────────────────────────────

    #[test]
    fn classification_from_sources_maps_weakly_gripped() -> Result<(), String> {
        let value: Value = serde_json::from_str(r#"{"grip_class": "weakly_gripped"}"#)
            .map_err(|e| e.to_string())?;
        let result = classification_from_sources(&[(Some(&value), &["grip_class"])]);
        assert!(
            (result.as_deref() == Some("weakly_exposed")),
            "expected weakly_exposed but got: {result:?}"
        );
        Ok(())
    }

    #[test]
    fn classification_from_sources_maps_strongly_gripped() -> Result<(), String> {
        let value: Value = serde_json::from_str(r#"{"grip_class": "strongly_gripped"}"#)
            .map_err(|e| e.to_string())?;
        let result = classification_from_sources(&[(Some(&value), &["grip_class"])]);
        assert!(
            (result.as_deref() == Some("exposed")),
            "expected exposed but got: {result:?}"
        );
        Ok(())
    }

    #[test]
    fn classification_from_sources_passes_through_other() -> Result<(), String> {
        let value: Value = serde_json::from_str(r#"{"classification": "static_unknown"}"#)
            .map_err(|e| e.to_string())?;
        let result = classification_from_sources(&[(Some(&value), &["classification"])]);
        assert!(
            (result.as_deref() == Some("static_unknown")),
            "expected static_unknown but got: {result:?}"
        );
        Ok(())
    }

    // ── path_value numeric index ─────────────────────────────────────────────

    #[test]
    fn path_value_resolves_numeric_array_index() -> Result<(), String> {
        let value: Value =
            serde_json::from_str(r#"{"items": ["a", "b", "c"]}"#).map_err(|e| e.to_string())?;
        let result = path_value(&value, &["items", "1"]);
        assert!(
            (result.and_then(Value::as_str) == Some("b")),
            "expected 'b' at index 1 but got: {result:?}"
        );
        Ok(())
    }

    #[test]
    fn path_value_returns_none_for_missing_key() -> Result<(), String> {
        let value: Value = serde_json::from_str(r#"{"a": 1}"#).map_err(|e| e.to_string())?;
        let result = path_value(&value, &["b", "c"]);
        assert!(result.is_none(), "expected None for missing key");
        Ok(())
    }

    // ── value_as_string numeric coercion ─────────────────────────────────────

    #[test]
    fn value_as_string_coerces_i64() -> Result<(), String> {
        let value: Value = serde_json::from_str(r#"-42"#).map_err(|e| e.to_string())?;
        let result = value_as_string(&value);
        assert!(
            (result.as_deref() == Some("-42")),
            "expected '-42' but got: {result:?}"
        );
        Ok(())
    }

    #[test]
    fn value_as_string_coerces_u64() -> Result<(), String> {
        let value: Value =
            serde_json::from_str(r#"18446744073709551615"#).map_err(|e| e.to_string())?;
        let result = value_as_string(&value);
        assert!(result.is_some(), "expected Some for large u64");
        Ok(())
    }

    #[test]
    fn value_as_string_returns_none_for_object() -> Result<(), String> {
        let value: Value = serde_json::from_str(r#"{"k": 1}"#).map_err(|e| e.to_string())?;
        let result = value_as_string(&value);
        assert!(result.is_none(), "expected None for object value");
        Ok(())
    }

    // ── bool_path ────────────────────────────────────────────────────────────

    #[test]
    fn bool_path_extracts_true_value() -> Result<(), String> {
        let value: Value = serde_json::from_str(r#"{"stale": true}"#).map_err(|e| e.to_string())?;
        let result = bool_path(&value, &["stale"]);
        assert!(
            (result == Some(true)),
            "expected Some(true) but got: {result:?}"
        );
        Ok(())
    }

    #[test]
    fn bool_path_extracts_false_value() -> Result<(), String> {
        let value: Value =
            serde_json::from_str(r#"{"stale": false}"#).map_err(|e| e.to_string())?;
        let result = bool_path(&value, &["stale"]);
        assert!(
            (result == Some(false)),
            "expected Some(false) but got: {result:?}"
        );
        Ok(())
    }

    #[test]
    fn bool_path_returns_none_for_missing_key() -> Result<(), String> {
        let value: Value = serde_json::from_str(r#"{}"#).map_err(|e| e.to_string())?;
        let result = bool_path(&value, &["stale"]);
        assert!(result.is_none(), "expected None for missing key");
        Ok(())
    }

    // ── has_any_input / has_any_parsed ───────────────────────────────────────

    #[test]
    fn has_any_input_false_when_all_none() -> Result<(), String> {
        let input = bare_input();
        assert!(
            !(has_any_input(&input)),
            "expected has_any_input to be false with all Nones"
        );
        Ok(())
    }

    #[test]
    fn has_any_input_true_when_one_path_set() -> Result<(), String> {
        let mut input = bare_input();
        input.coverage_frontier_path = Some("frontier.json".to_string());
        assert!(has_any_input(&input), "expected has_any_input to be true");
        Ok(())
    }

    #[test]
    fn has_any_parsed_false_when_all_none() -> Result<(), String> {
        let parsed = ParsedSources::default();
        assert!(
            !(has_any_parsed(&parsed)),
            "expected has_any_parsed to be false"
        );
        Ok(())
    }

    #[test]
    fn has_any_parsed_true_when_coverage_frontier_set() -> Result<(), String> {
        let parsed = ParsedSources {
            coverage_frontier: Some(serde_json::Value::Bool(true)),
            ..ParsedSources::default()
        };
        assert!(
            has_any_parsed(&parsed),
            "expected has_any_parsed to be true"
        );
        Ok(())
    }

    // ── first_guidance_item / first_summary_only_item / first_suppressed_item ─

    #[test]
    fn first_guidance_item_returns_first_comment() -> Result<(), String> {
        let guidance: Value =
            serde_json::from_str(r#"{"comments":[{"seam_id":"s1"},{"seam_id":"s2"}]}"#)
                .map_err(|e| e.to_string())?;
        let item = first_guidance_item(Some(&guidance));
        let seam_id = item.and_then(|v| v.get("seam_id")).and_then(|v| v.as_str());
        assert!(
            (seam_id == Some("s1")),
            "expected 's1' but got: {seam_id:?}"
        );
        Ok(())
    }

    #[test]
    fn first_guidance_item_returns_none_when_empty() -> Result<(), String> {
        let guidance: Value =
            serde_json::from_str(r#"{"comments":[]}"#).map_err(|e| e.to_string())?;
        assert!(
            first_guidance_item(Some(&guidance)).is_none(),
            "expected None for empty comments"
        );
        Ok(())
    }

    #[test]
    fn first_summary_only_item_returns_first() -> Result<(), String> {
        let guidance: Value = serde_json::from_str(r#"{"summary_only":[{"seam_id":"so1"}]}"#)
            .map_err(|e| e.to_string())?;
        let item = first_summary_only_item(Some(&guidance));
        assert!(item.is_some(), "expected Some for summary_only item");
        Ok(())
    }

    #[test]
    fn first_suppressed_item_returns_first() -> Result<(), String> {
        let guidance: Value = serde_json::from_str(r#"{"suppressed":[{"seam_id":"sup1"}]}"#)
            .map_err(|e| e.to_string())?;
        let item = first_suppressed_item(Some(&guidance));
        assert!(item.is_some(), "expected Some for suppressed item");
        Ok(())
    }

    // ── has_actionable_guidance ──────────────────────────────────────────────

    #[test]
    fn has_actionable_guidance_true_for_comments() -> Result<(), String> {
        let guidance: Value = serde_json::from_str(r#"{"comments":[{"seam_id":"s1"}]}"#)
            .map_err(|e| e.to_string())?;
        assert!(
            has_actionable_guidance(Some(&guidance)),
            "expected actionable guidance for comments"
        );
        Ok(())
    }

    #[test]
    fn has_actionable_guidance_true_for_summary_only() -> Result<(), String> {
        let guidance: Value = serde_json::from_str(r#"{"summary_only":[{"seam_id":"s2"}]}"#)
            .map_err(|e| e.to_string())?;
        assert!(
            has_actionable_guidance(Some(&guidance)),
            "expected actionable guidance for summary_only"
        );
        Ok(())
    }

    #[test]
    fn has_actionable_guidance_false_when_none() -> Result<(), String> {
        assert!(
            !(has_actionable_guidance(None)),
            "expected false when guidance is None"
        );
        Ok(())
    }

    // ── has_suppressed_guidance ──────────────────────────────────────────────

    #[test]
    fn has_suppressed_guidance_false_when_none() -> Result<(), String> {
        assert!(
            !(has_suppressed_guidance(None)),
            "expected false when guidance is None"
        );
        Ok(())
    }

    #[test]
    fn has_suppressed_guidance_true_for_suppressed_array() -> Result<(), String> {
        let guidance: Value = serde_json::from_str(r#"{"suppressed":[{"seam_id":"s"}]}"#)
            .map_err(|e| e.to_string())?;
        assert!(
            has_suppressed_guidance(Some(&guidance)),
            "expected suppressed guidance"
        );
        Ok(())
    }

    // ── first_item_with_bucket ───────────────────────────────────────────────

    #[test]
    fn first_item_with_bucket_finds_matching_bucket() -> Result<(), String> {
        let report: Value = serde_json::from_str(
            r#"{
            "items": [
                {"bucket": "resolved", "path": "a.rs"},
                {"bucket": "still_present", "path": "b.rs"}
            ]
        }"#,
        )
        .map_err(|e| e.to_string())?;
        let item = first_item_with_bucket(&report, &["still_present"]);
        let path = item.and_then(|v| v.get("path")).and_then(|v| v.as_str());
        assert!((path == Some("b.rs")), "expected b.rs but got: {path:?}");
        Ok(())
    }

    #[test]
    fn first_item_with_bucket_returns_none_when_no_match() -> Result<(), String> {
        let report: Value = serde_json::from_str(r#"{"items": [{"bucket": "resolved"}]}"#)
            .map_err(|e| e.to_string())?;
        assert!(
            first_item_with_bucket(&report, &["still_present"]).is_none(),
            "expected None when no bucket matches"
        );
        Ok(())
    }

    #[test]
    fn first_item_with_bucket_returns_none_when_no_items() -> Result<(), String> {
        let report: Value = serde_json::from_str(r#"{}"#).map_err(|e| e.to_string())?;
        assert!(
            first_item_with_bucket(&report, &["still_present"]).is_none(),
            "expected None when no items key"
        );
        Ok(())
    }

    // ── gate_has_waiver variants ─────────────────────────────────────────────

    #[test]
    fn gate_has_waiver_detects_decision_waived() -> Result<(), String> {
        let gate: Value =
            serde_json::from_str(r#"{"decision": "waived"}"#).map_err(|e| e.to_string())?;
        assert!(
            gate_has_waiver(&gate),
            "expected waiver for decision=waived"
        );
        Ok(())
    }

    #[test]
    fn gate_has_waiver_false_for_empty_waivers_array() -> Result<(), String> {
        let gate: Value = serde_json::from_str(r#"{"waivers": []}"#).map_err(|e| e.to_string())?;
        assert!(
            !(gate_has_waiver(&gate)),
            "expected no waiver for empty waivers array"
        );
        Ok(())
    }

    // ── first_gate_* helpers ─────────────────────────────────────────────────

    #[test]
    fn first_gate_seam_from_top_level() -> Result<(), String> {
        let gate: Value =
            serde_json::from_str(r#"{"seam_id": "top-seam"}"#).map_err(|e| e.to_string())?;
        let result = first_gate_seam(&gate);
        assert!(
            (result.as_deref() == Some("top-seam")),
            "expected top-seam but got: {result:?}"
        );
        Ok(())
    }

    #[test]
    fn first_gate_seam_from_items_array() -> Result<(), String> {
        let gate: Value = serde_json::from_str(r#"{"items": [{"seam_id": "item-seam"}]}"#)
            .map_err(|e| e.to_string())?;
        let result = first_gate_seam(&gate);
        assert!(
            (result.as_deref() == Some("item-seam")),
            "expected item-seam but got: {result:?}"
        );
        Ok(())
    }

    #[test]
    fn first_gate_path_from_top_level() -> Result<(), String> {
        let gate: Value =
            serde_json::from_str(r#"{"path": "src/gate.rs"}"#).map_err(|e| e.to_string())?;
        let result = first_gate_path(&gate);
        assert!(
            (result.as_deref() == Some("src/gate.rs")),
            "expected src/gate.rs but got: {result:?}"
        );
        Ok(())
    }

    #[test]
    fn first_gate_line_from_top_level() -> Result<(), String> {
        let gate: Value = serde_json::from_str(r#"{"line": 42}"#).map_err(|e| e.to_string())?;
        let result = first_gate_line(&gate);
        assert!(
            (result == Some(42)),
            "expected Some(42) but got: {result:?}"
        );
        Ok(())
    }

    #[test]
    fn first_gate_missing_discriminator_from_top_level() -> Result<(), String> {
        let gate: Value = serde_json::from_str(r#"{"missing_discriminator": "assert x == 1"}"#)
            .map_err(|e| e.to_string())?;
        let result = first_gate_missing_discriminator(&gate);
        assert!(
            (result.as_deref() == Some("assert x == 1")),
            "expected 'assert x == 1' but got: {result:?}"
        );
        Ok(())
    }

    // ── receipt_command / selected_seam_id ───────────────────────────────────

    #[test]
    fn receipt_command_with_seam_id_from_receipt_provenance() -> Result<(), String> {
        let receipt_json = r#"{"provenance": {"movement": "improved", "seam_id": "seam-rc"}}"#;
        let mut input = bare_input();
        input.receipt_path = Some("receipt.json".to_string());
        input.receipt_json = Some(Ok(receipt_json.to_string()));
        let report = build_first_useful_action_report(input);
        let rendered = render_first_useful_action_json(&report)?;
        // already_improved route → receipt command uses seam_id
        assert!(
            rendered.contains("seam-rc"),
            "expected seam-rc in rendered: {rendered}"
        );
        Ok(())
    }

    #[test]
    fn selected_seam_id_from_ledger_top_repair_route() -> Result<(), String> {
        let ledger_json =
            r#"{"movement": {"acknowledged": 1}, "top_repair_route": {"seam_id": "seam-ledger"}}"#;
        let mut input = bare_input();
        input.ledger_path = Some("ledger.json".to_string());
        input.ledger_json = Some(Ok(ledger_json.to_string()));
        let report = build_first_useful_action_report(input);
        let rendered = render_first_useful_action_json(&report)?;
        assert!(
            rendered.contains("seam-ledger"),
            "expected seam-ledger in rendered: {rendered}"
        );
        Ok(())
    }

    // ── gap_records: "records" key shape ─────────────────────────────────────

    #[test]
    fn gap_records_from_records_key() -> Result<(), String> {
        let value: Value = serde_json::from_str(r#"{"records": [{"gap_id": "g1"}]}"#)
            .map_err(|e| e.to_string())?;
        let records = gap_records(&value);
        assert!(
            (records.len() == 1),
            "expected 1 record but got {}",
            records.len()
        );
        Ok(())
    }

    #[test]
    fn gap_records_from_cases_key() -> Result<(), String> {
        let value: Value = serde_json::from_str(
            r#"{"cases": [{"expected_gap_record": {"gap_id": "g2"}}, {"no_gap": true}]}"#,
        )
        .map_err(|e| e.to_string())?;
        let records = gap_records(&value);
        // Only the case with expected_gap_record is included
        assert!(
            (records.len() == 1),
            "expected 1 record from cases but got {}",
            records.len()
        );
        Ok(())
    }

    #[test]
    fn gap_records_from_empty_object_returns_empty() -> Result<(), String> {
        let value: Value = serde_json::from_str(r#"{}"#).map_err(|e| e.to_string())?;
        let records = gap_records(&value);
        assert!(
            records.is_empty(),
            "expected empty records but got {}",
            records.len()
        );
        Ok(())
    }

    // ── first_actionable_gap_record: reintroduced policy_state ───────────────

    #[test]
    fn first_actionable_gap_record_accepts_reintroduced_policy_state() -> Result<(), String> {
        let record_json = r#"[{
            "gap_id": "g-r",
            "kind": "MissingAssertion",
            "language": "rust",
            "language_status": "stable",
            "scope": "pr_local",
            "gap_state": "actionable",
            "policy_state": "reintroduced",
            "repairability": "repairable",
            "repair_route": {"route_kind": "AddBoundaryAssertion"},
            "verification_commands": ["cargo test"]
        }]"#;
        let value: Value = serde_json::from_str(record_json).map_err(|e| e.to_string())?;
        assert!(
            first_actionable_gap_record(&value).is_some(),
            "expected reintroduced policy_state to be accepted"
        );
        Ok(())
    }

    #[test]
    fn first_actionable_gap_record_rejects_missing_repair_route() -> Result<(), String> {
        let record_json = r#"[{
            "gap_id": "g-no-route",
            "language": "rust",
            "language_status": "stable",
            "scope": "pr_local",
            "gap_state": "actionable",
            "policy_state": "new",
            "repairability": "repairable",
            "verification_commands": ["cargo test"]
        }]"#;
        let value: Value = serde_json::from_str(record_json).map_err(|e| e.to_string())?;
        assert!(
            first_actionable_gap_record(&value).is_none(),
            "expected None when repair_route is absent"
        );
        Ok(())
    }

    #[test]
    fn first_actionable_gap_record_rejects_empty_verification_commands() -> Result<(), String> {
        let record_json = r#"[{
            "gap_id": "g-empty-cmds",
            "language": "rust",
            "language_status": "stable",
            "scope": "pr_local",
            "gap_state": "actionable",
            "policy_state": "new",
            "repairability": "repairable",
            "repair_route": {"route_kind": "AddAssertion"},
            "verification_commands": ["   "]
        }]"#;
        let value: Value = serde_json::from_str(record_json).map_err(|e| e.to_string())?;
        assert!(
            first_actionable_gap_record(&value).is_none(),
            "expected None for whitespace-only verification commands"
        );
        Ok(())
    }

    // ── action_kind_for_gap_route ─────────────────────────────────────────────

    #[test]
    fn action_kind_for_add_output_golden() -> Result<(), String> {
        let result = action_kind_for_gap_route("AddOutputGolden");
        assert!(
            (result == "generate_missing_artifact"),
            "expected generate_missing_artifact but got: {result}"
        );
        Ok(())
    }

    #[test]
    fn action_kind_for_regenerate_artifact() -> Result<(), String> {
        let result = action_kind_for_gap_route("RegenerateArtifact");
        assert!(
            (result == "generate_missing_artifact"),
            "expected generate_missing_artifact but got: {result}"
        );
        Ok(())
    }

    #[test]
    fn action_kind_for_other_route() -> Result<(), String> {
        let result = action_kind_for_gap_route("AddBoundaryAssertion");
        assert!(
            (result == "write_focused_test"),
            "expected write_focused_test but got: {result}"
        );
        Ok(())
    }

    // ── target_from_gap_record filter ────────────────────────────────────────

    #[test]
    fn target_from_gap_record_returns_none_when_no_useful_fields() -> Result<(), String> {
        // repair_route present but none of file/related_test/assertion_shape set
        let record: Value =
            serde_json::from_str(r#"{"repair_route": {"route_kind": "AddAssertion"}}"#)
                .map_err(|e| e.to_string())?;
        let result = target_from_gap_record(&record);
        assert!(result.is_none(), "expected None when no target fields");
        Ok(())
    }

    #[test]
    fn target_from_gap_record_returns_some_when_file_set() -> Result<(), String> {
        let record: Value = serde_json::from_str(
            r#"{"repair_route": {"route_kind": "AddAssertion", "target_file": "tests/foo.rs"}}"#,
        )
        .map_err(|e| e.to_string())?;
        let result = target_from_gap_record(&record);
        assert!(result.is_some(), "expected Some when target_file is set");
        Ok(())
    }

    #[test]
    fn target_from_gap_record_none_when_no_repair_route() -> Result<(), String> {
        let record: Value =
            serde_json::from_str(r#"{"gap_id": "g1"}"#).map_err(|e| e.to_string())?;
        let result = target_from_gap_record(&record);
        assert!(result.is_none(), "expected None when repair_route missing");
        Ok(())
    }

    // ── push_wrapped_paragraph ───────────────────────────────────────────────

    #[test]
    fn push_wrapped_paragraph_formats_text() -> Result<(), String> {
        let mut out = String::new();
        push_wrapped_paragraph(&mut out, "short text");
        assert!(
            out.contains("short text"),
            "expected 'short text' in output: {out}"
        );
        assert!(out.ends_with('\n'), "expected trailing newline");
        Ok(())
    }

    #[test]
    fn push_wrapped_paragraph_wraps_long_text() -> Result<(), String> {
        let long_text = "word ".repeat(20).trim().to_string();
        let mut out = String::new();
        push_wrapped_paragraph(&mut out, &long_text);
        let lines: Vec<&str> = out.lines().collect();
        assert!(
            (lines.len() >= 2),
            "expected wrapped text to have multiple lines but got: {out}"
        );
        Ok(())
    }

    // ── selected_from_receipt_or_sources seam_id fallback ────────────────────

    #[test]
    fn selected_from_receipt_uses_proof_seam_id_fallback() -> Result<(), String> {
        // receipt has no seam_id, but assistant_proof has seam.seam_id
        let receipt_json = r#"{"provenance": {"movement": "improved"}}"#;
        let proof_json = r#"{"seam": {"seam_id": "proof-seam-id"}}"#;
        let pr_guidance_json = r#"{"comments":[{"seam_id":"g1"}]}"#;
        let mut input = bare_input();
        input.receipt_path = Some("receipt.json".to_string());
        input.receipt_json = Some(Ok(receipt_json.to_string()));
        input.assistant_proof_path = Some("proof.json".to_string());
        input.assistant_proof_json = Some(Ok(proof_json.to_string()));
        input.pr_guidance_path = Some("g.json".to_string());
        input.pr_guidance_json = Some(Ok(pr_guidance_json.to_string()));
        let report = build_first_useful_action_report(input);
        let rendered = render_first_useful_action_json(&report)?;
        assert!(
            rendered.contains("proof-seam-id"),
            "expected proof-seam-id in receipt selected: {rendered}"
        );
        Ok(())
    }

    // ── selected_from_guidance: summary_only fallback ────────────────────────

    #[test]
    fn selected_from_guidance_uses_summary_only_item() -> Result<(), String> {
        // suppressed items are absent but summary_only is present, and suppressed string in warnings
        // forces suppressed route → guidance selected from summary_only fallback
        let pr_guidance_json = r#"{
            "summary_only": [{"seam_id": "so-seam", "kind": "predicate_boundary"}],
            "warnings": ["seam configured off by policy"]
        }"#;
        let mut input = bare_input();
        input.pr_guidance_path = Some("g.json".to_string());
        input.pr_guidance_json = Some(Ok(pr_guidance_json.to_string()));
        let report = build_first_useful_action_report(input);
        let rendered = render_first_useful_action_json(&report)?;
        // suppressed route triggered by warning text
        assert!(
            rendered.contains(r#""status": "suppressed""#),
            "expected suppressed but got: {rendered}"
        );
        Ok(())
    }

    // ── selected_acknowledged: ledger fallback seam_id ───────────────────────

    #[test]
    fn selected_acknowledged_uses_fallback_seam_id_when_no_top_repair_route() -> Result<(), String>
    {
        let ledger_json = r#"{"movement": {"acknowledged": 2}}"#;
        let mut input = bare_input();
        input.ledger_path = Some("ledger.json".to_string());
        input.ledger_json = Some(Ok(ledger_json.to_string()));
        let report = build_first_useful_action_report(input);
        let rendered = render_first_useful_action_json(&report)?;
        assert!(
            rendered.contains("acknowledged-boundary-0001"),
            "expected fallback seam_id in acknowledged: {rendered}"
        );
        Ok(())
    }

    // ── selected_waived: fallback seam_id when no seam_id present ────────────

    #[test]
    fn selected_waived_uses_fallback_seam_id_when_no_seam_id() -> Result<(), String> {
        let gate_json = r#"{"waivers": [{"id": "w1"}]}"#;
        let mut input = bare_input();
        input.gate_decision_path = Some("gate.json".to_string());
        input.gate_decision_json = Some(Ok(gate_json.to_string()));
        let report = build_first_useful_action_report(input);
        let rendered = render_first_useful_action_json(&report)?;
        assert!(
            rendered.contains("waived-boundary-0001"),
            "expected fallback seam_id for waived: {rendered}"
        );
        Ok(())
    }

    // ── display_path round-trip ───────────────────────────────────────────────

    #[test]
    fn display_path_converts_backslashes() -> Result<(), String> {
        let path = Path::new("some\\windows\\path.json");
        let result = display_path(path);
        assert!(
            !(result.contains('\\')),
            "expected forward slashes but got: {result}"
        );
        assert!(
            result.contains("some/windows/path.json"),
            "unexpected path: {result}"
        );
        Ok(())
    }

    // ── render_first_useful_action_json: verify schema fields present ─────────

    #[test]
    fn render_first_useful_action_json_includes_schema_version() -> Result<(), String> {
        let report = build_first_useful_action_report(bare_input());
        let rendered = render_first_useful_action_json(&report)?;
        assert!(
            rendered.contains(r#""schema_version": "0.1""#),
            "expected schema_version in JSON output: {rendered}"
        );
        assert!(
            rendered.contains(r#""kind": "first_useful_action""#),
            "expected kind field in JSON output: {rendered}"
        );
        Ok(())
    }

    // ── coverage_frontier and editor_context parsed but not actionable ────────

    #[test]
    fn coverage_frontier_does_not_block_no_actionable_routing() -> Result<(), String> {
        let mut input = bare_input();
        input.coverage_frontier_path = Some("frontier.json".to_string());
        input.coverage_frontier_json = Some(Ok(r#"{"kind": "coverage_frontier"}"#.to_string()));
        let report = build_first_useful_action_report(input);
        let rendered = render_first_useful_action_json(&report)?;
        // coverage frontier alone doesn't trigger actionable routing
        assert!(
            rendered.contains(r#""status": "no_actionable_seam""#),
            "expected no_actionable_seam with only frontier: {rendered}"
        );
        Ok(())
    }

    // ── markdown receipt section ─────────────────────────────────────────────

    #[test]
    fn markdown_verify_section_present_for_actionable() -> Result<(), String> {
        // The actionable report emits seam_commands which include a verify command
        let proof_json = r#"{
            "seam": {
                "seam_id": "seam-verify",
                "seam_kind": "predicate_boundary",
                "grip_class": "weakly_gripped"
            },
            "recommendation": {}
        }"#;
        let pr_guidance_json = r#"{"comments":[{"seam_id":"seam-verify"}]}"#;
        let mut input = bare_input();
        input.pr_guidance_path = Some("g.json".to_string());
        input.assistant_proof_path = Some("proof.json".to_string());
        input.pr_guidance_json = Some(Ok(pr_guidance_json.to_string()));
        input.assistant_proof_json = Some(Ok(proof_json.to_string()));
        let report = build_first_useful_action_report(input);
        let md = render_first_useful_action_markdown(&report);
        assert!(
            md.contains("## Verify"),
            "expected Verify section in actionable markdown: {md}"
        );
        Ok(())
    }
}
