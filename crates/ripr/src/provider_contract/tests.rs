use serde_json::json;

use crate::domain::ExposureClass;

use super::*;

const HASH: &str = "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const OTHER_HASH: &str = "sha256:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
const TREE_ID: &str = "git-tree:0123456789abcdef0123456789abcdef01234567";
const TREE_DIGEST: &str = "sha256:a0ea7c116d4ed114740a21f4a4a5967e1e6e31755bb8c8896829278d75d55636";
const SHA256_TREE_ID: &str =
    "git-tree:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
const SHA256_TREE_DIGEST: &str =
    "sha256:6abeb43714bf5518cc497c7b6531cd712ca892edc8b5b63974645bdec678793a";

fn error_code(
    result: Result<(), RiprProviderContractErrorV1>,
) -> Option<RiprProviderContractErrorCodeV1> {
    result.err().map(|error| error.code)
}

fn json_error(message: impl Into<String>) -> serde_json::Error {
    <serde_json::Error as serde::de::Error>::custom(message.into())
}

fn required_excluded_claims() -> Vec<String> {
    RIPR_REQUIRED_EXCLUDED_CLAIMS
        .iter()
        .map(|claim| (*claim).to_string())
        .collect()
}

fn snapshot(source_view: RiprSourceViewV1) -> RiprRepositorySnapshotV1 {
    let snapshot_id = match source_view {
        RiprSourceViewV1::GitTree => TREE_ID.to_string(),
        RiprSourceViewV1::GitIndex => format!("git-index:{HASH}"),
        RiprSourceViewV1::Worktree => format!("worktree:{HASH}"),
        RiprSourceViewV1::CapturedSourceSet => format!("captured:{HASH}"),
    };
    let source_digest = match source_view {
        RiprSourceViewV1::GitTree => TREE_DIGEST,
        RiprSourceViewV1::GitIndex
        | RiprSourceViewV1::Worktree
        | RiprSourceViewV1::CapturedSourceSet => HASH,
    };
    RiprRepositorySnapshotV1 {
        repository_id: "EffortlessMetrics/ripr-swarm".into(),
        snapshot_id,
        source_view,
        source_digest: source_digest.into(),
    }
}

fn request() -> RiprAnalysisRequestV1 {
    RiprAnalysisRequestV1 {
        schema_version: RIPR_ANALYSIS_REQUEST_SCHEMA_VERSION.into(),
        request_id: "request-1".into(),
        capability: RiprProviderCapabilityV1::TestGripSummary,
        repository: snapshot(RiprSourceViewV1::GitTree),
        subject: RiprEvidenceSubjectV1 {
            requirement_id: Some("REQ-1".into()),
            evidence_purpose: Some("boundary discriminator".into()),
            seam_id: "seam-1".into(),
            subject_id: "subject-1".into(),
            subject_body_digest: HASH.into(),
        },
        analysis_mode: "ready".into(),
        profile: "rust".into(),
        config_digest: HASH.into(),
        analyzer_generation: "ripr-analyzer.v1".into(),
        output_root: "target/ripr-provider".into(),
        requested_claim: "static test-grip evidence".into(),
    }
}

fn summary() -> RiprProviderEvidenceSummaryV1 {
    RiprProviderEvidenceSummaryV1 {
        seam_id: "seam-1".into(),
        analyzed_subject_count: 1,
        related_tests: vec![RiprProviderEvidenceEntryV1 {
            test_name: "grips_boundary".into(),
            file: "crates/ripr/tests/grip.rs".into(),
            line: 42,
            oracle_kind: "exact_value".into(),
            oracle_strength: "strong".into(),
            relation_reason: "direct_owner_call".into(),
            has_test_target: true,
        }],
        missing_discriminator_count: 0,
        strongest_oracle: "strong".into(),
        fingerprint: "fp:exact_value:strong:direct_owner_call".into(),
    }
}

fn receipt() -> RiprAnalysisReceiptV1 {
    RiprAnalysisReceiptV1 {
        schema_version: RIPR_ANALYSIS_RECEIPT_SCHEMA_VERSION.into(),
        request: request(),
        provider_version: "0.4.0".into(),
        binary_digest: HASH.into(),
        analyzer_generation: "ripr-analyzer.v1".into(),
        result_class: RiprProviderResultClassV1::Completed,
        native_status: Some(ExposureClass::Exposed),
        analysis_complete: true,
        truncated: false,
        summary: Some(summary()),
        diagnostics: Vec::new(),
        limitations: Vec::new(),
        claim_boundary: RIPR_PROVIDER_CLAIM_BOUNDARY.into(),
        excluded_claims: required_excluded_claims(),
    }
}

#[test]
fn read_only_capabilities_validate_with_canonical_non_claims() {
    let capabilities = RiprProviderCapabilitySetV1::read_only("0.4.0");
    assert_eq!(capabilities.validate(), Ok(()));
    assert!(capabilities.capabilities.iter().all(|descriptor| {
        !descriptor.writes_source
            && !descriptor.executes_project_commands
            && !descriptor.uses_network
            && descriptor.claim_boundary == RIPR_PROVIDER_CLAIM_BOUNDARY
            && descriptor.excluded_claims == required_excluded_claims()
    }));
}

#[test]
fn capability_validation_rejects_schema_duplicates_empty_and_authority_drift() {
    let mut unsupported = RiprProviderCapabilitySetV1::read_only("0.4.0");
    unsupported.schema_version = "ripr_provider_capabilities.v2".into();
    assert_eq!(
        error_code(unsupported.validate()),
        Some(RiprProviderContractErrorCodeV1::UnsupportedSchema)
    );

    let mut empty = RiprProviderCapabilitySetV1::read_only("0.4.0");
    empty.capabilities.clear();
    assert_eq!(
        error_code(empty.validate()),
        Some(RiprProviderContractErrorCodeV1::MissingField)
    );

    let mut duplicate = RiprProviderCapabilitySetV1::read_only("0.4.0");
    if let Some(first) = duplicate.capabilities.first().cloned() {
        duplicate.capabilities.push(first);
    }
    assert_eq!(
        error_code(duplicate.validate()),
        Some(RiprProviderContractErrorCodeV1::DuplicateCapability)
    );

    let mut authority = RiprProviderCapabilitySetV1::read_only("0.4.0");
    if let Some(first) = authority.capabilities.first_mut() {
        first.writes_source = true;
    }
    assert_eq!(
        error_code(authority.validate()),
        Some(RiprProviderContractErrorCodeV1::AuthorityViolation)
    );
}

#[test]
fn capability_and_receipt_require_the_complete_excluded_claim_set() {
    let mut capabilities = RiprProviderCapabilitySetV1::read_only("0.4.0");
    if let Some(first) = capabilities.capabilities.first_mut() {
        first
            .excluded_claims
            .retain(|claim| claim != "runtime_test_execution");
    }
    assert_eq!(
        error_code(capabilities.validate()),
        Some(RiprProviderContractErrorCodeV1::MissingField)
    );

    let mut receipt = receipt();
    receipt
        .excluded_claims
        .retain(|claim| claim != "runtime_test_execution");
    assert_eq!(
        error_code(receipt.validate()),
        Some(RiprProviderContractErrorCodeV1::MissingField)
    );
}

#[test]
fn every_source_view_has_one_owned_snapshot_identity_rule() {
    for source_view in [
        RiprSourceViewV1::GitTree,
        RiprSourceViewV1::GitIndex,
        RiprSourceViewV1::Worktree,
        RiprSourceViewV1::CapturedSourceSet,
    ] {
        assert_eq!(snapshot(source_view).validate(), Ok(()));
    }

    let mut invalid_tree = snapshot(RiprSourceViewV1::GitTree);
    invalid_tree.snapshot_id = "git-tree:not-hex".into();
    assert_eq!(
        error_code(invalid_tree.validate()),
        Some(RiprProviderContractErrorCodeV1::MalformedIdentity)
    );

    let mut substituted_tree = snapshot(RiprSourceViewV1::GitTree);
    substituted_tree.snapshot_id = "git-tree:fedcba9876543210fedcba9876543210fedcba98".into();
    assert_eq!(
        error_code(substituted_tree.validate()),
        Some(RiprProviderContractErrorCodeV1::IdentityMismatch)
    );

    let mut mismatched_tree_digest = snapshot(RiprSourceViewV1::GitTree);
    mismatched_tree_digest.source_digest = OTHER_HASH.into();
    assert_eq!(
        error_code(mismatched_tree_digest.validate()),
        Some(RiprProviderContractErrorCodeV1::IdentityMismatch)
    );

    let mut sha256_tree = snapshot(RiprSourceViewV1::GitTree);
    sha256_tree.snapshot_id = SHA256_TREE_ID.into();
    sha256_tree.source_digest = SHA256_TREE_DIGEST.into();
    assert_eq!(sha256_tree.validate(), Ok(()));

    for source_view in [
        RiprSourceViewV1::GitIndex,
        RiprSourceViewV1::Worktree,
        RiprSourceViewV1::CapturedSourceSet,
    ] {
        let mut invalid = snapshot(source_view);
        invalid.source_digest = OTHER_HASH.into();
        assert_eq!(
            error_code(invalid.validate()),
            Some(RiprProviderContractErrorCodeV1::IdentityMismatch)
        );
    }
}

#[test]
fn request_rejects_nonportable_output_roots_and_malformed_digests() {
    for output_root in [
        "../outside".to_string(),
        "/outside".to_string(),
        format!("{}:/outside", 'C'),
        format!("target/{}:/outside", 'C'),
        "target//receipt".to_string(),
        "target/./receipt".to_string(),
        "target\\receipt".to_string(),
        "target/receipt/".to_string(),
    ] {
        let mut request = request();
        request.output_root = output_root.clone();
        assert_eq!(
            error_code(request.validate()),
            Some(RiprProviderContractErrorCodeV1::UnsafeOutputRoot),
            "output_root={output_root}"
        );
    }

    let mut malformed = request();
    malformed.config_digest = "sha256:short".into();
    assert_eq!(
        error_code(malformed.validate()),
        Some(RiprProviderContractErrorCodeV1::MalformedIdentity)
    );
}

#[test]
fn canonical_exposure_class_is_the_only_complete_native_status() -> Result<(), serde_json::Error> {
    let receipt = receipt();
    assert_eq!(receipt.validate(), Ok(()));
    assert_eq!(receipt.native_status, Some(ExposureClass::Exposed));

    let wire = serde_json::to_value(&receipt)?;
    assert_eq!(wire["native_status"], json!("exposed"));
    Ok(())
}

#[test]
fn completed_receipts_require_a_nonzero_analyzed_subject_denominator() {
    let mut zero = receipt();
    if let Some(summary) = zero.summary.as_mut() {
        summary.analyzed_subject_count = 0;
        summary.related_tests.clear();
    }
    assert_eq!(
        error_code(zero.validate()),
        Some(RiprProviderContractErrorCodeV1::CompletenessConflict)
    );
}

#[test]
fn complete_no_static_path_receipts_may_have_no_related_tests() {
    let mut no_path = receipt();
    no_path.native_status = Some(ExposureClass::NoStaticPath);
    if let Some(summary) = no_path.summary.as_mut() {
        summary.related_tests.clear();
        summary.strongest_oracle = "none".into();
        summary.fingerprint = "fp:".into();
    }
    assert_eq!(no_path.validate(), Ok(()));
}

#[test]
fn non_authoritative_results_require_explicit_disclosure() {
    let mut partial = receipt();
    partial.result_class = RiprProviderResultClassV1::Partial;
    partial.native_status = None;
    partial.analysis_complete = false;
    partial.summary = None;
    partial.limitations = vec!["analysis_scope_truncated".into()];
    assert_eq!(partial.validate(), Ok(()));

    partial.limitations.clear();
    assert_eq!(
        error_code(partial.validate()),
        Some(RiprProviderContractErrorCodeV1::MissingField)
    );

    let mut stale = receipt();
    stale.result_class = RiprProviderResultClassV1::StaleInput;
    stale.native_status = None;
    stale.analysis_complete = false;
    stale.summary = None;
    stale.diagnostics = vec![RiprProviderDiagnosticV1 {
        code: "stale_snapshot".into(),
        message: "source snapshot changed".into(),
        source_path: None,
        start_line: None,
        start_column: None,
        next_action: Some("rerun the provider".into()),
    }];
    assert_eq!(stale.validate(), Ok(()));
}

#[test]
fn receipt_validation_rejects_identity_schema_and_completeness_conflicts() {
    let mut analyzer = receipt();
    analyzer.analyzer_generation = "other".into();
    assert_eq!(
        error_code(analyzer.validate()),
        Some(RiprProviderContractErrorCodeV1::IdentityMismatch)
    );

    let mut truncated = receipt();
    truncated.truncated = true;
    assert_eq!(
        error_code(truncated.validate()),
        Some(RiprProviderContractErrorCodeV1::CompletenessConflict)
    );

    let mut unsupported = receipt();
    unsupported.schema_version = "ripr_analysis_receipt.v2".into();
    assert_eq!(
        error_code(unsupported.validate()),
        Some(RiprProviderContractErrorCodeV1::UnsupportedSchema)
    );

    let mut cross_seam = receipt();
    if let Some(summary) = cross_seam.summary.as_mut() {
        summary.seam_id = "other-seam".into();
    }
    assert_eq!(
        error_code(cross_seam.validate()),
        Some(RiprProviderContractErrorCodeV1::IdentityMismatch)
    );
}

#[test]
fn public_wire_round_trips_and_rejects_unknown_fields() -> Result<(), serde_json::Error> {
    let receipt = receipt();
    let serialized = serde_json::to_string(&receipt)?;
    let decoded = serde_json::from_str::<RiprAnalysisReceiptV1>(&serialized)?;
    assert_eq!(decoded, receipt);

    let mut request_value = serde_json::to_value(request())?;
    let Some(object) = request_value.as_object_mut() else {
        return Err(json_error("request did not serialize as an object"));
    };
    object.insert("unexpected".into(), json!(true));
    match serde_json::from_value::<RiprAnalysisRequestV1>(request_value) {
        Err(_) => Ok(()),
        Ok(_) => Err(json_error("unknown request field was accepted")),
    }
}

#[test]
fn capability_set_rejects_identity_drift_and_descriptor_contract_drift() {
    let mut foreign_provider = RiprProviderCapabilitySetV1::read_only("0.4.0");
    foreign_provider.provider = "other".into();
    assert_eq!(
        error_code(foreign_provider.validate()),
        Some(RiprProviderContractErrorCodeV1::IdentityMismatch)
    );

    let mut blank_version = RiprProviderCapabilitySetV1::read_only("0.4.0");
    blank_version.provider_version = "  ".into();
    assert_eq!(
        error_code(blank_version.validate()),
        Some(RiprProviderContractErrorCodeV1::MissingField)
    );

    let mut request_schema = RiprProviderCapabilitySetV1::read_only("0.4.0");
    if let Some(first) = request_schema.capabilities.first_mut() {
        first.supported_request_schema = "ripr_analysis_request.v2".into();
    }
    assert_eq!(
        error_code(request_schema.validate()),
        Some(RiprProviderContractErrorCodeV1::UnsupportedSchema)
    );

    let mut receipt_schema = RiprProviderCapabilitySetV1::read_only("0.4.0");
    if let Some(first) = receipt_schema.capabilities.first_mut() {
        first.supported_receipt_schema = "ripr_analysis_receipt.v2".into();
    }
    assert_eq!(
        error_code(receipt_schema.validate()),
        Some(RiprProviderContractErrorCodeV1::UnsupportedSchema)
    );

    let mut boundary = RiprProviderCapabilitySetV1::read_only("0.4.0");
    if let Some(first) = boundary.capabilities.first_mut() {
        first.claim_boundary = "custom boundary".into();
    }
    assert_eq!(
        error_code(boundary.validate()),
        Some(RiprProviderContractErrorCodeV1::AuthorityViolation)
    );

    let mut executing = RiprProviderCapabilitySetV1::read_only("0.4.0");
    if let Some(first) = executing.capabilities.first_mut() {
        first.executes_project_commands = true;
    }
    assert_eq!(
        error_code(executing.validate()),
        Some(RiprProviderContractErrorCodeV1::AuthorityViolation)
    );

    let mut online = RiprProviderCapabilitySetV1::read_only("0.4.0");
    if let Some(first) = online.capabilities.first_mut() {
        first.uses_network = true;
    }
    assert_eq!(
        error_code(online.validate()),
        Some(RiprProviderContractErrorCodeV1::AuthorityViolation)
    );

    let mut duplicate_claims = RiprProviderCapabilitySetV1::read_only("0.4.0");
    if let Some(first) = duplicate_claims.capabilities.first_mut() {
        first.excluded_claims.push("merge_readiness".into());
    }
    assert_eq!(
        error_code(duplicate_claims.validate()),
        Some(RiprProviderContractErrorCodeV1::MissingField)
    );
}

#[test]
fn contract_errors_display_the_variant_code_and_message() -> Result<(), String> {
    let mut foreign_provider = RiprProviderCapabilitySetV1::read_only("0.4.0");
    foreign_provider.provider = "other".into();
    let error = foreign_provider
        .validate()
        .err()
        .ok_or_else(|| "expected a contract error".to_string())?;
    let rendered = error.to_string();
    assert!(
        rendered.contains("IdentityMismatch"),
        "display must name the variant code: {rendered}"
    );
    assert!(
        rendered.contains("provider identity must be ripr"),
        "display must carry the message: {rendered}"
    );
    Ok(())
}

#[test]
fn snapshots_require_the_prefix_hex_width_and_digest_shape() {
    let mut unprefixed_tree = snapshot(RiprSourceViewV1::GitTree);
    unprefixed_tree.snapshot_id = TREE_ID.trim_start_matches("git-tree:").into();
    assert_eq!(
        error_code(unprefixed_tree.validate()),
        Some(RiprProviderContractErrorCodeV1::MalformedIdentity)
    );

    for width in [39usize, 41usize, 65usize] {
        let mut wrong_width = snapshot(RiprSourceViewV1::GitTree);
        wrong_width.snapshot_id = format!("git-tree:{}", "a".repeat(width));
        assert_eq!(
            error_code(wrong_width.validate()),
            Some(RiprProviderContractErrorCodeV1::MalformedIdentity),
            "width={width}"
        );
    }

    let mut non_hex_tree = snapshot(RiprSourceViewV1::GitTree);
    non_hex_tree.snapshot_id = format!("git-tree:{}", "g".repeat(40));
    assert_eq!(
        error_code(non_hex_tree.validate()),
        Some(RiprProviderContractErrorCodeV1::MalformedIdentity)
    );

    let mut unnamed_repository = snapshot(RiprSourceViewV1::GitTree);
    unnamed_repository.repository_id = "  ".into();
    assert_eq!(
        error_code(unnamed_repository.validate()),
        Some(RiprProviderContractErrorCodeV1::MissingField)
    );

    for source_view in [
        RiprSourceViewV1::GitTree,
        RiprSourceViewV1::GitIndex,
        RiprSourceViewV1::Worktree,
        RiprSourceViewV1::CapturedSourceSet,
    ] {
        let mut unprefixed_digest = snapshot(source_view);
        unprefixed_digest.source_digest = HASH.trim_start_matches("sha256:").to_string();
        assert_eq!(
            error_code(unprefixed_digest.validate()),
            Some(RiprProviderContractErrorCodeV1::MalformedIdentity),
            "view={source_view:?}"
        );

        let mut non_hex_digest = snapshot(source_view);
        non_hex_digest.source_digest = format!("sha256:{}", "z".repeat(64));
        assert_eq!(
            error_code(non_hex_digest.validate()),
            Some(RiprProviderContractErrorCodeV1::MalformedIdentity),
            "view={source_view:?}"
        );
    }
}

#[test]
fn requests_reject_unsupported_schemas_and_blank_required_fields() {
    let mut unsupported = request();
    unsupported.schema_version = RIPR_ANALYSIS_REQUEST_SCHEMA_VERSION.replace("v1", "v2");
    assert_eq!(
        error_code(unsupported.validate()),
        Some(RiprProviderContractErrorCodeV1::UnsupportedSchema)
    );

    let mut blank_request_id = request();
    blank_request_id.request_id = "".into();
    assert_eq!(
        error_code(blank_request_id.validate()),
        Some(RiprProviderContractErrorCodeV1::MissingField)
    );

    let mut blank_seam = request();
    blank_seam.subject.seam_id = " ".into();
    assert_eq!(
        error_code(blank_seam.validate()),
        Some(RiprProviderContractErrorCodeV1::MissingField)
    );

    let mut blank_subject = request();
    blank_subject.subject.subject_id = "".into();
    assert_eq!(
        error_code(blank_subject.validate()),
        Some(RiprProviderContractErrorCodeV1::MissingField)
    );

    let mut blank_mode = request();
    blank_mode.analysis_mode = "".into();
    assert_eq!(
        error_code(blank_mode.validate()),
        Some(RiprProviderContractErrorCodeV1::MissingField)
    );

    let mut blank_profile = request();
    blank_profile.profile = " ".into();
    assert_eq!(
        error_code(blank_profile.validate()),
        Some(RiprProviderContractErrorCodeV1::MissingField)
    );

    let mut blank_generation = request();
    blank_generation.analyzer_generation = "".into();
    assert_eq!(
        error_code(blank_generation.validate()),
        Some(RiprProviderContractErrorCodeV1::MissingField)
    );

    let mut blank_claim = request();
    blank_claim.requested_claim = "".into();
    assert_eq!(
        error_code(blank_claim.validate()),
        Some(RiprProviderContractErrorCodeV1::MissingField)
    );

    let mut blank_requirement = request();
    blank_requirement.subject.requirement_id = Some("".into());
    assert_eq!(
        error_code(blank_requirement.validate()),
        Some(RiprProviderContractErrorCodeV1::MissingField)
    );

    let mut blank_purpose = request();
    blank_purpose.subject.evidence_purpose = Some("   ".into());
    assert_eq!(
        error_code(blank_purpose.validate()),
        Some(RiprProviderContractErrorCodeV1::MissingField)
    );

    let mut short_subject_digest = request();
    short_subject_digest.subject.subject_body_digest = "sha256:tooshort".into();
    assert_eq!(
        error_code(short_subject_digest.validate()),
        Some(RiprProviderContractErrorCodeV1::MalformedIdentity)
    );

    let mut unprefixed_config = request();
    unprefixed_config.config_digest = HASH.trim_start_matches("sha256:").to_string();
    assert_eq!(
        error_code(unprefixed_config.validate()),
        Some(RiprProviderContractErrorCodeV1::MalformedIdentity)
    );
}

#[test]
fn completed_results_require_a_complete_analysis_with_status_and_summary() {
    let mut without_status = receipt();
    without_status.native_status = None;
    assert_eq!(
        error_code(without_status.validate()),
        Some(RiprProviderContractErrorCodeV1::MissingField)
    );

    let mut without_summary = receipt();
    without_summary.summary = None;
    assert_eq!(
        error_code(without_summary.validate()),
        Some(RiprProviderContractErrorCodeV1::MissingField)
    );

    let mut incomplete = receipt();
    incomplete.analysis_complete = false;
    assert_eq!(
        error_code(incomplete.validate()),
        Some(RiprProviderContractErrorCodeV1::CompletenessConflict)
    );
}

#[test]
fn every_non_authoritative_class_requires_incompleteness_and_disclosure() {
    fn disclosed_failure(class: RiprProviderResultClassV1) -> RiprAnalysisReceiptV1 {
        let mut failed = receipt();
        failed.result_class = class;
        failed.analysis_complete = false;
        failed.native_status = None;
        failed.summary = None;
        failed.diagnostics = vec![RiprProviderDiagnosticV1 {
            code: "analysis_deferred".into(),
            message: "seams deferred".into(),
            source_path: None,
            start_line: None,
            start_column: None,
            next_action: None,
        }];
        failed.limitations = Vec::new();
        failed
    }

    for class in [
        RiprProviderResultClassV1::Partial,
        RiprProviderResultClassV1::StaleInput,
        RiprProviderResultClassV1::Unsupported,
        RiprProviderResultClassV1::MalformedInput,
        RiprProviderResultClassV1::InstrumentFailure,
        RiprProviderResultClassV1::Cancelled,
        RiprProviderResultClassV1::NotProven,
    ] {
        let mut failed = disclosed_failure(class);
        assert_eq!(failed.validate(), Ok(()), "class={class:?}");

        failed.analysis_complete = true;
        assert_eq!(
            error_code(failed.validate()),
            Some(RiprProviderContractErrorCodeV1::CompletenessConflict),
            "class={class:?}"
        );
    }

    let mut silent = disclosed_failure(RiprProviderResultClassV1::Cancelled);
    silent.diagnostics.clear();
    assert_eq!(
        error_code(silent.validate()),
        Some(RiprProviderContractErrorCodeV1::MissingField)
    );
}

#[test]
fn summaries_validate_every_field_and_related_test_entry() {
    let mut blank_oracle = receipt();
    if let Some(summary) = blank_oracle.summary.as_mut() {
        summary.strongest_oracle = "".into();
    }
    assert_eq!(
        error_code(blank_oracle.validate()),
        Some(RiprProviderContractErrorCodeV1::MissingField)
    );

    let mut blank_fingerprint = receipt();
    if let Some(summary) = blank_fingerprint.summary.as_mut() {
        summary.fingerprint = " ".into();
    }
    assert_eq!(
        error_code(blank_fingerprint.validate()),
        Some(RiprProviderContractErrorCodeV1::MissingField)
    );

    let mut blank_test_name = receipt();
    if let Some(summary) = blank_test_name.summary.as_mut()
        && let Some(entry) = summary.related_tests.first_mut()
    {
        entry.test_name = "".into();
    }
    assert_eq!(
        error_code(blank_test_name.validate()),
        Some(RiprProviderContractErrorCodeV1::MissingField)
    );

    let mut absolute_file = receipt();
    if let Some(summary) = absolute_file.summary.as_mut()
        && let Some(entry) = summary.related_tests.first_mut()
    {
        entry.file = "/abs/tests/grip.rs".into();
    }
    assert_eq!(
        error_code(absolute_file.validate()),
        Some(RiprProviderContractErrorCodeV1::MalformedIdentity)
    );

    let mut zero_line = receipt();
    if let Some(summary) = zero_line.summary.as_mut()
        && let Some(entry) = summary.related_tests.first_mut()
    {
        entry.line = 0;
    }
    assert_eq!(
        error_code(zero_line.validate()),
        Some(RiprProviderContractErrorCodeV1::MalformedIdentity)
    );

    let mut blank_kind = receipt();
    if let Some(summary) = blank_kind.summary.as_mut()
        && let Some(entry) = summary.related_tests.first_mut()
    {
        entry.oracle_kind = "".into();
    }
    assert_eq!(
        error_code(blank_kind.validate()),
        Some(RiprProviderContractErrorCodeV1::MissingField)
    );

    let mut blank_strength = receipt();
    if let Some(summary) = blank_strength.summary.as_mut()
        && let Some(entry) = summary.related_tests.first_mut()
    {
        entry.oracle_strength = "".into();
    }
    assert_eq!(
        error_code(blank_strength.validate()),
        Some(RiprProviderContractErrorCodeV1::MissingField)
    );

    let mut blank_reason = receipt();
    if let Some(summary) = blank_reason.summary.as_mut()
        && let Some(entry) = summary.related_tests.first_mut()
    {
        entry.relation_reason = "".into();
    }
    assert_eq!(
        error_code(blank_reason.validate()),
        Some(RiprProviderContractErrorCodeV1::MissingField)
    );
}

#[test]
fn diagnostics_require_identity_and_a_coherent_source_position() {
    fn diagnostic_with(
        mutate: impl FnOnce(&mut RiprProviderDiagnosticV1),
    ) -> RiprAnalysisReceiptV1 {
        let mut receipt = receipt();
        receipt.result_class = RiprProviderResultClassV1::Partial;
        receipt.analysis_complete = false;
        receipt.native_status = None;
        receipt.summary = None;
        receipt.diagnostics = vec![RiprProviderDiagnosticV1 {
            code: "partial_scope".into(),
            message: "bounded run".into(),
            source_path: None,
            start_line: None,
            start_column: None,
            next_action: None,
        }];
        if let Some(diagnostic) = receipt.diagnostics.first_mut() {
            mutate(diagnostic);
        }
        receipt
    }

    let blank_code = diagnostic_with(|diagnostic| diagnostic.code = "".into());
    assert_eq!(
        error_code(blank_code.validate()),
        Some(RiprProviderContractErrorCodeV1::MissingField)
    );

    let blank_message = diagnostic_with(|diagnostic| diagnostic.message = " ".into());
    assert_eq!(
        error_code(blank_message.validate()),
        Some(RiprProviderContractErrorCodeV1::MissingField)
    );

    let blank_action = diagnostic_with(|diagnostic| diagnostic.next_action = Some("".into()));
    assert_eq!(
        error_code(blank_action.validate()),
        Some(RiprProviderContractErrorCodeV1::MissingField)
    );

    let absolute_path =
        diagnostic_with(|diagnostic| diagnostic.source_path = Some("/abs/src/lib.rs".into()));
    assert_eq!(
        error_code(absolute_path.validate()),
        Some(RiprProviderContractErrorCodeV1::MalformedIdentity)
    );

    for (line, column) in [
        (Some(0), Some(1)),
        (Some(1), Some(0)),
        (Some(1), None),
        (None, Some(1)),
    ] {
        let incoherent = diagnostic_with(|diagnostic| {
            diagnostic.start_line = line;
            diagnostic.start_column = column;
        });
        assert_eq!(
            error_code(incoherent.validate()),
            Some(RiprProviderContractErrorCodeV1::MalformedIdentity),
            "line={line:?} column={column:?}"
        );
    }

    let positioned = diagnostic_with(|diagnostic| {
        diagnostic.source_path = Some("crates/ripr/src/lib.rs".into());
        diagnostic.start_line = Some(3);
        diagnostic.start_column = Some(7);
    });
    assert_eq!(positioned.validate(), Ok(()));

    let mut blank_limitation = diagnostic_with(|_| ());
    blank_limitation.limitations = vec!["   ".into()];
    assert_eq!(
        error_code(blank_limitation.validate()),
        Some(RiprProviderContractErrorCodeV1::MissingField)
    );
}

#[test]
fn summary_rejects_noncanonical_taxonomy_and_counter_contradictions() {
    let mut invalid_kind = receipt();
    if let Some(summary) = invalid_kind.summary.as_mut()
        && let Some(entry) = summary.related_tests.first_mut()
    {
        entry.oracle_kind = "proven".into(); // ripr-allow: static-language: taxonomy fixture value
    }
    assert_eq!(
        error_code(invalid_kind.validate()),
        Some(RiprProviderContractErrorCodeV1::MalformedIdentity)
    );

    let mut invalid_strength = receipt();
    if let Some(summary) = invalid_strength.summary.as_mut()
        && let Some(entry) = summary.related_tests.first_mut()
    {
        entry.oracle_strength = "adequate".into(); // ripr-allow: static-language: taxonomy fixture value
    }
    assert_eq!(
        error_code(invalid_strength.validate()),
        Some(RiprProviderContractErrorCodeV1::MalformedIdentity)
    );

    let mut invalid_count = receipt();
    if let Some(summary) = invalid_count.summary.as_mut() {
        summary.missing_discriminator_count = 2;
    }
    assert_eq!(
        error_code(invalid_count.validate()),
        Some(RiprProviderContractErrorCodeV1::CompletenessConflict)
    );

    let mut empty_related = receipt();
    if let Some(summary) = empty_related.summary.as_mut() {
        summary.related_tests.clear();
    }
    assert_eq!(
        error_code(empty_related.validate()),
        Some(RiprProviderContractErrorCodeV1::CompletenessConflict)
    );
}

#[test]
fn findings_require_the_same_authoritative_shape_as_completed() {
    let mut findings = receipt();
    findings.result_class = RiprProviderResultClassV1::Findings;
    assert_eq!(findings.validate(), Ok(()));

    findings.native_status = None;
    assert_eq!(
        error_code(findings.validate()),
        Some(RiprProviderContractErrorCodeV1::MissingField)
    );

    let mut incomplete = receipt();
    incomplete.result_class = RiprProviderResultClassV1::Findings;
    incomplete.analysis_complete = false;
    assert_eq!(
        error_code(incomplete.validate()),
        Some(RiprProviderContractErrorCodeV1::CompletenessConflict)
    );
}

#[test]
fn summary_strongest_oracle_must_match_related_evidence() {
    let mut inconsistent = receipt();
    if let Some(summary) = inconsistent.summary.as_mut() {
        summary.strongest_oracle = "strong".into();
        summary.related_tests[0].oracle_strength = "smoke".into();
    }
    assert_eq!(
        error_code(inconsistent.validate()),
        Some(RiprProviderContractErrorCodeV1::CompletenessConflict)
    );
}
