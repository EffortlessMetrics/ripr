use super::super::rust_index::{OracleFact, TestSummary, extract_identifier_tokens};
use crate::domain::*;

pub(in crate::analysis) fn reveal_evidence(
    probe: &Probe,
    related_tests: &[&TestSummary],
) -> (StageEvidence, StageEvidence, Vec<RelatedTest>) {
    if related_tests.is_empty() {
        return (
            StageEvidence::new(
                StageState::No,
                Confidence::Medium,
                "No reachable test oracle found",
            ),
            StageEvidence::new(
                StageState::No,
                Confidence::Medium,
                "No assertion can discriminate the changed behavior without a reachable test",
            ),
            Vec::new(),
        );
    }

    let analysis = analyze_related_assertions(probe, related_tests);
    let related = finalize_related_tests(analysis.related);
    let observe = build_observe_evidence(analysis.matched_any);
    let discriminate =
        build_discriminate_evidence(&analysis.strongest, &analysis.strongest_kind, &probe.family);

    (observe, discriminate, related)
}

struct RevealAssertionAnalysis {
    related: Vec<RelatedTest>,
    strongest: OracleStrength,
    strongest_kind: OracleKind,
    matched_any: bool,
}

fn analyze_related_assertions(
    probe: &Probe,
    related_tests: &[&TestSummary],
) -> RevealAssertionAnalysis {
    let probe_tokens = extract_identifier_tokens(&probe.expression);
    let mut related = Vec::new();
    let mut strongest = OracleStrength::None;
    let mut strongest_kind = OracleKind::Unknown;
    let mut matched_any = false;

    for test in related_tests {
        if test.assertions.is_empty() {
            related.push(RelatedTest {
                name: test.name.clone(),
                file: test.file.clone(),
                line: test.start_line,
                oracle: None,
                oracle_kind: OracleKind::Unknown,
                oracle_strength: OracleStrength::None,
                relation_reason: None,
                relation_confidence: None,
                language: None,
            });
            continue;
        }
        for assertion in &test.assertions {
            if assertion_matches_probe(
                &probe_tokens,
                &probe.family,
                assertion,
                test.assertions.len(),
            ) {
                matched_any = true;
                let relative_strength = probe_relative_oracle_strength(&probe.family, assertion);
                if relative_strength.rank() > strongest.rank() {
                    strongest = relative_strength.clone();
                    strongest_kind = assertion.kind.clone();
                }
                related.push(RelatedTest {
                    name: test.name.clone(),
                    file: test.file.clone(),
                    line: test.start_line,
                    oracle: Some(assertion.text.clone()),
                    oracle_kind: assertion.kind.clone(),
                    oracle_strength: relative_strength,
                    relation_reason: None,
                    relation_confidence: None,
                    language: None,
                });
            }
        }
    }

    RevealAssertionAnalysis {
        related,
        strongest,
        strongest_kind,
        matched_any,
    }
}

fn assertion_matches_probe(
    probe_tokens: &[String],
    family: &ProbeFamily,
    assertion: &OracleFact,
    assertion_count: usize,
) -> bool {
    let token_match = probe_tokens
        .iter()
        .any(|token| token.len() > 3 && assertion.text.contains(token));
    let family_match = oracle_matches_family(family, assertion);
    token_match || family_match || assertion_count == 1
}

fn finalize_related_tests(mut related: Vec<RelatedTest>) -> Vec<RelatedTest> {
    related.sort_by(|a, b| a.name.cmp(&b.name).then(a.line.cmp(&b.line)));
    related.dedup_by(|a, b| a.name == b.name && a.oracle == b.oracle);
    related
}

fn build_observe_evidence(matched_any: bool) -> StageEvidence {
    if matched_any {
        StageEvidence::new(
            StageState::Yes,
            Confidence::Medium,
            "A related test observes a value or effect near the changed behavior",
        )
    } else {
        StageEvidence::new(
            StageState::No,
            Confidence::Medium,
            "Related tests were found, but no assertion appears to observe the changed value, error, field, or effect",
        )
    }
}

fn build_discriminate_evidence(
    strongest: &OracleStrength,
    strongest_kind: &OracleKind,
    family: &ProbeFamily,
) -> StageEvidence {
    match strongest {
        OracleStrength::Strong => StageEvidence::new(
            StageState::Yes,
            Confidence::Medium,
            match strongest_kind {
                OracleKind::ExactErrorVariant => {
                    "Strong oracle found: exact error variant assertion"
                }
                OracleKind::WholeObjectEquality => {
                    "Strong oracle found: whole-object equality assertion"
                }
                _ => "Strong oracle found: exact value or pattern assertion",
            },
        ),
        OracleStrength::Medium => StageEvidence::new(
            StageState::Weak,
            Confidence::Medium,
            match strongest_kind {
                OracleKind::Snapshot => {
                    "Medium oracle found: snapshot assertion observes the changed behavior"
                }
                OracleKind::MockExpectation => {
                    "Medium oracle found: mock or expectation observes the changed behavior"
                }
                _ => "Medium oracle found: property or partial structural assertion",
            },
        ),
        OracleStrength::Weak => StageEvidence::new(
            StageState::Weak,
            Confidence::High,
            match (strongest_kind, family) {
                (OracleKind::BroadError, ProbeFamily::ErrorPath) => {
                    "Only broad error oracle found; is_err() does not discriminate exact error variants"
                }
                (OracleKind::BroadError, _) => {
                    "Only broad error oracle found; it may not discriminate the changed behavior exactly"
                }
                (OracleKind::RelationalCheck, _) => {
                    "Only relational oracle found; it may not discriminate the changed value exactly"
                }
                _ => {
                    "Only weak oracle found, such as a broad relational assertion or non-empty check"
                }
            },
        ),
        OracleStrength::Smoke => StageEvidence::new(
            StageState::Weak,
            Confidence::High,
            "Only smoke oracle found, such as unwrap/expect or execution without a discriminator",
        ),
        OracleStrength::None => StageEvidence::new(
            StageState::No,
            Confidence::Medium,
            "No assertion found on related tests",
        ),
        OracleStrength::Unknown => StageEvidence::new(
            StageState::Unknown,
            Confidence::Low,
            "Assertions exist, but oracle strength is unknown",
        ),
    }
}

fn oracle_matches_family(family: &ProbeFamily, assertion: &OracleFact) -> bool {
    let text = assertion.text.as_str();
    match family {
        ProbeFamily::ErrorPath => {
            matches!(
                assertion.kind,
                OracleKind::ExactErrorVariant | OracleKind::BroadError
            ) || text.contains("Error::")
                || text.contains("Err")
        }
        ProbeFamily::SideEffect => {
            matches!(assertion.kind, OracleKind::MockExpectation)
                || text.contains("expect")
                || text.contains("mock")
                || text.contains("saved")
                || text.contains("published")
        }
        ProbeFamily::FieldConstruction => {
            matches!(
                assertion.kind,
                OracleKind::ExactValue
                    | OracleKind::WholeObjectEquality
                    | OracleKind::RelationalCheck
                    | OracleKind::Snapshot
            ) || text.contains('.')
        }
        ProbeFamily::Predicate => {
            matches!(
                assertion.kind,
                OracleKind::ExactValue
                    | OracleKind::RelationalCheck
                    | OracleKind::ExactErrorVariant
                    | OracleKind::Snapshot
            )
        }
        ProbeFamily::ReturnValue => [
            OracleKind::ExactValue,
            OracleKind::WholeObjectEquality,
            OracleKind::RelationalCheck,
            OracleKind::Snapshot,
            OracleKind::SmokeOnly,
        ]
        .contains(&assertion.kind),
        ProbeFamily::CallDeletion => {
            matches!(
                assertion.kind,
                OracleKind::MockExpectation
                    | OracleKind::ExactValue
                    | OracleKind::RelationalCheck
                    | OracleKind::SmokeOnly
            ) || text.contains("assert")
                || text.contains("expect")
        }
        ProbeFamily::MatchArm => [
            OracleKind::ExactErrorVariant,
            OracleKind::ExactValue,
            OracleKind::RelationalCheck,
            OracleKind::Snapshot,
        ]
        .contains(&assertion.kind),
        ProbeFamily::StaticUnknown => false,
    }
}

fn probe_relative_oracle_strength(family: &ProbeFamily, assertion: &OracleFact) -> OracleStrength {
    match family {
        ProbeFamily::ErrorPath => match assertion.kind {
            OracleKind::ExactErrorVariant => OracleStrength::Strong,
            OracleKind::BroadError => assertion.strength.clone(),
            OracleKind::SmokeOnly => OracleStrength::Smoke,
            _ => assertion.strength.clone(),
        },
        ProbeFamily::ReturnValue
        | ProbeFamily::Predicate
        | ProbeFamily::FieldConstruction
        | ProbeFamily::MatchArm => match assertion.kind {
            OracleKind::ExactValue
            | OracleKind::ExactErrorVariant
            | OracleKind::WholeObjectEquality => OracleStrength::Strong,
            OracleKind::Snapshot
            | OracleKind::MockExpectation
            | OracleKind::RelationalCheck
            | OracleKind::BroadError => assertion.strength.clone(),
            OracleKind::SmokeOnly => OracleStrength::Smoke,
            OracleKind::Unknown => OracleStrength::Unknown,
        },
        ProbeFamily::SideEffect | ProbeFamily::CallDeletion => match assertion.kind {
            OracleKind::MockExpectation => assertion.strength.clone(),
            OracleKind::ExactValue | OracleKind::WholeObjectEquality => OracleStrength::Strong,
            OracleKind::RelationalCheck | OracleKind::BroadError => assertion.strength.clone(),
            OracleKind::SmokeOnly => OracleStrength::Smoke,
            OracleKind::ExactErrorVariant => OracleStrength::Medium,
            OracleKind::Snapshot => assertion.strength.clone(),
            OracleKind::Unknown => OracleStrength::Unknown,
        },
        ProbeFamily::StaticUnknown => OracleStrength::Unknown,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn reveal_evidence_keeps_assertionless_related_test_without_observe_signal() {
        let probe = probe(ProbeFamily::ReturnValue, "score");
        let test = test_with_assertions("score_returns_value", Vec::new());
        let (observe, discriminate, related) = reveal_evidence(&probe, &[&test]);

        assert_eq!(observe.state, StageState::No);
        assert_eq!(discriminate.state, StageState::No);
        assert_eq!(related.len(), 1);
        assert_eq!(related[0].name, "score_returns_value");
        assert_eq!(related[0].oracle, None);
    }

    #[test]
    fn reveal_evidence_records_matching_assertions_and_sorts_related_tests() {
        let probe = probe(
            ProbeFamily::ErrorPath,
            "return Err(AuthError::RevokedToken);",
        );
        let late = test_with_assertions(
            "z_error_path",
            vec![oracle(
                "assert!(score(\"\").is_err());",
                OracleKind::BroadError,
                OracleStrength::Weak,
            )],
        );
        let early = test_with_assertions(
            "a_error_path",
            vec![oracle(
                "assert_matches!(score(\"\"), Err(AuthError::RevokedToken));",
                OracleKind::ExactErrorVariant,
                OracleStrength::Strong,
            )],
        );
        let (observe, discriminate, related) = reveal_evidence(&probe, &[&late, &early]);

        assert_eq!(observe.state, StageState::Yes);
        assert_eq!(discriminate.state, StageState::Yes);
        assert_eq!(related.len(), 2);
        assert_eq!(related[0].name, "a_error_path");
        assert_eq!(related[0].oracle_strength, OracleStrength::Strong);
        assert_eq!(related[1].name, "z_error_path");
    }

    #[test]
    fn reveal_evidence_ignores_unmatched_assertions() {
        let probe = probe(ProbeFamily::StaticUnknown, "opaque_changed_expr");
        let test = test_with_assertions(
            "opaque_behavior",
            vec![
                oracle(
                    "assert_eq!(unrelated, 3);",
                    OracleKind::Unknown,
                    OracleStrength::Unknown,
                ),
                oracle(
                    "assert!(other_value);",
                    OracleKind::Unknown,
                    OracleStrength::Unknown,
                ),
            ],
        );
        let (observe, discriminate, related) = reveal_evidence(&probe, &[&test]);

        assert_eq!(observe.state, StageState::No);
        assert_eq!(discriminate.state, StageState::No);
        assert!(related.is_empty());
    }

    #[test]
    fn assertion_matching_accepts_token_family_and_single_assertion_fallbacks() {
        let token_assertion = oracle(
            "assert_eq!(score, 3);",
            OracleKind::Unknown,
            OracleStrength::Unknown,
        );
        assert!(assertion_matches_probe(
            &["score".to_string()],
            &ProbeFamily::StaticUnknown,
            &token_assertion,
            2
        ));

        let family_assertion = oracle(
            "assert!(result.is_err());",
            OracleKind::BroadError,
            OracleStrength::Weak,
        );
        assert!(assertion_matches_probe(
            &["err".to_string()],
            &ProbeFamily::ErrorPath,
            &family_assertion,
            2
        ));

        let fallback_assertion = oracle(
            "assert!(ran);",
            OracleKind::Unknown,
            OracleStrength::Unknown,
        );
        assert!(assertion_matches_probe(
            &["run".to_string()],
            &ProbeFamily::StaticUnknown,
            &fallback_assertion,
            1
        ));
        assert!(!assertion_matches_probe(
            &["run".to_string()],
            &ProbeFamily::StaticUnknown,
            &fallback_assertion,
            2
        ));
    }

    #[test]
    fn discriminate_evidence_names_strength_and_oracle_kind() {
        let cases = [
            (
                OracleStrength::Strong,
                OracleKind::ExactErrorVariant,
                ProbeFamily::ErrorPath,
                StageState::Yes,
                "Strong oracle found: exact error variant assertion",
            ),
            (
                OracleStrength::Strong,
                OracleKind::WholeObjectEquality,
                ProbeFamily::ReturnValue,
                StageState::Yes,
                "Strong oracle found: whole-object equality assertion",
            ),
            (
                OracleStrength::Strong,
                OracleKind::ExactValue,
                ProbeFamily::ReturnValue,
                StageState::Yes,
                "Strong oracle found: exact value or pattern assertion",
            ),
            (
                OracleStrength::Medium,
                OracleKind::Snapshot,
                ProbeFamily::ReturnValue,
                StageState::Weak,
                "Medium oracle found: snapshot assertion observes the changed behavior",
            ),
            (
                OracleStrength::Medium,
                OracleKind::MockExpectation,
                ProbeFamily::SideEffect,
                StageState::Weak,
                "Medium oracle found: mock or expectation observes the changed behavior",
            ),
            (
                OracleStrength::Medium,
                OracleKind::ExactValue,
                ProbeFamily::ReturnValue,
                StageState::Weak,
                "Medium oracle found: property or partial structural assertion",
            ),
            (
                OracleStrength::Weak,
                OracleKind::BroadError,
                ProbeFamily::ErrorPath,
                StageState::Weak,
                "Only broad error oracle found; is_err() does not discriminate exact error variants",
            ),
            (
                OracleStrength::Weak,
                OracleKind::BroadError,
                ProbeFamily::ReturnValue,
                StageState::Weak,
                "Only broad error oracle found; it may not discriminate the changed behavior exactly",
            ),
            (
                OracleStrength::Weak,
                OracleKind::RelationalCheck,
                ProbeFamily::Predicate,
                StageState::Weak,
                "Only relational oracle found; it may not discriminate the changed value exactly",
            ),
            (
                OracleStrength::Weak,
                OracleKind::ExactValue,
                ProbeFamily::ReturnValue,
                StageState::Weak,
                "Only weak oracle found, such as a broad relational assertion or non-empty check",
            ),
            (
                OracleStrength::Smoke,
                OracleKind::SmokeOnly,
                ProbeFamily::ReturnValue,
                StageState::Weak,
                "Only smoke oracle found, such as unwrap/expect or execution without a discriminator",
            ),
            (
                OracleStrength::None,
                OracleKind::Unknown,
                ProbeFamily::ReturnValue,
                StageState::No,
                "No assertion found on related tests",
            ),
            (
                OracleStrength::Unknown,
                OracleKind::Unknown,
                ProbeFamily::ReturnValue,
                StageState::Unknown,
                "Assertions exist, but oracle strength is unknown",
            ),
        ];

        for (strength, kind, family, state, summary) in cases {
            let evidence = build_discriminate_evidence(&strength, &kind, &family);
            assert_eq!(evidence.state, state);
            assert_eq!(evidence.summary, summary);
        }
    }

    #[test]
    fn oracle_family_matching_covers_family_specific_shapes() {
        assert!(oracle_matches_family(
            &ProbeFamily::ErrorPath,
            &oracle(
                "assert_matches!(result, Err(AuthError::RevokedToken));",
                OracleKind::ExactErrorVariant,
                OracleStrength::Strong,
            )
        ));
        assert!(oracle_matches_family(
            &ProbeFamily::ErrorPath,
            &oracle(
                "assert!(result.is_err());",
                OracleKind::BroadError,
                OracleStrength::Weak
            )
        ));
        assert!(oracle_matches_family(
            &ProbeFamily::ErrorPath,
            &oracle(
                "assert!(matches!(result, Err(_)));",
                OracleKind::Unknown,
                OracleStrength::Unknown
            )
        ));
        assert!(oracle_matches_family(
            &ProbeFamily::ErrorPath,
            &oracle(
                "assert_eq!(kind, Error::Denied);",
                OracleKind::Unknown,
                OracleStrength::Unknown
            )
        ));
        assert!(oracle_matches_family(
            &ProbeFamily::SideEffect,
            &oracle(
                "mock.expect_send();",
                OracleKind::MockExpectation,
                OracleStrength::Medium
            )
        ));
        assert!(oracle_matches_family(
            &ProbeFamily::SideEffect,
            &oracle(
                "assert!(event.saved);",
                OracleKind::Unknown,
                OracleStrength::Unknown
            )
        ));
        assert!(oracle_matches_family(
            &ProbeFamily::SideEffect,
            &oracle(
                "assert!(event.published);",
                OracleKind::Unknown,
                OracleStrength::Unknown
            )
        ));
        assert!(oracle_matches_family(
            &ProbeFamily::FieldConstruction,
            &oracle(
                "assert_eq!(item.id, 3);",
                OracleKind::Unknown,
                OracleStrength::Unknown
            )
        ));
        assert!(oracle_matches_family(
            &ProbeFamily::FieldConstruction,
            &oracle(
                "assert_debug_snapshot!(item);",
                OracleKind::Snapshot,
                OracleStrength::Medium
            )
        ));
        assert!(oracle_matches_family(
            &ProbeFamily::Predicate,
            &oracle(
                "assert!(value >= 3);",
                OracleKind::RelationalCheck,
                OracleStrength::Weak
            )
        ));
        assert!(oracle_matches_family(
            &ProbeFamily::ReturnValue,
            &oracle(
                "assert_eq!(score(), 3);",
                OracleKind::ExactValue,
                OracleStrength::Strong
            )
        ));
        assert!(oracle_matches_family(
            &ProbeFamily::ReturnValue,
            &oracle(
                "score().unwrap();",
                OracleKind::SmokeOnly,
                OracleStrength::Smoke
            )
        ));
        assert!(oracle_matches_family(
            &ProbeFamily::CallDeletion,
            &oracle(
                "assert!(sent);",
                OracleKind::Unknown,
                OracleStrength::Unknown
            )
        ));
        assert!(oracle_matches_family(
            &ProbeFamily::CallDeletion,
            &oracle(
                "expect_send_called();",
                OracleKind::Unknown,
                OracleStrength::Unknown
            )
        ));
        assert!(oracle_matches_family(
            &ProbeFamily::CallDeletion,
            &oracle(
                "mock.expect_send();",
                OracleKind::MockExpectation,
                OracleStrength::Medium
            )
        ));
        assert!(oracle_matches_family(
            &ProbeFamily::MatchArm,
            &oracle(
                "assert_matches!(kind, Ready);",
                OracleKind::ExactErrorVariant,
                OracleStrength::Strong
            )
        ));
        assert!(oracle_matches_family(
            &ProbeFamily::MatchArm,
            &oracle(
                "assert_eq!(kind, Ready);",
                OracleKind::ExactValue,
                OracleStrength::Strong
            )
        ));
        assert!(!oracle_matches_family(
            &ProbeFamily::StaticUnknown,
            &oracle(
                "assert_eq!(value, 3);",
                OracleKind::ExactValue,
                OracleStrength::Strong
            )
        ));
    }

    #[test]
    fn probe_relative_oracle_strength_preserves_family_overrides() {
        let cases = [
            (
                ProbeFamily::ErrorPath,
                oracle("exact", OracleKind::ExactErrorVariant, OracleStrength::Weak),
                OracleStrength::Strong,
            ),
            (
                ProbeFamily::ErrorPath,
                oracle("broad", OracleKind::BroadError, OracleStrength::Weak),
                OracleStrength::Weak,
            ),
            (
                ProbeFamily::ErrorPath,
                oracle("smoke", OracleKind::SmokeOnly, OracleStrength::Strong),
                OracleStrength::Smoke,
            ),
            (
                ProbeFamily::ErrorPath,
                oracle("snapshot", OracleKind::Snapshot, OracleStrength::Medium),
                OracleStrength::Medium,
            ),
            (
                ProbeFamily::ReturnValue,
                oracle("exact", OracleKind::ExactValue, OracleStrength::Weak),
                OracleStrength::Strong,
            ),
            (
                ProbeFamily::Predicate,
                oracle("snapshot", OracleKind::Snapshot, OracleStrength::Medium),
                OracleStrength::Medium,
            ),
            (
                ProbeFamily::FieldConstruction,
                oracle("smoke", OracleKind::SmokeOnly, OracleStrength::Strong),
                OracleStrength::Smoke,
            ),
            (
                ProbeFamily::MatchArm,
                oracle("unknown", OracleKind::Unknown, OracleStrength::Strong),
                OracleStrength::Unknown,
            ),
            (
                ProbeFamily::SideEffect,
                oracle("mock", OracleKind::MockExpectation, OracleStrength::Medium),
                OracleStrength::Medium,
            ),
            (
                ProbeFamily::SideEffect,
                oracle(
                    "exact",
                    OracleKind::WholeObjectEquality,
                    OracleStrength::Weak,
                ),
                OracleStrength::Strong,
            ),
            (
                ProbeFamily::CallDeletion,
                oracle("rel", OracleKind::RelationalCheck, OracleStrength::Weak),
                OracleStrength::Weak,
            ),
            (
                ProbeFamily::CallDeletion,
                oracle("smoke", OracleKind::SmokeOnly, OracleStrength::Strong),
                OracleStrength::Smoke,
            ),
            (
                ProbeFamily::SideEffect,
                oracle(
                    "exact_error",
                    OracleKind::ExactErrorVariant,
                    OracleStrength::Strong,
                ),
                OracleStrength::Medium,
            ),
            (
                ProbeFamily::SideEffect,
                oracle("snapshot", OracleKind::Snapshot, OracleStrength::Weak),
                OracleStrength::Weak,
            ),
            (
                ProbeFamily::SideEffect,
                oracle("unknown", OracleKind::Unknown, OracleStrength::Strong),
                OracleStrength::Unknown,
            ),
            (
                ProbeFamily::StaticUnknown,
                oracle("exact", OracleKind::ExactValue, OracleStrength::Strong),
                OracleStrength::Unknown,
            ),
        ];

        for (family, assertion, expected) in cases {
            assert_eq!(
                probe_relative_oracle_strength(&family, &assertion),
                expected
            );
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

    fn test_with_assertions(name: &str, assertions: Vec<OracleFact>) -> TestSummary {
        TestSummary {
            name: name.to_string(),
            file: PathBuf::from("tests/value.rs"),
            start_line: 1,
            end_line: 3,
            body: "score();".to_string(),
            calls: Vec::new(),
            assertions,
            literals: Vec::new(),
            attrs: Vec::new(),
        }
    }

    fn oracle(text: &str, kind: OracleKind, strength: OracleStrength) -> OracleFact {
        OracleFact {
            line: 2,
            text: text.to_string(),
            kind,
            strength,
            observed_tokens: extract_identifier_tokens(text),
        }
    }
}
