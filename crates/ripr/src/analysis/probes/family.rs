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

#[cfg(test)]
mod tests {
    use super::*;

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
