# RIPR-SPEC-0229: Match-arm selection names the unselected arm

Status: proposed

Owner: product / analysis

Created: 2026-10-04

Linked proposal:

- None yet

Linked ADRs:

- None yet

Linked plan:

- None yet

Linked issues:

- #5432 (unknown-not-a-gap rule 3 withholds the canonical sibling-arm gap)

Linked PRs:

- #5416 (unknown, not a gap: rule 3 `rust_oracle_target_unresolved`)

Support-tier impact:

- No tier change. A Rust `match_arm` finding whose related tests provably
  select a different arm keeps its gap and names the arm that no test selects.
  A finding whose related test provably selects the changed arm and pins the
  result exactly can read `exposed`. Claim boundaries remain governed by
  [support tiers](../status/SUPPORT_TIERS.md).

Policy impact:

- Register this spec in `policy/doc-artifacts.toml` and
  `.ripr/traceability.toml`.
- No schema version bump. The missing discriminator reuses the existing
  `MissingDiscriminatorFact` (`value`, `reason`) and the existing
  `Missing discriminator value:` entry in `Finding.missing`.

## Problem

ripr never decides which arm a test input selects. For a changed `match_arm`
probe, reveal confirms only when an identifier after `::` in the probe text
(the arm pattern, plus the body on the lexical path) appears somewhere in an assertion, or when a string-literal arm equals a
direct owner-call argument (`analysis/classify/reveal.rs`,
`match_arm_variant_tokens` and `owner_call_literals`). Activation produces no
missing discriminator for the `match_arm` family
(`analysis/classify/activation.rs`, `missing_discriminator_facts` handles
predicate, error path and field construction only). Observed owner arguments
lose their wrapper, so `reason(Some(5))` is recorded as `x = 5`.

Three wrong outcomes follow:

1. **Correct gaps abstain.** `fixtures/match_arm_blind` has one test,
   `assert_eq!(reason(Some(5)), 6)`, and an added `None => 0` arm. No test
   selects `None`. Under #5416 rule 3 a strong nearest oracle with no named
   missing discriminator is withheld as `static_unknown`
   (`rust_oracle_target_unresolved`), so a real gap reads as an analyzer limit.
   #5432 counts 24 findings across the `match_arm_*`,
   `error_path_diagnostic_*` and `owner_return_pin_identity_traps` fixtures
   on the same route.
2. **Selected arms never credit.** `assert_eq!(reason(None), 0)` pins the
   changed arm exactly and still reads `weakly_exposed`, because `None` has no
   `::` token. Option and Result constructors, integer, char and bool
   literals, ranges, bindings, tuples and `_` never confirm, whatever the
   input. RIPR-SPEC-0093 records this as an accepted failure mode.
3. **An expected-side token confirms a sibling arm.** For
   `match k { Kind::A => Kind::B, Kind::B => Kind::A }` and the test
   `assert_eq!(f(Kind::A), Kind::B)`, a change to the `Kind::B =>` arm is
   confirmed by the `B` on the expected side, although the input selects the
   `Kind::A =>` arm.

## Behavior

### Selection facts

For a changed `match_arm` probe, activation collects every related test's
owner calls whose scrutinee is a parameter of the owner, not shadowed or
reassigned before the `match`. Each call is either resolved (its argument is
statically resolved, below) or unresolved; an unresolved call is kept as a
blocker, never dropped. A resolved call **provably selects** an arm when, in
source order,
every earlier arm provably does not match the argument and that arm provably
does.

Provable matching is defined for this pattern grammar and no other:

- integer, char, bool and string literals;
- integer ranges with literal bounds (`a..=b`, `a..b`, `..=b`, `a..`);
- enum variant paths, qualified (`Kind::A`) or bare imported, including the
  prelude constructors `Some`, `None`, `Ok` and `Err`;
- tuple-variant and tuple patterns whose subpatterns are in this grammar,
  wildcards (`_`) or bare bindings;
- or-patterns whose alternatives are all in this grammar;
- `name @ subpattern`, which matches exactly when `subpattern` does;
- a top-level `_` or bare identifier binding, which matches every argument.
  A binding with an `@` subpattern is not a catch-all.

An arm with a guard (`pat if cond`) is not provably matched or provably
unmatched. When a guarded arm comes before or is the changed arm, the call
does not provably select any arm at or after the guard.

A resolved argument is a literal, an enum path or a constructor call of the
grammar above written in the test call itself, or an immutable `let` binding
in the same test whose initializer is one and which is not shadowed before
the call. A `let mut` binding, or one passed as `&mut` before the call, is
unresolved. Arguments from helpers, fixtures, loops,
parameters or other calls are unresolved.

### Naming the unselected arm

When every related test call to the owner is resolved and none provably
selects the changed arm, activation emits one missing discriminator:

- `value`: the changed arm's pattern text, without the guard and `=>`
  (for example `None`).
- `reason`: names the arm and the observed values, for example
  "No related test call selects arm `None =>`; observed `x` values:
  `Some(5)`."

Observed values keep their constructor (`Some(5)`, not `5`).

The finding keeps its gap (`weakly_exposed` when a related test holds an
oracle on the result, `reachable_unrevealed` when none does). Because a
missing discriminator is named, the #5416 unknown-not-a-gap rule 3 does not
withhold it.

When the change is to the arm's pattern rather than its body, the arm is
named only when no resolved call selects it under either the original or
the changed pattern: an input that moved between arms is a discriminator.

When any related call is unresolved, or a guard blocks the decision, no
missing discriminator is named. Rule 3 then applies unchanged.

### Crediting the selected arm

When a related test call provably selects the changed arm and the same test
holds an admitted exact oracle on that call's result (the call itself as an
`assert_eq!` operand, or a `let` binding of the call that a later exact
assertion observes, as in RIPR-SPEC-0186 pairing), discrimination is
confirmed and the finding may read `exposed`. Selection replaces token
matching as the confirmation for that test.

This holds for a change to the arm's body. For a change to the arm's
pattern, selecting the changed arm is not enough, because an input inside
both the original and the changed pattern runs the same arm either way.
Credit then needs a resolved call whose selected arm differs between the
original and the changed `match`, two arms whose results are provably
unequal values, and a same-test exact oracle on that call's result. Results
are provably unequal only when both are literals, or enum paths or
constructors of literals in the grammar above, with different values. Any
other result expression (`1 + 1` against `2`, a call, a binding) gives no
credit, because different text can compute the same value.

### Selection outranks tokens

When every related call is resolved and none selects the changed arm, a
variant token in an assertion does not confirm the arm. This closes the
expected-side confirmation in Problem item 3. When any related call is
unresolved, token matching does not confirm the arm either: only a resolved
selecting call with an exact oracle confirms, and otherwise the finding
stays below `exposed` (under the #5416 unknown-not-a-gap rule 3 it reads
`static_unknown` when its nearest oracle is strong).

### Decisions

Steven delegated these choices on 2026-10-04 ("make reasonable documented
decisions and proceed"). Each records the adopted option, why, and the
rejected alternative. Any can be reversed later without touching the rest.

1. **Pattern grammar.** Adopted: the list above, including ranges and
   bare imported variants, because each is decidable from the pattern's
   syntax, the file's `use` imports and the argument literal alone. Rejected, narrower: literals and
   qualified enum paths only.
2. **`_` and binding arms by first-match.** Adopted: a trailing `_` is
   selected when every earlier arm provably does not match, so a change to
   `_ => 2` with only `f(Kind::A)` (selecting `Kind::A =>`) names `_` as
   missing. Rejected: never name `_`.
3. **Mixed resolved and unresolved calls.** Adopted: any unresolved call
   blocks the "no call selects" claim (fail closed), because the unresolved
   input may select the arm. Rejected: name the arm from the resolved calls
   and disclose the unresolved ones.
4. **Selected-arm credit.** Adopted: credit (`exposed`) as written above,
   because an input that provably selects the changed arm, paired with an
   exact oracle on that call's result, is the discriminator the change
   needs. Rejected: name the selection in the discriminate summary but keep the
   existing token rule for credit.

## Required Evidence

- `fixtures/match_arm_blind` reads `weakly_exposed` with a missing
  discriminator whose `value` is `None`, on main and with the #5416
  unknown-not-a-gap rules applied.
- A selected-arm control (`assert_eq!(reason(None), 0)`) reads `exposed`.
- An unresolved-input control reads no named missing discriminator; with
  the #5416 unknown-not-a-gap rules applied it reads `static_unknown` with
  `rust_oracle_target_unresolved`.
- A guarded-arm control reads no named missing discriminator.
- The expected-side trap reads below `exposed`.
- Golden drift on the #5432 list is reviewed row by row: each finding that
  leaves `static_unknown` names its unselected arm, and none gains `exposed`
  without a selecting call and an exact oracle in one test.

## Non-Goals

- No guard evaluation.
- No scrutinee that is not a direct owner parameter (fields, method results,
  computed tuples).
- No exhaustiveness checking beyond source-order first match.
- No change to RIPR-SPEC-0164 helper transfer, which keeps its own refusals.
- No new probe family, class, `static_limit_kind` or schema field.

## Acceptance Examples

Source for 1 to 3:
`fn reason(x: Option<i32>) -> i32 { match x { Some(v) => v + 1, None => 0 } }`,
the diff changes `None => 1` to `None => 0`.

1. Test `assert_eq!(reason(Some(5)), 6)`: `weakly_exposed`; missing
   discriminator `value` `None`, reason names arm `None =>` and observed
   `Some(5)`.
2. Test `assert_eq!(reason(None), 0)`: `exposed`.
3. Test `let x = make(); assert_eq!(reason(x), 6)`: no named missing
   discriminator; `static_unknown` / `rust_oracle_target_unresolved` under
   the #5416 unknown-not-a-gap rule 3.
4. `match k { Kind::A => 1, _ => 2 }`, the diff changes `_ => 0` to `_ => 2`, test
   `assert_eq!(f(Kind::A), 1)`: `weakly_exposed`; missing discriminator
   `value` `_`.
5. `match x { Some(v) if v > 4 => 1, _ => 0 }`, the diff changes the guard
   from `v > 3`, test `assert_eq!(f(Some(5)), 1)`: no named missing discriminator.
6. `match k { Kind::A => Kind::B, Kind::B => Kind::A }`, arm `Kind::B =>`
   changed, test `assert_eq!(f(Kind::A), Kind::B)`: `weakly_exposed`; missing
   discriminator `value` `Kind::B`.
7. `match n { 0 => "zero", 1..=9 => "digit", _ => "many" }`, arm
   `1..=9 =>` changed, tests `f(0)` and `f(42)` with exact assertions:
   `weakly_exposed`; missing discriminator `value` `1..=9`.
8. Same `match`, the diff changes the pattern `1..=8` to `1..=9`. Test
   `assert_eq!(f(5), "digit")`: not `exposed` (5 selects the arm under both
   patterns) and no missing discriminator is named, so with a strong oracle
   the #5416 unknown-not-a-gap rule 3 reads it `static_unknown`. Naming the
   boundary input (`9`) as missing, as RIPR-SPEC-0186 does for predicates,
   is a follow-up.
   Test `assert_eq!(f(9), "digit")`: `exposed` (9 moved from `_` to the
   changed arm).
9. `match n { 0..=4 => 1 + 1, _ => 2 }`, the diff changes the pattern
   `0..=3` to `0..=4`. Test `assert_eq!(f(4), 2)`: not `exposed` (4 moved
   arms, but both arms return 2).

## Test Mapping

- `fixtures/match_arm_blind` (example 1, with the arm added rather than
  changed; the expected result is the same)
- Planned: one fixture or verdict-corpus case per acceptance example 2 to 9.
- Planned: `crates/ripr/src/analysis/classify/activation.rs` unit tests for
  each grammar element, guard blocking, first-match order and unresolved
  arguments.
- Planned: a `gap_admission` test showing rule 3 keeps the gap once the arm is
  named.

## Implementation Mapping

- `crates/ripr/src/analysis/classify/activation.rs`: selection facts and the
  `match_arm` missing discriminator; keep constructors on observed values.
- `crates/ripr/src/analysis/classify/reveal.rs`: selection confirmation and
  the selection-outranks-tokens gate.
- `crates/ripr/src/analysis/classify/gap_admission.rs` (#5416): no change;
  rule 3 already keeps a gap with a named missing discriminator.

## Metrics

- `match_arm_unselected_arm_named`: a `match_arm` finding whose resolved
  related calls all select other arms names the changed arm as missing.
- `match_arm_selected_arm_credit`: a resolved selecting call with a same-test
  exact oracle confirms the arm.
- Verdict-corpus false-actionable and false-silent counts on `match_arm`
  cases do not rise.
