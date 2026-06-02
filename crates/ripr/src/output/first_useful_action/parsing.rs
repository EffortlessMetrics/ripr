use super::FirstUsefulActionInput;
use serde_json::Value;

#[derive(Default)]
pub(super) struct ParsedSources {
    pub(super) pr_guidance: Option<Value>,
    pub(super) assistant_proof: Option<Value>,
    pub(super) gap_ledger: Option<Value>,
    pub(super) ledger: Option<Value>,
    pub(super) baseline_delta: Option<Value>,
    pub(super) receipt: Option<Value>,
    pub(super) gate_decision: Option<Value>,
    pub(super) coverage_frontier: Option<Value>,
    pub(super) editor_context: Option<Value>,
    pub(super) warnings: Vec<String>,
    pub(super) read_errors: Vec<(String, String)>,
}

pub(super) fn parse_sources(input: &FirstUsefulActionInput) -> ParsedSources {
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

pub(super) fn parse_optional_json(
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
