use crate::app::{CheckInput, OutputFormat, check_workspace};
use std::io::{BufRead, BufReader, Write};

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
            let response = format!(
                r#"{{"jsonrpc":"2.0","id":{id},"result":{{"capabilities":{{"textDocumentSync":1,"hoverProvider":true,"codeActionProvider":true,"executeCommandProvider":{{"commands":["ripr.collectContext","ripr.refresh"]}}}},"serverInfo":{{"name":"ripr","version":"{}"}}}}}}"#,
                env!("CARGO_PKG_VERSION")
            );
            write_lsp_message(&mut writer, &response)?;
        } else if message.contains("\"method\":\"shutdown\"")
            || message.contains("\"method\": \"shutdown\"")
        {
            let id = extract_id(&message).unwrap_or_else(|| "1".to_string());
            write_lsp_message(
                &mut writer,
                &format!(r#"{{"jsonrpc":"2.0","id":{id},"result":null}}"#),
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
            let response = format!(
                r#"{{"jsonrpc":"2.0","id":{id},"result":{{"contents":{{"kind":"markdown","value":"ripr estimates static RIPR exposure for changed Rust behavior. Run `ripr check --format json` for current findings."}}}}}}"#
            );
            write_lsp_message(&mut writer, &response)?;
        } else if message.contains("\"method\":\"textDocument/codeAction\"")
            || message.contains("\"method\": \"textDocument/codeAction\"")
        {
            let id = extract_id(&message).unwrap_or_else(|| "1".to_string());
            let response = format!(
                r#"{{"jsonrpc":"2.0","id":{id},"result":[{{"title":"Copy ripr context packet","kind":"quickfix","command":{{"title":"Collect ripr context","command":"ripr.collectContext","arguments":[]}}}},{{"title":"Run ripr check","kind":"source","command":{{"title":"Refresh ripr analysis","command":"ripr.refresh","arguments":[]}}}}]}}"#
            );
            write_lsp_message(&mut writer, &response)?;
        } else if let Some(id) = extract_id(&message) {
            write_lsp_message(
                &mut writer,
                &format!(r#"{{"jsonrpc":"2.0","id":{id},"result":null}}"#),
            )?;
        }
    }

    Ok(())
}

fn publish_workspace_diagnostics(writer: &mut impl Write) -> Result<(), String> {
    let input = CheckInput {
        root: std::env::current_dir().map_err(|err| format!("failed to get current dir: {err}"))?,
        format: OutputFormat::Json,
        ..CheckInput::default()
    };
    let output = match check_workspace(input) {
        Ok(output) => output,
        Err(_) => return Ok(()),
    };
    // Minimal LSP diagnostics: group by file and include finding text. The JSON is
    // intentionally simple; richer code actions use the CLI/context path for now.
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
        let notif = format!(
            r#"{{"jsonrpc":"2.0","method":"textDocument/publishDiagnostics","params":{{"uri":{},"diagnostics":{diagnostics}}}}}"#,
            json_string(&uri)
        );
        write_lsp_message(writer, &notif)?;
    }
    Ok(())
}

fn lsp_message(finding: &crate::domain::Finding) -> String {
    finding
        .recommended_next_step
        .clone()
        .unwrap_or_else(|| format!("{} static RIPR exposure", finding.class.as_str()))
}

fn read_lsp_message(reader: &mut impl BufRead) -> Result<Option<String>, String> {
    let mut content_length = None::<usize>;
    loop {
        let mut line = String::new();
        let bytes = reader
            .read_line(&mut line)
            .map_err(|err| format!("failed to read LSP header: {err}"))?;
        if bytes == 0 {
            return Ok(None);
        }
        let trimmed = line.trim_end_matches(&['\r', '\n'][..]);
        if trimmed.is_empty() {
            break;
        }
        if let Some(value) = trimmed.strip_prefix("Content-Length:") {
            content_length = Some(
                value
                    .trim()
                    .parse::<usize>()
                    .map_err(|err| format!("invalid Content-Length: {err}"))?,
            );
        }
    }
    let len = content_length.ok_or_else(|| "missing Content-Length".to_string())?;
    let mut buf = vec![0u8; len];
    reader
        .read_exact(&mut buf)
        .map_err(|err| format!("failed to read LSP body: {err}"))?;
    Ok(Some(String::from_utf8_lossy(&buf).into_owned()))
}

fn write_lsp_message(writer: &mut impl Write, body: &str) -> Result<(), String> {
    write!(writer, "Content-Length: {}\r\n\r\n{}", body.len(), body)
        .and_then(|_| writer.flush())
        .map_err(|err| format!("failed to write LSP message: {err}"))
}

fn extract_id(message: &str) -> Option<String> {
    let idx = message.find("\"id\"")?;
    let after = &message[idx + 4..];
    let colon = after.find(':')?;
    let rest = after[colon + 1..].trim_start();
    if let Some(stripped) = rest.strip_prefix('"') {
        let end = stripped.find('"')?;
        Some(format!("\"{}\"", &stripped[..end]))
    } else {
        let end = rest.find([',', '}']).unwrap_or(rest.len());
        Some(rest[..end].trim().to_string())
    }
}

fn json_string(value: &str) -> String {
    serde_json::to_string(value).expect("serializing a string cannot fail")
}

#[cfg(test)]
mod tests {
    use super::{extract_id, json_string, read_lsp_message, write_lsp_message};
    use std::io::Cursor;

    #[test]
    fn json_string_escapes_lsp_control_characters() {
        let value = "quote\" slash\\ newline\n tab\t control\u{0001}";
        let encoded = json_string(value);
        let decoded: String = serde_json::from_str(&encoded).unwrap();
        assert_eq!(decoded, value);
        assert!(encoded.contains("\\\""));
        assert!(encoded.contains("\\\\"));
        assert!(encoded.contains("\\n"));
        assert!(encoded.contains("\\t"));
        assert!(encoded.contains("\\u0001"));
    }

    #[test]
    fn extract_id_supports_numeric_and_string_ids() {
        assert_eq!(
            extract_id(r#"{"jsonrpc":"2.0","id":7,"method":"initialize"}"#),
            Some("7".to_string())
        );
        assert_eq!(
            extract_id(r#"{"jsonrpc":"2.0","id":"req-42","method":"hover"}"#),
            Some("\"req-42\"".to_string())
        );
    }

    #[test]
    fn read_lsp_message_parses_single_framed_payload() {
        let body = r#"{"jsonrpc":"2.0","id":1,"method":"shutdown"}"#;
        let wire = format!("Content-Length: {}\r\n\r\n{}", body.len(), body);
        let mut reader = Cursor::new(wire.into_bytes());

        let message = read_lsp_message(&mut reader).unwrap();

        assert_eq!(message.as_deref(), Some(body));
    }

    #[test]
    fn read_lsp_message_requires_content_length_header() {
        let mut reader = Cursor::new(b"\r\n{}".to_vec());
        let err = read_lsp_message(&mut reader).unwrap_err();
        assert!(err.contains("missing Content-Length"));
    }

    #[test]
    fn write_lsp_message_emits_valid_length_prefixed_frame() {
        let body = r#"{"ok":true}"#;
        let mut writer = Cursor::new(Vec::new());
        write_lsp_message(&mut writer, body).unwrap();
        let wire = String::from_utf8(writer.into_inner()).unwrap();
        assert_eq!(wire, format!("Content-Length: {}\r\n\r\n{body}", body.len()));
    }
}
