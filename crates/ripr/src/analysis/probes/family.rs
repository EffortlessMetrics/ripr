use super::super::rust_index::{
    PROBE_SHAPE_CALL_DELETION, PROBE_SHAPE_ERROR_PATH, PROBE_SHAPE_FIELD_CONSTRUCTION,
    PROBE_SHAPE_MATCH_ARM, PROBE_SHAPE_PREDICATE, PROBE_SHAPE_RETURN_VALUE,
    PROBE_SHAPE_SIDE_EFFECT,
};
use crate::domain::{DeltaKind, ProbeFamily};

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

pub fn delta_for_family(family: &ProbeFamily) -> DeltaKind {
    match family {
        ProbeFamily::Predicate | ProbeFamily::MatchArm => DeltaKind::Control,
        ProbeFamily::SideEffect | ProbeFamily::CallDeletion => DeltaKind::Effect,
        ProbeFamily::ReturnValue | ProbeFamily::ErrorPath | ProbeFamily::FieldConstruction => {
            DeltaKind::Value
        }
        ProbeFamily::StaticUnknown => DeltaKind::Unknown,
    }
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
    use super::*;

    #[test]
    fn classify_changed_line_detects_core_probe_shapes() {
        let cases = [
            ("if x > 5 { }", ProbeFamily::Predicate),
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
    fn family_metadata_covers_every_probe_family() {
        let cases = [
            (ProbeFamily::Predicate, DeltaKind::Control),
            (ProbeFamily::ReturnValue, DeltaKind::Value),
            (ProbeFamily::ErrorPath, DeltaKind::Value),
            (ProbeFamily::CallDeletion, DeltaKind::Effect),
            (ProbeFamily::FieldConstruction, DeltaKind::Value),
            (ProbeFamily::SideEffect, DeltaKind::Effect),
            (ProbeFamily::MatchArm, DeltaKind::Control),
            (ProbeFamily::StaticUnknown, DeltaKind::Unknown),
        ];

        for (family, delta) in cases {
            assert_eq!(delta_for_family(&family), delta);
        }
    }

    #[test]
    fn family_for_probe_shape_maps_known_shape_strings() {
        let cases = [
            (PROBE_SHAPE_PREDICATE, ProbeFamily::Predicate),
            (PROBE_SHAPE_RETURN_VALUE, ProbeFamily::ReturnValue),
            (PROBE_SHAPE_ERROR_PATH, ProbeFamily::ErrorPath),
            (PROBE_SHAPE_CALL_DELETION, ProbeFamily::CallDeletion),
            (
                PROBE_SHAPE_FIELD_CONSTRUCTION,
                ProbeFamily::FieldConstruction,
            ),
            (PROBE_SHAPE_SIDE_EFFECT, ProbeFamily::SideEffect),
            (PROBE_SHAPE_MATCH_ARM, ProbeFamily::MatchArm),
        ];

        for (shape, family) in cases {
            assert_eq!(family_for_probe_shape(shape), Some(family));
        }
        assert_eq!(family_for_probe_shape("opaque_shape"), None);
    }
}
