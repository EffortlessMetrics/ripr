# `ripr` Soft-Gate

The `ripr` soft-gate turns the advisory `ripr` self-dogfood lane into an
**acknowledgeable gate**: narrow, calibrated, and explicit about what it
does and does not block on.

This page records the older repo-local soft-gate doctrine. For the current
generated-workflow gate evaluator, mode vocabulary, acknowledgement behavior,
and artifact layout, see [Calibrated gate policy](../CALIBRATED_GATE_POLICY.md).

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
  `docs/CI.md`'s Verification Economics section. It does not claim
  runtime mutation outcomes.

## Trigger criteria

The soft-gate fires only when **all** of the following are true:

1. **Finding class** is `reachable_unrevealed` or `weakly_exposed`.
2. **Production Rust changed** in this PR.
3. **No nearby test changed** (the heuristic for "nearby" is the same
   one `ripr` already uses internally: same module, same fixture, same
   fixture-test pair).
4. The finding is **not suppressed** in `.ripr/suppressions.toml` (the
   canonical path used by `crates/ripr/src/config.rs`).
5. The finding's **`confidence` field clears the gate threshold**.
   `confidence` is the numeric f32 documented in `docs/OUTPUT_SCHEMA.md`
   (range 0.0-1.0). The threshold is set in
   `policy/ripr-soft-gate.toml` (default proposal: `0.85`); it is tuned
   from the calibration data, not pinned in this doc.

If any of those is false, the gate stays green.

## What the soft-gate does **not** block on

- **Baseline (pre-existing) findings.** The gate evaluates only the
  delta against `origin/<base>`.
- **All non-trigger finding classes** (matching
  `policy/ripr-soft-gate.toml` `[gate].ignore_finding_classes`):
  `static_unknown`, `infection_unknown`, `propagation_unknown`,
  `no_static_path`, and `exposed`. The unknown classes exist precisely
  so the analyzer can record that some stage of the static path is
  undecided; failing on unknowns would punish honest uncertainty.
  `no_static_path` is noise the gate must not amplify, and `exposed`
  is the success state.
- **Mutation outcomes.** `ripr` is a static analyzer. Runtime mutation
  outcome terms are forbidden by `cargo xtask check-static-language`.
  The soft-gate inherits that vocabulary constraint.
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
loaded by `crates/ripr/src/config.rs`; parser in
`crates/ripr/src/output/suppressions.rs`). The entries follow the schema
documented in `docs/CONFIGURATION.md` and `docs/CI.md` (Verification
Economics section). Each suppression must record:

- a `kind` (`exposure_gap` or `test_efficiency`),
- the kind-specific selector: `finding_id` for `exposure_gap`, or
  `test` (with optional `path` narrowing) for `test_efficiency`,
- an `owner` (team/area),
- a `reason`.

`expires` (ISO-8601 `YYYY-MM-DD`) is optional. Expired entries do not
apply and surface as warnings on the badge (matching the `suppressions/v1`
policy in `docs/IMPLEMENTATION_CAMPAIGNS.md`); they do not hard-fail the
gate. Suppressions are reviewed at every release readiness check.

Identity is `(kind, selector)`: for `exposure_gap` the `finding_id`
itself is the unique key; for `test_efficiency` the `(test, path)` pair
is. There is no separate top-level `id` field.

## Implementation posture

- **Current state**: the historical soft-gate doctrine remains useful policy
  background, but the active gate implementation is `ripr gate evaluate` and
  the generated-workflow integration documented in
  [Calibrated gate policy](../CALIBRATED_GATE_POLICY.md).
- **Follow-up work**: tune explicit gate modes and baselines from calibration
  evidence. Do not treat this historical `cargo xtask ci ripr-soft-gate`
  sketch as the current implementation surface.

## Confidence field contract

The `confidence` field referenced as the gate threshold is the numeric
`f32` that `ripr` emits per finding. `docs/OUTPUT_SCHEMA.md` shows the
field by example (e.g. `0.92`).

**The authoritative type and range contract during the rollout is
`policy/ripr-soft-gate.toml`'s `[gate].confidence_threshold`
(`f32`, inclusive range `0.0-1.0`).** Reading order is: this section
first, then the policy TOML for the canonical numeric range, then
OUTPUT_SCHEMA.md for the field's appearance in `ripr` output. There is
no deferral loop; the policy file is authoritative today.

The follow-up PR that wires the xtask command will lift the same range
into a dedicated field-contract section in `docs/OUTPUT_SCHEMA.md` so
the canonical schema doc carries the contract directly. That move
**does not** change the meaning, only its location: the policy file
will then become a mirror.

The categorical `high | medium | low | unknown` values that exist on
RIPR stage evidence (`why_now.confidence`, `relation_confidence`,
`packets[].confidence`) are unrelated. The soft-gate thresholds against
the finding-level numeric field, not the stage-level enum.

## --labels-json contract

The `--labels-json` argument the future xtask command consumes is the
`pull_request.labels` array of objects from the GitHub Actions event
payload, passed through verbatim:

```json
[
  { "name": "full-ci" },
  { "name": "ripr-waive" }
]
```

The command consults only the `name` field of each entry, matches it
against `[labels]` in `policy/ripr-soft-gate.toml` (and, transitively,
the `[[label]]` entries in `policy/ci-budget.toml`), and treats
unrecognized labels as advisory metadata. An empty array is valid; it means no
acknowledgement labels are present.

In CI this is provided as
`--labels-json '${{ toJSON(github.event.pull_request.labels) }}'` so
the JSON is well-formed and quoted by the runner.

## Activation criteria (shadow to active transition)

The `status = "shadow"` value in `policy/ripr-soft-gate.toml` is flipped
to `status = "active"` by the follow-up PR that wires the
`cargo xtask ci ripr-soft-gate` command (see "Implementation posture"
above). That PR is also where the `--findings`, `--suppressions`,
`--threshold-config`, and `--labels-json` flow becomes a real exit code
instead of a `|| true` advisory.

The `ripr-self-dogfood` lane referenced below is registered in
`policy/ci-lane-whitelist.toml` and runs as part of the `rust` job in
`.github/workflows/ci.yml` via `cargo xtask dogfood` and
`cargo xtask reports index` (the lane's `commands` field). The xtask
command is gated to flip `status` only after **all** of these
conditions are verified
against that lane's `ci-actuals.json` upload:

1. The `ripr-self-dogfood` lane has uploaded at least 14
   `ci-actuals.json` artifacts on `main` over a rolling 14-day window
   (matching `[calibration]` in the policy file).
2. The 95th percentile of finding counts per `ci-actuals.json` is below
   a documented number that the threshold can credibly catch.
3. A documented number of `ripr-waive` labels were applied during
   shadow mode (so the gate's expected false-positive shape is known).
4. A 1-week dry-run period during which the xtask command emits a
   non-zero exit code into the step summary as if the gate were
   active, but the workflow conclusion stays success.

The follow-up PR records the actual numbers from the calibration data,
flips `status`, and turns the workflow step from `|| true` into the
real exit code. The transition is observable in git history; there is
no silent flag flip.

## Why a soft-gate and not a hard fail?

`ripr` makes claims about static *exposure*: whether a discriminator
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

- `docs/CI.md` - verification economics policy, LEM bands, labels,
  and the multi-PR rollout map (the canonical source for the wider
  CI economics policy).
- `docs/STATIC_EXPOSURE_MODEL.md`
- `docs/OUTPUT_SCHEMA.md`
- `docs/CONFIGURATION.md`
