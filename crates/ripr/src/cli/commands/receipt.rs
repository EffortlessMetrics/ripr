//! Arg-parsing and dispatch for `ripr receipt write / check`.
//!
//! This is the CLI adapter layer only.  All write/check logic lives in
//! `crate::app::receipt`.  This module is responsible for argument parsing
//! and calling into the app layer.

use crate::app::receipt::{
    ReceiptCheckOptions, ReceiptWriteOptions, check_receipt, receipt_out_path, write_receipt,
};
use crate::cli::parse::expect_value;
use crate::output;
use std::path::PathBuf;

// ── top-level receipt command ─────────────────────────────────────────────────

pub(in crate::cli) fn run_receipt(args: &[String]) -> Result<(), String> {
    match args.first().map(|s| s.as_str()) {
        None | Some("--help" | "-h") => {
            print_receipt_help();
            Ok(())
        }
        Some("write") => run_receipt_write(&args[1..]),
        Some("check") => run_receipt_check(&args[1..]),
        Some(other) => Err(format!(
            "unknown receipt subcommand {other:?}; expected `write` or `check`"
        )),
    }
}

// ── receipt write ─────────────────────────────────────────────────────────────

fn run_receipt_write(args: &[String]) -> Result<(), String> {
    if args.iter().any(|a| a == "--help" || a == "-h") {
        print_receipt_write_help();
        return Ok(());
    }

    let opts = parse_receipt_write_options(args)?;
    let rendered = write_receipt(&opts)?;

    if opts.json || opts.out.is_none() {
        // When --out is given without --json we still print the JSON to stdout
        // (the file write happens below regardless).  When --json alone, print.
        // Either way: print if --json flag is set OR no --out path given.
    }

    let path = receipt_out_path(&opts);

    if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
        std::fs::create_dir_all(parent)
            .map_err(|err| format!("create {} failed: {err}", parent.display()))?;
    }

    std::fs::write(&path, &rendered).map_err(|err| {
        format!(
            "write {} failed: {err}",
            output::outcome::display_path(&path)
        )
    })?;

    // Always print the rendered JSON (matches spec: --out writes AND prints).
    print!("{rendered}");
    Ok(())
}

// ── receipt check ─────────────────────────────────────────────────────────────

fn run_receipt_check(args: &[String]) -> Result<(), String> {
    if args.iter().any(|a| a == "--help" || a == "-h") {
        print_receipt_check_help();
        return Ok(());
    }

    let opts = parse_receipt_check_options(args)?;
    let msg = check_receipt(&opts)?;
    println!("{msg}");
    Ok(())
}

// ── option parsers ────────────────────────────────────────────────────────────

pub(in crate::cli) fn parse_receipt_write_options(
    args: &[String],
) -> Result<ReceiptWriteOptions, String> {
    let mut canonical_gap_id: Option<String> = None;
    let mut packet_id: Option<String> = None;
    let mut verify_command: Option<String> = None;
    let mut verify_status: Option<String> = None;
    let mut out: Option<PathBuf> = None;
    let mut json = false;

    let mut i = 0usize;
    while i < args.len() {
        match args[i].as_str() {
            "--gap" => {
                i += 1;
                let value = expect_value(args, i, "--gap")?;
                if value.trim().is_empty() {
                    return Err(
                        "receipt requires a canonical_gap_id; re-run with --gap".to_string()
                    );
                }
                canonical_gap_id = Some(value.to_string());
            }
            "--packet" => {
                i += 1;
                let value = expect_value(args, i, "--packet")?;
                if value.trim().is_empty() {
                    return Err("receipt write --packet requires a non-empty packet ID".to_string());
                }
                packet_id = Some(value.to_string());
            }
            "--verify-command" => {
                i += 1;
                let value = expect_value(args, i, "--verify-command")?;
                if value.trim().is_empty() {
                    return Err("receipt requires --verify-command".to_string());
                }
                verify_command = Some(value.to_string());
            }
            "--status" => {
                i += 1;
                let value = expect_value(args, i, "--status")?;
                verify_status = Some(value.to_string());
            }
            "--out" => {
                i += 1;
                let value = expect_value(args, i, "--out")?;
                if value.trim().is_empty() {
                    return Err("receipt write --out requires a non-empty path".to_string());
                }
                out = Some(PathBuf::from(value));
            }
            "--json" => json = true,
            other => return Err(format!("unknown receipt write argument {other:?}")),
        }
        i += 1;
    }

    let canonical_gap_id = canonical_gap_id
        .ok_or_else(|| "receipt requires a canonical_gap_id; re-run with --gap".to_string())?;
    let verify_command =
        verify_command.ok_or_else(|| "receipt requires --verify-command".to_string())?;
    let verify_status = verify_status
        .ok_or_else(|| "receipt requires --status (passed|failed|not_run|unknown)".to_string())?;

    Ok(ReceiptWriteOptions {
        canonical_gap_id,
        packet_id,
        verify_command,
        verify_status,
        out,
        json,
    })
}

pub(in crate::cli) fn parse_receipt_check_options(
    args: &[String],
) -> Result<ReceiptCheckOptions, String> {
    let mut gap: Option<String> = None;
    let mut path: Option<PathBuf> = None;

    let mut i = 0usize;
    while i < args.len() {
        match args[i].as_str() {
            "--gap" => {
                i += 1;
                let value = expect_value(args, i, "--gap")?;
                if value.trim().is_empty() {
                    return Err(
                        "receipt check --gap requires a non-empty canonical_gap_id".to_string()
                    );
                }
                gap = Some(value.to_string());
            }
            "--path" => {
                i += 1;
                let value = expect_value(args, i, "--path")?;
                if value.trim().is_empty() {
                    return Err("receipt check --path requires a non-empty path".to_string());
                }
                path = Some(PathBuf::from(value));
            }
            // Also allow a bare positional path argument (spec: `ripr receipt check [<path-or-dir>]`).
            other if !other.starts_with('-') => {
                if path.is_some() {
                    return Err(
                        "receipt check: multiple positional path arguments are not supported"
                            .to_string(),
                    );
                }
                path = Some(PathBuf::from(other));
            }
            other => return Err(format!("unknown receipt check argument {other:?}")),
        }
        i += 1;
    }

    Ok(ReceiptCheckOptions { gap, path })
}

// ── help text ─────────────────────────────────────────────────────────────────

fn print_receipt_help() {
    println!("{RECEIPT_HELP}");
}

fn print_receipt_write_help() {
    println!("{RECEIPT_WRITE_HELP}");
}

fn print_receipt_check_help() {
    println!("{RECEIPT_CHECK_HELP}");
}

pub(in crate::cli) const RECEIPT_HELP: &str = r#"Write and validate canonical repair receipts (RIPR-SPEC-0079).

Usage: ripr receipt <subcommand>

Subcommands:
  write    Author a receipt JSON for a completed repair attempt.
  check    Structurally validate a receipt JSON file.

The `ripr receipt write` command is the canonical receipt command.
New emitters must use `ripr receipt write`; `ripr agent receipt` remains
a legacy alias during the transition (tracked in issue #1123).

Run `ripr receipt write --help` or `ripr receipt check --help` for details.
"#;

pub(in crate::cli) const RECEIPT_WRITE_HELP: &str = r#"Author a canonical repair receipt for a completed gap repair attempt.

Usage: ripr receipt write --gap <canonical_gap_id> [--packet <packet_id>]
                          --verify-command "<cmd>" --status <verify_status>
                          [--out <path>] [--json]

Options:
  --gap ID              The canonical_gap_id this receipt closes. Required.
  --packet ID           The repair packet the agent acted on. Optional; records
                        null when not available.
  --verify-command CMD  The exact shell command run to verify the repair. Required.
  --status STATUS       Outcome of the verify command. Required.
                        Valid values: passed, failed, not_run, unknown.
  --out PATH            Write receipt JSON to this path. When omitted, writes to
                        target/ripr/receipts/<canonical_gap_id>.json.
  --json                Print JSON output to stdout (also written to --out path).

Fail-closed:
  Missing --gap or --verify-command or --status -> error, no receipt written.
  Invalid --status value -> error listing valid values, no receipt written.

This command is the canonical receipt writer per RIPR-SPEC-0079.
The legacy alias `ripr agent receipt` continues to work during the transition.
"#;

pub(in crate::cli) const RECEIPT_CHECK_HELP: &str = r#"Structurally validate a receipt JSON file.

Usage: ripr receipt check [--path <receipt_path>] [--gap <canonical_gap_id>]
       ripr receipt check <receipt_path>

Options:
  --path PATH   Path to the receipt JSON file to validate.
  --gap ID      Resolve path from canonical location
                target/ripr/receipts/<canonical_gap_id>.json.

When --gap is provided without --path, the path is resolved from the canonical
location.

Exits 0 if the receipt JSON is structurally valid (required fields present,
verify_status is a known value, JSON is parseable). Exits non-zero with an
explicit message on any error.

Note: This subcommand performs structural validation only. Orphan and stale
detection (cross-referencing the gap ledger to confirm the gap still exists)
is deferred to PR 4 of issue #1123 and is not performed here.
"#;

// ── tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::receipt::VALID_VERIFY_STATUSES;

    fn args(values: &[&str]) -> Vec<String> {
        values.iter().map(|v| v.to_string()).collect()
    }

    // ── write option parsing ──────────────────────────────────────────────────

    #[test]
    fn receipt_write_parse_full_options() -> Result<(), String> {
        let result = parse_receipt_write_options(&args(&[
            "--gap",
            "gap:test:aabbccdd",
            "--packet",
            "packet-123",
            "--verify-command",
            "cargo test -p ripr",
            "--status",
            "passed",
            "--out",
            "target/ripr/receipts/r.json",
            "--json",
        ]))?;
        assert_eq!(result.canonical_gap_id, "gap:test:aabbccdd");
        assert_eq!(result.packet_id, Some("packet-123".to_string()));
        assert_eq!(result.verify_command, "cargo test -p ripr");
        assert_eq!(result.verify_status, "passed");
        assert_eq!(
            result.out,
            Some(PathBuf::from("target/ripr/receipts/r.json"))
        );
        assert!(result.json);
        Ok(())
    }

    #[test]
    fn receipt_write_parse_minimal_options() -> Result<(), String> {
        let result = parse_receipt_write_options(&args(&[
            "--gap",
            "gap:test:aabbccdd",
            "--verify-command",
            "cargo test",
            "--status",
            "not_run",
        ]))?;
        assert_eq!(result.packet_id, None);
        assert_eq!(result.out, None);
        assert!(!result.json);
        Ok(())
    }

    #[test]
    fn receipt_write_missing_gap_returns_error() -> Result<(), String> {
        match parse_receipt_write_options(&args(&[
            "--verify-command",
            "cargo test",
            "--status",
            "passed",
        ])) {
            Ok(_) => Err("should have failed with missing gap".to_string()),
            Err(err) => {
                if err.contains("canonical_gap_id") {
                    Ok(())
                } else {
                    Err(format!("expected canonical_gap_id in error, got: {err}"))
                }
            }
        }
    }

    #[test]
    fn receipt_write_empty_gap_returns_error() -> Result<(), String> {
        match parse_receipt_write_options(&args(&[
            "--gap",
            "",
            "--verify-command",
            "cargo test",
            "--status",
            "passed",
        ])) {
            Ok(_) => Err("should have failed with empty gap".to_string()),
            Err(err) => {
                if err.contains("canonical_gap_id") {
                    Ok(())
                } else {
                    Err(format!("expected canonical_gap_id in error, got: {err}"))
                }
            }
        }
    }

    #[test]
    fn receipt_write_missing_verify_command_returns_error() -> Result<(), String> {
        match parse_receipt_write_options(&args(&[
            "--gap",
            "gap:test:aabbccdd",
            "--status",
            "passed",
        ])) {
            Ok(_) => Err("should have failed with missing verify-command".to_string()),
            Err(err) => {
                if err.contains("--verify-command") {
                    Ok(())
                } else {
                    Err(format!("expected --verify-command in error, got: {err}"))
                }
            }
        }
    }

    #[test]
    fn receipt_write_missing_status_returns_error() -> Result<(), String> {
        match parse_receipt_write_options(&args(&[
            "--gap",
            "gap:test:aabbccdd",
            "--verify-command",
            "cargo test",
        ])) {
            Ok(_) => Err("should have failed with missing status".to_string()),
            Err(err) => {
                if err.contains("--status") {
                    Ok(())
                } else {
                    Err(format!("expected --status in error, got: {err}"))
                }
            }
        }
    }

    #[test]
    fn receipt_write_rejects_unknown_argument() -> Result<(), String> {
        match parse_receipt_write_options(&args(&[
            "--gap",
            "gap:test",
            "--verify-command",
            "cmd",
            "--status",
            "passed",
            "--unknown-flag",
        ])) {
            Ok(_) => Err("should have failed with unknown argument".to_string()),
            Err(err) => {
                if err.contains("unknown") {
                    Ok(())
                } else {
                    Err(format!("expected unknown in error, got: {err}"))
                }
            }
        }
    }

    #[test]
    fn receipt_write_requires_values_for_value_flags() -> Result<(), String> {
        for flag in &["--gap", "--packet", "--verify-command", "--status", "--out"] {
            match parse_receipt_write_options(&args(&[flag])) {
                Ok(_) => {
                    return Err(format!("flag {flag} should require value"));
                }
                Err(err) => {
                    if !err.contains("missing value") {
                        return Err(format!(
                            "flag {flag} should say 'missing value', got: {err}"
                        ));
                    }
                }
            }
        }
        Ok(())
    }

    // ── check option parsing ──────────────────────────────────────────────────

    #[test]
    fn receipt_check_parse_path_option() -> Result<(), String> {
        let result =
            parse_receipt_check_options(&args(&["--path", "target/ripr/receipts/r.json"]))?;
        assert_eq!(
            result.path,
            Some(PathBuf::from("target/ripr/receipts/r.json"))
        );
        assert_eq!(result.gap, None);
        Ok(())
    }

    #[test]
    fn receipt_check_parse_gap_option() -> Result<(), String> {
        let result = parse_receipt_check_options(&args(&["--gap", "gap:test:aabbccdd"]))?;
        assert_eq!(result.gap, Some("gap:test:aabbccdd".to_string()));
        assert_eq!(result.path, None);
        Ok(())
    }

    #[test]
    fn receipt_check_parse_positional_path() -> Result<(), String> {
        let result = parse_receipt_check_options(&args(&["target/ripr/receipts/r.json"]))?;
        assert_eq!(
            result.path,
            Some(PathBuf::from("target/ripr/receipts/r.json"))
        );
        Ok(())
    }

    #[test]
    fn receipt_check_parse_empty_gives_both_none() -> Result<(), String> {
        let result = parse_receipt_check_options(&args(&[]))?;
        assert_eq!(result.gap, None);
        assert_eq!(result.path, None);
        Ok(())
    }

    #[test]
    fn receipt_check_rejects_empty_path() -> Result<(), String> {
        match parse_receipt_check_options(&args(&["--path", ""])) {
            Ok(_) => Err("should have failed with empty path".to_string()),
            Err(err) => {
                if err.contains("non-empty") {
                    Ok(())
                } else {
                    Err(format!("expected non-empty in error, got: {err}"))
                }
            }
        }
    }

    #[test]
    fn receipt_check_rejects_unknown_argument() -> Result<(), String> {
        match parse_receipt_check_options(&args(&["--unknown"])) {
            Ok(_) => Err("should have failed with unknown argument".to_string()),
            Err(err) => {
                if err.contains("unknown") {
                    Ok(())
                } else {
                    Err(format!("expected unknown in error, got: {err}"))
                }
            }
        }
    }

    // ── dispatch ──────────────────────────────────────────────────────────────

    #[test]
    fn run_receipt_unknown_subcommand_returns_error() -> Result<(), String> {
        match run_receipt(&args(&["unknown"])) {
            Ok(_) => Err("should have failed with unknown subcommand".to_string()),
            Err(err) => {
                if err.contains("unknown receipt subcommand") {
                    Ok(())
                } else {
                    Err(format!(
                        "expected 'unknown receipt subcommand' in error, got: {err}"
                    ))
                }
            }
        }
    }

    #[test]
    fn run_receipt_no_args_prints_help() {
        assert_eq!(run_receipt(&args(&[])), Ok(()));
    }

    // ── help text content ─────────────────────────────────────────────────────

    #[test]
    fn receipt_help_text_mentions_key_content() {
        assert!(RECEIPT_HELP.contains("ripr receipt write"));
        assert!(RECEIPT_HELP.contains("ripr receipt check"));
        assert!(RECEIPT_HELP.contains("RIPR-SPEC-0079"));
        assert!(RECEIPT_HELP.contains("ripr agent receipt"));
        assert!(RECEIPT_WRITE_HELP.contains("--gap"));
        assert!(RECEIPT_WRITE_HELP.contains("--verify-command"));
        assert!(RECEIPT_WRITE_HELP.contains("--status"));
        assert!(RECEIPT_WRITE_HELP.contains("passed"));
        assert!(RECEIPT_WRITE_HELP.contains("failed"));
        assert!(RECEIPT_WRITE_HELP.contains("not_run"));
        assert!(RECEIPT_WRITE_HELP.contains("unknown"));
        assert!(RECEIPT_WRITE_HELP.contains("canonical_gap_id"));
        assert!(RECEIPT_CHECK_HELP.contains("structural"));
        assert!(RECEIPT_CHECK_HELP.contains("--path"));
        assert!(RECEIPT_CHECK_HELP.contains("--gap"));
        // Confirm orphan/stale non-goal is documented
        assert!(RECEIPT_CHECK_HELP.contains("Orphan") || RECEIPT_CHECK_HELP.contains("orphan"));
    }

    #[test]
    fn valid_verify_statuses_are_the_four_canonical_values() {
        // Confirm the four canonical values from RIPR-SPEC-0079 are present.
        assert!(VALID_VERIFY_STATUSES.contains(&"passed"));
        assert!(VALID_VERIFY_STATUSES.contains(&"failed"));
        assert!(VALID_VERIFY_STATUSES.contains(&"not_run"));
        assert!(VALID_VERIFY_STATUSES.contains(&"unknown"));
        assert_eq!(VALID_VERIFY_STATUSES.len(), 4);
    }
}
