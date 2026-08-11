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
/// Version of the repo-exposure analysis input-identity algorithm (#2823).
/// The emitted identity is `input:{INPUT_IDENTITY_VERSION}:<digest>`. The
/// version is explicit in the emitted bytes so an algorithm change is a new
/// identity shape, never a silent re-meaning of an existing one. v3 removes
/// the concrete checkout root (and any host-specific path spelling) from the
/// fingerprint: `analysis.input_identity` is portable semantic/configuration
/// identity, while `repository.root` stays the concrete checkout-instance
/// evidence validated separately.
pub(crate) const INPUT_IDENTITY_VERSION: &str = "v3";
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
    /// Build the portable semantic input identity for one repo-exposure run.
    ///
    /// The v2 canonical string covers exactly the inputs that affect analysis
    /// meaning: identity version, mode, profile (this producer binds profile
    /// to mode; both are stated explicitly), base semantics, analysis format,
    /// manifest and lockfile content identities (root-relative, so equivalent
    /// checkouts under different roots agree), the repo-exposure
    /// producer-consumed configuration boundary
    /// (`crate::config::repo_exposure_config_identity_hash` — the three
    /// oracle-strength fields only), and the analyzer version.
    /// The concrete checkout root is deliberately absent: it is emitted as
    /// `repository.root` and validated with exact canonical-path equality.
    pub(crate) fn for_repo_exposure(
        root: PathBuf,
        mode: String,
        base_revision: Option<String>,
        config: &crate::config::RiprConfig,
    ) -> Result<Self, String> {
        let canonical_root = canonical_root(&root)?;
        let (manifest_identity, lockfile_identity) =
            crate::analysis::seam_cache::workspace_named_file_identities_relative(&canonical_root);
        let input_canonical = format!(
            "identity_version={};mode={};profile={};base={:?};format=repo-exposure-json;manifest={:?};lockfile={:?};config={};analyzer={}",
            INPUT_IDENTITY_VERSION,
            mode,
            mode,
            base_revision,
            manifest_identity,
            lockfile_identity,
            crate::config::repo_exposure_config_identity_hash(config),
            env!("CARGO_PKG_VERSION"),
        );
        let input_identity = format!(
            "input:{}:{}",
            INPUT_IDENTITY_VERSION,
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

// Deferred negatives (#2921 claim boundary): main has no migration producer
// and no binary/artifact-inventory producer, so "migrated fixture claims
// fresh production" and "binary/artifact inventory disagrees" have no real
// production condition to validate against. Do not fabricate those
// rejections here; land them with the producers that make them real.
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
    // Version gate (#2823): only the current input-identity algorithm
    // validates as current evidence. A previous-version identity (for example
    // the v1 `input:<digest>` form that embedded the absolute checkout root)
    // is rejected with an explicit bounded reason rather than silently
    // accepted; a compatibility/migration boundary for earlier versions is
    // deferred until a real migration producer exists (#2921 deferred
    // negatives — no fabricated migration authority here).
    let supported_prefix = format!("input:{INPUT_IDENTITY_VERSION}:");
    let Some(algorithm_and_digest) = identity
        .analysis
        .input_identity
        .strip_prefix(&supported_prefix)
    else {
        return Err(format!(
            "agent verify {label} artifact has unsupported input identity version `{}` (expected `{supported_prefix}<digest>`)",
            identity.analysis.input_identity
        ));
    };
    // Shape gate: the producer emits exactly `fnv1a64:<16 lowercase hex>`.
    // Anything else carrying the current version prefix is malformed, not
    // merely "a different digest", and is rejected with its own bounded
    // reason so a shape failure is never confused with a version failure.
    let Some(digest) = algorithm_and_digest.strip_prefix("fnv1a64:") else {
        return Err(format!(
            "agent verify {label} artifact has malformed input identity digest `{}` (expected `{supported_prefix}fnv1a64:<16 lowercase hex>`)",
            identity.analysis.input_identity
        ));
    };
    if digest.len() != 16
        || !digest
            .bytes()
            .all(|byte| matches!(byte, b'0'..=b'9' | b'a'..=b'f'))
    {
        return Err(format!(
            "agent verify {label} artifact has malformed input identity digest `{}` (expected `{supported_prefix}fnv1a64:<16 lowercase hex>`)",
            identity.analysis.input_identity
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
    let recomputed = content_sha256_with_placeholder(raw).map_err(|error| error.to_string())?;
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
    // Lineage presence (#2922 PR A): a well-formed head that does not name a
    // commit held by the checked repository is rejected, not merely disclosed
    // as historical. Git liveness is established by the HEAD and status reads
    // above, so a failed `cat-file` here means the object is absent.
    match git_object_type(&expected_root, &identity.repository.head)? {
        Some(kind) if kind == "commit" => {}
        Some(kind) => {
            return Err(format!(
                "agent verify {label} artifact repository head `{}` names a {kind}, not a commit",
                identity.repository.head
            ));
        }
        None => {
            return Err(format!(
                "agent verify {label} artifact repository head `{}` is not present in the checked repository",
                identity.repository.head
            ));
        }
    }
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
        content_sha256: identity.content_sha256,
    })
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum ArtifactCurrentness {
    Current,
    DirtyWorktree,
    Historical,
}

/// Pair-level currentness disclosure for `agent verify` / `agent receipt`
/// output (#3027). This is the single authority both renderers call, so the
/// receipt's canonical byte comparison always recomputes the same token the
/// verify path emitted. The pair token must state what each side actually
/// is: the previous `_ => "dirty_worktree"` fallthrough mislabeled every
/// mixed pair — including the expected clean historical-before/current-after
/// transaction — as a dirty worktree. `dirty_worktree` itself stays reserved
/// for per-artifact evidence; a pair with a dirty side names which side.
///
/// There is deliberately no `unavailable` token: per-artifact validation
/// (`validate_repo_exposure_artifact`) always classifies into one of the
/// three variants and rejects an unclassifiable artifact before pair
/// rendering, so no production path could produce it.
pub(crate) fn pair_currentness_label(
    before: &ArtifactCurrentness,
    after: &ArtifactCurrentness,
) -> &'static str {
    match (before, after) {
        (ArtifactCurrentness::Current, ArtifactCurrentness::Current) => "current",
        (ArtifactCurrentness::Historical, ArtifactCurrentness::Historical) => {
            "historical_noncurrent"
        }
        (ArtifactCurrentness::Historical, ArtifactCurrentness::Current) => {
            "historical_before_current_after"
        }
        (ArtifactCurrentness::Current, ArtifactCurrentness::Historical) => {
            "current_before_historical_after"
        }
        (ArtifactCurrentness::DirtyWorktree, ArtifactCurrentness::DirtyWorktree) => "dirty_both",
        (ArtifactCurrentness::DirtyWorktree, _) => "dirty_before",
        (_, ArtifactCurrentness::DirtyWorktree) => "dirty_after",
    }
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
    /// The validated exact-bytes content commitment declared by the artifact
    /// envelope. Verify output binds this value so a later byte change to the
    /// artifact is detectable downstream (#2922 PR B).
    pub(crate) content_sha256: String,
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

/// Pair lineage authority (#2922 PR A): comparability is necessary but not
/// sufficient for an ordered before/after reading. The after revision must
/// descend from the before revision in the checked repository. A
/// same-revision pair is lineage-valid (a commit is its own ancestor);
/// whether movement is required is a verify-layer decision
/// (`validate_verify_movement`). Reversed and unrelated histories fail for
/// distinct bounded reasons.
pub(crate) fn validate_pair_lineage(
    root: &Path,
    before: &ValidatedArtifact,
    after: &ValidatedArtifact,
) -> Result<(), String> {
    if before.repository_head == after.repository_head {
        return Ok(());
    }
    if git_merge_base_is_ancestor(root, &before.repository_head, &after.repository_head)? {
        return Ok(());
    }
    if git_merge_base_is_ancestor(root, &after.repository_head, &before.repository_head)? {
        return Err(format!(
            "repository revisions are reversed: after head {} does not descend from before head {}",
            after.repository_head, before.repository_head
        ));
    }
    Err(format!(
        "repository revisions are unrelated: before head {} and after head {} share no ancestry",
        before.repository_head, after.repository_head
    ))
}

/// Verify-layer movement contract (#2922 PR A). A pair of fully current
/// artifacts bound to the same revision cannot contain movement: the input
/// identity is equal and the observed clean state is identical, so any
/// reported change would be unverifiable. Dirty-worktree and historical pairs
/// stay admissible here because their currentness is already disclosed as
/// non-current downstream; the comparability authority
/// (`validate_comparable_pair`) deliberately keeps same-revision pairs valid.
pub(crate) fn validate_verify_movement(
    before: &ValidatedArtifact,
    after: &ValidatedArtifact,
) -> Result<(), String> {
    if before.repository_head == after.repository_head
        && before.currentness == ArtifactCurrentness::Current
        && after.currentness == ArtifactCurrentness::Current
    {
        return Err(format!(
            "no repository movement between before and after artifacts: both are current at revision {}",
            before.repository_head
        ));
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
    let output = git_spawn(root, args)?;
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

/// The single process-spawn site for every git adapter in this module: the
/// process-policy gate allows exactly one command spawn here.
fn git_spawn(root: &Path, args: &[&str]) -> Result<std::process::Output, String> {
    Command::new("git")
        .args(args)
        .current_dir(root)
        .output()
        .map_err(|err| format!("run git {:?} in {} failed: {err}", args, root.display()))
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

/// The object type Git reports for a well-formed revision, or `None` when the
/// object is absent. Callers must establish repository liveness first: any
/// non-zero `git cat-file` exit is read as absence, not as an infrastructure
/// verdict.
fn git_object_type(root: &Path, revision: &str) -> Result<Option<String>, String> {
    let output = git_spawn(root, &["cat-file", "-t", revision])?;
    if !output.status.success() {
        return Ok(None);
    }
    let kind = String::from_utf8(output.stdout)
        .map_err(|err| format!("git cat-file returned non-UTF-8 output: {err}"))?;
    Ok(Some(kind.trim().to_string()))
}

/// `git merge-base --is-ancestor` as a boolean: exit 0 is "is an ancestor",
/// exit 1 is "is not", and anything else is an infrastructure error rather
/// than an ancestry verdict.
fn git_merge_base_is_ancestor(
    root: &Path,
    ancestor: &str,
    descendant: &str,
) -> Result<bool, String> {
    let output = git_spawn(root, &["merge-base", "--is-ancestor", ancestor, descendant])?;
    match output.status.code() {
        Some(0) => Ok(true),
        Some(1) => Ok(false),
        _ => Err(format!(
            "git merge-base --is-ancestor {ancestor} {descendant} in {} failed with {}: {}",
            root.display(),
            output.status,
            String::from_utf8_lossy(&output.stderr).trim()
        )),
    }
}

fn is_full_sha(value: &str) -> bool {
    value.len() == 40 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

/// Bounded rejection reasons for the exact-one content commitment rule
/// (#2921). The `Display` strings are part of the CLI error surface:
/// `validate_repo_exposure_artifact` maps them through `to_string`, so
/// changing a message changes downstream CLI output bytes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum ContentCommitmentRejection {
    /// The raw text is not well-formed JSON (malformed or truncated). An
    /// unterminated governed value truncates the document, so it surfaces
    /// here rather than as `UnterminatedValue` on real inputs.
    MalformedJson(String),
    /// No `content_sha256` key exists at the canonical `artifact.` path. A
    /// same-named key anywhere else does not count.
    Missing,
    /// More than one `content_sha256` key exists at the canonical path.
    Duplicate,
    /// The governed field is present but its value is not a JSON string.
    MalformedValue,
    /// The governed string value never terminates. Unreachable through
    /// `content_sha256_with_placeholder` after the structured parse succeeds;
    /// retained as scanner defense-in-depth and exercised directly by
    /// `commitment_scanner_reports_unterminated_string_defense_in_depth`.
    UnterminatedValue,
    /// The governed string is not a `sha256:<64 hex>` digest.
    InvalidDigestShape,
}

impl std::fmt::Display for ContentCommitmentRejection {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MalformedJson(error) => write!(formatter, "artifact JSON is malformed: {error}"),
            Self::Missing => formatter.write_str("artifact is missing content_sha256 commitment"),
            Self::Duplicate => {
                formatter.write_str("artifact contains duplicate content_sha256 commitments")
            }
            Self::MalformedValue => {
                formatter.write_str("artifact content_sha256 value is malformed")
            }
            Self::UnterminatedValue => {
                formatter.write_str("artifact content_sha256 value is unterminated")
            }
            Self::InvalidDigestShape => {
                formatter.write_str("artifact content_sha256 must be a sha256:<64 hex> value")
            }
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum PathSegment<'a> {
    Key(&'a str),
    Index,
}

/// Byte-level JSON scanner that locates the value span of the governed
/// `artifact.content_sha256` field. `serde_json::Value` silently collapses
/// duplicate object keys, so exact-one enforcement has to see the raw object
/// text. The scanner tracks the object-key path stack and handles string
/// escapes, so a literal `\"content_sha256\"` inside a string value is never
/// counted. Keys are compared by raw bytes; an escape-spelled governed key is
/// not recognized, which fails closed as a missing commitment. Multi-byte
/// UTF-8 continuation bytes never equal `"` or `\`, so byte scanning stays on
/// char boundaries.
struct CommitmentScanner<'a> {
    raw: &'a str,
    position: usize,
    spans: Vec<(usize, usize)>,
}

impl<'a> CommitmentScanner<'a> {
    fn locate(raw: &'a str) -> Result<Vec<(usize, usize)>, ContentCommitmentRejection> {
        let mut scanner = CommitmentScanner {
            raw,
            position: 0,
            spans: Vec::new(),
        };
        scanner.scan_value(&mut Vec::new())?;
        Ok(scanner.spans)
    }

    fn peek(&self) -> Option<u8> {
        self.raw.as_bytes().get(self.position).copied()
    }

    fn skip_whitespace(&mut self) {
        while matches!(self.peek(), Some(b' ' | b'\t' | b'\r' | b'\n')) {
            self.position += 1;
        }
    }

    fn malformed(detail: &str) -> ContentCommitmentRejection {
        ContentCommitmentRejection::MalformedJson(detail.to_string())
    }

    fn scan_value(
        &mut self,
        path: &mut Vec<PathSegment<'a>>,
    ) -> Result<(), ContentCommitmentRejection> {
        self.skip_whitespace();
        match self.peek() {
            Some(b'{') => self.scan_object(path),
            Some(b'[') => self.scan_array(path),
            Some(b'"') => {
                self.scan_string()?;
                Ok(())
            }
            Some(_) => {
                self.scan_primitive();
                Ok(())
            }
            None => Err(Self::malformed("unexpected end of input")),
        }
    }

    fn scan_object(
        &mut self,
        path: &mut Vec<PathSegment<'a>>,
    ) -> Result<(), ContentCommitmentRejection> {
        self.position += 1;
        self.skip_whitespace();
        if self.peek() == Some(b'}') {
            self.position += 1;
            return Ok(());
        }
        loop {
            self.skip_whitespace();
            if self.peek() != Some(b'"') {
                return Err(Self::malformed("expected an object key"));
            }
            let key = self.scan_string()?;
            self.skip_whitespace();
            if self.peek() != Some(b':') {
                return Err(Self::malformed("expected `:` after an object key"));
            }
            self.position += 1;
            let governed = path.len() == 1
                && matches!(&path[0], PathSegment::Key(parent) if *parent == "artifact")
                && key == "content_sha256";
            if governed {
                self.scan_governed_value()?;
            } else {
                path.push(PathSegment::Key(key));
                self.scan_value(path)?;
                path.pop();
            }
            self.skip_whitespace();
            match self.peek() {
                Some(b',') => {
                    self.position += 1;
                }
                Some(b'}') => {
                    self.position += 1;
                    return Ok(());
                }
                _ => return Err(Self::malformed("expected `,` or `}` in object")),
            }
        }
    }

    fn scan_array(
        &mut self,
        path: &mut Vec<PathSegment<'a>>,
    ) -> Result<(), ContentCommitmentRejection> {
        self.position += 1;
        self.skip_whitespace();
        if self.peek() == Some(b']') {
            self.position += 1;
            return Ok(());
        }
        loop {
            path.push(PathSegment::Index);
            self.scan_value(path)?;
            path.pop();
            self.skip_whitespace();
            match self.peek() {
                Some(b',') => {
                    self.position += 1;
                }
                Some(b']') => {
                    self.position += 1;
                    return Ok(());
                }
                _ => return Err(Self::malformed("expected `,` or `]` in array")),
            }
        }
    }

    /// Record the contents span of the governed string value. A non-string
    /// value is a malformed commitment, not valid JSON at another shape.
    fn scan_governed_value(&mut self) -> Result<(), ContentCommitmentRejection> {
        self.skip_whitespace();
        if self.peek() != Some(b'"') {
            return Err(ContentCommitmentRejection::MalformedValue);
        }
        self.position += 1;
        let start = self.position;
        let end = self.scan_string_end()?;
        self.spans.push((start, end));
        Ok(())
    }

    /// Scan a string token and return its raw (still escaped) contents as a
    /// borrow of the input. Keys and string values are never heap-allocated:
    /// a tens-of-megabytes artifact scans with zero per-string allocations.
    fn scan_string(&mut self) -> Result<&'a str, ContentCommitmentRejection> {
        self.position += 1;
        let start = self.position;
        let end = self.scan_string_end()?;
        Ok(&self.raw[start..end])
    }

    /// Advance past the closing quote and return its position.
    fn scan_string_end(&mut self) -> Result<usize, ContentCommitmentRejection> {
        loop {
            match self.peek() {
                Some(b'\\') => {
                    self.position += 2;
                }
                Some(b'"') => {
                    let end = self.position;
                    self.position += 1;
                    return Ok(end);
                }
                Some(_) => {
                    self.position += 1;
                }
                None => return Err(ContentCommitmentRejection::UnterminatedValue),
            }
        }
    }

    fn scan_primitive(&mut self) {
        while let Some(byte) = self.peek() {
            if matches!(byte, b',' | b'}' | b']' | b' ' | b'\t' | b'\r' | b'\n') {
                break;
            }
            self.position += 1;
        }
    }
}

/// Locate exactly one governed `artifact.content_sha256` field and return the
/// span of its string contents. The structured parse runs first so malformed
/// or truncated JSON is a typed rejection before any span work; the byte
/// scanner then enforces exact-one at the canonical schema path, which the
/// duplicate-collapsing `serde_json::Value` tree cannot see.
fn governed_commitment_span(raw: &str) -> Result<(usize, usize), ContentCommitmentRejection> {
    serde_json::from_str::<Value>(raw)
        .map_err(|error| ContentCommitmentRejection::MalformedJson(error.to_string()))?;
    match CommitmentScanner::locate(raw)?.as_slice() {
        [] => Err(ContentCommitmentRejection::Missing),
        [(start, end)] => Ok((*start, *end)),
        _ => Err(ContentCommitmentRejection::Duplicate),
    }
}

fn content_sha256_with_placeholder(raw: &str) -> Result<String, ContentCommitmentRejection> {
    let (value_start, value_end) = governed_commitment_span(raw)?;
    let declared = &raw[value_start..value_end];
    if !declared.starts_with("sha256:")
        || declared.len() != 71
        || !declared[7..].bytes().all(|byte| byte.is_ascii_hexdigit())
    {
        return Err(ContentCommitmentRejection::InvalidDigestShape);
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

/// Splice `new_value` into exactly the governed `artifact.content_sha256`
/// field, leaving every other byte — including unrelated placeholder-shaped
/// strings — untouched. No production caller exists yet (there is no
/// migration producer on main); test and future migration helpers must route
/// through this instead of a global string replace, which would rewrite
/// decoy placeholder text elsewhere in the document.
#[cfg(test)]
fn replace_content_commitment(
    raw: &str,
    new_value: &str,
) -> Result<String, ContentCommitmentRejection> {
    let (value_start, value_end) = governed_commitment_span(raw)?;
    let mut replaced = String::with_capacity(raw.len() + new_value.len());
    replaced.push_str(&raw[..value_start]);
    replaced.push_str(new_value);
    replaced.push_str(&raw[value_end..]);
    Ok(replaced)
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
    fn commitment_scanner_reports_unterminated_string_defense_in_depth() {
        // Unreachable through `content_sha256_with_placeholder`: the
        // structured serde parse rejects truncated JSON as `MalformedJson`
        // before the scanner runs. Exercise the scanner's own contract
        // directly so the documented defense-in-depth variant stays real.
        let mut scanner = CommitmentScanner {
            raw: "\"unterminated",
            position: 1,
            spans: Vec::new(),
        };
        assert!(matches!(
            scanner.scan_string_end(),
            Err(ContentCommitmentRejection::UnterminatedValue)
        ));
        // And through the public path the same input is malformed JSON.
        let truncated = r#"{"artifact":{"content_sha256":"sha256:0000"#;
        assert!(matches!(
            content_sha256_with_placeholder(truncated),
            Err(ContentCommitmentRejection::MalformedJson(_))
        ));
    }

    #[test]
    fn content_placeholder_is_fixed_width() {
        assert_eq!(CONTENT_SHA256_PLACEHOLDER.len(), 71);
    }

    #[test]
    fn content_commitment_rejects_duplicate_fields() {
        let raw = r#"{"artifact":{"content_sha256":"sha256:0000000000000000000000000000000000000000000000000000000000000000","content_sha256":"sha256:0000000000000000000000000000000000000000000000000000000000000000"}}"#;
        let result = content_sha256_with_placeholder(raw);
        assert!(matches!(result, Err(ContentCommitmentRejection::Duplicate)));
    }

    #[test]
    fn content_commitment_rejects_non_hex_digest() {
        let raw = r#"{"artifact":{"content_sha256":"sha256:gggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggg"}}"#;
        let result = content_sha256_with_placeholder(raw);
        assert!(matches!(
            result,
            Err(ContentCommitmentRejection::InvalidDigestShape)
        ));
    }

    #[test]
    fn content_commitment_rejects_wrong_length_digest() {
        let short = r#"{"artifact":{"content_sha256":"sha256:abcd"}}"#;
        assert!(matches!(
            content_sha256_with_placeholder(short),
            Err(ContentCommitmentRejection::InvalidDigestShape)
        ));
        let missing_prefix = r#"{"artifact":{"content_sha256":"0000000000000000000000000000000000000000000000000000000000000000"}}"#;
        assert!(matches!(
            content_sha256_with_placeholder(missing_prefix),
            Err(ContentCommitmentRejection::InvalidDigestShape)
        ));
    }

    #[test]
    fn content_commitment_rejects_field_at_wrong_path() {
        // A `content_sha256` key anywhere but the canonical `artifact.` path
        // is not the governed field; the commitment counts as missing.
        let top_level = r#"{"content_sha256":"sha256:0000000000000000000000000000000000000000000000000000000000000000"}"#;
        assert!(matches!(
            content_sha256_with_placeholder(top_level),
            Err(ContentCommitmentRejection::Missing)
        ));
        let wrongly_nested = r#"{"other":{"content_sha256":"sha256:0000000000000000000000000000000000000000000000000000000000000000"}}"#;
        assert!(matches!(
            content_sha256_with_placeholder(wrongly_nested),
            Err(ContentCommitmentRejection::Missing)
        ));
        let inside_array = r#"{"artifact":[{"content_sha256":"sha256:0000000000000000000000000000000000000000000000000000000000000000"}]}"#;
        assert!(matches!(
            content_sha256_with_placeholder(inside_array),
            Err(ContentCommitmentRejection::Missing)
        ));
        let artifact_without_field = r#"{"artifact":{}}"#;
        assert!(matches!(
            content_sha256_with_placeholder(artifact_without_field),
            Err(ContentCommitmentRejection::Missing)
        ));
    }

    #[test]
    fn content_commitment_rejects_non_string_value() {
        for raw in [
            r#"{"artifact":{"content_sha256":123}}"#,
            r#"{"artifact":{"content_sha256":null}}"#,
            r#"{"artifact":{"content_sha256":["sha256:0000000000000000000000000000000000000000000000000000000000000000"]}}"#,
        ] {
            assert!(matches!(
                content_sha256_with_placeholder(raw),
                Err(ContentCommitmentRejection::MalformedValue)
            ));
        }
    }

    // Ported from stale PR #2916, adapted to the canonical `artifact.` path
    // and the typed rejection taxonomy (#2921).
    #[test]
    fn content_commitment_requires_one_terminated_field() {
        let missing = r#"{"kind":"repo_exposure"}"#;
        assert!(matches!(
            content_sha256_with_placeholder(missing),
            Err(ContentCommitmentRejection::Missing)
        ));

        // A document whose governed value is unterminated is truncated JSON,
        // so the structured parse rejects it before span location: the
        // "unterminated" surface folds into the malformed-JSON rejection.
        let unterminated = r#"{"artifact":{"content_sha256":"sha256:0000000000000000000000000000000000000000000000000000000000000000}"#;
        assert!(matches!(
            content_sha256_with_placeholder(unterminated),
            Err(ContentCommitmentRejection::MalformedJson(_))
        ));
    }

    // Ported from stale PR #2916, adapted to the canonical `artifact.` path
    // with the decoy placeholder-shaped string in an unrelated field (#2921).
    #[test]
    fn content_commitment_does_not_rewrite_unrelated_placeholder_text() -> Result<(), String> {
        let placeholder = CONTENT_SHA256_PLACEHOLDER;
        let declared = "sha256:1111111111111111111111111111111111111111111111111111111111111111";
        let raw =
            format!(r#"{{"artifact":{{"note":"{placeholder}","content_sha256":"{declared}"}}}}"#);
        let digest = content_sha256_with_placeholder(&raw).map_err(|error| error.to_string())?;
        let expected_input = raw.replacen(declared, placeholder, 1);
        let mut writer = Sha256Writer::new();
        writer
            .write_all(expected_input.as_bytes())
            .map_err(|error| format!("hash expected input: {error}"))?;
        let expected = writer.finish();
        if digest != expected {
            return Err(format!(
                "digest {digest} must equal {expected}: the hash of the document with only the governed value replaced"
            ));
        }
        let recommitted =
            replace_content_commitment(&raw, &digest).map_err(|error| error.to_string())?;
        let decoy = format!(r#""note":"{placeholder}""#);
        if !recommitted.contains(&decoy) {
            return Err(
                "recommit must not rewrite the unrelated placeholder-shaped string".to_string(),
            );
        }
        let governed = format!(r#""content_sha256":"{digest}""#);
        if !recommitted.contains(&governed) {
            return Err("recommit must splice the digest into the governed field".to_string());
        }
        Ok(())
    }

    #[test]
    fn content_commitment_ignores_escaped_key_text_in_string_values() {
        // A literal `\"content_sha256\"` inside a string value is not the
        // governed field; without escape handling it would read as a
        // duplicate commitment.
        let raw = r#"{"artifact":{"note":"see \"content_sha256\" in the schema","content_sha256":"sha256:0000000000000000000000000000000000000000000000000000000000000000"}}"#;
        let result = content_sha256_with_placeholder(raw);
        assert!(result.is_ok(), "unexpected rejection: {result:?}");
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
            content_sha256: format!("sha256:{}", "0".repeat(64)),
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
    fn comparable_pair_rejects_producer_version_drift() {
        let before = comparable_artifact();
        let mut after = before.clone();
        after.producer_version = "0.12.0".to_string();

        assert!(matches!(
            validate_comparable_pair(&before, &after),
            Err(error) if error.contains("producer versions differ")
        ));
    }

    #[test]
    fn comparable_pair_rejects_base_revision_drift() {
        let before = comparable_artifact();
        let mut after = before.clone();
        after.base_revision = Some("base:other".to_string());

        assert!(matches!(
            validate_comparable_pair(&before, &after),
            Err(error) if error.contains("base revisions differ")
        ));
    }

    #[test]
    fn pair_currentness_label_covers_the_closed_vocabulary() {
        use ArtifactCurrentness::{Current, DirtyWorktree, Historical};
        // The full 3x3 pair matrix (#3027). Two cells are unreachable through
        // the CLI verify path but stay truthful in the shared mapping: a
        // fully current pair fails the movement gate, and a
        // current-before/historical-after pair fails the lineage gate.
        let cases = [
            ((Current, Current), "current"),
            ((Historical, Historical), "historical_noncurrent"),
            ((Historical, Current), "historical_before_current_after"),
            ((Current, Historical), "current_before_historical_after"),
            ((DirtyWorktree, Current), "dirty_before"),
            ((DirtyWorktree, Historical), "dirty_before"),
            ((Current, DirtyWorktree), "dirty_after"),
            ((Historical, DirtyWorktree), "dirty_after"),
            ((DirtyWorktree, DirtyWorktree), "dirty_both"),
        ];
        for ((before, after), expected) in cases {
            assert_eq!(
                pair_currentness_label(&before, &after),
                expected,
                "({before:?}, {after:?})"
            );
        }
    }

    #[test]
    fn comparable_pair_rejects_analysis_mode_drift() {
        let before = comparable_artifact();
        let mut after = before.clone();
        // Keep the profile aligned with the drifted mode so only the mode
        // check fires.
        after.analysis_mode = "release".to_string();
        after.analysis_profile = "release".to_string();

        assert!(matches!(
            validate_comparable_pair(&before, &after),
            Err(error) if error.contains("analysis modes differ")
        ));
    }

    #[test]
    fn verify_movement_rejects_only_a_fully_current_same_revision_pair() {
        let before = comparable_artifact();
        let after = before.clone();
        // The comparability authority deliberately keeps the same-revision
        // pair valid; movement is a verify-layer requirement (#2922 PR A).
        assert_eq!(validate_comparable_pair(&before, &after), Ok(()));
        assert!(matches!(
            validate_verify_movement(&before, &after),
            Err(error) if error.contains("no repository movement")
        ));

        // A disclosed non-current observation stays admissible at this layer.
        let mut dirty_after = after.clone();
        dirty_after.currentness = ArtifactCurrentness::DirtyWorktree;
        assert_eq!(validate_verify_movement(&before, &dirty_after), Ok(()));
        let mut historical_after = after.clone();
        historical_after.currentness = ArtifactCurrentness::Historical;
        assert_eq!(validate_verify_movement(&before, &historical_after), Ok(()));

        // A distinct revision carries movement regardless of currentness.
        let mut moved_after = after.clone();
        moved_after.repository_head = "b".repeat(40);
        moved_after.snapshot_identity =
            format!("snapshot:input:stable;revision:{}", "b".repeat(40));
        assert_eq!(validate_verify_movement(&before, &moved_after), Ok(()));
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
            let config = crate::config::RiprConfig::default();
            let before = RepoExposureArtifactContext::for_repo_exposure(
                root.clone(),
                "draft".to_string(),
                None,
                &config,
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
                &config,
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
            &crate::config::RiprConfig::default(),
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
        let digest = content_sha256_with_placeholder(raw_with_placeholder)
            .map_err(|error| error.to_string())?;
        // Exact-one rule: splice only the governed field. A global
        // `str::replace` would also rewrite decoy placeholder-shaped strings
        // elsewhere in the document, which the commitment law forbids.
        replace_content_commitment(raw_with_placeholder, &digest).map_err(|error| error.to_string())
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

    /// Mutate one identity field of a canonical placeholder document,
    /// recommit the content, and return the validator result.
    fn validate_mutated_identity(
        root: &Path,
        document: &Value,
        mutate: impl Fn(&mut Value),
    ) -> Result<Result<ValidatedArtifact, String>, String> {
        let mut tampered = document.clone();
        mutate(&mut tampered);
        let rendered = serde_json::to_string_pretty(&tampered)
            .map_err(|error| format!("render tampered document: {error}"))?;
        let committed = commit_content(&rendered)?;
        Ok(validate_repo_exposure_artifact(
            root,
            &committed,
            "test before",
        ))
    }

    fn expect_identity_rejection(
        case: &str,
        result: Result<ValidatedArtifact, String>,
        expected: &str,
    ) -> Result<(), String> {
        match result {
            Err(error) if error.contains(expected) => Ok(()),
            Err(error) => Err(format!("{case}: unexpected rejection reason: {error}")),
            Ok(_) => Err(format!("{case}: mutated artifact must be rejected")),
        }
    }

    type IdentityMutationCase = (&'static str, fn(&mut Value));

    #[test]
    fn repo_exposure_validation_rejects_invalid_producer_identity() -> Result<(), String> {
        let root = temporary_git_root()?;
        let result = (|| -> Result<(), String> {
            commit_fixture_file(&root)?;
            let placeholder_raw = repo_exposure_raw_with_placeholder(&root)?;
            let document: Value = serde_json::from_str(&placeholder_raw)
                .map_err(|error| format!("parse placeholder document: {error}"))?;
            let cases: [IdentityMutationCase; 11] = [
                ("artifact kind is not repo_exposure", |document| {
                    document["artifact"]["kind"] = json!("repo_exposure_forged");
                }),
                ("artifact schema version is unsupported", |document| {
                    document["artifact"]["schema_version"] = json!("2");
                }),
                ("canonicalization is unsupported", |document| {
                    document["artifact"]["canonicalization"] = json!("pretty_json_v0");
                }),
                ("producer tool is not ripr", |document| {
                    document["artifact"]["producer"]["tool"] = json!("forged");
                }),
                ("producer version is empty", |document| {
                    document["artifact"]["producer"]["version"] = json!("");
                }),
                ("analysis format is unsupported", |document| {
                    document["artifact"]["analysis"]["format"] = json!("repo-exposure-yaml");
                }),
                ("analysis mode is empty", |document| {
                    document["artifact"]["analysis"]["mode"] = json!("");
                }),
                ("analysis input identity is empty", |document| {
                    document["artifact"]["analysis"]["input_identity"] = json!("");
                }),
                ("analysis command is wrong", |document| {
                    document["artifact"]["analysis"]["command"] = json!("ripr check");
                }),
                ("analysis profile differs from mode", |document| {
                    document["artifact"]["analysis"]["profile"] = json!("release");
                }),
                ("analysis worktree state is unknown", |document| {
                    document["artifact"]["analysis"]["worktree"] = json!("unavailable");
                }),
            ];
            for (case, mutate) in cases {
                let mutated = validate_mutated_identity(&root, &document, mutate)?;
                expect_identity_rejection(case, mutated, "invalid or unknown producer identity")?;
            }
            Ok(())
        })();
        let cleanup =
            std::fs::remove_dir_all(&root).map_err(|error| format!("remove temp root: {error}"));
        result?;
        cleanup?;
        Ok(())
    }

    #[test]
    fn repo_exposure_validation_rejects_unsupported_document_schema() -> Result<(), String> {
        let root = temporary_git_root()?;
        let result = (|| -> Result<(), String> {
            commit_fixture_file(&root)?;
            let placeholder_raw = repo_exposure_raw_with_placeholder(&root)?;
            let document: Value = serde_json::from_str(&placeholder_raw)
                .map_err(|error| format!("parse placeholder document: {error}"))?;
            let mutated = validate_mutated_identity(&root, &document, |document| {
                document["schema_version"] = json!("9.0");
            })?;
            expect_identity_rejection(
                "document schema version",
                mutated,
                "unsupported repo-exposure schema",
            )
        })();
        let cleanup =
            std::fs::remove_dir_all(&root).map_err(|error| format!("remove temp root: {error}"));
        result?;
        cleanup?;
        Ok(())
    }

    #[test]
    fn repo_exposure_validation_rejects_foreign_repository_root() -> Result<(), String> {
        let root = temporary_git_root()?;
        let foreign = temporary_git_root()?;
        let result = (|| -> Result<(), String> {
            commit_fixture_file(&root)?;
            let placeholder_raw = repo_exposure_raw_with_placeholder(&root)?;
            let raw = commit_content(&placeholder_raw)?;
            match validate_repo_exposure_artifact(&foreign, &raw, "test before") {
                Err(error)
                    if error.contains("repository root") && error.contains("does not match") =>
                {
                    Ok(())
                }
                Err(error) => Err(format!("unexpected rejection reason: {error}")),
                Ok(_) => Err("an artifact bound to another root must be rejected".to_string()),
            }
        })();
        let cleanup =
            std::fs::remove_dir_all(&root).map_err(|error| format!("remove temp root: {error}"));
        let cleanup_foreign = std::fs::remove_dir_all(&foreign)
            .map_err(|error| format!("remove foreign temp root: {error}"));
        result?;
        cleanup?;
        cleanup_foreign?;
        Ok(())
    }

    #[test]
    fn repo_exposure_validation_rejects_non_concrete_repository_head() -> Result<(), String> {
        let root = temporary_git_root()?;
        let result = (|| -> Result<(), String> {
            commit_fixture_file(&root)?;
            let placeholder_raw = repo_exposure_raw_with_placeholder(&root)?;
            let document: Value = serde_json::from_str(&placeholder_raw)
                .map_err(|error| format!("parse placeholder document: {error}"))?;
            let head = document["artifact"]["repository"]["head"]
                .as_str()
                .ok_or_else(|| "fixture document omitted repository head".to_string())?
                .to_string();
            let cases = [
                ("symbolic revision", "HEAD".to_string()),
                ("short revision", head[..7].to_string()),
                ("padded revision", format!("{head}0")),
                ("non-hex revision", "g".repeat(40)),
            ];
            for (case, tampered_head) in cases {
                let mutated = validate_mutated_identity(&root, &document, |document| {
                    document["artifact"]["repository"]["head"] = json!(tampered_head);
                })?;
                expect_identity_rejection(case, mutated, "invalid repository HEAD")?;
            }
            Ok(())
        })();
        let cleanup =
            std::fs::remove_dir_all(&root).map_err(|error| format!("remove temp root: {error}"));
        result?;
        cleanup?;
        Ok(())
    }

    #[test]
    fn repo_exposure_validation_rejects_repository_head_absent_from_the_repository()
    -> Result<(), String> {
        let root = temporary_git_root()?;
        let result = (|| -> Result<(), String> {
            commit_fixture_file(&root)?;
            let placeholder_raw = repo_exposure_raw_with_placeholder(&root)?;
            let document: Value = serde_json::from_str(&placeholder_raw)
                .map_err(|error| format!("parse placeholder document: {error}"))?;
            let original_head = document["artifact"]["repository"]["head"]
                .as_str()
                .ok_or_else(|| "fixture document omitted repository head".to_string())?
                .to_string();
            let input_identity = document["artifact"]["analysis"]["input_identity"]
                .as_str()
                .ok_or_else(|| "fixture document omitted input identity".to_string())?
                .to_string();

            // A well-formed 40-hex head the repository does not hold: flip one
            // hex digit of the real head so the mutation stays well-formed
            // but cannot name the real commit. The snapshot identity is
            // updated to match so only the presence check fires.
            let absent_head = format!(
                "{}{}",
                if original_head.starts_with('0') {
                    "1"
                } else {
                    "0"
                },
                &original_head[1..]
            );
            if absent_head == original_head {
                return Err("absent-head mutation must differ from the original head".to_string());
            }
            let absent_snapshot = repo_exposure_snapshot_identity(&input_identity, &absent_head);
            let mutated = validate_mutated_identity(&root, &document, |document| {
                document["artifact"]["repository"]["head"] = json!(absent_head.as_str());
                document["artifact"]["snapshot_identity"] = json!(absent_snapshot.as_str());
            })?;
            match mutated {
                Err(error) if error.contains("is not present in the checked repository") => {}
                Err(error) => {
                    return Err(format!(
                        "absent head: unexpected rejection reason: {error} (original head {original_head}, mutated head {absent_head})"
                    ));
                }
                Ok(_) => {
                    return Err(format!(
                        "absent head: mutated artifact must be rejected (original head {original_head}, mutated head {absent_head}, mutated snapshot {absent_snapshot})"
                    ));
                }
            }

            // A well-formed object the repository holds that is not a commit.
            let tree = run_git(&root, &["rev-parse", "HEAD^{tree}"])?
                .trim()
                .to_string();
            let tree_snapshot = repo_exposure_snapshot_identity(&input_identity, &tree);
            let mutated = validate_mutated_identity(&root, &document, |document| {
                document["artifact"]["repository"]["head"] = json!(tree.as_str());
                document["artifact"]["snapshot_identity"] = json!(tree_snapshot.as_str());
            })?;
            match mutated {
                Err(error) if error.contains("names a tree, not a commit") => {}
                Err(error) => {
                    return Err(format!(
                        "non-commit head: unexpected rejection reason: {error} (original head {original_head}, mutated head {tree})"
                    ));
                }
                Ok(_) => {
                    return Err(format!(
                        "non-commit head: mutated artifact must be rejected (original head {original_head}, mutated head {tree})"
                    ));
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

    fn commit_empty(root: &Path, message: &str) -> Result<(), String> {
        run_git(
            root,
            &[
                "-c",
                "commit.gpgSign=false",
                "commit",
                "--quiet",
                "--allow-empty",
                "-m",
                message,
            ],
        )?;
        Ok(())
    }

    /// Build and validate a canonical artifact bound to the repository's
    /// current HEAD.
    fn validated_artifact_at_head(root: &Path, label: &str) -> Result<ValidatedArtifact, String> {
        let placeholder_raw = repo_exposure_raw_with_placeholder(root)?;
        let raw = commit_content(&placeholder_raw)?;
        validate_repo_exposure_artifact(root, &raw, label)
    }

    #[test]
    fn pair_lineage_rejects_reversed_revisions() -> Result<(), String> {
        let root = temporary_git_root()?;
        let result = (|| -> Result<(), String> {
            commit_fixture_file(&root)?;
            let before = validated_artifact_at_head(&root, "test before")?;
            commit_empty(&root, "after")?;
            let after = validated_artifact_at_head(&root, "test after")?;
            // Positive control: ordered movement is lineage-valid.
            validate_pair_lineage(&root, &before, &after)
                .map_err(|error| format!("ordered revisions must be lineage-valid: {error}"))?;
            // Comparability alone does not catch a swapped pair: the artifacts
            // stay mutually comparable, so only the lineage check can fire.
            validate_comparable_pair(&after, &before)
                .map_err(|error| format!("swapped pair must stay comparable: {error}"))?;
            match validate_pair_lineage(&root, &after, &before) {
                Err(error) if error.contains("revisions are reversed") => {}
                Err(error) => {
                    return Err(format!(
                        "reversed pair: unexpected rejection reason: {error}"
                    ));
                }
                Ok(()) => {
                    return Err(format!(
                        "reversed pair must be rejected (ordered before head {}, ordered after head {})",
                        before.repository_head, after.repository_head
                    ));
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

    #[test]
    fn pair_lineage_rejects_unrelated_revisions() -> Result<(), String> {
        let root = temporary_git_root()?;
        let result = (|| -> Result<(), String> {
            commit_fixture_file(&root)?;
            let before = validated_artifact_at_head(&root, "test before")?;
            // An orphan branch shares no history with the before commit while
            // both objects stay present in the checked repository.
            run_git(&root, &["checkout", "--orphan", "unrelated-lineage"])?;
            run_git(
                &root,
                &[
                    "-c",
                    "commit.gpgSign=false",
                    "commit",
                    "--quiet",
                    "-m",
                    "unrelated after",
                ],
            )?;
            let after = validated_artifact_at_head(&root, "test after")?;
            validate_comparable_pair(&before, &after)
                .map_err(|error| format!("unrelated pair must stay comparable: {error}"))?;
            match validate_pair_lineage(&root, &before, &after) {
                Err(error) if error.contains("revisions are unrelated") => {}
                Err(error) => {
                    return Err(format!(
                        "unrelated pair: unexpected rejection reason: {error}"
                    ));
                }
                Ok(()) => {
                    return Err(format!(
                        "unrelated pair must be rejected (before head {}, after head {})",
                        before.repository_head, after.repository_head
                    ));
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

    #[test]
    fn repo_exposure_currentness_tracks_live_repository_state() -> Result<(), String> {
        let root = temporary_git_root()?;
        let result = (|| -> Result<(), String> {
            commit_fixture_file(&root)?;
            let placeholder_raw = repo_exposure_raw_with_placeholder(&root)?;
            let raw = commit_content(&placeholder_raw)?;
            let current = validate_repo_exposure_artifact(&root, &raw, "test before")
                .map_err(|error| format!("fresh canonical artifact must validate: {error}"))?;
            if current.currentness != ArtifactCurrentness::Current {
                return Err("fresh canonical artifact must be current".to_string());
            }
            // Positive control: revalidation at the same revision preserves
            // every validated field, including input and snapshot identity.
            let revalidated = validate_repo_exposure_artifact(&root, &raw, "test before")
                .map_err(|error| format!("revalidation must succeed: {error}"))?;
            if revalidated != current {
                return Err(
                    "same-revision revalidation must produce identical validated fields"
                        .to_string(),
                );
            }
            // A declared-clean artifact must not hide a dirty worktree: the
            // validator checks the live repository, not the claim.
            std::fs::write(root.join("Cargo.toml"), "[workspace]\n# unsaved edit\n")
                .map_err(|error| format!("dirty fixture file: {error}"))?;
            let dirty = validate_repo_exposure_artifact(&root, &raw, "test before")
                .map_err(|error| format!("dirty revalidation must succeed: {error}"))?;
            if dirty.currentness != ArtifactCurrentness::DirtyWorktree {
                return Err("a dirty worktree must be disclosed as DirtyWorktree".to_string());
            }
            run_git(&root, &["checkout", "--", "Cargo.toml"])?;
            let restored = validate_repo_exposure_artifact(&root, &raw, "test before")
                .map_err(|error| format!("restored revalidation must succeed: {error}"))?;
            if restored.currentness != ArtifactCurrentness::Current {
                return Err("a restored clean worktree must validate as current".to_string());
            }
            // Replaying the same artifact bytes after the repository moves
            // must disclose Historical, not current.
            run_git(
                &root,
                &[
                    "-c",
                    "commit.gpgSign=false",
                    "commit",
                    "--quiet",
                    "--allow-empty",
                    "-m",
                    "advance",
                ],
            )?;
            let historical = validate_repo_exposure_artifact(&root, &raw, "test before")
                .map_err(|error| format!("historical revalidation must succeed: {error}"))?;
            if historical.currentness != ArtifactCurrentness::Historical {
                return Err(
                    "an artifact replayed after repository movement must be historical".to_string(),
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

    #[test]
    fn repo_exposure_declared_dirty_is_never_reported_current() -> Result<(), String> {
        let root = temporary_git_root()?;
        let result = (|| -> Result<(), String> {
            commit_fixture_file(&root)?;
            // Produce the artifact while the worktree is dirty so it honestly
            // declares `worktree: "dirty"`.
            std::fs::write(root.join("Cargo.toml"), "[workspace]\n# unsaved edit\n")
                .map_err(|error| format!("dirty fixture file: {error}"))?;
            let placeholder_raw = repo_exposure_raw_with_placeholder(&root)?;
            let raw = commit_content(&placeholder_raw)?;
            // Restoring a clean worktree must not upgrade a declared-dirty
            // artifact to current.
            run_git(&root, &["checkout", "--", "Cargo.toml"])?;
            let validated = validate_repo_exposure_artifact(&root, &raw, "test before")
                .map_err(|error| format!("declared-dirty artifact must validate: {error}"))?;
            if validated.currentness != ArtifactCurrentness::DirtyWorktree {
                return Err(
                    "a declared-dirty artifact must stay DirtyWorktree on a clean worktree"
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

    /// A temp path that does not exist yet, for `git clone` targets.
    fn fresh_temp_path(label: &str) -> Result<PathBuf, String> {
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|error| format!("clock error: {error}"))?
            .as_nanos();
        Ok(std::env::temp_dir().join(format!("ripr-artifact-{label}-{stamp}")))
    }

    /// Clone `source` into a fresh temp path so two checkouts share the same
    /// commit, manifest, and lockfile contents under different roots.
    fn clone_git_root(source: &Path) -> Result<PathBuf, String> {
        let destination = fresh_temp_path("clone")?;
        let output = git_spawn(
            source,
            &[
                "clone",
                "--quiet",
                &source.display().to_string(),
                &destination.display().to_string(),
            ],
        )?;
        if !output.status.success() {
            let _ = std::fs::remove_dir_all(&destination);
            return Err(format!(
                "git clone {} -> {} failed: {}",
                source.display(),
                destination.display(),
                String::from_utf8_lossy(&output.stderr).trim()
            ));
        }
        Ok(destination)
    }

    fn artifact_metadata(
        root: &Path,
        mode: &str,
        base_revision: Option<String>,
        config: &crate::config::RiprConfig,
    ) -> Result<(RepoExposureArtifactContext, Value), String> {
        let context = RepoExposureArtifactContext::for_repo_exposure(
            root.to_path_buf(),
            mode.to_string(),
            base_revision,
            config,
        )?;
        let metadata = repo_exposure_artifact_metadata(&context, "sha256:test")?;
        Ok((context, metadata))
    }

    /// (#2823 test 1) Two equivalent checkouts of the same commit under
    /// different temporary roots share one portable semantic input identity
    /// and snapshot identity, while `repository.root` stays concrete
    /// per-checkout evidence.
    #[test]
    fn repo_exposure_input_identity_is_portable_across_checkout_roots() -> Result<(), String> {
        let root_a = temporary_git_root()?;
        let result = (|| -> Result<(), String> {
            commit_fixture_file(&root_a)?;
            let root_b = clone_git_root(&root_a)?;
            let result = (|| -> Result<(), String> {
                let config = crate::config::RiprConfig::default();
                let (context_a, metadata_a) = artifact_metadata(&root_a, "draft", None, &config)?;
                let (context_b, metadata_b) = artifact_metadata(&root_b, "draft", None, &config)?;
                let prefix = format!("input:{INPUT_IDENTITY_VERSION}:");
                if !context_a.input_identity.starts_with(&prefix) {
                    return Err(format!(
                        "input identity {} must carry the current version prefix {prefix}",
                        context_a.input_identity
                    ));
                }
                if context_a.input_identity != context_b.input_identity {
                    return Err(format!(
                        "equivalent checkouts must share one input identity: {} vs {}",
                        context_a.input_identity, context_b.input_identity
                    ));
                }
                if metadata_a["snapshot_identity"] != metadata_b["snapshot_identity"] {
                    return Err(format!(
                        "equivalent checkouts at the same revision must share one snapshot identity: {:?} vs {:?}",
                        metadata_a["snapshot_identity"], metadata_b["snapshot_identity"]
                    ));
                }
                if metadata_a["repository"]["root"] == metadata_b["repository"]["root"] {
                    return Err(
                        "distinct checkouts must keep distinct concrete repository roots"
                            .to_string(),
                    );
                }
                Ok(())
            })();
            let cleanup = std::fs::remove_dir_all(&root_b)
                .map_err(|error| format!("remove clone root: {error}"));
            result?;
            cleanup?;
            Ok(())
        })();
        let cleanup =
            std::fs::remove_dir_all(&root_a).map_err(|error| format!("remove temp root: {error}"));
        result?;
        cleanup?;
        Ok(())
    }

    /// (#2823 test 2) Portability of the semantic identity does not weaken
    /// the concrete-instance check: an artifact emitted in root A is still
    /// rejected when verified against an equivalent clone at root B, and by
    /// the root-mismatch check specifically.
    #[test]
    fn repo_exposure_artifact_from_one_root_is_rejected_at_an_equivalent_clone()
    -> Result<(), String> {
        let root_a = temporary_git_root()?;
        let result = (|| -> Result<(), String> {
            commit_fixture_file(&root_a)?;
            let root_b = clone_git_root(&root_a)?;
            let result = (|| -> Result<(), String> {
                let raw = commit_content(&repo_exposure_raw_with_placeholder(&root_a)?)?;
                // Positive control: the artifact validates at its own root.
                validate_repo_exposure_artifact(&root_a, &raw, "test before")
                    .map_err(|error| format!("artifact must validate at its own root: {error}"))?;
                match validate_repo_exposure_artifact(&root_b, &raw, "test before") {
                    Err(error)
                        if error.contains("repository root")
                            && error.contains("does not match") =>
                    {
                        Ok(())
                    }
                    Err(error) => Err(format!(
                        "foreign clone: unexpected rejection reason (must be the repository root mismatch): {error}"
                    )),
                    Ok(_) => Err(
                        "an artifact bound to root A must be rejected at equivalent clone B"
                            .to_string(),
                    ),
                }
            })();
            let cleanup = std::fs::remove_dir_all(&root_b)
                .map_err(|error| format!("remove clone root: {error}"));
            result?;
            cleanup?;
            Ok(())
        })();
        let cleanup =
            std::fs::remove_dir_all(&root_a).map_err(|error| format!("remove temp root: {error}"));
        result?;
        cleanup?;
        Ok(())
    }

    /// (#2823 test 3) Revision-only movement in one root: the input identity
    /// is unchanged (the revision is not a semantic input) while the snapshot
    /// identity tracks the new head.
    #[test]
    fn repo_exposure_revision_movement_keeps_input_identity_and_moves_snapshot()
    -> Result<(), String> {
        let root = temporary_git_root()?;
        let result = (|| -> Result<(), String> {
            commit_fixture_file(&root)?;
            let config = crate::config::RiprConfig::default();
            let (before, before_metadata) = artifact_metadata(&root, "draft", None, &config)?;
            commit_empty(&root, "advance")?;
            let (after, after_metadata) = artifact_metadata(&root, "draft", None, &config)?;
            if before.input_identity != after.input_identity {
                return Err(
                    "revision-only movement must not change the portable input identity"
                        .to_string(),
                );
            }
            if before_metadata["repository"]["head"] == after_metadata["repository"]["head"] {
                return Err("the fixture commit must move the repository head".to_string());
            }
            if before_metadata["snapshot_identity"] == after_metadata["snapshot_identity"] {
                return Err("revision movement must change the snapshot identity".to_string());
            }
            Ok(())
        })();
        let cleanup =
            std::fs::remove_dir_all(&root).map_err(|error| format!("remove temp root: {error}"));
        result?;
        cleanup?;
        Ok(())
    }

    /// (#2823 test 4) Movement of every semantic input — mode, base, config,
    /// manifest content, lockfile content — changes the input identity. The
    /// profile is bound to the mode by envelope validation
    /// (`profile == mode`), so a distinct-profile construction cannot reach
    /// the validator; the canonical string states `profile=` explicitly so a
    /// future producer that separates the two is versioned honestly.
    #[test]
    fn repo_exposure_input_identity_tracks_every_semantic_input() -> Result<(), String> {
        let root = temporary_git_root()?;
        let result = (|| -> Result<(), String> {
            commit_fixture_file(&root)?;
            let config = crate::config::RiprConfig::default();
            let baseline = RepoExposureArtifactContext::for_repo_exposure(
                root.clone(),
                "draft".to_string(),
                None,
                &config,
            )?;
            let expect_drift =
                |case: &str, moved: &RepoExposureArtifactContext| -> Result<(), String> {
                    if moved.input_identity == baseline.input_identity {
                        return Err(format!("{case} must change the input identity"));
                    }
                    Ok(())
                };

            let moved_mode = RepoExposureArtifactContext::for_repo_exposure(
                root.clone(),
                "deep".to_string(),
                None,
                &config,
            )?;
            expect_drift("mode movement", &moved_mode)?;

            let moved_base = RepoExposureArtifactContext::for_repo_exposure(
                root.clone(),
                "draft".to_string(),
                Some("origin/main".to_string()),
                &config,
            )?;
            expect_drift("base revision movement", &moved_base)?;

            let moved_config =
                crate::config::tests_only_parse("[oracles]\nsnapshot_strength = \"strong\"\n")?;
            if crate::config::check_artifact_config_identity_hash(&moved_config)
                == crate::config::check_artifact_config_identity_hash(&config)
            {
                return Err(
                    "the oracle-policy fixture must change the config identity hash".to_string(),
                );
            }
            let moved_config_context = RepoExposureArtifactContext::for_repo_exposure(
                root.clone(),
                "draft".to_string(),
                None,
                &moved_config,
            )?;
            expect_drift("config identity movement", &moved_config_context)?;

            std::fs::write(root.join("Cargo.toml"), "[workspace]\n# moved manifest\n")
                .map_err(|error| format!("move manifest: {error}"))?;
            let moved_manifest = RepoExposureArtifactContext::for_repo_exposure(
                root.clone(),
                "draft".to_string(),
                None,
                &config,
            )?;
            expect_drift("manifest content movement", &moved_manifest)?;

            std::fs::write(root.join("Cargo.lock"), "# moved lockfile\nversion = 4\n")
                .map_err(|error| format!("move lockfile: {error}"))?;
            let moved_lockfile = RepoExposureArtifactContext::for_repo_exposure(
                root.clone(),
                "draft".to_string(),
                None,
                &config,
            )?;
            expect_drift("lockfile content movement", &moved_lockfile)?;
            Ok(())
        })();
        let cleanup =
            std::fs::remove_dir_all(&root).map_err(|error| format!("remove temp root: {error}"));
        result?;
        cleanup?;
        Ok(())
    }

    /// (#2823 review repair) The config boundary is scoped to what the
    /// repo-exposure producer actually consumes: a config differing from the
    /// default only in `typescript.resolve_tsconfig_paths`, `perl.*`, or
    /// `languages.enabled` — inputs the Rust-only seam inventory never reads —
    /// produces the SAME input identity (the pair stays comparable), while an
    /// oracle-policy change still moves it (positive control, also covered by
    /// test 4).
    #[test]
    fn repo_exposure_input_identity_ignores_unconsumed_config_fields() -> Result<(), String> {
        let root = temporary_git_root()?;
        let result = (|| -> Result<(), String> {
            commit_fixture_file(&root)?;
            let config = crate::config::RiprConfig::default();
            let baseline = RepoExposureArtifactContext::for_repo_exposure(
                root.clone(),
                "draft".to_string(),
                None,
                &config,
            )?;
            // Negative controls: SPEC-0140 finding-affecting fields the seam
            // inventory does not consume must NOT move the identity. First
            // pin that each fixture really does move the diff-check config
            // hash, so a vacuous fixture cannot hide a real regression.
            let unconsumed = [
                (
                    "typescript.resolve_tsconfig_paths",
                    "[typescript]\nresolve_tsconfig_paths = true\n",
                ),
                ("perl.timeout_ms", "[perl]\ntimeout_ms = 1234\n"),
            ];
            for (case, toml) in unconsumed {
                let moved = crate::config::tests_only_parse(toml)?;
                if crate::config::check_artifact_config_identity_hash(&moved)
                    == crate::config::check_artifact_config_identity_hash(&config)
                {
                    return Err(format!(
                        "{case} fixture must move the diff-check (SPEC-0140) config hash, otherwise this test proves nothing"
                    ));
                }
                if crate::config::repo_exposure_config_identity_hash(&moved)
                    != crate::config::repo_exposure_config_identity_hash(&config)
                {
                    return Err(format!(
                        "{case} is not consumed by the repo-exposure producer and must not move its config identity"
                    ));
                }
                let context = RepoExposureArtifactContext::for_repo_exposure(
                    root.clone(),
                    "draft".to_string(),
                    None,
                    &moved,
                )?;
                if context.input_identity != baseline.input_identity {
                    return Err(format!(
                        "{case} must leave the repo-exposure input identity unchanged (comparable pair)"
                    ));
                }
            }
            // `languages.enabled` is recorded elsewhere in the diff-check
            // identity (SPEC-0140 CapturedElsewhere) and unconsumed by the
            // Rust-only seam inventory, so it must not move this identity
            // either.
            let languages_moved = crate::config::tests_only_parse(
                "[languages]\nenabled = [\"rust\", \"typescript\"]\n",
            )?;
            let languages_context = RepoExposureArtifactContext::for_repo_exposure(
                root.clone(),
                "draft".to_string(),
                None,
                &languages_moved,
            )?;
            if languages_context.input_identity != baseline.input_identity {
                return Err(
                    "languages.enabled must leave the repo-exposure input identity unchanged"
                        .to_string(),
                );
            }
            // Positive control: a consumed oracle field still moves it.
            let oracles_moved =
                crate::config::tests_only_parse("[oracles]\nsnapshot_strength = \"strong\"\n")?;
            let oracles_context = RepoExposureArtifactContext::for_repo_exposure(
                root.clone(),
                "draft".to_string(),
                None,
                &oracles_moved,
            )?;
            if oracles_context.input_identity == baseline.input_identity {
                return Err(
                    "a consumed oracle-policy change must move the input identity".to_string(),
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

    /// (#2823 test 5) A rerun at the same root and same revision is
    /// byte-stable in both identities.
    #[test]
    fn repo_exposure_identity_is_byte_stable_across_equivalent_reruns() -> Result<(), String> {
        let root = temporary_git_root()?;
        let result = (|| -> Result<(), String> {
            commit_fixture_file(&root)?;
            let config = crate::config::RiprConfig::default();
            let (first, first_metadata) = artifact_metadata(&root, "draft", None, &config)?;
            let (second, second_metadata) = artifact_metadata(&root, "draft", None, &config)?;
            if first.input_identity != second.input_identity {
                return Err("same-root reruns must keep a byte-stable input identity".to_string());
            }
            if first_metadata["snapshot_identity"] != second_metadata["snapshot_identity"] {
                return Err(
                    "same-root same-revision reruns must keep a byte-stable snapshot identity"
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

    /// (#2823 test 6, removal-experiment oracle) Reintroducing the absolute
    /// root into the canonical string — the v1 algorithm, rebuilt here in the
    /// test — fails the two-root portability property. This proves the
    /// portability test discriminates: it would catch a regression that
    /// re-binds the identity to the concrete checkout.
    #[test]
    fn repo_exposure_root_bound_identity_is_not_portable_across_checkout_roots()
    -> Result<(), String> {
        let root_a = temporary_git_root()?;
        let result = (|| -> Result<(), String> {
            commit_fixture_file(&root_a)?;
            let root_b = clone_git_root(&root_a)?;
            let result = (|| -> Result<(), String> {
                let v1_identity = |root: &Path| -> Result<String, String> {
                    let canonical_root = canonical_root(root)?;
                    // The v1 algorithm hashed absolute-path manifest/lockfile
                    // identities as well; reproduce it faithfully.
                    let (manifest_identity, lockfile_identity) =
                        crate::analysis::seam_cache::workspace_named_file_identities(
                            &canonical_root,
                        );
                    let input_canonical = format!(
                        "root={};base={:?};mode={};format=repo-exposure-json;manifest={:?};lockfile={:?};analyzer={}",
                        display_root(&canonical_root),
                        None::<String>,
                        "draft",
                        manifest_identity,
                        lockfile_identity,
                        env!("CARGO_PKG_VERSION"),
                    );
                    Ok(format!(
                        "input:{}",
                        crate::config::config_fingerprint(&input_canonical)
                    ))
                };
                let identity_a = v1_identity(&root_a)?;
                let identity_b = v1_identity(&root_b)?;
                if identity_a == identity_b {
                    return Err(
                        "the root-bound v1 algorithm must NOT be portable across checkout roots; \
                         if it is, the portability test does not discriminate"
                            .to_string(),
                    );
                }
                // And the v2 identity must disagree with the v1 shape: the
                // version prefix is a real algorithm boundary, not a relabel.
                let config = crate::config::RiprConfig::default();
                let (context_a, _) = artifact_metadata(&root_a, "draft", None, &config)?;
                if context_a.input_identity == identity_a {
                    return Err(
                        "v2 must not reproduce the v1 digest for the same checkout".to_string()
                    );
                }
                Ok(())
            })();
            let cleanup = std::fs::remove_dir_all(&root_b)
                .map_err(|error| format!("remove clone root: {error}"));
            result?;
            cleanup?;
            Ok(())
        })();
        let cleanup =
            std::fs::remove_dir_all(&root_a).map_err(|error| format!("remove temp root: {error}"));
        result?;
        cleanup?;
        Ok(())
    }

    /// (#2823 test 7) A previous-version input identity — the v1 and v2
    /// `input:<digest>` shape, or any non-`input:v3:` value — never validates
    /// as current evidence, even when the artifact is otherwise internally
    /// consistent; nor does a current-version identity whose digest is not
    /// exactly `fnv1a64:<16 lowercase hex>`.
    #[test]
    fn repo_exposure_validation_rejects_unsupported_input_identity_version() -> Result<(), String> {
        let root = temporary_git_root()?;
        let result = (|| -> Result<(), String> {
            commit_fixture_file(&root)?;
            let placeholder_raw = repo_exposure_raw_with_placeholder(&root)?;
            let document: Value = serde_json::from_str(&placeholder_raw)
                .map_err(|error| format!("parse placeholder document: {error}"))?;
            let head = document["artifact"]["repository"]["head"]
                .as_str()
                .ok_or_else(|| "fixture document omitted repository head".to_string())?
                .to_string();
            let legacy_identities = [
                "input:fnv1a64:0123456789abcdef",
                "input:legacy-unversioned",
                "input:v1:fnv1a64:0123456789abcdef",
                "input:v2:fnv1a64:0123456789abcdef",
            ];
            for legacy in legacy_identities {
                let legacy_snapshot = repo_exposure_snapshot_identity(legacy, &head);
                let mutated = validate_mutated_identity(&root, &document, |document| {
                    document["artifact"]["analysis"]["input_identity"] = json!(legacy);
                    document["artifact"]["snapshot_identity"] = json!(legacy_snapshot.as_str());
                })?;
                match mutated {
                    Err(error) if error.contains("unsupported input identity version") => {}
                    Err(error) => {
                        return Err(format!(
                            "legacy identity {legacy}: unexpected rejection reason (must name the unsupported input identity version): {error}"
                        ));
                    }
                    Ok(_) => {
                        return Err(format!(
                            "legacy identity {legacy} must never validate as current evidence"
                        ));
                    }
                }
            }

            // Shape gate: the current version prefix with anything but
            // exactly `fnv1a64:<16 lowercase hex>` is malformed, not merely
            // an unknown version, and gets its own bounded reason.
            let malformed_identities = [
                "input:v3:garbage",
                "input:v3:fnv1a64:",
                "input:v3:fnv1a64:0123456789abcde",
                "input:v3:fnv1a64:0123456789abcdef0",
                "input:v3:fnv1a64:0123456789ABCDEF",
            ];
            for malformed in malformed_identities {
                let malformed_snapshot = repo_exposure_snapshot_identity(malformed, &head);
                let mutated = validate_mutated_identity(&root, &document, |document| {
                    document["artifact"]["analysis"]["input_identity"] = json!(malformed);
                    document["artifact"]["snapshot_identity"] = json!(malformed_snapshot.as_str());
                })?;
                match mutated {
                    Err(error) if error.contains("malformed input identity digest") => {}
                    Err(error) => {
                        return Err(format!(
                            "malformed identity {malformed}: unexpected rejection reason (must name the malformed input identity digest): {error}"
                        ));
                    }
                    Ok(_) => {
                        return Err(format!(
                            "malformed identity {malformed} must never validate as current evidence"
                        ));
                    }
                }
            }

            // Positive control: the canonical producer shape validates, and a
            // well-formed foreign digest reaches the later checks (here: the
            // snapshot mismatch), proving the shape gate does not reject
            // well-formed identities.
            let well_formed = "input:v3:fnv1a64:fedcba9876543210";
            let mutated = validate_mutated_identity(&root, &document, |document| {
                document["artifact"]["analysis"]["input_identity"] = json!(well_formed);
            })?;
            match mutated {
                Err(error) if error.contains("snapshot identity does not match") => {}
                Err(error) => {
                    return Err(format!(
                        "well-formed identity: unexpected rejection reason (must pass the version and shape gates): {error}"
                    ));
                }
                Ok(_) => {
                    return Err(
                        "well-formed identity with a stale snapshot must be rejected by the snapshot check, not accepted"
                            .to_string(),
                    );
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
