use super::{
    SCHEMA_VERSION, STATIC_EVIDENCE_BOUNDARY, rendering::expected_output_state_for_selected,
    start_here_output_state_is_known, string_path,
};
use serde_json::Value;
use std::fs;
use std::path::Path;

pub(super) fn validate_start_here_packet(
    json_path: &Path,
    markdown_path: &Path,
) -> Result<Value, String> {
    let text = fs::read_to_string(json_path)
        .map_err(|err| format!("missing or unreadable {}: {err}", json_path.display()))?;
    let packet: Value = serde_json::from_str(&text)
        .map_err(|err| format!("{} is not valid JSON: {err}", json_path.display()))?;
    let mut violations = Vec::new();
    expect_string(&packet, "schema_version", SCHEMA_VERSION, &mut violations);
    expect_string(&packet, "tool", "ripr", &mut violations);
    expect_string(&packet, "kind", "first_pr_start_here", &mut violations);
    match packet.get("status").and_then(Value::as_str) {
        Some("actionable" | "blocked" | "no_action") => {}
        Some(status) => violations.push(format!("status {status:?} is not contract-valid")),
        None => violations.push("status is missing or not a string".to_string()),
    }
    expect_string(&packet, "posture", "advisory", &mut violations);
    if let Some(selected) = packet.get("selected").filter(|value| value.is_object()) {
        validate_selected_state(
            packet
                .get("status")
                .and_then(Value::as_str)
                .unwrap_or("unknown"),
            selected,
            &mut violations,
        );
    } else {
        violations.push("selected is missing or not an object".to_string());
    }
    if !packet.get("commands").is_some_and(Value::is_object) {
        violations.push("commands is missing or not an object".to_string());
    }
    if !packet.get("artifacts").is_some_and(Value::is_array) {
        violations.push("artifacts is missing or not an array".to_string());
    }
    if let Some(preflight) = packet.get("preflight") {
        validate_preflight(preflight, &mut violations);
    }
    if !markdown_path.exists() {
        violations.push(format!("{} is missing", markdown_path.display()));
    }
    if violations.is_empty() {
        Ok(packet)
    } else {
        Err(format!(
            "first-pr start-here contract violations:\n{}",
            violations
                .iter()
                .map(|violation| format!("- {violation}"))
                .collect::<Vec<_>>()
                .join("\n")
        ))
    }
}

fn validate_preflight(preflight: &Value, violations: &mut Vec<String>) {
    match preflight.get("status").and_then(Value::as_str) {
        Some("ready" | "needs_attention") => {}
        Some(status) => violations.push(format!("preflight.status {status:?} is not valid")),
        None => violations.push("preflight.status is missing or not a string".to_string()),
    }
    match preflight.get("mode").and_then(Value::as_str) {
        Some("write" | "check") => {}
        Some(mode) => violations.push(format!("preflight.mode {mode:?} is not valid")),
        None => violations.push("preflight.mode is missing or not a string".to_string()),
    }
    let Some(checks) = preflight.get("checks").and_then(Value::as_array) else {
        violations.push("preflight.checks is missing or not an array".to_string());
        return;
    };
    for check in checks {
        if check.get("id").and_then(Value::as_str).is_none() {
            violations.push("preflight check id is missing or not a string".to_string());
        }
        match check.get("status").and_then(Value::as_str) {
            Some("ok" | "needs_attention" | "no_action" | "defaulted" | "will_create") => {}
            Some(status) => {
                violations.push(format!("preflight check status {status:?} is not valid"));
            }
            None => {
                violations.push("preflight check status is missing or not a string".to_string())
            }
        }
        if check.get("message").and_then(Value::as_str).is_none() {
            violations.push("preflight check message is missing or not a string".to_string());
        }
    }
}

pub(super) fn validate_selected_state(
    status: &str,
    selected: &Value,
    violations: &mut Vec<String>,
) {
    let Some(state) = selected.get("state").and_then(Value::as_str) else {
        violations.push("selected.state is missing or not a string".to_string());
        return;
    };
    let expected_output_state = expected_output_state_for_selected(state, selected);
    match selected.get("output_state").and_then(Value::as_str) {
        Some(output_state) if output_state == expected_output_state => {}
        Some(output_state) if !start_here_output_state_is_known(output_state) => violations.push(
            format!("selected.output_state {output_state:?} is not contract-valid"),
        ),
        Some(output_state) => violations.push(format!(
            "selected.output_state must be {expected_output_state:?} for state {state:?}, found {output_state:?}"
        )),
        None => violations.push("selected.output_state is missing or not a string".to_string()),
    }
    let expected_status = match state {
        "top_gap" => "actionable",
        "missing_artifact" | "malformed_artifact" | "stale_artifact" | "wrong_root"
        | "blocked_artifact" | "timeout" => "blocked",
        "empty_diff" | "no_action" => "no_action",
        other => {
            violations.push(format!("selected.state {other:?} is not contract-valid"));
            return;
        }
    };
    if status != expected_status {
        violations.push(format!(
            "selected.state {state:?} requires status {expected_status:?}, found {status:?}"
        ));
    }
    if state == "top_gap" {
        validate_top_gap_contract(selected, violations);
    }
}

fn validate_top_gap_contract(selected: &Value, violations: &mut Vec<String>) {
    for (path, label) in [
        (&["kind"][..], "top actionable gap"),
        (&["changed_behavior"][..], "changed behavior"),
        (&["why"][..], "why this matters"),
        (
            &["current_evidence_strength"][..],
            "current evidence strength",
        ),
        (&["missing_discriminator"][..], "missing discriminator"),
        (&["focused_proof_intent"][..], "focused proof intent"),
        (&["verify_command"][..], "verify command"),
    ] {
        if string_path(selected, path).is_none() {
            violations.push(format!("selected top_gap must name {label}"));
        }
    }
    if string_path(selected, &["receipt_command"]).is_none()
        && string_path(selected, &["receipt_path"]).is_none()
    {
        violations.push("selected top_gap must name receipt command or path".to_string());
    }
    match string_path(selected, &["static_evidence_boundary"]) {
        Some(boundary) if boundary == STATIC_EVIDENCE_BOUNDARY => {}
        Some(boundary) => violations.push(format!(
            "selected.static_evidence_boundary must be {STATIC_EVIDENCE_BOUNDARY:?}, found {boundary:?}"
        )),
        None => violations.push("selected top_gap must name static_evidence_boundary".to_string()),
    }
}

fn expect_string(packet: &Value, key: &str, expected: &str, violations: &mut Vec<String>) {
    match packet.get(key).and_then(Value::as_str) {
        Some(actual) if actual == expected => {}
        Some(actual) => violations.push(format!("{key} is {actual:?}, expected {expected:?}")),
        None => violations.push(format!("{key} is missing or not a string")),
    }
}
