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
use std::path::PathBuf;
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
    if let Some(diagnostic) = params
        .context
        .diagnostics
        .iter()
        .find(|d| is_ripr_diagnostic(d) && !is_seam_diagnostic(d))
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

fn push_seam_actions(
    actions: &mut CodeActionResponse,
    params: &CodeActionParams,
    context: SeamActionContext<'_>,
) {
    actions.push(copy_context_action(
        INSPECT_SEAM_PACKET_TITLE,
        INSPECT_SEAM_PACKET_TITLE,
        copy_seam_packet_target(params, context.diagnostic, context.seam),
    ));
    actions.push(copy_targeted_test_brief_action(
        context.seam,
        targeted_test_brief_for_classified_seam(context.seam),
    ));
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
            loop_commands::check_repo_exposure_command(
                COMMAND_ROOT,
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
    if let Some(assertion) = suggested_assertion_for_classified_seam(context.seam) {
        actions.push(copy_suggested_assertion_action(context.seam, assertion));
    }
    if let Some(related) = best_related_test_for_editor(context.seam)
        && let Some(target) = related_test_target(context.snapshot, related)
    {
        actions.push(open_related_test_action(target));
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

const INSPECT_FINDING_CONTEXT_TITLE: &str = "Inspect finding: copy context packet";
const INSPECT_FINDING_CONTEXT_COMMAND_TITLE: &str = "Inspect finding: copy context";
const INSPECT_SEAM_PACKET_TITLE: &str = "Inspect seam: copy packet";
const TARGETED_TEST_BRIEF_TITLE: &str = "Write targeted test: copy brief";
const SUGGESTED_ASSERTION_TITLE: &str = "Write targeted test: copy suggested assertion";
const OPEN_RELATED_TEST_TITLE: &str = "Write targeted test: open best related test";
const AGENT_PACKET_COMMAND_TITLE: &str = "Agent handoff: copy packet command";
const AGENT_BRIEF_COMMAND_TITLE: &str = "Agent handoff: copy brief command";
const AFTER_SNAPSHOT_COMMAND_TITLE: &str = "Verify after test: copy after-snapshot command";
const AGENT_VERIFY_COMMAND_TITLE: &str = "Verify after test: copy verify command";
const AGENT_RECEIPT_COMMAND_TITLE: &str = "Review result: copy receipt command";
const REFRESH_ANALYSIS_TITLE: &str = "Refresh analysis: rerun saved-workspace check";

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
