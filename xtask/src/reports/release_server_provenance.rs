use std::fs;
use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::{command_success_owned, run_owned};

use super::release_server::{
    normalize_product_version, release_server_assets, release_server_target_set,
    release_server_target_set_digest, required_release_arg, sha256_file,
    validate_configured_release_server_targets,
    validate_release_server_relative_path,
};
#[cfg(test)]
use super::release_server::sha256_bytes;

#[derive(Debug, Clone, Serialize)]
struct FinalServerSubject {
    name: String,
    kind: String,
    target: Option<String>,
    source_role: String,
    mode: u32,
    size: u64,
    sha256: String,
    source_receipt: String,
}

const ATTESTATION_ACTION: &str =
    "actions/attest-build-provenance@4d101475d8b20a2381f78447822ac1eab6504dd8";
const RELEASE_WORKFLOW_PATH: &str = ".github/workflows/release-server-binaries.yml";

fn final_server_inventory_path(version: &str) -> PathBuf {
    Path::new("dist").join(format!("ripr-server-final-subjects-v{version}.json"))
}

fn final_server_attestation_receipt_path(version: &str) -> PathBuf {
    Path::new("dist").join(format!("ripr-server-attestation-v{version}.receipt.json"))
}

pub(crate) fn release_final_server_subjects(args: &[String]) -> Result<(), String> {
    let version = normalize_product_version(&required_release_arg(args, "version", "RAW_VERSION")?)?;
    let repository = required_release_arg(args, "repository", "REPOSITORY")?;
    let dist_dir = Path::new("dist");
    let assets = release_server_assets(dist_dir, &version)?;
    validate_configured_release_server_targets(&assets)?;

    let manifest_name = format!("ripr-server-manifest-v{version}.json");
    let assembly_name = format!("ripr-server-assembly-v{version}.receipt.json");
    let manifest_path = dist_dir.join(&manifest_name);
    let sums_path = dist_dir.join("SHA256SUMS");
    let assembly_path = dist_dir.join(&assembly_name);
    for path in [&manifest_path, &sums_path, &assembly_path] {
        require_regular_release_file(path)?;
    }

    let assembly_text = fs::read_to_string(&assembly_path)
        .map_err(|err| format!("failed to read {}: {err}", assembly_path.display()))?;
    let assembly: serde_json::Value = serde_json::from_str(&assembly_text)
        .map_err(|err| format!("malformed assembly receipt {}: {err}", assembly_path.display()))?;
    if assembly.get("disposition").and_then(serde_json::Value::as_str) != Some("assembled") {
        return Err("release server assembly receipt is not terminal 'assembled'".to_string());
    }
    if assembly.get("version").and_then(serde_json::Value::as_str) != Some(version.as_str()) {
        return Err("release server assembly receipt version does not match requested version".to_string());
    }
    let manifest_sha = sha256_file(&manifest_path)?;
    let sums_sha = sha256_file(&sums_path)?;
    require_receipt_digest(&assembly, "manifest", &manifest_sha)?;
    require_receipt_digest(&assembly, "sha256sums", &sums_sha)?;

    let mut subjects = Vec::new();
    for asset in &assets {
        let path = dist_dir.join(&asset.file_name);
        require_regular_release_file(&path)?;
        let metadata = fs::metadata(&path)
            .map_err(|err| format!("failed to stat {}: {err}", path.display()))?;
        subjects.push(FinalServerSubject {
            name: asset.file_name.clone(),
            kind: "server_archive".to_string(),
            target: Some(asset.target.clone()),
            source_role: "configured_target_archive".to_string(),
            mode: release_file_mode(&metadata),
            size: metadata.len(),
            sha256: sha256_file(&path)?,
            source_receipt: format!("ripr-server-v{version}-{}.receipt.json", asset.target),
        });
    }
    let manifest_metadata = fs::metadata(&manifest_path)
        .map_err(|err| format!("failed to stat {}: {err}", manifest_path.display()))?;
    subjects.push(FinalServerSubject {
        name: manifest_name.clone(),
        kind: "server_manifest".to_string(),
        target: None,
        source_role: "assembled_manifest".to_string(),
        mode: release_file_mode(&manifest_metadata),
        size: manifest_metadata.len(),
        sha256: manifest_sha,
        source_receipt: assembly_name.clone(),
    });
    let sums_metadata = fs::metadata(&sums_path)
        .map_err(|err| format!("failed to stat {}: {err}", sums_path.display()))?;
    subjects.push(FinalServerSubject {
        name: "SHA256SUMS".to_string(),
        kind: "checksums".to_string(),
        target: None,
        source_role: "aggregate_checksums".to_string(),
        mode: release_file_mode(&sums_metadata),
        size: sums_metadata.len(),
        sha256: sums_sha,
        source_receipt: assembly_name.clone(),
    });
    subjects.sort_by(|left, right| left.name.cmp(&right.name));
    let expected_names = subjects
        .iter()
        .map(|subject| subject.name.clone())
        .collect::<Vec<_>>();
    let unique_names = expected_names
        .iter()
        .collect::<std::collections::BTreeSet<_>>();
    if unique_names.len() != expected_names.len() {
        return Err("final server subject inventory contains duplicate normalized names".to_string());
    }

    validate_sha256sums_subjects(&sums_path, &subjects)?;
    validate_final_server_staging_entries(dist_dir, &version, &subjects)?;

    let build_identity = assembly
        .get("build_identity")
        .cloned()
        .ok_or_else(|| "release server assembly receipt has no build_identity".to_string())?;
    let receipt = serde_json::json!({
        "schema_version": "release-final-server-subjects/1",
        "producer": "xtask release-final-server-subjects",
        "repository": repository,
        "version": version,
        "build_identity": build_identity,
        "configured_target_set": {
            "targets": release_server_target_set(),
            "sha256": release_server_target_set_digest(),
        },
        "assembly_receipt": {
            "path": assembly_name,
            "sha256": sha256_file(&assembly_path)?,
            "schema_version": assembly.get("schema_version").cloned().unwrap_or(serde_json::Value::Null),
        },
        "subject_state": {
            "expected": expected_names,
            "observed": subjects.iter().map(|subject| subject.name.clone()).collect::<Vec<_>>(),
            "missing": Vec::<String>::new(),
            "duplicates": Vec::<String>::new(),
            "unexpected": Vec::<String>::new(),
        },
        "subjects": subjects,
        "subject_count": subjects.len(),
        "attestation_required": true,
        "upload_eligible": false,
        "publication_mutation_attempted": false,
        "disposition": "inventoried",
        "non_claims": [
            "inventory is not attestation verification",
            "inventory does not authorize release publication"
        ],
    });
    let text = serde_json::to_string_pretty(&receipt)
        .map_err(|err| format!("failed to render final server subject inventory: {err}"))?;
    let path = final_server_inventory_path(&version);
    fs::write(&path, format!("{text}\n"))
        .map_err(|err| format!("failed to write {}: {err}", path.display()))?;
    eprintln!("wrote {}", path.display());
    Ok(())
}


pub(crate) fn release_final_server_attestation_fixture(args: &[String]) -> Result<(), String> {
    let version = normalize_product_version(&required_release_arg(args, "version", "RAW_VERSION")?)?;
    let expected_source_sha =
        required_release_arg(args, "expected-source-sha", "EXPECTED_SOURCE_SHA")?;
    let repository = required_github_identity("GITHUB_REPOSITORY")?;
    let workflow_ref = required_github_identity("GITHUB_WORKFLOW_REF")?;
    let candidate_sha = required_github_identity("GITHUB_SHA")?;
    let git_ref = required_github_identity("GITHUB_REF")?;
    let run_id = required_github_identity("GITHUB_RUN_ID")?;
    let run_attempt = required_github_identity("GITHUB_RUN_ATTEMPT")?;
    if candidate_sha != expected_source_sha {
        return Err(format!(
            "manual attestation fixture expected source SHA '{expected_source_sha}' but workflow checked out '{candidate_sha}'"
        ));
    }

    let inventory_path = final_server_inventory_path(&version);
    let inventory_text = fs::read_to_string(&inventory_path)
        .map_err(|err| format!("failed to read {}: {err}", inventory_path.display()))?;
    let inventory: serde_json::Value = serde_json::from_str(&inventory_text)
        .map_err(|err| format!("malformed final server subject inventory: {err}"))?;
    if inventory.get("disposition").and_then(serde_json::Value::as_str) != Some("inventoried") {
        return Err("final server subject inventory is not terminal 'inventoried'".to_string());
    }
    let rows = inventory
        .get("subjects")
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| "final server subject inventory has no subjects".to_string())?;
    let subject_paths = final_server_subject_paths(Path::new("dist"), &version)?;
    if subject_paths.len() != rows.len() {
        return Err("final server subject path count differs from inventory rows".to_string());
    }

    let signer_workflow = format!("{repository}/{RELEASE_WORKFLOW_PATH}");
    let subjects = rows
        .iter()
        .map(|row| {
            serde_json::json!({
                "name": row.get("name").cloned().unwrap_or(serde_json::Value::Null),
                "sha256": row.get("sha256").cloned().unwrap_or(serde_json::Value::Null),
                "verified": false,
                "reason": "fixture_only_no_attestation_permission",
                "verification_plan": {
                    "repository": repository,
                    "signer_workflow": signer_workflow,
                    "signer_digest": candidate_sha,
                    "source_digest": candidate_sha,
                    "source_ref": git_ref,
                    "predicate_type": "https://slsa.dev/provenance/v1",
                    "attestation_action": ATTESTATION_ACTION,
                }
            })
        })
        .collect::<Vec<_>>();
    let receipt = serde_json::json!({
        "schema_version": "release-server-attestation/1",
        "producer": "xtask release-final-server-attestation-fixture",
        "version": version,
        "inventory": {
            "path": inventory_path.file_name().and_then(|name| name.to_str()).unwrap_or_default(),
            "sha256": sha256_file(&inventory_path)?,
            "configured_target_set": inventory.get("configured_target_set").cloned().unwrap_or(serde_json::Value::Null),
            "assembly_receipt": inventory.get("assembly_receipt").cloned().unwrap_or(serde_json::Value::Null),
            "subject_state": inventory.get("subject_state").cloned().unwrap_or(serde_json::Value::Null),
        },
        "producer_identity": {
            "repository": repository,
            "workflow_ref": workflow_ref,
            "candidate_sha": candidate_sha,
            "git_ref": git_ref,
            "run_id": run_id,
            "run_attempt": run_attempt,
            "attestation_action": ATTESTATION_ACTION,
            "release_workflow_path": RELEASE_WORKFLOW_PATH,
        },
        "subjects": subjects,
        "permission_requested": {
            "id_token_write": false,
            "attestations_write": false,
        },
        "permission_available": {
            "id_token": false,
            "attestations_write": false,
        },
        "release_upload_eligible": false,
        "release_upload_attempted": false,
        "publication_mutation_attempted": false,
        "disposition": "not_authorized",
        "reason": "manual fixture proves exact subject and verifier construction without attestation or publication authority",
    });
    write_attestation_receipt(&version, &receipt)
}

pub(crate) fn release_final_server_attestation_receipt(args: &[String]) -> Result<(), String> {
    let version = normalize_product_version(&required_release_arg(args, "version", "RAW_VERSION")?)?;
    let verified_path = PathBuf::from(required_release_arg(
        args,
        "verified-subjects",
        "VERIFIED_SUBJECTS",
    )?);
    let inventory_path = final_server_inventory_path(&version);
    let inventory_text = fs::read_to_string(&inventory_path)
        .map_err(|err| format!("failed to read {}: {err}", inventory_path.display()))?;
    let inventory: serde_json::Value = serde_json::from_str(&inventory_text)
        .map_err(|err| format!("malformed final server subject inventory: {err}"))?;
    if inventory.get("disposition").and_then(serde_json::Value::as_str) != Some("inventoried") {
        return Err("final server subject inventory is not terminal 'inventoried'".to_string());
    }
    let rows = inventory
        .get("subjects")
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| "final server subject inventory has no subjects".to_string())?;
    let subject_paths = final_server_subject_paths(Path::new("dist"), &version)?;
    if subject_paths.len() != rows.len() {
        return Err("final server subject path count differs from inventory rows".to_string());
    }

    let repository = required_github_identity("GITHUB_REPOSITORY")?;
    let workflow_ref = required_github_identity("GITHUB_WORKFLOW_REF")?;
    let candidate_sha = required_github_identity("GITHUB_SHA")?;
    let git_ref = required_github_identity("GITHUB_REF")?;
    let run_id = required_github_identity("GITHUB_RUN_ID")?;
    let run_attempt = required_github_identity("GITHUB_RUN_ATTEMPT")?;
    let verified_text = fs::read_to_string(&verified_path)
        .map_err(|err| format!("failed to read {}: {err}", verified_path.display()))?;
    let mut verified = std::collections::BTreeMap::new();
    for (index, line) in verified_text.lines().enumerate() {
        let Some((sha, name)) = line.split_once("  ") else {
            return Err(format!("verified-subjects line {} is malformed", index + 1));
        };
        if verified.insert(name.to_string(), sha.to_string()).is_some() {
            return Err(format!("verified-subjects duplicates '{name}'"));
        }
    }

    let mut attested = Vec::new();
    for row in rows {
        let name = row
            .get("name")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| "subject row has no name".to_string())?;
        let sha = row
            .get("sha256")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| format!("subject '{name}' has no sha256"))?;
        match verified.remove(name) {
            Some(observed) if observed == sha => {}
            Some(observed) => {
                return Err(format!(
                    "attestation verification digest mismatch for '{name}': expected '{sha}', observed '{observed}'"
                ));
            }
            None => return Err(format!("missing attestation verification for '{name}'")),
        }
        attested.push(serde_json::json!({
            "name": name,
            "sha256": sha,
            "verified": true,
            "verification": {
                "repository": repository,
                "workflow_ref": workflow_ref,
                "source_ref": git_ref,
                "source_sha": candidate_sha,
                "attestation_action": ATTESTATION_ACTION,
                "predicate_type": "https://slsa.dev/provenance/v1",
            },
        }));
    }
    if !verified.is_empty() {
        return Err(format!(
            "attestation verification includes unexpected subjects: {:?}",
            verified.keys().collect::<Vec<_>>()
        ));
    }


    let receipt = serde_json::json!({
        "schema_version": "release-server-attestation/1",
        "producer": "xtask release-final-server-attestation-receipt",
        "version": version,
        "inventory": {
            "path": inventory_path.file_name().and_then(|name| name.to_str()).unwrap_or_default(),
            "sha256": sha256_file(&inventory_path)?,
            "configured_target_set": inventory.get("configured_target_set").cloned().unwrap_or(serde_json::Value::Null),
            "assembly_receipt": inventory.get("assembly_receipt").cloned().unwrap_or(serde_json::Value::Null),
            "subject_state": inventory.get("subject_state").cloned().unwrap_or(serde_json::Value::Null),
        },
        "producer_identity": {
            "repository": repository,
            "workflow_ref": workflow_ref,
            "candidate_sha": candidate_sha,
            "git_ref": git_ref,
            "run_id": run_id,
            "run_attempt": run_attempt,
            "attestation_action": ATTESTATION_ACTION,
            "release_workflow_path": RELEASE_WORKFLOW_PATH,
        },
        "subjects": attested,
        "permission_requested": {
            "id_token_write": true,
            "attestations_write": true,
        },
        "permission_available": {
            "id_token": github_oidc_available(),
            "attestations_write": std::env::var("RIPR_ATTESTATION_ACTION_COMPLETED").ok().as_deref() == Some("true"),
        },
        "release_upload_eligible": true,
        "release_upload_attempted": false,
        "publication_mutation_attempted": false,
        "disposition": "attested",
        "non_claims": [
            "attestation does not authorize a release channel",
            "no release asset has been uploaded by this receipt producer"
        ],
    });
    write_attestation_receipt(&version, &receipt)
}

fn write_attestation_receipt(version: &str, receipt: &serde_json::Value) -> Result<(), String> {
    let text = serde_json::to_string_pretty(receipt)
        .map_err(|err| format!("failed to render server attestation receipt: {err}"))?;
    let path = final_server_attestation_receipt_path(version);
    fs::write(&path, format!("{text}\n"))
        .map_err(|err| format!("failed to write {}: {err}", path.display()))?;
    eprintln!("wrote {}", path.display());
    Ok(())
}

fn github_oidc_available() -> bool {
    std::env::var("ACTIONS_ID_TOKEN_REQUEST_URL")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .is_some()
        && std::env::var("ACTIONS_ID_TOKEN_REQUEST_TOKEN")
            .ok()
            .filter(|value| !value.trim().is_empty())
            .is_some()
}

#[cfg(unix)]
fn release_file_mode(metadata: &fs::Metadata) -> u32 {
    use std::os::unix::fs::PermissionsExt;
    metadata.permissions().mode() & 0o777
}

#[cfg(not(unix))]
fn release_file_mode(_metadata: &fs::Metadata) -> u32 {
    0
}

fn required_github_identity(name: &str) -> Result<String, String> {
    std::env::var(name)
        .ok()
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| format!("required GitHub Actions identity {name} is unavailable"))
}

fn final_server_subject_paths(dist_dir: &Path, version: &str) -> Result<Vec<PathBuf>, String> {
    let inventory_path = final_server_inventory_path(version);
    let text = fs::read_to_string(&inventory_path)
        .map_err(|err| format!("final subject inventory {} is unavailable: {err}", inventory_path.display()))?;
    let inventory: serde_json::Value = serde_json::from_str(&text)
        .map_err(|err| format!("malformed final subject inventory: {err}"))?;
    let rows = inventory
        .get("subjects")
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| "final subject inventory has no subjects".to_string())?;
    let mut paths = Vec::new();
    for row in rows {
        let name = row
            .get("name")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| "final subject row has no name".to_string())?;
        validate_release_server_relative_path(name, "final subject", "release")?;
        let path = dist_dir.join(name);
        require_regular_release_file(&path)?;
        let expected = row
            .get("sha256")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| format!("final subject '{name}' has no sha256"))?;
        let actual = sha256_file(&path)?;
        if actual != expected {
            return Err(format!(
                "final subject '{name}' changed after inventory: expected '{expected}', actual '{actual}'"
            ));
        }
        paths.push(path);
    }
    paths.sort();
    Ok(paths)
}

fn require_regular_release_file(path: &Path) -> Result<(), String> {
    let metadata = fs::symlink_metadata(path)
        .map_err(|err| format!("release subject {} is unavailable: {err}", path.display()))?;
    if !metadata.file_type().is_file() {
        return Err(format!("release subject is not a regular file: {}", path.display()));
    }
    Ok(())
}

fn require_receipt_digest(
    assembly: &serde_json::Value,
    field: &str,
    actual: &str,
) -> Result<(), String> {
    let expected = assembly
        .get(field)
        .and_then(|value| value.get("sha256"))
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| format!("assembly receipt has no '{field}.sha256'"))?;
    if expected != actual {
        return Err(format!(
            "assembly receipt '{field}' digest mismatch: expected '{expected}', actual '{actual}'"
        ));
    }
    Ok(())
}

fn validate_sha256sums_subjects(
    path: &Path,
    subjects: &[FinalServerSubject],
) -> Result<(), String> {
    let text = fs::read_to_string(path)
        .map_err(|err| format!("failed to read {}: {err}", path.display()))?;
    let mut observed = std::collections::BTreeMap::new();
    for (index, line) in text.lines().enumerate() {
        let Some((sha, name)) = line.split_once("  ") else {
            return Err(format!("SHA256SUMS line {} is malformed", index + 1));
        };
        if sha.len() != 64 || !sha.bytes().all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase()) {
            return Err(format!("SHA256SUMS line {} has a non-canonical sha256", index + 1));
        }
        if observed.insert(name.to_string(), sha.to_string()).is_some() {
            return Err(format!("SHA256SUMS duplicates '{name}'"));
        }
    }
    for subject in subjects.iter().filter(|subject| subject.kind != "checksums") {
        match observed.remove(&subject.name) {
            Some(sha) if sha == subject.sha256 => {}
            Some(sha) => {
                return Err(format!(
                    "SHA256SUMS digest mismatch for '{}': expected '{}', observed '{sha}'",
                    subject.name, subject.sha256
                ));
            }
            None => return Err(format!("SHA256SUMS omits '{}'", subject.name)),
        }
    }
    if !observed.is_empty() {
        return Err(format!(
            "SHA256SUMS contains unexpected subjects: {:?}",
            observed.keys().collect::<Vec<_>>()
        ));
    }
    Ok(())
}

fn validate_final_server_staging_entries(
    dist_dir: &Path,
    version: &str,
    subjects: &[FinalServerSubject],
) -> Result<(), String> {
    let subject_names = subjects
        .iter()
        .map(|subject| subject.name.as_str())
        .collect::<std::collections::BTreeSet<_>>();
    let inventory_name = format!("ripr-server-final-subjects-v{version}.json");
    let attestation_name = format!("ripr-server-attestation-v{version}.receipt.json");
    let mut allowed_internal = std::collections::BTreeSet::new();
    allowed_internal.insert(format!("ripr-server-assembly-v{version}.receipt.json"));
    allowed_internal.insert(inventory_name.clone());
    allowed_internal.insert(attestation_name.clone());
    for subject in subjects.iter().filter(|subject| subject.kind == "server_archive") {
        allowed_internal.insert(format!("{}.sha256", subject.name));
    }
    for target in release_server_target_set() {
        allowed_internal.insert(format!("ripr-server-v{version}-{target}.receipt.json"));
    }
    for entry in fs::read_dir(dist_dir)
        .map_err(|err| format!("failed to read {}: {err}", dist_dir.display()))?
    {
        let path = entry
            .map_err(|err| format!("failed to read {} entry: {err}", dist_dir.display()))?
            .path();
        let metadata = fs::symlink_metadata(&path)
            .map_err(|err| format!("failed to inspect {}: {err}", path.display()))?;
        if !metadata.file_type().is_file() {
            return Err(format!("non-regular final server staging entry '{}'", path.display()));
        }
        let name = path
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or_else(|| format!("non-UTF8 final server staging entry '{}'", path.display()))?;
        if subject_names.contains(name) || allowed_internal.contains(name) {
            continue;
        }
        return Err(format!("unexpected final server staging entry '{name}'"));
    }
    Ok(())
}

pub(crate) fn release_upload_assets(args: &[String]) -> Result<(), String> {
    let version = normalize_product_version(&required_release_arg(args, "version", "RAW_VERSION")?)?;
    let attestation_path = final_server_attestation_receipt_path(&version);
    let attestation_text = fs::read_to_string(&attestation_path).map_err(|err| {
        format!(
            "verified attestation receipt {} is required before release upload: {err}",
            attestation_path.display()
        )
    })?;
    let attestation: serde_json::Value = serde_json::from_str(&attestation_text)
        .map_err(|err| format!("malformed server attestation receipt: {err}"))?;
    if attestation.get("disposition").and_then(serde_json::Value::as_str) != Some("attested")
        || attestation
            .get("release_upload_eligible")
            .and_then(serde_json::Value::as_bool)
            != Some(true)
    {
        return Err("release upload is blocked until final subjects are attested and verified".to_string());
    }
    let producer = attestation
        .get("producer_identity")
        .ok_or_else(|| "server attestation receipt has no producer_identity".to_string())?;
    if producer.get("attestation_action").and_then(serde_json::Value::as_str)
        != Some(ATTESTATION_ACTION)
    {
        return Err("server attestation receipt action identity is not the pinned release action".to_string());
    }
    let expected_repository = required_github_identity("GITHUB_REPOSITORY")?;
    let expected_workflow_ref = required_github_identity("GITHUB_WORKFLOW_REF")?;
    let expected_sha = required_github_identity("GITHUB_SHA")?;
    let expected_ref = required_github_identity("GITHUB_REF")?;
    let expected_run_id = required_github_identity("GITHUB_RUN_ID")?;
    let expected_run_attempt = required_github_identity("GITHUB_RUN_ATTEMPT")?;
    if producer.get("repository").and_then(serde_json::Value::as_str)
        != Some(expected_repository.as_str())
    {
        return Err("server attestation receipt repository differs from the upload workflow".to_string());
    }
    if producer.get("workflow_ref").and_then(serde_json::Value::as_str)
        != Some(expected_workflow_ref.as_str())
    {
        return Err("server attestation receipt workflow ref differs from the upload workflow".to_string());
    }
    if producer.get("candidate_sha").and_then(serde_json::Value::as_str)
        != Some(expected_sha.as_str())
    {
        return Err("server attestation receipt candidate SHA differs from the upload workflow".to_string());
    }
    if producer.get("git_ref").and_then(serde_json::Value::as_str)
        != Some(expected_ref.as_str())
    {
        return Err("server attestation receipt ref differs from the upload workflow".to_string());
    }
    if producer.get("run_id").and_then(serde_json::Value::as_str)
        != Some(expected_run_id.as_str())
    {
        return Err("server attestation receipt run ID differs from the upload workflow".to_string());
    }
    if producer.get("run_attempt").and_then(serde_json::Value::as_str)
        != Some(expected_run_attempt.as_str())
    {
        return Err("server attestation receipt run attempt differs from the upload workflow".to_string());
    }
    let tag = format!("v{version}");
    if !command_success_owned(
        "gh",
        &["release".to_string(), "view".to_string(), tag.clone()],
    )? {
        run_owned(
            "gh",
            &[
                "release".to_string(),
                "create".to_string(),
                tag.clone(),
                "--title".to_string(),
                format!("ripr {version}"),
            ],
        )?;
    }

    let mut upload_args = vec!["release".to_string(), "upload".to_string(), tag];
    for path in release_server_public_asset_paths(Path::new("dist"), &version)? {
        upload_args.push(path.to_string_lossy().to_string());
    }
    upload_args.push("--clobber".to_string());
    run_owned("gh", &upload_args)
}

pub(crate) fn release_server_public_asset_paths(
    dist_dir: &Path,
    version: &str,
) -> Result<Vec<PathBuf>, String> {
    final_server_subject_paths(dist_dir, version)
}
#[cfg(test)]
mod final_subject_tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn scratch(name: &str) -> Result<PathBuf, String> {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|err| format!("clock before epoch: {err}"))?
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "ripr-release-server-{name}-{}-{nanos}",
            std::process::id()
        ));
        fs::create_dir_all(&path)
            .map_err(|err| format!("create scratch {}: {err}", path.display()))?;
        Ok(path)
    }

    fn subject(name: &str, kind: &str, bytes: &[u8]) -> FinalServerSubject {
        FinalServerSubject {
            name: name.to_string(),
            kind: kind.to_string(),
            target: None,
            source_role: "fixture".to_string(),
            mode: 0o644,
            size: u64::try_from(bytes.len()).unwrap_or(u64::MAX),
            sha256: sha256_bytes(bytes),
            source_receipt: "fixture.receipt.json".to_string(),
        }
    }

    #[test]
    fn final_subject_checksum_contract_rejects_missing_extra_and_drift() -> Result<(), String> {
        let root = scratch("checksums")?;
        let sums = root.join("SHA256SUMS");
        let archive = subject("ripr-server-v0.11.0-test.tar.gz", "server_archive", b"archive");
        let manifest = subject("ripr-server-manifest-v0.11.0.json", "server_manifest", b"manifest");
        let checksums = subject("SHA256SUMS", "checksums", b"not-self-covered");
        let subjects = vec![archive.clone(), manifest.clone(), checksums];

        fs::write(
            &sums,
            format!(
                "{}  {}\n{}  {}\n",
                archive.sha256, archive.name, manifest.sha256, manifest.name
            ),
        )
        .map_err(|err| err.to_string())?;
        validate_sha256sums_subjects(&sums, &subjects)?;

        fs::write(&sums, format!("{}  {}\n", archive.sha256, archive.name))
            .map_err(|err| err.to_string())?;
        let Err(missing) = validate_sha256sums_subjects(&sums, &subjects) else {
            return Err("missing manifest row must reject".to_string());
        };
        assert!(missing.contains("omits"), "{missing}");

        fs::write(
            &sums,
            format!(
                "{}  {}\n{}  {}\n{}  extra.bin\n",
                archive.sha256,
                archive.name,
                manifest.sha256,
                manifest.name,
                sha256_bytes(b"extra")
            ),
        )
        .map_err(|err| err.to_string())?;
        let Err(extra) = validate_sha256sums_subjects(&sums, &subjects) else {
            return Err("unexpected checksum subject must reject".to_string());
        };
        assert!(extra.contains("unexpected subjects"), "{extra}");

        fs::write(
            &sums,
            format!(
                "{}  {}\n{}  {}\n",
                sha256_bytes(b"changed"),
                archive.name,
                manifest.sha256,
                manifest.name
            ),
        )
        .map_err(|err| err.to_string())?;
        let Err(drift) = validate_sha256sums_subjects(&sums, &subjects) else {
            return Err("changed digest must reject".to_string());
        };
        assert!(drift.contains("digest mismatch"), "{drift}");

        fs::remove_dir_all(&root).map_err(|err| err.to_string())
    }

    #[test]
    fn final_subject_staging_rejects_unexpected_and_non_regular_entries() -> Result<(), String> {
        let root = scratch("staging")?;
        let version = "0.11.0";
        let subject_name = "ripr-server-v0.11.0-test.tar.gz";
        fs::write(root.join(subject_name), b"archive").map_err(|err| err.to_string())?;
        fs::write(
            root.join(format!("ripr-server-assembly-v{version}.receipt.json")),
            b"{}",
        )
        .map_err(|err| err.to_string())?;
        let subjects = vec![subject(subject_name, "server_archive", b"archive")];
        validate_final_server_staging_entries(&root, version, &subjects)?;

        fs::write(root.join("unexpected.log"), b"no").map_err(|err| err.to_string())?;
        let Err(extra) = validate_final_server_staging_entries(&root, version, &subjects) else {
            return Err("unexpected staging file must reject".to_string());
        };
        assert!(extra.contains("unexpected final server staging entry"), "{extra}");
        fs::remove_file(root.join("unexpected.log")).map_err(|err| err.to_string())?;

        fs::write(root.join("rogue.sha256"), b"no").map_err(|err| err.to_string())?;
        let Err(rogue_sidecar) = validate_final_server_staging_entries(&root, version, &subjects)
        else {
            return Err("unowned checksum sidecar must reject".to_string());
        };
        assert!(
            rogue_sidecar.contains("unexpected final server staging entry"),
            "{rogue_sidecar}"
        );
        fs::remove_file(root.join("rogue.sha256")).map_err(|err| err.to_string())?;

        fs::create_dir(root.join("unexpected-dir")).map_err(|err| err.to_string())?;
        let Err(non_regular) = validate_final_server_staging_entries(&root, version, &subjects) else {
            return Err("non-regular staging entry must reject".to_string());
        };
        assert!(non_regular.contains("non-regular final server staging entry"), "{non_regular}");

        fs::remove_dir_all(&root).map_err(|err| err.to_string())
    }
}
