use super::source_promotion_validate_resolved_tree::resolved_tree_receipt_is_admissible;
use super::source_promotion_verify::{validate_manifest, validate_preflight};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::Write;
use std::path::{Component, Path, PathBuf};
use std::process::{Command, Stdio};

pub(crate) const SOURCE_PROMOTION_TRUSTED_BUILDER_SUBCOMMAND: &str =
    "write-trusted-builder-receipt";
pub(crate) const SOURCE_PROMOTION_ADMIT_RESOLVED_TREE_SUBCOMMAND: &str = "admit-resolved-tree";
pub(crate) const SOURCE_PROMOTION_CONSTRUCT_EXACT_JOIN_SUBCOMMAND: &str = "construct-exact-join";
pub(crate) const SOURCE_PROMOTION_PUBLISH_CANDIDATE_REF_SUBCOMMAND: &str = "publish-candidate-ref";

const BUILDER_SCHEMA: &str = "ripr.source_promotion_trusted_builder.v1";
const ADMISSION_SCHEMA: &str = "ripr.source_promotion_resolved_tree_admission.v1";
const CONSTRUCTION_SCHEMA: &str = "ripr.source_promotion_exact_join_construction.v1";
const PUBLICATION_SCHEMA: &str = "ripr.source_promotion_candidate_ref_publication.v1";
const CONTROL_PACKET_SCHEMA: &str = "ripr.source_promotion_control_packet.v1";
const RESOLVED_TREE_PACKET_SCHEMA: &str = "ripr.source_promotion_resolved_tree_packet.v1";
const INTEGRATION_INDEX_SCHEMA: &str = "ripr.source_promotion_integration_index.v1";
const QUALIFICATION_SCHEMA: &str = "ripr.source_promotion_tree_qualification.v1";
const SOURCE_REPOSITORY_URL: &str = "https://github.com/EffortlessMetrics/ripr.git";
const SWARM_REPOSITORY_URL: &str = "https://github.com/EffortlessMetrics/ripr-swarm.git";
const SOURCE_MAIN_REF: &str = "refs/heads/main";

const BUILDER_REPORT: &str = "trusted-builder.json";
const ADMISSION_REPORT: &str = "resolved-tree-admission.json";
const CONSTRUCTION_REPORT: &str = "exact-join-construction.json";
const PUBLICATION_REPORT: &str = "candidate-ref-publication.json";
const PACKET_INDEX: &str = "packet-index.json";
const VALIDATION_REPORT: &str = "resolved-tree-validation.json";

const DEFAULT_BUILDER_OUT: &str = "target/ripr/source-promotion/trusted-builder";
const DEFAULT_ADMISSION_OUT: &str = "target/ripr/source-promotion/resolved-tree-admission";
const DEFAULT_CONSTRUCTION_OUT: &str = "target/ripr/source-promotion/exact-join-construction";
const DEFAULT_PUBLICATION_OUT: &str = "target/ripr/source-promotion/candidate-ref-publication";

const RUST_TOOLCHAIN: &str = "1.95.0";
const JOIN_AUTHOR_NAME: &str = "EffortlessSteven";
const JOIN_AUTHOR_EMAIL: &str = "git@effortlesssteven.com";
const JOIN_MESSAGE: &str = "promote: join ripr source with frozen W7 for 0.11.0\n\nConstruct the exact qualified reviewed tree with source as parent 1\nand frozen W7 as parent 2. No release publication authority.";
const REQUIRED_INTEGRATION_KINDS: &[&str] =
    &["command_catalog_integration", "network_policy_integration"];
const REQUIRED_QUALIFICATION_LANES: &[&str] = &[
    "editor_package_linux",
    "editor_package_windows",
    "rust_product",
    "source_governance",
    "source_survivors",
    "trusted_product_journeys",
    "untrusted_workspace_contract",
    "w7_product",
];

fn validate_source_main_ref(reference: &str) -> Result<(), String> {
    validate_full_ref(reference, "source main ref")?;
    if reference != SOURCE_MAIN_REF {
        return Err(format!(
            "source main ref must be the exact protected ref {SOURCE_MAIN_REF}"
        ));
    }
    Ok(())
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct PromotionIdentity {
    source_parent: String,
    swarm_parent: String,
    join_tree: String,
    preflight_sha256: String,
    resolution_sha256: String,
}

impl PromotionIdentity {
    fn from_values(values: &ParsedArgs) -> Result<Self, String> {
        let identity = Self {
            source_parent: values.required("--source-parent")?,
            swarm_parent: values.required("--swarm-parent")?,
            join_tree: values.required("--join-tree")?,
            preflight_sha256: values.required("--preflight-sha256")?,
            resolution_sha256: values.required("--resolution-sha256")?,
        };
        validate_exact_hex("--source-parent", &identity.source_parent, 40)?;
        validate_exact_hex("--swarm-parent", &identity.swarm_parent, 40)?;
        validate_exact_hex("--join-tree", &identity.join_tree, 40)?;
        validate_exact_hex("--preflight-sha256", &identity.preflight_sha256, 64)?;
        validate_exact_hex("--resolution-sha256", &identity.resolution_sha256, 64)?;
        Ok(identity)
    }

    fn matches_json(&self, value: &Value) -> bool {
        json_string(value, "source_parent") == Some(self.source_parent.as_str())
            && json_string(value, "swarm_parent") == Some(self.swarm_parent.as_str())
            && json_string(value, "join_tree") == Some(self.join_tree.as_str())
            && json_string(value, "preflight_sha256") == Some(self.preflight_sha256.as_str())
            && json_string(value, "resolution_manifest_sha256")
                == Some(self.resolution_sha256.as_str())
    }

    fn as_json(&self) -> Value {
        serde_json::json!({
            "source_parent": self.source_parent,
            "swarm_parent": self.swarm_parent,
            "join_tree": self.join_tree,
            "preflight_sha256": self.preflight_sha256,
            "resolution_manifest_sha256": self.resolution_sha256,
        })
    }
}

#[derive(Clone, Debug)]
struct ParsedArgs {
    values: BTreeMap<String, String>,
    flags: BTreeSet<String>,
}

impl ParsedArgs {
    fn required(&self, key: &str) -> Result<String, String> {
        self.values
            .get(key)
            .cloned()
            .filter(|value| !value.trim().is_empty())
            .ok_or_else(|| format!("missing {key}"))
    }

    fn optional(&self, key: &str) -> Option<String> {
        self.values
            .get(key)
            .cloned()
            .filter(|value| !value.trim().is_empty())
    }

    fn has_flag(&self, key: &str) -> bool {
        self.flags.contains(key)
    }
}

#[derive(Clone, Debug)]
struct BuilderOptions {
    repo: PathBuf,
    source_parent: String,
    workflow_source_sha: String,
    executable: PathBuf,
    cargo_target_dir: PathBuf,
    out: PathBuf,
    locked_build: bool,
    isolated_target_dir: bool,
}

#[derive(Clone, Debug)]
struct AdmissionOptions {
    repo: PathBuf,
    identity: PromotionIdentity,
    validation_packet: PathBuf,
    builder_packet: PathBuf,
    integration_index: PathBuf,
    integration_index_sha256: String,
    preflight: PathBuf,
    resolution_manifest: PathBuf,
    out: PathBuf,
}

#[derive(Clone, Debug)]
struct ConstructionOptions {
    repo: PathBuf,
    admission_packet: PathBuf,
    validation_packet: PathBuf,
    integration_index: PathBuf,
    integration_index_sha256: String,
    preflight: PathBuf,
    resolution_manifest: PathBuf,
    qualification_receipt: PathBuf,
    qualification_receipt_sha256: String,
    source_main_ref: String,
    swarm_ref: String,
    candidate_ref: String,
    out: PathBuf,
}

#[derive(Clone, Debug)]
struct PublicationOptions {
    repo: PathBuf,
    construction_packet: PathBuf,
    source_main_ref: String,
    remote: String,
    source_remote_url: String,
    swarm_remote_url: String,
    target_ref: String,
    expected_old: Option<String>,
    expected_absent: bool,
    out: PathBuf,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct IndexedFile {
    sha256: String,
    contents: Vec<u8>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct IndexedPacket {
    index_sha256: String,
    files: BTreeMap<String, IndexedFile>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct AdmissionSnapshot {
    source_head: String,
    swarm_head: String,
    join_tree: String,
    preflight_sha256: String,
    resolution_sha256: String,
    validation_index_sha256: String,
    builder_index_sha256: String,
    integration_index_sha256: String,
    executable_sha256: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ConstructionSnapshot {
    source_head: String,
    swarm_head: String,
    join_tree: String,
    preflight_sha256: String,
    resolution_sha256: String,
    validation_index_sha256: String,
    admission_index_sha256: String,
    integration_index_sha256: String,
    qualification_sha256: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct IntegrationEvidence {
    index_sha256: String,
    receipt_digests: BTreeMap<String, String>,
}

#[derive(Clone, Debug)]
struct AdmissionEvidence {
    identity: PromotionIdentity,
    swarm_ref: String,
    validation_index_sha256: String,
    validation_receipt_sha256: String,
    builder_index_sha256: String,
    builder_receipt_sha256: String,
    integration: IntegrationEvidence,
    executable_sha256: String,
}

#[derive(Clone, Debug)]
struct ConstructionEvidence {
    identity: PromotionIdentity,
    swarm_ref: String,
    candidate_ref: String,
    admission_index_sha256: String,
    admission_receipt_sha256: String,
    validation_index_sha256: String,
    integration_index_sha256: String,
    qualification_sha256: String,
    join_commit: String,
    commit_timestamp: String,
}

#[derive(Clone, Debug, Default)]
struct PublicationState {
    local_ref_attempts: u64,
    remote_push_attempts: u64,
    merge_command_attempts: u64,
    local_ref_before: Option<String>,
    local_ref_after: Option<String>,
    observed_final_ref: Option<String>,
    remote_state_observed: bool,
    local_ref_rollback_succeeded: Option<bool>,
    push_process_succeeded: Option<bool>,
    target_ref_updated: Option<bool>,
    source_main_unchanged: Option<bool>,
    swarm_parent_unchanged: Option<bool>,
    construction_packet_unchanged: Option<bool>,
    remote_authority_unchanged: Option<bool>,
}

type ConstructionFailure = (String, Option<Box<PromotionIdentity>>, bool);
type PublicationFailure = (
    String,
    Option<Box<ConstructionEvidence>>,
    Box<PublicationState>,
);

pub(crate) fn source_promotion_control_handles(args: &[String]) -> bool {
    matches!(
        args.first().map(String::as_str),
        Some(SOURCE_PROMOTION_TRUSTED_BUILDER_SUBCOMMAND)
            | Some(SOURCE_PROMOTION_ADMIT_RESOLVED_TREE_SUBCOMMAND)
            | Some(SOURCE_PROMOTION_CONSTRUCT_EXACT_JOIN_SUBCOMMAND)
            | Some(SOURCE_PROMOTION_PUBLISH_CANDIDATE_REF_SUBCOMMAND)
    )
}

pub(crate) fn source_promotion_control(args: &[String]) -> Result<(), String> {
    match args.first().map(String::as_str) {
        Some(SOURCE_PROMOTION_TRUSTED_BUILDER_SUBCOMMAND) => write_trusted_builder_receipt(args),
        Some(SOURCE_PROMOTION_ADMIT_RESOLVED_TREE_SUBCOMMAND) => admit_resolved_tree(args),
        Some(SOURCE_PROMOTION_CONSTRUCT_EXACT_JOIN_SUBCOMMAND) => construct_exact_join(args),
        Some(SOURCE_PROMOTION_PUBLISH_CANDIDATE_REF_SUBCOMMAND) => publish_candidate_ref(args),
        _ => Err(control_usage()),
    }
}

fn control_usage() -> String {
    [
        "usage:",
        "  cargo xtask source-promotion write-trusted-builder-receipt --source-parent <sha> --workflow-source-sha <sha> --executable <path> --cargo-target-dir <path> --locked-build --isolated-target-dir [--out <dir>]",
        "  cargo xtask source-promotion admit-resolved-tree --source-parent <sha> --swarm-parent <sha> --join-tree <tree> --preflight <path> --preflight-sha256 <digest> --resolution-manifest <path> --resolution-sha256 <digest> --validation-packet <dir> --builder-packet <dir> --integration-index <path> --integration-index-sha256 <digest> [--out <dir>]",
        "  cargo xtask source-promotion construct-exact-join --admission-packet <dir> --validation-packet <dir> --integration-index <path> --integration-index-sha256 <digest> --preflight <path> --resolution-manifest <path> --qualification-receipt <path> --qualification-receipt-sha256 <digest> --source-main-ref <ref> --swarm-ref <ref> --candidate-ref <ref> [--out <dir>]",
        "  cargo xtask source-promotion publish-candidate-ref --construction-packet <dir> --source-main-ref <ref> --remote origin --target-ref <refs/heads/promote/0.11.0-...> (--expected-absent | --expected-old <sha>) [--out <dir>]",
    ]
    .join("\n")
}
