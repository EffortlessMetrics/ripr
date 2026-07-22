//! Producer-owned identity for analysis artifacts consumed by repair flows.
//!
//! This is an integrity and currentness boundary, not a signature system.  A
//! caller must present a RIPR-shaped artifact emitted with the producer
//! marker, repository identity, revision, and a content commitment.  The
//! commitment is calculated over the exact JSON bytes with the digest field
//! replaced by the fixed placeholder described by this module.

use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::Command;

pub(crate) const ARTIFACT_IDENTITY_SCHEMA_VERSION: &str = "1";
pub(crate) const CONTENT_COMMITMENT_CANONICALIZATION: &str = "raw_json_placeholder_v1";
pub(crate) const CONTENT_SHA256_PLACEHOLDER: &str =
    "sha256:0000000000000000000000000000000000000000000000000000000000000000";

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct RepoExposureArtifactContext {
    pub(crate) root: PathBuf,
    pub(crate) mode: String,
    pub(crate) base_revision: Option<String>,
    pub(crate) input_identity: String,
}

impl RepoExposureArtifactContext {
    pub(crate) fn for_repo_exposure(
        root: PathBuf,
        mode: String,
        base_revision: Option<String>,
    ) -> Result<Self, String> {
        let canonical_root = canonical_root(&root)?;
        let (manifest_identity, lockfile_identity) =
            crate::analysis::seam_cache::workspace_named_file_identities(&canonical_root);
        let input_canonical = format!(
            "root={};base={:?};mode={};format=repo-exposure-json;manifest={:?};lockfile={:?};analyzer={}",
            display_root(&canonical_root),
            base_revision,
            mode,
            manifest_identity,
            lockfile_identity,
            env!("CARGO_PKG_VERSION"),
        );
        let input_identity = format!(
            "input:{}",
            crate::config::config_fingerprint(&input_canonical)
        );
        Ok(Self {
            root,
            mode,
            base_revision,
            input_identity,
        })
    }
}

pub(crate) struct Sha256Writer {
    hasher: Sha256,
}

impl Sha256Writer {
    pub(crate) fn new() -> Self {
        Self {
            hasher: Sha256::new(),
        }
    }

    pub(crate) fn finish(self) -> String {
        let digest = self.hasher.finalize();
        let mut rendered = String::from("sha256:");
        for byte in digest {
            rendered.push_str(&format!("{byte:02x}"));
        }
        rendered
    }
}

impl Write for Sha256Writer {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.hasher.update(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

pub(crate) fn repo_exposure_artifact_metadata(
    context: &RepoExposureArtifactContext,
    content_sha256: &str,
) -> Result<Value, String> {
    let root = canonical_root(&context.root)?;
    let head = git_output(&root, &["rev-parse", "HEAD"])
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| is_full_sha(value))
        .unwrap_or_else(|| "unavailable".to_string());
    let status = git_output(&root, &["status", "--porcelain", "--untracked-files=no"])
        .ok()
        .map(|value| {
            if value.trim().is_empty() {
                "clean"
            } else {
                "dirty"
            }
        })
        .unwrap_or("unavailable");
    Ok(json!({
        "kind": "repo_exposure",
        "schema_version": ARTIFACT_IDENTITY_SCHEMA_VERSION,
        "canonicalization": CONTENT_COMMITMENT_CANONICALIZATION,
        "producer": {
            "tool": "ripr",
            "version": env!("CARGO_PKG_VERSION"),
        },
        "repository": {
            "root": display_root(&root),
            "head": head,
        },
            "analysis": {
                "format": "repo-exposure-json",
                "mode": context.mode,
                "base_revision": context.base_revision,
                "input_identity": context.input_identity,
                "command": "ripr check --format repo-exposure-json",
                "profile": context.mode,
                "worktree": status,
            },
        "snapshot_identity": format!("snapshot:{}", context.input_identity),
        "content_sha256": content_sha256,
    }))
}

pub(crate) fn validate_repo_exposure_artifact(
    root: &Path,
    raw: &str,
    label: &str,
) -> Result<ValidatedArtifact, String> {
    let document: RepoExposureDocument = serde_json::from_str(raw).map_err(|err| {
        format!("agent verify {label} artifact is not a canonical repo-exposure artifact: {err}")
    })?;
    let RepoExposureDocument {
        schema_version,
        scope,
        artifact: identity,
        seams,
    } = document;
    if schema_version != crate::output::repo_exposure::REPO_EXPOSURE_SCHEMA_VERSION {
        return Err(format!(
            "agent verify {label} artifact has unsupported repo-exposure schema `{}`",
            schema_version
        ));
    }
    if scope != "repo" {
        return Err(format!(
            "agent verify {label} artifact has unsupported scope `{}`",
            scope
        ));
    }
    if seams.iter().any(|seam| {
        seam.seam_id.trim().is_empty()
            || seam.kind.trim().is_empty()
            || seam.file.trim().is_empty()
            || seam.line == 0
            || seam.grip_class.trim().is_empty()
    }) {
        return Err(format!(
            "agent verify {label} artifact contains an invalid canonical seam"
        ));
    }
    if identity.kind != "repo_exposure"
        || identity.schema_version != ARTIFACT_IDENTITY_SCHEMA_VERSION
        || identity.canonicalization != CONTENT_COMMITMENT_CANONICALIZATION
        || identity.producer.tool != "ripr"
        || identity.producer.version.trim().is_empty()
        || identity.analysis.format != "repo-exposure-json"
        || identity.analysis.mode.trim().is_empty()
        || identity.analysis.input_identity.trim().is_empty()
        || identity.analysis.command != "ripr check --format repo-exposure-json"
        || identity.analysis.profile != identity.analysis.mode
        || !identity.snapshot_identity.starts_with("snapshot:input:")
        || !matches!(identity.analysis.worktree.as_str(), "clean" | "dirty")
    {
        return Err(format!(
            "agent verify {label} artifact has invalid or unknown producer identity"
        ));
    }
    let expected_root = canonical_root(root)?;
    let declared_root = canonical_root(Path::new(&identity.repository.root))?;
    if declared_root != expected_root {
        return Err(format!(
            "agent verify {label} artifact repository root {} does not match {}",
            declared_root.display(),
            expected_root.display()
        ));
    }
    if !is_full_sha(&identity.repository.head) {
        return Err(format!(
            "agent verify {label} artifact has invalid repository HEAD `{}`",
            identity.repository.head
        ));
    }
    if !identity.content_sha256.starts_with("sha256:") {
        return Err(format!(
            "agent verify {label} artifact is missing a sha256 content commitment"
        ));
    }
    let recomputed = content_sha256_with_placeholder(raw)?;
    if recomputed != identity.content_sha256 {
        return Err(format!(
            "agent verify {label} artifact content commitment mismatch: declared {}, recomputed {}",
            identity.content_sha256, recomputed
        ));
    }

    let actual_head = git_output(&expected_root, &["rev-parse", "HEAD"])?;
    let actual_head = actual_head.trim();
    let actual_worktree_dirty = git_output(
        &expected_root,
        &["status", "--porcelain", "--untracked-files=no"],
    )
    .map(|status| !status.trim().is_empty())?;
    let currentness = if actual_head == identity.repository.head {
        if identity.analysis.worktree == "dirty" || actual_worktree_dirty {
            ArtifactCurrentness::DirtyWorktree
        } else {
            ArtifactCurrentness::Current
        }
    } else {
        ArtifactCurrentness::Historical
    };
    Ok(ValidatedArtifact {
        currentness,
        base_revision: identity.analysis.base_revision,
        input_identity: identity.analysis.input_identity,
        snapshot_identity: identity.snapshot_identity,
        repository_head: identity.repository.head,
    })
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum ArtifactCurrentness {
    Current,
    DirtyWorktree,
    Historical,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ValidatedArtifact {
    pub(crate) currentness: ArtifactCurrentness,
    pub(crate) base_revision: Option<String>,
    pub(crate) input_identity: String,
    pub(crate) snapshot_identity: String,
    pub(crate) repository_head: String,
}

#[derive(serde::Deserialize)]
struct RepoExposureDocument {
    schema_version: String,
    scope: String,
    artifact: ArtifactIdentity,
    seams: Vec<RepoExposureSeam>,
}

#[derive(serde::Deserialize)]
struct RepoExposureSeam {
    seam_id: String,
    kind: String,
    file: String,
    line: u64,
    grip_class: String,
}

#[derive(serde::Deserialize)]
struct ArtifactIdentity {
    kind: String,
    schema_version: String,
    canonicalization: String,
    producer: ProducerIdentity,
    repository: RepositoryIdentity,
    analysis: AnalysisIdentity,
    snapshot_identity: String,
    content_sha256: String,
}

#[derive(serde::Deserialize)]
struct ProducerIdentity {
    tool: String,
    version: String,
}

#[derive(serde::Deserialize)]
struct RepositoryIdentity {
    root: String,
    head: String,
}

#[derive(serde::Deserialize)]
struct AnalysisIdentity {
    format: String,
    mode: String,
    base_revision: Option<String>,
    input_identity: String,
    command: String,
    profile: String,
    worktree: String,
}

fn canonical_root(root: &Path) -> Result<PathBuf, String> {
    root.canonicalize().map_err(|err| {
        format!(
            "canonicalize artifact repository root {} failed: {err}",
            root.display()
        )
    })
}

fn display_root(root: &Path) -> String {
    root.to_string_lossy().replace('\\', "/")
}

fn git_output(root: &Path, args: &[&str]) -> Result<String, String> {
    let output = Command::new("git")
        .args(args)
        .current_dir(root)
        .output()
        .map_err(|err| format!("run git {:?} in {} failed: {err}", args, root.display()))?;
    if !output.status.success() {
        return Err(format!(
            "git {:?} in {} failed with {}: {}",
            args,
            root.display(),
            output.status,
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    String::from_utf8(output.stdout)
        .map_err(|err| format!("git {args:?} returned non-UTF-8 output: {err}"))
}

fn is_full_sha(value: &str) -> bool {
    value.len() == 40 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn content_sha256_with_placeholder(raw: &str) -> Result<String, String> {
    let key = "\"content_sha256\"";
    let mut matches = raw.match_indices(key);
    let (key_start, _) = matches
        .next()
        .ok_or_else(|| "artifact is missing content_sha256 commitment".to_string())?;
    if matches.next().is_some() {
        return Err("artifact contains duplicate content_sha256 commitments".to_string());
    }
    let value_start = raw[key_start + key.len()..]
        .find('"')
        .map(|offset| key_start + key.len() + offset + 1)
        .ok_or_else(|| "artifact content_sha256 value is malformed".to_string())?;
    let value_end = raw[value_start..]
        .find('"')
        .map(|offset| value_start + offset)
        .ok_or_else(|| "artifact content_sha256 value is unterminated".to_string())?;
    let declared = &raw[value_start..value_end];
    if !declared.starts_with("sha256:")
        || declared.len() != 71
        || !declared[7..].bytes().all(|byte| byte.is_ascii_hexdigit())
    {
        return Err("artifact content_sha256 must be a sha256:<64 hex> value".to_string());
    }
    let mut normalized = String::with_capacity(raw.len());
    normalized.push_str(&raw[..value_start]);
    normalized.push_str(CONTENT_SHA256_PLACEHOLDER);
    normalized.push_str(&raw[value_end..]);
    let mut hasher = Sha256::new();
    hasher.update(normalized.as_bytes());
    let digest = hasher.finalize();
    let mut rendered = String::from("sha256:");
    for byte in digest {
        rendered.push_str(&format!("{byte:02x}"));
    }
    Ok(rendered)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn content_placeholder_is_fixed_width() {
        assert_eq!(CONTENT_SHA256_PLACEHOLDER.len(), 71);
    }

    #[test]
    fn content_commitment_rejects_duplicate_fields() {
        let raw = r#"{"content_sha256":"sha256:0000000000000000000000000000000000000000000000000000000000000000","content_sha256":"sha256:0000000000000000000000000000000000000000000000000000000000000000"}"#;
        let result = content_sha256_with_placeholder(raw);
        assert!(matches!(result, Err(error) if error.contains("duplicate")));
    }

    #[test]
    fn content_commitment_rejects_non_hex_digest() {
        let raw = r#"{"content_sha256":"sha256:gggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggg"}"#;
        let result = content_sha256_with_placeholder(raw);
        assert!(matches!(result, Err(error) if error.contains("sha256:<64 hex>")));
    }
}
