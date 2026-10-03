//! Actual Cargo producer, archive inventory, install and executable custody.
use super::{AdmittedSource, safe_artifact_path};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::Duration;

/// Archive custody starts at an observed successful Cargo producer, never a
/// caller's archive/version string. Ordinary entries retain raw Git attribution.
pub(crate) struct AttributedArchive {
    source: AdmittedSource,
    path: PathBuf,
    bytes: Vec<u8>,
    inventory: BTreeMap<String, ArchiveFile>,
}

struct ArchiveFile {
    bytes: Vec<u8>,
    mode: u32,
}

impl AttributedArchive {
    pub(crate) fn produce(source: AdmittedSource, owned_root: &Path) -> Result<Self, String> {
        source.revalidate()?;
        let target = owned_root.join("package-target");
        let manifest = source.package_root().join("Cargo.toml");
        let config = owned_cargo_config(owned_root, "package")?;
        let args = vec![
            "--config".to_string(),
            config.to_string_lossy().into_owned(),
            "package".to_string(),
            "--jobs".to_string(),
            "2".to_string(),
            "--locked".to_string(),
            "--manifest-path".to_string(),
            manifest.to_string_lossy().into_owned(),
            "--target-dir".to_string(),
            target.to_string_lossy().into_owned(),
        ];
        qualified_command(
            Path::new("cargo"),
            &args,
            source.root(),
            Duration::from_mins(15),
            "qualified cargo package",
        )?;
        source.revalidate()?;
        let path = target.join("package").join(format!(
            "{}-{}.crate",
            source.package_name(),
            source.version()
        ));
        let bytes =
            std::fs::read(&path).map_err(|error| format!("read produced archive: {error}"))?;
        let inventory = archive_inventory(&bytes, &source)?;
        let archive = Self {
            source,
            path,
            bytes,
            inventory,
        };
        archive.revalidate()?;
        Ok(archive)
    }

    pub(crate) fn revalidate(&self) -> Result<(), String> {
        self.source.revalidate()?;
        let fresh = std::fs::read(&self.path)
            .map_err(|error| format!("revalidate package bytes: {error}"))?;
        if fresh != self.bytes {
            return Err("produced archive bytes changed after attribution".to_string());
        }
        Ok(())
    }

    #[cfg(test)]
    pub(crate) fn archive_path(&self) -> &Path {
        &self.path
    }

    pub(crate) fn install(self, owned_root: &Path) -> Result<InstalledCandidate, String> {
        self.revalidate()?;
        // Cargo 1.95 stops workspace ancestor discovery at target/package.
        // Keep attributed manifests unchanged while isolating this owned extraction.
        let package_boundary = owned_root.join("target/package");
        std::fs::create_dir_all(&package_boundary)
            .map_err(|error| format!("create owned Cargo package boundary: {error}"))?;
        let extracted = package_boundary.join("extracted");
        std::fs::create_dir(&extracted)
            .map_err(|error| format!("create owned extraction root: {error}"))?;
        for (path, file) in &self.inventory {
            let output = extracted.join(path);
            if let Some(parent) = output.parent() {
                std::fs::create_dir_all(parent)
                    .map_err(|error| format!("create owned package directory: {error}"))?;
            }
            std::fs::write(&output, &file.bytes)
                .map_err(|error| format!("write attributed package entry: {error}"))?;
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                std::fs::set_permissions(&output, std::fs::Permissions::from_mode(file.mode))
                    .map_err(|error| format!("restore attributed package permissions: {error}"))?;
            }
            #[cfg(not(unix))]
            let _mode = file.mode;
        }
        let install_root = owned_root.join("installed");
        let target_root = owned_root.join("install-target");
        let config = owned_cargo_config(owned_root, "install")?;
        let args = vec![
            "--config".to_string(),
            config.to_string_lossy().into_owned(),
            "install".to_string(),
            "--jobs".to_string(),
            "2".to_string(),
            "--path".to_string(),
            extracted.to_string_lossy().into_owned(),
            "--locked".to_string(),
            "--offline".to_string(),
            "--root".to_string(),
            install_root.to_string_lossy().into_owned(),
            "--target-dir".to_string(),
            target_root.to_string_lossy().into_owned(),
        ];
        qualified_command(
            Path::new("cargo"),
            &args,
            &extracted,
            Duration::from_mins(15),
            "qualified cargo install",
        )?;
        self.revalidate()?;
        for (path, file) in &self.inventory {
            if std::fs::read(extracted.join(path))
                .map_err(|error| format!("revalidate install input: {error}"))?
                != file.bytes
            {
                return Err("attributed extraction bytes changed during installation".to_string());
            }
        }
        let binary = install_root.join("bin").join(format!(
            "{}{}",
            self.source.package_name(),
            std::env::consts::EXE_SUFFIX
        ));
        let executable_bytes = std::fs::read(&binary)
            .map_err(|error| format!("read installed candidate executable: {error}"))?;
        if executable_bytes.is_empty() {
            return Err("installed candidate executable is empty".to_string());
        }
        let installed = InstalledCandidate {
            archive: self,
            binary,
            executable_bytes,
        };
        installed.revalidate()?;
        Ok(installed)
    }
}

pub(crate) struct InstalledCandidate {
    archive: AttributedArchive,
    binary: PathBuf,
    executable_bytes: Vec<u8>,
}

/// Shared authentic-chain semantics, with qualified custody enforced at each
/// producer invocation. The legacy caller retains its existing process mode.
#[derive(Clone, Copy)]
pub(crate) enum CandidateExecution<'a> {
    Legacy(&'a Path),
    Qualified(&'a InstalledCandidate),
}

impl CandidateExecution<'_> {
    pub(crate) fn fixture_git(
        self,
        root: &Path,
        args: &[&str],
        context: &str,
    ) -> Result<super::super::CommandResult, String> {
        match self {
            Self::Legacy(_) => super::super::run_fixture_git_command(root, args, context),
            Self::Qualified(candidate) => {
                candidate.revalidate()?;
                let args = args
                    .iter()
                    .map(|arg| (*arg).to_string())
                    .collect::<Vec<_>>();
                let output = qualified_command(
                    Path::new("git"),
                    &args,
                    root,
                    Duration::from_secs(30),
                    context,
                )?;
                candidate.revalidate()?;
                let status = output
                    .status
                    .ok_or_else(|| format!("{context} has no process status"))?;
                Ok(super::super::CommandResult {
                    status: status.code(),
                    success: status.success(),
                    stdout: String::from_utf8(output.stdout)
                        .map_err(|error| format!("{context} stdout UTF-8: {error}"))?,
                    stderr: String::from_utf8(output.stderr)
                        .map_err(|error| format!("{context} stderr UTF-8: {error}"))?,
                })
            }
        }
    }
    pub(crate) fn fixture_head(self, root: &Path) -> Result<String, String> {
        if matches!(self, Self::Legacy(_)) {
            return super::super::fixture_head(root);
        }
        let result = self.fixture_git(root, &["rev-parse", "HEAD"], "read fixture HEAD")?;
        let head = result.stdout.trim();
        if head.len() != 40 || !head.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return Err(format!("fixture HEAD is not a full commit SHA: {head}"));
        }
        Ok(head.to_string())
    }
    pub(crate) fn fixture_checkout(self, root: &Path, sha: &str) -> Result<(), String> {
        if matches!(self, Self::Legacy(_)) {
            return super::super::checkout_fixture_commit(root, sha);
        }
        self.fixture_git(
            root,
            &["checkout", "--quiet", "--detach", sha],
            "checkout fixture commit",
        )?;
        Ok(())
    }
    pub(crate) fn run(
        self,
        args: &[String],
        cwd: &Path,
        context: &str,
    ) -> Result<super::super::CommandResult, String> {
        match self {
            Self::Legacy(binary) => super::super::run_command_in_dir(binary, args, cwd, context),
            Self::Qualified(candidate) => {
                candidate.revalidate()?;
                let output = crate::run::capture_bytes_in_dir_with_timeout(
                    candidate.binary(),
                    args,
                    cwd,
                    &[],
                    &[],
                    Duration::from_mins(2),
                    context,
                )?;
                candidate.revalidate()?;
                if output.timed_out {
                    return Err(format!("{context} exceeded qualified process deadline"));
                }
                let status = output
                    .status
                    .ok_or_else(|| format!("{context} has no process status"))?;
                Ok(super::super::CommandResult {
                    status: status.code(),
                    success: status.success(),
                    stdout: String::from_utf8(output.stdout)
                        .map_err(|error| format!("{context} stdout UTF-8: {error}"))?,
                    stderr: String::from_utf8(output.stderr)
                        .map_err(|error| format!("{context} stderr UTF-8: {error}"))?,
                })
            }
        }
    }
}

impl InstalledCandidate {
    pub(crate) fn fixture_bytes(&self, relative: &str) -> Result<Vec<u8>, String> {
        self.revalidate()?;
        let path = format!("fixtures/boundary_gap/input/{relative}");
        let bytes = self
            .archive
            .source
            .committed_file(&path)
            .ok_or_else(|| format!("admitted source lacks authentic fixture {path}"))?
            .to_vec();
        self.revalidate()?;
        Ok(bytes)
    }
    pub(crate) fn revalidate(&self) -> Result<(), String> {
        self.archive.revalidate()?;
        if std::fs::read(&self.binary)
            .map_err(|error| format!("revalidate installed executable: {error}"))?
            != self.executable_bytes
        {
            return Err("installed executable bytes changed after custody capture".to_string());
        }
        Ok(())
    }
    pub(crate) fn binary(&self) -> &Path {
        &self.binary
    }
    pub(crate) fn custody_json(&self) -> serde_json::Value {
        serde_json::json!({"source": self.archive.source.custody_json(),
            "archive_sha256": format!("{:x}", Sha256::digest(&self.archive.bytes)),
            "executable_sha256": format!("{:x}", Sha256::digest(&self.executable_bytes)),
            "archive": self.archive.path, "executable": self.binary,
            "claim": "unlocked byte custody checks; not authenticated or atomic provenance"})
    }
}

fn owned_cargo_config(root: &Path, phase: &str) -> Result<PathBuf, String> {
    let temporary = root.join(format!("{phase}-temporary"));
    std::fs::create_dir(&temporary)
        .map_err(|error| format!("create owned Cargo temporary directory: {error}"))?;
    let temporary = temporary
        .canonicalize()
        .map_err(|error| format!("resolve owned Cargo temporary directory: {error}"))?;
    let value = temporary
        .to_str()
        .ok_or_else(|| "owned Cargo temporary path is not UTF-8".to_string())?;
    let mut environment = toml::map::Map::new();
    for name in ["TEMP", "TMP", "TMPDIR"] {
        let mut setting = toml::map::Map::new();
        setting.insert("value".to_string(), toml::Value::String(value.to_string()));
        setting.insert("force".to_string(), toml::Value::Boolean(true));
        setting.insert("relative".to_string(), toml::Value::Boolean(false));
        environment.insert(name.to_string(), toml::Value::Table(setting));
    }
    let mut config = toml::map::Map::new();
    config.insert("env".to_string(), toml::Value::Table(environment));
    let path = root.join(format!("{phase}-cargo-config.toml"));
    std::fs::write(
        &path,
        toml::to_string(&toml::Value::Table(config))
            .map_err(|error| format!("encode owned Cargo configuration: {error}"))?,
    )
    .map_err(|error| format!("write owned Cargo configuration: {error}"))?;
    Ok(path)
}

fn qualified_command(
    program: &Path,
    args: &[String],
    cwd: &Path,
    timeout: Duration,
    context: &str,
) -> Result<crate::run::TimedBytesOutput, String> {
    let output = crate::run::capture_bytes_in_dir_with_timeout(
        program,
        args,
        cwd,
        &[],
        &[
            "CARGO_TARGET_DIR",
            "GIT_DIR",
            "GIT_WORK_TREE",
            "GIT_INDEX_FILE",
        ],
        timeout,
        context,
    )?;
    if output.timed_out {
        return Err(format!("{context} exceeded owned process deadline"));
    }
    if !output.status.is_some_and(|status| status.success()) {
        return Err(format!(
            "{context} failed with native status {:?}: {}",
            output.status.and_then(|status| status.code()),
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    Ok(output)
}

fn archive_inventory(
    bytes: &[u8],
    source: &AdmittedSource,
) -> Result<BTreeMap<String, ArchiveFile>, String> {
    let decoder = flate2::read::GzDecoder::new(bytes);
    let mut archive = tar::Archive::new(decoder);
    let prefix = format!("{}-{}", source.package_name(), source.version());
    let expected_root = Path::new(&prefix);
    let mut inventory = BTreeMap::new();
    for entry in archive
        .entries()
        .map_err(|error| format!("read produced archive entries: {error}"))?
    {
        let mut entry = entry.map_err(|error| format!("read produced archive entry: {error}"))?;
        let path = entry
            .path()
            .map_err(|error| format!("produced archive path: {error}"))?
            .into_owned();
        super::super::validate_package_entry(&path, entry.header().entry_type(), expected_root)?;
        if entry.header().entry_type().is_dir() {
            continue;
        }
        if !entry.header().entry_type().is_file() {
            return Err("unsupported produced archive entry type".to_string());
        }
        let relative = path
            .strip_prefix(expected_root)
            .map_err(|error| format!("archive root: {error}"))?;
        if !safe_artifact_path(relative) {
            return Err("unsupported archive relative path".to_string());
        }
        let relative = relative
            .to_str()
            .ok_or_else(|| "archive relative path UTF-8".to_string())?
            .replace('\\', "/");
        let mode = entry
            .header()
            .mode()
            .map_err(|error| format!("archive file mode: {error}"))?;
        if mode & !0o777 != 0 {
            return Err("unsupported archive file permission bits".to_string());
        }
        let mut body = Vec::new();
        entry
            .read_to_end(&mut body)
            .map_err(|error| format!("read produced entry bytes: {error}"))?;
        let committed_path = format!("{}{relative}", source.package_prefix());
        match relative.as_str() {
            ".cargo_vcs_info.json" => {
                let vcs: serde_json::Value = serde_json::from_slice(&body)
                    .map_err(|error| format!("Cargo VCS info: {error}"))?;
                // Cargo may omit false. A present dirty field must be the
                // boolean false; absence is accepted, not a provenance claim.
                let dirty_ok = vcs.pointer("/git/dirty").is_none()
                    || vcs
                        .pointer("/git/dirty")
                        .and_then(serde_json::Value::as_bool)
                        == Some(false);
                if vcs.pointer("/git/sha1").and_then(serde_json::Value::as_str)
                    != Some(source.source_sha()?)
                    || !dirty_ok
                    || vcs.get("path_in_vcs").and_then(serde_json::Value::as_str)
                        != Some(source.package_prefix().trim_end_matches('/'))
                {
                    return Err(
                        "produced Cargo VCS identity differs from admitted source".to_string()
                    );
                }
            }
            "Cargo.toml.orig" => {
                if source.committed_file(&format!("{}Cargo.toml", source.package_prefix()))
                    != Some(body.as_slice())
                {
                    return Err(
                        "Cargo original manifest differs from committed package manifest"
                            .to_string(),
                    );
                }
            }
            "Cargo.toml" => {
                let manifest: toml::Value = toml::from_str(
                    std::str::from_utf8(&body)
                        .map_err(|error| format!("packaged manifest UTF-8: {error}"))?,
                )
                .map_err(|error| format!("packaged manifest TOML: {error}"))?;
                if manifest
                    .get("package")
                    .and_then(|value| value.get("name"))
                    .and_then(toml::Value::as_str)
                    != Some(source.package_name())
                    || manifest
                        .get("package")
                        .and_then(|value| value.get("version"))
                        .and_then(toml::Value::as_str)
                        != Some(source.version())
                {
                    return Err("Cargo normalized package identity mismatch".to_string());
                }
            }
            "Cargo.lock" => {
                let lock: toml::Value = toml::from_str(
                    std::str::from_utf8(&body)
                        .map_err(|error| format!("packaged lock UTF-8: {error}"))?,
                )
                .map_err(|error| format!("packaged lock TOML: {error}"))?;
                if lock
                    .get("version")
                    .and_then(toml::Value::as_integer)
                    .is_none()
                {
                    return Err("Cargo generated lock has no supported version".to_string());
                }
            }
            _ => {
                if source.committed_file(&committed_path) != Some(body.as_slice()) {
                    return Err(format!(
                        "packaged ordinary entry is not the committed source blob: {relative}"
                    ));
                }
            }
        }
        if inventory
            .insert(relative, ArchiveFile { bytes: body, mode })
            .is_some()
        {
            return Err("duplicate archive path".to_string());
        }
    }
    for required in [
        "Cargo.toml",
        "Cargo.toml.orig",
        "Cargo.lock",
        ".cargo_vcs_info.json",
    ] {
        if !inventory.contains_key(required) {
            return Err(format!("produced archive lacks {required}"));
        }
    }
    Ok(inventory)
}
