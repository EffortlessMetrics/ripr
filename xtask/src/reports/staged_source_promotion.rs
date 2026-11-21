//! #1627 Phase 1 & Phase 2: Versioned staged P/S/T/K/A source-promotion
//! admission contract, independent qualification-receipt custody verification,
//! real Git object-store survivor/nested-import verification, and typed
//! required-result consumer verdict.
//!
//! This module is strictly read-only: it never mutates any repository ref,
//! raises analysis limits, or renames cross-repository swarm qualification as
//! native source proof.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

pub(crate) const SOURCE_PROMOTION_VERIFY_STAGED_PACKET_SUBCOMMAND: &str = "verify-staged-packet";
pub(crate) const STAGED_PACKET_SCHEMA: &str = "ripr.staged_source_promotion_packet.v1";
pub(crate) const STAGED_VERDICT_SCHEMA: &str = "ripr.staged_source_promotion_verdict.v1";
pub(crate) const EXPECTED_ISSUER_REPOSITORY: &str = "EffortlessMetrics/ripr-swarm";
pub(crate) const EXPECTED_AUTHORITY_KIND: &str = "retained_cross_repo_qualification";
pub(crate) const EXPECTED_QUALIFICATION_INSTRUMENT: &str = "ripr-xtask-qualification";
pub(crate) const EXPECTED_QUALIFICATION_SCHEMA: &str = "ripr.swarm_qualification_receipt.v1";
const REPORT_JSON: &str = "staged-source-promotion-verdict.json";

const VALID_GIT_MODES: [&str; 4] = ["100644", "100755", "120000", "160000"];

#[derive(Clone, Debug, Deserialize, Serialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
pub(crate) struct StagedPromotionPacket {
    pub(crate) schema: String,
    pub(crate) public_base_p: String,
    pub(crate) swarm_return_s: String,
    pub(crate) swarm_tree_s: String,
    pub(crate) qualified_tree_t: String,
    pub(crate) p_is_ancestor_of_s: bool,
    pub(crate) github_preview_k: PreviewMergeK,
    #[serde(default)]
    pub(crate) observed_public_a: Option<ObservedPublicMergeA>,
    pub(crate) qualification: CrossRepoQualification,
    pub(crate) inventory: SurvivorAndImportInventory,
}

#[derive(Clone, Debug, Deserialize, Serialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
pub(crate) struct PreviewMergeK {
    pub(crate) commit_sha: String,
    pub(crate) parents: Vec<String>,
    pub(crate) tree: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
pub(crate) struct ObservedPublicMergeA {
    pub(crate) commit_sha: String,
    pub(crate) parents: Vec<String>,
    pub(crate) tree: String,
    pub(crate) current_main_sha: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
pub(crate) struct CrossRepoQualification {
    pub(crate) issuer_repository: String,
    pub(crate) authority_kind: String,
    pub(crate) instrument: String,
    pub(crate) schema: String,
    pub(crate) receipt_id: String,
    pub(crate) receipt_sha256: String,
    pub(crate) qualified_s: String,
    pub(crate) qualified_t: String,
    pub(crate) mandatory_commands_executed: u64,
    pub(crate) mandatory_commands_skipped: u64,
    pub(crate) stale: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
pub(crate) struct ExternalQualificationReceipt {
    pub(crate) schema: String,
    pub(crate) issuer_repository: String,
    pub(crate) authority_kind: String,
    pub(crate) instrument: String,
    pub(crate) receipt_id: String,
    pub(crate) qualified_s: String,
    pub(crate) qualified_t: String,
    pub(crate) outcome: String,
    pub(crate) mandatory_commands_executed: u64,
    pub(crate) mandatory_commands_skipped: u64,
    pub(crate) stale: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub(crate) enum SurvivorChangeKind {
    Retained,
    Renamed,
    Deleted,
}

#[derive(Clone, Debug, Deserialize, Serialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
pub(crate) struct PublicSurvivorEntry {
    pub(crate) path: String,
    pub(crate) mode: String,
    pub(crate) object_sha: Option<String>,
    pub(crate) change_kind: SurvivorChangeKind,
    #[serde(default)]
    pub(crate) previous_path: Option<String>,
    pub(crate) nested_import_commit: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
pub(crate) struct NestedImportEntry {
    pub(crate) import_commit: String,
    pub(crate) source_public_base: String,
    pub(crate) tree_sha: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, Eq, PartialEq)]
#[serde(tag = "state", rename_all = "snake_case")]
pub(crate) enum WholeDiffAnalysisDisposition {
    Complete,
    LimitedUnavailable { reason: String },
}

#[derive(Clone, Debug, Deserialize, Serialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
pub(crate) struct SurvivorAndImportInventory {
    pub(crate) public_survivors_sha256: String,
    pub(crate) nested_import_sha256: String,
    pub(crate) survivors: Vec<PublicSurvivorEntry>,
    pub(crate) nested_imports: Vec<NestedImportEntry>,
    pub(crate) analysis_disposition: WholeDiffAnalysisDisposition,
}

#[derive(Clone, Debug, Serialize, Eq, PartialEq)]
pub(crate) struct StagedPromotionIdentities {
    pub(crate) public_base_p: String,
    pub(crate) swarm_return_s: String,
    pub(crate) qualified_tree_t: String,
    pub(crate) github_preview_k: String,
    pub(crate) observed_public_a: Option<String>,
}

#[derive(Clone, Debug, Serialize, Eq, PartialEq)]
pub(crate) struct StagedPromotionConsumerVerdict {
    pub(crate) schema: String,
    pub(crate) status: String,
    pub(crate) verification_scope: String,
    pub(crate) phase: String,
    pub(crate) reason: String,
    pub(crate) qualification_authority: String,
    pub(crate) analysis_disposition: WholeDiffAnalysisDisposition,
    pub(crate) identities: StagedPromotionIdentities,
    pub(crate) claim: String,
}

fn is_hex(value: &str, len: usize) -> bool {
    value.len() == len
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
}

pub(crate) fn compute_survivors_digest(
    survivors: &[PublicSurvivorEntry],
) -> Result<String, String> {
    let encoded = serde_json::to_vec(survivors)
        .map_err(|err| format!("serialize public survivor inventory: {err}"))?;
    Ok(sha256_hex(&encoded))
}

pub(crate) fn compute_nested_imports_digest(
    imports: &[NestedImportEntry],
) -> Result<String, String> {
    let encoded = serde_json::to_vec(imports)
        .map_err(|err| format!("serialize nested import inventory: {err}"))?;
    Ok(sha256_hex(&encoded))
}

fn reject_with_scope(
    packet: &StagedPromotionPacket,
    scope: &str,
    phase: &str,
    reason: impl Into<String>,
) -> StagedPromotionConsumerVerdict {
    StagedPromotionConsumerVerdict {
        schema: STAGED_VERDICT_SCHEMA.to_string(),
        status: "rejected".to_string(),
        verification_scope: scope.to_string(),
        phase: phase.to_string(),
        reason: reason.into(),
        qualification_authority: packet.qualification.authority_kind.clone(),
        analysis_disposition: packet.inventory.analysis_disposition.clone(),
        identities: StagedPromotionIdentities {
            public_base_p: packet.public_base_p.clone(),
            swarm_return_s: packet.swarm_return_s.clone(),
            qualified_tree_t: packet.qualified_tree_t.clone(),
            github_preview_k: packet.github_preview_k.commit_sha.clone(),
            observed_public_a: packet
                .observed_public_a
                .as_ref()
                .map(|a| a.commit_sha.clone()),
        },
        claim: "Staged admission verdict only; does not authorize repository settings, merge, tag, or release publication.".to_string(),
    }
}

fn reject(
    packet: &StagedPromotionPacket,
    phase: &str,
    reason: impl Into<String>,
) -> StagedPromotionConsumerVerdict {
    reject_with_scope(packet, "packet_self_consistency_only", phase, reason)
}

/// Evaluate packet-level self-consistency (Phase 1). When external receipt
/// custody and Git object-store verification are not supplied, this returns
/// `packet_consistent` (never `admitted`).
pub(crate) fn evaluate_staged_promotion_packet(
    packet: &StagedPromotionPacket,
) -> StagedPromotionConsumerVerdict {
    if packet.schema != STAGED_PACKET_SCHEMA {
        return reject(
            packet,
            "schema",
            format!("unsupported staged packet schema: {}", packet.schema),
        );
    }
    for (label, value) in [
        ("public_base_p", &packet.public_base_p),
        ("swarm_return_s", &packet.swarm_return_s),
        ("swarm_tree_s", &packet.swarm_tree_s),
        ("qualified_tree_t", &packet.qualified_tree_t),
        (
            "github_preview_k.commit_sha",
            &packet.github_preview_k.commit_sha,
        ),
        ("github_preview_k.tree", &packet.github_preview_k.tree),
    ] {
        if !is_hex(value, 40) {
            return reject(
                packet,
                "geometry",
                format!("{label} must be a 40-char lowercase hex SHA"),
            );
        }
    }
    if packet.public_base_p == packet.swarm_return_s {
        return reject(
            packet,
            "geometry",
            "public base P and swarm return head S must be distinct commits",
        );
    }
    if !packet.p_is_ancestor_of_s {
        return reject(
            packet,
            "geometry",
            "captured public base P is not an ancestor of swarm return head S",
        );
    }
    let expected_parents = vec![packet.public_base_p.clone(), packet.swarm_return_s.clone()];
    if packet.github_preview_k.parents != expected_parents {
        return reject(
            packet,
            "geometry",
            "GitHub preview K parents must be exact ordered [P, S]",
        );
    }
    if packet.swarm_tree_s != packet.qualified_tree_t {
        return reject(
            packet,
            "geometry",
            "swarm return head tree S.tree differs from qualified tree T",
        );
    }
    if packet.github_preview_k.tree != packet.qualified_tree_t {
        return reject(
            packet,
            "geometry",
            "GitHub preview K.tree differs from qualified tree T (wrapper/copy-tree substitution rejected)",
        );
    }

    if let Some(observed_a) = &packet.observed_public_a {
        for (label, value) in [
            ("observed_public_a.commit_sha", &observed_a.commit_sha),
            ("observed_public_a.tree", &observed_a.tree),
            (
                "observed_public_a.current_main_sha",
                &observed_a.current_main_sha,
            ),
        ] {
            if !is_hex(value, 40) {
                return reject(
                    packet,
                    "post_merge_a",
                    format!("{label} must be a 40-char lowercase hex SHA"),
                );
            }
        }
        if observed_a.parents != expected_parents {
            return reject(
                packet,
                "post_merge_a",
                "observed public merge A parents must be exact ordered [P, S]",
            );
        }
        if observed_a.tree != packet.qualified_tree_t {
            return reject(
                packet,
                "post_merge_a",
                "observed public merge A.tree differs from qualified tree T",
            );
        }
        if observed_a.current_main_sha != observed_a.commit_sha {
            return reject(
                packet,
                "post_merge_a",
                "observed public main moved past recorded merge A",
            );
        }
    }

    let qual = &packet.qualification;
    if qual.issuer_repository != EXPECTED_ISSUER_REPOSITORY {
        return reject(
            packet,
            "qualification_issuer",
            format!(
                "foreign qualification issuer repository rejected: {}",
                qual.issuer_repository
            ),
        );
    }
    if qual.authority_kind != EXPECTED_AUTHORITY_KIND {
        return reject(
            packet,
            "qualification_issuer",
            "cross-repository swarm qualification must remain retained_cross_repo_qualification, never renamed native source proof",
        );
    }
    if qual.instrument != EXPECTED_QUALIFICATION_INSTRUMENT {
        return reject(
            packet,
            "qualification_issuer",
            format!("unsupported qualification instrument: {}", qual.instrument),
        );
    }
    if qual.schema != EXPECTED_QUALIFICATION_SCHEMA {
        return reject(
            packet,
            "qualification_issuer",
            format!("incompatible qualification receipt schema: {}", qual.schema),
        );
    }
    if qual.receipt_id.trim().is_empty() || !is_hex(&qual.receipt_sha256, 64) {
        return reject(
            packet,
            "qualification_issuer",
            "qualification receipt_id and 64-char receipt_sha256 are required",
        );
    }
    if qual.stale {
        return reject(
            packet,
            "qualification_issuer",
            "stale qualification receipt rejected",
        );
    }
    if qual.qualified_s != packet.swarm_return_s || qual.qualified_t != packet.qualified_tree_t {
        return reject(
            packet,
            "qualification_issuer",
            "qualification receipt S/T does not match staged packet S/T",
        );
    }
    if qual.mandatory_commands_executed == 0 {
        return reject(
            packet,
            "qualification_execution",
            "zero mandatory qualification commands executed",
        );
    }
    if qual.mandatory_commands_skipped > 0 {
        return reject(
            packet,
            "qualification_execution",
            "skipped mandatory qualification commands are rejected",
        );
    }

    let inv = &packet.inventory;
    if !is_hex(&inv.public_survivors_sha256, 64) || !is_hex(&inv.nested_import_sha256, 64) {
        return reject(
            packet,
            "inventory",
            "survivor and nested-import inventory digests must be 64-char lowercase hex SHA-256",
        );
    }
    if inv.nested_imports.is_empty() {
        return reject(
            packet,
            "inventory",
            "nested public-import provenance is required; outer PR second parent is insufficient",
        );
    }
    let mut known_imports = BTreeSet::new();
    for entry in &inv.nested_imports {
        if !is_hex(&entry.import_commit, 40)
            || !is_hex(&entry.source_public_base, 40)
            || !is_hex(&entry.tree_sha, 40)
        {
            return reject(
                packet,
                "inventory",
                "nested import entry contains invalid 40-char SHA",
            );
        }
        known_imports.insert(entry.import_commit.as_str());
    }
    let mut seen_paths = BTreeSet::new();
    for survivor in &inv.survivors {
        if survivor.path.trim().is_empty()
            || survivor.path.starts_with('/')
            || survivor.path.contains("..")
            || !seen_paths.insert(survivor.path.as_str())
        {
            return reject(
                packet,
                "inventory",
                format!("invalid or duplicate survivor path: {}", survivor.path),
            );
        }
        if !VALID_GIT_MODES.contains(&survivor.mode.as_str()) {
            return reject(
                packet,
                "inventory",
                format!(
                    "invalid Git file mode {} for {}",
                    survivor.mode, survivor.path
                ),
            );
        }
        if !known_imports.contains(survivor.nested_import_commit.as_str()) {
            return reject(
                packet,
                "inventory",
                format!(
                    "survivor {} references unknown nested_import_commit {}",
                    survivor.path, survivor.nested_import_commit
                ),
            );
        }
        match survivor.change_kind {
            SurvivorChangeKind::Retained => {
                if !survivor
                    .object_sha
                    .as_deref()
                    .is_some_and(|s| is_hex(s, 40))
                {
                    return reject(
                        packet,
                        "inventory",
                        format!(
                            "retained survivor {} requires 40-char object_sha",
                            survivor.path
                        ),
                    );
                }
            }
            SurvivorChangeKind::Renamed => {
                if !survivor
                    .object_sha
                    .as_deref()
                    .is_some_and(|s| is_hex(s, 40))
                    || survivor
                        .previous_path
                        .as_deref()
                        .is_none_or(|p| p.trim().is_empty())
                {
                    return reject(
                        packet,
                        "inventory",
                        format!(
                            "renamed survivor {} requires object_sha and previous_path",
                            survivor.path
                        ),
                    );
                }
            }
            SurvivorChangeKind::Deleted => {
                if survivor.object_sha.is_some() {
                    return reject(
                        packet,
                        "inventory",
                        format!(
                            "deleted survivor {} must have null object_sha",
                            survivor.path
                        ),
                    );
                }
            }
        }
    }
    if let WholeDiffAnalysisDisposition::LimitedUnavailable { reason } = &inv.analysis_disposition
        && reason.trim().is_empty()
    {
        return reject(
            packet,
            "inventory",
            "limited_unavailable analysis disposition requires a non-empty reason",
        );
    }
    match compute_survivors_digest(&inv.survivors) {
        Ok(actual) if actual == inv.public_survivors_sha256 => {}
        Ok(_) => {
            return reject(
                packet,
                "inventory",
                "public_survivors_sha256 does not match serialized survivor inventory",
            );
        }
        Err(err) => return reject(packet, "inventory", err),
    }
    match compute_nested_imports_digest(&inv.nested_imports) {
        Ok(actual) if actual == inv.nested_import_sha256 => {}
        Ok(_) => {
            return reject(
                packet,
                "inventory",
                "nested_import_sha256 does not match serialized nested-import inventory",
            );
        }
        Err(err) => return reject(packet, "inventory", err),
    }

    StagedPromotionConsumerVerdict {
        schema: STAGED_VERDICT_SCHEMA.to_string(),
        status: "packet_consistent".to_string(),
        verification_scope: "packet_self_consistency_only".to_string(),
        phase: "complete".to_string(),
        reason: "staged_packet_consistency_verified".to_string(),
        qualification_authority: qual.authority_kind.clone(),
        analysis_disposition: inv.analysis_disposition.clone(),
        identities: StagedPromotionIdentities {
            public_base_p: packet.public_base_p.clone(),
            swarm_return_s: packet.swarm_return_s.clone(),
            qualified_tree_t: packet.qualified_tree_t.clone(),
            github_preview_k: packet.github_preview_k.commit_sha.clone(),
            observed_public_a: packet
                .observed_public_a
                .as_ref()
                .map(|a| a.commit_sha.clone()),
        },
        claim: "Packet self-consistency verdict only; does not independently verify external qualification receipt bytes, Git-history survivor/nested-import completeness, repository settings, merge, tag, or release publication.".to_string(),
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ExpectedStagedSubject {
    pub(crate) public_base_p: String,
    pub(crate) swarm_return_s: String,
    pub(crate) github_preview_k: String,
    pub(crate) qualified_tree_t: String,
}

/// Phase 2 independent verification: verifies packet self-consistency, binds
/// the packet to the caller's live protected PR subject (P/S/K/T), hashes and
/// parses the retained qualification receipt file on disk, and verifies
/// P/S/K/T/A ancestry, trees, merge-parent nested imports, and public
/// survivors against the local Git object store without lazy network fetches.
pub(crate) fn evaluate_staged_promotion_independently(
    packet: &StagedPromotionPacket,
    expected_subject: &ExpectedStagedSubject,
    receipt_path: &Path,
    repo: &Path,
) -> StagedPromotionConsumerVerdict {
    let scope = "independent_receipt_bytes_and_git_verified";
    let base = evaluate_staged_promotion_packet(packet);
    if base.status != "packet_consistent" {
        return reject_with_scope(packet, scope, &base.phase, base.reason);
    }
    if packet.public_base_p != expected_subject.public_base_p
        || packet.swarm_return_s != expected_subject.swarm_return_s
        || packet.github_preview_k.commit_sha != expected_subject.github_preview_k
        || packet.qualified_tree_t != expected_subject.qualified_tree_t
    {
        return reject_with_scope(
            packet,
            scope,
            "subject_binding",
            format!(
                "staged packet P/S/K/T ({}/{}/{}/{}) does not match live protected PR subject ({}/{}/{}/{})",
                packet.public_base_p,
                packet.swarm_return_s,
                packet.github_preview_k.commit_sha,
                packet.qualified_tree_t,
                expected_subject.public_base_p,
                expected_subject.swarm_return_s,
                expected_subject.github_preview_k,
                expected_subject.qualified_tree_t,
            ),
        );
    }
    if let Err((phase, reason)) = verify_external_qualification_receipt(packet, receipt_path) {
        return reject_with_scope(packet, scope, &phase, reason);
    }
    if let Err((phase, reason)) = verify_git_object_store(packet, repo) {
        return reject_with_scope(packet, scope, &phase, reason);
    }
    StagedPromotionConsumerVerdict {
        schema: STAGED_VERDICT_SCHEMA.to_string(),
        status: "admitted".to_string(),
        verification_scope: scope.to_string(),
        phase: "complete".to_string(),
        reason: "staged_packet_independently_verified".to_string(),
        qualification_authority: packet.qualification.authority_kind.clone(),
        analysis_disposition: packet.inventory.analysis_disposition.clone(),
        identities: StagedPromotionIdentities {
            public_base_p: packet.public_base_p.clone(),
            swarm_return_s: packet.swarm_return_s.clone(),
            qualified_tree_t: packet.qualified_tree_t.clone(),
            github_preview_k: packet.github_preview_k.commit_sha.clone(),
            observed_public_a: packet
                .observed_public_a
                .as_ref()
                .map(|a| a.commit_sha.clone()),
        },
        claim: "Independently verified against live P/S/K/T subject binding, retained qualification receipt bytes (SHA-256 byte custody, not external API/cryptographic producer attestation), and local Git object store (ancestry, trees, merge-parent nested imports, and public survivors); does not authorize repository settings, merge, tag, or release publication.".to_string(),
    }
}

fn verify_external_qualification_receipt(
    packet: &StagedPromotionPacket,
    receipt_path: &Path,
) -> Result<(), (String, String)> {
    let phase = "qualification_receipt_custody".to_string();
    let meta = fs::symlink_metadata(receipt_path).map_err(|err| {
        (
            phase.clone(),
            format!(
                "inspect qualification receipt {}: {err}",
                receipt_path.display()
            ),
        )
    })?;
    if !meta.is_file() || meta.file_type().is_symlink() {
        return Err((
            phase,
            "qualification receipt must be a regular non-symlink file".to_string(),
        ));
    }
    let bytes = fs::read(receipt_path).map_err(|err| {
        (
            phase.clone(),
            format!(
                "read qualification receipt {}: {err}",
                receipt_path.display()
            ),
        )
    })?;
    let actual_sha256 = sha256_hex(&bytes);
    if actual_sha256 != packet.qualification.receipt_sha256 {
        return Err((
            phase,
            format!(
                "qualification receipt bytes SHA-256 mismatch: expected {}, got {actual_sha256}",
                packet.qualification.receipt_sha256
            ),
        ));
    }
    let text = std::str::from_utf8(&bytes).map_err(|err| {
        (
            phase.clone(),
            format!("qualification receipt is not UTF-8: {err}"),
        )
    })?;
    let receipt: ExternalQualificationReceipt =
        parse_unique_json(text).map_err(|err| (phase.clone(), format!("parse receipt: {err}")))?;
    if receipt.schema != EXPECTED_QUALIFICATION_SCHEMA {
        return Err((
            phase,
            format!("external receipt schema mismatch: {}", receipt.schema),
        ));
    }
    if receipt.issuer_repository != EXPECTED_ISSUER_REPOSITORY {
        return Err((
            phase,
            format!(
                "external receipt foreign issuer rejected: {}",
                receipt.issuer_repository
            ),
        ));
    }
    if receipt.authority_kind != EXPECTED_AUTHORITY_KIND {
        return Err((
            phase,
            format!(
                "external receipt authority kind rejected: {}",
                receipt.authority_kind
            ),
        ));
    }
    if receipt.instrument != EXPECTED_QUALIFICATION_INSTRUMENT {
        return Err((
            phase,
            format!(
                "external receipt instrument rejected: {}",
                receipt.instrument
            ),
        ));
    }
    if receipt.receipt_id != packet.qualification.receipt_id {
        return Err((
            phase,
            "external receipt_id disagrees with staged packet".to_string(),
        ));
    }
    if receipt.qualified_s != packet.swarm_return_s
        || receipt.qualified_t != packet.qualified_tree_t
    {
        return Err((
            phase,
            "external receipt S/T disagrees with staged packet".to_string(),
        ));
    }
    if receipt.outcome != "passed" {
        return Err((
            phase,
            format!(
                "external receipt outcome is not passed: {}",
                receipt.outcome
            ),
        ));
    }
    if receipt.stale {
        return Err((phase, "external receipt is marked stale".to_string()));
    }
    if receipt.mandatory_commands_executed == 0
        || receipt.mandatory_commands_executed != packet.qualification.mandatory_commands_executed
        || receipt.mandatory_commands_skipped != 0
        || receipt.mandatory_commands_skipped != packet.qualification.mandatory_commands_skipped
    {
        return Err((
            phase,
            "external receipt mandatory execution counts disagree or indicate skipped/zero runs"
                .to_string(),
        ));
    }
    Ok(())
}

fn run_git(repo: &Path, args: &[&str]) -> Result<Vec<u8>, String> {
    let mut cmd = Command::new("git");
    cmd.env("GIT_NO_LAZY_FETCH", "1")
        .arg("-C")
        .arg(repo)
        .arg("--no-replace-objects");
    for arg in args {
        cmd.arg(arg);
    }
    let output = cmd
        .output()
        .map_err(|err| format!("spawn git {}: {err}", args.join(" ")))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("git {} failed: {}", args.join(" "), stderr.trim()));
    }
    Ok(output.stdout)
}

fn git_stdout_trimmed(repo: &Path, args: &[&str]) -> Result<String, String> {
    let bytes = run_git(repo, args)?;
    String::from_utf8(bytes)
        .map(|s| s.trim().to_string())
        .map_err(|err| format!("decode git {} stdout: {err}", args.join(" ")))
}

fn git_tree_entries(
    repo: &Path,
    tree_sha: &str,
) -> Result<BTreeMap<String, (String, String)>, String> {
    let raw = run_git(repo, &["ls-tree", "-r", "-z", tree_sha])?;
    let mut entries = BTreeMap::new();
    for record in raw.split(|b| *b == 0) {
        if record.is_empty() {
            continue;
        }
        let text = std::str::from_utf8(record)
            .map_err(|err| format!("non-UTF-8 path in tree {tree_sha}: {err}"))?;
        let (meta, path) = text
            .split_once('\t')
            .ok_or_else(|| format!("malformed ls-tree entry in {tree_sha}: {text}"))?;
        let mut parts = meta.split_whitespace();
        let mode = parts
            .next()
            .ok_or_else(|| format!("missing mode in ls-tree entry: {text}"))?;
        let _kind = parts
            .next()
            .ok_or_else(|| format!("missing object kind in ls-tree entry: {text}"))?;
        let sha = parts
            .next()
            .ok_or_else(|| format!("missing object sha in ls-tree entry: {text}"))?;
        entries.insert(path.to_string(), (mode.to_string(), sha.to_string()));
    }
    Ok(entries)
}

fn verify_git_object_store(
    packet: &StagedPromotionPacket,
    repo: &Path,
) -> Result<(), (String, String)> {
    let geom = "git_geometry".to_string();
    for (label, sha) in [
        ("public_base_p", packet.public_base_p.as_str()),
        ("swarm_return_s", packet.swarm_return_s.as_str()),
        (
            "github_preview_k",
            packet.github_preview_k.commit_sha.as_str(),
        ),
    ] {
        let rev = format!("{sha}^{{commit}}");
        let resolved =
            git_stdout_trimmed(repo, &["rev-parse", "--verify", &rev]).map_err(|err| {
                (
                    geom.clone(),
                    format!("{label} commit missing in repo: {err}"),
                )
            })?;
        if resolved != sha {
            return Err((geom, format!("{label} rev-parse resolved to {resolved}")));
        }
    }
    if run_git(
        repo,
        &[
            "merge-base",
            "--is-ancestor",
            &packet.public_base_p,
            &packet.swarm_return_s,
        ],
    )
    .is_err()
    {
        return Err((
            geom,
            "Git object store: public base P is not an ancestor of swarm return head S".to_string(),
        ));
    }
    let expected_k_parents = format!(
        "{} {} {}",
        packet.github_preview_k.commit_sha, packet.public_base_p, packet.swarm_return_s
    );
    let actual_k_parents = git_stdout_trimmed(
        repo,
        &[
            "rev-list",
            "--parents",
            "-n",
            "1",
            &packet.github_preview_k.commit_sha,
        ],
    )
    .map_err(|err| (geom.clone(), err))?;
    if actual_k_parents != expected_k_parents {
        return Err((
            geom,
            format!(
                "Git object store: preview K parents mismatch (expected '{expected_k_parents}', got '{actual_k_parents}')"
            ),
        ));
    }
    let s_tree = git_stdout_trimmed(
        repo,
        &["rev-parse", &format!("{}^{{tree}}", packet.swarm_return_s)],
    )
    .map_err(|err| (geom.clone(), err))?;
    if s_tree != packet.qualified_tree_t {
        return Err((
            geom,
            format!(
                "Git object store: S.tree ({s_tree}) differs from qualified tree T ({})",
                packet.qualified_tree_t
            ),
        ));
    }
    let k_tree = git_stdout_trimmed(
        repo,
        &[
            "rev-parse",
            &format!("{}^{{tree}}", packet.github_preview_k.commit_sha),
        ],
    )
    .map_err(|err| (geom.clone(), err))?;
    if k_tree != packet.qualified_tree_t {
        return Err((
            geom,
            format!(
                "Git object store: K.tree ({k_tree}) differs from qualified tree T ({})",
                packet.qualified_tree_t
            ),
        ));
    }
    if let Some(observed_a) = &packet.observed_public_a {
        let a_phase = "git_post_merge_a".to_string();
        let expected_a_parents = format!(
            "{} {} {}",
            observed_a.commit_sha, packet.public_base_p, packet.swarm_return_s
        );
        let actual_a_parents = git_stdout_trimmed(
            repo,
            &["rev-list", "--parents", "-n", "1", &observed_a.commit_sha],
        )
        .map_err(|err| (a_phase.clone(), err))?;
        if actual_a_parents != expected_a_parents {
            return Err((
                a_phase,
                format!(
                    "Git object store: observed merge A parents mismatch (expected '{expected_a_parents}', got '{actual_a_parents}')"
                ),
            ));
        }
        let a_tree = git_stdout_trimmed(
            repo,
            &["rev-parse", &format!("{}^{{tree}}", observed_a.commit_sha)],
        )
        .map_err(|err| (a_phase.clone(), err))?;
        if a_tree != packet.qualified_tree_t {
            return Err((
                a_phase,
                format!(
                    "Git object store: A.tree ({a_tree}) differs from T ({})",
                    packet.qualified_tree_t
                ),
            ));
        }
    }

    // Verify nested-import entries are genuine merge imports with
    // source_public_base as an actual parent in S's real Git ancestry.
    let inv_phase = "git_inventory".to_string();
    for entry in &packet.inventory.nested_imports {
        let import_tree = git_stdout_trimmed(
            repo,
            &["rev-parse", &format!("{}^{{tree}}", entry.import_commit)],
        )
        .map_err(|err| {
            (
                inv_phase.clone(),
                format!(
                    "nested import commit {} missing: {err}",
                    entry.import_commit
                ),
            )
        })?;
        if import_tree != entry.tree_sha {
            return Err((
                inv_phase,
                format!(
                    "nested import {} tree mismatch (expected {}, got {import_tree})",
                    entry.import_commit, entry.tree_sha
                ),
            ));
        }
        let parents_line = git_stdout_trimmed(
            repo,
            &["rev-list", "--parents", "-n", "1", &entry.import_commit],
        )
        .map_err(|err| (inv_phase.clone(), err))?;
        let mut parent_tokens = parents_line.split_whitespace();
        let _self_sha = parent_tokens.next();
        let import_parents: Vec<&str> = parent_tokens.collect();
        if import_parents.len() < 2 || !import_parents.contains(&entry.source_public_base.as_str())
        {
            return Err((
                inv_phase,
                format!(
                    "nested import {} must be a merge commit with source_public_base {} as an actual parent (found parents: {:?})",
                    entry.import_commit, entry.source_public_base, import_parents
                ),
            ));
        }
        if run_git(
            repo,
            &[
                "merge-base",
                "--is-ancestor",
                &entry.import_commit,
                &packet.swarm_return_s,
            ],
        )
        .is_err()
        {
            return Err((
                inv_phase,
                format!(
                    "nested import {} is not an ancestor of swarm return head S",
                    entry.import_commit
                ),
            ));
        }
    }

    // Verify public survivors against actual trees P^{tree} and T.
    let p_tree_sha = git_stdout_trimmed(
        repo,
        &["rev-parse", &format!("{}^{{tree}}", packet.public_base_p)],
    )
    .map_err(|err| (inv_phase.clone(), err))?;
    let p_entries = git_tree_entries(repo, &p_tree_sha).map_err(|err| (inv_phase.clone(), err))?;
    let t_entries =
        git_tree_entries(repo, &packet.qualified_tree_t).map_err(|err| (inv_phase.clone(), err))?;

    let mut accounted_deleted_or_moved = BTreeSet::new();
    for survivor in &packet.inventory.survivors {
        match survivor.change_kind {
            SurvivorChangeKind::Retained => {
                if !p_entries.contains_key(&survivor.path) {
                    return Err((
                        inv_phase,
                        format!(
                            "retained survivor {} was not present in base tree P",
                            survivor.path
                        ),
                    ));
                }
                let Some((actual_mode, actual_sha)) = t_entries.get(&survivor.path) else {
                    return Err((
                        inv_phase,
                        format!("retained survivor {} is missing from tree T", survivor.path),
                    ));
                };
                if actual_mode != &survivor.mode
                    || survivor.object_sha.as_deref() != Some(actual_sha.as_str())
                {
                    return Err((
                        inv_phase,
                        format!(
                            "retained survivor {} disagrees with tree T (expected {} {:?}, got {} {})",
                            survivor.path,
                            survivor.mode,
                            survivor.object_sha,
                            actual_mode,
                            actual_sha
                        ),
                    ));
                }
            }
            SurvivorChangeKind::Renamed => {
                let Some((actual_mode, actual_sha)) = t_entries.get(&survivor.path) else {
                    return Err((
                        inv_phase,
                        format!("renamed survivor {} is missing from tree T", survivor.path),
                    ));
                };
                if actual_mode != &survivor.mode
                    || survivor.object_sha.as_deref() != Some(actual_sha.as_str())
                {
                    return Err((
                        inv_phase,
                        format!("renamed survivor {} disagrees with tree T", survivor.path),
                    ));
                }
                if let Some(prev) = &survivor.previous_path {
                    if !p_entries.contains_key(prev) {
                        return Err((
                            inv_phase,
                            format!(
                                "renamed survivor previous_path {prev} is absent from base tree P"
                            ),
                        ));
                    }
                    if t_entries.contains_key(prev) {
                        return Err((
                            inv_phase,
                            format!("renamed survivor previous_path {prev} still exists in tree T"),
                        ));
                    }
                    if !accounted_deleted_or_moved.insert(prev.as_str()) {
                        return Err((
                            inv_phase,
                            format!("conflicting survivor dispositions for base path {prev}"),
                        ));
                    }
                }
            }
            SurvivorChangeKind::Deleted => {
                let Some((p_mode, _)) = p_entries.get(&survivor.path) else {
                    return Err((
                        inv_phase,
                        format!(
                            "deleted survivor {} was not present in base tree P",
                            survivor.path
                        ),
                    ));
                };
                if p_mode != &survivor.mode {
                    return Err((
                        inv_phase,
                        format!(
                            "deleted survivor {} mode disagrees with base tree P (expected {}, got {p_mode})",
                            survivor.path, survivor.mode
                        ),
                    ));
                }
                if t_entries.contains_key(&survivor.path) {
                    return Err((
                        inv_phase,
                        format!("deleted survivor {} still exists in tree T", survivor.path),
                    ));
                }
                if !accounted_deleted_or_moved.insert(survivor.path.as_str()) {
                    return Err((
                        inv_phase,
                        format!(
                            "conflicting survivor dispositions for base path {}",
                            survivor.path
                        ),
                    ));
                }
            }
        }
    }

    // Completeness check: every path present in P^{tree} that disappeared in T
    // must be explicitly accounted for as Deleted or Renamed in the inventory.
    for base_path in p_entries.keys() {
        if !t_entries.contains_key(base_path)
            && !accounted_deleted_or_moved.contains(base_path.as_str())
        {
            return Err((
                inv_phase,
                format!(
                    "public base path {base_path} disappeared in tree T without a survivor inventory disposition"
                ),
            ));
        }
    }
    Ok(())
}

struct UniqueKeysOnly;

impl<'de> serde::de::DeserializeSeed<'de> for UniqueKeysOnly {
    type Value = ();

    fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        deserializer.deserialize_any(UniqueKeysVisitor)
    }
}

struct UniqueKeysVisitor;

impl<'de> serde::de::Visitor<'de> for UniqueKeysVisitor {
    type Value = ();

    fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("a JSON value without duplicate object keys")
    }

    fn visit_bool<E>(self, _: bool) -> Result<(), E> {
        Ok(())
    }
    fn visit_i64<E>(self, _: i64) -> Result<(), E> {
        Ok(())
    }
    fn visit_u64<E>(self, _: u64) -> Result<(), E> {
        Ok(())
    }
    fn visit_f64<E>(self, _: f64) -> Result<(), E> {
        Ok(())
    }
    fn visit_str<E>(self, _: &str) -> Result<(), E> {
        Ok(())
    }
    fn visit_unit<E>(self) -> Result<(), E> {
        Ok(())
    }
    fn visit_seq<A>(self, mut seq: A) -> Result<(), A::Error>
    where
        A: serde::de::SeqAccess<'de>,
    {
        while seq.next_element_seed(UniqueKeysOnly)?.is_some() {}
        Ok(())
    }
    fn visit_map<A>(self, mut map: A) -> Result<(), A::Error>
    where
        A: serde::de::MapAccess<'de>,
    {
        let mut seen = BTreeSet::new();
        while let Some(key) = map.next_key::<String>()? {
            if !seen.insert(key.clone()) {
                return Err(serde::de::Error::custom(format!(
                    "duplicate JSON object key: {key}"
                )));
            }
            map.next_value_seed(UniqueKeysOnly)?;
        }
        Ok(())
    }
}

fn parse_unique_json<T: for<'de> Deserialize<'de>>(raw: &str) -> Result<T, String> {
    let mut de = serde_json::Deserializer::from_str(raw);
    serde::de::DeserializeSeed::deserialize(UniqueKeysOnly, &mut de)
        .map_err(|err| format!("JSON rejected: {err}"))?;
    de.end()
        .map_err(|err| format!("JSON has trailing data: {err}"))?;
    serde_json::from_str(raw).map_err(|err| format!("decode JSON: {err}"))
}

pub(crate) fn parse_staged_promotion_packet(raw: &str) -> Result<StagedPromotionPacket, String> {
    parse_unique_json(raw)
}

pub(crate) fn source_promotion_verify_staged_packet(args: &[String]) -> Result<(), String> {
    if args.first().map(String::as_str) != Some(SOURCE_PROMOTION_VERIFY_STAGED_PACKET_SUBCOMMAND) {
        return Err(format!(
            "expected subcommand {SOURCE_PROMOTION_VERIFY_STAGED_PACKET_SUBCOMMAND}"
        ));
    }
    let mut packet_path: Option<PathBuf> = None;
    let mut receipt_path: Option<PathBuf> = None;
    let mut repo_path: Option<PathBuf> = None;
    let mut expected_p: Option<String> = None;
    let mut expected_s: Option<String> = None;
    let mut expected_k: Option<String> = None;
    let mut expected_t: Option<String> = None;
    let mut out_dir: Option<PathBuf> = None;
    let mut idx = 1;
    while idx < args.len() {
        match args[idx].as_str() {
            "--packet" => {
                idx += 1;
                let value = args
                    .get(idx)
                    .ok_or_else(|| "missing value for --packet".to_string())?;
                packet_path = Some(PathBuf::from(value));
            }
            "--qualification-receipt" => {
                idx += 1;
                let value = args
                    .get(idx)
                    .ok_or_else(|| "missing value for --qualification-receipt".to_string())?;
                receipt_path = Some(PathBuf::from(value));
            }
            "--repo" => {
                idx += 1;
                let value = args
                    .get(idx)
                    .ok_or_else(|| "missing value for --repo".to_string())?;
                repo_path = Some(PathBuf::from(value));
            }
            "--expected-p" => {
                idx += 1;
                let value = args
                    .get(idx)
                    .ok_or_else(|| "missing value for --expected-p".to_string())?;
                expected_p = Some(value.clone());
            }
            "--expected-s" => {
                idx += 1;
                let value = args
                    .get(idx)
                    .ok_or_else(|| "missing value for --expected-s".to_string())?;
                expected_s = Some(value.clone());
            }
            "--expected-k" => {
                idx += 1;
                let value = args
                    .get(idx)
                    .ok_or_else(|| "missing value for --expected-k".to_string())?;
                expected_k = Some(value.clone());
            }
            "--expected-t" => {
                idx += 1;
                let value = args
                    .get(idx)
                    .ok_or_else(|| "missing value for --expected-t".to_string())?;
                expected_t = Some(value.clone());
            }
            "--out" => {
                idx += 1;
                let value = args
                    .get(idx)
                    .ok_or_else(|| "missing value for --out".to_string())?;
                out_dir = Some(PathBuf::from(value));
            }
            other => {
                return Err(format!(
                    "unknown argument for source-promotion {SOURCE_PROMOTION_VERIFY_STAGED_PACKET_SUBCOMMAND}: {other}"
                ));
            }
        }
        idx += 1;
    }
    let packet_path = packet_path.ok_or_else(|| {
        "usage: cargo xtask source-promotion verify-staged-packet --packet <packet.json> [--qualification-receipt <receipt.json> --repo <path> --expected-p <sha> --expected-s <sha> --expected-k <sha> --expected-t <sha>] [--out <dir>]"
            .to_string()
    })?;
    let any_independent = receipt_path.is_some()
        || repo_path.is_some()
        || expected_p.is_some()
        || expected_s.is_some()
        || expected_k.is_some()
        || expected_t.is_some();
    let all_independent = receipt_path.is_some()
        && repo_path.is_some()
        && expected_p.is_some()
        && expected_s.is_some()
        && expected_k.is_some()
        && expected_t.is_some();
    if any_independent && !all_independent {
        return Err(
            "independent verification requires --qualification-receipt, --repo, --expected-p, --expected-s, --expected-k, and --expected-t together"
                .to_string(),
        );
    }
    if let Some(dir) = out_dir.as_deref() {
        let stale_path = dir.join(REPORT_JSON);
        if let Err(err) = fs::remove_file(&stale_path)
            && err.kind() != std::io::ErrorKind::NotFound
        {
            return Err(format!(
                "remove stale staged verdict {}: {err}",
                stale_path.display()
            ));
        }
    }
    let raw = fs::read_to_string(&packet_path)
        .map_err(|err| format!("read staged packet {}: {err}", packet_path.display()))?;
    let packet = parse_staged_promotion_packet(&raw)
        .map_err(|err| format!("{}: {err}", packet_path.display()))?;
    let (verdict, expected_ok_status) = match (
        receipt_path.as_deref(),
        repo_path.as_deref(),
        expected_p,
        expected_s,
        expected_k,
        expected_t,
    ) {
        (Some(receipt), Some(repo), Some(p), Some(s), Some(k), Some(t)) => {
            let subject = ExpectedStagedSubject {
                public_base_p: p,
                swarm_return_s: s,
                github_preview_k: k,
                qualified_tree_t: t,
            };
            (
                evaluate_staged_promotion_independently(&packet, &subject, receipt, repo),
                "admitted",
            )
        }
        _ => (
            evaluate_staged_promotion_packet(&packet),
            "packet_consistent",
        ),
    };
    if let Some(dir) = out_dir {
        write_verdict(&dir, &verdict)?;
    }
    if verdict.status != expected_ok_status {
        return Err(format!(
            "staged source-promotion packet rejected [{}]: {}",
            verdict.phase, verdict.reason
        ));
    }
    Ok(())
}

fn write_verdict(dir: &Path, verdict: &StagedPromotionConsumerVerdict) -> Result<(), String> {
    fs::create_dir_all(dir).map_err(|err| format!("create output dir {}: {err}", dir.display()))?;
    let rendered = serde_json::to_string_pretty(verdict)
        .map_err(|err| format!("serialize staged verdict: {err}"))?;
    let out_path = dir.join(REPORT_JSON);
    fs::write(&out_path, format!("{rendered}\n"))
        .map_err(|err| format!("write {}: {err}", out_path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    const P: &str = "1111111111111111111111111111111111111111";
    const S: &str = "2222222222222222222222222222222222222222";
    const T: &str = "3333333333333333333333333333333333333333";
    const K: &str = "4444444444444444444444444444444444444444";
    const A: &str = "5555555555555555555555555555555555555555";
    const IMPORT: &str = "6666666666666666666666666666666666666666";
    const BLOB: &str = "7777777777777777777777777777777777777777";
    const RECEIPT_DIGEST: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

    fn valid_packet() -> Result<StagedPromotionPacket, String> {
        let survivors = vec![
            PublicSurvivorEntry {
                path: "docs/PUBLIC_NOTICE.md".to_string(),
                mode: "100644".to_string(),
                object_sha: Some(BLOB.to_string()),
                change_kind: SurvivorChangeKind::Retained,
                previous_path: None,
                nested_import_commit: IMPORT.to_string(),
            },
            PublicSurvivorEntry {
                path: "docs/RENAMED.md".to_string(),
                mode: "100644".to_string(),
                object_sha: Some(BLOB.to_string()),
                change_kind: SurvivorChangeKind::Renamed,
                previous_path: Some("docs/OLD.md".to_string()),
                nested_import_commit: IMPORT.to_string(),
            },
            PublicSurvivorEntry {
                path: "docs/OBSOLETE.md".to_string(),
                mode: "100644".to_string(),
                object_sha: None,
                change_kind: SurvivorChangeKind::Deleted,
                previous_path: None,
                nested_import_commit: IMPORT.to_string(),
            },
        ];
        let nested_imports = vec![NestedImportEntry {
            import_commit: IMPORT.to_string(),
            source_public_base: P.to_string(),
            tree_sha: T.to_string(),
        }];
        let public_survivors_sha256 = compute_survivors_digest(&survivors)?;
        let nested_import_sha256 = compute_nested_imports_digest(&nested_imports)?;
        Ok(StagedPromotionPacket {
            schema: STAGED_PACKET_SCHEMA.to_string(),
            public_base_p: P.to_string(),
            swarm_return_s: S.to_string(),
            swarm_tree_s: T.to_string(),
            qualified_tree_t: T.to_string(),
            p_is_ancestor_of_s: true,
            github_preview_k: PreviewMergeK {
                commit_sha: K.to_string(),
                parents: vec![P.to_string(), S.to_string()],
                tree: T.to_string(),
            },
            observed_public_a: None,
            qualification: CrossRepoQualification {
                issuer_repository: EXPECTED_ISSUER_REPOSITORY.to_string(),
                authority_kind: EXPECTED_AUTHORITY_KIND.to_string(),
                instrument: EXPECTED_QUALIFICATION_INSTRUMENT.to_string(),
                schema: EXPECTED_QUALIFICATION_SCHEMA.to_string(),
                receipt_id: "swarm-2769-0.11.0".to_string(),
                receipt_sha256: RECEIPT_DIGEST.to_string(),
                qualified_s: S.to_string(),
                qualified_t: T.to_string(),
                mandatory_commands_executed: 12,
                mandatory_commands_skipped: 0,
                stale: false,
            },
            inventory: SurvivorAndImportInventory {
                public_survivors_sha256,
                nested_import_sha256,
                survivors,
                nested_imports,
                analysis_disposition: WholeDiffAnalysisDisposition::LimitedUnavailable {
                    reason: "whole-diff analysis exceeds bounded single-diff cap; bounded inventory retained".to_string(),
                },
            },
        })
    }

    struct TempFixture {
        root: PathBuf,
    }

    impl TempFixture {
        fn new(label: &str) -> Result<Self, String> {
            let nanos = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_err(|err| err.to_string())?
                .as_nanos();
            let root = std::env::temp_dir().join(format!(
                "ripr-staged-promo-{label}-{}-{nanos}",
                std::process::id()
            ));
            fs::create_dir_all(&root).map_err(|err| err.to_string())?;
            Ok(Self { root })
        }
    }

    impl Drop for TempFixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.root);
        }
    }

    fn git_fixture_cmd(repo: &Path, args: &[&str]) -> Result<String, String> {
        let mut cmd = Command::new("git");
        for key in [
            "GIT_DIR",
            "GIT_WORK_TREE",
            "GIT_INDEX_FILE",
            "GIT_COMMON_DIR",
            "GIT_OBJECT_DIRECTORY",
            "GIT_ALTERNATE_OBJECT_DIRECTORIES",
            "GIT_CONFIG_PARAMETERS",
            "GIT_CONFIG_COUNT",
        ] {
            cmd.env_remove(key);
        }
        let null_config = if cfg!(windows) { "NUL" } else { "/dev/null" };
        cmd.arg("-C")
            .arg(repo)
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env("GIT_CONFIG_GLOBAL", null_config)
            .env("GIT_AUTHOR_NAME", "Test")
            .env("GIT_AUTHOR_EMAIL", "test@example.com")
            .env("GIT_COMMITTER_NAME", "Test")
            .env("GIT_COMMITTER_EMAIL", "test@example.com")
            .env("GIT_AUTHOR_DATE", "2026-11-21T00:00:00Z")
            .env("GIT_COMMITTER_DATE", "2026-11-21T00:00:00Z");
        for arg in args {
            cmd.arg(arg);
        }
        let out = cmd.output().map_err(|err| err.to_string())?;
        if !out.status.success() {
            return Err(format!(
                "git {} failed: {}",
                args.join(" "),
                String::from_utf8_lossy(&out.stderr)
            ));
        }
        String::from_utf8(out.stdout)
            .map(|s| s.trim().to_string())
            .map_err(|err| err.to_string())
    }

    fn build_real_git_fixture(
        label: &str,
    ) -> Result<(TempFixture, StagedPromotionPacket, PathBuf), String> {
        let fixture = TempFixture::new(label)?;
        let repo = fixture.root.join("repo");
        fs::create_dir_all(&repo).map_err(|err| err.to_string())?;
        git_fixture_cmd(&repo, &["init", "-q", "--object-format=sha1"])?;

        // Base commit P with 3 public files: PUBLIC_NOTICE.md (retained), OLD.md (renamed), OBSOLETE.md (deleted).
        fs::create_dir_all(repo.join("docs")).map_err(|err| err.to_string())?;
        fs::write(repo.join("docs/PUBLIC_NOTICE.md"), "public notice\n")
            .map_err(|err| err.to_string())?;
        fs::write(repo.join("docs/OLD.md"), "renamed content\n").map_err(|err| err.to_string())?;
        fs::write(repo.join("docs/OBSOLETE.md"), "obsolete\n").map_err(|err| err.to_string())?;
        git_fixture_cmd(&repo, &["add", "."])?;
        git_fixture_cmd(&repo, &["commit", "-q", "-m", "public base P"])?;
        let p = git_fixture_cmd(&repo, &["rev-parse", "HEAD"])?;

        // Prior swarm commit on a separate lineage so the nested public import is a real 2-parent merge commit [swarm_prior, P].
        let swarm_prior = git_fixture_cmd(
            &repo,
            &[
                "commit-tree",
                &git_fixture_cmd(&repo, &["rev-parse", "HEAD^{tree}"])?,
                "-m",
                "swarm prior",
            ],
        )?;

        // Nested import tree (retains PUBLIC_NOTICE.md, renames OLD.md -> RENAMED.md, deletes OBSOLETE.md).
        fs::write(repo.join("docs/RENAMED.md"), "renamed content\n")
            .map_err(|err| err.to_string())?;
        fs::remove_file(repo.join("docs/OLD.md")).map_err(|err| err.to_string())?;
        fs::remove_file(repo.join("docs/OBSOLETE.md")).map_err(|err| err.to_string())?;
        git_fixture_cmd(&repo, &["add", "-A"])?;
        let import_tree = git_fixture_cmd(&repo, &["write-tree"])?;
        let import_commit = git_fixture_cmd(
            &repo,
            &[
                "commit-tree",
                &import_tree,
                "-p",
                &swarm_prior,
                "-p",
                &p,
                "-m",
                "nested public import",
            ],
        )?;

        // Swarm return head S descending from import_commit (and thus P).
        fs::write(repo.join("docs/SWARM_WORK.md"), "swarm feature\n")
            .map_err(|err| err.to_string())?;
        git_fixture_cmd(&repo, &["add", "."])?;
        let t = git_fixture_cmd(&repo, &["write-tree"])?;
        let s = git_fixture_cmd(
            &repo,
            &[
                "commit-tree",
                &t,
                "-p",
                &import_commit,
                "-m",
                "swarm return S",
            ],
        )?;

        // GitHub merge preview K with exact parents [P, S] and tree T.
        let k = git_fixture_cmd(
            &repo,
            &["commit-tree", &t, "-p", &p, "-p", &s, "-m", "preview K"],
        )?;
        // Observed public merge A with exact parents [P, S] and tree T.
        let a = git_fixture_cmd(
            &repo,
            &["commit-tree", &t, "-p", &p, "-p", &s, "-m", "merge A"],
        )?;

        let notice_sha =
            git_fixture_cmd(&repo, &["rev-parse", &format!("{t}:docs/PUBLIC_NOTICE.md")])?;
        let renamed_sha = git_fixture_cmd(&repo, &["rev-parse", &format!("{t}:docs/RENAMED.md")])?;

        let receipt = ExternalQualificationReceipt {
            schema: EXPECTED_QUALIFICATION_SCHEMA.to_string(),
            issuer_repository: EXPECTED_ISSUER_REPOSITORY.to_string(),
            authority_kind: EXPECTED_AUTHORITY_KIND.to_string(),
            instrument: EXPECTED_QUALIFICATION_INSTRUMENT.to_string(),
            receipt_id: "swarm-2769-0.11.0".to_string(),
            qualified_s: s.clone(),
            qualified_t: t.clone(),
            outcome: "passed".to_string(),
            mandatory_commands_executed: 12,
            mandatory_commands_skipped: 0,
            stale: false,
        };
        let receipt_bytes = serde_json::to_vec_pretty(&receipt).map_err(|err| err.to_string())?;
        let receipt_sha256 = sha256_hex(&receipt_bytes);
        let receipt_path = fixture.root.join("qualification-receipt.json");
        fs::write(&receipt_path, &receipt_bytes).map_err(|err| err.to_string())?;

        let survivors = vec![
            PublicSurvivorEntry {
                path: "docs/PUBLIC_NOTICE.md".to_string(),
                mode: "100644".to_string(),
                object_sha: Some(notice_sha),
                change_kind: SurvivorChangeKind::Retained,
                previous_path: None,
                nested_import_commit: import_commit.clone(),
            },
            PublicSurvivorEntry {
                path: "docs/RENAMED.md".to_string(),
                mode: "100644".to_string(),
                object_sha: Some(renamed_sha),
                change_kind: SurvivorChangeKind::Renamed,
                previous_path: Some("docs/OLD.md".to_string()),
                nested_import_commit: import_commit.clone(),
            },
            PublicSurvivorEntry {
                path: "docs/OBSOLETE.md".to_string(),
                mode: "100644".to_string(),
                object_sha: None,
                change_kind: SurvivorChangeKind::Deleted,
                previous_path: None,
                nested_import_commit: import_commit.clone(),
            },
        ];
        let nested_imports = vec![NestedImportEntry {
            import_commit,
            source_public_base: p.clone(),
            tree_sha: import_tree,
        }];
        let packet = StagedPromotionPacket {
            schema: STAGED_PACKET_SCHEMA.to_string(),
            public_base_p: p.clone(),
            swarm_return_s: s.clone(),
            swarm_tree_s: t.clone(),
            qualified_tree_t: t.clone(),
            p_is_ancestor_of_s: true,
            github_preview_k: PreviewMergeK {
                commit_sha: k,
                parents: vec![p.clone(), s.clone()],
                tree: t.clone(),
            },
            observed_public_a: Some(ObservedPublicMergeA {
                commit_sha: a.clone(),
                parents: vec![p, s.clone()],
                tree: t.clone(),
                current_main_sha: a,
            }),
            qualification: CrossRepoQualification {
                issuer_repository: EXPECTED_ISSUER_REPOSITORY.to_string(),
                authority_kind: EXPECTED_AUTHORITY_KIND.to_string(),
                instrument: EXPECTED_QUALIFICATION_INSTRUMENT.to_string(),
                schema: EXPECTED_QUALIFICATION_SCHEMA.to_string(),
                receipt_id: "swarm-2769-0.11.0".to_string(),
                receipt_sha256,
                qualified_s: s,
                qualified_t: t,
                mandatory_commands_executed: 12,
                mandatory_commands_skipped: 0,
                stale: false,
            },
            inventory: SurvivorAndImportInventory {
                public_survivors_sha256: compute_survivors_digest(&survivors)?,
                nested_import_sha256: compute_nested_imports_digest(&nested_imports)?,
                survivors,
                nested_imports,
                analysis_disposition: WholeDiffAnalysisDisposition::LimitedUnavailable {
                    reason: "whole-diff analysis bounded; independent Git inventory verified"
                        .to_string(),
                },
            },
        };
        Ok((fixture, packet, receipt_path))
    }

    #[test]
    fn valid_preview_and_post_merge_packets_are_admitted_preserving_limited_disposition()
    -> Result<(), String> {
        let mut packet = valid_packet()?;
        let preview_verdict = evaluate_staged_promotion_packet(&packet);
        assert_eq!(
            preview_verdict.status, "packet_consistent",
            "{preview_verdict:?}"
        );
        assert_eq!(
            preview_verdict.qualification_authority,
            EXPECTED_AUTHORITY_KIND
        );
        assert!(matches!(
            preview_verdict.analysis_disposition,
            WholeDiffAnalysisDisposition::LimitedUnavailable { .. }
        ));

        packet.observed_public_a = Some(ObservedPublicMergeA {
            commit_sha: A.to_string(),
            parents: vec![P.to_string(), S.to_string()],
            tree: T.to_string(),
            current_main_sha: A.to_string(),
        });
        let post_verdict = evaluate_staged_promotion_packet(&packet);
        assert_eq!(post_verdict.status, "packet_consistent", "{post_verdict:?}");
        assert_eq!(
            post_verdict.identities.observed_public_a.as_deref(),
            Some(A)
        );
        Ok(())
    }

    #[test]
    fn geometry_negatives_fail_closed() -> Result<(), String> {
        let mut stale_p = valid_packet()?;
        stale_p.p_is_ancestor_of_s = false;
        let v = evaluate_staged_promotion_packet(&stale_p);
        assert_eq!(v.status, "rejected");
        assert_eq!(v.phase, "geometry");

        let mut swapped = valid_packet()?;
        swapped.github_preview_k.parents = vec![S.to_string(), P.to_string()];
        let v = evaluate_staged_promotion_packet(&swapped);
        assert_eq!(v.status, "rejected");
        assert_eq!(v.phase, "geometry");

        let mut extra = valid_packet()?;
        extra.github_preview_k.parents.push(A.to_string());
        let v = evaluate_staged_promotion_packet(&extra);
        assert_eq!(v.status, "rejected");
        assert_eq!(v.phase, "geometry");

        let mut wrapper = valid_packet()?;
        wrapper.github_preview_k.tree = A.to_string();
        let v = evaluate_staged_promotion_packet(&wrapper);
        assert_eq!(v.status, "rejected");
        assert!(v.reason.contains("wrapper/copy-tree"), "{v:?}");

        let mut moved_main = valid_packet()?;
        moved_main.observed_public_a = Some(ObservedPublicMergeA {
            commit_sha: A.to_string(),
            parents: vec![P.to_string(), S.to_string()],
            tree: T.to_string(),
            current_main_sha: K.to_string(),
        });
        let v = evaluate_staged_promotion_packet(&moved_main);
        assert_eq!(v.status, "rejected");
        assert_eq!(v.phase, "post_merge_a");
        Ok(())
    }

    #[test]
    fn issuer_instrument_and_execution_negatives_fail_closed() -> Result<(), String> {
        let mut foreign = valid_packet()?;
        foreign.qualification.issuer_repository = "EvilOrg/fork".to_string();
        assert_eq!(
            evaluate_staged_promotion_packet(&foreign).phase,
            "qualification_issuer"
        );

        let mut renamed_native = valid_packet()?;
        renamed_native.qualification.authority_kind = "native_source_proof".to_string();
        assert_eq!(
            evaluate_staged_promotion_packet(&renamed_native).phase,
            "qualification_issuer"
        );

        let mut old_schema = valid_packet()?;
        old_schema.qualification.schema = "ripr.swarm_qualification_receipt.v0".to_string();
        assert_eq!(
            evaluate_staged_promotion_packet(&old_schema).phase,
            "qualification_issuer"
        );

        let mut stale = valid_packet()?;
        stale.qualification.stale = true;
        assert_eq!(
            evaluate_staged_promotion_packet(&stale).phase,
            "qualification_issuer"
        );

        let mut wrong_st = valid_packet()?;
        wrong_st.qualification.qualified_t = A.to_string();
        assert_eq!(
            evaluate_staged_promotion_packet(&wrong_st).phase,
            "qualification_issuer"
        );

        let mut zero_cmd = valid_packet()?;
        zero_cmd.qualification.mandatory_commands_executed = 0;
        assert_eq!(
            evaluate_staged_promotion_packet(&zero_cmd).phase,
            "qualification_execution"
        );

        let mut skipped_cmd = valid_packet()?;
        skipped_cmd.qualification.mandatory_commands_skipped = 1;
        assert_eq!(
            evaluate_staged_promotion_packet(&skipped_cmd).phase,
            "qualification_execution"
        );
        Ok(())
    }

    #[test]
    fn inventory_survivor_mode_and_nested_import_negatives_fail_closed() -> Result<(), String> {
        let mut tampered = valid_packet()?;
        tampered.inventory.public_survivors_sha256 =
            "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb".to_string();
        let v = evaluate_staged_promotion_packet(&tampered);
        assert_eq!(v.status, "rejected");
        assert_eq!(v.phase, "inventory");

        let mut bad_mode = valid_packet()?;
        bad_mode.inventory.survivors[0].mode = "100600".to_string();
        bad_mode.inventory.public_survivors_sha256 =
            compute_survivors_digest(&bad_mode.inventory.survivors)?;
        let v = evaluate_staged_promotion_packet(&bad_mode);
        assert_eq!(v.status, "rejected");
        assert!(v.reason.contains("invalid Git file mode"), "{v:?}");

        let mut no_import = valid_packet()?;
        no_import.inventory.nested_imports.clear();
        no_import.inventory.nested_import_sha256 =
            compute_nested_imports_digest(&no_import.inventory.nested_imports)?;
        let v = evaluate_staged_promotion_packet(&no_import);
        assert_eq!(v.status, "rejected");
        assert!(v.reason.contains("nested public-import"), "{v:?}");

        let mut bad_rename = valid_packet()?;
        bad_rename.inventory.survivors[1].previous_path = None;
        bad_rename.inventory.public_survivors_sha256 =
            compute_survivors_digest(&bad_rename.inventory.survivors)?;
        assert_eq!(
            evaluate_staged_promotion_packet(&bad_rename).status,
            "rejected"
        );

        let mut bad_delete = valid_packet()?;
        bad_delete.inventory.survivors[2].object_sha = Some(BLOB.to_string());
        bad_delete.inventory.public_survivors_sha256 =
            compute_survivors_digest(&bad_delete.inventory.survivors)?;
        assert_eq!(
            evaluate_staged_promotion_packet(&bad_delete).status,
            "rejected"
        );

        let mut empty_reason = valid_packet()?;
        empty_reason.inventory.analysis_disposition =
            WholeDiffAnalysisDisposition::LimitedUnavailable {
                reason: "   ".to_string(),
            };
        assert_eq!(
            evaluate_staged_promotion_packet(&empty_reason).status,
            "rejected"
        );
        Ok(())
    }

    #[test]
    fn duplicate_json_keys_outer_and_nested_are_rejected() -> Result<(), String> {
        let packet = valid_packet()?;
        let valid_json = serde_json::to_string(&packet).map_err(|err| err.to_string())?;
        let _ = parse_staged_promotion_packet(&valid_json)?;

        let outer_dup = format!(
            "{{\"schema\":\"{STAGED_PACKET_SCHEMA}\",\"schema\":\"{STAGED_PACKET_SCHEMA}\"}}"
        );
        let Err(err) = parse_staged_promotion_packet(&outer_dup) else {
            return Err("outer duplicate key must be rejected".to_string());
        };
        assert!(err.contains("duplicate JSON object key: schema"), "{err}");

        let nested_dup =
            valid_json.replacen("\"stale\":false", "\"stale\":true,\"stale\":false", 1);
        let Err(err) = parse_staged_promotion_packet(&nested_dup) else {
            return Err("nested duplicate key must be rejected".to_string());
        };
        assert!(err.contains("duplicate JSON object key: stale"), "{err}");
        Ok(())
    }

    #[test]
    fn independent_receipt_and_real_git_verification_admits_and_rejects_adversaries()
    -> Result<(), String> {
        let (fixture, packet, receipt_path) = build_real_git_fixture("phase2-admit")?;
        let repo = fixture.root.join("repo");
        let subject_of = |pkt: &StagedPromotionPacket| ExpectedStagedSubject {
            public_base_p: pkt.public_base_p.clone(),
            swarm_return_s: pkt.swarm_return_s.clone(),
            github_preview_k: pkt.github_preview_k.commit_sha.clone(),
            qualified_tree_t: pkt.qualified_tree_t.clone(),
        };
        let live_subject = subject_of(&packet);

        // Positive control: valid real Git objects + valid receipt bytes + matching live subject -> admitted.
        let verdict =
            evaluate_staged_promotion_independently(&packet, &live_subject, &receipt_path, &repo);
        assert_eq!(verdict.status, "admitted", "{verdict:?}");
        assert_eq!(
            verdict.verification_scope,
            "independent_receipt_bytes_and_git_verified"
        );
        assert!(
            verdict
                .claim
                .contains("not external API/cryptographic producer attestation"),
            "{}",
            verdict.claim
        );

        // Negative 0 (wrong-subject binding): valid packet+receipt+objects for a DIFFERENT return candidate -> rejected.
        let mut wrong_subject = live_subject.clone();
        wrong_subject.swarm_return_s = A.to_string();
        let v =
            evaluate_staged_promotion_independently(&packet, &wrong_subject, &receipt_path, &repo);
        assert_eq!(v.status, "rejected");
        assert_eq!(v.phase, "subject_binding");

        // Negative 1: tampered receipt bytes on disk -> rejected.
        let tampered_receipt = fixture.root.join("tampered-receipt.json");
        let mut bytes = fs::read(&receipt_path).map_err(|err| err.to_string())?;
        bytes.push(b' ');
        fs::write(&tampered_receipt, &bytes).map_err(|err| err.to_string())?;
        let v = evaluate_staged_promotion_independently(
            &packet,
            &live_subject,
            &tampered_receipt,
            &repo,
        );
        assert_eq!(v.status, "rejected");
        assert_eq!(v.phase, "qualification_receipt_custody");
        assert!(v.reason.contains("SHA-256 mismatch"), "{v:?}");

        // Negative 2: external receipt with failed outcome (even with matching digest!) -> rejected.
        let mut failed_receipt_val: ExternalQualificationReceipt =
            serde_json::from_slice(&fs::read(&receipt_path).map_err(|err| err.to_string())?)
                .map_err(|err| err.to_string())?;
        failed_receipt_val.outcome = "failed".to_string();
        let failed_bytes =
            serde_json::to_vec_pretty(&failed_receipt_val).map_err(|err| err.to_string())?;
        let failed_path = fixture.root.join("failed-receipt.json");
        fs::write(&failed_path, &failed_bytes).map_err(|err| err.to_string())?;
        let mut packet_failed_receipt = packet.clone();
        packet_failed_receipt.qualification.receipt_sha256 = sha256_hex(&failed_bytes);
        let v = evaluate_staged_promotion_independently(
            &packet_failed_receipt,
            &subject_of(&packet_failed_receipt),
            &failed_path,
            &repo,
        );
        assert_eq!(v.status, "rejected");
        assert_eq!(v.phase, "qualification_receipt_custody");
        assert!(v.reason.contains("outcome is not passed"), "{v:?}");

        // Negative 3: spoofed survivor inventory omitting deleted docs/OBSOLETE.md -> rejected.
        let mut spoofed = packet.clone();
        spoofed.inventory.survivors.pop();
        spoofed.inventory.public_survivors_sha256 =
            compute_survivors_digest(&spoofed.inventory.survivors)?;
        let v = evaluate_staged_promotion_independently(
            &spoofed,
            &subject_of(&spoofed),
            &receipt_path,
            &repo,
        );
        assert_eq!(v.status, "rejected");
        assert_eq!(v.phase, "git_inventory");
        assert!(v.reason.contains("docs/OBSOLETE.md"), "{v:?}");

        // Negative 3b: non-import ordinary single-parent descendant (S) masquerading as nested import -> rejected.
        let mut fake_import = packet.clone();
        fake_import.inventory.nested_imports[0].import_commit = packet.swarm_return_s.clone();
        fake_import.inventory.nested_imports[0].tree_sha = packet.qualified_tree_t.clone();
        for surv in &mut fake_import.inventory.survivors {
            surv.nested_import_commit = packet.swarm_return_s.clone();
        }
        fake_import.inventory.nested_import_sha256 =
            compute_nested_imports_digest(&fake_import.inventory.nested_imports)?;
        fake_import.inventory.public_survivors_sha256 =
            compute_survivors_digest(&fake_import.inventory.survivors)?;
        let v = evaluate_staged_promotion_independently(
            &fake_import,
            &subject_of(&fake_import),
            &receipt_path,
            &repo,
        );
        assert_eq!(v.status, "rejected");
        assert_eq!(v.phase, "git_inventory");
        assert!(v.reason.contains("must be a merge commit"), "{v:?}");

        // Negative 3c: deleted survivor claiming wrong pre-deletion mode against P -> rejected.
        let mut bad_del_mode = packet.clone();
        bad_del_mode.inventory.survivors[2].mode = "100755".to_string();
        bad_del_mode.inventory.public_survivors_sha256 =
            compute_survivors_digest(&bad_del_mode.inventory.survivors)?;
        let v = evaluate_staged_promotion_independently(
            &bad_del_mode,
            &subject_of(&bad_del_mode),
            &receipt_path,
            &repo,
        );
        assert_eq!(v.status, "rejected");
        assert_eq!(v.phase, "git_inventory");
        assert!(
            v.reason.contains("mode disagrees with base tree P"),
            "{v:?}"
        );

        // Negative 3d: swarm-only file (absent from P^{tree}) declared as Retained survivor -> rejected.
        let swarm_blob = git_fixture_cmd(
            &repo,
            &[
                "rev-parse",
                &format!("{}:docs/SWARM_WORK.md", packet.swarm_return_s),
            ],
        )?;
        let mut swarm_as_retained = packet.clone();
        swarm_as_retained
            .inventory
            .survivors
            .push(PublicSurvivorEntry {
                path: "docs/SWARM_WORK.md".to_string(),
                mode: "100644".to_string(),
                object_sha: Some(swarm_blob),
                change_kind: SurvivorChangeKind::Retained,
                previous_path: None,
                nested_import_commit: packet.inventory.nested_imports[0].import_commit.clone(),
            });
        swarm_as_retained.inventory.public_survivors_sha256 =
            compute_survivors_digest(&swarm_as_retained.inventory.survivors)?;
        let v = evaluate_staged_promotion_independently(
            &swarm_as_retained,
            &subject_of(&swarm_as_retained),
            &receipt_path,
            &repo,
        );
        assert_eq!(v.status, "rejected");
        assert_eq!(v.phase, "git_inventory");
        assert!(v.reason.contains("was not present in base tree P"), "{v:?}");

        // Negative 3e: base path docs/OLD.md claimed as both Renamed previous_path and Deleted -> rejected.
        let mut conflicting_disp = packet.clone();
        conflicting_disp
            .inventory
            .survivors
            .push(PublicSurvivorEntry {
                path: "docs/OLD.md".to_string(),
                mode: "100644".to_string(),
                object_sha: None,
                change_kind: SurvivorChangeKind::Deleted,
                previous_path: None,
                nested_import_commit: packet.inventory.nested_imports[0].import_commit.clone(),
            });
        conflicting_disp.inventory.public_survivors_sha256 =
            compute_survivors_digest(&conflicting_disp.inventory.survivors)?;
        let v = evaluate_staged_promotion_independently(
            &conflicting_disp,
            &subject_of(&conflicting_disp),
            &receipt_path,
            &repo,
        );
        assert_eq!(v.status, "rejected");
        assert_eq!(v.phase, "git_inventory");
        assert!(
            v.reason
                .contains("conflicting survivor dispositions for base path docs/OLD.md"),
            "{v:?}"
        );

        // Negative 4: swapped K parents [S, P] in real Git object store -> rejected.
        let swapped_k = git_fixture_cmd(
            &repo,
            &[
                "commit-tree",
                &packet.qualified_tree_t,
                "-p",
                &packet.swarm_return_s,
                "-p",
                &packet.public_base_p,
                "-m",
                "swapped K",
            ],
        )?;
        let mut packet_swapped = packet.clone();
        packet_swapped.github_preview_k.commit_sha = swapped_k;
        let v = evaluate_staged_promotion_independently(
            &packet_swapped,
            &subject_of(&packet_swapped),
            &receipt_path,
            &repo,
        );
        assert_eq!(v.status, "rejected");
        assert_eq!(v.phase, "git_geometry");
        assert!(v.reason.contains("parents mismatch"), "{v:?}");

        // Negative 5: real Git moved base P (orphan commit not ancestor of S) -> rejected.
        let moved_p = git_fixture_cmd(
            &repo,
            &[
                "commit-tree",
                &packet.qualified_tree_t,
                "-m",
                "moved base P",
            ],
        )?;
        let mut packet_moved_p = packet.clone();
        packet_moved_p.public_base_p = moved_p.clone();
        packet_moved_p.github_preview_k.parents[0] = moved_p.clone();
        if let Some(a) = &mut packet_moved_p.observed_public_a {
            a.parents[0] = moved_p;
        }
        let v = evaluate_staged_promotion_independently(
            &packet_moved_p,
            &subject_of(&packet_moved_p),
            &receipt_path,
            &repo,
        );
        assert_eq!(v.status, "rejected");
        assert_eq!(v.phase, "git_geometry");
        assert!(v.reason.contains("not an ancestor"), "{v:?}");

        // Negative 6: stale verdict file in --out dir is removed before early parse failure.
        let out_dir = fixture.root.join("verdict-out");
        fs::create_dir_all(&out_dir).map_err(|err| err.to_string())?;
        let stale_verdict = out_dir.join(REPORT_JSON);
        fs::write(&stale_verdict, "{\"status\":\"admitted\"}\n").map_err(|err| err.to_string())?;
        let bad_packet = fixture.root.join("malformed-packet.json");
        fs::write(&bad_packet, "{not-valid-json").map_err(|err| err.to_string())?;
        let cli_args = vec![
            "verify-staged-packet".to_string(),
            "--packet".to_string(),
            bad_packet.to_string_lossy().into_owned(),
            "--out".to_string(),
            out_dir.to_string_lossy().into_owned(),
        ];
        let Err(err) = source_promotion_verify_staged_packet(&cli_args) else {
            return Err("malformed packet unexpectedly succeeded".to_string());
        };
        assert!(err.contains("malformed-packet.json"), "{err}");
        assert!(
            !stale_verdict.exists(),
            "stale verdict file must be removed on early failure"
        );

        Ok(())
    }
}
