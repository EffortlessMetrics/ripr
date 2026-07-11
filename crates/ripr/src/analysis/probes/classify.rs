use super::super::rust_index::RustIndex;
use super::family::family_for_probe_shape;
use crate::domain::ProbeFamily;
use std::path::Path;

pub fn classify_changed_syntax(
    index: &RustIndex,
    file: &Path,
    line: usize,
    changed_text: &str,
) -> Option<Vec<ProbeFamily>> {
    let facts = index.files.get(file)?;
    let mut families = facts
        .probe_shapes
        .iter()
        .filter(|shape| {
            shape.start_line <= line
                && line <= shape.end_line
                && shape_contains_changed_text(&shape.text, changed_text)
        })
        .filter_map(|shape| family_for_probe_shape(&shape.kind))
        .collect::<Vec<_>>();
    if families.is_empty() {
        return None;
    }
    families.sort_by(|a, b| a.as_str().cmp(b.as_str()));
    families.dedup_by(|a, b| a.as_str() == b.as_str());
    Some(families)
}

pub(crate) fn parser_expression_for_probe<'a>(
    index: &'a RustIndex,
    file: &Path,
    line: usize,
    family: &ProbeFamily,
    changed_text: &str,
) -> Option<&'a str> {
    let facts = index.files.get(file).or_else(|| {
        index
            .files
            .iter()
            .filter(|(indexed_path, _)| file.ends_with(indexed_path))
            .max_by_key(|(indexed_path, _)| indexed_path.as_os_str().len())
            .map(|(_, facts)| facts)
    })?;
    facts
        .probe_shapes
        .iter()
        .filter(|shape| {
            shape.start_line <= line
                && line <= shape.end_line
                && family_for_probe_shape(&shape.kind).as_ref() == Some(family)
                && shape_contains_changed_text(&shape.text, changed_text)
        })
        .min_by(|left, right| {
            left.text
                .len()
                .cmp(&right.text.len())
                .then(left.text.cmp(&right.text))
        })
        .map(|shape| shape.text.as_str())
}

fn shape_contains_changed_text(shape_text: &str, changed_text: &str) -> bool {
    let changed = changed_text
        .trim()
        .trim_end_matches(';')
        .trim_end_matches(',');
    if changed.is_empty() {
        return false;
    }
    let shape = shape_text.trim();
    shape.contains(changed) || changed.contains(shape)
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
    fn classify_changed_syntax_uses_matching_probe_shape() {
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

        let families = classify_changed_syntax(&index, &path, 3, "amount >= threshold;");
        assert_eq!(families, Some(vec![ProbeFamily::Predicate]));
    }

    #[test]
    fn classify_changed_syntax_returns_none_without_matching_shape() {
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

        let families = classify_changed_syntax(&index, &path, 4, "return total");
        assert_eq!(families, None);
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
        Ok(())
    }
}
