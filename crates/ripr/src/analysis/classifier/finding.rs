use super::evidence::ClassifiedProbeEvidence;
use crate::analysis::classify::{
    BOUNDARY_OPERAND_UNRESOLVED_MARKER, ProbeContext, body_contains_owner_call,
    ensure_unknown_stop_reason, exact_error_variant, missing_evidence, recommended_next_step,
    stop_reasons, unresolved_guard_error_edge,
};

/// #1429: repair assignment for a predicate boundary discriminator the
/// analyzer cannot confirm missing. It must not prescribe a specific
/// boundary input the suite may already contain; the typed limitation
/// names what static analysis cannot establish, deferring the
/// discrimination verdict to real mutation testing.
pub(in crate::analysis) const BOUNDARY_OPERAND_UNRESOLVED_NEXT_STEP: &str = "Typed static limitation (rust_value_propagation_unresolved): ripr cannot statically resolve the boundary operand values through the producer operations, so it does not prescribe a specific boundary input the suite may already cover. Verify via real mutation testing whether the asserted sink observes the missing discriminator value.";
/// #1579: repair assignment for a changed error return behind a
/// boolean guard the analyzer cannot carry the producing expression
/// through. It must not prescribe a boundary or error-assertion test
/// the suite already contains; the typed limitation names the guard
/// edge static analysis cannot establish, deferring the
/// discrimination verdict to real mutation testing.
pub(in crate::analysis) const ERROR_RETURN_GUARD_UNRESOLVED_NEXT_STEP: &str = "Typed static limitation (rust_value_propagation_unresolved): ripr cannot statically resolve the changed error return's producing expression through the boolean guard, so it does not prescribe a boundary or error-assertion test the suite may already contain. Verify via real mutation testing whether the exact observer discriminates the producing expression.";
use crate::analysis::rust_index::TestSummary;
use crate::domain::*;

pub(in crate::analysis) fn build_finding(
    context: &ProbeContext<'_>,
    class: ExposureClass,
    evidence: ClassifiedProbeEvidence,
) -> Finding {
    let missing = missing_evidence(
        context.probe,
        &class,
        &evidence.infect,
        &evidence.observe,
        &evidence.discriminate,
        &evidence.activation,
    );
    let test_summaries = context.related_test_summaries();
    let mut stop_reasons = stop_reasons(context.probe, context.owner_fn, &test_summaries);
    ensure_unknown_stop_reason(&class, &mut stop_reasons);
    let exact_oracle_covers_direct_sink = evidence.discriminate.state == StageState::Yes
        && matches!(
            context.probe.family,
            ProbeFamily::ReturnValue | ProbeFamily::ErrorPath | ProbeFamily::FieldConstruction
        )
        && exact_oracle_aligns_with_sink(
            context.probe,
            context.owner_fn.map(|owner| owner.name.as_str()),
            &context.related_tests,
            &evidence,
        );
    // #1429: a predicate boundary discriminator whose operand values the
    // bounded evaluator cannot resolve (e.g. `rfind(?)/len_utf8`
    // producers) is unconfirmed, not established-missing. Prescribing a
    // specific boundary input would instruct the user to add a test the
    // suite may already contain, so the typed static limitation replaces
    // the prescription. The class stays `WeaklyExposed`: the gap is
    // still visible, only the impossible repair assignment is withheld.
    let boundary_operand_unresolved = class == ExposureClass::WeaklyExposed
        && matches!(context.probe.family, ProbeFamily::Predicate)
        && evidence
            .activation
            .missing_discriminators
            .iter()
            .any(|fact| fact.reason.contains(BOUNDARY_OPERAND_UNRESOLVED_MARKER));
    // #1579: a bare-guard predicate over a changed error return an
    // exact observer already covers is unconfirmed, not
    // established-missing. Prescribing a boundary or error-assertion
    // test would instruct the user to add a test the suite already
    // contains, so the typed static limitation replaces the
    // prescription. The class stays `InfectionUnknown`: the gap is
    // still visible, only the impossible repair assignment is withheld.
    let error_guard_unresolved: Option<String> = if class == ExposureClass::InfectionUnknown
        && matches!(context.probe.family, ProbeFamily::Predicate)
    {
        unresolved_guard_error_edge(
            context.probe,
            context.owner_fn,
            &test_summaries,
            &evidence.flow_sinks,
            context.helper_chain.as_ref(),
        )
        .filter(|_| {
            strong_assertion_observes_owner_result(
                &context.related_tests,
                context.owner_fn.map(|owner| owner.name.as_str()),
            )
        })
    } else {
        None
    };
    let recommended_next_step =
        if class == ExposureClass::WeaklyExposed && exact_oracle_covers_direct_sink {
            None
        } else if boundary_operand_unresolved {
            Some(BOUNDARY_OPERAND_UNRESOLVED_NEXT_STEP.to_string())
        } else if error_guard_unresolved.is_some() {
            Some(ERROR_RETURN_GUARD_UNRESOLVED_NEXT_STEP.to_string())
        } else {
            recommended_next_step(context.probe, &class, context.owner_assertion_shaped)
        };
    let confidence = evidence.confidence(&class);
    let invalid_propagation_witness = evidence.propagation_witness().is_some_and(|diagnostic| {
        diagnostic.is_invalid() || !diagnostic.witness().digest_matches()
    });
    let mut evidence_lines = evidence.evidence;
    if class == ExposureClass::WeaklyExposed && exact_oracle_covers_direct_sink {
        evidence_lines.push(
            "static limitation: exact oracle established; no assertion repair is indicated"
                .to_string(),
        );
    }
    if boundary_operand_unresolved {
        evidence_lines.push(
            "Typed static limitation (rust_value_propagation_unresolved): boundary operand values are not statically resolvable, so the missing discriminator is unconfirmed and no specific boundary input is prescribed"
                .to_string(),
        );
    }
    if let Some(reason) = &error_guard_unresolved {
        evidence_lines.push(format!(
            "Typed static limitation (rust_value_propagation_unresolved): {reason}, so the missing discriminator is unconfirmed and no boundary or error-assertion test is prescribed"
        ));
    }
    if invalid_propagation_witness {
        evidence_lines
            .push("propagation witness digest invalid; diagnostic witness withheld".to_string());
    }
    // RIPR-SPEC-0133: disclose the oracle reframe so JSON/human projections
    // show why the guidance is phrased for an assertion helper.
    if context.owner_assertion_shaped {
        evidence_lines.push(format!(
            "owner_shape: assertion_shaped ({})",
            crate::domain::ASSERTION_SHAPED_OWNER_REASON
        ));
    }

    Finding {
        id: context.probe.id.0.clone(),
        canonical_gap: None,
        probe: context.probe.clone(),
        class,
        ripr: evidence.ripr,
        confidence,
        evidence: evidence_lines,
        missing,
        flow_sinks: evidence.flow_sinks,
        activation: evidence.activation,
        stop_reasons,
        related_tests: evidence.related_tests,
        recommended_next_step,
        // Language metadata is populated by the per-language adapter
        // (e.g. `analysis::language::RustAdapter::analyze_diff`) after
        // classification. The classifier itself stays language-neutral.
        language: None,
        language_status: None,
        owner_kind: None,
        // #1429: carry the #3271 named limitation so CLI, ledger, gate,
        // LSP, and packet projections consume the same limitation fact.
        // The language adapter only assigns a limit when none is set,
        // so this producer-assigned kind is never overwritten.
        static_limit_kind: (boundary_operand_unresolved || error_guard_unresolved.is_some())
            .then_some(StaticLimitKind::RustValuePropagationUnresolved),
        changed_sink: None,
        observed_sink: None,
        oracle_alignment: None,
        alignment_reason: None,
        // Source currentness is resolved by the producer that observed the diff
        // evidence; this constructor has none, so the disposition stays the
        // explicit unknown (#3280).
        source_currentness: crate::domain::SourceCurrentness::UnresolvedSubject,
    }
}

/// True when a **single** related-test assertion is simultaneously a strong
/// exact oracle and aligned with the changed sink's identity.
///
/// Strength and alignment must come from the same assertion: an unaligned
/// strong oracle in one test must not be combined with a token-matching weak
/// assertion in another to suppress assertion-repair guidance. Token overlap
/// alone is also not sink identity — the asserted value must be bound to the
/// changed owner (the assertion calls the owner, or the test binds the
/// asserted receiver from an owner call), so a coincident `Ok(amount)` on an
/// unrelated receiver cannot claim an exact oracle. When identity cannot be
/// established the check fails closed and repair guidance is retained.
fn exact_oracle_aligns_with_sink(
    probe: &Probe,
    owner_name: Option<&str>,
    related_tests: &[(&TestSummary, RelationReason)],
    evidence: &ClassifiedProbeEvidence,
) -> bool {
    let Some(sink) = evidence
        .flow_sinks
        .iter()
        .find(|sink| sink_kind_corresponds(&probe.family, &sink.kind))
    else {
        return false;
    };
    evidence.related_tests.iter().any(|related| {
        let Some(oracle) = related.oracle.as_deref() else {
            return false;
        };
        if related.oracle_strength != OracleStrength::Strong {
            return false;
        }
        let Some((test, _)) = related_tests
            .iter()
            .find(|(summary, _)| summary.name == related.name)
        else {
            // Without the test body the asserted value cannot be bound to the
            // changed owner; fail closed instead of crediting token overlap.
            return false;
        };
        oracle_text_aligns_with_sink(
            &probe.expression,
            &sink.kind,
            &sink.text,
            oracle,
            owner_name,
            test.body.as_str(),
        )
    })
}

/// #1726 review: the guard producer accepts a strong assertion in any
/// owner-calling test, so a strong assertion on an unrelated value
/// could take the limitation. The consumer additionally requires the
/// strong assertion to mention an identifier bound from an owner call
/// (the same owner-bound authority `oracle_binds_sink_identity`
/// applies to value sinks), so only an observer of the owner result
/// withholds the prescription. A directly asserted owner call with no
/// `let` binding stays fail-closed and keeps its repair.
fn strong_assertion_observes_owner_result(
    related_tests: &[(&TestSummary, RelationReason)],
    owner_name: Option<&str>,
) -> bool {
    let Some(owner_name) = owner_name else {
        return false;
    };
    related_tests.iter().any(|(test, _)| {
        let bound = bound_identifiers_from_owner_calls(&test.body, owner_name);
        test.assertions.iter().any(|assertion| {
            assertion.strength == OracleStrength::Strong
                && bound
                    .iter()
                    .any(|name| contains_identifier(&assertion.text, name))
        })
    })
}

fn sink_kind_corresponds(family: &ProbeFamily, kind: &FlowSinkKind) -> bool {
    matches!(
        (family, kind),
        (ProbeFamily::ReturnValue, FlowSinkKind::ReturnValue)
            // A return-value companion probe on an `Err(...)` change observes
            // the error-variant sink, not a plain return value; alignment must
            // pin that variant exactly (RIPR-SPEC-0106).
            | (ProbeFamily::ReturnValue, FlowSinkKind::ErrorVariant)
            | (ProbeFamily::ErrorPath, FlowSinkKind::ErrorVariant)
            | (ProbeFamily::FieldConstruction, FlowSinkKind::StructField)
    )
}

fn oracle_text_aligns_with_sink(
    probe_expression: &str,
    sink_kind: &FlowSinkKind,
    sink_text: &str,
    oracle: &str,
    owner_name: Option<&str>,
    test_body: &str,
) -> bool {
    match sink_kind {
        // Error-variant sinks are aligned when the assertion pins the exact
        // error variant constructed by the changed expression: the variant
        // path is the sink identity, so a shared enum qualifier token alone
        // cannot align.
        FlowSinkKind::ErrorVariant => exact_error_variant(probe_expression)
            .is_some_and(|variant| contains_token_sequence(oracle, &qualified_tokens(&variant))),
        // Value and field sinks have no self-identifying path inside the
        // oracle: token overlap alone accepts `assert_eq!(other, Ok(amount))`
        // for an unrelated `other`. Require the asserted value to be bound to
        // the changed owner before crediting alignment.
        FlowSinkKind::ReturnValue | FlowSinkKind::StructField => {
            let sink_tokens = semantic_tokens(sink_text);
            // A sink with no identifier tokens (a bare literal such as "0")
            // asserts nothing about the changed value; the vacuous `.all`
            // must not credit alignment on an owner-bound literal.
            !sink_tokens.is_empty()
                && sink_tokens
                    .iter()
                    .all(|token| contains_identifier(oracle, token))
                && oracle_binds_sink_identity(oracle, test_body, owner_name)
        }
        _ => false,
    }
}

/// Bounded sink-identity binding for value and field sinks.
///
/// A value assertion observes the changed sink only when the asserted value is
/// bound to the changed owner's result. Two bounded signals count:
/// - the assertion text itself calls the owner
///   (`assert_eq!(calculate(5), ...)`); or
/// - the test body binds the asserted identifier from an owner call
///   (`let result = calculate(5); ... assert_eq!(result, ...)`).
///
/// The check fails closed: without an owner name, without a test body, or
/// without such a binding, alignment is not credited and the repair guidance
/// is retained (RIPR-SPEC-0001; AGENTS.md: align on entity identity, not
/// token coincidence).
fn oracle_binds_sink_identity(oracle: &str, test_body: &str, owner_name: Option<&str>) -> bool {
    let Some(owner_name) = owner_name else {
        return false;
    };
    if text_calls_owner(oracle, owner_name) {
        return true;
    }
    bound_identifiers_from_owner_calls(test_body, owner_name)
        .iter()
        .any(|name| contains_identifier(oracle, name))
}

/// True when `text` contains `owner_name` immediately followed by `(` with a
/// non-identifier character before it. Delegates to the shared
/// `body_contains_owner_call` boundary authority so oracle text and test
/// bodies apply the same owner-call rule.
fn text_calls_owner(text: &str, owner_name: &str) -> bool {
    body_contains_owner_call(text, owner_name)
}

/// Identifiers bound by `let <ident> = ...` statements in `body` whose
/// initializer calls `owner_name`. Bounded text scan: each binding's
/// initializer extends to the next `;`. Uppercase pattern names
/// (`let Some(x) = ...`) and tuple destructuring are skipped — they cannot
/// name the bound variable, so the binding fails closed.
fn bound_identifiers_from_owner_calls(body: &str, owner_name: &str) -> Vec<String> {
    fn is_ident_byte(byte: u8) -> bool {
        byte.is_ascii_alphanumeric() || byte == b'_'
    }
    let bytes = body.as_bytes();
    let mut bound = Vec::new();
    let mut cursor = 0;
    while let Some(offset) = body[cursor..].find("let ") {
        let start = cursor + offset;
        cursor = start + "let ".len();
        if start > 0 && is_ident_byte(bytes[start - 1]) {
            continue;
        }
        let mut name_end = cursor;
        while name_end < bytes.len() && is_ident_byte(bytes[name_end]) {
            name_end += 1;
        }
        let mut name = &body[cursor..name_end];
        if matches!(name, "mut" | "ref") {
            let mut pattern_start = name_end;
            while pattern_start < bytes.len() && bytes[pattern_start].is_ascii_whitespace() {
                pattern_start += 1;
            }
            name_end = pattern_start;
            while name_end < bytes.len() && is_ident_byte(bytes[name_end]) {
                name_end += 1;
            }
            name = &body[pattern_start..name_end];
        }
        if name.is_empty()
            || name
                .chars()
                .next()
                .is_some_and(|character| character.is_ascii_uppercase())
        {
            continue;
        }
        let Some(assign) = body[name_end..].find('=') else {
            continue;
        };
        let Some(terminator) = body[name_end..].find(';') else {
            continue;
        };
        if assign >= terminator {
            continue;
        }
        let initializer = &body[name_end + assign + 1..name_end + terminator];
        if text_calls_owner(initializer, owner_name) {
            bound.push(name.to_string());
        }
        cursor = name_end.max(cursor);
    }
    bound
}

fn semantic_tokens(text: &str) -> Vec<String> {
    text.split(|character: char| !character.is_ascii_alphanumeric() && character != '_')
        .filter(|token| !token.is_empty())
        .filter(|token| {
            token
                .chars()
                .any(|character| character.is_ascii_alphabetic())
        })
        .map(str::to_string)
        .collect()
}

fn contains_identifier(text: &str, token: &str) -> bool {
    text.split(|character: char| !character.is_ascii_alphanumeric() && character != '_')
        .any(|candidate| candidate == token)
}

fn qualified_tokens(text: &str) -> Vec<String> {
    text.split(|character: char| !character.is_ascii_alphanumeric() && character != '_')
        .filter(|token| !token.is_empty())
        .map(str::to_string)
        .collect()
}

fn contains_token_sequence(text: &str, expected: &[String]) -> bool {
    if expected.is_empty() {
        return false;
    }
    let actual = qualified_tokens(text);
    actual
        .windows(expected.len())
        .any(|window| window == expected)
}

#[cfg(test)]
mod tests {
    use super::{
        bound_identifiers_from_owner_calls, exact_oracle_aligns_with_sink,
        oracle_binds_sink_identity, oracle_text_aligns_with_sink, sink_kind_corresponds,
        strong_assertion_observes_owner_result,
    };
    use crate::analysis::classifier::evidence::ClassifiedProbeEvidence;
    use crate::analysis::rust_index::TestSummary;
    use crate::domain::{
        ActivationEvidence, Confidence, DeltaKind, FlowSinkFact, FlowSinkKind, OracleKind,
        OracleStrength, Probe, ProbeFamily, ProbeId, RelatedTest, RelationReason, RevealEvidence,
        RiprEvidence, SourceLocation, StageEvidence, StageState, SymbolId,
    };
    use std::path::PathBuf;

    fn probe(family: ProbeFamily, expression: &str) -> Probe {
        Probe {
            id: ProbeId("probe:test".to_string()),
            location: SourceLocation::new("src/lib.rs", 12, 2),
            owner: Some(SymbolId("owner:calculate".to_string())),
            family,
            delta: DeltaKind::Value,
            before: Some(expression.to_string()),
            after: Some(expression.to_string()),
            expression: expression.to_string(),
            expected_sinks: Vec::new(),
            required_oracles: Vec::new(),
        }
    }

    fn sink(kind: FlowSinkKind, text: &str) -> FlowSinkFact {
        FlowSinkFact {
            kind,
            text: text.to_string(),
            line: 12,
            owner: Some(SymbolId("owner:calculate".to_string())),
        }
    }

    fn related(name: &str, oracle: &str, strength: OracleStrength) -> RelatedTest {
        RelatedTest {
            name: name.to_string(),
            file: PathBuf::from("tests/errors.rs"),
            line: 4,
            oracle: Some(oracle.to_string()),
            oracle_kind: OracleKind::ExactValue,
            oracle_strength: strength,
            relation_reason: Some(RelationReason::DirectOwnerCall),
            relation_confidence: Some(crate::domain::RelationConfidence::High),
        }
    }

    fn evidence(
        sinks: Vec<FlowSinkFact>,
        related_tests: Vec<RelatedTest>,
    ) -> ClassifiedProbeEvidence {
        let yes = StageEvidence::new(StageState::Yes, Confidence::Medium, "stage");
        ClassifiedProbeEvidence {
            ripr: RiprEvidence {
                reach: yes.clone(),
                infect: yes.clone(),
                propagate: StageEvidence::new(
                    StageState::Weak,
                    Confidence::Low,
                    "propagation weak",
                ),
                reveal: RevealEvidence {
                    observe: yes.clone(),
                    discriminate: yes.clone(),
                },
            },
            evidence: Vec::new(),
            flow_sinks: sinks,
            propagation_witness: None,
            activation: ActivationEvidence::default(),
            related_tests,
            reach: yes.clone(),
            infect: yes.clone(),
            propagate: StageEvidence::new(StageState::Weak, Confidence::Low, "propagation weak"),
            observe: yes.clone(),
            discriminate: yes,
        }
    }

    fn test_summary(name: &str, body: &str) -> TestSummary {
        TestSummary {
            name: name.to_string(),
            file: PathBuf::from("tests/errors.rs"),
            start_line: 1,
            end_line: 6,
            body: body.to_string(),
            calls: Vec::new(),
            assertions: Vec::new(),
            literals: Vec::new(),
            attrs: Vec::new(),
            nested_fn_names: Vec::new(),
            let_bindings: Vec::new(),
        }
    }

    fn alignment(
        family: ProbeFamily,
        expression: &str,
        sinks: Vec<FlowSinkFact>,
        related_tests: Vec<RelatedTest>,
        summaries: &[TestSummary],
    ) -> bool {
        let paired = summaries
            .iter()
            .map(|summary| (summary, RelationReason::DirectOwnerCall))
            .collect::<Vec<_>>();
        exact_oracle_aligns_with_sink(
            &probe(family, expression),
            Some("calculate"),
            &paired,
            &evidence(sinks, related_tests),
        )
    }

    #[test]
    fn family_selects_corresponding_sink_and_rejects_siblings() {
        assert!(sink_kind_corresponds(
            &ProbeFamily::ReturnValue,
            &FlowSinkKind::ReturnValue
        ));
        // WCtF: a return-value companion probe on an `Err(...)` change observes
        // the error-variant sink.
        assert!(sink_kind_corresponds(
            &ProbeFamily::ReturnValue,
            &FlowSinkKind::ErrorVariant
        ));
        assert!(sink_kind_corresponds(
            &ProbeFamily::ErrorPath,
            &FlowSinkKind::ErrorVariant
        ));
        assert!(sink_kind_corresponds(
            &ProbeFamily::FieldConstruction,
            &FlowSinkKind::StructField
        ));
        assert!(!sink_kind_corresponds(
            &ProbeFamily::FieldConstruction,
            &FlowSinkKind::ReturnValue
        ));
        assert!(!sink_kind_corresponds(
            &ProbeFamily::ReturnValue,
            &FlowSinkKind::StructField
        ));
    }

    #[test]
    fn error_variant_sink_accepts_return_value_probe_with_pinned_variant() {
        // WCtF: the companion return-value probe of an `Err(...)` change must
        // credit an exact-variant oracle instead of demanding a "broader"
        // assertion.
        assert!(alignment(
            ProbeFamily::ReturnValue,
            "return Err(ParseError::TooLong(name.len()))",
            vec![sink(
                FlowSinkKind::ErrorVariant,
                "Result::Err(ParseError::TooLong)"
            )],
            vec![related(
                "too_long_rejected",
                "assert_eq!(err, ParseError::TooLong(12));",
                OracleStrength::Strong,
            )],
            &[test_summary(
                "too_long_rejected",
                "let err = validate(\"aaaaaaaaaaaa\").unwrap_err();"
            )],
        ));
    }

    #[test]
    fn direct_value_and_field_alignment_rejects_embedded_tokens() {
        // An owner-bound assertion aligns the direct value sink.
        assert!(alignment(
            ProbeFamily::ReturnValue,
            "amount",
            vec![sink(FlowSinkKind::ReturnValue, "amount")],
            vec![related(
                "amount_pinned",
                "assert_eq!(amount, 3)",
                OracleStrength::Strong,
            )],
            &[test_summary("amount_pinned", "let amount = calculate(5);")],
        ));
        // An owner-bound field assertion aligns the field sink.
        assert!(alignment(
            ProbeFamily::FieldConstruction,
            "status: amount,",
            vec![sink(FlowSinkKind::StructField, "status: amount")],
            vec![related(
                "status_pinned",
                "assert_eq!(cfg.status, amount)",
                OracleStrength::Strong,
            )],
            &[test_summary("status_pinned", "let cfg = calculate(5);")],
        ));
        // Near-name tokens never align.
        assert!(!alignment(
            ProbeFamily::ReturnValue,
            "amount",
            vec![sink(FlowSinkKind::ReturnValue, "amount")],
            vec![related(
                "amount_total",
                "assert_eq!(amount_total, 3)",
                OracleStrength::Strong,
            )],
            &[test_summary("amount_total", "let amount = calculate(5);")],
        ));
        assert!(!alignment(
            ProbeFamily::FieldConstruction,
            "status: amount,",
            vec![sink(FlowSinkKind::StructField, "status: amount")],
            vec![related(
                "status_code",
                "assert_eq!(cfg.status_code, amount)",
                OracleStrength::Strong,
            )],
            &[test_summary("status_code", "let cfg = calculate(5);")],
        ));
    }

    #[test]
    fn token_only_alignment_without_sink_identity_fails_closed() {
        // WCtH: `other` is a different value than the wrapped `Ok(amount)`
        // sink; the test body binds `other` from an unrelated source, so the
        // token overlap must not suppress repair guidance.
        assert!(!alignment(
            ProbeFamily::ReturnValue,
            "amount",
            vec![sink(FlowSinkKind::ReturnValue, "Ok(amount)")],
            vec![related(
                "wrapped_compare",
                "assert_eq!(other, Ok(amount))",
                OracleStrength::Strong,
            )],
            &[test_summary(
                "wrapped_compare",
                "let other = other_producer(); assert_eq!(other, Ok(amount));"
            )],
        ));
        // XQFW: `other.status` is not the constructed sink `status: amount`
        // unless `other` is bound from the changed owner.
        assert!(!alignment(
            ProbeFamily::FieldConstruction,
            "status: amount,",
            vec![sink(FlowSinkKind::StructField, "status: amount")],
            vec![related(
                "field_compare",
                "assert_eq!(other.status, amount)",
                OracleStrength::Strong,
            )],
            &[test_summary(
                "field_compare",
                "let other = unrelated_builder();"
            )],
        ));
    }

    #[test]
    fn owner_bound_assertions_credit_sink_identity() {
        // The test body binds the asserted receiver from the owner call.
        assert!(alignment(
            ProbeFamily::ReturnValue,
            "amount",
            vec![sink(FlowSinkKind::ReturnValue, "Ok(amount)")],
            vec![related(
                "wrapped_compare",
                "assert_eq!(other, Ok(amount))",
                OracleStrength::Strong,
            )],
            &[test_summary(
                "wrapped_compare",
                "let other = calculate(5); assert_eq!(other, Ok(amount));"
            )],
        ));
        // The assertion itself calls the owner.
        assert!(alignment(
            ProbeFamily::ReturnValue,
            "amount",
            vec![sink(FlowSinkKind::ReturnValue, "Ok(amount)")],
            vec![related(
                "inline_call",
                "assert_eq!(calculate(5), Ok(amount))",
                OracleStrength::Strong,
            )],
            &[test_summary("inline_call", "let seed = 5;")],
        ));
    }

    #[test]
    fn mixed_strength_oracles_require_one_aligned_exact_assertion() {
        // XNFm: a strong-but-unaligned assertion in one test must not be
        // combined with a weak assertion that happens to contain every sink
        // token in another; the repair guidance stays.
        let shared_sinks = vec![sink(FlowSinkKind::ReturnValue, "Ok(amount)")];
        let summaries = [test_summary("weak_token_match", "let seed = calculate(5);")];
        let mixed = vec![
            related(
                "strong_unaligned",
                "assert_eq!(score, 42);",
                OracleStrength::Strong,
            ),
            related(
                "weak_token_match",
                "assert_eq!(result, Ok(amount));",
                OracleStrength::Weak,
            ),
        ];
        assert!(!alignment(
            ProbeFamily::ReturnValue,
            "amount",
            shared_sinks.clone(),
            mixed,
            &summaries,
        ));
        // The same weak token-matcher plus an aligned strong assertion keeps
        // the suppression: one assertion carries both strength and alignment.
        let aligned_strong = vec![
            related(
                "weak_token_match",
                "assert_eq!(result, Ok(amount));",
                OracleStrength::Weak,
            ),
            related(
                "aligned_strong",
                "assert_eq!(other, Ok(amount))",
                OracleStrength::Strong,
            ),
        ];
        let bound_summaries = [
            test_summary("weak_token_match", "let seed = calculate(5);"),
            test_summary("aligned_strong", "let other = calculate(5);"),
        ];
        assert!(alignment(
            ProbeFamily::ReturnValue,
            "amount",
            shared_sinks,
            aligned_strong,
            &bound_summaries,
        ));
    }

    #[test]
    fn alignment_fails_closed_without_test_body_or_owner() {
        // No matching test summary (body unavailable) → fail closed.
        assert!(!alignment(
            ProbeFamily::ReturnValue,
            "amount",
            vec![sink(FlowSinkKind::ReturnValue, "amount")],
            vec![related(
                "amount_pinned",
                "assert_eq!(amount, 3)",
                OracleStrength::Strong,
            )],
            &[test_summary("another_test", "let amount = calculate(5);")],
        ));
        // Token rule without an owner name cannot bind identity.
        assert!(!oracle_text_aligns_with_sink(
            "amount",
            &FlowSinkKind::ReturnValue,
            "amount",
            "assert_eq!(amount, 3)",
            None,
            "let amount = calculate(5);"
        ));
    }

    #[test]
    fn owner_binding_scan_handles_mut_and_fails_closed_on_patterns() {
        assert_eq!(
            bound_identifiers_from_owner_calls(
                "let mut result = calculate(5); let other = helper();",
                "calculate"
            ),
            vec!["result".to_string()]
        );
        // `let Some(x) = calculate()` is a pattern: the bound name is not
        // textually derivable, so nothing is credited.
        assert!(
            bound_identifiers_from_owner_calls("let Some(x) = calculate();", "calculate")
                .is_empty()
        );
        // A longer identifier ending in the owner name is not an owner call.
        assert!(!oracle_binds_sink_identity(
            "assert_eq!(other, Ok(amount))",
            "let other = recalculate(5);",
            Some("calculate")
        ));
    }

    // #1726 review T2: the guard limitation needs a strong assertion
    // observing an owner-bound result, not just any strong assertion
    // in an owner-calling test.
    #[test]
    fn owner_result_observation_requires_bound_strong_assertion() {
        use crate::analysis::rust_index::OracleFact;

        fn summary(body: &str, assertions: Vec<OracleFact>) -> TestSummary {
            TestSummary {
                name: "case".to_string(),
                file: PathBuf::from("tests/errors.rs"),
                start_line: 1,
                end_line: 6,
                body: body.to_string(),
                calls: Vec::new(),
                assertions,
                literals: Vec::new(),
                attrs: Vec::new(),
                nested_fn_names: Vec::new(),
                let_bindings: Vec::new(),
            }
        }

        fn strong(text: &str) -> OracleFact {
            OracleFact {
                kind: OracleKind::ExactValue,
                strength: OracleStrength::Strong,
                line: 2,
                text: text.to_string(),
                observed_tokens: Vec::new(),
                ok_value_observed: None,
            }
        }

        fn paired(test: &TestSummary) -> Vec<(&TestSummary, RelationReason)> {
            vec![(test, RelationReason::DirectOwnerCall)]
        }

        // A strong assertion on the owner-bound result observes it.
        let bound = summary(
            "let err = calculate(true).expect_err(\"x\");",
            vec![strong("assert_eq!(err, cancelled_error());")],
        );
        assert!(strong_assertion_observes_owner_result(
            &paired(&bound),
            Some("calculate")
        ));
        // A strong assertion on an unrelated value does not, even
        // when the test calls the owner.
        let unbound = summary(
            "calculate(true);\nassert_eq!(other, 42);",
            vec![strong("assert_eq!(other, 42);")],
        );
        assert!(!strong_assertion_observes_owner_result(
            &paired(&unbound),
            Some("calculate")
        ));
        // A weak assertion on the bound result does not qualify.
        let mut weak_assertion = strong("assert_eq!(err, cancelled_error());");
        weak_assertion.strength = OracleStrength::Weak;
        let weak = summary(
            "let err = calculate(true).expect_err(\"x\");",
            vec![weak_assertion],
        );
        assert!(!strong_assertion_observes_owner_result(
            &paired(&weak),
            Some("calculate")
        ));
        // No owner means no binding to observe through.
        assert!(!strong_assertion_observes_owner_result(
            &paired(&bound),
            None
        ));
    }

    #[test]
    fn error_alignment_rejects_near_name_variant_tokens() {
        assert!(oracle_text_aligns_with_sink(
            "return Err(CalcError::TooLarge);",
            &FlowSinkKind::ErrorVariant,
            "Result::Err(CalcError::TooLarge)",
            "assert_eq!(err, CalcError::TooLarge)",
            Some("calculate"),
            "let err = calculate(5).unwrap_err();",
        ));
        assert!(!oracle_text_aligns_with_sink(
            "return Err(CalcError::TooLarge);",
            &FlowSinkKind::ErrorVariant,
            "Result::Err(CalcError::TooLarge)",
            "assert_eq!(err, CalcError::TooLarger)",
            Some("calculate"),
            "let err = calculate(5).unwrap_err();",
        ));
        // A sibling variant of the same enum does not align an error sink.
        assert!(!oracle_text_aligns_with_sink(
            "return Err(CalcError::TooLarge);",
            &FlowSinkKind::ErrorVariant,
            "Result::Err(CalcError::TooLarge)",
            "assert_eq!(err, CalcError::Negative)",
            Some("calculate"),
            "let err = calculate(5).unwrap_err();",
        ));
    }
}
