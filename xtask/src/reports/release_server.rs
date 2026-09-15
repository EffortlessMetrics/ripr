use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};

use flate2::Compression;
use flate2::GzBuilder;
use flate2::read::GzDecoder;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tar::Builder;

use crate::{command_success_owned, run_output, run_owned};

pub(crate) fn release_server_archive(args: &[String]) -> Result<(), String> {
    let version = required_release_arg(args, "version", "RAW_VERSION")?;
    let target = required_release_arg(args, "target", "TARGET")?;
    let executable = required_release_arg(args, "executable", "EXECUTABLE")?;
    let archive = required_release_arg(args, "archive", "ARCHIVE")?;
    let version = normalize_release_version(&version);
    let asset_name = format!("ripr-server-v{version}-{target}.{archive}");
    let package_dir = Path::new("package");
    let dist_dir = Path::new("dist");

    if package_dir.exists() {
        fs::remove_dir_all(package_dir)
            .map_err(|err| format!("failed to remove {}: {err}", package_dir.display()))?;
    }
    fs::create_dir_all(package_dir)
        .map_err(|err| format!("failed to create {}: {err}", package_dir.display()))?;
    fs::create_dir_all(dist_dir)
        .map_err(|err| format!("failed to create {}: {err}", dist_dir.display()))?;

    let built_executable = Path::new("target")
        .join(&target)
        .join("release")
        .join(&executable);
    fs::copy(&built_executable, package_dir.join(&executable)).map_err(|err| {
        format!(
            "failed to copy {} into {}: {err}",
            built_executable.display(),
            package_dir.display()
        )
    })?;
    copy_release_file("LICENSE-MIT", package_dir)?;
    copy_release_file("LICENSE-APACHE", package_dir)?;
    fs::write(
        package_dir.join("README-server.txt"),
        release_server_readme(&version),
    )
    .map_err(|err| {
        format!(
            "failed to write {}: {err}",
            package_dir.join("README-server.txt").display()
        )
    })?;

    let asset_path = dist_dir.join(&asset_name);
    if asset_path.exists() {
        fs::remove_file(&asset_path)
            .map_err(|err| format!("failed to remove {}: {err}", asset_path.display()))?;
    }
    match archive.as_str() {
        "zip" => create_zip_archive(package_dir, &asset_path)?,
        "tar.gz" => create_tar_gz_archive(package_dir, &asset_path)?,
        other => {
            return Err(format!(
                "unsupported release server archive format `{other}`"
            ));
        }
    }

    let sha = sha256_file(&asset_path)?;
    write_release_server_receipt(
        &version,
        &target,
        &executable,
        &archive,
        package_dir,
        &asset_path,
    )?;
    fs::write(
        dist_dir.join(format!("{asset_name}.sha256")),
        format!("{sha}\n"),
    )
    .map_err(|err| {
        format!(
            "failed to write {}: {err}",
            dist_dir.join(format!("{asset_name}.sha256")).display()
        )
    })?;
    eprintln!("wrote {}", asset_path.display());
    Ok(())
}

pub(crate) fn release_server_manifest(args: &[String]) -> Result<(), String> {
    let version = required_release_arg(args, "version", "RAW_VERSION")?;
    let repository = required_release_arg(args, "repository", "REPOSITORY")?;
    let version = normalize_release_version(&version);
    let dist_dir = Path::new("dist");
    // Published as `SHA256SUMS` (the near-universal ecosystem convention) so
    // consumers can run `sha256sum -c SHA256SUMS` against the release assets.
    // The content format is unchanged (`<sha256>  <file_name>` per line).
    let sha256sums_path = dist_dir.join("SHA256SUMS");
    // Also remove any legacy `checksums.txt` left in a reused `dist/` from a
    // pre-rename run so the stale sidecar cannot linger beside — or be hashed
    // into — the new `SHA256SUMS`.
    let legacy_checksums_path = dist_dir.join("checksums.txt");
    let manifest_path = dist_dir.join(format!("ripr-server-manifest-v{version}.json"));
    let assembly_receipt_path =
        dist_dir.join(format!("ripr-server-assembly-v{version}.receipt.json"));
    if legacy_checksums_path.exists() {
        fs::remove_file(&legacy_checksums_path).map_err(|err| {
            format!(
                "failed to remove {} before validation: {err}",
                legacy_checksums_path.display()
            )
        })?;
    }
    let receipt_set = validate_release_server_receipts(dist_dir, &version)?;
    let receipt_targets = &receipt_set.targets;
    let discovered_assets = release_server_assets(dist_dir, &version)?;
    validate_configured_release_server_targets(&discovered_assets)?;
    let asset_targets = discovered_assets
        .iter()
        .map(|asset| asset.target.clone())
        .collect::<std::collections::BTreeSet<_>>();
    if *receipt_targets != asset_targets {
        return Err(format!(
            "release server receipt targets do not match archive targets: receipts {:?}, archives {:?}",
            receipt_targets, asset_targets
        ));
    }
    validate_release_server_staging_inventory(
        dist_dir,
        &version,
        &discovered_assets,
        receipt_targets,
    )?;
    let mut assets = serde_json::Map::new();
    let build_identity = &receipt_set.baseline;
    for asset in discovered_assets {
        let receipt = receipt_set
            .receipts
            .get(&asset.target)
            .ok_or_else(|| format!("missing validated receipt for target `{}`", asset.target))?;
        let sha_path = dist_dir.join(format!("{}.sha256", asset.file_name));
        let sha = read_trimmed(&sha_path)?;
        let actual_sha = sha256_file(&dist_dir.join(&asset.file_name))?;
        if sha != actual_sha {
            return Err(format!(
                "archive checksum mismatch for `{}`: sidecar `{sha}`, actual `{actual_sha}`",
                asset.file_name
            ));
        }
        let url = format!(
            "https://github.com/{repository}/releases/download/v{version}/{}",
            asset.file_name
        );
        assets.insert(
            asset.target,
            serde_json::json!({
                "url": url,
                "sha256": sha,
                "receipt": {
                    "path": receipt.path,
                    "sha256": receipt.sha256,
                    "schema_version": receipt.schema_version,
                    "target": receipt.target,
                },
                "archive": {
                    "path": receipt.archive.path,
                    "size": receipt.archive.size,
                    "sha256": receipt.archive.sha256,
                },
            }),
        );
    }

    let manifest = serde_json::json!({
        "schema_version": "0.1",
        "version": version,
        "build_identity": {
            "repository": build_identity.repository,
            "candidate_sha": build_identity.candidate_sha,
            "candidate_tree": build_identity.candidate_tree,
            "toolchain": build_identity.toolchain,
            "toolchain_file_sha256": build_identity.toolchain_file_sha256,
            "cargo_lock_sha256": build_identity.cargo_lock_sha256,
            "profile": build_identity.profile,
            "features": build_identity.features,
            "locked": build_identity.locked,
        },
        "assets": assets,
    });
    let manifest_text = serde_json::to_string_pretty(&manifest)
        .map_err(|err| format!("failed to render release server manifest: {err}"))?;
    let manifest_text = format!("{manifest_text}\n");
    let mut checksum_lines = Vec::new();
    for path in sorted_dist_files(dist_dir)? {
        let Some(file_name) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        if file_name.ends_with(".sha256")
            || file_name == "SHA256SUMS"
            || file_name == "checksums.txt"
            || file_name.ends_with(".receipt.json")
            || file_name
                == manifest_path
                    .file_name()
                    .and_then(|name| name.to_str())
                    .unwrap_or_default()
        {
            continue;
        }
        checksum_lines.push(format!("{}  {file_name}", sha256_file(&path)?));
    }
    checksum_lines.push(format!(
        "{}  {}",
        sha256_bytes(manifest_text.as_bytes()),
        manifest_path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or_default()
    ));
    let checksum_text = format!("{}\n", checksum_lines.join("\n"));
    let assembly_receipt = serde_json::json!({
        "schema_version": "0.1",
        "assembler": "xtask",
        "version": version,
        "build_identity": {
            "repository": build_identity.repository,
            "candidate_sha": build_identity.candidate_sha,
            "candidate_tree": build_identity.candidate_tree,
            "toolchain": build_identity.toolchain,
            "toolchain_file_sha256": build_identity.toolchain_file_sha256,
            "cargo_lock_sha256": build_identity.cargo_lock_sha256,
            "profile": build_identity.profile,
            "features": build_identity.features,
            "locked": build_identity.locked,
        },
        "inputs": {
            "receipt_count": receipt_set.receipts.len(),
            "receipts": receipt_set.receipts.iter().map(|(target, receipt)| serde_json::json!({
                "target": target,
                "path": receipt.path,
                "sha256": receipt.sha256,
                "schema_version": receipt.schema_version,
            })).collect::<Vec<_>>(),
        },
        "accepted_subject_count": receipt_set.targets.len(),
        "manifest": {
            "path": manifest_path.file_name().and_then(|name| name.to_str()).unwrap_or_default(),
            "size": manifest_text.len(),
            "sha256": sha256_bytes(manifest_text.as_bytes()),
        },
        "sha256sums": {
            "path": "SHA256SUMS",
            "size": checksum_text.len(),
            "sha256": sha256_bytes(checksum_text.as_bytes()),
        },
        "publication_mutation_attempted": false,
        "disposition": "assembled",
        "non_claims": ["no publication or ref mutation", "final attestation remains downstream"],
    });
    let assembly_text = serde_json::to_string_pretty(&assembly_receipt)
        .map_err(|err| format!("failed to render assembly receipt: {err}"))?;
    let assembly_text = format!("{assembly_text}\n");
    write_release_server_outputs_transactional(&[
        (&manifest_path, manifest_text.as_str()),
        (&sha256sums_path, checksum_text.as_str()),
        (&assembly_receipt_path, assembly_text.as_str()),
    ])?;
    eprintln!("wrote {}", manifest_path.display());
    eprintln!("wrote {}", sha256sums_path.display());
    eprintln!("wrote {}", assembly_receipt_path.display());
    Ok(())
}

pub(crate) fn release_upload_assets(args: &[String]) -> Result<(), String> {
    let version = normalize_release_version(&required_release_arg(args, "version", "RAW_VERSION")?);
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
    let assets = release_server_assets(dist_dir, version)?;
    let mut paths = Vec::with_capacity(assets.len() * 2 + 2);
    for asset in assets {
        paths.push(dist_dir.join(&asset.file_name));
        paths.push(dist_dir.join(format!("{}.sha256", asset.file_name)));
    }
    paths.push(dist_dir.join(format!("ripr-server-manifest-v{version}.json")));
    paths.push(dist_dir.join("SHA256SUMS"));
    for path in &paths {
        let metadata = fs::symlink_metadata(path)
            .map_err(|err| format!("release asset {} is unavailable: {err}", path.display()))?;
        if !metadata.file_type().is_file() {
            return Err(format!(
                "release asset is not a regular file: {}",
                path.display()
            ));
        }
    }
    Ok(paths)
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) struct ReleaseServerAsset {
    pub(crate) target: String,
    pub(crate) file_name: String,
}

pub(crate) fn release_server_assets(
    dist_dir: &Path,
    version: &str,
) -> Result<Vec<ReleaseServerAsset>, String> {
    let prefix = format!("ripr-server-v{version}-");
    let mut assets = Vec::new();
    for entry in fs::read_dir(dist_dir)
        .map_err(|err| format!("failed to read {}: {err}", dist_dir.display()))?
    {
        let path = entry
            .map_err(|err| format!("failed to read entry under {}: {err}", dist_dir.display()))?
            .path();
        let metadata = fs::symlink_metadata(&path)
            .map_err(|err| format!("failed to inspect {}: {err}", path.display()))?;
        if !metadata.file_type().is_file() {
            return Err(format!(
                "non-regular release server staging entry `{}`",
                path.display()
            ));
        }
        let Some(file_name) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        let Some(target_with_suffix) = file_name.strip_prefix(&prefix) else {
            continue;
        };
        let target = target_with_suffix
            .strip_suffix(".tar.gz")
            .or_else(|| target_with_suffix.strip_suffix(".zip"));
        let Some(target) = target else {
            continue;
        };
        if assets
            .iter()
            .any(|asset: &ReleaseServerAsset| asset.target == target)
        {
            return Err(format!(
                "duplicate release server target `{target}` in `{file_name}`"
            ));
        }
        assets.push(ReleaseServerAsset {
            target: target.to_string(),
            file_name: file_name.to_string(),
        });
    }
    assets.sort_by(|left, right| left.target.cmp(&right.target));
    Ok(assets)
}

pub(crate) fn validate_configured_release_server_targets(
    assets: &[ReleaseServerAsset],
) -> Result<(), String> {
    const CONFIGURED_TARGETS: [&str; 5] = [
        "x86_64-pc-windows-msvc",
        "x86_64-unknown-linux-gnu",
        "aarch64-unknown-linux-gnu",
        "x86_64-apple-darwin",
        "aarch64-apple-darwin",
    ];
    let observed = assets
        .iter()
        .map(|asset| asset.target.as_str())
        .collect::<std::collections::BTreeSet<_>>();
    for expected in CONFIGURED_TARGETS {
        if !observed.contains(expected) {
            return Err(format!(
                "missing configured release server target `{expected}`"
            ));
        }
    }
    for target in observed {
        if !CONFIGURED_TARGETS.contains(&target) {
            return Err(format!(
                "unknown configured release server target `{target}`"
            ));
        }
    }
    Ok(())
}

pub(crate) fn required_release_arg(
    args: &[String],
    flag: &str,
    env_name: &str,
) -> Result<String, String> {
    let flag_name = format!("--{flag}");
    for window in args.windows(2) {
        if window[0] == flag_name {
            return Ok(window[1].clone());
        }
    }
    let inline_prefix = format!("{flag_name}=");
    for arg in args {
        if let Some(value) = arg.strip_prefix(&inline_prefix) {
            return Ok(value.to_string());
        }
    }
    std::env::var(env_name).map_err(|err| format!("missing {flag_name} or {env_name}: {err}"))
}

pub(crate) fn normalize_release_version(version: &str) -> String {
    version.trim().trim_start_matches('v').to_string()
}

fn copy_release_file(file_name: &str, package_dir: &Path) -> Result<(), String> {
    fs::copy(file_name, package_dir.join(file_name)).map_err(|err| {
        format!(
            "failed to copy {file_name} into {}: {err}",
            package_dir.display()
        )
    })?;
    Ok(())
}

pub(crate) fn release_server_readme(version: &str) -> String {
    format!(
        "ripr server {version}\n\nThis archive contains the ripr executable used by the VS Code/Open VSX\nextension. It is distributed under MIT OR Apache-2.0."
    )
}

pub(crate) fn create_tar_gz_archive(package_dir: &Path, asset_path: &Path) -> Result<(), String> {
    let output = fs::File::create(asset_path)
        .map_err(|err| format!("failed to create {}: {err}", asset_path.display()))?;
    let encoder = GzBuilder::new()
        .mtime(0)
        .write(output, Compression::default());
    let mut builder = Builder::new(encoder);
    for path in sorted_package_files(package_dir)? {
        let name = path
            .file_name()
            .ok_or_else(|| format!("invalid package path {}", path.display()))?;
        let mut header = tar::Header::new_gnu();
        let metadata = fs::metadata(&path)
            .map_err(|err| format!("failed to stat {}: {err}", path.display()))?;
        header.set_size(metadata.len());
        header.set_mode(package_mode(name.to_string_lossy().as_ref()));
        header.set_mtime(0);
        header.set_uid(0);
        header.set_gid(0);
        header
            .set_username("")
            .map_err(|err| format!("set tar username: {err}"))?;
        header
            .set_groupname("")
            .map_err(|err| format!("set tar groupname: {err}"))?;
        header.set_cksum();
        let mut input = fs::File::open(&path)
            .map_err(|err| format!("failed to open {} for tar: {err}", path.display()))?;
        builder
            .append_data(&mut header, name, &mut input)
            .map_err(|err| format!("failed to write tar entry {}: {err}", path.display()))?;
    }
    let encoder = builder
        .into_inner()
        .map_err(|err| format!("failed to finalize tar {}: {err}", asset_path.display()))?;
    encoder
        .finish()
        .map_err(|err| format!("failed to finalize gzip {}: {err}", asset_path.display()))?;
    Ok(())
}

pub(crate) fn create_zip_archive(package_dir: &Path, asset_path: &Path) -> Result<(), String> {
    let file = fs::File::create(asset_path)
        .map_err(|err| format!("failed to create {}: {err}", asset_path.display()))?;
    let mut writer = zip::ZipWriter::new(file);
    let options: zip::write::FileOptions<'_, ()> =
        zip::write::FileOptions::default().compression_method(zip::CompressionMethod::Deflated);

    let entries = fs::read_dir(package_dir)
        .map_err(|err| format!("failed to read {}: {err}", package_dir.display()))?;
    let mut sorted: Vec<PathBuf> = entries
        .filter_map(|entry| entry.ok().map(|entry| entry.path()))
        .collect();
    sorted.sort();

    for path in sorted {
        let metadata = fs::metadata(&path)
            .map_err(|err| format!("failed to stat {}: {err}", path.display()))?;
        if !metadata.is_file() {
            return Err(format!(
                "release server package directory must be flat; found non-file `{}`",
                path.display()
            ));
        }
        let file_name = path
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or_else(|| format!("invalid file name in {}", path.display()))?
            .to_string();
        let entry_options = options.unix_permissions(package_mode(&file_name));
        writer
            .start_file(&file_name, entry_options)
            .map_err(|err| format!("failed to start zip entry {file_name}: {err}"))?;
        let mut input = fs::File::open(&path)
            .map_err(|err| format!("failed to open {} for zip: {err}", path.display()))?;
        std::io::copy(&mut input, &mut writer)
            .map_err(|err| format!("failed to write zip entry {file_name}: {err}"))?;
    }
    writer
        .finish()
        .map_err(|err| format!("failed to finalize {}: {err}", asset_path.display()))?;
    Ok(())
}

#[derive(Debug, Deserialize, Serialize, Clone)]
struct ReleaseServerBuildReceipt {
    schema_version: String,
    repository: String,
    candidate_sha: String,
    candidate_tree: String,
    toolchain: ReleaseServerToolchain,
    toolchain_file_sha256: String,
    cargo_lock_sha256: String,
    profile: String,
    features: Vec<String>,
    locked: bool,
    build_command: String,
    version: String,
    target: String,
    archive_format: String,
    executable: ReleaseServerFile,
    archive: ReleaseServerFile,
    members: Vec<ReleaseServerMember>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
struct ReleaseServerToolchain {
    rustc: String,
    cargo: String,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
struct ReleaseServerFile {
    path: String,
    size: u64,
    sha256: String,
}

#[derive(Debug, Deserialize, Eq, PartialEq, Serialize, Clone)]
struct ReleaseServerMember {
    path: String,
    kind: String,
    role: String,
    size: u64,
    sha256: String,
    mode: u32,
}

fn write_release_server_receipt(
    version: &str,
    target: &str,
    executable: &str,
    archive: &str,
    package_dir: &Path,
    archive_path: &Path,
) -> Result<(), String> {
    let executable_path = package_dir.join(executable);
    let executable_file = release_server_file(executable, &executable_path)?;
    let archive_file = release_server_file(
        archive_path
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or_else(|| format!("invalid archive path {}", archive_path.display()))?,
        archive_path,
    )?;
    let members = archive_member_inventory(archive_path, archive, executable)?;
    let identity = release_server_build_identity()?;
    let receipt = ReleaseServerBuildReceipt {
        schema_version: "0.2".to_string(),
        repository: identity.repository,
        candidate_sha: identity.candidate_sha,
        candidate_tree: identity.candidate_tree,
        toolchain: identity.toolchain,
        toolchain_file_sha256: identity.toolchain_file_sha256,
        cargo_lock_sha256: identity.cargo_lock_sha256,
        profile: "release".to_string(),
        features: Vec::new(),
        locked: true,
        build_command: format!("cargo build -p ripr --release --locked --target {target}"),
        version: version.to_string(),
        target: target.to_string(),
        archive_format: archive.to_string(),
        executable: executable_file,
        archive: archive_file,
        members,
    };
    let receipt_path =
        Path::new("dist").join(format!("ripr-server-v{version}-{target}.receipt.json"));
    if let Some(parent) = receipt_path.parent() {
        fs::create_dir_all(parent)
            .map_err(|err| format!("failed to create {}: {err}", parent.display()))?;
    }
    let text = serde_json::to_string_pretty(&receipt)
        .map_err(|err| format!("failed to render release server receipt: {err}"))?;
    fs::write(&receipt_path, format!("{text}\n"))
        .map_err(|err| format!("failed to write {}: {err}", receipt_path.display()))?;
    eprintln!("wrote {}", receipt_path.display());
    Ok(())
}

struct ReleaseServerBuildIdentity {
    repository: String,
    candidate_sha: String,
    candidate_tree: String,
    toolchain: ReleaseServerToolchain,
    toolchain_file_sha256: String,
    cargo_lock_sha256: String,
}

fn release_server_build_identity() -> Result<ReleaseServerBuildIdentity, String> {
    let candidate_sha = std::env::var("CANDIDATE_SHA")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .or_else(|| {
            std::env::var("GITHUB_SHA")
                .ok()
                .filter(|value| !value.trim().is_empty())
        })
        .or_else(|| run_output("git", &["rev-parse", "HEAD"]).ok())
        .unwrap_or_else(|| "unavailable".to_string());
    let candidate_tree = run_output("git", &["rev-parse", "HEAD^{tree}"])
        .unwrap_or_else(|_| "unavailable".to_string())
        .trim()
        .to_string();
    let rustc = run_output("rustc", &["-vV"])
        .map(|value| value.trim().to_string())
        .unwrap_or_else(|_| "unavailable".to_string());
    let cargo = run_output("cargo", &["--version"])
        .map(|value| value.trim().to_string())
        .unwrap_or_else(|_| "unavailable".to_string());
    let toolchain_file = Path::new("rust-toolchain.toml");
    let lockfile = Path::new("Cargo.lock");
    let toolchain_file_sha256 = if toolchain_file.is_file() {
        sha256_file(toolchain_file)?
    } else {
        "unavailable".to_string()
    };
    let cargo_lock_sha256 = if lockfile.is_file() {
        sha256_file(lockfile)?
    } else {
        "unavailable".to_string()
    };
    Ok(ReleaseServerBuildIdentity {
        repository: std::env::var("GITHUB_REPOSITORY").unwrap_or_else(|_| "local".to_string()),
        candidate_sha: candidate_sha.trim().to_string(),
        candidate_tree,
        toolchain: ReleaseServerToolchain { rustc, cargo },
        toolchain_file_sha256,
        cargo_lock_sha256,
    })
}

fn release_server_file(path: &str, file: &Path) -> Result<ReleaseServerFile, String> {
    let metadata =
        fs::metadata(file).map_err(|err| format!("failed to stat {}: {err}", file.display()))?;
    Ok(ReleaseServerFile {
        path: path.to_string(),
        size: metadata.len(),
        sha256: sha256_file(file)?,
    })
}

fn archive_member_inventory(
    archive_path: &Path,
    archive_format: &str,
    executable: &str,
) -> Result<Vec<ReleaseServerMember>, String> {
    match archive_format {
        "zip" => {
            let file = fs::File::open(archive_path)
                .map_err(|err| format!("failed to open {}: {err}", archive_path.display()))?;
            let mut archive = zip::ZipArchive::new(file)
                .map_err(|err| format!("failed to read {}: {err}", archive_path.display()))?;
            let mut members = Vec::new();
            for index in 0..archive.len() {
                let mut entry = archive
                    .by_index(index)
                    .map_err(|err| format!("failed to read zip member {index}: {err}"))?;
                if entry.is_dir() {
                    return Err(format!(
                        "release server archive contains directory `{}`",
                        entry.name()
                    ));
                }
                let name = entry.name().trim_start_matches("./").to_string();
                let mode = entry.unix_mode().unwrap_or_else(|| package_mode(&name));
                members.push(archive_member(
                    &name,
                    entry.size(),
                    mode,
                    sha256_reader(&mut entry)?,
                    executable,
                ));
            }
            validate_archive_members(&members, executable)?;
            Ok(members)
        }
        "tar.gz" => {
            let file = fs::File::open(archive_path)
                .map_err(|err| format!("failed to open {}: {err}", archive_path.display()))?;
            let decoder = GzDecoder::new(file);
            let mut archive = tar::Archive::new(decoder);
            let mut members = Vec::new();
            for entry in archive
                .entries()
                .map_err(|err| format!("failed to read tar members: {err}"))?
            {
                let mut entry = entry.map_err(|err| format!("failed to read tar member: {err}"))?;
                let kind = entry.header().entry_type();
                if !kind.is_file() {
                    return Err(format!(
                        "release server archive contains non-regular member `{}`",
                        entry
                            .path()
                            .map_err(|err| format!("read tar member path: {err}"))?
                            .display()
                    ));
                }
                let name = entry
                    .path()
                    .map_err(|err| format!("read tar member path: {err}"))?
                    .to_string_lossy()
                    .trim_start_matches("./")
                    .to_string();
                let size = entry.size();
                let mode = entry
                    .header()
                    .mode()
                    .map_err(|err| format!("read tar member mode: {err}"))?;
                let sha256 = sha256_reader(&mut entry)?;
                members.push(archive_member(&name, size, mode, sha256, executable));
            }
            validate_archive_members(&members, executable)?;
            Ok(members)
        }
        other => Err(format!(
            "unsupported release server archive format `{other}`"
        )),
    }
}

fn archive_member(
    name: &str,
    size: u64,
    mode: u32,
    sha256: String,
    executable: &str,
) -> ReleaseServerMember {
    let role = if name == executable {
        "executable"
    } else if name == "LICENSE-MIT" {
        "license_mit"
    } else if name == "LICENSE-APACHE" {
        "license_apache"
    } else if name == "README-server.txt" {
        "readme"
    } else {
        "reviewed_other"
    };
    ReleaseServerMember {
        path: name.to_string(),
        kind: "regular_file".to_string(),
        role: role.to_string(),
        size,
        sha256,
        mode,
    }
}

fn validate_archive_members(
    members: &[ReleaseServerMember],
    executable: &str,
) -> Result<(), String> {
    let executable_members = members
        .iter()
        .filter(|member| member.role == "executable")
        .collect::<Vec<_>>();
    if executable_members.len() != 1 {
        return Err(format!(
            "release server archive must contain exactly one executable role, found {}",
            executable_members.len()
        ));
    }
    if executable_members[0].path != executable || executable_members[0].mode & 0o111 == 0 {
        return Err(format!(
            "release server executable `{executable}` has an invalid path or mode"
        ));
    }
    for member in members {
        if member.role != "executable" && member.mode & 0o111 != 0 {
            return Err(format!(
                "release server non-executable member `{}` has executable mode {:o}",
                member.path, member.mode
            ));
        }
    }
    Ok(())
}

fn sorted_package_files(package_dir: &Path) -> Result<Vec<PathBuf>, String> {
    let mut paths = Vec::new();
    for entry in fs::read_dir(package_dir)
        .map_err(|err| format!("failed to read {}: {err}", package_dir.display()))?
    {
        let path = entry
            .map_err(|err| format!("failed to read package entry: {err}"))?
            .path();
        if !path.is_file() {
            return Err(format!(
                "release server package must be flat: {}",
                path.display()
            ));
        }
        paths.push(path);
    }
    paths.sort();
    Ok(paths)
}

fn package_mode(file_name: &str) -> u32 {
    if file_name == "ripr" || file_name == "ripr.exe" {
        0o755
    } else {
        0o644
    }
}

pub(crate) fn sha256_file(path: &Path) -> Result<String, String> {
    let mut file = fs::File::open(path)
        .map_err(|err| format!("failed to open {} for hashing: {err}", path.display()))?;
    let mut buffer = [0_u8; 8192];
    let mut hasher = Sha256::new();
    loop {
        let read = file
            .read(&mut buffer)
            .map_err(|err| format!("failed to read {} for hashing: {err}", path.display()))?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(hex_lower(&hasher.finalize()))
}

fn sha256_reader(reader: &mut impl Read) -> Result<String, String> {
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; 8192];
    loop {
        let read = reader
            .read(&mut buffer)
            .map_err(|err| format!("failed to read archive member: {err}"))?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(hex_lower(&hasher.finalize()))
}

pub(crate) fn hex_lower(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push(char::from(HEX[usize::from(byte >> 4)]));
        output.push(char::from(HEX[usize::from(byte & 0x0f)]));
    }
    output
}

fn sorted_dist_files(dist_dir: &Path) -> Result<Vec<PathBuf>, String> {
    let mut paths = Vec::new();
    for entry in fs::read_dir(dist_dir)
        .map_err(|err| format!("failed to read {}: {err}", dist_dir.display()))?
    {
        let path = entry
            .map_err(|err| format!("failed to read entry under {}: {err}", dist_dir.display()))?
            .path();
        if path.is_file() {
            paths.push(path);
        }
    }
    paths.sort();
    Ok(paths)
}

#[derive(Debug, Clone)]
struct ReleaseServerReceiptSummary {
    path: String,
    sha256: String,
    schema_version: String,
    target: String,
    archive: ReleaseServerFile,
}

pub(crate) struct ReleaseServerReceiptSet {
    targets: std::collections::BTreeSet<String>,
    baseline: ReleaseServerBuildReceipt,
    receipts: std::collections::BTreeMap<String, ReleaseServerReceiptSummary>,
}

pub(crate) fn validate_release_server_receipts(
    dist_dir: &Path,
    version: &str,
) -> Result<ReleaseServerReceiptSet, String> {
    let mut baseline: Option<ReleaseServerBuildReceipt> = None;
    let mut receipt_targets = std::collections::BTreeSet::new();
    let mut receipts = std::collections::BTreeMap::new();
    for path in sorted_dist_files(dist_dir)? {
        let Some(file_name) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        if !file_name.ends_with(".receipt.json") {
            continue;
        }
        if file_name == format!("ripr-server-assembly-v{version}.receipt.json") {
            continue;
        }
        let text = fs::read_to_string(&path).map_err(|err| {
            format!(
                "failed to read release server receipt {}: {err}",
                path.display()
            )
        })?;
        let receipt = serde_json::from_str::<ReleaseServerBuildReceipt>(&text).map_err(|err| {
            format!(
                "malformed release server receipt `{}`: {err}",
                path.display()
            )
        })?;
        if receipt.schema_version != "0.2" {
            return Err(format!(
                "unsupported release server receipt schema `{}` for `{file_name}`; expected `0.2`",
                receipt.schema_version
            ));
        }
        if receipt.version != version {
            return Err(format!(
                "release server receipt version `{}` does not match requested version `{version}`",
                receipt.version
            ));
        }
        let receipt_target = file_name
            .strip_prefix(&format!("ripr-server-v{version}-"))
            .and_then(|name| name.strip_suffix(".receipt.json"))
            .ok_or_else(|| format!("invalid release server receipt name `{file_name}`"))?;
        let expected_archive = format!(
            "ripr-server-v{version}-{receipt_target}.{}",
            receipt.archive_format
        );
        validate_release_server_relative_path(&receipt.archive.path, "archive", receipt_target)?;
        validate_release_server_relative_path(
            &receipt.executable.path,
            "executable",
            receipt_target,
        )?;
        if receipt.target != receipt_target || receipt.archive.path != expected_archive {
            return Err(format!(
                "release server receipt archive mapping mismatch for target `{receipt_target}`"
            ));
        }
        if !receipt_targets.insert(receipt_target.to_string()) {
            return Err(format!(
                "duplicate release server receipt target `{receipt_target}`"
            ));
        }
        let archive_path = dist_dir.join(&receipt.archive.path);
        let archive_metadata = fs::metadata(&archive_path).map_err(|err| {
            format!(
                "release server receipt archive `{}` is unavailable: {err}",
                receipt.archive.path
            )
        })?;
        if archive_metadata.len() != receipt.archive.size {
            return Err(format!(
                "release server receipt archive size mismatch for target `{receipt_target}`"
            ));
        }
        let archive_sha = sha256_file(&archive_path)?;
        if receipt.archive.sha256 != archive_sha {
            return Err(format!(
                "release server receipt archive digest mismatch for target `{receipt_target}`"
            ));
        }
        let actual_members = archive_member_inventory(
            &archive_path,
            &receipt.archive_format,
            &receipt.executable.path,
        )?;
        if receipt.members != actual_members {
            return Err(format!(
                "release server receipt member inventory mismatch for target `{receipt_target}`"
            ));
        }
        receipts.insert(
            receipt_target.to_string(),
            ReleaseServerReceiptSummary {
                path: file_name.to_string(),
                sha256: sha256_file(&path)?,
                schema_version: receipt.schema_version.clone(),
                target: receipt.target.clone(),
                archive: receipt.archive.clone(),
            },
        );
        if let Some(expected) = &baseline {
            if receipt.repository != expected.repository {
                return Err(format!(
                    "release server receipt repository mismatch for target `{}`",
                    receipt.target
                ));
            }
            if receipt.candidate_sha != expected.candidate_sha {
                return Err(format!(
                    "release server receipt candidate SHA mismatch for target `{}`",
                    receipt.target
                ));
            }
            if receipt.candidate_tree != expected.candidate_tree {
                return Err(format!(
                    "release server receipt candidate tree mismatch for target `{}`",
                    receipt.target
                ));
            }
            if rustc_toolchain_identity(&receipt.toolchain.rustc)?
                != rustc_toolchain_identity(&expected.toolchain.rustc)?
                || receipt.toolchain.cargo != expected.toolchain.cargo
            {
                return Err(format!(
                    "release server receipt toolchain mismatch for target `{}`",
                    receipt.target
                ));
            }
            if receipt.toolchain_file_sha256 != expected.toolchain_file_sha256 {
                return Err(format!(
                    "release server receipt toolchain-file identity mismatch for target `{}`",
                    receipt.target
                ));
            }
            if receipt.cargo_lock_sha256 != expected.cargo_lock_sha256 {
                return Err(format!(
                    "release server receipt Cargo.lock identity mismatch for target `{}`",
                    receipt.target
                ));
            }
            if receipt.profile != expected.profile
                || receipt.features != expected.features
                || receipt.locked != expected.locked
            {
                return Err(format!(
                    "release server receipt build contract mismatch for target `{}`",
                    receipt.target
                ));
            }
        } else {
            baseline = Some(receipt);
        }
    }
    let baseline = baseline.ok_or_else(|| "no release server receipts found".to_string())?;
    Ok(ReleaseServerReceiptSet {
        targets: receipt_targets,
        baseline,
        receipts,
    })
}

fn validate_release_server_relative_path(
    value: &str,
    field: &str,
    target: &str,
) -> Result<(), String> {
    let path = Path::new(value);
    let mut components = path.components();
    match (components.next(), components.next()) {
        (Some(std::path::Component::Normal(_)), None)
            if !value.contains('/') && !value.contains('\\') =>
        {
            Ok(())
        }
        _ => Err(format!(
            "release server receipt {field} path is not a safe staging file for target `{target}`: `{value}`"
        )),
    }
}

fn rustc_toolchain_identity(value: &str) -> Result<(String, String), String> {
    let mut release = None;
    let mut commit_hash = None;
    for line in value.lines() {
        let Some((key, field)) = line.split_once(':') else {
            continue;
        };
        match key.trim() {
            "release" => release = Some(field.trim().to_string()),
            "commit-hash" => commit_hash = Some(field.trim().to_string()),
            _ => {}
        }
    }
    match (release, commit_hash) {
        (Some(release), Some(commit_hash)) => Ok((release, commit_hash)),
        _ => Err("rustc toolchain identity is missing release or commit-hash".to_string()),
    }
}

pub(crate) fn validate_release_server_staging_inventory(
    dist_dir: &Path,
    version: &str,
    assets: &[ReleaseServerAsset],
    receipt_targets: &std::collections::BTreeSet<String>,
) -> Result<(), String> {
    let mut allowed = std::collections::BTreeSet::new();
    for asset in assets {
        allowed.insert(asset.file_name.clone());
        allowed.insert(format!("{}.sha256", asset.file_name));
    }
    for target in receipt_targets {
        allowed.insert(format!("ripr-server-v{version}-{target}.receipt.json"));
    }
    allowed.insert("SHA256SUMS".to_string());
    allowed.insert(format!("ripr-server-manifest-v{version}.json"));
    allowed.insert(format!("ripr-server-assembly-v{version}.receipt.json"));
    for path in sorted_dist_files(dist_dir)? {
        let Some(file_name) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        if !allowed.contains(file_name) {
            return Err(format!(
                "unrecognized staged release server file `{file_name}`"
            ));
        }
    }
    Ok(())
}

pub(crate) fn write_release_server_outputs_transactional(
    outputs: &[(&Path, &str)],
) -> Result<(), String> {
    let transaction_id = format!("{}-{}", std::process::id(), outputs.len());
    let mut staged = Vec::with_capacity(outputs.len());
    for (index, (path, contents)) in outputs.iter().enumerate() {
        let file_name = path
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or_else(|| format!("invalid release server output path {}", path.display()))?;
        let temporary = path.with_file_name(format!(".{file_name}.{transaction_id}.{index}.tmp"));
        if let Err(error) = fs::write(&temporary, contents) {
            for (staged_path, _) in &staged {
                let _ = fs::remove_file(staged_path);
            }
            let _ = fs::remove_file(&temporary);
            return Err(format!("failed to stage {}: {error}", path.display()));
        }
        staged.push((temporary, (*path).to_path_buf()));
    }

    let mut backups = Vec::new();
    let mut installed = Vec::new();
    let result = (|| {
        for (index, (_, destination)) in staged.iter().enumerate() {
            if destination.exists() {
                let backup = destination.with_file_name(format!(".{transaction_id}.{index}.bak"));
                fs::rename(destination, &backup).map_err(|error| {
                    format!("failed to preserve {}: {error}", destination.display())
                })?;
                backups.push((backup, destination.clone()));
            }
        }
        for (temporary, destination) in &staged {
            fs::rename(temporary, destination)
                .map_err(|error| format!("failed to install {}: {error}", destination.display()))?;
            installed.push(destination.clone());
        }
        Ok(())
    })();

    if let Err(error) = result {
        for destination in installed {
            let _ = fs::remove_file(destination);
        }
        for (backup, destination) in backups.iter().rev() {
            let _ = fs::rename(backup, destination);
        }
        for (temporary, _) in &staged {
            let _ = fs::remove_file(temporary);
        }
        return Err(error);
    }
    for (backup, _) in backups {
        let _ = fs::remove_file(backup);
    }
    Ok(())
}

fn sha256_bytes(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    format!("{digest:x}")
}

pub(crate) fn read_trimmed(path: &Path) -> Result<String, String> {
    fs::read_to_string(path)
        .map(|text| text.trim().to_string())
        .map_err(|err| format!("failed to read {}: {err}", path.display()))
}
