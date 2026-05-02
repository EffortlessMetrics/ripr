use std::io::{BufRead, Write};

pub fn read_lsp_message(reader: &mut impl BufRead) -> Result<Option<String>, String> {
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

pub fn write_lsp_message(writer: &mut impl Write, body: &str) -> Result<(), String> {
    write!(writer, "Content-Length: {}\r\n\r\n{}", body.len(), body)
        .and_then(|_| writer.flush())
        .map_err(|err| format!("failed to write LSP message: {err}"))
}

pub fn extract_id(message: &str) -> Option<String> {
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

pub fn json_string(value: &str) -> String {
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
    fn read_lsp_message_parses_multiple_framed_payloads() -> Result<(), String> {
        let first = r#"{"jsonrpc":"2.0","id":1,"method":"shutdown"}"#;
        let second = r#"{"jsonrpc":"2.0","id":2,"method":"exit"}"#;
        let wire = format!(
            "Content-Length: {}\r\n\r\n{}Content-Length: {}\r\n\r\n{}",
            first.len(),
            first,
            second.len(),
            second
        );
        let mut reader = Cursor::new(wire.into_bytes());

        assert_eq!(read_lsp_message(&mut reader)?.as_deref(), Some(first));
        assert_eq!(read_lsp_message(&mut reader)?.as_deref(), Some(second));
        Ok(())
    }

    #[test]
    fn write_lsp_message_emits_valid_length_prefixed_frame() -> Result<(), String> {
        let body = r#"{"ok":true}"#;
        let mut writer = Cursor::new(Vec::new());
        write_lsp_message(&mut writer, body)?;
        let wire = String::from_utf8(writer.into_inner()).map_err(|err| err.to_string())?;
        assert_eq!(
            wire,
            format!("Content-Length: {}\r\n\r\n{body}", body.len())
        );
        Ok(())
    }
}
