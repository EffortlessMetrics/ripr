//! Centralized analyzer-span → LSP Range/Position conversion.
//!
//! Every surface that constructs an LSP [`Range`] or [`Position`] from
//! analyzer line/column/expression data must route through this module so
//! that position-encoding decisions (UTF-16 vs UTF-8 vs UTF-32) are made in
//! exactly one place. See #1626 / #1748.
//!
//! **Current encoding:** UTF-16 (the LSP default prior to `general.
//! positionEncodings` negotiation). When #1626 PR B lands, the encoding
//! will be selected from the negotiated client capability and plumbed
//! through these functions.

use tower_lsp_server::ls_types::{Position, Range};

/// The maximum character width used for full-line diagnostic spans.
/// Diagnostics that cover a whole line (seam/gap diagnostics without a
/// specific expression) span from character 0 to this width.
pub(crate) const MAX_LINE_SPAN_WIDTH: u32 = 120;

/// Compute the UTF-16 code-unit width of a text string.
///
/// This is the LSP default character offset: each character contributes
/// `char::len_utf16()` code units (1 for BMP, 2 for astral planes).
/// Returns at least 1 so a non-empty expression always has a visible span.
pub(crate) fn utf16_character_width(text: &str) -> u32 {
    text.chars()
        .map(|character| character.len_utf16() as u32)
        .sum::<u32>()
        .max(1)
}

/// Build a [`Range`] covering an expression span on a single line.
///
/// `line` is 0-based (LSP convention). `column` is 1-based from the
/// analyzer and is converted to 0-based here. The span width is the UTF-16
/// width of `expression`, capped at [`MAX_LINE_SPAN_WIDTH`].
pub(crate) fn expression_span_range(line: u32, column: u32, expression: &str) -> Range {
    let start_character = column.saturating_sub(1);
    let width = utf16_character_width(expression).min(MAX_LINE_SPAN_WIDTH);
    Range {
        start: Position {
            line,
            character: start_character,
        },
        end: Position {
            line,
            character: start_character.saturating_add(width),
        },
    }
}

/// Build a [`Range`] covering a full line (character 0 to
/// [`MAX_LINE_SPAN_WIDTH`]). Used for seam/gap diagnostics that don't have
/// a specific expression to highlight.
pub(crate) fn line_span_range(line: u32) -> Range {
    Range {
        start: Position { line, character: 0 },
        end: Position {
            line,
            character: MAX_LINE_SPAN_WIDTH,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn utf16_character_width_ascii() {
        assert_eq!(utf16_character_width("hello"), 5);
    }

    #[test]
    fn utf16_character_width_empty_returns_one() {
        assert_eq!(utf16_character_width(""), 1);
    }

    #[test]
    fn utf16_character_width_cjk_counts_two() {
        // CJK characters are in the BMP (1 UTF-16 unit each).
        assert_eq!(utf16_character_width("日本語"), 3);
    }

    #[test]
    fn utf16_character_width_astral_plane_counts_four() {
        // Emoji outside the BMP: each is 2 UTF-16 code units.
        assert_eq!(utf16_character_width("🎉"), 2);
        assert_eq!(utf16_character_width("🎉🎉"), 4);
    }

    #[test]
    fn expression_span_range_basic() {
        let range = expression_span_range(5, 10, "foo");
        assert_eq!(range.start.line, 5);
        assert_eq!(range.start.character, 9); // column 10 → 0-based 9
        assert_eq!(range.end.line, 5);
        assert_eq!(range.end.character, 12); // 9 + 3
    }

    #[test]
    fn expression_span_range_caps_at_max_width() {
        let long = "x".repeat(200);
        let range = expression_span_range(0, 1, &long);
        assert_eq!(
            range.end.character - range.start.character,
            MAX_LINE_SPAN_WIDTH
        );
    }

    #[test]
    fn line_span_range_covers_zero_to_max() {
        let range = line_span_range(42);
        assert_eq!(range.start.line, 42);
        assert_eq!(range.start.character, 0);
        assert_eq!(range.end.line, 42);
        assert_eq!(range.end.character, MAX_LINE_SPAN_WIDTH);
    }
}
