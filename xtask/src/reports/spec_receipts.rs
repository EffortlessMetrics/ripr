//! Content-bound review receipts for specification maintenance (#3466).
//!
//! A `SpecReviewReceiptV1` records that one exact spec content received a
//! bounded advisory maintenance disposition. Receipts are operational review
//! records, one committed TOML file per spec under
//! `.allow/spec-system/reviews/`, mirroring the per-item `slices/` storage
//! precedent: unrelated per-spec closes never collide, Git history preserves
//! prior generations, and the current file records the latest observation.
//!
//! Receipts are advisory only: they never change document status, requirement
//! lifecycle, implementation, evidence, or support state, never alter any
//! command's exit code, and are consumed by `specs maintenance` and the
//! repo-level validator test — nothing else.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

pub(crate) const RECEIPT_SCHEMA_VERSION: &str = "1";
pub(crate) const RECEIPTS_DEFAULT_DIR: &str = ".allow/spec-system/reviews";
pub(crate) const RECEIPT_PRODUCER: &str = "xtask specs close";

/// The fixed advisory claim boundary every receipt carries. The verifiable
/// human decision is the PR review that lands the receipt; `reviewed_by` is an
/// asserted claim inside that reviewed diff.
pub(crate) const RECEIPT_CLAIM_BOUNDARY: &str = "An asserted, content-bound record that this exact spec content was observed by the named reviewer and reached the labeled advisory maintenance disposition; the PR review that lands the receipt is the verification act.";

/// The fixed non-claims list every receipt carries: a maintenance disposition
/// is not normative status, implementation, proof, support, or live-work
/// authority.
pub(crate) const RECEIPT_NON_CLAIMS: [&str; 3] = [
    "specification correctness or completeness",
    "normative document status, requirement lifecycle, implementation, evidence, or support state",
    "acceptance, deprecation, supersession, migration, merge eligibility, or branch protection",
];

/// Closed maintenance dispositions (#3466). These are review-specific
/// maintenance labels, not document or requirement lifecycle states; the
/// maintenance report renders them verbatim with no interpretation.
const DISPOSITIONS: [&str; 10] = [
    "current_no_source_change",
    "retain_proposed_named_evidence_gap",
    "source_correction_required",
    "acceptance_candidate",
    "deprecation_candidate",
    "supersession_candidate",
    "v2_migration_candidate",
    "followup_issue_required",
    "needs_more_evidence",
    "not_applicable",
];

/// Rejected-observation reason tokens surfaced by `specs maintenance`.
/// A rejected receipt closes nothing and never fails the advisory command.
pub(crate) const REJECTED_MALFORMED: &str = "receipt-malformed";
pub(crate) const REJECTED_UNKNOWN_SCHEMA: &str = "receipt-unknown-schema";
pub(crate) const REJECTED_SPEC_MISMATCH: &str = "receipt-spec-mismatch";
pub(crate) const REJECTED_PATH_MISMATCH: &str = "receipt-path-mismatch";
pub(crate) const REJECTED_DUPLICATE_SPEC: &str = "receipt-duplicate-spec";
pub(crate) const REJECTED_NAME_NOT_A_SPEC_ID: &str = "receipt-name-not-a-spec-id";
pub(crate) const REJECTED_UNSUPPORTED_FILE: &str = "unsupported-receipt-file";

const CLOSE_USAGE: &str = "usage: cargo xtask specs close --spec RIPR-SPEC-NNNN --disposition <label> --as-of YYYY-MM-DD --reviewed-by <identity> [--waived-until YYYY-MM-DD] [--detail <text>]";

/// On-disk receipt contract (`SpecReviewReceiptV1`). Field declaration order
/// is the deterministic writer output order. Unknown keys are rejected so a
/// committed receipt cannot silently carry fields outside this enumeration.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct SpecReviewReceiptV1 {
    pub(crate) schema_version: String,
    pub(crate) producer: String,
    pub(crate) spec_id: String,
    pub(crate) spec_path: String,
    pub(crate) content_digest: String,
    pub(crate) status_observed: String,
    pub(crate) observed_at: String,
    pub(crate) reviewed_by: String,
    pub(crate) disposition: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) waived_until: Option<String>,
    #[serde(default)]
    pub(crate) disposition_detail: String,
    #[serde(default)]
    pub(crate) reasons_inspected: Vec<String>,
    #[serde(default)]
    pub(crate) evidence_refs: Vec<String>,
    #[serde(default)]
    pub(crate) limitations: Vec<String>,
    pub(crate) receipt_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) predecessor_receipt_id: Option<String>,
    pub(crate) claim_boundary: String,
    pub(crate) non_claims: Vec<String>,
}

/// One receipt observation attached to a maintenance row.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub(crate) struct ReceiptObservation {
    pub(crate) status: String,
    pub(crate) receipt_id: String,
    pub(crate) disposition: String,
    pub(crate) observed_at: String,
    pub(crate) waived_until: Option<String>,
    pub(crate) reviewed_by: String,
}

/// A receipt file that produced no observation, with a named reason.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub(crate) struct RejectedReceipt {
    pub(crate) path: String,
    pub(crate) reason: String,
}

/// Receipts directory observation rendered by `specs maintenance`.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub(crate) struct ReceiptsObservation {
    pub(crate) source: String,
    pub(crate) parsed: usize,
    pub(crate) applied: usize,
    pub(crate) rejected: Vec<RejectedReceipt>,
}

impl ReceiptsObservation {
    pub(crate) fn none() -> Self {
        Self {
            source: "none".to_string(),
            parsed: 0,
            applied: 0,
            rejected: Vec::new(),
        }
    }
}

/// Injectable receipts input for `build_report`, mirroring `HistoryInput`:
/// `Default` is absent, so the zero-receipt baseline stays trivial and
/// fixtures inject a temp directory.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct ReceiptInput {
    pub(crate) directory: Option<PathBuf>,
}

/// A receipt that validated and matched a spec filename key.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct MatchedReceipt {
    pub(crate) file_path: String,
    pub(crate) receipt: SpecReviewReceiptV1,
}

/// Result of scanning one receipts directory. `matched` is keyed by spec ID;
/// the first valid receipt in sorted path order wins and later receipts for
/// the same ID become `receipt-duplicate-spec` rejections.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct ReceiptScan {
    pub(crate) source: String,
    pub(crate) parsed: usize,
    pub(crate) matched: BTreeMap<String, MatchedReceipt>,
    pub(crate) rejected: Vec<RejectedReceipt>,
}

/// Deterministic semantic receipt identity (#3466 identity law):
/// `sha256:` over a canonical newline-joined string of the disposition-
/// bearing fields, in fixed order, with the scheduling/provenance fields
/// (`observed_at`, `reviewed_by`, `producer`, `predecessor_receipt_id`)
/// excluded. Equivalent content, evidence, and disposition therefore produce
/// the same identity, and re-observation updates the file without changing it.
pub(crate) fn compute_receipt_id(receipt: &SpecReviewReceiptV1) -> String {
    let mut canonical = String::new();
    let mut push = |part: &str| {
        if !canonical.is_empty() {
            canonical.push('\n');
        }
        canonical.push_str(part);
    };
    push(&receipt.schema_version);
    push(&receipt.spec_id);
    push(&receipt.spec_path);
    push(&receipt.content_digest);
    push(&receipt.status_observed);
    push(&receipt.disposition);
    push(receipt.waived_until.as_deref().unwrap_or_default());
    push(&receipt.disposition_detail);
    for list in [
        sorted_deduped(&receipt.reasons_inspected),
        sorted_deduped(&receipt.evidence_refs),
        sorted_deduped(&receipt.limitations),
    ] {
        for value in &list {
            push(value);
        }
    }
    let mut hasher = Sha256::new();
    hasher.update(canonical.as_bytes());
    format!("sha256:{:x}", hasher.finalize())
}

fn sorted_deduped(values: &[String]) -> Vec<String> {
    let mut sorted = values.to_vec();
    sorted.sort();
    sorted.dedup();
    sorted
}

/// Validate one receipt document. The `Err` payload is exactly the
/// rejected-observation reason token, so the advisory report can name why a
/// receipt was rejected without ever becoming an instrument failure.
pub(crate) fn validate_receipt(raw: &str) -> Result<SpecReviewReceiptV1, String> {
    // The rejected-observation contract names *why* a receipt was rejected
    // without embedding parser diagnostics; the discarded originals are
    // deliberately unnamed (`_toml_error`, `_date_error`) so the advisory
    // reason token stays stable.
    let receipt: SpecReviewReceiptV1 =
        toml::from_str(raw).map_err(|_toml_error| REJECTED_MALFORMED.to_string())?;
    if receipt.schema_version != RECEIPT_SCHEMA_VERSION {
        return Err(REJECTED_UNKNOWN_SCHEMA.to_string());
    }
    if receipt.producer.trim().is_empty()
        || crate::spec_id_from_file_name(&receipt.spec_id).as_deref()
            != Some(receipt.spec_id.as_str())
        || receipt.spec_path.is_empty()
        || receipt.spec_path.starts_with('/')
        || receipt.spec_path.contains('\\')
        || !is_sha256_digest(&receipt.content_digest)
        || receipt.status_observed.trim().is_empty()
        || receipt.reviewed_by.trim().is_empty()
        || !DISPOSITIONS.contains(&receipt.disposition.as_str())
    {
        return Err(REJECTED_MALFORMED.to_string());
    }
    crate::reports::spec_maintenance::validate_date(&receipt.observed_at)
        .map_err(|_date_error| REJECTED_MALFORMED.to_string())?;
    if let Some(until) = &receipt.waived_until {
        crate::reports::spec_maintenance::validate_date(until)
            .map_err(|_date_error| REJECTED_MALFORMED.to_string())?;
    }
    for list in [
        &receipt.reasons_inspected,
        &receipt.evidence_refs,
        &receipt.limitations,
    ] {
        if list.iter().any(|value| value.trim().is_empty()) {
            return Err(REJECTED_MALFORMED.to_string());
        }
    }
    if receipt.claim_boundary != RECEIPT_CLAIM_BOUNDARY
        || receipt.non_claims
            != RECEIPT_NON_CLAIMS
                .iter()
                .map(|claim| claim.to_string())
                .collect::<Vec<_>>()
    {
        return Err(REJECTED_MALFORMED.to_string());
    }
    if compute_receipt_id(&receipt) != receipt.receipt_id {
        return Err(REJECTED_MALFORMED.to_string());
    }
    Ok(receipt)
}

fn is_sha256_digest(value: &str) -> bool {
    let Some(hex) = value.strip_prefix("sha256:") else {
        return false;
    };
    hex.len() == 64
        && hex
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

/// Read one receipts directory deterministically: entries are sorted,
/// symlinks are skipped, and every entry that does not become a matched
/// receipt is a rejected observation with a named reason.
pub(crate) fn scan_receipt_directory(directory: &Path) -> Result<ReceiptScan, String> {
    let mut scan = ReceiptScan {
        source: crate::normalize_path(directory),
        parsed: 0,
        matched: BTreeMap::new(),
        rejected: Vec::new(),
    };
    if !directory.exists() {
        return Ok(scan);
    }
    let mut entries: Vec<PathBuf> = Vec::new();
    for entry in
        fs::read_dir(directory).map_err(|error| format!("read {}: {error}", directory.display()))?
    {
        let entry = entry.map_err(|error| format!("read receipts directory entry: {error}"))?;
        let path = entry.path();
        // A symlinked entry can point anywhere: receipts are read from the
        // committed directory only, so symbolic links are never followed.
        let is_symlink = entry
            .file_type()
            .map(|kind| kind.is_symlink())
            .unwrap_or(false);
        if is_symlink {
            continue;
        }
        entries.push(path);
    }
    entries.sort();
    for path in entries {
        let display = crate::normalize_path(&path);
        let reject = |reason: &str| RejectedReceipt {
            path: display.clone(),
            reason: reason.to_string(),
        };
        if path.is_dir() {
            scan.rejected.push(reject(REJECTED_UNSUPPORTED_FILE));
            continue;
        }
        if path.extension().and_then(|value| value.to_str()) != Some("toml") {
            scan.rejected.push(reject(REJECTED_UNSUPPORTED_FILE));
            continue;
        }
        // Receipt files are keyed by stem: `RIPR-SPEC-0005.toml` parses
        // through the shared spec-ID rule exactly like a spec file name
        // does, and slug suffixes stay legal (`RIPR-SPEC-0005-old.toml`).
        let file_stem = path
            .file_stem()
            .and_then(|value| value.to_str())
            .unwrap_or_default();
        let Some(name_id) = crate::spec_id_from_file_name(file_stem) else {
            scan.rejected.push(reject(REJECTED_NAME_NOT_A_SPEC_ID));
            continue;
        };
        let raw = match fs::read_to_string(&path) {
            Ok(raw) => raw,
            Err(_) => {
                scan.rejected.push(reject(REJECTED_MALFORMED));
                continue;
            }
        };
        let receipt = match validate_receipt(&raw) {
            Ok(receipt) => receipt,
            Err(reason) => {
                scan.rejected.push(reject(&reason));
                continue;
            }
        };
        if receipt.spec_id != name_id {
            scan.rejected.push(reject(REJECTED_SPEC_MISMATCH));
            continue;
        }
        scan.parsed += 1;
        if scan.matched.contains_key(&receipt.spec_id) {
            scan.rejected.push(reject(REJECTED_DUPLICATE_SPEC));
            continue;
        }
        scan.matched.insert(
            receipt.spec_id.clone(),
            MatchedReceipt {
                file_path: display,
                receipt,
            },
        );
    }
    scan.rejected
        .sort_by(|left, right| left.path.cmp(&right.path));
    Ok(scan)
}

/// Resolve and validate one committed receipt by spec ID. `Ok(None)` means no
/// receipt exists yet; `Err` means one exists but is not a valid
/// `SpecReviewReceiptV1`, which the writer refuses to overwrite silently so
/// the predecessor chain never breaks without a visible human action.
pub(crate) fn read_receipt_for_spec(
    reviews_dir: &Path,
    spec_id: &str,
) -> Result<Option<SpecReviewReceiptV1>, String> {
    let receipt_path = reviews_dir.join(format!("{spec_id}.toml"));
    if !receipt_path.exists() {
        return Ok(None);
    }
    let raw = fs::read_to_string(&receipt_path)
        .map_err(|error| format!("read existing receipt {}: {error}", receipt_path.display()))?;
    validate_receipt(&raw)
        .map(Some)
        .map_err(|reason| {
            format!(
                "refusing to close over an unreadable existing receipt {} (reason: {reason}); fix or remove it first",
                receipt_path.display()
            )
        })
}

/// The `specs close` writer: one deterministic, content-bound receipt per
/// call. It computes the digest from current spec bytes itself, derives the
/// semantic receipt identity, links the previous generation, and never edits
/// the spec file.
pub(crate) fn spec_close(args: &[String]) -> Result<(), String> {
    if args.iter().any(|arg| arg == "--help" || arg == "-h") {
        println!("{CLOSE_USAGE}");
        return Ok(());
    }
    let close = parse_close_options(args)?;
    let root = Path::new(".");
    let specs = crate::collect_spec_files_for_root(root)?;
    let spec = specs
        .iter()
        .find(|spec| spec.id == close.spec_id)
        .ok_or_else(|| {
            format!(
                "unknown spec ID `{}`\n{CLOSE_USAGE}\nRun `cargo xtask specs next` or list docs/specs to find a valid ID.",
                close.spec_id
            )
        })?;
    let spec_bytes = fs::read(root.join(&spec.relative_path))
        .map_err(|error| format!("read required spec {}: {error}", spec.relative_path))?;
    let text = String::from_utf8(spec_bytes.clone())
        .map_err(|error| format!("required spec {} is not UTF-8: {error}", spec.relative_path))?;
    let status_observed = text
        .lines()
        .find_map(|line| line.strip_prefix("Status:"))
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("unknown")
        .to_string();
    let mut hasher = Sha256::new();
    hasher.update(&spec_bytes);
    let content_digest = format!("sha256:{:x}", hasher.finalize());
    let reviews_dir = root.join(RECEIPTS_DEFAULT_DIR);
    let predecessor_receipt_id =
        read_receipt_for_spec(&reviews_dir, &close.spec_id)?.map(|existing| existing.receipt_id);
    let mut receipt = SpecReviewReceiptV1 {
        schema_version: RECEIPT_SCHEMA_VERSION.to_string(),
        producer: RECEIPT_PRODUCER.to_string(),
        spec_id: close.spec_id.clone(),
        spec_path: spec.relative_path.clone(),
        content_digest,
        status_observed,
        observed_at: close.as_of.clone(),
        reviewed_by: close.reviewed_by.clone(),
        disposition: close.disposition.clone(),
        waived_until: close.waived_until.clone(),
        disposition_detail: close.detail.clone(),
        reasons_inspected: Vec::new(),
        evidence_refs: Vec::new(),
        limitations: Vec::new(),
        receipt_id: String::new(),
        predecessor_receipt_id,
        claim_boundary: RECEIPT_CLAIM_BOUNDARY.to_string(),
        non_claims: RECEIPT_NON_CLAIMS
            .iter()
            .map(|claim| claim.to_string())
            .collect(),
    };
    receipt.receipt_id = compute_receipt_id(&receipt);
    fs::create_dir_all(&reviews_dir)
        .map_err(|error| format!("create {}: {error}", reviews_dir.display()))?;
    let receipt_path = reviews_dir.join(format!("{}.toml", close.spec_id));
    let mut body =
        toml::to_string(&receipt).map_err(|error| format!("serialize review receipt: {error}"))?;
    if !body.ends_with('\n') {
        body.push('\n');
    }
    fs::write(&receipt_path, body)
        .map_err(|error| format!("write {}: {error}", receipt_path.display()))?;
    println!("Wrote {}", crate::normalize_path(&receipt_path));
    println!("Spec digest: {}", receipt.content_digest);
    println!("Receipt id: {}", receipt.receipt_id);
    Ok(())
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct CloseOptions {
    spec_id: String,
    disposition: String,
    as_of: String,
    reviewed_by: String,
    waived_until: Option<String>,
    detail: String,
}

fn parse_close_options(args: &[String]) -> Result<CloseOptions, String> {
    let mut spec_id = None;
    let mut disposition = None;
    let mut as_of = None;
    let mut reviewed_by = None;
    let mut waived_until = None;
    let mut detail = String::new();
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--spec" => {
                index += 1;
                let value = args
                    .get(index)
                    .ok_or_else(|| format!("missing value for --spec\n{CLOSE_USAGE}"))?;
                if crate::spec_id_from_file_name(value).as_deref() != Some(value.as_str()) {
                    return Err(format!(
                        "--spec must be a bare spec ID like RIPR-SPEC-0005, got `{value}`\n{CLOSE_USAGE}"
                    ));
                }
                spec_id = Some(value.clone());
            }
            "--disposition" => {
                index += 1;
                let value = args
                    .get(index)
                    .ok_or_else(|| format!("missing value for --disposition\n{CLOSE_USAGE}"))?;
                if !DISPOSITIONS.contains(&value.as_str()) {
                    return Err(format!(
                        "unknown --disposition `{value}`; expected one of: {}\n{CLOSE_USAGE}",
                        DISPOSITIONS.join(", ")
                    ));
                }
                disposition = Some(value.clone());
            }
            "--as-of" => {
                index += 1;
                let value = args
                    .get(index)
                    .ok_or_else(|| format!("missing value for --as-of\n{CLOSE_USAGE}"))?;
                crate::reports::spec_maintenance::validate_date(value)?;
                as_of = Some(value.clone());
            }
            "--reviewed-by" => {
                index += 1;
                let value = args
                    .get(index)
                    .ok_or_else(|| format!("missing value for --reviewed-by\n{CLOSE_USAGE}"))?;
                if value.trim().is_empty() {
                    return Err(format!(
                        "--reviewed-by must be a non-empty reviewer identity\n{CLOSE_USAGE}"
                    ));
                }
                reviewed_by = Some(value.clone());
            }
            "--waived-until" => {
                index += 1;
                let value = args
                    .get(index)
                    .ok_or_else(|| format!("missing value for --waived-until\n{CLOSE_USAGE}"))?;
                crate::reports::spec_maintenance::validate_date(value)?;
                waived_until = Some(value.clone());
            }
            "--detail" => {
                index += 1;
                detail = args
                    .get(index)
                    .ok_or_else(|| format!("missing value for --detail\n{CLOSE_USAGE}"))?
                    .clone();
            }
            other => {
                return Err(format!(
                    "unknown specs close argument `{other}`\n{CLOSE_USAGE}"
                ));
            }
        }
        index += 1;
    }
    let spec_id = spec_id.ok_or_else(|| format!("missing required --spec\n{CLOSE_USAGE}"))?;
    let disposition =
        disposition.ok_or_else(|| format!("missing required --disposition\n{CLOSE_USAGE}"))?;
    let as_of = as_of.ok_or_else(|| format!("missing required --as-of\n{CLOSE_USAGE}"))?;
    let reviewed_by =
        reviewed_by.ok_or_else(|| format!("missing required --reviewed-by\n{CLOSE_USAGE}"))?;
    Ok(CloseOptions {
        spec_id,
        disposition,
        as_of,
        reviewed_by,
        waived_until,
        detail,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn fixture_root(name: &str) -> Result<PathBuf, String> {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|error| error.to_string())?
            .as_nanos();
        let root = std::env::temp_dir().join(format!("ripr-{name}-{stamp}"));
        fs::create_dir_all(root.join("docs/specs")).map_err(|error| error.to_string())?;
        Ok(root)
    }

    fn spec_bytes(status: &str) -> Vec<u8> {
        format!("# Example\n\nStatus: {status}\n").into_bytes()
    }

    fn digest_of(bytes: &[u8]) -> String {
        let mut hasher = Sha256::new();
        hasher.update(bytes);
        format!("sha256:{:x}", hasher.finalize())
    }

    fn base_receipt(spec_id: &str, digest: &str) -> SpecReviewReceiptV1 {
        let content_digest = if digest.starts_with("sha256:") {
            digest.to_string()
        } else {
            format!("sha256:{digest}")
        };
        SpecReviewReceiptV1 {
            schema_version: RECEIPT_SCHEMA_VERSION.to_string(),
            producer: "hand-authored".to_string(),
            spec_id: spec_id.to_string(),
            spec_path: format!("docs/specs/{spec_id}-example.md"),
            content_digest,
            status_observed: "proposed".to_string(),
            observed_at: "2026-08-27".to_string(),
            reviewed_by: "maintainer".to_string(),
            disposition: "current_no_source_change".to_string(),
            waived_until: None,
            disposition_detail: String::new(),
            reasons_inspected: Vec::new(),
            evidence_refs: Vec::new(),
            limitations: Vec::new(),
            receipt_id: String::new(),
            predecessor_receipt_id: None,
            claim_boundary: RECEIPT_CLAIM_BOUNDARY.to_string(),
            non_claims: RECEIPT_NON_CLAIMS
                .iter()
                .map(|claim| claim.to_string())
                .collect(),
        }
    }

    fn sealed_receipt(spec_id: &str, digest: &str) -> SpecReviewReceiptV1 {
        let mut receipt = base_receipt(spec_id, digest);
        receipt.receipt_id = compute_receipt_id(&receipt);
        receipt
    }

    fn write_receipt(
        root: &Path,
        spec_id: &str,
        receipt: &SpecReviewReceiptV1,
    ) -> Result<(), String> {
        let dir = root.join(RECEIPTS_DEFAULT_DIR);
        fs::create_dir_all(&dir).map_err(|error| error.to_string())?;
        let mut body = toml::to_string(receipt).map_err(|error| error.to_string())?;
        if !body.ends_with('\n') {
            body.push('\n');
        }
        fs::write(dir.join(format!("{spec_id}.toml")), body).map_err(|error| error.to_string())
    }

    #[test]
    fn receipt_round_trips_and_identity_is_deterministic() -> Result<(), String> {
        let receipt = sealed_receipt("RIPR-SPEC-0001", &"a".repeat(64));
        let mut body = toml::to_string(&receipt).map_err(|error| error.to_string())?;
        if !body.ends_with('\n') {
            body.push('\n');
        }
        let parsed = validate_receipt(&body)
            .map_err(|reason| format!("valid receipt rejected: {reason}; body:\n{body}"))?;
        if parsed != receipt {
            return Err("round trip did not preserve the receipt".to_string());
        }
        if compute_receipt_id(&parsed) != receipt.receipt_id {
            return Err("identity was not stable across round trip".to_string());
        }
        // Deterministic serialization: field order is the declaration order.
        if !body.starts_with("schema_version = \"1\"\nproducer = \"hand-authored\"\n") {
            return Err("receipt field order is not the deterministic contract order".to_string());
        }
        Ok(())
    }

    #[test]
    fn identity_law_pins_scheduling_metadata_and_content_sensitivity() -> Result<(), String> {
        let digest = &"a".repeat(64);
        let receipt = sealed_receipt("RIPR-SPEC-0001", digest);
        // Scheduling/provenance metadata does not change semantic identity.
        let mut reobserved = receipt.clone();
        reobserved.observed_at = "2027-01-01".to_string();
        reobserved.reviewed_by = "other-reviewer".to_string();
        reobserved.producer = "hand-authored".to_string();
        reobserved.predecessor_receipt_id = Some("sha256:".to_string() + &"b".repeat(64));
        if compute_receipt_id(&reobserved) != receipt.receipt_id {
            return Err("scheduling metadata changed the semantic identity".to_string());
        }
        // Content, disposition, waiver, detail, and evidence do.
        let mut changed = receipt.clone();
        changed.content_digest = "sha256:".to_string() + &"c".repeat(64);
        if compute_receipt_id(&changed) == receipt.receipt_id {
            return Err("content change kept the same identity".to_string());
        }
        let mut disposition = receipt.clone();
        disposition.disposition = "not_applicable".to_string();
        if compute_receipt_id(&disposition) == receipt.receipt_id {
            return Err("disposition change kept the same identity".to_string());
        }
        let mut waived = receipt.clone();
        waived.waived_until = Some("2027-02-28".to_string());
        if compute_receipt_id(&waived) == receipt.receipt_id {
            return Err("waiver change kept the same identity".to_string());
        }
        let mut detailed = receipt.clone();
        detailed.disposition_detail = "file a follow-up".to_string();
        if compute_receipt_id(&detailed) == receipt.receipt_id {
            return Err("detail change kept the same identity".to_string());
        }
        let mut evidence = receipt.clone();
        evidence.evidence_refs = vec!["docs/evidence.md".to_string()];
        if compute_receipt_id(&evidence) == receipt.receipt_id {
            return Err("evidence change kept the same identity".to_string());
        }
        Ok(())
    }

    #[test]
    fn validator_rejects_each_broken_contract_shape() -> Result<(), String> {
        let digest = &"a".repeat(64);
        let receipt = sealed_receipt("RIPR-SPEC-0001", digest);
        let serialize = |receipt: &SpecReviewReceiptV1| -> Result<String, String> {
            let mut body = toml::to_string(receipt).map_err(|error| error.to_string())?;
            if !body.ends_with('\n') {
                body.push('\n');
            }
            Ok(body)
        };
        if validate_receipt("not toml at all").err().as_deref() != Some(REJECTED_MALFORMED) {
            return Err("malformed TOML was not rejected as malformed".to_string());
        }
        let mut unknown_schema = receipt.clone();
        unknown_schema.schema_version = "2".to_string();
        unknown_schema.receipt_id = compute_receipt_id(&unknown_schema);
        if validate_receipt(&serialize(&unknown_schema)?)
            .err()
            .as_deref()
            != Some(REJECTED_UNKNOWN_SCHEMA)
        {
            return Err("unknown schema was not reported".to_string());
        }
        let mut unknown_disposition = receipt.clone();
        unknown_disposition.disposition = "retired_by_ruling".to_string();
        unknown_disposition.receipt_id = compute_receipt_id(&unknown_disposition);
        if validate_receipt(&serialize(&unknown_disposition)?)
            .err()
            .as_deref()
            != Some(REJECTED_MALFORMED)
        {
            return Err("unknown disposition was not rejected".to_string());
        }
        let mut bad_digest = receipt.clone();
        bad_digest.content_digest = "md3:deadbeef".to_string();
        bad_digest.receipt_id = compute_receipt_id(&bad_digest);
        if validate_receipt(&serialize(&bad_digest)?).is_ok() {
            return Err("non-sha256 digest was accepted".to_string());
        }
        let mut bad_date = receipt.clone();
        bad_date.observed_at = "2026-02-31".to_string();
        bad_date.receipt_id = compute_receipt_id(&bad_date);
        if validate_receipt(&serialize(&bad_date)?).is_ok() {
            return Err("malformed observation date was accepted".to_string());
        }
        let mut tampered = receipt.clone();
        tampered.disposition_detail = "quietly edited".to_string();
        if validate_receipt(&serialize(&tampered)?).err().as_deref() != Some(REJECTED_MALFORMED) {
            return Err("receipt_id mismatch (tampered identity) was accepted".to_string());
        }
        let mut foreign_boundary = receipt.clone();
        foreign_boundary.claim_boundary = "proves the spec correct".to_string();
        foreign_boundary.receipt_id = compute_receipt_id(&foreign_boundary);
        if validate_receipt(&serialize(&foreign_boundary)?).is_ok() {
            return Err(
                "a receipt claiming more than the advisory boundary was accepted".to_string(),
            );
        }
        let mut unknown_key = receipt.clone();
        unknown_key.receipt_id = compute_receipt_id(&unknown_key);
        let mut body = serialize(&unknown_key)?;
        body.push_str("reviewer_quality_score = 5\n");
        if validate_receipt(&body).is_ok() {
            return Err("unknown key outside the field enumeration was accepted".to_string());
        }
        Ok(())
    }

    #[test]
    fn scan_names_every_rejection_and_deduplicates_by_spec() -> Result<(), String> {
        let root = fixture_root("receipt-scan")?;
        let dir = root.join(RECEIPTS_DEFAULT_DIR);
        fs::create_dir_all(&dir).map_err(|error| error.to_string())?;
        fs::write(dir.join("RIPR-SPEC-0001.toml"), "disposition = ") // malformed TOML
            .map_err(|error| error.to_string())?;
        fs::write(dir.join("notes.toml"), "x = 1\n").map_err(|error| error.to_string())?;
        fs::write(dir.join("RIPR-SPEC-0003.md"), "not toml\n")
            .map_err(|error| error.to_string())?;
        // spec_id field disagrees with the filename.
        let mismatched = sealed_receipt("RIPR-SPEC-0004", &"a".repeat(64));
        write_receipt(&root, "RIPR-SPEC-0005", &mismatched)?;
        // Two valid receipts for one spec: the first in sorted path order
        // wins (`-` sorts before `.`, so the `-b` suffix file is first).
        write_receipt(
            &root,
            "RIPR-SPEC-0007",
            &sealed_receipt("RIPR-SPEC-0007", &"a".repeat(64)),
        )?;
        write_receipt(
            &root,
            "RIPR-SPEC-0007-b",
            &sealed_receipt("RIPR-SPEC-0007", &"a".repeat(64)),
        )?;
        let scan = scan_receipt_directory(&dir)?;
        if scan.parsed != 2 {
            return Err(format!("expected 2 parsed receipts, got {}", scan.parsed));
        }
        if scan.matched.len() != 1 || !scan.matched.contains_key("RIPR-SPEC-0007") {
            return Err("the valid receipt was not matched by spec ID".to_string());
        }
        if !scan.matched["RIPR-SPEC-0007"]
            .file_path
            .ends_with("RIPR-SPEC-0007-b.toml")
        {
            return Err("sorted-path-order winner was not deterministic".to_string());
        }
        let reasons: BTreeMap<String, String> = scan
            .rejected
            .iter()
            .map(|rejected| {
                (
                    rejected
                        .path
                        .split('/')
                        .next_back()
                        .unwrap_or_default()
                        .to_string(),
                    rejected.reason.clone(),
                )
            })
            .collect();
        if reasons.get("RIPR-SPEC-0001.toml").map(String::as_str) != Some(REJECTED_MALFORMED) {
            return Err("malformed receipt was not named".to_string());
        }
        if reasons.get("notes.toml").map(String::as_str) != Some(REJECTED_NAME_NOT_A_SPEC_ID) {
            return Err("unparsable receipt filename was not named".to_string());
        }
        if reasons.get("RIPR-SPEC-0003.md").map(String::as_str) != Some(REJECTED_UNSUPPORTED_FILE) {
            return Err("non-TOML receipt file was not named".to_string());
        }
        if reasons.get("RIPR-SPEC-0005.toml").map(String::as_str) != Some(REJECTED_SPEC_MISMATCH) {
            return Err("spec_id/filename mismatch was not named".to_string());
        }
        if reasons.get("RIPR-SPEC-0007.toml").map(String::as_str) != Some(REJECTED_DUPLICATE_SPEC) {
            return Err("duplicate receipt was not named".to_string());
        }
        if scan
            .rejected
            .windows(2)
            .any(|pair| pair[0].path > pair[1].path)
        {
            return Err("rejected observations are not sorted by path".to_string());
        }
        let _ = mismatched;
        Ok(())
    }

    #[test]
    fn writer_computes_digest_links_predecessor_and_never_touches_the_spec() -> Result<(), String> {
        let root = fixture_root("receipt-writer")?;
        let _guard = CwdGuard::new(&root)?;
        let spec_relative = "docs/specs/RIPR-SPEC-9101-example.md";
        let original = spec_bytes("proposed");
        fs::write(root.join(spec_relative), &original).map_err(|error| error.to_string())?;
        let args = |extra: &[&str]| -> Vec<String> {
            let mut all: Vec<String> = [
                "--spec".to_string(),
                "RIPR-SPEC-9101".to_string(),
                "--disposition".to_string(),
                "current_no_source_change".to_string(),
                "--as-of".to_string(),
                "2026-08-29".to_string(),
                "--reviewed-by".to_string(),
                "maintainer".to_string(),
            ]
            .into_iter()
            .collect();
            all.extend(extra.iter().map(|value| value.to_string()));
            all
        };
        spec_close(&args(&[]))?;
        let receipt_path = root.join(RECEIPTS_DEFAULT_DIR).join("RIPR-SPEC-9101.toml");
        let body = fs::read_to_string(&receipt_path).map_err(|error| error.to_string())?;
        let receipt =
            validate_receipt(&body).map_err(|reason| format!("writer output invalid: {reason}"))?;
        if receipt.content_digest != digest_of(&original) {
            return Err("writer did not bind the receipt to the computed digest".to_string());
        }
        if receipt.spec_path != spec_relative
            || receipt.status_observed != "proposed"
            || receipt.observed_at != "2026-08-29"
            || receipt.producer != RECEIPT_PRODUCER
            || receipt.predecessor_receipt_id.is_some()
        {
            return Err("writer did not record the observed fields honestly".to_string());
        }
        // The writer is the only writer of the spec file: bytes unchanged.
        if fs::read(root.join(spec_relative)).map_err(|error| error.to_string())? != original {
            return Err("the close writer edited the spec file".to_string());
        }
        // First regeneration links its predecessor.
        spec_close(&args(&[]))?;
        let second_body = fs::read_to_string(&receipt_path).map_err(|error| error.to_string())?;
        let second = validate_receipt(&second_body)
            .map_err(|reason| format!("regeneration invalid: {reason}"))?;
        if second.predecessor_receipt_id.as_deref() != Some(receipt.receipt_id.as_str()) {
            return Err("regeneration did not link its predecessor".to_string());
        }
        // Identity excludes scheduling/provenance fields, so a re-observation
        // with identical inputs keeps the same semantic receipt identity.
        if second.receipt_id != receipt.receipt_id {
            return Err("re-observation changed the semantic identity".to_string());
        }
        // Steady-state determinism: the same inputs against the same prior
        // receipt state produce byte-identical output.
        let steady = fs::read(&receipt_path).map_err(|error| error.to_string())?;
        spec_close(&args(&[]))?;
        if fs::read(&receipt_path).map_err(|error| error.to_string())? != steady {
            return Err("repeated close was not byte-deterministic".to_string());
        }
        // A changed disposition-bearing field creates a new generation with a
        // new identity while linking the old one.
        spec_close(&args(&["--detail", "re-observed after waiver renewal"]))?;
        let third_body = fs::read_to_string(&receipt_path).map_err(|error| error.to_string())?;
        let third = validate_receipt(&third_body)
            .map_err(|reason| format!("third generation invalid: {reason}"))?;
        if third.predecessor_receipt_id.as_deref() != Some(receipt.receipt_id.as_str()) {
            return Err("third generation did not link its predecessor".to_string());
        }
        if third.receipt_id == receipt.receipt_id {
            return Err("detail change did not produce a new identity".to_string());
        }
        Ok(())
    }

    #[test]
    fn writer_refuses_invalid_inputs_and_broken_predecessors() -> Result<(), String> {
        let root = fixture_root("receipt-writer-refusals")?;
        let _guard = CwdGuard::new(&root)?;
        fs::write(
            root.join("docs/specs/RIPR-SPEC-9102-example.md"),
            spec_bytes("proposed"),
        )
        .map_err(|error| error.to_string())?;
        let base = |extra: &[&str]| -> Vec<String> {
            let mut all: Vec<String> = [
                "--spec".to_string(),
                "RIPR-SPEC-9102".to_string(),
                "--disposition".to_string(),
                "current_no_source_change".to_string(),
                "--as-of".to_string(),
                "2026-08-29".to_string(),
                "--reviewed-by".to_string(),
                "maintainer".to_string(),
            ]
            .into_iter()
            .collect();
            all.extend(extra.iter().map(|value| value.to_string()));
            all
        };
        if spec_close(&base(&["--reviewed-by", "  "])).is_ok() {
            return Err("empty reviewer identity was accepted".to_string());
        }
        if spec_close(&base(&["--disposition", "retired_by_ruling"])).is_ok() {
            return Err("unknown disposition label was accepted".to_string());
        }
        if spec_close(&base(&["--waived-until", "2026-13-01"])).is_ok() {
            return Err("invalid waiver date was accepted".to_string());
        }
        if spec_close(&[
            "--spec".to_string(),
            "RIPR-SPEC-9999".to_string(),
            "--disposition".to_string(),
            "current_no_source_change".to_string(),
            "--as-of".to_string(),
            "2026-08-29".to_string(),
            "--reviewed-by".to_string(),
            "maintainer".to_string(),
        ])
        .is_ok()
        {
            return Err("unknown spec ID was accepted".to_string());
        }
        // A broken existing receipt is never silently overwritten.
        let dir = root.join(RECEIPTS_DEFAULT_DIR);
        fs::create_dir_all(&dir).map_err(|error| error.to_string())?;
        fs::write(dir.join("RIPR-SPEC-9102.toml"), "schema_version = \"9\"\n")
            .map_err(|error| error.to_string())?;
        if spec_close(&base(&[])).is_ok() {
            return Err("close over an unreadable existing receipt was accepted".to_string());
        }
        Ok(())
    }

    /// Serializes cwd-sensitive tests: `spec_close` reads `.` as the repo
    /// root, so each fixture runs under its own current directory. The crate
    /// cwd write guard keeps this exclusive against `with_temp_cwd` users.
    struct CwdGuard {
        _lock: crate::CwdWriteGuard<'static>,
        previous: PathBuf,
    }

    impl CwdGuard {
        fn new(root: &Path) -> Result<Self, String> {
            let lock = crate::acquire_test_cwd_write_guard();
            let previous = std::env::current_dir().map_err(|error| error.to_string())?;
            std::env::set_current_dir(root).map_err(|error| error.to_string())?;
            Ok(Self {
                _lock: lock,
                previous,
            })
        }
    }

    impl Drop for CwdGuard {
        fn drop(&mut self) {
            let _ = std::env::set_current_dir(&self.previous);
        }
    }
}
