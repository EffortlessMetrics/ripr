//! Portable original bytes authenticate projections; digest-shaped metadata
//! alone cannot establish manifest applicability or complete bundle rows.
use super::*;
use std::collections::BTreeMap;

const MAX_FILE: usize = 16 * 1024 * 1024;
const MAX_TOTAL: usize = 64 * 1024 * 1024;
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Manifest {
    schema_version: String,
    kind: String,
    release_line: String,
    authority_issue: u64,
    candidate_owner_issue: u64,
    status: String,
    candidate: Candidate,
    range: RangeIdentity,
    prerequisites: Prerequisites,
    pin: Pin,
    qualification: ManifestQualification,
    source_parent: Option<String>,
    non_claims: Vec<String>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Candidate {
    repository: String,
    sha: String,
    tree: String,
    #[serde(rename = "ref")]
    git_ref: String,
    package: Package,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Package {
    name: String,
    version: String,
    workspace_manifest_sha256: String,
    package_manifest_sha256: String,
    lock_sha256: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RangeIdentity {
    last_integrated_swarm_parent: String,
    all_reachable_count: u64,
    first_parent_count: u64,
    all_reachable_sha256: String,
    first_parent_sha256: String,
    record_set_sha256: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Prerequisites {
    selected_claims: AcceptedEvidence,
    denominator: AcceptedEvidence,
    audit: AcceptedEvidence,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Pin {
    remote_ref_readback: Evidence,
    ruleset: Evidence,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ManifestQualification {
    state: String,
    required_execution_owners: Vec<u64>,
    proof_inputs: Vec<Evidence>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Bundle {
    schema_version: u32,
    kind: String,
    status: String,
    subject: Subject,
    selection_decision: String,
    selection_decision_sha256: String,
    excluded_subjects: Vec<Excluded>,
    rows: Vec<QualifiedRow>,
}

pub(super) fn validate(
    receipt: &Receipt,
    selected: &Selection,
    version: &str,
) -> Result<(), String> {
    let mut total = 0;
    budget(&receipt.manifest_bytes, &mut total)?;
    budget(&receipt.qualification_bundle_bytes, &mut total)?;
    if receipt.packet_inputs.len() > 2 * MAX_ROWS + 5 {
        return Err("retained packet count exceeds budget".into());
    }
    let mut packets = BTreeMap::new();
    for packet in &receipt.packet_inputs {
        evidence(
            &Evidence {
                owner_issue: 1,
                path: packet.path.clone(),
                sha256: packet.sha256.clone(),
            },
            1,
        )?;
        budget(&packet.bytes, &mut total)?;
        if digest(&packet.bytes) != packet.sha256
            || packets.insert(packet.path.as_str(), packet).is_some()
        {
            return Err("retained packet raw digest differs or path repeats".into());
        }
    }
    if digest(&receipt.manifest_bytes) != receipt.subject.manifest_sha256 {
        return Err("retained manifest differs from native accepted raw digest".into());
    }
    let manifest: Manifest = serde_json::from_slice(&receipt.manifest_bytes)
        .map_err(|error| format!("retained manifest: {error}"))?;
    if manifest.schema_version != "1.1"
        || manifest.kind != "ripr_swarm_live_head_release_authority"
        || manifest.release_line != "0.11.0"
        || manifest.release_line != version
        || manifest.authority_issue != 2379
        || manifest.candidate_owner_issue != 1609
        || manifest.status != "pinned_exact_head"
        || manifest.source_parent.is_some()
        || manifest.non_claims.is_empty()
        || manifest.candidate.repository != "EffortlessMetrics/ripr-swarm"
        || manifest.candidate.sha != receipt.subject.candidate_sha
        || manifest.candidate.tree != receipt.subject.candidate_tree
        || manifest.candidate.git_ref != receipt.subject.candidate_ref
        || manifest.candidate.package.name != "ripr"
        || manifest.candidate.package.version != version
    {
        return Err("retained manifest authority/candidate/package identity differs".into());
    }
    for hash in [
        &manifest.candidate.package.workspace_manifest_sha256,
        &manifest.candidate.package.package_manifest_sha256,
        &manifest.candidate.package.lock_sha256,
        &manifest.range.all_reachable_sha256,
        &manifest.range.first_parent_sha256,
        &manifest.range.record_set_sha256,
    ] {
        hex(hash, 64)?;
    }
    if manifest.range.last_integrated_swarm_parent != "45b56c0957ad7e7360114edceca4b844c85f846e"
        || manifest.range.first_parent_count == 0
        || manifest.range.all_reachable_count < manifest.range.first_parent_count
    {
        return Err("retained manifest denominator is empty or names another boundary".into());
    }
    if manifest.qualification.state != "required_not_run"
        || owner_set(&manifest.qualification.required_execution_owners)?
            != owner_set(&receipt.required_execution_owners)?
        || manifest.qualification.proof_inputs != receipt.proof_inputs
        || manifest.qualification.proof_inputs.is_empty()
        || manifest.qualification.proof_inputs.len() > 64
        || !manifest
            .qualification
            .proof_inputs
            .iter()
            .any(|proof| proof.owner_issue == 4510)
        || (receipt.required_execution_owners.contains(&4604)
            && !manifest
                .qualification
                .proof_inputs
                .iter()
                .any(|proof| proof.owner_issue == 4603))
        || manifest.prerequisites.selected_claims != selected.selected_claims
    {
        return Err("retained manifest prerequisite/proof/owner applicability differs".into());
    }
    let mut expected = BTreeMap::<&str, &str>::new();
    for (owner, accepted) in [
        (2766, &manifest.prerequisites.selected_claims),
        (2768, &manifest.prerequisites.denominator),
        (3807, &manifest.prerequisites.audit),
    ] {
        evidence(&accepted.packet, owner)?;
        let prefix =
            format!("https://github.com/EffortlessMetrics/ripr-swarm/issues/{owner}#issuecomment-");
        let id = accepted
            .acceptance
            .decision_ref
            .strip_prefix(&prefix)
            .unwrap_or_default();
        if accepted.acceptance.status != "accepted"
            || accepted.acceptance.candidate_sha != receipt.subject.candidate_sha
            || accepted.acceptance.candidate_tree != receipt.subject.candidate_tree
            || accepted.acceptance.reviewed_packet_sha256 != accepted.packet.sha256
            || id.is_empty()
            || !id.bytes().all(|byte| byte.is_ascii_digit())
            || id.bytes().all(|byte| byte == b'0')
        {
            return Err(format!(
                "retained manifest #{owner} prerequisite acceptance differs"
            ));
        }
        insert_unique(&mut expected, &accepted.packet)?;
    }
    for pin in [&manifest.pin.remote_ref_readback, &manifest.pin.ruleset] {
        evidence(pin, 1609)?;
        insert_unique(&mut expected, pin)?;
    }
    for proof in &manifest.qualification.proof_inputs {
        evidence(proof, proof.owner_issue)?;
        insert_unique(&mut expected, proof)?;
    }
    if digest(&receipt.qualification_bundle_bytes) != receipt.qualification_bundle_sha256 {
        return Err("retained qualification bundle differs from native accepted raw digest".into());
    }
    let bundle: Bundle = serde_json::from_slice(&receipt.qualification_bundle_bytes)
        .map_err(|error| format!("retained qualification bundle: {error}"))?;
    if bundle.schema_version != 1
        || bundle.kind != "ripr_complete_qualification_bundle"
        || bundle.status != "qualified"
        || bundle.subject != receipt.subject
        || bundle.selection_decision != receipt.selection.reference
        || bundle.selection_decision_sha256 != receipt.selection.body_sha256
        || bundle.excluded_subjects != receipt.excluded_subjects
        || bundle.rows != receipt.rows
    {
        return Err(
            "retained qualification bundle subject/selection/exclusions/rows differ".into(),
        );
    }
    for row in &receipt.rows {
        // The producer permits a required row to reuse an identical proof packet.
        if let Some(existing) = expected.get(row.packet.path.as_str()) {
            if *existing != row.packet.sha256 {
                return Err("required row changes existing packet identity".into());
            }
        } else {
            expected.insert(&row.packet.path, &row.packet.sha256);
        }
    }
    if packets.len() != expected.len()
        || expected.iter().any(|(path, hash)| {
            packets
                .get(path)
                .is_none_or(|packet| packet.sha256 != *hash)
        })
    {
        return Err(
            "retained packet inventory has missing, unreferenced or mismatched inputs".into(),
        );
    }
    let readback = packets
        .get(manifest.pin.remote_ref_readback.path.as_str())
        .ok_or_else(|| "retained candidate pin readback missing".to_string())?;
    if readback.bytes != format!("{}\n", receipt.subject.candidate_sha).as_bytes() {
        return Err("retained remote pin readback names another candidate".into());
    }
    let ruleset = packets
        .get(manifest.pin.ruleset.path.as_str())
        .ok_or_else(|| "retained pin ruleset missing".to_string())?;
    let rules: Value = serde_json::from_slice(&ruleset.bytes)
        .map_err(|error| format!("retained pin ruleset: {error}"))?;
    let has_rule = |kind| {
        rules
            .get("rules")
            .and_then(Value::as_array)
            .is_some_and(|rows| {
                rows.iter()
                    .any(|row| row.get("type").and_then(Value::as_str) == Some(kind))
            })
    };
    if rules.get("name").and_then(Value::as_str) != Some("release-transaction-pins")
        || rules.get("target").and_then(Value::as_str) != Some("tag")
        || rules.get("enforcement").and_then(Value::as_str) != Some("active")
        || rules.pointer("/conditions/ref_name/include")
            != Some(&serde_json::json!(["refs/tags/ripr-release-*"]))
        || rules.pointer("/conditions/ref_name/exclude") != Some(&serde_json::json!([]))
        || rules.get("bypass_actors") != Some(&serde_json::json!([]))
        || !has_rule("update")
        || !has_rule("deletion")
    {
        return Err("retained ruleset does not protect candidate pin without bypass".into());
    }
    Ok(())
}
fn insert_unique<'a>(
    expected: &mut BTreeMap<&'a str, &'a str>,
    packet: &'a Evidence,
) -> Result<(), String> {
    if expected.insert(&packet.path, &packet.sha256).is_some() {
        return Err("retained manifest repeats an evidence path".into());
    }
    Ok(())
}
fn budget(bytes: &[u8], total: &mut usize) -> Result<(), String> {
    if bytes.is_empty() || bytes.len() > MAX_FILE {
        return Err("retained input is empty or exceeds 16 MiB per-file budget".into());
    }
    *total = total
        .checked_add(bytes.len())
        .ok_or_else(|| "retained-byte budget overflow".to_string())?;
    if *total > MAX_TOTAL {
        return Err("handoff exceeds 64 MiB aggregate retained-byte budget".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn retained_byte_budgets_reject_empty_oversized_and_aggregate_overflow() -> Result<(), String> {
        assert!(budget(&[], &mut 0).is_err());
        assert!(budget(&vec![0; MAX_FILE + 1], &mut 0).is_err());
        let mut exhausted = MAX_TOTAL;
        assert!(budget(&[0], &mut exhausted).is_err());
        let mut overflow = usize::MAX;
        assert!(budget(&[0], &mut overflow).is_err());
        let mut total = MAX_TOTAL - 1;
        budget(&[0], &mut total)?;
        assert_eq!(total, MAX_TOTAL);
        Ok(())
    }
}
