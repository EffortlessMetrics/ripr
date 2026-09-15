//! Behavior suite for the Python preview adapter.
//!
//! Moved verbatim from the former inline `mod tests` in `python.rs`.
//! Covers: test-framework detection, `parse_source` acceptance, owner and
//! test harvesting, probe-shape and constructor/return-field
//! discriminators, class/method/free-function identity semantics,
//! changed-sink delta rules, annotation-only and docstring/comment
//! no-behavior suppression, dict/list element and fstring/length
//! invariants, changed-default policy, `weakly_exposed` boundaries,
//! repair-class discriminators, related-test matching, static-limit
//! fail-closed behavior, and end-to-end `analyze_diff` /
//! `analyze_repo` calls.
//!
//! The sibling `python_tests` module holds the separate coverage-pinning
//! suite for previously uncovered adapter branches; the two suites are
//! intentionally not merged.

fn unique_test_root(label: &str) -> std::path::PathBuf {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or(0);
    std::env::temp_dir().join(format!("ripr-py-fw-{label}-{}-{stamp}", std::process::id()))
}

#[test]
fn detect_python_test_framework_reads_setup_cfg_and_tox_sections() -> Result<(), String> {
    let root = unique_test_root("setup-cfg");
    std::fs::create_dir_all(&root).map_err(|err| format!("create root: {err}"))?;
    std::fs::write(root.join("setup.cfg"), "[tool:pytest]\naddopts = -q\n")
        .map_err(|err| format!("write setup.cfg: {err}"))?;
    assert_eq!(super::detect_python_test_framework(&root), Some("pytest"));
    std::fs::remove_dir_all(&root).map_err(|err| format!("remove root: {err}"))?;

    let root = unique_test_root("tox");
    std::fs::create_dir_all(&root).map_err(|err| format!("create root: {err}"))?;
    std::fs::write(root.join("tox.ini"), "[pytest]\naddopts = -q\n")
        .map_err(|err| format!("write tox.ini: {err}"))?;
    assert_eq!(super::detect_python_test_framework(&root), Some("pytest"));
    std::fs::remove_dir_all(&root).map_err(|err| format!("remove root: {err}"))?;
    Ok(())
}

#[test]
fn detect_python_test_framework_reads_conftest_py() -> Result<(), String> {
    let root = unique_test_root("conftest");
    std::fs::create_dir_all(&root).map_err(|err| format!("create root: {err}"))?;
    std::fs::write(root.join("conftest.py"), "import pytest\n")
        .map_err(|err| format!("write conftest.py: {err}"))?;
    assert_eq!(super::detect_python_test_framework(&root), Some("pytest"));
    std::fs::remove_dir_all(&root).map_err(|err| format!("remove root: {err}"))?;
    Ok(())
}

#[test]
fn detect_python_test_framework_reads_pytest_ini_and_pyproject() -> Result<(), String> {
    let root = unique_test_root("pytest-ini");
    std::fs::create_dir_all(&root).map_err(|err| format!("create root: {err}"))?;
    std::fs::write(root.join("pytest.ini"), "[pytest]\n")
        .map_err(|err| format!("write pytest.ini: {err}"))?;
    assert_eq!(super::detect_python_test_framework(&root), Some("pytest"));
    std::fs::remove_dir_all(&root).map_err(|err| format!("remove root: {err}"))?;

    let root = unique_test_root("pyproject");
    std::fs::create_dir_all(&root).map_err(|err| format!("create root: {err}"))?;
    std::fs::write(root.join("pyproject.toml"), "[tool.pytest.ini_options]\n")
        .map_err(|err| format!("write pyproject.toml: {err}"))?;
    assert_eq!(super::detect_python_test_framework(&root), Some("pytest"));
    std::fs::remove_dir_all(&root).map_err(|err| format!("remove root: {err}"))?;
    Ok(())
}

#[test]
fn detect_python_test_framework_recognizes_from_unittest_import() -> Result<(), String> {
    let root = unique_test_root("from-unittest");
    std::fs::create_dir_all(&root).map_err(|err| format!("create root: {err}"))?;
    std::fs::write(
        root.join("test_pricing.py"),
        "from unittest import TestCase\n\nclass TestPricing(TestCase):\n    pass\n",
    )
    .map_err(|err| format!("write test file: {err}"))?;
    assert_eq!(super::detect_python_test_framework(&root), Some("unittest"));
    std::fs::remove_dir_all(&root).map_err(|err| format!("remove root: {err}"))?;
    Ok(())
}

#[test]
fn detect_python_test_framework_rejects_lookalike_and_commented_imports() -> Result<(), String> {
    // Negative fixtures (#2106 review): a lookalike identifier and a
    // commented-out import must NOT report unittest.
    let root = unique_test_root("lookalike");
    std::fs::create_dir_all(&root).map_err(|err| format!("create root: {err}"))?;
    std::fs::write(root.join("test_lookalike.py"), "import unittesting\n")
        .map_err(|err| format!("write test file: {err}"))?;
    assert_eq!(super::detect_python_test_framework(&root), None);
    std::fs::remove_dir_all(&root).map_err(|err| format!("remove root: {err}"))?;

    let root = unique_test_root("commented");
    std::fs::create_dir_all(&root).map_err(|err| format!("create root: {err}"))?;
    std::fs::write(root.join("test_commented.py"), "# import unittest\n")
        .map_err(|err| format!("write test file: {err}"))?;
    assert_eq!(super::detect_python_test_framework(&root), None);
    std::fs::remove_dir_all(&root).map_err(|err| format!("remove root: {err}"))?;
    Ok(())
}

#[test]
fn detect_python_test_framework_detects_unittest_from_code_evidence() -> Result<(), String> {
    let root = unique_test_root("unittest");
    let tests_dir = root.join("tests");
    std::fs::create_dir_all(&tests_dir).map_err(|err| format!("create tests dir: {err}"))?;
    std::fs::write(
        tests_dir.join("test_pricing.py"),
        "import unittest\n\nclass TestPricing(unittest.TestCase):\n    pass\n",
    )
    .map_err(|err| format!("write test file: {err}"))?;
    assert_eq!(super::detect_python_test_framework(&root), Some("unittest"));
    std::fs::remove_dir_all(&root).map_err(|err| format!("remove root: {err}"))?;
    Ok(())
}

#[test]
fn detect_python_test_framework_is_fail_closed_for_empty_root() -> Result<(), String> {
    let root = unique_test_root("empty");
    std::fs::create_dir_all(&root).map_err(|err| format!("create root: {err}"))?;
    assert_eq!(super::detect_python_test_framework(&root), None);
    std::fs::remove_dir_all(&root).map_err(|err| format!("remove root: {err}"))?;
    Ok(())
}
use super::owners_tests::{extract_owners, extract_tests};
use super::*;
use std::path::{Path, PathBuf};

fn changed(path: &str) -> ChangedFile {
    ChangedFile {
        path: PathBuf::from(path),
        added_lines: Vec::new(),
        removed_lines: Vec::new(),
    }
}

fn missing_discriminator_values(finding: &Finding) -> Vec<&str> {
    finding
        .activation
        .missing_discriminators
        .iter()
        .map(|missing| missing.value.as_str())
        .collect()
}

fn evidence_value<'a>(finding: &'a Finding, prefix: &str) -> Option<&'a str> {
    finding
        .evidence
        .iter()
        .find_map(|entry| entry.strip_prefix(prefix))
}

#[test]
fn accepts_py_paths() {
    let adapter = PythonAdapter;
    assert!(adapter.accepts_path(Path::new("scripts/run.py")));
    assert!(adapter.accepts_path(Path::new("src/lib/util.py")));
    assert!(!adapter.accepts_path(Path::new("src/lib.rs")));
    assert!(!adapter.accepts_path(Path::new("src/index.ts")));
    assert!(!adapter.accepts_path(Path::new("src/index.tsx")));
    assert!(!adapter.accepts_path(Path::new("README.md")));
    assert!(!adapter.accepts_path(Path::new("no-extension")));
}

#[test]
fn parse_source_accepts_simple_python() {
    let ok = parse_module(
        Path::new("src/discount.py"),
        "def discount(amount: int) -> int:\n    return amount\n",
    )
    .is_some();
    assert!(ok, "valid Python should parse without errors");
}

#[test]
fn parse_source_accepts_class_and_decorator() {
    let ok = parse_module(
        Path::new("src/repo.py"),
        "class Repo:\n    @staticmethod\n    def make() -> 'Repo':\n        return Repo()\n",
    )
    .is_some();
    assert!(ok, "decorated class methods should parse");
}

#[test]
fn parse_source_accepts_async_def_and_fstring() {
    let ok = parse_module(
        Path::new("src/http.py"),
        "async def load(url: str) -> str:\n    return f\"{url}!\"\n",
    )
    .is_some();
    assert!(ok, "async def + f-string should parse");
}

#[test]
fn parse_source_rejects_garbage() {
    let ok = parse_module(
        Path::new("src/oops.py"),
        "this is not :: valid +++ python at all",
    )
    .is_some();
    assert!(!ok, "garbage source should produce parse errors");
}

#[test]
fn extract_owners_recognizes_functions_and_methods() {
    let owners = extract_owners(
        Path::new("src/pricing.py"),
        r#"
def apply_discount(amount):
    return amount

async def load_total(client):
    return await client.total()

class Policy:
    def apply(self, amount):
        return amount

    @staticmethod
    def normalize(amount):
        return amount

    @classmethod
    def from_config(cls, config):
        return cls()
"#,
    );

    assert_eq!(
        owners
            .iter()
            .map(|owner| owner.qualified_name.as_str())
            .collect::<Vec<_>>(),
        vec![
            "apply_discount",
            "load_total",
            "Policy.apply",
            "Policy.normalize",
            "Policy.from_config",
            "Policy",
            "<module>"
        ]
    );
    assert_eq!(owners[0].owner_kind, Some(OwnerKind::Function));
    assert_eq!(owners[1].decorators, vec!["async_def"]);
    assert_eq!(owners[2].owner_kind, Some(OwnerKind::Method));
    assert_eq!(owners[3].owner_kind, Some(OwnerKind::ClassMethod));
    assert_eq!(owners[4].owner_kind, Some(OwnerKind::ClassMethod));
}

#[test]
fn extract_tests_recognizes_pytest_parametrize_and_unittest() {
    let tests = extract_tests(
        Path::new("tests/test_pricing.py"),
        r#"
import unittest
import pytest

@pytest.mark.parametrize("amount", [1, 2])
def test_apply_discount(amount):
    apply_discount(amount)

class TestPytestStyle:
    def test_class_style(self, client):
        assert client.get("/discount").status_code == 200

class Helper:
    def test_not_a_pytest_class(self):
        apply_discount(10)

class PriceTests(unittest.TestCase):
    def test_apply_method(self):
        Policy().apply(10)
"#,
    );

    assert_eq!(
        tests
            .iter()
            .map(|test| test.name.as_str())
            .collect::<Vec<_>>(),
        vec![
            "test_apply_discount",
            "test_class_style",
            "test_apply_method"
        ]
    );
    assert!(tests[0].parametrized);
    assert_eq!(tests[0].fixtures, vec!["amount".to_string()]);
    assert_eq!(tests[0].qualified_name, "test_apply_discount");
    assert_eq!(tests[0].framework, "pytest");
    assert_eq!(tests[1].fixtures, vec!["client".to_string()]);
    assert_eq!(tests[1].qualified_name, "TestPytestStyle.test_class_style");
    assert_eq!(tests[1].framework, "pytest");
    assert_eq!(tests[2].qualified_name, "PriceTests.test_apply_method");
    assert_eq!(tests[2].framework, "unittest");
    assert!(
        tests
            .iter()
            .all(|test| test.name != "test_not_a_pytest_class")
    );
}

#[test]
fn extract_tests_records_module_import_aliases() {
    let tests = extract_tests(
        Path::new("tests/test_imports.py"),
        r#"
import src.catalog as catalog
from src.tax import apply_fee, apply_tax as taxed

def test_imports():
    assert catalog.calculate_total(10) == 17
    assert taxed(10) == 12
"#,
    );

    assert_eq!(
        tests[0]
            .imports
            .iter()
            .map(|import| (import.imported.as_str(), import.alias.as_str()))
            .collect::<Vec<_>>(),
        vec![
            ("src.catalog", "catalog"),
            ("apply_fee", "apply_fee"),
            ("apply_tax", "taxed")
        ]
    );
}

#[test]
fn extract_tests_collects_pytest_assertion_oracles() {
    let tests = extract_tests(
        Path::new("tests/test_pricing.py"),
        r#"
def test_apply_discount_exact():
    assert apply_discount(100, 50) == 90

def test_apply_discount_negative():
    assert apply_discount(10, 50) != 90

def test_apply_discount_smoke():
    assert apply_discount(10, 50)

def test_apply_discount_type():
    assert isinstance(apply_discount(10, 50), int)
"#,
    );

    assert_eq!(tests.len(), 4);
    assert_eq!(tests[0].assertions[0].oracle_kind, OracleKind::ExactValue);
    assert_eq!(
        tests[0].assertions[0].oracle_strength,
        OracleStrength::Strong
    );
    assert_eq!(
        tests[1].assertions[0].oracle_kind,
        OracleKind::RelationalCheck
    );
    assert_eq!(tests[1].assertions[0].oracle_strength, OracleStrength::Weak);
    assert_eq!(tests[2].assertions[0].oracle_kind, OracleKind::SmokeOnly);
    assert_eq!(
        tests[2].assertions[0].oracle_strength,
        OracleStrength::Smoke
    );
    assert_eq!(
        tests[3].assertions[0].oracle_kind,
        OracleKind::RelationalCheck
    );
    assert_eq!(tests[3].assertions[0].oracle_strength, OracleStrength::Weak);
}

#[test]
fn extract_tests_collects_pytest_raises_oracle() {
    let tests = extract_tests(
        Path::new("tests/test_validation.py"),
        r#"
import pytest

def test_apply_discount_rejects_negative():
    with pytest.raises(ValueError):
        apply_discount(-1, 50)
"#,
    );

    assert_eq!(tests.len(), 1);
    assert_eq!(tests[0].assertions[0].oracle_kind, OracleKind::BroadError);
    assert_eq!(tests[0].assertions[0].oracle_strength, OracleStrength::Weak);
}

#[test]
fn extract_tests_collects_unittest_assertion_oracles() {
    let tests = extract_tests(
        Path::new("tests/test_pricing.py"),
        r#"
import unittest

class PriceTests(unittest.TestCase):
    def test_apply_discount_exact(self):
        self.assertEqual(apply_discount(100, 50), 90)

    def test_apply_discount_raises(self):
        with self.assertRaises(ValueError):
            apply_discount(-1, 50)

    def test_apply_discount_boolean(self):
        self.assertTrue(apply_discount(10, 50) >= 0)
"#,
    );

    assert_eq!(tests.len(), 3);
    assert_eq!(tests[0].assertions[0].oracle_kind, OracleKind::ExactValue);
    assert_eq!(
        tests[0].assertions[0].oracle_strength,
        OracleStrength::Strong
    );
    assert_eq!(tests[1].assertions[0].oracle_kind, OracleKind::BroadError);
    assert_eq!(tests[1].assertions[0].oracle_strength, OracleStrength::Weak);
    assert_eq!(tests[2].assertions[0].oracle_kind, OracleKind::SmokeOnly);
    assert_eq!(
        tests[2].assertions[0].oracle_strength,
        OracleStrength::Smoke
    );
}

#[test]
fn extract_tests_collects_mock_call_oracle() {
    let tests = extract_tests(
        Path::new("tests/test_notifier.py"),
        r#"
def test_notifies_callback():
    callback = Mock()
    send_alert(callback)
    callback.assert_called_once_with("sent")
"#,
    );

    assert_eq!(tests.len(), 1);
    assert_eq!(
        tests[0].assertions[0].oracle_kind,
        OracleKind::MockExpectation
    );
    assert_eq!(
        tests[0].assertions[0].oracle_strength,
        OracleStrength::Medium
    );
}

#[test]
fn classify_probe_shape_recognizes_python_predicate_shapes() {
    let (family, delta) = classify_probe_shape("    if amount >= threshold:");
    assert_eq!(family, ProbeFamily::Predicate);
    assert_eq!(delta, DeltaKind::Control);

    let (family, delta) =
        classify_probe_shape("    label = \"high\" if amount >= threshold else \"normal\"");
    assert_eq!(family, ProbeFamily::Predicate);
    assert_eq!(delta, DeltaKind::Control);
}

#[test]
fn classify_probe_shape_recognizes_python_return_and_error_shapes() {
    let (family, delta) = classify_probe_shape("    return amount - 10");
    assert_eq!(family, ProbeFamily::ReturnValue);
    assert_eq!(delta, DeltaKind::Value);

    let (family, delta) = classify_probe_shape("    raise ValueError(\"bad\")");
    assert_eq!(family, ProbeFamily::ErrorPath);
    assert_eq!(delta, DeltaKind::Control);

    let (family, delta) = classify_probe_shape("    except ValueError:");
    assert_eq!(family, ProbeFamily::ErrorPath);
    assert_eq!(delta, DeltaKind::Control);
}

#[test]
fn classify_probe_shape_recognizes_python_field_and_call_shapes() {
    let (family, delta) = classify_probe_shape("    self.status = \"paid\"");
    assert_eq!(family, ProbeFamily::FieldConstruction);
    assert_eq!(delta, DeltaKind::Value);

    let (family, delta) = classify_probe_shape("    return User(active=True)");
    assert_eq!(family, ProbeFamily::FieldConstruction);
    assert_eq!(delta, DeltaKind::Value);

    let (family, delta) = classify_probe_shape("    notifier(\"receipt.sent\", order_id)");
    assert_eq!(family, ProbeFamily::SideEffect);
    assert_eq!(delta, DeltaKind::Effect);

    let (family, delta) = classify_probe_shape("    callback = MagicMock(name=\"receipt\")");
    assert_eq!(family, ProbeFamily::SideEffect);
    assert_eq!(delta, DeltaKind::Effect);
}

#[test]
fn return_dict_field_parts_prefer_literal_changed_value_candidates() {
    assert_eq!(
        python_return_dict_field_parts("return {\"name\": name, \"status\": \"active\"}"),
        Some(("status".to_string(), "\"active\"".to_string()))
    );
    assert_eq!(
        python_return_dict_field_discriminator("return {\"name\": name, \"status\": \"active\"}")
            .as_deref(),
        Some("status == \"active\"")
    );
    assert_eq!(
        python_return_dict_field_parts("return {\"label\": \"ready, set\", \"status\": status}"),
        Some(("label".to_string(), "\"ready, set\"".to_string()))
    );
    assert_eq!(
        python_return_dict_field_parts("return {\"status\": status}"),
        Some(("status".to_string(), "status".to_string()))
    );
}

#[test]
fn return_dict_field_parts_handle_nested_segments_and_literal_kinds() {
    assert_eq!(
        top_level_python_segments(
            "\"payload\": {\"status\": \"active, pending\"}, \"note\": \"a,b\""
        ),
        vec![
            "\"payload\": {\"status\": \"active, pending\"}",
            "\"note\": \"a,b\""
        ]
    );
    assert_eq!(
        top_level_python_segments("\"label\": \"ready\\\"set\", \"status\": status"),
        vec!["\"label\": \"ready\\\"set\"", "\"status\": status"]
    );
    assert_eq!(
        python_dict_field_segment_parts("\"url\": \"https://example.test/a:b\""),
        Some(("url", "\"https://example.test/a:b\""))
    );
    assert_eq!(python_dict_field_segment_parts("\"status\""), None);
    assert!(is_literal_python_model_field_value("True"));
    assert!(is_literal_python_model_field_value("-1.5"));
    assert!(!is_literal_python_model_field_value("status"));
    assert_eq!(
        python_return_dict_field_parts(
            "return {\"status\": status, invalid_segment, \"count\": total}"
        ),
        Some(("status".to_string(), "status".to_string()))
    );
    assert_eq!(
        python_return_dict_field_parts("return {\"payload\": make_payload(a, b)}"),
        Some(("payload".to_string(), "make_payload(a, b)".to_string()))
    );
    assert_eq!(python_return_dict_field_parts("return {}"), None);
}

#[test]
fn constructor_keyword_field_parts_accept_simple_model_field_values() {
    assert_eq!(
        python_return_constructor_field_parts("return User(active=True)"),
        Some(("User".to_string(), "active".to_string(), "True".to_string()))
    );
    assert_eq!(
        python_return_constructor_field_parts("return models.User(name=\"Ada\")"),
        Some((
            "models.User".to_string(),
            "name".to_string(),
            "\"Ada\"".to_string()
        ))
    );
    assert_eq!(
        python_return_constructor_field_parts("return _User(score=-1.5)"),
        Some(("_User".to_string(), "score".to_string(), "-1.5".to_string()))
    );
    assert_eq!(
        python_return_constructor_field_parts("return User(plan=default_plan)"),
        Some((
            "User".to_string(),
            "plan".to_string(),
            "default_plan".to_string()
        ))
    );
    assert_eq!(
        python_return_constructor_field_parts("return User(label=\"a=b\")"),
        Some((
            "User".to_string(),
            "label".to_string(),
            "\"a=b\"".to_string()
        ))
    );
}

#[test]
fn constructor_keyword_field_parts_fail_closed_for_ambiguous_shapes() {
    assert_eq!(
        python_return_constructor_field_parts("return build_user(active=True)"),
        None
    );
    assert_eq!(
        python_return_constructor_field_parts("return User(\"Ada\")"),
        None
    );
    assert_eq!(
        python_return_constructor_field_parts("return User(profile.active=True)"),
        None
    );
    assert_eq!(
        python_return_constructor_field_parts("return User(active=build_active())"),
        None
    );
    assert_eq!(
        python_return_constructor_field_parts(
            "return User(config={\"active\": True}, active=True)"
        ),
        None
    );
    assert_eq!(
        python_return_constructor_field_parts("value = User(active=True)"),
        None
    );
}

#[test]
fn first_python_keyword_argument_skips_positional_and_nested_arguments() {
    assert_eq!(
        first_python_keyword_argument("factory(a=b), active=True"),
        Some(("active", "True"))
    );
    assert_eq!(
        first_python_keyword_argument("name=\"Ada, Lovelace\", active=True"),
        Some(("name", "\"Ada, Lovelace\""))
    );
    assert_eq!(
        first_python_keyword_argument("metadata={\"a\": \"b,c\"}, active=True"),
        Some(("metadata", "{\"a\": \"b,c\"}"))
    );
    assert_eq!(first_python_keyword_argument("factory(a=b), user"), None);
}

#[test]
fn classify_change_uses_constructor_keyword_field_discriminator() -> Result<(), String> {
    let source = r#"
from dataclasses import dataclass

@dataclass
class User:
    active: bool

def build_user():
    return User(active=True)
"#;
    let owners = extract_owners(Path::new("src/users.py"), source);
    let tests = extract_tests(
        Path::new("tests/test_users.py"),
        r#"
from src.users import build_user

def test_build_user_smoke():
    user = build_user()
    assert user
"#,
    );

    let Some(finding) = classify_change(
        Path::new("src/users.py"),
        9,
        "    return User(active=True)",
        &owners,
        &tests,
    ) else {
        return Err("changed constructor return inside owner should classify".to_string());
    };

    assert_eq!(finding.class, ExposureClass::WeaklyExposed);
    assert_eq!(finding.probe.family, ProbeFamily::FieldConstruction);
    assert_eq!(
        finding
            .activation
            .missing_discriminators
            .first()
            .map(|missing| missing.value.as_str()),
        Some("result.active == True")
    );
    assert!(
        finding
            .evidence
            .iter()
            .any(|entry| entry == "missing_discriminator: result.active == True")
    );
    Ok(())
}

#[test]
fn constructor_keyword_field_helpers_stay_bounded_and_fail_closed() {
    assert_eq!(
        python_return_constructor_field_discriminator("return User(active=True)").as_deref(),
        Some("result.active == True")
    );
    assert_eq!(
        python_return_constructor_field_discriminator("return models.User(score=-1.5)").as_deref(),
        Some("result.score == -1.5")
    );
    assert_eq!(
        split_python_constructor_call("User(active=True)"),
        Some(("User", "active=True"))
    );
    assert_eq!(split_python_constructor_call("User()"), None);
    assert_eq!(split_python_constructor_call("(User(active=True))"), None);
    assert!(is_python_constructor_callee("models.User"));
    assert!(is_python_constructor_callee("_PrivateUser"));
    assert!(!is_python_constructor_callee("make_user"));
    assert_eq!(
        first_python_keyword_argument("ignored, active=True"),
        Some(("active", "True"))
    );
    assert_eq!(
        first_python_keyword_argument("label=\"a,b=c\", active=False"),
        Some(("label", "\"a,b=c\""))
    );
    assert_eq!(
        first_python_keyword_argument("meta={\"threshold\": \"a=b,c\"}"),
        Some(("meta", "{\"threshold\": \"a=b,c\"}"))
    );
    assert_eq!(python_keyword_argument_parts("not keyword"), None);
    assert_eq!(top_level_equals("metadata={\"a\": \"b=c\"}"), Some(8));
    assert_eq!(top_level_equals("metadata"), None);
    assert!(is_simple_python_model_field_value("\"active\""));
    assert!(is_simple_python_model_field_value("True"));
    assert!(is_simple_python_model_field_value("None"));
    assert!(is_simple_python_model_field_value("-1.25"));
    assert!(is_simple_python_model_field_value(".5"));
    assert!(!is_simple_python_model_field_value("."));
    assert!(!is_simple_python_model_field_value("-"));
    assert!(!is_simple_python_model_field_value("1.2.3"));
    assert!(!is_simple_python_model_field_value("make_value()"));
    assert_eq!(
        python_return_constructor_field_discriminator("return make_user(active=True)"),
        None
    );
    assert_eq!(
        python_return_constructor_field_discriminator("return User(active=make_value())"),
        None
    );
    assert_eq!(
        python_assignment_constructor_field_parts(
            "response = Response(status_code=422, detail=\"coupon expired\")"
        ),
        Some((
            "response".to_string(),
            "Response".to_string(),
            "status_code".to_string(),
            "422".to_string()
        ))
    );
    assert_eq!(
        python_assignment_constructor_field_parts("response.body = Response(status_code=422)"),
        None
    );
    assert_eq!(
        python_assignment_constructor_field_parts("response = make_response(status_code=422)"),
        None
    );
    assert_eq!(
        python_assignment_constructor_field_parts("response = Response(detail=message())"),
        None
    );
    assert_eq!(
        python_route_response_field_discriminator("status_code", "422").as_deref(),
        Some("response.status_code == 422")
    );
    assert_eq!(
        python_route_response_field_discriminator("detail", "\"coupon expired\"").as_deref(),
        Some("response.json()[\"detail\"] == \"coupon expired\"")
    );
    assert_eq!(
        python_route_response_field_discriminator("headers", "expected_headers").as_deref(),
        Some("response.headers == expected_headers")
    );
}

#[test]
fn construct_result_is_called_distinguishes_inline_from_bound() {
    // open-paren index of the first `(` in each fixture string (always present).
    let at = |s: &str| s.find('(').unwrap_or(0);
    // Inline construct-call `C(...)(...)`: the constructed instance is called.
    let inline = "Renderer()(None, event)";
    assert!(construct_result_is_called(inline, at(inline)));
    let inline_args = "Renderer(sort=True)(event)";
    assert!(construct_result_is_called(inline_args, at(inline_args)));
    // Bound local `x = C(...)` then a separate `x(...)`: NOT an inline call —
    // the constructor's `)` is followed by a newline, not `(`. Keeps the
    // local-callable case uncertain (consistent with #1221).
    let bound = "stop = stop_after_attempt(3)\n    stop(3)";
    assert!(!construct_result_is_called(bound, at(bound)));
    // Plain construction with no following call.
    let plain = "r = Renderer()";
    assert!(!construct_result_is_called(plain, at(plain)));
}

fn call_owner(owners: &[PythonOwner]) -> Result<&PythonOwner, String> {
    owners
        .iter()
        .find(|owner| owner.name == "__call__")
        .ok_or_else(|| "fixture defines a __call__ owner".to_string())
}

const STOP_SOURCE: &str = "class stop_after_attempt:\n    def __init__(self, max_attempt_number):\n        self.max_attempt_number = max_attempt_number\n\n    def __call__(self, attempt_number):\n        return attempt_number >= self.max_attempt_number\n";

#[test]
fn local_binding_relation_links_direct_and_surfaces_smoke_oracle() -> Result<(), String> {
    // The tenacity false-actionable shape: `stop = stop_after_attempt(3)`
    // bound once and called via `stop(3)` under a broad-boolean smoke oracle.
    let owners = extract_owners(Path::new("src/stop.py"), STOP_SOURCE);
    let tests = extract_tests(
        Path::new("tests/test_stop.py"),
        "import unittest\n\nfrom src.stop import stop_after_attempt\n\n\nclass StopTest(unittest.TestCase):\n    def test_stop_after_attempt(self):\n        stop = stop_after_attempt(3)\n        self.assertTrue(stop(3))\n",
    );
    let owner = call_owner(&owners)?;
    assert_eq!(
        related_test_relation(&tests[0], owner),
        Some(PythonRelationKind::LocalBinding),
        "single bound local called as `stop(3)` should link directly via local_binding"
    );

    let Some(finding) = classify_change(
        Path::new("src/stop.py"),
        6,
        "        return attempt_number >= self.max_attempt_number",
        &owners,
        &tests,
    ) else {
        return Err("changed return inside __call__ should classify".to_string());
    };
    // Must STAY weakly_exposed — a smoke oracle never credits `exposed`.
    assert_eq!(finding.class, ExposureClass::WeaklyExposed);
    assert_eq!(finding.related_tests.len(), 1);
    assert_eq!(
        finding.related_tests[0].oracle_strength,
        OracleStrength::Smoke,
        "the assertTrue(stop(3)) smoke oracle must be surfaced, not dropped to unknown"
    );
    Ok(())
}

#[test]
fn local_binding_does_not_fire_for_inline_construct_call() -> Result<(), String> {
    // Inline `C()(...)` is ConstructCall territory, not a bound local.
    let owners = extract_owners(Path::new("src/stop.py"), STOP_SOURCE);
    let tests = extract_tests(
        Path::new("tests/test_stop.py"),
        "from src.stop import stop_after_attempt\n\n\ndef test_inline():\n    assert stop_after_attempt(3)(3)\n",
    );
    let owner = call_owner(&owners)?;
    assert_eq!(
        related_test_relation(&tests[0], owner),
        Some(PythonRelationKind::ConstructCall),
        "inline construct-call must stay ConstructCall, not LocalBinding"
    );
    Ok(())
}

#[test]
fn local_binding_does_not_fire_for_reassigned_binding() -> Result<(), String> {
    // A rebound local is ambiguous: which construction is called is unclear.
    let owners = extract_owners(Path::new("src/stop.py"), STOP_SOURCE);
    let tests = extract_tests(
        Path::new("tests/test_stop.py"),
        "from src.stop import stop_after_attempt\n\n\ndef test_reassigned():\n    stop = stop_after_attempt(3)\n    stop = stop_after_attempt(4)\n    assert stop(3)\n",
    );
    let owner = call_owner(&owners)?;
    assert!(
        !local_binding_calls_owner(&tests[0], owner),
        "two constructions / reassigned binding must not link via local_binding"
    );
    Ok(())
}

#[test]
fn local_binding_does_not_fire_for_wrapper_keyword_argument() -> Result<(), String> {
    // `Retrying(stop=stop_after_attempt(3))` binds a wrapper, not the class —
    // the assignment target is `retrying`, not `stop_after_attempt(...)`.
    let owners = extract_owners(Path::new("src/stop.py"), STOP_SOURCE);
    let tests = extract_tests(
        Path::new("tests/test_stop.py"),
        "from src.stop import stop_after_attempt\nimport tenacity\n\n\ndef test_wrapper():\n    retrying = tenacity.Retrying(stop=stop_after_attempt(3))\n    assert retrying(lambda: None)\n",
    );
    let owner = call_owner(&owners)?;
    assert!(
        !local_binding_calls_owner(&tests[0], owner),
        "a keyword-argument construction inside a wrapper must not link via local_binding"
    );
    Ok(())
}

#[test]
fn local_binding_requires_importing_the_owner_class() -> Result<(), String> {
    // Guard B: a same-named local without importing the class must not link.
    let owners = extract_owners(Path::new("src/stop.py"), STOP_SOURCE);
    let tests = extract_tests(
        Path::new("tests/test_stop.py"),
        "def test_no_import():\n    stop = stop_after_attempt(3)\n    assert stop(3)\n",
    );
    let owner = call_owner(&owners)?;
    assert!(
        !local_binding_calls_owner(&tests[0], owner),
        "without importing the owner class, local_binding must not fire (Guard B)"
    );
    Ok(())
}

#[test]
fn binding_target_extracts_only_direct_identifier_assignment() {
    // Direct `local = Class(` extracts the bare identifier.
    let direct = "    stop = stop_after_attempt(3)\n";
    let idx = direct.find("stop_after_attempt(").unwrap_or(0);
    assert_eq!(
        binding_target_for_construction(direct, idx).as_deref(),
        Some("stop")
    );
    // Keyword argument is not an assignment target.
    let kwarg = "    retrying = Retrying(stop=stop_after_attempt(3))\n";
    let kidx = kwarg.find("stop_after_attempt(").unwrap_or(0);
    assert_eq!(binding_target_for_construction(kwarg, kidx), None);
    // Attribute target `self.x = Class(` is not a bare local.
    let attr = "    self.stop = stop_after_attempt(3)\n";
    let aidx = attr.find("stop_after_attempt(").unwrap_or(0);
    assert_eq!(binding_target_for_construction(attr, aidx), None);
}

#[test]
fn oracle_text_observes_token_requires_identifier_boundary() {
    // Whole-word matches still observe (preserves genuine sink alignment).
    assert!(oracle_text_observes_token(
        "assert raises(ValueError, match='Invalid key: x')",
        "key"
    ));
    assert!(oracle_text_observes_token("assert stop(3)", "stop"));
    assert!(oracle_text_observes_token(
        "assert x.max_buffer_size == 2",
        "max_buffer_size"
    ));
    // Substring co-occurrence must NOT observe: the confirmed false-exposed
    // vector — `buffer` (a changed-sink token) inside an unrelated
    // `buffered_stream` oracle from a different class.
    assert!(!oracle_text_observes_token(
        "assert buffered_stream.receive_exactly(10) == b\"x\"",
        "buffer"
    ));
    assert!(!oracle_text_observes_token("assert client.send()", "len"));
    assert!(!oracle_text_observes_token("assert keys() == []", "key"));
    assert!(!oracle_text_observes_token("anything", ""));
}

#[test]
fn classify_change_returns_exposed_when_related_test_has_strong_oracle() -> Result<(), String> {
    let owners = extract_owners(
        Path::new("src/pricing.py"),
        "def apply_discount(amount):\n    if amount >= 100:\n        return amount - 10\n    return amount\n",
    );
    let tests = extract_tests(
        Path::new("tests/test_pricing.py"),
        "from src.pricing import apply_discount\n\n\ndef test_apply_discount():\n    assert apply_discount(100) == 90\n",
    );

    let Some(finding) = classify_change(
        Path::new("src/pricing.py"),
        2,
        "    if amount >= 100:",
        &owners,
        &tests,
    ) else {
        return Err("changed line inside owner should classify".to_string());
    };

    assert_eq!(finding.class, ExposureClass::Exposed);
    assert!(
        (finding.confidence - 0.6).abs() < 0.0001,
        "exposed Python preview confidence should be 0.6"
    );
    assert_eq!(finding.related_tests.len(), 1);
    assert_eq!(finding.related_tests[0].oracle_kind, OracleKind::ExactValue);
    assert_eq!(
        finding.related_tests[0].oracle_strength,
        OracleStrength::Strong
    );
    assert!(finding.activation.missing_discriminators.is_empty());
    assert!(
        finding
            .evidence
            .iter()
            .all(|entry| !entry.starts_with("missing_discriminator:"))
    );
    Ok(())
}

const AUTH_SOURCE: &str = "class TokenValidator:\n    def __init__(self, valid):\n        self._valid = valid\n\n    def validate(self, token):\n        return token.strip() in self._valid\n";
const AUTH_CHANGED_LINE: &str = "        return token.strip() in self._valid";

#[test]
fn method_name_without_class_identity_stays_orthogonal() -> Result<(), String> {
    // False-`exposed` guard: the only related test exercises a DIFFERENT
    // class's same-named method (`PaymentProcessor.validate`) and never
    // imports the owner's class. The bare method-name token `validate` must
    // not credit `direct` alignment without owner-class identity.
    let owners = extract_owners(Path::new("src/auth.py"), AUTH_SOURCE);
    let tests = extract_tests(
        Path::new("tests/test_billing.py"),
        "from src.billing import PaymentProcessor\n\n\ndef test_billing_validate():\n    proc = PaymentProcessor()\n    assert proc.validate(\"card1234 \") == True\n",
    );
    let Some(finding) = classify_change(
        Path::new("src/auth.py"),
        6,
        AUTH_CHANGED_LINE,
        &owners,
        &tests,
    ) else {
        return Err("changed return inside validate should classify".to_string());
    };
    assert_eq!(
        finding.class,
        ExposureClass::WeaklyExposed,
        "a same-named method on an unrelated class must not credit exposed"
    );
    assert_eq!(finding.oracle_alignment.as_deref(), Some("orthogonal"));
    Ok(())
}

#[test]
fn method_name_with_class_import_identity_credits_exposed() -> Result<(), String> {
    // Identity preserved: the test imports and constructs the owner's class,
    // then observes its method under an exact-value oracle. The bare
    // method-name match is legitimate here, so `direct`/`exposed` stands.
    let owners = extract_owners(Path::new("src/auth.py"), AUTH_SOURCE);
    let tests = extract_tests(
        Path::new("tests/test_auth.py"),
        "from src.auth import TokenValidator\n\n\ndef test_auth_validate():\n    validator = TokenValidator([\"card1234\"])\n    assert validator.validate(\"card1234\") == True\n",
    );
    let Some(finding) = classify_change(
        Path::new("src/auth.py"),
        6,
        AUTH_CHANGED_LINE,
        &owners,
        &tests,
    ) else {
        return Err("changed return inside validate should classify".to_string());
    };
    assert_eq!(
        finding.class,
        ExposureClass::Exposed,
        "a test that imports and exercises the owner class keeps exposed credit"
    );
    assert_eq!(finding.oracle_alignment.as_deref(), Some("direct"));
    Ok(())
}

#[test]
fn method_name_dead_import_does_not_credit_exposed() -> Result<(), String> {
    // Bypass guard: a DEAD import of the owner class (never used in the test
    // body) is not identity evidence. The test exercises a different class's
    // same-named method, so it must stay conservative.
    let owners = extract_owners(Path::new("src/auth.py"), AUTH_SOURCE);
    let tests = extract_tests(
        Path::new("tests/test_billing.py"),
        "from src.billing import PaymentProcessor\nfrom src.auth import TokenValidator\n\n\ndef test_billing_validate():\n    proc = PaymentProcessor()\n    assert proc.validate(\"card1234 \") == True\n",
    );
    let Some(finding) = classify_change(
        Path::new("src/auth.py"),
        6,
        AUTH_CHANGED_LINE,
        &owners,
        &tests,
    ) else {
        return Err("changed return inside validate should classify".to_string());
    };
    assert_eq!(
        finding.class,
        ExposureClass::WeaklyExposed,
        "a dead import of the owner class must not credit exposed"
    );
    assert_ne!(finding.oracle_alignment.as_deref(), Some("direct"));
    Ok(())
}

#[test]
fn method_owner_free_function_alias_does_not_credit_exposed() -> Result<(), String> {
    // Bypass guard: a same-named FREE function aliased from an unrelated
    // module must not credit `exposed`/`alias` for a method owner. The test
    // never imports or exercises the owner's class.
    let owners = extract_owners(Path::new("src/auth.py"), AUTH_SOURCE);
    let tests = extract_tests(
        Path::new("tests/test_helpers.py"),
        "from src.helpers import validate as run_check\n\n\ndef test_helpers_run_check():\n    assert run_check(\"data\") == True\n",
    );
    if let Some(finding) = classify_change(
        Path::new("src/auth.py"),
        6,
        AUTH_CHANGED_LINE,
        &owners,
        &tests,
    ) {
        assert_ne!(
            finding.class,
            ExposureClass::Exposed,
            "a same-named free-function alias must not credit exposed for a method owner"
        );
        assert_ne!(finding.oracle_alignment.as_deref(), Some("alias"));
    }
    Ok(())
}

#[test]
fn method_name_class_constructed_but_method_on_other_receiver_does_not_credit_exposed()
-> Result<(), String> {
    // Receiver-identity guard (the residual leak the #1253 class-import gate
    // left open): the owner class is imported AND constructed in real code, but
    // the strong oracle's `.validate(` runs on an UNRELATED receiver. Class
    // identity is present; receiver identity is not, so it must stay
    // conservative.
    let owners = extract_owners(Path::new("src/auth.py"), AUTH_SOURCE);
    let tests = extract_tests(
        Path::new("tests/test_billing.py"),
        "from src.auth import TokenValidator\nfrom src.billing import PaymentProcessor\n\n\ndef test_billing_validate():\n    reference = TokenValidator([\"card1234\"])\n    proc = PaymentProcessor()\n    assert proc.validate(\"card1234 \") == True\n",
    );
    let Some(finding) = classify_change(
        Path::new("src/auth.py"),
        6,
        AUTH_CHANGED_LINE,
        &owners,
        &tests,
    ) else {
        return Err("changed return inside validate should classify".to_string());
    };
    assert_eq!(
        finding.class,
        ExposureClass::WeaklyExposed,
        "constructing the owner class is not enough; the asserted method ran on a different receiver"
    );
    assert_ne!(finding.oracle_alignment.as_deref(), Some("direct"));
    Ok(())
}

#[test]
fn method_name_inline_construct_call_credits_exposed() -> Result<(), String> {
    // Positive control: an inline `OwnerClass(...).method(...)` binds the
    // receiver to the owner class, and a strong exact-value oracle observes it.
    let owners = extract_owners(Path::new("src/auth.py"), AUTH_SOURCE);
    let tests = extract_tests(
        Path::new("tests/test_auth.py"),
        "from src.auth import TokenValidator\n\n\ndef test_auth_inline():\n    assert TokenValidator([\"card1234\"]).validate(\"card1234\") == True\n",
    );
    let Some(finding) = classify_change(
        Path::new("src/auth.py"),
        6,
        AUTH_CHANGED_LINE,
        &owners,
        &tests,
    ) else {
        return Err("changed return inside validate should classify".to_string());
    };
    assert_eq!(
        finding.class,
        ExposureClass::Exposed,
        "inline construct-call binds the receiver to the owner class"
    );
    assert_eq!(finding.oracle_alignment.as_deref(), Some("direct"));
    Ok(())
}

#[test]
fn method_name_classmethod_direct_call_credits_exposed() -> Result<(), String> {
    // Positive control: a classmethod called directly on the owner class
    // (`OwnerClass.method(...)`) is receiver-bound by construction.
    let source = "class TokenRegistry:\n    @classmethod\n    def lookup(cls, token):\n        return token.strip()\n";
    let owners = extract_owners(Path::new("src/registry.py"), source);
    let tests = extract_tests(
        Path::new("tests/test_registry.py"),
        "from src.registry import TokenRegistry\n\n\ndef test_registry_lookup():\n    assert TokenRegistry.lookup(\"abc \") == \"abc\"\n",
    );
    let Some(finding) = classify_change(
        Path::new("src/registry.py"),
        4,
        "        return token.strip()",
        &owners,
        &tests,
    ) else {
        return Err("changed return inside lookup should classify".to_string());
    };
    assert_eq!(
        finding.class,
        ExposureClass::Exposed,
        "a classmethod called on the owner class is receiver-bound"
    );
    assert_eq!(finding.oracle_alignment.as_deref(), Some("direct"));
    Ok(())
}

const SESSION_SOURCE: &str =
    "class Session:\n    def refresh(self):\n        self.status = \"active\"\n";
const SESSION_CHANGED_LINE: &str = "        self.status = \"active\"";

#[test]
fn attribute_sink_different_receiver_and_value_does_not_credit_exposed() -> Result<(), String> {
    // Cluster A guard: a changed attribute write `self.status = "active"` must
    // not credit `changed_sink_token` when the strong oracle observes a
    // DIFFERENT receiver's same-named attribute with a different value
    // (`conn.status == "closed"`) — pure attribute-name token coincidence.
    let owners = extract_owners(Path::new("src/session.py"), SESSION_SOURCE);
    let tests = extract_tests(
        Path::new("tests/test_session.py"),
        "from src.session import Session\n\n\ndef test_refresh(conn):\n    Session().refresh()\n    assert conn.status == \"closed\"\n",
    );
    let Some(finding) = classify_change(
        Path::new("src/session.py"),
        3,
        SESSION_CHANGED_LINE,
        &owners,
        &tests,
    ) else {
        return Err("changed attribute write should classify".to_string());
    };
    assert_ne!(
        finding.class,
        ExposureClass::Exposed,
        "a different receiver AND different value must not credit exposed"
    );
    assert_ne!(
        finding.oracle_alignment.as_deref(),
        Some("changed_sink_token")
    );
    Ok(())
}

#[test]
fn attribute_sink_value_and_attr_observed_credits_exposed() -> Result<(), String> {
    // Positive control: observing the assigned VALUE together with the
    // attribute name (`assert s.status == "active"`) is change-specific
    // evidence, so the changed-sink token credit stands.
    let owners = extract_owners(Path::new("src/session.py"), SESSION_SOURCE);
    let tests = extract_tests(
        Path::new("tests/test_session.py"),
        "from src.session import Session\n\n\ndef test_refresh_status():\n    s = Session()\n    s.refresh()\n    assert s.status == \"active\"\n",
    );
    let Some(finding) = classify_change(
        Path::new("src/session.py"),
        3,
        SESSION_CHANGED_LINE,
        &owners,
        &tests,
    ) else {
        return Err("changed attribute write should classify".to_string());
    };
    assert_eq!(
        finding.class,
        ExposureClass::Exposed,
        "observing the assigned value and the attribute name keeps exposed"
    );
    assert_eq!(
        finding.oracle_alignment.as_deref(),
        Some("changed_sink_token")
    );
    Ok(())
}

#[test]
fn attribute_sink_common_value_without_attr_does_not_credit_exposed() -> Result<(), String> {
    // Common-literal guard: the assigned value "active" is ubiquitous;
    // observing it on a DIFFERENT attribute (`widget.state == "active"`) must
    // not credit, because the changed attribute name `status` is not
    // co-observed.
    let owners = extract_owners(Path::new("src/session.py"), SESSION_SOURCE);
    let tests = extract_tests(
        Path::new("tests/test_session.py"),
        "from src.session import Session\n\n\ndef test_refresh_widget(widget):\n    Session().refresh()\n    assert widget.state == \"active\"\n",
    );
    let Some(finding) = classify_change(
        Path::new("src/session.py"),
        3,
        SESSION_CHANGED_LINE,
        &owners,
        &tests,
    ) else {
        return Err("changed attribute write should classify".to_string());
    };
    assert_ne!(
        finding.class,
        ExposureClass::Exposed,
        "a common assigned value on a different attribute must not credit exposed"
    );
    assert_ne!(
        finding.oracle_alignment.as_deref(),
        Some("changed_sink_token")
    );
    Ok(())
}

const HANDLER_SOURCE: &str = "def normalize(payload):\n    return payload.strip()\n";
const HANDLER_CHANGED_LINE: &str = "    return payload.strip()";

#[test]
fn free_function_imported_from_other_module_does_not_credit_exposed() -> Result<(), String> {
    // Cluster B guard: the changed free function is src.handler.normalize, but
    // the only related test imports a same-named normalize from src.checker.
    // The bare function-name token is not identity-bearing across modules.
    let owners = extract_owners(Path::new("src/handler.py"), HANDLER_SOURCE);
    let tests = extract_tests(
        Path::new("tests/test_checker.py"),
        "from src.checker import normalize\n\n\ndef test_checker_normalize():\n    assert normalize(\" ok \") == \"ok\"\n",
    );
    let Some(finding) = classify_change(
        Path::new("src/handler.py"),
        2,
        HANDLER_CHANGED_LINE,
        &owners,
        &tests,
    ) else {
        return Err("changed return inside normalize should classify".to_string());
    };
    assert_ne!(
        finding.class,
        ExposureClass::Exposed,
        "a same-named free function imported from a different module is not identity-bearing"
    );
    Ok(())
}

#[test]
fn free_function_imported_from_owner_module_credits_exposed() -> Result<(), String> {
    // Positive control: same-module import + a strong exact-value oracle.
    let owners = extract_owners(Path::new("src/handler.py"), HANDLER_SOURCE);
    let tests = extract_tests(
        Path::new("tests/test_handler.py"),
        "from src.handler import normalize\n\n\ndef test_handler_normalize():\n    assert normalize(\" ok \") == \"ok\"\n",
    );
    let Some(finding) = classify_change(
        Path::new("src/handler.py"),
        2,
        HANDLER_CHANGED_LINE,
        &owners,
        &tests,
    ) else {
        return Err("changed return inside normalize should classify".to_string());
    };
    assert_eq!(
        finding.class,
        ExposureClass::Exposed,
        "importing the function from the owner's module is identity-bearing"
    );
    assert_eq!(finding.oracle_alignment.as_deref(), Some("direct"));
    Ok(())
}

#[test]
fn free_function_relative_import_from_owner_module_credits_exposed() -> Result<(), String> {
    // Common package-local Python tests often use explicit relative imports.
    // Resolve the importing file's package before checking free-function
    // module identity so `from .handler import normalize` is not treated as
    // unrelated bare-name token coincidence.
    let owners = extract_owners(Path::new("src/handler.py"), HANDLER_SOURCE);
    let tests = extract_tests(
        Path::new("src/test_handler.py"),
        "from .handler import normalize\n\n\ndef test_handler_normalize_relative():\n    assert normalize(\" ok \") == \"ok\"\n",
    );
    let Some(finding) = classify_change(
        Path::new("src/handler.py"),
        2,
        HANDLER_CHANGED_LINE,
        &owners,
        &tests,
    ) else {
        return Err("changed return inside normalize should classify".to_string());
    };
    assert_eq!(
        finding.class,
        ExposureClass::Exposed,
        "a resolved relative import from the owner's module is identity-bearing"
    );
    assert_eq!(finding.oracle_alignment.as_deref(), Some("direct"));
    Ok(())
}

#[test]
fn free_function_relative_import_from_sibling_module_stays_fail_closed() -> Result<(), String> {
    // Boundary: `from .other import normalize` resolves to `src.other`, not
    // the owner's `src.handler` module — identity must NOT be credited from
    // the bare-name token coincidence.
    let owners = extract_owners(Path::new("src/handler.py"), HANDLER_SOURCE);
    let tests = extract_tests(
        Path::new("src/test_handler.py"),
        "from .other import normalize\n\n\ndef test_handler_normalize_sibling():\n    assert normalize(\" ok \") == \"ok\"\n",
    );
    let Some(finding) = classify_change(
        Path::new("src/handler.py"),
        2,
        HANDLER_CHANGED_LINE,
        &owners,
        &tests,
    ) else {
        return Err("changed return inside normalize should classify".to_string());
    };
    if finding.class == ExposureClass::Exposed {
        return Err(format!(
            "a relative import from a different module was wrongly credited: {:?}",
            finding.class
        ));
    }
    Ok(())
}

#[test]
fn relative_import_escaping_the_package_fails_closed() -> Result<(), String> {
    // `from ...handler import normalize` from a shallow file traverses above
    // the package root: the resolver must fail closed to an empty module
    // rather than fabricate an identity.
    let owners = extract_owners(Path::new("src/handler.py"), HANDLER_SOURCE);
    let tests = extract_tests(
        Path::new("src/test_handler.py"),
        "from ...handler import normalize\n\n\ndef test_handler_normalize_overtraverse():\n    assert normalize(\" ok \") == \"ok\"\n",
    );
    let Some(finding) = classify_change(
        Path::new("src/handler.py"),
        2,
        HANDLER_CHANGED_LINE,
        &owners,
        &tests,
    ) else {
        return Err("changed return inside normalize should classify".to_string());
    };
    if finding.class == ExposureClass::Exposed {
        return Err(format!(
            "an over-traversing relative import was wrongly credited: {:?}",
            finding.class
        ));
    }
    Ok(())
}

#[test]
fn relative_import_of_same_stem_helper_in_sibling_package_stays_fail_closed() -> Result<(), String>
{
    // The P1 review case: `src/tests/test_handler.py` importing
    // `from .handler import normalize` resolves to `src.tests.handler` — a
    // DIFFERENT module with the same stem as the owner's `src.handler`.
    // Stem-only matching would wrongly credit identity here.
    let owners = extract_owners(Path::new("src/handler.py"), HANDLER_SOURCE);
    let tests = extract_tests(
        Path::new("src/tests/test_handler.py"),
        "from .handler import normalize\n\n\ndef test_handler_same_stem_helper():\n    assert normalize(\" ok \") == \"ok\"\n",
    );
    let Some(finding) = classify_change(
        Path::new("src/handler.py"),
        2,
        HANDLER_CHANGED_LINE,
        &owners,
        &tests,
    ) else {
        return Err("changed return inside normalize should classify".to_string());
    };
    if finding.class == ExposureClass::Exposed {
        return Err(format!(
            "a same-stem helper in a sibling package was wrongly credited: {:?}",
            finding.class
        ));
    }
    Ok(())
}

#[test]
fn vendored_same_stem_module_does_not_match_owner_identity() -> Result<(), String> {
    // `from src.vendor.handler import normalize` names a same-stem module in
    // a different package — full-path identity must reject it.
    let owners = extract_owners(Path::new("src/handler.py"), HANDLER_SOURCE);
    let tests = extract_tests(
        Path::new("src/tests/test_handler.py"),
        "from src.vendor.handler import normalize\n\n\ndef test_handler_vendored():\n    assert normalize(\" ok \") == \"ok\"\n",
    );
    let Some(finding) = classify_change(
        Path::new("src/handler.py"),
        2,
        HANDLER_CHANGED_LINE,
        &owners,
        &tests,
    ) else {
        return Err("changed return inside normalize should classify".to_string());
    };
    if finding.class == ExposureClass::Exposed {
        return Err(format!(
            "a vendored same-stem module was wrongly credited: {:?}",
            finding.class
        ));
    }
    Ok(())
}

#[test]
fn free_function_aliased_import_from_owner_module_credits_exposed() -> Result<(), String> {
    // Positive control: aliased same-module import (`as norm`) keeps identity.
    let owners = extract_owners(Path::new("src/handler.py"), HANDLER_SOURCE);
    let tests = extract_tests(
        Path::new("tests/test_handler.py"),
        "from src.handler import normalize as norm\n\n\ndef test_handler_normalize_alias():\n    assert norm(\" ok \") == \"ok\"\n",
    );
    let Some(finding) = classify_change(
        Path::new("src/handler.py"),
        2,
        HANDLER_CHANGED_LINE,
        &owners,
        &tests,
    ) else {
        return Err("changed return inside normalize should classify".to_string());
    };
    assert_eq!(
        finding.class,
        ExposureClass::Exposed,
        "an aliased same-module import is identity-bearing"
    );
    assert_eq!(finding.oracle_alignment.as_deref(), Some("alias"));
    Ok(())
}

#[test]
fn free_function_changed_value_token_from_other_module_does_not_credit_exposed()
-> Result<(), String> {
    // Sibling-branch guard (#1249 lesson): even when a wrong-module test's
    // strong oracle observes the changed VALUE token ("ok"), the
    // changed_sink_token path must require free-function module identity too —
    // not just the direct/alias paths.
    let source = "def classify(payload):\n    return payload.strip() == \"ok\"\n";
    let owners = extract_owners(Path::new("src/handler.py"), source);
    let tests = extract_tests(
        Path::new("tests/test_checker.py"),
        "from src.checker import classify\n\n\ndef test_checker_classify():\n    assert classify(\"ok\") == \"ok\"\n",
    );
    let Some(finding) = classify_change(
        Path::new("src/handler.py"),
        2,
        "    return payload.strip() == \"ok\"",
        &owners,
        &tests,
    ) else {
        return Err("changed return inside classify should classify".to_string());
    };
    assert_ne!(
        finding.class,
        ExposureClass::Exposed,
        "a changed-value token observed by a different-module test is not identity-bearing"
    );
    assert_ne!(
        finding.oracle_alignment.as_deref(),
        Some("changed_sink_token")
    );
    Ok(())
}

#[test]
fn changed_sink_token_requires_delta_not_unchanged_operand() -> Result<(), String> {
    // #1276: the delta is `max` (the wrap); the oracle observes the UNCHANGED
    // operand `_balance` and never invokes the changed `balance` property, so it
    // does not discriminate the change. Must not credit changed_sink_token.
    let source = "class Account:\n    def __init__(self, balance):\n        self._balance = balance\n\n    @property\n    def balance(self):\n        return max(0, self._balance)\n";
    let owners = extract_owners(Path::new("src/account.py"), source);
    let tests = extract_tests(
        Path::new("tests/test_account.py"),
        "from src.account import Account\n\n\ndef test_account_init():\n    account = Account(100)\n    assert account._balance == 100\n",
    );
    let Some(finding) = classify_change_with_old(
        Path::new("src/account.py"),
        7,
        "        return max(0, self._balance)",
        Some("        return self._balance"),
        &owners,
        &tests,
    ) else {
        return Err("changed property body should classify".to_string());
    };
    assert_ne!(
        finding.class,
        ExposureClass::Exposed,
        "an unchanged operand observed by the test is not the behavior delta"
    );
    assert_ne!(
        finding.oracle_alignment.as_deref(),
        Some("changed_sink_token")
    );
    Ok(())
}

#[test]
fn changed_sink_token_credits_when_oracle_observes_the_delta_value() -> Result<(), String> {
    // Positive control: the changed VALUE "paid" IS the delta, and the oracle
    // observes it on the same owner instance — the credit stands.
    let source = "class Invoice:\n    def __init__(self):\n        self.status = \"open\"\n\n    def settle(self):\n        self.status = \"paid\"\n";
    let owners = extract_owners(Path::new("src/invoice.py"), source);
    let tests = extract_tests(
        Path::new("tests/test_invoice.py"),
        "from src.invoice import Invoice\n\n\ndef test_settle():\n    inv = Invoice()\n    inv.settle()\n    assert inv.status == \"paid\"\n",
    );
    let Some(finding) = classify_change_with_old(
        Path::new("src/invoice.py"),
        6,
        "        self.status = \"paid\"",
        Some("        self.status = \"settled\""),
        &owners,
        &tests,
    ) else {
        return Err("changed field write should classify".to_string());
    };
    assert_eq!(
        finding.class,
        ExposureClass::Exposed,
        "observing the changed value (the delta) credits exposed"
    );
    assert_eq!(
        finding.oracle_alignment.as_deref(),
        Some("changed_sink_token")
    );
    Ok(())
}

#[test]
fn empty_delta_operator_change_does_not_credit_unchanged_input_operand() -> Result<(), String> {
    // #1278: `+` -> `-` is an operator change with an EMPTY token delta. The only
    // strong oracle observes the UNCHANGED input parameter `count`, not the
    // changed return value, so the test does not discriminate the change. The
    // #1277 empty-delta fallback must NOT credit a value-family change on an
    // input operand.
    let source = "def next_value(count):\n    return count - 1\n";
    let owners = extract_owners(Path::new("src/counter.py"), source);
    let tests = extract_tests(
        Path::new("tests/test_counter.py"),
        "from src.counter import next_value\n\n\ndef test_next():\n    count = 5\n    result = next_value(count)\n    assert count == 5\n    assert result > 0\n",
    );
    let Some(finding) = classify_change_with_old(
        Path::new("src/counter.py"),
        2,
        "    return count - 1",
        Some("    return count + 1"),
        &owners,
        &tests,
    ) else {
        return Err("changed return body should classify".to_string());
    };
    assert_ne!(
        finding.class,
        ExposureClass::Exposed,
        "an operator change observed only via an unchanged input operand is not exposed"
    );
    assert_ne!(
        finding.oracle_alignment.as_deref(),
        Some("changed_sink_token")
    );
    Ok(())
}

#[test]
fn empty_delta_operator_change_stays_exposed_when_oracle_observes_owner_output()
-> Result<(), String> {
    // #1278 inverse control: the same operator change IS exposed when a strong
    // oracle observes the owner's OUTPUT by calling it (the `direct` path), not
    // via an input operand. This proves operator-change discrimination is
    // preserved when the test actually exercises the changed value.
    let source = "def next_value(count):\n    return count - 1\n";
    let owners = extract_owners(Path::new("src/counter.py"), source);
    let tests = extract_tests(
        Path::new("tests/test_counter.py"),
        "from src.counter import next_value\n\n\ndef test_next():\n    assert next_value(5) == 4\n",
    );
    let Some(finding) = classify_change_with_old(
        Path::new("src/counter.py"),
        2,
        "    return count - 1",
        Some("    return count + 1"),
        &owners,
        &tests,
    ) else {
        return Err("changed return body should classify".to_string());
    };
    assert_eq!(
        finding.class,
        ExposureClass::Exposed,
        "observing the owner's output (the call result) discriminates the operator change"
    );
    Ok(())
}

#[test]
fn empty_delta_predicate_change_still_credits_outcome_oracle() -> Result<(), String> {
    // #1278 preserve: a CONTROL-flow operator change (`<=` -> `<`) keeps the
    // empty-delta fallback — an outcome oracle (`pytest.raises`) discriminates the
    // changed branch, mirroring `python_cross_file_construct_call`.
    let source = "class Formatter:\n    def __call__(self, event):\n        for key in event:\n            if any(c < \" \" for c in key):\n                raise ValueError(f'Invalid key: \"{key}\"')\n        return \",\".join(event)\n";
    let owners = extract_owners(Path::new("src/formatter.py"), source);
    let tests = extract_tests(
        Path::new("tests/test_render.py"),
        "import pytest\n\nfrom src.formatter import Formatter\n\n\ndef test_rejects_space_in_key():\n    with pytest.raises(ValueError, match='Invalid key'):\n        Formatter()({\"bad key\": \"value\"})\n",
    );
    let Some(finding) = classify_change_with_old(
        Path::new("src/formatter.py"),
        4,
        "            if any(c < \" \" for c in key):",
        Some("            if any(c <= \" \" for c in key):"),
        &owners,
        &tests,
    ) else {
        return Err("changed predicate should classify".to_string());
    };
    assert_eq!(
        finding.class,
        ExposureClass::Exposed,
        "a control-flow operator change observed by an outcome oracle stays exposed"
    );
    Ok(())
}

#[test]
fn local_assignment_operator_change_does_not_credit_input_operand() -> Result<(), String> {
    // #1288: `total = base + bonus` -> `base - bonus` is a plain LOCAL ASSIGNMENT
    // with an empty token delta. `classify_probe_shape` defaults it to Control, so
    // the #1278 gate (keyed on delta_kind == Control) wrongly kept the operand
    // fallback and credited the UNCHANGED input `base`. The precise control-flow
    // line check must withhold the fallback for a non-control assignment.
    let source = "def compute(base, bonus):\n    total = base - bonus\n    return total\n";
    let owners = extract_owners(Path::new("src/calc.py"), source);
    let tests = extract_tests(
        Path::new("tests/test_calc.py"),
        "from src.calc import compute\n\n\ndef test_base_unchanged():\n    base = 10\n    compute(base, 3)\n    assert base == 10\n",
    );
    let Some(finding) = classify_change_with_old(
        Path::new("src/calc.py"),
        2,
        "    total = base - bonus",
        Some("    total = base + bonus"),
        &owners,
        &tests,
    ) else {
        return Err("changed local assignment should classify".to_string());
    };
    assert_ne!(
        finding.class,
        ExposureClass::Exposed,
        "a local-assignment operator change observed only via an unchanged input operand is not exposed"
    );
    assert_ne!(
        finding.oracle_alignment.as_deref(),
        Some("changed_sink_token")
    );
    Ok(())
}

#[test]
fn augmented_assignment_operator_change_does_not_credit_input_operand() -> Result<(), String> {
    // #1288: augmented assignment `acc += step` -> `acc -= step` likewise defaults
    // to Control in classify_probe_shape; the precise control-flow check must
    // withhold the operand fallback for the unchanged input `step`.
    let source = "def accumulate(values, step):\n    acc = 0\n    for value in values:\n        acc -= step\n    return acc\n";
    let owners = extract_owners(Path::new("src/agg.py"), source);
    let tests = extract_tests(
        Path::new("tests/test_agg.py"),
        "from src.agg import accumulate\n\n\ndef test_step_unchanged():\n    step = 2\n    accumulate([1, 2, 3], step)\n    assert step == 2\n",
    );
    let Some(finding) = classify_change_with_old(
        Path::new("src/agg.py"),
        4,
        "        acc -= step",
        Some("        acc += step"),
        &owners,
        &tests,
    ) else {
        return Err("changed augmented assignment should classify".to_string());
    };
    assert_ne!(
        finding.class,
        ExposureClass::Exposed,
        "an augmented-assignment operator change observed only via an unchanged input is not exposed"
    );
    Ok(())
}

#[test]
fn annotation_only_def_change_emits_no_probe() -> Result<(), String> {
    // #1289: changing only a parameter annotation (`int` -> `str`) has no runtime
    // behavior; Python does not enforce annotations. No probe must be emitted.
    let source = "def discount(amount: str) -> int:\n    return amount\n";
    let owners = extract_owners(Path::new("src/pricing.py"), source);
    let tests = extract_tests(
        Path::new("tests/test_pricing.py"),
        "from src.pricing import discount\n\n\ndef test_discount_passthrough():\n    assert discount(100) == 100\n",
    );
    let finding = classify_change_with_old(
        Path::new("src/pricing.py"),
        1,
        "def discount(amount: str) -> int:",
        Some("def discount(amount: int) -> int:"),
        &owners,
        &tests,
    );
    assert!(
        finding.is_none(),
        "an annotation-only def change carries no behavior delta and must emit no probe"
    );
    Ok(())
}

#[test]
fn return_annotation_only_change_emits_no_probe() -> Result<(), String> {
    // #1289: a return-annotation-only change is likewise a no-op.
    let source = "def parse(text):\n    return int(text)\n";
    let owners = extract_owners(Path::new("src/p.py"), source);
    let tests = extract_tests(
        Path::new("tests/test_p.py"),
        "from src.p import parse\n\n\ndef test_parse():\n    assert parse(\"4\") == 4\n",
    );
    let finding = classify_change_with_old(
        Path::new("src/p.py"),
        1,
        "def parse(text) -> str:",
        Some("def parse(text) -> int:"),
        &owners,
        &tests,
    );
    assert!(
        finding.is_none(),
        "a return-annotation-only change must emit no probe"
    );
    Ok(())
}

#[test]
fn default_value_change_in_def_still_classifies() -> Result<(), String> {
    // #1289 safety: a DEFAULT-VALUE change on a def header is behavioral and must
    // NOT be mistaken for an annotation-only change. The skeleton captures default
    // value source text, so this differs and is still analyzed.
    let source = "def page(size=20):\n    return size\n";
    let owners = extract_owners(Path::new("src/page.py"), source);
    let tests = extract_tests(
        Path::new("tests/test_page.py"),
        "from src.page import page\n\n\ndef test_page_default():\n    assert page() == 20\n",
    );
    let finding = classify_change_with_old(
        Path::new("src/page.py"),
        1,
        "def page(size=20):",
        Some("def page(size=10):"),
        &owners,
        &tests,
    );
    assert!(
        finding.is_some(),
        "a default-value change is behavioral and must still classify (not suppressed as annotation-only)"
    );
    Ok(())
}

#[test]
fn annotation_only_detection_is_conservative() {
    // Annotation-only (suppress):
    assert!(is_annotation_only_def_change(
        "def f(x: int):",
        "def f(x: str):"
    ));
    assert!(is_annotation_only_def_change(
        "def f(x) -> int:",
        "def f(x) -> str:"
    ));
    assert!(is_annotation_only_def_change(
        "    def m(self, a: int, b: Dict[str, int]) -> None:",
        "    def m(self, a: str, b: Dict[str, str]) -> None:"
    ));
    // NOT annotation-only (must still analyze):
    assert!(!is_annotation_only_def_change("def f(x=1):", "def f(x=2):")); // default value
    assert!(!is_annotation_only_def_change("def f(a):", "def f(b):")); // param rename
    assert!(!is_annotation_only_def_change("def f(a):", "def f(a, b):")); // added param
    assert!(!is_annotation_only_def_change(
        "def f(x):",
        "async def f(x):"
    )); // async-ness
    assert!(!is_annotation_only_def_change("def f(x):", "def f(x):")); // identical
    assert!(!is_annotation_only_def_change(
        "    return x + 1",
        "    return x - 1"
    )); // not a def
}

#[test]
fn bare_var_annotation_only_change_at_module_scope_emits_no_probe() -> Result<(), String> {
    // #1289: a module-scope annotated variable whose ONLY change is the
    // annotation (`int` -> `str`, value unchanged) has no runtime behavior —
    // Python does not enforce annotations at module scope. No probe.
    let source = "CACHE_TTL: str = 30\n\n\ndef get_ttl():\n    return CACHE_TTL\n";
    let owners = extract_owners(Path::new("src/config.py"), source);
    let tests = extract_tests(
        Path::new("tests/test_config.py"),
        "from src.config import get_ttl\n\n\ndef test_ttl():\n    assert get_ttl() == 30\n",
    );
    let finding = classify_change_with_old(
        Path::new("src/config.py"),
        1,
        "CACHE_TTL: str = 30",
        Some("CACHE_TTL: int = 30"),
        &owners,
        &tests,
    );
    assert!(
        finding.is_none(),
        "a module-scope annotation-only var change carries no behavior delta and must emit no probe"
    );
    Ok(())
}

#[test]
fn bare_var_annotation_only_change_no_value_emits_no_probe() -> Result<(), String> {
    // #1289: a pure annotation with no value (`x: int` -> `x: str`) is also a
    // no-op at module scope when only the annotation differs.
    let source = "LABEL: str\n\n\ndef get_label():\n    return LABEL\n";
    let owners = extract_owners(Path::new("src/config.py"), source);
    let tests = extract_tests(
        Path::new("tests/test_config.py"),
        "from src.config import get_label\n\n\ndef test_label():\n    assert get_label() == \"x\"\n",
    );
    let finding = classify_change_with_old(
        Path::new("src/config.py"),
        1,
        "LABEL: str",
        Some("LABEL: int"),
        &owners,
        &tests,
    );
    assert!(
        finding.is_none(),
        "a pure annotation change (no value) at module scope must emit no probe"
    );
    Ok(())
}

#[test]
fn bare_var_value_change_still_classifies() -> Result<(), String> {
    // #1289 safety: a VALUE change on an annotated variable is behavioral
    // and must NOT be suppressed. The skeleton captures the value source, so
    // `= 5` vs `= 6` differs and the line is still analyzed.
    let source = "LIMIT: int = 6\n\n\ndef get_limit():\n    return LIMIT\n";
    let owners = extract_owners(Path::new("src/config.py"), source);
    let tests = extract_tests(
        Path::new("tests/test_config.py"),
        "from src.config import get_limit\n\n\ndef test_limit():\n    assert get_limit() == 6\n",
    );
    let finding = classify_change_with_old(
        Path::new("src/config.py"),
        1,
        "LIMIT: int = 6",
        Some("LIMIT: int = 5"),
        &owners,
        &tests,
    );
    assert!(
        finding.is_some(),
        "a value change is behavioral and must still classify (not suppressed as annotation-only)"
    );
    Ok(())
}

#[test]
fn bare_var_annotation_change_in_class_body_still_classifies() -> Result<(), String> {
    // #1289 safety: an annotation-only change INSIDE a class body is NOT
    // suppressed — `@dataclass`/Pydantic make class-body annotations
    // runtime-meaningful, and base-class tracking does not exist yet. The
    // guard is module-scope only; fail closed for class bodies.
    let source =
        "class Config:\n    ttl: str = 30\n\n    def get(self):\n        return self.ttl\n";
    let owners = extract_owners(Path::new("src/config.py"), source);
    let tests = extract_tests(
        Path::new("tests/test_config.py"),
        "from src.config import Config\n\n\ndef test_ttl():\n    assert Config().get() == 30\n",
    );
    let finding = classify_change_with_old(
        Path::new("src/config.py"),
        2,
        "    ttl: str = 30",
        Some("    ttl: int = 30"),
        &owners,
        &tests,
    );
    assert!(
        finding.is_some(),
        "a class-body annotation-only change must still classify (fail closed for class bodies)"
    );
    Ok(())
}

#[test]
fn non_annotation_assignment_still_classifies() -> Result<(), String> {
    // #1289 safety: a plain assignment (`x = 5`, no annotation) is not an
    // annotated variable and must classify normally.
    let source = "COUNT = 6\n\n\ndef get_count():\n    return COUNT\n";
    let owners = extract_owners(Path::new("src/config.py"), source);
    let tests = extract_tests(
        Path::new("tests/test_config.py"),
        "from src.config import get_count\n\n\ndef test_count():\n    assert get_count() == 6\n",
    );
    let finding = classify_change_with_old(
        Path::new("src/config.py"),
        1,
        "COUNT = 6",
        Some("COUNT = 5"),
        &owners,
        &tests,
    );
    assert!(
        finding.is_some(),
        "a plain assignment (no annotation) must still classify"
    );
    Ok(())
}

#[test]
fn is_annotation_only_var_change_is_conservative() {
    // Annotation-only (suppress):
    assert!(is_annotation_only_var_change("x: int = 5", "x: str = 5")); // value identical
    assert!(is_annotation_only_var_change("x: int", "x: str")); // no value either side
    assert!(is_annotation_only_var_change(
        "MAX: List[int] = []",
        "MAX: List[str] = []"
    )); // subscripted annotation, value identical
    // NOT annotation-only (must still analyze):
    assert!(!is_annotation_only_var_change("x: int = 5", "x: int = 6")); // value changed
    assert!(!is_annotation_only_var_change("x: int", "x: int = 5")); // value added
    assert!(!is_annotation_only_var_change("x: int = 5", "x: int")); // value removed
    assert!(!is_annotation_only_var_change("a: int = 5", "b: int = 5")); // target rename
    assert!(!is_annotation_only_var_change("x: int = 5", "x: int = 5")); // identical
    assert!(!is_annotation_only_var_change("x = 5", "x = 6")); // not an annotation
    assert!(!is_annotation_only_var_change(
        "    return x + 1",
        "    return x - 1"
    )); // not an assignment at all
}

#[test]
fn dict_changed_element_sibling_key_oracle_not_exposed() -> Result<(), String> {
    // #1290: the changed key is `port`, but the only strong oracle observes the
    // unchanged SIBLING key `host`, so it does not discriminate the change.
    let source = "def build_config():\n    return {\"host\": \"localhost\", \"port\": 9090}\n";
    let owners = extract_owners(Path::new("src/conf.py"), source);
    let tests = extract_tests(
        Path::new("tests/test_conf.py"),
        "from src.conf import build_config\n\n\ndef test_host():\n    assert build_config()[\"host\"] == \"localhost\"\n",
    );
    let Some(finding) = classify_change_with_old(
        Path::new("src/conf.py"),
        2,
        "    return {\"host\": \"localhost\", \"port\": 9090}",
        Some("    return {\"host\": \"localhost\", \"port\": 8080}"),
        &owners,
        &tests,
    ) else {
        return Err("changed dict literal should classify".to_string());
    };
    assert_ne!(
        finding.class,
        ExposureClass::Exposed,
        "an oracle observing a sibling dict key does not discriminate the changed key"
    );
    Ok(())
}

#[test]
fn dict_changed_element_observed_value_stays_exposed() -> Result<(), String> {
    // #1290 preserve: the oracle observes the CHANGED key's new value, so it
    // genuinely discriminates the change.
    let source = "def build_config():\n    return {\"host\": \"localhost\", \"port\": 9090}\n";
    let owners = extract_owners(Path::new("src/conf.py"), source);
    let tests = extract_tests(
        Path::new("tests/test_conf.py"),
        "from src.conf import build_config\n\n\ndef test_port():\n    assert build_config()[\"port\"] == 9090\n",
    );
    let Some(finding) = classify_change_with_old(
        Path::new("src/conf.py"),
        2,
        "    return {\"host\": \"localhost\", \"port\": 9090}",
        Some("    return {\"host\": \"localhost\", \"port\": 8080}"),
        &owners,
        &tests,
    ) else {
        return Err("changed dict literal should classify".to_string());
    };
    assert_eq!(
        finding.class,
        ExposureClass::Exposed,
        "observing the changed key's value discriminates the change"
    );
    Ok(())
}

#[test]
fn list_changed_element_sibling_index_oracle_not_exposed() -> Result<(), String> {
    // #1290: index 1 changed (`search` -> `browse`), but the only strong oracle
    // observes the unchanged SIBLING index 0, so it does not discriminate.
    let source = "def route_order():\n    return [\"index\", \"browse\", \"detail\"]\n";
    let owners = extract_owners(Path::new("src/routes.py"), source);
    let tests = extract_tests(
        Path::new("tests/test_routes.py"),
        "from src.routes import route_order\n\n\ndef test_first():\n    assert route_order()[0] == \"index\"\n",
    );
    let Some(finding) = classify_change_with_old(
        Path::new("src/routes.py"),
        2,
        "    return [\"index\", \"browse\", \"detail\"]",
        Some("    return [\"index\", \"search\", \"detail\"]"),
        &owners,
        &tests,
    ) else {
        return Err("changed list literal should classify".to_string());
    };
    assert_ne!(
        finding.class,
        ExposureClass::Exposed,
        "an oracle observing a sibling list index does not discriminate the changed index"
    );
    Ok(())
}

#[test]
fn list_changed_element_observed_index_stays_exposed() -> Result<(), String> {
    // #1290 preserve: observing the changed index credits.
    let source = "def route_order():\n    return [\"index\", \"browse\", \"detail\"]\n";
    let owners = extract_owners(Path::new("src/routes.py"), source);
    let tests = extract_tests(
        Path::new("tests/test_routes.py"),
        "from src.routes import route_order\n\n\ndef test_second():\n    assert route_order()[1] == \"browse\"\n",
    );
    let Some(finding) = classify_change_with_old(
        Path::new("src/routes.py"),
        2,
        "    return [\"index\", \"browse\", \"detail\"]",
        Some("    return [\"index\", \"search\", \"detail\"]"),
        &owners,
        &tests,
    ) else {
        return Err("changed list literal should classify".to_string());
    };
    assert_eq!(
        finding.class,
        ExposureClass::Exposed,
        "observing the changed index discriminates the change"
    );
    Ok(())
}

#[test]
fn fstring_return_is_not_treated_as_dict_literal() -> Result<(), String> {
    // #1290 follow-up regression guard: an f-string `f"{value:.3f}"` contains `{`
    // and `}` but is NOT a dict literal — it must not be gated by the dict-element
    // check. A genuine f-string discriminator stays exposed.
    assert!(parse_dict_literal_fields("    return f\"{value:.3f}\"").is_none());
    assert!(
        dict_changed_keys_and_values(
            Some("    return f\"{value:.2f}\""),
            "    return f\"{value:.3f}\""
        )
        .is_none()
    );
    let source = "def render_price(value):\n    return f\"{value:.3f}\"\n";
    let owners = extract_owners(Path::new("src/price.py"), source);
    let tests = extract_tests(
        Path::new("tests/test_price.py"),
        "from src.price import render_price\n\n\ndef test_render_price_uses_three_decimals():\n    assert render_price(3.14159) == \"3.142\"\n",
    );
    let Some(finding) = classify_change_with_old(
        Path::new("src/price.py"),
        2,
        "    return f\"{value:.3f}\"",
        Some("    return f\"{value:.2f}\""),
        &owners,
        &tests,
    ) else {
        return Err("changed f-string should classify".to_string());
    };
    assert_eq!(
        finding.class,
        ExposureClass::Exposed,
        "a genuine f-string discriminator must not be downgraded by the dict-element gate"
    );
    Ok(())
}

#[test]
fn fstring_length_invariant_change_via_len_aggregate_not_exposed() -> Result<(), String> {
    // #1290 1b: `f"OK:{code}"` -> `f"NO:{code}"` changes only equal-length literal
    // text (interpolation unchanged), so output length is invariant. The only
    // strong oracle is `len(...)`, which cannot discriminate it.
    let source = "def status_label(code):\n    return f\"NO:{code}\"\n";
    let owners = extract_owners(Path::new("src/status.py"), source);
    let tests = extract_tests(
        Path::new("tests/test_status.py"),
        "from src.status import status_label\n\n\ndef test_len():\n    assert len(status_label(7)) == 4\n",
    );
    let Some(finding) = classify_change_with_old(
        Path::new("src/status.py"),
        2,
        "    return f\"NO:{code}\"",
        Some("    return f\"OK:{code}\""),
        &owners,
        &tests,
    ) else {
        return Err("changed f-string should classify".to_string());
    };
    assert_ne!(
        finding.class,
        ExposureClass::Exposed,
        "a length-invariant f-string change observed only via len() is not discriminated"
    );
    Ok(())
}

#[test]
fn fstring_length_invariant_change_with_string_oracle_stays_exposed() -> Result<(), String> {
    // #1290 1b preserve: the same length-invariant change observed by an exact
    // string comparison IS discriminated.
    let source = "def status_label(code):\n    return f\"NO:{code}\"\n";
    let owners = extract_owners(Path::new("src/status.py"), source);
    let tests = extract_tests(
        Path::new("tests/test_status.py"),
        "from src.status import status_label\n\n\ndef test_exact():\n    assert status_label(7) == \"NO:7\"\n",
    );
    let Some(finding) = classify_change_with_old(
        Path::new("src/status.py"),
        2,
        "    return f\"NO:{code}\"",
        Some("    return f\"OK:{code}\""),
        &owners,
        &tests,
    ) else {
        return Err("changed f-string should classify".to_string());
    };
    assert_eq!(
        finding.class,
        ExposureClass::Exposed,
        "an exact string oracle observes the changed f-string output"
    );
    Ok(())
}

#[test]
fn error_path_change_with_value_oracle_not_exposed() -> Result<(), String> {
    // #1290 Class C: a `raise` type change on an untaken branch, observed only by a
    // normal-path value oracle (the test never triggers the raise), is not
    // discriminated.
    let source = "def parse(text):\n    if not text:\n        raise KeyError(\"empty\")\n    return int(text)\n";
    let owners = extract_owners(Path::new("src/parseint.py"), source);
    let tests = extract_tests(
        Path::new("tests/test_parseint.py"),
        "from src.parseint import parse\n\n\ndef test_parse_ok():\n    assert parse(\"42\") == 42\n",
    );
    let Some(finding) = classify_change_with_old(
        Path::new("src/parseint.py"),
        3,
        "        raise KeyError(\"empty\")",
        Some("        raise ValueError(\"empty\")"),
        &owners,
        &tests,
    ) else {
        return Err("changed raise should classify".to_string());
    };
    assert_ne!(
        finding.class,
        ExposureClass::Exposed,
        "a raise change observed only by a normal-path value oracle is not discriminated"
    );
    Ok(())
}

#[test]
fn error_path_change_with_exception_oracle_stays_exposed() -> Result<(), String> {
    // #1290 Class C preserve: the same raise change IS exposed when the test
    // observes the raised exception via pytest.raises.
    let source = "def parse(text):\n    if not text:\n        raise KeyError(\"empty\")\n    return int(text)\n";
    let owners = extract_owners(Path::new("src/parseint.py"), source);
    let tests = extract_tests(
        Path::new("tests/test_parseint.py"),
        "import pytest\n\nfrom src.parseint import parse\n\n\ndef test_parse_empty():\n    with pytest.raises(KeyError, match=\"empty\"):\n        parse(\"\")\n",
    );
    let Some(finding) = classify_change_with_old(
        Path::new("src/parseint.py"),
        3,
        "        raise KeyError(\"empty\")",
        Some("        raise ValueError(\"empty\")"),
        &owners,
        &tests,
    ) else {
        return Err("changed raise should classify".to_string());
    };
    assert_eq!(
        finding.class,
        ExposureClass::Exposed,
        "an exception oracle observes the changed raise"
    );
    Ok(())
}

#[test]
fn changed_default_explicit_kwarg_override_not_exposed() -> Result<(), String> {
    // #1289 trap 45: the `verbose` default changes (False -> True), but the only
    // strong oracle calls `render("Sam", verbose=False)`, explicitly overriding
    // the parameter. The changed default is never exercised, so the test passes
    // identically before and after — not discriminated.
    let source =
        "def render(name, verbose=True):\n    return f\"[debug] {name}\" if verbose else name\n";
    let owners = extract_owners(Path::new("src/render.py"), source);
    let tests = extract_tests(
        Path::new("tests/test_render.py"),
        "from src.render import render\n\n\ndef test_render_explicit_verbose_false():\n    assert render(\"Sam\", verbose=False) == \"Sam\"\n",
    );
    let Some(finding) = classify_change_with_old(
        Path::new("src/render.py"),
        1,
        "def render(name, verbose=True):",
        Some("def render(name, verbose=False):"),
        &owners,
        &tests,
    ) else {
        return Err("a default-value change should classify".to_string());
    };
    assert_eq!(
        finding.class,
        ExposureClass::WeaklyExposed,
        "an explicit kwarg override does not exercise the changed default"
    );
    assert!(
        finding
            .missing
            .iter()
            .any(|entry| entry.contains("without `verbose`")),
        "the downgrade must name the parameter to test by omission"
    );
    assert!(
        finding
            .activation
            .missing_discriminators
            .iter()
            .any(|fact| fact.value == "call `render` without `verbose`"),
        "the structured missing discriminator must carry the omission guidance"
    );
    Ok(())
}

#[test]
fn changed_default_explicit_positional_override_not_exposed() -> Result<(), String> {
    // #1289 trap 45: a positional argument at `verbose`'s index (1) overrides the
    // changed default just as a kwarg would.
    let source =
        "def render(name, verbose=True):\n    return f\"[debug] {name}\" if verbose else name\n";
    let owners = extract_owners(Path::new("src/render.py"), source);
    let tests = extract_tests(
        Path::new("tests/test_render.py"),
        "from src.render import render\n\n\ndef test_render_positional_false():\n    assert render(\"Sam\", False) == \"Sam\"\n",
    );
    let Some(finding) = classify_change_with_old(
        Path::new("src/render.py"),
        1,
        "def render(name, verbose=True):",
        Some("def render(name, verbose=False):"),
        &owners,
        &tests,
    ) else {
        return Err("a default-value change should classify".to_string());
    };
    assert_eq!(
        finding.class,
        ExposureClass::WeaklyExposed,
        "an explicit positional override does not exercise the changed default"
    );
    assert!(
        finding
            .missing
            .iter()
            .any(|entry| entry.contains("without `verbose`")),
        "the downgrade must name the parameter to test by omission"
    );
    assert!(
        finding
            .activation
            .missing_discriminators
            .iter()
            .any(|fact| fact.value == "call `render` without `verbose`"),
        "the structured missing discriminator must carry the omission guidance"
    );
    Ok(())
}

#[test]
fn changed_default_used_by_omission_stays_exposed() -> Result<(), String> {
    // #1289 trap 45 preserve: when the call OMITS the parameter, the changed
    // default IS exercised, and a strong oracle observing the output discriminates
    // it. Must stay exposed.
    let source =
        "def render(name, verbose=True):\n    return f\"[debug] {name}\" if verbose else name\n";
    let owners = extract_owners(Path::new("src/render.py"), source);
    let tests = extract_tests(
        Path::new("tests/test_render.py"),
        "from src.render import render\n\n\ndef test_render_default_verbose():\n    assert render(\"Sam\") == \"[debug] Sam\"\n",
    );
    let Some(finding) = classify_change_with_old(
        Path::new("src/render.py"),
        1,
        "def render(name, verbose=True):",
        Some("def render(name, verbose=False):"),
        &owners,
        &tests,
    ) else {
        return Err("a default-value change should classify".to_string());
    };
    assert_eq!(
        finding.class,
        ExposureClass::Exposed,
        "omitting the parameter exercises the changed default under a strong oracle"
    );
    Ok(())
}

#[test]
fn changed_default_value_params_detects_pure_value_change_only() -> Result<(), String> {
    // Pure default-value change -> the changed parameter is reported.
    let Some(changed) = changed_default_value_params(
        "def render(name, verbose=False):",
        "def render(name, verbose=True):",
    ) else {
        return Err("a value-to-value default change is a pure default-value change".to_string());
    };
    assert_eq!(changed.len(), 1);
    assert_eq!(changed[0].name, "verbose");
    assert_eq!(changed[0].index, 1);
    assert!(changed[0].positionally_bindable);
    // No default change at all -> None.
    assert!(changed_default_value_params("def f(x=1):", "def f(x=1):").is_none());
    // Added default (requiredness change) -> None, fail open.
    assert!(changed_default_value_params("def f(x):", "def f(x=1):").is_none());
    // Removed default -> None, fail open.
    assert!(changed_default_value_params("def f(x=1):", "def f(x):").is_none());
    // Param rename alongside a default change -> None (not a pure value change).
    assert!(changed_default_value_params("def f(a=1):", "def f(b=2):").is_none());
    // Added parameter -> None.
    assert!(changed_default_value_params("def f(x=1):", "def f(x=1, y=2):").is_none());
    // Not a def header -> None.
    assert!(changed_default_value_params("    return x + 1", "    return x - 1").is_none());
    Ok(())
}

#[test]
fn analyze_call_args_classifies_positional_and_keyword() -> Result<(), String> {
    let Some(shape) = analyze_call_args("\"Sam\", verbose=False") else {
        return Err("positional + kwarg call is tractable".to_string());
    };
    assert_eq!(shape.positional_count, 1);
    assert_eq!(shape.keywords, vec!["verbose".to_string()]);
    // Comparison operators are not keyword bindings.
    let Some(cmp) = analyze_call_args("x == 1, y") else {
        return Err("comparison-operand call is tractable".to_string());
    };
    assert_eq!(cmp.positional_count, 2);
    assert!(cmp.keywords.is_empty());
    // Nested calls and brackets stay one positional argument each.
    let Some(nested) = analyze_call_args("g(a, b), [1, 2], k=3") else {
        return Err("nested-argument call is tractable".to_string());
    };
    assert_eq!(nested.positional_count, 2);
    assert_eq!(nested.keywords, vec!["k".to_string()]);
    // *args / **kwargs unpacking is undecidable -> None (fail open).
    assert!(analyze_call_args("*args").is_none());
    assert!(analyze_call_args("a, **kwargs").is_none());
    // An inline `# comment` in the arglist makes binding ambiguous -> None
    // (fail open, never a false-clean from a comment-parsed `)` or text).
    assert!(analyze_call_args("a  # note with ) paren").is_none());
    Ok(())
}

#[test]
fn free_function_call_arglists_matches_only_direct_calls() {
    let body =
        "render(\"Sam\")\nobj.render(\"X\")\nrenderer(\"Y\")\nrender(\"Z\", verbose=False)\n";
    let calls = free_function_call_arglists(body, "render");
    // `obj.render(...)` (method access) and `renderer(...)` (longer name) excluded.
    assert_eq!(calls, vec!["\"Sam\"", "\"Z\", verbose=False"]);
}

#[test]
fn free_function_call_arglists_skips_comment_and_string_mentions() {
    // A `# comment` or string literal that mentions the owner name must not be
    // read as a live call: a comment with `)` would otherwise break paren
    // matching and a string mention would invent a call that does not run.
    let body = "render(\"Sam\")\n# see also render(other)\nx = \"render(unparsed)\"\n";
    let calls = free_function_call_arglists(body, "render");
    // Only the first, real call is captured.
    assert_eq!(calls, vec!["\"Sam\""]);
}

#[test]
fn fstring_format_spec_change_is_not_length_invariant() {
    // #1290 1b: a format-spec change alters an interpolation, so it is NOT
    // length-invariant — a `len` oracle could discriminate it, and it must not be
    // gated. (Trap 58 / 53 preservation.)
    assert!(!fstring_change_is_length_invariant(
        "    return f\"{value:.2f}\"",
        "    return f\"{value:.3f}\""
    ));
    // Equal-length literal-only change IS length-invariant.
    assert!(fstring_change_is_length_invariant(
        "    return f\"OK:{code}\"",
        "    return f\"NO:{code}\""
    ));
    // A literal change that alters length is NOT invariant (len can discriminate).
    assert!(!fstring_change_is_length_invariant(
        "    return f\"{x}\"",
        "    return f\"{x}!\""
    ));
    // Non-f-string lines are never invariant.
    assert!(!fstring_change_is_length_invariant(
        "    return x + 1",
        "    return x - 1"
    ));
    // Fail open on unsupported shapes (escaped / nested braces) -> no template.
    assert_eq!(fstring_template("    return f\"{{OK}}:{code}\""), None);
    assert_eq!(fstring_template("    return f\"{value:{width}}\""), None);
    assert!(!fstring_change_is_length_invariant(
        "    return f\"{{OK}}:{code}\"",
        "    return f\"{{NO}}:{code}\""
    ));
}

#[test]
fn oracle_pure_len_aggregate_detection() {
    // Pure length-only observation:
    assert!(oracle_is_pure_len_aggregate(
        "len(status_label(7)) == 4",
        "NO:"
    ));
    // Also observes the output exactly -> NOT pure aggregate (keeps credit):
    assert!(!oracle_is_pure_len_aggregate(
        "len(status_label(7)) == 4 and status_label(7) == \"NO:7\"",
        "NO:"
    ));
    // Contains the changed literal -> NOT pure aggregate:
    assert!(!oracle_is_pure_len_aggregate(
        "status_label(7).startswith(\"NO:\")",
        "NO:"
    ));
    // No len at all -> not a len aggregate:
    assert!(!oracle_is_pure_len_aggregate(
        "status_label(7) == \"NO:7\"",
        "NO:"
    ));
}

#[test]
fn fstring_len_plus_exact_oracle_stays_exposed() -> Result<(), String> {
    // #1290 1b hardening: when the test observes BOTH len() AND the exact output
    // (in separate assertions), the exact-value assertion is the strong oracle and
    // must keep the credit — the len-aggregate gate must not downgrade it. (A single
    // `assert a and b` is `smoke_only`/weak for an unrelated reason, so the
    // discriminating form uses separate assertions.)
    let source = "def status_label(code):\n    return f\"NO:{code}\"\n";
    let owners = extract_owners(Path::new("src/status.py"), source);
    let tests = extract_tests(
        Path::new("tests/test_status.py"),
        "from src.status import status_label\n\n\ndef test_both():\n    assert len(status_label(7)) == 4\n    assert status_label(7) == \"NO:7\"\n",
    );
    let Some(finding) = classify_change_with_old(
        Path::new("src/status.py"),
        2,
        "    return f\"NO:{code}\"",
        Some("    return f\"OK:{code}\""),
        &owners,
        &tests,
    ) else {
        return Err("changed f-string should classify".to_string());
    };
    assert_eq!(
        finding.class,
        ExposureClass::Exposed,
        "an oracle that also observes the exact output is not a pure len aggregate"
    );
    Ok(())
}

#[test]
fn dict_changed_element_whole_comparison_stays_exposed() -> Result<(), String> {
    // #1290 preserve: a whole-collection comparison observes every element,
    // including the changed one.
    let source = "def build_config():\n    return {\"host\": \"localhost\", \"port\": 9090}\n";
    let owners = extract_owners(Path::new("src/conf.py"), source);
    let tests = extract_tests(
        Path::new("tests/test_conf.py"),
        "from src.conf import build_config\n\n\ndef test_cfg():\n    assert build_config() == {\"host\": \"localhost\", \"port\": 9090}\n",
    );
    let Some(finding) = classify_change_with_old(
        Path::new("src/conf.py"),
        2,
        "    return {\"host\": \"localhost\", \"port\": 9090}",
        Some("    return {\"host\": \"localhost\", \"port\": 8080}"),
        &owners,
        &tests,
    ) else {
        return Err("changed dict literal should classify".to_string());
    };
    assert_eq!(
        finding.class,
        ExposureClass::Exposed,
        "a whole-collection comparison observes the changed element"
    );
    Ok(())
}

#[test]
fn noop_docstring_only_change_emits_no_probe() -> Result<(), String> {
    // #1279: a docstring-only change has no behavior delta. Even though the
    // strong `== 80` oracle observes the owner's output, there is nothing for
    // the test to discriminate, so no behavior probe must be emitted.
    let source = "def discount(price):\n    \"\"\"Apply the standard discount to a price.\"\"\"\n    return price * 0.8\n";
    let owners = extract_owners(Path::new("src/pricing.py"), source);
    let tests = extract_tests(
        Path::new("tests/test_pricing.py"),
        "from src.pricing import discount\n\n\ndef test_discount():\n    assert discount(100) == 80\n",
    );
    let finding = classify_change_with_old(
        Path::new("src/pricing.py"),
        2,
        "    \"\"\"Apply the standard discount to a price.\"\"\"",
        Some("    \"Apply a discount.\""),
        &owners,
        &tests,
    );
    assert!(
        finding.is_none(),
        "a docstring-only change carries no behavior delta and must emit no probe"
    );
    Ok(())
}

#[test]
fn noop_comment_only_change_emits_no_probe() -> Result<(), String> {
    // #1279: a comment-only change is likewise a no-op.
    let source =
        "def discount(price):\n    # apply the standard discount\n    return price * 0.8\n";
    let owners = extract_owners(Path::new("src/pricing.py"), source);
    let tests = extract_tests(
        Path::new("tests/test_pricing.py"),
        "from src.pricing import discount\n\n\ndef test_discount():\n    assert discount(100) == 80\n",
    );
    let finding = classify_change_with_old(
        Path::new("src/pricing.py"),
        2,
        "    # apply the standard discount",
        Some("    # apply a discount"),
        &owners,
        &tests,
    );
    assert!(
        finding.is_none(),
        "a comment-only change carries no behavior delta and must emit no probe"
    );
    Ok(())
}

#[test]
fn real_body_change_in_same_function_still_classifies() -> Result<(), String> {
    // #1279 inverse control: the no-op guard must not suppress a genuine body
    // change. The same `discount` owner with a real return-value edit still
    // classifies `exposed` under the strong output oracle.
    let source = "def discount(price):\n    return price * 0.8\n";
    let owners = extract_owners(Path::new("src/pricing.py"), source);
    let tests = extract_tests(
        Path::new("tests/test_pricing.py"),
        "from src.pricing import discount\n\n\ndef test_discount():\n    assert discount(100) == 80\n",
    );
    let Some(finding) = classify_change_with_old(
        Path::new("src/pricing.py"),
        2,
        "    return price * 0.8",
        Some("    return price * 0.9"),
        &owners,
        &tests,
    ) else {
        return Err("a real body change must still classify".to_string());
    };
    assert_eq!(
        finding.class,
        ExposureClass::Exposed,
        "a real return-value change observed by a strong oracle stays exposed"
    );
    Ok(())
}

#[test]
fn no_behavior_line_detection_is_conservative() {
    // No-op shapes:
    assert!(is_python_no_behavior_line("    \"\"\"A docstring.\"\"\""));
    assert!(is_python_no_behavior_line("'''triple single'''"));
    assert!(is_python_no_behavior_line("    \"single line string\""));
    assert!(is_python_no_behavior_line("    # a comment"));
    assert!(is_python_no_behavior_line("   "));
    assert!(is_python_no_behavior_line("r\"raw docstring\""));
    assert!(is_python_no_behavior_line("b\"bytes literal\""));
    // Behavioral shapes must NOT be treated as no-ops:
    assert!(!is_python_no_behavior_line("    return \"x\""));
    assert!(!is_python_no_behavior_line("    result = \"x\""));
    assert!(!is_python_no_behavior_line("    raise ValueError(\"x\")"));
    assert!(!is_python_no_behavior_line("    f\"{compute()}\""));
    assert!(!is_python_no_behavior_line("    rf\"{compute()}\""));
    assert!(!is_python_no_behavior_line("    \"a\" + str(x)"));
    assert!(!is_python_no_behavior_line("    return price * 0.8"));
}

#[test]
fn classify_change_exposed_boundary_does_not_emit_missing_discriminator() -> Result<(), String> {
    let owners = extract_owners(
        Path::new("src/discount.py"),
        "def apply_discount(amount, threshold):\n    if amount >= threshold:\n        return amount - 10\n    return amount\n",
    );
    let tests = extract_tests(
        Path::new("tests/test_discount.py"),
        "from src.discount import apply_discount\n\ndef test_apply_discount_boundary():\n    assert apply_discount(100, 100) == 90\n",
    );

    let Some(finding) = classify_change(
        Path::new("src/discount.py"),
        2,
        "    if amount >= threshold:",
        &owners,
        &tests,
    ) else {
        return Err("changed predicate inside owner should classify".to_string());
    };

    assert_eq!(finding.class, ExposureClass::Exposed);
    assert!(finding.activation.missing_discriminators.is_empty());
    assert!(
        finding
            .evidence
            .iter()
            .all(|entry| !entry.starts_with("missing_discriminator:"))
    );
    Ok(())
}

#[test]
fn classify_change_returns_weakly_exposed_when_related_test_exists() -> Result<(), String> {
    let owners = extract_owners(
        Path::new("src/pricing.py"),
        "def apply_discount(amount, threshold):\n    if amount >= threshold:\n        return amount - 10\n    return amount\n",
    );
    let tests = extract_tests(
        Path::new("tests/test_pricing.py"),
        "def test_apply_discount():\n    result = apply_discount(100, 50)\n",
    );

    let Some(finding) = classify_change(
        Path::new("src/pricing.py"),
        2,
        "    if amount >= threshold:",
        &owners,
        &tests,
    ) else {
        return Err("changed line inside owner should classify".to_string());
    };

    assert_eq!(finding.class, ExposureClass::WeaklyExposed);
    assert_eq!(finding.language, Some(DomainLanguageId::Python));
    assert_eq!(finding.language_status, Some(LanguageStatus::Preview));
    assert_eq!(finding.owner_kind, Some(OwnerKind::Function));
    assert_eq!(finding.ripr.reach.state, StageState::Yes);
    assert_eq!(finding.ripr.infect.state, StageState::Yes);
    assert_eq!(finding.ripr.propagate.state, StageState::Weak);
    assert_eq!(finding.ripr.reveal.observe.state, StageState::Weak);
    assert_eq!(finding.ripr.reveal.discriminate.state, StageState::Weak);
    assert_eq!(finding.related_tests.len(), 1);
    assert_eq!(finding.flow_sinks.len(), 1);
    assert_eq!(finding.flow_sinks[0].kind, FlowSinkKind::Unknown);
    assert_eq!(finding.activation.missing_discriminators.len(), 1);
    assert_eq!(
        finding.activation.missing_discriminators[0].value,
        "amount == threshold"
    );
    assert!(
        finding
            .evidence
            .iter()
            .any(|entry| entry == "missing_discriminator: amount == threshold")
    );
    assert!(finding.canonical_gap.is_some());
    assert!(finding.recommended_next_step.is_some());
    Ok(())
}

#[test]
fn classify_exception_match_assertion_as_exposed() -> Result<(), String> {
    let finding = classify_change(
        Path::new("src/validation.py"),
        3,
        "        raise ValueError(\"positive required\")",
        &extract_owners(
            Path::new("src/validation.py"),
            "def require_positive(value):\n    if value <= 0:\n        raise ValueError(\"positive required\")\n    return value\n",
        ),
        &extract_tests(
            Path::new("tests/test_validation.py"),
            "import pytest\nfrom src.validation import require_positive\n\n\
                 def test_rejects_zero_value():\n    with pytest.raises(ValueError, match=\"positive required\"):\n        require_positive(0)\n",
        ),
    )
    .ok_or_else(|| "exception-path change should classify".to_string())?;
    assert_eq!(finding.class, ExposureClass::Exposed);
    assert!(missing_discriminator_values(&finding).is_empty());
    assert_eq!(
        finding.related_tests[0].oracle_kind,
        OracleKind::ExactErrorVariant
    );
    assert_eq!(
        finding.related_tests[0].oracle_strength,
        OracleStrength::Strong
    );
    Ok(())
}

#[test]
fn classify_field_assertion_as_exposed() -> Result<(), String> {
    let finding = classify_change(
        Path::new("src/invoice.py"),
        2,
        "    return {\"status\": \"paid\", \"id\": invoice_id}",
        &extract_owners(
            Path::new("src/invoice.py"),
            "def invoice_payload(invoice_id):\n    return {\"status\": \"paid\", \"id\": invoice_id}\n",
        ),
        &extract_tests(
            Path::new("tests/test_invoice.py"),
            "from src.invoice import invoice_payload\n\n\
                 def test_invoice_payload_status():\n    payload = invoice_payload(\"inv-123\")\n    assert payload[\"status\"] == \"paid\"\n",
        ),
    )
    .ok_or_else(|| "field-value change should classify".to_string())?;
    assert_eq!(finding.class, ExposureClass::Exposed);
    assert!(missing_discriminator_values(&finding).is_empty());
    assert_eq!(finding.related_tests[0].oracle_kind, OracleKind::ExactValue);
    assert_eq!(
        finding.related_tests[0].oracle_strength,
        OracleStrength::Strong
    );
    Ok(())
}

#[test]
fn classify_output_assertion_as_exposed() -> Result<(), String> {
    let finding = classify_change(
        Path::new("src/notifications.py"),
        5,
        "    logger.warning(\"coupon expired\")",
        &extract_owners(
            Path::new("src/notifications.py"),
            "import logging\n\nlogger = logging.getLogger(__name__)\n\ndef warn_coupon():\n    logger.warning(\"coupon expired\")\n",
        ),
        &extract_tests(
            Path::new("tests/test_notifications.py"),
            "from src.notifications import warn_coupon\n\n\
                 def test_warn_coupon_exact_output(caplog):\n    warn_coupon()\n    assert caplog.text == \"coupon expired\"\n",
        ),
    )
    .ok_or_else(|| "output/log change should classify".to_string())?;
    assert_eq!(finding.class, ExposureClass::Exposed);
    assert!(missing_discriminator_values(&finding).is_empty());
    assert_eq!(finding.related_tests[0].oracle_kind, OracleKind::ExactValue);
    assert_eq!(
        finding.related_tests[0].oracle_strength,
        OracleStrength::Strong
    );
    Ok(())
}

#[test]
fn classify_click_output_change_as_repairable_cli_gap() -> Result<(), String> {
    let finding = classify_change(
        Path::new("src/commands.py"),
        5,
        "    click.echo(\"shipment queued\")",
        &extract_owners(
            Path::new("src/commands.py"),
            "import click\n\n@click.command()\ndef ship():\n    click.echo(\"shipment queued\")\n",
        ),
        &extract_tests(
            Path::new("tests/test_commands.py"),
            "from src.commands import ship\n\n\
                 def test_ship_smoke(capsys):\n    ship()\n    captured = capsys.readouterr()\n    assert captured.out\n",
        ),
    )
    .ok_or_else(|| "click output change should classify".to_string())?;

    assert_eq!(finding.class, ExposureClass::WeaklyExposed);
    assert_eq!(finding.static_limit_kind, None);
    assert_eq!(
        missing_discriminator_values(&finding),
        vec!["output contains \"shipment queued\""]
    );
    assert_eq!(
        evidence_value(&finding, "suggested_verify_command: "),
        Some("pytest tests/test_commands.py::test_ship_smoke")
    );
    Ok(())
}

#[test]
fn classify_change_emits_first_python_repair_class_discriminators() -> Result<(), String> {
    let return_finding = classify_change(
        Path::new("src/priority.py"),
        2,
        "    return amount >= 100",
        &extract_owners(
            Path::new("src/priority.py"),
            "def is_priority(amount):\n    return amount >= 100\n",
        ),
        &extract_tests(
            Path::new("tests/test_priority.py"),
            "from src.priority import is_priority\n\n\
                 def test_priority_amount():\n    assert is_priority(150)\n",
        ),
    )
    .ok_or_else(|| "return-value change should classify".to_string())?;
    assert_eq!(return_finding.class, ExposureClass::WeaklyExposed);
    assert_eq!(
        missing_discriminator_values(&return_finding),
        vec!["return value == amount >= 100"]
    );
    assert!(
        return_finding
            .recommended_next_step
            .as_deref()
            .is_some_and(|step| step.contains("return-value assertion"))
    );

    let exception_finding = classify_change(
        Path::new("src/validation.py"),
        3,
        "        raise ValueError(\"positive required\")",
        &extract_owners(
            Path::new("src/validation.py"),
            "def require_positive(value):\n    if value <= 0:\n        raise ValueError(\"positive required\")\n    return value\n",
        ),
        &extract_tests(
            Path::new("tests/test_validation.py"),
            "import pytest\nfrom src.validation import require_positive\n\n\
                 def test_rejects_zero_value():\n    with pytest.raises(ValueError):\n        require_positive(0)\n",
        ),
    )
    .ok_or_else(|| "exception-path change should classify".to_string())?;
    assert_eq!(exception_finding.class, ExposureClass::WeaklyExposed);
    assert_eq!(
        missing_discriminator_values(&exception_finding),
        vec!["raises ValueError matching \"positive required\""]
    );
    assert!(
        exception_finding
            .recommended_next_step
            .as_deref()
            .is_some_and(|step| step.contains("exception assertion"))
    );

    let field_finding = classify_change(
        Path::new("src/invoice.py"),
        3,
        "        self.status = \"paid\"",
        &extract_owners(
            Path::new("src/invoice.py"),
            "class Invoice:\n    def mark_paid(self):\n        self.status = \"paid\"\n",
        ),
        &extract_tests(
            Path::new("tests/test_invoice.py"),
            "from src.invoice import Invoice\n\n\
                 def test_mark_paid_smoke():\n    invoice = Invoice()\n    invoice.mark_paid()\n    assert invoice\n",
        ),
    )
    .ok_or_else(|| "field-value change should classify".to_string())?;
    assert_eq!(field_finding.class, ExposureClass::WeaklyExposed);
    assert_eq!(
        missing_discriminator_values(&field_finding),
        vec!["self.status == \"paid\""]
    );
    assert!(
        field_finding
            .recommended_next_step
            .as_deref()
            .is_some_and(|step| step.contains("field/object assertion"))
    );

    let output_finding = classify_change(
        Path::new("src/notifications.py"),
        5,
        "    logger.warning(\"coupon expired\")",
        &extract_owners(
            Path::new("src/notifications.py"),
            "import logging\n\nlogger = logging.getLogger(__name__)\n\ndef warn_coupon():\n    logger.warning(\"coupon expired\")\n",
        ),
        &extract_tests(
            Path::new("tests/test_notifications.py"),
            "from src.notifications import warn_coupon\n\n\
                 def test_warn_coupon_smoke(caplog):\n    warn_coupon()\n    assert caplog.text\n",
        ),
    )
    .ok_or_else(|| "output/log change should classify".to_string())?;
    assert_eq!(output_finding.class, ExposureClass::WeaklyExposed);
    assert_eq!(
        missing_discriminator_values(&output_finding),
        vec!["log contains \"coupon expired\""]
    );
    assert!(
        output_finding
            .recommended_next_step
            .as_deref()
            .is_some_and(|step| step.contains("output/log/call-effect assertion"))
    );

    Ok(())
}

#[test]
fn classify_change_emits_python_repair_placement_and_verify_command() -> Result<(), String> {
    let pytest_finding = classify_change(
        Path::new("src/pricing.py"),
        2,
        "    if amount >= threshold:",
        &extract_owners(
            Path::new("src/pricing.py"),
            "def calculate_discount(amount, threshold):\n    if amount >= threshold:\n        return amount - 10\n    return amount\n",
        ),
        &extract_tests(
            Path::new("tests/test_pricing.py"),
            "from src.pricing import calculate_discount\n\n\
                 def test_calculate_discount_smoke():\n    result = calculate_discount(150, 100)\n    assert result\n",
        ),
    )
    .ok_or_else(|| "pytest boundary change should classify".to_string())?;
    assert_eq!(pytest_finding.class, ExposureClass::WeaklyExposed);
    assert_eq!(
        evidence_value(&pytest_finding, "suggested_repair_action: "),
        Some("strengthen_existing_test")
    );
    assert_eq!(
        evidence_value(&pytest_finding, "suggested_test_file: "),
        Some("tests/test_pricing.py")
    );
    assert_eq!(
        evidence_value(&pytest_finding, "suggested_test_name: "),
        Some("test_calculate_discount_smoke")
    );
    assert_eq!(
        evidence_value(&pytest_finding, "suggested_test_node_id: "),
        Some("tests/test_pricing.py::test_calculate_discount_smoke")
    );
    assert_eq!(
        evidence_value(&pytest_finding, "suggested_verify_command: "),
        Some("pytest tests/test_pricing.py::test_calculate_discount_smoke")
    );
    assert_eq!(
        evidence_value(&pytest_finding, "suggested_verify_command_confidence: "),
        Some("high")
    );

    let unittest_finding = classify_change(
        Path::new("src/validation.py"),
        3,
        "        raise ValueError(\"positive required\")",
        &extract_owners(
            Path::new("src/validation.py"),
            "def require_positive(value):\n    if value <= 0:\n        raise ValueError(\"positive required\")\n    return value\n",
        ),
        &extract_tests(
            Path::new("tests/test_validation.py"),
            "import unittest\nfrom src.validation import require_positive\n\n\
                 class TestValidation(unittest.TestCase):\n    def test_rejects_zero_value(self):\n        with self.assertRaises(ValueError):\n            require_positive(0)\n",
        ),
    )
    .ok_or_else(|| "unittest exception change should classify".to_string())?;
    assert_eq!(unittest_finding.class, ExposureClass::WeaklyExposed);
    assert_eq!(
        evidence_value(&unittest_finding, "suggested_repair_action: "),
        Some("strengthen_existing_test")
    );
    assert_eq!(
        evidence_value(&unittest_finding, "suggested_test_file: "),
        Some("tests/test_validation.py")
    );
    assert_eq!(
        evidence_value(&unittest_finding, "suggested_test_name: "),
        Some("test_rejects_zero_value")
    );
    assert_eq!(
        evidence_value(&unittest_finding, "suggested_test_node_id: "),
        None
    );
    assert_eq!(
        evidence_value(&unittest_finding, "suggested_verify_command: "),
        Some("python -m unittest tests.test_validation.TestValidation.test_rejects_zero_value")
    );
    assert_eq!(
        evidence_value(&unittest_finding, "suggested_verify_command_confidence: "),
        Some("high")
    );

    Ok(())
}

#[test]
fn classify_change_suppresses_repair_guidance_for_non_actionable_python_cases() -> Result<(), String>
{
    let exposed = classify_change(
        Path::new("src/pricing.py"),
        2,
        "    if amount >= threshold:",
        &extract_owners(
            Path::new("src/pricing.py"),
            "def apply_discount(amount, threshold):\n    if amount >= threshold:\n        return amount - 10\n    return amount\n",
        ),
        &extract_tests(
            Path::new("tests/test_pricing.py"),
            "from src.pricing import apply_discount\n\n\
                 def test_apply_discount_boundary():\n    assert apply_discount(100, 100) == 90\n",
        ),
    )
    .ok_or_else(|| "strong predicate change should classify".to_string())?;
    assert_eq!(exposed.class, ExposureClass::Exposed);
    assert!(exposed.activation.missing_discriminators.is_empty());
    assert!(
        exposed
            .recommended_next_step
            .as_deref()
            .is_some_and(|step| step.contains("observed under a strong oracle"))
    );

    let static_unknown = classify_change(
        Path::new("src/service.py"),
        2,
        "    return getattr(client, name)()",
        &extract_owners(
            Path::new("src/service.py"),
            "def call_named(client, name):\n    return getattr(client, name)()\n",
        ),
        &extract_tests(
            Path::new("tests/test_service.py"),
            "from src.service import call_named\n\n\
                 def test_call_named_dispatches():\n    assert call_named(client, \"total\") == 10\n",
        ),
    )
    .ok_or_else(|| "dynamic dispatch change should classify".to_string())?;
    assert_eq!(static_unknown.class, ExposureClass::StaticUnknown);
    assert!(static_unknown.activation.missing_discriminators.is_empty());
    assert!(static_unknown.recommended_next_step.is_none());

    let no_static_path = classify_change(
        Path::new("src/pricing.py"),
        2,
        "    return amount - 10",
        &extract_owners(
            Path::new("src/pricing.py"),
            "def apply_discount(amount):\n    return amount - 10\n",
        ),
        &extract_tests(
            Path::new("tests/test_other.py"),
            "def test_other():\n    other_behavior()\n",
        ),
    )
    .ok_or_else(|| "unrelated return change should classify".to_string())?;
    assert_eq!(no_static_path.class, ExposureClass::NoStaticPath);
    assert!(no_static_path.activation.missing_discriminators.is_empty());
    assert!(no_static_path.recommended_next_step.is_none());

    Ok(())
}

#[test]
fn classify_change_populates_language_qualified_owner_ids() -> Result<(), String> {
    let function_owners = extract_owners(
        Path::new("src/pricing.py"),
        "def apply_discount(amount):\n    return amount - 10\n",
    );
    let function = classify_change(
        Path::new("src/pricing.py"),
        2,
        "    return amount - 10",
        &function_owners,
        &[],
    )
    .ok_or_else(|| "function changed line should classify".to_string())?;
    assert_eq!(
        function.probe.owner.as_ref().map(ToString::to_string),
        Some("python:src/pricing.py::apply_discount".to_string())
    );

    let method_owners = extract_owners(
        Path::new("src/cart.py"),
        "class Cart:\n    def apply_discount(self, amount):\n        return amount - 10\n",
    );
    let method = classify_change(
        Path::new("src/cart.py"),
        3,
        "        return amount - 10",
        &method_owners,
        &[],
    )
    .ok_or_else(|| "method changed line should classify".to_string())?;
    assert_eq!(
        method.probe.owner.as_ref().map(ToString::to_string),
        Some("python:src/cart.py::Cart.apply_discount".to_string())
    );

    let class_owners = extract_owners(
        Path::new("src/models.py"),
        "class Invoice:\n    status = \"pending\"\n\n    def mark_paid(self):\n        return \"paid\"\n",
    );
    let class_body = classify_change(
        Path::new("src/models.py"),
        2,
        "    status = \"pending\"",
        &class_owners,
        &[],
    )
    .ok_or_else(|| "class body changed line should classify".to_string())?;
    assert_eq!(
        class_body.probe.owner.as_ref().map(ToString::to_string),
        Some("python:src/models.py::Invoice".to_string())
    );
    assert_eq!(class_body.owner_kind, None);
    assert!(
        class_body
            .evidence
            .iter()
            .any(|entry| entry == "owner_kind: class")
    );

    let module_owners = extract_owners(
        Path::new("src/settings.py"),
        "DISCOUNT_THRESHOLD = 100\n\ndef threshold():\n    return DISCOUNT_THRESHOLD\n",
    );
    let module = classify_change(
        Path::new("src/settings.py"),
        1,
        "DISCOUNT_THRESHOLD = 100",
        &module_owners,
        &[],
    )
    .ok_or_else(|| "module changed line should classify".to_string())?;
    assert_eq!(
        module.probe.owner.as_ref().map(ToString::to_string),
        Some("python:src/settings.py::<module>".to_string())
    );
    assert_eq!(module.owner_kind, Some(OwnerKind::ModuleFunction));
    Ok(())
}

#[test]
fn python_owner_id_is_stable_when_owner_line_moves() -> Result<(), String> {
    let before = extract_owners(
        Path::new("src/pricing.py"),
        "def apply_discount(amount):\n    return amount - 10\n",
    );
    let after = extract_owners(
        Path::new("src/pricing.py"),
        "\n\n\ndef apply_discount(amount):\n    return amount - 10\n",
    );
    let before_finding = classify_change(
        Path::new("src/pricing.py"),
        2,
        "    return amount - 10",
        &before,
        &[],
    )
    .ok_or_else(|| "before owner should classify".to_string())?;
    let after_finding = classify_change(
        Path::new("src/pricing.py"),
        5,
        "    return amount - 10",
        &after,
        &[],
    )
    .ok_or_else(|| "after owner should classify".to_string())?;

    assert_eq!(before_finding.probe.owner, after_finding.probe.owner);
    // With content-addressed ids, moving the owner to a different line
    // (without changing the expression) must NOT change the probe id.
    assert_eq!(
        before_finding.probe.id, after_finding.probe.id,
        "content-addressed id must be stable across line movement"
    );
    Ok(())
}

#[test]
fn find_related_tests_matches_import_alias_call() {
    let owners = extract_owners(
        Path::new("src/pricing.py"),
        "def apply_discount(amount):\n    return amount - 10\n",
    );
    let tests = extract_tests(
        Path::new("tests/test_alias_pricing.py"),
        "from src.pricing import apply_discount as discount\n\ndef test_discount_alias():\n    assert discount(100) == 90\n",
    );

    let related = find_related_tests(&owners[0], &tests);

    assert_eq!(related.len(), 1);
    assert_eq!(related[0].name, "test_discount_alias");
    assert_eq!(related[0].oracle_kind, OracleKind::ExactValue);
    assert_eq!(related[0].oracle_strength, OracleStrength::Strong);
}

#[test]
fn related_test_matching_ignores_object_method_calls_for_free_functions() {
    let owners = extract_owners(
        Path::new("src/pricing.py"),
        "def apply_discount(amount):\n    return amount - 10\n",
    );
    let tests = extract_tests(
        Path::new("tests/test_order_methods.py"),
        "def test_order_discount_method():\n    assert order.apply_discount(100) == 90\n",
    );

    let related = related_test_candidates(&owners[0], &tests);

    assert!(related.is_empty());
}

#[test]
fn related_test_matching_accepts_module_alias_attribute_calls() {
    let owners = extract_owners(
        Path::new("src/pricing.py"),
        "def apply_discount(amount):\n    return amount - 10\n",
    );
    let tests = extract_tests(
        Path::new("tests/test_module_alias_pricing.py"),
        "import src.pricing as pricing\n\ndef test_discount_module_alias():\n    assert pricing.apply_discount(100) == 90\n",
    );

    let related = related_test_candidates(&owners[0], &tests);

    assert_eq!(related.len(), 1);
    assert_eq!(related[0].relation, PythonRelationKind::ImportAliasCall);
}

#[test]
fn related_test_matching_keeps_method_owner_object_calls() {
    let owners = extract_owners(
        Path::new("src/cart.py"),
        "class Cart:\n    def apply_discount(self, amount):\n        return amount - 10\n",
    );
    let tests = extract_tests(
        Path::new("tests/test_cart.py"),
        "def test_cart_discount_method():\n    assert cart.apply_discount(100) == 90\n",
    );

    let related = related_test_candidates(&owners[0], &tests);

    assert_eq!(related.len(), 1);
    assert_eq!(related[0].relation, PythonRelationKind::SyntacticCall);
}

#[test]
fn classify_change_uses_import_alias_call_as_strong_relation() -> Result<(), String> {
    let owners = extract_owners(
        Path::new("src/tax.py"),
        "def apply_tax(amount):\n    return amount + 2\n",
    );
    let tests = extract_tests(
        Path::new("tests/test_checkout_tax.py"),
        "from src.tax import apply_tax as taxed\n\ndef test_checkout_tax_alias_import():\n    assert taxed(10) == 12\n",
    );

    let Some(finding) = classify_change(
        Path::new("src/tax.py"),
        2,
        "    return amount + 2",
        &owners,
        &tests,
    ) else {
        return Err("changed line inside owner should classify".to_string());
    };

    assert_eq!(finding.class, ExposureClass::Exposed);
    assert!(finding.evidence.iter().any(|entry| entry
        == "related_test_relation: import_alias_call (test_checkout_tax_alias_import)"));
    Ok(())
}

#[test]
fn classify_change_uses_same_stem_test_as_weak_proximity() -> Result<(), String> {
    let owners = extract_owners(
        Path::new("src/pricing.py"),
        "def apply_discount(amount):\n    return amount - 10\n",
    );
    let tests = extract_tests(
        Path::new("tests/test_pricing.py"),
        "def test_boundary_documented_elsewhere():\n    assert 90 == 90\n",
    );

    let Some(finding) = classify_change(
        Path::new("src/pricing.py"),
        2,
        "    return amount - 10",
        &owners,
        &tests,
    ) else {
        return Err("changed line inside owner should classify".to_string());
    };

    assert_eq!(finding.class, ExposureClass::WeaklyExposed);
    assert_eq!(finding.related_tests.len(), 1);
    assert_eq!(finding.related_tests[0].oracle_kind, OracleKind::Unknown);
    assert!(
        finding.evidence.iter().any(|entry| entry
            == "related_test_relation: same_stem (test_boundary_documented_elsewhere)")
    );
    Ok(())
}

#[test]
fn same_stem_relation_accepts_suffix_and_orders_after_direct_calls() {
    let owners = extract_owners(
        Path::new("src/pricing.py"),
        "def apply_discount(amount):\n    return amount - 10\n",
    );
    let mut tests = extract_tests(
        Path::new("tests/pricing_test.py"),
        "def test_same_stem_only():\n    assert 90 == 90\n",
    );
    tests.extend(extract_tests(
        Path::new("tests/test_checkout.py"),
        "def test_direct_call():\n    assert apply_discount(100) == 90\n",
    ));

    let related = related_test_candidates(&owners[0], &tests);

    assert_eq!(normalize_test_stem("pricing_test"), "pricing");
    assert_eq!(related.len(), 2);
    assert_eq!(related[0].relation, PythonRelationKind::SyntacticCall);
    assert_eq!(related[1].relation, PythonRelationKind::SameStem);
}

#[test]
fn static_limit_detection_covers_python_preview_limit_kinds() {
    let imported_owner = extract_owners(
        Path::new("src/service.py"),
        "from external.client import remote_total\n\ndef total():\n    return remote_total()\n",
    )
    .remove(0);
    let decorated_owner = extract_owners(
        Path::new("src/service.py"),
        "@retry(times=3)\ndef total():\n    return 1\n",
    )
    .remove(0);
    let plain_owner =
        extract_owners(Path::new("src/service.py"), "def total():\n    return 1\n").remove(0);
    let tests = extract_tests(
        Path::new("tests/test_service.py"),
        "from unittest.mock import patch\nfrom src.service import total\n\n@patch(\"src.service.remote_total\")\ndef test_total(mock_remote):\n    assert total() == 1\n",
    );
    let candidates = related_test_candidates(&plain_owner, &tests);
    let monkeypatch_tests = extract_tests(
        Path::new("tests/test_service.py"),
        "from src.service import total\n\ndef test_total(monkeypatch):\n    monkeypatch.setattr(\"src.service.remote_total\", lambda: 1)\n    assert total() == 1\n",
    );
    let monkeypatch_candidates = related_test_candidates(&plain_owner, &monkeypatch_tests);
    let property_based_tests = extract_tests(
        Path::new("tests/test_service.py"),
        "from hypothesis import given, strategies as st\nfrom src.service import total\n\n@given(st.integers())\ndef test_total_property_based(value):\n    assert total(value) >= 0\n",
    );
    let property_based_candidates = related_test_candidates(&plain_owner, &property_based_tests);
    let property_based_with_exact_tests = {
        let mut tests = property_based_tests.clone();
        tests.extend(extract_tests(
            Path::new("tests/test_service_exact.py"),
            "from src.service import total\n\ndef test_total_exact():\n    assert total(1) == 1\n",
        ));
        tests
    };
    let property_based_with_exact_candidates =
        related_test_candidates(&plain_owner, &property_based_with_exact_tests);
    let unresolved_fixture_tests = extract_tests(
        Path::new("tests/test_service.py"),
        "from src.service import total\n\ndef test_total_fixture_case(case):\n    assert total(case.value) == case.expected\n",
    );
    let unresolved_fixture_candidates =
        related_test_candidates(&plain_owner, &unresolved_fixture_tests);
    let unresolved_fixture_with_exact_tests = {
        let mut tests = unresolved_fixture_tests.clone();
        tests.extend(extract_tests(
            Path::new("tests/test_service_exact.py"),
            "from src.service import total\n\ndef test_total_exact():\n    assert total(1) == 1\n",
        ));
        tests
    };
    let unresolved_fixture_with_exact_candidates =
        related_test_candidates(&plain_owner, &unresolved_fixture_with_exact_tests);
    let parametrized_tests = extract_tests(
        Path::new("tests/test_service.py"),
        "import pytest\nfrom src.service import total\n\n@pytest.mark.parametrize(\"value, expected\", [(1, 1)])\ndef test_total_parametrized(value, expected):\n    assert total(value) == expected\n",
    );
    let parametrized_candidates = related_test_candidates(&plain_owner, &parametrized_tests);
    let property_based_with_same_test_exact_tests = extract_tests(
        Path::new("tests/test_service.py"),
        "from hypothesis import given, strategies as st\nfrom src.service import total\n\n@given(st.integers())\ndef test_total_property_based(value):\n    assert total(1) == 1\n",
    );
    let property_based_with_same_test_exact_candidates =
        related_test_candidates(&plain_owner, &property_based_with_same_test_exact_tests);
    let opaque_helper_tests = extract_tests(
        Path::new("tests/test_service.py"),
        "from src.service import total\n\ndef test_total_custom_helper():\n    result = total()\n    assert_total_result(result)\n",
    );
    let opaque_helper_candidates = related_test_candidates(&plain_owner, &opaque_helper_tests);
    let opaque_helper_with_exact_tests = extract_tests(
        Path::new("tests/test_service.py"),
        "from src.service import total\n\ndef test_total_custom_helper_and_exact():\n    result = total()\n    assert_total_result(result)\n    assert result == 1\n",
    );
    let opaque_helper_with_exact_candidates =
        related_test_candidates(&plain_owner, &opaque_helper_with_exact_tests);

    assert_eq!(
        static_limit_for_change("    return getattr(client, name)()", &plain_owner, &[])
            .map(|limit| limit.kind),
        Some(StaticLimitKind::DynamicDispatch)
    );
    assert_eq!(
        static_limit_for_change("    return type(\"Dynamic\", (), {})", &plain_owner, &[])
            .map(|limit| limit.kind),
        Some(StaticLimitKind::Metaprogramming)
    );
    assert_eq!(
        static_limit_for_change("    return 1", &decorated_owner, &[]).map(|limit| limit.kind),
        Some(StaticLimitKind::DecoratorIndirection)
    );
    assert_eq!(
        static_limit_for_change("    return total()", &plain_owner, &candidates)
            .map(|limit| limit.kind),
        Some(StaticLimitKind::MockedModule)
    );
    assert_eq!(
        static_limit_for_change("    return total()", &plain_owner, &monkeypatch_candidates)
            .map(|limit| limit.kind),
        Some(StaticLimitKind::MockedModule)
    );
    assert_eq!(
        static_limit_for_change("    return 1", &plain_owner, &property_based_candidates)
            .map(|limit| limit.kind),
        Some(StaticLimitKind::PropertyBasedTest)
    );
    assert_eq!(
        static_limit_for_change(
            "    return 1",
            &plain_owner,
            &property_based_with_exact_candidates
        )
        .map(|limit| limit.kind),
        Some(StaticLimitKind::PropertyBasedTest)
    );
    assert_eq!(
        static_limit_for_change(
            "    return 1",
            &plain_owner,
            &property_based_with_same_test_exact_candidates
        )
        .map(|limit| limit.kind),
        None
    );
    assert_eq!(
        static_limit_for_change("    return 1", &plain_owner, &unresolved_fixture_candidates)
            .map(|limit| limit.kind),
        Some(StaticLimitKind::UnresolvedPytestFixture)
    );
    assert_eq!(
        static_limit_for_change(
            "    return 1",
            &plain_owner,
            &unresolved_fixture_with_exact_candidates
        )
        .map(|limit| limit.kind),
        None
    );
    assert_eq!(
        static_limit_for_change("    return 1", &plain_owner, &parametrized_candidates)
            .map(|limit| limit.kind),
        None
    );
    assert_eq!(
        static_limit_for_change("    return 1", &plain_owner, &opaque_helper_candidates)
            .map(|limit| limit.kind),
        Some(StaticLimitKind::OpaqueCustomAssertionHelper)
    );
    assert_eq!(
        static_limit_for_change(
            "    return 1",
            &plain_owner,
            &opaque_helper_with_exact_candidates
        )
        .map(|limit| limit.kind),
        None
    );
    assert_eq!(
        static_limit_for_change("    return remote_total()", &imported_owner, &[])
            .map(|limit| limit.kind),
        Some(StaticLimitKind::MissingImportGraph)
    );
    assert_eq!(
        static_limit_for_change("    return lambda value: value + 1", &plain_owner, &[])
            .map(|limit| limit.kind),
        Some(StaticLimitKind::UnsupportedSyntax)
    );
    let mock_owner = extract_owners(
        Path::new("src/callbacks.py"),
        "from unittest.mock import MagicMock\n\ndef recording_callback():\n    callback = MagicMock(name=\"receipt\")\n    return callback\n",
    )
    .remove(0);
    assert_eq!(
        static_limit_for_change(
            "    callback = MagicMock(name=\"receipt.sent\")",
            &mock_owner,
            &[]
        )
        .map(|limit| limit.kind),
        None
    );
}

#[test]
fn classify_change_static_limit_fails_closed_even_with_strong_oracle() -> Result<(), String> {
    let owners = extract_owners(
        Path::new("src/service.py"),
        "def call_named(client, name):\n    return getattr(client, name)()\n",
    );
    let tests = extract_tests(
        Path::new("tests/test_service.py"),
        "from src.service import call_named\n\ndef test_call_named_dispatches():\n    assert call_named(client, \"total\") == 10\n",
    );

    let Some(finding) = classify_change(
        Path::new("src/service.py"),
        2,
        "    return getattr(client, name)()",
        &owners,
        &tests,
    ) else {
        return Err("changed line inside owner should classify".to_string());
    };

    assert_eq!(finding.class, ExposureClass::StaticUnknown);
    assert_eq!(
        finding.static_limit_kind,
        Some(StaticLimitKind::DynamicDispatch)
    );
    assert_eq!(
        finding.stop_reasons,
        vec![StopReason::DynamicDispatchUnresolved]
    );
    assert!(finding.recommended_next_step.is_none());
    assert!(finding.canonical_gap.is_none());
    assert_eq!(finding.ripr.infect.state, StageState::Unknown);
    assert_eq!(finding.ripr.propagate.state, StageState::Unknown);
    assert_eq!(finding.ripr.reveal.discriminate.state, StageState::Unknown);
    assert!(
        finding
            .evidence
            .iter()
            .any(|entry| entry.starts_with("static_limit dynamic_dispatch:"))
    );
    assert!(
        finding
            .missing
            .iter()
            .any(|entry| entry.contains("Static limit `dynamic_dispatch`"))
    );
    Ok(())
}

#[test]
fn classify_change_property_based_test_fails_closed() -> Result<(), String> {
    let owners = extract_owners(
        Path::new("src/pricing.py"),
        "def apply_discount(amount, threshold):\n    if amount >= threshold:\n        return amount - 10\n    return amount\n",
    );
    let tests = extract_tests(
        Path::new("tests/test_pricing.py"),
        "from hypothesis import given, strategies as st\nfrom src.pricing import apply_discount\n\n@given(st.integers(min_value=0), st.integers(min_value=0))\ndef test_apply_discount_property(amount, threshold):\n    assert apply_discount(amount, threshold) <= amount\n",
    );

    let Some(finding) = classify_change(
        Path::new("src/pricing.py"),
        2,
        "    if amount >= threshold:",
        &owners,
        &tests,
    ) else {
        return Err("changed predicate inside owner should classify".to_string());
    };

    assert_eq!(finding.class, ExposureClass::StaticUnknown);
    assert_eq!(
        finding.static_limit_kind,
        Some(StaticLimitKind::PropertyBasedTest)
    );
    assert_eq!(finding.stop_reasons, vec![StopReason::StaticProbeUnknown]);
    assert!(finding.canonical_gap.is_none());
    assert!(finding.recommended_next_step.is_none());
    assert!(finding.activation.missing_discriminators.is_empty());
    assert_eq!(finding.ripr.reveal.discriminate.state, StageState::Unknown);
    assert!(
        finding
            .evidence
            .iter()
            .any(|entry| entry.starts_with("static_limit property_based_test:"))
    );
    assert!(
        finding
            .missing
            .iter()
            .any(|entry| entry.contains("Static limit `property_based_test`"))
    );
    Ok(())
}

#[test]
fn classify_change_unresolved_pytest_fixture_fails_closed() -> Result<(), String> {
    let owners = extract_owners(
        Path::new("src/pricing.py"),
        "def apply_discount(amount, threshold):\n    if amount >= threshold:\n        return amount - 10\n    return amount\n",
    );
    let tests = extract_tests(
        Path::new("tests/test_pricing.py"),
        "from src.pricing import apply_discount\n\ndef test_apply_discount_fixture_case(discount_case):\n    assert apply_discount(discount_case.amount, discount_case.threshold) == discount_case.expected\n",
    );

    let Some(finding) = classify_change(
        Path::new("src/pricing.py"),
        2,
        "    if amount >= threshold:",
        &owners,
        &tests,
    ) else {
        return Err("changed predicate inside owner should classify".to_string());
    };

    assert_eq!(finding.class, ExposureClass::StaticUnknown);
    assert_eq!(
        finding.static_limit_kind,
        Some(StaticLimitKind::UnresolvedPytestFixture)
    );
    assert_eq!(finding.stop_reasons, vec![StopReason::StaticProbeUnknown]);
    assert!(finding.canonical_gap.is_none());
    assert!(finding.recommended_next_step.is_none());
    assert!(finding.activation.missing_discriminators.is_empty());
    assert_eq!(finding.ripr.reveal.discriminate.state, StageState::Unknown);
    assert!(
        finding
            .evidence
            .iter()
            .any(|entry| entry.starts_with("static_limit unresolved_pytest_fixture:"))
    );
    assert!(
        finding
            .missing
            .iter()
            .any(|entry| entry.contains("Static limit `unresolved_pytest_fixture`"))
    );
    Ok(())
}

#[test]
fn classify_change_opaque_custom_assertion_helper_fails_closed() -> Result<(), String> {
    let owners = extract_owners(
        Path::new("src/pricing.py"),
        "def apply_discount(amount, threshold):\n    if amount >= threshold:\n        return amount - 10\n    return amount\n",
    );
    let tests = extract_tests(
        Path::new("tests/test_pricing.py"),
        "from src.pricing import apply_discount\n\ndef assert_discounted(result):\n    assert result < 100\n\ndef test_apply_discount_custom_helper():\n    result = apply_discount(100, 50)\n    assert_discounted(result)\n",
    );

    let Some(finding) = classify_change(
        Path::new("src/pricing.py"),
        2,
        "    if amount >= threshold:",
        &owners,
        &tests,
    ) else {
        return Err("changed predicate inside owner should classify".to_string());
    };

    assert_eq!(finding.class, ExposureClass::StaticUnknown);
    assert_eq!(
        finding.static_limit_kind,
        Some(StaticLimitKind::OpaqueCustomAssertionHelper)
    );
    assert_eq!(finding.stop_reasons, vec![StopReason::StaticProbeUnknown]);
    assert!(finding.canonical_gap.is_none());
    assert!(finding.recommended_next_step.is_none());
    assert!(finding.activation.missing_discriminators.is_empty());
    assert_eq!(finding.ripr.reveal.discriminate.state, StageState::Unknown);
    assert!(
        finding
            .evidence
            .iter()
            .any(|entry| entry.starts_with("static_limit opaque_custom_assertion_helper:"))
    );
    assert!(
        finding
            .missing
            .iter()
            .any(|entry| entry.contains("Static limit `opaque_custom_assertion_helper`"))
    );
    Ok(())
}

#[test]
fn classify_change_static_limit_omits_activation_discriminators() -> Result<(), String> {
    let owners = extract_owners(
        Path::new("src/service.py"),
        "def has_named_value(client, name, threshold):\n    if getattr(client, name) >= threshold:\n        return True\n    return False\n",
    );
    let tests = extract_tests(
        Path::new("tests/test_service.py"),
        "from src.service import has_named_value\n\ndef test_has_named_value():\n    assert has_named_value(client, \"total\", 10) is True\n",
    );

    let Some(finding) = classify_change(
        Path::new("src/service.py"),
        2,
        "    if getattr(client, name) >= threshold:",
        &owners,
        &tests,
    ) else {
        return Err("changed predicate inside owner should classify".to_string());
    };

    assert_eq!(finding.class, ExposureClass::StaticUnknown);
    assert_eq!(
        finding.static_limit_kind,
        Some(StaticLimitKind::DynamicDispatch)
    );
    assert!(finding.flow_sinks.is_empty());
    assert!(finding.activation.missing_discriminators.is_empty());
    assert!(
        finding
            .evidence
            .iter()
            .all(|entry| !entry.starts_with("missing_discriminator:"))
    );
    Ok(())
}

#[test]
fn classify_change_returns_no_static_path_without_related_test() -> Result<(), String> {
    let owners = extract_owners(
        Path::new("src/pricing.py"),
        "def apply_discount(amount):\n    return amount - 10\n",
    );
    let tests = extract_tests(
        Path::new("tests/test_other.py"),
        "def test_other():\n    other_behavior()\n",
    );

    let Some(finding) = classify_change(
        Path::new("src/pricing.py"),
        2,
        "    return amount - 10",
        &owners,
        &tests,
    ) else {
        return Err("changed line inside owner should classify".to_string());
    };

    assert_eq!(finding.class, ExposureClass::NoStaticPath);
    assert_eq!(finding.owner_kind, Some(OwnerKind::Function));
    assert!(finding.related_tests.is_empty());
    assert_eq!(finding.ripr.reach.state, StageState::No);
    assert_eq!(finding.ripr.infect.state, StageState::Yes);
    assert_eq!(finding.ripr.propagate.state, StageState::Yes);
    assert!(finding.recommended_next_step.is_none());
    Ok(())
}

#[test]
fn classify_change_ignores_unrelated_text_mentions() -> Result<(), String> {
    let owners = extract_owners(
        Path::new("src/pricing.py"),
        "def apply_discount(amount):\n    return amount - 10\n",
    );
    let tests = extract_tests(
        Path::new("tests/test_docs.py"),
        "def test_docs_mentions_owner():\n    assert \"apply_discount(\" in \"apply_discount(\"\n",
    );

    let Some(finding) = classify_change(
        Path::new("src/pricing.py"),
        2,
        "    return amount - 10",
        &owners,
        &tests,
    ) else {
        return Err("changed line inside owner should classify".to_string());
    };

    assert_eq!(finding.class, ExposureClass::NoStaticPath);
    assert!(finding.related_tests.is_empty());
    Ok(())
}

#[test]
fn analyze_diff_returns_zero_findings_and_counts_accepted_files() -> Result<(), String> {
    let adapter = PythonAdapter;
    let options = AnalysisOptions {
        root: PathBuf::from("."),
        base: None,
        diff_file: None,
        mode: crate::analysis::AnalysisMode::Draft,
        include_unchanged_tests: false,
        resolve_tsconfig_paths: false,
        perl_facts_path: None,
        git_timeout: None,
        git_candidate: None,
        production_like_targets: Default::default(),
        test_harnesses: Vec::new(),
        resolved_subject_identity: None,
    };
    let policy = OraclePolicy::default();
    let changed_files = vec![
        changed("scripts/run.py"),
        changed("src/lib.rs"),
        changed("docs/README.md"),
        changed("src/util.py"),
        changed("src/index.ts"),
    ];
    let result = adapter.analyze_diff(&options, &policy, &changed_files)?;
    assert!(result.findings.is_empty());
    assert_eq!(result.changed_files, 2);
    Ok(())
}

/// Shared helpers for the repo-mode adapter tests (#3554 PR C): a tiny
/// isolated workspace, explicit working-set limits (#2109), and a digest
/// for the byte-determinism proof.
fn unique_repo_test_root(label: &str) -> PathBuf {
    static COUNTER: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let count = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    std::env::temp_dir().join(format!(
        "ripr-py-repo-adapter-{label}-{}-{count}",
        std::process::id()
    ))
}

fn write_repo_file(path: &Path, contents: &str) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|err| format!("create parent: {err}"))?;
    }
    std::fs::write(path, contents).map_err(|err| format!("write {}: {err}", path.display()))
}

fn repo_options(root: &Path) -> AnalysisOptions {
    AnalysisOptions {
        root: root.to_path_buf(),
        base: None,
        diff_file: None,
        mode: crate::analysis::AnalysisMode::Deep,
        include_unchanged_tests: false,
        resolve_tsconfig_paths: false,
        perl_facts_path: None,
        git_timeout: None,
        git_candidate: None,
        production_like_targets: Default::default(),
        test_harnesses: Vec::new(),
        resolved_subject_identity: None,
    }
}

fn repo_working_set_limit(limit: usize) -> repo::RepoWorkingSetLimit {
    repo::RepoWorkingSetLimit {
        limit,
        source: repo::RepoWorkingSetCapSource::Default,
    }
}

fn fnv1a64(bytes: &[u8]) -> u64 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash
}

/// The empty scaffold is gone (#3554 PR C): repo mode over a supported flat
/// layout returns non-zero native Python evidence through the public
/// adapter surface — diff-mode finding shapes, native Python identity, and
/// input-authority counts.
#[test]
fn analyze_repo_returns_bounded_native_evidence() -> Result<(), String> {
    let root = unique_repo_test_root("native-evidence");
    write_repo_file(&root.join("app.py"), "def run():\n    return 1\n")?;
    write_repo_file(
        &root.join("test_app.py"),
        "from app import run\n\n\ndef test_run():\n    assert run() == 1\n",
    )?;
    let adapter = PythonAdapter;
    let result = adapter.analyze_repo(&repo_options(&root), &OraclePolicy::default())?;

    assert!(
        !result.findings.is_empty(),
        "repo mode must return non-zero evidence for a supported workspace"
    );
    assert_eq!(result.production_files, 1);
    assert_eq!(result.skipped_files, 0);
    assert!(
        result.harness_projections.is_empty(),
        "no harness registrations exist in repo mode"
    );
    assert_eq!(
        result.partial_reason, None,
        "a fully analyzed working set is not a partial run"
    );
    for finding in &result.findings {
        assert_eq!(
            finding.language,
            Some(crate::domain::LanguageId::Python),
            "native Python identity is retained"
        );
        assert_eq!(
            finding.language_status,
            Some(crate::domain::LanguageStatus::Preview),
            "repo-mode Python findings stay preview-tier"
        );
        let gap = finding
            .canonical_gap
            .as_ref()
            .ok_or("canonical gap expected for an unlimited behavior item")?;
        assert!(
            gap.id.starts_with("gap:python:app.py:"),
            "canonical gap keeps native Python identity: {}",
            gap.id
        );
        assert_eq!(gap.language, "python");
        assert_eq!(
            finding.probe.location.file,
            PathBuf::from("app.py"),
            "finding locations name the analyzed workspace-relative file"
        );
    }
    // The discriminated owner carries the exact-value oracle evidence.
    let run = result
        .findings
        .iter()
        .find(|finding| {
            finding
                .probe
                .owner
                .as_ref()
                .is_some_and(|owner| owner.0.ends_with("::run"))
        })
        .ok_or("expected a finding for the `run` owner")?;
    assert!(
        matches!(run.class, crate::domain::ExposureClass::Exposed),
        "a strong exact-value oracle that observes the changed owner is the exposed shape, got {:?}",
        run.class
    );
    std::fs::remove_dir_all(&root).map_err(|err| format!("remove root: {err}"))?;
    Ok(())
}

/// Repeated `analyze_repo` runs over identical input produce identical
/// findings (byte- and digest-identical), and the result counts reflect the
/// selected working set (#3554 PR C determinism/currentness proof). There
/// is no cache in the Python producer, so determinism holds by
/// construction; the digest pins it against accidental nondeterminism.
#[test]
fn analyze_repo_is_deterministic_and_candidate_current() -> Result<(), String> {
    let root = unique_repo_test_root("determinism");
    write_repo_file(&root.join("app.py"), "def run():\n    return 1\n")?;
    write_repo_file(
        &root.join("test_app.py"),
        "from app import run\n\n\ndef test_run():\n    assert run() == 1\n",
    )?;
    // A vendor file is excluded by role and must show up in skipped_files.
    write_repo_file(&root.join("vendor/dep.py"), "VALUE = 2\n")?;

    let first = PythonAdapter::analyze_repo_with_limit(&root, repo_working_set_limit(800))?;
    let second = PythonAdapter::analyze_repo_with_limit(&root, repo_working_set_limit(800))?;

    assert_eq!(first.findings, second.findings);
    let first_digest = fnv1a64(format!("{:?}", first.findings).as_bytes());
    let second_digest = fnv1a64(format!("{:?}", second.findings).as_bytes());
    assert_eq!(first_digest, second_digest);
    assert!(!first.findings.is_empty());
    // Input identity: counts come from the selected working-set authority.
    assert_eq!(first.production_files, 1);
    assert_eq!(first.skipped_files, 1, "the vendor file is role-excluded");
    // Currentness: repo mode seeds probes from the live tree (#3280).
    assert!(
        first
            .findings
            .iter()
            .all(|finding| finding.source_currentness
                == crate::domain::SourceCurrentness::CandidateCurrent)
    );
    assert_eq!(first.partial_reason, None);
    std::fs::remove_dir_all(&root).map_err(|err| format!("remove root: {err}"))?;
    Ok(())
}

/// Test and helper files contribute evidence but never seed production
/// findings — pinned at the `analyze_repo` level (#3554 claim boundary).
#[test]
fn analyze_repo_test_files_never_seed_production_findings() -> Result<(), String> {
    let root = unique_repo_test_root("no-seed");
    write_repo_file(&root.join("app.py"), "def app_value():\n    return 1\n")?;
    write_repo_file(
        &root.join("test_helper.py"),
        "def seeded():\n    return 42\n\n\ndef test_real():\n    assert True\n",
    )?;
    write_repo_file(&root.join("conftest.py"), "def helper():\n    return 7\n")?;
    let result = PythonAdapter::analyze_repo_with_limit(&root, repo_working_set_limit(800))?;

    assert_eq!(result.production_files, 1);
    assert!(
        !result.findings.is_empty(),
        "the production file still yields evidence"
    );
    for finding in &result.findings {
        assert_eq!(
            finding.probe.location.file,
            PathBuf::from("app.py"),
            "a production-looking def in a test/helper file must never seed a finding, got {}",
            finding.probe.location.file.display()
        );
    }
    assert_eq!(result.partial_reason, None);
    std::fs::remove_dir_all(&root).map_err(|err| format!("remove root: {err}"))?;
    Ok(())
}

/// A run capped by the working-set limit (#2109) keeps its computed
/// findings and discloses the cap through `partial_reason`, which the
/// pipeline maps onto the shared partial-run channel.
#[test]
fn analyze_repo_discloses_a_capped_run_as_partial() -> Result<(), String> {
    let root = unique_repo_test_root("capped");
    write_repo_file(&root.join("alpha.py"), "def one():\n    return 1\n")?;
    write_repo_file(&root.join("zeta.py"), "def two():\n    return 2\n")?;
    let result = PythonAdapter::analyze_repo_with_limit(&root, repo_working_set_limit(1))?;

    assert_eq!(result.production_files, 1, "the cap selected one file");
    assert_eq!(result.skipped_files, 1, "the capped file is counted");
    let reason = result
        .partial_reason
        .ok_or("a capped run must disclose a partial reason")?;
    assert!(reason.contains("capped"), "{reason}");
    assert!(
        reason.contains("RIPR_MAX_REPO_INDEX_FILES"),
        "the recovery route is retained: {reason}"
    );
    // Findings cover only the selected file.
    for finding in &result.findings {
        assert_eq!(
            finding.probe.location.file,
            PathBuf::from("alpha.py"),
            "findings beyond the cap must not exist"
        );
    }
    std::fs::remove_dir_all(&root).map_err(|err| format!("remove root: {err}"))?;
    Ok(())
}

/// A parse failure is disclosed as a partial run — the analyzed files keep
/// their findings and the denominator stays explicit (#3554).
#[test]
fn analyze_repo_discloses_parse_failures_as_partial() -> Result<(), String> {
    let root = unique_repo_test_root("parse-failure");
    write_repo_file(&root.join("good.py"), "def good():\n    return 1\n")?;
    write_repo_file(&root.join("broken.py"), "def broken(:\n    pass\n")?;
    let result = PythonAdapter::analyze_repo_with_limit(&root, repo_working_set_limit(800))?;

    assert_eq!(result.production_files, 2);
    let reason = result
        .partial_reason
        .ok_or("a parse-failure run must disclose a partial reason")?;
    assert!(reason.contains("partial"), "{reason}");
    assert!(reason.contains("failed to read or parse"), "{reason}");
    assert!(
        result
            .findings
            .iter()
            .all(|finding| finding.probe.location.file == Path::new("good.py")),
        "the failed file produced no findings"
    );
    std::fs::remove_dir_all(&root).map_err(|err| format!("remove root: {err}"))?;
    Ok(())
}

/// A workspace with no Python production source is an honest zero — not a
/// partial run and not a fabricated limitation (#3554).
#[test]
fn analyze_repo_returns_an_honest_zero_for_a_tests_only_workspace() -> Result<(), String> {
    let root = unique_repo_test_root("tests-only");
    write_repo_file(
        &root.join("test_only.py"),
        "def test_only():\n    assert True\n",
    )?;
    let result = PythonAdapter::analyze_repo_with_limit(&root, repo_working_set_limit(800))?;

    assert!(result.findings.is_empty());
    assert_eq!(result.production_files, 0);
    assert_eq!(result.partial_reason, None);
    std::fs::remove_dir_all(&root).map_err(|err| format!("remove root: {err}"))?;
    Ok(())
}
