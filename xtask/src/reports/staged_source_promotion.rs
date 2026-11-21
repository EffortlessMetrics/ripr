//! #1627 Phase 1: Versioned staged P/S/T/K/A source-promotion admission
//! contract and typed required-result consumer verdict.
//!
//! This module validates the staged public-to-swarm-to-public return packet
//! without mutating any repository ref, raising analysis limits, or renaming
//! cross-repository swarm qualification as native source proof.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

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

fn reject(
    packet: &StagedPromotionPacket,
    phase: &str,
    reason: impl Into<String>,
) -> StagedPromotionConsumerVerdict {
    StagedPromotionConsumerVerdict {
        schema: STAGED_VERDICT_SCHEMA.to_string(),
        status: "rejected".to_string(),
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

/// Evaluate a staged P/S/T/K/A source-promotion packet for the required-result
/// consumer. Every failure mode fails closed with an explicit phase and reason.
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

    // Explicit cross-repository qualification issuer/instrument admission.
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

    // Public-survivor and nested-import inventory verification.
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
        status: "admitted".to_string(),
        phase: "complete".to_string(),
        reason: "staged_packet_admitted".to_string(),
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
        claim: "Staged admission verdict only; retains cross-repo qualification authority and bounded analysis disposition without authorizing merge, tag, or release publication.".to_string(),
    }
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

pub(crate) fn parse_staged_promotion_packet(raw: &str) -> Result<StagedPromotionPacket, String> {
    let mut de = serde_json::Deserializer::from_str(raw);
    serde::de::DeserializeSeed::deserialize(UniqueKeysOnly, &mut de)
        .map_err(|err| format!("staged packet JSON rejected: {err}"))?;
    de.end()
        .map_err(|err| format!("staged packet JSON has trailing data: {err}"))?;
    serde_json::from_str(raw).map_err(|err| format!("parse staged packet: {err}"))
}

pub(crate) fn source_promotion_verify_staged_packet(args: &[String]) -> Result<(), String> {
    if args.first().map(String::as_str) != Some(SOURCE_PROMOTION_VERIFY_STAGED_PACKET_SUBCOMMAND) {
        return Err(format!(
            "expected subcommand {SOURCE_PROMOTION_VERIFY_STAGED_PACKET_SUBCOMMAND}"
        ));
    }
    let mut packet_path: Option<PathBuf> = None;
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
        "usage: cargo xtask source-promotion verify-staged-packet --packet <packet.json> [--out <dir>]"
            .to_string()
    })?;
    let raw = fs::read_to_string(&packet_path)
        .map_err(|err| format!("read staged packet {}: {err}", packet_path.display()))?;
    let packet = parse_staged_promotion_packet(&raw)
        .map_err(|err| format!("{}: {err}", packet_path.display()))?;
    let verdict = evaluate_staged_promotion_packet(&packet);
    if let Some(dir) = out_dir {
        write_verdict(&dir, &verdict)?;
    }
    if verdict.status != "admitted" {
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

    #[test]
    fn valid_preview_and_post_merge_packets_are_admitted_preserving_limited_disposition()
    -> Result<(), String> {
        let mut packet = valid_packet()?;
        let preview_verdict = evaluate_staged_promotion_packet(&packet);
        assert_eq!(preview_verdict.status, "admitted", "{preview_verdict:?}");
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
        assert_eq!(post_verdict.status, "admitted", "{post_verdict:?}");
        assert_eq!(
            post_verdict.identities.observed_public_a.as_deref(),
            Some(A)
        );
        Ok(())
    }

    #[test]
    fn geometry_negatives_fail_closed() -> Result<(), String> {
        // Stale P (not ancestor of S)
        let mut stale_p = valid_packet()?;
        stale_p.p_is_ancestor_of_s = false;
        let v = evaluate_staged_promotion_packet(&stale_p);
        assert_eq!(v.status, "rejected");
        assert_eq!(v.phase, "geometry");

        // Swapped K parents [S, P]
        let mut swapped = valid_packet()?;
        swapped.github_preview_k.parents = vec![S.to_string(), P.to_string()];
        let v = evaluate_staged_promotion_packet(&swapped);
        assert_eq!(v.status, "rejected");
        assert_eq!(v.phase, "geometry");

        // Extra K parent
        let mut extra = valid_packet()?;
        extra.github_preview_k.parents.push(A.to_string());
        let v = evaluate_staged_promotion_packet(&extra);
        assert_eq!(v.status, "rejected");
        assert_eq!(v.phase, "geometry");

        // Wrapper/copy-tree substitution (K.tree != T)
        let mut wrapper = valid_packet()?;
        wrapper.github_preview_k.tree = A.to_string();
        let v = evaluate_staged_promotion_packet(&wrapper);
        assert_eq!(v.status, "rejected");
        assert!(v.reason.contains("wrapper/copy-tree"), "{v:?}");

        // Post-merge A with moved main
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
        // Tampered survivor digest
        let mut tampered = valid_packet()?;
        tampered.inventory.public_survivors_sha256 =
            "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb".to_string();
        let v = evaluate_staged_promotion_packet(&tampered);
        assert_eq!(v.status, "rejected");
        assert_eq!(v.phase, "inventory");

        // Invalid Git file mode
        let mut bad_mode = valid_packet()?;
        bad_mode.inventory.survivors[0].mode = "100600".to_string();
        bad_mode.inventory.public_survivors_sha256 =
            compute_survivors_digest(&bad_mode.inventory.survivors)?;
        let v = evaluate_staged_promotion_packet(&bad_mode);
        assert_eq!(v.status, "rejected");
        assert!(v.reason.contains("invalid Git file mode"), "{v:?}");

        // Missing nested import provenance
        let mut no_import = valid_packet()?;
        no_import.inventory.nested_imports.clear();
        no_import.inventory.nested_import_sha256 =
            compute_nested_imports_digest(&no_import.inventory.nested_imports)?;
        let v = evaluate_staged_promotion_packet(&no_import);
        assert_eq!(v.status, "rejected");
        assert!(v.reason.contains("nested public-import"), "{v:?}");

        // Rename missing previous_path
        let mut bad_rename = valid_packet()?;
        bad_rename.inventory.survivors[1].previous_path = None;
        bad_rename.inventory.public_survivors_sha256 =
            compute_survivors_digest(&bad_rename.inventory.survivors)?;
        assert_eq!(
            evaluate_staged_promotion_packet(&bad_rename).status,
            "rejected"
        );

        // Deletion with non-null object_sha
        let mut bad_delete = valid_packet()?;
        bad_delete.inventory.survivors[2].object_sha = Some(BLOB.to_string());
        bad_delete.inventory.public_survivors_sha256 =
            compute_survivors_digest(&bad_delete.inventory.survivors)?;
        assert_eq!(
            evaluate_staged_promotion_packet(&bad_delete).status,
            "rejected"
        );

        // Empty limited_unavailable reason rejected
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
}
