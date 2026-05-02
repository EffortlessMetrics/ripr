use std::io::BufReader;

mod diagnostics;
mod protocol;
mod responses;

use diagnostics::publish_workspace_diagnostics;
use protocol::{extract_id, read_lsp_message, write_lsp_message};
use responses::{code_action_response, hover_response, initialize_response};

pub fn serve() -> Result<(), String> {
    let stdin = std::io::stdin();
    let stdout = std::io::stdout();
    let mut reader = BufReader::new(stdin.lock());
    let mut writer = stdout.lock();

    while let Some(message) = read_lsp_message(&mut reader)? {
        if message.contains("\"method\":\"initialize\"")
            || message.contains("\"method\": \"initialize\"")
        {
            let id = extract_id(&message).unwrap_or_else(|| "1".to_string());
            write_lsp_message(&mut writer, &initialize_response(&id))?;
        } else if message.contains("\"method\":\"shutdown\"")
            || message.contains("\"method\": \"shutdown\"")
        {
            let id = extract_id(&message).unwrap_or_else(|| "1".to_string());
            write_lsp_message(
                &mut writer,
                &format!(r#"{{\"jsonrpc\":\"2.0\",\"id\":{id},\"result\":null}}"#),
            )?;
        } else if message.contains("\"method\":\"exit\"")
            || message.contains("\"method\": \"exit\"")
        {
            break;
        } else if message.contains("textDocument/didOpen")
            || message.contains("textDocument/didChange")
            || message.contains("textDocument/didSave")
            || message.contains("ripr.refresh")
        {
            publish_workspace_diagnostics(&mut writer)?;
        } else if message.contains("\"method\":\"textDocument/hover\"")
            || message.contains("\"method\": \"textDocument/hover\"")
        {
            let id = extract_id(&message).unwrap_or_else(|| "1".to_string());
            write_lsp_message(&mut writer, &hover_response(&id))?;
        } else if message.contains("\"method\":\"textDocument/codeAction\"")
            || message.contains("\"method\": \"textDocument/codeAction\"")
        {
            let id = extract_id(&message).unwrap_or_else(|| "1".to_string());
            write_lsp_message(&mut writer, &code_action_response(&id))?;
        } else if let Some(id) = extract_id(&message) {
            write_lsp_message(
                &mut writer,
                &format!(r#"{{\"jsonrpc\":\"2.0\",\"id\":{id},\"result\":null}}"#),
            )?;
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests;
