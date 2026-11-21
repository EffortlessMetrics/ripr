# RIPR-SPEC-0094: observation_unverified Guard Generalization

Status: proposed

Owner: product / swarm

Created: 2026-06-13

Linked issues:

- #1216

Linked PRs:

- None yet

Support-tier impact:

- Narrows false-actionable over-claims for ReturnValue, FieldConstruction,
  SideEffect, and CallDeletion probes. Also closes a type-blind hole in the
  MatchArm guard from RIPR-SPEC-0093. Honesty improvement only; no tier change.
  Claim boundaries remain governed by the canonical ledger in
  [support tiers](../status/SUPPORT_TIERS.md).

Policy impact:

- No new crates, binaries, dependencies, parsers, or LSP servers.
- Register this spec in `policy/doc-artifacts.toml`.
- No schema version bump. The `discriminate.summary` message for the unverified
  case changes text only (not a new JSON field).

## Behavior

When a probe belongs to a family where the changed sub-expression must be
directly witnessed by an assertion to justify `exposed` (ReturnValue,
FieldConstruction, SideEffect, CallDeletion, MatchArm), and no assertion
confirms observation of that changed expression, the discriminate stage is
marked `weak` and the finding is downgraded to `weakly_exposed`. The
discriminate summary contains `observation_unverified`.

Observation confirmation differs by family group:

- **Value families** (ReturnValue, FieldConstruction, MatchArm): the only
  static confirmation signal is a `token_match` — an assertion whose text
  contains an identifier token of length > 3 from the probe expression (for
  MatchArm, restricted to the variant tokens after `::`). A value assertion
  whose oracle kind merely *shape*-matches the seam (e.g. an `assert_eq!`
  comparing whole objects) does **not** confirm; it must name the changed
  sub-expression.
- **Effect families** (SideEffect, CallDeletion): the canonical observer of a
  side effect or outbound call is a mock/expectation, a snapshot, or a
  whole-object equality capturing the resulting persisted state. These
  kind-match the seam without sharing a probe token. A genuine effect observer
  (`effect_observer_confirms`: `MockExpectation | Snapshot |
  WholeObjectEquality`) therefore confirms observation in addition to
  `token_match`. A plain non-observing assertion (e.g. `assert!(result)`) that
  only fired via the single-assertion escape hatch is **not** an effect
  observer, so it stays `observation_unverified`.

## Problem

RIPR-SPEC-0093 introduced `arm_observation_unverified` for MatchArm only.
The same structural gap exists for four other families: ReturnValue,
FieldConstruction, SideEffect, and CallDeletion. Each can reach
`exposed`/1.00 via the `assertion_count == 1` escape hatch without any
assertion token referencing the changed sub-expression.

Additionally, inside the MatchArm guard, token_match was computed type-blind:
for `Mode::Frozen`, the tokens include `["Frozen", "Mode"]`. A sibling assertion
for `Mode::Warm` contains `"Mode"` (shared qualifier), which spuriously cleared
`observation_unverified` even though the test only exercises `Warm`.

## Fix

### Part A: Generalize the guard

Replace the `is_match_arm` single-family predicate with `needs_token_confirmation`
covering `{MatchArm, ReturnValue, FieldConstruction, SideEffect, CallDeletion}`.
Rename `arm_observation_unverified` to `observation_unverified`. Apply the
start-pessimistic / token_match-clears logic to all covered families. The
discriminate message when unverified becomes:
`"Discriminator unconfirmed: no assertion text references this probe's changed expression (observation_unverified)"`.

### Part B: Variant-scoped MatchArm token_match

Add `match_arm_variant_tokens(expression)` to extract only the variant tokens
(identifiers immediately after `::`) from the probe expression. For MatchArm,
`has_token_match` uses only these variant tokens, not the full token set. This
prevents the shared enum qualifier from confirming a sibling-arm assertion.

### Part C: Effect-family observers confirm without a token

For the **effect** families (SideEffect, CallDeletion), the changed behavior is
a side effect or outbound call whose legitimate observer is a mock/expectation
or a persisted-state snapshot/whole-object assertion that kind-matches the seam
without naming a probe token. Generalizing Part A on `token_match` alone would
wrongly flag a real mock observer as `observation_unverified`. Part C adds
`effect_observer_confirms(assertion)` (`MockExpectation | Snapshot |
WholeObjectEquality`) and ORs it into the clear-signal **for effect families
only**. This is intentionally narrower than `oracle_matches_family` for effect
families — it excludes the broad `text.contains("assert" | "expect")` substring
matches, so a plain non-observing assertion does not clear the guard. Value
families are unchanged: only a `token_match` clears them.

## Non-Goals

- Running mutations or dynamic analysis.
- Changing behavior for Predicate or ErrorPath probes.
- Bumping the JSON schema version.
- Changing the oracle-strength → discriminate-state mapping. A Medium effect
  observer (bare `MockExpectation`) remains `weakly_exposed` via the existing
  strength path; `exposed` still requires a Strong (exact-value /
  whole-object) discriminator. Part C only fixes *which* observers clear
  `observation_unverified`; it does not promote Medium oracles to Strong.

## Required Evidence

Per-family fixture pairs (blind and confirmed) plus the MatchArm qualifier-blind lock.

## Inputs

- A diff changing a return expression, struct field initializer, side-effect call, or call-deletion.
- Related tests found by `find_related_tests`.

## Outputs

- `weakly_exposed` with `observation_unverified` when no confirming assertion
  exists (value family: no `token_match`; effect family: no `token_match` and
  no effect observer).
- `exposed` when the discriminator is confirmed AND strong: a `token_match`
  with a Strong oracle (value families), or a Strong effect observer
  (exact-value / whole-object persisted state) for effect families.
- `discriminate.summary` contains `"observation_unverified"` only when the
  guard fires; otherwise the summary reflects oracle strength.

## Acceptance Examples

### ReturnValue blind (must downgrade)

```
probe family:  return_value
expression:    base * 2
test:          assert!(compute_score(3) > 0);
Before fix:    exposed / confidence 1.0
After fix:     weakly_exposed / confidence 0.92 / observation_unverified
```

### ReturnValue confirmed (must stay exposed)

```
probe family:  return_value
expression:    x * SCALE_FACTOR
test:          assert_eq!(compute_score(3), 3 * SCALE_FACTOR);
After fix:     exposed / confidence 1.0 (SCALE_FACTOR token_match fires)
```

### MatchArm sibling-qualifier blind (Part B, must downgrade)

```
probe family:  match_arm
expression:    Mode::Frozen => -1,
test:          assert_eq!(classify(Mode::Warm), 1);
Before fix:    exposed / confidence 1.0
After fix:     weakly_exposed / confidence 0.92 / observation_unverified
```

### SideEffect / CallDeletion blind (must downgrade)

```
probe family:  call_deletion (effect)
expression:    notifier.send(order_id)
test:          assert!(result);              // plain assert, no mock, no token
After fix:     weakly_exposed / observation_unverified
```

### SideEffect / CallDeletion confirmed (Part C, must stay exposed)

```
probe family:  call_deletion (effect)
expression:    notifier.send(order_id)
test:          assert_eq!(*notifier.sent.borrow(), vec!["order-42".to_string()]);
After fix:     exposed   (strong persisted-state observer; token_match on
                          `notifier`; effect observer kind-matches the seam)
```

A bare Medium mock observer (`mock.verify();`, no token) clears
`observation_unverified` via Part C but remains `weakly_exposed` because Medium
strength maps to a weak discriminator — see Non-Goals.

## Test Mapping

- `crates/ripr/src/analysis/classify/reveal.rs` unit tests for all new families.
- 9 new golden fixtures (see traceability.toml for the full list).
- `crates/ripr/src/analysis/classifier.rs` — 2 existing tests updated.

## Implementation Mapping

- `crates/ripr/src/analysis/classify/reveal.rs`:
  - `needs_token_confirmation(family)` new predicate.
  - `match_arm_variant_tokens(expression)` new helper for Part B.
  - `is_effect_family(family)` + `effect_observer_confirms(assertion)` new
    helpers for Part C.
  - `RevealAssertionAnalysis.observation_unverified` (renamed).
  - `analyze_related_assertions` computes `observation_confirmed =
    has_token_match || (is_effect_family && effect_observer_confirms)`.
  - `assertion_matches_probe_detail` receives `match_arm_variants` param.
  - `build_discriminate_evidence` updated message.

## CI Proof

- `cargo xtask goldens check`
- `cargo test -p ripr`

## Metrics

- `observation_unverified_downgrades_non_match_arm_to_weakly_exposed`: each
  blind fixture produces `weakly_exposed` with `observation_unverified` in the
  discriminate summary.

No tier change required. Honesty narrowing within the existing `usable alpha` Rust exposure loop.
