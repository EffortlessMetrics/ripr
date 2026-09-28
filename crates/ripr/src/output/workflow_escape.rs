//! GitHub workflow-command escaping authority (#4065).
//!
//! GitHub's command parser decodes two different escape sets: message
//! *data* (after `::`) decodes percent, CR, and LF only, while *property*
//! values (`file=`, `title=`) additionally decode comma and colon. Encoding
//! comma/colon in data leaves visible `%2C`/`%3A` scars; leaving them raw
//! in properties splits values into metadata. Every workflow-command
//! emitter (raw-check renderer, binary annotations adapter, and any future
//! consumer) must route through these three functions — never a local
//! `escape_cmd` fork — so the wire contract has exactly one owner.
//!
//! Percent handling differs by input kind, and the distinction is load
//! bearing: [`escape_property`] takes raw text and encodes `%` itself,
//! while [`escape_property_pre_encoded`] takes stable path text (which
//! already encodes every literal `%` as `%25` and every non-UTF-8 byte as
//! `%XX`, keeping distinct names distinct) and must NOT encode `%` again —
//! double-encoding yields `%2525`. Unicode passes through raw in all three:
//! the escape map has no UTF-8 branch.

/// Escape workflow-command *data* (the message after `::`).
///
/// Single pass: each input char is visited once, so the `%` introduced by
/// an insertion is never rescanned — literal `%0A` sequences survive one
/// decode as `%250A` instead of becoming a newline. Never blanket-decode
/// the input to "repair" this.
pub(crate) fn escape_data(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len());
    for c in value.chars() {
        match c {
            '%' => escaped.push_str("%25"),
            '\r' => escaped.push_str("%0D"),
            '\n' => escaped.push_str("%0A"),
            _ => escaped.push(c),
        }
    }
    escaped
}

/// Escape a workflow-command *property* value from raw text.
///
/// Use for values that did not pass through stable path encoding
/// (annotation titles, comments.json placement paths).
pub(crate) fn escape_property(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len());
    for c in value.chars() {
        match c {
            '%' => escaped.push_str("%25"),
            '\r' => escaped.push_str("%0D"),
            '\n' => escaped.push_str("%0A"),
            ',' => escaped.push_str("%2C"),
            ':' => escaped.push_str("%3A"),
            _ => escaped.push(c),
        }
    }
    escaped
}

/// Escape a workflow-command *property* value from stable path text.
///
/// Use only for values produced by stable path normalization, where `%`
/// is already encoded. Encoding `%` again double-encodes (`%2525`).
pub(crate) fn escape_property_pre_encoded(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len());
    for c in value.chars() {
        match c {
            '\r' => escaped.push_str("%0D"),
            '\n' => escaped.push_str("%0A"),
            ',' => escaped.push_str("%2C"),
            ':' => escaped.push_str("%3A"),
            _ => escaped.push(c),
        }
    }
    escaped
}

#[cfg(test)]
mod tests {
    use super::{escape_data, escape_property, escape_property_pre_encoded};

    #[test]
    fn data_keeps_comma_colon_literal() {
        assert_eq!(
            escape_data("Result::Err from assert_eq!(actual, expected) at 100%"),
            "Result::Err from assert_eq!(actual, expected) at 100%25"
        );
    }

    #[test]
    fn data_escapes_cr_lf_and_percent_first() {
        assert_eq!(escape_data("a\rb\nc%d"), "a%0Db%0Ac%25d");
        assert_eq!(escape_data("literal %0A token"), "literal %250A token");
    }

    #[test]
    fn property_escapes_all_five_from_raw_text() {
        assert_eq!(
            escape_property("src/a,b:c%d\r\né.rs"),
            "src/a%2Cb%3Ac%25d%0D%0Aé.rs"
        );
    }

    #[test]
    fn pre_encoded_property_skips_percent_only() {
        assert_eq!(
            escape_property_pre_encoded("src/a%25b,c:d\r\nx.rs"),
            "src/a%25b%2Cc%3Ad%0D%0Ax.rs"
        );
    }

    #[test]
    fn unicode_passes_through_everywhere() {
        assert_eq!(escape_data("naïve ünïcode"), "naïve ünïcode");
        assert_eq!(escape_property("src/dé.rs"), "src/dé.rs");
        assert_eq!(escape_property_pre_encoded("src/dé.rs"), "src/dé.rs");
    }
}
