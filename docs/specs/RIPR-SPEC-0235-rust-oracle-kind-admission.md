# RIPR-SPEC-0235: Rust oracle kind and strength admission

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

- #5513 (observer substrings grant medium `mock_expectation` credit)

Linked PRs:

- #5410 (stop crediting unguarded wildcard assertions)
- #5416 (unknown, not a gap: rule 3 reads the nearest oracle's strength)

Support-tier impact:

- No tier change. The `oracle_kind` and `oracle_strength` that ripr reports
  for a Rust related test stop overstating what the assertion pins. No finding
  moves to a stronger class (gains credit) from this spec. Under the #5416
  unknown-not-a-gap rule 3, a finding whose only strong oracle this spec
  weakens may move from `static_unknown` back to its named gap. Claim
  boundaries remain governed by [support tiers](../status/SUPPORT_TIERS.md).

Policy impact:

- Register this spec in `policy/doc-artifacts.toml` and
  `.ripr/traceability.toml`.
- No schema version bump. Kind and strength values are unchanged; only which
  assertions earn them.

## Problem

Every Rust related test carries an `oracle_kind` and `oracle_strength`, shown
in JSON, repair cards and the "Strong oracle found" discriminate summary.
They are decided by one precedence chain of substring checks
(`analysis/extract/oracles/classify.rs`, `classify_assertion`, and
`patterns.rs`). No spec defines that chain. Several links overstate the
oracle. Measured on origin/main e177461 with `ripr check --diff` on a
one-function crate. The oracle code is unchanged through cd5f0d473 except
for #5410 (de56e085a, the wildcard pre-check below), which affects no row:

| Test assertion | Reported kind / strength | What it pins |
| --- | --- | --- |
| `assert_ne!(score(2), 0)` | `exact_value` / strong | only that the result is not 0 |
| `assert_ne!(check(20), Ok(20))` | `exact_value` / strong | only that the result is not `Ok(20)` |
| `assert!(matches!(check(20), Err(e) if !e.is_empty()))` | `exact_error_variant` / strong | any non-empty error |
| `let e = String::from("big"); assert_eq!(check(20), Err(e))` | `exact_error_variant` / strong | the exact error value (correct) |
| `assert!(score(2).to_string().len() == 1 \|\| is_present())` | `mock_expectation` / medium | nothing about an effect: "sent" in `is_present` |

None of these reads `exposed` on main, because reveal still needs a token or
owner-pin confirmation. The strength still matters:

- the predicate finding prints "Strong oracle found: exact value or pattern
  assertion" for the `assert_ne!` test;
- the #5416 unknown-not-a-gap rule 3 withholds a `weakly_exposed` gap as
  `static_unknown` when the nearest tests hold a **strong** oracle. A test
  whose only assertion is `assert_ne!(score(2), 0)` therefore turns a real
  gap into "unknown" once #5416 lands;
- `mock_expectation` medium credit lets an effect family confirm on an
  unrelated identifier.

The other overstatements in the chain:

- `whole_object_equality` / strong for any `assert_eq!` or `assert_ne!` line
  containing `{` (governed by RIPR-SPEC-0225);
- `matches!` and `assert_matches!` as `exact_value` / strong whatever the
  pattern, except `Err(..)` patterns, which steps 1 and 2 decide (wildcards
  are #5410);
- a custom helper is `exact_value` / strong when its name contains `_eq`,
  `_equal` or `_matches` and it has two arguments (one for a `.assert_*`
  method call), or when its name ends in `eq`, `equal` or `matches`, so
  `assert_not_equal(a, b)` qualifies;
- `smoke_only` and `broad_error` are decided by substrings (`is_ok`,
  `is_err`), so an identifier such as `is_okay` or `this_errs` qualifies.

## Behavior

### One authority

`classify_assertion` stays the single classifier for Rust oracle kind and
strength. The only later adjustment is the RIPR-SPEC-0106 upgrade in
`scan.rs`, which turns an `exact_value` or `whole_object_equality` assertion
on an `unwrap_err`-bound variable into `exact_error_variant`. Kind is decided
from the assertion's operand text after message arguments are removed
(`assertion_oracle_text`), never from a format string or a diagnostic
argument, for the macros `assertion_oracle_text` recognizes; a custom helper
is read from its whole line. The RIPR-SPEC-0106 upgrade never applies to an
assertion that an admission rule below weakened.

### Precedence

The chain runs in this order; the first match wins. Before it, the
wildcard pre-check (#5410) assigns `relational_check` / weak to an
`assert!`, `debug_assert!` or `ensure!` whose whole condition is
`matches!(x, _)`, and to an `assert_matches!` or `debug_assert_matches!`
whose pattern is a whole unguarded `_`. Admission rule 3 widens this
pre-check to every whole irrefutable pattern, guarded or not.

0. an `ensure!` condition runs its own sub-chain
   (`classify_fallible_assertion`): `exact_error_variant` / strong, then
   `broad_error` / weak, then an exact comparison that is not duplicative as
   `exact_value` / strong, then `smoke_only` / smoke, otherwise
   `relational_check` / weak
1. `exact_error_variant` / strong
2. `broad_error` / weak
3. duplicative equality (the same expression on both sides) as
   `relational_check` / weak
4. `whole_object_equality` / strong
5. `exact_value` / strong
6. `snapshot` / medium (known `insta` and `expect_test` forms only)
7. `smoke_only` / smoke
8. scalar path-versus-integer relation as `relational_check` / weak
9. `mock_expectation` / medium
10. exact custom helper as `exact_value` / strong
11. other custom helper as `unknown`
12. `<` or `>` anywhere in the operand text, `is_empty`, `contains` or a
    bare `assert!` as `relational_check` / weak
13. otherwise `unknown`

This order is the existing behavior and is normative. The admission rules
below change what a step admits. A rule that takes an assertion out of a
step assigns the kind it states at that step's position, so the assertion
does not fall through to a later step by accident. No rule raises a
strength.

### Admission rules

Two pattern terms are used below.

- An **irrefutable pattern** accepts every value of its type: `_`; a
  catch-all binding (a lowercase-initial identifier other than `true` and
  `false`, including `_name`, optionally with `ref`, `mut` or `ref mut`);
  `name @ p`, `&p` or `box p` where `p` is irrefutable; a tuple pattern,
  or a struct or tuple-struct pattern whose path is shown to name a struct
  type, whose every sub-pattern is irrefutable or `..` (`(_, _)`,
  `S { .. }`); the sole variant of an enum shown to have one variant,
  with irrefutable sub-patterns; the slice patterns `[..]` and `[name @ ..]` (other slice
  patterns check a length); and an or-pattern whose alternatives
  together cover every constructor of the type (`Some(_) | None`,
  `Ok(_) | Err(_)`). `name @ E::X` and `name @ Some(_)` are read through
  their sub-pattern.
  When a path cannot be shown to name a struct (for example `Cfg { .. }`
  under a glob `use` of an enum), it is read as a variant and is not
  irrefutable.
- A **side-only pattern** checks only which side of an `Option` or `Result`
  the value is on: `Some(p)`, `Ok(p)` or `Err(p)` where every payload
  sub-pattern is irrefutable, `..` or itself side-only (`Some(Ok(_))`), and
  `None` (RIPR-SPEC-0227). `&p`, `box p` and `name @ p` are side-only when
  `p` is.

1. **Inequality is never exact.** At steps 0, 1, 5 and 10, an inequality
   assigns `relational_check` / weak, whatever its operands: `assert_ne!`,
   `!=` in an `ensure!` condition, a negated `!matches!(..)` (which today
   reads `exact_error_variant` / strong at step 1 when its pattern is
   `Err(E::X)`), and a custom helper whose name has a `ne`, `not` or `neq`
   segment that step 10 admits today. At step 4, every `assert_ne!` assigns
   weak strength: a struct-literal operand keeps the `whole_object_equality`
   kind, as RIPR-SPEC-0225 says, with no field credit; any other `{` (a
   closure or block operand) assigns `relational_check`. A negated pattern
   assertion takes rule 1 before rules 2 and 3.
2. **Patterns with bindings are not variant pins.** At steps 0 and 1, a
   `matches!` or `assert_matches!` whose `Err(..)` inner pattern is
   irrefutable, `..` or pins no value (rule 3's test) assigns
   `broad_error` / weak, guard or not.
   `Err(E::X)`, `Err(E::X(..))` and `Err(E::X { .. })` stay
   `exact_error_variant`. `assert_eq!` against `Err(value)` with any
   expression stays `exact_error_variant`, because equality pins the value.
3. **Pattern assertions follow their pattern.** Before step 0, with the
   wildcard pre-check, a `matches!` or `assert_matches!` whose whole
   pattern is irrefutable assigns `relational_check` / weak, guard or not,
   so `Ok(_) | Err(_)` never reaches step 2 as a result-side oracle that
   RIPR-SPEC-0227 rules 3 and 3b could credit. Rule 2 takes precedence
   over the rest of this rule for an `Err(..)` pattern it covers, so
   `Err(_)` and a guarded `Err(e)` stay `broad_error` / weak. At steps 0,
   1 and 5, any other side-only pattern assigns `smoke_only` / smoke when
   unguarded, because it only checks the side, and `relational_check` /
   weak with a guard; a
   range pattern, or a constructor whose payload is a range (`1..=5`,
   `Some(1..=5)`), assigns `relational_check` / weak, and an `Err` range
   payload (`Err(1..=5)`) assigns `broad_error` / weak. At steps 0, 1 and
   5, an or-pattern that is not irrefutable but has alternatives on both
   sides (`Ok` and `Err`, or `Some` and `None`) assigns `relational_check` /
   weak, because it does not observe the side (`Ok(_) | Err(E::X)`); any
   other or-pattern reads as its weakest alternative. Any other pattern
   stays `exact_value` only when it pins a value: it contains a literal
   outside a range, a constant, or a variant of an enum shown to have more
   than one variant (the `Option` and `Result` constructors are read as
   side-only above). A pattern that pins no value assigns
   `relational_check` / weak: a slice that only fixes a length (`[_, ..]`,
   `[_]`), or the sole variant of a single-variant enum (`Only::Value`,
   `Only::Value(_)`). A variant whose enum cannot be resolved counts as
   pinning, which keeps today's reading.
4. **Method checks match whole method names.** At steps 0 and 7, `is_ok`,
   `is_some` and `is_none` count only as a method-call segment (`.is_ok(`,
   `Option::is_some(`), not as a substring of another identifier such as
   `is_okay`. The combinator forms `.is_some_and(`, `.is_ok_and(`,
   `.is_err_and(` and `.is_none_or(` count as the same segment. `.unwrap(`
   and `.expect(` already match this way. At step 2, `is_err` matches the
   same way, so `this_errs` is not a broad error. A condition that tests both sides
   (`is_ok()` with `is_err()`, or `is_some()` with `is_none()`, joined by
   `||`) assigns `relational_check` / weak at steps 0, 2 and 7, because it
   accepts every value.
5. **Effect observer words match whole identifier segments.** At step 9,
   `event`, `emitted`, `published`, `sent`, `saved`, `persist`, `state`,
   `stored`, `metric`, `counter` and `recorded` count only as a whole
   `_`-separated or case-split segment of an identifier, with an optional
   trailing `s` (`events_sent`, `sentCount`, `events`, `state`), never inside
   another word (`present`, `statement`, `consent`). The segment must sit in
   the asserted subject, not only in an unrelated call; today any operand
   text counts, including a call such as `is_present()`. The `mock` and
   `expect_` call checks keep their current form.
6. **Custom helpers by name are strong only for equality names.** Step 10
   needs a name whose last `_`-segment is `eq`, `equal` or `equals`, or
   which ends in `_eq` / `_equal` / `_equals` / `_matches`, with no `ne`,
   `not` or `neq` segment, and at least two arguments (one for a
   `.assert_*` method call). An inequality-named helper is rule 1's case.
   Any other helper is step 11 `unknown`.

### Decisions

Steven delegated these choices on 2026-10-04 ("make reasonable documented
decisions and proceed"). Each records the adopted option, why, and the
rejected alternative. Any can be reversed later without touching the rest.

1. **`assert_ne!` strength.** Adopted: `relational_check` / weak, as rule
   1, and weak `whole_object_equality` for a struct literal. Rejected: a
   new `inequality` kind, because that is a schema addition for a reading
   that is already weak.
2. **Observer words.** Adopted: whole segments as rule 5, because names
   such as `events_sent` do observe an effect and keep their medium
   credit. Rejected: drop step 9's observer words entirely and keep only the
   mock call forms.

## Required Evidence

- Each row of the Problem table, and each overstatement listed after it,
  reads the kind and strength the rules give, in JSON `related_tests[]`.
- `assert_eq!(check(20), Err(e))` with a bound value stays
  `exact_error_variant` / strong.
- Whole-segment observer names (example 10) still earn `mock_expectation`,
  and `is_present()` (example 8) does not.
- No finding moves to a stronger class. Golden drift lists every related
  test whose kind or strength moved, and every finding that moved from
  `static_unknown` back to a named gap.
- Under #5416, a test whose only assertion is `assert_ne!` no longer
  withholds a gap as `static_unknown`.

## Non-Goals

- No new oracle kind or strength value.
- No resolution of custom helper bodies (RIPR-SPEC-0120 owns macro-wrapped
  assertions).
- No change to reveal's token or owner-pin confirmation.
- No change to TypeScript or Python oracle classification.

## Acceptance Examples

`score(x) = x * 2` (changed from `x * 3`); `check(x)` returns `Err` above 10.

1. `assert_ne!(score(2), 0)`: `relational_check` / weak.
2. `assert_ne!(check(20), Ok(20))`: `relational_check` / weak.
3. `assert!(matches!(check(20), Err(e) if !e.is_empty()))`: `broad_error` /
   weak.
4. `assert!(matches!(check(20), Err(_)))`: `broad_error` / weak (unchanged).
5. `assert_eq!(check(20), Err(e))`: `exact_error_variant` / strong (unchanged).
6. `assert!(matches!(check(5), Ok(5)))`: `exact_value` / strong (unchanged).
7. `assert!(matches!(check(5), Ok(_)))`: `smoke_only` / smoke.
8. `assert!(is_present())`: not `mock_expectation`.
9. `assert_eq!(events_sent.len(), 1)`: `exact_value` / strong (unchanged;
   step 5 wins before step 9).
10. `assert!(events_sent.contains(&id))`: `mock_expectation` / medium
    (unchanged).
11. `assert_not_equal(score(2), 0)`: `relational_check` / weak.
12. `assert_json_eq(actual, expected)`: `exact_value` / strong (unchanged).
13. `ensure!(score(2) != 0)`: `relational_check` / weak (today
    `exact_value` / strong).
14. `assert_ne!(build(3), Config { retries: 9 })`: `whole_object_equality` /
    weak.
15. `assert!(opt.is_some_and(|v| v > 1))`: `smoke_only` / smoke (unchanged).
16. `assert!(!events.is_empty())`: `mock_expectation` / medium (unchanged).
17. `assert!(matches!(check(20), Err(_e)))`: `broad_error` / weak (today
    `exact_value` / strong).
18. `ensure!(matches!(check(5), Ok(_)))`: `smoke_only` / smoke (today
    `exact_value` / strong).
19. `assert!(!matches!(check(20), Err(E::Bad)))`: `relational_check` / weak
    (today `exact_error_variant` / strong).
20. `assert_ne!(check(20), Err(E::Bad))`: `relational_check` / weak (today
    `exact_value` / strong at step 5).
21. `assert!(matches!(check(20), Err(ref e) if e.len() > 1))`, `Err(mut e)`
    and `Err(e @ _)`: `broad_error` / weak.
22. `assert!(matches!(check(20), Err(e @ E::Bad)))`: `exact_error_variant` /
    strong (unchanged).
23. `assert!(matches!(lookup(1), Some(ref x)))` and `Ok(x @ _)`:
    `smoke_only` / smoke (today `exact_value` / strong);
    `assert!(matches!(lookup(1), Some(x @ 3)))` stays `exact_value`.
24. `assert!(matches!(lookup(1), Some(_) | None))`: `relational_check` / weak
    (today `exact_value` / strong at step 5).
    `assert!(matches!(check(5), Ok(_) | Err(_)))`: `relational_check` /
    weak (today `broad_error` / weak, which RIPR-SPEC-0227 could credit).
    `assert!(matches!(pair(), (_, _)))` and `matches!(cfg(), Cfg { .. })`:
    `relational_check` / weak (today `exact_value` / strong).
25. `assert!(matches!(lookup(1), Some(1..=5)))`: `relational_check` / weak;
    `assert!(matches!(parse(), Some(Ok(_))))` and `&Some(_)`: `smoke_only` /
    smoke; `assert!(matches!(items(), [_, ..]))` and, for
    `enum Only { Value }`, `assert!(matches!(make(), Only::Value))`:
    `relational_check` / weak, because neither pins a value;
    `assert!(matches!(items(), [1, ..]))` stays `exact_value`; all read
    `exact_value` / strong today. `assert!(matches!(check(20), Ok(_) | Err(E::Bad)))`:
    `relational_check` / weak (today `exact_error_variant` / strong).
    `assert!(matches!(lookup(1), Some(3) | Some(_)))`: `smoke_only` / smoke
    (weakest alternative).
26. `assert!(r.is_ok() || r.is_err())`: `relational_check` / weak, because
    it accepts every value (today `broad_error` / weak, because step 2
    finds `is_err`). `assert!(!matches!(check(5), Ok(_)))`:
    `relational_check` / weak under rule 1, not rule 3's smoke.
    `assert!(matches!(items(), [..]))` and
    `assert!(matches!(items(), [rest @ ..]))`: `relational_check` / weak
    through the widened pre-check (today `exact_value` / strong at step 5).

## Test Mapping

- Existing: `crates/ripr/src/analysis/extract/oracles/classify.rs` unit tests.
- Existing: `classify.rs` unit test for `ensure!(s != X)` changes with
  example 13.
- Planned: one classifier unit test per acceptance example, and a fixture
  for example 1 showing the related test's reported kind and strength.

## Implementation Mapping

- `crates/ripr/src/analysis/extract/oracles/classify.rs`: precedence chain.
- `crates/ripr/src/analysis/extract/oracles/scan.rs`: the RIPR-SPEC-0106
  upgrade, unchanged.
- `crates/ripr/src/analysis/extract/oracles/patterns.rs`: token-level
  matching for rules 1 to 6.

## Metrics

- `rust_oracle_strength_overstated`: related tests whose reported strength
  exceeds what their assertion pins, on the acceptance set; must be zero.
