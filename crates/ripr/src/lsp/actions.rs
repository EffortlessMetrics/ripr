use super::{COPY_CONTEXT_COMMAND, REFRESH_COMMAND};
use tower_lsp_server::ls_types::{
    CodeAction, CodeActionKind, CodeActionOrCommand, CodeActionResponse, Command,
};

pub(super) fn code_action_response() -> CodeActionResponse {
    vec![
        CodeActionOrCommand::CodeAction(CodeAction {
            title: "Copy ripr context packet".to_string(),
            kind: Some(CodeActionKind::QUICKFIX),
            command: Some(Command {
                title: "Copy ripr context".to_string(),
                command: COPY_CONTEXT_COMMAND.to_string(),
                arguments: Some(Vec::new()),
            }),
            ..CodeAction::default()
        }),
        CodeActionOrCommand::CodeAction(CodeAction {
            title: "Run ripr check".to_string(),
            kind: Some(CodeActionKind::SOURCE),
            command: Some(Command {
                title: "Refresh ripr analysis".to_string(),
                command: REFRESH_COMMAND.to_string(),
                arguments: Some(Vec::new()),
            }),
            ..CodeAction::default()
        }),
    ]
}
