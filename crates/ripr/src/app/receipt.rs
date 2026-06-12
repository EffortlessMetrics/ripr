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
//!   (PR 3 scope: structural write + check only; cross-gap validation is PR 4).
//! - `receipt check` validates JSON structure only; orphan/stale detection is
//!   deferred to PR 4 (requires gap cross-reference).
//! - All error paths are fail-closed: on any validation failure nothing is
//!   written and a non-zero exit is triggered via `Err(String)`.

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

/// Options for `ripr receipt check`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ReceiptCheckOptions {
    /// Resolve path from canonical location when `path` is absent.
    pub(crate) gap: Option<String>,
    /// Explicit path to a receipt file.
    pub(crate) path: Option<PathBuf>,
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

/// Validate the structural integrity of a receipt JSON file and return a
/// structured-OK message.  Does NOT cross-reference the gap ledger
/// (orphan/stale detection is PR 4 scope).
pub(crate) fn check_receipt(opts: &ReceiptCheckOptions) -> Result<String, String> {
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

    Ok(format!(
        "receipt at {} is structurally valid (structural check only; orphan/stale detection requires gap cross-reference — see PR 4)",
        path.display()
    ))
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
        };
        let result = check_receipt(&check_opts)?;
        assert!(
            result.contains("structurally valid"),
            "should report valid, got: {result}"
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
