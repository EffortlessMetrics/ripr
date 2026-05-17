use serde::Serialize;
use serde_json::Value;

const SCHEMA_VERSION: &str = "0.1";
const REPORT_KIND: &str = "assistant_loop_health";
const PROOF_KIND: &str = "test_oracle_assistant_loop";
pub(crate) const DEFAULT_ASSISTANT_LOOP_HEALTH_OUT: &str =
    "target/ripr/reports/assistant-loop-health.json";
pub(crate) const DEFAULT_ASSISTANT_LOOP_HEALTH_MD_OUT: &str =
    "target/ripr/reports/assistant-loop-health.md";

const LIMITS: &[&str] = &[
    "Static RIPR evidence only.",
    "Does not provide runtime confirmation.",
    "Does not run mutation testing.",
    "Does not call providers.",
    "Does not edit source or generate tests.",
    "Does not change default CI blocking.",
    "Gate evaluator remains pass/fail authority.",
];

const MARKDOWN_LIMITS: &[&str] = &[
    "Static RIPR evidence only.",
    "Does not run mutation testing.",
    "Does not call providers.",
    "Does not edit source or generate tests.",
    "Gate evaluator remains pass/fail authority.",
];

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct AssistantLoopHealthInput {
    pub(crate) root: String,
    pub(crate) generated_at: String,
    pub(crate) proofs: Vec<AssistantLoopHealthProofInput>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct AssistantLoopHealthProofInput {
    pub(crate) source_artifact: String,
    pub(crate) proof_json: Result<String, String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct AssistantLoopHealthReport {
    status: String,
    root: String,
    generated_at: String,
    inputs: HealthInputs,
    summary: HealthSummary,
    proofs: Vec<HealthProof>,
    warning_summary: Vec<WarningSummary>,
    repair_queue: Vec<RepairItem>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
struct HealthInputs {
    proofs: Vec<String>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
struct HealthSummary {
    proofs: usize,
    complete: usize,
    partial: usize,
    missing_required_input: usize,
    missing_optional_input: usize,
    improved: usize,
    unchanged: usize,
    regressed: usize,
    unknown_movement: usize,
    warnings: usize,
    repair_queue: usize,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
struct HealthProof {
    id: String,
    source_artifact: String,
    proof_state: String,
    movement_state: String,
    seam: Option<HealthSeam>,
    recommendation: Option<HealthRecommendation>,
    handoff: Option<HealthHandoff>,
    receipt: HealthReceipt,
    movement: HealthMovement,
    optional_context: OptionalContext,
    warnings: Vec<HealthWarning>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
struct HealthSeam {
    seam_id: Option<String>,
    seam_kind: Option<String>,
    path: Option<String>,
    line: Option<u64>,
    grip_class: Option<String>,
    missing_discriminator: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
struct HealthRecommendation {
    placement: Option<String>,
    related_test: Option<String>,
    suggested_test: Option<String>,
    verify_command: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
struct HealthHandoff {
    artifact: Option<String>,
    agent_command: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
struct HealthReceipt {
    artifact: Option<String>,
    status: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
struct HealthMovement {
    before_class: Option<String>,
    after_class: Option<String>,
    source: Option<String>,
    source_state: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
struct OptionalContext {
    ledger: Option<String>,
    gate_decision: Option<String>,
    coverage_frontier: Option<String>,
    first_useful_action: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
struct HealthWarning {
    kind: String,
    message: String,
    source_artifact: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
struct WarningSummary {
    kind: String,
    count: usize,
    examples: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
struct RepairItem {
    repair_kind: String,
    source_artifact: String,
    seam_id: Option<String>,
    path: Option<String>,
    line: Option<u64>,
    reason: String,
    next_command: String,
    expected_result: String,
}

pub(crate) fn build_assistant_loop_health_report(
    input: AssistantLoopHealthInput,
) -> AssistantLoopHealthReport {
    let proof_inputs = input.proofs;
    let input_paths = proof_inputs
        .iter()
        .map(|proof| proof.source_artifact.clone())
        .collect::<Vec<_>>();
    let proofs = proof_inputs
        .into_iter()
        .map(classify_proof)
        .collect::<Vec<_>>();
    let warning_summary = warning_summary(&proofs);
    let repair_queue = repair_queue(&proofs);
    let summary = health_summary(&proofs, &repair_queue);
    let status = if proofs
        .iter()
        .any(|proof| proof.proof_state != "missing_required_input")
    {
        "advisory"
    } else {
        "incomplete"
    }
    .to_string();

    AssistantLoopHealthReport {
        status,
        root: input.root,
        generated_at: input.generated_at,
        inputs: HealthInputs {
            proofs: input_paths,
        },
        summary,
        proofs,
        warning_summary,
        repair_queue,
    }
}

pub(crate) fn render_assistant_loop_health_json(
    report: &AssistantLoopHealthReport,
) -> Result<String, String> {
    #[derive(Serialize)]
    struct JsonReport<'a> {
        schema_version: &'static str,
        tool: &'static str,
        kind: &'static str,
        status: &'a str,
        root: &'a str,
        generated_at: &'a str,
        inputs: &'a HealthInputs,
        summary: &'a HealthSummary,
        proofs: &'a [HealthProof],
        warning_summary: &'a [WarningSummary],
        repair_queue: &'a [RepairItem],
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
        proofs: &report.proofs,
        warning_summary: &report.warning_summary,
        repair_queue: &report.repair_queue,
        limits: LIMITS.to_vec(),
    })
    .map_err(|err| format!("render assistant loop health JSON failed: {err}"))
}

pub(crate) fn render_assistant_loop_health_markdown(report: &AssistantLoopHealthReport) -> String {
    let mut out = String::new();
    out.push_str("# RIPR Assistant Loop Health\n\n");
    out.push_str(&format!("Status: {}\n\n", report.status));
    out.push_str("Proof packets:\n");
    out.push_str(&format!("- complete: {}\n", report.summary.complete));
    out.push_str(&format!("- partial: {}\n", report.summary.partial));
    out.push_str(&format!(
        "- missing required inputs: {}\n",
        report.summary.missing_required_input
    ));
    out.push_str(&format!(
        "- missing optional inputs: {}\n\n",
        report.summary.missing_optional_input
    ));

    out.push_str("Evidence movement:\n");
    out.push_str(&format!("- improved: {}\n", report.summary.improved));
    out.push_str(&format!("- unchanged: {}\n", report.summary.unchanged));
    out.push_str(&format!("- regressed: {}\n", report.summary.regressed));
    out.push_str(&format!(
        "- unknown: {}\n\n",
        report.summary.unknown_movement
    ));

    out.push_str("Top warnings:\n");
    if report.warning_summary.is_empty() {
        out.push_str("- none\n\n");
    } else {
        for warning in &report.warning_summary {
            out.push_str(&format!("- {}: {}\n", warning.kind, warning.count));
        }
        out.push('\n');
    }

    out.push_str("Next repair queue:\n");
    if report.repair_queue.is_empty() {
        out.push_str("- none\n\n");
    } else {
        for repair in &report.repair_queue {
            out.push_str(&format!(
                "- `{}` - {}\n",
                repair.repair_kind,
                repair_markdown_line(repair)
            ));
        }
        out.push('\n');
    }

    out.push_str("Limits:\n");
    for limit in MARKDOWN_LIMITS {
        out.push_str(&format!("- {limit}\n"));
    }
    out
}

pub(crate) fn assistant_loop_health_proof_count(report: &AssistantLoopHealthReport) -> usize {
    report.proofs.len()
}

pub(crate) use crate::output::path::display_path;

fn classify_proof(input: AssistantLoopHealthProofInput) -> HealthProof {
    match input.proof_json {
        Ok(text) => match serde_json::from_str::<Value>(&text) {
            Ok(value) => proof_from_value(input.source_artifact, &value),
            Err(error) => missing_required_proof(
                input.source_artifact,
                format!("Proof input is malformed: {error}"),
            ),
        },
        Err(error) => missing_required_proof(
            input.source_artifact,
            format!("Proof input is unreadable: {error}"),
        ),
    }
}

fn proof_from_value(source_artifact: String, value: &Value) -> HealthProof {
    if value.get("schema_version").and_then(Value::as_str) != Some(SCHEMA_VERSION) {
        return missing_required_proof(
            source_artifact,
            "Proof input has an unsupported schema version.".to_string(),
        );
    }
    if value.get("kind").and_then(Value::as_str) != Some(PROOF_KIND) {
        return missing_required_proof(
            source_artifact,
            "Proof input has an unsupported kind.".to_string(),
        );
    }

    let seam = health_seam(value);
    let movement = health_movement(value);
    let missing_required = value.get("status").and_then(Value::as_str)
        == Some("missing_required_input")
        || seam
            .as_ref()
            .and_then(|seam| seam.seam_id.as_ref())
            .is_none()
        || (movement.before_class.is_none()
            && movement.after_class.is_none()
            && movement.source_state.as_deref() != Some("unknown"));

    if missing_required || value.get("evidence_movement").is_none() {
        return missing_required_proof(
            source_artifact,
            "Proof input is missing selected seam and before/after movement context.".to_string(),
        );
    }

    let recommendation = health_recommendation(value);
    let handoff = health_handoff(value);
    let optional_context = optional_context(value);
    let receipt = health_receipt(value, &movement);
    let movement_state = normalize_movement_state(movement.source_state.as_deref());
    let mut warnings = proof_warnings(value, &source_artifact, &movement_state);
    add_derived_warnings(value, &movement, &movement_state, &mut warnings);
    let proof_state = proof_state(&warnings, recommendation.as_ref(), handoff.as_ref());
    let id = proof_id(seam.as_ref(), &source_artifact, &proof_state);

    HealthProof {
        id,
        source_artifact,
        proof_state,
        movement_state,
        seam,
        recommendation,
        handoff,
        receipt,
        movement,
        optional_context,
        warnings,
    }
}

fn missing_required_proof(source_artifact: String, message: String) -> HealthProof {
    HealthProof {
        id: "proof-missing-required-input".to_string(),
        source_artifact: source_artifact.clone(),
        proof_state: "missing_required_input".to_string(),
        movement_state: "unknown".to_string(),
        seam: None,
        recommendation: None,
        handoff: None,
        receipt: HealthReceipt {
            artifact: None,
            status: "missing".to_string(),
        },
        movement: HealthMovement {
            before_class: None,
            after_class: None,
            source: None,
            source_state: None,
        },
        optional_context: OptionalContext {
            ledger: None,
            gate_decision: None,
            coverage_frontier: None,
            first_useful_action: None,
        },
        warnings: vec![HealthWarning {
            kind: "missing_required_input".to_string(),
            message,
            source_artifact: Some(source_artifact),
        }],
    }
}

fn health_seam(value: &Value) -> Option<HealthSeam> {
    let seam = value.get("seam")?;
    if seam.is_null() {
        return None;
    }
    Some(HealthSeam {
        seam_id: string_path(seam, &["seam_id"]),
        seam_kind: string_path(seam, &["seam_kind"]),
        path: string_path(seam, &["path"]),
        line: u64_path(seam, &["line"]),
        grip_class: string_path(seam, &["grip_class"])
            .or_else(|| string_path(seam, &["static_class"])),
        missing_discriminator: string_path(seam, &["missing_discriminator"]),
    })
}

fn health_recommendation(value: &Value) -> Option<HealthRecommendation> {
    let recommendation = value.get("recommendation")?;
    if recommendation.is_null() {
        return None;
    }
    Some(HealthRecommendation {
        placement: string_path(recommendation, &["placement"]),
        related_test: string_path(recommendation, &["related_test"]),
        suggested_test: string_path(recommendation, &["suggested_test"]),
        verify_command: string_path(recommendation, &["verify_command"]),
    })
}

fn health_handoff(value: &Value) -> Option<HealthHandoff> {
    let handoff = value.get("handoff")?;
    if handoff.is_null() {
        return None;
    }
    Some(HealthHandoff {
        artifact: string_path(handoff, &["artifact"]),
        agent_command: string_path(handoff, &["agent_command"]),
    })
}

fn health_movement(value: &Value) -> HealthMovement {
    let movement = value.get("evidence_movement");
    HealthMovement {
        before_class: movement.and_then(|movement| string_path(movement, &["before_class"])),
        after_class: movement.and_then(|movement| string_path(movement, &["after_class"])),
        source: movement.and_then(|movement| string_path(movement, &["source"])),
        source_state: movement.and_then(|movement| string_path(movement, &["state"])),
    }
}

fn health_receipt(value: &Value, movement: &HealthMovement) -> HealthReceipt {
    let artifact = value
        .get("evidence_movement")
        .and_then(|movement| string_path(movement, &["artifact"]));
    let artifact = artifact.or_else(|| {
        if movement.source.as_deref() == Some("agent_receipt") {
            string_path(value, &["inputs", "receipt"])
        } else {
            None
        }
    });
    let status = if artifact.is_some() {
        "present"
    } else {
        "missing"
    }
    .to_string();
    HealthReceipt { artifact, status }
}

fn optional_context(value: &Value) -> OptionalContext {
    let ci = value.get("ci_projection");
    OptionalContext {
        ledger: ci
            .and_then(|ci| string_path(ci, &["ledger"]))
            .or_else(|| string_path(value, &["inputs", "ledger"])),
        gate_decision: ci
            .and_then(|ci| string_path(ci, &["gate_decision"]))
            .or_else(|| string_path(value, &["inputs", "gate_decision"])),
        coverage_frontier: ci
            .and_then(|ci| string_path(ci, &["coverage_frontier"]))
            .or_else(|| string_path(value, &["inputs", "coverage_frontier"])),
        first_useful_action: ci
            .and_then(|ci| string_path(ci, &["first_useful_action"]))
            .or_else(|| string_path(value, &["inputs", "first_useful_action"])),
    }
}

fn proof_warnings(
    value: &Value,
    source_artifact: &str,
    movement_state: &str,
) -> Vec<HealthWarning> {
    match value.get("warnings").and_then(Value::as_array) {
        Some(warnings) => warnings
            .iter()
            .filter_map(Value::as_str)
            .map(|warning| structured_warning(warning, value, source_artifact, movement_state))
            .collect(),
        None => Vec::new(),
    }
}

fn structured_warning(
    warning: &str,
    value: &Value,
    source_artifact: &str,
    movement_state: &str,
) -> HealthWarning {
    let normalized = warning.to_ascii_lowercase();
    if normalized.contains("missing optional pr ledger")
        || normalized.contains("optional pr ledger input is missing")
    {
        return warning_value(
            "missing_optional_input",
            "No PR ledger input was supplied.",
            None,
        );
    }
    if normalized.contains("missing optional gate decision") {
        return warning_value(
            "missing_optional_input",
            "No gate decision input was supplied.",
            None,
        );
    }
    if normalized.contains("missing optional coverage frontier") {
        return warning_value(
            "missing_optional_input",
            "No coverage frontier input was supplied.",
            None,
        );
    }
    if normalized.contains("missing optional first useful action")
        || normalized.contains("optional first useful action input is missing")
    {
        return warning_value(
            "missing_optional_input",
            "No first useful action input was supplied.",
            None,
        );
    }
    if normalized.contains("required proof input is missing")
        || normalized.contains("missing selected seam")
    {
        return warning_value(
            "missing_required_input",
            "Proof input is missing selected seam and before/after movement context.",
            Some(source_artifact.to_string()),
        );
    }
    if normalized.contains("summary-only guidance") {
        return warning_value(
            "summary_only_guidance",
            "Summary-only guidance needs placement review.",
            string_path(value, &["inputs", "pr_guidance"]),
        );
    }
    if normalized.contains("after snapshot is stale") {
        return warning_value("stale_input", "After snapshot is stale or missing.", None);
    }
    if normalized.contains("agent receipt is missing") {
        return warning_value("missing_receipt", "Agent receipt is missing.", None);
    }
    if normalized.contains("static movement remained unchanged") || movement_state == "unchanged" {
        return warning_value(
            "unchanged_movement",
            "Static movement remained unchanged after the focused attempt.",
            string_path(value, &["evidence_movement", "artifact"]),
        );
    }
    if normalized.contains("static movement regressed") || movement_state == "regressed" {
        return warning_value(
            "regressed_movement",
            "Static movement regressed after the focused attempt.",
            string_path(value, &["evidence_movement", "artifact"]),
        );
    }
    if normalized.contains("receipt") {
        return warning_value("missing_receipt", warning, None);
    }
    if normalized.contains("unknown") {
        return warning_value(
            "unknown_movement",
            warning,
            Some(source_artifact.to_string()),
        );
    }
    warning_value("other", warning, Some(source_artifact.to_string()))
}

fn warning_value(kind: &str, message: &str, source_artifact: Option<String>) -> HealthWarning {
    HealthWarning {
        kind: kind.to_string(),
        message: message.to_string(),
        source_artifact,
    }
}

fn add_derived_warnings(
    value: &Value,
    movement: &HealthMovement,
    movement_state: &str,
    warnings: &mut Vec<HealthWarning>,
) {
    if movement_state == "unchanged" && !has_warning_kind(warnings, "unchanged_movement") {
        warnings.push(warning_value(
            "unchanged_movement",
            "Static movement remained unchanged after the focused attempt.",
            movement
                .source
                .as_deref()
                .filter(|source| *source == "agent_receipt")
                .and_then(|_| string_path(value, &["evidence_movement", "artifact"])),
        ));
    }
    if movement_state == "regressed" && !has_warning_kind(warnings, "regressed_movement") {
        warnings.push(warning_value(
            "regressed_movement",
            "Static movement regressed after the focused attempt.",
            movement
                .source
                .as_deref()
                .filter(|source| *source == "agent_receipt")
                .and_then(|_| string_path(value, &["evidence_movement", "artifact"])),
        ));
    }
    if movement_state == "unknown" && !has_warning_kind(warnings, "unknown_movement") {
        let missing_receipt = value
            .get("evidence_movement")
            .and_then(|movement| string_path(movement, &["source"]))
            .as_deref()
            == Some("missing_receipt");
        if !missing_receipt && !has_warning_kind(warnings, "missing_receipt") {
            warnings.push(warning_value(
                "unknown_movement",
                "Static movement is unknown from the supplied proof.",
                None,
            ));
        }
    }
}

fn has_warning_kind(warnings: &[HealthWarning], kind: &str) -> bool {
    warnings.iter().any(|warning| warning.kind == kind)
}

fn proof_state(
    warnings: &[HealthWarning],
    recommendation: Option<&HealthRecommendation>,
    handoff: Option<&HealthHandoff>,
) -> String {
    let partial = recommendation.is_none()
        || handoff.is_none()
        || warnings.iter().any(|warning| {
            matches!(
                warning.kind.as_str(),
                "missing_optional_input"
                    | "stale_input"
                    | "malformed_input"
                    | "incompatible_schema"
                    | "summary_only_guidance"
                    | "missing_receipt"
                    | "missing_handoff"
                    | "unknown_movement"
            )
        });
    if partial { "partial" } else { "complete" }.to_string()
}

fn normalize_movement_state(source_state: Option<&str>) -> String {
    match source_state {
        Some("improved" | "resolved") => "improved",
        Some("unchanged") => "unchanged",
        Some("regressed") => "regressed",
        _ => "unknown",
    }
    .to_string()
}

fn proof_id(seam: Option<&HealthSeam>, source_artifact: &str, proof_state: &str) -> String {
    if proof_state == "missing_required_input" {
        return "proof-missing-required-input".to_string();
    }
    let suffix = proof_suffix(source_artifact);
    match seam.and_then(|seam| seam.seam_id.as_deref()) {
        Some(seam_id) if !suffix.is_empty() => format!("proof-{seam_id}-{suffix}"),
        Some(seam_id) => format!("proof-{seam_id}"),
        None => "proof-missing-required-input".to_string(),
    }
}

fn proof_suffix(source_artifact: &str) -> String {
    let normalized = source_artifact.replace('\\', "/");
    let file = match normalized.rsplit('/').next() {
        Some(file) => file,
        None => normalized.as_str(),
    };
    let without_json = match file.strip_suffix(".json") {
        Some(value) => value,
        None => file,
    };
    match without_json.strip_suffix("-proof") {
        Some(value) => value.to_string(),
        None => without_json.to_string(),
    }
}

fn health_summary(proofs: &[HealthProof], repair_queue: &[RepairItem]) -> HealthSummary {
    let mut summary = HealthSummary {
        proofs: proofs.len(),
        repair_queue: repair_queue.len(),
        ..HealthSummary::default()
    };
    for proof in proofs {
        match proof.proof_state.as_str() {
            "complete" => summary.complete += 1,
            "partial" => summary.partial += 1,
            "missing_required_input" => summary.missing_required_input += 1,
            _ => {}
        }
        match proof.movement_state.as_str() {
            "improved" => summary.improved += 1,
            "unchanged" => summary.unchanged += 1,
            "regressed" => summary.regressed += 1,
            _ => summary.unknown_movement += 1,
        }
        summary.missing_optional_input += proof
            .warnings
            .iter()
            .filter(|warning| warning.kind == "missing_optional_input")
            .count();
        summary.warnings += proof.warnings.len();
    }
    summary
}

fn warning_summary(proofs: &[HealthProof]) -> Vec<WarningSummary> {
    let mut summaries = Vec::<WarningSummary>::new();
    for warning in proofs.iter().flat_map(|proof| proof.warnings.iter()) {
        if let Some(existing) = summaries
            .iter_mut()
            .find(|summary| summary.kind == warning.kind)
        {
            existing.count += 1;
            if existing.examples.len() < 2 && !existing.examples.contains(&warning.message) {
                existing.examples.push(warning.message.clone());
            }
        } else {
            summaries.push(WarningSummary {
                kind: warning.kind.clone(),
                count: 1,
                examples: vec![warning.message.clone()],
            });
        }
    }
    summaries
}

fn repair_queue(proofs: &[HealthProof]) -> Vec<RepairItem> {
    let mut repairs = Vec::new();
    for proof in proofs {
        for warning in &proof.warnings {
            if let Some(repair) = repair_for_warning(proof, warning) {
                repairs.push(repair);
            }
        }
    }
    repairs
}

fn repair_for_warning(proof: &HealthProof, warning: &HealthWarning) -> Option<RepairItem> {
    match warning.kind.as_str() {
        "missing_required_input" => Some(RepairItem {
            repair_kind: "regenerate_proof".to_string(),
            source_artifact: proof.source_artifact.clone(),
            seam_id: None,
            path: None,
            line: None,
            reason: "The supplied proof is missing required selected seam and movement evidence."
                .to_string(),
            next_command: "ripr assistant-loop proof --out target/ripr/reports/test-oracle-assistant-proof.json --out-md target/ripr/reports/test-oracle-assistant-proof.md".to_string(),
            expected_result: "Regenerate a proof packet with selected seam and before/after static movement context.".to_string(),
        }),
        "unchanged_movement" => proof.seam.as_ref().map(|seam| RepairItem {
            repair_kind: "inspect_unchanged_attempt".to_string(),
            source_artifact: proof.source_artifact.clone(),
            seam_id: seam.seam_id.clone(),
            path: seam.path.clone(),
            line: seam.line,
            reason: "Static evidence did not move after the focused attempt.".to_string(),
            next_command: match proof
                .recommendation
                .as_ref()
                .and_then(|recommendation| recommendation.verify_command.clone()) {
                    Some(command) => command,
                    None => "ripr agent verify --json".to_string(),
                },
            expected_result:
                "Inspect whether the focused test observes the missing equality boundary before rerunning receipt generation."
                    .to_string(),
        }),
        "regressed_movement" => proof.seam.as_ref().map(|seam| RepairItem {
            repair_kind: "inspect_regression".to_string(),
            source_artifact: proof.source_artifact.clone(),
            seam_id: seam.seam_id.clone(),
            path: seam.path.clone(),
            line: seam.line,
            reason: "Static evidence weakened after the focused attempt.".to_string(),
            next_command: receipt_command(proof),
            expected_result:
                "Inspect the changed test and receipt before treating the assistant loop as repaired."
                    .to_string(),
        }),
        "summary_only_guidance" => proof.seam.as_ref().map(|seam| RepairItem {
            repair_kind: "inspect_summary_only_guidance".to_string(),
            source_artifact: proof.source_artifact.clone(),
            seam_id: seam.seam_id.clone(),
            path: seam.path.clone(),
            line: seam.line,
            reason: "Guidance was summary-only, so inline placement was not safe.".to_string(),
            next_command: format!(
                "ripr review-comments --root {} --base <base> --head <head> --out target/ripr/review/comments.json",
                command_root(proof)
            ),
            expected_result:
                "Refresh review guidance and inspect whether placement remains summary-only."
                    .to_string(),
        }),
        "stale_input" => proof.seam.as_ref().map(|seam| RepairItem {
            repair_kind: "refresh_before_after_evidence".to_string(),
            source_artifact: proof.source_artifact.clone(),
            seam_id: seam.seam_id.clone(),
            path: seam.path.clone(),
            line: seam.line,
            reason: "The after snapshot is stale or missing.".to_string(),
            next_command: format!(
                "ripr check --root {} --mode draft --format repo-exposure-json > target/ripr/workflow/after.repo-exposure.json",
                command_root(proof)
            ),
            expected_result: "Refresh after evidence before judging assistant-loop movement."
                .to_string(),
        }),
        "missing_receipt" => proof.seam.as_ref().map(|seam| RepairItem {
            repair_kind: "attach_receipt".to_string(),
            source_artifact: proof.source_artifact.clone(),
            seam_id: seam.seam_id.clone(),
            path: seam.path.clone(),
            line: seam.line,
            reason: "No agent receipt was attached to the proof packet.".to_string(),
            next_command: receipt_command(proof),
            expected_result:
                "Attach a receipt so reviewers can inspect static before/after movement."
                    .to_string(),
        }),
        "unknown_movement" => proof.seam.as_ref().map(|seam| RepairItem {
            repair_kind: "refresh_before_after_evidence".to_string(),
            source_artifact: proof.source_artifact.clone(),
            seam_id: seam.seam_id.clone(),
            path: seam.path.clone(),
            line: seam.line,
            reason: "Static movement is unknown from the supplied proof.".to_string(),
            next_command: format!(
                "ripr check --root {} --mode draft --format repo-exposure-json > target/ripr/workflow/after.repo-exposure.json",
                command_root(proof)
            ),
            expected_result: "Refresh before/after evidence before judging assistant-loop movement."
                .to_string(),
        }),
        _ => None,
    }
}

fn receipt_command(proof: &HealthProof) -> String {
    let seam_id = proof
        .seam
        .as_ref()
        .and_then(|seam| seam.seam_id.as_deref())
        .map_or("<seam-id>", |seam_id| seam_id);
    format!(
        "ripr agent receipt --root {} --verify-json target/ripr/workflow/agent-verify.json --seam-id {seam_id} --json",
        command_root(proof)
    )
}

fn command_root(proof: &HealthProof) -> String {
    match proof
        .handoff
        .as_ref()
        .and_then(|handoff| handoff.agent_command.as_deref())
        .and_then(root_from_agent_command)
    {
        Some(root) => root,
        None => ".".to_string(),
    }
}

fn root_from_agent_command(command: &str) -> Option<String> {
    let mut tokens = command.split_whitespace();
    while let Some(token) = tokens.next() {
        if token == "--root" {
            return tokens.next().map(ToOwned::to_owned);
        }
    }
    None
}

fn repair_markdown_line(repair: &RepairItem) -> String {
    match repair.repair_kind.as_str() {
        "regenerate_proof" => {
            "regenerate proof; supply selected seam and before/after static movement context."
                .to_string()
        }
        "inspect_unchanged_attempt" => format!(
            "{} - unchanged movement; inspect whether the focused test observes the missing equality boundary.",
            repair_location(repair)
        ),
        "inspect_regression" => format!(
            "{} - regressed movement; inspect the changed test and receipt.",
            repair_location(repair)
        ),
        "inspect_summary_only_guidance" => format!(
            "{} - summary-only guidance; inspect placement before routing test work.",
            repair_location(repair)
        ),
        "refresh_before_after_evidence" => {
            format!(
                "{} - stale after evidence; refresh before/after evidence.",
                repair_location(repair)
            )
        }
        "attach_receipt" => format!(
            "{} - missing receipt; rerun verify and receipt.",
            repair_location(repair)
        ),
        _ => repair.reason.clone(),
    }
}

fn repair_location(repair: &RepairItem) -> String {
    match (repair.path.as_deref(), repair.line) {
        (Some(path), Some(line)) => format!("{path}:{line}"),
        (Some(path), None) => path.to_string(),
        (None, Some(line)) => format!("unknown:{line}"),
        (None, None) => "unknown".to_string(),
    }
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::{Path, PathBuf};

    #[test]
    fn assistant_loop_health_matches_fixture_corpus() -> Result<(), String> {
        let repo_root = repo_root()?;
        let corpus_path =
            repo_root.join("fixtures/boundary_gap/expected/assistant-loop-health/corpus.json");
        let corpus: Value = serde_json::from_str(&read_file(&corpus_path)?)
            .map_err(|err| format!("parse corpus failed: {err}"))?;
        let cases = corpus
            .get("cases")
            .and_then(Value::as_array)
            .ok_or_else(|| "corpus cases missing".to_string())?;

        for case in cases {
            let case_id = match string_path(case, &["id"]) {
                Some(case_id) => case_id,
                None => "unknown".to_string(),
            };
            let proofs = case
                .get("proofs")
                .and_then(Value::as_array)
                .ok_or_else(|| format!("{case_id} proofs missing"))?
                .iter()
                .map(|proof| {
                    let path = proof
                        .as_str()
                        .ok_or_else(|| format!("{case_id} proof path is not a string"))?;
                    let proof_path = repo_root.join(path);
                    Ok(AssistantLoopHealthProofInput {
                        source_artifact: path.to_string(),
                        proof_json: Ok(read_file(&proof_path)?),
                    })
                })
                .collect::<Result<Vec<_>, String>>()?;
            let report = build_assistant_loop_health_report(AssistantLoopHealthInput {
                root: ".".to_string(),
                generated_at: "2026-05-09T12:00:00Z".to_string(),
                proofs,
            });

            let expected_json_path = repo_root.join(
                string_path(case, &["expected_report"])
                    .ok_or_else(|| format!("{case_id} expected_report missing"))?,
            );
            let expected_md_path = repo_root.join(
                string_path(case, &["expected_markdown"])
                    .ok_or_else(|| format!("{case_id} expected_markdown missing"))?,
            );

            assert_eq!(
                render_assistant_loop_health_json(&report)?,
                read_file(&expected_json_path)?.trim_end(),
                "{case_id} JSON fixture drifted"
            );
            assert_eq!(
                render_assistant_loop_health_markdown(&report),
                read_file(&expected_md_path)?,
                "{case_id} Markdown fixture drifted"
            );
        }
        Ok(())
    }

    #[test]
    fn assistant_loop_health_reports_unreadable_proof_as_incomplete() -> Result<(), String> {
        let report = build_assistant_loop_health_report(AssistantLoopHealthInput {
            root: ".".to_string(),
            generated_at: "2026-05-09T12:00:00Z".to_string(),
            proofs: vec![AssistantLoopHealthProofInput {
                source_artifact: "missing.json".to_string(),
                proof_json: Err("not found".to_string()),
            }],
        });

        let rendered = render_assistant_loop_health_json(&report)?;
        assert!(rendered.contains("\"status\": \"incomplete\""));
        assert!(rendered.contains("\"proof_state\": \"missing_required_input\""));
        assert!(rendered.contains("Proof input is unreadable"));
        Ok(())
    }

    #[test]
    fn assistant_loop_health_classifies_malformed_and_unsupported_proofs() -> Result<(), String> {
        let unsupported_schema = serde_json::json!({
            "schema_version": "99.0",
            "kind": PROOF_KIND
        })
        .to_string();
        let unsupported_kind = serde_json::json!({
            "schema_version": SCHEMA_VERSION,
            "kind": "other_report"
        })
        .to_string();
        let report = build_assistant_loop_health_report(AssistantLoopHealthInput {
            root: ".".to_string(),
            generated_at: "2026-05-09T12:00:00Z".to_string(),
            proofs: vec![
                AssistantLoopHealthProofInput {
                    source_artifact: "malformed-proof.json".to_string(),
                    proof_json: Ok("{".to_string()),
                },
                AssistantLoopHealthProofInput {
                    source_artifact: "unsupported-schema.json".to_string(),
                    proof_json: Ok(unsupported_schema),
                },
                AssistantLoopHealthProofInput {
                    source_artifact: "unsupported-kind.json".to_string(),
                    proof_json: Ok(unsupported_kind),
                },
            ],
        });

        let rendered = render_assistant_loop_health_json(&report)?;
        assert!(rendered.contains("\"status\": \"incomplete\""));
        assert!(rendered.contains("\"missing_required_input\": 3"));
        assert!(rendered.contains("Proof input is malformed"));
        assert!(rendered.contains("unsupported schema version"));
        assert!(rendered.contains("unsupported kind"));
        Ok(())
    }

    #[test]
    fn assistant_loop_health_keeps_receipt_fallback_and_warning_repairs_visible()
    -> Result<(), String> {
        let proof = serde_json::json!({
            "schema_version": SCHEMA_VERSION,
            "tool": "ripr",
            "kind": PROOF_KIND,
            "status": "advisory",
            "root": ".",
            "inputs": {
                "receipt": "target/ripr/workflow/agent-receipt.json"
            },
            "seam": {
                "seam_id": "seam-warning",
                "seam_kind": "predicate_boundary",
                "path": "src/lib.rs",
                "line": 7,
                "grip_class": "weakly_gripped",
                "missing_discriminator": "amount == threshold"
            },
            "recommendation": null,
            "handoff": null,
            "evidence_movement": {
                "state": "unchanged",
                "before_class": "weakly_gripped",
                "after_class": "weakly_gripped",
                "source": "agent_receipt",
                "artifact": null
            },
            "warnings": [
                "Required proof input is missing selected seam context.",
                "Agent receipt is missing."
            ]
        })
        .to_string();
        let unknown_proof = serde_json::json!({
            "schema_version": SCHEMA_VERSION,
            "tool": "ripr",
            "kind": PROOF_KIND,
            "status": "advisory",
            "root": ".",
            "inputs": {},
            "seam": {
                "seam_id": "seam-unknown",
                "seam_kind": "predicate_boundary",
                "path": "src/unknown.rs",
                "line": 11,
                "grip_class": "weakly_gripped"
            },
            "recommendation": {
                "placement": "summary_only",
                "related_test": null,
                "suggested_test": "Add one focused test.",
                "verify_command": null
            },
            "handoff": {
                "artifact": "target/ripr/workflow/agent-brief.json",
                "agent_command": "ripr agent start --root repo-root --seam-id seam-unknown --out target/ripr/workflow"
            },
            "evidence_movement": {
                "state": "unknown",
                "before_class": null,
                "after_class": null,
                "source": "missing_receipt",
                "artifact": null
            },
            "warnings": [
                "Unknown movement from optional artifact.",
                "An unusual advisory warning."
            ]
        })
        .to_string();
        let report = build_assistant_loop_health_report(AssistantLoopHealthInput {
            root: ".".to_string(),
            generated_at: "2026-05-09T12:00:00Z".to_string(),
            proofs: vec![
                AssistantLoopHealthProofInput {
                    source_artifact: "warning-proof.json".to_string(),
                    proof_json: Ok(proof),
                },
                AssistantLoopHealthProofInput {
                    source_artifact: "unknown-proof.json".to_string(),
                    proof_json: Ok(unknown_proof),
                },
            ],
        });

        let rendered = render_assistant_loop_health_json(&report)?;
        assert!(rendered.contains("\"receipt\": {\n        \"artifact\": \"target/ripr/workflow/agent-receipt.json\",\n        \"status\": \"present\""));
        assert!(rendered.contains("\"proof_state\": \"partial\""));
        assert!(rendered.contains("\"kind\": \"missing_required_input\""));
        assert!(rendered.contains("\"kind\": \"missing_receipt\""));
        assert!(rendered.contains("\"kind\": \"unknown_movement\""));
        assert!(rendered.contains("\"kind\": \"other\""));
        assert!(rendered.contains("\"repair_kind\": \"regenerate_proof\""));
        assert!(rendered.contains("\"repair_kind\": \"attach_receipt\""));

        let markdown = render_assistant_loop_health_markdown(&report);
        assert!(markdown.contains("regenerate proof; supply selected seam"));
        assert!(markdown.contains("src/lib.rs:7 - missing receipt"));
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
}
