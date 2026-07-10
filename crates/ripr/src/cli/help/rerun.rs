pub(super) const RERUN_HELP: &str = r#"Re-evaluate static evidence affected by one edited Rust test.

Usage: ripr rerun --changed-test PATH [--root PATH] [--json] [--out PATH]

Options:
  --changed-test PATH  Edited Rust integration or unit-test file. Required.
  --root PATH          Workspace root. Defaults to current directory.
  --json               Emit the structured current-state report.
  --out PATH           Write the report to a file instead of stdout.

This first targeted-rerun slice recomputes only seams owned by uniquely resolved
functions directly called from the selected test file, while reusing valid per-file facts.
Without a --before snapshot it reports current_state_only and never infers
improved, closed, unchanged, or regressed movement. Gap-ledger selection and
before/after receipts remain follow-up work under RIPR-SPEC-0123.
"#;
