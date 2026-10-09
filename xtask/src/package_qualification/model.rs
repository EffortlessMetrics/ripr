use serde::{Deserialize, Serialize};

pub(crate) const RECEIPT_SCHEMA_VERSION: &str = "1.0";
pub(crate) const RECEIPT_KIND: &str = "package_qualification_receipt";
pub(crate) const GATE_SCHEMA_VERSION: &str = "1.0";
pub(crate) const GATE_KIND: &str = "package_qualification_gate";

pub(crate) const CLAIM_BOUNDARY: &str = "This receipt records no-publish package-qualification evidence for one exact candidate. It does not publish, sign, promote support, authorize a registry, or qualify a different source tree.";

pub(crate) const STANDING_NON_CLAIMS: [&str; 4] = [
    "not a public registry publication",
    "not a support-tier promotion",
    "not a runtime mutation result",
    "not authorization to publish",
];

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct PackageQualificationReceipt {
    pub(crate) schema_version: String,
    pub(crate) kind: String,
    pub(crate) source: SourceIdentity,
    pub(crate) release_identity: ReleaseIdentity,
    pub(crate) package: PackageIdentity,
    pub(crate) payload: PayloadIdentity,
    pub(crate) tools: ToolIdentity,
    pub(crate) subjects: SubjectCounts,
    pub(crate) selection_scope: SelectionScope,
    pub(crate) required_rows: Vec<RowKey>,
    pub(crate) rows: Vec<QualificationRow>,
    pub(crate) executed_steps: Vec<String>,
    pub(crate) elapsed: ElapsedMeasurements,
    pub(crate) cleanup: CleanupResult,
    pub(crate) limitations: Vec<String>,
    pub(crate) non_claims: Vec<String>,
    pub(crate) claim_boundary: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct SourceIdentity {
    pub(crate) commit: String,
    pub(crate) tree: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ReleaseIdentity {
    pub(crate) product: String,
    pub(crate) version: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct PackageIdentity {
    pub(crate) channel: Channel,
    pub(crate) name: String,
    pub(crate) hash: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct PayloadIdentity {
    pub(crate) target: String,
    pub(crate) hash: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ToolIdentity {
    pub(crate) runtime: String,
    pub(crate) package_manager: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct SubjectCounts {
    pub(crate) selected: u64,
    pub(crate) executed: u64,
    pub(crate) failed: u64,
    pub(crate) skipped: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum SelectionScope {
    ExplicitSubset,
    DeclaredFullMatrix,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Channel {
    Pypi,
    Npm,
}

impl Channel {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Pypi => "pypi",
            Self::Npm => "npm",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum RowStatus {
    Passed,
    Failed,
    NotRun,
    UnsupportedByContract,
}

impl RowStatus {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Passed => "passed",
            Self::Failed => "failed",
            Self::NotRun => "not_run",
            Self::UnsupportedByContract => "unsupported_by_contract",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct RowKey {
    pub(crate) channel: Channel,
    pub(crate) target: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct QualificationRow {
    pub(crate) channel: Channel,
    pub(crate) target: String,
    pub(crate) status: RowStatus,
    pub(crate) subject_count: u64,
    pub(crate) package_hash: String,
    pub(crate) payload_hash: String,
    pub(crate) executed_payload_hash: Option<String>,
    pub(crate) steps: Vec<String>,
    pub(crate) limitations: Vec<String>,
    pub(crate) non_claims: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ElapsedMeasurements {
    pub(crate) install_ms: Option<u64>,
    pub(crate) install_condition: Option<String>,
    pub(crate) launcher_ms: Option<u64>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct CleanupResult {
    pub(crate) status: RowStatus,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct ExpectedIdentity {
    pub(crate) commit: Option<String>,
    pub(crate) tree: Option<String>,
    pub(crate) package_hash: Option<String>,
    pub(crate) payload_hash: Option<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum GateVerdict {
    Passed,
    Failed,
}

impl GateVerdict {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Passed => "passed",
            Self::Failed => "failed",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct PackageQualificationGate {
    pub(crate) schema_version: String,
    pub(crate) kind: String,
    pub(crate) verdict: GateVerdict,
    pub(crate) source: SourceIdentity,
    pub(crate) package: PackageIdentity,
    pub(crate) payload: PayloadIdentity,
    pub(crate) selection_scope: SelectionScope,
    pub(crate) subjects: SubjectCounts,
    pub(crate) required_row_results: Vec<RequiredRowResult>,
    pub(crate) failures: Vec<String>,
    pub(crate) limitations: Vec<String>,
    pub(crate) non_claims: Vec<String>,
    pub(crate) claim_boundary: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct RequiredRowResult {
    pub(crate) channel: Channel,
    pub(crate) target: String,
    pub(crate) status: Option<RowStatus>,
    pub(crate) subject_count: Option<u64>,
    pub(crate) disposition: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct GateInput<'a> {
    pub(crate) receipt: &'a PackageQualificationReceipt,
    pub(crate) expected: &'a ExpectedIdentity,
    pub(crate) full_matrix_targets: &'a [String],
}
