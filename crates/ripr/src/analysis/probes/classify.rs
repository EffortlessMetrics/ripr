use super::super::rust_index::{FileFacts, RustIndex};
use super::family::family_for_probe_shape;
use crate::domain::ProbeFamily;
use std::path::Path;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ParserProbeShape<'a> {
    pub(crate) family: ProbeFamily,
    pub(crate) start_line: usize,
    pub(crate) start_byte: usize,
    pub(crate) text: &'a str,
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
        if shape_match_rank(&shape.text, changed_text).is_none() {
            continue;
        }
        let candidate = ParserProbeShape {
            family,
            start_line: shape.start_line,
            start_byte: shape.start_byte,
            text: &shape.text,
        };
        if let Some(position) = selected
            .iter()
            .position(|current| current.family == candidate.family)
        {
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
