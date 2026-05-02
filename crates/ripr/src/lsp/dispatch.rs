use super::protocol::extract_id;

pub enum MessageAction {
    Respond(String),
    PublishDiagnostics,
    Acknowledge,
    Exit,
    Ignore,
}

pub fn route_message(message: &str) -> MessageAction {
    if is_initialize(message) {
        let id = extract_id(message).unwrap_or_else(|| "1".to_string());
        return MessageAction::Respond(initialize_response(&id));
    }
    if is_shutdown(message) {
        let id = extract_id(message).unwrap_or_else(|| "1".to_string());
        return MessageAction::Respond(format!(r#"{{"jsonrpc":"2.0","id":{id},"result":null}}"#));
    }
    if is_exit(message) {
        return MessageAction::Exit;
    }
    if should_refresh_diagnostics(message) {
        return MessageAction::PublishDiagnostics;
    }
    if is_hover(message) {
        let id = extract_id(message).unwrap_or_else(|| "1".to_string());
        return MessageAction::Respond(hover_response(&id));
    }
    if is_code_action(message) {
        let id = extract_id(message).unwrap_or_else(|| "1".to_string());
        return MessageAction::Respond(code_action_response(&id));
    }
    if extract_id(message).is_some() {
        return MessageAction::Acknowledge;
    }
    MessageAction::Ignore
}

fn is_initialize(message: &str) -> bool {
    message.contains("\"method\":\"initialize\"") || message.contains("\"method\": \"initialize\"")
}

fn is_shutdown(message: &str) -> bool {
    message.contains("\"method\":\"shutdown\"") || message.contains("\"method\": \"shutdown\"")
}

fn is_exit(message: &str) -> bool {
    message.contains("\"method\":\"exit\"") || message.contains("\"method\": \"exit\"")
}

fn should_refresh_diagnostics(message: &str) -> bool {
    message.contains("textDocument/didOpen")
        || message.contains("textDocument/didChange")
        || message.contains("textDocument/didSave")
        || message.contains("ripr.refresh")
}

fn is_hover(message: &str) -> bool {
    message.contains("\"method\":\"textDocument/hover\"")
        || message.contains("\"method\": \"textDocument/hover\"")
}

fn is_code_action(message: &str) -> bool {
    message.contains("\"method\":\"textDocument/codeAction\"")
        || message.contains("\"method\": \"textDocument/codeAction\"")
}

fn initialize_response(id: &str) -> String {
    format!(
        r#"{{"jsonrpc":"2.0","id":{id},"result":{{"capabilities":{{"textDocumentSync":1,"hoverProvider":true,"codeActionProvider":true,"executeCommandProvider":{{"commands":["ripr.collectContext","ripr.refresh"]}}}},"serverInfo":{{"name":"ripr","version":"{}"}}}}}}"#,
        env!("CARGO_PKG_VERSION")
    )
}

fn hover_response(id: &str) -> String {
    format!(
        r#"{{"jsonrpc":"2.0","id":{id},"result":{{"contents":{{"kind":"markdown","value":"ripr estimates static RIPR exposure for changed Rust behavior. Run `ripr check --format json` for current findings."}}}}}}"#
    )
}

fn code_action_response(id: &str) -> String {
    format!(
        r#"{{"jsonrpc":"2.0","id":{id},"result":[{{"title":"Copy ripr context packet","kind":"quickfix","command":{{"title":"Collect ripr context","command":"ripr.collectContext","arguments":[]}}}},{{"title":"Run ripr check","kind":"source","command":{{"title":"Refresh ripr analysis","command":"ripr.refresh","arguments":[]}}}}]}}"#
    )
}
