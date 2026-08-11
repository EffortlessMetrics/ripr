pub const PROBE_SHAPE_PREDICATE: &str = "predicate";
pub const PROBE_SHAPE_RETURN_VALUE: &str = "return_value";
pub const PROBE_SHAPE_ERROR_PATH: &str = "error_path";
pub const PROBE_SHAPE_CALL_DELETION: &str = "call_deletion";
pub const PROBE_SHAPE_FIELD_CONSTRUCTION: &str = "field_construction";
pub const PROBE_SHAPE_SIDE_EFFECT: &str = "side_effect";
pub const PROBE_SHAPE_MATCH_ARM: &str = "match_arm";

/// All known probe shape names, in canonical order.
pub const KNOWN_PROBE_SHAPES: &[&str] = &[
    PROBE_SHAPE_PREDICATE,
    PROBE_SHAPE_RETURN_VALUE,
    PROBE_SHAPE_ERROR_PATH,
    PROBE_SHAPE_CALL_DELETION,
    PROBE_SHAPE_FIELD_CONSTRUCTION,
    PROBE_SHAPE_SIDE_EFFECT,
    PROBE_SHAPE_MATCH_ARM,
];

/// Returns true when `name` is exactly one of the known probe shapes.
///
/// This rejects unknown/typo'd shape names so downstream analysis does not
/// silently accept an invalid probe classification.
pub fn is_known_probe_shape(name: &str) -> bool {
    KNOWN_PROBE_SHAPES.contains(&name)
}

#[cfg(test)]
mod tests {
    use super::{KNOWN_PROBE_SHAPES, is_known_probe_shape};

    #[test]
    fn known_probe_shape_names_are_exact() -> Result<(), String> {
        for shape in KNOWN_PROBE_SHAPES {
            if !is_known_probe_shape(shape) {
                return Err(format!("expected `{shape}` to be known"));
            }
        }

        for unknown in [
            "",
            "not_return_value",
            "return_value_extra",
            "predicate ",
            "side-effect",
            "MATCH_ARM",
        ] {
            if is_known_probe_shape(unknown) {
                return Err(format!("expected `{unknown}` to stay unknown"));
            }
        }

        Ok(())
    }
}
