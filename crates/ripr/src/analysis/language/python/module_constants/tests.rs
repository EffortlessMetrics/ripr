//! Named module-constant resolution for Python predicate boundaries (#4227):
//! which names resolve, which stay unresolved because something can rebind
//! them, and what the boundary classification does with each.

use super::super::classify::classify_change;
use super::super::owners_tests::{extract_owners, extract_tests};
use crate::domain::{ExposureClass, Finding};
use std::path::Path;

const OWNER: &str = "def discounted_total(amount):\n    if amount >= DISCOUNT_THRESHOLD:\n        return amount - amount // 10\n    return amount\n";
const OFF_BOUNDARY_TESTS: &str = "from src.pricing import discounted_total\n\ndef test_below():\n    assert discounted_total(5_000) == 5_000\n\ndef test_above():\n    assert discounted_total(20_000) == 18_000\n";

/// Constants visible in `discounted_total` after `prelude` at module scope.
fn visible_constants(prelude: &str) -> Vec<(String, String)> {
    let source = format!("{prelude}\n{OWNER}");
    extract_owners(Path::new("src/pricing.py"), &source)
        .into_iter()
        .find(|owner| owner.name == "discounted_total")
        .map(|owner| {
            owner
                .module_constants
                .into_iter()
                .map(|constant| (constant.name, constant.value))
                .collect()
        })
        .unwrap_or_default()
}

fn threshold() -> Vec<(String, String)> {
    vec![("DISCOUNT_THRESHOLD".to_string(), "10000".to_string())]
}

/// Classify the changed predicate for `prelude` + the pricing owner.
fn classify(prelude: &str, tests: &str) -> Result<Finding, String> {
    let source = format!("{prelude}\n{OWNER}");
    let line = source
        .lines()
        .position(|line| line.contains("if amount >= DISCOUNT_THRESHOLD"))
        .map(|index| index + 1)
        .ok_or("fixture must contain the changed predicate")?;
    let file = Path::new("src/pricing.py");
    let owners = extract_owners(file, &source);
    let tests = extract_tests(Path::new("tests/test_pricing.py"), tests);
    classify_change(
        file,
        line,
        "    if amount >= DISCOUNT_THRESHOLD:",
        &owners,
        &tests,
    )
    .ok_or_else(|| "changed predicate must classify".to_string())
}

fn discriminators(finding: &Finding) -> Vec<&str> {
    finding
        .activation
        .missing_discriminators
        .iter()
        .map(|fact| fact.value.as_str())
        .collect()
}

#[test]
fn literal_bound_once_resolves_to_its_canonical_value() {
    assert_eq!(
        visible_constants("DISCOUNT_THRESHOLD = 10_000\n"),
        threshold()
    );
    assert_eq!(
        visible_constants("DISCOUNT_THRESHOLD: int = 10_000\n"),
        threshold()
    );
}

#[test]
fn rebindable_or_non_literal_names_stay_unresolved() {
    let cases = [
        (
            "reassigned",
            "DISCOUNT_THRESHOLD = 10_000\nDISCOUNT_THRESHOLD = 5_000\n",
        ),
        (
            "augmented",
            "DISCOUNT_THRESHOLD = 10_000\nDISCOUNT_THRESHOLD += 1\n",
        ),
        ("non-literal", "DISCOUNT_THRESHOLD = int('10000')\n"),
        (
            "conditional",
            "DISCOUNT_THRESHOLD = 10_000\nif DEBUG:\n    DISCOUNT_THRESHOLD = 1\n",
        ),
        (
            "tuple target",
            "DISCOUNT_THRESHOLD = 10_000\nDISCOUNT_THRESHOLD, OTHER = 1, 2\n",
        ),
        (
            "for target",
            "DISCOUNT_THRESHOLD = 10_000\nfor DISCOUNT_THRESHOLD in range(3):\n    pass\n",
        ),
        (
            "import",
            "DISCOUNT_THRESHOLD = 10_000\nfrom config import DISCOUNT_THRESHOLD\n",
        ),
        (
            "global",
            "DISCOUNT_THRESHOLD = 10_000\ndef configure(value):\n    global DISCOUNT_THRESHOLD\n    DISCOUNT_THRESHOLD = value\n",
        ),
        (
            "walrus",
            "DISCOUNT_THRESHOLD = 10_000\nprint(DISCOUNT_THRESHOLD := 5)\n",
        ),
        (
            "star import",
            "DISCOUNT_THRESHOLD = 10_000\nfrom config import *\n",
        ),
        (
            "globals()",
            "DISCOUNT_THRESHOLD = 10_000\nglobals ()['DISCOUNT_THRESHOLD'] = 1\n",
        ),
        (
            "exec",
            "DISCOUNT_THRESHOLD = 10_000\nexec('DISCOUNT_THRESHOLD = 5')\n",
        ),
        (
            "sys.modules",
            "import sys\nDISCOUNT_THRESHOLD = 10_000\nsys.modules[__name__].DISCOUNT_THRESHOLD = 5\n",
        ),
        (
            "vars()",
            "DISCOUNT_THRESHOLD = 10_000\nvars()['DISCOUNT_THRESHOLD'] = 5\n",
        ),
        (
            "mock.patch",
            "from unittest import mock\nDISCOUNT_THRESHOLD = 10_000\nmock.patch(f'{__name__}.DISCOUNT_THRESHOLD', 5).start()\n",
        ),
        (
            "patch.object",
            "import sys\nfrom unittest.mock import patch\nDISCOUNT_THRESHOLD = 10_000\npatch.object(sys.modules[__name__], 'DISCOUNT_THRESHOLD', 5)\n",
        ),
        (
            "self-alias attribute in a helper",
            "import pricing as _self\nDISCOUNT_THRESHOLD = 10_000\ndef tweak():\n    _self.DISCOUNT_THRESHOLD = 5\n",
        ),
        (
            "self-alias augmented attribute",
            "import pricing as _self\nDISCOUNT_THRESHOLD = 10_000\n_self.DISCOUNT_THRESHOLD += 1\n",
        ),
        (
            "match",
            "DISCOUNT_THRESHOLD = 10_000\nmatch MODE:\n    case DISCOUNT_THRESHOLD:\n        pass\n",
        ),
    ];
    for (label, prelude) in cases {
        assert_eq!(visible_constants(prelude), Vec::new(), "{label}");
    }
}

/// Control for the self-alias cases: an attribute write that names a
/// different attribute leaves the constant resolved.
#[test]
fn unrelated_attribute_assignment_keeps_the_constant() {
    assert_eq!(
        visible_constants("import config\nDISCOUNT_THRESHOLD = 10_000\nconfig.OTHER_LIMIT = 5\n"),
        threshold()
    );
}

#[test]
fn owner_local_bindings_shadow_the_module_constant() {
    let source = "DISCOUNT_THRESHOLD = 10_000\n\ndef local(amount):\n    DISCOUNT_THRESHOLD = 5\n    return amount >= DISCOUNT_THRESHOLD\n\ndef param(amount, DISCOUNT_THRESHOLD):\n    return amount >= DISCOUNT_THRESHOLD\n\ndef plain(amount):\n    return amount >= DISCOUNT_THRESHOLD\n";
    let owners = extract_owners(Path::new("src/pricing.py"), source);
    let constants = |name: &str| {
        owners
            .iter()
            .find(|owner| owner.name == name)
            .map(|owner| owner.module_constants.len())
    };
    assert_eq!(constants("local"), Some(0));
    assert_eq!(constants("param"), Some(0));
    assert_eq!(constants("plain"), Some(1));
}

#[test]
fn owner_with_a_nested_scope_sees_no_constants() {
    // A changed line inside `check` is attributed to `discounted_total`, and
    // `check`'s parameter shadows the module constant there.
    let source = "DISCOUNT_THRESHOLD = 10_000\n\ndef nested(amount, tier):\n    def check(DISCOUNT_THRESHOLD):\n        return amount >= DISCOUNT_THRESHOLD\n    return check(tier)\n\ndef lam(amount):\n    check = lambda DISCOUNT_THRESHOLD: amount >= DISCOUNT_THRESHOLD\n    return check(5)\n";
    let owners = extract_owners(Path::new("src/pricing.py"), source);
    for name in ["nested", "lam"] {
        let constants = owners
            .iter()
            .find(|owner| owner.name == name)
            .map(|owner| owner.module_constants.len());
        assert_eq!(constants, Some(0), "{name}");
    }
}

#[test]
fn off_boundary_calls_name_the_constant_boundary_with_its_value() -> Result<(), String> {
    let finding = classify("DISCOUNT_THRESHOLD = 10_000\n", OFF_BOUNDARY_TESTS)?;
    assert_eq!(finding.class, ExposureClass::WeaklyExposed);
    assert_eq!(discriminators(&finding), ["amount == DISCOUNT_THRESHOLD"]);
    let reason = &finding.activation.missing_discriminators[0].reason;
    assert!(
        reason.contains("DISCOUNT_THRESHOLD = 10000 at line 1")
            && reason.contains("observed amount values: 20000, 5000"),
        "{reason}"
    );
    Ok(())
}

#[test]
fn rebound_constant_names_no_repair_target() -> Result<(), String> {
    let finding = classify(
        "DISCOUNT_THRESHOLD = 10_000\n\ndef configure(value):\n    global DISCOUNT_THRESHOLD\n    DISCOUNT_THRESHOLD = value\n",
        OFF_BOUNDARY_TESTS,
    )?;
    assert_eq!(finding.class, ExposureClass::WeaklyExposed);
    assert!(discriminators(&finding).is_empty());
    Ok(())
}

#[test]
fn boundary_call_by_literal_or_imported_constant_is_exposed() -> Result<(), String> {
    // Each variant keeps the off-boundary literal calls, so the rule is engaged
    // and only a call that binds `amount` to 10000 can credit `exposed`.
    let at_boundary = [
        ("", "discounted_total(10_000)"),
        (
            "from src.pricing import DISCOUNT_THRESHOLD\n",
            "discounted_total(DISCOUNT_THRESHOLD)",
        ),
        (
            "from src.pricing import DISCOUNT_THRESHOLD as T\n",
            "discounted_total(T)",
        ),
        (
            "import src.pricing as pricing\n",
            "discounted_total(pricing.DISCOUNT_THRESHOLD)",
        ),
    ];
    for (import, call) in at_boundary {
        let tests =
            format!("{import}{OFF_BOUNDARY_TESTS}\ndef test_at():\n    assert {call} == 9_000\n");
        let finding = classify("DISCOUNT_THRESHOLD = 10_000\n", &tests)?;
        assert_eq!(finding.class, ExposureClass::Exposed, "{tests}");
    }
    Ok(())
}

#[test]
fn constant_name_not_imported_from_the_owner_module_does_not_bind() -> Result<(), String> {
    // Same name, different module: the argument is not the owner's constant.
    let tests = "from src.pricing import discounted_total\nfrom config import DISCOUNT_THRESHOLD\n\ndef test_other():\n    assert discounted_total(5_000) == 5_000\n    assert discounted_total(DISCOUNT_THRESHOLD) == 9_000\n";
    let finding = classify("DISCOUNT_THRESHOLD = 10_000\n", tests)?;
    assert_eq!(finding.class, ExposureClass::WeaklyExposed);
    assert_eq!(discriminators(&finding), ["amount == DISCOUNT_THRESHOLD"]);
    Ok(())
}

#[test]
fn test_side_rebinding_blocks_the_imported_constant_binding() -> Result<(), String> {
    // Each test passes a name that no longer holds the owner's 10000, next to
    // an off-boundary literal call that engages the rule.
    let rebound = [
        (
            "module reassignment",
            "from src.pricing import DISCOUNT_THRESHOLD, discounted_total\nDISCOUNT_THRESHOLD = 5\n",
            "",
        ),
        (
            "test local",
            "from src.pricing import DISCOUNT_THRESHOLD, discounted_total\n",
            "    DISCOUNT_THRESHOLD = 5\n",
        ),
        (
            "function-level import",
            "from src.pricing import DISCOUNT_THRESHOLD, discounted_total\n",
            "    from src.other import DISCOUNT_THRESHOLD\n",
        ),
        (
            "autouse fixture declaring global",
            "import pytest\nfrom src.pricing import DISCOUNT_THRESHOLD, discounted_total\n\n@pytest.fixture(autouse=True)\ndef shift():\n    global DISCOUNT_THRESHOLD\n    DISCOUNT_THRESHOLD = 5\n",
            "",
        ),
        (
            "later module import",
            "from src.pricing import DISCOUNT_THRESHOLD, discounted_total\nfrom src.other import DISCOUNT_THRESHOLD\n",
            "",
        ),
    ];
    for (label, header, prefix) in rebound {
        let tests = format!(
            "{header}\ndef test_mixed():\n{prefix}    assert discounted_total(20_000) == 18_000\n    assert discounted_total(DISCOUNT_THRESHOLD) == 9_000\n"
        );
        let finding = classify("DISCOUNT_THRESHOLD = 10_000\n", &tests)?;
        assert_eq!(finding.class, ExposureClass::WeaklyExposed, "{label}");
    }
    let parametrized = "import pytest\nfrom src.pricing import DISCOUNT_THRESHOLD, discounted_total\n\n@pytest.mark.parametrize('DISCOUNT_THRESHOLD', [5])\ndef test_param(DISCOUNT_THRESHOLD):\n    assert discounted_total(20_000) == 18_000\n    assert discounted_total(DISCOUNT_THRESHOLD) == 9_000\n";
    let finding = classify("DISCOUNT_THRESHOLD = 10_000\n", parametrized)?;
    assert_eq!(finding.class, ExposureClass::WeaklyExposed, "parametrized");
    Ok(())
}

#[test]
fn test_file_attribute_assignment_unresolves_the_constant() -> Result<(), String> {
    // `pricing.DISCOUNT_THRESHOLD = 5` rewrites the owner's constant at
    // runtime, so the boundary is not named as a repair target.
    let tests = format!(
        "import src.pricing as pricing\n{OFF_BOUNDARY_TESTS}\ndef test_patched():\n    pricing.DISCOUNT_THRESHOLD = 5\n    assert discounted_total(5) == 4\n"
    );
    let finding = classify("DISCOUNT_THRESHOLD = 10_000\n", &tests)?;
    assert_eq!(finding.class, ExposureClass::WeaklyExposed);
    assert!(discriminators(&finding).is_empty());
    Ok(())
}

/// #4429 review: a related test file that patches the constant through
/// `unittest.mock` rewrites it without an attribute assignment, so the file
/// counts as a dynamic writer of every attribute.
#[test]
fn mock_patching_in_a_test_file_counts_as_a_dynamic_writer() {
    for source in [
        "from unittest import mock\n\n@mock.patch('pricing.DISCOUNT_THRESHOLD', 5)\ndef test_x():\n    pass\n",
        "from unittest.mock import patch\nimport pricing\n\ndef test_x():\n    with patch.object(pricing, 'DISCOUNT_THRESHOLD', 5):\n        pass\n",
    ] {
        assert!(super::writes_namespace_dynamically(source), "{source}");
    }
    assert!(!super::writes_namespace_dynamically(
        "import pricing\n\ndef test_x():\n    assert pricing.shipping(5_000) == 0\n"
    ));
}

/// `walrus_target_names` finds exactly the names `walrus_targets` accepts,
/// on every name drawn from the text and on names that are not in it.
#[test]
fn walrus_target_names_match_walrus_targets_for_every_name() {
    let sources = [
        "",
        "x := 1",
        "if (n := len(a)) > 10: pass",
        "LIMIT:=5\nother = LIMIT",
        "a ::= 1\nb :== 2\nc\t\n := 3",
        "xLIMIT := 1\nLIMITx := 2",
        "é_x := 1  # é_x\nif (y:=2): y",
        ":= 1\n  := 2",
        "LIMIT = 3\nprint(LIMIT, n := LIMIT)",
    ];
    let mut compared = 0;
    for source in sources {
        let names = super::walrus_target_names(source);
        let mut candidates: Vec<&str> = source
            .split(|ch: char| !(ch.is_alphanumeric() || ch == '_'))
            .filter(|word| !word.is_empty())
            .collect();
        candidates.extend(["LIMIT", "missing", "x", "n"]);
        for name in candidates {
            assert_eq!(
                names.contains(name),
                super::walrus_targets(source, name),
                "{name:?} in {source:?}"
            );
            compared += 1;
        }
    }
    assert!(compared > 40, "{compared}");
}
