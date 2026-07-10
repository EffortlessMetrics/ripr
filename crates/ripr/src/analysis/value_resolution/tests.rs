use super::*;
use crate::analysis::seams::{ExpectedSink, RepoSeam, SeamKind};

fn predicate_seam() -> RepoSeam {
    RepoSeam::new(
        "src/pricing.rs",
        "pricing::discounted_total",
        SeamKind::PredicateBoundary,
        0,
        1,
        "amount >= discount_threshold",
        RequiredDiscriminator::BoundaryValue {
            description: "amount >= discount_threshold".to_string(),
        },
        ExpectedSink::ReturnValue,
    )
}

fn invalidation_lines(
    invalidations: &BTreeMap<String, Vec<SourcePosition>>,
    ident: &str,
) -> Vec<usize> {
    invalidations
        .get(ident)
        .into_iter()
        .flatten()
        .map(|position| position.line)
        .collect()
}

#[test]
fn extract_let_bindings_picks_up_literal_rhs_and_skips_expressions() {
    let body = "let a = 100;\nlet b: i32 = 200;\nlet mut c = 300;\nlet d = a + 1;\n";
    let bindings = extract_let_bindings(body);
    assert_eq!(bindings.get("a").map(String::as_str), Some("100"));
    assert_eq!(bindings.get("b").map(String::as_str), Some("200"));
    assert_eq!(bindings.get("c").map(String::as_str), Some("300"));
    assert!(!bindings.contains_key("d"), "non-literal RHS must not bind");
}

#[test]
fn extract_struct_field_bindings_picks_up_literal_fields_only() -> Result<(), String> {
    let body = "let case = DiscountCase { amount: 100, threshold: 100, \
                    computed: make_amount() };\n";
    let (bindings, invalidations) = extract_struct_field_bindings(body, 1, &[]);
    let binding = bindings
        .get("case")
        .ok_or_else(|| "same-test struct literal should be indexed".to_string())?;
    let fields = &binding.fields;

    assert_eq!(fields.get("amount").map(String::as_str), Some("100"));
    assert_eq!(fields.get("threshold").map(String::as_str), Some("100"));
    assert!(
        !fields.contains_key("computed"),
        "non-literal struct fields must stay unresolved"
    );
    assert!(
        !invalidations.contains_key("case"),
        "literal-only struct binding should not be invalidated"
    );
    Ok(())
}

#[test]
fn extract_struct_field_bindings_records_shadow_or_mutation_lines() {
    let shadowed = "let case = DiscountCase { amount: 100 };\nlet case = make_discount_case();\n";
    let (bindings, invalidations) = extract_struct_field_bindings(shadowed, 1, &[]);
    assert!(
        bindings.contains_key("case"),
        "literal binding remains available for calls before the shadow"
    );
    assert_eq!(invalidation_lines(&invalidations, "case"), vec![2]);

    let mutated = "let case = DiscountCase { amount: 100 };\ncase.amount = make_amount();\n";
    let (bindings, invalidations) = extract_struct_field_bindings(mutated, 1, &[]);
    assert!(
        bindings.contains_key("case"),
        "literal binding remains available for calls before mutation"
    );
    assert_eq!(invalidation_lines(&invalidations, "case"), vec![2]);

    let mutable = "let mut case = DiscountCase { amount: 100 };\n";
    let (bindings, invalidations) = extract_struct_field_bindings(mutable, 1, &[]);
    assert!(
        !bindings.contains_key("case"),
        "mutable fixture bindings must stay unresolved"
    );
    assert_eq!(invalidation_lines(&invalidations, "case"), vec![1]);
}

#[test]
fn extract_struct_field_bindings_records_test_function_parameter_invalidations() {
    let body = "fn via_param(case: DiscountCase) { \
                        discounted_total(case.amount, case.threshold); \
                        let case = DiscountCase { amount: 100, threshold: 100 }; \
                    }\n";
    let param_names = extract_fn_param_names(body);
    let (_bindings, invalidations) = extract_struct_field_bindings(body, 1, &param_names);

    assert!(
        invalidation_lines(&invalidations, "case").contains(&1),
        "fixture parameter names must invalidate same-name projection resolution"
    );
}

#[test]
fn extract_struct_field_bindings_records_common_non_let_shadowing_lines() {
    for (body, ident) in [
        (
            "let case = DiscountCase { amount: 100 };\n\
                 for case in helper_cases() { discounted_total(case.amount, 100); }\n",
            "case",
        ),
        (
            "let q = Quote { amount: 100 };\n\
                 for q in helper_cases() { discounted_total(q.amount, 100); }\n",
            "q",
        ),
        (
            "let case = DiscountCase { amount: 100 };\n\
                 if let Some(case) = make_case() { discounted_total(case.amount, 100); }\n",
            "case",
        ),
        (
            "let case = DiscountCase { amount: 100 };\n\
                 cases.iter().for_each(|case| discounted_total(case.amount, 100));\n",
            "case",
        ),
        (
            "let case = DiscountCase { amount: 100 };\n\
                 match make_case() { Some(case) => discounted_total(case.amount, 100), _ => 0 };\n",
            "case",
        ),
    ] {
        let (bindings, invalidations) = extract_struct_field_bindings(body, 1, &[]);
        assert!(
            bindings.contains_key(ident),
            "literal binding remains available for calls before non-let shadowing: {body}"
        );
        assert!(
            invalidation_lines(&invalidations, ident)
                .iter()
                .any(|line| *line >= 2),
            "non-let shadowing binders must invalidate projection values at their line: {body}"
        );
    }
}

#[test]
fn extract_struct_field_bindings_records_non_simple_let_pattern_shadowing_lines() {
    for body in [
        "let case = DiscountCase { amount: 100 };\n\
             let Some(case) = make_case() else { return; };\n\
             discounted_total(case.amount, 100);\n",
        "let case = DiscountCase { amount: 100 };\n\
             let (case, _) = helper_case();\n\
             discounted_total(case.amount, 100);\n",
    ] {
        let (bindings, invalidations) = extract_struct_field_bindings(body, 1, &[]);
        assert!(
            bindings.contains_key("case"),
            "literal binding remains available for calls before let-pattern shadowing: {body}"
        );
        assert!(
            invalidation_lines(&invalidations, "case").contains(&2),
            "non-simple let pattern binders must invalidate projection values at their line: {body}"
        );
    }
}

#[test]
fn resolve_same_test_struct_field_projection() {
    let seam = predicate_seam();
    let (struct_field_bindings, struct_field_invalidations) = extract_struct_field_bindings(
        "let case = DiscountCase { amount: 100, discount_threshold: 100 };\n",
        1,
        &[],
    );
    let facts = ValueEnvFacts {
        struct_field_bindings,
        struct_field_invalidations,
        ..ValueEnvFacts::default()
    };
    let env = ValueEnv::new(&seam, &facts);

    assert_eq!(
        env.resolve("case.amount"),
        vec![("100".to_string(), ValueContext::FunctionArgument)]
    );
    assert!(
        env.resolve("make_case().amount").is_empty(),
        "helper-built fixture projections must remain opaque"
    );
}

#[test]
fn resolve_shared_borrowed_identifier_once() {
    let seam = predicate_seam();
    let facts = ValueEnvFacts {
        let_bindings: BTreeMap::from([("amount".to_string(), "100".to_string())]),
        ..ValueEnvFacts::default()
    };
    let env = ValueEnv::new(&seam, &facts);

    assert_eq!(
        env.resolve("&amount"),
        vec![("100".to_string(), ValueContext::FunctionArgument)]
    );
    assert!(
        env.resolve("&mut amount").is_empty(),
        "mutable borrows stay opaque until mutation ordering is tracked"
    );
    assert!(
        env.resolve("&make_amount()").is_empty(),
        "borrowed helper expressions must not invent activation values"
    );
}

#[test]
fn resolve_same_test_struct_field_projection_is_source_order_scoped() {
    let seam = predicate_seam();
    let body = "let case = DiscountCase { amount: 100, discount_threshold: 100 };\n\
                    discounted_total(case.amount, case.discount_threshold);\n\
                    let case = make_discount_case();\n";
    let (struct_field_bindings, struct_field_invalidations) =
        extract_struct_field_bindings(body, 10, &[]);
    let facts = ValueEnvFacts {
        struct_field_bindings,
        struct_field_invalidations,
        ..ValueEnvFacts::default()
    };
    let env = ValueEnv::new(&seam, &facts);

    assert_eq!(
        env.resolve_at("case.amount", 11),
        vec![("100".to_string(), ValueContext::FunctionArgument)],
        "later shadowing must not erase values for an earlier owner call"
    );
    assert!(
        env.resolve_at("case.amount", 12).is_empty(),
        "projection values must stay unresolved once the shadowing line is reached"
    );
}

#[test]
fn resolve_same_test_struct_field_projection_requires_value_visible_at_call() {
    let seam = predicate_seam();
    let body = "discounted_total(case.amount, case.discount_threshold);\n\
                    let case = DiscountCase { amount: 100, discount_threshold: 100 };\n";
    let (struct_field_bindings, struct_field_invalidations) =
        extract_struct_field_bindings(body, 10, &[]);
    let facts = ValueEnvFacts {
        struct_field_bindings,
        struct_field_invalidations,
        ..ValueEnvFacts::default()
    };
    let env = ValueEnv::new(&seam, &facts);

    assert!(
        env.resolve_at("case.amount", 10).is_empty(),
        "later literals must not explain owner-call projections before the binding"
    );
    assert_eq!(
        env.resolve_at("case.amount", 11),
        vec![("100".to_string(), ValueContext::FunctionArgument)],
        "the literal becomes available only once the binding line is reached"
    );
}

#[test]
fn resolve_same_test_struct_field_projection_is_column_scoped_on_same_line() {
    let seam = predicate_seam();
    let late_body = "discounted_total(case.amount, case.discount_threshold); \
                         let case = DiscountCase { amount: 100, discount_threshold: 100 };\n";
    let (struct_field_bindings, struct_field_invalidations) =
        extract_struct_field_bindings(late_body, 10, &[]);
    let facts = ValueEnvFacts {
        struct_field_bindings,
        struct_field_invalidations,
        ..ValueEnvFacts::default()
    };
    let env = ValueEnv::new(&seam, &facts);
    assert!(
        env.resolve_at_call("case.amount", 10, "discounted_total", late_body.trim())
            .is_empty(),
        "same-line literals after the owner call must not explain earlier field projections"
    );

    let visible_body = "let case = DiscountCase { amount: 100, discount_threshold: 100 }; \
                            discounted_total(case.amount, case.discount_threshold);\n";
    let (struct_field_bindings, struct_field_invalidations) =
        extract_struct_field_bindings(visible_body, 10, &[]);
    let facts = ValueEnvFacts {
        struct_field_bindings,
        struct_field_invalidations,
        ..ValueEnvFacts::default()
    };
    let env = ValueEnv::new(&seam, &facts);
    assert_eq!(
        env.resolve_at_call("case.amount", 10, "discounted_total", visible_body.trim()),
        vec![("100".to_string(), ValueContext::FunctionArgument)],
        "same-line literals before the owner call remain safe activation values"
    );
}

#[test]
fn resolve_same_test_struct_field_projection_is_mutation_scoped() {
    let seam = predicate_seam();
    let body = "let case = DiscountCase { amount: 100, discount_threshold: 100 };\n\
                    discounted_total(case.amount, case.discount_threshold);\n\
                    case.amount = make_amount();\n";
    let (struct_field_bindings, struct_field_invalidations) =
        extract_struct_field_bindings(body, 10, &[]);
    let facts = ValueEnvFacts {
        struct_field_bindings,
        struct_field_invalidations,
        ..ValueEnvFacts::default()
    };
    let env = ValueEnv::new(&seam, &facts);

    assert_eq!(
        env.resolve_at("case.amount", 11),
        vec![("100".to_string(), ValueContext::FunctionArgument)],
        "later mutation must not erase values for an earlier owner call"
    );
    assert!(
        env.resolve_at("case.amount", 12).is_empty(),
        "projection values must stay unresolved once the mutation line is reached"
    );
}

#[test]
fn resolve_at_ignores_empty_arguments() {
    let seam = predicate_seam();
    let facts = ValueEnvFacts::default();
    let env = ValueEnv::new(&seam, &facts);

    assert!(
        env.resolve_at("   ", 1).is_empty(),
        "empty argument text must not produce activation values"
    );
}

#[test]
fn resolve_at_unwraps_literal_path_constructors_only() {
    let seam = predicate_seam();
    let facts = ValueEnvFacts {
        bare_std_path_imported: true,
        bare_std_path_buf_imported: true,
        ..ValueEnvFacts::default()
    };
    let env = ValueEnv::new(&seam, &facts);

    assert_eq!(
        env.resolve_at(r#"Path::new("target/ripr/workflow")"#, 1),
        vec![(
            r#""target/ripr/workflow""#.to_string(),
            ValueContext::FunctionArgument
        )],
        "Path::new string literals should become concrete activation values"
    );
    assert_eq!(
        env.resolve_at(r#"PathBuf::from("target/ripr/workflow")"#, 1),
        vec![(
            r#""target/ripr/workflow""#.to_string(),
            ValueContext::FunctionArgument
        )],
        "imported PathBuf::from string literals should become concrete activation values"
    );
    assert_eq!(
        env.resolve_at(r#"std::path::Path::new(".")"#, 1),
        vec![(r#"".""#.to_string(), ValueContext::FunctionArgument)],
        "fully qualified Path::new literals should become concrete activation values"
    );
    assert_eq!(
        env.resolve_at(r#"::std::path::Path::new(".")"#, 1),
        vec![(r#"".""#.to_string(), ValueContext::FunctionArgument)],
        "root-qualified Path::new literals should become concrete activation values"
    );
    assert_eq!(
        env.resolve_at(r#"std::path::PathBuf::from(".")"#, 1),
        vec![(r#"".""#.to_string(), ValueContext::FunctionArgument)],
        "PathBuf::from string literals should become concrete activation values"
    );
    assert_eq!(
        env.resolve_at(r#"::std::path::PathBuf::from(".")"#, 1),
        vec![(r#"".""#.to_string(), ValueContext::FunctionArgument)],
        "root-qualified PathBuf::from literals should become concrete activation values"
    );
    assert!(
        env.resolve_at("Path::new(out_dir)", 1).is_empty(),
        "non-literal path constructors must remain unresolved"
    );
}

#[test]
fn resolve_at_requires_std_import_for_bare_path_constructors() {
    let seam = predicate_seam();
    let facts = ValueEnvFacts::default();
    let env = ValueEnv::new(&seam, &facts);

    assert!(
        env.resolve_at(r#"Path::new("target/ripr/workflow")"#, 1)
            .is_empty(),
        "bare Path::new must stay unresolved without same-file std import evidence"
    );
    assert!(
        env.resolve_at(r#"PathBuf::from("target/ripr/workflow")"#, 1)
            .is_empty(),
        "bare PathBuf::from must stay unresolved without same-file std import evidence"
    );
    assert_eq!(
        env.resolve_at(r#"std::path::Path::new("target/ripr/workflow")"#, 1),
        vec![(
            r#""target/ripr/workflow""#.to_string(),
            ValueContext::FunctionArgument
        )],
        "fully qualified std path constructors do not need bare import evidence"
    );
}

#[test]
fn extract_path_constructor_imports_excludes_same_file_shadows() {
    let imports = extract_path_constructor_imports(
        r#"
use std::path::{Path, PathBuf};
fn uses_path(path: &Path) {}
"#,
    );
    assert!(imports.path, "brace import should enable bare Path::new");
    assert!(
        imports.path_buf,
        "brace import should enable bare PathBuf::from"
    );

    let alias = extract_path_constructor_imports("use std::path::Path as StdPath;\n");
    assert!(
        !alias.path,
        "aliased std Path import must not enable bare Path::new"
    );

    let shadow = extract_path_constructor_imports(
        r#"
use std::path::Path;
struct Path;
"#,
    );
    assert!(
        !shadow.path,
        "same-file Path shadow must keep bare Path::new unresolved"
    );

    let visible_shadow = extract_path_constructor_imports(
        r#"
use std::path::PathBuf;
pub(super) struct PathBuf;
"#,
    );
    assert!(
        !visible_shadow.path_buf,
        "same-file visible PathBuf shadow must keep bare PathBuf::from unresolved"
    );
}

#[test]
fn extract_module_constants_finds_const_and_static_top_level() {
    let source = "pub const A: i32 = 1;\nstatic B: i32 = 2;\n\
                      pub(crate) const C: i32 = 3;\n";
    let consts = extract_module_constants(source);
    assert_eq!(consts.get("A").map(String::as_str), Some("1"));
    assert_eq!(consts.get("B").map(String::as_str), Some("2"));
    assert_eq!(consts.get("C").map(String::as_str), Some("3"));
}

#[test]
fn looks_like_literal_accepts_numbers_strings_bools_paths_and_rejects_others() {
    for ok in [
        "100",
        "-5",
        "1_000",
        "1.5",
        "\"hi\"",
        "true",
        "false",
        "None",
        "Color::Red",
        "MyError::ParseError",
    ] {
        assert!(looks_like_literal(ok), "{ok} should look like a literal");
    }
    for bad in ["amount", "make_quote()", "x + 1"] {
        assert!(
            !looks_like_literal(bad),
            "{bad} must not look like a literal"
        );
    }
}

#[test]
fn unwrap_option_or_result_peels_one_level_only() {
    assert_eq!(unwrap_option_or_result("Some(100)").as_deref(), Some("100"));
    assert_eq!(unwrap_option_or_result("Ok(42)").as_deref(), Some("42"));
    assert_eq!(
        unwrap_option_or_result("Err(MyError::A)").as_deref(),
        Some("MyError::A")
    );
    assert_eq!(unwrap_option_or_result("100"), None);
}

#[test]
fn resolve_option_result_constructor_keeps_unresolved_inner_opaque() {
    let seam = predicate_seam();
    let facts = ValueEnvFacts::default();
    let env = ValueEnv::new(&seam, &facts);
    assert!(
        env.resolve("Some(make_amount())").is_empty(),
        "opaque constructor payloads must not become observed values"
    );
}

#[test]
fn extract_rstest_cases_preserves_string_literal_whitespace() {
    let test = TestSummary {
        name: "t".to_string(),
        file: std::path::PathBuf::from("tests/x.rs"),
        start_line: 1,
        end_line: 1,
        body: "fn t(input: &str) { check(input); }".to_string(),
        calls: Vec::new(),
        assertions: Vec::new(),
        literals: Vec::new(),
        attrs: vec!["#[rstest]".to_string(), "#[case(\"a b\")]".to_string()],
    };
    let (cases, params) = extract_rstest_cases(&test);
    assert_eq!(params, vec!["input"]);
    assert_eq!(cases, vec![vec!["\"a b\"".to_string()]]);
}

#[test]
fn strip_comments_and_strings_removes_line_comments_and_string_contents() {
    let input = "let x = 1; // let x = 999;\nlet s = \"shadow = 0\";\n";
    let cleaned = strip_comments_and_strings(input);
    assert!(
        !cleaned.contains("999"),
        "comment-shadowed value must be stripped"
    );
    assert!(
        !cleaned.contains("shadow = 0"),
        "string-shadowed value must be stripped"
    );
}

#[test]
fn scan_for_table_loops_extracts_named_columns() {
    let body = "for (a, b, c) in [(1, 2, 3), (4, 5, 6)] { let _ = (a, b, c); }\n";
    let captures = scan_for_table_loops(body);
    assert_eq!(captures.len(), 1);
    let cap = &captures[0];
    assert_eq!(cap.idents.len(), 3);
    assert_eq!(cap.rows.len(), 2);
    assert_eq!(cap.rows[0], vec!["1", "2", "3"]);
    assert_eq!(cap.rows[1], vec!["4", "5", "6"]);
}

#[test]
fn scan_builder_calls_finds_method_chain_arguments() {
    let body = "let q = Quote::new().amount(100).threshold(200).build();\n";
    let calls = scan_builder_calls(body);
    let methods: Vec<&str> = calls.iter().map(|c| c.method.as_str()).collect();
    assert!(methods.contains(&"amount"));
    assert!(methods.contains(&"threshold"));
    assert!(methods.contains(&"build"));
}

#[test]
fn builder_method_match_accepts_fixture_override_prefixes_and_rejects_unrelated_methods() {
    let allowed: std::collections::BTreeSet<String> = ["amount", "threshold"]
        .into_iter()
        .map(str::to_string)
        .collect();
    assert!(builder_method_matches_allowed("amount", &allowed));
    assert!(builder_method_matches_allowed("with_amount", &allowed));
    assert!(builder_method_matches_allowed("set_threshold", &allowed));
    assert!(builder_method_matches_allowed("amount_cents", &allowed));
    assert!(!builder_method_matches_allowed("with_seed", &allowed));
    assert!(!builder_method_matches_allowed("discount", &allowed));
}

#[test]
fn allowed_builder_method_names_includes_required_discriminator_tokens() {
    // Build a minimal env; we only need the seam for this assertion.
    let seam = predicate_seam();
    let test = TestSummary {
        name: "t".to_string(),
        file: std::path::PathBuf::from("tests/x.rs"),
        start_line: 1,
        end_line: 1,
        body: String::new(),
        calls: Vec::new(),
        assertions: Vec::new(),
        literals: Vec::new(),
        attrs: Vec::new(),
    };
    let facts = ValueEnvFacts::default();
    let env = ValueEnv::new(&seam, &facts);
    // Suppress dead-code warnings by referencing the param.
    let _ = &test;
    let allowed = env.allowed_builder_method_names();
    assert!(allowed.contains("amount"));
    assert!(allowed.contains("discount_threshold"));
}
