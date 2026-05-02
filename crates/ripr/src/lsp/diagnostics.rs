use crate::app::{CheckInput, OutputFormat, check_workspace};
use crate::domain::Finding;
use crate::lsp::protocol::{json_string, write_lsp_message};
use std::collections::BTreeMap;
use std::io::Write;

pub(crate) fn publish_workspace_diagnostics(writer: &mut impl Write) -> Result<(), String> {
    let input = CheckInput {
        root: std::env::current_dir().map_err(|err| format!("failed to get current dir: {err}"))?,
        format: OutputFormat::Json,
        ..CheckInput::default()
    };
    let output = match check_workspace(input) {
        Ok(output) => output,
        Err(_) => return Ok(()),
    };

    let mut grouped: BTreeMap<String, Vec<&Finding>> = BTreeMap::new();
    for finding in &output.findings {
        grouped
            .entry(format!("file://{}", finding.probe.location.file.display()))
            .or_default()
            .push(finding);
    }
    for (uri, findings) in grouped {
        let diagnostics = render_diagnostics(&findings);
        let notif = format!(
            r#"{{\"jsonrpc\":\"2.0\",\"method\":\"textDocument/publishDiagnostics\",\"params\":{{\"uri\":{},\"diagnostics\":{diagnostics}}}}}"#,
            json_string(&uri)
        );
        write_lsp_message(writer, &notif)?;
    }
    Ok(())
}

fn render_diagnostics(findings: &[&Finding]) -> String {
    let mut diagnostics = String::from("[");
    for (idx, finding) in findings.iter().enumerate() {
        let line = finding.probe.location.line.saturating_sub(1);
        diagnostics.push_str(&format!(
            r#"{{\"range\":{{\"start\":{{\"line\":{line},\"character\":0}},\"end\":{{\"line\":{line},\"character\":120}}}},\"severity\":2,\"source\":\"ripr\",\"code\":{},\"message\":{},\"data\":{{\"probeId\":{},\"class\":{},\"family\":{},\"confidence\":{:.2}}}}}"#,
            json_string(finding.class.as_str()),
            json_string(&lsp_message(finding)),
            json_string(&finding.id),
            json_string(finding.class.as_str()),
            json_string(finding.probe.family.as_str()),
            finding.confidence,
        ));
        if idx + 1 != findings.len() {
            diagnostics.push(',');
        }
    }
    diagnostics.push(']');
    diagnostics
}

fn lsp_message(finding: &Finding) -> String {
    finding
        .recommended_next_step
        .clone()
        .unwrap_or_else(|| format!("{} static RIPR exposure", finding.class.as_str()))
}
