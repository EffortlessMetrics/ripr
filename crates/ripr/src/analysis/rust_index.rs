use crate::config::OraclePolicy;
#[cfg(test)]
use crate::domain::{OracleKind, OracleStrength};
use std::path::Path;
#[cfg(test)]
use std::path::PathBuf;

#[cfg(test)]
pub(crate) use super::extract::contains_macro_invocation;
pub(crate) use super::extract::{
    PROBE_SHAPE_CALL_DELETION, PROBE_SHAPE_ERROR_PATH, PROBE_SHAPE_FIELD_CONSTRUCTION,
    PROBE_SHAPE_MATCH_ARM, PROBE_SHAPE_PREDICATE, PROBE_SHAPE_RETURN_VALUE,
    PROBE_SHAPE_SIDE_EFFECT, classify_assertion, extract_assertions, extract_call_facts,
    extract_identifier_tokens, extract_line_scanned_oracles, extract_literal_facts,
    extract_literals, extract_return_facts,
};
pub(crate) use super::facts::build_index_from_loaded_files_with_cache;
#[cfg(test)]
pub use super::facts::{CallFact, LiteralFact, ReturnFact};
pub use super::facts::{
    FileFacts, FunctionFact, FunctionSummary, OracleFact, ProbeShapeFact, RustIndex, TestFact,
    TestSummary, build_index,
};
#[cfg(test)]
use super::syntax::LexicalRustSyntaxAdapter;
pub use super::syntax::{RaRustSyntaxAdapter, RustSyntaxAdapter, SyntaxNodeFact, TextRange};

pub(crate) fn apply_oracle_policy(index: &mut RustIndex, policy: &OraclePolicy) {
    for test in &mut index.tests {
        apply_oracle_policy_to_assertions(&mut test.assertions, policy);
    }
    for facts in index.files.values_mut() {
        for test in &mut facts.tests {
            apply_oracle_policy_to_assertions(&mut test.assertions, policy);
        }
    }
}

fn apply_oracle_policy_to_assertions(assertions: &mut [OracleFact], policy: &OraclePolicy) {
    for assertion in assertions {
        assertion.strength = policy.strength_for_kind(&assertion.kind, assertion.strength.clone());
    }
}

#[cfg(test)]
fn summarize_file(path: PathBuf, text: String) -> FileFacts {
    match RaRustSyntaxAdapter.summarize_file(&path, &text) {
        Ok(facts) => facts,
        Err(_) => super::syntax::lexical::summarize_file_lexically(path, text),
    }
}

pub fn find_owner_function<'a>(
    index: &'a RustIndex,
    file: &Path,
    line: usize,
) -> Option<&'a FunctionSummary> {
    index.files.get(file).and_then(|summary| {
        summary
            .functions
            .iter()
            .filter(|f| f.start_line <= line && line <= f.end_line)
            .max_by_key(|f| f.start_line)
    })
}

pub fn changed_nodes_for_lines(
    index: &RustIndex,
    file: &Path,
    lines: &[usize],
) -> Vec<SyntaxNodeFact> {
    let Some(facts) = index.files.get(file) else {
        return Vec::new();
    };
    let ranges = lines
        .iter()
        .map(|line| TextRange {
            start_line: *line,
            start_column: 1,
            end_line: *line,
            end_column: usize::MAX,
        })
        .collect::<Vec<_>>();
    RaRustSyntaxAdapter.changed_nodes(facts, &ranges)
}

pub(crate) fn is_test_file(path: &Path) -> bool {
    path.starts_with("tests")
        || path
            .to_string_lossy()
            .replace('\\', "/")
            .contains("/tests/")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_tests_and_assertions() {
        let file = summarize_file(
            PathBuf::from("src/lib.rs"),
            r#"
#[test]
fn checks_error() {
    let result = parse("x");
    assert!(result.is_err());
}
"#
            .to_string(),
        );
        assert_eq!(file.tests.len(), 1);
        assert_eq!(file.tests[0].assertions.len(), 1);
        assert_eq!(file.tests[0].assertions[0].kind, OracleKind::BroadError);
        assert_eq!(file.tests[0].assertions[0].strength, OracleStrength::Weak);
    }

    #[test]
    fn classifies_exact_error_variants_separately_from_broad_error_shapes() {
        let exact = classify_assertion("assert_matches!(result, Err(AuthError::RevokedToken));");
        let broad = classify_assertion("assert_matches!(result, Err(_));");
        let ok_pattern = classify_assertion("assert_matches!(result, Ok(Value::Ready));");

        assert_eq!(exact.kind, OracleKind::ExactErrorVariant);
        assert_eq!(exact.strength, OracleStrength::Strong);
        assert_eq!(broad.kind, OracleKind::BroadError);
        assert_eq!(broad.strength, OracleStrength::Weak);
        assert_eq!(ok_pattern.kind, OracleKind::ExactValue);
        assert_eq!(ok_pattern.strength, OracleStrength::Strong);
    }

    #[test]
    fn snapshot_macro_detection_uses_invocation_boundaries() {
        assert!(contains_macro_invocation(
            "insta::assert_snapshot!(value)",
            "assert_snapshot!"
        ));
        assert!(contains_macro_invocation(
            "assert_snapshot! (value)",
            "assert_snapshot!"
        ));
        assert!(contains_macro_invocation(
            "assert_snapshot![value]",
            "assert_snapshot!"
        ));
        assert!(contains_macro_invocation(
            "assert_snapshot!{value}",
            "assert_snapshot!"
        ));
        assert!(!contains_macro_invocation(
            "my_assert_snapshot!(value)",
            "assert_snapshot!"
        ));
        assert!(!contains_macro_invocation(
            "assert_snapshot_extra!(value)",
            "assert_snapshot!"
        ));
        assert!(!contains_macro_invocation(
            "assert_snapshot!value",
            "assert_snapshot!"
        ));
        assert!(!contains_macro_invocation(
            "assert_snapshot! value",
            "assert_snapshot!"
        ));
    }

    #[test]
    fn classifies_snapshot_mock_relational_smoke_and_unknown_oracles() {
        let snapshot_cases = [
            "insta::assert_snapshot!(rendered);",
            "insta::assert_yaml_snapshot!(payload);",
            "assert_snapshot!(rendered);",
            "assert_json_snapshot!(payload);",
            "assert_debug_snapshot!(payload);",
            "assert_csv_snapshot!(payload);",
            "assert_compact_debug_snapshot!(payload);",
            "assert_compact_json_snapshot!(payload);",
            "assert_binary_snapshot!(artifact);",
            r##"expect![[r#"ok"#]].assert_eq(&rendered);"##,
            r##"expect![[r#"ok"#]].assert_debug_eq(&rendered);"##,
            r##"expect![[r#"ok"#]].assert_json_eq(&rendered);"##,
            r#"expect_file!["snapshots/render.snap"].assert_eq(&rendered);"#,
            r#"expect_file!["snapshots/render.snap"].assert_debug_eq(&rendered);"#,
            r#"expect_file!["snapshots/render.snap"].assert_json_eq(&rendered);"#,
        ];

        for case in snapshot_cases {
            let snapshot = classify_assertion(case);
            assert_eq!(snapshot.kind, OracleKind::Snapshot, "case: {case}");
            assert_eq!(snapshot.strength, OracleStrength::Medium, "case: {case}");
        }

        let bare_expect_file = classify_assertion(r#"let expected = expect_file!["render.snap"];"#);
        let non_snapshot_method = classify_assertion("helper.assert_eq(&rendered);");
        let non_snapshot_insta_assertion = classify_assertion("insta::assert_redacted!(payload);");
        let unrelated_snapshot_macro = classify_assertion("snapshot!(rendered);");
        let mock = classify_assertion("mock.expect_publish().times(1);");
        let relational = classify_assertion("assert!(total > 0);");
        let smoke = classify_assertion("assert!(result.is_ok());");
        let unknown = classify_assertion("helper_records_observation();");

        assert_ne!(bare_expect_file.kind, OracleKind::Snapshot);
        assert_ne!(non_snapshot_method.kind, OracleKind::Snapshot);
        assert_ne!(non_snapshot_insta_assertion.kind, OracleKind::Snapshot);
        assert_ne!(unrelated_snapshot_macro.kind, OracleKind::Snapshot);
        assert_eq!(mock.kind, OracleKind::MockExpectation);
        assert_eq!(mock.strength, OracleStrength::Medium);
        assert_eq!(relational.kind, OracleKind::RelationalCheck);
        assert_eq!(relational.strength, OracleStrength::Weak);
        assert_eq!(smoke.kind, OracleKind::SmokeOnly);
        assert_eq!(smoke.strength, OracleStrength::Smoke);
        assert_eq!(unknown.kind, OracleKind::Unknown);
        assert_eq!(unknown.strength, OracleStrength::Unknown);
    }

    #[test]
    fn classifies_field_whole_object_side_effect_and_custom_helper_oracles() {
        let field = classify_assertion("assert_eq!(quote.total, 100);");
        let whole_object = classify_assertion("assert_eq!(quote, Quote { total: 100 });");
        let side_effect = classify_assertion("assert!(events.published().contains(&Event::Sent));");
        let custom_helper = classify_assertion("assert_total_matches(&quote, 100);");
        let mock_setup = classify_assertion("let mock_service = MockPublisher::new();");
        let mock_expectation = classify_assertion("mock_service.expect_publish().times(1);");

        assert_eq!(field.kind, OracleKind::ExactValue);
        assert_eq!(field.strength, OracleStrength::Strong);
        assert_eq!(whole_object.kind, OracleKind::WholeObjectEquality);
        assert_eq!(whole_object.strength, OracleStrength::Strong);
        assert_eq!(side_effect.kind, OracleKind::MockExpectation);
        assert_eq!(side_effect.strength, OracleStrength::Medium);
        assert_eq!(custom_helper.kind, OracleKind::ExactValue);
        assert_eq!(custom_helper.strength, OracleStrength::Strong);
        assert_eq!(mock_setup.kind, OracleKind::Unknown);
        assert_eq!(mock_setup.strength, OracleStrength::Unknown);
        assert_eq!(mock_expectation.kind, OracleKind::MockExpectation);
        assert_eq!(mock_expectation.strength, OracleStrength::Medium);
    }

    #[test]
    fn parser_adapter_extracts_custom_helper_and_side_effect_oracles() -> Result<(), String> {
        let adapter = RaRustSyntaxAdapter;
        let facts = adapter.summarize_file(
            Path::new("tests/oracle_shape.rs"),
            r#"
#[test]
fn event_is_published() {
    publish_message();
    let mock_service = MockPublisher::new();
    assert_event_published("invoice.created");
    mock_service.expect_publish().times(1);
}
"#,
        )?;

        let test = facts
            .tests
            .iter()
            .find(|test| test.name == "event_is_published")
            .ok_or_else(|| "expected test fact".to_string())?;
        assert!(
            test.assertions
                .iter()
                .any(|oracle| oracle.kind == OracleKind::MockExpectation),
            "parser path should extract side-effect observer oracles: {:?}",
            test.assertions
        );
        assert!(
            test.assertions
                .iter()
                .any(|oracle| oracle.text.contains("assert_event_published")),
            "custom assertion helper should be captured: {:?}",
            test.assertions
        );
        assert!(
            test.assertions
                .iter()
                .all(|oracle| !oracle.text.contains("MockPublisher::new")),
            "mock setup should not be captured as an oracle: {:?}",
            test.assertions
        );
        Ok(())
    }

    #[test]
    fn summarize_file_emits_file_facts() {
        let file = summarize_file(
            PathBuf::from("src/lib.rs"),
            r#"
pub fn parse(input: &str) -> Result<i32, Error> {
    if input == "42" {
        return Ok(42);
    }
    Err(Error::Bad)
}
"#
            .to_string(),
        );

        assert_eq!(file.path, PathBuf::from("src/lib.rs"));
        assert_eq!(file.functions.len(), 1);
        assert_eq!(file.functions[0].name, "parse");
        assert!(file.calls.iter().any(|call| call.name == "Ok"));
        assert!(file.returns.iter().any(|fact| fact.text.contains("Ok(42)")));
        assert!(file.literals.iter().any(|fact| fact.value == "42"));
        assert!(
            file.probe_shapes
                .iter()
                .any(|shape| shape.kind == PROBE_SHAPE_RETURN_VALUE)
        );
    }

    #[test]
    fn lexical_adapter_exposes_syntax_boundary() -> Result<(), String> {
        let adapter = LexicalRustSyntaxAdapter;
        let facts = adapter.summarize_file(
            Path::new("src/lib.rs"),
            r#"
pub fn price(amount: i32) -> i32 {
    if amount > 10 { amount - 1 } else { amount }
}
"#,
        )?;
        let nodes = adapter.changed_nodes(
            &facts,
            &[TextRange {
                start_line: 3,
                start_column: 5,
                end_line: 3,
                end_column: 40,
            }],
        );

        assert_eq!(facts.functions.len(), 1);
        assert_eq!(nodes.len(), 1);
        assert_eq!(nodes[0].kind, "function");
        assert_eq!(
            nodes[0].owner.as_ref().map(|owner| owner.0.as_str()),
            Some("src/lib.rs::price")
        );
        Ok(())
    }

    #[test]
    fn parser_owner_symbols_include_module_paths() -> Result<(), String> {
        let adapter = RaRustSyntaxAdapter;
        let facts = adapter.summarize_file(
            Path::new("src/lib.rs"),
            r#"
mod pricing {
    pub fn score(amount: i32) -> i32 {
        amount + 1
    }
}

mod reporting {
    pub fn score(amount: i32) -> i32 {
        amount + 2
    }
}
"#,
        )?;
        let ids = facts
            .functions
            .iter()
            .map(|function| function.id.0.as_str())
            .collect::<Vec<_>>();

        assert!(ids.contains(&"src/lib.rs::pricing::score"));
        assert!(ids.contains(&"src/lib.rs::reporting::score"));
        Ok(())
    }

    #[test]
    fn parser_owner_symbols_include_impl_targets() -> Result<(), String> {
        let adapter = RaRustSyntaxAdapter;
        let facts = adapter.summarize_file(
            Path::new("src/lib.rs"),
            r#"
struct Discount;

impl Discount {
    pub fn score(&self, amount: i32) -> i32 {
        amount + 1
    }
}

struct Tax;

impl Tax {
    pub fn score(&self, amount: i32) -> i32 {
        amount + 2
    }
}
"#,
        )?;
        let ids = facts
            .functions
            .iter()
            .map(|function| function.id.0.as_str())
            .collect::<Vec<_>>();

        assert!(ids.contains(&"src/lib.rs::impl Discount::score"));
        assert!(ids.contains(&"src/lib.rs::impl Tax::score"));
        Ok(())
    }

    #[test]
    fn changed_nodes_use_module_qualified_owner() -> Result<(), String> {
        let adapter = RaRustSyntaxAdapter;
        let source = r#"
mod pricing {
    pub fn score(amount: i32) -> i32 {
        if amount >= 100 { 90 } else { 100 }
    }
}

mod reporting {
    pub fn score(amount: i32) -> i32 {
        amount + 2
    }
}
"#;
        let facts = adapter.summarize_file(Path::new("src/lib.rs"), source)?;
        let changed_line = line_containing(source, "amount >= 100")?;
        let mut index = RustIndex::default();
        index.files.insert(PathBuf::from("src/lib.rs"), facts);
        let nodes = changed_nodes_for_lines(&index, Path::new("src/lib.rs"), &[changed_line]);

        assert_eq!(nodes.len(), 1);
        assert_eq!(
            nodes[0].owner.as_ref().map(|owner| owner.0.as_str()),
            Some("src/lib.rs::pricing::score")
        );
        Ok(())
    }

    #[test]
    fn changed_nodes_preserve_test_owner_under_cfg_module() -> Result<(), String> {
        let adapter = RaRustSyntaxAdapter;
        let source = r#"
#[cfg(test)]
mod tests {
    #[test]
    fn checks_boundary() {
        assert_eq!(discounted_total(100), 90);
    }
}
"#;
        let facts = adapter.summarize_file(Path::new("src/lib.rs"), source)?;
        let changed_line = line_containing(source, "discounted_total")?;
        let mut index = RustIndex::default();
        index.files.insert(PathBuf::from("src/lib.rs"), facts);
        let nodes = changed_nodes_for_lines(&index, Path::new("src/lib.rs"), &[changed_line]);

        assert_eq!(nodes.len(), 1);
        assert_eq!(nodes[0].kind, "test_function");
        assert_eq!(
            nodes[0].owner.as_ref().map(|owner| owner.0.as_str()),
            Some("src/lib.rs::tests::checks_boundary")
        );
        Ok(())
    }

    #[test]
    fn parser_adapter_extracts_probe_shapes_from_syntax() -> Result<(), String> {
        let adapter = RaRustSyntaxAdapter;
        let facts = adapter.summarize_file(
            Path::new("src/lib.rs"),
            r#"
pub fn classify(amount: i32, service: &mut Service) -> Result<Quote, Error> {
    if amount >= 100 {
        service.publish(
            Event::Discounted,
        );
        return Ok(Quote {
            total: 90,
        });
    }

    let _marker = match service.kind() {
        "=>" => 1,
        _ => 0,
    };

    match amount {
        0 => Err(Error::Zero),
        _ => Ok(Quote { total: amount }),
    }
}
"#,
        )?;
        let kinds = facts
            .probe_shapes
            .iter()
            .map(|shape| shape.kind.as_str())
            .collect::<Vec<_>>();

        assert!(kinds.contains(&PROBE_SHAPE_PREDICATE));
        assert!(kinds.contains(&PROBE_SHAPE_RETURN_VALUE));
        assert!(kinds.contains(&PROBE_SHAPE_ERROR_PATH));
        assert!(kinds.contains(&PROBE_SHAPE_CALL_DELETION));
        assert!(kinds.contains(&PROBE_SHAPE_FIELD_CONSTRUCTION));
        assert!(kinds.contains(&PROBE_SHAPE_SIDE_EFFECT));
        assert!(kinds.contains(&PROBE_SHAPE_MATCH_ARM));

        let match_shapes = facts
            .probe_shapes
            .iter()
            .filter(|shape| shape.kind == PROBE_SHAPE_MATCH_ARM)
            .map(|shape| shape.text.as_str())
            .collect::<Vec<_>>();
        assert!(match_shapes.contains(&"match amount"));
        assert!(match_shapes.contains(&"match service.kind()"));
        assert!(match_shapes.contains(&r#""=>" =>"#));
        assert!(match_shapes.contains(&"0 =>"));
        assert!(match_shapes.contains(&"_ =>"));
        assert!(!match_shapes.contains(&"=>"));
        Ok(())
    }

    #[test]
    fn parser_adapter_extracts_multiline_assertion_macro() -> Result<(), String> {
        let adapter = RaRustSyntaxAdapter;
        let facts = adapter.summarize_file(
            Path::new("tests/pricing.rs"),
            r#"
use fixture::discounted_total;

#[test]
#[cfg_attr(feature = "slow", ignore)]
fn exact_boundary_value_is_checked() {
    assert_eq!(
        discounted_total(100, 100),
        90
    );
}
"#,
        )?;

        assert_eq!(facts.tests.len(), 1);
        assert_eq!(facts.tests[0].name, "exact_boundary_value_is_checked");
        assert_eq!(facts.tests[0].start_line, 6);
        assert_eq!(facts.tests[0].assertions.len(), 1);
        assert_eq!(facts.tests[0].assertions[0].line, 7);
        assert_eq!(
            facts.tests[0].assertions[0].strength,
            OracleStrength::Strong
        );
        assert_eq!(facts.tests[0].assertions[0].kind, OracleKind::ExactValue);
        assert!(facts.tests[0].assertions[0].text.contains("assert_eq!"));
        assert!(
            facts.tests[0].assertions[0]
                .text
                .contains("discounted_total(100, 100)")
        );
        Ok(())
    }

    #[test]
    fn parser_adapter_treats_unwrap_and_expect_as_smoke_oracles() -> Result<(), String> {
        let adapter = RaRustSyntaxAdapter;
        let expect_call = format!(r#"    parse("").{}("parse succeeds");"#, "expect");
        let unwrap_call = format!(r#"    parse("42").{}();"#, "unwrap");
        let source = [
            "",
            "#[test]",
            "fn only_smoke_checks_error_path() {",
            expect_call.as_str(),
            unwrap_call.as_str(),
            "}",
            "",
        ]
        .join("\n");
        let facts = adapter.summarize_file(Path::new("tests/errors.rs"), &source)?;

        let assertions = &facts.tests[0].assertions;
        assert_eq!(assertions.len(), 2);
        assert_eq!(assertions[0].kind, OracleKind::SmokeOnly);
        assert_eq!(assertions[0].strength, OracleStrength::Smoke);
        assert_eq!(assertions[1].kind, OracleKind::SmokeOnly);
        assert_eq!(assertions[1].strength, OracleStrength::Smoke);
        assert!(
            assertions
                .iter()
                .any(|assertion| assertion.text.contains("expect"))
        );
        assert!(
            assertions
                .iter()
                .any(|assertion| assertion.text.contains("unwrap"))
        );
        Ok(())
    }

    #[test]
    fn preserves_test_marker_across_stacked_attributes() {
        let file = summarize_file(
            PathBuf::from("src/lib.rs"),
            r#"
#[test]
#[should_panic]
fn panics_on_bad_input() {}

#[test]
#[ignore]
fn slow_but_real_test() {}

#[test]
#[cfg(feature = "foo")]
fn feature_gated_test() {}
"#
            .to_string(),
        );
        let names = file
            .tests
            .iter()
            .map(|test| test.name.as_str())
            .collect::<Vec<_>>();
        assert_eq!(
            names,
            vec![
                "panics_on_bad_input",
                "slow_but_real_test",
                "feature_gated_test"
            ]
        );
    }

    fn line_containing(source: &str, needle: &str) -> Result<usize, String> {
        match source.lines().position(|line| line.contains(needle)) {
            Some(index) => Ok(index + 1),
            None => Err(format!("missing line containing {needle}")),
        }
    }
}
