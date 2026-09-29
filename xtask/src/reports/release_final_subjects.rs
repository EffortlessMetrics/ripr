use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use serde_json::Value;

use super::release_server::{
    normalize_product_version, release_server_assets, release_server_public_asset_paths,
    release_server_target_set_digest, required_release_arg, sha256_bytes, sha256_file,
    validate_configured_release_server_targets, validate_release_server_receipts,
};

pub(crate) fn release_server_final_subjects(args: &[String]) -> Result<(), String> {
    let version = normalize_product_version(&required_release_arg(args, "version", "RAW_VERSION")?)?;
    let dist_dir = Path::new("dist");

    // Reuse the existing producer/verifier contracts first. This command does
    // not reconstruct archive, target, manifest, or build identity authority.
    let _validated_receipts = validate_release_server_receipts(dist_dir, &version)?;
    let assets = release_server_assets(dist_dir, &version)?;
    validate_configured_release_server_targets(&assets)?;

    let manifest_name = format!("ripr-server-manifest-v{version}.json");
    let manifest_path = dist_dir.join(&manifest_name);
    let checksum_path = dist_dir.join("SHA256SUMS");
    let assembly_name = format!("ripr-server-assembly-v{version}.receipt.json");
    let assembly_path = dist_dir.join(&assembly_name);

    for path in [&manifest_path, &checksum_path, &assembly_path] {
        require_regular_file(path)?;
    }

    let checksum_text = fs::read_to_string(&checksum_path)
        .map_err(|err| format!("failed to read {}: {err}", checksum_path.display()))?;
    let listed = parse_sha256sums(&checksum_text)?;

    let mut subjects = Vec::new();
    let mut expected_listed = BTreeSet::new();
    for asset in &assets {
        let path = dist_dir.join(&asset.file_name);
        require_regular_file(&path)?;
        let metadata = fs::metadata(&path)
            .map_err(|err| format!("failed to stat {}: {err}", path.display()))?;
        let digest = sha256_file(&path)?;
        require_listed_digest(&listed, &asset.file_name, &digest)?;
        expected_listed.insert(asset.file_name.clone());
        subjects.push(serde_json::json!({
            "name": asset.file_name,
            "kind": "server_archive",
            "target": asset.target,
            "size": metadata.len(),
            "sha256": digest,
        }));
    }

    let manifest_metadata = fs::metadata(&manifest_path)
        .map_err(|err| format!("failed to stat {}: {err}", manifest_path.display()))?;
    let manifest_sha256 = sha256_file(&manifest_path)?;
    require_listed_digest(&listed, &manifest_name, &manifest_sha256)?;
    expected_listed.insert(manifest_name.clone());
    subjects.push(serde_json::json!({
        "name": manifest_name,
        "kind": "server_manifest",
        "size": manifest_metadata.len(),
        "sha256": manifest_sha256,
    }));

    let observed_listed = listed.keys().cloned().collect::<BTreeSet<_>>();
    if observed_listed != expected_listed {
        return Err(format!(
            "SHA256SUMS subject set mismatch: expected {:?}, observed {:?}",
            expected_listed, observed_listed
        ));
    }

    let checksum_metadata = fs::metadata(&checksum_path)
        .map_err(|err| format!("failed to stat {}: {err}", checksum_path.display()))?;
    let checksum_sha256 = sha256_file(&checksum_path)?;
    subjects.push(serde_json::json!({
        "name": "SHA256SUMS",
        "kind": "checksum_manifest",
        "size": checksum_metadata.len(),
        "sha256": checksum_sha256,
    }));

    // The existing upload selector must agree exactly with this public subject
    // model. Per-archive .sha256 files and receipts are control inputs.
    let upload_paths = release_server_public_asset_paths(dist_dir, &version)?;
    let upload_names = upload_paths
        .iter()
        .map(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .map(str::to_string)
                .ok_or_else(|| format!("invalid release asset path {}", path.display()))
        })
        .collect::<Result<BTreeSet<_>, _>>()?;
    let expected_public = subjects
        .iter()
        .filter_map(|subject| subject["name"].as_str().map(str::to_string))
        .collect::<BTreeSet<_>>();
    if upload_names != expected_public {
        return Err(format!(
            "release upload subject set differs from final admission: upload {:?}, admitted {:?}",
            upload_names, expected_public
        ));
    }

    let assembly_text = fs::read_to_string(&assembly_path)
        .map_err(|err| format!("failed to read {}: {err}", assembly_path.display()))?;
    let assembly: Value = serde_json::from_str(&assembly_text)
        .map_err(|err| format!("malformed assembly receipt {assembly_name}: {err}"))?;
    require_assembly_identity(&assembly, &assembly_name, &version)?;

    let assembly_manifest_sha = required_json_string(&assembly, "/manifest/sha256", &assembly_name)?;
    if assembly_manifest_sha != manifest_sha256 {
        return Err("assembly receipt manifest digest does not match final manifest bytes".to_string());
    }
    let assembly_checksums_sha =
        required_json_string(&assembly, "/sha256sums/sha256", &assembly_name)?;
    if assembly_checksums_sha != checksum_sha256 {
        return Err("assembly receipt SHA256SUMS digest does not match final checksum bytes".to_string());
    }

    subjects.sort_by(|left, right| {
        left["name"]
            .as_str()
            .unwrap_or_default()
            .cmp(right["name"].as_str().unwrap_or_default())
    });
    let subject_digest_material = subjects
        .iter()
        .map(|subject| {
            format!(
                "{}:{}",
                subject["name"].as_str().unwrap_or_default(),
                subject["sha256"].as_str().unwrap_or_default()
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    let subject_set_sha256 = sha256_bytes(subject_digest_material.as_bytes());
    let assembly_sha256 = sha256_file(&assembly_path)?;
    let candidate_sha = required_json_string(&assembly, "/build_identity/candidate_sha", &assembly_name)?;
    let candidate_tree = required_json_string(&assembly, "/build_identity/candidate_tree", &assembly_name)?;
    let distribution_generation =
        required_json_string(&assembly, "/distribution_generation", &assembly_name)?;

    let report = serde_json::json!({
        "schema_version": "ripr.release_server_final_subjects.v1",
        "producer": "cargo xtask release-server-final-subjects",
        "version": version,
        "candidate": {
            "sha": candidate_sha,
            "tree": candidate_tree,
        },
        "distribution_generation": distribution_generation,
        "target_set_digest": release_server_target_set_digest(),
        "assembly_receipt": {
            "path": assembly_name,
            "sha256": assembly_sha256,
        },
        "subject_count": subjects.len(),
        "subject_set_sha256": subject_set_sha256,
        "subjects": subjects,
        "checksum_contract": {
            "path": "SHA256SUMS",
            "sha256": checksum_sha256,
            "self_listed": false,
            "covers_archives_and_manifest": true,
        },
        "attestation": {
            "required_before_upload": true,
            "performed_by_this_command": false,
        },
        "release_upload_eligible": false,
        "publication_mutation_attempted": false,
        "disposition": "admitted_pre_attestation",
        "non_claims": [
            "does not create an attestation",
            "does not authorize release upload",
            "does not create or mutate a tag or GitHub Release",
            "does not publish per-archive .sha256 sidecars or control receipts",
        ],
    });

    let out_dir = optional_arg(args, "out")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            Path::new("target")
                .join("ripr")
                .join("release-server-final-subjects")
                .join(format!("v{version}"))
        });
    fs::create_dir_all(&out_dir)
        .map_err(|err| format!("failed to create {}: {err}", out_dir.display()))?;

    let json_path = out_dir.join("final-subjects.json");
    fs::write(
        &json_path,
        format!(
            "{}\n",
            serde_json::to_string_pretty(&report)
                .map_err(|err| format!("failed to render final-subject receipt: {err}"))?
        ),
    )
    .map_err(|err| format!("failed to write {}: {err}", json_path.display()))?;

    let markdown_path = out_dir.join("final-subjects.md");
    fs::write(
        &markdown_path,
        render_markdown(
            &version,
            candidate_sha,
            candidate_tree,
            distribution_generation,
            &subject_set_sha256,
            report["subjects"].as_array().map(Vec::as_slice).unwrap_or_default(),
        ),
    )
    .map_err(|err| format!("failed to write {}: {err}", markdown_path.display()))?;

    eprintln!("wrote {}", json_path.display());
    eprintln!("wrote {}", markdown_path.display());
    Ok(())
}

fn parse_sha256sums(text: &str) -> Result<BTreeMap<String, String>, String> {
    let mut listed = BTreeMap::new();
    for (index, raw_line) in text.lines().enumerate() {
        let line = raw_line.trim_end();
        if line.is_empty() {
            continue;
        }
        let (digest, name) = line.split_once("  ").ok_or_else(|| {
            format!(
                "SHA256SUMS line {} is not in sha256-space-space-name form",
                index + 1
            )
        })?;
        if !is_sha256_hex(digest) {
            return Err(format!(
                "SHA256SUMS line {} has a non-SHA-256 digest",
                index + 1
            ));
        }
        require_flat_subject_name(name)?;
        if listed
            .insert(name.to_string(), digest.to_ascii_lowercase())
            .is_some()
        {
            return Err(format!("SHA256SUMS contains duplicate subject {name}"));
        }
    }
    Ok(listed)
}

fn require_listed_digest(
    listed: &BTreeMap<String, String>,
    name: &str,
    actual: &str,
) -> Result<(), String> {
    let listed_digest = listed
        .get(name)
        .ok_or_else(|| format!("SHA256SUMS is missing final subject {name}"))?;
    if listed_digest != actual {
        return Err(format!(
            "SHA256SUMS digest mismatch for {name}: listed {listed_digest}, actual {actual}"
        ));
    }
    Ok(())
}

fn require_regular_file(path: &Path) -> Result<(), String> {
    let metadata = fs::symlink_metadata(path)
        .map_err(|err| format!("required final subject {} is unavailable: {err}", path.display()))?;
    if !metadata.file_type().is_file() {
        return Err(format!(
            "required final subject is not a regular file: {}",
            path.display()
        ));
    }
    Ok(())
}

fn require_flat_subject_name(name: &str) -> Result<(), String> {
    let path = Path::new(name);
    let mut components = path.components();
    match (components.next(), components.next()) {
        (Some(std::path::Component::Normal(_)), None)
            if !name.contains('/') && !name.contains('\\') =>
        {
            Ok(())
        }
        _ => Err(format!("unsafe final subject name {name}")),
    }
}

fn require_assembly_identity(
    assembly: &Value,
    source: &str,
    version: &str,
) -> Result<(), String> {
    if assembly.pointer("/schema_version").and_then(Value::as_str) != Some("0.2")
        || assembly.pointer("/version").and_then(Value::as_str) != Some(version)
        || assembly
            .pointer("/placement_independent")
            .and_then(Value::as_bool)
            != Some(true)
        || assembly.pointer("/disposition").and_then(Value::as_str) != Some("assembled")
    {
        return Err(format!(
            "assembly receipt {source} is not an accepted placement-independent v{version} assembly"
        ));
    }
    Ok(())
}

fn required_json_string<'a>(
    value: &'a Value,
    pointer: &str,
    source: &str,
) -> Result<&'a str, String> {
    value
        .pointer(pointer)
        .and_then(Value::as_str)
        .ok_or_else(|| format!("{source} is missing string field {pointer}"))
}

fn optional_arg(args: &[String], flag: &str) -> Option<String> {
    let flag_name = format!("--{flag}");
    for window in args.windows(2) {
        if window[0] == flag_name {
            return Some(window[1].clone());
        }
    }
    let inline_prefix = format!("{flag_name}=");
    args.iter()
        .find_map(|arg| arg.strip_prefix(&inline_prefix).map(str::to_string))
}

fn is_sha256_hex(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn render_markdown(
    version: &str,
    candidate_sha: &str,
    candidate_tree: &str,
    distribution_generation: &str,
    subject_set_sha256: &str,
    subjects: &[Value],
) -> String {
    let mut output = format!(
        "# ripr server final subjects v{version}\n\n\
         Candidate SHA: {candidate_sha}\n\n\
         Candidate tree: {candidate_tree}\n\n\
         Distribution generation: {distribution_generation}\n\n\
         Subject set SHA-256: {subject_set_sha256}\n\n\
         Disposition: admitted_pre_attestation\n\n\
         Release upload eligible: no; provenance attestation is still required.\n\n\
         | Subject | Kind | Size | SHA-256 |\n\
         | --- | --- | ---: | --- |\n"
    );
    for subject in subjects {
        output.push_str(&format!(
            "| {} | {} | {} | {} |\n",
            subject["name"].as_str().unwrap_or_default(),
            subject["kind"].as_str().unwrap_or_default(),
            subject["size"].as_u64().unwrap_or_default(),
            subject["sha256"].as_str().unwrap_or_default(),
        ));
    }
    output.push_str(
        "\nSHA256SUMS covers the archives and manifest but intentionally does not self-list. \
         Per-archive .sha256 files and build/assembly receipts are control inputs, not public subjects.\n",
    );
    output
}


#[cfg(test)]
mod tests {
    use super::{parse_sha256sums, require_flat_subject_name};

    #[test]
    fn checksum_subjects_are_exact_flat_names() {
        let a = "a".repeat(64);
        let b = "b".repeat(64);
        let parsed = parse_sha256sums(&format!(
            "{a}  ripr-server-v0.11.0-x86_64-unknown-linux-gnu.tar.gz\n{b}  ripr-server-manifest-v0.11.0.json\n"
        ))
        .expect("valid checksum manifest");
        assert_eq!(parsed.len(), 2);
        assert_eq!(
            parsed["ripr-server-v0.11.0-x86_64-unknown-linux-gnu.tar.gz"],
            a
        );
    }

    #[test]
    fn checksum_manifest_rejects_duplicates_bad_digests_and_paths() {
        let digest = "a".repeat(64);
        assert!(parse_sha256sums(&format!(
            "{digest}  archive.tar.gz\n{digest}  archive.tar.gz\n"
        ))
        .unwrap_err()
        .contains("duplicate subject"));
        assert!(parse_sha256sums("abcd  archive.tar.gz\n")
            .unwrap_err()
            .contains("non-SHA-256"));
        for name in [
            "../archive.tar.gz",
            "nested/archive.tar.gz",
            r"nested\archive.tar.gz",
            "/tmp/archive.tar.gz",
        ] {
            assert!(require_flat_subject_name(name).is_err(), "{name}");
        }
    }
}
