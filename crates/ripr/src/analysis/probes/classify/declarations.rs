//! Parser-confirmed declarations stay unknown, not executable fields.

use super::super::lexical::classify_changed_line;
use super::{ParserProbeShape, source_line_byte_range};
use crate::analysis::rust_index::FileFacts;
use crate::domain::ProbeFamily;
use ra_ap_syntax::{AstNode, Edition, SourceFile, SyntaxKind, SyntaxNode, ast};
use std::ops::Range;

/// This fallback is used only after ordinary parser-owned expressions have
/// been considered. It never promotes exposure or drops the changed source.
/// Exact current-line equality prevents borrowing new-side context for a
/// different removed/stale line. Shared declaration/expression lines are not
/// eligible. Record fields containing const expressions or macros remain with
/// the existing analysis rather than acquiring a declaration-only disposition.
pub(super) fn declaration_shape<'a>(
    facts: &'a FileFacts,
    line: usize,
    changed_text: &str,
) -> Option<ParserProbeShape<'a>> {
    if facts.used_lexical_fallback
        || !classify_changed_line(changed_text).contains(&ProbeFamily::FieldConstruction)
    {
        return None;
    }
    let line_range = source_line_byte_range(&facts.source, line)?;
    let source_line = facts.source.get(line_range.clone())?;
    let text = source_line.trim();
    if text.is_empty() || text != changed_text.trim() {
        return None;
    }

    // Reuse the existing parser, not a colon or comment-delimiter heuristic.
    // Real expression shapes take precedence and do not enter this fallback.
    let parse = SourceFile::parse(&facts.source, Edition::CURRENT);
    if !parse.errors().is_empty() {
        return None;
    }
    let root = parse.tree();
    let declaration = declaration_line_span(root.syntax(), line_range)?;
    for node in root.syntax().descendants().filter(supported_declaration) {
        let range = node.text_range();
        let start = u32::from(range.start()) as usize;
        let end = u32::from(range.end()) as usize;
        if start <= declaration.start && declaration.end <= end {
            return Some(ParserProbeShape {
                family: ProbeFamily::StaticUnknown,
                start_line: line,
                start_byte: declaration.start,
                text,
                standalone_call: false,
                unsafe_boundary: false,
            });
        }
    }
    None
}

fn supported_declaration(node: &SyntaxNode) -> bool {
    if ast::Param::can_cast(node.kind()) || ast::SelfParam::can_cast(node.kind()) {
        return node
            .parent()
            .and_then(ast::ParamList::cast)
            .and_then(|list| list.syntax().parent())
            .is_some_and(|parent| ast::Fn::can_cast(parent.kind()));
    }
    if !ast::RecordField::can_cast(node.kind()) {
        return false;
    }
    // Only named struct fields are admitted here. Enum/union/tuple and macro
    // contexts have separate contracts; a real RecordExprField never matches.
    let owned_by_struct = node
        .parent()
        .and_then(ast::RecordFieldList::cast)
        .and_then(|list| list.syntax().parent())
        .is_some_and(|parent| ast::Struct::can_cast(parent.kind()));
    owned_by_struct
        && !node.descendants().any(|child| {
            ast::Expr::can_cast(child.kind()) || ast::MacroCall::can_cast(child.kind())
        })
}

/// Bound the line by parser-owned nontrivia tokens, excluding its trailing
/// separator. Comment text is preserved in the shape, but cannot extend its
/// containment range. Real code after a comment remains in range.
fn declaration_line_span(root: &SyntaxNode, line: Range<usize>) -> Option<Range<usize>> {
    let mut start = None;
    let mut end = None;
    for token in root
        .descendants_with_tokens()
        .filter_map(|element| element.into_token())
    {
        let range = token.text_range();
        let token_start = u32::from(range.start()) as usize;
        let token_end = u32::from(range.end()) as usize;
        if token_start >= line.end {
            break;
        }
        if token_end <= line.start
            || matches!(token.kind(), SyntaxKind::WHITESPACE | SyntaxKind::COMMENT)
        {
            continue;
        }
        // A multiline nontrivia token cannot be a complete declaration-only line.
        if token_start < line.start || token_end > line.end {
            return None;
        }
        if start.is_none() {
            start = Some(token_start);
        }
        if token.kind() != SyntaxKind::COMMA {
            end = Some(token_end);
        }
    }
    let start = start?;
    let end = end?;
    Some(start..end)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::syntax::{RaRustSyntaxAdapter, RustSyntaxAdapter};
    use std::path::Path;

    const SOURCE: &str = "struct Path;\nfn project(\n    out: &Path,\n) {}\n";

    #[test]
    fn parameter_shape_preserves_exact_coordinates_and_unknown_family() -> Result<(), String> {
        for source in [SOURCE.to_string(), SOURCE.replace('\n', "\r\n")] {
            let facts = RaRustSyntaxAdapter.summarize_file(Path::new("src/lib.rs"), &source)?;
            let shape = declaration_shape(&facts, 3, "    out: &Path,")
                .ok_or_else(|| "missing parser-confirmed parameter".to_string())?;
            assert_eq!(shape.family, ProbeFamily::StaticUnknown);
            assert_eq!(shape.start_line, 3);
            assert_eq!(Some(shape.start_byte), source.find("out: &Path,"));
            assert_eq!(shape.text, "out: &Path,");
            assert!(!shape.standalone_call);
            assert!(!shape.unsafe_boundary);
        }
        Ok(())
    }

    #[test]
    fn parameter_shape_requires_current_parser_owned_source() -> Result<(), String> {
        let mut facts = RaRustSyntaxAdapter.summarize_file(Path::new("src/lib.rs"), SOURCE)?;
        assert!(declaration_shape(&facts, 3, "out: &Other,").is_none());
        assert!(declaration_shape(&facts, 0, "out: &Path,").is_none());
        assert!(declaration_shape(&facts, 30, "out: &Path,").is_none());
        facts.used_lexical_fallback = true;
        assert!(declaration_shape(&facts, 3, "out: &Path,").is_none());
        facts.used_lexical_fallback = false;
        facts.source = "struct Path;\nfn project(\n    out: &Path,\n".to_string();
        assert!(declaration_shape(&facts, 3, "out: &Path,").is_none());
        facts.source.clear();
        assert!(declaration_shape(&facts, 3, "out: &Path,").is_none());
        Ok(())
    }

    #[test]
    fn parameter_shape_rejects_non_fn_context_and_shared_body() -> Result<(), String> {
        for (source, line, text) in [
            (
                "struct Path;\nstruct Wrap<'a> { out: &'a Path }\nfn p() -> Wrap<'static> {\n    Wrap {\n        out: &Path,\n    }\n}\n",
                5,
                "out: &Path,",
            ),
            (
                "struct Path;\ntype Handler = fn(\n    out: &Path,\n);\n",
                3,
                "out: &Path,",
            ),
            (
                "struct Path;\nfn project(\n    out: &Path,) -> i32 { 7 }\n",
                3,
                "out: &Path,) -> i32 { 7 }",
            ),
            (
                "struct Path;\nfn project() { let callback = |\n    out: &Path,\n| 7; }\n",
                3,
                "out: &Path,",
            ),
            (
                "struct Path;\nfn project(\n    out: &Path, /* note */) -> i32 { 7 }\n",
                3,
                "out: &Path, /* note */) -> i32 { 7 }",
            ),
            (
                "struct Path;\nfn project(\n    out: &Path, /* note */ other: &Path,\n) {}\n",
                3,
                "out: &Path, /* note */ other: &Path,",
            ),
        ] {
            let facts = RaRustSyntaxAdapter.summarize_file(Path::new("src/lib.rs"), source)?;
            assert!(
                declaration_shape(&facts, line, text).is_none(),
                "non-parameter or shared line was reclassified: {source}"
            );
        }
        Ok(())
    }

    #[test]
    fn parameter_comment_trivia_preserves_text_and_byte_coordinates() -> Result<(), String> {
        for suffix in [
            ", // output: never executable",
            ", /* output */",
            ", /* outer /* nested */ note */ // tail",
            " /* no comma on last parameter */",
        ] {
            for newline in ["\n", "\r\n"] {
                let text = format!("out: &Path{suffix}");
                let source = format!("// λ\nstruct Path;\nfn project(\n    {text}\n) {{}}\n")
                    .replace('\n', newline);
                let facts = RaRustSyntaxAdapter.summarize_file(Path::new("src/lib.rs"), &source)?;
                let shape = declaration_shape(&facts, 4, &text)
                    .ok_or_else(|| format!("commented parameter was lost: {text}"))?;
                assert_eq!(shape.family, ProbeFamily::StaticUnknown);
                assert_eq!(shape.start_line, 4);
                assert_eq!(Some(shape.start_byte), source.find("out: &Path"));
                assert_eq!(shape.text, text);
                assert!(!shape.unsafe_boundary);
            }
        }
        Ok(())
    }

    #[test]
    fn record_fields_preserve_trivia_visibility_and_byte_identity() -> Result<(), String> {
        for text in [
            "value: Marker,",
            "pub value: Marker, // field",
            "pub(crate) value: Marker, /* outer /* nested */ tail */",
            "value: Marker /* final field */",
        ] {
            for newline in ["\n", "\r\n"] {
                let source = format!("// λ\nstruct Marker;\nstruct Packet {{\n    {text}\n}}\n")
                    .replace('\n', newline);
                let facts = RaRustSyntaxAdapter.summarize_file(Path::new("src/lib.rs"), &source)?;
                let shape = declaration_shape(&facts, 4, text)
                    .ok_or_else(|| format!("record declaration was not retained: {text}"))?;
                assert_eq!(shape.family, ProbeFamily::StaticUnknown);
                assert_eq!(shape.text, text);
                assert_eq!(shape.start_line, 4);
                assert_eq!(Some(shape.start_byte), source.find(text));
                assert!(!shape.standalone_call);
                assert!(!shape.unsafe_boundary);
            }
        }
        Ok(())
    }

    #[test]
    fn record_field_fallback_requires_current_valid_source() -> Result<(), String> {
        let source = "struct Packet {\n    value: u8,\n}\n";
        let mut facts = RaRustSyntaxAdapter.summarize_file(Path::new("src/lib.rs"), source)?;
        assert!(declaration_shape(&facts, 2, "value: u8,").is_some());
        assert!(declaration_shape(&facts, 2, "value: u16,").is_none());
        assert!(declaration_shape(&facts, 0, "value: u8,").is_none());
        assert!(declaration_shape(&facts, 99, "value: u8,").is_none());
        facts.used_lexical_fallback = true;
        assert!(declaration_shape(&facts, 2, "value: u8,").is_none());
        facts.used_lexical_fallback = false;
        facts.source = "struct Packet {\n    value: u8,\n".to_string();
        assert!(declaration_shape(&facts, 2, "value: u8,").is_none());
        facts.source.clear();
        assert!(declaration_shape(&facts, 2, "value: u8,").is_none());
        Ok(())
    }

    #[test]
    fn record_field_fallback_rejects_shared_or_unsupported_contexts() -> Result<(), String> {
        for (source, line, text) in [
            (
                "struct Packet {\n    first: u8, /* separator */ second: u8,\n}\n",
                2,
                "first: u8, /* separator */ second: u8,",
            ),
            (
                "struct Packet { value: u8 } fn make() -> Packet { Packet { value: 7 } }\n",
                1,
                "struct Packet { value: u8 } fn make() -> Packet { Packet { value: 7 } }",
            ),
            (
                "enum Packet { Data {\n    value: u8,\n} }\n",
                2,
                "value: u8,",
            ),
            ("union Packet {\n    value: u8,\n}\n", 2, "value: u8,"),
            (
                "const fn width() -> usize { 4 }\nstruct Packet {\n    value: [u8; width()],\n}\n",
                3,
                "value: [u8; width()],",
            ),
            (
                "struct Packet {\n    value: field_type!(),\n}\n",
                2,
                "value: field_type!(),",
            ),
        ] {
            let facts = RaRustSyntaxAdapter.summarize_file(Path::new("src/lib.rs"), source)?;
            assert!(
                declaration_shape(&facts, line, text).is_none(),
                "shared or unsupported field context was admitted: {text}"
            );
        }
        Ok(())
    }
}
