use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

use crate::{command_success_owned, run_owned};

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
    for path in [&sha256sums_path, &legacy_checksums_path] {
        if path.exists() {
            fs::remove_file(path)
                .map_err(|err| format!("failed to remove {}: {err}", path.display()))?;
        }
    }

    let mut assets = serde_json::Map::new();
    for asset in release_server_assets(dist_dir, &version)? {
        let sha_path = dist_dir.join(format!("{}.sha256", asset.file_name));
        let sha = read_trimmed(&sha_path)?;
        let url = format!(
            "https://github.com/{repository}/releases/download/v{version}/{}",
            asset.file_name
        );
        assets.insert(
            asset.target,
            serde_json::json!({ "url": url, "sha256": sha }),
        );
    }

    let manifest = serde_json::json!({
        "version": version,
        "assets": assets,
    });
    let manifest_path = dist_dir.join(format!("ripr-server-manifest-v{version}.json"));
    let manifest_text = serde_json::to_string_pretty(&manifest)
        .map_err(|err| format!("failed to render release server manifest: {err}"))?;
    fs::write(&manifest_path, format!("{manifest_text}\n"))
        .map_err(|err| format!("failed to write {}: {err}", manifest_path.display()))?;

    let mut checksum_lines = Vec::new();
    for path in sorted_dist_files(dist_dir)? {
        let Some(file_name) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        if file_name.ends_with(".sha256")
            || file_name == "SHA256SUMS"
            || file_name == "checksums.txt"
        {
            continue;
        }
        checksum_lines.push(format!("{}  {file_name}", sha256_file(&path)?));
    }
    fs::write(&sha256sums_path, format!("{}\n", checksum_lines.join("\n")))
        .map_err(|err| format!("failed to write {}: {err}", sha256sums_path.display()))?;
    eprintln!("wrote {}", manifest_path.display());
    eprintln!("wrote {}", sha256sums_path.display());
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
    for path in sorted_dist_files(Path::new("dist"))? {
        upload_args.push(path.to_string_lossy().to_string());
    }
    upload_args.push("--clobber".to_string());
    run_owned("gh", &upload_args)
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
    for path in sorted_dist_files(dist_dir)? {
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
        assets.push(ReleaseServerAsset {
            target: target.to_string(),
            file_name: file_name.to_string(),
        });
    }
    Ok(assets)
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
    run_owned(
        "tar",
        &[
            "-czf".to_string(),
            asset_path.to_string_lossy().to_string(),
            "-C".to_string(),
            package_dir.to_string_lossy().to_string(),
            ".".to_string(),
        ],
    )
}

pub(crate) fn create_zip_archive(package_dir: &Path, asset_path: &Path) -> Result<(), String> {
    let file = fs::File::create(asset_path)
        .map_err(|err| format!("failed to create {}: {err}", asset_path.display()))?;
    let mut writer = zip::ZipWriter::new(file);
    let options: zip::write::FileOptions<'_, ()> = zip::write::FileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated)
        .unix_permissions(0o644);

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
        let entry_options = if file_name.ends_with(".exe") {
            options.unix_permissions(0o755)
        } else if metadata.permissions().readonly() {
            options
        } else {
            // Best-effort executable bit for Unix-style binaries (ripr) without an extension.
            if !file_name.contains('.') {
                options.unix_permissions(0o755)
            } else {
                options
            }
        };
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

pub(crate) fn sha256_file(path: &Path) -> Result<String, String> {
    let mut file = fs::File::open(path)
        .map_err(|err| format!("failed to open {} for hashing: {err}", path.display()))?;
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; 8192];
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

pub(crate) fn read_trimmed(path: &Path) -> Result<String, String> {
    fs::read_to_string(path)
        .map(|text| text.trim().to_string())
        .map_err(|err| format!("failed to read {}: {err}", path.display()))
}

