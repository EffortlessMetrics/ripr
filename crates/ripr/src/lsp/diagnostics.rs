use crate::app::{CheckInput, OutputFormat, check_workspace};
use std::io::Write;

pub fn publish_workspace_diagnostics(writer: &mut impl Write) -> Result<(), String> {
    let input = CheckInput {
        root: std::env::current_dir().map_err(|err| format!("failed to get current dir: {err}"))?,
        format: OutputFormat::Json,
        ..CheckInput::default()
    };
    let output = match check_workspace(input) {
        Ok(output) => output,
        Err(_) => return Ok(()),
    };
    let mut grouped: std::collections::BTreeMap<String, Vec<&crate::domain::Finding>> =
        std::collections::BTreeMap::new();
    for finding in &output.findings {
        grouped
            .entry(format!("file://{}", finding.probe.location.file.display()))
            .or_default()
            .push(finding);
    }
    for (uri, findings) in grouped {
        let mut diagnostics = String::new();
        diagnostics.push('[');
        for (idx, finding) in findings.iter().enumerate() {
            let line = finding.probe.location.line.saturating_sub(1);
            diagnostics.push_str(&format!(
                r#"{{"range":{{"start":{{"line":{line},"character":0}},"end":{{"line":{line},"character":120}}}},"severity":2,"source":"ripr","code":{},"message":{},"data":{{"probeId":{},"class":{},"family":{},"confidence":{:.2}}}}}"#,
                super::protocol::json_string(finding.class.as_str()),
                super::lsp_message(finding),
                super::protocol::json_string(&finding.id),
                super::protocol::json_string(finding.class.as_str()),
                super::protocol::json_string(finding.probe.family.as_str()),
                finding.confidence,
            ));
            if idx + 1 != findings.len() {
                diagnostics.push(',');
            }
        }
        diagnostics.push(']');
        let notif = format!(
            r#"{{"jsonrpc":"2.0","method":"textDocument/publishDiagnostics","params":{{"uri":{},"diagnostics":{diagnostics}}}}}"#,
            super::protocol::json_string(&uri)
        );
        super::protocol::write_lsp_message(writer, &notif)?;
    }
    Ok(())
}
