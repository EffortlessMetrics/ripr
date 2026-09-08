# RIPR-SPEC-0158: Bounded value transfer from exact test inputs

Status: proposed

Issue: #3295 (parent #3215; builds on #3294 / RIPR-SPEC-0157)

## Problem

After #3294 connects a changed binding to its predicate use, the #3215
equality-boundary shape still reports `missing discriminator end ==
start … observed end values: unknown` even when the related test's
exact inputs determine both operands (`input = "ab"`, `delim = 'x'` →
`end = 1`, `start = 1`). Without evaluating the short deterministic
chain that produces the operands, the analyzer cannot distinguish an
exercised boundary from a missing one.

## Behavior

- One typed value-transfer interface
  (`analysis/classify/value_transfer.rs`) evaluates a single-line `let`
  initializer over one related-test call row's exact input literals.
- The value lattice is typed (`Str`, `Char`, `Bool`, `Index`, option
  forms, `CharSeq`); every exact result carries per-step provenance
  (operation family, receiver/argument rendering, chain depth) and the
  source inputs are retained on the observed-value fact text.
- Evaluated families: `find`/`rfind` (present, absent, empty,
  multibyte), `len`, `starts_with`/`ends_with`/`contains`,
  `strip_prefix`/`strip_suffix`, `chars().next()`/`next_back()`,
  `char::len_utf8`, `map_or` with a literal default and the identity
  closure, `checked_add`/`checked_sub`, and bounded string slicing
  from exact indices.
- Failure modes are typed and named: `Unsupported` names the earliest
  edge (unknown method family, non-identity closure, `map_or_else`,
  dynamic non-input identifier, over-limit chain/literal);
  `InvalidBoundary` names a non-boundary slice index. A `None` option
  arm is consumed inside the chain by `map_or`'s default, so a bare
  not-applicable never escapes. Nothing becomes exact from token
  resemblance, an oracle expectation, or a runtime result.
- Wiring: a comparison operand resolves exactly when it is a parameter
  (the row literal) or a local binding whose live span (verified with
  the #3294 relation) covers the predicate line. Exact equality under
  any row observes the boundary: the missing-discriminator fact
  disappears, `end == start` joins the observed values, and the
  infection stage can reach `yes` at the changed boundary.
- Char literals in test-call arguments are extracted as exact inputs
  (lifetimes are not).

## Required Evidence

- The #3215 reproduction flip (`observed end values: unknown` →
  `infection yes: Detected related test input at the changed boundary`,
  `end == start` observed) and the removal experiment: without the
  evaluator call the computed-local test regresses to `unknown`.
- Table-driven family tests in `value_transfer.rs` (each family with
  positive, negative, empty, multibyte, overflow, and boundary shapes —
  different inputs produce different exact results, so no family can
  be replaced by a constant).
- The classifier wiring test
  (`activation_evidence_resolves_computed_local_boundary_operands`) and
  the end-to-end diff-analysis test.
- Golden blast radius measured: the equality-boundary fixture flips
  (the intended behavior change) and `observation_unverified_call_deletion`
  gains the additive boolean observed-value fact; `cargo xtask goldens
  check` and `cargo xtask dogfood` green otherwise.

## Required guards

- The evaluator never runs arbitrary code; only the enumerated
  families with explicitly implemented semantics.
- Scope verification is mandatory: the initializer must feed the
  predicate through the #3294 live-span relation.
- Observation/discrimination still requires the test's own oracle; an
  exact input value establishes activation/infection evidence only.
- Chain depth and literal size are bounded and the limit is disclosed
  (`Unsupported`), never silently truncated.
- Exact operand comparison uses the classifier's existing literal
  normalization (`comparable_value`): underscores are ignored and
  string quotes stripped, so a computed `"a_b"` compares equal to a
  literal `"ab"`. The normalization is pre-existing and applies
  identically to parameter operands.

## Acceptance Examples

- Accept: `end = input.rfind(delim).map_or(1, |idx| idx)` with
  `input = "ab"`, `delim = 'x'` → `end = 1` exactly; `start =
  delim.len_utf8()` → `1`; the boundary is observed.
- Accept: `strip_prefix` absent → the `None` arm flows into the
  evaluated default.
- Reject: exactness from a method outside the families, a non-identity
  closure, a dynamic input, or a shadowed/out-of-scope binding.

## Test Mapping

`analysis/classify/value_transfer.rs` `tests`;
`analysis/classify/activation.rs`
`activation_evidence_resolves_computed_local_boundary_operands`;
`analysis/language/rust.rs` end-to-end retarget+evaluation tests;
fixtures `binding_predicate_equality_boundary` (re-blessed flip) and
the #3295 family fixtures.

## Non-Goals

- No macro expansion, no arbitrary closure interpretation, no trait
  dispatch, no cross-function propagation (later #3215 slices).
- Literal-driven match arms are not an operand-value family: a match
  scrutinee reaches the retarget as a probe, but its arms feed sinks,
  not comparison operands; they stay unsupported until a sink-path
  slice consumes them.
- No propagation claim: operand exactness does not prove the changed
  value reaches an observable sink (`propagation_unknown` stays
  honest).

## Implementation Mapping

- `analysis/classify/value_transfer.rs` — the typed evaluator.
- `analysis/classify/activation.rs` — exact operand resolution and the
  boundary/missing-discriminator wiring; char-literal extraction in
  `scalar_values`.

## Metrics

No new metric; computed-local boundary shapes move from
`infection_unknown`-adjacent weakness to observed-boundary evidence in
existing activation counts.
