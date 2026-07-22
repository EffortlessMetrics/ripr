/// RIPR-SPEC-0113: honest per-finding guidance for `no_static_path` findings.
///
/// Defined once here in `domain/` so both `analysis::classify::decision` (production path)
/// and `analysis::classifier` (test assertions) can reference the same literal without
/// duplicating it. Exported via `crate::domain::*`.
pub(crate) const NO_STATIC_PATH_NEXT_STEP: &str = "ripr found no static test path to this change \u{2014} this is not a coverage assessment. \
A test may already exercise it through macros, helper-call chains, or integration tests that \
ripr's static model does not yet trace. If none does, add a co-located test that reaches and \
observes the changed behavior so a discriminator exists.";

/// RIPR-SPEC-0133: shared one-sentence reason an owner counts as an
/// assertion-shaped oracle. Kept in `domain/` so the detector
/// (`analysis::classify::owner_shape`), the `Finding.evidence` disclosure line,
/// and the guidance strings below state the rule identically (a unit test pins
/// each guidance string to contain this clause).
pub(crate) const ASSERTION_SHAPED_OWNER_REASON: &str =
    "its body is dominated by assert*/expect calls and nothing outside tests calls it";

/// RIPR-SPEC-0133: per-class `recommended_next_step` text when the changed
/// owner is itself an assertion-shaped helper (an oracle), so the standard
/// code-under-test advice would ask for a test of the test. The exposure class
/// is unchanged; only the guidance is reframed for oracle-shaped owners. Each
/// string embeds `ASSERTION_SHAPED_OWNER_REASON` verbatim so the rule is
/// explainable in one sentence in output.
pub(crate) const ASSERTION_SHAPED_NO_STATIC_PATH_NEXT_STEP: &str = "The changed owner is itself an assertion helper (its body is dominated by assert*/expect calls and \
nothing outside tests calls it), so a test that observes it would be a test of the oracle. Sharpen its \
assertions, or exercise it indirectly through the code it checks.";

pub(crate) const ASSERTION_SHAPED_WEAKLY_EXPOSED_NEXT_STEP: &str = "The changed owner is an assertion helper (its body is dominated by assert*/expect calls and nothing \
outside tests calls it), so exact-equality advice for code under test may not apply — boolean invariants \
have no exact equality. Tighten the loosest assertion in this helper: a sharper predicate, an exact \
expected value where one exists, or a case the current assertions would not flag.";

pub(crate) const ASSERTION_SHAPED_REACHABLE_UNREVEALED_NEXT_STEP: &str = "The changed owner is an assertion helper (its body is dominated by assert*/expect calls and nothing \
outside tests calls it); it is the observation, not the code under observation. Ensure at least one test \
calls this helper over inputs that exercise the changed check.";

pub(crate) const ASSERTION_SHAPED_INFECTION_UNKNOWN_NEXT_STEP: &str = "The changed owner is an assertion helper (its body is dominated by assert*/expect calls and nothing \
outside tests calls it); there is no fixture/builder for ripr.toml to describe. Add a boundary or \
negative-path case for the code this helper checks.";

/// RIPR-SPEC-0115: stable leading phrase of the transitive-reach *witness*
/// pointer. The producer (`analysis::classify::transitive_reach`) begins the
/// pointer with this phrase, and the human renderer (`output::human`) recognizes
/// it in `Finding.evidence` to surface a concrete "Where to look" line. Shared
/// here in `domain/` so the producer and renderer agree on one literal across
/// the analysis/output seam (reuse, don't fork).
pub(crate) const TRANSITIVE_REACH_WITNESS_PREFIX: &str = "For example, the test ";

/// RIPR-SPEC-0114/0117: stable evidence prefixes for named static-limitation
/// detail. Producers append these lines to `Finding.evidence`; renderers and
/// corpus checks consume the same prefixes so the unresolved edge stays visible
/// across JSON and human projections.
pub(crate) const LIMITATION_LAST_ESTABLISHED_EDGE_PREFIX: &str =
    "limitation_last_established_edge: ";
pub(crate) const LIMITATION_FIRST_UNRESOLVED_EDGE_PREFIX: &str =
    "limitation_first_unresolved_edge: ";
pub(crate) const LIMITATION_ANALYZER_ROUTE_PREFIX: &str = "limitation_analyzer_route: ";
pub(crate) const LIMITATION_NON_CLAIM_PREFIX: &str = "limitation_non_claim: ";

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ExposureClass {
    Exposed,
    WeaklyExposed,
    ReachableUnrevealed,
    NoStaticPath,
    InfectionUnknown,
    PropagationUnknown,
    StaticUnknown,
}

impl ExposureClass {
    pub fn as_str(&self) -> &'static str {
        profile::for_class(self).label
    }

    pub fn severity(&self) -> &'static str {
        profile::for_class(self).severity
    }

    pub fn requires_stop_reason(&self) -> bool {
        profile::for_class(self).requires_stop_reason
    }
}

mod profile {
    use super::ExposureClass;

    pub(super) struct ExposureProfile {
        pub(super) label: &'static str,
        pub(super) severity: &'static str,
        pub(super) requires_stop_reason: bool,
    }

    pub(super) fn for_class(class: &ExposureClass) -> ExposureProfile {
        match class {
            ExposureClass::Exposed => ExposureProfile {
                label: "exposed",
                severity: "info",
                requires_stop_reason: false,
            },
            ExposureClass::WeaklyExposed => ExposureProfile {
                label: "weakly_exposed",
                severity: "warning",
                requires_stop_reason: false,
            },
            ExposureClass::ReachableUnrevealed => ExposureProfile {
                label: "reachable_unrevealed",
                severity: "warning",
                requires_stop_reason: false,
            },
            ExposureClass::NoStaticPath => ExposureProfile {
                label: "no_static_path",
                severity: "warning",
                requires_stop_reason: false,
            },
            ExposureClass::InfectionUnknown => ExposureProfile {
                label: "infection_unknown",
                severity: "warning",
                requires_stop_reason: true,
            },
            ExposureClass::PropagationUnknown => ExposureProfile {
                label: "propagation_unknown",
                severity: "note",
                requires_stop_reason: true,
            },
            ExposureClass::StaticUnknown => ExposureProfile {
                label: "static_unknown",
                severity: "note",
                requires_stop_reason: true,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::ExposureClass;
    use std::collections::HashSet;

    fn all_exposure_classes() -> [ExposureClass; 7] {
        [
            ExposureClass::Exposed,
            ExposureClass::WeaklyExposed,
            ExposureClass::ReachableUnrevealed,
            ExposureClass::NoStaticPath,
            ExposureClass::InfectionUnknown,
            ExposureClass::PropagationUnknown,
            ExposureClass::StaticUnknown,
        ]
    }

    #[test]
    fn exposure_class_strings_match_contract_terms() {
        let cases = [
            (ExposureClass::Exposed, "exposed"),
            (ExposureClass::WeaklyExposed, "weakly_exposed"),
            (ExposureClass::ReachableUnrevealed, "reachable_unrevealed"),
            (ExposureClass::NoStaticPath, "no_static_path"),
            (ExposureClass::InfectionUnknown, "infection_unknown"),
            (ExposureClass::PropagationUnknown, "propagation_unknown"),
            (ExposureClass::StaticUnknown, "static_unknown"),
        ];

        for (class, expected) in cases {
            assert_eq!(class.as_str(), expected);
        }
    }

    #[test]
    fn exposure_class_severities_match_output_expectations() {
        let cases = [
            (ExposureClass::Exposed, "info"),
            (ExposureClass::WeaklyExposed, "warning"),
            (ExposureClass::ReachableUnrevealed, "warning"),
            (ExposureClass::NoStaticPath, "warning"),
            (ExposureClass::InfectionUnknown, "warning"),
            (ExposureClass::PropagationUnknown, "note"),
            (ExposureClass::StaticUnknown, "note"),
        ];

        for (class, expected) in cases {
            assert_eq!(class.severity(), expected);
        }
    }

    #[test]
    fn stop_reason_requirement_is_only_for_unknown_classes() {
        assert!(!ExposureClass::Exposed.requires_stop_reason());
        assert!(!ExposureClass::WeaklyExposed.requires_stop_reason());
        assert!(!ExposureClass::ReachableUnrevealed.requires_stop_reason());
        assert!(!ExposureClass::NoStaticPath.requires_stop_reason());
        assert!(ExposureClass::InfectionUnknown.requires_stop_reason());
        assert!(ExposureClass::PropagationUnknown.requires_stop_reason());
        assert!(ExposureClass::StaticUnknown.requires_stop_reason());
    }

    #[test]
    fn exposure_class_contract_terms_are_unique() {
        let mut seen = HashSet::new();

        for class in all_exposure_classes() {
            assert!(
                seen.insert(class.as_str()),
                "duplicate contract term found for {}",
                class.as_str()
            );
        }

        assert_eq!(seen.len(), 7, "every class should map to a unique term");
    }

    #[test]
    fn exposure_class_severity_values_stay_in_supported_set() {
        let supported = HashSet::from(["info", "warning", "note"]);
        for class in all_exposure_classes() {
            assert!(
                supported.contains(class.severity()),
                "unsupported severity {} for {}",
                class.severity(),
                class.as_str()
            );
        }
    }
}
