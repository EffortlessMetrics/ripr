mod diagnostics;
mod dispatch;
mod protocol;

use dispatch::MessageAction;
use protocol::{extract_id, read_lsp_message, write_lsp_message};
use std::io::BufReader;

pub fn serve() -> Result<(), String> {
    let stdin = std::io::stdin();
    let stdout = std::io::stdout();
    let mut reader = BufReader::new(stdin.lock());
    let mut writer = stdout.lock();

    while let Some(message) = read_lsp_message(&mut reader)? {
        match dispatch::route_message(&message) {
            MessageAction::Respond(body) => write_lsp_message(&mut writer, &body)?,
            MessageAction::PublishDiagnostics => {
                diagnostics::publish_workspace_diagnostics(&mut writer)?
            }
            MessageAction::Acknowledge => {
                if let Some(id) = extract_id(&message) {
                    write_lsp_message(
                        &mut writer,
                        &format!(r#"{{"jsonrpc":"2.0","id":{id},"result":null}}"#),
                    )?;
                }
            }
            MessageAction::Exit => break,
            MessageAction::Ignore => {}
        }
    }

    Ok(())
}

fn lsp_message(finding: &crate::domain::Finding) -> String {
    crate::lsp::protocol::json_string(
        &finding
            .recommended_next_step
            .clone()
            .unwrap_or_else(|| format!("{} static RIPR exposure", finding.class.as_str())),
    )
}
