use std::collections::BTreeSet;

use sha2::{Digest, Sha256};

use super::*;
use crate::domain::{ExposureClass, OracleKind, OracleStrength, RelationReason};

impl RiprProviderCapabilitySetV1 {
    pub fn read_only(provider_version: impl Into<String>) -> Self {
        let capabilities = [
            RiprProviderCapabilityV1::TestGripSummary,
            RiprProviderCapabilityV1::ActivationPropagationObservableAnalysis,
            RiprProviderCapabilityV1::RequiredDiscriminatorAnalysis,
            RiprProviderCapabilityV1::OracleScopeAnalysis,
            RiprProviderCapabilityV1::SelectedSeamEvidenceSummary,
            RiprProviderCapabilityV1::CapturedRiprReceiptValidation,
        ]
        .into_iter()
        .map(read_only_descriptor)
        .collect();

        Self {
            schema_version: RIPR_PROVIDER_CAPABILITY_SCHEMA_VERSION.into(),
            provider: "ripr".into(),
            provider_version: provider_version.into(),
            capabilities,
        }
    }

    pub fn validate(&self) -> Result<(), RiprProviderContractErrorV1> {
        if self.schema_version != RIPR_PROVIDER_CAPABILITY_SCHEMA_VERSION {
            return Err(error(
                RiprProviderContractErrorCodeV1::UnsupportedSchema,
                "unsupported RIPR capability schema",
            ));
        }
        if self.provider != "ripr" {
            return Err(error(
                RiprProviderContractErrorCodeV1::IdentityMismatch,
                "provider identity must be ripr",
            ));
        }
        require_non_empty("provider_version", &self.provider_version)?;
        if self.capabilities.is_empty() {
            return Err(error(
                RiprProviderContractErrorCodeV1::MissingField,
                "capability set is empty",
            ));
        }

        let mut capabilities = BTreeSet::new();
        for descriptor in &self.capabilities {
            if !capabilities.insert(descriptor.capability) {
                return Err(error(
                    RiprProviderContractErrorCodeV1::DuplicateCapability,
                    "capability set contains a duplicate capability",
                ));
            }
            if descriptor.writes_source
                || descriptor.executes_project_commands
                || descriptor.uses_network
            {
                return Err(error(
                    RiprProviderContractErrorCodeV1::AuthorityViolation,
                    "RIPR provider capabilities must remain read-only, offline, and non-executing",
                ));
            }
            if descriptor.supported_request_schema != RIPR_ANALYSIS_REQUEST_SCHEMA_VERSION
                || descriptor.supported_receipt_schema != RIPR_ANALYSIS_RECEIPT_SCHEMA_VERSION
            {
                return Err(error(
                    RiprProviderContractErrorCodeV1::UnsupportedSchema,
                    "capability descriptor references an unsupported request or receipt schema",
                ));
            }
            require_claim_boundary(&descriptor.claim_boundary)?;
            require_excluded_claims(&descriptor.excluded_claims)?;
        }
        Ok(())
    }
}

impl RiprRepositorySnapshotV1 {
    pub fn validate(&self) -> Result<(), RiprProviderContractErrorV1> {
        require_non_empty("repository_id", &self.repository_id)?;
        require_sha256("source_digest", &self.source_digest)?;

        match self.source_view {
            RiprSourceViewV1::GitTree => {
                let object_id = self.snapshot_id.strip_prefix("git-tree:").ok_or_else(|| {
                    error(
                        RiprProviderContractErrorCodeV1::MalformedIdentity,
                        "git-tree snapshot_id must use git-tree:<object-id>",
                    )
                })?;
                if !is_lower_hex(object_id) || !matches!(object_id.len(), 40 | 64) {
                    return Err(error(
                        RiprProviderContractErrorCodeV1::MalformedIdentity,
                        "git-tree snapshot object id must be 40 or 64 lowercase hexadecimal characters",
                    ));
                }
                require_git_tree_digest(&self.snapshot_id, &self.source_digest)?;
            }
            RiprSourceViewV1::GitIndex => {
                require_derived_snapshot("git-index:", &self.snapshot_id, &self.source_digest)?;
            }
            RiprSourceViewV1::Worktree => {
                require_derived_snapshot("worktree:", &self.snapshot_id, &self.source_digest)?;
            }
            RiprSourceViewV1::CapturedSourceSet => {
                require_derived_snapshot("captured:", &self.snapshot_id, &self.source_digest)?;
            }
        }
        Ok(())
    }
}

impl RiprAnalysisRequestV1 {
    pub fn validate(&self) -> Result<(), RiprProviderContractErrorV1> {
        if self.schema_version != RIPR_ANALYSIS_REQUEST_SCHEMA_VERSION {
            return Err(error(
                RiprProviderContractErrorCodeV1::UnsupportedSchema,
                "unsupported RIPR analysis request schema",
            ));
        }
        require_non_empty("request_id", &self.request_id)?;
        self.repository.validate()?;
        require_optional_non_empty("requirement_id", self.subject.requirement_id.as_deref())?;
        require_optional_non_empty("evidence_purpose", self.subject.evidence_purpose.as_deref())?;
        require_non_empty("seam_id", &self.subject.seam_id)?;
        require_non_empty("subject_id", &self.subject.subject_id)?;
        require_sha256("subject_body_digest", &self.subject.subject_body_digest)?;
        require_non_empty("analysis_mode", &self.analysis_mode)?;
        require_non_empty("profile", &self.profile)?;
        require_sha256("config_digest", &self.config_digest)?;
        require_non_empty("analyzer_generation", &self.analyzer_generation)?;
        validate_portable_relative_path(
            "output_root",
            &self.output_root,
            RiprProviderContractErrorCodeV1::UnsafeOutputRoot,
        )?;
        require_non_empty("requested_claim", &self.requested_claim)
    }
}

impl RiprAnalysisReceiptV1 {
    pub fn validate(&self) -> Result<(), RiprProviderContractErrorV1> {
        if self.schema_version != RIPR_ANALYSIS_RECEIPT_SCHEMA_VERSION {
            return Err(error(
                RiprProviderContractErrorCodeV1::UnsupportedSchema,
                "unsupported RIPR analysis receipt schema",
            ));
        }
        self.request.validate()?;
        require_non_empty("provider_version", &self.provider_version)?;
        require_sha256("binary_digest", &self.binary_digest)?;
        require_non_empty("analyzer_generation", &self.analyzer_generation)?;
        if self.analyzer_generation != self.request.analyzer_generation {
            return Err(error(
                RiprProviderContractErrorCodeV1::IdentityMismatch,
                "receipt analyzer generation does not match the request",
            ));
        }
        require_claim_boundary(&self.claim_boundary)?;
        require_excluded_claims(&self.excluded_claims)?;
        validate_text_list("limitations", &self.limitations)?;
        for diagnostic in &self.diagnostics {
            validate_diagnostic(diagnostic)?;
        }
        let authoritative = matches!(
            self.result_class,
            RiprProviderResultClassV1::Completed | RiprProviderResultClassV1::Findings
        );
        if let Some(summary) = &self.summary {
            if !authoritative {
                return Err(error(
                    RiprProviderContractErrorCodeV1::CompletenessConflict,
                    "non-authoritative results cannot carry evidence summaries",
                ));
            }
            validate_summary(
                summary,
                &self.request.subject.seam_id,
                authoritative,
                matches!(self.native_status, Some(ExposureClass::NoStaticPath)),
            )?;
        }
        if self.analysis_complete && self.truncated {
            return Err(error(
                RiprProviderContractErrorCodeV1::CompletenessConflict,
                "a truncated analysis cannot be complete",
            ));
        }

        match self.result_class {
            RiprProviderResultClassV1::Completed | RiprProviderResultClassV1::Findings => {
                if !self.analysis_complete || self.truncated {
                    return Err(error(
                        RiprProviderContractErrorCodeV1::CompletenessConflict,
                        "complete or findings results require a complete, untruncated analysis",
                    ));
                }
                if self.native_status.is_none() || self.summary.is_none() {
                    return Err(error(
                        RiprProviderContractErrorCodeV1::MissingField,
                        "complete or findings results require canonical exposure status and summary",
                    ));
                }
            }
            RiprProviderResultClassV1::Partial => {
                if self.native_status.is_some() {
                    return Err(error(
                        RiprProviderContractErrorCodeV1::CompletenessConflict,
                        "partial results cannot carry native exposure status",
                    ));
                }
                if self.analysis_complete {
                    return Err(error(
                        RiprProviderContractErrorCodeV1::CompletenessConflict,
                        "partial result cannot claim complete analysis",
                    ));
                }
                require_disclosure(self)?;
            }
            RiprProviderResultClassV1::StaleInput
            | RiprProviderResultClassV1::Unsupported
            | RiprProviderResultClassV1::MalformedInput
            | RiprProviderResultClassV1::InstrumentFailure
            | RiprProviderResultClassV1::Cancelled
            | RiprProviderResultClassV1::NotProven => {
                if self.native_status.is_some() {
                    return Err(error(
                        RiprProviderContractErrorCodeV1::CompletenessConflict,
                        "non-authoritative results cannot carry native exposure status",
                    ));
                }
                if self.analysis_complete {
                    return Err(error(
                        RiprProviderContractErrorCodeV1::CompletenessConflict,
                        "non-authoritative result class cannot claim complete analysis",
                    ));
                }
                require_disclosure(self)?;
            }
        }
        Ok(())
    }
}

fn read_only_descriptor(
    capability: RiprProviderCapabilityV1,
) -> RiprProviderCapabilityDescriptorV1 {
    RiprProviderCapabilityDescriptorV1 {
        capability,
        reads_source: capability != RiprProviderCapabilityV1::CapturedRiprReceiptValidation,
        writes_source: false,
        executes_project_commands: false,
        uses_network: false,
        supported_request_schema: RIPR_ANALYSIS_REQUEST_SCHEMA_VERSION.into(),
        supported_receipt_schema: RIPR_ANALYSIS_RECEIPT_SCHEMA_VERSION.into(),
        claim_boundary: RIPR_PROVIDER_CLAIM_BOUNDARY.into(),
        excluded_claims: RIPR_REQUIRED_EXCLUDED_CLAIMS
            .iter()
            .map(|claim| (*claim).to_string())
            .collect(),
    }
}

fn validate_summary(
    summary: &RiprProviderEvidenceSummaryV1,
    expected_seam_id: &str,
    require_denominator: bool,
    allow_empty_related_tests: bool,
) -> Result<(), RiprProviderContractErrorV1> {
    if summary.seam_id != expected_seam_id {
        return Err(error(
            RiprProviderContractErrorCodeV1::IdentityMismatch,
            "summary seam identity does not match the request",
        ));
    }
    if require_denominator && summary.analyzed_subject_count == 0 {
        return Err(error(
            RiprProviderContractErrorCodeV1::CompletenessConflict,
            "a complete result requires a nonzero analyzed-subject denominator",
        ));
    }
    if summary.missing_discriminator_count > summary.analyzed_subject_count {
        return Err(error(
            RiprProviderContractErrorCodeV1::CompletenessConflict,
            "missing discriminators cannot exceed analyzed subjects",
        ));
    }
    if require_denominator && !allow_empty_related_tests && summary.related_tests.is_empty() {
        return Err(error(
            RiprProviderContractErrorCodeV1::CompletenessConflict,
            "a complete result with analyzed subjects requires related test evidence",
        ));
    }
    require_non_empty("summary.strongest_oracle", &summary.strongest_oracle)?;
    require_oracle_strength("summary.strongest_oracle", &summary.strongest_oracle)?;
    require_non_empty("summary.fingerprint", &summary.fingerprint)?;
    for entry in &summary.related_tests {
        require_non_empty("summary.related_tests.test_name", &entry.test_name)?;
        validate_portable_relative_path(
            "summary.related_tests.file",
            &entry.file,
            RiprProviderContractErrorCodeV1::MalformedIdentity,
        )?;
        if entry.line == 0 {
            return Err(error(
                RiprProviderContractErrorCodeV1::MalformedIdentity,
                "summary related-test line must be positive",
            ));
        }
        require_non_empty("summary.related_tests.oracle_kind", &entry.oracle_kind)?;
        require_oracle_kind(&entry.oracle_kind)?;
        require_non_empty(
            "summary.related_tests.oracle_strength",
            &entry.oracle_strength,
        )?;
        require_oracle_strength(
            "summary.related_tests.oracle_strength",
            &entry.oracle_strength,
        )?;
        require_non_empty(
            "summary.related_tests.relation_reason",
            &entry.relation_reason,
        )?;
        require_relation_reason(&entry.relation_reason)?;
    }
    if !summary.related_tests.is_empty() {
        let strongest = summary
            .related_tests
            .iter()
            .filter_map(|entry| {
                require_oracle_strength(
                    "summary.related_tests.oracle_strength",
                    &entry.oracle_strength,
                )
                .ok()
            })
            .max_by_key(OracleStrength::rank)
            .map(|strength| strength.as_str())
            .unwrap_or("none");
        if summary.strongest_oracle != strongest {
            return Err(error(
                RiprProviderContractErrorCodeV1::CompletenessConflict,
                "summary strongest oracle must match the strongest related-test oracle",
            ));
        }
    }
    Ok(())
}

fn require_oracle_kind(value: &str) -> Result<OracleKind, RiprProviderContractErrorV1> {
    [
        OracleKind::ExactValue,
        OracleKind::ExactErrorVariant,
        OracleKind::WholeObjectEquality,
        OracleKind::Snapshot,
        OracleKind::RelationalCheck,
        OracleKind::BroadError,
        OracleKind::SmokeOnly,
        OracleKind::MockExpectation,
        OracleKind::Unknown,
    ]
    .into_iter()
    .find(|kind| kind.as_str() == value)
    .ok_or_else(|| {
        error(
            RiprProviderContractErrorCodeV1::MalformedIdentity,
            "oracle kind is not canonical",
        )
    })
}
fn require_oracle_strength(
    field: &str,
    value: &str,
) -> Result<OracleStrength, RiprProviderContractErrorV1> {
    [
        OracleStrength::Strong,
        OracleStrength::Medium,
        OracleStrength::Weak,
        OracleStrength::Smoke,
        OracleStrength::None,
        OracleStrength::Unknown,
    ]
    .into_iter()
    .find(|strength| strength.as_str() == value)
    .ok_or_else(|| {
        error(
            RiprProviderContractErrorCodeV1::MalformedIdentity,
            format!("{field} is not canonical"),
        )
    })
}
fn require_relation_reason(value: &str) -> Result<RelationReason, RiprProviderContractErrorV1> {
    [
        RelationReason::DirectOwnerCall,
        RelationReason::HelperOwnerCall,
        RelationReason::AssertionTargetAffinity,
        RelationReason::SameTestFile,
        RelationReason::SameModule,
        RelationReason::OwnerNamedTest,
        RelationReason::ImportPathAffinity,
        RelationReason::FixtureOwnerAffinity,
        RelationReason::WeakTokenSubstring,
        RelationReason::ReExportChainFollowed,
    ]
    .into_iter()
    .find(|reason| reason.as_str() == value)
    .ok_or_else(|| {
        error(
            RiprProviderContractErrorCodeV1::MalformedIdentity,
            "relation reason is not canonical",
        )
    })
}
fn validate_diagnostic(
    diagnostic: &RiprProviderDiagnosticV1,
) -> Result<(), RiprProviderContractErrorV1> {
    require_non_empty("diagnostic.code", &diagnostic.code)?;
    require_non_empty("diagnostic.message", &diagnostic.message)?;
    require_optional_non_empty("diagnostic.next_action", diagnostic.next_action.as_deref())?;
    if let Some(path) = &diagnostic.source_path {
        validate_portable_relative_path(
            "diagnostic.source_path",
            path,
            RiprProviderContractErrorCodeV1::MalformedIdentity,
        )?;
    }
    match (diagnostic.start_line, diagnostic.start_column) {
        (None, None) => Ok(()),
        (Some(line), Some(column)) if line > 0 && column > 0 => Ok(()),
        _ => Err(error(
            RiprProviderContractErrorCodeV1::MalformedIdentity,
            "diagnostic source position must be absent or contain positive line and column",
        )),
    }
}

fn require_disclosure(receipt: &RiprAnalysisReceiptV1) -> Result<(), RiprProviderContractErrorV1> {
    if receipt.diagnostics.is_empty() && receipt.limitations.is_empty() {
        return Err(error(
            RiprProviderContractErrorCodeV1::MissingField,
            "a non-authoritative result must disclose a diagnostic or limitation",
        ));
    }
    Ok(())
}

fn require_claim_boundary(value: &str) -> Result<(), RiprProviderContractErrorV1> {
    if value != RIPR_PROVIDER_CLAIM_BOUNDARY {
        return Err(error(
            RiprProviderContractErrorCodeV1::AuthorityViolation,
            "provider claim boundary does not match the canonical contract",
        ));
    }
    Ok(())
}

fn require_excluded_claims(values: &[String]) -> Result<(), RiprProviderContractErrorV1> {
    let claims = values.iter().map(String::as_str).collect::<BTreeSet<_>>();
    if claims.len() != values.len()
        || claims.len() != RIPR_REQUIRED_EXCLUDED_CLAIMS.len()
        || RIPR_REQUIRED_EXCLUDED_CLAIMS
            .iter()
            .any(|required| !claims.contains(required))
    {
        return Err(error(
            RiprProviderContractErrorCodeV1::MissingField,
            "provider contract must retain the complete unique excluded-claims set",
        ));
    }
    Ok(())
}

fn require_git_tree_digest(
    snapshot_id: &str,
    source_digest: &str,
) -> Result<(), RiprProviderContractErrorV1> {
    let digest = Sha256::digest(snapshot_id.as_bytes());
    let expected = format!("sha256:{digest:x}");
    if source_digest != expected {
        return Err(error(
            RiprProviderContractErrorCodeV1::IdentityMismatch,
            "git-tree source_digest must be the SHA-256 digest of the canonical snapshot_id",
        ));
    }
    Ok(())
}

fn require_derived_snapshot(
    prefix: &str,
    snapshot_id: &str,
    source_digest: &str,
) -> Result<(), RiprProviderContractErrorV1> {
    let expected = format!("{prefix}{source_digest}");
    if snapshot_id != expected {
        return Err(error(
            RiprProviderContractErrorCodeV1::IdentityMismatch,
            "snapshot_id must bind the source view to the exact source_digest",
        ));
    }
    Ok(())
}

fn require_sha256(field: &str, value: &str) -> Result<(), RiprProviderContractErrorV1> {
    let digest = value.strip_prefix("sha256:").ok_or_else(|| {
        error(
            RiprProviderContractErrorCodeV1::MalformedIdentity,
            format!("{field} must use sha256:<64-hex>"),
        )
    })?;
    if digest.len() != 64 || !is_hex(digest) {
        return Err(error(
            RiprProviderContractErrorCodeV1::MalformedIdentity,
            format!("{field} must use sha256:<64-hex>"),
        ));
    }
    Ok(())
}

fn is_lower_hex(value: &str) -> bool {
    value
        .bytes()
        .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
}

fn is_hex(value: &str) -> bool {
    value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn validate_portable_relative_path(
    field: &str,
    value: &str,
    error_code: RiprProviderContractErrorCodeV1,
) -> Result<(), RiprProviderContractErrorV1> {
    if value.is_empty()
        || value.starts_with('/')
        || value.starts_with('\\')
        || value.contains('\\')
        || value.split('/').any(|component| {
            component.is_empty() || component == "." || component == ".." || component.contains(':')
        })
    {
        return Err(error(
            error_code,
            format!("{field} must be a portable repository-relative path without traversal"),
        ));
    }
    Ok(())
}

fn validate_text_list(field: &str, values: &[String]) -> Result<(), RiprProviderContractErrorV1> {
    if values.iter().any(|value| value.trim().is_empty()) {
        return Err(error(
            RiprProviderContractErrorCodeV1::MissingField,
            format!("{field} contains an empty entry"),
        ));
    }
    Ok(())
}

fn require_optional_non_empty(
    field: &str,
    value: Option<&str>,
) -> Result<(), RiprProviderContractErrorV1> {
    if value.is_some_and(|value| value.trim().is_empty()) {
        return Err(error(
            RiprProviderContractErrorCodeV1::MissingField,
            format!("{field} is empty"),
        ));
    }
    Ok(())
}

fn require_non_empty(field: &str, value: &str) -> Result<(), RiprProviderContractErrorV1> {
    if value.trim().is_empty() {
        return Err(error(
            RiprProviderContractErrorCodeV1::MissingField,
            format!("{field} is empty"),
        ));
    }
    Ok(())
}

fn error(
    code: RiprProviderContractErrorCodeV1,
    message: impl Into<String>,
) -> RiprProviderContractErrorV1 {
    RiprProviderContractErrorV1 {
        code,
        message: message.into(),
    }
}
