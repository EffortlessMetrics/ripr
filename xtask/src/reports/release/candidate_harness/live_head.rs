//! Direct #1609 manifest custody. This is not the deferred lifecycle registry.
//! A producer-created file is not admission: the caller must supply the exact
//! manifest digest accepted by #1609, independently of the file being read.
pub(crate) mod handoff;
mod input;
use super::safe_artifact_path;
use input::{MAX_RETAINED_BYTES, read_owned};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

const SCHEMA: &str = "1.1";
const KIND: &str = "ripr_swarm_live_head_release_authority";
const REPOSITORY: &str = "EffortlessMetrics/ripr-swarm";
const LAST_INTEGRATED: &str = "45b56c0957ad7e7360114edceca4b844c85f846e";
const MAX_PROOF_INPUTS: usize = 64;

/// One manifest owns both projections. None-valued transaction fields belong
/// only to the committed template; `admit` never grants that template authority.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct LiveHeadManifest {
    schema_version: String,
    kind: String,
    release_line: String,
    authority_issue: u64,
    candidate_owner_issue: u64,
    status: String,
    candidate: Option<Candidate>,
    range: Option<RangeIdentity>,
    prerequisites: Option<Prerequisites>,
    pin: Option<PinEvidence>,
    qualification: Qualification,
    source_parent: Option<String>,
    non_claims: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct Candidate {
    repository: String,
    sha: String,
    tree: String,
    #[serde(rename = "ref")]
    git_ref: String,
    package: Package,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct Package {
    name: String,
    version: String,
    workspace_manifest_sha256: String,
    package_manifest_sha256: String,
    lock_sha256: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct RangeIdentity {
    last_integrated_swarm_parent: String,
    all_reachable_count: u64,
    first_parent_count: u64,
    all_reachable_sha256: String,
    first_parent_sha256: String,
    record_set_sha256: String,
}

/// References to already accepted owner packets, not a second interpretation
/// of their schemas or a self-issued acceptance decision. #1609 reviews those
/// packets before accepting the complete manifest's raw digest.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct Evidence {
    owner_issue: u64,
    path: String,
    sha256: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct Prerequisites {
    selected_claims: AcceptedEvidence,
    denominator: AcceptedEvidence,
    audit: AcceptedEvidence,
}

/// Minimal #1609 status envelope over an existing owner's independently
/// reviewed raw packet. This is supplied by the trusted controller; admission
/// checks its bindings but does not re-prove or issue the underlying judgment.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct AcceptedEvidence {
    packet: Evidence,
    acceptance: OwnerAcceptance,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct OwnerAcceptance {
    status: String,
    candidate_sha: String,
    candidate_tree: String,
    reviewed_packet_sha256: String,
    decision_ref: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct PinEvidence {
    remote_ref_readback: Evidence,
    ruleset: Evidence,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct Qualification {
    state: String,
    required_execution_owners: Vec<u64>,
    proof_inputs: Vec<Evidence>,
}

/// Private admitted state retains raw accepted inputs and can only be made by
/// the digest-pinned constructor. It does not select, approve or create a pin.
pub(crate) struct LiveHeadSnapshot {
    root: PathBuf,
    artifact_path: PathBuf,
    approved_digest: String,
    manifest: LiveHeadManifest,
    raw: BTreeMap<String, Vec<u8>>,
}

impl LiveHeadSnapshot {
    pub(crate) fn admit(
        root: &Path,
        version: &str,
        artifact_path: &Path,
        approved_digest: &str,
    ) -> Result<Self, String> {
        require_hex(
            "independently accepted manifest digest",
            approved_digest,
            64,
        )?;
        let root = root
            .canonicalize()
            .map_err(|error| format!("resolve direct-manifest controller root: {error}"))?;
        let path = artifact_path
            .to_str()
            .ok_or_else(|| "direct manifest path must be UTF-8".to_string())?;
        let bytes = read_owned(&root, path, MAX_RETAINED_BYTES)?;
        let mut remaining_bytes = MAX_RETAINED_BYTES - bytes.len() as u64;
        if digest(&bytes) != approved_digest {
            return Err(
                "direct manifest differs from the independently accepted raw digest".to_string(),
            );
        }
        let manifest: LiveHeadManifest = serde_json::from_slice(&bytes)
            .map_err(|error| format!("parse direct #1609 manifest: {error}"))?;
        validate_manifest(&manifest, version)?;
        let mut raw = BTreeMap::from([(path.to_string(), bytes)]);
        let prerequisites = manifest
            .prerequisites
            .as_ref()
            .ok_or_else(|| "direct manifest has no accepted prerequisites".to_string())?;
        let pin = manifest
            .pin
            .as_ref()
            .ok_or_else(|| "direct manifest has no pin evidence".to_string())?;
        let references = [
            &prerequisites.selected_claims.packet,
            &prerequisites.denominator.packet,
            &prerequisites.audit.packet,
            &pin.remote_ref_readback,
            &pin.ruleset,
        ];
        for reference in references
            .into_iter()
            .chain(manifest.qualification.proof_inputs.iter())
        {
            if raw.contains_key(&reference.path) {
                return Err(format!(
                    "direct manifest repeats an evidence path: {}",
                    reference.path
                ));
            }
            let bytes = read_owned(&root, &reference.path, remaining_bytes)?;
            remaining_bytes -= bytes.len() as u64;
            if digest(&bytes) != reference.sha256 {
                return Err(format!(
                    "direct manifest evidence digest changed: {}",
                    reference.path
                ));
            }
            raw.insert(reference.path.clone(), bytes);
        }
        let candidate = manifest
            .candidate
            .as_ref()
            .ok_or_else(|| "direct manifest has no candidate".to_string())?;
        let readback = raw
            .get(&pin.remote_ref_readback.path)
            .ok_or_else(|| "direct manifest remote readback missing".to_string())?;
        if readback.as_slice() != format!("{}\n", candidate.sha).as_bytes() {
            return Err("direct manifest remote readback names another candidate".to_string());
        }
        validate_ruleset(
            raw.get(&pin.ruleset.path)
                .ok_or_else(|| "direct manifest ruleset bytes missing".to_string())?,
        )?;
        Ok(Self {
            root,
            artifact_path: artifact_path.to_path_buf(),
            approved_digest: approved_digest.to_string(),
            manifest,
            raw,
        })
    }

    pub(crate) fn root(&self) -> &Path {
        &self.root
    }
    pub(crate) fn candidate_sha(&self) -> Result<&str, String> {
        Ok(&self.candidate()?.sha)
    }
    pub(crate) fn candidate_tree(&self) -> Result<&str, String> {
        Ok(&self.candidate()?.tree)
    }
    pub(crate) fn candidate_ref(&self) -> Result<&str, String> {
        Ok(&self.candidate()?.git_ref)
    }

    fn candidate(&self) -> Result<&Candidate, String> {
        self.manifest
            .candidate
            .as_ref()
            .ok_or_else(|| "admitted manifest lost its candidate".to_string())
    }

    pub(crate) fn revalidate(&self) -> Result<(), String> {
        let fresh = Self::admit(
            &self.root,
            &self.manifest.release_line,
            &self.artifact_path,
            &self.approved_digest,
        )?;
        if fresh.raw != self.raw || fresh.manifest != self.manifest {
            return Err("direct manifest custody changed after admission".to_string());
        }
        Ok(())
    }

    pub(crate) fn verify_ranges(&self, source: &Path) -> Result<(), String> {
        let expected = self
            .manifest
            .range
            .as_ref()
            .ok_or_else(|| "direct manifest range missing".to_string())?;
        let sha = self.candidate_sha()?;
        let base = &expected.last_integrated_swarm_parent;
        super::source::git_bytes(source, &["merge-base", "--is-ancestor", base, sha])?;
        let actual = crate::reports::source_promotion::commit_range_with(base, sha, |args| {
            String::from_utf8(super::source::git_bytes(source, args)?)
                .map_err(|error| format!("candidate range UTF-8: {error}"))
        })?;
        if actual.all_reachable_count as u64 != expected.all_reachable_count
            || actual.first_parent_count as u64 != expected.first_parent_count
            || actual.all_reachable_sha256 != format!("sha256:{}", expected.all_reachable_sha256)
            || actual.first_parent_ordered_sha256
                != format!("sha256:{}", expected.first_parent_sha256)
        {
            return Err(
                "direct manifest Git range counts or ordered digests differ from admitted source"
                    .to_string(),
            );
        }
        Ok(())
    }

    pub(crate) fn verify_package_inputs(
        &self,
        package_name: &str,
        blobs: &BTreeMap<String, Vec<u8>>,
    ) -> Result<(), String> {
        let package = &self.candidate()?.package;
        if package_name != package.name {
            return Err("direct manifest package differs from actual source package".to_string());
        }
        for (path, expected) in [
            ("Cargo.toml", &package.workspace_manifest_sha256),
            ("crates/ripr/Cargo.toml", &package.package_manifest_sha256),
            ("Cargo.lock", &package.lock_sha256),
        ] {
            let bytes = blobs
                .get(path)
                .ok_or_else(|| format!("direct manifest source input missing: {path}"))?;
            if digest(bytes) != *expected {
                return Err(format!("direct manifest source input differs: {path}"));
            }
        }
        Ok(())
    }

    pub(crate) fn custody_json(&self) -> serde_json::Value {
        serde_json::json!({
            "authority_kind": "direct_1609_live_head_manifest",
            "controller_root": self.root,
            "manifest": self.artifact_path,
            "accepted_manifest_sha256": self.approved_digest,
            "candidate": self.manifest.candidate,
            "range": self.manifest.range,
            "range_claim": "Git counts and ordered SHA+LF digests checked at source boundaries; record_set_sha256 remains an owner-reviewed packet claim",
            "prerequisites": self.manifest.prerequisites,
            "pin": self.manifest.pin,
            "qualification": self.manifest.qualification,
            "claim": "exact retained input custody; acceptance comes from #1609, not this producer; no authenticated or atomic provenance"
        })
    }
}

fn validate_manifest(manifest: &LiveHeadManifest, version: &str) -> Result<(), String> {
    if manifest.schema_version != SCHEMA
        || manifest.kind != KIND
        || manifest.authority_issue != 2379
        || manifest.candidate_owner_issue != 1609
        || manifest.release_line != "0.11.0"
        || manifest.release_line != version
    {
        return Err(
            "direct manifest schema, authority or release identity is unsupported".to_string(),
        );
    }
    if manifest.status != "pinned_exact_head" {
        return Err(
            "direct manifest is not pinned_exact_head; templates and history cannot qualify"
                .to_string(),
        );
    }
    if manifest.source_parent.is_some() {
        return Err(
            "swarm manifest must not predict SOURCE_PARENT before source preflight".to_string(),
        );
    }
    let candidate = manifest
        .candidate
        .as_ref()
        .ok_or_else(|| "direct manifest candidate binding missing".to_string())?;
    require_hex("candidate SHA", &candidate.sha, 40)?;
    require_hex("candidate tree", &candidate.tree, 40)?;
    if candidate.repository != REPOSITORY
        || candidate.git_ref != format!("refs/tags/ripr-release-{}-{}", version, candidate.sha)
        || candidate.package.name != "ripr"
        || candidate.package.version != version
    {
        return Err("direct manifest repository/ref/package identity disagrees".to_string());
    }
    for hash in [
        &candidate.package.workspace_manifest_sha256,
        &candidate.package.package_manifest_sha256,
        &candidate.package.lock_sha256,
    ] {
        require_hex("package input digest", hash, 64)?;
    }
    let range = manifest
        .range
        .as_ref()
        .ok_or_else(|| "direct manifest denominator identity missing".to_string())?;
    if range.last_integrated_swarm_parent != LAST_INTEGRATED
        || range.first_parent_count == 0
        || range.all_reachable_count < range.first_parent_count
    {
        return Err("direct manifest denominator is empty or has the wrong boundary".to_string());
    }
    for hash in [
        &range.all_reachable_sha256,
        &range.first_parent_sha256,
        &range.record_set_sha256,
    ] {
        require_hex("denominator digest", hash, 64)?;
    }
    let prerequisites = manifest
        .prerequisites
        .as_ref()
        .ok_or_else(|| "direct manifest accepted prerequisite bindings missing".to_string())?;
    for (owner, evidence) in [
        (2766, &prerequisites.selected_claims),
        (2768, &prerequisites.denominator),
        (3807, &prerequisites.audit),
    ] {
        validate_accepted_evidence(evidence, owner, candidate)?;
    }
    let pin = manifest
        .pin
        .as_ref()
        .ok_or_else(|| "direct manifest pin bindings missing".to_string())?;
    validate_evidence(&pin.remote_ref_readback, 1609)?;
    validate_evidence(&pin.ruleset, 1609)?;
    if manifest.qualification.state != "required_not_run"
        || manifest.qualification.required_execution_owners.is_empty()
        || manifest.qualification.proof_inputs.is_empty()
        || manifest.non_claims.is_empty()
    {
        return Err(
            "direct manifest must retain nonempty proof inputs and required_not_run executions"
                .to_string(),
        );
    }
    if manifest.qualification.proof_inputs.len() > MAX_PROOF_INPUTS {
        return Err(format!(
            "direct manifest proof_inputs exceeds {MAX_PROOF_INPUTS}-entry budget"
        ));
    }
    let owners = &manifest.qualification.required_execution_owners;
    let unique: std::collections::BTreeSet<_> = owners.iter().collect();
    if unique.len() != owners.len() || owners.contains(&0) {
        return Err("direct manifest repeats or omits an execution owner".to_string());
    }
    if !manifest
        .qualification
        .proof_inputs
        .iter()
        .any(|input| input.owner_issue == 4510)
        || (owners.contains(&4604)
            && !manifest
                .qualification
                .proof_inputs
                .iter()
                .any(|input| input.owner_issue == 4603))
    {
        return Err(
            "direct manifest lacks its installed-custody or selected blind contract input"
                .to_string(),
        );
    }
    for evidence in &manifest.qualification.proof_inputs {
        validate_evidence(evidence, evidence.owner_issue)?;
    }
    Ok(())
}

fn validate_accepted_evidence(
    reference: &AcceptedEvidence,
    owner: u64,
    candidate: &Candidate,
) -> Result<(), String> {
    validate_evidence(&reference.packet, owner)?;
    let acceptance = &reference.acceptance;
    let prefix =
        format!("https://github.com/EffortlessMetrics/ripr-swarm/issues/{owner}#issuecomment-");
    let decision_id = acceptance
        .decision_ref
        .strip_prefix(&prefix)
        .unwrap_or_default();
    if acceptance.status != "accepted"
        || acceptance.candidate_sha != candidate.sha
        || acceptance.candidate_tree != candidate.tree
        || acceptance.reviewed_packet_sha256 != reference.packet.sha256
        || decision_id.is_empty()
        || !decision_id.bytes().all(|byte| byte.is_ascii_digit())
        || decision_id.bytes().all(|byte| byte == b'0')
    {
        return Err(format!(
            "direct manifest owner #{owner} acceptance is not_established for this exact packet and candidate"
        ));
    }
    Ok(())
}

fn validate_evidence(evidence: &Evidence, owner: u64) -> Result<(), String> {
    if owner == 0 || evidence.owner_issue != owner || !safe_artifact_path(Path::new(&evidence.path))
    {
        return Err("direct manifest evidence owner/path is invalid".to_string());
    }
    require_hex("evidence digest", &evidence.sha256, 64)
}

fn validate_ruleset(bytes: &[u8]) -> Result<(), String> {
    let value: serde_json::Value = serde_json::from_slice(bytes)
        .map_err(|error| format!("parse retained pin ruleset: {error}"))?;
    let includes = serde_json::json!(["refs/tags/ripr-release-*"]);
    let no_bypass = serde_json::json!([]);
    let has_rule = |name| {
        value
            .get("rules")
            .and_then(serde_json::Value::as_array)
            .is_some_and(|rules| {
                rules
                    .iter()
                    .any(|rule| rule.get("type").and_then(serde_json::Value::as_str) == Some(name))
            })
    };
    if value.get("name").and_then(serde_json::Value::as_str) != Some("release-transaction-pins")
        || value.get("target").and_then(serde_json::Value::as_str) != Some("tag")
        || value.get("enforcement").and_then(serde_json::Value::as_str) != Some("active")
        || value.pointer("/conditions/ref_name/include") != Some(&includes)
        || value.pointer("/conditions/ref_name/exclude") != Some(&no_bypass)
        || value.get("bypass_actors") != Some(&no_bypass)
        || !has_rule("update")
        || !has_rule("deletion")
    {
        return Err(
            "retained pin ruleset does not protect the exact tag namespace without bypass"
                .to_string(),
        );
    }
    Ok(())
}

pub(super) fn require_hex(label: &str, value: &str, length: usize) -> Result<(), String> {
    if value.len() != length
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(format!(
            "{label} must be {length} lowercase hexadecimal characters"
        ));
    }
    Ok(())
}

fn digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

#[cfg(test)]
pub(crate) mod tests;
