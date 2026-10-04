//! Immutable source and controller custody for qualification-only producers.
use super::QualificationInput;
mod inventory;
use crate::policy::{CandidateAuthoritySnapshot, capture_candidate_authority};
use inventory::{committed_blobs, verify_checkout};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// Legacy registry fixtures remain separate from explicit direct #1609 input.
enum SourceAuthority {
    Registry(Box<CandidateAuthoritySnapshot>),
    Direct(Box<super::live_head::LiveHeadSnapshot>),
}

impl SourceAuthority {
    fn root(&self) -> &Path {
        match self {
            Self::Registry(v) => v.root(),
            Self::Direct(v) => v.root(),
        }
    }
    fn candidate_sha(&self) -> Result<&str, String> {
        match self {
            Self::Registry(v) => v.candidate_sha(),
            Self::Direct(v) => v.candidate_sha(),
        }
    }
    fn candidate_tree(&self) -> Result<&str, String> {
        match self {
            Self::Registry(v) => v.candidate_tree(),
            Self::Direct(v) => v.candidate_tree(),
        }
    }
    fn candidate_ref(&self) -> Result<&str, String> {
        match self {
            Self::Registry(v) => v.candidate_ref(),
            Self::Direct(v) => v.candidate_ref(),
        }
    }
    fn revalidate(&self) -> Result<(), String> {
        match self {
            Self::Registry(v) => v.revalidate(),
            Self::Direct(v) => v.revalidate(),
        }
    }
    fn verify_repository(&self, root: &Path) -> Result<(), String> {
        if matches!(self, Self::Direct(_)) {
            let origin = git_bytes(root, &["remote", "get-url", "origin"])?;
            let origin = std::str::from_utf8(&origin)
                .map_err(|error| format!("source origin UTF-8: {error}"))?
                .trim();
            if !matches!(
                origin,
                "https://github.com/EffortlessMetrics/ripr-swarm.git"
                    | "https://github.com/EffortlessMetrics/ripr-swarm"
                    | "git@github.com:EffortlessMetrics/ripr-swarm.git"
            ) {
                return Err(
                    "direct manifest source origin is unsupported; expected exact https://github.com/EffortlessMetrics/ripr-swarm[.git] or git@github.com:EffortlessMetrics/ripr-swarm.git".to_string(),
                );
            }
        }
        if let Self::Direct(manifest) = self {
            manifest.verify_ranges(root)?;
        }
        Ok(())
    }
    fn verify_package_inputs(
        &self,
        name: &str,
        blobs: &BTreeMap<String, Vec<u8>>,
    ) -> Result<(), String> {
        match self {
            Self::Registry(_) => Ok(()),
            Self::Direct(v) => v.verify_package_inputs(name, blobs),
        }
    }
    fn custody_json(&self) -> serde_json::Value {
        match self {
            Self::Registry(v) => {
                let mut custody = v.custody_json();
                if let Some(object) = custody.as_object_mut() {
                    object.insert(
                        "authority_kind".to_string(),
                        serde_json::json!("historical_registry_mode"),
                    );
                }
                custody
            }
            Self::Direct(v) => v.custody_json(),
        }
    }
}

/// Actual source custody, constructed only from the validated controller and
/// the selected checkout's Git objects. Caller-supplied identity strings cannot
/// construct this handle.
pub(crate) struct AdmittedSource {
    authority: SourceAuthority,
    root: PathBuf,
    version: String,
    package_prefix: String,
    package_name: String,
    blobs: BTreeMap<String, Vec<u8>>,
}

impl AdmittedSource {
    pub(crate) fn admit(input: &QualificationInput, version: &str) -> Result<Self, String> {
        let artifact = input
            .artifact()
            .to_str()
            .ok_or_else(|| "candidate artifact is not UTF-8".to_string())?;
        // Explicit modes, no retry or fallback across authority boundaries.
        let authority = match input.approved_manifest_digest() {
            Some(digest) => {
                SourceAuthority::Direct(Box::new(super::live_head::LiveHeadSnapshot::admit(
                    input.controller_root(),
                    version,
                    input.artifact(),
                    digest,
                )?))
            }
            None => SourceAuthority::Registry(Box::new(capture_candidate_authority(
                input.controller_root(),
                version,
                artifact,
            )?)),
        };
        let root = input
            .source_root()
            .canonicalize()
            .map_err(|error| format!("resolve candidate source root: {error}"))?;
        if root.to_str().is_none() {
            return Err("qualification source root must be UTF-8".to_string());
        }
        if root.starts_with(authority.root()) || authority.root().starts_with(&root) {
            return Err("qualification source/controller roots must be physically separate, not equal or nested".to_string());
        }
        verify_source_identity(&root, &authority)?;
        let blobs = committed_blobs(&root, authority.candidate_sha()?)?;
        let workspace_bytes = blobs
            .get("Cargo.toml")
            .ok_or_else(|| "candidate source lacks committed root Cargo.toml".to_string())?;
        let workspace: toml::Value = toml::from_str(
            std::str::from_utf8(workspace_bytes)
                .map_err(|error| format!("source manifest UTF-8: {error}"))?,
        )
        .map_err(|error| format!("source manifest TOML: {error}"))?;
        let package_prefix = if workspace.get("package").is_some() {
            ""
        } else {
            "crates/ripr/"
        };
        let manifest_path = format!("{package_prefix}Cargo.toml");
        let manifest_bytes = blobs
            .get(&manifest_path)
            .ok_or_else(|| "candidate source lacks committed package Cargo.toml".to_string())?;
        let manifest: toml::Value = toml::from_str(
            std::str::from_utf8(manifest_bytes)
                .map_err(|error| format!("package manifest UTF-8: {error}"))?,
        )
        .map_err(|error| format!("package manifest TOML: {error}"))?;
        let package = manifest
            .get("package")
            .ok_or_else(|| "candidate manifest lacks package".to_string())?;
        let package_name = package
            .get("name")
            .and_then(toml::Value::as_str)
            .ok_or_else(|| "candidate package name missing".to_string())?
            .to_string();
        let actual_version = package
            .get("version")
            .and_then(toml::Value::as_str)
            .or_else(|| {
                package
                    .get("version")
                    .and_then(|value| value.get("workspace"))
                    .and_then(toml::Value::as_bool)
                    .filter(|value| *value)
                    .and_then(|_| workspace.get("workspace"))
                    .and_then(|value| value.get("package"))
                    .and_then(|value| value.get("version"))
                    .and_then(toml::Value::as_str)
            });
        if actual_version != Some(version) {
            return Err(
                "candidate source package version differs from requested release".to_string(),
            );
        }
        if !blobs.contains_key("Cargo.lock") {
            return Err("candidate source lacks committed Cargo.lock".to_string());
        }
        authority.verify_package_inputs(&package_name, &blobs)?;
        let admitted = Self {
            authority,
            root,
            version: version.to_string(),
            package_prefix: package_prefix.to_string(),
            package_name,
            blobs,
        };
        admitted.revalidate()?;
        Ok(admitted)
    }

    pub(crate) fn root(&self) -> &Path {
        &self.root
    }
    pub(crate) fn version(&self) -> &str {
        &self.version
    }
    pub(super) fn package_prefix(&self) -> &str {
        &self.package_prefix
    }
    pub(crate) fn package_name(&self) -> &str {
        &self.package_name
    }
    pub(crate) fn package_root(&self) -> PathBuf {
        self.root.join(&self.package_prefix)
    }
    pub(crate) fn revalidate(&self) -> Result<(), String> {
        self.authority.revalidate()?;
        verify_source_identity(&self.root, &self.authority)?;
        // Git's clean status does not detect skip-worktree/filter substitutions.
        // This first slice supports raw-byte checkouts only; it does not guess
        // Cargo normalization for source inputs or accept ignored file bytes.
        verify_checkout(&self.root, &self.blobs)?;
        Ok(())
    }
    pub(crate) fn committed_file(&self, path: &str) -> Option<&[u8]> {
        self.blobs.get(path).map(Vec::as_slice)
    }
    pub(crate) fn source_sha(&self) -> Result<&str, String> {
        self.authority.candidate_sha()
    }
    pub(crate) fn custody_json(&self) -> serde_json::Value {
        serde_json::json!({"controller": self.authority.custody_json(),
            "source_root": self.root, "package_name": self.package_name,
            "package_version": self.version, "package_prefix": self.package_prefix,
            "source_checkout_contract": "ordinary tracked files match raw committed blobs; transformed/sparse inputs refuse; unlocked observed snapshots",
            "source_capture_mode": "byte_budgeted_strict_terminal_drain",
            "git_object_mode": "--no-replace-objects",
            "source_resource_budget": {"ordinary_blobs": inventory::MAX_SOURCE_FILES, "file_bytes": inventory::MAX_SOURCE_FILE_BYTES,
                "retained_blob_bytes": inventory::MAX_SOURCE_BYTES, "metadata_stdout_bytes": inventory::MAX_GIT_METADATA_BYTES,
                "stderr_bytes": inventory::MAX_GIT_STDERR_BYTES, "batch_stdout": "declared total body bytes plus exact per-object protocol headers; bounded before capture"}})
    }
}

pub(super) fn git_bytes(root: &Path, args: &[&str]) -> Result<Vec<u8>, String> {
    let output = inventory::git_capture(
        root,
        args,
        None,
        inventory::MAX_GIT_METADATA_BYTES,
        "candidate source Git identity",
    )?;
    if output.timed_out || !output.status.is_some_and(|status| status.success()) {
        let stderr_prefix = output
            .stderr
            .get(..output.stderr.len().min(4096))
            .unwrap_or_default();
        return Err(format!(
            "candidate source Git identity command failed or timed out: args={args:?}, root={root:?}, timed_out={}, status={:?}, duration={:?}, stdout_bytes={}, stderr_bytes={}, stderr_prefix={}",
            output.timed_out,
            output.status,
            output.duration,
            output.stdout.len(),
            output.stderr.len(),
            String::from_utf8_lossy(stderr_prefix).escape_debug()
        ));
    }
    Ok(output.stdout)
}

fn verify_source_identity(root: &Path, authority: &SourceAuthority) -> Result<(), String> {
    authority.verify_repository(root)?;
    let sha = authority.candidate_sha()?;
    let tree = authority.candidate_tree()?;
    let git_ref = authority.candidate_ref()?;
    for (revision, expected) in [
        ("HEAD".to_string(), sha),
        (format!("{sha}^{{tree}}"), tree),
        (format!("{git_ref}^{{commit}}"), sha),
    ] {
        let bytes = git_bytes(root, &["rev-parse", "--verify", &revision])?;
        let actual = std::str::from_utf8(&bytes)
            .map_err(|error| format!("Git identity UTF-8: {error}"))?
            .trim();
        if actual != expected {
            return Err(format!("candidate source identity changed at {revision}"));
        }
    }
    if !git_bytes(root, &["status", "--porcelain=v1", "--untracked-files=all"])?.is_empty() {
        return Err("candidate source checkout is not clean".to_string());
    }
    Ok(())
}
