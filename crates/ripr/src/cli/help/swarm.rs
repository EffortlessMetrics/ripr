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
  --top N            Maximum assignable frontier candidates to return. Defaults to 10.
  --format json      Explicit JSON output. JSON is the only queue format.
  --json             Compatibility alias for JSON output.

The queue command reads existing GapRecord artifacts and includes only records
that are already eligible for `ripr agent packet --gap-ledger ... --gap-id ...`.
It groups packets by `allowed_edit_surface` conflict group so schedulers can
avoid parallel edits to the same test file. A packet is assignable only when
producer-owned currentness validation yields `queue_state = queued` and
`staleness_status = current`, and `--top` selects from that live-current
assignable frontier after filtering, so a blocked record never consumes a
bounded packet slot. The summary keeps the honest denominators: total
validated candidates, assignable candidates, returned candidates, unreturned
assignable candidates, stale candidates, and unevaluated candidates. That
validation binds the ledger to its declared
repo-exposure source, selected canonical root, exact clean HEAD, snapshot and
content identities, and freshly reproduced GapRecords. Candidates that reach
this live-currentness validation but are not assignable — legacy, unsupported,
dirty, historical, mismatched, or receipt-stale evidence — remain visible in a
bounded, command-free `blocked_review` projection with their typed state,
reason, conflict group, and exact refresh route. A malformed ledger, a missing
root provenance, or a root mismatch follows the error and blocker-envelope
paths instead and does not carry the projection. The
`source_currentness = candidate_current` record label alone is not freshness
proof. If the gap ledger omits root provenance or declares a root that differs
from `--root`, the command emits the existing blocked root-provenance envelope.
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
