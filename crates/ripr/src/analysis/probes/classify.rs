use super::super::rust_index::{
    PROBE_SHAPE_CALL_DELETION, PROBE_SHAPE_ERROR_PATH, PROBE_SHAPE_FIELD_CONSTRUCTION,
    PROBE_SHAPE_MATCH_ARM, PROBE_SHAPE_PREDICATE, PROBE_SHAPE_RETURN_VALUE,
    PROBE_SHAPE_SIDE_EFFECT, RustIndex,
};
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

pub fn family_for_probe_shape(kind: &str) -> Option<ProbeFamily> {
    match kind {
        PROBE_SHAPE_PREDICATE => Some(ProbeFamily::Predicate),
        PROBE_SHAPE_RETURN_VALUE => Some(ProbeFamily::ReturnValue),
        PROBE_SHAPE_ERROR_PATH => Some(ProbeFamily::ErrorPath),
        PROBE_SHAPE_CALL_DELETION => Some(ProbeFamily::CallDeletion),
        PROBE_SHAPE_FIELD_CONSTRUCTION => Some(ProbeFamily::FieldConstruction),
        PROBE_SHAPE_SIDE_EFFECT => Some(ProbeFamily::SideEffect),
        PROBE_SHAPE_MATCH_ARM => Some(ProbeFamily::MatchArm),
        _ => None,
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

pub fn classify_changed_line(text: &str) -> Vec<ProbeFamily> {
    let mut out = Vec::new();
    if has_predicate_shape(text) {
        out.push(ProbeFamily::Predicate);
    }
    if has_error_shape(text) {
        out.push(ProbeFamily::ErrorPath);
    }
    if has_return_shape(text) {
        out.push(ProbeFamily::ReturnValue);
    }
    if has_effect_shape(text) {
        out.push(ProbeFamily::SideEffect);
    }
    if has_call_shape(text) {
        out.push(ProbeFamily::CallDeletion);
    }
    if has_field_shape(text) {
        out.push(ProbeFamily::FieldConstruction);
    }
    if text.starts_with("match ") || text.contains("=>") {
        out.push(ProbeFamily::MatchArm);
    }
    if out.is_empty() {
        out.push(ProbeFamily::StaticUnknown);
    }
    out.sort_by(|a, b| a.as_str().cmp(b.as_str()));
    out.dedup_by(|a, b| a.as_str() == b.as_str());
    out
}

fn has_predicate_shape(text: &str) -> bool {
    text.contains(" if ")
        || text.starts_with("if ")
        || text.starts_with("while ")
        || text.contains(" >= ")
        || text.contains(" <= ")
        || text.contains(" > ")
        || text.contains(" < ")
        || text.contains(" == ")
        || text.contains(" != ")
        || text.contains("&&")
        || text.contains("||")
}

fn has_return_shape(text: &str) -> bool {
    text.starts_with("return ")
        || text.contains(" Ok(")
        || text.starts_with("Ok(")
        || text.contains(" Some(")
        || text.starts_with("Some(")
        || text.contains("None")
        || text.contains("return")
}

fn has_error_shape(text: &str) -> bool {
    text.contains("Err(")
        || text.contains("Error::")
        || text.contains("map_err")
        || text.contains("bail!")
        || text.contains("anyhow!")
        || text.contains("?") && text.contains("Err")
}

fn has_effect_shape(text: &str) -> bool {
    let lower = text.to_ascii_lowercase();
    [
        ".save(",
        ".publish(",
        ".send(",
        ".write(",
        ".insert(",
        ".push(",
        ".remove(",
        ".delete(",
        ".emit(",
        ".increment(",
        "metrics.",
        "log::",
    ]
    .iter()
    .any(|needle| lower.contains(needle))
}

fn has_call_shape(text: &str) -> bool {
    text.contains('(')
        && text.contains(')')
        && !text.starts_with("fn ")
        && !text.starts_with("pub fn ")
        && !text.contains("assert")
}

fn has_field_shape(text: &str) -> bool {
    text.contains(':') && !text.contains("::") && !text.starts_with("fn ")
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

        let families = classify_changed_line("if x > 5 { }");
        assert!(families.contains(&ProbeFamily::Predicate));
    }

    #[test]
    fn classify_changed_line_detects_core_probe_shapes() {
        let cases = [
            ("return Ok(total)", ProbeFamily::ReturnValue),
            ("Err(AuthError::Revoked)", ProbeFamily::ErrorPath),
            ("events.publish(invoice)", ProbeFamily::SideEffect),
            ("send_invoice(invoice)", ProbeFamily::CallDeletion),
            ("total: discounted_total", ProbeFamily::FieldConstruction),
            ("match status {", ProbeFamily::MatchArm),
            ("Status::Ready => total", ProbeFamily::MatchArm),
            ("let value = total;", ProbeFamily::StaticUnknown),
        ];

        for (text, expected) in cases {
            let families = classify_changed_line(text);
            assert!(
                families.contains(&expected),
                "{text} did not classify as {}",
                expected.as_str()
            );
        }
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
}
