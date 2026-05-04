use super::{COPY_CONTEXT_COMMAND, REFRESH_COMMAND};
use tower_lsp_server::ls_types::{
    CodeAction, CodeActionKind, CodeActionOrCommand, CodeActionParams, CodeActionResponse, Command,
    Diagnostic, LSPAny,
};

pub(super) fn code_action_response(params: &CodeActionParams) -> CodeActionResponse {
    let mut actions = Vec::new();
    if let Some(diagnostic) = params
        .context
        .diagnostics
        .iter()
        .find(|d| is_ripr_diagnostic(d))
    {
        actions.push(CodeActionOrCommand::CodeAction(CodeAction {
            title: "Copy ripr context packet".to_string(),
            kind: Some(CodeActionKind::QUICKFIX),
            command: Some(Command {
                title: "Copy ripr context".to_string(),
                command: COPY_CONTEXT_COMMAND.to_string(),
                arguments: Some(vec![copy_context_target(params, diagnostic)]),
            }),
            ..CodeAction::default()
        }));
    }
    actions.push(CodeActionOrCommand::CodeAction(CodeAction {
        title: "Run ripr check".to_string(),
        kind: Some(CodeActionKind::SOURCE),
        command: Some(Command {
            title: "Refresh ripr analysis".to_string(),
            command: REFRESH_COMMAND.to_string(),
            arguments: Some(Vec::new()),
        }),
        ..CodeAction::default()
    }));
    actions
}

fn is_ripr_diagnostic(diagnostic: &Diagnostic) -> bool {
    diagnostic.source.as_deref() == Some("ripr")
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
    }
    serde_json::Value::Object(target)
}
