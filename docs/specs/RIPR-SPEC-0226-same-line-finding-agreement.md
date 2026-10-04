# RIPR-SPEC-0226: Findings on one line agree on shared facts

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

- None yet

Linked PRs:

- None yet

Support-tier impact:

- No tier change. Findings that share a changed line and owner stop
  contradicting each other on reach, related tests and observation of the same
  call. Different probe families may still read different classes. Claim
  boundaries remain governed by [support tiers](../status/SUPPORT_TIERS.md).

Policy impact:

- Register this spec in `policy/doc-artifacts.toml` and
  `.ripr/traceability.toml`.
- No schema version bump.

## Problem

One changed line can produce several probes: parser shapes are deduplicated by
start byte and family, and the lexical fallback adds one probe per family
(`analysis/probes/diff.rs`). Related tests are found per probe
(`analysis/classify/related_tests.rs`), and every surface reports each finding
on its own: JSON, SARIF, GitHub annotations, LSP diagnostics and code lenses,
gate records and PR comments. Only the verdict-corpus harness
(RIPR-SPEC-0219) collapses a line, and that precedence is explicitly not a
product rule.

So a developer reading one line can see two answers that cannot both be true.
Committed goldens on main show it. A scan of the 337
`fixtures/*/expected/check.json` files found 32 lines whose findings differ in
class, related-test count or reach:

- `error_variant_wrapper_callee_only_pin`, line 26: the `call_deletion`
  finding says related tests reach `parse_summary` through
  `try_parse_summary_pins_malformed_source` while its `related_tests_total` is
  0, and the `error_path` finding on the same line lists that same test twice.
- The "Fill spec-defined corpus cases" ledger crate: on
  `self.history.push(amount)` the `side_effect` finding reads `exposed` and the
  `call_deletion` finding on the same call reads `weakly_exposed`. A test whose
  exact assertion on `history` sees the pushed value also sees the push
  disappear.

Some disagreement is correct. In `split_test_boundary_oracle` the predicate
`input >= 10` reads `weakly_exposed` (no same-test boundary pairing,
RIPR-SPEC-0186) while the `return_value` finding reads `exposed`. Those are
different mutants with different evidence. This spec separates facts that
belong to the line or the call, which must agree, from evidence that belongs
to the family, which may differ.

## Behavior

### Within one finding

1. A finding whose reach summary names a related test has
   `related_tests_total` of at least 1 and lists that test. (RIPR-SPEC-0219
   already counts the violation as `reach_yes_without_related_tests`.)
2. `related_tests` holds each test identity at most once.

### Across findings with the same file, changed line and owner

3. **Reach agrees.** If any finding reports reach `yes` through test `T`, no
   other finding reports `no_static_path`, and none says no related tests were
   found, unless its own evidence names a family-specific reason (for example
   the `wrapper_seam_callee` limit, which applies only to `error_path` and
   `return_value`). The reason appears in that finding's text.

   The related-test sets themselves are equal across these findings, keyed by
   test identity (file, module path and test function name), not by oracle row: two
   findings that list the same test with different oracle kinds or
   strengths agree on the set (rule 5 governs those differences). The sets
   may differ only for tests a family-specific rule adds or drops; each such difference is
   named in the finding whose set differs. One finding listing `{T}` and
   another listing `{U}` with no named rule is a contradiction.
4. **Call observation agrees.** For a `call_deletion` finding and a
   `side_effect` finding on the same call expression, the side effect's
   confirmation carries to the deletion only when deleting the call provably
   changes what the oracle sees:
   - for an exact oracle on the written state (sink `state_write`), the
     receiver's pre-call state is provably known in that test (bound from a
     literal, `new()` or `default()` with no earlier write) and the asserted
     post-call state differs from it, so deleting the call fails the
     assertion; and nothing between the call and the assertion can write
     the receiver (no mutating call on, assignment to, or `&mut` borrow of
     it, and no alias of it such as an `Rc` clone or a shared reference with
     interior mutability); an idempotent write (`insert` of a present key, `clear` of an
     empty collection) or an unknown pre-call state does not carry;
   - for a mock expectation, it carries an exact call count (`times(n)` with
     `n` at least 1); an expectation that allows zero calls does not carry.

   Otherwise the `call_deletion` finding keeps its own evidence, and its text
   says the deletion is not established by the side-effect oracle. The
   reverse never carries: observing that the call happened does not observe
   its argument.
5. **Family evidence may differ.** Classes may differ across families when
   each finding's discriminate or infect evidence says why (pairing, missing
   discriminator, oracle kind). Every finding stays in every per-finding
   surface.

### Decisions for the owner

1. **Line verdict in human output and LSP hover.** Recommended: when findings
   on one line read different classes, human output and LSP hover print one
   line-level summary that uses the RIPR-SPEC-0219 precedence (gap over
   credit over limit), followed by each finding. JSON, SARIF, annotations and
   gates stay per finding. Alternative: no line-level summary anywhere.
2. **Catch-all beside a typed finding.** When a lexical `static_unknown`
   finding shares the exact expression span of a typed finding with a
   definite class, recommended: keep it on every per-finding surface (JSON,
   SARIF, annotations, diagnostics, gates, PR comments), as rule 5 requires,
   and drop it only from human output and LSP hover, which are not
   per-finding surfaces, because it adds no evidence the typed finding
   lacks. Alternative: keep it everywhere.

## Required Evidence

- A contradiction check over all committed goldens that fails on rules 1 to 4.
  The 32 lines found on main are re-blessed or fixed, each listed with its
  rule.
- `error_variant_wrapper_callee_only_pin` line 26 satisfies rules 1 to 3.
- A side-effect and call-deletion case on one `push` call with an exact
  assertion on the pushed collection reads the same confirmation for both
  findings.
- `split_test_boundary_oracle` still reads `weakly_exposed` for the predicate
  and `exposed` for the return value (rule 5).

## Non-Goals

- No merging of findings into one.
- No change to triage "Start here" ranking (RIPR-SPEC-0122).
- No change to probe extraction or deduplication.
- No change to gate semantics: any blocking record still fails the run.

## Acceptance Examples

1. `pub fn gate(input: u32) -> bool { input >= 10 }`, changed from `input > 10`,
   tests `boundary()` calling `gate(10)` and `gate(9)` without asserting and
   `far()` asserting `assert_eq!(gate(100), true)`: predicate
   `weakly_exposed`, return value `exposed`, both with related tests
   {`boundary`, `far`}.
2. `fn record(&mut self, amount: u64) { self.history.push(amount); }`, the
   argument changed, test `l.record(5); assert_eq!(l.history, vec![5])`:
   `side_effect` and `call_deletion` both confirmed.
   Here `l` is built by `Ledger::default()` with nothing pushed before. A
   control that pushes `5` before calling `record(5)` and then asserts only
   `assert!(l.history.contains(&5))` does not carry: the `side_effect`
   finding may stand on its own evidence, but the `call_deletion` finding is
   not confirmed by it.
3. `try_parse_summary(raw).map_err(Into::into)` in `parse_summary`, test
   `try_parse_summary_pins_malformed_source` matching
   `Err(ParseSummaryError::MalformedSource)`: no finding names that test in
   reach while reporting zero related tests, and no `related_tests` list
   repeats it.
4. `if a > b { return x + 1 }`, test `t` calling `f(2, 1)` and asserting the
   result: the predicate finding is never `no_static_path` while the
   return-value finding reports reach through `t`.

## Test Mapping

- Existing: `fixtures/split_test_boundary_oracle`,
  `fixtures/error_variant_wrapper_callee_only_pin`.
- Planned: a goldens-wide contradiction check in `xtask` reusing the
  verdict-corpus contradiction codes.
- Planned: a fixture or verdict-corpus case for acceptance example 2.

## Implementation Mapping

- `crates/ripr/src/analysis/classify/related_tests.rs`: deduplicate test
  identities; share line-level reach.
- `crates/ripr/src/analysis/classify/reveal.rs` and
  `crates/ripr/src/analysis/classify/flow.rs`: share state-write observation
  between `side_effect` and `call_deletion` on one call.
- `xtask/src/reports/verdict_corpus.rs`: contradiction codes reused by the
  goldens check.

## Metrics

- `same_line_reach_contradictions`: must be zero across committed goldens.
- `duplicate_related_test_entries`: must be zero.
- `same_call_effect_deletion_disagreements`: must be zero.
