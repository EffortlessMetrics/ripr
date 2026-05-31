use super::*;
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

// --------------------------------------------------------------
// Coverage-pinning tests below.
//
// These exercise previously uncovered branches in this file:
// assertion harvesting through nested control flow (if/for/while/
// with/try/match), additional oracle dispatch arms, probe-shape
// classification arms (elif, while, for, match, case, except*,
// finally, try:, with raises, await call, assign-from-call,
// mock initializer), call-boundary helpers, comment / string
// suppression in body scanning, path / file helpers, the
// workspace walker, and a real end-to-end `analyze_diff` call
// that emits findings against an on-disk fixture workspace.
// --------------------------------------------------------------

fn write_file(path: &Path, contents: &str) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|err| format!("create_dir_all({}): {err}", parent.display()))?;
    }
    std::fs::write(path, contents).map_err(|err| format!("write({}): {err}", path.display()))?;
    Ok(())
}

fn unique_tempdir(label: &str) -> Result<PathBuf, String> {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|err| format!("system time: {err}"))?
        .as_nanos();
    let dir = std::env::temp_dir().join(format!(
        "ripr-python-coverage-{label}-{}-{nanos}",
        std::process::id()
    ));
    std::fs::create_dir_all(&dir)
        .map_err(|err| format!("create_dir_all({}): {err}", dir.display()))?;
    Ok(dir)
}

fn assertion_oracles(source: &str) -> Vec<(OracleKind, OracleStrength)> {
    let tests = extract_tests(Path::new("tests/test_inline.py"), source);
    let mut out = Vec::new();
    for test in tests {
        for assertion in test.assertions {
            out.push((assertion.oracle_kind, assertion.oracle_strength));
        }
    }
    out
}

fn assertion_shapes(source: &str) -> Vec<PythonOracleShape> {
    let tests = extract_tests(Path::new("tests/test_inline.py"), source);
    let mut out = Vec::new();
    for test in tests {
        for assertion in test.assertions {
            out.push(assertion.oracle_shape);
        }
    }
    out
}

fn source_fact_kind_names(facts: &PythonSourceFacts) -> BTreeSet<&'static str> {
    facts.facts.iter().map(|fact| fact.kind.as_str()).collect()
}

#[test]
fn extract_source_facts_covers_python_static_fact_shapes() -> Result<(), String> {
    let source = r#"
import logging

logger = logging.getLogger(__name__)
MODULE_STATE = {"status": "pending", "items": [1], "tags": {"new"}}

@dataclass
class Invoice:
    status: str = "pending"

    @classmethod
    def from_total(cls, total, threshold=10):
        invoice = cls()
        invoice.status = "paid" if total >= threshold and total != 0 else "pending"
        if total >= threshold and total != 0:
            print("paid")
            logger.warning("paid")
            return {"status": invoice.status, "items": [total], "tags": {"paid"}}
        raise ValueError("too low")

def summarize(invoice):
    return f"status={invoice.status}"
"#;

    let facts = extract_source_facts(Path::new("src/invoice.py"), source);
    let kinds = source_fact_kind_names(&facts);
    let expected = [
        "module",
        "class",
        "function",
        "method",
        "decorator",
        "parameter",
        "return",
        "raise",
        "predicate",
        "comparison",
        "boolean_expression",
        "call",
        "assignment",
        "attribute_write",
        "dict_literal",
        "list_literal",
        "set_literal",
        "string_literal",
        "print_call",
        "log_call",
    ];
    for kind in expected {
        if !kinds.contains(kind) {
            return Err(format!("expected source fact kind `{kind}`, got {kinds:?}"));
        }
    }

    if facts.language != "python" {
        return Err(format!(
            "expected python language metadata, got {}",
            facts.language
        ));
    }
    if facts.file != Path::new("src/invoice.py") {
        return Err(format!("expected file metadata, got {:?}", facts.file));
    }
    if !facts.limitations.is_empty() {
        return Err(format!(
            "valid source should not emit limitations: {:?}",
            facts.limitations
        ));
    }
    if !facts
        .owners
        .iter()
        .any(|owner| owner.qualified_name == "Invoice.from_total")
    {
        return Err(format!("expected method owner, got {:?}", facts.owners));
    }
    let method_return = facts
        .facts
        .iter()
        .find(|fact| {
            fact.kind == PythonSourceFactKind::Return
                && fact.owner.as_deref() == Some("Invoice.from_total")
        })
        .ok_or_else(|| "expected return fact owned by Invoice.from_total".to_string())?;
    if method_return.start_line == 0 || method_return.end_line < method_return.start_line {
        return Err(format!("invalid return span: {:?}", method_return));
    }
    if !method_return.text.starts_with("return {") {
        return Err(format!(
            "expected trimmed return text, got {:?}",
            method_return.text
        ));
    }
    Ok(())
}

#[test]
fn extract_source_facts_reports_malformed_python_as_named_limit() -> Result<(), String> {
    let facts = extract_source_facts(Path::new("src/oops.py"), "def broken(:\n    pass\n");
    if !facts.facts.is_empty() {
        return Err(format!(
            "malformed source should not emit facts, got {:?}",
            facts.facts
        ));
    }
    let limit = facts
        .limitations
        .first()
        .ok_or_else(|| "expected unsupported_syntax limitation".to_string())?;
    if limit.kind != StaticLimitKind::UnsupportedSyntax {
        return Err(format!("expected unsupported_syntax, got {:?}", limit.kind));
    }
    if !limit
        .evidence
        .starts_with("source_fact_parse_error: parse_error:")
    {
        return Err(format!(
            "expected named parse evidence, got {}",
            limit.evidence
        ));
    }
    if !limit
        .missing
        .contains("malformed Python prevented source-fact extraction")
    {
        return Err(format!(
            "expected actionable missing text, got {}",
            limit.missing
        ));
    }
    Ok(())
}

#[test]
fn collect_assertions_walks_control_flow_bodies() -> Result<(), String> {
    let source = r#"
def test_walks_control_flow():
    if value:
        assert value == 1
    else:
        assert value == 2
    for item in items:
        assert item == 0
    else:
        assert items == []
    while count:
        assert count == 5
    else:
        assert count == 0
    with open("p") as f:
        assert f.read() == "ok"
    try:
        assert raw == 7
    except ValueError:
        assert handled == 8
    else:
        assert orelse == 9
    finally:
        assert finalbody == 10
    match value:
        case 1:
            assert value == 11
        case _:
            assert value == 12
"#;
    let tests = extract_tests(Path::new("tests/test_walks.py"), source);
    if tests.len() != 1 {
        return Err(format!("expected single test, got {}", tests.len()));
    }
    let exact = tests[0]
        .assertions
        .iter()
        .filter(|assertion| assertion.oracle_kind == OracleKind::ExactValue)
        .count();
    // Every assert above uses `==`, so each must surface as ExactValue.
    if exact < 12 {
        return Err(format!(
            "expected at least 12 exact-value assertions across nested control flow, got {exact}"
        ));
    }
    Ok(())
}

#[test]
fn collect_assertions_walks_try_star_and_async_for_and_async_with() -> Result<(), String> {
    let source = r#"
async def test_walks_async_and_try_star():
    async for chunk in stream:
        assert chunk == "ok"
    async with lock:
        assert held == 1
    try:
        await do()
    except* RuntimeError:
        assert grouped == 2
"#;
    let tests = extract_tests(Path::new("tests/test_async.py"), source);
    if tests.len() != 1 {
        return Err(format!("expected single async test, got {}", tests.len()));
    }
    let kinds: Vec<&OracleKind> = tests[0]
        .assertions
        .iter()
        .map(|assertion| &assertion.oracle_kind)
        .collect();
    if kinds.len() < 3 {
        return Err(format!("expected three nested asserts, got {:?}", kinds));
    }
    if !kinds.iter().all(|kind| **kind == OracleKind::ExactValue) {
        return Err(format!(
            "expected all nested asserts to be exact-value, got {:?}",
            kinds
        ));
    }
    Ok(())
}

#[test]
fn collect_with_item_assertions_extracts_pytest_raises_in_context() -> Result<(), String> {
    let source = r#"
import pytest

def test_with_item_assertion():
    with pytest.raises(ValueError):
        do_thing()
"#;
    let oracles = assertion_oracles(source);
    if !oracles.iter().any(|(kind, strength)| {
        matches!(kind, OracleKind::BroadError) && *strength == OracleStrength::Weak
    }) {
        return Err(format!(
            "expected pytest.raises(...) context manager to register BroadError oracle, got {:?}",
            oracles
        ));
    }
    Ok(())
}

#[test]
fn collect_with_item_assertions_treats_pytest_raises_match_as_exact_error() -> Result<(), String> {
    let source = r#"
import pytest

def test_with_item_assertion():
    with pytest.raises(ValueError, match="positive required"):
        do_thing()
"#;
    let oracles = assertion_oracles(source);
    if !oracles.iter().any(|(kind, strength)| {
        matches!(kind, OracleKind::ExactErrorVariant) && *strength == OracleStrength::Strong
    }) {
        return Err(format!(
            "expected pytest.raises(..., match=...) to register ExactErrorVariant oracle, got {:?}",
            oracles
        ));
    }
    Ok(())
}

#[test]
fn pytest_oracle_shapes_cover_repair_routing_categories() -> Result<(), String> {
    let source = r#"
from pytest import raises

def test_pytest_shapes(client, caplog, capsys, monkeypatch):
    assert calculate(2) == 3
    assert amount >= threshold
    with raises(ValueError):
        fail()
    assert result["status"] == "paid"
    assert "expired" in caplog.text
    assert (caplog.text or captured.stdout) == "expired"
    assert caplog.text.lower == "expired"
    assert (result.kind or result["kind"]) == "paid"
    assert capsys.readouterr().out == "ok\n"
    assert response.status_code == 422
    assert flag
    assert_valid(result)
"#;
    let tests = extract_tests(Path::new("tests/test_shapes.py"), source);
    let test = tests
        .first()
        .ok_or_else(|| "expected pytest test to be extracted".to_string())?;
    assert_eq!(
        test.fixtures,
        vec![
            "caplog".to_string(),
            "capsys".to_string(),
            "client".to_string(),
            "monkeypatch".to_string()
        ]
    );

    let helper_shapes = assertion_shapes(source);
    if helper_shapes.len() != test.assertions.len() {
        return Err(format!(
            "shape helper should see same assertions, got helper={} direct={}",
            helper_shapes.len(),
            test.assertions.len()
        ));
    }
    let shapes: BTreeSet<_> = helper_shapes.into_iter().collect();
    for expected in [
        PythonOracleShape::ExactAssertion,
        PythonOracleShape::BoundaryAssertion,
        PythonOracleShape::ExceptionAssertion,
        PythonOracleShape::FieldAssertion,
        PythonOracleShape::OutputAssertion,
        PythonOracleShape::StatusCodeAssertion,
        PythonOracleShape::BroadSmokeAssertion,
        PythonOracleShape::UnknownCustomHelper,
    ] {
        if !shapes.contains(&expected) {
            return Err(format!(
                "expected oracle shape `{}` in {:?}",
                expected.as_str(),
                shapes
            ));
        }
    }
    Ok(())
}

#[test]
fn oracle_shape_names_are_stable() {
    let names: Vec<_> = [
        PythonOracleShape::ExactAssertion,
        PythonOracleShape::BoundaryAssertion,
        PythonOracleShape::ExceptionAssertion,
        PythonOracleShape::FieldAssertion,
        PythonOracleShape::OutputAssertion,
        PythonOracleShape::StatusCodeAssertion,
        PythonOracleShape::BroadSmokeAssertion,
        PythonOracleShape::MockExpectation,
        PythonOracleShape::UnknownCustomHelper,
    ]
    .into_iter()
    .map(PythonOracleShape::as_str)
    .collect();
    assert_eq!(
        names,
        vec![
            "exact_assertion",
            "boundary_assertion",
            "exception_assertion",
            "field_assertion",
            "output_assertion",
            "status_code_assertion",
            "broad_smoke_assertion",
            "mock_expectation",
            "unknown_custom_helper",
        ]
    );
}

#[test]
fn extract_tests_records_vararg_and_kwarg_pytest_fixtures() -> Result<(), String> {
    let tests = extract_tests(
        Path::new("tests/test_fixtures.py"),
        r#"
def test_fixture_args(amount, *extras, client, **kw):
    assert amount == 1
"#,
    );
    let test = tests
        .first()
        .ok_or_else(|| "expected pytest test to be extracted".to_string())?;
    assert_eq!(
        test.fixtures,
        vec![
            "amount".to_string(),
            "client".to_string(),
            "extras".to_string(),
            "kw".to_string(),
        ]
    );
    Ok(())
}

#[test]
fn verify_command_for_test_selects_pytest_and_unittest_runners() -> Result<(), String> {
    let tests = extract_tests(
        Path::new("tests/test_checkout.py"),
        r#"
import unittest

class TestCheckout:
    def test_pytest_route(self):
        assert checkout() == "ok"

class CheckoutTests(unittest.TestCase):
    def test_unittest_route(self):
        self.assertEqual(checkout(), "ok")
"#,
    );
    let pytest_test = tests
        .iter()
        .find(|test| test.name == "test_pytest_route")
        .ok_or_else(|| "missing pytest class test".to_string())?;
    let unittest_test = tests
        .iter()
        .find(|test| test.name == "test_unittest_route")
        .ok_or_else(|| "missing unittest class test".to_string())?;

    assert_eq!(pytest_test.qualified_name, "TestCheckout.test_pytest_route");
    assert_eq!(
        verify_command_for_test(pytest_test).as_deref(),
        Some("pytest tests/test_checkout.py::TestCheckout::test_pytest_route")
    );
    assert_eq!(
        unittest_test.qualified_name,
        "CheckoutTests.test_unittest_route"
    );
    assert_eq!(
        verify_command_for_test(unittest_test).as_deref(),
        Some("python -m unittest tests.test_checkout.CheckoutTests.test_unittest_route")
    );
    Ok(())
}

#[test]
fn unittest_oracle_shapes_use_assertion_arguments() -> Result<(), String> {
    let source = r#"
import unittest

class ResponseTests(unittest.TestCase):
    def test_shapes(self):
        self.assertEqual(response.status_code, 422)
        self.assertDictEqual(payload, {"status": "paid"})
        self.assertIn("expired", result.output)
        self.assertRegex(result.stderr, "expired")
"#;
    let shapes: BTreeSet<_> = assertion_shapes(source).into_iter().collect();
    for expected in [
        PythonOracleShape::StatusCodeAssertion,
        PythonOracleShape::FieldAssertion,
        PythonOracleShape::OutputAssertion,
    ] {
        if !shapes.contains(&expected) {
            return Err(format!(
                "expected unittest oracle shape `{}` in {:?}",
                expected.as_str(),
                shapes
            ));
        }
    }
    Ok(())
}

#[test]
fn oracle_for_call_recognizes_all_unittest_and_mock_variants() -> Result<(), String> {
    let source = r#"
import unittest

class CaseAll(unittest.TestCase):
    def test_variants(self):
        self.assertEqual(actual, 1)
        self.assertNotEqual(actual, 2)
        self.assertTrue(actual)
        self.assertFalse(actual)
        with self.assertRaises(ValueError):
            do_one()
        with self.assertRaisesRegex(ValueError, "bad"):
            do_two()
        self.assertIn("status", payload)
        self.assertRegex(message, "paid")
        self.assertDictEqual(payload, {"status": "paid"})
        mock.assert_called()
        mock.assert_called_once()
        mock.assert_called_with(1)
        mock.assert_called_once_with(1)
        mock.assert_any_call(1)
        mock.assert_has_calls([call(1)])
        mock.assert_not_called()
        unknown_call(actual)
"#;
    let oracles = assertion_oracles(source);
    let strong = oracles
        .iter()
        .filter(|(kind, _)| matches!(kind, OracleKind::ExactValue))
        .count();
    let relational = oracles
        .iter()
        .filter(|(kind, _)| matches!(kind, OracleKind::RelationalCheck))
        .count();
    let smoke = oracles
        .iter()
        .filter(|(kind, _)| matches!(kind, OracleKind::SmokeOnly))
        .count();
    let broad_error = oracles
        .iter()
        .filter(|(kind, _)| matches!(kind, OracleKind::BroadError))
        .count();
    let exact_error = oracles
        .iter()
        .filter(|(kind, _)| matches!(kind, OracleKind::ExactErrorVariant))
        .count();
    let mock_expectations = oracles
        .iter()
        .filter(|(kind, _)| matches!(kind, OracleKind::MockExpectation))
        .count();
    if strong < 2 {
        return Err(format!(
            "expected exact-value oracles (assertEqual/assertDictEqual), got {:?}",
            oracles
        ));
    }
    if relational < 3 {
        return Err(format!(
            "expected relational oracles (assertNotEqual/assertIn/assertRegex), got {:?}",
            oracles
        ));
    }
    if smoke < 2 {
        return Err(format!(
            "expected assertTrue + assertFalse smoke-only oracles, got {:?}",
            oracles
        ));
    }
    if broad_error < 1 {
        return Err(format!(
            "expected assertRaises broad-error oracle, got {:?}",
            oracles
        ));
    }
    if exact_error < 1 {
        return Err(format!(
            "expected assertRaisesRegex exact-error oracle, got {:?}",
            oracles
        ));
    }
    if mock_expectations < 7 {
        return Err(format!(
            "expected seven mock expectation oracles, got {} (oracles: {:?})",
            mock_expectations, oracles
        ));
    }
    Ok(())
}

#[test]
fn oracle_for_assert_expr_falls_back_to_smoke_for_bare_name() -> Result<(), String> {
    let source = r#"
def test_bare_truthy():
    assert flag
"#;
    let oracles = assertion_oracles(source);
    let bare = oracles
        .first()
        .ok_or_else(|| "expected one assertion".to_string())?;
    if bare.0 != OracleKind::SmokeOnly || bare.1 != OracleStrength::Smoke {
        return Err(format!(
            "bare-name assertion should be SmokeOnly/Smoke, got {:?}",
            bare
        ));
    }
    Ok(())
}

#[test]
fn classify_probe_shape_covers_all_python_branches() {
    let predicate_cases = [
        "    elif amount > 0:",
        "    while remaining:",
        "    for entry in items:",
        "    match command:",
        "    case Cmd.Pay():",
    ];
    for line in predicate_cases {
        let (family, delta) = classify_probe_shape(line);
        assert_eq!(family, ProbeFamily::Predicate, "predicate for `{line}`");
        assert_eq!(delta, DeltaKind::Control, "predicate delta for `{line}`");
    }
    let error_cases = [
        "    raise",
        "    try:",
        "    except* RuntimeError:",
        "    finally:",
        "    with pytest.raises(ValueError):",
    ];
    for line in error_cases {
        let (family, delta) = classify_probe_shape(line);
        assert_eq!(family, ProbeFamily::ErrorPath, "error path for `{line}`");
        assert_eq!(delta, DeltaKind::Control, "error delta for `{line}`");
    }
    let (family, delta) = classify_probe_shape("    return");
    assert_eq!(family, ProbeFamily::ReturnValue);
    assert_eq!(delta, DeltaKind::Value);

    // assign whose RHS looks like a call -> side effect via Effect.
    let (family, delta) = classify_probe_shape("    handle = service.handler()");
    assert_eq!(family, ProbeFamily::SideEffect);
    assert_eq!(delta, DeltaKind::Effect);

    // `await call()` should classify as SideEffect via the await-strip
    // branch in `classify_probe_shape`.
    let (family, delta) = classify_probe_shape("    await pump.push(event)");
    assert_eq!(family, ProbeFamily::SideEffect);
    assert_eq!(delta, DeltaKind::Effect);

    // Plain string-only line falls into the conservative predicate default.
    let (family, delta) = classify_probe_shape("    \"docstring\"");
    assert_eq!(family, ProbeFamily::Predicate);
    assert_eq!(delta, DeltaKind::Control);

    // Mock initializer assignment ends with `)` and contains `Mock(`,
    // so it must take the SideEffect/Effect branch before the call
    // fallthrough.
    let (family, delta) = classify_probe_shape("    callback = Mock(name=\"sent\")");
    assert_eq!(family, ProbeFamily::SideEffect);
    assert_eq!(delta, DeltaKind::Effect);
}

#[test]
fn body_calls_owner_filters_comments_and_string_mentions() {
    let owner = PythonOwner {
        name: "apply_discount".to_string(),
        qualified_name: "apply_discount".to_string(),
        file: PathBuf::from("src/pricing.py"),
        start_line: 1,
        end_line: 5,
        owner_kind: Some(OwnerKind::Function),
        decorators: Vec::new(),
        imports: Vec::new(),
    };

    let comment_only = "    # apply_discount(100)\n    other()\n";
    assert!(
        !body_calls_owner(comment_only, &owner),
        "matches on commented-out call sites should be filtered"
    );

    let inside_string = "    note = \"call apply_discount(\"\n    other()\n";
    assert!(
        !body_calls_owner(inside_string, &owner),
        "matches inside an open string should be filtered"
    );

    let identifier_prefix = "    not_apply_discount(1)\n";
    assert!(
        !body_calls_owner(identifier_prefix, &owner),
        "identifier-prefixed names should not match call boundary"
    );

    let real = "    result = apply_discount(100)\n";
    assert!(
        body_calls_owner(real, &owner),
        "real top-level call should match"
    );
}

#[test]
fn has_unclosed_quote_handles_escapes_and_nested_quotes() {
    assert!(!has_unclosed_quote("\"closed\""));
    assert!(!has_unclosed_quote("'closed'"));
    assert!(has_unclosed_quote("\"open"));
    assert!(has_unclosed_quote("'open"));
    // Escape sequence inside a string keeps it closed.
    assert!(!has_unclosed_quote("\"escape \\\" here\""));
    // A double-quote inside a single-quoted string does not toggle.
    assert!(has_unclosed_quote("'still \"open"));
    // A backslash followed by nothing is consumed without panic.
    assert!(!has_unclosed_quote("\\"));
}

#[test]
fn contains_any_attribute_call_matches_arbitrary_receiver() {
    assert!(contains_any_attribute_call(
        "    order.apply_discount(100)\n",
        "apply_discount"
    ));
    assert!(!contains_any_attribute_call(
        "    # order.apply_discount(100)\n",
        "apply_discount"
    ));
}

#[test]
fn is_test_file_recognizes_file_and_directory_conventions() {
    assert!(is_test_file(Path::new("tests/foo.py")));
    assert!(is_test_file(Path::new("test/foo.py")));
    assert!(is_test_file(Path::new("src/test_thing.py")));
    assert!(is_test_file(Path::new("src/thing_test.py")));
    assert!(is_test_file(Path::new("nested/tests/sub/utility.py")));
    assert!(!is_test_file(Path::new("src/pricing.py")));
    assert!(!is_test_file(Path::new("src/testing.py")));
}

#[test]
fn is_unittest_class_accepts_bare_and_dotted_test_case_bases() -> Result<(), String> {
    let bare = extract_tests(
        Path::new("tests/test_bare.py"),
        "import unittest\nclass A(TestCase):\n    def test_a(self):\n        pass\n",
    );
    let dotted = extract_tests(
        Path::new("tests/test_dotted.py"),
        "import unittest\nclass B(unittest.TestCase):\n    def test_b(self):\n        pass\n",
    );
    let neither = extract_tests(
        Path::new("tests/test_neither.py"),
        "class C(object):\n    def test_c(self):\n        pass\n",
    );
    if bare.first().map(|t| t.framework) != Some("unittest") {
        return Err("bare `TestCase` base should mark unittest framework".to_string());
    }
    if dotted.first().map(|t| t.framework) != Some("unittest") {
        return Err("`unittest.TestCase` base should mark unittest framework".to_string());
    }
    if !neither.is_empty() {
        return Err("non-TestCase, non-Test* class should not be a pytest class".to_string());
    }
    Ok(())
}

#[test]
fn normalized_path_strips_dot_slash_prefix_and_normalizes_separators() {
    assert_eq!(normalized_path(Path::new("./src/foo.py")), "src/foo.py");
    assert_eq!(normalized_path(Path::new("src/foo.py")), "src/foo.py");
    // Forward slashes pass through.
    assert_eq!(
        normalized_path(Path::new("crates/ripr/src/lib.py")),
        "crates/ripr/src/lib.py"
    );
}

#[test]
fn text_for_range_clamps_out_of_bounds_offsets() {
    use rustpython_parser::text_size::{TextRange, TextSize};
    let source = "abc";
    let huge = TextRange::new(TextSize::from(10_u32), TextSize::from(99_u32));
    assert_eq!(text_for_range(source, huge), "");
    let partial = TextRange::new(TextSize::from(0_u32), TextSize::from(99_u32));
    assert_eq!(text_for_range(source, partial), "abc");
}

#[test]
fn line_for_offset_counts_newlines() {
    let source = "alpha\nbeta\ngamma";
    // Offset 0 is line 1.
    assert_eq!(line_for_offset(source, 0), 1);
    // Offset exactly on the newline stops before counting that newline.
    assert_eq!(line_for_offset(source, 5), 1);
    // Offset immediately after the newline counts the next segment as line 2.
    assert_eq!(line_for_offset(source, 6), 2);
    // Offset on the second segment is line 2.
    assert_eq!(line_for_offset(source, 7), 2);
    // Offset past end stops at the last counted line.
    assert_eq!(line_for_offset(source, 999), 3);
}

#[test]
fn looks_like_call_expression_handles_trailing_semicolons_and_whitespace() {
    assert!(looks_like_call_expression("notify(event);"));
    assert!(looks_like_call_expression("notify(event)   "));
    assert!(!looks_like_call_expression("notify"));
    assert!(!looks_like_call_expression("notify("));
}

#[test]
fn contains_mock_initializer_recognizes_both_constructors() {
    assert!(contains_mock_initializer("callback = Mock(name='x')"));
    assert!(contains_mock_initializer("callback = MagicMock(name='y')"));
    assert!(!contains_mock_initializer("notify(payload)"));
}

#[test]
fn is_known_mock_constructor_import_matches_imported_and_aliased() {
    let imported = PythonImport {
        imported: "Mock".to_string(),
        alias: "Mock".to_string(),
    };
    let aliased = PythonImport {
        imported: "MagicMock".to_string(),
        alias: "MM".to_string(),
    };
    let alias_only = PythonImport {
        imported: "Other".to_string(),
        alias: "Mock".to_string(),
    };
    let unrelated = PythonImport {
        imported: "json".to_string(),
        alias: "json".to_string(),
    };
    assert!(is_known_mock_constructor_import(&imported));
    assert!(is_known_mock_constructor_import(&aliased));
    assert!(is_known_mock_constructor_import(&alias_only));
    assert!(!is_known_mock_constructor_import(&unrelated));
}

#[test]
fn static_limit_detects_monkeypatch_setitem_and_delattr() -> Result<(), String> {
    let owner = extract_owners(Path::new("src/service.py"), "def total():\n    return 1\n")
        .into_iter()
        .next()
        .ok_or_else(|| "missing owner".to_string())?;
    let setitem_tests = extract_tests(
        Path::new("tests/test_setitem.py"),
        "from src.service import total\n\ndef test_total(monkeypatch):\n    monkeypatch.setitem({}, \"key\", lambda: 1)\n    assert total() == 1\n",
    );
    let delattr_tests = extract_tests(
        Path::new("tests/test_delattr.py"),
        "from src.service import total\n\ndef test_total(monkeypatch):\n    monkeypatch.delattr(\"src.service.helper\")\n    assert total() == 1\n",
    );
    let setitem_candidates = related_test_candidates(&owner, &setitem_tests);
    let delattr_candidates = related_test_candidates(&owner, &delattr_tests);
    let setitem = static_limit_for_change("    return total()", &owner, &setitem_candidates)
        .ok_or_else(|| "expected MockedModule for monkeypatch.setitem".to_string())?;
    let delattr = static_limit_for_change("    return total()", &owner, &delattr_candidates)
        .ok_or_else(|| "expected MockedModule for monkeypatch.delattr".to_string())?;
    if setitem.kind != StaticLimitKind::MockedModule {
        return Err(format!(
            "expected MockedModule for monkeypatch.setitem, got {:?}",
            setitem.kind
        ));
    }
    if delattr.kind != StaticLimitKind::MockedModule {
        return Err(format!(
            "expected MockedModule for monkeypatch.delattr, got {:?}",
            delattr.kind
        ));
    }
    Ok(())
}

#[test]
fn static_limit_returns_none_when_no_limits_apply() -> Result<(), String> {
    let owner = extract_owners(Path::new("src/service.py"), "def total():\n    return 1\n")
        .into_iter()
        .next()
        .ok_or_else(|| "missing owner".to_string())?;
    assert!(
        static_limit_for_change("    return 1", &owner, &[]).is_none(),
        "plain return without indirection should not raise a static_limit"
    );
    Ok(())
}

#[test]
fn static_limit_picks_first_non_transparent_decorator() -> Result<(), String> {
    // The owner has `@staticmethod` (transparent) followed by a
    // non-transparent decorator. The static-limit picker must skip
    // the transparent one and report the non-transparent one.
    let owner = extract_owners(
            Path::new("src/service.py"),
            "class Service:\n    @staticmethod\n    @retry(times=3)\n    def total():\n        return 1\n",
        )
        .into_iter()
        .next()
        .ok_or_else(|| "missing owner".to_string())?;
    let limit = static_limit_for_change("    return 1", &owner, &[])
        .ok_or_else(|| "expected DecoratorIndirection limit".to_string())?;
    if limit.kind != StaticLimitKind::DecoratorIndirection {
        return Err(format!(
            "expected DecoratorIndirection, got {:?}",
            limit.kind
        ));
    }
    if !limit.evidence.contains("retry") {
        return Err(format!(
            "expected evidence to name the `retry` decorator, got {}",
            limit.evidence
        ));
    }
    Ok(())
}

#[test]
fn imported_module_matches_owner_compares_last_segment_to_owner_stem() {
    let owner = PythonOwner {
        name: "apply_discount".to_string(),
        qualified_name: "apply_discount".to_string(),
        file: PathBuf::from("src/pricing.py"),
        start_line: 1,
        end_line: 1,
        owner_kind: Some(OwnerKind::Function),
        decorators: Vec::new(),
        imports: Vec::new(),
    };
    let dotted = PythonImport {
        imported: "src.pricing".to_string(),
        alias: "pricing".to_string(),
    };
    let plain = PythonImport {
        imported: "pricing".to_string(),
        alias: "pricing".to_string(),
    };
    let mismatched = PythonImport {
        imported: "src.tax".to_string(),
        alias: "tax".to_string(),
    };
    assert!(imported_module_matches_owner(&dotted, &owner));
    assert!(imported_module_matches_owner(&plain, &owner));
    assert!(!imported_module_matches_owner(&mismatched, &owner));
}

#[test]
fn same_stem_related_handles_missing_stems() {
    let owner = PythonOwner {
        name: "apply_discount".to_string(),
        qualified_name: "apply_discount".to_string(),
        file: PathBuf::from(""),
        start_line: 1,
        end_line: 1,
        owner_kind: Some(OwnerKind::Function),
        decorators: Vec::new(),
        imports: Vec::new(),
    };
    let test = PythonTest {
        name: "test_x".to_string(),
        qualified_name: "test_x".to_string(),
        file: PathBuf::from("tests/test_pricing.py"),
        line: 1,
        body_text: String::new(),
        imports: Vec::new(),
        decorators: Vec::new(),
        fixtures: Vec::new(),
        parametrized: false,
        framework: "pytest",
        assertions: Vec::new(),
    };
    // An owner with no file stem cannot match by stem.
    assert!(!same_stem_related(&test, &owner));
}

#[test]
fn classify_change_returns_none_when_line_outside_any_owner() {
    let owners = extract_owners(
        Path::new("src/pricing.py"),
        "def apply_discount(amount):\n    return amount - 10\n",
    );
    let tests = Vec::new();
    let finding = classify_change(
        Path::new("src/pricing.py"),
        999,
        "    return amount - 10",
        &owners,
        &tests,
    );
    assert!(finding.is_none());
}

#[test]
fn classify_change_returns_none_for_different_file_owners() {
    let owners = extract_owners(
        Path::new("src/pricing.py"),
        "def apply_discount(amount):\n    return amount - 10\n",
    );
    let tests = Vec::new();
    let finding = classify_change(
        Path::new("src/elsewhere.py"),
        2,
        "    return amount - 10",
        &owners,
        &tests,
    );
    assert!(finding.is_none());
}

#[test]
fn analyze_diff_emits_finding_for_changed_python_file_on_disk() -> Result<(), String> {
    let root = unique_tempdir("analyze-diff-finding")?;
    let production_rel = PathBuf::from("src/pricing.py");
    let test_rel = PathBuf::from("tests/test_pricing.py");
    write_file(
        &root.join(&production_rel),
        "def apply_discount(amount):\n    if amount >= 100:\n        return amount - 10\n    return amount\n",
    )?;
    write_file(
        &root.join(&test_rel),
        "from src.pricing import apply_discount\n\ndef test_apply_discount():\n    assert apply_discount(100) == 90\n",
    )?;

    let adapter = PythonAdapter;
    let options = AnalysisOptions {
        root: root.clone(),
        base: None,
        diff_file: None,
        mode: crate::analysis::AnalysisMode::Draft,
        include_unchanged_tests: false,
    };
    let policy = OraclePolicy::default();
    let changed_files = vec![
        ChangedFile {
            path: production_rel.clone(),
            added_lines: vec![crate::analysis::diff::ChangedLine {
                line: 2,
                text: "    if amount >= 100:".to_string(),
            }],
            removed_lines: Vec::new(),
        },
        // Test files in the diff are accepted-but-skipped for findings;
        // they still count toward `changed_files`.
        ChangedFile {
            path: test_rel.clone(),
            added_lines: vec![crate::analysis::diff::ChangedLine {
                line: 1,
                text: "from src.pricing import apply_discount".to_string(),
            }],
            removed_lines: Vec::new(),
        },
        // Non-python files should not be counted.
        ChangedFile {
            path: PathBuf::from("README.md"),
            added_lines: Vec::new(),
            removed_lines: Vec::new(),
        },
    ];

    let result = adapter.analyze_diff(&options, &policy, &changed_files);
    // Always try to clean up the tempdir before bubbling errors.
    let cleanup = std::fs::remove_dir_all(&root);
    let result = result?;
    cleanup.map_err(|err| format!("remove_dir_all({}): {err}", root.display()))?;

    if result.changed_files != 2 {
        return Err(format!(
            "expected two accepted changed files (production + test), got {}",
            result.changed_files
        ));
    }
    if result.findings.len() != 1 {
        return Err(format!(
            "expected exactly one finding from the production diff line, got {}",
            result.findings.len()
        ));
    }
    let finding = &result.findings[0];
    if finding.class != ExposureClass::Exposed {
        return Err(format!(
            "expected an exposed finding when the related test has a strong oracle, got {:?}",
            finding.class
        ));
    }
    if finding.language != Some(DomainLanguageId::Python) {
        return Err("language metadata should be Python".to_string());
    }
    if finding.language_status != Some(LanguageStatus::Preview) {
        return Err("language status should be Preview".to_string());
    }
    Ok(())
}

#[test]
fn analyze_diff_skips_detectable_generated_python_files() -> Result<(), String> {
    let root = unique_tempdir("analyze-diff-generated-file")?;
    let generated_rel = PathBuf::from("src/schema_pb2.py");
    write_file(
        &root.join(&generated_rel),
        "def encode_status(status):\n    return {'status': status, 'version': 2}\n",
    )?;
    write_file(
        &root.join("tests/test_schema.py"),
        "from src.schema_pb2 import encode_status\n\n\
def test_encode_status():\n    assert encode_status('paid')['status'] == 'paid'\n",
    )?;

    let adapter = PythonAdapter;
    let options = AnalysisOptions {
        root: root.clone(),
        base: None,
        diff_file: None,
        mode: crate::analysis::AnalysisMode::Draft,
        include_unchanged_tests: false,
    };
    let policy = OraclePolicy::default();
    let changed_files = vec![ChangedFile {
        path: generated_rel,
        added_lines: vec![crate::analysis::diff::ChangedLine {
            line: 2,
            text: "    return {'status': status, 'version': 2}".to_string(),
        }],
        removed_lines: Vec::new(),
    }];

    let result = adapter.analyze_diff(&options, &policy, &changed_files);
    let cleanup = std::fs::remove_dir_all(&root);
    let result = result?;
    cleanup.map_err(|err| format!("remove_dir_all({}): {err}", root.display()))?;

    if result.changed_files != 0 {
        return Err(format!(
            "expected generated Python diff to be excluded from changed files, got {}",
            result.changed_files
        ));
    }
    if !result.findings.is_empty() {
        return Err(format!(
            "expected generated Python diff to emit no preview findings, got {}",
            result.findings.len()
        ));
    }
    Ok(())
}

#[test]
fn collect_workspace_python_files_skips_excluded_directories() -> Result<(), String> {
    let root = unique_tempdir("workspace-walk")?;
    let included = [
        PathBuf::from("src/keep.py"),
        PathBuf::from("nested/also_keep.py"),
    ];
    let excluded = [
        PathBuf::from(".git/skip.py"),
        PathBuf::from("target/skip.py"),
        PathBuf::from("node_modules/skip.py"),
        PathBuf::from(".ripr/skip.py"),
        PathBuf::from(".direnv/skip.py"),
        PathBuf::from("__pycache__/skip.py"),
        PathBuf::from(".venv/skip.py"),
        PathBuf::from("venv/skip.py"),
        PathBuf::from("env/skip.py"),
        PathBuf::from(".tox/skip.py"),
        PathBuf::from(".nox/skip.py"),
        PathBuf::from("site-packages/skip.py"),
        PathBuf::from(".pytest_cache/skip.py"),
        PathBuf::from(".mypy_cache/skip.py"),
        PathBuf::from("dist/skip.py"),
        PathBuf::from("build/skip.py"),
        PathBuf::from("src/generated_client.py"),
        PathBuf::from("src/schema_pb2.py"),
        PathBuf::from("src/schema_pb2_grpc.py"),
        PathBuf::from("src/client.generated.py"),
        PathBuf::from("src/client_generated.py"),
        // Not python -> filtered by `accepts_path`.
        PathBuf::from("src/keep.rs"),
        PathBuf::from("docs/README.md"),
    ];
    for rel in included.iter().chain(excluded.iter()) {
        write_file(&root.join(rel), "x = 1\n")?;
    }

    let files = collect_workspace_python_files(&root);
    let cleanup = std::fs::remove_dir_all(&root);

    for expected in included.iter() {
        if !files.iter().any(|path| path == expected) {
            let _ = cleanup;
            return Err(format!(
                "expected workspace walker to include {} (got {:?})",
                expected.display(),
                files
            ));
        }
    }
    let still_present_excluded: Vec<_> = excluded
        .iter()
        .filter(|expected| files.iter().any(|path| path == *expected))
        .collect();
    cleanup.map_err(|err| format!("remove_dir_all({}): {err}", root.display()))?;
    if !still_present_excluded.is_empty() {
        return Err(format!(
            "workspace walker should skip excluded paths but included {:?}",
            still_present_excluded
        ));
    }
    Ok(())
}

#[test]
fn collect_workspace_python_files_returns_empty_for_missing_root() {
    let missing = PathBuf::from(format!(
        "/tmp/ripr-python-coverage-missing-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    ));
    assert!(collect_workspace_python_files(&missing).is_empty());
}

#[test]
fn related_test_matching_falls_back_to_same_stem_when_no_call() {
    let owners = extract_owners(
        Path::new("src/pricing.py"),
        "def apply_discount(amount):\n    return amount - 10\n",
    );
    let tests = extract_tests(
        Path::new("tests/test_pricing.py"),
        "def test_unrelated():\n    do_something_else()\n",
    );
    let candidates = related_test_candidates(&owners[0], &tests);
    assert_eq!(candidates.len(), 1);
    assert_eq!(
        candidates[0].relation,
        PythonRelationKind::SameStem,
        "same-stem proximity should kick in when no direct or import-alias call is seen"
    );
}

#[test]
fn related_test_matching_uses_test_name_similarity_as_uncertain_relation() -> Result<(), String> {
    let owner_file = Path::new("src/pricing.py");
    let owners = extract_owners(
        owner_file,
        "def apply_discount(amount):\n    return amount - 10\n",
    );
    let tests = extract_tests(
        Path::new("tests/test_checkout.py"),
        "def test_apply_discount_boundary_case():\n    assert 90 == 90\n",
    );
    let candidates = related_test_candidates(&owners[0], &tests);
    if candidates.len() != 1 {
        return Err(format!(
            "expected one name-similarity candidate, got {}",
            candidates.len()
        ));
    }
    assert_eq!(
        candidates[0].relation,
        PythonRelationKind::TestNameSimilarity
    );
    let finding = classify_change(owner_file, 2, "    return amount - 10", &owners, &tests)
        .ok_or_else(|| "expected classification".to_string())?;
    assert!(finding.evidence.iter().any(|item| item
        == "related_test_relation: test_name_similarity (test_apply_discount_boundary_case)"));
    assert!(finding.evidence.iter().any(|item| item
        == "related_test_uncertain: test_name_similarity (test_apply_discount_boundary_case)"));
    assert_eq!(finding.related_tests[0].oracle_kind, OracleKind::Unknown);
    Ok(())
}

#[test]
fn related_test_matching_uses_fixture_name_as_uncertain_relation() {
    let owners = extract_owners(
        Path::new("src/pricing.py"),
        "def calculate_fee(amount):\n    return amount + 2\n",
    );
    let tests = extract_tests(
        Path::new("tests/test_checkout.py"),
        "def test_checkout_total(pricing):\n    assert 102 == 102\n",
    );
    let candidates = related_test_candidates(&owners[0], &tests);
    assert_eq!(candidates.len(), 1);
    assert_eq!(candidates[0].relation, PythonRelationKind::FixtureName);
}

#[test]
fn owner_similarity_keys_cover_qualified_names_modules_and_token_boundaries() -> Result<(), String>
{
    let owners = extract_owners(
        Path::new("src/users.py"),
        "class User:\n    def activate(self):\n        return True\n",
    );
    let method_owner = owners
        .iter()
        .find(|owner| owner.qualified_name == "User.activate")
        .ok_or_else(|| "missing method owner".to_string())?;
    let method_keys = owner_similarity_keys(method_owner);
    assert!(method_keys.contains(&"activate".to_string()));
    assert!(method_keys.contains(&"user_activate".to_string()));
    assert!(method_keys.contains(&"users".to_string()));

    let module_owner = owners
        .iter()
        .find(|owner| owner.qualified_name == "<module>")
        .ok_or_else(|| "missing module owner".to_string())?;
    assert_eq!(
        owner_similarity_keys(module_owner),
        vec!["users".to_string()]
    );

    assert_eq!(
        normalize_similarity_key("__Apply Discount!!"),
        "apply_discount"
    );
    assert!(similarity_key_contains(
        "test_apply_discount_boundary",
        "apply_discount"
    ));
    assert!(similarity_key_contains(
        "apply_discount_boundary",
        "apply_discount"
    ));
    assert!(similarity_key_contains(
        "boundary_apply_discount",
        "apply_discount"
    ));
    assert!(similarity_key_contains("apply_discount", "apply_discount"));
    assert!(!similarity_key_contains(
        "reapply_discounted",
        "apply_discount"
    ));
    assert!(!similarity_key_contains("", "apply_discount"));
    Ok(())
}

#[test]
fn canonical_python_gap_id_is_stable_across_line_movement_and_expression_spacing()
-> Result<(), String> {
    let owner = extract_owners(
        Path::new("src/pricing.py"),
        "def apply_discount(amount, threshold):\n    if amount >= threshold:\n        return amount - 10\n    return amount\n",
    )
    .into_iter()
    .find(|owner| owner.qualified_name == "apply_discount")
    .ok_or_else(|| "missing apply_discount owner".to_string())?;

    let before = canonical_python_gap_for(
        Path::new("src/pricing.py"),
        &owner,
        &ProbeFamily::Predicate,
        "    if amount >= threshold:",
    );
    let after = canonical_python_gap_for(
        Path::new("src/pricing.py"),
        &owner,
        &ProbeFamily::Predicate,
        "        if (amount) >= threshold:",
    );

    assert_eq!(before.id, after.id);
    assert_eq!(
        before.id,
        "gap:python:src/pricing.py:apply_discount:predicate_boundary:predicate:amount>=threshold"
    );
    assert_eq!(before.language, "python");
    assert_eq!(before.file, "src/pricing.py");
    assert_eq!(before.owner, "apply_discount");
    assert_eq!(before.behavior_kind, "predicate_boundary");
    assert_eq!(before.probe_kind, "predicate");
    assert_eq!(before.normalized_discriminator, "amount>=threshold");
    Ok(())
}

#[test]
fn canonical_python_gap_id_distinguishes_owner_behavior_and_discriminator() -> Result<(), String> {
    let pricing_owner = extract_owners(
        Path::new("src/pricing.py"),
        "def apply_discount(amount, threshold):\n    return amount - 10\n",
    )
    .remove(0);
    let billing_owner = extract_owners(
        Path::new("src/billing.py"),
        "def apply_discount(amount, threshold):\n    return amount - 10\n",
    )
    .remove(0);

    let pricing_return = canonical_python_gap_for(
        Path::new("src/pricing.py"),
        &pricing_owner,
        &ProbeFamily::ReturnValue,
        "    return amount - 10",
    );
    let pricing_error = canonical_python_gap_for(
        Path::new("src/pricing.py"),
        &pricing_owner,
        &ProbeFamily::ErrorPath,
        "    raise ValueError(\"invalid coupon\")",
    );
    let billing_return = canonical_python_gap_for(
        Path::new("src/billing.py"),
        &billing_owner,
        &ProbeFamily::ReturnValue,
        "    return amount - 10",
    );

    assert_ne!(pricing_return.id, pricing_error.id);
    assert_ne!(pricing_return.id, billing_return.id);
    assert_eq!(pricing_return.behavior_kind, "return_value");
    assert_eq!(pricing_return.normalized_discriminator, "amount-10");
    assert_eq!(pricing_error.behavior_kind, "exception_path");
    assert_eq!(
        pricing_error.normalized_discriminator,
        "valueerror_invalid_coupon"
    );
    Ok(())
}

#[test]
fn extract_owners_returns_empty_when_source_is_unparseable() {
    let owners = extract_owners(Path::new("src/oops.py"), "def !!!");
    assert!(owners.is_empty());
}

#[test]
fn extract_tests_returns_empty_when_source_is_unparseable() {
    let tests = extract_tests(Path::new("tests/test_oops.py"), "def !!!");
    assert!(tests.is_empty());
}

#[test]
fn related_test_candidates_break_ties_by_oracle_then_file_then_name() {
    // All three tests share the same relation rank (same_stem), so the
    // sort cascades into the assertion-rank tie-breaker and then into
    // the file/name fallbacks.
    let owner = extract_owners(
        Path::new("src/pricing.py"),
        "def apply_discount(amount):\n    return amount - 10\n",
    )
    .remove(0);
    let mut tests = extract_tests(
        Path::new("tests/test_pricing.py"),
        "def test_alpha():\n    assert 1 == 1\n\ndef test_beta():\n    assert 1 == 1\n",
    );
    tests.extend(extract_tests(
        Path::new("tests/pricing_test.py"),
        "def test_alpha():\n    assert 1 == 1\n",
    ));
    let candidates = related_test_candidates(&owner, &tests);
    assert!(
        candidates.len() >= 3,
        "expected at least three same-stem candidates, got {}",
        candidates.len()
    );
    // Sort must be deterministic across runs.
    let first_pass: Vec<(String, String)> = candidates
        .iter()
        .map(|candidate| {
            (
                candidate.test.file.display().to_string(),
                candidate.test.name.clone(),
            )
        })
        .collect();
    let second_pass = related_test_candidates(&owner, &tests);
    let second_keys: Vec<(String, String)> = second_pass
        .iter()
        .map(|candidate| {
            (
                candidate.test.file.display().to_string(),
                candidate.test.name.clone(),
            )
        })
        .collect();
    assert_eq!(first_pass, second_keys, "sort must be stable across runs");
}

#[test]
fn find_related_tests_marks_parametrized_test_when_no_assertion_extracted() -> Result<(), String> {
    // A parametrized test whose body calls the owner but contains no
    // assertion at all should fall through to the parametrize-marker
    // oracle text in `find_related_tests`.
    let owner = extract_owners(
        Path::new("src/pricing.py"),
        "def apply_discount(amount):\n    return amount - 10\n",
    )
    .remove(0);
    let tests = extract_tests(
        Path::new("tests/test_pricing.py"),
        r#"
import pytest

@pytest.mark.parametrize("amount", [1, 2])
def test_apply_discount(amount):
    apply_discount(amount)
"#,
    );
    let related = find_related_tests(&owner, &tests);
    if related.len() != 1 {
        return Err(format!(
            "expected one related test for parametrized matcher, got {}",
            related.len()
        ));
    }
    if related[0].oracle.as_deref() != Some("pytest.mark.parametrize") {
        return Err(format!(
            "expected parametrize-marker oracle text, got {:?}",
            related[0].oracle
        ));
    }
    if related[0].oracle_kind != OracleKind::Unknown {
        return Err(format!(
            "parametrize fallback should keep Unknown oracle kind, got {:?}",
            related[0].oracle_kind
        ));
    }
    Ok(())
}

#[test]
fn test_has_mocked_module_recognizes_dotted_patch_decorator() {
    let mocked = PythonTest {
        name: "test_x".to_string(),
        qualified_name: "test_x".to_string(),
        file: PathBuf::from("tests/test_x.py"),
        line: 1,
        body_text: String::new(),
        imports: Vec::new(),
        // Dotted form like `@mock.patch(...)` must satisfy the
        // `decorator.ends_with(".patch")` branch.
        decorators: vec!["mock.patch".to_string()],
        fixtures: Vec::new(),
        parametrized: false,
        framework: "pytest",
        assertions: Vec::new(),
    };
    assert!(test_has_mocked_module(&mocked));
    let bare = PythonTest {
        name: "test_y".to_string(),
        qualified_name: "test_y".to_string(),
        file: PathBuf::from("tests/test_y.py"),
        line: 1,
        body_text: String::new(),
        imports: Vec::new(),
        decorators: vec!["patch".to_string()],
        fixtures: Vec::new(),
        parametrized: false,
        framework: "pytest",
        assertions: Vec::new(),
    };
    assert!(test_has_mocked_module(&bare));
    let clean = PythonTest {
        name: "test_z".to_string(),
        qualified_name: "test_z".to_string(),
        file: PathBuf::from("tests/test_z.py"),
        line: 1,
        body_text: String::new(),
        imports: Vec::new(),
        decorators: vec!["pytest.mark.skip".to_string()],
        fixtures: Vec::new(),
        parametrized: false,
        framework: "pytest",
        assertions: Vec::new(),
    };
    assert!(!test_has_mocked_module(&clean));
}

#[test]
fn contains_dynamic_dispatch_detects_registry_indexed_call() {
    // Hits the second branch of `contains_dynamic_dispatch` - a
    // bracketed index followed by an open-paren `]( ` - which the
    // existing fixture only exercised via `getattr(`.
    assert!(contains_dynamic_dispatch("    return registry[key]()"));
    assert!(!contains_dynamic_dispatch("    return registry[key]"));
    assert!(!contains_dynamic_dispatch("    return notify()"));
}

#[test]
fn contains_dynamic_import_detects_runtime_import_calls() {
    assert!(contains_dynamic_import(
        "    return importlib.import_module(module_name).handle(payload)"
    ));
    assert!(contains_dynamic_import(
        "    return __import__(module_name).handle(payload)"
    ));
    assert!(contains_dynamic_import(
        "    return __import__ (module_name).handle(payload)"
    ));
    assert!(!contains_dynamic_import(
        "    return imported_module.handle(payload)"
    ));
    assert!(!contains_dynamic_import(
        "    note = \"call importlib.import_module(module_name)\""
    ));
    assert!(!contains_dynamic_import(
        "    return payload  # __import__(module_name)"
    ));
    assert!(!contains_dynamic_import(
        "    return not_importlib.import_module(module_name)"
    ));
}

#[test]
fn looks_like_call_expression_rejects_text_without_parens() {
    assert!(!looks_like_call_expression(""));
    assert!(!looks_like_call_expression("name"));
    // Trailing whitespace between identifier and `(` keeps the
    // shape from looking like a real call expression.
    assert!(!looks_like_call_expression("name ("));
}

#[test]
fn classify_change_emits_decorator_evidence_when_owner_has_decorator() -> Result<(), String> {
    // The owner has a non-transparent decorator. The classifier
    // should surface `owner_decorators: ...` in the evidence list,
    // which exercises the `if !owner.decorators.is_empty()` branch.
    let owners = extract_owners(
        Path::new("src/service.py"),
        "@retry(times=3)\ndef total():\n    return 1\n",
    );
    let tests = extract_tests(
        Path::new("tests/test_service.py"),
        "def test_total():\n    assert total() == 1\n",
    );
    let finding = classify_change(
        Path::new("src/service.py"),
        2,
        "    return 1",
        &owners,
        &tests,
    )
    .ok_or_else(|| "expected a finding".to_string())?;
    let evidence_joined = finding.evidence.join("\n");
    if !evidence_joined.contains("owner_decorators: ") {
        return Err(format!(
            "expected owner_decorators evidence line, got: {evidence_joined}"
        ));
    }
    if !evidence_joined.contains("retry") {
        return Err(format!(
            "expected `retry` decorator to be listed, got: {evidence_joined}"
        ));
    }
    if finding.canonical_gap.is_some() {
        return Err("static-limit Python findings should not carry canonical gaps yet".to_string());
    }
    Ok(())
}

#[test]
fn classify_change_emits_pytest_repair_evidence() -> Result<(), String> {
    let owners = extract_owners(
        Path::new("src/checkout.py"),
        "def checkout():\n    return Response(422)\n",
    );
    let tests = extract_tests(
        Path::new("tests/test_checkout.py"),
        r#"
import pytest

@pytest.mark.parametrize("coupon", ["expired"])
def test_checkout_expired_coupon(client, caplog, coupon):
    response = checkout()
    assert response.status_code == 422
"#,
    );
    let finding = classify_change(
        Path::new("src/checkout.py"),
        2,
        "    return Response(422)",
        &owners,
        &tests,
    )
    .ok_or_else(|| "expected a finding".to_string())?;
    let evidence_joined = finding.evidence.join("\n");
    for expected in [
        "test_fixtures: caplog, client, coupon",
        "test_parametrized: pytest",
        "test_oracle_shape: status_code_assertion",
    ] {
        if !evidence_joined.contains(expected) {
            return Err(format!(
                "expected evidence `{expected}`, got: {evidence_joined}"
            ));
        }
    }
    Ok(())
}

#[test]
fn oracle_for_call_returns_none_for_unknown_callable() -> Result<(), String> {
    // The `_ => None` arm of `oracle_for_call` is the harness for
    // every non-oracle call shape inside a test body.
    let tests = extract_tests(
        Path::new("tests/test_unknown.py"),
        "def test_unknown_call():\n    something_random(payload)\n",
    );
    if tests.len() != 1 {
        return Err(format!("expected single test, got {}", tests.len()));
    }
    if !tests[0].assertions.is_empty() {
        return Err(format!(
            "non-oracle calls must not register as assertions, got {:?}",
            tests[0].assertions
        ));
    }
    Ok(())
}

#[test]
fn assertion_from_expr_returns_none_for_non_call_expressions() -> Result<(), String> {
    // A bare expression statement that is not a call must not become
    // an assertion. Use a name reference like `value` to drive the
    // `Expr::Call(call) else return None` branch of `assertion_from_expr`.
    let tests = extract_tests(
        Path::new("tests/test_bare.py"),
        "def test_bare_expression():\n    value\n    other\n",
    );
    if tests.len() != 1 {
        return Err(format!("expected single test, got {}", tests.len()));
    }
    if !tests[0].assertions.is_empty() {
        return Err(format!(
            "expression statements without calls must not assert, got {:?}",
            tests[0].assertions
        ));
    }
    Ok(())
}

#[test]
fn visit_workspace_returns_silently_when_directory_is_unreadable() {
    // Pointing visit_workspace at a non-existent directory must hit
    // the `Err(_) => return;` early exit without panicking.
    let mut out = Vec::new();
    visit_workspace(
        Path::new("/definitely-not-a-real-dir-ripr"),
        Path::new("/definitely-not-a-real-dir-ripr"),
        &mut out,
    );
    assert!(out.is_empty());
}

#[test]
fn async_test_inside_unittest_class_is_marked_as_unittest_framework() -> Result<(), String> {
    // The `async def test_*` arm of `collect_tests_from_statements`
    // has a branch where the test is inside a unittest.TestCase
    // class. Exercising it pins the `framework: "unittest"` literal
    // for async tests.
    let tests = extract_tests(
        Path::new("tests/test_async_unittest.py"),
        r#"
import unittest

class Async(unittest.TestCase):
    async def test_async_path(self):
        self.assertEqual(await compute(), 1)
"#,
    );
    let async_test = tests
        .iter()
        .find(|test| test.name == "test_async_path")
        .ok_or_else(|| {
            format!(
                "expected `test_async_path`, got names {:?}",
                tests.iter().map(|t| t.name.as_str()).collect::<Vec<_>>()
            )
        })?;
    if async_test.framework != "unittest" {
        return Err(format!(
            "async test inside unittest.TestCase should be unittest, got {}",
            async_test.framework
        ));
    }
    Ok(())
}

#[test]
fn expr_full_name_returns_none_for_unsupported_decorator_shapes() {
    // A `parametrize` decorator whose target uses subscript syntax
    // like `pytest.mark.parametrize["int"]` is not a Name / Attribute
    // / Call. `decorator_names` should silently drop it via the
    // `_ => None` arm in `expr_full_name`, leaving the test's
    // recognized decorator list empty.
    let tests = extract_tests(
        Path::new("tests/test_unsupported_decorator.py"),
        r#"
import pytest

@pytest.mark.parametrize[int]("amount", [1])
def test_apply_discount(amount):
    apply_discount(amount)
"#,
    );
    // The decorator should be silently filtered out - meaning the
    // test does not get marked as parametrized via the recognized
    // shapes.
    if let Some(test) = tests.first() {
        assert!(
            !test
                .decorators
                .iter()
                .any(|decorator| decorator.contains("parametrize")),
            "subscript decorator shape should not yield a parametrize name; got {:?}",
            test.decorators
        );
    }
}

#[test]
fn line_uses_imported_symbol_matches_attribute_access_on_imported_alias() {
    let symbol = PythonImport {
        imported: "logger".to_string(),
        alias: "log".to_string(),
    };
    // `log.warn(...)` exercises the `text.contains("{}.")` arm of
    // `line_uses_imported_symbol`, since the `(` form follows the
    // attribute access only after a dot.
    let imports = vec![symbol];
    assert!(line_uses_imported_symbol(
        "    log.warn(\"problem\")",
        &imports
    ));
    // A bare identifier with no dot and no call form should not match.
    assert!(!line_uses_imported_symbol("    unrelated", &imports));
}

#[test]
fn classify_probe_shape_assign_with_non_call_rhs_falls_through_to_predicate() {
    // `total = amount + 10` is an assignment whose LHS is a plain
    // identifier and whose RHS does not look like a call. The
    // classifier must fall through past the assign branches to the
    // conservative predicate default.
    let (family, delta) = classify_probe_shape("    total = amount + 10");
    assert_eq!(family, ProbeFamily::Predicate);
    assert_eq!(delta, DeltaKind::Control);
}

#[test]
fn analyze_diff_counts_python_file_but_skips_unreadable_workspace_source() -> Result<(), String> {
    // The walker enumerates a `.py` file we never create on disk.
    // `std::fs::read_to_string` must fail and trigger the
    // `Err(_) => continue;` branch in `analyze_diff`, while the
    // accepted changed-file count keeps growing.
    let root = unique_tempdir("analyze-diff-unreadable")?;
    // Create one valid file and one entry that is a directory rather
    // than a file with a `.py` extension. The directory cannot be
    // read as a source file, so `read_to_string` errors and the
    // workspace loop continues without any owners/tests being
    // collected from it.
    write_file(&root.join("src/keep.py"), "def keep():\n    return 1\n")?;
    let unreadable = root.join("src/looks_like_source.py");
    std::fs::create_dir_all(&unreadable)
        .map_err(|err| format!("create_dir_all({}): {err}", unreadable.display()))?;

    let adapter = PythonAdapter;
    let options = AnalysisOptions {
        root: root.clone(),
        base: None,
        diff_file: None,
        mode: crate::analysis::AnalysisMode::Draft,
        include_unchanged_tests: false,
    };
    let policy = OraclePolicy::default();
    let changed_files = vec![ChangedFile {
        path: PathBuf::from("src/keep.py"),
        added_lines: vec![crate::analysis::diff::ChangedLine {
            line: 2,
            text: "    return 1".to_string(),
        }],
        removed_lines: Vec::new(),
    }];
    let result = adapter.analyze_diff(&options, &policy, &changed_files);
    let cleanup = std::fs::remove_dir_all(&root);
    let result = result?;
    cleanup.map_err(|err| format!("remove_dir_all({}): {err}", root.display()))?;

    if result.changed_files != 1 {
        return Err(format!(
            "expected 1 accepted changed file, got {}",
            result.changed_files
        ));
    }
    // No test exists, so the lone production change must produce a
    // NoStaticPath finding.
    if result.findings.len() != 1 {
        return Err(format!(
            "expected one NoStaticPath finding, got {} findings",
            result.findings.len()
        ));
    }
    if result.findings[0].class != ExposureClass::NoStaticPath {
        return Err(format!(
            "expected NoStaticPath, got {:?}",
            result.findings[0].class
        ));
    }
    Ok(())
}
