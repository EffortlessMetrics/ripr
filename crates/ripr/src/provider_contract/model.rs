use serde::{Deserialize, Serialize};

use crate::domain::ExposureClass;

pub const RIPR_PROVIDER_CAPABILITY_SCHEMA_VERSION: &str = "ripr_provider_capabilities.v1";
pub const RIPR_ANALYSIS_REQUEST_SCHEMA_VERSION: &str = "ripr_analysis_request.v1";
pub const RIPR_ANALYSIS_RECEIPT_SCHEMA_VERSION: &str = "ripr_analysis_receipt.v1";
pub const RIPR_PROVIDER_CLAIM_BOUNDARY: &str = "Static RIPR evidence for one exact source snapshot and selected seam; no runtime test execution, mutation adequacy, semantic correctness, or merge-readiness claim.";
pub const RIPR_REQUIRED_EXCLUDED_CLAIMS: [&str; 4] = [
    "runtime_test_execution",
    "mutation_adequacy",
    "semantic_correctness",
    "merge_readiness",
];

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RiprProviderCapabilityV1 {
    TestGripSummary,
    ActivationPropagationObservableAnalysis,
    RequiredDiscriminatorAnalysis,
    OracleScopeAnalysis,
    SelectedSeamEvidenceSummary,
    CapturedRiprReceiptValidation,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RiprProviderCapabilityDescriptorV1 {
    pub capability: RiprProviderCapabilityV1,
    pub reads_source: bool,
    pub writes_source: bool,
    pub executes_project_commands: bool,
    pub uses_network: bool,
    pub supported_request_schema: String,
    pub supported_receipt_schema: String,
    pub claim_boundary: String,
    pub excluded_claims: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RiprProviderCapabilitySetV1 {
    pub schema_version: String,
    pub provider: String,
    pub provider_version: String,
    pub capabilities: Vec<RiprProviderCapabilityDescriptorV1>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RiprSourceViewV1 {
    GitTree,
    GitIndex,
    Worktree,
    CapturedSourceSet,
}

/// Exact repository source snapshot selected by an external orchestrator.
///
/// This DTO binds identities supplied to RIPR. Resolving a Git object against a
/// repository remains the provider operation's responsibility rather than a
/// deserialization or receipt-validation side effect.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RiprRepositorySnapshotV1 {
    pub repository_id: String,
    /// Canonical source-view identity. Git trees use
    /// `git-tree:<40-or-64-lowercase-hex-object-id>`; derived views embed their
    /// SHA-256 source digest after the view prefix.
    pub snapshot_id: String,
    pub source_view: RiprSourceViewV1,
    /// Portable SHA-256 identity binding. For a Git tree, hash the exact UTF-8
    /// bytes of the canonical `snapshot_id`, including the `git-tree:` prefix.
    pub source_digest: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RiprEvidenceSubjectV1 {
    pub requirement_id: Option<String>,
    pub evidence_purpose: Option<String>,
    pub seam_id: String,
    pub subject_id: String,
    pub subject_body_digest: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RiprAnalysisRequestV1 {
    pub schema_version: String,
    pub request_id: String,
    pub capability: RiprProviderCapabilityV1,
    pub repository: RiprRepositorySnapshotV1,
    pub subject: RiprEvidenceSubjectV1,
    pub analysis_mode: String,
    pub profile: String,
    pub config_digest: String,
    pub analyzer_generation: String,
    pub output_root: String,
    pub requested_claim: String,
}

pub type RiprProviderNativeStatusV1 = ExposureClass;

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RiprProviderResultClassV1 {
    Completed,
    Findings,
    Partial,
    StaleInput,
    Unsupported,
    MalformedInput,
    InstrumentFailure,
    Cancelled,
    NotProven,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RiprProviderDiagnosticV1 {
    pub code: String,
    pub message: String,
    pub source_path: Option<String>,
    pub start_line: Option<u32>,
    pub start_column: Option<u32>,
    pub next_action: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RiprProviderEvidenceEntryV1 {
    pub test_name: String,
    pub file: String,
    pub line: u64,
    pub oracle_kind: String,
    pub oracle_strength: String,
    pub relation_reason: String,
    pub has_test_target: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RiprProviderEvidenceSummaryV1 {
    pub seam_id: String,
    pub analyzed_subject_count: u64,
    pub related_tests: Vec<RiprProviderEvidenceEntryV1>,
    pub missing_discriminator_count: u64,
    pub strongest_oracle: String,
    pub fingerprint: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RiprAnalysisReceiptV1 {
    pub schema_version: String,
    pub request: RiprAnalysisRequestV1,
    pub provider_version: String,
    pub binary_digest: String,
    pub analyzer_generation: String,
    pub result_class: RiprProviderResultClassV1,
    pub native_status: Option<RiprProviderNativeStatusV1>,
    pub analysis_complete: bool,
    pub truncated: bool,
    pub summary: Option<RiprProviderEvidenceSummaryV1>,
    pub diagnostics: Vec<RiprProviderDiagnosticV1>,
    pub limitations: Vec<String>,
    pub claim_boundary: String,
    pub excluded_claims: Vec<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RiprProviderContractErrorCodeV1 {
    MissingField,
    UnsupportedSchema,
    DuplicateCapability,
    AuthorityViolation,
    UnsafeOutputRoot,
    IdentityMismatch,
    CompletenessConflict,
    MalformedIdentity,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RiprProviderContractErrorV1 {
    pub code: RiprProviderContractErrorCodeV1,
    pub message: String,
}

impl std::fmt::Display for RiprProviderContractErrorV1 {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{:?}: {}", self.code, self.message)
    }
}

impl std::error::Error for RiprProviderContractErrorV1 {}
