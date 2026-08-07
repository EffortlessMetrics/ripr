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
        "snapshot_identity": repo_exposure_snapshot_identity(&context.input_identity, &head),
        "content_sha256": content_sha256,
    }))
}

fn repo_exposure_snapshot_identity(input_identity: &str, repository_head: &str) -> String {
    format!("snapshot:{input_identity};revision:{repository_head}")
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
    let expected_snapshot = repo_exposure_snapshot_identity(
        &identity.analysis.input_identity,
        &identity.repository.head,
    );
    if identity.snapshot_identity != expected_snapshot {
        return Err(format!(
            "agent verify {label} artifact snapshot identity does not match the declared analysis input identity and repository head"
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
        producer_version: identity.producer.version,
        analysis_mode: identity.analysis.mode,
        analysis_profile: identity.analysis.profile,
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
    pub(crate) producer_version: String,
    pub(crate) analysis_mode: String,
    pub(crate) analysis_profile: String,
}

/// Validate that two producer artifacts describe comparable analyses. The
/// input identity is stable semantic/configuration identity: it must be equal
/// across a comparable pair (the positive control), independent of the
/// repository head. The snapshot identity is producer-owned observation
/// identity, exactly bound to the input identity plus the concrete repository
/// head, so it must differ across distinct heads and agree for the same head.
pub(crate) fn validate_comparable_pair(
    before: &ValidatedArtifact,
    after: &ValidatedArtifact,
) -> Result<(), String> {
    if before.base_revision != after.base_revision {
        return Err(format!(
            "base revisions differ ({:?} vs {:?})",
            before.base_revision, after.base_revision
        ));
    }
    if before.producer_version != after.producer_version {
        return Err(format!(
            "producer versions differ ({} vs {})",
            before.producer_version, after.producer_version
        ));
    }
    if before.analysis_mode != after.analysis_mode {
        return Err(format!(
            "analysis modes differ ({} vs {})",
            before.analysis_mode, after.analysis_mode
        ));
    }
    if before.analysis_profile != after.analysis_profile {
        return Err(format!(
            "analysis profiles differ ({} vs {})",
            before.analysis_profile, after.analysis_profile
        ));
    }
    if before.input_identity != after.input_identity {
        return Err("analysis input identities differ".to_string());
    }
    if before.repository_head != after.repository_head {
        if before.snapshot_identity == after.snapshot_identity {
            return Err(
                "snapshot identities are identical for distinct repository heads".to_string(),
            );
        }
    } else if before.snapshot_identity != after.snapshot_identity {
        return Err("snapshot identities differ for the same repository head".to_string());
    }
    Ok(())
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

/// Resolve the commit that Git reports for a repository root.
///
/// This is intentionally a small shared adapter for provenance consumers. It
/// does not make an artifact or receipt current by itself; callers must still
/// compare the result with the identity they are validating.
pub(crate) fn current_git_head(root: &Path) -> Result<String, String> {
    let head = git_output(root, &["rev-parse", "HEAD"])?;
    let head = head.trim();
    if !is_full_sha(head) {
        return Err(format!(
            "git rev-parse HEAD in {} returned an invalid commit `{head}`",
            root.display()
        ));
    }
    Ok(head.to_string())
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

    fn run_git(root: &Path, args: &[&str]) -> Result<String, String> {
        git_output(root, args)
    }

    fn temporary_git_root() -> Result<PathBuf, String> {
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|error| format!("clock error: {error}"))?
            .as_nanos();
        let root = std::env::temp_dir().join(format!("ripr-artifact-identity-{stamp}"));
        std::fs::create_dir_all(&root).map_err(|error| format!("create temp root: {error}"))?;
        let init = (|| -> Result<(), String> {
            run_git(&root, &["init", "--quiet"])?;
            run_git(&root, &["config", "user.name", "RIPR test"])?;
            run_git(
                &root,
                &["config", "user.email", "ripr-test@example.invalid"],
            )?;
            Ok(())
        })();
        if let Err(error) = init {
            let _ = std::fs::remove_dir_all(&root);
            return Err(error);
        }
        Ok(root)
    }

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

    fn comparable_artifact() -> ValidatedArtifact {
        ValidatedArtifact {
            currentness: ArtifactCurrentness::Current,
            base_revision: None,
            input_identity: "input:stable".to_string(),
            snapshot_identity: format!("snapshot:input:stable;revision:{}", "a".repeat(40)),
            repository_head: "a".repeat(40),
            producer_version: "0.11.0".to_string(),
            analysis_mode: "draft".to_string(),
            analysis_profile: "draft".to_string(),
        }
    }

    #[test]
    fn comparable_pair_rejects_profile_drift_after_artifact_validation() {
        let before = comparable_artifact();
        let mut after = before.clone();
        after.analysis_profile = "release".to_string();

        assert!(matches!(
            validate_comparable_pair(&before, &after),
            Err(error) if error.contains("analysis profiles differ")
        ));
    }

    #[test]
    fn comparable_pair_accepts_stable_input_identity_across_distinct_heads() {
        let before = comparable_artifact();
        let mut after = before.clone();
        after.repository_head = "b".repeat(40);
        after.snapshot_identity = format!("snapshot:input:stable;revision:{}", "b".repeat(40));

        assert_eq!(validate_comparable_pair(&before, &after), Ok(()));
    }

    #[test]
    fn comparable_pair_rejects_identical_snapshot_across_distinct_heads() {
        let before = comparable_artifact();
        let mut after = before.clone();
        after.repository_head = "b".repeat(40);

        assert!(matches!(
            validate_comparable_pair(&before, &after),
            Err(error) if error.contains("snapshot identities are identical")
        ));
    }

    #[test]
    fn comparable_pair_rejects_input_identity_drift() {
        let before = comparable_artifact();
        let mut same_head = before.clone();
        same_head.input_identity = "input:drifted".to_string();
        same_head.snapshot_identity = "snapshot:input:drifted".to_string();
        assert!(matches!(
            validate_comparable_pair(&before, &same_head),
            Err(error) if error.contains("analysis input identities differ")
        ));

        let mut distinct_head = same_head.clone();
        distinct_head.repository_head = "b".repeat(40);
        distinct_head.snapshot_identity =
            format!("snapshot:input:drifted;revision:{}", "b".repeat(40));
        assert!(matches!(
            validate_comparable_pair(&before, &distinct_head),
            Err(error) if error.contains("analysis input identities differ")
        ));
    }

    #[test]
    fn comparable_pair_rejects_snapshot_drift_for_the_same_head() {
        let before = comparable_artifact();
        let mut after = before.clone();
        after.snapshot_identity = "snapshot:input:other".to_string();

        assert!(matches!(
            validate_comparable_pair(&before, &after),
            Err(error) if error.contains("snapshot identities differ for the same repository head")
        ));
    }

    #[test]
    fn repo_exposure_identity_changes_with_controlled_git_revision() -> Result<(), String> {
        let root = temporary_git_root()?;
        let result = (|| -> Result<(), String> {
            std::fs::write(root.join("Cargo.toml"), "[workspace]\n")
                .map_err(|error| format!("write Cargo.toml: {error}"))?;
            run_git(&root, &["add", "Cargo.toml"])?;
            run_git(
                &root,
                &[
                    "-c",
                    "commit.gpgSign=false",
                    "commit",
                    "--quiet",
                    "-m",
                    "before",
                ],
            )?;
            let before = RepoExposureArtifactContext::for_repo_exposure(
                root.clone(),
                "draft".to_string(),
                None,
            )?;
            let before_artifact = repo_exposure_artifact_metadata(&before, "sha256:test")?;

            run_git(
                &root,
                &[
                    "-c",
                    "commit.gpgSign=false",
                    "commit",
                    "--quiet",
                    "--allow-empty",
                    "-m",
                    "after",
                ],
            )?;
            let after = RepoExposureArtifactContext::for_repo_exposure(
                root.clone(),
                "draft".to_string(),
                None,
            )?;
            let after_artifact = repo_exposure_artifact_metadata(&after, "sha256:test")?;

            if before.input_identity != after.input_identity {
                return Err(
                    "controlled Git revisions must preserve the comparable input identity"
                        .to_string(),
                );
            }
            let before_snapshot = before_artifact["snapshot_identity"]
                .as_str()
                .ok_or_else(|| "before artifact omitted snapshot identity".to_string())?;
            let after_snapshot = after_artifact["snapshot_identity"]
                .as_str()
                .ok_or_else(|| "after artifact omitted snapshot identity".to_string())?;
            let before_head = before_artifact["repository"]["head"]
                .as_str()
                .ok_or_else(|| "before artifact omitted repository head".to_string())?;
            let after_head = after_artifact["repository"]["head"]
                .as_str()
                .ok_or_else(|| "after artifact omitted repository head".to_string())?;
            if before_snapshot
                != repo_exposure_snapshot_identity(&before.input_identity, before_head)
                || after_snapshot
                    != repo_exposure_snapshot_identity(&after.input_identity, after_head)
            {
                return Err(
                    "snapshot identity must use the production snapshot identity builder"
                        .to_string(),
                );
            }
            if before_snapshot == after_snapshot {
                return Err(
                    "controlled Git revisions must produce distinct snapshot identities"
                        .to_string(),
                );
            }
            Ok(())
        })();
        let cleanup =
            std::fs::remove_dir_all(&root).map_err(|error| format!("remove temp root: {error}"));
        result?;
        cleanup?;
        Ok(())
    }

    fn commit_fixture_file(root: &Path) -> Result<(), String> {
        std::fs::write(root.join("Cargo.toml"), "[workspace]\n")
            .map_err(|error| format!("write Cargo.toml: {error}"))?;
        run_git(root, &["add", "Cargo.toml"])?;
        run_git(
            root,
            &[
                "-c",
                "commit.gpgSign=false",
                "commit",
                "--quiet",
                "-m",
                "fixture",
            ],
        )?;
        Ok(())
    }

    /// Render a canonical artifact document with the content commitment left
    /// at the fixed placeholder so variants can be recommitted after edits.
    fn repo_exposure_raw_with_placeholder(root: &Path) -> Result<String, String> {
        let context = RepoExposureArtifactContext::for_repo_exposure(
            root.to_path_buf(),
            "draft".to_string(),
            None,
        )?;
        let identity = repo_exposure_artifact_metadata(&context, CONTENT_SHA256_PLACEHOLDER)?;
        let document = json!({
            "schema_version": crate::output::repo_exposure::REPO_EXPOSURE_SCHEMA_VERSION,
            "scope": "repo",
            "run_status": "complete",
            "artifact": identity,
            "seams": [],
        });
        serde_json::to_string_pretty(&document)
            .map_err(|error| format!("render artifact document: {error}"))
    }

    fn commit_content(raw_with_placeholder: &str) -> Result<String, String> {
        let digest = content_sha256_with_placeholder(raw_with_placeholder)?;
        Ok(raw_with_placeholder.replace(CONTENT_SHA256_PLACEHOLDER, &digest))
    }

    #[test]
    fn repo_exposure_validation_requires_exact_snapshot_identity() -> Result<(), String> {
        let root = temporary_git_root()?;
        let result = (|| -> Result<(), String> {
            commit_fixture_file(&root)?;
            let placeholder_raw = repo_exposure_raw_with_placeholder(&root)?;
            let raw = commit_content(&placeholder_raw)?;
            let validated = validate_repo_exposure_artifact(&root, &raw, "test before")
                .map_err(|error| format!("canonical artifact must validate: {error}"))?;
            if validated.currentness != ArtifactCurrentness::Current {
                return Err("fresh canonical artifact must be current".to_string());
            }

            let head = validated.repository_head.clone();
            let input = validated.input_identity.clone();
            let valid_snapshot = validated.snapshot_identity.clone();
            let wrong_head = "0".repeat(40);
            let cases = [
                (
                    "wrong revision component",
                    repo_exposure_snapshot_identity(&input, &wrong_head),
                ),
                (
                    "wrong input component",
                    repo_exposure_snapshot_identity("input:0000000000000000", &head),
                ),
                (
                    "prefix-compatible legacy shape",
                    format!("snapshot:{input}"),
                ),
                (
                    "arbitrary prefix-compatible text",
                    "snapshot:input:arbitrary".to_string(),
                ),
                (
                    "reordered components",
                    format!("revision:{head};snapshot:{input}"),
                ),
            ];
            for (case, snapshot) in cases {
                if snapshot == valid_snapshot {
                    return Err(format!(
                        "{case} fixture must differ from the valid snapshot identity"
                    ));
                }
                let tampered =
                    commit_content(&placeholder_raw.replace(&valid_snapshot, &snapshot))?;
                match validate_repo_exposure_artifact(&root, &tampered, "test before") {
                    Err(error) if error.contains("snapshot identity does not match") => {}
                    Err(error) => {
                        return Err(format!("{case}: unexpected rejection reason: {error}"));
                    }
                    Ok(_) => {
                        return Err(format!(
                            "{case}: tampered snapshot identity must be rejected"
                        ));
                    }
                }
            }
            Ok(())
        })();
        let cleanup =
            std::fs::remove_dir_all(&root).map_err(|error| format!("remove temp root: {error}"));
        result?;
        cleanup?;
        Ok(())
    }
}
