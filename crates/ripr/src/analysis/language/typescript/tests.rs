//! Tests for the TypeScript preview adapter.

use super::*;
use std::path::{Path, PathBuf};

fn changed(path: &str) -> ChangedFile {
    ChangedFile {
        path: PathBuf::from(path),
        added_lines: Vec::new(),
        removed_lines: Vec::new(),
    }
}

fn test_owner(name: &str, file: &str) -> TypeScriptOwner {
    TypeScriptOwner {
        name: name.to_string(),
        file: PathBuf::from(file),
        start_line: 1,
        end_line: 20,
        owner_kind: OwnerKind::Function,
        class_name: None,
        decorated: false,
        exported_as_default: false,
        class_default_export: false,
        arity: None,
        parameters: Vec::new(),
        source_text: None,
        imports: Vec::new(),
        method_kind: TypeScriptMethodKind::Ordinary,
    }
}

fn smoke_assertion() -> TypeScriptAssertion {
    TypeScriptAssertion {
        matcher: "toBeTruthy".to_string(),
        argument_count: 0,
        line: 2,
        oracle_kind: OracleKind::SmokeOnly,
        oracle_strength: OracleStrength::Smoke,
        mock_payload: None,
        error_payload: None,
        observed_expression: None,
        expected_value_or_variant: None,
        has_dynamic_matcher_arg: false,
        oracle_confidence: OracleConfidence::Low,
    }
}

fn weak_direct_test_for(owner_name: &str) -> TypeScriptTest {
    TypeScriptTest {
        name: format!("{owner_name} smoke"),
        local_name: format!("{owner_name} smoke"),
        describe_names: Vec::new(),
        file: PathBuf::from("tests/lib.test.ts"),
        line: 1,
        body_text: format!("const result = {owner_name}(50, 100);\nexpect(result).toBeTruthy();"),
        assertions: vec![smoke_assertion()],
        mocks_in_file: Vec::new(),
        imports_in_file: Vec::new(),
    }
}

#[test]
fn class_method_no_static_path_guidance_names_current_supported_boundary() {
    let mut owner = test_owner("build", "src/owners.ts");
    owner.owner_kind = OwnerKind::ClassMethod;
    owner.class_name = Some("Cart".to_string());

    let missing = no_static_path_missing(&owner);
    let recommendation = no_static_path_recommendation(&owner);

    assert!(
        missing.contains("Direct same-file or imported `Class.method(...)` calls are supported")
    );
    assert!(missing.contains("local shadows"));
    assert!(missing.contains("dynamic member access"));
    assert!(!missing.contains("class-method related-test matching lands"));
    assert!(recommendation.contains("direct same-file or imported `Class.method(...)` observer"));
    assert!(recommendation.contains("namespace chains"));
    assert!(!recommendation.contains("class-method related-test matching lands"));
}

fn mock_interaction_test_for(owner_name: &str) -> TypeScriptTest {
    TypeScriptTest {
        name: format!("{owner_name} records status"),
        local_name: format!("{owner_name} records status"),
        describe_names: Vec::new(),
        file: PathBuf::from("tests/lib.test.ts"),
        line: 1,
        body_text: format!(
            "const sink = {{ record: vi.fn() }};\n{owner_name}(status, sink);\nexpect(sink.record).toHaveBeenCalledWith(status);"
        ),
        assertions: vec![TypeScriptAssertion {
            matcher: "toHaveBeenCalledWith".to_string(),
            argument_count: 1,
            line: 3,
            oracle_kind: OracleKind::MockExpectation,
            oracle_strength: OracleStrength::Medium,
            mock_payload: None,
            error_payload: None,
            observed_expression: None,
            expected_value_or_variant: None,
            has_dynamic_matcher_arg: false,
            oracle_confidence: OracleConfidence::Medium,
        }],
        mocks_in_file: Vec::new(),
        imports_in_file: Vec::new(),
    }
}

fn direct_test_with_assertion(
    test_name: &str,
    body_text: impl Into<String>,
    matcher: &str,
    argument_count: usize,
    oracle_kind: OracleKind,
    oracle_strength: OracleStrength,
) -> TypeScriptTest {
    TypeScriptTest {
        name: test_name.to_string(),
        local_name: test_name.to_string(),
        describe_names: Vec::new(),
        file: PathBuf::from("tests/lib.test.ts"),
        line: 1,
        body_text: body_text.into(),
        assertions: vec![TypeScriptAssertion {
            matcher: matcher.to_string(),
            argument_count,
            line: 2,
            oracle_kind: oracle_kind.clone(),
            oracle_strength: oracle_strength.clone(),
            mock_payload: None,
            error_payload: None,
            observed_expression: None,
            expected_value_or_variant: None,
            has_dynamic_matcher_arg: false,
            oracle_confidence: OracleConfidence::Unknown,
        }],
        mocks_in_file: Vec::new(),
        imports_in_file: Vec::new(),
    }
}

fn heuristic_name_test_for(owner_name: &str) -> TypeScriptTest {
    TypeScriptTest {
        name: format!("{owner_name} boundary"),
        local_name: format!("{owner_name} boundary"),
        describe_names: Vec::new(),
        file: PathBuf::from("tests/lib.test.ts"),
        line: 1,
        // References the owner without a recognized call shape: a heuristic
        // link requires a reference (RIPR-SPEC-0027).
        body_text: format!("const subject = {owner_name};\nexpect(90).toBe(90);"),
        assertions: vec![TypeScriptAssertion {
            matcher: "toBe".to_string(),
            argument_count: 1,
            line: 1,
            oracle_kind: OracleKind::ExactValue,
            oracle_strength: OracleStrength::Strong,
            mock_payload: None,
            error_payload: None,
            observed_expression: None,
            expected_value_or_variant: None,
            has_dynamic_matcher_arg: false,
            oracle_confidence: OracleConfidence::Medium,
        }],
        mocks_in_file: Vec::new(),
        imports_in_file: Vec::new(),
    }
}

fn classify_weak_direct_line(line_text: &str) -> Result<Finding, String> {
    let owner = test_owner("applyDiscount", "src/lib.ts");
    let test = weak_direct_test_for("applyDiscount");
    classify_change(
        Path::new("src/lib.ts"),
        2,
        line_text,
        &[owner],
        &[test],
        None,
        &ReExportIndex::empty(),
        None,
    )
    .ok_or_else(|| "expected TypeScript preview finding".to_string())
}

fn missing_discriminator_values(finding: &Finding) -> Vec<String> {
    finding
        .activation
        .missing_discriminators
        .iter()
        .map(|fact| fact.value.clone())
        .collect()
}

fn bun_fact_kinds_for_source(source: &str) -> Vec<&'static str> {
    let tests = extract_tests(Path::new("test/js/web/fetch/blob.test.ts"), source);
    let mut kinds = tests
        .iter()
        .flat_map(bun_array_buffer_facts_for_test)
        .map(|fact| fact.kind.as_str())
        .collect::<Vec<_>>();
    kinds.sort();
    kinds.dedup();
    kinds
}

fn assert_static_limit(finding: &Finding, kind: StaticLimitKind, expected_text: &str) {
    assert_eq!(finding.static_limit_kind, Some(kind));
    assert!(
        finding
            .evidence
            .iter()
            .any(|line| line.contains(expected_text)),
        "expected evidence containing {expected_text:?}, got {:?}",
        finding.evidence
    );
    assert!(
        finding
            .missing
            .iter()
            .any(|line| line.contains(expected_text)),
        "expected missing text containing {expected_text:?}, got {:?}",
        finding.missing
    );
    let recommended = finding.recommended_next_step.as_deref().unwrap_or_default();
    assert!(
        recommended.contains(expected_text) && recommended.contains("Repair route:"),
        "expected limitation-oriented next step for {expected_text:?}, got {recommended:?}"
    );
    assert!(finding.activation.missing_discriminators.is_empty());
    assert_evidence_contains(finding, "gap_state: static_limitation");
    assert_evidence_contains(
        finding,
        &format!("actionability_category: {}", kind.as_str()),
    );
    assert_evidence_contains(finding, "why_not_actionable: static limit");
}

fn assert_bun_fact(source: &str, expected: TypeScriptBunArrayBufferFactKind) {
    let kinds = bun_fact_kinds_for_source(source);
    assert!(
        kinds.contains(&expected.as_str()),
        "expected Bun ArrayBuffer fact {:?}, got {:?}",
        expected,
        kinds
    );
}

fn bun_bridge_hint_for_source_with_confidence(
    source: &str,
    confidence: TypeScriptBunBridgeConfidence,
) -> Option<TypeScriptBunBridgeHint> {
    let tests = extract_tests(Path::new(BUN_BLOB_ARRAY_BUFFER_TS_TEST_FILE), source);
    let facts = tests
        .iter()
        .flat_map(bun_array_buffer_facts_for_test)
        .collect::<Vec<_>>();
    let profile = TypeScriptBunBridgeProfile {
        confidence,
        ..BUN_BLOB_ARRAY_BUFFER_BRIDGE_PROFILE
    };
    bun_bridge_hint_for_profile(&facts, profile)
}

fn bun_bridge_hint_for_source(source: &str) -> Result<TypeScriptBunBridgeHint, String> {
    bun_bridge_hint_for_source_with_confidence(
        source,
        TypeScriptBunBridgeConfidence::ConfiguredHint,
    )
    .ok_or_else(|| "expected configured Bun bridge hint".to_string())
}

fn bun_cross_language_finding_for_source(source: &str) -> Result<Finding, String> {
    bun_cross_language_finding_for_source_with_confidence(
        source,
        TypeScriptBunBridgeConfidence::ConfiguredHint,
    )
}

fn bun_cross_language_finding_for_source_with_confidence(
    source: &str,
    confidence: TypeScriptBunBridgeConfidence,
) -> Result<Finding, String> {
    bun_cross_language_finding_for_source_with_profile_and_confidence(
        source,
        BUN_BLOB_ARRAY_BUFFER_BRIDGE_PROFILE,
        confidence,
        3420,
        "    if (array_buffer.shared || array_buffer.resizable) {",
    )
}

fn bun_cross_language_finding_for_source_with_profile_and_confidence(
    source: &str,
    profile: TypeScriptBunBridgeProfile,
    confidence: TypeScriptBunBridgeConfidence,
    rust_line: usize,
    line_text: &str,
) -> Result<Finding, String> {
    let tests = extract_tests(Path::new(profile.ts_test_file), source);
    let profile = TypeScriptBunBridgeProfile {
        confidence,
        ..profile
    };
    bun_cross_language_finding_for_changed_rust_line_with_profile(
        Path::new(profile.rust_file),
        rust_line,
        line_text,
        &tests,
        profile,
    )
    .ok_or_else(|| "expected Bun cross-language finding".to_string())
}

fn bun_markdown_bridge_hint_for_source(source: &str) -> Result<TypeScriptBunBridgeHint, String> {
    let tests = extract_tests(Path::new(BUN_MARKDOWN_RESIZABLE_TS_TEST_FILE), source);
    let facts = tests
        .iter()
        .flat_map(bun_array_buffer_facts_for_test)
        .collect::<Vec<_>>();
    bun_bridge_hint_for_profile(&facts, BUN_MARKDOWN_RESIZABLE_BRIDGE_PROFILE)
        .ok_or_else(|| "expected Bun markdown bridge hint".to_string())
}

fn bun_markdown_cross_language_finding_for_source(source: &str) -> Result<Finding, String> {
    let tests = extract_tests(Path::new(BUN_MARKDOWN_RESIZABLE_TS_TEST_FILE), source);
    bun_cross_language_finding_for_changed_rust_line_with_profile(
        Path::new(BUN_MARKDOWN_RESIZABLE_RUST_FILE),
        60,
        "    if self.0.resizable && !self.0.shared {",
        &tests,
        BUN_MARKDOWN_RESIZABLE_BRIDGE_PROFILE,
    )
    .ok_or_else(|| "expected Bun markdown cross-language finding".to_string())
}

fn assert_evidence_contains(finding: &Finding, expected_text: &str) {
    assert!(
        finding
            .evidence
            .iter()
            .any(|line| line.contains(expected_text)),
        "expected evidence containing {expected_text:?}, got {:?}",
        finding.evidence
    );
}

fn assert_evidence_lacks(finding: &Finding, unexpected_text: &str) {
    assert!(
        finding
            .evidence
            .iter()
            .all(|line| !line.contains(unexpected_text)),
        "unexpected evidence containing {unexpected_text:?}, got {:?}",
        finding.evidence
    );
}

#[test]
fn extract_tests_classifies_bun_blob_shared_and_resizable_discriminators() {
    let source = r#"
test("blob copies shared and resizable buffers", async () => {
  const shared = new SharedArrayBuffer(4);
  const growable = new ArrayBuffer(4, { maxByteLength: 8 });
  growable.resize(6);
  const view = new Uint8Array(growable);
  const blob = new Blob([view, new Uint8Array(shared)]);
  const copied = new Uint8Array(await blob.arrayBuffer());
  expect([...copied]).toEqual([0, 0, 0, 0, 0, 0]);
});
"#;
    let kinds = bun_fact_kinds_for_source(source);

    assert_eq!(
        kinds,
        vec![
            "array_buffer_resize",
            "array_buffer_view",
            "blob_array_buffer_observer",
            "resizable_array_buffer",
            "shared_array_buffer",
            "stable_byte_copy_oracle",
            "view_backed_blob_input",
        ]
    );
}

#[test]
fn extract_tests_marks_max_byte_length_without_blob_observer_as_mention_only() {
    let source = r#"
test("records growable allocation shape", () => {
  const growable = new ArrayBuffer(4, { maxByteLength: 8 });
  expect(growable.byteLength).toBe(4);
});
"#;
    let kinds = bun_fact_kinds_for_source(source);

    assert!(kinds.contains(&"resizable_array_buffer"));
    assert!(kinds.contains(&"max_byte_length_mention_only"));
    assert!(!kinds.contains(&"view_backed_blob_input"));
    assert!(!kinds.contains(&"stable_byte_copy_oracle"));
}

#[test]
fn extract_tests_does_not_credit_blob_without_parts_array_as_view_backed() {
    let source = r#"
test("unrelated view and scalar blob", () => {
  const view = new Uint8Array(4);
  const blob = new Blob("not a parts array");
  expect(view.byteLength).toBe(4);
  expect(blob).toBeDefined();
});
"#;
    let kinds = bun_fact_kinds_for_source(source);

    assert!(kinds.contains(&"array_buffer_view"));
    assert!(!kinds.contains(&"view_backed_blob_input"));
}

#[test]
fn extract_tests_ignores_bun_array_buffer_comment_and_string_mentions() {
    let source = r#"
test("mentions new SharedArrayBuffer( in the title", () => {
  // new ArrayBuffer(4, { maxByteLength: 8 })
  const note = "new Blob([new Uint8Array(await blob.arrayBuffer())])";
  expect(note).toBe("new Blob([new Uint8Array(await blob.arrayBuffer())])");
});
"#;

    assert!(bun_fact_kinds_for_source(source).is_empty());
}

#[test]
fn extract_tests_recognizes_text_blob_stable_oracle() {
    let source = r#"
test("blob text is stable", async () => {
  const view = new Uint8Array(new ArrayBuffer(4, { maxByteLength: 8 }));
  const blob = new Blob([view]);
  expect(await blob.text()).toBe("\0\0\0\0");
});
"#;

    assert_bun_fact(
        source,
        TypeScriptBunArrayBufferFactKind::StableByteCopyOracle,
    );
    assert_bun_fact(
        source,
        TypeScriptBunArrayBufferFactKind::ViewBackedBlobInput,
    );
    assert!(!bun_fact_kinds_for_source(source).contains(&"max_byte_length_mention_only"));
}

#[test]
fn extract_tests_marks_blob_byte_smoke_assertion_as_weak_oracle() {
    let source = r#"
test("blob byte smoke is not stable", async () => {
  const view = new Uint8Array(new ArrayBuffer(4, { maxByteLength: 8 }));
  const blob = new Blob([view]);
  const copied = new Uint8Array(await blob.arrayBuffer());
  expect(copied).toBeDefined();
});
"#;
    let kinds = bun_fact_kinds_for_source(source);

    assert!(kinds.contains(&"weak_byte_smoke_oracle"));
    assert!(!kinds.contains(&"stable_byte_copy_oracle"));
}

#[test]
fn extract_tests_marks_blob_byte_snapshot_assertion_as_weak_oracle() {
    let source = r#"
test("blob byte snapshot is not stable", async () => {
  const view = new Uint8Array(new ArrayBuffer(4, { maxByteLength: 8 }));
  const blob = new Blob([view]);
  const copied = new Uint8Array(await blob.arrayBuffer());
  expect([...copied]).toMatchSnapshot();
});
"#;
    let kinds = bun_fact_kinds_for_source(source);

    assert!(kinds.contains(&"weak_byte_snapshot_oracle"));
    assert!(!kinds.contains(&"stable_byte_copy_oracle"));
}

#[test]
fn extract_tests_marks_blob_byte_read_without_assertion_as_mention_only() {
    let source = r#"
test("blob byte read alone is not an oracle", async () => {
  const view = new Uint8Array(new ArrayBuffer(4, { maxByteLength: 8 }));
  const blob = new Blob([view]);
  await blob.arrayBuffer();
});
"#;
    let kinds = bun_fact_kinds_for_source(source);

    assert!(kinds.contains(&"byte_oracle_mention_only"));
    assert!(!kinds.contains(&"stable_byte_copy_oracle"));
}

#[test]
fn bun_bridge_hint_classifies_shared_and_resizable_blob_observer() -> Result<(), String> {
    let source = r#"
test("blob copies shared and resizable buffers", async () => {
  const shared = new SharedArrayBuffer(4);
  const growable = new ArrayBuffer(4, { maxByteLength: 8 });
  const view = new Uint8Array(growable);
  const blob = new Blob([view, new Uint8Array(shared)]);
  const copied = new Uint8Array(await blob.arrayBuffer());
  expect([...copied]).toEqual([0, 0, 0, 0]);
});
"#;

    let hint = bun_bridge_hint_for_source(source)?;

    assert_eq!(
        hint.confidence,
        TypeScriptBunBridgeConfidence::ConfiguredHint
    );
    assert_eq!(hint.verdict, TypeScriptBunBridgeVerdict::TsDiscriminated);
    assert_eq!(hint.verdict.missing_discriminators(), &[] as &[&str]);
    assert_eq!(hint.rust_file, BUN_BLOB_ARRAY_BUFFER_RUST_FILE);
    assert_eq!(hint.rust_owner, BUN_BLOB_ARRAY_BUFFER_RUST_OWNER);
    Ok(())
}

#[test]
fn bun_bridge_hint_names_missing_resizable_discriminator() -> Result<(), String> {
    let source = r#"
test("blob copies shared buffers", async () => {
  const shared = new SharedArrayBuffer(4);
  const blob = new Blob([new Uint8Array(shared)]);
  const copied = new Uint8Array(await blob.arrayBuffer());
  expect([...copied]).toEqual([0, 0, 0, 0]);
});
"#;

    let hint = bun_bridge_hint_for_source(source)?;

    assert_eq!(hint.verdict, TypeScriptBunBridgeVerdict::TsMissingResizable);
    assert_eq!(
        hint.verdict.missing_discriminators(),
        &["resizable_array_buffer"]
    );
    assert_eq!(
        hint.verdict.expected_action(),
        "route_cross_language_oracle_visibility_limitation"
    );
    assert_eq!(
        hint.suggested_test_file(),
        BUN_BLOB_ARRAY_BUFFER_TS_TEST_FILE
    );
    assert_eq!(
        hint.placement_reason().as_deref(),
        Some(
            "existing Blob + ArrayBuffer integration tests live there; missing discriminator is resizable ArrayBuffer"
        )
    );
    Ok(())
}

#[test]
fn bun_bridge_hint_names_missing_shared_discriminator() -> Result<(), String> {
    let source = r#"
test("blob copies resizable buffers", async () => {
  const growable = new ArrayBuffer(4, { maxByteLength: 8 });
  const view = new Uint8Array(growable);
  const blob = new Blob([view]);
  const copied = new Uint8Array(await blob.arrayBuffer());
  expect([...copied]).toEqual([0, 0, 0, 0]);
});
"#;

    let hint = bun_bridge_hint_for_source(source)?;

    assert_eq!(hint.verdict, TypeScriptBunBridgeVerdict::TsMissingShared);
    assert_eq!(
        hint.verdict.missing_discriminators(),
        &["shared_array_buffer"]
    );
    assert_eq!(
        hint.verdict.expected_action(),
        "route_cross_language_oracle_visibility_limitation"
    );
    assert_eq!(
        hint.suggested_test_file(),
        BUN_BLOB_ARRAY_BUFFER_TS_TEST_FILE
    );
    assert_eq!(
        hint.placement_reason().as_deref(),
        Some(
            "existing Blob + ArrayBuffer integration tests live there; missing discriminator is SharedArrayBuffer"
        )
    );
    Ok(())
}

#[test]
fn bun_bridge_hint_names_both_missing_boundary_discriminators() -> Result<(), String> {
    let source = r#"
test("blob copies scalar view buffers", async () => {
  const view = new Uint8Array(4);
  const blob = new Blob([view]);
  const copied = new Uint8Array(await blob.arrayBuffer());
  expect([...copied]).toEqual([0, 0, 0, 0]);
});
"#;

    let hint = bun_bridge_hint_for_source(source)?;

    assert_eq!(
        hint.verdict,
        TypeScriptBunBridgeVerdict::TsMissingSharedAndResizable
    );
    assert_eq!(
        hint.verdict.missing_discriminators(),
        &["shared_array_buffer", "resizable_array_buffer"]
    );
    assert_eq!(
        hint.verdict.expected_action(),
        "route_cross_language_oracle_visibility_limitation"
    );
    assert_eq!(
        hint.suggested_test_file(),
        BUN_BLOB_ARRAY_BUFFER_TS_TEST_FILE
    );
    assert_eq!(
        hint.placement_reason().as_deref(),
        Some(
            "existing Blob + ArrayBuffer integration tests live there; missing discriminators are SharedArrayBuffer and resizable ArrayBuffer"
        )
    );
    Ok(())
}

#[test]
fn bun_bridge_hint_does_not_credit_max_byte_length_mention_without_blob_observer()
-> Result<(), String> {
    let source = r#"
test("records growable allocation shape", () => {
  const growable = new ArrayBuffer(4, { maxByteLength: 8 });
  expect(growable.byteLength).toBe(4);
});
"#;

    let hint = bun_bridge_hint_for_source(source)?;

    assert_eq!(
        hint.verdict,
        TypeScriptBunBridgeVerdict::TsMentionNotObserver
    );
    assert_eq!(
        hint.verdict.expected_action(),
        "do_not_credit_token_mention"
    );
    assert_eq!(hint.suggested_test_file(), "not_applicable");
    Ok(())
}

#[test]
fn bun_bridge_hint_routes_partial_blob_observer_as_missing_external_oracle() -> Result<(), String> {
    let source = r#"
test("blob records shared and growable inputs without byte oracle", () => {
  const shared = new SharedArrayBuffer(4);
  const growable = new ArrayBuffer(4, { maxByteLength: 8 });
  const blob = new Blob([new Uint8Array(shared), new Uint8Array(growable)]);
  expect(blob.size).toBe(8);
});
"#;

    let hint = bun_bridge_hint_for_source(source)?;

    assert_eq!(
        hint.verdict,
        TypeScriptBunBridgeVerdict::TsMissingExternalOracle
    );
    assert_eq!(hint.verdict.missing_discriminators(), &[] as &[&str]);
    assert_eq!(
        hint.verdict.cross_language_state(),
        "rust_ungripped_ts_missing_external_oracle"
    );
    assert_eq!(
        hint.verdict.expected_action(),
        "route_cross_language_oracle_visibility_limitation"
    );
    Ok(())
}

#[test]
fn bun_bridge_hint_can_report_unknown_bridge_confidence() -> Result<(), String> {
    let source = r#"
test("blob copies shared and resizable buffers", async () => {
  const shared = new SharedArrayBuffer(4);
  const growable = new ArrayBuffer(4, { maxByteLength: 8 });
  const view = new Uint8Array(growable);
  const blob = new Blob([view, new Uint8Array(shared)]);
  const copied = new Uint8Array(await blob.arrayBuffer());
  expect([...copied]).toEqual([0, 0, 0, 0]);
});
"#;

    let hint =
        bun_bridge_hint_for_source_with_confidence(source, TypeScriptBunBridgeConfidence::Unknown)
            .ok_or_else(|| {
                "complete TS discriminators should produce bridge_unknown with an unknown profile"
                    .to_string()
            })?;

    assert_eq!(hint.confidence, TypeScriptBunBridgeConfidence::Unknown);
    assert_eq!(hint.verdict, TypeScriptBunBridgeVerdict::BridgeUnknown);
    assert_eq!(
        hint.verdict.expected_action(),
        "report_bridge_unknown_not_no_static_path"
    );
    Ok(())
}

#[test]
fn classify_change_projects_trusted_related_bun_array_buffer_facts_as_advisory_evidence()
-> Result<(), String> {
    let owner = test_owner("hydrateBlob", "src/blob.ts");
    let tests = extract_tests(
        Path::new("test/js/web/fetch/blob.test.ts"),
        r#"
test("Blob copies ArrayBuffer-backed bytes", async () => {
  const shared = new SharedArrayBuffer(4);
  const fixed = new ArrayBuffer(4);
  const view = new Uint8Array(fixed);
  const blob = new Blob([view, new Uint8Array(shared)]);
  hydrateBlob(blob);
  const copied = new Uint8Array(await blob.arrayBuffer());
  expect([...copied]).toEqual([0, 0, 0, 0]);
});
"#,
    );
    assert_eq!(tests.len(), 1);
    let finding = classify_change(
        Path::new("src/blob.ts"),
        2,
        "  return blob;",
        &[owner],
        &tests,
        None,
        &ReExportIndex::empty(),
        None,
    )
    .ok_or_else(|| "expected TypeScript preview finding".to_string())?;

    // RIPR-SPEC-0098: the assertion observes `[...copied]`, not the direct
    // return value of hydrateBlob, so observation_confirmed fails.
    // The finding is correctly downgraded to WeaklyExposed.
    assert!(
        matches!(
            finding.class,
            ExposureClass::Exposed | ExposureClass::WeaklyExposed
        ),
        "expected Exposed or WeaklyExposed for bun advisory test, got {:?}",
        finding.class
    );
    assert_evidence_contains(
        &finding,
        "typescript_bun_ub_advisory_fact: shared_array_buffer",
    );
    assert_evidence_contains(
        &finding,
        "typescript_bun_ub_advisory_fact: view_backed_blob_input",
    );
    assert_evidence_contains(
        &finding,
        "typescript_bun_ub_advisory_fact: stable_byte_copy_oracle",
    );
    assert_evidence_contains(
        &finding,
        "typescript_bun_ub_bridge_hint: confidence=configured_hint",
    );
    assert_evidence_contains(&finding, "rust_owner=Blob::from_js_without_defer_gc");
    assert_evidence_contains(&finding, "rust_owner=copy_to_unshared");
    assert_evidence_contains(
        &finding,
        "typescript_bun_ub_bridge_verdict: ts_missing_resizable missing_discriminators=resizable_array_buffer action=route_cross_language_oracle_visibility_limitation",
    );
    assert_evidence_contains(
        &finding,
        "typescript_bun_ub_bridge_boundary: preview_advisory_only",
    );
    assert!(
        finding
            .evidence
            .iter()
            .all(|entry| !entry.contains("max_byte_length_mention_only")),
        "maxByteLength mention-only must not be emitted for a Blob stable-byte observer: {:?}",
        finding.evidence
    );
    let placement_evidence: Vec<_> = finding
        .evidence
        .iter()
        .filter(|entry| entry.starts_with("typescript_bun_ub_test_placement:"))
        .collect();
    assert_eq!(
        placement_evidence.len(),
        1,
        "expected Blob-only placement evidence"
    );
    assert!(
        placement_evidence[0].contains("missing discriminator is resizable ArrayBuffer"),
        "expected the sole placement record to belong to the Blob profile: {placement_evidence:?}"
    );
    Ok(())
}

#[test]
fn changed_rust_blob_boundary_projects_ts_discriminated_cross_language_grip() -> Result<(), String>
{
    let source = r#"
test("blob copies shared and resizable buffers", async () => {
  const shared = new SharedArrayBuffer(4);
  const growable = new ArrayBuffer(4, { maxByteLength: 8 });
  const blob = new Blob([new Uint8Array(shared), new Uint8Array(growable)]);
  const copied = new Uint8Array(await blob.arrayBuffer());
  expect([...copied]).toEqual([0, 0, 0, 0]);
});
"#;
    let finding = bun_cross_language_finding_for_source(source)?;

    assert!(matches!(finding.class, ExposureClass::Exposed));
    assert_eq!(finding.language, Some(DomainLanguageId::TypeScript));
    assert_eq!(finding.language_status, Some(LanguageStatus::Preview));
    assert_eq!(
        finding.probe.location.file,
        PathBuf::from("src/jsc/Blob.rs")
    );
    assert_eq!(
        finding.related_tests[0].file,
        PathBuf::from(BUN_BLOB_ARRAY_BUFFER_TS_TEST_FILE)
    );
    assert!(finding.activation.missing_discriminators.is_empty());
    assert_evidence_contains(
        &finding,
        "typescript_bun_ub_cross_language_grip: state=rust_ungripped_ts_discriminated",
    );
    assert_evidence_contains(
        &finding,
        "typescript_bun_ub_bridge_verdict: ts_discriminated missing_discriminators=none action=no_missing_bridge_discriminator",
    );
    assert_evidence_contains(&finding, "raw_evidence_ref: leg=rust_seam;");
    assert_evidence_contains(&finding, "raw_evidence_ref: leg=binding_edge;");
    assert_evidence_contains(&finding, "raw_evidence_ref: leg=boundary_discriminator;");
    assert_evidence_contains(&finding, "raw_evidence_ref: leg=external_callsite;");
    assert_evidence_contains(&finding, "raw_evidence_ref: leg=external_oracle;");
    assert!(
        finding
            .evidence
            .iter()
            .all(|entry| !entry.starts_with("missing_graph_legs:")),
        "complete TS witness must not report missing graph legs: {:?}",
        finding.evidence
    );
    assert!(
        finding
            .recommended_next_step
            .as_deref()
            .is_some_and(|step| step.contains("no new test suggested"))
    );
    Ok(())
}

#[test]
fn changed_rust_markdown_boundary_projects_ts_discriminated_cross_language_grip()
-> Result<(), String> {
    let source = r#"
test("markdown accepts a resizable ArrayBuffer", () => {
  const growable = new ArrayBuffer(16, { maxByteLength: 32 });
  const html = Bun.markdown(growable);
  expect(html).toBe("<p>hello</p>\n");
});
"#;
    let hint = bun_markdown_bridge_hint_for_source(source)?;
    assert_eq!(
        hint.profile_kind,
        TypeScriptBunBridgeProfileKind::MarkdownResizableArrayBuffer
    );
    assert_eq!(hint.verdict, TypeScriptBunBridgeVerdict::TsDiscriminated);
    assert_eq!(hint.suggested_test_file(), "not_applicable");

    let finding = bun_markdown_cross_language_finding_for_source(source)?;

    assert!(matches!(finding.class, ExposureClass::Exposed));
    assert_eq!(finding.language, Some(DomainLanguageId::TypeScript));
    assert_eq!(finding.language_status, Some(LanguageStatus::Preview));
    assert_eq!(
        finding.probe.location.file,
        PathBuf::from(BUN_MARKDOWN_RESIZABLE_RUST_FILE)
    );
    assert_eq!(
        finding.related_tests[0].file,
        PathBuf::from(BUN_MARKDOWN_RESIZABLE_TS_TEST_FILE)
    );
    assert!(finding.activation.missing_discriminators.is_empty());
    assert_evidence_contains(
        &finding,
        "typescript_bun_ub_advisory_fact: resizable_array_buffer",
    );
    assert_evidence_contains(
        &finding,
        "typescript_bun_ub_advisory_fact: bun_markdown_callsite",
    );
    assert_evidence_contains(
        &finding,
        "typescript_bun_ub_advisory_fact: markdown_strong_oracle",
    );
    assert_evidence_contains(
        &finding,
        "typescript_bun_ub_cross_language_grip: state=rust_ungripped_ts_discriminated",
    );
    assert_evidence_contains(
        &finding,
        "typescript_bun_ub_bridge_verdict: ts_discriminated missing_discriminators=none action=no_missing_bridge_discriminator suggested_test_file=not_applicable repair_packet_ready=false",
    );
    assert_evidence_contains(&finding, "gap_state: already_observed");
    assert_evidence_contains(&finding, "raw_evidence_ref: leg=rust_seam;");
    assert_evidence_contains(&finding, "raw_evidence_ref: leg=binding_edge;");
    assert_evidence_contains(&finding, "raw_evidence_ref: leg=boundary_discriminator;");
    assert_evidence_contains(&finding, "raw_evidence_ref: leg=external_callsite;");
    assert_evidence_contains(&finding, "raw_evidence_ref: leg=external_oracle;");
    assert!(
        finding
            .evidence
            .iter()
            .all(|entry| !entry.starts_with("missing_graph_legs:")),
        "complete Markdown TS witness must not report missing graph legs: {:?}",
        finding.evidence
    );
    assert!(
        finding
            .recommended_next_step
            .as_deref()
            .is_some_and(|step| step.contains("no new test suggested"))
    );
    Ok(())
}

#[test]
fn changed_rust_markdown_cross_language_without_strong_oracle_stays_limitation()
-> Result<(), String> {
    let source = r#"
test("markdown smoke covers a resizable ArrayBuffer", () => {
  const growable = new ArrayBuffer(16, { maxByteLength: 32 });
  const html = Bun.markdown(growable);
  expect(html).toBeDefined();
});
"#;
    let finding = bun_markdown_cross_language_finding_for_source(source)?;

    assert!(matches!(finding.class, ExposureClass::StaticUnknown));
    assert_eq!(finding.stop_reasons, vec![StopReason::StaticProbeUnknown]);
    assert_evidence_contains(&finding, "gap_state: static_limitation");
    assert_evidence_contains(
        &finding,
        "actionability_category: cross_language_oracle_visibility_unresolved",
    );
    assert_evidence_contains(
        &finding,
        "repair_route: analysis/cross-language-oracle-visibility",
    );
    assert_evidence_contains(
        &finding,
        "missing_graph_legs: external_oracle:markdown_strong_oracle",
    );
    assert_evidence_contains(
        &finding,
        "unlock_condition: Connect the partial Bun markdown evidence to a strong markdown output oracle",
    );
    assert_evidence_contains(
        &finding,
        "typescript_bun_ub_bridge_verdict: ts_missing_external_oracle missing_discriminators=none action=route_cross_language_oracle_visibility_limitation suggested_test_file=not_applicable repair_packet_ready=false",
    );
    assert!(
        finding
            .recommended_next_step
            .as_deref()
            .is_some_and(|step| step.contains("before suggesting a test target"))
    );
    Ok(())
}

#[test]
fn changed_rust_markdown_cross_language_without_resizable_stays_targetless() -> Result<(), String> {
    let source = r##"
test("markdown string input has a strong oracle", () => {
  const html = Bun.markdown("# hello");
  expect(html).toBe("<h1>hello</h1>\n");
});
"##;
    let finding = bun_markdown_cross_language_finding_for_source(source)?;

    assert!(matches!(finding.class, ExposureClass::StaticUnknown));
    assert_eq!(finding.stop_reasons, vec![StopReason::StaticProbeUnknown]);
    assert_eq!(
        missing_discriminator_values(&finding),
        vec!["resizable_array_buffer"]
    );
    assert_evidence_contains(&finding, "gap_state: static_limitation");
    assert_evidence_contains(
        &finding,
        "missing_graph_legs: boundary_discriminator:resizable_array_buffer",
    );
    assert_evidence_contains(
        &finding,
        "typescript_bun_ub_bridge_verdict: ts_missing_resizable missing_discriminators=resizable_array_buffer action=route_cross_language_oracle_visibility_limitation suggested_test_file=not_applicable repair_packet_ready=false",
    );
    assert!(
        finding
            .evidence
            .iter()
            .all(|entry| !entry.starts_with("typescript_bun_ub_test_placement:")),
        "MarkdownObject missing-discriminator limitation must not infer a TypeScript placement: {:?}",
        finding.evidence
    );
    Ok(())
}

#[test]
fn changed_rust_blob_boundary_projects_missing_resizable_cross_language_grip() -> Result<(), String>
{
    let source = r#"
test("blob copies shared buffers", async () => {
  const shared = new SharedArrayBuffer(4);
  const blob = new Blob([new Uint8Array(shared)]);
  const copied = new Uint8Array(await blob.arrayBuffer());
  expect([...copied]).toEqual([0, 0, 0, 0]);
});
"#;
    let finding = bun_cross_language_finding_for_source(source)?;

    assert!(matches!(finding.class, ExposureClass::StaticUnknown));
    assert_eq!(finding.stop_reasons, vec![StopReason::StaticProbeUnknown]);
    assert_eq!(
        missing_discriminator_values(&finding),
        vec!["resizable_array_buffer"]
    );
    assert_evidence_contains(&finding, "gap_state: static_limitation");
    assert_evidence_contains(
        &finding,
        "actionability_category: cross_language_oracle_visibility_unresolved",
    );
    assert_evidence_contains(
        &finding,
        "repair_route: analysis/cross-language-oracle-visibility",
    );
    assert_evidence_contains(
        &finding,
        "missing_graph_legs: boundary_discriminator:resizable_array_buffer",
    );
    assert_evidence_contains(
        &finding,
        "unlock_condition: add or inspect the missing external TypeScript discriminator(s) in test/js/web/fetch/blob.test.ts",
    );
    assert_evidence_contains(&finding, "raw_evidence_ref: leg=rust_seam;");
    assert_evidence_contains(&finding, "raw_evidence_ref: leg=binding_edge;");
    assert_evidence_contains(&finding, "raw_evidence_ref: leg=boundary_discriminator;");
    assert_evidence_contains(&finding, "raw_evidence_ref: leg=external_callsite;");
    assert_evidence_contains(&finding, "raw_evidence_ref: leg=external_oracle;");
    assert_evidence_contains(
        &finding,
        "typescript_bun_ub_cross_language_grip: state=rust_ungripped_ts_missing_discriminator",
    );
    assert_evidence_contains(
        &finding,
        "typescript_bun_ub_bridge_verdict: ts_missing_resizable missing_discriminators=resizable_array_buffer action=route_cross_language_oracle_visibility_limitation suggested_test_file=test/js/web/fetch/blob.test.ts repair_packet_ready=false",
    );
    assert_evidence_contains(
        &finding,
        "typescript_bun_ub_test_placement: rank=1 suggested_test_file=test/js/web/fetch/blob.test.ts reason=\"existing Blob + ArrayBuffer integration tests live there; missing discriminator is resizable ArrayBuffer\"",
    );
    assert!(
        finding
            .recommended_next_step
            .as_deref()
            .is_some_and(|step| step.contains(
                "suggest the configured TypeScript observer file only as advisory placement"
            ))
    );
    Ok(())
}

#[test]
fn changed_rust_blob_boundary_with_unknown_bridge_stays_limitation() -> Result<(), String> {
    let source = r#"
test("blob copies shared and resizable buffers", async () => {
  const shared = new SharedArrayBuffer(4);
  const growable = new ArrayBuffer(4, { maxByteLength: 8 });
  const blob = new Blob([new Uint8Array(shared), new Uint8Array(growable)]);
  const copied = new Uint8Array(await blob.arrayBuffer());
  expect([...copied]).toEqual([0, 0, 0, 0]);
});
"#;
    let finding = bun_cross_language_finding_for_source_with_confidence(
        source,
        TypeScriptBunBridgeConfidence::Unknown,
    )?;

    assert!(matches!(finding.class, ExposureClass::StaticUnknown));
    assert_eq!(finding.stop_reasons, vec![StopReason::StaticProbeUnknown]);
    assert!(finding.activation.missing_discriminators.is_empty());
    assert_evidence_contains(&finding, "gap_state: static_limitation");
    assert_evidence_contains(
        &finding,
        "actionability_category: cross_language_oracle_visibility_unresolved",
    );
    assert_evidence_contains(
        &finding,
        "typescript_bun_ub_bridge_hint: confidence=unknown",
    );
    assert_evidence_contains(
        &finding,
        "typescript_bun_ub_cross_language_grip: state=bridge_unknown",
    );
    assert_evidence_contains(
        &finding,
        "typescript_bun_ub_bridge_verdict: bridge_unknown missing_discriminators=none action=report_bridge_unknown_not_no_static_path suggested_test_file=not_applicable repair_packet_ready=false",
    );
    assert_evidence_contains(&finding, "missing_graph_legs: binding_or_ffi_edge");
    assert_evidence_contains(
        &finding,
        "unlock_condition: name the binding or FFI edge from the Rust seam to the external test",
    );
    assert_evidence_contains(&finding, "raw_evidence_ref: leg=rust_seam;");
    assert_evidence_contains(&finding, "raw_evidence_ref: leg=boundary_discriminator;");
    assert_evidence_contains(&finding, "raw_evidence_ref: leg=external_callsite;");
    assert_evidence_contains(&finding, "raw_evidence_ref: leg=external_oracle;");
    assert_evidence_lacks(&finding, "raw_evidence_ref: leg=binding_edge;");
    assert!(
        finding
            .recommended_next_step
            .as_deref()
            .is_some_and(|step| step.contains("analysis/cross-language-oracle-visibility"))
    );
    Ok(())
}

#[test]
fn changed_rust_copy_to_unshared_projects_configured_bridge_evidence() -> Result<(), String> {
    let source = r#"
test("blob copies shared and resizable buffers through copy path", async () => {
  const shared = new SharedArrayBuffer(4);
  const growable = new ArrayBuffer(4, { maxByteLength: 8 });
  const blob = new Blob([new Uint8Array(shared), new Uint8Array(growable)]);
  const copied = new Uint8Array(await blob.arrayBuffer());
  expect([...copied]).toEqual([0, 0, 0, 0]);
});
"#;
    let finding = bun_cross_language_finding_for_source_with_profile_and_confidence(
        source,
        BUN_ARRAY_BUFFER_COPY_TO_UNSHARED_BRIDGE_PROFILE,
        TypeScriptBunBridgeConfidence::ConfiguredHint,
        341,
        "pub fn copy_to_unshared(buffer: JSValue) -> JSValue {",
    )?;

    assert!(matches!(finding.class, ExposureClass::Exposed));
    assert_eq!(
        finding.probe.location.file,
        PathBuf::from(BUN_ARRAY_BUFFER_COPY_TO_UNSHARED_RUST_FILE)
    );
    assert_evidence_contains(&finding, "rust_owner=copy_to_unshared");
    assert_evidence_contains(
        &finding,
        "rust_file=src/jsc/array_buffer.rs rust_owner=copy_to_unshared",
    );
    assert_evidence_contains(
        &finding,
        "typescript_bun_ub_cross_language_grip: state=rust_ungripped_ts_discriminated",
    );
    assert_evidence_contains(
        &finding,
        "typescript_bun_ub_bridge_verdict: ts_discriminated missing_discriminators=none action=no_missing_bridge_discriminator",
    );
    assert_evidence_contains(
        &finding,
        "raw_evidence_ref: leg=binding_edge;file=src/jsc/array_buffer.rs;line=341;kind=configured_bridge;",
    );
    assert_evidence_contains(&finding, "raw_evidence_ref: leg=external_callsite;");
    assert_evidence_contains(&finding, "raw_evidence_ref: leg=external_oracle;");
    assert!(
        finding
            .evidence
            .iter()
            .all(|entry| !entry.starts_with("missing_graph_legs:")),
        "configured copy_to_unshared bridge must not report missing graph legs: {:?}",
        finding.evidence
    );
    assert!(
        finding
            .recommended_next_step
            .as_deref()
            .is_some_and(|step| step.contains("no new test suggested"))
    );
    Ok(())
}

#[test]
fn related_copy_to_unshared_test_emits_configured_bridge_hint() -> Result<(), String> {
    let source = r#"
test("blob copies shared and resizable buffers through copy path", async () => {
  const shared = new SharedArrayBuffer(4);
  const growable = new ArrayBuffer(4, { maxByteLength: 8 });
  const blob = new Blob([new Uint8Array(shared), new Uint8Array(growable)]);
  const copied = new Uint8Array(await blob.arrayBuffer());
  expect([...copied]).toEqual([0, 0, 0, 0]);
});
"#;
    let tests = extract_tests(Path::new(BUN_BLOB_ARRAY_BUFFER_TS_TEST_FILE), source);
    let facts = tests
        .iter()
        .flat_map(bun_array_buffer_facts_for_test)
        .collect::<Vec<_>>();

    let hints = collect_related_bun_bridge_hints(&facts);
    assert!(hints.iter().any(|hint| {
        hint.profile_kind == TypeScriptBunBridgeProfileKind::ArrayBufferCopyToUnshared
            && hint.rust_owner == BUN_ARRAY_BUFFER_COPY_TO_UNSHARED_RUST_OWNER
            && hint.ts_test_file == Path::new(BUN_BLOB_ARRAY_BUFFER_TS_TEST_FILE)
    }));
    Ok(())
}

#[test]
fn changed_rust_copy_to_unshared_unknown_bridge_stays_limitation() -> Result<(), String> {
    let source = r#"
test("blob copies shared and resizable buffers through copy path", async () => {
  const shared = new SharedArrayBuffer(4);
  const growable = new ArrayBuffer(4, { maxByteLength: 8 });
  const blob = new Blob([new Uint8Array(shared), new Uint8Array(growable)]);
  const copied = new Uint8Array(await blob.arrayBuffer());
  expect([...copied]).toEqual([0, 0, 0, 0]);
});
"#;
    let finding = bun_cross_language_finding_for_source_with_profile_and_confidence(
        source,
        BUN_ARRAY_BUFFER_COPY_TO_UNSHARED_BRIDGE_PROFILE,
        TypeScriptBunBridgeConfidence::Unknown,
        341,
        "pub fn copy_to_unshared(buffer: JSValue) -> JSValue {",
    )?;

    assert!(matches!(finding.class, ExposureClass::StaticUnknown));
    assert_evidence_contains(
        &finding,
        "typescript_bun_ub_bridge_hint: confidence=unknown rust_file=src/jsc/array_buffer.rs rust_owner=copy_to_unshared",
    );
    assert_evidence_contains(
        &finding,
        "typescript_bun_ub_cross_language_grip: state=bridge_unknown",
    );
    assert_evidence_contains(&finding, "missing_graph_legs: binding_or_ffi_edge");
    assert_evidence_lacks(&finding, "raw_evidence_ref: leg=binding_edge;");
    assert_evidence_contains(&finding, "suggested_test_file=not_applicable");
    Ok(())
}

#[test]
fn changed_rust_blob_boundary_keeps_max_byte_length_mention_out_of_grip() -> Result<(), String> {
    let source = r#"
test("mentions growable buffers without Blob observer", () => {
  const growable = new ArrayBuffer(4, { maxByteLength: 8 });
  expect(growable.byteLength).toBe(4);
});
"#;
    let finding = bun_cross_language_finding_for_source(source)?;

    assert!(matches!(finding.class, ExposureClass::StaticUnknown));
    assert_eq!(finding.stop_reasons, vec![StopReason::StaticProbeUnknown]);
    assert!(finding.activation.missing_discriminators.is_empty());
    assert_evidence_contains(
        &finding,
        "typescript_bun_ub_cross_language_grip: state=ts_mention_not_observer",
    );
    assert_evidence_contains(
        &finding,
        "typescript_bun_ub_bridge_verdict: ts_mention_not_observer missing_discriminators=none action=do_not_credit_token_mention",
    );
    assert_evidence_contains(
        &finding,
        "missing_graph_legs: external_callsite:view_backed_blob_input, external_oracle:stable_byte_copy",
    );
    assert_evidence_contains(
        &finding,
        "unlock_condition: connect a Blob-backed external callsite and stable-byte oracle",
    );
    Ok(())
}

#[test]
fn changed_rust_blob_boundary_projects_partial_blob_observer_as_limitation() -> Result<(), String> {
    let source = r#"
test("blob records shared and growable inputs without byte oracle", () => {
  const shared = new SharedArrayBuffer(4);
  const growable = new ArrayBuffer(4, { maxByteLength: 8 });
  const blob = new Blob([new Uint8Array(shared), new Uint8Array(growable)]);
  expect(blob.size).toBe(8);
});
"#;
    let finding = bun_cross_language_finding_for_source(source)?;

    assert!(matches!(finding.class, ExposureClass::StaticUnknown));
    assert_eq!(finding.stop_reasons, vec![StopReason::StaticProbeUnknown]);
    assert!(finding.activation.missing_discriminators.is_empty());
    assert_evidence_contains(&finding, "gap_state: static_limitation");
    assert_evidence_contains(
        &finding,
        "actionability_category: cross_language_oracle_visibility_unresolved",
    );
    assert_evidence_contains(
        &finding,
        "repair_route: analysis/cross-language-oracle-visibility",
    );
    assert_evidence_contains(
        &finding,
        "typescript_bun_ub_cross_language_grip: state=rust_ungripped_ts_missing_external_oracle",
    );
    assert_evidence_contains(
        &finding,
        "typescript_bun_ub_bridge_verdict: ts_missing_external_oracle missing_discriminators=none action=route_cross_language_oracle_visibility_limitation suggested_test_file=not_applicable repair_packet_ready=false",
    );
    assert_evidence_contains(
        &finding,
        "missing_graph_legs: external_oracle:stable_byte_copy",
    );
    assert_evidence_contains(
        &finding,
        "unlock_condition: Connect the partial Blob observer evidence to a stable byte oracle",
    );
    assert_evidence_contains(&finding, "raw_evidence_ref: leg=rust_seam;");
    assert_evidence_contains(&finding, "raw_evidence_ref: leg=binding_edge;");
    assert_evidence_contains(&finding, "raw_evidence_ref: leg=boundary_discriminator;");
    assert_evidence_contains(&finding, "raw_evidence_ref: leg=external_callsite;");
    assert_evidence_lacks(&finding, "raw_evidence_ref: leg=external_oracle;");
    assert!(
        !finding
            .recommended_next_step
            .as_deref()
            .unwrap_or_default()
            .contains("no new test suggested")
    );
    assert!(
        finding
            .recommended_next_step
            .as_deref()
            .is_some_and(|step| step.contains("analysis/cross-language-oracle-visibility"))
    );
    Ok(())
}

#[test]
fn accepts_ts_jsx_paths() {
    let adapter = TypeScriptAdapter;
    assert!(adapter.accepts_path(Path::new("src/index.ts")));
    assert!(adapter.accepts_path(Path::new("src/component.tsx")));
    assert!(adapter.accepts_path(Path::new("src/index.js")));
    assert!(adapter.accepts_path(Path::new("src/component.jsx")));
    // Modern ESM/CJS extensions route to this adapter as well.
    assert!(adapter.accepts_path(Path::new("src/module.mts")));
    assert!(adapter.accepts_path(Path::new("src/module.cts")));
    assert!(adapter.accepts_path(Path::new("src/module.mjs")));
    assert!(adapter.accepts_path(Path::new("src/module.cjs")));
    // `.d.ts` declaration files keep routing here via "ts" and stay accepted.
    assert!(adapter.accepts_path(Path::new("src/types.d.ts")));
    assert!(!adapter.accepts_path(Path::new("src/lib.rs")));
    assert!(!adapter.accepts_path(Path::new("scripts/run.py")));
    assert!(!adapter.accepts_path(Path::new("README.md")));
}

#[test]
fn esm_cjs_extensions_parse_with_module_correct_source_type() {
    // `.mts` is TypeScript ESM: type annotations must parse, `import` must
    // be legal module syntax. `.cts` is TypeScript CJS: `require` must parse.
    assert!(
        parse_error_reason(
            Path::new("src/cart.mts"),
            "export function f(x: number): number { return x; }\n"
        )
        .is_none(),
        ".mts source with type annotations must parse"
    );
    assert!(
        parse_error_reason(
            Path::new("src/tool.cts"),
            "const path = require('node:path');\n"
        )
        .is_none(),
        ".cts source with require must parse"
    );
    assert!(
        parse_error_reason(Path::new("src/tool.mjs"), "export const value = 1;\n").is_none(),
        ".mjs source must parse"
    );
    assert!(
        parse_error_reason(Path::new("src/tool.cjs"), "module.exports = 1;\n").is_none(),
        ".cjs source with module.exports must parse"
    );
    // And each routed extension still surfaces parser errors instead of
    // silently dropping the file.
    assert!(
        parse_error_reason(
            Path::new("src/cart.mts"),
            "this is not :: valid +++ typescript"
        )
        .is_some(),
        ".mts parse errors must be reported, not swallowed"
    );
}

#[test]
fn extract_owners_returns_empty_when_source_does_not_parse() {
    let owners = extract_owners(
        Path::new("src/index.ts"),
        "this is not :: valid +++ typescript",
    );
    assert!(owners.is_empty());
}

#[test]
fn parse_error_reason_reports_parser_errors() {
    let reason = parse_error_reason(
        Path::new("src/index.ts"),
        "this is not :: valid +++ typescript",
    );
    assert!(reason.is_some());
    let reason = reason.unwrap_or_default();
    assert!(reason.contains("parser error"));
}

#[test]
fn parse_error_reason_includes_first_parser_message() {
    // The reason must carry the first oxc message, not just a bare count,
    // so the limitation is actionable.
    let reason = parse_error_reason(Path::new("src/index.ts"), "const x = ;").unwrap_or_default();
    assert!(
        reason.starts_with("1 parser error(s): "),
        "reason must include the first parser message: {reason}"
    );
    let message = reason.trim_start_matches("1 parser error(s): ");
    assert!(!message.is_empty(), "parser message must be non-empty");
}

#[test]
fn unsupported_syntax_finding_is_preview_static_unknown() {
    let limit = TypeScriptParseLimit {
        file: PathBuf::from("src/index.ts"),
        reason: "1 parser error(s)".to_string(),
    };
    let finding =
        unsupported_syntax_finding(Path::new("src/index.ts"), 3, "  const value = ;", &limit);

    assert!(matches!(finding.class, ExposureClass::StaticUnknown));
    assert_eq!(
        finding.static_limit_kind,
        Some(StaticLimitKind::UnsupportedSyntax)
    );
    assert_eq!(finding.language, Some(DomainLanguageId::TypeScript));
    assert_eq!(finding.language_status, Some(LanguageStatus::Preview));
    assert_eq!(finding.stop_reasons, vec![StopReason::StaticProbeUnknown]);
    assert_evidence_contains(
        &finding,
        "evidence_needed_to_promote: resolve the named static limit and re-run TypeScript preview evidence extraction",
    );
}

#[test]
fn is_test_file_matches_test_and_spec_suffixes() {
    assert!(is_test_file(Path::new("tests/lib.test.ts")));
    assert!(is_test_file(Path::new("src/Header.spec.tsx")));
    assert!(is_test_file(Path::new("legacy.test.js")));
    // Modern ESM/CJS extensions follow the same suffix conventions.
    assert!(is_test_file(Path::new("tests/lib.test.mts")));
    assert!(is_test_file(Path::new("tests/lib.spec.mjs")));
    assert!(is_test_file(Path::new("tests/lib.test.cjs")));
    assert!(!is_test_file(Path::new("src/lib.ts")));
    assert!(!is_test_file(Path::new("src/lib.mts")));
    assert!(!is_test_file(Path::new("README.md")));
}

#[test]
fn is_test_file_matches_test_directory_convention() {
    // AVA / Mocha / node:test: feature-named source under test/ or tests/.
    assert!(is_test_file(Path::new("test/body-size.ts")));
    assert!(is_test_file(Path::new("tests/utils.ts")));
    assert!(is_test_file(Path::new("src/__tests__/Header.tsx")));
    assert!(is_test_file(Path::new("packages/core/test/index.jsx")));
    // Component match, not substring — these are NOT tests.
    assert!(!is_test_file(Path::new("src/latest/feature.ts")));
    assert!(!is_test_file(Path::new("test-utils/helper.ts")));
    assert!(!is_test_file(Path::new("src/contest.ts")));
    // Non-TS/JS files under test/ are not source test files.
    assert!(!is_test_file(Path::new("test/fixtures/data.json")));
}

#[test]
fn line_for_offset_counts_newlines() {
    let source = "line1\nline2\nline3\n";
    assert_eq!(line_for_offset(source, 0), 1);
    assert_eq!(line_for_offset(source, 5), 1);
    assert_eq!(line_for_offset(source, 6), 2);
    assert_eq!(line_for_offset(source, 12), 3);
}

#[test]
fn normalized_path_strips_dot_prefix_and_normalizes_separators() {
    assert_eq!(normalized_path(Path::new(r".\src\b.ts")), "src/b.ts");
}

#[test]
fn extract_owners_recognizes_function_declaration() {
    let owners = extract_owners(
        Path::new("src/lib.ts"),
        "function applyDiscount(amount: number): number {\n    return amount;\n}\n",
    );
    assert_eq!(owners.len(), 1);
    assert_eq!(owners[0].name, "applyDiscount");
    assert_eq!(owners[0].start_line, 1);
    assert_eq!(owners[0].owner_kind, OwnerKind::Function);
}

#[test]
fn extract_owners_recognizes_exported_function() {
    let owners = extract_owners(
        Path::new("src/lib.ts"),
        "export function publicHelper(): void {}\n",
    );
    assert_eq!(owners.len(), 1);
    assert_eq!(owners[0].name, "publicHelper");
    assert_eq!(owners[0].owner_kind, OwnerKind::Function);
}

#[test]
fn extract_owners_recognizes_arrow_const_and_module_initializer() {
    let owners = extract_owners(
        Path::new("src/lib.ts"),
        r#"const formatPrice = (amount: number) => {
    return amount.toFixed(2);
};
const defaultRate = 0.08;
"#,
    );
    assert_eq!(owners.len(), 2);
    assert_eq!(owners[0].name, "formatPrice");
    assert_eq!(owners[0].owner_kind, OwnerKind::ArrowFunction);
    assert_eq!(owners[0].start_line, 1);
    assert_eq!(owners[0].end_line, 3);
    assert_eq!(owners[1].name, "defaultRate");
    assert_eq!(owners[1].owner_kind, OwnerKind::ModuleFunction);
    assert_eq!(owners[1].start_line, 4);
}

#[test]
fn extract_owners_recognizes_class_methods() {
    let owners = extract_owners(
        Path::new("src/cart.ts"),
        r#"class Cart {
    total() {
        return 1;
    }
    static build() {
        return new Cart();
    }
}
"#,
    );
    assert_eq!(owners.len(), 2);
    assert_eq!(owners[0].name, "total");
    assert_eq!(owners[0].owner_kind, OwnerKind::Method);
    assert_eq!(owners[0].start_line, 2);
    assert_eq!(owners[1].name, "build");
    assert_eq!(owners[1].owner_kind, OwnerKind::ClassMethod);
    assert_eq!(owners[1].start_line, 5);
}

#[test]
fn extract_owners_recognizes_default_function_and_class_methods() {
    let function_owners = extract_owners(
        Path::new("src/defaults.ts"),
        r#"export default function calculate(value: number) {
    return value + 1;
}
"#,
    );
    let class_owners = extract_owners(
        Path::new("src/default-class.ts"),
        r#"
export default class Formatter {
    render() {
        return "ok";
    }
}
"#,
    );
    assert_eq!(function_owners.len(), 1);
    assert_eq!(function_owners[0].name, "calculate");
    assert_eq!(function_owners[0].owner_kind, OwnerKind::Function);
    assert_eq!(class_owners.len(), 1);
    assert_eq!(class_owners[0].name, "render");
    assert_eq!(class_owners[0].owner_kind, OwnerKind::Method);
}

#[test]
fn extract_owners_recognizes_reactish_function_and_arrow_components() {
    let owners = extract_owners(
        Path::new("src/card.tsx"),
        r#"export function PriceTag() {
    return <span>price</span>;
}
const InlinePrice = () => (
    <span>price</span>
);
"#,
    );
    assert_eq!(owners.len(), 2);
    assert_eq!(owners[0].name, "PriceTag");
    assert_eq!(owners[0].owner_kind, OwnerKind::Component);
    assert_eq!(owners[1].name, "InlinePrice");
    assert_eq!(owners[1].owner_kind, OwnerKind::Component);
}

#[test]
fn extract_owners_does_not_create_owner_from_comments_or_strings() {
    let owners = extract_owners(
        Path::new("src/docs.ts"),
        r#"// function fakeOwner() {}
"function stringOwner() {}";
"#,
    );
    assert!(owners.is_empty());
}

#[test]
fn extract_tests_recognizes_test_and_it_blocks() {
    let tests = extract_tests(
        Path::new("tests/lib.test.ts"),
        r#"test("alpha", () => { expect(applyDiscount(50, 100)).toBe(50); });
it("beta", () => { expect(otherHelper()).toBe(true); });
"#,
    );
    assert_eq!(tests.len(), 2);
    assert_eq!(tests[0].name, "alpha");
    assert_eq!(tests[1].name, "beta");
    assert!(tests[0].body_text.contains("applyDiscount(50, 100)"));
}

#[test]
fn find_related_tests_matches_by_call_name() {
    let owner = TypeScriptOwner {
        name: "applyDiscount".to_string(),
        file: PathBuf::from("src/lib.ts"),
        start_line: 1,
        end_line: 5,
        owner_kind: OwnerKind::Function,
        class_name: None,
        decorated: false,
        exported_as_default: false,
        class_default_export: false,
        arity: None,
        parameters: Vec::new(),
        source_text: None,
        imports: Vec::new(),
        method_kind: TypeScriptMethodKind::Ordinary,
    };
    let tests = vec![
        TypeScriptTest {
            name: "alpha".to_string(),
            local_name: "alpha".to_string(),
            describe_names: Vec::new(),
            file: PathBuf::from("tests/lib.test.ts"),
            line: 1,
            body_text: r#"test("alpha", () => { expect(applyDiscount(50, 100)).toBe(50); });"#
                .to_string(),
            assertions: Vec::new(),
            mocks_in_file: Vec::new(),
            imports_in_file: Vec::new(),
        },
        TypeScriptTest {
            name: "unrelated".to_string(),
            local_name: "unrelated".to_string(),
            describe_names: Vec::new(),
            file: PathBuf::from("tests/other.test.ts"),
            line: 1,
            body_text: r#"test("unrelated", () => { expect(otherHelper()).toBe(true); });"#
                .to_string(),
            assertions: Vec::new(),
            mocks_in_file: Vec::new(),
            imports_in_file: Vec::new(),
        },
    ];
    let related = find_related_tests(&owner, &tests, None, &ReExportIndex::empty(), None);
    assert_eq!(related.len(), 1);
    assert_eq!(related[0].name, "alpha");
}

#[test]
fn find_related_tests_ignores_object_method_calls_for_function_owners() {
    let owner = TypeScriptOwner {
        name: "applyDiscount".to_string(),
        file: PathBuf::from("src/lib.ts"),
        start_line: 1,
        end_line: 5,
        owner_kind: OwnerKind::Function,
        class_name: None,
        decorated: false,
        exported_as_default: false,
        class_default_export: false,
        arity: None,
        parameters: Vec::new(),
        source_text: None,
        imports: Vec::new(),
        method_kind: TypeScriptMethodKind::Ordinary,
    };
    let tests = vec![TypeScriptTest {
        name: "method call on another object".to_string(),
        local_name: "method call on another object".to_string(),
        describe_names: Vec::new(),
        file: PathBuf::from("tests/cart.test.ts"),
        line: 1,
        body_text: "expect(order.applyDiscount(50)).toBe(40);".to_string(),
        assertions: Vec::new(),
        mocks_in_file: Vec::new(),
        imports_in_file: Vec::new(),
    }];

    let related = find_related_tests(&owner, &tests, None, &ReExportIndex::empty(), None);

    assert!(related.is_empty());
}

#[test]
fn find_related_tests_matches_bounded_method_receiver_calls() {
    let owner = TypeScriptOwner {
        name: "total".to_string(),
        file: PathBuf::from("src/owners.ts"),
        start_line: 5,
        end_line: 8,
        owner_kind: OwnerKind::Method,
        class_name: Some("Cart".to_string()),
        decorated: false,
        exported_as_default: false,
        class_default_export: false,
        arity: None,
        parameters: Vec::new(),
        source_text: None,
        imports: Vec::new(),
        method_kind: TypeScriptMethodKind::Ordinary,
    };
    let tests = extract_tests(
        Path::new("tests/owners.test.ts"),
        r#"import { Cart as Subject } from "../src/owners";

test("cart total observes receiver", () => {
    const cart = new Subject();
    expect(cart.total()).toBe(1);
});
"#,
    );

    let candidates = related_test_candidates(&owner, &tests, None, &ReExportIndex::empty(), None);
    let related = find_related_tests(&owner, &tests, None, &ReExportIndex::empty(), None);

    assert_eq!(candidates.len(), 1);
    assert_eq!(
        candidates[0].relation,
        TypeScriptRelationKind::ReceiverOwnerCall
    );
    assert_eq!(related.len(), 1);
    assert_eq!(related[0].name, "cart total observes receiver");
    assert_eq!(related[0].oracle_kind, OracleKind::ExactValue);
    assert_eq!(related[0].oracle_strength, OracleStrength::Strong);
}

#[test]
fn find_related_tests_keeps_factory_receiver_calls_unrelated_for_method_owners() {
    let owner = TypeScriptOwner {
        name: "total".to_string(),
        file: PathBuf::from("src/owners.ts"),
        start_line: 5,
        end_line: 8,
        owner_kind: OwnerKind::Method,
        class_name: Some("Cart".to_string()),
        decorated: false,
        exported_as_default: false,
        class_default_export: false,
        arity: None,
        parameters: Vec::new(),
        source_text: None,
        imports: Vec::new(),
        method_kind: TypeScriptMethodKind::Ordinary,
    };
    let tests = extract_tests(
        Path::new("tests/owners.test.ts"),
        r#"import { Cart } from "../src/owners";

test("cart total through factory stays ambiguous", () => {
    const cart = makeCart();
    expect(cart.total()).toBe(1);
});
"#,
    );

    let related = find_related_tests(&owner, &tests, None, &ReExportIndex::empty(), None);

    assert!(related.is_empty());
}

#[test]
fn find_related_tests_keeps_dynamic_method_receiver_calls_unrelated() {
    let owner = TypeScriptOwner {
        name: "total".to_string(),
        file: PathBuf::from("src/owners.ts"),
        start_line: 5,
        end_line: 8,
        owner_kind: OwnerKind::Method,
        class_name: Some("Cart".to_string()),
        decorated: false,
        exported_as_default: false,
        class_default_export: false,
        arity: None,
        parameters: Vec::new(),
        source_text: None,
        imports: Vec::new(),
        method_kind: TypeScriptMethodKind::Ordinary,
    };
    let tests = extract_tests(
        Path::new("tests/owners.test.ts"),
        r#"import { Cart } from "../src/owners";

test("cart total through dynamic method stays ambiguous", () => {
    const cart = new Cart();
    const method = "total";
    expect(cart[method]()).toBe(1);
});
"#,
    );

    let related = find_related_tests(&owner, &tests, None, &ReExportIndex::empty(), None);

    assert!(related.is_empty());
}

#[test]
fn find_related_tests_keeps_mocked_method_receiver_calls_unrelated() {
    let owner = TypeScriptOwner {
        name: "total".to_string(),
        file: PathBuf::from("src/owners.ts"),
        start_line: 5,
        end_line: 8,
        owner_kind: OwnerKind::Method,
        class_name: Some("Cart".to_string()),
        decorated: false,
        exported_as_default: false,
        class_default_export: false,
        arity: None,
        parameters: Vec::new(),
        source_text: None,
        imports: Vec::new(),
        method_kind: TypeScriptMethodKind::Ordinary,
    };
    let tests = extract_tests(
        Path::new("tests/owners.test.ts"),
        r#"import { Cart } from "../src/owners";

vi.mock("../src/owners");

test("mocked cart total stays ambiguous", () => {
    const cart = new Cart();
    expect(cart.total()).toBe(1);
});
"#,
    );

    let related = find_related_tests(&owner, &tests, None, &ReExportIndex::empty(), None);

    assert!(related.is_empty());
}

#[test]
fn find_related_tests_keeps_mocked_function_owner_call_at_proximity() {
    // issue #2269: a test that mocks the changed Function owner's OWN module
    // must NOT be credited `DirectOwnerCall` — the owner call executes the
    // mock, not the changed code. Only the advisory same-file-stem proximity
    // heuristic may link the test.
    let owner = TypeScriptOwner {
        name: "applyDiscount".to_string(),
        file: PathBuf::from("src/owners.ts"),
        start_line: 1,
        end_line: 5,
        owner_kind: OwnerKind::Function,
        class_name: None,
        decorated: false,
        exported_as_default: false,
        class_default_export: false,
        arity: None,
        parameters: Vec::new(),
        source_text: None,
        imports: Vec::new(),
        method_kind: TypeScriptMethodKind::Ordinary,
    };
    let tests = extract_tests(
        Path::new("tests/owners.test.ts"),
        r#"import { applyDiscount } from "../src/owners";

vi.mock("../src/owners");

test("mocked applyDiscount stays ambiguous", () => {
    const result = applyDiscount(100, 100);
    expect(result).toBe(90);
});
"#,
    );

    let candidates = related_test_candidates(&owner, &tests, None, &ReExportIndex::empty(), None);

    assert!(
        candidates
            .iter()
            .all(|candidate| candidate.relation.is_uncertain()),
        "mocked owner module must not credit any oracle-using relation: {candidates:?}"
    );
    assert_eq!(candidates.len(), 1);
    assert_eq!(
        candidates[0].relation,
        TypeScriptRelationKind::SameFileProximity
    );
}

#[test]
fn find_related_tests_keeps_mocked_arrow_function_owner_call_at_proximity() {
    // issue #2269 (ArrowFunction arm): the owner-module mock guard must cover
    // both Function and ArrowFunction owner kinds — an arrow-function owner
    // call under its own module's mock executes the mock, not the changed
    // code, so only the advisory proximity heuristic may link the test.
    let owner = TypeScriptOwner {
        name: "applyDiscount".to_string(),
        file: PathBuf::from("src/owners.ts"),
        start_line: 1,
        end_line: 5,
        owner_kind: OwnerKind::ArrowFunction,
        class_name: None,
        decorated: false,
        exported_as_default: false,
        class_default_export: false,
        arity: None,
        parameters: Vec::new(),
        source_text: None,
        imports: Vec::new(),
        method_kind: TypeScriptMethodKind::Ordinary,
    };
    let tests = extract_tests(
        Path::new("tests/owners.test.ts"),
        r#"import { applyDiscount } from "../src/owners";

vi.mock("../src/owners");

test("mocked arrow applyDiscount stays ambiguous", () => {
    const result = applyDiscount(100, 100);
    expect(result).toBe(90);
});
"#,
    );

    let candidates = related_test_candidates(&owner, &tests, None, &ReExportIndex::empty(), None);

    assert!(
        candidates
            .iter()
            .all(|candidate| candidate.relation.is_uncertain()),
        "mocked owner module must not credit any oracle-using relation: {candidates:?}"
    );
    assert_eq!(candidates.len(), 1);
    assert_eq!(
        candidates[0].relation,
        TypeScriptRelationKind::SameFileProximity
    );
}

#[test]
fn find_related_tests_keeps_mocked_namespace_import_owner_call_at_proximity() {
    // issue #2269 (ImportedOwnerCall arm): the #2269 guard sits above BOTH the
    // `DirectOwnerCall` and the `ImportedOwnerCall` arms in
    // `owner_call_relation`. A namespace-import member call
    // (`discount.applyDiscount(...)`) cannot hit `DirectOwnerCall` (the `.`
    // receiver breaks the call boundary) and would credit `ImportedOwnerCall`
    // (`uses_oracle`) if unguarded — see the unmocked positive control
    // `find_related_tests_namespace_import_unchanged_imported_owner_call`.
    // Under an owner-module mock it must fall back to the advisory
    // same-file-stem proximity link only.
    let owner = TypeScriptOwner {
        name: "applyDiscount".to_string(),
        file: PathBuf::from("src/owners.ts"),
        start_line: 1,
        end_line: 5,
        owner_kind: OwnerKind::Function,
        class_name: None,
        decorated: false,
        exported_as_default: false,
        class_default_export: false,
        arity: None,
        parameters: Vec::new(),
        source_text: None,
        imports: Vec::new(),
        method_kind: TypeScriptMethodKind::Ordinary,
    };
    let tests = extract_tests(
        Path::new("tests/owners.test.ts"),
        r#"import * as discount from "../src/owners";

jest.mock("../src/owners");

test("mocked namespace applyDiscount stays ambiguous", () => {
    const result = discount.applyDiscount(100, 100);
    expect(result).toBe(90);
});
"#,
    );

    let candidates = related_test_candidates(&owner, &tests, None, &ReExportIndex::empty(), None);

    assert!(
        candidates
            .iter()
            .all(|candidate| candidate.relation.is_uncertain()),
        "mocked owner module must not credit any oracle-using relation: {candidates:?}"
    );
    assert_eq!(candidates.len(), 1);
    assert_eq!(
        candidates[0].relation,
        TypeScriptRelationKind::SameFileProximity
    );
}

#[test]
fn find_related_tests_credits_unmocked_function_owner_call() {
    // Positive control for the #2269 guard: the same scenario WITHOUT the
    // owner-module mock keeps the `DirectOwnerCall` credit.
    let owner = TypeScriptOwner {
        name: "applyDiscount".to_string(),
        file: PathBuf::from("src/owners.ts"),
        start_line: 1,
        end_line: 5,
        owner_kind: OwnerKind::Function,
        class_name: None,
        decorated: false,
        exported_as_default: false,
        class_default_export: false,
        arity: None,
        parameters: Vec::new(),
        source_text: None,
        imports: Vec::new(),
        method_kind: TypeScriptMethodKind::Ordinary,
    };
    let tests = extract_tests(
        Path::new("tests/owners.test.ts"),
        r#"import { applyDiscount } from "../src/owners";

test("unmocked applyDiscount observes the owner", () => {
    const result = applyDiscount(100, 100);
    expect(result).toBe(90);
});
"#,
    );

    let candidates = related_test_candidates(&owner, &tests, None, &ReExportIndex::empty(), None);

    assert_eq!(candidates.len(), 1);
    assert_eq!(
        candidates[0].relation,
        TypeScriptRelationKind::DirectOwnerCall
    );
}

#[test]
fn classify_change_stays_weakly_exposed_when_test_mocks_owner_module() -> Result<(), String> {
    // issue #2269: mocked owner module + strong exact-value oracle must not
    // classify `exposed`; the `mocked_module` static limit stays disclosed.
    let owner = TypeScriptOwner {
        name: "applyDiscount".to_string(),
        file: PathBuf::from("src/lib.ts"),
        start_line: 1,
        end_line: 5,
        owner_kind: OwnerKind::Function,
        class_name: None,
        decorated: false,
        exported_as_default: false,
        class_default_export: false,
        arity: None,
        parameters: Vec::new(),
        source_text: None,
        imports: Vec::new(),
        method_kind: TypeScriptMethodKind::Ordinary,
    };
    let tests = extract_tests(
        Path::new("tests/lib.test.ts"),
        r#"import { applyDiscount } from "../src/lib";

jest.mock("../src/lib");

test("mocked applyDiscount stays ambiguous", () => {
    const result = applyDiscount(100, 100);
    expect(result).toBe(90);
});
"#,
    );
    let finding = classify_change(
        Path::new("src/lib.ts"),
        2,
        "    if (amount >= threshold) {",
        &[owner],
        &tests,
        None,
        &ReExportIndex::empty(),
        None,
    )
    .ok_or_else(|| "expected a finding for the changed line".to_string())?;
    assert!(
        matches!(finding.class, ExposureClass::WeaklyExposed),
        "expected weakly_exposed, got {:?}",
        finding.class
    );
    assert_eq!(
        finding.static_limit_kind,
        Some(StaticLimitKind::MockedModule)
    );
    // Fail-closed actionability contract (review round, #2269): the
    // classify-level `Finding` carries no `repair_packet_ready` field — the
    // preview actionability evidence is the surface the struct does carry.
    // Pin the static-limitation gap and the absence of any packet credit; the
    // output-layer `repair packet ready: false` is pinned by the
    // `typescript_adversarial_owner_module_mock` golden and the corpus gate's
    // `must_not_emit_repair_packet` assertion.
    assert_evidence_contains(&finding, "gap_state: static_limitation");
    assert_evidence_contains(&finding, "actionability_category: mocked_module");
    assert_evidence_contains(
        &finding,
        "why_not_actionable: static limit `mocked_module` prevents bounded TypeScript repair guidance",
    );
    assert_evidence_lacks(&finding, "repair_packet_ready: true");
    Ok(())
}

#[test]
fn find_related_tests_matches_bounded_class_method_calls() {
    let owner = TypeScriptOwner {
        name: "build".to_string(),
        file: PathBuf::from("src/owners.ts"),
        start_line: 10,
        end_line: 12,
        owner_kind: OwnerKind::ClassMethod,
        class_name: Some("Cart".to_string()),
        decorated: false,
        exported_as_default: false,
        class_default_export: false,
        arity: None,
        parameters: Vec::new(),
        source_text: None,
        imports: Vec::new(),
        method_kind: TypeScriptMethodKind::Ordinary,
    };
    let tests = extract_tests(
        Path::new("tests/owners.test.ts"),
        r#"import { Cart as Subject } from "../src/owners";

test("static build observes class method", () => {
    expect(Subject.build()).toBeDefined();
});
"#,
    );

    let candidates = related_test_candidates(&owner, &tests, None, &ReExportIndex::empty(), None);
    let related = find_related_tests(&owner, &tests, None, &ReExportIndex::empty(), None);

    assert_eq!(candidates.len(), 1);
    assert_eq!(
        candidates[0].relation,
        TypeScriptRelationKind::ClassMethodCall
    );
    assert_eq!(related.len(), 1);
    assert_eq!(related[0].name, "static build observes class method");
    assert_eq!(related[0].oracle_kind, OracleKind::SmokeOnly);
    assert_eq!(related[0].oracle_strength, OracleStrength::Smoke);
}

#[test]
fn find_related_tests_keeps_shadowed_class_method_calls_unrelated() {
    let owner = TypeScriptOwner {
        name: "build".to_string(),
        file: PathBuf::from("src/owners.ts"),
        start_line: 10,
        end_line: 12,
        owner_kind: OwnerKind::ClassMethod,
        class_name: Some("Cart".to_string()),
        decorated: false,
        exported_as_default: false,
        class_default_export: false,
        arity: None,
        parameters: Vec::new(),
        source_text: None,
        imports: Vec::new(),
        method_kind: TypeScriptMethodKind::Ordinary,
    };
    let tests = extract_tests(
        Path::new("tests/owners.test.ts"),
        r#"import { Cart } from "../src/owners";

test("shadowed static build stays ambiguous", () => {
    const Cart = { build: () => "shadow" };
    expect(Cart.build()).toBe("shadow");
});
"#,
    );

    let related = find_related_tests(&owner, &tests, None, &ReExportIndex::empty(), None);

    assert!(related.is_empty());
}

#[test]
fn find_related_tests_matches_same_file_class_method_calls() {
    let owner = TypeScriptOwner {
        name: "build".to_string(),
        file: PathBuf::from("src/owners.ts"),
        start_line: 10,
        end_line: 12,
        owner_kind: OwnerKind::ClassMethod,
        class_name: Some("Cart".to_string()),
        decorated: false,
        exported_as_default: false,
        class_default_export: false,
        arity: None,
        parameters: Vec::new(),
        source_text: None,
        imports: Vec::new(),
        method_kind: TypeScriptMethodKind::Ordinary,
    };
    let tests = extract_tests(
        Path::new("src/owners.ts"),
        r#"test("same file static build observes class method", () => {
    expect(Cart.build()).toBeDefined();
});
"#,
    );

    let candidates = related_test_candidates(&owner, &tests, None, &ReExportIndex::empty(), None);
    let related = find_related_tests(&owner, &tests, None, &ReExportIndex::empty(), None);

    assert_eq!(candidates.len(), 1);
    assert_eq!(
        candidates[0].relation,
        TypeScriptRelationKind::ClassMethodCall
    );
    assert_eq!(related.len(), 1);
    assert_eq!(
        related[0].name,
        "same file static build observes class method"
    );
}

#[test]
fn find_related_tests_keeps_namespace_class_method_calls_unrelated() {
    let owner = TypeScriptOwner {
        name: "build".to_string(),
        file: PathBuf::from("src/owners.ts"),
        start_line: 10,
        end_line: 12,
        owner_kind: OwnerKind::ClassMethod,
        class_name: Some("Cart".to_string()),
        decorated: false,
        exported_as_default: false,
        class_default_export: false,
        arity: None,
        parameters: Vec::new(),
        source_text: None,
        imports: Vec::new(),
        method_kind: TypeScriptMethodKind::Ordinary,
    };
    let tests = extract_tests(
        Path::new("tests/owners.test.ts"),
        r#"import * as Owners from "../src/owners";

test("namespace static build stays ambiguous", () => {
    expect(Owners.Cart.build()).toBeDefined();
});
"#,
    );

    let related = find_related_tests(&owner, &tests, None, &ReExportIndex::empty(), None);

    assert!(related.is_empty());
}

#[test]
fn find_related_tests_keeps_mocked_class_method_calls_unrelated() {
    let owner = TypeScriptOwner {
        name: "build".to_string(),
        file: PathBuf::from("src/owners.ts"),
        start_line: 10,
        end_line: 12,
        owner_kind: OwnerKind::ClassMethod,
        class_name: Some("Cart".to_string()),
        decorated: false,
        exported_as_default: false,
        class_default_export: false,
        arity: None,
        parameters: Vec::new(),
        source_text: None,
        imports: Vec::new(),
        method_kind: TypeScriptMethodKind::Ordinary,
    };
    let tests = extract_tests(
        Path::new("tests/owners.test.ts"),
        r#"import { Cart } from "../src/owners";

vi.mock("../src/owners");

test("mocked static build stays ambiguous", () => {
    expect(Cart.build()).toBeDefined();
});
"#,
    );

    let related = find_related_tests(&owner, &tests, None, &ReExportIndex::empty(), None);

    assert!(related.is_empty());
}

#[test]
fn find_related_tests_requires_class_name_for_class_method_calls() {
    let owner = TypeScriptOwner {
        name: "build".to_string(),
        file: PathBuf::from("src/owners.ts"),
        start_line: 10,
        end_line: 12,
        owner_kind: OwnerKind::ClassMethod,
        class_name: None,
        decorated: false,
        exported_as_default: false,
        class_default_export: false,
        arity: None,
        parameters: Vec::new(),
        source_text: None,
        imports: Vec::new(),
        method_kind: TypeScriptMethodKind::Ordinary,
    };
    let tests = extract_tests(
        Path::new("tests/owners.test.ts"),
        r#"import { Cart } from "../src/owners";

test("unknown class static build stays ambiguous", () => {
    expect(Cart.build()).toBeDefined();
});
"#,
    );

    let related = find_related_tests(&owner, &tests, None, &ReExportIndex::empty(), None);

    assert!(related.is_empty());
}

#[test]
fn find_related_tests_matches_module_initializer_named_import_observer() {
    let owner = TypeScriptOwner {
        name: "DEFAULT_RATE".to_string(),
        file: PathBuf::from("src/owners.ts"),
        start_line: 15,
        end_line: 15,
        owner_kind: OwnerKind::ModuleFunction,
        class_name: None,
        decorated: false,
        exported_as_default: false,
        class_default_export: false,
        arity: None,
        parameters: Vec::new(),
        source_text: None,
        imports: Vec::new(),
        method_kind: TypeScriptMethodKind::Ordinary,
    };
    let tests = extract_tests(
        Path::new("tests/owners.test.ts"),
        r#"import { DEFAULT_RATE as rate } from "../src/owners";

test("rate value observes initializer", () => {
    expect(rate).toBe(0.09);
});
"#,
    );

    let candidates = related_test_candidates(&owner, &tests, None, &ReExportIndex::empty(), None);
    let related = find_related_tests(&owner, &tests, None, &ReExportIndex::empty(), None);

    assert_eq!(candidates.len(), 1);
    assert_eq!(
        candidates[0].relation,
        TypeScriptRelationKind::ModuleValueReference
    );
    assert_eq!(related.len(), 1);
    assert_eq!(related[0].name, "rate value observes initializer");
    assert_eq!(related[0].oracle_kind, OracleKind::ExactValue);
    assert_eq!(related[0].oracle_strength, OracleStrength::Strong);
}

#[test]
fn find_related_tests_matches_module_initializer_namespace_observer() {
    let owner = TypeScriptOwner {
        name: "DEFAULT_RATE".to_string(),
        file: PathBuf::from("src/owners.ts"),
        start_line: 15,
        end_line: 15,
        owner_kind: OwnerKind::ModuleFunction,
        class_name: None,
        decorated: false,
        exported_as_default: false,
        class_default_export: false,
        arity: None,
        parameters: Vec::new(),
        source_text: None,
        imports: Vec::new(),
        method_kind: TypeScriptMethodKind::Ordinary,
    };
    let tests = extract_tests(
        Path::new("tests/owners.test.ts"),
        r#"import * as owners from "../src/owners";

test("rate value observes namespace initializer", () => {
    expect(owners.DEFAULT_RATE).toBe(0.09);
});
"#,
    );

    let related = find_related_tests(&owner, &tests, None, &ReExportIndex::empty(), None);

    assert_eq!(related.len(), 1);
    assert_eq!(related[0].name, "rate value observes namespace initializer");
    assert_eq!(related[0].oracle_kind, OracleKind::ExactValue);
}

#[test]
fn find_related_tests_keeps_module_initializer_shadow_and_non_expect_references_unrelated() {
    let owner = TypeScriptOwner {
        name: "DEFAULT_RATE".to_string(),
        file: PathBuf::from("src/owners.ts"),
        start_line: 15,
        end_line: 15,
        owner_kind: OwnerKind::ModuleFunction,
        class_name: None,
        decorated: false,
        exported_as_default: false,
        class_default_export: false,
        arity: None,
        parameters: Vec::new(),
        source_text: None,
        imports: Vec::new(),
        method_kind: TypeScriptMethodKind::Ordinary,
    };
    let tests = extract_tests(
        Path::new("tests/owners.test.ts"),
        r#"import { DEFAULT_RATE } from "../src/owners";

test("shadowed rate stays ambiguous", () => {
    const DEFAULT_RATE = 0.1;
    expect(DEFAULT_RATE).toBe(0.1);
});

test("derived rate stays ambiguous", () => {
    const actual = DEFAULT_RATE;
    expect(actual).toBe(0.09);
});

test("string mention stays ambiguous", () => {
    expect("DEFAULT_RATE").toBe("DEFAULT_RATE");
});
"#,
    );

    let related = find_related_tests(&owner, &tests, None, &ReExportIndex::empty(), None);

    assert!(related.is_empty());
}

#[test]
fn find_related_tests_matches_named_import_alias_calls() {
    let owner = TypeScriptOwner {
        name: "applyDiscount".to_string(),
        file: PathBuf::from("src/pricing.ts"),
        start_line: 1,
        end_line: 5,
        owner_kind: OwnerKind::Function,
        class_name: None,
        decorated: false,
        exported_as_default: false,
        class_default_export: false,
        arity: None,
        parameters: Vec::new(),
        source_text: None,
        imports: Vec::new(),
        method_kind: TypeScriptMethodKind::Ordinary,
    };
    let tests = extract_tests(
        Path::new("tests/pricing.test.ts"),
        r#"import { applyDiscount as subject } from "../src/pricing";

test("alias import observes threshold", () => {
    expect(subject(100, 100)).toBe(90);
});
"#,
    );

    let candidates = related_test_candidates(&owner, &tests, None, &ReExportIndex::empty(), None);
    let related = find_related_tests(&owner, &tests, None, &ReExportIndex::empty(), None);

    // RIPR-SPEC-0102: alias-rename import must produce ImportAliasOwnerCall,
    // which maps to relation_reason=direct_owner_call / relation_confidence=high.
    assert_eq!(related.len(), 1);
    assert_eq!(related[0].name, "alias import observes threshold");
    assert_eq!(
        candidates[0].relation,
        TypeScriptRelationKind::ImportAliasOwnerCall,
        "alias-rename import must be classified as ImportAliasOwnerCall"
    );
    assert_eq!(
        related[0].relation_reason,
        Some(crate::domain::RelationReason::DirectOwnerCall),
        "alias-rename import maps to domain DirectOwnerCall"
    );
    assert_eq!(
        related[0].relation_confidence,
        Some(crate::domain::RelationConfidence::High),
        "alias-rename import must have High relation confidence"
    );
}

/// RIPR-SPEC-0102 control 2: wrong-name alias (`import { otherFn as cv }`,
/// otherFn != owner) must NOT be credited as ImportAliasOwnerCall.
#[test]
fn find_related_tests_alias_wrong_name_not_credited() {
    let owner = TypeScriptOwner {
        name: "applyDiscount".to_string(),
        file: PathBuf::from("src/pricing.ts"),
        start_line: 1,
        end_line: 5,
        owner_kind: OwnerKind::Function,
        class_name: None,
        decorated: false,
        exported_as_default: false,
        class_default_export: false,
        arity: None,
        parameters: Vec::new(),
        source_text: None,
        imports: Vec::new(),
        method_kind: TypeScriptMethodKind::Ordinary,
    };
    let tests = extract_tests(
        Path::new("tests/pricing.test.ts"),
        r#"import { otherFn as cv } from "../src/pricing";

test("wrong-name alias", () => {
    expect(cv(100)).toBe(90);
});
"#,
    );

    let candidates = related_test_candidates(&owner, &tests, None, &ReExportIndex::empty(), None);

    assert!(
        candidates
            .iter()
            .all(|c| c.relation != TypeScriptRelationKind::ImportAliasOwnerCall),
        "wrong-name alias (otherFn != owner) must not produce ImportAliasOwnerCall"
    );
}

/// RIPR-SPEC-0102 control 3: shadowed local binding — the test re-declares
/// the alias in the body (`const cv = ...`), so `cv(...)` reaches the shadow,
/// not the owner.  Must NOT be credited at High confidence.
#[test]
fn find_related_tests_alias_shadowed_local_not_credited_high() {
    let owner = TypeScriptOwner {
        name: "computeValue".to_string(),
        file: PathBuf::from("src/compute.ts"),
        start_line: 1,
        end_line: 5,
        owner_kind: OwnerKind::Function,
        class_name: None,
        decorated: false,
        exported_as_default: false,
        class_default_export: false,
        arity: None,
        parameters: Vec::new(),
        source_text: None,
        imports: Vec::new(),
        method_kind: TypeScriptMethodKind::Ordinary,
    };
    let tests = extract_tests(
        Path::new("tests/compute.test.ts"),
        r#"import { computeValue as cv } from "../src/compute";

test("shadow guard", () => {
    const cv = () => 42;
    expect(cv(5)).toBe(42);
});
"#,
    );

    let candidates = related_test_candidates(&owner, &tests, None, &ReExportIndex::empty(), None);

    assert!(
        candidates
            .iter()
            .all(|c| c.relation != TypeScriptRelationKind::ImportAliasOwnerCall),
        "shadowed local alias must not produce ImportAliasOwnerCall (shadow guard)"
    );
}

/// RIPR-SPEC-0102 control 4: non-alias import (`import { computeValue }`)
/// still produces DirectOwnerCall via the existing path.
#[test]
fn find_related_tests_non_alias_import_still_direct_owner_call() {
    let owner = TypeScriptOwner {
        name: "computeValue".to_string(),
        file: PathBuf::from("src/compute.ts"),
        start_line: 1,
        end_line: 5,
        owner_kind: OwnerKind::Function,
        class_name: None,
        decorated: false,
        exported_as_default: false,
        class_default_export: false,
        arity: None,
        parameters: Vec::new(),
        source_text: None,
        imports: Vec::new(),
        method_kind: TypeScriptMethodKind::Ordinary,
    };
    let tests = extract_tests(
        Path::new("tests/compute.test.ts"),
        r#"import { computeValue } from "../src/compute";

test("non-alias direct call", () => {
    expect(computeValue(5)).toBe(10);
});
"#,
    );

    let candidates = related_test_candidates(&owner, &tests, None, &ReExportIndex::empty(), None);
    let related = find_related_tests(&owner, &tests, None, &ReExportIndex::empty(), None);

    assert_eq!(candidates.len(), 1);
    assert_eq!(
        candidates[0].relation,
        TypeScriptRelationKind::DirectOwnerCall,
        "non-alias import must remain DirectOwnerCall, not ImportAliasOwnerCall"
    );
    assert_eq!(
        related[0].relation_reason,
        Some(crate::domain::RelationReason::DirectOwnerCall)
    );
    assert_eq!(
        related[0].relation_confidence,
        Some(crate::domain::RelationConfidence::High)
    );
}

/// RIPR-SPEC-0102 control 5: namespace import (`import * as ns`) still
/// produces ImportedOwnerCall (out of scope for alias upgrade) → import_path_affinity/medium.
#[test]
fn find_related_tests_namespace_import_unchanged_imported_owner_call() {
    let owner = TypeScriptOwner {
        name: "computeValue".to_string(),
        file: PathBuf::from("src/compute.ts"),
        start_line: 1,
        end_line: 5,
        owner_kind: OwnerKind::Function,
        class_name: None,
        decorated: false,
        exported_as_default: false,
        class_default_export: false,
        arity: None,
        parameters: Vec::new(),
        source_text: None,
        imports: Vec::new(),
        method_kind: TypeScriptMethodKind::Ordinary,
    };
    let tests = extract_tests(
        Path::new("tests/compute.test.ts"),
        r#"import * as ns from "../src/compute";

test("namespace import member call", () => {
    expect(ns.computeValue(5)).toBe(10);
});
"#,
    );

    let candidates = related_test_candidates(&owner, &tests, None, &ReExportIndex::empty(), None);
    let related = find_related_tests(&owner, &tests, None, &ReExportIndex::empty(), None);

    assert_eq!(candidates.len(), 1);
    assert_eq!(
        candidates[0].relation,
        TypeScriptRelationKind::ImportedOwnerCall,
        "namespace import must remain ImportedOwnerCall (out of scope for alias upgrade)"
    );
    assert_eq!(
        related[0].relation_reason,
        Some(crate::domain::RelationReason::ImportPathAffinity),
        "namespace import must still map to import_path_affinity"
    );
    assert_eq!(
        related[0].relation_confidence,
        Some(crate::domain::RelationConfidence::Medium),
        "namespace import must remain Medium confidence"
    );
}

#[test]
fn find_related_tests_matches_namespace_import_member_calls() {
    let owner = TypeScriptOwner {
        name: "applyDiscount".to_string(),
        file: PathBuf::from("src/pricing.ts"),
        start_line: 1,
        end_line: 5,
        owner_kind: OwnerKind::Function,
        class_name: None,
        decorated: false,
        exported_as_default: false,
        class_default_export: false,
        arity: None,
        parameters: Vec::new(),
        source_text: None,
        imports: Vec::new(),
        method_kind: TypeScriptMethodKind::Ordinary,
    };
    let tests = extract_tests(
        Path::new("tests/pricing.test.ts"),
        r#"import * as pricing from "../src/pricing";

test("namespace import observes threshold", () => {
    expect(pricing.applyDiscount(100, 100)).toBe(90);
});
"#,
    );

    let related = find_related_tests(&owner, &tests, None, &ReExportIndex::empty(), None);

    assert_eq!(related.len(), 1);
    assert_eq!(related[0].name, "namespace import observes threshold");
}

#[test]
fn find_related_tests_ignores_unrelated_and_type_only_import_aliases() {
    let owner = TypeScriptOwner {
        name: "applyDiscount".to_string(),
        file: PathBuf::from("src/pricing.ts"),
        start_line: 1,
        end_line: 5,
        owner_kind: OwnerKind::Function,
        class_name: None,
        decorated: false,
        exported_as_default: false,
        class_default_export: false,
        arity: None,
        parameters: Vec::new(),
        source_text: None,
        imports: Vec::new(),
        method_kind: TypeScriptMethodKind::Ordinary,
    };
    let tests = extract_tests(
        Path::new("tests/pricing.test.ts"),
        r#"import { applyDiscount as otherSubject } from "../src/other-pricing";
import type { applyDiscount as typeOnlySubject } from "../src/pricing";
import { applyDiscount } from "../src/other-pricing";

test("wrong import source", () => {
    expect(otherSubject(100, 100)).toBe(90);
});

test("wrong direct import source", () => {
    expect(applyDiscount(100, 100)).toBe(90);
});

test("type only import", () => {
    expect(typeOnlySubject(100, 100)).toBe(90);
});
"#,
    );

    let related = find_related_tests(&owner, &tests, None, &ReExportIndex::empty(), None);

    assert!(related.is_empty());
}

#[test]
fn find_related_tests_ignores_call_shaped_string_mentions() {
    let owner = TypeScriptOwner {
        name: "applyDiscount".to_string(),
        file: PathBuf::from("src/lib.ts"),
        start_line: 1,
        end_line: 5,
        owner_kind: OwnerKind::Function,
        class_name: None,
        decorated: false,
        exported_as_default: false,
        class_default_export: false,
        arity: None,
        parameters: Vec::new(),
        source_text: None,
        imports: Vec::new(),
        method_kind: TypeScriptMethodKind::Ordinary,
    };
    let tests = vec![TypeScriptTest {
        name: "string mention".to_string(),
        local_name: "string mention".to_string(),
        describe_names: Vec::new(),
        file: PathBuf::from("tests/docs.test.ts"),
        line: 1,
        body_text: r#"expect("applyDiscount(").toContain("applyDiscount(");"#.to_string(),
        assertions: Vec::new(),
        mocks_in_file: Vec::new(),
        imports_in_file: Vec::new(),
    }];

    let related = find_related_tests(&owner, &tests, None, &ReExportIndex::empty(), None);

    assert!(related.is_empty());
}

#[test]
fn find_related_tests_ignores_call_shaped_comment_mentions() {
    let owner = TypeScriptOwner {
        name: "applyDiscount".to_string(),
        file: PathBuf::from("src/lib.ts"),
        start_line: 1,
        end_line: 5,
        owner_kind: OwnerKind::Function,
        class_name: None,
        decorated: false,
        exported_as_default: false,
        class_default_export: false,
        arity: None,
        parameters: Vec::new(),
        source_text: None,
        imports: Vec::new(),
        method_kind: TypeScriptMethodKind::Ordinary,
    };
    let tests = vec![
        TypeScriptTest {
            name: "line comment mention".to_string(),
            local_name: "line comment mention".to_string(),
            describe_names: Vec::new(),
            file: PathBuf::from("tests/docs.test.ts"),
            line: 1,
            body_text: "// applyDiscount(\nexpect(total).toBe(40);".to_string(),
            assertions: Vec::new(),
            mocks_in_file: Vec::new(),
            imports_in_file: Vec::new(),
        },
        TypeScriptTest {
            name: "block comment mention".to_string(),
            local_name: "block comment mention".to_string(),
            describe_names: Vec::new(),
            file: PathBuf::from("tests/docs.test.ts"),
            line: 4,
            body_text: "/* applyDiscount(\n */\nexpect(total).toBe(40);".to_string(),
            assertions: Vec::new(),
            mocks_in_file: Vec::new(),
            imports_in_file: Vec::new(),
        },
    ];

    let related = find_related_tests(&owner, &tests, None, &ReExportIndex::empty(), None);

    assert!(related.is_empty());
}

#[test]
fn related_test_candidates_use_name_and_proximity_links_as_uncertain_relations() {
    let owner = TypeScriptOwner {
        name: "applyDiscount".to_string(),
        file: PathBuf::from("src/pricing.ts"),
        start_line: 1,
        end_line: 5,
        owner_kind: OwnerKind::Function,
        class_name: None,
        decorated: false,
        exported_as_default: false,
        class_default_export: false,
        arity: None,
        parameters: Vec::new(),
        source_text: None,
        imports: Vec::new(),
        method_kind: TypeScriptMethodKind::Ordinary,
    };
    // Each test references the owner without a recognized call shape, so the
    // proximity/name heuristics only rank an existing reference
    // (RIPR-SPEC-0027).
    let mut tests = extract_tests(
        Path::new("tests/pricing.test.ts"),
        r#"test("threshold documented elsewhere", () => {
    const discount = applyDiscount;
    expect(90).toBe(90);
});
"#,
    );
    tests.extend(extract_tests(
        Path::new("tests/checkout.test.ts"),
        r#"describe("applyDiscount", () => {
    test("threshold documented elsewhere", () => {
        expect(applyDiscount).toBeDefined();
        expect(90).toBe(90);
    });
});
"#,
    ));
    tests.extend(extract_tests(
        Path::new("tests/cart.test.ts"),
        r#"test("applyDiscount boundary", () => {
    [100].map(applyDiscount);
    expect(90).toBe(90);
});
"#,
    ));

    let candidates = related_test_candidates(&owner, &tests, None, &ReExportIndex::empty(), None);
    let relations: Vec<_> = candidates
        .iter()
        .map(|candidate| candidate.relation)
        .collect();

    assert_eq!(
        relations,
        vec![
            TypeScriptRelationKind::SameFileProximity,
            TypeScriptRelationKind::DescribeName,
            TypeScriptRelationKind::TestName,
        ]
    );
    assert!(
        candidates
            .iter()
            .all(|candidate| candidate.relation.is_uncertain())
    );

    let related = find_related_tests(&owner, &tests, None, &ReExportIndex::empty(), None);
    assert_eq!(related.len(), 3);
    assert!(
        related
            .iter()
            .all(|test| test.oracle_kind == OracleKind::Unknown)
    );
}

#[test]
fn related_test_name_proximity_ignores_partial_tokens() {
    let owner = TypeScriptOwner {
        name: "applyDiscount".to_string(),
        file: PathBuf::from("src/pricing.ts"),
        start_line: 1,
        end_line: 5,
        owner_kind: OwnerKind::Function,
        class_name: None,
        decorated: false,
        exported_as_default: false,
        class_default_export: false,
        arity: None,
        parameters: Vec::new(),
        source_text: None,
        imports: Vec::new(),
        method_kind: TypeScriptMethodKind::Ordinary,
    };
    let tests = extract_tests(
        Path::new("tests/checkout.test.ts"),
        r#"describe("application discounting", () => {
    test("discount boundary", () => {
        expect(90).toBe(90);
    });
});
"#,
    );

    let candidates = related_test_candidates(&owner, &tests, None, &ReExportIndex::empty(), None);

    assert!(candidates.is_empty());
}

#[test]
fn classify_change_uses_heuristic_links_as_weak_uncertain_proximity() -> Result<(), String> {
    let owner = TypeScriptOwner {
        name: "applyDiscount".to_string(),
        file: PathBuf::from("src/pricing.ts"),
        start_line: 1,
        end_line: 5,
        owner_kind: OwnerKind::Function,
        class_name: None,
        decorated: false,
        exported_as_default: false,
        class_default_export: false,
        arity: None,
        parameters: Vec::new(),
        source_text: None,
        imports: Vec::new(),
        method_kind: TypeScriptMethodKind::Ordinary,
    };
    let tests = extract_tests(
        Path::new("tests/pricing.test.ts"),
        r#"test("threshold documented elsewhere", () => {
    const discount = applyDiscount;
    expect(90).toBe(90);
});
"#,
    );

    let finding = classify_change(
        Path::new("src/pricing.ts"),
        2,
        "    if (amount >= threshold) {",
        &[owner],
        &tests,
        None,
        &ReExportIndex::empty(),
        None,
    )
    .ok_or_else(|| "expected a finding when an owner contains the changed line".to_string())?;

    assert!(matches!(finding.class, ExposureClass::WeaklyExposed));
    assert_eq!(finding.ripr.reach.state, StageState::Weak);
    assert_eq!(finding.related_tests.len(), 1);
    assert_eq!(finding.related_tests[0].oracle_kind, OracleKind::Unknown);
    assert!(finding.evidence.iter().any(|item| item
        == "related_test_relation: same_file_proximity (threshold documented elsewhere)"));
    assert!(finding.evidence.iter().any(|item| item
        == "related_test_uncertain: same_file_proximity (threshold documented elsewhere)"));
    assert!(
        finding
            .recommended_next_step
            .as_deref()
            .is_some_and(|step| step.contains("heuristic only"))
    );
    Ok(())
}

#[test]
fn classify_change_returns_weakly_exposed_when_related_test_exists() -> Result<(), String> {
    let owner = TypeScriptOwner {
        name: "applyDiscount".to_string(),
        file: PathBuf::from("src/lib.ts"),
        start_line: 1,
        end_line: 5,
        owner_kind: OwnerKind::Function,
        class_name: None,
        decorated: false,
        exported_as_default: false,
        class_default_export: false,
        arity: None,
        parameters: Vec::new(),
        source_text: None,
        imports: Vec::new(),
        method_kind: TypeScriptMethodKind::Ordinary,
    };
    let test = TypeScriptTest {
        name: "alpha".to_string(),
        local_name: "alpha".to_string(),
        describe_names: Vec::new(),
        file: PathBuf::from("tests/lib.test.ts"),
        line: 1,
        body_text: "applyDiscount(50, 100)".to_string(),
        assertions: Vec::new(),
        mocks_in_file: Vec::new(),
        imports_in_file: Vec::new(),
    };
    let finding = classify_change(
        Path::new("src/lib.ts"),
        2,
        "    if (amount >= threshold) {",
        &[owner],
        &[test],
        None,
        &ReExportIndex::empty(),
        None,
    )
    .ok_or_else(|| "expected a finding when an owner contains the changed line".to_string())?;
    assert!(matches!(finding.class, ExposureClass::WeaklyExposed));
    assert_eq!(finding.language, Some(DomainLanguageId::TypeScript));
    assert_eq!(finding.language_status, Some(LanguageStatus::Preview));
    assert_eq!(finding.related_tests.len(), 1);
    Ok(())
}

#[test]
fn classify_change_marks_weak_direct_typescript_candidate_advisory() -> Result<(), String> {
    let finding = classify_weak_direct_line("    if (amount >= threshold) {")?;

    assert!(finding.canonical_gap.is_none());
    assert_evidence_contains(&finding, "gap_state: advisory");
    assert_evidence_contains(&finding, "actionability_category: incomplete_repair_packet");
    assert_evidence_contains(
        &finding,
        "why_not_actionable: TypeScript preview has owner, related-test, oracle, and probe evidence but lacks a complete repair packet contract",
    );
    assert_evidence_contains(&finding, "missing_actionability_fields: canonical_gap_id");
    assert_evidence_contains(&finding, "verify_command");
    assert_evidence_contains(&finding, "receipt_command");
    assert_evidence_contains(
        &finding,
        "raw_evidence_ref: file=src/lib.ts;line=2;kind=typescript_preview_probe",
    );
    assert!(
        finding
            .missing
            .iter()
            .any(|line| line.contains("incomplete_repair_packet")),
        "expected actionability summary in missing text, got {:?}",
        finding.missing
    );
    assert!(
        finding
            .missing
            .iter()
            .any(|line| line.contains("smoke-only oracle")),
        "expected weak smoke oracle guidance, got {:?}",
        finding.missing
    );
    assert!(
        finding
            .recommended_next_step
            .as_deref()
            .is_some_and(|step| step.contains("smoke-only assertion")
                && step.contains("no actionable repair packet is emitted"))
    );
    Ok(())
}

#[test]
fn typescript_preview_weak_oracle_guidance_names_snapshot_exact_value_shape() -> Result<(), String>
{
    let owner = test_owner("renderSummary", "src/lib.ts");
    let test = direct_test_with_assertion(
        "renders summary snapshot",
        "const value = renderSummary(status);\nexpect(value).toMatchSnapshot();",
        "toMatchSnapshot",
        0,
        OracleKind::Snapshot,
        OracleStrength::Medium,
    );
    let finding = classify_change(
        Path::new("src/lib.ts"),
        2,
        "    return `summary:${status.trim()}`;",
        &[owner],
        &[test],
        None,
        &ReExportIndex::empty(),
        None,
    )
    .ok_or_else(|| "expected TypeScript preview finding".to_string())?;

    assert!(matches!(finding.class, ExposureClass::WeaklyExposed));
    assert_eq!(finding.related_tests[0].oracle_kind, OracleKind::Snapshot);
    assert!(finding.canonical_gap.is_none());
    assert_evidence_contains(&finding, "gap_state: advisory");
    assert!(
        finding.missing.iter().any(|line| {
            line.contains("snapshot evidence") && line.contains("add an exact-value assertion")
        }),
        "expected snapshot exact-value guidance, got {:?}",
        finding.missing
    );
    let recommended = finding
        .recommended_next_step
        .as_deref()
        .ok_or_else(|| "expected recommended next step".to_string())?;
    assert!(
        recommended.contains("add an exact-value assertion alongside the snapshot")
            && recommended.contains("no actionable repair packet is emitted"),
        "expected snapshot advisory recommendation, got {recommended:?}"
    );
    Ok(())
}

#[test]
fn typescript_preview_weak_oracle_guidance_names_smoke_exact_value_shape() -> Result<(), String> {
    let finding = classify_weak_direct_line("    return count >= 1;")?;

    assert!(matches!(finding.class, ExposureClass::WeaklyExposed));
    assert_eq!(finding.related_tests[0].oracle_kind, OracleKind::SmokeOnly);
    assert!(finding.canonical_gap.is_none());
    assert_evidence_contains(&finding, "gap_state: advisory");
    assert!(
        finding.missing.iter().any(|line| {
            line.contains("smoke-only oracle") && line.contains("exact-value assertion")
        }),
        "expected smoke-only exact-value guidance, got {:?}",
        finding.missing
    );
    let recommended = finding
        .recommended_next_step
        .as_deref()
        .ok_or_else(|| "expected recommended next step".to_string())?;
    assert!(
        recommended.contains("replace or augment the smoke-only assertion")
            && recommended.contains("no actionable repair packet is emitted"),
        "expected smoke-only advisory recommendation, got {recommended:?}"
    );
    Ok(())
}

#[test]
fn typescript_preview_weak_oracle_guidance_keeps_broad_error_advisory() -> Result<(), String> {
    let owner = test_owner("parseUser", "src/lib.ts");
    let test = direct_test_with_assertion(
        "rejects empty user broadly",
        "expect(() => parseUser('')).toThrow();",
        "toThrow",
        0,
        OracleKind::BroadError,
        OracleStrength::Weak,
    );
    let finding = classify_change(
        Path::new("src/lib.ts"),
        2,
        "    throw new Error(\"empty user\");",
        &[owner],
        &[test],
        None,
        &ReExportIndex::empty(),
        None,
    )
    .ok_or_else(|| "expected TypeScript preview finding".to_string())?;

    assert!(matches!(finding.class, ExposureClass::WeaklyExposed));
    assert_eq!(finding.related_tests[0].oracle_kind, OracleKind::BroadError);
    assert!(finding.canonical_gap.is_none());
    assert_evidence_contains(&finding, "gap_state: advisory");
    assert!(
        finding
            .missing
            .iter()
            .any(|line| line.contains("broad error evidence") && line.contains("keep it weak")),
        "expected broad-error advisory guidance, got {:?}",
        finding.missing
    );
    let recommended = finding
        .recommended_next_step
        .as_deref()
        .ok_or_else(|| "expected recommended next step".to_string())?;
    assert!(
        recommended.contains("broad error evidence does not establish missing discriminator")
            && recommended.contains("no actionable repair packet is emitted"),
        "expected broad-error advisory recommendation, got {recommended:?}"
    );
    assert!(
        !recommended.contains("exact-value assertion"),
        "broad error preview guidance should not ask for an exact-value assertion: {recommended:?}"
    );
    Ok(())
}

#[test]
fn typescript_preview_weak_oracle_guidance_distinguishes_mock_payload_limits() -> Result<(), String>
{
    let owner = test_owner("notifyStatus", "src/lib.ts");
    let test = mock_interaction_test_for("notifyStatus");
    let finding = classify_change(
        Path::new("src/lib.ts"),
        2,
        "    sink.record(status);",
        &[owner],
        &[test],
        None,
        &ReExportIndex::empty(),
        None,
    )
    .ok_or_else(|| "expected TypeScript preview finding".to_string())?;

    assert!(matches!(finding.class, ExposureClass::WeaklyExposed));
    assert_eq!(
        finding.related_tests[0].oracle_kind,
        OracleKind::MockExpectation
    );
    assert_eq!(
        finding.related_tests[0].oracle_strength,
        OracleStrength::Medium
    );
    assert!(finding.canonical_gap.is_none());
    assert_evidence_contains(&finding, "gap_state: advisory");
    assert_evidence_contains(&finding, "actionability_category: incomplete_repair_packet");
    assert!(
            finding.missing.iter().any(|line| line.contains(
                "mock interaction oracle, but TypeScript preview does not yet establish the changed call payload"
            )),
            "expected mock-payload limitation in missing text, got {:?}",
            finding.missing
        );
    let recommended = finding
        .recommended_next_step
        .as_deref()
        .ok_or_else(|| "expected recommended next step".to_string())?;
    assert!(
        recommended.contains("mock payloads are not yet a safe discriminator"),
        "expected mock-payload recommendation, got {recommended:?}"
    );
    assert!(
        !recommended.contains("exact-value assertion"),
        "mock interaction preview guidance should not ask for an exact-value assertion: {recommended:?}"
    );
    Ok(())
}

#[test]
fn typescript_preview_mock_payload_guidance_names_literal_payload_without_repair_packet()
-> Result<(), String> {
    let owner = test_owner("notifyReady", "src/lib.ts");
    let tests = extract_tests(
        Path::new("tests/lib.test.ts"),
        r#"test("records ready status", () => {
    const sink = { record: vi.fn() };
    notifyReady(sink);
    expect(sink.record).toHaveBeenCalledWith("ready");
});
"#,
    );
    let finding = classify_change(
        Path::new("src/lib.ts"),
        2,
        "    sink.record(\"ready\");",
        &[owner],
        &tests,
        None,
        &ReExportIndex::empty(),
        None,
    )
    .ok_or_else(|| "expected TypeScript preview finding".to_string())?;

    assert!(matches!(finding.class, ExposureClass::WeaklyExposed));
    assert_eq!(
        finding.related_tests[0].oracle_kind,
        OracleKind::MockExpectation
    );
    assert_eq!(
        finding.related_tests[0].oracle.as_deref(),
        Some("expect(sink.record).toHaveBeenCalledWith(\"ready\")")
    );
    assert!(finding.canonical_gap.is_none());
    assert_evidence_contains(&finding, "gap_state: advisory");
    assert_evidence_contains(
        &finding,
        "mock_payload_evidence: expect(sink.record).toHaveBeenCalledWith(\"ready\")",
    );
    assert!(
        finding
            .missing
            .iter()
            .any(|line| line.contains("bounded mock payload evidence")
                && line.contains("expect(sink.record).toHaveBeenCalledWith(\"ready\")")),
        "expected bounded mock-payload guidance, got {:?}",
        finding.missing
    );
    let recommended = finding
        .recommended_next_step
        .as_deref()
        .ok_or_else(|| "expected recommended next step".to_string())?;
    assert!(
        recommended.contains("related mock payload evidence")
            && recommended.contains("syntax-bounded")
            && recommended.contains("no actionable repair packet is emitted"),
        "expected advisory mock-payload recommendation, got {recommended:?}"
    );
    assert!(
        !recommended.contains("exact-value assertion"),
        "mock payload preview guidance should not ask for an exact-value assertion: {recommended:?}"
    );
    Ok(())
}

#[test]
fn classify_change_labels_javascript_sources_separately() -> Result<(), String> {
    let owner = TypeScriptOwner {
        name: "applyDiscount".to_string(),
        file: PathBuf::from("src/lib.js"),
        start_line: 1,
        end_line: 5,
        owner_kind: OwnerKind::Function,
        class_name: None,
        decorated: false,
        exported_as_default: false,
        class_default_export: false,
        arity: None,
        parameters: Vec::new(),
        source_text: None,
        imports: Vec::new(),
        method_kind: TypeScriptMethodKind::Ordinary,
    };
    let test = TypeScriptTest {
        name: "alpha".to_string(),
        local_name: "alpha".to_string(),
        describe_names: Vec::new(),
        file: PathBuf::from("tests/lib.test.js"),
        line: 1,
        body_text: "applyDiscount(50, 100)".to_string(),
        assertions: Vec::new(),
        mocks_in_file: Vec::new(),
        imports_in_file: Vec::new(),
    };

    let finding = classify_change(
        Path::new("src/lib.js"),
        2,
        "    if (amount >= threshold) {",
        &[owner],
        &[test],
        None,
        &ReExportIndex::empty(),
        None,
    )
    .ok_or_else(|| "expected a JavaScript preview finding".to_string())?;

    assert_eq!(finding.language, Some(DomainLanguageId::JavaScript));
    assert_eq!(finding.language_status, Some(LanguageStatus::Preview));
    Ok(())
}

#[test]
fn classify_change_matches_owner_file_before_line_range() -> Result<(), String> {
    let owners = vec![
        TypeScriptOwner {
            name: "alphaScore".to_string(),
            file: PathBuf::from("src/a.ts"),
            start_line: 1,
            end_line: 5,
            owner_kind: OwnerKind::Function,
            class_name: None,
            decorated: false,
            exported_as_default: false,
            class_default_export: false,
            arity: None,
            parameters: Vec::new(),
            source_text: None,
            imports: Vec::new(),
            method_kind: TypeScriptMethodKind::Ordinary,
        },
        TypeScriptOwner {
            name: "betaScore".to_string(),
            file: PathBuf::from("src/b.ts"),
            start_line: 1,
            end_line: 5,
            owner_kind: OwnerKind::Function,
            class_name: None,
            decorated: false,
            exported_as_default: false,
            class_default_export: false,
            arity: None,
            parameters: Vec::new(),
            source_text: None,
            imports: Vec::new(),
            method_kind: TypeScriptMethodKind::Ordinary,
        },
    ];
    let tests = vec![
        TypeScriptTest {
            name: "alpha keeps its threshold".to_string(),
            local_name: "alpha keeps its threshold".to_string(),
            describe_names: Vec::new(),
            file: PathBuf::from("tests/a.test.ts"),
            line: 1,
            body_text: "expect(alphaScore(12)).toBe(13);".to_string(),
            assertions: Vec::new(),
            mocks_in_file: Vec::new(),
            imports_in_file: Vec::new(),
        },
        TypeScriptTest {
            name: "beta keeps its threshold".to_string(),
            local_name: "beta keeps its threshold".to_string(),
            describe_names: Vec::new(),
            file: PathBuf::from("tests/b.test.ts"),
            line: 1,
            body_text: "expect(betaScore(12)).toBe(13);".to_string(),
            assertions: Vec::new(),
            mocks_in_file: Vec::new(),
            imports_in_file: Vec::new(),
        },
    ];

    let finding = classify_change(
        Path::new("src/b.ts"),
        2,
        "    if (value >= 10) {",
        &owners,
        &tests,
        None,
        &ReExportIndex::empty(),
        None,
    )
    .ok_or_else(|| "expected the changed file's owner to be selected".to_string())?;

    assert!(
        finding
            .evidence
            .iter()
            .any(|entry| entry == "owner: betaScore")
    );
    assert_eq!(finding.related_tests.len(), 1);
    assert_eq!(finding.related_tests[0].name, "beta keeps its threshold");
    assert_eq!(
        finding.related_tests[0].file,
        PathBuf::from("tests/b.test.ts")
    );
    assert!(finding.missing.iter().all(|line| !line.contains("alpha")));
    Ok(())
}

#[test]
fn extract_tests_collects_expect_to_be_as_strong_oracle() {
    let tests = extract_tests(
        Path::new("tests/lib.test.ts"),
        r#"test("alpha", () => {
    expect(applyDiscount(50, 100)).toBe(50);
    expect(applyDiscount(10000, 100)).toEqual(9990);
});
"#,
    );
    assert_eq!(tests.len(), 1);
    assert_eq!(tests[0].assertions.len(), 2);
    assert_eq!(tests[0].assertions[0].matcher, "toBe");
    assert_eq!(tests[0].assertions[0].oracle_kind, OracleKind::ExactValue);
    assert_eq!(
        tests[0].assertions[0].oracle_strength,
        OracleStrength::Strong
    );
    assert_eq!(tests[0].assertions[1].matcher, "toEqual");
}

#[test]
fn extract_tests_recurses_nested_describe_blocks() {
    let tests = extract_tests(
        Path::new("tests/lib.test.ts"),
        r#"describe("pricing", () => {
    describe("discounts", () => {
        it("pins threshold", () => {
            expect(applyDiscount(100, 100)).toStrictEqual(90);
        });
    });
});
"#,
    );
    assert_eq!(tests.len(), 1);
    assert_eq!(tests[0].name, "pricing discounts pins threshold");
    assert_eq!(tests[0].assertions.len(), 1);
    assert_eq!(tests[0].assertions[0].matcher, "toStrictEqual");
    assert_eq!(tests[0].assertions[0].oracle_kind, OracleKind::ExactValue);
}

#[test]
fn extract_tests_recognizes_test_each_table_calls() {
    let tests = extract_tests(
        Path::new("tests/lib.test.ts"),
        r#"test.each([
    [100, 100, 90],
    [150, 100, 140],
])("discounts %#", (amount, threshold, expected) => {
    expect(applyDiscount(amount, threshold)).toBe(expected);
});
"#,
    );
    assert_eq!(tests.len(), 1);
    assert_eq!(tests[0].name, "discounts %#");
    assert_eq!(tests[0].assertions.len(), 1);
    assert_eq!(tests[0].assertions[0].matcher, "toBe");
    assert!(tests[0].body_text.contains("applyDiscount("));
}

#[test]
fn extract_tests_recognizes_it_each_table_calls() {
    let tests = extract_tests(
        Path::new("tests/lib.test.ts"),
        r#"it.each([
    ["ready"],
])("notifies %s", (status) => {
    const sink = { record: vi.fn() };
    notifyStatus(status, sink);
    expect(sink.record).toHaveBeenCalledWith(status);
});
"#,
    );
    assert_eq!(tests.len(), 1);
    assert_eq!(tests[0].name, "notifies %s");
    assert_eq!(tests[0].assertions.len(), 1);
    assert_eq!(tests[0].assertions[0].matcher, "toHaveBeenCalledWith");
    assert_eq!(
        tests[0].assertions[0].oracle_kind,
        OracleKind::MockExpectation
    );
}

#[test]
fn extract_tests_records_safe_mock_payload_shapes() {
    let tests = extract_tests(
        Path::new("tests/lib.test.ts"),
        r#"test("mock payloads", () => {
    const sink = { record: vi.fn() };
    expect(sink.record).toHaveBeenCalledWith("ready");
    expect(sink.record).toHaveBeenCalledWith({ status: "ok" });
    expect(sink.record).toHaveBeenCalledTimes(1);
});
"#,
    );
    assert_eq!(tests.len(), 1);
    let payloads: Vec<Option<String>> = tests[0]
        .assertions
        .iter()
        .map(|assertion| {
            assertion
                .mock_payload
                .as_ref()
                .map(TypeScriptMockPayload::oracle_text)
        })
        .collect();
    assert_eq!(
        payloads,
        vec![
            Some("expect(sink.record).toHaveBeenCalledWith(\"ready\")".to_string()),
            Some("expect(sink.record).toHaveBeenCalledWith({ status: \"ok\" })".to_string()),
            Some("expect(sink.record).toHaveBeenCalledTimes(1)".to_string()),
        ]
    );
}

#[test]
fn extract_tests_keeps_ambiguous_mock_payload_shapes_unbounded() {
    let tests = extract_tests(
        Path::new("tests/lib.test.ts"),
        r#"test("mock payloads", () => {
    expect(sink.record).toHaveBeenCalledWith(status);
    expect(sink.record).toHaveBeenCalledWith({ status });
    expect(sink.record).toHaveBeenCalledWith(...args);
    expect(sink.record).toHaveBeenCalledWith("ready", "extra");
    expect(sink[method]).toHaveBeenCalledWith("ready");
    expect(getSink()).toHaveBeenCalledTimes(1);
});
"#,
    );
    assert_eq!(tests.len(), 1);
    assert_eq!(tests[0].assertions.len(), 6);
    assert!(
        tests[0]
            .assertions
            .iter()
            .all(|assertion| assertion.mock_payload.is_none()),
        "ambiguous mock payloads must stay unbounded: {:?}",
        tests[0].assertions
    );
}

#[test]
fn extract_tests_recognizes_resolves_async_chain() {
    let tests = extract_tests(
        Path::new("tests/lib.test.ts"),
        r#"test("async", async () => {
    await expect(loader()).resolves.toBe(42);
});
"#,
    );
    assert_eq!(tests.len(), 1);
    assert_eq!(tests[0].assertions.len(), 1);
    assert_eq!(tests[0].assertions[0].matcher, "toBe");
    assert_eq!(tests[0].assertions[0].oracle_kind, OracleKind::ExactValue);
}

#[test]
fn extract_tests_recognizes_return_await_resolves_async_chain() {
    let tests = extract_tests(
        Path::new("tests/lib.test.ts"),
        r#"test("async return", async () => {
    return await expect(loader()).resolves.toBe(42);
});
"#,
    );
    assert_eq!(tests.len(), 1);
    assert_eq!(tests[0].assertions.len(), 1);
    assert_eq!(tests[0].assertions[0].matcher, "toBe");
    assert_eq!(tests[0].assertions[0].oracle_kind, OracleKind::ExactValue);
}

#[test]
fn extract_tests_collects_assertions_nested_in_control_flow() {
    let tests = extract_tests(
        Path::new("tests/lib.test.ts"),
        r#"test("nested", () => {
    if (enabled) {
        expect(applyDiscount(50, 100)).toBe(50);
    } else {
        expect(applyDiscount(1, 100)).toEqual(1);
    }
});
"#,
    );
    assert_eq!(tests.len(), 1);
    assert_eq!(tests[0].assertions.len(), 2);
    assert_eq!(tests[0].assertions[0].matcher, "toBe");
    assert_eq!(tests[0].assertions[1].matcher, "toEqual");
}

#[test]
fn extract_tests_collects_assertions_nested_in_loop_switch_and_label_bodies() {
    let tests = extract_tests(
        Path::new("tests/lib.test.ts"),
        r#"test("nested statements", () => {
    while (enabled) {
        expect(loopValue).toBe(1);
    }
    do {
        expect(done).toBeTruthy();
    } while (retry);
    for (let index = 0; index < items.length; index++) {
        expect(items[index]).toBeDefined();
    }
    for (const key in record) {
        expect(record[key]).toEqual("value");
    }
    for (const item of items) {
        expect(item).toBeDefined();
    }
    retry: {
        expect(labelled).toBe(false);
    }
    switch (kind) {
        case "a":
            expect(kind).toBe("a");
            break;
        default:
            expect(kind).toEqual("fallback");
    }
});
"#,
    );
    assert_eq!(tests.len(), 1);
    let matchers: Vec<&str> = tests[0]
        .assertions
        .iter()
        .map(|assertion| assertion.matcher.as_str())
        .collect();
    assert_eq!(
        matchers,
        vec![
            "toBe",
            "toBeTruthy",
            "toBeDefined",
            "toEqual",
            "toBeDefined",
            "toBe",
            "toBe",
            "toEqual"
        ]
    );
}

#[test]
fn extract_tests_collects_assertions_nested_in_try_catch_finally() {
    let tests = extract_tests(
        Path::new("tests/lib.test.ts"),
        r#"test("try-catch", () => {
    try {
        expect(parseUser("Ada")).toEqual({ name: "Ada" });
    } catch (err) {
        expect(err).toBeDefined();
    } finally {
        expect(cleanup).toHaveBeenCalled();
    }
});
"#,
    );
    assert_eq!(tests.len(), 1);
    assert_eq!(tests[0].assertions.len(), 3);
    assert_eq!(tests[0].assertions[0].matcher, "toEqual");
    assert_eq!(tests[0].assertions[1].matcher, "toBeDefined");
    assert_eq!(tests[0].assertions[2].matcher, "toHaveBeenCalled");
}

#[test]
fn extract_tests_unknown_matcher_maps_to_unknown_oracle() {
    let tests = extract_tests(
        Path::new("tests/lib.test.ts"),
        r#"test("alpha", () => {
    expect(applyDiscount(50, 100)).customDomainMatcher(50);
});
"#,
    );
    assert_eq!(tests.len(), 1);
    assert_eq!(tests[0].assertions.len(), 1);
    assert_eq!(tests[0].assertions[0].oracle_kind, OracleKind::Unknown);
    assert_eq!(
        tests[0].assertions[0].oracle_strength,
        OracleStrength::Unknown
    );
}

#[test]
fn extract_tests_maps_bare_tothrow_to_broad_error_oracle() {
    let tests = extract_tests(
        Path::new("tests/lib.test.ts"),
        r#"test("throws", () => {
    expect(() => parseUser("")).toThrow();
});
"#,
    );
    assert_eq!(tests.len(), 1);
    assert_eq!(tests[0].assertions.len(), 1);
    assert_eq!(tests[0].assertions[0].matcher, "toThrow");
    assert_eq!(tests[0].assertions[0].argument_count, 0);
    assert_eq!(tests[0].assertions[0].oracle_kind, OracleKind::BroadError);
    assert_eq!(tests[0].assertions[0].oracle_strength, OracleStrength::Weak);
}

#[test]
fn extract_tests_maps_literal_tothrow_to_exact_error_variant_oracle() {
    let tests = extract_tests(
        Path::new("tests/lib.test.ts"),
        r#"test("throws", () => {
    expect(() => parseUser("")).toThrow("empty user");
});
"#,
    );
    assert_eq!(tests.len(), 1);
    assert_eq!(tests[0].assertions.len(), 1);
    assert_eq!(tests[0].assertions[0].matcher, "toThrow");
    assert_eq!(tests[0].assertions[0].argument_count, 1);
    assert_eq!(
        tests[0].assertions[0].oracle_kind,
        OracleKind::ExactErrorVariant
    );
    assert_eq!(
        tests[0].assertions[0].oracle_strength,
        OracleStrength::Strong
    );
    assert_eq!(
        tests[0].assertions[0]
            .error_payload
            .as_ref()
            .map(TypeScriptErrorPayload::oracle_text)
            .as_deref(),
        Some("expect(...).toThrow(\"empty user\")")
    );
}

#[test]
fn extract_tests_keeps_dynamic_tothrow_payload_broad() {
    let tests = extract_tests(
        Path::new("tests/lib.test.ts"),
        r#"test("throws", () => {
    expect(() => parseUser("")).toThrow(message);
});
"#,
    );
    assert_eq!(tests.len(), 1);
    assert_eq!(tests[0].assertions.len(), 1);
    assert_eq!(tests[0].assertions[0].matcher, "toThrow");
    assert_eq!(tests[0].assertions[0].argument_count, 1);
    assert_eq!(tests[0].assertions[0].oracle_kind, OracleKind::BroadError);
    assert_eq!(tests[0].assertions[0].oracle_strength, OracleStrength::Weak);
    assert!(tests[0].assertions[0].error_payload.is_none());
}

#[test]
fn extract_tests_maps_rejects_tothrow_literal_to_exact_error_variant_oracle() {
    let tests = extract_tests(
        Path::new("tests/lib.test.ts"),
        r#"test("rejects", async () => {
    await expect(loadProfile("")).rejects.toThrow("missing id");
});
"#,
    );
    assert_eq!(tests.len(), 1);
    assert_eq!(tests[0].assertions.len(), 1);
    assert_eq!(tests[0].assertions[0].matcher, "toThrow");
    assert_eq!(
        tests[0].assertions[0].oracle_kind,
        OracleKind::ExactErrorVariant
    );
    assert_eq!(
        tests[0].assertions[0].oracle_strength,
        OracleStrength::Strong
    );
    assert_eq!(
        tests[0].assertions[0]
            .error_payload
            .as_ref()
            .map(TypeScriptErrorPayload::oracle_text)
            .as_deref(),
        Some("await expect(...).rejects.toThrow(\"missing id\")")
    );
}

#[test]
fn extract_tests_maps_rejects_match_object_literal_to_exact_error_variant_oracle() {
    let tests = extract_tests(
        Path::new("tests/lib.test.ts"),
        r#"test("rejects", async () => {
    await expect(loadProfile("")).rejects.toMatchObject({ code: "E_AUTH" });
});
"#,
    );
    assert_eq!(tests.len(), 1);
    assert_eq!(tests[0].assertions.len(), 1);
    assert_eq!(tests[0].assertions[0].matcher, "toMatchObject");
    assert_eq!(
        tests[0].assertions[0].oracle_kind,
        OracleKind::ExactErrorVariant
    );
    assert_eq!(
        tests[0].assertions[0].oracle_strength,
        OracleStrength::Strong
    );
    assert_eq!(
        tests[0].assertions[0]
            .error_payload
            .as_ref()
            .map(TypeScriptErrorPayload::oracle_text)
            .as_deref(),
        Some("await expect(...).rejects.toMatchObject({ code: \"E_AUTH\" })")
    );
}

#[test]
fn extract_tests_keeps_dynamic_rejects_match_object_unbounded() {
    let tests = extract_tests(
        Path::new("tests/lib.test.ts"),
        r#"test("rejects", async () => {
    await expect(loadProfile("")).rejects.toMatchObject({ code });
});
"#,
    );
    assert_eq!(tests.len(), 1);
    assert_eq!(tests[0].assertions.len(), 1);
    assert_eq!(tests[0].assertions[0].matcher, "toMatchObject");
    assert_eq!(tests[0].assertions[0].oracle_kind, OracleKind::Unknown);
    assert_eq!(
        tests[0].assertions[0].oracle_strength,
        OracleStrength::Unknown
    );
    assert!(tests[0].assertions[0].error_payload.is_none());
}

// --- toThrow exact-payload oracle upgrade tests (RIPR-SPEC-0097) ---

#[test]
fn extract_tests_maps_object_tothrow_to_exact_error_variant_oracle() {
    let tests = extract_tests(
        Path::new("tests/lib.test.ts"),
        r#"test("throws with code", () => {
    expect(() => parseUser("")).toThrow({ code: "ENOENT", message: "not found" });
});
"#,
    );
    assert_eq!(tests.len(), 1);
    assert_eq!(tests[0].assertions.len(), 1);
    assert_eq!(tests[0].assertions[0].matcher, "toThrow");
    assert_eq!(tests[0].assertions[0].argument_count, 1);
    assert_eq!(
        tests[0].assertions[0].oracle_kind,
        OracleKind::ExactErrorVariant
    );
    assert_eq!(
        tests[0].assertions[0].oracle_strength,
        OracleStrength::Strong
    );
    assert_eq!(
        tests[0].assertions[0]
            .error_payload
            .as_ref()
            .map(TypeScriptErrorPayload::oracle_text)
            .as_deref(),
        Some("expect(...).toThrow({ code: \"ENOENT\", message: \"not found\" })")
    );
}

#[test]
fn extract_tests_maps_class_tothrow_to_exact_error_variant_oracle() {
    let tests = extract_tests(
        Path::new("tests/lib.test.ts"),
        r#"test("throws class", () => {
    expect(() => parseUser("")).toThrow(TypeError);
});
"#,
    );
    assert_eq!(tests.len(), 1);
    assert_eq!(tests[0].assertions.len(), 1);
    assert_eq!(tests[0].assertions[0].matcher, "toThrow");
    assert_eq!(tests[0].assertions[0].argument_count, 1);
    assert_eq!(
        tests[0].assertions[0].oracle_kind,
        OracleKind::ExactErrorVariant
    );
    assert_eq!(
        tests[0].assertions[0].oracle_strength,
        OracleStrength::Strong
    );
    assert_eq!(
        tests[0].assertions[0]
            .error_payload
            .as_ref()
            .map(TypeScriptErrorPayload::oracle_text)
            .as_deref(),
        Some("expect(...).toThrow(TypeError)")
    );
}

#[test]
fn extract_tests_maps_dotted_class_tothrow_to_exact_error_variant_oracle() {
    let tests = extract_tests(
        Path::new("tests/lib.test.ts"),
        r#"test("throws namespaced class", () => {
    expect(() => parseUser("")).toThrow(Errors.NotFoundError);
});
"#,
    );
    assert_eq!(tests.len(), 1);
    assert_eq!(tests[0].assertions.len(), 1);
    assert_eq!(
        tests[0].assertions[0].oracle_kind,
        OracleKind::ExactErrorVariant
    );
    assert_eq!(
        tests[0].assertions[0].oracle_strength,
        OracleStrength::Strong
    );
    assert_eq!(
        tests[0].assertions[0]
            .error_payload
            .as_ref()
            .map(TypeScriptErrorPayload::oracle_text)
            .as_deref(),
        Some("expect(...).toThrow(Errors.NotFoundError)")
    );
}

#[test]
fn extract_tests_keeps_lowercase_ident_tothrow_broad() {
    // Control: lowercase-first identifier cannot be confirmed as a class ref.
    // Fail-closed: stays BroadError / Weak.
    let tests = extract_tests(
        Path::new("tests/lib.test.ts"),
        r#"test("throws", () => {
    expect(() => parseUser("")).toThrow(message);
});
"#,
    );
    assert_eq!(tests.len(), 1);
    assert_eq!(tests[0].assertions.len(), 1);
    assert_eq!(tests[0].assertions[0].matcher, "toThrow");
    assert_eq!(tests[0].assertions[0].argument_count, 1);
    assert_eq!(tests[0].assertions[0].oracle_kind, OracleKind::BroadError);
    assert_eq!(tests[0].assertions[0].oracle_strength, OracleStrength::Weak);
    assert!(tests[0].assertions[0].error_payload.is_none());
}

#[test]
fn extract_tests_keeps_dynamic_object_tothrow_broad() {
    // Control: object with shorthand (non-literal) value stays broad.
    let tests = extract_tests(
        Path::new("tests/lib.test.ts"),
        r#"test("throws dynamic", () => {
    expect(() => parseUser("")).toThrow({ code });
});
"#,
    );
    assert_eq!(tests.len(), 1);
    assert_eq!(tests[0].assertions.len(), 1);
    assert_eq!(tests[0].assertions[0].matcher, "toThrow");
    assert_eq!(tests[0].assertions[0].argument_count, 1);
    assert_eq!(tests[0].assertions[0].oracle_kind, OracleKind::BroadError);
    assert_eq!(tests[0].assertions[0].oracle_strength, OracleStrength::Weak);
    assert!(tests[0].assertions[0].error_payload.is_none());
}

#[test]
fn extract_tests_maps_class_rejects_tothrow_to_exact_error_variant_oracle() {
    let tests = extract_tests(
        Path::new("tests/lib.test.ts"),
        r#"test("async throws class", async () => {
    await expect(loadProfile("")).rejects.toThrow(AuthError);
});
"#,
    );
    assert_eq!(tests.len(), 1);
    assert_eq!(tests[0].assertions.len(), 1);
    assert_eq!(tests[0].assertions[0].matcher, "toThrow");
    assert_eq!(
        tests[0].assertions[0].oracle_kind,
        OracleKind::ExactErrorVariant
    );
    assert_eq!(
        tests[0].assertions[0].oracle_strength,
        OracleStrength::Strong
    );
    assert_eq!(
        tests[0].assertions[0]
            .error_payload
            .as_ref()
            .map(TypeScriptErrorPayload::oracle_text)
            .as_deref(),
        Some("await expect(...).rejects.toThrow(AuthError)")
    );
}

#[test]
fn extract_tests_maps_object_rejects_tothrow_to_exact_error_variant_oracle() {
    let tests = extract_tests(
        Path::new("tests/lib.test.ts"),
        r#"test("async throws with object", async () => {
    await expect(loadProfile("")).rejects.toThrow({ code: "AUTH_FAILED" });
});
"#,
    );
    assert_eq!(tests.len(), 1);
    assert_eq!(tests[0].assertions.len(), 1);
    assert_eq!(tests[0].assertions[0].matcher, "toThrow");
    assert_eq!(
        tests[0].assertions[0].oracle_kind,
        OracleKind::ExactErrorVariant
    );
    assert_eq!(
        tests[0].assertions[0].oracle_strength,
        OracleStrength::Strong
    );
    assert_eq!(
        tests[0].assertions[0]
            .error_payload
            .as_ref()
            .map(TypeScriptErrorPayload::oracle_text)
            .as_deref(),
        Some("await expect(...).rejects.toThrow({ code: \"AUTH_FAILED\" })")
    );
}

#[test]
fn oracle_for_matcher_covers_canonical_jest_vitest_set() {
    assert_eq!(
        oracle_for_matcher("toBe"),
        (OracleKind::ExactValue, OracleStrength::Strong)
    );
    assert_eq!(
        oracle_for_matcher("toEqual"),
        (OracleKind::ExactValue, OracleStrength::Strong)
    );
    assert_eq!(
        oracle_for_matcher("toThrow"),
        (OracleKind::BroadError, OracleStrength::Weak)
    );
    assert_eq!(
        oracle_for_matcher("toMatchSnapshot"),
        (OracleKind::Snapshot, OracleStrength::Medium)
    );
    assert_eq!(
        oracle_for_matcher("toHaveBeenCalledWith"),
        (OracleKind::MockExpectation, OracleStrength::Medium)
    );
    assert_eq!(
        oracle_for_matcher("toBeTruthy"),
        (OracleKind::SmokeOnly, OracleStrength::Smoke)
    );
    assert_eq!(
        oracle_for_matcher("toContain"),
        (OracleKind::RelationalCheck, OracleStrength::Weak)
    );
    assert_eq!(
        oracle_for_matcher("someUnknownMatcher"),
        (OracleKind::Unknown, OracleStrength::Unknown)
    );
}

#[test]
fn classify_change_returns_exposed_when_related_test_has_strong_oracle() -> Result<(), String> {
    let owner = TypeScriptOwner {
        name: "applyDiscount".to_string(),
        file: PathBuf::from("src/lib.ts"),
        start_line: 1,
        end_line: 5,
        owner_kind: OwnerKind::Function,
        class_name: None,
        decorated: false,
        exported_as_default: false,
        class_default_export: false,
        arity: None,
        parameters: Vec::new(),
        source_text: None,
        imports: Vec::new(),
        method_kind: TypeScriptMethodKind::Ordinary,
    };
    let test = TypeScriptTest {
        name: "alpha".to_string(),
        local_name: "alpha".to_string(),
        describe_names: Vec::new(),
        file: PathBuf::from("tests/lib.test.ts"),
        line: 1,
        // RIPR-SPEC-0027: the observed call sits at the changed boundary
        // (`amount == threshold`), so the strong oracle witnesses it.
        body_text: "expect(applyDiscount(100, 100)).toBe(90)".to_string(),
        assertions: vec![TypeScriptAssertion {
            matcher: "toBe".to_string(),
            argument_count: 1,
            line: 2,
            oracle_kind: OracleKind::ExactValue,
            oracle_strength: OracleStrength::Strong,
            mock_payload: None,
            error_payload: None,
            observed_expression: Some("applyDiscount(100, 100)".to_string()),
            expected_value_or_variant: None,
            has_dynamic_matcher_arg: false,
            oracle_confidence: OracleConfidence::Medium,
        }],
        mocks_in_file: Vec::new(),
        imports_in_file: Vec::new(),
    };
    let finding = classify_change(
        Path::new("src/lib.ts"),
        2,
        "    if (amount >= threshold) {",
        &[owner],
        &[test],
        None,
        &ReExportIndex::empty(),
        None,
    )
    .ok_or_else(|| "expected a finding for the changed line".to_string())?;
    assert!(matches!(finding.class, ExposureClass::Exposed));
    assert_eq!(finding.related_tests.len(), 1);
    assert_eq!(finding.related_tests[0].oracle_kind, OracleKind::ExactValue);
    assert_eq!(
        finding.related_tests[0].oracle_strength,
        OracleStrength::Strong
    );
    assert!(finding.canonical_gap.is_none());
    assert_evidence_contains(&finding, "gap_state: already_observed");
    assert_evidence_contains(&finding, "actionability_category: strong_oracle_observed");
    assert_evidence_contains(
        &finding,
        "why_not_actionable: related Jest/Vitest evidence already has a strong exact oracle",
    );
    Ok(())
}

#[test]
fn classify_change_exposed_t_assertion_uses_execution_context_label() -> Result<(), String> {
    let owner = TypeScriptOwner {
        name: "applyDiscount".to_string(),
        file: PathBuf::from("src/lib.ts"),
        start_line: 1,
        end_line: 5,
        owner_kind: OwnerKind::Function,
        class_name: None,
        decorated: false,
        exported_as_default: false,
        class_default_export: false,
        arity: None,
        parameters: Vec::new(),
        source_text: None,
        imports: Vec::new(),
        method_kind: TypeScriptMethodKind::Ordinary,
    };
    let test = TypeScriptTest {
        name: "alpha".to_string(),
        local_name: "alpha".to_string(),
        describe_names: Vec::new(),
        file: PathBuf::from("tests/lib.test.ts"),
        line: 1,
        // RIPR-SPEC-0027: the observed call sits at the changed boundary.
        body_text: "t.is(applyDiscount(100, 100), 90);".to_string(),
        assertions: vec![TypeScriptAssertion {
            matcher: "is".to_string(),
            argument_count: 2,
            line: 2,
            oracle_kind: OracleKind::ExactValue,
            oracle_strength: OracleStrength::Strong,
            mock_payload: None,
            error_payload: None,
            observed_expression: Some("applyDiscount(100, 100)".to_string()),
            expected_value_or_variant: Some("90".to_string()),
            has_dynamic_matcher_arg: false,
            oracle_confidence: OracleConfidence::High,
        }],
        mocks_in_file: Vec::new(),
        imports_in_file: Vec::new(),
    };
    let finding = classify_change(
        Path::new("src/lib.ts"),
        2,
        "    if (amount >= threshold) {",
        &[owner],
        &[test],
        None,
        &ReExportIndex::empty(),
        None,
    )
    .ok_or_else(|| "expected a finding for the changed line".to_string())?;
    assert!(matches!(finding.class, ExposureClass::Exposed));
    assert_eq!(finding.related_tests.len(), 1);
    assert_eq!(
        finding.related_tests[0].oracle.as_deref(),
        Some("t.is(...)")
    );
    assert_evidence_contains(&finding, "gap_state: already_observed");
    assert_evidence_contains(
        &finding,
        "why_not_actionable: related TypeScript `t.*` evidence already has a strong exact oracle",
    );
    Ok(())
}

#[test]
fn classify_change_returns_no_static_path_when_no_related_test() -> Result<(), String> {
    let owner = TypeScriptOwner {
        name: "applyDiscount".to_string(),
        file: PathBuf::from("src/lib.ts"),
        start_line: 1,
        end_line: 5,
        owner_kind: OwnerKind::Function,
        class_name: None,
        decorated: false,
        exported_as_default: false,
        class_default_export: false,
        arity: None,
        parameters: Vec::new(),
        source_text: None,
        imports: Vec::new(),
        method_kind: TypeScriptMethodKind::Ordinary,
    };
    let finding = classify_change(
        Path::new("src/lib.ts"),
        2,
        "    if (amount >= threshold) {",
        &[owner],
        &[],
        None,
        &ReExportIndex::empty(),
        None,
    )
    .ok_or_else(|| "expected a finding when an owner contains the changed line".to_string())?;
    assert!(matches!(finding.class, ExposureClass::NoStaticPath));
    assert!(finding.related_tests.is_empty());
    assert!(finding.canonical_gap.is_none());
    assert_evidence_contains(&finding, "gap_state: advisory");
    assert_evidence_contains(&finding, "actionability_category: missing_context");
    assert_evidence_contains(&finding, "related_test_or_observer");
    Ok(())
}

#[test]
fn classify_change_returns_none_when_line_is_outside_any_owner() {
    let owner = TypeScriptOwner {
        name: "applyDiscount".to_string(),
        file: PathBuf::from("src/lib.ts"),
        start_line: 10,
        end_line: 20,
        owner_kind: OwnerKind::Function,
        class_name: None,
        decorated: false,
        exported_as_default: false,
        class_default_export: false,
        arity: None,
        parameters: Vec::new(),
        source_text: None,
        imports: Vec::new(),
        method_kind: TypeScriptMethodKind::Ordinary,
    };
    let finding = classify_change(
        Path::new("src/lib.ts"),
        5,
        "// top-level comment",
        &[owner],
        &[],
        None,
        &ReExportIndex::empty(),
        None,
    );
    assert!(finding.is_none());
}

#[test]
fn analyze_diff_returns_zero_findings_and_counts_accepted_files() -> Result<(), String> {
    let adapter = TypeScriptAdapter;
    let options = AnalysisOptions {
        root: PathBuf::from("/nonexistent_workspace"),
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
        changed("src/index.ts"),
        changed("src/lib.rs"),
        changed("docs/README.md"),
        changed("src/Header.tsx"),
    ];
    let result = adapter.analyze_diff(&options, &policy, &changed_files)?;
    // No workspace files on disk -> no findings; counted-file tally
    // still reflects accepted changed paths.
    assert!(result.findings.is_empty());
    assert_eq!(result.changed_files, 2);
    Ok(())
}

#[test]
fn invalid_utf8_source_produces_no_finding_or_is_disclosed() -> Result<(), String> {
    // PINS CURRENT BEHAVIOR (agentic-trust failure-mode audit): a changed
    // TypeScript file whose bytes are not valid UTF-8 hits the
    // `read_to_string` guard in `analyze_diff` and is skipped silently — no
    // finding is emitted and no static limit is disclosed. Lane
    // ts-d-silent-gaps owns adding that disclosure; this test must be
    // updated when the disclosure lands.
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.subsec_nanos())
        .unwrap_or(0);
    let root = std::env::temp_dir().join(format!("ripr-ts-utf8-{stamp}"));
    let _ = fs::create_dir_all(root.join("src"));
    // 0xFE 0xFF is never valid UTF-8.
    let _ = fs::write(
        root.join("src").join("broken.ts"),
        [0xFE_u8, 0xFF, 0x20, 0x3B],
    );

    let adapter = TypeScriptAdapter;
    let options = AnalysisOptions {
        root: root.clone(),
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
    let changed_files = vec![changed("src/broken.ts")];
    let result = adapter.analyze_diff(&options, &policy, &changed_files)?;
    assert!(
        result
            .findings
            .iter()
            .all(|finding| finding.probe.location.file.as_path() != Path::new("src/broken.ts")),
        "invalid UTF-8 source must not produce a finding for src/broken.ts (silently skipped today; disclosure owned by lane ts-d-silent-gaps)"
    );
    Ok(())
}

#[test]
fn analyze_diff_splits_changed_files_into_typescript_and_javascript() -> Result<(), String> {
    // #2103 review: this adapter covers .js/.jsx/.mjs/.cjs as javascript;
    // the summary must not attribute JS files to typescript.
    let adapter = TypeScriptAdapter;
    let options = AnalysisOptions {
        root: PathBuf::from("/nonexistent_workspace"),
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
        changed("src/index.ts"),
        changed("src/app.js"),
        changed("src/Header.jsx"),
        changed("src/module.mts"),
        changed("src/module.cts"),
        changed("src/app.mjs"),
        changed("src/app.cjs"),
        changed("src/lib.rs"),
    ];
    let result = adapter.analyze_diff(&options, &policy, &changed_files)?;
    assert_eq!(result.changed_files, 7);
    assert_eq!(
        result.changed_files_by_language,
        vec![
            (crate::analysis::language::LanguageId::TypeScript, 3),
            (crate::analysis::language::LanguageId::JavaScript, 4),
        ]
    );
    Ok(())
}

/// A `.mts` source file must be discovered, parsed, and analyzed — before
/// the extension-routing lane the router dropped `.mts`/`.cts`/`.mjs`/`.cjs`
/// entirely, so half of a modern ESM/CJS tree was silently unread with no
/// limitation.
///
/// This is the cross-extension end-to-end discriminator: a real temp
/// workspace, the IMPORTED source file actually created on disk, and a test
/// that imports it through the *other* modern suffix (`.mjs` import naming a
/// `.mts` owner). The oracle asserted is the evidence decision — a Strong
/// ExactValue related test credited to the owner — not merely a changed-file
/// count. Removing the new-suffix module stripping in
/// `strip_typescript_module_extension` turns this finding into
/// `NoStaticPath` with zero related tests.
#[test]
fn analyze_diff_credits_cross_extension_related_test_oracle_for_mts_sources() -> Result<(), String>
{
    let root = ts_unique_tempdir("mts-cross-extension")?;

    ts_write_file(
        &root.join("package.json"),
        r#"{"name":"pkg","scripts":{"test":"vitest"},"devDependencies":{"vitest":"^1.0.0"}}"#,
    )?;
    ts_write_file(
        &root.join("src/cart.mts"),
        "export function cartTotal(items: number[]): number {\n  return items.reduce((a, b) => a + b, 0);\n}\n",
    )?;
    // The import names `../src/cart.mjs` while the owner on disk is
    // `src/cart.mts` — only cross-extension module normalization can
    // connect them.
    ts_write_file(
        &root.join("tests/cart.test.mts"),
        "import { cartTotal } from '../src/cart.mjs';\ntest('totals items', () => {\n  const result = cartTotal([1, 2]);\n  expect(result).toBe(3);\n});\n",
    )?;
    let adapter = TypeScriptAdapter;
    let options = AnalysisOptions {
        root: root.clone(),
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
    let changed_files = vec![ChangedFile {
        path: PathBuf::from("src/cart.mts"),
        added_lines: vec![crate::analysis::diff::ChangedLine {
            line: 1,
            new_side_line: 1,
            text: "export function cartTotal(items: number[]): number {".to_string(),
        }],
        removed_lines: Vec::new(),
    }];

    let result = adapter.analyze_diff(&options, &policy, &changed_files);
    let _ = std::fs::remove_dir_all(&root);
    let result = result?;

    assert_eq!(
        result.changed_files, 1,
        "the .mts change must be counted as analyzed"
    );
    let finding = result
        .findings
        .iter()
        .find(|f| f.probe.location.file.as_path() == Path::new("src/cart.mts"))
        .ok_or_else(|| {
            format!(
                "expected a finding for the .mts owner; got {} finding(s)",
                result.findings.len()
            )
        })?;

    assert_eq!(
        finding.language,
        Some(DomainLanguageId::TypeScript),
        "the .mts finding must be attributed to typescript"
    );
    // The evidence decision, not the file count: a `.mjs` import must credit
    // the `.mts` owner a real oracle-bearing related test.
    assert_eq!(
        finding.related_tests.len(),
        1,
        "the cross-extension import must credit exactly one related test; got {:?}",
        finding
            .related_tests
            .iter()
            .map(|t| t.file.clone())
            .collect::<Vec<_>>()
    );
    assert_eq!(
        finding.related_tests[0].oracle_kind,
        OracleKind::ExactValue,
        "the cross-extension related test must carry a real extracted oracle"
    );
    assert_eq!(
        finding.related_tests[0].oracle_strength,
        OracleStrength::Strong,
        "the `toBe(3)` assertion must classify as a Strong oracle"
    );
    assert!(
        !matches!(finding.class, ExposureClass::NoStaticPath),
        "a credited cross-extension Strong oracle must not fall back to no_static_path"
    );
    assert_evidence_contains(finding, "typescript_oracle_expected: 3");
    Ok(())
}

/// CommonJS half of the cross-extension seam: a `.cts` owner imported through
/// a `.cjs` specifier must resolve the same way the ESM half does. This is a
/// separate control, not a duplicate — it fails independently if only the ESM
/// suffixes are wired into module normalization.
#[test]
fn analyze_diff_credits_cross_extension_related_test_oracle_for_cts_sources() -> Result<(), String>
{
    let root = ts_unique_tempdir("cts-cross-extension")?;

    ts_write_file(
        &root.join("package.json"),
        r#"{"name":"pkg","scripts":{"test":"vitest"},"devDependencies":{"vitest":"^1.0.0"}}"#,
    )?;
    ts_write_file(
        &root.join("src/cart.cts"),
        "export function cartTotal(items: number[]): number {\n  return items.reduce((a, b) => a + b, 0);\n}\n",
    )?;
    ts_write_file(
        &root.join("tests/cart.test.cts"),
        "import { cartTotal } from '../src/cart.cjs';\ntest('totals items', () => {\n  const result = cartTotal([1, 2]);\n  expect(result).toBe(3);\n});\n",
    )?;

    let adapter = TypeScriptAdapter;
    let options = AnalysisOptions {
        root: root.clone(),
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
    let changed_files = vec![ChangedFile {
        path: PathBuf::from("src/cart.cts"),
        added_lines: vec![crate::analysis::diff::ChangedLine {
            line: 1,
            new_side_line: 1,
            text: "export function cartTotal(items: number[]): number {".to_string(),
        }],
        removed_lines: Vec::new(),
    }];

    let result = adapter.analyze_diff(&options, &policy, &changed_files);
    let _ = std::fs::remove_dir_all(&root);
    let result = result?;

    let finding = result
        .findings
        .iter()
        .find(|f| f.probe.location.file.as_path() == Path::new("src/cart.cts"))
        .ok_or_else(|| {
            format!(
                "expected a finding for the .cts owner; got {} finding(s)",
                result.findings.len()
            )
        })?;

    assert_eq!(finding.related_tests.len(), 1);
    assert_eq!(
        finding.related_tests[0].oracle_strength,
        OracleStrength::Strong,
        "the .cjs-imported related test must carry a Strong oracle"
    );
    assert!(
        !matches!(finding.class, ExposureClass::NoStaticPath),
        "a credited cross-extension Strong oracle must not fall back to no_static_path"
    );
    Ok(())
}

/// Cross-extension resolution must fail closed rather than invent credit: a
/// `.mjs` test importing a DIFFERENT modern module must not pick up the
/// `.mts` owner's oracle. Without this, a green "related test exists" result
/// would be untrustworthy.
#[test]
fn analyze_diff_does_not_credit_related_test_from_a_different_modern_module() -> Result<(), String>
{
    let root = ts_unique_tempdir("mts-cross-extension-negative")?;

    ts_write_file(
        &root.join("package.json"),
        r#"{"name":"pkg","scripts":{"test":"vitest"},"devDependencies":{"vitest":"^1.0.0"}}"#,
    )?;
    ts_write_file(
        &root.join("src/cart.mts"),
        "export function cartTotal(items: number[]): number {\n  return items.reduce((a, b) => a + b, 0);\n}\n",
    )?;
    // A different modern module, asserting a different owner entirely.
    ts_write_file(
        &root.join("tests/basket.test.mts"),
        "import { basketTotal } from '../src/basket.mjs';\ntest('totals basket', () => {\n  const result = basketTotal([1, 2]);\n  expect(result).toBe(3);\n});\n",
    )?;

    let adapter = TypeScriptAdapter;
    let options = AnalysisOptions {
        root: root.clone(),
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
    let changed_files = vec![ChangedFile {
        path: PathBuf::from("src/cart.mts"),
        added_lines: vec![crate::analysis::diff::ChangedLine {
            line: 1,
            new_side_line: 1,
            text: "export function cartTotal(items: number[]): number {".to_string(),
        }],
        removed_lines: Vec::new(),
    }];

    let result = adapter.analyze_diff(&options, &policy, &changed_files);
    let _ = std::fs::remove_dir_all(&root);
    let result = result?;

    let finding = result
        .findings
        .iter()
        .find(|f| f.probe.location.file.as_path() == Path::new("src/cart.mts"))
        .ok_or_else(|| {
            format!(
                "expected a finding for the .mts owner; got {} finding(s)",
                result.findings.len()
            )
        })?;

    assert!(
        finding.related_tests.is_empty(),
        "an unrelated modern module must not credit a related test to the cart owner; got {:?}",
        finding
            .related_tests
            .iter()
            .map(|t| t.file.clone())
            .collect::<Vec<_>>()
    );
    Ok(())
}

#[test]
fn analyze_diff_surfaces_over_limit_read_as_named_limitation() -> Result<(), String> {
    // A workspace file larger than the 16 MiB default per-file cap must not be
    // silently skipped: it surfaces as a named limitation whose recovery names
    // the env knob (bounded_read.rs contract).
    let root = ts_unique_tempdir("capped-read")?;
    ts_write_file(&root.join("src/ok.ts"), "export const ok = 1;\n")?;
    let over_limit = vec![b'x'; (DEFAULT_TS_MAX_FILE_READ_BYTES + 1) as usize];
    std::fs::write(root.join("src/huge.ts"), over_limit)
        .map_err(|err| format!("write huge fixture: {err}"))?;

    let adapter = TypeScriptAdapter;
    let options = AnalysisOptions {
        root: root.clone(),
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
    let result = adapter.analyze_diff(&options, &OraclePolicy::default(), &[]);
    let _ = std::fs::remove_dir_all(&root);
    let result = result?;

    let capped = result.limitations.iter().find(|limitation| {
        limitation
            .bounded_detail
            .as_deref()
            .is_some_and(|detail| detail.contains("file_read_capped"))
    });
    let capped = capped.ok_or_else(|| {
        format!(
            "expected a named limitation for the over-limit file; got {:?}",
            result
                .limitations
                .iter()
                .map(|limitation| limitation.bounded_detail.clone())
                .collect::<Vec<_>>()
        )
    })?;
    assert_eq!(capped.path.as_deref(), Some("src/huge.ts"));
    assert!(matches!(
        capped.kind,
        AnalysisLimitationKind::LanguageScopeUnsupported
    ));
    assert!(
        matches!(
            capped.recovery.kind,
            AnalysisRecoveryKind::IncreaseConfiguredLimit
        ),
        "recovery must name the configured limit, got {:?}",
        capped.recovery.kind
    );
    assert!(
        capped
            .recovery
            .detail
            .contains("RIPR_TS_MAX_FILE_READ_BYTES"),
        "recovery must name the env knob: {}",
        capped.recovery.detail
    );
    Ok(())
}

#[test]
fn analyze_diff_does_not_count_excluded_or_generated_typescript_files() -> Result<(), String> {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or(0);
    let root = std::env::temp_dir().join(format!(
        "ripr-ts-excluded-diff-{}-{stamp}",
        std::process::id()
    ));
    let source = "export function shippingFee(total: number) {\n  return total > 50 ? 0 : 5;\n}\n";
    for rel in [
        "src/ok.ts",
        "node_modules/pkg/index.ts",
        "dist/bundle.js",
        "vendor/lib.ts",
        "build/out.js",
        "coverage/report.ts",
        "src/client.generated.ts",
    ] {
        let path = root.join(rel);
        let parent = path
            .parent()
            .ok_or_else(|| format!("missing parent for {rel}"))?;
        std::fs::create_dir_all(parent)
            .map_err(|err| format!("create {}: {err}", parent.display()))?;
        std::fs::write(&path, source).map_err(|err| format!("write {}: {err}", path.display()))?;
    }
    let adapter = TypeScriptAdapter;
    let options = AnalysisOptions {
        root: root.clone(),
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
    let changed_line = crate::analysis::diff::ChangedLine {
        line: 2,
        new_side_line: 2,
        text: "  return total >= 50 ? 0 : 5;".to_string(),
    };
    let changed_files = [
        "src/ok.ts",
        "node_modules/pkg/index.ts",
        "dist/bundle.js",
        "vendor/lib.ts",
        "build/out.js",
        "coverage/report.ts",
        "src/client.generated.ts",
    ]
    .into_iter()
    .map(|path| ChangedFile {
        path: PathBuf::from(path),
        added_lines: vec![changed_line.clone()],
        removed_lines: Vec::new(),
    })
    .collect::<Vec<_>>();
    let result = adapter.analyze_diff(&options, &OraclePolicy::default(), &changed_files);
    let cleanup = std::fs::remove_dir_all(&root);
    let result = result?;
    cleanup.map_err(|err| format!("remove temp workspace: {err}"))?;

    if result.changed_files != 1 {
        return Err(format!(
            "expected only src/ok.ts to count, got {} (by language {:?})",
            result.changed_files, result.changed_files_by_language
        ));
    }
    let excluded_finding = result.findings.iter().any(|finding| {
        finding.probe.location.file.components().any(|component| {
            matches!(
                component.as_os_str().to_str(),
                Some("node_modules" | "dist" | "vendor" | "build" | "coverage" | "__generated__")
            )
        }) || finding
            .probe
            .location
            .file
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.contains(".generated."))
    });
    if excluded_finding {
        return Err(format!(
            "excluded or generated change produced a finding: {:?}",
            result
                .findings
                .iter()
                .map(|finding| finding.probe.location.file.display().to_string())
                .collect::<Vec<_>>()
        ));
    }
    if !result
        .findings
        .iter()
        .any(|finding| finding.probe.location.file == Path::new("src/ok.ts"))
    {
        return Err(format!(
            "the src/ok.ts control change produced no finding; files were {:?}",
            result
                .findings
                .iter()
                .map(|finding| finding.probe.location.file.display().to_string())
                .collect::<Vec<_>>()
        ));
    }
    Ok(())
}

#[test]
fn analyze_diff_surfaces_over_limit_tsconfig_read_as_named_limitation() -> Result<(), String> {
    // An over-limit tsconfig.json must fail-close the alias map AND surface a
    // named limitation naming the env knob, so the missing alias resolution is
    // not silent (bounded_read.rs disclosure contract).
    let root = ts_unique_tempdir("capped-tsconfig-read")?;
    ts_write_file(&root.join("src/ok.ts"), "export const ok = 1;\n")?;
    let over_limit = vec![b'{'; (DEFAULT_TS_MAX_FILE_READ_BYTES + 1) as usize];
    std::fs::write(root.join("tsconfig.json"), over_limit)
        .map_err(|err| format!("write over-limit tsconfig fixture: {err}"))?;

    let adapter = TypeScriptAdapter;
    let options = AnalysisOptions {
        root: root.clone(),
        base: None,
        diff_file: None,
        mode: crate::analysis::AnalysisMode::Draft,
        include_unchanged_tests: false,
        resolve_tsconfig_paths: true,
        perl_facts_path: None,
        git_timeout: None,
        git_candidate: None,
        production_like_targets: Default::default(),
        test_harnesses: Vec::new(),
        resolved_subject_identity: None,
    };
    let result = adapter.analyze_diff(&options, &OraclePolicy::default(), &[]);
    let _ = std::fs::remove_dir_all(&root);
    let result = result?;

    let capped = result.limitations.iter().find(|limitation| {
        limitation
            .bounded_detail
            .as_deref()
            .is_some_and(|detail| detail.contains("file_read_capped"))
    });
    let capped = capped.ok_or_else(|| {
        format!(
            "expected a named limitation for the over-limit tsconfig; got {:?}",
            result
                .limitations
                .iter()
                .map(|limitation| limitation.bounded_detail.clone())
                .collect::<Vec<_>>()
        )
    })?;
    assert!(
        capped
            .path
            .as_deref()
            .is_some_and(|path| path.ends_with("tsconfig.json")),
        "limitation path must name tsconfig.json, got {:?}",
        capped.path
    );
    assert!(
        matches!(
            capped.recovery.kind,
            AnalysisRecoveryKind::IncreaseConfiguredLimit
        ),
        "recovery must name the configured limit, got {:?}",
        capped.recovery.kind
    );
    assert!(
        capped
            .recovery
            .detail
            .contains("RIPR_TS_MAX_FILE_READ_BYTES"),
        "recovery must name the env knob: {}",
        capped.recovery.detail
    );
    Ok(())
}

#[test]
fn analyze_diff_surfaces_absolute_base_url_as_named_limitation() -> Result<(), String> {
    // An absolute compilerOptions.baseUrl cannot be anchored to the
    // workspace root by single-hop resolution. Alias lookup must fail
    // closed AND surface the named limitation
    // `typescript_base_url_absolute_unsupported` instead of silently
    // trimming the leading slash into a wrong in-root path.
    let root = ts_unique_tempdir("absolute-base-url")?;
    ts_write_file(&root.join("src/ok.ts"), "export const ok = 1;\n")?;
    ts_write_file(
        &root.join("tsconfig.json"),
        r#"{"compilerOptions":{"baseUrl":"/abs/base","paths":{"@/*":["src/*"]}}}"#,
    )?;

    let adapter = TypeScriptAdapter;
    let options = AnalysisOptions {
        root: root.clone(),
        base: None,
        diff_file: None,
        mode: crate::analysis::AnalysisMode::Draft,
        include_unchanged_tests: false,
        resolve_tsconfig_paths: true,
        perl_facts_path: None,
        git_timeout: None,
        git_candidate: None,
        production_like_targets: Default::default(),
        test_harnesses: Vec::new(),
        resolved_subject_identity: None,
    };
    let result = adapter.analyze_diff(&options, &OraclePolicy::default(), &[]);
    let _ = std::fs::remove_dir_all(&root);
    let result = result?;

    let limited = result.limitations.iter().find(|limitation| {
        limitation
            .bounded_detail
            .as_deref()
            .is_some_and(|detail| detail.contains("typescript_base_url_absolute_unsupported"))
    });
    let limited = limited.ok_or_else(|| {
        format!(
            "expected the absolute-baseUrl named limitation; got {:?}",
            result
                .limitations
                .iter()
                .map(|limitation| limitation.bounded_detail.clone())
                .collect::<Vec<_>>()
        )
    })?;
    assert_eq!(limited.path.as_deref(), Some("tsconfig.json"));
    assert!(matches!(
        limited.kind,
        AnalysisLimitationKind::LanguageScopeUnsupported
    ));
    Ok(())
}

#[test]
fn analyze_repo_discloses_partial_run_instead_of_silent_empty() -> Result<(), String> {
    let adapter = TypeScriptAdapter;
    let options = AnalysisOptions {
        root: PathBuf::from("/nonexistent_workspace"),
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
    };
    let policy = OraclePolicy::default();
    let result = adapter.analyze_repo(&options, &policy)?;
    assert!(result.findings.is_empty());
    assert_eq!(result.production_files, 0);
    // The stub discloses the partial run on the shared channel (the
    // pipeline records a `Partial` language run from `partial_reason`),
    // so repo-mode output is not a silently clean result.
    assert_eq!(
        result.partial_reason.as_deref(),
        Some("typescript_repo_mode_not_implemented_diff_first")
    );
    Ok(())
}

/// Two identical added lines in the same owner collide on the
/// content-addressed probe id (path/family/owner/normalized expression,
/// no line number — classifier.rs). The adapter's post-hoc ordinal pass
/// must keep both findings distinct: the first keeps its id, the second
/// gets the `.2` suffix (mirror of the Rust path's `dedup_probe_ids`),
/// so the packet projection's `finding.id` dedupe fingerprint cannot
/// collapse two distinct changed lines into one.
#[test]
fn analyze_diff_dedups_colliding_probe_ids_for_identical_added_lines() -> Result<(), String> {
    let root = ts_unique_tempdir("probe-dedup")?;

    // Two identical `if (x > 0) {` guards in one owner function.
    ts_write_file(
        &root.join("src/lib.ts"),
        "export function classify(x: number): number {\n  if (x > 0) {\n    return 1;\n  }\n  if (x > 0) {\n    return 2;\n  }\n  return 0;\n}\n",
    )?;

    let adapter = TypeScriptAdapter;
    let options = AnalysisOptions {
        root: root.clone(),
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
    let changed_files = vec![ChangedFile {
        path: PathBuf::from("src/lib.ts"),
        added_lines: vec![
            crate::analysis::diff::ChangedLine {
                line: 2,
                new_side_line: 2,
                text: "  if (x > 0) {".to_string(),
            },
            crate::analysis::diff::ChangedLine {
                line: 5,
                new_side_line: 5,
                text: "  if (x > 0) {".to_string(),
            },
        ],
        removed_lines: Vec::new(),
    }];

    let result = adapter.analyze_diff(&options, &policy, &changed_files);
    let _ = std::fs::remove_dir_all(&root);
    let result = result?;

    assert_eq!(
        result.findings.len(),
        2,
        "both identical added lines must produce a finding each"
    );
    let first = &result.findings[0];
    let second = &result.findings[1];
    assert_eq!(first.probe.location.line, 2, "findings stay in diff order");
    assert_eq!(second.probe.location.line, 5);
    assert_ne!(
        first.id, second.id,
        "colliding probe ids must be de-duped into distinct identities"
    );
    // Second occurrence carries the ordinal suffix on probe and finding id.
    let expected_second = format!("{}.2", first.probe.id.0);
    assert_eq!(
        second.probe.id.0, expected_second,
        "second occurrence must append the .2 collision suffix"
    );
    assert_eq!(
        second.id, second.probe.id.0,
        "finding id must track the de-duped probe id"
    );
    // First occurrence keeps its ordinal-1 id: the fp8 hex tail carries no
    // `.N` suffix (the id legitimately contains '.' from the file name, so
    // only the last colon-separated segment is checked).
    let tail = first.probe.id.0.rsplit(':').next().unwrap_or("");
    assert!(
        !tail.contains('.'),
        "ordinal-1 id must stay suffix-free, got {}",
        first.probe.id.0
    );
    Ok(())
}

/// Single-occurrence findings keep their ordinal-1 ids: the de-dup pass
/// must not perturb ids that occur once. This pins the stability of the
/// existing TS fixture goldens (e.g. `fixtures/typescript_strong_oracle`
/// pins `probe:src_discount.ts:typescript_preview:2396aec1`).
#[test]
fn analyze_diff_keeps_single_occurrence_probe_ids_stable() -> Result<(), String> {
    let root = ts_unique_tempdir("probe-stable")?;

    ts_write_file(
        &root.join("src/lib.ts"),
        "export function applyDiscount(amount: number, threshold: number): number {\n  if (amount >= threshold) {\n    return amount - 10;\n  }\n  return amount;\n}\n",
    )?;

    let adapter = TypeScriptAdapter;
    let options = AnalysisOptions {
        root: root.clone(),
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
    let changed_files = vec![ChangedFile {
        path: PathBuf::from("src/lib.ts"),
        added_lines: vec![crate::analysis::diff::ChangedLine {
            line: 2,
            new_side_line: 2,
            text: "  if (amount >= threshold) {".to_string(),
        }],
        removed_lines: Vec::new(),
    }];

    let result = adapter.analyze_diff(&options, &policy, &changed_files);
    let _ = std::fs::remove_dir_all(&root);
    let result = result?;

    assert_eq!(result.findings.len(), 1);
    let finding = &result.findings[0];
    let owner = finding
        .probe
        .owner
        .as_ref()
        .ok_or_else(|| "expected a resolved owner".to_string())?;
    // Recompute the content-addressed id exactly as the adapter does;
    // ordinal 1 means no collision suffix.
    let expected = fingerprint_probe_id(
        "probe",
        "src_lib.ts",
        "typescript_preview",
        owner.0.as_str(),
        &normalize_expression("  if (amount >= threshold) {"),
        1,
    );
    assert_eq!(
        finding.probe.id, expected,
        "single-occurrence probe id must stay exactly the ordinal-1 id"
    );
    assert_eq!(finding.id, expected.0);
    Ok(())
}

#[test]
fn classify_probe_shape_recognises_if_predicate() {
    let (family, delta) = classify_probe_shape("    if (amount >= threshold) {");
    assert_eq!(family, ProbeFamily::Predicate);
    assert_eq!(delta, DeltaKind::Control);
}

#[test]
fn classify_probe_shape_recognises_else_if_predicate() {
    let (family, delta) = classify_probe_shape("    } else if (amount === 0) {");
    assert_eq!(family, ProbeFamily::Predicate);
    assert_eq!(delta, DeltaKind::Control);
}

#[test]
fn classify_probe_shape_recognises_return_value() {
    let (family, delta) = classify_probe_shape("    return amount - 10;");
    assert_eq!(family, ProbeFamily::ReturnValue);
    assert_eq!(delta, DeltaKind::Value);
}

#[test]
fn classify_probe_shape_recognises_bare_return() {
    let (family, delta) = classify_probe_shape("    return;");
    assert_eq!(family, ProbeFamily::ReturnValue);
    assert_eq!(delta, DeltaKind::Value);
}

#[test]
fn classify_probe_shape_recognises_throw_error_path() {
    let (family, delta) = classify_probe_shape("    throw new Error('out of range');");
    assert_eq!(family, ProbeFamily::ErrorPath);
    assert_eq!(delta, DeltaKind::Control);
}

#[test]
fn classify_probe_shape_recognises_promise_reject_error_path() {
    let (family, delta) = classify_probe_shape("    return Promise.reject(new Error('boom'));");
    assert_eq!(family, ProbeFamily::ErrorPath);
    assert_eq!(delta, DeltaKind::Control);
}

#[test]
fn classify_probe_shape_recognises_return_await_promise_reject_error_path() {
    let (family, delta) =
        classify_probe_shape("    return await Promise.reject(new Error('boom'));");
    assert_eq!(family, ProbeFamily::ErrorPath);
    assert_eq!(delta, DeltaKind::Control);
}

#[test]
fn classify_probe_shape_recognises_bare_await_promise_reject_error_path() {
    let (family, delta) = classify_probe_shape("    await Promise.reject(new Error('boom'));");
    assert_eq!(family, ProbeFamily::ErrorPath);
    assert_eq!(delta, DeltaKind::Control);
}

#[test]
fn classify_probe_shape_recognises_field_construction() {
    let (family, delta) = classify_probe_shape("    this.count = next;");
    assert_eq!(family, ProbeFamily::FieldConstruction);
    assert_eq!(delta, DeltaKind::Value);
}

#[test]
fn classify_probe_shape_recognises_side_effect_call() {
    let (family, delta) = classify_probe_shape("    logger.record(event);");
    assert_eq!(family, ProbeFamily::SideEffect);
    assert_eq!(delta, DeltaKind::Effect);
}

#[test]
fn classify_probe_shape_recognises_await_side_effect_call() {
    let (family, delta) = classify_probe_shape("    await logger.flush();");
    assert_eq!(family, ProbeFamily::SideEffect);
    assert_eq!(delta, DeltaKind::Effect);
}

#[test]
fn classify_probe_shape_recognises_ternary_as_predicate() {
    let (family, delta) = classify_probe_shape("    amount >= threshold ? amount - 10 : amount;");
    assert_eq!(family, ProbeFamily::Predicate);
    assert_eq!(delta, DeltaKind::Control);
}

#[test]
fn classify_probe_shape_falls_through_to_predicate_default_for_const_decl() {
    // `const` declarations do not match a specific family in the
    // preview adapter; conservative fall-through keeps the historical
    // owner+test sub-slice default (#777) rather than guessing.
    let (family, delta) =
        classify_probe_shape("    const total = applyDiscount(amount, threshold);");
    assert_eq!(family, ProbeFamily::Predicate);
    assert_eq!(delta, DeltaKind::Control);
}

#[test]
fn classify_change_emits_predicate_probe_fact_discriminator() -> Result<(), String> {
    let finding = classify_weak_direct_line("    if (amount >= threshold) {")?;

    assert_eq!(finding.probe.family, ProbeFamily::Predicate);
    assert!(
        finding
            .probe
            .expected_sinks
            .contains(&"branch result".to_string())
    );
    assert!(
        finding
            .probe
            .required_oracles
            .contains(&"boundary input".to_string())
    );
    assert!(finding.flow_sinks.is_empty());
    assert_eq!(
        missing_discriminator_values(&finding),
        vec!["amount == threshold"]
    );
    assert!(
        finding
            .evidence
            .iter()
            .any(|entry| entry == "missing_discriminator: amount == threshold")
    );
    Ok(())
}

#[test]
fn classify_change_emits_return_value_probe_fact_discriminator() -> Result<(), String> {
    let finding = classify_weak_direct_line("    return amount - discount;")?;

    assert_eq!(finding.probe.family, ProbeFamily::ReturnValue);
    assert_eq!(finding.flow_sinks.len(), 1);
    assert_eq!(finding.flow_sinks[0].kind, FlowSinkKind::ReturnValue);
    assert_eq!(
        missing_discriminator_values(&finding),
        vec!["return value == amount - discount"]
    );
    assert_eq!(
        finding.activation.missing_discriminators[0]
            .flow_sink
            .as_ref()
            .map(|sink| &sink.kind),
        Some(&FlowSinkKind::ReturnValue)
    );
    Ok(())
}

#[test]
fn classify_change_omits_return_value_discriminator_for_bare_return() -> Result<(), String> {
    let finding = classify_weak_direct_line("    return;")?;

    assert_eq!(finding.probe.family, ProbeFamily::ReturnValue);
    assert_eq!(finding.flow_sinks.len(), 1);
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
fn classify_change_emits_error_path_probe_fact_discriminator() -> Result<(), String> {
    let finding = classify_weak_direct_line("    throw new RangeError(\"too low\");")?;

    assert_eq!(finding.probe.family, ProbeFamily::ErrorPath);
    assert_eq!(finding.flow_sinks.len(), 1);
    assert_eq!(finding.flow_sinks[0].kind, FlowSinkKind::ErrorVariant);
    assert_eq!(
        missing_discriminator_values(&finding),
        vec!["throws RangeError matching \"too low\""]
    );
    Ok(())
}

#[test]
fn classify_change_omits_error_discriminator_for_generic_throw_identifier() -> Result<(), String> {
    let finding = classify_weak_direct_line("    throw err;")?;

    assert_eq!(finding.probe.family, ProbeFamily::ErrorPath);
    assert_eq!(finding.flow_sinks.len(), 1);
    assert!(finding.activation.missing_discriminators.is_empty());
    Ok(())
}

#[test]
fn classify_change_omits_error_discriminator_for_generic_rejected_identifier() -> Result<(), String>
{
    let finding = classify_weak_direct_line("    return Promise.reject(err);")?;

    assert_eq!(finding.probe.family, ProbeFamily::ErrorPath);
    assert_eq!(finding.flow_sinks.len(), 1);
    assert!(finding.activation.missing_discriminators.is_empty());
    Ok(())
}

#[test]
fn classify_change_emits_field_construction_probe_fact_discriminator() -> Result<(), String> {
    let finding = classify_weak_direct_line("    profile.status = nextStatus;")?;

    assert_eq!(finding.probe.family, ProbeFamily::FieldConstruction);
    assert_eq!(finding.flow_sinks.len(), 1);
    assert_eq!(finding.flow_sinks[0].kind, FlowSinkKind::StructField);
    assert_eq!(
        missing_discriminator_values(&finding),
        vec!["profile.status == nextStatus"]
    );
    Ok(())
}

#[test]
fn classify_change_omits_field_discriminator_for_computed_field_assignment() -> Result<(), String> {
    let finding = classify_weak_direct_line("    profile[key] = nextStatus;")?;

    assert_eq!(finding.probe.family, ProbeFamily::FieldConstruction);
    assert!(finding.flow_sinks.is_empty());
    assert!(finding.activation.missing_discriminators.is_empty());
    Ok(())
}

#[test]
fn classify_change_emits_object_literal_field_probe_fact_discriminator() -> Result<(), String> {
    let finding = classify_weak_direct_line("    return { status: nextStatus, total };")?;

    assert_eq!(finding.probe.family, ProbeFamily::FieldConstruction);
    assert_eq!(finding.flow_sinks.len(), 1);
    assert_eq!(finding.flow_sinks[0].kind, FlowSinkKind::StructField);
    assert_eq!(
        missing_discriminator_values(&finding),
        vec!["status == nextStatus"]
    );
    Ok(())
}

#[test]
fn classify_change_omits_object_field_discriminator_for_computed_object_key() -> Result<(), String>
{
    let finding = classify_weak_direct_line("    return { [key]: nextStatus, total };")?;

    assert_eq!(finding.probe.family, ProbeFamily::FieldConstruction);
    assert!(finding.flow_sinks.is_empty());
    assert!(finding.activation.missing_discriminators.is_empty());
    Ok(())
}

#[test]
fn classify_change_emits_call_side_effect_probe_fact_discriminator() -> Result<(), String> {
    let finding = classify_weak_direct_line("    audit.record(status);")?;

    assert_eq!(finding.probe.family, ProbeFamily::SideEffect);
    assert_eq!(finding.flow_sinks.len(), 1);
    assert_eq!(finding.flow_sinks[0].kind, FlowSinkKind::CallEffect);
    assert_eq!(
        missing_discriminator_values(&finding),
        vec!["call audit.record includes status"]
    );
    assert!(
        missing_discriminator_values(&finding)
            .iter()
            .all(|value| !value.contains("mock interaction"))
    );
    Ok(())
}

#[test]
fn classify_change_emits_mock_interaction_probe_fact_discriminator() -> Result<(), String> {
    let finding = classify_weak_direct_line("    mockSend(payload);")?;

    assert_eq!(finding.probe.family, ProbeFamily::SideEffect);
    assert_eq!(finding.flow_sinks.len(), 1);
    assert_eq!(finding.flow_sinks[0].kind, FlowSinkKind::CallEffect);
    assert_eq!(
        missing_discriminator_values(&finding),
        vec!["mock interaction mockSend called with payload"]
    );
    Ok(())
}

#[test]
fn classify_change_uses_call_effect_wording_for_console_log_without_literal() -> Result<(), String>
{
    let finding = classify_weak_direct_line("    console.log(status);")?;

    assert_eq!(finding.probe.family, ProbeFamily::SideEffect);
    assert_eq!(
        missing_discriminator_values(&finding),
        vec!["call console.log includes status"]
    );
    assert!(
        missing_discriminator_values(&finding)
            .iter()
            .all(|value| !value.contains("log contains"))
    );
    Ok(())
}

#[test]
fn classify_change_omits_probe_facts_for_ambiguous_const_expression() -> Result<(), String> {
    let finding = classify_weak_direct_line("    const total = applyDiscount(amount, threshold);")?;

    assert_eq!(finding.probe.family, ProbeFamily::Predicate);
    assert!(finding.probe.expected_sinks.is_empty());
    assert!(finding.probe.required_oracles.is_empty());
    assert!(finding.flow_sinks.is_empty());
    assert!(finding.activation.missing_discriminators.is_empty());
    assert!(
        finding
            .evidence
            .iter()
            .any(|entry| entry == "probe_fact: ambiguous_fallback")
    );
    Ok(())
}

#[test]
fn classify_change_omits_probe_facts_for_ambiguous_computed_member_call() -> Result<(), String> {
    let finding = classify_weak_direct_line("    handlers[name](payload);")?;

    assert_eq!(finding.probe.family, ProbeFamily::SideEffect);
    assert!(finding.flow_sinks.is_empty());
    assert!(finding.activation.missing_discriminators.is_empty());
    assert_static_limit(
        &finding,
        StaticLimitKind::DynamicDispatch,
        "dynamic_dispatch",
    );
    Ok(())
}

#[test]
fn classify_change_surfaces_metaprogramming_static_limit() -> Result<(), String> {
    let finding = classify_weak_direct_line("    return new Proxy(target, handler);")?;

    assert_eq!(finding.probe.family, ProbeFamily::ReturnValue);
    assert_static_limit(
        &finding,
        StaticLimitKind::Metaprogramming,
        "metaprogramming",
    );
    Ok(())
}

#[test]
fn classify_change_does_not_surface_static_limits_from_string_literals() -> Result<(), String> {
    let proxy_string = classify_weak_direct_line("    return \"Proxy(\";")?;
    let computed_string = classify_weak_direct_line("    return \"actions[key](\";")?;

    assert_eq!(proxy_string.static_limit_kind, None);
    assert_eq!(computed_string.static_limit_kind, None);
    Ok(())
}

#[test]
fn classify_change_surfaces_decorator_indirection_static_limit() -> Result<(), String> {
    let mut owner = test_owner("save", "src/service.ts");
    owner.decorated = true;
    let test = weak_direct_test_for("save");
    let finding = classify_change(
        Path::new("src/service.ts"),
        2,
        "    return value;",
        &[owner],
        &[test],
        None,
        &ReExportIndex::empty(),
        None,
    )
    .ok_or_else(|| "expected decorated owner finding".to_string())?;

    assert_static_limit(
        &finding,
        StaticLimitKind::DecoratorIndirection,
        "decorator_indirection",
    );
    Ok(())
}

#[test]
fn extract_owners_marks_class_method_as_decorated_when_class_is_decorated() {
    let owners = extract_owners(
        Path::new("src/service.ts"),
        r#"@sealed
class Service {
    save(value: string) {
        return value;
    }
}
"#,
    );

    assert_eq!(owners.len(), 1);
    assert_eq!(owners[0].name, "save");
    assert!(owners[0].decorated);
}

#[test]
fn classify_change_surfaces_missing_import_graph_static_limit() -> Result<(), String> {
    let owners = extract_owners(
        Path::new("src/pricing.ts"),
        r#"import { normalizeTotal } from "./math";

export function discountedTotal(amount: number): number {
    return normalizeTotal(amount);
}
"#,
    );
    let test = weak_direct_test_for("discountedTotal");
    let finding = classify_change(
        Path::new("src/pricing.ts"),
        4,
        "    return normalizeTotal(amount);",
        &owners,
        &[test],
        None,
        &ReExportIndex::empty(),
        None,
    )
    .ok_or_else(|| "expected imported-symbol finding".to_string())?;

    assert_static_limit(
        &finding,
        StaticLimitKind::MissingImportGraph,
        "missing_import_graph",
    );
    assert!(
        finding
            .evidence
            .iter()
            .any(|line| line.contains("normalizeTotal"))
    );
    Ok(())
}

#[test]
fn classify_change_omits_discriminator_for_call_shaped_predicate_operand() -> Result<(), String> {
    let finding = classify_weak_direct_line("    if (input.trim() === \"\") {")?;

    assert_eq!(finding.probe.family, ProbeFamily::Predicate);
    assert!(finding.flow_sinks.is_empty());
    assert!(finding.activation.missing_discriminators.is_empty());
    Ok(())
}

#[test]
fn classify_change_omits_probe_facts_for_heuristic_only_related_test() -> Result<(), String> {
    let owner = test_owner("applyDiscount", "src/lib.ts");
    let test = heuristic_name_test_for("applyDiscount");
    let finding = classify_change(
        Path::new("src/lib.ts"),
        2,
        "    if (amount >= threshold) {",
        &[owner],
        &[test],
        None,
        &ReExportIndex::empty(),
        None,
    )
    .ok_or_else(|| "expected heuristic TypeScript preview finding".to_string())?;

    assert!(matches!(finding.class, ExposureClass::WeaklyExposed));
    assert!(finding.activation.missing_discriminators.is_empty());
    assert!(
        finding
            .recommended_next_step
            .as_deref()
            .is_some_and(|step| step.contains("heuristic only"))
    );
    Ok(())
}

#[test]
fn extract_tests_collects_vi_mock_paths_in_file() {
    let source = r#"
import { vi } from "vitest";
vi.mock("./api");
vi.mock("./logger");
test("alpha", () => {
    expect(applyDiscount(50, 100)).toBe(50);
});
"#;
    let tests = extract_tests(Path::new("tests/lib.test.ts"), source);
    assert_eq!(tests.len(), 1);
    assert_eq!(
        tests[0].mocks_in_file,
        vec!["./api".to_string(), "./logger".to_string()]
    );
}

#[test]
fn extract_tests_collects_jest_mock_paths_in_file() {
    let source = r#"
jest.mock("./repository");
test("alpha", () => {
    expect(applyDiscount(50, 100)).toBe(50);
});
"#;
    let tests = extract_tests(Path::new("tests/lib.test.ts"), source);
    assert_eq!(tests.len(), 1);
    assert_eq!(tests[0].mocks_in_file, vec!["./repository".to_string()]);
}

#[test]
fn extract_tests_returns_empty_mock_list_when_no_mock_call() {
    let source = r#"
test("alpha", () => {
    expect(applyDiscount(50, 100)).toBe(50);
});
"#;
    let tests = extract_tests(Path::new("tests/lib.test.ts"), source);
    assert_eq!(tests.len(), 1);
    assert!(tests[0].mocks_in_file.is_empty());
}

#[test]
fn collect_related_mock_paths_dedups_across_tests_in_same_file() {
    let owner = TypeScriptOwner {
        name: "applyDiscount".to_string(),
        file: PathBuf::from("src/lib.ts"),
        start_line: 1,
        end_line: 5,
        owner_kind: OwnerKind::Function,
        class_name: None,
        decorated: false,
        exported_as_default: false,
        class_default_export: false,
        arity: None,
        parameters: Vec::new(),
        source_text: None,
        imports: Vec::new(),
        method_kind: TypeScriptMethodKind::Ordinary,
    };
    let tests = vec![
        TypeScriptTest {
            name: "alpha".to_string(),
            local_name: "alpha".to_string(),
            describe_names: Vec::new(),
            file: PathBuf::from("tests/lib.test.ts"),
            line: 1,
            body_text: "applyDiscount(1, 2)".to_string(),
            assertions: Vec::new(),
            mocks_in_file: vec!["./api".to_string()],
            imports_in_file: Vec::new(),
        },
        TypeScriptTest {
            name: "beta".to_string(),
            local_name: "beta".to_string(),
            describe_names: Vec::new(),
            file: PathBuf::from("tests/lib.test.ts"),
            line: 2,
            body_text: "applyDiscount(3, 4)".to_string(),
            assertions: Vec::new(),
            mocks_in_file: vec!["./api".to_string()],
            imports_in_file: Vec::new(),
        },
    ];
    let paths = collect_related_mock_paths(&owner, &tests, None, &ReExportIndex::empty(), None);
    assert_eq!(paths, vec!["./api".to_string()]);
}

#[test]
fn collect_related_mock_paths_ignores_unrelated_tests() {
    let owner = TypeScriptOwner {
        name: "applyDiscount".to_string(),
        file: PathBuf::from("src/lib.ts"),
        start_line: 1,
        end_line: 5,
        owner_kind: OwnerKind::Function,
        class_name: None,
        decorated: false,
        exported_as_default: false,
        class_default_export: false,
        arity: None,
        parameters: Vec::new(),
        source_text: None,
        imports: Vec::new(),
        method_kind: TypeScriptMethodKind::Ordinary,
    };
    let tests = vec![TypeScriptTest {
        name: "unrelated".to_string(),
        local_name: "unrelated".to_string(),
        describe_names: Vec::new(),
        file: PathBuf::from("tests/other.test.ts"),
        line: 1,
        body_text: "otherHelper()".to_string(),
        assertions: Vec::new(),
        mocks_in_file: vec!["./api".to_string()],
        imports_in_file: Vec::new(),
    }];
    let paths = collect_related_mock_paths(&owner, &tests, None, &ReExportIndex::empty(), None);
    assert!(paths.is_empty());
}

#[test]
fn collect_related_mock_paths_ignores_object_method_mentions() {
    let owner = TypeScriptOwner {
        name: "applyDiscount".to_string(),
        file: PathBuf::from("src/lib.ts"),
        start_line: 1,
        end_line: 5,
        owner_kind: OwnerKind::Function,
        class_name: None,
        decorated: false,
        exported_as_default: false,
        class_default_export: false,
        arity: None,
        parameters: Vec::new(),
        source_text: None,
        imports: Vec::new(),
        method_kind: TypeScriptMethodKind::Ordinary,
    };
    let tests = vec![TypeScriptTest {
        name: "unrelated method".to_string(),
        local_name: "unrelated method".to_string(),
        describe_names: Vec::new(),
        file: PathBuf::from("tests/cart.test.ts"),
        line: 1,
        body_text: "expect(order.applyDiscount(50)).toBe(40);".to_string(),
        assertions: Vec::new(),
        mocks_in_file: vec!["./api".to_string()],
        imports_in_file: Vec::new(),
    }];
    let paths = collect_related_mock_paths(&owner, &tests, None, &ReExportIndex::empty(), None);
    assert!(paths.is_empty());
}

#[test]
fn classify_change_surfaces_mocked_module_static_limit_in_missing_and_evidence()
-> Result<(), String> {
    let owner = TypeScriptOwner {
        name: "applyDiscount".to_string(),
        file: PathBuf::from("src/lib.ts"),
        start_line: 1,
        end_line: 5,
        owner_kind: OwnerKind::Function,
        class_name: None,
        decorated: false,
        exported_as_default: false,
        class_default_export: false,
        arity: None,
        parameters: Vec::new(),
        source_text: None,
        imports: Vec::new(),
        method_kind: TypeScriptMethodKind::Ordinary,
    };
    let tests = vec![TypeScriptTest {
        name: "alpha".to_string(),
        local_name: "alpha".to_string(),
        describe_names: Vec::new(),
        file: PathBuf::from("tests/lib.test.ts"),
        line: 1,
        body_text: "applyDiscount(50, 100)".to_string(),
        assertions: Vec::new(),
        mocks_in_file: vec!["./api".to_string()],
        imports_in_file: Vec::new(),
    }];
    let finding = classify_change(
        Path::new("src/lib.ts"),
        2,
        "    if (amount >= threshold) {",
        &[owner],
        &tests,
        None,
        &ReExportIndex::empty(),
        None,
    )
    .ok_or_else(|| "expected a finding for the changed line".to_string())?;
    assert!(
        finding
            .missing
            .iter()
            .any(|line| line.contains("Static limit `mocked_module`") && line.contains("./api"))
    );
    assert!(
        finding
            .evidence
            .iter()
            .any(|line| line.starts_with("static_limit mocked_module:"))
    );
    assert_eq!(
        finding.static_limit_kind,
        Some(StaticLimitKind::MockedModule)
    );
    Ok(())
}

/// Cross-package negative control: a test in `packages/b` that mocks a path
/// resolving to the owner's module in `packages/a` must NOT surface a
/// `mocked_module` static limit on the `packages/a` finding. The package-local
/// filter excludes the test from the credited relation set; the mock
/// collector must not re-admit it (a `mocked_module` limit forces
/// `gap_state: static_limitation` with empty missing fields, so this would be
/// a wrong actionable signal built from a deliberately excluded test).
#[test]
fn classify_change_cross_package_mock_does_not_surface_mocked_module_limit() -> Result<(), String> {
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.subsec_nanos())
        .unwrap_or(0);
    let root = std::env::temp_dir().join(format!("ripr-mock-cross-pkg-{stamp}"));
    let pkg_a = root.join("packages").join("pkg-a");
    let pkg_b = root.join("packages").join("pkg-b");
    let _ = fs::create_dir_all(pkg_a.join("src"));
    let _ = fs::create_dir_all(pkg_b.join("tests"));
    let _ = fs::write(pkg_a.join("package.json"), r#"{"name":"pkg-a"}"#);
    let _ = fs::write(pkg_b.join("package.json"), r#"{"name":"pkg-b"}"#);

    let owner = TypeScriptOwner {
        name: "doWork".to_string(),
        file: pkg_a.join("src").join("work.ts"),
        start_line: 1,
        end_line: 10,
        owner_kind: OwnerKind::Function,
        class_name: None,
        decorated: false,
        exported_as_default: false,
        class_default_export: false,
        imports: Vec::new(),
        method_kind: TypeScriptMethodKind::Ordinary,
        arity: None,
        parameters: Vec::new(),
        source_text: None,
    };
    // The test body calls the owner (it would be credited without the
    // package-local filter) and mocks a path resolving to the owner's module.
    let tests = vec![TypeScriptTest {
        name: "cross-package doWork test".to_string(),
        local_name: "cross-package doWork test".to_string(),
        describe_names: Vec::new(),
        file: pkg_b.join("tests").join("work.test.ts"),
        line: 1,
        body_text: "doWork();".to_string(),
        assertions: Vec::new(),
        mocks_in_file: vec!["./work".to_string()],
        imports_in_file: Vec::new(),
    }];

    // Live pipeline (workspace_root supplied): the cross-package test is
    // excluded from the credited relation set, so no `mocked_module` limit.
    let finding = classify_change(
        &pkg_a.join("src").join("work.ts"),
        2,
        "    return doWorkImpl();",
        &[owner],
        &tests,
        Some(&root),
        &ReExportIndex::empty(),
        None,
    )
    .ok_or_else(|| "expected a finding for the changed line".to_string())?;
    assert!(
        finding.static_limit_kind != Some(StaticLimitKind::MockedModule),
        "cross-package mock must not surface a `mocked_module` limit, got {:?}",
        finding.static_limit_kind
    );
    assert!(
        !finding
            .evidence
            .iter()
            .any(|line| line.starts_with("static_limit mocked_module:")),
        "cross-package mock must not emit a `mocked_module` evidence line"
    );

    // Removal/known-wrong control: without the package-local filter (the
    // single-package path), the same fixtures DO credit the test and surface
    // the limit — proving the negative control exercises the real producer.
    let owner = TypeScriptOwner {
        name: "doWork".to_string(),
        file: pkg_a.join("src").join("work.ts"),
        start_line: 1,
        end_line: 10,
        owner_kind: OwnerKind::Function,
        class_name: None,
        decorated: false,
        exported_as_default: false,
        class_default_export: false,
        imports: Vec::new(),
        method_kind: TypeScriptMethodKind::Ordinary,
        arity: None,
        parameters: Vec::new(),
        source_text: None,
    };
    let unfiltered = classify_change(
        &pkg_a.join("src").join("work.ts"),
        2,
        "    return doWorkImpl();",
        &[owner],
        &tests,
        None,
        &ReExportIndex::empty(),
        None,
    )
    .ok_or_else(|| "expected a finding for the changed line".to_string())?;
    assert_eq!(
        unfiltered.static_limit_kind,
        Some(StaticLimitKind::MockedModule),
        "without the package-local filter the cross-package test is credited \
         and the `mocked_module` limit must fire (non-vacuous control)"
    );
    Ok(())
}

// ── Named limitation taxonomy (RIPR-SPEC-0085 §PR4) ────────────────────────
//
// Each test below verifies a REAL producer for its named limitation.
// Unit tests hand-set fixture data but each producer corresponds to a real
// TypeScript AST condition; the full production path is exercised via the
// fixture goldens (typescript_mocked_module_limit,
// typescript_static_limit_taxonomy, typescript_jest_vitest_assertion_facts,
// typescript_limitation_custom_matcher).

/// `typescript_mock_only_observer` fires when `static_limit_kind == MockedModule`,
/// which is produced by `collect_related_mock_paths` when a related test file
/// contains `vi.mock(...)` / `jest.mock(...)` calls.
#[test]
fn named_limitation_mock_only_observer_emitted_for_mocked_module_static_limit() -> Result<(), String>
{
    let owner = TypeScriptOwner {
        name: "applyDiscount".to_string(),
        file: PathBuf::from("src/lib.ts"),
        start_line: 1,
        end_line: 5,
        owner_kind: OwnerKind::Function,
        class_name: None,
        decorated: false,
        exported_as_default: false,
        class_default_export: false,
        arity: None,
        parameters: Vec::new(),
        source_text: None,
        imports: Vec::new(),
        method_kind: TypeScriptMethodKind::Ordinary,
    };
    let tests = vec![TypeScriptTest {
        name: "alpha".to_string(),
        local_name: "alpha".to_string(),
        describe_names: Vec::new(),
        file: PathBuf::from("tests/lib.test.ts"),
        line: 1,
        body_text: "applyDiscount(50, 100)".to_string(),
        assertions: vec![TypeScriptAssertion {
            matcher: "toBe".to_string(),
            argument_count: 1,
            line: 2,
            oracle_kind: OracleKind::ExactValue,
            oracle_strength: OracleStrength::Strong,
            mock_payload: None,
            error_payload: None,
            observed_expression: None,
            expected_value_or_variant: None,
            has_dynamic_matcher_arg: false,
            oracle_confidence: OracleConfidence::Medium,
        }],
        mocks_in_file: vec!["./api".to_string()],
        imports_in_file: Vec::new(),
    }];
    let finding = classify_change(
        Path::new("src/lib.ts"),
        2,
        "    if (amount >= threshold) {",
        &[owner],
        &tests,
        None,
        &ReExportIndex::empty(),
        None,
    )
    .ok_or_else(|| "expected a finding".to_string())?;

    // The named limitation must be present in evidence
    assert_evidence_contains(
        &finding,
        "typescript_limitation: typescript_mock_only_observer",
    );
    assert_evidence_contains(
        &finding,
        "typescript_limitation_sample: typescript_mock_only_observer at src/lib.ts:2",
    );
    assert_evidence_contains(
        &finding,
        "typescript_limitation_why: typescript_mock_only_observer",
    );
    assert_evidence_contains(
        &finding,
        "typescript_limitation_repair_route: typescript_mock_only_observer → analysis/typescript-mock-shape-resolution",
    );
    // The existing static_limit_kind field must NOT change
    assert_eq!(
        finding.static_limit_kind,
        Some(StaticLimitKind::MockedModule)
    );
    // repair_packet_ready remains false (checked via gap_state)
    assert_evidence_contains(&finding, "gap_state: static_limitation");
    Ok(())
}

/// `typescript_import_graph_unresolved` fires when `static_limit_kind == MissingImportGraph`,
/// which is produced by `static_limit_for_change` when the changed line calls
/// an imported symbol from the owner's import list.
#[test]
fn named_limitation_import_graph_unresolved_emitted_for_missing_import_graph() -> Result<(), String>
{
    let owner = TypeScriptOwner {
        name: "LabelView".to_string(),
        file: PathBuf::from("src/label.jsx"),
        start_line: 1,
        end_line: 5,
        owner_kind: OwnerKind::Function,
        class_name: None,
        decorated: false,
        exported_as_default: false,
        class_default_export: false,
        arity: None,
        parameters: Vec::new(),
        source_text: None,
        imports: vec![TypeScriptImport {
            source: "./labels".to_string(),
            imported: Some("normalizeLabel".to_string()),
            local: "normalizeLabel".to_string(),
            namespace: false,
        }],
        method_kind: TypeScriptMethodKind::Ordinary,
    };
    let tests = vec![TypeScriptTest {
        name: "LabelView smoke".to_string(),
        local_name: "LabelView smoke".to_string(),
        describe_names: Vec::new(),
        file: PathBuf::from("tests/label.test.jsx"),
        line: 1,
        body_text: "LabelView({ label: ' Ready ' });".to_string(),
        assertions: vec![TypeScriptAssertion {
            matcher: "toBeTruthy".to_string(),
            argument_count: 0,
            line: 2,
            oracle_kind: OracleKind::SmokeOnly,
            oracle_strength: OracleStrength::Smoke,
            mock_payload: None,
            error_payload: None,
            observed_expression: None,
            expected_value_or_variant: None,
            has_dynamic_matcher_arg: false,
            oracle_confidence: OracleConfidence::Low,
        }],
        mocks_in_file: Vec::new(),
        imports_in_file: Vec::new(),
    }];
    let finding = classify_change(
        Path::new("src/label.jsx"),
        2,
        "    return normalizeLabel(props.label);",
        &[owner],
        &tests,
        None,
        &ReExportIndex::empty(),
        None,
    )
    .ok_or_else(|| "expected a finding".to_string())?;

    assert_evidence_contains(
        &finding,
        "typescript_limitation: typescript_import_graph_unresolved",
    );
    assert_evidence_contains(
        &finding,
        "typescript_limitation_sample: typescript_import_graph_unresolved at src/label.jsx:2",
    );
    assert_evidence_contains(
        &finding,
        "typescript_limitation_why: typescript_import_graph_unresolved",
    );
    assert_evidence_contains(
        &finding,
        "typescript_limitation_repair_route: typescript_import_graph_unresolved → analysis/typescript-import-graph",
    );
    assert_eq!(
        finding.static_limit_kind,
        Some(StaticLimitKind::MissingImportGraph)
    );
    assert_evidence_contains(&finding, "gap_state: static_limitation");
    Ok(())
}

/// `typescript_snapshot_discriminator_unresolved` fires when an oracle-eligible
/// related test has an assertion with `OracleKind::Snapshot`.
/// Real producer: `oracle.rs::oracle_for_matcher` maps `toMatchSnapshot` /
/// `toMatchInlineSnapshot` to `OracleKind::Snapshot`.
#[test]
fn named_limitation_snapshot_discriminator_emitted_for_snapshot_oracle() -> Result<(), String> {
    let owner = test_owner("renderSummary", "src/signals.ts");
    let tests = extract_tests(
        Path::new("tests/signals.test.ts"),
        r#"import { renderSummary } from "../src/signals";
test("renders summary snapshot", () => {
  expect(renderSummary("ready")).toMatchSnapshot();
});
"#,
    );
    let finding = classify_change(
        Path::new("src/signals.ts"),
        2,
        "    return `summary:${status.trim()}`;",
        &[owner],
        &tests,
        None,
        &ReExportIndex::empty(),
        None,
    )
    .ok_or_else(|| "expected a finding".to_string())?;

    assert_evidence_contains(
        &finding,
        "typescript_limitation: typescript_snapshot_discriminator_unresolved",
    );
    assert_evidence_contains(
        &finding,
        "typescript_limitation_why: typescript_snapshot_discriminator_unresolved",
    );
    assert_evidence_contains(
        &finding,
        "typescript_limitation_repair_route: typescript_snapshot_discriminator_unresolved → analysis/typescript-snapshot-oracle-hardening",
    );
    // static_limit_kind is unset (snapshot is not a static_limit, it is oracle-based)
    assert_eq!(finding.static_limit_kind, None);
    // repair_packet_ready remains false — gap_state is advisory (not static_limitation)
    // because snapshot is not a StaticLimitKind
    assert!(
        !finding
            .evidence
            .iter()
            .any(|e| e.contains("gap_state: static_limitation"))
    );
    Ok(())
}

/// `typescript_custom_matcher_unresolved` fires when an oracle-eligible related
/// test has an assertion with `OracleKind::Unknown` AND a non-empty matcher string.
/// Real producer: any `expect(x).<matcher>(...)` whose matcher is not in
/// `oracle.rs`'s recognised set returns `(OracleKind::Unknown, OracleStrength::Unknown)`.
#[test]
fn named_limitation_custom_matcher_emitted_for_unrecognised_matcher() -> Result<(), String> {
    let owner = test_owner("computePrice", "src/pricing.ts");
    let tests = extract_tests(
        Path::new("tests/pricing.test.ts"),
        r#"import { computePrice } from "../src/pricing";
test("price is in expected range", () => {
  const price = computePrice(10, 3);
  expect(price).toBeWithinRange(20, 40);
});
"#,
    );
    let finding = classify_change(
        Path::new("src/pricing.ts"),
        2,
        "    if (base >= 0) {",
        &[owner],
        &tests,
        None,
        &ReExportIndex::empty(),
        None,
    )
    .ok_or_else(|| "expected a finding".to_string())?;

    assert_evidence_contains(
        &finding,
        "typescript_limitation: typescript_custom_matcher_unresolved",
    );
    assert_evidence_contains(
        &finding,
        "typescript_limitation_why: typescript_custom_matcher_unresolved — the test uses an unrecognised matcher `.toBeWithinRange(...)`",
    );
    assert_evidence_contains(
        &finding,
        "typescript_limitation_repair_route: typescript_custom_matcher_unresolved → analysis/typescript-custom-matcher-resolution",
    );
    // Static limit kind is unset (custom matcher is oracle-based, not a static limit)
    assert_eq!(finding.static_limit_kind, None);
    Ok(())
}

/// Recognised matchers (e.g. `toBe`) must NOT trigger
/// `typescript_custom_matcher_unresolved`.
#[test]
fn named_limitation_custom_matcher_not_emitted_for_recognised_matcher() -> Result<(), String> {
    let owner = test_owner("applyDiscount", "src/lib.ts");
    let test = direct_test_with_assertion(
        "discount test",
        "applyDiscount(100, 100)",
        "toBe",
        1,
        OracleKind::ExactValue,
        OracleStrength::Strong,
    );
    let finding = classify_change(
        Path::new("src/lib.ts"),
        2,
        "    if (amount >= threshold) {",
        &[owner],
        &[test],
        None,
        &ReExportIndex::empty(),
        None,
    )
    .ok_or_else(|| "expected a finding".to_string())?;

    assert_evidence_lacks(
        &finding,
        "typescript_limitation: typescript_custom_matcher_unresolved",
    );
    Ok(())
}

/// `typescript_oracle_helper_gated` fires when an oracle-eligible related test
/// wraps the changed owner call in an assertion-shaped helper, but the
/// syntax-first extractor finds no direct supported assertion to credit.
#[test]
fn named_limitation_oracle_helper_gated_emitted_for_assertion_helper_wrapping_owner_call()
-> Result<(), String> {
    let owner = test_owner("computePrice", "src/pricing.ts");
    let tests = extract_tests(
        Path::new("tests/pricing.test.ts"),
        r#"import { computePrice } from "../src/pricing";
test("price is checked through helper", () => {
  assertPriceBoundary(computePrice(10, 3), 20);
});
"#,
    );
    let finding = classify_change(
        Path::new("src/pricing.ts"),
        2,
        "    if (base >= 0) {",
        &[owner],
        &tests,
        None,
        &ReExportIndex::empty(),
        None,
    )
    .ok_or_else(|| "expected a finding".to_string())?;

    assert_evidence_contains(
        &finding,
        "typescript_limitation: typescript_oracle_helper_gated",
    );
    assert_evidence_contains(
        &finding,
        "typescript_limitation_sample: typescript_oracle_helper_gated at tests/pricing.test.ts:3",
    );
    assert_evidence_contains(
        &finding,
        "typescript_limitation_why: typescript_oracle_helper_gated — the test calls assertion helper `assertPriceBoundary(...)` around owner `computePrice`",
    );
    assert_evidence_contains(
        &finding,
        "typescript_limitation_repair_route: typescript_oracle_helper_gated → analysis/typescript-oracle-helper-resolution",
    );
    assert_eq!(finding.static_limit_kind, None);
    assert!(!matches!(finding.class, ExposureClass::Exposed));
    assert_evidence_lacks(&finding, "repair_packet_ready: true");
    Ok(())
}

/// Heuristic-only (name/proximity) related tests must NOT trigger oracle-based
/// named limitations, because heuristic relations are not oracle-eligible.
#[test]
fn named_limitation_oracle_based_not_emitted_for_heuristic_only_relation() -> Result<(), String> {
    // A heuristic (test-name match) test with snapshot assertion — must NOT
    // trigger typescript_snapshot_discriminator_unresolved because the relation
    // is not oracle-eligible.
    let owner = test_owner("renderSummary", "src/signals.ts");
    let test = TypeScriptTest {
        name: "renderSummary snapshot".to_string(), // name match only
        local_name: "renderSummary snapshot".to_string(),
        describe_names: Vec::new(),
        file: PathBuf::from("tests/signals.test.ts"),
        line: 1,
        // No call to renderSummary( in body_text → heuristic-only name relation
        body_text: "expect(getOutput()).toMatchSnapshot();".to_string(),
        assertions: vec![TypeScriptAssertion {
            matcher: "toMatchSnapshot".to_string(),
            argument_count: 0,
            line: 2,
            oracle_kind: OracleKind::Snapshot,
            oracle_strength: OracleStrength::Medium,
            mock_payload: None,
            error_payload: None,
            observed_expression: None,
            expected_value_or_variant: None,
            has_dynamic_matcher_arg: false,
            oracle_confidence: OracleConfidence::Medium,
        }],
        mocks_in_file: Vec::new(),
        imports_in_file: Vec::new(),
    };
    let finding = classify_change(
        Path::new("src/signals.ts"),
        2,
        "    return `summary:${status.trim()}`;",
        &[owner],
        &[test],
        None,
        &ReExportIndex::empty(),
        None,
    )
    .ok_or_else(|| "expected a finding".to_string())?;

    // Heuristic relation: no oracle-eligible relation → no snapshot limitation
    assert_evidence_lacks(
        &finding,
        "typescript_limitation: typescript_snapshot_discriminator_unresolved",
    );
    Ok(())
}

// ──────────────────────────────────────────────────────────────────────────────
// PR5: Oracle metadata evidence lines (RIPR-SPEC-0085 §PR5)
// ──────────────────────────────────────────────────────────────────────────────

/// Oracle metadata lines are emitted for an oracle-eligible relation with a
/// literal expected value: observed_expression, expected_value_or_variant,
/// confidence (high), and evidence_ref.
#[test]
fn oracle_metadata_emitted_for_literal_expected_value() {
    // Parse a bare expression statement (not wrapped in a test() call)
    // so collect_expect_assertions_in_statements can find it directly.
    let source = "expect(clamp(-5, 0, 10)).toBe(0);";
    let file = PathBuf::from("tests/clamp.test.ts");
    let allocator = Allocator::default();
    let parse_result = Parser::new(&allocator, source, SourceType::ts()).parse();
    let assertions =
        collect_expect_assertions_in_statements(&parse_result.program.body, source, None);
    assert_eq!(assertions.len(), 1, "should extract one assertion");
    let assertion = &assertions[0];
    assert_eq!(assertion.matcher, "toBe");
    assert_eq!(
        assertion.observed_expression.as_deref(),
        Some("clamp(-5, 0, 10)")
    );
    assert_eq!(assertion.expected_value_or_variant.as_deref(), Some("0"));
    assert!(!assertion.has_dynamic_matcher_arg);
    assert_eq!(assertion.oracle_confidence, OracleConfidence::High);

    let lines = oracle_metadata_evidence_lines(assertion, &file);
    assert!(
        lines
            .iter()
            .any(|l| l == "typescript_oracle_observed: clamp(-5, 0, 10)"),
        "expected observed line, got: {lines:?}"
    );
    assert!(
        lines.iter().any(|l| l == "typescript_oracle_expected: 0"),
        "expected expected line, got: {lines:?}"
    );
    assert!(
        lines
            .iter()
            .any(|l| l == "typescript_oracle_confidence: high"),
        "expected confidence high, got: {lines:?}"
    );
    assert!(
        lines
            .iter()
            .any(|l| l.starts_with("typescript_oracle_evidence_ref: tests/clamp.test.ts:")),
        "expected evidence_ref line, got: {lines:?}"
    );
}

/// Oracle metadata: `has_dynamic_matcher_arg = true` and no expected value when
/// the matcher argument is a variable (non-literal dynamic expression).
#[test]
fn oracle_metadata_has_dynamic_matcher_arg_for_variable_expected() {
    let source = "expect(clamp(-5, 0, 10)).toBe(expected);";
    let allocator = Allocator::default();
    let parse_result = Parser::new(&allocator, source, SourceType::ts()).parse();
    let assertions =
        collect_expect_assertions_in_statements(&parse_result.program.body, source, None);
    assert_eq!(assertions.len(), 1);
    let assertion = &assertions[0];
    assert_eq!(assertion.matcher, "toBe");
    assert_eq!(
        assertion.observed_expression.as_deref(),
        Some("clamp(-5, 0, 10)")
    );
    assert!(assertion.expected_value_or_variant.is_none());
    assert!(assertion.has_dynamic_matcher_arg);
    assert_eq!(assertion.oracle_confidence, OracleConfidence::Medium);
}

/// Oracle metadata: `has_dynamic_matcher_arg = true` and no expected value when
/// the matcher argument is a function call expression.
#[test]
fn oracle_metadata_has_dynamic_matcher_arg_for_call_expression() {
    let source = "expect(getValue()).toBe(computeExpected(0));";
    let allocator = Allocator::default();
    let parse_result = Parser::new(&allocator, source, SourceType::ts()).parse();
    let assertions =
        collect_expect_assertions_in_statements(&parse_result.program.body, source, None);
    assert_eq!(assertions.len(), 1);
    let assertion = &assertions[0];
    assert!(assertion.has_dynamic_matcher_arg);
    assert!(assertion.expected_value_or_variant.is_none());
}

/// Oracle metadata: no `has_dynamic_matcher_arg` for matchers that take no argument.
#[test]
fn oracle_metadata_no_dynamic_flag_for_no_arg_matchers() {
    let source = "expect(result).toBeTruthy();\nexpect(fn).toThrow();";
    let allocator = Allocator::default();
    let parse_result = Parser::new(&allocator, source, SourceType::ts()).parse();
    let assertions =
        collect_expect_assertions_in_statements(&parse_result.program.body, source, None);
    for assertion in &assertions {
        assert!(
            !assertion.has_dynamic_matcher_arg,
            "no-arg matchers should not set has_dynamic_matcher_arg: {assertion:?}"
        );
    }
}

/// AVA `t.is(actual, expected)` is recognized as an exact-value oracle when the
/// receiver matches the test callback's first parameter. Observed = arg 0
/// (`actual`), expected = arg 1 literal, mirroring Jest's
/// `expect(actual).toBe(expected)`.
#[test]
fn ava_is_assertion_extracts_exact_value_oracle() {
    let source = "t.is(score(10, 3), 7);";
    let allocator = Allocator::default();
    let parse_result = Parser::new(&allocator, source, SourceType::ts()).parse();
    let assertions =
        collect_expect_assertions_in_statements(&parse_result.program.body, source, Some("t"));
    assert_eq!(assertions.len(), 1, "should extract one AVA assertion");
    let assertion = &assertions[0];
    assert_eq!(assertion.matcher, "is");
    assert_eq!(assertion.oracle_kind, OracleKind::ExactValue);
    assert_eq!(assertion.oracle_strength, OracleStrength::Strong);
    assert_eq!(
        assertion.observed_expression.as_deref(),
        Some("score(10, 3)")
    );
    assert_eq!(assertion.expected_value_or_variant.as_deref(), Some("7"));
    assert!(!assertion.has_dynamic_matcher_arg);
}

/// AVA `t.not(actual, expected)` reaches the observed value but only proves a
/// non-equality relation. It must stay weak relational evidence rather than a
/// strong exact-value oracle.
#[test]
fn ava_not_assertion_extracts_relational_oracle() {
    let source = "t.not(score(10, 3), 8);";
    let allocator = Allocator::default();
    let parse_result = Parser::new(&allocator, source, SourceType::ts()).parse();
    let assertions =
        collect_expect_assertions_in_statements(&parse_result.program.body, source, Some("t"));
    assert_eq!(assertions.len(), 1, "should extract one AVA assertion");
    let assertion = &assertions[0];
    assert_eq!(assertion.matcher, "not");
    assert_eq!(assertion.oracle_kind, OracleKind::RelationalCheck);
    assert_eq!(assertion.oracle_strength, OracleStrength::Weak);
    assert_eq!(
        assertion.observed_expression.as_deref(),
        Some("score(10, 3)")
    );
    assert_eq!(assertion.expected_value_or_variant.as_deref(), Some("8"));
    assert_eq!(assertion_oracle_text(assertion), "t.not(...)");
    assert!(!assertion.has_dynamic_matcher_arg);
}

/// End-to-end: a full AVA `test('name', t => { t.is(...) })` call has its
/// callback receiver (`t`) extracted and threaded so the inner `t.is(...)` is
/// credited as an exact-value oracle.
#[test]
fn ava_test_call_threads_callback_receiver() {
    let source = "test('scores the difference', t => { t.is(score(10, 3), 7); });";
    let allocator = Allocator::default();
    let parse_result = Parser::new(&allocator, source, SourceType::ts()).parse();
    let call = parse_result
        .program
        .body
        .iter()
        .find_map(|stmt| match stmt {
            Statement::ExpressionStatement(stmt) => match &stmt.expression {
                Expression::CallExpression(call) => Some(call),
                _ => None,
            },
            _ => None,
        });
    assert!(call.is_some(), "expected a test() call expression");
    let Some(call) = call else { return };
    let result = test_name_and_assertions_from_call(call, source);
    assert!(result.is_some(), "should recognize the AVA test call");
    let Some((name, assertions)) = result else {
        return;
    };
    assert_eq!(name, "scores the difference");
    assert_eq!(assertions.len(), 1, "AVA assertion should be threaded");
    assert_eq!(assertions[0].oracle_kind, OracleKind::ExactValue);
    assert_eq!(assertions[0].oracle_strength, OracleStrength::Strong);
}

/// Fail-closed: an AVA assertion is only credited when its receiver is the test
/// callback's parameter. A same-named method on an unrelated object
/// (`helper.is(...)`) is NOT an AVA assertion.
#[test]
fn ava_assertion_requires_matching_receiver() {
    let source = "helper.is(score(10, 3), 7);";
    let allocator = Allocator::default();
    let parse_result = Parser::new(&allocator, source, SourceType::ts()).parse();
    let assertions =
        collect_expect_assertions_in_statements(&parse_result.program.body, source, Some("t"));
    assert!(
        assertions.is_empty(),
        "wrong receiver must not be credited as an AVA assertion: {assertions:?}"
    );
}

/// Fail-closed: an unrecognized method on the AVA receiver yields no oracle (no
/// assertion), so an unknown discriminator is never over-credited.
#[test]
fn ava_unknown_method_not_credited() {
    let source = "t.frobnicate(score(10, 3), 7);";
    let allocator = Allocator::default();
    let parse_result = Parser::new(&allocator, source, SourceType::ts()).parse();
    let assertions =
        collect_expect_assertions_in_statements(&parse_result.program.body, source, Some("t"));
    assert!(
        assertions.is_empty(),
        "unknown AVA method must not be credited: {assertions:?}"
    );
}

/// AVA `t.truthy(...)` is a smoke-only oracle — it reaches but does not pin the
/// exact changed value, so it must not be promoted to a strong exact oracle.
#[test]
fn ava_truthy_is_smoke_only() {
    let source = "t.truthy(score(10, 3));";
    let allocator = Allocator::default();
    let parse_result = Parser::new(&allocator, source, SourceType::ts()).parse();
    let assertions =
        collect_expect_assertions_in_statements(&parse_result.program.body, source, Some("t"));
    assert_eq!(assertions.len(), 1);
    assert_eq!(assertions[0].oracle_kind, OracleKind::SmokeOnly);
    assert_eq!(assertions[0].oracle_strength, OracleStrength::Smoke);
}

/// Tape / node:test positive equality aliases use the same receiver-gated path
/// as AVA: `t.equal(...)`, `t.strictEqual(...)`, and positive deep-equality
/// forms are exact-value oracles when the receiver matches the test callback
/// parameter.
#[test]
fn tape_equal_aliases_extract_exact_value_oracles() {
    for method in ["equal", "strictEqual", "deepEqual"] {
        let source = format!("t.{method}(score(10, 3), 7);");
        let allocator = Allocator::default();
        let parse_result = Parser::new(&allocator, &source, SourceType::ts()).parse();
        let assertions =
            collect_expect_assertions_in_statements(&parse_result.program.body, &source, Some("t"));
        assert_eq!(
            assertions.len(),
            1,
            "{method} should extract one receiver-gated assertion"
        );
        let assertion = &assertions[0];
        assert_eq!(assertion.matcher, method);
        assert_eq!(assertion.oracle_kind, OracleKind::ExactValue);
        assert_eq!(assertion.oracle_strength, OracleStrength::Strong);
        assert_eq!(
            assertion.observed_expression.as_deref(),
            Some("score(10, 3)")
        );
        assert_eq!(assertion.expected_value_or_variant.as_deref(), Some("7"));
        assert_eq!(assertion_oracle_text(assertion), format!("t.{method}(...)"));
        assert!(!assertion.has_dynamic_matcher_arg);
    }
}

/// Tape / node:test negated equality aliases are not exact-value oracles. They
/// observe that the value is not equal to another value, so they stay weak
/// relational evidence.
#[test]
fn tape_negated_equal_aliases_extract_relational_oracles() {
    for method in ["notEqual", "notStrictEqual", "notDeepEqual"] {
        let source = format!("t.{method}(score(10, 3), 8);");
        let allocator = Allocator::default();
        let parse_result = Parser::new(&allocator, &source, SourceType::ts()).parse();
        let assertions =
            collect_expect_assertions_in_statements(&parse_result.program.body, &source, Some("t"));
        assert_eq!(
            assertions.len(),
            1,
            "{method} should extract one receiver-gated assertion"
        );
        let assertion = &assertions[0];
        assert_eq!(assertion.matcher, method);
        assert_eq!(assertion.oracle_kind, OracleKind::RelationalCheck);
        assert_eq!(assertion.oracle_strength, OracleStrength::Weak);
        assert_eq!(
            assertion.observed_expression.as_deref(),
            Some("score(10, 3)")
        );
        assert_eq!(assertion.expected_value_or_variant.as_deref(), Some("8"));
        assert_eq!(assertion_oracle_text(assertion), format!("t.{method}(...)"));
        assert!(!assertion.has_dynamic_matcher_arg);
    }
}

/// Tape `t.ok(...)` / `t.notOk(...)` reach the value but do not pin the changed
/// discriminator, so they remain smoke-only.
#[test]
fn tape_ok_aliases_are_smoke_only() {
    for method in ["ok", "notOk"] {
        let source = format!("t.{method}(score(10, 3));");
        let allocator = Allocator::default();
        let parse_result = Parser::new(&allocator, &source, SourceType::ts()).parse();
        let assertions =
            collect_expect_assertions_in_statements(&parse_result.program.body, &source, Some("t"));
        assert_eq!(
            assertions.len(),
            1,
            "{method} should extract one receiver-gated assertion"
        );
        let assertion = &assertions[0];
        assert_eq!(assertion.matcher, method);
        assert_eq!(assertion.oracle_kind, OracleKind::SmokeOnly);
        assert_eq!(assertion.oracle_strength, OracleStrength::Smoke);
        assert_eq!(
            assertion.observed_expression.as_deref(),
            Some("score(10, 3)")
        );
        assert!(assertion.expected_value_or_variant.is_none());
        assert!(!assertion.has_dynamic_matcher_arg);
    }
}

/// Without a receiver (Jest/Vitest callbacks take no execution context), AVA
/// matching is never attempted — `t.is(...)` here is just an unrelated call.
#[test]
fn ava_assertion_not_attempted_without_receiver() {
    let source = "t.is(score(10, 3), 7);";
    let allocator = Allocator::default();
    let parse_result = Parser::new(&allocator, source, SourceType::ts()).parse();
    let assertions =
        collect_expect_assertions_in_statements(&parse_result.program.body, source, None);
    assert!(
        assertions.is_empty(),
        "no receiver means no AVA assertion: {assertions:?}"
    );
}

/// `typescript_dynamic_assertion_unresolved` limitation emitted when a direct
/// oracle-eligible related test has `has_dynamic_matcher_arg = true`.
#[test]
fn named_limitation_dynamic_assertion_emitted_for_dynamic_matcher_arg() -> Result<(), String> {
    let owner = test_owner("clamp", "src/clamp.ts");
    // Manually construct a test with a dynamic matcher arg
    let test = TypeScriptTest {
        name: "clamps below minimum".to_string(),
        local_name: "clamps below minimum".to_string(),
        describe_names: Vec::new(),
        file: PathBuf::from("tests/clamp.test.ts"),
        line: 1,
        body_text: "const expected = computeExpected(0);\nclamp(-5, 0, 10);\nexpect(clamp(-5, 0, 10)).toBe(expected);".to_string(),
        assertions: vec![TypeScriptAssertion {
            matcher: "toBe".to_string(),
            argument_count: 1,
            line: 3,
            oracle_kind: OracleKind::ExactValue,
            oracle_strength: OracleStrength::Strong,
            mock_payload: None,
            error_payload: None,
            observed_expression: Some("clamp(-5, 0, 10)".to_string()),
            expected_value_or_variant: None,
            has_dynamic_matcher_arg: true,
            oracle_confidence: OracleConfidence::Medium,
        }],
        mocks_in_file: Vec::new(),
        imports_in_file: Vec::new(),
    };
    let finding = classify_change(
        Path::new("src/clamp.ts"),
        2,
        "    if (value < min) {",
        &[owner],
        &[test],
        None,
        &ReExportIndex::empty(),
        None,
    )
    .ok_or_else(|| "expected a finding".to_string())?;

    assert_evidence_contains(
        &finding,
        "typescript_limitation: typescript_dynamic_assertion_unresolved",
    );
    assert_evidence_lacks(
        &finding,
        "typescript_limitation: typescript_table_case_unresolved",
    );
    assert_evidence_contains(
        &finding,
        "typescript_limitation_sample: typescript_dynamic_assertion_unresolved at tests/clamp.test.ts:3",
    );
    assert_evidence_contains(&finding, "typescript_oracle_observed: clamp(-5, 0, 10)");
    // No expected value — dynamic arg
    assert_evidence_lacks(&finding, "typescript_oracle_expected:");
    assert_evidence_contains(&finding, "typescript_oracle_confidence: medium");
    // repair_packet_ready stays false
    assert_evidence_contains(&finding, "repair_route:");
    Ok(())
}

/// `typescript_table_case_unresolved` limitation emitted when an oracle-eligible
/// `test.each` / `it.each` table case uses a row-derived dynamic matcher arg.
#[test]
fn named_limitation_table_case_emitted_for_table_dynamic_matcher_arg() -> Result<(), String> {
    let owner = test_owner("clamp", "src/clamp.ts");
    let test = TypeScriptTest {
        name: "clamps table %#".to_string(),
        local_name: "clamps table %#".to_string(),
        describe_names: Vec::new(),
        file: PathBuf::from("tests/clamp.test.ts"),
        line: 1,
        body_text: "test.each([[ -5, 0 ]])(\"clamps table %#\", (value, expected) => {\nclamp(value, 0, 10);\nexpect(clamp(value, 0, 10)).toBe(expected);\n});".to_string(),
        assertions: vec![TypeScriptAssertion {
            matcher: "toBe".to_string(),
            argument_count: 1,
            line: 3,
            oracle_kind: OracleKind::ExactValue,
            oracle_strength: OracleStrength::Strong,
            mock_payload: None,
            error_payload: None,
            observed_expression: Some("clamp(value, 0, 10)".to_string()),
            expected_value_or_variant: None,
            has_dynamic_matcher_arg: true,
            oracle_confidence: OracleConfidence::Medium,
        }],
        mocks_in_file: Vec::new(),
        imports_in_file: Vec::new(),
    };
    let finding = classify_change(
        Path::new("src/clamp.ts"),
        2,
        "    if (value < min) {",
        &[owner],
        &[test],
        None,
        &ReExportIndex::empty(),
        None,
    )
    .ok_or_else(|| "expected a finding".to_string())?;

    assert_evidence_contains(
        &finding,
        "typescript_limitation: typescript_table_case_unresolved",
    );
    assert_evidence_contains(
        &finding,
        "typescript_limitation_sample: typescript_table_case_unresolved at tests/clamp.test.ts:3",
    );
    assert_evidence_contains(
        &finding,
        "typescript_limitation_repair_route: typescript_table_case_unresolved",
    );
    assert_evidence_contains(
        &finding,
        "typescript_limitation: typescript_dynamic_assertion_unresolved",
    );
    assert_evidence_lacks(&finding, "repair_packet_ready: true");
    Ok(())
}

/// `typescript_dynamic_assertion_unresolved` must NOT fire for heuristic-only
/// relations (not oracle-eligible).
#[test]
fn named_limitation_dynamic_assertion_not_emitted_for_heuristic_only_relation() -> Result<(), String>
{
    let owner = test_owner("clamp", "src/clamp.ts");
    // Heuristic: no "clamp(" in body_text → name-proximity only
    let test = TypeScriptTest {
        name: "clamp boundary".to_string(),
        local_name: "clamp boundary".to_string(),
        describe_names: Vec::new(),
        file: PathBuf::from("tests/clamp.test.ts"),
        line: 1,
        body_text: "const expected = computeExpected(0);\nexpect(result).toBe(expected);"
            .to_string(),
        assertions: vec![TypeScriptAssertion {
            matcher: "toBe".to_string(),
            argument_count: 1,
            line: 2,
            oracle_kind: OracleKind::ExactValue,
            oracle_strength: OracleStrength::Strong,
            mock_payload: None,
            error_payload: None,
            observed_expression: Some("result".to_string()),
            expected_value_or_variant: None,
            has_dynamic_matcher_arg: true,
            oracle_confidence: OracleConfidence::Medium,
        }],
        mocks_in_file: Vec::new(),
        imports_in_file: Vec::new(),
    };
    let finding = classify_change(
        Path::new("src/clamp.ts"),
        2,
        "    if (value < min) {",
        &[owner],
        &[test],
        None,
        &ReExportIndex::empty(),
        None,
    )
    .ok_or_else(|| "expected a finding".to_string())?;

    // Heuristic relation → no dynamic assertion limitation
    assert_evidence_lacks(
        &finding,
        "typescript_limitation: typescript_dynamic_assertion_unresolved",
    );
    Ok(())
}

/// Oracle metadata confidence is `high` when oracle strength is Strong and
/// expected value is a concrete literal.
#[test]
fn oracle_confidence_high_when_strong_oracle_and_literal_expected() {
    assert_eq!(
        derive_oracle_confidence(&OracleStrength::Strong, &Some("42".to_string()), "toBe"),
        OracleConfidence::High
    );
}

/// Oracle metadata confidence is `medium` when strong oracle but no literal.
#[test]
fn oracle_confidence_medium_when_strong_oracle_and_no_literal() {
    assert_eq!(
        derive_oracle_confidence(&OracleStrength::Strong, &None, "toBe"),
        OracleConfidence::Medium
    );
}

/// Oracle metadata confidence is `medium` for Medium oracle strength.
#[test]
fn oracle_confidence_medium_for_medium_strength() {
    assert_eq!(
        derive_oracle_confidence(&OracleStrength::Medium, &None, "toMatchSnapshot"),
        OracleConfidence::Medium
    );
}

/// Oracle metadata confidence is `low` for Weak and Smoke oracle strengths.
#[test]
fn oracle_confidence_low_for_weak_and_smoke() {
    assert_eq!(
        derive_oracle_confidence(&OracleStrength::Weak, &None, "toContain"),
        OracleConfidence::Low
    );
    assert_eq!(
        derive_oracle_confidence(&OracleStrength::Smoke, &None, "toBeTruthy"),
        OracleConfidence::Low
    );
}

// ──────────────────────────────────────────────────────────────────────────────
// PR6: Ownership hardening — package-local, import forms, typescript_target_unresolved
// ──────────────────────────────────────────────────────────────────────────────

/// Package-local filter: a test in the SAME package as the owner IS selected.
#[test]
fn package_local_filter_selects_same_package_test() {
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.subsec_nanos())
        .unwrap_or(0);
    let root = std::env::temp_dir().join(format!("ripr-pkg-local-{stamp}"));
    // pkg-a directory with its own package.json
    let pkg_a = root.join("packages").join("pkg-a");
    let _ = fs::create_dir_all(pkg_a.join("src"));
    let _ = fs::create_dir_all(pkg_a.join("tests"));
    let _ = fs::write(
        pkg_a.join("package.json"),
        r#"{"name":"pkg-a","devDependencies":{"jest":"^29"}}"#,
    );

    let owner = TypeScriptOwner {
        name: "doWork".to_string(),
        file: pkg_a.join("src").join("work.ts"),
        start_line: 1,
        end_line: 10,
        owner_kind: OwnerKind::Function,
        class_name: None,
        decorated: false,
        exported_as_default: false,
        class_default_export: false,
        arity: None,
        parameters: Vec::new(),
        source_text: None,
        imports: Vec::new(),
        method_kind: TypeScriptMethodKind::Ordinary,
    };
    let test = TypeScriptTest {
        name: "do work test".to_string(),
        local_name: "do work test".to_string(),
        describe_names: Vec::new(),
        file: pkg_a.join("tests").join("work.test.ts"),
        line: 1,
        body_text: "doWork();".to_string(),
        assertions: Vec::new(),
        mocks_in_file: Vec::new(),
        imports_in_file: Vec::new(),
    };

    let tests_slice = [test.clone()];
    let candidates = related_test_candidates(
        &owner,
        &tests_slice,
        Some(&root),
        &ReExportIndex::empty(),
        None,
    );
    // The package-local filter must NOT exclude same-package tests.
    // (candidates may still be empty if body_text has no call — that's tested
    // elsewhere; here we only verify the package filter is not the bottleneck.)
    // Use workspace_root=None to check the unfiltered count equals
    // workspace_root=Some count (filter didn't discard it).
    let candidates_no_filter =
        related_test_candidates(&owner, &tests_slice, None, &ReExportIndex::empty(), None);
    assert_eq!(
        candidates.len(),
        candidates_no_filter.len(),
        "package-local filter must not discard same-package tests"
    );
}

/// Package-local filter: a test in a DIFFERENT package must NOT be selected.
#[test]
fn package_local_filter_rejects_cross_package_test() {
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.subsec_nanos())
        .unwrap_or(0);
    let root = std::env::temp_dir().join(format!("ripr-pkg-cross-{stamp}"));
    let pkg_a = root.join("packages").join("pkg-a");
    let pkg_b = root.join("packages").join("pkg-b");
    let _ = fs::create_dir_all(pkg_a.join("src"));
    let _ = fs::create_dir_all(pkg_b.join("tests"));
    let _ = fs::write(pkg_a.join("package.json"), r#"{"name":"pkg-a"}"#);
    let _ = fs::write(pkg_b.join("package.json"), r#"{"name":"pkg-b"}"#);

    let owner = TypeScriptOwner {
        name: "doWork".to_string(),
        file: pkg_a.join("src").join("work.ts"),
        start_line: 1,
        end_line: 10,
        owner_kind: OwnerKind::Function,
        class_name: None,
        decorated: false,
        exported_as_default: false,
        class_default_export: false,
        arity: None,
        parameters: Vec::new(),
        source_text: None,
        imports: Vec::new(),
        method_kind: TypeScriptMethodKind::Ordinary,
    };
    let test = TypeScriptTest {
        name: "cross-package doWork test".to_string(),
        local_name: "cross-package doWork test".to_string(),
        describe_names: Vec::new(),
        file: pkg_b.join("tests").join("work.test.ts"),
        line: 1,
        body_text: "doWork();".to_string(),
        assertions: Vec::new(),
        mocks_in_file: Vec::new(),
        imports_in_file: Vec::new(),
    };

    let tests_slice = [test.clone()];
    let candidates = related_test_candidates(
        &owner,
        &tests_slice,
        Some(&root),
        &ReExportIndex::empty(),
        None,
    );
    assert!(
        candidates.is_empty(),
        "cross-package test must NOT be selected, got {candidates:?}"
    );
}

/// CommonJS require() destructuring: `const { fn } = require('./path')` should
/// be extracted as an import with `imported = Some("fn")`, `local = "fn"`,
/// `namespace = false`.
#[test]
fn extract_imports_recognizes_commonjs_destructured_require() {
    let source = r#"const { formatAmount, parseAmount: parse } = require('../src/format');
const mod = require('../src/utils');
"#;
    let allocator = Allocator::default();
    let parse_result = Parser::new(&allocator, source, SourceType::mjs()).parse();
    let imports = extract_imports_from_statements(&parse_result.program.body);

    // Destructured: formatAmount
    let fmt_matches: Vec<_> = imports
        .iter()
        .filter(|i| i.local == "formatAmount")
        .collect();
    assert_eq!(
        fmt_matches.len(),
        1,
        "expected exactly 1 formatAmount import, got: {imports:?}"
    );
    assert_eq!(fmt_matches[0].source, "../src/format");
    assert_eq!(fmt_matches[0].imported.as_deref(), Some("formatAmount"));
    assert!(!fmt_matches[0].namespace);

    // Renamed destructure: parseAmount → parse
    let parse_matches: Vec<_> = imports.iter().filter(|i| i.local == "parse").collect();
    assert_eq!(
        parse_matches.len(),
        1,
        "expected exactly 1 parse import, got: {imports:?}"
    );
    assert_eq!(parse_matches[0].source, "../src/format");
    assert_eq!(parse_matches[0].imported.as_deref(), Some("parseAmount"));
    assert!(!parse_matches[0].namespace);

    // Namespace-like require: const mod = require(...)
    let mod_matches: Vec<_> = imports.iter().filter(|i| i.local == "mod").collect();
    assert_eq!(
        mod_matches.len(),
        1,
        "expected exactly 1 mod import, got: {imports:?}"
    );
    assert_eq!(mod_matches[0].source, "../src/utils");
    assert!(mod_matches[0].namespace);
}

/// Re-export form: `export { x } from './y'` should be extracted as an import
/// so that `import_source_matches_owner` can follow the re-export one hop.
#[test]
fn extract_imports_recognizes_re_export_form() {
    let source = r#"export { applyDiscount, computeBase as base } from './pricing';
"#;
    let allocator = Allocator::default();
    let parse_result = Parser::new(&allocator, source, SourceType::ts()).parse();
    let imports = extract_imports_from_statements(&parse_result.program.body);

    // Re-exported applyDiscount
    let ad_matches: Vec<_> = imports
        .iter()
        .filter(|i| i.local == "applyDiscount")
        .collect();
    assert_eq!(
        ad_matches.len(),
        1,
        "expected applyDiscount re-export import, got: {imports:?}"
    );
    assert_eq!(ad_matches[0].source, "./pricing");
    assert_eq!(ad_matches[0].imported.as_deref(), Some("applyDiscount"));
    assert!(!ad_matches[0].namespace);

    // Renamed re-export: computeBase exported as base
    let base_matches: Vec<_> = imports.iter().filter(|i| i.local == "base").collect();
    assert_eq!(
        base_matches.len(),
        1,
        "expected base re-export import, got: {imports:?}"
    );
    assert_eq!(base_matches[0].source, "./pricing");
    assert_eq!(base_matches[0].imported.as_deref(), Some("computeBase"));
}

/// `typescript_target_unresolved` is emitted when a cross-package test
/// references the owner by name but is excluded by the package-local filter.
#[test]
fn named_limitation_target_unresolved_emitted_for_cross_package_reference() -> Result<(), String> {
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.subsec_nanos())
        .unwrap_or(0);
    let root = std::env::temp_dir().join(format!("ripr-target-unresolved-{stamp}"));
    let pkg_a = root.join("packages").join("pkg-a");
    let pkg_b = root.join("packages").join("pkg-b");
    let _ = fs::create_dir_all(pkg_a.join("src"));
    let _ = fs::create_dir_all(pkg_b.join("tests"));
    let _ = fs::write(
        pkg_a.join("package.json"),
        r#"{"name":"pkg-a","devDependencies":{"jest":"^29"}}"#,
    );
    let _ = fs::write(
        pkg_b.join("package.json"),
        r#"{"name":"pkg-b","devDependencies":{"jest":"^29"}}"#,
    );

    let owner = TypeScriptOwner {
        name: "applyDiscount".to_string(),
        file: pkg_a.join("src").join("discount.ts"),
        start_line: 1,
        end_line: 10,
        owner_kind: OwnerKind::Function,
        class_name: None,
        decorated: false,
        exported_as_default: false,
        class_default_export: false,
        arity: None,
        parameters: Vec::new(),
        source_text: None,
        imports: Vec::new(),
        method_kind: TypeScriptMethodKind::Ordinary,
    };
    // Cross-package test that calls applyDiscount by name (local shadow or
    // referencing it without a resolvable import).
    let cross_pkg_test = TypeScriptTest {
        name: "cross-pkg discount test".to_string(),
        local_name: "cross-pkg discount test".to_string(),
        describe_names: Vec::new(),
        file: pkg_b.join("tests").join("pricing.test.ts"),
        line: 5,
        body_text: "const result = applyDiscount(100, 20);\nexpect(result).toBe(80);".to_string(),
        assertions: vec![TypeScriptAssertion {
            matcher: "toBe".to_string(),
            argument_count: 1,
            line: 6,
            oracle_kind: OracleKind::ExactValue,
            oracle_strength: OracleStrength::Strong,
            mock_payload: None,
            error_payload: None,
            observed_expression: None,
            expected_value_or_variant: None,
            has_dynamic_matcher_arg: false,
            oracle_confidence: OracleConfidence::High,
        }],
        mocks_in_file: Vec::new(),
        imports_in_file: Vec::new(),
    };
    // Same-package test that correctly imports
    let same_pkg_test = TypeScriptTest {
        name: "same-pkg discount test".to_string(),
        local_name: "same-pkg discount test".to_string(),
        describe_names: Vec::new(),
        file: pkg_a.join("tests").join("discount.test.ts"),
        line: 1,
        body_text: "applyDiscount(100, 20);".to_string(),
        assertions: Vec::new(),
        mocks_in_file: Vec::new(),
        imports_in_file: Vec::new(),
    };

    let all_owners = vec![owner.clone()];
    let all_tests = vec![cross_pkg_test, same_pkg_test];
    let finding = classify_change(
        &pkg_a.join("src").join("discount.ts"),
        2,
        "    if (discountPct >= 100) {",
        &all_owners,
        &all_tests,
        Some(&root),
        &ReExportIndex::empty(),
        None,
    )
    .ok_or_else(|| "expected a finding".to_string())?;

    // The typescript_target_unresolved limitation must be emitted for the
    // cross-package test reference.
    assert_evidence_contains(
        &finding,
        "typescript_limitation: typescript_target_unresolved",
    );
    assert_evidence_contains(
        &finding,
        "typescript_limitation_why: typescript_target_unresolved",
    );
    assert_evidence_contains(
        &finding,
        "typescript_limitation_repair_route: typescript_target_unresolved → analysis/typescript-cross-package-ownership",
    );
    // repair_packet_ready stays false
    assert_evidence_lacks(&finding, "repair_packet_ready: true");
    Ok(())
}

/// `typescript_target_unresolved` must NOT be emitted when all tests are in
/// the same package (single-package workspace without a package.json hierarchy
/// does not trigger cross-package detection).
#[test]
fn named_limitation_target_unresolved_not_emitted_for_same_package() -> Result<(), String> {
    let owner = test_owner("applyDiscount", "src/discount.ts");
    let test = TypeScriptTest {
        name: "discount applies".to_string(),
        local_name: "discount applies".to_string(),
        describe_names: Vec::new(),
        file: PathBuf::from("tests/discount.test.ts"),
        line: 1,
        body_text: "applyDiscount(100, 20);".to_string(),
        assertions: Vec::new(),
        mocks_in_file: Vec::new(),
        imports_in_file: Vec::new(),
    };
    // No workspace_root → no package-local filter → no typescript_target_unresolved
    let finding = classify_change(
        Path::new("src/discount.ts"),
        2,
        "    if (discountPct >= 100) {",
        &[owner],
        &[test],
        None,
        &ReExportIndex::empty(),
        None,
    )
    .ok_or_else(|| "expected a finding".to_string())?;

    assert_evidence_lacks(
        &finding,
        "typescript_limitation: typescript_target_unresolved",
    );
    Ok(())
}

// ── RIPR-SPEC-0098: Observation guard tests ───────────────────────────────────

/// Repro fixture (RIPR-SPEC-0098 §fixture-1): a changed `console.log("audit",
/// amount*9)` line is a SideEffect whose value never escapes.  Two unrelated
/// `toBe` assertions in the test body assert the UNCHANGED return value of
/// `applyDiscount(...)`.  Before the fix, this produced `class:"exposed"`.
/// After the fix it MUST produce `class:"weakly_exposed"` with a
/// `propagation_unknown` limitation and `discriminate != yes`.
#[test]
fn ts_swallowed_console_log_exposed_downgrade() -> Result<(), String> {
    let owner = TypeScriptOwner {
        name: "applyDiscount".to_string(),
        file: PathBuf::from("src/discount.ts"),
        start_line: 1,
        end_line: 10,
        owner_kind: OwnerKind::Function,
        class_name: None,
        decorated: false,
        exported_as_default: false,
        class_default_export: false,
        arity: None,
        parameters: Vec::new(),
        source_text: None,
        imports: Vec::new(),
        method_kind: TypeScriptMethodKind::Ordinary,
    };
    // Two strong `toBe` assertions on the UNCHANGED return value.
    // Neither `observed_expression` mentions `amount` or `audit` (the changed
    // tokens), so the observation guard MUST fail.
    let test = TypeScriptTest {
        name: "applies discount correctly".to_string(),
        local_name: "applies discount correctly".to_string(),
        describe_names: Vec::new(),
        file: PathBuf::from("tests/discount.test.ts"),
        line: 1,
        body_text: "applyDiscount(100, 10);\nexpect(applyDiscount(100, 10)).toBe(90);\nexpect(applyDiscount(50, 10)).toBe(45);".to_string(),
        assertions: vec![
            TypeScriptAssertion {
                matcher: "toBe".to_string(),
                argument_count: 1,
                line: 2,
                oracle_kind: OracleKind::ExactValue,
                oracle_strength: OracleStrength::Strong,
                mock_payload: None,
                error_payload: None,
                // observed_expression is the return value call — NOT the audit expression
                observed_expression: Some("applyDiscount(100, 10)".to_string()),
                expected_value_or_variant: Some("90".to_string()),
                has_dynamic_matcher_arg: false,
                oracle_confidence: OracleConfidence::High,
            },
            TypeScriptAssertion {
                matcher: "toBe".to_string(),
                argument_count: 1,
                line: 3,
                oracle_kind: OracleKind::ExactValue,
                oracle_strength: OracleStrength::Strong,
                mock_payload: None,
                error_payload: None,
                observed_expression: Some("applyDiscount(50, 10)".to_string()),
                expected_value_or_variant: Some("45".to_string()),
                has_dynamic_matcher_arg: false,
                oracle_confidence: OracleConfidence::High,
            },
        ],
        mocks_in_file: Vec::new(),
        imports_in_file: Vec::new(),
    };
    // Changed line: SideEffect (console.log call) — the `amount` and `audit`
    // tokens are not in any assertion's observed_expression.
    let finding = classify_change(
        Path::new("src/discount.ts"),
        4,
        "    console.log(\"audit\", amount * 9);",
        &[owner],
        &[test],
        None,
        &ReExportIndex::empty(),
        None,
    )
    .ok_or_else(|| "expected a finding".to_string())?;

    // MUST be downgraded to weakly_exposed — not exposed.
    assert!(
        matches!(finding.class, ExposureClass::WeaklyExposed),
        "expected WeaklyExposed after observation guard, got {:?}",
        finding.class
    );
    // discriminate MUST NOT be Yes.
    assert!(
        !matches!(finding.ripr.reveal.discriminate.state, StageState::Yes),
        "discriminate must not be Yes after observation guard, got {:?}",
        finding.ripr.reveal.discriminate.state
    );
    // Must have a propagation_unknown limitation.
    let all_text: String = finding.missing.join("\n");
    assert!(
        all_text.contains("propagation_unknown"),
        "expected propagation_unknown in missing, got: {all_text:?}"
    );
    Ok(())
}

/// Control fixture (RIPR-SPEC-0098 §fixture-2): the OWNER return arithmetic
/// is changed (`amount - 10` → changed line), and the test asserts
/// `expect(applyDiscount(100, 100)).toBe(90)`.  The `observed_expression` is
/// `applyDiscount(100, 100)` which contains the owner name.  The observation
/// guard MUST confirm (owner-call observation on a ReturnValue family), so the
/// finding MUST remain `class:exposed, discriminate:yes`.
///
/// This is the over-correction control: future tightening that accidentally
/// downgrades a genuine whole-value return discriminator will fail here.
#[test]
fn ts_returnvalue_genuinely_observed_control() -> Result<(), String> {
    let owner = TypeScriptOwner {
        name: "applyDiscount".to_string(),
        file: PathBuf::from("src/discount.ts"),
        start_line: 1,
        end_line: 8,
        owner_kind: OwnerKind::Function,
        class_name: None,
        decorated: false,
        exported_as_default: false,
        class_default_export: false,
        arity: None,
        parameters: Vec::new(),
        source_text: None,
        imports: Vec::new(),
        method_kind: TypeScriptMethodKind::Ordinary,
    };
    let test = TypeScriptTest {
        name: "applies discount".to_string(),
        local_name: "applies discount".to_string(),
        describe_names: Vec::new(),
        file: PathBuf::from("tests/discount.test.ts"),
        line: 1,
        body_text: "expect(applyDiscount(100, 100)).toBe(90);".to_string(),
        assertions: vec![TypeScriptAssertion {
            matcher: "toBe".to_string(),
            argument_count: 1,
            line: 2,
            oracle_kind: OracleKind::ExactValue,
            oracle_strength: OracleStrength::Strong,
            mock_payload: None,
            error_payload: None,
            // Owner name is IN the observed_expression — confirms ReturnValue.
            observed_expression: Some("applyDiscount(100, 100)".to_string()),
            expected_value_or_variant: Some("90".to_string()),
            has_dynamic_matcher_arg: false,
            oracle_confidence: OracleConfidence::High,
        }],
        mocks_in_file: Vec::new(),
        imports_in_file: Vec::new(),
    };
    let finding = classify_change(
        Path::new("src/discount.ts"),
        3,
        "  return amount - 12;",
        &[owner],
        &[test],
        None,
        &ReExportIndex::empty(),
        None,
    )
    .ok_or_else(|| "expected a finding".to_string())?;

    // MUST stay exposed — the owner-call observation confirms the return value.
    assert!(
        matches!(finding.class, ExposureClass::Exposed),
        "expected Exposed (owner-call observation on ReturnValue), got {:?}",
        finding.class
    );
    assert!(
        matches!(finding.ripr.reveal.discriminate.state, StageState::Yes),
        "expected discriminate==Yes, got {:?}",
        finding.ripr.reveal.discriminate.state
    );
    Ok(())
}

/// Repro control (RIPR-SPEC-0098 value-sink extension): a ReturnValue seam
/// whose confirming strong assertion observes an UNRELATED expression.
/// The test calls the changed owner (`applyDiscount(100, 10);` →
/// DirectOwnerCall relation) but its only strong assertion is
/// `expect(formatDate(now)).toBe('2024-01-01')` — neither the owner name nor
/// a changed token (`amount`) appears in the observed_expression. Before the
/// value-sink extension this classified `exposed`; now the observation guard
/// MUST fail closed and downgrade to `weakly_exposed` with a
/// `propagation_unknown` limitation.
#[test]
fn ts_returnvalue_unrelated_strong_assertion_downgrades() -> Result<(), String> {
    let owner = TypeScriptOwner {
        name: "applyDiscount".to_string(),
        file: PathBuf::from("src/discount.ts"),
        start_line: 1,
        end_line: 8,
        owner_kind: OwnerKind::Function,
        class_name: None,
        decorated: false,
        exported_as_default: false,
        class_default_export: false,
        imports: Vec::new(),
        method_kind: TypeScriptMethodKind::Ordinary,
        arity: None,
        parameters: Vec::new(),
        source_text: None,
    };
    let test = TypeScriptTest {
        name: "discount side checks".to_string(),
        local_name: "discount side checks".to_string(),
        describe_names: Vec::new(),
        file: PathBuf::from("tests/discount.test.ts"),
        line: 1,
        body_text: "applyDiscount(100, 10);\nexpect(formatDate(now)).toBe('2024-01-01');"
            .to_string(),
        assertions: vec![TypeScriptAssertion {
            matcher: "toBe".to_string(),
            argument_count: 1,
            line: 2,
            oracle_kind: OracleKind::ExactValue,
            oracle_strength: OracleStrength::Strong,
            mock_payload: None,
            error_payload: None,
            // Unrelated expression: no owner name, no changed token (`amount`).
            observed_expression: Some("formatDate(now)".to_string()),
            expected_value_or_variant: Some("'2024-01-01'".to_string()),
            has_dynamic_matcher_arg: false,
            oracle_confidence: OracleConfidence::High,
        }],
        mocks_in_file: Vec::new(),
        imports_in_file: Vec::new(),
    };
    let finding = classify_change(
        Path::new("src/discount.ts"),
        3,
        "  return amount - 12;",
        &[owner],
        &[test],
        None,
        &ReExportIndex::empty(),
        None,
    )
    .ok_or_else(|| "expected a finding".to_string())?;

    // MUST downgrade: the strong assertion observes an unrelated expression.
    assert!(
        matches!(finding.class, ExposureClass::WeaklyExposed),
        "expected WeaklyExposed (unrelated strong assertion on ReturnValue), got {:?}",
        finding.class
    );
    assert!(
        !matches!(finding.ripr.reveal.discriminate.state, StageState::Yes),
        "discriminate must not be Yes after value-sink observation guard, got {:?}",
        finding.ripr.reveal.discriminate.state
    );
    let all_text: String = finding.missing.join("\n");
    assert!(
        all_text.contains("propagation_unknown"),
        "expected propagation_unknown in missing, got: {all_text:?}"
    );
    Ok(())
}

/// Over-correction control (RIPR-SPEC-0098 value-sink extension): the exact
/// audit repro — changed line `return amount - 12;` — but this time the test
/// asserts the OWNER CALL (`expect(applyDiscount(100, 10)).toBe(88)`), so the
/// observed_expression references the owner and the guard MUST confirm. The
/// finding stays `class:exposed, discriminate:yes`.
#[test]
fn ts_returnvalue_owner_call_observation_stays_exposed() -> Result<(), String> {
    let owner = TypeScriptOwner {
        name: "applyDiscount".to_string(),
        file: PathBuf::from("src/discount.ts"),
        start_line: 1,
        end_line: 8,
        owner_kind: OwnerKind::Function,
        class_name: None,
        decorated: false,
        exported_as_default: false,
        class_default_export: false,
        imports: Vec::new(),
        method_kind: TypeScriptMethodKind::Ordinary,
        arity: None,
        parameters: Vec::new(),
        source_text: None,
    };
    let test = TypeScriptTest {
        name: "applies discount".to_string(),
        local_name: "applies discount".to_string(),
        describe_names: Vec::new(),
        file: PathBuf::from("tests/discount.test.ts"),
        line: 1,
        body_text: "expect(applyDiscount(100, 10)).toBe(88);".to_string(),
        assertions: vec![TypeScriptAssertion {
            matcher: "toBe".to_string(),
            argument_count: 1,
            line: 2,
            oracle_kind: OracleKind::ExactValue,
            oracle_strength: OracleStrength::Strong,
            mock_payload: None,
            error_payload: None,
            // Owner name is IN the observed_expression — confirms ReturnValue.
            observed_expression: Some("applyDiscount(100, 10)".to_string()),
            expected_value_or_variant: Some("88".to_string()),
            has_dynamic_matcher_arg: false,
            oracle_confidence: OracleConfidence::High,
        }],
        mocks_in_file: Vec::new(),
        imports_in_file: Vec::new(),
    };
    let finding = classify_change(
        Path::new("src/discount.ts"),
        3,
        "  return amount - 12;",
        &[owner],
        &[test],
        None,
        &ReExportIndex::empty(),
        None,
    )
    .ok_or_else(|| "expected a finding".to_string())?;

    // MUST stay exposed — the owner-call observation confirms the return value.
    assert!(
        matches!(finding.class, ExposureClass::Exposed),
        "expected Exposed (owner-call observation on ReturnValue), got {:?}",
        finding.class
    );
    assert!(
        matches!(finding.ripr.reveal.discriminate.state, StageState::Yes),
        "expected discriminate==Yes, got {:?}",
        finding.ripr.reveal.discriminate.state
    );
    Ok(())
}

/// One-hop aliasing control (RIPR-SPEC-0108 `must_stay_exposed` controls):
/// the canonical assert-the-return-value pattern captures the owner call in
/// a local and asserts the local — `const result = applyDiscount(100, 10);
/// expect(result).toBe(88)`. The bare-local `observed_expression` (`result`)
/// carries no owner reference, but the initializer does, so the guard MUST
/// confirm via the one-hop aliasing credit and keep the finding
/// `class:exposed, discriminate:yes` (no repair packet, no receipt command).
#[test]
fn ts_returnvalue_owner_aliased_local_observation_stays_exposed() -> Result<(), String> {
    let owner = TypeScriptOwner {
        name: "applyDiscount".to_string(),
        file: PathBuf::from("src/discount.ts"),
        start_line: 1,
        end_line: 8,
        owner_kind: OwnerKind::Function,
        class_name: None,
        decorated: false,
        exported_as_default: false,
        class_default_export: false,
        imports: Vec::new(),
        method_kind: TypeScriptMethodKind::Ordinary,
        arity: None,
        parameters: Vec::new(),
        source_text: None,
    };
    let test = TypeScriptTest {
        name: "applies discount".to_string(),
        local_name: "applies discount".to_string(),
        describe_names: Vec::new(),
        file: PathBuf::from("tests/discount.test.ts"),
        line: 1,
        body_text: "const result = applyDiscount(100, 10);\nexpect(result).toBe(88);".to_string(),
        assertions: vec![TypeScriptAssertion {
            matcher: "toBe".to_string(),
            argument_count: 1,
            line: 2,
            oracle_kind: OracleKind::ExactValue,
            oracle_strength: OracleStrength::Strong,
            mock_payload: None,
            error_payload: None,
            // Bare local — no owner name, no changed token in the expression
            // itself; only the initializer aliases the owner call.
            observed_expression: Some("result".to_string()),
            expected_value_or_variant: Some("88".to_string()),
            has_dynamic_matcher_arg: false,
            oracle_confidence: OracleConfidence::High,
        }],
        mocks_in_file: Vec::new(),
        imports_in_file: Vec::new(),
    };
    let finding = classify_change(
        Path::new("src/discount.ts"),
        3,
        "  return amount - 12;",
        &[owner],
        &[test],
        None,
        &ReExportIndex::empty(),
        None,
    )
    .ok_or_else(|| "expected a finding".to_string())?;

    // MUST stay exposed — the aliased local observes the changed sink.
    assert!(
        matches!(finding.class, ExposureClass::Exposed),
        "expected Exposed (owner-aliased local observation on ReturnValue), got {:?}",
        finding.class
    );
    assert!(
        matches!(finding.ripr.reveal.discriminate.state, StageState::Yes),
        "expected discriminate==Yes, got {:?}",
        finding.ripr.reveal.discriminate.state
    );
    Ok(())
}

/// Aliasing negative control: a bare-local observed_expression whose
/// initializer is an UNRELATED call (`const other = formatDate(now)`) does
/// not observe the changed sink, even when the test body also calls the
/// owner (`applyDiscount(100, 10);` establishes reach). The one-hop credit
/// must NOT fire, the guard fails closed, and the finding downgrades to
/// `weakly_exposed`.
#[test]
fn ts_returnvalue_unrelated_aliased_local_observation_downgrades() -> Result<(), String> {
    let owner = TypeScriptOwner {
        name: "applyDiscount".to_string(),
        file: PathBuf::from("src/discount.ts"),
        start_line: 1,
        end_line: 8,
        owner_kind: OwnerKind::Function,
        class_name: None,
        decorated: false,
        exported_as_default: false,
        class_default_export: false,
        imports: Vec::new(),
        method_kind: TypeScriptMethodKind::Ordinary,
        arity: None,
        parameters: Vec::new(),
        source_text: None,
    };
    let test = TypeScriptTest {
        name: "discount side checks".to_string(),
        local_name: "discount side checks".to_string(),
        describe_names: Vec::new(),
        file: PathBuf::from("tests/discount.test.ts"),
        line: 1,
        body_text: "applyDiscount(100, 10);\nconst other = formatDate(now);\nexpect(other).toBe('2024-01-01');"
            .to_string(),
        assertions: vec![TypeScriptAssertion {
            matcher: "toBe".to_string(),
            argument_count: 1,
            line: 2,
            oracle_kind: OracleKind::ExactValue,
            oracle_strength: OracleStrength::Strong,
            mock_payload: None,
            error_payload: None,
            // Bare local, but the initializer names neither the owner nor a
            // changed token — the aliasing credit must not fire.
            observed_expression: Some("other".to_string()),
            expected_value_or_variant: Some("'2024-01-01'".to_string()),
            has_dynamic_matcher_arg: false,
            oracle_confidence: OracleConfidence::High,
        }],
        mocks_in_file: Vec::new(),
        imports_in_file: Vec::new(),
    };
    let finding = classify_change(
        Path::new("src/discount.ts"),
        3,
        "  return amount - 12;",
        &[owner],
        &[test],
        None,
        &ReExportIndex::empty(),
        None,
    )
    .ok_or_else(|| "expected a finding".to_string())?;

    // MUST downgrade — the aliased local does not observe the changed sink.
    assert!(
        matches!(finding.class, ExposureClass::WeaklyExposed),
        "expected WeaklyExposed (unrelated aliased local on ReturnValue), got {:?}",
        finding.class
    );
    assert!(
        !matches!(finding.ripr.reveal.discriminate.state, StageState::Yes),
        "discriminate must not be Yes after value-sink observation guard, got {:?}",
        finding.ripr.reveal.discriminate.state
    );
    Ok(())
}

/// Conservative no-downgrade (RIPR-SPEC-0098 §fixture-3): a SideEffect where
/// the test body has TWO strong assertions — one asserting the owner return
/// value (`observed_expression = "trackAction(...)"`) AND one asserting a
/// closure-local side-effect variable (`observed_expression = "sideEffectLog"`).
///
/// Because the second assertion does NOT contain the owner name, the guard
/// treats it as a potential effect observer and returns `confirmed = true`.
/// The finding MUST stay `class:exposed` — we do not downgrade when any
/// assertion is asserting something other than the owner return value.
#[test]
fn ts_sibling_assertion_non_owner_prevents_downgrade() -> Result<(), String> {
    let owner = TypeScriptOwner {
        name: "applyDiscount".to_string(),
        file: PathBuf::from("src/discount.ts"),
        start_line: 1,
        end_line: 10,
        owner_kind: OwnerKind::Function,
        class_name: None,
        decorated: false,
        exported_as_default: false,
        class_default_export: false,
        arity: None,
        parameters: Vec::new(),
        source_text: None,
        imports: Vec::new(),
        method_kind: TypeScriptMethodKind::Ordinary,
    };
    let test = TypeScriptTest {
        name: "side-effect is visible and return value correct".to_string(),
        local_name: "side-effect is visible and return value correct".to_string(),
        describe_names: Vec::new(),
        file: PathBuf::from("tests/discount.test.ts"),
        line: 1,
        body_text: "const discount = applyDiscount(100, 10);\nconsole.log(discount);\nexpect(applyDiscount(100, 10)).toBe(90);\nexpect(sideEffectLog).toBe(true);".to_string(),
        assertions: vec![
            TypeScriptAssertion {
                matcher: "toBe".to_string(),
                argument_count: 1,
                line: 3,
                oracle_kind: OracleKind::ExactValue,
                oracle_strength: OracleStrength::Strong,
                mock_payload: None,
                error_payload: None,
                // This assertion contains the owner name → owner-return-value pattern.
                observed_expression: Some("applyDiscount(100, 10)".to_string()),
                expected_value_or_variant: Some("90".to_string()),
                has_dynamic_matcher_arg: false,
                oracle_confidence: OracleConfidence::High,
            },
            TypeScriptAssertion {
                matcher: "toBe".to_string(),
                argument_count: 1,
                line: 4,
                oracle_kind: OracleKind::ExactValue,
                oracle_strength: OracleStrength::Strong,
                mock_payload: None,
                error_payload: None,
                // This assertion does NOT contain the owner name → could be effect observer.
                observed_expression: Some("sideEffectLog".to_string()),
                expected_value_or_variant: Some("true".to_string()),
                has_dynamic_matcher_arg: false,
                oracle_confidence: OracleConfidence::High,
            },
        ],
        mocks_in_file: Vec::new(),
        imports_in_file: Vec::new(),
    };
    // Changed line: SideEffect (console.log call)
    let finding = classify_change(
        Path::new("src/discount.ts"),
        4,
        "    console.log(\"audit\", amount * 9);",
        &[owner],
        &[test],
        None,
        &ReExportIndex::empty(),
        None,
    )
    .ok_or_else(|| "expected a finding".to_string())?;

    // Because `sideEffectLog` does NOT contain the owner name, the guard
    // treats it as a potential side-effect observer → observation confirmed.
    // The finding MUST stay Exposed (conservative: no downgrade when any
    // assertion might be observing the side effect via a side channel).
    assert!(
        matches!(finding.class, ExposureClass::Exposed),
        "expected Exposed (non-owner assertion prevents downgrade), got {:?}",
        finding.class
    );
    Ok(())
}

/// FieldConstruction control (RIPR-SPEC-0098 §fixture-4): a FieldConstruction
/// change where the changed FIELD NAME appears in a strong `toEqual`
/// assertion's `observed_expression` — the token match confirms.  The finding
/// MUST stay `class:exposed`.
#[test]
fn ts_field_construction_observed_control() -> Result<(), String> {
    let owner = TypeScriptOwner {
        name: "buildConfig".to_string(),
        file: PathBuf::from("src/config.ts"),
        start_line: 1,
        end_line: 10,
        owner_kind: OwnerKind::Function,
        class_name: None,
        decorated: false,
        exported_as_default: false,
        class_default_export: false,
        arity: None,
        parameters: Vec::new(),
        source_text: None,
        imports: Vec::new(),
        method_kind: TypeScriptMethodKind::Ordinary,
    };
    let test = TypeScriptTest {
        name: "builds config with timeout".to_string(),
        local_name: "builds config with timeout".to_string(),
        describe_names: Vec::new(),
        file: PathBuf::from("tests/config.test.ts"),
        line: 1,
        body_text: "expect(buildConfig()).toEqual({ timeout: 5000 });".to_string(),
        assertions: vec![TypeScriptAssertion {
            matcher: "toEqual".to_string(),
            argument_count: 1,
            line: 2,
            oracle_kind: OracleKind::WholeObjectEquality,
            oracle_strength: OracleStrength::Strong,
            mock_payload: None,
            error_payload: None,
            // Contains the owner name — confirms via owner-call observation for value families.
            observed_expression: Some("buildConfig()".to_string()),
            expected_value_or_variant: Some("{ timeout: 5000 }".to_string()),
            has_dynamic_matcher_arg: false,
            oracle_confidence: OracleConfidence::High,
        }],
        mocks_in_file: Vec::new(),
        imports_in_file: Vec::new(),
    };
    // Changed line: a field value assignment (FieldConstruction)
    let finding = classify_change(
        Path::new("src/config.ts"),
        3,
        "  timeout: 5000,",
        &[owner],
        &[test],
        None,
        &ReExportIndex::empty(),
        None,
    )
    .ok_or_else(|| "expected a finding".to_string())?;

    // Owner-call observation on a value family → guard confirms → Exposed.
    assert!(
        matches!(finding.class, ExposureClass::Exposed),
        "expected Exposed (owner-call observation on FieldConstruction), got {:?}",
        finding.class
    );
    assert!(
        matches!(finding.ripr.reveal.discriminate.state, StageState::Yes),
        "expected discriminate==Yes, got {:?}",
        finding.ripr.reveal.discriminate.state
    );
    Ok(())
}

/// FieldConstruction downgrade mirror (Droid Auto Review confirmed finding
/// on #4095): the value-family guard branch is shared between ReturnValue
/// and FieldConstruction, so this control pins the sibling seam directly
/// instead of inferring it from the ReturnValue control. A FieldConstruction
/// change (`timeout: 5000,`) reached by a test whose only strong assertion
/// observes an UNRELATED expression MUST downgrade to `weakly_exposed` with
/// a `propagation_unknown` limitation — symmetric to
/// `ts_returnvalue_unrelated_strong_assertion_downgrades`.
#[test]
fn ts_fieldconstruction_unrelated_strong_assertion_downgrades() -> Result<(), String> {
    let owner = TypeScriptOwner {
        name: "buildConfig".to_string(),
        file: PathBuf::from("src/config.ts"),
        start_line: 1,
        end_line: 10,
        owner_kind: OwnerKind::Function,
        class_name: None,
        decorated: false,
        exported_as_default: false,
        class_default_export: false,
        imports: Vec::new(),
        method_kind: TypeScriptMethodKind::Ordinary,
        arity: None,
        parameters: Vec::new(),
        source_text: None,
    };
    let test = TypeScriptTest {
        name: "config side checks".to_string(),
        local_name: "config side checks".to_string(),
        describe_names: Vec::new(),
        file: PathBuf::from("tests/config.test.ts"),
        line: 1,
        body_text: "buildConfig();\nexpect(formatDate(now)).toBe('2024-01-01');".to_string(),
        assertions: vec![TypeScriptAssertion {
            matcher: "toBe".to_string(),
            argument_count: 1,
            line: 2,
            oracle_kind: OracleKind::ExactValue,
            oracle_strength: OracleStrength::Strong,
            mock_payload: None,
            error_payload: None,
            // Unrelated expression: no owner name, no changed token (`timeout`).
            observed_expression: Some("formatDate(now)".to_string()),
            expected_value_or_variant: Some("'2024-01-01'".to_string()),
            has_dynamic_matcher_arg: false,
            oracle_confidence: OracleConfidence::High,
        }],
        mocks_in_file: Vec::new(),
        imports_in_file: Vec::new(),
    };
    // Changed line: a field value assignment (FieldConstruction).
    let finding = classify_change(
        Path::new("src/config.ts"),
        3,
        "  timeout: 5000,",
        &[owner],
        &[test],
        None,
        &ReExportIndex::empty(),
        None,
    )
    .ok_or_else(|| "expected a finding".to_string())?;

    // MUST downgrade: the strong assertion observes an unrelated expression.
    assert!(
        matches!(finding.class, ExposureClass::WeaklyExposed),
        "expected WeaklyExposed (unrelated strong assertion on FieldConstruction), got {:?}",
        finding.class
    );
    assert!(
        !matches!(finding.ripr.reveal.discriminate.state, StageState::Yes),
        "discriminate must not be Yes after value-sink observation guard, got {:?}",
        finding.ripr.reveal.discriminate.state
    );
    let all_text: String = finding.missing.join("\n");
    assert!(
        all_text.contains("propagation_unknown"),
        "expected propagation_unknown in missing, got: {all_text:?}"
    );
    Ok(())
}

/// LIVE-pipeline guard test (RIPR-SPEC-0098 §fixture-5, #1235): exercises the
/// REAL oracle extractor end-to-end. Source and test text are parsed by
/// `extract_owners` / `extract_tests`, so the assertions carry whatever
/// `observed_expression` the live extractor produces (NOT a hand-set field).
///
/// This is the regression guard the #1235 review demanded: it fails if the
/// observation guard ever regresses to a fail-OPEN that depends on
/// `observed_expression` being populated in a particular way. A swallowed
/// `console.log` side effect, observed only by value-shaped `toBe` assertions on
/// the owner return value, MUST downgrade to WeaklyExposed.
#[test]
fn ts_swallowed_console_log_downgrade_live_extractor() -> Result<(), String> {
    let owner_src = "export function applyDiscount(amount, threshold) {\n  console.log(\"audit\", amount * 9);\n  if (amount >= threshold) return amount - 12;\n  return amount;\n}\n";
    let test_src = "import { applyDiscount } from \"./owner\";\ntest(\"discount big\", () => { expect(applyDiscount(100, 100)).toBe(88); });\ntest(\"discount small\", () => { expect(applyDiscount(50, 100)).toBe(50); });\n";

    let owners = extract_owners(Path::new("owner.ts"), owner_src);
    let tests = extract_tests(Path::new("owner.test.ts"), test_src);
    assert_eq!(owners.len(), 1, "expected one owner from live extractor");
    assert!(
        !tests.is_empty(),
        "expected at least one test from live extractor"
    );
    // Sanity: the live extractor produces a strong assertion — proving the guard
    // keys on real extracted data, not a hand-set field.
    let any_strong = tests
        .iter()
        .flat_map(|t| t.assertions.iter())
        .any(|a| a.oracle_strength.rank() >= OracleStrength::Strong.rank());
    assert!(
        any_strong,
        "expected a strong assertion from live extractor"
    );

    // Changed line is the console.log side effect (line 2 of owner_src).
    let finding = classify_change(
        Path::new("owner.ts"),
        2,
        "  console.log(\"audit\", amount * 9);",
        &owners,
        &tests,
        None,
        &ReExportIndex::empty(),
        None,
    )
    .ok_or_else(|| "expected a finding".to_string())?;

    assert!(
        matches!(finding.class, ExposureClass::WeaklyExposed),
        "live extractor: expected WeaklyExposed after observation guard, got {:?}",
        finding.class
    );
    assert!(
        !matches!(finding.ripr.reveal.discriminate.state, StageState::Yes),
        "live extractor: discriminate must not be Yes, got {:?}",
        finding.ripr.reveal.discriminate.state
    );
    let all_text: String = finding.missing.join("\n");
    assert!(
        all_text.contains("propagation_unknown"),
        "live extractor: expected propagation_unknown in missing, got: {all_text:?}"
    );
    Ok(())
}

/// MockExpectation stays exposed (RIPR-SPEC-0098 §fixture-6, #1235): a
/// SideEffect change observed by a `toHaveBeenCalledWith` mock expectation IS a
/// genuine effect observer. The guard MUST confirm via `oracle_kind` alone
/// (MockExpectation), keeping the finding Exposed even when
/// `observed_expression` is None. This protects the effect-observer path from
/// the fail-closed downgrade.
#[test]
fn ts_side_effect_observed_by_mock_expectation_stays_exposed() -> Result<(), String> {
    let owner = TypeScriptOwner {
        name: "applyDiscount".to_string(),
        file: PathBuf::from("src/discount.ts"),
        start_line: 1,
        end_line: 10,
        owner_kind: OwnerKind::Function,
        class_name: None,
        decorated: false,
        exported_as_default: false,
        class_default_export: false,
        arity: None,
        parameters: Vec::new(),
        source_text: None,
        imports: Vec::new(),
        method_kind: TypeScriptMethodKind::Ordinary,
    };
    // A strong MockExpectation assertion with observed_expression None — exactly
    // the live shape for `expect(spy).toHaveBeenCalledWith(...)` where the
    // decision must rest on oracle_kind, NOT observed_expression.
    let test = TypeScriptTest {
        name: "logs audit with discounted amount".to_string(),
        local_name: "logs audit with discounted amount".to_string(),
        describe_names: Vec::new(),
        file: PathBuf::from("tests/discount.test.ts"),
        line: 1,
        body_text: "applyDiscount(100, 10);\nexpect(logSpy).toHaveBeenCalledWith(\"audit\", 900);"
            .to_string(),
        assertions: vec![TypeScriptAssertion {
            matcher: "toHaveBeenCalledWith".to_string(),
            argument_count: 2,
            line: 2,
            oracle_kind: OracleKind::MockExpectation,
            oracle_strength: OracleStrength::Strong,
            mock_payload: None,
            error_payload: None,
            observed_expression: None,
            expected_value_or_variant: None,
            has_dynamic_matcher_arg: false,
            oracle_confidence: OracleConfidence::High,
        }],
        mocks_in_file: Vec::new(),
        imports_in_file: Vec::new(),
    };
    let finding = classify_change(
        Path::new("src/discount.ts"),
        4,
        "    console.log(\"audit\", amount * 9);",
        &[owner],
        &[test],
        None,
        &ReExportIndex::empty(),
        None,
    )
    .ok_or_else(|| "expected a finding".to_string())?;

    assert!(
        matches!(finding.class, ExposureClass::Exposed),
        "expected Exposed (MockExpectation observes the side effect), got {:?}",
        finding.class
    );
    assert!(
        matches!(finding.ripr.reveal.discriminate.state, StageState::Yes),
        "expected discriminate==Yes for MockExpectation, got {:?}",
        finding.ripr.reveal.discriminate.state
    );
    Ok(())
}

/// Should-stay-`weakly_exposed` control (RIPR-SPEC-0098 false-confirmation
/// family): a SideEffect seam whose discriminator is the call-effect sentence
/// `call tracker.record includes event` must NOT be confirmed by a strong
/// assertion that merely happens to call `.includes(...)` on the owner return
/// value. Before this control, the raw substring check
/// `observed.contains("includes")` promoted the swallowed side effect to
/// `Exposed`. Template vocabulary (`includes` / `occurs` / …) is generator
/// wording, not changed code, and dot-adjacent segments (`tracker`, `record`)
/// name a different receiver's members; none of them may confirm.
#[test]
fn ts_side_effect_includes_template_word_does_not_confirm() -> Result<(), String> {
    let owner = TypeScriptOwner {
        name: "trackLogin".to_string(),
        file: PathBuf::from("src/tracker.ts"),
        start_line: 1,
        end_line: 10,
        owner_kind: OwnerKind::Function,
        class_name: None,
        decorated: false,
        exported_as_default: false,
        class_default_export: false,
        imports: Vec::new(),
        method_kind: TypeScriptMethodKind::Ordinary,
        arity: None,
        parameters: Vec::new(),
        source_text: None,
    };
    // The observed expression names the owner (so the side-channel arm does
    // not fire) but only "confirms" via the substring `includes` — which is
    // synthesized discriminator vocabulary, not a changed token.
    let test = TypeScriptTest {
        name: "tracks login and checks the label".to_string(),
        local_name: "tracks login and checks the label".to_string(),
        describe_names: Vec::new(),
        file: PathBuf::from("tests/tracker.test.ts"),
        line: 1,
        body_text: "expect(trackLogin(\"login\").includes(payload)).toBe(true);".to_string(),
        assertions: vec![TypeScriptAssertion {
            matcher: "toBe".to_string(),
            argument_count: 1,
            line: 2,
            oracle_kind: OracleKind::ExactValue,
            oracle_strength: OracleStrength::Strong,
            mock_payload: None,
            error_payload: None,
            observed_expression: Some("trackLogin(\"login\").includes(payload)".to_string()),
            expected_value_or_variant: Some("true".to_string()),
            has_dynamic_matcher_arg: false,
            oracle_confidence: OracleConfidence::High,
        }],
        mocks_in_file: Vec::new(),
        imports_in_file: Vec::new(),
    };
    // Changed line: SideEffect member call — the effect never escapes.
    let finding = classify_change(
        Path::new("src/tracker.ts"),
        4,
        "    tracker.record(event);",
        &[owner],
        &[test],
        None,
        &ReExportIndex::empty(),
        None,
    )
    .ok_or_else(|| "expected a finding".to_string())?;

    // The `.includes(...)` assertion does NOT observe the call effect: it
    // must fail closed to weakly_exposed with a propagation_unknown limitation.
    assert!(
        matches!(finding.class, ExposureClass::WeaklyExposed),
        "expected WeaklyExposed (template word must not confirm), got {:?}",
        finding.class
    );
    assert!(
        !matches!(finding.ripr.reveal.discriminate.state, StageState::Yes),
        "discriminate must not be Yes when only a template word matched, got {:?}",
        finding.ripr.reveal.discriminate.state
    );
    let all_text: String = finding.missing.join("\n");
    assert!(
        all_text.contains("propagation_unknown"),
        "expected propagation_unknown in missing, got: {all_text:?}"
    );
    Ok(())
}

// ── RIPR-SPEC-0099: tsconfig.json path-alias resolution ───────────────────────

/// Build a strong `toBe` assertion for RIPR-SPEC-0099 alias tests.
fn strong_be_assertion() -> TypeScriptAssertion {
    TypeScriptAssertion {
        matcher: "toBe".to_string(),
        argument_count: 1,
        line: 3,
        oracle_kind: OracleKind::ExactValue,
        oracle_strength: OracleStrength::Strong,
        mock_payload: None,
        error_payload: None,
        observed_expression: Some("applyDiscount(100, 10)".to_string()),
        expected_value_or_variant: Some("90".to_string()),
        has_dynamic_matcher_arg: false,
        oracle_confidence: OracleConfidence::High,
    }
}

/// RIPR-SPEC-0099 test 1 — POSITIVE (flag ON):
/// tsconfig baseUrl="." paths={"@/*":["src/*"]}, owner=src/owner.ts,
/// test imports `applyDiscount` from `@/owner` with strong assertion
/// → alias resolves to unique file → test linked → `exposed`.
#[test]
fn tsconfig_alias_resolution_flag_on_credits_test_as_exposed() -> Result<(), String> {
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let root = std::env::temp_dir().join(format!("ripr-tscfg-pos-{stamp}"));
    fs::create_dir_all(root.join("src")).map_err(|e| e.to_string())?;
    fs::write(
        root.join("tsconfig.json"),
        r#"{"compilerOptions":{"baseUrl":".","paths":{"@/*":["src/*"]}}}"#,
    )
    .map_err(|e| e.to_string())?;
    fs::write(
        root.join("src").join("owner.ts"),
        "export function applyDiscount(a: number, b: number): number { return a - b; }",
    )
    .map_err(|e| e.to_string())?;

    let alias_map = load_alias_map(&root);
    let alias_map =
        alias_map.ok_or_else(|| "tsconfig.json should parse with a baseUrl".to_string())?;

    let owner = TypeScriptOwner {
        name: "applyDiscount".to_string(),
        file: PathBuf::from("src/owner.ts"),
        start_line: 1,
        end_line: 1,
        owner_kind: OwnerKind::Function,
        class_name: None,
        decorated: false,
        exported_as_default: false,
        class_default_export: false,
        arity: None,
        parameters: Vec::new(),
        source_text: None,
        imports: Vec::new(),
        method_kind: TypeScriptMethodKind::Ordinary,
    };
    let test = TypeScriptTest {
        name: "applyDiscount returns correct value".to_string(),
        local_name: "applyDiscount returns correct value".to_string(),
        describe_names: Vec::new(),
        // Use a relative path so import_source_matches_owner's relative walk works.
        file: PathBuf::from("src/owner.test.ts"),
        line: 1,
        body_text: "const result = applyDiscount(100, 10);\nexpect(result).toBe(90);".to_string(),
        assertions: vec![strong_be_assertion()],
        mocks_in_file: Vec::new(),
        imports_in_file: vec![TypeScriptImport {
            source: "@/owner".to_string(),
            imported: Some("applyDiscount".to_string()),
            local: "applyDiscount".to_string(),
            namespace: false,
        }],
    };
    let all_owners = [owner];
    let all_tests = [test];

    // Flag ON and tsconfig present: alias @/owner resolves to src/owner.ts → exposed.
    // Pass relative paths matching owner.file so the owner-lookup in classify_change
    // finds the owner (normalized_path must equal owner.file's normalized form).
    let finding = classify_change(
        Path::new("src/owner.ts"),
        1,
        "return a - b;",
        &all_owners,
        &all_tests,
        Some(&root),
        &ReExportIndex::empty(),
        Some(&alias_map),
    )
    .ok_or_else(|| "expected a finding".to_string())?;

    assert_eq!(
        finding.class,
        ExposureClass::Exposed,
        "flag ON + unique alias resolution: expected Exposed, got {:?}",
        finding.class
    );
    assert_eq!(
        finding.related_tests.len(),
        1,
        "flag ON: expected 1 related test, got {:?}",
        finding.related_tests
    );
    // No disclosure limitation should appear — the import was credited.
    assert_evidence_lacks(&finding, "typescript_path_alias_unresolved");
    Ok(())
}

/// RIPR-SPEC-0099 test 2 — DEFAULT-OFF CONTROL:
/// Identical setup, alias_map=None (flag OFF) → stays `no_static_path`,
/// BUT the `typescript_path_alias_unresolved` disclosure limitation IS emitted.
#[test]
fn tsconfig_alias_resolution_flag_off_stays_no_static_path_with_disclosure() -> Result<(), String> {
    let owner = TypeScriptOwner {
        name: "applyDiscount".to_string(),
        file: PathBuf::from("src/owner.ts"),
        start_line: 1,
        end_line: 1,
        owner_kind: OwnerKind::Function,
        class_name: None,
        decorated: false,
        exported_as_default: false,
        class_default_export: false,
        arity: None,
        parameters: Vec::new(),
        source_text: None,
        imports: Vec::new(),
        method_kind: TypeScriptMethodKind::Ordinary,
    };
    let test = TypeScriptTest {
        name: "applyDiscount returns correct value".to_string(),
        local_name: "applyDiscount returns correct value".to_string(),
        describe_names: Vec::new(),
        file: PathBuf::from("src/owner.test.ts"),
        line: 1,
        body_text: "const result = applyDiscount(100, 10);\nexpect(result).toBe(90);".to_string(),
        assertions: vec![strong_be_assertion()],
        mocks_in_file: Vec::new(),
        imports_in_file: vec![TypeScriptImport {
            source: "@/owner".to_string(),
            imported: Some("applyDiscount".to_string()),
            local: "applyDiscount".to_string(),
            namespace: false,
        }],
    };
    let all_owners = [owner];
    let all_tests = [test];

    // Flag OFF (no alias map) → alias cannot resolve → no_static_path.
    let finding = classify_change(
        Path::new("src/owner.ts"),
        1,
        "return a - b;",
        &all_owners,
        &all_tests,
        None,
        &ReExportIndex::empty(),
        None, // flag OFF
    )
    .ok_or_else(|| "expected a finding".to_string())?;

    assert_eq!(
        finding.class,
        ExposureClass::NoStaticPath,
        "flag OFF: expected NoStaticPath (alias not resolved), got {:?}",
        finding.class
    );
    assert!(
        finding.related_tests.is_empty(),
        "flag OFF: no test should be credited, got {:?}",
        finding.related_tests
    );
    // The always-on disclosure limitation MUST be present.
    assert_evidence_contains(
        &finding,
        "typescript_limitation: typescript_path_alias_unresolved",
    );
    Ok(())
}

/// RIPR-SPEC-0099 test 3 — AMBIGUOUS FAIL-CLOSED (flag ON):
/// paths value `["src/*","lib/*"]` (multi-entry) → excluded from alias map
/// → uncredited, stays `no_static_path`, disclosure limitation IS emitted.
#[test]
fn tsconfig_alias_resolution_multi_entry_value_fails_closed() -> Result<(), String> {
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let root = std::env::temp_dir().join(format!("ripr-tscfg-amb-{stamp}"));
    fs::create_dir_all(root.join("src")).map_err(|e| e.to_string())?;
    // multi-entry value array → fail-closed, entry excluded from alias map
    fs::write(
        root.join("tsconfig.json"),
        r#"{"compilerOptions":{"baseUrl":".","paths":{"@/*":["src/*","lib/*"]}}}"#,
    )
    .map_err(|e| e.to_string())?;
    fs::write(
        root.join("src").join("owner.ts"),
        "export function applyDiscount() {}",
    )
    .map_err(|e| e.to_string())?;

    // The map parses (baseUrl is present) but the multi-entry key is excluded.
    let alias_map =
        load_alias_map(&root).ok_or_else(|| "tsconfig.json should parse".to_string())?;

    let owner = TypeScriptOwner {
        name: "applyDiscount".to_string(),
        file: PathBuf::from("src/owner.ts"),
        start_line: 1,
        end_line: 1,
        owner_kind: OwnerKind::Function,
        class_name: None,
        decorated: false,
        exported_as_default: false,
        class_default_export: false,
        arity: None,
        parameters: Vec::new(),
        source_text: None,
        imports: Vec::new(),
        method_kind: TypeScriptMethodKind::Ordinary,
    };
    let test = TypeScriptTest {
        name: "applyDiscount test".to_string(),
        local_name: "applyDiscount test".to_string(),
        describe_names: Vec::new(),
        file: PathBuf::from("src/owner.test.ts"),
        line: 1,
        body_text: "const result = applyDiscount();\nexpect(result).toBe(90);".to_string(),
        assertions: vec![strong_be_assertion()],
        mocks_in_file: Vec::new(),
        imports_in_file: vec![TypeScriptImport {
            source: "@/owner".to_string(),
            imported: Some("applyDiscount".to_string()),
            local: "applyDiscount".to_string(),
            namespace: false,
        }],
    };
    let all_owners = [owner];
    let all_tests = [test];

    // Flag ON but multi-entry value → alias map has no entry for @/* → fail-closed.
    let finding = classify_change(
        Path::new("src/owner.ts"),
        1,
        "export function applyDiscount() {}",
        &all_owners,
        &all_tests,
        None,
        &ReExportIndex::empty(),
        Some(&alias_map),
    )
    .ok_or_else(|| "expected a finding".to_string())?;

    assert_eq!(
        finding.class,
        ExposureClass::NoStaticPath,
        "ambiguous (multi-entry): expected NoStaticPath (fail-closed), got {:?}",
        finding.class
    );
    // Disclosure limitation must be emitted even when flag ON but alias ambiguous.
    assert_evidence_contains(
        &finding,
        "typescript_limitation: typescript_path_alias_unresolved",
    );
    Ok(())
}

/// RIPR-SPEC-0099 test 4 — NON-MATCH NEGATIVE:
/// Test imports `cloneDeep` from `lodash` (non-relative, NOT owner name)
/// → NO `typescript_path_alias_unresolved` limitation emitted.
#[test]
fn tsconfig_alias_non_owner_import_emits_no_limitation() -> Result<(), String> {
    let owner = TypeScriptOwner {
        name: "applyDiscount".to_string(),
        file: PathBuf::from("src/owner.ts"),
        start_line: 1,
        end_line: 5,
        owner_kind: OwnerKind::Function,
        class_name: None,
        decorated: false,
        exported_as_default: false,
        class_default_export: false,
        arity: None,
        parameters: Vec::new(),
        source_text: None,
        imports: Vec::new(),
        method_kind: TypeScriptMethodKind::Ordinary,
    };
    // Test only imports `cloneDeep` from lodash — unrelated to the owner name.
    let test = TypeScriptTest {
        name: "some unrelated test".to_string(),
        local_name: "some unrelated test".to_string(),
        describe_names: Vec::new(),
        file: PathBuf::from("src/other.test.ts"),
        line: 1,
        body_text: "const _ = cloneDeep({});\nexpect(_.x).toBe(1);".to_string(),
        assertions: vec![strong_be_assertion()],
        mocks_in_file: Vec::new(),
        imports_in_file: vec![TypeScriptImport {
            source: "lodash".to_string(),
            imported: Some("cloneDeep".to_string()),
            local: "cloneDeep".to_string(),
            namespace: false,
        }],
    };
    let all_owners = [owner];
    let all_tests = [test];

    let finding = classify_change(
        Path::new("src/owner.ts"),
        1,
        "return a - b;",
        &all_owners,
        &all_tests,
        None,
        &ReExportIndex::empty(),
        None,
    )
    .ok_or_else(|| "expected a finding".to_string())?;

    // `cloneDeep` != `applyDiscount` → no alias-gap limitation must fire.
    assert_evidence_lacks(&finding, "typescript_path_alias_unresolved");
    Ok(())
}

/// RIPR-SPEC-0099 test 5 — DEFAULT-IMPORT NEGATIVE CONTROL:
/// `import React from 'react'` is recorded as `imported: "default"`, which is
/// NOT the owner's exported name. The limitation must NOT fire: a default
/// import only plausibly targets the owner when the LOCAL binding name matches
/// (`React` != `applyDiscount`). Before the local-binding check, any default
/// import from any non-relative package false-fired this limitation.
#[test]
fn tsconfig_alias_default_import_local_name_mismatch_emits_no_limitation() -> Result<(), String> {
    let owner = TypeScriptOwner {
        name: "applyDiscount".to_string(),
        file: PathBuf::from("src/owner.ts"),
        start_line: 1,
        end_line: 5,
        owner_kind: OwnerKind::Function,
        class_name: None,
        decorated: false,
        exported_as_default: false,
        class_default_export: false,
        imports: Vec::new(),
        method_kind: TypeScriptMethodKind::Ordinary,
        arity: None,
        parameters: Vec::new(),
        source_text: None,
    };
    // Test only imports the React default binding — unrelated to the owner.
    let test = TypeScriptTest {
        name: "renders the app".to_string(),
        local_name: "renders the app".to_string(),
        describe_names: Vec::new(),
        file: PathBuf::from("src/app.test.tsx"),
        line: 1,
        body_text: "const view = renderApp();\nexpect(view).toBeTruthy();".to_string(),
        assertions: vec![strong_be_assertion()],
        mocks_in_file: Vec::new(),
        imports_in_file: vec![TypeScriptImport {
            source: "react".to_string(),
            imported: Some("default".to_string()),
            local: "React".to_string(),
            namespace: false,
        }],
    };
    let all_owners = [owner];
    let all_tests = [test];

    let finding = classify_change(
        Path::new("src/owner.ts"),
        1,
        "return a - b;",
        &all_owners,
        &all_tests,
        None,
        &ReExportIndex::empty(),
        None,
    )
    .ok_or_else(|| "expected a finding".to_string())?;

    // `React` (local) != `applyDiscount` (owner) → no alias-gap limitation.
    assert_evidence_lacks(&finding, "typescript_path_alias_unresolved");
    Ok(())
}

/// RIPR-SPEC-0099 test 6 — DEFAULT-IMPORT POSITIVE CONTROL:
/// `import applyDiscount from '@/applyDiscount'` records `imported: "default"`
/// but the LOCAL binding name matches the owner — the import plausibly targets
/// the owner, so the limitation MUST still fire when resolution fails.
#[test]
fn tsconfig_alias_default_import_local_name_match_emits_limitation() -> Result<(), String> {
    let owner = TypeScriptOwner {
        name: "applyDiscount".to_string(),
        file: PathBuf::from("src/owner.ts"),
        start_line: 1,
        end_line: 5,
        owner_kind: OwnerKind::Function,
        class_name: None,
        decorated: false,
        exported_as_default: false,
        class_default_export: false,
        imports: Vec::new(),
        method_kind: TypeScriptMethodKind::Ordinary,
        arity: None,
        parameters: Vec::new(),
        source_text: None,
    };
    let test = TypeScriptTest {
        name: "default import test".to_string(),
        local_name: "default import test".to_string(),
        describe_names: Vec::new(),
        file: PathBuf::from("src/owner.test.ts"),
        line: 1,
        body_text: "const result = applyDiscount(100);\nexpect(result).toBe(90);".to_string(),
        assertions: vec![strong_be_assertion()],
        mocks_in_file: Vec::new(),
        imports_in_file: vec![TypeScriptImport {
            source: "@/owner".to_string(),
            imported: Some("default".to_string()),
            local: "applyDiscount".to_string(),
            namespace: false,
        }],
    };
    let all_owners = [owner];
    let all_tests = [test];

    let finding = classify_change(
        Path::new("src/owner.ts"),
        1,
        "return a - b;",
        &all_owners,
        &all_tests,
        None,
        &ReExportIndex::empty(),
        None,
    )
    .ok_or_else(|| "expected a finding".to_string())?;

    // Local binding == owner name → alias-gap limitation must fire.
    assert_evidence_contains(
        &finding,
        "typescript_limitation: typescript_path_alias_unresolved",
    );
    Ok(())
}

/// #4106-B remainder: the alias-unresolved advice must name the actual
/// fail-closed cause. Flag OFF → the cause is the missing alias map.
#[test]
fn tsconfig_alias_advice_names_map_unavailable_cause() -> Result<(), String> {
    let owner = TypeScriptOwner {
        name: "applyDiscount".to_string(),
        file: PathBuf::from("src/owner.ts"),
        start_line: 1,
        end_line: 5,
        owner_kind: OwnerKind::Function,
        class_name: None,
        decorated: false,
        exported_as_default: false,
        class_default_export: false,
        imports: Vec::new(),
        method_kind: TypeScriptMethodKind::Ordinary,
        arity: None,
        parameters: Vec::new(),
        source_text: None,
    };
    let test = TypeScriptTest {
        name: "applyDiscount test".to_string(),
        local_name: "applyDiscount test".to_string(),
        describe_names: Vec::new(),
        file: PathBuf::from("src/owner.test.ts"),
        line: 1,
        body_text: "const result = applyDiscount();\nexpect(result).toBe(90);".to_string(),
        assertions: vec![strong_be_assertion()],
        mocks_in_file: Vec::new(),
        imports_in_file: vec![TypeScriptImport {
            source: "@/owner".to_string(),
            imported: Some("applyDiscount".to_string()),
            local: "applyDiscount".to_string(),
            namespace: false,
        }],
    };
    let all_owners = [owner];
    let all_tests = [test];

    let finding = classify_change(
        Path::new("src/owner.ts"),
        1,
        "return a - b;",
        &all_owners,
        &all_tests,
        None,
        &ReExportIndex::empty(),
        None, // flag OFF → no alias map
    )
    .ok_or_else(|| "expected a finding".to_string())?;

    // #4106-B: a None map with no load gap means the FLAG IS OFF, so the
    // advice names the flag instead of a generic "no map" message.
    assert_evidence_contains(&finding, "alias resolution is not enabled");
    assert_evidence_contains(&finding, "set `[typescript] resolve_tsconfig_paths = true`");
    Ok(())
}

/// #4106-B remainder: flag ON with a parsed map whose `paths` keys do NOT
/// match the specifier → the advice names the unmatched-pattern cause.
#[test]
fn tsconfig_alias_advice_names_unmatched_pattern_cause() -> Result<(), String> {
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let root = std::env::temp_dir().join(format!("ripr-tscfg-unmatched-{stamp}"));
    fs::create_dir_all(root.join("src")).map_err(|e| e.to_string())?;
    // The map only owns `@/lib`; the test imports `@/owner`.
    fs::write(
        root.join("tsconfig.json"),
        r#"{"compilerOptions":{"baseUrl":".","paths":{"@/lib":["src/lib"]}}}"#,
    )
    .map_err(|e| e.to_string())?;
    let alias_map =
        load_alias_map(&root).ok_or_else(|| "tsconfig.json should parse".to_string())?;

    let owner = TypeScriptOwner {
        name: "applyDiscount".to_string(),
        file: PathBuf::from("src/owner.ts"),
        start_line: 1,
        end_line: 5,
        owner_kind: OwnerKind::Function,
        class_name: None,
        decorated: false,
        exported_as_default: false,
        class_default_export: false,
        imports: Vec::new(),
        method_kind: TypeScriptMethodKind::Ordinary,
        arity: None,
        parameters: Vec::new(),
        source_text: None,
    };
    let test = TypeScriptTest {
        name: "applyDiscount test".to_string(),
        local_name: "applyDiscount test".to_string(),
        describe_names: Vec::new(),
        file: PathBuf::from("src/owner.test.ts"),
        line: 1,
        body_text: "const result = applyDiscount();\nexpect(result).toBe(90);".to_string(),
        assertions: vec![strong_be_assertion()],
        mocks_in_file: Vec::new(),
        imports_in_file: vec![TypeScriptImport {
            source: "@/owner".to_string(),
            imported: Some("applyDiscount".to_string()),
            local: "applyDiscount".to_string(),
            namespace: false,
        }],
    };
    let all_owners = [owner];
    let all_tests = [test];

    let finding = classify_change(
        Path::new("src/owner.ts"),
        1,
        "return a - b;",
        &all_owners,
        &all_tests,
        None,
        &ReExportIndex::empty(),
        Some(&alias_map),
    )
    .ok_or_else(|| "expected a finding".to_string())?;

    assert_evidence_contains(
        &finding,
        "no compilerOptions.paths key (literal or single-`*`) matches this specifier",
    );
    let _ = fs::remove_dir_all(&root);
    Ok(())
}

/// #4106-B remainder: flag ON, pattern matches, but the candidate file does
/// not exist → the advice names the unresolved-candidate cause.
#[test]
fn tsconfig_alias_advice_names_unresolved_candidate_cause() -> Result<(), String> {
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let root = std::env::temp_dir().join(format!("ripr-tscfg-nocand-{stamp}"));
    fs::create_dir_all(root.join("src")).map_err(|e| e.to_string())?;
    // `@/*` owns the specifier, but src/owner.ts does not exist.
    fs::write(
        root.join("tsconfig.json"),
        r#"{"compilerOptions":{"baseUrl":".","paths":{"@/*":["src/*"]}}}"#,
    )
    .map_err(|e| e.to_string())?;
    let alias_map =
        load_alias_map(&root).ok_or_else(|| "tsconfig.json should parse".to_string())?;

    let owner = TypeScriptOwner {
        name: "applyDiscount".to_string(),
        file: PathBuf::from("src/owner.ts"),
        start_line: 1,
        end_line: 5,
        owner_kind: OwnerKind::Function,
        class_name: None,
        decorated: false,
        exported_as_default: false,
        class_default_export: false,
        imports: Vec::new(),
        method_kind: TypeScriptMethodKind::Ordinary,
        arity: None,
        parameters: Vec::new(),
        source_text: None,
    };
    let test = TypeScriptTest {
        name: "applyDiscount test".to_string(),
        local_name: "applyDiscount test".to_string(),
        describe_names: Vec::new(),
        file: PathBuf::from("src/owner.test.ts"),
        line: 1,
        body_text: "const result = applyDiscount();\nexpect(result).toBe(90);".to_string(),
        assertions: vec![strong_be_assertion()],
        mocks_in_file: Vec::new(),
        imports_in_file: vec![TypeScriptImport {
            source: "@/owner".to_string(),
            imported: Some("applyDiscount".to_string()),
            local: "applyDiscount".to_string(),
            namespace: false,
        }],
    };
    let all_owners = [owner];
    let all_tests = [test];

    let finding = classify_change(
        Path::new("src/owner.ts"),
        1,
        "return a - b;",
        &all_owners,
        &all_tests,
        None,
        &ReExportIndex::empty(),
        Some(&alias_map),
    )
    .ok_or_else(|| "expected a finding".to_string())?;

    assert_evidence_contains(
        &finding,
        "the matched pattern's candidate did not resolve to exactly one in-root workspace file",
    );
    let _ = fs::remove_dir_all(&root);
    Ok(())
}

/// #4106-B remainder: flag ON with an absolute baseUrl → the advice names
/// the absolute-baseUrl cause (typed limitation
/// `typescript_base_url_absolute_unsupported` owns the map side).
#[test]
fn tsconfig_alias_advice_names_absolute_base_url_cause() -> Result<(), String> {
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let root = std::env::temp_dir().join(format!("ripr-tscfg-absadv-{stamp}"));
    fs::create_dir_all(root.join("src")).map_err(|e| e.to_string())?;
    fs::write(
        root.join("tsconfig.json"),
        r#"{"compilerOptions":{"baseUrl":"/abs/base","paths":{"@/*":["src/*"]}}}"#,
    )
    .map_err(|e| e.to_string())?;
    fs::write(
        root.join("src").join("owner.ts"),
        "export function applyDiscount() {}",
    )
    .map_err(|e| e.to_string())?;
    let alias_map =
        load_alias_map(&root).ok_or_else(|| "tsconfig.json should parse".to_string())?;
    assert!(
        alias_map.base_url_absolute(),
        "absolute baseUrl must be flagged on the map"
    );

    let owner = TypeScriptOwner {
        name: "applyDiscount".to_string(),
        file: PathBuf::from("src/owner.ts"),
        start_line: 1,
        end_line: 1,
        owner_kind: OwnerKind::Function,
        class_name: None,
        decorated: false,
        exported_as_default: false,
        class_default_export: false,
        imports: Vec::new(),
        method_kind: TypeScriptMethodKind::Ordinary,
        arity: None,
        parameters: Vec::new(),
        source_text: None,
    };
    let test = TypeScriptTest {
        name: "applyDiscount test".to_string(),
        local_name: "applyDiscount test".to_string(),
        describe_names: Vec::new(),
        file: PathBuf::from("src/owner.test.ts"),
        line: 1,
        body_text: "const result = applyDiscount();\nexpect(result).toBe(90);".to_string(),
        assertions: vec![strong_be_assertion()],
        mocks_in_file: Vec::new(),
        imports_in_file: vec![TypeScriptImport {
            source: "@/owner".to_string(),
            imported: Some("applyDiscount".to_string()),
            local: "applyDiscount".to_string(),
            namespace: false,
        }],
    };
    let all_owners = [owner];
    let all_tests = [test];

    let finding = classify_change(
        Path::new("src/owner.ts"),
        1,
        "export function applyDiscount() {}",
        &all_owners,
        &all_tests,
        None,
        &ReExportIndex::empty(),
        Some(&alias_map),
    )
    .ok_or_else(|| "expected a finding".to_string())?;

    assert_evidence_contains(
        &finding,
        "compilerOptions.baseUrl is absolute or non-normal",
    );
    let _ = fs::remove_dir_all(&root);
    Ok(())
}

// ── RIPR-SPEC-0104: family↔oracle-kind matching (4 controls) ─────────────────

/// RIPR-SPEC-0104 control 1 (REPRO — headline fix):
/// ReturnValue seam; two SEPARATE tests: one with `.toThrow(DiscountError)`
/// (ExactErrorVariant, Strong) on the error path, one with `.toBeGreaterThan(0)`
/// (RelationalCheck, Weak) on the return value.
///
/// The diff changes `0.95 → 0.90` on the gold return value line.
/// Before the fix: the ExactErrorVariant (Strong) was promoted as
/// `strongest_kind`, yielding `Exposed / strong_oracle_observed`.
/// After the fix: only assertions matching the `ReturnValue` family are
/// considered; the best match is `RelationalCheck/Weak` → `weakly_exposed`
/// with an actionable repair card.
#[test]
fn spec_0104_repro_cross_family_error_oracle_does_not_promote_return_value_seam()
-> Result<(), String> {
    let owner = TypeScriptOwner {
        name: "applyDiscount".to_string(),
        file: PathBuf::from("src/discount.ts"),
        start_line: 1,
        end_line: 20,
        owner_kind: OwnerKind::Function,
        class_name: None,
        decorated: false,
        exported_as_default: false,
        class_default_export: false,
        arity: None,
        parameters: Vec::new(),
        source_text: None,
        imports: Vec::new(),
        method_kind: TypeScriptMethodKind::Ordinary,
    };
    // Test A: error-path observer — toThrow(DiscountError) — Strong, ExactErrorVariant.
    // This test does NOT match the ReturnValue seam family.
    let error_test = TypeScriptTest {
        name: "applyDiscount throws on invalid input".to_string(),
        local_name: "applyDiscount throws on invalid input".to_string(),
        describe_names: Vec::new(),
        file: PathBuf::from("tests/discount.test.ts"),
        line: 10,
        body_text: "expect(() => applyDiscount(-1, 'gold')).toThrow(DiscountError);".to_string(),
        assertions: vec![TypeScriptAssertion {
            matcher: "toThrow".to_string(),
            argument_count: 1,
            line: 11,
            oracle_kind: OracleKind::ExactErrorVariant,
            oracle_strength: OracleStrength::Strong,
            mock_payload: None,
            error_payload: None,
            observed_expression: None,
            expected_value_or_variant: None,
            has_dynamic_matcher_arg: false,
            oracle_confidence: OracleConfidence::Medium,
        }],
        mocks_in_file: Vec::new(),
        imports_in_file: Vec::new(),
    };
    // Test B: return-value observer — toBeGreaterThan(0) — Weak, RelationalCheck.
    // This test DOES match the ReturnValue seam family, but only weakly.
    let return_test = TypeScriptTest {
        name: "applyDiscount returns positive amount for gold".to_string(),
        local_name: "applyDiscount returns positive amount for gold".to_string(),
        describe_names: Vec::new(),
        file: PathBuf::from("tests/discount.test.ts"),
        line: 15,
        body_text: "expect(applyDiscount(100, 'gold')).toBeGreaterThan(0);".to_string(),
        assertions: vec![TypeScriptAssertion {
            matcher: "toBeGreaterThan".to_string(),
            argument_count: 1,
            line: 16,
            oracle_kind: OracleKind::RelationalCheck,
            oracle_strength: OracleStrength::Weak,
            mock_payload: None,
            error_payload: None,
            observed_expression: Some("applyDiscount(100, 'gold')".to_string()),
            expected_value_or_variant: None,
            has_dynamic_matcher_arg: false,
            oracle_confidence: OracleConfidence::Low,
        }],
        mocks_in_file: Vec::new(),
        imports_in_file: Vec::new(),
    };
    // Changed line: the gold-tier return value (ReturnValue seam).
    let finding = classify_change(
        Path::new("src/discount.ts"),
        5,
        "  return amount * 0.90;",
        &[owner],
        &[error_test, return_test],
        None,
        &ReExportIndex::empty(),
        None,
    )
    .ok_or_else(|| "expected a finding".to_string())?;

    // MUST be weakly_exposed — the only ReturnValue-matching oracle is Weak.
    assert!(
        matches!(finding.class, ExposureClass::WeaklyExposed),
        "RIPR-SPEC-0104 control 1: expected WeaklyExposed for ReturnValue seam with \
         only cross-family Strong oracle; got {:?}",
        finding.class
    );
    // The strongest oracle reported back MUST NOT be ExactErrorVariant.
    // (The summary text contains the oracle kind.)
    assert!(
        finding
            .evidence
            .iter()
            .all(|line| !line.contains("exact_error_variant")),
        "RIPR-SPEC-0104 control 1: evidence must not name exact_error_variant; \
         got {:?}",
        finding.evidence
    );
    Ok(())
}

/// RIPR-SPEC-0104 control 2 (MUST-NOT-OVER-CORRECT — return + exact value):
/// ReturnValue seam with a `toBe(90)` assertion (ExactValue, Strong).
/// The family-matching filter allows ExactValue for ReturnValue seams.
/// MUST stay `Exposed` (ExactValue matches ReturnValue).
#[test]
fn spec_0104_no_over_correct_return_value_with_exact_value_stays_exposed() -> Result<(), String> {
    let owner = TypeScriptOwner {
        name: "applyDiscount".to_string(),
        file: PathBuf::from("src/discount.ts"),
        start_line: 1,
        end_line: 10,
        owner_kind: OwnerKind::Function,
        class_name: None,
        decorated: false,
        exported_as_default: false,
        class_default_export: false,
        arity: None,
        parameters: Vec::new(),
        source_text: None,
        imports: Vec::new(),
        method_kind: TypeScriptMethodKind::Ordinary,
    };
    let exact_value_test = TypeScriptTest {
        name: "applyDiscount gold rate".to_string(),
        local_name: "applyDiscount gold rate".to_string(),
        describe_names: Vec::new(),
        file: PathBuf::from("tests/discount.test.ts"),
        line: 1,
        body_text: "expect(applyDiscount(100, 'gold')).toBe(90);".to_string(),
        assertions: vec![TypeScriptAssertion {
            matcher: "toBe".to_string(),
            argument_count: 1,
            line: 2,
            oracle_kind: OracleKind::ExactValue,
            oracle_strength: OracleStrength::Strong,
            mock_payload: None,
            error_payload: None,
            observed_expression: Some("applyDiscount(100, 'gold')".to_string()),
            expected_value_or_variant: Some("90".to_string()),
            has_dynamic_matcher_arg: false,
            oracle_confidence: OracleConfidence::High,
        }],
        mocks_in_file: Vec::new(),
        imports_in_file: Vec::new(),
    };
    let finding = classify_change(
        Path::new("src/discount.ts"),
        3,
        "  return amount * 0.90;",
        &[owner],
        &[exact_value_test],
        None,
        &ReExportIndex::empty(),
        None,
    )
    .ok_or_else(|| "expected a finding".to_string())?;

    // ExactValue matches ReturnValue → MUST stay Exposed.
    assert!(
        matches!(finding.class, ExposureClass::Exposed),
        "RIPR-SPEC-0104 control 2: expected Exposed for ReturnValue seam with \
         ExactValue Strong oracle; got {:?}",
        finding.class
    );
    Ok(())
}

/// RIPR-SPEC-0104 control 3 (MUST-NOT-OVER-CORRECT — error + toThrow exact):
/// ErrorPath seam with `expect(() => fn(-1)).toThrow('Invalid amount')`.
/// The `toThrow` with a string literal yields ExactErrorVariant (Strong).
/// ExactErrorVariant matches ErrorPath → MUST stay `Exposed`.
#[test]
fn spec_0104_no_over_correct_error_path_with_exact_error_variant_stays_exposed()
-> Result<(), String> {
    let owner = TypeScriptOwner {
        name: "applyDiscount".to_string(),
        file: PathBuf::from("src/discount.ts"),
        start_line: 1,
        end_line: 10,
        owner_kind: OwnerKind::Function,
        class_name: None,
        decorated: false,
        exported_as_default: false,
        class_default_export: false,
        arity: None,
        parameters: Vec::new(),
        source_text: None,
        imports: Vec::new(),
        method_kind: TypeScriptMethodKind::Ordinary,
    };
    let throw_test = TypeScriptTest {
        name: "applyDiscount throws on negative amount".to_string(),
        local_name: "applyDiscount throws on negative amount".to_string(),
        describe_names: Vec::new(),
        file: PathBuf::from("tests/discount.test.ts"),
        line: 1,
        body_text: "expect(() => applyDiscount(-1, 'gold')).toThrow('Invalid amount');".to_string(),
        assertions: vec![TypeScriptAssertion {
            matcher: "toThrow".to_string(),
            argument_count: 1,
            line: 2,
            oracle_kind: OracleKind::ExactErrorVariant,
            oracle_strength: OracleStrength::Strong,
            mock_payload: None,
            error_payload: None,
            observed_expression: None,
            expected_value_or_variant: Some("'Invalid amount'".to_string()),
            has_dynamic_matcher_arg: false,
            oracle_confidence: OracleConfidence::High,
        }],
        mocks_in_file: Vec::new(),
        imports_in_file: Vec::new(),
    };
    let finding = classify_change(
        Path::new("src/discount.ts"),
        3,
        "  throw new Error('Invalid amount');",
        &[owner],
        &[throw_test],
        None,
        &ReExportIndex::empty(),
        None,
    )
    .ok_or_else(|| "expected a finding".to_string())?;

    // ExactErrorVariant matches ErrorPath → MUST stay Exposed.
    assert!(
        matches!(finding.class, ExposureClass::Exposed),
        "RIPR-SPEC-0104 control 3: expected Exposed for ErrorPath seam with \
         ExactErrorVariant Strong oracle; got {:?}",
        finding.class
    );
    Ok(())
}

/// RIPR-SPEC-0104 control 4 (SINGLE-TEST-BOTH-ASSERTIONS — over-correction guard):
/// ONE test that asserts BOTH `.toThrow(DiscountError)` (ExactErrorVariant,
/// Strong, wrong-family for ReturnValue) AND `.toBe(90)` (ExactValue, Strong,
/// matches ReturnValue) in a single test body.
/// The diff changes the gold return value.
///
/// This test proves the fix operates at the ASSERTION level, not the TEST level.
/// Filtering at the test level would DROP the whole test because its
/// overall-strongest assertion (ExactErrorVariant) is wrong-family, losing the
/// legitimate `.toBe(90)` assertion → false weakly_exposed.
/// With assertion-level filtering, the `.toBe(90)` assertion is kept and the
/// seam MUST stay `Exposed`.
#[test]
fn spec_0104_single_test_both_assertions_retains_matching_family_assertion_stays_exposed()
-> Result<(), String> {
    let owner = TypeScriptOwner {
        name: "applyDiscount".to_string(),
        file: PathBuf::from("src/discount.ts"),
        start_line: 1,
        end_line: 20,
        owner_kind: OwnerKind::Function,
        class_name: None,
        decorated: false,
        exported_as_default: false,
        class_default_export: false,
        arity: None,
        parameters: Vec::new(),
        source_text: None,
        imports: Vec::new(),
        method_kind: TypeScriptMethodKind::Ordinary,
    };
    // ONE test with TWO assertions:
    //   1. `.toThrow(DiscountError)` — ExactErrorVariant, Strong — wrong-family for ReturnValue.
    //   2. `.toBe(90)` — ExactValue, Strong — matches ReturnValue.
    // A test-level filter would drop the whole test due to assertion 1.
    // An assertion-level filter retains assertion 2 → Exposed.
    let dual_assertion_test = TypeScriptTest {
        name: "applyDiscount gold path and error path".to_string(),
        local_name: "applyDiscount gold path and error path".to_string(),
        describe_names: Vec::new(),
        file: PathBuf::from("tests/discount.test.ts"),
        line: 1,
        body_text: concat!(
            "expect(() => applyDiscount(-1, 'gold')).toThrow(DiscountError);\n",
            "expect(applyDiscount(100, 'gold')).toBe(90);"
        )
        .to_string(),
        assertions: vec![
            // Assertion 1: error-path observer (wrong-family for the changed ReturnValue seam).
            TypeScriptAssertion {
                matcher: "toThrow".to_string(),
                argument_count: 1,
                line: 2,
                oracle_kind: OracleKind::ExactErrorVariant,
                oracle_strength: OracleStrength::Strong,
                mock_payload: None,
                error_payload: None,
                observed_expression: None,
                expected_value_or_variant: None,
                has_dynamic_matcher_arg: false,
                oracle_confidence: OracleConfidence::Medium,
            },
            // Assertion 2: return-value observer (family-matching for ReturnValue seam).
            TypeScriptAssertion {
                matcher: "toBe".to_string(),
                argument_count: 1,
                line: 3,
                oracle_kind: OracleKind::ExactValue,
                oracle_strength: OracleStrength::Strong,
                mock_payload: None,
                error_payload: None,
                observed_expression: Some("applyDiscount(100, 'gold')".to_string()),
                expected_value_or_variant: Some("90".to_string()),
                has_dynamic_matcher_arg: false,
                oracle_confidence: OracleConfidence::High,
            },
        ],
        mocks_in_file: Vec::new(),
        imports_in_file: Vec::new(),
    };
    // Changed line: the gold-tier return value (ReturnValue seam).
    let finding = classify_change(
        Path::new("src/discount.ts"),
        5,
        "  return amount * 0.90;",
        &[owner],
        &[dual_assertion_test],
        None,
        &ReExportIndex::empty(),
        None,
    )
    .ok_or_else(|| "expected a finding".to_string())?;

    // The toBe(90) assertion is family-matching (ExactValue for ReturnValue).
    // The test MUST NOT be dropped due to its toThrow assertion.
    // MUST stay Exposed (assertion-level filter keeps the matching assertion).
    assert!(
        matches!(finding.class, ExposureClass::Exposed),
        "RIPR-SPEC-0104 control 4: expected Exposed (assertion-level filter retains \
         matching ExactValue assertion); got {:?}. \
         If WeaklyExposed, the fix incorrectly filtered at the test level.",
        finding.class
    );
    Ok(())
}

/// Unit test for `ts_oracle_kind_matches_seam`: validates the mapping table.
///
/// Key invariants (RIPR-SPEC-0104 §3):
/// - `ExactErrorVariant` (Strong, error-path discriminator) does NOT match
///   value-family seams (`ReturnValue`, `Predicate`, `FieldConstruction`, `MatchArm`).
///   This is THE PRIMARY FIX — ExactErrorVariant must not promote a ReturnValue seam.
/// - `ExactValue` (Strong, value discriminator) does NOT match `ErrorPath` seams.
/// - `BroadError` is excluded from value-family seams (it is error-domain).
/// - `MockExpectation` is excluded from value-family and error-path seams.
/// - For `SideEffect` / `CallDeletion`: all oracle kinds are admitted (RIPR-SPEC-0098
///   observation guard handles those seams independently).
/// - `StaticUnknown` probe family is fail-open: all oracle kinds admitted.
#[test]
fn spec_0104_ts_oracle_kind_matches_seam_mapping_table() {
    // ErrorPath family: excludes value/mock oracles.
    assert!(ts_oracle_kind_matches_seam(
        &OracleKind::ExactErrorVariant,
        &ProbeFamily::ErrorPath
    ));
    assert!(ts_oracle_kind_matches_seam(
        &OracleKind::BroadError,
        &ProbeFamily::ErrorPath
    ));
    assert!(ts_oracle_kind_matches_seam(
        &OracleKind::Snapshot,
        &ProbeFamily::ErrorPath
    ));
    assert!(ts_oracle_kind_matches_seam(
        &OracleKind::SmokeOnly,
        &ProbeFamily::ErrorPath
    ));
    assert!(ts_oracle_kind_matches_seam(
        &OracleKind::Unknown,
        &ProbeFamily::ErrorPath
    ));
    // Excluded from ErrorPath: strong value and mock oracles.
    assert!(!ts_oracle_kind_matches_seam(
        &OracleKind::ExactValue,
        &ProbeFamily::ErrorPath
    ));
    assert!(!ts_oracle_kind_matches_seam(
        &OracleKind::MockExpectation,
        &ProbeFamily::ErrorPath
    ));
    assert!(!ts_oracle_kind_matches_seam(
        &OracleKind::RelationalCheck,
        &ProbeFamily::ErrorPath
    ));

    // Value families: admit value oracles; reject ExactErrorVariant, BroadError, MockExpectation.
    for value_family in [
        ProbeFamily::ReturnValue,
        ProbeFamily::Predicate,
        ProbeFamily::FieldConstruction,
        ProbeFamily::MatchArm,
    ] {
        assert!(
            ts_oracle_kind_matches_seam(&OracleKind::ExactValue, &value_family),
            "{value_family:?} must accept ExactValue"
        );
        assert!(
            ts_oracle_kind_matches_seam(&OracleKind::WholeObjectEquality, &value_family),
            "{value_family:?} must accept WholeObjectEquality"
        );
        assert!(
            ts_oracle_kind_matches_seam(&OracleKind::Snapshot, &value_family),
            "{value_family:?} must accept Snapshot"
        );
        assert!(
            ts_oracle_kind_matches_seam(&OracleKind::RelationalCheck, &value_family),
            "{value_family:?} must accept RelationalCheck"
        );
        assert!(
            ts_oracle_kind_matches_seam(&OracleKind::SmokeOnly, &value_family),
            "{value_family:?} must admit SmokeOnly (weak oracle, rank below Strong)"
        );
        assert!(
            ts_oracle_kind_matches_seam(&OracleKind::Unknown, &value_family),
            "{value_family:?} must admit Unknown (absent oracle)"
        );
        // THE KEY FIX: ExactErrorVariant must NOT match value-family seams.
        assert!(
            !ts_oracle_kind_matches_seam(&OracleKind::ExactErrorVariant, &value_family),
            "{value_family:?} must reject ExactErrorVariant (error-path discriminator \
             must not promote a value seam to Exposed)"
        );
        assert!(
            !ts_oracle_kind_matches_seam(&OracleKind::BroadError, &value_family),
            "{value_family:?} must reject BroadError (error-domain oracle)"
        );
        assert!(
            !ts_oracle_kind_matches_seam(&OracleKind::MockExpectation, &value_family),
            "{value_family:?} must reject MockExpectation (effect observer)"
        );
    }

    // SideEffect / CallDeletion: all oracle kinds admitted (RIPR-SPEC-0098 handles these).
    for effect_family in [ProbeFamily::SideEffect, ProbeFamily::CallDeletion] {
        for kind in [
            OracleKind::ExactValue,
            OracleKind::ExactErrorVariant,
            OracleKind::MockExpectation,
            OracleKind::RelationalCheck,
            OracleKind::BroadError,
            OracleKind::Snapshot,
            OracleKind::SmokeOnly,
            OracleKind::Unknown,
        ] {
            assert!(
                ts_oracle_kind_matches_seam(&kind, &effect_family),
                "{effect_family:?} must admit {kind:?} (RIPR-SPEC-0098 handles downgrade)"
            );
        }
    }

    // StaticUnknown is fail-open: accepts all oracle kinds.
    for kind in [
        OracleKind::ExactValue,
        OracleKind::ExactErrorVariant,
        OracleKind::MockExpectation,
        OracleKind::RelationalCheck,
        OracleKind::BroadError,
        OracleKind::Snapshot,
        OracleKind::SmokeOnly,
        OracleKind::Unknown,
    ] {
        assert!(
            ts_oracle_kind_matches_seam(&kind, &ProbeFamily::StaticUnknown),
            "StaticUnknown must accept {kind:?} (fail-open)"
        );
    }
}

// ── Helpers for cockpit-delta-5 / issue-#1245 tests ──────────────────────────

fn ts_unique_tempdir(label: &str) -> Result<PathBuf, String> {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|err| format!("system time: {err}"))?
        .as_nanos();
    let dir = std::env::temp_dir().join(format!(
        "ripr-ts-delta5-{label}-{}-{nanos}",
        std::process::id()
    ));
    std::fs::create_dir_all(&dir)
        .map_err(|err| format!("create_dir_all({}): {err}", dir.display()))?;
    Ok(dir)
}

fn ts_write_file(path: &Path, contents: &str) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|err| format!("create_dir_all({}): {err}", parent.display()))?;
    }
    std::fs::write(path, contents).map_err(|err| format!("write({}): {err}", path.display()))
}

/// Control 1 (cockpit delta #5, issue #1245):
/// When the test runner IS resolved (Vitest detected from package.json) and a
/// related test exists, the emitted evidence MUST carry
/// `typescript_verify_command` AND must NOT list `verify_command` in
/// `missing_actionability_fields`.  Fail-open before this fix; fail-closed now.
#[test]
fn delta5_verify_command_absent_from_missing_list_when_runner_resolved() -> Result<(), String> {
    let root = ts_unique_tempdir("derived")?;

    // package.json with vitest in devDependencies — enough for framework detection.
    ts_write_file(
        &root.join("package.json"),
        r#"{"name":"pkg","scripts":{"test":"vitest"},"devDependencies":{"vitest":"^1.0.0"}}"#,
    )?;
    // Lockfile — its presence lets runner detection via lockfile fire; the
    // framework hint already suffices for verify_command but having a lockfile
    // also exercises the medium-confidence path.
    ts_write_file(&root.join("package-lock.json"), "{}")?;

    // Production file: a simple function in a named owner block.
    ts_write_file(
        &root.join("src/lib.ts"),
        "export function applyDiscount(amount: number, threshold: number): number {\n  if (amount >= threshold) {\n    return amount - 10;\n  }\n  return amount;\n}\n",
    )?;

    // Test file: calls applyDiscount directly → DirectOwnerCall relation.
    ts_write_file(
        &root.join("tests/lib.test.ts"),
        "import { applyDiscount } from '../src/lib';\ntest('applies discount when above threshold', () => {\n  const result = applyDiscount(50, 100);\n  expect(result).toBeTruthy();\n});\n",
    )?;

    let adapter = TypeScriptAdapter;
    let options = AnalysisOptions {
        root: root.clone(),
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
    let changed_files = vec![ChangedFile {
        path: PathBuf::from("src/lib.ts"),
        added_lines: vec![crate::analysis::diff::ChangedLine {
            line: 2,
            new_side_line: 2,
            text: "  if (amount >= threshold) {".to_string(),
        }],
        removed_lines: Vec::new(),
    }];

    let result = adapter.analyze_diff(&options, &policy, &changed_files);
    let _ = std::fs::remove_dir_all(&root);
    let result = result?;

    if result.findings.is_empty() {
        return Err(format!(
            "expected at least one finding; got none (changed_files={})",
            result.changed_files
        ));
    }
    let finding = &result.findings[0];

    // verify_command evidence MUST be present (runner resolved).
    let has_verify_cmd = finding
        .evidence
        .iter()
        .any(|ev| ev.starts_with("typescript_verify_command:"));
    if !has_verify_cmd {
        return Err(format!(
            "expected typescript_verify_command evidence when runner resolves; evidence={:?}",
            finding.evidence
        ));
    }

    // missing_actionability_fields MUST NOT list verify_command.
    let bad_line = finding.evidence.iter().find(|ev| {
        ev.starts_with("missing_actionability_fields:") && ev.contains("verify_command")
    });
    if let Some(line) = bad_line {
        return Err(format!(
            "missing_actionability_fields still lists verify_command even though typescript_verify_command is present (self-contradiction): {line:?}"
        ));
    }

    // receipt_command and canonical_gap_id MUST still be in missing list
    // (they are genuinely unprojected — fail-closed).
    let missing_line = finding
        .evidence
        .iter()
        .find(|ev| ev.starts_with("missing_actionability_fields:"));
    if let Some(line) = missing_line {
        // receipt_command is in all blocked categories that include verify_command.
        if !line.contains("receipt_command") {
            return Err(format!(
                "receipt_command should still be in missing_actionability_fields: {line:?}"
            ));
        }
    }

    Ok(())
}

/// Control 2 (cockpit delta #5, issue #1245 — fail-closed):
/// When the test runner is NOT resolved (no package.json, no framework), the
/// emitted evidence MUST NOT carry `typescript_verify_command` and MUST still
/// list `verify_command` in `missing_actionability_fields`.
#[test]
fn delta5_verify_command_stays_in_missing_list_when_runner_unresolved() -> Result<(), String> {
    let root = ts_unique_tempdir("unresolved")?;

    // No package.json / no framework — runner cannot be inferred.
    ts_write_file(
        &root.join("src/lib.ts"),
        "export function applyDiscount(amount: number, threshold: number): number {\n  if (amount >= threshold) {\n    return amount - 10;\n  }\n  return amount;\n}\n",
    )?;
    ts_write_file(
        &root.join("tests/lib.test.ts"),
        "import { applyDiscount } from '../src/lib';\ntest('stub', () => {\n  applyDiscount(50, 100);\n});\n",
    )?;

    let adapter = TypeScriptAdapter;
    let options = AnalysisOptions {
        root: root.clone(),
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
    let changed_files = vec![ChangedFile {
        path: PathBuf::from("src/lib.ts"),
        added_lines: vec![crate::analysis::diff::ChangedLine {
            line: 2,
            new_side_line: 2,
            text: "  if (amount >= threshold) {".to_string(),
        }],
        removed_lines: Vec::new(),
    }];

    let result = adapter.analyze_diff(&options, &policy, &changed_files);
    let _ = std::fs::remove_dir_all(&root);
    let result = result?;

    if result.findings.is_empty() {
        return Err(format!(
            "expected at least one finding; got none (changed_files={})",
            result.changed_files
        ));
    }
    let finding = &result.findings[0];

    // typescript_verify_command MUST NOT be present (fail-closed).
    let invented_cmd = finding
        .evidence
        .iter()
        .find(|ev| ev.starts_with("typescript_verify_command:"));
    if let Some(cmd) = invented_cmd {
        return Err(format!(
            "runner unresolved: no command should be invented, but got: {cmd:?}"
        ));
    }

    // missing_actionability_fields MUST still list verify_command.
    let missing_line = finding
        .evidence
        .iter()
        .find(|ev| ev.starts_with("missing_actionability_fields:"));
    match missing_line {
        None => {
            // No missing_actionability_fields line at all: acceptable only if
            // the finding is strong_oracle_observed (empty missing list). In
            // that case verify_command absence is trivially correct.
        }
        Some(line) if line.contains("verify_command") => {
            // Good — verify_command is still listed as missing.
        }
        Some(line) => {
            return Err(format!(
                "runner unresolved: verify_command should be in missing_actionability_fields, but got: {line:?}"
            ));
        }
    }

    Ok(())
}

/// Mocha fail-closed control: a detected mocha framework has NO file-target
/// command mapping, so with no lockfile/runner evidence the inferred command
/// is `None`. The evidence MUST carry
/// `typescript_package_limitation: typescript_test_runner_unresolved` (the
/// limitation fires whenever the command is unresolved — even though a
/// framework WAS detected) and MUST NOT invent a `typescript_verify_command`.
#[test]
fn mocha_no_lockfile_emits_runner_unresolved_limitation() -> Result<(), String> {
    let root = ts_unique_tempdir("mocha-no-lockfile")?;

    // package.json with mocha in devDependencies; NO lockfile → no runner
    // evidence and no file-target mocha command.
    ts_write_file(
        &root.join("package.json"),
        r#"{"name":"pkg","scripts":{"test":"mocha"},"devDependencies":{"mocha":"^10.0.0"}}"#,
    )?;

    ts_write_file(
        &root.join("src/lib.ts"),
        "export function applyDiscount(amount: number, threshold: number): number {\n  if (amount >= threshold) {\n    return amount - 10;\n  }\n  return amount;\n}\n",
    )?;
    ts_write_file(
        &root.join("tests/lib.test.ts"),
        "import { applyDiscount } from '../src/lib';\ntest('applies discount', () => {\n  const result = applyDiscount(50, 100);\n  expect(result).toBeTruthy();\n});\n",
    )?;

    let adapter = TypeScriptAdapter;
    let options = AnalysisOptions {
        root: root.clone(),
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
    let changed_files = vec![ChangedFile {
        path: PathBuf::from("src/lib.ts"),
        added_lines: vec![crate::analysis::diff::ChangedLine {
            line: 2,
            new_side_line: 2,
            text: "  if (amount >= threshold) {".to_string(),
        }],
        removed_lines: Vec::new(),
    }];

    let result = adapter.analyze_diff(&options, &policy, &changed_files);
    let _ = std::fs::remove_dir_all(&root);
    let result = result?;

    if result.findings.is_empty() {
        return Err(format!(
            "expected at least one finding; got none (changed_files={})",
            result.changed_files
        ));
    }
    let finding = &result.findings[0];

    // Framework WAS detected — the runner name line must be present.
    let has_runner_name = finding
        .evidence
        .iter()
        .any(|ev| ev == "typescript_test_runner: mocha");
    if !has_runner_name {
        return Err(format!(
            "expected typescript_test_runner: mocha evidence; evidence={:?}",
            finding.evidence
        ));
    }

    // No command must be invented.
    let invented_cmd = finding
        .evidence
        .iter()
        .find(|ev| ev.starts_with("typescript_verify_command:"));
    if let Some(cmd) = invented_cmd {
        return Err(format!(
            "mocha without lockfile: no command should be invented, but got: {cmd:?}"
        ));
    }

    // The unresolved limitation MUST fire even though a framework was detected.
    let has_limitation = finding
        .evidence
        .iter()
        .any(|ev| ev == "typescript_package_limitation: typescript_test_runner_unresolved");
    if !has_limitation {
        return Err(format!(
            "expected typescript_test_runner_unresolved limitation when the inferred command is None; evidence={:?}",
            finding.evidence
        ));
    }

    Ok(())
}

/// Control 3 (cockpit delta #5, issue #1245 — unchanged-case guard):
/// `remove_field_from_missing_list` unit tests: correct removal, idempotency,
/// and leave-alone for non-matching input.
#[test]
fn remove_field_from_missing_list_unit() {
    // Removes the named field from the middle of the list.
    assert_eq!(
        remove_field_from_missing_list(
            "missing_actionability_fields: canonical_gap_id, verify_command, receipt_command",
            "verify_command"
        ),
        "missing_actionability_fields: canonical_gap_id, receipt_command"
    );

    // Removes from the start of the list.
    assert_eq!(
        remove_field_from_missing_list(
            "missing_actionability_fields: verify_command, receipt_command",
            "verify_command"
        ),
        "missing_actionability_fields: receipt_command"
    );

    // Removes from the end of the list.
    assert_eq!(
        remove_field_from_missing_list(
            "missing_actionability_fields: canonical_gap_id, verify_command",
            "verify_command"
        ),
        "missing_actionability_fields: canonical_gap_id"
    );

    // Field absent — line returned unchanged.
    assert_eq!(
        remove_field_from_missing_list(
            "missing_actionability_fields: canonical_gap_id, receipt_command",
            "verify_command"
        ),
        "missing_actionability_fields: canonical_gap_id, receipt_command"
    );

    // Wrong prefix — line returned unchanged.
    assert_eq!(
        remove_field_from_missing_list("gap_state: advisory", "verify_command"),
        "gap_state: advisory"
    );

    // Removing another field leaves verify_command intact.
    assert_eq!(
        remove_field_from_missing_list(
            "missing_actionability_fields: canonical_gap_id, verify_command, receipt_command",
            "canonical_gap_id"
        ),
        "missing_actionability_fields: verify_command, receipt_command"
    );
}

fn parse_limit_owner_and_exact_value_test() -> (TypeScriptOwner, TypeScriptTest) {
    let owner = TypeScriptOwner {
        name: "parseLimit".to_string(),
        file: PathBuf::from("src/limiter.ts"),
        start_line: 1,
        end_line: 8,
        owner_kind: OwnerKind::Function,
        class_name: None,
        decorated: false,
        exported_as_default: false,
        class_default_export: false,
        arity: None,
        parameters: Vec::new(),
        source_text: None,
        imports: Vec::new(),
        method_kind: TypeScriptMethodKind::Ordinary,
    };
    let test = TypeScriptTest {
        name: "parseLimit parses".to_string(),
        local_name: "parseLimit parses".to_string(),
        describe_names: Vec::new(),
        file: PathBuf::from("tests/limiter.test.ts"),
        line: 3,
        body_text: "expect(parseLimit('10')).toBe(10);".to_string(),
        assertions: vec![TypeScriptAssertion {
            matcher: "toBe".to_string(),
            argument_count: 1,
            line: 4,
            oracle_kind: OracleKind::ExactValue,
            oracle_strength: OracleStrength::Strong,
            mock_payload: None,
            error_payload: None,
            observed_expression: Some("parseLimit('10')".to_string()),
            expected_value_or_variant: Some("10".to_string()),
            has_dynamic_matcher_arg: false,
            oracle_confidence: OracleConfidence::High,
        }],
        mocks_in_file: Vec::new(),
        imports_in_file: Vec::new(),
    };
    (owner, test)
}

/// A newly added `throw` must not borrow a related test's exact-value
/// assertion as its oracle target. The oracle metadata lines are what the
/// repair-packet projection reads (RIPR-SPEC-0087 G-C); a wrong-family
/// borrow produced a "complete" packet telling the user to strengthen
/// `expect(parseLimit('10')).toBe(10)`, which cannot observe the throw.
#[test]
fn oracle_metadata_is_not_borrowed_from_a_wrong_family_assertion() -> Result<(), String> {
    let (owner, test) = parse_limit_owner_and_exact_value_test();
    let finding = classify_change(
        Path::new("src/limiter.ts"),
        4,
        "    throw new TypeError('invalid limit');",
        &[owner],
        &[test],
        None,
        &ReExportIndex::empty(),
        None,
    )
    .ok_or_else(|| "expected a finding".to_string())?;

    assert_eq!(finding.probe.family, ProbeFamily::ErrorPath);
    assert!(
        finding
            .evidence
            .iter()
            .all(|line| !line.starts_with("typescript_oracle_expected:")
                && !line.starts_with("typescript_oracle_observed:")),
        "an exact-value assertion must not be surfaced as the oracle for an error-path change; got {:?}",
        finding.evidence
    );
    Ok(())
}

/// Control for the wrong-family guard: the same exact-value assertion is
/// still surfaced for a value change it can observe, so the guard does not
/// strip oracle metadata wholesale.
#[test]
fn oracle_metadata_is_kept_for_a_matching_family_assertion() -> Result<(), String> {
    let (owner, test) = parse_limit_owner_and_exact_value_test();
    let finding = classify_change(
        Path::new("src/limiter.ts"),
        6,
        "    return n * 2;",
        &[owner],
        &[test],
        None,
        &ReExportIndex::empty(),
        None,
    )
    .ok_or_else(|| "expected a finding".to_string())?;

    assert_eq!(finding.probe.family, ProbeFamily::ReturnValue);
    assert!(
        finding
            .evidence
            .iter()
            .any(|line| line == "typescript_oracle_expected: 10"),
        "a matching-family exact-value assertion keeps its oracle metadata; got {:?}",
        finding.evidence
    );
    Ok(())
}

// ── RIPR-SPEC-0027: predicate boundary witness ───────────────────────────────

fn exact_value_test(owner_name: &str, observed: &str, expected: &str) -> TypeScriptTest {
    TypeScriptTest {
        name: format!("{owner_name} {observed}"),
        local_name: format!("{owner_name} {observed}"),
        describe_names: Vec::new(),
        file: PathBuf::from("tests/lib.test.ts"),
        line: 1,
        body_text: format!("expect({observed}).toBe({expected});"),
        assertions: vec![TypeScriptAssertion {
            matcher: "toBe".to_string(),
            argument_count: 1,
            line: 2,
            oracle_kind: OracleKind::ExactValue,
            oracle_strength: OracleStrength::Strong,
            mock_payload: None,
            error_payload: None,
            observed_expression: Some(observed.to_string()),
            expected_value_or_variant: Some(expected.to_string()),
            has_dynamic_matcher_arg: false,
            oracle_confidence: OracleConfidence::High,
        }],
        mocks_in_file: Vec::new(),
        imports_in_file: Vec::new(),
    }
}

fn classify_boundary_line(
    owner_name: &str,
    line_text: &str,
    tests: &[TypeScriptTest],
) -> Result<Finding, String> {
    classify_change(
        Path::new("src/lib.ts"),
        2,
        line_text,
        &[test_owner(owner_name, "src/lib.ts")],
        tests,
        None,
        &ReExportIndex::empty(),
        None,
    )
    .ok_or_else(|| format!("expected a finding for `{line_text}`"))
}

/// RIPR-SPEC-0027 repro: `total > 50` → `total >= 50` with strong exact
/// assertions only at 60 and 10. Neither input discriminates the changed
/// comparison, so the finding must not be `exposed`; the weak path names the
/// boundary discriminator `total == 50`.
#[test]
fn spec_0027_off_boundary_strong_assertions_do_not_expose_literal_predicate() -> Result<(), String>
{
    let tests = [
        exact_value_test("shippingFee", "shippingFee(60)", "0"),
        exact_value_test("shippingFee", "shippingFee(10)", "5"),
    ];
    let finding = classify_boundary_line("shippingFee", "  if (total >= 50) {", &tests)?;
    assert_eq!(finding.class, ExposureClass::WeaklyExposed);
    assert_eq!(
        missing_discriminator_values(&finding),
        vec!["total == 50".to_string()]
    );
    assert!(
        finding
            .missing
            .iter()
            .any(|line| line.contains("changed predicate boundary `total == 50`")),
        "boundary limitation must be named: {:?}",
        finding.missing
    );
    assert!(
        finding.evidence.iter().any(|line| line
            == "actionability_category: incomplete_repair_packet"
            || line == "actionability_category: complete_repair_packet"),
        "downgraded boundary must route to the repair path: {:?}",
        finding.evidence
    );
    Ok(())
}

/// RIPR-SPEC-0027 control: an exact assertion at the boundary literal keeps
/// the predicate `exposed` (including `50.0`, and a literal inside an object
/// argument).
#[test]
fn spec_0027_boundary_literal_exact_assertion_stays_exposed() -> Result<(), String> {
    for observed in [
        "shippingFee(50)",
        "shippingFee(50.0)",
        "await shippingFee({ total: 50 })",
    ] {
        let tests = [
            exact_value_test("shippingFee", "shippingFee(60)", "0"),
            exact_value_test("shippingFee", observed, "0"),
        ];
        let finding = classify_boundary_line("shippingFee", "  if (total >= 50) {", &tests)?;
        assert_eq!(
            finding.class,
            ExposureClass::Exposed,
            "`{observed}` sits at the boundary"
        );
        assert!(finding.activation.missing_discriminators.is_empty());
    }
    Ok(())
}

/// RIPR-SPEC-0027 negative controls for the witness itself: a boundary value
/// seen in a weak assertion or in a test set whose related owner calls never
/// sit at the boundary does not witness the boundary. Every test here
/// references the owner, so the F5-9 relation gate is passed and the boundary
/// rule fail-closes to `weakly_exposed`.
#[test]
fn spec_0027_boundary_witness_fails_closed_without_strong_owner_call_at_boundary()
-> Result<(), String> {
    let mut weak_at_boundary = exact_value_test("shippingFee", "shippingFee(50)", "0");
    for assertion in &mut weak_at_boundary.assertions {
        assertion.oracle_kind = OracleKind::RelationalCheck;
        assertion.oracle_strength = OracleStrength::Weak;
    }
    let cases = [
        vec![
            exact_value_test("shippingFee", "shippingFee(60)", "0"),
            weak_at_boundary,
        ],
        vec![
            exact_value_test("shippingFee", "shippingFee(60)", "0"),
            exact_value_test("shippingFee", "otherShippingFee(50)", "0"),
        ],
        vec![exact_value_test("shippingFee", "shippingFee(500)", "0")],
    ];
    for tests in cases {
        let finding = classify_boundary_line("shippingFee", "  if (total >= 50) {", &tests)?;
        assert_eq!(
            finding.class,
            ExposureClass::WeaklyExposed,
            "tests {:?} must not witness the boundary",
            tests
                .iter()
                .map(|test| test.body_text.as_str())
                .collect::<Vec<_>>()
        );
    }
    Ok(())
}

/// Should-stay-`weakly_exposed` control (RIPR-SPEC-0027 false-witness family):
/// a strong assertion that calls a SAME-NAMED method on a DIFFERENT receiver
/// (`expect(other.total(50)).toBe(120)`) must not witness the changed
/// predicate boundary of owner `total`. The literal `50` is present, but the
/// observed call sits on `other`, not on the owner, so the boundary is not
/// witnessed and the finding must fail closed to `weakly_exposed`.
#[test]
fn spec_0027_same_named_method_on_other_receiver_does_not_witness() -> Result<(), String> {
    // The first test anchors the owner relation with a real owner call that
    // is NOT at the boundary literal (`60` vs boundary `50`). The second
    // asserts on a same-named method of a DIFFERENT receiver at the literal;
    // its namespace import binds `other` to `src/other-lib`, NOT to the
    // owner's module, so it is a related, oracle-eligible test whose only
    // boundary-shaped assertion must still be rejected.
    let mut foreign = exact_value_test("total", "other.total(50)", "120");
    foreign.imports_in_file = vec![TypeScriptImport {
        source: "../src/other-lib".to_string(),
        imported: None,
        local: "other".to_string(),
        namespace: true,
    }];
    let tests = [exact_value_test("total", "total(60)", "120"), foreign];
    let finding = classify_boundary_line("total", "  if (total >= 50) {", &tests)?;
    assert_eq!(
        finding.class,
        ExposureClass::WeaklyExposed,
        "a same-named method on a different receiver must not witness the boundary"
    );
    assert!(
        finding
            .missing
            .iter()
            .any(|line| line.contains("changed predicate boundary `total == 50`")),
        "boundary limitation must be named: {:?}",
        finding.missing
    );
    Ok(())
}

/// Over-correction control (RIPR-SPEC-0027 receiver resolution): a member
/// call whose receiver is a NAMESPACE IMPORT of the owner's own module
/// (`import * as pricing from "../src/pricing"` observing
/// `pricing.applyDiscount(100, 100)`) IS a genuine owner call. With the
/// boundary `amount == threshold` (no literal operands), the two identical
/// arguments `(100, 100)` witness the boundary and the finding MUST stay
/// `exposed`. This pins the namespace-import witness that a blanket dot-skip
/// would have falsely downgraded.
#[test]
fn spec_0027_namespace_import_member_call_witnesses_boundary() -> Result<(), String> {
    let owner = TypeScriptOwner {
        name: "applyDiscount".to_string(),
        file: PathBuf::from("src/pricing.ts"),
        start_line: 1,
        end_line: 10,
        owner_kind: OwnerKind::Function,
        class_name: None,
        decorated: false,
        exported_as_default: false,
        class_default_export: false,
        imports: Vec::new(),
        method_kind: TypeScriptMethodKind::Ordinary,
        arity: None,
        parameters: Vec::new(),
        source_text: None,
    };
    let mut test = exact_value_test("applyDiscount", "pricing.applyDiscount(100, 100)", "90");
    test.file = PathBuf::from("tests/pricing.test.ts");
    test.imports_in_file = vec![TypeScriptImport {
        source: "../src/pricing".to_string(),
        imported: None,
        local: "pricing".to_string(),
        namespace: true,
    }];
    let finding = classify_change(
        Path::new("src/pricing.ts"),
        2,
        "  if (amount >= threshold) {",
        &[owner],
        &[test],
        None,
        &ReExportIndex::empty(),
        None,
    )
    .ok_or_else(|| "expected a finding".to_string())?;
    assert_eq!(
        finding.class,
        ExposureClass::Exposed,
        "a namespace-import member call on the owner's module must witness the boundary"
    );
    Ok(())
}

/// RIPR-SPEC-0027 + F5-9: a test that never references the owner is not
/// related at all, so the relation gate answers `no_static_path` before the
/// boundary witness is consulted.
#[test]
fn spec_0027_unrelated_test_does_not_reach_the_boundary_witness() -> Result<(), String> {
    let tests = [exact_value_test("shippingFee", "result", "0")];
    let finding = classify_boundary_line("shippingFee", "  if (total >= 50) {", &tests)?;
    assert_eq!(finding.class, ExposureClass::NoStaticPath);
    assert_eq!(finding.ripr.reach.state, StageState::No);
    assert!(
        finding.missing.iter().any(|line| line
            == "No test references `shippingFee(` — add a test that calls the changed owner."),
        "no_static_path must name the missing owner reference: {:?}",
        finding.missing
    );
    Ok(())
}

/// RIPR-SPEC-0027: without literal operands the adapter only accepts an owner
/// call with two identical arguments as the boundary witness.
#[test]
fn spec_0027_non_literal_boundary_requires_identical_arguments() -> Result<(), String> {
    let below = [exact_value_test("isAllowed", "isAllowed(4, 5)", "true")];
    let finding = classify_boundary_line("isAllowed", "  if (count <= limit) {", &below)?;
    assert_eq!(finding.class, ExposureClass::WeaklyExposed);
    assert_eq!(
        missing_discriminator_values(&finding),
        vec!["count == limit".to_string()]
    );

    let at = [
        exact_value_test("isAllowed", "isAllowed(4, 5)", "true"),
        exact_value_test("isAllowed", "isAllowed(5, 5)", "false"),
    ];
    let finding = classify_boundary_line("isAllowed", "  if (count <= limit) {", &at)?;
    assert_eq!(finding.class, ExposureClass::Exposed);

    // Object-literal props name both operands: equal values witness the
    // boundary, unequal values do not.
    let props_at = [exact_value_test(
        "PriceLabel",
        "PriceLabel({ amount: 100, threshold: 100 })",
        "90",
    )];
    let finding = classify_boundary_line("PriceLabel", "  if (amount >= threshold) {", &props_at)?;
    assert_eq!(finding.class, ExposureClass::Exposed);
    let props_above = [exact_value_test(
        "PriceLabel",
        "PriceLabel({ amount: 100, threshold: 50 })",
        "90",
    )];
    let finding =
        classify_boundary_line("PriceLabel", "  if (amount >= threshold) {", &props_above)?;
    assert_eq!(finding.class, ExposureClass::WeaklyExposed);
    Ok(())
}

/// RIPR-SPEC-0027: string-literal equality predicates need the literal in the
/// observed owner call.
#[test]
fn spec_0027_string_literal_predicate_requires_literal_argument() -> Result<(), String> {
    let other = [exact_value_test(
        "applyDiscount",
        "applyDiscount(100, 'silver')",
        "95",
    )];
    let finding = classify_boundary_line("applyDiscount", "  if (tier === 'gold') {", &other)?;
    assert_eq!(finding.class, ExposureClass::WeaklyExposed);

    let gold = [exact_value_test(
        "applyDiscount",
        "applyDiscount(100, \"gold\")",
        "90",
    )];
    let finding = classify_boundary_line("applyDiscount", "  if (tier === 'gold') {", &gold)?;
    assert_eq!(finding.class, ExposureClass::Exposed);
    Ok(())
}

/// RIPR-SPEC-0027: a new guard with no comparison (`Number.isNaN(n)`) is not
/// exposed by a strong assertion on the unguarded path, and gets no invented
/// discriminator.
#[test]
fn spec_0027_guard_without_comparison_is_not_exposed() -> Result<(), String> {
    let tests = [exact_value_test("parseLimit", "parseLimit('10')", "10")];
    let finding = classify_boundary_line("parseLimit", "  if (Number.isNaN(n)) {", &tests)?;
    assert_eq!(finding.class, ExposureClass::WeaklyExposed);
    assert!(finding.activation.missing_discriminators.is_empty());
    assert!(
        finding
            .evidence
            .iter()
            .any(|line| line == "actionability_category: missing_target_shape"),
        "no discriminator means no repair packet: {:?}",
        finding.evidence
    );
    Ok(())
}

/// RIPR-SPEC-0027: an ambiguous-fallback shape is never `exposed`, and
/// punctuation- or comment-only added lines produce no probe at all.
#[test]
fn spec_0027_ambiguous_fallback_is_never_exposed_and_punctuation_is_ignored() -> Result<(), String>
{
    let tests = [exact_value_test("parseLimit", "parseLimit('10')", "10")];
    let finding = classify_boundary_line("parseLimit", "  n.value", &tests)?;
    assert!(!classify_probe_shape_detail("  n.value").specific);
    assert_eq!(finding.class, ExposureClass::WeaklyExposed);
    assert!(finding.activation.missing_discriminators.is_empty());

    for ignored in ["  }", "  });", ")", "  ],", "", "  // note", "  /* note */"] {
        assert!(
            should_ignore_typescript_changed_line(ignored),
            "`{ignored}` carries no behavior"
        );
    }
    for kept in [
        "  if (x) {",
        "  } else {",
        "  return;",
        "  foo();",
        "  x = 1;",
    ] {
        assert!(
            !should_ignore_typescript_changed_line(kept),
            "`{kept}` must still be classified"
        );
    }
    Ok(())
}

/// RIPR-SPEC-0027: the boundary witness does not touch non-predicate families;
/// an exact return-value assertion still exposes a return-value change.
#[test]
fn spec_0027_boundary_witness_leaves_return_value_family_exposed() -> Result<(), String> {
    let tests = [exact_value_test("shippingFee", "shippingFee(60)", "0")];
    let finding = classify_boundary_line("shippingFee", "  return 0;", &tests)?;
    assert_eq!(finding.class, ExposureClass::Exposed);
    Ok(())
}

// ── RIPR-SPEC-0027 boundary-witness identity and liveness guards (#4102) ─────

/// Owner facts mirroring the #4102 fixture owner: a single-parameter
/// `applyDiscount(total)` whose changed predicate `total >= 100` selects
/// between `return total * 0.9;` (changed behavior at the boundary input:
/// 90) and `return total;` (unchanged behavior there: 100). These are the
/// extraction-populated facts the witness needs for position/arity matching
/// and expected-side liveness.
fn boundary_witness_owner() -> TypeScriptOwner {
    TypeScriptOwner {
        name: "applyDiscount".to_string(),
        file: PathBuf::from("src/lib.ts"),
        start_line: 1,
        end_line: 6,
        owner_kind: OwnerKind::Function,
        class_name: None,
        decorated: false,
        exported_as_default: false,
        class_default_export: false,
        imports: Vec::new(),
        method_kind: TypeScriptMethodKind::Ordinary,
        arity: Some(1),
        parameters: vec!["total".to_string()],
        source_text: Some(
            concat!(
                "export function applyDiscount(total: number): number {\n",
                "    if (total >= 100) {\n",
                "        return total * 0.9;\n",
                "    }\n",
                "    return total;\n",
                "}",
            )
            .to_string(),
        ),
    }
}

fn classify_boundary_line_for_owner(
    owner: &TypeScriptOwner,
    line_text: &str,
    tests: &[TypeScriptTest],
) -> Result<Finding, String> {
    classify_change(
        Path::new("src/lib.ts"),
        2,
        line_text,
        std::slice::from_ref(owner),
        tests,
        None,
        &ReExportIndex::empty(),
        None,
    )
    .ok_or_else(|| format!("expected a finding for `{line_text}`"))
}

/// #4102 guard 1 (dead argument): `applyDiscount(150, 100)` against a
/// single-parameter owner puts the boundary literal `100` in an argument the
/// changed comparison (`total >= 100`) never reads — the owner reads only
/// `total` (parameter 0), and the expected `135` is identical under both
/// behaviors at input 150. The boundary must fail closed.
#[test]
fn spec_0027_boundary_literal_in_unread_argument_does_not_witness() -> Result<(), String> {
    let owner = boundary_witness_owner();
    let tests = [exact_value_test(
        "applyDiscount",
        "applyDiscount(150, 100)",
        "135",
    )];
    let finding = classify_boundary_line_for_owner(&owner, "  if (total >= 100) {", &tests)?;
    assert_eq!(
        finding.class,
        ExposureClass::WeaklyExposed,
        "a literal in an argument the single-parameter owner never reads must not witness"
    );
    assert!(
        finding
            .missing
            .iter()
            .any(|line| line.contains("changed predicate boundary `total == 100`")),
        "boundary limitation must be named: {:?}",
        finding.missing
    );
    Ok(())
}

/// #4102 guard 1 (padding argument): `applyDiscount(0, 100)` — the boundary
/// literal sits in the dead second argument; input `0` behaves identically
/// under both comparisons. The boundary must fail closed.
#[test]
fn spec_0027_boundary_literal_in_padding_argument_does_not_witness() -> Result<(), String> {
    let owner = boundary_witness_owner();
    let tests = [exact_value_test(
        "applyDiscount",
        "applyDiscount(0, 100)",
        "0",
    )];
    let finding = classify_boundary_line_for_owner(&owner, "  if (total >= 100) {", &tests)?;
    assert_eq!(
        finding.class,
        ExposureClass::WeaklyExposed,
        "a literal in a padding argument must not witness"
    );
    Ok(())
}

/// #4102 guard 2 (nested containment): `applyDiscount(price + 100)` with
/// `price = 60` — the argument tokenizes beyond the literal, so the effective
/// input (160) is not the boundary even though the literal text is present.
/// Must fail closed even without owner parameter facts.
#[test]
fn spec_0027_contained_boundary_literal_does_not_witness() -> Result<(), String> {
    let tests = [exact_value_test(
        "applyDiscount",
        "applyDiscount(price + 100)",
        "144",
    )];
    let finding = classify_boundary_line("applyDiscount", "  if (total >= 100) {", &tests)?;
    assert_eq!(
        finding.class,
        ExposureClass::WeaklyExposed,
        "a literal contained in a larger argument expression must not witness"
    );
    Ok(())
}

/// #4102 guard 3 (receiver-qualified same-name): `pricing.applyDiscount(100)`
/// where `pricing` is a test-local object carrying an unrelated same-name
/// member is NOT an owner call — only a namespace import of the owner's own
/// module binds a receiver. The anchor `applyDiscount(150)` call elsewhere in
/// the body does not rescue the boundary-shaped shadow assertion.
#[test]
fn spec_0027_receiver_qualified_same_name_object_does_not_witness() -> Result<(), String> {
    let owner = boundary_witness_owner();
    let mut test = exact_value_test("applyDiscount", "pricing.applyDiscount(100)", "200");
    test.body_text =
        "const unused = applyDiscount(150);\nexpect(pricing.applyDiscount(100)).toBe(200);"
            .to_string();
    let finding = classify_boundary_line_for_owner(&owner, "  if (total >= 100) {", &[test])?;
    assert_eq!(
        finding.class,
        ExposureClass::WeaklyExposed,
        "a same-named member on an unresolvable receiver must not witness"
    );
    Ok(())
}

/// Control for guard 3: a namespace import of the owner's own module DOES
/// bind the receiver — `pricing.applyDiscount(100)` at the boundary with a
/// live expected value stays `exposed` (the #4102 `w15` genuine control).
#[test]
fn spec_0027_namespace_receiver_owner_call_stays_exposed() -> Result<(), String> {
    let owner = boundary_witness_owner();
    let mut test = exact_value_test("applyDiscount", "pricing.applyDiscount(100)", "90");
    test.imports_in_file = vec![TypeScriptImport {
        source: "../src/lib".to_string(),
        imported: None,
        local: "pricing".to_string(),
        namespace: true,
    }];
    let finding = classify_boundary_line_for_owner(&owner, "  if (total >= 100) {", &[test])?;
    assert_eq!(
        finding.class,
        ExposureClass::Exposed,
        "a namespace-import receiver at the boundary is a genuine witness"
    );
    Ok(())
}

/// #4102 guard 4 (body-local shadow): a `function applyDiscount(...)`
/// declaration inside the test body executes the shadow, not the changed
/// owner, so its assertions cannot witness the owner's boundary — and the
/// owner-call relation must not be credited over the shadow. The shadowed
/// reference severs owner-call relations entirely (the same tradeoff the
/// landed alias-arm shadow guard makes): the changed owner is genuinely
/// unreferenced by this test, so `no_static_path` is the honest verdict and
/// its "add a test that calls the changed owner" guidance is true.
#[test]
fn spec_0027_body_local_shadow_does_not_witness() -> Result<(), String> {
    let owner = boundary_witness_owner();
    let mut test = exact_value_test("applyDiscount", "applyDiscount(100)", "42");
    test.body_text = "function applyDiscount(total: number): number {\n        return 42;\n    }\n    expect(applyDiscount(100)).toBe(42);".to_string();
    let finding = classify_boundary_line_for_owner(&owner, "  if (total >= 100) {", &[test])?;
    assert_eq!(
        finding.class,
        ExposureClass::NoStaticPath,
        "a body-local same-name declaration must not witness the owner boundary, and the \
         owner-call relation must not be credited over the shadow"
    );
    assert!(
        finding.related_tests.is_empty(),
        "a shadowed body must not credit owner-call relations: {:?}",
        finding.related_tests
    );
    Ok(())
}

/// #4102 guard 5a (self-comparing expectation): `toBe(applyDiscount(100))`
/// compares the owner to itself — statically detectable, never
/// discriminating. Must fail closed.
#[test]
fn spec_0027_tautological_expected_owner_call_does_not_witness() -> Result<(), String> {
    let owner = boundary_witness_owner();
    let tests = [exact_value_test(
        "applyDiscount",
        "applyDiscount(100)",
        "applyDiscount(100)",
    )];
    let finding = classify_boundary_line_for_owner(&owner, "  if (total >= 100) {", &tests)?;
    assert_eq!(
        finding.class,
        ExposureClass::WeaklyExposed,
        "an expected side that calls the owner itself must not witness"
    );
    Ok(())
}

/// #4102 guard 5b (dead expected): `toBe(999)` is wrong under BOTH behaviors
/// at the boundary input (changed: 90, unchanged: 100) — statically provable
/// from the owner body and the changed comparison. Must fail closed.
#[test]
fn spec_0027_dead_expected_value_does_not_witness() -> Result<(), String> {
    let owner = boundary_witness_owner();
    let tests = [exact_value_test(
        "applyDiscount",
        "applyDiscount(100)",
        "999",
    )];
    let finding = classify_boundary_line_for_owner(&owner, "  if (total >= 100) {", &tests)?;
    assert_eq!(
        finding.class,
        ExposureClass::WeaklyExposed,
        "an expected literal that matches neither behavior at the boundary must not witness"
    );
    Ok(())
}

/// Control for guard 5b: the honest boundary assertion `toBe(90)` — the
/// changed behavior's value at the boundary input, distinct from the
/// unchanged behavior's `100` — stays `exposed` with discriminate=yes.
#[test]
fn spec_0027_live_expected_value_stays_exposed() -> Result<(), String> {
    let owner = boundary_witness_owner();
    let tests = [exact_value_test(
        "applyDiscount",
        "applyDiscount(100)",
        "90",
    )];
    let finding = classify_boundary_line_for_owner(&owner, "  if (total >= 100) {", &tests)?;
    assert_eq!(
        finding.class,
        ExposureClass::Exposed,
        "the genuine boundary assertion must stay exposed"
    );
    assert!(
        matches!(finding.ripr.reveal.discriminate.state, StageState::Yes),
        "discriminate must be Yes for a live boundary assertion, got {:?}",
        finding.ripr.reveal.discriminate.state
    );
    Ok(())
}

/// Control for guard 2's object-pin exception: an object argument that binds
/// the comparison operand's field to the boundary value
/// (`applyDiscount({ total: 100 })`) keeps the effective input known and
/// stays `exposed`.
#[test]
fn spec_0027_object_pin_at_read_position_stays_exposed() -> Result<(), String> {
    let owner = boundary_witness_owner();
    let tests = [exact_value_test(
        "applyDiscount",
        "applyDiscount({ total: 100 })",
        "90",
    )];
    let finding = classify_boundary_line_for_owner(&owner, "  if (total >= 100) {", &tests)?;
    assert_eq!(
        finding.class,
        ExposureClass::Exposed,
        "an object pin of the read operand at the boundary is a genuine witness"
    );
    Ok(())
}

/// #4117 review of guard 5b (fail-closed branch scan): a parameter
/// reassignment before the branch `return` invalidates the folded branch
/// value, so `branch_return_expressions` must give up (`None`) instead of
/// folding a guessed expression — otherwise the dead-expected guard skips a
/// genuine boundary witness.
#[test]
fn branch_return_expressions_fails_closed_on_intervening_statement() {
    let lines: Vec<&str> = [
        "export function applyDiscount(total: number): number {",
        "    if (total >= 100) {",
        "        total = 90;",
        "        return total * 0.9;",
        "    }",
        "    return total;",
        "}",
    ]
    .into_iter()
    .collect();
    assert_eq!(
        branch_return_expressions(&lines, 1),
        None,
        "a parameter reassignment before the branch return must fail the scan closed"
    );
}

/// The fallthrough path fails closed the same way (#4117 review): a statement
/// before the fall-through `return` (here a reassignment) makes the folded
/// value wrong, so the scan is unattributable.
#[test]
fn branch_return_expressions_fails_closed_before_fallthrough_return() {
    let lines: Vec<&str> = [
        "export function applyDiscount(total: number): number {",
        "    if (total >= 100) {",
        "        return total * 0.9;",
        "    }",
        "    total = total - 5;",
        "    return total;",
        "}",
    ]
    .into_iter()
    .collect();
    assert_eq!(
        branch_return_expressions(&lines, 1),
        None,
        "a statement before the fall-through return must fail the scan closed"
    );
}

/// Control for the fail-closed branch scan: the attributed shapes are
/// unchanged — a plain two-`return` owner body still folds to both branch
/// expressions, so the dead-expected guard keeps its discriminating power.
#[test]
fn branch_return_expressions_single_return_shapes_still_attributed() {
    let lines: Vec<&str> = [
        "export function applyDiscount(total: number): number {",
        "    if (total >= 100) {",
        "        return total * 0.9;",
        "    }",
        "    return total;",
        "}",
    ]
    .into_iter()
    .collect();
    assert_eq!(
        branch_return_expressions(&lines, 1),
        Some(("total * 0.9".to_string(), "total".to_string()))
    );
}

/// #4117 review of the liveness lookup: a duplicated predicate line cannot be
/// attributed through the declared offset (a multiline declarator can shift
/// the offset onto the identical line of a different function), so the check
/// gives up instead of reading the wrong branch pair.
#[test]
fn expected_side_is_live_gives_up_on_duplicated_predicate_line() {
    let owner = TypeScriptOwner {
        source_text: Some(
            concat!(
                "export function applyDiscount(total: number): number {\n",
                "    if (total >= 100) {\n",
                "        return total * 0.9;\n",
                "    }\n",
                "    return total;\n",
                "}\n",
                "\n",
                "export function legacyDiscount(total: number): number {\n",
                "    if (total >= 100) {\n",
                "        return 42;\n",
                "    }\n",
                "    return total;\n",
                "}",
            )
            .to_string(),
        ),
        ..boundary_witness_owner()
    };
    assert_eq!(
        expected_side_is_live(
            &owner,
            "  if (total >= 100) {",
            "total",
            "100",
            &["100".to_string()],
            "42",
        ),
        None,
        "two identical predicate lines must not attribute a branch pair"
    );
}

/// Control: with a unique predicate line the liveness fold still resolves —
/// a live expected value is `Some(true)` and a dead one `Some(false)`.
#[test]
fn expected_side_is_live_unique_line_still_folds_liveness() {
    let owner = boundary_witness_owner();
    assert_eq!(
        expected_side_is_live(
            &owner,
            "  if (total >= 100) {",
            "total",
            "100",
            &["100".to_string()],
            "90",
        ),
        Some(true)
    );
    assert_eq!(
        expected_side_is_live(
            &owner,
            "  if (total >= 100) {",
            "total",
            "100",
            &["100".to_string()],
            "999",
        ),
        Some(false)
    );
}

/// #4117 review (fail-closed branch scan, end to end): the test pins the
/// genuine changed value — at input 100 the changed comparison enters the
/// branch, reassigns `total` to `90`, and returns `81`, while the unchanged
/// comparison returns `100`. The branch scan must give up on the reassignment
/// instead of folding `total * 0.9` to a value the reassignment already
/// replaced, which would have skipped this witness as a dead expectation.
#[test]
fn spec_0027_reassigned_parameter_branch_stays_exposed() -> Result<(), String> {
    let owner = TypeScriptOwner {
        source_text: Some(
            concat!(
                "export function applyDiscount(total: number): number {\n",
                "    if (total >= 100) {\n",
                "        total = 90;\n",
                "        return total * 0.9;\n",
                "    }\n",
                "    return total;\n",
                "}",
            )
            .to_string(),
        ),
        ..boundary_witness_owner()
    };
    let tests = [exact_value_test(
        "applyDiscount",
        "applyDiscount(100)",
        "81",
    )];
    let finding = classify_boundary_line_for_owner(&owner, "  if (total >= 100) {", &tests)?;
    assert_eq!(
        finding.class,
        ExposureClass::Exposed,
        "a reassigned parameter before the branch return must not let the dead-expected \
         guard skip a genuine witness"
    );
    Ok(())
}

/// #4117 review (uniqueness before offset, end to end): the identical
/// predicate line of a second function makes the declared line offset
/// untrustworthy; the liveness check must give up so the genuine witness
/// stays exposed instead of being skipped as a dead expectation attributed
/// from the wrong function's branches.
#[test]
fn spec_0027_duplicate_predicate_line_keeps_genuine_witness_exposed() -> Result<(), String> {
    let owner = TypeScriptOwner {
        source_text: Some(
            concat!(
                "export function applyDiscount(total: number): number {\n",
                "    if (total >= 100) {\n",
                "        return total * 0.9;\n",
                "    }\n",
                "    return total;\n",
                "}\n",
                "\n",
                "export function legacyDiscount(total: number): number {\n",
                "    if (total >= 100) {\n",
                "        return 42;\n",
                "    }\n",
                "    return total;\n",
                "}",
            )
            .to_string(),
        ),
        ..boundary_witness_owner()
    };
    let tests = [exact_value_test(
        "applyDiscount",
        "applyDiscount(100)",
        "42",
    )];
    let finding = classify_boundary_line_for_owner(&owner, "  if (total >= 100) {", &tests)?;
    assert_eq!(
        finding.class,
        ExposureClass::Exposed,
        "a duplicated predicate line must not attribute the wrong branch pair and skip \
         the witness"
    );
    Ok(())
}

/// #4117 review (scope-aware shadow guard): a `const applyDiscount` declared
/// inside a nested block does not shadow the imported-owner call made outside
/// that block — the direct-owner relation and the boundary witness survive.
#[test]
fn spec_0027_nested_block_shadow_does_not_reject_outer_owner_call() -> Result<(), String> {
    let owner = boundary_witness_owner();
    let mut test = exact_value_test("applyDiscount", "applyDiscount(100)", "90");
    test.body_text = concat!(
        "if (warm) {\n",
        "    const applyDiscount = () => 42;\n",
        "}\n",
        "expect(applyDiscount(100)).toBe(90);",
    )
    .to_string();
    let finding = classify_boundary_line_for_owner(&owner, "  if (total >= 100) {", &[test])?;
    assert_eq!(
        finding.class,
        ExposureClass::Exposed,
        "a nested-block declaration must not shadow the outer imported-owner call"
    );
    assert!(
        finding
            .related_tests
            .iter()
            .any(|related| related.relation_reason
                == Some(crate::domain::RelationReason::DirectOwnerCall)),
        "the outer call must still credit the direct-owner relation: {:?}",
        finding.related_tests
    );
    Ok(())
}

/// Control for the scope-aware shadow guard: a call INSIDE the block that
/// declares the local still executes the shadow, so the guard must keep
/// rejecting the relation and the boundary witness (over-credit stays closed).
#[test]
fn spec_0027_nested_block_shadow_still_rejects_call_in_scope() -> Result<(), String> {
    let owner = boundary_witness_owner();
    let mut test = exact_value_test("applyDiscount", "applyDiscount(100)", "42");
    test.body_text = concat!(
        "if (warm) {\n",
        "    const applyDiscount = () => 42;\n",
        "    expect(applyDiscount(100)).toBe(42);\n",
        "}",
    )
    .to_string();
    let finding = classify_boundary_line_for_owner(&owner, "  if (total >= 100) {", &[test])?;
    assert_eq!(
        finding.class,
        ExposureClass::NoStaticPath,
        "a call inside the declaring block still executes the shadow"
    );
    assert!(
        finding.related_tests.is_empty(),
        "the shadowed in-block call must not credit owner-call relations: {:?}",
        finding.related_tests
    );
    Ok(())
}

/// #4117 review (TDZ): a `const applyDiscount` declaration binds its WHOLE
/// block, so a same-block call placed BEFORE the declaration still executes
/// the shadow — it can never reach the imported owner (the binding is in the
/// temporal dead zone at that point, so the call throws). Crediting the
/// owner-call relation for it is an over-credit; the guard must reject the
/// relation and the boundary witness fail-closed.
#[test]
fn spec_0027_same_block_call_before_const_declaration_still_shadowed() -> Result<(), String> {
    let owner = boundary_witness_owner();
    let mut test = exact_value_test("applyDiscount", "applyDiscount(100)", "42");
    test.body_text = concat!(
        "const run = () => applyDiscount(100);\n",
        "const applyDiscount = () => 42;\n",
        "expect(run()).toBe(42);",
    )
    .to_string();
    let finding = classify_boundary_line_for_owner(&owner, "  if (total >= 100) {", &[test])?;
    assert_eq!(
        finding.class,
        ExposureClass::NoStaticPath,
        "a same-block call before the const declaration still executes the shadow (TDZ)"
    );
    assert!(
        finding.related_tests.is_empty(),
        "the pre-declaration same-block call must not credit owner-call relations: {:?}",
        finding.related_tests
    );
    Ok(())
}

// ── F5-9: a test is related to an owner only when it references the owner ────

/// Owners from the F5-9 re-walk shape: `discountedTotal` (tested) and a new
/// `loyaltyPrice` (no test references it) in the same `src/pricing.ts`.
fn f5_9_owners() -> Vec<TypeScriptOwner> {
    vec![
        TypeScriptOwner {
            start_line: 3,
            end_line: 8,
            ..test_owner("discountedTotal", "src/pricing.ts")
        },
        TypeScriptOwner {
            start_line: 10,
            end_line: 15,
            ..test_owner("loyaltyPrice", "src/pricing.ts")
        },
    ]
}

const F5_9_SIBLING_ONLY_TESTS: &str = r#"import { describe, it, expect } from "vitest";
import { discountedTotal } from "../src/pricing";

describe("discountedTotal", () => {
  it("no discount below threshold", () => {
    expect(discountedTotal(5000)).toBe(5000);
  });
  it("discounts far above threshold", () => {
    expect(discountedTotal(20000)).toBe(18000);
  });
});
"#;

fn classify_f5_9_loyalty_line(tests: &[TypeScriptTest]) -> Result<Finding, String> {
    classify_change(
        Path::new("src/pricing.ts"),
        11,
        "  if (years >= 5) {",
        &f5_9_owners(),
        tests,
        None,
        &ReExportIndex::empty(),
        None,
    )
    .ok_or_else(|| "expected a loyaltyPrice finding".to_string())
}

#[test]
fn same_stem_tests_that_only_call_a_sibling_owner_are_not_related() -> Result<(), String> {
    let tests = extract_tests(Path::new("test/pricing.test.ts"), F5_9_SIBLING_ONLY_TESTS);
    assert_eq!(tests.len(), 2, "fixture must parse both sibling tests");
    assert!(
        tests
            .iter()
            .all(|test| test.body_text.contains("discountedTotal(")),
        "both parsed tests must call the sibling owner"
    );

    let finding = classify_f5_9_loyalty_line(&tests)?;

    assert_eq!(finding.class, ExposureClass::NoStaticPath);
    assert_eq!(finding.ripr.reach.state, StageState::No);
    assert!(finding.related_tests.is_empty());
    assert_eq!(
        finding.ripr.reach.summary,
        "0 related test(s) found for owner `loyaltyPrice`"
    );
    assert!(
        finding.missing.iter().any(|line| line
            == "No test references `loyaltyPrice(` — add a test that calls the changed owner."),
        "no_static_path must name the missing owner reference: {:?}",
        finding.missing
    );
    assert!(
        !finding
            .evidence
            .iter()
            .any(|line| line.starts_with("related_test_relation:")),
        "no heuristic relation may be disclosed: {:?}",
        finding.evidence
    );
    Ok(())
}

#[test]
fn same_stem_tests_still_relate_the_sibling_owner_they_call() -> Result<(), String> {
    let tests = extract_tests(Path::new("test/pricing.test.ts"), F5_9_SIBLING_ONLY_TESTS);
    let finding = classify_change(
        Path::new("src/pricing.ts"),
        4,
        "  if (amount >= DISCOUNT_THRESHOLD) {",
        &f5_9_owners(),
        &tests,
        None,
        &ReExportIndex::empty(),
        None,
    )
    .ok_or_else(|| "expected a discountedTotal finding".to_string())?;

    assert_eq!(finding.ripr.reach.state, StageState::Yes);
    assert_eq!(finding.related_tests.len(), 2);
    Ok(())
}

#[test]
fn a_test_that_calls_the_owner_is_related_and_sibling_only_tests_are_not() -> Result<(), String> {
    let source = r#"import { describe, it, expect } from "vitest";
import { discountedTotal, loyaltyPrice } from "../src/pricing";

describe("pricing", () => {
  it("no discount below threshold", () => {
    expect(discountedTotal(5000)).toBe(5000);
  });
  it("loyal customers get five percent off", () => {
    expect(loyaltyPrice(1000, 5)).toBe(950);
  });
});
"#;
    let tests = extract_tests(Path::new("test/pricing.test.ts"), source);
    assert_eq!(tests.len(), 2);

    let finding = classify_f5_9_loyalty_line(&tests)?;

    assert_eq!(finding.ripr.reach.state, StageState::Yes);
    let names: Vec<&str> = finding
        .related_tests
        .iter()
        .map(|test| test.name.as_str())
        .collect();
    assert_eq!(names, vec!["pricing loyal customers get five percent off"]);
    Ok(())
}

#[test]
fn a_non_call_reference_keeps_a_weak_heuristic_link() -> Result<(), String> {
    let source = r#"import { expect, it } from "vitest";
import { loyaltyPrice } from "../src/pricing";

it("prices every tier", () => {
  const prices = [1000, 2000].map((amount) => amount);
  expect(prices.map((amount) => amount)).toEqual([1000, 2000]);
  expect(loyaltyPrice).toBeTypeOf("function");
});
"#;
    let tests = extract_tests(Path::new("test/pricing.test.ts"), source);
    assert_eq!(tests.len(), 1);

    let finding = classify_f5_9_loyalty_line(&tests)?;

    assert_eq!(finding.class, ExposureClass::WeaklyExposed);
    assert_eq!(finding.ripr.reach.state, StageState::Weak);
    assert!(
        finding
            .evidence
            .iter()
            .any(|line| line == "related_test_relation: same_file_proximity (prices every tier)")
    );
    Ok(())
}

#[test]
fn owner_names_in_titles_comments_strings_and_keys_are_not_references() {
    let owner = &f5_9_owners()[1];
    let source = r#"describe("loyaltyPrice", () => {
  it("loyaltyPrice is documented", () => {
    // loyaltyPrice(1000, 5) is covered elsewhere
    /* loyaltyPrice */
    const label = "loyaltyPrice";
    const config = { loyaltyPrice: 5, other: 1 };
    const also = { other: 1, loyaltyPrice: 5 };
    expect(label.length + config.other + also.other).toBe(14);
  });
});
"#;
    let tests = extract_tests(Path::new("test/pricing.test.ts"), source);
    assert_eq!(tests.len(), 1);

    let candidates = related_test_candidates(owner, &tests, None, &ReExportIndex::empty(), None);

    assert!(
        candidates.is_empty(),
        "title, describe, comment, string and object-key mentions are not references: {:?}",
        candidates
            .iter()
            .map(|candidate| candidate.relation)
            .collect::<Vec<_>>()
    );
}

#[test]
fn renamed_and_namespace_references_relate_the_owner_but_member_calls_do_not() {
    let owner = &f5_9_owners()[1];
    let renamed = extract_tests(
        Path::new("test/pricing.test.ts"),
        r#"import { loyaltyPrice as lp } from "../src/pricing";
it("tiers", () => {
  const price = lp;
  expect(typeof price).toBe("function");
});
"#,
    );
    let namespace = extract_tests(
        Path::new("test/pricing.test.ts"),
        r#"import * as pricing from "../src/pricing";
it("tiers", () => {
  const price = pricing.loyaltyPrice;
  expect(typeof price).toBe("function");
});
"#,
    );
    let member_call = extract_tests(
        Path::new("test/pricing.test.ts"),
        r#"it("tiers", () => {
  const result = (globalThis as any).pricing?.loyaltyPrice(1000, 5) ?? 950;
  expect(result).toBe(950);
});
"#,
    );
    let shadowed = extract_tests(
        Path::new("test/pricing.test.ts"),
        r#"it("tiers", () => {
  const loyaltyPrice = (amount: number) => amount;
  expect(typeof loyaltyPrice).toBe("function");
});
"#,
    );

    for (label, tests, related) in [
        ("renamed import", &renamed, true),
        ("namespace member", &namespace, true),
        ("object member call", &member_call, false),
        ("locally shadowed", &shadowed, false),
    ] {
        assert_eq!(tests.len(), 1, "{label}: fixture must parse one test");
        let candidates = related_test_candidates(owner, tests, None, &ReExportIndex::empty(), None);
        assert_eq!(
            !candidates.is_empty(),
            related,
            "{label}: unexpected relation set {:?}",
            candidates
                .iter()
                .map(|candidate| candidate.relation)
                .collect::<Vec<_>>()
        );
    }
}

// ── Silent-gap disclosure: unreadable files, test-file parse errors, ────────
// ── and partial test extraction (typescript_test_extraction_partial) ────────
//
// Spec: a workspace file that cannot be read, a changed test file that cannot
// be parsed, or a recognized test file whose registrations the extractor
// silently drops must produce typed limitations (or at least a real
// `skipped_files` count) instead of vanishing with zero disclosure. Without
// this, owners whose only tests live in those files get a confident false
// `no_static_path` and the advice "add a test that calls the changed owner"
// points at tests that already exist.

fn ts_analysis_options(root: PathBuf) -> AnalysisOptions {
    AnalysisOptions {
        root,
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
    }
}

/// (a) An unreadable CHANGED file must be disclosed with its path and the
/// read failure, and `skipped_files` must report the real count — including
/// unreadable files that are not part of the diff (counted, not disclosed).
#[test]
fn analyze_diff_discloses_unreadable_changed_file_with_real_skipped_count() -> Result<(), String> {
    let root = ts_unique_tempdir("readfail")?;
    ts_write_file(
        &root.join("src/lib.ts"),
        "export function add(a: number, b: number): number {\n  return a + b;\n}\n",
    )?;
    // Invalid UTF-8 CHANGED file: its added lines can never be classified.
    std::fs::write(
        root.join("src/evil.ts"),
        b"export function evil(): number {\n  \xff\xfe\n}\n",
    )
    .map_err(|err| format!("write invalid-utf8 changed file: {err}"))?;
    // Invalid UTF-8 UNCHANGED file: counted in skipped_files, no limitation.
    std::fs::write(
        root.join("src/stale.ts"),
        b"export function stale(): number {\n  \xff\xfe\n}\n",
    )
    .map_err(|err| format!("write invalid-utf8 unchanged file: {err}"))?;

    let adapter = TypeScriptAdapter;
    let options = ts_analysis_options(root.clone());
    let result = adapter.analyze_diff(
        &options,
        &OraclePolicy::default(),
        &[changed("src/evil.ts")],
    )?;
    assert_eq!(
        result.skipped_files, 2,
        "both unreadable files must be counted in skipped_files"
    );
    let read_limits = result
        .limitations
        .iter()
        .filter(|limitation| {
            limitation
                .bounded_detail
                .as_deref()
                .is_some_and(|detail| detail.starts_with("read failed:"))
        })
        .collect::<Vec<_>>();
    assert_eq!(
        read_limits.len(),
        1,
        "only the CHANGED unreadable file is disclosed, got {:?}",
        result.limitations
    );
    let limitation = read_limits[0];
    assert_eq!(limitation.path.as_deref(), Some("src/evil.ts"));
    let detail = limitation.bounded_detail.as_deref().unwrap_or_default();
    assert!(
        detail.contains("read failed:"),
        "detail must name the read failure, got {detail:?}"
    );
    Ok(())
}

/// A CHANGED test file with a parse error must produce a limitation just like
/// a changed production file — its tests would otherwise vanish from
/// `all_tests` and flip owners to false `no_static_path` with no trace.
#[test]
fn analyze_diff_discloses_changed_test_file_parse_error() -> Result<(), String> {
    let root = ts_unique_tempdir("testparse")?;
    ts_write_file(
        &root.join("src/lib.ts"),
        "export function add(a: number, b: number): number {\n  return a + b;\n}\n",
    )?;
    // Unclosed arrow body → parser error.
    ts_write_file(
        &root.join("tests/lib.test.ts"),
        "test(\"adds\", () => {\n  expect(add(1, 2)).toBe(3);\n",
    )?;

    let adapter = TypeScriptAdapter;
    let options = ts_analysis_options(root.clone());
    let result = adapter.analyze_diff(
        &options,
        &OraclePolicy::default(),
        &[changed("tests/lib.test.ts")],
    )?;
    assert!(
        result.limitations.iter().any(|limitation| {
            limitation.path.as_deref() == Some("tests/lib.test.ts")
                && limitation
                    .bounded_detail
                    .as_deref()
                    .is_some_and(|detail| detail.contains("parser error"))
        }),
        "expected a parse-error limitation naming the changed test file, got {:?}",
        result.limitations
    );
    Ok(())
}

/// (b) A recognized test file that parses but registers a test with a
/// template-literal title must emit `typescript_test_extraction_partial`.
#[test]
fn analyze_diff_emits_test_extraction_partial_for_template_literal_title() -> Result<(), String> {
    let root = ts_unique_tempdir("tmpltitle")?;
    ts_write_file(
        &root.join("src/calc.ts"),
        "export function add(a: number, b: number): number {\n  return a + b;\n}\n",
    )?;
    ts_write_file(
        &root.join("tests/calc.test.ts"),
        "import { add } from '../src/calc';\nit(`adds ${1} and ${2}`, () => {\n  expect(add(1, 2)).toBe(3);\n});\n",
    )?;

    let adapter = TypeScriptAdapter;
    let options = ts_analysis_options(root.clone());
    let result = adapter.analyze_diff(
        &options,
        &OraclePolicy::default(),
        &[changed("src/calc.ts")],
    )?;
    assert!(
        result.limitations.iter().any(|limitation| {
            limitation
                .bounded_detail
                .as_deref()
                .is_some_and(|detail| detail.contains("typescript_test_extraction_partial"))
        }),
        "expected a typescript_test_extraction_partial limitation, got {:?}",
        result.limitations
    );
    Ok(())
}

/// #4099 end-to-end: a VALID tagged-template `.each` registration (the table
/// jest itself accepts) parses cleanly, is not extractable by design, and
/// must surface `typescript_test_extraction_partial` through `analyze_diff`
/// — not only at the detector level. (The original #4104 fx13 fixture was
/// malformed JavaScript — a stray `}` before the call, rejected by node
/// itself — so its silence was the parse-error lane, not this disclosure.)
#[test]
fn analyze_diff_discloses_valid_tagged_template_each() -> Result<(), String> {
    let root = ts_unique_tempdir("fx13-repro")?;
    ts_write_file(
        &root.join("src/pricing.ts"),
        "export function tier(amount: number): string {\n  if (amount >= 150) {\n    return 'premium';\n  }\n  return 'standard';\n}\n",
    )?;
    ts_write_file(
        &root.join("tests/tiers.test.ts"),
        "import { expect, test } from 'vitest';\nimport { tier } from '../src/pricing';\n\ntest.each`\n  amount | expected\n  ${100} | ${'standard'}\n  ${150} | ${'premium'}\n`('computes the boundary case', ({ amount, expected }) => {\n  expect(tier(amount)).toBe(expected);\n});\n",
    )?;
    let adapter = TypeScriptAdapter;
    let options = ts_analysis_options(root.clone());
    let result = adapter.analyze_diff(
        &options,
        &OraclePolicy::default(),
        &[changed("src/pricing.ts")],
    )?;
    assert!(
        result.limitations.iter().any(|limitation| {
            limitation
                .bounded_detail
                .as_deref()
                .is_some_and(|detail| detail.contains("typescript_test_extraction_partial"))
        }),
        "a valid tagged-template .each file must disclose the partial test extraction e2e, got {:?}",
        result.limitations
    );
    Ok(())
}

/// (c) Negative control: a normal, fully extracted test file (plain titles,
/// describe nesting, array-form `.each`) must NOT emit the new limitation.
#[test]
fn analyze_diff_no_extraction_partial_for_fully_extracted_test_file() -> Result<(), String> {
    let root = ts_unique_tempdir("full-extract")?;
    ts_write_file(
        &root.join("src/calc.ts"),
        "export function add(a: number, b: number): number {\n  return a + b;\n}\n",
    )?;
    ts_write_file(
        &root.join("tests/calc.test.ts"),
        "import { add } from '../src/calc';\n\
         describe(\"add\", () => {\n\
         \x20 it(\"adds two numbers\", () => {\n\
         \x20   expect(add(1, 2)).toBe(3);\n\
         \x20 });\n\
         \x20 test.each([[1, 2, 3]])(\"row %#\", (row) => {\n\
         \x20   expect(add(row[0], row[1])).toBe(row[2]);\n\
         \x20 });\n\
         });\n",
    )?;

    let adapter = TypeScriptAdapter;
    let options = ts_analysis_options(root.clone());
    let result = adapter.analyze_diff(
        &options,
        &OraclePolicy::default(),
        &[changed("src/calc.ts")],
    )?;
    assert!(
        !result.limitations.iter().any(|limitation| {
            limitation
                .bounded_detail
                .as_deref()
                .is_some_and(|detail| detail.contains("typescript_test_extraction_partial"))
        }),
        "fully extracted test file must NOT emit typescript_test_extraction_partial, got {:?}",
        result.limitations
    );
    Ok(())
}

/// Detector unit shape: tagged-template `.each` in callee position is flagged.
#[test]
fn detect_partial_flags_tagged_template_each() -> Result<(), String> {
    let file = Path::new("tests/table.test.ts");
    let source =
        "test.each`\n a | b\n 1 | 2\n`('row %#', ({ a, b }) => {\n  expect(a).toBe(b);\n});\n";
    let extracted = extract_tests(file, source);
    assert!(
        extracted.is_empty(),
        "tagged-template .each is not extractable by design, got {extracted:?}"
    );
    let gap = detect_partial_test_extraction(file, source, &extracted)
        .ok_or_else(|| "tagged-template .each must be disclosed".to_string())?;
    assert_eq!(gap.shape, "tagged-template .each");
    assert_eq!(gap.sample_line, 1);
    Ok(())
}

/// Detector unit shape: `it(...)` generated inside a loop body is flagged.
#[test]
fn detect_partial_flags_test_registered_in_loop() -> Result<(), String> {
    let file = Path::new("tests/loop.test.ts");
    let source = "for (const n of [1, 2]) {\n  it(\"case \" + n, () => {\n    expect(n).toBe(1);\n  });\n}\n";
    let extracted = extract_tests(file, source);
    assert!(
        extracted.is_empty(),
        "loop-generated tests are not extractable by design, got {extracted:?}"
    );
    let gap = detect_partial_test_extraction(file, source, &extracted)
        .ok_or_else(|| "loop-generated test must be disclosed".to_string())?;
    assert_eq!(gap.shape, "test/it call in loop/callback/nested body");
    assert_eq!(gap.sample_line, 2);
    Ok(())
}

/// Detector unit shape: template-literal `it(`/`test(` titles are flagged.
#[test]
fn detect_partial_flags_template_literal_title() -> Result<(), String> {
    let file = Path::new("tests/tmpl.test.ts");
    let source = "it(`adds ${1}`, () => {\n  expect(1 + 1).toBe(2);\n});\n";
    let extracted = extract_tests(file, source);
    assert!(
        extracted.is_empty(),
        "template-literal titles are not extractable by design, got {extracted:?}"
    );
    let gap = detect_partial_test_extraction(file, source, &extracted)
        .ok_or_else(|| "template-literal title must be disclosed".to_string())?;
    assert_eq!(gap.shape, "template-literal title");
    assert_eq!(gap.sample_line, 1);
    Ok(())
}

/// Detector negative control: a fully extracted file reports no gap.
#[test]
fn detect_partial_none_when_every_test_extracted() {
    let file = Path::new("tests/plain.test.ts");
    let source = "describe(\"suite\", () => {\n  it(\"works\", () => {\n    expect(1).toBe(1);\n  });\n  test.each([[1]])(\"row %#\", (n) => {\n    expect(n).toBe(1);\n  });\n});\n";
    let extracted = extract_tests(file, source);
    assert_eq!(extracted.len(), 2, "both registrations extract");
    assert!(
        detect_partial_test_extraction(file, source, &extracted).is_none(),
        "fully extracted file must not report a partial-extraction gap"
    );
}

// ── #4104-A: owner-extraction gap disclosure ─────────────────────────────────

fn changed_with_lines(path: &str, lines: &[(usize, &str)]) -> ChangedFile {
    ChangedFile {
        path: PathBuf::from(path),
        added_lines: lines
            .iter()
            .map(|(line, text)| crate::analysis::diff::ChangedLine {
                line: *line,
                text: (*text).to_string(),
                new_side_line: *line,
            })
            .collect(),
        removed_lines: Vec::new(),
    }
}

/// Shared runner: analyze a fixture whose changed line sits inside an
/// unsupported owner shape and require the typed
/// `typescript_owner_extraction_partial` limitation naming the shape.
fn assert_owner_extraction_gap_disclosed(
    label: &str,
    src_path: &str,
    src: &str,
    added_line: (usize, &str),
    expected_shape: &str,
) -> Result<(), String> {
    let root = ts_unique_tempdir(label)?;
    ts_write_file(&root.join(src_path), src)?;
    let adapter = TypeScriptAdapter;
    let options = ts_analysis_options(root.clone());
    let result = adapter.analyze_diff(
        &options,
        &OraclePolicy::default(),
        &[changed_with_lines(src_path, &[added_line])],
    )?;
    let disclosed = result.limitations.iter().any(|limitation| {
        limitation.bounded_detail.as_deref().is_some_and(|detail| {
            detail.contains("typescript_owner_extraction_partial")
                && detail.contains(expected_shape)
        })
    });
    assert!(
        disclosed,
        "expected typescript_owner_extraction_partial ({expected_shape}) limitation, got {limitation_result:?}",
        limitation_result = result.limitations
    );
    Ok(())
}

#[test]
fn owner_extraction_gap_discloses_arrow_function_class_field() -> Result<(), String> {
    assert_owner_extraction_gap_disclosed(
        "owner-gap-arrow",
        "src/checkout.ts",
        "export class Checkout {\n  applyDiscount = (amount: number): number => {\n    if (amount >= 100) {\n      return amount * 0.9;\n    }\n    return amount;\n  };\n}\n",
        (3, "    if (amount >= 100) {"),
        "arrow-function class field",
    )
}

#[test]
fn owner_extraction_gap_discloses_private_method() -> Result<(), String> {
    assert_owner_extraction_gap_disclosed(
        "owner-gap-private",
        "src/ledger.ts",
        "export class Ledger {\n  #adjust(amount: number): number {\n    if (amount >= 50) {\n      return amount - 5;\n    }\n    return amount;\n  }\n\n  entry(amount: number): number {\n    return this.#adjust(amount);\n  }\n}\n",
        (3, "    if (amount >= 50) {"),
        "class method with unsupported key",
    )
}

#[test]
fn owner_extraction_gap_discloses_enum_member_change() -> Result<(), String> {
    assert_owner_extraction_gap_disclosed(
        "owner-gap-enum",
        "src/plans.ts",
        "export enum PlanRate {\n  Basic = 8,\n  Premium = 18,\n}\n",
        (2, "  Basic = 8,"),
        "enum declaration",
    )
}

#[test]
fn owner_extraction_gap_discloses_namespace_function() -> Result<(), String> {
    assert_owner_extraction_gap_disclosed(
        "owner-gap-namespace",
        "src/geometry.ts",
        "export namespace Pricing {\n  export function tax(amount: number): number {\n    if (amount >= 200) {\n      return amount * 0.3;\n    }\n    return 0;\n  }\n}\n",
        (4, "      return amount * 0.3;"),
        "module/namespace declaration",
    )
}

#[test]
fn owner_extraction_gap_discloses_accessor_auto_accessor() -> Result<(), String> {
    assert_owner_extraction_gap_disclosed(
        "owner-gap-accessor",
        "src/gauge.ts",
        "export class Gauge {\n  accessor level = 2;\n}\n",
        (2, "  accessor level = 2;"),
        "accessor auto-accessor",
    )
}

#[test]
fn owner_extraction_gap_discloses_static_block() -> Result<(), String> {
    assert_owner_extraction_gap_disclosed(
        "owner-gap-static-block",
        "src/registry.ts",
        "export class Registry {\n  static defaults = new Map<string, number>();\n\n  static {\n    Registry.defaults.set(\"standard\", 8);\n  }\n\n  rate(key: string): number {\n    return Registry.defaults.get(key) ?? 0;\n  }\n}\n",
        (5, "    Registry.defaults.set(\"standard\", 8);"),
        "class static block",
    )
}

#[test]
fn no_owner_extraction_gap_for_supported_owner_shape() -> Result<(), String> {
    let root = ts_unique_tempdir("owner-gap-negative")?;
    ts_write_file(
        &root.join("src/calc.ts"),
        "export function calc(amount: number): number {\n  if (amount >= 6) {\n    return amount * 2;\n  }\n  return amount;\n}\n",
    )?;
    let adapter = TypeScriptAdapter;
    let options = ts_analysis_options(root.clone());
    let result = adapter.analyze_diff(
        &options,
        &OraclePolicy::default(),
        &[changed_with_lines(
            "src/calc.ts",
            &[(2, "  if (amount >= 6) {")],
        )],
    )?;
    assert!(
        !result.limitations.iter().any(|limitation| {
            limitation
                .bounded_detail
                .as_deref()
                .is_some_and(|detail| detail.contains("typescript_owner_extraction_partial"))
        }),
        "supported owner shapes must not emit typescript_owner_extraction_partial, got {:?}",
        result.limitations
    );
    Ok(())
}

// ── #4104-B: relation fixes ───────────────────────────────────────────────────

/// Getter owner: a property READ on a constructed receiver runs the getter
/// body, so `expect(cart.total).toBe(80)` must credit the relation.
#[test]
fn getter_property_read_on_constructed_receiver_credits_relation() -> Result<(), String> {
    let root = ts_unique_tempdir("getter-read")?;
    ts_write_file(
        &root.join("src/cart.ts"),
        "export class Cart {\n  private items: number[] = [30, 40, 30];\n\n  get total(): number {\n    return this.items.reduce((sum, n) => sum + n, 0) * 0.8;\n  }\n}\n",
    )?;
    ts_write_file(
        &root.join("tests/cart.test.ts"),
        "import { describe, it, expect } from 'vitest';\nimport { Cart } from '../src/cart';\n\ndescribe('Cart', () => {\n  it('totals with the member discount', () => {\n    const cart = new Cart();\n    expect(cart.total).toBe(80);\n  });\n});\n",
    )?;
    let adapter = TypeScriptAdapter;
    let options = ts_analysis_options(root.clone());
    let result = adapter.analyze_diff(
        &options,
        &OraclePolicy::default(),
        &[changed_with_lines(
            "src/cart.ts",
            &[(
                6,
                "    return this.items.reduce((sum, n) => sum + n, 0) * 0.8;",
            )],
        )],
    )?;
    let finding = result
        .findings
        .first()
        .ok_or_else(|| "expected a finding for the getter seam".to_string())?;
    assert_eq!(
        finding.ripr.reach.state,
        StageState::Yes,
        "getter property read must credit the receiver relation, evidence: {:?}",
        finding.evidence
    );
    assert!(!matches!(finding.class, ExposureClass::NoStaticPath));
    Ok(())
}

/// Ordinary (non-getter) methods keep the call-only rule: a bare property
/// read must not credit them (negative control for the getter read needle).
#[test]
fn ordinary_method_property_read_does_not_credit_relation() -> Result<(), String> {
    let root = ts_unique_tempdir("method-read-negative")?;
    ts_write_file(
        &root.join("src/counter.ts"),
        "export class Counter {\n  current = 1;\n\n  step(): number {\n    return this.current * 2;\n  }\n}\n",
    )?;
    ts_write_file(
        &root.join("tests/counter.test.ts"),
        "import { describe, it, expect } from 'vitest';\nimport { Counter } from '../src/counter';\n\ndescribe('Counter', () => {\n  it('reads the field but never calls step', () => {\n    const counter = new Counter();\n    expect(counter.current).toBe(1);\n  });\n});\n",
    )?;
    let adapter = TypeScriptAdapter;
    let options = ts_analysis_options(root.clone());
    let result = adapter.analyze_diff(
        &options,
        &OraclePolicy::default(),
        &[changed_with_lines(
            "src/counter.ts",
            &[(5, "    return this.current * 2;")],
        )],
    )?;
    let finding = result
        .findings
        .first()
        .ok_or_else(|| "expected a finding for the method seam".to_string())?;
    assert_eq!(
        finding.ripr.reach.state,
        StageState::No,
        "a property read must NOT credit an ordinary method (no call exists), evidence: {:?}",
        finding.evidence
    );
    Ok(())
}

/// Constructor owner: `new Wallet(100)` executes the changed constructor.
#[test]
fn constructor_owner_is_credited_by_new_expression() -> Result<(), String> {
    let root = ts_unique_tempdir("ctor-new")?;
    ts_write_file(
        &root.join("src/wallet.ts"),
        "export class Wallet {\n  balance: number;\n\n  constructor(start: number) {\n    this.balance = start >= 100 ? start - 10 : start;\n  }\n}\n",
    )?;
    ts_write_file(
        &root.join("tests/wallet.test.ts"),
        "import { describe, it, expect } from 'vitest';\nimport { Wallet } from '../src/wallet';\n\ndescribe('Wallet', () => {\n  it('applies the opening bonus at the boundary', () => {\n    const wallet = new Wallet(100);\n    expect(wallet.balance).toBe(90);\n  });\n});\n",
    )?;
    let adapter = TypeScriptAdapter;
    let options = ts_analysis_options(root.clone());
    let result = adapter.analyze_diff(
        &options,
        &OraclePolicy::default(),
        &[changed_with_lines(
            "src/wallet.ts",
            &[(5, "    this.balance = start >= 100 ? start - 10 : start;")],
        )],
    )?;
    let finding = result
        .findings
        .first()
        .ok_or_else(|| "expected a finding for the constructor seam".to_string())?;
    assert_eq!(
        finding.ripr.reach.state,
        StageState::Yes,
        "new Wallet(100) must credit the constructor owner, evidence: {:?}",
        finding.evidence
    );
    assert!(!matches!(finding.class, ExposureClass::NoStaticPath));
    Ok(())
}

#[test]
fn new_expression_call_needle_boundary_guards() {
    // Positive.
    assert!(contains_new_expression_call(
        "const wallet = new Wallet(100);",
        "Wallet"
    ));
    // Not the tail of a longer identifier.
    assert!(!contains_new_expression_call(
        "const w = renew Wallet(100);",
        "Wallet"
    ));
    // A different constructor: WalletFactory is not Wallet.
    assert!(!contains_new_expression_call(
        "const w = new WalletFactory(100);",
        "Wallet"
    ));
    // Member chain selects a different constructor.
    assert!(!contains_new_expression_call(
        "const w = new Wallet.Provider(100);",
        "Wallet"
    ));
    // Unsafe identifier never matches.
    assert!(!contains_new_expression_call(
        "const w = new has space(100);",
        "has space"
    ));
}

#[test]
fn member_read_needle_boundary_guards() {
    // Positive read.
    assert!(contains_member_read_name(
        "expect(cart.total).toBe(80);",
        "cart",
        "total"
    ));
    // A call is not a read (the call needle covers it).
    assert!(!contains_member_read_name(
        "expect(cart.total(80)).toBe(80);",
        "cart",
        "total"
    ));
    // Longer identifier does not match.
    assert!(!contains_member_read_name(
        "expect(cart.totalAmount).toBe(80);",
        "cart",
        "total"
    ));
    // A different property does not match.
    assert!(!contains_member_read_name(
        "expect(cart.itemCount).toBe(3);",
        "cart",
        "total"
    ));
}

/// Methods of a default-exported class are not themselves the module's
/// default export (review #4138): they carry `class_default_export` for
/// constructor matching while `exported_as_default` stays false. A named
/// class carries neither.
#[test]
fn default_exported_class_methods_carry_class_marker_not_default_flag() -> Result<(), String> {
    let owners = extract_owners(
        Path::new("src/cart.ts"),
        "export default class Cart {\n  balance: number;\n\n  constructor(start: number) {\n    this.balance = start;\n  }\n\n  get total(): number {\n    return this.balance;\n  }\n}\n",
    );
    let constructor = owners
        .iter()
        .find(|owner| owner.method_kind == TypeScriptMethodKind::Constructor)
        .ok_or("constructor owner extracted")?;
    assert!(
        !constructor.exported_as_default,
        "a method is not the module's default export"
    );
    assert!(
        constructor.class_default_export,
        "the containing class IS the module's default export"
    );
    let getter = owners
        .iter()
        .find(|owner| owner.method_kind == TypeScriptMethodKind::Getter)
        .ok_or("getter owner extracted")?;
    assert!(!getter.exported_as_default);
    assert!(getter.class_default_export);

    let named = extract_owners(
        Path::new("src/plain.ts"),
        "export class Plain {\n  step(): number {\n    return 1;\n  }\n}\n",
    );
    let step = named
        .iter()
        .find(|owner| owner.name == "step")
        .ok_or("method owner extracted")?;
    assert!(!step.exported_as_default);
    assert!(!step.class_default_export);
    Ok(())
}

/// Default-import alias: `import CartAlias from './cart'` + `new
/// CartAlias(100)` runs the constructor of `export default class Cart` even
/// though the class name never appears in the test body (review #4138).
#[test]
fn default_import_alias_constructs_default_exported_class() -> Result<(), String> {
    let root = ts_unique_tempdir("ctor-alias")?;
    ts_write_file(
        &root.join("src/cart.ts"),
        "export default class Cart {\n  balance: number;\n\n  constructor(start: number) {\n    this.balance = start >= 100 ? start - 10 : start;\n  }\n}\n",
    )?;
    ts_write_file(
        &root.join("tests/cart.test.ts"),
        "import { describe, it, expect } from 'vitest';\nimport CartAlias from '../src/cart';\n\ndescribe('Cart', () => {\n  it('applies the opening bonus at the boundary', () => {\n    const cart = new CartAlias(100);\n    expect(cart.balance).toBe(90);\n  });\n});\n",
    )?;
    let adapter = TypeScriptAdapter;
    let options = ts_analysis_options(root.clone());
    let result = adapter.analyze_diff(
        &options,
        &OraclePolicy::default(),
        &[changed_with_lines(
            "src/cart.ts",
            &[(5, "    this.balance = start >= 100 ? start - 10 : start;")],
        )],
    )?;
    let finding = result
        .findings
        .first()
        .ok_or_else(|| "expected a finding for the aliased constructor seam".to_string())?;
    assert_eq!(
        finding.ripr.reach.state,
        StageState::Yes,
        "new CartAlias(100) must credit the default-exported class constructor, evidence: {:?}",
        finding.evidence
    );
    assert!(!matches!(finding.class, ExposureClass::NoStaticPath));
    Ok(())
}

/// Shadow guard: a test-body `function CartAlias` is a genuine constructor
/// of a different binding, so `new CartAlias(100)` inside that body must NOT
/// credit the default-exported class constructor through the alias
/// (review #4138). The declaration guard is the same test-body discipline
/// the other import arms apply.
#[test]
fn locally_redeclared_alias_does_not_credit_default_exported_constructor() -> Result<(), String> {
    let root = ts_unique_tempdir("ctor-alias-shadow")?;
    ts_write_file(
        &root.join("src/cart.ts"),
        "export default class Cart {\n  balance: number;\n\n  constructor(start: number) {\n    this.balance = start >= 100 ? start - 10 : start;\n  }\n}\n",
    )?;
    ts_write_file(
        &root.join("tests/cart.test.ts"),
        "import { describe, it, expect } from 'vitest';\nimport CartAlias from '../src/cart';\n\ndescribe('local shadow', () => {\n  it('constructs the local binding', () => {\n    function CartAlias(start: number) {\n      return { balance: start };\n    }\n    const cart = new CartAlias(100);\n    expect(cart.balance).toBe(100);\n  });\n});\n",
    )?;
    let adapter = TypeScriptAdapter;
    let options = ts_analysis_options(root.clone());
    let result = adapter.analyze_diff(
        &options,
        &OraclePolicy::default(),
        &[changed_with_lines(
            "src/cart.ts",
            &[(5, "    this.balance = start >= 100 ? start - 10 : start;")],
        )],
    )?;
    let finding = result
        .findings
        .first()
        .ok_or_else(|| "expected a finding for the shadowed constructor seam".to_string())?;
    assert_eq!(
        finding.ripr.reach.state,
        StageState::No,
        "a locally re-declared alias constructs the shadow, not the owner constructor, evidence: {:?}",
        finding.evidence
    );
    Ok(())
}

/// Owner-file rename export: `export { computeTotals as totals }` must let
/// `import { totals } from './totals'` credit the owner.
#[test]
fn owner_file_rename_export_credits_the_owner() -> Result<(), String> {
    let root = ts_unique_tempdir("rename-export")?;
    ts_write_file(
        &root.join("src/totals.ts"),
        "function computeTotals(amount: number): number {\n  if (amount >= 100) {\n    return amount - 10;\n  }\n  return amount;\n}\n\nexport { computeTotals as totals };\n",
    )?;
    ts_write_file(
        &root.join("tests/billing.test.ts"),
        "import { describe, it, expect } from 'vitest';\nimport { totals } from '../src/totals';\n\ndescribe('invoice totals', () => {\n  it('deducts at the threshold', () => {\n    expect(totals(100)).toBe(90);\n  });\n});\n",
    )?;
    let adapter = TypeScriptAdapter;
    let options = ts_analysis_options(root.clone());
    let result = adapter.analyze_diff(
        &options,
        &OraclePolicy::default(),
        &[changed_with_lines(
            "src/totals.ts",
            &[(2, "  if (amount >= 100) {")],
        )],
    )?;
    let finding = result
        .findings
        .first()
        .ok_or_else(|| "expected a finding for the rename-export seam".to_string())?;
    assert_eq!(
        finding.ripr.reach.state,
        StageState::Yes,
        "the owner-file rename export must credit the import, evidence: {:?}",
        finding.evidence
    );
    assert!(!matches!(finding.class, ExposureClass::NoStaticPath));
    Ok(())
}

/// Default import bound to a local name: the owner IS the module's default
/// export, so `import ride from './greeter'` + `ride(40)` credits it.
#[test]
fn default_import_with_local_name_credits_default_export_owner() -> Result<(), String> {
    let root = ts_unique_tempdir("default-import")?;
    ts_write_file(
        &root.join("src/greeter.ts"),
        "export default function fare(base: number): number {\n  if (base >= 40) {\n    return base + 2;\n  }\n  return base;\n}\n",
    )?;
    ts_write_file(
        &root.join("tests/ride.test.ts"),
        "import { describe, it, expect } from 'vitest';\nimport ride from '../src/greeter';\n\ndescribe('boundary surcharge', () => {\n  it('adds the fee at the threshold', () => {\n    expect(ride(40)).toBe(42);\n  });\n});\n",
    )?;
    let adapter = TypeScriptAdapter;
    let options = ts_analysis_options(root.clone());
    let result = adapter.analyze_diff(
        &options,
        &OraclePolicy::default(),
        &[changed_with_lines(
            "src/greeter.ts",
            &[(2, "  if (base >= 40) {")],
        )],
    )?;
    let finding = result
        .findings
        .first()
        .ok_or_else(|| "expected a finding for the default-import seam".to_string())?;
    assert_eq!(
        finding.ripr.reach.state,
        StageState::Yes,
        "the default import of a default-export owner must credit, evidence: {:?}",
        finding.evidence
    );
    Ok(())
}

/// Negative control: a default import does NOT credit a named (non-default)
/// export just because the specifier resolves to the owner file.
#[test]
fn default_import_does_not_credit_named_non_default_owner() -> Result<(), String> {
    let root = ts_unique_tempdir("default-import-negative")?;
    ts_write_file(
        &root.join("src/greeter.ts"),
        "export function fare(base: number): number {\n  if (base >= 40) {\n    return base + 2;\n  }\n  return base;\n}\n",
    )?;
    ts_write_file(
        &root.join("tests/ride.test.ts"),
        "import { describe, it, expect } from 'vitest';\nimport ride from '../src/greeter';\n\ndescribe('boundary surcharge', () => {\n  it('adds the fee at the threshold', () => {\n    expect(ride(40)).toBe(42);\n  });\n});\n",
    )?;
    let adapter = TypeScriptAdapter;
    let options = ts_analysis_options(root.clone());
    let result = adapter.analyze_diff(
        &options,
        &OraclePolicy::default(),
        &[changed_with_lines(
            "src/greeter.ts",
            &[(2, "  if (base >= 40) {")],
        )],
    )?;
    let finding = result
        .findings
        .first()
        .ok_or_else(|| "expected a finding for the named-export seam".to_string())?;
    assert_eq!(
        finding.ripr.reach.state,
        StageState::No,
        "a default import must not credit a named non-default owner, evidence: {:?}",
        finding.evidence
    );
    Ok(())
}

// ── #4104-C: ghost relative import disclosure ─────────────────────────────────

#[test]
fn ghost_relative_import_is_disclosed_not_silent() -> Result<(), String> {
    let root = ts_unique_tempdir("ghost-import")?;
    ts_write_file(
        &root.join("src/calc.ts"),
        "export function calc(amount: number): number {\n  if (amount >= 6) {\n    return amount * 2;\n  }\n  return amount;\n}\n",
    )?;
    ts_write_file(
        &root.join("tests/calc.test.ts"),
        "import { describe, it, expect } from 'vitest';\nimport { calc } from './no-such-module';\n\nit('doubles', () => {\n  expect(calc(6)).toBe(12);\n});\n",
    )?;
    let adapter = TypeScriptAdapter;
    let options = ts_analysis_options(root.clone());
    let result = adapter.analyze_diff(
        &options,
        &OraclePolicy::default(),
        &[changed_with_lines(
            "src/calc.ts",
            &[(2, "  if (amount >= 6) {")],
        )],
    )?;
    let finding = result
        .findings
        .first()
        .ok_or_else(|| "expected a finding for the ghost-import seam".to_string())?;
    assert!(
        finding
            .evidence
            .iter()
            .any(|line| line.contains("typescript_relative_import_unresolved")),
        "the ghost relative import must be disclosed, evidence: {:?}",
        finding.evidence
    );
    Ok(())
}

#[test]
fn resolving_relative_import_emits_no_ghost_disclosure() -> Result<(), String> {
    let root = ts_unique_tempdir("valid-import")?;
    ts_write_file(
        &root.join("src/calc.ts"),
        "export function calc(amount: number): number {\n  if (amount >= 6) {\n    return amount * 2;\n  }\n  return amount;\n}\n",
    )?;
    ts_write_file(
        &root.join("tests/calc.test.ts"),
        "import { describe, it, expect } from 'vitest';\nimport { calc } from '../src/calc';\n\nit('doubles', () => {\n  expect(calc(6)).toBe(12);\n});\n",
    )?;
    let adapter = TypeScriptAdapter;
    let options = ts_analysis_options(root.clone());
    let result = adapter.analyze_diff(
        &options,
        &OraclePolicy::default(),
        &[changed_with_lines(
            "src/calc.ts",
            &[(2, "  if (amount >= 6) {")],
        )],
    )?;
    let finding = result
        .findings
        .first()
        .ok_or_else(|| "expected a finding for the valid-import seam".to_string())?;
    assert!(
        !finding
            .evidence
            .iter()
            .any(|line| line.contains("typescript_relative_import_unresolved")),
        "a resolving relative import must NOT be disclosed as ghost, evidence: {:?}",
        finding.evidence
    );
    Ok(())
}

// ── #4106-B: tsconfig unavailability causes ───────────────────────────────────

/// ESM-style `../src/calc.js` specifiers resolve to the `calc.ts` source
/// (TypeScript's JS-extension rewrite; the relation layer already strips the
/// `.js` spelling). The ghost disclosure must agree: probing only `calc.js`
/// previously claimed the specifier "resolves to no workspace file" — a
/// wrong cause for an import that credits (review #4138).
#[test]
fn js_extension_relative_import_resolves_to_ts_source() -> Result<(), String> {
    let root = ts_unique_tempdir("js-ext-import")?;
    ts_write_file(
        &root.join("src/calc.ts"),
        "export function calc(amount: number): number {\n  if (amount >= 6) {\n    return amount * 2;\n  }\n  return amount;\n}\n",
    )?;
    ts_write_file(
        &root.join("tests/calc.test.ts"),
        "import { describe, it, expect } from 'vitest';\nimport { calc } from '../src/calc.js';\n\nit('doubles', () => {\n  expect(calc(6)).toBe(12);\n});\n",
    )?;
    let adapter = TypeScriptAdapter;
    let options = ts_analysis_options(root.clone());
    let result = adapter.analyze_diff(
        &options,
        &OraclePolicy::default(),
        &[changed_with_lines(
            "src/calc.ts",
            &[(2, "  if (amount >= 6) {")],
        )],
    )?;
    let finding = result
        .findings
        .first()
        .ok_or_else(|| "expected a finding for the .js-import seam".to_string())?;
    assert_eq!(
        finding.ripr.reach.state,
        StageState::Yes,
        "a .js specifier resolving to the .ts source must credit, evidence: {:?}",
        finding.evidence
    );
    assert!(
        !finding
            .evidence
            .iter()
            .any(|line| line.contains("typescript_relative_import_unresolved")),
        "a resolving .js specifier must NOT be disclosed as ghost, evidence: {:?}",
        finding.evidence
    );
    Ok(())
}

/// Dotted module names are NOT JS-extension spellings: `./bar.v2` resolves
/// to `bar.v2.ts` by plain extension-append, never by replacing the suffix
/// (`bar.ts` would be an unrelated file). Positive: the dotted module
/// resolves. Negative control: a specifier for a module that does not exist
/// still discloses.
#[test]
fn dotted_module_name_resolves_by_append_not_replacement() -> Result<(), String> {
    let root = ts_unique_tempdir("dotted-module")?;
    ts_write_file(
        &root.join("src/bar.v2.ts"),
        "export function bar(amount: number): number {\n  if (amount >= 6) {\n    return amount * 2;\n  }\n  return amount;\n}\n",
    )?;
    ts_write_file(
        &root.join("tests/dotted.test.ts"),
        "import { describe, it, expect } from 'vitest';\nimport { bar } from '../src/bar.v2';\n\nit('doubles', () => {\n  expect(bar(6)).toBe(12);\n});\n",
    )?;
    let adapter = TypeScriptAdapter;
    let options = ts_analysis_options(root.clone());
    let result = adapter.analyze_diff(
        &options,
        &OraclePolicy::default(),
        &[changed_with_lines(
            "src/bar.v2.ts",
            &[(2, "  if (amount >= 6) {")],
        )],
    )?;
    let finding = result
        .findings
        .first()
        .ok_or_else(|| "expected a finding for the dotted-module seam".to_string())?;
    assert_eq!(
        finding.ripr.reach.state,
        StageState::Yes,
        "the dotted module resolves by extension-append and must credit, evidence: {:?}",
        finding.evidence
    );
    assert!(
        !finding
            .evidence
            .iter()
            .any(|line| line.contains("typescript_relative_import_unresolved")),
        "the resolving dotted import must NOT be disclosed as ghost, evidence: {:?}",
        finding.evidence
    );
    // Negative control: a specifier with NO matching module still discloses —
    // the rewrite must not have widened resolution into over-matching.
    let ghost_root = ts_unique_tempdir("dotted-module-ghost")?;
    ts_write_file(
        &ghost_root.join("src/bar.v2.ts"),
        "export function bar(amount: number): number {\n  if (amount >= 6) {\n    return amount * 2;\n  }\n  return amount;\n}\n",
    )?;
    ts_write_file(
        &ghost_root.join("tests/dotted.test.ts"),
        "import { describe, it, expect } from 'vitest';\nimport { bar } from '../src/no-such';\n\nit('doubles', () => {\n  expect(bar(6)).toBe(12);\n});\n",
    )?;
    let ghost_result = adapter.analyze_diff(
        &ts_analysis_options(ghost_root.clone()),
        &OraclePolicy::default(),
        &[changed_with_lines(
            "src/bar.v2.ts",
            &[(2, "  if (amount >= 6) {")],
        )],
    )?;
    let ghost_finding = ghost_result
        .findings
        .first()
        .ok_or_else(|| "expected a finding for the ghost seam".to_string())?;
    assert!(
        ghost_finding
            .evidence
            .iter()
            .any(|line| line.contains("typescript_relative_import_unresolved")),
        "an unresolvable specifier must still be disclosed as ghost, evidence: {:?}",
        ghost_finding.evidence
    );
    Ok(())
}

#[test]
fn alias_load_gap_types_distinguish_flag_on_failures() -> Result<(), String> {
    let root = ts_unique_tempdir("alias-gap-types")?;

    // No config at all.
    let (map, err, gap) = load_alias_map_with_read_error(&root);
    assert!(map.is_none());
    assert!(err.is_none());
    assert_eq!(gap, Some(TsAliasMapLoadGap::ConfigMissing));

    // Invalid strict JSON (no comments) — parse failure without JSONC hint.
    ts_write_file(
        &root.join("tsconfig.json"),
        "{ \"compilerOptions\": { INVALID }\n",
    )?;
    let (map, err, gap) = load_alias_map_with_read_error(&root);
    assert!(map.is_none() && err.is_none());
    match gap {
        Some(TsAliasMapLoadGap::ConfigUnparseable {
            jsonc_comments: false,
            ..
        }) => {}
        other => {
            return Err(format!("expected plain ConfigUnparseable, got {other:?}"));
        }
    }

    // JSONC: comments make the strict parser fail and the gap names them.
    ts_write_file(
        &root.join("tsconfig.json"),
        "{\n  // compiler options\n  \"compilerOptions\": { \"baseUrl\": \".\", \"paths\": { \"@/*\": [\"./src/*\"] } }\n}\n",
    )?;
    let (map, err, gap) = load_alias_map_with_read_error(&root);
    assert!(map.is_none() && err.is_none());
    match gap {
        Some(TsAliasMapLoadGap::ConfigUnparseable {
            jsonc_comments: true,
            ..
        }) => {}
        other => {
            return Err(format!("expected JSONC ConfigUnparseable, got {other:?}"));
        }
    }

    // extends is a distinct typed cause.
    ts_write_file(
        &root.join("tsconfig.json"),
        "{\n  \"extends\": \"./base.json\",\n  \"compilerOptions\": { \"baseUrl\": \".\", \"paths\": { \"@/*\": [\"./src/*\"] } }\n}\n",
    )?;
    let (map, err, gap) = load_alias_map_with_read_error(&root);
    assert!(map.is_none() && err.is_none());
    assert_eq!(
        gap,
        Some(TsAliasMapLoadGap::ExtendsUnsupported {
            config: "tsconfig.json"
        })
    );

    // Parsed config but no baseUrl is incomplete, not unparseable.
    ts_write_file(
        &root.join("tsconfig.json"),
        "{ \"compilerOptions\": { \"paths\": { \"@/*\": [\"./src/*\"] } } }\n",
    )?;
    let (map, err, gap) = load_alias_map_with_read_error(&root);
    assert!(map.is_none() && err.is_none());
    assert_eq!(
        gap,
        Some(TsAliasMapLoadGap::IncompleteConfig {
            config: "tsconfig.json"
        })
    );

    // A valid config yields the map and no gap.
    ts_write_file(
        &root.join("tsconfig.json"),
        "{ \"compilerOptions\": { \"baseUrl\": \".\", \"paths\": { \"@/*\": [\"./src/*\"] } } }\n",
    )?;
    let (map, err, gap) = load_alias_map_with_read_error(&root);
    assert!(map.is_some() && err.is_none() && gap.is_none());
    Ok(())
}

/// JavaScript-only projects load `jsconfig.json` when `tsconfig.json` is
/// absent. The typed gap and its recovery hints must name the file that was
/// actually loaded, not tell the user to fix a `tsconfig.json` that does not
/// exist (review #4138).
#[test]
fn alias_load_gap_names_the_jsconfig_fallback_file() -> Result<(), String> {
    let root = ts_unique_tempdir("alias-gap-jsconfig")?;
    ts_write_file(
        &root.join("jsconfig.json"),
        "{\n  \"extends\": \"./base.json\",\n  \"compilerOptions\": { \"baseUrl\": \".\", \"paths\": { \"@/*\": [\"./src/*\"] } }\n}\n",
    )?;
    let (map, err, gap) = load_alias_map_with_read_error(&root);
    assert!(map.is_none() && err.is_none());
    let gap = gap.ok_or("expected a typed load gap for the jsconfig fallback")?;
    assert_eq!(
        gap,
        TsAliasMapLoadGap::ExtendsUnsupported {
            config: "jsconfig.json"
        },
        "the gap must record the jsconfig.json fallback as the loaded config"
    );
    let (cause, hint) = gap.parts();
    assert!(
        cause.contains("jsconfig.json") && !cause.contains("the tsconfig at"),
        "cause must name jsconfig.json, got: {cause}"
    );
    assert!(
        hint.contains("jsconfig.json") && !hint.contains("tsconfig.json"),
        "hint must point at jsconfig.json, got: {hint}"
    );
    Ok(())
}

/// End-to-end #4106-B: with the flag ON and an unparseable tsconfig, the
/// alias advice must name the parse failure and NOT tell the user to enable
/// the already-enabled flag.
#[test]
fn alias_advice_names_the_real_cause_when_flag_is_on() -> Result<(), String> {
    let root = ts_unique_tempdir("alias-advice-flag-on")?;
    ts_write_file(
        &root.join("lib/util.ts"),
        "export function util(amount: number): number {\n  if (amount >= 4) {\n    return amount * 2;\n  }\n  return amount;\n}\n",
    )?;
    ts_write_file(
        &root.join("tests/util.test.ts"),
        "import { describe, it, expect } from 'vitest';\nimport { util } from '@/util';\n\nit('doubles', () => {\n  expect(util(4)).toBe(8);\n});\n",
    )?;
    ts_write_file(
        &root.join("tsconfig.json"),
        "{ \"compilerOptions\": { \"baseUrl\": \".\", INVALID }\n",
    )?;
    let mut options = ts_analysis_options(root.clone());
    options.resolve_tsconfig_paths = true;
    let adapter = TypeScriptAdapter;
    let result = adapter.analyze_diff(
        &options,
        &OraclePolicy::default(),
        &[changed_with_lines(
            "lib/util.ts",
            &[(2, "  if (amount >= 4) {")],
        )],
    )?;
    let finding = result
        .findings
        .first()
        .ok_or_else(|| "expected a finding for the alias seam".to_string())?;
    let why = finding
        .evidence
        .iter()
        .find(|line| line.contains("typescript_limitation_why: typescript_path_alias_unresolved"))
        .ok_or_else(|| {
            format!(
                "expected the alias limitation to fire, evidence: {:?}",
                finding.evidence
            )
        })?;
    assert!(
        why.contains("could not be parsed as strict JSON"),
        "the advice must name the parse failure, got: {why}"
    );
    assert!(
        !why.contains("resolve_tsconfig_paths = true"),
        "the advice must NOT tell the user to enable an already-enabled flag, got: {why}"
    );
    Ok(())
}

/// Flag-OFF keeps the honest "enable the flag" advice (negative control).
#[test]
fn alias_advice_names_the_flag_only_when_flag_is_off() -> Result<(), String> {
    let root = ts_unique_tempdir("alias-advice-flag-off")?;
    ts_write_file(
        &root.join("lib/util.ts"),
        "export function util(amount: number): number {\n  if (amount >= 4) {\n    return amount * 2;\n  }\n  return amount;\n}\n",
    )?;
    ts_write_file(
        &root.join("tests/util.test.ts"),
        "import { describe, it, expect } from 'vitest';\nimport { util } from '@/util';\n\nit('doubles', () => {\n  expect(util(4)).toBe(8);\n});\n",
    )?;
    let options = ts_analysis_options(root.clone());
    let adapter = TypeScriptAdapter;
    let result = adapter.analyze_diff(
        &options,
        &OraclePolicy::default(),
        &[changed_with_lines(
            "lib/util.ts",
            &[(2, "  if (amount >= 4) {")],
        )],
    )?;
    let finding = result
        .findings
        .first()
        .ok_or_else(|| "expected a finding for the alias seam".to_string())?;
    let why = finding
        .evidence
        .iter()
        .find(|line| line.contains("typescript_limitation_why: typescript_path_alias_unresolved"))
        .ok_or_else(|| {
            format!(
                "expected the alias limitation to fire, evidence: {:?}",
                finding.evidence
            )
        })?;
    assert!(
        why.contains("alias resolution is not enabled")
            && why.contains("resolve_tsconfig_paths = true"),
        "flag-off advice must name the flag, got: {why}"
    );
    Ok(())
}
// ── #4103 under-credit half: genuine owner references the gate missed ──────
//
// Red witnesses: the ts-relation probe fixtures (case04b, case07, case08,
// case11/11b, case12a/12b/12d) against main `f72022049`. Every shape below
// reported `no_static_path` with the false "No test references `X(`" message
// (case12b capped at a bare-reference heuristic) before these fixes. Each
// positive test has a wrong-identity negative control.

#[test]
fn undercredit_4103_optional_chain_namespace_member_call_is_credited() {
    let owner = test_owner("loyaltyPrice", "src/pricing.ts");
    let tests = extract_tests(
        Path::new("tests/pricing.test.ts"),
        r#"import * as pricing from "../src/pricing";
describe("tiers", () => {
  it("veteran", () => {
    expect(pricing?.loyaltyPrice(1000, 5)).toBe(950);
  });
});
"#,
    );
    let candidates = related_test_candidates(&owner, &tests, None, &ReExportIndex::empty(), None);
    assert_eq!(
        candidates.len(),
        1,
        "optional-chain member call must credit"
    );
    assert_eq!(
        candidates[0].relation,
        TypeScriptRelationKind::ImportedOwnerCall
    );
}

#[test]
fn undercredit_4103_optional_chain_member_reference_counts_as_reference() {
    let owner = test_owner("loyaltyPrice", "src/pricing.ts");
    let tests = extract_tests(
        Path::new("tests/pricing.test.ts"),
        r#"import * as pricing from "../src/pricing";
describe("tiers", () => {
  it("veteran", () => {
    expect(pricing?.loyaltyPrice).toBeDefined();
  });
});
"#,
    );
    let candidates = related_test_candidates(&owner, &tests, None, &ReExportIndex::empty(), None);
    assert_eq!(
        candidates.len(),
        1,
        "optional-chain member reference must count as a reference"
    );
    assert!(candidates[0].relation.is_uncertain());
}

#[test]
fn undercredit_4103_direct_optional_call_is_credited_as_call() {
    let owner = test_owner("loyaltyPrice", "src/pricing.ts");
    let tests = extract_tests(
        Path::new("tests/pricing.test.ts"),
        r#"import { loyaltyPrice } from "../src/pricing";
describe("tiers", () => {
  it("veteran", () => {
    expect(loyaltyPrice?.(1000, 5)).toBe(950);
  });
});
"#,
    );
    let candidates = related_test_candidates(&owner, &tests, None, &ReExportIndex::empty(), None);
    assert_eq!(candidates.len(), 1, "optional direct call must credit");
    assert_eq!(
        candidates[0].relation,
        TypeScriptRelationKind::DirectOwnerCall
    );
}

#[test]
fn undercredit_4103_optional_chain_unrelated_receiver_is_not_credited() {
    let owner = test_owner("loyaltyPrice", "src/pricing.ts");
    let tests = extract_tests(
        Path::new("tests/other.test.ts"),
        r#"test("same name on an unrelated local", () => {
    const odd = { loyaltyPrice: () => 1 };
    expect(odd?.loyaltyPrice(2)).toBe(1);
});
"#,
    );
    let related = find_related_tests(&owner, &tests, None, &ReExportIndex::empty(), None);
    assert!(
        related.is_empty(),
        "a same-name member on an unrelated local must not relate"
    );
}

#[test]
fn undercredit_4103_dynamic_import_member_call_is_credited() {
    let owner = test_owner("loyaltyPrice", "src/pricing.ts");
    let tests = extract_tests(
        Path::new("tests/pricing.test.ts"),
        r#"import { describe, it, expect } from "vitest";
describe("pricing", () => {
  it("loyal customers", async () => {
    const m = await import("../src/pricing");
    expect(m.loyaltyPrice(1000, 5)).toBe(950);
  });
});
"#,
    );
    let candidates = related_test_candidates(&owner, &tests, None, &ReExportIndex::empty(), None);
    assert_eq!(
        candidates.len(),
        1,
        "dynamic import member call must credit"
    );
    assert_eq!(
        candidates[0].relation,
        TypeScriptRelationKind::ImportedOwnerCall
    );
}

#[test]
fn undercredit_4103_dynamic_import_spaced_specifier_is_credited() {
    let owner = test_owner("loyaltyPrice", "src/pricing.ts");
    let tests = extract_tests(
        Path::new("tests/pricing.test.ts"),
        r#"it("loyal customers", async () => {
    const m = await import( "../src/pricing" );
    expect(m.loyaltyPrice(1000, 5)).toBe(950);
});
"#,
    );
    let candidates = related_test_candidates(&owner, &tests, None, &ReExportIndex::empty(), None);
    assert_eq!(
        candidates.len(),
        1,
        "spaced dynamic import specifier must credit"
    );
    assert_eq!(
        candidates[0].relation,
        TypeScriptRelationKind::ImportedOwnerCall
    );
}

#[test]
fn undercredit_4103_dynamic_import_other_module_is_not_credited() {
    let owner = test_owner("loyaltyPrice", "src/pricing.ts");
    let tests = extract_tests(
        Path::new("tests/other.test.ts"),
        r#"it("imports a different module", async () => {
    const o = await import("../src/other");
    expect(o.loyaltyPrice(1000, 5)).toBe(950);
});
"#,
    );
    let related = find_related_tests(&owner, &tests, None, &ReExportIndex::empty(), None);
    assert!(
        related.is_empty(),
        "a dynamic import of another module must not relate"
    );
}

#[test]
fn undercredit_4103_dynamic_import_without_await_is_not_credited() {
    let owner = test_owner("loyaltyPrice", "src/pricing.ts");
    let tests = extract_tests(
        Path::new("tests/pricing.test.ts"),
        r#"it("holds a promise", async () => {
    const m = import("../src/pricing");
    expect(m.loyaltyPrice(1000, 5)).toBe(950);
});
"#,
    );
    let related = find_related_tests(&owner, &tests, None, &ReExportIndex::empty(), None);
    assert!(
        related.is_empty(),
        "an unawaited dynamic import binds a promise, so the member call cannot reach the owner"
    );
}

#[test]
fn undercredit_4103_named_default_import_under_other_local_is_credited() {
    let mut owner = test_owner("formatPrice", "src/defaulted.ts");
    owner.exported_as_default = true;
    let tests = extract_tests(
        Path::new("tests/money.test.ts"),
        r#"import fp from "../src/defaulted";
describe("money", () => {
  it("formats", () => {
    expect(fp(199)).toBe("1.99");
  });
});
"#,
    );
    let candidates = related_test_candidates(&owner, &tests, None, &ReExportIndex::empty(), None);
    assert_eq!(
        candidates.len(),
        1,
        "a default import of a named default export must credit"
    );
    assert_eq!(
        candidates[0].relation,
        TypeScriptRelationKind::ImportedOwnerCall
    );
}

#[test]
fn undercredit_4103_default_import_when_owner_is_not_the_default_export_is_not_credited() {
    let owner = test_owner("formatPrice", "src/defaulted.ts");
    assert!(!owner.exported_as_default);
    let tests = extract_tests(
        Path::new("tests/money.test.ts"),
        r#"import fp from "../src/defaulted";
describe("money", () => {
  it("formats", () => {
    expect(fp(199)).toBe("1.99");
  });
});
"#,
    );
    let related = find_related_tests(&owner, &tests, None, &ReExportIndex::empty(), None);
    assert!(
        related.is_empty(),
        "a default import must not credit an owner that is not the module's default export"
    );
}

#[test]
fn undercredit_4103_star_barrel_import_is_credited() {
    let owner = test_owner("slugify", "src/utils.ts");
    let index = ReExportIndex::from_parts(
        Vec::new(),
        vec![("src/index".to_string(), "src/utils".to_string())],
    );
    let tests = extract_tests(
        Path::new("tests/slug.test.ts"),
        r#"import { slugify } from "../src/index";
describe("slugify", () => {
  it("slugs", () => {
    expect(slugify("Hello World")).toBe("hello world");
  });
});
"#,
    );
    let candidates = related_test_candidates(&owner, &tests, None, &index, None);
    assert_eq!(candidates.len(), 1, "star barrel import must credit");
    assert_eq!(
        candidates[0].relation,
        TypeScriptRelationKind::ReExportChainFollowed
    );
}

#[test]
fn undercredit_4103_star_barrel_other_name_is_not_credited() {
    let owner = test_owner("slugify", "src/utils.ts");
    let index = ReExportIndex::from_parts(
        Vec::new(),
        vec![("src/index".to_string(), "src/utils".to_string())],
    );
    let tests = extract_tests(
        Path::new("tests/slug.test.ts"),
        r#"import { notslugify } from "../src/index";
describe("slug", () => {
  it("slugs", () => {
    expect(notslugify("Hello World")).toBe("hello world");
  });
});
"#,
    );
    let related = find_related_tests(&owner, &tests, None, &index, None);
    assert!(
        related.is_empty(),
        "a star hop invents no aliases: another name through the barrel must not credit"
    );
}

#[test]
fn undercredit_4103_default_as_reexport_is_credited() {
    let mut owner = test_owner("formatPrice", "src/defaulted.ts");
    owner.exported_as_default = true;
    let index = ReExportIndex::from_parts(
        vec![(
            ("src/index".to_string(), "formatPrice".to_string()),
            ("default".to_string(), "src/defaulted".to_string()),
        )],
        Vec::new(),
    );
    let tests = extract_tests(
        Path::new("tests/money.test.ts"),
        r#"import { formatPrice } from "../src/index";
describe("formatPrice", () => {
  it("formats", () => {
    expect(formatPrice(199)).toBe("1.99");
  });
});
"#,
    );
    let candidates = related_test_candidates(&owner, &tests, None, &index, None);
    assert_eq!(
        candidates.len(),
        1,
        "export {{ default as X }} of a named default export must credit"
    );
    assert_eq!(
        candidates[0].relation,
        TypeScriptRelationKind::ReExportChainFollowed
    );
}

#[test]
fn undercredit_4103_default_as_reexport_non_default_owner_is_not_credited() {
    let owner = test_owner("formatPrice", "src/defaulted.ts");
    let index = ReExportIndex::from_parts(
        vec![(
            ("src/index".to_string(), "formatPrice".to_string()),
            ("default".to_string(), "src/defaulted".to_string()),
        )],
        Vec::new(),
    );
    let tests = extract_tests(
        Path::new("tests/money.test.ts"),
        r#"import { formatPrice } from "../src/index";
describe("formatPrice", () => {
  it("formats", () => {
    expect(formatPrice(199)).toBe("1.99");
  });
});
"#,
    );
    let related = find_related_tests(&owner, &tests, None, &index, None);
    assert!(
        related.is_empty(),
        "the `default` original name must not match an owner that is not the module's default export"
    );
}

#[test]
fn undercredit_4103_owner_extraction_records_default_export_fact() {
    let owners = extract_owners(
        Path::new("src/defaulted.ts"),
        "export default function formatPrice(cents: number): string {\n  return (cents / 100).toFixed(2);\n}\n",
    );
    assert_eq!(owners.len(), 1);
    assert!(owners[0].exported_as_default);

    let plain = extract_owners(
        Path::new("src/plain.ts"),
        "export function other(cents: number): string {\n  return \"x\";\n}\n",
    );
    assert_eq!(plain.len(), 1);
    assert!(!plain[0].exported_as_default);
}
