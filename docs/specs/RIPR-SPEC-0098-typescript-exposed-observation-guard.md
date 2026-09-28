# RIPR-SPEC-0098: TypeScript Exposed Observation Guard

Status: accepted

Owner: product / swarm

Created: 2026-06-14

Linked issues:

- #1235

Linked PRs:

- None yet

Support-tier impact:

- Narrows false-exposed over-claims for TypeScript preview findings where a
  strong oracle exists in the test body but the oracle's `observed_expression`
  does not flow from the changed sub-expression. Honesty improvement only
  (`exposed` → `weakly_exposed`); language_status stays Preview; no tier change.
  Claim boundaries and tier labels remain governed by the canonical ledger in
  [support tiers](../status/SUPPORT_TIERS.md).

Policy impact:

- No new crates, binaries, dependencies, parsers, or LSP servers.
- Register this spec in `policy/doc-artifacts.toml`.
- No schema version bump. The `discriminate.summary` message for the
  unverified case changes text only (not a new JSON field).

## Problem

In `crates/ripr/src/analysis/language/typescript/classifier.rs`, the ONLY
gate to `ExposureClass::Exposed` was:

```
} else if strongest_strength >= OracleStrength::Strong.rank() {
    (ExposureClass::Exposed, ...)
```

This fires whenever the reaching test contains ANY strong oracle
(`toBe` / `toEqual` / `toStrictEqual` / exact-payload `toThrow`) ANYWHERE in
its body — with NO check that the strong oracle's `observed_expression` flows
from the CHANGED line.

A concrete repro:

- Changed line: `console.log("audit", amount * 9)` (SideEffect, value never escapes)
- Related test has two `toBe` assertions on the UNCHANGED return value
  (`expect(applyDiscount(100, 10)).toBe(90)`, `expect(applyDiscount(50, 10)).toBe(45)`)
- Result before fix: `class:"exposed"`, `discriminate:"yes"` — a false claim

The TypeScript path has no analog of Rust's `decision.rs` gate
(Exposed requires propagate==Yes). Without an observation guard, any test
that contains strong oracles for OTHER values promotes a SideEffect or
non-escaping call to Exposed regardless of whether those oracles witness
the changed expression.

## Behavior

### Observation guard scope

The guard is scoped to **SideEffect and CallDeletion families only**, where
the false-exposed pattern is clearest and well-defined. For all other probe
families (ReturnValue, FieldConstruction, Predicate, MatchArm, ErrorPath)
the guard always confirms — the pre-existing behavior is preserved.

The reason for the narrow scope: value families commonly use the
"assign to variable then assert" pattern (`const result = fn(); expect(result).toBe(...)`)
where `observed_expression = "result"` does not contain the owner name.
Static analysis cannot reliably distinguish a variable assigned from a return
value from one set by a side effect, so the guard conservatively confirms
for these families.

### Observation guard logic (SideEffect / CallDeletion only)

When the strongest related-test oracle satisfies the Strong threshold
(`rank >= 5`), the classifier checks whether the observation is confirmed:

The confirmation decision keys on **`oracle_kind`**, which the live oracle
extractor (`oracle.rs`) always populates, rather than on `observed_expression`,
which is optional metadata. A strong assertion confirms the observation
(stays Exposed) when ANY of:
1. Its `oracle_kind` is `MockExpectation | Snapshot | WholeObjectEquality`
   (effect-shape observer — confirms the call side effect unconditionally).
2. Its `observed_expression` is present and contains a changed identifier token
   (length > 3, alphanumeric/underscore only) from the changed sub-expression.
3. Its `observed_expression` is present and does NOT contain the owner name
   (i.e. it is asserting something other than the owner return value — may be a
   closure-local side-effect variable, a mock result, or side channel).

**Fail-closed default (#1235 hardening)**: confirmation must be affirmatively
established by one of the rules above. If no strong assertion qualifies — i.e.
the only strong oracles are value-shaped (`ExactValue` / `ExactErrorVariant`)
observing the owner return value, OR no `observed_expression` metadata is
available to prove a side-channel — the guard returns
`observation_confirmed = false` and the finding is downgraded.

This is the exact false-positive pattern: the only strong assertions visible
in the test body are `expect(ownerFn(...)).toBe(...)` calls asserting the
UNCHANGED return value, while the changed SideEffect line (`console.log(...)`)
has no direct observer.

**Why not fail-open on missing `observed_expression`** (#1235): an earlier
revision returned `confirmed` when no strong assertion carried
`observed_expression` ("can't tell what's observed → assume observed →
exposed"). That is precisely the over-claim this guard exists to kill, and it
is unsafe because `observed_expression` is optional. Absence of proof is not
proof of observation, so the missing-metadata case now downgrades. Effect
observers are still credited because they are detected from `oracle_kind`
alone, independent of `observed_expression`.

### Downgraded arm

When the observation guard FAILS for a SideEffect / CallDeletion probe,
the finding is downgraded to `WeaklyExposed` with:

- `reach: yes`, `observe: weak`, `discriminate: weak`
- A named limitation in `missing`:
  `propagation_unknown: changed value sinks to a non-escaping call effect
  (<expr>); all strong assertions observe the owner return value, not this
  call effect; propagation unknown`
- `discriminate_summary` changes from "changed behavior is discriminated"
  to: "strong oracle found but no assertion's observed_expression flows from
  the changed sub-expression; observation_unverified — discriminate unknown"
- `confidence`: 0.4 (same as other WeaklyExposed findings; no new constant)

### Token rule

Identifier tokens are extracted by splitting on any character that is not
ASCII alphanumeric or underscore, then keeping only segments of length > 3.
This excludes short qualifiers and operator sub-strings while retaining
meaningful variable names.

### What does NOT change

- `language_status` stays `Preview`.
- `infect`/`propagate` stages remain `Unknown` — the TS adapter still does
  not model infection or propagation.
- Class may ONLY move `exposed` → `weakly_exposed`. NEVER to `no_static_path`.
- Forbidden output vocabulary (`killed`, `survived`, `untested`, `proven`,
  `adequate`) is not used.
- No new JSON fields; no schema version bump.
- Value families (ReturnValue, FieldConstruction, Predicate, MatchArm, ErrorPath)
  are NOT affected by this guard — their existing behavior is unchanged.

## Required Evidence

Unit tests (in `crates/ripr/src/analysis/language/typescript/tests.rs`):

1. `ts_swallowed_console_log_exposed_downgrade` — repro: changed
   `console.log("audit", amount*9)` (SideEffect, non-escaping), two strong
   `toBe` on the UNCHANGED return value (`observed_expression` contains owner
   name `applyDiscount`). MUST produce `class:weakly_exposed`,
   `propagation_unknown` in missing, `discriminate != yes`.

2. `ts_returnvalue_genuinely_observed_control` — owner return arithmetic
   changed (`return amount - 12`, ReturnValue family), test
   `expect(applyDiscount(100,100)).toBe(90)` with
   `observed_expression = "applyDiscount(100,100)"`. MUST STAY `class:exposed`,
   `discriminate:yes`. This verifies the guard does not apply to ReturnValue
   (non-effect) families. Explicit assertion so future tightening that
   accidentally breaks ReturnValue probes fails here.

3. `ts_sibling_assertion_non_owner_prevents_downgrade` — a SideEffect where
   one strong assertion contains the owner name AND a second strong assertion
   does NOT contain the owner name (`observed_expression: "sideEffectLog"`).
   Since not ALL observed_expressions contain the owner name, the guard treats
   the second assertion as a potential side-effect observer → MUST stay
   `class:exposed` (conservative: no downgrade when any assertion might
   observe the effect).

4. `ts_field_construction_observed_control` — a FieldConstruction change
   (`timeout: 5000,`) with a strong `toEqual` assertion. MUST stay
   `class:exposed` (guard is not applied to FieldConstruction family).

5. `ts_swallowed_console_log_downgrade_live_extractor` (#1235) — LIVE-pipeline
   regression guard. Source and test text are parsed by the REAL
   `extract_owners` / `extract_tests` (so assertions carry whatever
   `observed_expression` the live extractor produces, not a hand-set field).
   A swallowed `console.log` observed only by value-shaped `toBe` on the owner
   return value MUST downgrade to `weakly_exposed` with `propagation_unknown`.
   Fails if the guard ever regresses to a fail-OPEN keyed on
   `observed_expression` population.

6. `ts_side_effect_observed_by_mock_expectation_stays_exposed` (#1235) — a
   SideEffect observed by `toHaveBeenCalledWith` (`oracle_kind:
   MockExpectation`, `observed_expression: None`). The decision rests on
   `oracle_kind` alone, so it MUST stay `class:exposed`, `discriminate:yes`.
   Protects the effect-observer path from the fail-closed downgrade.

No new golden fixtures are required for this change (the guard operates on
already-extracted oracle metadata; no JSON schema changes).

## Non-Goals

- The `typescript_flow_sink_for` → `FlowSinkKind::Unknown` sink-honesty half
  (#1235 follow-up): the guard reads `observed_expression`, not sink-Unknown,
  so the repro is fixed without the sink half. Note it in a follow-up issue.
- Type inference or tsc integration to resolve whether a value flows to an
  assertion at runtime.
- Changing the TypeScript support tier (stays PREVIEW, advisory-only).
- Emitting actionable repair packets from TypeScript preview findings.
- Adding a JSON schema version bump.
- Upgrading `infect`/`propagate` stage evidence in the TypeScript adapter.

## Acceptance Examples

### Before (false-exposed)

```
changed:  console.log("audit", amount * 9);   // SideEffect
related:  expect(applyDiscount(100, 10)).toBe(90);  // observed_expression = "applyDiscount(100, 10)"
          expect(applyDiscount(50, 10)).toBe(45);   // observed_expression = "applyDiscount(50, 10)"
result:   class: exposed  ← WRONG
```

### After (correctly downgraded)

```
changed:  console.log("audit", amount * 9);
result:   class: weakly_exposed
          missing: propagation_unknown: changed value sinks to a non-escaping call effect ...
          discriminate: weak
```

### Control 1 (must not over-correct: ReturnValue family bypasses guard)

```
changed:  return amount - 12;   // ReturnValue — guard NOT applied
related:  expect(applyDiscount(100, 100)).toBe(90);  // observed_expression = "applyDiscount(100, 100)"
result:   class: exposed  ← CORRECT (ReturnValue family not in guard scope)
          discriminate: yes
```

### Control 2 (must not over-correct: non-owner assertion prevents downgrade)

```
changed:  console.log("audit", amount * 9);   // SideEffect — guard applied
related:  expect(applyDiscount(100, 10)).toBe(90);   // has owner name → owner-return-value assertion
          expect(sideEffectLog).toBe(true);           // no owner name → potential effect observer
result:   class: exposed  ← CORRECT (conservative: sideEffectLog might be observing the side effect)
```

## Test Mapping

- `crates/ripr/src/analysis/language/typescript/tests.rs::tests::ts_swallowed_console_log_exposed_downgrade`
- `crates/ripr/src/analysis/language/typescript/tests.rs::tests::ts_returnvalue_genuinely_observed_control`
- `crates/ripr/src/analysis/language/typescript/tests.rs::tests::ts_sibling_assertion_non_owner_prevents_downgrade`
- `crates/ripr/src/analysis/language/typescript/tests.rs::tests::ts_field_construction_observed_control`

## Implementation Mapping

- `crates/ripr/src/analysis/language/typescript/classifier.rs` — `ts_changed_value_is_observed`, `ts_observation_guard_limitation`, updated class-decision arm, moved `flow_sink` computation, updated `discriminate_summary`

## Metrics

- `ts_exposed_observation_guard_downgrades_to_weakly_exposed` — console.log repro now weakly_exposed
- `ts_returnvalue_non_effect_family_bypasses_guard` — ReturnValue (non-effect) family is not guarded
- `ts_non_owner_assertion_prevents_downgrade` — non-owner assertion in test body prevents downgrade
- `ts_field_construction_non_effect_family_bypasses_guard` — FieldConstruction (non-effect) family is not guarded
