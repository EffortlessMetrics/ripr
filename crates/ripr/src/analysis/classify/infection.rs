use super::super::rust_index::{TestSummary, extract_literals};
use super::activation::has_observed_boundary_equality;
use crate::domain::*;

pub(in crate::analysis) fn infection_evidence(
    probe: &Probe,
    related_tests: &[&TestSummary],
    activation: &ActivationEvidence,
) -> StageEvidence {
    match probe.family {
        ProbeFamily::Predicate => {
            let probe_literals = extract_literals(&probe.expression);
            let test_literals = related_tests
                .iter()
                .flat_map(|test| test.literals.iter().map(|literal| literal.value.clone()))
                .collect::<Vec<_>>();
            if related_tests.is_empty() {
                StageEvidence::new(
                    StageState::Unknown,
                    Confidence::Low,
                    "No tests were found, so activation/infection cannot be estimated",
                )
            } else if activation
                .missing_discriminators
                .iter()
                .any(|fact| fact.value.contains("=="))
            {
                StageEvidence::new(
                    StageState::Weak,
                    Confidence::Medium,
                    "Related tests contain input values, but the equality-boundary discriminator is missing",
                )
            } else if has_observed_boundary_equality(activation) {
                StageEvidence::new(
                    StageState::Yes,
                    Confidence::Medium,
                    "Detected related test input at the changed boundary",
                )
            } else if probe_literals.is_empty() {
                StageEvidence::new(
                    StageState::Unknown,
                    Confidence::Low,
                    "Predicate changed, but no literal boundary was visible in the changed expression",
                )
            } else if probe_literals
                .iter()
                .any(|literal| test_literals.iter().any(|t| t == literal))
            {
                StageEvidence::new(
                    StageState::Yes,
                    Confidence::Medium,
                    format!(
                        "Detected test input literal matching changed boundary: {}",
                        probe_literals.join(", ")
                    ),
                )
            } else if !test_literals.is_empty() {
                StageEvidence::new(
                    StageState::Weak,
                    Confidence::Medium,
                    format!(
                        "Tests have literals [{}], but no detected value matches changed boundary [{}]",
                        test_literals.join(", "),
                        probe_literals.join(", ")
                    ),
                )
            } else {
                StageEvidence::new(
                    StageState::Unknown,
                    Confidence::Low,
                    "Related tests use opaque fixtures; activation/infection is unknown",
                )
            }
        }
        ProbeFamily::StaticUnknown => StageEvidence::new(
            StageState::Unknown,
            Confidence::Unknown,
            "Changed syntax is not mapped to a high-confidence probe family",
        ),
        _ => {
            if related_tests.is_empty() {
                StageEvidence::new(
                    StageState::Unknown,
                    Confidence::Low,
                    "No reachable tests were found, so infection cannot be established",
                )
            } else {
                StageEvidence::new(
                    StageState::Yes,
                    Confidence::Medium,
                    "Reachable tests can plausibly activate this changed behavior",
                )
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::super::rust_index::LiteralFact;
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn predicate_infection_uses_matching_test_literal() {
        let probe = probe(ProbeFamily::Predicate, "value > 10");
        let test = test_with_literals(&["10"]);
        let evidence = infection_evidence(&probe, &[&test], &ActivationEvidence::default());

        assert_eq!(evidence.state, StageState::Yes);
        assert_eq!(
            evidence.summary,
            "Detected test input literal matching changed boundary: 10"
        );
    }

    #[test]
    fn predicate_infection_matches_decimal_exponent_case() {
        let probe = probe(ProbeFamily::Predicate, "ratio < 4E-2");
        let test = test_with_literals(&["4e-2"]);
        let evidence = infection_evidence(&probe, &[&test], &ActivationEvidence::default());

        assert_eq!(evidence.state, StageState::Yes);
        assert_eq!(
            evidence.summary,
            "Detected test input literal matching changed boundary: 4e-2"
        );
    }

    #[test]
    fn predicate_infection_reports_opaque_fixture_when_literals_are_missing() {
        let probe = probe(ProbeFamily::Predicate, "value > 10");
        let test = test_with_literals(&[]);
        let evidence = infection_evidence(&probe, &[&test], &ActivationEvidence::default());

        assert_eq!(evidence.state, StageState::Unknown);
        assert_eq!(
            evidence.summary,
            "Related tests use opaque fixtures; activation/infection is unknown"
        );
    }

    #[test]
    fn non_predicate_infection_without_related_tests_is_unknown() {
        let probe = probe(ProbeFamily::ReturnValue, "value + 1");
        let evidence = infection_evidence(&probe, &[], &ActivationEvidence::default());

        assert_eq!(evidence.state, StageState::Unknown);
        assert_eq!(
            evidence.summary,
            "No reachable tests were found, so infection cannot be established"
        );
    }

    fn probe(family: ProbeFamily, expression: &str) -> Probe {
        Probe {
            id: ProbeId("probe:test".to_string()),
            location: SourceLocation::new("src/lib.rs", 1, 1),
            owner: None,
            family,
            delta: DeltaKind::Value,
            before: None,
            after: None,
            expression: expression.to_string(),
            expected_sinks: Vec::new(),
            required_oracles: Vec::new(),
        }
    }

    fn test_with_literals(values: &[&str]) -> TestSummary {
        TestSummary {
            name: "value_boundary".to_string(),
            file: PathBuf::from("tests/value.rs"),
            start_line: 1,
            end_line: 3,
            body: "assert_eq!(score(10), 11);".to_string(),
            calls: Vec::new(),
            assertions: Vec::new(),
            literals: values
                .iter()
                .map(|value| LiteralFact {
                    line: 1,
                    value: (*value).to_string(),
                })
                .collect(),
            attrs: Vec::new(),
        }
    }
}
