use super::super::rust_index::{TestSummary, extract_literals};
use super::activation::{
    boundary_constant_operand_name, has_observed_boundary_equality, owner_input_values,
};
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
            // Only a literal that flows into the changed owner's inputs can
            // activate the boundary. The activation authority separates
            // owner-call arguments (and table/builder inputs) from assertion
            // arguments; an expected value such as the `2000` in
            // `assert_eq!(tax_bps("EU"), 2000)` is an oracle, not an input.
            let mut input_literals = owner_input_values(activation)
                .into_iter()
                .flat_map(extract_literals)
                .collect::<Vec<_>>();
            input_literals.sort();
            input_literals.dedup();
            let boundary_input_literals = probe_literals
                .iter()
                .filter(|literal| input_literals.contains(literal))
                .cloned()
                .collect::<Vec<_>>();
            let boundary_oracle_only_literals = probe_literals
                .iter()
                .filter(|literal| {
                    !input_literals.contains(literal) && test_literals.contains(literal)
                })
                .cloned()
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
                    no_literal_boundary_summary(&probe.expression),
                )
            } else if !boundary_input_literals.is_empty() {
                StageEvidence::new(
                    StageState::Yes,
                    Confidence::Medium,
                    format!(
                        "Detected test input literal matching changed boundary: {}",
                        boundary_input_literals.join(", ")
                    ),
                )
            } else if !boundary_oracle_only_literals.is_empty() {
                StageEvidence::new(
                    StageState::Weak,
                    Confidence::Medium,
                    format!(
                        "Related tests contain the changed boundary literal [{}] only outside the changed owner's inputs (for example as an expected value); no test input at the changed boundary was detected",
                        boundary_oracle_only_literals.join(", ")
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
            } else if is_wildcard_discard(&probe.expression) {
                StageEvidence::new(
                    StageState::Unknown,
                    Confidence::Low,
                    "Changed value is bound to a discard pattern; it cannot infect a sink",
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

/// Why a changed predicate with no literal boundary stays unknown. A
/// boundary that names a constant says so: ripr could not see that
/// constant's value in the owner's file (or it is declared more than once),
/// and no related test passes the constant itself, so it cannot tell
/// whether any test input sits on the boundary.
fn no_literal_boundary_summary(expression: &str) -> String {
    match boundary_constant_operand_name(expression) {
        Some(name) => format!(
            "Predicate compares against constant {name}; ripr cannot see its value statically or match it to a related test input (a same-file integer const, a matching literal input, or an argument naming {name} would resolve it), so activation/infection is unknown"
        ),
        None => "Predicate changed, but no literal boundary was visible in the changed expression"
            .to_string(),
    }
}

/// Returns true iff the expression is an exact wildcard discard that provably
/// cannot infect any sink.  Matches the `let` + `_` + `:`/`=` token grammar
/// across any legal whitespace (#3233) but NOT `let _name` — those bindings
/// are still used.
fn is_wildcard_discard(expression: &str) -> bool {
    // Shared whitespace-stable predicate (#3233): the flow stage's
    // `value_is_swallowed` consumes the same authority, so the two stages
    // cannot drift apart on `let _ =` vs `let _=` vs `let _ :` tokenizations.
    super::text::is_wildcard_discard_binding(expression)
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
        let activation = activation_with(&[("value = 10", ValueContext::FunctionArgument)]);
        let evidence = infection_evidence(&probe, &[&test], &activation);

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
        let activation = activation_with(&[("ratio = 4e-2", ValueContext::FunctionArgument)]);
        let evidence = infection_evidence(&probe, &[&test], &activation);

        assert_eq!(evidence.state, StageState::Yes);
        assert_eq!(
            evidence.summary,
            "Detected test input literal matching changed boundary: 4e-2"
        );
    }

    #[test]
    fn predicate_infection_names_an_unmatched_constant_boundary() {
        let constant = probe(ProbeFamily::Predicate, "amount >= DISCOUNT_THRESHOLD");
        let test = test_with_literals(&["10000"]);
        let evidence = infection_evidence(&constant, &[&test], &ActivationEvidence::default());

        assert_eq!(evidence.state, StageState::Unknown);
        assert!(
            evidence
                .summary
                .starts_with("Predicate compares against constant DISCOUNT_THRESHOLD; ripr cannot see its value statically"),
            "{}",
            evidence.summary
        );

        let parameter = probe(ProbeFamily::Predicate, "amount >= threshold");
        assert_eq!(
            infection_evidence(&parameter, &[&test], &ActivationEvidence::default()).summary,
            "Predicate changed, but no literal boundary was visible in the changed expression"
        );
    }

    #[test]
    fn predicate_infection_ignores_boundary_literal_used_only_as_expected_value() {
        // `assert_eq!(tax_bps("EU"), 2000)` in a related test that never
        // passes 2000 into the changed owner: the literal is the oracle's
        // expected value, not an input, so it cannot activate the boundary.
        let probe = probe(ProbeFamily::Predicate, "weight_grams > 2_000");
        let test = test_with_literals(&["2000"]);
        let activation = activation_with(&[("2000", ValueContext::AssertionArgument)]);
        let evidence = infection_evidence(&probe, &[&test], &activation);

        assert_eq!(evidence.state, StageState::Weak);
        assert_eq!(
            evidence.summary,
            "Related tests contain the changed boundary literal [2000] only outside the changed owner's inputs (for example as an expected value); no test input at the changed boundary was detected"
        );
    }

    #[test]
    fn predicate_infection_credits_the_same_literal_when_it_is_an_owner_input() {
        // Alternate of the expected-value case: the same boundary literal
        // passed as the owner's argument activates the boundary.
        let probe = probe(ProbeFamily::Predicate, "weight_grams > 2_000");
        let test = test_with_literals(&["2000", "400"]);
        let activation = activation_with(&[
            ("400", ValueContext::AssertionArgument),
            ("weight_grams = 2_000", ValueContext::FunctionArgument),
        ]);
        let evidence = infection_evidence(&probe, &[&test], &activation);

        assert_eq!(evidence.state, StageState::Yes);
        assert_eq!(
            evidence.summary,
            "Detected test input literal matching changed boundary: 2000"
        );
    }

    #[test]
    fn predicate_infection_credits_table_row_inputs_but_not_enum_variants() {
        let probe = probe(ProbeFamily::Predicate, "amount > 10");
        let test = test_with_literals(&["10", "11"]);
        let table = activation_with(&[("10", ValueContext::TableRow)]);
        assert_eq!(
            infection_evidence(&probe, &[&test], &table).state,
            StageState::Yes
        );

        // A non-input context (an enum variant) never counts as an input.
        let enum_only = activation_with(&[("10", ValueContext::EnumVariant)]);
        assert_eq!(
            infection_evidence(&probe, &[&test], &enum_only).state,
            StageState::Weak
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

    #[test]
    fn wildcard_discard_is_infection_unknown_even_with_related_tests() {
        let probe = probe(ProbeFamily::SideEffect, "let _ = compute_fee(amount)");
        let test = test_with_literals(&["42"]);
        let evidence = infection_evidence(&probe, &[&test], &ActivationEvidence::default());

        assert_eq!(evidence.state, StageState::Unknown);
        assert_eq!(
            evidence.summary,
            "Changed value is bound to a discard pattern; it cannot infect a sink"
        );
    }

    #[test]
    fn typed_wildcard_discard_is_infection_unknown() {
        let probe = probe(ProbeFamily::ReturnValue, "let _: u32 = helper(x)");
        let test = test_with_literals(&["1"]);
        let evidence = infection_evidence(&probe, &[&test], &ActivationEvidence::default());

        assert_eq!(evidence.state, StageState::Unknown);
        assert_eq!(
            evidence.summary,
            "Changed value is bound to a discard pattern; it cannot infect a sink"
        );
    }

    #[test]
    fn whitespace_padded_wildcard_discards_are_infection_unknown() {
        // #3233: the infection stage consumes the shared predicate directly,
        // so non-canonical whitespace shapes must classify as discards here
        // too — a regression to the old exact-prefix match in this stage
        // alone would fail this test even if the flow stage stayed correct.
        for expression in [
            "let _ : u32 = helper(x);",
            "let _=helper(x);",
            "let   _   =   helper(x);",
        ] {
            let probe = probe(ProbeFamily::ReturnValue, expression);
            let test = test_with_literals(&["1"]);
            let evidence = infection_evidence(&probe, &[&test], &ActivationEvidence::default());
            assert_eq!(
                evidence.state,
                StageState::Unknown,
                "`{expression}` must be a discard in the infection stage"
            );
        }
    }

    #[test]
    fn named_binding_is_not_a_discard_stays_yes() {
        // `let _name = ...` is a named binding that could be used — must stay Yes
        let probe = probe(ProbeFamily::ReturnValue, "let _result = helper(a)");
        let test = test_with_literals(&["1"]);
        let evidence = infection_evidence(&probe, &[&test], &ActivationEvidence::default());

        assert_eq!(evidence.state, StageState::Yes);
    }

    #[test]
    fn return_value_read_into_tail_stays_exposed() {
        // Control: `let result = helper(a); result + 1` (value read into return) must stay Yes
        let probe = probe(ProbeFamily::ReturnValue, "result + 1");
        let test = test_with_literals(&["1"]);
        let evidence = infection_evidence(&probe, &[&test], &ActivationEvidence::default());

        assert_eq!(evidence.state, StageState::Yes);
    }

    fn activation_with(facts: &[(&str, ValueContext)]) -> ActivationEvidence {
        ActivationEvidence {
            observed_values: facts
                .iter()
                .map(|(value, context)| ValueFact {
                    line: 1,
                    text: String::new(),
                    value: (*value).to_string(),
                    context: context.clone(),
                })
                .collect(),
            missing_discriminators: Vec::new(),
        }
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
            nested_fn_names: Vec::new(),
            let_bindings: Vec::new(),
        }
    }
}
