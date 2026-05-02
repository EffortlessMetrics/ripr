use super::protocol::{extract_id, json_string, read_lsp_message, write_lsp_message};
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
    assert_eq!(
        extract_id(r#"{"jsonrpc":"2.0","method":"hover","params":{},"id":9}"#),
        Some("9".to_string())
    );
    assert_eq!(
        extract_id(r#"{"jsonrpc":"2.0","method":"initialized","params":{}}"#),
        None
    );
}

#[test]
fn read_lsp_message_parses_single_framed_payload() -> Result<(), String> {
    let body = r#"{"jsonrpc":"2.0","id":1,"method":"shutdown"}"#;
    let wire = format!("Content-Length: {}\r\n\r\n{}", body.len(), body);
    let mut reader = Cursor::new(wire.into_bytes());

    let message = read_lsp_message(&mut reader)?;

    assert_eq!(message.as_deref(), Some(body));
    Ok(())
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
fn read_lsp_message_requires_content_length_header() -> Result<(), String> {
    let mut reader = Cursor::new(b"\r\n{}".to_vec());
    let err = match read_lsp_message(&mut reader) {
        Ok(_) => return Err("expected missing Content-Length error".to_string()),
        Err(err) => err,
    };
    assert!(err.contains("missing Content-Length"));
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
