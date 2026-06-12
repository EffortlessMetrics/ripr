# RIPR-SPEC-0079: Canonical Receipt Command Contract

Status: accepted

Owner: product-swarm

Created: 2026-06-11

Linked proposal:

- None yet

Linked ADRs:

- None yet

Linked plan:

- None yet

Linked issues:

- #1123 — Receipt → outcome → route-quality loop: canonical receipt_command contract

Linked PRs:

- None yet

Support-tier impact:

- None. This spec defines the canonical receipt command syntax and
  alias rules. No existing contract is modified beyond naming which
  command is canonical in the `receipt_command` field. No language,
  surface, or evidence class is promoted to a stronger support tier.
- Claim boundaries remain governed by the canonical ledger in
  [support tiers](../status/SUPPORT_TIERS.md); nothing here promotes a tier.

Policy impact:

- Register this spec in `policy/doc-artifacts.toml`.
- No new crates, binaries, dependencies, parsers, runtime executors,
  or LSP servers introduced by this spec. The `ripr receipt write /
  check` front-end (a later PR) will add CLI subcommands to the
  existing binary under the existing workspace shape.

## Problem

A repair packet is only safe to delegate when its `receipt_command`
field names a command that actually writes a receipt with provenance.
Today two different commands are emitted into the same
`receipt_command` slot:

- `ripr agent receipt --root .. --verify-json .. --seam-id .. --json`
  — the real receipt writer: snapshots before/after state, records
  sha256 artifact provenance, and writes a structured receipt file.
  Built by `agent_receipt_command()` (`crates/ripr/src/agent/loop_commands.rs:106`).
  This is the dominant form (~28 files) and is the true receipt
  command.

- `ripr outcome --before .. --after ..` — computes before/after
  **evidence movement**, not a receipt. Built by `outcome_command()`
  (`crates/ripr/src/agent/loop_commands.rs:155`). This command is
  mis-placed in the `receipt_command` field of
  `crates/ripr/src/output/agent_seam_packets.rs` (9 occurrences),
  `crates/ripr/src/output/first_pr.rs:3125`,
  `crates/ripr/src/lsp/backend.rs:1502`,
  `crates/ripr/src/cli/commands/agent_gap_packet.rs:52`, and
  `xtask/src/main.rs:77814,79391`.

Without a written contract that names exactly one canonical form,
emitter-alignment PR 2 has no authoritative target, consumers (LSP,
PR summary, review card, gap decision ledger) cannot enforce a
consistent field value, and agents receive contradictory instructions
depending on which surface generated the packet.

This spec writes that contract.

## Behavior

### Canonical receipt command

The canonical receipt command is:

```
ripr receipt write --gap <canonical_gap_id> --packet <packet_id> \
  --verify-command "<cmd>" --status <verify_status> [--out <path>]
```

| Argument | Required | Description |
| --- | --- | --- |
| `--gap <canonical_gap_id>` | required | Identifies the gap this receipt closes. Must be a `canonical_gap_id` as defined below. |
| `--packet <packet_id>` | required | Identifies the repair packet the agent acted on. |
| `--verify-command "<cmd>"` | required | The exact shell command that was run to verify the repair (e.g. `cargo test -p ripr`). Quoted. |
| `--status <verify_status>` | required | The outcome of the verify command. See valid values below. |
| `--out <path>` | optional | Write the receipt JSON to this path. When omitted, writes to `target/ripr/receipts/<canonical_gap_id>.json`. |

The `ripr receipt check` command reads a receipt file and reports
whether it is structurally valid and not stale:

```
ripr receipt check [--gap <canonical_gap_id>] [--path <receipt_path>]
```

When `--gap` is provided without `--path`, `ripr receipt check`
resolves the path from the canonical location
`target/ripr/receipts/<canonical_gap_id>.json`.

### `canonical_receipt_command` field rule

Every surface that emits a `receipt_command` field MUST emit the
canonical `ripr receipt write ...` form. The field MUST NOT contain
`ripr outcome ...` or any other movement / outcome command. This
applies equally to:

- agent seam packets (`crates/ripr/src/output/agent_seam_packets.rs`)
- first-PR packets (`crates/ripr/src/output/first_pr.rs`)
- LSP repair packets (`crates/ripr/src/lsp/backend.rs`)
- agent gap-packet command (`crates/ripr/src/cli/commands/agent_gap_packet.rs`)
- xtask report surfaces (`xtask/src/main.rs`)
- VS Code extension copyable fields
- PR summary and review-card renderers

All surfaces must emit the same command string for the same gap+packet
pair so that an agent or human can copy-paste from any surface and
obtain the same result.

### Valid `verify_status` values

| Value | Meaning |
| --- | --- |
| `passed` | The verify command exited 0 and all checks passed. |
| `failed` | The verify command exited non-zero or a check failed. |
| `not_run` | The verify command was not run (e.g., the agent skipped it). |
| `unknown` | Verify status cannot be determined from available information. |

These values align with the closed outcome vocabulary in
RIPR-SPEC-0073. A `verify_status` of `not_run` or `unknown` caps the
attempt outcome at `receipt_present` / `attempted_no_receipt`; it
never reaches `evidence_improved` or `resolved`.

### `canonical_gap_id` and `packet_id` requirements

A receipt MUST bind to a `canonical_gap_id`. The `canonical_gap_id` is
the stable identifier for the gap as emitted by `ripr` in the gap
decision ledger and repair packets (format:
`<crate>_<module>_<gap_kind>_<fp8>`). It MUST NOT be a
line-keyed or session-local identifier.

A receipt SHOULD carry a `packet_id`. The `packet_id` is the
identifier of the specific repair packet the agent acted on. When a
`packet_id` is unavailable (e.g., the agent operated outside the
normal LSP/CLI handoff), `ripr receipt write` accepts the gap alone
and records `packet_id: null` in the output JSON, with
`packet_id_available: false`. Consumers MUST treat a receipt with
`packet_id: null` as valid but unlinked; they MUST NOT reject it.

When neither `canonical_gap_id` nor `packet_id` context is available,
`ripr receipt write` returns an explicit error (see Fail-Closed
Behavior). A receipt without a bound gap has no evidential value and
MUST NOT be written.

### Legacy alias behavior

`ripr agent receipt --root <root> --verify-json <path> --seam-id <id> --json`
is accepted as a legacy alias for `ripr receipt write`. Agents using
`ripr agent receipt` directly continue to work during the transition.
New emitters MUST use `ripr receipt write`. The `ripr agent receipt`
alias is documented but not promoted; it will be deprecated in a
future release once all emitters have migrated (tracked in issue
#1123).

`ripr agent receipt` is the current real writer
(`agent_receipt_command()` at `crates/ripr/src/agent/loop_commands.rs:106`).
`ripr receipt write` will be implemented as a thin front over this
existing machinery in the implementation PR (PR 3 of issue #1123).

### `ripr outcome` is not a receipt command

`ripr outcome --before .. --after ..` computes before/after evidence
movement (`outcome_command()` at
`crates/ripr/src/agent/loop_commands.rs:155`). It MUST NOT appear in
the `receipt_command` field of any packet, report, or surface. It
belongs in a separate `outcome_command` field when that movement data
is needed. The current mis-placements listed in the Problem section are
defects that PR 2 of issue #1123 corrects.

### Fail-closed behavior

| Condition | Required behavior |
| --- | --- |
| `--gap` not provided | Explicit error: "receipt requires a canonical_gap_id; re-run with --gap". No receipt written. |
| `canonical_gap_id` does not match any known gap in the ledger | Explicit error: "canonical_gap_id not found; verify the gap id with `ripr agent status`". No receipt written. |
| `--verify-command` absent | Explicit error: "receipt requires --verify-command". No receipt written. |
| `--status` absent | Explicit error: "receipt requires --status (passed|failed|not_run|unknown)". No receipt written. |
| `--status` is not a valid value | Explicit error listing valid values. No receipt written. |
| Malformed receipt on `ripr receipt check` | Explicit error: "receipt at <path> is malformed: <reason>". Exit non-zero. |
| Receipt references unknown `canonical_gap_id` on check | Explicit error: "receipt gap_id not found in current ledger (stale or orphan)". Exit non-zero. |

All errors exit non-zero. No silent failure. No false success. The
receipt command is fail-closed: if it cannot write a valid receipt, it
writes nothing and reports the problem.

### Consistency requirement

Every surface that emits `receipt_command` MUST emit the canonical
`ripr receipt write ...` form (see `canonical_receipt_command` field
rule above). A receipt command that varies between the LSP repair
packet, the PR summary, the review card, and the agent seam packet for
the same gap+packet pair is a defect. PR 2 of issue #1123 enforces
this alignment across all emitters and updates golden fixtures.

### Sequencing note

`ripr receipt write` does not exist yet; only `ripr agent receipt`
exists. Because emitters cannot emit a command that does not exist, PR
3 (implement `ripr receipt write / check` as a thin front over
`agent_receipt_command()`) MUST land before or together with PR 2
(align all emitters). PR 1 (this spec) is a pure docs/contract PR with
no code changes.

## Non-Goals

- Does not prove semantic correctness beyond the recorded verify
  command. A receipt records what was run; it does not certify
  the repair is complete or correct.
- Does not use mutation-runtime vocabulary. The static-language
  constraint applies: the words `killed`, `survived`, `untested`,
  `proven`, and `adequate` are forbidden in receipt output.
- Does not prove evidence improvement from limited or stale input.
  When evidence is limited or stale, the outcome resolves to
  `unknown` with `reason: limited_input`, not `evidence_improved`.
- Limitations are never repair receipts. A packet that reports a
  static-analysis limitation MUST NOT have a `receipt_command`
  pointing at `ripr receipt write`; the packet is not actionable and
  the `receipt_command` field MUST be absent or null.
- Does not introduce runtime mutation execution, test runner
  invocation, or any non-static analysis step. `ripr` remains a static
  RIPR exposure analyzer.
- Does not define the route-quality report, outcome classifier, or
  attempt ledger hardening — those are PR 5–7 of issue #1123.

## Required Evidence

- A gap repair packet with `canonical_gap_id` present.
- A verify command that was run (or an explicit record that it was
  not run, with `verify_status: not_run`).

## Inputs

`ripr receipt write`:

- `--gap <canonical_gap_id>` (required)
- `--packet <packet_id>` (required; null recorded if genuinely unavailable)
- `--verify-command "<cmd>"` (required)
- `--status <verify_status>` (required)
- `--out <path>` (optional)

`ripr receipt check`:

- `--gap <canonical_gap_id>` (optional if `--path` provided)
- `--path <receipt_path>` (optional if `--gap` provided)

## Outputs

Receipt JSON written to `--out` or `target/ripr/receipts/<canonical_gap_id>.json`:

```json
{
  "schema_version": "0.1",
  "tool": "ripr",
  "kind": "receipt",
  "canonical_gap_id": "crates_ripr_src_lib.rs:error_path:c1a03250",
  "packet_id": "packet-abc123",
  "packet_id_available": true,
  "verify_command": "cargo test -p ripr",
  "verify_status": "passed",
  "written_at": "2026-06-11T00:00:00Z",
  "limits_note": "Static evidence only. Receipt records what was run; does not certify semantic correctness."
}
```

`ripr receipt check` exits 0 on a valid, non-stale receipt and exits
non-zero on any error, printing a human-readable message to stderr.

## Acceptance Examples

1. `ripr receipt write --gap crates_ripr_src_lib.rs:error_path:c1a03250 --packet packet-abc123 --verify-command "cargo test -p ripr" --status passed` writes a valid receipt JSON to the canonical path and exits 0.
2. `ripr receipt write` with no `--gap` argument exits non-zero with an explicit error message containing "canonical_gap_id".
3. `ripr receipt write ... --status invalid_status` exits non-zero with an explicit error listing valid `verify_status` values.
4. `ripr receipt check --gap crates_ripr_src_lib.rs:error_path:c1a03250` exits 0 when a valid receipt is present.
5. `ripr receipt check --path target/ripr/receipts/nonexistent.json` exits non-zero with an explicit "not found" error.
6. `ripr agent receipt --root . --verify-json target/ripr/workflow/agent-verify.json --seam-id <id> --json` is accepted as the legacy alias and produces the same receipt output.
7. No surface emits `ripr outcome ...` in the `receipt_command` field; any such occurrence is a failing test.

## Test Mapping

Unit tests (app layer) — `crates/ripr/src/app/receipt.rs`:

- `receipt_write_valid_args_writes_json`
- `receipt_write_without_packet_id_records_null`
- `receipt_write_all_valid_statuses_accepted`
- `receipt_write_missing_gap_exits_nonzero`
- `receipt_write_invalid_status_exits_nonzero`
- `receipt_write_missing_verify_command_exits_nonzero`
- `receipt_check_valid_receipt_exits_zero`
- `receipt_check_missing_file_exits_nonzero`
- `receipt_check_malformed_json_exits_nonzero`
- `receipt_check_missing_required_field_exits_nonzero`
- `receipt_check_invalid_status_in_file_exits_nonzero`
- `receipt_check_no_path_no_gap_exits_nonzero`
- `receipt_out_path_uses_explicit_path_when_provided`
- `receipt_out_path_defaults_to_canonical_location`

Integration smoke tests — `crates/ripr/tests/cli_smoke.rs`:

- `receipt_write_then_check_exits_zero`
- `receipt_write_with_packet_id_smoke`
- `receipt_write_missing_gap_exits_nonzero_smoke`
- `receipt_write_invalid_status_exits_nonzero_smoke`
- `receipt_check_missing_file_exits_nonzero_smoke`
- `receipt_help_exits_zero_smoke`
- `agent_receipt_legacy_alias_still_dispatches_smoke`

Emitter-alignment tests (PR 2):

- `crates/ripr/src/output/tests.rs::agent_seam_packet_receipt_command_is_canonical`
- `crates/ripr/src/output/tests.rs::first_pr_packet_receipt_command_is_canonical`
- `crates/ripr/src/lsp/tests.rs::backend_repair_packet_receipt_command_is_canonical`

## Implementation Mapping

PR 3 (this PR):

- `crates/ripr/src/app/receipt.rs` — `write_receipt()`, `check_receipt()`, `receipt_out_path()`, `ReceiptWriteOptions`, `ReceiptCheckOptions`.
- `crates/ripr/src/cli/commands/receipt.rs` — argv adapter only; delegates to `crate::app::receipt`.

Emitter-alignment (PR 2 — already shipped):

- `crates/ripr/src/output/agent_seam_packets.rs` — `canonical_receipt_command()` used in all seam-packet emitters.
- `crates/ripr/src/output/first_pr.rs` — same.
- `crates/ripr/src/lsp/backend.rs` — same.
- `crates/ripr/src/cli/commands/agent_gap_packet.rs` — same.
- `xtask/src/main.rs` — same.

## CI Proof

- `cargo xtask check-spec-format` passes on this file.
- `cargo xtask check-doc-artifacts` passes with the new registration.
- `cargo xtask check-doc-index` passes with the README entry.
- `cargo xtask check-traceability` passes (tests + code arrays populated in PR 3).
- `cargo xtask check-static-language` passes (no forbidden vocabulary).
- `cargo xtask check-architecture` passes.
- `cargo xtask check-public-api` passes (no new public symbols).
- `cargo xtask check-no-panic-family` passes.
- `cargo xtask check-allow-attributes` passes.
- `cargo xtask check-output-contracts` passes.
- `cargo test -p ripr` — 2480 pass, 1 ignored.

## Metrics

- Gate: acceptance tests 1–7 pass (PR 3 ships `ripr receipt write / check`; PR 2 shipped emitter alignment).
- Status promoted to `accepted` in PR 3 once all gates green.

## Failure Modes

- Missing `--gap` → explicit error, exit non-zero (never silent).
- Invalid `--status` → explicit error listing valid values, exit non-zero.
- Unknown `canonical_gap_id` → explicit error, exit non-zero.
- Malformed receipt on check → explicit error with reason, exit non-zero.
- `ripr outcome` in `receipt_command` slot → test failure (PR 2 enforcement).
