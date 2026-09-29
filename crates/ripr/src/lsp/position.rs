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
//! when the client advertises nothing.
//!
//! **Start offset:** analyzer columns for diff probes are commonly 1 even on
//! indented lines (#4602). When the saved line is available, the start is the
//! negotiated-encoding width of the prefix before the first verbatim occurrence
//! of the probe expression. If the expression is empty or not on the line, the
//! start is the first non-whitespace character. If the saved file cannot be
//! read, the start stays `column - 1`. This is LSP-local; it does not change
//! analyzer columns or SARIF. Producer-owned byte origins (#4464) override this
//! heuristic when the analysis context carries an exact or coarse Rust record.
//! Width-only non-ASCII fixtures remain #1737.
//!
//! **Range-constructor inventory** — which spans depend on source-text width:
//! - [`expression_span_range`] / [`expression_span_range_on_saved_line`] — the
//!   span width is the negotiated-encoding width of the changed expression, so
//!   it is **encoding-aware**. The start is encoding-aware only when a saved
//!   line is supplied.
//! - [`line_span_range`] — a fixed `0..MAX_LINE_SPAN_WIDTH` column span used by
//!   seam/gap diagnostics that have no specific expression; it measures no
//!   source text, so it is **encoding-independent** (line-only).
//!
//! Hover, code-action, and lens surfaces reuse these ranges or the analyzer's
//! own locators; none measure source-text width independently. Finding hover
//! matches [`expression_span_range_on_saved_line`] via `position_in_range`.
//! Line terminators (CR/LF) never appear inside a single-line expression span.

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
    encoding_unit_offset(text, encoding).max(1)
}

/// Code-unit length of `text` in the negotiated encoding, including 0 for
/// an empty prefix. Unlike [`character_width`], this is not floored at 1.
fn encoding_unit_offset(text: &str, encoding: &PositionEncodingKind) -> u32 {
    if *encoding == PositionEncodingKind::UTF8 {
        text.len() as u32
    } else if *encoding == PositionEncodingKind::UTF32 {
        text.chars().count() as u32
    } else {
        text.chars()
            .map(|character| character.len_utf16() as u32)
            .sum()
    }
}

/// Build a [`Range`] covering an expression span on a single line when the
/// saved source line is not available. Start is `column - 1`.
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
    finished_expression_span(line, column.saturating_sub(1) as u32, expression, encoding)
}

/// Build a [`Range`] covering an expression span, using `saved_line` to
/// locate the start when present.
///
/// When `saved_line` is `Some`, the start is the negotiated-encoding width of
/// the prefix before the first verbatim `expression`, or the first
/// non-whitespace character when the expression is empty or absent. When it is
/// `None`, the start is the analyzer `column - 1` fallback.
pub(crate) fn expression_span_range_on_saved_line(
    line: u32,
    column: usize,
    expression: &str,
    encoding: &PositionEncodingKind,
    saved_line: Option<&str>,
) -> Range {
    match saved_line {
        Some(source) => finished_expression_span(
            line,
            start_character_on_saved_line(source, expression, encoding),
            expression,
            encoding,
        ),
        None => expression_span_range(line, column, expression, encoding),
    }
}

fn finished_expression_span(
    line: u32,
    start_character: u32,
    expression: &str,
    encoding: &PositionEncodingKind,
) -> Range {
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

fn start_character_on_saved_line(
    saved_line: &str,
    expression: &str,
    encoding: &PositionEncodingKind,
) -> u32 {
    let prefix_bytes = match saved_line.find(expression) {
        Some(offset) if !expression.is_empty() => offset,
        _ => first_non_whitespace_byte(saved_line),
    };
    encoding_unit_offset(&saved_line[..prefix_bytes], encoding)
}

fn first_non_whitespace_byte(line: &str) -> usize {
    line.char_indices()
        .find(|(_, ch)| !ch.is_whitespace())
        .map_or(line.len(), |(idx, _)| idx)
}

/// Select stored producer-owned endpoints for the negotiated encoding.
pub(crate) fn range_from_encoded_origin(
    origin: &crate::analysis::diagnostic_origin::EncodedOrigin,
    encoding: &PositionEncodingKind,
) -> Range {
    let span = if *encoding == PositionEncodingKind::UTF8 {
        origin.for_utf8()
    } else if *encoding == PositionEncodingKind::UTF32 {
        origin.for_utf32()
    } else {
        origin.for_utf16()
    };
    Range {
        start: Position {
            line: origin.line,
            character: span.start,
        },
        end: Position {
            line: origin.line,
            character: span.end,
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
    fn origin_max_span_matches_position_cap() {
        assert_eq!(
            crate::analysis::diagnostic_origin::ORIGIN_MAX_SPAN_WIDTH,
            MAX_LINE_SPAN_WIDTH
        );
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

    #[test]
    fn encoding_unit_offset_empty_prefix_is_zero() {
        for encoding in [
            PositionEncodingKind::UTF8,
            PositionEncodingKind::UTF16,
            PositionEncodingKind::UTF32,
        ] {
            assert_eq!(encoding_unit_offset("", &encoding), 0);
        }
    }

    #[test]
    fn saved_line_locates_verbatim_expression_after_indent() {
        let line = "    total >= limit";
        let range = expression_span_range_on_saved_line(
            1,
            1,
            "total >= limit",
            &PositionEncodingKind::UTF16,
            Some(line),
        );
        assert_eq!(range.start.character, 4);
        assert_eq!(range.end.character, 18);
    }

    #[test]
    fn saved_line_measures_unicode_prefix_per_encoding() {
        let line = "\tlet s = \"日本語🎉\"; let _ = s; return x >= 5;";
        let utf16 = expression_span_range_on_saved_line(
            0,
            1,
            "x >= 5",
            &PositionEncodingKind::UTF16,
            Some(line),
        );
        assert_eq!((utf16.start.character, utf16.end.character), (36, 42));
        let utf8 = expression_span_range_on_saved_line(
            0,
            1,
            "x >= 5",
            &PositionEncodingKind::UTF8,
            Some(line),
        );
        assert_eq!((utf8.start.character, utf8.end.character), (44, 50));
        let utf32 = expression_span_range_on_saved_line(
            0,
            1,
            "x >= 5",
            &PositionEncodingKind::UTF32,
            Some(line),
        );
        assert_eq!((utf32.start.character, utf32.end.character), (35, 41));
    }

    #[test]
    fn saved_line_falls_back_to_first_non_whitespace_when_expression_absent() {
        let range = expression_span_range_on_saved_line(
            0,
            1,
            "x >= 5",
            &PositionEncodingKind::UTF16,
            Some("\t    return true;"),
        );
        assert_eq!(range.start.character, 5);
        assert_eq!(range.end.character, 11);
    }

    #[test]
    fn saved_line_empty_expression_starts_at_first_non_whitespace() {
        let range = expression_span_range_on_saved_line(
            0,
            3,
            "",
            &PositionEncodingKind::UTF16,
            Some("  body"),
        );
        assert_eq!(range.start.character, 2);
        assert_eq!(range.end.character, 3);
    }

    #[test]
    fn missing_saved_line_keeps_analyzer_column() {
        let range =
            expression_span_range_on_saved_line(0, 5, "total", &PositionEncodingKind::UTF16, None);
        assert_eq!(range.start.character, 4);
        assert_eq!(range.end.character, 9);
    }

    #[test]
    fn saved_line_uses_first_verbatim_occurrence() {
        let range = expression_span_range_on_saved_line(
            0,
            1,
            "x >= 5",
            &PositionEncodingKind::UTF16,
            Some("    x >= 5; let _ = x >= 5;"),
        );
        assert_eq!(range.start.character, 4);
        assert_eq!(range.end.character, 10);
    }

    #[test]
    fn saved_line_expression_at_column_zero_stays_at_zero() {
        let range = expression_span_range_on_saved_line(
            0,
            1,
            "x >= 5",
            &PositionEncodingKind::UTF16,
            Some("x >= 5;"),
        );
        assert_eq!(range.start.character, 0);
        assert_eq!(range.end.character, 6);
    }

    #[test]
    fn saved_line_keeps_indent_when_expression_includes_it() {
        let line = "    return remote_total(client, include_tax=True)";
        let range = expression_span_range_on_saved_line(
            0,
            1,
            line,
            &PositionEncodingKind::UTF16,
            Some(line),
        );
        assert_eq!(range.start.character, 0);
        assert_eq!(range.end.character, line.len() as u32);
    }

    #[test]
    fn saved_line_combining_prefix_counts_each_scalar() {
        // "e" + combining acute is two UTF-16 units before `x`.
        let line = "  e\u{0301} x";
        let range = expression_span_range_on_saved_line(
            0,
            1,
            "x",
            &PositionEncodingKind::UTF16,
            Some(line),
        );
        assert_eq!(range.start.character, 5);
        let utf8 =
            expression_span_range_on_saved_line(0, 1, "x", &PositionEncodingKind::UTF8, Some(line));
        assert_eq!(utf8.start.character, 6);
    }

    #[test]
    fn saved_line_whitespace_only_starts_at_line_end() {
        let range = expression_span_range_on_saved_line(
            0,
            1,
            "x >= 5",
            &PositionEncodingKind::UTF16,
            Some("\t  "),
        );
        assert_eq!(range.start.character, 3);
        assert_eq!(range.end.character, 9);
    }
}
