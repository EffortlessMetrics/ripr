use super::super::extract::PROBE_SHAPE_UNSAFE_BOUNDARY;
use super::super::rust_index::{FileFacts, ProbeShapeFact, RustIndex};
use super::family::family_for_probe_shape;
use crate::domain::ProbeFamily;
use ra_ap_syntax::{AstNode, Edition, SourceFile, ast};
use std::ops::Range;
use std::path::Path;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ParserProbeShape<'a> {
    pub(crate) family: ProbeFamily,
    pub(crate) start_line: usize,
    pub(crate) start_byte: usize,
    pub(crate) text: &'a str,
    pub(crate) standalone_call: bool,
    pub(crate) unsafe_boundary: bool,
}

pub(crate) fn parser_probe_shapes_for_changed_line<'a>(
    index: &'a RustIndex,
    file: &Path,
    line: usize,
    changed_text: &str,
) -> Vec<ParserProbeShape<'a>> {
    let Some(facts) = file_facts(index, file) else {
        return Vec::new();
    };
    let mut selected = Vec::<ParserProbeShape<'a>>::new();
    for shape in &facts.probe_shapes {
        if shape.start_line > line || line > shape.end_line {
            continue;
        }
        let Some(family) = family_for_probe_shape(&shape.kind) else {
            continue;
        };
        let unsafe_boundary = shape.kind == PROBE_SHAPE_UNSAFE_BOUNDARY;
        if unsafe_boundary && !unsafe_boundary_owns_changed_line(facts, shape, line) {
            continue;
        }
        if !unsafe_boundary && shape_match_rank(&shape.text, changed_text).is_none() {
            continue;
        }
        let candidate = ParserProbeShape {
            family,
            // Unsafe boundaries match by parser-owned containment rather than
            // source text. Project them at the changed line so diff synthesis
            // emits one explicit canonical probe while `start_byte` keeps the
            // stable boundary identity used for deduplication.
            start_line: if unsafe_boundary {
                line
            } else {
                shape.start_line
            },
            start_byte: shape.start_byte,
            text: &shape.text,
            standalone_call: parser_call_shape_is_standalone(facts, shape),
            unsafe_boundary,
        };
        if let Some(position) = selected
            .iter()
            .position(|current| current.family == candidate.family)
        {
            if candidate.unsafe_boundary && selected[position].unsafe_boundary {
                if candidate.start_byte > selected[position].start_byte {
                    selected[position] = candidate;
                }
                continue;
            }
            let current = &selected[position];
            let candidate_rank = shape_match_rank(candidate.text, changed_text);
            let current_rank = shape_match_rank(current.text, changed_text);
            if candidate_rank < current_rank
                || (candidate_rank == current_rank && candidate.text < current.text)
            {
                selected[position] = candidate;
            }
        } else {
            selected.push(candidate);
        }
    }
    selected.sort_by(|left, right| {
        left.family
            .as_str()
            .cmp(right.family.as_str())
            .then(left.start_byte.cmp(&right.start_byte))
            .then(left.text.cmp(right.text))
    });
    selected
}

/// A line-only diff cannot identify which bytes changed when code outside an
/// unsafe boundary shares its opening or closing line. Re-resolve the exact
/// AST range and fail closed on those ambiguous edge lines; interior lines and
/// edge lines wholly owned by the boundary remain eligible.
fn unsafe_boundary_owns_changed_line(
    facts: &FileFacts,
    shape: &ProbeShapeFact,
    line: usize,
) -> bool {
    let Some(boundary) = unsafe_boundary_syntax_range(facts, shape) else {
        return false;
    };
    let Some(line_range) = source_line_byte_range(&facts.source, line) else {
        return false;
    };
    let boundary_start = u32::from(boundary.start()) as usize;
    let boundary_end = u32::from(boundary.end()) as usize;
    if boundary_start >= line_range.end || boundary_end <= line_range.start {
        return false;
    }

    if line == shape.start_line {
        let prefix_end = boundary_start.max(line_range.start).min(line_range.end);
        let Some(prefix) = facts.source.get(line_range.start..prefix_end) else {
            return false;
        };
        if !boundary_edge_is_empty(prefix) {
            return false;
        }
    }
    if line == shape.end_line {
        let suffix_start = boundary_end.max(line_range.start).min(line_range.end);
        let Some(suffix) = facts.source.get(suffix_start..line_range.end) else {
            return false;
        };
        if !boundary_edge_is_empty(suffix) {
            return false;
        }
    }
    true
}

fn unsafe_boundary_syntax_range(
    facts: &FileFacts,
    shape: &ProbeShapeFact,
) -> Option<ra_ap_syntax::TextRange> {
    let parse = SourceFile::parse(&facts.source, Edition::CURRENT);
    if !parse.errors().is_empty() {
        return None;
    }
    let root = parse.tree();
    for function in root.syntax().descendants().filter_map(ast::Fn::cast) {
        let Some(token) = function.unsafe_token() else {
            continue;
        };
        if u32::from(token.text_range().start()) as usize == shape.start_byte {
            return Some(function.syntax().text_range());
        }
    }
    for block in root.syntax().descendants().filter_map(ast::BlockExpr::cast) {
        let Some(token) = block.unsafe_token() else {
            continue;
        };
        if u32::from(token.text_range().start()) as usize == shape.start_byte {
            return Some(block.syntax().text_range());
        }
    }
    None
}

fn source_line_byte_range(source: &str, line: usize) -> Option<Range<usize>> {
    if line == 0 {
        return None;
    }
    let mut start = 0usize;
    for _ in 1..line {
        let next = source.get(start..)?.find('\n')?;
        start = start.saturating_add(next).saturating_add(1);
    }
    let end = source
        .get(start..)?
        .find('\n')
        .map(|offset| start.saturating_add(offset))
        .unwrap_or(source.len());
    Some(start..end)
}

fn boundary_edge_is_empty(text: &str) -> bool {
    text.chars()
        .all(|character| character.is_whitespace() || character == ';')
}

fn parser_call_shape_is_standalone(facts: &FileFacts, shape: &ProbeShapeFact) -> bool {
    if family_for_probe_shape(&shape.kind) != Some(ProbeFamily::CallDeletion) {
        return true;
    }
    let end = shape.start_byte.saturating_add(shape.text.len());
    let Some(before) = facts.source.get(..shape.start_byte) else {
        return false;
    };
    let prefix = before.rsplit('\n').next().unwrap_or(before).trim();
    if !prefix.is_empty() {
        return false;
    }
    let Some(after) = facts.source.get(end..) else {
        return false;
    };
    let suffix = after.split('\n').next().unwrap_or(after).trim_start();
    suffix.starts_with(';') || suffix.starts_with("?;")
}

pub(crate) fn parser_expression_for_probe<'a>(
    index: &'a RustIndex,
    file: &Path,
    line: usize,
    family: &ProbeFamily,
    changed_text: &str,
) -> Option<&'a str> {
    parser_probe_shapes_for_changed_line(index, file, line, changed_text)
        .into_iter()
        .find(|shape| &shape.family == family)
        .map(|shape| shape.text)
}

fn file_facts<'a>(index: &'a RustIndex, file: &Path) -> Option<&'a FileFacts> {
    index.files.get(file).or_else(|| {
        index
            .files
            .iter()
            .filter(|(indexed_path, _)| file.ends_with(indexed_path))
            .max_by_key(|(indexed_path, _)| indexed_path.as_os_str().len())
            .map(|(_, facts)| facts)
    })
}

fn shape_match_rank(shape_text: &str, changed_text: &str) -> Option<(u8, usize)> {
    let changed = changed_text
        .trim()
        .trim_end_matches(';')
        .trim_end_matches(',');
    if changed.is_empty() {
        return None;
    }
    let shape = shape_text.trim();
    if shape == changed {
        Some((0, 0))
    } else if shape.contains(changed) {
        Some((1, shape.len().saturating_sub(changed.len())))
    } else if changed.contains(shape) {
        Some((2, changed.len().saturating_sub(shape.len())))
    } else {
        None
    }
}

pub fn should_ignore_changed_line(text: &str) -> bool {
    text.is_empty()
        || text.starts_with("//")
        || text.starts_with("use ")
        || text.starts_with("pub use ")
        || text.starts_with("mod ")
        || text.starts_with("#")
}

#[cfg(test)]
mod tests {
    use super::super::super::rust_index::{
        FileFacts, PROBE_SHAPE_ERROR_PATH, PROBE_SHAPE_PREDICATE, ProbeShapeFact, RustIndex,
    };
    use super::*;
    use std::collections::BTreeMap;
    use std::path::PathBuf;

    #[test]
    fn classify_functions_are_callable() {
        assert!(should_ignore_changed_line("// comment"));
        assert!(!should_ignore_changed_line("let x = 5;"));
    }

    #[test]
    fn parser_probe_shapes_use_matching_syntax_shape() {
        let path = PathBuf::from("src/lib.rs");
        let index = RustIndex {
            files: BTreeMap::from([(
                path.clone(),
                FileFacts {
                    path: path.clone(),
                    probe_shapes: vec![
                        ProbeShapeFact {
                            start_line: 3,
                            end_line: 3,
                            start_byte: 0,
                            kind: PROBE_SHAPE_PREDICATE.to_string(),
                            text: "if amount >= threshold {".to_string(),
                        },
                        ProbeShapeFact {
                            start_line: 7,
                            end_line: 7,
                            start_byte: 20,
                            kind: PROBE_SHAPE_ERROR_PATH.to_string(),
                            text: "Err(AuthError::Revoked)".to_string(),
                        },
                    ],
                    ..FileFacts::default()
                },
            )]),
            ..RustIndex::default()
        };

        let shapes = parser_probe_shapes_for_changed_line(&index, &path, 3, "amount >= threshold;");
        assert_eq!(shapes.len(), 1);
        assert_eq!(shapes[0].family, ProbeFamily::Predicate);
    }

    #[test]
    fn parser_probe_shapes_are_empty_without_matching_shape() {
        let path = PathBuf::from("src/lib.rs");
        let index = RustIndex {
            files: BTreeMap::from([(
                path.clone(),
                FileFacts {
                    path: path.clone(),
                    probe_shapes: vec![ProbeShapeFact {
                        start_line: 3,
                        end_line: 3,
                        start_byte: 0,
                        kind: PROBE_SHAPE_PREDICATE.to_string(),
                        text: "if amount >= threshold {".to_string(),
                    }],
                    ..FileFacts::default()
                },
            )]),
            ..RustIndex::default()
        };

        let shapes = parser_probe_shapes_for_changed_line(&index, &path, 4, "return total");
        assert!(shapes.is_empty());
    }

    #[test]
    fn unsafe_boundaries_match_by_span_and_prefer_the_innermost_boundary() -> Result<(), String> {
        let path = PathBuf::from("src/lib.rs");
        let source = "pub unsafe fn read_raw(value: i32, limit: i32) -> i32 {\n    unsafe {\n        if value < limit { value } else { limit }\n    }\n}\n";
        let function_start = source
            .find("unsafe fn")
            .ok_or_else(|| "missing unsafe function token".to_string())?;
        let block_start = source
            .rfind("unsafe {")
            .ok_or_else(|| "missing unsafe block token".to_string())?;
        let predicate_start = source
            .find("value < limit")
            .ok_or_else(|| "missing predicate".to_string())?;
        let index = RustIndex {
            files: BTreeMap::from([(
                path.clone(),
                FileFacts {
                    path: path.clone(),
                    source: source.to_string(),
                    probe_shapes: vec![
                        ProbeShapeFact {
                            start_line: 1,
                            end_line: 5,
                            start_byte: function_start,
                            kind: PROBE_SHAPE_UNSAFE_BOUNDARY.to_string(),
                            text: "unsafe fn read_raw".to_string(),
                        },
                        ProbeShapeFact {
                            start_line: 2,
                            end_line: 4,
                            start_byte: block_start,
                            kind: PROBE_SHAPE_UNSAFE_BOUNDARY.to_string(),
                            text: "unsafe block".to_string(),
                        },
                        ProbeShapeFact {
                            start_line: 3,
                            end_line: 3,
                            start_byte: predicate_start,
                            kind: PROBE_SHAPE_PREDICATE.to_string(),
                            text: "value < limit".to_string(),
                        },
                    ],
                    ..FileFacts::default()
                },
            )]),
            ..RustIndex::default()
        };

        let shapes = parser_probe_shapes_for_changed_line(&index, &path, 3, "value < limit");
        assert_eq!(shapes.len(), 2);
        assert!(
            shapes
                .iter()
                .any(|shape| shape.family == ProbeFamily::Predicate)
        );
        let unsafe_shape = shapes
            .iter()
            .find(|shape| shape.family == ProbeFamily::StaticUnknown)
            .ok_or_else(|| "missing unsafe boundary shape".to_string())?;
        assert_eq!(unsafe_shape.start_line, 3);
        assert_eq!(unsafe_shape.start_byte, block_start);
        assert_eq!(unsafe_shape.text, "unsafe block");
        assert!(unsafe_shape.unsafe_boundary);
        Ok(())
    }

    #[test]
    fn unsafe_boundary_rejects_shared_edge_lines_with_outside_code() -> Result<(), String> {
        let path = PathBuf::from("src/lib.rs");
        let source = "pub fn read(ptr: *const u8) -> u8 { let limit = 2; unsafe { ptr.read() } }\n";
        let block_start = source
            .find("unsafe {")
            .ok_or_else(|| "missing unsafe block token".to_string())?;
        let index = RustIndex {
            files: BTreeMap::from([(
                path.clone(),
                FileFacts {
                    path: path.clone(),
                    source: source.to_string(),
                    probe_shapes: vec![ProbeShapeFact {
                        start_line: 1,
                        end_line: 1,
                        start_byte: block_start,
                        kind: PROBE_SHAPE_UNSAFE_BOUNDARY.to_string(),
                        text: "unsafe block".to_string(),
                    }],
                    ..FileFacts::default()
                },
            )]),
            ..RustIndex::default()
        };

        let shapes = parser_probe_shapes_for_changed_line(
            &index,
            &path,
            1,
            "pub fn read(ptr: *const u8) -> u8 { let limit = 3; unsafe { ptr.read() } }",
        );
        assert!(shapes.is_empty());
        Ok(())
    }

    #[test]
    fn unsafe_boundary_accepts_an_edge_line_owned_by_the_boundary() -> Result<(), String> {
        let path = PathBuf::from("src/lib.rs");
        let source = "pub fn read(ptr: *const u8) -> u8 {\n    unsafe { ptr.read() }\n}\n";
        let block_start = source
            .find("unsafe {")
            .ok_or_else(|| "missing unsafe block token".to_string())?;
        let index = RustIndex {
            files: BTreeMap::from([(
                path.clone(),
                FileFacts {
                    path: path.clone(),
                    source: source.to_string(),
                    probe_shapes: vec![ProbeShapeFact {
                        start_line: 2,
                        end_line: 2,
                        start_byte: block_start,
                        kind: PROBE_SHAPE_UNSAFE_BOUNDARY.to_string(),
                        text: "unsafe block".to_string(),
                    }],
                    ..FileFacts::default()
                },
            )]),
            ..RustIndex::default()
        };

        let shapes =
            parser_probe_shapes_for_changed_line(&index, &path, 2, "unsafe { ptr.add(1).read() }");
        assert_eq!(shapes.len(), 1);
        assert_eq!(shapes[0].family, ProbeFamily::StaticUnknown);
        Ok(())
    }

    #[test]
    fn parser_call_shapes_preserve_inventory_and_mark_diff_eligibility() -> Result<(), String> {
        let path = PathBuf::from("src/lib.rs");
        let source = "fn demo() {\n    read()?;\n    let value = read()?;\n}\n";
        let first = source
            .find("read()")
            .ok_or_else(|| "missing standalone call".to_string())?;
        let second = source
            .rfind("read()")
            .ok_or_else(|| "missing initializer call".to_string())?;
        let index = RustIndex {
            files: BTreeMap::from([(
                path.clone(),
                FileFacts {
                    path: path.clone(),
                    source: source.to_string(),
                    probe_shapes: vec![
                        ProbeShapeFact {
                            start_line: 2,
                            end_line: 2,
                            start_byte: first,
                            kind: crate::analysis::rust_index::PROBE_SHAPE_CALL_DELETION
                                .to_string(),
                            text: "read()".to_string(),
                        },
                        ProbeShapeFact {
                            start_line: 3,
                            end_line: 3,
                            start_byte: second,
                            kind: crate::analysis::rust_index::PROBE_SHAPE_CALL_DELETION
                                .to_string(),
                            text: "read()".to_string(),
                        },
                    ],
                    ..FileFacts::default()
                },
            )]),
            ..RustIndex::default()
        };

        let standalone = parser_probe_shapes_for_changed_line(&index, &path, 2, "read()?;");
        let initializer =
            parser_probe_shapes_for_changed_line(&index, &path, 3, "let value = read()?;");
        if standalone.len() != 1 || !standalone[0].standalone_call {
            return Err(format!("standalone call was not retained: {standalone:?}"));
        }
        if initializer.len() != 1 || initializer[0].standalone_call {
            return Err(format!("initializer call was promoted: {initializer:?}"));
        }
        Ok(())
    }

    #[test]
    fn parser_expression_resolves_absolute_probe_to_most_specific_shape() -> Result<(), String> {
        let path = PathBuf::from("src/gate_watchdog.rs");
        let index = RustIndex {
            files: BTreeMap::from([(
                path,
                FileFacts {
                    path: PathBuf::from("src/gate_watchdog.rs"),
                    probe_shapes: vec![
                        ProbeShapeFact {
                            start_line: 10,
                            end_line: 13,
                            start_byte: 100,
                            kind: crate::analysis::rust_index::PROBE_SHAPE_CALL_DELETION
                                .to_string(),
                            text: "watchdog_reason(\n    \"run-missing\",\n    receipt,\n)"
                                .to_string(),
                        },
                        ProbeShapeFact {
                            start_line: 10,
                            end_line: 15,
                            start_byte: 90,
                            kind: crate::analysis::rust_index::PROBE_SHAPE_CALL_DELETION
                                .to_string(),
                            text: "with_reason(watchdog_reason(\"run-missing\", receipt))"
                                .to_string(),
                        },
                    ],
                    ..FileFacts::default()
                },
            )]),
            ..RustIndex::default()
        };

        let expression = parser_expression_for_probe(
            &index,
            Path::new("/repo/ub-review/src/gate_watchdog.rs"),
            10,
            &ProbeFamily::CallDeletion,
            "watchdog_reason(",
        )
        .ok_or_else(|| "expected parser expression".to_string())?;
        if expression != "watchdog_reason(\n    \"run-missing\",\n    receipt,\n)" {
            return Err(format!("unexpected parser expression: {expression}"));
        }
        let outer = parser_expression_for_probe(
            &index,
            Path::new("/repo/ub-review/src/gate_watchdog.rs"),
            10,
            &ProbeFamily::CallDeletion,
            "with_reason(watchdog_reason(\"run-missing\", receipt))",
        )
        .ok_or_else(|| "expected outer parser expression".to_string())?;
        if outer != "with_reason(watchdog_reason(\"run-missing\", receipt))" {
            return Err(format!("nested shape replaced exact outer call: {outer}"));
        }
        Ok(())
    }
}
