//! Discriminator narration must follow the final classification, not oracle rank.

use super::super::owners_tests::{extract_owners, extract_tests};
use super::classify_change_with_old;
use crate::domain::{ExposureClass, Finding, StageState};
use std::path::Path;

/// Exercise the existing source-fact producer and final classifier together.
fn classify_case(
    source: &str,
    tests: &str,
    line: usize,
    before: &str,
    after: &str,
) -> Result<Finding, String> {
    let file = Path::new("src/subject.py");
    let owners = extract_owners(file, source);
    let tests = extract_tests(Path::new("tests/test_subject.py"), tests);
    classify_change_with_old(file, line, after, Some(before), &owners, &tests)
        .ok_or_else(|| "behavioral fixture must produce a finding".to_string())
}

/// Keep the exact non-credit wording separate from the positive control below.
fn assert_strong_oracle_is_not_discrimination(finding: &Finding) {
    assert_eq!(finding.class, ExposureClass::WeaklyExposed);
    assert_eq!(finding.ripr.reveal.discriminate.state, StageState::Weak);
    assert_eq!(
        finding.ripr.reveal.discriminate.summary,
        "Related Python test uses a `exact_value` oracle, but static evidence does not establish discrimination of the changed behavior."
    );
}

/// Explicit positional and keyword arguments both bypass the changed default.
#[test]
fn overridden_default_does_not_get_a_positive_discriminator_summary() -> Result<(), String> {
    let source =
        "def render(name, verbose=True):\n    return f\"[debug] {name}\" if verbose else name\n";
    for invocation in ["render(\"Sam\", verbose=False)", "render(\"Sam\", False)"] {
        let tests = format!(
            "from src.subject import render\n\ndef test_render():\n    assert {invocation} == \"Sam\"\n"
        );
        let finding = classify_case(
            source,
            &tests,
            1,
            "def render(name, verbose=False):",
            "def render(name, verbose=True):",
        )?;
        assert_strong_oracle_is_not_discrimination(&finding);
        assert!(
            finding
                .activation
                .missing_discriminators
                .iter()
                .any(|missing| missing.value == "call `render` without `verbose`"),
            "the existing omission repair must survive the narration fix"
        );
    }
    Ok(())
}

/// Omitting the parameter reaches the changed default under an exact observer.
#[test]
fn exercised_default_retains_its_positive_discriminator_summary() -> Result<(), String> {
    let finding = classify_case(
        "def render(name, verbose=True):\n    return f\"[debug] {name}\" if verbose else name\n",
        "from src.subject import render\n\ndef test_render():\n    assert render(\"Sam\") == \"[debug] Sam\"\n",
        1,
        "def render(name, verbose=False):",
        "def render(name, verbose=True):",
    )?;
    assert_eq!(finding.class, ExposureClass::Exposed);
    assert_eq!(finding.ripr.reveal.discriminate.state, StageState::Yes);
    assert_eq!(
        finding.ripr.reveal.discriminate.summary,
        "Related Python test uses a `exact_value` oracle; static evidence suggests the changed behavior is discriminated."
    );
    Ok(())
}

/// Observing the unchanged input is not observing the changed return value.
#[test]
fn orthogonal_strong_oracle_does_not_get_a_positive_discriminator_summary() -> Result<(), String> {
    let finding = classify_case(
        "def next_value(count):\n    return count - 1\n",
        "from src.subject import next_value\n\ndef test_input():\n    count = 5\n    result = next_value(count)\n    assert count == 5\n    assert result > 0\n",
        2,
        "    return count + 1",
        "    return count - 1",
    )?;
    assert_strong_oracle_is_not_discrimination(&finding);
    Ok(())
}

/// An exact normal-path result does not discriminate a changed raised exception.
#[test]
fn normal_path_oracle_does_not_credit_the_changed_error_path_summary() -> Result<(), String> {
    let finding = classify_case(
        "def parse(text):\n    if not text:\n        raise KeyError(\"empty\")\n    return int(text)\n",
        "from src.subject import parse\n\ndef test_parse():\n    assert parse(\"42\") == 42\n",
        3,
        "        raise ValueError(\"empty\")",
        "        raise KeyError(\"empty\")",
    )?;
    assert_strong_oracle_is_not_discrimination(&finding);
    Ok(())
}

/// A named static limitation remains the first authority even with a strong test.
#[test]
fn static_limit_keeps_its_named_discriminator_summary() -> Result<(), String> {
    let finding = classify_case(
        "def call_named(client, name):\n    return getattr(client, name)()\n",
        "from src.subject import call_named\n\ndef test_call():\n    assert call_named(client, \"total\") == 10\n",
        2,
        "    return client.total()",
        "    return getattr(client, name)()",
    )?;
    assert_eq!(finding.class, ExposureClass::StaticUnknown);
    assert_eq!(finding.ripr.reveal.discriminate.state, StageState::Unknown);
    assert_eq!(
        finding.ripr.reveal.discriminate.summary,
        "Static limit `dynamic_dispatch` prevents a safe Python discriminator claim."
    );
    Ok(())
}

/// Assert the fixture parsed into the intended owner before the verdict is read.
fn assert_owner(finding: &Finding, qualified: &str) {
    assert_eq!(
        finding.probe.owner.as_ref().map(|owner| owner.0.as_str()),
        Some(format!("python:src/subject.py::{qualified}").as_str()),
        "fixture must parse the intended owner"
    );
}

fn missing_boundary<'a>(finding: &'a Finding, value: &str) -> Option<&'a str> {
    finding
        .activation
        .missing_discriminators
        .iter()
        .find(|missing| missing.value == value)
        .map(|missing| missing.reason.as_str())
}

fn observed(finding: &Finding, value: &str) -> bool {
    finding
        .activation
        .observed_values
        .iter()
        .any(|fact| fact.value == value)
}

const INVENTORY_SOURCE: &str = "from dataclasses import dataclass\n\n\n@dataclass\nclass Item:\n    sku: str\n    on_hand: int\n\n\ndef reserve(item: Item, qty: int) -> int:\n    if qty >= item.on_hand:\n        raise ValueError(\"insufficient stock\")\n    item.on_hand -= qty\n    return item.on_hand\n";

/// The walkthrough inventory shape: `qty > item.on_hand` -> `qty >= item.on_hand`
/// with one exact assertion far from the boundary (10 on hand, reserve 3). The
/// old and new predicates agree on that input, so the exact oracle does not
/// discriminate the change and the finding must not be `exposed`.
#[test]
fn predicate_boundary_off_boundary_exact_oracle_is_weakly_exposed() -> Result<(), String> {
    let finding = classify_case(
        INVENTORY_SOURCE,
        "import unittest\n\nfrom src.subject import Item, reserve\n\n\nclass ItemTests(unittest.TestCase):\n    def test_reserve_reduces(self):\n        item = Item(\"a\", 10)\n        self.assertEqual(reserve(item, 3), 7)\n",
        11,
        "    if qty > item.on_hand:",
        "    if qty >= item.on_hand:",
    )?;
    assert_owner(&finding, "reserve");
    assert_eq!(finding.class, ExposureClass::WeaklyExposed);
    assert_eq!(finding.ripr.infect.state, StageState::Weak);
    assert_eq!(finding.ripr.reveal.observe.state, StageState::Yes);
    assert_eq!(finding.ripr.reveal.discriminate.state, StageState::Weak);
    // `item.on_hand` is unresolved, so the boundary is not established either
    // way: the finding is weak, the reason names the unresolved operand, and
    // no typed repair target (and so no repair card) is produced.
    assert!(
        finding.activation.missing_discriminators.is_empty(),
        "an unresolved operand must not become a typed repair target: {:?}",
        finding.activation.missing_discriminators
    );
    assert!(
        finding.missing.iter().any(|line| line.contains(
            "No strong related test call places qty equal to item.on_hand; observed qty values: 3; observed item.on_hand values: unresolved"
        )),
        "{:?}",
        finding.missing
    );
    assert!(observed(&finding, "qty = 3"), "{finding:?}");
    assert!(!observed(&finding, "qty == item.on_hand"));
    Ok(())
}

/// Unresolved attribute operands fail closed even when the test input happens
/// to sit on the boundary at runtime: static evidence cannot bind
/// `item.on_hand` through the `Item(...)` constructor.
#[test]
fn predicate_boundary_unresolved_attribute_operand_fails_closed() -> Result<(), String> {
    let finding = classify_case(
        INVENTORY_SOURCE,
        "from src.subject import Item, reserve\n\ndef test_reserve_all():\n    assert reserve(Item(\"a\", 3), 3) == 0\n",
        11,
        "    if qty > item.on_hand:",
        "    if qty >= item.on_hand:",
    )?;
    assert_owner(&finding, "reserve");
    assert_eq!(finding.class, ExposureClass::WeaklyExposed);
    // This test is on the boundary at runtime; naming the equality as missing
    // would hand out a repair card for a test that already exists.
    // The Python repair card requires a first missing discriminator, so an
    // empty list is what keeps the card off this finding.
    assert!(
        finding.activation.missing_discriminators.is_empty(),
        "{:?}",
        finding.activation.missing_discriminators
    );
    Ok(())
}

const DISCOUNT_SOURCE: &str =
    "def bulk_discount(quantity):\n    if quantity >= 100:\n        return 0.15\n    return 0.0\n";

/// Positive control: an exact assertion that calls the owner exactly at the
/// literal boundary pins the changed predicate and keeps `exposed`.
#[test]
fn predicate_boundary_literal_boundary_call_stays_exposed() -> Result<(), String> {
    let finding = classify_case(
        DISCOUNT_SOURCE,
        "from src.subject import bulk_discount\n\ndef test_bulk_discount_at_threshold():\n    assert bulk_discount(100) == 0.15\n",
        2,
        "    if quantity > 100:",
        "    if quantity >= 100:",
    )?;
    assert_owner(&finding, "bulk_discount");
    assert_eq!(finding.class, ExposureClass::Exposed);
    assert_eq!(finding.ripr.infect.state, StageState::Yes);
    assert!(finding.activation.missing_discriminators.is_empty());
    assert!(observed(&finding, "quantity == 100"), "{finding:?}");
    assert!(observed(&finding, "quantity = 100"), "{finding:?}");
    Ok(())
}

/// Discriminating negative for the control above: the same exact oracle one
/// step away from the literal boundary does not pin it.
#[test]
fn predicate_boundary_literal_off_boundary_call_is_weakly_exposed() -> Result<(), String> {
    let finding = classify_case(
        DISCOUNT_SOURCE,
        "from src.subject import bulk_discount\n\ndef test_bulk_discount_large():\n    assert bulk_discount(150) == 0.15\n",
        2,
        "    if quantity > 100:",
        "    if quantity >= 100:",
    )?;
    assert_eq!(finding.class, ExposureClass::WeaklyExposed);
    assert_eq!(
        missing_boundary(&finding, "quantity == 100"),
        Some(
            "No strong related test call places quantity equal to 100; observed quantity values: 150"
        )
    );
    assert!(!observed(&finding, "quantity == 100"));
    Ok(())
}

/// Two parameters bound to equal literals (positional and keyword forms, and
/// numeric forms Python compares equal) pin a parameter-to-parameter boundary.
#[test]
fn predicate_boundary_equal_parameter_bindings_stay_exposed() -> Result<(), String> {
    let source = "def reserve(on_hand, qty):\n    if qty >= on_hand:\n        return 0\n    return on_hand - qty\n";
    for call in [
        "reserve(3, 3)",
        "reserve(3, qty=3)",
        "reserve(qty=3, on_hand=3.0)",
    ] {
        let tests = format!(
            "from src.subject import reserve\n\ndef test_reserve_boundary():\n    assert {call} == 0\n"
        );
        let finding = classify_case(
            source,
            &tests,
            2,
            "    if qty > on_hand:",
            "    if qty >= on_hand:",
        )?;
        assert_eq!(finding.class, ExposureClass::Exposed, "{call}: {finding:?}");
        assert!(observed(&finding, "qty == on_hand"), "{call}");
    }
    Ok(())
}

/// A literal default binds an omitted parameter; an explicit override that
/// moves off the boundary does not.
#[test]
fn predicate_boundary_uses_literal_defaults_for_omitted_parameters() -> Result<(), String> {
    let source = "def over_limit(total, limit=10):\n    if total > limit:\n        return True\n    return False\n";
    let at_default = classify_case(
        source,
        "from src.subject import over_limit\n\ndef test_at_limit():\n    assert over_limit(10) == False\n",
        2,
        "    if total >= limit:",
        "    if total > limit:",
    )?;
    assert_eq!(at_default.class, ExposureClass::Exposed, "{at_default:?}");
    let overridden = classify_case(
        source,
        "from src.subject import over_limit\n\ndef test_at_limit():\n    assert over_limit(10, limit=5) == True\n",
        2,
        "    if total >= limit:",
        "    if total > limit:",
    )?;
    assert_eq!(overridden.class, ExposureClass::WeaklyExposed);
    assert_eq!(
        missing_boundary(&overridden, "total == limit"),
        Some(
            "No strong related test call places total equal to limit; observed total values: 10; observed limit values: 5"
        )
    );
    Ok(())
}

/// Method calls bind arguments after the implicit `self` receiver.
#[test]
fn predicate_boundary_binds_method_arguments_after_self() -> Result<(), String> {
    let source = "class Cart:\n    def is_bulk(self, count):\n        if count >= 3:\n            return True\n        return False\n";
    for (count, expected) in [
        ("3", ExposureClass::Exposed),
        ("4", ExposureClass::WeaklyExposed),
    ] {
        let tests = format!(
            "from src.subject import Cart\n\ndef test_is_bulk():\n    cart = Cart()\n    assert cart.is_bulk({count}) == True\n"
        );
        let finding = classify_case(
            source,
            &tests,
            3,
            "        if count > 3:",
            "        if count >= 3:",
        )?;
        assert_owner(&finding, "Cart.is_bulk");
        assert_eq!(finding.class, expected, "count={count}: {finding:?}");
    }
    Ok(())
}

/// A computed operand cannot be bound to a test input: with literal owner
/// inputs visible it fails closed without inventing a typed discriminator.
/// A non-relational predicate is outside the boundary rule and keeps the
/// existing strong-oracle verdict.
#[test]
fn predicate_boundary_scope_computed_and_non_relational_predicates() -> Result<(), String> {
    let computed = classify_case(
        "def is_long(name):\n    if len(name) > 3:\n        return True\n    return False\n",
        "from src.subject import is_long\n\ndef test_is_long():\n    assert is_long(\"abcd\") == True\n",
        2,
        "    if len(name) >= 3:",
        "    if len(name) > 3:",
    )?;
    assert_eq!(computed.class, ExposureClass::WeaklyExposed);
    assert_eq!(computed.ripr.infect.state, StageState::Weak);
    assert!(
        computed.activation.missing_discriminators.is_empty(),
        "a computed operand must not become a typed repair target: {:?}",
        computed.activation.missing_discriminators
    );
    assert!(observed(&computed, "name = \"abcd\""));

    let equality = classify_case(
        "def is_zero(value):\n    if value == 0:\n        return True\n    return False\n",
        "from src.subject import is_zero\n\ndef test_is_zero():\n    assert is_zero(5) == False\n",
        2,
        "    if value != 0:",
        "    if value == 0:",
    )?;
    assert_eq!(equality.class, ExposureClass::Exposed);
    assert!(equality.activation.observed_values.is_empty());
    Ok(())
}

/// Without any literal owner argument the gate cannot see the activating
/// input in either direction (Rust's empty `call_values` rule). The verdict
/// stays with the oracle rules and the finding names the limitation, so the
/// unresolved activation stays visible rather than silently credited.
#[test]
fn predicate_boundary_without_literal_inputs_names_the_limitation() -> Result<(), String> {
    for tests in [
        "from src.subject import bulk_discount\n\ndef test_bulk_discount_local():\n    quantity = 150\n    assert bulk_discount(quantity) == 0.15\n",
        "from src.subject import bulk_discount\n\ndef test_bulk_discount_list():\n    assert bulk_discount(*[100]) == 0.15\n",
    ] {
        let finding = classify_case(
            DISCOUNT_SOURCE,
            tests,
            2,
            "    if quantity > 100:",
            "    if quantity >= 100:",
        )?;
        assert_owner(&finding, "bulk_discount");
        assert_eq!(finding.class, ExposureClass::Exposed, "{tests}");
        assert!(finding.activation.observed_values.is_empty());
        assert!(
            finding.evidence.iter().any(|line| line
                == "boundary_activation_unresolved: no strong related test call to `bulk_discount` binds a literal argument, so static evidence cannot place an input at the changed boundary of `quantity >= 100`"),
            "{:?}",
            finding.evidence
        );
    }
    Ok(())
}

/// Import aliases are followed when binding call arguments.
#[test]
fn predicate_boundary_follows_import_alias_calls() -> Result<(), String> {
    let finding = classify_case(
        DISCOUNT_SOURCE,
        "from src.subject import bulk_discount as bd\n\ndef test_alias_at_threshold():\n    assert bd(100) == 0.15\n",
        2,
        "    if quantity > 100:",
        "    if quantity >= 100:",
    )?;
    assert_eq!(finding.class, ExposureClass::Exposed, "{finding:?}");
    Ok(())
}
