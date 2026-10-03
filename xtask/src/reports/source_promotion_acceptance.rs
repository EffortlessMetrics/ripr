//! Current handoff validation. Retained bodies are replayable evidence, not
//! authenticated responses; live consumers must also call `revalidate`.
use serde::Deserialize;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
use std::path::Path;
use std::time::Duration;

mod material;
mod native;
use native::NativeDecision;

const MAX_ROWS: usize = 256;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
struct Subject {
    candidate_sha: String,
    candidate_tree: String,
    candidate_ref: String,
    manifest_sha256: String,
}
#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
struct Evidence {
    owner_issue: u64,
    path: String,
    sha256: String,
}
#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
struct OwnerAcceptance {
    status: String,
    candidate_sha: String,
    candidate_tree: String,
    reviewed_packet_sha256: String,
    decision_ref: String,
}
#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
struct AcceptedEvidence {
    packet: Evidence,
    acceptance: OwnerAcceptance,
}
#[derive(Clone, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd)]
#[serde(deny_unknown_fields)]
struct RequiredRow {
    id: String,
    owner_issue: u64,
}
#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
struct Excluded {
    id: String,
    owner_issue: u64,
    count: u64,
    disposition: String,
    reason: String,
}
#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
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
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Receipt {
    schema: String,
    subject: Subject,
    selection: NativeDecision,
    selected_claims: NativeDecision,
    qualification: NativeDecision,
    qualification_bundle_sha256: String,
    required_execution_owners: Vec<u64>,
    excluded_subjects: Vec<Excluded>,
    proof_inputs: Vec<Evidence>,
    rows: Vec<QualifiedRow>,
    #[serde(deserialize_with = "deserialize_bytes_hex")]
    manifest_bytes: Vec<u8>,
    #[serde(deserialize_with = "deserialize_bytes_hex")]
    qualification_bundle_bytes: Vec<u8>,
    packet_inputs: Vec<RetainedPacket>,
    claim: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RetainedPacket {
    path: String,
    sha256: String,
    #[serde(deserialize_with = "deserialize_bytes_hex")]
    bytes: Vec<u8>,
}

fn deserialize_bytes_hex<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<Vec<u8>, D::Error> {
    let encoded = String::deserialize(deserializer)?;
    if encoded.len() > 32 * 1024 * 1024
        || encoded.len() % 2 != 0
        || !encoded
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(serde::de::Error::custom(
            "retained bytes require bounded, even-length lowercase hex string",
        ));
    }
    let nibble = |byte: u8| {
        if byte.is_ascii_digit() {
            byte - b'0'
        } else {
            byte - b'a' + 10
        }
    };
    Ok(encoded
        .as_bytes()
        .chunks_exact(2)
        .map(|pair| (nibble(pair[0]) << 4) | nibble(pair[1]))
        .collect())
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Selection {
    schema_version: u32,
    kind: String,
    status: String,
    subject: Subject,
    selected_claims: AcceptedEvidence,
    selected_claims_decision_sha256: String,
    required_execution_owners: Vec<u64>,
    excluded_subjects: Vec<Excluded>,
    proof_inputs: Vec<Evidence>,
    required_qualification_rows: Vec<RequiredRow>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Qualification {
    schema_version: u32,
    kind: String,
    status: String,
    subject: Subject,
    selection_decision: String,
    selection_decision_sha256: String,
    qualification_bundle_sha256: String,
    required_qualification_rows: Vec<RequiredRow>,
}

pub(crate) fn validate(preflight: &Value) -> Result<(), String> {
    admitted(preflight).map(|_| ())
}

fn admitted(preflight: &Value) -> Result<Receipt, String> {
    if preflight.get("schema").and_then(Value::as_str) != Some("ripr.source_promotion_preflight.v2")
    {
        return Err("current source handoff requires preflight v2".into());
    }
    let receipt: Receipt = serde_json::from_value(
        preflight
            .get("acceptance")
            .cloned()
            .ok_or_else(|| "preflight v2 is missing native acceptance".to_string())?,
    )
    .map_err(|error| format!("source handoff receipt: {error}"))?;
    if receipt.schema != "ripr.source_handoff_acceptance.v2" || receipt.claim.trim().is_empty() {
        return Err("unsupported or incomplete source handoff acceptance".into());
    }
    let subject = &receipt.subject;
    hex(&subject.candidate_sha, 40)?;
    hex(&subject.candidate_tree, 40)?;
    hex(&subject.manifest_sha256, 64)?;
    if preflight.get("swarm_parent").and_then(Value::as_str) != Some(subject.candidate_sha.as_str())
        || preflight.get("swarm_ref_sha").and_then(Value::as_str)
            != Some(subject.candidate_sha.as_str())
        || preflight.get("swarm_ref").and_then(Value::as_str)
            != Some(subject.candidate_ref.as_str())
    {
        return Err("source handoff candidate/ref differs from preflight".into());
    }
    let version = preflight
        .pointer("/version_state/requested_version")
        .and_then(Value::as_str)
        .filter(|version| !version.is_empty())
        .ok_or_else(|| "missing handoff version".to_string())?;
    if subject.candidate_ref
        != format!("refs/tags/ripr-release-{version}-{}", subject.candidate_sha)
    {
        return Err("source handoff requires exact protected candidate tag".into());
    }
    for (decision, owner) in [
        (&receipt.selection, 1609),
        (&receipt.selected_claims, 2766),
        (&receipt.qualification, 2769),
    ] {
        decision.validate(owner)?;
    }
    let selected: Selection = receipt.selection.payload()?;
    if selected.schema_version != 1
        || selected.kind != "ripr_native_selection_acceptance"
        || selected.status != "accepted"
        || selected.subject != *subject
    {
        return Err("native #1609 selection is nonterminal or names another subject".into());
    }
    let owners = owner_set(&receipt.required_execution_owners)?;
    if owners != owner_set(&selected.required_execution_owners)?
        || receipt.excluded_subjects != selected.excluded_subjects
        || receipt.proof_inputs != selected.proof_inputs
    {
        return Err("source handoff selection roster/exclusions/proof identities differ".into());
    }
    let claims = &selected.selected_claims;
    evidence(&claims.packet, 2766)?;
    if claims.acceptance.status != "accepted"
        || claims.acceptance.candidate_sha != subject.candidate_sha
        || claims.acceptance.candidate_tree != subject.candidate_tree
        || claims.acceptance.reviewed_packet_sha256 != claims.packet.sha256
        || claims.acceptance.decision_ref != receipt.selected_claims.reference
        || selected.selected_claims_decision_sha256 != receipt.selected_claims.body_sha256
    {
        return Err("source handoff #2766 selected-claims identity differs".into());
    }
    let required = required_rows(&selected.required_qualification_rows)?;
    if required
        .iter()
        .map(|row| row.owner_issue)
        .collect::<BTreeSet<_>>()
        != owners
    {
        return Err("selection row denominator differs from applicable owner roster".into());
    }
    let mut excluded_ids = BTreeSet::new();
    if receipt.excluded_subjects.len() > MAX_ROWS
        || receipt.excluded_subjects.iter().any(|row| {
            row.id.trim().is_empty()
                || row.id.len() > 128
                || row.owner_issue == 0
                || row.count == 0
                || !matches!(row.disposition.as_str(), "excluded" | "deferred")
                || row.reason.trim().is_empty()
                || !excluded_ids.insert(&row.id)
                || required.iter().any(|required| required.id == row.id)
        })
    {
        return Err("invalid or overlapping excluded subjects".into());
    }
    if receipt.proof_inputs.len() > MAX_ROWS {
        return Err("proof input budget exceeded".into());
    }
    for proof in &receipt.proof_inputs {
        evidence(proof, proof.owner_issue)?;
    }
    let qualified: Qualification = receipt.qualification.payload()?;
    hex(&receipt.qualification_bundle_sha256, 64)?;
    if qualified.schema_version != 1
        || qualified.kind != "ripr_native_qualification_acceptance"
        || qualified.status != "qualified"
        || qualified.subject != *subject
        || qualified.selection_decision != receipt.selection.reference
        || qualified.selection_decision_sha256 != receipt.selection.body_sha256
        || qualified.qualification_bundle_sha256 != receipt.qualification_bundle_sha256
        || required_rows(&qualified.required_qualification_rows)? != required
    {
        return Err("native #2769 complete-bundle acceptance is nonterminal or mismatched".into());
    }
    let actual = receipt
        .rows
        .iter()
        .map(|row| RequiredRow {
            id: row.id.clone(),
            owner_issue: row.owner_issue,
        })
        .collect::<Vec<_>>();
    if required_rows(&actual)? != required {
        return Err("handoff omits or changes required rows".into());
    }
    let mut paths = BTreeSet::new();
    for row in &receipt.rows {
        evidence(&row.packet, row.owner_issue)?;
        if row.status != "passed"
            || row.selected == 0
            || row.executed != row.selected
            || row.failed != 0
            || row.skipped != 0
            || !paths.insert(&row.packet.path)
        {
            return Err(format!(
                "required qualification row {} is incomplete or repeats a packet",
                row.id
            ));
        }
    }
    material::validate(&receipt, &selected, version)?;
    Ok(receipt)
}

/// Re-fetch native identities and bind the candidate tree to the actual object.
/// This observes a snapshot; it does not promise atomic GitHub provenance.
pub(crate) fn revalidate(preflight: &Value, root: &Path) -> Result<(), String> {
    revalidate_with_reader_and_tree(preflight, root, |reference, owner| {
        native::read(root, reference, owner)
    })
}

pub(crate) fn revalidate_with_reader_and_tree(
    preflight: &Value,
    root: &Path,
    read: impl FnMut(&str, u64) -> Result<Vec<u8>, String>,
) -> Result<(), String> {
    revalidate_with_reader(preflight, read)?;
    let subject = admitted(preflight)?.subject;
    let args = vec![
        "--no-replace-objects".into(),
        "rev-parse".into(),
        "--verify".into(),
        format!("{}^{{tree}}", subject.candidate_sha),
    ];
    let output = crate::run::capture_output_in_dir_with_timeout_bounded(
        Path::new("git"),
        &args,
        &[],
        root,
        Duration::from_secs(30),
        1024 * 1024,
        "handoff candidate tree",
    )?;
    if output.timed_out
        || output.stdout_truncated
        || output.stderr_truncated
        || output.stderr.len() > 64 * 1024
        || output.status.is_none_or(|status| !status.success())
        || output.stdout != format!("{}\n", subject.candidate_tree)
    {
        return Err("handoff candidate tree differs or object read failed".into());
    }
    Ok(())
}

/// Reader returns actual API response bytes; identity/trust decoding remains here.
pub(crate) fn revalidate_with_reader(
    preflight: &Value,
    mut read: impl FnMut(&str, u64) -> Result<Vec<u8>, String>,
) -> Result<(), String> {
    let receipt = admitted(preflight)?;
    for (retained, owner) in [
        (&receipt.selection, 1609),
        (&receipt.selected_claims, 2766),
        (&receipt.qualification, 2769),
    ] {
        let fresh = native::decode(
            &retained.reference,
            owner,
            &read(&retained.reference, owner)?,
        )?;
        if fresh != *retained {
            return Err(format!(
                "native #{owner} acceptance changed since preflight"
            ));
        }
    }
    Ok(())
}

fn hex(value: &str, length: usize) -> Result<(), String> {
    if value.len() != length
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
    {
        return Err("handoff identity must be exact lowercase hexadecimal".into());
    }
    Ok(())
}
fn digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
fn evidence(packet: &Evidence, owner: u64) -> Result<(), String> {
    if owner == 0
        || packet.owner_issue != owner
        || packet.path.is_empty()
        || packet.path.contains(['\\', ':', '\0'])
        || packet.path.starts_with('/')
        || packet
            .path
            .split('/')
            .any(|part| part.is_empty() || matches!(part, "." | ".."))
    {
        return Err("handoff evidence owner/path is invalid".into());
    }
    hex(&packet.sha256, 64)
}
fn owner_set(owners: &[u64]) -> Result<BTreeSet<u64>, String> {
    let set = owners.iter().copied().collect::<BTreeSet<_>>();
    if owners.is_empty() || owners.len() > MAX_ROWS || set.len() != owners.len() || set.contains(&0)
    {
        return Err("handoff owner roster is empty, repeated or exceeds budget".into());
    }
    Ok(set)
}
fn required_rows(rows: &[RequiredRow]) -> Result<BTreeSet<RequiredRow>, String> {
    let set = rows.iter().cloned().collect::<BTreeSet<_>>();
    if rows.is_empty()
        || rows.len() > MAX_ROWS
        || set.len() != rows.len()
        || rows
            .iter()
            .map(|row| &row.id)
            .collect::<BTreeSet<_>>()
            .len()
            != rows.len()
        || rows.iter().any(|row| {
            row.owner_issue == 0
                || row.id.is_empty()
                || row.id.len() > 128
                || !row
                    .id
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || b"._-".contains(&byte))
        })
    {
        return Err("handoff qualification denominator is invalid".into());
    }
    Ok(set)
}

#[cfg(test)]
pub(crate) mod tests;
