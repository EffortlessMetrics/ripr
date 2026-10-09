use super::super::facts::ProbeShapeKind;
use crate::domain::{DeltaKind, ProbeFamily};

pub fn family_for_probe_shape(kind: ProbeShapeKind) -> ProbeFamily {
    // Total: the closed kind vocabulary (#5415 step 1) cannot hold an
    // unknown string. Unknown wire strings fail at the decode boundary and
    // the cache entry takes the corrupt-entry quarantine path.
    match kind {
        ProbeShapeKind::Predicate => ProbeFamily::Predicate,
        ProbeShapeKind::ReturnValue => ProbeFamily::ReturnValue,
        ProbeShapeKind::ErrorPath => ProbeFamily::ErrorPath,
        ProbeShapeKind::CallDeletion => ProbeFamily::CallDeletion,
        ProbeShapeKind::FieldConstruction => ProbeFamily::FieldConstruction,
        ProbeShapeKind::SideEffect => ProbeFamily::SideEffect,
        ProbeShapeKind::MatchArm => ProbeFamily::MatchArm,
        ProbeShapeKind::UnsafeBoundary => ProbeFamily::StaticUnknown,
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
    fn family_for_probe_shape_maps_every_kind() {
        let cases = [
            (ProbeShapeKind::Predicate, ProbeFamily::Predicate),
            (ProbeShapeKind::ReturnValue, ProbeFamily::ReturnValue),
            (ProbeShapeKind::ErrorPath, ProbeFamily::ErrorPath),
            (ProbeShapeKind::CallDeletion, ProbeFamily::CallDeletion),
            (
                ProbeShapeKind::FieldConstruction,
                ProbeFamily::FieldConstruction,
            ),
            (ProbeShapeKind::SideEffect, ProbeFamily::SideEffect),
            (ProbeShapeKind::MatchArm, ProbeFamily::MatchArm),
            (ProbeShapeKind::UnsafeBoundary, ProbeFamily::StaticUnknown),
        ];

        for (shape, family) in cases {
            assert_eq!(family_for_probe_shape(shape), family);
        }
        // Unknown wire strings no longer reach this function: they fail at
        // the decode boundary (see probe_shape_kind_rejects_unknown_wire_strings_at_decode).
    }
}
