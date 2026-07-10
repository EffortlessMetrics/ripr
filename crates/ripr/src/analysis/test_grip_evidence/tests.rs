use super::related_tests::context::*;
use super::related_tests::*;
use super::*;
use crate::analysis::rust_index::{RaRustSyntaxAdapter, RustSyntaxAdapter};
use crate::analysis::seam_inventory::inventory_seams_from_index;

fn index_from_files(files: &[(PathBuf, &str)]) -> Result<RustIndex, String> {
    let adapter = RaRustSyntaxAdapter;
    let mut index = RustIndex::default();
    for (path, source) in files {
        let facts = adapter.summarize_file(path, source)?;
        index.tests.extend(facts.tests.iter().cloned());
        index.functions.extend(facts.functions.iter().cloned());
        index.files.insert(path.clone(), facts);
    }
    Ok(index)
}

#[test]
fn call_arguments_uses_identifier_boundary_for_callee_name() {
    let text =
        "fn borrowed_amount_matches() { let amount = 100; let actual = amount_matches(&amount); }";

    assert_eq!(
        call_arguments(text, "amount_matches"),
        Some(vec!["&amount".to_string()])
    );
}

#[test]
fn latency_trace_line_uses_repo_exposure_trace_shape() {
    let line = latency_trace_line(
        "evidence_for_seams_progress",
        "processed_500_of_12337",
        Duration::from_millis(42),
    );

    assert_eq!(
        line,
        "ripr_repo_exposure_latency phase=evidence_for_seams_progress status=processed_500_of_12337 duration_ms=42"
    );
}

#[test]
fn latency_trace_line_can_report_evidence_context_start() {
    let line = latency_trace_line("evidence_context", "start_seams_12337", Duration::ZERO);

    assert_eq!(
        line,
        "ripr_repo_exposure_latency phase=evidence_context status=start_seams_12337 duration_ms=0"
    );
}

#[test]
fn oracle_semantics_explains_broad_error_gap_and_upgrade() {
    let semantics = oracle_semantics_for(
        &OracleKind::BroadError,
        &OracleStrength::Weak,
        SeamKind::ErrorVariant,
    );

    assert_eq!(semantics.observes, "some error occurred");
    assert_eq!(
        semantics.missing,
        "the exact error variant or payload that would discriminate the changed behavior"
    );
    assert_eq!(
        semantics.upgrade_suggestion.as_deref(),
        Some("assert the exact error variant with matches! or assert_matches!")
    );
}

#[test]
fn oracle_semantics_explains_smoke_only_boundary_gap() {
    let semantics = oracle_semantics_for(
        &OracleKind::SmokeOnly,
        &OracleStrength::Smoke,
        SeamKind::PredicateBoundary,
    );

    assert_eq!(
        semantics.observes,
        "the call completed or returned a broad ok/some/none shape"
    );
    assert_eq!(
        semantics.missing,
        "the output value, error variant, field, effect, or call discriminator"
    );
    assert_eq!(
        semantics.upgrade_suggestion.as_deref(),
        Some("add an exact returned-value assertion at the missing boundary value")
    );
}

#[test]
fn oracle_semantics_keeps_exact_value_without_extra_upgrade() {
    let semantics = oracle_semantics_for(
        &OracleKind::ExactValue,
        &OracleStrength::Strong,
        SeamKind::ReturnValue,
    );

    assert_eq!(
        semantics.observes,
        "the exact value or value pattern asserted by the test"
    );
    assert_eq!(
        semantics.missing,
        "no obvious value-shape discriminator gap under static scope"
    );
    assert!(semantics.upgrade_suggestion.is_none());
}

#[test]
fn oracle_semantics_covers_supported_oracle_families() {
    let cases = [
        (
            OracleKind::ExactErrorVariant,
            OracleStrength::Strong,
            SeamKind::ErrorVariant,
            "the exact error variant",
            Some(
                "assert the payload inside the matched error variant when payload behavior changed",
            ),
        ),
        (
            OracleKind::WholeObjectEquality,
            OracleStrength::Strong,
            SeamKind::ReturnValue,
            "whole output object equality",
            None,
        ),
        (
            OracleKind::Snapshot,
            OracleStrength::Medium,
            SeamKind::ReturnValue,
            "a snapshot of rendered or debug output",
            Some(
                "add an exact assertion for the changed field or value when the snapshot is broad",
            ),
        ),
        (
            OracleKind::RelationalCheck,
            OracleStrength::Weak,
            SeamKind::MatchArm,
            "a partial relationship or broad predicate about the result",
            Some("assert the exact enum or value produced by the changed match arm"),
        ),
        (
            OracleKind::MockExpectation,
            OracleStrength::Medium,
            SeamKind::SideEffect,
            "an expected call, event, state write, or persistence effect",
            None,
        ),
        (
            OracleKind::Unknown,
            OracleStrength::Unknown,
            SeamKind::CallPresence,
            "no recognized concrete oracle shape",
            Some("assert the expected call happened with the relevant arguments"),
        ),
        (
            OracleKind::Unknown,
            OracleStrength::None,
            SeamKind::FieldConstruction,
            "no recognized test oracle",
            Some("assert the specific output field that carries the changed behavior"),
        ),
    ];

    for (kind, strength, seam_kind, observes, upgrade) in cases {
        let semantics = oracle_semantics_for(&kind, &strength, seam_kind);
        assert_eq!(semantics.observes, observes);
        assert_eq!(semantics.upgrade_suggestion.as_deref(), upgrade);
    }
}

#[test]
fn opaque_custom_assertion_helper_stays_unknown_oracle() -> Result<(), String> {
    let files: Vec<(PathBuf, &str)> = vec![
        (
            PathBuf::from("src/pricing.rs"),
            "pub fn discounted_total(amount: i32, threshold: i32) -> i32 \
                 { if amount >= threshold { amount - 10 } else { amount } }\n",
        ),
        (
            PathBuf::from("tests/pricing_tests.rs"),
            "#[test]\n\
                 fn opaque_helper() {\n\
                     let total = discounted_total(100, 100);\n\
                     assert_discount_is_valid(&total);\n\
                 }\n",
        ),
    ];
    let index = index_from_files(&files)?;
    let seams = inventory_seams_from_index(&[PathBuf::from("src/pricing.rs")], &index);
    let predicate = seams
        .iter()
        .find(|seam| seam.kind() == SeamKind::PredicateBoundary)
        .ok_or_else(|| "predicate seam present".to_string())?;
    let evidence = evidence_for_seam(predicate, &index);
    let first = evidence
        .related_tests
        .first()
        .ok_or_else(|| "related test present".to_string())?;

    assert_eq!(first.oracle_kind, OracleKind::Unknown);
    assert_eq!(first.oracle_strength, OracleStrength::Unknown);
    assert_eq!(evidence.discriminate.state, StageState::Unknown);
    let semantics =
        oracle_semantics_for(&first.oracle_kind, &first.oracle_strength, predicate.kind());
    assert_eq!(semantics.observes, "no recognized concrete oracle shape");
    assert_eq!(
        semantics.upgrade_suggestion.as_deref(),
        Some("add an exact returned-value assertion at the missing boundary value")
    );
    Ok(())
}

#[test]
fn duplicative_equality_assertion_stays_weak_oracle() -> Result<(), String> {
    let files: Vec<(PathBuf, &str)> = vec![
        (
            PathBuf::from("src/pricing.rs"),
            "pub fn discounted_total(amount: i32, threshold: i32) -> i32 \
                 { if amount >= threshold { amount - 10 } else { amount } }\n",
        ),
        (
            PathBuf::from("tests/pricing_tests.rs"),
            "#[test]\n\
                 fn duplicated_equality() {\n\
                     let total = discounted_total(100, 100);\n\
                     assert_eq!(total, total);\n\
                 }\n",
        ),
    ];
    let index = index_from_files(&files)?;
    let seams = inventory_seams_from_index(&[PathBuf::from("src/pricing.rs")], &index);
    let predicate = seams
        .iter()
        .find(|seam| seam.kind() == SeamKind::PredicateBoundary)
        .ok_or_else(|| "predicate seam present".to_string())?;
    let evidence = evidence_for_seam(predicate, &index);
    let first = evidence
        .related_tests
        .first()
        .ok_or_else(|| "related test present".to_string())?;

    assert_eq!(first.oracle_kind, OracleKind::RelationalCheck);
    assert_eq!(first.oracle_strength, OracleStrength::Weak);
    assert_eq!(evidence.discriminate.state, StageState::Weak);
    let semantics =
        oracle_semantics_for(&first.oracle_kind, &first.oracle_strength, predicate.kind());
    assert_eq!(
        semantics.missing,
        "the exact changed value or boundary discriminator"
    );
    Ok(())
}

#[test]
fn given_boundary_seam_when_tests_skip_equal_value_then_evidence_reports_missing_boundary_discriminator()
-> Result<(), String> {
    // Production predicate compares amount >= threshold.
    let prod = PathBuf::from("src/pricing.rs");
    let prod_src = r#"
pub fn discounted_total(amount: i32, threshold: i32) -> i32 {
    if amount >= threshold { amount - 10 } else { amount }
}
"#;
    // Test calls owner with values strictly above and strictly below
    // the threshold but never with the equality case.
    let tests = PathBuf::from("tests/pricing_tests.rs");
    let tests_src = r#"
#[test]
fn below_threshold_has_no_discount() {
    assert_eq!(discounted_total(50, 100), 50);
}

#[test]
fn far_above_threshold_discounts() {
    assert_eq!(discounted_total(10000, 100), 9990);
}
"#;
    let index = index_from_files(&[(prod, prod_src), (tests, tests_src)])?;
    let seams = inventory_seams_from_index(&[PathBuf::from("src/pricing.rs")], &index);
    let predicate = seams
        .iter()
        .find(|s| s.kind() == SeamKind::PredicateBoundary)
        .ok_or_else(|| "expected predicate seam".to_string())?;

    let evidence = evidence_for_seam(predicate, &index);
    if evidence.related_tests.is_empty() {
        return Err("expected reach evidence to find related tests".to_string());
    }
    if evidence.missing_discriminators.is_empty() {
        return Err(format!(
            "expected at least one missing-discriminator hypothesis for boundary seam `{}`",
            predicate.expression()
        ));
    }
    let mentions_threshold = evidence
        .missing_discriminators
        .iter()
        .any(|fact| fact.value.contains("threshold"));
    if !mentions_threshold {
        return Err(format!(
            "missing-discriminator hypothesis should name the boundary identifier; got {:?}",
            evidence
                .missing_discriminators
                .iter()
                .map(|f| f.value.clone())
                .collect::<Vec<_>>()
        ));
    }
    Ok(())
}

#[test]
fn given_boundary_seam_when_test_uses_equal_value_and_exact_assertion_then_discriminate_evidence_is_yes()
-> Result<(), String> {
    let prod = PathBuf::from("src/pricing.rs");
    let prod_src = r#"
pub fn discounted_total(amount: i32, threshold: i32) -> i32 {
    if amount >= threshold { amount - 10 } else { amount }
}
"#;
    let tests = PathBuf::from("tests/pricing_tests.rs");
    let tests_src = r#"
#[test]
fn equality_boundary_returns_discount() {
    assert_eq!(discounted_total(100, 100), 90);
}
"#;
    let index = index_from_files(&[(prod, prod_src), (tests, tests_src)])?;
    let seams = inventory_seams_from_index(&[PathBuf::from("src/pricing.rs")], &index);
    let predicate = seams
        .iter()
        .find(|s| s.kind() == SeamKind::PredicateBoundary)
        .ok_or_else(|| "expected predicate seam".to_string())?;

    let evidence = evidence_for_seam(predicate, &index);
    if evidence.discriminate.state != StageState::Yes {
        return Err(format!(
            "expected discriminate=Yes, got {} ({})",
            evidence.discriminate.state.as_str(),
            evidence.discriminate.summary
        ));
    }
    assert!(
        evidence.missing_discriminators.is_empty(),
        "equal boundary arguments should satisfy the missing-discriminator hypothesis: {:?}",
        evidence.missing_discriminators
    );
    Ok(())
}

#[test]
fn given_boundary_seam_when_parameter_operands_are_called_with_equal_strings_then_missing_discriminator_is_cleared()
-> Result<(), String> {
    let prod = PathBuf::from("src/similarity.rs");
    let prod_src = r#"
pub fn similarity_key_contains(haystack: &str, needle: &str) -> bool {
    haystack == needle || haystack.contains(needle)
}
"#;
    let tests = PathBuf::from("tests/similarity_tests.rs");
    let tests_src = r#"
#[test]
fn exact_similarity_key_matches() {
    assert!(similarity_key_contains("apply_discount", "apply_discount"));
}
"#;
    let index = index_from_files(&[(prod, prod_src), (tests, tests_src)])?;
    let seams = inventory_seams_from_index(&[PathBuf::from("src/similarity.rs")], &index);
    let predicate = seams
        .iter()
        .find(|s| {
            s.kind() == SeamKind::PredicateBoundary && s.expression().contains("haystack == needle")
        })
        .ok_or_else(|| "expected equality predicate seam".to_string())?;

    let evidence = evidence_for_seam(predicate, &index);

    assert!(
        evidence.missing_discriminators.is_empty(),
        "equal string boundary arguments should clear the missing-discriminator hypothesis: {:?}",
        evidence.missing_discriminators
    );
    Ok(())
}

#[test]
fn given_error_variant_seam_when_test_only_asserts_is_err_then_discriminate_evidence_is_weak()
-> Result<(), String> {
    let prod = PathBuf::from("src/parse.rs");
    let prod_src = r#"
pub enum AuthError { RevokedToken, Expired }

pub fn parse(value: &str) -> Result<i32, AuthError> {
    if value.is_empty() {
        return Err(AuthError::RevokedToken);
    }
    Ok(0)
}
"#;
    let tests = PathBuf::from("tests/parse_tests.rs");
    let tests_src = r#"
#[test]
fn parse_rejects_empty() {
    assert!(parse("").is_err());
}
"#;
    let index = index_from_files(&[(prod, prod_src), (tests, tests_src)])?;
    let seams = inventory_seams_from_index(&[PathBuf::from("src/parse.rs")], &index);
    let error_seam = seams
        .iter()
        .find(|s| s.kind() == SeamKind::ErrorVariant)
        .ok_or_else(|| "expected error_variant seam".to_string())?;

    let evidence = evidence_for_seam(error_seam, &index);
    if evidence.discriminate.state != StageState::Weak
        && evidence.discriminate.state != StageState::Unknown
    {
        return Err(format!(
            "expected discriminate=Weak|Unknown for is_err-only oracle, got {}",
            evidence.discriminate.state.as_str()
        ));
    }
    Ok(())
}

#[test]
fn given_error_variant_seam_when_test_asserts_exact_variant_then_discriminate_evidence_is_yes()
-> Result<(), String> {
    let prod = PathBuf::from("src/parse.rs");
    let prod_src = r#"
pub enum AuthError { RevokedToken, Expired }

pub fn parse(value: &str) -> Result<i32, AuthError> {
    if value.is_empty() {
        return Err(AuthError::RevokedToken);
    }
    Ok(0)
}
"#;
    let tests = PathBuf::from("tests/parse_tests.rs");
    let tests_src = r#"
#[test]
fn parse_returns_revoked_token_on_empty() {
    assert!(matches!(parse(""), Err(AuthError::RevokedToken)));
}
"#;
    let index = index_from_files(&[(prod, prod_src), (tests, tests_src)])?;
    let seams = inventory_seams_from_index(&[PathBuf::from("src/parse.rs")], &index);
    let error_seam = seams
        .iter()
        .find(|s| s.kind() == SeamKind::ErrorVariant)
        .ok_or_else(|| "expected error_variant seam".to_string())?;

    let evidence = evidence_for_seam(error_seam, &index);
    if evidence.discriminate.state != StageState::Yes {
        return Err(format!(
            "expected discriminate=Yes for matches!(...AuthError::RevokedToken), got {} ({})",
            evidence.discriminate.state.as_str(),
            evidence.discriminate.summary
        ));
    }
    Ok(())
}

#[test]
fn given_error_constructor_payload_seam_when_test_asserts_exact_payload_then_discriminate_evidence_is_yes()
-> Result<(), String> {
    let prod = PathBuf::from("src/entry_validation.rs");
    let prod_src = r#"
#[derive(Debug, PartialEq, Eq)]
pub struct CargoAllowError(String);

impl CargoAllowError {
    pub fn new(message: String) -> Self {
        Self(message)
    }
}

pub fn validate_allow_entry_identity(id: &str, already_seen: bool) -> Result<(), CargoAllowError> {
    if already_seen {
        return Err(CargoAllowError::new(format!("duplicate allow id `{}`", id)));
    }
    Ok(())
}
"#;
    let tests = PathBuf::from("tests/entry_validation_tests.rs");
    let tests_src = r#"
use entry_validation::{CargoAllowError, validate_allow_entry_identity};

#[test]
fn duplicate_allow_id_reports_exact_error_payload() {
    let duplicate_id = "duplicate";
    let err = validate_allow_entry_identity(duplicate_id, true)
        .expect_err("duplicate allow ids should fail identity validation");
    assert_eq!(
        err,
        CargoAllowError::new(format!("duplicate allow id `{}`", duplicate_id))
    );
}
"#;
    let index = index_from_files(&[(prod, prod_src), (tests, tests_src)])?;
    let seams = inventory_seams_from_index(&[PathBuf::from("src/entry_validation.rs")], &index);
    let error_seam = seams
        .iter()
        .find(|seam| {
            seam.kind() == SeamKind::ErrorVariant
                && seam.expression().contains("CargoAllowError::new")
        })
        .ok_or_else(|| "expected CargoAllowError::new error_variant seam".to_string())?;

    let evidence = evidence_for_seam(error_seam, &index);
    if evidence.discriminate.state != StageState::Yes {
        return Err(format!(
            "expected discriminate=Yes for exact constructor payload assertion, got {} ({})",
            evidence.discriminate.state.as_str(),
            evidence.discriminate.summary
        ));
    }
    Ok(())
}

#[test]
fn error_constructor_payload_match_requires_same_constructor_and_literal() -> Result<(), String> {
    let seam = r#"return Err(CargoAllowError::new(format!("duplicate allow id `{}`", id)));"#;
    let matching_oracle =
        r#"assert_eq!(err, CargoAllowError::new(format!("duplicate allow id `{}`", id)));"#;
    if !error_constructor_payload_oracle_matches_seam(seam, matching_oracle) {
        return Err("same constructor and payload literal should match".to_string());
    }

    let different_payload =
        r#"assert_eq!(err, CargoAllowError::new(format!("unknown allow id `{}`", id)));"#;
    if error_constructor_payload_oracle_matches_seam(seam, different_payload) {
        return Err("different payload literal must not match".to_string());
    }

    let different_constructor =
        r#"assert_eq!(err, OtherAllowError::new(format!("duplicate allow id `{}`", id)));"#;
    if error_constructor_payload_oracle_matches_seam(seam, different_constructor) {
        return Err("different constructor path must not match".to_string());
    }
    let assertion_message_only = r#"assert_eq!(err, CargoAllowError::new(format!("unknown allow id `{}`", id)), "duplicate allow id `{}`");"#;
    if error_constructor_payload_oracle_matches_seam(seam, assertion_message_only) {
        return Err("assertion message literals must not satisfy constructor payload".to_string());
    }

    let multi_arg_seam =
        r#"return Err(CargoAllowError::new("E_DUPLICATE", "duplicate allow id"));"#;
    let partially_shared_multi_arg =
        r#"assert_eq!(err, CargoAllowError::new("E_DUPLICATE", "unknown allow id"));"#;
    if error_constructor_payload_oracle_matches_seam(multi_arg_seam, partially_shared_multi_arg) {
        return Err("partially shared constructor literals must not match".to_string());
    }
    Ok(())
}

#[test]
fn given_string_error_payload_seam_when_test_asserts_exact_err_then_discriminate_is_yes()
-> Result<(), String> {
    let prod = PathBuf::from("src/artifact_sample_schema_support.rs");
    let prod_src = r#"
pub fn schema_covers_sample_value(path: &str, missing: &[&str]) -> Result<(), String> {
    if !missing.is_empty() {
        return Err(format!(
            "{path} is missing schema-required keys: {}",
            missing.join(", ")
        ));
    }
    Ok(())
}
"#;
    let tests = PathBuf::from("tests/artifact_sample_schema_support_tests.rs");
    let tests_src = r#"
use artifact_sample_schema_support::schema_covers_sample_value;

#[test]
fn artifact_sample_validator_reports_object_shape_errors() {
    assert_eq!(
        schema_covers_sample_value("$", &["name"]),
        Err("$ is missing schema-required keys: name".to_string())
    );
}
"#;
    let index = index_from_files(&[(prod, prod_src), (tests, tests_src)])?;
    let seams = inventory_seams_from_index(
        &[PathBuf::from("src/artifact_sample_schema_support.rs")],
        &index,
    );
    let error_seam = seams
        .iter()
        .find(|seam| {
            seam.kind() == SeamKind::ErrorVariant
                && seam.expression().contains("schema-required keys")
        })
        .ok_or_else(|| "expected string payload error_variant seam".to_string())?;

    let evidence = evidence_for_seam(error_seam, &index);
    if evidence.discriminate.state != StageState::Yes {
        return Err(format!(
            "expected discriminate=Yes for exact Err(String) payload assertion, got {} ({})",
            evidence.discriminate.state.as_str(),
            evidence.discriminate.summary
        ));
    }
    Ok(())
}

#[test]
fn given_bound_string_error_payload_assertion_then_discriminate_is_yes() -> Result<(), String> {
    let prod = PathBuf::from("src/artifact_sample_schema_support.rs");
    let prod_src = r#"
pub fn schema_covers_sample_value(path: &str, reference: &str) -> Result<(), String> {
    return Err(format!("{path} schema uses non-local ref {reference}"));
}
"#;
    let tests = PathBuf::from("tests/artifact_sample_schema_support_tests.rs");
    let tests_src = r#"
use artifact_sample_schema_support::schema_covers_sample_value;

#[test]
fn artifact_sample_validator_reports_ref_errors() {
    let path = "$";
    let reference = "other.json";
    let err = schema_covers_sample_value(path, reference)
        .expect_err("non-local schema refs should fail validation");
    assert_eq!(err, format!("{path} schema uses non-local ref {reference}"));
}
"#;
    let index = index_from_files(&[(prod, prod_src), (tests, tests_src)])?;
    let seams = inventory_seams_from_index(
        &[PathBuf::from("src/artifact_sample_schema_support.rs")],
        &index,
    );
    let error_seam = seams
        .iter()
        .find(|seam| {
            seam.kind() == SeamKind::ErrorVariant
                && seam.expression().contains("schema uses non-local ref")
        })
        .ok_or_else(|| "expected string payload error_variant seam".to_string())?;

    let evidence = evidence_for_seam(error_seam, &index);
    if evidence.discriminate.state != StageState::Yes {
        return Err(format!(
            "expected discriminate=Yes for bound exact string payload assertion, got {} ({})",
            evidence.discriminate.state.as_str(),
            evidence.discriminate.summary
        ));
    }
    Ok(())
}

#[test]
fn string_error_payload_match_ignores_assertion_message_and_thin_templates() -> Result<(), String> {
    let seam =
        r#"return Err(format!("{path} is missing schema-required keys: {}", missing.join(", ")));"#;
    let matching_oracle =
        r#"assert_eq!(validate(), Err("$ is missing schema-required keys: name".to_string()));"#;
    if !error_string_payload_oracle_matches_seam(seam, matching_oracle) {
        return Err("format payload should match concrete asserted Err string".to_string());
    }

    let assertion_message_only = r#"assert_eq!(validate(), Err("$ has unrelated error".to_string()), "$ is missing schema-required keys: name");"#;
    if error_string_payload_oracle_matches_seam(seam, assertion_message_only) {
        return Err("assertion message must not satisfy string error payload".to_string());
    }

    let thin_template = r#"return Err(format!("{}"));"#;
    let observed = r#"assert_eq!(validate(), Err("anything".to_string()));"#;
    if error_string_payload_oracle_matches_seam(thin_template, observed) {
        return Err("format templates without meaningful fixed text must fail closed".to_string());
    }
    Ok(())
}

#[test]
fn string_error_payload_match_accepts_multiline_any_of_assertion() -> Result<(), String> {
    let seam = r#"return Err(format!(
            "{path} did not match any anyOf branch: {}",
            errors.join("; ")
        ))"#;
    let matching_oracle = r##"assert_eq!(
            schema_covers_sample_value(&any_of_schema, &any_of_schema, &json_value(r#""check""#), "$.mode"),
            Err("$.mode did not match any anyOf branch: $.mode has value \"check\", expected const \"allow\"; $.mode has value \"check\", expected const \"audit\"".to_string())
        );"##;
    if !error_string_payload_oracle_matches_seam(seam, matching_oracle) {
        return Err("real cargo-allow anyOf payload assertion should match".to_string());
    }
    Ok(())
}

#[test]
fn string_error_payload_match_requires_exact_plain_payload() -> Result<(), String> {
    let seam = r#"return Err("permission denied".to_string());"#;
    let containing_oracle = r#"assert_eq!(validate(), Err("not permission denied".to_string()));"#;
    if error_string_payload_oracle_matches_seam(seam, containing_oracle) {
        return Err("plain error payloads must not match by substring".to_string());
    }

    let exact_oracle = r#"assert_eq!(validate(), Err("permission denied".to_string()));"#;
    if !error_string_payload_oracle_matches_seam(seam, exact_oracle) {
        return Err("plain error payloads should still match exact assertions".to_string());
    }
    Ok(())
}

#[test]
fn string_error_payload_match_fails_closed_for_unmatched_payload_shapes() -> Result<(), String> {
    let cases = [
        (
            "non Err seam",
            r#"return Ok(());"#,
            r#"assert_eq!(validate(), Err("permission denied".to_string()));"#,
            false,
        ),
        (
            "oracle without payload literal",
            r#"return Err("permission denied".to_string());"#,
            r#"assert!(validate().is_err());"#,
            false,
        ),
        (
            "missing required format fragment",
            r#"return Err(format!("schema-required {path} keys missing"));"#,
            r#"assert_eq!(validate(), Err("schema-required $".to_string()));"#,
            false,
        ),
        (
            "first required format fragment absent",
            r#"return Err(format!("schema-required {path} keys missing"));"#,
            r#"assert_eq!(validate(), Err("prefix $ keys missing".to_string()));"#,
            false,
        ),
        (
            "shared delimiter literal is auxiliary",
            r#"return Err(format!("left payload {}", values.join(", ")));"#,
            r#"assert_eq!(validate(), Err(format!("right payload {}", values.join(", "))));"#,
            false,
        ),
        (
            "leading fixed fragment is anchored",
            r#"return Err(format!("permission denied: {reason}"));"#,
            r#"assert_eq!(validate(), Err("not permission denied: root".to_string()));"#,
            false,
        ),
        (
            "trailing fixed fragment is anchored",
            r#"return Err(format!("{path} permission denied"));"#,
            r#"assert_eq!(validate(), Err("$.mode permission denied extra".to_string()));"#,
            false,
        ),
        (
            "leading placeholder can precede fixed text",
            r#"return Err(format!("{path} schema uses non-local ref {reference}"));"#,
            r#"assert_eq!(validate(), Err("$ schema uses non-local ref other.json".to_string()));"#,
            true,
        ),
        (
            "escaped braces remain fixed text",
            r#"return Err(format!("schema {{required}} {path} keys missing"));"#,
            r#"assert_eq!(validate(), Err("schema {required} $.mode keys missing".to_string()));"#,
            true,
        ),
    ];
    for (label, seam, oracle, expected) in cases {
        let actual = error_string_payload_oracle_matches_seam(seam, oracle);
        if actual != expected {
            return Err(format!(
                "{label}: expected {expected}, got {actual} for seam {seam:?} and oracle {oracle:?}"
            ));
        }
    }
    Ok(())
}

#[test]
fn given_side_effect_seam_when_no_effect_observer_exists_then_observe_evidence_is_weak_or_unknown()
-> Result<(), String> {
    let prod = PathBuf::from("src/publish.rs");
    // The production function calls `service.publish(...)` — a method
    // whose name matches `is_effect_call_name`, so the parser emits
    // a side_effect probe shape on the call site.
    let prod_src = r#"
pub struct Service;
pub struct Event;

impl Service {
    pub fn publish(&mut self, _event: Event) {}
}

pub fn publish_message(service: &mut Service, event: Event) {
    service.publish(event);
}
"#;
    let tests = PathBuf::from("tests/publish_tests.rs");
    // Test reaches `publish_message` but does not observe the
    // side-effect (no mock, no assertion that the publish happened).
    let tests_src = r#"
#[test]
fn publish_runs_without_panic() {
    let mut service = Service;
    publish_message(&mut service, Event);
}
"#;
    let index = index_from_files(&[(prod, prod_src), (tests, tests_src)])?;
    let seams = inventory_seams_from_index(&[PathBuf::from("src/publish.rs")], &index);
    let side_effect = seams
        .iter()
        .find(|s| s.kind() == SeamKind::SideEffect)
        .ok_or_else(|| {
            format!(
                "expected side_effect seam, got kinds: {:?}",
                seams.iter().map(|s| s.kind().as_str()).collect::<Vec<_>>()
            )
        })?;

    let evidence = evidence_for_seam(side_effect, &index);
    match evidence.observe.state {
        StageState::No | StageState::Weak | StageState::Unknown => Ok(()),
        other => Err(format!(
            "expected observe in {{No, Weak, Unknown}} for side-effect with no observer, got {}",
            other.as_str()
        )),
    }
}

#[test]
fn given_side_effect_seam_when_event_assertion_exists_then_oracle_observes_effect()
-> Result<(), String> {
    let prod = PathBuf::from("src/publish.rs");
    let prod_src = r#"
pub struct Service;
pub struct Event;

impl Service {
    pub fn publish(&mut self, _event: Event) {}
}

pub fn publish_message(service: &mut Service, event: Event) {
    service.publish(event);
}
"#;
    let tests = PathBuf::from("tests/publish_tests.rs");
    let tests_src = r#"
#[test]
fn publish_records_event() {
    let mut service = Service;
    publish_message(&mut service, Event);
    assert!(service.published_events().contains(&"message"));
}
"#;
    let index = index_from_files(&[(prod, prod_src), (tests, tests_src)])?;
    let seams = inventory_seams_from_index(&[PathBuf::from("src/publish.rs")], &index);
    let side_effect = seams
        .iter()
        .find(|s| s.kind() == SeamKind::SideEffect)
        .ok_or_else(|| "expected side_effect seam".to_string())?;

    let evidence = evidence_for_seam(side_effect, &index);
    assert_eq!(evidence.observe.state, StageState::Yes);
    assert_eq!(evidence.propagate.state, StageState::Yes);
    assert_eq!(evidence.discriminate.state, StageState::Yes);
    assert!(
        evidence
            .related_tests
            .iter()
            .any(|test| test.oracle_kind == OracleKind::MockExpectation)
    );
    Ok(())
}

#[test]
fn given_opaque_helper_when_values_cannot_be_seen_then_evidence_records_static_limitation()
-> Result<(), String> {
    // Test reaches the owner only through a helper, so no concrete
    // activation values are visible. Activation should not be Yes.
    let prod = PathBuf::from("src/pricing.rs");
    let prod_src = r#"
pub fn discounted_total(amount: i32, threshold: i32) -> i32 {
    if amount >= threshold { amount - 10 } else { amount }
}
"#;
    let tests = PathBuf::from("tests/pricing_tests.rs");
    let tests_src = r#"
fn make_input() -> (i32, i32) { (50, 100) }

#[test]
fn helper_path_runs() {
    let (a, t) = make_input();
    let _ = discounted_total(a, t);
    assert!(true);
}
"#;
    let index = index_from_files(&[(prod, prod_src), (tests, tests_src)])?;
    let seams = inventory_seams_from_index(&[PathBuf::from("src/pricing.rs")], &index);
    let predicate = seams
        .iter()
        .find(|s| s.kind() == SeamKind::PredicateBoundary)
        .ok_or_else(|| "expected predicate seam".to_string())?;

    let evidence = evidence_for_seam(predicate, &index);
    if evidence.activate.state == StageState::Yes {
        return Err(format!(
            "expected activate != Yes for helper-supplied values, got {} ({})",
            evidence.activate.state.as_str(),
            evidence.activate.summary
        ));
    }
    Ok(())
}

#[test]
fn evidence_for_seams_is_deterministic_across_input_order() -> Result<(), String> {
    let prod = PathBuf::from("src/pricing.rs");
    let prod_src = r#"
pub fn discounted_total(amount: i32, threshold: i32) -> i32 {
    if amount >= threshold { amount - 10 } else { amount }
}
"#;
    let tests = PathBuf::from("tests/pricing_tests.rs");
    let tests_src = r#"
#[test]
fn boundary_case() {
    assert_eq!(discounted_total(100, 100), 90);
}
#[test]
fn below_case() {
    assert_eq!(discounted_total(50, 100), 50);
}
"#;
    let index = index_from_files(&[(prod, prod_src), (tests, tests_src)])?;
    let mut seams = inventory_seams_from_index(&[PathBuf::from("src/pricing.rs")], &index);
    let forward_ids: Vec<String> = evidence_for_seams(&seams, &index)
        .iter()
        .map(|e| e.seam_id.as_str().to_string())
        .collect();
    seams.reverse();
    let reversed_ids: Vec<String> = evidence_for_seams(&seams, &index)
        .iter()
        .map(|e| e.seam_id.as_str().to_string())
        .collect();
    if forward_ids != reversed_ids {
        return Err(format!(
            "evidence order is not stable:\n  forward: {forward_ids:?}\n  reversed: {reversed_ids:?}"
        ));
    }
    Ok(())
}

#[test]
fn evidence_for_seams_matches_single_seam_evidence_while_reusing_context() -> Result<(), String> {
    let prod = PathBuf::from("src/pricing.rs");
    let prod_src = r#"
pub fn discounted_total(amount: i32, threshold: i32) -> i32 {
    if amount >= threshold { amount - 10 } else { amount }
}
"#;
    let tests = PathBuf::from("tests/pricing_tests.rs");
    let tests_src = r#"
#[test]
fn equality_boundary_returns_discount() {
    assert_eq!(discounted_total(100, 100), 90);
}
#[test]
fn import_only_mentions_owner() {
    use crate::pricing::discounted_total;
    assert_eq!(1, 1);
}
"#;
    let index = index_from_files(&[(prod, prod_src), (tests, tests_src)])?;
    let seams = inventory_seams_from_index(&[PathBuf::from("src/pricing.rs")], &index);
    let batch = evidence_for_seams(&seams, &index);

    for seam in &seams {
        let single = evidence_for_seam(seam, &index);
        let Some(from_batch) = batch.iter().find(|entry| entry.seam_id == *seam.id()) else {
            return Err(format!(
                "batch evidence missing seam {}",
                seam.id().as_str()
            ));
        };
        let single_json =
            serde_json::to_string(&single).map_err(|err| format!("encode single: {err}"))?;
        let batch_json =
            serde_json::to_string(from_batch).map_err(|err| format!("encode batch: {err}"))?;
        assert_eq!(single_json, batch_json);
    }
    Ok(())
}

#[test]
fn given_compact_evidence_when_direct_owner_call_reaches_error_seam_then_activation_is_yes()
-> Result<(), String> {
    let prod = PathBuf::from("src/parse.rs");
    let prod_src = r#"
pub enum AuthError { RevokedToken, Expired }

pub fn parse(value: &str) -> Result<i32, AuthError> {
    if value.is_empty() {
        return Err(AuthError::RevokedToken);
    }
    Ok(0)
}
"#;
    let tests = PathBuf::from("tests/parse_tests.rs");
    let tests_src = r#"
#[test]
fn parse_rejects_empty() {
    assert!(parse("").is_err());
}
"#;
    let index = index_from_files(&[(prod, prod_src), (tests, tests_src)])?;
    let seams = inventory_seams_from_index(&[PathBuf::from("src/parse.rs")], &index);
    let error_seam = seams
        .iter()
        .find(|s| s.kind() == SeamKind::ErrorVariant)
        .ok_or_else(|| "expected error_variant seam".to_string())?;
    let context = CompactGripContext::new(&index);

    let evidence = compact_evidence_for_seam(error_seam, &context);

    assert_eq!(evidence.reach.state, StageState::Yes);
    assert_eq!(evidence.activate.state, StageState::Yes);
    assert_eq!(evidence.related_tests.len(), 0);
    assert_eq!(evidence.observed_values.len(), 0);
    assert_eq!(evidence.missing_discriminators.len(), 0);
    Ok(())
}

#[test]
fn given_full_evidence_when_no_arg_owner_call_reaches_return_seam_then_activation_is_yes()
-> Result<(), String> {
    let prod = PathBuf::from("src/labels.rs");
    let prod_src = r#"
pub fn device_labels() -> Vec<&'static str> {
    Vec::new()
}
"#;
    let tests = PathBuf::from("tests/labels_tests.rs");
    let tests_src = r#"
#[test]
fn device_labels_start_empty() {
    assert!(device_labels().is_empty());
}
"#;
    let index = index_from_files(&[(prod, prod_src), (tests, tests_src)])?;
    let seams = inventory_seams_from_index(&[PathBuf::from("src/labels.rs")], &index);
    let return_seam = seams
        .iter()
        .find(|s| s.kind() == SeamKind::ReturnValue && s.expression().contains("Vec::new()"))
        .ok_or_else(|| "expected Vec::new return_value seam".to_string())?;

    let evidence = evidence_for_seam(return_seam, &index);

    assert_eq!(evidence.reach.state, StageState::Yes);
    assert_eq!(evidence.activate.state, StageState::Yes);
    assert!(
        evidence.observed_values.is_empty(),
        "no-arg activation should not invent observed values: {:?}",
        evidence.observed_values
    );
    assert!(
        evidence
            .activate
            .summary
            .contains("direct owner call for value-insensitive seam"),
        "activation summary should explain the value-insensitive owner-call route: {}",
        evidence.activate.summary
    );
    assert!(
        evidence.missing_discriminators.is_empty(),
        "return-value no-arg activation must not create boundary debt"
    );
    Ok(())
}

#[test]
fn given_full_evidence_when_multiline_no_arg_owner_call_reaches_return_seam_then_activation_is_yes()
-> Result<(), String> {
    let prod = PathBuf::from("src/labels.rs");
    let prod_src = r#"
pub fn device_labels() -> Vec<&'static str> {
    Vec::new()
}
"#;
    let tests = PathBuf::from("tests/labels_tests.rs");
    let tests_src = r#"
#[test]
fn device_labels_start_empty() {
    let labels = device_labels(
    );
    assert!(labels.is_empty());
}
"#;
    let index = index_from_files(&[(prod, prod_src), (tests, tests_src)])?;
    let seams = inventory_seams_from_index(&[PathBuf::from("src/labels.rs")], &index);
    let return_seam = seams
        .iter()
        .find(|s| s.kind() == SeamKind::ReturnValue && s.expression().contains("Vec::new()"))
        .ok_or_else(|| "expected Vec::new return_value seam".to_string())?;

    let evidence = evidence_for_seam(return_seam, &index);

    assert_eq!(evidence.reach.state, StageState::Yes);
    assert_eq!(evidence.activate.state, StageState::Yes);
    assert!(
        evidence.observed_values.is_empty(),
        "multiline no-arg activation should not invent observed values: {:?}",
        evidence.observed_values
    );
    assert!(
        evidence
            .activate
            .summary
            .contains("direct owner call for value-insensitive seam"),
        "activation summary should explain the value-insensitive owner-call route: {}",
        evidence.activate.summary
    );
    assert!(
        evidence.missing_discriminators.is_empty(),
        "multiline value-insensitive direct owner calls must not create boundary debt"
    );
    Ok(())
}

#[test]
fn given_full_evidence_when_one_hop_helper_calls_owner_then_value_insensitive_activation_is_yes()
-> Result<(), String> {
    let source = PathBuf::from("src/labels.rs");
    let source_src = r#"
pub fn device_labels() -> Vec<&'static str> {
    Vec::new()
}

fn exercise_device_labels() -> Vec<&'static str> {
    device_labels()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn helper_reaches_device_labels() {
        let labels = exercise_device_labels();
        assert!(labels.is_empty());
    }
}
"#;
    let index = index_from_files(&[(source, source_src)])?;
    let seams = inventory_seams_from_index(&[PathBuf::from("src/labels.rs")], &index);
    let return_seam = seams
        .iter()
        .find(|s| {
            s.kind() == SeamKind::ReturnValue
                && s.owner().ends_with("::device_labels")
                && s.expression().contains("Vec::new()")
        })
        .ok_or_else(|| "expected Vec::new return_value seam".to_string())?;

    let evidence = evidence_for_seam(return_seam, &index);

    assert_eq!(evidence.reach.state, StageState::Yes);
    assert_eq!(evidence.activate.state, StageState::Yes);
    assert!(
        evidence
            .related_tests
            .iter()
            .any(|test| test.relation_reason == RelationReason::HelperOwnerCall),
        "expected helper owner-call related test, got {:?}",
        evidence.related_tests
    );
    assert!(
        evidence.observed_values.is_empty(),
        "one-hop helper activation should not invent observed values: {:?}",
        evidence.observed_values
    );
    assert!(
        evidence
            .activate
            .summary
            .contains("helper owner call for value-insensitive seam"),
        "activation summary should explain the helper owner-call route: {}",
        evidence.activate.summary
    );
    assert!(
        evidence.missing_discriminators.is_empty(),
        "helper owner calls must not create boundary debt"
    );
    Ok(())
}

#[test]
fn given_full_evidence_when_one_hop_helper_does_not_call_owner_then_activation_stays_unknown()
-> Result<(), String> {
    let source = PathBuf::from("src/labels.rs");
    let source_src = r#"
pub fn device_labels() -> Vec<&'static str> {
    Vec::new()
}

fn exercise_device_labels() -> Vec<&'static str> {
    Vec::new()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn return_value_contract_mentions_empty_output() {
        let return_value = exercise_device_labels();
        assert!(return_value.is_empty());
    }
}
"#;
    let index = index_from_files(&[(source, source_src)])?;
    let seams = inventory_seams_from_index(&[PathBuf::from("src/labels.rs")], &index);
    let return_seam = seams
        .iter()
        .find(|s| {
            s.kind() == SeamKind::ReturnValue
                && s.owner().ends_with("::device_labels")
                && s.expression().contains("Vec::new()")
        })
        .ok_or_else(|| "expected Vec::new return_value seam".to_string())?;

    let evidence = evidence_for_seam(return_seam, &index);

    assert_eq!(evidence.reach.state, StageState::Yes);
    assert!(
        !evidence
            .related_tests
            .iter()
            .any(|test| test.relation_reason == RelationReason::HelperOwnerCall),
        "helper that does not call the owner must not get helper-owner relation: {:?}",
        evidence.related_tests
    );
    assert_eq!(evidence.activate.state, StageState::Unknown);
    assert!(
        evidence
            .activate
            .summary
            .contains("No direct owner call observed for value-insensitive seam"),
        "activation summary should keep owner-call limitation, got {}",
        evidence.activate.summary
    );
    Ok(())
}

#[test]
fn given_full_evidence_when_generic_helper_name_mentions_owner_then_activation_stays_unknown()
-> Result<(), String> {
    let source = PathBuf::from("src/parser.rs");
    let source_src = r#"
pub fn parse() -> String {
    String::new()
}

fn parse_fixture() -> String {
    parse()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fixture_checks_empty_output() {
        let parsed = parse_fixture();
        assert!(parsed.is_empty());
    }
}
"#;
    let index = index_from_files(&[(source, source_src)])?;
    let seams = inventory_seams_from_index(&[PathBuf::from("src/parser.rs")], &index);
    let return_seam = seams
        .iter()
        .find(|s| {
            s.kind() == SeamKind::ReturnValue
                && s.owner().ends_with("::parse")
                && s.expression().contains("String::new()")
        })
        .ok_or_else(|| "expected String::new return_value seam".to_string())?;

    let evidence = evidence_for_seam(return_seam, &index);

    assert_eq!(evidence.reach.state, StageState::Yes);
    assert!(
        !evidence
            .related_tests
            .iter()
            .any(|test| test.relation_reason == RelationReason::HelperOwnerCall),
        "generic helper-owner token must not get helper relation: {:?}",
        evidence.related_tests
    );
    assert_eq!(evidence.activate.state, StageState::Unknown);
    assert!(
        evidence.observed_values.is_empty(),
        "generic helper route must not invent observed values: {:?}",
        evidence.observed_values
    );
    Ok(())
}

#[test]
fn given_value_insensitive_seam_when_only_affinity_related_then_activation_names_owner_call_limitation()
-> Result<(), String> {
    let prod = PathBuf::from("src/labels.rs");
    let prod_src = r#"
pub fn device_labels() -> Vec<&'static str> {
    Vec::new()
}
"#;
    let tests = PathBuf::from("tests/contract_tests.rs");
    let tests_src = r#"
#[test]
fn return_value_contract_mentions_empty_output() {
    // Text mentions like `device_labels(` are not owner calls.
    let note = "device_labels(";
    let return_value = Vec::<&str>::new();
    assert!(!note.is_empty());
    assert!(return_value.is_empty());
}
"#;
    let index = index_from_files(&[(prod, prod_src), (tests, tests_src)])?;
    let seams = inventory_seams_from_index(&[PathBuf::from("src/labels.rs")], &index);
    let return_seam = seams
        .iter()
        .find(|s| s.kind() == SeamKind::ReturnValue && s.expression().contains("Vec::new()"))
        .ok_or_else(|| "expected Vec::new return_value seam".to_string())?;

    let evidence = evidence_for_seam(return_seam, &index);

    assert_eq!(evidence.reach.state, StageState::Yes);
    assert!(
        evidence
            .related_tests
            .iter()
            .any(|test| test.relation_reason == RelationReason::AssertionTargetAffinity),
        "expected assertion-target affinity related test, got {:?}",
        evidence.related_tests
    );
    assert!(
        !evidence
            .related_tests
            .iter()
            .any(|test| test.relation_reason == RelationReason::DirectOwnerCall),
        "string/comment owner mentions must not become direct owner-call evidence: {:?}",
        evidence.related_tests
    );
    assert_eq!(evidence.activate.state, StageState::Unknown);
    assert!(
        evidence
            .activate
            .summary
            .contains("No direct owner call observed for value-insensitive seam"),
        "activation summary should name the owner-call limitation, got {}",
        evidence.activate.summary
    );
    assert!(
        evidence.observed_values.is_empty(),
        "affinity-only activation must not invent observed values: {:?}",
        evidence.observed_values
    );
    Ok(())
}

#[test]
fn given_value_insensitive_seam_when_multi_owner_wrapper_has_target_affinity_then_activation_is_yes()
-> Result<(), String> {
    let prod = PathBuf::from("src/labels.rs");
    let prod_src = r#"
pub const DEVICE_LABELS_EMPTY: &str = "empty";
pub const USER_LABELS_READY: &str = "ready";

pub fn device_label_status() -> &'static str {
    DEVICE_LABELS_EMPTY
}

pub fn user_label_status() -> &'static str {
    USER_LABELS_READY
}

pub fn exercise_statuses() -> (&'static str, &'static str) {
    (device_label_status(), user_label_status())
}
"#;
    let tests = PathBuf::from("tests/status_contract.rs");
    let tests_src = r#"
use labels::{exercise_statuses, DEVICE_LABELS_EMPTY};

#[test]
fn status_wrapper_observes_device_target() {
    let (return_value, _) = exercise_statuses();
    assert_eq!(return_value, DEVICE_LABELS_EMPTY);
}
"#;
    let index = index_from_files(&[(prod, prod_src), (tests, tests_src)])?;
    let seams = inventory_seams_from_index(&[PathBuf::from("src/labels.rs")], &index);
    let return_seam = seams
        .iter()
        .find(|s| {
            s.kind() == SeamKind::ReturnValue
                && s.owner().ends_with("::device_label_status")
                && s.expression().contains("DEVICE_LABELS_EMPTY")
        })
        .ok_or_else(|| "expected DEVICE_LABELS_EMPTY return_value seam".to_string())?;

    let evidence = evidence_for_seam(return_seam, &index);

    assert_eq!(evidence.reach.state, StageState::Yes);
    assert!(
        evidence
            .related_tests
            .iter()
            .any(|test| test.relation_reason == RelationReason::HelperOwnerCall),
        "expected target-affinity production wrapper owner-call relation, got {:?}",
        evidence.related_tests
    );
    assert_eq!(evidence.activate.state, StageState::Yes);
    assert!(
        evidence
            .activate
            .summary
            .contains("helper owner call for value-insensitive seam"),
        "activation summary should explain the target-affinity owner-call route: {}",
        evidence.activate.summary
    );
    assert!(
        evidence.observed_values.is_empty(),
        "target-affinity wrapper activation must not invent observed values: {:?}",
        evidence.observed_values
    );
    Ok(())
}

#[test]
fn given_value_insensitive_seam_when_multi_owner_wrapper_asserts_other_target_then_activation_stays_unknown()
-> Result<(), String> {
    let prod = PathBuf::from("src/labels.rs");
    let prod_src = r#"
pub const DEVICE_LABELS_EMPTY: &str = "empty";
pub const USER_LABELS_READY: &str = "ready";

pub fn device_label_status() -> &'static str {
    DEVICE_LABELS_EMPTY
}

pub fn user_label_status() -> &'static str {
    USER_LABELS_READY
}

pub fn exercise_statuses() -> (&'static str, &'static str) {
    (device_label_status(), user_label_status())
}
"#;
    let tests = PathBuf::from("tests/status_contract.rs");
    let tests_src = r#"
use labels::{exercise_statuses, USER_LABELS_READY};

#[test]
fn status_wrapper_observes_user_target() {
    let (_, user_status) = exercise_statuses();
    assert_eq!(user_status, USER_LABELS_READY);
}
"#;
    let index = index_from_files(&[(prod, prod_src), (tests, tests_src)])?;
    let seams = inventory_seams_from_index(&[PathBuf::from("src/labels.rs")], &index);
    let return_seam = seams
        .iter()
        .find(|s| {
            s.kind() == SeamKind::ReturnValue
                && s.owner().ends_with("::device_label_status")
                && s.expression().contains("DEVICE_LABELS_EMPTY")
        })
        .ok_or_else(|| "expected DEVICE_LABELS_EMPTY return_value seam".to_string())?;

    let evidence = evidence_for_seam(return_seam, &index);

    assert!(
        !evidence
            .related_tests
            .iter()
            .any(|test| test.relation_reason == RelationReason::HelperOwnerCall),
        "other target token must not prove device owner relation: {:?}",
        evidence.related_tests
    );
    assert_ne!(evidence.activate.state, StageState::Yes);
    assert!(
        evidence.observed_values.is_empty(),
        "other-target wrapper must not invent observed values: {:?}",
        evidence.observed_values
    );
    Ok(())
}

#[test]
fn given_full_evidence_when_owner_call_with_opaque_args_reaches_return_seam_then_activation_is_yes()
-> Result<(), String> {
    let prod = PathBuf::from("src/labels.rs");
    let prod_src = r#"
pub fn render_label(input: &str) -> String {
    input.to_string()
}
"#;
    let tests = PathBuf::from("tests/labels_tests.rs");
    let tests_src = r#"
fn fixture_label() -> String { "alpha".to_string() }

#[test]
fn render_label_matches_fixture() {
    let label = fixture_label();
    assert_eq!(render_label(&label), "alpha");
}
"#;
    let index = index_from_files(&[(prod, prod_src), (tests, tests_src)])?;
    let seams = inventory_seams_from_index(&[PathBuf::from("src/labels.rs")], &index);
    let return_seam = seams
        .iter()
        .find(|s| s.kind() == SeamKind::ReturnValue && s.expression().contains("to_string"))
        .ok_or_else(|| "expected to_string return_value seam".to_string())?;

    let evidence = evidence_for_seam(return_seam, &index);

    assert_eq!(evidence.reach.state, StageState::Yes);
    assert_eq!(evidence.activate.state, StageState::Yes);
    assert!(
        evidence.observed_values.is_empty(),
        "opaque direct-call arguments must not become observed values: {:?}",
        evidence.observed_values
    );
    assert!(
        evidence
            .activate
            .summary
            .contains("direct owner call for value-insensitive seam"),
        "activation summary should explain the value-insensitive owner-call route: {}",
        evidence.activate.summary
    );
    assert!(
        evidence.missing_discriminators.is_empty(),
        "value-insensitive direct owner calls must not create boundary debt"
    );
    Ok(())
}

#[test]
fn given_compact_evidence_when_import_affinity_has_no_owner_call_then_activation_is_unknown()
-> Result<(), String> {
    let prod = PathBuf::from("src/parse.rs");
    let prod_src = r#"
pub enum AuthError { RevokedToken, Expired }

pub fn parse(value: &str) -> Result<i32, AuthError> {
    if value.is_empty() {
        return Err(AuthError::RevokedToken);
    }
    Ok(0)
}
"#;
    let tests = PathBuf::from("tests/wrapper_tests.rs");
    let tests_src = r#"
fn helper() -> Result<i32, AuthError> { Err(AuthError::RevokedToken) }

#[test]
fn wrapper_rejects_empty() {
    use crate::parse;
    assert!(helper().is_err());
}
"#;
    let index = index_from_files(&[(prod, prod_src), (tests, tests_src)])?;
    let seams = inventory_seams_from_index(&[PathBuf::from("src/parse.rs")], &index);
    let error_seam = seams
        .iter()
        .find(|s| s.kind() == SeamKind::ErrorVariant)
        .ok_or_else(|| "expected error_variant seam".to_string())?;
    let context = CompactGripContext::new(&index);

    let related = find_related_tests_compact(error_seam, &context);
    assert_eq!(related.len(), 1);
    assert_eq!(related[0].test.name, "wrapper_rejects_empty");

    let evidence = compact_evidence_for_seam(error_seam, &context);
    assert_eq!(evidence.reach.state, StageState::Yes);
    assert_eq!(evidence.activate.state, StageState::Unknown);
    Ok(())
}

#[test]
fn given_compact_related_tests_when_more_than_limit_match_then_results_are_capped()
-> Result<(), String> {
    let prod = PathBuf::from("src/pricing.rs");
    let prod_src = r#"
pub fn discounted_total(amount: i32, threshold: i32) -> i32 {
    if amount >= threshold { amount - 10 } else { amount }
}
"#;
    let mut tests_src = String::new();
    for idx in 0..14 {
        tests_src.push_str(&format!(
            "#[test]\nfn direct_{idx:02}() {{ assert_eq!(discounted_total(100, 100), 90); }}\n"
        ));
    }
    let tests = PathBuf::from("tests/pricing_tests.rs");
    let index = index_from_files(&[(prod, prod_src), (tests, tests_src.as_str())])?;
    let seams = inventory_seams_from_index(&[PathBuf::from("src/pricing.rs")], &index);
    let predicate = seams
        .iter()
        .find(|s| s.kind() == SeamKind::PredicateBoundary)
        .ok_or_else(|| "expected predicate seam".to_string())?;
    let context = CompactGripContext::new(&index);

    let related = find_related_tests_compact(predicate, &context);

    assert_eq!(related.len(), COMPACT_RELATED_TEST_LIMIT);
    assert_eq!(related[0].test.name, "direct_00");
    assert_eq!(
        related[COMPACT_RELATED_TEST_LIMIT - 1].test.name,
        "direct_11"
    );
    Ok(())
}

#[test]
fn given_compact_import_affinity_when_owner_only_in_comment_or_string_then_no_relation_is_found()
-> Result<(), String> {
    let prod = PathBuf::from("src/parse.rs");
    let prod_src = r#"
pub enum AuthError { RevokedToken, Expired }

pub fn parse(value: &str) -> Result<i32, AuthError> {
    if value.is_empty() {
        return Err(AuthError::RevokedToken);
    }
    Ok(0)
}
"#;
    let tests = PathBuf::from("tests/noise_tests.rs");
    let tests_src = r#"
#[test]
fn wrapper_mentions_owner_only_in_non_code() {
    // use crate::parse;
    let _path = "crate::parse";
    assert!(helper().is_err());
}
"#;
    let index = index_from_files(&[(prod, prod_src), (tests, tests_src)])?;
    let seams = inventory_seams_from_index(&[PathBuf::from("src/parse.rs")], &index);
    let error_seam = seams
        .iter()
        .find(|s| s.kind() == SeamKind::ErrorVariant)
        .ok_or_else(|| "expected error_variant seam".to_string())?;
    let context = CompactGripContext::new(&index);

    let related = find_related_tests_compact(error_seam, &context);

    assert_eq!(related.len(), 0);
    Ok(())
}

// -- relation_reason / relation_confidence ranking ----------------
//
// Pins the ranking contract:
//   confidence (high first) → reason priority → file → name → line.
// Reason detection is exercised here through `find_related_tests`
// via `evidence_for_seam`. Each test fabricates a small index and
// inspects the first emitted RelatedTestGrip per seam.

fn first_grip_for(
    seam_file: &str,
    prod_src: &str,
    tests: &[(&str, &str)],
) -> Result<RelatedTestGrip, String> {
    let mut files: Vec<(PathBuf, &str)> = vec![(PathBuf::from(seam_file), prod_src)];
    for (path, src) in tests {
        files.push((PathBuf::from(*path), *src));
    }
    let index = index_from_files(&files)?;
    let seams = inventory_seams_from_index(&[PathBuf::from(seam_file)], &index);
    let predicate = seams
        .iter()
        .find(|s| s.kind() == SeamKind::PredicateBoundary)
        .ok_or_else(|| "predicate seam present".to_string())?;
    let evidence = evidence_for_seam(predicate, &index);
    evidence
        .related_tests
        .into_iter()
        .next()
        .ok_or_else(|| "at least one related test".to_string())
}

#[test]
fn given_direct_owner_call_and_same_file_match_when_related_tests_are_ranked_then_direct_call_is_first()
-> Result<(), String> {
    // One test in the same file (would match same_test_file) plus
    // one that calls the owner directly. Ranking must put the
    // direct-call test first.
    let prod_src = "pub fn discounted_total(amount: i32, threshold: i32) -> i32 \
                        { if amount >= threshold { amount - 10 } else { amount } }\n";
    // Test in pricing_tests.rs has the same file stem as src/pricing.rs.
    let same_file_only = (
        "tests/pricing_tests.rs",
        "#[test] fn pricing_smoke() { assert_eq!(1, 1); }\n",
    );
    // Test in unrelated.rs calls the owner directly.
    let direct = (
        "tests/unrelated.rs",
        "#[test] fn calls_owner() { assert_eq!(discounted_total(100, 100), 90); }\n",
    );

    let files: Vec<(PathBuf, &str)> = vec![
        (PathBuf::from("src/pricing.rs"), prod_src),
        (PathBuf::from(same_file_only.0), same_file_only.1),
        (PathBuf::from(direct.0), direct.1),
    ];
    let index = index_from_files(&files)?;
    let seams = inventory_seams_from_index(&[PathBuf::from("src/pricing.rs")], &index);
    let predicate = seams
        .iter()
        .find(|s| s.kind() == SeamKind::PredicateBoundary)
        .ok_or_else(|| "predicate seam present".to_string())?;
    let evidence = evidence_for_seam(predicate, &index);

    let first = evidence
        .related_tests
        .first()
        .ok_or_else(|| "at least one related test".to_string())?;
    let labels: Vec<_> = evidence
        .related_tests
        .iter()
        .map(|g| (g.test_name.clone(), g.relation_reason))
        .collect();
    assert_eq!(
        first.relation_reason,
        RelationReason::DirectOwnerCall,
        "direct owner call must outrank same-file affinity; got grips {labels:?}"
    );
    assert_eq!(first.relation_confidence, RelationConfidence::High);
    Ok(())
}

#[test]
fn given_owner_named_test_without_call_when_related_tests_are_ranked_then_confidence_is_medium()
-> Result<(), String> {
    // Test name embeds the owner name but does not call it and is
    // not in the same module / file. Should classify as
    // owner_named_test with medium confidence.
    let prod_src = "pub fn discounted_total(amount: i32, threshold: i32) -> i32 \
                        { if amount >= threshold { amount - 10 } else { amount } }\n";
    let test = (
        "tests/billing.rs",
        "#[test] fn discounted_total_smoke() { assert_eq!(1, 1); }\n",
    );
    let grip = first_grip_for("src/pricing.rs", prod_src, &[test])?;
    assert_eq!(grip.relation_reason, RelationReason::OwnerNamedTest);
    assert_eq!(grip.relation_confidence, RelationConfidence::Medium);
    Ok(())
}

#[test]
fn given_fixture_only_affinity_when_related_tests_are_ranked_then_confidence_is_low()
-> Result<(), String> {
    // Test calls a fixture-named helper in the owner's source file
    // but never the owner itself, and the test name does not embed
    // the owner. Should classify as fixture_owner_affinity with
    // exactly Low confidence (Opaque is reserved for cases the
    // detector does not yet emit).
    let prod_src = "pub fn discounted_total(amount: i32, threshold: i32) -> i32 \
                        { if amount >= threshold { amount - 10 } else { amount } }\n\
                        pub fn make_quote() -> i32 { 100 }\n";
    let test = (
        "tests/integration.rs",
        "#[test] fn quote_smoke() { let _ = make_quote(); assert!(true); }\n",
    );
    let grip = first_grip_for("src/pricing.rs", prod_src, &[test])?;
    assert_eq!(grip.relation_reason, RelationReason::FixtureOwnerAffinity);
    assert_eq!(grip.relation_confidence, RelationConfidence::Low);
    Ok(())
}

#[test]
fn given_assertion_target_affinity_uses_token_aware_match_not_substring() -> Result<(), String> {
    // The seam's required-discriminator description contains the
    // identifier `discount_threshold`. A test whose assertion uses
    // `discount_threshold_factor` (a longer identifier that contains
    // the discriminator string as a substring) must NOT be
    // classified as assertion_target_affinity — token-aware matching
    // requires whole-identifier hits, not substring contains.
    //
    // The test calls a different function (no direct_owner_call)
    // and lives in an unrelated file (no same_test_file/module),
    // and its name does not embed the owner.
    let prod_src = "pub fn discounted_total(amount: i32, discount_threshold: i32) -> i32 \
                        { if amount >= discount_threshold { amount - 10 } else { amount } }\n";
    let test = (
        "tests/billing.rs",
        "fn other() -> i32 { 0 }\n\
             #[test] fn smoke() { let discount_threshold_factor = 5; assert_eq!(other(), 0); let _ = discount_threshold_factor; }\n",
    );
    let files: Vec<(PathBuf, &str)> = vec![
        (PathBuf::from("src/pricing.rs"), prod_src),
        (PathBuf::from(test.0), test.1),
    ];
    let index = index_from_files(&files)?;
    let seams = inventory_seams_from_index(&[PathBuf::from("src/pricing.rs")], &index);
    let predicate = seams
        .iter()
        .find(|s| s.kind() == SeamKind::PredicateBoundary)
        .ok_or_else(|| "predicate seam present".to_string())?;
    let evidence = evidence_for_seam(predicate, &index);
    // The test must not appear as assertion_target_affinity. It is
    // OK for it to be excluded entirely (no reason fires) — the
    // contract is "do not falsely classify substring hits".
    for grip in &evidence.related_tests {
        assert_ne!(
            grip.relation_reason,
            RelationReason::AssertionTargetAffinity,
            "substring hit (`discount_threshold_factor`) must not match \
                 assertion_target_affinity; got {grip:?}"
        );
    }
    Ok(())
}

#[test]
fn given_call_presence_when_assertion_mentions_only_generic_argument_token_then_no_affinity()
-> Result<(), String> {
    // Live Lane 1 audit showed many call_presence limitations where
    // assertion-target affinity came from generic argument names
    // such as `path`, plus generic field/method targets such as
    // `description.clone()` and `is_empty()`, enum/match field
    // names such as `variant` and `arm`, and argument/context tokens
    // from full call expressions such as `source`, `current_owner`,
    // and `out`. That is not enough evidence that the test reaches
    // the owner or observes the call site.
    for token in [
        "arm",
        "path",
        "side_effect",
        "description",
        "field",
        "from",
        "is_empty",
        "byte",
        "bytes",
        "context",
        "probe",
        "sink",
        "u64",
        "variant",
    ] {
        assert!(
            !call_presence_assertion_affinity_token_is_specific_enough(token),
            "generic call-presence token must not create assertion-target affinity: {token}"
        );
    }
    assert!(call_presence_assertion_affinity_token_is_specific_enough(
        "zq_quote_target_token"
    ));
    let prod_src = "pub fn zq_call_presence_owner(path: &std::path::Path) -> String { \
                            zq_quote_target_token(&zq_render_target_token(path)) \
                        }\n\
                        fn zq_render_target_token(path: &std::path::Path) -> String { \
                            path.display().to_string() \
                        }\n\
                        fn zq_quote_target_token(input: &str) -> String { input.to_string() }\n\
                        pub fn zq_description_owner(description: &str) -> bool { \
                            description.is_empty() \
                        }\n\
                        pub fn zq_variant_owner(variant: &str, arm: &str) -> String { \
                            let _arm = arm.clone(); \
                            variant.to_string() \
                        }\n\
                        pub fn zq_collect_source_owner(file: &str, source: &str, current_owner: &str, out: &mut Vec<String>) { \
                            collect_source_facts_from_expr(file, source, current_owner, out); \
                        }\n\
                        fn collect_source_facts_from_expr(_file: &str, _source: &str, _current_owner: &str, _out: &mut Vec<String>) { \
                        }\n\
                        pub fn zq_byte_owner(bytes: &[u8]) -> Vec<u64> { \
                            bytes.iter().map(|byte| u64::from(*byte)).collect() \
                        }\n\
                        pub struct ZqContext { \
                            pub probe: String, \
                            pub class: String, \
                            pub evidence: String, \
                        }\n\
                        pub fn zq_probe_owner(context: &ZqContext) -> String { \
                            zq_missing_evidence(&context.probe, &context.class, &context.evidence) \
                        }\n\
                        fn zq_missing_evidence(probe: &str, class: &str, evidence: &str) -> String { \
                            format!(\"{probe}:{class}:{evidence}\") \
                        }\n";
    let test = (
        "tests/unrelated.rs",
        "#[test] fn unrelated_path_assertion() { \
                let path = \"target/ripr\"; \
                assert_eq!(path, \"target/ripr\"); \
            }\n\
             #[test] fn unrelated_side_effect_assertion() { \
                 let side_effect = \"target/ripr\"; \
                 assert_eq!(side_effect, \"target/ripr\"); \
             }\n\
             #[test] fn unrelated_description_assertion() { \
                 let description = \"target/ripr\"; \
                 assert!(!description.is_empty()); \
             }\n\
             #[test] fn unrelated_variant_assertion() { \
                 let variant = \"NotFound\"; \
                 let arm = \"fallback\"; \
                 assert_eq!(variant, \"NotFound\"); \
                 assert_eq!(arm, \"fallback\"); \
             }\n\
             #[test] fn unrelated_call_argument_assertion() { \
                 let source = \"src/lib.rs\"; \
                 let current_owner = \"current_owner\"; \
                 let out = \"out\"; \
                 assert_eq!(source, \"src/lib.rs\"); \
                 assert_eq!(current_owner, \"current_owner\"); \
                 assert_eq!(out, \"out\"); \
             }\n\
             #[test] fn unrelated_byte_assertion() { \
                 let bytes = vec![1u8, 2u8]; \
                 let byte = bytes[0]; \
                 assert_eq!(u64::from(byte), 1); \
             }\n\
             #[test] fn unrelated_probe_context_assertion() { \
                 let context = \"probe\"; \
                 let probe = \"alpha\"; \
                 let evidence = \"beta\"; \
                 assert_eq!(context, \"probe\"); \
                 assert_eq!(probe, \"alpha\"); \
                 assert_eq!(evidence, \"beta\"); \
             }\n",
    );
    let files: Vec<(PathBuf, &str)> = vec![
        (PathBuf::from("src/agent_paths.rs"), prod_src),
        (PathBuf::from(test.0), test.1),
    ];
    let index = index_from_files(&files)?;
    let seams = inventory_seams_from_index(&[PathBuf::from("src/agent_paths.rs")], &index);
    let call_presence = seams
        .iter()
        .find(|s| {
            s.kind() == SeamKind::CallPresence && s.expression().contains("zq_render_target_token")
        })
        .ok_or_else(|| "zq_render_target_token call_presence seam present".to_string())?;

    let evidence = evidence_for_seam(call_presence, &index);

    assert!(
        evidence.related_tests.is_empty(),
        "generic argument token `path` must not create assertion-target affinity; got {:?}",
        evidence.related_tests
    );

    let description_call_presence = seams
        .iter()
        .find(|s| s.kind() == SeamKind::CallPresence && s.expression().contains("is_empty"))
        .ok_or_else(|| "description is_empty call_presence seam present".to_string())?;
    let description_evidence = evidence_for_seam(description_call_presence, &index);
    assert!(
        description_evidence.related_tests.is_empty(),
        "generic field/method tokens `description` and `is_empty` must not create assertion-target affinity; got {:?}",
        description_evidence.related_tests
    );
    assert_eq!(description_evidence.reach.state, StageState::No);

    let variant_call_presence = seams
        .iter()
        .find(|s| {
            s.kind() == SeamKind::CallPresence && s.expression().contains("variant.to_string")
        })
        .ok_or_else(|| "variant to_string call_presence seam present".to_string())?;
    let variant_evidence = evidence_for_seam(variant_call_presence, &index);
    assert!(
        variant_evidence.related_tests.is_empty(),
        "generic enum/match tokens `variant` and `arm` must not create assertion-target affinity; got {:?}",
        variant_evidence.related_tests
    );
    assert_eq!(variant_evidence.reach.state, StageState::No);

    let arm_call_presence = seams
        .iter()
        .find(|s| s.kind() == SeamKind::CallPresence && s.expression().contains("arm.clone"))
        .ok_or_else(|| "arm clone call_presence seam present".to_string())?;
    let arm_evidence = evidence_for_seam(arm_call_presence, &index);
    assert!(
        arm_evidence.related_tests.is_empty(),
        "generic match-arm token `arm` must not create assertion-target affinity; got {:?}",
        arm_evidence.related_tests
    );
    assert_eq!(arm_evidence.reach.state, StageState::No);

    let collect_call_presence = seams
        .iter()
        .find(|s| {
            s.kind() == SeamKind::CallPresence
                && s.expression().contains("collect_source_facts_from_expr")
        })
        .ok_or_else(|| "collect_source_facts_from_expr call_presence seam present".to_string())?;
    let collect_evidence = evidence_for_seam(collect_call_presence, &index);
    assert!(
        collect_evidence.related_tests.is_empty(),
        "call-presence assertion-target affinity must not match argument/context tokens from the full call expression; got {:?}",
        collect_evidence.related_tests
    );
    assert_eq!(collect_evidence.reach.state, StageState::No);
    let byte_call_presence = seams
        .iter()
        .find(|s| s.kind() == SeamKind::CallPresence && s.expression().contains("u64::from"))
        .ok_or_else(|| "byte conversion call_presence seam present".to_string())?;
    let byte_evidence = evidence_for_seam(byte_call_presence, &index);
    assert!(
        byte_evidence.related_tests.is_empty(),
        "generic conversion tokens `u64`, `from`, `byte`, and `bytes` must not create assertion-target affinity; got {:?}",
        byte_evidence.related_tests
    );
    assert_eq!(byte_evidence.reach.state, StageState::No);
    let probe_call_presence = seams
        .iter()
        .find(|s| {
            s.kind() == SeamKind::CallPresence && s.expression().contains("zq_missing_evidence")
        })
        .ok_or_else(|| "probe context call_presence seam present".to_string())?;
    let probe_evidence = evidence_for_seam(probe_call_presence, &index);
    assert!(
        probe_evidence.related_tests.is_empty(),
        "generic context tokens `context`, `probe`, and `evidence` must not create assertion-target affinity; got {:?}",
        probe_evidence.related_tests
    );
    assert_eq!(probe_evidence.reach.state, StageState::No);
    assert_eq!(evidence.reach.state, StageState::No);
    Ok(())
}

#[test]
fn given_call_presence_when_direct_owner_call_has_mock_expectation_then_activation_is_yes()
-> Result<(), String> {
    let prod = PathBuf::from("src/receipt.rs");
    let prod_src = r#"
pub struct Recorder;

impl Recorder {
    pub fn send(&mut self, _value: &str) {}
}

pub fn emit_receipt(recorder: &mut Recorder, value: &str) {
    recorder.send(value);
}
"#;
    let tests = PathBuf::from("tests/receipt_tests.rs");
    let tests_src = r#"
fn receipt_payload() -> String { "sent".to_string() }

#[test]
fn emit_receipt_sends_value() {
    let mut recorder = Recorder;
    let value = receipt_payload();
    emit_receipt(&mut recorder, &value);
    mock_recorder.expect_send().times(1);
}
"#;
    let index = index_from_files(&[(prod, prod_src), (tests, tests_src)])?;
    let seams = inventory_seams_from_index(&[PathBuf::from("src/receipt.rs")], &index);
    let call_presence = seams
        .iter()
        .find(|s| {
            s.kind() == SeamKind::CallPresence
                && s.owner().ends_with("::emit_receipt")
                && s.expression().contains("send")
        })
        .ok_or_else(|| "expected send call_presence seam".to_string())?;

    let evidence = evidence_for_seam(call_presence, &index);

    assert_eq!(evidence.reach.state, StageState::Yes);
    assert_eq!(evidence.activate.state, StageState::Yes);
    assert!(
        evidence.related_tests.iter().any(|test| {
            test.relation_reason == RelationReason::DirectOwnerCall
                && test.oracle_kind == OracleKind::MockExpectation
        }),
        "expected direct owner-call related mock expectation, got {:?}",
        evidence.related_tests
    );
    assert!(
        evidence.observed_values.is_empty(),
        "call_presence activation must not invent observed values: {:?}",
        evidence.observed_values
    );
    assert!(
        evidence.missing_discriminators.is_empty(),
        "call_presence owner calls must not create boundary debt"
    );
    assert_eq!(evidence.propagate.state, StageState::Yes);
    assert_eq!(evidence.observe.state, StageState::Yes);
    assert_eq!(evidence.discriminate.state, StageState::Yes);
    Ok(())
}

#[test]
fn given_call_presence_when_direct_owner_call_uses_turbofish_then_activation_is_yes()
-> Result<(), String> {
    let prod = PathBuf::from("src/pipeline.rs");
    let prod_src = r#"
pub fn render_pipeline<T: ToString>(input: T) -> String {
    format_output(input.to_string())
}

fn format_output(input: String) -> String {
    input
}
"#;
    let tests = PathBuf::from("tests/pipeline_tests.rs");
    let tests_src = r#"
use pipeline::render_pipeline;

#[test]
fn direct_generic_owner_call_observes_call_presence() {
    let rendered = render_pipeline::<String>("alpha".to_string());
    assert_eq!(rendered, "alpha");
}
"#;
    let index = index_from_files(&[(prod, prod_src), (tests, tests_src)])?;
    let seams = inventory_seams_from_index(&[PathBuf::from("src/pipeline.rs")], &index);
    let call_presence = seams
        .iter()
        .find(|s| {
            s.kind() == SeamKind::CallPresence
                && s.owner().ends_with("::render_pipeline")
                && s.expression().contains("format_output")
        })
        .ok_or_else(|| "expected render_pipeline call_presence seam".to_string())?;

    let evidence = evidence_for_seam(call_presence, &index);

    assert_eq!(evidence.reach.state, StageState::Yes);
    assert_eq!(evidence.activate.state, StageState::Yes);
    assert!(
        evidence.related_tests.iter().any(|test| {
            test.relation_reason == RelationReason::DirectOwnerCall
                && test.test_name == "direct_generic_owner_call_observes_call_presence"
        }),
        "expected turbofish direct owner-call relation, got {:?}",
        evidence.related_tests
    );
    assert!(
        evidence.observed_values.is_empty(),
        "call_presence turbofish activation must not invent values: {:?}",
        evidence.observed_values
    );
    assert!(
        evidence.missing_discriminators.is_empty(),
        "call_presence turbofish owner calls must not create boundary debt"
    );
    assert!(
        evidence
            .activate
            .summary
            .contains("Observed direct owner call for value-insensitive seam"),
        "activation summary should explain direct owner-call route: {}",
        evidence.activate.summary
    );
    Ok(())
}

#[test]
fn given_call_presence_when_direct_owner_call_has_space_before_paren_then_activation_is_yes()
-> Result<(), String> {
    let prod = PathBuf::from("src/pipeline.rs");
    let prod_src = r#"
pub fn render_pipeline(input: String) -> String {
    format_output(input)
}

fn format_output(input: String) -> String {
    input
}
"#;
    let tests = PathBuf::from("tests/pipeline_tests.rs");
    let tests_src = r#"
use pipeline::render_pipeline;

#[test]
fn direct_spaced_owner_call_observes_call_presence() {
    let rendered = render_pipeline ("alpha".to_string());
    assert_eq!(rendered, "alpha");
}
"#;
    let index = index_from_files(&[(prod, prod_src), (tests, tests_src)])?;
    let seams = inventory_seams_from_index(&[PathBuf::from("src/pipeline.rs")], &index);
    let call_presence = seams
        .iter()
        .find(|s| {
            s.kind() == SeamKind::CallPresence
                && s.owner().ends_with("::render_pipeline")
                && s.expression().contains("format_output")
        })
        .ok_or_else(|| "expected render_pipeline call_presence seam".to_string())?;

    let evidence = evidence_for_seam(call_presence, &index);

    assert_eq!(evidence.reach.state, StageState::Yes);
    assert_eq!(evidence.activate.state, StageState::Yes);
    assert!(
        evidence.related_tests.iter().any(|test| {
            test.relation_reason == RelationReason::DirectOwnerCall
                && test.test_name == "direct_spaced_owner_call_observes_call_presence"
        }),
        "expected spaced direct owner-call relation, got {:?}",
        evidence.related_tests
    );
    assert!(
        evidence.observed_values.is_empty(),
        "call_presence spaced owner calls must not invent values: {:?}",
        evidence.observed_values
    );
    assert!(
        evidence.missing_discriminators.is_empty(),
        "call_presence spaced owner calls must not create boundary debt"
    );
    assert!(
        evidence
            .activate
            .summary
            .contains("Observed direct owner call for value-insensitive seam"),
        "activation summary should explain direct owner-call route: {}",
        evidence.activate.summary
    );
    Ok(())
}

#[test]
fn given_call_presence_when_assertion_mentions_short_specific_call_target_then_affinity_remains()
-> Result<(), String> {
    let prod = PathBuf::from("src/agent_paths.rs");
    let prod_src = "pub fn zq_call_presence_owner(path: &std::path::Path) -> String { \
                            zq_quote_target_token(&zq_render_target_token(path)) \
                        }\n\
                        fn zq_render_target_token(path: &std::path::Path) -> String { \
                            path.display().to_string() \
                        }\n\
                        fn zq_quote_target_token(input: &str) -> String { input.to_string() }\n";
    let tests = PathBuf::from("tests/target_affinity.rs");
    let tests_src = "#[test] fn unrelated_specific_target_assertion() { \
                let observed = \"zq_render_target_token\"; \
                assert_eq!(observed, \"zq_render_target_token\"); \
            }\n";
    let index = index_from_files(&[(prod, prod_src), (tests, tests_src)])?;
    let seams = inventory_seams_from_index(&[PathBuf::from("src/agent_paths.rs")], &index);
    let call_presence = seams
        .iter()
        .find(|s| {
            s.kind() == SeamKind::CallPresence && s.expression().contains("zq_render_target_token")
        })
        .ok_or_else(|| "zq_render_target_token call_presence seam present".to_string())?;

    let evidence = evidence_for_seam(call_presence, &index);

    assert_eq!(evidence.reach.state, StageState::Yes);
    assert!(
        evidence
            .related_tests
            .iter()
            .any(|test| test.relation_reason == RelationReason::AssertionTargetAffinity),
        "specific target token should remain eligible for assertion-target affinity; got {:?}",
        evidence.related_tests
    );
    assert!(
        evidence.related_tests.iter().any(|test| {
            test.relation_reason == RelationReason::AssertionTargetAffinity
                && test.relation_confidence == RelationConfidence::Medium
        }),
        "assertion-target affinity should stay medium confidence because it does not prove owner execution; got {:?}",
        evidence.related_tests
    );
    assert_eq!(evidence.activate.state, StageState::Unknown);
    assert!(
        evidence
            .activate
            .summary
            .contains("No direct owner call observed for value-insensitive seam"),
        "specific target affinity alone should remain a named owner-call limitation: {}",
        evidence.activate.summary
    );
    assert!(
        evidence.observed_values.is_empty(),
        "call_presence affinity-only activation must not invent observed values: {:?}",
        evidence.observed_values
    );
    Ok(())
}

#[test]
fn given_call_presence_when_same_file_wrapper_directly_calls_owner_then_activation_is_yes()
-> Result<(), String> {
    let source = PathBuf::from("src/pipeline.rs");
    let source_src = r#"
fn render_pipeline(input: &str) -> String {
    format_output(input)
}

fn exercise_pipeline() -> String {
    render_pipeline("alpha")
}

fn format_output(input: &str) -> String {
    input.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wrapper_exercises_pipeline() {
        let output = exercise_pipeline();
        assert_eq!(output, "alpha");
    }
}
"#;
    let index = index_from_files(&[(source, source_src)])?;
    let seams = inventory_seams_from_index(&[PathBuf::from("src/pipeline.rs")], &index);
    let call_presence = seams
        .iter()
        .find(|s| {
            s.kind() == SeamKind::CallPresence
                && s.owner().ends_with("::render_pipeline")
                && s.expression().contains("format_output")
        })
        .ok_or_else(|| "expected render_pipeline call_presence seam".to_string())?;

    let evidence = evidence_for_seam(call_presence, &index);

    assert_eq!(evidence.reach.state, StageState::Yes);
    assert_eq!(evidence.activate.state, StageState::Yes);
    assert!(
        evidence
            .related_tests
            .iter()
            .any(|test| test.relation_reason == RelationReason::HelperOwnerCall),
        "expected same-file wrapper owner-call relation, got {:?}",
        evidence.related_tests
    );
    assert!(
        evidence.observed_values.is_empty(),
        "call_presence wrapper activation must not invent values: {:?}",
        evidence.observed_values
    );
    assert!(
        evidence
            .activate
            .summary
            .contains("helper owner call for value-insensitive seam"),
        "activation summary should explain the wrapper owner-call route: {}",
        evidence.activate.summary
    );
    Ok(())
}

#[test]
fn given_call_presence_when_same_file_wrapper_calls_owner_method_then_activation_is_yes()
-> Result<(), String> {
    let source = PathBuf::from("src/pipeline.rs");
    let source_src = r#"
struct Pipeline;

impl Pipeline {
    fn render_pipeline(&self, input: &str) -> String {
        input.trim().to_string()
    }
}

fn exercise_pipeline(input: &str) -> String {
    let pipeline = Pipeline;
    pipeline.render_pipeline(input)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wrapper_exercises_pipeline_method() {
        let output = exercise_pipeline(" alpha ");
        assert_eq!(output, "alpha");
    }
}
"#;
    let index = index_from_files(&[(source, source_src)])?;
    let seams = inventory_seams_from_index(&[PathBuf::from("src/pipeline.rs")], &index);
    let call_presence = seams
        .iter()
        .find(|s| {
            s.kind() == SeamKind::CallPresence
                && s.owner().ends_with("::impl Pipeline::render_pipeline")
                && s.expression().contains("trim")
        })
        .ok_or_else(|| "expected render_pipeline method call_presence seam".to_string())?;

    let evidence = evidence_for_seam(call_presence, &index);

    assert_eq!(evidence.reach.state, StageState::Yes);
    assert_eq!(evidence.activate.state, StageState::Yes);
    assert!(
        evidence
            .related_tests
            .iter()
            .any(|test| test.relation_reason == RelationReason::HelperOwnerCall),
        "expected same-file receiver-method wrapper owner-call relation, got {:?}",
        evidence.related_tests
    );
    assert!(
        evidence.observed_values.is_empty(),
        "method wrapper activation must not invent observed values: {:?}",
        evidence.observed_values
    );
    assert!(
        evidence
            .activate
            .summary
            .contains("helper owner call for value-insensitive seam"),
        "activation summary should explain the method wrapper owner-call route: {}",
        evidence.activate.summary
    );
    Ok(())
}

#[test]
fn given_call_presence_when_same_file_wrapper_uses_dynamic_method_receiver_then_activation_stays_unknown()
-> Result<(), String> {
    let source = PathBuf::from("src/pipeline.rs");
    let source_src = r#"
struct Pipeline;

impl Pipeline {
    fn render_pipeline(&self, input: &str) -> String {
        input.trim().to_string()
    }
}

fn pipeline_factory() -> Pipeline {
    Pipeline
}

fn exercise_pipeline(input: &str) -> String {
    pipeline_factory().render_pipeline(input)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wrapper_uses_dynamic_receiver() {
        let output = exercise_pipeline(" alpha ");
        assert_eq!(output, "alpha");
    }
}
"#;
    let index = index_from_files(&[(source, source_src)])?;
    let seams = inventory_seams_from_index(&[PathBuf::from("src/pipeline.rs")], &index);
    let call_presence = seams
        .iter()
        .find(|s| {
            s.kind() == SeamKind::CallPresence
                && s.owner().ends_with("::impl Pipeline::render_pipeline")
                && s.expression().contains("trim")
        })
        .ok_or_else(|| "expected render_pipeline method call_presence seam".to_string())?;

    let evidence = evidence_for_seam(call_presence, &index);

    assert_eq!(evidence.reach.state, StageState::Yes);
    assert_eq!(evidence.activate.state, StageState::Unknown);
    assert!(
        !evidence
            .related_tests
            .iter()
            .any(|test| test.relation_reason == RelationReason::HelperOwnerCall),
        "dynamic receiver method call must not get helper-owner relation: {:?}",
        evidence.related_tests
    );
    assert!(
        evidence.observed_values.is_empty(),
        "dynamic receiver method route must not invent observed values: {:?}",
        evidence.observed_values
    );
    assert!(
        evidence
            .activate
            .summary
            .contains("No direct owner call observed for value-insensitive seam"),
        "activation summary should keep owner-call limitation, got {}",
        evidence.activate.summary
    );
    Ok(())
}

#[test]
fn given_call_presence_when_test_local_helper_directly_calls_owner_then_activation_is_yes()
-> Result<(), String> {
    let prod = PathBuf::from("src/pipeline.rs");
    let prod_src = r#"
pub fn render_pipeline(input: &str) -> String {
    format_output(input)
}

fn format_output(input: &str) -> String {
    input.to_string()
}
"#;
    let tests = PathBuf::from("tests/pipeline_tests.rs");
    let tests_src = r#"
use pipeline::render_pipeline;

fn exercise_pipeline() -> String {
    render_pipeline("alpha")
}

#[test]
fn helper_exercises_pipeline() {
    let output = exercise_pipeline();
    assert_eq!(output, "alpha");
}
"#;
    let index = index_from_files(&[(prod, prod_src), (tests, tests_src)])?;
    let seams = inventory_seams_from_index(&[PathBuf::from("src/pipeline.rs")], &index);
    let call_presence = seams
        .iter()
        .find(|s| {
            s.kind() == SeamKind::CallPresence
                && s.owner().ends_with("::render_pipeline")
                && s.expression().contains("format_output")
        })
        .ok_or_else(|| "expected render_pipeline call_presence seam".to_string())?;

    let evidence = evidence_for_seam(call_presence, &index);

    assert_eq!(evidence.reach.state, StageState::Yes);
    assert_eq!(evidence.activate.state, StageState::Yes);
    assert!(
        evidence
            .related_tests
            .iter()
            .any(|test| test.relation_reason == RelationReason::HelperOwnerCall),
        "expected test-local one-hop helper owner-call relation, got {:?}",
        evidence.related_tests
    );
    assert!(
        evidence.observed_values.is_empty(),
        "call_presence test-local helper activation must not invent values: {:?}",
        evidence.observed_values
    );
    assert!(
        evidence
            .activate
            .summary
            .contains("helper owner call for value-insensitive seam"),
        "activation summary should explain the test-local helper owner-call route: {}",
        evidence.activate.summary
    );
    Ok(())
}

#[test]
fn given_call_presence_when_helper_calls_owner_then_logs_then_activation_is_yes()
-> Result<(), String> {
    let prod = PathBuf::from("src/pipeline.rs");
    let prod_src = r#"
pub fn render_pipeline(input: &str) -> String {
    format_output(input)
}

fn format_output(input: &str) -> String {
    input.to_string()
}
"#;
    let tests = PathBuf::from("tests/pipeline_tests.rs");
    let tests_src = r#"
use pipeline::render_pipeline;

fn note() {}

fn exercise_pipeline() -> String {
    let output = render_pipeline("alpha");
    note();
    output
}

#[test]
fn helper_exercises_pipeline() {
    let output = exercise_pipeline();
    assert_eq!(output, "alpha");
}
"#;
    let index = index_from_files(&[(prod, prod_src), (tests, tests_src)])?;
    let seams = inventory_seams_from_index(&[PathBuf::from("src/pipeline.rs")], &index);
    let call_presence = seams
        .iter()
        .find(|s| {
            s.kind() == SeamKind::CallPresence
                && s.owner().ends_with("::render_pipeline")
                && s.expression().contains("format_output")
        })
        .ok_or_else(|| "expected render_pipeline call_presence seam".to_string())?;

    let evidence = evidence_for_seam(call_presence, &index);

    assert_eq!(evidence.reach.state, StageState::Yes);
    assert_eq!(evidence.activate.state, StageState::Yes);
    assert!(
        evidence
            .related_tests
            .iter()
            .any(|test| test.relation_reason == RelationReason::HelperOwnerCall),
        "expected helper owner-call relation through direct owner call statement, got {:?}",
        evidence.related_tests
    );
    assert!(
        evidence.observed_values.is_empty(),
        "helper activation must not invent observed values: {:?}",
        evidence.observed_values
    );
    Ok(())
}

#[test]
fn given_call_presence_when_integration_test_calls_production_wrapper_then_activation_is_yes()
-> Result<(), String> {
    let prod = PathBuf::from("src/pipeline.rs");
    let prod_src = r#"
pub fn render_pipeline(input: &str) -> String {
    format_output(input)
}

pub fn exercise_pipeline() -> String {
    render_pipeline("alpha")
}

fn format_output(input: &str) -> String {
    input.to_string()
}
"#;
    let tests = PathBuf::from("tests/pipeline_tests.rs");
    let tests_src = r#"
use pipeline::exercise_pipeline;

#[test]
fn production_wrapper_exercises_pipeline() {
    let format_output = exercise_pipeline();
    assert_eq!(format_output, "alpha");
}
"#;
    let index = index_from_files(&[(prod, prod_src), (tests, tests_src)])?;
    let seams = inventory_seams_from_index(&[PathBuf::from("src/pipeline.rs")], &index);
    let call_presence = seams
        .iter()
        .find(|s| {
            s.kind() == SeamKind::CallPresence
                && s.owner().ends_with("::render_pipeline")
                && s.expression().contains("format_output")
        })
        .ok_or_else(|| "expected render_pipeline call_presence seam".to_string())?;

    let evidence = evidence_for_seam(call_presence, &index);

    assert_eq!(evidence.reach.state, StageState::Yes);
    assert_eq!(evidence.activate.state, StageState::Yes);
    assert!(
        evidence
            .related_tests
            .iter()
            .any(|test| test.relation_reason == RelationReason::HelperOwnerCall),
        "expected production wrapper owner-call relation, got {:?}",
        evidence.related_tests
    );
    assert!(
        evidence.observed_values.is_empty(),
        "production wrapper activation must not invent values: {:?}",
        evidence.observed_values
    );
    Ok(())
}

#[test]
fn given_call_presence_when_integration_test_calls_two_hop_production_wrapper_then_activation_is_yes()
-> Result<(), String> {
    let prod = PathBuf::from("src/pipeline.rs");
    let prod_src = r#"
pub fn render_pipeline(input: &str) -> String {
    format_output(input)
}

pub fn build_pipeline(input: &str) -> String {
    render_pipeline(input)
}

pub fn exercise_pipeline(input: &str) -> String {
    build_pipeline(input)
}

fn format_output(input: &str) -> String {
    input.to_string()
}
"#;
    let tests = PathBuf::from("tests/pipeline_tests.rs");
    let tests_src = r#"
use pipeline::exercise_pipeline;

#[test]
fn production_wrapper_chain_exercises_pipeline() {
    let format_output = exercise_pipeline("alpha");
    assert_eq!(format_output, "alpha");
}
"#;
    let index = index_from_files(&[(prod, prod_src), (tests, tests_src)])?;
    let seams = inventory_seams_from_index(&[PathBuf::from("src/pipeline.rs")], &index);
    let call_presence = seams
        .iter()
        .find(|s| {
            s.kind() == SeamKind::CallPresence
                && s.owner().ends_with("::render_pipeline")
                && s.expression().contains("format_output")
        })
        .ok_or_else(|| "expected render_pipeline call_presence seam".to_string())?;

    let evidence = evidence_for_seam(call_presence, &index);

    assert_eq!(evidence.reach.state, StageState::Yes);
    assert_eq!(evidence.activate.state, StageState::Yes);
    assert!(
        evidence
            .related_tests
            .iter()
            .any(|test| test.relation_reason == RelationReason::HelperOwnerCall),
        "expected bounded production wrapper owner-call relation, got {:?}",
        evidence.related_tests
    );
    assert!(
        evidence.observed_values.is_empty(),
        "bounded production wrapper activation must not invent values: {:?}",
        evidence.observed_values
    );
    assert!(
        evidence
            .activate
            .summary
            .contains("helper owner call for value-insensitive seam"),
        "activation summary should explain the bounded helper owner-call route: {}",
        evidence.activate.summary
    );
    Ok(())
}

#[test]
fn given_call_presence_when_two_hop_production_wrapper_reaches_multiple_owners_then_activation_stays_unknown()
-> Result<(), String> {
    let prod = PathBuf::from("src/loop_commands.rs");
    let prod_src = r#"
pub fn quote_arg(value: &str) -> String {
    value.replace(' ', "\\ ")
}

pub fn normalize_arg(value: &str) -> String {
    value.trim().to_string()
}

pub fn quote_root(root: &str) -> String {
    quote_arg(root)
}

pub fn format_command(root: &str, packet: &str) -> String {
    let root_arg = quote_root(root);
    let packet_arg = normalize_arg(packet);
    format!("ripr start --root {root_arg} --packet {packet_arg}")
}
"#;
    let tests = PathBuf::from("tests/loop_commands_tests.rs");
    let tests_src = r#"
use loop_commands::format_command;

#[test]
fn command_formats_dynamic_args() {
    let command = format_command("tmp root", "gap 1");
    assert_eq!(
        command,
        "ripr start --root tmp\\ root --packet gap 1"
    );
}
"#;
    let index = index_from_files(&[(prod, prod_src), (tests, tests_src)])?;
    let seams = inventory_seams_from_index(&[PathBuf::from("src/loop_commands.rs")], &index);
    let call_presence = seams
        .iter()
        .find(|s| {
            s.kind() == SeamKind::CallPresence
                && s.owner().ends_with("::quote_arg")
                && s.expression().contains("replace")
        })
        .ok_or_else(|| "expected quote_arg call_presence seam".to_string())?;

    let evidence = evidence_for_seam(call_presence, &index);

    assert_eq!(evidence.reach.state, StageState::Yes);
    assert_eq!(evidence.activate.state, StageState::Unknown);
    assert!(
        !evidence
            .related_tests
            .iter()
            .any(|test| test.relation_reason == RelationReason::HelperOwnerCall),
        "mixed-owner production graph must not get helper-owner relation: {:?}",
        evidence.related_tests
    );
    assert!(
        evidence.observed_values.is_empty(),
        "mixed-owner production graph must not invent observed values: {:?}",
        evidence.observed_values
    );
    Ok(())
}

#[test]
fn given_call_presence_when_production_wrapper_calls_same_owner_multiple_times_then_activation_is_yes()
-> Result<(), String> {
    let prod = PathBuf::from("src/loop_commands.rs");
    let prod_src = r#"
pub fn shell_arg(value: &str) -> String {
    value.replace(' ', "\\ ")
}

pub fn agent_start_command(root: &str, packet: &str) -> String {
    let root_arg = shell_arg(root);
    let packet_arg = shell_arg(packet);
    format!("ripr agent start --root {root_arg} --packet {packet_arg}")
}
"#;
    let tests = PathBuf::from("tests/loop_commands_tests.rs");
    let tests_src = r#"
use loop_commands::agent_start_command;

#[test]
fn command_quotes_each_dynamic_arg() {
    let command = agent_start_command("tmp root", "gap 1");
    assert_eq!(
        command,
        "ripr agent start --root tmp\\ root --packet gap\\ 1"
    );
}
"#;
    let index = index_from_files(&[(prod, prod_src), (tests, tests_src)])?;
    let seams = inventory_seams_from_index(&[PathBuf::from("src/loop_commands.rs")], &index);
    let call_presence = seams
        .iter()
        .find(|s| {
            s.kind() == SeamKind::CallPresence
                && s.owner().ends_with("::shell_arg")
                && s.expression().contains("replace")
        })
        .ok_or_else(|| "expected shell_arg call_presence seam".to_string())?;

    let evidence = evidence_for_seam(call_presence, &index);

    assert_eq!(evidence.reach.state, StageState::Yes);
    assert_eq!(evidence.activate.state, StageState::Yes);
    assert!(
        evidence
            .related_tests
            .iter()
            .any(|test| test.relation_reason == RelationReason::HelperOwnerCall),
        "expected same-owner production wrapper relation, got {:?}",
        evidence.related_tests
    );
    assert!(
        evidence.observed_values.is_empty(),
        "same-owner production wrapper activation must not invent values: {:?}",
        evidence.observed_values
    );
    Ok(())
}

#[test]
fn given_call_presence_when_production_wrapper_calls_multiple_owners_then_activation_stays_unknown()
-> Result<(), String> {
    let prod = PathBuf::from("src/loop_commands.rs");
    let prod_src = r#"
pub fn quote_arg(value: &str) -> String {
    value.replace(' ', "\\ ")
}

pub fn normalize_arg(value: &str) -> String {
    value.trim().to_string()
}

pub fn agent_start_command(root: &str, packet: &str) -> String {
    let root_arg = quote_arg(root);
    let packet_arg = normalize_arg(packet);
    format!("ripr agent start --root {root_arg} --packet {packet_arg}")
}
"#;
    let tests = PathBuf::from("tests/loop_commands_tests.rs");
    let tests_src = r#"
use loop_commands::agent_start_command;

#[test]
fn command_formats_dynamic_args() {
    let command = agent_start_command("tmp root", "gap 1");
    assert_eq!(
        command,
        "ripr agent start --root tmp\\ root --packet gap 1"
    );
}
"#;
    let index = index_from_files(&[(prod, prod_src), (tests, tests_src)])?;
    let seams = inventory_seams_from_index(&[PathBuf::from("src/loop_commands.rs")], &index);
    let call_presence = seams
        .iter()
        .find(|s| {
            s.kind() == SeamKind::CallPresence
                && s.owner().ends_with("::quote_arg")
                && s.expression().contains("replace")
        })
        .ok_or_else(|| "expected quote_arg call_presence seam".to_string())?;

    let evidence = evidence_for_seam(call_presence, &index);

    assert_eq!(evidence.reach.state, StageState::Yes);
    assert_eq!(evidence.activate.state, StageState::Unknown);
    assert!(
        !evidence
            .related_tests
            .iter()
            .any(|test| test.relation_reason == RelationReason::HelperOwnerCall),
        "mixed-owner production wrapper must not get helper-owner relation: {:?}",
        evidence.related_tests
    );
    assert!(
        evidence.observed_values.is_empty(),
        "mixed-owner production wrapper must not invent observed values: {:?}",
        evidence.observed_values
    );
    Ok(())
}

#[test]
fn given_call_presence_when_multi_owner_production_wrapper_has_target_affinity_then_activation_is_yes()
-> Result<(), String> {
    let prod = PathBuf::from("src/pipeline.rs");
    let prod_src = r#"
pub fn render_pipeline(input: &str) -> String {
    format_output(input)
}

pub fn render_report(input: &str) -> String {
    format_report(input)
}

pub fn exercise_both(input: &str) -> String {
    let pipeline = render_pipeline(input);
    let report = render_report(input);
    format!("format_output={pipeline};format_report={report}")
}

fn format_output(input: &str) -> String {
    input.to_string()
}

fn format_report(input: &str) -> String {
    input.to_string()
}
"#;
    let tests = PathBuf::from("tests/pipeline_tests.rs");
    let tests_src = r#"
use pipeline::exercise_both;

#[test]
fn multi_owner_wrapper_observes_pipeline_call_target() {
    let rendered = exercise_both("alpha");
    assert!(rendered.contains("format_output"));
}
"#;
    let index = index_from_files(&[(prod, prod_src), (tests, tests_src)])?;
    let seams = inventory_seams_from_index(&[PathBuf::from("src/pipeline.rs")], &index);
    let call_presence = seams
        .iter()
        .find(|s| {
            s.kind() == SeamKind::CallPresence
                && s.owner().ends_with("::render_pipeline")
                && s.expression().contains("format_output")
        })
        .ok_or_else(|| "expected render_pipeline call_presence seam".to_string())?;

    let evidence = evidence_for_seam(call_presence, &index);

    assert_eq!(evidence.reach.state, StageState::Yes);
    assert!(
        evidence
            .related_tests
            .iter()
            .any(|test| test.relation_reason == RelationReason::HelperOwnerCall),
        "expected target-affinity production wrapper owner-call relation, got {:?}",
        evidence.related_tests
    );
    assert_eq!(evidence.activate.state, StageState::Yes);
    assert!(
        evidence.observed_values.is_empty(),
        "target-affinity call_presence wrapper activation must not invent values: {:?}",
        evidence.observed_values
    );
    Ok(())
}

#[test]
fn given_call_presence_when_unit_test_calls_same_file_target_affinity_wrapper_then_activation_is_yes()
-> Result<(), String> {
    let source = PathBuf::from("src/pipeline.rs");
    let source_src = r#"
pub fn render_pipeline(input: &str) -> String {
    format_output(input)
}

pub fn render_report(input: &str) -> String {
    format_report(input)
}

pub fn exercise_both(input: &str) -> String {
    let pipeline = render_pipeline(input);
    let report = render_report(input);
    format!("format_output={pipeline};format_report={report}")
}

fn format_output(input: &str) -> String {
    input.to_string()
}

fn format_report(input: &str) -> String {
    input.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unit_wrapper_observes_pipeline_call_target() {
        let rendered = exercise_both("alpha");
        assert!(rendered.contains("format_output"));
    }
}
"#;
    let index = index_from_files(&[(source, source_src)])?;
    let seams = inventory_seams_from_index(&[PathBuf::from("src/pipeline.rs")], &index);
    let call_presence = seams
        .iter()
        .find(|s| {
            s.kind() == SeamKind::CallPresence
                && s.owner().ends_with("::render_pipeline")
                && s.expression().contains("format_output")
        })
        .ok_or_else(|| "expected render_pipeline call_presence seam".to_string())?;

    let evidence = evidence_for_seam(call_presence, &index);

    assert_eq!(evidence.reach.state, StageState::Yes);
    assert!(
        evidence
            .related_tests
            .iter()
            .any(|test| test.relation_reason == RelationReason::HelperOwnerCall),
        "expected same-file target-affinity wrapper owner-call relation, got {:?}",
        evidence.related_tests
    );
    assert_eq!(evidence.activate.state, StageState::Yes);
    assert!(
        evidence.observed_values.is_empty(),
        "same-file target-affinity wrapper activation must not invent values: {:?}",
        evidence.observed_values
    );
    Ok(())
}

#[test]
fn given_call_presence_when_multi_owner_wrapper_asserts_other_target_then_activation_stays_unknown()
-> Result<(), String> {
    let prod = PathBuf::from("src/pipeline.rs");
    let prod_src = r#"
pub fn render_pipeline(input: &str) -> String {
    format_output(input)
}

pub fn render_report(input: &str) -> String {
    format_report(input)
}

pub fn exercise_both(input: &str) -> String {
    let pipeline = render_pipeline(input);
    let report = render_report(input);
    format!("format_output={pipeline};format_report={report}")
}

fn format_output(input: &str) -> String {
    input.to_string()
}

fn format_report(input: &str) -> String {
    input.to_string()
}
"#;
    let tests = PathBuf::from("tests/pipeline_tests.rs");
    let tests_src = r#"
use pipeline::exercise_both;

#[test]
fn multi_owner_wrapper_observes_report_call_target() {
    let rendered = exercise_both("alpha");
    assert!(rendered.contains("format_report"));
}
"#;
    let index = index_from_files(&[(prod, prod_src), (tests, tests_src)])?;
    let seams = inventory_seams_from_index(&[PathBuf::from("src/pipeline.rs")], &index);
    let call_presence = seams
        .iter()
        .find(|s| {
            s.kind() == SeamKind::CallPresence
                && s.owner().ends_with("::render_pipeline")
                && s.expression().contains("format_output")
        })
        .ok_or_else(|| "expected render_pipeline call_presence seam".to_string())?;

    let evidence = evidence_for_seam(call_presence, &index);

    assert!(
        !evidence
            .related_tests
            .iter()
            .any(|test| test.relation_reason == RelationReason::HelperOwnerCall),
        "other target token must not prove pipeline owner relation: {:?}",
        evidence.related_tests
    );
    assert_ne!(evidence.activate.state, StageState::Yes);
    assert!(
        evidence.observed_values.is_empty(),
        "other-target wrapper must not invent observed values: {:?}",
        evidence.observed_values
    );
    Ok(())
}

#[test]
fn given_call_presence_when_test_local_fanout_helper_asserts_other_target_then_activation_stays_unknown()
-> Result<(), String> {
    let prod = PathBuf::from("src/pipeline.rs");
    let prod_src = r#"
pub fn render_pipeline(input: &str) -> String {
    format_output(input)
}

pub fn render_report(input: &str) -> String {
    format_report(input)
}

fn format_output(input: &str) -> String {
    input.to_string()
}

fn format_report(input: &str) -> String {
    input.to_string()
}
"#;
    let tests = PathBuf::from("tests/pipeline_tests.rs");
    let tests_src = r#"
use pipeline::{render_pipeline, render_report};

fn exercise_both(input: &str) -> String {
    let pipeline = render_pipeline(input);
    let report = render_report(input);
    format!("format_output={pipeline};format_report={report}")
}

#[test]
fn test_local_fanout_observes_report_call_target() {
    let rendered = exercise_both("alpha");
    assert!(rendered.contains("format_report"));
}
"#;
    let index = index_from_files(&[(prod, prod_src), (tests, tests_src)])?;
    let seams = inventory_seams_from_index(&[PathBuf::from("src/pipeline.rs")], &index);
    let call_presence = seams
        .iter()
        .find(|s| {
            s.kind() == SeamKind::CallPresence
                && s.owner().ends_with("::render_pipeline")
                && s.expression().contains("format_output")
        })
        .ok_or_else(|| "expected render_pipeline call_presence seam".to_string())?;

    let evidence = evidence_for_seam(call_presence, &index);

    assert!(
        !evidence
            .related_tests
            .iter()
            .any(|test| test.relation_reason == RelationReason::HelperOwnerCall),
        "test-local fanout helper must not bypass target affinity: {:?}",
        evidence.related_tests
    );
    assert_ne!(evidence.activate.state, StageState::Yes);
    assert!(
        evidence.observed_values.is_empty(),
        "test-local fanout helper must not invent observed values: {:?}",
        evidence.observed_values
    );
    Ok(())
}

#[test]
fn given_call_presence_when_production_wrapper_name_is_ambiguous_then_activation_stays_unknown()
-> Result<(), String> {
    let pipeline = PathBuf::from("src/pipeline.rs");
    let pipeline_src = r#"
pub fn render_pipeline(input: &str) -> String {
    format_output(input)
}

pub fn exercise_pipeline() -> String {
    render_pipeline("alpha")
}

fn format_output(input: &str) -> String {
    input.to_string()
}
"#;
    let report = PathBuf::from("src/report.rs");
    let report_src = r#"
pub fn render_report(input: &str) -> String {
    format_report(input)
}

pub fn exercise_pipeline() -> String {
    render_report("beta")
}

fn format_report(input: &str) -> String {
    input.to_string()
}
"#;
    let tests = PathBuf::from("tests/pipeline_tests.rs");
    let tests_src = r#"
use pipeline::exercise_pipeline;

#[test]
fn ambiguous_production_wrapper_keeps_pipeline_limited() {
    let format_output = exercise_pipeline();
    assert_eq!(format_output, "alpha");
}
"#;
    let index = index_from_files(&[
        (pipeline, pipeline_src),
        (report, report_src),
        (tests, tests_src),
    ])?;
    let seams = inventory_seams_from_index(&[PathBuf::from("src/pipeline.rs")], &index);
    let call_presence = seams
        .iter()
        .find(|s| {
            s.kind() == SeamKind::CallPresence
                && s.owner().ends_with("::render_pipeline")
                && s.expression().contains("format_output")
        })
        .ok_or_else(|| "expected render_pipeline call_presence seam".to_string())?;

    let evidence = evidence_for_seam(call_presence, &index);

    assert_eq!(evidence.reach.state, StageState::Yes);
    assert_eq!(evidence.activate.state, StageState::Unknown);
    assert!(
        !evidence
            .related_tests
            .iter()
            .any(|test| test.relation_reason == RelationReason::HelperOwnerCall),
        "ambiguous production wrapper name must not get helper-owner relation: {:?}",
        evidence.related_tests
    );
    Ok(())
}

#[test]
fn given_call_presence_when_module_qualified_ambiguous_production_wrapper_has_target_affinity_then_activation_is_yes()
-> Result<(), String> {
    let pipeline = PathBuf::from("src/pipeline.rs");
    let pipeline_src = r#"
pub fn render_pipeline(input: &str) -> String {
    format_output(input)
}

pub fn exercise_pipeline() -> String {
    render_pipeline("alpha")
}

fn format_output(input: &str) -> String {
    input.to_string()
}
"#;
    let report = PathBuf::from("src/report.rs");
    let report_src = r#"
pub fn render_report(input: &str) -> String {
    format_report(input)
}

pub fn exercise_pipeline() -> String {
    render_report("beta")
}

fn format_report(input: &str) -> String {
    input.to_string()
}
"#;
    let tests = PathBuf::from("tests/contract_tests.rs");
    let tests_src = r#"
#[test]
fn qualified_wrapper_observes_pipeline_call_target() {
    let format_output = pipeline::exercise_pipeline();
    assert_eq!(format_output, "alpha");
}
"#;
    let index = index_from_files(&[
        (pipeline, pipeline_src),
        (report, report_src),
        (tests, tests_src),
    ])?;
    let seams = inventory_seams_from_index(&[PathBuf::from("src/pipeline.rs")], &index);
    let call_presence = seams
        .iter()
        .find(|s| {
            s.kind() == SeamKind::CallPresence
                && s.owner().ends_with("::render_pipeline")
                && s.expression().contains("format_output")
        })
        .ok_or_else(|| "expected render_pipeline call_presence seam".to_string())?;

    let evidence = evidence_for_seam(call_presence, &index);

    assert_eq!(evidence.reach.state, StageState::Yes);
    assert!(
        evidence
            .related_tests
            .iter()
            .any(|test| test.relation_reason == RelationReason::HelperOwnerCall),
        "expected module-qualified target-affinity wrapper owner-call relation, got {:?}",
        evidence.related_tests
    );
    assert_eq!(evidence.activate.state, StageState::Yes);
    assert!(
        evidence.observed_values.is_empty(),
        "qualified target-affinity wrapper activation must not invent values: {:?}",
        evidence.observed_values
    );
    Ok(())
}

#[test]
fn given_call_presence_when_aliased_module_wrapper_has_target_affinity_then_activation_is_yes()
-> Result<(), String> {
    let pipeline = PathBuf::from("src/pipeline.rs");
    let pipeline_src = r#"
pub fn render_pipeline(input: &str) -> String {
    format_output(input)
}

pub fn exercise_pipeline() -> String {
    render_pipeline("alpha")
}

fn format_output(input: &str) -> String {
    input.to_string()
}
"#;
    let report = PathBuf::from("src/report.rs");
    let report_src = r#"
pub fn render_report(input: &str) -> String {
    format_report(input)
}

pub fn exercise_pipeline() -> String {
    render_report("beta")
}

fn format_report(input: &str) -> String {
    input.to_string()
}
"#;
    let tests = PathBuf::from("tests/contract_tests.rs");
    let tests_src = r#"
use crate::pipeline as pipe;

#[test]
fn aliased_wrapper_observes_pipeline_call_target() {
    let format_output = pipe::exercise_pipeline();
    assert_eq!(format_output, "alpha");
}
"#;
    let index = index_from_files(&[
        (pipeline, pipeline_src),
        (report, report_src),
        (tests, tests_src),
    ])?;
    let seams = inventory_seams_from_index(&[PathBuf::from("src/pipeline.rs")], &index);
    let call_presence = seams
        .iter()
        .find(|s| {
            s.kind() == SeamKind::CallPresence
                && s.owner().ends_with("::render_pipeline")
                && s.expression().contains("format_output")
        })
        .ok_or_else(|| "expected render_pipeline call_presence seam".to_string())?;

    let evidence = evidence_for_seam(call_presence, &index);

    assert_eq!(evidence.reach.state, StageState::Yes);
    assert!(
        evidence
            .related_tests
            .iter()
            .any(|test| test.relation_reason == RelationReason::HelperOwnerCall),
        "expected aliased module target-affinity wrapper owner-call relation, got {:?}",
        evidence.related_tests
    );
    assert_eq!(evidence.activate.state, StageState::Yes);
    assert!(
        evidence.observed_values.is_empty(),
        "aliased target-affinity wrapper activation must not invent values: {:?}",
        evidence.observed_values
    );
    Ok(())
}

#[test]
fn given_call_presence_when_bare_aliased_module_wrapper_has_target_affinity_then_activation_stays_unknown()
-> Result<(), String> {
    let pipeline = PathBuf::from("src/pipeline.rs");
    let pipeline_src = r#"
pub fn render_pipeline(input: &str) -> String {
    format_output(input)
}

pub fn exercise_pipeline() -> String {
    render_pipeline("alpha")
}

fn format_output(input: &str) -> String {
    input.to_string()
}
"#;
    let report = PathBuf::from("src/report.rs");
    let report_src = r#"
pub fn render_report(input: &str) -> String {
    format_report(input)
}

pub fn exercise_pipeline() -> String {
    render_report("beta")
}

fn format_report(input: &str) -> String {
    input.to_string()
}
"#;
    let tests = PathBuf::from("tests/contract_tests.rs");
    let tests_src = r#"
use pipeline as pipe;

#[test]
fn bare_aliased_wrapper_observes_pipeline_call_target() {
    let format_output = pipe::exercise_pipeline();
    assert_eq!(format_output, "alpha");
}
"#;
    let index = index_from_files(&[
        (pipeline, pipeline_src),
        (report, report_src),
        (tests, tests_src),
    ])?;
    let seams = inventory_seams_from_index(&[PathBuf::from("src/pipeline.rs")], &index);
    let call_presence = seams
        .iter()
        .find(|s| {
            s.kind() == SeamKind::CallPresence
                && s.owner().ends_with("::render_pipeline")
                && s.expression().contains("format_output")
        })
        .ok_or_else(|| "expected render_pipeline call_presence seam".to_string())?;

    let evidence = evidence_for_seam(call_presence, &index);

    assert!(
        !evidence
            .related_tests
            .iter()
            .any(|test| test.relation_reason == RelationReason::HelperOwnerCall),
        "bare module alias must not prove a local wrapper owner relation: {:?}",
        evidence.related_tests
    );
    assert_ne!(evidence.activate.state, StageState::Yes);
    assert!(
        evidence.observed_values.is_empty(),
        "bare aliased wrapper activation must not invent observed values: {:?}",
        evidence.observed_values
    );
    Ok(())
}

#[test]
fn given_call_presence_when_imported_module_wrapper_has_target_affinity_then_activation_is_yes()
-> Result<(), String> {
    let loop_commands = PathBuf::from("src/agent/loop_commands.rs");
    let loop_commands_src = r#"
use std::path::Path;

pub fn display_path(path: &Path) -> String {
    path.display().to_string()
}

pub fn shell_path(path: &Path) -> String {
    shell_arg(&display_path(path))
}

pub fn shell_arg(value: &str) -> String {
    value.to_string()
}
"#;
    let pilot_commands = PathBuf::from("src/output/pilot/commands.rs");
    let pilot_commands_src = r#"
use crate::agent::loop_commands::{self, display_path};
use std::path::Path;

pub fn pilot_paths(root: &Path, after: &Path) -> String {
    let root = display_path(root);
    let after = loop_commands::shell_path(after);
    format!("display_path={root};shell_arg={after}")
}
"#;
    let tests = PathBuf::from("tests/pilot_commands_tests.rs");
    let tests_src = r#"
use output::pilot::commands::pilot_paths;
use std::path::Path;

#[test]
fn pilot_paths_preserve_shell_arg_route() {
    let rendered = pilot_paths(Path::new("."), Path::new("target/out file.json"));
    assert!(rendered.contains("shell_arg="));
}
"#;
    let index = index_from_files(&[
        (loop_commands, loop_commands_src),
        (pilot_commands, pilot_commands_src),
        (tests, tests_src),
    ])?;
    let seams = inventory_seams_from_index(&[PathBuf::from("src/agent/loop_commands.rs")], &index);
    let call_presence = seams
        .iter()
        .find(|s| {
            s.kind() == SeamKind::CallPresence
                && s.owner().ends_with("::shell_path")
                && s.expression().contains("shell_arg")
        })
        .ok_or_else(|| "expected shell_path call_presence seam".to_string())?;

    let evidence = evidence_for_seam(call_presence, &index);

    assert_eq!(evidence.reach.state, StageState::Yes);
    assert!(
        evidence
            .related_tests
            .iter()
            .any(|test| test.relation_reason == RelationReason::HelperOwnerCall),
        "expected imported-module wrapper owner-call relation, got {:?}",
        evidence.related_tests
    );
    assert_eq!(evidence.activate.state, StageState::Yes);
    assert!(
        evidence.observed_values.is_empty(),
        "imported module call_presence wrapper activation must not invent values: {:?}",
        evidence.observed_values
    );
    Ok(())
}

#[test]
fn given_call_presence_when_direct_imported_wrapper_has_target_affinity_then_activation_is_yes()
-> Result<(), String> {
    let classify = PathBuf::from("src/analysis/classify.rs");
    let classify_src = r#"
pub fn reach_evidence(input: &str) -> String {
    format_marker(input)
}

pub fn reveal_evidence(input: &str) -> String {
    reveal_marker(input)
}

fn format_marker(input: &str) -> String {
    input.to_string()
}

fn reveal_marker(input: &str) -> String {
    input.to_string()
}
"#;
    let evidence = PathBuf::from("src/analysis/classifier/evidence.rs");
    let evidence_src = r#"
use crate::analysis::classify::{reach_evidence as reach, reveal_evidence};

pub fn gather(input: &str) -> String {
    let reach = reach(input);
    let reveal = reveal_evidence(input);
    format!("format_marker={reach};reveal_marker={reveal}")
}
"#;
    let tests = PathBuf::from("tests/classifier_evidence_tests.rs");
    let tests_src = r#"
use analysis::classifier::evidence::gather;

#[test]
fn direct_imported_wrapper_observes_format_marker_target() {
    let rendered = gather("alpha");
    assert!(rendered.contains("format_marker"));
}
"#;
    let index = index_from_files(&[
        (classify, classify_src),
        (evidence, evidence_src),
        (tests, tests_src),
    ])?;
    let seams = inventory_seams_from_index(&[PathBuf::from("src/analysis/classify.rs")], &index);
    let call_presence = seams
        .iter()
        .find(|s| {
            s.kind() == SeamKind::CallPresence
                && s.owner().ends_with("::reach_evidence")
                && s.expression().contains("format_marker")
        })
        .ok_or_else(|| "expected reach_evidence call_presence seam".to_string())?;

    let evidence = evidence_for_seam(call_presence, &index);

    assert_eq!(evidence.reach.state, StageState::Yes);
    assert!(
        evidence
            .related_tests
            .iter()
            .any(|test| test.relation_reason == RelationReason::HelperOwnerCall),
        "expected direct imported target-affinity wrapper owner-call relation, got {:?}",
        evidence.related_tests
    );
    assert_eq!(evidence.activate.state, StageState::Yes);
    assert!(
        evidence.observed_values.is_empty(),
        "direct imported target-affinity wrapper activation must not invent values: {:?}",
        evidence.observed_values
    );
    Ok(())
}

#[test]
fn given_call_presence_when_crate_qualified_wrapper_has_target_affinity_then_activation_is_yes()
-> Result<(), String> {
    let classify = PathBuf::from("src/analysis/classify.rs");
    let classify_src = r#"
pub fn reach_evidence(input: &str) -> String {
    format_marker(input)
}

pub fn reveal_evidence(input: &str) -> String {
    reveal_marker(input)
}

fn format_marker(input: &str) -> String {
    input.to_string()
}

fn reveal_marker(input: &str) -> String {
    input.to_string()
}
"#;
    let evidence = PathBuf::from("src/analysis/classifier/evidence.rs");
    let evidence_src = r#"
pub fn gather(input: &str) -> String {
    let reach = crate::analysis::classify::reach_evidence(input);
    let reveal = crate::analysis::classify::reveal_evidence(input);
    format!("format_marker={reach};reveal_marker={reveal}")
}
"#;
    let tests = PathBuf::from("tests/classifier_evidence_tests.rs");
    let tests_src = r#"
use analysis::classifier::evidence::gather;

#[test]
fn crate_qualified_wrapper_observes_format_marker_target() {
    let rendered = gather("alpha");
    assert!(rendered.contains("format_marker"));
}
"#;
    let index = index_from_files(&[
        (classify, classify_src),
        (evidence, evidence_src),
        (tests, tests_src),
    ])?;
    let seams = inventory_seams_from_index(&[PathBuf::from("src/analysis/classify.rs")], &index);
    let call_presence = seams
        .iter()
        .find(|s| {
            s.kind() == SeamKind::CallPresence
                && s.owner().ends_with("::reach_evidence")
                && s.expression().contains("format_marker")
        })
        .ok_or_else(|| "expected reach_evidence call_presence seam".to_string())?;

    let evidence = evidence_for_seam(call_presence, &index);

    assert_eq!(evidence.reach.state, StageState::Yes);
    assert!(
        evidence
            .related_tests
            .iter()
            .any(|test| test.relation_reason == RelationReason::HelperOwnerCall),
        "expected crate-qualified target-affinity wrapper owner-call relation, got {:?}",
        evidence.related_tests
    );
    assert_eq!(evidence.activate.state, StageState::Yes);
    assert!(
        evidence.observed_values.is_empty(),
        "crate-qualified target-affinity wrapper activation must not invent values: {:?}",
        evidence.observed_values
    );
    Ok(())
}

#[test]
fn given_call_presence_when_crate_qualified_wrapper_asserts_other_target_then_activation_stays_unknown()
-> Result<(), String> {
    let classify = PathBuf::from("src/analysis/classify.rs");
    let classify_src = r#"
pub fn reach_evidence(input: &str) -> String {
    format_marker(input)
}

pub fn reveal_evidence(input: &str) -> String {
    reveal_marker(input)
}

fn format_marker(input: &str) -> String {
    input.to_string()
}

fn reveal_marker(input: &str) -> String {
    input.to_string()
}
"#;
    let evidence = PathBuf::from("src/analysis/classifier/evidence.rs");
    let evidence_src = r#"
pub fn gather(input: &str) -> String {
    let reach = crate::analysis::classify::reach_evidence(input);
    let reveal = crate::analysis::classify::reveal_evidence(input);
    format!("format_marker={reach};reveal_marker={reveal}")
}
"#;
    let tests = PathBuf::from("tests/classifier_evidence_tests.rs");
    let tests_src = r#"
use analysis::classifier::evidence::gather;

#[test]
fn crate_qualified_wrapper_observes_reveal_marker_target() {
    let rendered = gather("alpha");
    assert!(rendered.contains("reveal_marker"));
}
"#;
    let index = index_from_files(&[
        (classify, classify_src),
        (evidence, evidence_src),
        (tests, tests_src),
    ])?;
    let seams = inventory_seams_from_index(&[PathBuf::from("src/analysis/classify.rs")], &index);
    let call_presence = seams
        .iter()
        .find(|s| {
            s.kind() == SeamKind::CallPresence
                && s.owner().ends_with("::reach_evidence")
                && s.expression().contains("format_marker")
        })
        .ok_or_else(|| "expected reach_evidence call_presence seam".to_string())?;

    let evidence = evidence_for_seam(call_presence, &index);

    assert!(
        !evidence
            .related_tests
            .iter()
            .any(|test| test.relation_reason == RelationReason::HelperOwnerCall),
        "crate-qualified other target token must not prove wrapper owner relation: {:?}",
        evidence.related_tests
    );
    assert_ne!(evidence.activate.state, StageState::Yes);
    assert!(
        evidence.observed_values.is_empty(),
        "crate-qualified other-target wrapper must not invent observed values: {:?}",
        evidence.observed_values
    );
    Ok(())
}

#[test]
fn given_call_presence_when_bare_qualified_wrapper_has_target_affinity_then_activation_stays_unknown()
-> Result<(), String> {
    let classify = PathBuf::from("src/analysis/classify.rs");
    let classify_src = r#"
pub fn reach_evidence(input: &str) -> String {
    format_marker(input)
}

fn format_marker(input: &str) -> String {
    input.to_string()
}
"#;
    let evidence = PathBuf::from("src/analysis/classifier/evidence.rs");
    let evidence_src = r#"
pub fn gather(input: &str) -> String {
    let reach = analysis::classify::reach_evidence(input);
    format!("format_marker={reach}")
}
"#;
    let tests = PathBuf::from("tests/classifier_evidence_tests.rs");
    let tests_src = r#"
use analysis::classifier::evidence::gather;

#[test]
fn bare_qualified_wrapper_mentions_format_marker_target() {
    let rendered = gather("alpha");
    assert!(rendered.contains("format_marker"));
}
"#;
    let index = index_from_files(&[
        (classify, classify_src),
        (evidence, evidence_src),
        (tests, tests_src),
    ])?;
    let seams = inventory_seams_from_index(&[PathBuf::from("src/analysis/classify.rs")], &index);
    let call_presence = seams
        .iter()
        .find(|s| {
            s.kind() == SeamKind::CallPresence
                && s.owner().ends_with("::reach_evidence")
                && s.expression().contains("format_marker")
        })
        .ok_or_else(|| "expected reach_evidence call_presence seam".to_string())?;

    let evidence = evidence_for_seam(call_presence, &index);

    assert!(
        !evidence
            .related_tests
            .iter()
            .any(|test| test.relation_reason == RelationReason::HelperOwnerCall),
        "bare qualified owner path must not prove a crate-local owner relation: {:?}",
        evidence.related_tests
    );
    assert_ne!(evidence.activate.state, StageState::Yes);
    assert!(
        evidence.observed_values.is_empty(),
        "bare qualified wrapper activation must not invent observed values: {:?}",
        evidence.observed_values
    );
    Ok(())
}

#[test]
fn given_call_presence_when_crate_qualified_owner_exists_only_in_other_package_then_activation_stays_unknown()
-> Result<(), String> {
    let alpha_owner = PathBuf::from("crates/alpha/src/analysis/other.rs");
    let alpha_owner_src = r#"
pub fn reach_evidence(input: &str) -> String {
    format_marker(input)
}

fn format_marker(input: &str) -> String {
    input.to_string()
}
"#;
    let alpha_wrapper = PathBuf::from("crates/alpha/src/analysis/classifier/evidence.rs");
    let alpha_wrapper_src = r#"
pub fn gather(input: &str) -> String {
    let reach = crate::analysis::classify::reach_evidence(input);
    format!("format_marker={reach}")
}
"#;
    let beta_owner = PathBuf::from("crates/beta/src/analysis/classify.rs");
    let beta_owner_src = r#"
pub fn reach_evidence(input: &str) -> String {
    beta_marker(input)
}

fn beta_marker(input: &str) -> String {
    input.to_string()
}
"#;
    let tests = PathBuf::from("crates/alpha/tests/classifier_evidence_tests.rs");
    let tests_src = r#"
use alpha::analysis::classifier::evidence::gather;

#[test]
fn crate_qualified_wrapper_mentions_format_marker_target() {
    let rendered = gather("alpha");
    assert!(rendered.contains("format_marker"));
}
"#;
    let index = index_from_files(&[
        (alpha_owner, alpha_owner_src),
        (alpha_wrapper, alpha_wrapper_src),
        (beta_owner, beta_owner_src),
        (tests, tests_src),
    ])?;
    let seams = inventory_seams_from_index(
        &[PathBuf::from("crates/alpha/src/analysis/other.rs")],
        &index,
    );
    let call_presence = seams
        .iter()
        .find(|s| {
            s.kind() == SeamKind::CallPresence
                && s.owner().ends_with("::reach_evidence")
                && s.expression().contains("format_marker")
        })
        .ok_or_else(|| "expected alpha reach_evidence call_presence seam".to_string())?;

    let evidence = evidence_for_seam(call_presence, &index);

    assert!(
        !evidence
            .related_tests
            .iter()
            .any(|test| test.relation_reason == RelationReason::HelperOwnerCall),
        "crate-qualified call must not resolve through another package's owner map: {:?}",
        evidence.related_tests
    );
    assert_ne!(evidence.activate.state, StageState::Yes);
    assert!(
        evidence.observed_values.is_empty(),
        "cross-package crate-qualified miss must not invent observed values: {:?}",
        evidence.observed_values
    );
    Ok(())
}

#[test]
fn given_call_presence_when_external_direct_import_matches_local_owner_name_then_activation_stays_unknown()
-> Result<(), String> {
    let classify = PathBuf::from("src/analysis/classify.rs");
    let classify_src = r#"
pub fn reach_evidence(input: &str) -> String {
    format_marker(input)
}

fn format_marker(input: &str) -> String {
    input.to_string()
}
"#;
    let evidence = PathBuf::from("src/analysis/classifier/evidence.rs");
    let evidence_src = r#"
use external_crate::{reach_evidence as reach};

pub fn gather(input: &str) -> String {
    let reach = reach(input);
    format!("format_marker={reach}")
}
"#;
    let tests = PathBuf::from("tests/classifier_evidence_tests.rs");
    let tests_src = r#"
use analysis::classifier::evidence::gather;

#[test]
fn external_direct_import_mentions_format_marker_target() {
    let rendered = gather("alpha");
    assert!(rendered.contains("format_marker"));
}
"#;
    let index = index_from_files(&[
        (classify, classify_src),
        (evidence, evidence_src),
        (tests, tests_src),
    ])?;
    let seams = inventory_seams_from_index(&[PathBuf::from("src/analysis/classify.rs")], &index);
    let call_presence = seams
        .iter()
        .find(|s| {
            s.kind() == SeamKind::CallPresence
                && s.owner().ends_with("::reach_evidence")
                && s.expression().contains("format_marker")
        })
        .ok_or_else(|| "expected reach_evidence call_presence seam".to_string())?;

    let evidence = evidence_for_seam(call_presence, &index);

    assert!(
        !evidence
            .related_tests
            .iter()
            .any(|test| test.relation_reason == RelationReason::HelperOwnerCall),
        "external direct import must not prove a local owner relation: {:?}",
        evidence.related_tests
    );
    assert_ne!(evidence.activate.state, StageState::Yes);
    assert!(
        evidence.observed_values.is_empty(),
        "external direct import activation must not invent observed values: {:?}",
        evidence.observed_values
    );
    Ok(())
}

#[test]
fn given_call_presence_when_ambiguous_direct_import_owner_name_then_activation_stays_unknown()
-> Result<(), String> {
    let classify = PathBuf::from("src/analysis/classify.rs");
    let classify_src = r#"
pub fn reach_evidence(input: &str) -> String {
    format_marker(input)
}

fn format_marker(input: &str) -> String {
    input.to_string()
}
"#;
    let alternate = PathBuf::from("src/analysis/alternate.rs");
    let alternate_src = r#"
pub fn reach_evidence(input: &str) -> String {
    alternate_marker(input)
}

fn alternate_marker(input: &str) -> String {
    input.to_string()
}
"#;
    let evidence = PathBuf::from("src/analysis/classifier/evidence.rs");
    let evidence_src = r#"
use crate::analysis::classify::reach_evidence as reach;

pub fn gather(input: &str) -> String {
    let reach = reach(input);
    format!("format_marker={reach}")
}
"#;
    let tests = PathBuf::from("tests/classifier_evidence_tests.rs");
    let tests_src = r#"
use analysis::classifier::evidence::gather;

#[test]
fn ambiguous_direct_import_mentions_format_marker_target() {
    let rendered = gather("alpha");
    assert!(rendered.contains("format_marker"));
}
"#;
    let index = index_from_files(&[
        (classify, classify_src),
        (alternate, alternate_src),
        (evidence, evidence_src),
        (tests, tests_src),
    ])?;
    let seams = inventory_seams_from_index(&[PathBuf::from("src/analysis/classify.rs")], &index);
    let call_presence = seams
        .iter()
        .find(|s| {
            s.kind() == SeamKind::CallPresence
                && s.owner().ends_with("::reach_evidence")
                && s.expression().contains("format_marker")
        })
        .ok_or_else(|| "expected reach_evidence call_presence seam".to_string())?;

    let evidence = evidence_for_seam(call_presence, &index);

    assert!(
        !evidence
            .related_tests
            .iter()
            .any(|test| test.relation_reason == RelationReason::HelperOwnerCall),
        "ambiguous direct import owner name must not prove wrapper owner relation: {:?}",
        evidence.related_tests
    );
    assert_ne!(evidence.activate.state, StageState::Yes);
    assert!(
        evidence.observed_values.is_empty(),
        "ambiguous direct import activation must not invent observed values: {:?}",
        evidence.observed_values
    );
    Ok(())
}

#[test]
fn given_call_presence_when_module_qualified_wrapper_asserts_other_target_then_activation_stays_unknown()
-> Result<(), String> {
    let pipeline = PathBuf::from("src/pipeline.rs");
    let pipeline_src = r#"
pub fn render_pipeline(input: &str) -> String {
    format_output(input)
}

pub fn exercise_pipeline() -> String {
    render_pipeline("alpha")
}

fn format_output(input: &str) -> String {
    input.to_string()
}
"#;
    let report = PathBuf::from("src/report.rs");
    let report_src = r#"
pub fn render_report(input: &str) -> String {
    format_report(input)
}

pub fn exercise_pipeline() -> String {
    render_report("beta")
}

fn format_report(input: &str) -> String {
    input.to_string()
}
"#;
    let tests = PathBuf::from("tests/contract_tests.rs");
    let tests_src = r#"
#[test]
fn qualified_wrapper_observes_report_call_target() {
    let rendered = pipeline::exercise_pipeline();
    assert!(rendered.contains("format_report"));
}
"#;
    let index = index_from_files(&[
        (pipeline, pipeline_src),
        (report, report_src),
        (tests, tests_src),
    ])?;
    let seams = inventory_seams_from_index(&[PathBuf::from("src/pipeline.rs")], &index);
    let call_presence = seams
        .iter()
        .find(|s| {
            s.kind() == SeamKind::CallPresence
                && s.owner().ends_with("::render_pipeline")
                && s.expression().contains("format_output")
        })
        .ok_or_else(|| "expected render_pipeline call_presence seam".to_string())?;

    let evidence = evidence_for_seam(call_presence, &index);

    assert!(
        !evidence
            .related_tests
            .iter()
            .any(|test| test.relation_reason == RelationReason::HelperOwnerCall),
        "other target token must not prove qualified wrapper owner relation: {:?}",
        evidence.related_tests
    );
    assert_ne!(evidence.activate.state, StageState::Yes);
    assert!(
        evidence.observed_values.is_empty(),
        "other-target qualified wrapper must not invent observed values: {:?}",
        evidence.observed_values
    );
    Ok(())
}

#[test]
fn given_call_presence_when_aliased_module_wrapper_asserts_other_target_then_activation_stays_unknown()
-> Result<(), String> {
    let pipeline = PathBuf::from("src/pipeline.rs");
    let pipeline_src = r#"
pub fn render_pipeline(input: &str) -> String {
    format_output(input)
}

pub fn exercise_pipeline() -> String {
    render_pipeline("alpha")
}

fn format_output(input: &str) -> String {
    input.to_string()
}
"#;
    let report = PathBuf::from("src/report.rs");
    let report_src = r#"
pub fn render_report(input: &str) -> String {
    format_report(input)
}

pub fn exercise_pipeline() -> String {
    render_report("beta")
}

fn format_report(input: &str) -> String {
    input.to_string()
}
"#;
    let tests = PathBuf::from("tests/contract_tests.rs");
    let tests_src = r#"
use crate::pipeline as pipe;

#[test]
fn aliased_wrapper_observes_report_call_target() {
    let rendered = pipe::exercise_pipeline();
    assert!(rendered.contains("format_report"));
}
"#;
    let index = index_from_files(&[
        (pipeline, pipeline_src),
        (report, report_src),
        (tests, tests_src),
    ])?;
    let seams = inventory_seams_from_index(&[PathBuf::from("src/pipeline.rs")], &index);
    let call_presence = seams
        .iter()
        .find(|s| {
            s.kind() == SeamKind::CallPresence
                && s.owner().ends_with("::render_pipeline")
                && s.expression().contains("format_output")
        })
        .ok_or_else(|| "expected render_pipeline call_presence seam".to_string())?;

    let evidence = evidence_for_seam(call_presence, &index);

    assert!(
        !evidence
            .related_tests
            .iter()
            .any(|test| test.relation_reason == RelationReason::HelperOwnerCall),
        "aliased other target token must not prove wrapper owner relation: {:?}",
        evidence.related_tests
    );
    assert_ne!(evidence.activate.state, StageState::Yes);
    assert!(
        evidence.observed_values.is_empty(),
        "aliased other-target wrapper must not invent observed values: {:?}",
        evidence.observed_values
    );
    Ok(())
}

#[test]
fn given_call_presence_when_direct_imported_wrapper_asserts_other_target_then_activation_stays_unknown()
-> Result<(), String> {
    let classify = PathBuf::from("src/analysis/classify.rs");
    let classify_src = r#"
pub fn reach_evidence(input: &str) -> String {
    format_marker(input)
}

pub fn reveal_evidence(input: &str) -> String {
    reveal_marker(input)
}

fn format_marker(input: &str) -> String {
    input.to_string()
}

fn reveal_marker(input: &str) -> String {
    input.to_string()
}
"#;
    let evidence = PathBuf::from("src/analysis/classifier/evidence.rs");
    let evidence_src = r#"
use crate::analysis::classify::{reach_evidence as reach, reveal_evidence};

pub fn gather(input: &str) -> String {
    let reach = reach(input);
    let reveal = reveal_evidence(input);
    format!("format_marker={reach};reveal_marker={reveal}")
}
"#;
    let tests = PathBuf::from("tests/classifier_evidence_tests.rs");
    let tests_src = r#"
use analysis::classifier::evidence::gather;

#[test]
fn direct_imported_wrapper_observes_reveal_marker_target() {
    let rendered = gather("alpha");
    assert!(rendered.contains("reveal_marker"));
}
"#;
    let index = index_from_files(&[
        (classify, classify_src),
        (evidence, evidence_src),
        (tests, tests_src),
    ])?;
    let seams = inventory_seams_from_index(&[PathBuf::from("src/analysis/classify.rs")], &index);
    let call_presence = seams
        .iter()
        .find(|s| {
            s.kind() == SeamKind::CallPresence
                && s.owner().ends_with("::reach_evidence")
                && s.expression().contains("format_marker")
        })
        .ok_or_else(|| "expected reach_evidence call_presence seam".to_string())?;

    let evidence = evidence_for_seam(call_presence, &index);

    assert!(
        !evidence
            .related_tests
            .iter()
            .any(|test| test.relation_reason == RelationReason::HelperOwnerCall),
        "direct imported other target token must not prove wrapper owner relation: {:?}",
        evidence.related_tests
    );
    assert_ne!(evidence.activate.state, StageState::Yes);
    assert!(
        evidence.observed_values.is_empty(),
        "direct imported other-target wrapper must not invent observed values: {:?}",
        evidence.observed_values
    );
    Ok(())
}

#[test]
fn given_call_presence_when_test_local_helper_shadows_production_wrapper_then_activation_stays_unknown()
-> Result<(), String> {
    let prod = PathBuf::from("src/pipeline.rs");
    let prod_src = r#"
pub fn render_pipeline(input: &str) -> String {
    format_output(input)
}

pub fn exercise_pipeline() -> String {
    render_pipeline("alpha")
}

fn format_output(input: &str) -> String {
    input.to_string()
}
"#;
    let tests = PathBuf::from("tests/pipeline_tests.rs");
    let tests_src = r#"
fn exercise_pipeline() -> String {
    "alpha".to_string()
}

#[test]
fn local_shadow_keeps_pipeline_limited() {
    let format_output = exercise_pipeline();
    assert_eq!(format_output, "alpha");
}
"#;
    let index = index_from_files(&[(prod, prod_src), (tests, tests_src)])?;
    let seams = inventory_seams_from_index(&[PathBuf::from("src/pipeline.rs")], &index);
    let call_presence = seams
        .iter()
        .find(|s| {
            s.kind() == SeamKind::CallPresence
                && s.owner().ends_with("::render_pipeline")
                && s.expression().contains("format_output")
        })
        .ok_or_else(|| "expected render_pipeline call_presence seam".to_string())?;

    let evidence = evidence_for_seam(call_presence, &index);

    assert_eq!(evidence.reach.state, StageState::Yes);
    assert_eq!(evidence.activate.state, StageState::Unknown);
    assert!(
        !evidence
            .related_tests
            .iter()
            .any(|test| test.relation_reason == RelationReason::HelperOwnerCall),
        "test-local shadow must not inherit production wrapper relation: {:?}",
        evidence.related_tests
    );
    Ok(())
}

#[test]
fn given_call_presence_when_test_local_helper_shadows_target_affinity_wrapper_then_activation_stays_unknown()
-> Result<(), String> {
    let source = PathBuf::from("src/pipeline.rs");
    let source_src = r#"
pub fn render_pipeline(input: &str) -> String {
    format_output(input)
}

pub fn render_report(input: &str) -> String {
    format_report(input)
}

pub fn exercise_both(input: &str) -> String {
    let pipeline = render_pipeline(input);
    let report = render_report(input);
    format!("format_output={pipeline};format_report={report}")
}

fn format_output(input: &str) -> String {
    input.to_string()
}

fn format_report(input: &str) -> String {
    input.to_string()
}

#[cfg(test)]
mod tests {
    fn exercise_both(_: &str) -> String {
        "format_output".to_string()
    }

    #[test]
    fn local_shadow_only_mentions_target() {
        let rendered = exercise_both("alpha");
        assert!(rendered.contains("format_output"));
    }
}
"#;
    let index = index_from_files(&[(source, source_src)])?;
    let seams = inventory_seams_from_index(&[PathBuf::from("src/pipeline.rs")], &index);
    let call_presence = seams
        .iter()
        .find(|s| {
            s.kind() == SeamKind::CallPresence
                && s.owner().ends_with("::render_pipeline")
                && s.expression().contains("format_output")
        })
        .ok_or_else(|| "expected render_pipeline call_presence seam".to_string())?;

    let evidence = evidence_for_seam(call_presence, &index);

    assert_eq!(evidence.reach.state, StageState::Yes);
    assert!(
        !evidence
            .related_tests
            .iter()
            .any(|test| test.relation_reason == RelationReason::HelperOwnerCall),
        "test-local target-affinity shadow must not get helper-owner relation: {:?}",
        evidence.related_tests
    );
    assert_eq!(evidence.activate.state, StageState::Unknown);
    assert!(
        evidence.observed_values.is_empty(),
        "test-local target-affinity shadow must not invent observed values: {:?}",
        evidence.observed_values
    );
    Ok(())
}

#[test]
fn given_call_presence_when_unique_test_support_helper_calls_owner_then_activation_is_yes()
-> Result<(), String> {
    let prod = PathBuf::from("src/pipeline.rs");
    let prod_src = r#"
pub fn render_pipeline(input: &str) -> String {
    format_output(input)
}

fn format_output(input: &str) -> String {
    input.to_string()
}
"#;
    let support = PathBuf::from("tests/support.rs");
    let support_src = r#"
use pipeline::render_pipeline;

pub fn exercise_pipeline() -> String {
    render_pipeline("alpha")
}
"#;
    let tests = PathBuf::from("tests/pipeline_tests.rs");
    let tests_src = r#"
use support::exercise_pipeline;

#[test]
fn pipeline_from_support_helper() {
    let rendered = exercise_pipeline();
    assert_eq!(rendered, "alpha");
}
"#;
    let index = index_from_files(&[(prod, prod_src), (support, support_src), (tests, tests_src)])?;
    let seams = inventory_seams_from_index(&[PathBuf::from("src/pipeline.rs")], &index);
    let call_presence = seams
        .iter()
        .find(|s| {
            s.kind() == SeamKind::CallPresence
                && s.owner().ends_with("::render_pipeline")
                && s.expression().contains("format_output")
        })
        .ok_or_else(|| "expected render_pipeline call_presence seam".to_string())?;

    let evidence = evidence_for_seam(call_presence, &index);

    assert_eq!(evidence.reach.state, StageState::Yes);
    assert_eq!(evidence.activate.state, StageState::Yes);
    assert!(
        evidence
            .related_tests
            .iter()
            .any(|test| test.relation_reason == RelationReason::HelperOwnerCall),
        "expected unique test-support helper owner-call relation, got {:?}",
        evidence.related_tests
    );
    assert!(
        evidence.observed_values.is_empty(),
        "call_presence support helper activation must not invent values: {:?}",
        evidence.observed_values
    );
    assert!(
        evidence
            .activate
            .summary
            .contains("helper owner call for value-insensitive seam"),
        "activation summary should explain the unique support helper owner-call route: {}",
        evidence.activate.summary
    );
    Ok(())
}

#[test]
fn given_call_presence_when_duplicate_test_support_helpers_share_owner_then_activation_is_yes()
-> Result<(), String> {
    let prod = PathBuf::from("src/pipeline.rs");
    let prod_src = r#"
pub fn render_pipeline(input: &str) -> String {
    format_output(input)
}

fn format_output(input: &str) -> String {
    input.to_string()
}
"#;
    let support_a = PathBuf::from("tests/support_a.rs");
    let support_a_src = r#"
use pipeline::render_pipeline;

pub fn exercise_pipeline() -> String {
    render_pipeline("alpha")
}
"#;
    let support_b = PathBuf::from("tests/support_b.rs");
    let support_b_src = r#"
use pipeline::render_pipeline;

pub fn exercise_pipeline() -> String {
    render_pipeline("beta")
}
"#;
    let tests = PathBuf::from("tests/pipeline_tests.rs");
    let tests_src = r#"
use support_a::exercise_pipeline;

#[test]
fn pipeline_from_support_helper() {
    let rendered = exercise_pipeline();
    assert_eq!(rendered, "alpha");
}
"#;
    let index = index_from_files(&[
        (prod, prod_src),
        (support_a, support_a_src),
        (support_b, support_b_src),
        (tests, tests_src),
    ])?;
    let seams = inventory_seams_from_index(&[PathBuf::from("src/pipeline.rs")], &index);
    let call_presence = seams
        .iter()
        .find(|s| {
            s.kind() == SeamKind::CallPresence
                && s.owner().ends_with("::render_pipeline")
                && s.expression().contains("format_output")
        })
        .ok_or_else(|| "expected render_pipeline call_presence seam".to_string())?;

    let evidence = evidence_for_seam(call_presence, &index);

    assert_eq!(evidence.reach.state, StageState::Yes);
    assert_eq!(evidence.activate.state, StageState::Yes);
    assert!(
        evidence
            .related_tests
            .iter()
            .any(|test| test.relation_reason == RelationReason::HelperOwnerCall),
        "expected duplicate same-owner support helpers to prove helper-owner relation, got {:?}",
        evidence.related_tests
    );
    assert!(
        evidence.observed_values.is_empty(),
        "call_presence duplicate support helper activation must not invent values: {:?}",
        evidence.observed_values
    );
    Ok(())
}
#[test]
fn given_call_presence_when_test_support_helper_name_is_ambiguous_then_activation_stays_unknown()
-> Result<(), String> {
    let pipeline = PathBuf::from("src/pipeline.rs");
    let pipeline_src = r#"
pub fn render_pipeline(input: &str) -> String {
    format_output(input)
}

fn format_output(input: &str) -> String {
    input.to_string()
}
"#;
    let report = PathBuf::from("src/report.rs");
    let report_src = r#"
pub fn render_report(input: &str) -> String {
    format_report(input)
}

fn format_report(input: &str) -> String {
    input.to_string()
}
"#;
    let support_a = PathBuf::from("tests/support_a.rs");
    let support_a_src = r#"
use pipeline::render_pipeline;

pub fn exercise_pipeline() -> String {
    render_pipeline("alpha")
}
"#;
    let support_b = PathBuf::from("tests/support_b.rs");
    let support_b_src = r#"
use report::render_report;

pub fn exercise_pipeline() -> String {
    render_report("alpha")
}
"#;
    let tests = PathBuf::from("tests/pipeline_tests.rs");
    let tests_src = r#"
#[test]
fn ambiguous_support_helper_smoke() {
    let rendered = exercise_pipeline();
    assert!(!rendered.is_empty());
}
"#;
    let index = index_from_files(&[
        (pipeline, pipeline_src),
        (report, report_src),
        (support_a, support_a_src),
        (support_b, support_b_src),
        (tests, tests_src),
    ])?;
    let seams = inventory_seams_from_index(&[PathBuf::from("src/pipeline.rs")], &index);
    let call_presence = seams
        .iter()
        .find(|s| {
            s.kind() == SeamKind::CallPresence
                && s.owner().ends_with("::render_pipeline")
                && s.expression().contains("format_output")
        })
        .ok_or_else(|| "expected render_pipeline call_presence seam".to_string())?;

    let evidence = evidence_for_seam(call_presence, &index);

    assert_eq!(evidence.reach.state, StageState::Yes);
    assert_eq!(evidence.activate.state, StageState::Unknown);
    assert!(
        !evidence
            .related_tests
            .iter()
            .any(|test| test.relation_reason == RelationReason::HelperOwnerCall),
        "ambiguous test-support helper name must not get helper-owner relation: {:?}",
        evidence.related_tests
    );
    Ok(())
}

#[test]
fn given_call_presence_when_direct_imported_ambiguous_support_helper_calls_owner_then_activation_is_yes()
-> Result<(), String> {
    let pipeline = PathBuf::from("src/pipeline.rs");
    let pipeline_src = r#"
pub fn render_pipeline(input: &str) -> String {
    format_output(input)
}

fn format_output(input: &str) -> String {
    input.to_string()
}
"#;
    let report = PathBuf::from("src/report.rs");
    let report_src = r#"
pub fn render_report(input: &str) -> String {
    format_report(input)
}

fn format_report(input: &str) -> String {
    input.to_string()
}
"#;
    let support_a = PathBuf::from("tests/support_a.rs");
    let support_a_src = r#"
use pipeline::render_pipeline;

pub fn exercise_pipeline() -> String {
    render_pipeline("alpha")
}
"#;
    let support_b = PathBuf::from("tests/support_b.rs");
    let support_b_src = r#"
use report::render_report;

pub fn exercise_pipeline() -> String {
    render_report("beta")
}
"#;
    let tests = PathBuf::from("tests/pipeline_tests.rs");
    let tests_src = r#"
use support_a::exercise_pipeline;

#[test]
fn direct_imported_support_helper_reaches_pipeline() {
    let rendered = exercise_pipeline();
    assert_eq!(rendered, "alpha");
}
"#;
    let index = index_from_files(&[
        (pipeline, pipeline_src),
        (report, report_src),
        (support_a, support_a_src),
        (support_b, support_b_src),
        (tests, tests_src),
    ])?;
    let seams = inventory_seams_from_index(&[PathBuf::from("src/pipeline.rs")], &index);
    let call_presence = seams
        .iter()
        .find(|s| {
            s.kind() == SeamKind::CallPresence
                && s.owner().ends_with("::render_pipeline")
                && s.expression().contains("format_output")
        })
        .ok_or_else(|| "expected render_pipeline call_presence seam".to_string())?;

    let evidence = evidence_for_seam(call_presence, &index);

    assert_eq!(evidence.reach.state, StageState::Yes);
    assert_eq!(evidence.activate.state, StageState::Yes);
    assert!(
        evidence
            .related_tests
            .iter()
            .any(|test| test.relation_reason == RelationReason::HelperOwnerCall),
        "expected direct imported support helper to disambiguate owner-call relation, got {:?}",
        evidence.related_tests
    );
    assert!(
        evidence.observed_values.is_empty(),
        "direct imported support helper activation must not invent values: {:?}",
        evidence.observed_values
    );
    Ok(())
}

#[test]
fn given_call_presence_when_aliased_direct_imported_support_helper_calls_owner_then_activation_is_yes()
-> Result<(), String> {
    let pipeline = PathBuf::from("src/pipeline.rs");
    let pipeline_src = r#"
pub fn render_pipeline(input: &str) -> String {
    format_output(input)
}

fn format_output(input: &str) -> String {
    input.to_string()
}
"#;
    let report = PathBuf::from("src/report.rs");
    let report_src = r#"
pub fn render_report(input: &str) -> String {
    format_report(input)
}

fn format_report(input: &str) -> String {
    input.to_string()
}
"#;
    let support_a = PathBuf::from("tests/support_a.rs");
    let support_a_src = r#"
use pipeline::render_pipeline;

pub fn exercise_pipeline() -> String {
    render_pipeline("alpha")
}
"#;
    let support_b = PathBuf::from("tests/support_b.rs");
    let support_b_src = r#"
use report::render_report;

pub fn exercise_pipeline() -> String {
    render_report("beta")
}
"#;
    let tests = PathBuf::from("tests/pipeline_tests.rs");
    let tests_src = r#"
use support_a::exercise_pipeline as exercise;

#[test]
fn aliased_direct_imported_support_helper_reaches_pipeline() {
    let rendered = exercise();
    assert_eq!(rendered, "alpha");
}
"#;
    let index = index_from_files(&[
        (pipeline, pipeline_src),
        (report, report_src),
        (support_a, support_a_src),
        (support_b, support_b_src),
        (tests, tests_src),
    ])?;
    let seams = inventory_seams_from_index(&[PathBuf::from("src/pipeline.rs")], &index);
    let call_presence = seams
        .iter()
        .find(|s| {
            s.kind() == SeamKind::CallPresence
                && s.owner().ends_with("::render_pipeline")
                && s.expression().contains("format_output")
        })
        .ok_or_else(|| "expected render_pipeline call_presence seam".to_string())?;

    let evidence = evidence_for_seam(call_presence, &index);

    assert_eq!(evidence.reach.state, StageState::Yes);
    assert_eq!(evidence.activate.state, StageState::Yes);
    assert!(
        evidence
            .related_tests
            .iter()
            .any(|test| test.relation_reason == RelationReason::HelperOwnerCall),
        "expected aliased direct imported support helper owner-call relation, got {:?}",
        evidence.related_tests
    );
    assert!(
        evidence.observed_values.is_empty(),
        "aliased direct imported support helper activation must not invent values: {:?}",
        evidence.observed_values
    );
    Ok(())
}

#[test]
fn given_call_presence_when_direct_imported_support_helper_targets_other_owner_then_activation_stays_unknown()
-> Result<(), String> {
    let pipeline = PathBuf::from("src/pipeline.rs");
    let pipeline_src = r#"
pub fn render_pipeline(input: &str) -> String {
    format_output(input)
}

fn format_output(input: &str) -> String {
    input.to_string()
}
"#;
    let report = PathBuf::from("src/report.rs");
    let report_src = r#"
pub fn render_report(input: &str) -> String {
    format_report(input)
}

fn format_report(input: &str) -> String {
    input.to_string()
}
"#;
    let support_b = PathBuf::from("tests/support_b.rs");
    let support_b_src = r#"
use report::render_report;

pub fn exercise_pipeline() -> String {
    render_report("beta")
}
"#;
    let tests = PathBuf::from("tests/pipeline_tests.rs");
    let tests_src = r#"
use support_b::exercise_pipeline;

#[test]
fn direct_imported_support_helper_reaches_report() {
    let rendered = exercise_pipeline();
    assert_eq!(rendered, "beta");
}
"#;
    let index = index_from_files(&[
        (pipeline, pipeline_src),
        (report, report_src),
        (support_b, support_b_src),
        (tests, tests_src),
    ])?;
    let seams = inventory_seams_from_index(&[PathBuf::from("src/pipeline.rs")], &index);
    let call_presence = seams
        .iter()
        .find(|s| {
            s.kind() == SeamKind::CallPresence
                && s.owner().ends_with("::render_pipeline")
                && s.expression().contains("format_output")
        })
        .ok_or_else(|| "expected render_pipeline call_presence seam".to_string())?;

    let evidence = evidence_for_seam(call_presence, &index);

    assert_eq!(evidence.reach.state, StageState::Yes);
    assert_eq!(evidence.activate.state, StageState::Unknown);
    assert!(
        !evidence
            .related_tests
            .iter()
            .any(|test| test.relation_reason == RelationReason::HelperOwnerCall),
        "direct imported support helper for another owner must not prove this owner relation: {:?}",
        evidence.related_tests
    );
    assert!(
        evidence.observed_values.is_empty(),
        "other-owner support helper activation must not invent values: {:?}",
        evidence.observed_values
    );
    Ok(())
}

#[test]
fn given_call_presence_when_direct_imported_external_helper_then_activation_stays_unknown()
-> Result<(), String> {
    let pipeline = PathBuf::from("src/pipeline.rs");
    let pipeline_src = r#"
pub fn render_pipeline(input: &str) -> String {
    format_output(input)
}

fn format_output(input: &str) -> String {
    input.to_string()
}
"#;
    let tests = PathBuf::from("tests/pipeline_tests.rs");
    let tests_src = r#"
use external_support::exercise_pipeline;

#[test]
fn external_direct_import_mentions_pipeline() {
    let rendered = exercise_pipeline();
    assert_eq!(rendered, "alpha");
}
"#;
    let index = index_from_files(&[(pipeline, pipeline_src), (tests, tests_src)])?;
    let seams = inventory_seams_from_index(&[PathBuf::from("src/pipeline.rs")], &index);
    let call_presence = seams
        .iter()
        .find(|s| {
            s.kind() == SeamKind::CallPresence
                && s.owner().ends_with("::render_pipeline")
                && s.expression().contains("format_output")
        })
        .ok_or_else(|| "expected render_pipeline call_presence seam".to_string())?;

    let evidence = evidence_for_seam(call_presence, &index);

    assert_eq!(evidence.reach.state, StageState::Yes);
    assert_eq!(evidence.activate.state, StageState::Unknown);
    assert!(
        !evidence
            .related_tests
            .iter()
            .any(|test| test.relation_reason == RelationReason::HelperOwnerCall),
        "external direct imported helper must not prove a local owner relation: {:?}",
        evidence.related_tests
    );
    assert!(
        evidence.observed_values.is_empty(),
        "external direct imported helper activation must not invent values: {:?}",
        evidence.observed_values
    );
    Ok(())
}

#[test]
fn given_call_presence_when_block_local_direct_imported_helper_is_not_file_scoped_then_activation_stays_unknown()
-> Result<(), String> {
    let pipeline = PathBuf::from("src/pipeline.rs");
    let pipeline_src = r#"
pub fn render_pipeline(input: &str) -> String {
    format_output(input)
}

fn format_output(input: &str) -> String {
    input.to_string()
}
"#;
    let report = PathBuf::from("src/report.rs");
    let report_src = r#"
pub fn render_report(input: &str) -> String {
    format_report(input)
}

fn format_report(input: &str) -> String {
    input.to_string()
}
"#;
    let support_a = PathBuf::from("tests/support_a.rs");
    let support_a_src = r#"
use pipeline::render_pipeline;

pub fn exercise_pipeline() -> String {
    render_pipeline("alpha")
}
"#;
    let support_b = PathBuf::from("tests/support_b.rs");
    let support_b_src = r#"
use report::render_report;

pub fn exercise_pipeline() -> String {
    render_report("beta")
}
"#;
    let tests = PathBuf::from("tests/pipeline_tests.rs");
    let tests_src = r#"
#[test]
fn block_local_import_reaches_pipeline() {
    use support_a::exercise_pipeline;
    let rendered = exercise_pipeline();
    assert_eq!(rendered, "alpha");
}

#[test]
fn sibling_without_import_mentions_pipeline() {
    let rendered = exercise_pipeline();
    assert_eq!(rendered, "alpha");
}
"#;
    let index = index_from_files(&[
        (pipeline, pipeline_src),
        (report, report_src),
        (support_a, support_a_src),
        (support_b, support_b_src),
        (tests, tests_src),
    ])?;
    let seams = inventory_seams_from_index(&[PathBuf::from("src/pipeline.rs")], &index);
    let call_presence = seams
        .iter()
        .find(|s| {
            s.kind() == SeamKind::CallPresence
                && s.owner().ends_with("::render_pipeline")
                && s.expression().contains("format_output")
        })
        .ok_or_else(|| "expected render_pipeline call_presence seam".to_string())?;

    let evidence = evidence_for_seam(call_presence, &index);

    assert_ne!(evidence.activate.state, StageState::Yes);
    assert!(
        !evidence
            .related_tests
            .iter()
            .any(|test| test.relation_reason == RelationReason::HelperOwnerCall),
        "block-local direct helper import must not leak to sibling tests: {:?}",
        evidence.related_tests
    );
    assert!(
        evidence.observed_values.is_empty(),
        "block-local direct helper import must not invent values: {:?}",
        evidence.observed_values
    );
    Ok(())
}

#[test]
fn given_call_presence_when_ambiguous_support_helper_is_module_qualified_then_activation_is_yes()
-> Result<(), String> {
    let pipeline = PathBuf::from("src/pipeline.rs");
    let pipeline_src = r#"
pub fn render_pipeline(input: &str) -> String {
    format_output(input)
}

fn format_output(input: &str) -> String {
    input.to_string()
}
"#;
    let report = PathBuf::from("src/report.rs");
    let report_src = r#"
pub fn render_report(input: &str) -> String {
    format_report(input)
}

fn format_report(input: &str) -> String {
    input.to_string()
}
"#;
    let support_a = PathBuf::from("tests/support_a.rs");
    let support_a_src = r#"
use pipeline::render_pipeline;

pub fn exercise_pipeline() -> String {
    render_pipeline("alpha")
}
"#;
    let support_b = PathBuf::from("tests/support_b.rs");
    let support_b_src = r#"
use report::render_report;

pub fn exercise_pipeline() -> String {
    render_report("beta")
}
"#;
    let tests = PathBuf::from("tests/pipeline_tests.rs");
    let tests_src = r#"
#[test]
fn qualified_support_helper_reaches_pipeline() {
    let rendered = support_a::exercise_pipeline();
    assert_eq!(rendered, "alpha");
}
"#;
    let index = index_from_files(&[
        (pipeline, pipeline_src),
        (report, report_src),
        (support_a, support_a_src),
        (support_b, support_b_src),
        (tests, tests_src),
    ])?;
    let seams = inventory_seams_from_index(&[PathBuf::from("src/pipeline.rs")], &index);
    let call_presence = seams
        .iter()
        .find(|s| {
            s.kind() == SeamKind::CallPresence
                && s.owner().ends_with("::render_pipeline")
                && s.expression().contains("format_output")
        })
        .ok_or_else(|| "expected render_pipeline call_presence seam".to_string())?;

    let evidence = evidence_for_seam(call_presence, &index);

    assert_eq!(evidence.reach.state, StageState::Yes);
    assert_eq!(evidence.activate.state, StageState::Yes);
    assert!(
        evidence
            .related_tests
            .iter()
            .any(|test| test.relation_reason == RelationReason::HelperOwnerCall),
        "expected qualified support helper to disambiguate helper-owner relation, got {:?}",
        evidence.related_tests
    );
    assert!(
        evidence.observed_values.is_empty(),
        "qualified call_presence helper activation must not invent values: {:?}",
        evidence.observed_values
    );
    Ok(())
}

#[test]
fn given_call_presence_when_qualified_support_helper_targets_other_owner_then_no_helper_relation()
-> Result<(), String> {
    let pipeline = PathBuf::from("src/pipeline.rs");
    let pipeline_src = r#"
pub fn render_pipeline(input: &str) -> String {
    format_output(input)
}

fn format_output(input: &str) -> String {
    input.to_string()
}
"#;
    let report = PathBuf::from("src/report.rs");
    let report_src = r#"
pub fn render_report(input: &str) -> String {
    format_report(input)
}

fn format_report(input: &str) -> String {
    input.to_string()
}
"#;
    let support_a = PathBuf::from("tests/support_a.rs");
    let support_a_src = r#"
use pipeline::render_pipeline;

pub fn exercise_pipeline() -> String {
    render_pipeline("alpha")
}
"#;
    let support_b = PathBuf::from("tests/support_b.rs");
    let support_b_src = r#"
use report::render_report;

pub fn exercise_pipeline() -> String {
    render_report("beta")
}
"#;
    let tests = PathBuf::from("tests/pipeline_tests.rs");
    let tests_src = r#"
#[test]
fn qualified_support_helper_reaches_report() {
    let rendered = support_b::exercise_pipeline();
    assert_eq!(rendered, "beta");
}
"#;
    let index = index_from_files(&[
        (pipeline, pipeline_src),
        (report, report_src),
        (support_a, support_a_src),
        (support_b, support_b_src),
        (tests, tests_src),
    ])?;
    let seams = inventory_seams_from_index(&[PathBuf::from("src/pipeline.rs")], &index);
    let call_presence = seams
        .iter()
        .find(|s| {
            s.kind() == SeamKind::CallPresence
                && s.owner().ends_with("::render_pipeline")
                && s.expression().contains("format_output")
        })
        .ok_or_else(|| "expected render_pipeline call_presence seam".to_string())?;

    let evidence = evidence_for_seam(call_presence, &index);

    assert!(
        !evidence
            .related_tests
            .iter()
            .any(|test| test.relation_reason == RelationReason::HelperOwnerCall),
        "qualified helper for another owner must not prove pipeline owner relation: {:?}",
        evidence.related_tests
    );
    assert!(
        evidence.observed_values.is_empty(),
        "other-owner qualified helper must not invent observed values: {:?}",
        evidence.observed_values
    );
    Ok(())
}

#[test]
fn given_call_presence_when_crate_qualified_support_helper_calls_owner_then_activation_is_yes()
-> Result<(), String> {
    let pipeline = PathBuf::from("src/pipeline.rs");
    let pipeline_src = r#"
pub fn render_pipeline(input: &str) -> String {
    format_output(input)
}

fn format_output(input: &str) -> String {
    input.to_string()
}
"#;
    let report = PathBuf::from("src/report.rs");
    let report_src = r#"
pub fn render_report(input: &str) -> String {
    format_report(input)
}

fn format_report(input: &str) -> String {
    input.to_string()
}
"#;
    let support_a = PathBuf::from("tests/support_a.rs");
    let support_a_src = r#"
use pipeline::render_pipeline;

pub fn exercise_pipeline() -> String {
    render_pipeline("alpha")
}
"#;
    let support_b = PathBuf::from("tests/support_b.rs");
    let support_b_src = r#"
use report::render_report;

pub fn exercise_pipeline() -> String {
    render_report("beta")
}
"#;
    let tests = PathBuf::from("tests/pipeline_tests.rs");
    let tests_src = r#"
#[test]
fn crate_qualified_support_helper_reaches_pipeline() {
    let rendered = crate::support_a::exercise_pipeline();
    assert_eq!(rendered, "alpha");
}
"#;
    let index = index_from_files(&[
        (pipeline, pipeline_src),
        (report, report_src),
        (support_a, support_a_src),
        (support_b, support_b_src),
        (tests, tests_src),
    ])?;
    let seams = inventory_seams_from_index(&[PathBuf::from("src/pipeline.rs")], &index);
    let call_presence = seams
        .iter()
        .find(|s| {
            s.kind() == SeamKind::CallPresence
                && s.owner().ends_with("::render_pipeline")
                && s.expression().contains("format_output")
        })
        .ok_or_else(|| "expected render_pipeline call_presence seam".to_string())?;

    let evidence = evidence_for_seam(call_presence, &index);

    assert_eq!(evidence.reach.state, StageState::Yes);
    assert_eq!(evidence.activate.state, StageState::Yes);
    assert!(
        evidence
            .related_tests
            .iter()
            .any(|test| test.relation_reason == RelationReason::HelperOwnerCall),
        "expected crate-qualified support helper to prove helper-owner relation, got {:?}",
        evidence.related_tests
    );
    assert!(
        evidence.observed_values.is_empty(),
        "crate-qualified call_presence helper activation must not invent values: {:?}",
        evidence.observed_values
    );
    Ok(())
}

#[test]
fn given_call_presence_when_super_qualified_support_helper_calls_owner_then_activation_is_yes()
-> Result<(), String> {
    let pipeline = PathBuf::from("src/pipeline.rs");
    let pipeline_src = r#"
pub fn render_pipeline(input: &str) -> String {
    format_output(input)
}

fn format_output(input: &str) -> String {
    input.to_string()
}
"#;
    let report = PathBuf::from("src/report.rs");
    let report_src = r#"
pub fn render_report(input: &str) -> String {
    format_report(input)
}

fn format_report(input: &str) -> String {
    input.to_string()
}
"#;
    let support_a = PathBuf::from("tests/support_a.rs");
    let support_a_src = r#"
use pipeline::render_pipeline;

pub fn exercise_pipeline() -> String {
    render_pipeline("alpha")
}
"#;
    let support_b = PathBuf::from("tests/support_b.rs");
    let support_b_src = r#"
use report::render_report;

pub fn exercise_pipeline() -> String {
    render_report("beta")
}
"#;
    let tests = PathBuf::from("tests/pipeline_tests.rs");
    let tests_src = r#"
mod nested {
    #[test]
    fn super_qualified_support_helper_reaches_pipeline() {
        let rendered = super::support_a::exercise_pipeline();
        assert_eq!(rendered, "alpha");
    }
}
"#;
    let index = index_from_files(&[
        (pipeline, pipeline_src),
        (report, report_src),
        (support_a, support_a_src),
        (support_b, support_b_src),
        (tests, tests_src),
    ])?;
    let seams = inventory_seams_from_index(&[PathBuf::from("src/pipeline.rs")], &index);
    let call_presence = seams
        .iter()
        .find(|s| {
            s.kind() == SeamKind::CallPresence
                && s.owner().ends_with("::render_pipeline")
                && s.expression().contains("format_output")
        })
        .ok_or_else(|| "expected render_pipeline call_presence seam".to_string())?;

    let evidence = evidence_for_seam(call_presence, &index);

    assert_eq!(evidence.reach.state, StageState::Yes);
    assert_eq!(evidence.activate.state, StageState::Yes);
    assert!(
        evidence
            .related_tests
            .iter()
            .any(|test| test.relation_reason == RelationReason::HelperOwnerCall),
        "expected super-qualified support helper to prove helper-owner relation, got {:?}",
        evidence.related_tests
    );
    assert!(
        evidence.observed_values.is_empty(),
        "super-qualified call_presence helper activation must not invent values: {:?}",
        evidence.observed_values
    );
    Ok(())
}

#[test]
fn given_qualified_support_helper_only_in_comment_or_string_then_call_match_is_ignored() {
    let cleaned = strip_comments_and_strings(
        "let doc = \"support_a::exercise_pipeline()\"; // support_a::exercise_pipeline()",
    );
    assert!(!code_contains_qualified_helper_call(
        &cleaned,
        "support_a",
        "exercise_pipeline"
    ));
    assert!(code_contains_qualified_helper_call(
        "let rendered = support_a::exercise_pipeline();",
        "support_a",
        "exercise_pipeline"
    ));
    assert!(code_contains_qualified_helper_call(
        "let rendered = crate::support_a::exercise_pipeline();",
        "support_a",
        "exercise_pipeline"
    ));
    assert!(code_contains_qualified_helper_call(
        "let rendered = self::support_a::exercise_pipeline();",
        "support_a",
        "exercise_pipeline"
    ));
    assert!(code_contains_qualified_helper_call(
        "let rendered = super::support_a::exercise_pipeline();",
        "support_a",
        "exercise_pipeline"
    ));
    assert!(!code_contains_qualified_helper_call(
        "let rendered = other_support_a::exercise_pipeline();",
        "support_a",
        "exercise_pipeline"
    ));
    assert!(!code_contains_qualified_helper_call(
        "let rendered = my_super::support_a::exercise_pipeline();",
        "support_a",
        "exercise_pipeline"
    ));
    assert!(!code_contains_qualified_helper_call(
        "let rendered = other::support_a::exercise_pipeline();",
        "support_a",
        "exercise_pipeline"
    ));
}

#[test]
fn direct_delegate_wrapper_name_preserves_turbofish_constructor() {
    assert_eq!(direct_delegate_wrapper_name("Ok::<String, ()>"), "Ok");
    assert_eq!(
        direct_delegate_wrapper_name("decorate::<String>"),
        "decorate"
    );
    assert_eq!(direct_delegate_wrapper_name("Box::<String>::new"), "new");
}

#[test]
fn direct_delegate_condition_prefix_accepts_only_leading_condition_owner_call() {
    assert!(direct_delegate_condition_prefix_is_allowed("if"));
    assert!(direct_delegate_condition_prefix_is_allowed("if !"));
    assert!(!direct_delegate_condition_prefix_is_allowed("} else if"));
    assert!(!direct_delegate_condition_prefix_is_allowed(
        "    } else if"
    ));
    assert!(!direct_delegate_condition_prefix_is_allowed("} else if !"));
    assert!(!direct_delegate_condition_prefix_is_allowed("if ready &&"));
    assert!(!direct_delegate_condition_prefix_is_allowed("while"));
    assert!(!direct_delegate_condition_prefix_is_allowed("match"));
}

#[test]
fn direct_receiver_method_prefix_accepts_only_leading_condition_receiver_call() {
    assert!(direct_receiver_method_prefix_is_allowed("pipeline."));
    assert!(direct_receiver_method_prefix_is_allowed("if pipeline."));
    assert!(direct_receiver_method_prefix_is_allowed("if !pipeline."));
    assert!(direct_receiver_method_prefix_is_allowed("if ! pipeline."));
    assert!(!direct_receiver_method_prefix_is_allowed(
        "} else if pipeline."
    ));
    assert!(!direct_receiver_method_prefix_is_allowed(
        "if pipeline.ready && other."
    ));
    assert!(!direct_receiver_method_prefix_is_allowed(
        "pipeline_factory()."
    ));
}

#[test]
fn given_call_presence_when_test_local_helper_wraps_owner_call_in_option_then_activation_is_yes()
-> Result<(), String> {
    let prod = PathBuf::from("src/pipeline.rs");
    let prod_src = r#"
pub fn render_pipeline(input: &str) -> String {
    format_output(input)
}

fn format_output(input: &str) -> String {
    input.to_string()
}
"#;
    let tests = PathBuf::from("tests/pipeline_tests.rs");
    let tests_src = r#"
use pipeline::render_pipeline;

fn exercise_pipeline() -> Option<String> {
    Some(render_pipeline("alpha"))
}

#[test]
fn helper_exercises_pipeline() {
    let output = exercise_pipeline().unwrap();
    assert_eq!(output, "alpha");
}
"#;
    let index = index_from_files(&[(prod, prod_src), (tests, tests_src)])?;
    let seams = inventory_seams_from_index(&[PathBuf::from("src/pipeline.rs")], &index);
    let call_presence = seams
        .iter()
        .find(|s| {
            s.kind() == SeamKind::CallPresence
                && s.owner().ends_with("::render_pipeline")
                && s.expression().contains("format_output")
        })
        .ok_or_else(|| "expected render_pipeline call_presence seam".to_string())?;

    let evidence = evidence_for_seam(call_presence, &index);

    assert_eq!(evidence.reach.state, StageState::Yes);
    assert_eq!(evidence.activate.state, StageState::Yes);
    assert!(
        evidence
            .related_tests
            .iter()
            .any(|test| test.relation_reason == RelationReason::HelperOwnerCall),
        "expected option-wrapped helper owner-call relation, got {:?}",
        evidence.related_tests
    );
    assert!(
        evidence.observed_values.is_empty(),
        "option-wrapped call_presence helper activation must not invent values: {:?}",
        evidence.observed_values
    );
    assert!(
        evidence.missing_discriminators.is_empty(),
        "option-wrapped call_presence helper activation must not create boundary debt"
    );
    Ok(())
}

#[test]
fn given_call_presence_when_test_local_helper_wraps_owner_call_in_result_then_activation_is_yes()
-> Result<(), String> {
    let prod = PathBuf::from("src/pipeline.rs");
    let prod_src = r#"
pub fn render_pipeline(input: &str) -> String {
    format_output(input)
}

fn format_output(input: &str) -> String {
    input.to_string()
}
"#;
    let tests = PathBuf::from("tests/pipeline_tests.rs");
    let tests_src = r#"
use pipeline::render_pipeline;

fn exercise_pipeline() -> Result<String, ()> {
    Ok(render_pipeline("alpha"))
}

#[test]
fn helper_exercises_pipeline() {
    let output = exercise_pipeline().unwrap();
    assert_eq!(output, "alpha");
}
"#;
    let index = index_from_files(&[(prod, prod_src), (tests, tests_src)])?;
    let seams = inventory_seams_from_index(&[PathBuf::from("src/pipeline.rs")], &index);
    let call_presence = seams
        .iter()
        .find(|s| {
            s.kind() == SeamKind::CallPresence
                && s.owner().ends_with("::render_pipeline")
                && s.expression().contains("format_output")
        })
        .ok_or_else(|| "expected render_pipeline call_presence seam".to_string())?;

    let evidence = evidence_for_seam(call_presence, &index);

    assert_eq!(evidence.reach.state, StageState::Yes);
    assert_eq!(evidence.activate.state, StageState::Yes);
    assert!(
        evidence
            .related_tests
            .iter()
            .any(|test| test.relation_reason == RelationReason::HelperOwnerCall),
        "expected result-wrapped helper owner-call relation, got {:?}",
        evidence.related_tests
    );
    assert!(
        evidence.observed_values.is_empty(),
        "result-wrapped call_presence helper activation must not invent values: {:?}",
        evidence.observed_values
    );
    assert!(
        evidence.missing_discriminators.is_empty(),
        "result-wrapped call_presence helper activation must not create boundary debt"
    );
    Ok(())
}

#[test]
fn given_call_presence_when_test_local_helper_wraps_owner_call_in_result_turbofish_then_activation_is_yes()
-> Result<(), String> {
    let prod = PathBuf::from("src/pipeline.rs");
    let prod_src = r#"
pub fn render_pipeline(input: &str) -> String {
    format_output(input)
}

fn format_output(input: &str) -> String {
    input.to_string()
}
"#;
    let tests = PathBuf::from("tests/pipeline_tests.rs");
    let tests_src = r#"
use pipeline::render_pipeline;

fn exercise_pipeline() -> Result<String, ()> {
    Ok::<String, ()>(render_pipeline("alpha"))
}

#[test]
fn helper_exercises_pipeline() {
    let output = exercise_pipeline().unwrap();
    assert_eq!(output, "alpha");
}
"#;
    let index = index_from_files(&[(prod, prod_src), (tests, tests_src)])?;
    let seams = inventory_seams_from_index(&[PathBuf::from("src/pipeline.rs")], &index);
    let call_presence = seams
        .iter()
        .find(|s| {
            s.kind() == SeamKind::CallPresence
                && s.owner().ends_with("::render_pipeline")
                && s.expression().contains("format_output")
        })
        .ok_or_else(|| "expected render_pipeline call_presence seam".to_string())?;

    let evidence = evidence_for_seam(call_presence, &index);

    assert_eq!(evidence.reach.state, StageState::Yes);
    assert_eq!(evidence.activate.state, StageState::Yes);
    assert!(
        evidence
            .related_tests
            .iter()
            .any(|test| test.relation_reason == RelationReason::HelperOwnerCall),
        "expected result turbofish helper owner-call relation, got {:?}",
        evidence.related_tests
    );
    assert!(
        evidence.observed_values.is_empty(),
        "result turbofish helper activation must not invent values: {:?}",
        evidence.observed_values
    );
    assert!(
        evidence.missing_discriminators.is_empty(),
        "result turbofish helper activation must not create boundary debt"
    );
    Ok(())
}

#[test]
fn given_call_presence_when_test_local_helper_wraps_owner_call_in_err_then_activation_is_yes()
-> Result<(), String> {
    let prod = PathBuf::from("src/pipeline.rs");
    let prod_src = r#"
pub fn render_pipeline(input: &str) -> String {
    format_output(input)
}

fn format_output(input: &str) -> String {
    input.to_string()
}
"#;
    let tests = PathBuf::from("tests/pipeline_tests.rs");
    let tests_src = r#"
use pipeline::render_pipeline;

fn exercise_pipeline() -> Result<(), String> {
    Err(render_pipeline("alpha"))
}

#[test]
fn helper_exercises_pipeline() {
    let format_output = exercise_pipeline().unwrap_err();
    assert_eq!(format_output, "alpha");
}
"#;
    let index = index_from_files(&[(prod, prod_src), (tests, tests_src)])?;
    let seams = inventory_seams_from_index(&[PathBuf::from("src/pipeline.rs")], &index);
    let call_presence = seams
        .iter()
        .find(|s| {
            s.kind() == SeamKind::CallPresence
                && s.owner().ends_with("::render_pipeline")
                && s.expression().contains("format_output")
        })
        .ok_or_else(|| "expected render_pipeline call_presence seam".to_string())?;

    let evidence = evidence_for_seam(call_presence, &index);

    assert_eq!(evidence.reach.state, StageState::Yes);
    assert_eq!(evidence.activate.state, StageState::Yes);
    assert!(
        evidence
            .related_tests
            .iter()
            .any(|test| test.relation_reason == RelationReason::HelperOwnerCall),
        "expected Err-wrapped helper owner-call relation, got {:?}",
        evidence.related_tests
    );
    assert!(
        evidence.observed_values.is_empty(),
        "Err-wrapped call_presence helper activation must not invent values: {:?}",
        evidence.observed_values
    );
    assert!(
        evidence.missing_discriminators.is_empty(),
        "Err-wrapped call_presence helper activation must not create boundary debt"
    );
    Ok(())
}

#[test]
fn given_call_presence_when_test_local_helper_unwraps_owner_call_result_then_activation_is_yes()
-> Result<(), String> {
    let prod = PathBuf::from("src/pipeline.rs");
    let prod_src = r#"
pub fn render_pipeline(input: &str) -> Result<String, ()> {
    Ok(format_output(input))
}

fn format_output(input: &str) -> String {
    input.to_string()
}
"#;
    let tests = PathBuf::from("tests/pipeline_tests.rs");
    let tests_src = r#"
use pipeline::render_pipeline;

fn exercise_pipeline() -> String {
    render_pipeline("alpha").unwrap()
}

#[test]
fn helper_exercises_pipeline() {
    let output = exercise_pipeline();
    assert_eq!(output, "alpha");
}
"#;
    let index = index_from_files(&[(prod, prod_src), (tests, tests_src)])?;
    let seams = inventory_seams_from_index(&[PathBuf::from("src/pipeline.rs")], &index);
    let call_presence = seams
        .iter()
        .find(|s| {
            s.kind() == SeamKind::CallPresence
                && s.owner().ends_with("::render_pipeline")
                && s.expression().contains("format_output")
        })
        .ok_or_else(|| "expected render_pipeline call_presence seam".to_string())?;

    let evidence = evidence_for_seam(call_presence, &index);

    assert_eq!(evidence.reach.state, StageState::Yes);
    assert_eq!(evidence.activate.state, StageState::Yes);
    assert!(
        evidence
            .related_tests
            .iter()
            .any(|test| test.relation_reason == RelationReason::HelperOwnerCall),
        "expected unwrap helper owner-call relation, got {:?}",
        evidence.related_tests
    );
    assert!(
        evidence.observed_values.is_empty(),
        "unwrap helper activation must not invent values: {:?}",
        evidence.observed_values
    );
    assert!(
        evidence.missing_discriminators.is_empty(),
        "unwrap helper activation must not create boundary debt"
    );
    Ok(())
}

#[test]
fn given_call_presence_when_test_local_helper_expects_owner_call_result_then_activation_is_yes()
-> Result<(), String> {
    let prod = PathBuf::from("src/pipeline.rs");
    let prod_src = r#"
pub fn render_pipeline(input: &str) -> Result<String, ()> {
    Ok(format_output(input))
}

fn format_output(input: &str) -> String {
    input.to_string()
}
"#;
    let tests = PathBuf::from("tests/pipeline_tests.rs");
    let tests_src = r#"
use pipeline::render_pipeline;

fn exercise_pipeline() -> String {
    render_pipeline("alpha").expect("pipeline should render")
}

#[test]
fn helper_exercises_pipeline() {
    let output = exercise_pipeline();
    assert_eq!(output, "alpha");
}
"#;
    let index = index_from_files(&[(prod, prod_src), (tests, tests_src)])?;
    let seams = inventory_seams_from_index(&[PathBuf::from("src/pipeline.rs")], &index);
    let call_presence = seams
        .iter()
        .find(|s| {
            s.kind() == SeamKind::CallPresence
                && s.owner().ends_with("::render_pipeline")
                && s.expression().contains("format_output")
        })
        .ok_or_else(|| "expected render_pipeline call_presence seam".to_string())?;

    let evidence = evidence_for_seam(call_presence, &index);

    assert_eq!(evidence.reach.state, StageState::Yes);
    assert_eq!(evidence.activate.state, StageState::Yes);
    assert!(
        evidence
            .related_tests
            .iter()
            .any(|test| test.relation_reason == RelationReason::HelperOwnerCall),
        "expected expect helper owner-call relation, got {:?}",
        evidence.related_tests
    );
    assert!(
        evidence.observed_values.is_empty(),
        "expect helper activation must not invent values: {:?}",
        evidence.observed_values
    );
    assert!(
        evidence.missing_discriminators.is_empty(),
        "expect helper activation must not create boundary debt"
    );
    Ok(())
}

#[test]
fn given_call_presence_when_test_local_helper_borrows_owner_call_result_then_activation_is_yes()
-> Result<(), String> {
    let prod = PathBuf::from("src/pipeline.rs");
    let prod_src = r#"
pub fn render_pipeline(input: &str) -> Result<String, ()> {
    Ok(format_output(input))
}

fn format_output(input: &str) -> String {
    input.to_string()
}
"#;
    let tests = PathBuf::from("tests/pipeline_tests.rs");
    let tests_src = r#"
use pipeline::render_pipeline;

fn exercise_pipeline() -> String {
    render_pipeline("alpha").as_ref().unwrap().clone()
}

#[test]
fn helper_exercises_pipeline() {
    let output = exercise_pipeline();
    assert_eq!(output, "alpha");
}
"#;
    let index = index_from_files(&[(prod, prod_src), (tests, tests_src)])?;
    let seams = inventory_seams_from_index(&[PathBuf::from("src/pipeline.rs")], &index);
    let call_presence = seams
        .iter()
        .find(|s| {
            s.kind() == SeamKind::CallPresence
                && s.owner().ends_with("::render_pipeline")
                && s.expression().contains("format_output")
        })
        .ok_or_else(|| "expected render_pipeline call_presence seam".to_string())?;

    let evidence = evidence_for_seam(call_presence, &index);

    assert_eq!(evidence.reach.state, StageState::Yes);
    assert_eq!(evidence.activate.state, StageState::Yes);
    assert!(
        evidence
            .related_tests
            .iter()
            .any(|test| test.relation_reason == RelationReason::HelperOwnerCall),
        "expected as_ref borrow-chain helper owner-call relation, got {:?}",
        evidence.related_tests
    );
    assert!(
        evidence.observed_values.is_empty(),
        "as_ref borrow-chain helper activation must not invent observed values: {:?}",
        evidence.observed_values
    );
    assert!(
        evidence.missing_discriminators.is_empty(),
        "as_ref borrow-chain helper activation must not create boundary debt"
    );
    Ok(())
}

#[test]
fn given_call_presence_when_test_local_helper_trims_owner_call_result_then_activation_is_yes()
-> Result<(), String> {
    let prod = PathBuf::from("src/pipeline.rs");
    let prod_src = r#"
pub fn render_pipeline(input: &str) -> String {
    format_output(input)
}

fn format_output(input: &str) -> String {
    input.to_string()
}
"#;
    let tests = PathBuf::from("tests/pipeline_tests.rs");
    let tests_src = r#"
use pipeline::render_pipeline;

fn exercise_pipeline() -> String {
    render_pipeline(" alpha ").trim().to_string()
}

#[test]
fn helper_exercises_pipeline() {
    let output = exercise_pipeline();
    assert_eq!(output, "alpha");
}
"#;
    let index = index_from_files(&[(prod, prod_src), (tests, tests_src)])?;
    let seams = inventory_seams_from_index(&[PathBuf::from("src/pipeline.rs")], &index);
    let call_presence = seams
        .iter()
        .find(|s| {
            s.kind() == SeamKind::CallPresence
                && s.owner().ends_with("::render_pipeline")
                && s.expression().contains("format_output")
        })
        .ok_or_else(|| "expected render_pipeline call_presence seam".to_string())?;

    let evidence = evidence_for_seam(call_presence, &index);

    assert_eq!(evidence.reach.state, StageState::Yes);
    assert_eq!(evidence.activate.state, StageState::Yes);
    assert!(
        evidence
            .related_tests
            .iter()
            .any(|test| test.relation_reason == RelationReason::HelperOwnerCall),
        "expected trim-chain helper owner-call relation, got {:?}",
        evidence.related_tests
    );
    assert!(
        evidence.observed_values.is_empty(),
        "trim-chain helper activation must not invent values: {:?}",
        evidence.observed_values
    );
    Ok(())
}

#[test]
fn given_call_presence_when_test_local_helper_uses_unknown_owner_call_chain_then_activation_stays_unknown()
-> Result<(), String> {
    let prod = PathBuf::from("src/pipeline.rs");
    let prod_src = r#"
pub fn render_pipeline(input: &str) -> String {
    format_output(input)
}

fn format_output(input: &str) -> String {
    input.to_string()
}
"#;
    let tests = PathBuf::from("tests/pipeline_tests.rs");
    let tests_src = r#"
use pipeline::render_pipeline;

fn exercise_pipeline() -> String {
    render_pipeline("alpha").normalize()
}

#[test]
fn helper_exercises_pipeline() {
    let output = exercise_pipeline();
    assert_eq!(output, "alpha");
}
"#;
    let index = index_from_files(&[(prod, prod_src), (tests, tests_src)])?;
    let seams = inventory_seams_from_index(&[PathBuf::from("src/pipeline.rs")], &index);
    let call_presence = seams
        .iter()
        .find(|s| {
            s.kind() == SeamKind::CallPresence
                && s.owner().ends_with("::render_pipeline")
                && s.expression().contains("format_output")
        })
        .ok_or_else(|| "expected render_pipeline call_presence seam".to_string())?;

    let evidence = evidence_for_seam(call_presence, &index);

    assert_eq!(evidence.reach.state, StageState::Yes);
    assert_eq!(evidence.activate.state, StageState::Unknown);
    assert!(
        !evidence
            .related_tests
            .iter()
            .any(|test| test.relation_reason == RelationReason::HelperOwnerCall),
        "unknown post-owner method chain must not get helper-owner relation: {:?}",
        evidence.related_tests
    );
    assert!(
        evidence.observed_values.is_empty(),
        "unknown post-owner method-chain activation must not invent values: {:?}",
        evidence.observed_values
    );
    Ok(())
}

#[test]
fn given_call_presence_when_test_local_assertion_helper_calls_owner_then_activation_is_yes()
-> Result<(), String> {
    let prod = PathBuf::from("src/pipeline.rs");
    let prod_src = r#"
pub fn render_pipeline(input: &str) -> String {
    format_output(input)
}

fn format_output(input: &str) -> String {
    input.to_string()
}
"#;
    let tests = PathBuf::from("tests/pipeline_tests.rs");
    let tests_src = r#"
use pipeline::render_pipeline;

fn assert_pipeline_output() {
    assert_eq!(render_pipeline("alpha"), "alpha");
}

#[test]
fn helper_asserts_pipeline_output() {
    assert_pipeline_output();
}
"#;
    let index = index_from_files(&[(prod, prod_src), (tests, tests_src)])?;
    let seams = inventory_seams_from_index(&[PathBuf::from("src/pipeline.rs")], &index);
    let call_presence = seams
        .iter()
        .find(|s| {
            s.kind() == SeamKind::CallPresence
                && s.owner().ends_with("::render_pipeline")
                && s.expression().contains("format_output")
        })
        .ok_or_else(|| "expected render_pipeline call_presence seam".to_string())?;

    let evidence = evidence_for_seam(call_presence, &index);

    assert_eq!(evidence.reach.state, StageState::Yes);
    assert_eq!(evidence.activate.state, StageState::Yes);
    assert!(
        evidence
            .related_tests
            .iter()
            .any(|test| test.relation_reason == RelationReason::HelperOwnerCall),
        "expected assertion helper owner-call relation, got {:?}",
        evidence.related_tests
    );
    assert!(
        evidence.observed_values.is_empty(),
        "assertion helper activation must not invent values: {:?}",
        evidence.observed_values
    );
    assert!(
        evidence.missing_discriminators.is_empty(),
        "assertion helper activation must not create boundary debt"
    );
    Ok(())
}

#[test]
fn given_call_presence_when_test_local_equality_helper_calls_owner_as_later_arg_then_activation_is_yes()
-> Result<(), String> {
    let prod = PathBuf::from("src/pipeline.rs");
    let prod_src = r#"
pub fn render_pipeline(input: &str) -> String {
    format_output(input)
}

fn format_output(input: &str) -> String {
    input.to_string()
}
"#;
    let tests = PathBuf::from("tests/pipeline_tests.rs");
    let tests_src = r#"
use pipeline::render_pipeline;

fn assert_pipeline_output(expected: &str) {
    assert_eq!(expected, render_pipeline("alpha"));
}

#[test]
fn helper_asserts_pipeline_output() {
    assert_pipeline_output("alpha");
}
"#;
    let index = index_from_files(&[(prod, prod_src), (tests, tests_src)])?;
    let seams = inventory_seams_from_index(&[PathBuf::from("src/pipeline.rs")], &index);
    let call_presence = seams
        .iter()
        .find(|s| {
            s.kind() == SeamKind::CallPresence
                && s.owner().ends_with("::render_pipeline")
                && s.expression().contains("format_output")
        })
        .ok_or_else(|| "expected render_pipeline call_presence seam".to_string())?;

    let evidence = evidence_for_seam(call_presence, &index);

    assert_eq!(evidence.reach.state, StageState::Yes);
    assert_eq!(evidence.activate.state, StageState::Yes);
    assert!(
        evidence
            .related_tests
            .iter()
            .any(|test| test.relation_reason == RelationReason::HelperOwnerCall),
        "expected equality helper owner-call relation, got {:?}",
        evidence.related_tests
    );
    assert!(
        evidence.observed_values.is_empty(),
        "equality helper activation must not invent values: {:?}",
        evidence.observed_values
    );
    assert!(
        evidence.missing_discriminators.is_empty(),
        "equality helper activation must not create boundary debt"
    );
    Ok(())
}

#[test]
fn given_call_presence_when_assert_message_arg_calls_owner_then_activation_stays_unknown()
-> Result<(), String> {
    let prod = PathBuf::from("src/pipeline.rs");
    let prod_src = r#"
pub fn render_pipeline(input: &str) -> String {
    format_output(input)
}

fn format_output(input: &str) -> String {
    input.to_string()
}
"#;
    let tests = PathBuf::from("tests/pipeline_tests.rs");
    let tests_src = r#"
use pipeline::render_pipeline;

fn assert_pipeline_output() {
    assert!(true, "{}", render_pipeline("alpha"));
}

#[test]
fn helper_asserts_pipeline_output() {
    assert_pipeline_output();
}
"#;
    let index = index_from_files(&[(prod, prod_src), (tests, tests_src)])?;
    let seams = inventory_seams_from_index(&[PathBuf::from("src/pipeline.rs")], &index);
    let call_presence = seams
        .iter()
        .find(|s| {
            s.kind() == SeamKind::CallPresence
                && s.owner().ends_with("::render_pipeline")
                && s.expression().contains("format_output")
        })
        .ok_or_else(|| "expected render_pipeline call_presence seam".to_string())?;

    let evidence = evidence_for_seam(call_presence, &index);

    assert_eq!(evidence.reach.state, StageState::Yes);
    assert_eq!(evidence.activate.state, StageState::Unknown);
    assert!(
        !evidence
            .related_tests
            .iter()
            .any(|test| test.relation_reason == RelationReason::HelperOwnerCall),
        "assert message argument must not get helper-owner relation: {:?}",
        evidence.related_tests
    );
    assert!(
        evidence.observed_values.is_empty(),
        "assert message argument must not invent values: {:?}",
        evidence.observed_values
    );
    Ok(())
}

#[test]
fn given_call_presence_when_test_local_assert_macro_helper_calls_owner_then_activation_is_yes()
-> Result<(), String> {
    let prod = PathBuf::from("src/pipeline.rs");
    let prod_src = r#"
pub fn render_pipeline(input: &str) -> String {
    format_output(input)
}

fn format_output(input: &str) -> String {
    input.to_string()
}
"#;
    let tests = PathBuf::from("tests/pipeline_tests.rs");
    let tests_src = r#"
use pipeline::render_pipeline;

fn assert_pipeline_output() {
    assert!(render_pipeline("alpha") == "alpha");
}

#[test]
fn helper_asserts_pipeline_output() {
    assert_pipeline_output();
}
"#;
    let index = index_from_files(&[(prod, prod_src), (tests, tests_src)])?;
    let seams = inventory_seams_from_index(&[PathBuf::from("src/pipeline.rs")], &index);
    let call_presence = seams
        .iter()
        .find(|s| {
            s.kind() == SeamKind::CallPresence
                && s.owner().ends_with("::render_pipeline")
                && s.expression().contains("format_output")
        })
        .ok_or_else(|| "expected render_pipeline call_presence seam".to_string())?;

    let evidence = evidence_for_seam(call_presence, &index);

    assert_eq!(evidence.reach.state, StageState::Yes);
    assert_eq!(evidence.activate.state, StageState::Yes);
    assert!(
        evidence
            .related_tests
            .iter()
            .any(|test| test.relation_reason == RelationReason::HelperOwnerCall),
        "expected assert macro helper owner-call relation, got {:?}",
        evidence.related_tests
    );
    assert!(
        evidence.observed_values.is_empty(),
        "assert macro helper activation must not invent values: {:?}",
        evidence.observed_values
    );
    assert!(
        evidence.missing_discriminators.is_empty(),
        "assert macro helper activation must not create boundary debt"
    );
    Ok(())
}

#[test]
fn given_call_presence_when_test_local_assert_macro_short_circuits_owner_then_activation_stays_unknown()
-> Result<(), String> {
    let prod = PathBuf::from("src/pipeline.rs");
    let prod_src = r#"
pub fn render_pipeline(input: &str) -> String {
    format_output(input)
}

fn format_output(input: &str) -> String {
    input.to_string()
}
"#;
    let tests = PathBuf::from("tests/pipeline_tests.rs");
    let tests_src = r#"
use pipeline::render_pipeline;

fn assert_pipeline_output(enabled: bool) {
    assert!(enabled && render_pipeline("alpha") == "alpha");
}

#[test]
fn helper_asserts_pipeline_output() {
    assert_pipeline_output(true);
}
"#;
    let index = index_from_files(&[(prod, prod_src), (tests, tests_src)])?;
    let seams = inventory_seams_from_index(&[PathBuf::from("src/pipeline.rs")], &index);
    let call_presence = seams
        .iter()
        .find(|s| {
            s.kind() == SeamKind::CallPresence
                && s.owner().ends_with("::render_pipeline")
                && s.expression().contains("format_output")
        })
        .ok_or_else(|| "expected render_pipeline call_presence seam".to_string())?;

    let evidence = evidence_for_seam(call_presence, &index);

    assert_eq!(evidence.reach.state, StageState::Yes);
    assert_eq!(evidence.activate.state, StageState::Unknown);
    assert!(
        !evidence
            .related_tests
            .iter()
            .any(|test| test.relation_reason == RelationReason::HelperOwnerCall),
        "short-circuiting assert macro must not get helper-owner relation: {:?}",
        evidence.related_tests
    );
    assert!(
        evidence.observed_values.is_empty(),
        "short-circuiting assert macro activation must not invent values: {:?}",
        evidence.observed_values
    );
    Ok(())
}

#[test]
fn given_call_presence_when_test_local_matches_macro_helper_calls_owner_then_activation_is_yes()
-> Result<(), String> {
    let prod = PathBuf::from("src/pipeline.rs");
    let prod_src = r#"
pub fn render_pipeline(input: &str) -> bool {
    is_alpha(input)
}

fn is_alpha(input: &str) -> bool {
    input == "alpha"
}
"#;
    let tests = PathBuf::from("tests/pipeline_tests.rs");
    let tests_src = r#"
use pipeline::render_pipeline;

fn exercise_pipeline() -> bool {
    matches!(render_pipeline("alpha"), true)
}

#[test]
fn helper_exercises_pipeline() {
    assert!(exercise_pipeline());
}
"#;
    let index = index_from_files(&[(prod, prod_src), (tests, tests_src)])?;
    let seams = inventory_seams_from_index(&[PathBuf::from("src/pipeline.rs")], &index);
    let call_presence = seams
        .iter()
        .find(|s| {
            s.kind() == SeamKind::CallPresence
                && s.owner().ends_with("::render_pipeline")
                && s.expression().contains("is_alpha")
        })
        .ok_or_else(|| "expected render_pipeline call_presence seam".to_string())?;

    let evidence = evidence_for_seam(call_presence, &index);

    assert_eq!(evidence.reach.state, StageState::Yes);
    assert_eq!(evidence.activate.state, StageState::Yes);
    assert!(
        evidence
            .related_tests
            .iter()
            .any(|test| test.relation_reason == RelationReason::HelperOwnerCall),
        "expected matches macro helper owner-call relation, got {:?}",
        evidence.related_tests
    );
    assert!(
        evidence.observed_values.is_empty(),
        "matches macro helper activation must not invent values: {:?}",
        evidence.observed_values
    );
    Ok(())
}

#[test]
fn given_call_presence_when_test_local_matches_macro_short_circuits_owner_then_activation_stays_unknown()
-> Result<(), String> {
    let prod = PathBuf::from("src/pipeline.rs");
    let prod_src = r#"
pub fn render_pipeline(input: &str) -> bool {
    is_alpha(input)
}

fn is_alpha(input: &str) -> bool {
    input == "alpha"
}
"#;
    let tests = PathBuf::from("tests/pipeline_tests.rs");
    let tests_src = r#"
use pipeline::render_pipeline;

fn exercise_pipeline(enabled: bool) -> bool {
    matches!(enabled && render_pipeline("alpha"), true)
}

#[test]
fn helper_exercises_pipeline() {
    assert!(exercise_pipeline(true));
}
"#;
    let index = index_from_files(&[(prod, prod_src), (tests, tests_src)])?;
    let seams = inventory_seams_from_index(&[PathBuf::from("src/pipeline.rs")], &index);
    let call_presence = seams
        .iter()
        .find(|s| {
            s.kind() == SeamKind::CallPresence
                && s.owner().ends_with("::render_pipeline")
                && s.expression().contains("is_alpha")
        })
        .ok_or_else(|| "expected render_pipeline call_presence seam".to_string())?;

    let evidence = evidence_for_seam(call_presence, &index);

    assert_eq!(evidence.reach.state, StageState::Yes);
    assert_eq!(evidence.activate.state, StageState::Unknown);
    assert!(
        !evidence
            .related_tests
            .iter()
            .any(|test| test.relation_reason == RelationReason::HelperOwnerCall),
        "short-circuiting matches macro must not get helper-owner relation: {:?}",
        evidence.related_tests
    );
    assert!(
        evidence.observed_values.is_empty(),
        "short-circuiting matches macro activation must not invent values: {:?}",
        evidence.observed_values
    );
    Ok(())
}

#[test]
fn given_call_presence_when_test_local_dbg_macro_helper_calls_owner_then_activation_is_yes()
-> Result<(), String> {
    let prod = PathBuf::from("src/pipeline.rs");
    let prod_src = r#"
pub fn render_pipeline(input: &str) -> String {
    format_output(input)
}

fn format_output(input: &str) -> String {
    input.to_string()
}
"#;
    let tests = PathBuf::from("tests/pipeline_tests.rs");
    let tests_src = r#"
use pipeline::render_pipeline;

fn exercise_pipeline() -> String {
    dbg!(render_pipeline("alpha"))
}

#[test]
fn helper_exercises_pipeline() {
    let output = exercise_pipeline();
    assert_eq!(output, "alpha");
}
"#;
    let index = index_from_files(&[(prod, prod_src), (tests, tests_src)])?;
    let seams = inventory_seams_from_index(&[PathBuf::from("src/pipeline.rs")], &index);
    let call_presence = seams
        .iter()
        .find(|s| {
            s.kind() == SeamKind::CallPresence
                && s.owner().ends_with("::render_pipeline")
                && s.expression().contains("format_output")
        })
        .ok_or_else(|| "expected render_pipeline call_presence seam".to_string())?;

    let evidence = evidence_for_seam(call_presence, &index);

    assert_eq!(evidence.reach.state, StageState::Yes);
    assert_eq!(evidence.activate.state, StageState::Yes);
    assert!(
        evidence
            .related_tests
            .iter()
            .any(|test| test.relation_reason == RelationReason::HelperOwnerCall),
        "expected dbg macro helper owner-call relation, got {:?}",
        evidence.related_tests
    );
    assert!(
        evidence.observed_values.is_empty(),
        "dbg macro helper activation must not invent values: {:?}",
        evidence.observed_values
    );
    Ok(())
}

#[test]
fn given_call_presence_when_test_local_dbg_macro_short_circuits_owner_then_activation_stays_unknown()
-> Result<(), String> {
    let prod = PathBuf::from("src/pipeline.rs");
    let prod_src = r#"
pub fn render_pipeline(input: &str) -> bool {
    is_alpha(input)
}

fn is_alpha(input: &str) -> bool {
    input == "alpha"
}
"#;
    let tests = PathBuf::from("tests/pipeline_tests.rs");
    let tests_src = r#"
use pipeline::render_pipeline;

fn exercise_pipeline(enabled: bool) -> bool {
    dbg!(enabled && render_pipeline("alpha"))
}

#[test]
fn helper_exercises_pipeline() {
    assert!(exercise_pipeline(true));
}
"#;
    let index = index_from_files(&[(prod, prod_src), (tests, tests_src)])?;
    let seams = inventory_seams_from_index(&[PathBuf::from("src/pipeline.rs")], &index);
    let call_presence = seams
        .iter()
        .find(|s| {
            s.kind() == SeamKind::CallPresence
                && s.owner().ends_with("::render_pipeline")
                && s.expression().contains("is_alpha")
        })
        .ok_or_else(|| "expected render_pipeline call_presence seam".to_string())?;

    let evidence = evidence_for_seam(call_presence, &index);

    assert_eq!(evidence.reach.state, StageState::Yes);
    assert_eq!(evidence.activate.state, StageState::Unknown);
    assert!(
        !evidence
            .related_tests
            .iter()
            .any(|test| test.relation_reason == RelationReason::HelperOwnerCall),
        "short-circuiting dbg macro must not get helper-owner relation: {:?}",
        evidence.related_tests
    );
    assert!(
        evidence.observed_values.is_empty(),
        "short-circuiting dbg macro activation must not invent values: {:?}",
        evidence.observed_values
    );
    Ok(())
}

#[test]
fn given_call_presence_when_test_local_helper_wraps_owner_call_in_format_macro_then_activation_is_yes()
-> Result<(), String> {
    let prod = PathBuf::from("src/pipeline.rs");
    let prod_src = r#"
pub fn render_pipeline(input: &str) -> String {
    format_output(input)
}

fn format_output(input: &str) -> String {
    input.to_string()
}
"#;
    let tests = PathBuf::from("tests/pipeline_tests.rs");
    let tests_src = r#"
use pipeline::render_pipeline;

fn exercise_pipeline() -> String {
    format!("pipeline={}", render_pipeline("alpha"))
}

#[test]
fn helper_exercises_pipeline() {
    let output = exercise_pipeline();
    assert_eq!(output, "pipeline=alpha");
}
"#;
    let index = index_from_files(&[(prod, prod_src), (tests, tests_src)])?;
    let seams = inventory_seams_from_index(&[PathBuf::from("src/pipeline.rs")], &index);
    let call_presence = seams
        .iter()
        .find(|s| {
            s.kind() == SeamKind::CallPresence
                && s.owner().ends_with("::render_pipeline")
                && s.expression().contains("format_output")
        })
        .ok_or_else(|| "expected render_pipeline call_presence seam".to_string())?;

    let evidence = evidence_for_seam(call_presence, &index);

    assert_eq!(evidence.reach.state, StageState::Yes);
    assert_eq!(evidence.activate.state, StageState::Yes);
    assert!(
        evidence
            .related_tests
            .iter()
            .any(|test| test.relation_reason == RelationReason::HelperOwnerCall),
        "expected format macro helper owner-call relation, got {:?}",
        evidence.related_tests
    );
    assert!(
        evidence.observed_values.is_empty(),
        "format macro helper activation must not invent values: {:?}",
        evidence.observed_values
    );
    Ok(())
}

#[test]
fn given_call_presence_when_test_local_helper_wraps_owner_call_in_format_args_macro_then_activation_is_yes()
-> Result<(), String> {
    let prod = PathBuf::from("src/pipeline.rs");
    let prod_src = r#"
pub fn render_pipeline(input: &str) -> String {
    format_output(input)
}

fn format_output(input: &str) -> String {
    input.to_string()
}
"#;
    let tests = PathBuf::from("tests/pipeline_tests.rs");
    let tests_src = r#"
use pipeline::render_pipeline;

fn exercise_pipeline() -> String {
    std::fmt::format(format_args!("pipeline={}", render_pipeline("alpha")))
}

#[test]
fn helper_exercises_pipeline() {
    let output = exercise_pipeline();
    assert_eq!(output, "pipeline=alpha");
}
"#;
    let index = index_from_files(&[(prod, prod_src), (tests, tests_src)])?;
    let seams = inventory_seams_from_index(&[PathBuf::from("src/pipeline.rs")], &index);
    let call_presence = seams
        .iter()
        .find(|s| {
            s.kind() == SeamKind::CallPresence
                && s.owner().ends_with("::render_pipeline")
                && s.expression().contains("format_output")
        })
        .ok_or_else(|| "expected render_pipeline call_presence seam".to_string())?;

    let evidence = evidence_for_seam(call_presence, &index);

    assert_eq!(evidence.reach.state, StageState::Yes);
    assert_eq!(evidence.activate.state, StageState::Yes);
    assert!(
        evidence
            .related_tests
            .iter()
            .any(|test| test.relation_reason == RelationReason::HelperOwnerCall),
        "expected format_args macro helper owner-call relation, got {:?}",
        evidence.related_tests
    );
    assert!(
        evidence.observed_values.is_empty(),
        "format_args macro helper activation must not invent values: {:?}",
        evidence.observed_values
    );
    Ok(())
}

#[test]
fn given_call_presence_when_test_local_format_args_macro_short_circuits_owner_then_activation_stays_unknown()
-> Result<(), String> {
    let prod = PathBuf::from("src/pipeline.rs");
    let prod_src = r#"
pub fn render_pipeline(input: &str) -> String {
    format_output(input)
}

fn format_output(input: &str) -> String {
    input.to_string()
}
"#;
    let tests = PathBuf::from("tests/pipeline_tests.rs");
    let tests_src = r#"
use pipeline::render_pipeline;

fn exercise_pipeline(enabled: bool) -> String {
    std::fmt::format(format_args!("pipeline={}", enabled && render_pipeline("alpha").is_empty()))
}

#[test]
fn helper_exercises_pipeline() {
    let output = exercise_pipeline(true);
    assert_eq!(output, "pipeline=false");
}
"#;
    let index = index_from_files(&[(prod, prod_src), (tests, tests_src)])?;
    let seams = inventory_seams_from_index(&[PathBuf::from("src/pipeline.rs")], &index);
    let call_presence = seams
        .iter()
        .find(|s| {
            s.kind() == SeamKind::CallPresence
                && s.owner().ends_with("::render_pipeline")
                && s.expression().contains("format_output")
        })
        .ok_or_else(|| "expected render_pipeline call_presence seam".to_string())?;

    let evidence = evidence_for_seam(call_presence, &index);

    assert_eq!(evidence.reach.state, StageState::Yes);
    assert_eq!(evidence.activate.state, StageState::Unknown);
    assert!(
        !evidence
            .related_tests
            .iter()
            .any(|test| test.relation_reason == RelationReason::HelperOwnerCall),
        "short-circuiting format_args macro must not get helper-owner relation: {:?}",
        evidence.related_tests
    );
    assert!(
        evidence.observed_values.is_empty(),
        "short-circuiting format_args macro activation must not invent values: {:?}",
        evidence.observed_values
    );
    Ok(())
}

#[test]
fn given_call_presence_when_test_local_helper_wraps_owner_call_in_block_then_activation_is_yes()
-> Result<(), String> {
    let prod = PathBuf::from("src/pipeline.rs");
    let prod_src = r#"
pub fn render_pipeline(input: &str) -> String {
    format_output(input)
}

fn format_output(input: &str) -> String {
    input.to_string()
}
"#;
    let tests = PathBuf::from("tests/pipeline_tests.rs");
    let tests_src = r#"
use pipeline::render_pipeline;

fn exercise_pipeline() -> String {
    { render_pipeline("alpha") }
}

#[test]
fn helper_exercises_pipeline() {
    let output = exercise_pipeline();
    assert_eq!(output, "alpha");
}
"#;
    let index = index_from_files(&[(prod, prod_src), (tests, tests_src)])?;
    let seams = inventory_seams_from_index(&[PathBuf::from("src/pipeline.rs")], &index);
    let call_presence = seams
        .iter()
        .find(|s| {
            s.kind() == SeamKind::CallPresence
                && s.owner().ends_with("::render_pipeline")
                && s.expression().contains("format_output")
        })
        .ok_or_else(|| "expected render_pipeline call_presence seam".to_string())?;

    let evidence = evidence_for_seam(call_presence, &index);

    assert_eq!(evidence.reach.state, StageState::Yes);
    assert_eq!(evidence.activate.state, StageState::Yes);
    assert!(
        evidence
            .related_tests
            .iter()
            .any(|test| test.relation_reason == RelationReason::HelperOwnerCall),
        "expected block helper owner-call relation, got {:?}",
        evidence.related_tests
    );
    assert!(
        evidence.observed_values.is_empty(),
        "block helper activation must not invent values: {:?}",
        evidence.observed_values
    );
    Ok(())
}

#[test]
fn given_call_presence_when_test_local_helper_conditionally_calls_owner_then_activation_stays_unknown()
-> Result<(), String> {
    let prod = PathBuf::from("src/pipeline.rs");
    let prod_src = r#"
pub fn render_pipeline(input: &str) -> String {
    format_output(input)
}

fn format_output(input: &str) -> String {
    input.to_string()
}
"#;
    let tests = PathBuf::from("tests/pipeline_tests.rs");
    let tests_src = r#"
use pipeline::render_pipeline;

fn exercise_pipeline(enabled: bool) -> String {
    if enabled { render_pipeline("alpha") } else { "beta".to_string() }
}

#[test]
fn helper_exercises_pipeline() {
    let output = exercise_pipeline(true);
    assert_eq!(output, "alpha");
}
"#;
    let index = index_from_files(&[(prod, prod_src), (tests, tests_src)])?;
    let seams = inventory_seams_from_index(&[PathBuf::from("src/pipeline.rs")], &index);
    let call_presence = seams
        .iter()
        .find(|s| {
            s.kind() == SeamKind::CallPresence
                && s.owner().ends_with("::render_pipeline")
                && s.expression().contains("format_output")
        })
        .ok_or_else(|| "expected render_pipeline call_presence seam".to_string())?;

    let evidence = evidence_for_seam(call_presence, &index);

    assert_eq!(evidence.reach.state, StageState::Yes);
    assert_eq!(evidence.activate.state, StageState::Unknown);
    assert!(
        !evidence
            .related_tests
            .iter()
            .any(|test| test.relation_reason == RelationReason::HelperOwnerCall),
        "conditional block helper must not get helper-owner relation: {:?}",
        evidence.related_tests
    );
    assert!(
        evidence.observed_values.is_empty(),
        "conditional helper activation must not invent values: {:?}",
        evidence.observed_values
    );
    Ok(())
}

#[test]
fn given_call_presence_when_test_local_helper_wraps_owner_call_in_vec_macro_then_activation_is_yes()
-> Result<(), String> {
    let prod = PathBuf::from("src/pipeline.rs");
    let prod_src = r#"
pub fn render_pipeline(input: &str) -> String {
    format_output(input)
}

fn format_output(input: &str) -> String {
    input.to_string()
}
"#;
    let tests = PathBuf::from("tests/pipeline_tests.rs");
    let tests_src = r#"
use pipeline::render_pipeline;

fn exercise_pipeline() -> Vec<String> {
    vec![render_pipeline("alpha")]
}

#[test]
fn helper_exercises_pipeline() {
    let output = exercise_pipeline();
    assert_eq!(output[0], "alpha");
}
"#;
    let index = index_from_files(&[(prod, prod_src), (tests, tests_src)])?;
    let seams = inventory_seams_from_index(&[PathBuf::from("src/pipeline.rs")], &index);
    let call_presence = seams
        .iter()
        .find(|s| {
            s.kind() == SeamKind::CallPresence
                && s.owner().ends_with("::render_pipeline")
                && s.expression().contains("format_output")
        })
        .ok_or_else(|| "expected render_pipeline call_presence seam".to_string())?;

    let evidence = evidence_for_seam(call_presence, &index);

    assert_eq!(evidence.reach.state, StageState::Yes);
    assert_eq!(evidence.activate.state, StageState::Yes);
    assert!(
        evidence
            .related_tests
            .iter()
            .any(|test| test.relation_reason == RelationReason::HelperOwnerCall),
        "expected vec macro helper owner-call relation, got {:?}",
        evidence.related_tests
    );
    assert!(
        evidence.observed_values.is_empty(),
        "vec macro helper activation must not invent values: {:?}",
        evidence.observed_values
    );
    Ok(())
}

#[test]
fn given_call_presence_when_test_local_helper_wraps_owner_call_in_array_literal_then_activation_is_yes()
-> Result<(), String> {
    let prod = PathBuf::from("src/pipeline.rs");
    let prod_src = r#"
pub fn render_pipeline(input: &str) -> String {
    format_output(input)
}

fn format_output(input: &str) -> String {
    input.to_string()
}
"#;
    let tests = PathBuf::from("tests/pipeline_tests.rs");
    let tests_src = r#"
use pipeline::render_pipeline;

fn exercise_pipeline() -> [String; 1] {
    [render_pipeline("alpha")]
}

#[test]
fn helper_exercises_pipeline() {
    let output = exercise_pipeline();
    assert_eq!(output[0], "alpha");
}
"#;
    let index = index_from_files(&[(prod, prod_src), (tests, tests_src)])?;
    let seams = inventory_seams_from_index(&[PathBuf::from("src/pipeline.rs")], &index);
    let call_presence = seams
        .iter()
        .find(|s| {
            s.kind() == SeamKind::CallPresence
                && s.owner().ends_with("::render_pipeline")
                && s.expression().contains("format_output")
        })
        .ok_or_else(|| "expected render_pipeline call_presence seam".to_string())?;

    let evidence = evidence_for_seam(call_presence, &index);

    assert_eq!(evidence.reach.state, StageState::Yes);
    assert_eq!(evidence.activate.state, StageState::Yes);
    assert!(
        evidence
            .related_tests
            .iter()
            .any(|test| test.relation_reason == RelationReason::HelperOwnerCall),
        "expected array literal helper owner-call relation, got {:?}",
        evidence.related_tests
    );
    assert!(
        evidence.observed_values.is_empty(),
        "array literal helper activation must not invent values: {:?}",
        evidence.observed_values
    );
    Ok(())
}

#[test]
fn given_call_presence_when_test_local_helper_wraps_owner_call_in_tuple_literal_then_activation_is_yes()
-> Result<(), String> {
    let prod = PathBuf::from("src/pipeline.rs");
    let prod_src = r#"
pub fn render_pipeline(input: &str) -> String {
    format_output(input)
}

fn format_output(input: &str) -> String {
    input.to_string()
}
"#;
    let tests = PathBuf::from("tests/pipeline_tests.rs");
    let tests_src = r#"
use pipeline::render_pipeline;

fn exercise_pipeline() -> (String, bool) {
    (render_pipeline("alpha"), true)
}

#[test]
fn helper_exercises_pipeline() {
    let output = exercise_pipeline();
    assert_eq!(output.0, "alpha");
}
"#;
    let index = index_from_files(&[(prod, prod_src), (tests, tests_src)])?;
    let seams = inventory_seams_from_index(&[PathBuf::from("src/pipeline.rs")], &index);
    let call_presence = seams
        .iter()
        .find(|s| {
            s.kind() == SeamKind::CallPresence
                && s.owner().ends_with("::render_pipeline")
                && s.expression().contains("format_output")
        })
        .ok_or_else(|| "expected render_pipeline call_presence seam".to_string())?;

    let evidence = evidence_for_seam(call_presence, &index);

    assert_eq!(evidence.reach.state, StageState::Yes);
    assert_eq!(evidence.activate.state, StageState::Yes);
    assert!(
        evidence
            .related_tests
            .iter()
            .any(|test| test.relation_reason == RelationReason::HelperOwnerCall),
        "expected tuple literal helper owner-call relation, got {:?}",
        evidence.related_tests
    );
    assert!(
        evidence.observed_values.is_empty(),
        "tuple literal helper activation must not invent values: {:?}",
        evidence.observed_values
    );
    Ok(())
}

#[test]
fn given_call_presence_when_test_local_helper_wraps_owner_call_in_unknown_macro_then_activation_stays_unknown()
-> Result<(), String> {
    let prod = PathBuf::from("src/pipeline.rs");
    let prod_src = r#"
pub fn render_pipeline(input: &str) -> String {
    format_output(input)
}

fn format_output(input: &str) -> String {
    input.to_string()
}
"#;
    let tests = PathBuf::from("tests/pipeline_tests.rs");
    let tests_src = r#"
use pipeline::render_pipeline;

macro_rules! trace_value {
    ($value:expr) => {
        $value
    };
}

fn exercise_pipeline() -> String {
    trace_value!(render_pipeline("alpha"))
}

#[test]
fn helper_exercises_pipeline() {
    let output = exercise_pipeline();
    assert_eq!(output, "alpha");
}
"#;
    let index = index_from_files(&[(prod, prod_src), (tests, tests_src)])?;
    let seams = inventory_seams_from_index(&[PathBuf::from("src/pipeline.rs")], &index);
    let call_presence = seams
        .iter()
        .find(|s| {
            s.kind() == SeamKind::CallPresence
                && s.owner().ends_with("::render_pipeline")
                && s.expression().contains("format_output")
        })
        .ok_or_else(|| "expected render_pipeline call_presence seam".to_string())?;

    let evidence = evidence_for_seam(call_presence, &index);

    assert_eq!(evidence.reach.state, StageState::Yes);
    assert!(
        !evidence
            .related_tests
            .iter()
            .any(|test| test.relation_reason == RelationReason::HelperOwnerCall),
        "unknown macro wrapper must not get helper-owner relation: {:?}",
        evidence.related_tests
    );
    assert_eq!(evidence.activate.state, StageState::Unknown);
    assert!(
        evidence.observed_values.is_empty(),
        "unknown macro wrapper must not invent observed values: {:?}",
        evidence.observed_values
    );
    Ok(())
}

#[test]
fn given_call_presence_when_helper_wraps_owner_call_with_non_container_call_then_activation_stays_unknown()
-> Result<(), String> {
    let prod = PathBuf::from("src/pipeline.rs");
    let prod_src = r#"
pub fn render_pipeline(input: &str) -> String {
    format_output(input)
}

fn format_output(input: &str) -> String {
    input.to_string()
}
"#;
    let tests = PathBuf::from("tests/pipeline_tests.rs");
    let tests_src = r#"
use pipeline::render_pipeline;

fn decorate(input: String) -> String {
    input
}

fn exercise_pipeline() -> String {
    decorate(render_pipeline("alpha"))
}

#[test]
fn helper_exercises_pipeline() {
    let output = exercise_pipeline();
    assert_eq!(output, "alpha");
}
"#;
    let index = index_from_files(&[(prod, prod_src), (tests, tests_src)])?;
    let seams = inventory_seams_from_index(&[PathBuf::from("src/pipeline.rs")], &index);
    let call_presence = seams
        .iter()
        .find(|s| {
            s.kind() == SeamKind::CallPresence
                && s.owner().ends_with("::render_pipeline")
                && s.expression().contains("format_output")
        })
        .ok_or_else(|| "expected render_pipeline call_presence seam".to_string())?;

    let evidence = evidence_for_seam(call_presence, &index);

    assert_eq!(evidence.reach.state, StageState::Yes);
    assert!(
        !evidence
            .related_tests
            .iter()
            .any(|test| test.relation_reason == RelationReason::HelperOwnerCall),
        "non-container wrapper must not get helper-owner relation: {:?}",
        evidence.related_tests
    );
    assert_eq!(evidence.activate.state, StageState::Unknown);
    assert!(
        evidence.observed_values.is_empty(),
        "non-container wrapper must not invent observed values: {:?}",
        evidence.observed_values
    );
    Ok(())
}

#[test]
fn given_call_presence_when_test_local_helper_wraps_owner_call_in_std_identity_then_activation_is_yes()
-> Result<(), String> {
    for identity_path in [
        "std::convert::identity",
        "::std::convert::identity",
        "core::convert::identity",
        "::core::convert::identity",
    ] {
        let prod = PathBuf::from("src/pipeline.rs");
        let prod_src = r#"
pub fn render_pipeline(input: &str) -> String {
    format_output(input)
}

fn format_output(input: &str) -> String {
    input.to_string()
}
"#;
        let tests = PathBuf::from("tests/pipeline_tests.rs");
        let tests_src = r#"
use pipeline::render_pipeline;

fn exercise_pipeline() -> String {
    IDENTITY_PATH(render_pipeline("alpha"))
}

#[test]
fn helper_exercises_pipeline() {
    let output = exercise_pipeline();
    assert_eq!(output, "alpha");
}
"#
        .replace("IDENTITY_PATH", identity_path);
        let index = index_from_files(&[(prod, prod_src), (tests, tests_src.as_str())])?;
        let seams = inventory_seams_from_index(&[PathBuf::from("src/pipeline.rs")], &index);
        let call_presence = seams
            .iter()
            .find(|s| {
                s.kind() == SeamKind::CallPresence
                    && s.owner().ends_with("::render_pipeline")
                    && s.expression().contains("format_output")
            })
            .ok_or_else(|| {
                format!("expected render_pipeline call_presence seam for {identity_path}")
            })?;

        let evidence = evidence_for_seam(call_presence, &index);

        assert_eq!(evidence.reach.state, StageState::Yes);
        assert_eq!(evidence.activate.state, StageState::Yes);
        assert!(
            evidence
                .related_tests
                .iter()
                .any(|test| test.relation_reason == RelationReason::HelperOwnerCall),
            "expected {identity_path} helper owner-call relation, got {:?}",
            evidence.related_tests
        );
        assert!(
            evidence.observed_values.is_empty(),
            "{identity_path} helper activation must not invent observed values: {:?}",
            evidence.observed_values
        );
    }
    Ok(())
}

#[test]
fn given_call_presence_when_test_local_helper_wraps_owner_call_in_local_identity_then_activation_stays_unknown()
-> Result<(), String> {
    let prod = PathBuf::from("src/pipeline.rs");
    let prod_src = r#"
pub fn render_pipeline(input: &str) -> String {
    format_output(input)
}

fn format_output(input: &str) -> String {
    input.to_string()
}
"#;
    let tests = PathBuf::from("tests/pipeline_tests.rs");
    let tests_src = r#"
use pipeline::render_pipeline;

fn identity(input: String) -> String {
    input
}

fn exercise_pipeline() -> String {
    identity(render_pipeline("alpha"))
}

#[test]
fn helper_exercises_pipeline() {
    let output = exercise_pipeline();
    assert_eq!(output, "alpha");
}
"#;
    let index = index_from_files(&[(prod, prod_src), (tests, tests_src)])?;
    let seams = inventory_seams_from_index(&[PathBuf::from("src/pipeline.rs")], &index);
    let call_presence = seams
        .iter()
        .find(|s| {
            s.kind() == SeamKind::CallPresence
                && s.owner().ends_with("::render_pipeline")
                && s.expression().contains("format_output")
        })
        .ok_or_else(|| "expected render_pipeline call_presence seam".to_string())?;

    let evidence = evidence_for_seam(call_presence, &index);

    assert_eq!(evidence.reach.state, StageState::Yes);
    assert_eq!(evidence.activate.state, StageState::Unknown);
    assert!(
        !evidence
            .related_tests
            .iter()
            .any(|test| test.relation_reason == RelationReason::HelperOwnerCall),
        "local identity wrapper must not get helper-owner relation: {:?}",
        evidence.related_tests
    );
    assert!(
        evidence.observed_values.is_empty(),
        "local identity wrapper must not invent observed values: {:?}",
        evidence.observed_values
    );
    Ok(())
}

#[test]
fn given_call_presence_when_test_local_two_hop_helper_calls_owner_then_activation_is_yes()
-> Result<(), String> {
    let prod = PathBuf::from("src/pipeline.rs");
    let prod_src = r#"
pub fn render_pipeline(input: &str) -> String {
    format_output(input)
}

fn format_output(input: &str) -> String {
    input.to_string()
}
"#;
    let tests = PathBuf::from("tests/pipeline_tests.rs");
    let tests_src = r#"
use pipeline::render_pipeline;

fn exercise_pipeline() -> String {
    render_pipeline("alpha")
}

fn outer_pipeline() -> String {
    exercise_pipeline()
}

#[test]
fn outer_helper_reaches_pipeline_indirectly() {
    let output = outer_pipeline();
    assert_eq!(output, "alpha");
}
"#;
    let index = index_from_files(&[(prod, prod_src), (tests, tests_src)])?;
    let seams = inventory_seams_from_index(&[PathBuf::from("src/pipeline.rs")], &index);
    let call_presence = seams
        .iter()
        .find(|s| {
            s.kind() == SeamKind::CallPresence
                && s.owner().ends_with("::render_pipeline")
                && s.expression().contains("format_output")
        })
        .ok_or_else(|| "expected render_pipeline call_presence seam".to_string())?;

    let evidence = evidence_for_seam(call_presence, &index);

    assert_eq!(evidence.reach.state, StageState::Yes);
    assert!(
        evidence
            .related_tests
            .iter()
            .any(|test| test.relation_reason == RelationReason::HelperOwnerCall),
        "test-local two-hop helper should get bounded helper-owner relation: {:?}",
        evidence.related_tests
    );
    assert_eq!(evidence.activate.state, StageState::Yes);
    assert!(
        evidence
            .activate
            .summary
            .contains("helper owner call for value-insensitive seam"),
        "activation summary should explain the bounded helper owner-call route: {}",
        evidence.activate.summary
    );
    assert!(
        evidence.observed_values.is_empty(),
        "test-local two-hop helper must not invent observed values: {:?}",
        evidence.observed_values
    );
    Ok(())
}

#[test]
fn given_call_presence_when_same_file_three_hop_helper_reaches_owner_then_activation_is_yes()
-> Result<(), String> {
    let source = PathBuf::from("src/activation.rs");
    let source_src = r#"
pub fn activation_evidence(rows: &[Vec<String>], parameter: &str) -> Option<Vec<String>> {
    missing_discriminator_facts(rows, parameter)
}

fn missing_discriminator_facts(rows: &[Vec<String>], parameter: &str) -> Option<Vec<String>> {
    missing_boundary_discriminator(rows, parameter)
}

fn missing_boundary_discriminator(rows: &[Vec<String>], parameter: &str) -> Option<Vec<String>> {
    parameter_value_set(rows, parameter)
}

fn parameter_value_set(rows: &[Vec<String>], parameter: &str) -> Option<Vec<String>> {
    let values = observed_parameter_values(rows, parameter);
    if values.is_empty() { None } else { Some(values) }
}

fn observed_parameter_values(rows: &[Vec<String>], parameter: &str) -> Vec<String> {
    rows.iter()
        .flatten()
        .filter(|value| value.as_str() == parameter)
        .cloned()
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn activation_evidence_reports_parameter_value_set() {
        let rows = vec![vec!["amount".to_string()]];
        let values = activation_evidence(&rows, "amount");
        assert_eq!(values, Some(vec!["amount".to_string()]));
    }
}
"#;
    let index = index_from_files(&[(source, source_src)])?;
    let seams = inventory_seams_from_index(&[PathBuf::from("src/activation.rs")], &index);
    let call_presence = seams
        .iter()
        .find(|s| {
            s.kind() == SeamKind::CallPresence
                && s.owner().ends_with("::parameter_value_set")
                && s.expression()
                    .contains("observed_parameter_values(rows, parameter)")
        })
        .ok_or_else(|| "expected parameter_value_set call_presence seam".to_string())?;

    let evidence = evidence_for_seam(call_presence, &index);

    assert_eq!(evidence.reach.state, StageState::Yes);
    assert_eq!(evidence.activate.state, StageState::Yes);
    assert!(
        evidence
            .related_tests
            .iter()
            .any(|test| test.relation_reason == RelationReason::HelperOwnerCall),
        "expected bounded three-hop same-file helper owner-call relation; got {:?}",
        evidence.related_tests
    );
    assert!(
        evidence.observed_values.is_empty(),
        "three-hop same-file helper activation must not invent observed values: {:?}",
        evidence.observed_values
    );
    Ok(())
}

#[test]
fn given_call_presence_when_same_file_method_chain_owner_through_helpers_then_activation_is_yes()
-> Result<(), String> {
    let source = PathBuf::from("src/canonical_gap.rs");
    let source_src = r#"
enum RequiredDiscriminator {
    BoundaryValue { description: String },
    ErrorVariant { variant: String },
}

struct RepoSeam {
    discriminator: RequiredDiscriminator,
}

impl RepoSeam {
    fn required_discriminator(&self) -> &RequiredDiscriminator {
        &self.discriminator
    }
}

struct ClassifiedSeam {
    seam: RepoSeam,
}

fn canonical_gap_identity(entry: &ClassifiedSeam) -> String {
    missing_discriminator_key(entry)
}

fn missing_discriminator_key(entry: &ClassifiedSeam) -> String {
    required_discriminator_text(entry.seam.required_discriminator())
}

fn required_discriminator_text(discriminator: &RequiredDiscriminator) -> String {
    match discriminator {
        RequiredDiscriminator::BoundaryValue { description } => description.clone(),
        RequiredDiscriminator::ErrorVariant { variant } => variant.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canonical_gap_uses_required_discriminator_text() {
        let entry = ClassifiedSeam {
            seam: RepoSeam {
                discriminator: RequiredDiscriminator::BoundaryValue {
                    description: "threshold".to_string(),
                },
            },
        };
        assert_eq!(canonical_gap_identity(&entry), "threshold");
    }
}
"#;
    let index = index_from_files(&[(source, source_src)])?;
    let seams = inventory_seams_from_index(&[PathBuf::from("src/canonical_gap.rs")], &index);
    let call_presence = seams
        .iter()
        .find(|s| {
            s.kind() == SeamKind::CallPresence
                && s.owner().ends_with("::required_discriminator_text")
                && s.expression().contains("description.clone()")
        })
        .ok_or_else(|| "expected required_discriminator_text call_presence seam".to_string())?;

    let evidence = evidence_for_seam(call_presence, &index);

    assert_eq!(evidence.reach.state, StageState::Yes);
    assert_eq!(evidence.activate.state, StageState::Yes);
    assert!(
        evidence
            .related_tests
            .iter()
            .any(|test| test.relation_reason == RelationReason::HelperOwnerCall),
        "expected same-file method-chain helper owner-call relation; got {:?}",
        evidence.related_tests
    );
    assert!(
        evidence.observed_values.is_empty(),
        "method-chain helper activation must not invent observed values: {:?}",
        evidence.observed_values
    );
    Ok(())
}

#[test]
fn given_call_presence_when_same_file_condition_helper_calls_owner_then_activation_is_yes()
-> Result<(), String> {
    let source = PathBuf::from("src/flow.rs");
    let source_src = r#"
fn effect_sink_kind(text: &str) -> &'static str {
    if looks_like_event_call_effect(text) {
        "event"
    } else if looks_like_log_effect(text) {
        "log"
    } else {
        "call"
    }
}

fn looks_like_log_effect(text: &str) -> bool {
    text.contains("log")
}

fn looks_like_event_call_effect(text: &str) -> bool {
    [".publish(", ".emit("]
        .iter()
        .any(|needle| text.contains(needle))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn effect_sink_detects_event_call() {
        assert_eq!(effect_sink_kind("bus.emit(value)"), "event");
    }
}
"#;
    let index = index_from_files(&[(source, source_src)])?;
    let seams = inventory_seams_from_index(&[PathBuf::from("src/flow.rs")], &index);
    let call_presence = seams
        .iter()
        .find(|s| {
            s.kind() == SeamKind::CallPresence
                && s.owner().ends_with("::looks_like_event_call_effect")
                && s.expression().contains(".iter()")
        })
        .ok_or_else(|| "expected looks_like_event_call_effect call_presence seam".to_string())?;

    let evidence = evidence_for_seam(call_presence, &index);

    assert_eq!(evidence.reach.state, StageState::Yes);
    assert_eq!(evidence.activate.state, StageState::Yes);
    assert!(
        evidence
            .related_tests
            .iter()
            .any(|test| test.relation_reason == RelationReason::HelperOwnerCall),
        "same-file condition helper should get helper-owner relation: {:?}",
        evidence.related_tests
    );
    assert!(
        evidence.observed_values.is_empty(),
        "condition helper activation must not invent observed values: {:?}",
        evidence.observed_values
    );
    assert!(
        evidence.missing_discriminators.is_empty(),
        "condition helper activation must not create boundary debt: {:?}",
        evidence.missing_discriminators
    );
    Ok(())
}

#[test]
fn given_call_presence_when_same_file_condition_helper_calls_owner_method_then_activation_is_yes()
-> Result<(), String> {
    let source = PathBuf::from("src/pipeline.rs");
    let source_src = r#"
struct Pipeline;

impl Pipeline {
    fn render_pipeline(&self, input: &str) -> String {
        input.trim().to_string()
    }
}

fn should_render_pipeline(input: &str) -> bool {
    let pipeline = Pipeline;
    if pipeline.render_pipeline(input) == "alpha" {
        true
    } else {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn condition_wrapper_exercises_pipeline_method() {
        assert!(should_render_pipeline(" alpha "));
    }
}
"#;
    let index = index_from_files(&[(source, source_src)])?;
    let seams = inventory_seams_from_index(&[PathBuf::from("src/pipeline.rs")], &index);
    let call_presence = seams
        .iter()
        .find(|s| {
            s.kind() == SeamKind::CallPresence
                && s.owner().ends_with("::impl Pipeline::render_pipeline")
                && s.expression().contains("trim")
        })
        .ok_or_else(|| "expected render_pipeline method call_presence seam".to_string())?;

    let evidence = evidence_for_seam(call_presence, &index);

    assert_eq!(evidence.reach.state, StageState::Yes);
    assert_eq!(evidence.activate.state, StageState::Yes);
    assert!(
        evidence
            .related_tests
            .iter()
            .any(|test| test.relation_reason == RelationReason::HelperOwnerCall),
        "expected conditional receiver-method helper owner-call relation, got {:?}",
        evidence.related_tests
    );
    assert!(
        evidence.observed_values.is_empty(),
        "conditional receiver-method route must not invent observed values: {:?}",
        evidence.observed_values
    );
    Ok(())
}

#[test]
fn given_call_presence_when_same_file_negated_condition_helper_calls_owner_then_activation_is_yes()
-> Result<(), String> {
    let source = PathBuf::from("src/pipeline.rs");
    let source_src = r#"
fn should_skip_pipeline(input: &str) -> bool {
    if !render_pipeline(input) {
        return true;
    }
    false
}

fn render_pipeline(input: &str) -> bool {
    format_output(input).is_empty()
}

fn format_output(input: &str) -> String {
    input.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn skip_helper_reaches_pipeline_owner() {
        assert!(!should_skip_pipeline("alpha"));
    }
}
"#;
    let index = index_from_files(&[(source, source_src)])?;
    let seams = inventory_seams_from_index(&[PathBuf::from("src/pipeline.rs")], &index);
    let call_presence = seams
        .iter()
        .find(|s| {
            s.kind() == SeamKind::CallPresence
                && s.owner().ends_with("::render_pipeline")
                && s.expression().contains("format_output")
        })
        .ok_or_else(|| "expected render_pipeline call_presence seam".to_string())?;

    let evidence = evidence_for_seam(call_presence, &index);

    assert_eq!(evidence.reach.state, StageState::Yes);
    assert_eq!(evidence.activate.state, StageState::Yes);
    assert!(
        evidence
            .related_tests
            .iter()
            .any(|test| test.relation_reason == RelationReason::HelperOwnerCall),
        "negated condition helper should get helper-owner relation: {:?}",
        evidence.related_tests
    );
    assert!(
        evidence.observed_values.is_empty(),
        "negated condition helper route must not invent observed values: {:?}",
        evidence.observed_values
    );
    Ok(())
}

#[test]
fn given_call_presence_when_same_file_else_if_condition_helper_is_skipped_then_activation_stays_unknown()
-> Result<(), String> {
    let source = PathBuf::from("src/flow.rs");
    let source_src = r#"
fn effect_sink_kind(text: &str) -> &'static str {
    if looks_like_log_effect(text) {
        "log"
    } else if looks_like_event_call_effect(text) {
        "event"
    } else {
        "call"
    }
}

fn looks_like_log_effect(text: &str) -> bool {
    text.contains("log")
}

fn looks_like_event_call_effect(text: &str) -> bool {
    [".publish(", ".emit("]
        .iter()
        .any(|needle| text.contains(needle))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn effect_sink_detects_log_call() {
        assert_eq!(effect_sink_kind("write log line"), "log");
    }
}
"#;
    let index = index_from_files(&[(source, source_src)])?;
    let seams = inventory_seams_from_index(&[PathBuf::from("src/flow.rs")], &index);
    let call_presence = seams
        .iter()
        .find(|s| {
            s.kind() == SeamKind::CallPresence
                && s.owner().ends_with("::looks_like_event_call_effect")
                && s.expression().contains(".iter()")
        })
        .ok_or_else(|| "expected looks_like_event_call_effect call_presence seam".to_string())?;

    let evidence = evidence_for_seam(call_presence, &index);

    assert_eq!(evidence.reach.state, StageState::Yes);
    assert_eq!(evidence.activate.state, StageState::Unknown);
    assert!(
        !evidence
            .related_tests
            .iter()
            .any(|test| test.relation_reason == RelationReason::HelperOwnerCall),
        "skipped else-if helper must not get helper-owner relation: {:?}",
        evidence.related_tests
    );
    assert!(
        evidence.observed_values.is_empty(),
        "skipped else-if helper must not invent observed values: {:?}",
        evidence.observed_values
    );
    Ok(())
}

#[test]
fn given_call_presence_when_same_file_helper_clones_owner_result_then_activation_is_yes()
-> Result<(), String> {
    let source = PathBuf::from("src/activation.rs");
    let source_src = r#"
#[derive(Clone)]
struct FlowSinkFact {
    kind: FlowSinkKind,
}

#[derive(PartialEq)]
enum FlowSinkKind {
    Unknown,
    Return,
}

struct MissingDiscriminatorFact {
    flow_sink: Option<FlowSinkFact>,
}

fn activation_evidence(flow_sinks: &[FlowSinkFact]) -> Option<MissingDiscriminatorFact> {
    missing_boundary_discriminator(flow_sinks)
}

fn missing_boundary_discriminator(flow_sinks: &[FlowSinkFact]) -> Option<MissingDiscriminatorFact> {
    Some(MissingDiscriminatorFact {
        flow_sink: first_visible_flow_sink(flow_sinks).cloned(),
    })
}

fn first_visible_flow_sink(flow_sinks: &[FlowSinkFact]) -> Option<&FlowSinkFact> {
    flow_sinks
        .iter()
        .find(|sink| sink.kind != FlowSinkKind::Unknown)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn activation_evidence_reports_visible_flow_sink() {
        let sinks = vec![FlowSinkFact { kind: FlowSinkKind::Return }];
        let missing = activation_evidence(&sinks);
        assert!(missing.and_then(|fact| fact.flow_sink).is_some());
    }
}
"#;
    let index = index_from_files(&[(source, source_src)])?;
    let seams = inventory_seams_from_index(&[PathBuf::from("src/activation.rs")], &index);
    let call_presence = seams
        .iter()
        .find(|s| {
            s.kind() == SeamKind::CallPresence
                && s.owner().ends_with("::first_visible_flow_sink")
                && s.expression().contains("flow_sinks")
                && s.expression().contains(".iter()")
        })
        .ok_or_else(|| "expected first_visible_flow_sink call_presence seam".to_string())?;

    let evidence = evidence_for_seam(call_presence, &index);

    assert_eq!(evidence.reach.state, StageState::Yes);
    assert_eq!(evidence.activate.state, StageState::Yes);
    assert!(
        evidence
            .related_tests
            .iter()
            .any(|test| test.relation_reason == RelationReason::HelperOwnerCall),
        "same-file helper with cloned owner result should get helper-owner relation: {:?}",
        evidence.related_tests
    );
    assert!(
        evidence.observed_values.is_empty(),
        "cloned owner-result helper must not invent observed values: {:?}",
        evidence.observed_values
    );
    Ok(())
}

#[test]
fn given_call_presence_when_same_file_wrapper_skips_owner_then_activation_stays_unknown()
-> Result<(), String> {
    let source = PathBuf::from("src/pipeline.rs");
    let source_src = r#"
fn render_pipeline(input: &str) -> String {
    format_output(input)
}

fn render_pipeline_fixture() -> String {
    format_output("alpha")
}

fn format_output(input: &str) -> String {
    input.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wrapper_mentions_pipeline_name_without_calling_owner() {
        let output = render_pipeline_fixture();
        assert_eq!(output, "alpha");
    }
}
"#;
    let index = index_from_files(&[(source, source_src)])?;
    let seams = inventory_seams_from_index(&[PathBuf::from("src/pipeline.rs")], &index);
    let call_presence = seams
        .iter()
        .find(|s| {
            s.kind() == SeamKind::CallPresence
                && s.owner().ends_with("::render_pipeline")
                && s.expression().contains("format_output")
        })
        .ok_or_else(|| "expected render_pipeline call_presence seam".to_string())?;

    let evidence = evidence_for_seam(call_presence, &index);

    assert_eq!(evidence.reach.state, StageState::Yes);
    assert!(
        !evidence
            .related_tests
            .iter()
            .any(|test| test.relation_reason == RelationReason::HelperOwnerCall),
        "wrapper that skips the owner must not get helper-owner relation: {:?}",
        evidence.related_tests
    );
    assert_eq!(evidence.activate.state, StageState::Unknown);
    assert!(
        evidence
            .activate
            .summary
            .contains("No direct owner call observed for value-insensitive seam"),
        "activation summary should keep owner-call limitation, got {}",
        evidence.activate.summary
    );
    Ok(())
}

#[test]
fn given_call_presence_when_same_file_unit_parent_qualified_wrapper_calls_owner_then_activation_is_yes()
-> Result<(), String> {
    let source = PathBuf::from("src/pipeline.rs");
    let source_src = r#"
fn render_pipeline(input: &str) -> String {
    format_output(input)
}

fn exercise_pipeline() -> String {
    render_pipeline("alpha")
}

fn format_output(input: &str) -> String {
    input.to_string()
}

#[cfg(test)]
mod tests {
    #[test]
    fn parent_qualified_wrapper_reaches_pipeline() {
        let output = super::exercise_pipeline();
        assert_eq!(output, "alpha");
    }
}
"#;
    let index = index_from_files(&[(source, source_src)])?;
    let seams = inventory_seams_from_index(&[PathBuf::from("src/pipeline.rs")], &index);
    let call_presence = seams
        .iter()
        .find(|s| {
            s.kind() == SeamKind::CallPresence
                && s.owner().ends_with("::render_pipeline")
                && s.expression().contains("format_output")
        })
        .ok_or_else(|| "expected render_pipeline call_presence seam".to_string())?;

    let evidence = evidence_for_seam(call_presence, &index);

    assert_eq!(evidence.reach.state, StageState::Yes);
    assert_eq!(evidence.activate.state, StageState::Yes);
    assert!(
        evidence
            .related_tests
            .iter()
            .any(|test| test.relation_reason == RelationReason::HelperOwnerCall),
        "parent-qualified same-file unit helper should get helper-owner relation: {:?}",
        evidence.related_tests
    );
    assert!(
        evidence
            .activate
            .summary
            .contains("helper owner call for value-insensitive seam"),
        "activation summary should explain the parent-qualified helper route: {}",
        evidence.activate.summary
    );
    assert!(
        evidence.observed_values.is_empty(),
        "parent-qualified same-file unit helper must not invent observed values: {:?}",
        evidence.observed_values
    );
    Ok(())
}

#[test]
fn given_call_presence_when_same_file_unit_shadow_calls_wrapper_name_then_activation_stays_unknown()
-> Result<(), String> {
    let source = PathBuf::from("src/pipeline.rs");
    let source_src = r#"
fn render_pipeline(input: &str) -> String {
    format_output(input)
}

fn exercise_pipeline() -> String {
    render_pipeline("alpha")
}

fn format_output(input: &str) -> String {
    input.to_string()
}

#[cfg(test)]
mod tests {
    fn exercise_pipeline() -> String {
        "shadow".to_string()
    }

    #[test]
    fn bare_shadowed_wrapper_name_does_not_reach_pipeline() {
        let output = exercise_pipeline();
        assert_eq!(output, "shadow");
    }
}
"#;
    let index = index_from_files(&[(source, source_src)])?;
    let seams = inventory_seams_from_index(&[PathBuf::from("src/pipeline.rs")], &index);
    let call_presence = seams
        .iter()
        .find(|s| {
            s.kind() == SeamKind::CallPresence
                && s.owner().ends_with("::render_pipeline")
                && s.expression().contains("format_output")
        })
        .ok_or_else(|| "expected render_pipeline call_presence seam".to_string())?;

    let evidence = evidence_for_seam(call_presence, &index);

    assert_eq!(evidence.reach.state, StageState::Yes);
    assert_eq!(evidence.activate.state, StageState::Unknown);
    assert!(
        !evidence
            .related_tests
            .iter()
            .any(|test| test.relation_reason == RelationReason::HelperOwnerCall),
        "test-local shadow must not inherit production wrapper owner relation: {:?}",
        evidence.related_tests
    );
    assert!(
        evidence
            .activate
            .summary
            .contains("No direct owner call observed for value-insensitive seam"),
        "activation summary should keep owner-call limitation, got {}",
        evidence.activate.summary
    );
    assert!(
        evidence.observed_values.is_empty(),
        "test-local shadow must not invent observed values: {:?}",
        evidence.observed_values
    );
    Ok(())
}

#[test]
fn given_call_presence_when_test_calls_two_hop_production_wrapper_then_activation_is_yes()
-> Result<(), String> {
    let source = PathBuf::from("src/pipeline.rs");
    let source_src = r#"
fn render_pipeline(input: &str) -> String {
    format_output(input)
}

fn exercise_pipeline() -> String {
    render_pipeline("alpha")
}

fn outer_pipeline() -> String {
    exercise_pipeline()
}

fn format_output(input: &str) -> String {
    input.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn outer_wrapper_reaches_pipeline_indirectly() {
        let output = outer_pipeline();
        assert_eq!(output, "alpha");
    }
}
"#;
    let index = index_from_files(&[(source, source_src)])?;
    let seams = inventory_seams_from_index(&[PathBuf::from("src/pipeline.rs")], &index);
    let call_presence = seams
        .iter()
        .find(|s| {
            s.kind() == SeamKind::CallPresence
                && s.owner().ends_with("::render_pipeline")
                && s.expression().contains("format_output")
        })
        .ok_or_else(|| "expected render_pipeline call_presence seam".to_string())?;

    let evidence = evidence_for_seam(call_presence, &index);

    assert_eq!(evidence.reach.state, StageState::Yes);
    assert!(
        evidence
            .related_tests
            .iter()
            .any(|test| test.relation_reason == RelationReason::HelperOwnerCall),
        "two-hop production wrapper should get helper-owner relation: {:?}",
        evidence.related_tests
    );
    assert_eq!(evidence.activate.state, StageState::Yes);
    assert!(
        evidence.observed_values.is_empty(),
        "two-hop wrapper must not invent observed values: {:?}",
        evidence.observed_values
    );
    Ok(())
}

#[test]
fn given_call_presence_when_production_wrapper_imports_owner_helper_then_activation_is_yes()
-> Result<(), String> {
    let path_file = PathBuf::from("src/parser/path.rs");
    let path_src = r#"
use std::path::PathBuf;

pub fn parse_new_path_marker(raw: &str) -> Option<PathBuf> {
    let path = parse_diff_path_token(raw)?;
    Some(PathBuf::from(path))
}

fn parse_diff_path_token(raw: &str) -> Option<String> {
    let quoted = raw.strip_prefix('"')?;
    parse_c_quoted_path(quoted)
}

fn parse_c_quoted_path(raw: &str) -> Option<String> {
    let mut chars = raw.chars().peekable();
    let ch = chars.next()?;
    match ch {
        '"' => Some(String::new()),
        '\\' => Some(parse_c_escape(&mut chars).to_string()),
        _ => Some(ch.to_string()),
    }
}

fn parse_c_escape<I>(chars: &mut std::iter::Peekable<I>) -> char
where
    I: Iterator<Item = char>,
{
    chars.next().unwrap_or('\\')
}
"#;
    let parse_file = PathBuf::from("src/parser/parse.rs");
    let parse_src = r#"
use std::path::PathBuf;
use crate::parser::path::parse_new_path_marker;

pub fn parse_unified_diff(raw: &str) -> Option<PathBuf> {
    parse_line(raw)
}

fn parse_line(raw: &str) -> Option<PathBuf> {
    parse_new_path_marker(raw)
}
"#;
    let tests = PathBuf::from("tests/parser_tests.rs");
    let tests_src = r#"
use parser::parse::parse_unified_diff;

#[test]
fn quoted_path_reaches_path_parser() {
    assert_eq!(parse_unified_diff("\"src/lib.rs\"").unwrap(), std::path::PathBuf::from("src/lib.rs"));
}
"#;
    let index = index_from_files(&[
        (path_file, path_src),
        (parse_file, parse_src),
        (tests, tests_src),
    ])?;
    let seams = inventory_seams_from_index(&[PathBuf::from("src/parser/path.rs")], &index);
    let call_presence = seams
        .iter()
        .find(|s| {
            s.kind() == SeamKind::CallPresence
                && s.owner().ends_with("::parse_c_quoted_path")
                && s.expression().contains("parse_c_escape")
        })
        .ok_or_else(|| "expected parse_c_quoted_path call_presence seam".to_string())?;

    let evidence = evidence_for_seam(call_presence, &index);

    assert_eq!(evidence.reach.state, StageState::Yes);
    assert_eq!(evidence.activate.state, StageState::Yes);
    assert!(
        evidence
            .related_tests
            .iter()
            .any(|test| test.relation_reason == RelationReason::HelperOwnerCall),
        "imported production helper chain should get helper-owner relation: {:?}",
        evidence.related_tests
    );
    assert!(
        evidence.observed_values.is_empty(),
        "imported production helper activation must not invent observed values: {:?}",
        evidence.observed_values
    );
    Ok(())
}

#[test]
fn given_call_presence_when_test_calls_same_file_fanout_wrapper_then_activation_is_yes()
-> Result<(), String> {
    let source = PathBuf::from("src/pipeline.rs");
    let source_src = r#"
fn render_pipeline(input: &str) -> String {
    format_output(input)
}

fn collect_pipeline_context() -> String {
    "context".to_string()
}

fn build_pipeline_report() -> String {
    let context = collect_pipeline_context();
    let output = render_pipeline("alpha");
    format!("{context}:{output}")
}

fn format_output(input: &str) -> String {
    input.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fanout_wrapper_reaches_pipeline_indirectly() {
        let output = build_pipeline_report();
        assert!(output.ends_with(":alpha"));
    }
}
"#;
    let index = index_from_files(&[(source, source_src)])?;
    let seams = inventory_seams_from_index(&[PathBuf::from("src/pipeline.rs")], &index);
    let call_presence = seams
        .iter()
        .find(|s| {
            s.kind() == SeamKind::CallPresence
                && s.owner().ends_with("::render_pipeline")
                && s.expression().contains("format_output")
        })
        .ok_or_else(|| "expected render_pipeline call_presence seam".to_string())?;

    let evidence = evidence_for_seam(call_presence, &index);

    assert_eq!(evidence.reach.state, StageState::Yes);
    assert!(
        evidence
            .related_tests
            .iter()
            .any(|test| test.relation_reason == RelationReason::HelperOwnerCall),
        "fanout production wrapper should get helper-owner relation: {:?}",
        evidence.related_tests
    );
    assert_eq!(evidence.activate.state, StageState::Yes);
    assert!(
        evidence
            .activate
            .summary
            .contains("helper owner call for value-insensitive seam"),
        "activation summary should explain the fanout helper owner-call route: {}",
        evidence.activate.summary
    );
    assert!(
        evidence.observed_values.is_empty(),
        "fanout helper route must not invent observed values: {:?}",
        evidence.observed_values
    );
    Ok(())
}

#[test]
fn given_call_presence_when_same_file_wrapper_extends_owner_call_then_activation_is_yes()
-> Result<(), String> {
    let source = PathBuf::from("src/pipeline.rs");
    let source_src = r#"
fn render_pipeline(input: &str) -> Option<String> {
    Some(format_output(input))
}

fn collect_pipeline_outputs(input: &str) -> Vec<String> {
    let mut outputs = Vec::new();
    outputs.extend(render_pipeline(input));
    outputs
}

fn format_output(input: &str) -> String {
    input.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extend_wrapper_reaches_pipeline_owner() {
        let outputs = collect_pipeline_outputs("alpha");
        assert_eq!(outputs, vec!["alpha".to_string()]);
    }
}
"#;
    let index = index_from_files(&[(source, source_src)])?;
    let seams = inventory_seams_from_index(&[PathBuf::from("src/pipeline.rs")], &index);
    let call_presence = seams
        .iter()
        .find(|s| {
            s.kind() == SeamKind::CallPresence
                && s.owner().ends_with("::render_pipeline")
                && s.expression().contains("format_output")
        })
        .ok_or_else(|| "expected render_pipeline call_presence seam".to_string())?;

    let evidence = evidence_for_seam(call_presence, &index);

    assert_eq!(evidence.reach.state, StageState::Yes);
    assert_eq!(evidence.activate.state, StageState::Yes);
    assert!(
        evidence
            .related_tests
            .iter()
            .any(|test| test.relation_reason == RelationReason::HelperOwnerCall),
        "extend wrapper should get helper-owner relation: {:?}",
        evidence.related_tests
    );
    assert!(
        evidence.observed_values.is_empty(),
        "extend wrapper route must not invent observed values: {:?}",
        evidence.observed_values
    );
    Ok(())
}

#[test]
fn given_call_presence_when_same_file_wrapper_unwraps_or_defaults_owner_then_activation_is_yes()
-> Result<(), String> {
    let source = PathBuf::from("src/pipeline.rs");
    let source_src = r#"
fn render_pipeline(input: &str) -> Option<String> {
    Some(format_output(input))
}

fn collect_pipeline_output(input: &str) -> String {
    render_pipeline(input).unwrap_or_default()
}

fn format_output(input: &str) -> String {
    input.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unwrap_or_default_wrapper_reaches_pipeline_owner() {
        let output = collect_pipeline_output("alpha");
        assert_eq!(output, "alpha");
    }
}
"#;
    let index = index_from_files(&[(source, source_src)])?;
    let seams = inventory_seams_from_index(&[PathBuf::from("src/pipeline.rs")], &index);
    let call_presence = seams
        .iter()
        .find(|s| {
            s.kind() == SeamKind::CallPresence
                && s.owner().ends_with("::render_pipeline")
                && s.expression().contains("format_output")
        })
        .ok_or_else(|| "expected render_pipeline call_presence seam".to_string())?;

    let evidence = evidence_for_seam(call_presence, &index);

    assert_eq!(evidence.reach.state, StageState::Yes);
    assert_eq!(evidence.activate.state, StageState::Yes);
    assert!(
        evidence
            .related_tests
            .iter()
            .any(|test| test.relation_reason == RelationReason::HelperOwnerCall),
        "unwrap_or_default wrapper should get helper-owner relation: {:?}",
        evidence.related_tests
    );
    assert!(
        evidence.observed_values.is_empty(),
        "unwrap_or_default wrapper route must not invent observed values: {:?}",
        evidence.observed_values
    );
    Ok(())
}

#[test]
fn given_related_tests_with_same_confidence_when_sorted_then_order_is_stable_by_file_name_line()
-> Result<(), String> {
    // Two tests with the same reason (both owner_named_test) but
    // different (file, name). Sort tie-break must be deterministic:
    // file → name → line.
    let prod_src = "pub fn discounted_total(amount: i32, threshold: i32) -> i32 \
                        { if amount >= threshold { amount - 10 } else { amount } }\n";
    let test_a = (
        "tests/zeta.rs",
        "#[test] fn discounted_total_one() { assert_eq!(1, 1); }\n",
    );
    let test_b = (
        "tests/alpha.rs",
        "#[test] fn discounted_total_two() { assert_eq!(1, 1); }\n",
    );
    let files: Vec<(PathBuf, &str)> = vec![
        (PathBuf::from("src/pricing.rs"), prod_src),
        (PathBuf::from(test_a.0), test_a.1),
        (PathBuf::from(test_b.0), test_b.1),
    ];
    let index = index_from_files(&files)?;
    let seams = inventory_seams_from_index(&[PathBuf::from("src/pricing.rs")], &index);
    let predicate = seams
        .iter()
        .find(|s| s.kind() == SeamKind::PredicateBoundary)
        .ok_or_else(|| "predicate seam present".to_string())?;
    let evidence = evidence_for_seam(predicate, &index);
    assert!(
        evidence.related_tests.len() >= 2,
        "expected at least 2 related tests, got {}",
        evidence.related_tests.len()
    );
    // alpha.rs sorts before zeta.rs.
    assert_eq!(evidence.related_tests[0].file, Path::new("tests/alpha.rs"));
    assert_eq!(evidence.related_tests[1].file, Path::new("tests/zeta.rs"));
    Ok(())
}

#[test]
fn given_higher_confidence_related_test_when_sorted_then_it_comes_before_lower_confidence()
-> Result<(), String> {
    // Two tests, one with high confidence (direct_owner_call) and
    // one with low confidence (fixture_owner_affinity via a fixture
    // helper). High must come first regardless of file/name order.
    let prod_src = "pub fn discounted_total(amount: i32, threshold: i32) -> i32 \
                        { if amount >= threshold { amount - 10 } else { amount } }\n\
                        pub fn make_quote() -> i32 { 100 }\n";
    // The fixture user lives in 'a_first.rs' (alphabetically before)
    // so without confidence ordering it would naively sort first.
    let fixture_user = (
        "tests/a_first.rs",
        "#[test] fn fx() { let _ = make_quote(); assert!(true); }\n",
    );
    // The direct caller lives in 'z_last.rs'.
    let direct_caller = (
        "tests/z_last.rs",
        "#[test] fn caller() { assert_eq!(discounted_total(100, 100), 90); }\n",
    );
    let files: Vec<(PathBuf, &str)> = vec![
        (PathBuf::from("src/pricing.rs"), prod_src),
        (PathBuf::from(fixture_user.0), fixture_user.1),
        (PathBuf::from(direct_caller.0), direct_caller.1),
    ];
    let index = index_from_files(&files)?;
    let seams = inventory_seams_from_index(&[PathBuf::from("src/pricing.rs")], &index);
    let predicate = seams
        .iter()
        .find(|s| s.kind() == SeamKind::PredicateBoundary)
        .ok_or_else(|| "predicate seam present".to_string())?;
    let evidence = evidence_for_seam(predicate, &index);
    let first = evidence
        .related_tests
        .first()
        .ok_or_else(|| "at least one related test".to_string())?;
    assert_eq!(first.relation_reason, RelationReason::DirectOwnerCall);
    assert_eq!(first.relation_confidence, RelationConfidence::High);
    Ok(())
}

#[test]
fn given_related_tests_with_same_relation_when_ranked_then_strong_oracle_precedes_smoke_oracle()
-> Result<(), String> {
    // Both tests are direct owner calls. The strong exact-value
    // oracle lives in an alphabetically later file, so the v2
    // ranking must use oracle strength before file/name tie-breaks.
    let prod_src = "pub fn discounted_total(amount: i32, threshold: i32) -> Result<i32, ()> \
                        { if amount >= threshold { Ok(amount - 10) } else { Ok(amount) } }\n";
    let smoke = (
        "tests/a_smoke.rs",
        "#[test] fn smoke_owner_call() { assert!(discounted_total(100, 100).is_ok()); }\n",
    );
    let strong = (
        "tests/z_exact.rs",
        "#[test] fn exact_owner_call() { assert_eq!(discounted_total(100, 100).unwrap(), 90); }\n",
    );
    let files: Vec<(PathBuf, &str)> = vec![
        (PathBuf::from("src/pricing.rs"), prod_src),
        (PathBuf::from(smoke.0), smoke.1),
        (PathBuf::from(strong.0), strong.1),
    ];
    let index = index_from_files(&files)?;
    let seams = inventory_seams_from_index(&[PathBuf::from("src/pricing.rs")], &index);
    let predicate = seams
        .iter()
        .find(|s| s.kind() == SeamKind::PredicateBoundary)
        .ok_or_else(|| "predicate seam present".to_string())?;
    let evidence = evidence_for_seam(predicate, &index);
    let first = evidence
        .related_tests
        .first()
        .ok_or_else(|| "at least one related test".to_string())?;

    assert_eq!(first.test_name, "exact_owner_call");
    assert_eq!(first.relation_reason, RelationReason::DirectOwnerCall);
    assert_eq!(first.oracle_strength, OracleStrength::Strong);
    Ok(())
}

#[test]
fn given_related_tests_with_same_relation_and_oracle_when_ranked_then_activation_overlap_precedes_file_order()
-> Result<(), String> {
    // Both tests are direct owner calls with strong exact-value
    // oracles. The equality-boundary call lives in an
    // alphabetically later file; it should still be the nearest
    // imitation target because its activation values overlap the
    // predicate boundary.
    let prod_src = "pub fn discounted_total(amount: i32, threshold: i32) -> i32 \
                        { if amount >= threshold { amount - 10 } else { amount } }\n";
    let above = (
        "tests/a_above.rs",
        "#[test] fn above_boundary() { let actual = discounted_total(101, 100); assert_eq!(actual, 91); }\n",
    );
    let equality = (
        "tests/z_equal.rs",
        "#[test] fn equality_boundary() { let actual = discounted_total(100, 100); assert_eq!(actual, 90); }\n",
    );
    let files: Vec<(PathBuf, &str)> = vec![
        (PathBuf::from("src/pricing.rs"), prod_src),
        (PathBuf::from(above.0), above.1),
        (PathBuf::from(equality.0), equality.1),
    ];
    let index = index_from_files(&files)?;
    let seams = inventory_seams_from_index(&[PathBuf::from("src/pricing.rs")], &index);
    let predicate = seams
        .iter()
        .find(|s| s.kind() == SeamKind::PredicateBoundary)
        .ok_or_else(|| "predicate seam present".to_string())?;
    let evidence = evidence_for_seam(predicate, &index);
    let first = evidence
        .related_tests
        .first()
        .ok_or_else(|| "at least one related test".to_string())?;

    assert_eq!(first.test_name, "equality_boundary");
    assert_eq!(first.relation_reason, RelationReason::DirectOwnerCall);
    assert_eq!(first.oracle_strength, OracleStrength::Strong);
    Ok(())
}

// -- import_path_affinity tightening (#310 review) ---------------
//
// The detector requires explicit `module::owner_name` qualified-
// path syntax or an inline `use ... owner_name` line — pure token
// co-occurrence (owner_name + module token both present in the
// body without path syntax) must NOT fire.

#[test]
fn given_import_path_affinity_without_direct_call_when_related_tests_are_ranked_then_confidence_is_medium()
-> Result<(), String> {
    // Test references `crate::pricing::discounted_total` as a
    // function value (no parens → not a CallFact, so
    // direct_owner_call cannot fire). The qualified path satisfies
    // the tightened import_path_affinity detector. The test name
    // does not contain "discounted_total" and the file is not
    // pricing-flavoured, so no other reason fires either.
    let prod_src = "pub fn discounted_total(amount: i32, threshold: i32) -> i32 \
                        { if amount >= threshold { amount - 10 } else { amount } }\n";
    let test = (
        "tests/integration_smoke.rs",
        "#[test] fn smoke() { let _f = crate::pricing::discounted_total; assert_eq!(1, 1); }\n",
    );
    let grip = first_grip_for("src/pricing.rs", prod_src, &[test])?;
    assert_eq!(grip.relation_reason, RelationReason::ImportPathAffinity);
    assert_eq!(grip.relation_confidence, RelationConfidence::Medium);
    Ok(())
}

#[test]
fn given_qualified_owner_path_only_in_comment_or_string_when_related_tests_are_ranked_then_import_path_affinity_does_not_fire()
-> Result<(), String> {
    // Per CodeRabbit on #310: `test_imports_owner` previously did
    // a raw `body.contains("::owner")` which matched substrings
    // inside `// ...` comments and `"..."` string literals. That
    // re-introduced the noise the detector was meant to avoid.
    // After the fix, neither shape should match.
    let prod_src = "pub fn discounted_total(amount: i32, threshold: i32) -> i32 \
                        { if amount >= threshold { amount - 10 } else { amount } }\n";
    // Comment carries the qualified path; code does not. Test name
    // and file are both neutral so no other reason fires.
    let comment_only = (
        "tests/integration_a.rs",
        "#[test] fn smoke_a() { \
                // see crate::pricing::discounted_total for background \n\
                assert_eq!(1, 1); \
            }\n",
    );
    // String literal carries the qualified path.
    let string_only = (
        "tests/integration_b.rs",
        "#[test] fn smoke_b() { \
                let _doc = \"crate::pricing::discounted_total\"; \
                let _ = _doc; assert_eq!(1, 1); \
            }\n",
    );
    for (path, src) in [comment_only, string_only] {
        let files: Vec<(PathBuf, &str)> = vec![
            (PathBuf::from("src/pricing.rs"), prod_src),
            (PathBuf::from(path), src),
        ];
        let index = index_from_files(&files)?;
        let seams = inventory_seams_from_index(&[PathBuf::from("src/pricing.rs")], &index);
        let predicate = seams
            .iter()
            .find(|s| s.kind() == SeamKind::PredicateBoundary)
            .ok_or_else(|| "predicate seam present".to_string())?;
        let evidence = evidence_for_seam(predicate, &index);
        for grip in &evidence.related_tests {
            assert_ne!(
                grip.relation_reason,
                RelationReason::ImportPathAffinity,
                "qualified path inside comment/string in {path} must not match \
                     ImportPathAffinity; got {grip:?}"
            );
        }
    }
    Ok(())
}

#[test]
fn given_owner_and_module_tokens_without_import_path_when_related_tests_are_ranked_then_import_path_affinity_does_not_fire()
-> Result<(), String> {
    // Body contains `pricing` and `discounted_total` as bare
    // identifiers but never as a `::path::owner_name` shape and
    // never on a `use ...` line. The pre-tightening detector
    // would have fired (owner token + parent dir token both
    // present); the tightened detector must not.
    //
    // The test name embeds "discounted_total" — that is OK because
    // it triggers `owner_named_test`, a *different* reason. The
    // contract under test is "ImportPathAffinity does not fire on
    // mere token co-occurrence".
    let prod_src = "pub fn discounted_total(amount: i32, threshold: i32) -> i32 \
                        { if amount >= threshold { amount - 10 } else { amount } }\n";
    let test = (
        "tests/billing.rs",
        "#[test] fn discounted_total_token_smoke() { \
                let pricing = \"pricing\"; let discounted_total = 5; \
                let _ = (pricing, discounted_total); assert_eq!(1, 1); \
            }\n",
    );
    let files: Vec<(PathBuf, &str)> = vec![
        (PathBuf::from("src/pricing.rs"), prod_src),
        (PathBuf::from(test.0), test.1),
    ];
    let index = index_from_files(&files)?;
    let seams = inventory_seams_from_index(&[PathBuf::from("src/pricing.rs")], &index);
    let predicate = seams
        .iter()
        .find(|s| s.kind() == SeamKind::PredicateBoundary)
        .ok_or_else(|| "predicate seam present".to_string())?;
    let evidence = evidence_for_seam(predicate, &index);
    for grip in &evidence.related_tests {
        assert_ne!(
            grip.relation_reason,
            RelationReason::ImportPathAffinity,
            "token co-occurrence (`pricing` + `discounted_total` in body without \
                 `::` path syntax) must not match ImportPathAffinity; got {grip:?}"
        );
    }
    Ok(())
}

#[test]
fn given_same_module_test_without_direct_call_when_related_tests_are_ranked_then_confidence_is_medium()
-> Result<(), String> {
    // Owner sits in `src/pricing/discount.rs`; test sits in
    // `tests/pricing/integration.rs`. Different file stem (no
    // same_test_file). Same parent module (`pricing`) so
    // `same_module` is the right reason. No direct call, no
    // owner-named test, no qualified path / use line.
    let prod_src = "pub fn apply_discount(amount: i32, threshold: i32) -> i32 \
                        { if amount >= threshold { amount - 10 } else { amount } }\n";
    let test = (
        "tests/pricing/integration.rs",
        "#[test] fn module_neighbour() { assert_eq!(1, 1); }\n",
    );
    let files: Vec<(PathBuf, &str)> = vec![
        (PathBuf::from("src/pricing/discount.rs"), prod_src),
        (PathBuf::from(test.0), test.1),
    ];
    let index = index_from_files(&files)?;
    let seams = inventory_seams_from_index(&[PathBuf::from("src/pricing/discount.rs")], &index);
    let predicate = seams
        .iter()
        .find(|s| s.kind() == SeamKind::PredicateBoundary)
        .ok_or_else(|| "predicate seam present".to_string())?;
    let evidence = evidence_for_seam(predicate, &index);
    let grip = evidence
        .related_tests
        .first()
        .ok_or_else(|| "expected at least one related test for same-module pairing".to_string())?;
    assert_eq!(grip.relation_reason, RelationReason::SameModule);
    assert_eq!(grip.relation_confidence, RelationConfidence::Medium);
    Ok(())
}

// -- helper coverage ---------------------------------------------
//
// Targeted unit tests for the small private helpers introduced by
// analysis/related-test-precision-v1. The integration BDD tests
// above exercise the most common paths through `find_related_tests`,
// but each helper has a few branches that are not naturally hit by
// a single BDD scenario. The tests below pin those branches so
// codecov coverage reflects intent rather than scenario count.

#[test]
fn relation_reason_as_str_priority_and_confidence_are_pinned_per_variant() {
    // Pin the (variant -> "string", priority, confidence) mapping
    // for every reason. Catches accidental swaps in the match arms
    // of `as_str` / `priority` / `confidence`.
    let table = [
        (
            RelationReason::DirectOwnerCall,
            "direct_owner_call",
            0u8,
            RelationConfidence::High,
        ),
        (
            RelationReason::HelperOwnerCall,
            "helper_owner_call",
            1,
            RelationConfidence::High,
        ),
        (
            RelationReason::AssertionTargetAffinity,
            "assertion_target_affinity",
            2,
            RelationConfidence::Medium,
        ),
        (
            RelationReason::SameTestFile,
            "same_test_file",
            3,
            RelationConfidence::Medium,
        ),
        (
            RelationReason::SameModule,
            "same_module",
            4,
            RelationConfidence::Medium,
        ),
        (
            RelationReason::OwnerNamedTest,
            "owner_named_test",
            5,
            RelationConfidence::Medium,
        ),
        (
            RelationReason::ImportPathAffinity,
            "import_path_affinity",
            6,
            RelationConfidence::Medium,
        ),
        (
            RelationReason::FixtureOwnerAffinity,
            "fixture_owner_affinity",
            7,
            RelationConfidence::Low,
        ),
    ];
    for (reason, name, prio, conf) in table {
        assert_eq!(reason.as_str(), name, "{reason:?}.as_str()");
        assert_eq!(reason.priority(), prio, "{reason:?}.priority()");
        assert_eq!(reason.confidence(), conf, "{reason:?}.confidence()");
    }
}

#[test]
fn relation_confidence_as_str_and_rank_are_pinned_per_variant() {
    let table = [
        (RelationConfidence::High, "high", 0u8),
        (RelationConfidence::Medium, "medium", 1),
        (RelationConfidence::Low, "low", 2),
        (RelationConfidence::Opaque, "opaque", 3),
    ];
    for (conf, name, rank) in table {
        assert_eq!(conf.as_str(), name, "{conf:?}.as_str()");
        assert_eq!(conf.rank(), rank, "{conf:?}.rank()");
    }
}

#[test]
fn required_discriminator_tokens_extracts_text_from_every_variant() {
    use crate::analysis::seams::{ExpectedSink, RepoSeam, RequiredDiscriminator};
    let make = |rd: RequiredDiscriminator| {
        RepoSeam::new(
            "src/x.rs",
            "x::owner",
            SeamKind::PredicateBoundary,
            0,
            1,
            "irrelevant",
            rd,
            ExpectedSink::ReturnValue,
        )
    };
    // Each arm carries a distinctive token so we can confirm the
    // right field was picked. Tokens longer than 2 chars survive
    // `is_interesting_token`.
    let cases: Vec<(RequiredDiscriminator, &str)> = vec![
        (
            RequiredDiscriminator::BoundaryValue {
                description: "boundary_token".to_string(),
            },
            "boundary_token",
        ),
        (
            RequiredDiscriminator::ReturnValue {
                description: "returnval_token".to_string(),
            },
            "returnval_token",
        ),
        (
            RequiredDiscriminator::ErrorVariant {
                variant: "errvar_token".to_string(),
            },
            "errvar_token",
        ),
        (
            RequiredDiscriminator::FieldValue {
                field: "fieldval_token".to_string(),
            },
            "fieldval_token",
        ),
        (
            RequiredDiscriminator::Effect {
                sink: "effect_token".to_string(),
            },
            "effect_token",
        ),
        (
            RequiredDiscriminator::MatchArmTaken {
                arm: "matcharm_token".to_string(),
            },
            "matcharm_token",
        ),
        (
            RequiredDiscriminator::CallSite {
                target: "callsite_token".to_string(),
            },
            "callsite_token",
        ),
    ];
    for (rd, expected_token) in cases {
        let seam = make(rd.clone());
        let tokens = required_discriminator_tokens(&seam);
        assert!(
            tokens.iter().any(|t| t == expected_token),
            "{rd:?} -> tokens {tokens:?} must contain {expected_token}"
        );
    }
}

#[test]
fn same_test_file_accepts_stem_match_and_test_suffixes() {
    assert!(same_test_file(Path::new("tests/foo.rs"), "foo"));
    assert!(same_test_file(Path::new("tests/foo_test.rs"), "foo"));
    assert!(same_test_file(Path::new("tests/foo_tests.rs"), "foo"));
    assert!(!same_test_file(Path::new("tests/bar.rs"), "foo"));
    assert!(!same_test_file(Path::new(""), "foo"));
}

#[test]
fn module_path_for_handles_every_root_shape() {
    let cases: Vec<(&str, Option<&str>)> = vec![
        ("src/foo.rs", Some("foo")),
        ("tests/cli_smoke.rs", Some("cli_smoke")),
        ("crates/ripr/src/auth/login.rs", Some("auth/login")),
        ("crates/ripr/tests/integration.rs", Some("integration")),
        ("docs/note.rs", None),
        // `body = ".rs"` after stripping `src/`; trimmed = "" → None.
        ("src/.rs", None),
    ];
    for (input, expected) in cases {
        let got = module_path_for(Path::new(input));
        let want = expected.map(str::to_string);
        assert_eq!(got, want, "module_path_for({input})");
    }
}

#[test]
fn same_module_matches_parent_prefix_and_underscore_form() {
    assert!(same_module("pricing/discount", "pricing/integration"));
    assert!(same_module("a/b/c", "a_b/d"));
    assert!(!same_module("flat", "anything"));
    assert!(!same_module("pricing/discount", "billing/integration"));
}

#[test]
fn is_fixture_named_recognises_each_prefix_and_suffix() {
    let positives = [
        "fixture_quote",
        "setup_db",
        "make_quote",
        "build_request",
        "new_user",
        "mock_clock",
        "quote_fixture",
        "quote_factory",
    ];
    for name in positives {
        assert!(is_fixture_named(name), "{name} should be fixture-named");
    }
    for name in ["compute_total", "discount", "verify"] {
        assert!(
            !is_fixture_named(name),
            "{name} should NOT be fixture-named"
        );
    }
}

#[test]
fn given_assertion_target_token_in_test_assertion_when_related_tests_are_ranked_then_assertion_target_affinity_fires()
-> Result<(), String> {
    // Positive case for `assertion_target_affinity`: the seam's
    // `RequiredDiscriminator::BoundaryValue.description` carries
    // the identifier `discount_threshold`; a test assertion that
    // mentions `discount_threshold` as a whole identifier matches.
    // The test does not call the owner directly, the test file
    // stem is unrelated, and the test name does not embed the
    // owner — so this is the only reason that fires.
    let prod_src = "pub fn discounted_total(amount: i32, discount_threshold: i32) -> i32 \
                        { if amount >= discount_threshold { amount - 10 } else { amount } }\n";
    let test = (
        "tests/billing.rs",
        "fn other() -> i32 { 0 }\n\
             #[test] fn smoke() { let discount_threshold = 5; assert_eq!(discount_threshold, 5); }\n",
    );
    let grip = first_grip_for("src/pricing.rs", prod_src, &[test])?;
    assert_eq!(
        grip.relation_reason,
        RelationReason::AssertionTargetAffinity
    );
    assert_eq!(grip.relation_confidence, RelationConfidence::Medium);
    Ok(())
}

#[test]
fn given_generic_option_match_arm_binding_when_related_tests_are_ranked_then_assertion_target_affinity_does_not_fire()
-> Result<(), String> {
    let prod_src = r#"
pub fn outcome_command(out_path: Option<&str>) -> &'static str {
    match out_path {
        Some(path) => path,
        None => "missing",
    }
}
"#;
    let test = (
        "tests/status_contract.rs",
        r#"
#[test]
fn path_word_is_not_owner_evidence() {
    let path = "target/ripr/outcome.json";
    assert_eq!(path, "target/ripr/outcome.json");
}
"#,
    );
    let files: Vec<(PathBuf, &str)> = vec![
        (PathBuf::from("src/commands.rs"), prod_src),
        (PathBuf::from(test.0), test.1),
    ];
    let index = index_from_files(&files)?;
    let seams = inventory_seams_from_index(&[PathBuf::from("src/commands.rs")], &index);
    let some_arm = seams
        .iter()
        .find(|s| s.kind() == SeamKind::MatchArm && s.expression().contains("Some(path)"))
        .ok_or_else(|| "expected Some(path) match-arm seam".to_string())?;
    let evidence = evidence_for_seam(some_arm, &index);

    assert!(
        evidence
            .related_tests
            .iter()
            .all(|test| { test.relation_reason != RelationReason::AssertionTargetAffinity }),
        "generic match-arm binding token should not create assertion-target affinity: {:?}",
        evidence.related_tests
    );
    Ok(())
}

#[test]
fn given_specific_match_arm_variant_when_related_tests_are_ranked_then_assertion_target_affinity_still_fires()
-> Result<(), String> {
    let prod_src = r#"
pub enum Mode {
    Json,
    Text,
}

pub fn render_mode(mode: Mode) -> &'static str {
    match mode {
        Mode::Json => "json",
        Mode::Text => "text",
    }
}
"#;
    let test = (
        "tests/render_contract.rs",
        r#"
#[test]
fn mentions_json_variant() {
    let rendered = "Json";
    assert_eq!(rendered, "Json");
}
"#,
    );
    let files: Vec<(PathBuf, &str)> = vec![
        (PathBuf::from("src/render.rs"), prod_src),
        (PathBuf::from(test.0), test.1),
    ];
    let index = index_from_files(&files)?;
    let seams = inventory_seams_from_index(&[PathBuf::from("src/render.rs")], &index);
    let json_arm = seams
        .iter()
        .find(|s| s.kind() == SeamKind::MatchArm && s.expression().contains("Mode::Json"))
        .ok_or_else(|| "expected Mode::Json match-arm seam".to_string())?;
    let evidence = evidence_for_seam(json_arm, &index);

    assert!(
        evidence
            .related_tests
            .iter()
            .any(|test| test.relation_reason == RelationReason::AssertionTargetAffinity),
        "specific enum variant should still support assertion-target affinity: {:?}",
        evidence.related_tests
    );
    Ok(())
}

#[test]
fn given_match_arm_expected_sink_token_when_related_tests_are_ranked_then_assertion_target_affinity_does_not_fire()
-> Result<(), String> {
    let prod_src = r#"
pub fn parse_c_escape(ch: char) -> char {
    match ch {
        '"' => '"',
        '\\' => '\\',
        _ => ch,
    }
}
"#;
    let test = (
        "tests/path_contract.rs",
        r#"
#[test]
fn return_value_word_is_not_match_arm_evidence() {
    let return_value = '"';
    assert_eq!(return_value, '"');
}
"#,
    );
    let files: Vec<(PathBuf, &str)> = vec![
        (PathBuf::from("src/diff_path.rs"), prod_src),
        (PathBuf::from(test.0), test.1),
    ];
    let index = index_from_files(&files)?;
    let seams = inventory_seams_from_index(&[PathBuf::from("src/diff_path.rs")], &index);
    let quote_arm = seams
        .iter()
        .find(|s| s.kind() == SeamKind::MatchArm && s.expression().contains("'\"'"))
        .ok_or_else(|| "expected quote match-arm seam".to_string())?;
    let evidence = evidence_for_seam(quote_arm, &index);

    assert!(
        evidence
            .related_tests
            .iter()
            .all(|test| { test.relation_reason != RelationReason::AssertionTargetAffinity }),
        "generic expected-sink token should not create match-arm assertion-target affinity: {:?}",
        evidence.related_tests
    );
    Ok(())
}

#[test]
fn assertion_targets_seam_returns_false_for_empty_token_list() {
    // The `tokens.is_empty()` early-return is the cheap escape
    // hatch when a seam's `RequiredDiscriminator` carries no
    // interesting tokens (e.g. a one-character variable name).
    use crate::analysis::rust_index::TestFact;
    let test = TestFact {
        name: "synth".to_string(),
        file: PathBuf::from("tests/x.rs"),
        start_line: 1,
        end_line: 5,
        body: "assert_eq!(1, 1);".to_string(),
        calls: Vec::new(),
        assertions: Vec::new(),
        literals: Vec::new(),
        attrs: Vec::new(),
    };
    assert!(!assertion_targets_seam(&test, &[]));
}

#[test]
fn package_prefix_resolves_crates_and_nested_src_tests_layouts() {
    // `crates/<name>/src/...` form returns the `crates/<name>/` prefix.
    assert_eq!(
        package_prefix(Path::new("crates/ripr/src/auth/login.rs")).as_deref(),
        Some("crates/ripr/")
    );
    // `crates/<name>/tests/...` form (the second branch of the
    // strip_prefix-and-or guard) also returns the package prefix.
    assert_eq!(
        package_prefix(Path::new("crates/ripr/tests/integration.rs")).as_deref(),
        Some("crates/ripr/")
    );
    // Nested workspace path (rfind branch): the marker scan falls
    // through to the `/src/` rfind path.
    assert_eq!(
        package_prefix(Path::new("workspaces/foo/src/auth/login.rs")).as_deref(),
        Some("workspaces/foo/")
    );
    // Bare `src/...` returns None (prefix would be empty).
    assert_eq!(package_prefix(Path::new("src/foo.rs")), None);
    // Path under neither root.
    assert_eq!(package_prefix(Path::new("docs/note.rs")), None);
}

#[test]
fn given_owner_in_workspace_crate_when_test_is_in_other_crate_then_it_is_filtered_out()
-> Result<(), String> {
    // Owner lives in `crates/ripr_pricing/src/discount.rs`; a test
    // in a different package (`crates/ripr_other/tests/x.rs`)
    // must not appear as a related test, even if it would
    // otherwise satisfy a reason. Exercises the package-prefix
    // skip branch in `find_related_tests`.
    let prod_src = "pub fn discounted_total(amount: i32, threshold: i32) -> i32 \
                        { if amount >= threshold { amount - 10 } else { amount } }\n";
    let other_pkg_test = (
        "crates/ripr_other/tests/x.rs",
        "#[test] fn discounted_total_other_pkg() { assert_eq!(1, 1); }\n",
    );
    let files: Vec<(PathBuf, &str)> = vec![
        (
            PathBuf::from("crates/ripr_pricing/src/discount.rs"),
            prod_src,
        ),
        (PathBuf::from(other_pkg_test.0), other_pkg_test.1),
    ];
    let index = index_from_files(&files)?;
    let seams = inventory_seams_from_index(
        &[PathBuf::from("crates/ripr_pricing/src/discount.rs")],
        &index,
    );
    let predicate = seams
        .iter()
        .find(|s| s.kind() == SeamKind::PredicateBoundary)
        .ok_or_else(|| "predicate seam present".to_string())?;
    let evidence = evidence_for_seam(predicate, &index);
    for grip in &evidence.related_tests {
        assert_ne!(
            grip.file,
            Path::new("crates/ripr_other/tests/x.rs"),
            "test in unrelated package should be filtered by package_prefix; \
                 got {grip:?}"
        );
    }
    Ok(())
}

#[test]
fn given_test_calls_helper_with_fixture_attribute_then_fixture_owner_affinity_fires()
-> Result<(), String> {
    // `test_uses_owner_fixture` accepts EITHER a fixture-named
    // helper OR a helper whose body contains `#[fixture]`. The
    // earlier `given_fixture_only_affinity_…` test exercises the
    // name-based branch (`make_quote`); this one exercises the
    // body-marker branch by using a non-fixture helper name but
    // placing the `#[fixture]` marker as an inline comment inside
    // the body. `FunctionFact.body` slices from the `fn` keyword
    // to the end of the function, so attributes ABOVE the `fn`
    // line are not captured — the marker must live inside the
    // body block.
    let prod_src = "pub fn discounted_total(amount: i32, threshold: i32) -> i32 \
                        { if amount >= threshold { amount - 10 } else { amount } }\n\
                        pub fn provide_quote() -> i32 {\n    // #[fixture]\n    100\n}\n";
    let test = (
        "tests/integration.rs",
        "#[test] fn quote_smoke() { let _ = provide_quote(); assert!(true); }\n",
    );
    let grip = first_grip_for("src/pricing.rs", prod_src, &[test])?;
    assert_eq!(grip.relation_reason, RelationReason::FixtureOwnerAffinity);
    Ok(())
}

// -- value-extraction-v2 ------------------------------------------
//
// Each test exercises one resolution path through `activate_evidence`:
// a related test calls the seam owner, the call arg is something
// `scalar_values` would reject (bare identifier, builder method,
// table row, rstest case, Some/Err wrapper), and the resolver in
// `analysis::value_resolution` should turn it into observed values
// - which `evidence_for_seam` then exposes via
// `TestGripEvidence.observed_values`. The negative tests pin the
// false-positive guards for comment/string shadows and unrelated
// identifiers.

fn observed_values_for(prod_src: &str, tests: &[(&str, &str)]) -> Result<Vec<String>, String> {
    let mut files: Vec<(PathBuf, &str)> = vec![(PathBuf::from("src/pricing.rs"), prod_src)];
    for (path, src) in tests {
        files.push((PathBuf::from(*path), *src));
    }
    let index = index_from_files(&files)?;
    let seams = inventory_seams_from_index(&[PathBuf::from("src/pricing.rs")], &index);
    let predicate = seams
        .iter()
        .find(|s| s.kind() == SeamKind::PredicateBoundary)
        .ok_or_else(|| "predicate seam present".to_string())?;
    let evidence = evidence_for_seam(predicate, &index);
    Ok(evidence
        .observed_values
        .into_iter()
        .map(|v| v.value)
        .collect())
}

#[test]
fn given_boundary_owner_call_when_argument_is_path_constructor_then_observed_values_are_resolved()
-> Result<(), String> {
    let source = PathBuf::from("src/agent/loop_commands.rs");
    let source_src = r#"
use std::path::Path;

pub fn workflow_artifact_path(out_dir: &Path, file_name: &str) -> String {
    let out_dir = display_path(out_dir);
    if out_dir == "." {
        file_name.to_string()
    } else {
        format!("{}/{}", out_dir.trim_end_matches('/'), file_name)
    }
}

pub fn display_path(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn workflow_artifact_path_uses_output_directory() {
        let path = workflow_artifact_path(Path::new("target/ripr/workflow"), "workflow.json");
        assert_eq!(path, "target/ripr/workflow/workflow.json");
    }
}
"#;
    let index = index_from_files(&[(source, source_src)])?;
    let seams = inventory_seams_from_index(&[PathBuf::from("src/agent/loop_commands.rs")], &index);
    let predicate = seams
        .iter()
        .find(|s| s.kind() == SeamKind::PredicateBoundary && s.expression().contains("=="))
        .ok_or_else(|| "out_dir predicate seam present".to_string())?;
    let evidence = evidence_for_seam(predicate, &index);
    let values: Vec<String> = evidence
        .observed_values
        .iter()
        .map(|value| value.value.clone())
        .collect();

    assert!(
        values
            .iter()
            .any(|value| value == "\"target/ripr/workflow\""),
        "Path::new literal should become a concrete observed activation value; got {values:?}"
    );
    assert_eq!(evidence.activate.state, StageState::Yes);
    Ok(())
}

#[test]
fn given_let_binding_values_when_owner_call_uses_identifiers_then_observed_values_are_resolved()
-> Result<(), String> {
    let prod_src = "pub fn discounted_total(amount: i32, threshold: i32) -> i32 \
                        { if amount >= threshold { amount - 10 } else { amount } }\n";
    let test = (
        "tests/pricing_tests.rs",
        "#[test] fn at_threshold() { let amount = 100; let threshold = 100; \
             assert_eq!(discounted_total(amount, threshold), 90); }\n",
    );
    let values = observed_values_for(prod_src, &[test])?;
    assert!(
        values.iter().any(|v| v == "100"),
        "let-resolved 100 must appear in observed values; got {values:?}"
    );
    Ok(())
}

#[test]
fn given_shared_borrowed_let_binding_when_owner_call_uses_reference_then_observed_value_is_resolved()
-> Result<(), String> {
    let prod_src = "pub fn amount_matches(amount: &i32) -> bool { amount == &100 }\n";
    let test = (
        "tests/pricing_tests.rs",
        "#[test] fn borrowed_amount_reference() { \
                 let amount = 100; \
                 let actual = amount_matches(&amount); \
                 assert!(actual); \
             }\n",
    );
    let files: Vec<(PathBuf, &str)> = vec![
        (PathBuf::from("src/pricing.rs"), prod_src),
        (PathBuf::from(test.0), test.1),
    ];
    let index = index_from_files(&files)?;
    let seams = inventory_seams_from_index(&[PathBuf::from("src/pricing.rs")], &index);
    let predicate = seams
        .iter()
        .find(|seam| seam.kind() == SeamKind::PredicateBoundary)
        .ok_or_else(|| "predicate seam present".to_string())?;
    let evidence = evidence_for_seam(predicate, &index);
    let values: Vec<String> = evidence
        .observed_values
        .iter()
        .map(|value| value.value.clone())
        .collect();
    assert!(
        values.iter().any(|value| value == "100"),
        "borrowed let binding literal must appear in observed values; got {values:?}; activation: {}; related: {:?}",
        evidence.activate.summary,
        evidence.related_tests
    );
    Ok(())
}

#[test]
fn given_boundary_owner_call_when_threshold_is_parameter_then_observed_values_stay_on_input_operand()
-> Result<(), String> {
    let prod_src = "pub fn discounted_total(amount: i32, discount_threshold: i32) -> i32 \
                        { if amount >= discount_threshold { amount - 10 } else { amount } }\n";
    let test = (
        "tests/pricing_tests.rs",
        "#[test] fn below_threshold() { \
                 assert_eq!(discounted_total(50, 100), 50); \
             }\n",
    );
    let values = observed_values_for(prod_src, &[test])?;
    assert_eq!(
        values,
        vec!["50".to_string()],
        "observed values should describe the tested input operand, not the boundary parameter"
    );
    Ok(())
}

#[test]
fn given_boundary_owner_call_when_input_operand_is_direct_parameter_alias_then_observed_values_are_resolved()
-> Result<(), String> {
    let prod_src = r#"
pub fn discounted_total(raw_amount: i32, threshold: i32) -> i32 {
    let amount = raw_amount;
    if amount >= threshold { amount - 10 } else { amount }
}
"#;
    let test = (
        "tests/pricing_tests.rs",
        "#[test] fn below_threshold() { \
                 assert_eq!(discounted_total(50, 100), 50); \
             }\n",
    );
    let values = observed_values_for(prod_src, &[test])?;
    assert_eq!(
        values,
        vec!["50".to_string()],
        "direct local aliases of owner parameters should resolve to the original owner-call argument"
    );
    Ok(())
}

#[test]
fn given_boundary_owner_call_when_if_let_alias_parameter_name_has_prefix_then_exact_parameter_is_used()
-> Result<(), String> {
    let prod_src = r#"
pub fn discounted_total(raw_amount: Option<i32>, raw_amount_extra: Option<i32>, threshold: i32) -> i32 {
    if let Some(amount) = raw_amount_extra {
        if amount >= threshold { amount - 10 } else { amount }
    } else {
        0
    }
}
"#;
    let test = (
        "tests/pricing_tests.rs",
        "#[test] fn below_threshold() { \
                 assert_eq!(discounted_total(Some(50), Some(60), 50), 60); \
             }\n",
    );
    let mut files: Vec<(PathBuf, &str)> = vec![(PathBuf::from("src/pricing.rs"), prod_src)];
    files.push((PathBuf::from(test.0), test.1));
    let index = index_from_files(&files)?;
    let seams = inventory_seams_from_index(&[PathBuf::from("src/pricing.rs")], &index);
    let predicate = seams
        .iter()
        .find(|s| {
            s.kind() == SeamKind::PredicateBoundary
                && s.expression().contains("amount >= threshold")
        })
        .ok_or_else(|| "amount predicate seam present".to_string())?;
    let evidence = evidence_for_seam(predicate, &index);
    let values: Vec<String> = evidence
        .observed_values
        .iter()
        .map(|v| v.value.clone())
        .collect();
    assert_eq!(
        values,
        vec!["60".to_string()],
        "prefix parameter matches must resolve amount from raw_amount_extra, not raw_amount"
    );
    Ok(())
}

#[test]
fn given_boundary_owner_call_when_match_alias_is_comment_then_operand_stays_unresolved()
-> Result<(), String> {
    let prod_src = r#"
pub fn discounted_total(raw_amount: Option<i32>, threshold: i32) -> i32 {
    // match raw_amount { Some(amount) => if amount >= threshold { amount - 10 } else { amount }, _ => 0 }
    let amount = 1;
    if amount >= threshold { amount - 10 } else { amount }
}
"#;
    let test = (
        "tests/pricing_tests.rs",
        "#[test] fn at_threshold() { \
                 assert_eq!(discounted_total(Some(50), 50), -9); \
             }\n",
    );
    let mut files: Vec<(PathBuf, &str)> = vec![(PathBuf::from("src/pricing.rs"), prod_src)];
    files.push((PathBuf::from(test.0), test.1));
    let index = index_from_files(&files)?;
    let seams = inventory_seams_from_index(&[PathBuf::from("src/pricing.rs")], &index);
    let predicate = seams
        .iter()
        .find(|s| {
            s.kind() == SeamKind::PredicateBoundary
                && s.expression().contains("amount >= threshold")
        })
        .ok_or_else(|| "amount predicate seam present".to_string())?;
    let evidence = evidence_for_seam(predicate, &index);
    assert!(
        evidence.observed_values.is_empty(),
        "commented match aliases must not resolve boundary operands; got {:?}",
        evidence.observed_values
    );
    assert!(
        evidence.missing_discriminators.is_empty(),
        "unresolved commented match alias should stay a limitation, not an exact repair candidate; got {:?}",
        evidence.missing_discriminators
    );
    Ok(())
}

#[test]
fn given_boundary_owner_call_when_inline_match_alias_is_comment_then_operand_stays_unresolved()
-> Result<(), String> {
    let prod_src = r#"
pub fn discounted_total(raw_amount: Option<i32>, threshold: i32) -> i32 {
    let _note = 0; // match raw_amount { Some(amount) => if amount >= threshold { amount - 10 } else { amount }, _ => 0 }
    let amount = 1;
    if amount >= threshold { amount - 10 } else { amount }
}
"#;
    let test = (
        "tests/pricing_tests.rs",
        "#[test] fn at_threshold() { \
                 assert_eq!(discounted_total(Some(50), 50), -9); \
             }\n",
    );
    let mut files: Vec<(PathBuf, &str)> = vec![(PathBuf::from("src/pricing.rs"), prod_src)];
    files.push((PathBuf::from(test.0), test.1));
    let index = index_from_files(&files)?;
    let seams = inventory_seams_from_index(&[PathBuf::from("src/pricing.rs")], &index);
    let predicate = seams
        .iter()
        .find(|s| {
            s.kind() == SeamKind::PredicateBoundary
                && s.expression().contains("amount >= threshold")
        })
        .ok_or_else(|| "amount predicate seam present".to_string())?;
    let evidence = evidence_for_seam(predicate, &index);
    assert!(
        evidence.observed_values.is_empty(),
        "inline commented match aliases must not resolve boundary operands; got {:?}",
        evidence.observed_values
    );
    assert!(
        evidence.missing_discriminators.is_empty(),
        "unresolved inline commented match alias should stay a limitation, not an exact repair candidate; got {:?}",
        evidence.missing_discriminators
    );
    Ok(())
}

#[test]
fn given_boundary_owner_call_when_match_wrapper_is_comment_then_operand_stays_unresolved()
-> Result<(), String> {
    let prod_src = r#"
pub fn discounted_total(raw_amount: Option<i32>, threshold: i32) -> i32 {
    let _seen = match raw_amount { _ => false };
    // Some(amount)
    let amount = 1;
    if amount >= threshold { amount - 10 } else { amount }
}
"#;
    let test = (
        "tests/pricing_tests.rs",
        "#[test] fn at_threshold() { \
                 assert_eq!(discounted_total(Some(50), 50), -9); \
             }\n",
    );
    let mut files: Vec<(PathBuf, &str)> = vec![(PathBuf::from("src/pricing.rs"), prod_src)];
    files.push((PathBuf::from(test.0), test.1));
    let index = index_from_files(&files)?;
    let seams = inventory_seams_from_index(&[PathBuf::from("src/pricing.rs")], &index);
    let predicate = seams
        .iter()
        .find(|s| {
            s.kind() == SeamKind::PredicateBoundary
                && s.expression().contains("amount >= threshold")
        })
        .ok_or_else(|| "amount predicate seam present".to_string())?;
    let evidence = evidence_for_seam(predicate, &index);
    assert!(
        evidence.observed_values.is_empty(),
        "commented wrapper patterns must not resolve boundary operands; got {:?}",
        evidence.observed_values
    );
    assert!(
        evidence.missing_discriminators.is_empty(),
        "unresolved commented wrapper pattern should stay a limitation, not an exact repair candidate; got {:?}",
        evidence.missing_discriminators
    );
    Ok(())
}

#[test]
fn given_boundary_owner_call_when_inline_match_wrapper_is_comment_then_operand_stays_unresolved()
-> Result<(), String> {
    let prod_src = r#"
pub fn discounted_total(raw_amount: Option<i32>, threshold: i32) -> i32 {
    let _seen = match raw_amount { _ => false }; // Some(amount)
    let amount = 1;
    if amount >= threshold { amount - 10 } else { amount }
}
"#;
    let test = (
        "tests/pricing_tests.rs",
        "#[test] fn at_threshold() { \
                 assert_eq!(discounted_total(Some(50), 50), -9); \
             }\n",
    );
    let mut files: Vec<(PathBuf, &str)> = vec![(PathBuf::from("src/pricing.rs"), prod_src)];
    files.push((PathBuf::from(test.0), test.1));
    let index = index_from_files(&files)?;
    let seams = inventory_seams_from_index(&[PathBuf::from("src/pricing.rs")], &index);
    let predicate = seams
        .iter()
        .find(|s| {
            s.kind() == SeamKind::PredicateBoundary
                && s.expression().contains("amount >= threshold")
        })
        .ok_or_else(|| "amount predicate seam present".to_string())?;
    let evidence = evidence_for_seam(predicate, &index);
    assert!(
        evidence.observed_values.is_empty(),
        "inline commented wrapper patterns must not resolve boundary operands; got {:?}",
        evidence.observed_values
    );
    assert!(
        evidence.missing_discriminators.is_empty(),
        "unresolved inline commented wrapper should stay a limitation, not an exact repair candidate; got {:?}",
        evidence.missing_discriminators
    );
    Ok(())
}

#[test]
fn given_boundary_owner_call_when_input_operand_is_iterator_local_then_activation_is_static_limitation()
-> Result<(), String> {
    let prod_src = "pub fn sum_from_offset(values: &[i32], offset: usize) -> i32 { \
                            let mut total = 0; \
                            for (idx, value) in values.iter().enumerate() { \
                                if idx >= offset { total += *value; } \
                            } \
                            total \
                        }\n";
    let test = (
        "tests/pricing_tests.rs",
        "#[test] fn sums_after_offset() { \
                 assert_eq!(sum_from_offset(&[1, 2, 3], 1), 5); \
             }\n",
    );
    let files: Vec<(PathBuf, &str)> = vec![
        (PathBuf::from("src/pricing.rs"), prod_src),
        (PathBuf::from(test.0), test.1),
    ];
    let index = index_from_files(&files)?;
    let seams = inventory_seams_from_index(&[PathBuf::from("src/pricing.rs")], &index);
    let predicate = seams
        .iter()
        .find(|s| s.kind() == SeamKind::PredicateBoundary)
        .ok_or_else(|| "predicate seam present".to_string())?;
    let evidence = evidence_for_seam(predicate, &index);

    assert_eq!(evidence.activate.state, StageState::Unknown);
    assert!(
        evidence.activate.summary.contains("iterator-derived"),
        "unresolved iterator-local boundary must explain why it is limited; got {}",
        evidence.activate.summary
    );
    assert!(
        evidence.observed_values.is_empty(),
        "iterator-local activation values must not be invented from owner-call args; got {:?}",
        evidence.observed_values
    );
    assert!(
        evidence.missing_discriminators.is_empty(),
        "unresolved iterator-local boundary must not emit exact candidate discriminator; got {:?}",
        evidence.missing_discriminators
    );
    Ok(())
}

#[test]
fn given_boundary_owner_call_when_input_operand_is_computed_local_then_activation_stays_static_limitation()
-> Result<(), String> {
    let prod_src = "pub fn discounted_total(raw_amount: i32, threshold: i32) -> i32 { \
                            let amount = raw_amount + 1; \
                            if amount >= threshold { amount - 10 } else { amount } \
                        }\n";
    let test = (
        "tests/pricing_tests.rs",
        "#[test] fn below_threshold() { \
                 assert_eq!(discounted_total(50, 100), 51); \
             }\n",
    );
    let files: Vec<(PathBuf, &str)> = vec![
        (PathBuf::from("src/pricing.rs"), prod_src),
        (PathBuf::from(test.0), test.1),
    ];
    let index = index_from_files(&files)?;
    let seams = inventory_seams_from_index(&[PathBuf::from("src/pricing.rs")], &index);
    let predicate = seams
        .iter()
        .find(|s| s.kind() == SeamKind::PredicateBoundary)
        .ok_or_else(|| "predicate seam present".to_string())?;
    let evidence = evidence_for_seam(predicate, &index);

    assert_eq!(evidence.activate.state, StageState::Unknown);
    assert!(
        evidence.activate.summary.contains("local or computed"),
        "computed local boundary operands must remain a named limitation; got {}",
        evidence.activate.summary
    );
    assert!(
        !evidence.activate.summary.contains("iterator-derived"),
        "computed local boundary operands must not be routed as iterator-derived; got {}",
        evidence.activate.summary
    );
    assert!(
        evidence.observed_values.is_empty(),
        "computed local activation values must not be invented from owner-call args; got {:?}",
        evidence.observed_values
    );
    assert!(
        evidence.missing_discriminators.is_empty(),
        "computed local boundary operands must not emit exact candidate discriminator; got {:?}",
        evidence.missing_discriminators
    );
    Ok(())
}

#[test]
fn given_non_comparison_predicate_when_direct_owner_call_exists_then_activation_is_value_insensitive()
-> Result<(), String> {
    let prod_src = "pub fn has_missing(missing: &[String]) -> bool { \
                            if missing.is_empty() { true } else { false } \
                        }\n";
    let test = (
        "tests/missing_tests.rs",
        "#[test] fn empty_missing_is_true() { \
                 assert!(has_missing(&[])); \
             }\n",
    );
    let files: Vec<(PathBuf, &str)> = vec![
        (PathBuf::from("src/missing.rs"), prod_src),
        (PathBuf::from(test.0), test.1),
    ];
    let index = index_from_files(&files)?;
    let seams = inventory_seams_from_index(&[PathBuf::from("src/missing.rs")], &index);
    let predicate = seams
        .iter()
        .find(|s| {
            s.kind() == SeamKind::PredicateBoundary && s.expression().contains("missing.is_empty()")
        })
        .ok_or_else(|| "non-comparison predicate seam present".to_string())?;
    let evidence = evidence_for_seam(predicate, &index);

    assert_eq!(evidence.activate.state, StageState::Yes);
    assert!(
        evidence
            .activate
            .summary
            .contains("direct owner call for value-insensitive seam"),
        "non-comparison predicates should not require scalar activation values; got {}",
        evidence.activate.summary
    );
    assert!(
        evidence.observed_values.is_empty(),
        "non-comparison predicate activation must not invent collection literal values; got {:?}",
        evidence.observed_values
    );
    assert!(
        evidence.missing_discriminators.is_empty(),
        "non-comparison predicates must not emit exact boundary candidates; got {:?}",
        evidence.missing_discriminators
    );
    Ok(())
}

#[test]
fn given_boundary_owner_call_when_input_operand_is_closure_local_then_activation_is_static_limitation()
-> Result<(), String> {
    let prod_src = "pub struct FunctionSummary { pub id: &'static str } \
                        pub fn has_owner(functions: &[FunctionSummary], owner: &str) -> bool { \
                            functions.iter().any(|function| function.id == owner) \
                        }\n";
    let test = (
        "tests/owner_tests.rs",
        "#[test] fn finds_owner() { \
                 let functions = [FunctionSummary { id: \"score\" }]; \
                 assert!(has_owner(&functions, \"score\")); \
             }\n",
    );
    let files: Vec<(PathBuf, &str)> = vec![
        (PathBuf::from("src/owner.rs"), prod_src),
        (PathBuf::from(test.0), test.1),
    ];
    let index = index_from_files(&files)?;
    let seams = inventory_seams_from_index(&[PathBuf::from("src/owner.rs")], &index);
    let predicate = seams
        .iter()
        .find(|s| {
            s.kind() == SeamKind::PredicateBoundary
                && s.expression().contains("function.id == owner")
        })
        .ok_or_else(|| "closure predicate seam present".to_string())?;
    let evidence = evidence_for_seam(predicate, &index);

    assert_eq!(evidence.activate.state, StageState::Unknown);
    assert!(
        evidence.activate.summary.contains("closure-derived"),
        "closure boundary operands must be routed precisely; got {}",
        evidence.activate.summary
    );
    assert!(
        !evidence.activate.summary.contains("iterator-derived"),
        "closure boundary operands must not be routed as iterator-derived; got {}",
        evidence.activate.summary
    );
    assert!(
        evidence.observed_values.is_empty(),
        "closure-local activation values must not be invented from owner-call args; got {:?}",
        evidence.observed_values
    );
    assert!(
        evidence.missing_discriminators.is_empty(),
        "closure-local boundary operands must not emit exact candidate discriminator; got {:?}",
        evidence.missing_discriminators
    );
    Ok(())
}

#[test]
fn closure_boundary_operand_route_ignores_comment_only_closure_pattern() {
    let owner = FunctionSummary {
            id: crate::domain::SymbolId("src/lib.rs::score".to_string()),
            name: "score".to_string(),
            file: PathBuf::from("src/lib.rs"),
            start_line: 1,
            end_line: 5,
            body: "pub fn score(raw_amount: i32, threshold: i32) -> bool {\n    // values.iter().any(|amount| amount >= threshold)\n    let amount = raw_amount + 1;\n    amount >= threshold\n}".to_string(),
            calls: Vec::new(),
            returns: Vec::new(),
            literals: Vec::new(),
            is_test: false,
            attrs: Vec::new(),
        };

    assert!(!boundary_operand_is_closure_derived(&owner, "amount"));
}

#[test]
fn given_boundary_owner_call_when_parameter_field_operands_resolve_then_activation_is_yes()
-> Result<(), String> {
    let prod_src = "pub struct BoundarySide { pub value: i32 } \
                        pub fn equal_boundary(left: BoundarySide, right: BoundarySide) -> bool { \
                            left.value == right.value \
                        }\n";
    let test = (
        "tests/boundary_tests.rs",
        "#[test] fn equal_field_boundary() { \
                 let left = BoundarySide { value: 10 }; \
                 let right = BoundarySide { value: 10 }; \
                 assert!(equal_boundary(left, right)); \
             }\n",
    );
    let files: Vec<(PathBuf, &str)> = vec![
        (PathBuf::from("src/boundary.rs"), prod_src),
        (PathBuf::from(test.0), test.1),
    ];
    let index = index_from_files(&files)?;
    let seams = inventory_seams_from_index(&[PathBuf::from("src/boundary.rs")], &index);
    let predicate = seams
        .iter()
        .find(|s| s.kind() == SeamKind::PredicateBoundary)
        .ok_or_else(|| "predicate seam present".to_string())?;
    let evidence = evidence_for_seam(predicate, &index);

    assert_eq!(evidence.activate.state, StageState::Yes);
    assert!(
        evidence
            .activate
            .summary
            .contains("Observed 1 concrete activation value(s)"),
        "parameter field operands must resolve through same-test struct field bindings; got {}",
        evidence.activate.summary
    );
    assert!(
        evidence
            .observed_values
            .iter()
            .any(|fact| fact.value == "10" && fact.context == ValueContext::FunctionArgument),
        "expected resolved struct-field activation value; got {:?}",
        evidence.observed_values
    );
    assert!(
        evidence.missing_discriminators.is_empty(),
        "field operands with equal observed values must not emit boundary repair debt; got {:?}",
        evidence.missing_discriminators
    );
    Ok(())
}

#[test]
fn given_boundary_owner_call_when_parameter_field_operands_are_opaque_then_activation_stays_static_limitation()
-> Result<(), String> {
    let prod_src = "pub struct BoundarySide { pub value: i32 } \
                        pub fn equal_boundary(left: BoundarySide, right: BoundarySide) -> bool { \
                            left.value == right.value \
                        }\n";
    let test = (
        "tests/boundary_tests.rs",
        "#[test] fn equal_field_boundary() { \
                 let left = make_side(10); \
                 let right = make_side(10); \
                 assert!(equal_boundary(left, right)); \
             }\n",
    );
    let files: Vec<(PathBuf, &str)> = vec![
        (PathBuf::from("src/boundary.rs"), prod_src),
        (PathBuf::from(test.0), test.1),
    ];
    let index = index_from_files(&files)?;
    let seams = inventory_seams_from_index(&[PathBuf::from("src/boundary.rs")], &index);
    let predicate = seams
        .iter()
        .find(|s| s.kind() == SeamKind::PredicateBoundary)
        .ok_or_else(|| "predicate seam present".to_string())?;
    let evidence = evidence_for_seam(predicate, &index);

    assert_eq!(evidence.activate.state, StageState::Unknown);
    assert!(
        evidence.activate.summary.contains("local or computed"),
        "opaque parameter field operands must remain a named limitation; got {}",
        evidence.activate.summary
    );
    assert!(
        evidence.observed_values.is_empty(),
        "opaque parameter field operands must not invent activation values; got {:?}",
        evidence.observed_values
    );
    assert!(
        evidence.missing_discriminators.is_empty(),
        "opaque parameter field operands must not emit exact candidate discriminator; got {:?}",
        evidence.missing_discriminators
    );
    Ok(())
}

fn field_constant_evidence(test_module: &str) -> Result<TestGripEvidence, String> {
    let source = format!(
        r#"
const SUPPORTED_VERSION: u32 = 1;
const SUPPORTED_VERSION_COPY: u32 = 1;

pub struct File {{
    pub body_model_version: u32,
    pub other_version: u32,
}}

pub fn lower(file: &File) -> bool {{
    file.body_model_version == SUPPORTED_VERSION
}}

{test_module}
"#,
    );
    let path = PathBuf::from("src/lib.rs");
    let index = index_from_files(&[(path.clone(), source.as_str())])?;
    let seams = inventory_seams_from_index(&[path], &index);
    let predicate = seams
        .iter()
        .find(|seam| {
            seam.kind() == SeamKind::PredicateBoundary
                && seam
                    .expression()
                    .contains("body_model_version == SUPPORTED_VERSION")
        })
        .ok_or_else(|| "field-constant predicate seam present".to_string())?;
    Ok(evidence_for_seam(predicate, &index))
}

#[test]
fn given_direct_field_assignments_from_named_constant_boundaries_then_values_are_observed()
-> Result<(), String> {
    let evidence = field_constant_evidence(
        r#"
#[cfg(test)]
mod tests {
    use super::*;

    fn build_file() -> File {
        File { body_model_version: 99, other_version: 99 }
    }

    #[test]
    fn boundary_versions() {
        let mut equal = build_file();
        equal.body_model_version = SUPPORTED_VERSION;
        assert!(lower(&equal));

        let mut below = build_file();
        below.body_model_version = SUPPORTED_VERSION - 1;
        assert!(!lower(&below));

        let mut above = build_file();
        above.body_model_version = SUPPORTED_VERSION + 1;
        assert!(!lower(&above));
    }
}
"#,
    )?;
    let values: BTreeSet<&str> = evidence
        .observed_values
        .iter()
        .map(|fact| fact.value.as_str())
        .collect();
    for expected in ["SUPPORTED_VERSION", "0", "2"] {
        if !values.contains(expected) {
            return Err(format!(
                "expected direct field-assignment value {expected}; got {:?}",
                evidence.observed_values
            ));
        }
    }
    if !evidence.missing_discriminators.is_empty() {
        return Err(format!(
            "the exact named-constant boundary must close; got {:?}",
            evidence.missing_discriminators
        ));
    }
    Ok(())
}

#[test]
fn given_other_object_or_field_assignments_then_boundary_value_is_not_credited()
-> Result<(), String> {
    let evidence = field_constant_evidence(
        r#"
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wrong_targets() {
        let mut file = File { body_model_version: 0, other_version: 0 };
        let mut other = File { body_model_version: 0, other_version: 0 };
        other.body_model_version = SUPPORTED_VERSION;
        file.other_version = SUPPORTED_VERSION;
        assert!(!lower(&file));
    }
}
"#,
    )?;
    if evidence
        .observed_values
        .iter()
        .any(|fact| fact.value == "SUPPORTED_VERSION")
    {
        return Err(format!(
            "other objects or fields must not satisfy this seam: {:?}",
            evidence.observed_values
        ));
    }
    Ok(())
}

#[test]
fn given_similarly_named_constant_then_equality_boundary_stays_missing() -> Result<(), String> {
    let evidence = field_constant_evidence(
        r#"
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wrong_constant() {
        let mut file = File { body_model_version: 0, other_version: 0 };
        file.body_model_version = SUPPORTED_VERSION_COPY;
        assert!(lower(&file));
    }
}
"#,
    )?;
    if !evidence
        .observed_values
        .iter()
        .any(|fact| fact.value == "SUPPORTED_VERSION_COPY")
    {
        return Err(format!(
            "the exact assigned constant should remain visible: {:?}",
            evidence.observed_values
        ));
    }
    if evidence.missing_discriminators.is_empty() {
        return Err("a similarly named constant must not satisfy SUPPORTED_VERSION".to_string());
    }
    Ok(())
}

#[test]
fn given_assignment_only_in_unrelated_helper_or_test_then_value_is_not_credited()
-> Result<(), String> {
    let evidence = field_constant_evidence(
        r#"
#[cfg(test)]
mod tests {
    use super::*;

    fn assign_supported(file: &mut File) {
        file.body_model_version = SUPPORTED_VERSION;
    }

    #[test]
    fn unrelated_assignment() {
        let mut file = File { body_model_version: 0, other_version: 0 };
        assign_supported(&mut file);
    }

    #[test]
    fn related_without_direct_assignment() {
        let file = File { body_model_version: 0, other_version: 0 };
        assert!(!lower(&file));
    }
}
"#,
    )?;
    if evidence
        .observed_values
        .iter()
        .any(|fact| fact.value == "SUPPORTED_VERSION")
    {
        return Err(format!(
            "helper or unrelated-test assignments must not cross test scope: {:?}",
            evidence.observed_values
        ));
    }
    Ok(())
}

#[test]
fn given_direct_field_assignment_with_opaque_rhs_then_names_field_assignment_limitation()
-> Result<(), String> {
    let evidence = field_constant_evidence(
        r#"
#[cfg(test)]
mod tests {
    use super::*;

    fn unknown_version() -> u32 { 1 }

    #[test]
    fn opaque_assignment() {
        let mut file = File { body_model_version: 0, other_version: 0 };
        file.body_model_version = unknown_version();
        assert!(lower(&file));
    }
}
"#,
    )?;
    if !evidence
        .activate
        .summary
        .contains("Field assignment value is unresolved")
    {
        return Err(format!(
            "opaque direct assignment must name its limitation: {}",
            evidence.activate.summary
        ));
    }
    if !evidence.missing_discriminators.is_empty() {
        return Err(format!(
            "unsupported field assignment must not emit an ineffective repair: {:?}",
            evidence.missing_discriminators
        ));
    }
    Ok(())
}

#[test]
fn given_control_flow_nested_field_assignment_then_boundary_value_is_not_credited()
-> Result<(), String> {
    let evidence = field_constant_evidence(
        r#"
#[cfg(test)]
mod tests {
    use super::*;

    fn runtime_flag() -> bool { true }

    #[test]
    fn conditional_assignment() {
        let mut file = File { body_model_version: 0, other_version: 0 };
        if runtime_flag() {
            file.body_model_version = SUPPORTED_VERSION;
        }
        assert!(lower(&file));
    }
}
"#,
    )?;
    if evidence
        .observed_values
        .iter()
        .any(|fact| fact.value == "SUPPORTED_VERSION")
    {
        return Err(format!(
            "control-flow-nested field assignment must not be unconditional evidence: {:?}",
            evidence.observed_values
        ));
    }
    if !evidence
        .activate
        .summary
        .contains("Field assignment value is unresolved")
    {
        return Err(format!(
            "conditional direct assignment must name its limitation: {}",
            evidence.activate.summary
        ));
    }
    Ok(())
}

#[test]
fn given_mutable_borrow_after_field_assignment_then_stale_value_is_not_credited()
-> Result<(), String> {
    let evidence = field_constant_evidence(
        r#"
#[cfg(test)]
mod tests {
    use super::*;

    fn mutate(file: &mut File) {
        file.body_model_version = 0;
    }

    #[test]
    fn assignment_then_mutation() {
        let mut file = File { body_model_version: 0, other_version: 0 };
        file.body_model_version = SUPPORTED_VERSION;
        mutate(&mut file);
        assert!(lower(&file));
    }
}
"#,
    )?;
    if evidence
        .observed_values
        .iter()
        .any(|fact| fact.value == "SUPPORTED_VERSION")
    {
        return Err(format!(
            "field value must be invalidated by a later mutable borrow: {:?}",
            evidence.observed_values
        ));
    }
    if !evidence
        .activate
        .summary
        .contains("Field assignment value is unresolved")
    {
        return Err(format!(
            "mutable-alias invalidation must name its limitation: {}",
            evidence.activate.summary
        ));
    }
    Ok(())
}

#[test]
fn given_mutable_borrow_before_field_assignment_then_later_value_is_credited() -> Result<(), String>
{
    let evidence = field_constant_evidence(
        r#"
#[cfg(test)]
mod tests {
    use super::*;

    fn mutate(file: &mut File) {
        file.body_model_version = 0;
    }

    #[test]
    fn mutation_then_assignment() {
        let mut file = File { body_model_version: 0, other_version: 0 };
        mutate(&mut file);
        file.body_model_version = SUPPORTED_VERSION;
        assert!(lower(&file));
    }
}
"#,
    )?;
    if !evidence
        .observed_values
        .iter()
        .any(|fact| fact.value == "SUPPORTED_VERSION")
    {
        return Err(format!(
            "a later exact assignment must supersede an earlier mutable borrow: {:?}",
            evidence.observed_values
        ));
    }
    if !evidence.missing_discriminators.is_empty() {
        return Err(format!(
            "the later exact assignment should satisfy the equality boundary: {:?}",
            evidence.missing_discriminators
        ));
    }
    Ok(())
}

#[test]
fn iterator_boundary_operand_route_only_matches_iterator_loop_bindings() {
    for source in [
        "for (idx, value) in values.iter().enumerate() {",
        "for item in values.iter() {",
        "for item in values.iter_mut() {",
        "for item in values.into_iter() {",
        "for key in values.keys() {",
        "for value in values.values() {",
        "if ready { for idx in values.iter() {",
    ] {
        let operand = if source.contains("idx") {
            "idx"
        } else if source.contains("key") {
            "key"
        } else if source.contains("value in") {
            "value"
        } else {
            "item"
        };
        assert!(
            loop_binds_operand_from_iterator(source, operand),
            "iterator loop should bind {operand}: {source}"
        );
    }

    for (source, operand) in [
        ("let idx = offset + 1;", "idx"),
        ("for idx in 0..values.len() {", "idx"),
        ("for (idx, value) in values.iter().enumerate() {", "offset"),
        ("perform idx boundary checks", "idx"),
    ] {
        assert!(
            !loop_binds_operand_from_iterator(source, operand),
            "non-iterator or unbound operand must not match: {source}"
        );
    }

    assert!(is_boundary_operand_identifier("idx"));
    assert!(is_boundary_operand_identifier("_idx2"));
    assert!(!is_boundary_operand_identifier("idx + 1"));
    assert!(!is_boundary_operand_identifier("100"));
}

#[test]
fn given_same_file_const_when_owner_call_uses_identifier_then_observed_value_is_resolved()
-> Result<(), String> {
    let prod_src = "pub fn discounted_total(amount: i32, threshold: i32) -> i32 \
                        { if amount >= threshold { amount - 10 } else { amount } }\n";
    let test = (
        "tests/pricing_tests.rs",
        "const THRESHOLD: i32 = 100;\n\
             #[test] fn at_threshold() { \
                 assert_eq!(discounted_total(THRESHOLD, THRESHOLD), 90); \
             }\n",
    );
    let values = observed_values_for(prod_src, &[test])?;
    assert!(
        values.iter().any(|v| v == "100"),
        "const-resolved 100 must appear; got {values:?}"
    );
    Ok(())
}

#[test]
fn given_table_driven_cases_when_owner_call_uses_row_values_then_each_case_value_is_recorded()
-> Result<(), String> {
    let prod_src = "pub fn discounted_total(amount: i32, threshold: i32) -> i32 \
                        { if amount >= threshold { amount - 10 } else { amount } }\n";
    let test = (
        "tests/pricing_tests.rs",
        "#[test] fn table() { \
                 for (amount, threshold, expected) in [(50, 100, 50), (100, 100, 90)] { \
                     assert_eq!(discounted_total(amount, threshold), expected); \
                 } \
             }\n",
    );
    let values = observed_values_for(prod_src, &[test])?;
    assert!(
        values.iter().any(|v| v == "50"),
        "table row value 50 must appear; got {values:?}"
    );
    assert!(
        values.iter().any(|v| v == "100"),
        "table row value 100 must appear; got {values:?}"
    );
    Ok(())
}

#[test]
fn given_option_result_constructor_when_owner_call_uses_shape_then_inner_value_is_recorded()
-> Result<(), String> {
    // Owner takes a wrapped value; test calls with Some(literal).
    // Resolver should peel one level and emit the inner literal.
    let prod_src = "pub fn process(value: Option<i32>, threshold: i32) -> i32 \
                        { match value { Some(v) if v >= threshold => v - 10, _ => 0 } }\n";
    let test = (
        "tests/pricing_tests.rs",
        "#[test] fn at_boundary() { \
                 assert_eq!(process(Some(100), 100), 90); \
             }\n",
    );
    // The seam in this case is the predicate inside `process`.
    let mut files: Vec<(PathBuf, &str)> = vec![(PathBuf::from("src/pricing.rs"), prod_src)];
    files.push((PathBuf::from(test.0), test.1));
    let index = index_from_files(&files)?;
    let seams = inventory_seams_from_index(&[PathBuf::from("src/pricing.rs")], &index);
    let predicate = seams
        .iter()
        .find(|s| s.kind() == SeamKind::PredicateBoundary)
        .ok_or_else(|| "predicate seam present".to_string())?;
    let evidence = evidence_for_seam(predicate, &index);
    let values: Vec<String> = evidence
        .observed_values
        .iter()
        .map(|v| v.value.clone())
        .collect();
    assert!(
        values.iter().any(|v| v == "100"),
        "Some(100) must unwrap and contribute 100; got {values:?}"
    );
    Ok(())
}

#[test]
fn given_builder_methods_matching_parameter_tokens_then_observed_values_are_recorded()
-> Result<(), String> {
    // The seam's required-discriminator description carries the
    // identifiers `amount` and `discount_threshold`. A test that
    // builds a value via `.amount(100).discount_threshold(100)`
    // should have those literals counted as observed via the
    // BuilderMethod context. Owner name unused inside the builder
    // call — the test references the owner directly elsewhere so
    // it qualifies as related.
    let prod_src = "pub fn discounted_total(amount: i32, discount_threshold: i32) -> i32 \
                        { if amount >= discount_threshold { amount - 10 } else { amount } }\n";
    let test = (
        "tests/pricing_tests.rs",
        "#[test] fn via_builder() { \
                 let q = Quote::new().amount(100).discount_threshold(100).build(); \
                 assert_eq!(discounted_total(q.amount, q.discount_threshold), 90); \
             }\n",
    );
    let values = observed_values_for(prod_src, &[test])?;
    // `amount` and `discount_threshold` are seam-discriminator
    // tokens, so the builder method facts should land.
    assert!(
        values.iter().filter(|v| v.as_str() == "100").count() >= 1,
        "builder method 100 must be recorded; got {values:?}"
    );
    Ok(())
}

#[test]
fn given_fixture_factory_override_methods_matching_seam_tokens_then_values_are_recorded()
-> Result<(), String> {
    // Fixture factories often use explicit override method names
    // like `with_amount`. These should count when the wrapped
    // method token aligns with the changed seam.
    let prod_src = "pub fn discounted_total(amount: i32, threshold: i32) -> i32 \
                        { if amount >= threshold { amount - 10 } else { amount } }\n";
    let test = (
        "tests/pricing_tests.rs",
        "#[test] fn via_fixture_override() { \
                 let q = QuoteFixture::default().with_amount(100).with_threshold(100).build(); \
                 assert_eq!(discounted_total(q.amount, q.threshold), 90); \
             }\n",
    );
    let values = observed_values_for(prod_src, &[test])?;
    assert!(
        values.iter().filter(|v| v.as_str() == "100").count() >= 1,
        "fixture override 100 must be recorded; got {values:?}"
    );
    Ok(())
}

#[test]
fn given_same_test_struct_literal_fields_when_owner_call_uses_projection_then_values_are_recorded()
-> Result<(), String> {
    // Same-test struct literals are a syntactic fixture shape:
    // the field values are explicit in the test body and the owner
    // call passes those field projections directly.
    let prod_src = "pub fn discounted_total(amount: i32, threshold: i32) -> i32 \
                        { if amount >= threshold { amount - 10 } else { amount } }\n";
    let test = (
        "tests/pricing_tests.rs",
        "#[test] fn via_struct_literal() { \
                 let case = DiscountCase { amount: 100, threshold: 100 }; \
                 assert_eq!(discounted_total(case.amount, case.threshold), 90); \
             }\n",
    );
    let values = observed_values_for(prod_src, &[test])?;
    assert!(
        values.iter().any(|v| v == "100"),
        "same-test struct literal field value 100 must be recorded; got {values:?}"
    );
    Ok(())
}

#[test]
fn given_same_line_struct_literal_after_owner_call_then_projection_values_stay_unresolved()
-> Result<(), String> {
    // Call facts preserve only the line and full text, so
    // same-line ordering must stay conservative. A literal that
    // appears after the owner call cannot explain that call's
    // field projections.
    let prod_src = "pub fn discounted_total(amount: i32, threshold: i32) -> i32 \
                        { if amount >= threshold { amount - 10 } else { amount } }\n";
    let test = (
        "tests/pricing_tests.rs",
        "#[test] fn via_same_line_late_literal() { \
                 assert_eq!(discounted_total(case.amount, case.threshold), 90); \
                 let case = DiscountCase { amount: 100, threshold: 100 }; \
                 let _ = case; \
             }\n",
    );
    let values = observed_values_for(prod_src, &[test])?;
    assert!(
        values.is_empty(),
        "same-line literals introduced after the owner call must not produce fake values; got {values:?}"
    );
    Ok(())
}

#[test]
fn given_struct_literal_shadowed_after_owner_call_then_values_are_recorded() -> Result<(), String> {
    // Source order matters for the fixture shape: a later shadow
    // should not erase a direct owner call that already used the
    // same-test literal field projection.
    let prod_src = "pub fn discounted_total(amount: i32, threshold: i32) -> i32 \
                        { if amount >= threshold { amount - 10 } else { amount } }\n";
    let test = (
        "tests/pricing_tests.rs",
        "#[test]\nfn via_later_shadowed_fixture() {\n    \
                 let case = DiscountCase { amount: 100, threshold: 100 };\n    \
                 assert_eq!(discounted_total(case.amount, case.threshold), 90);\n    \
                 let case = make_discount_case();\n    \
                 let _ = case;\n\
             }\n",
    );
    let values = observed_values_for(prod_src, &[test])?;
    assert!(
        values.iter().any(|v| v == "100"),
        "later-shadowed struct literal field value 100 must still be recorded for the earlier owner call; got {values:?}"
    );
    Ok(())
}

#[test]
fn given_struct_literal_after_owner_call_then_projection_values_stay_unresolved()
-> Result<(), String> {
    // A later literal cannot explain a field projection that reached
    // the owner before the literal binding existed.
    let prod_src = "pub fn discounted_total(amount: i32, threshold: i32) -> i32 \
                        { if amount >= threshold { amount - 10 } else { amount } }\n";
    let test = (
        "tests/pricing_tests.rs",
        "#[test]\nfn via_late_literal_fixture() {\n    \
                 assert_eq!(discounted_total(case.amount, case.threshold), 90);\n    \
                 let case = DiscountCase { amount: 100, threshold: 100 };\n    \
                 let _ = case;\n\
             }\n",
    );
    let values = observed_values_for(prod_src, &[test])?;
    assert!(
        values.is_empty(),
        "literal struct fields introduced after the owner call must not produce fake values; got {values:?}"
    );
    Ok(())
}

#[test]
fn given_struct_literal_field_mutated_before_owner_call_then_values_stay_unresolved()
-> Result<(), String> {
    // A mutation before the owner call makes the original literal
    // field stale for activation-value evidence.
    let prod_src = "pub fn discounted_total(amount: i32, threshold: i32) -> i32 \
                        { if amount >= threshold { amount - 10 } else { amount } }\n";
    let test = (
        "tests/pricing_tests.rs",
        "#[test]\nfn via_mutated_fixture() {\n    \
                 let case = DiscountCase { amount: 100, threshold: 100 };\n    \
                 case.amount = make_amount();\n    \
                 assert_eq!(discounted_total(case.amount, case.threshold), 90);\n\
             }\n",
    );
    let values = observed_values_for(prod_src, &[test])?;
    assert!(
        values.is_empty(),
        "mutated struct literal fields must not reuse stale literal values; got {values:?}"
    );
    Ok(())
}

#[test]
fn given_struct_literal_field_mutated_after_owner_call_then_values_are_recorded()
-> Result<(), String> {
    // A mutation after the owner call should not erase the literal
    // value observed by the earlier owner call.
    let prod_src = "pub fn discounted_total(amount: i32, threshold: i32) -> i32 \
                        { if amount >= threshold { amount - 10 } else { amount } }\n";
    let test = (
        "tests/pricing_tests.rs",
        "#[test]\nfn via_later_mutated_fixture() {\n    \
                 let case = DiscountCase { amount: 100, threshold: 100 };\n    \
                 assert_eq!(discounted_total(case.amount, case.threshold), 90);\n    \
                 case.amount = make_amount();\n\
             }\n",
    );
    let values = observed_values_for(prod_src, &[test])?;
    assert!(
        values.iter().any(|v| v == "100"),
        "later-mutated struct literal field value 100 must still be recorded for the earlier owner call; got {values:?}"
    );
    Ok(())
}

#[test]
fn given_helper_built_struct_when_owner_call_uses_projection_then_values_stay_unresolved()
-> Result<(), String> {
    // Do not infer through helper-returned fixtures. Without the
    // literal struct body in the same test, `case.amount` remains
    // an opaque activation value and should stay a named static
    // limitation instead of becoming user test debt.
    let prod_src = "pub fn discounted_total(amount: i32, threshold: i32) -> i32 \
                        { if amount >= threshold { amount - 10 } else { amount } }\n";
    let test = (
        "tests/pricing_tests.rs",
        "#[test] fn via_helper_fixture() { \
                 let case = make_discount_case(); \
                 assert_eq!(discounted_total(case.amount, case.threshold), 90); \
             }\n",
    );
    let values = observed_values_for(prod_src, &[test])?;
    assert!(
        values.is_empty(),
        "helper-built struct projections must not produce fake values; got {values:?}"
    );
    Ok(())
}

#[test]
fn given_shadowed_struct_literal_when_owner_call_uses_projection_then_values_stay_unresolved()
-> Result<(), String> {
    // A same-test literal stops being a safe activation value once
    // the binding is shadowed before the owner call. The resolver
    // deliberately avoids reusing stale literal fields after the
    // shadowing line.
    let prod_src = "pub fn discounted_total(amount: i32, threshold: i32) -> i32 \
                        { if amount >= threshold { amount - 10 } else { amount } }\n";
    let test = (
        "tests/pricing_tests.rs",
        "#[test] fn via_shadowed_fixture() { \
                 let case = DiscountCase { amount: 100, threshold: 100 }; \
                 let case = make_discount_case(); \
                 assert_eq!(discounted_total(case.amount, case.threshold), 90); \
             }\n",
    );
    let values = observed_values_for(prod_src, &[test])?;
    assert!(
        values.is_empty(),
        "shadowed struct projections must not reuse stale literal fields; got {values:?}"
    );
    Ok(())
}

#[test]
fn given_fixture_parameter_and_later_same_name_struct_literal_then_values_stay_unresolved()
-> Result<(), String> {
    // A fixture/rstest parameter is runtime-provided. A later
    // same-name literal in the same body cannot safely explain the
    // earlier owner-call field projection.
    let prod_src = "pub fn discounted_total(amount: i32, threshold: i32) -> i32 \
                        { if amount >= threshold { amount - 10 } else { amount } }\n";
    let test = (
        "tests/pricing_tests.rs",
        "#[rstest] fn via_fixture_param(case: DiscountCase) { \
                 assert_eq!(discounted_total(case.amount, case.threshold), 90); \
                 let case = DiscountCase { amount: 100, threshold: 100 }; \
                 let _ = case; \
             }\n",
    );
    let values = observed_values_for(prod_src, &[test])?;
    assert!(
        values.is_empty(),
        "fixture parameter projections must not resolve from later literals; got {values:?}"
    );
    Ok(())
}

#[test]
fn given_for_loop_shadowing_struct_literal_then_values_stay_unresolved() -> Result<(), String> {
    // The whole-test literal map cannot safely explain a projection
    // when a later loop binder reuses the same identifier.
    let prod_src = "pub fn discounted_total(amount: i32, threshold: i32) -> i32 \
                        { if amount >= threshold { amount - 10 } else { amount } }\n";
    let test = (
        "tests/pricing_tests.rs",
        "#[test] fn via_shadowing_loop() { \
                 let case = DiscountCase { amount: 100, threshold: 100 }; \
                 for case in helper_cases() { \
                     assert_eq!(discounted_total(case.amount, case.threshold), 90); \
                 } \
             }\n",
    );
    let values = observed_values_for(prod_src, &[test])?;
    assert!(
        values.is_empty(),
        "loop-shadowed struct projections must not reuse stale literal fields; got {values:?}"
    );
    Ok(())
}

#[test]
fn given_let_pattern_shadowing_struct_literal_then_values_stay_unresolved() -> Result<(), String> {
    // Non-simple let patterns can bind a fresh value under the same
    // name. Without source-order scope, that remains a limitation.
    let prod_src = "pub fn discounted_total(amount: i32, threshold: i32) -> i32 \
                        { if amount >= threshold { amount - 10 } else { amount } }\n";
    let test = (
        "tests/pricing_tests.rs",
        "#[test] fn via_shadowing_let_pattern() { \
                 let case = DiscountCase { amount: 100, threshold: 100 }; \
                 let Some(case) = make_discount_case() else { return; }; \
                 assert_eq!(discounted_total(case.amount, case.threshold), 90); \
             }\n",
    );
    let values = observed_values_for(prod_src, &[test])?;
    assert!(
        values.is_empty(),
        "let-pattern-shadowed projections must not reuse stale literal fields; got {values:?}"
    );
    Ok(())
}

#[test]
fn given_builder_method_with_unrelated_name_then_value_is_not_counted_for_seam_activation()
-> Result<(), String> {
    // `.with_seed(42)` is a builder method whose name does NOT
    // align with any seam token. The value 42 must NOT appear
    // among observed values for this seam, even though the test
    // directly calls the owner.
    let prod_src = "pub fn discounted_total(amount: i32, threshold: i32) -> i32 \
                        { if amount >= threshold { amount - 10 } else { amount } }\n";
    let test = (
        "tests/pricing_tests.rs",
        "#[test] fn via_unrelated_builder() { \
                 let _q = Foo::new().with_seed(42).build(); \
                 assert_eq!(discounted_total(50, 100), 50); \
             }\n",
    );
    let values = observed_values_for(prod_src, &[test])?;
    assert!(
        !values.iter().any(|v| v == "42"),
        "unrelated builder literal 42 must NOT count; got {values:?}"
    );
    Ok(())
}

#[test]
fn given_unrelated_string_literal_mentions_value_when_extracting_values_then_no_observed_discriminator_is_recorded()
-> Result<(), String> {
    // String literal in the body mentions `100` and `threshold`
    // but the call site uses an unresolved identifier. v2 must
    // not pull literals out of strings.
    let prod_src = "pub fn discounted_total(amount: i32, threshold: i32) -> i32 \
                        { if amount >= threshold { amount - 10 } else { amount } }\n";
    let test = (
        "tests/pricing_tests.rs",
        "#[test] fn string_only() { \
                 let _doc = \"threshold = 100\"; \
                 let unresolved = make_amount(); \
                 assert_eq!(discounted_total(unresolved, unresolved), 0); \
             }\n",
    );
    let values = observed_values_for(prod_src, &[test])?;
    assert!(
        !values.iter().any(|v| v == "100"),
        "string literal 100 must NOT be observed; got {values:?}"
    );
    Ok(())
}

#[test]
fn given_shared_fixture_module_constant_when_extracting_v2_values_then_no_cross_file_value_is_resolved()
-> Result<(), String> {
    // Strict syntactic scope: cross-file constants must NOT
    // resolve. The const lives in tests/common/mod.rs; the test
    // lives in tests/pricing_tests.rs. v2 is single-file scope -
    // cross-file resolution is a future item and must not creep
    // in via "helpful" expansion.
    let prod_src = "pub fn discounted_total(amount: i32, threshold: i32) -> i32 \
                        { if amount >= threshold { amount - 10 } else { amount } }\n";
    let common = (
        "tests/common/mod.rs",
        "pub const SHARED_THRESHOLD: i32 = 100;\n",
    );
    let test = (
        "tests/pricing_tests.rs",
        "#[test] fn cross_file() { \
                 assert_eq!(discounted_total(SHARED_THRESHOLD, SHARED_THRESHOLD), 90); \
             }\n",
    );
    let values = observed_values_for(prod_src, &[test, common])?;
    assert!(
        !values.iter().any(|v| v == "100"),
        "cross-file SHARED_THRESHOLD = 100 must NOT resolve in v2; got {values:?}"
    );
    Ok(())
}

#[test]
fn given_let_binding_shadowed_by_comment_when_extracting_then_real_binding_wins()
-> Result<(), String> {
    // Mirrors #310's comment-stripping defense: a `// let amount = 999;`
    // comment must NOT shadow the real `let amount = 100;` binding.
    let prod_src = "pub fn discounted_total(amount: i32, threshold: i32) -> i32 \
                        { if amount >= threshold { amount - 10 } else { amount } }\n";
    let test = (
        "tests/pricing_tests.rs",
        "#[test] fn at_threshold() { \
                 // let amount = 999; let threshold = 999;\n\
                 let amount = 100; let threshold = 100; \
                 assert_eq!(discounted_total(amount, threshold), 90); \
             }\n",
    );
    let values = observed_values_for(prod_src, &[test])?;
    assert!(
        values.iter().any(|v| v == "100"),
        "real let binding 100 must be observed; got {values:?}"
    );
    assert!(
        !values.iter().any(|v| v == "999"),
        "commented-out let binding 999 must NOT be observed; got {values:?}"
    );
    Ok(())
}

#[test]
fn given_unresolved_identifier_arg_when_extracting_values_then_no_observed_value_is_recorded()
-> Result<(), String> {
    // Identifier resolved through a helper call (no `let` binding,
    // no const, no rstest case, no table row, no Some wrapper).
    // Must stay opaque — the previous behavior is preserved for
    // the unresolved case.
    let prod_src = "pub fn discounted_total(amount: i32, threshold: i32) -> i32 \
                        { if amount >= threshold { amount - 10 } else { amount } }\n";
    let test = (
        "tests/pricing_tests.rs",
        "#[test] fn opaque() { \
                 let amount = make_amount(); \
                 let threshold = make_threshold(); \
                 assert_eq!(discounted_total(amount, threshold), 0); \
             }\n",
    );
    let values = observed_values_for(prod_src, &[test])?;
    // The let RHS isn't a literal, so the binding shouldn't
    // resolve. observed_values for these args should stay empty.
    assert!(
        values.is_empty()
            || values
                .iter()
                .all(|v| !matches!(v.as_str(), "100" | "0" | "make_amount")),
        "opaque args must not produce a fake observed value; got {values:?}"
    );
    Ok(())
}

#[test]
fn same_file_test_helper_call_counts_as_owner_call_evidence() {
    let file = PathBuf::from("src/pricing.rs");
    let index = RustIndex {
            functions: vec![FunctionSummary {
                id: crate::domain::SymbolId("src/pricing.rs::discounted_total".to_string()),
                name: "discounted_total".to_string(),
                file: file.clone(),
                start_line: 1,
                end_line: 5,
                body: "pub fn discounted_total(amount: i32, threshold: i32) -> i32 { if amount >= threshold { amount - 10 } else { amount } }".to_string(),
                calls: Vec::new(),
                returns: Vec::new(),
                literals: Vec::new(),
                is_test: false,
                attrs: Vec::new(),
            }, FunctionSummary {
                id: crate::domain::SymbolId("src/pricing.rs::case_at_threshold".to_string()),
                name: "case_at_threshold".to_string(),
                file: file.clone(),
                start_line: 10,
                end_line: 12,
                body: "fn case_at_threshold() -> i32 { discounted_total(100, 100) }".to_string(),
                calls: vec![CallFact {
                    line: 11,
                    name: "discounted_total".to_string(),
                    text: "discounted_total(100, 100)".to_string(),
                }],
                returns: Vec::new(),
                literals: Vec::new(),
                is_test: false,
                attrs: Vec::new(),
            }],
            tests: vec![TestSummary {
                name: "unit_test_uses_same_file_helper".to_string(),
                file: file.clone(),
                start_line: 20,
                end_line: 23,
                body: "#[test] fn unit_test_uses_same_file_helper() { assert_eq!(case_at_threshold(), 90); }".to_string(),
                calls: vec![CallFact {
                    line: 21,
                    name: "case_at_threshold".to_string(),
                    text: "case_at_threshold()".to_string(),
                }],
                assertions: vec![OracleFact {
                    line: 21,
                    kind: OracleKind::ExactValue,
                    strength: OracleStrength::Strong,
                    text: "assert_eq!(case_at_threshold(), 90)".to_string(),
                    observed_tokens: Vec::new(),
                }],
                literals: Vec::new(),
                attrs: Vec::new(),
            }],
            ..RustIndex::default()
        };

    let context = CompactGripContext::new(&index);

    assert!(
        context.tests[0]
            .helper_owner_call_names
            .contains("discounted_total"),
        "same-file helper call must prove the production owner call"
    );
    assert_eq!(
        context
            .tests_by_helper_owner_call_name
            .get("discounted_total")
            .cloned()
            .unwrap_or_default(),
        vec![0]
    );
}

// RIPR-SPEC-0103 fixture 5: parity table for oracle_kind_matches_seam_kind.
// ErrorVariant accepts ONLY ExactErrorVariant; rejects all value/mock oracles.
// Value seams accept ExactValue/WholeObjectEquality/Snapshot/RelationalCheck.
// SideEffect/CallPresence accept ONLY MockExpectation.
#[test]
fn oracle_kind_matches_seam_kind_error_variant_accepts_only_exact_error_variant() {
    use crate::domain::OracleKind;
    // ErrorVariant + ExactErrorVariant → true
    assert!(
        oracle_kind_matches_seam_kind(SeamKind::ErrorVariant, &OracleKind::ExactErrorVariant),
        "ErrorVariant must accept ExactErrorVariant"
    );
    // ErrorVariant rejects all other kinds
    for rejected in [
        OracleKind::ExactValue,
        OracleKind::WholeObjectEquality,
        OracleKind::Snapshot,
        OracleKind::RelationalCheck,
        OracleKind::MockExpectation,
        OracleKind::BroadError,
        OracleKind::SmokeOnly,
    ] {
        assert!(
            !oracle_kind_matches_seam_kind(SeamKind::ErrorVariant, &rejected),
            "ErrorVariant must reject {rejected:?}"
        );
    }
}

#[test]
fn oracle_kind_matches_seam_kind_value_seams_accept_exact_value() {
    use crate::domain::OracleKind;
    for value_seam in [
        SeamKind::PredicateBoundary,
        SeamKind::ReturnValue,
        SeamKind::MatchArm,
        SeamKind::FieldConstruction,
    ] {
        assert!(
            oracle_kind_matches_seam_kind(value_seam, &OracleKind::ExactValue),
            "{value_seam:?} must accept ExactValue"
        );
        assert!(
            oracle_kind_matches_seam_kind(value_seam, &OracleKind::WholeObjectEquality),
            "{value_seam:?} must accept WholeObjectEquality"
        );
        assert!(
            oracle_kind_matches_seam_kind(value_seam, &OracleKind::Snapshot),
            "{value_seam:?} must accept Snapshot"
        );
        assert!(
            oracle_kind_matches_seam_kind(value_seam, &OracleKind::RelationalCheck),
            "{value_seam:?} must accept RelationalCheck"
        );
        // Value seams must reject error-kind and mock-kind oracles
        assert!(
            !oracle_kind_matches_seam_kind(value_seam, &OracleKind::ExactErrorVariant),
            "{value_seam:?} must reject ExactErrorVariant"
        );
        assert!(
            !oracle_kind_matches_seam_kind(value_seam, &OracleKind::MockExpectation),
            "{value_seam:?} must reject MockExpectation"
        );
    }
}

#[test]
fn oracle_kind_matches_seam_kind_side_effect_accepts_only_mock_expectation() {
    use crate::domain::OracleKind;
    for effect_seam in [SeamKind::SideEffect, SeamKind::CallPresence] {
        assert!(
            oracle_kind_matches_seam_kind(effect_seam, &OracleKind::MockExpectation),
            "{effect_seam:?} must accept MockExpectation"
        );
        assert!(
            !oracle_kind_matches_seam_kind(effect_seam, &OracleKind::ExactValue),
            "{effect_seam:?} must reject ExactValue"
        );
        assert!(
            !oracle_kind_matches_seam_kind(effect_seam, &OracleKind::ExactErrorVariant),
            "{effect_seam:?} must reject ExactErrorVariant"
        );
    }
}
