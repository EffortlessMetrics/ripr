use super::{STATIC_EVIDENCE_BOUNDARY, string_path};
use crate::output::start_here_state::{
    START_HERE_PREVIEW_LIMITED, normalize_start_here_output_state,
};
use serde_json::Value;
use std::path::Path;

pub(super) fn render_start_here_markdown(packet: &Value) -> String {
    let selected = packet.get("selected").unwrap_or(&Value::Null);
    let state = string_path(selected, &["state"]).unwrap_or_else(|| "unknown".to_string());
    let mut out = String::new();
    out.push_str("# RIPR First PR Start Here\n\n");
    out.push_str("Status: advisory\n");
    out.push_str(&format!(
        "State: {}\n\n",
        packet
            .get("status")
            .and_then(Value::as_str)
            .unwrap_or("unknown")
    ));

    match state.as_str() {
        "top_gap" => render_top_gap_markdown(selected, &mut out),
        "missing_artifact" => render_missing_artifact_markdown(selected, &mut out),
        "empty_diff" | "no_action" => render_no_action_markdown(selected, &mut out),
        _ => render_blocked_markdown(selected, &mut out),
    }

    render_preflight_markdown(packet, &mut out);

    out.push_str("\n## Artifacts\n\n");
    if let Some(artifacts) = packet.get("artifacts").and_then(Value::as_array) {
        for artifact in artifacts {
            let label = artifact
                .get("label")
                .and_then(Value::as_str)
                .unwrap_or("artifact");
            let path = artifact.get("path").and_then(Value::as_str).unwrap_or("");
            let status = artifact
                .get("status")
                .and_then(Value::as_str)
                .unwrap_or("unknown");
            out.push_str(&format!("- {label}: `{path}` ({status})\n"));
        }
    }

    out.push_str("\n## Authority\n\n");
    out.push_str(
        "This packet is advisory. Pass/fail authority remains with explicit gate-decision artifacts when configured.\n",
    );

    out.push_str("\n## Limits\n\n");
    if let Some(limits) = packet.get("limits").and_then(Value::as_array) {
        for limit in limits.iter().filter_map(Value::as_str) {
            out.push_str(&format!("- {limit}\n"));
        }
    }
    out
}

pub(super) fn start_here_cli_summary(
    packet: &Value,
    json_path: &Path,
    markdown_path: &Path,
) -> String {
    let selected = packet.get("selected").unwrap_or(&Value::Null);
    let state = string_path(selected, &["state"]).unwrap_or_else(|| "unknown".to_string());
    let mut out = String::new();
    out.push_str(&format!("Start here: {}\n", markdown_path.display()));
    out.push_str(&format!("State: {}\n", cli_state_label(&state)));
    out.push_str(&format!(
        "Output state: {}\n",
        selected_output_state(selected, &state)
    ));
    if let Some(limit) = cli_evidence_limit(selected) {
        out.push_str(&format!("Evidence boundary: {limit}\n"));
    }
    out.push_str(&format!(
        "Safe next action: {}\n",
        cli_safe_next_action(&state, selected)
    ));
    match state.as_str() {
        "top_gap" => {
            if let Some(kind) = string_path(selected, &["kind"]) {
                out.push_str(&format!("Top actionable gap: {}\n", sentence_case(&kind)));
            }
            if let Some(changed) = string_path(selected, &["changed_behavior"]) {
                out.push_str(&format!("Changed behavior: `{}`\n", changed.trim()));
            }
            if let Some(why) = string_path(selected, &["why"]) {
                out.push_str(&format!("Why this matters: {why}\n"));
            }
            if let Some(strength) = string_path(selected, &["current_evidence_strength"]) {
                out.push_str(&format!("Current evidence strength: {strength}\n"));
            }
            if let Some(discriminator) = string_path(selected, &["missing_discriminator"]) {
                out.push_str(&format!("Missing discriminator: {discriminator}\n"));
            }
            if let Some(intent) = string_path(selected, &["focused_proof_intent"]) {
                out.push_str(&format!("Focused proof intent: {intent}\n"));
            }
            if let Some(command) = string_path(selected, &["verify_command"]) {
                out.push_str(&format!("Verify command: `{command}`\n"));
            }
            if let Some(command) = string_path(selected, &["receipt_command"]) {
                out.push_str(&format!("Receipt command: `{command}`\n"));
            }
            out.push_str(&format!(
                "Receipt path: `{}`\n",
                string_path(selected, &["receipt_path"])
                    .unwrap_or_else(|| "not_available".to_string())
            ));
        }
        "missing_artifact" => {
            let artifact = selected.get("artifact").unwrap_or(&Value::Null);
            if let Some(label) = string_path(artifact, &["label"]) {
                let path =
                    string_path(artifact, &["path"]).unwrap_or_else(|| "unknown".to_string());
                out.push_str(&format!("Missing artifact: {label} at `{path}`\n"));
            }
            if let Some(command) = string_path(selected, &["regeneration_command"]) {
                out.push_str(&format!("Regeneration command: `{command}`\n"));
            }
            out.push_str("Receipt path: `not_applicable`\n");
        }
        "empty_diff" | "no_action" => {
            if let Some(reason) = string_path(selected, &["reason"]) {
                out.push_str(&format!("Reason: {reason}\n"));
            }
            out.push_str("Verify command: `not_applicable`\n");
            out.push_str("Receipt command: `not_applicable`\n");
            out.push_str("Receipt path: `not_applicable`\n");
        }
        _ => {
            if let Some(message) = string_path(selected, &["message"]) {
                out.push_str(&format!("Recovery reason: {message}\n"));
            }
            if let Some(command) = string_path(selected, &["next_command"]) {
                out.push_str(&format!("Next command: `{command}`\n"));
            }
            out.push_str("Receipt path: `not_applicable`\n");
        }
    }
    out.push_str(&format!(
        "Artifacts: `{}`, `{}`\n",
        json_path.display(),
        markdown_path.display()
    ));
    out.push_str(&format!("Boundary: {STATIC_EVIDENCE_BOUNDARY}\n"));
    out
}

fn selected_output_state(selected: &Value, state: &str) -> String {
    string_path(selected, &["output_state"])
        .unwrap_or_else(|| expected_output_state_for_selected(state, selected).to_string())
}

pub(super) fn expected_output_state_for_selected(state: &str, selected: &Value) -> &'static str {
    if state == "top_gap"
        && (string_path(selected, &["language_status"]).as_deref() == Some("preview")
            || selected
                .get("static_limit_kind")
                .and_then(Value::as_str)
                .is_some())
    {
        START_HERE_PREVIEW_LIMITED
    } else {
        normalize_start_here_output_state(state)
    }
}

fn cli_state_label(state: &str) -> &'static str {
    match state {
        "top_gap" => "top_gap",
        "missing_artifact" => "missing artifact",
        "malformed_artifact" => "malformed artifact",
        "stale_artifact" => "stale evidence",
        "wrong_root" => "wrong root",
        "timeout" => "timeout partial",
        "empty_diff" => "empty diff",
        "no_action" => "no actionable gap",
        "blocked_artifact" => "blocked artifact",
        _ => "blocked artifact",
    }
}

fn cli_safe_next_action(state: &str, selected: &Value) -> String {
    match state {
        "top_gap" => {
            let identity = string_path(selected, &["canonical_gap_id"])
                .or_else(|| string_path(selected, &["gap_id"]))
                .unwrap_or_else(|| "the selected gap".to_string());
            let intent = string_path(selected, &["focused_proof_intent"])
                .unwrap_or_else(|| "add one focused proof for the selected gap".to_string());
            format!("repair one named gap `{identity}`: {intent}")
        }
        "missing_artifact" => "regenerate the missing artifact before repair work".to_string(),
        "malformed_artifact" => "regenerate the malformed artifact before repair work".to_string(),
        "stale_artifact" => "refresh stale evidence before repair work".to_string(),
        "wrong_root" => "rerun from the matching workspace root before repair work".to_string(),
        "timeout" => "rerun with a bounded refresh command before repair work".to_string(),
        "empty_diff" => {
            "no repair action selected; choose a head with changes or rerun after PR work"
                .to_string()
        }
        "no_action" => {
            "no repair action selected; inspect supporting evidence or rerun after relevant changes"
                .to_string()
        }
        _ => string_path(selected, &["message"]).unwrap_or_else(|| {
            "resolve the blocked start-here state before repair work".to_string()
        }),
    }
}

fn cli_evidence_limit(selected: &Value) -> Option<String> {
    let language = string_path(selected, &["language"]);
    let status = string_path(selected, &["language_status"]);
    let static_limit = string_path(selected, &["static_limit_kind"]);
    let static_detail = string_path(selected, &["static_limit_detail"]);
    if status.as_deref() == Some("preview") {
        let language = language.unwrap_or_else(|| "preview language".to_string());
        let limit = static_limit.unwrap_or_else(|| "preview_limited".to_string());
        return Some(format!(
            "preview-limited evidence for `{language}`; static limit `{limit}` appears before repair language"
        ));
    }
    static_limit.map(|limit| {
        let detail = static_detail
            .map(|detail| format!(" ({detail})"))
            .unwrap_or_default();
        format!("static limit `{limit}`{detail}")
    })
}

fn render_preflight_markdown(packet: &Value, out: &mut String) {
    let Some(preflight) = packet.get("preflight").and_then(Value::as_object) else {
        return;
    };
    out.push_str("\n## Preflight\n\n");
    let status = preflight
        .get("status")
        .and_then(Value::as_str)
        .unwrap_or("unknown");
    let mode = preflight
        .get("mode")
        .and_then(Value::as_str)
        .unwrap_or("unknown");
    out.push_str(&format!("Status: `{status}`\n"));
    out.push_str(&format!("Mode: `{mode}`\n"));
    if let Some(command) = preflight.get("next_command").and_then(Value::as_str) {
        out.push_str(&format!(
            "Next command: {}\n",
            markdown_code_or_text(command)
        ));
    }
    out.push('\n');
    if let Some(checks) = preflight.get("checks").and_then(Value::as_array) {
        for check in checks {
            let label = check
                .get("label")
                .and_then(Value::as_str)
                .unwrap_or("check");
            let status = check
                .get("status")
                .and_then(Value::as_str)
                .unwrap_or("unknown");
            let message = check.get("message").and_then(Value::as_str).unwrap_or("");
            out.push_str(&format!("- {label}: `{status}` - {message}\n"));
        }
    }
}

pub(super) fn markdown_code_or_text(value: &str) -> String {
    if value.contains('`') {
        value.to_string()
    } else {
        format!("`{value}`")
    }
}

fn render_top_gap_markdown(selected: &Value, out: &mut String) {
    out.push_str("## Start Here\n\n");
    out.push_str("- State: `top_gap`\n");
    out.push_str(&format!(
        "- Output state: `{}`\n",
        selected_output_state(selected, "top_gap")
    ));
    out.push_str(&format!(
        "- Safe next action: repair one named {}.\n",
        top_gap_language_label(selected)
    ));
    let kind = selected
        .get("kind")
        .and_then(Value::as_str)
        .unwrap_or("gap");
    out.push_str(&format!("- Top actionable gap: {}\n", sentence_case(kind)));
    if let Some(changed) = selected.get("changed_behavior").and_then(Value::as_str) {
        out.push_str(&format!("- Changed behavior: `{}`\n", changed.trim()));
    }
    if let Some(why) = selected.get("why").and_then(Value::as_str) {
        out.push_str(&format!("- Why this matters: {why}\n"));
    }
    if let Some(strength) = selected
        .get("current_evidence_strength")
        .and_then(Value::as_str)
    {
        out.push_str(&format!("- Current evidence strength: {strength}\n"));
    }
    if let Some(discriminator) = selected
        .get("missing_discriminator")
        .and_then(Value::as_str)
    {
        out.push_str(&format!("- Missing discriminator: {discriminator}\n"));
    }
    if let Some(intent) = selected.get("focused_proof_intent").and_then(Value::as_str) {
        out.push_str(&format!("- Focused proof intent: {intent}\n"));
    }
    if let Some(command) = selected.get("verify_command").and_then(Value::as_str) {
        out.push_str(&format!("- Verify command: `{command}`\n"));
    }
    if let Some(command) = selected.get("receipt_command").and_then(Value::as_str) {
        out.push_str(&format!("- Receipt command: `{command}`\n"));
    }
    if let Some(path) = selected.get("receipt_path").and_then(Value::as_str) {
        out.push_str(&format!("- Receipt path: `{path}`\n"));
    }
    out.push_str(&format!("- Boundary: {STATIC_EVIDENCE_BOUNDARY}\n\n"));
    out.push_str("Evidence boundary:\n");
    if let Some(gap_id) = selected.get("canonical_gap_id").and_then(Value::as_str) {
        out.push_str(&format!("- Canonical gap: `{gap_id}`\n"));
    } else if let Some(gap_id) = selected.get("gap_id").and_then(Value::as_str) {
        out.push_str(&format!("- Gap: `{gap_id}`\n"));
    }
    let language = selected
        .get("language")
        .and_then(Value::as_str)
        .unwrap_or("unknown");
    let language_status = selected
        .get("language_status")
        .and_then(Value::as_str)
        .unwrap_or("unknown");
    out.push_str(&format!("- Language: `{language}` ({language_status})\n"));
    if let Some(limit) = selected.get("static_limit_kind").and_then(Value::as_str) {
        out.push_str(&format!("- Static limit: `{limit}`\n"));
        if let Some(detail) = selected.get("static_limit_detail").and_then(Value::as_str) {
            out.push_str(&format!("  - {detail}\n"));
        }
    }
    let receipt_state = selected
        .get("receipt_state")
        .and_then(Value::as_str)
        .unwrap_or("receipt_missing");
    out.push_str(&format!("- Receipt state: `{receipt_state}`\n\n"));
    if let Some(why) = selected.get("why").and_then(Value::as_str) {
        out.push_str("Why this matters:\n");
        out.push_str(why);
        out.push_str("\n\n");
    }
    if let Some(repair) = selected.get("repair").and_then(Value::as_object) {
        out.push_str("Repair:\n");
        if let Some(route) = repair.get("route").and_then(Value::as_str) {
            out.push_str(&format!("- Route: `{route}`\n"));
        }
        if let Some(target) = repair.get("target_file").and_then(Value::as_str) {
            out.push_str(&format!("- Target: `{target}`\n"));
        }
        if let Some(assertion) = repair.get("suggested_assertion").and_then(Value::as_str) {
            out.push_str(&format!("- Assertion: `{assertion}`\n"));
        }
        out.push('\n');
    }
    if let Some(command) = selected.get("verify_command").and_then(Value::as_str) {
        out.push_str("Verify command:\n");
        out.push_str(&format!("`{command}`\n\n"));
    }
    if let Some(command) = selected.get("receipt_command").and_then(Value::as_str) {
        out.push_str("Receipt command:\n");
        out.push_str(&format!("`{command}`\n\n"));
    }
    if let Some(command) = selected.get("agent_packet_command").and_then(Value::as_str) {
        out.push_str("Agent packet command:\n");
        out.push_str(&format!("`{command}`\n"));
    }
}

fn top_gap_language_label(selected: &Value) -> &'static str {
    match (
        string_path(selected, &["language"]).as_deref(),
        string_path(selected, &["language_status"]).as_deref(),
    ) {
        (Some("python"), Some("preview")) => "preview Python gap",
        (Some("rust"), Some("stable")) => "stable Rust gap",
        _ => "gap",
    }
}

fn render_missing_artifact_markdown(selected: &Value, out: &mut String) {
    out.push_str("## Start Here\n\n");
    out.push_str("- State: `missing_artifact`\n");
    out.push_str(&format!(
        "- Output state: `{}`\n",
        selected_output_state(selected, "missing_artifact")
    ));
    out.push_str(
        "- Safe next action: regenerate the missing artifact before assigning repair work.\n",
    );
    let artifact = selected.get("artifact").unwrap_or(&Value::Null);
    let label = artifact
        .get("label")
        .and_then(Value::as_str)
        .unwrap_or("required artifact");
    let path = artifact.get("path").and_then(Value::as_str).unwrap_or("");
    out.push_str(&format!("- Missing artifact: {label}\n"));
    out.push_str(&format!("- Artifact path: `{path}`\n"));
    if let Some(command) = selected.get("regeneration_command").and_then(Value::as_str) {
        out.push_str(&format!("- Regeneration command: `{command}`\n"));
    }
}

fn render_no_action_markdown(selected: &Value, out: &mut String) {
    out.push_str("## Start Here\n\n");
    let state = selected
        .get("state")
        .and_then(Value::as_str)
        .unwrap_or("no_action");
    out.push_str(&format!("- State: `{state}`\n"));
    out.push_str(&format!(
        "- Output state: `{}`\n",
        selected_output_state(selected, state)
    ));
    out.push_str(
        "- Safe next action: stop on no-action; refresh evidence only after relevant PR changes.\n",
    );
    let reason = selected
        .get("reason")
        .and_then(Value::as_str)
        .unwrap_or("No repairable PR-local Rust gap was selected.");
    out.push_str(&format!("- Reason: {reason}\n"));
    out.push_str("- Boundary: no actionable gap is not runtime, coverage, or mutation adequacy.\n");
}

fn render_blocked_markdown(selected: &Value, out: &mut String) {
    out.push_str("## Start Here\n\n");
    let state = selected
        .get("state")
        .and_then(Value::as_str)
        .unwrap_or("blocked_artifact");
    out.push_str(&format!("- State: `{state}`\n"));
    out.push_str(&format!(
        "- Output state: `{}`\n",
        selected_output_state(selected, state)
    ));
    out.push_str(
        "- Safe next action: resolve this fail-closed state before assigning repair work.\n",
    );
    let message = selected
        .get("message")
        .and_then(Value::as_str)
        .unwrap_or("First-run packet is blocked by unavailable evidence.");
    out.push_str(&format!("- Reason: {message}\n"));
    if let Some(command) = selected.get("next_command").and_then(Value::as_str) {
        out.push_str(&format!("- Next command: `{command}`\n"));
    }
}

fn sentence_case(value: &str) -> String {
    let mut out = String::new();
    for (index, ch) in value.chars().enumerate() {
        if index > 0 && ch.is_uppercase() {
            out.push(' ');
        }
        out.push(ch.to_ascii_lowercase());
    }
    out
}
