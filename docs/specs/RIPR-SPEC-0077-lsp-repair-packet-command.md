# RIPR-SPEC-0077: LSP Repair Packet Command

Status: proposed

Owner: product / swarm

Created: 2026-06-11

Linked proposal:

- None yet

Linked ADRs:

- None yet

Linked plan:

- None yet

Linked issues:

- None yet

Linked PRs:

- None yet

Support-tier impact:

- None. This spec adds `ripr.collectRepairPacket` as a new LSP
  `executeCommand`. No existing contract is modified. No language,
  surface, or evidence class is promoted to a stronger support tier.

Policy impact:

- None. The command is advisory-only. It reads existing artifacts
  (`actionable-gaps.json`, `gap-decision-ledger.json`) and returns a
  structured packet. No new files are written.

## Problem

Coding agents reading repair instructions from raw reports face two
problems: the reports are large (full repo exposure or gap ledger) and
the agent must reconstruct the bounded cage itself (edit-surface, verify
command, receipt command, must-not-change) from scattered fields. An
agent without a bounded cage may edit the wrong files or miss the
verification step.

## Behavior

`ripr.collectRepairPacket` is a new LSP `executeCommand` that:

1. Resolves the target gap from an optional `gap_id` argument (first
   argument is an object with an optional `gap_id` string field). If
   `gap_id` is absent or empty, the top actionable gap is used.
2. Reads `target/ripr/reports/actionable-gaps.json` (preferred) or
   falls back to `target/ripr/reports/gap-decision-ledger.json`.
3. Validates that the packet is complete: `canonical_gap_id`,
   `repair_kind`, `verify_command`, `receipt_command`,
   `allowed_edit_surface` (non-empty), `must_not_change` (non-empty),
   `raw_evidence_refs` (non-empty) all present.
4. Resolves `source_location` from `primary_anchor.file` +
   `primary_anchor.line` — never fabricated. If the line is absent or
   zero, emits `{ "status": "source_location_unresolved" }`.
5. Emits the complete packet on success, or a
   `{ "status": "not_actionable_or_incomplete", "reason": "..." }`
   sentinel when the gap is not actionable or the packet is incomplete.
   Never emits a partial packet.

## Non-Goals

- Does not run mutants, execute tests, or modify any file.
- Does not replace `ripr.collectContext` or the gap-decision-ledger
  packet commands.
- Does not expose limitation or advisory metadata beyond the
  `limits_note` field.

## Required Evidence

- `actionable-gaps.json` or `gap-decision-ledger.json` in
  `target/ripr/reports/` (written by `ripr check`).

## Inputs

- Optional `gap_id` in the first argument object.
- `target/ripr/reports/actionable-gaps.json` (preferred).
- `target/ripr/reports/gap-decision-ledger.json` (fallback).

## Outputs

Complete packet (when actionable and complete):

```json
{
  "schema_version": "0.1",
  "tool": "ripr",
  "kind": "repair_packet",
  "canonical_gap_id": "gap:rust:pricing-boundary",
  "language": "rust",
  "repair_kind": "add_boundary_assertion",
  "source_location": { "file": "src/pricing.rs", "line": 42 },
  "allowed_edit_surface": ["tests/pricing.rs"],
  "verify_command": "ripr agent verify --root . --json",
  "receipt_command": "ripr agent receipt --root . --json",
  "must_not_change": ["Do not infer actionability from raw static class."],
  "raw_evidence_refs": [...],
  "confidence": "static_only",
  "limits_note": "Static evidence only; advisory, not a gate decision."
}
```

Not-actionable / incomplete sentinel:

```json
{
  "schema_version": "0.1",
  "tool": "ripr",
  "kind": "repair_packet",
  "status": "not_actionable_or_incomplete",
  "reason": "<from the completeness gate>"
}
```

## Acceptance Examples

1. No artifact on disk → null (no sentinel, no packet).
2. Actionable-gaps.json with missing `receipt_command` → sentinel with
   `status: "not_actionable_or_incomplete"`.
3. Well-formed actionable-gaps.json → full packet with real integer
   `line`, non-empty `allowed_edit_surface`, non-empty
   `must_not_change`, non-empty `raw_evidence_refs`.
4. Capabilities list contains exactly 5 commands including
   `ripr.collectRepairPacket`.

## Test Mapping

- `crates/ripr/src/lsp/tests.rs::execute_command_collect_repair_packet_no_snapshot_and_no_file_returns_null`
- `crates/ripr/src/lsp/tests.rs::execute_command_collect_repair_packet_incomplete_gap_returns_sentinel`
- `crates/ripr/src/lsp/tests.rs::execute_command_collect_repair_packet_complete_gap_returns_full_packet`
- `crates/ripr/src/lsp/tests.rs::execute_command_collect_repair_packet_registered_in_capabilities`

## Implementation Mapping

- `crates/ripr/src/lsp.rs` — `COLLECT_REPAIR_PACKET_COMMAND` constant.
- `crates/ripr/src/lsp/capabilities.rs` — command registered.
- `crates/ripr/src/lsp/backend.rs` — `collect_repair_packet`,
  `collect_repair_packet_from_actionable_gaps`,
  `validate_and_render_actionable_gap_packet`,
  `collect_repair_packet_from_ledger`, `repair_packet_sentinel`.
- `crates/ripr/src/output/agent_seam_packets.rs` —
  `validate_agent_gap_record_packet` made `pub(crate)`;
  `allowed_edit_surface_for_gap_route` and
  `gap_record_packet_do_not_do` made `pub(crate)`.

## CI Proof

- `cargo build -p ripr` clean.
- `cargo test -p ripr lsp` (incl. new tests + 5-command capabilities).

## Metrics

- Gate: all four acceptance tests pass.
- Promote to accepted when the command ships in a tagged release and the
  VS Code extension exercises it end-to-end.

## Failure Modes

- No artifact on disk → null (safe fallback, not an error).
- Incomplete packet → sentinel, never a partial packet.
- `source_location` line is zero or absent → `source_location_unresolved`, never fabricated.
