use super::{COPY_CONTEXT_COMMAND, REFRESH_COMMAND};
use tower_lsp_server::ls_types::{
    CodeAction, CodeActionKind, CodeActionOrCommand, CodeActionParams, CodeActionResponse, Command,
    Diagnostic, LSPAny,
};

pub(super) fn code_action_response(params: &CodeActionParams) -> CodeActionResponse {
    let mut actions = Vec::new();
    if params.context.diagnostics.iter().any(is_ripr_diagnostic) {
        actions.push(CodeActionOrCommand::CodeAction(CodeAction {
            title: "Copy ripr context packet".to_string(),
            kind: Some(CodeActionKind::QUICKFIX),
            command: Some(Command {
                title: "Copy ripr context".to_string(),
                command: COPY_CONTEXT_COMMAND.to_string(),
                arguments: Some(vec![copy_context_target(params)]),
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

fn copy_context_target(params: &CodeActionParams) -> LSPAny {
    serde_json::json!({
        "uri": params.text_document.uri.as_str(),
        "line": params.range.start.line.saturating_add(1),
    })
}
