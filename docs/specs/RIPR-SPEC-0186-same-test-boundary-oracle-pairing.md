# RIPR-SPEC-0186: Same-test pairing for boundary-class probes

Status: proposed

Owner: product / analysis

Created: 2026-09-29

Linked proposal:

Linked ADRs:

Linked plan:

Linked issues:

- #4828
- #5027 (shared execution admission before pairing)

Linked PRs:

Support-tier impact:

- Honesty fix only. Narrows a false `exposed` on predicate probes. No support-tier
  change. Claim boundaries remain governed by
  [support tiers](../status/SUPPORT_TIERS.md).

Policy impact:

- Register this spec in `policy/doc-artifacts.toml`.
- No schema version bump. The discriminate summary for the split case is text
  only (`same_test_pairing_missing`); not a new JSON field.

## Problem

A predicate probe can read `exposed` when no test discriminates the boundary.
One test calls the owner at the boundary without asserting. A different test
asserts exactly, but only far from the boundary:

```rust
pub fn gate(input: u32) -> bool { input >= 10 }

#[test] fn boundary() { let _ = gate(10); let _ = gate(9); }
#[test] fn far() { assert_eq!(gate(100), true); }
```

Infection credits "related test input at the changed boundary" from `boundary`.
Discrimination credits the strong oracle from `far`. Nothing requires those
signals to come from the same test, or from the same owner call. The mutant
`>=` → `>` still passes both tests.

#4486 covers a proximity-only test supplying the oracle. #4574 / #4715 cover
test-side helper credit. #4760 covers bare-name method relation. This spec is
only the split between a real boundary input and a real oracle on different
tests or different calls.

## Behavior

For a `predicate` probe, `exposed` requires one related test that both:

1. feeds a boundary input to the changed owner, and
2. holds an admitted discriminating oracle on that call's result.

For the bare equality subset in RIPR-SPEC-0197, pairing uses the same parser-backed
execution admission as reveal. A refused boundary equality cannot pair with an
admitted far-input oracle, within one test or across tests. Original assertion
cardinality is retained; a filtered clone cannot create a singleton fallback.

Otherwise the finding reads at most `weakly_exposed`. The discriminate stage
is `weak` and its summary names `same_test_pairing_missing`.

A control of the form `assert_eq!(gate(10), true)` stays `exposed`. A let-bound
owner call whose binding a later exact assertion observes (`let got = gate(10);
assert_eq!(got, true)`) also pairs. Pairing reuses activation's boundary `==`
facts, so a same-test exact oracle that already infected through a named
constant or helper hop stays paired. A same-test mix that calls the boundary
without asserting and asserts a far call (`let _ = gate(10); assert_eq!(gate(100),
true)`) does not pair.

Helper-call transfer, proximity-only oracle credit, and bare-name method
relation are out of scope.

## Required Evidence

- A fixture matching the reproduction reads below `exposed`, with
  `same_test_pairing_missing` in the discriminate summary.
- A control where one test does both (`assert_eq!(gate(10), true)`, and
  `fixtures/strong_boundary_oracle`) stays `exposed`.
- Unit tests cover split tests, same-call pairing, same-test split calls,
  same-line split calls, unused-argument literals, shadowed bindings, and
  let-bound pairing including short names.
- Golden drift is reviewed row by row: every downgrade names the missing
  same-test pairing, and no finding gains a class.
- An honesty-corpus case independently prohibits `exposed` on the split
  reproduction.

## Non-Goals

- No helper-call assertion credit (#4574, #4715).
- No change to proximity-only oracle credit (#4486).
- No change to bare-name method relation (#4760).
- No runtime mutation vocabulary.
- No schema bump.

## Acceptance Examples

- Given `boundary` calling `gate(10)` with no oracle and `far` asserting
  `gate(100) == true`, when the predicate `input >= 10` is classified, then
  the finding is at most `weakly_exposed` and names `same_test_pairing_missing`.
- Given `assert_eq!(gate(10), true)` in one test, when the same predicate is
  classified, then the finding stays `exposed`. The `strong_boundary_oracle`
  fixture remains `exposed`.
- Given `let _ = gate(10); assert_eq!(gate(100), true)` in one test, when the
  predicate is classified, then it does not pair.

## Test Mapping

- `crates/ripr/src/analysis/classify/boundary_pairing.rs`
- `fixtures/split_test_boundary_oracle`
- `fixtures/predicate_boundary_oracle_refused`
- `fixtures/predicate_boundary_oracle_admitted`
- `crates/ripr/tests/owner_pin_execution.rs::predicate_pairing_cannot_reuse_refused_boundary_equalities`

## Implementation Mapping

- `crates/ripr/src/analysis/classify/boundary_pairing.rs`: pairing authority.
- `crates/ripr/src/analysis/classifier/evidence.rs`: apply the pairing gate
  before `exposed`.
- `crates/ripr/src/analysis/classify/decision.rs`: missing evidence names the
  split.

## Metrics

- `same_test_boundary_oracle_pairing`: a predicate does not read `exposed`
  unless one test both feeds a boundary input to the owner and holds a
  discriminating oracle on that call's result.
