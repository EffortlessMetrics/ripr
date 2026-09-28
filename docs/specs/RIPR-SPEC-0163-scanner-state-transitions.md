# RIPR-SPEC-0163: Bounded scanner-state transitions

Status: proposed

Issue: #3296 (parent #3215; builds on #3295 / RIPR-SPEC-0158 and the
helper-chain slice of #3296 / RIPR-SPEC-0159)

## Problem

After RIPR-SPEC-0159, a caller-side equality boundary whose operand is
a scanner helper's return (`let final_state = scan_state(input); if
final_state == "word"`) still reports the operand as unknown: the
operand-resolution stages handle parameters, plain locals, and direct
call operands, but a local whose initializer is a call never reaches
the helper authority, and the helper-return evaluator itself fails
closed on any loop body — so a literal-driven state machine with fully
exact inputs keeps `weakly_exposed` with `observed ... values: unknown`
even when every related test row pins the exact state.

## Behavior

- One scanner authority (`analysis/classify/scanner_transfer.rs`)
  evaluates a helper's final state when the body has the exact pinned
  shape: `let mut state = <state>;` then `for symbol in <iterable> {`
  then `state = match (state, symbol) {` with literal arms, closing
  braces, and a bare `state` tail. Any other line inside the body
  refuses the whole shape (the #3295/#3299 families keep their own
  authorities).
- A comparison operand that is a **local whose initializer is a direct
  call to a unique helper** now resolves through the helper authority
  (the operand jump): the call decomposition (unique callee, splittable
  arguments) is the same one raw call operands use, and the argument
  binding stays strict — a literal or the owner's bound parameter. A
  computed argument still stops by name (RIPR-SPEC-0159 edge).
- State tokens are plain string literals (`"text"`) or path-qualified
  identifiers (`Scan::Word`). A **bare single-segment identifier is
  refused in every position** — pattern, next state, and initializer
  all fall back to the exact-input binding first, because in Rust the
  same text may be a variable or binding (the token-coincidence
  family); an unbound bare identifier refuses the scan rather than
  reading its text as a state.
- The initial state and the symbol iterable resolve through the #3295
  evaluator over the row's bound inputs (a string or `<input>.chars()`
  char sequence). The evaluation unrolls at most
  `MAX_SCANNER_STEPS` (32) transitions; a longer exact input refuses
  the evaluation (no partial state is ever presented as exact).
- On resolution the final state is an exact typed value; the boundary
  machinery compares it against the row's other operand exactly as
  #3295 does, and the observed-value provenance names the hop
  (``scan_state = "text" via helper return of `scan_state` over bound
  inputs (1 hop)``).

## Required Evidence

- The reproduction flip: the same caller-side boundary moves from
  `weakly_exposed` (`observed final_state values: unknown`) to
  `exposed` (per-row exact states, boundary equality observed on one
  row), and the removal experiment — the scanner branch disabled in
  the helper authority — regresses it to `weakly_exposed`.
- Authority unit tests: exact final-state resolution over string and
  qualified-path states, wildcard-vs-explicit arm precedence, the
  step bound refusing beyond-limit inputs while resolving at the
  bound, initial state from a bound input identifier, fail-closed
  controls (missing input, computed next-state arm, extra loop
  mutation, escaped/empty string states, bare-identifier states in
  pattern, next, and initializer positions).
- The fixture corpus: `scanner_positive` (exposed with the hop
  provenance) and `scanner_controls` (step-bound, computed-argument,
  computed next-state, and bare-identifier controls stay
  `weakly_exposed` with no scanner hop or invented state).
- Golden blast radius measured; `cargo xtask goldens check` and
  `cargo xtask dogfood` green otherwise.

## Required guards

- The scanner never fabricates states: every state in the unroll came
  from a parsed literal arm or the exact initial state.
- The shape is a strict grammar, not a heuristic: one unrecognized
  line inside the loop refuses the whole evaluation (the #3317
  over-credit lesson applied at the parse level).
- The step bound is a refusal, not a truncation.
- The workspace-completeness and callee-uniqueness guards of
  RIPR-SPEC-0159 apply unchanged to the operand jump.
- Call reach alone still establishes nothing: without an exact
  evaluation on a related-test row the boundary stays unobserved.

## Acceptance Examples

- Accept: `test -> classify("ab ") -> scan_state(input)` with a
  string-state scanner — the operand resolves per row
  (`"ab"` -> `"word"`, `"ab "` -> `"text"`), the boundary equality is
  observed, and the finding exposes with the hop in the provenance.
- Accept: `Scan::Text`/`Scan::Word` qualified-path states evaluate
  identically to string states.
- Reject: a computed argument (`scan_state(input.trim())`), a
  computed next state (`=> next_for(input)`), a bare-identifier state
  (`=> input`), a data-dependent iterable, an input beyond the step
  bound, or any unrecognized loop-body line — each keeps the operand
  unknown and the finding at its fail-closed class.

## Test Mapping

`analysis/classify/scanner_transfer.rs` `tests` and
`string_state_tests`; `analysis/classify/activation.rs` (the
local-with-call-initializer operand jump); fixtures
`scanner_{positive,controls}`.

## Non-Goals

- No `while` loops, no data-dependent loop bounds, no break/continue,
  no nested loops, no recursion, no state mutation outside the match,
  no non-literal arms (each a named refusal, not a partial credit).
- No new limitation-disclosure surface: a refusal keeps the existing
  `unknown` operand wording, matching the #3295 `Unsupported`
  precedent.
- No enum-type semantics: qualified-path states compare as normalized
  text tokens, and the right-hand operand still binds only through the
  existing literal machinery (a bare path operand on the right remains
  unresolved — a named limitation of this slice).
- No change to the helper-chain relation stage (RIPR-SPEC-0159 owns
  reach and rows; this spec only consumes them).

## Implementation Mapping

- `analysis/classify/scanner_transfer.rs` — the scanner authority.
- `analysis/classify/helper_transfer.rs` — the scanner branch precedes
  the let-chain evaluator in `helper_return_value`.
- `analysis/classify/activation.rs` — the operand jump through the
  shared direct-call decomposition.

## Metrics

No new metric; caller-side boundaries over resolvable scanners move
from `weakly_exposed` to `exposed` in existing counts, and the hop
appears in observed-value provenance.
