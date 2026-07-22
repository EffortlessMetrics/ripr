mod evidence;
mod finding;
mod owner;

use self::evidence::ClassifiedProbeEvidence;
use self::finding::build_finding;
use self::owner::resolve_owner_function;
use super::classify::{ProbeContext, find_related_tests, is_assertion_shaped_owner};
use super::probes::parser_expression_for_probe;
use super::rust_index::RustIndex;
use crate::domain::*;

pub fn classify_probe(probe: &Probe, index: &RustIndex) -> Finding {
    let owner_fn = resolve_owner_function(probe, index);
    let related_tests = find_related_tests(probe, owner_fn, index);
    // RIPR-SPEC-0133: detect assertion-shaped owners (oracles) here, where the
    // full index is available; the context carries the verdict so guidance can
    // be reframed. The exposure class is never changed by this flag.
    let owner_assertion_shaped =
        owner_fn.is_some_and(|owner| is_assertion_shaped_owner(owner, index));
    let context = ProbeContext::new(probe, owner_fn, related_tests, owner_assertion_shaped);
    let reveal_expression = parser_expression_for_probe(
        index,
        &probe.location.file,
        probe.location.line,
        &probe.family,
        &probe.expression,
    )
    .unwrap_or(&probe.expression);
    let evidence = ClassifiedProbeEvidence::gather(&context, reveal_expression);
    let class = evidence.classify(context.probe);

    build_finding(&context, class, evidence)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::classify::{recommended_next_step, stop_reasons};
    use crate::analysis::rust_index::{
        CallFact, FileFacts, FunctionSummary, LiteralFact, OracleFact, PROBE_SHAPE_CALL_DELETION,
        ProbeShapeFact, ReturnFact, TestSummary, extract_identifier_tokens,
    };
    use std::collections::BTreeMap;
    use std::path::PathBuf;

    #[test]
    fn given_owner_symbol_when_resolving_owner_then_matches_full_identity() {
        let crate_b_fn = function("crates/crate_b/src/lib.rs", "score");
        let crate_a_fn = function("crates/crate_a/src/lib.rs", "score");
        let index = RustIndex {
            functions: vec![crate_b_fn, crate_a_fn],
            tests: vec![
                test(
                    "crates/crate_b/tests/score.rs",
                    "crate_b_score_test",
                    "score(2)",
                    "assert_eq!(score(2), 3);",
                ),
                test(
                    "crates/crate_a/tests/score.rs",
                    "crate_a_score_test",
                    "score(1)",
                    "assert_eq!(score(1), 2);",
                ),
            ],
            ..RustIndex::default()
        };
        let probe = Probe {
            id: ProbeId("probe:crate_a:score".to_string()),
            location: SourceLocation::new("crates/crate_a/src/lib.rs", 2, 1),
            owner: Some(SymbolId("crates/crate_a/src/lib.rs::score".to_string())),
            family: ProbeFamily::ReturnValue,
            delta: DeltaKind::Value,
            before: None,
            after: Some("score + 1".to_string()),
            expression: "score + 1".to_string(),
            expected_sinks: vec![],
            required_oracles: vec![],
        };

        let finding = classify_probe(&probe, &index);

        assert_eq!(finding.related_tests.len(), 1);
        assert_eq!(finding.related_tests[0].name, "crate_a_score_test");
    }

    #[test]
    fn given_unrelated_test_mentions_probe_token_when_owner_is_not_called_then_no_static_path() {
        let index = RustIndex {
            functions: vec![function("src/lib.rs", "discounted_total")],
            tests: vec![TestSummary {
                name: "token_label_includes_token_text".to_string(),
                file: PathBuf::from("tests/tokens.rs"),
                start_line: 1,
                end_line: 4,
                body: "token_label(\"discount_threshold\");\nassert_eq!(token_label(\"discount_threshold\"), \"token:discount_threshold\");".to_string(),
                calls: vec![CallFact {
                    line: 1,
                    name: "token_label".to_string(),
                    text: "token_label(\"discount_threshold\")".to_string(),
                }],
                assertions: vec![oracle_fact(
                    "assert_eq!(token_label(\"discount_threshold\"), \"token:discount_threshold\");",
                    OracleKind::ExactValue,
                    OracleStrength::Strong,
                )],
                literals: vec![LiteralFact {
                    line: 1,
                    value: "\"discount_threshold\"".to_string(),
                }],
                attrs: vec![],
            }],
            ..RustIndex::default()
        };
        let probe = Probe {
            id: ProbeId("probe:src_lib_rs:2:predicate".to_string()),
            location: SourceLocation::new("src/lib.rs", 2, 1),
            owner: Some(SymbolId("src/lib.rs::discounted_total".to_string())),
            family: ProbeFamily::Predicate,
            delta: DeltaKind::Control,
            before: None,
            after: Some("amount >= discount_threshold".to_string()),
            expression: "amount >= discount_threshold".to_string(),
            expected_sinks: vec![],
            required_oracles: vec![],
        };

        let finding = classify_probe(&probe, &index);

        assert_eq!(finding.class, ExposureClass::NoStaticPath);
        assert_eq!(finding.ripr.reach.state, StageState::No);
        assert!(finding.related_tests.is_empty());
    }

    #[test]
    fn given_three_character_probe_token_in_test_name_when_owner_is_not_called_then_test_is_related()
     {
        let index = RustIndex {
            functions: vec![function("src/lib.rs", "tax_total")],
            tests: vec![TestSummary {
                name: "vat_boundary_is_checked_by_macro".to_string(),
                file: PathBuf::from("tests/tax.rs"),
                start_line: 1,
                end_line: 4,
                body: "assert_eq!(macro_tax_case!(100), 120);".to_string(),
                calls: vec![CallFact {
                    line: 1,
                    name: "macro_tax_case".to_string(),
                    text: "macro_tax_case!(100)".to_string(),
                }],
                assertions: vec![oracle_fact(
                    "assert_eq!(macro_tax_case!(100), 120);",
                    OracleKind::ExactValue,
                    OracleStrength::Strong,
                )],
                literals: vec![LiteralFact {
                    line: 1,
                    value: "100".to_string(),
                }],
                attrs: vec![],
            }],
            ..RustIndex::default()
        };
        let probe = Probe {
            id: ProbeId("probe:src_lib_rs:2:predicate".to_string()),
            location: SourceLocation::new("src/lib.rs", 2, 1),
            owner: Some(SymbolId("src/lib.rs::tax_total".to_string())),
            family: ProbeFamily::Predicate,
            delta: DeltaKind::Control,
            before: None,
            after: Some("vat >= threshold".to_string()),
            expression: "vat >= threshold".to_string(),
            expected_sinks: vec![],
            required_oracles: vec![],
        };

        let finding = classify_probe(&probe, &index);

        assert_eq!(finding.ripr.reach.state, StageState::Yes);
        assert_eq!(finding.related_tests.len(), 1);
        assert_eq!(
            finding.related_tests[0].name,
            "vat_boundary_is_checked_by_macro"
        );
    }

    #[test]
    fn given_infection_unknown_probe_when_classified_then_stop_reason_is_present() {
        let index = RustIndex {
            functions: vec![function("src/lib.rs", "price")],
            tests: vec![test(
                "tests/pricing.rs",
                "price_test",
                "price(1)",
                "assert_eq!(price(1), 2);",
            )],
            ..RustIndex::default()
        };
        let probe = Probe {
            id: ProbeId("probe:src_lib_rs:2:predicate".to_string()),
            location: SourceLocation::new("src/lib.rs", 2, 1),
            owner: Some(SymbolId("src/lib.rs::price".to_string())),
            family: ProbeFamily::Predicate,
            delta: DeltaKind::Control,
            before: None,
            after: Some("amount >= threshold".to_string()),
            expression: "amount >= threshold".to_string(),
            expected_sinks: vec![],
            required_oracles: vec![],
        };

        let finding = classify_probe(&probe, &index);

        assert_eq!(finding.class, ExposureClass::InfectionUnknown);
        assert!(finding.unknown_has_stop_reason());
        assert!(
            finding
                .stop_reasons
                .iter()
                .any(|reason| { reason.as_str() == StopReason::InfectionEvidenceUnknown.as_str() })
        );
    }

    #[test]
    fn given_propagation_unknown_probe_when_classified_then_stop_reason_is_present() {
        let function = FunctionSummary {
            body: "value".to_string(),
            ..function("src/lib.rs", "score")
        };
        let index = RustIndex {
            functions: vec![function],
            tests: vec![test(
                "tests/score.rs",
                "score_test",
                "score(1)",
                "assert_eq!(score(1), 2);",
            )],
            ..RustIndex::default()
        };
        let probe = Probe {
            id: ProbeId("probe:src_lib_rs:2:return_value".to_string()),
            location: SourceLocation::new("src/lib.rs", 2, 1),
            owner: Some(SymbolId("src/lib.rs::score".to_string())),
            family: ProbeFamily::ReturnValue,
            delta: DeltaKind::Value,
            before: None,
            after: Some("value".to_string()),
            expression: "value".to_string(),
            expected_sinks: vec![],
            required_oracles: vec![],
        };

        let finding = classify_probe(&probe, &index);

        assert_eq!(finding.class, ExposureClass::PropagationUnknown);
        assert!(finding.unknown_has_stop_reason());
        assert!(
            finding.stop_reasons.iter().any(|reason| {
                reason.as_str() == StopReason::PropagationEvidenceUnknown.as_str()
            })
        );
    }

    #[test]
    fn given_static_unknown_probe_when_classified_then_stop_reason_is_present() {
        let index = RustIndex {
            functions: vec![function("src/lib.rs", "score")],
            tests: vec![test(
                "tests/score.rs",
                "score_test",
                "score(1)",
                "assert_eq!(score(1), 2);",
            )],
            ..RustIndex::default()
        };
        let probe = Probe {
            id: ProbeId("probe:src_lib_rs:2:static_unknown".to_string()),
            location: SourceLocation::new("src/lib.rs", 2, 1),
            owner: Some(SymbolId("src/lib.rs::score".to_string())),
            family: ProbeFamily::StaticUnknown,
            delta: DeltaKind::Unknown,
            before: None,
            after: Some("score!(1)".to_string()),
            expression: "score".to_string(),
            expected_sinks: vec![],
            required_oracles: vec![],
        };

        let finding = classify_probe(&probe, &index);

        assert_eq!(finding.class, ExposureClass::StaticUnknown);
        assert!(finding.unknown_has_stop_reason());
        assert!(
            finding
                .stop_reasons
                .iter()
                .any(|reason| { reason.as_str() == StopReason::StaticProbeUnknown.as_str() })
        );
    }

    #[test]
    fn given_exact_error_variant_assertion_when_error_path_probe_changes_then_oracle_is_strong() {
        let index = RustIndex {
            functions: vec![function("src/lib.rs", "score")],
            tests: vec![test_with_oracle(
                "tests/errors.rs",
                "revoked_token_is_exact",
                "score(\"\")",
                oracle_fact(
                    "assert_matches!(score(\"\"), Err(AuthError::RevokedToken));",
                    OracleKind::ExactErrorVariant,
                    OracleStrength::Strong,
                ),
            )],
            ..RustIndex::default()
        };
        let probe = Probe {
            id: ProbeId("probe:src_lib_rs:2:error_path".to_string()),
            location: SourceLocation::new("src/lib.rs", 2, 1),
            owner: Some(SymbolId("src/lib.rs::score".to_string())),
            family: ProbeFamily::ErrorPath,
            delta: DeltaKind::Control,
            before: None,
            after: Some("Err(AuthError::RevokedToken)".to_string()),
            expression: "Err(AuthError::RevokedToken)".to_string(),
            expected_sinks: vec![],
            required_oracles: vec![],
        };

        let finding = classify_probe(&probe, &index);

        assert_eq!(finding.ripr.reveal.discriminate.state, StageState::Yes);
        assert_eq!(
            finding.ripr.reveal.discriminate.summary,
            "Strong oracle found: exact error variant assertion"
        );
        assert_eq!(
            finding.related_tests[0].oracle_strength,
            OracleStrength::Strong
        );
    }

    #[test]
    fn given_broad_is_err_assertion_when_error_variant_changes_then_oracle_is_weak() {
        let index = RustIndex {
            functions: vec![function("src/lib.rs", "score")],
            tests: vec![test_with_oracle(
                "tests/errors.rs",
                "revoked_token_is_broad",
                "score(\"\")",
                oracle_fact(
                    "assert!(score(\"\").is_err());",
                    OracleKind::BroadError,
                    OracleStrength::Weak,
                ),
            )],
            ..RustIndex::default()
        };
        let probe = Probe {
            id: ProbeId("probe:src_lib_rs:2:error_path".to_string()),
            location: SourceLocation::new("src/lib.rs", 2, 1),
            owner: Some(SymbolId("src/lib.rs::score".to_string())),
            family: ProbeFamily::ErrorPath,
            delta: DeltaKind::Control,
            before: None,
            after: Some("Err(AuthError::RevokedToken)".to_string()),
            expression: "Err(AuthError::RevokedToken)".to_string(),
            expected_sinks: vec![],
            required_oracles: vec![],
        };

        let finding = classify_probe(&probe, &index);

        assert_eq!(finding.ripr.reveal.discriminate.state, StageState::Weak);
        // RIPR-SPEC-0107: ErrorPath now in needs_token_confirmation. A broad
        // is_err() oracle has no variant token (AuthError/RevokedToken) in its
        // text, so observation_unverified fires instead of the oracle-strength
        // "Only broad error oracle found" message.
        assert!(
            finding
                .ripr
                .reveal
                .discriminate
                .summary
                .contains("observation_unverified"),
            "broad is_err() oracle must emit observation_unverified for error_path after RIPR-SPEC-0107: got `{}`",
            finding.ripr.reveal.discriminate.summary
        );
        assert_eq!(
            finding.related_tests[0].oracle_strength,
            OracleStrength::Weak
        );
        assert!(
            finding
                .missing
                .iter()
                .any(|missing| { missing == "No exact error variant discriminator was detected" })
        );
    }

    #[test]
    fn given_unwrap_only_test_when_return_value_probe_changes_then_oracle_is_smoke() {
        let unwrap_only = format!("score(1).{}();", "unwrap");
        let index = RustIndex {
            functions: vec![FunctionSummary {
                body: "pub fn score(input: i32) -> Result<i32, Error> { Ok(input) }".to_string(),
                ..function("src/lib.rs", "score")
            }],
            tests: vec![test_with_oracle(
                "tests/score.rs",
                "score_smoke",
                "score(1)",
                oracle_fact(&unwrap_only, OracleKind::SmokeOnly, OracleStrength::Smoke),
            )],
            ..RustIndex::default()
        };
        let probe = Probe {
            id: ProbeId("probe:src_lib_rs:2:return_value".to_string()),
            location: SourceLocation::new("src/lib.rs", 2, 1),
            owner: Some(SymbolId("src/lib.rs::score".to_string())),
            family: ProbeFamily::ReturnValue,
            delta: DeltaKind::Value,
            before: None,
            after: Some("return Ok(input + 1)".to_string()),
            expression: "return Ok(input + 1)".to_string(),
            expected_sinks: vec![],
            required_oracles: vec![],
        };

        let finding = classify_probe(&probe, &index);

        assert_eq!(finding.ripr.reveal.discriminate.state, StageState::Weak);
        // The smoke-only assertion score(1).unwrap() contains no token from the
        // probe expression "return Ok(input + 1)" — observation_unverified fires
        // before the smoke oracle path, so the summary reflects the honesty fix.
        assert!(
            finding
                .ripr
                .reveal
                .discriminate
                .summary
                .contains("observation_unverified"),
            "ReturnValue with no token_match must emit observation_unverified: got `{}`",
            finding.ripr.reveal.discriminate.summary
        );
        assert_eq!(
            finding.related_tests[0].oracle_strength,
            OracleStrength::Smoke
        );
    }

    #[test]
    fn given_broad_error_assertion_when_non_error_probe_changes_then_gap_stays_generic() {
        let index = RustIndex {
            functions: vec![function("src/lib.rs", "score")],
            tests: vec![test_with_oracle(
                "tests/score.rs",
                "score_call_is_broad",
                "score(1)",
                oracle_fact(
                    "assert!(score(1).is_err());",
                    OracleKind::BroadError,
                    OracleStrength::Weak,
                ),
            )],
            ..RustIndex::default()
        };
        let probe = Probe {
            id: ProbeId("probe:src_lib_rs:2:call_deletion".to_string()),
            location: SourceLocation::new("src/lib.rs", 2, 1),
            owner: Some(SymbolId("src/lib.rs::score".to_string())),
            family: ProbeFamily::CallDeletion,
            delta: DeltaKind::Effect,
            before: None,
            after: Some("client.send(input)".to_string()),
            expression: "client.send(input)".to_string(),
            expected_sinks: vec![],
            required_oracles: vec![],
        };

        let finding = classify_probe(&probe, &index);

        assert_eq!(finding.ripr.reveal.discriminate.state, StageState::Weak);
        // The broad error assertion "assert!(score(1).is_err())" contains no
        // token from the CallDeletion probe "client.send(input)" — observation_unverified
        // fires before the broad-error oracle path, reflecting the honesty fix.
        assert!(
            finding
                .ripr
                .reveal
                .discriminate
                .summary
                .contains("observation_unverified"),
            "CallDeletion with no token_match must emit observation_unverified: got `{}`",
            finding.ripr.reveal.discriminate.summary
        );
        assert!(
            finding
                .missing
                .iter()
                .any(|missing| { missing == "No strong discriminator was detected" })
        );
        assert!(
            !finding
                .missing
                .iter()
                .any(|missing| { missing == "No exact error variant discriminator was detected" })
        );
    }

    #[test]
    fn given_changed_predicate_when_branch_returns_value_then_flow_sink_is_return_value() {
        let function = FunctionSummary {
            body: r#"pub fn score(amount: i32, threshold: i32) -> i32 {
    if amount >= threshold {
        amount - 10
    } else {
        amount
    }
}"#
            .to_string(),
            start_line: 1,
            end_line: 7,
            ..function("src/lib.rs", "score")
        };
        let index = RustIndex {
            functions: vec![function],
            tests: vec![test(
                "tests/score.rs",
                "score_threshold",
                "score(100, 50)",
                "assert_eq!(score(100, 50), 90);",
            )],
            ..RustIndex::default()
        };
        let probe = Probe {
            id: ProbeId("probe:src_lib_rs:2:predicate".to_string()),
            location: SourceLocation::new("src/lib.rs", 2, 1),
            owner: Some(SymbolId("src/lib.rs::score".to_string())),
            family: ProbeFamily::Predicate,
            delta: DeltaKind::Control,
            before: None,
            after: Some("amount >= threshold".to_string()),
            expression: "amount >= threshold".to_string(),
            expected_sinks: vec![],
            required_oracles: vec![],
        };

        let finding = classify_probe(&probe, &index);

        assert_eq!(finding.flow_sinks.len(), 1);
        assert_eq!(finding.flow_sinks[0].kind, FlowSinkKind::ReturnValue);
        assert_eq!(finding.flow_sinks[0].text, "amount - 10");
        assert_eq!(finding.flow_sinks[0].line, 3);
        assert_eq!(
            finding.ripr.propagate.summary,
            "Changed behavior appears to influence returned value: amount - 10"
        );
    }

    #[test]
    fn given_changed_error_variant_when_result_err_is_returned_then_flow_sink_is_error_variant() {
        let index = RustIndex {
            functions: vec![function("src/lib.rs", "score")],
            tests: vec![test_with_oracle(
                "tests/errors.rs",
                "revoked_token_is_broad",
                "score(\"\")",
                oracle_fact(
                    "assert!(score(\"\").is_err());",
                    OracleKind::BroadError,
                    OracleStrength::Weak,
                ),
            )],
            ..RustIndex::default()
        };
        let probe = Probe {
            id: ProbeId("probe:src_lib_rs:2:error_path".to_string()),
            location: SourceLocation::new("src/lib.rs", 2, 1),
            owner: Some(SymbolId("src/lib.rs::score".to_string())),
            family: ProbeFamily::ErrorPath,
            delta: DeltaKind::Value,
            before: None,
            after: Some("return Err(AuthError::RevokedToken);".to_string()),
            expression: "return Err(AuthError::RevokedToken);".to_string(),
            expected_sinks: vec![],
            required_oracles: vec![],
        };

        let finding = classify_probe(&probe, &index);

        assert_eq!(finding.flow_sinks.len(), 1);
        assert_eq!(finding.flow_sinks[0].kind, FlowSinkKind::ErrorVariant);
        assert_eq!(
            finding.flow_sinks[0].text,
            "Result::Err(AuthError::RevokedToken)"
        );
    }

    #[test]
    fn given_changed_side_effect_call_when_event_is_published_then_flow_sink_is_event_call() {
        let index = RustIndex {
            functions: vec![function("src/lib.rs", "score")],
            tests: vec![test(
                "tests/score.rs",
                "score_publishes",
                "score(1)",
                "assert_eq!(score(1), 2);",
            )],
            ..RustIndex::default()
        };
        let probe = Probe {
            id: ProbeId("probe:src_lib_rs:2:side_effect".to_string()),
            location: SourceLocation::new("src/lib.rs", 2, 1),
            owner: Some(SymbolId("src/lib.rs::score".to_string())),
            family: ProbeFamily::SideEffect,
            delta: DeltaKind::Effect,
            before: None,
            after: Some("events.publish(score)".to_string()),
            expression: "events.publish(score)".to_string(),
            expected_sinks: vec![],
            required_oracles: vec![],
        };

        let finding = classify_probe(&probe, &index);

        assert_eq!(finding.flow_sinks.len(), 1);
        assert_eq!(finding.flow_sinks[0].kind, FlowSinkKind::EventCall);
        assert_eq!(finding.flow_sinks[0].text, "events.publish(score)");
    }

    #[test]
    fn given_changed_field_construction_when_field_is_assigned_then_flow_sink_is_struct_field() {
        let index = RustIndex {
            functions: vec![function("src/lib.rs", "score")],
            tests: vec![test(
                "tests/score.rs",
                "score_builds_field",
                "score(1)",
                "assert_eq!(score(1).total, 2);",
            )],
            ..RustIndex::default()
        };
        let probe = Probe {
            id: ProbeId("probe:src_lib_rs:2:field_construction".to_string()),
            location: SourceLocation::new("src/lib.rs", 2, 1),
            owner: Some(SymbolId("src/lib.rs::score".to_string())),
            family: ProbeFamily::FieldConstruction,
            delta: DeltaKind::Value,
            before: None,
            after: Some("total: computed_total".to_string()),
            expression: "total: computed_total".to_string(),
            expected_sinks: vec![],
            required_oracles: vec![],
        };

        let finding = classify_probe(&probe, &index);

        assert_eq!(finding.flow_sinks.len(), 1);
        assert_eq!(finding.flow_sinks[0].kind, FlowSinkKind::StructField);
        assert_eq!(finding.flow_sinks[0].text, "total: computed_total");
    }

    #[test]
    fn given_changed_match_arm_when_arm_returns_value_then_flow_sink_is_match_arm_return() {
        let index = RustIndex {
            functions: vec![function("src/lib.rs", "score")],
            tests: vec![test(
                "tests/score.rs",
                "score_matches",
                "score(1)",
                "assert_eq!(score(1), 2);",
            )],
            ..RustIndex::default()
        };
        let probe = Probe {
            id: ProbeId("probe:src_lib_rs:2:match_arm".to_string()),
            location: SourceLocation::new("src/lib.rs", 2, 1),
            owner: Some(SymbolId("src/lib.rs::score".to_string())),
            family: ProbeFamily::MatchArm,
            delta: DeltaKind::Control,
            before: None,
            after: Some("Some(value) => value + 1,".to_string()),
            expression: "Some(value) => value + 1,".to_string(),
            expected_sinks: vec![],
            required_oracles: vec![],
        };

        let finding = classify_probe(&probe, &index);

        assert_eq!(finding.flow_sinks.len(), 1);
        assert_eq!(finding.flow_sinks[0].kind, FlowSinkKind::MatchArm);
        assert_eq!(finding.flow_sinks[0].text, "value + 1");
    }

    #[test]
    fn given_changed_return_binding_when_function_returns_ok_then_flow_sink_is_return_value() {
        let function = FunctionSummary {
            body: "pub fn score(input: i32) -> Result<i32, Error> { let value = input + 1; Ok(value) }"
                .to_string(),
            returns: vec![ReturnFact {
                line: 1,
                text: "Ok(value)".to_string(),
            }],
            ..function("src/lib.rs", "score")
        };
        let index = RustIndex {
            functions: vec![function],
            tests: vec![test(
                "tests/score.rs",
                "score_returns_value",
                "score(1)",
                "assert_eq!(score(1), Ok(2));",
            )],
            ..RustIndex::default()
        };
        let probe = Probe {
            id: ProbeId("probe:src_lib_rs:1:return_value".to_string()),
            location: SourceLocation::new("src/lib.rs", 1, 1),
            owner: Some(SymbolId("src/lib.rs::score".to_string())),
            family: ProbeFamily::ReturnValue,
            delta: DeltaKind::Value,
            before: None,
            after: Some("let value = input + 1".to_string()),
            expression: "let value = input + 1".to_string(),
            expected_sinks: vec![],
            required_oracles: vec![],
        };

        let finding = classify_probe(&probe, &index);

        assert_eq!(finding.flow_sinks.len(), 1);
        assert_eq!(finding.flow_sinks[0].kind, FlowSinkKind::ReturnValue);
        assert_eq!(finding.flow_sinks[0].text, "Ok(value)");
    }

    #[test]
    fn given_changed_predicate_when_branch_returns_error_then_flow_sink_is_error_variant() {
        let function = FunctionSummary {
            body: r#"pub fn authenticate(token: &str) -> Result<User, AuthError> {
    if token.is_empty() {
        return Err(AuthError::RevokedToken);
    }
    Ok(User)
}"#
            .to_string(),
            start_line: 1,
            end_line: 6,
            returns: vec![
                ReturnFact {
                    line: 3,
                    text: "return Err(AuthError::RevokedToken);".to_string(),
                },
                ReturnFact {
                    line: 5,
                    text: "Ok(User)".to_string(),
                },
            ],
            ..function("src/lib.rs", "authenticate")
        };
        let index = RustIndex {
            functions: vec![function],
            tests: vec![test_with_oracle(
                "tests/auth.rs",
                "empty_token_is_rejected",
                "authenticate(\"\")",
                oracle_fact(
                    "assert!(authenticate(\"\").is_err());",
                    OracleKind::BroadError,
                    OracleStrength::Weak,
                ),
            )],
            ..RustIndex::default()
        };
        let probe = Probe {
            id: ProbeId("probe:src_lib_rs:2:predicate".to_string()),
            location: SourceLocation::new("src/lib.rs", 2, 1),
            owner: Some(SymbolId("src/lib.rs::authenticate".to_string())),
            family: ProbeFamily::Predicate,
            delta: DeltaKind::Control,
            before: None,
            after: Some("token.is_empty()".to_string()),
            expression: "token.is_empty()".to_string(),
            expected_sinks: vec![],
            required_oracles: vec![],
        };

        let finding = classify_probe(&probe, &index);

        assert_eq!(finding.flow_sinks.len(), 1);
        assert_eq!(finding.flow_sinks[0].kind, FlowSinkKind::ErrorVariant);
        assert_eq!(
            finding.flow_sinks[0].text,
            "Result::Err(AuthError::RevokedToken)"
        );
    }

    #[test]
    fn given_changed_predicate_when_branch_constructs_field_then_flow_sink_is_struct_field() {
        let function = FunctionSummary {
            body: r#"pub fn quote(amount: i32) -> Quote {
    if amount > 0 {
        Quote {
            total: amount - 10,
        }
    } else {
        Quote {
            total: amount,
        }
    }
}"#
            .to_string(),
            start_line: 1,
            end_line: 11,
            ..function("src/lib.rs", "quote")
        };
        let index = RustIndex {
            functions: vec![function],
            tests: vec![test(
                "tests/quote.rs",
                "positive_quote_has_total",
                "quote(100)",
                "assert_eq!(quote(100).total, 90);",
            )],
            ..RustIndex::default()
        };
        let probe = Probe {
            id: ProbeId("probe:src_lib_rs:2:predicate".to_string()),
            location: SourceLocation::new("src/lib.rs", 2, 1),
            owner: Some(SymbolId("src/lib.rs::quote".to_string())),
            family: ProbeFamily::Predicate,
            delta: DeltaKind::Control,
            before: None,
            after: Some("amount > 0".to_string()),
            expression: "amount > 0".to_string(),
            expected_sinks: vec![],
            required_oracles: vec![],
        };

        let finding = classify_probe(&probe, &index);

        assert_eq!(finding.flow_sinks.len(), 1);
        assert_eq!(finding.flow_sinks[0].kind, FlowSinkKind::StructField);
        assert_eq!(finding.flow_sinks[0].text, "total: amount - 10");
    }

    #[test]
    fn given_changed_predicate_when_return_contains_colon_in_string_then_flow_sink_is_return_value()
    {
        let function = FunctionSummary {
            body: r#"pub fn message(code: i32) -> String {
    if code > 0 {
        format!("error:{code}")
    } else {
        "ok".to_string()
    }
}"#
            .to_string(),
            start_line: 1,
            end_line: 7,
            ..function("src/lib.rs", "message")
        };
        let index = RustIndex {
            functions: vec![function],
            tests: vec![test(
                "tests/message.rs",
                "message_returns_error_code",
                "message(1)",
                "assert_eq!(message(1), \"error:1\");",
            )],
            ..RustIndex::default()
        };
        let probe = Probe {
            id: ProbeId("probe:src_lib_rs:2:predicate".to_string()),
            location: SourceLocation::new("src/lib.rs", 2, 1),
            owner: Some(SymbolId("src/lib.rs::message".to_string())),
            family: ProbeFamily::Predicate,
            delta: DeltaKind::Control,
            before: None,
            after: Some("code > 0".to_string()),
            expression: "code > 0".to_string(),
            expected_sinks: vec![],
            required_oracles: vec![],
        };

        let finding = classify_probe(&probe, &index);

        assert_eq!(finding.flow_sinks.len(), 1);
        assert_eq!(finding.flow_sinks[0].kind, FlowSinkKind::ReturnValue);
        assert_eq!(finding.flow_sinks[0].text, "format!(\"error:{code}\")");
    }

    #[test]
    fn given_changed_predicate_when_next_line_is_assignment_then_flow_sink_stays_unknown() {
        let function = FunctionSummary {
            body: r#"pub fn score(amount: i32, threshold: i32) -> i32 {
    if amount >= threshold {
        let discounted = amount - 10;
        discounted
    } else {
        amount
    }
}"#
            .to_string(),
            start_line: 1,
            end_line: 8,
            ..function("src/lib.rs", "score")
        };
        let index = RustIndex {
            functions: vec![function],
            tests: vec![test(
                "tests/score.rs",
                "score_threshold",
                "score(100, 50)",
                "assert_eq!(score(100, 50), 90);",
            )],
            ..RustIndex::default()
        };
        let probe = Probe {
            id: ProbeId("probe:src_lib_rs:2:predicate".to_string()),
            location: SourceLocation::new("src/lib.rs", 2, 1),
            owner: Some(SymbolId("src/lib.rs::score".to_string())),
            family: ProbeFamily::Predicate,
            delta: DeltaKind::Control,
            before: None,
            after: Some("amount >= threshold".to_string()),
            expression: "amount >= threshold".to_string(),
            expected_sinks: vec![],
            required_oracles: vec![],
        };

        let finding = classify_probe(&probe, &index);

        assert!(finding.flow_sinks.is_empty());
        assert_eq!(finding.ripr.propagate.state, StageState::Unknown);
    }

    #[test]
    fn given_changed_return_after_early_return_when_no_downstream_return_exists_then_sink_is_unknown()
     {
        let function = FunctionSummary {
            body: r#"pub fn score(amount: i32) -> i32 {
    if amount < 0 {
        return 0;
    }
    let adjusted = amount;
    adjusted
}"#
            .to_string(),
            start_line: 1,
            end_line: 7,
            returns: vec![ReturnFact {
                line: 3,
                text: "return 0;".to_string(),
            }],
            ..function("src/lib.rs", "score")
        };
        let index = RustIndex {
            functions: vec![function],
            tests: vec![test(
                "tests/score.rs",
                "score_positive",
                "score(1)",
                "assert_eq!(score(1), 1);",
            )],
            ..RustIndex::default()
        };
        let probe = Probe {
            id: ProbeId("probe:src_lib_rs:5:return_value".to_string()),
            location: SourceLocation::new("src/lib.rs", 5, 1),
            owner: Some(SymbolId("src/lib.rs::score".to_string())),
            family: ProbeFamily::ReturnValue,
            delta: DeltaKind::Value,
            before: None,
            after: Some("adjusted".to_string()),
            expression: "adjusted".to_string(),
            expected_sinks: vec![],
            required_oracles: vec![],
        };

        let finding = classify_probe(&probe, &index);

        assert_eq!(finding.flow_sinks.len(), 1);
        assert_eq!(finding.flow_sinks[0].kind, FlowSinkKind::Unknown);
        assert_eq!(finding.ripr.propagate.state, StageState::Unknown);
    }

    #[test]
    fn given_changed_call_deletion_when_result_ok_is_returned_then_flow_sink_is_return_value() {
        let index = RustIndex {
            functions: vec![function("src/lib.rs", "score")],
            tests: vec![test(
                "tests/score.rs",
                "score_returns_value",
                "score(1)",
                "assert_eq!(score(1), Ok(2));",
            )],
            ..RustIndex::default()
        };
        let probe = Probe {
            id: ProbeId("probe:src_lib_rs:2:call".to_string()),
            location: SourceLocation::new("src/lib.rs", 2, 1),
            owner: Some(SymbolId("src/lib.rs::score".to_string())),
            family: ProbeFamily::CallDeletion,
            delta: DeltaKind::Effect,
            before: None,
            after: Some("Ok(total)".to_string()),
            expression: "Ok(total)".to_string(),
            expected_sinks: vec![],
            required_oracles: vec![],
        };

        let finding = classify_probe(&probe, &index);

        assert_eq!(finding.flow_sinks.len(), 1);
        assert_eq!(finding.flow_sinks[0].kind, FlowSinkKind::ReturnValue);
        assert_eq!(finding.flow_sinks[0].text, "Ok(total)");
    }

    #[test]
    fn given_changed_match_arm_when_arm_returns_error_then_flow_sink_is_error_variant() {
        let index = RustIndex {
            functions: vec![function("src/lib.rs", "authenticate")],
            tests: vec![test_with_oracle(
                "tests/auth.rs",
                "revoked_token_is_exact",
                "authenticate(\"\")",
                oracle_fact(
                    "assert_matches!(authenticate(\"\"), Err(AuthError::RevokedToken));",
                    OracleKind::ExactErrorVariant,
                    OracleStrength::Strong,
                ),
            )],
            ..RustIndex::default()
        };
        let probe = Probe {
            id: ProbeId("probe:src_lib_rs:2:match_arm".to_string()),
            location: SourceLocation::new("src/lib.rs", 2, 1),
            owner: Some(SymbolId("src/lib.rs::authenticate".to_string())),
            family: ProbeFamily::MatchArm,
            delta: DeltaKind::Control,
            before: None,
            after: Some("None => Err(AuthError::RevokedToken),".to_string()),
            expression: "None => Err(AuthError::RevokedToken),".to_string(),
            expected_sinks: vec![],
            required_oracles: vec![],
        };

        let finding = classify_probe(&probe, &index);

        assert_eq!(finding.flow_sinks.len(), 1);
        assert_eq!(finding.flow_sinks[0].kind, FlowSinkKind::ErrorVariant);
        assert_eq!(
            finding.flow_sinks[0].text,
            "Result::Err(AuthError::RevokedToken)"
        );
    }

    #[test]
    fn given_changed_opaque_return_expression_when_no_sink_is_obvious_then_propagation_is_unknown()
    {
        let index = RustIndex {
            functions: vec![function("src/lib.rs", "score")],
            tests: vec![test(
                "tests/score.rs",
                "score_returns_value",
                "score(1)",
                "assert_eq!(score(1), 2);",
            )],
            ..RustIndex::default()
        };
        let probe = Probe {
            id: ProbeId("probe:src_lib_rs:2:return_value".to_string()),
            location: SourceLocation::new("src/lib.rs", 2, 1),
            owner: Some(SymbolId("src/lib.rs::score".to_string())),
            family: ProbeFamily::ReturnValue,
            delta: DeltaKind::Value,
            before: None,
            after: Some("value".to_string()),
            expression: "value".to_string(),
            expected_sinks: vec![],
            required_oracles: vec![],
        };

        let finding = classify_probe(&probe, &index);

        assert_eq!(finding.flow_sinks.len(), 1);
        assert_eq!(finding.flow_sinks[0].kind, FlowSinkKind::Unknown);
        assert_eq!(finding.ripr.propagate.state, StageState::Unknown);
        assert_eq!(
            finding.ripr.propagate.summary,
            "Propagation is not statically obvious from syntax-first analysis"
        );
    }

    #[test]
    fn given_boundary_predicate_when_tests_skip_equal_value_then_activation_names_missing_boundary()
    {
        let function = FunctionSummary {
            body: r#"pub fn score(amount: i32, threshold: i32) -> i32 {
    if amount >= threshold {
        amount - 10
    } else {
        amount
    }
}"#
            .to_string(),
            start_line: 1,
            end_line: 7,
            ..function("src/lib.rs", "score")
        };
        let index = RustIndex {
            functions: vec![function],
            tests: vec![
                test(
                    "tests/score.rs",
                    "below_threshold_has_no_discount",
                    "score(50, 100)",
                    "assert_eq!(score(50, 100), 50);",
                ),
                test(
                    "tests/score.rs",
                    "far_above_threshold_discounts",
                    "score(10_000, 100)",
                    "assert_eq!(score(10_000, 100), 9_990);",
                ),
            ],
            ..RustIndex::default()
        };
        let probe = Probe {
            id: ProbeId("probe:src_lib_rs:2:predicate".to_string()),
            location: SourceLocation::new("src/lib.rs", 2, 1),
            owner: Some(SymbolId("src/lib.rs::score".to_string())),
            family: ProbeFamily::Predicate,
            delta: DeltaKind::Control,
            before: None,
            after: Some("amount >= threshold".to_string()),
            expression: "amount >= threshold".to_string(),
            expected_sinks: vec![],
            required_oracles: vec![],
        };

        let finding = classify_probe(&probe, &index);

        assert_eq!(finding.class, ExposureClass::WeaklyExposed);
        assert_eq!(finding.ripr.infect.state, StageState::Weak);
        assert!(finding.activation.observed_values.iter().any(|fact| {
            fact.context == ValueContext::FunctionArgument && fact.value == "amount = 50"
        }));
        assert!(finding.activation.observed_values.iter().any(|fact| {
            fact.context == ValueContext::FunctionArgument && fact.value == "amount = 10_000"
        }));
        assert!(finding.activation.observed_values.iter().any(|fact| {
            fact.context == ValueContext::FunctionArgument && fact.value == "threshold = 100"
        }));
        assert_eq!(finding.activation.missing_discriminators.len(), 1);
        assert_eq!(
            finding.activation.missing_discriminators[0].value,
            "amount == threshold"
        );
        assert_eq!(
            finding.activation.missing_discriminators[0]
                .flow_sink
                .as_ref()
                .map(|sink| &sink.kind),
            Some(&FlowSinkKind::ReturnValue)
        );
    }

    #[test]
    fn given_boundary_predicate_when_equal_value_exists_then_activation_has_no_missing_boundary() {
        let function = FunctionSummary {
            body: r#"pub fn score(amount: i32, threshold: i32) -> i32 {
    if amount >= threshold {
        amount - 10
    } else {
        amount
    }
}"#
            .to_string(),
            start_line: 1,
            end_line: 7,
            ..function("src/lib.rs", "score")
        };
        let index = RustIndex {
            functions: vec![function],
            tests: vec![test(
                "tests/score.rs",
                "equal_threshold_discounts",
                "score(100, 100)",
                "assert_eq!(score(100, 100), 90);",
            )],
            ..RustIndex::default()
        };
        let probe = Probe {
            id: ProbeId("probe:src_lib_rs:2:predicate".to_string()),
            location: SourceLocation::new("src/lib.rs", 2, 1),
            owner: Some(SymbolId("src/lib.rs::score".to_string())),
            family: ProbeFamily::Predicate,
            delta: DeltaKind::Control,
            before: None,
            after: Some("amount >= threshold".to_string()),
            expression: "amount >= threshold".to_string(),
            expected_sinks: vec![],
            required_oracles: vec![],
        };

        let finding = classify_probe(&probe, &index);

        assert_eq!(finding.ripr.infect.state, StageState::Yes);
        assert!(finding.activation.missing_discriminators.is_empty());
        assert!(finding.activation.observed_values.iter().any(|fact| {
            fact.context == ValueContext::FunctionArgument && fact.value == "amount == threshold"
        }));
    }

    #[test]
    fn given_error_path_probe_when_test_uses_is_err_then_exact_error_variant_is_missing() {
        let function = FunctionSummary {
            body: r#"pub fn score(token: &str) -> Result<&'static str, AuthError> {
    if token.is_empty() {
        return Err(AuthError::RevokedToken);
    }
    Ok("accepted")
}"#
            .to_string(),
            start_line: 1,
            end_line: 6,
            returns: vec![
                ReturnFact {
                    line: 3,
                    text: "return Err(AuthError::RevokedToken);".to_string(),
                },
                ReturnFact {
                    line: 5,
                    text: "Ok(\"accepted\")".to_string(),
                },
            ],
            ..function("src/lib.rs", "score")
        };
        let index = RustIndex {
            functions: vec![function],
            tests: vec![test_with_oracle(
                "tests/errors.rs",
                "empty_token_is_rejected",
                "score(\"\")",
                oracle_fact(
                    "assert!(score(\"\").is_err());",
                    OracleKind::BroadError,
                    OracleStrength::Weak,
                ),
            )],
            ..RustIndex::default()
        };
        let probe = Probe {
            id: ProbeId("probe:src_lib_rs:3:error_path".to_string()),
            location: SourceLocation::new("src/lib.rs", 3, 1),
            owner: Some(SymbolId("src/lib.rs::score".to_string())),
            family: ProbeFamily::ErrorPath,
            delta: DeltaKind::Value,
            before: None,
            after: Some("return Err(AuthError::RevokedToken);".to_string()),
            expression: "return Err(AuthError::RevokedToken);".to_string(),
            expected_sinks: vec![],
            required_oracles: vec![],
        };

        let finding = classify_probe(&probe, &index);

        assert!(finding.activation.observed_values.iter().any(|fact| {
            fact.context == ValueContext::FunctionArgument && fact.value == "token = \"\""
        }));
        assert_eq!(finding.activation.missing_discriminators.len(), 1);
        assert_eq!(
            finding.activation.missing_discriminators[0].value,
            "AuthError::RevokedToken"
        );
        assert_eq!(
            finding.activation.missing_discriminators[0]
                .flow_sink
                .as_ref()
                .map(|sink| &sink.kind),
            Some(&FlowSinkKind::ErrorVariant)
        );
    }

    #[test]
    fn given_probe_family_and_exposure_class_when_recommending_next_step_then_guidance_matches() {
        let predicate_probe = probe(ProbeFamily::Predicate, DeltaKind::Control, "value > 10");
        let return_value_probe = probe(ProbeFamily::ReturnValue, DeltaKind::Value, "value + 1");

        assert_eq!(
            recommended_next_step(&predicate_probe, &ExposureClass::Exposed, false),
            None
        );
        assert_eq!(
            recommended_next_step(&predicate_probe, &ExposureClass::WeaklyExposed, false)
                .as_deref(),
            Some(
                "Add boundary tests for below, equal, and above the changed threshold with exact assertions."
            )
        );
        assert_eq!(
            recommended_next_step(&return_value_probe, &ExposureClass::WeaklyExposed, false)
                .as_deref(),
            Some(
                "Replace broad assertions with exact equality or a property that constrains the changed returned value."
            )
        );
        assert_eq!(
            recommended_next_step(&predicate_probe, &ExposureClass::ReachableUnrevealed, false)
                .as_deref(),
            Some(
                "Add a meaningful assertion that observes the changed value, branch, error, field, event, or side effect."
            )
        );
        assert_eq!(
            recommended_next_step(&predicate_probe, &ExposureClass::NoStaticPath, false).as_deref(),
            Some(NO_STATIC_PATH_NEXT_STEP)
        );
        assert_eq!(
            recommended_next_step(&predicate_probe, &ExposureClass::InfectionUnknown, false)
                .as_deref(),
            Some(
                "Add a targeted boundary or negative-path test, or teach ripr about the fixture/builder in ripr.toml."
            )
        );
        assert_eq!(
            recommended_next_step(&predicate_probe, &ExposureClass::StaticUnknown, false)
                .as_deref(),
            Some("Escalate to real mutation testing or deep static analysis for this probe.")
        );
    }

    // RIPR-SPEC-0133: end-to-end through `classify_probe` — an assertion-shaped
    // owner keeps its exposure class but gets oracle-shaped guidance plus the
    // `owner_shape` disclosure line.
    #[test]
    fn given_assertion_shaped_owner_when_classifying_then_guidance_is_reframed_not_reclassified() {
        let mut owner = function("tests/fragments.rs", "assert_paths_are_stable");
        owner.body = "fn assert_paths_are_stable(spans: &[Span]) {\n    for span in spans {\n        assert!(!span.file.contains('\\\\'));\n        assert_eq!(span.root.as_deref(), Some(\"src\"));\n    }\n}\n".to_string();
        let index = RustIndex {
            functions: vec![owner],
            tests: vec![test(
                "tests/fragments.rs",
                "paths_stay_stable",
                "assert_paths_are_stable(&spans)",
                "assert!(true);",
            )],
            ..RustIndex::default()
        };
        let probe = Probe {
            id: ProbeId("probe:tests_fragments_rs:2:predicate".to_string()),
            location: SourceLocation::new("tests/fragments.rs", 3, 1),
            owner: Some(SymbolId(
                "tests/fragments.rs::assert_paths_are_stable".to_string(),
            )),
            family: ProbeFamily::Predicate,
            delta: DeltaKind::Control,
            before: None,
            after: Some("!span.file.contains('\\\\')".to_string()),
            expression: "!span.file.contains('\\\\')".to_string(),
            expected_sinks: vec![],
            required_oracles: vec![],
        };

        let finding = classify_probe(&probe, &index);

        let step = finding.recommended_next_step.as_deref().unwrap_or("");
        assert!(
            finding
                .evidence
                .iter()
                .any(|line| line.starts_with("owner_shape: assertion_shaped (")),
            "finding must disclose the assertion-shaped owner shape; evidence: {:?}",
            finding.evidence
        );
        assert!(
            !step.contains("co-located test")
                && !step.contains("Replace broad assertions")
                && !step.contains("fixture/builder in ripr.toml"),
            "guidance must be reframed for the oracle, got: {step}"
        );
        // The unknown/static classes keep their standard escalation text; every
        // other class maps to a reframed const.
        let expected = match finding.class {
            ExposureClass::Exposed => None,
            ExposureClass::WeaklyExposed => Some(ASSERTION_SHAPED_WEAKLY_EXPOSED_NEXT_STEP),
            ExposureClass::ReachableUnrevealed => {
                Some(ASSERTION_SHAPED_REACHABLE_UNREVEALED_NEXT_STEP)
            }
            ExposureClass::NoStaticPath => Some(ASSERTION_SHAPED_NO_STATIC_PATH_NEXT_STEP),
            ExposureClass::InfectionUnknown => Some(ASSERTION_SHAPED_INFECTION_UNKNOWN_NEXT_STEP),
            ExposureClass::PropagationUnknown | ExposureClass::StaticUnknown => {
                Some("Escalate to real mutation testing or deep static analysis for this probe.")
            }
        };
        assert_eq!(
            finding.recommended_next_step.as_deref(),
            expected,
            "class {} guidance mismatch",
            finding.class.as_str()
        );
    }

    // RIPR-SPEC-0133 control: an assert-dominated helper WITH a production
    // caller keeps the standard code-under-test guidance.
    #[test]
    fn given_assertion_heavy_owner_with_production_caller_when_classifying_then_guidance_is_standard()
     {
        let mut owner = function("src/lib.rs", "check_invariants");
        owner.body = "fn check_invariants(value: i32) {\n    assert!(value >= 0);\n    assert_eq!(value % 2, 0);\n}\n".to_string();
        let mut production_caller = function("src/lib.rs", "validate");
        production_caller.calls = vec![CallFact {
            line: 2,
            name: "check_invariants".to_string(),
            text: "check_invariants(value);".to_string(),
        }];
        let index = RustIndex {
            functions: vec![owner, production_caller],
            tests: vec![test(
                "tests/lib.rs",
                "validate_works",
                "validate(2)",
                "assert_eq!(2 + 2, 4);",
            )],
            ..RustIndex::default()
        };
        let probe = Probe {
            id: ProbeId("probe:src_lib_rs:2:predicate".to_string()),
            location: SourceLocation::new("src/lib.rs", 2, 1),
            owner: Some(SymbolId("src/lib.rs::check_invariants".to_string())),
            family: ProbeFamily::Predicate,
            delta: DeltaKind::Control,
            before: None,
            after: Some("value >= 0".to_string()),
            expression: "value >= 0".to_string(),
            expected_sinks: vec![],
            required_oracles: vec![],
        };

        let finding = classify_probe(&probe, &index);

        assert!(
            !finding
                .evidence
                .iter()
                .any(|line| line.starts_with("owner_shape:")),
            "production-called helper must not disclose an oracle shape; evidence: {:?}",
            finding.evidence
        );
        if let Some(step) = finding.recommended_next_step.as_deref() {
            assert!(
                !step.contains(crate::domain::ASSERTION_SHAPED_OWNER_REASON),
                "guidance must stay in the code-under-test voice, got: {step}"
            );
        }
    }

    #[test]
    fn given_macro_like_expression_when_collecting_stop_reasons_then_ignores_inequality_tokens() {
        let inequality = probe(
            ProbeFamily::StaticUnknown,
            DeltaKind::Unknown,
            "value != threshold",
        );
        let unary_not = probe(ProbeFamily::StaticUnknown, DeltaKind::Unknown, "!enabled");
        let macro_with_inequality = probe(
            ProbeFamily::StaticUnknown,
            DeltaKind::Unknown,
            "value != threshold && trace!(value)",
        );

        assert_eq!(stop_reason_labels(&inequality), Vec::<&str>::new());
        assert_eq!(stop_reason_labels(&unary_not), Vec::<&str>::new());
        assert_eq!(
            stop_reason_labels(&macro_with_inequality),
            vec!["proc_macro_opaque"]
        );
    }

    #[test]
    fn given_duplicate_stop_reasons_when_collecting_then_results_are_deduplicated_and_sorted() {
        let probe = probe(
            ProbeFamily::StaticUnknown,
            DeltaKind::Unknown,
            "async move { spawn(task).await; trace!(task); }",
        );

        let labels = stop_reason_labels(&probe);
        assert_eq!(labels, vec!["async_boundary_opaque", "proc_macro_opaque"]);
    }

    #[test]
    fn stop_reasons_include_fixture_and_missing_owner_signals() {
        let probe = probe(
            ProbeFamily::CallDeletion,
            DeltaKind::Effect,
            "client.send(input)",
        );
        let fixture_test = test(
            "tests/service.rs",
            "service_uses_fixture",
            "score(1)",
            "let fixture = build_fixture(); assert_eq!(score(1), 2);",
        );

        let reasons = stop_reasons(&probe, None, &[&fixture_test]);
        let labels: Vec<&str> = reasons.iter().map(StopReason::as_str).collect();

        assert_eq!(labels, vec!["fixture_opaque", "no_changed_rust_line"]);
    }

    fn stop_reason_labels(probe: &Probe) -> Vec<&str> {
        let owner = function("crates/ripr/src/lib.rs", "dummy");
        let reasons = stop_reasons(probe, Some(&owner), &[]);
        let labels: Vec<&str> = reasons.iter().map(StopReason::as_str).collect();
        labels
    }

    fn probe(family: ProbeFamily, delta: DeltaKind, expression: &str) -> Probe {
        Probe {
            id: ProbeId("probe:test".to_string()),
            location: SourceLocation::new("crates/ripr/src/lib.rs", 1, 1),
            owner: None,
            family,
            delta,
            before: None,
            after: None,
            expression: expression.to_string(),
            expected_sinks: vec![],
            required_oracles: vec![],
        }
    }

    #[test]
    fn parser_expression_confirms_payload_without_rewriting_emitted_probe() -> Result<(), String> {
        let owner = function("src/gate_watchdog.rs", "classify_gate_watchdog");
        let owner_id = owner.id.clone();
        let path = PathBuf::from("src/gate_watchdog.rs");
        let assertion = r#"ensure!(reason.kind == "run-missing");"#;
        let index = RustIndex {
            functions: vec![owner],
            tests: vec![test_with_oracle(
                "src/tests/gate_watchdog_tests.rs",
                "classify_gate_watchdog_reports_run_missing",
                "classify_gate_watchdog(&input)",
                oracle_fact(assertion, OracleKind::ExactValue, OracleStrength::Strong),
            )],
            files: BTreeMap::from([(
                path,
                FileFacts {
                    path: PathBuf::from("src/gate_watchdog.rs"),
                    probe_shapes: vec![ProbeShapeFact {
                        start_line: 190,
                        end_line: 194,
                        start_byte: 1_024,
                        kind: PROBE_SHAPE_CALL_DELETION.to_string(),
                        text: "watchdog_reason(\n    \"run-missing\",\n    receipt,\n)".to_string(),
                    }],
                    ..FileFacts::default()
                },
            )]),
        };
        let probe = Probe {
            id: ProbeId("probe:watchdog-reason".to_string()),
            location: SourceLocation::new(
                PathBuf::from("/repo/ub-review/src/gate_watchdog.rs"),
                190,
                1,
            ),
            owner: Some(owner_id),
            family: ProbeFamily::CallDeletion,
            delta: DeltaKind::Effect,
            before: None,
            after: Some("watchdog_reason(".to_string()),
            expression: "watchdog_reason(".to_string(),
            expected_sinks: vec![],
            required_oracles: vec![],
        };

        let finding = classify_probe(&probe, &index);
        if finding.class != ExposureClass::Exposed {
            return Err(format!("parser payload was not discriminated: {finding:?}"));
        }
        if finding.probe.expression != "watchdog_reason(" {
            return Err(format!(
                "emitted probe expression changed: {}",
                finding.probe.expression
            ));
        }
        Ok(())
    }

    #[test]
    fn parser_argument_identifier_does_not_confirm_call_effect() -> Result<(), String> {
        let owner = function("src/gate_watchdog.rs", "classify_gate_watchdog");
        let owner_id = owner.id.clone();
        let path = PathBuf::from("src/gate_watchdog.rs");
        let assertion = "ensure!(reason.receipt == receipt);";
        let index = RustIndex {
            functions: vec![owner],
            tests: vec![test_with_oracle(
                "src/tests/gate_watchdog_tests.rs",
                "classify_gate_watchdog_preserves_receipt",
                "classify_gate_watchdog(&input)",
                oracle_fact(assertion, OracleKind::ExactValue, OracleStrength::Strong),
            )],
            files: BTreeMap::from([(
                path,
                FileFacts {
                    path: PathBuf::from("src/gate_watchdog.rs"),
                    probe_shapes: vec![ProbeShapeFact {
                        start_line: 190,
                        end_line: 194,
                        start_byte: 1_024,
                        kind: PROBE_SHAPE_CALL_DELETION.to_string(),
                        text: "watchdog_reason(\n    \"run-missing\",\n    receipt,\n)".to_string(),
                    }],
                    ..FileFacts::default()
                },
            )]),
        };
        let probe = Probe {
            id: ProbeId("probe:watchdog-reason".to_string()),
            location: SourceLocation::new(
                PathBuf::from("/repo/ub-review/src/gate_watchdog.rs"),
                190,
                1,
            ),
            owner: Some(owner_id),
            family: ProbeFamily::CallDeletion,
            delta: DeltaKind::Effect,
            before: None,
            after: Some("watchdog_reason(".to_string()),
            expression: "watchdog_reason(".to_string(),
            expected_sinks: vec![],
            required_oracles: vec![],
        };

        let finding = classify_probe(&probe, &index);
        if finding.class != ExposureClass::WeaklyExposed {
            return Err(format!(
                "argument identifier falsely confirmed call effect: {finding:?}"
            ));
        }
        if !finding
            .ripr
            .reveal
            .discriminate
            .summary
            .contains("observation_unverified")
        {
            return Err(format!(
                "missing observation boundary for argument-only oracle: {finding:?}"
            ));
        }
        Ok(())
    }

    fn function(file: &str, name: &str) -> FunctionSummary {
        FunctionSummary {
            id: SymbolId(format!("{file}::{name}")),
            name: name.to_string(),
            file: PathBuf::from(file),
            start_line: 1,
            end_line: 3,
            body: format!("pub fn {name}(input: i32) -> i32 {{ input }}"),
            calls: vec![],
            returns: vec![],
            literals: vec![],
            is_test: false,
            attrs: vec![],
        }
    }

    fn test(file: &str, name: &str, call: &str, assertion: &str) -> TestSummary {
        test_with_oracle(
            file,
            name,
            call,
            oracle_fact(assertion, OracleKind::ExactValue, OracleStrength::Strong),
        )
    }

    fn test_with_oracle(file: &str, name: &str, call: &str, oracle: OracleFact) -> TestSummary {
        let body = format!("{call};\n{}", oracle.text.as_str());
        TestSummary {
            name: name.to_string(),
            file: PathBuf::from(file),
            start_line: 1,
            end_line: 4,
            body,
            calls: vec![CallFact {
                line: 1,
                name: "score".to_string(),
                text: call.to_string(),
            }],
            assertions: vec![oracle],
            literals: vec![LiteralFact {
                line: 1,
                value: "1".to_string(),
            }],
            attrs: vec![],
        }
    }

    fn oracle_fact(assertion: &str, kind: OracleKind, strength: OracleStrength) -> OracleFact {
        OracleFact {
            line: 2,
            text: assertion.to_string(),
            kind,
            strength,
            observed_tokens: extract_identifier_tokens(assertion),
        }
    }
}
