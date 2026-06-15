//! First-class `ripr receipt write / check` logic.
//!
//! This module owns the write and check operations for the canonical
//! `ripr receipt write / check` command defined in RIPR-SPEC-0079.
//! The CLI layer in `crates/ripr/src/cli/commands/receipt.rs` parses
//! arguments and delegates here; it must not contain analysis logic.
//!
//! ## Design notes
//!
//! - `receipt write` does NOT parse or validate a gap ledger at this stage
//!   (structural write only).
//! - `receipt check` validates JSON structure always; when `--ledger` is
//!   supplied it also cross-references the receipt's `canonical_gap_id`
//!   against the live gap set (RIPR-SPEC-0110).
//! - When `--ledger` is ABSENT or UNREADABLE the cross-reference result is
//!   `not_available` — NOT `receipt_ok`. Absence of the ledger must never be
//!   read as "the receipt is valid/fresh" (fail-closed honesty rule).
//! - All error paths are fail-closed: on any validation failure nothing is
//!   written and a non-zero exit is triggered via `Err(String)`.

use crate::output::gap_decision_ledger::parse_gap_records_json;
use crate::output::json;
use std::path::{Path, PathBuf};

/// Schema version for receipts written by this command.
pub(crate) const RECEIPT_SCHEMA_VERSION: &str = "0.1";

/// The four allowed verify_status values (RIPR-SPEC-0079).
pub(crate) const VALID_VERIFY_STATUSES: &[&str] = &["passed", "failed", "not_run", "unknown"];

/// Options for `ripr receipt write`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ReceiptWriteOptions {
    pub(crate) canonical_gap_id: String,
    pub(crate) packet_id: Option<String>,
    pub(crate) verify_command: String,
    pub(crate) verify_status: String,
    /// When `None`, defaults to `target/ripr/receipts/<canonical_gap_id>.json`.
    pub(crate) out: Option<PathBuf>,
    pub(crate) json: bool,
}

/// The cross-reference result from comparing a receipt against the live gap set.
///
/// Returned by `check_receipt` when `--ledger` is provided.  When the ledger
/// is absent or unreadable this is always `NotAvailable` (fail-closed rule:
/// absence of the ledger must never be interpreted as "receipt is ok").
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum ReceiptCrossRefResult {
    /// The ledger was not provided or could not be read.  This is the default
    /// fail-closed value; it does NOT mean the receipt is valid or fresh.
    NotAvailable,
    /// `canonical_gap_id` found in the live gap set.  Gap is present.
    ReceiptOk,
    /// `canonical_gap_id` not found in the live gap set.  The gap disappeared
    /// or never existed.  This is a real problem — callers must exit non-zero.
    OrphanReceipt,
    /// `canonical_gap_id` found but the gap's dedupe fingerprint differs from
    /// the receipt's stored fingerprint.  The gap moved or changed identity.
    /// This is a real problem — callers must exit non-zero.
    ReceiptGapMismatch,
}

impl ReceiptCrossRefResult {
    pub(crate) fn as_str(&self) -> &'static str {
        match self {
            Self::NotAvailable => "not_available",
            Self::ReceiptOk => "receipt_ok",
            Self::OrphanReceipt => "orphan_receipt",
            Self::ReceiptGapMismatch => "receipt_gap_mismatch",
        }
    }

    /// Returns `true` if this result represents a real problem that should
    /// cause a non-zero exit from `ripr receipt check`.
    pub(crate) fn is_error(&self) -> bool {
        matches!(self, Self::OrphanReceipt | Self::ReceiptGapMismatch)
    }
}

/// Options for `ripr receipt check`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ReceiptCheckOptions {
    /// Resolve path from canonical location when `path` is absent.
    pub(crate) gap: Option<String>,
    /// Explicit path to a receipt file.
    pub(crate) path: Option<PathBuf>,
    /// Optional path to a gap-decision-ledger JSON file.  When provided,
    /// the receipt's `canonical_gap_id` is cross-referenced against the live
    /// gap set.  When absent, cross-reference result is `not_available`.
    pub(crate) ledger: Option<PathBuf>,
}

/// Write a receipt JSON according to RIPR-SPEC-0079 and return the rendered
/// JSON string.  The caller is responsible for writing to disk (or stdout).
pub(crate) fn write_receipt(opts: &ReceiptWriteOptions) -> Result<String, String> {
    validate_write_options(opts)?;

    let packet_id_available = opts.packet_id.is_some();
    let packet_id_json = match &opts.packet_id {
        Some(id) => serde_json::Value::String(id.clone()),
        None => serde_json::Value::Null,
    };

    let written_at = written_at_rfc3339()?;

    let value = serde_json::json!({
        "schema_version": RECEIPT_SCHEMA_VERSION,
        "tool": "ripr",
        "kind": "receipt",
        "canonical_gap_id": opts.canonical_gap_id,
        "packet_id": packet_id_json,
        "packet_id_available": packet_id_available,
        "verify_command": opts.verify_command,
        "verify_status": opts.verify_status,
        "written_at": written_at,
        "limits_note": "Static evidence only. Receipt records what was run; does not certify semantic correctness."
    });

    json::render_pretty_with_newline(&value, "receipt")
}

/// Validate the structural integrity of a receipt JSON file, optionally
/// cross-referencing it against a gap-decision-ledger (RIPR-SPEC-0110).
///
/// Returns `(message, cross_ref_result)` on success.  The caller is
/// responsible for exiting non-zero when `cross_ref_result.is_error()`.
///
/// ## Fail-closed honesty rule
///
/// When `opts.ledger` is `None` or the ledger file cannot be read, the
/// `cross_ref_result` is always `ReceiptCrossRefResult::NotAvailable`.
/// This must NEVER be interpreted as "the receipt is ok / fresh" — the
/// absence of a ledger is not evidence of validity.
pub(crate) fn check_receipt(
    opts: &ReceiptCheckOptions,
) -> Result<(String, ReceiptCrossRefResult), String> {
    let path = resolve_check_path(opts)?;

    let content = std::fs::read_to_string(&path).map_err(|err| {
        format!(
            "receipt at {} not found or unreadable: {err}",
            path.display()
        )
    })?;

    let value: serde_json::Value = serde_json::from_str(&content).map_err(|err| {
        format!(
            "receipt at {} is malformed: invalid JSON: {err}",
            path.display()
        )
    })?;

    validate_receipt_structure(&value, &path)?;

    // Extract the canonical_gap_id from the validated receipt.
    let canonical_gap_id = value["canonical_gap_id"].as_str().unwrap_or("").to_string();

    // Cross-reference against the ledger when provided.
    let cross_ref = cross_reference_receipt(&canonical_gap_id, opts.ledger.as_deref());

    let msg = format!(
        "receipt at {} is structurally valid; cross_reference: {}",
        path.display(),
        cross_ref.as_str()
    );
    Ok((msg, cross_ref))
}

/// Cross-reference a receipt's `canonical_gap_id` against the live gap set in
/// a gap-decision-ledger JSON file.
///
/// Returns `NotAvailable` when `ledger_path` is `None` or cannot be read.
/// This is the fail-closed sentinel: absence of the ledger is NOT evidence
/// that the receipt is valid/fresh.
fn cross_reference_receipt(
    canonical_gap_id: &str,
    ledger_path: Option<&Path>,
) -> ReceiptCrossRefResult {
    let Some(path) = ledger_path else {
        return ReceiptCrossRefResult::NotAvailable;
    };

    let contents = match std::fs::read_to_string(path) {
        Ok(c) => c,
        Err(_) => return ReceiptCrossRefResult::NotAvailable,
    };

    let records = match parse_gap_records_json(&contents) {
        Ok(r) => r,
        Err(_) => return ReceiptCrossRefResult::NotAvailable,
    };

    // Search the live gap set for the receipt's canonical_gap_id.
    // We match on canonical_gap_id (primary) or gap_id (secondary).
    let matched = records.iter().find(|r| {
        (!r.canonical_gap_id.is_empty() && r.canonical_gap_id == canonical_gap_id)
            || (!r.gap_id.is_empty() && r.gap_id == canonical_gap_id)
    });

    match matched {
        None => ReceiptCrossRefResult::OrphanReceipt,
        Some(record) => {
            // If the record carries a dedupe_fingerprint, compare it against
            // the canonical_gap_id the receipt holds.  A mismatch means the
            // gap moved or changed identity.  If no fingerprint is available
            // we accept the match as-is (receipt_ok) — we do NOT invent a
            // fingerprint that doesn't exist in the data.
            if record
                .anchor
                .as_ref()
                .and_then(|a| a.dedupe_fingerprint.as_deref())
                .filter(|fp| !fp.is_empty() && *fp != canonical_gap_id)
                .is_some()
            {
                return ReceiptCrossRefResult::ReceiptGapMismatch;
            }
            ReceiptCrossRefResult::ReceiptOk
        }
    }
}

/// Return the output path for a receipt write operation.
pub(crate) fn receipt_out_path(opts: &ReceiptWriteOptions) -> PathBuf {
    match &opts.out {
        Some(p) => p.clone(),
        None => {
            PathBuf::from("target/ripr/receipts").join(format!("{}.json", opts.canonical_gap_id))
        }
    }
}

// ── internal helpers ──────────────────────────────────────────────────────────

fn validate_write_options(opts: &ReceiptWriteOptions) -> Result<(), String> {
    if opts.canonical_gap_id.trim().is_empty() {
        return Err("receipt requires a canonical_gap_id; re-run with --gap".to_string());
    }
    if opts.verify_command.trim().is_empty() {
        return Err("receipt requires --verify-command".to_string());
    }
    if opts.verify_status.trim().is_empty() {
        return Err(format!(
            "receipt requires --status ({})",
            VALID_VERIFY_STATUSES.join("|")
        ));
    }
    if !VALID_VERIFY_STATUSES.contains(&opts.verify_status.as_str()) {
        return Err(format!(
            "invalid --status {:?}; valid values are: {}",
            opts.verify_status,
            VALID_VERIFY_STATUSES.join(", ")
        ));
    }
    Ok(())
}

fn validate_receipt_structure(value: &serde_json::Value, path: &Path) -> Result<(), String> {
    let required_fields = [
        "schema_version",
        "tool",
        "kind",
        "canonical_gap_id",
        "verify_command",
        "verify_status",
        "written_at",
    ];

    for field in &required_fields {
        if value.get(field).is_none() {
            return Err(format!(
                "receipt at {} is malformed: missing required field `{field}`",
                path.display()
            ));
        }
    }

    // Validate verify_status is a known value.
    let status = value["verify_status"].as_str().ok_or_else(|| {
        format!(
            "receipt at {} is malformed: `verify_status` must be a string",
            path.display()
        )
    })?;

    if !VALID_VERIFY_STATUSES.contains(&status) {
        return Err(format!(
            "receipt at {} is malformed: `verify_status` {status:?} is not a valid value; \
             expected one of: {}",
            path.display(),
            VALID_VERIFY_STATUSES.join(", ")
        ));
    }

    Ok(())
}

fn resolve_check_path(opts: &ReceiptCheckOptions) -> Result<PathBuf, String> {
    match (&opts.path, &opts.gap) {
        (Some(p), _) => Ok(p.clone()),
        (None, Some(gap)) => Ok(PathBuf::from("target/ripr/receipts").join(format!("{gap}.json"))),
        (None, None) => Err(
            "receipt check requires --path <receipt_path> or --gap <canonical_gap_id>".to_string(),
        ),
    }
}

fn written_at_rfc3339() -> Result<String, String> {
    use std::time::{SystemTime, UNIX_EPOCH};
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|err| format!("system clock before unix epoch: {err}"))?
        .as_secs();
    // Format as ISO 8601 / RFC 3339 UTC without external deps.
    let (year, month, day, hour, minute, second) = seconds_to_ymdhms(secs);
    Ok(format!(
        "{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:{second:02}Z"
    ))
}

fn seconds_to_ymdhms(mut secs: u64) -> (u64, u64, u64, u64, u64, u64) {
    let second = secs % 60;
    secs /= 60;
    let minute = secs % 60;
    secs /= 60;
    let hour = secs % 24;
    secs /= 24;

    // Days since 1970-01-01 (Gregorian proleptic calendar).
    let mut days = secs;
    let mut year = 1970u64;
    loop {
        let days_in_year = if is_leap(year) { 366 } else { 365 };
        if days < days_in_year {
            break;
        }
        days -= days_in_year;
        year += 1;
    }
    let leap = is_leap(year);
    let months = [
        31u64,
        if leap { 29 } else { 28 },
        31,
        30,
        31,
        30,
        31,
        31,
        30,
        31,
        30,
        31,
    ];
    let mut month = 1u64;
    for &m in &months {
        if days < m {
            break;
        }
        days -= m;
        month += 1;
    }
    let day = days + 1;
    (year, month, day, hour, minute, second)
}

fn is_leap(year: u64) -> bool {
    year.is_multiple_of(400) || (year.is_multiple_of(4) && !year.is_multiple_of(100))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write_opts(gap: &str, status: &str) -> ReceiptWriteOptions {
        ReceiptWriteOptions {
            canonical_gap_id: gap.to_string(),
            packet_id: None,
            verify_command: "cargo test -p ripr".to_string(),
            verify_status: status.to_string(),
            out: None,
            json: true,
        }
    }

    // ── write happy path ──────────────────────────────────────────────────────

    #[test]
    fn receipt_write_valid_args_writes_json() -> Result<(), String> {
        let opts = ReceiptWriteOptions {
            canonical_gap_id: "crates_ripr_src_lib.rs:error_path:c1a03250".to_string(),
            packet_id: Some("packet-abc123".to_string()),
            verify_command: "cargo test -p ripr".to_string(),
            verify_status: "passed".to_string(),
            out: None,
            json: true,
        };
        let rendered = write_receipt(&opts)?;
        let value: serde_json::Value = serde_json::from_str(&rendered)
            .map_err(|err| format!("receipt JSON should parse: {err}"))?;

        assert_eq!(value["schema_version"], "0.1");
        assert_eq!(value["tool"], "ripr");
        assert_eq!(value["kind"], "receipt");
        assert_eq!(
            value["canonical_gap_id"],
            "crates_ripr_src_lib.rs:error_path:c1a03250"
        );
        assert_eq!(value["packet_id"], "packet-abc123");
        assert_eq!(value["packet_id_available"], true);
        assert_eq!(value["verify_command"], "cargo test -p ripr");
        assert_eq!(value["verify_status"], "passed");
        assert!(value["written_at"].as_str().unwrap_or("").contains('T'));
        assert!(
            value["limits_note"]
                .as_str()
                .unwrap_or("")
                .contains("Static evidence")
        );
        Ok(())
    }

    #[test]
    fn receipt_write_without_packet_id_records_null() -> Result<(), String> {
        let opts = write_opts("crates_ripr_src_lib.rs:error_path:c1a03250", "not_run");
        let rendered = write_receipt(&opts)?;
        let value: serde_json::Value = serde_json::from_str(&rendered)
            .map_err(|err| format!("receipt JSON should parse: {err}"))?;
        assert_eq!(value["packet_id"], serde_json::Value::Null);
        assert_eq!(value["packet_id_available"], false);
        Ok(())
    }

    #[test]
    fn receipt_write_all_valid_statuses_accepted() -> Result<(), String> {
        for status in VALID_VERIFY_STATUSES {
            write_receipt(&write_opts("gap:test:aabbccdd", status))
                .map_err(|e| format!("status {status:?} should be accepted: {e}"))?;
        }
        Ok(())
    }

    // ── write fail-closed ─────────────────────────────────────────────────────

    #[test]
    fn receipt_write_missing_gap_exits_nonzero() -> Result<(), String> {
        let opts = ReceiptWriteOptions {
            canonical_gap_id: "".to_string(),
            packet_id: None,
            verify_command: "cargo test".to_string(),
            verify_status: "passed".to_string(),
            out: None,
            json: true,
        };
        match write_receipt(&opts) {
            Ok(_) => Err("write_receipt should have failed with missing gap".to_string()),
            Err(err) => {
                if err.contains("canonical_gap_id") {
                    Ok(())
                } else {
                    Err(format!("error should mention canonical_gap_id, got: {err}"))
                }
            }
        }
    }

    #[test]
    fn receipt_write_invalid_status_exits_nonzero() -> Result<(), String> {
        let opts = write_opts("gap:demo:aabbccdd", "bogus");
        match write_receipt(&opts) {
            Ok(_) => Err("write_receipt should have failed with invalid status".to_string()),
            Err(err) => {
                if !err.contains("bogus") {
                    return Err(format!("error should echo the bad status, got: {err}"));
                }
                if !err.contains("passed") {
                    return Err(format!("error should list valid values, got: {err}"));
                }
                Ok(())
            }
        }
    }

    #[test]
    fn receipt_write_missing_verify_command_exits_nonzero() -> Result<(), String> {
        let opts = ReceiptWriteOptions {
            canonical_gap_id: "gap:demo:aabbccdd".to_string(),
            packet_id: None,
            verify_command: "".to_string(),
            verify_status: "passed".to_string(),
            out: None,
            json: true,
        };
        match write_receipt(&opts) {
            Ok(_) => {
                Err("write_receipt should have failed with missing verify-command".to_string())
            }
            Err(err) => {
                if err.contains("--verify-command") {
                    Ok(())
                } else {
                    Err(format!("error should mention --verify-command, got: {err}"))
                }
            }
        }
    }

    // ── check happy path ──────────────────────────────────────────────────────

    #[test]
    fn receipt_check_valid_receipt_exits_zero() -> Result<(), String> {
        let dir =
            std::env::temp_dir().join(format!("ripr-receipt-check-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).map_err(|e| format!("create temp dir failed: {e}"))?;
        let path = dir.join("receipt.json");

        let opts = write_opts("gap:check:aabbccdd", "passed");
        let rendered = write_receipt(&opts)?;
        std::fs::write(&path, &rendered).map_err(|e| format!("write temp receipt failed: {e}"))?;

        let check_opts = ReceiptCheckOptions {
            gap: None,
            path: Some(path.clone()),
            ledger: None,
        };
        let (result, cross_ref) = check_receipt(&check_opts)?;
        assert!(
            result.contains("structurally valid"),
            "should report valid, got: {result}"
        );
        // No ledger provided → cross-reference must be not_available (fail-closed).
        assert_eq!(
            cross_ref,
            ReceiptCrossRefResult::NotAvailable,
            "cross_ref should be not_available when no ledger given"
        );

        let _ = std::fs::remove_dir_all(&dir);
        Ok(())
    }

    // ── check fail-closed ─────────────────────────────────────────────────────

    #[test]
    fn receipt_check_missing_file_exits_nonzero() -> Result<(), String> {
        let opts = ReceiptCheckOptions {
            gap: None,
            path: Some(PathBuf::from(
                "target/ripr/receipts/nonexistent-receipt.json",
            )),
            ledger: None,
        };
        match check_receipt(&opts) {
            Ok(_) => Err("check_receipt should have failed for missing file".to_string()),
            Err(err) => {
                if err.contains("not found") || err.contains("unreadable") {
                    Ok(())
                } else {
                    Err(format!(
                        "error should say not found or unreadable, got: {err}"
                    ))
                }
            }
        }
    }

    #[test]
    fn receipt_check_malformed_json_exits_nonzero() -> Result<(), String> {
        let dir = std::env::temp_dir().join(format!(
            "ripr-receipt-check-malformed-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&dir).map_err(|e| format!("create temp dir failed: {e}"))?;
        let path = dir.join("bad.json");
        std::fs::write(&path, b"{ not valid json")
            .map_err(|e| format!("write bad json failed: {e}"))?;

        let check_opts = ReceiptCheckOptions {
            gap: None,
            path: Some(path),
            ledger: None,
        };
        match check_receipt(&check_opts) {
            Ok(_) => {
                let _ = std::fs::remove_dir_all(&dir);
                Err("check_receipt should have failed for malformed JSON".to_string())
            }
            Err(err) => {
                let _ = std::fs::remove_dir_all(&dir);
                if err.contains("malformed") {
                    Ok(())
                } else {
                    Err(format!("error should say malformed, got: {err}"))
                }
            }
        }
    }

    #[test]
    fn receipt_check_missing_required_field_exits_nonzero() -> Result<(), String> {
        let dir = std::env::temp_dir().join(format!(
            "ripr-receipt-check-missing-field-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&dir).map_err(|e| format!("create temp dir failed: {e}"))?;
        let path = dir.join("missing_field.json");
        // Missing canonical_gap_id
        let json = serde_json::json!({
            "schema_version": "0.1",
            "tool": "ripr",
            "kind": "receipt",
            "verify_command": "cargo test",
            "verify_status": "passed",
            "written_at": "2026-06-11T00:00:00Z"
        });
        std::fs::write(&path, json.to_string()).map_err(|e| format!("write json failed: {e}"))?;

        let check_opts = ReceiptCheckOptions {
            gap: None,
            path: Some(path),
            ledger: None,
        };
        match check_receipt(&check_opts) {
            Ok(_) => {
                let _ = std::fs::remove_dir_all(&dir);
                Err("check_receipt should have failed for missing field".to_string())
            }
            Err(err) => {
                let _ = std::fs::remove_dir_all(&dir);
                if err.contains("canonical_gap_id") {
                    Ok(())
                } else {
                    Err(format!("error should mention canonical_gap_id, got: {err}"))
                }
            }
        }
    }

    #[test]
    fn receipt_check_invalid_status_in_file_exits_nonzero() -> Result<(), String> {
        let dir = std::env::temp_dir().join(format!(
            "ripr-receipt-check-bad-status-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&dir).map_err(|e| format!("create temp dir failed: {e}"))?;
        let path = dir.join("bad_status.json");
        let json = serde_json::json!({
            "schema_version": "0.1",
            "tool": "ripr",
            "kind": "receipt",
            "canonical_gap_id": "gap:test:aabbccdd",
            "verify_command": "cargo test",
            "verify_status": "invalid_status",
            "written_at": "2026-06-11T00:00:00Z"
        });
        std::fs::write(&path, json.to_string()).map_err(|e| format!("write json failed: {e}"))?;

        let check_opts = ReceiptCheckOptions {
            gap: None,
            path: Some(path),
            ledger: None,
        };
        match check_receipt(&check_opts) {
            Ok(_) => {
                let _ = std::fs::remove_dir_all(&dir);
                Err("check_receipt should have failed for invalid status".to_string())
            }
            Err(err) => {
                let _ = std::fs::remove_dir_all(&dir);
                if err.contains("invalid_status") {
                    Ok(())
                } else {
                    Err(format!(
                        "error should mention the bad status value, got: {err}"
                    ))
                }
            }
        }
    }

    #[test]
    fn receipt_check_no_path_no_gap_exits_nonzero() -> Result<(), String> {
        let opts = ReceiptCheckOptions {
            gap: None,
            path: None,
            ledger: None,
        };
        match check_receipt(&opts) {
            Ok(_) => Err("check_receipt should have failed with no path and no gap".to_string()),
            Err(err) => {
                if err.contains("--path") || err.contains("--gap") {
                    Ok(())
                } else {
                    Err(format!("error should require path or gap, got: {err}"))
                }
            }
        }
    }

    // ── RIPR-SPEC-0110 controls: cross-reference tests ────────────────────────

    fn make_receipt_file(dir: &std::path::Path, gap_id: &str) -> Result<PathBuf, String> {
        let path = dir.join("receipt.json");
        let json = serde_json::json!({
            "schema_version": "0.1",
            "tool": "ripr",
            "kind": "receipt",
            "canonical_gap_id": gap_id,
            "verify_command": "cargo test",
            "verify_status": "passed",
            "written_at": "2026-06-14T00:00:00Z"
        });
        std::fs::write(&path, json.to_string())
            .map_err(|e| format!("write receipt failed: {e}"))?;
        Ok(path)
    }

    fn make_ledger_file(dir: &std::path::Path, gap_ids: &[&str]) -> Result<PathBuf, String> {
        let path = dir.join("ledger.json");
        let records: Vec<serde_json::Value> = gap_ids
            .iter()
            .map(|id| {
                serde_json::json!({
                    "gap_id": id,
                    "canonical_gap_id": id,
                    "kind": "MissingValueAssertion",
                    "language": "rust",
                    "language_status": "stable",
                    "scope": "repo_scoped",
                    "evidence_class": "return_value",
                    "gap_state": "actionable",
                    "policy_state": "new",
                    "repairability": "repairable",
                    "authority_boundary": "gate_decision_artifact_only"
                })
            })
            .collect();
        let ledger = serde_json::json!(records);
        std::fs::write(&path, ledger.to_string())
            .map_err(|e| format!("write ledger failed: {e}"))?;
        Ok(path)
    }

    /// Control 1 (RIPR-SPEC-0110): receipt's canonical_gap_id NOT in the live
    /// gap set → `orphan_receipt`.
    #[test]
    fn receipt_check_orphan_when_gap_absent_from_ledger() -> Result<(), String> {
        let dir = std::env::temp_dir().join(format!("ripr-xref-orphan-{}", std::process::id()));
        std::fs::create_dir_all(&dir).map_err(|e| format!("create temp dir failed: {e}"))?;

        let receipt_path = make_receipt_file(&dir, "gap:missing:aabbccdd")?;
        // Ledger contains a DIFFERENT gap — the receipt's gap is absent.
        let ledger_path = make_ledger_file(&dir, &["gap:other:1234abcd"])?;

        let opts = ReceiptCheckOptions {
            gap: None,
            path: Some(receipt_path),
            ledger: Some(ledger_path),
        };
        let (msg, cross_ref) = check_receipt(&opts)?;
        assert_eq!(
            cross_ref,
            ReceiptCrossRefResult::OrphanReceipt,
            "gap absent from ledger → orphan_receipt; msg: {msg}"
        );
        assert!(
            msg.contains("orphan_receipt"),
            "message should mention orphan_receipt, got: {msg}"
        );

        let _ = std::fs::remove_dir_all(&dir);
        Ok(())
    }

    /// Control 2 (RIPR-SPEC-0110): no `--ledger` → cross-reference is
    /// `not_available` (NEVER `receipt_ok`); structural check still exits 0.
    /// This is the fail-closed core: ledger-absent ≠ receipt valid.
    #[test]
    fn receipt_check_cross_reference_not_available_when_ledger_absent() -> Result<(), String> {
        let dir = std::env::temp_dir().join(format!("ripr-xref-noledger-{}", std::process::id()));
        std::fs::create_dir_all(&dir).map_err(|e| format!("create temp dir failed: {e}"))?;

        let receipt_path = make_receipt_file(&dir, "gap:demo:aabbccdd")?;

        let opts = ReceiptCheckOptions {
            gap: None,
            path: Some(receipt_path),
            ledger: None, // ← no ledger
        };
        let (msg, cross_ref) = check_receipt(&opts)?;

        // Must be not_available, NEVER receipt_ok.
        assert_eq!(
            cross_ref,
            ReceiptCrossRefResult::NotAvailable,
            "ledger absent → must be not_available, not receipt_ok; msg: {msg}"
        );
        assert_ne!(
            cross_ref,
            ReceiptCrossRefResult::ReceiptOk,
            "ledger absent must NEVER produce receipt_ok"
        );
        assert!(
            msg.contains("not_available"),
            "message should contain not_available, got: {msg}"
        );
        // Structural check succeeds → check_receipt returns Ok (exit 0 semantics).

        let _ = std::fs::remove_dir_all(&dir);
        Ok(())
    }

    /// Control 3 (RIPR-SPEC-0110): receipt's canonical_gap_id IS in the live
    /// gap set → `receipt_ok`.
    #[test]
    fn receipt_check_ok_when_gap_present() -> Result<(), String> {
        let dir = std::env::temp_dir().join(format!("ripr-xref-ok-{}", std::process::id()));
        std::fs::create_dir_all(&dir).map_err(|e| format!("create temp dir failed: {e}"))?;

        let gap_id = "gap:present:aabbccdd";
        let receipt_path = make_receipt_file(&dir, gap_id)?;
        let ledger_path = make_ledger_file(&dir, &[gap_id])?;

        let opts = ReceiptCheckOptions {
            gap: None,
            path: Some(receipt_path),
            ledger: Some(ledger_path),
        };
        let (msg, cross_ref) = check_receipt(&opts)?;
        assert_eq!(
            cross_ref,
            ReceiptCrossRefResult::ReceiptOk,
            "gap present in ledger → receipt_ok; msg: {msg}"
        );
        assert!(
            msg.contains("receipt_ok"),
            "message should mention receipt_ok, got: {msg}"
        );

        let _ = std::fs::remove_dir_all(&dir);
        Ok(())
    }

    // ── receipt_out_path resolution ───────────────────────────────────────────

    #[test]
    fn receipt_out_path_uses_explicit_path_when_provided() {
        let opts = ReceiptWriteOptions {
            canonical_gap_id: "gap:test:aabbccdd".to_string(),
            packet_id: None,
            verify_command: "cargo test".to_string(),
            verify_status: "passed".to_string(),
            out: Some(PathBuf::from("custom/path/r.json")),
            json: true,
        };
        assert_eq!(receipt_out_path(&opts), PathBuf::from("custom/path/r.json"));
    }

    #[test]
    fn receipt_out_path_defaults_to_canonical_location() {
        let opts = write_opts("gap:test:aabbccdd", "passed");
        assert_eq!(
            receipt_out_path(&opts),
            PathBuf::from("target/ripr/receipts/gap:test:aabbccdd.json")
        );
    }

    // ── written_at format ─────────────────────────────────────────────────────

    #[test]
    fn written_at_rfc3339_produces_valid_timestamp() -> Result<(), String> {
        let ts = written_at_rfc3339()?;
        // Should look like 2026-06-11T12:34:56Z
        assert!(
            ts.len() == 20 && ts.ends_with('Z') && ts.contains('T'),
            "timestamp should be RFC3339 UTC: {ts}"
        );
        Ok(())
    }
}
