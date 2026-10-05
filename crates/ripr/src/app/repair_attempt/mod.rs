//! Durable repair-attempt identity, retained inputs, and finish authority.
//!
//! Repository-global artifacts under `target/ripr/workflow` remain compatibility
//! projections. The durable transaction is selected by `RepairAttemptId`, and
//! the after phase consumes the retained before snapshot and packet attached to
//! that exact attempt. Store location is owned by [`store`]: before, status,
//! and after resolve one store identity instead of joining a hardcoded path.

mod store;

use crate::agent::loop_commands::{bound_root, display_path, root_path_display, shell_arg};
use crate::analysis::is_test_surface_path;
use crate::edit_cage::{
    AttemptBaseline, EditCagePolicy, EditCageVerdict, HeadMovement,
    evaluate_repository_edit_cage_with_head_movement,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

pub(crate) const REPAIR_ATTEMPT_SCHEMA_VERSION: &str = "0.1";
pub(crate) const REPAIR_ATTEMPT_DIRECTORY: &str = "target/ripr/repair-attempts";
const REPAIR_ATTEMPT_MANIFEST: &str = "attempt.json";
const REPAIR_ATTEMPT_COMMITMENT: &str = "before-commitment.sha256";
const REPAIR_ATTEMPT_ARTIFACTS_DIRECTORY: &str = "artifacts";
const REPAIR_ATTEMPT_ID_PREFIX: &str = "repair-attempt-";
const REPAIR_ATTEMPT_ID_HEX_LEN: usize = 24;
/// Role of the attempt-local agent receipt retained at finish. The
/// repository-global `target/ripr/reports/agent-receipt.json` file is a
/// one-slot compatibility projection of the latest finish, not the
/// authoritative copy.
pub(crate) const TERMINAL_RECEIPT_ROLE: &str = "agent_receipt";
/// Role of the verify document the after phase used to produce that receipt.
pub(crate) const TERMINAL_VERIFY_ROLE: &str = "agent_verify";
/// Cargo's default build directory, relative to the workspace root.
const CARGO_DEFAULT_BUILD_OUTPUT_DIR: &str = "target";
/// The lockfile Cargo writes at the workspace root when it resolves
/// dependencies.
const CARGO_WORKSPACE_LOCKFILE: &str = "Cargo.lock";

pub(crate) use store::{
    RepairAttemptStoreAccess, RepairAttemptStoreCurrentness, RepairAttemptStoreIdentity,
    RepairAttemptStoreLocationClass, RepairAttemptStoreRef, quoted_store_flag,
    quoted_store_flag_from_identity, resolve_store,
};

static ATTEMPT_NONCE: AtomicU64 = AtomicU64::new(0);

#[derive(Clone, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(transparent)]
pub(crate) struct RepairAttemptId(String);

impl RepairAttemptId {
    pub(crate) fn parse(value: impl Into<String>) -> Result<Self, String> {
        let value = value.into();
        let Some(suffix) = value.strip_prefix(REPAIR_ATTEMPT_ID_PREFIX) else {
            return Err(format!(
                "repair attempt ID must start with `{REPAIR_ATTEMPT_ID_PREFIX}`"
            ));
        };
        if suffix.len() != REPAIR_ATTEMPT_ID_HEX_LEN
            || !suffix
                .bytes()
                .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
        {
            return Err(format!(
                "repair attempt ID suffix must contain {REPAIR_ATTEMPT_ID_HEX_LEN} lowercase hexadecimal characters"
            ));
        }
        Ok(Self(value))
    }

    pub(crate) fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum RepairAttemptState {
    Prepared,
    AwaitingEdit,
    ReadyToFinish,
    Stale,
    Incomparable,
    Failed,
}

impl RepairAttemptState {
    /// The serialized spelling of the state, for surfaces that must use the
    /// same vocabulary the manifest serializes (the #6033 after-phase
    /// failure envelope). One owner beside the enum so the serde rename and
    /// this label cannot drift.
    pub(crate) fn as_label(&self) -> &'static str {
        match self {
            Self::Prepared => "prepared",
            Self::AwaitingEdit => "awaiting_edit",
            Self::ReadyToFinish => "ready_to_finish",
            Self::Stale => "stale",
            Self::Incomparable => "incomparable",
            Self::Failed => "failed",
        }
    }
}

/// The durable state a finished attempt carries for a given finish outcome.
/// One owner beside the finish transition, so the manifest write, `agent
/// status`, and the #6033 after-phase failure envelope cannot drift.
pub(crate) fn repair_attempt_state_for_finish(after: &RepairAttemptAfter) -> RepairAttemptState {
    if after.current {
        match after.verdict.status {
            crate::edit_cage::EditCageVerdictStatus::Compliant => RepairAttemptState::ReadyToFinish,
            crate::edit_cage::EditCageVerdictStatus::Violated => RepairAttemptState::Failed,
            crate::edit_cage::EditCageVerdictStatus::Incomparable => {
                RepairAttemptState::Incomparable
            }
        }
    } else {
        RepairAttemptState::Stale
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct RepairAttemptArtifact {
    pub(crate) role: String,
    pub(crate) path: String,
    pub(crate) sha256: String,
    pub(crate) bytes: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct RepairAttemptManifest {
    pub(crate) schema_version: String,
    pub(crate) kind: String,
    pub(crate) repair_attempt_id: RepairAttemptId,
    pub(crate) state: RepairAttemptState,
    pub(crate) root: String,
    pub(crate) repository_head: String,
    pub(crate) producer_version: String,
    pub(crate) seam_id: String,
    pub(crate) created_unix_ms: u64,
    pub(crate) artifacts: Vec<RepairAttemptArtifact>,
    pub(crate) next_command: String,
    pub(crate) limitations: Vec<String>,
    pub(crate) non_claims: Vec<String>,
    #[serde(default)]
    pub(crate) after: Option<RepairAttemptAfter>,
    /// The refusal of this attempt's most recent after phase, when that phase
    /// refused after selecting the attempt. It is an observation, not a state:
    /// it never changes `state` or `after`, the before commitment excludes it,
    /// and the next after phase that reaches the durable finish clears it.
    /// Absent (not `null`) when no refusal is recorded, so manifests without
    /// one keep their exact bytes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) last_after_refusal: Option<RepairAttemptAfterRefusal>,
    /// Terminal static-result artifacts retained at finish (`agent_receipt`,
    /// and the `agent_verify` document it was built from). Absent or empty on
    /// legacy manifests. Excluded from the before commitment, so adding them
    /// cannot rewrite prepared inputs. Status prefers these over the
    /// one-slot compatibility receipt.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) terminal_artifacts: Vec<RepairAttemptArtifact>,
    /// Portable store identity. Absent on default-store and legacy manifests
    /// so ordinary before → after bytes stay compatible. Explicit stores
    /// record the locator; absolute checkout spelling is not identity.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) store: Option<RepairAttemptStoreIdentity>,
}

/// Why the last after phase of an attempt refused, recorded by the attempt
/// authority so `ripr agent status` can report the outcome instead of
/// repeating the refused command as if nothing had happened.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct RepairAttemptAfterRefusal {
    /// The refusal message the after phase exited with, bounded to
    /// `REPAIR_ATTEMPT_REFUSAL_MAX_BYTES`.
    pub(crate) reason: String,
    /// The repository HEAD when the refusal was recorded, or `None` when it
    /// could not be read.
    pub(crate) repository_head: Option<String>,
    pub(crate) recorded_unix_ms: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct RepairAttemptAfter {
    pub(crate) attempt_id: RepairAttemptId,
    pub(crate) repository_head: String,
    pub(crate) delta_sha256: String,
    pub(crate) packet_sha256: String,
    pub(crate) current: bool,
    pub(crate) verdict: EditCageVerdict,
}

fn open_attempt(
    root: &Path,
    store: Option<&Path>,
    attempt_id: &RepairAttemptId,
) -> Result<(RepairAttemptStoreRef, PathBuf, RepairAttemptManifest), String> {
    let store = resolve_store(root, store, RepairAttemptStoreAccess::Open)?;
    let (path, manifest) = load_repair_attempt_by_id(&store, attempt_id)?;
    Ok((store, path, manifest))
}

/// Opens the attempt and returns the raw snapshot bytes the manifest was
/// decoded from, so committing writers can prove their base matches their
/// parse (see `load_repair_attempt_snapshot_by_id`).
fn open_attempt_with_bytes(
    root: &Path,
    store: Option<&Path>,
    attempt_id: &RepairAttemptId,
) -> Result<
    (
        RepairAttemptStoreRef,
        PathBuf,
        Vec<u8>,
        RepairAttemptManifest,
    ),
    String,
> {
    let store = resolve_store(root, store, RepairAttemptStoreAccess::Open)?;
    let (path, raw, manifest) = load_repair_attempt_snapshot_by_id(&store, attempt_id)?;
    Ok((store, path, raw, manifest))
}

/// Resolves the retained packet an attempt-bound receipt must bind against.
/// The public `agent receipt --attempt <id>` route reads this attempt's
/// retained packet — the same authority the after phase consumes — instead
/// of the repository-global compatibility packet, which a later attempt for
/// any seam silently replaces (#4332).
pub(crate) fn retained_attempt_packet_path(
    root: &Path,
    seam_id: &str,
    attempt_id: &str,
) -> Result<PathBuf, String> {
    let store = resolve_store(root, None, RepairAttemptStoreAccess::Open)?;
    let attempt_id = RepairAttemptId::parse(attempt_id.to_string())?;
    let (_, manifest) = load_repair_attempt_by_id(&store, &attempt_id)?;
    if manifest.seam_id != seam_id {
        return Err(format!(
            "repair attempt {} belongs to seam `{}`, not `{seam_id}`",
            attempt_id.as_str(),
            manifest.seam_id
        ));
    }
    let packet = find_manifest_artifact(&manifest, "agent_packet")?;
    Ok(store.canonical_root().join(&packet.path))
}

/// The only attempt state that may authorize a receipt. This is deliberately
/// derived from the durable manifest and its immutable before artifacts rather
/// than from the workflow filenames, which are compatibility outputs.
pub(crate) fn receipt_binding_from(
    root: &Path,
    store: Option<&Path>,
    seam_id: &str,
    packet_path: &Path,
    attempt_id: Option<&str>,
) -> Result<serde_json::Value, String> {
    // Refusals name the root the caller typed, bound lexically against the
    // working directory so a pasted command runs from anywhere (#3999), never
    // the canonicalized verbatim path the store resolver hands back below
    // (#4332); the resolver owns the store identity and its canonicalization
    // (#4797).
    let root_display = bound_root(&root.to_string_lossy());
    let store = resolve_store(root, store, RepairAttemptStoreAccess::Open)?;
    let root = store.canonical_root().to_path_buf();
    let packet = std::fs::read(packet_path).map_err(|error| {
        format!(
            "read repair packet {} failed: {error}",
            packet_path.display()
        )
    })?;
    let packet_sha256 = sha256_bytes(&packet);
    let mut policy = edit_cage_policy_from_packet(
        std::str::from_utf8(&packet)
            .map_err(|error| format!("repair packet is not UTF-8: {error}"))?,
        seam_id,
    )?;
    include_explicit_store_operational_write(
        &mut policy,
        store.canonical_root(),
        Some(Path::new(store.locator())),
    )?;
    let (manifest_path, manifest) = if let Some(attempt_id) = attempt_id {
        let attempt_id = RepairAttemptId::parse(attempt_id.to_string())?;
        let loaded = load_repair_attempt_by_id(&store, &attempt_id)?;
        if loaded.1.seam_id != seam_id {
            return Err(format!(
                "repair attempt {} belongs to seam `{}`, not `{seam_id}`",
                attempt_id.as_str(),
                loaded.1.seam_id
            ));
        }
        loaded
    } else {
        let mut matches = Vec::new();
        for entry in inventory_repair_attempts_in(&store)? {
            let RepairAttemptInventoryEntry::Valid(manifest) = entry else {
                continue;
            };
            if manifest.seam_id == seam_id {
                let path = store
                    .attempt_directory(&manifest.repair_attempt_id)
                    .join(REPAIR_ATTEMPT_MANIFEST);
                matches.push((path, *manifest));
            }
        }
        if matches.len() != 1 {
            // #4332: the message names the working next action in both
            // directions. Found zero: there is nothing to pick, so it names
            // the start command. Found many: it names the ids so the agent
            // can select one with the receipt's own `--attempt` flag instead
            // of rerunning to discover them.
            let restart = format!(
                "ripr agent repair --root {} --seam-id {} --phase before",
                shell_arg(&root_display),
                shell_arg(seam_id)
            );
            let ids = matches
                .iter()
                .map(|(_, manifest)| manifest.repair_attempt_id.as_str())
                .collect::<Vec<_>>()
                .join(", ");
            let detail = if matches.is_empty() {
                format!("no repair attempt exists for seam `{seam_id}`; start one with `{restart}`")
            } else {
                format!(
                    "found {}: {ids}; pass --attempt <id> to select the attempt the receipt binds to exactly",
                    matches.len()
                )
            };
            return Err(format!(
                "receipt requires exactly one repair attempt for seam `{seam_id}`: {detail}"
            ));
        }
        matches.pop().ok_or_else(|| "missing attempt".to_string())?
    };
    let after = manifest
        .after
        .as_ref()
        .ok_or_else(|| "repair attempt has no after-phase verdict".to_string())?;
    if manifest.state != RepairAttemptState::ReadyToFinish
        || !after.current
        || after.verdict.status != crate::edit_cage::EditCageVerdictStatus::Compliant
    {
        // #4332: the refusal uses the vocabulary the serialized manifest and
        // the cage verdict use (`ready_to_finish`, `compliant`), not Debug
        // spellings the agent just read differently, and glosses `current`
        // because a bare `current false` does not say what moved.
        let current_gloss = if after.current {
            String::new()
        } else {
            " (repository HEAD moved since the after phase recorded its verdict; `ripr agent status` reads which head)".to_string()
        };
        return Err(format!(
            "repair attempt {} is not receipt-ready: state `{}`, current {}{}, verdict `{}`",
            manifest.repair_attempt_id.as_str(),
            repair_attempt_state_label(&manifest.state),
            after.current,
            current_gloss,
            after.verdict.status.as_label(),
        ));
    }
    // The receipt names the after head the finish recorded. That head is the
    // prepared head, or a descendant reached only by commits the edit cage
    // evaluated (a committed focused test); anything else is stale.
    let current_head = crate::agent::artifact::current_git_head(&root)?;
    if current_head != after.repository_head
        || (current_head != manifest.repository_head
            && !crate::agent::artifact::git_merge_base_is_ancestor(
                &root,
                &manifest.repository_head,
                &current_head,
            )?)
    {
        return Err("repair attempt receipt is stale relative to repository HEAD".to_string());
    }
    let packet_artifact = find_manifest_artifact(&manifest, "agent_packet")?;
    let packet_artifact_bytes = std::fs::read(root.join(&packet_artifact.path))
        .map_err(|error| format!("read staged agent packet failed: {error}"))?;
    if sha256_bytes(&packet_artifact_bytes) != packet_artifact.sha256
        || packet_artifact.sha256 != packet_sha256
        || after.packet_sha256 != packet_sha256
    {
        return Err("repair attempt packet binding is tampered or replayed".to_string());
    }
    let baseline_artifact = find_manifest_artifact(&manifest, "edit_cage_baseline")?;
    let baseline_bytes = std::fs::read(root.join(&baseline_artifact.path))
        .map_err(|error| format!("read staged edit-cage baseline failed: {error}"))?;
    if sha256_bytes(&baseline_bytes) != baseline_artifact.sha256 {
        return Err("repair attempt edit-cage baseline binding is tampered".to_string());
    }
    let baseline: AttemptBaseline = serde_json::from_slice(&baseline_bytes)
        .map_err(|error| format!("decode edit-cage baseline failed: {error}"))?;
    // Recomputed under the admitting rule: for an attempt whose head did not
    // move it is identical to the exact-head rule, and an attempt whose finish
    // required the exact head and saw it move is already refused as stale.
    let (delta, _) = evaluate_repository_edit_cage_with_head_movement(
        &baseline,
        HeadMovement::AdmitDescendantCommits,
    )?;
    // The after phase, a rerun of `ripr agent receipt`, and `ripr agent
    // status` write command-owned files under `target/ripr` after the finish
    // bound its verdict. Those later operational writes are not part of the
    // attempt; any other movement still breaks the binding.
    let delta = crate::edit_cage::without_later_operational_writes(
        baseline.policy(),
        &delta,
        &after.verdict.changed_paths,
    );
    let verdict = crate::edit_cage::evaluate_edit_cage(baseline.policy(), &delta);
    let delta_bytes = serde_json::to_vec(&delta)
        .map_err(|error| format!("serialize repair delta failed: {error}"))?;
    if sha256_bytes(&delta_bytes) != after.delta_sha256 || verdict != after.verdict {
        return Err("repair attempt after verdict binding is tampered or stale".to_string());
    }
    validate_trusted_head_surface(
        &root,
        &policy,
        &verdict.changed_paths,
        &manifest.repository_head,
    )?;
    let manifest_path = display_path(&manifest_path);
    Ok(serde_json::json!({
        "attempt_id": after.attempt_id.as_str(),
        "manifest": manifest_path,
        "seam_id": manifest.seam_id,
        "before_head": manifest.repository_head,
        "after_head": after.repository_head,
        "packet_sha256": after.packet_sha256,
        "delta_sha256": after.delta_sha256,
        "current": after.current,
        "edit_cage_verdict": after.verdict,
    }))
}

/// Resolve the retained before snapshot a verify document must name, and read
/// its bytes. Shared path binding for receipt issuance
/// ([`validate_verify_binding_from`]) and stored-receipt reads
/// ([`validate_verify_before_commitment_to_manifest`]); each caller keeps its
/// own content-digest check by design (full live validation at issuance,
/// commitment recomputation at read).
fn read_retained_before_snapshot(
    root: &Path,
    manifest: &RepairAttemptManifest,
    verify_before_path: &str,
) -> Result<String, String> {
    let attempt_id = manifest.repair_attempt_id.as_str();
    let expected = find_manifest_artifact(manifest, "before_snapshot")?;
    let expected_path = root
        .join(&expected.path)
        .canonicalize()
        .map_err(|error| format!("canonicalize retained before snapshot failed: {error}"))?;
    let actual_path = root
        .join(verify_before_path)
        .canonicalize()
        .map_err(|error| format!("canonicalize verify before snapshot failed: {error}"))?;
    if actual_path != expected_path {
        return Err(format!(
            "verify before snapshot {} is not the retained snapshot for attempt {}",
            actual_path.display(),
            attempt_id
        ));
    }
    std::fs::read_to_string(&expected_path).map_err(|error| {
        format!(
            "read retained before snapshot {} failed: {error}",
            expected_path.display()
        )
    })
}

/// Ensure a verify document consumed for an exact attempt names that attempt's
/// retained before snapshot and its committed content digest.
pub(crate) fn validate_verify_binding_from(
    root: &Path,
    store: Option<&Path>,
    attempt_id: &str,
    verify_before_path: &str,
    verify_before_sha256: &str,
) -> Result<(), String> {
    let attempt_id = RepairAttemptId::parse(attempt_id.to_string())?;
    let (store, _, manifest) = open_attempt(root, store, &attempt_id)?;
    let root = store.canonical_root().to_path_buf();
    let before_snapshot = read_retained_before_snapshot(&root, &manifest, verify_before_path)?;
    let validated = crate::agent::artifact::validate_repo_exposure_artifact(
        &root,
        &before_snapshot,
        "repair attempt before",
    )?;
    if verify_before_sha256 != validated.content_sha256 {
        return Err(format!(
            "verify before snapshot digest does not match attempt {}",
            attempt_id.as_str()
        ));
    }
    Ok(())
}

/// Bind a verify document's declared before content digest to the retained
/// before snapshot's recomputed commitment, for readers that already hold a
/// validated manifest. Unlike [`validate_verify_binding_from`] (receipt
/// issuance, which fully validates the evidence), this performs no
/// repository access beyond resolving the retained path: the retained bytes
/// are already commitment-anchored, and issued receipts must stay readable
/// at an unknown HEAD.
pub(crate) fn validate_verify_before_commitment_to_manifest(
    root: &Path,
    manifest: &RepairAttemptManifest,
    verify_before_path: &str,
    verify_before_sha256: &str,
) -> Result<(), String> {
    let attempt_id = manifest.repair_attempt_id.as_str();
    let before_snapshot = read_retained_before_snapshot(root, manifest, verify_before_path)?;
    let recomputed = crate::agent::artifact::recompute_content_commitment(&before_snapshot)?;
    if verify_before_sha256 != recomputed {
        return Err(format!(
            "verify before snapshot digest does not match attempt {attempt_id}"
        ));
    }
    Ok(())
}

/// Validate a stored receipt against the verify document it claims, binding
/// the verdict instead of trusting the receipt's verdict strings alone
/// (#5256). This is the one verdict-binding authority for the terminal
/// loader, the legacy compatibility read, and pending retention: every path
/// that can report `finished` re-validates the pair through it.
///
/// The checks, in order: the verify document carries the current
/// agent-verify schema; the receipt's recorded verify digest matches
/// `verify_bytes`; the receipt names this attempt's seam in both rendered
/// seam fields; the verify document names this attempt's retained before
/// snapshot with its committed content digest; the receipt's seam change,
/// movement, lifecycle state, and guidance kind equal the verdict the verify
/// document records for this seam; and the receipt's status equals what its
/// own recorded analysis-outcome projection renders. Anything else is a
/// binding failure, never an issued reading.
///
/// This is digest-plus-semantics validation over stored bytes, deliberately
/// not a canonical re-render: the named after snapshot may be superseded or
/// gone, so readers must not recompute the verify document here. Promotion
/// (pending retention) additionally runs the full canonical receipt-issuance
/// validation while the workflow files are fresh. No repository access beyond
/// path resolution happens here, so issued receipts stay readable at an
/// unknown HEAD; only the before commitment is recomputed from the retained
/// bytes, never re-validated as live evidence.
///
/// Passing this validator is necessary but not sufficient for an issued
/// reading: both read paths additionally run
/// [`canonically_admit_terminal_pair`], which refuses never-promoted and
/// jointly rewritten pairs whenever the named snapshots are still present.
pub(crate) fn validate_issued_receipt_evidence(
    root: &Path,
    manifest: &RepairAttemptManifest,
    receipt: &serde_json::Value,
    verify_bytes: &[u8],
) -> Result<(), String> {
    let verify: serde_json::Value = serde_json::from_slice(verify_bytes)
        .map_err(|error| format!("repair attempt verify document is not JSON: {error}"))?;
    let schema_version = verify
        .get("schema_version")
        .and_then(serde_json::Value::as_str);
    if schema_version != Some(crate::output::outcome::AGENT_VERIFY_SCHEMA_VERSION) {
        return Err(format!(
            "repair attempt verify document has unsupported schema version `{}`; expected `{}` from ripr agent verify",
            schema_version.unwrap_or("<missing>"),
            crate::output::outcome::AGENT_VERIFY_SCHEMA_VERSION
        ));
    }
    let recorded = receipt
        .pointer("/provenance/verify_artifact/sha256")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| {
            "repair attempt receipt does not record its verify artifact digest".to_string()
        })?;
    if recorded != sha256_bytes(verify_bytes) {
        return Err(
            "repair attempt receipt names a different verify document than the stored bytes"
                .to_string(),
        );
    }
    // The receipt must name the seam this attempt repaired, in both fields
    // the producer renders: a receipt minted for another seam — even one
    // with the same movement — is not this attempt's evidence.
    for pointer in ["/seam/seam_id", "/provenance/seam_id"] {
        let recorded_seam = receipt.pointer(pointer).and_then(serde_json::Value::as_str);
        if recorded_seam != Some(manifest.seam_id.as_str()) {
            return Err(format!(
                "repair attempt receipt {pointer} `{}` is not the attempt seam `{}`",
                recorded_seam.unwrap_or("<missing>"),
                manifest.seam_id
            ));
        }
    }
    let input_paths =
        crate::output::agent_receipt::agent_receipt_input_paths_from_value(&verify)
            .map_err(|error| format!("repair attempt verify document is malformed: {error}"))?;
    let before_sha256 = verify
        .pointer("/inputs/before_content_sha256")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| {
            "repair attempt verify document is missing before_content_sha256".to_string()
        })?;
    validate_verify_before_commitment_to_manifest(
        root,
        manifest,
        &input_paths.before,
        before_sha256,
    )?;
    let expected =
        crate::output::agent_receipt::expected_receipt_verdict(&verify, &manifest.seam_id)
            .map_err(|error| {
                format!(
                    "repair attempt verify document names no verdict for seam `{}`: {error}",
                    manifest.seam_id
                )
            })?;
    let field = |pointer: &str| receipt.pointer(pointer).and_then(serde_json::Value::as_str);
    if field("/seam/change") != Some(expected.change.as_str()) {
        return Err(format!(
            "repair attempt receipt seam change `{}` does not match the verify verdict `{}` for seam `{}`",
            field("/seam/change").unwrap_or("<missing>"),
            expected.change,
            manifest.seam_id
        ));
    }
    if field("/provenance/movement") != Some(expected.change.as_str()) {
        return Err(format!(
            "repair attempt receipt movement `{}` does not match the verify verdict `{}` for seam `{}`",
            field("/provenance/movement").unwrap_or("<missing>"),
            expected.change,
            manifest.seam_id
        ));
    }
    let state = field("/summary/receipt_state")
        .map(crate::output::receipt_lifecycle::normalize_receipt_lifecycle_state)
        .unwrap_or_else(|| crate::output::receipt_lifecycle::RECEIPT_MISSING.to_string());
    if state != expected.receipt_state {
        return Err(format!(
            "repair attempt receipt state `{}` does not match the verify verdict state `{}` for seam `{}`",
            field("/summary/receipt_state").unwrap_or("<missing>"),
            expected.receipt_state,
            manifest.seam_id
        ));
    }
    if field("/summary/next_action/kind") != Some(expected.guidance_kind.as_str()) {
        return Err(format!(
            "repair attempt receipt next-action kind `{}` does not match the verify verdict `{}` for seam `{}`",
            field("/summary/next_action/kind").unwrap_or("<missing>"),
            expected.guidance_kind,
            manifest.seam_id
        ));
    }
    // The receipt status must equal what its own recorded analysis-outcome
    // projection renders: flipping `incomplete` to `advisory` while the
    // projection still says incomplete is a binding failure, never finished.
    let projection = receipt
        .pointer("/analysis_outcome_status")
        .and_then(serde_json::Value::as_str);
    let Some(expected_status) =
        crate::output::agent_receipt::expected_agent_receipt_status_for_projection(projection)
    else {
        return Err(format!(
            "repair attempt receipt records no usable analysis outcome status (got `{}`)",
            projection.unwrap_or("<missing>")
        ));
    };
    if field("/status") != Some(expected_status) {
        return Err(format!(
            "repair attempt receipt status `{}` does not match its recorded analysis outcome status `{}`",
            field("/status").unwrap_or("<missing>"),
            projection.unwrap_or("<missing>")
        ));
    }
    Ok(())
}

/// How a stored verify document compares to the canonical render of the
/// snapshots it names, for readers that already passed
/// [`validate_issued_receipt_evidence`].
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum CanonicalAdmission {
    /// The named snapshots are present and the verify document matches their
    /// canonical render: the pair reads as issued.
    Admitted,
    /// The named snapshots are present and the verify document is not their
    /// canonical render: never-promoted or jointly rewritten evidence.
    Refused { reason: String },
    /// Admission cannot be evaluated: a named snapshot is gone (historical),
    /// or the repository HEAD cannot be read (unknown). The caller keeps the
    /// validator-only reading instead of inventing a refusal.
    Unavailable { reason: String },
}

/// Opportunistically admit a stored verify document against the canonical
/// render of the snapshots it names (#5256 review). This defeats
/// never-promoted invalid pairs and post-promotion joint rewrites — a verify
/// document rewritten together with its receipt to a consistent but false
/// verdict is not the canonical render — whenever the snapshots are still
/// present to compare against.
///
/// The comparison is semantic modulo `artifact_currentness`: that label is a
/// read-time observation of HEAD and worktree state, so requiring byte
/// identity would un-issue every retained receipt at any later or dirty
/// HEAD, contradicting durable readability (#5399). Every verdict-relevant
/// field — the inputs binding, seams, and summary — must match exactly.
///
/// Residual: a fully consistent fabrication of a HISTORICAL pair, whose
/// snapshots are gone, is outside the file-local model: with nothing to
/// compare against, the validator-only reading stands.
pub(crate) fn canonically_admit_terminal_pair(
    root: &Path,
    verify_bytes: &[u8],
) -> CanonicalAdmission {
    let verify_value: serde_json::Value = match serde_json::from_slice(verify_bytes) {
        Ok(value) => value,
        Err(error) => {
            return CanonicalAdmission::Refused {
                reason: format!("repair attempt verify document is not JSON: {error}"),
            };
        }
    };
    let input_paths =
        match crate::output::agent_receipt::agent_receipt_input_paths_from_value(&verify_value) {
            Ok(paths) => paths,
            Err(error) => {
                return CanonicalAdmission::Refused {
                    reason: format!("repair attempt verify document is malformed: {error}"),
                };
            }
        };
    for candidate in [&input_paths.before, &input_paths.after] {
        let path = Path::new(candidate);
        let resolved = if path.is_absolute() {
            path.to_path_buf()
        } else {
            root.join(path)
        };
        if !resolved.is_file() {
            return CanonicalAdmission::Unavailable {
                reason: format!(
                    "repair attempt verify document names snapshot `{candidate}` whose bytes are no longer present, so canonical admission cannot be evaluated"
                ),
            };
        }
    }
    // Currentness and lineage are evaluated against the live repository HEAD;
    // without it the canonical render cannot be recomputed, so an unknown
    // HEAD keeps the validator-only reading (downstream currentness reports
    // the unknown HEAD instead of inventing a refusal).
    if crate::agent::artifact::current_git_head(root).is_err() {
        return CanonicalAdmission::Unavailable {
            reason:
                "the repository HEAD cannot be read, so canonical admission cannot be evaluated"
                    .to_string(),
        };
    }
    let verify_text = match std::str::from_utf8(verify_bytes) {
        Ok(text) => text,
        Err(error) => {
            return CanonicalAdmission::Refused {
                reason: format!("repair attempt verify document is not UTF-8: {error}"),
            };
        }
    };
    let canonical =
        match crate::app::agent_receipt::canonical_agent_receipt_verify_render(root, verify_text) {
            Ok(canonical) => canonical,
            Err(reason) => return CanonicalAdmission::Refused { reason },
        };
    let mut supplied = verify_value;
    let mut expected = canonical.verify;
    for document in [&mut supplied, &mut expected] {
        if let Some(object) = document.as_object_mut() {
            object.remove("artifact_currentness");
        }
    }
    if supplied == expected {
        return CanonicalAdmission::Admitted;
    }
    CanonicalAdmission::Refused {
        reason: format!(
            "repair attempt verify document is not the canonical render of its named snapshots ({})",
            canonical_divergence_summary(&supplied, &expected)
        ),
    }
}

/// Name the top-level fields where a stored verify document diverges from the
/// canonical render, bounded for refusal messages.
fn canonical_divergence_summary(
    supplied: &serde_json::Value,
    expected: &serde_json::Value,
) -> String {
    let supplied_object = supplied.as_object();
    let expected_object = expected.as_object();
    let mut keys = BTreeSet::new();
    if let Some(object) = supplied_object {
        keys.extend(object.keys().cloned());
    }
    if let Some(object) = expected_object {
        keys.extend(object.keys().cloned());
    }
    let mut differing = Vec::new();
    for key in keys {
        let left = supplied_object.and_then(|object| object.get(&key));
        let right = expected_object.and_then(|object| object.get(&key));
        if left != right {
            differing.push(format!("`{key}`"));
            if differing.len() >= 4 {
                break;
            }
        }
    }
    if differing.is_empty() {
        return "the documents differ below the top level".to_string();
    }
    format!("differing fields: {}", differing.join(", "))
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct BeforeArtifactSource<'a> {
    pub(crate) role: &'a str,
    pub(crate) path: &'a Path,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct BeginRepairAttemptResult {
    pub(crate) manifest: RepairAttemptManifest,
    pub(crate) manifest_path: PathBuf,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ResolvedRepairAttempt {
    pub(crate) attempt_id: RepairAttemptId,
    pub(crate) seam_id: String,
    /// The repository HEAD the before phase prepared the attempt at.
    pub(crate) repository_head: String,
    pub(crate) manifest_path: PathBuf,
    pub(crate) before_snapshot_path: PathBuf,
    pub(crate) packet_path: PathBuf,
}

/// Inputs of one durable before-phase attempt publication.
///
/// `expected_repository_head` pins the repository head the caller verified
/// immediately before publication (for a trust-bound attempt, the binding's
/// head pin). The gate runs BEFORE anything is reserved or published, so a
/// HEAD move between preparation and publication refuses with a typed error
/// and no attempt record exists — a mismatched tree never strands an
/// `awaiting_edit` attempt.
///
/// `next_command_suffix` extends the published follow-up command. A
/// trust-bound attempt always re-verifies the explicit authorization at
/// apply time, so its follow-up must name the authorization pair with an
/// explicit placeholder identity; the driver never persists a granted
/// authorization.
#[derive(Clone, Copy)]
pub(crate) struct BeginRepairAttemptOptions<'a> {
    pub(crate) root: &'a Path,
    pub(crate) root_argument: &'a Path,
    pub(crate) seam_id: &'a str,
    pub(crate) sources: &'a [BeforeArtifactSource<'a>],
    pub(crate) expected_repository_head: Option<&'a str>,
    pub(crate) next_command_suffix: Option<&'a str>,
    /// Explicit store locator, resolved against `root`. `None` is the
    /// repository-local default and keeps next-command bytes compatible.
    pub(crate) store: Option<&'a Path>,
}

/// Private before-phase identity, allocated before the packet is rendered.
/// Allocation publishes nothing; the original transaction still owns
/// reservation, artifact commitments and final HEAD admission.
pub(crate) struct BeforeRepairAttemptIdentity {
    canonical_root: PathBuf,
    seam_id: String,
    repository_head: String,
    created_unix_ms: u64,
    repair_attempt_id: RepairAttemptId,
}

impl BeforeRepairAttemptIdentity {
    pub(crate) fn prepare(root: &Path, seam_id: &str) -> Result<Self, String> {
        if seam_id.trim().is_empty() {
            return Err("repair attempt requires a non-empty seam ID".to_string());
        }
        let canonical_root = root
            .canonicalize()
            .map_err(|error| format!("canonicalize repair attempt root failed: {error}"))?;
        let repository_head =
            crate::agent::artifact::current_git_head(&canonical_root).map_err(|error| {
                format!("repair attempt requires a concrete repository HEAD: {error}")
            })?;
        let created_unix_ms = current_unix_ms()?;
        let nonce = ATTEMPT_NONCE.fetch_add(1, Ordering::Relaxed);
        let repair_attempt_id = repair_attempt_id_from_parts(
            &display_path(&canonical_root),
            seam_id,
            &repository_head,
            created_unix_ms,
            std::process::id(),
            nonce,
        )?;
        Ok(Self {
            canonical_root,
            seam_id: seam_id.to_owned(),
            repository_head,
            created_unix_ms,
            repair_attempt_id,
        })
    }

    pub(crate) fn attempt_id(&self) -> &str {
        self.repair_attempt_id.as_str()
    }
}

#[cfg(test)]
pub(crate) fn begin_repair_attempt_with(
    options: BeginRepairAttemptOptions<'_>,
) -> Result<BeginRepairAttemptResult, String> {
    if options.seam_id.trim().is_empty() {
        return Err("repair attempt requires a non-empty seam ID".to_string());
    }
    if options.sources.is_empty() {
        return Err("repair attempt requires at least one before-phase artifact".to_string());
    }
    let identity = BeforeRepairAttemptIdentity::prepare(options.root, options.seam_id)?;
    begin_repair_attempt_with_identity(options, &identity)
}

pub(crate) fn begin_repair_attempt_with_identity(
    options: BeginRepairAttemptOptions<'_>,
    identity: &BeforeRepairAttemptIdentity,
) -> Result<BeginRepairAttemptResult, String> {
    let BeginRepairAttemptOptions {
        root,
        root_argument,
        seam_id,
        sources,
        expected_repository_head,
        next_command_suffix,
        store,
    } = options;
    if seam_id.trim().is_empty() {
        return Err("repair attempt requires a non-empty seam ID".to_string());
    }
    if sources.is_empty() {
        return Err("repair attempt requires at least one before-phase artifact".to_string());
    }

    let store = resolve_store(root, store, RepairAttemptStoreAccess::Prepare)?;
    let canonical_root = store.canonical_root();
    if canonical_root != identity.canonical_root.as_path() || seam_id != identity.seam_id {
        return Err(
            "prepared repair attempt identity does not match its root and seam".to_string(),
        );
    }
    let repository_head = crate::agent::artifact::current_git_head(canonical_root)
        .map_err(|error| format!("repair attempt requires a concrete repository HEAD: {error}"))?;
    // Pre-publication head gate: compare the caller's verified pin against
    // the repository HEAD this publication would record, before the attempt
    // directory is reserved. On mismatch nothing exists to clean up.
    if let Some(expected) = expected_repository_head
        && expected != repository_head
    {
        return Err(format!(
            "python repair-trust binding head moved during attempt publication; the binding pins head `{expected}` but the repository HEAD is now `{repository_head}`; re-run the before phase to prepare a fresh binding"
        ));
    }
    if repository_head != identity.repository_head {
        return Err(format!(
            "repository HEAD moved after repair attempt identity preparation; the packet pins head `{}` but the repository HEAD is now `{repository_head}`; re-run the before phase to prepare a fresh attempt",
            identity.repository_head,
        ));
    }
    let created_unix_ms = identity.created_unix_ms;
    let repair_attempt_id = identity.repair_attempt_id.clone();
    let attempt_directory = reserve_attempt_directory(&store, &repair_attempt_id)?;
    complete_repair_attempt(
        &store,
        &attempt_directory,
        AttemptPublication {
            root_argument,
            seam_id,
            repository_head,
            expected_repository_head,
            created_unix_ms,
            repair_attempt_id,
            sources,
            next_command_suffix,
        },
    )
}

struct AttemptPublication<'a> {
    root_argument: &'a Path,
    seam_id: &'a str,
    repository_head: String,
    /// The caller's verified head pin, when the publication is trust-bound.
    /// Re-checked immediately before the durable manifest write, so the
    /// finalize path verifies rather than trusting the earlier read.
    expected_repository_head: Option<&'a str>,
    created_unix_ms: u64,
    repair_attempt_id: RepairAttemptId,
    sources: &'a [BeforeArtifactSource<'a>],
    next_command_suffix: Option<&'a str>,
}

/// Stage artifacts and publish the manifest inside a reserved attempt
/// directory. Any failure removes the reserved directory, so a failed begin
/// leaves neither an orphan attempt nor a partial artifact set behind.
fn complete_repair_attempt(
    store: &RepairAttemptStoreRef,
    attempt_directory: &Path,
    publication: AttemptPublication<'_>,
) -> Result<BeginRepairAttemptResult, String> {
    let canonical_root = store.canonical_root();
    let result = stage_before_artifacts(canonical_root, attempt_directory, publication.sources)
        .and_then(|artifacts| {
            let store_flag = store.quoted_store_flag();
            let next_command = format!(
                "ripr agent repair --root {}{store_flag} --attempt {} --phase after{}",
                shell_arg(&bound_root(&publication.root_argument.to_string_lossy())),
                shell_arg(publication.repair_attempt_id.as_str()),
                publication.next_command_suffix.unwrap_or_default()
            );
            // Finalize-path head re-verification: the earlier pre-publication
            // gate read HEAD before the artifacts were staged; the durable
            // manifest is the authority, so HEAD is re-read immediately
            // before it is written and any move in the window aborts with the
            // typed refusal instead of publishing a mismatched attempt.
            let final_head = crate::agent::artifact::current_git_head(canonical_root)
                .map_err(|error| {
                    format!("repair attempt finalize head verification failed: {error}")
                })?;
            if let Some(expected) = publication.expected_repository_head
                && expected != final_head
            {
                return Err(format!(
                    "python repair-trust binding head moved during attempt publication; the binding pins head `{expected}` but the repository HEAD is now `{final_head}`; re-run the before phase to prepare a fresh binding"
                ));
            }
            if final_head != publication.repository_head {
                return Err(format!(
                    "repository HEAD moved during attempt publication; the attempt pins head `{}` but the repository HEAD is now `{final_head}`; re-run the before phase to prepare a fresh attempt",
                    publication.repository_head
                ));
            }
            let manifest = RepairAttemptManifest {
                schema_version: REPAIR_ATTEMPT_SCHEMA_VERSION.to_string(),
                kind: "repair_attempt".to_string(),
                repair_attempt_id: publication.repair_attempt_id,
                state: RepairAttemptState::AwaitingEdit,
                root: root_path_display(canonical_root),
                repository_head: publication.repository_head,
                producer_version: env!("CARGO_PKG_VERSION").to_string(),
                seam_id: publication.seam_id.to_string(),
                created_unix_ms: publication.created_unix_ms,
                artifacts,
                next_command,
                limitations: vec![
                    "after-phase verify and receipt outputs remain mirrored through target/ripr/workflow compatibility paths; those files are compatibility projections and not the sole surviving copy of an attempt result".to_string(),
                ],
                non_claims: vec![
                    "RIPR does not author or apply the focused test edit".to_string(),
                    "prepared evidence does not mean the gap is fixed or verified".to_string(),
                    "this manifest does not authorize mutation execution or merge".to_string(),
                ],
                after: None,
                last_after_refusal: None,
                terminal_artifacts: Vec::new(),
                store: store.manifest_identity(),
            };
            let manifest_path = write_repair_attempt_manifest(store, &manifest)?;
            Ok(BeginRepairAttemptResult {
                manifest,
                manifest_path,
            })
        });
    if result.is_err() {
        let _ = std::fs::remove_dir_all(attempt_directory);
    }
    result
}

#[cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "default-store wrappers are the test and remaining-default caller surface; CLI explicit-store uses the _from authority"
    )
)]
pub(crate) fn repair_attempt_directory(root: &Path, attempt_id: &RepairAttemptId) -> PathBuf {
    root.join(REPAIR_ATTEMPT_DIRECTORY)
        .join(attempt_id.as_str())
}

/// Loads and fully validates one durable attempt manifest by identity,
/// including its before commitment and artifact digest bindings. Consumers
/// that extend the attempt (the Python repair-trust binding) read the
/// retained provenance through this authority instead of re-parsing files.
#[cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "default-store wrappers are the test and remaining-default caller surface; CLI explicit-store uses the _from authority"
    )
)]
pub(crate) fn load_repair_attempt_manifest(
    root: &Path,
    attempt_id: &RepairAttemptId,
) -> Result<RepairAttemptManifest, String> {
    load_repair_attempt_manifest_from(root, None, attempt_id)
}

pub(crate) fn load_repair_attempt_manifest_from(
    root: &Path,
    store: Option<&Path>,
    attempt_id: &RepairAttemptId,
) -> Result<RepairAttemptManifest, String> {
    let store = resolve_store(root, store, RepairAttemptStoreAccess::Open)?;
    let (_, manifest) = load_repair_attempt_by_id(&store, attempt_id)?;
    Ok(manifest)
}

/// One entry of the read-only attempt inventory: a fully validated manifest,
/// or the directory name and the reason its manifest was refused.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum RepairAttemptInventoryEntry {
    Valid(Box<RepairAttemptManifest>),
    Invalid { directory: String, error: String },
}

/// Lists every attempt under `target/ripr/repair-attempts` without changing
/// any of them, ordered by directory name. Each manifest goes through the same
/// validation as the after phase, so a consumer never sees an attempt the
/// after phase would refuse. Ordering is by identity, not creation time:
/// consumers must not read "newest" into it (docs/REPAIR_ATTEMPT.md).
/// A missing attempts directory is an empty inventory.
pub(crate) fn inventory_repair_attempts(
    root: &Path,
) -> Result<Vec<RepairAttemptInventoryEntry>, String> {
    inventory_repair_attempts_from(root, None)
}

pub(crate) fn inventory_repair_attempts_from(
    root: &Path,
    store: Option<&Path>,
) -> Result<Vec<RepairAttemptInventoryEntry>, String> {
    let store = resolve_store(root, store, RepairAttemptStoreAccess::Open)?;
    inventory_repair_attempts_in(&store)
}

fn inventory_repair_attempts_in(
    store: &RepairAttemptStoreRef,
) -> Result<Vec<RepairAttemptInventoryEntry>, String> {
    let manifests_root = store.resolved_path();
    if store.currentness() == RepairAttemptStoreCurrentness::Missing || !manifests_root.is_dir() {
        return Ok(Vec::new());
    }
    let mut entries = Vec::new();
    for entry in std::fs::read_dir(manifests_root)
        .map_err(|error| format!("read {} failed: {error}", manifests_root.display()))?
    {
        let entry = entry.map_err(|error| format!("read repair attempt entry failed: {error}"))?;
        let path = entry.path().join(REPAIR_ATTEMPT_MANIFEST);
        if !path.is_file() {
            continue;
        }
        let directory = entry.file_name().to_string_lossy().into_owned();
        entries.push(match read_repair_attempt_manifest_at(store, &path) {
            Ok(manifest) => RepairAttemptInventoryEntry::Valid(Box::new(manifest)),
            Err(error) => RepairAttemptInventoryEntry::Invalid { directory, error },
        });
    }
    entries.sort_by(|left, right| inventory_key(left).cmp(inventory_key(right)));
    Ok(entries)
}

fn inventory_key(entry: &RepairAttemptInventoryEntry) -> &str {
    match entry {
        RepairAttemptInventoryEntry::Valid(manifest) => manifest.repair_attempt_id.as_str(),
        RepairAttemptInventoryEntry::Invalid { directory, .. } => directory,
    }
}

/// Finds one staged artifact by role, if the attempt carries it.
pub(crate) fn find_manifest_artifact_by_role<'a>(
    manifest: &'a RepairAttemptManifest,
    role: &str,
) -> Option<&'a RepairAttemptArtifact> {
    manifest
        .artifacts
        .iter()
        .find(|artifact| artifact.role == role)
}

/// Finds one terminal static-result artifact by role, if the attempt retained it.
pub(crate) fn find_terminal_artifact_by_role<'a>(
    manifest: &'a RepairAttemptManifest,
    role: &str,
) -> Option<&'a RepairAttemptArtifact> {
    manifest
        .terminal_artifacts
        .iter()
        .find(|artifact| artifact.role == role)
}

/// How status and other exact-attempt readers load the retained terminal receipt.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum AttemptTerminalReceipt {
    /// Legacy manifest: no terminal retention was declared. Callers may use a
    /// still-present exact matching compatibility receipt, but must not
    /// reconstruct an outcome from a superseded or unrelated global file.
    NotRetained,
    /// Digest-bound receipt under this attempt, matching its after verdict.
    Issued {
        path: String,
        value: serde_json::Value,
    },
    /// Terminal retention was declared but cannot be projected. Never fall
    /// back to another attempt's compatibility receipt.
    Unavailable {
        path: Option<String>,
        reason: String,
    },
}

/// Loads the attempt-local terminal receipt, validating path, digest, root,
/// and result binding. Missing, corrupt, escaped, or mis-bound files are
/// `Unavailable` — not a license to read `agent-receipt.json` for a different
/// attempt.
pub(crate) fn load_attempt_terminal_receipt(
    root: &Path,
    manifest: &RepairAttemptManifest,
) -> AttemptTerminalReceipt {
    if manifest.state != RepairAttemptState::ReadyToFinish {
        return AttemptTerminalReceipt::NotRetained;
    }
    let Some(receipt_artifact) = find_terminal_artifact_by_role(manifest, TERMINAL_RECEIPT_ROLE)
    else {
        return if manifest.terminal_artifacts.is_empty() {
            AttemptTerminalReceipt::NotRetained
        } else {
            AttemptTerminalReceipt::Unavailable {
                path: None,
                reason: "repair attempt declares terminal artifacts but no agent_receipt"
                    .to_string(),
            }
        };
    };
    // The retained verify artifact is part of the bound terminal evidence,
    // not decoration: a declared artifact that is missing or digest-mismatched
    // makes the whole terminal record unavailable, so status never reads
    // `finished_*` from a receipt whose verify half was deleted or replaced.
    // Non-legacy retention requires both roles: the before commitment
    // excludes `terminal_artifacts`, so a receipt without its verify
    // descriptor is unavailable, not issued.
    let Some(verify_artifact) = find_terminal_artifact_by_role(manifest, TERMINAL_VERIFY_ROLE)
    else {
        return AttemptTerminalReceipt::Unavailable {
            path: None,
            reason: "repair attempt declares terminal artifacts but no agent_verify".to_string(),
        };
    };
    let verify_bytes = match read_terminal_artifact_bytes(root, manifest, verify_artifact) {
        Ok(bytes) => bytes,
        Err(reason) => {
            return AttemptTerminalReceipt::Unavailable {
                path: Some(verify_artifact.path.clone()),
                reason,
            };
        }
    };
    match read_bound_terminal_receipt(root, manifest, receipt_artifact, &verify_bytes) {
        Ok((path, value)) => AttemptTerminalReceipt::Issued { path, value },
        Err(reason) => AttemptTerminalReceipt::Unavailable {
            path: Some(receipt_artifact.path.clone()),
            reason,
        },
    }
}

fn read_bound_terminal_receipt(
    root: &Path,
    manifest: &RepairAttemptManifest,
    artifact: &RepairAttemptArtifact,
    verify_bytes: &[u8],
) -> Result<(String, serde_json::Value), String> {
    let bytes = read_terminal_artifact_bytes(root, manifest, artifact)?;
    let value: serde_json::Value = serde_json::from_slice(&bytes).map_err(|error| {
        format!(
            "repair attempt terminal receipt {} is not JSON: {error}",
            artifact.path
        )
    })?;
    let after = manifest.after.as_ref().ok_or_else(|| {
        "repair attempt is ready_to_finish but carries no after verdict".to_string()
    })?;
    let bound = |pointer: &str, expected: &str| {
        value.pointer(pointer).and_then(serde_json::Value::as_str) == Some(expected)
    };
    if !(bound("/repair_attempt/attempt_id", after.attempt_id.as_str())
        && bound("/repair_attempt/after_head", &after.repository_head)
        && bound("/repair_attempt/delta_sha256", &after.delta_sha256)
        && bound("/repair_attempt/packet_sha256", &after.packet_sha256))
    {
        return Err(format!(
            "repair attempt terminal receipt {} is not bound to this attempt's after verdict",
            artifact.path
        ));
    }
    // Attempt identity alone does not bind the verdict: the receipt's
    // movement, seam change, lifecycle state, and guidance kind must equal
    // what the retained verify document records for this seam (#5256).
    validate_issued_receipt_evidence(root, manifest, &value, verify_bytes).map_err(|error| {
        format!(
            "repair attempt terminal receipt {} fails verdict binding: {error}",
            artifact.path
        )
    })?;
    // No canonical admission here: a retained pair claims a historical
    // basis, while admission re-renders from the live shared snapshots,
    // which a later attempt legitimately advances. Comparing the two
    // false-refuses an honest earlier attempt after a later finish (the
    // live basis is no longer this pair's basis). Canonical admission is
    // enforced where the pair's basis is live by construction: pending
    // promotion and the never-promoted legacy fallback. A pair admitted
    // at promotion keeps its validator-only reading here, and retained
    // storage stays trusted under the manifest digest model (#5256).
    Ok((artifact.path.clone(), value))
}

fn read_terminal_artifact_bytes(
    root: &Path,
    manifest: &RepairAttemptManifest,
    artifact: &RepairAttemptArtifact,
) -> Result<Vec<u8>, String> {
    if artifact.path.is_empty() || Path::new(&artifact.path).is_absolute() {
        return Err(format!(
            "repair attempt terminal artifact path is not a relative attempt path: {}",
            artifact.path
        ));
    }
    let locator = manifest
        .store
        .as_ref()
        .map(|identity| Path::new(identity.locator.as_str()));
    let store = resolve_store(root, locator, RepairAttemptStoreAccess::Open)?;
    store.matches_manifest(manifest.store.as_ref())?;
    let artifacts_root = store
        .attempt_directory(&manifest.repair_attempt_id)
        .join(REPAIR_ATTEMPT_ARTIFACTS_DIRECTORY);
    let artifacts_root = artifacts_root.canonicalize().map_err(|error| {
        format!(
            "canonicalize repair attempt artifacts directory {} failed: {error}",
            artifacts_root.display()
        )
    })?;
    let path = root.join(&artifact.path);
    let canonical = path.canonicalize().map_err(|error| {
        format!(
            "canonicalize repair attempt terminal artifact {} failed: {error}",
            artifact.path
        )
    })?;
    if !canonical.starts_with(&artifacts_root) {
        return Err(format!(
            "repair attempt terminal artifact escapes its attempt: {}",
            artifact.path
        ));
    }
    let bytes = std::fs::read(&canonical)
        .map_err(|error| format!("read terminal artifact {} failed: {error}", artifact.path))?;
    if u64::try_from(bytes.len()).map_err(|error| error.to_string())? != artifact.bytes
        || sha256_bytes(&bytes) != artifact.sha256
    {
        return Err(format!(
            "repair attempt terminal artifact binding failed: {}",
            artifact.path
        ));
    }
    Ok(bytes)
}

/// Copies the after-phase verify/receipt bytes under the attempt directory
/// and records them on the manifest. Files are published first; the manifest
/// reference is updated only after those bytes are durable. Existing matching
/// files are reused; a different committed payload is refused rather than
/// replaced.
#[cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "default-store wrappers are the test and remaining-default caller surface; CLI explicit-store uses the _from authority"
    )
)]
pub(crate) fn retain_terminal_evidence(
    root: &Path,
    attempt_id: &RepairAttemptId,
    sources: &[BeforeArtifactSource<'_>],
) -> Result<Vec<RepairAttemptArtifact>, String> {
    retain_terminal_evidence_from(root, None, attempt_id, sources)
}

pub(crate) fn retain_terminal_evidence_from(
    root: &Path,
    store: Option<&Path>,
    attempt_id: &RepairAttemptId,
    sources: &[BeforeArtifactSource<'_>],
) -> Result<Vec<RepairAttemptArtifact>, String> {
    if sources.is_empty() {
        return Err("terminal retention requires at least one after-phase artifact".to_string());
    }
    let (store, manifest_path, base_bytes, mut manifest) =
        open_attempt_with_bytes(root, store, attempt_id)?;
    let root = store.canonical_root().to_path_buf();
    if manifest.state != RepairAttemptState::ReadyToFinish {
        return Err(format!(
            "repair attempt {} is not ready_to_finish; terminal evidence is retained only after a compliant finish",
            attempt_id.as_str()
        ));
    }
    let destination_directory = store
        .attempt_directory(attempt_id)
        .join(REPAIR_ATTEMPT_ARTIFACTS_DIRECTORY);
    let mut retained = Vec::with_capacity(sources.len());
    let mut roles = BTreeSet::new();
    let mut names = BTreeSet::new();
    for source in sources {
        if source.role.trim().is_empty() || !roles.insert(source.role) {
            return Err(format!(
                "repair attempt terminal artifact role is blank or duplicated: `{}`",
                source.role
            ));
        }
        if source.role != TERMINAL_RECEIPT_ROLE && source.role != TERMINAL_VERIFY_ROLE {
            return Err(format!(
                "repair attempt terminal artifact role `{}` is not a retained after-phase result",
                source.role
            ));
        }
        let source_path = source.path.canonicalize().map_err(|error| {
            format!(
                "canonicalize terminal source {} failed: {error}",
                source.path.display()
            )
        })?;
        if !source_path.starts_with(&root) {
            return Err(format!(
                "repair attempt terminal artifact escapes root: {}",
                source.path.display()
            ));
        }
        let file_name = source_path
            .file_name()
            .ok_or_else(|| {
                format!(
                    "repair attempt terminal artifact has no file name: {}",
                    source_path.display()
                )
            })?
            .to_owned();
        if !names.insert(file_name.clone()) {
            return Err(format!(
                "repair attempt terminal artifact file name is duplicated: {}",
                file_name.to_string_lossy()
            ));
        }
        let bytes = std::fs::read(&source_path)
            .map_err(|error| format!("read {} failed: {error}", source_path.display()))?;
        let digest = sha256_bytes(&bytes);
        let size = u64::try_from(bytes.len()).map_err(|error| {
            format!("repair attempt terminal artifact size does not fit u64: {error}")
        })?;
        let destination = destination_directory.join(&file_name);
        if destination.exists() {
            let existing = std::fs::read(&destination)
                .map_err(|error| format!("read {} failed: {error}", destination.display()))?;
            if sha256_bytes(&existing) != digest {
                if !manifest.terminal_artifacts.is_empty() {
                    return Err(format!(
                        "repair attempt destination is immutable and already exists: {}",
                        destination.display()
                    ));
                }
                // Restore and crash-before-manifest leave unpublished leftover
                // files. They are not a committed result; retry may replace them.
                replace_file_atomically(&destination, &bytes)?;
            }
        } else {
            write_bytes_atomic(&destination, &bytes)?;
        }
        let relative = destination.strip_prefix(&root).map_err(|error| {
            format!(
                "repair attempt destination {} is not under root {}: {error}",
                destination.display(),
                root.display()
            )
        })?;
        retained.push(RepairAttemptArtifact {
            role: source.role.to_string(),
            path: display_path(relative),
            sha256: digest,
            bytes: size,
        });
    }
    retained.sort_by(|left, right| left.role.cmp(&right.role));
    if !manifest.terminal_artifacts.is_empty() && manifest.terminal_artifacts != retained {
        return Err(format!(
            "repair attempt {} already retained different terminal evidence; committed results are not replaced",
            attempt_id.as_str()
        ));
    }
    if manifest.terminal_artifacts == retained {
        return Ok(retained);
    }
    manifest.terminal_artifacts = retained.clone();
    validate_manifest(&manifest)?;
    let mut bytes = serde_json::to_vec_pretty(&manifest)
        .map_err(|error| format!("serialize repair attempt terminal retention failed: {error}"))?;
    bytes.push(b'\n');
    commit_manifest_locked(&manifest_path, attempt_id, &base_bytes, &bytes, |current| {
        if current.state != RepairAttemptState::ReadyToFinish {
            return Err(format!(
                "repair attempt {} is not ready_to_finish; terminal evidence is retained only after a compliant finish",
                attempt_id.as_str()
            ));
        }
        if !current.terminal_artifacts.is_empty() && current.terminal_artifacts != retained {
            return Err(format!(
                "repair attempt {} already retained different terminal evidence; committed results are not replaced",
                attempt_id.as_str()
            ));
        }
        Ok(())
    })?;
    read_repair_attempt_manifest_at(&store, &manifest_path)?;
    Ok(retained)
}

/// Completes terminal retention for a finished attempt whose after phase
/// wrote the compatibility receipt but did not yet record attempt-local
/// artifacts. No-op when the attempt is not finished, already retained, or
/// the compatibility projection does not qualify. Promotion validates the
/// verify document with the same canonical receipt-issuance checks the after
/// phase applies, plus the shared receipt/verdict binding, so planted files
/// are never laundered into terminal evidence (#5256). Does not rewrite
/// committed bytes.
#[cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "default-store wrappers are the test and remaining-default caller surface; CLI explicit-store uses the _from authority"
    )
)]
pub(crate) fn complete_pending_terminal_retention(
    root: &Path,
    attempt_id: &str,
) -> Result<bool, String> {
    complete_pending_terminal_retention_from(root, None, attempt_id)
}

pub(crate) fn complete_pending_terminal_retention_from(
    root: &Path,
    store: Option<&Path>,
    attempt_id: &str,
) -> Result<bool, String> {
    let attempt_id = RepairAttemptId::parse(attempt_id.to_string())?;
    let locator = store;
    let Ok((store, _, manifest)) = open_attempt(root, locator, &attempt_id) else {
        return Ok(false);
    };
    let root = store.canonical_root().to_path_buf();
    if manifest.state != RepairAttemptState::ReadyToFinish
        || find_terminal_artifact_by_role(&manifest, TERMINAL_RECEIPT_ROLE).is_some()
    {
        return Ok(false);
    }
    let receipt_path = root.join(crate::agent::loop_commands::WORKFLOW_AGENT_RECEIPT_ARTIFACT);
    let verify_path = root.join(crate::agent::loop_commands::WORKFLOW_AGENT_VERIFY_ARTIFACT);
    if !receipt_path.is_file() || !verify_path.is_file() {
        return Ok(false);
    }
    let receipt_text = std::fs::read_to_string(&receipt_path)
        .map_err(|error| format!("read {} failed: {error}", receipt_path.display()))?;
    let receipt: serde_json::Value = match serde_json::from_str(&receipt_text) {
        Ok(value) => value,
        Err(_) => return Ok(false),
    };
    let Some(after) = manifest.after.as_ref() else {
        return Ok(false);
    };
    let bound = |pointer: &str, expected: &str| {
        receipt.pointer(pointer).and_then(serde_json::Value::as_str) == Some(expected)
    };
    if !(bound("/repair_attempt/attempt_id", after.attempt_id.as_str())
        && bound("/repair_attempt/after_head", &after.repository_head)
        && bound("/repair_attempt/delta_sha256", &after.delta_sha256)
        && bound("/repair_attempt/packet_sha256", &after.packet_sha256))
    {
        return Ok(false);
    }
    let verify_bytes = std::fs::read(&verify_path)
        .map_err(|error| format!("read {} failed: {error}", verify_path.display()))?;
    let Some(expected_verify) = receipt
        .pointer("/provenance/verify_artifact/sha256")
        .and_then(serde_json::Value::as_str)
    else {
        return Ok(false);
    };
    if expected_verify != sha256_bytes(&verify_bytes) {
        return Ok(false);
    }
    // Promotion is the explicit admission step for terminal evidence: the
    // verify document must pass the same canonical validation the after
    // phase's receipt issuance applies, and the receipt's verdict must bind
    // to it. A projection that fails either check is left unpromoted — a
    // no-op here, never a laundered promotion (#5256).
    let Ok(verify_text) = std::str::from_utf8(&verify_bytes) else {
        return Ok(false);
    };
    if crate::app::agent_receipt::validate_agent_receipt_verify_json(&root, verify_text).is_err() {
        return Ok(false);
    }
    if validate_issued_receipt_evidence(&root, &manifest, &receipt, &verify_bytes).is_err() {
        return Ok(false);
    }
    retain_terminal_evidence_from(
        &root,
        locator,
        &attempt_id,
        &[
            BeforeArtifactSource {
                role: TERMINAL_RECEIPT_ROLE,
                path: &receipt_path,
            },
            BeforeArtifactSource {
                role: TERMINAL_VERIFY_ROLE,
                path: &verify_path,
            },
        ],
    )?;
    Ok(true)
}

/// Loads the retained edit-cage policy of a durable attempt from its staged
/// baseline artifact, re-verifying the artifact digest first.
pub(crate) fn load_edit_cage_policy_from(
    root: &Path,
    store: Option<&Path>,
    attempt_id: &RepairAttemptId,
) -> Result<crate::edit_cage::EditCagePolicy, String> {
    Ok(load_edit_cage_baseline_from(root, store, attempt_id)?
        .policy()
        .clone())
}

/// Loads the retained edit-cage baseline of a durable attempt from its staged
/// artifact, re-verifying the artifact digest first.
pub(crate) fn load_edit_cage_baseline_from(
    root: &Path,
    store: Option<&Path>,
    attempt_id: &RepairAttemptId,
) -> Result<crate::edit_cage::AttemptBaseline, String> {
    let (store, _, manifest) = open_attempt(root, store, attempt_id)?;
    let root = store.canonical_root();
    let artifact = find_manifest_artifact(&manifest, "edit_cage_baseline")?;
    let path = root.join(&artifact.path);
    let bytes =
        std::fs::read(&path).map_err(|error| format!("read {} failed: {error}", path.display()))?;
    if u64::try_from(bytes.len()).map_err(|error| error.to_string())? != artifact.bytes
        || sha256_bytes(&bytes) != artifact.sha256
    {
        return Err("repair attempt edit-cage baseline binding failed".to_string());
    }
    serde_json::from_slice(&bytes)
        .map_err(|error| format!("decode edit-cage baseline failed: {error}"))
}

/// Reserve the attempt transaction exclusively. Creating the directory with
/// `create_dir` (not check-then-act) fails closed when the attempt identity is
/// already taken, so an existing attempt is never reused or overwritten.
fn reserve_attempt_directory(
    store: &RepairAttemptStoreRef,
    attempt_id: &RepairAttemptId,
) -> Result<PathBuf, String> {
    let attempts_root = store.resolved_path();
    std::fs::create_dir_all(attempts_root)
        .map_err(|error| format!("create {} failed: {error}", attempts_root.display()))?;
    let attempt_directory = store.attempt_directory(attempt_id);
    std::fs::create_dir(&attempt_directory).map_err(|error| {
        if error.kind() == std::io::ErrorKind::AlreadyExists {
            return format!(
                "repair attempt directory already exists: {}",
                attempt_directory.display()
            );
        }
        format!("create {} failed: {error}", attempt_directory.display())
    })?;
    Ok(attempt_directory)
}

pub(crate) fn write_repair_attempt_manifest(
    store: &RepairAttemptStoreRef,
    manifest: &RepairAttemptManifest,
) -> Result<PathBuf, String> {
    validate_manifest(manifest)?;
    let path = store
        .attempt_directory(&manifest.repair_attempt_id)
        .join(REPAIR_ATTEMPT_MANIFEST);
    let mut rendered = serde_json::to_vec_pretty(manifest)
        .map_err(|error| format!("serialize repair attempt manifest failed: {error}"))?;
    rendered.push(b'\n');
    write_bytes_atomic(&path, &rendered)?;
    if manifest.state == RepairAttemptState::AwaitingEdit {
        let commitment_path = store
            .attempt_directory(&manifest.repair_attempt_id)
            .join(REPAIR_ATTEMPT_COMMITMENT);
        let commitment = sha256_bytes(&manifest_before_bytes(manifest)?);
        if commitment_path.exists() {
            let existing = std::fs::read_to_string(&commitment_path)
                .map_err(|error| format!("read {} failed: {error}", commitment_path.display()))?;
            if existing.trim() != commitment {
                return Err("repair attempt before commitment mismatch".to_string());
            }
        } else {
            write_bytes_atomic(&commitment_path, commitment.as_bytes())?;
        }
    }
    Ok(path)
}

fn manifest_before_bytes(manifest: &RepairAttemptManifest) -> Result<Vec<u8>, String> {
    let mut before = manifest.clone();
    before.state = RepairAttemptState::AwaitingEdit;
    before.after = None;
    before.last_after_refusal = None;
    before.terminal_artifacts.clear();
    serde_json::to_vec_pretty(&before)
        .map_err(|error| format!("serialize repair attempt commitment failed: {error}"))
}

/// Text of the test-surface requirement, matching exactly what
/// [`crate::analysis::is_test_surface_path`] accepts in this build.
/// A Rust-only binary does not compile the TypeScript conventions, so it
/// must not tell the user those paths are accepted.
fn test_surface_requirement() -> &'static str {
    #[cfg(feature = "lang-typescript")]
    {
        "a `tests` or `test` path component, or a `test_*.py`/`*_test.py`/`*_tests.py`/`*_test.rs`/`*_tests.rs` file-name convention, or a TypeScript/JavaScript test path such as `*.test.ts`, `*.spec.*`, `*.cy.*`, or `__tests__`, is required"
    }
    #[cfg(not(feature = "lang-typescript"))]
    {
        "a `tests` or `test` path component, or a `test_*.py`/`*_test.py`/`*_tests.py`/`*_test.rs`/`*_tests.rs` file-name convention, is required"
    }
}

/// Constructs the edit-cage policy from a repair packet, refusing any packet
/// whose selected edit target (the first `allowed_edit_surface` path, or the
/// `recommended_test.file` when the packet names one instead) is not a
/// recognized test surface. The positive test-surface gate runs on the raw
/// packet text BEFORE any `CagePathRule` is constructed, so a production file
/// can never become the authored edit target: a non-test selected target
/// fails closed with a named diagnostic and no cage policy exists. The
/// recognition itself is the producer-owned typed fact
/// (`analysis::workspace::is_test_surface_path`); this layer owns only the
/// policy decision and its diagnostic.
pub(crate) fn edit_cage_policy_from_packet(
    packet: &str,
    seam_id: &str,
) -> Result<EditCagePolicy, String> {
    let value: serde_json::Value = serde_json::from_str(packet)
        .map_err(|error| format!("decode repair packet for edit cage failed: {error}"))?;
    if value.get("packets").is_none()
        && value.get("seam_id").and_then(serde_json::Value::as_str) != Some(seam_id)
    {
        return Err(format!(
            "repair packet seam does not match requested seam `{seam_id}`"
        ));
    }
    // `agent packet` emits a repo packet containing a `packets` list, while
    // language adapters may emit a single packet. Select the only actionable
    // packet without treating the report envelope as edit-cage authority.
    let value = value
        .get("packets")
        .and_then(serde_json::Value::as_array)
        .map(|packets| {
            packets
                .iter()
                .filter(|packet| {
                    packet.get("seam_id").and_then(serde_json::Value::as_str) == Some(seam_id)
                })
                .collect::<Vec<_>>()
        })
        .map(|matches| {
            if matches.len() != 1 {
                return Err(format!(
                    "repair packet must contain exactly one packet for seam `{seam_id}`, found {}",
                    matches.len()
                ));
            }
            Ok(matches[0])
        })
        .transpose()?
        .unwrap_or(&value);
    let value =
        if value.get("allowed_edit_surface").is_none() && value.get("recommended_test").is_none() {
            let packets = value
                .get("packets")
                .and_then(serde_json::Value::as_array)
                .ok_or_else(|| "repair packet is missing allowed_edit_surface".to_string())?;
            packets
                .first()
                .ok_or_else(|| "repair packet contains no actionable packet".to_string())?
        } else {
            value
        };
    // Positive test-surface gate on the selected edit target, checked on the
    // raw packet text before any cage rule is constructed: path-shaped
    // allowed values can name production files (`src/...`), and the
    // denied-surface denylist only refuses generated/vendor/environment
    // prefixes, so without this gate a production file could become the
    // authored edit target of a bound attempt.
    let selected_target_text = match value.get("allowed_edit_surface") {
        Some(_) => value
            .get("allowed_edit_surface")
            .and_then(serde_json::Value::as_array)
            .and_then(|paths| paths.first())
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| "repair packet has no selected edit target".to_string())?,
        None => value
            .get("recommended_test")
            .and_then(|test| test.get("file"))
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| "repair packet is missing allowed edit target".to_string())?,
    };
    if !is_test_surface_path(selected_target_text) {
        return Err(format!(
            "repair packet selected edit target `{selected_target_text}` is not a test surface ({}); a production file is never the authored edit target and no edit cage is constructed",
            test_surface_requirement()
        ));
    }
    let paths = |name: &str| -> Result<Vec<crate::edit_cage::CagePathRule>, String> {
        let values = value
            .get(name)
            .and_then(serde_json::Value::as_array)
            .ok_or_else(|| format!("repair packet is missing {name}"))?;
        values
            .iter()
            .map(|path| {
                path.as_str()
                    .ok_or_else(|| format!("repair packet {name} contains a non-string path"))
                    .and_then(crate::edit_cage::CagePathRule::exact)
            })
            .collect()
    };
    let allowed = match value.get("allowed_edit_surface") {
        Some(surface) => {
            let entries = surface
                .as_array()
                .ok_or_else(|| "repair packet allowed_edit_surface must be an array".to_string())?;
            let mut allowed = Vec::new();
            for entry in entries {
                let path = entry.as_str().ok_or_else(|| {
                    "repair packet allowed_edit_surface contains a non-string path".to_string()
                })?;
                if !is_test_surface_path(path) {
                    return Err(format!(
                        "repair packet allowed edit surface `{path}` is not a test surface; a production file is never an allowed edit path"
                    ));
                }
                allowed.push(crate::edit_cage::CagePathRule::exact(path)?);
            }
            allowed
        }
        None => {
            let file = value
                .get("recommended_test")
                .and_then(|test| test.get("file"))
                .and_then(serde_json::Value::as_str)
                .ok_or_else(|| "repair packet is missing allowed edit target".to_string())?;
            if !is_test_surface_path(file) {
                return Err(format!(
                    "repair packet recommended test `{file}` is not a test surface"
                ));
            }
            vec![crate::edit_cage::CagePathRule::exact(file)?]
        }
    };
    let selected_target = allowed
        .first()
        .cloned()
        .ok_or_else(|| "repair packet has no selected edit target".to_string())?;
    // A Rust repair's focused test is built and run with Cargo, whose default
    // build directory is `target/` at the workspace root this attempt is bound
    // to. The documented loop runs the project tests between the phases, so
    // the git-ignored contents of that directory are expected build output,
    // not edits. The cage still observes tracked and untracked-not-ignored
    // paths there, every rule path named here (including `target/ripr`), and
    // every other ignored path. Other languages keep observing every ignored
    // path: the trust-bound Python verify phase executes tests and retains
    // its full ignored-path guard.
    let ignored_build_output =
        if Path::new(selected_target.path()).extension() == Some(std::ffi::OsStr::new("rs")) {
            Some(crate::edit_cage::CagePathRule::subtree(
                CARGO_DEFAULT_BUILD_OUTPUT_DIR,
            )?)
        } else {
            None
        };
    // The same Rust repair runs Cargo, which writes the workspace-root
    // `Cargo.lock` when the project does not commit one (the first
    // `cargo test` of a library crate). While Git does not track it, it is
    // build state, and the analysis input identity does not count it either.
    let untracked_build_lockfile = if ignored_build_output.is_some() {
        Some(crate::edit_cage::CagePathRule::exact(
            CARGO_WORKSPACE_LOCKFILE,
        )?)
    } else {
        None
    };
    Ok(EditCagePolicy {
        selected_target,
        allowed_edit_surface: allowed,
        forbidden_paths: value
            .get("forbidden_files")
            .map(|_| paths("forbidden_files"))
            .transpose()?
            .unwrap_or_default(),
        expected_operational_writes: vec![crate::edit_cage::CagePathRule::subtree("target/ripr")?],
        ignored_build_output,
        untracked_build_lockfile,
    })
}

/// Adds an explicit store outside `target/ripr` to the cage's operational
/// writes so before-phase baseline capture does not treat attempt publication
/// as an authored edit. Stores already under `target/ripr` stay covered by
/// that subtree and are not duplicated.
pub(crate) fn include_explicit_store_operational_write(
    policy: &mut EditCagePolicy,
    root: &Path,
    store: Option<&Path>,
) -> Result<(), String> {
    let Some(store) = store else {
        return Ok(());
    };
    let relative = if store.is_absolute() {
        store
            .strip_prefix(root)
            .map_err(|_prefix| {
                format!(
                    "repair attempt store {} is not contained in repository root {}",
                    display_path(store),
                    display_path(root)
                )
            })?
            .to_path_buf()
    } else {
        store.to_path_buf()
    };
    let locator = crate::edit_cage::CagePathRule::subtree(&relative)?;
    if policy
        .expected_operational_writes
        .iter()
        .any(|rule| rule.matches(locator.path()))
    {
        return Ok(());
    }
    policy.expected_operational_writes.push(locator);
    Ok(())
}

/// Captures the edit-cage baseline and writes it to a workflow compatibility
/// path. Unlike the immutable attempt destinations, this copy is refreshed on
/// every before phase (the durable authority is the baseline staged inside
/// the attempt), so an existing projection is replaced. The captured baseline
/// is dropped before the replacement so its Windows write authorities cannot
/// block the removal of the file it just probed.
pub(crate) fn write_edit_cage_baseline(
    root: &Path,
    path: &Path,
    policy: &EditCagePolicy,
) -> Result<(), String> {
    let bytes = {
        let baseline = crate::edit_cage::capture_attempt_baseline(root, policy)?;
        serde_json::to_vec_pretty(&baseline)
            .map_err(|error| format!("serialize edit-cage baseline failed: {error}"))?
    };
    if path.exists() {
        std::fs::remove_file(path)
            .map_err(|error| format!("replace {} failed: {error}", path.display()))?;
    }
    write_bytes_atomic(path, &bytes)
}

/// Resolve the durable before inputs for one after-phase invocation. Attempt ID
/// is the ordinary authority; seam selection remains a compatibility route and
/// fails closed when more than one awaiting attempt shares that seam.
#[cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "default-store wrappers are the test and remaining-default caller surface; CLI explicit-store uses the _from authority"
    )
)]
pub(crate) fn resolve_awaiting_repair_attempt(
    root: &Path,
    attempt_id: Option<&str>,
    seam_id: Option<&str>,
) -> Result<ResolvedRepairAttempt, String> {
    resolve_awaiting_repair_attempt_from(root, None, attempt_id, seam_id)
}

pub(crate) fn resolve_awaiting_repair_attempt_from(
    root: &Path,
    store: Option<&Path>,
    attempt_id: Option<&str>,
    seam_id: Option<&str>,
) -> Result<ResolvedRepairAttempt, String> {
    let root_argument = root;
    let store = resolve_store(root, store, RepairAttemptStoreAccess::Open)?;
    let canonical_root = store.canonical_root();
    let (manifest_path, manifest) = match (attempt_id, seam_id) {
        (Some(attempt_id), None) => {
            let attempt_id = RepairAttemptId::parse(attempt_id.to_string())?;
            load_repair_attempt_by_id(&store, &attempt_id)?
        }
        (None, Some(seam_id)) => select_awaiting_repair_attempt_by_seam(
            &store,
            &bound_root(&root_argument.to_string_lossy()),
            seam_id,
        )?,
        (Some(_), Some(_)) => {
            return Err(
                "repair after selection accepts either attempt ID or seam ID, not both".to_string(),
            );
        }
        (None, None) => {
            return Err("repair after selection requires an attempt ID or seam ID".to_string());
        }
    };
    if manifest.state != RepairAttemptState::AwaitingEdit {
        return Err(after_phase_not_awaiting_error(
            &bound_root(&root_argument.to_string_lossy()),
            &manifest,
        ));
    }
    let before_snapshot_path =
        canonical_root.join(&find_manifest_artifact(&manifest, "before_snapshot")?.path);
    let packet_path = canonical_root.join(&find_manifest_artifact(&manifest, "agent_packet")?.path);
    Ok(ResolvedRepairAttempt {
        attempt_id: manifest.repair_attempt_id,
        seam_id: manifest.seam_id,
        repository_head: manifest.repository_head,
        manifest_path,
        before_snapshot_path,
        packet_path,
    })
}

/// Restores a finished attempt to `awaiting_edit` so a failed apply-record
/// publication stays retryable. `finish_repair_attempt` commits the terminal
/// after state before the compatibility apply record is rendered, so a record
/// write failure would otherwise strand the attempt: the identical retry is
/// rejected (the attempt reads as already finished) and the record can never
/// be recreated. The restore is the inverse transition owned by this
/// authority: it requires the committed after state and rewrites exactly
/// `state = awaiting_edit, after = None`, which the immutable before
/// commitment re-verifies on the next load — any other drift fails closed.
#[cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "default-store wrappers are the test and remaining-default caller surface; CLI explicit-store uses the _from authority"
    )
)]
pub(crate) fn restore_repair_attempt_to_awaiting_edit(
    root: &Path,
    attempt_id: &RepairAttemptId,
) -> Result<(), String> {
    restore_repair_attempt_to_awaiting_edit_from(root, None, attempt_id)
}

pub(crate) fn restore_repair_attempt_to_awaiting_edit_from(
    root: &Path,
    store: Option<&Path>,
    attempt_id: &RepairAttemptId,
) -> Result<(), String> {
    let (_store, manifest_path, base_bytes, mut manifest) =
        open_attempt_with_bytes(root, store, attempt_id)?;
    if manifest.state == RepairAttemptState::AwaitingEdit {
        return Err(format!(
            "repair attempt {} is already awaiting_edit; no restore is needed",
            attempt_id.as_str()
        ));
    }
    if manifest.after.is_none() {
        return Err(format!(
            "repair attempt {} carries no after verdict; only a finished attempt can be restored for a retry",
            attempt_id.as_str()
        ));
    }
    manifest.state = RepairAttemptState::AwaitingEdit;
    manifest.after = None;
    // Terminal files stay on disk. They are not before-phase inputs; clearing
    // the manifest field keeps the restored awaiting_edit record from claiming
    // a finished result. A retry reuses them when the bytes still match, and
    // may replace unpublished leftovers when a later finish produces a new
    // timestamped receipt.
    manifest.terminal_artifacts.clear();
    validate_manifest(&manifest)?;
    let mut bytes = serde_json::to_vec_pretty(&manifest)
        .map_err(|error| format!("serialize restored repair attempt failed: {error}"))?;
    bytes.push(b'\n');
    commit_manifest_locked(&manifest_path, attempt_id, &base_bytes, &bytes, |current| {
        if current.state == RepairAttemptState::AwaitingEdit {
            return Err(format!(
                "repair attempt {} is already awaiting_edit; no restore is needed",
                attempt_id.as_str()
            ));
        }
        if current.after.is_none() {
            return Err(format!(
                "repair attempt {} carries no after verdict; only a finished attempt can be restored for a retry",
                attempt_id.as_str()
            ));
        }
        Ok(())
    })?;
    Ok(())
}

/// Records the after-phase verdict. `movement` states whether commits made
/// on top of the prepared head belong to the attempt: the ordinary repair
/// transaction admits them (the cage evaluates every committed path), while
/// a trust-bound attempt, whose selection pins the head, requires the exact
/// prepared head. Any other head movement records `stale`.
#[cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "default-store wrappers are the test and remaining-default caller surface; CLI explicit-store uses the _from authority"
    )
)]
pub(crate) fn finish_repair_attempt(
    root: &Path,
    attempt_id: &RepairAttemptId,
    packet_path: &Path,
    movement: HeadMovement,
) -> Result<RepairAttemptAfter, String> {
    finish_repair_attempt_from(root, None, attempt_id, packet_path, movement)
}

pub(crate) fn finish_repair_attempt_from(
    root: &Path,
    store: Option<&Path>,
    attempt_id: &RepairAttemptId,
    packet_path: &Path,
    movement: HeadMovement,
) -> Result<RepairAttemptAfter, String> {
    let root_argument = root;
    let (store, manifest_path, base_bytes, mut manifest) =
        open_attempt_with_bytes(root, store, attempt_id)?;
    let root = store.canonical_root().to_path_buf();
    if manifest.state != RepairAttemptState::AwaitingEdit {
        return Err(after_phase_not_awaiting_error(
            &bound_root(&root_argument.to_string_lossy()),
            &manifest,
        ));
    }
    let packet_artifact = find_manifest_artifact(&manifest, "agent_packet")?;
    let retained_packet_path = root.join(&packet_artifact.path);
    let supplied_packet_path = packet_path
        .canonicalize()
        .map_err(|error| format!("canonicalize repair packet failed: {error}"))?;
    let retained_packet_path = retained_packet_path
        .canonicalize()
        .map_err(|error| format!("canonicalize retained repair packet failed: {error}"))?;
    if supplied_packet_path != retained_packet_path {
        return Err(format!(
            "repair attempt {} must finish with its retained agent_packet artifact",
            attempt_id.as_str()
        ));
    }
    let packet = std::fs::read(&retained_packet_path).map_err(|error| {
        format!(
            "read repair packet {} failed: {error}",
            retained_packet_path.display()
        )
    })?;
    let packet_sha256 = sha256_bytes(&packet);
    if u64::try_from(packet.len()).map_err(|error| error.to_string())? != packet_artifact.bytes
        || packet_sha256 != packet_artifact.sha256
    {
        return Err("repair attempt packet binding failed".to_string());
    }
    let baseline_artifact = find_manifest_artifact(&manifest, "edit_cage_baseline")?;
    let baseline_path = root.join(&baseline_artifact.path);
    let baseline_bytes = std::fs::read(&baseline_path)
        .map_err(|error| format!("read {} failed: {error}", baseline_path.display()))?;
    if u64::try_from(baseline_bytes.len()).map_err(|error| error.to_string())?
        != baseline_artifact.bytes
        || sha256_bytes(&baseline_bytes) != baseline_artifact.sha256
    {
        return Err("repair attempt edit-cage baseline binding failed".to_string());
    }
    let baseline: AttemptBaseline = serde_json::from_slice(&baseline_bytes)
        .map_err(|error| format!("decode edit-cage baseline failed: {error}"))?;
    if baseline.root() != root {
        return Err("edit-cage baseline root does not match selected repository".to_string());
    }
    let current_head = crate::agent::artifact::current_git_head(&root)?;
    let (delta, mut verdict) =
        evaluate_repository_edit_cage_with_head_movement(&baseline, movement)?;
    let current = current_head == crate::agent::artifact::current_git_head(&root)?
        && (current_head == manifest.repository_head
            || (movement == HeadMovement::AdmitDescendantCommits
                && crate::agent::artifact::git_merge_base_is_ancestor(
                    &root,
                    &manifest.repository_head,
                    &current_head,
                )?));
    if !current {
        verdict.status = crate::edit_cage::EditCageVerdictStatus::Incomparable;
    }
    let delta_bytes = serde_json::to_vec(&delta)
        .map_err(|error| format!("serialize repair delta failed: {error}"))?;
    let after = RepairAttemptAfter {
        attempt_id: manifest.repair_attempt_id.clone(),
        repository_head: current_head,
        delta_sha256: sha256_bytes(&delta_bytes),
        packet_sha256,
        current,
        verdict,
    };
    manifest.after = Some(after.clone());
    // This after phase reached the durable finish, so an earlier refusal no
    // longer describes the attempt's last after phase.
    manifest.last_after_refusal = None;
    manifest.state = repair_attempt_state_for_finish(&after);
    let mut bytes = serde_json::to_vec_pretty(&manifest)
        .map_err(|error| format!("serialize completed repair attempt failed: {error}"))?;
    bytes.push(b'\n');
    let root_display = bound_root(&root_argument.to_string_lossy());
    commit_manifest_locked(&manifest_path, attempt_id, &base_bytes, &bytes, |current| {
        if current.state != RepairAttemptState::AwaitingEdit {
            return Err(after_phase_not_awaiting_error(&root_display, current));
        }
        Ok(())
    })?;
    Ok(after)
}

/// Records why the most recent after phase of an attempt refused. The attempt
/// authority owns the write: the manifest is re-validated (before commitment
/// and artifact digests) before and after the field changes, and only
/// `last_after_refusal` moves, so the refusal can never alter the attempt's
/// state, its after verdict, or its receipt binding. A later after phase that
/// reaches `finish_repair_attempt` clears it.
#[cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "default-store wrappers are the test and remaining-default caller surface; CLI explicit-store uses the _from authority"
    )
)]
pub(crate) fn record_repair_attempt_after_refusal(
    root: &Path,
    attempt_id: &RepairAttemptId,
    reason: &str,
) -> Result<(), String> {
    record_repair_attempt_after_refusal_from(root, None, attempt_id, reason)
}

pub(crate) fn record_repair_attempt_after_refusal_from(
    root: &Path,
    store: Option<&Path>,
    attempt_id: &RepairAttemptId,
    reason: &str,
) -> Result<(), String> {
    let (store, manifest_path, base_bytes, mut manifest) =
        open_attempt_with_bytes(root, store, attempt_id)?;
    let root = store.canonical_root().to_path_buf();
    let reason = bounded_refusal_reason(reason);
    if reason.is_empty() {
        return Err("an after-phase refusal needs a non-empty reason".to_string());
    }
    manifest.last_after_refusal = Some(RepairAttemptAfterRefusal {
        reason,
        repository_head: crate::agent::artifact::current_git_head(&root).ok(),
        recorded_unix_ms: current_unix_ms()?,
    });
    validate_manifest(&manifest)?;
    let mut bytes = serde_json::to_vec_pretty(&manifest)
        .map_err(|error| format!("serialize repair attempt refusal failed: {error}"))?;
    bytes.push(b'\n');
    // A refusal moves no state, but it must not land beside a committed
    // after verdict: the check runs under the commit lock against the
    // current manifest, so it refuses both an attempt that finished before
    // this write opened and one that finished while it ran. The byte
    // comparison alone cannot see the first case (base already carries the
    // verdict), and that terminal verdict supersedes the refusal observation.
    commit_manifest_locked(&manifest_path, attempt_id, &base_bytes, &bytes, |current| {
        if current.after.is_some() {
            return Err(format!(
                "repair attempt {} already has an after verdict; a refusal is recorded only while the attempt is resumable",
                attempt_id.as_str()
            ));
        }
        Ok(())
    })?;
    read_repair_attempt_manifest_at(&store, &manifest_path).map(|_| ())
}

/// Records why the receipt of an already finished attempt refused, beside the
/// committed after verdict (#5262). The after phase's trusted-surface refusal
/// fires only after `finish_repair_attempt` has committed the verdict, so the
/// resumable authority above refuses the write by design; the narration the
/// after phase printed must still reach `agent status`, which reads this
/// record for a finished attempt whose receipt is missing. The caller asserts
/// the refusal was observed after this attempt's durable finish, and the
/// locked commit keeps that assertion honest: the write is refused when the
/// attempt carries no after verdict (that observation belongs to the
/// resumable authority) and when the after verdict moved between the read and
/// the locked commit (a concurrent finish would supersede the observation
/// this refusal describes). Only `last_after_refusal` moves; state, verdict,
/// and bindings are re-validated and byte-preserved.
pub(crate) fn record_repair_attempt_terminal_receipt_refusal_from(
    root: &Path,
    store: Option<&Path>,
    attempt_id: &RepairAttemptId,
    reason: &str,
) -> Result<(), String> {
    let (store, manifest_path, base_bytes, base_manifest) =
        open_attempt_with_bytes(root, store, attempt_id)?;
    let root = store.canonical_root().to_path_buf();
    let after = base_manifest.after.clone().ok_or_else(|| {
        format!(
            "repair attempt {} has no after verdict; record the refusal with the resumable authority instead",
            attempt_id.as_str()
        )
    })?;
    let reason = bounded_refusal_reason(reason);
    if reason.is_empty() {
        return Err("an after-phase refusal needs a non-empty reason".to_string());
    }
    let mut manifest = base_manifest;
    manifest.last_after_refusal = Some(RepairAttemptAfterRefusal {
        reason,
        repository_head: crate::agent::artifact::current_git_head(&root).ok(),
        recorded_unix_ms: current_unix_ms()?,
    });
    validate_manifest(&manifest)?;
    let mut bytes = serde_json::to_vec_pretty(&manifest)
        .map_err(|error| format!("serialize repair attempt refusal failed: {error}"))?;
    bytes.push(b'\n');
    commit_manifest_locked(&manifest_path, attempt_id, &base_bytes, &bytes, |current| {
        if current.after.as_ref() != Some(&after) {
            return Err(format!(
                "repair attempt {} moved while its receipt refusal was recorded",
                attempt_id.as_str()
            ));
        }
        Ok(())
    })?;
    read_repair_attempt_manifest_at(&store, &manifest_path).map(|_| ())
}

/// Upper bound on a recorded after-phase refusal message.
const REPAIR_ATTEMPT_REFUSAL_MAX_BYTES: usize = 4096;

fn bounded_refusal_reason(reason: &str) -> String {
    let reason = reason.trim();
    if reason.len() <= REPAIR_ATTEMPT_REFUSAL_MAX_BYTES {
        return reason.to_string();
    }
    let mut end = REPAIR_ATTEMPT_REFUSAL_MAX_BYTES;
    while !reason.is_char_boundary(end) {
        end -= 1;
    }
    format!("{} [truncated]", &reason[..end])
}

/// Where the repository HEAD stands relative to an attempt's prepared head.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum AttemptHeadLineage {
    /// HEAD is the prepared head.
    Prepared,
    /// HEAD moved only forward, by commits on top of the prepared head.
    Descendant { current_head: String },
    /// HEAD moved to a commit that does not descend from the prepared head
    /// (an amend, rebase, reset, or checkout).
    Diverged { current_head: String },
}

pub(crate) fn attempt_head_lineage(
    root: &Path,
    prepared_head: &str,
) -> Result<AttemptHeadLineage, String> {
    let current_head = crate::agent::artifact::current_git_head(root)?;
    if current_head == prepared_head {
        return Ok(AttemptHeadLineage::Prepared);
    }
    if crate::agent::artifact::git_merge_base_is_ancestor(root, prepared_head, &current_head)? {
        Ok(AttemptHeadLineage::Descendant { current_head })
    } else {
        Ok(AttemptHeadLineage::Diverged { current_head })
    }
}

/// How an after phase would treat the current repository HEAD for one
/// attempt. The after phase and `ripr agent status` both read it, so status
/// resumes exactly the attempts whose after phase would evaluate the current
/// HEAD, and reports the same refusal and recovery for the rest.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum AfterPhaseHeadAdmission {
    /// The after phase evaluates the current HEAD as the attempt's with
    /// `movement`: the prepared head, or, for an ordinary attempt, a commit
    /// that descends from it (a committed focused test).
    Current { movement: HeadMovement },
    /// A trust-bound attempt whose HEAD is no longer its prepared head: the
    /// after phase still runs, but the finish records the attempt `stale`.
    FinishesStale { current_head: String },
    /// An ordinary attempt whose HEAD does not descend from its prepared head
    /// (an amend, rebase, reset, or checkout): the after phase refuses before
    /// finishing, so the attempt keeps awaiting the edit and restoring the
    /// prepared head makes it usable again.
    RefusedDiverged { current_head: String },
}

/// The head movement an attempt's after phase applies. A trust-bound attempt
/// (one that retains a Python repair-trust binding) pins its selection to the
/// exact prepared head; an ordinary attempt admits commits made on top of it.
pub(crate) fn after_phase_head_movement(manifest: &RepairAttemptManifest) -> HeadMovement {
    if find_manifest_artifact_by_role(
        manifest,
        crate::app::python_repair_binding::BINDING_ARTIFACT_ROLE,
    )
    .is_some()
    {
        HeadMovement::RequireBaselineHead
    } else {
        HeadMovement::AdmitDescendantCommits
    }
}

/// The after phase's head rule for one attempt at the current HEAD. The
/// lineage is asked of Git only for an ordinary attempt: a trust-bound
/// attempt compares the exact head.
pub(crate) fn after_phase_head_admission(
    root: &Path,
    manifest: &RepairAttemptManifest,
) -> Result<AfterPhaseHeadAdmission, String> {
    let movement = after_phase_head_movement(manifest);
    if movement == HeadMovement::RequireBaselineHead {
        let current_head = crate::agent::artifact::current_git_head(root)?;
        return Ok(if current_head == manifest.repository_head {
            AfterPhaseHeadAdmission::Current { movement }
        } else {
            AfterPhaseHeadAdmission::FinishesStale { current_head }
        });
    }
    Ok(
        match attempt_head_lineage(root, &manifest.repository_head)? {
            AttemptHeadLineage::Prepared | AttemptHeadLineage::Descendant { .. } => {
                AfterPhaseHeadAdmission::Current { movement }
            }
            AttemptHeadLineage::Diverged { current_head } => {
                AfterPhaseHeadAdmission::RefusedDiverged { current_head }
            }
        },
    )
}

/// Read-time HEAD applicability, separate from the admission recorded at finish.
/// CLI and MCP use this same reading: an awaiting ordinary attempt admits a
/// descendant, while terminal evidence applies only at its exact after HEAD.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct RepairAttemptHeadReading {
    pub(crate) evidence_head: String,
    pub(crate) head_current: Option<bool>,
    pub(crate) after_admission: Option<AfterPhaseHeadAdmission>,
}

impl RepairAttemptHeadReading {
    pub(crate) fn currentness(&self) -> &'static str {
        match self.head_current {
            Some(true) => "current",
            Some(false) => "historical",
            None => "unknown",
        }
    }
}

pub(crate) fn repair_attempt_head_reading(
    root: &Path,
    manifest: &RepairAttemptManifest,
    current_head: Option<&str>,
) -> RepairAttemptHeadReading {
    let evidence_head = manifest.after.as_ref().map_or_else(
        || manifest.repository_head.clone(),
        |after| after.repository_head.clone(),
    );
    let after_admission =
        if manifest.state == RepairAttemptState::AwaitingEdit && current_head.is_some() {
            after_phase_head_admission(root, manifest).ok()
        } else {
            None
        };
    let head_current = if manifest.state == RepairAttemptState::AwaitingEdit {
        after_admission
            .as_ref()
            .map(|admission| matches!(admission, AfterPhaseHeadAdmission::Current { .. }))
    } else {
        current_head.map(|head| head == evidence_head)
    };
    RepairAttemptHeadReading {
        evidence_head,
        head_current,
        after_admission,
    }
}

/// [`after_phase_head_admission`] for an attempt selected by identity.
pub(crate) fn after_phase_head_admission_by_id_from(
    root: &Path,
    store: Option<&Path>,
    attempt_id: &RepairAttemptId,
) -> Result<AfterPhaseHeadAdmission, String> {
    let manifest = load_repair_attempt_manifest_from(root, store, attempt_id)?;
    after_phase_head_admission(root, &manifest)
}

/// Recovery for an ordinary attempt whose HEAD no longer descends from its
/// prepared head. The after phase prints it when it refuses, and `ripr agent
/// status` repeats it for the same state, so both name the same cause and the
/// same two routes: restore the prepared head, or prepare a new attempt.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct DivergedHeadRecovery {
    /// Which HEAD moved where, and which history changes are accepted.
    pub(crate) cause: String,
    /// Restore the prepared head, then rerun the attempt's after phase.
    pub(crate) reset: String,
    /// Prepare a new attempt at the current HEAD instead.
    pub(crate) restart: String,
}

impl DivergedHeadRecovery {
    /// The after phase's narration, in order.
    pub(crate) fn lines(&self) -> Vec<String> {
        vec![
            self.cause.clone(),
            "the attempt was not finished and is still awaiting the focused test edit.".to_string(),
            self.reset.clone(),
            self.restart.clone(),
        ]
    }
}

pub(crate) fn diverged_head_recovery(
    root_display: &str,
    attempt_id: &str,
    seam_id: &str,
    prepared_head: &str,
    current_head: &str,
    store_flag: &str,
) -> DivergedHeadRecovery {
    let root_arg = shell_arg(root_display);
    let attempt_arg = shell_arg(attempt_id);
    let seam_arg = shell_arg(seam_id);
    DivergedHeadRecovery {
        cause: format!(
            "HEAD {} does not descend from {}, the head attempt `{attempt_id}` was prepared at (for example after `git commit --amend`, a rebase, a reset, or a checkout); commits made on top of that head are accepted, other history changes are not.",
            short_head(current_head),
            short_head(prepared_head),
        ),
        reset: format!(
            "to recover when only your own test commit was rewritten: `git reset --soft {prepared_head}` restores the prepared head and keeps your edit staged; then rerun `ripr agent repair --root {root_arg}{store_flag} --attempt {attempt_arg} --phase after`."
        ),
        restart: format!(
            "otherwise, prepare a new attempt at the current HEAD: set your test edit aside, run `ripr agent repair --root {root_arg}{store_flag} --seam-id {seam_arg} --phase before` while the gap still exists, restore the edit, then run the new --attempt command it prints."
        ),
    }
}

fn short_head(head: &str) -> &str {
    head.get(..12).unwrap_or(head)
}

/// The refusal an after phase gives for an attempt that is no longer awaiting
/// its edit. It names the state in its documented spelling, says whether the
/// attempt already finished, where its receipt was written, and what to run
/// next, instead of a bare state name.
fn after_phase_not_awaiting_error(root_display: &str, manifest: &RepairAttemptManifest) -> String {
    let root_arg = shell_arg(root_display);
    let attempt_id = manifest.repair_attempt_id.as_str();
    let seam_id = &manifest.seam_id;
    let store_flag = quoted_store_flag_from_identity(manifest.store.as_ref());
    let status = format!("ripr agent status --root {root_arg}{store_flag}");
    let restart = format!(
        "ripr agent repair --root {root_arg}{store_flag} --seam-id {} --phase before",
        shell_arg(seam_id)
    );
    let after_head = manifest
        .after
        .as_ref()
        .map_or("an unrecorded HEAD", |after| {
            short_head(&after.repository_head)
        });
    match manifest.state {
        RepairAttemptState::ReadyToFinish => format!(
            "repair attempt {attempt_id} (seam `{seam_id}`) already finished: its after phase ran at HEAD {after_head} and the edit cage admitted the edit (state `ready_to_finish`), so there is no after phase left to run. Its retained receipt stays under `{REPAIR_ATTEMPT_DIRECTORY}/{attempt_id}/`; `{receipt}` is only a compatibility projection of the latest finish — the workflow keeps one receipt file, and a later attempt's after phase replaces it, so verify the file's `repair_attempt.attempt_id` names this attempt before relying on it. Next: `{status}` reads the attempt's outcome; if the receipt leaves the gap open, start a new attempt with `{restart}`",
            receipt = crate::agent::loop_commands::WORKFLOW_AGENT_RECEIPT_ARTIFACT,
        ),
        RepairAttemptState::Stale
        | RepairAttemptState::Incomparable
        | RepairAttemptState::Failed => format!(
            "repair attempt {attempt_id} (seam `{seam_id}`) already ended `{state}` at its after phase (HEAD {after_head}); an ended attempt cannot run again or produce a receipt. Next: `{status}` says why it ended; while the gap is still open, start a new attempt with `{restart}`",
            state = repair_attempt_state_label(&manifest.state),
        ),
        RepairAttemptState::Prepared => format!(
            "repair attempt {attempt_id} (seam `{seam_id}`) was prepared but never published, so it has no after phase to run. Next: start a new attempt with `{restart}`"
        ),
        RepairAttemptState::AwaitingEdit => {
            format!("repair attempt {attempt_id} (seam `{seam_id}`) is awaiting its edit")
        }
    }
}

/// The documented (serialized) spelling of an attempt state.
pub(crate) fn repair_attempt_state_label(state: &RepairAttemptState) -> &'static str {
    match state {
        RepairAttemptState::Prepared => "prepared",
        RepairAttemptState::AwaitingEdit => "awaiting_edit",
        RepairAttemptState::ReadyToFinish => "ready_to_finish",
        RepairAttemptState::Stale => "stale",
        RepairAttemptState::Incomparable => "incomparable",
        RepairAttemptState::Failed => "failed",
    }
}

/// Repository paths that feed the repo-exposure analysis input identity and
/// changed since the attempt's before phase: Cargo manifests, Git-tracked
/// Cargo lockfiles, and the root `ripr.toml`. Read-only: it evaluates the
/// retained edit-cage baseline (committed changes included) without
/// finishing the attempt, so an after phase refused for incomparable
/// analysis inputs can name what moved.
pub(crate) fn analysis_input_changes_from(
    root: &Path,
    store: Option<&Path>,
    attempt_id: &RepairAttemptId,
) -> Result<Vec<String>, String> {
    let (store, _, manifest) = open_attempt(root, store, attempt_id)?;
    let root = store.canonical_root().to_path_buf();
    let baseline_artifact = find_manifest_artifact(&manifest, "edit_cage_baseline")?;
    let baseline_bytes = std::fs::read(root.join(&baseline_artifact.path))
        .map_err(|error| format!("read staged edit-cage baseline failed: {error}"))?;
    if sha256_bytes(&baseline_bytes) != baseline_artifact.sha256 {
        return Err("repair attempt edit-cage baseline binding failed".to_string());
    }
    let baseline: AttemptBaseline = serde_json::from_slice(&baseline_bytes)
        .map_err(|error| format!("decode edit-cage baseline failed: {error}"))?;
    let (_, verdict) = evaluate_repository_edit_cage_with_head_movement(
        &baseline,
        HeadMovement::AdmitDescendantCommits,
    )?;
    Ok(verdict
        .changed_paths
        .into_iter()
        .filter(|path| is_analysis_input_path(path))
        .collect())
}

fn is_analysis_input_path(path: &str) -> bool {
    path == crate::config::CONFIG_FILE_NAME
        || matches!(
            path.rsplit('/').next(),
            Some("Cargo.toml" | CARGO_WORKSPACE_LOCKFILE)
        )
}

fn select_awaiting_repair_attempt_by_seam(
    store: &RepairAttemptStoreRef,
    root_display: &str,
    seam_id: &str,
) -> Result<(PathBuf, RepairAttemptManifest), String> {
    if seam_id.trim().is_empty() {
        return Err("repair after selection requires a non-empty seam ID".to_string());
    }
    let manifests_root = store.resolved_path();
    let mut matches = Vec::new();
    // A fresh workspace has no attempts directory at all: that is zero
    // matches, not an operational error (#4332), so the refusal still names
    // the start command.
    let entries: Vec<std::fs::DirEntry> = match std::fs::read_dir(manifests_root) {
        Ok(entries) => {
            // A failed directory entry is an operational error, not a silent
            // skip: a partial listing must never masquerade as a complete one
            // and misselect the sole awaiting attempt or advise a false
            // found-zero start (review finding on #4332).
            let mut collected = Vec::new();
            for entry in entries {
                collected.push(entry.map_err(|error| {
                    format!("read {} failed: {error}", manifests_root.display())
                })?);
            }
            collected
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Vec::new(),
        Err(error) => return Err(format!("read {} failed: {error}", manifests_root.display())),
    };
    for entry in entries {
        let path = entry.path().join(REPAIR_ATTEMPT_MANIFEST);
        if !path.is_file() {
            continue;
        }
        let manifest = read_repair_attempt_manifest_at(store, &path)?;
        if manifest.seam_id == seam_id && manifest.state == RepairAttemptState::AwaitingEdit {
            matches.push((path, manifest));
        }
    }
    if matches.len() != 1 {
        // #4332: found zero has no id to pass — the working action is the
        // start command; found many names the ids so the agent can pick one
        // with `--attempt` without rerunning to discover them.
        let restart = format!(
            "ripr agent repair --root {} --seam-id {} --phase before",
            shell_arg(root_display),
            shell_arg(seam_id)
        );
        let ids = matches
            .iter()
            .map(|(_, manifest)| manifest.repair_attempt_id.as_str())
            .collect::<Vec<_>>()
            .join(", ");
        let detail = if matches.is_empty() {
            format!(
                "no awaiting repair attempt exists for seam `{seam_id}`; start one with `{restart}`"
            )
        } else {
            format!(
                "found {}: {ids}; pass --attempt <id> to select the prepared work exactly",
                matches.len()
            )
        };
        return Err(format!(
            "expected exactly one awaiting repair attempt for seam `{seam_id}`: {detail}"
        ));
    }
    matches.pop().ok_or_else(|| "missing attempt".to_string())
}

fn load_repair_attempt_by_id(
    store: &RepairAttemptStoreRef,
    attempt_id: &RepairAttemptId,
) -> Result<(PathBuf, RepairAttemptManifest), String> {
    let path = store
        .attempt_directory(attempt_id)
        .join(REPAIR_ATTEMPT_MANIFEST);
    if !path.is_file() {
        return Err(format!(
            "repair attempt manifest not found for {} at {}",
            attempt_id.as_str(),
            path.display()
        ));
    }
    let manifest = read_repair_attempt_manifest_at(store, &path)?;
    if manifest.repair_attempt_id != *attempt_id {
        return Err("repair attempt manifest identity does not match its selector".to_string());
    }
    Ok((path, manifest))
}

/// Loads the manifest and the exact raw bytes it was decoded from in one
/// read. Writers that commit through `commit_manifest_locked` must take
/// their base from this snapshot: a second read after the parse could
/// already contain a concurrent commit, and the commit check would then
/// accept bytes derived from the stale parse (#5406 review).
fn load_repair_attempt_snapshot_by_id(
    store: &RepairAttemptStoreRef,
    attempt_id: &RepairAttemptId,
) -> Result<(PathBuf, Vec<u8>, RepairAttemptManifest), String> {
    let path = store
        .attempt_directory(attempt_id)
        .join(REPAIR_ATTEMPT_MANIFEST);
    if !path.is_file() {
        return Err(format!(
            "repair attempt manifest not found for {} at {}",
            attempt_id.as_str(),
            path.display()
        ));
    }
    let raw =
        std::fs::read(&path).map_err(|error| format!("read {} failed: {error}", path.display()))?;
    let manifest = decode_and_validate_repair_attempt_manifest(store, &path, &raw)?;
    if manifest.repair_attempt_id != *attempt_id {
        return Err("repair attempt manifest identity does not match its selector".to_string());
    }
    Ok((path, raw, manifest))
}

fn read_repair_attempt_manifest_at(
    store: &RepairAttemptStoreRef,
    path: &Path,
) -> Result<RepairAttemptManifest, String> {
    let raw = std::fs::read_to_string(path)
        .map_err(|error| format!("read {} failed: {error}", path.display()))?;
    decode_and_validate_repair_attempt_manifest(store, path, raw.as_bytes())
}

fn decode_and_validate_repair_attempt_manifest(
    store: &RepairAttemptStoreRef,
    path: &Path,
    raw: &[u8],
) -> Result<RepairAttemptManifest, String> {
    let manifest: RepairAttemptManifest = serde_json::from_slice(raw)
        .map_err(|error| format!("decode {} failed: {error}", path.display()))?;
    validate_manifest_at(store, path, &manifest)?;
    Ok(manifest)
}

fn find_manifest_artifact<'a>(
    manifest: &'a RepairAttemptManifest,
    role: &str,
) -> Result<&'a RepairAttemptArtifact, String> {
    manifest
        .artifacts
        .iter()
        .find(|artifact| artifact.role == role)
        .ok_or_else(|| format!("repair attempt is missing {role} artifact"))
}

fn replace_manifest_bytes(path: &Path, bytes: &[u8]) -> Result<(), String> {
    replace_file_atomically(path, bytes)
}

/// Name of the attempt-directory file whose OS advisory lock serializes
/// after-phase manifest commits (#5287). The file carries no data; its lock
/// is the mutual exclusion. The OS releases the lock on process exit or
/// crash, so a dead holder never wedges the attempt.
const REPAIR_ATTEMPT_COMMIT_LOCK: &str = "after.lock";

/// Win32 `ERROR_LOCK_VIOLATION`, returned by `LockFileEx` with
/// `LOCKFILE_FAIL_IMMEDIATE` when another handle holds the range. fs2
/// surfaces it unmapped, so contention detection matches it alongside
/// `ErrorKind::WouldBlock` (the Unix `EWOULDBLOCK` path).
const WINDOWS_ERROR_LOCK_VIOLATION: i32 = 33;

/// Fixed tail of the commit-contention refusal: another after phase holds
/// the attempt lock, so this phase must retry after it completes.
pub(crate) const REPAIR_ATTEMPT_CONTENTION_TAIL: &str =
    "is being finished by another process; retry after it completes";

/// Fixed tail of the changed-underfoot refusal: a concurrent writer
/// committed between this phase's snapshot and its commit.
pub(crate) const REPAIR_ATTEMPT_CHANGED_UNDERFOOT_TAIL: &str =
    "changed while the after phase ran; retry the after phase";

/// True when a manifest-commit error is a retryable named refusal (lock
/// contention or a concurrent commit underfoot) rather than an operational
/// failure. The matched spans are fixed product sentences carrying no
/// interpolated user input, so a user-supplied path or id cannot forge
/// them; the CLI boundary maps these to the typed-refusal exit code 3.
pub(crate) fn repair_attempt_commit_error_is_retry_refusal(error: &str) -> bool {
    error.contains(REPAIR_ATTEMPT_CONTENTION_TAIL)
        || error.contains(REPAIR_ATTEMPT_CHANGED_UNDERFOOT_TAIL)
}

/// Short-held exclusive OS lock over one manifest commit. Released on drop.
struct ManifestCommitLock {
    file: std::fs::File,
}

impl Drop for ManifestCommitLock {
    fn drop(&mut self) {
        let _ = fs2::FileExt::unlock(&self.file);
    }
}

fn lock_manifest_for_commit(
    attempt_dir: &Path,
    attempt_id: &RepairAttemptId,
) -> Result<ManifestCommitLock, String> {
    let lock_path = attempt_dir.join(REPAIR_ATTEMPT_COMMIT_LOCK);
    let file = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(&lock_path)
        .map_err(|error| format!("open {} failed: {error}", lock_path.display()))?;
    if let Err(error) = fs2::FileExt::try_lock_exclusive(&file) {
        if error.kind() == std::io::ErrorKind::WouldBlock
            || error.raw_os_error() == Some(WINDOWS_ERROR_LOCK_VIOLATION)
        {
            return Err(format!(
                "repair attempt {} {REPAIR_ATTEMPT_CONTENTION_TAIL}",
                attempt_id.as_str()
            ));
        }
        return Err(format!("lock {} failed: {error}", lock_path.display()));
    }
    Ok(ManifestCommitLock { file })
}

/// Commits replacement manifest bytes only when the attempt is unchanged
/// since the caller loaded `base_bytes`: the commit lock is held across a
/// re-read, the caller's state check against the fresh manifest, and the
/// replace, so two concurrent after phases cannot both derive from one base
/// and lose a verdict (#5287). The state check runs before the byte
/// comparison so a loser behind a finished winner gets the informative
/// already-ended refusal rather than the changed-underfoot one.
fn commit_manifest_locked(
    manifest_path: &Path,
    attempt_id: &RepairAttemptId,
    base_bytes: &[u8],
    bytes: &[u8],
    check: impl FnOnce(&RepairAttemptManifest) -> Result<(), String>,
) -> Result<(), String> {
    let attempt_dir = manifest_path
        .parent()
        .ok_or_else(|| "repair attempt manifest path has no parent".to_string())?;
    let _lock = lock_manifest_for_commit(attempt_dir, attempt_id)?;
    let current_bytes = std::fs::read(manifest_path)
        .map_err(|error| format!("read {} failed: {error}", manifest_path.display()))?;
    let current: RepairAttemptManifest = serde_json::from_slice(&current_bytes)
        .map_err(|error| format!("decode {} failed: {error}", manifest_path.display()))?;
    check(&current)?;
    if current_bytes != base_bytes {
        return Err(format!(
            "repair attempt {} {REPAIR_ATTEMPT_CHANGED_UNDERFOOT_TAIL}",
            attempt_id.as_str()
        ));
    }
    replace_manifest_bytes(manifest_path, bytes)?;
    Ok(())
}

/// Replaces a shared compatibility file through the staged atomic-rename
/// pattern: the bytes are written and synced to a temporary file, then moved
/// onto the destination. A concurrent reader never observes partial bytes and
/// never observes the destination missing: `std::fs::rename` replaces an
/// existing destination on every supported platform (POSIX semantics on Unix;
/// `MoveFileExW`/POSIX-rename semantics on Windows), so no delete step runs
/// before the rename — a failed replacement leaves the previous content in
/// place. Unlike the exclusive `write_bytes_atomic` this replaces an existing
/// destination, so it is only for files that are refreshed in place (attempt
/// destinations stay immutable).
pub(crate) fn replace_file_atomically(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let nonce = ATTEMPT_NONCE.fetch_add(1, Ordering::Relaxed);
    let temporary = path.with_extension(format!("tmp-{}-{nonce}", std::process::id()));
    let write_result = (|| -> Result<(), String> {
        // An interrupted run can leave this name behind (pids and the counter
        // repeat across processes); removing a planted link removes only it.
        let _ = std::fs::remove_file(&temporary);
        let mut file = crate::output::file_write::create_exclusive(&temporary)
            .map_err(|error| format!("create {} failed: {error}", temporary.display()))?;
        file.write_all(bytes)
            .map_err(|error| format!("write {} failed: {error}", temporary.display()))?;
        file.sync_all()
            .map_err(|error| format!("sync {} failed: {error}", temporary.display()))?;
        Ok(())
    })();
    if let Err(error) = write_result {
        let _ = std::fs::remove_file(&temporary);
        return Err(error);
    }
    if let Err(error) = std::fs::rename(&temporary, path) {
        let _ = std::fs::remove_file(&temporary);
        return Err(format!("replace {} failed: {error}", path.display()));
    }
    Ok(())
}

fn stage_before_artifacts(
    root: &Path,
    attempt_directory: &Path,
    sources: &[BeforeArtifactSource<'_>],
) -> Result<Vec<RepairAttemptArtifact>, String> {
    let destination_directory = attempt_directory.join(REPAIR_ATTEMPT_ARTIFACTS_DIRECTORY);
    let nonce = ATTEMPT_NONCE.fetch_add(1, Ordering::Relaxed);
    let staging_directory = attempt_directory.join(format!(
        ".{REPAIR_ATTEMPT_ARTIFACTS_DIRECTORY}.tmp-{}-{nonce}",
        std::process::id()
    ));
    let artifacts = match stage_sources(root, &staging_directory, &destination_directory, sources) {
        Ok(artifacts) => artifacts,
        Err(error) => {
            let _ = std::fs::remove_dir_all(&staging_directory);
            return Err(error);
        }
    };
    if let Err(error) = std::fs::rename(&staging_directory, &destination_directory) {
        let _ = std::fs::remove_dir_all(&staging_directory);
        return Err(format!(
            "publish {} failed: {error}",
            destination_directory.display()
        ));
    }
    Ok(artifacts)
}

fn stage_sources(
    root: &Path,
    staging_directory: &Path,
    destination_directory: &Path,
    sources: &[BeforeArtifactSource<'_>],
) -> Result<Vec<RepairAttemptArtifact>, String> {
    let mut roles = BTreeSet::new();
    let mut names = BTreeSet::new();
    let mut artifacts = Vec::with_capacity(sources.len());

    for source in sources {
        if source.role.trim().is_empty() || !roles.insert(source.role) {
            return Err(format!(
                "repair attempt artifact role is blank or duplicated: `{}`",
                source.role
            ));
        }
        let source_path = source
            .path
            .canonicalize()
            .map_err(|error| format!("canonicalize {} failed: {error}", source.path.display()))?;
        if !source_path.starts_with(root) {
            return Err(format!(
                "repair attempt artifact escapes root: {}",
                source.path.display()
            ));
        }
        let file_name = source_path
            .file_name()
            .ok_or_else(|| {
                format!(
                    "repair attempt artifact has no file name: {}",
                    source_path.display()
                )
            })?
            .to_owned();
        if !names.insert(file_name.clone()) {
            return Err(format!(
                "repair attempt artifact file name is duplicated: {}",
                file_name.to_string_lossy()
            ));
        }
        let bytes = std::fs::read(&source_path)
            .map_err(|error| format!("read {} failed: {error}", source_path.display()))?;
        let staged = staging_directory.join(&file_name);
        write_bytes_atomic(&staged, &bytes)?;
        let destination = destination_directory.join(&file_name);
        let relative = destination.strip_prefix(root).map_err(|error| {
            format!(
                "repair attempt destination {} is not under root {}: {error}",
                destination.display(),
                root.display()
            )
        })?;
        artifacts.push(RepairAttemptArtifact {
            role: source.role.to_string(),
            path: display_path(relative),
            sha256: sha256_bytes(&bytes),
            bytes: u64::try_from(bytes.len()).map_err(|error| {
                format!("repair attempt artifact size does not fit u64: {error}")
            })?,
        });
    }

    artifacts.sort_by(|left, right| left.role.cmp(&right.role));
    Ok(artifacts)
}

fn validate_manifest(manifest: &RepairAttemptManifest) -> Result<(), String> {
    if manifest.schema_version != REPAIR_ATTEMPT_SCHEMA_VERSION {
        return Err(format!(
            "repair attempt schema must be {REPAIR_ATTEMPT_SCHEMA_VERSION}, got {}",
            manifest.schema_version
        ));
    }
    RepairAttemptId::parse(manifest.repair_attempt_id.as_str())?;
    if manifest.kind != "repair_attempt"
        || manifest.root.is_empty()
        || manifest.repository_head.len() != 40
        || !manifest
            .repository_head
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit())
        || manifest.producer_version.is_empty()
        || manifest.seam_id.is_empty()
        || manifest.artifacts.is_empty()
        || manifest.next_command.is_empty()
        || manifest
            .limitations
            .iter()
            .any(|limitation| limitation.is_empty())
        || manifest.non_claims.is_empty()
        || manifest
            .non_claims
            .iter()
            .any(|non_claim| non_claim.is_empty())
    {
        return Err("repair attempt manifest is incomplete or malformed".to_string());
    }
    if let Some(store) = &manifest.store {
        if store.schema_version != store::REPAIR_ATTEMPT_STORE_SCHEMA_VERSION {
            return Err(format!(
                "repair attempt store schema must be {}, got {}",
                store::REPAIR_ATTEMPT_STORE_SCHEMA_VERSION,
                store.schema_version
            ));
        }
        if store.locator.trim().is_empty() {
            return Err("repair attempt store locator is empty".to_string());
        }
    }
    let has_after = manifest.after.is_some();
    let state_requires_after = matches!(
        manifest.state,
        RepairAttemptState::ReadyToFinish
            | RepairAttemptState::Stale
            | RepairAttemptState::Incomparable
            | RepairAttemptState::Failed
    );
    if has_after != state_requires_after {
        return Err("repair attempt state/after boundary is inconsistent".to_string());
    }
    if manifest
        .last_after_refusal
        .as_ref()
        .is_some_and(|refusal| refusal.reason.trim().is_empty())
    {
        return Err("repair attempt after-phase refusal has an empty reason".to_string());
    }
    if manifest.artifacts.iter().any(|artifact| {
        artifact.role.is_empty() || artifact.path.is_empty() || !is_sha256_digest(&artifact.sha256)
    }) {
        return Err("repair attempt manifest contains an invalid artifact".to_string());
    }
    let mut roles = BTreeSet::new();
    let mut paths = BTreeSet::new();
    if manifest
        .artifacts
        .iter()
        .any(|artifact| !roles.insert(&artifact.role) || !paths.insert(&artifact.path))
    {
        return Err("repair attempt manifest contains duplicate artifact identity".to_string());
    }
    if manifest.terminal_artifacts.iter().any(|artifact| {
        artifact.role.is_empty() || artifact.path.is_empty() || !is_sha256_digest(&artifact.sha256)
    }) {
        return Err("repair attempt manifest contains an invalid terminal artifact".to_string());
    }
    let mut terminal_roles = BTreeSet::new();
    let mut terminal_paths = BTreeSet::new();
    if manifest.terminal_artifacts.iter().any(|artifact| {
        !terminal_roles.insert(&artifact.role)
            || !terminal_paths.insert(&artifact.path)
            || paths.contains(&artifact.path)
    }) {
        return Err(
            "repair attempt manifest contains duplicate or colliding terminal artifact identity"
                .to_string(),
        );
    }
    Ok(())
}

fn validate_manifest_at(
    store: &RepairAttemptStoreRef,
    manifest_path: &Path,
    manifest: &RepairAttemptManifest,
) -> Result<(), String> {
    validate_manifest(manifest)?;
    store.matches_manifest(manifest.store.as_ref())?;
    let root = store.canonical_root();
    let declared_root = PathBuf::from(&manifest.root)
        .canonicalize()
        .map_err(|error| format!("canonicalize declared manifest root failed: {error}"))?;
    if declared_root != root {
        return Err("repair attempt manifest root does not match selected repository".to_string());
    }
    let expected_manifest = store
        .attempt_directory(&manifest.repair_attempt_id)
        .canonicalize()
        .map_err(|error| format!("canonicalize expected attempt directory failed: {error}"))?
        .join(REPAIR_ATTEMPT_MANIFEST);
    if manifest_path
        .canonicalize()
        .map_err(|error| format!("canonicalize manifest path failed: {error}"))?
        != expected_manifest
    {
        return Err("repair attempt manifest path is not bound to its identity".to_string());
    }
    let commitment_path = store
        .attempt_directory(&manifest.repair_attempt_id)
        .join(REPAIR_ATTEMPT_COMMITMENT);
    let commitment = std::fs::read_to_string(&commitment_path)
        .map_err(|error| format!("read before commitment failed: {error}"))?;
    if commitment.trim() != sha256_bytes(&manifest_before_bytes(manifest)?) {
        return Err("repair attempt before commitment failed".to_string());
    }
    let artifacts_root = store
        .attempt_directory(&manifest.repair_attempt_id)
        .join(REPAIR_ATTEMPT_ARTIFACTS_DIRECTORY);
    for artifact in &manifest.artifacts {
        let path = root.join(&artifact.path);
        let canonical = path
            .canonicalize()
            .map_err(|error| format!("canonicalize artifact {} failed: {error}", artifact.path))?;
        if !canonical.starts_with(&artifacts_root) {
            return Err(format!(
                "repair attempt artifact escapes its attempt: {}",
                artifact.path
            ));
        }
        let bytes = std::fs::read(&canonical)
            .map_err(|error| format!("read artifact {} failed: {error}", artifact.path))?;
        if u64::try_from(bytes.len()).map_err(|error| error.to_string())? != artifact.bytes
            || sha256_bytes(&bytes) != artifact.sha256
        {
            return Err(format!(
                "repair attempt artifact binding failed: {}",
                artifact.path
            ));
        }
    }
    Ok(())
}

/// Refuses receipt admission when the repository differs from the trusted
/// surface.
///
/// Tracked content is compared in full with the head the attempt was
/// prepared at (`before_head`): that covers uncommitted edits and every commit
/// made on top of it, so any tracked difference outside the trusted surface,
/// pre-existing or not, committed or not, blocks admission. An untracked path belongs to no
/// commit; it blocks admission only when the attempt observably wrote it,
/// which is exactly the edit cage's baseline-relative delta
/// (`observed_changes`, built from exact content digests of untracked files).
/// A pre-existing untracked file whose bytes are unchanged since the before
/// phase (for example a `Cargo.lock` an earlier build generated) is not an
/// attempt write, and the cage's compliant verdict already reported it as
/// unchanged, so refusing it here would contradict the cage's own delta.
fn validate_trusted_head_surface(
    root: &Path,
    policy: &EditCagePolicy,
    observed_changes: &[String],
    before_head: &str,
) -> Result<(), String> {
    match trusted_head_surface_violations(root, policy, observed_changes, before_head)?.as_slice() {
        [] => Ok(()),
        [path, ..] => Err(format!(
            "repair receipt observed repository path {TRUSTED_SURFACE_REFUSAL_TAIL}: {path}"
        )),
    }
}

/// The stable tail of the receipt's trusted-surface admission refusal. The
/// after phase matches it to narrate the commit-first recovery when the
/// refusal still fires mid-loop, so the shared vocabulary cannot drift
/// between the refusing validator and its narration (#5262).
pub(crate) const TRUSTED_SURFACE_REFUSAL_TAIL: &str = "outside trusted edit surface";

/// The repository paths a receipt would refuse for this head: tracked paths
/// that differ from `head` and untracked paths the attempt observably wrote,
/// filtered to the paths outside the attempt's allowed edit surface. An empty
/// list means the surface is trusted; the list is evidence for the caller's
/// refusal, not a failure by itself. The receipt admits only when the list is
/// empty, and the before phase runs the same inventory over the freshly read
/// HEAD to refuse a dirty production premise before any attempt exists
/// (#5262), where a dirty allowed-surface test file stays allowed because the
/// loop expects the focused test edit.
pub(crate) fn trusted_head_surface_violations(
    root: &Path,
    policy: &EditCagePolicy,
    observed_changes: &[String],
    head: &str,
) -> Result<Vec<String>, String> {
    let tracked = git_paths(
        root,
        &[
            "diff",
            "--no-renames",
            "--no-ext-diff",
            "--name-only",
            "-z",
            head,
            "--",
        ],
    )?;
    let untracked = git_paths(root, &["ls-files", "--others", "--exclude-standard", "-z"])?;
    let observed = observed_changes
        .iter()
        .map(String::as_str)
        .collect::<BTreeSet<_>>();
    let written_untracked = untracked
        .into_iter()
        .filter(|path| observed.contains(path.as_str()));
    Ok(tracked
        .into_iter()
        .chain(written_untracked)
        .filter(|path| !policy.allows_path(path))
        .collect())
}

/// Cooperative deadline for the trusted-surface git inventory (#2303, #4363).
/// Receipt admission is a bounded repair flow: a hung git must not block it
/// past the deadline. `diff --name-only` and `ls-files` are near-instant on
/// any real repository; one minute matches the `GIT_DEADLINE` family used by
/// the other bounded git consumers.
const GIT_PATHS_DEADLINE: Duration = Duration::from_mins(1);

fn git_paths(root: &Path, args: &[&str]) -> Result<Vec<String>, String> {
    git_paths_with_deadline(root, args, Some(GIT_PATHS_DEADLINE))
}

/// The one-parameter production wrapper binds the fixed one-minute ceiling;
/// tests inject a deadline through `git_paths_with_deadline` to prove the
/// caller-supplied bound is plumbed into the shared authority rather than
/// dropped on the way (#4363 review).
fn git_paths_with_deadline(
    root: &Path,
    args: &[&str],
    deadline: Option<Duration>,
) -> Result<Vec<String>, String> {
    // Callers pass `-z` output, which is never C-quoted; decoding rules
    // come from the shared NUL path-record authority (#4006). Strict:
    // non-UTF-8 or empty records fail loudly instead of collapsing through
    // lossy conversion, which refuses admission in the trusted-surface
    // validator rather than admitting a rewritten path.
    let output = crate::git::run_git_output_with_deadline(root, args, deadline)
        .map_err(|error| format!("run git {} failed: {error}", args.join(" ")))?;
    if !output.status.success() {
        return Err(format!(
            "git {} failed: {}",
            args.join(" "),
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    decode_git_paths(&output.stdout, &args.join(" "))
}

/// Decode raw `-z` path-inventory bytes through the shared NUL path-record
/// authority (#4006). Strict: non-UTF-8 or empty records fail loudly.
fn decode_git_paths(output: &[u8], argv: &str) -> Result<Vec<String>, String> {
    crate::analysis::parse_git_path_records(output)
        .map_err(|err| format!("git {argv} path inventory: {err}"))
        .and_then(|paths| {
            paths
                .iter()
                .map(|path| {
                    path.to_str().map(str::to_string).ok_or_else(|| {
                        format!(
                            "git {argv} path inventory: decoded path {} is not valid UTF-8",
                            path.display()
                        )
                    })
                })
                .collect()
        })
}

/// Schema 0.1 artifact digest shape: `sha256:` plus 64 lowercase hex digits.
fn is_sha256_digest(value: &str) -> bool {
    let Some(digest) = value.strip_prefix("sha256:") else {
        return false;
    };
    digest.len() == 64
        && digest
            .bytes()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
}

fn repair_attempt_id_from_parts(
    root: &str,
    seam_id: &str,
    repository_head: &str,
    created_unix_ms: u64,
    process_id: u32,
    nonce: u64,
) -> Result<RepairAttemptId, String> {
    let mut hasher = Sha256::new();
    for value in [root, seam_id, repository_head] {
        hasher.update(value.as_bytes());
        hasher.update([0]);
    }
    hasher.update(created_unix_ms.to_le_bytes());
    hasher.update(process_id.to_le_bytes());
    hasher.update(nonce.to_le_bytes());
    let digest = hasher.finalize();
    let mut suffix = String::with_capacity(REPAIR_ATTEMPT_ID_HEX_LEN);
    for byte in digest.iter().take(REPAIR_ATTEMPT_ID_HEX_LEN / 2) {
        suffix.push_str(&format!("{byte:02x}"));
    }
    RepairAttemptId::parse(format!("{REPAIR_ATTEMPT_ID_PREFIX}{suffix}"))
}

fn current_unix_ms() -> Result<u64, String> {
    let duration = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| format!("system clock is before the Unix epoch: {error}"))?;
    u64::try_from(duration.as_millis())
        .map_err(|error| format!("current Unix timestamp does not fit u64: {error}"))
}

fn sha256_bytes(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let mut rendered = String::from("sha256:");
    for byte in digest {
        rendered.push_str(&format!("{byte:02x}"));
    }
    rendered
}

fn write_bytes_atomic(path: &Path, bytes: &[u8]) -> Result<(), String> {
    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        std::fs::create_dir_all(parent)
            .map_err(|error| format!("create {} failed: {error}", parent.display()))?;
    }
    let nonce = ATTEMPT_NONCE.fetch_add(1, Ordering::Relaxed);
    let temporary = path.with_extension(format!("tmp-{}-{nonce}", std::process::id()));
    let write_result = (|| -> Result<(), String> {
        // An interrupted run can leave this name behind (pids and the counter
        // repeat across processes); removing a planted link removes only it.
        let _ = std::fs::remove_file(&temporary);
        let mut file = crate::output::file_write::create_exclusive(&temporary)
            .map_err(|error| format!("create {} failed: {error}", temporary.display()))?;
        file.write_all(bytes)
            .map_err(|error| format!("write {} failed: {error}", temporary.display()))?;
        file.sync_all()
            .map_err(|error| format!("sync {} failed: {error}", temporary.display()))?;
        Ok(())
    })();
    if let Err(error) = write_result {
        let _ = std::fs::remove_file(&temporary);
        return Err(error);
    }

    // Publish exclusively: attempt destinations are immutable, so linking the
    // temporary file into place fails closed instead of deleting or
    // overwriting an existing destination (which also removes the Windows
    // delete-before-rename crash gap).
    if let Err(error) = std::fs::hard_link(&temporary, path) {
        let _ = std::fs::remove_file(&temporary);
        if error.kind() == std::io::ErrorKind::AlreadyExists {
            return Err(format!(
                "repair attempt destination is immutable and already exists: {}",
                path.display()
            ));
        }
        return Err(format!("publish {} failed: {error}", path.display()));
    }
    let _ = std::fs::remove_file(&temporary);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::fixture_git::fixture_git_ok as run_git;
    use crate::testing::verify_fixture::{mint_bound_receipt, mint_bound_receipt_pair};

    fn test_root(label: &str) -> Result<PathBuf, String> {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|error| format!("test clock failed: {error}"))?
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "ripr-repair-attempt-{label}-{}-{stamp}",
            std::process::id()
        ));
        std::fs::create_dir_all(&root)
            .map_err(|error| format!("create {} failed: {error}", root.display()))?;
        root.canonicalize()
            .map_err(|error| format!("canonicalize {} failed: {error}", root.display()))
    }

    fn sample_manifest(root: &Path) -> Result<RepairAttemptManifest, String> {
        let attempt_id = repair_attempt_id_from_parts(
            &display_path(root),
            "seam:sample",
            "0123456789abcdef0123456789abcdef01234567",
            1,
            2,
            3,
        )?;
        let next_command = format!(
            "ripr agent repair --root . --attempt {} --phase after",
            attempt_id.as_str()
        );
        Ok(RepairAttemptManifest {
            schema_version: REPAIR_ATTEMPT_SCHEMA_VERSION.to_string(),
            kind: "repair_attempt".to_string(),
            repair_attempt_id: attempt_id,
            state: RepairAttemptState::AwaitingEdit,
            root: display_path(root),
            repository_head: "0123456789abcdef0123456789abcdef01234567".to_string(),
            producer_version: env!("CARGO_PKG_VERSION").to_string(),
            seam_id: "seam:sample".to_string(),
            created_unix_ms: 1,
            artifacts: vec![RepairAttemptArtifact {
                role: "before_snapshot".to_string(),
                path: "target/ripr/repair-attempts/sample/artifacts/before.json".to_string(),
                sha256: "sha256:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"
                    .to_string(),
                bytes: 2,
            }],
            next_command,
            limitations: Vec::new(),
            non_claims: vec!["not merge authority".to_string()],
            after: None,
            last_after_refusal: None,
            terminal_artifacts: Vec::new(),
            store: None,
        })
    }

    fn prepared_store(root: &Path) -> Result<RepairAttemptStoreRef, String> {
        resolve_store(root, None, RepairAttemptStoreAccess::Prepare)
    }

    #[test]
    fn repair_attempt_id_has_a_closed_shape() -> Result<(), String> {
        let id = repair_attempt_id_from_parts(
            "/repo",
            "seam:sample",
            "0123456789abcdef0123456789abcdef01234567",
            1,
            2,
            3,
        )?;
        if id.as_str().len() != REPAIR_ATTEMPT_ID_PREFIX.len() + REPAIR_ATTEMPT_ID_HEX_LEN {
            return Err(format!("unexpected repair attempt ID: {}", id.as_str()));
        }
        if RepairAttemptId::parse("attempt-not-valid").is_ok() {
            return Err("invalid repair attempt ID was accepted".to_string());
        }
        Ok(())
    }

    #[test]
    fn packet_policy_selects_exact_seam_and_rejects_wrong_selection() -> Result<(), String> {
        let packet = serde_json::json!({
            "packets": [
                {"seam_id": "other", "allowed_edit_surface": ["tests/other.rs"], "forbidden_files": []},
                {"seam_id": "seam:sample", "allowed_edit_surface": ["tests/target.rs"], "forbidden_files": []}
            ]
        });
        let rendered = serde_json::to_string(&packet).map_err(|error| error.to_string())?;
        let selected = edit_cage_policy_from_packet(&rendered, "seam:sample")?;
        let selected = serde_json::to_value(selected).map_err(|error| error.to_string())?;
        if !selected.to_string().contains("tests/target.rs") {
            return Err("packet policy selected the wrong seam".to_string());
        }
        if edit_cage_policy_from_packet(&rendered, "missing-seam").is_ok() {
            return Err("packet policy accepted a missing seam".to_string());
        }
        Ok(())
    }

    #[test]
    fn cage_policy_refuses_a_non_test_selected_edit_target_before_any_cage() -> Result<(), String> {
        // A gap route's path-shaped `target_file` (`src/...`) can arrive as
        // the packet's allowed surface; the positive test-surface gate must
        // refuse it with a named diagnostic before any cage policy exists,
        // so a production file can never become the authored edit target.
        let packet = serde_json::json!({
            "seam_id": "seam:sample",
            "allowed_edit_surface": ["src/production.rs"],
            "forbidden_files": []
        });
        let rendered = serde_json::to_string(&packet).map_err(|error| error.to_string())?;
        let error = match edit_cage_policy_from_packet(&rendered, "seam:sample") {
            Err(error) => error,
            Ok(_) => {
                return Err(
                    "a production selected edit target constructed a cage policy".to_string(),
                );
            }
        };
        for needle in [
            "src/production.rs",
            "is not a test surface",
            "no edit cage is constructed",
        ] {
            if !error.contains(needle) {
                return Err(format!("refusal did not name `{needle}`: {error}"));
            }
        }
        // The recommended-test construction route is gated identically.
        let packet = serde_json::json!({
            "seam_id": "seam:sample",
            "recommended_test": { "file": "src/production.rs" }
        });
        let rendered = serde_json::to_string(&packet).map_err(|error| error.to_string())?;
        if edit_cage_policy_from_packet(&rendered, "seam:sample").is_ok() {
            return Err("a production recommended test constructed a cage policy".to_string());
        }
        Ok(())
    }

    #[test]
    fn cage_policy_proceeds_for_test_surface_selected_edit_targets() -> Result<(), String> {
        // A focused test file under `tests/` proceeds, and so does a
        // test-surface helper: the `tests` path component is the positive
        // signal, independent of the file-name convention. TypeScript
        // conventions are accepted only when that adapter is compiled.
        let mut targets = vec!["tests/pricing.rs", "tests/helpers/mod.rs", "test/smoke.py"];
        #[cfg(feature = "lang-typescript")]
        {
            targets.push("src/cart.test.ts");
            targets.push("src/__tests__/Header.tsx");
        }
        for target in targets {
            let packet = serde_json::json!({
                "seam_id": "seam:sample",
                "allowed_edit_surface": [target],
                "forbidden_files": []
            });
            let rendered = serde_json::to_string(&packet).map_err(|error| error.to_string())?;
            let policy = edit_cage_policy_from_packet(&rendered, "seam:sample")
                .map_err(|error| format!("test target `{target}` was refused: {error}"))?;
            if policy.selected_target.path() != target {
                return Err(format!(
                    "selected target `{}` does not match `{target}`",
                    policy.selected_target.path()
                ));
            }
            // A Rust repair runs Cargo: its build directory and the
            // workspace-root lockfile it may generate are declared build
            // state. A Python repair declares neither.
            let rust = target.ends_with(".rs");
            let expected_lockfile = if rust {
                Some(crate::edit_cage::CagePathRule::exact(
                    CARGO_WORKSPACE_LOCKFILE,
                )?)
            } else {
                None
            };
            if policy.untracked_build_lockfile != expected_lockfile
                || policy.ignored_build_output.is_some() != rust
            {
                return Err(format!(
                    "`{target}` declared build state {:?} / {:?}",
                    policy.untracked_build_lockfile, policy.ignored_build_output
                ));
            }
        }
        Ok(())
    }

    #[test]
    fn cage_policy_includes_explicit_store_outside_target_ripr() -> Result<(), String> {
        let packet = serde_json::json!({
            "seam_id": "seam:sample",
            "allowed_edit_surface": ["tests/target.rs"],
            "forbidden_files": []
        });
        let rendered = serde_json::to_string(&packet).map_err(|error| error.to_string())?;
        let mut covered = edit_cage_policy_from_packet(&rendered, "seam:sample")?;
        include_explicit_store_operational_write(
            &mut covered,
            Path::new("/repo"),
            Some(Path::new("target/ripr/alt-attempts")),
        )?;
        if covered.expected_operational_writes.len() != 1
            || covered.expected_operational_writes[0].path() != "target/ripr"
        {
            return Err(format!(
                "store under target/ripr must stay covered by the existing subtree: {:?}",
                covered
                    .expected_operational_writes
                    .iter()
                    .map(crate::edit_cage::CagePathRule::path)
                    .collect::<Vec<_>>()
            ));
        }

        let mut outside = edit_cage_policy_from_packet(&rendered, "seam:sample")?;
        include_explicit_store_operational_write(
            &mut outside,
            Path::new("/repo"),
            Some(Path::new(".ripr/attempts")),
        )?;
        let paths = outside
            .expected_operational_writes
            .iter()
            .map(crate::edit_cage::CagePathRule::path)
            .collect::<Vec<_>>();
        if !paths.contains(&"target/ripr") || !paths.contains(&".ripr/attempts") {
            return Err(format!(
                "explicit store outside target/ripr must be an operational write: {paths:?}"
            ));
        }
        if !outside.allows_path(".ripr/attempts/id/attempt.json") {
            return Err("explicit store path was not admitted as an operational write".to_string());
        }
        Ok(())
    }

    #[test]
    fn diverged_head_recovery_repeats_explicit_store_on_follow_up_commands() -> Result<(), String> {
        let recovery = diverged_head_recovery(
            ".",
            "repair-attempt-0123456789abcdef01234567",
            "seam:sample",
            "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
            " --store .ripr/attempts",
        );
        for line in recovery.lines() {
            if line.contains("ripr agent repair") && !line.contains("--store .ripr/attempts") {
                return Err(format!("recovery command lost --store: {line}"));
            }
        }
        if !recovery.reset.contains("--phase after")
            || !recovery.restart.contains("--phase before")
            || !recovery.reset.contains("--store .ripr/attempts")
            || !recovery.restart.contains("--store .ripr/attempts")
        {
            return Err(format!(
                "diverged recovery lost store identity: reset={} restart={}",
                recovery.reset, recovery.restart
            ));
        }
        Ok(())
    }

    #[test]
    fn after_phase_not_awaiting_error_names_explicit_store() -> Result<(), String> {
        let root = test_root("after-store")?;
        let mut manifest = sample_manifest(&root)?;
        manifest.state = RepairAttemptState::ReadyToFinish;
        manifest.after = Some(RepairAttemptAfter {
            attempt_id: manifest.repair_attempt_id.clone(),
            repository_head: manifest.repository_head.clone(),
            delta_sha256: "sha256:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"
                .to_string(),
            packet_sha256:
                "sha256:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"
                    .to_string(),
            current: true,
            verdict: crate::edit_cage::EditCageVerdict {
                status: crate::edit_cage::EditCageVerdictStatus::Compliant,
                changed_paths: Vec::new(),
                violations: Vec::new(),
            },
        });
        manifest.store = Some(RepairAttemptStoreIdentity {
            schema_version: store::REPAIR_ATTEMPT_STORE_SCHEMA_VERSION.to_string(),
            location_class: store::RepairAttemptStoreLocationClass::ExplicitRepository,
            locator: ".ripr/attempts".to_string(),
        });
        let error = after_phase_not_awaiting_error(".", &manifest);
        std::fs::remove_dir_all(&root)
            .map_err(|error| format!("remove {} failed: {error}", root.display()))?;
        if !error.contains("--store") || !error.contains(".ripr/attempts") {
            return Err(format!("finished-attempt recovery lost --store: {error}"));
        }
        if !error.contains("ripr agent status") || !error.contains("--phase before") {
            return Err(format!(
                "finished-attempt recovery lost follow-up commands: {error}"
            ));
        }
        Ok(())
    }

    #[test]
    fn repair_attempt_schema_carries_terminal_after_contract() -> Result<(), String> {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../schemas/ripr/repair-attempt.schema.json");
        let schema: serde_json::Value = serde_json::from_slice(
            &std::fs::read(&path).map_err(|error| format!("read schema failed: {error}"))?,
        )
        .map_err(|error| format!("decode schema failed: {error}"))?;
        let states = schema["properties"]["state"]["enum"]
            .as_array()
            .ok_or_else(|| "schema state enum missing".to_string())?;
        for state in [
            "awaiting_edit",
            "ready_to_finish",
            "stale",
            "incomparable",
            "failed",
        ] {
            if !states.iter().any(|value| value == state) {
                return Err(format!("schema omitted state {state}"));
            }
        }
        if schema["$defs"]["after"].is_null() {
            return Err("schema omitted terminal after definition".to_string());
        }
        let required = schema["$defs"]["after"]["required"]
            .as_array()
            .ok_or_else(|| "schema after required list missing".to_string())?;
        if !required.iter().any(|value| value == "attempt_id") {
            return Err("schema after omitted durable attempt_id".to_string());
        }
        Ok(())
    }

    #[test]
    fn manifest_write_is_atomic_and_round_trips() -> Result<(), String> {
        let root = test_root("manifest")?;
        let manifest = sample_manifest(&root)?;
        let path = write_repair_attempt_manifest(&prepared_store(&root)?, &manifest)?;
        let raw = std::fs::read_to_string(&path)
            .map_err(|error| format!("read {} failed: {error}", path.display()))?;
        let decoded: RepairAttemptManifest = serde_json::from_str(&raw)
            .map_err(|error| format!("decode repair attempt manifest failed: {error}"))?;
        if decoded != manifest {
            return Err("repair attempt manifest changed during round trip".to_string());
        }
        let mut unknown_top_level = serde_json::to_value(&manifest)
            .map_err(|error| format!("encode manifest for unknown-field test failed: {error}"))?;
        unknown_top_level["unexpected"] = serde_json::Value::Bool(true);
        if serde_json::from_value::<RepairAttemptManifest>(unknown_top_level).is_ok() {
            return Err("manifest decoder accepted an unknown top-level field".to_string());
        }
        let mut unknown_artifact = serde_json::to_value(&manifest)
            .map_err(|error| format!("encode artifact for unknown-field test failed: {error}"))?;
        unknown_artifact["artifacts"][0]["unexpected"] = serde_json::Value::Bool(true);
        if serde_json::from_value::<RepairAttemptManifest>(unknown_artifact).is_ok() {
            return Err("manifest decoder accepted an unknown artifact field".to_string());
        }
        let leftovers = std::fs::read_dir(
            path.parent()
                .ok_or_else(|| "repair attempt manifest has no parent directory".to_string())?,
        )
        .map_err(|error| format!("read manifest directory failed: {error}"))?
        .filter_map(Result::ok)
        .filter(|entry| entry.file_name().to_string_lossy().contains("tmp-"))
        .count();
        std::fs::remove_dir_all(&root)
            .map_err(|error| format!("remove {} failed: {error}", root.display()))?;
        if leftovers != 0 {
            return Err(format!(
                "atomic manifest write left {leftovers} temporary files"
            ));
        }
        Ok(())
    }

    #[test]
    fn staged_artifacts_are_isolated_and_digest_bound() -> Result<(), String> {
        let root = test_root("artifacts")?;
        let source = root.join("before.json");
        std::fs::write(&source, b"{}")
            .map_err(|error| format!("write {} failed: {error}", source.display()))?;
        let manifest = sample_manifest(&root)?;
        let attempt_directory = repair_attempt_directory(&root, &manifest.repair_attempt_id);
        let artifacts = stage_before_artifacts(
            &root,
            &attempt_directory,
            &[BeforeArtifactSource {
                role: "before_snapshot",
                path: &source,
            }],
        )?;
        if artifacts.len() != 1
            || artifacts[0].bytes != 2
            || artifacts[0].sha256 != sha256_bytes(b"{}")
            || !root.join(&artifacts[0].path).is_file()
        {
            return Err(format!("unexpected staged artifact: {artifacts:?}"));
        }
        let staging_leftovers = std::fs::read_dir(&attempt_directory)
            .map_err(|error| format!("read attempt directory failed: {error}"))?
            .filter_map(Result::ok)
            .filter(|entry| entry.file_name().to_string_lossy().contains("tmp-"))
            .count();
        std::fs::remove_dir_all(&root)
            .map_err(|error| format!("remove {} failed: {error}", root.display()))?;
        if staging_leftovers != 0 {
            return Err(format!(
                "staging left {staging_leftovers} temporary entries in the attempt directory"
            ));
        }
        Ok(())
    }

    #[test]
    fn repair_attempt_id_rejects_uppercase_and_preserves_value() -> Result<(), String> {
        let canonical = "repair-attempt-0123456789abcdef01234567";
        let parsed = RepairAttemptId::parse(canonical)?;
        if parsed.as_str() != canonical {
            return Err(format!(
                "repair attempt ID was rewritten during parse: {}",
                parsed.as_str()
            ));
        }
        if RepairAttemptId::parse("repair-attempt-0123456789ABCDEF01234567").is_ok() {
            return Err("uppercase repair attempt ID was accepted".to_string());
        }
        if RepairAttemptId::parse("repair-attempt-0123456789abcdef0123456").is_ok() {
            return Err("short repair attempt ID was accepted".to_string());
        }
        Ok(())
    }

    type ManifestMutation = (&'static str, fn(&mut RepairAttemptManifest));

    #[test]
    fn validate_manifest_accepts_sample_and_rejects_each_schema_invariant() -> Result<(), String> {
        let root = test_root("validate")?;
        validate_manifest(&sample_manifest(&root)?)
            .map_err(|error| format!("sample manifest was rejected: {error}"))?;
        let cases: Vec<ManifestMutation> = vec![
            ("schema_version is not 0.1", |manifest| {
                manifest.schema_version = "9.9".to_string();
            }),
            ("kind is not repair_attempt", |manifest| {
                manifest.kind = "repair_attempts".to_string();
            }),
            ("repair_attempt_id has uppercase hex", |manifest| {
                manifest.repair_attempt_id =
                    RepairAttemptId("repair-attempt-0123456789ABCDEF01234567".to_string());
            }),
            ("repair_attempt_id has wrong length", |manifest| {
                manifest.repair_attempt_id = RepairAttemptId("repair-attempt-0123".to_string());
            }),
            ("terminal state is missing after", |manifest| {
                manifest.state = RepairAttemptState::ReadyToFinish;
            }),
            ("root is empty", |manifest| {
                manifest.root.clear();
            }),
            ("repository_head is not 40 characters", |manifest| {
                manifest.repository_head = "0123".to_string();
            }),
            ("repository_head is not hexadecimal", |manifest| {
                manifest.repository_head = "g".repeat(40);
            }),
            ("producer_version is empty", |manifest| {
                manifest.producer_version.clear();
            }),
            ("seam_id is empty", |manifest| {
                manifest.seam_id.clear();
            }),
            ("artifacts is empty", |manifest| {
                manifest.artifacts.clear();
            }),
            ("next_command is empty", |manifest| {
                manifest.next_command.clear();
            }),
            ("artifact role is blank", |manifest| {
                manifest.artifacts[0].role.clear();
            }),
            ("artifact path is blank", |manifest| {
                manifest.artifacts[0].path.clear();
            }),
            ("artifact role is duplicated", |manifest| {
                let mut duplicate = manifest.artifacts[0].clone();
                duplicate.path =
                    "target/ripr/repair-attempts/sample/artifacts/other.json".to_string();
                manifest.artifacts.push(duplicate);
            }),
            ("artifact path is duplicated", |manifest| {
                let mut duplicate = manifest.artifacts[0].clone();
                duplicate.role = "other_snapshot".to_string();
                manifest.artifacts.push(duplicate);
            }),
            ("artifact sha256 is missing the prefix", |manifest| {
                manifest.artifacts[0].sha256 =
                    "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef".to_string();
            }),
            ("artifact sha256 has the wrong length", |manifest| {
                manifest.artifacts[0].sha256 = "sha256:0123".to_string();
            }),
            ("artifact sha256 has uppercase hex", |manifest| {
                manifest.artifacts[0].sha256 = format!("sha256:{}", "A".repeat(64));
            }),
            ("limitations entry is empty", |manifest| {
                manifest.limitations = vec![String::new()];
            }),
            ("non_claims is empty", |manifest| {
                manifest.non_claims.clear();
            }),
            ("non_claims entry is empty", |manifest| {
                manifest.non_claims = vec![String::new()];
            }),
            ("nested store schema_version is not 0.1", |manifest| {
                manifest.store = Some(RepairAttemptStoreIdentity {
                    schema_version: "9.9".to_string(),
                    location_class: store::RepairAttemptStoreLocationClass::ExplicitRepository,
                    locator: "target/ripr/alt-attempts".to_string(),
                });
            }),
        ];
        for (name, mutate) in cases {
            let mut manifest = sample_manifest(&root)?;
            mutate(&mut manifest);
            if validate_manifest(&manifest).is_ok() {
                std::fs::remove_dir_all(&root)
                    .map_err(|error| format!("remove {} failed: {error}", root.display()))?;
                return Err(format!("validate_manifest accepted invalid case: {name}"));
            }
        }
        std::fs::remove_dir_all(&root)
            .map_err(|error| format!("remove {} failed: {error}", root.display()))?;
        Ok(())
    }

    #[test]
    fn attempt_directory_reservation_is_exclusive() -> Result<(), String> {
        let root = test_root("reserve")?;
        let attempt_id = RepairAttemptId::parse("repair-attempt-0123456789abcdef01234567")?;
        let store = prepared_store(&root)?;
        reserve_attempt_directory(&store, &attempt_id)?;
        let second = reserve_attempt_directory(&store, &attempt_id);
        std::fs::remove_dir_all(&root)
            .map_err(|error| format!("remove {} failed: {error}", root.display()))?;
        match second {
            Err(error) if error.contains("already exists") => Ok(()),
            other => Err(format!(
                "second reservation of the same attempt was not rejected: {other:?}"
            )),
        }
    }

    #[test]
    fn write_bytes_atomic_refuses_to_overwrite_a_destination() -> Result<(), String> {
        let root = test_root("immutable")?;
        let path = root.join("attempt.json");
        write_bytes_atomic(&path, b"first")?;
        let second = write_bytes_atomic(&path, b"second");
        let contents = std::fs::read(&path)
            .map_err(|error| format!("read {} failed: {error}", path.display()))?;
        std::fs::remove_dir_all(&root)
            .map_err(|error| format!("remove {} failed: {error}", root.display()))?;
        if second.is_ok() {
            return Err("immutable repair attempt destination was overwritten".to_string());
        }
        if contents != b"first" {
            return Err("immutable repair attempt destination bytes changed".to_string());
        }
        Ok(())
    }

    #[test]
    fn replace_file_atomically_swaps_in_place_and_fails_without_destroying() -> Result<(), String> {
        let root = test_root("replace")?;
        // An existing destination is replaced in place by the rename itself:
        // no delete step runs, so a reader never observes the path missing
        // and a failed replacement leaves the previous content in place.
        let path = root.join("projection.json");
        replace_file_atomically(&path, b"first")?;
        replace_file_atomically(&path, b"second")?;
        let contents = std::fs::read(&path)
            .map_err(|error| format!("read {} failed: {error}", path.display()))?;
        if contents != b"second" {
            return Err("replace_file_atomically did not swap the destination bytes".to_string());
        }
        // No staging residue: every temporary sibling is cleaned up.
        let residue = std::fs::read_dir(&root)
            .map_err(|error| format!("read {} failed: {error}", root.display()))?
            .filter_map(Result::ok)
            .filter(|entry| entry.file_name().to_string_lossy().contains(".tmp-"))
            .count();
        if residue != 0 {
            return Err(format!("replace_file_atomically left {residue} temp files"));
        }
        // A failed replacement (the destination is a directory, so the rename
        // cannot land) returns an error and does not destroy the destination.
        let blocked = root.join("blocked.json");
        std::fs::create_dir(&blocked)
            .map_err(|error| format!("create blocking directory failed: {error}"))?;
        let result = replace_file_atomically(&blocked, b"unlandable");
        let blocked_still_there = blocked.is_dir();
        std::fs::remove_dir_all(&root)
            .map_err(|error| format!("remove {} failed: {error}", root.display()))?;
        if result.is_ok() {
            return Err("a rename onto a directory destination succeeded".to_string());
        }
        if !blocked_still_there {
            return Err("a failed replacement destroyed the pre-existing destination".to_string());
        }
        Ok(())
    }

    #[test]
    fn failed_begin_leaves_no_orphan_attempt_directory() -> Result<(), String> {
        let root = test_repo_root("orphan")?;
        let missing = root.join("missing-before.json");
        let result = begin_repair_attempt_with(BeginRepairAttemptOptions {
            root: &root,
            root_argument: &root,
            seam_id: "seam:sample",
            sources: &[BeforeArtifactSource {
                role: "before_snapshot",
                path: &missing,
            }],
            expected_repository_head: None,
            next_command_suffix: None,
            store: None,
        });
        if result.is_ok() {
            return Err("begin_repair_attempt accepted a missing artifact source".to_string());
        }
        let attempts_root = root.join(REPAIR_ATTEMPT_DIRECTORY);
        let orphans = std::fs::read_dir(&attempts_root)
            .map_err(|error| format!("read {} failed: {error}", attempts_root.display()))?
            .filter_map(Result::ok)
            .filter(|entry| {
                entry
                    .file_name()
                    .to_string_lossy()
                    .starts_with(REPAIR_ATTEMPT_ID_PREFIX)
            })
            .count();
        std::fs::remove_dir_all(&root)
            .map_err(|error| format!("remove {} failed: {error}", root.display()))?;
        if orphans != 0 {
            return Err(format!(
                "failed begin_repair_attempt left {orphans} orphan attempt directories"
            ));
        }
        Ok(())
    }

    #[test]
    fn failed_manifest_write_removes_staged_attempt_and_artifacts() -> Result<(), String> {
        // A real repository: the finalize-path head re-verification must pass
        // so the occupied manifest path is what actually forces the publish
        // failure.
        let root = test_repo_root("publish")?;
        let head = crate::agent::artifact::current_git_head(&root)?;
        let attempt_id = RepairAttemptId::parse("repair-attempt-0123456789abcdef01234567")?;
        let store = prepared_store(&root)?;
        let attempt_directory = reserve_attempt_directory(&store, &attempt_id)?;
        let source = root.join("before.json");
        std::fs::write(&source, b"{}")
            .map_err(|error| format!("write {} failed: {error}", source.display()))?;
        // Force the manifest publish to fail after staging succeeds: the
        // manifest destination already exists as a directory, so the
        // exclusive link in write_bytes_atomic fails closed.
        std::fs::create_dir(attempt_directory.join(REPAIR_ATTEMPT_MANIFEST))
            .map_err(|error| format!("create occupied manifest path failed: {error}"))?;
        let result = complete_repair_attempt(
            &store,
            &attempt_directory,
            AttemptPublication {
                root_argument: &root,
                seam_id: "seam:sample",
                repository_head: head,
                expected_repository_head: None,
                created_unix_ms: 1,
                repair_attempt_id: attempt_id,
                sources: &[BeforeArtifactSource {
                    role: "before_snapshot",
                    path: &source,
                }],
                next_command_suffix: None,
            },
        );
        let attempt_remaining = attempt_directory.exists();
        std::fs::remove_dir_all(&root)
            .map_err(|error| format!("remove {} failed: {error}", root.display()))?;
        if result.is_ok() {
            return Err(
                "complete_repair_attempt published over an occupied manifest path".to_string(),
            );
        }
        if attempt_remaining {
            return Err(
                "failed manifest write left the attempt directory and staged artifacts behind"
                    .to_string(),
            );
        }
        Ok(())
    }

    #[test]
    fn exact_attempt_selection_survives_same_seam_concurrency() -> Result<(), String> {
        let root = test_repo_root("exact-selection")?;
        let first = prepare_sample_attempt(&root, "seam:sample", "first")?;
        let second = prepare_sample_attempt(&root, "seam:sample", "second")?;

        let exact = resolve_awaiting_repair_attempt(
            &root,
            Some(first.manifest.repair_attempt_id.as_str()),
            None,
        )?;
        if exact.attempt_id != first.manifest.repair_attempt_id
            || exact.seam_id != "seam:sample"
            || !exact
                .before_snapshot_path
                .starts_with(repair_attempt_directory(
                    &root,
                    &first.manifest.repair_attempt_id,
                ))
            || !exact.packet_path.starts_with(repair_attempt_directory(
                &root,
                &first.manifest.repair_attempt_id,
            ))
        {
            return Err(format!(
                "exact attempt resolved the wrong inputs: {exact:?}"
            ));
        }

        let ambiguous = resolve_awaiting_repair_attempt(&root, None, Some("seam:sample"));
        match ambiguous {
            Err(error) if error.contains("found 2") && error.contains("--attempt") => {}
            other => {
                return Err(format!(
                    "same-seam compatibility selection was not rejected: {other:?}"
                ));
            }
        }

        let second_exact = resolve_awaiting_repair_attempt(
            &root,
            Some(second.manifest.repair_attempt_id.as_str()),
            None,
        )?;
        let wrong_packet = finish_repair_attempt(
            &root,
            &first.manifest.repair_attempt_id,
            &second_exact.packet_path,
            HeadMovement::AdmitDescendantCommits,
        );
        match wrong_packet {
            Err(error) if error.contains("retained agent_packet") => {}
            other => {
                return Err(format!(
                    "finish accepted another attempt's retained packet: {other:?}"
                ));
            }
        }

        let test_path = root.join("tests/target.rs");
        let test_parent = test_path
            .parent()
            .ok_or_else(|| "test path has no parent".to_string())?;
        std::fs::create_dir_all(test_parent)
            .map_err(|error| format!("create {} failed: {error}", test_parent.display()))?;
        std::fs::write(&test_path, "#[test]\nfn focused() {}\n")
            .map_err(|error| format!("write {} failed: {error}", test_path.display()))?;

        let after = finish_repair_attempt(
            &root,
            &first.manifest.repair_attempt_id,
            &exact.packet_path,
            HeadMovement::AdmitDescendantCommits,
        )?;
        if after.attempt_id != first.manifest.repair_attempt_id
            || !after.current
            || after.verdict.status != crate::edit_cage::EditCageVerdictStatus::Compliant
        {
            return Err(format!("unexpected exact finish result: {after:?}"));
        }
        let (_, _, first_manifest) = open_attempt(&root, None, &first.manifest.repair_attempt_id)?;
        let (_, _, second_manifest) =
            open_attempt(&root, None, &second.manifest.repair_attempt_id)?;
        if first_manifest.state != RepairAttemptState::ReadyToFinish
            || second_manifest.state != RepairAttemptState::AwaitingEdit
        {
            return Err(format!(
                "exact finish changed the wrong attempt states: first={:?}, second={:?}",
                first_manifest.state, second_manifest.state
            ));
        }

        std::fs::remove_dir_all(&root)
            .map_err(|error| format!("remove {} failed: {error}", root.display()))?;
        Ok(())
    }

    #[test]
    fn expected_head_mismatch_refuses_before_any_attempt_is_published() -> Result<(), String> {
        let root = test_repo_root("head-gate")?;
        let actual_head = crate::agent::artifact::current_git_head(&root)?;
        let source = root.join("before.json");
        std::fs::write(&source, b"{}")
            .map_err(|error| format!("write {} failed: {error}", source.display()))?;

        // The binding's head pin no longer matches the repository HEAD (HEAD
        // moved after the binding was read): the publication refuses with a
        // typed error BEFORE the attempt directory is reserved, so no
        // awaiting_edit attempt from a mismatched tree can survive.
        let drifted_pin = "1111111111111111111111111111111111111111";
        let mismatch = begin_repair_attempt_with(BeginRepairAttemptOptions {
            root: &root,
            root_argument: &root,
            seam_id: "seam:sample",
            sources: &[BeforeArtifactSource {
                role: "before_snapshot",
                path: &source,
            }],
            expected_repository_head: Some(drifted_pin),
            next_command_suffix: None,
            store: None,
        });
        match mismatch {
            Err(error) if error.contains("head moved") => {}
            other => {
                return Err(format!(
                    "a drifted head pin was not refused before publication: {other:?}"
                ));
            }
        }
        let attempts_root = root.join(REPAIR_ATTEMPT_DIRECTORY);
        let survivors = if attempts_root.is_dir() {
            std::fs::read_dir(&attempts_root)
                .map_err(|error| format!("read {} failed: {error}", attempts_root.display()))?
                .filter_map(Result::ok)
                .filter(|entry| {
                    entry
                        .file_name()
                        .to_string_lossy()
                        .starts_with(REPAIR_ATTEMPT_ID_PREFIX)
                })
                .count()
        } else {
            0
        };
        if survivors != 0 {
            return Err(format!(
                "a head-mismatch refusal left {survivors} attempt directories behind"
            ));
        }

        // The matching pin publishes, and the trust-bound suffix rides in the
        // published follow-up command.
        let published = begin_repair_attempt_with(BeginRepairAttemptOptions {
            root: &root,
            root_argument: &root,
            seam_id: "seam:sample",
            sources: &[BeforeArtifactSource {
                role: "before_snapshot",
                path: &source,
            }],
            expected_repository_head: Some(&actual_head),
            next_command_suffix: Some(
                " --edit-authorized --edit-authority <operator-or-agent-identity>",
            ),
            store: None,
        })?;
        for fragment in ["--edit-authorized", "--edit-authority"] {
            if !published.manifest.next_command.contains(fragment) {
                return Err(format!(
                    "the trust-bound follow-up command does not name `{fragment}`: {}",
                    published.manifest.next_command
                ));
            }
        }
        std::fs::remove_dir_all(&root)
            .map_err(|error| format!("remove {} failed: {error}", root.display()))?;
        Ok(())
    }

    /// The receipt's tracked-surface check compares with the prepared head,
    /// so a production change the attempt committed on top of it is refused
    /// even though it is no longer a difference from the current HEAD.
    #[test]
    fn trusted_surface_includes_changes_committed_after_the_prepared_head() -> Result<(), String> {
        let root = test_repo_root("trusted-surface-commits")?;
        let result = (|| -> Result<(), String> {
            let packet = serde_json::json!({
                "seam_id": "seam:sample",
                "allowed_edit_surface": ["tests/target.rs"],
                "forbidden_files": []
            });
            let policy = edit_cage_policy_from_packet(
                &serde_json::to_string(&packet).map_err(|error| error.to_string())?,
                "seam:sample",
            )?;
            let prepared_head = crate::agent::artifact::current_git_head(&root)?;
            std::fs::create_dir_all(root.join("tests"))
                .map_err(|error| format!("create tests: {error}"))?;
            std::fs::write(root.join("tests/target.rs"), "#[test]\nfn focused() {}\n")
                .map_err(|error| format!("write test: {error}"))?;
            run_git(&root, &["add", "tests/target.rs"])?;
            run_git(&root, &["commit", "--no-gpg-sign", "-qm", "focused test"])?;
            validate_trusted_head_surface(&root, &policy, &[], &prepared_head)?;

            std::fs::write(root.join("src.rs"), "pub fn moved() {}\n")
                .map_err(|error| format!("write production file: {error}"))?;
            run_git(&root, &["add", "src.rs"])?;
            run_git(&root, &["commit", "--no-gpg-sign", "-qm", "production"])?;
            // Precondition: the production change is not a difference from
            // the current HEAD, only from the prepared head.
            if !git_paths(&root, &["diff", "--name-only", "-z", "HEAD"])?.is_empty() {
                return Err("the production change must be committed".to_string());
            }
            match validate_trusted_head_surface(&root, &policy, &[], &prepared_head) {
                Err(error) if error.contains("outside trusted edit surface: src.rs") => Ok(()),
                other => Err(format!(
                    "a committed production change must block the receipt: {other:?}"
                )),
            }
        })();
        let _ = std::fs::remove_dir_all(&root);
        result
    }

    /// The surface inventory's boundary is the allowed edit surface, not
    /// worktree cleanliness (#5262): a dirty focused test file inside the
    /// allowed surface is the loop's expected mid-loop edit and is not a
    /// violation, while the same inventory refuses dirty production content
    /// outside it. The before-phase premise gate shares this function, so the
    /// production-vs-test boundary is pinned at the shared owner.
    #[test]
    fn trusted_surface_violations_allow_a_dirty_test_file_and_name_production_only()
    -> Result<(), String> {
        let root = test_repo_root("trusted-surface-boundary")?;
        let result = (|| -> Result<(), String> {
            std::fs::create_dir_all(root.join("tests"))
                .map_err(|error| format!("create tests dir: {error}"))?;
            std::fs::write(root.join("tests/target.rs"), "#[test]\nfn focused() {}\n")
                .map_err(|error| format!("write test: {error}"))?;
            run_git(&root, &["add", "tests/target.rs"])?;
            run_git(&root, &["commit", "--no-gpg-sign", "-qm", "focused test"])?;
            let packet = serde_json::json!({
                "seam_id": "seam:sample",
                "allowed_edit_surface": ["tests/target.rs"],
                "forbidden_files": []
            });
            let policy = edit_cage_policy_from_packet(
                &serde_json::to_string(&packet).map_err(|error| error.to_string())?,
                "seam:sample",
            )?;
            let head = crate::agent::artifact::current_git_head(&root)?;

            // A tracked, committed, then dirtied allowed-surface test file is
            // a tracked difference from HEAD that the surface allows.
            std::fs::write(root.join("tests/target.rs"), "#[test]\nfn stronger() {}\n")
                .map_err(|error| format!("edit test: {error}"))?;
            let violations = trusted_head_surface_violations(&root, &policy, &[], &head)?;
            if !violations.is_empty() {
                return Err(format!(
                    "a dirty allowed-surface test file must not violate the surface: {violations:?}"
                ));
            }

            // The same dirty test edit plus a tracked, worktree-modified
            // production file violates exactly the production path: the
            // issue's premise is a committed file modified but not
            // recommitted, which untracked content alone would not exercise.
            std::fs::write(root.join("src.rs"), "pub fn base() {}\n")
                .map_err(|error| format!("write production file: {error}"))?;
            run_git(&root, &["add", "src.rs"])?;
            run_git(&root, &["commit", "--no-gpg-sign", "-qm", "production"])?;
            std::fs::write(root.join("src.rs"), "pub fn drift() {}\n")
                .map_err(|error| format!("modify production file: {error}"))?;
            let head = crate::agent::artifact::current_git_head(&root)?;
            let violations = trusted_head_surface_violations(&root, &policy, &[], &head)?;
            if violations != vec!["src.rs".to_string()] {
                return Err(format!(
                    "dirty production content must be the only violation: {violations:?}"
                ));
            }
            Ok(())
        })();
        let _ = std::fs::remove_dir_all(&root);
        result
    }

    /// The terminal receipt-refusal authority records the narration beside a
    /// committed after verdict (#5262 review): the late trusted-surface
    /// refusal fires only after `finish_repair_attempt`, so the resumable
    /// authority refuses it by design and the recovery route would otherwise
    /// never reach `agent status`. Pins that the record lands, that state and
    /// the after verdict are untouched, and that the resumable authority
    /// stays guarded for this attempt.
    #[test]
    fn terminal_receipt_refusal_records_beside_the_after_verdict() -> Result<(), String> {
        let root = test_repo_root("terminal-receipt-refusal")?;
        let result = (|| -> Result<(), String> {
            let prepared = prepare_sample_attempt(&root, "seam:sample", "a")?;
            let finished = finish_sample_attempt(&root, &prepared)?;
            let attempt_id = &finished.repair_attempt_id;
            let reason = "repair receipt observed repository path outside trusted edit surface: src.rs. to recover: commit the production change, then start a new attempt.";

            record_repair_attempt_terminal_receipt_refusal_from(&root, None, attempt_id, reason)?;
            let manifest = load_repair_attempt_manifest(&root, attempt_id)?;
            let refusal = manifest.last_after_refusal.as_ref().ok_or_else(|| {
                "the terminal receipt refusal was not recorded on the manifest".to_string()
            })?;
            if !refusal.reason.contains("to recover:") {
                return Err(format!(
                    "recorded refusal lost the recovery route: {refusal:?}"
                ));
            }
            if manifest.after.is_none() || manifest.state != RepairAttemptState::ReadyToFinish {
                return Err(format!(
                    "the terminal record moved the attempt's finish: state {:?}, after present {}",
                    manifest.state,
                    manifest.after.is_some()
                ));
            }

            // The resumable authority stays guarded beside the committed
            // verdict: only the trusted-surface family routes to the terminal
            // writer.
            match record_repair_attempt_after_refusal_from(&root, None, attempt_id, "resumable") {
                Err(error) if error.contains("already has an after verdict") => {}
                other => {
                    return Err(format!(
                        "resumable refusal authority must stay guarded: {other:?}"
                    ));
                }
            }
            // The terminal authority refuses an attempt that never finished:
            // its observation belongs to the resumable authority.
            let awaiting = prepare_sample_attempt(&root, "seam:sample", "b")?;
            match record_repair_attempt_terminal_receipt_refusal_from(
                &root,
                None,
                &awaiting.manifest.repair_attempt_id,
                reason,
            ) {
                Err(error) if error.contains("no after verdict") => {}
                other => {
                    return Err(format!(
                        "terminal authority must refuse an attempt without a verdict: {other:?}"
                    ));
                }
            }
            Ok(())
        })();
        let _ = std::fs::remove_dir_all(&root);
        result
    }

    #[test]
    fn strict_git_paths_inventory_rejects_non_utf8() -> Result<(), String> {
        // The strict-failure side of the NUL authority at the repair-attempt
        // decode boundary: non-UTF-8 records fail loudly instead of
        // collapsing through lossy conversion (which would admit a rewritten
        // path in the trusted-surface validator).
        let err = match decode_git_paths(b"ok.txt\0\xffbad\0", "diff --name-only -z") {
            Err(err) => err,
            Ok(paths) => {
                return Err(format!("non-UTF-8 inventory must fail, decoded {paths:?}"));
            }
        };
        if !err.contains("not valid UTF-8") {
            return Err(format!("unexpected strict-decode error: {err}"));
        }
        Ok(())
    }

    #[test]
    fn git_paths_decode_exotic_names_exact() -> Result<(), String> {
        // No-regression pin for the #4006 strict-decode migration: this
        // route already passed `-z`, so space and non-ASCII names decoded
        // before and must decode after. The migration's behavior delta is
        // strictness (non-UTF-8/empty records fail instead of collapsing),
        // pinned by `strict_git_paths_inventory_rejects_non_utf8`; live
        // non-UTF-8 names are impractical on Windows runners. Asserts
        // through the real `git_paths` production path.
        let root = test_repo_root("nul-paths")?;
        let result = (|| -> Result<(), String> {
            std::fs::write(root.join("sp ace.txt"), "spaces\n")
                .map_err(|error| format!("write fixture failed: {error}"))?;
            std::fs::write(root.join("uni-\u{e9}.txt"), "unicode\n")
                .map_err(|error| format!("write fixture failed: {error}"))?;
            run_git(&root, &["add", "-A"])?;
            run_git(&root, &["commit", "--no-gpg-sign", "-qm", "exotic"])?;
            let mut paths = git_paths(&root, &["diff", "--name-only", "-z", "HEAD~1", "HEAD"])?;
            paths.sort();
            let expected = vec!["sp ace.txt".to_string(), "uni-\u{e9}.txt".to_string()];
            if paths != expected {
                return Err(format!(
                    "exotic path inventory mismatch: got {paths:?}, want {expected:?}"
                ));
            }
            Ok(())
        })();
        let _ = std::fs::remove_dir_all(&root);
        result
    }

    #[test]
    fn git_paths_spawns_through_the_shared_git_authority() -> Result<(), String> {
        // #4363: the trusted-surface inventory must spawn through the shared
        // `crate::git` deadline/process-owner authority, not a direct git
        // process construction. A missing root fails the spawn inside the
        // shared collector, and the collector's describe text (`git -C <root>
        // ...`) is produced only on that shared path — a direct spawn can
        // never emit it, so its presence discriminates the routing. The
        // caller's own `run git ... failed` wrapper must survive so the
        // fail-closed admission error family is unchanged. The deadline
        // plumbing is established adapter-level by
        // `git_paths_supplies_the_promised_bounded_deadline`; the
        // terminate-and-reap behavior with a named timeout error is owned
        // by `git.rs`'s re-exec harness tests.
        let missing = Path::new("definitely-missing-git-root-for-4363");
        let Err(error) = git_paths(
            missing,
            &["ls-files", "--others", "--exclude-standard", "-z"],
        ) else {
            return Err(
                "an inventory against a missing root must fail closed, not succeed".to_string(),
            );
        };
        if !error.starts_with("run git ") {
            return Err(format!(
                "expected the caller wrapper to survive, got: {error}"
            ));
        }
        if !error.contains("git -C") {
            return Err(format!(
                "expected the shared authority's describe text (git -C), got: {error}"
            ));
        }
        if !error.contains("failed to run") {
            return Err(format!(
                "expected the shared spawn-failure family text, got: {error}"
            ));
        }
        Ok(())
    }

    #[test]
    fn git_paths_supplies_the_promised_bounded_deadline() -> Result<(), String> {
        // #4363 review: the routing witness above proves git_paths spawns
        // through the shared authority, but not that it supplies a bounded
        // deadline — a `None` (unbounded) argument would pass it. A zero
        // injected deadline through the same production wrapper path is
        // rejected by the shared authority BEFORE any spawn with the named
        // timeout-family error, so this case fails deterministically (no
        // git execution, no hung fixture) unless the deadline reaches the
        // shared authority. The production wrapper binds the fixed
        // `GIT_PATHS_DEADLINE` ceiling by construction; this test pins the
        // ceiling as nonzero and proves the plumbing honors a supplied
        // bound.
        if GIT_PATHS_DEADLINE.is_zero() {
            return Err("the trusted-surface deadline must be positive".to_string());
        }
        let plain_dir = std::env::temp_dir().join(format!(
            "ripr-git-paths-deadline-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ));
        std::fs::create_dir_all(&plain_dir).map_err(|err| format!("create plain root: {err}"))?;
        let result = git_paths_with_deadline(
            &plain_dir,
            &["ls-files", "--others", "--exclude-standard", "-z"],
            Some(Duration::ZERO),
        );
        std::fs::remove_dir_all(&plain_dir).map_err(|err| format!("remove plain root: {err}"))?;
        let Err(error) = result else {
            return Err("a zero injected deadline must fail closed, not succeed".to_string());
        };
        if !error.starts_with("run git ") {
            return Err(format!(
                "expected the caller wrapper to survive, got: {error}"
            ));
        }
        if !error.contains(crate::git::GIT_INVOCATION_TIMEOUT_PREFIX) {
            return Err(format!(
                "expected the shared timeout family for the injected deadline, got: {error}"
            ));
        }
        if !error.contains("zero deadline") {
            return Err(format!(
                "expected the shared zero-deadline rejection (no spawn), got: {error}"
            ));
        }
        Ok(())
    }

    #[test]
    fn restore_returns_a_finished_attempt_to_retryable_awaiting_edit() -> Result<(), String> {
        let root = test_repo_root("restore")?;
        let prepared = prepare_sample_attempt(&root, "seam:sample", "restore")?;
        let attempt_id = prepared.manifest.repair_attempt_id.clone();
        let resolved = resolve_awaiting_repair_attempt(&root, Some(attempt_id.as_str()), None)?;

        // Finish without the focused edit: a terminal after state the apply
        // path can no longer retry through.
        finish_repair_attempt(
            &root,
            &attempt_id,
            &resolved.packet_path,
            HeadMovement::AdmitDescendantCommits,
        )?;
        let (_, _, finished) = open_attempt(&root, None, &attempt_id)?;
        if finished.state == RepairAttemptState::AwaitingEdit || finished.after.is_none() {
            return Err(format!(
                "the sample attempt did not finish into a terminal state: {:?}",
                finished.state
            ));
        }

        // Restoring returns the exact before state, so the identical retry
        // resolves the attempt again.
        restore_repair_attempt_to_awaiting_edit(&root, &attempt_id)?;
        let (_, _, restored) = open_attempt(&root, None, &attempt_id)?;
        if restored.state != RepairAttemptState::AwaitingEdit || restored.after.is_some() {
            return Err(format!(
                "the restored attempt is not awaiting_edit without an after block: {:?}",
                restored.state
            ));
        }
        resolve_awaiting_repair_attempt(&root, Some(attempt_id.as_str()), None)?;

        // Restoring an already-awaiting attempt is a typed refusal.
        let again = restore_repair_attempt_to_awaiting_edit(&root, &attempt_id);
        match again {
            Err(error) if error.contains("already awaiting_edit") => {}
            other => {
                return Err(format!(
                    "restoring an awaiting attempt was not refused: {other:?}"
                ));
            }
        }
        std::fs::remove_dir_all(&root)
            .map_err(|error| format!("remove {} failed: {error}", root.display()))?;
        Ok(())
    }

    /// Two after phases racing the same attempt must not both report success
    /// with a silent lost update: exactly one commits, the loser gets a typed
    /// refusal (contention or already-ended), and the manifest matches the
    /// winner's verdict (#5287).
    ///
    /// A race that ends in an infrastructure error (a transient file or
    /// process failure under parallel load, never a typed refusal) retries
    /// with a fresh attempt: only a clean double-success fails fast, so the
    /// lost-update signal stays loud while Windows rename flakes stay quiet.
    #[test]
    fn concurrent_finish_attempts_leave_exactly_one_winner() -> Result<(), String> {
        let mut last_infrastructure_error = String::new();
        for round in 0..3 {
            let label = format!("concurrent-finish-{round}");
            // Setup/teardown IO failures (fixture git, temp dirs) are
            // infrastructure too: a real product bug still fails, either as a
            // lost update, a wrong refusal, or three exhausted rounds.
            let outcome = match race_two_finishes(&label) {
                Ok(outcome) => outcome,
                Err(error) => RaceOutcome::Infrastructure(error),
            };
            match outcome {
                RaceOutcome::Clean => return Ok(()),
                RaceOutcome::LostUpdate => {
                    return Err(
                        "both concurrent after phases reported success; one verdict was silently lost"
                            .to_string(),
                    );
                }
                RaceOutcome::Infrastructure(error) => {
                    last_infrastructure_error = error;
                }
                RaceOutcome::WrongRefusal(error) => return Err(error),
            }
        }
        Err(format!(
            "the race kept hitting infrastructure errors: {last_infrastructure_error}"
        ))
    }

    enum RaceOutcome {
        Clean,
        LostUpdate,
        Infrastructure(String),
        WrongRefusal(String),
    }

    fn race_two_finishes(label: &str) -> Result<RaceOutcome, String> {
        let root = test_repo_root(label)?;
        let outcome = race_two_finishes_at(&root, label)?;
        std::fs::remove_dir_all(&root)
            .map_err(|error| format!("remove {} failed: {error}", root.display()))?;
        Ok(outcome)
    }

    fn race_two_finishes_at(root: &Path, label: &str) -> Result<RaceOutcome, String> {
        let prepared = prepare_sample_attempt(root, "seam:sample", label)?;
        let attempt_id = prepared.manifest.repair_attempt_id.clone();
        let packet = root.join(&find_manifest_artifact(&prepared.manifest, "agent_packet")?.path);
        let test_path = root.join("tests/target.rs");
        std::fs::create_dir_all(
            test_path
                .parent()
                .ok_or_else(|| "test path has no parent".to_string())?,
        )
        .map_err(|error| format!("create tests dir failed: {error}"))?;
        std::fs::write(&test_path, "#[test]\nfn focused() {}\n")
            .map_err(|error| format!("write {} failed: {error}", test_path.display()))?;

        let barrier = std::sync::Arc::new(std::sync::Barrier::new(2));
        let run_finish = |barrier: std::sync::Arc<std::sync::Barrier>| {
            let root = root.to_path_buf();
            let attempt_id = attempt_id.clone();
            let packet = packet.clone();
            std::thread::spawn(move || {
                barrier.wait();
                finish_repair_attempt(
                    &root,
                    &attempt_id,
                    &packet,
                    HeadMovement::AdmitDescendantCommits,
                )
            })
        };
        let first = run_finish(std::sync::Arc::clone(&barrier));
        let second = run_finish(barrier);
        let first = first
            .join()
            .map_err(|payload| format!("first finish thread panicked: {payload:?}"))?;
        let second = second
            .join()
            .map_err(|payload| format!("second finish thread panicked: {payload:?}"))?;

        let (winner, loser) = match (first, second) {
            (Ok(after), Err(error)) | (Err(error), Ok(after)) => (after, error),
            (Ok(_), Ok(_)) => return Ok(RaceOutcome::LostUpdate),
            (Err(first), Err(second)) => {
                return Ok(RaceOutcome::Infrastructure(format!(
                    "both concurrent after phases failed: {first:?} / {second:?}"
                )));
            }
        };
        if loser.contains("failed:") {
            return Ok(RaceOutcome::Infrastructure(format!(
                "the losing after phase hit an infrastructure error: {loser:?}"
            )));
        }
        if !loser.contains("another process") && !loser.contains("already ") {
            return Ok(RaceOutcome::WrongRefusal(format!(
                "the losing after phase was not refused as contention or already-ended: {loser:?}"
            )));
        }
        let finished = load_repair_attempt_manifest(root, &attempt_id)?;
        if finished.after.as_ref() != Some(&winner) {
            return Ok(RaceOutcome::WrongRefusal(format!(
                "the manifest verdict does not match the winning after phase: {:?}",
                finished.after
            )));
        }
        Ok(RaceOutcome::Clean)
    }

    /// A commit attempted while another holder owns the attempt lock is
    /// refused as contention without touching the manifest (#5287).
    #[test]
    fn manifest_commit_under_a_held_lock_is_refused_as_contention() -> Result<(), String> {
        let root = test_repo_root("commit-contention")?;
        let prepared = prepare_sample_attempt(&root, "seam:sample", "commit-contention")?;
        let attempt_id = prepared.manifest.repair_attempt_id.clone();
        let (_store, manifest_path, _manifest) = open_attempt(&root, None, &attempt_id)?;
        let attempt_dir = manifest_path
            .parent()
            .ok_or_else(|| "manifest path has no parent".to_string())?;
        let base_bytes = std::fs::read(&manifest_path)
            .map_err(|error| format!("read {} failed: {error}", manifest_path.display()))?;

        let _held = lock_manifest_for_commit(attempt_dir, &attempt_id)?;
        let refused = commit_manifest_locked(
            &manifest_path,
            &attempt_id,
            &base_bytes,
            &base_bytes,
            |_| Ok(()),
        );
        match refused {
            Err(error) if error.contains("being finished by another process") => {}
            other => {
                return Err(format!(
                    "a commit under a held lock was not refused as contention: {other:?}"
                ));
            }
        }
        let untouched = std::fs::read(&manifest_path)
            .map_err(|error| format!("read {} failed: {error}", manifest_path.display()))?;
        if untouched != base_bytes {
            return Err("a refused commit changed the manifest bytes".to_string());
        }
        drop(_held);
        std::fs::remove_dir_all(&root)
            .map_err(|error| format!("remove {} failed: {error}", root.display()))?;
        Ok(())
    }

    /// A commit whose base predates a concurrent manifest change is refused
    /// without overwriting the newer bytes (#5287).
    #[test]
    fn manifest_commit_on_a_stale_base_is_refused_without_overwriting() -> Result<(), String> {
        let root = test_repo_root("commit-stale-base")?;
        let prepared = prepare_sample_attempt(&root, "seam:sample", "commit-stale-base")?;
        let attempt_id = prepared.manifest.repair_attempt_id.clone();
        let (_store, manifest_path, _manifest) = open_attempt(&root, None, &attempt_id)?;
        let base_bytes = std::fs::read(&manifest_path)
            .map_err(|error| format!("read {} failed: {error}", manifest_path.display()))?;

        // A concurrent writer moves the manifest (a refusal keeps the state,
        // so only the byte comparison can catch it).
        record_repair_attempt_after_refusal(&root, &attempt_id, "concurrent refusal")?;
        let committed = commit_manifest_locked(
            &manifest_path,
            &attempt_id,
            &base_bytes,
            &base_bytes,
            |_| Ok(()),
        );
        match committed {
            Err(error) if error.contains("changed while the after phase ran") => {}
            other => {
                return Err(format!(
                    "a commit on a stale base was not refused: {other:?}"
                ));
            }
        }
        let (_, _, current) = open_attempt(&root, None, &attempt_id)?;
        if current.last_after_refusal.is_none() {
            return Err("a refused stale commit clobbered the concurrent refusal".to_string());
        }
        std::fs::remove_dir_all(&root)
            .map_err(|error| format!("remove {} failed: {error}", root.display()))?;
        Ok(())
    }

    /// The committing opener returns the exact bytes its manifest was
    /// decoded from: base and parse are one snapshot, so no concurrent
    /// commit can slip between them (#5406 review).
    #[test]
    fn open_attempt_with_bytes_returns_its_decoded_snapshot() -> Result<(), String> {
        let root = test_repo_root("snapshot-round-trip")?;
        let prepared = prepare_sample_attempt(&root, "seam:sample", "snapshot-round-trip")?;
        let attempt_id = prepared.manifest.repair_attempt_id.clone();
        let (_, manifest_path, base_bytes, manifest) =
            open_attempt_with_bytes(&root, None, &attempt_id)?;
        let decoded: RepairAttemptManifest = serde_json::from_slice(&base_bytes)
            .map_err(|error| format!("decode snapshot bytes failed: {error}"))?;
        if decoded.repair_attempt_id != manifest.repair_attempt_id
            || decoded.state != manifest.state
        {
            return Err("snapshot bytes do not decode to the opened manifest".to_string());
        }
        let on_disk = std::fs::read(&manifest_path)
            .map_err(|error| format!("read {} failed: {error}", manifest_path.display()))?;
        if on_disk != base_bytes {
            return Err("snapshot bytes do not match the manifest on disk".to_string());
        }
        std::fs::remove_dir_all(&root)
            .map_err(|error| format!("remove {} failed: {error}", root.display()))?;
        Ok(())
    }

    /// The retry-refusal classifier accepts exactly the two commit-refusal
    /// sentences and rejects terminal, operational, and lookalike errors,
    /// so only a genuinely retryable commit loss maps to exit 3.
    #[test]
    fn commit_retry_refusal_classifier_matches_only_retryable_losses() -> Result<(), String> {
        let id = "repair-attempt-0123456789abcdef01234567";
        let contention = format!("repair attempt {id} {REPAIR_ATTEMPT_CONTENTION_TAIL}");
        let underfoot = format!("repair attempt {id} {REPAIR_ATTEMPT_CHANGED_UNDERFOOT_TAIL}");
        for error in [&contention, &underfoot] {
            if !repair_attempt_commit_error_is_retry_refusal(error) {
                return Err(format!("retry refusal not classified: {error:?}"));
            }
        }
        for error in [
            format!("repair attempt {id} already finished; restore it for a retry"),
            "read attempt.json failed: Access is denied.".to_string(),
            "repair attempt is being finished".to_string(),
            String::new(),
        ] {
            if repair_attempt_commit_error_is_retry_refusal(&error) {
                return Err(format!("non-retry error misclassified: {error:?}"));
            }
        }
        Ok(())
    }

    /// A recorded after-phase refusal is an observation on the attempt: the
    /// manifest still validates against its before commitment, the attempt
    /// stays resumable, the reason is bounded, and the next after phase that
    /// reaches the durable finish clears it.
    #[test]
    fn recorded_after_refusal_keeps_the_attempt_resumable_until_finish() -> Result<(), String> {
        let root = test_repo_root("refusal")?;
        let prepared = prepare_sample_attempt(&root, "seam:sample", "refusal")?;
        let attempt_id = prepared.manifest.repair_attempt_id.clone();
        let (_, manifest_path, _) = open_attempt(&root, None, &attempt_id)?;
        let before_bytes = std::fs::read(&manifest_path)
            .map_err(|error| format!("read {} failed: {error}", manifest_path.display()))?;
        if String::from_utf8_lossy(&before_bytes).contains("last_after_refusal") {
            return Err("a manifest without a refusal must not carry the field".to_string());
        }

        let long_reason = format!("agent verify refused: {}", "x".repeat(10_000));
        record_repair_attempt_after_refusal(&root, &attempt_id, &long_reason)?;
        let (_, _, refused) = open_attempt(&root, None, &attempt_id)?;
        let refusal = refused
            .last_after_refusal
            .clone()
            .ok_or_else(|| "the refusal was not recorded".to_string())?;
        if refused.state != RepairAttemptState::AwaitingEdit || refused.after.is_some() {
            return Err(format!(
                "recording a refusal moved the attempt: {:?}",
                refused.state
            ));
        }
        if !refusal.reason.starts_with("agent verify refused: ")
            || !refusal.reason.ends_with(" [truncated]")
            || refusal.reason.len() > REPAIR_ATTEMPT_REFUSAL_MAX_BYTES + " [truncated]".len()
        {
            return Err(format!(
                "the refusal reason is not bounded: {} bytes",
                refusal.reason.len()
            ));
        }
        if refusal.repository_head.as_deref() != Some(refused.repository_head.as_str()) {
            return Err(format!(
                "the refusal did not record the current HEAD: {:?}",
                refusal.repository_head
            ));
        }
        // The inventory status reads sees the same refusal, and the after
        // phase can still select the attempt.
        let inventory = inventory_repair_attempts(&root)?;
        match inventory.as_slice() {
            [RepairAttemptInventoryEntry::Valid(manifest)]
                if manifest.last_after_refusal.as_ref() == Some(&refusal) => {}
            other => return Err(format!("inventory lost the refusal: {other:?}")),
        }
        let resolved = resolve_awaiting_repair_attempt(&root, Some(attempt_id.as_str()), None)?;

        if record_repair_attempt_after_refusal(&root, &attempt_id, "  ").is_ok() {
            return Err("an empty refusal reason must be refused".to_string());
        }

        // The sample attempt is an ordinary (not trust-bound) attempt, so its
        // after phase admits commits on top of the prepared head.
        finish_repair_attempt(
            &root,
            &attempt_id,
            &resolved.packet_path,
            HeadMovement::AdmitDescendantCommits,
        )?;
        let (_, _, finished) = open_attempt(&root, None, &attempt_id)?;
        if finished.last_after_refusal.is_some() {
            return Err("finish must clear the earlier refusal".to_string());
        }
        std::fs::remove_dir_all(&root)
            .map_err(|error| format!("remove {} failed: {error}", root.display()))?;
        Ok(())
    }

    /// A refusal recorded for an already-finished attempt is refused and
    /// leaves the terminal manifest byte-identical: the after verdict
    /// supersedes the refusal observation.
    #[test]
    fn after_refusal_on_a_finished_attempt_is_refused_without_touching_the_manifest()
    -> Result<(), String> {
        let root = test_repo_root("refusal-finished")?;
        let prepared = prepare_sample_attempt(&root, "seam:sample", "refusal-finished")?;
        let attempt_id = prepared.manifest.repair_attempt_id.clone();
        let resolved = resolve_awaiting_repair_attempt(&root, Some(attempt_id.as_str()), None)?;
        finish_repair_attempt(
            &root,
            &attempt_id,
            &resolved.packet_path,
            HeadMovement::AdmitDescendantCommits,
        )?;
        let (_, manifest_path, _) = open_attempt(&root, None, &attempt_id)?;
        let finished_bytes = std::fs::read(&manifest_path)
            .map_err(|error| format!("read {} failed: {error}", manifest_path.display()))?;
        match record_repair_attempt_after_refusal(&root, &attempt_id, "late refusal") {
            Err(error) if error.contains("already has an after verdict") => {}
            Err(error) => return Err(format!("unexpected refusal error: {error:?}")),
            Ok(()) => return Err("a refusal landed on a finished attempt".to_string()),
        }
        let kept_bytes = std::fs::read(&manifest_path)
            .map_err(|error| format!("read {} failed: {error}", manifest_path.display()))?;
        if kept_bytes != finished_bytes {
            return Err("the refused write still touched the terminal manifest".to_string());
        }
        std::fs::remove_dir_all(&root)
            .map_err(|error| format!("remove {} failed: {error}", root.display()))?;
        Ok(())
    }

    /// One head rule for the after phase and `ripr agent status`: an
    /// ordinary attempt is current at its prepared head and at a commit on
    /// top of it, and refused (still awaiting the edit) once history is
    /// rewritten; a trust-bound attempt is current only at the exact
    /// prepared head and otherwise finishes stale.
    #[test]
    fn after_phase_head_admission_follows_the_attempt_lineage() -> Result<(), String> {
        let root = test_repo_root("head-admission")?;
        let prepared = prepare_sample_attempt(&root, "seam:sample", "admission")?;
        let manifest = prepared.manifest;
        let current = || AfterPhaseHeadAdmission::Current {
            movement: HeadMovement::AdmitDescendantCommits,
        };
        if after_phase_head_admission(&root, &manifest)? != current() {
            return Err("an ordinary attempt at its prepared head is current".to_string());
        }
        let mut trust_bound = manifest.clone();
        trust_bound.artifacts.push(RepairAttemptArtifact {
            role: crate::app::python_repair_binding::BINDING_ARTIFACT_ROLE.to_string(),
            path: "unused".to_string(),
            sha256: "unused".to_string(),
            bytes: 0,
        });
        if after_phase_head_admission(&root, &trust_bound)?
            != (AfterPhaseHeadAdmission::Current {
                movement: HeadMovement::RequireBaselineHead,
            })
        {
            return Err("a trust-bound attempt at its prepared head is current".to_string());
        }

        run_git(
            &root,
            &[
                "commit",
                "--no-gpg-sign",
                "--allow-empty",
                "-qm",
                "focused test",
            ],
        )?;
        let descendant = crate::agent::artifact::current_git_head(&root)?;
        if after_phase_head_admission(&root, &manifest)? != current() {
            return Err("an ordinary attempt admits a descendant commit".to_string());
        }
        if after_phase_head_admission(&root, &trust_bound)?
            != (AfterPhaseHeadAdmission::FinishesStale {
                current_head: descendant,
            })
        {
            return Err("a trust-bound attempt pins the exact prepared head".to_string());
        }

        // Rewrite the prepared commit itself (as `git commit --amend` right
        // after the before phase would).
        run_git(&root, &["reset", "-q", "--soft", &manifest.repository_head])?;
        run_git(
            &root,
            &[
                "commit",
                "--no-gpg-sign",
                "--amend",
                "--allow-empty",
                "-qm",
                "rewritten",
            ],
        )?;
        let rewritten = crate::agent::artifact::current_git_head(&root)?;
        match after_phase_head_admission(&root, &manifest)? {
            AfterPhaseHeadAdmission::RefusedDiverged { current_head }
                if current_head == rewritten => {}
            other => return Err(format!("rewritten history must refuse: {other:?}")),
        }
        std::fs::remove_dir_all(&root)
            .map_err(|error| format!("remove {} failed: {error}", root.display()))?;
        Ok(())
    }

    fn prepare_sample_attempt(
        root: &Path,
        seam_id: &str,
        label: &str,
    ) -> Result<BeginRepairAttemptResult, String> {
        let workflow = root.join("target/ripr/workflow");
        std::fs::create_dir_all(&workflow)
            .map_err(|error| format!("create {} failed: {error}", workflow.display()))?;
        let before = workflow.join(format!("before-{label}.json"));
        let packet = workflow.join(format!("packet-{label}.json"));
        let baseline = workflow.join(format!("baseline-{label}.json"));
        // Evidence-grade before snapshot, not an opaque blob: terminal-receipt
        // and retention tests bind verify documents to its validated content
        // digest, and every other consumer treats the bytes opaquely.
        let before_snapshot = crate::testing::verify_fixture::mint_repo_exposure_snapshot(
            root,
            serde_json::json!([crate::testing::verify_fixture::snapshot_seam(
                seam_id,
                "predicate_boundary",
                "src/lib.rs",
                1,
                "weakly_gripped",
            )]),
        )?;
        std::fs::write(&before, before_snapshot.as_bytes())
            .map_err(|error| format!("write {} failed: {error}", before.display()))?;
        let packet_value = serde_json::json!({
            "seam_id": seam_id,
            "allowed_edit_surface": ["tests/target.rs"],
            "forbidden_files": []
        });
        let packet_text = serde_json::to_string_pretty(&packet_value)
            .map_err(|error| format!("serialize test packet failed: {error}"))?;
        std::fs::write(&packet, packet_text.as_bytes())
            .map_err(|error| format!("write {} failed: {error}", packet.display()))?;
        let policy = edit_cage_policy_from_packet(&packet_text, seam_id)?;
        write_edit_cage_baseline(root, &baseline, &policy)?;
        begin_repair_attempt_with(BeginRepairAttemptOptions {
            root,
            root_argument: root,
            seam_id,
            sources: &[
                BeforeArtifactSource {
                    role: "before_snapshot",
                    path: &before,
                },
                BeforeArtifactSource {
                    role: "agent_packet",
                    path: &packet,
                },
                BeforeArtifactSource {
                    role: "edit_cage_baseline",
                    path: &baseline,
                },
            ],
            expected_repository_head: None,
            next_command_suffix: None,
            store: None,
        })
    }

    fn test_repo_root(label: &str) -> Result<PathBuf, String> {
        let root = test_root(label)?;
        run_git(&root, &["init"])?;
        run_git(
            &root,
            &["config", "user.email", "ripr-test@example.invalid"],
        )?;
        run_git(&root, &["config", "user.name", "RIPR Test"])?;
        let readme = root.join("README.md");
        std::fs::write(&readme, "# test\n")
            .map_err(|error| format!("write {} failed: {error}", readme.display()))?;
        run_git(&root, &["add", "."])?;
        run_git(&root, &["commit", "--no-gpg-sign", "-m", "initial"])?;
        Ok(root)
    }

    fn finish_sample_attempt(
        root: &Path,
        prepared: &BeginRepairAttemptResult,
    ) -> Result<RepairAttemptManifest, String> {
        let test_path = root.join("tests/target.rs");
        std::fs::create_dir_all(
            test_path
                .parent()
                .ok_or_else(|| "test path has no parent".to_string())?,
        )
        .map_err(|error| format!("create tests dir failed: {error}"))?;
        std::fs::write(&test_path, "#[test]\nfn focused() {}\n")
            .map_err(|error| format!("write {} failed: {error}", test_path.display()))?;
        let packet = root.join(&find_manifest_artifact(&prepared.manifest, "agent_packet")?.path);
        finish_repair_attempt(
            root,
            &prepared.manifest.repair_attempt_id,
            &packet,
            HeadMovement::AdmitDescendantCommits,
        )?;
        load_repair_attempt_manifest(root, &prepared.manifest.repair_attempt_id)
    }

    fn bound_receipt_bytes(manifest: &RepairAttemptManifest) -> Result<Vec<u8>, String> {
        bound_receipt_bytes_with_verify(manifest, None)
    }

    /// Retain a receipt/verify pair through the production path and return
    /// the reloaded manifest.
    fn retain_minted_pair(
        root: &Path,
        finished: &RepairAttemptManifest,
        receipt_bytes: &[u8],
        verify_bytes: &[u8],
    ) -> Result<RepairAttemptManifest, String> {
        let reports = root.join("target/ripr/reports");
        std::fs::create_dir_all(&reports)
            .map_err(|error| format!("create {} failed: {error}", reports.display()))?;
        let receipt_source = reports.join("agent-receipt.json");
        let verify_source = root.join("target/ripr/workflow/agent-verify.json");
        std::fs::write(&receipt_source, receipt_bytes)
            .map_err(|error| format!("write receipt failed: {error}"))?;
        std::fs::write(&verify_source, verify_bytes)
            .map_err(|error| format!("write verify failed: {error}"))?;
        retain_terminal_evidence(
            root,
            &finished.repair_attempt_id,
            &[
                BeforeArtifactSource {
                    role: TERMINAL_RECEIPT_ROLE,
                    path: &receipt_source,
                },
                BeforeArtifactSource {
                    role: TERMINAL_VERIFY_ROLE,
                    path: &verify_source,
                },
            ],
        )?;
        load_repair_attempt_manifest(root, &finished.repair_attempt_id)
    }

    /// Rewrite one retained terminal artifact's bytes and rebind its manifest
    /// entry (bytes + sha256), as a forgery that also holds the manifest
    /// would. Returns the reloaded manifest.
    fn rewrite_retained_terminal_artifact(
        root: &Path,
        manifest: &RepairAttemptManifest,
        role: &str,
        bytes: &[u8],
    ) -> Result<RepairAttemptManifest, String> {
        let artifact_path = root.join(
            &find_terminal_artifact_by_role(manifest, role)
                .ok_or_else(|| format!("retained pair has no {role}"))?
                .path,
        );
        std::fs::write(&artifact_path, bytes)
            .map_err(|error| format!("write forged {role} failed: {error}"))?;
        let manifest_path = repair_attempt_directory(root, &manifest.repair_attempt_id)
            .join(REPAIR_ATTEMPT_MANIFEST);
        let manifest_raw = std::fs::read_to_string(&manifest_path)
            .map_err(|error| format!("read manifest failed: {error}"))?;
        let mut manifest_value: serde_json::Value = serde_json::from_str(&manifest_raw)
            .map_err(|error| format!("parse manifest failed: {error}"))?;
        let entry = manifest_value["terminal_artifacts"]
            .as_array_mut()
            .ok_or("manifest lost terminal_artifacts")?
            .iter_mut()
            .find(|artifact| artifact["role"] == role)
            .ok_or_else(|| format!("manifest lost its {role} entry"))?;
        entry["sha256"] = serde_json::Value::String(sha256_bytes(bytes));
        entry["bytes"] =
            serde_json::Value::from(u64::try_from(bytes.len()).map_err(|error| error.to_string())?);
        std::fs::write(
            &manifest_path,
            serde_json::to_vec_pretty(&manifest_value)
                .map_err(|error| format!("serialize rebound manifest failed: {error}"))?,
        )
        .map_err(|error| format!("write rebound manifest failed: {error}"))?;
        load_repair_attempt_manifest(root, &manifest.repair_attempt_id)
    }

    /// The CLI status disposition for one attempt, plus whether the
    /// unconfirmed-receipt warning fired.
    fn cli_disposition_for(root: &Path, attempt_id: &str) -> Result<(String, bool), String> {
        let report = crate::app::agent_status::build_agent_status_report_from(root, root, None);
        let attempt = report
            .repair_attempts
            .iter()
            .find(|attempt| attempt.attempt_id == attempt_id)
            .ok_or("status lost the attempt")?;
        let warned = report
            .warnings
            .iter()
            .any(|warning| warning.kind == "repair_receipt_unconfirmed");
        Ok((attempt.disposition.to_string(), warned))
    }

    fn bound_receipt_bytes_with_verify(
        manifest: &RepairAttemptManifest,
        verify_sha256: Option<&str>,
    ) -> Result<Vec<u8>, String> {
        let after = manifest
            .after
            .as_ref()
            .ok_or("finished sample has no after")?;
        let mut provenance = serde_json::json!({ "movement": "unchanged" });
        if let Some(sha256) = verify_sha256 {
            provenance["verify_artifact"] = serde_json::json!({
                "path": crate::agent::loop_commands::WORKFLOW_AGENT_VERIFY_ARTIFACT,
                "sha256": sha256
            });
        }
        let value = serde_json::json!({
            "status": "advisory",
            "provenance": provenance,
            "repair_attempt": {
                "attempt_id": after.attempt_id.as_str(),
                "after_head": after.repository_head,
                "delta_sha256": after.delta_sha256,
                "packet_sha256": after.packet_sha256
            }
        });
        serde_json::to_vec_pretty(&value)
            .map_err(|error| format!("serialize sample receipt failed: {error}"))
    }

    #[test]
    fn terminal_retention_survives_a_later_compatibility_receipt() -> Result<(), String> {
        let root = test_repo_root("retain-terminal")?;
        let prepared = prepare_sample_attempt(&root, "seam:sample", "a")?;
        let finished = finish_sample_attempt(&root, &prepared)?;
        let before_commitment = std::fs::read_to_string(
            repair_attempt_directory(&root, &finished.repair_attempt_id)
                .join(REPAIR_ATTEMPT_COMMITMENT),
        )
        .map_err(|error| format!("read before commitment failed: {error}"))?;

        let reports = root.join("target/ripr/reports");
        std::fs::create_dir_all(&reports)
            .map_err(|error| format!("create {} failed: {error}", reports.display()))?;
        let receipt_path = reports.join("agent-receipt.json");
        let verify_path = root.join("target/ripr/workflow/agent-verify.json");
        let (receipt_bytes, verify_bytes) = mint_bound_receipt_pair(&root, &finished, "unchanged")?;
        std::fs::write(&receipt_path, &receipt_bytes)
            .map_err(|error| format!("write receipt failed: {error}"))?;
        std::fs::write(&verify_path, &verify_bytes)
            .map_err(|error| format!("write verify failed: {error}"))?;

        retain_terminal_evidence(
            &root,
            &finished.repair_attempt_id,
            &[
                BeforeArtifactSource {
                    role: TERMINAL_RECEIPT_ROLE,
                    path: &receipt_path,
                },
                BeforeArtifactSource {
                    role: TERMINAL_VERIFY_ROLE,
                    path: &verify_path,
                },
            ],
        )?;
        let retained = load_repair_attempt_manifest(&root, &finished.repair_attempt_id)?;
        let after_commitment = std::fs::read_to_string(
            repair_attempt_directory(&root, &retained.repair_attempt_id)
                .join(REPAIR_ATTEMPT_COMMITMENT),
        )
        .map_err(|error| format!("reread before commitment failed: {error}"))?;
        if before_commitment != after_commitment {
            return Err("terminal retention rewrote the immutable before commitment".to_string());
        }
        match load_attempt_terminal_receipt(&root, &retained) {
            AttemptTerminalReceipt::Issued { path, value } => {
                if !path.contains(retained.repair_attempt_id.as_str()) {
                    return Err(format!("retained path left the attempt directory: {path}"));
                }
                if value["repair_attempt"]["attempt_id"] != retained.repair_attempt_id.as_str() {
                    return Err("retained receipt is not bound to this attempt".to_string());
                }
            }
            other => return Err(format!("expected issued local receipt, got {other:?}")),
        }

        std::fs::write(
            &receipt_path,
            b"{\"repair_attempt\":{\"attempt_id\":\"other\"}}\n",
        )
        .map_err(|error| format!("overwrite compatibility receipt failed: {error}"))?;
        match load_attempt_terminal_receipt(&root, &retained) {
            AttemptTerminalReceipt::Issued { .. } => {}
            other => {
                return Err(format!(
                    "rewriting the one-slot projection must not drop A's result: {other:?}"
                ));
            }
        }

        let local = root.join(
            find_terminal_artifact_by_role(&retained, TERMINAL_RECEIPT_ROLE)
                .ok_or("missing retained receipt")?
                .path
                .clone(),
        );
        std::fs::write(&local, b"tampered")
            .map_err(|error| format!("tamper local receipt failed: {error}"))?;
        match load_attempt_terminal_receipt(&root, &retained) {
            AttemptTerminalReceipt::Unavailable { .. } => {}
            other => {
                return Err(format!(
                    "a tampered local receipt must be unavailable, not {other:?}"
                ));
            }
        }

        std::fs::remove_dir_all(&root)
            .map_err(|error| format!("remove {} failed: {error}", root.display()))?;
        Ok(())
    }

    #[test]
    fn terminal_receipt_path_escape_is_unavailable() -> Result<(), String> {
        let root = test_repo_root("retain-escape")?;
        let prepared = prepare_sample_attempt(&root, "seam:sample", "escape")?;
        let finished = finish_sample_attempt(&root, &prepared)?;
        // Retain a complete pair through the production path; the escape
        // below is the only defect under test.
        let reports = root.join("target/ripr/reports");
        std::fs::create_dir_all(&reports)
            .map_err(|error| format!("create {} failed: {error}", reports.display()))?;
        let receipt_source = reports.join("agent-receipt.json");
        let verify_source = root.join("target/ripr/workflow/agent-verify.json");
        std::fs::write(&receipt_source, bound_receipt_bytes(&finished)?)
            .map_err(|error| format!("write receipt failed: {error}"))?;
        std::fs::write(&verify_source, b"{\"kind\":\"verify\"}\n")
            .map_err(|error| format!("write verify failed: {error}"))?;
        retain_terminal_evidence(
            &root,
            &finished.repair_attempt_id,
            &[
                BeforeArtifactSource {
                    role: TERMINAL_RECEIPT_ROLE,
                    path: &receipt_source,
                },
                BeforeArtifactSource {
                    role: TERMINAL_VERIFY_ROLE,
                    path: &verify_source,
                },
            ],
        )?;
        let mut retained = load_repair_attempt_manifest(&root, &finished.repair_attempt_id)?;
        // Point the retained receipt at a same-bytes file outside the attempt.
        let outside = reports.join("other-receipt.json");
        std::fs::write(&outside, bound_receipt_bytes(&finished)?)
            .map_err(|error| format!("write outside receipt failed: {error}"))?;
        let receipt = retained
            .terminal_artifacts
            .iter_mut()
            .find(|artifact| artifact.role == TERMINAL_RECEIPT_ROLE)
            .ok_or("retained pair has no agent_receipt")?;
        receipt.path = display_path(
            outside
                .strip_prefix(&root)
                .map_err(|error| format!("outside receipt is not under root: {error}"))?,
        );
        match load_attempt_terminal_receipt(&root, &retained) {
            AttemptTerminalReceipt::Unavailable { reason, .. }
                if reason.contains("escapes its attempt") => {}
            other => {
                return Err(format!(
                    "a path outside the attempt must be unavailable, not {other:?}"
                ));
            }
        }
        std::fs::remove_dir_all(&root)
            .map_err(|error| format!("remove {} failed: {error}", root.display()))?;
        Ok(())
    }

    #[test]
    fn terminal_receipt_without_verify_descriptor_is_unavailable() -> Result<(), String> {
        let root = test_repo_root("retain-no-verify-role")?;
        let prepared = prepare_sample_attempt(&root, "seam:sample", "no-verify-role")?;
        let finished = finish_sample_attempt(&root, &prepared)?;
        let reports = root.join("target/ripr/reports");
        std::fs::create_dir_all(&reports)
            .map_err(|error| format!("create {} failed: {error}", reports.display()))?;
        let receipt_source = reports.join("agent-receipt.json");
        let verify_source = root.join("target/ripr/workflow/agent-verify.json");
        let (receipt_bytes, verify_bytes) = mint_bound_receipt_pair(&root, &finished, "unchanged")?;
        std::fs::write(&receipt_source, &receipt_bytes)
            .map_err(|error| format!("write receipt failed: {error}"))?;
        std::fs::write(&verify_source, &verify_bytes)
            .map_err(|error| format!("write verify failed: {error}"))?;
        retain_terminal_evidence(
            &root,
            &finished.repair_attempt_id,
            &[
                BeforeArtifactSource {
                    role: TERMINAL_RECEIPT_ROLE,
                    path: &receipt_source,
                },
                BeforeArtifactSource {
                    role: TERMINAL_VERIFY_ROLE,
                    path: &verify_source,
                },
            ],
        )?;
        let mut retained = load_repair_attempt_manifest(&root, &finished.repair_attempt_id)?;
        match load_attempt_terminal_receipt(&root, &retained) {
            AttemptTerminalReceipt::Issued { .. } => {}
            other => return Err(format!("the retained pair must read issued: {other:?}")),
        }
        // The before commitment excludes `terminal_artifacts`, so dropping the
        // verify descriptor models a manifest that lost its verify role; the
        // surviving receipt alone must not read `finished_*`.
        retained
            .terminal_artifacts
            .retain(|artifact| artifact.role != TERMINAL_VERIFY_ROLE);
        match load_attempt_terminal_receipt(&root, &retained) {
            AttemptTerminalReceipt::Unavailable { reason, .. }
                if reason.contains("no agent_verify") => {}
            other => {
                return Err(format!(
                    "a receipt without its verify role must be unavailable, not {other:?}"
                ));
            }
        }
        std::fs::remove_dir_all(&root)
            .map_err(|error| format!("remove {} failed: {error}", root.display()))?;
        Ok(())
    }

    #[test]
    fn pending_terminal_retention_completes_from_a_matching_projection() -> Result<(), String> {
        let root = test_repo_root("retain-pending")?;
        let prepared = prepare_sample_attempt(&root, "seam:sample", "pending")?;
        let finished = finish_sample_attempt(&root, &prepared)?;
        // A qualifying projection: the canonical verify render over the
        // retained before snapshot and a descendant-head after snapshot, plus
        // a verdict-consistent receipt. Commit the finished test edit first
        // so the after snapshot's head descends from the before head.
        run_git(&root, &["add", "tests/target.rs"])?;
        run_git(&root, &["commit", "--no-gpg-sign", "-m", "focused test"])?;
        let after_path = root.join("target/ripr/workflow/after.json");
        let after_snapshot = crate::testing::verify_fixture::mint_repo_exposure_snapshot(
            root.as_path(),
            serde_json::json!([crate::testing::verify_fixture::snapshot_seam(
                "seam:sample",
                "predicate_boundary",
                "src/lib.rs",
                1,
                "strongly_gripped",
            )]),
        )?;
        std::fs::write(&after_path, after_snapshot.as_bytes())
            .map_err(|error| format!("write {} failed: {error}", after_path.display()))?;
        let retained_before =
            root.join(&find_manifest_artifact(&finished, "before_snapshot")?.path);
        let verify = crate::testing::verify_fixture::mint_canonical_verify(
            root.as_path(),
            &retained_before,
            &after_path,
        )?;
        let verify_value: serde_json::Value = serde_json::from_str(&verify)
            .map_err(|error| format!("parse canonical verify failed: {error}"))?;
        let movement = verify_value["changed_seams"][0]["change"]
            .as_str()
            .ok_or("canonical verify lost its seam movement")?;
        if movement != "improved" {
            return Err(format!(
                "canonical verify must record improved movement, got {movement}"
            ));
        }
        let reports = root.join("target/ripr/reports");
        std::fs::create_dir_all(&reports)
            .map_err(|error| format!("create {} failed: {error}", reports.display()))?;
        let verify_path = root.join("target/ripr/workflow/agent-verify.json");
        std::fs::write(&verify_path, verify.as_bytes())
            .map_err(|error| format!("write verify failed: {error}"))?;
        let verify_bytes =
            std::fs::read(&verify_path).map_err(|error| format!("read verify failed: {error}"))?;
        std::fs::write(
            reports.join("agent-receipt.json"),
            mint_bound_receipt(&finished, movement, &sha256_bytes(&verify_bytes))?,
        )
        .map_err(|error| format!("write receipt failed: {error}"))?;

        if !complete_pending_terminal_retention(&root, finished.repair_attempt_id.as_str())? {
            return Err("matching compatibility receipt should complete retention".to_string());
        }
        let retained = load_repair_attempt_manifest(&root, &finished.repair_attempt_id)?;
        match load_attempt_terminal_receipt(&root, &retained) {
            AttemptTerminalReceipt::Issued { .. } => {}
            other => return Err(format!("pending completion did not retain: {other:?}")),
        }
        if complete_pending_terminal_retention(&root, finished.repair_attempt_id.as_str())? {
            return Err("a second completion must be a no-op".to_string());
        }

        std::fs::remove_dir_all(&root)
            .map_err(|error| format!("remove {} failed: {error}", root.display()))?;
        Ok(())
    }

    #[test]
    fn pending_terminal_retention_refuses_a_replaced_verify_projection() -> Result<(), String> {
        let root = test_repo_root("retain-pending-verify")?;
        let prepared = prepare_sample_attempt(&root, "seam:sample", "pending-verify")?;
        let finished = finish_sample_attempt(&root, &prepared)?;
        let reports = root.join("target/ripr/reports");
        std::fs::create_dir_all(&reports)
            .map_err(|error| format!("create {} failed: {error}", reports.display()))?;
        let original_verify = b"{\"kind\":\"verify\",\"attempt\":\"a\"}\n";
        std::fs::write(
            reports.join("agent-receipt.json"),
            bound_receipt_bytes_with_verify(&finished, Some(&sha256_bytes(original_verify)))?,
        )
        .map_err(|error| format!("write receipt failed: {error}"))?;
        std::fs::write(
            root.join("target/ripr/workflow/agent-verify.json"),
            b"{\"kind\":\"verify\",\"attempt\":\"b\"}\n",
        )
        .map_err(|error| format!("write replaced verify failed: {error}"))?;

        if complete_pending_terminal_retention(&root, finished.repair_attempt_id.as_str())? {
            return Err(
                "a replaced verify projection must not be retained as A's evidence".to_string(),
            );
        }
        let manifest = load_repair_attempt_manifest(&root, &finished.repair_attempt_id)?;
        if !manifest.terminal_artifacts.is_empty() {
            return Err("pending completion must leave terminal_artifacts empty".to_string());
        }

        std::fs::remove_dir_all(&root)
            .map_err(|error| format!("remove {} failed: {error}", root.display()))?;
        Ok(())
    }

    /// #5256 F1: flipping the terminal receipt's verdict strings
    /// (`summary.receipt_state`, `summary.next_action.kind`, `seam.change`)
    /// and rebinding the manifest digest must read as a binding failure —
    /// never `finished` — with an `unconfirmed` disposition and warning.
    /// The forgery leaves `/provenance/movement` behind exactly as filed, so
    /// the receipt is also internally inconsistent.
    #[test]
    fn terminal_receipt_forged_verdict_never_reports_finished() -> Result<(), String> {
        let root = test_repo_root("forged-verdict")?;
        let prepared = prepare_sample_attempt(&root, "seam:sample", "forged")?;
        let finished = finish_sample_attempt(&root, &prepared)?;
        // Honest gap-open pair first: the positive control reads issued.
        let (receipt_bytes, verify_bytes) = mint_bound_receipt_pair(&root, &finished, "unchanged")?;
        let reports = root.join("target/ripr/reports");
        std::fs::create_dir_all(&reports)
            .map_err(|error| format!("create {} failed: {error}", reports.display()))?;
        let receipt_source = reports.join("agent-receipt.json");
        let verify_source = root.join("target/ripr/workflow/agent-verify.json");
        std::fs::write(&receipt_source, &receipt_bytes)
            .map_err(|error| format!("write receipt failed: {error}"))?;
        std::fs::write(&verify_source, &verify_bytes)
            .map_err(|error| format!("write verify failed: {error}"))?;
        retain_terminal_evidence(
            &root,
            &finished.repair_attempt_id,
            &[
                BeforeArtifactSource {
                    role: TERMINAL_RECEIPT_ROLE,
                    path: &receipt_source,
                },
                BeforeArtifactSource {
                    role: TERMINAL_VERIFY_ROLE,
                    path: &verify_source,
                },
            ],
        )?;
        let retained = load_repair_attempt_manifest(&root, &finished.repair_attempt_id)?;
        match load_attempt_terminal_receipt(&root, &retained) {
            AttemptTerminalReceipt::Issued { .. } => {}
            other => return Err(format!("honest pair must read issued, got {other:?}")),
        }
        // Forge the retained receipt exactly as filed, then rebind the
        // manifest entry (bytes + sha256). The before commitment excludes
        // `terminal_artifacts`, so the manifest itself stays valid.
        let receipt_path = root.join(
            &find_terminal_artifact_by_role(&retained, TERMINAL_RECEIPT_ROLE)
                .ok_or("retained pair has no agent_receipt")?
                .path,
        );
        let raw = std::fs::read(&receipt_path)
            .map_err(|error| format!("read retained receipt failed: {error}"))?;
        let mut forged: serde_json::Value = serde_json::from_slice(&raw)
            .map_err(|error| format!("parse retained receipt failed: {error}"))?;
        forged["summary"]["receipt_state"] =
            serde_json::Value::String("receipt_movement_improved".to_string());
        forged["summary"]["next_action"]["kind"] =
            serde_json::Value::String("improved".to_string());
        forged["seam"]["change"] = serde_json::Value::String("improved".to_string());
        if forged["provenance"]["movement"] != "unchanged" {
            return Err("forgery precondition lost its unchanged movement".to_string());
        }
        let mut forged_bytes = serde_json::to_vec_pretty(&forged)
            .map_err(|error| format!("serialize forged receipt failed: {error}"))?;
        forged_bytes.push(b'\n');
        let forged_manifest = rewrite_retained_terminal_artifact(
            &root,
            &retained,
            TERMINAL_RECEIPT_ROLE,
            &forged_bytes,
        )?;
        match load_attempt_terminal_receipt(&root, &forged_manifest) {
            AttemptTerminalReceipt::Unavailable { reason, .. } if reason.contains("verdict") => {}
            other => {
                return Err(format!(
                    "a forged verdict must fail verdict binding, got {other:?}"
                ));
            }
        }
        let report = crate::app::agent_status::build_agent_status_report_from(&root, &root, None);
        let attempt = report
            .repair_attempts
            .iter()
            .find(|attempt| attempt.attempt_id == forged_manifest.repair_attempt_id.as_str())
            .ok_or("status lost the forged attempt")?;
        if attempt.disposition == "finished" {
            return Err("a forged verdict must never report finished".to_string());
        }
        if attempt.disposition != "unconfirmed" {
            return Err(format!(
                "a forged verdict must read unconfirmed, got {}",
                attempt.disposition
            ));
        }
        if !report
            .warnings
            .iter()
            .any(|warning| warning.kind == "repair_receipt_unconfirmed")
        {
            return Err("a forged verdict must warn it is unconfirmed".to_string());
        }
        std::fs::remove_dir_all(&root)
            .map_err(|error| format!("remove {} failed: {error}", root.display()))?;
        Ok(())
    }

    /// #5256 F2: planted workflow receipt+verify files that are 4-field bound
    /// and digest-consistent — but verdict-inconsistent and non-canonical —
    /// must not be promoted by pending retention, and status must not read
    /// them as finished through the legacy fallback either.
    #[test]
    fn pending_terminal_retention_refuses_planted_projection() -> Result<(), String> {
        let root = test_repo_root("planted-retention")?;
        let prepared = prepare_sample_attempt(&root, "seam:sample", "planted")?;
        let finished = finish_sample_attempt(&root, &prepared)?;
        // The plant passes the old checks: the receipt is 4-field bound and
        // records the planted verify's digest. It claims improved while the
        // verify records unchanged, and the hand-shaped verify is not the
        // canonical render of the named snapshots.
        let (_, verify_bytes) = mint_bound_receipt_pair(&root, &finished, "unchanged")?;
        let (receipt_bytes, _) = mint_bound_receipt_pair(&root, &finished, "improved")?;
        let mut planted: serde_json::Value = serde_json::from_slice(&receipt_bytes)
            .map_err(|error| format!("parse planted receipt failed: {error}"))?;
        planted["provenance"]["verify_artifact"]["sha256"] =
            serde_json::Value::String(sha256_bytes(&verify_bytes));
        let mut planted_bytes = serde_json::to_vec_pretty(&planted)
            .map_err(|error| format!("serialize planted receipt failed: {error}"))?;
        planted_bytes.push(b'\n');
        let reports = root.join("target/ripr/reports");
        std::fs::create_dir_all(&reports)
            .map_err(|error| format!("create {} failed: {error}", reports.display()))?;
        std::fs::write(reports.join("agent-receipt.json"), &planted_bytes)
            .map_err(|error| format!("plant receipt failed: {error}"))?;
        std::fs::write(
            root.join("target/ripr/workflow/agent-verify.json"),
            &verify_bytes,
        )
        .map_err(|error| format!("plant verify failed: {error}"))?;
        if complete_pending_terminal_retention(&root, finished.repair_attempt_id.as_str())? {
            return Err("a planted projection must not complete retention".to_string());
        }
        let manifest = load_repair_attempt_manifest(&root, &finished.repair_attempt_id)?;
        if !manifest.terminal_artifacts.is_empty() {
            return Err("a refused plant must leave terminal_artifacts empty".to_string());
        }
        let report = crate::app::agent_status::build_agent_status_report_from(&root, &root, None);
        let attempt = report
            .repair_attempts
            .iter()
            .find(|attempt| attempt.attempt_id == finished.repair_attempt_id.as_str())
            .ok_or("status lost the planted attempt")?;
        if attempt.disposition == "finished" {
            return Err("a planted projection must never report finished".to_string());
        }
        if attempt.disposition != "unconfirmed" {
            return Err(format!(
                "a planted projection must read unconfirmed, got {}",
                attempt.disposition
            ));
        }
        std::fs::remove_dir_all(&root)
            .map_err(|error| format!("remove {} failed: {error}", root.display()))?;
        Ok(())
    }

    /// Pending retention runs the full canonical receipt-issuance validation,
    /// not just binding and consistency: a verdict-consistent pair that is
    /// not the canonical render of the named snapshots must still be refused.
    /// This isolates the canonical gate — the consistency gate alone would
    /// admit this pair. The legacy status read refuses it too, through the
    /// same canonical admission over the still-present snapshots.
    #[test]
    fn pending_terminal_retention_requires_canonical_verify() -> Result<(), String> {
        let root = test_repo_root("retention-canonical")?;
        let prepared = prepare_sample_attempt(&root, "seam:sample", "canonical")?;
        let finished = finish_sample_attempt(&root, &prepared)?;
        // Fully consistent improved pair, but hand-shaped: no canonical
        // render of the named snapshots can match it.
        let (receipt_bytes, verify_bytes) = mint_bound_receipt_pair(&root, &finished, "improved")?;
        let reports = root.join("target/ripr/reports");
        std::fs::create_dir_all(&reports)
            .map_err(|error| format!("create {} failed: {error}", reports.display()))?;
        std::fs::write(reports.join("agent-receipt.json"), &receipt_bytes)
            .map_err(|error| format!("write receipt failed: {error}"))?;
        std::fs::write(
            root.join("target/ripr/workflow/agent-verify.json"),
            &verify_bytes,
        )
        .map_err(|error| format!("write verify failed: {error}"))?;
        // The snapshots are present, so readers can evaluate canonical
        // admission against them.
        let after_path = root.join("target/ripr/workflow/after.json");
        let after_snapshot = crate::testing::verify_fixture::mint_repo_exposure_snapshot(
            root.as_path(),
            serde_json::json!([crate::testing::verify_fixture::snapshot_seam(
                "seam:sample",
                "predicate_boundary",
                "src/lib.rs",
                1,
                "strongly_gripped",
            )]),
        )?;
        std::fs::write(&after_path, after_snapshot.as_bytes())
            .map_err(|error| format!("write {} failed: {error}", after_path.display()))?;
        if complete_pending_terminal_retention(&root, finished.repair_attempt_id.as_str())? {
            return Err("a non-canonical projection must not complete retention".to_string());
        }
        let manifest = load_repair_attempt_manifest(&root, &finished.repair_attempt_id)?;
        if !manifest.terminal_artifacts.is_empty() {
            return Err("a refused projection must leave terminal_artifacts empty".to_string());
        }
        match canonically_admit_terminal_pair(&root, &verify_bytes) {
            CanonicalAdmission::Refused { .. } => {}
            other => {
                return Err(format!(
                    "a non-canonical projection must be refused admission, got {other:?}"
                ));
            }
        }
        let (disposition, warned) =
            cli_disposition_for(&root, finished.repair_attempt_id.as_str())?;
        if disposition == "finished" {
            return Err("a non-canonical projection must never report finished".to_string());
        }
        if disposition != "unconfirmed" {
            return Err(format!(
                "a refused non-canonical projection must read unconfirmed, got {disposition}"
            ));
        }
        if !warned {
            return Err(
                "a refused non-canonical projection must warn it is unconfirmed".to_string(),
            );
        }
        std::fs::remove_dir_all(&root)
            .map_err(|error| format!("remove {} failed: {error}", root.display()))?;
        Ok(())
    }

    /// #5256 review R1: a retained receipt rewritten to another seam — with
    /// the same movement and a rebound manifest digest — must fail seam
    /// binding, never report finished. Each rendered seam field binds
    /// independently.
    #[test]
    fn terminal_receipt_wrong_seam_never_reports_finished() -> Result<(), String> {
        let root = test_repo_root("wrong-seam")?;
        let prepared = prepare_sample_attempt(&root, "seam:sample", "wrong-seam")?;
        let finished = finish_sample_attempt(&root, &prepared)?;
        let (receipt_bytes, verify_bytes) = mint_bound_receipt_pair(&root, &finished, "improved")?;
        let retained = retain_minted_pair(&root, &finished, &receipt_bytes, &verify_bytes)?;
        match load_attempt_terminal_receipt(&root, &retained) {
            AttemptTerminalReceipt::Issued { .. } => {}
            other => return Err(format!("honest pair must read issued, got {other:?}")),
        }
        // Each seam field binds independently: breaking either one alone
        // fails the shared validator while the other still matches.
        let honest: serde_json::Value = serde_json::from_slice(&receipt_bytes)
            .map_err(|error| format!("parse honest receipt failed: {error}"))?;
        for pointer in ["/seam/seam_id", "/provenance/seam_id"] {
            let mut broken = honest.clone();
            let slot = broken
                .pointer_mut(pointer)
                .ok_or_else(|| format!("honest receipt lost {pointer}"))?;
            *slot = serde_json::Value::String("seam:other".to_string());
            match validate_issued_receipt_evidence(&root, &finished, &broken, &verify_bytes) {
                Err(reason) if reason.contains("is not the attempt seam") => {}
                other => {
                    return Err(format!(
                        "breaking {pointer} alone must fail seam binding, got {other:?}"
                    ));
                }
            }
        }
        let raw = std::fs::read(
            root.join(
                &find_terminal_artifact_by_role(&retained, TERMINAL_RECEIPT_ROLE)
                    .ok_or("retained pair has no agent_receipt")?
                    .path,
            ),
        )
        .map_err(|error| format!("read retained receipt failed: {error}"))?;
        let mut forged: serde_json::Value = serde_json::from_slice(&raw)
            .map_err(|error| format!("parse retained receipt failed: {error}"))?;
        // Same movement, another seam: the verdict checks alone would pass.
        forged["seam"]["seam_id"] = serde_json::Value::String("seam:other".to_string());
        forged["provenance"]["seam_id"] = serde_json::Value::String("seam:other".to_string());
        let mut forged_bytes = serde_json::to_vec_pretty(&forged)
            .map_err(|error| format!("serialize forged receipt failed: {error}"))?;
        forged_bytes.push(b'\n');
        let forged_manifest = rewrite_retained_terminal_artifact(
            &root,
            &retained,
            TERMINAL_RECEIPT_ROLE,
            &forged_bytes,
        )?;
        match load_attempt_terminal_receipt(&root, &forged_manifest) {
            AttemptTerminalReceipt::Unavailable { reason, .. }
                if reason.contains("is not the attempt seam") => {}
            other => {
                return Err(format!(
                    "a wrong-seam receipt must fail seam binding, got {other:?}"
                ));
            }
        }
        let (disposition, warned) =
            cli_disposition_for(&root, forged_manifest.repair_attempt_id.as_str())?;
        if disposition == "finished" {
            return Err("a wrong-seam receipt must never report finished".to_string());
        }
        if disposition != "unconfirmed" {
            return Err(format!(
                "a wrong-seam receipt must read unconfirmed, got {disposition}"
            ));
        }
        if !warned {
            return Err("a wrong-seam receipt must warn it is unconfirmed".to_string());
        }
        std::fs::remove_dir_all(&root)
            .map_err(|error| format!("remove {} failed: {error}", root.display()))?;
        Ok(())
    }

    /// #5256 review R1 (legacy path): a workflow receipt rewritten to another
    /// seam — digest-consistent with its verify document — must read
    /// unconfirmed through the legacy fallback, never finished, and must not
    /// be promoted.
    #[test]
    fn legacy_workflow_receipt_wrong_seam_is_unconfirmed() -> Result<(), String> {
        let root = test_repo_root("wrong-seam-legacy")?;
        let prepared = prepare_sample_attempt(&root, "seam:sample", "wrong-seam")?;
        let finished = finish_sample_attempt(&root, &prepared)?;
        let (receipt_bytes, verify_bytes) = mint_bound_receipt_pair(&root, &finished, "improved")?;
        let mut planted: serde_json::Value = serde_json::from_slice(&receipt_bytes)
            .map_err(|error| format!("parse planted receipt failed: {error}"))?;
        planted["seam"]["seam_id"] = serde_json::Value::String("seam:other".to_string());
        planted["provenance"]["seam_id"] = serde_json::Value::String("seam:other".to_string());
        // The verify digest still matches: only the seam binding is broken.
        let mut planted_bytes = serde_json::to_vec_pretty(&planted)
            .map_err(|error| format!("serialize planted receipt failed: {error}"))?;
        planted_bytes.push(b'\n');
        let reports = root.join("target/ripr/reports");
        std::fs::create_dir_all(&reports)
            .map_err(|error| format!("create {} failed: {error}", reports.display()))?;
        std::fs::write(reports.join("agent-receipt.json"), &planted_bytes)
            .map_err(|error| format!("plant receipt failed: {error}"))?;
        std::fs::write(
            root.join("target/ripr/workflow/agent-verify.json"),
            &verify_bytes,
        )
        .map_err(|error| format!("plant verify failed: {error}"))?;
        if complete_pending_terminal_retention(&root, finished.repair_attempt_id.as_str())? {
            return Err("a wrong-seam projection must not complete retention".to_string());
        }
        let (disposition, warned) =
            cli_disposition_for(&root, finished.repair_attempt_id.as_str())?;
        if disposition == "finished" {
            return Err("a wrong-seam projection must never report finished".to_string());
        }
        if disposition != "unconfirmed" {
            return Err(format!(
                "a wrong-seam projection must read unconfirmed, got {disposition}"
            ));
        }
        if !warned {
            return Err("a wrong-seam projection must warn it is unconfirmed".to_string());
        }
        std::fs::remove_dir_all(&root)
            .map_err(|error| format!("remove {} failed: {error}", root.display()))?;
        Ok(())
    }

    /// #5256 review R5: a retained receipt whose status says `advisory` while
    /// its own recorded analysis-outcome projection still says incomplete is
    /// a status-only forgery: it must fail status binding, never finished. A
    /// consistent incomplete pair stays issued as the control.
    #[test]
    fn terminal_receipt_status_flip_never_reports_finished() -> Result<(), String> {
        let root = test_repo_root("status-flip")?;
        let prepared = prepare_sample_attempt(&root, "seam:sample", "status-flip")?;
        let finished = finish_sample_attempt(&root, &prepared)?;
        let (receipt_bytes, verify_bytes) = mint_bound_receipt_pair(&root, &finished, "improved")?;
        let retained = retain_minted_pair(&root, &finished, &receipt_bytes, &verify_bytes)?;
        match load_attempt_terminal_receipt(&root, &retained) {
            AttemptTerminalReceipt::Issued { .. } => {}
            other => return Err(format!("honest pair must read issued, got {other:?}")),
        }
        // Consistent incomplete control: status and projection agree, so the
        // pair stays issued (the gap simply does not read closed).
        let mut control: serde_json::Value = serde_json::from_slice(&receipt_bytes)
            .map_err(|error| format!("parse retained receipt failed: {error}"))?;
        control["status"] = serde_json::Value::String("incomplete".to_string());
        control["analysis_outcome_status"] = serde_json::Value::String("incomplete".to_string());
        let mut control_bytes = serde_json::to_vec_pretty(&control)
            .map_err(|error| format!("serialize control receipt failed: {error}"))?;
        control_bytes.push(b'\n');
        let control_manifest = rewrite_retained_terminal_artifact(
            &root,
            &retained,
            TERMINAL_RECEIPT_ROLE,
            &control_bytes,
        )?;
        match load_attempt_terminal_receipt(&root, &control_manifest) {
            AttemptTerminalReceipt::Issued { .. } => {}
            other => {
                return Err(format!(
                    "a consistent incomplete receipt must stay issued, got {other:?}"
                ));
            }
        }
        // The attack: the projection says incomplete, the status says advisory.
        let mut forged: serde_json::Value = serde_json::from_slice(&control_bytes)
            .map_err(|error| format!("parse control receipt failed: {error}"))?;
        forged["status"] = serde_json::Value::String("advisory".to_string());
        let mut forged_bytes = serde_json::to_vec_pretty(&forged)
            .map_err(|error| format!("serialize forged receipt failed: {error}"))?;
        forged_bytes.push(b'\n');
        let forged_manifest = rewrite_retained_terminal_artifact(
            &root,
            &control_manifest,
            TERMINAL_RECEIPT_ROLE,
            &forged_bytes,
        )?;
        match load_attempt_terminal_receipt(&root, &forged_manifest) {
            AttemptTerminalReceipt::Unavailable { reason, .. }
                if reason.contains("does not match its recorded analysis outcome status") => {}
            other => {
                return Err(format!(
                    "a status-flipped receipt must fail status binding, got {other:?}"
                ));
            }
        }
        let (disposition, warned) =
            cli_disposition_for(&root, forged_manifest.repair_attempt_id.as_str())?;
        if disposition == "finished" {
            return Err("a status-flipped receipt must never report finished".to_string());
        }
        if disposition != "unconfirmed" {
            return Err(format!(
                "a status-flipped receipt must read unconfirmed, got {disposition}"
            ));
        }
        if !warned {
            return Err("a status-flipped receipt must warn it is unconfirmed".to_string());
        }
        std::fs::remove_dir_all(&root)
            .map_err(|error| format!("remove {} failed: {error}", root.display()))?;
        Ok(())
    }

    /// #5256 review R6+R7: a joint rewrite of the verify document (seam
    /// change) and receipt (verdict fields), with the receipt's verify
    /// digest rebound, passes digest-plus-verdict validation — but the
    /// rewritten verify is not the canonical render of the still-present
    /// snapshots, so pending promotion must refuse it and readers must
    /// stay unconfirmed. Retained reads are validator-only by design: a
    /// retained pair claims a historical basis the live snapshots
    /// legitimately advance past, so admission is enforced at the
    /// promotion boundary, not at retained read.
    #[test]
    fn terminal_pair_joint_forgery_with_present_snapshots_is_refused() -> Result<(), String> {
        let root = test_repo_root("joint-forgery")?;
        let prepared = prepare_sample_attempt(&root, "seam:sample", "joint")?;
        let finished = finish_sample_attempt(&root, &prepared)?;
        // Honest canonical unchanged pair: commit the focused test so the
        // after snapshot descends, then mint through the production render.
        run_git(&root, &["add", "tests/target.rs"])?;
        run_git(&root, &["commit", "--no-gpg-sign", "-m", "focused test"])?;
        let after_path = root.join("target/ripr/workflow/after.json");
        let after_snapshot = crate::testing::verify_fixture::mint_repo_exposure_snapshot(
            root.as_path(),
            serde_json::json!([crate::testing::verify_fixture::snapshot_seam(
                "seam:sample",
                "predicate_boundary",
                "src/lib.rs",
                1,
                "weakly_gripped",
            )]),
        )?;
        std::fs::write(&after_path, after_snapshot.as_bytes())
            .map_err(|error| format!("write {} failed: {error}", after_path.display()))?;
        let retained_before =
            root.join(&find_manifest_artifact(&finished, "before_snapshot")?.path);
        let verify = crate::testing::verify_fixture::mint_canonical_verify(
            root.as_path(),
            &retained_before,
            &after_path,
        )?;
        let verify_value: serde_json::Value = serde_json::from_str(&verify)
            .map_err(|error| format!("parse canonical verify failed: {error}"))?;
        let movement = verify_value["unchanged_seams"][0]["change"]
            .as_str()
            .ok_or("canonical verify lost its seam movement")?;
        if movement != "unchanged" {
            return Err(format!(
                "canonical verify must record unchanged movement, got {movement}"
            ));
        }
        let verify_bytes = verify.into_bytes();
        let receipt_bytes =
            mint_bound_receipt(&finished, "unchanged", &sha256_bytes(&verify_bytes))?;
        // Positive control: the honest canonical pair is admitted. The
        // attempt stays unretained so the forged projection below can drive
        // pending promotion (promotion is a no-op once terminal artifacts
        // exist, which would make the refusal assertion vacuous).
        match canonically_admit_terminal_pair(&root, &verify_bytes) {
            CanonicalAdmission::Admitted => {}
            other => return Err(format!("honest pair must be admitted, got {other:?}")),
        }
        // Joint rewrite: the verify seam change and every receipt verdict
        // field move to improved together, with the digest and both manifest
        // entries rebound. Digest-plus-verdict validation passes by
        // construction; only canonical admission can refuse it.
        // The joint rewrite lands as a never-promoted pending projection
        // in the compatibility files, not as retained bytes.
        let mut forged_verify: serde_json::Value = serde_json::from_slice(&verify_bytes)
            .map_err(|error| format!("parse honest verify failed: {error}"))?;
        let unchanged = forged_verify["unchanged_seams"]
            .as_array_mut()
            .ok_or("retained verify lost unchanged_seams")?
            .pop()
            .ok_or("retained verify lost its seam row")?;
        let mut improved_row = unchanged;
        improved_row["change"] = serde_json::Value::String("improved".to_string());
        improved_row["after"] = serde_json::Value::String("strongly_gripped".to_string());
        forged_verify["changed_seams"]
            .as_array_mut()
            .ok_or("retained verify lost changed_seams")?
            .push(improved_row);
        forged_verify["summary"]["unchanged"] = serde_json::json!(0);
        forged_verify["summary"]["improved"] = serde_json::json!(1);
        let mut forged_verify_bytes = serde_json::to_vec_pretty(&forged_verify)
            .map_err(|error| format!("serialize forged verify failed: {error}"))?;
        forged_verify_bytes.push(b'\n');
        let mut forged_receipt: serde_json::Value = serde_json::from_slice(&receipt_bytes)
            .map_err(|error| format!("parse honest receipt failed: {error}"))?;
        forged_receipt["seam"]["change"] = serde_json::Value::String("improved".to_string());
        forged_receipt["provenance"]["movement"] =
            serde_json::Value::String("improved".to_string());
        forged_receipt["summary"]["receipt_state"] =
            serde_json::Value::String("receipt_movement_improved".to_string());
        forged_receipt["summary"]["next_action"]["kind"] =
            serde_json::Value::String("improved".to_string());
        forged_receipt["provenance"]["verify_artifact"]["sha256"] =
            serde_json::Value::String(sha256_bytes(&forged_verify_bytes));
        let mut forged_receipt_bytes = serde_json::to_vec_pretty(&forged_receipt)
            .map_err(|error| format!("serialize forged receipt failed: {error}"))?;
        forged_receipt_bytes.push(b'\n');
        // The validator alone passes: prove the joint rewrite is consistent.
        let forged_value: serde_json::Value = serde_json::from_slice(&forged_receipt_bytes)
            .map_err(|error| format!("parse forged receipt failed: {error}"))?;
        validate_issued_receipt_evidence(&root, &finished, &forged_value, &forged_verify_bytes)
            .map_err(|error| format!("joint forgery must pass the validator: {error}"))?;
        // ...but canonical admission refuses it, so pending promotion must
        // refuse it and readers stay unconfirmed.
        match canonically_admit_terminal_pair(&root, &forged_verify_bytes) {
            CanonicalAdmission::Refused { .. } => {}
            other => {
                return Err(format!(
                    "joint forgery must be refused admission, got {other:?}"
                ));
            }
        }
        let reports = root.join("target/ripr/reports");
        std::fs::create_dir_all(&reports)
            .map_err(|error| format!("create {} failed: {error}", reports.display()))?;
        std::fs::write(reports.join("agent-receipt.json"), &forged_receipt_bytes)
            .map_err(|error| format!("write forged receipt failed: {error}"))?;
        std::fs::write(
            root.join("target/ripr/workflow/agent-verify.json"),
            &forged_verify_bytes,
        )
        .map_err(|error| format!("write forged verify failed: {error}"))?;
        if complete_pending_terminal_retention(&root, finished.repair_attempt_id.as_str())? {
            return Err("a jointly forged projection must not complete retention".to_string());
        }
        let manifest = load_repair_attempt_manifest(&root, &finished.repair_attempt_id)?;
        if !manifest.terminal_artifacts.is_empty() {
            return Err("a refused projection must leave terminal_artifacts empty".to_string());
        }
        let (disposition, warned) =
            cli_disposition_for(&root, finished.repair_attempt_id.as_str())?;
        if disposition == "finished" {
            return Err("a jointly forged pair must never report finished".to_string());
        }
        if disposition != "unconfirmed" {
            return Err(format!(
                "a jointly forged pair must read unconfirmed, got {disposition}"
            ));
        }
        if !warned {
            return Err("a jointly forged pair must warn it is unconfirmed".to_string());
        }
        std::fs::remove_dir_all(&root)
            .map_err(|error| format!("remove {} failed: {error}", root.display()))?;
        Ok(())
    }

    /// #5256 CI regression: a retained honest pair keeps its reading after
    /// a later attempt advances the live shared snapshots. Canonical
    /// admission is enforced at promotion, never at retained read, so the
    /// earlier attempt still reports its retained outcome once its
    /// historical basis is no longer live.
    #[test]
    fn retained_honest_pair_survives_later_snapshot_advance() -> Result<(), String> {
        let root = test_repo_root("retained-advance")?;
        let prepared = prepare_sample_attempt(&root, "seam:sample", "advance")?;
        let finished = finish_sample_attempt(&root, &prepared)?;
        run_git(&root, &["add", "tests/target.rs"])?;
        run_git(&root, &["commit", "--no-gpg-sign", "-m", "focused test"])?;
        let after_path = root.join("target/ripr/workflow/after.json");
        let after_snapshot = crate::testing::verify_fixture::mint_repo_exposure_snapshot(
            root.as_path(),
            serde_json::json!([crate::testing::verify_fixture::snapshot_seam(
                "seam:sample",
                "predicate_boundary",
                "src/lib.rs",
                1,
                "weakly_gripped",
            )]),
        )?;
        std::fs::write(&after_path, after_snapshot.as_bytes())
            .map_err(|error| format!("write {} failed: {error}", after_path.display()))?;
        let retained_before =
            root.join(&find_manifest_artifact(&finished, "before_snapshot")?.path);
        let verify = crate::testing::verify_fixture::mint_canonical_verify(
            root.as_path(),
            &retained_before,
            &after_path,
        )?;
        let verify_bytes = verify.into_bytes();
        let receipt_bytes =
            mint_bound_receipt(&finished, "unchanged", &sha256_bytes(&verify_bytes))?;
        let retained = retain_minted_pair(&root, &finished, &receipt_bytes, &verify_bytes)?;
        match load_attempt_terminal_receipt(&root, &retained) {
            AttemptTerminalReceipt::Issued { .. } => {}
            other => return Err(format!("honest pair must read issued, got {other:?}")),
        }
        let (first_disposition, _) =
            cli_disposition_for(&root, finished.repair_attempt_id.as_str())?;
        // A later attempt advances the live shared after snapshot. The
        // retained pair's basis is historical; the advance must not
        // un-issue it.
        let advanced = crate::testing::verify_fixture::mint_repo_exposure_snapshot(
            root.as_path(),
            serde_json::json!([crate::testing::verify_fixture::snapshot_seam(
                "seam:sample",
                "predicate_boundary",
                "src/lib.rs",
                1,
                "strongly_gripped",
            )]),
        )?;
        std::fs::write(&after_path, advanced.as_bytes())
            .map_err(|error| format!("advance {} failed: {error}", after_path.display()))?;
        match load_attempt_terminal_receipt(&root, &retained) {
            AttemptTerminalReceipt::Issued { .. } => {}
            other => {
                return Err(format!(
                    "honest pair must stay issued after the snapshots advance, got {other:?}"
                ));
            }
        }
        let (later_disposition, _) =
            cli_disposition_for(&root, finished.repair_attempt_id.as_str())?;
        if later_disposition != first_disposition {
            return Err(format!(
                "honest pair must keep disposition {first_disposition} after the snapshots advance, got {later_disposition}"
            ));
        }
        std::fs::remove_dir_all(&root)
            .map_err(|error| format!("remove {} failed: {error}", root.display()))?;
        Ok(())
    }

    /// #5256 review: a pending projection whose verify names a missing
    /// after snapshot must stay unconfirmed. The validator binds digest,
    /// seam, verdict, and status but does not check after-path existence,
    /// so without fail-closed unavailable handling the legacy fallback
    /// would issue a gap-closing receipt from an unevaluable basis.
    #[test]
    fn pending_pair_naming_missing_snapshot_stays_unconfirmed() -> Result<(), String> {
        let root = test_repo_root("missing-after")?;
        let prepared = prepare_sample_attempt(&root, "seam:sample", "missing")?;
        let finished = finish_sample_attempt(&root, &prepared)?;
        run_git(&root, &["add", "tests/target.rs"])?;
        run_git(&root, &["commit", "--no-gpg-sign", "-m", "focused test"])?;
        let after_path = root.join("target/ripr/workflow/after.json");
        let after_snapshot = crate::testing::verify_fixture::mint_repo_exposure_snapshot(
            root.as_path(),
            serde_json::json!([crate::testing::verify_fixture::snapshot_seam(
                "seam:sample",
                "predicate_boundary",
                "src/lib.rs",
                1,
                "weakly_gripped",
            )]),
        )?;
        std::fs::write(&after_path, after_snapshot.as_bytes())
            .map_err(|error| format!("write {} failed: {error}", after_path.display()))?;
        let retained_before =
            root.join(&find_manifest_artifact(&finished, "before_snapshot")?.path);
        let verify = crate::testing::verify_fixture::mint_canonical_verify(
            root.as_path(),
            &retained_before,
            &after_path,
        )?;
        // Control: the honest canonical pair is admitted while its basis
        // is present.
        match canonically_admit_terminal_pair(root.as_path(), verify.as_bytes()) {
            CanonicalAdmission::Admitted => {}
            other => return Err(format!("honest pair must be admitted, got {other:?}")),
        }
        // Attack: point inputs.after at a missing path and rebind the
        // receipt's verify digest so the validator still passes.
        let mut forged_verify: serde_json::Value = serde_json::from_str(&verify)
            .map_err(|error| format!("parse honest verify failed: {error}"))?;
        forged_verify["inputs"]["after"] =
            serde_json::Value::String("target/ripr/workflow/after.gone.json".to_string());
        let mut forged_verify_bytes = serde_json::to_vec_pretty(&forged_verify)
            .map_err(|error| format!("serialize forged verify failed: {error}"))?;
        forged_verify_bytes.push(b'\n');
        let receipt_bytes =
            mint_bound_receipt(&finished, "unchanged", &sha256_bytes(&forged_verify_bytes))?;
        let receipt_value: serde_json::Value = serde_json::from_slice(&receipt_bytes)
            .map_err(|error| format!("parse forged receipt failed: {error}"))?;
        validate_issued_receipt_evidence(&root, &finished, &receipt_value, &forged_verify_bytes)
            .map_err(|error| format!("missing-basis pair must pass the validator: {error}"))?;
        match canonically_admit_terminal_pair(&root, &forged_verify_bytes) {
            CanonicalAdmission::Unavailable { .. } => {}
            other => {
                return Err(format!(
                    "missing-basis pair must be unavailable, not {other:?}"
                ));
            }
        }
        let reports = root.join("target/ripr/reports");
        std::fs::create_dir_all(&reports)
            .map_err(|error| format!("create {} failed: {error}", reports.display()))?;
        std::fs::write(reports.join("agent-receipt.json"), &receipt_bytes)
            .map_err(|error| format!("write forged receipt failed: {error}"))?;
        std::fs::write(
            root.join("target/ripr/workflow/agent-verify.json"),
            &forged_verify_bytes,
        )
        .map_err(|error| format!("write forged verify failed: {error}"))?;
        if complete_pending_terminal_retention(&root, finished.repair_attempt_id.as_str())? {
            return Err("a missing-basis projection must not complete retention".to_string());
        }
        let (disposition, warned) =
            cli_disposition_for(&root, finished.repair_attempt_id.as_str())?;
        if disposition == "finished" {
            return Err("a missing-basis pair must never report finished".to_string());
        }
        if disposition != "unconfirmed" {
            return Err(format!(
                "a missing-basis pair must read unconfirmed, got {disposition}"
            ));
        }
        if !warned {
            return Err("a missing-basis pair must warn it is unconfirmed".to_string());
        }
        std::fs::remove_dir_all(&root)
            .map_err(|error| format!("remove {} failed: {error}", root.display()))?;
        Ok(())
    }

    /// #5256 review R6 residual: a validator-consistent pair whose snapshots
    /// are gone (historical) keeps its validator-only reading: canonical
    /// admission is unavailable, not a refusal.
    #[test]
    fn historical_pair_without_snapshots_stays_readable() -> Result<(), String> {
        let root = test_repo_root("historical-readable")?;
        let prepared = prepare_sample_attempt(&root, "seam:sample", "historical")?;
        let finished = finish_sample_attempt(&root, &prepared)?;
        let (receipt_bytes, verify_bytes) = mint_bound_receipt_pair(&root, &finished, "improved")?;
        let retained = retain_minted_pair(&root, &finished, &receipt_bytes, &verify_bytes)?;
        // The minted after snapshot was never written: the pair is historical.
        match canonically_admit_terminal_pair(&root, &verify_bytes) {
            CanonicalAdmission::Unavailable { .. } => {}
            other => {
                return Err(format!(
                    "a pair with gone snapshots must be unavailable, not {other:?}"
                ));
            }
        }
        match load_attempt_terminal_receipt(&root, &retained) {
            AttemptTerminalReceipt::Issued { .. } => {}
            other => {
                return Err(format!(
                    "a historical consistent pair must stay issued, got {other:?}"
                ));
            }
        }
        let (disposition, _) = cli_disposition_for(&root, retained.repair_attempt_id.as_str())?;
        if disposition != "finished" {
            return Err(format!(
                "a historical improved pair must still report finished, got {disposition}"
            ));
        }
        std::fs::remove_dir_all(&root)
            .map_err(|error| format!("remove {} failed: {error}", root.display()))?;
        Ok(())
    }

    #[test]
    fn restored_attempt_may_replace_unpublished_terminal_files() -> Result<(), String> {
        let root = test_repo_root("retain-restore")?;
        let prepared = prepare_sample_attempt(&root, "seam:sample", "restore")?;
        let finished = finish_sample_attempt(&root, &prepared)?;
        let reports = root.join("target/ripr/reports");
        std::fs::create_dir_all(&reports)
            .map_err(|error| format!("create {} failed: {error}", reports.display()))?;
        let receipt_path = reports.join("agent-receipt.json");
        let verify_path = root.join("target/ripr/workflow/agent-verify.json");
        let (receipt_bytes, verify_bytes) = mint_bound_receipt_pair(&root, &finished, "unchanged")?;
        std::fs::write(&receipt_path, &receipt_bytes)
            .map_err(|error| format!("write receipt failed: {error}"))?;
        std::fs::write(&verify_path, &verify_bytes)
            .map_err(|error| format!("write verify failed: {error}"))?;
        retain_terminal_evidence(
            &root,
            &finished.repair_attempt_id,
            &[
                BeforeArtifactSource {
                    role: TERMINAL_RECEIPT_ROLE,
                    path: &receipt_path,
                },
                BeforeArtifactSource {
                    role: TERMINAL_VERIFY_ROLE,
                    path: &verify_path,
                },
            ],
        )?;

        restore_repair_attempt_to_awaiting_edit(&root, &finished.repair_attempt_id)?;
        let restored = load_repair_attempt_manifest(&root, &finished.repair_attempt_id)?;
        let packet = root.join(&find_manifest_artifact(&restored, "agent_packet")?.path);
        finish_repair_attempt(
            &root,
            &finished.repair_attempt_id,
            &packet,
            HeadMovement::AdmitDescendantCommits,
        )?;
        let retried = load_repair_attempt_manifest(&root, &finished.repair_attempt_id)?;
        let (retried_receipt_bytes, retried_verify_bytes) =
            mint_bound_receipt_pair(&root, &retried, "unchanged")?;
        let mut receipt: serde_json::Value = serde_json::from_slice(&retried_receipt_bytes)
            .map_err(|error| format!("parse retried receipt: {error}"))?;
        // The marker changes the receipt bytes only; the recorded verify
        // digest still covers the retried verify bytes minted above.
        receipt["generated_at"] = serde_json::json!("retry");
        std::fs::write(
            &receipt_path,
            serde_json::to_vec_pretty(&receipt)
                .map_err(|error| format!("serialize retried receipt: {error}"))?,
        )
        .map_err(|error| format!("write retried receipt failed: {error}"))?;
        std::fs::write(&verify_path, &retried_verify_bytes)
            .map_err(|error| format!("write retried verify failed: {error}"))?;

        retain_terminal_evidence(
            &root,
            &retried.repair_attempt_id,
            &[
                BeforeArtifactSource {
                    role: TERMINAL_RECEIPT_ROLE,
                    path: &receipt_path,
                },
                BeforeArtifactSource {
                    role: TERMINAL_VERIFY_ROLE,
                    path: &verify_path,
                },
            ],
        )?;
        let retained = load_repair_attempt_manifest(&root, &retried.repair_attempt_id)?;
        match load_attempt_terminal_receipt(&root, &retained) {
            AttemptTerminalReceipt::Issued { value, .. } => {
                if value["generated_at"] != "retry" {
                    return Err("retry must retain the new receipt bytes".to_string());
                }
            }
            other => return Err(format!("expected issued retried receipt, got {other:?}")),
        }

        std::fs::remove_dir_all(&root)
            .map_err(|error| format!("remove {} failed: {error}", root.display()))?;
        Ok(())
    }

    #[test]
    fn explicit_store_before_and_after_share_identity_and_stay_isolated() -> Result<(), String> {
        let root = test_repo_root("store-roundtrip")?;
        let alt = Path::new("target/ripr/alt-attempts");
        let first = {
            let workflow = root.join("target/ripr/workflow");
            std::fs::create_dir_all(&workflow)
                .map_err(|error| format!("create {} failed: {error}", workflow.display()))?;
            let before = workflow.join("before-alt.json");
            let packet = workflow.join("packet-alt.json");
            let baseline = workflow.join("baseline-alt.json");
            std::fs::write(&before, b"{}")
                .map_err(|error| format!("write {} failed: {error}", before.display()))?;
            let packet_text = serde_json::json!({
                "seam_id": "seam:alt",
                "allowed_edit_surface": ["tests/target.rs"],
                "forbidden_files": []
            })
            .to_string();
            std::fs::write(&packet, packet_text.as_bytes())
                .map_err(|error| format!("write {} failed: {error}", packet.display()))?;
            let policy = edit_cage_policy_from_packet(&packet_text, "seam:alt")?;
            write_edit_cage_baseline(&root, &baseline, &policy)?;
            begin_repair_attempt_with(BeginRepairAttemptOptions {
                root: &root,
                root_argument: &root,
                seam_id: "seam:alt",
                sources: &[
                    BeforeArtifactSource {
                        role: "before_snapshot",
                        path: &before,
                    },
                    BeforeArtifactSource {
                        role: "agent_packet",
                        path: &packet,
                    },
                    BeforeArtifactSource {
                        role: "edit_cage_baseline",
                        path: &baseline,
                    },
                ],
                expected_repository_head: None,
                next_command_suffix: None,
                store: Some(alt),
            })?
        };
        if first.manifest.store.is_none() {
            return Err("explicit store omitted its manifest identity".to_string());
        }
        if !first.manifest.next_command.contains("--store")
            || !first
                .manifest
                .next_command
                .contains("target/ripr/alt-attempts")
        {
            return Err(format!(
                "explicit next command lost --store: {}",
                first.manifest.next_command
            ));
        }
        let encoded = serde_json::to_value(&first.manifest)
            .map_err(|error| format!("encode explicit manifest failed: {error}"))?;
        if encoded.get("store").and_then(|value| value.get("locator"))
            != Some(&serde_json::Value::String(
                "target/ripr/alt-attempts".to_string(),
            ))
        {
            return Err(format!(
                "explicit store identity was not retained: {encoded}"
            ));
        }

        let resolved = resolve_awaiting_repair_attempt_from(
            &root,
            Some(alt),
            Some(first.manifest.repair_attempt_id.as_str()),
            None,
        )?;
        if resolved.attempt_id != first.manifest.repair_attempt_id {
            return Err("explicit after resolved a different attempt".to_string());
        }

        let default_miss = resolve_awaiting_repair_attempt(
            &root,
            Some(first.manifest.repair_attempt_id.as_str()),
            None,
        );
        match default_miss {
            Err(error) if error.contains("not found") => {}
            other => {
                return Err(format!(
                    "default store must not see an explicit-store attempt: {other:?}"
                ));
            }
        }

        let other = Path::new("target/ripr/other-attempts");
        let other_miss = resolve_awaiting_repair_attempt_from(
            &root,
            Some(other),
            Some(first.manifest.repair_attempt_id.as_str()),
            None,
        );
        match other_miss {
            Err(error) if error.contains("does not fall back") || error.contains("not found") => {}
            other => {
                return Err(format!(
                    "a second explicit store must not resolve the first: {other:?}"
                ));
            }
        }

        let defaulted = prepare_sample_attempt(&root, "seam:default", "default")?;
        if defaulted.manifest.store.is_some() {
            return Err("default store leaked a manifest store object".to_string());
        }
        if defaulted.manifest.next_command.contains("--store") {
            return Err(format!(
                "default next command named --store: {}",
                defaulted.manifest.next_command
            ));
        }
        let default_bytes = serde_json::to_vec(&defaulted.manifest)
            .map_err(|error| format!("encode default manifest failed: {error}"))?;
        if default_bytes
            .windows(7)
            .any(|window| window == b"\"store\"")
        {
            return Err("default manifest serialized a store field".to_string());
        }

        std::fs::remove_dir_all(&root)
            .map_err(|error| format!("remove {} failed: {error}", root.display()))?;
        Ok(())
    }
}
