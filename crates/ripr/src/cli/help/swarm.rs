pub(super) const SWARM_HELP: &str = r#"Queue bounded repair work for external agent coordination.

Usage: ripr swarm <subcommand>

Subcommands:
  queue  Rank GapRecord-backed agent packets into a bounded repair queue.
  ingest Classify one external agent result without trusting it blindly.

Run `ripr swarm queue --help` or `ripr swarm ingest --help` for the subcommand
surfaces. Swarm commands are
advisory and static; they do not run tests, call providers, generate tests,
edit files, run mutation testing, create receipts, or change gate behavior.
"#;

pub(super) const SWARM_QUEUE_HELP: &str = r#"Queue GapRecord-backed repair packets for safe agent assignment.

Usage: ripr swarm queue [--root PATH] [--gap-ledger PATH] [--language python] [--top N] [--format json]

Options:
  --root PATH        Workspace root. Defaults to current directory.
  --gap-ledger PATH  Gap decision ledger JSON. Defaults to target/ripr/reports/gap-decision-ledger.json.
  --language NAME    Language filter. Defaults to python.
  --top N            Maximum queued packets to return. Defaults to 10.
  --format json      Explicit JSON output. JSON is the only queue format.
  --json             Compatibility alias for JSON output.

The queue command reads existing GapRecord artifacts and includes only records
that are already eligible for `ripr agent packet --gap-ledger ... --gap-id ...`.
It groups packets by `allowed_edit_surface` conflict group so schedulers can
avoid parallel edits to the same test file. Staleness is reported as
`not_evaluated` until a later receipt/ledger step compares the queue with the
current git state.
"#;

pub(super) const SWARM_INGEST_HELP: &str = r#"Classify one external agent result for safe repair-loop ingestion.

Usage: ripr swarm ingest [--root PATH] --result PATH [--format json]

Options:
  --root PATH    Workspace root. Defaults to current directory.
  --result PATH  Agent result JSON to classify. Must stay under --root.
  --format json  Explicit JSON output. JSON is the only ingest format.
  --json         Compatibility alias for JSON output.

The ingest command reads an existing result artifact and classifies it as
closed, partially_improved, verify_failed, edited_forbidden_file,
stopped_by_agent, stale_packet, or uncertain. Missing verify evidence is never
treated as success, and edits to packet forbidden_files are flagged before any
reported verify or receipt success. JSON also emits attempt_outcome using the
repair-loop vocabulary: attempted_no_receipt, receipt_present,
evidence_improved, evidence_unchanged, evidence_regressed, resolved, or
unknown.
"#;
