//! Consumed native acceptance at the existing source-promotion boundary.
//!
//! Custody alone is preparation evidence. Only independently retrieved owner
//! decisions admit that same manifest and complete qualification bundle here.
mod native;
use super::{AcceptedEvidence, Evidence, LiveHeadSnapshot, digest, input::read_owned, require_hex};
use native::{NativeDecision, read_native};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

const MAX_BUNDLE_BYTES: u64 = 64 * 1024 * 1024;
const MAX_ROWS: usize = 256;

#[derive(Clone, Debug)]
pub(crate) struct HandoffInput {
    pub(crate) controller_root: PathBuf,
    pub(crate) manifest: PathBuf,
    pub(crate) selection_decision: String,
    pub(crate) qualification_bundle: PathBuf,
    pub(crate) qualification_decision: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct Subject {
    candidate_sha: String,
    candidate_tree: String,
    candidate_ref: String,
    manifest_sha256: String,
}

#[derive(Clone, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(deny_unknown_fields)]
struct RequiredRow {
    id: String,
    owner_issue: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct ExcludedSubjects {
    id: String,
    owner_issue: u64,
    count: u64,
    disposition: String,
    reason: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct SelectionAcceptance {
    schema_version: u32,
    kind: String,
    status: String,
    subject: Subject,
    selected_claims: AcceptedEvidence,
    selected_claims_decision_sha256: String,
    required_execution_owners: Vec<u64>,
    excluded_subjects: Vec<ExcludedSubjects>,
    proof_inputs: Vec<Evidence>,
    required_qualification_rows: Vec<RequiredRow>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct QualificationAcceptance {
    schema_version: u32,
    kind: String,
    status: String,
    subject: Subject,
    selection_decision: String,
    selection_decision_sha256: String,
    qualification_bundle_sha256: String,
    required_qualification_rows: Vec<RequiredRow>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct QualificationBundle {
    schema_version: u32,
    kind: String,
    status: String,
    subject: Subject,
    selection_decision: String,
    selection_decision_sha256: String,
    excluded_subjects: Vec<ExcludedSubjects>,
    rows: Vec<QualifiedRow>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct QualifiedRow {
    id: String,
    owner_issue: u64,
    status: String,
    selected: u64,
    executed: u64,
    failed: u64,
    skipped: u64,
    packet: Evidence,
}

/// No public constructor: the production receipt comes only from admission.
#[derive(Clone, Debug, Serialize)]
pub(crate) struct HandoffReceipt {
    schema: String,
    subject: Subject,
    selection: NativeDecision,
    selected_claims: NativeDecision,
    qualification: NativeDecision,
    qualification_bundle_sha256: String,
    required_execution_owners: Vec<u64>,
    excluded_subjects: Vec<ExcludedSubjects>,
    proof_inputs: Vec<Evidence>,
    rows: Vec<QualifiedRow>,
    // Original admitted bytes, never reconstructed JSON or checkout substitutes.
    #[serde(serialize_with = "serialize_bytes_hex")]
    manifest_bytes: Vec<u8>,
    #[serde(serialize_with = "serialize_bytes_hex")]
    qualification_bundle_bytes: Vec<u8>,
    packet_inputs: Vec<RetainedPacket>,
    claim: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(deny_unknown_fields)]
struct RetainedPacket {
    path: String,
    sha256: String,
    #[serde(serialize_with = "serialize_bytes_hex")]
    bytes: Vec<u8>,
}

fn serialize_bytes_hex<S: serde::Serializer>(
    bytes: &[u8],
    serializer: S,
) -> Result<S::Ok, S::Error> {
    use std::fmt::Write;
    let mut encoded = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        write!(&mut encoded, "{byte:02x}").map_err(serde::ser::Error::custom)?;
    }
    serializer.serialize_str(&encoded)
}

pub(crate) struct AdmittedHandoff {
    manifest: LiveHeadSnapshot,
    receipt: HandoffReceipt,
    input: HandoffInput,
}

impl AdmittedHandoff {
    pub(crate) fn admit(input: &HandoffInput, version: &str) -> Result<Self, String> {
        admit_with(input, version, |reference, owner| {
            read_native(&input.controller_root, reference, owner)
        })
    }

    /// Reuse the raw-object owner; later swarm main movement cannot substitute
    /// checkout bytes for this exact pinned commit's package or denominator.
    pub(crate) fn verify_source(
        &self,
        root: &Path,
        sha: &str,
        reference: &str,
    ) -> Result<(), String> {
        use super::super::source::git_bytes;
        if self.manifest.candidate_sha()? != sha || self.manifest.candidate_ref()? != reference {
            return Err("source handoff candidate/ref differs from native acceptance".to_string());
        }
        let tree = git_bytes(root, &["rev-parse", "--verify", &format!("{sha}^{{tree}}")])?;
        if tree != format!("{}\n", self.manifest.candidate_tree()?).as_bytes() {
            return Err("source handoff tree differs from native acceptance".to_string());
        }
        self.manifest.verify_ranges(root)?;
        let mut blobs = std::collections::BTreeMap::new();
        for path in ["Cargo.toml", "crates/ripr/Cargo.toml", "Cargo.lock"] {
            blobs.insert(
                path.to_string(),
                git_bytes(root, &["show", &format!("{sha}:{path}")])?,
            );
        }
        self.manifest.verify_package_inputs("ripr", &blobs)
    }

    pub(crate) fn revalidate(self, version: &str) -> Result<HandoffReceipt, String> {
        // Re-fetch native bodies and retained files after the local preflight.
        // This is an observed snapshot, not an atomic GitHub transaction.
        let root = self.input.controller_root.clone();
        self.revalidate_with(version, |reference, owner| {
            read_native(&root, reference, owner)
        })
    }

    fn revalidate_with(
        self,
        version: &str,
        read: impl FnMut(&str, u64) -> Result<NativeDecision, String>,
    ) -> Result<HandoffReceipt, String> {
        let fresh = admit_with(&self.input, version, read)?;
        let old = serde_json::to_vec(&self.receipt).map_err(|error| error.to_string())?;
        let new = serde_json::to_vec(&fresh.receipt).map_err(|error| error.to_string())?;
        if old != new {
            return Err("source handoff acceptance changed during preflight".to_string());
        }
        self.manifest.revalidate()?;
        Ok(self.receipt)
    }
}

fn admit_with(
    input: &HandoffInput,
    version: &str,
    mut read: impl FnMut(&str, u64) -> Result<NativeDecision, String>,
) -> Result<AdmittedHandoff, String> {
    let selection = read(&input.selection_decision, 1609)?;
    let selected: SelectionAcceptance = selection.payload()?;
    if selected.schema_version != 1
        || selected.kind != "ripr_native_selection_acceptance"
        || selected.status != "accepted"
    {
        return Err("native #1609 selection is not accepted".to_string());
    }
    let manifest = LiveHeadSnapshot::admit(
        &input.controller_root,
        version,
        &input.manifest,
        &selected.subject.manifest_sha256,
    )?;
    let candidate = manifest.candidate()?;
    let expected = Subject {
        candidate_sha: candidate.sha.clone(),
        candidate_tree: candidate.tree.clone(),
        candidate_ref: candidate.git_ref.clone(),
        manifest_sha256: manifest.approved_digest.clone(),
    };
    if selected.subject != expected {
        return Err("native #1609 selection names another candidate/manifest".to_string());
    }
    let prerequisites = manifest
        .manifest
        .prerequisites
        .as_ref()
        .ok_or_else(|| "admitted manifest has no prerequisites".to_string())?;
    if selected.selected_claims != prerequisites.selected_claims
        || selected.proof_inputs != manifest.manifest.qualification.proof_inputs
        || owner_set(&selected.required_execution_owners)?
            != owner_set(&manifest.manifest.qualification.required_execution_owners)?
    {
        return Err(
            "native #1609 selection roster, #2766 packet or proof-input identities differ"
                .to_string(),
        );
    }
    let selected_claims = read(&selected.selected_claims.acceptance.decision_ref, 2766)?;
    require_hex(
        "native #2766 decision digest",
        &selected.selected_claims_decision_sha256,
        64,
    )?;
    if selected_claims.body_sha256 != selected.selected_claims_decision_sha256 {
        return Err("native #2766 decision differs from accepted selection".to_string());
    }
    let expected_rows = required_rows(&selected.required_qualification_rows)?;
    let row_owners = expected_rows
        .iter()
        .map(|row| row.owner_issue)
        .collect::<BTreeSet<_>>();
    if row_owners != owner_set(&selected.required_execution_owners)? {
        return Err(
            "native selection omits qualification rows for its applicable owner roster".to_string(),
        );
    }
    let mut excluded_ids = BTreeSet::new();
    if selected.excluded_subjects.len() > MAX_ROWS
        || selected.excluded_subjects.iter().any(|row| {
            row.id.trim().is_empty()
                || row.id.len() > 128
                || row.owner_issue == 0
                || row.count == 0
                || !matches!(row.disposition.as_str(), "excluded" | "deferred")
                || row.reason.trim().is_empty()
                || !excluded_ids.insert(&row.id)
                || expected_rows.iter().any(|required| required.id == row.id)
        })
    {
        return Err("native selection has invalid or overlapping excluded subjects".to_string());
    }
    let qualification = read(&input.qualification_decision, 2769)?;
    let accepted: QualificationAcceptance = qualification.payload()?;
    if accepted.schema_version != 1
        || accepted.kind != "ripr_native_qualification_acceptance"
        || accepted.status != "qualified"
        || accepted.subject != expected
        || accepted.selection_decision != selection.reference
        || accepted.selection_decision_sha256 != selection.body_sha256
        || required_rows(&accepted.required_qualification_rows)? != expected_rows
    {
        return Err(
            "native #2769 complete-bundle acceptance is missing, nonterminal or mismatched"
                .to_string(),
        );
    }
    require_hex(
        "native #2769 bundle digest",
        &accepted.qualification_bundle_sha256,
        64,
    )?;
    let bundle_path = input
        .qualification_bundle
        .to_str()
        .ok_or_else(|| "qualification bundle path must be UTF-8".to_string())?;
    let manifest_path = manifest
        .artifact_path
        .to_str()
        .ok_or_else(|| "manifest path must be UTF-8".to_string())?;
    let manifest_bytes = manifest
        .raw
        .get(manifest_path)
        .ok_or_else(|| "admitted manifest lost original bytes".to_string())?
        .clone();
    let mut packets = manifest
        .raw
        .iter()
        .filter(|(path, _)| path.as_str() != manifest_path)
        .map(|(path, bytes)| (path.clone(), bytes.clone()))
        .collect::<BTreeMap<_, _>>();
    let retained_size = manifest.raw.values().try_fold(0_u64, |size, bytes| {
        size.checked_add(bytes.len() as u64)
            .ok_or_else(|| "handoff retained-byte budget overflow".to_string())
    })?;
    let remaining = MAX_BUNDLE_BYTES
        .checked_sub(retained_size)
        .ok_or_else(|| "handoff exceeds 64 MiB aggregate retained-byte budget".to_string())?;
    if bundle_path == manifest_path || packets.contains_key(bundle_path) {
        return Err("qualification bundle overlaps admitted manifest evidence".to_string());
    }
    let qualification_bundle_bytes = read_owned(manifest.root(), bundle_path, remaining)?;
    if digest(&qualification_bundle_bytes) != accepted.qualification_bundle_sha256 {
        return Err(
            "qualification bundle differs from native #2769 accepted raw digest".to_string(),
        );
    }
    let bundle: QualificationBundle = serde_json::from_slice(&qualification_bundle_bytes)
        .map_err(|error| format!("parse complete qualification bundle: {error}"))?;
    if bundle.schema_version != 1
        || bundle.kind != "ripr_complete_qualification_bundle"
        || bundle.status != "qualified"
        || bundle.subject != expected
        || bundle.selection_decision != selection.reference
        || bundle.selection_decision_sha256 != selection.body_sha256
        || bundle.excluded_subjects != selected.excluded_subjects
    {
        return Err(
            "complete qualification bundle is nonterminal or names another selection".to_string(),
        );
    }
    let actual_rows = bundle
        .rows
        .iter()
        .map(|row| RequiredRow {
            id: row.id.clone(),
            owner_issue: row.owner_issue,
        })
        .collect::<Vec<_>>();
    if required_rows(&actual_rows)? != expected_rows {
        return Err("complete qualification bundle omits or changes required rows".to_string());
    }
    let mut remaining = remaining - qualification_bundle_bytes.len() as u64;
    let mut paths = BTreeSet::from([bundle_path.to_string()]);
    for row in &bundle.rows {
        if row.status != "passed"
            || row.selected == 0
            || row.executed != row.selected
            || row.failed != 0
            || row.skipped != 0
            || row.packet.owner_issue != row.owner_issue
        {
            return Err(format!(
                "required qualification row {} is non-positive or incomplete",
                row.id
            ));
        }
        super::validate_evidence(&row.packet, row.owner_issue)?;
        if row.packet.path == manifest_path {
            return Err("qualification packet overlaps admitted manifest".to_string());
        }
        if !paths.insert(row.packet.path.clone()) {
            return Err("qualification bundle repeats a packet path".to_string());
        }
        let bytes = match packets.get(&row.packet.path) {
            Some(bytes) => bytes.clone(),
            None => {
                let bytes = read_owned(manifest.root(), &row.packet.path, remaining)?;
                remaining -= bytes.len() as u64;
                bytes
            }
        };
        if digest(&bytes) != row.packet.sha256 {
            return Err(format!("qualification packet changed: {}", row.id));
        }
        packets.insert(row.packet.path.clone(), bytes);
    }
    manifest.revalidate()?;
    Ok(AdmittedHandoff {
        receipt: HandoffReceipt {
            schema: "ripr.source_handoff_acceptance.v2".to_string(), subject: expected,
            selection, selected_claims, qualification,
            qualification_bundle_sha256: accepted.qualification_bundle_sha256,
            required_execution_owners: selected.required_execution_owners,
            excluded_subjects: selected.excluded_subjects,
            proof_inputs: selected.proof_inputs, rows: bundle.rows,
            manifest_bytes, qualification_bundle_bytes,
            packet_inputs: packets.into_iter().map(|(path, bytes)| RetainedPacket {
                sha256: digest(&bytes), path, bytes,
            }).collect(),
            claim: "Native GitHub owner/member/collaborator decisions and exact retained bytes observed; trusted operator semantic judgment, not signatures, atomic provenance, source-join approval or publication authority".to_string(),
        },
        manifest, input: input.clone(),
    })
}

fn owner_set(owners: &[u64]) -> Result<BTreeSet<u64>, String> {
    let set = owners.iter().copied().collect::<BTreeSet<_>>();
    if owners.is_empty() || owners.len() > MAX_ROWS || set.len() != owners.len() || set.contains(&0)
    {
        return Err("acceptance owner roster is empty, repeated or exceeds budget".to_string());
    }
    Ok(set)
}

fn required_rows(rows: &[RequiredRow]) -> Result<BTreeSet<RequiredRow>, String> {
    let set = rows.iter().cloned().collect::<BTreeSet<_>>();
    let ids = rows.iter().map(|row| &row.id).collect::<BTreeSet<_>>();
    if rows.is_empty()
        || rows.len() > MAX_ROWS
        || set.len() != rows.len()
        || ids.len() != rows.len()
        || rows.iter().any(|row| {
            row.owner_issue == 0
                || row.id.is_empty()
                || row.id.len() > 128
                || !row
                    .id
                    .bytes()
                    .all(|c| c.is_ascii_alphanumeric() || b"._-".contains(&c))
        })
    {
        return Err(
            "qualification row denominator is empty, repeated, invalid or exceeds budget"
                .to_string(),
        );
    }
    Ok(set)
}

#[cfg(test)]
mod tests;
