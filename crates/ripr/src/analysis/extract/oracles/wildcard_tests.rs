use super::{classify_assertion, extract_assertions};
use crate::domain::{OracleKind, OracleStrength};

#[test]
fn unguarded_wildcard_patterns_are_not_exact_oracles() -> Result<(), String> {
    for text in [
        "assert!(matches!(value, _));",
        "assert!(matches!(value, (_)));",
        "assert!(matches!(value, (( /* explanation */ _ ))));",
        "assert!((matches!(value, _)));",
        "assert!(/* explanation */ (matches!(value, _)));",
        "assert!((matches!(value, _)) /* explanation */);",
        "assert!(std::matches!(value, _));",
        "assert!(::std::matches!(value, _));",
        "assert!(std :: matches!(value, _));",
        "assert!(core /* explanation */ :: matches!(value, _));",
        "assert!(core::matches! { value, _ });",
        "assert!(matches![value, _], \"{:?}\", unrelated.is_err());",
        "assert!(matches!(value, _ /* every value */));",
        "assert!(/* explanation */ matches!(value, _));",
        "assert!(matches! /* explanation */ (value, _));",
        "assert!(matches!(value, _, /* trailing comma */));",
        "assert!(matches!(\"actual\", _, /* trailing comma */));",
        "assert_matches!(value, _);",
        "assert_matches!(value, (_));",
        "assert_matches! { value, _ };",
        "debug_assert_matches!(value, _);",
        "ensure!(matches!(value, _), \"not a value pin\");",
    ] {
        let classification = classify_assertion(text);
        if classification.kind != OracleKind::RelationalCheck
            || classification.strength != OracleStrength::Weak
        {
            return Err(format!(
                "unguarded wildcard must not claim an exact discriminator: {text}: {classification:?}"
            ));
        }
        let facts = extract_assertions(text, 40);
        let [fact] = facts.as_slice() else {
            return Err(format!("expected one actual oracle for {text}: {facts:?}"));
        };
        if fact.line != 40
            || fact.kind != OracleKind::RelationalCheck
            || fact.strength != OracleStrength::Weak
        {
            return Err(format!("extraction lost wildcard weakness: {fact:?}"));
        }
    }
    Ok(())
}

#[test]
fn wildcard_downgrade_preserves_patterns_guards_and_other_conditions() -> Result<(), String> {
    for text in [
        "assert!(matches!(value, 2));",
        "assert!(matches!(value, Some(2)));",
        "assert!(matches!(value, (2)));",
        "assert!(matches!(value, _ if value == 2));",
        "assert!(::std::matches!(value, _ if value == 2));",
        "assert!(/* explanation */ (std :: matches!(value, 2)));",
        "assert_matches!(value, _ if value == 2);",
        "assert!(custom::matches!(value, _));",
        "assert!(matches!(value, _) && value == 2);",
        "assert_eq!(value, 2, \"{:?}\", matches!(unrelated, _));",
        "assert_eq!(value, \"_\");",
    ] {
        let classification = classify_assertion(text);
        if classification.kind != OracleKind::ExactValue
            || classification.strength != OracleStrength::Strong
        {
            return Err(format!(
                "wildcard-only repair changed a retained oracle: {text}: {classification:?}"
            ));
        }
    }
    let variant = classify_assertion("assert!(matches!(value, Err(ParseError::InvalidDigit)));");
    if variant.kind != OracleKind::ExactErrorVariant || variant.strength != OracleStrength::Strong {
        return Err(format!("exact error variant lost specificity: {variant:?}"));
    }
    Ok(())
}

#[test]
fn wildcard_runtime_control_accepts_wrong_behavior_while_exact_pin_distinguishes_it() {
    fn original(value: i32) -> i32 {
        value + 1
    }
    fn wrong(value: i32) -> i32 {
        value + 2
    }
    let original_value = original(1);
    let wrong_value = wrong(1);
    assert_ne!(original_value, wrong_value);
    assert!(matches!(original_value, _));
    assert!(matches!(wrong_value, _));
    assert!(matches!(original_value, 2));
    assert!(!matches!(wrong_value, 2));
    assert!(matches!(original_value, _ if original_value == 2));
    assert!(!matches!(wrong_value, _ if wrong_value == 2));
}

#[test]
fn parser_backed_wildcard_uses_the_same_non_exact_authority() -> Result<(), String> {
    use crate::analysis::syntax::{RaRustSyntaxAdapter, RustSyntaxAdapter};
    let source = r#"
#[test]
fn observes_score() {
    let value = score(1);
    assert!(matches!(value, _));
    assert!(/* explanation */ (matches!(value, _)));
    assert!((matches!(value, _)) /* explanation */);
    assert!(::std::matches!(value, _));
    assert!(std :: matches!(value, _));
    assert!(matches!(value, 2));
}
"#;
    let facts = RaRustSyntaxAdapter.summarize_file(std::path::Path::new("src/lib.rs"), source)?;
    let [test] = facts.tests.as_slice() else {
        return Err(format!(
            "expected one parsed test, got {}",
            facts.tests.len()
        ));
    };
    let [
        wildcard,
        grouped_before,
        grouped_after,
        global_path,
        spaced_path,
        exact,
    ] = test.assertions.as_slice()
    else {
        return Err(format!(
            "expected six parsed oracles: {:?}",
            test.assertions
        ));
    };
    if [
        wildcard,
        grouped_before,
        grouped_after,
        global_path,
        spaced_path,
    ]
    .into_iter()
    .any(|fact| fact.kind != OracleKind::RelationalCheck || fact.strength != OracleStrength::Weak)
        || exact.kind != OracleKind::ExactValue
        || exact.strength != OracleStrength::Strong
    {
        return Err(format!(
            "parsed wildcard/exact controls disagree: {:?}",
            test.assertions
        ));
    }
    Ok(())
}
