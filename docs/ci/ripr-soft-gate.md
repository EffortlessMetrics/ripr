# `ripr` Soft-Gate

The `ripr` soft-gate is the final stage in the multi-PR rollout. It turns
the advisory `ripr` self-dogfood lane (PR 10) into an **acknowledgeable
gate** — narrow, calibrated, and explicit about what it does and does
not block on.

## Doctrine constraints

This document records the contract that the soft-gate must respect:

- The gate is **soft**: failure can always be acknowledged with a label.
  It never permanently blocks a merge.
- The gate is **scoped**: it triggers only on a tightly defined set of
  finding-state combinations. It does not block on baseline (pre-existing)
  findings.
- The gate is **calibrated**: it does not turn on until at least 2 weeks
  of `ci-actuals.json` data has accumulated for the `ripr` lane (so the
  threshold is informed by distribution, not guesswork).
- The gate uses **only** the static-language vocabulary defined in
  `docs/RIPR_EVIDENCE_POLICY.md`. It does not claim runtime mutation
  outcomes.

## Trigger criteria

The soft-gate fires only when **all** of the following are true:

1. **Finding class** is `reachable_unrevealed` or `weakly_exposed`.
2. **Production Rust changed** in this PR.
3. **No nearby test changed** (the heuristic for "nearby" is the same
   one `ripr` already uses internally — same module, same fixture, same
   fixture-test pair).
4. The finding is **not suppressed** in `.ripr/suppressions.toml` (the
   canonical path used by `crates/ripr/src/config.rs`).
5. The finding's **`confidence` field clears the gate threshold**.
   `confidence` is the numeric f32 documented in `docs/OUTPUT_SCHEMA.md`
   (range 0.0–1.0). The threshold is set in
   `policy/ripr-soft-gate.toml` (default proposal: `0.85`); it is tuned
   from the calibration data, not pinned in this doc.

If any of those is false, the gate stays green.

## What the soft-gate does **not** block on

- **Baseline (pre-existing) findings.** The gate evaluates only the
  delta against `origin/<base>`.
- **`static_unknown` findings.** The unknown class exists precisely so
  the analyzer can record that the static path is undecided; failing on
  unknowns would punish honest uncertainty.
- **Mutation outcomes.** `ripr` is a static analyzer. Words like
  `killed` and `survived` are forbidden by `cargo xtask
  check-static-language`. The soft-gate inherits that vocabulary
  constraint.
- **Findings outside the trigger criteria.** PRs that change tests
  alongside production, or that touch only docs/policy, never trip the
  gate.

## Acknowledgement labels

| Label                  | Effect                                                        |
| ---------------------- | ------------------------------------------------------------- |
| `ripr-waive`           | Acknowledge the finding for this PR. Reviewer must comment.   |
| `full-ci`              | Run all advisory lanes; demotes `ripr-waive` requirement.     |
| `ci-budget-ack`        | Acknowledge elevated forecast (does not waive `ripr` itself). |

Per the doctrine, `ripr-waive` is intentionally noisy in PR summaries so
reviewers can see when it has been used.

## Suppression file

Long-lived suppressions live in `.ripr/suppressions.toml` (canonical;
loaded by `crates/ripr/src/config.rs`). The entries follow the schema
documented in `docs/OUTPUT_SCHEMA.md` and `docs/CONFIGURATION.md`. Each
suppression must record:

- a unique `id`,
- an `owner` (team/area),
- a `reason`,
- an `expires` date.

Expired suppressions fail the gate. Suppressions are reviewed at every
release readiness check.

## Implementation posture

- **PR 14 (this PR)**: contract, scope, trigger criteria, and label
  vocabulary documented. **No enforcement code yet.**
- **Follow-up PR**: wire `cargo xtask ci ripr-soft-gate
  --findings target/ripr/reports/ripr-diff.json
  --suppressions .ripr/suppressions.toml
  --threshold-config policy/ripr-soft-gate.toml
  --labels-json "$LABELS_JSON"`, then activate the gate after the
  calibration window has produced 2 weeks of `ci-actuals.json` data on
  the `ripr_self_dogfood` lane.

## Why a soft-gate and not a hard fail?

`ripr` makes claims about static *exposure* — whether a discriminator
appears to exist. The right reaction to a `reachable_unrevealed` is
"investigate", not "must fix before merge". A hard fail would either:

- Frustrate authors with false positives (the static analysis is
  intentionally conservative; some unrevealed paths are genuinely
  unreachable in practice and need a suppression entry, not a code
  change), **or**
- Encourage reviewers to write throwaway tests just to silence the gate,
  which damages the test signal.

A soft-gate with a recorded waiver gives the same author-attention
without the false-positive damage.

## See also

- `docs/RIPR_EVIDENCE_POLICY.md`
- `docs/ci/cost-and-verification-policy.md`
- `docs/ci/ripr-ci-rollout.md`
- `docs/STATIC_EXPOSURE_MODEL.md`
- `docs/OUTPUT_SCHEMA.md`
- `docs/CONFIGURATION.md`
