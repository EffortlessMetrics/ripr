# RIPR-SPEC-0093: Match-Arm Blind-Reach Downgrade

Status: proposed

Owner: product / swarm

Created: 2026-06-13

Linked proposal:

- None

Linked ADRs:

- None

Linked plan:

- None

Linked issues:

- #1198 — Arm-blind reach: match-arm mutation reported exposed/confidence-1.0 via a test that never observes the arm

Linked PRs:

- None yet

Support-tier impact:

- Narrows one false-actionable in the Rust static exposure loop: `MatchArm`
  probes whose only associated test has no identifier token from the arm's
  pattern or variant in its assertion text are downgraded from `exposed` to
  `weakly_exposed`. This is a honesty improvement, not a tier promotion or
  demotion. The `usable alpha` tier for "Rust static exposure loop" remains
  unchanged. Claim boundaries and tier governance remain governed by the
  canonical ledger in [support tiers](../status/SUPPORT_TIERS.md).

Policy impact:

- No new crates, binaries, dependencies, parsers, or LSP servers.
- Register this spec in `policy/doc-artifacts.toml`.
- No schema version bump (`arm_observation_unverified` is a reason string inside
  the existing `discriminate.summary` field, not a new JSON field).

## Problem

A `MatchArm` probe represents a mutation on one arm of a `match` expression.
`ripr` reports it `exposed` (confidence 1.0) when:

1. A related test reaches the owner function (`calls_owner`).
2. That test has an exact-value oracle (`OracleKind::ExactValue`) which fires
   `oracle_matches_family(MatchArm, _)`.
3. `classify()` sees `discriminate.state == Yes` → `ExposureClass::Exposed`.

The false-actionable: `oracle_matches_family` for `MatchArm` accepts ANY
`ExactValue` assertion — including one that exercises a DIFFERENT arm. The
static data (call names, assertion text) does not encode which arm the test
actually enters, so the classification is wrong.

Concrete repro:

```rust
pub fn reason(x: Option<i32>) -> i32 {
    match x {
        Some(v) => v + 1,
        None => 0,   // ← probe target (new arm in diff)
    }
}

#[test]
fn only_some_arm() {
    assert_eq!(reason(Some(5)), 6);  // exercises Some arm only
}
```

Before this spec: ripr reports `None => 0` as `exposed / confidence 1.0`.
After: `weakly_exposed / arm_observation_unverified / confidence 0.92`.

## Behavior

For `ProbeFamily::MatchArm` probes:

1. For each assertion that matches the probe (via token, family, or escape
   hatch), record whether the assertion text contains any identifier token from
   the probe expression (`has_token_match`).
2. If the probe is a `MatchArm` AND no matched assertion has `has_token_match`,
   set `arm_observation_unverified = true`.
3. When `arm_observation_unverified` is true, `build_discriminate_evidence`
   returns `StageState::Weak` with summary
   `"Discriminator unconfirmed: no assertion text references this arm's pattern or variant (arm_observation_unverified)"`.
4. `classify()` sees `discriminate.state == Weak` → returns
   `ExposureClass::WeaklyExposed` instead of `ExposureClass::Exposed`.

Non-MatchArm probes are UNCHANGED. The escape-hatch behavior for other families
is also unchanged.

### Token-match exception

A `MatchArm` probe that has at least one matched assertion whose text contains
an identifier token from the probe expression (e.g. `Idle` in
`Status::Idle => 0,` matched by `assert_eq!(classify(Status::Idle), 0)`) stays
`exposed`. This confirms the test references the arm's specific pattern or
variant.

## Non-Goals

- Running mutations or dynamic analysis to confirm arm coverage.
- Distinguishing which specific arm a test enters at runtime.
- Changing the behavior for non-MatchArm probes.
- Bumping the JSON schema version (no new output fields).
- Changing `reachable_unrevealed` (the test IS reachable and HAS an oracle).

## Required Evidence

- `fixtures/match_arm_blind`: probe on `None => 0`, single-assertion test on
  `Some` arm → must produce `weakly_exposed`, `arm_observation_unverified`.
- `fixtures/match_arm_observer`: probe on `Status::Idle => 0`, test asserts
  `classify(Status::Idle)` (token `Idle` in assertion) → must stay `exposed`.

## Inputs

- A diff adding or modifying a `match` arm.
- Related tests found by `find_related_tests`.

## Outputs

- `weakly_exposed` (severity: warning) when `arm_observation_unverified`.
- `discriminate.summary` contains `arm_observation_unverified`.
- `recommended_next_step`: existing MatchArm guidance ("Strengthen the related
  assertion so it discriminates the changed behavior.").
- `missing`: `["No strong discriminator was detected"]`.

## Acceptance Examples

### arm-blind (must downgrade)

```
probe family:  match_arm
expression:    None => 0,
test:          assert_eq!(reason(Some(5)), 6);

Before fix:    exposed / confidence 1.0
After fix:     weakly_exposed / confidence 0.92 / arm_observation_unverified
```

### arm-observer (must stay exposed)

```
probe family:  match_arm
expression:    Status::Idle => 0,
test:          assert_eq!(classify(Status::Idle), 0);

Before fix:    exposed / confidence 1.0
After fix:     exposed / confidence 1.0   (unchanged — Idle token_match fires)
```

## Test Mapping

- `crates/ripr/src/analysis/classify/reveal.rs` — unit tests for
  `assertion_matches_probe_detail` and `analyze_related_assertions`.
- `fixtures/match_arm_blind/` — golden fixture for the arm-blind case.
- `fixtures/match_arm_observer/` — golden fixture for the arm-observer case.

## Implementation Mapping

- `crates/ripr/src/analysis/classify/reveal.rs`:
  - `assertion_matches_probe_detail` (renamed from `assertion_matches_probe`,
    returns `(matched, has_token_match)`).
  - `RevealAssertionAnalysis.arm_observation_unverified`.
  - `analyze_related_assertions` populates `arm_observation_unverified`.
  - `build_discriminate_evidence` checks `arm_observation_unverified`.

## CI Proof

- `cargo xtask fixtures match_arm_blind`
- `cargo xtask fixtures match_arm_observer`
- `cargo xtask goldens check`
- `cargo test -p ripr`

## Metrics

- `match_arm_blind_downgrades_to_weakly_exposed`: fixture `match_arm_blind`
  produces `weakly_exposed` with `arm_observation_unverified` in the
  discriminate summary (validated by `cargo xtask fixtures match_arm_blind`).

No tier change required. This is a honesty narrowing of a known false-actionable
within the existing `usable alpha` Rust exposure loop.

## Failure Modes

- A MatchArm probe with no filtered tokens (e.g. `None => 0`) will always
  produce `weakly_exposed` regardless of what the test does. This is acceptable:
  we cannot confirm arm observation when the static data has no arm-specific
  tokens. Callers with a genuine observer should use a named constant or variant
  that produces a token in the probe expression.
- A MatchArm probe that shares enum-type tokens across arms (e.g. `Status::X`
  and `Status::Y` share `Status`) may have spurious token_match from the shared
  prefix. This is a known limitation; the net direction is still fail-closed
  (fewer false-actables, not fewer true-positives).
