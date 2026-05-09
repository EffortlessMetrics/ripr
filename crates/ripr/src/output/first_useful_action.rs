use serde::Serialize;
use serde_json::Value;
use std::path::Path;

const SCHEMA_VERSION: &str = "0.1";
const REPORT_KIND: &str = "first_useful_action";
const DEFAULT_ASSISTANT_PROOF_COMMAND: &str = "ripr assistant-loop proof --pr-guidance target/ripr/review/comments.json --agent-packet target/ripr/workflow/agent-brief.json --before target/ripr/workflow/before.repo-exposure.json --after target/ripr/workflow/after.repo-exposure.json --receipt target/ripr/reports/agent-receipt.json --ledger target/ripr/reports/pr-evidence-ledger.json --out target/ripr/reports/test-oracle-assistant-proof.json --out-md target/ripr/reports/test-oracle-assistant-proof.md";
pub(crate) const DEFAULT_FIRST_USEFUL_ACTION_OUT: &str =
    "target/ripr/reports/first-useful-action.json";
pub(crate) const DEFAULT_FIRST_USEFUL_ACTION_MD_OUT: &str =
    "target/ripr/reports/first-useful-action.md";

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

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub(crate) struct FirstUsefulActionReport {
    schema_version: &'static str,
    tool: &'static str,
    kind: &'static str,
    status: String,
    audience: String,
    action_kind: String,
    root: String,
    generated_at: String,
    inputs: FirstActionInputs,
    selected: Option<SelectedAction>,
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
struct FirstActionInputs {
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
struct SelectedAction {
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

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
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

#[derive(Clone, Debug, Default, Eq, PartialEq)]
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
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum ActionRoute {
    Actionable,
    Stale,
    MissingAssistantProof(String),
    BaselineOnly,
    Acknowledged,
    Waived,
    Suppressed,
    NoActionableSeam,
    AlreadyImproved,
    UnchangedAfterAttempt,
}

pub(crate) fn build_first_useful_action_report(
    input: FirstUsefulActionInput,
) -> FirstUsefulActionReport {
    let parsed = parse_sources(&input);
    let route = select_route(&input, &parsed);
    let mut warnings = parsed.warnings.clone();
    if let ActionRoute::MissingAssistantProof(path) = &route {
        warnings.push(format!("missing required artifact: {path}"));
    }

    let selected = selected_for_route(&route, &input, &parsed);
    let target = target_for_route(&route, &parsed);
    let commands = commands_for_route(&route, &input, &parsed, selected.as_ref());
    let evidence = evidence_for_route(&input, &parsed);
    let inputs = inputs_for_report(&input, &parsed);
    let fallback = fallback_for_route(&route);
    let route_text = route_text(&route);

    FirstUsefulActionReport {
        schema_version: SCHEMA_VERSION,
        tool: "ripr",
        kind: REPORT_KIND,
        status: route_text.status.to_string(),
        audience: route_text.audience.to_string(),
        action_kind: route_text.action_kind.to_string(),
        root: input.root,
        generated_at: input.generated_at,
        inputs,
        selected,
        title: route_text.title.to_string(),
        why: route_text.why.to_string(),
        why_first: route_text
            .why_first
            .iter()
            .map(|reason| (*reason).to_string())
            .collect(),
        target,
        commands,
        evidence,
        fallback,
        warnings,
        limits: route_text
            .limits
            .iter()
            .map(|limit| (*limit).to_string())
            .collect(),
    }
}

pub(crate) fn render_first_useful_action_json(
    report: &FirstUsefulActionReport,
) -> Result<String, String> {
    serde_json::to_string_pretty(report)
        .map_err(|err| format!("render first useful action JSON failed: {err}"))
}

pub(crate) fn render_first_useful_action_markdown(report: &FirstUsefulActionReport) -> String {
    let mut out = String::new();
    out.push_str("# RIPR First Useful Action\n\n");
    out.push_str(&format!("Status: {}\n", report.status));
    out.push_str(&format!("Audience: {}\n", report.audience));
    out.push_str(&format!("Action: {}\n\n", report.action_kind));
    out.push_str("## Next\n\n");
    out.push_str(&report.title);
    out.push_str(".\n\n");
    out.push_str("## Why First\n\n");
    for reason in &report.why_first {
        out.push_str(&format!("- {}\n", wrap_markdown_reason(reason)));
    }
    if let Some(fallback) = &report.fallback {
        out.push_str("\n## Fallback\n\n");
        out.push_str(&format!("- Kind: {}\n", fallback.kind));
        if let Some(summary) = &fallback.summary {
            out.push_str(&format!("- Summary: {summary}\n"));
        }
        if let Some(missing) = &fallback.missing {
            out.push_str(&format!("- Missing: {missing}\n"));
        }
    }
    if let Some(target) = &report.target {
        out.push_str("\n## Where\n\n");
        out.push_str(&format!(
            "- File: `{}`\n",
            target.file.as_deref().unwrap_or("unknown")
        ));
        out.push_str(&format!(
            "- Related test: `{}`\n",
            target.related_test.as_deref().unwrap_or("unknown")
        ));
        out.push_str(&format!(
            "- Suggested test: `{}`\n",
            target.suggested_test_name.as_deref().unwrap_or("unknown")
        ));
    }
    if let Some(verify) = &report.commands.verify {
        out.push_str("\n## Verify\n\n");
        out.push_str(&format!("`{verify}`\n"));
    }
    if let Some(receipt) = &report.commands.receipt {
        out.push_str("\n## Receipt\n\n");
        out.push_str(&format!("`{receipt}`\n"));
    }
    if !report.warnings.is_empty() {
        out.push_str("\n## Warnings\n\n");
        for warning in &report.warnings {
            out.push_str(&format!("- {warning}\n"));
        }
    }
    out.push_str("\n## Limits\n\n");
    for limit in &report.limits {
        out.push_str(&format!("- {limit}\n"));
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
        &mut parsed.warnings,
        true,
    );
    parsed.assistant_proof = parse_optional_json(
        "assistant proof",
        input.assistant_proof_path.as_deref(),
        &input.assistant_proof_json,
        &mut parsed.warnings,
        false,
    );
    parsed.ledger = parse_optional_json(
        "PR evidence ledger",
        input.ledger_path.as_deref(),
        &input.ledger_json,
        &mut parsed.warnings,
        true,
    );
    parsed.baseline_delta = parse_optional_json(
        "baseline debt delta",
        input.baseline_delta_path.as_deref(),
        &input.baseline_delta_json,
        &mut parsed.warnings,
        true,
    );
    parsed.receipt = parse_optional_json(
        "receipt",
        input.receipt_path.as_deref(),
        &input.receipt_json,
        &mut parsed.warnings,
        true,
    );
    parsed.gate_decision = parse_optional_json(
        "gate decision",
        input.gate_decision_path.as_deref(),
        &input.gate_decision_json,
        &mut parsed.warnings,
        true,
    );
    parsed.coverage_frontier = parse_optional_json(
        "coverage/grip frontier",
        input.coverage_frontier_path.as_deref(),
        &input.coverage_frontier_json,
        &mut parsed.warnings,
        true,
    );
    parsed.editor_context = parse_optional_json(
        "editor context",
        input.editor_context_path.as_deref(),
        &input.editor_context_json,
        &mut parsed.warnings,
        true,
    );
    parsed
}

fn parse_optional_json(
    label: &str,
    path: Option<&str>,
    text: &Option<Result<String, String>>,
    warnings: &mut Vec<String>,
    warn_on_error: bool,
) -> Option<Value> {
    let path = path?;
    let Some(text) = text else {
        if warn_on_error {
            warnings.push(format!(
                "{label} path {path} was supplied but no input text was loaded"
            ));
        }
        return None;
    };
    let text = match text {
        Ok(text) => text,
        Err(error) => {
            if warn_on_error {
                warnings.push(format!("optional {label} input {path} is invalid: {error}"));
            }
            return None;
        }
    };
    match serde_json::from_str::<Value>(text) {
        Ok(value) => Some(value),
        Err(error) => {
            if warn_on_error {
                warnings.push(format!("optional {label} input {path} is invalid: {error}"));
            }
            None
        }
    }
}

fn select_route(input: &FirstUsefulActionInput, parsed: &ParsedSources) -> ActionRoute {
    if evidence_is_stale(parsed.editor_context.as_ref()) {
        return ActionRoute::Stale;
    }
    if input.assistant_proof_path.is_some()
        && parsed.assistant_proof.is_none()
        && assistant_proof_load_failed(input)
    {
        return ActionRoute::MissingAssistantProof(
            input
                .assistant_proof_path
                .clone()
                .unwrap_or_else(|| DEFAULT_FIRST_USEFUL_ACTION_OUT.to_string()),
        );
    }
    let movement = explicit_receipt_movement(parsed.receipt.as_ref());
    if matches!(movement.as_deref(), Some("improved" | "resolved")) {
        return ActionRoute::AlreadyImproved;
    }
    if matches!(movement.as_deref(), Some("unchanged" | "regressed")) {
        return ActionRoute::UnchangedAfterAttempt;
    }
    if has_suppressed_item(parsed.pr_guidance.as_ref(), parsed.baseline_delta.as_ref()) {
        return ActionRoute::Suppressed;
    }
    if has_acknowledged_item(parsed.ledger.as_ref(), parsed.gate_decision.as_ref()) {
        return ActionRoute::Acknowledged;
    }
    if has_waived_item(parsed.ledger.as_ref(), parsed.gate_decision.as_ref()) {
        return ActionRoute::Waived;
    }
    if has_actionable_pr_local(parsed.pr_guidance.as_ref(), parsed.assistant_proof.as_ref()) {
        return ActionRoute::Actionable;
    }
    if has_baseline_only(parsed.baseline_delta.as_ref(), parsed.ledger.as_ref()) {
        return ActionRoute::BaselineOnly;
    }
    ActionRoute::NoActionableSeam
}

fn assistant_proof_load_failed(input: &FirstUsefulActionInput) -> bool {
    matches!(input.assistant_proof_json.as_ref(), Some(Err(_)) | None)
        || matches!(
            input.assistant_proof_json.as_ref(),
            Some(Ok(text)) if serde_json::from_str::<Value>(text).is_err()
        )
}

fn selected_for_route(
    route: &ActionRoute,
    input: &FirstUsefulActionInput,
    parsed: &ParsedSources,
) -> Option<SelectedAction> {
    match route {
        ActionRoute::MissingAssistantProof(_) | ActionRoute::NoActionableSeam => None,
        ActionRoute::Stale => selected_from_sources(
            "editor_context",
            input.editor_context_path.as_deref(),
            parsed.editor_context.as_ref(),
            parsed,
        ),
        ActionRoute::Actionable => selected_from_sources(
            "assistant_proof",
            input.assistant_proof_path.as_deref(),
            parsed.assistant_proof.as_ref(),
            parsed,
        ),
        ActionRoute::AlreadyImproved | ActionRoute::UnchangedAfterAttempt => selected_from_sources(
            "receipt",
            input.receipt_path.as_deref(),
            parsed.receipt.as_ref(),
            parsed,
        ),
        ActionRoute::BaselineOnly => selected_from_sources(
            "baseline_delta",
            input.baseline_delta_path.as_deref(),
            parsed.baseline_delta.as_ref(),
            parsed,
        ),
        ActionRoute::Acknowledged => selected_from_sources(
            "ledger",
            input.ledger_path.as_deref(),
            parsed.ledger.as_ref(),
            parsed,
        ),
        ActionRoute::Waived => selected_from_sources(
            "gate_decision",
            input.gate_decision_path.as_deref(),
            parsed.gate_decision.as_ref(),
            parsed,
        ),
        ActionRoute::Suppressed => selected_from_sources(
            "pr_guidance",
            input.pr_guidance_path.as_deref(),
            parsed.pr_guidance.as_ref(),
            parsed,
        ),
    }
}

fn selected_from_sources(
    source: &str,
    source_artifact: Option<&str>,
    primary: Option<&Value>,
    parsed: &ParsedSources,
) -> Option<SelectedAction> {
    let primary_selected = selected_from_source(source, primary);
    let proof_selected = selected_from_proof(parsed.assistant_proof.as_ref());
    let guidance_selected = selected_from_guidance(parsed.pr_guidance.as_ref());
    let receipt_selected = selected_from_receipt(parsed.receipt.as_ref());
    let baseline_selected = selected_from_baseline_delta(parsed.baseline_delta.as_ref());
    let ledger_selected = selected_from_ledger(parsed.ledger.as_ref());
    let gate_selected = selected_from_gate(parsed.gate_decision.as_ref());
    let seam_id = string_from_selected(&[
        primary_selected.as_ref(),
        proof_selected.as_ref(),
        guidance_selected.as_ref(),
        receipt_selected.as_ref(),
        baseline_selected.as_ref(),
        ledger_selected.as_ref(),
        gate_selected.as_ref(),
    ]);
    if seam_id.is_none()
        && primary_selected.is_none()
        && proof_selected.is_none()
        && guidance_selected.is_none()
        && receipt_selected.is_none()
        && baseline_selected.is_none()
        && ledger_selected.is_none()
        && gate_selected.is_none()
    {
        return None;
    }
    Some(SelectedAction {
        source: source.to_string(),
        source_artifact: source_artifact.unwrap_or("not_available").to_string(),
        seam_id,
        seam_kind: option_from_selected(
            &[
                primary_selected.as_ref(),
                proof_selected.as_ref(),
                guidance_selected.as_ref(),
                receipt_selected.as_ref(),
                baseline_selected.as_ref(),
                ledger_selected.as_ref(),
                gate_selected.as_ref(),
            ],
            |selected| selected.seam_kind.clone(),
        ),
        path: option_from_selected(
            &[
                primary_selected.as_ref(),
                proof_selected.as_ref(),
                guidance_selected.as_ref(),
                receipt_selected.as_ref(),
                baseline_selected.as_ref(),
                ledger_selected.as_ref(),
                gate_selected.as_ref(),
            ],
            |selected| selected.path.clone(),
        ),
        line: option_from_selected(
            &[
                primary_selected.as_ref(),
                proof_selected.as_ref(),
                guidance_selected.as_ref(),
                receipt_selected.as_ref(),
                baseline_selected.as_ref(),
                ledger_selected.as_ref(),
                gate_selected.as_ref(),
            ],
            |selected| selected.line,
        ),
        classification: option_from_selected(
            &[
                primary_selected.as_ref(),
                proof_selected.as_ref(),
                guidance_selected.as_ref(),
                receipt_selected.as_ref(),
                baseline_selected.as_ref(),
                ledger_selected.as_ref(),
                gate_selected.as_ref(),
            ],
            |selected| selected.classification.clone(),
        ),
        missing_discriminator: option_from_selected(
            &[
                primary_selected.as_ref(),
                proof_selected.as_ref(),
                guidance_selected.as_ref(),
                receipt_selected.as_ref(),
                baseline_selected.as_ref(),
                ledger_selected.as_ref(),
                gate_selected.as_ref(),
            ],
            |selected| selected.missing_discriminator.clone(),
        ),
    })
}

fn selected_from_source(source: &str, value: Option<&Value>) -> Option<SelectedAction> {
    match source {
        "assistant_proof" => selected_from_proof(value),
        "pr_guidance" => selected_from_guidance(value),
        "receipt" => selected_from_receipt(value),
        "baseline_delta" => selected_from_baseline_delta(value),
        "ledger" => selected_from_ledger(value),
        "gate_decision" => selected_from_gate(value),
        "editor_context" => selected_from_editor_context(value),
        _ => None,
    }
}

fn selected_from_proof(value: Option<&Value>) -> Option<SelectedAction> {
    let value = value?;
    let seam = value.get("seam")?;
    Some(SelectedAction {
        source: "assistant_proof".to_string(),
        source_artifact: "assistant_proof".to_string(),
        seam_id: string_path(seam, &["seam_id"]),
        seam_kind: string_path(seam, &["seam_kind"]),
        path: string_path(seam, &["path"]),
        line: u64_path(seam, &["line"]),
        classification: string_path(seam, &["grip_class"]).map(static_classification),
        missing_discriminator: string_path(seam, &["missing_discriminator"]),
    })
}

fn selected_from_guidance(value: Option<&Value>) -> Option<SelectedAction> {
    let item = first_guidance_item(value).or_else(|| first_suppressed_item(value))?;
    Some(SelectedAction {
        source: "pr_guidance".to_string(),
        source_artifact: "pr_guidance".to_string(),
        seam_id: string_path(item, &["seam_id"]),
        seam_kind: string_path(item, &["kind"]).or_else(|| string_path(item, &["seam", "kind"])),
        path: string_path(item, &["placement", "path"])
            .or_else(|| string_path(item, &["seam", "file"]))
            .or_else(|| string_path(item, &["path"])),
        line: u64_path(item, &["placement", "line"])
            .or_else(|| u64_path(item, &["seam", "line"]))
            .or_else(|| u64_path(item, &["line"])),
        classification: string_path(item, &["classification"])
            .or_else(|| string_path(item, &["grip_class"]))
            .map(static_classification),
        missing_discriminator: string_path(item, &["missing_discriminator"]),
    })
}

fn selected_from_receipt(value: Option<&Value>) -> Option<SelectedAction> {
    let value = value?;
    Some(SelectedAction {
        source: "receipt".to_string(),
        source_artifact: "receipt".to_string(),
        seam_id: string_path(value, &["provenance", "seam_id"])
            .or_else(|| string_path(value, &["seam", "seam_id"])),
        seam_kind: string_path(value, &["seam", "seam_kind"]),
        path: string_path(value, &["seam", "file"]),
        line: u64_path(value, &["seam", "line"]),
        classification: string_path(value, &["seam", "grip_class"])
            .or_else(|| string_path(value, &["provenance", "after_class"]))
            .or_else(|| string_path(value, &["seam", "after"]))
            .map(static_classification),
        missing_discriminator: None,
    })
}

fn selected_from_baseline_delta(value: Option<&Value>) -> Option<SelectedAction> {
    let item = value
        .and_then(|value| value.get("items"))
        .and_then(Value::as_array)
        .and_then(|items| {
            items.iter().find(|item| {
                matches!(
                    string_path(item, &["bucket"]).as_deref(),
                    Some("still_present" | "new_policy_eligible")
                )
            })
        })?;
    Some(SelectedAction {
        source: "baseline_delta".to_string(),
        source_artifact: "baseline_delta".to_string(),
        seam_id: string_path(item, &["identity", "seam_id"])
            .or_else(|| string_path(item, &["seam_id"])),
        seam_kind: string_path(item, &["seam_kind"]).or_else(|| string_path(item, &["kind"])),
        path: string_path(item, &["path"]),
        line: u64_path(item, &["line"]),
        classification: string_path(item, &["static_class"])
            .or_else(|| string_path(item, &["grip_class"]))
            .map(static_classification),
        missing_discriminator: string_path(item, &["missing_discriminator"]),
    })
}

fn selected_from_ledger(value: Option<&Value>) -> Option<SelectedAction> {
    let value = value?;
    let route = value.get("top_repair_route");
    let receipt = value
        .get("repair_receipts")
        .and_then(Value::as_array)
        .and_then(|items| items.first());
    Some(SelectedAction {
        source: "ledger".to_string(),
        source_artifact: "ledger".to_string(),
        seam_id: string_from_sources(&[
            (route, &["seam_id"]),
            (receipt, &["seam_id"]),
            (
                value
                    .get("acknowledged")
                    .and_then(Value::as_array)
                    .and_then(|items| items.first()),
                &["seam_id"],
            ),
            (
                value
                    .get("waivers")
                    .and_then(Value::as_array)
                    .and_then(|items| items.first()),
                &["seam_id"],
            ),
        ]),
        seam_kind: string_from_sources(&[(route, &["seam_kind"]), (receipt, &["seam_kind"])]),
        path: string_from_sources(&[(route, &["path"]), (receipt, &["path"])]),
        line: u64_from_sources(&[(route, &["line"]), (receipt, &["line"])]),
        classification: string_from_sources(&[
            (route, &["classification"]),
            (route, &["grip_class"]),
        ])
        .map(static_classification),
        missing_discriminator: string_from_sources(&[(route, &["missing_discriminator"])]),
    })
}

fn selected_from_gate(value: Option<&Value>) -> Option<SelectedAction> {
    let decision = value
        .and_then(|value| value.get("decisions"))
        .and_then(Value::as_array)
        .and_then(|items| items.first())?;
    Some(SelectedAction {
        source: "gate_decision".to_string(),
        source_artifact: "gate_decision".to_string(),
        seam_id: string_path(decision, &["seam_id"])
            .or_else(|| string_path(decision, &["evidence", "seam_id"])),
        seam_kind: string_path(decision, &["seam_kind"])
            .or_else(|| string_path(decision, &["evidence", "seam_kind"])),
        path: string_path(decision, &["path"])
            .or_else(|| string_path(decision, &["evidence", "path"])),
        line: u64_path(decision, &["line"]).or_else(|| u64_path(decision, &["evidence", "line"])),
        classification: string_path(decision, &["classification"])
            .or_else(|| string_path(decision, &["evidence", "classification"]))
            .map(static_classification),
        missing_discriminator: string_path(decision, &["missing_discriminator"])
            .or_else(|| string_path(decision, &["evidence", "missing_discriminator"])),
    })
}

fn selected_from_editor_context(value: Option<&Value>) -> Option<SelectedAction> {
    let value = value?;
    let seam = value
        .get("selected")
        .or_else(|| value.get("seam"))
        .or_else(|| value.get("top_seam"))?;
    Some(SelectedAction {
        source: "editor_context".to_string(),
        source_artifact: "editor_context".to_string(),
        seam_id: string_path(seam, &["seam_id"]),
        seam_kind: string_path(seam, &["seam_kind"]).or_else(|| string_path(seam, &["kind"])),
        path: string_path(seam, &["path"]).or_else(|| string_path(seam, &["file"])),
        line: u64_path(seam, &["line"]),
        classification: string_path(seam, &["classification"])
            .or_else(|| string_path(seam, &["grip_class"]))
            .map(static_classification),
        missing_discriminator: string_path(seam, &["missing_discriminator"]),
    })
}

fn target_for_route(route: &ActionRoute, parsed: &ParsedSources) -> Option<ActionTarget> {
    if matches!(
        route,
        ActionRoute::BaselineOnly
            | ActionRoute::Acknowledged
            | ActionRoute::Waived
            | ActionRoute::Suppressed
            | ActionRoute::Stale
            | ActionRoute::MissingAssistantProof(_)
            | ActionRoute::NoActionableSeam
    ) {
        return None;
    }
    let guidance = first_guidance_item(parsed.pr_guidance.as_ref());
    let proof = parsed.assistant_proof.as_ref();
    let suggested = guidance.and_then(|item| item.get("suggested_test"));
    let file = suggested
        .and_then(|value| string_path(value, &["recommended_file"]))
        .or_else(|| string_path_any(proof, &[&["target", "file"]]));
    let related_test = suggested
        .and_then(related_test_from_suggested)
        .or_else(|| string_path_any(proof, &[&["recommendation", "related_test"]]));
    let suggested_test_name = suggested
        .and_then(|value| string_path(value, &["recommended_name"]))
        .or_else(|| string_path_any(proof, &[&["target", "suggested_test_name"]]));
    let suggested_assertion = suggested
        .and_then(suggested_assertion_sentence)
        .or_else(|| string_path_any(proof, &[&["target", "suggested_assertion"]]));
    if file.is_none() && related_test.is_none() && suggested_test_name.is_none() {
        return None;
    }
    Some(ActionTarget {
        file,
        related_test,
        suggested_test_name,
        suggested_assertion,
    })
}

fn commands_for_route(
    route: &ActionRoute,
    input: &FirstUsefulActionInput,
    parsed: &ParsedSources,
    selected: Option<&SelectedAction>,
) -> ActionCommands {
    let seam_id = selected.and_then(|selected| selected.seam_id.as_deref());
    let root = input.root.as_str();
    match route {
        ActionRoute::Actionable | ActionRoute::UnchangedAfterAttempt => ActionCommands {
            context_packet: seam_id
                .map(|seam_id| format!("ripr agent packet --root {root} --seam-id {seam_id} --json")),
            after_snapshot: Some(format!(
                "ripr check --root {root} --mode draft --format repo-exposure-json > target/ripr/workflow/after.repo-exposure.json"
            )),
            verify: verify_command_from_sources(parsed).or_else(|| {
                Some(format!(
                    "ripr agent verify --root {root} --before target/ripr/workflow/before.repo-exposure.json --after target/ripr/workflow/after.repo-exposure.json --json"
                ))
            }),
            receipt: seam_id.map(|seam_id| {
                format!(
                    "ripr agent receipt --root {root} --verify-json target/ripr/workflow/agent-verify.json --seam-id {seam_id} --json"
                )
            }),
            assistant_proof: None,
            status: None,
        },
        ActionRoute::AlreadyImproved => ActionCommands {
            context_packet: None,
            after_snapshot: None,
            verify: None,
            receipt: seam_id.map(|seam_id| {
                format!(
                    "ripr agent receipt --root {root} --verify-json target/ripr/workflow/agent-verify.json --seam-id {seam_id} --json"
                )
            }),
            assistant_proof: None,
            status: None,
        },
        ActionRoute::MissingAssistantProof(_) => ActionCommands {
            context_packet: None,
            after_snapshot: None,
            verify: None,
            receipt: None,
            assistant_proof: Some(DEFAULT_ASSISTANT_PROOF_COMMAND.to_string()),
            status: None,
        },
        ActionRoute::Stale => ActionCommands {
            context_packet: None,
            after_snapshot: None,
            verify: None,
            receipt: None,
            assistant_proof: None,
            status: Some(format!("ripr agent status --root {root} --json")),
        },
        _ => ActionCommands {
            context_packet: None,
            after_snapshot: None,
            verify: None,
            receipt: None,
            assistant_proof: None,
            status: None,
        },
    }
}

fn evidence_for_route(input: &FirstUsefulActionInput, parsed: &ParsedSources) -> ActionEvidence {
    ActionEvidence {
        pr_guidance: path_if_present(&input.pr_guidance_path, &parsed.pr_guidance),
        assistant_proof: path_if_present(&input.assistant_proof_path, &parsed.assistant_proof),
        receipt: path_if_present(&input.receipt_path, &parsed.receipt),
        ledger: path_if_present(&input.ledger_path, &parsed.ledger),
        baseline_delta: path_if_present(&input.baseline_delta_path, &parsed.baseline_delta),
        static_movement: explicit_receipt_movement(parsed.receipt.as_ref())
            .unwrap_or_else(|| "unknown".to_string()),
    }
}

fn inputs_for_report(input: &FirstUsefulActionInput, parsed: &ParsedSources) -> FirstActionInputs {
    FirstActionInputs {
        pr_guidance: path_if_present(&input.pr_guidance_path, &parsed.pr_guidance),
        assistant_proof: path_if_present(&input.assistant_proof_path, &parsed.assistant_proof),
        ledger: path_if_present(&input.ledger_path, &parsed.ledger),
        baseline_delta: path_if_present(&input.baseline_delta_path, &parsed.baseline_delta),
        receipt: path_if_present(&input.receipt_path, &parsed.receipt),
        gate_decision: path_if_present(&input.gate_decision_path, &parsed.gate_decision),
        coverage_frontier: path_if_present(
            &input.coverage_frontier_path,
            &parsed.coverage_frontier,
        ),
        editor_context: path_if_present(&input.editor_context_path, &parsed.editor_context),
    }
}

fn fallback_for_route(route: &ActionRoute) -> Option<ActionFallback> {
    match route {
        ActionRoute::Actionable => None,
        ActionRoute::Stale => Some(ActionFallback {
            kind: "refresh_evidence".to_string(),
            summary: Some("Refresh RIPR evidence before selecting a focused-test action.".to_string()),
            missing: None,
        }),
        ActionRoute::MissingAssistantProof(path) => Some(ActionFallback {
            kind: "missing_required_artifact".to_string(),
            summary: None,
            missing: Some(path.clone()),
        }),
        ActionRoute::BaselineOnly => Some(ActionFallback {
            kind: "baseline_only".to_string(),
            summary: Some(
                "Track or acknowledge baseline debt separately from PR-local first action."
                    .to_string(),
            ),
            missing: None,
        }),
        ActionRoute::Acknowledged => Some(ActionFallback {
            kind: "acknowledged".to_string(),
            summary: Some(
                "Inspect the proof report or acknowledgement context instead of requesting a new focused test."
                    .to_string(),
            ),
            missing: None,
        }),
        ActionRoute::Waived => Some(ActionFallback {
            kind: "waived".to_string(),
            summary: Some("No first action while the waiver is in force.".to_string()),
            missing: None,
        }),
        ActionRoute::Suppressed => Some(ActionFallback {
            kind: "suppressed".to_string(),
            summary: Some(
                "Suppressed evidence remains visible for audit, but no focused-test action is emitted."
                    .to_string(),
            ),
            missing: None,
        }),
        ActionRoute::NoActionableSeam => Some(ActionFallback {
            kind: "no_actionable_seam".to_string(),
            summary: Some("No first useful test action is available from the supplied artifacts.".to_string()),
            missing: None,
        }),
        ActionRoute::AlreadyImproved => Some(ActionFallback {
            kind: "already_improved".to_string(),
            summary: Some("Include the receipt in review instead of requesting another test.".to_string()),
            missing: None,
        }),
        ActionRoute::UnchangedAfterAttempt => Some(ActionFallback {
            kind: "unchanged_after_attempt".to_string(),
            summary: Some(
                "Revise the focused test using the missing discriminator before moving to another seam."
                    .to_string(),
            ),
            missing: None,
        }),
    }
}

struct RouteText {
    status: &'static str,
    audience: &'static str,
    action_kind: &'static str,
    title: &'static str,
    why: &'static str,
    why_first: &'static [&'static str],
    limits: &'static [&'static str],
}

fn route_text(route: &ActionRoute) -> RouteText {
    match route {
        ActionRoute::Actionable => RouteText {
            status: "actionable",
            audience: "developer",
            action_kind: "write_focused_test",
            title: "Add equality-boundary discriminator test",
            why: "Changed predicate boundary is weakly exposed and lacks an equality-boundary discriminator.",
            why_first: &[
                "The seam is PR-local.",
                "The assistant proof report links guidance, handoff, before/after evidence, and receipt inputs.",
                "No waiver, acknowledgement, or suppression applies.",
            ],
            limits: &[
                "Static evidence only.",
                "Does not run mutation testing.",
                "Does not edit source or generate tests.",
                "Does not make CI blocking by default.",
            ],
        },
        ActionRoute::Stale => RouteText {
            status: "stale",
            audience: "developer",
            action_kind: "refresh_evidence",
            title: "Refresh RIPR evidence before acting",
            why: "The best available seam evidence is stale.",
            why_first: &[
                "Stale evidence blocks first-action routing.",
                "The report must not present stale seam evidence as current.",
            ],
            limits: &[
                "Static evidence only.",
                "Does not rerun hidden analysis.",
                "Does not edit source or generate tests.",
            ],
        },
        ActionRoute::MissingAssistantProof(_) => RouteText {
            status: "missing_required_artifact",
            audience: "agent",
            action_kind: "generate_missing_artifact",
            title: "Generate assistant proof before routing",
            why: "Required joined proof input is missing.",
            why_first: &[
                "Required joined proof input is missing.",
                "The report must not infer proof state from a raw artifact chain.",
            ],
            limits: &[
                "Static evidence only.",
                "Does not search hidden state.",
                "Does not change CI blocking.",
            ],
        },
        ActionRoute::BaselineOnly => RouteText {
            status: "baseline_only",
            audience: "reviewer",
            action_kind: "acknowledge_baseline",
            title: "Leave existing baseline debt outside this PR action",
            why: "The visible debt is baseline-only and not PR-local first-action work.",
            why_first: &[
                "The visible debt is baseline-only.",
                "No new PR-local actionable seam outranks it.",
            ],
            limits: &[
                "Static evidence only.",
                "Does not invent policy.",
                "Does not make CI blocking by default.",
            ],
        },
        ActionRoute::Acknowledged => RouteText {
            status: "acknowledged",
            audience: "reviewer",
            action_kind: "inspect_proof_report",
            title: "Review acknowledged RIPR item",
            why: "The item has explicit acknowledgement.",
            why_first: &[
                "The item has explicit acknowledgement.",
                "Acknowledged evidence remains visible but should not outrank unsuppressed PR-local work.",
            ],
            limits: &[
                "Static evidence only.",
                "Does not invent policy.",
                "Does not edit source or generate tests.",
            ],
        },
        ActionRoute::Waived => RouteText {
            status: "waived",
            audience: "reviewer",
            action_kind: "no_action",
            title: "No first action for waived RIPR item",
            why: "The item has an explicit waiver.",
            why_first: &[
                "The item has an explicit waiver.",
                "Waived evidence stays visible but does not create a focused-test action.",
            ],
            limits: &[
                "Static evidence only.",
                "Does not invent policy.",
                "Does not change CI blocking.",
            ],
        },
        ActionRoute::Suppressed => RouteText {
            status: "suppressed",
            audience: "developer",
            action_kind: "no_action",
            title: "No first action for suppressed seam",
            why: "The seam is suppressed or configured off.",
            why_first: &[
                "The seam is suppressed or configured off.",
                "Suppression state must not be treated as improvement.",
            ],
            limits: &[
                "Static evidence only.",
                "Does not edit source or generate tests.",
                "Does not change policy.",
            ],
        },
        ActionRoute::NoActionableSeam => RouteText {
            status: "no_actionable_seam",
            audience: "developer",
            action_kind: "no_action",
            title: "No actionable RIPR seam found",
            why: "Fresh inputs do not contain a PR-local actionable seam.",
            why_first: &[
                "Fresh inputs do not contain a PR-local actionable seam.",
                "The report should return an explicit clean state instead of silence.",
            ],
            limits: &[
                "Static evidence only.",
                "Does not prove runtime adequacy.",
                "Does not run mutation testing.",
            ],
        },
        ActionRoute::AlreadyImproved => RouteText {
            status: "already_improved",
            audience: "reviewer",
            action_kind: "no_action",
            title: "Static evidence already improved",
            why: "The supplied receipt records improved or resolved static movement.",
            why_first: &[
                "The supplied receipt records improved or resolved static movement.",
                "No additional focused-test action should outrank the receipt.",
            ],
            limits: &[
                "Static evidence only.",
                "Does not prove runtime adequacy.",
                "Does not run mutation testing.",
            ],
        },
        ActionRoute::UnchangedAfterAttempt => RouteText {
            status: "unchanged_after_attempt",
            audience: "agent",
            action_kind: "revise_focused_test",
            title: "Revise the focused test for unchanged static movement",
            why: "The supplied receipt records unchanged static movement after a focused-test attempt.",
            why_first: &[
                "The supplied receipt records unchanged static movement after a focused-test attempt.",
                "The next safe action is to revise the test rather than request a new unrelated seam.",
            ],
            limits: &[
                "Static evidence only.",
                "Does not edit source or generate tests.",
                "Does not run mutation testing.",
            ],
        },
    }
}

fn evidence_is_stale(editor_context: Option<&Value>) -> bool {
    let Some(editor_context) = editor_context else {
        return false;
    };
    matches!(
        string_from_sources(&[
            (Some(editor_context), &["freshness"]),
            (Some(editor_context), &["staleness", "state"]),
            (Some(editor_context), &["state"]),
        ])
        .as_deref(),
        Some("stale")
    )
}

fn has_actionable_pr_local(pr_guidance: Option<&Value>, assistant_proof: Option<&Value>) -> bool {
    first_guidance_item(pr_guidance).is_some() && selected_from_proof(assistant_proof).is_some()
}

fn has_baseline_only(baseline_delta: Option<&Value>, ledger: Option<&Value>) -> bool {
    let delta_present = usize_path_from_sources(&[
        (baseline_delta, &["delta", "still_present"]),
        (baseline_delta, &["movement", "baseline_still_present"]),
    ])
    .unwrap_or(0)
        > 0
        || selected_from_baseline_delta(baseline_delta).is_some();
    let new_pr_local =
        usize_path_from_sources(&[(ledger, &["movement", "new_policy_eligible"])]).unwrap_or(0);
    delta_present && new_pr_local == 0
}

fn has_acknowledged_item(ledger: Option<&Value>, gate_decision: Option<&Value>) -> bool {
    usize_path_from_sources(&[
        (ledger, &["movement", "acknowledged"]),
        (gate_decision, &["summary", "acknowledged"]),
    ])
    .unwrap_or(0)
        > 0
        || decisions_include(gate_decision, "acknowledged")
}

fn has_waived_item(ledger: Option<&Value>, gate_decision: Option<&Value>) -> bool {
    ledger
        .and_then(|value| value.get("waivers"))
        .and_then(Value::as_array)
        .is_some_and(|items| !items.is_empty())
        || decisions_include(gate_decision, "waived")
        || string_from_sources(&[(gate_decision, &["gate", "decision"])]).as_deref()
            == Some("waived")
}

fn has_suppressed_item(pr_guidance: Option<&Value>, baseline_delta: Option<&Value>) -> bool {
    pr_guidance
        .and_then(|value| value.get("suppressed"))
        .and_then(Value::as_array)
        .is_some_and(|items| !items.is_empty())
        || usize_path_from_sources(&[
            (pr_guidance, &["summary", "suppressed"]),
            (baseline_delta, &["delta", "suppressed"]),
        ])
        .unwrap_or(0)
            > 0
        || pr_guidance
            .and_then(|value| value.get("warnings"))
            .and_then(Value::as_array)
            .is_some_and(|warnings| {
                warnings.iter().any(|warning| {
                    warning.as_str().is_some_and(|warning| {
                        warning.contains("configured off") || warning.contains("suppressed")
                    })
                })
            })
}

fn decisions_include(gate_decision: Option<&Value>, decision: &str) -> bool {
    gate_decision
        .and_then(|value| value.get("decisions"))
        .and_then(Value::as_array)
        .is_some_and(|items| {
            items.iter().any(|item| {
                string_path(item, &["decision"]).as_deref() == Some(decision)
                    || string_path(item, &["status"]).as_deref() == Some(decision)
            })
        })
}

fn explicit_receipt_movement(receipt: Option<&Value>) -> Option<String> {
    string_from_sources(&[
        (receipt, &["provenance", "movement"]),
        (receipt, &["seam", "change"]),
        (receipt, &["static_movement", "state"]),
    ])
}

fn first_guidance_item(pr_guidance: Option<&Value>) -> Option<&Value> {
    pr_guidance
        .and_then(|value| value.get("comments"))
        .and_then(Value::as_array)
        .and_then(|items| items.first())
}

fn first_suppressed_item(pr_guidance: Option<&Value>) -> Option<&Value> {
    pr_guidance
        .and_then(|value| value.get("suppressed"))
        .and_then(Value::as_array)
        .and_then(|items| items.first())
}

fn related_test_from_suggested(value: &Value) -> Option<String> {
    let name = string_path(value, &["near_test"])
        .or_else(|| string_path(value, &["recommended_name"]))
        .or_else(|| string_path(value, &["related_test"]))?;
    let file = string_path(value, &["recommended_file"]);
    Some(match file {
        Some(file) if !name.contains("::") => format!("{file}::{name}"),
        _ => name,
    })
}

fn suggested_assertion_sentence(value: &Value) -> Option<String> {
    let assertion = string_path(value, &["assertion_shape"])?;
    let expression = assertion
        .strip_prefix("assert_eq!(")
        .unwrap_or(assertion.as_str());
    let Some(function) = expression.split('(').next().map(str::trim) else {
        return Some(assertion);
    };
    let boundary = assertion
        .split("/*")
        .nth(1)
        .and_then(|rest| rest.split("*/").next())
        .map(str::trim)
        .filter(|value| !value.is_empty());
    match boundary {
        Some(boundary) if !function.is_empty() => {
            Some(format!("Assert the exact {function} output at {boundary}."))
        }
        _ => Some(assertion),
    }
}

fn verify_command_from_sources(parsed: &ParsedSources) -> Option<String> {
    string_from_sources(&[
        (
            parsed.assistant_proof.as_ref(),
            &["recommendation", "verify_command"],
        ),
        (
            first_guidance_item(parsed.pr_guidance.as_ref()),
            &["llm_guidance", "verify_command"],
        ),
    ])
}

fn static_classification(value: String) -> String {
    match value.as_str() {
        "strongly_gripped" => "exposed".to_string(),
        "weakly_gripped" => "weakly_exposed".to_string(),
        "ungripped" => "reachable_unrevealed".to_string(),
        "activation_unknown" => "infection_unknown".to_string(),
        "observation_unknown" | "discrimination_unknown" => "static_unknown".to_string(),
        _ => value,
    }
}

fn path_if_present(path: &Option<String>, value: &Option<Value>) -> Option<String> {
    if value.is_some() { path.clone() } else { None }
}

fn option_from_selected<T, F>(items: &[Option<&SelectedAction>], f: F) -> Option<T>
where
    F: Fn(&SelectedAction) -> Option<T>,
{
    items.iter().find_map(|item| item.and_then(&f))
}

fn string_from_selected(items: &[Option<&SelectedAction>]) -> Option<String> {
    option_from_selected(items, |selected| selected.seam_id.clone())
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

fn string_path_any(value: Option<&Value>, paths: &[&[&str]]) -> Option<String> {
    let value = value?;
    paths.iter().find_map(|path| string_path(value, path))
}

fn string_path(value: &Value, path: &[&str]) -> Option<String> {
    path_value(value, path).and_then(value_as_string)
}

fn u64_path(value: &Value, path: &[&str]) -> Option<u64> {
    path_value(value, path).and_then(value_as_u64)
}

fn usize_path_from_sources(sources: &[(Option<&Value>, &[&str])]) -> Option<usize> {
    sources
        .iter()
        .find_map(|(value, path)| value.and_then(|value| usize_path(value, path)))
}

fn usize_path(value: &Value, path: &[&str]) -> Option<usize> {
    path_value(value, path).and_then(value_as_usize)
}

fn path_value<'a>(value: &'a Value, path: &[&str]) -> Option<&'a Value> {
    let mut current = value;
    for key in path {
        current = current.get(*key)?;
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

fn value_as_u64(value: &Value) -> Option<u64> {
    if let Some(number) = value.as_u64() {
        return Some(number);
    }
    if let Some(number) = value.as_i64() {
        return u64::try_from(number).ok();
    }
    value.as_str().and_then(|text| text.trim().parse().ok())
}

fn value_as_usize(value: &Value) -> Option<usize> {
    value_as_u64(value).and_then(|number| usize::try_from(number).ok())
}

fn wrap_markdown_reason(reason: &str) -> String {
    if reason
        == "The assistant proof report links guidance, handoff, before/after evidence, and receipt inputs."
    {
        return "The assistant proof report links guidance, handoff, before/after evidence,\n  and receipt inputs.".to_string();
    }
    reason.to_string()
}

#[cfg(test)]
mod tests {
    use super::{
        FirstUsefulActionInput, build_first_useful_action_report, render_first_useful_action_json,
        render_first_useful_action_markdown,
    };
    use std::path::{Path, PathBuf};

    #[test]
    fn first_useful_action_matches_actionable_fixture() -> Result<(), String> {
        let repo_root = repo_root()?;
        let fixture =
            repo_root.join("fixtures/boundary_gap/expected/first-useful-action/actionable");
        let proof = repo_root
            .join("fixtures/boundary_gap/expected/test-oracle-assistant-loop/canonical/test-oracle-assistant-proof.json");
        let guidance = repo_root.join(
            "fixtures/boundary_gap/expected/test-oracle-assistant-loop/canonical/pr-guidance.json",
        );
        let ledger = repo_root
            .join("fixtures/boundary_gap/expected/test-oracle-assistant-loop/canonical/pr-evidence-ledger.json");

        let report = build_first_useful_action_report(FirstUsefulActionInput {
            root: "fixtures/boundary_gap/input".to_string(),
            generated_at: "2026-05-09T12:00:00Z".to_string(),
            pr_guidance_path: Some(fixture_path(&repo_root, &guidance)),
            assistant_proof_path: Some(fixture_path(&repo_root, &proof)),
            ledger_path: Some(fixture_path(&repo_root, &ledger)),
            baseline_delta_path: None,
            receipt_path: None,
            gate_decision_path: None,
            coverage_frontier_path: None,
            editor_context_path: None,
            pr_guidance_json: Some(Ok(read_file(&guidance)?)),
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
            read_file(&fixture.join("first-useful-action.json"))?.trim_end()
        );
        assert_eq!(
            render_first_useful_action_markdown(&report),
            read_file(&fixture.join("first-useful-action.md"))?
        );
        Ok(())
    }

    #[test]
    fn first_useful_action_reports_missing_assistant_proof() -> Result<(), String> {
        let repo_root = repo_root()?;
        let fixture = repo_root
            .join("fixtures/boundary_gap/expected/first-useful-action/missing-required-artifact");
        let guidance = repo_root.join(
            "fixtures/boundary_gap/expected/test-oracle-assistant-loop/canonical/pr-guidance.json",
        );
        let ledger = repo_root
            .join("fixtures/boundary_gap/expected/test-oracle-assistant-loop/canonical/pr-evidence-ledger.json");

        let report = build_first_useful_action_report(FirstUsefulActionInput {
            root: "fixtures/boundary_gap/input".to_string(),
            generated_at: "2026-05-09T12:00:00Z".to_string(),
            pr_guidance_path: Some(fixture_path(&repo_root, &guidance)),
            assistant_proof_path: Some("target/ripr/reports/test-oracle-assistant-proof.json".to_string()),
            ledger_path: Some(fixture_path(&repo_root, &ledger)),
            baseline_delta_path: None,
            receipt_path: None,
            gate_decision_path: None,
            coverage_frontier_path: None,
            editor_context_path: None,
            pr_guidance_json: Some(Ok(read_file(&guidance)?)),
            assistant_proof_json: Some(Err(
                "read assistant proof target/ripr/reports/test-oracle-assistant-proof.json failed: missing"
                    .to_string(),
            )),
            ledger_json: Some(Ok(read_file(&ledger)?)),
            baseline_delta_json: None,
            receipt_json: None,
            gate_decision_json: None,
            coverage_frontier_json: None,
            editor_context_json: None,
        });
        assert_eq!(
            render_first_useful_action_json(&report)?,
            read_file(&fixture.join("first-useful-action.json"))?.trim_end()
        );
        Ok(())
    }

    #[test]
    fn first_useful_action_routes_unchanged_receipt_back_to_same_task() -> Result<(), String> {
        let repo_root = repo_root()?;
        let fixture = repo_root
            .join("fixtures/boundary_gap/expected/first-useful-action/unchanged-after-attempt");
        let proof = repo_root
            .join("fixtures/boundary_gap/expected/test-oracle-assistant-loop/canonical/test-oracle-assistant-proof.json");
        let guidance = repo_root.join(
            "fixtures/boundary_gap/expected/test-oracle-assistant-loop/canonical/pr-guidance.json",
        );
        let ledger = repo_root
            .join("fixtures/boundary_gap/expected/test-oracle-assistant-loop/canonical/pr-evidence-ledger.json");
        let receipt =
            repo_root.join("fixtures/boundary_gap/expected/editor-agent-loop/agent-receipt.json");

        let report = build_first_useful_action_report(FirstUsefulActionInput {
            root: "fixtures/boundary_gap/input".to_string(),
            generated_at: "2026-05-09T12:00:00Z".to_string(),
            pr_guidance_path: Some(fixture_path(&repo_root, &guidance)),
            assistant_proof_path: Some(fixture_path(&repo_root, &proof)),
            ledger_path: Some(fixture_path(&repo_root, &ledger)),
            baseline_delta_path: None,
            receipt_path: Some(fixture_path(&repo_root, &receipt)),
            gate_decision_path: None,
            coverage_frontier_path: None,
            editor_context_path: None,
            pr_guidance_json: Some(Ok(read_file(&guidance)?)),
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
            read_file(&fixture.join("first-useful-action.json"))?.trim_end()
        );
        Ok(())
    }

    fn repo_root() -> Result<PathBuf, String> {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(Path::parent)
            .map(Path::to_path_buf)
            .ok_or_else(|| "CARGO_MANIFEST_DIR did not have a workspace parent".to_string())
    }

    fn read_file(path: &Path) -> Result<String, String> {
        std::fs::read_to_string(path)
            .map_err(|err| format!("read {} failed: {err}", path.display()))
    }

    fn fixture_path(repo_root: &Path, path: &Path) -> String {
        match path.strip_prefix(repo_root) {
            Ok(relative) => relative.to_string_lossy().replace('\\', "/"),
            Err(_) => path.to_string_lossy().replace('\\', "/"),
        }
    }
}
