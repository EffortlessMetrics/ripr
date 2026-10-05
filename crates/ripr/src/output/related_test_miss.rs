//! One sentence per examined test saying why it would not notice the change.
//!
//! The analyzer records the fact as a [`RelatedTestMiss`]; this module is the
//! single place that turns it into prose, so human, JSON, LSP and agent output
//! say the same thing about the same test.

use crate::domain::{
    MissingDiscriminatorFact, ProbeFamily, RelatedTest, RelatedTestMiss, RelationReason,
    exact_assertion_fact, input_boundary_fact,
};

/// Short reason the test misses the changed behavior, or `None` when the
/// analyzer did not establish one.
pub(crate) fn related_test_miss_reason(
    test: &RelatedTest,
    missing_discriminators: &[MissingDiscriminatorFact],
) -> Option<String> {
    Some(match test.miss? {
        RelatedTestMiss::NoCallPath => match test.relation_reason.and_then(link_label) {
            Some(link) => format!("no call to the changed code found; linked by {link} only"),
            None => "no call to the changed code found".to_string(),
        },
        RelatedTestMiss::NoAssertion => "has no assertion".to_string(),
        RelatedTestMiss::AssertionNotObserving => {
            "asserts, but not on the changed value".to_string()
        }
        RelatedTestMiss::AssertionNotCredited => {
            "assertion not credited: ripr could not establish that it runs as the standard macro"
                .to_string()
        }
        RelatedTestMiss::WeakAssertion => {
            "assertion too weak to tell the old behavior from the new".to_string()
        }
        // #5508: an unknown observation edge, not an established miss. Only
        // `assertion_not_observing` claims the assertion observes something
        // else.
        RelatedTestMiss::ObservationUnconfirmed => {
            "ripr could not confirm that this assertion observes the changed behavior".to_string()
        }
        // The analyzer assigns `missing_input` only for a predicate probe
        // with a boundary fact, and `missing_exact_assertion` only when no
        // boundary fact exists, so reading the facts as a predicate's picks
        // the fact each miss was assigned from.
        RelatedTestMiss::MissingInput => {
            match input_boundary_fact(missing_discriminators, &ProbeFamily::Predicate) {
                Some(fact) => format!("no test input reaches `{}`", one_line(&fact.value)),
                None => "no test input reaches the changed boundary".to_string(),
            }
        }
        RelatedTestMiss::MissingExactAssertion => {
            match exact_assertion_fact(missing_discriminators, &ProbeFamily::Predicate) {
                Some(fact) => format!("no assertion pins `{}`", one_line(&fact.value)),
                None => "no assertion pins the exact changed value".to_string(),
            }
        }
    })
}

/// The word placed before the reason. An unconfirmed observation is an
/// unknown, so it is not introduced as a miss (#5508).
pub(crate) fn related_test_miss_label(test: &RelatedTest) -> &'static str {
    match test.miss {
        Some(RelatedTestMiss::ObservationUnconfirmed) => "unconfirmed",
        _ => "misses",
    }
}

/// The assertion text a miss was judged by, on one line and without the
/// statement's trailing `;`, for quoting after the reason.
pub(crate) fn checked_assertion_text(oracle: &str) -> String {
    one_line(oracle)
        .trim_end_matches(';')
        .trim_end()
        .to_string()
}

fn link_label(reason: RelationReason) -> Option<&'static str> {
    match reason {
        RelationReason::SameTestFile => Some("file location"),
        RelationReason::SameModule => Some("module location"),
        RelationReason::OwnerNamedTest | RelationReason::WeakTokenSubstring => Some("name"),
        _ => None,
    }
}

fn one_line(value: &str) -> String {
    value.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{MissingDiscriminatorFact, OracleKind, OracleStrength};
    use crate::output::perl_preview_card::tests::sample_perl_finding as sample_finding;
    use std::path::PathBuf;

    fn test_with(miss: Option<RelatedTestMiss>, reason: Option<RelationReason>) -> RelatedTest {
        RelatedTest {
            name: "parses_ge".to_string(),
            file: PathBuf::from("tests/parse.rs"),
            line: 3,
            oracle: Some("assert_eq!(op(\">=1\"), Op::GreaterEq)".to_string()),
            oracle_kind: OracleKind::ExactValue,
            oracle_strength: OracleStrength::Strong,
            relation_reason: reason,
            relation_confidence: None,
            miss,
        }
    }

    #[test]
    fn a_test_without_a_recorded_miss_gets_no_reason() {
        let finding = sample_finding();
        assert_eq!(
            related_test_miss_reason(
                &test_with(None, None),
                &finding.activation.missing_discriminators
            ),
            None
        );
    }

    #[test]
    fn each_miss_names_a_checkable_fact() {
        let mut finding = sample_finding();
        finding.activation.missing_discriminators = vec![MissingDiscriminatorFact {
            value: "len == 0".to_string(),
            reason: "no related test passes an empty input".to_string(),
            flow_sink: None,
        }];
        let cases = [
            (
                RelatedTestMiss::NoCallPath,
                Some(RelationReason::SameTestFile),
                "no call to the changed code found; linked by file location only",
            ),
            (
                RelatedTestMiss::NoCallPath,
                Some(RelationReason::DirectOwnerCall),
                "no call to the changed code found",
            ),
            (RelatedTestMiss::NoAssertion, None, "has no assertion"),
            (
                RelatedTestMiss::AssertionNotObserving,
                None,
                "asserts, but not on the changed value",
            ),
            (
                RelatedTestMiss::WeakAssertion,
                None,
                "assertion too weak to tell the old behavior from the new",
            ),
            (
                RelatedTestMiss::MissingInput,
                None,
                "no test input reaches `len == 0`",
            ),
            (
                RelatedTestMiss::ObservationUnconfirmed,
                None,
                "ripr could not confirm that this assertion observes the changed behavior",
            ),
        ];
        for (miss, reason, expected) in cases {
            assert_eq!(
                related_test_miss_reason(
                    &test_with(Some(miss), reason),
                    &finding.activation.missing_discriminators
                )
                .as_deref(),
                Some(expected),
                "{miss:?}"
            );
        }
    }

    #[test]
    fn only_an_unconfirmed_observation_drops_the_misses_label() {
        let established = [
            RelatedTestMiss::NoCallPath,
            RelatedTestMiss::NoAssertion,
            RelatedTestMiss::AssertionNotObserving,
            RelatedTestMiss::AssertionNotCredited,
            RelatedTestMiss::WeakAssertion,
            RelatedTestMiss::MissingInput,
            RelatedTestMiss::MissingExactAssertion,
        ];
        for miss in established {
            assert_eq!(
                related_test_miss_label(&test_with(Some(miss), None)),
                "misses",
                "{miss:?}"
            );
        }
        assert_eq!(
            related_test_miss_label(&test_with(
                Some(RelatedTestMiss::ObservationUnconfirmed),
                None
            )),
            "unconfirmed"
        );
    }

    #[test]
    fn an_exact_assertion_miss_names_the_variant_not_an_input() {
        let mut finding = sample_finding();
        finding.activation.missing_discriminators = vec![MissingDiscriminatorFact {
            value: "CalcError::TooLarge".to_string(),
            reason: "No exact error variant assertion for CalcError::TooLarge".to_string(),
            flow_sink: None,
        }];
        let test = test_with(Some(RelatedTestMiss::MissingExactAssertion), None);
        assert_eq!(
            related_test_miss_reason(&test, &finding.activation.missing_discriminators).as_deref(),
            Some("no assertion pins `CalcError::TooLarge`")
        );
    }
}
