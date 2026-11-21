use super::*;
use crate::analysis::syntax::{LexicalRustSyntaxAdapter, RaRustSyntaxAdapter, RustSyntaxAdapter};
use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

fn test_names(facts: &FileFacts) -> Vec<&str> {
    facts.tests.iter().map(|test| test.name.as_str()).collect()
}

fn index_test_names(index: &RustIndex) -> Vec<&str> {
    index.tests.iter().map(|test| test.name.as_str()).collect()
}

fn assert_non_test_functions(facts: &FileFacts, names: &[&str]) {
    for name in names {
        assert!(
            facts
                .functions
                .iter()
                .find(|function| function.name == *name)
                .is_some_and(|function| !function.source_role.is_evidence_role()),
            "ordinary function was incorrectly classified as a test: {name}"
        );
    }
}

#[test]
fn exact_classifier_accepts_supported_test_defining_attributes() {
    let supported = [
        "#[test]",
        "#[ tokio :: test (flavor = \"multi_thread\") ]",
        "#[async_std::test]",
        "#[rstest]",
        "#[rstest::rstest]",
        "#[quickcheck]",
        "#[quickcheck_macros::quickcheck]",
        "#[wasm_bindgen_test(unsupported = test)]",
        "#[wasm_bindgen_test::wasm_bindgen_test]",
        "#[test_case(1)]",
        "#[test_case::test_case(1)]",
        "#[ntest::test_case(1)]",
        "#[test_matrix([1, 2])]",
        "#[test_case::test_matrix([1, 2])] // generated cases",
    ];

    for attribute in supported {
        assert!(
            normalized_test_attribute_path(attribute)
                .as_deref()
                .is_some_and(is_test_attribute_path),
            "supported test-defining attribute was not recognized: {attribute}"
        );
    }
}

#[test]
fn exact_classifier_rejects_prefix_lookalikes_and_ambiguous_attributes() {
    let unsupported = [
        "#[tokio::test_helper]",
        "#[async_std::test_config]",
        "#[rstest_helper]",
        "#[quickcheck_config]",
        "#[wasm_bindgen_test_helper]",
        "#[test_case_helper]",
        "#[test_matrix_config]",
        "#[cfg_attr(test, test)]",
        "#[test = \"custom\"]",
        "#[quickcheck(foo]",
        "#[test_case(1))]",
        "#[test] trailing_tokens",
        "#[proptest]",
        "#[doc(hidden)]",
    ];

    for attribute in unsupported {
        assert!(
            normalized_test_attribute_path(attribute)
                .as_deref()
                .is_none_or(|path| !is_test_attribute_path(path)),
            "non-test or ambiguous attribute was promoted: {attribute}"
        );
    }
}

#[test]
fn parser_facts_recognize_explicit_nonstandard_test_styles() -> Result<(), String> {
    let adapter = RaRustSyntaxAdapter;
    let mut facts = adapter.summarize_file(
        Path::new("src/lib.rs"),
        r#"
#[test]
fn standard_test() { assert!(true); }

#[tokio::test]
async fn tokio_test() { assert!(true); }

#[async_std::test]
async fn async_std_test() { assert!(true); }

#[rstest]
fn rstest_test() { assert!(true); }

#[rstest::rstest]
fn qualified_rstest() { assert!(true); }

#[quickcheck]
fn quickcheck_test(value: u8) -> bool { value == value }

#[quickcheck_macros::quickcheck]
fn qualified_quickcheck(value: u8) -> bool { value == value }

#[wasm_bindgen_test(unsupported = test)]
fn wasm_test() { assert!(true); }

#[wasm_bindgen_test::wasm_bindgen_test]
fn qualified_wasm() { assert!(true); }

#[test_case(1)]
fn test_case_test(value: i32) { assert_eq!(value, 1); }

#[test_case::test_case(1)]
fn qualified_test_case(value: i32) { assert_eq!(value, 1); }

#[ntest::test_case(1)]
fn ntest_case(value: i32) { assert_eq!(value, 1); }

#[test_matrix([1, 2])]
fn test_matrix_test(value: i32) { assert!(value > 0); }

#[test_case::test_matrix([1, 2])]
fn qualified_test_matrix(value: i32) { assert!(value > 0); }

#[tokio::test_helper]
fn tokio_lookalike() {}

#[rstest_helper]
fn rstest_lookalike() {}

#[test_case_helper]
fn test_case_lookalike() {}

#[quickcheck_config]
fn quickcheck_lookalike() {}

fn unannotated_helper() {}

#[cfg(test)]
mod tests {
    fn cfg_test_helper() {}

    #[tokio::test_helper]
    fn cfg_test_lookalike() {}
}
"#,
    )?;

    normalize_file_test_styles(&mut facts);

    assert_eq!(
        test_names(&facts),
        [
            "standard_test",
            "tokio_test",
            "async_std_test",
            "rstest_test",
            "qualified_rstest",
            "quickcheck_test",
            "qualified_quickcheck",
            "wasm_test",
            "qualified_wasm",
            "test_case_test",
            "qualified_test_case",
            "ntest_case",
            "test_matrix_test",
            "qualified_test_matrix",
        ]
    );
    assert_non_test_functions(
        &facts,
        &[
            "tokio_lookalike",
            "rstest_lookalike",
            "test_case_lookalike",
            "quickcheck_lookalike",
            "unannotated_helper",
        ],
    );
    assert!(
        facts
            .functions
            .iter()
            .find(|function| function.name == "cfg_test_helper")
            .is_some_and(|function| function.source_role == FunctionSourceRole::CfgTestModule),
        "cfg(test) helper role must remain the evidence-only cfg-test-module role"
    );
    assert!(
        facts
            .tests
            .iter()
            .all(|test| test.name != "cfg_test_helper"),
        "cfg(test) helper must not become an executable TestFact"
    );
    assert!(
        facts
            .functions
            .iter()
            .find(|function| function.name == "cfg_test_lookalike")
            .is_some_and(|function| function.source_role == FunctionSourceRole::CfgTestModule),
        "cfg(test) lookalike must retain its evidence-role function"
    );
    assert!(
        facts
            .tests
            .iter()
            .all(|test| test.name != "cfg_test_lookalike"),
        "cfg(test) lookalike must not remain an executable TestFact"
    );
    Ok(())
}

#[test]
fn normalizer_preserves_cfg_all_test_module_roles_in_both_conjunct_orders() -> Result<(), String> {
    let adapter = RaRustSyntaxAdapter;
    let mut facts = adapter.summarize_file(
        Path::new("src/lib.rs"),
        r#"
#[cfg(all(feature = "slow", test))]
mod slow_tests {
    fn test_second_helper() {}

    #[test]
    fn test_second_case() { assert!(true); }
}

#[cfg(all(test, feature = "slow"))]
mod test_first_tests {
    fn test_first_helper() {}
}

#[cfg(any(test, feature = "slow"))]
mod any_tests {
    fn any_helper() {}
}
"#,
    )?;

    normalize_file_test_styles(&mut facts);

    for helper in ["test_second_helper", "test_first_helper"] {
        assert!(
            facts
                .functions
                .iter()
                .find(|function| function.name == helper)
                .is_some_and(|function| function.source_role == FunctionSourceRole::CfgTestModule),
            "{helper} must keep its producer-owned cfg-test evidence role"
        );
        assert!(
            facts.tests.iter().all(|test| test.name != helper),
            "{helper} must not become an executable TestFact"
        );
    }
    assert!(
        facts
            .tests
            .iter()
            .any(|test| test.name == "test_second_case"),
        "the executable test inside the test-second cfg(all(...)) module is unaffected"
    );
    assert!(
        facts
            .functions
            .iter()
            .find(|function| function.name == "any_helper")
            .is_some_and(|function| !function.source_role.is_evidence_role()),
        "cfg(any(test, ..)) modules do not grant test roles"
    );
    Ok(())
}

#[test]
fn normalizer_preserves_producer_roles_for_shared_authority_spellings() {
    // #3530: the preservation walk consumes the shared cfg-predicate
    // authority, so whitespace variants and multi-line attribute spellings
    // the producer accepts keep their evidence role, while a non-test cfg
    // gate still fails closed. Reverting the walk to a drifted line matcher
    // demotes the first two helpers and fails this test.
    let source = "\
#[ cfg(test) ]
mod whitespace_gate {
    fn whitespace_helper() {}
}

#[cfg(
    all(unix, test)
)]
mod multiline_gate {
    fn multiline_helper() {}
}

#[cfg(unix)]
mod production_gate {
    fn production_helper() {}
}
";
    let mut facts = FileFacts {
        path: PathBuf::from("src/lib.rs"),
        functions: vec![
            cfg_function_fact("whitespace_helper", 3),
            cfg_function_fact("multiline_helper", 10),
            cfg_function_fact("production_helper", 15),
        ],
        source: source.to_string(),
        ..FileFacts::default()
    };

    normalize_file_test_styles(&mut facts);

    for name in ["whitespace_helper", "multiline_helper"] {
        assert!(
            facts
                .functions
                .iter()
                .find(|function| function.name == name)
                .is_some_and(|function| function.source_role == FunctionSourceRole::CfgTestModule),
            "{name} must keep its producer-owned cfg-test evidence role through the shared authority"
        );
        assert!(
            facts.tests.iter().all(|test| test.name != name),
            "{name} must stay evidence-only without an executable TestFact"
        );
    }
    assert!(
        facts
            .functions
            .iter()
            .find(|function| function.name == "production_helper")
            .is_some_and(|function| !function.source_role.is_evidence_role()),
        "a cfg gate without a test requirement must fail closed"
    );
}

fn cfg_function_fact(name: &str, start_line: usize) -> FunctionFact {
    FunctionFact {
        id: crate::domain::SymbolId(format!("src/lib.rs::{name}:{start_line}")),
        name: name.to_string(),
        file: PathBuf::from("src/lib.rs"),
        start_line,
        end_line: start_line,
        body: String::new(),
        calls: Vec::new(),
        returns: Vec::new(),
        literals: Vec::new(),
        // Models the parser producer's evidence-only cfg-test-module output.
        source_role: FunctionSourceRole::CfgTestModule,
        attrs: Vec::new(),
        nested_fn_names: Vec::new(),
        let_bindings: Vec::new(),
    }
}

#[test]
fn lexical_facts_share_exact_test_style_recognition() -> Result<(), String> {
    let adapter = LexicalRustSyntaxAdapter;
    let mut facts = adapter.summarize_file(
        Path::new("tests/unconventional.rs"),
        r#"
#[test]
fn standard_test() { assert!(true); }

#[ quickcheck ]
fn whitespace_quickcheck(value: u8) -> bool { value == value }

#[quickcheck_macros::quickcheck]
fn qualified_quickcheck(value: u8) -> bool { value == value }

#[test_case (1)]
fn parameterized_case(value: i32) { assert_eq!(value, 1); }

#[wasm_bindgen_test(unsupported = test)]
fn wasm_case() { assert!(true); }

#[rstest_helper]
fn rstest_lookalike() {}

#[tokio::test_helper]
fn tokio_lookalike() {}

#[cfg_attr(test, test)]
fn conditional_test_attribute() {}
"#,
    )?;

    normalize_file_test_styles(&mut facts);

    assert_eq!(
        test_names(&facts),
        [
            "standard_test",
            "whitespace_quickcheck",
            "qualified_quickcheck",
            "parameterized_case",
            "wasm_case",
        ]
    );
    assert!(
        facts.tests.iter().all(|test| test.attrs.is_empty()),
        "lexical fallback must not invent parser-backed attribute facts"
    );
    assert_non_test_functions(
        &facts,
        &[
            "rstest_lookalike",
            "tokio_lookalike",
            "conditional_test_attribute",
        ],
    );
    Ok(())
}

#[test]
fn build_paths_normalize_parser_and_fallback_test_roles() -> Result<(), Box<dyn Error>> {
    struct Cleanup(PathBuf);

    impl Drop for Cleanup {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    let stamp = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
    let root = std::env::temp_dir().join(format!("ripr-test-styles-{stamp}"));
    let _cleanup = Cleanup(root.clone());
    fs::create_dir_all(root.join("src"))?;
    fs::create_dir_all(root.join("tests"))?;
    fs::write(
        root.join("Cargo.toml"),
        "[package]\nname='test-styles'\nversion='0.1.0'\nedition='2024'\n",
    )?;
    fs::write(
        root.join("src/lib.rs"),
        r#"
#[quickcheck]
fn parser_property(value: u8) -> bool { value == value }

#[tokio::test_helper]
fn parser_lookalike() {}
"#,
    )?;
    fs::write(
        root.join("tests/fallback.rs"),
        r#"
this is intentionally invalid rust

#[ quickcheck ]
fn fallback_property(value: u8) -> bool { value == value }

#[rstest_helper]
fn fallback_lookalike() {}
"#,
    )?;

    let files = vec![
        PathBuf::from("src/lib.rs"),
        PathBuf::from("tests/fallback.rs"),
    ];
    let index = super::super::build_index(&root, &files).map_err(std::io::Error::other)?;
    let expected_tests = ["parser_property", "fallback_property"];
    assert_eq!(index_test_names(&index), expected_tests);

    let loaded_files = files
        .iter()
        .map(|file| fs::read(root.join(file)).map(|bytes| (file.clone(), bytes)))
        .collect::<Result<Vec<_>, _>>()?;
    let cold_cached = super::super::build_index_from_loaded_files_with_cache_and_test_harnesses(
        &root,
        &loaded_files,
        &[],
    )
    .map_err(std::io::Error::other)?;
    let warm_cached = super::super::build_index_from_loaded_files_with_cache_and_test_harnesses(
        &root,
        &loaded_files,
        &[],
    )
    .map_err(std::io::Error::other)?;
    assert_eq!(index_test_names(&cold_cached.index), expected_tests);
    assert_eq!(index_test_names(&warm_cached.index), expected_tests);
    assert!(
        index
            .files
            .get(Path::new("tests/fallback.rs"))
            .is_some_and(|facts| facts.used_lexical_fallback),
        "invalid source must exercise the lexical fallback producer"
    );
    assert!(
        index
            .functions
            .iter()
            .find(|function| function.name == "parser_lookalike")
            .is_some_and(|function| !function.source_role.is_evidence_role())
    );
    assert!(
        index
            .functions
            .iter()
            .find(|function| function.name == "fallback_lookalike")
            .is_some_and(|function| !function.source_role.is_evidence_role())
    );
    Ok(())
}

/// Role-mapping table (#3531): each producer path lands in exactly one typed
/// `FunctionSourceRole`, pinned through the full `build_index` pipeline so a
/// drifted producer, normalizer, or promotion path cannot silently re-collapse
/// the roles into one bit.
#[test]
fn function_role_table_maps_every_producer_path_to_one_typed_role()
-> Result<(), Box<dyn std::error::Error>> {
    struct Cleanup(PathBuf);

    impl Drop for Cleanup {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    let stamp = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
    let root = std::env::temp_dir().join(format!("ripr-role-table-{stamp}"));
    let _cleanup = Cleanup(root.clone());
    fs::create_dir_all(root.join("src"))?;
    fs::create_dir_all(root.join("tests"))?;
    fs::write(
        root.join("Cargo.toml"),
        "[package]\nname='role-table'\nversion='0.1.0'\nedition='2024'\n",
    )?;
    fs::write(
        root.join("src/lib.rs"),
        r#"
pub fn ordinary_production() -> i32 { 1 }

#[test]
fn plain_attribute_test() { assert_eq!(ordinary_production(), 1); }

#[cfg(test)]
mod tests {
    fn cfg_test_helper() -> i32 { super::ordinary_production() }

    #[test]
    fn attribute_test_inside_cfg_test_module() {
        assert_eq!(super::ordinary_production(), 1);
    }
}

#[cfg(all(feature = "slow", test))]
mod test_second_gate {
    fn cfg_all_helper_test_second() -> i32 { super::ordinary_production() }
}

#[cfg(all(test, feature = "slow"))]
mod test_first_gate {
    fn cfg_all_helper_test_first() -> i32 { super::ordinary_production() }
}

#[cfg(any(test, feature = "slow"))]
mod any_gate {
    fn any_gate_helper() -> i32 { 2 }
}

#[cfg_attr(test, test)]
fn cfg_attr_lookalike() {}

#[quickcheck]
fn normalizer_credits_explicit_attribute(value: u8) -> bool { value == value }

#[test_case(1)]
fn promoted_parameterized_expansion(value: i32) {
    assert_eq!(value, 1);
}
"#,
    )?;
    // Invalid syntax forces the lexical fallback producer for this file.
    fs::write(
        root.join("tests/integration_fallback.rs"),
        r#"
this is intentionally invalid rust

#[test]
fn fallback_attribute_test() { assert!(true); }

fn fallback_unannotated_helper() -> i32 { 3 }
"#,
    )?;
    // Valid `tests/**` file: parsed by the parser producer.
    fs::write(
        root.join("tests/suite.rs"),
        r#"
fn helper_beside_test() -> i32 { 4 }

#[test]
fn integration_test() {
    assert_eq!(helper_beside_test(), 4);
}
"#,
    )?;

    let files = vec![
        PathBuf::from("src/lib.rs"),
        PathBuf::from("tests/integration_fallback.rs"),
        PathBuf::from("tests/suite.rs"),
    ];
    let index = super::super::build_index(&root, &files).map_err(std::io::Error::other)?;

    let role_of = |file: &str, name: &str| -> Result<FunctionSourceRole, String> {
        index
            .functions
            .iter()
            .find(|function| function.file == Path::new(file) && function.name == name)
            .map(|function| function.source_role)
            .ok_or_else(|| format!("missing function fact for {file}::{name}"))
    };

    // (file, function, expected role, producer that owns the classification)
    let table: &[(&str, &str, FunctionSourceRole, &str)] = &[
        (
            "src/lib.rs",
            "ordinary_production",
            FunctionSourceRole::Production,
            "parser: no attribute, no test-required module",
        ),
        (
            "src/lib.rs",
            "plain_attribute_test",
            FunctionSourceRole::TestAttribute,
            "parser: exact test-defining attribute",
        ),
        (
            "src/lib.rs",
            "cfg_test_helper",
            FunctionSourceRole::CfgTestModule,
            "parser: cfg-test module membership, no TestFact",
        ),
        (
            "src/lib.rs",
            "attribute_test_inside_cfg_test_module",
            FunctionSourceRole::TestAttribute,
            "parser: exact attribute wins over module membership",
        ),
        (
            "src/lib.rs",
            "cfg_all_helper_test_second",
            FunctionSourceRole::CfgTestModule,
            "parser: cfg(all(feature, test)) test-second conjunct",
        ),
        (
            "src/lib.rs",
            "cfg_all_helper_test_first",
            FunctionSourceRole::CfgTestModule,
            "parser: cfg(all(test, feature)) test-first conjunct",
        ),
        (
            "src/lib.rs",
            "any_gate_helper",
            FunctionSourceRole::Production,
            "parser: cfg(any(...)) never requires test; fail closed",
        ),
        (
            "src/lib.rs",
            "cfg_attr_lookalike",
            FunctionSourceRole::Production,
            "parser: cfg_attr introductions never promote",
        ),
        (
            "src/lib.rs",
            "normalizer_credits_explicit_attribute",
            FunctionSourceRole::TestAttribute,
            "normalizer: exact attribute vocabulary the parser table misses",
        ),
        (
            "src/lib.rs",
            "promoted_parameterized_expansion",
            FunctionSourceRole::ParameterizedExpansion,
            "promotion: explicit test_case attribute keeps its provenance",
        ),
        (
            "tests/integration_fallback.rs",
            "fallback_attribute_test",
            FunctionSourceRole::TestAttribute,
            "lexical fallback: exact attribute prefix (provenance on the file fact)",
        ),
        (
            "tests/integration_fallback.rs",
            "fallback_unannotated_helper",
            FunctionSourceRole::Production,
            "lexical fallback: no attribute seen",
        ),
        (
            "tests/suite.rs",
            "helper_beside_test",
            FunctionSourceRole::Production,
            "parser: tests/** helper role is Production; the file-level SourceRole \
             keeps it out of production subjects at consumers",
        ),
        (
            "tests/suite.rs",
            "integration_test",
            FunctionSourceRole::TestAttribute,
            "parser: integration test attribute",
        ),
    ];

    for (file, name, expected, producer) in table {
        let actual = role_of(file, name).map_err(std::io::Error::other)?;
        assert_eq!(
            actual, *expected,
            "role mismatch for {file}::{name} ({producer})"
        );
    }

    // Executable-test membership stays TestFact-driven: evidence-only roles
    // never register a TestFact.
    let test_names: Vec<&str> = index.tests.iter().map(|test| test.name.as_str()).collect();
    for evidence_only in [
        "cfg_test_helper",
        "cfg_all_helper_test_second",
        "cfg_all_helper_test_first",
    ] {
        assert!(
            !test_names.contains(&evidence_only),
            "{evidence_only} must stay evidence-only without an executable TestFact"
        );
    }
    for executable in [
        "plain_attribute_test",
        "attribute_test_inside_cfg_test_module",
        "promoted_parameterized_expansion",
        "fallback_attribute_test",
        "integration_test",
    ] {
        assert!(
            test_names.contains(&executable),
            "{executable} must register an executable TestFact"
        );
    }

    // The serialized spelling is part of the on-disk file-fact cache identity;
    // pin the snake_case names so silent renames cannot drift cached entries.
    assert_eq!(
        serde_json::to_value(FunctionSourceRole::CfgTestModule).map_err(std::io::Error::other)?,
        serde_json::json!("cfg_test_module")
    );
    assert_eq!(
        serde_json::to_value(FunctionSourceRole::ParameterizedExpansion)
            .map_err(std::io::Error::other)?,
        serde_json::json!("parameterized_expansion")
    );
    Ok(())
}
