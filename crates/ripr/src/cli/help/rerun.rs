pub(super) const RERUN_HELP: &str = r#"Re-evaluate static evidence affected by one edited Rust test.

Usage: ripr rerun --changed-test PATH [--root PATH] [--json] [--out PATH]
       ripr rerun --gap CANONICAL_GAP_ID --gap-ledger PATH [--root PATH] [--json] [--out PATH]

Options:
  --changed-test PATH  Edited Rust integration or unit-test file. Required.
  --gap ID             Canonical gap identity from the explicit gap ledger.
  --gap-ledger PATH    Gap decision ledger required by --gap.
  --root PATH          Workspace root. Defaults to current directory.
  --json               Emit the structured current-state report.
  --out PATH           Write the report to a file instead of stdout.

The changed-test selector recomputes only seams owned by uniquely resolved
functions directly called from the selected test file. The gap selector groups
every supplied ledger record with that canonical identity, scopes recomputation
to each unique anchored file/owner, and returns only current seams with that
same domain ID. Missing, stale, anchorless, or root-mismatched ledger selections
become named limitations; a stale record does not hide other current scopes.
RIPR never scans unrelated seams as a fallback.

Both selectors reuse valid per-file facts.
Without a --before snapshot it reports current_state_only and never infers
improved, closed, unchanged, or regressed movement. Before/after receipts remain
follow-up work under RIPR-SPEC-0123.
"#;
