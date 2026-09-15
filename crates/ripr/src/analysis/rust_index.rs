#[cfg(test)]
use super::facts::RustIncludeLimitation;
use crate::config::OraclePolicy;
#[cfg(test)]
use crate::domain::{OracleKind, OracleStrength};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

#[cfg(test)]
pub(crate) use super::extract::contains_macro_invocation;
pub(crate) use super::extract::{
    OracleTextShape, PROBE_SHAPE_CALL_DELETION, PROBE_SHAPE_ERROR_PATH,
    PROBE_SHAPE_FIELD_CONSTRUCTION, PROBE_SHAPE_MATCH_ARM, PROBE_SHAPE_PREDICATE,
    PROBE_SHAPE_RETURN_VALUE, PROBE_SHAPE_SIDE_EFFECT, classify_assertion,
    err_return_guard_oracles, extract_assertions, extract_call_facts, extract_identifier_tokens,
    extract_line_scanned_oracles, extract_literal_facts, extract_literals, extract_return_facts,
    guarded_result_match_scan_with_shadow_authority, has_oracle_text_shape, is_known_probe_shape,
    is_unwrap_err_bound_error_assertion, unwrap_err_bound_variables,
};
use super::facts::ModulePathTarget;
pub(crate) use super::facts::build_index_from_loaded_files_with_cache_and_test_harnesses;
pub(crate) use super::facts::validated_file_wide_harness_targets;
#[cfg(test)]
pub use super::facts::{CallFact, LiteralFact, ReturnFact};
pub use super::facts::{
    FileFacts, FunctionFact, FunctionSummary, OracleFact, ProbeShapeFact, RustIndex, TestFact,
    TestSummary, build_index, build_index_with_test_harnesses,
};
#[cfg(test)]
use super::syntax::LexicalRustSyntaxAdapter;
pub use super::syntax::{RaRustSyntaxAdapter, RustSyntaxAdapter, SyntaxNodeFact, TextRange};

pub(crate) fn lexical_fallback_files(index: &RustIndex) -> Vec<PathBuf> {
    let mut files = index
        .files
        .values()
        .filter(|facts| facts.used_lexical_fallback)
        .map(|facts| facts.path.clone())
        .collect::<Vec<_>>();
    files.sort_unstable();
    files
}

pub(crate) fn lexical_fallback_disclosure_for_files(files: &[PathBuf]) -> Option<String> {
    if files.is_empty() {
        return None;
    }
    let mut displayed = files
        .iter()
        .map(|path| escaped_path_display(path))
        .collect::<Vec<_>>();
    displayed.sort_unstable();
    Some(format!(
        "ripr: lexical fallback was used for {} Rust file(s): {}; repo seam inventory may under-credit these files because lexical fallback emits no probe shapes.",
        displayed.len(),
        displayed.join(", ")
    ))
}

fn escaped_path_display(path: &Path) -> String {
    let mut displayed = String::new();
    for character in path.to_string_lossy().chars() {
        match character {
            '\n' => displayed.push_str("\\n"),
            '\r' => displayed.push_str("\\r"),
            '\t' => displayed.push_str("\\t"),
            '\u{1b}' => displayed.push_str("\\x1b"),
            character if character.is_control() => {
                displayed.push_str(&format!("\\u{{{:04x}}}", character as u32));
            }
            character => displayed.push(character),
        }
    }
    displayed
}

/// Returns a stable disclosure when indexed Rust files used lexical fallback.
pub(crate) fn lexical_fallback_disclosure(index: &RustIndex) -> Option<String> {
    lexical_fallback_disclosure_for_files(&lexical_fallback_files(index))
}

pub(crate) fn compilation_unit_path(index: &RustIndex, file: &Path) -> PathBuf {
    super::facts::compilation_unit_path_from_parents(&index.include_parents, file)
}

pub(crate) fn include_resolution_disclosure(index: &RustIndex) -> Option<String> {
    if index.include_limitations.is_empty() {
        return None;
    }
    let details = index
        .include_limitations
        .iter()
        .map(|limitation| {
            format!(
                "{}:{}:{}",
                escaped_path_display(&limitation.parent),
                limitation.line,
                limitation.reason_code
            )
        })
        .collect::<Vec<_>>()
        .join(", ");
    Some(format!(
        "ripr: {} Rust include boundary limitation(s): {details}; affected compilation-unit relations remain fail-closed.",
        index.include_limitations.len()
    ))
}

/// Returns a stable disclosure when Rust module composition failed closed
/// (#3533): a file whose composed context chain could not be resolved
/// (ambiguous ownership, cycle or depth bound, context conflict), or a
/// `mod` declaration whose target is a typed unknown (dynamic or
/// conditionally introduced `#[path]`). Without this, module-side stop
/// reasons were invisible outside the per-file provenance while include-side
/// limitations were disclosed.
pub(crate) fn module_composition_disclosure(index: &RustIndex) -> Option<String> {
    let mut details = BTreeSet::new();
    let mut count = 0usize;
    for (file, facts) in &index.files {
        if let Some(reason) = &facts.role_provenance.earliest_unresolved_reason
            && reason.starts_with("rust_module_")
        {
            details.insert(format!("{}:{reason}", escaped_path_display(file)));
            count += 1;
        }
        for declaration in &facts.module_declarations {
            if declaration.path_target == ModulePathTarget::Unknown {
                let inserted = details.insert(format!(
                    "{}:{}:rust_module_unresolved_target",
                    escaped_path_display(file),
                    declaration.line
                ));
                if inserted {
                    count += 1;
                }
            }
        }
    }
    if count == 0 {
        return None;
    }
    Some(format!(
        "ripr: {count} Rust module composition limitation(s): {}; affected module contexts remain fail-closed.",
        details.into_iter().collect::<Vec<_>>().join(", ")
    ))
}

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
        Err(_) => {
            let mut facts = super::syntax::lexical::summarize_file_lexically(path, text);
            facts.used_lexical_fallback = true;
            facts
        }
    }
}

pub fn find_owner_function<'a>(
    index: &'a RustIndex,
    file: &Path,
    line: usize,
) -> Option<&'a FunctionSummary> {
    find_file_facts(index, file)?
        .functions
        .iter()
        .filter(|f| f.start_line <= line && line <= f.end_line)
        .max_by_key(|f| f.start_line)
}

pub(crate) fn find_file_facts<'a>(index: &'a RustIndex, file: &Path) -> Option<&'a FileFacts> {
    if let Some(summary) = index.files.get(file) {
        return Some(summary);
    }
    // Repo seams normalize their file identity to `/` separators
    // (#3469 family) while index keys keep the producing host's
    // separators. On the opposite-separator host the component-wise
    // lookup above misses (`src\pricing.rs` is a single component on
    // Linux), so fall back to matching normalized key forms. The scan
    // only runs after the direct lookup missed, so native-separator
    // inputs keep the exact-lookup cost. Non-UTF-8 paths fail closed:
    // lossy replacement characters can collapse distinct names into
    // one form and attribute the wrong owner, so neither side of the
    // comparison may pass through lossy conversion.
    let target = file.to_str()?.replace('\\', "/");
    index.files.iter().find_map(|(key, summary)| {
        let key_text = key.to_str()?;
        (key_text.replace('\\', "/") == target).then_some(summary)
    })
}

/// `/`-separated display form of a path, independent of the host's
/// native separator. Used by [`is_test_file`] to judge the normalized
/// form; lossy text is safe here because replacement characters can
/// never become separators. The [`find_owner_function`] fallback does
/// NOT use this helper — identity attribution must not pass through
/// lossy conversion.
fn normalize_display(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
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
    let normalized = normalize_display(path);
    normalized == "tests" || normalized.starts_with("tests/") || normalized.contains("/tests/")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn find_owner_function_resolves_normalized_seam_paths() {
        let facts = summarize_file(
            PathBuf::from("src\\pricing.rs"),
            "fn apply_discount(amount: i32, threshold: i32) -> i32 { \
                 if amount >= threshold { amount - 10 } else { amount } }\n"
                .to_string(),
        );
        let mut index = RustIndex::default();
        index.files.insert(PathBuf::from("src\\pricing.rs"), facts);
        // Repo seams carry `/`-normalized file identity; on Linux this is
        // not component-equal to the `\`-separator index key, so the
        // direct lookup misses and the normalized fallback must resolve.
        let owner = find_owner_function(&index, Path::new("src/pricing.rs"), 1);
        assert_eq!(owner.map(|f| f.name.as_str()), Some("apply_discount"));
    }

    #[test]
    fn is_test_file_judges_foreign_separator_forms_like_native_ones() {
        assert!(is_test_file(Path::new("tests\\pricing_test.rs")));
        assert!(is_test_file(Path::new("tests/pricing_test.rs")));
        assert!(is_test_file(Path::new("crates\\demo\\tests\\main.rs")));
        assert!(!is_test_file(Path::new("src\\pricing.rs")));
        // A `tests`-prefixed sibling directory name stays production.
        assert!(!is_test_file(Path::new("testing\\pricing.rs")));
    }

    #[test]
    #[cfg(unix)]
    fn find_owner_function_fails_closed_on_non_utf8_fallback() {
        use std::ffi::OsStr;
        use std::os::unix::ffi::OsStrExt;
        let facts = summarize_file(
            PathBuf::from(OsStr::from_bytes(b"src/pricing_\xff.rs")),
            "fn apply_discount(amount: i32) -> i32 { amount }\n".to_string(),
        );
        let mut index = RustIndex::default();
        index.files.insert(
            PathBuf::from(OsStr::from_bytes(b"src/pricing_\xff.rs")),
            facts,
        );
        // A query whose lossy rendering collides with the indexed key
        // must not attribute the owner through lossy text: the fallback
        // requires valid UTF-8 on both sides.
        let colliding = Path::new(OsStr::from_bytes(b"src\\pricing_\xfe.rs"));
        assert_eq!(
            find_owner_function(&index, colliding, 1).map(|f| f.name.as_str()),
            None
        );
    }

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
        let opaque_helper = classify_assertion("assert_discount_is_valid(&quote);");
        let duplicated_equality = classify_assertion("assert_eq!(quote.total, quote.total);");
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
        assert_eq!(opaque_helper.kind, OracleKind::Unknown);
        assert_eq!(opaque_helper.strength, OracleStrength::Unknown);
        assert_eq!(duplicated_equality.kind, OracleKind::RelationalCheck);
        assert_eq!(duplicated_equality.strength, OracleStrength::Weak);
        assert_eq!(mock_setup.kind, OracleKind::Unknown);
        assert_eq!(mock_setup.strength, OracleStrength::Unknown);
        assert_eq!(mock_expectation.kind, OracleKind::MockExpectation);
        assert_eq!(mock_expectation.strength, OracleStrength::Medium);
    }

    #[test]
    fn classifies_only_clear_custom_helpers_as_exact_value_oracles() {
        for exact in [
            "assert_total_matches(&quote, 100);",
            "helpers::assert_amount_equal(actual, 100);",
            "quote.assert_total_eq(100);",
        ] {
            let oracle = classify_assertion(exact);
            assert_eq!(oracle.kind, OracleKind::ExactValue, "case: {exact}");
            assert_eq!(oracle.strength, OracleStrength::Strong, "case: {exact}");
        }

        for opaque in [
            "assert_valid_quote(&quote);",
            "helpers::assert_discount(&quote);",
            "quote.assert_business_rules();",
        ] {
            let oracle = classify_assertion(opaque);
            assert_eq!(oracle.kind, OracleKind::Unknown, "case: {opaque}");
            assert_eq!(oracle.strength, OracleStrength::Unknown, "case: {opaque}");
        }
    }

    #[test]
    fn classifies_duplicative_equality_as_weak_oracle() {
        for duplicative in [
            "assert_eq!(quote.total, quote.total);",
            "assert_eq!(render(actual), render(actual), \"same expression\");",
            "assert_ne!(status, status);",
        ] {
            let oracle = classify_assertion(duplicative);
            assert_eq!(
                oracle.kind,
                OracleKind::RelationalCheck,
                "case: {duplicative}"
            );
            assert_eq!(oracle.strength, OracleStrength::Weak, "case: {duplicative}");
        }

        let exact = classify_assertion("assert_eq!(quote.total, 100);");
        assert_eq!(exact.kind, OracleKind::ExactValue);
        assert_eq!(exact.strength, OracleStrength::Strong);
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

    let _block_marker = match { service.kind() } {
        "block" => 1,
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
        assert!(match_shapes.contains(&"match { service.kind() }"));
        assert!(match_shapes.contains(&r#""=>" =>"#));
        assert!(match_shapes.contains(&r#""block" =>"#));
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
    fn literal_extraction_keeps_decimal_boundaries_intact() {
        let literals = extract_literals("value >= 1_000u32 && ratio < 2.5f64 && delta == -3_i32");

        assert_eq!(literals, vec!["-3", "1000", "2.5"]);
    }

    #[test]
    fn literal_extraction_ignores_subtraction_operator_without_fabricating_negative_literal() {
        let literals = extract_literals("let remaining = total - discount + total-25;");

        assert_eq!(literals, vec!["25"]);
    }

    #[test]
    fn literal_extraction_preserves_radix_boundaries_without_suffixes() {
        let literals = extract_literals("mask == 0xff_u8 || flags == 0b1010usize || mode == 0o77");

        assert_eq!(literals, vec!["0b1010", "0o77", "0xff"]);
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

    #[test]
    fn lexical_fallback_disclosure_is_stable_and_conservative() {
        let mut index = RustIndex::default();
        for path in ["z.rs", "a.rs"] {
            index.files.insert(
                PathBuf::from(path),
                FileFacts {
                    path: PathBuf::from(path),
                    used_lexical_fallback: true,
                    ..FileFacts::default()
                },
            );
        }

        assert_eq!(
            lexical_fallback_disclosure(&index).as_deref(),
            Some(
                "ripr: lexical fallback was used for 2 Rust file(s): a.rs, z.rs; repo seam inventory may under-credit these files because lexical fallback emits no probe shapes."
            )
        );
    }

    #[test]
    fn lexical_fallback_disclosure_escapes_control_characters() {
        let files = vec![
            PathBuf::from("src/line\nreturn\r\t\u{1b}[31m.rs"),
            PathBuf::from("src/unit\u{0007}.rs"),
        ];

        assert_eq!(
            lexical_fallback_disclosure_for_files(&files).as_deref(),
            Some(
                "ripr: lexical fallback was used for 2 Rust file(s): src/line\\nreturn\\r\\t\\x1b[31m.rs, src/unit\\u{0007}.rs; repo seam inventory may under-credit these files because lexical fallback emits no probe shapes."
            )
        );
    }

    #[test]
    fn include_resolution_disclosure_names_stable_reason_and_source() {
        let index = RustIndex {
            include_limitations: vec![RustIncludeLimitation {
                parent: PathBuf::from("src/lib.rs"),
                line: 7,
                expression: "include!(concat!(...))".to_string(),
                reason_code: "rust_include_dynamic_expression".to_string(),
            }],
            ..RustIndex::default()
        };

        assert_eq!(
            include_resolution_disclosure(&index).as_deref(),
            Some(
                "ripr: 1 Rust include boundary limitation(s): src/lib.rs:7:rust_include_dynamic_expression; affected compilation-unit relations remain fail-closed."
            )
        );
    }

    #[test]
    fn module_composition_disclosure_names_stop_reasons_and_unknown_targets() -> Result<(), String>
    {
        let no_disclosure = || String::from("module limitations must be disclosed");
        let mut index = RustIndex::default();
        // A file whose composed context chain failed closed (ambiguous
        // ownership), and a declaration whose target is a typed unknown.
        index.files.insert(
            PathBuf::from("src/shared.rs"),
            FileFacts {
                role_provenance: crate::analysis::facts::SourceRoleProvenance {
                    edges: Vec::new(),
                    earliest_unresolved_reason: Some("rust_module_ambiguous_parent".to_string()),
                },
                ..FileFacts::default()
            },
        );
        index.files.insert(
            PathBuf::from("src/lib.rs"),
            FileFacts {
                module_declarations: vec![crate::analysis::facts::ModuleDeclarationFact {
                    name: "generated".to_string(),
                    line: 4,
                    path_target: ModulePathTarget::Unknown,
                    requires_test: false,
                }],
                ..FileFacts::default()
            },
        );
        // A clean file must not appear.
        index
            .files
            .insert(PathBuf::from("src/plain.rs"), FileFacts::default());

        let disclosure = module_composition_disclosure(&index).ok_or_else(no_disclosure)?;
        assert!(
            disclosure.contains("2 Rust module composition limitation(s)"),
            "both limitation kinds are counted: {disclosure}"
        );
        assert!(
            disclosure.contains("src/lib.rs:4:rust_module_unresolved_target"),
            "typed-unknown targets are named with their line: {disclosure}"
        );
        assert!(
            disclosure.contains("src/shared.rs:rust_module_ambiguous_parent"),
            "composed-chain stop reasons are named: {disclosure}"
        );
        assert!(!disclosure.contains("src/plain.rs"));

        // No limitations: no disclosure.
        let clean = RustIndex::default();
        assert_eq!(module_composition_disclosure(&clean), None);
        Ok(())
    }
}
