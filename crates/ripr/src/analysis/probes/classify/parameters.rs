//! Parser-confirmed parameter declarations stay unknown, not executable fields.

use super::super::lexical::classify_changed_line;
use super::{ParserProbeShape, source_line_byte_range};
use crate::analysis::rust_index::FileFacts;
use crate::domain::ProbeFamily;
use ra_ap_syntax::{AstNode, Edition, SourceFile, ast};

/// This fallback is used only after ordinary parser-owned expressions have
/// been considered. It never promotes exposure or drops the changed source.
/// Exact current-line equality prevents borrowing new-side context for a
/// different removed/stale line. Shared signature/body lines are not eligible.
pub(super) fn parameter_declaration_shape<'a>(
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
    let start_byte = line_range.start + source_line.len() - source_line.trim_start().len();
    let declaration = text.strip_suffix(',').unwrap_or(text).trim_end();
    let end_byte = start_byte + declaration.len();

    // Reuse the existing parser, not a colon/capitalization heuristic. Parsing
    // happens only for unmatched lexical field candidates; real expression shapes
    // take precedence and do not enter this fallback.
    let parse = SourceFile::parse(&facts.source, Edition::CURRENT);
    if !parse.errors().is_empty() {
        return None;
    }
    for parameter in parse
        .tree()
        .syntax()
        .descendants()
        .filter_map(ast::Param::cast)
    {
        let Some(list) = parameter.syntax().parent().and_then(ast::ParamList::cast) else {
            continue;
        };
        if list.syntax().parent().and_then(ast::Fn::cast).is_none() {
            continue;
        }
        let range = parameter.syntax().text_range();
        let parameter_start = u32::from(range.start()) as usize;
        let parameter_end = u32::from(range.end()) as usize;
        if parameter_start <= start_byte && end_byte <= parameter_end {
            return Some(ParserProbeShape {
                family: ProbeFamily::StaticUnknown,
                start_line: line,
                start_byte,
                text,
                standalone_call: false,
                unsafe_boundary: false,
            });
        }
    }
    None
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
            let shape = parameter_declaration_shape(&facts, 3, "    out: &Path,")
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
        assert!(parameter_declaration_shape(&facts, 3, "out: &Other,").is_none());
        assert!(parameter_declaration_shape(&facts, 0, "out: &Path,").is_none());
        assert!(parameter_declaration_shape(&facts, 30, "out: &Path,").is_none());
        facts.used_lexical_fallback = true;
        assert!(parameter_declaration_shape(&facts, 3, "out: &Path,").is_none());
        facts.used_lexical_fallback = false;
        facts.source = "struct Path;\nfn project(\n    out: &Path,\n".to_string();
        assert!(parameter_declaration_shape(&facts, 3, "out: &Path,").is_none());
        facts.source.clear();
        assert!(parameter_declaration_shape(&facts, 3, "out: &Path,").is_none());
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
        ] {
            let facts = RaRustSyntaxAdapter.summarize_file(Path::new("src/lib.rs"), source)?;
            assert!(
                parameter_declaration_shape(&facts, line, text).is_none(),
                "non-parameter or shared line was reclassified: {source}"
            );
        }
        Ok(())
    }
}
