//! Same-test pairing for boundary-class (predicate) probes.
//!
//! `exposed` requires one test that both feeds a boundary input to the owner
//! and holds a discriminating oracle on that call's result. Crediting a
//! boundary call from one test and an exact oracle from another is a false
//! `exposed` (#4828): `>=` → `>` still passes both tests.

use super::super::facts::CallFact;
use super::super::rust_index::{FunctionSummary, OracleFact, TestSummary, extract_literals};
use super::activation::{
    call_arguments, comparison_operands, function_parameters, owner_argument_values,
};
use super::text::delimited_contents_at;
use crate::domain::*;

/// Token carried in the discriminate summary when a predicate would otherwise
/// read `exposed` from a split: boundary input in one test, oracle in another.
pub(in crate::analysis) const SAME_TEST_PAIRING_MISSING: &str = "same_test_pairing_missing";

pub(in crate::analysis) fn same_test_pairing_missing_summary() -> String {
    format!(
        "Discriminator unconfirmed: boundary input and discriminating oracle come from different tests ({SAME_TEST_PAIRING_MISSING}); no test both feeds a boundary input to the owner and holds a discriminating oracle on that call's result"
    )
}

/// True when some related test both feeds a boundary input to the owner and
/// holds a discriminating oracle on that call's result.
///
/// Boundary credit is the activation authority's `==` facts (named constants,
/// helper hops, local bindings), not a second matcher. Call-argument matching
/// remains only for the literal/parameter shape those facts do not cover.
///
/// Non-predicate probes are not this gate; the caller must not use a `true`
/// result to promote a family this function does not judge.
pub(in crate::analysis) fn has_same_test_boundary_oracle_pairing(
    probe: &Probe,
    owner_fn: Option<&FunctionSummary>,
    related_tests: &[&TestSummary],
    activation: &ActivationEvidence,
) -> bool {
    if !matches!(probe.family, ProbeFamily::Predicate) {
        return false;
    }
    let Some(owner) = owner_fn else {
        return false;
    };
    related_tests
        .iter()
        .any(|test| test_pairs_boundary_input_with_oracle(probe, owner, test, activation))
}

fn test_pairs_boundary_input_with_oracle(
    probe: &Probe,
    owner: &FunctionSummary,
    test: &TestSummary,
    activation: &ActivationEvidence,
) -> bool {
    let bound_names = boundary_bound_locals(probe, owner, test, activation);
    test.assertions.iter().any(|assertion| {
        if !assertion_is_discriminating(assertion) {
            return false;
        }
        assertion_observes_boundary_owner_call(probe, owner, test, assertion, activation)
            || assertion_observes_bound_name(assertion, &bound_names)
    })
}

fn assertion_is_discriminating(assertion: &OracleFact) -> bool {
    matches!(assertion.strength, OracleStrength::Strong)
        || matches!(
            assertion.kind,
            OracleKind::ExactValue
                | OracleKind::WholeObjectEquality
                | OracleKind::ExactErrorVariant
        )
}

fn assertion_observes_boundary_owner_call(
    probe: &Probe,
    owner: &FunctionSummary,
    test: &TestSummary,
    assertion: &OracleFact,
    activation: &ActivationEvidence,
) -> bool {
    let subject = assertion_subject(&assertion.text);
    if owner_call_argument_lists(&subject, &owner.name)
        .iter()
        .any(|arguments| argument_list_activates_boundary(probe, owner, test, arguments))
    {
        return true;
    }
    // Line-level activation cannot tell two same-name calls apart. Use it
    // only when the assertion text names the owner once.
    owner_call_count(&assertion.text, &owner.name) == 1
        && activation_marks_boundary_call(
            activation,
            &CallFact {
                line: assertion.line,
                name: owner.name.clone(),
                text: assertion.text.clone(),
            },
        )
}

fn assertion_observes_bound_name(assertion: &OracleFact, bound_names: &[String]) -> bool {
    bound_names
        .iter()
        .any(|name| contains_ident(&assertion.text, name))
}

fn boundary_bound_locals(
    probe: &Probe,
    owner: &FunctionSummary,
    test: &TestSummary,
    activation: &ActivationEvidence,
) -> Vec<String> {
    // Last binding of a name wins so `let got = gate(10); let got = gate(100)`
    // does not keep the shadowed boundary result.
    let mut last: Vec<(String, bool)> = Vec::new();
    for (offset, line) in test.body.lines().enumerate() {
        let Some(name) = let_binding_name(line) else {
            continue;
        };
        let line_number = test.start_line + offset;
        let call = CallFact {
            line: line_number,
            name: owner.name.clone(),
            text: line.trim().to_string(),
        };
        let is_boundary = owner_call_activates_boundary(probe, owner, test, &call, activation);
        if let Some(existing) = last.iter_mut().find(|(bound, _)| bound == &name) {
            existing.1 = is_boundary;
        } else {
            last.push((name, is_boundary));
        }
    }
    last.into_iter()
        .filter_map(|(name, is_boundary)| is_boundary.then_some(name))
        .collect()
}

fn let_binding_name(line: &str) -> Option<String> {
    let trimmed = line.trim();
    let rest = trimmed.strip_prefix("let")?;
    if rest
        .chars()
        .next()
        .is_some_and(|ch| ch.is_ascii_alphanumeric() || ch == '_')
    {
        return None;
    }
    let rest = rest.trim_start();
    let rest = rest.strip_prefix("mut ").unwrap_or(rest).trim_start();
    let name: String = rest
        .chars()
        .take_while(|ch| ch.is_ascii_alphanumeric() || *ch == '_')
        .collect();
    if name.is_empty() || name == "_" || name.starts_with(|ch: char| ch.is_ascii_digit()) {
        return None;
    }
    let after = rest.get(name.len()..)?.trim_start();
    let (before_eq, _) = after.split_once('=')?;
    let before_eq = before_eq.trim();
    (before_eq.is_empty() || before_eq.starts_with(':')).then_some(name)
}

fn owner_call_activates_boundary(
    probe: &Probe,
    owner: &FunctionSummary,
    test: &TestSummary,
    call: &CallFact,
    activation: &ActivationEvidence,
) -> bool {
    if call.name != owner.name {
        return false;
    }
    if let Some(arguments) = call_arguments(&call.text, &call.name)
        && argument_list_activates_boundary(probe, owner, test, &arguments)
    {
        return true;
    }
    owner_call_count(&call.text, &owner.name) == 1
        && activation_marks_boundary_call(activation, call)
}

fn argument_list_activates_boundary(
    probe: &Probe,
    owner: &FunctionSummary,
    test: &TestSummary,
    arguments: &[String],
) -> bool {
    let arg_values: Vec<Vec<String>> = arguments
        .iter()
        .map(|argument| owner_argument_values(test, argument))
        .collect();
    let Some((left, right)) = comparison_operands(&probe.expression) else {
        return false;
    };
    let parameters = function_parameters(owner);
    let left_index = parameter_index(&parameters, &left);
    let right_index = parameter_index(&parameters, &right);
    if let (Some(left_index), Some(right_index)) = (left_index, right_index) {
        return values_overlap(
            arg_values.get(left_index).map(Vec::as_slice).unwrap_or(&[]),
            arg_values
                .get(right_index)
                .map(Vec::as_slice)
                .unwrap_or(&[]),
        );
    }
    if let Some(left_index) = left_index {
        let right_literals = extract_literals(&right);
        return values_overlap(
            arg_values.get(left_index).map(Vec::as_slice).unwrap_or(&[]),
            &right_literals,
        );
    }
    if let Some(right_index) = right_index {
        let left_literals = extract_literals(&left);
        return values_overlap(
            arg_values
                .get(right_index)
                .map(Vec::as_slice)
                .unwrap_or(&[]),
            &left_literals,
        );
    }
    false
}

fn assertion_subject(text: &str) -> String {
    for marker in ["assert_eq!(", "assert_ne!(", "assert_matches!(", "assert!("] {
        if let Some(index) = find_marker(text, marker)
            && let Some(open) = index.checked_add(marker.len().saturating_sub(1))
            && let Some(inner) = delimited_contents_at(text, open)
        {
            return inner;
        }
    }
    text.to_string()
}

fn owner_call_argument_lists(text: &str, name: &str) -> Vec<Vec<String>> {
    let needle = format!("{name}(");
    let mut lists = Vec::new();
    let mut from = 0usize;
    while from < text.len() {
        let Some(rel) = text.get(from..).and_then(|rest| rest.find(&needle)) else {
            break;
        };
        let abs = from + rel;
        if abs > 0 {
            let before = text.as_bytes()[abs - 1];
            if before.is_ascii_alphanumeric() || before == b'_' {
                from = abs + 1;
                continue;
            }
        }
        if let Some(arguments) = call_arguments(text.get(abs..).unwrap_or(""), name) {
            lists.push(arguments);
        }
        from = abs + needle.len();
    }
    lists
}

fn owner_call_count(text: &str, name: &str) -> usize {
    owner_call_argument_lists(text, name).len()
}

fn contains_ident(text: &str, name: &str) -> bool {
    find_ident_at(text, name).is_some()
}

fn find_ident_at(text: &str, name: &str) -> Option<usize> {
    if name.is_empty() {
        return None;
    }
    let mut from = 0usize;
    while from < text.len() {
        let rel = text.get(from..).and_then(|rest| rest.find(name))?;
        let abs = from + rel;
        let before_ok = abs == 0 || {
            let before = text.as_bytes()[abs - 1];
            !(before.is_ascii_alphanumeric() || before == b'_')
        };
        let end = abs + name.len();
        let after_ok = end >= text.len() || {
            let after = text.as_bytes()[end];
            !(after.is_ascii_alphanumeric() || after == b'_')
        };
        if before_ok && after_ok {
            return Some(abs);
        }
        from = abs + 1;
    }
    None
}

fn find_marker(text: &str, marker: &str) -> Option<usize> {
    let mut from = 0usize;
    while from < text.len() {
        let rel = text.get(from..).and_then(|rest| rest.find(marker))?;
        let abs = from + rel;
        let before_ok = abs == 0 || {
            let before = text.as_bytes()[abs - 1];
            !(before.is_ascii_alphanumeric() || before == b'_')
        };
        if before_ok {
            return Some(abs);
        }
        from = abs + 1;
    }
    None
}

fn parameter_index(parameters: &[String], operand: &str) -> Option<usize> {
    parameters.iter().position(|parameter| parameter == operand)
}

fn values_overlap(left: &[String], right: &[String]) -> bool {
    !left.is_empty() && !right.is_empty() && left.iter().any(|value| right.contains(value))
}

/// Infection already recorded that this owner-call line sits on the boundary
/// (`left == right` from named constants, helper hops, or local bindings).
fn activation_marks_boundary_call(activation: &ActivationEvidence, call: &CallFact) -> bool {
    activation.observed_values.iter().any(|fact| {
        fact.line == call.line
            && fact.value.contains(" == ")
            && (fact.text.is_empty() || fact.text.contains(&call.text))
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::facts::FunctionSourceRole;
    use crate::analysis::rust_index::{
        CallFact, LiteralFact, OracleFact, extract_identifier_tokens,
    };
    use std::path::PathBuf;

    #[test]
    fn split_tests_do_not_pair() {
        let probe = predicate_probe("input >= 10");
        let owner = gate_owner();
        let boundary = test_summary(
            "boundary",
            "let _ = gate(10); let _ = gate(9);",
            vec![
                call("gate", "let _ = gate(10);"),
                call("gate", "let _ = gate(9);"),
            ],
            vec![],
            &["10", "9"],
        );
        let far = test_summary(
            "far",
            "assert_eq!(gate(100), true);",
            vec![call("gate", "assert_eq!(gate(100), true);")],
            vec![exact("assert_eq!(gate(100), true);")],
            &["100"],
        );
        assert!(
            !has_same_test_boundary_oracle_pairing(
                &probe,
                Some(&owner),
                &[&boundary, &far],
                &ActivationEvidence::default(),
            ),
            "a no-oracle boundary call plus a far exact oracle must not pair"
        );
    }

    #[test]
    fn same_call_assert_eq_pairs() {
        let probe = predicate_probe("input >= 10");
        let owner = gate_owner();
        let paired = test_summary(
            "paired_boundary",
            "assert_eq!(gate(10), true);",
            vec![call("gate", "assert_eq!(gate(10), true);")],
            vec![exact("assert_eq!(gate(10), true);")],
            &["10"],
        );
        assert!(
            has_same_test_boundary_oracle_pairing(
                &probe,
                Some(&owner),
                &[&paired],
                &ActivationEvidence::default(),
            ),
            "assert_eq!(gate(10), true) must pair"
        );
    }

    #[test]
    fn same_test_split_calls_do_not_pair() {
        let probe = predicate_probe("input >= 10");
        let owner = gate_owner();
        let mixed = test_summary(
            "mixed",
            "let _ = gate(10);\nassert_eq!(gate(100), true);",
            vec![
                call("gate", "let _ = gate(10);"),
                call("gate", "assert_eq!(gate(100), true);"),
            ],
            vec![exact("assert_eq!(gate(100), true);")],
            &["10", "100"],
        );
        assert!(
            !has_same_test_boundary_oracle_pairing(
                &probe,
                Some(&owner),
                &[&mixed],
                &ActivationEvidence::default(),
            ),
            "boundary call and far oracle in the same test still need the oracle on the boundary call"
        );
    }

    #[test]
    fn same_line_split_calls_do_not_pair() {
        let probe = predicate_probe("input >= 10");
        let owner = gate_owner();
        let mixed = test_summary(
            "mixed",
            "let _ = gate(10); assert_eq!(gate(100), true);",
            vec![
                call("gate", "let _ = gate(10); assert_eq!(gate(100), true);"),
                call("gate", "let _ = gate(10); assert_eq!(gate(100), true);"),
            ],
            vec![exact("let _ = gate(10); assert_eq!(gate(100), true);")],
            &["10", "100"],
        );
        assert!(
            !has_same_test_boundary_oracle_pairing(
                &probe,
                Some(&owner),
                &[&mixed],
                &ActivationEvidence::default(),
            ),
            "a same-line unasserted boundary call plus a far exact oracle must not pair"
        );
    }

    #[test]
    fn irrelevant_argument_does_not_pair() {
        let probe = predicate_probe("input >= 10");
        let owner = FunctionSummary {
            id: SymbolId("src/lib.rs::gate".to_string()),
            name: "gate".to_string(),
            file: PathBuf::from("src/lib.rs"),
            start_line: 1,
            end_line: 3,
            body: "pub fn gate(input: u32, marker: u32) -> bool { input >= 10 }".to_string(),
            calls: vec![],
            returns: vec![],
            literals: vec![],
            source_role: FunctionSourceRole::Production,
            attrs: vec![],
            impl_attrs: Vec::new(),
            nested_fn_names: Vec::new(),
            let_bindings: Vec::new(),
            impl_context: Default::default(),
            item: Default::default(),
        };
        let far = test_summary(
            "far",
            "assert_eq!(gate(100, 10), true);",
            vec![call("gate", "assert_eq!(gate(100, 10), true);")],
            vec![exact("assert_eq!(gate(100, 10), true);")],
            &["100", "10"],
        );
        assert!(
            !has_same_test_boundary_oracle_pairing(
                &probe,
                Some(&owner),
                &[&far],
                &ActivationEvidence::default(),
            ),
            "a boundary literal in an unused argument must not pair"
        );
    }

    #[test]
    fn shadowed_boundary_binding_does_not_pair() {
        let probe = predicate_probe("input >= 10");
        let owner = gate_owner();
        let mut shadowed = test_summary(
            "shadowed",
            "let got = gate(10);\nlet got = gate(100);\nassert_eq!(got, true);",
            vec![
                call("gate", "let got = gate(10);"),
                call("gate", "let got = gate(100);"),
            ],
            vec![exact("assert_eq!(got, true);")],
            &["10", "100"],
        );
        shadowed.calls[0].line = 1;
        shadowed.calls[1].line = 2;
        shadowed.assertions[0].line = 3;
        shadowed.end_line = 4;
        assert!(
            !has_same_test_boundary_oracle_pairing(
                &probe,
                Some(&owner),
                &[&shadowed],
                &ActivationEvidence::default(),
            ),
            "asserting a binding that later shadows the boundary result must not pair"
        );
    }

    #[test]
    fn short_let_bound_name_pairs() {
        let probe = predicate_probe("input >= 10");
        let owner = gate_owner();
        let mut bound = test_summary(
            "bound",
            "let x = gate(10);\nassert_eq!(x, true);",
            vec![call("gate", "let x = gate(10);")],
            vec![exact("assert_eq!(x, true);")],
            &["10"],
        );
        bound.calls[0].line = 1;
        bound.assertions[0].line = 2;
        bound.end_line = 3;
        assert!(
            has_same_test_boundary_oracle_pairing(
                &probe,
                Some(&owner),
                &[&bound],
                &ActivationEvidence::default(),
            ),
            "let x = gate(10); assert_eq!(x, true) must pair"
        );
    }

    #[test]
    fn let_bound_boundary_call_asserted_later_pairs() {
        let probe = predicate_probe("input >= 10");
        let owner = gate_owner();
        let mut bound = test_summary(
            "bound",
            "let got = gate(10);\nassert_eq!(got, true);",
            vec![call("gate", "let got = gate(10);")],
            vec![exact("assert_eq!(got, true);")],
            &["10"],
        );
        bound.calls[0].line = 1;
        bound.assertions[0].line = 2;
        bound.end_line = 3;
        assert!(
            has_same_test_boundary_oracle_pairing(
                &probe,
                Some(&owner),
                &[&bound],
                &ActivationEvidence::default(),
            ),
            "let got = gate(10); assert_eq!(got, true) must pair"
        );
    }

    #[test]
    fn typed_let_bound_boundary_call_asserted_later_pairs() {
        let probe = predicate_probe("input >= 10");
        let owner = gate_owner();
        let mut bound = test_summary(
            "bound",
            "let got: bool = gate(10);\nassert_eq!(got, true);",
            vec![call("gate", "let got: bool = gate(10);")],
            vec![exact("assert_eq!(got, true);")],
            &["10"],
        );
        bound.calls[0].line = 1;
        bound.assertions[0].line = 2;
        bound.end_line = 3;
        assert!(
            has_same_test_boundary_oracle_pairing(
                &probe,
                Some(&owner),
                &[&bound],
                &ActivationEvidence::default(),
            ),
            "let got: bool = gate(10); assert_eq!(got, true) must pair"
        );
    }

    #[test]
    fn let_binding_name_requires_keyword_boundary_and_allows_type_ascription() {
        assert_eq!(
            let_binding_name("let got = gate(10);").as_deref(),
            Some("got")
        );
        assert_eq!(
            let_binding_name("let mut got = gate(10);").as_deref(),
            Some("got")
        );
        assert_eq!(
            let_binding_name("let got: bool = gate(10);").as_deref(),
            Some("got")
        );
        assert_eq!(
            let_binding_name("let mut got: bool = gate(10);").as_deref(),
            Some("got")
        );
        assert_eq!(let_binding_name("letter = gate(10);"), None);
        assert_eq!(let_binding_name("let_got = gate(10);"), None);
        assert_eq!(let_binding_name("let _ = gate(10);"), None);
    }

    #[test]
    fn two_parameter_equal_args_in_assert_eq_pair() {
        let probe = predicate_probe("amount >= discount_threshold");
        let owner = FunctionSummary {
            id: SymbolId("src/lib.rs::discounted_total".to_string()),
            name: "discounted_total".to_string(),
            file: PathBuf::from("src/lib.rs"),
            start_line: 1,
            end_line: 6,
            body: "pub fn discounted_total(amount: i32, discount_threshold: i32) -> i32 { amount }"
                .to_string(),
            calls: vec![],
            returns: vec![],
            literals: vec![],
            source_role: FunctionSourceRole::Production,
            attrs: vec![],
            impl_attrs: Vec::new(),
            nested_fn_names: Vec::new(),
            let_bindings: Vec::new(),
            impl_context: Default::default(),
            item: Default::default(),
        };
        let paired = test_summary(
            "equality_threshold_discounts",
            "assert_eq!(discounted_total(100, 100), 90);",
            vec![call(
                "discounted_total",
                "assert_eq!(discounted_total(100, 100), 90);",
            )],
            vec![exact("assert_eq!(discounted_total(100, 100), 90);")],
            &["100", "90"],
        );
        assert!(
            has_same_test_boundary_oracle_pairing(
                &probe,
                Some(&owner),
                &[&paired],
                &ActivationEvidence::default(),
            ),
            "equal parameter arguments in the asserted owner call must pair"
        );
    }

    #[test]
    fn weak_assertion_on_boundary_call_does_not_pair() {
        let probe = predicate_probe("input >= 10");
        let owner = gate_owner();
        let weak = test_summary(
            "smoke",
            "assert!(gate(10));",
            vec![call("gate", "assert!(gate(10));")],
            vec![OracleFact {
                line: 1,
                text: "assert!(gate(10));".to_string(),
                kind: OracleKind::RelationalCheck,
                strength: OracleStrength::Weak,
                observed_tokens: extract_identifier_tokens("assert!(gate(10));"),
                ok_value_observed: None,
            }],
            &["10"],
        );
        assert!(
            !has_same_test_boundary_oracle_pairing(
                &probe,
                Some(&owner),
                &[&weak],
                &ActivationEvidence::default(),
            ),
            "a weak oracle on the boundary call is not a discriminating pairing"
        );
    }

    #[test]
    fn activation_equality_fact_pairs_when_call_args_are_not_probe_literals() {
        let probe = predicate_probe("final_label == \"alpha\"");
        let owner = FunctionSummary {
            id: SymbolId("src/lib.rs::classify".to_string()),
            name: "classify".to_string(),
            file: PathBuf::from("src/lib.rs"),
            start_line: 9,
            end_line: 16,
            body: "pub fn classify(input: &str) -> &'static str { input }".to_string(),
            calls: vec![],
            returns: vec![],
            literals: vec![],
            source_role: FunctionSourceRole::Production,
            attrs: vec![],
            impl_attrs: Vec::new(),
            nested_fn_names: Vec::new(),
            let_bindings: Vec::new(),
            impl_context: Default::default(),
            item: Default::default(),
        };
        let paired = test_summary(
            "word_label_is_word",
            "assert_eq!(classify(\"word\"), \"word\");",
            vec![call(
                "classify",
                "assert_eq!(classify(\"word\"), \"word\");",
            )],
            vec![exact("assert_eq!(classify(\"word\"), \"word\");")],
            &["word"],
        );
        let activation = ActivationEvidence {
            observed_values: vec![ValueFact {
                line: 1,
                text: "assert_eq!(classify(\"word\"), \"word\"); | helper hop".to_string(),
                value: "final_label == \"alpha\"".to_string(),
                context: ValueContext::FunctionArgument,
            }],
            missing_discriminators: Vec::new(),
        };
        assert!(
            has_same_test_boundary_oracle_pairing(&probe, Some(&owner), &[&paired], &activation),
            "an activation == fact on the asserted owner call must pair even when the call args are not the probe literals"
        );
        assert!(
            !has_same_test_boundary_oracle_pairing(
                &probe,
                Some(&owner),
                &[&paired],
                &ActivationEvidence::default(),
            ),
            "without the activation == fact, classify(\"word\") must not pair against final_label == \"alpha\""
        );
    }

    fn predicate_probe(expression: &str) -> Probe {
        Probe {
            id: ProbeId("probe:test".to_string()),
            location: SourceLocation::new("src/lib.rs", 2, 1),
            owner: Some(SymbolId("src/lib.rs::gate".to_string())),
            family: ProbeFamily::Predicate,
            delta: DeltaKind::Control,
            before: None,
            after: None,
            expression: expression.to_string(),
            expected_sinks: Vec::new(),
            required_oracles: Vec::new(),
        }
    }

    fn gate_owner() -> FunctionSummary {
        FunctionSummary {
            id: SymbolId("src/lib.rs::gate".to_string()),
            name: "gate".to_string(),
            file: PathBuf::from("src/lib.rs"),
            start_line: 1,
            end_line: 3,
            body: "pub fn gate(input: u32) -> bool { input >= 10 }".to_string(),
            calls: vec![],
            returns: vec![],
            literals: vec![],
            source_role: FunctionSourceRole::Production,
            attrs: vec![],
            impl_attrs: Vec::new(),
            nested_fn_names: Vec::new(),
            let_bindings: Vec::new(),
            impl_context: Default::default(),
            item: Default::default(),
        }
    }

    fn test_summary(
        name: &str,
        body: &str,
        calls: Vec<CallFact>,
        assertions: Vec<OracleFact>,
        literals: &[&str],
    ) -> TestSummary {
        TestSummary {
            name: name.to_string(),
            file: PathBuf::from("tests/gate.rs"),
            start_line: 1,
            end_line: 4,
            body: body.to_string(),
            calls,
            assertions,
            literals: literals
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

    fn call(name: &str, text: &str) -> CallFact {
        CallFact {
            line: 1,
            name: name.to_string(),
            text: text.to_string(),
        }
    }

    fn exact(text: &str) -> OracleFact {
        OracleFact {
            line: 1,
            text: text.to_string(),
            kind: OracleKind::ExactValue,
            strength: OracleStrength::Strong,
            observed_tokens: extract_identifier_tokens(text),
            ok_value_observed: None,
        }
    }
}
