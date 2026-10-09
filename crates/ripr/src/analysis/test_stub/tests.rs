use super::*;
use crate::analysis::new_test_target::NewTestProposalProvenance;
use crate::analysis::seams::ExpectedSink;

fn seam_at(
    file: &str,
    source: &str,
    needle: &str,
    kind: SeamKind,
    required: RequiredDiscriminator,
) -> Result<RepoSeam, String> {
    let offset = source
        .find(needle)
        .ok_or_else(|| format!("fixture has `{needle}`"))?;
    let line = source[..offset].matches('\n').count() + 1;
    Ok(RepoSeam::new(
        file,
        "owner",
        kind,
        offset,
        line,
        needle,
        required,
        ExpectedSink::ReturnValue,
    ))
}

fn boundary(description: &str) -> RequiredDiscriminator {
    RequiredDiscriminator::BoundaryValue {
        description: description.to_string(),
    }
}

const PRICE: &str = "pub fn price(amount: u32, threshold: u32) -> u32 {
    if amount >= threshold { amount - 10 } else { amount }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn price_works() {
        assert!(price(100, 50) > 0);
    }
}
";

#[test]
fn boundary_stub_inserts_into_the_existing_inline_module_and_stops_at_the_expected_value()
-> Result<(), String> {
    let seam = seam_at(
        "src/lib.rs",
        PRICE,
        "amount >= threshold",
        SeamKind::PredicateBoundary,
        boundary("amount == threshold"),
    )?;
    let stub = rust_test_stub(&seam, None, PRICE).map_err(|r| r.reason().to_string())?;
    let close = PRICE.rfind('}').ok_or("module close brace")?;
    assert_eq!(
        stub.placement,
        TestStubPlacement::ExistingInlineModule {
            file: PathBuf::from("src/lib.rs"),
            module_name: "tests".to_string(),
            offset: close,
        }
    );
    assert_eq!(stub.test_name, "price_boundary_discriminator");
    assert_eq!(
        stub.derived_inputs,
        vec!["amount = 100".to_string(), "threshold = 100".to_string()]
    );
    let expected = "
    // ripr: discriminate `amount >= threshold` in `price` (src/lib.rs:2).
    #[test]
    #[allow(unreachable_code)] // delete with the last todo!()
    fn price_boundary_discriminator() {
        use super::*;
        let amount: u32 = 100;
        let threshold: u32 = 100;
        let actual = price(amount, threshold);
        let expected: u32 = todo!(\"ripr: write the value `price` should return for amount = 100, threshold = 100\");
        assert_eq!(actual, expected);
    }
";
    assert_eq!(stub.text, expected);
    Ok(())
}

#[test]
fn method_owner_uses_receiver_syntax_not_a_free_call() -> Result<(), String> {
    // #5357: a method owner was suggested as `as_whole_units(...)`.
    let source = "pub struct ByteSize(u64);
impl ByteSize {
    pub fn as_whole_units(&self, unit: u64) -> u64 {
        if unit == 0 { 0 } else { self.0 / unit }
    }
}
";
    let seam = seam_at(
        "src/lib.rs",
        source,
        "unit == 0",
        SeamKind::PredicateBoundary,
        boundary("unit == 0"),
    )?;
    let stub = rust_test_stub(&seam, None, source).map_err(|r| r.reason().to_string())?;
    assert!(matches!(
        stub.placement,
        TestStubPlacement::NewInlineModule { offset, .. } if offset == source.len()
    ));
    assert!(stub.text.starts_with("\n#[cfg(test)]\nmod ripr_tests {\n"));
    assert!(stub.text.contains(
        "let subject: ByteSize = todo!(\"ripr: build the `ByteSize` that `as_whole_units` runs on\");"
    ));
    assert!(stub.text.contains("let unit: u64 = 0;"));
    assert!(
        stub.text
            .contains("let actual = subject.as_whole_units(unit);")
    );
    assert!(!stub.text.contains("= as_whole_units("));
    Ok(())
}

#[test]
fn literal_and_const_comparisons_fill_one_side_and_leave_the_rest_as_todo() -> Result<(), String> {
    let source = "const LIMIT: i32 = 7;
fn grade(score: i32, bonus: i32) -> i32 { if score > LIMIT { bonus } else { 0 } }
fn floor(x: i64, y: i64) -> i64 { if x <= -3 { y } else { x } }
";
    let seam = seam_at(
        "src/lib.rs",
        source,
        "score > LIMIT",
        SeamKind::PredicateBoundary,
        boundary("score == LIMIT"),
    )?;
    let stub = rust_test_stub(&seam, None, source).map_err(|r| r.reason().to_string())?;
    assert_eq!(stub.derived_inputs, vec!["score = LIMIT".to_string()]);
    assert!(
        stub.text
            .contains("let bonus: i32 = todo!(\"ripr: choose `bonus`\");")
    );

    let seam = seam_at(
        "src/lib.rs",
        source,
        "x <= -3",
        SeamKind::PredicateBoundary,
        boundary("x == -3"),
    )?;
    let stub = rust_test_stub(&seam, None, source).map_err(|r| r.reason().to_string())?;
    assert_eq!(stub.derived_inputs, vec!["x = -3".to_string()]);
    Ok(())
}

#[test]
fn error_variant_stub_matches_the_variant_without_an_expected_todo() -> Result<(), String> {
    let source = "pub enum ParseError { Empty, TooLong(usize) }
pub fn parse(input: &str) -> Result<u8, ParseError> {
    if input.len() > 3 { return Err(ParseError::TooLong(input.len())); }
    Ok(0)
}
";
    let seam = seam_at(
        "src/lib.rs",
        source,
        "Err(ParseError::TooLong(input.len()))",
        SeamKind::ErrorVariant,
        RequiredDiscriminator::ErrorVariant {
            variant: "ParseError::TooLong".to_string(),
        },
    )?;
    let stub = rust_test_stub(&seam, None, source).map_err(|r| r.reason().to_string())?;
    assert!(stub.text.contains("let input: &str = todo!("));
    assert!(stub.text.contains(
        "assert!(matches!(actual, Err(ParseError::TooLong { .. })), \"expected Err(ParseError::TooLong {{ .. }})\");"
    ));
    assert!(!stub.text.contains("let expected"));
    Ok(())
}

#[test]
fn error_seam_without_a_nameable_variant_asserts_the_whole_value() -> Result<(), String> {
    let source = "pub fn parse_summary(raw: &str) -> Result<u8, String> {
    try_parse(raw).map_err(|error| error.to_string())
}
";
    let seam = seam_at(
        "src/lib.rs",
        source,
        "try_parse(raw).map_err(|error| error.to_string())",
        SeamKind::ErrorVariant,
        RequiredDiscriminator::ErrorVariant {
            variant: "try_parse(raw).map_err(|error| error.to_string())".to_string(),
        },
    )?;
    let stub = rust_test_stub(&seam, None, source).map_err(|r| r.reason().to_string())?;
    assert!(!stub.text.contains("matches!"));
    assert!(
        stub.text
            .contains("let expected: Result<u8, String> = todo!(")
    );
    assert_eq!(
        variant_pattern("Err(Error::TooLong(n))", None, PathScope::ChildModule),
        Some("Error::TooLong { .. }".to_string())
    );
    assert_eq!(
        variant_pattern("ErrorKind::Empty", None, PathScope::ChildModule),
        Some("ErrorKind::Empty { .. }".to_string())
    );
    assert_eq!(
        variant_pattern("try_parse(raw)", None, PathScope::ChildModule),
        None
    );
    Ok(())
}

#[test]
fn integration_proposal_writes_a_new_file_through_the_crate_path() -> Result<(), String> {
    let seam = seam_at(
        "src/lib.rs",
        PRICE,
        "amount >= threshold",
        SeamKind::PredicateBoundary,
        boundary("amount == threshold"),
    )?;
    let proposal = NewTestTargetProposal {
        kind: NewTestKind::Integration,
        file: PathBuf::from("tests/price_tests.rs"),
        owner: "demo::price".to_string(),
        provenance: NewTestProposalProvenance::ProducerOwned,
    };
    let stub = rust_test_stub(&seam, Some(&proposal), PRICE).map_err(|r| r.reason().to_string())?;
    assert_eq!(
        stub.placement,
        TestStubPlacement::NewIntegrationFile {
            file: PathBuf::from("tests/price_tests.rs")
        }
    );
    assert!(stub.text.starts_with("// ripr: discriminate"));
    assert!(stub.text.contains("    use demo::*;\n"));
    Ok(())
}

#[test]
fn return_types_without_visible_partial_eq_leave_the_assertion_to_fill_in() -> Result<(), String> {
    let source = "pub struct Out(u8);
#[derive(Debug, Clone, PartialEq)]
pub struct Seen(u8);
pub fn seen(n: u8) -> Seen { if n > 4 { Seen(n) } else { Seen(0) } }
pub fn build(n: u8) -> Result<Out, String> { if n > 2 { Ok(Out(n)) } else { Err(String::new()) } }
pub fn count(n: u8) -> Option<Vec<u8>> { if n > 2 { Some(vec![n]) } else { None } }
";
    let seam = seam_at(
        "src/lib.rs",
        source,
        "n > 2 { Ok",
        SeamKind::PredicateBoundary,
        boundary(""),
    )?;
    let stub = rust_test_stub(&seam, None, source).map_err(|r| r.reason().to_string())?;
    assert!(stub.text.contains("let _ = &actual;"), "{}", stub.text);
    assert!(
        stub.text
            .contains("todo!(\"ripr: assert that `actual` is the value `build` should return"),
        "{}",
        stub.text
    );
    assert!(!stub.text.contains("assert_eq!"), "{}", stub.text);
    assert!(!stub.text.contains("format!"), "{}", stub.text);
    // A local type whose derives name PartialEq and Debug compares directly.
    let seam = seam_at(
        "src/lib.rs",
        source,
        "n > 4",
        SeamKind::PredicateBoundary,
        boundary(""),
    )?;
    let stub = rust_test_stub(&seam, None, source).map_err(|r| r.reason().to_string())?;
    assert!(
        stub.text.contains("let expected: Seen = todo!("),
        "{}",
        stub.text
    );
    assert!(
        stub.text.contains("assert_eq!(actual, expected);"),
        "{}",
        stub.text
    );
    let seam = seam_at(
        "src/lib.rs",
        source,
        "n > 2 { Some",
        SeamKind::PredicateBoundary,
        boundary(""),
    )?;
    let stub = rust_test_stub(&seam, None, source).map_err(|r| r.reason().to_string())?;
    assert!(stub.text.contains("assert_eq!(actual, expected);"));
    Ok(())
}

#[test]
fn existing_test_name_gets_a_numbered_suffix() -> Result<(), String> {
    let source = PRICE.replace("fn price_works()", "fn price_boundary_discriminator()");
    let seam = seam_at(
        "src/lib.rs",
        &source,
        "amount >= threshold",
        SeamKind::PredicateBoundary,
        boundary("amount == threshold"),
    )?;
    let stub = rust_test_stub(&seam, None, &source).map_err(|r| r.reason().to_string())?;
    assert_eq!(stub.test_name, "price_boundary_discriminator_2");
    Ok(())
}

#[test]
fn refusals_name_the_blocker() -> Result<(), String> {
    let cases: [(&str, &str, SeamKind, TestStubRefusal); 6] = [
        (
            "pub async fn f(x: u8) -> u8 { if x > 1 { 1 } else { 0 } }\n",
            "x > 1",
            SeamKind::PredicateBoundary,
            TestStubRefusal::OwnerAsync,
        ),
        (
            "pub fn f<T: Copy>(x: T, n: u8) -> u8 { if n > 1 { 1 } else { 0 } }\n",
            "n > 1",
            SeamKind::PredicateBoundary,
            TestStubRefusal::OwnerGeneric,
        ),
        (
            "pub fn f(x: u8) { if x > 1 { println!(\"hi\"); } }\n",
            "x > 1",
            SeamKind::PredicateBoundary,
            TestStubRefusal::NoReturnValue,
        ),
        (
            "pub fn f(x: u8) -> u8 { if x > 1 { 1 } else { 0 } }\n#[cfg(test)]\nmod tests;\n",
            "x > 1",
            SeamKind::PredicateBoundary,
            TestStubRefusal::OutOfLineTestModule,
        ),
        (
            "mod inner { pub fn f(x: u8) -> u8 { if x > 1 { 1 } else { 0 } } }\n",
            "x > 1",
            SeamKind::PredicateBoundary,
            TestStubRefusal::OwnerInNestedModule,
        ),
        (
            "pub fn f(x: u8) -> u8 { log(x); x }\n",
            "log(x)",
            SeamKind::SideEffect,
            TestStubRefusal::ObserverRequired,
        ),
    ];
    for (source, needle, kind, refusal) in cases {
        let seam = seam_at("src/lib.rs", source, needle, kind, boundary(""))?;
        assert_eq!(
            rust_test_stub(&seam, None, source),
            Err(refusal),
            "{source}"
        );
    }
    let seam = seam_at(
        "src/app.py",
        "def f(x):\n    return x > 1\n",
        "x > 1",
        SeamKind::PredicateBoundary,
        boundary(""),
    )?;
    assert_eq!(
        rust_test_stub(&seam, None, "def f(x):\n    return x > 1\n"),
        Err(TestStubRefusal::NotRustSource)
    );
    Ok(())
}

#[test]
fn stub_text_never_uses_overclaiming_vocabulary() -> Result<(), String> {
    let seam = seam_at(
        "src/lib.rs",
        PRICE,
        "amount >= threshold",
        SeamKind::PredicateBoundary,
        boundary("amount == threshold"),
    )?;
    let stub = rust_test_stub(&seam, None, PRICE).map_err(|r| r.reason().to_string())?;
    for word in [
        "killed",   // ripr-allow: static-language: test guard verifying this term stays absent
        "survived", // ripr-allow: static-language: test guard verifying this term stays absent
        "untested", // ripr-allow: static-language: test guard verifying this term stays absent
        "proven",   // ripr-allow: static-language: test guard verifying this term stays absent
        "adequate", // ripr-allow: static-language: test guard verifying this term stays absent
    ] {
        assert!(!stub.text.contains(word), "{word}");
    }
    Ok(())
}

#[test]
fn lifetime_annotated_mut_reference_binds_the_referent_mutably() -> Result<(), String> {
    const SOURCE: &str = "pub fn bump<'a>(counter: &'a mut u32, limit: u32) -> bool {
    *counter >= limit
}
";
    let seam = seam_at(
        "src/lib.rs",
        SOURCE,
        "*counter >= limit",
        SeamKind::PredicateBoundary,
        boundary("*counter == limit"),
    )?;
    let stub = rust_test_stub(&seam, None, SOURCE).map_err(|r| r.reason().to_string())?;
    assert!(
        stub.text.contains("let mut counter: u32 ="),
        "{}",
        stub.text
    );
    assert!(
        stub.text.contains("bump(&mut counter, limit)"),
        "{}",
        stub.text
    );
    assert_eq!(strip_mut_reference("&mut Vec<u8>"), Some("Vec<u8>"));
    assert_eq!(strip_mut_reference("&'_ mut u32"), Some("u32"));
    assert_eq!(strip_mut_reference("&'static mut u32"), Some("u32"));
    assert_eq!(strip_mut_reference("&'_ u32"), None);
    assert_eq!(strip_mut_reference("&mutable"), None);
    Ok(())
}

#[test]
fn integration_stub_leaves_a_named_constant_as_a_fill_in() -> Result<(), String> {
    let proposal = NewTestTargetProposal {
        kind: NewTestKind::Integration,
        file: PathBuf::from("tests/gate.rs"),
        owner: "demo::gate".to_string(),
        provenance: NewTestProposalProvenance::ProducerOwned,
    };
    for source in [
        "const LIMIT: u8 = 3;
pub fn gate(n: u8) -> u8 {
    if n > LIMIT { n } else { 0 }
}
",
        "pub(crate) const LIMIT: u8 = 3;
pub fn gate(n: u8) -> u8 {
    if n > LIMIT { n } else { 0 }
}
",
    ] {
        let seam = seam_at(
            "src/lib.rs",
            source,
            "n > LIMIT",
            SeamKind::PredicateBoundary,
            boundary("n == LIMIT"),
        )?;
        // Inline, the private constant is in scope through `use super::*`.
        let inline = rust_test_stub(&seam, None, source).map_err(|r| r.reason().to_string())?;
        assert!(
            inline.text.contains("let n: u8 = LIMIT;"),
            "{}",
            inline.text
        );
        // A `tests/` file cannot see private or `pub(crate)` constants.
        let stub =
            rust_test_stub(&seam, Some(&proposal), source).map_err(|r| r.reason().to_string())?;
        assert!(!stub.text.contains("= LIMIT;"), "{}", stub.text);
        assert!(stub.text.contains("let n: u8 = todo!("), "{}", stub.text);
        assert!(stub.derived_inputs.is_empty());
    }
    Ok(())
}

#[test]
fn integration_stub_leaves_a_nested_module_public_constant_as_a_fill_in() -> Result<(), String> {
    const SOURCE: &str = "mod limits {
    pub const LIMIT: u8 = 3;
}
use limits::LIMIT;
pub fn gate(n: u8) -> u8 {
    if n > LIMIT { n } else { 0 }
}
";
    let seam = seam_at(
        "src/lib.rs",
        SOURCE,
        "n > LIMIT",
        SeamKind::PredicateBoundary,
        boundary("n == LIMIT"),
    )?;
    let proposal = NewTestTargetProposal {
        kind: NewTestKind::Integration,
        file: PathBuf::from("tests/gate.rs"),
        owner: "demo::gate".to_string(),
        provenance: NewTestProposalProvenance::ProducerOwned,
    };
    let inline = rust_test_stub(&seam, None, SOURCE).map_err(|r| r.reason().to_string())?;
    assert!(
        inline.text.contains("let n: u8 = LIMIT;"),
        "{}",
        inline.text
    );
    let stub =
        rust_test_stub(&seam, Some(&proposal), SOURCE).map_err(|r| r.reason().to_string())?;
    assert!(!stub.text.contains("= LIMIT;"), "{}", stub.text);
    assert!(stub.text.contains("let n: u8 = todo!("), "{}", stub.text);
    assert!(stub.derived_inputs.is_empty());
    Ok(())
}

#[test]
fn integration_stub_ignores_nested_const_after_a_brace_in_a_string() -> Result<(), String> {
    const SOURCE: &str = "mod limits {
    const PRELUDE: &str = \"}\";
    pub const LIMIT: u8 = 3;
}
use limits::LIMIT;
pub fn gate(n: u8) -> u8 {
    if n > LIMIT { n } else { 0 }
}
";
    let seam = seam_at(
        "src/lib.rs",
        SOURCE,
        "n > LIMIT",
        SeamKind::PredicateBoundary,
        boundary("n == LIMIT"),
    )?;
    let proposal = NewTestTargetProposal {
        kind: NewTestKind::Integration,
        file: PathBuf::from("tests/gate.rs"),
        owner: "demo::gate".to_string(),
        provenance: NewTestProposalProvenance::ProducerOwned,
    };
    let stub =
        rust_test_stub(&seam, Some(&proposal), SOURCE).map_err(|r| r.reason().to_string())?;
    assert!(!stub.text.contains("= LIMIT;"), "{}", stub.text);
    assert!(stub.text.contains("let n: u8 = todo!("), "{}", stub.text);
    assert!(stub.derived_inputs.is_empty());
    Ok(())
}

#[test]
fn integration_stub_leaves_a_cfg_test_public_constant_as_a_fill_in() -> Result<(), String> {
    const SOURCE: &str = "#[cfg(test)]
pub const LIMIT: u8 = 3;
pub fn gate(n: u8) -> u8 {
    #[cfg(test)]
    if n > LIMIT {
        return n;
    }
    n
}
";
    let seam = seam_at(
        "src/lib.rs",
        SOURCE,
        "n > LIMIT",
        SeamKind::PredicateBoundary,
        boundary("n == LIMIT"),
    )?;
    let proposal = NewTestTargetProposal {
        kind: NewTestKind::Integration,
        file: PathBuf::from("tests/gate.rs"),
        owner: "demo::gate".to_string(),
        provenance: NewTestProposalProvenance::ProducerOwned,
    };
    let inline = rust_test_stub(&seam, None, SOURCE).map_err(|r| r.reason().to_string())?;
    assert!(
        inline.text.contains("let n: u8 = LIMIT;"),
        "{}",
        inline.text
    );
    let stub =
        rust_test_stub(&seam, Some(&proposal), SOURCE).map_err(|r| r.reason().to_string())?;
    assert!(!stub.text.contains("= LIMIT;"), "{}", stub.text);
    assert!(stub.text.contains("let n: u8 = todo!("), "{}", stub.text);
    assert!(stub.derived_inputs.is_empty());
    Ok(())
}

#[test]
fn integration_stub_leaves_a_feature_gated_public_constant_as_a_fill_in() -> Result<(), String> {
    const SOURCE: &str = "#[cfg(feature = \"special\")]
pub const LIMIT: u8 = 3;
pub fn gate(n: u8) -> u8 {
    #[cfg(feature = \"special\")]
    if n > LIMIT {
        return n;
    }
    n
}
";
    let seam = seam_at(
        "src/lib.rs",
        SOURCE,
        "n > LIMIT",
        SeamKind::PredicateBoundary,
        boundary("n == LIMIT"),
    )?;
    let proposal = NewTestTargetProposal {
        kind: NewTestKind::Integration,
        file: PathBuf::from("tests/gate.rs"),
        owner: "demo::gate".to_string(),
        provenance: NewTestProposalProvenance::ProducerOwned,
    };
    let stub =
        rust_test_stub(&seam, Some(&proposal), SOURCE).map_err(|r| r.reason().to_string())?;
    assert!(!stub.text.contains("= LIMIT;"), "{}", stub.text);
    assert!(stub.text.contains("let n: u8 = todo!("), "{}", stub.text);
    assert!(stub.derived_inputs.is_empty());
    Ok(())
}

#[test]
fn integration_stub_rebases_crate_paths_and_keeps_a_public_constant() -> Result<(), String> {
    const SOURCE: &str = "pub const LIMIT: u8 = 3;
pub struct Tag;
pub fn gate(n: u8, tag: crate::Tag) -> u8 {
    let _ = tag;
    if n > LIMIT { n } else { 0 }
}
";
    let seam = seam_at(
        "src/lib.rs",
        SOURCE,
        "n > LIMIT",
        SeamKind::PredicateBoundary,
        boundary("n == LIMIT"),
    )?;
    let proposal = NewTestTargetProposal {
        kind: NewTestKind::Integration,
        file: PathBuf::from("tests/gate.rs"),
        owner: "demo::gate".to_string(),
        provenance: NewTestProposalProvenance::ProducerOwned,
    };
    let stub =
        rust_test_stub(&seam, Some(&proposal), SOURCE).map_err(|r| r.reason().to_string())?;
    assert_eq!(
        stub.placement,
        TestStubPlacement::NewIntegrationFile {
            file: PathBuf::from("tests/gate.rs")
        }
    );
    assert_eq!(stub.derived_inputs, vec!["n = LIMIT".to_string()]);
    assert!(stub.text.contains("use demo::*;\n"), "{}", stub.text);
    assert!(stub.text.contains("let n: u8 = LIMIT;"), "{}", stub.text);
    assert!(
        stub.text.contains("let tag: demo::Tag = todo!("),
        "{}",
        stub.text
    );
    assert!(
        !stub.text.contains("crate::"),
        "integration stubs must rebase crate:: to the crate name: {}",
        stub.text
    );
    Ok(())
}

#[test]
fn integration_stub_refuses_self_and_super_parameter_paths() -> Result<(), String> {
    let proposal = |owner: &str| NewTestTargetProposal {
        kind: NewTestKind::Integration,
        file: PathBuf::from("tests/flag.rs"),
        owner: owner.to_string(),
        provenance: NewTestProposalProvenance::ProducerOwned,
    };
    let self_source = "pub struct Cfg { pub on: bool }
pub fn flag(cfg: &self::Cfg, n: i32) -> bool {
    if n < 0 { cfg.on } else { !cfg.on }
}
";
    let super_source = "pub struct Cfg { pub on: bool }
pub fn flag(cfg: &super::Cfg, n: i32) -> bool {
    if n < 0 { cfg.on } else { !cfg.on }
}
";
    for (source, needle) in [(self_source, "n < 0"), (super_source, "n < 0")] {
        let seam = seam_at(
            "src/lib.rs",
            source,
            needle,
            SeamKind::PredicateBoundary,
            boundary(""),
        )?;
        assert_eq!(
            rust_test_stub(&seam, Some(&proposal("demo::flag")), source),
            Err(TestStubRefusal::ParameterUnsupported),
            "{source}"
        );
    }
    Ok(())
}

#[test]
fn result_aliases_and_qualified_or_shadowed_types_are_not_assumed_comparable() {
    let plain = "pub fn f() {}";
    assert_eq!(
        value_traits("Result<u8, String>", plain),
        ValueTraits::PartialEqAndDebug
    );
    assert_eq!(
        value_traits("std::io::Result<u8>", plain),
        ValueTraits::Unknown
    );
    assert_eq!(value_traits("Result<u8>", plain), ValueTraits::Unknown);
    assert_eq!(
        value_traits("Result<Vec<(u8, u8)>, String>", plain),
        ValueTraits::PartialEqAndDebug
    );
    let alias = "pub type Result<T> = std::result::Result<T, Bad>;
#[derive(Debug)]
pub struct Bad;";
    assert_eq!(value_traits("Result<u8, Bad>", alias), ValueTraits::Unknown);
    let local_error = "#[derive(Debug, PartialEq)]
pub enum Error { A }";
    assert_eq!(
        value_traits("Error", local_error),
        ValueTraits::PartialEqAndDebug
    );
    assert_eq!(
        value_traits("Result<u8, std::io::Error>", local_error),
        ValueTraits::Unknown
    );
    let shadow = "pub struct Duration(pub u64);";
    assert_eq!(value_traits("Duration", shadow), ValueTraits::Unknown);
    let test_only = "pub fn f() {}
#[cfg(test)]
mod tests {
    #[derive(Debug, PartialEq)]
    pub struct Out;
}";
    assert_eq!(value_traits("Out", test_only), ValueTraits::Unknown);
}

/// The inline module a boundary stub for `needle` is inserted into.
fn stub_module_name(source: &str, needle: &str) -> Result<String, String> {
    let seam = seam_at(
        "src/lib.rs",
        source,
        needle,
        SeamKind::PredicateBoundary,
        boundary(""),
    )?;
    match rust_test_stub(&seam, None, source).map_err(|r| r.as_str().to_string())? {
        RustTestStub {
            placement: TestStubPlacement::ExistingInlineModule { module_name, .. },
            ..
        } => Ok(module_name),
        other => Err(format!("unexpected placement {:?}", other.placement)),
    }
}

#[test]
fn several_inline_test_modules_pick_the_one_naming_the_owner_then_the_nearest() -> Result<(), String>
{
    // #5471: bytesize keeps two `#[cfg(test)]` modules in `lib.rs`.
    let source = "pub fn early(x: u8) -> u8 { if x > 1 { 1 } else { 0 } }

#[cfg(test)]
mod a {
    #[test]
    fn smoke() {}
}

pub fn late(x: u8) -> u8 { if x > 2 { 1 } else { 0 } }

#[cfg(test)]
mod b {
    use super::*;
    #[test]
    fn early_works() { assert_eq!(early(0), 0); }
}
";
    let module_name = stub_module_name;
    // `b` already names `early`, so it wins over the nearer `a`.
    assert_eq!(module_name(source, "x > 1")?, "b");
    // Nothing names `late`: the nearest module after it.
    assert_eq!(module_name(source, "x > 2")?, "b");
    // Nothing names `tail` and nothing follows it: the nearest before it.
    let with_tail =
        format!("{source}pub fn tail(x: u8) -> u8 {{ if x > 3 {{ 1 }} else {{ 0 }} }}\n");
    assert_eq!(module_name(&with_tail, "x > 3")?, "b");
    // Nothing names `head`: the nearest after it is `a`, not `b`.
    let with_head =
        format!("pub fn head(x: u8) -> u8 {{ if x > 4 {{ 1 }} else {{ 0 }} }}\n{source}");
    assert_eq!(module_name(&with_head, "x > 4")?, "a");
    Ok(())
}

#[test]
fn comments_and_strings_do_not_count_as_naming_the_owner() -> Result<(), String> {
    // Codex review of #5477: a nearer module mentioning `early` only in a
    // comment or string must not outrank the one that calls it.
    let source = "pub fn early(x: u8) -> u8 { if x > 1 { 1 } else { 0 } }

#[cfg(test)]
mod a {
    // early is covered in b
    #[test]
    fn s() { let _ = \"early\"; }
}

#[cfg(test)]
mod b {
    use super::*;
    #[test]
    fn early_works() { assert_eq!(early(0), 0); }
}
";
    assert_eq!(stub_module_name(source, "x > 1")?, "b");
    // `early_works` is a different identifier, not a mention of `early`.
    let renamed = source.replace("assert_eq!(early(0), 0)", "assert!(true)");
    assert_eq!(stub_module_name(&renamed, "x > 1")?, "a");
    Ok(())
}

#[test]
fn lifetime_only_impl_binds_the_subject_with_an_elided_lifetime() -> Result<(), String> {
    // #5471: humantime `impl<'a> Parser<'a> { fn parse_unit(&mut self, ..) }`.
    for header in ["impl<'a> Parser<'a> {", "impl Parser<'_> {"] {
        let source = format!(
            "pub struct Parser<'a> {{ src: &'a str }}
{header}
    pub fn parse_unit(&mut self, start: usize, end: usize) -> Result<u64, String> {{
        if end > start {{ Ok(1) }} else {{ Err(String::new()) }}
    }}
    pub fn open(src: &str, limit: usize) -> usize {{
        if src.len() > limit {{ limit }} else {{ 0 }}
    }}
}}
"
        );
        let seam = seam_at(
            "src/lib.rs",
            &source,
            "end > start",
            SeamKind::PredicateBoundary,
            boundary("end == start"),
        )?;
        let stub = rust_test_stub(&seam, None, &source).map_err(|r| r.reason().to_string())?;
        assert!(
            stub.text.contains(
                "let mut subject: Parser<'_> = todo!(\"ripr: build the `Parser<'_>` that `parse_unit` runs on\");"
            ),
            "{header}: {}",
            stub.text
        );
        assert!(
            stub.text
                .contains("let actual = subject.parse_unit(start, end);"),
            "{header}: {}",
            stub.text
        );
        assert!(!stub.text.contains("'a"), "{header}: {}", stub.text);
        // An associated function is called through the bare type path.
        let seam = seam_at(
            "src/lib.rs",
            &source,
            "src.len() > limit",
            SeamKind::PredicateBoundary,
            boundary(""),
        )?;
        let stub = rust_test_stub(&seam, None, &source).map_err(|r| r.reason().to_string())?;
        assert!(
            stub.text.contains("let actual = Parser::open(src, limit);"),
            "{header}: {}",
            stub.text
        );
    }
    Ok(())
}

#[test]
fn type_or_const_generic_impls_are_refused_as_generic_impls() -> Result<(), String> {
    for source in [
        "pub struct W<T>(T);\nimpl<T> W<T> {\n    pub fn f(&self, n: u8) -> u8 { if n > 1 { 1 } else { 0 } }\n}\n",
        "pub struct W<T>(T);\nimpl W<u8> {\n    pub fn f(&self, n: u8) -> u8 { if n > 1 { 1 } else { 0 } }\n}\n",
        "pub struct A<const N: usize>;\nimpl<const N: usize> A<N> {\n    pub fn f(&self, n: u8) -> u8 { if n > 1 { 1 } else { 0 } }\n}\n",
    ] {
        let seam = seam_at(
            "src/lib.rs",
            source,
            "n > 1",
            SeamKind::PredicateBoundary,
            boundary(""),
        )?;
        assert_eq!(
            rust_test_stub(&seam, None, source),
            Err(TestStubRefusal::OwnerGenericImpl),
            "{source}"
        );
    }
    assert_eq!(
        TestStubRefusal::OwnerGenericImpl.as_str(),
        "owner_generic_impl"
    );
    assert!(
        TestStubRefusal::OwnerGenericImpl
            .reason()
            .contains("type or const generics")
    );
    Ok(())
}

const FROM_STR: &str = "use std::str::FromStr;
#[derive(Debug, PartialEq)]
pub enum Unit { Second, Minute }
#[derive(Debug, PartialEq)]
pub enum Error { Unknown }
impl FromStr for Unit {
    type Err = Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            \"s\" => Ok(Unit::Second),
            \"m\" => Ok(Unit::Minute),
            _ => Err(Error::Unknown),
        }
    }
}
";

#[test]
fn trait_impl_method_is_called_through_its_trait_path() -> Result<(), String> {
    // #5471: humantime `impl FromStr for Unit { fn from_str(..) }`.
    let seam = seam_at(
        "src/lib.rs",
        FROM_STR,
        "\"m\" => Ok(Unit::Minute)",
        SeamKind::MatchArm,
        RequiredDiscriminator::MatchArmTaken {
            arm: "\"m\" => Ok(Unit::Minute)".to_string(),
        },
    )?;
    let stub = rust_test_stub(&seam, None, FROM_STR).map_err(|r| r.reason().to_string())?;
    assert!(
        stub.text
            .contains("let actual = <Unit as FromStr>::from_str(s);"),
        "{}",
        stub.text
    );
    // The return type names `Self::Err`, so no comparison is assumed: the
    // assertion stays the developer's fill-in.
    assert!(stub.text.contains("let _ = &actual;"), "{}", stub.text);
    assert!(!stub.text.contains("Unit::Err"), "{}", stub.text);
    assert!(!stub.text.contains("let expected"), "{}", stub.text);

    let seam = seam_at(
        "src/lib.rs",
        FROM_STR,
        "Err(Error::Unknown)",
        SeamKind::ErrorVariant,
        RequiredDiscriminator::ErrorVariant {
            variant: "Error::Unknown".to_string(),
        },
    )?;
    let stub = rust_test_stub(&seam, None, FROM_STR).map_err(|r| r.reason().to_string())?;
    assert!(
        stub.text
            .contains("assert!(matches!(actual, Err(Error::Unknown { .. }))"),
        "{}",
        stub.text
    );
    assert_eq!(
        concrete_type("Result<Self, Self::Err>", Some("Unit"), Some("FromStr")),
        "Result<Unit, <Unit as FromStr>::Err>"
    );
    assert_eq!(
        variant_pattern("Self::Err::Bad", Some("Unit"), PathScope::ChildModule),
        None
    );
    Ok(())
}

#[test]
fn trait_impl_method_with_a_receiver_passes_it_by_reference() -> Result<(), String> {
    let source = "pub struct Meter(u32);
impl std::fmt::Display for Meter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.0 > 3 { write!(f, \"big\") } else { write!(f, \"small\") }
    }
}
";
    let seam = seam_at(
        "src/lib.rs",
        source,
        "self.0 > 3",
        SeamKind::PredicateBoundary,
        boundary(""),
    )?;
    let stub = rust_test_stub(&seam, None, source).map_err(|r| r.reason().to_string())?;
    assert!(
        stub.text
            .contains("let actual = <Meter as std::fmt::Display>::fmt(&subject, &mut f);"),
        "{}",
        stub.text
    );
    assert!(
        stub.text
            .contains("let mut f: std::fmt::Formatter<'_> = todo!("),
        "{}",
        stub.text
    );
    Ok(())
}

const VERSION: &str = "#[derive(Debug, Clone, PartialEq)]
pub struct Version { pub major: u64, pub minor: u64, pub patch: u64 }
impl Version {
    pub fn next_minor(&self) -> Version {
        Version { major: self.major, minor: self.minor + 1, patch: 0 }
    }
    pub fn staged(&self) -> u64 {
        let v = Version { major: 1, minor: self.minor * 2, patch: 3 };
        v.minor
    }
}
";

#[test]
fn field_of_the_returned_struct_literal_asserts_the_whole_return_value() -> Result<(), String> {
    // #5471: semver `Version::next_minor` builds and returns a `Version`.
    let field = |needle: &str| RequiredDiscriminator::FieldValue {
        field: needle.to_string(),
    };
    let seam = seam_at(
        "src/lib.rs",
        VERSION,
        "minor: self.minor + 1",
        SeamKind::FieldConstruction,
        field("minor: self.minor + 1"),
    )?;
    let stub = rust_test_stub(&seam, None, VERSION).map_err(|r| r.reason().to_string())?;
    assert_eq!(stub.test_name, "next_minor_field_discriminator");
    assert!(
        stub.text.contains("let actual = subject.next_minor();"),
        "{}",
        stub.text
    );
    assert!(
        stub.text.contains(
            "let expected: Version = todo!(\"ripr: write the value `next_minor` should return, including its field `minor: self.minor + 1`\");"
        ),
        "{}",
        stub.text
    );
    // A literal bound to a local is not the returned value: refused, with a
    // reason naming that.
    let seam = seam_at(
        "src/lib.rs",
        VERSION,
        "minor: self.minor * 2",
        SeamKind::FieldConstruction,
        field("minor: self.minor * 2"),
    )?;
    assert_eq!(
        rust_test_stub(&seam, None, VERSION),
        Err(TestStubRefusal::FieldNotReturned)
    );
    assert!(
        TestStubRefusal::FieldNotReturned
            .reason()
            .contains("does not return directly")
    );
    Ok(())
}

#[test]
fn feature_gated_test_modules_are_never_chosen() -> Result<(), String> {
    // Review of #5477: `cargo test` does not build a feature-gated module, so
    // a stub there would never compile or run.
    let gated_first = "pub fn early(x: u32) -> u32 { if x > 40 { 1 } else { 0 } }

#[cfg(all(test, feature = \"slow\"))]
mod slow_tests {
    #[test]
    fn s() {}
}

#[cfg(test)]
mod tests {
    #[test]
    fn smoke() {}
}
";
    let seam = seam_at(
        "src/lib.rs",
        gated_first,
        "x > 40",
        SeamKind::PredicateBoundary,
        boundary(""),
    )?;
    match rust_test_stub(&seam, None, gated_first).map_err(|r| r.as_str().to_string())? {
        RustTestStub {
            placement: TestStubPlacement::ExistingInlineModule { module_name, .. },
            ..
        } => assert_eq!(module_name, "tests"),
        other => return Err(format!("unexpected placement {:?}", other.placement)),
    }
    for only_gated in [
        "pub fn early(x: u32) -> u32 { if x > 40 { 1 } else { 0 } }

#[cfg(all(test, feature = \"slow\"))]
mod slow_tests {
    #[test]
    fn s() {}
}
",
        "pub fn early(x: u32) -> u32 { if x > 40 { 1 } else { 0 } }

#[cfg(test)]
mod slow_tests {
    #![cfg(feature = \"slow\")]
    #[test]
    fn s() {}
}
",
    ] {
        let seam = seam_at(
            "src/lib.rs",
            only_gated,
            "x > 40",
            SeamKind::PredicateBoundary,
            boundary(""),
        )?;
        assert_eq!(
            rust_test_stub(&seam, None, only_gated).map(|stub| stub.test_name),
            Err(TestStubRefusal::AmbiguousTestModule),
            "{only_gated}"
        );
    }
    Ok(())
}

#[test]
fn impls_local_to_a_block_are_refused() -> Result<(), String> {
    // Review of #5477: names inside a fn body or `const _` block are out of
    // the test module's reach.
    for source in [
        "pub struct X(pub u8);
const _: () = {
    use std::fmt::Display;
    impl Display for X {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            if self.0 > 31 { write!(f, \"a\") } else { write!(f, \"b\") }
        }
    }
};
",
        "pub fn outer() {
    struct L(u8);
    impl<'a> L {
        fn m2(&self, n: u8) -> u8 { if n > 31 { 1 } else { 0 } }
    }
}
",
    ] {
        let needle = if source.contains("self.0 > 31") {
            "self.0 > 31"
        } else {
            "n > 31"
        };
        let seam = seam_at(
            "src/lib.rs",
            source,
            needle,
            SeamKind::PredicateBoundary,
            boundary(""),
        )?;
        assert_eq!(
            rust_test_stub(&seam, None, source).map(|stub| stub.test_name),
            Err(TestStubRefusal::OwnerUnsupported),
            "{source}"
        );
    }
    Ok(())
}

#[test]
fn a_proptest_sibling_module_does_not_outrank_one_naming_the_owner() -> Result<(), String> {
    // A macro-only `proptest!` module is nearer, but the module that already
    // calls the owner wins.
    let source = "pub fn early(x: u32) -> u32 { if x > 40 { 1 } else { 0 } }

#[cfg(test)]
mod props {
    proptest::proptest! {
        #[test]
        fn any(x in 0u32..10) { let _ = x; }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn early_works() { assert_eq!(early(0), 0); }
}
";
    let seam = seam_at(
        "src/lib.rs",
        source,
        "x > 40",
        SeamKind::PredicateBoundary,
        boundary(""),
    )?;
    match rust_test_stub(&seam, None, source).map_err(|r| r.as_str().to_string())? {
        RustTestStub {
            placement: TestStubPlacement::ExistingInlineModule { module_name, .. },
            ..
        } => assert_eq!(module_name, "tests"),
        other => return Err(format!("unexpected placement {:?}", other.placement)),
    }
    Ok(())
}

#[test]
fn owners_behind_a_non_test_cfg_are_refused() -> Result<(), String> {
    // Codex review of #5477: a stub beside a feature-gated owner compiles
    // out of a plain `cargo test`, which then builds zero tests and passes.
    let refused = [
        "#![cfg(feature = \"x\")]\npub fn f(n: u8) -> u8 { if n > 5 { 1 } else { 0 } }\n",
        "#[cfg(feature = \"x\")]\nmod inner {\n    pub fn f(n: u8) -> u8 { if n > 5 { 1 } else { 0 } }\n    #[cfg(test)]\n    mod tests {}\n}\n",
        "mod inner {\n    #![cfg(unix)]\n    pub fn f(n: u8) -> u8 { if n > 5 { 1 } else { 0 } }\n}\n",
        "#[cfg(feature = \"x\")]\npub fn f(n: u8) -> u8 { if n > 5 { 1 } else { 0 } }\n",
        "pub struct S;\n#[cfg(not(test))]\nimpl S {\n    pub fn f(&self, n: u8) -> u8 { if n > 5 { 1 } else { 0 } }\n}\n",
    ];
    for source in refused {
        let seam = seam_at(
            "src/lib.rs",
            source,
            "n > 5",
            SeamKind::PredicateBoundary,
            boundary(""),
        )?;
        assert_eq!(
            rust_test_stub(&seam, None, source).map(|stub| stub.test_name),
            Err(TestStubRefusal::OwnerUnsupported),
            "{source}"
        );
    }
    // Attributes a plain test build keeps do not refuse.
    let kept =
        "/// Docs.\n#[inline]\n#[must_use]\npub fn f(n: u8) -> u8 { if n > 5 { 1 } else { 0 } }\n";
    let seam = seam_at(
        "src/lib.rs",
        kept,
        "n > 5",
        SeamKind::PredicateBoundary,
        boundary(""),
    )?;
    assert!(rust_test_stub(&seam, None, kept).is_ok(), "{kept}");
    Ok(())
}
