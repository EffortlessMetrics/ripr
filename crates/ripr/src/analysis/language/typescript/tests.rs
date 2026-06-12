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
        imports: Vec::new(),
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
            oracle_kind,
            oracle_strength,
            mock_payload: None,
            error_payload: None,
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
        body_text: "expect(90).toBe(90);".to_string(),
        assertions: vec![TypeScriptAssertion {
            matcher: "toBe".to_string(),
            argument_count: 1,
            line: 1,
            oracle_kind: OracleKind::ExactValue,
            oracle_strength: OracleStrength::Strong,
            mock_payload: None,
            error_payload: None,
        }],
        mocks_in_file: Vec::new(),
        imports_in_file: Vec::new(),
    }
}

fn classify_weak_direct_line(line_text: &str) -> Result<Finding, String> {
    let owner = test_owner("applyDiscount", "src/lib.ts");
    let test = weak_direct_test_for("applyDiscount");
    classify_change(Path::new("src/lib.ts"), 2, line_text, &[owner], &[test])
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
  const growable = new ArrayBuffer(4, { maxByteLength: 8 });
  const view = new Uint8Array(growable);
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
    )
    .ok_or_else(|| "expected TypeScript preview finding".to_string())?;

    assert!(matches!(finding.class, ExposureClass::Exposed));
    assert_evidence_contains(
        &finding,
        "typescript_bun_ub_advisory_fact: shared_array_buffer",
    );
    assert_evidence_contains(
        &finding,
        "typescript_bun_ub_advisory_fact: resizable_array_buffer",
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
    assert_evidence_contains(
        &finding,
        "typescript_bun_ub_bridge_verdict: ts_discriminated missing_discriminators=none action=no_missing_bridge_discriminator",
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
    assert!(!adapter.accepts_path(Path::new("src/lib.rs")));
    assert!(!adapter.accepts_path(Path::new("scripts/run.py")));
    assert!(!adapter.accepts_path(Path::new("README.md")));
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
    assert!(!is_test_file(Path::new("src/lib.ts")));
    assert!(!is_test_file(Path::new("README.md")));
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
        imports: Vec::new(),
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
    let related = find_related_tests(&owner, &tests);
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
        imports: Vec::new(),
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

    let related = find_related_tests(&owner, &tests);

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
        imports: Vec::new(),
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

    let candidates = related_test_candidates(&owner, &tests);
    let related = find_related_tests(&owner, &tests);

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
        imports: Vec::new(),
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

    let related = find_related_tests(&owner, &tests);

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
        imports: Vec::new(),
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

    let related = find_related_tests(&owner, &tests);

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
        imports: Vec::new(),
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

    let related = find_related_tests(&owner, &tests);

    assert!(related.is_empty());
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
        imports: Vec::new(),
    };
    let tests = extract_tests(
        Path::new("tests/owners.test.ts"),
        r#"import { Cart as Subject } from "../src/owners";

test("static build observes class method", () => {
    expect(Subject.build()).toBeDefined();
});
"#,
    );

    let candidates = related_test_candidates(&owner, &tests);
    let related = find_related_tests(&owner, &tests);

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
        imports: Vec::new(),
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

    let related = find_related_tests(&owner, &tests);

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
        imports: Vec::new(),
    };
    let tests = extract_tests(
        Path::new("src/owners.ts"),
        r#"test("same file static build observes class method", () => {
    expect(Cart.build()).toBeDefined();
});
"#,
    );

    let candidates = related_test_candidates(&owner, &tests);
    let related = find_related_tests(&owner, &tests);

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
        imports: Vec::new(),
    };
    let tests = extract_tests(
        Path::new("tests/owners.test.ts"),
        r#"import * as Owners from "../src/owners";

test("namespace static build stays ambiguous", () => {
    expect(Owners.Cart.build()).toBeDefined();
});
"#,
    );

    let related = find_related_tests(&owner, &tests);

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
        imports: Vec::new(),
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

    let related = find_related_tests(&owner, &tests);

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
        imports: Vec::new(),
    };
    let tests = extract_tests(
        Path::new("tests/owners.test.ts"),
        r#"import { Cart } from "../src/owners";

test("unknown class static build stays ambiguous", () => {
    expect(Cart.build()).toBeDefined();
});
"#,
    );

    let related = find_related_tests(&owner, &tests);

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
        imports: Vec::new(),
    };
    let tests = extract_tests(
        Path::new("tests/owners.test.ts"),
        r#"import { DEFAULT_RATE as rate } from "../src/owners";

test("rate value observes initializer", () => {
    expect(rate).toBe(0.09);
});
"#,
    );

    let candidates = related_test_candidates(&owner, &tests);
    let related = find_related_tests(&owner, &tests);

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
        imports: Vec::new(),
    };
    let tests = extract_tests(
        Path::new("tests/owners.test.ts"),
        r#"import * as owners from "../src/owners";

test("rate value observes namespace initializer", () => {
    expect(owners.DEFAULT_RATE).toBe(0.09);
});
"#,
    );

    let related = find_related_tests(&owner, &tests);

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
        imports: Vec::new(),
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

    let related = find_related_tests(&owner, &tests);

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
        imports: Vec::new(),
    };
    let tests = extract_tests(
        Path::new("tests/pricing.test.ts"),
        r#"import { applyDiscount as subject } from "../src/pricing";

test("alias import observes threshold", () => {
    expect(subject(100, 100)).toBe(90);
});
"#,
    );

    let related = find_related_tests(&owner, &tests);

    assert_eq!(related.len(), 1);
    assert_eq!(related[0].name, "alias import observes threshold");
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
        imports: Vec::new(),
    };
    let tests = extract_tests(
        Path::new("tests/pricing.test.ts"),
        r#"import * as pricing from "../src/pricing";

test("namespace import observes threshold", () => {
    expect(pricing.applyDiscount(100, 100)).toBe(90);
});
"#,
    );

    let related = find_related_tests(&owner, &tests);

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
        imports: Vec::new(),
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

    let related = find_related_tests(&owner, &tests);

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
        imports: Vec::new(),
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

    let related = find_related_tests(&owner, &tests);

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
        imports: Vec::new(),
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

    let related = find_related_tests(&owner, &tests);

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
        imports: Vec::new(),
    };
    let mut tests = extract_tests(
        Path::new("tests/pricing.test.ts"),
        r#"test("threshold documented elsewhere", () => {
    expect(90).toBe(90);
});
"#,
    );
    tests.extend(extract_tests(
        Path::new("tests/checkout.test.ts"),
        r#"describe("applyDiscount", () => {
    test("threshold documented elsewhere", () => {
        expect(90).toBe(90);
    });
});
"#,
    ));
    tests.extend(extract_tests(
        Path::new("tests/cart.test.ts"),
        r#"test("applyDiscount boundary", () => {
    expect(90).toBe(90);
});
"#,
    ));

    let candidates = related_test_candidates(&owner, &tests);
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

    let related = find_related_tests(&owner, &tests);
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
        imports: Vec::new(),
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

    let candidates = related_test_candidates(&owner, &tests);

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
        imports: Vec::new(),
    };
    let tests = extract_tests(
        Path::new("tests/pricing.test.ts"),
        r#"test("threshold documented elsewhere", () => {
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
        imports: Vec::new(),
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
        imports: Vec::new(),
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
            imports: Vec::new(),
        },
        TypeScriptOwner {
            name: "betaScore".to_string(),
            file: PathBuf::from("src/b.ts"),
            start_line: 1,
            end_line: 5,
            owner_kind: OwnerKind::Function,
            class_name: None,
            decorated: false,
            imports: Vec::new(),
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
        imports: Vec::new(),
    };
    let test = TypeScriptTest {
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
fn classify_change_returns_no_static_path_when_no_related_test() -> Result<(), String> {
    let owner = TypeScriptOwner {
        name: "applyDiscount".to_string(),
        file: PathBuf::from("src/lib.ts"),
        start_line: 1,
        end_line: 5,
        owner_kind: OwnerKind::Function,
        class_name: None,
        decorated: false,
        imports: Vec::new(),
    };
    let finding = classify_change(
        Path::new("src/lib.ts"),
        2,
        "    if (amount >= threshold) {",
        &[owner],
        &[],
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
        imports: Vec::new(),
    };
    let finding = classify_change(
        Path::new("src/lib.ts"),
        5,
        "// top-level comment",
        &[owner],
        &[],
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
fn analyze_repo_returns_empty_scaffold() -> Result<(), String> {
    let adapter = TypeScriptAdapter;
    let options = AnalysisOptions {
        root: PathBuf::from("/nonexistent_workspace"),
        base: None,
        diff_file: None,
        mode: crate::analysis::AnalysisMode::Deep,
        include_unchanged_tests: false,
    };
    let policy = OraclePolicy::default();
    let result = adapter.analyze_repo(&options, &policy)?;
    assert!(result.findings.is_empty());
    assert_eq!(result.production_files, 0);
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
        imports: Vec::new(),
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
    let paths = collect_related_mock_paths(&owner, &tests);
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
        imports: Vec::new(),
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
    let paths = collect_related_mock_paths(&owner, &tests);
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
        imports: Vec::new(),
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
    let paths = collect_related_mock_paths(&owner, &tests);
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
        imports: Vec::new(),
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
