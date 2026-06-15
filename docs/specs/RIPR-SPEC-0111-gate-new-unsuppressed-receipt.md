# RIPR-SPEC-0111: Gate `new_unsuppressed` Receipt Field

Status: accepted

Owner: product / swarm

Created: 2026-06-15

Linked issues:

- #1038 (canonical thresholding receipt for `EffortlessMetrics/ub-review`)

Linked PRs:

- None yet

Support-tier impact:

- Additive JSON field on `gate-decision.json`; no schema version bump; no new
  command; no new flag; no new output format; no new blocking rule. The field
  is a downstream-thresholding receipt only. Tier labels and claim boundaries
  remain governed by the canonical ledger in
  [support tiers](../status/SUPPORT_TIERS.md).

Policy impact:

- New `NewUnsuppressed` struct in `crates/ripr/src/output/gate/model.rs`.
- `compute_new_unsuppressed` fn in `crates/ripr/src/output/gate.rs`.
- `new_unsuppressed_json` fn in `crates/ripr/src/output/gate/presentation.rs`.
- New `is_baseline_new` field on `GateDecision` struct (internal, not serialized).
- No new crate. No workspace-shape change. No public API change.

## Problem

### No single stable thresholding number

`ripr gate evaluate` writes `gate-decision.json` with `summary.blocking` and
`summary.advisory`, but no single number a generic downstream gate can use for
`max_new_unsuppressed=0` thresholding. `summary.blocking` excludes advisory
candidates; reading both fields requires custom logic and drifts when the gate
mode changes.

`EffortlessMetrics/ub-review` (and any similar upstream-review consumer) needs
one canonical count it can read and compare to a threshold without understanding
ripr's internal `blocking` vs. `advisory` distinction.

### The wrong place to read it

The `badge.json` finding count and the `check.json` summary count represent a
different analysis scope (repo-wide or diff findings) from the gate's own
`decisions[]`. Reading them would create a fourth drifting number — inconsistent
with the receipt the consumer already holds.

## Behavior

### `new_unsuppressed` top-level object

`gate evaluate` writes a new top-level `new_unsuppressed` object in
`gate-decision.json`:

```json
"new_unsuppressed": { "basis": "diff", "count": 3, "reason": null }
```

This field is `APPEND-ONLY` (no field removed, renamed, or re-meaninged) so
the `schema_version` stays `"0.1"`.

### Honest definition (locked)

`new_unsuppressed.count` is the number of decisions `d` in `decisions[]` where:

1. `candidate_class_is_policy_eligible(d.static_class)` is true — i.e.
   `d.static_class ∈ {"weakly_gripped","ungripped","reachable_unrevealed","weakly_exposed"}`.
2. `d.decision ∈ {"blocking","advisory"}` — suppressed, acknowledged, and
   not_applicable candidates are excluded (they are handled or ineligible).
3. If `basis == "baseline"`: additionally `d.is_baseline_new` is true (the
   per-decision baseline-new flag). If `basis == "diff"`: all surviving
   candidates in step 1 and 2 count — diff scope equals "new".

CRITICAL HONESTY POINT: this count INCLUDES policy-eligible `advisory`
decisions. It is NOT equal to `summary.blocking`. In `visible-only` mode, every
eligible candidate is advisory (`blocking=0`) but `count` must be the advisory
candidate count if any exist. An external thresholder applies its own
policy; ripr reports the candidate count regardless of its own block/advisory
label. Equating count to blocking would be WRONG.

### `basis` values

| basis | Meaning |
|---|---|
| `"diff"` | Diff-scoped run (`visible-only`, `acknowledgeable`, or new-gap without baseline). All surviving candidates are new by definition. |
| `"baseline"` | Baseline-aware run (`baseline-check` or `calibrated-gate` AND a baseline was actually read/used). Only candidates where `is_baseline_new=true` are counted. |
| `null` | FAIL CLOSED. `config_errors` is non-empty — analysis did not run. `count=0` and `reason` discloses the first config error. |

### Fail-closed rule

When `config_errors` is non-empty, analysis did not complete. Returning
`basis=null, count=0, reason=<first error>` ensures `count=0` NEVER reads as a
clean pass. A downstream thresholder at `max_new_unsuppressed=0` would pass on
`count=0` — the `reason` field is the mandatory disclosure.

## Controls

Two unit tests in `crates/ripr/src/output/gate.rs` (RIPR-SPEC-0111 block):

1. `new_unsuppressed_counts_advisory_policy_eligible_candidates_not_just_blocking`:
   visible-only mode, 1 policy-eligible candidate → decision is "advisory"
   (blocking=0) but `count=1 > blocking=0`. Proves advisory inclusion and that
   count ≠ blocking.

2. `new_unsuppressed_config_error_produces_null_basis_and_zero_count_with_reason`:
   calibrated-gate without baseline → `config_error` status → `basis=null`,
   `count=0`, `reason` starts with `"analysis did not run"`. Proves fail-closed
   behavior.

## Non-Goals

- `new_unsuppressed.count` is NOT `summary.blocking`. Equating them is a bug.
- This spec does NOT add a new CLI flag, command, or output format.
- The schema_version does NOT bump. The field is purely additive.
- `ripr` does NOT apply any threshold internally. The threshold
  (`max_new_unsuppressed=0` or any other value) is the consumer's policy.
- No mutation calibration is involved in counting. The count is purely a filter
  over the existing `decisions[]` array.
- This spec does NOT replace `summary.blocking` or `summary.advisory`.
  Consumers may still read those for mode-specific information.
- No new public API symbol is added. `NewUnsuppressed` is `pub(crate)`.
- No version bump / publish / release.

## Acceptance

1. All 2 unit controls pass.
2. `RUSTFLAGS="-D warnings" cargo build -p ripr -p xtask` passes.
3. `cargo clippy -p ripr --all-targets -- -D warnings` reports no issues.
4. `cargo fmt --check` passes under the pinned toolchain.
5. All xtask policy gates pass (incl. `check-output-contracts`,
   `check-traceability`, `check-spec-format`, `check-spec-numbering`,
   `check-doc-index`, `check-support-tiers`, `cargo xtask goldens check`,
   `cargo xtask fixtures`).
6. Behavioral honesty confirmed on the 5 hand-checked fixtures:
   - `gate-adoption/visible-only`: basis="diff", count=1 (advisory included, count ≠ blocking=0).
   - `calibrated-gate/visible-only-advisory`: basis="diff", count=1 (advisory; count ≠ blocking=0).
   - `gate-adoption/baseline-new-gap`: basis="baseline", count=1 (baseline-new gap counted).
   - `gate-adoption/missing-baseline-config`: basis=null, count=0, reason discloses config error (FAIL CLOSED).
   - `gate-adoption/acknowledged`: basis="diff", count=0 (acknowledged candidate excluded).

## Acceptance Examples

### visible-only mode (advisory candidate counted)

```json
"summary": { "blocking": 0, "advisory": 1, ... },
"new_unsuppressed": { "basis": "diff", "count": 1, "reason": null }
```

`count=1 > blocking=0` — proves advisory inclusion.

### baseline mode (new gap counted, existing gap excluded)

```json
"new_unsuppressed": { "basis": "baseline", "count": 1, "reason": null }
```

### config error (fail-closed)

```json
"new_unsuppressed": { "basis": null, "count": 0, "reason": "analysis did not run: baseline-check mode requires an explicit --baseline artifact" }
```

`basis=null` and `reason` prevent `count=0` from reading as "clean/pass".

### acknowledged candidate (excluded from count)

```json
"summary": { "acknowledged": 1, ... },
"new_unsuppressed": { "basis": "diff", "count": 0, "reason": null }
```

`count=0` even though a candidate existed — it is handled (acknowledged), not new+unsuppressed.

## Required Evidence

### Unit tests (output/gate.rs)

| Test | Control |
|---|---|
| `new_unsuppressed_counts_advisory_policy_eligible_candidates_not_just_blocking` | Control 1: advisory inclusion, count ≠ blocking |
| `new_unsuppressed_config_error_produces_null_basis_and_zero_count_with_reason` | Control 2: fail-closed |

### Golden fixtures

| Fixture | new_unsuppressed |
|---|---|
| `gate-adoption/visible-only` | basis="diff", count=1, reason=null |
| `calibrated-gate/visible-only-advisory` | basis="diff", count=1, reason=null |
| `gate-adoption/baseline-new-gap` | basis="baseline", count=1, reason=null |
| `gate-adoption/missing-baseline-config` | basis=null, count=0, reason="analysis did not run: ..." |
| `gate-adoption/acknowledged` | basis="diff", count=0, reason=null |
| `calibrated-gate/summary-and-suppressed` | basis="diff", count=1, reason=null (suppressed excluded) |

## Test Mapping

| Test | Spec section |
|---|---|
| `new_unsuppressed_counts_advisory_policy_eligible_candidates_not_just_blocking` | Controls §1, Behavior (advisory included), Acceptance §6 |
| `new_unsuppressed_config_error_produces_null_basis_and_zero_count_with_reason` | Controls §2, Behavior (fail-closed rule), Acceptance §6 |

## Implementation Mapping

| Component | Location |
|---|---|
| `NewUnsuppressed` struct | `crates/ripr/src/output/gate/model.rs` |
| `is_baseline_new` field on `GateDecision` | `crates/ripr/src/output/gate/model.rs` |
| `compute_new_unsuppressed` fn | `crates/ripr/src/output/gate.rs` |
| `new_unsuppressed_json` fn | `crates/ripr/src/output/gate/presentation.rs` |

## Metrics

- `unit_test_pass_rate`: 2 new controls pass.
- `gate_decisions_evaluated` (unchanged): still tracks the full candidate set.
