# RIPR-SPEC-0110: Receipt-Gap Cross-Reference

Status: accepted

Owner: product / swarm

Created: 2026-06-14

Linked issues:

- #1123 (receipt↔outcome→route-quality loop; this closes the explicitly-deferred "PR 4")

Linked PRs:

- None yet

Support-tier impact:

- CLI-only new flag on `ripr receipt check`; no schema version bump; no new
  output contract enum value in the existing output_contracts.txt schema (the
  cross-reference labels `not_available`, `receipt_ok`, `orphan_receipt`,
  `receipt_gap_mismatch` are printed as advisory text on the `check` result
  line, not as typed JSON fields in the check output schema). Tier labels and
  claim boundaries remain governed by the canonical ledger in
  [support tiers](../status/SUPPORT_TIERS.md).

Policy impact:

- Production delta: `check_receipt` in `crates/ripr/src/app/receipt.rs` and
  `parse_receipt_check_options` in `crates/ripr/src/cli/commands/receipt.rs`.
- New type `ReceiptCrossRefResult` in `crates/ripr/src/app/receipt.rs`.
- Reuses `parse_gap_records_json` from
  `crates/ripr/src/output/gap_decision_ledger.rs` (no fork).
- No new crate. No workspace-shape change.

## Problem

### The labels existed but no producer computed them

`ripr receipt check` validated JSON structure only and printed: "structural
check only; orphan/stale detection requires gap cross-reference — see PR 4".

The labels `receipt_stale`, `orphan_receipt`, and `receipt_gap_mismatch`
existed in `output/receipt_lifecycle.rs` but no producer computed them.
The #1130 honesty test
(`ledger_summary_does_not_emit_fabricated_receipt_state_counts`) enforced these
labels stay absent / `not_available` — fabricating a fake-zero count was
forbidden.

The data to compute the cross-reference was already available: the receipt's
`canonical_gap_id` field and the live gap set from `parse_gap_records_json`
in `output/gap_decision_ledger.rs`.

### The residual after PR 3

PR 3 (#1123) shipped structural write + check only. The cross-reference that
closes the receipt→outcome feedback loop was explicitly deferred as "PR 4".
This spec is PR 4.

## Behavior

### `--ledger` flag

`ripr receipt check` gains an OPTIONAL `--ledger <gap-decision-ledger.json>`
flag. When provided, the receipt's `canonical_gap_id` is cross-referenced
against the live gap set and classified:

| Result | Meaning | Exit |
|---|---|---|
| `not_available` | No ledger provided or unreadable. Default fail-closed sentinel. | 0 |
| `receipt_ok` | `canonical_gap_id` found in the live gap set. | 0 |
| `orphan_receipt` | `canonical_gap_id` NOT in the live gap set (gap disappeared / never existed). | non-zero |
| `receipt_gap_mismatch` | `canonical_gap_id` found but dedupe fingerprint differs (gap moved / changed identity). | non-zero |

### Fail-closed honesty rule

When `--ledger` is **ABSENT or UNREADABLE**, the cross-reference result is
**`not_available`** — NEVER `receipt_ok`. Absence of the ledger must NEVER be
interpreted as "the receipt is valid/fresh." The structural check still runs
independently (exit 0 for structural-only when no `--ledger`).

### Implementation

`cross_reference_receipt` in `app/receipt.rs` reads the ledger via
`parse_gap_records_json` (reused, not forked), searches the gap set for the
receipt's `canonical_gap_id`, and returns the `ReceiptCrossRefResult` enum.
The CLI layer in `cli/commands/receipt.rs` exits non-zero when
`cross_ref.is_error()`.

No fingerprint is invented when the data does not carry one — `receipt_ok` is
returned when the gap is found and no fingerprint mismatch exists.

## Controls

Four unit tests in `crates/ripr/src/app/receipt.rs` (RIPR-SPEC-0110 block)
plus one CLI smoke test in `crates/ripr/tests/cli_smoke.rs`:

1. `receipt_check_orphan_when_gap_absent_from_ledger`: receipt's
   `canonical_gap_id` NOT in the ledger's live set → `orphan_receipt`.
2. `receipt_check_cross_reference_not_available_when_ledger_absent`: no
   `--ledger` → result `not_available` (NEVER `receipt_ok`); structural check
   still exits 0. This is the fail-closed core.
3. `receipt_check_ok_when_gap_present`: receipt's `canonical_gap_id` IS in the
   ledger → `receipt_ok`, exit 0.
4. `receipt_check_orphan_exits_nonzero` (CLI smoke): shell out to binary,
   confirm non-zero exit and `orphan_receipt` in output.

Plus the pre-existing #1130 honesty test
`ledger_summary_does_not_emit_fabricated_receipt_state_counts` must still pass
unchanged.

## Non-Goals

- No fingerprint is invented when the gap record does not carry one.
- `receipt_stale` as a distinct label is not emitted (the data to distinguish
  it from `receipt_gap_mismatch` does not exist in the current ledger format).
- Ledger-absent → `not_available` is a permanent, correct behavior, not a
  placeholder.
- `ripr receipt check` never requires `--ledger`; structural-only mode
  continues to work and exit 0 on valid receipts.
- This spec completes the deferral in RIPR-SPEC-0079 ("PR 4 scope") and
  provides the real producer that RIPR-SPEC-0073 planned for in its outcome
  set.
- No version bump / publish / release.

## Acceptance

1. All 4 controls pass (unit + smoke).
2. `cargo test -p ripr ledger_summary_does_not_emit_fabricated_receipt_state_counts`
   passes (#1130 honesty test unchanged).
3. `RUSTFLAGS="-D warnings" cargo build -p ripr -p xtask` passes.
4. `cargo clippy -p ripr --all-targets -- -D warnings` reports no issues.
5. `cargo fmt --check` passes under rustfmt 1.9.0-stable.
6. All xtask policy gates pass (incl. check-evidence-promotion-honesty
   meta-gate, check-output-contracts, check-traceability, check-spec-format,
   check-spec-numbering, check-doc-index, check-support-tiers,
   `cargo xtask goldens check`, `cargo xtask fixtures`).
7. Behavioral 3-way output confirmed with the worktree binary: orphan → non-zero
   exit with `orphan_receipt`; no ledger → exit 0 with `not_available`; gap
   present → exit 0 with `receipt_ok`.

## Acceptance Examples

### Orphan receipt — gap absent from ledger

```
receipt at /tmp/r/receipt.json is structurally valid; cross_reference: orphan_receipt
ripr: receipt cross-reference failed: orphan_receipt
```

Exit code: non-zero.

### No ledger — fail-closed not_available

```
receipt at /tmp/r/receipt.json is structurally valid; cross_reference: not_available
```

Exit code: 0. Ledger absent ≠ receipt valid.

### Gap present in ledger

```
receipt at /tmp/r/receipt.json is structurally valid; cross_reference: receipt_ok
```

Exit code: 0.

## Required Evidence

### Unit tests (app/receipt.rs)

| Test | Control |
|---|---|
| `receipt_check_orphan_when_gap_absent_from_ledger` | Control 1: orphan_receipt |
| `receipt_check_cross_reference_not_available_when_ledger_absent` | Control 2: fail-closed not_available |
| `receipt_check_ok_when_gap_present` | Control 3: receipt_ok |

### CLI smoke (tests/cli_smoke.rs)

| Test | Control |
|---|---|
| `receipt_check_orphan_exits_nonzero` | Control 4: binary non-zero exit for orphan |

### Pre-existing honesty guard

| Test | Role |
|---|---|
| `ledger_summary_does_not_emit_fabricated_receipt_state_counts` | #1130: ledger summary must not fabricate counts; must still pass |

## Test Mapping

| Test | Spec section |
|---|---|
| `receipt_check_orphan_when_gap_absent_from_ledger` | Controls §1, Behavior (orphan_receipt row) |
| `receipt_check_cross_reference_not_available_when_ledger_absent` | Controls §2, Behavior (fail-closed honesty rule) |
| `receipt_check_ok_when_gap_present` | Controls §3, Behavior (receipt_ok row) |
| `receipt_check_orphan_exits_nonzero` | Controls §4, Acceptance §7 |
| `ledger_summary_does_not_emit_fabricated_receipt_state_counts` | #1130 alignment |

## Implementation Mapping

| Component | Location |
|---|---|
| `ReceiptCrossRefResult` enum | `crates/ripr/src/app/receipt.rs` |
| `cross_reference_receipt` fn | `crates/ripr/src/app/receipt.rs` |
| `check_receipt` (returns tuple) | `crates/ripr/src/app/receipt.rs` |
| `--ledger` arg parsing | `crates/ripr/src/cli/commands/receipt.rs` |
| `parse_gap_records_json` (reused) | `crates/ripr/src/output/gap_decision_ledger.rs` |

## Metrics

- `unit_test_pass_rate`: 4 new controls (3 unit + 1 smoke) pass.
