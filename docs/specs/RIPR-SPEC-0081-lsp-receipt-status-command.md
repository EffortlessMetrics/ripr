# RIPR-SPEC-0081: LSP Receipt-Status Command

Status: proposed

Owner: product / swarm

Created: 2026-06-12

Linked proposal:

- None yet

Linked ADRs:

- None yet

Linked plan:

- None yet

Linked issues:

- #1133 — Extend LSP cockpit to expose receipt status, latest outcome, and copy receipt command
- #1123 — Receipt -> outcome -> route-quality loop (umbrella)

Linked PRs:

- None yet

Support-tier impact:

- None. This spec adds `ripr.collectReceiptStatus` as a new LSP
  `executeCommand` and augments `ripr.collectWorkspaceStatus` with a
  compact `receipt_status_summary` block. No existing contract is
  modified. No language, surface, or evidence class is promoted to a
  stronger support tier.
- Claim boundaries remain governed by the canonical ledger in
  [support tiers](../status/SUPPORT_TIERS.md); nothing here promotes a tier.

Policy impact:

- Register this spec in `policy/doc-artifacts.toml`.
- No new crates, binaries, dependencies, parsers, runtime executors, or
  LSP servers introduced by this spec.

## Problem

The LSP cockpit already surfaces workspace diagnostics, repair packets,
and top limitations, but has no way for a cockpit consumer to ask: "what
is the current receipt lifecycle state for the top actionable gap, and
has a repair attempt been made?"

An agent or operator using the cockpit must either parse the full
gap-decision-ledger or read the swarm-attempt-ledger directly. Both are
multi-kilobyte artifacts with more information than needed for a
status-at-a-glance query.

`ripr.collectReceiptStatus` answers these questions with a compact,
honest packet that reads the three real artifact sources and returns
structured fields with explicit `not_available` sentinels when an
artifact is absent.

## Behavior

`ripr.collectReceiptStatus` is a new LSP `executeCommand` that:

1. Locks `latest_analysis` briefly.
2. If no snapshot exists, returns the `no_snapshot` sentinel (all fields
   `not_available`).
3. Otherwise reads the three real artifacts (gap-decision-ledger,
   swarm-attempt-ledger, route-quality.json) and builds the packet.
4. Returns the packet as structured JSON.

`ripr.collectWorkspaceStatus` is augmented to include a compact
`receipt_status_summary` block (fields: `receipt_movement`,
`latest_attempt_outcome`, `route_quality_summary`) derived by the same
artifact readers, so a cockpit single-status call shows receipt state
without a second round-trip.

### Per-field real artifact producers

| Field | Real producer / artifact | not_available condition |
| --- | --- | --- |
| `receipt_status` | `GapRecord.receipt.movement` from `target/ripr/reports/gap-decision-ledger.json` (`parse_gap_records_json`) | ledger absent, unreadable, or no matching record |
| `missing_receipt_reason` | `GapRecord.gap_state == "actionable"` + `receipt_command.is_some()` + movement absent | receipt present, or gap not actionable-repairable |
| `copy_receipt_command` | `ValidatedGapArtifact.receipt_commands` first entry (via `command_payload_is_safe`) | no snapshot, limitation present, packet incomplete (missing verify_command or receipt_command) |
| `open_attempt_ledger` | `target/ripr/reports/swarm-attempt-ledger.json` file-exists check | file absent |
| `latest_attempt_outcome` | `latest_attempts[].outcome` from `target/ripr/reports/swarm-attempt-ledger.json` | file absent OR unreadable (`absence != no outcome`) |
| `route_quality_summary` | `repair_route_quality_latest` rows from `target/ripr/reports/route-quality.json` (RIPR-SPEC-0080) | file absent, unreadable, or `status == "blocked"` |

### `not_available` rule (HONESTY BAR)

Every field MUST come from reading a REAL artifact. If the artifact is
absent or unreadable, the field value is the string `"not_available"`.

`"not_available"` means "not derivable from any real artifact at this
moment", distinct from a real empty result or zero. Agents and
operators must treat it as "data not present", not as "confirmed zero"
or "confirmed no attempts".

### RIPR-SPEC-0076 harmonization (no implied actionability)

- A snapshot with `gap_artifact_rejections` (a "limitation surface")
  MUST NOT emit a `copy_receipt_command` — limitations are not repair
  packets and must never surface a receipt command.
- An incomplete packet (missing `verify_command` OR `receipt_command` in
  the validated artifact) MUST NOT emit `copy_receipt_command`.
- This matches the RIPR-SPEC-0076 diagnostics policy: limitations must
  not imply actionability.

### `route_quality_summary` compact format

When `route-quality.json` is present and `status != "blocked"`, the
summary is a JSON object with:

```json
{
  "report": "route-quality",
  "status": "<advisory|...>",
  "top_repair_kind_rows": [
    { "repair_kind": "...", "attempted": N, "success_rate": N_or_null }
  ]
}
```

Up to 3 rows from `repair_route_quality_latest`. When the file is
absent or `status == "blocked"`, the field value is `"not_available"`.

## Non-Goals

- Does not run mutants, execute tests, or modify any file.
- Does not replace `ripr.collectRepairPacket` or `ripr.collectWorkspaceStatus`.
- Does not wire the VS Code TypeScript extension — that is the next PR (#1134).
- Does not surface `stale_receipt_count`, `orphan_receipt_count`, or
  `gap_mismatch_receipt_count` — no real producers exist for these yet.
- Does not compute `receipts_present` or `missing_receipts` count aggregates
  from the ledger summary — that is deferred per #1133 acceptance criteria.
  Count aggregates will land in a follow-up that connects to
  `GapDecisionLedgerSummary.receipt_improved_total` and
  `receipt_unchanged_after_attempt_total`.

## Required Evidence

- `target/ripr/reports/gap-decision-ledger.json` — for `receipt_status`
  and `missing_receipt_reason`.
- `target/ripr/reports/swarm-attempt-ledger.json` — for
  `latest_attempt_outcome` and `open_attempt_ledger`.
- `target/ripr/reports/route-quality.json` (RIPR-SPEC-0080) — for
  `route_quality_summary`.

All three artifacts are optional. Absence → `not_available`.

## Inputs

- No arguments. The command reads the in-memory snapshot and artifact
  files relative to `snapshot.root`.

## Outputs

Full receipt-status packet (when snapshot is present):

```json
{
  "schema_version": "0.1",
  "tool": "ripr",
  "kind": "receipt_status",
  "receipt_status": "improved",
  "missing_receipt_reason": "not_available",
  "copy_receipt_command": "ripr receipt write --gap gap:rust:pricing:threshold-boundary",
  "open_attempt_ledger": "target/ripr/reports/swarm-attempt-ledger.json",
  "latest_attempt_outcome": "evidence_improved",
  "route_quality_summary": {
    "report": "route-quality",
    "status": "advisory",
    "top_repair_kind_rows": [
      { "repair_kind": "AddBoundaryAssertion", "attempted": 2, "success_rate": 0.5 }
    ]
  },
  "report_paths": {
    "gap_decision_ledger": "target/ripr/reports/gap-decision-ledger.json",
    "swarm_attempt_ledger": "target/ripr/reports/swarm-attempt-ledger.json",
    "route_quality": "target/ripr/reports/route-quality.json"
  },
  "limits_note": "Static evidence only; advisory, not a gate decision."
}
```

Absent-artifacts sentinel (artifacts not on disk):

```json
{
  "schema_version": "0.1",
  "tool": "ripr",
  "kind": "receipt_status",
  "receipt_status": "not_available",
  "missing_receipt_reason": "not_available",
  "copy_receipt_command": "not_available",
  "open_attempt_ledger": "not_available",
  "latest_attempt_outcome": "not_available",
  "route_quality_summary": "not_available",
  "report_paths": { ... },
  "limits_note": "Static evidence only; advisory, not a gate decision."
}
```

No-snapshot sentinel (no analysis run yet):

```json
{
  "schema_version": "0.1",
  "tool": "ripr",
  "kind": "receipt_status",
  "status": "no_snapshot",
  "receipt_status": "not_available",
  "missing_receipt_reason": "not_available",
  "copy_receipt_command": "not_available",
  "open_attempt_ledger": "not_available",
  "latest_attempt_outcome": "not_available",
  "route_quality_summary": "not_available",
  "limits_note": "Static evidence only; advisory, not a gate decision."
}
```

## Acceptance Examples

1. No snapshot → packet with `status == "no_snapshot"` and all artifact
   fields `"not_available"`.
2. Snapshot present, NO attempt-ledger or route-quality on disk →
   `latest_attempt_outcome == "not_available"` and
   `route_quality_summary == "not_available"` (absence != no outcome).
3. Snapshot present WITH attempt-ledger containing `outcome = "evidence_improved"` →
   `latest_attempt_outcome == "evidence_improved"`.
4. Snapshot present WITH route-quality.json having `status = "advisory"` and
   rows → `route_quality_summary` is a structured object, not the sentinel.
5. route-quality.json with `status = "blocked"` →
   `route_quality_summary == "not_available"`.
6. Snapshot with `gap_artifact_rejections` (limitation) →
   `copy_receipt_command == "not_available"` (RIPR-SPEC-0076 harmonization).
7. `collectWorkspaceStatus` result includes `receipt_status_summary` field with
   `latest_attempt_outcome == "not_available"` when artifacts are absent.

## Test Mapping

- `crates/ripr/src/lsp/tests.rs::execute_command_collect_receipt_status_no_snapshot_returns_not_available_fields`
- `crates/ripr/src/lsp/tests.rs::execute_command_collect_receipt_status_absent_artifacts_yield_not_available`
- `crates/ripr/src/lsp/tests.rs::execute_command_collect_receipt_status_with_attempt_ledger_returns_real_outcome`
- `crates/ripr/src/lsp/tests.rs::execute_command_collect_receipt_status_with_route_quality_returns_summary`
- `crates/ripr/src/lsp/tests.rs::execute_command_collect_receipt_status_blocked_route_quality_yields_not_available`
- `crates/ripr/src/lsp/tests.rs::execute_command_collect_receipt_status_limitation_hides_receipt_command`
- `crates/ripr/src/lsp/tests.rs::execute_command_collect_receipt_status_registered_in_capabilities`
- `crates/ripr/src/lsp/tests.rs::execute_command_collect_workspace_status_includes_receipt_status_summary_field`

## Implementation Mapping

- `crates/ripr/src/lsp.rs` — `COLLECT_RECEIPT_STATUS_COMMAND` constant.
- `crates/ripr/src/lsp/capabilities.rs` — command registered (7 total).
- `crates/ripr/src/lsp/backend.rs` — `collect_receipt_status`,
  `collect_receipt_status_fields`, `receipt_status_from_ledger`,
  `read_latest_attempt_outcome`, `read_route_quality_summary`,
  `workspace_receipt_status_report_paths`,
  `workspace_status_receipt_summary` (augments `collect_workspace_status`).

## CI Proof

- `RUSTFLAGS="-D warnings" cargo build -p ripr -p xtask` — exit 0 each.
- `cargo test -p ripr lsp` — 259 pass (8 new receipt-status tests).
- `cargo clippy -p ripr -p xtask --all-targets -- -D warnings` clean.
- `cargo fmt --check` clean.
- `cargo xtask check-static-language` pass.
- `cargo xtask check-architecture` pass.
- `cargo xtask check-no-panic-family` pass.
- `cargo xtask check-public-api` pass.
- `cargo xtask check-doc-artifacts` pass.
- `cargo xtask check-doc-index` pass.
- `cargo xtask check-spec-format` pass.
- `cargo xtask check-traceability` pass.
- `cargo xtask check-support-tiers` pass.
- `cargo xtask check-output-contracts` pass.

## Metrics

- Gate: all eight acceptance tests pass.
- Promote to accepted when the command ships in a tagged release and an
  agent exercises it end-to-end with real attempt-ledger and route-quality
  artifacts.

## Failure Modes

- No snapshot → `no_snapshot` sentinel with all fields `not_available`.
- Artifact absent/unreadable → field value `"not_available"` (graceful
  degradation; never crashes or fabricates).
- Limitation present → `copy_receipt_command == "not_available"` (safe
  fallback; no receipt command for limitation surfaces).
- Incomplete packet (missing verify or receipt command) →
  `copy_receipt_command == "not_available"`.
- Lock failure → null (graceful degradation; never panics).
