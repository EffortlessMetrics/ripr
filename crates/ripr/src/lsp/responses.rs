use crate::domain::Finding;
use std::collections::BTreeMap;

use super::protocol::{extract_id, json_string};

pub(super) fn response_for_message(message: &str) -> Option<String> {
    if is_initialize_request(message) {
        let id = extract_id(message).unwrap_or_else(|| "1".to_string());
        return Some(initialize_response(&id));
    }
    if is_shutdown_request(message) {
        let id = extract_id(message).unwrap_or_else(|| "1".to_string());
        return Some(null_result(&id));
    }
    if is_hover_request(message) {
        let id = extract_id(message).unwrap_or_else(|| "1".to_string());
        return Some(hover_response(&id));
    }
    if is_code_action_request(message) {
        let id = extract_id(message).unwrap_or_else(|| "1".to_string());
        return Some(code_action_response(&id));
    }
    None
}

pub(super) fn should_publish_diagnostics(message: &str) -> bool {
    message.contains("textDocument/didOpen")
        || message.contains("textDocument/didChange")
        || message.contains("textDocument/didSave")
        || message.contains("ripr.refresh")
}

pub(super) fn is_exit_request(message: &str) -> bool {
    message.contains("\"method\":\"exit\"") || message.contains("\"method\": \"exit\"")
}

pub(super) fn null_result(id: &str) -> String {
    format!(r#"{{"jsonrpc":"2.0","id":{id},"result":null}}"#)
}

fn is_initialize_request(message: &str) -> bool {
    message.contains("\"method\":\"initialize\"") || message.contains("\"method\": \"initialize\"")
}

fn is_shutdown_request(message: &str) -> bool {
    message.contains("\"method\":\"shutdown\"") || message.contains("\"method\": \"shutdown\"")
}

fn is_hover_request(message: &str) -> bool {
    message.contains("\"method\":\"textDocument/hover\"")
        || message.contains("\"method\": \"textDocument/hover\"")
}

fn is_code_action_request(message: &str) -> bool {
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

pub(super) fn diagnostic_notifications(findings: &[Finding]) -> Vec<String> {
    let mut grouped: BTreeMap<String, Vec<&Finding>> = BTreeMap::new();
    for finding in findings {
        grouped
            .entry(format!("file://{}", finding.probe.location.file.display()))
            .or_default()
            .push(finding);
    }

    grouped
        .into_iter()
        .map(|(uri, findings)| {
            let diagnostics = render_diagnostics(&findings);
            format!(
                r#"{{"jsonrpc":"2.0","method":"textDocument/publishDiagnostics","params":{{"uri":{},"diagnostics":{diagnostics}}}}}"#,
                json_string(&uri)
            )
        })
        .collect()
}

fn render_diagnostics(findings: &[&Finding]) -> String {
    let mut diagnostics = String::from("[");
    for (idx, finding) in findings.iter().enumerate() {
        let line = finding.probe.location.line.saturating_sub(1);
        diagnostics.push_str(&format!(
            r#"{{"range":{{"start":{{"line":{line},"character":0}},"end":{{"line":{line},"character":120}}}},"severity":2,"source":"ripr","code":{},"message":{},"data":{{"probeId":{},"class":{},"family":{},"confidence":{:.2}}}}}"#,
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
