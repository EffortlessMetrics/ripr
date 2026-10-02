use super::{classify_assertion, extract_assertions, extract_line_scanned_oracles};
use crate::analysis::facts::OracleFact;
use crate::domain::{OracleKind, OracleStrength};

fn require_kind(
    fact: &OracleFact,
    kind: OracleKind,
    strength: OracleStrength,
) -> Result<(), String> {
    if fact.kind != kind || fact.strength != strength {
        return Err(format!(
            "{}: expected {kind:?}/{strength:?}, got {:?}/{:?}",
            fact.text, fact.kind, fact.strength
        ));
    }
    Ok(())
}

#[test]
fn relational_scalar_fields_do_not_inherit_observer_or_mock_names() -> Result<(), String> {
    let fields = [
        "published_payload_bytes",
        "suppressed_payload_bytes",
        "counter",
        "state_count",
        "expect_published_count",
    ];
    for field in fields {
        let text = format!("assert!(plan.{field} > 0);");
        let facts = extract_assertions(&text, 3096);
        if facts.len() != 1 {
            return Err(format!(
                "expected one actual fact for {text}, got {facts:?}"
            ));
        }
        let fact = facts.first().ok_or("missing actual predicate fact")?;
        if fact.line != 3096
            || fact.text != text
            || !fact.observed_tokens.iter().any(|token| token == field)
            || !fact.observed_tokens.iter().any(|token| token == "plan")
        {
            return Err(format!("predicate source identity lost: {fact:?}"));
        }
        require_kind(fact, OracleKind::RelationalCheck, OracleStrength::Weak)?;
    }
    Ok(())
}

#[test]
fn line_scanned_relational_predicate_preserves_recognition_boundary() -> Result<(), String> {
    let body = format!(
        "assert!(unchanged.suppressed_payload_bytes > 0);\n{}assert!(changed_plan.published_payload_bytes > 0);\nmock_service.expect_publish().times(1);",
        "\n".repeat(12)
    );
    let parsed = extract_assertions(&body, 3096);
    if parsed.len() != 3 {
        return Err(format!(
            "parsed route must emit three real facts: {parsed:?}"
        ));
    }
    for (position, line) in [(0, 3096), (1, 3109)] {
        let fact = parsed.get(position).ok_or("missing parsed twin")?;
        if fact.line != line {
            return Err(format!("parsed line changed: {fact:?}"));
        }
        require_kind(fact, OracleKind::RelationalCheck, OracleStrength::Weak)?;
    }
    let scanned = extract_line_scanned_oracles(&body, 3096);
    if scanned.len() != 2 {
        return Err(format!(
            "fallback must retain published/mock only, not general asserts: {scanned:?}"
        ));
    }
    let published = scanned
        .first()
        .ok_or("missing recognized published predicate")?;
    if published.line != 3109
        || !published
            .observed_tokens
            .iter()
            .any(|token| token == "published_payload_bytes")
    {
        return Err(format!("fallback predicate identity lost: {published:?}"));
    }
    require_kind(published, OracleKind::RelationalCheck, OracleStrength::Weak)?;
    let mock = scanned.get(1).ok_or("missing real mock expectation")?;
    if mock.line != 3110 {
        return Err(format!("mock line changed: {mock:?}"));
    }
    require_kind(mock, OracleKind::MockExpectation, OracleStrength::Medium)
}

#[test]
fn relational_messages_and_call_chains_do_not_steal_oracle_kind() -> Result<(), String> {
    for text in [
        "assert!((plan.published_payload_bytes > 0));",
        "assert!(amount > 0, \"published assert!(plan.counter > 0)\");",
    ] {
        let facts = extract_assertions(text, 40);
        if facts.len() != 1 {
            return Err(format!("condition must emit one real fact: {facts:?}"));
        }
        require_kind(
            facts.first().ok_or("missing condition fact")?,
            OracleKind::RelationalCheck,
            OracleStrength::Weak,
        )?;
    }
    for text in [
        "mock_service.expect_publish().times(1);",
        "assert!(mock_service.expect_publish().times(1) > 0);",
        "assert!(event.published);",
    ] {
        let facts = extract_assertions(text, 41);
        if facts.len() != 1 {
            return Err(format!(
                "mock/observer control must emit one real fact: {facts:?}"
            ));
        }
        require_kind(
            facts.first().ok_or("missing retained mock control")?,
            OracleKind::MockExpectation,
            OracleStrength::Medium,
        )?;
    }
    // Quoted/raw assertion spellings and extra tokens are not a complete outer
    // scalar assertion. Retain the pre-existing generic observer classification.
    for text in [
        "\"assert!(plan.published > 0)\"",
        "r#\"assert!(plan.published > 0)\"#",
        "wrapper(assert!(plan.published > 0));",
        "// assert!(plan.published > 0)",
        "assert!(matches!(plan.published, _));",
        "assert!(plan.published > 0) trailing",
        "assert!(plan.published > 0",
    ] {
        let classification = classify_assertion(text);
        if classification.kind == OracleKind::RelationalCheck {
            return Err(format!(
                "new scalar matcher must not admit non-outer macro text: {text}"
            ));
        }
    }
    Ok(())
}

#[test]
fn scalar_admission_preserves_diagnostic_projection_and_stronger_oracles() -> Result<(), String> {
    for text in [
        "assert!(plan.published_payload_bytes >= 1);",
        "assert!(plan.published_payload_bytes < 1);",
        "assert!(0 <= plan.published_payload_bytes);",
        "assert!(0 < plan.expect_published_count);",
        "assert!(plan.published_payload_bytes > 0, \"{:?}\", unrelated.is_err());",
        "ensure!(plan.published_payload_bytes > 0, \"{:?}\", unrelated.is_err());",
    ] {
        let classification = classify_assertion(text);
        if classification.kind != OracleKind::RelationalCheck
            || classification.strength != OracleStrength::Weak
        {
            return Err(format!(
                "scalar condition changed: {text}: {classification:?}"
            ));
        }
    }
    for (text, kind, strength) in [
        (
            "assert_eq!(plan.published_payload_bytes, 7, \"{:?}\", unrelated.is_err());",
            OracleKind::ExactValue,
            OracleStrength::Strong,
        ),
        (
            "assert_matches!(result, Err(ParseError::InvalidDigit));",
            OracleKind::ExactErrorVariant,
            OracleStrength::Strong,
        ),
        (
            "assert!(result.is_err());",
            OracleKind::BroadError,
            OracleStrength::Weak,
        ),
        (
            "assert_snapshot!(published);",
            OracleKind::Snapshot,
            OracleStrength::Medium,
        ),
        (
            "assert!(plan.published.is_some());",
            OracleKind::SmokeOnly,
            OracleStrength::Smoke,
        ),
        (
            "mock_service.expect_publish().times(1);",
            OracleKind::MockExpectation,
            OracleStrength::Medium,
        ),
        (
            "assert!(event.published);",
            OracleKind::MockExpectation,
            OracleStrength::Medium,
        ),
    ] {
        let classification = classify_assertion(text);
        if classification.kind != kind || classification.strength != strength {
            return Err(format!(
                "retained oracle changed: {text}: {classification:?}"
            ));
        }
    }
    Ok(())
}

#[test]
fn scalar_parentheses_do_not_admit_blocks_or_arrays() -> Result<(), String> {
    for text in [
        "assert!({ plan.published_payload_bytes > 0 });",
        "assert!([plan.published_payload_bytes > 0]);",
        "assert!(({ plan.published_payload_bytes > 0 }));",
        "assert!(([plan.published_payload_bytes > 0]));",
    ] {
        let classification = classify_assertion(text);
        if classification.kind != OracleKind::MockExpectation
            || classification.strength != OracleStrength::Medium
        {
            return Err(format!(
                "non-parenthesis wrapper admitted: {text}: {classification:?}"
            ));
        }
    }
    let text = "assert!(((plan.published_payload_bytes > 0)));";
    let classification = classify_assertion(text);
    if classification.kind != OracleKind::RelationalCheck
        || classification.strength != OracleStrength::Weak
    {
        return Err(format!("nested parentheses lost: {classification:?}"));
    }
    Ok(())
}
