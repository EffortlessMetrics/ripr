//! DTOs, closed key sets, and schema-identity constants for driver binding
//! records. Decoding helpers live here; identity agreement with the accepted
//! selection row is owned by `validate`.
//!
//! Shared types stay with their semantic owner in `python_repair_trust`. This
//! module does not copy verification logic or invent a second parser.

use std::collections::BTreeSet;

use super::super::python_repair_trust::KNOWN_SPEC;

pub(crate) const RERUN_COMMAND: &str = "cargo xtask python-repair-trust check-driver";

/// The record schema mirrors the driver's binding record exactly. The crate
/// and xtask share the schema by construction, not by a shared dependency:
/// the digest anchors are the contract.
pub(crate) const BINDING_KIND: &str = "python_repair_driver_binding";
pub(crate) const BINDING_SPEC: &str = KNOWN_SPEC;
pub(crate) const BINDING_SCHEMA_VERSION: &str = "0.1";

pub(crate) const RECORD_KEYS_PREPARE: [&str; 14] = [
    "schema_version",
    "kind",
    "spec",
    "phase",
    "seam_id",
    "repository_head",
    "selection_manifest_path",
    "driver",
    "config",
    "input",
    "trust",
    "edit_surface",
    "authorization",
    "non_claims",
];

pub(crate) const RECORD_KEYS_APPLY: [&str; 17] = [
    "schema_version",
    "kind",
    "spec",
    "phase",
    "seam_id",
    "repository_head",
    "selection_manifest_path",
    "durable_attempt_id",
    "binding_artifact_sha256",
    "driver",
    "config",
    "input",
    "trust",
    "edit_surface",
    "authorization",
    "apply",
    "non_claims",
];

pub(crate) const TRUST_KEYS: [&str; 18] = [
    "attempt_id",
    "case_id",
    "subject_id",
    "repository",
    "base",
    "head",
    "tree",
    "source_currentness",
    "selection_manifest_sha256",
    "selection_digest",
    "target_path",
    "target_state",
    "family",
    "owner",
    "discriminator",
    "relation",
    "oracle",
    "limitation",
];

pub(crate) const DRIVER_KEYS: [&str; 2] = ["binary_sha256", "version"];
pub(crate) const CONFIG_KEYS: [&str; 1] = ["profile"];
pub(crate) const INPUT_KEYS: [&str; 2] = ["packet_sha256", "before_snapshot_sha256"];
pub(crate) const EDIT_SURFACE_KEYS: [&str; 2] = ["allowed", "forbidden"];
pub(crate) const AUTHORIZATION_KEYS: [&str; 3] = ["status", "authority", "method"];
pub(crate) const APPLY_KEYS: [&str; 5] = [
    "patch_sha256",
    "changed_paths",
    "cage_status",
    "repository_head_after",
    "current",
];

pub(crate) const CAGE_STATUSES: [&str; 3] = ["compliant", "violated", "incomparable"];
pub(crate) const AUTHORIZATION_STATUSES: [&str; 1] = ["granted"];
pub(crate) const AUTHORIZATION_METHODS: [&str; 1] = ["explicit-operator-flags"];

/// The only target state a driver binding may declare: the offline check
/// mirrors the producer, which refuses proposed/ambiguous/unavailable/unsafe
/// targets before any attempt exists.
pub(crate) const BINDABLE_TARGET_STATE: &str = "existing";

/// The standing non-claims every driver record must carry verbatim.
pub(crate) const BINDING_NON_CLAIMS: [&str; 3] = [
    "no verification result is claimed by the driver",
    "no static movement is claimed by the driver",
    "no closure is claimed by the driver",
];

/// Fail-closed error text for the driver check: names subject/field/reason
/// plus the driver rerun command (the shared `fail` names the corpus check).
pub(crate) fn driver_fail(subject: &str, field: &str, reason: impl std::fmt::Display) -> String {
    format!(
        "python-repair-trust check-driver failed: subject=`{subject}` field=`{field}`: {reason}
rerun: {RERUN_COMMAND}"
    )
}

/// One validated record, reduced to what the report and the prepare-to-apply
/// digest chain need. The prepare-bound identity fields (seam, head, target,
/// selection digest, input digests, edit surface, authorization) are retained
/// so the chain can require the apply record to agree with ITS prepare record,
/// not merely to name a valid prepare digest.
pub(crate) struct DriverBindingRecord {
    pub(crate) display: String,
    pub(crate) phase: String,
    pub(crate) trust_attempt_id: String,
    pub(crate) target_path: String,
    pub(crate) selection_digest: String,
    pub(crate) seam_id: String,
    pub(crate) repository_head: String,
    pub(crate) packet_sha256: String,
    pub(crate) before_snapshot_sha256: String,
    pub(crate) allowed_surface: BTreeSet<String>,
    pub(crate) forbidden_surface: BTreeSet<String>,
    pub(crate) authorization_status: String,
    pub(crate) authorization_authority: String,
    pub(crate) authorization_method: String,
    /// The apply block's edit-cage decision, verbatim (`None` on prepare
    /// records). `violated` records are typed failed results: the offline
    /// validator accepts them as retained failure evidence while a
    /// `compliant` record must still cover the selected target in-cage.
    pub(crate) cage_status: Option<String>,
    /// The recomputed exact-byte sha256 of the loaded record file.
    pub(crate) artifact_sha256: String,
    /// The apply-phase claim into the prepare-record digest chain.
    pub(crate) binding_artifact_sha256: Option<String>,
}
