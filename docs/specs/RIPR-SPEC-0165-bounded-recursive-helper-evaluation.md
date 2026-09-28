# RIPR-SPEC-0165: Bounded recursive helper evaluation

Status: proposed

Issue: #3296 (parent #3215; builds on RIPR-SPEC-0159 and
RIPR-SPEC-0164)

## Problem

After the match-arm slice (RIPR-SPEC-0164), the last #3296 fixture
row is still unmet: a helper that calls itself (or another helper)
from inside its evaluated body. The caller-side chain resolver names
recursion as a stop edge, and the return-value evaluator had no
context at all — a nested direct call inside a match arm value
(`"word" => label_of("text")`) refused the whole helper, so a caller
boundary over a recursive helper kept `weakly_exposed` with an unknown
operand even when every related test row pins the exact value and the
recursion unrolls trivially on distinct inputs.

## Behavior

- One bounded evaluation context (`HelperEval` in
  `analysis/classify/helper_transfer.rs`) threads the workspace index
  and completeness through every helper-return evaluation. It counts
  helper-return evaluations on the current path against the existing
  explicit [`MAX_HELPER_HOPS`] (3) bound — the same bound the
  caller-side chain enforces — and records each `(helper, bound
  inputs)` state already evaluated.
- A nested state with **distinct bound inputs** unrolls within the
  bound (`label_of("word")` -> `label_of("text")`). A **repeated
  state** — the same helper over the same inputs — is a true cycle
  (the same inputs take the same arm) and refuses. A chain that would
  exceed the bound refuses at the boundary evaluation.
- The context gates the entry of `helper_return_value` itself, before
  any body shape is considered, so every authority (scanner, match,
  let-chain) shares one recursion bound.
- A match arm value (RIPR-SPEC-0164) may be one nested direct call
  (`label_of("text")`). Decomposition, callee uniqueness, argument
  splittability, and binding are the shared direct-call authority
  (`resolve_direct_call`); argument binding stays strict — a literal
  or a parameter bound in the calling helper's inputs. A computed
  argument or non-unique callee refuses by rule.
- The observed-value provenance keeps naming the operand-to-authority
  edge (`(1 hop)`); internal recursion consumes bounded evaluation
  slots but is not surfaced as additional operand hops.

## Required Evidence

- A positive fixture (`fixtures/recursive_positive`): a self-recursive
  arm value resolves through two distinct states and flips the caller
  boundary to `exposed` with the hop provenance.
- A controls fixture (`fixtures/recursive_controls`) with four
  fail-closed variants — a repeated-state cycle, a chain one step
  beyond the bound, a computed nested argument, and a non-unique
  nested callee — each staying `weakly_exposed` with zero hop
  provenance.
- Unit tests pinning: the within-bound nested resolution, the repeated
  state refusal, the at-bound resolution and beyond-bound refusal
  (three evaluations accepted, four refused).
- A removal experiment: disabling the context's entry gate regresses
  the positive fixture to `weakly_exposed`.

## Required guards

- The bound is a refusal, never a truncation: no partial unroll is
  presented as exact.
- The state key includes the bound inputs, so genuinely distinct
  inputs are not false cycles — and identical inputs are never
  re-evaluated.
- Nested binding reuses the strict operand rules; no new matcher is
  forked from the shared direct-call authority.

## Acceptance Examples

```text
Given:  pub fn label_of(kind: &str) -> &'static str {
            match kind {
                "word" => label_of("text"),
                "text" => "beta",
                _ => "alpha",
            }
        }
        pub fn classify(input: &str) -> &'static str {
            let final_label = label_of(input);
            if final_label == "alpha" { "word" } else { "blank" }
        }
Diff:   the equality constant changes "alpha" -> "beta"
Tests:  assert_eq!(classify("word"), "blank"); assert_eq!(classify("zz"), "word")
Then:   exposed — label_of("word") unrolls through label_of("text") to
        "beta" within the bound, the boundary is observed exactly, and
        the provenance names the helper hop
Not:    `_ => label_of(kind)` (a repeated state), a d->c->b->a chain
        (four evaluations), a computed nested argument, or a
        non-unique nested callee resolves (controls fixture)
```

## Test Mapping

- `crates/ripr/src/analysis/classify/match_transfer.rs::tests` —
  nested resolution, repeated state, at-bound and beyond-bound chains.
- `crates/ripr/src/analysis/classify/helper_transfer.rs::tests` — the
  context threads the existing evaluator bodies unchanged.
- `fixtures/recursive_positive` / `fixtures/recursive_controls` — the
  flip and the four fail-closed variants.

## Non-Goals

- Recursion whose termination depends on input consumption not
  representable as distinct bound states (slicing walks): these stay
  beyond the bound by construction and refuse.
- Nested calls in scanner state tokens or let-chain initializers
  (arm values only in this slice).
- Recursion depth beyond `MAX_HELPER_HOPS`, non-unique callees, and
  computed arguments: named refusals, not silent drops.
- Any claim beyond draft-mode exposure evidence; real mutation
  testing confirms later.

## Implementation Mapping

- `analysis/classify/helper_transfer.rs` — `HelperEval`, the entry
  gate, and `nested_call_value` (new).
- `analysis/classify/match_transfer.rs` — `MatchValue::Call` arm
  values resolved through the shared context.
- `analysis/classify/activation.rs` — `resolve_direct_call` and
  `function_parameters` become `pub(crate)` shared authorities; the
  operand path builds the root context.

## Metrics

- `unit_test_pass_rate` — the helper/match transfer unit tests.
- `golden_fixture_pass_rate` — the two fixtures' expected outputs.
