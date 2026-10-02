//! Final-byte inventory preparation. This module cannot attest or publish.
//! The existing assembler and uploader allowlist remain the only subject owners.
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::Read;
use std::path::{Component, Path, PathBuf};

use serde::Serialize;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

use super::release_server::{
    hex_lower, normalize_product_version, release_server_public_asset_paths,
    release_server_target_set, release_server_target_set_digest, render_release_server_assembly,
    sha256_bytes, write_release_server_outputs_transactional,
};

const INVENTORY_SCHEMA: &str = "ripr.release_final_server_subject_inventory.v1";
const INPUTS_SCHEMA: &str = "ripr.release_final_server_provenance_inputs.v1";
const RECEIPT_SCHEMA: &str = "ripr.release_final_server_subject_receipt.v1";
const MAX_STAGING_FILES: usize = 64;
const MAX_METADATA_BYTES: u64 = 4 * 1024 * 1024;

struct Options {
    version: String,
    repository: String,
    candidate_sha: String,
    candidate_tree: String,
    dist: PathBuf,
    output: PathBuf,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
struct FileIdentity {
    size: u64,
    sha256: String,
    mode: Option<u32>,
    read_only: bool,
}

#[derive(Serialize)]
struct Observation {
    name: String,
    file: Option<FileIdentity>,
    error: Option<String>,
}

type Snapshot = BTreeMap<String, FileIdentity>;

struct Packet {
    inventory: String,
    provenance_inputs: String,
    subject_checksums: String,
}

pub(crate) fn release_final_server_subjects(args: &[String]) -> Result<(), String> {
    let options = parse_options(args)?;
    let output = fresh_output(&options.dist, &options.output)?;
    let (snapshot, observations, mut failures) = snapshot_directory(&options.dist);
    let packet = if failures.is_empty() {
        match prepare_packet(&options, &snapshot) {
            Ok(packet) => {
                let (after, _, recheck_failures) = snapshot_directory(&options.dist);
                failures.extend(recheck_failures);
                if after != snapshot {
                    failures.push("staging file identity changed during final inventory".into());
                }
                if failures.is_empty() {
                    Some(packet)
                } else {
                    None
                }
            }
            Err(error) => {
                failures.push(error);
                None
            }
        }
    } else {
        None
    };
    let disposition = if packet.is_some() {
        "inventoried"
    } else {
        "rejected"
    };
    let inventory_digest = packet
        .as_ref()
        .map(|value| sha256_bytes(value.inventory.as_bytes()));
    let mut expected_targets = release_server_target_set().to_vec();
    expected_targets.sort_unstable();
    let receipt = json!({
        "schema": RECEIPT_SCHEMA,
        "producer": "xtask release-final-server-subjects",
        "disposition": disposition,
        "repository": options.repository,
        "candidate_sha": options.candidate_sha,
        "candidate_tree": options.candidate_tree,
        "product_version": options.version,
        "inventory_sha256": inventory_digest,
        "expected_targets": expected_targets,
        "observed_staging_files": observations,
        "failures": failures,
        "credential_requested": false,
        "attestation_attempted": false,
        "provenance_verified": false,
        "release_upload_eligible": false,
        "publication_mutation_attempted": false,
        "next_required_transition": "pinned producer action, genuine verifier, and live upload admission",
        "non_claims": ["inventory is not provenance", "no release or channel authority", "no final candidate qualification"],
    });
    let receipt_text = pretty(&receipt)?;
    let mut markdown = format!(
        "# Final server subject preparation\n\nDisposition: {disposition}\n\nProvenance verified: false\nRelease upload eligible: false\nPublication attempted: false\n\n"
    );
    if let Some(digest) = inventory_digest {
        markdown.push_str(&format!("Inventory SHA-256: `{digest}`\n\n"));
    }
    for failure in &failures {
        markdown.push_str(&format!("- {}\n", failure.replace(['\n', '\r'], " ")));
    }
    markdown.push_str("\nNext: pin the producer action, obtain genuine cryptographic verification, and make live upload consume that admission. This preparation cannot unlock publication.\n");
    let mut files = Vec::new();
    if let Some(packet) = packet {
        files.extend([
            (output.join("final-server-subjects.json"), packet.inventory),
            (
                output.join("final-server-provenance-inputs.json"),
                packet.provenance_inputs,
            ),
            (
                output.join("final-server-subjects.sha256"),
                packet.subject_checksums,
            ),
        ]);
    }
    // Install the machine receipt last. An interrupted write must not present
    // an inventoried marker before its digest-bound preparation files exist.
    files.extend([
        (output.join("final-server-subjects.receipt.md"), markdown),
        (
            output.join("final-server-subjects.receipt.json"),
            receipt_text,
        ),
    ]);
    let writes = files
        .iter()
        .map(|(path, text)| (path.as_path(), text.as_str()))
        .collect::<Vec<_>>();
    write_release_server_outputs_transactional(&writes)?;
    if failures.is_empty() {
        eprintln!(
            "prepared final server subjects in {}; publication remains ineligible",
            output.display()
        );
        Ok(())
    } else {
        Err(format!(
            "final server subject preparation rejected; retained receipt in {}: {}",
            output.display(),
            failures.join("; ")
        ))
    }
}

fn parse_options(args: &[String]) -> Result<Options, String> {
    let allowed = [
        "--version",
        "--repository",
        "--candidate-sha",
        "--candidate-tree",
        "--dist",
        "--out",
    ];
    let mut fields = BTreeMap::new();
    let mut arguments = args.iter();
    while let Some(flag) = arguments.next() {
        if !allowed.contains(&flag.as_str()) {
            return Err(format!(
                "unknown final-subject option `{flag}`; this command cannot request attestation or publication"
            ));
        }
        let value = arguments
            .next()
            .ok_or_else(|| format!("missing value for `{flag}`"))?;
        if value.is_empty() || value.starts_with("--") || value.chars().any(char::is_control) {
            return Err(format!("invalid value for `{flag}`"));
        }
        if fields.insert(flag.as_str(), value.clone()).is_some() {
            return Err(format!("duplicate final-subject option `{flag}`"));
        }
    }
    let mut required = |flag| {
        fields
            .remove(flag)
            .ok_or_else(|| format!("missing final-subject option `{flag}`"))
    };
    let version = normalize_product_version(&required("--version")?)?;
    let repository = required("--repository")?;
    if repository != "EffortlessMetrics/ripr" {
        return Err(
            "final server subjects require the source repository EffortlessMetrics/ripr".into(),
        );
    }
    let candidate_sha = required("--candidate-sha")?;
    let candidate_tree = required("--candidate-tree")?;
    for (label, value) in [
        ("candidate SHA", &candidate_sha),
        ("candidate tree", &candidate_tree),
    ] {
        require_digest(value, 40, label)?;
    }
    Ok(Options {
        version,
        repository,
        candidate_sha,
        candidate_tree,
        dist: PathBuf::from(required("--dist")?),
        output: PathBuf::from(required("--out")?),
    })
}

fn require_digest(value: &str, length: usize, label: &str) -> Result<(), String> {
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

fn fresh_output(dist: &Path, output: &Path) -> Result<PathBuf, String> {
    for path in [dist, output] {
        if path
            .components()
            .any(|part| matches!(part, Component::ParentDir))
        {
            return Err("staging/output paths must not contain parent traversal".into());
        }
    }
    let metadata = fs::symlink_metadata(dist)
        .map_err(|error| format!("inspect staging directory: {error}"))?;
    if !metadata.file_type().is_dir() {
        return Err("staging root must be a real directory, not a symlink".into());
    }
    let staging = fs::canonicalize(dist).map_err(|error| error.to_string())?;
    let name = output
        .file_name()
        .ok_or("output must name a fresh directory")?;
    let parent = output
        .parent()
        .filter(|path| !path.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let parent = fs::canonicalize(parent)
        .map_err(|error| format!("output parent must already exist: {error}"))?;
    let output = parent.join(name);
    if output.starts_with(&staging) || staging.starts_with(&output) {
        return Err("output must be outside the staging directory".into());
    }
    fs::create_dir(&output)
        .map_err(|error| format!("output must be a fresh directory: {error}"))?;
    Ok(output)
}

fn snapshot_directory(dist: &Path) -> (Snapshot, Vec<Observation>, Vec<String>) {
    let mut snapshot = Snapshot::new();
    let mut observations = Vec::new();
    let mut failures = Vec::new();
    match fs::symlink_metadata(dist) {
        Ok(metadata) if metadata.file_type().is_dir() => {}
        _ => {
            return (
                snapshot,
                observations,
                vec!["staging root is unavailable, symlinked, or non-directory".into()],
            );
        }
    }
    let entries = match fs::read_dir(dist) {
        Ok(entries) => entries,
        Err(error) => {
            return (
                snapshot,
                observations,
                vec![format!("read staging directory: {error}")],
            );
        }
    };
    for (index, entry) in entries.enumerate() {
        if index >= MAX_STAGING_FILES {
            failures.push(format!(
                "staging directory exceeds the {MAX_STAGING_FILES}-file bound"
            ));
            break;
        }
        let entry = match entry {
            Ok(entry) => entry,
            Err(error) => {
                failures.push(format!("read staging entry: {error}"));
                continue;
            }
        };
        let filename = entry.file_name();
        let name = filename
            .to_str()
            .map(String::from)
            .unwrap_or_else(|| format!("{filename:?}"));
        let result = if filename.to_str().is_none() || !safe_name(&name) {
            Err("staging names must be literal, single-component ASCII basenames".into())
        } else {
            let metadata_limit =
                (name.ends_with(".json") || name.ends_with(".sha256") || name == "SHA256SUMS")
                    .then_some(MAX_METADATA_BYTES);
            snapshot_file(&entry.path(), metadata_limit)
        };
        match result {
            Ok(file) => {
                snapshot.insert(name.clone(), file.clone());
                observations.push(Observation {
                    name,
                    file: Some(file),
                    error: None,
                });
            }
            Err(error) => {
                failures.push(format!("{name}: {error}"));
                observations.push(Observation {
                    name,
                    file: None,
                    error: Some(error),
                });
            }
        }
    }
    observations.sort_by(|left, right| left.name.cmp(&right.name));
    failures.sort();
    (snapshot, observations, failures)
}

fn safe_name(name: &str) -> bool {
    !name.is_empty()
        && name != "."
        && name != ".."
        && !name.starts_with('-')
        && name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"._-+".contains(&byte))
}

fn snapshot_file(path: &Path, metadata_limit: Option<u64>) -> Result<FileIdentity, String> {
    let before = fs::symlink_metadata(path).map_err(|error| error.to_string())?;
    require_single_regular_file(&before)?;
    if let Some(limit) = metadata_limit
        && before.len() > limit
    {
        return Err(format!("metadata exceeds the {limit}-byte bound"));
    }
    let mut file = fs::File::open(path).map_err(|error| error.to_string())?;
    let opened = file.metadata().map_err(|error| error.to_string())?;
    require_single_regular_file(&opened)?;
    if !same_file(&before, &opened) {
        return Err("file changed before hashing".into());
    }
    let sha256 = hash_initial_file_bytes(&mut file, opened.len())?;
    let after = fs::symlink_metadata(path).map_err(|error| error.to_string())?;
    require_single_regular_file(&after)?;
    if !same_file(
        &opened,
        &file.metadata().map_err(|error| error.to_string())?,
    ) || !same_file(&opened, &after)
    {
        return Err("file changed during hashing".into());
    }
    Ok(FileIdentity {
        size: opened.len(),
        sha256,
        mode: file_mode(&opened),
        read_only: opened.permissions().readonly(),
    })
}

fn hash_initial_file_bytes(reader: &mut impl Read, initial_size: u64) -> Result<String, String> {
    let limit = initial_size
        .checked_add(1)
        .ok_or("initial file size cannot be bounded")?;
    let mut bounded = reader.take(limit);
    let mut hasher = Sha256::new();
    let mut observed = 0_u64;
    let mut buffer = [0_u8; 8192];
    loop {
        let count = bounded
            .read(&mut buffer)
            .map_err(|error| error.to_string())?;
        if count == 0 {
            break;
        }
        observed += count as u64;
        hasher.update(&buffer[..count]);
    }
    if observed != initial_size {
        return Err("file length changed while hashing".into());
    }
    Ok(hex_lower(&hasher.finalize()))
}

fn require_single_regular_file(metadata: &fs::Metadata) -> Result<(), String> {
    if !metadata.file_type().is_file() {
        return Err("input is a symlink or nonregular file".into());
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        if metadata.nlink() != 1 {
            return Err("hard-linked/aliased input is not admitted".into());
        }
        Ok(())
    }
    #[cfg(not(unix))]
    Err("single-link identity is unavailable on this host; use the Unix assembly runner".into())
}

fn file_mode(metadata: &fs::Metadata) -> Option<u32> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        Some(metadata.permissions().mode() & 0o7777)
    }
    #[cfg(not(unix))]
    {
        let _ = metadata;
        None
    }
}

fn same_file(left: &fs::Metadata, right: &fs::Metadata) -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        if left.dev() != right.dev() || left.ino() != right.ino() || left.nlink() != right.nlink() {
            return false;
        }
    }
    left.len() == right.len()
        && left.modified().ok() == right.modified().ok()
        && file_mode(left) == file_mode(right)
        && left.permissions() == right.permissions()
}

fn prepare_packet(options: &Options, snapshot: &Snapshot) -> Result<Packet, String> {
    let accepted =
        render_release_server_assembly(&options.dist, &options.version, &options.repository)?;
    let manifest_name = format!("ripr-server-manifest-v{}.json", options.version);
    let assembly_name = format!("ripr-server-assembly-v{}.receipt.json", options.version);
    for (name, bytes) in [
        (&manifest_name, &accepted.manifest),
        (&"SHA256SUMS".to_string(), &accepted.checksums),
        (&assembly_name, &accepted.assembly_receipt),
    ] {
        let file = snapshot
            .get(name)
            .ok_or_else(|| format!("missing final assembled file `{name}`"))?;
        if file.size != bytes.len() as u64 || file.sha256 != sha256_bytes(bytes.as_bytes()) {
            return Err(format!(
                "final `{name}` bytes differ from the accepted assembler output"
            ));
        }
    }
    let assembly: Value =
        serde_json::from_str(&accepted.assembly_receipt).map_err(|error| error.to_string())?;
    let identity = &assembly["build_identity"];
    for (key, expected) in [
        ("repository", &options.repository),
        ("candidate_sha", &options.candidate_sha),
        ("candidate_tree", &options.candidate_tree),
    ] {
        if identity[key].as_str() != Some(expected.as_str()) {
            return Err(format!(
                "accepted assembly {key} does not match the expected source identity"
            ));
        }
    }
    for key in ["toolchain_file_sha256", "cargo_lock_sha256"] {
        require_digest(
            identity[key]
                .as_str()
                .ok_or_else(|| format!("missing assembly {key}"))?,
            64,
            key,
        )?;
    }
    if identity["locked"] != true
        || identity["profile"] != "release"
        || identity["features"] != json!([])
    {
        return Err(
            "assembly does not use the current locked release-profile/no-extra-feature contract"
                .into(),
        );
    }
    let manifest: Value =
        serde_json::from_str(&accepted.manifest).map_err(|error| error.to_string())?;
    let assets = manifest["assets"]
        .as_object()
        .ok_or("accepted manifest has no assets")?;
    let mut roles = BTreeMap::<String, Value>::new();
    let source_receipt = |name: &str| -> Result<Value, String> {
        Ok(
            json!({ "name": name, "sha256": snapshot.get(name).ok_or_else(|| format!("missing source receipt `{name}`"))?.sha256 }),
        )
    };
    for (target, asset) in assets {
        let name = asset["subject"]
            .as_str()
            .ok_or("accepted archive has no subject")?;
        let receipt = asset["receipt"]["path"]
            .as_str()
            .ok_or("accepted archive has no receipt path")?;
        let sidecar = format!("{name}.sha256");
        let archive = snapshot
            .get(name)
            .ok_or_else(|| format!("missing archive `{name}`"))?;
        let checksum = snapshot
            .get(&sidecar)
            .ok_or_else(|| format!("missing archive checksum `{sidecar}`"))?;
        if checksum.sha256 != sha256_bytes(format!("{}\n", archive.sha256).as_bytes()) {
            return Err(format!(
                "archive sidecar `{sidecar}` is not the exact canonical checksum"
            ));
        }
        roles.insert(name.into(), json!({ "kind": "server_archive", "target": target, "source_receipt": source_receipt(receipt)? }));
        roles.insert(sidecar, json!({ "kind": "archive_checksum", "target": target, "source_receipt": source_receipt(receipt)? }));
    }
    roles.insert(
        manifest_name,
        json!({ "kind": "server_manifest", "source_receipt": source_receipt(&assembly_name)? }),
    );
    roles.insert(
        "SHA256SUMS".into(),
        json!({ "kind": "aggregate_checksum", "source_receipt": source_receipt(&assembly_name)? }),
    );
    let public_paths = release_server_public_asset_paths(&options.dist, &options.version)?;
    let subjects = bind_upload_paths(&options.dist, &public_paths, &roles, snapshot)?;
    let inventory = pretty(&json!({
        "schema": INVENTORY_SCHEMA,
        "producer": "xtask release-final-server-subjects",
        "product_version": options.version,
        "build_identity": identity,
        "target_set_digest": release_server_target_set_digest(),
        "assembly_receipt": source_receipt(&assembly_name)?,
        "subject_set_contract": "current_server_upload_allowlist_with_archive_checksum_sidecars",
        "subjects": subjects,
        "provenance_verified": false,
        "release_upload_eligible": false,
    }))?;
    let mut subject_checksums = String::new();
    let mut attestation_subjects = Vec::new();
    for subject in &subjects {
        let name = subject["name"]
            .as_str()
            .ok_or("prepared subject has no name")?;
        let digest = subject["sha256"]
            .as_str()
            .ok_or("prepared subject has no digest")?;
        subject_checksums.push_str(&format!("{digest}  {name}\n"));
        attestation_subjects.push(json!({ "name": name, "digest": { "sha256": digest } }));
    }
    let provenance_inputs = pretty(&json!({
        "schema": INPUTS_SCHEMA,
        "inventory_sha256": sha256_bytes(inventory.as_bytes()),
        "subject_checksums_sha256": sha256_bytes(subject_checksums.as_bytes()),
        "repository": options.repository,
        "candidate_sha": options.candidate_sha,
        "candidate_tree": options.candidate_tree,
        "subjects": attestation_subjects,
        "execution_state": "not_requested",
        "action_identity": null,
        "verification_observed": false,
        "release_upload_eligible": false,
        "required_next_inputs": ["reviewed full-SHA action identity", "authorized producer workflow/run/ref", "genuine per-subject cryptographic verification"],
    }))?;
    Ok(Packet {
        inventory,
        provenance_inputs,
        subject_checksums,
    })
}

fn bind_upload_paths(
    dist: &Path,
    paths: &[PathBuf],
    roles: &BTreeMap<String, Value>,
    snapshot: &Snapshot,
) -> Result<Vec<Value>, String> {
    let mut names = BTreeSet::new();
    for path in paths {
        let name = path
            .file_name()
            .and_then(|value| value.to_str())
            .ok_or("upload path has no UTF-8 basename")?;
        if !safe_name(name) || path != &dist.join(name) || !names.insert(name.to_string()) {
            return Err("upload paths contain an unsafe or duplicate subject".into());
        }
    }
    if names != roles.keys().cloned().collect() {
        return Err("uploader subject set differs from the exact accepted role set".into());
    }
    names
        .into_iter()
        .map(|name| {
            let file = snapshot
                .get(&name)
                .ok_or_else(|| format!("unbound upload subject `{name}`"))?;
            let mut subject = roles
                .get(&name)
                .cloned()
                .ok_or("missing upload subject role")?;
            subject["name"] = name.into();
            subject["size"] = file.size.into();
            subject["sha256"] = file.sha256.clone().into();
            subject["mode"] = json!(file.mode);
            subject["read_only"] = file.read_only.into();
            Ok(subject)
        })
        .collect()
}

fn pretty(value: &Value) -> Result<String, String> {
    serde_json::to_string_pretty(value)
        .map(|text| format!("{text}\n"))
        .map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn release_final_subject_inventory_binds_every_upload_path() -> Result<(), String> {
        let dist = Path::new("dist");
        let roles = BTreeMap::from([("archive".into(), json!({ "kind": "server_archive" }))]);
        let snapshot = BTreeMap::from([(
            "archive".into(),
            FileIdentity {
                size: 7,
                sha256: "a".repeat(64),
                mode: Some(0o644),
                read_only: false,
            },
        )]);
        assert_eq!(
            bind_upload_paths(dist, &[dist.join("archive")], &roles, &snapshot)?.len(),
            1
        );
        for (paths, expected) in [
            (
                vec![],
                "uploader subject set differs from the exact accepted role set",
            ),
            (
                vec![dist.join("archive"), dist.join("extra")],
                "uploader subject set differs from the exact accepted role set",
            ),
            (
                vec![dist.join("archive"), dist.join("archive")],
                "upload paths contain an unsafe or duplicate subject",
            ),
            (
                vec![Path::new("another-directory").join("archive")],
                "upload paths contain an unsafe or duplicate subject",
            ),
            (
                vec![dist.join("../archive")],
                "upload paths contain an unsafe or duplicate subject",
            ),
        ] {
            assert_eq!(
                bind_upload_paths(dist, &paths, &roles, &snapshot)
                    .err()
                    .as_deref(),
                Some(expected),
                "{paths:?}"
            );
        }
        assert_eq!(
            bind_upload_paths(dist, &[dist.join("archive")], &roles, &Snapshot::new())
                .err()
                .as_deref(),
            Some("unbound upload subject `archive`")
        );
        Ok(())
    }

    #[test]
    fn release_final_subject_inventory_hashing_stops_at_initial_size_plus_one() -> Result<(), String>
    {
        let mut grown = std::io::Cursor::new(vec![0_u8; 4096]);
        assert_eq!(
            hash_initial_file_bytes(&mut grown, 16).err().as_deref(),
            Some("file length changed while hashing")
        );
        assert_eq!(
            grown.position(),
            17,
            "a growing input must not be streamed to its later EOF"
        );
        let mut shortened = std::io::Cursor::new(vec![0_u8; 8]);
        assert_eq!(
            hash_initial_file_bytes(&mut shortened, 16).err().as_deref(),
            Some("file length changed while hashing")
        );
        let mut exact = std::io::Cursor::new(vec![0_u8; 16]);
        assert_eq!(
            hash_initial_file_bytes(&mut exact, 16)?,
            sha256_bytes(&[0_u8; 16])
        );
        Ok(())
    }
}
