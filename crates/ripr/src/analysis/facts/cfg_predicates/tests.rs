use super::*;

#[test]
fn cfg_predicate_spelling_table_pins_requires_test_forms() {
    let requires_test = [
        "#[cfg(test)]",
        // Whitespace variants: trivia cannot change the term structure.
        "#[ cfg ( test ) ]",
        "#[cfg( test )]",
        "#[cfg(all(test, unix))]",
        "#[cfg(all(unix, test))]",
        "#[cfg(all(unix, windows, test))]",
        // Nested conjunctions require test at any depth (issue #3530).
        "#[cfg(all(unix, all(test, feature = \"slow\")))]",
        "#[cfg(all(all(test, unix), windows))]",
        // A trailing comma still leaves the `test` conjunct.
        "#[cfg(all(test,))]",
        // Comments and raw strings cannot break the structural conjunct.
        "#[cfg(all(feature = \"slow\" /* fake ,test, */, test))]",
        "#[cfg(all(unix, test, feature = r#\"slow,test\"#))]",
        // Inner-attribute form gates its module the same way.
        "#![cfg(test)]",
    ];

    for spelling in requires_test {
        assert_eq!(
            classify_attribute(spelling),
            CfgTestRequirement::RequiresTest,
            "structurally test-required spelling must classify requires_test: {spelling}"
        );
        assert!(
            attribute_requires_test(spelling),
            "requires_test spelling must gate on test: {spelling}"
        );
    }
}

#[test]
fn alternatives_negation_literals_and_lookalikes_stay_production() {
    let expectations: &[(&str, CfgTestRequirement)] = &[
        // Alternatives admit non-test builds.
        (
            "#[cfg(any(test, unix))]",
            CfgTestRequirement::MayIncludeTest,
        ),
        (
            "#[cfg(all(any(test, feature = \"slow\"), unix))]",
            CfgTestRequirement::MayIncludeTest,
        ),
        // Negation is provably not a test requirement.
        ("#[cfg(not(test))]", CfgTestRequirement::IndependentOfTest),
        (
            "#[cfg(not(any(test, unix)))]",
            CfgTestRequirement::IndependentOfTest,
        ),
        // Key-value literals and lookalike identifiers cannot earn the role.
        (
            "#[cfg(target_os = \"test\")]",
            CfgTestRequirement::IndependentOfTest,
        ),
        (
            "#[cfg(feature = \"test\")]",
            CfgTestRequirement::IndependentOfTest,
        ),
        (
            "#[cfg(test_support)]",
            CfgTestRequirement::IndependentOfTest,
        ),
        // Substring control: `test` inside a value is not a conjunct.
        (
            "#[cfg(all(unix, feature = \"test\"))]",
            CfgTestRequirement::IndependentOfTest,
        ),
        (
            "#[cfg(all(feature = \"slow,test\", unix))]",
            CfgTestRequirement::IndependentOfTest,
        ),
        (
            "#[cfg(all(feature = r#\"slow,test\"#, unix))]",
            CfgTestRequirement::IndependentOfTest,
        ),
        (
            "#[cfg(all(feature = \"slow\" /*,test,*/, unix))]",
            CfgTestRequirement::IndependentOfTest,
        ),
        // Non-cfg attributes are not this authority's test gates.
        ("#[inline]", CfgTestRequirement::IndependentOfTest),
        ("#[tokio::test]", CfgTestRequirement::IndependentOfTest),
    ];

    for (spelling, expected) in expectations {
        assert_eq!(
            &classify_attribute(spelling),
            expected,
            "spelling must not classify requires_test: {spelling}"
        );
        assert!(
            !attribute_requires_test(spelling),
            "spelling must stay production: {spelling}"
        );
    }

    // A lookalike identifier next to a real conjunct still requires test.
    assert_eq!(
        classify_attribute("#[cfg(all(testsupport, test))]"),
        CfgTestRequirement::RequiresTest,
        "the exact `test` conjunct remains authoritative next to lookalikes"
    );
}

#[test]
fn malformed_and_unsupported_predicates_stay_unknown() {
    let unknown = [
        "#[cfg()]",
        "#[cfg(all())]",
        "#[cfg(any())]",
        // Unbalanced input cannot be established.
        "#[cfg(test",
        "#[cfg(all(test, unix)]",
        "#[cfg(test)))]",
        // String content is opaque, so a string predicate is not a term.
        "#[cfg(\"test\")]",
        // Two bare words in one predicate are not a supported shape.
        "#[cfg(test unix)]",
        "#[cfg(test, unix)]",
        // Unsupported call shapes stay unknown.
        "#[cfg(mixed(test))]",
    ];

    for spelling in unknown {
        assert_eq!(
            classify_attribute(spelling),
            CfgTestRequirement::Unknown,
            "malformed or unsupported spelling must fail closed: {spelling}"
        );
        assert!(
            !attribute_requires_test(spelling),
            "unknown must never gate on test: {spelling}"
        );
    }
}

#[test]
fn cfg_attr_forms_never_require_test() {
    let expectations: &[(&str, CfgTestRequirement)] = &[
        // A test-conditional lint does not make the item test-only.
        (
            "#[cfg_attr(test, allow(dead_code))]",
            CfgTestRequirement::IndependentOfTest,
        ),
        // A test-conditional cfg(feature) does not remove non-test builds.
        (
            "#[cfg_attr(test, cfg(feature = \"slow\"))]",
            CfgTestRequirement::IndependentOfTest,
        ),
        // The issue's own example: a test-requiring outer condition with a
        // non-test inner gate does not itself gate the item on test.
        (
            "#[cfg_attr(all(feature = \"internal-tests\", test), cfg(unix))]",
            CfgTestRequirement::IndependentOfTest,
        ),
        // Conditional introduction of cfg(test) may include a test build,
        // but feature activation is not established.
        (
            "#[cfg_attr(feature = \"internal-tests\", cfg(test))]",
            CfgTestRequirement::MayIncludeTest,
        ),
        (
            "#[cfg_attr(test, cfg(any(test, unix)))]",
            CfgTestRequirement::MayIncludeTest,
        ),
        (
            "#[cfg_attr(any(test, unix), cfg(test))]",
            CfgTestRequirement::MayIncludeTest,
        ),
        // Plain conditional lints stay independent.
        (
            "#[cfg_attr(feature = \"x\", allow(dead_code))]",
            CfgTestRequirement::IndependentOfTest,
        ),
        // Documented bound: nested cfg_attr introductions are not credited.
        (
            "#[cfg_attr(test, cfg_attr(test, cfg(test)))]",
            CfgTestRequirement::IndependentOfTest,
        ),
        // Malformed cfg_attr forms fail closed.
        ("#[cfg_attr()]", CfgTestRequirement::Unknown),
        ("#[cfg_attr(test)]", CfgTestRequirement::Unknown),
        ("#[cfg_attr(test, cfg(test]", CfgTestRequirement::Unknown),
        (
            "#[cfg_attr(unreadable condition, cfg(test))]",
            CfgTestRequirement::Unknown,
        ),
    ];

    for (spelling, expected) in expectations {
        assert_eq!(
            &classify_attribute(spelling),
            expected,
            "cfg_attr classification drifted from the closed model: {spelling}"
        );
        assert!(
            !attribute_requires_test(spelling),
            "cfg_attr must never promote an item to test-only: {spelling}"
        );
    }
}

/// Conditional `path` introductions make a module's file target
/// configuration-dependent (#3533): every spelling that introduces `path`
/// from a `cfg_attr` must be detected so the module-path producer fails
/// closed to a typed unknown instead of resolving the default layout.
#[test]
fn cfg_attr_introduced_path_attributes_are_detected() {
    // The review's exact spelling: a conditional test target next to the
    // test gate that activates it.
    assert!(super::attributes_conditionally_introduce_path([
        "#[cfg(test)]",
        "#[cfg_attr(test, path = \"test_impl.rs\")]",
    ]));
    // Platform selection shape.
    assert!(super::attributes_conditionally_introduce_path([
        "#[cfg_attr(windows, path = \"windows.rs\")]",
    ]));
    // A non-literal introduction value, and the bare attribute head.
    assert!(super::attributes_conditionally_introduce_path([
        "#[cfg_attr(feature = \"x\", path = concat!(env!(\"OUT_DIR\"), \"gen.rs\"))]",
    ]));
    assert!(super::attributes_conditionally_introduce_path([
        "#[cfg_attr(feature = \"x\", path)]",
    ]));
    // Nested cfg_attr introductions are followed: a missed conditional
    // `path` would resolve the wrong module file (over-emission direction).
    assert!(super::attributes_conditionally_introduce_path([
        "#[cfg_attr(unix, cfg_attr(test, path = \"unix_test.rs\"))]",
    ]));
    // Whitespace inside the introduced attribute still matches.
    assert!(super::attributes_conditionally_introduce_path([
        "#[ cfg_attr( test , path = \"late.rs\" ) ]",
    ]));
}

/// Controls: cfg_attr without a `path` introduction, direct `#[path]`,
/// cfg key-value predicates whose strings mention path, and other
/// attributes must not be read as conditional module paths.
#[test]
fn non_path_cfg_attr_shapes_are_not_path_introductions() {
    assert!(!super::attributes_conditionally_introduce_path([
        "#[cfg_attr(test, allow(dead_code))]",
    ]));
    assert!(!super::attributes_conditionally_introduce_path([
        "#[cfg_attr(test, cfg(test))]",
    ]));
    // A direct `#[path]` is unconditional, not a conditional introduction.
    assert!(!super::attributes_conditionally_introduce_path([
        "#[path = \"direct.rs\"]",
    ]));
    // The word `path` inside an opaque literal or a predicate value cannot
    // manufacture an introduction.
    assert!(!super::attributes_conditionally_introduce_path([
        "#[cfg(feature = \"path\")]",
    ]));
    assert!(!super::attributes_conditionally_introduce_path([
        "#[cfg_attr(test, doc = \"path = x\")]",
    ]));
    // A cfg(...) introduction is skipped entirely: its key-value custom
    // options are predicates, not attributes.
    assert!(!super::attributes_conditionally_introduce_path([
        "#[cfg_attr(unix, cfg(path = \"x\"))]",
    ]));
    // Non-attribute text and malformed envelopes detect nothing.
    assert!(!super::attributes_conditionally_introduce_path([
        "fn plain() {}",
        "#[cfg_attr(test, path = \"unbalanced\"",
    ]));
}

/// An unlexable `cfg_attr` fails closed (#3533 review): Rust accepts Unicode
/// identifiers in predicate position (XID_Start/XID_Continue), so
/// `#[cfg_attr(é, path = "alternate.rs")]` is valid Rust the bounded ASCII
/// lexer cannot tokenize. Converting that lexer failure to `false` would let
/// the default-layout file resolve as the module child — the wrong identity.
/// Only a `cfg_attr` head fails closed; unrelated unlexable attributes must
/// not be read as conditional paths.
#[test]
fn unlexable_cfg_attr_conditions_fail_closed_to_conditional() {
    // Unicode identifier condition that introduces a path: conditional.
    assert!(super::attributes_conditionally_introduce_path([
        "#[cfg_attr(é, path = \"alternate.rs\")]",
    ]));
    // Inner-head spelling with surrounding whitespace.
    assert!(super::attributes_conditionally_introduce_path([
        "#![ cfg_attr( φόβος , path = \"inner.rs\") ]",
    ]));
    // The condition is unreadable but no path is introduced: still
    // conditional (unreadable arguments could hide a nested introduction).
    assert!(super::attributes_conditionally_introduce_path([
        "#[cfg_attr(测试, allow(dead_code))]",
    ]));
    // Non-cfg_attr controls: Unicode in an unrelated attribute cannot
    // manufacture a conditional path.
    assert!(!super::attributes_conditionally_introduce_path([
        "#[allow(é)]",
    ]));
    assert!(!super::attributes_conditionally_introduce_path([
        "#[doc = \"café\"]",
    ]));
    // cfg_attr-prefixed lookalike heads are different attributes.
    assert!(!super::attributes_conditionally_introduce_path([
        "#[cfg_attrs(é, path = \"x.rs\")]",
    ]));
    assert!(!super::attributes_conditionally_introduce_path([
        "#[my_cfg_attr(é, path = \"x.rs\")]",
    ]));
}

#[test]
fn multiple_attributes_compose_conjunctively_without_optimism() {
    // A definitely test-required gate dominates independent gates.
    assert!(attributes_require_test([
        "#[cfg(unix)]",
        "#[cfg(all(test, windows))]",
    ]));
    // Alternatives never promote, even next to other gates.
    assert!(!attributes_require_test([
        "#[cfg(any(test, unix))]",
        "#[cfg(windows)]",
    ]));
    // An unknown gate cannot strengthen an otherwise non-test item.
    assert!(!attributes_require_test([
        "#[cfg(unix)]",
        "#[cfg(all(test]"
    ]));
    // Optimistic cfg_attr handling would flip this control; the closed
    // model keeps it production (focused negative control, #3530).
    assert!(!attributes_require_test([
        "#[cfg_attr(feature = \"internal-tests\", cfg(test))]"
    ]));
}

#[test]
fn split_leading_attribute_extracts_balanced_prefix_with_literals() {
    assert_eq!(
        split_leading_attribute("#[cfg(test)] mod tests {"),
        Some(("#[cfg(test)]", " mod tests {"))
    );
    assert_eq!(
        split_leading_attribute("#![cfg(test)]"),
        Some(("#![cfg(test)]", ""))
    );
    // Bracket-looking string content must not close the attribute early.
    assert_eq!(
        split_leading_attribute("#[cfg(feature = \"]\")] let x = 1;"),
        Some(("#[cfg(feature = \"]\")]", " let x = 1;"))
    );
    assert_eq!(
        split_leading_attribute("   #[ cfg(test) ] mod m {"),
        Some(("#[ cfg(test) ]", " mod m {"))
    );
    // Incomplete or non-attribute text yields no split (fail closed).
    assert_eq!(split_leading_attribute("#[cfg(test"), None);
    assert_eq!(split_leading_attribute("fn plain() {}"), None);
    assert_eq!(split_leading_attribute(""), None);
}

#[test]
fn deeply_nested_predicates_fail_closed_instead_of_overflowing() {
    let deep = format!(
        "#[cfg(all({}test{}))]",
        "all(".repeat(4_000),
        ")".repeat(4_000)
    );
    assert_eq!(
        super::classify_attribute(&deep),
        CfgTestRequirement::Unknown,
        "exotic nesting depth must fail closed, not abort"
    );
}

#[test]
fn split_handles_leading_whitespace_and_trailing_non_ascii() {
    let text = "   #[ cfg(test) ] mod m {";
    assert_eq!(
        super::split_leading_attribute(text),
        Some(("#[ cfg(test) ]", " mod m {"))
    );
    let non_ascii = "#[cfg(test)] mod módulos {";
    assert_eq!(
        super::split_leading_attribute(non_ascii),
        Some(("#[cfg(test)]", " mod módulos {"))
    );
}
