pub(super) const RERUN_HELP: &str = r#"Re-evaluate static evidence affected by one edited Rust test.

Usage: ripr rerun --changed-test PATH [--before PATH] [--root PATH] [--json] [--out PATH]
       ripr rerun --gap CANONICAL_GAP_ID --gap-ledger PATH [--before PATH] [--root PATH] [--json] [--out PATH]

Options:
  --changed-test PATH  Edited Rust integration or unit-test file. Required.
  --gap ID             Canonical gap identity from the explicit gap ledger.
  --gap-ledger PATH    Gap decision ledger required by --gap.
  --before PATH        Explicit prior targeted-rerun, repo-exposure, or compatible static snapshot.
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

Both selectors reuse valid per-file facts. Without --before the receipt reports
current_state_only and never infers movement. With an explicit compatible
before artifact it reports closed, improved, unchanged, or regressed static
movement for every selected current seam; a missing or incompatible before
artifact is a named limited result. RIPR never discovers an ambient report as
before state.
"#;
