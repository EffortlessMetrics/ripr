//! Centralized analyzer-span → LSP Range/Position conversion.
//!
//! Every surface that constructs an LSP [`Range`] or [`Position`] from
//! analyzer line/column/expression data must route through this module so
//! that position-encoding decisions (UTF-16 vs UTF-8 vs UTF-32) are made in
//! exactly one place. See #1626 / #1748.
//!
//! **Encoding:** selected from the negotiated `general.positionEncodings`
//! client capability at `initialize` (see #1626 PR B / #1749). The chosen
//! [`PositionEncodingKind`] is plumbed through these functions so an expression
//! span's *width* is measured in the negotiated encoding. UTF-16 is the default
//! when the client advertises nothing. Converting an analyzer column to a
//! negotiated-encoding *start* offset for non-ASCII line prefixes is a separate
//! concern tracked with the non-ASCII fixtures (#1737).
//!
//! **Range-constructor inventory** — which spans depend on source-text width:
//! - [`expression_span_range`] — the span width is the negotiated-encoding
//!   width of the changed expression, so it is **encoding-aware**.
//! - [`line_span_range`] — a fixed `0..MAX_LINE_SPAN_WIDTH` column span used by
//!   seam/gap diagnostics that have no specific expression; it measures no
//!   source text, so it is **encoding-independent** (line-only).
//!
//! Hover, code-action, and lens surfaces reuse these ranges or the analyzer's
//! own locators; none measure source-text width. `expression_span_range` is
//! therefore the only encoding-sensitive width in the LSP layer. Line
//! terminators (CR/LF) never appear inside a single-line expression span.

use tower_lsp_server::ls_types::{Position, PositionEncodingKind, Range};

/// The maximum character width used for full-line diagnostic spans.
/// Diagnostics that cover a whole line (seam/gap diagnostics without a
/// specific expression) span from character 0 to this width.
pub(crate) const MAX_LINE_SPAN_WIDTH: u32 = 120;

/// Compute the code-unit width of a text string in the negotiated encoding.
///
/// - UTF-8: byte length (each UTF-8 code unit is one byte).
/// - UTF-32: Unicode scalar-value count (one code unit per `char`).
/// - UTF-16 (default): `char::len_utf16()` summed (1 for BMP, 2 for astral).
///
/// Returns at least 1 so a non-empty expression always has a visible span.
pub(crate) fn character_width(text: &str, encoding: &PositionEncodingKind) -> u32 {
    let width = if *encoding == PositionEncodingKind::UTF8 {
        text.len() as u32
    } else if *encoding == PositionEncodingKind::UTF32 {
        text.chars().count() as u32
    } else {
        text.chars()
            .map(|character| character.len_utf16() as u32)
            .sum::<u32>()
    };
    width.max(1)
}

/// Build a [`Range`] covering an expression span on a single line.
///
/// `line` is 0-based (LSP convention). `column` is 1-based from the
/// analyzer and is converted to 0-based here. The span width is the width of
/// `expression` in the negotiated `encoding`, capped at
/// [`MAX_LINE_SPAN_WIDTH`].
pub(crate) fn expression_span_range(
    line: u32,
    column: usize,
    expression: &str,
    encoding: &PositionEncodingKind,
) -> Range {
    let start_character = column.saturating_sub(1) as u32;
    let width = character_width(expression, encoding).min(MAX_LINE_SPAN_WIDTH);
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
    fn character_width_utf16_ascii() {
        assert_eq!(character_width("hello", &PositionEncodingKind::UTF16), 5);
    }

    #[test]
    fn character_width_utf16_bmp_counts_one() {
        // CJK characters are in the BMP (1 UTF-16 unit each).
        assert_eq!(character_width("日本語", &PositionEncodingKind::UTF16), 3);
    }

    #[test]
    fn character_width_utf16_astral_plane_counts_two() {
        // Emoji outside the BMP: each is 2 UTF-16 code units.
        assert_eq!(character_width("🎉", &PositionEncodingKind::UTF16), 2);
        assert_eq!(character_width("🎉🎉", &PositionEncodingKind::UTF16), 4);
    }

    #[test]
    fn character_width_utf8_counts_bytes() {
        // "é" is 2 bytes in UTF-8, 1 UTF-16 unit, 1 scalar value.
        assert_eq!(character_width("é", &PositionEncodingKind::UTF8), 2);
        assert_eq!(character_width("é", &PositionEncodingKind::UTF16), 1);
        assert_eq!(character_width("é", &PositionEncodingKind::UTF32), 1);
    }

    #[test]
    fn character_width_astral_plane_per_encoding() {
        // "🎉" is 4 bytes (UTF-8), 2 code units (UTF-16), 1 scalar (UTF-32).
        assert_eq!(character_width("🎉", &PositionEncodingKind::UTF8), 4);
        assert_eq!(character_width("🎉", &PositionEncodingKind::UTF16), 2);
        assert_eq!(character_width("🎉", &PositionEncodingKind::UTF32), 1);
    }

    #[test]
    fn character_width_cjk_and_accented_text() {
        // CJK are BMP scalars: 3 UTF-8 bytes, 1 UTF-16 unit, 1 scalar each.
        assert_eq!(character_width("日本語", &PositionEncodingKind::UTF8), 9);
        assert_eq!(character_width("日本語", &PositionEncodingKind::UTF16), 3);
        assert_eq!(character_width("日本語", &PositionEncodingKind::UTF32), 3);
        // "café": é is 2 UTF-8 bytes, 1 UTF-16 unit, 1 scalar.
        assert_eq!(character_width("café", &PositionEncodingKind::UTF8), 5);
        assert_eq!(character_width("café", &PositionEncodingKind::UTF16), 4);
        assert_eq!(character_width("café", &PositionEncodingKind::UTF32), 4);
    }

    #[test]
    fn character_width_combining_sequence_counts_each_scalar() {
        // "e" + U+0301 (combining acute): two scalars. U+0301 is 2 UTF-8 bytes.
        let combining = "e\u{0301}";
        assert_eq!(character_width(combining, &PositionEncodingKind::UTF8), 3);
        assert_eq!(character_width(combining, &PositionEncodingKind::UTF16), 2);
        assert_eq!(character_width(combining, &PositionEncodingKind::UTF32), 2);
    }

    #[test]
    fn character_width_tab_is_one_unit_in_every_encoding() {
        for encoding in [
            PositionEncodingKind::UTF8,
            PositionEncodingKind::UTF16,
            PositionEncodingKind::UTF32,
        ] {
            assert_eq!(character_width("\t", &encoding), 1);
        }
    }

    #[test]
    fn character_width_empty_returns_one_in_every_encoding() {
        for encoding in [
            PositionEncodingKind::UTF8,
            PositionEncodingKind::UTF16,
            PositionEncodingKind::UTF32,
        ] {
            assert_eq!(character_width("", &encoding), 1);
        }
    }

    #[test]
    fn expression_span_range_basic() {
        let range = expression_span_range(5, 10, "foo", &PositionEncodingKind::UTF16);
        assert_eq!(range.start.line, 5);
        assert_eq!(range.start.character, 9); // column 10 → 0-based 9
        assert_eq!(range.end.line, 5);
        assert_eq!(range.end.character, 12); // 9 + 3
    }

    #[test]
    fn expression_span_range_uses_negotiated_encoding_width() {
        // A 2-byte UTF-8 character spans 2 in UTF-8 but 1 in UTF-16.
        let utf8 = expression_span_range(0, 1, "é", &PositionEncodingKind::UTF8);
        assert_eq!(utf8.end.character - utf8.start.character, 2);
        let utf16 = expression_span_range(0, 1, "é", &PositionEncodingKind::UTF16);
        assert_eq!(utf16.end.character - utf16.start.character, 1);
    }

    #[test]
    fn expression_span_range_caps_at_max_width() {
        let long = "x".repeat(200);
        let range = expression_span_range(0, 1, &long, &PositionEncodingKind::UTF16);
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
