//! Terminal-safe rendering of repository text.
//!
//! The character policy (`needs_terminal_escape`) lives in
//! `agent/loop_commands.rs`, which `xtask` includes by path; this module is the
//! library-only escape built on it, importable from any layer (reports, stderr
//! notices in analysis, workflow encoders).

use crate::agent::loop_commands::needs_terminal_escape;

/// Make a finished human report safe to print to a terminal. Repository text
/// (assertion source, test names, observed values) reaches the report verbatim,
/// so a hostile repository could otherwise carry ESC/CSI/OSC sequences (clear
/// the screen, retitle the window), BEL, a bare CR that overwrites a line, or a
/// bidi override that reorders what the reader sees. Every control character
/// except `\n` and `\t`, and the bidi/invisible formatting characters, renders
/// as `\u{XX}`. Machine formats (JSON, SARIF) keep the raw value, escaped by
/// their own encoders.
pub(crate) fn terminal_safe(text: String) -> String {
    if !text.chars().any(needs_terminal_escape) {
        return text;
    }
    let mut out = String::with_capacity(text.len());
    for ch in text.chars() {
        if needs_terminal_escape(ch) {
            out.push_str(&format!("\\u{{{:02x}}}", ch as u32));
        } else {
            out.push(ch);
        }
    }
    out
}

/// Make a serialized JSON document safe to print to a terminal without
/// breaking it. serde_json already escapes ASCII controls; C1 controls and
/// bidi characters stay raw inside strings, so they are spelled as `\uXXXX`,
/// which a JSON parser reads back as the same characters. Use this, not
/// [`terminal_safe`], for machine-readable documents printed to stderr.
pub(crate) fn json_terminal_safe(json: String) -> String {
    if !json.chars().any(needs_terminal_escape) {
        return json;
    }
    let mut out = String::with_capacity(json.len());
    for ch in json.chars() {
        if needs_terminal_escape(ch) {
            out.push_str(&format!("\\u{:04x}", ch as u32));
        } else {
            out.push(ch);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::json_terminal_safe;

    #[test]
    fn json_terminal_safe_keeps_the_document_parseable() -> Result<(), String> {
        let value = serde_json::json!({"seam_id": "a\u{202e}b\u{85}c\u{1b}d"});
        let text = serde_json::to_string_pretty(&value).map_err(|e| e.to_string())?;
        let safe = json_terminal_safe(text);
        assert!(!safe.contains('\u{202e}') && !safe.contains('\u{85}') && !safe.contains('\u{1b}'));
        let back: serde_json::Value = serde_json::from_str(&safe).map_err(|e| e.to_string())?;
        assert_eq!(back, value);
        Ok(())
    }
}
