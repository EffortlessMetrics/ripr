use super::gap_artifacts::{ValidatedGapArtifact, command_payload_is_safe, workspace_path_is_safe};
use super::state::AnalysisSnapshot;
use super::uri::file_uri_for_path;
use super::{
    COPY_AFTER_SNAPSHOT_COMMAND, COPY_AGENT_BRIEF_COMMAND, COPY_AGENT_PACKET_COMMAND,
    COPY_AGENT_RECEIPT_COMMAND, COPY_AGENT_VERIFY_COMMAND, COPY_CONTEXT_COMMAND,
    COPY_SUGGESTED_ASSERTION_COMMAND, COPY_TARGETED_TEST_BRIEF_COMMAND, OPEN_RELATED_TEST_COMMAND,
    REFRESH_COMMAND,
};
use crate::agent::loop_commands;
use crate::analysis::ClassifiedSeam;
use crate::analysis::test_grip_evidence::{RelatedTestGrip, RelationConfidence};
use crate::domain::OracleStrength;
use crate::output::agent_seam_packets::{
    suggested_assertion_for_classified_seam, targeted_test_brief_for_classified_seam,
};
use serde_json::Value;
use std::path::{Path, PathBuf};
use tower_lsp_server::ls_types::{
    CodeAction, CodeActionKind, CodeActionOrCommand, CodeActionParams, CodeActionResponse, Command,
    Diagnostic, LSPAny,
};

pub(super) fn code_action_response(
    params: &CodeActionParams,
    snapshot: Option<&AnalysisSnapshot>,
) -> CodeActionResponse {
    let mut actions = Vec::new();
    if let Some(context) = seam_action_context(params, snapshot) {
        push_seam_actions(&mut actions, params, context);
    }
    if let Some(context) = gap_action_context(params, snapshot) {
        push_gap_actions(&mut actions, params, context);
    }
    if let Some(diagnostic) = params
        .context
        .diagnostics
        .iter()
        .find(|d| is_ripr_diagnostic(d) && !is_seam_diagnostic(d) && !is_gap_diagnostic(d))
    {
        actions.push(copy_context_action(
            INSPECT_FINDING_CONTEXT_TITLE,
            INSPECT_FINDING_CONTEXT_COMMAND_TITLE,
            copy_context_target(params, diagnostic),
        ));
    }
    actions.push(CodeActionOrCommand::CodeAction(CodeAction {
        title: REFRESH_ANALYSIS_TITLE.to_string(),
        kind: Some(CodeActionKind::SOURCE),
        command: Some(Command {
            title: REFRESH_ANALYSIS_TITLE.to_string(),
            command: REFRESH_COMMAND.to_string(),
            arguments: Some(Vec::new()),
        }),
        ..CodeAction::default()
    }));
    actions
}

struct SeamActionContext<'a> {
    diagnostic: &'a Diagnostic,
    seam: &'a ClassifiedSeam,
    snapshot: &'a AnalysisSnapshot,
}

struct GapActionContext<'a> {
    diagnostic: &'a Diagnostic,
    data: &'a Value,
    snapshot: &'a AnalysisSnapshot,
}

fn seam_action_context<'a>(
    params: &'a CodeActionParams,
    snapshot: Option<&'a AnalysisSnapshot>,
) -> Option<SeamActionContext<'a>> {
    let snapshot = snapshot?;
    params
        .context
        .diagnostics
        .iter()
        .filter(|d| is_ripr_diagnostic(d) && is_seam_diagnostic(d))
        .find_map(|diagnostic| {
            snapshot
                .classified_seam_for_diagnostic(diagnostic)
                .map(|seam| SeamActionContext {
                    diagnostic,
                    seam,
                    snapshot,
                })
        })
}

fn gap_action_context<'a>(
    params: &'a CodeActionParams,
    snapshot: Option<&'a AnalysisSnapshot>,
) -> Option<GapActionContext<'a>> {
    let snapshot = snapshot?;
    let diagnostic = params
        .context
        .diagnostics
        .iter()
        .find(|d| is_ripr_diagnostic(d) && is_gap_diagnostic(d))?;
    let data = diagnostic.data.as_ref()?;
    if !snapshot_has_current_gap_diagnostic(params, snapshot, data) {
        return None;
    }
    let _artifact = snapshot.gap_artifacts.iter().find(|artifact| {
        artifact.is_safe_projection_input()
            && artifact.is_actionable_gap()
            && artifact_matches_gap_diagnostic(artifact, data)
    })?;
    Some(GapActionContext {
        diagnostic,
        data,
        snapshot,
    })
}

fn snapshot_has_current_gap_diagnostic(
    params: &CodeActionParams,
    snapshot: &AnalysisSnapshot,
    data: &Value,
) -> bool {
    snapshot
        .diagnostics_for_uri(&params.text_document.uri)
        .is_some_and(|diagnostics| {
            diagnostics.iter().any(|diagnostic| {
                diagnostic.data.as_ref().is_some_and(|current| {
                    is_gap_diagnostic(diagnostic) && gap_identities_overlap(current, data)
                })
            })
        })
}

fn artifact_matches_gap_diagnostic(artifact: &ValidatedGapArtifact, data: &Value) -> bool {
    let canonical_gap_id = string_at(data, &["canonical_gap_id"]);
    let seam_id = string_at(data, &["seam_id"]);
    let finding_id = string_at(data, &["finding_id"]);
    artifact.identities.iter().any(|identity| {
        canonical_gap_id.is_some_and(|value| identity.canonical_gap_id.as_deref() == Some(value))
            || seam_id.is_some_and(|value| identity.seam_id.as_deref() == Some(value))
            || finding_id.is_some_and(|value| identity.finding_id.as_deref() == Some(value))
    })
}

fn gap_identities_overlap(left: &Value, right: &Value) -> bool {
    for key in ["canonical_gap_id", "gap_id", "seam_id", "finding_id"] {
        if let (Some(left), Some(right)) = (string_at(left, &[key]), string_at(right, &[key]))
            && left == right
        {
            return true;
        }
    }
    false
}

fn push_seam_actions(
    actions: &mut CodeActionResponse,
    params: &CodeActionParams,
    context: SeamActionContext<'_>,
) {
    let suggested_assertion = suggested_assertion_for_classified_seam(context.seam);
    let related_test = best_related_test_for_editor(context.seam);
    actions.push(copy_context_action(
        INSPECT_SEAM_PACKET_TITLE,
        INSPECT_SEAM_PACKET_TITLE,
        copy_seam_packet_target(params, context.diagnostic, context.seam),
    ));
    if suggested_assertion.is_some() || related_test.is_some() {
        actions.push(copy_targeted_test_brief_action(
            context.seam,
            targeted_test_brief_for_classified_seam(context.seam),
        ));
    }
    actions.push(copy_agent_loop_command_action(
        AGENT_PACKET_COMMAND_TITLE,
        COPY_AGENT_PACKET_COMMAND,
        agent_loop_command_target(
            context.snapshot,
            context.diagnostic,
            context.seam,
            "agent_packet",
            loop_commands::EDITOR_AGENT_PACKET_ARTIFACT,
            loop_commands::agent_packet_command(
                COMMAND_ROOT,
                context.seam.seam.id().as_str(),
                loop_commands::EDITOR_AGENT_PACKET_ARTIFACT,
            ),
        ),
    ));
    actions.push(copy_agent_loop_command_action(
        AGENT_BRIEF_COMMAND_TITLE,
        COPY_AGENT_BRIEF_COMMAND,
        agent_loop_command_target(
            context.snapshot,
            context.diagnostic,
            context.seam,
            "agent_brief",
            loop_commands::EDITOR_AGENT_BRIEF_ARTIFACT,
            loop_commands::agent_brief_command(
                COMMAND_ROOT,
                context.seam.seam.id().as_str(),
                loop_commands::EDITOR_AGENT_BRIEF_ARTIFACT,
            ),
        ),
    ));
    actions.push(copy_agent_loop_command_action(
        AFTER_SNAPSHOT_COMMAND_TITLE,
        COPY_AFTER_SNAPSHOT_COMMAND,
        agent_loop_command_target(
            context.snapshot,
            context.diagnostic,
            context.seam,
            "after_snapshot",
            loop_commands::PILOT_AFTER_SNAPSHOT_ARTIFACT,
            loop_commands::check_repo_exposure_command_with_base(
                COMMAND_ROOT,
                context.snapshot.base.as_deref(),
                context.snapshot.mode.as_str(),
                loop_commands::PILOT_AFTER_SNAPSHOT_ARTIFACT,
            ),
        ),
    ));
    actions.push(copy_agent_loop_command_action(
        AGENT_VERIFY_COMMAND_TITLE,
        COPY_AGENT_VERIFY_COMMAND,
        agent_loop_command_target(
            context.snapshot,
            context.diagnostic,
            context.seam,
            "agent_verify",
            loop_commands::EDITOR_AGENT_VERIFY_ARTIFACT,
            loop_commands::agent_verify_command(
                COMMAND_ROOT,
                loop_commands::PILOT_BEFORE_SNAPSHOT_ARTIFACT,
                loop_commands::PILOT_AFTER_SNAPSHOT_ARTIFACT,
                Some(loop_commands::EDITOR_AGENT_VERIFY_ARTIFACT),
            ),
        ),
    ));
    actions.push(copy_agent_loop_command_action(
        AGENT_RECEIPT_COMMAND_TITLE,
        COPY_AGENT_RECEIPT_COMMAND,
        agent_loop_command_target(
            context.snapshot,
            context.diagnostic,
            context.seam,
            "agent_receipt",
            loop_commands::EDITOR_AGENT_RECEIPT_ARTIFACT,
            loop_commands::agent_receipt_command(
                COMMAND_ROOT,
                loop_commands::EDITOR_AGENT_VERIFY_ARTIFACT,
                context.seam.seam.id().as_str(),
                Some(loop_commands::EDITOR_AGENT_RECEIPT_ARTIFACT),
            ),
        ),
    ));
    if let Some(assertion) = suggested_assertion {
        actions.push(copy_suggested_assertion_action(context.seam, assertion));
    }
    if let Some(related) = related_test
        && let Some(target) = related_test_target(context.snapshot, related)
    {
        actions.push(open_related_test_action(target));
    }
}

fn push_gap_actions(
    actions: &mut CodeActionResponse,
    params: &CodeActionParams,
    context: GapActionContext<'_>,
) {
    if let Some(target) = first_repair_packet_target(context.snapshot, context.diagnostic) {
        actions.push(copy_context_action(
            COPY_FIRST_REPAIR_PACKET_TITLE,
            COPY_FIRST_REPAIR_PACKET_TITLE,
            target,
        ));
    }
    if let Some(target) = gap_repair_packet_target(params, context.snapshot, context.diagnostic) {
        actions.push(copy_context_action(
            INSPECT_GAP_PACKET_TITLE,
            INSPECT_GAP_PACKET_COMMAND_TITLE,
            target,
        ));
    }
    if let Some(target) = gap_related_test_target(context.snapshot, context.data) {
        actions.push(open_related_test_action(target));
    }
    let verify_command = first_safe_command_at(
        context.snapshot.root.as_path(),
        context.data,
        &["verification_commands"],
    );
    if let Some(command) = &verify_command {
        actions.push(copy_agent_loop_command_action(
            AGENT_VERIFY_COMMAND_TITLE,
            COPY_AGENT_VERIFY_COMMAND,
            gap_command_target(context.diagnostic, "gap_verify", command),
        ));
    }
    if verify_command.is_some()
        && let Some(command) =
            first_safe_receipt_command(context.snapshot.root.as_path(), context.data)
    {
        actions.push(copy_agent_loop_command_action(
            AGENT_RECEIPT_COMMAND_TITLE,
            COPY_AGENT_RECEIPT_COMMAND,
            gap_command_target(context.diagnostic, "gap_receipt", &command),
        ));
    }
    if let Some(target) = static_limit_note_target(context.diagnostic) {
        actions.push(copy_context_action(
            COPY_STATIC_LIMIT_NOTE_TITLE,
            COPY_STATIC_LIMIT_NOTE_TITLE,
            target,
        ));
    }
}

fn copy_context_action(title: &str, command_title: &str, target: LSPAny) -> CodeActionOrCommand {
    CodeActionOrCommand::CodeAction(CodeAction {
        title: title.to_string(),
        kind: Some(CodeActionKind::QUICKFIX),
        command: Some(Command {
            title: command_title.to_string(),
            command: COPY_CONTEXT_COMMAND.to_string(),
            arguments: Some(vec![target]),
        }),
        ..CodeAction::default()
    })
}

const COMMAND_ROOT: &str = ".";

const INSPECT_GAP_PACKET_TITLE: &str = "Inspect gap: copy repair packet";
const INSPECT_GAP_PACKET_COMMAND_TITLE: &str = "Inspect gap: copy context";
const INSPECT_FINDING_CONTEXT_TITLE: &str = "Inspect finding: copy context packet";
const INSPECT_FINDING_CONTEXT_COMMAND_TITLE: &str = "Inspect finding: copy context";
const INSPECT_SEAM_PACKET_TITLE: &str = "Inspect Test Gap - Copy Context";
const TARGETED_TEST_BRIEF_TITLE: &str = "Write targeted test: copy brief";
const SUGGESTED_ASSERTION_TITLE: &str = "Write targeted test: copy suggested assertion";
const OPEN_RELATED_TEST_TITLE: &str = "Write targeted test: open best related test";
const AGENT_PACKET_COMMAND_TITLE: &str = "Agent handoff: copy packet command";
const AGENT_BRIEF_COMMAND_TITLE: &str = "Agent handoff: copy brief command";
const AFTER_SNAPSHOT_COMMAND_TITLE: &str = "Verify after test: copy after-snapshot command";
const AGENT_VERIFY_COMMAND_TITLE: &str = "Verify after test: copy verify command";
const AGENT_RECEIPT_COMMAND_TITLE: &str = "Review result: copy receipt command";
const COPY_STATIC_LIMIT_NOTE_TITLE: &str = "Inspect gap: copy static-limit note";
const COPY_FIRST_REPAIR_PACKET_TITLE: &str = "Copy first repair packet";
const REFRESH_ANALYSIS_TITLE: &str = "Refresh Analysis - Saved Workspace Check";

fn copy_agent_loop_command_action(
    title: &str,
    command: &str,
    target: LSPAny,
) -> CodeActionOrCommand {
    CodeActionOrCommand::CodeAction(CodeAction {
        title: title.to_string(),
        kind: Some(CodeActionKind::QUICKFIX),
        command: Some(Command {
            title: title.to_string(),
            command: command.to_string(),
            arguments: Some(vec![target]),
        }),
        ..CodeAction::default()
    })
}

fn agent_loop_command_target(
    snapshot: &AnalysisSnapshot,
    diagnostic: &Diagnostic,
    seam: &ClassifiedSeam,
    label: &str,
    target_artifact: &str,
    command: String,
) -> LSPAny {
    serde_json::json!({
        "label": label,
        "command": command,
        "root": COMMAND_ROOT,
        "base": snapshot.base.as_deref(),
        "mode": snapshot.mode.as_str(),
        "seam_id": seam.seam.id().as_str(),
        "seam_kind": seam.seam.kind().as_str(),
        "seam_file": seam.seam.file().to_string_lossy(),
        "owner": seam.seam.owner(),
        "line": seam.seam.display_line(),
        "severity": diagnostic.severity.and_then(diagnostic_severity_label),
        "diagnostic_range": {
            "start": {
                "line": diagnostic.range.start.line,
                "character": diagnostic.range.start.character,
            },
            "end": {
                "line": diagnostic.range.end.line,
                "character": diagnostic.range.end.character,
            },
        },
        "target_artifact": target_artifact,
        "before_snapshot": loop_commands::PILOT_BEFORE_SNAPSHOT_ARTIFACT,
        "after_snapshot": loop_commands::PILOT_AFTER_SNAPSHOT_ARTIFACT,
        "agent_packet_json": loop_commands::EDITOR_AGENT_PACKET_ARTIFACT,
        "agent_brief_json": loop_commands::EDITOR_AGENT_BRIEF_ARTIFACT,
        "agent_verify_json": loop_commands::EDITOR_AGENT_VERIFY_ARTIFACT,
        "agent_receipt_json": loop_commands::EDITOR_AGENT_RECEIPT_ARTIFACT,
    })
}

fn diagnostic_severity_label(
    severity: tower_lsp_server::ls_types::DiagnosticSeverity,
) -> Option<&'static str> {
    match severity {
        tower_lsp_server::ls_types::DiagnosticSeverity::ERROR => Some("error"),
        tower_lsp_server::ls_types::DiagnosticSeverity::WARNING => Some("warning"),
        tower_lsp_server::ls_types::DiagnosticSeverity::INFORMATION => Some("information"),
        tower_lsp_server::ls_types::DiagnosticSeverity::HINT => Some("hint"),
        _ => None,
    }
}

fn gap_repair_packet_target(
    params: &CodeActionParams,
    snapshot: &AnalysisSnapshot,
    diagnostic: &Diagnostic,
) -> Option<LSPAny> {
    let data = diagnostic.data.as_ref()?;
    let repair_route = data.get("repair_route")?;
    repair_route.get("route_kind").and_then(non_empty_string)?;
    for key in ["target_file", "related_test"] {
        if let Some(path) = repair_route.get(key).and_then(non_empty_string)
            && !workspace_path_is_safe(snapshot.root.as_path(), path)
        {
            return None;
        }
    }
    let mut target = copy_context_target(params, diagnostic);
    let object = target.as_object_mut()?;
    object.insert(
        "label".to_string(),
        Value::String("gap_repair_packet".to_string()),
    );
    copy_optional_string(object, data, "language");
    copy_optional_string(object, data, "language_status");
    copy_optional_string(object, data, "gap_state");
    copy_optional_string(object, data, "policy_state");
    copy_optional_string(object, data, "repairability");
    copy_optional_string(object, data, "authority_boundary");
    copy_optional_value(object, data, "repair_route");
    let verification_commands =
        safe_commands_at(snapshot.root.as_path(), data, &["verification_commands"]);
    if !verification_commands.is_empty() {
        object.insert(
            "verification_commands".to_string(),
            serde_json::json!(verification_commands),
        );
    }
    let regeneration_commands =
        safe_commands_at(snapshot.root.as_path(), data, &["regeneration_commands"]);
    if !regeneration_commands.is_empty() {
        object.insert(
            "regeneration_commands".to_string(),
            serde_json::json!(regeneration_commands),
        );
    }
    if let Some(command) = first_safe_receipt_command(snapshot.root.as_path(), data) {
        object.insert("receipt_command".to_string(), Value::String(command));
    }
    copy_optional_value(object, data, "receipt");
    if let Some(note) = static_limit_note(data) {
        object.insert("static_limit_note".to_string(), Value::String(note));
    }
    object.insert(
        "limits_note".to_string(),
        Value::String(
            "Static evidence only; no source edits, generated tests, provider calls, or runtime mutation execution."
                .to_string(),
        ),
    );
    Some(target)
}

fn first_repair_packet_target(
    snapshot: &AnalysisSnapshot,
    diagnostic: &Diagnostic,
) -> Option<LSPAny> {
    let data = diagnostic.data.as_ref()?;
    let gap_identity = first_gap_identity(data)?;
    let repair_route = data.get("repair_route")?;
    repair_route.get("route_kind").and_then(non_empty_string)?;
    for key in ["target_file", "related_test"] {
        if let Some(path) = repair_route.get(key).and_then(non_empty_string)
            && !workspace_path_is_safe(snapshot.root.as_path(), path)
        {
            return None;
        }
    }
    let verify_command =
        first_safe_command_at(snapshot.root.as_path(), data, &["verification_commands"])?;
    let receipt_command = first_safe_receipt_command(snapshot.root.as_path(), data)?;
    let packet = first_repair_packet_text(data, repair_route, &verify_command, &receipt_command)?;
    let mut target = serde_json::Map::new();
    target.insert(
        "label".to_string(),
        Value::String("first_repair_packet".to_string()),
    );
    target.insert("packet".to_string(), Value::String(packet));
    target.insert(
        "gap_identity".to_string(),
        Value::String(gap_identity.to_string()),
    );
    copy_optional_string(&mut target, data, "gap_id");
    copy_optional_string(&mut target, data, "canonical_gap_id");
    copy_optional_string(&mut target, data, "seam_id");
    copy_optional_string(&mut target, data, "finding_id");
    copy_optional_string(&mut target, data, "language");
    copy_optional_string(&mut target, data, "language_status");
    copy_optional_string(&mut target, data, "gap_state");
    copy_optional_value(&mut target, data, "repair_route");
    target.insert("verify_command".to_string(), Value::String(verify_command));
    target.insert(
        "receipt_command".to_string(),
        Value::String(receipt_command),
    );
    Some(Value::Object(target))
}

fn first_gap_identity(data: &Value) -> Option<&str> {
    ["canonical_gap_id", "gap_id", "seam_id", "finding_id"]
        .iter()
        .find_map(|key| string_at(data, &[*key]))
}

fn first_repair_packet_text(
    data: &Value,
    repair_route: &Value,
    verify_command: &str,
    receipt_command: &str,
) -> Option<String> {
    let gap_identity = first_gap_identity(data)?;
    let route_kind = repair_route.get("route_kind").and_then(non_empty_string)?;
    let mut lines = vec![
        "RIPR first repair packet".to_string(),
        String::new(),
        format!("Gap identity: {gap_identity}"),
    ];
    if let Some(language) = string_at(data, &["language"]) {
        lines.push(format!("Language: {language}"));
    }
    if let Some(status) = string_at(data, &["language_status"]) {
        lines.push(format!("Language status: {status}"));
    }
    if let Some(state) = string_at(data, &["gap_state"]) {
        lines.push(format!("Gap state: {state}"));
    }
    if let Some(note) = static_limit_note(data) {
        lines.push(String::new());
        lines.push(note);
    }
    lines.push(String::new());
    lines.push("Suggested action:".to_string());
    lines.push(format!("- Route: {route_kind}"));
    if let Some(changed_behavior) = repair_route
        .get("changed_behavior")
        .and_then(non_empty_string)
    {
        lines.push(format!("- Changed behavior: {changed_behavior}"));
    }
    if let Some(assertion_shape) = repair_route
        .get("assertion_shape")
        .and_then(non_empty_string)
    {
        lines.push(format!(
            "- Add or strengthen one focused assertion: {assertion_shape}"
        ));
    }
    if let Some(related_test) = repair_route.get("related_test").and_then(non_empty_string) {
        lines.push(format!("- Related test: {related_test}"));
    } else if let Some(target_file) = repair_route.get("target_file").and_then(non_empty_string) {
        lines.push(format!("- Repair target: {target_file}"));
    }
    if let Some(items) = repair_route
        .get("stop_conditions")
        .and_then(Value::as_array)
        .filter(|items| !items.is_empty())
    {
        lines.push("- Stop conditions:".to_string());
        for item in items {
            if let Some(text) = item.as_str().map(str::trim).filter(|text| !text.is_empty()) {
                lines.push(format!("  - {text}"));
            }
        }
    }
    lines.push(String::new());
    lines.push("Verify command:".to_string());
    lines.push(verify_command.to_string());
    lines.push(String::new());
    lines.push("Receipt command:".to_string());
    lines.push(receipt_command.to_string());
    lines.push(String::new());
    lines.push("Limits and non-claims:".to_string());
    lines.push("- Static editor evidence only.".to_string());
    lines.push("- Advisory by default; no gate eligibility or runtime adequacy claim.".to_string());
    lines.push("- Do not edit production code unless the packet explicitly scopes it.".to_string());
    lines.push(
        "- Do not generate tests, call providers, or run mutation execution from the editor."
            .to_string(),
    );
    Some(lines.join("\n"))
}

fn copy_optional_string(object: &mut serde_json::Map<String, Value>, data: &Value, key: &str) {
    if let Some(value) = data.get(key).and_then(non_empty_string) {
        object.insert(key.to_string(), Value::String(value.to_string()));
    }
}

fn copy_optional_value(object: &mut serde_json::Map<String, Value>, data: &Value, key: &str) {
    if let Some(value) = data.get(key) {
        object.insert(key.to_string(), value.clone());
    }
}

fn gap_command_target(diagnostic: &Diagnostic, label: &str, command: &str) -> LSPAny {
    let mut target = serde_json::json!({
        "label": label,
        "command": command,
        "root": COMMAND_ROOT,
    });
    if let Some(data) = &diagnostic.data
        && let Some(object) = target.as_object_mut()
    {
        for key in [
            "gap_id",
            "canonical_gap_id",
            "gap_kind",
            "language",
            "language_status",
            "gap_state",
            "policy_state",
            "repairability",
        ] {
            copy_optional_string(object, data, key);
        }
    }
    target
}

fn first_safe_command_at(root: &Path, data: &Value, path: &[&str]) -> Option<String> {
    safe_commands_at(root, data, path).into_iter().next()
}

fn safe_commands_at(root: &Path, data: &Value, path: &[&str]) -> Vec<String> {
    value_at(data, path)
        .and_then(Value::as_array)
        .into_iter()
        .flat_map(|items| items.iter())
        .filter_map(Value::as_str)
        .map(str::trim)
        .filter(|command| command_payload_is_safe(root, command))
        .map(ToOwned::to_owned)
        .collect()
}

fn first_safe_receipt_command(root: &Path, data: &Value) -> Option<String> {
    [
        &["receipt_command"][..],
        &["commands", "receipt"][..],
        &["receipt", "command"][..],
    ]
    .iter()
    .filter_map(|path| string_at(data, path))
    .find(|command| command_payload_is_safe(root, command))
    .map(ToOwned::to_owned)
}

fn gap_related_test_target(snapshot: &AnalysisSnapshot, data: &Value) -> Option<LSPAny> {
    let route = data.get("repair_route")?;
    let related_test = route
        .get("related_test")
        .and_then(non_empty_string)
        .or_else(|| route.get("target_file").and_then(non_empty_string))?;
    if !workspace_path_is_safe(snapshot.root.as_path(), related_test) {
        return None;
    }
    let file = related_test_path_part(related_test);
    if !path_matches_diagnostic_language(data, file) {
        return None;
    }
    let absolute = if Path::new(file).is_absolute() {
        PathBuf::from(file)
    } else {
        snapshot.root.join(file)
    };
    if !absolute.is_file() {
        return None;
    }
    let uri = file_uri_for_path(&absolute).ok()?;
    let line = route
        .get("target_line")
        .and_then(Value::as_u64)
        .unwrap_or(1);
    Some(serde_json::json!({
        "uri": uri.as_str(),
        "line": line,
        "test_name": related_test_name(related_test),
    }))
}

fn related_test_path_part(raw: &str) -> &str {
    raw.split_once("::").map_or(raw, |(path, _)| path).trim()
}

fn related_test_name(raw: &str) -> &str {
    raw.split_once("::").map_or("", |(_, name)| name).trim()
}

fn path_matches_diagnostic_language(data: &Value, path: &str) -> bool {
    match string_at(data, &["language"]) {
        Some("rust") => path.ends_with(".rs"),
        Some("python") => path.ends_with(".py"),
        Some("typescript") => path.ends_with(".ts") || path.ends_with(".tsx"),
        Some("javascript") => path.ends_with(".js") || path.ends_with(".jsx"),
        _ => false,
    }
}

fn static_limit_note_target(diagnostic: &Diagnostic) -> Option<LSPAny> {
    let data = diagnostic.data.as_ref()?;
    let note = static_limit_note(data)?;
    Some(serde_json::json!({
        "label": "static_limit_note",
        "gap_id": string_at(data, &["gap_id"]),
        "canonical_gap_id": string_at(data, &["canonical_gap_id"]),
        "language": string_at(data, &["language"]),
        "language_status": string_at(data, &["language_status"]),
        "note": note,
        "limits_note": "Static evidence only; no runtime adequacy claim.",
    }))
}

fn static_limit_note(data: &Value) -> Option<String> {
    let mut lines = Vec::new();
    if let Some(kind) = string_at(data, &["static_limit_kind"]) {
        lines.push(format!("Static limit: {kind}"));
    }
    if let Some(detail) = string_at(data, &["static_limit_detail"]) {
        lines.push(format!("Detail: {detail}"));
    }
    if let Some(items) = value_at(data, &["static_limits"]).and_then(Value::as_array) {
        for item in items {
            if let Some(kind) = string_at(item, &["static_limit_kind"]) {
                lines.push(format!("Static limit: {kind}"));
            }
            if let Some(detail) = string_at(item, &["static_limit_detail"]) {
                lines.push(format!("Detail: {detail}"));
            }
        }
    }
    if lines.is_empty() {
        None
    } else {
        lines.push("Boundary: static evidence only; advisory action.".to_string());
        Some(lines.join("\n"))
    }
}

fn string_at<'a>(value: &'a Value, path: &[&str]) -> Option<&'a str> {
    value_at(value, path).and_then(non_empty_string)
}

fn value_at<'a>(value: &'a Value, path: &[&str]) -> Option<&'a Value> {
    let mut current = value;
    for key in path {
        current = current.get(*key)?;
    }
    Some(current)
}

fn non_empty_string(value: &Value) -> Option<&str> {
    let text = value.as_str()?.trim();
    if text.is_empty() { None } else { Some(text) }
}

fn copy_targeted_test_brief_action(seam: &ClassifiedSeam, brief: String) -> CodeActionOrCommand {
    CodeActionOrCommand::CodeAction(CodeAction {
        title: TARGETED_TEST_BRIEF_TITLE.to_string(),
        kind: Some(CodeActionKind::QUICKFIX),
        command: Some(Command {
            title: TARGETED_TEST_BRIEF_TITLE.to_string(),
            command: COPY_TARGETED_TEST_BRIEF_COMMAND.to_string(),
            arguments: Some(vec![serde_json::json!({
                "seam_id": seam.seam.id().as_str(),
                "brief": brief,
            })]),
        }),
        ..CodeAction::default()
    })
}

fn copy_suggested_assertion_action(
    seam: &ClassifiedSeam,
    assertion: String,
) -> CodeActionOrCommand {
    CodeActionOrCommand::CodeAction(CodeAction {
        title: SUGGESTED_ASSERTION_TITLE.to_string(),
        kind: Some(CodeActionKind::QUICKFIX),
        command: Some(Command {
            title: SUGGESTED_ASSERTION_TITLE.to_string(),
            command: COPY_SUGGESTED_ASSERTION_COMMAND.to_string(),
            arguments: Some(vec![serde_json::json!({
                "seam_id": seam.seam.id().as_str(),
                "assertion": assertion,
            })]),
        }),
        ..CodeAction::default()
    })
}

fn open_related_test_action(target: LSPAny) -> CodeActionOrCommand {
    CodeActionOrCommand::CodeAction(CodeAction {
        title: OPEN_RELATED_TEST_TITLE.to_string(),
        kind: Some(CodeActionKind::QUICKFIX),
        command: Some(Command {
            title: OPEN_RELATED_TEST_TITLE.to_string(),
            command: OPEN_RELATED_TEST_COMMAND.to_string(),
            arguments: Some(vec![target]),
        }),
        ..CodeAction::default()
    })
}

fn is_ripr_diagnostic(diagnostic: &Diagnostic) -> bool {
    diagnostic.source.as_deref() == Some("ripr")
}

fn is_seam_diagnostic(diagnostic: &Diagnostic) -> bool {
    diagnostic
        .data
        .as_ref()
        .and_then(|data| data.get("seam_id"))
        .and_then(|value| value.as_str())
        .is_some()
}

fn is_gap_diagnostic(diagnostic: &Diagnostic) -> bool {
    diagnostic
        .data
        .as_ref()
        .and_then(|data| data.get("gap_id"))
        .and_then(|value| value.as_str())
        .is_some()
}

fn copy_context_target(params: &CodeActionParams, diagnostic: &Diagnostic) -> LSPAny {
    let mut target = serde_json::Map::new();
    target.insert(
        "uri".to_string(),
        serde_json::Value::String(params.text_document.uri.as_str().to_string()),
    );
    target.insert(
        "line".to_string(),
        serde_json::Value::Number(serde_json::Number::from(
            params.range.start.line.saturating_add(1),
        )),
    );
    if let Some(data) = &diagnostic.data
        && let Some(obj) = data.as_object()
    {
        if let Some(finding_id) = obj.get("finding_id").and_then(|v| v.as_str()) {
            target.insert(
                "finding_id".to_string(),
                serde_json::Value::String(finding_id.to_string()),
            );
        }
        if let Some(probe_id) = obj.get("probe_id").and_then(|v| v.as_str()) {
            target.insert(
                "probe_id".to_string(),
                serde_json::Value::String(probe_id.to_string()),
            );
        }
        if let Some(seam_id) = obj.get("seam_id").and_then(|v| v.as_str()) {
            target.insert(
                "seam_id".to_string(),
                serde_json::Value::String(seam_id.to_string()),
            );
        }
        if let Some(seam_kind) = obj.get("seam_kind").and_then(|v| v.as_str()) {
            target.insert(
                "seam_kind".to_string(),
                serde_json::Value::String(seam_kind.to_string()),
            );
        }
        for key in ["gap_id", "canonical_gap_id", "gap_kind", "gap_ledger"] {
            if let Some(value) = obj.get(key).and_then(|v| v.as_str()) {
                target.insert(
                    key.to_string(),
                    serde_json::Value::String(value.to_string()),
                );
            }
        }
    }
    serde_json::Value::Object(target)
}

fn copy_seam_packet_target(
    params: &CodeActionParams,
    diagnostic: &Diagnostic,
    seam: &ClassifiedSeam,
) -> LSPAny {
    let mut target = copy_context_target(params, diagnostic);
    if let Some(obj) = target.as_object_mut() {
        obj.insert(
            "line".to_string(),
            serde_json::Value::Number(serde_json::Number::from(seam.seam.display_line())),
        );
        obj.insert(
            "seam_id".to_string(),
            serde_json::Value::String(seam.seam.id().as_str().to_string()),
        );
        obj.insert(
            "seam_kind".to_string(),
            serde_json::Value::String(seam.seam.kind().as_str().to_string()),
        );
    }
    target
}

fn best_related_test_for_editor(seam: &ClassifiedSeam) -> Option<&RelatedTestGrip> {
    seam.evidence
        .related_tests
        .iter()
        .find(|test| test.oracle_strength == OracleStrength::Strong)
        .or_else(|| {
            seam.evidence
                .related_tests
                .iter()
                .min_by_key(|test| relation_confidence_rank(test.relation_confidence))
        })
}

fn relation_confidence_rank(confidence: RelationConfidence) -> u8 {
    match confidence {
        RelationConfidence::High => 0,
        RelationConfidence::Medium => 1,
        RelationConfidence::Low => 2,
        RelationConfidence::Opaque => 3,
    }
}

fn related_test_target(snapshot: &AnalysisSnapshot, related: &RelatedTestGrip) -> Option<LSPAny> {
    let path = absolute_related_test_path(snapshot, related);
    let uri = file_uri_for_path(&path).ok()?;
    Some(serde_json::json!({
        "uri": uri.as_str(),
        "line": related.line,
        "test_name": related.test_name.as_str(),
    }))
}

fn absolute_related_test_path(snapshot: &AnalysisSnapshot, related: &RelatedTestGrip) -> PathBuf {
    if related.file.is_absolute() {
        related.file.clone()
    } else {
        snapshot.root.join(&related.file)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tower_lsp_server::ls_types::{
        CodeActionContext, DiagnosticSeverity, Position, Range, TextDocumentIdentifier, Uri,
    };

    #[test]
    fn gap_diagnostic_without_snapshot_gets_refresh_only() -> Result<(), String> {
        let diagnostic = gap_diagnostic();
        let params = code_action_params(vec![diagnostic])?;

        let actions = code_action_response(&params, None);
        let titles = action_titles(&actions);

        assert_eq!(titles, vec![REFRESH_ANALYSIS_TITLE]);
        Ok(())
    }

    #[test]
    fn copy_context_target_forwards_gap_identity_and_ledger() -> Result<(), String> {
        let diagnostic = gap_diagnostic();
        let params = code_action_params(vec![diagnostic.clone()])?;

        let target = copy_context_target(&params, &diagnostic);

        assert_eq!(
            target["uri"], "file:///workspace/src/pricing.rs",
            "target URI should match request URI"
        );
        assert_eq!(target["line"], 12);
        assert_eq!(target["gap_id"], "gap:pr:pricing:threshold-boundary");
        assert_eq!(
            target["canonical_gap_id"],
            "gap:rust:pricing:threshold-boundary"
        );
        assert_eq!(target["gap_kind"], "MissingBoundaryAssertion");
        assert_eq!(
            target["gap_ledger"],
            "target/ripr/reports/gap-decision-ledger.json"
        );
        Ok(())
    }

    fn code_action_params(diagnostics: Vec<Diagnostic>) -> Result<CodeActionParams, String> {
        Ok(CodeActionParams {
            text_document: TextDocumentIdentifier::new(test_uri(
                "file:///workspace/src/pricing.rs",
            )?),
            range: Range {
                start: Position {
                    line: 11,
                    character: 0,
                },
                end: Position {
                    line: 11,
                    character: 120,
                },
            },
            context: CodeActionContext {
                diagnostics,
                only: None,
                trigger_kind: None,
            },
            work_done_progress_params: Default::default(),
            partial_result_params: Default::default(),
        })
    }

    fn gap_diagnostic() -> Diagnostic {
        Diagnostic {
            range: Range {
                start: Position {
                    line: 11,
                    character: 0,
                },
                end: Position {
                    line: 11,
                    character: 120,
                },
            },
            severity: Some(DiagnosticSeverity::WARNING),
            code: None,
            code_description: None,
            source: Some("ripr".to_string()),
            message: "ripr gap: MissingBoundaryAssertion".to_string(),
            related_information: None,
            tags: None,
            data: Some(serde_json::json!({
                "source": "gap_decision_ledger",
                "gap_id": "gap:pr:pricing:threshold-boundary",
                "canonical_gap_id": "gap:rust:pricing:threshold-boundary",
                "gap_kind": "MissingBoundaryAssertion",
                "gap_ledger": "target/ripr/reports/gap-decision-ledger.json"
            })),
        }
    }

    fn action_titles(actions: &[CodeActionOrCommand]) -> Vec<&str> {
        actions
            .iter()
            .map(|action| match action {
                CodeActionOrCommand::CodeAction(action) => action.title.as_str(),
                CodeActionOrCommand::Command(command) => command.title.as_str(),
            })
            .collect()
    }

    fn test_uri(uri: &str) -> Result<Uri, String> {
        uri.parse::<Uri>()
            .map_err(|err| format!("failed to parse test URI: {err}"))
    }
}
