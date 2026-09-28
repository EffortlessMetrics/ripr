# RIPR-SPEC-0159: Bounded helper-call transfer

Status: proposed

Issue: #3296 (parent #3215; builds on #3294 / RIPR-SPEC-0157 and
#3295 / RIPR-SPEC-0158)

## Problem

After #3295, a changed binding inside a helper that related tests
reach only through a short call chain (`test -> classify ->
is_word_start`) still reports `no_static_path`: the relation stage
requires a direct call to the owner, the exact input literals never
cross the helper edge, and the transitive-reach walk (RIPR-SPEC-0114)
can only hint that a caller "may lead here" without changing anything.

## Behavior

- One helper-transfer authority (`analysis/classify/helper_transfer.rs`)
  resolves the chain above the owner once; the relation stage
  (`find_related_tests`), the activation rows, and the call-operand
  evaluation all consume the same resolution.
- V1 transfers only: direct same-workspace calls with a **unique
  callee identity** (the #2971 workspace-complete uniqueness rule),
  **positional argument-to-parameter binding** where every bound
  argument is a literal or the caller's own parameter (resolved
  through that caller's rows), at most **3 hops**, **acyclic**, with a
  single caller and a single call site per hop.
- On resolution, a test that calls any resolved hop's caller relates
  to the helper-owned probe (`RelationReason::HelperOwnerCall`): the
  reach, observation, and oracle stages see the same related tests a
  direct call would produce. The owner's exact input rows bind down
  the chain, so #3295's boundary machinery evaluates the helper's
  operands with the tests' literals.
- A comparison operand that is a **direct call to a unique helper**
  (`is_word_start(input, 0) == want`) evaluates through the helper's
  return when the body is simple single-line `let` statements plus a
  bare-binding tail expression evaluated through the #3295 families.
  Anything else fails closed.
- Every unsupported edge is named, never silently dropped: non-unique
  callee, incomplete workspace index, recursion, multiple callers,
  multiple call sites, computed (non-literal, non-parameter)
  arguments, hop-bound exhaustion, and unsupported return bodies.

## Relation to RIPR-SPEC-0114

The 0114 transitive-reach walk is **lexical** (name-match only) and
stays fail-closed: it names a candidate path and never changes the
classification. The 0159 transfer is a different, stronger relation —
typed callee identity, workspace-complete uniqueness, and exact
argument binding — and only that relation relates tests and carries
values. A chain the typed relation cannot fully resolve keeps the 0114
limitation unchanged.

## Required Evidence

- The reproduction flip (helper-owned probe: `reach no` + limitation
  hint -> `reach yes` with both exact oracles related through the
  chain) and the removal experiment: with the relation branch disabled
  the reproduction regresses to the limitation-hint state.
- Authority unit tests: one-hop resolution with arguments, non-unique
  callee stop, incomplete-workspace stop, recursion stop, multi-caller
  stop, entry-hop test reach, helper return evaluation (identity tail
  evaluates; non-identity closure fails closed).
- `rust_constructor_field_wrong_field_observer` re-blessed and its
  corpus expectation graduated (`no_static_path` ->
  `propagation_unknown`): its chain is genuinely resolvable under the
  typed conditions, so the test legitimately relates while
  `must_not_promote` and every repair guard stay intact.
  `rust_transitive_reach_positive` keeps its SPEC-0114 pin exactly: a
  second same-name `inner` makes the callee non-unique, the typed
  transfer refuses, and the lexical-walk limitation stays the pinned
  outcome (golden byte-identical to main).
- The fixture corpus: one-hop positive/negative with exact tests,
  bounded multi-hop, the fail-closed controls (same-name helper in
  another module, computed argument, wrong-sink assertion).
- Golden blast radius measured; `cargo xtask goldens check` and
  `cargo xtask dogfood` green otherwise.

## Required guards

- Call reach alone never establishes propagation or discrimination:
  the oracle still has to observe the sink through the ordinary
  evidence stages.
- A chain that stops keeps the pre-#3296 output exactly (the 0114
  limitation remains).
- The workspace-completeness gate is mandatory: a partial index never
  transfers (a same-named function in an unindexed file would make the
  name falsely unique).
- No cross-crate, trait/dyn/function-pointer, macro, or loop transfer
  in V1; those edges stop by name.

## Acceptance Examples

- Accept: `test -> classify(" x") -> is_word_start(input, 0)` — the
  helper-owned probe relates to the test, its oracles connect, and its
  operands evaluate with `input = " x"`.
- Accept: a comparison operand that is a unique helper call evaluates
  through a simple binding-tail return.
- Reject: relating through a non-unique callee, a method or
  path-qualified call site, a partial index, recursion, or multiple
  callers. A computed argument stops the value/row transfer (named
  edge) while the reach relation may still hold — the call chain is
  genuinely reachable; only the exact values stop.

## Test Mapping

`analysis/classify/helper_transfer.rs` `tests`;
`analysis/classify/related_tests.rs` (the `HelperOwnerCall` relation
branch); `analysis/classify/activation.rs` (transferred rows and the
call operand); fixtures `helper_chain_{one_hop,multi_hop,controls}`.

## Non-Goals

- No scanner-state or loop evaluation, no recursive-positive support,
  no trait/dyn/function-pointer/macro/FFI transfer (named V1 stops;
  scanner transitions remain a #3296 remainder).
- No cross-function predicate transfer: a helper-owned probe still
  names its own body's predicates; the caller's predicate on the
  helper call evaluates only when it is the probe's comparison
  operand.
- No propagation claim beyond the existing owner-local machinery.

## Implementation Mapping

- `analysis/classify/helper_transfer.rs` — the authority.
- `analysis/classify/related_tests.rs` — the relation branch.
- `analysis/classify/activation.rs` — transferred rows, call operands.
- `analysis/classify/context.rs` — the chain on `ProbeContext`.

## Metrics

No new metric; helper-owned probes with resolvable chains move from
`no_static_path` to the ordinary reachable classes in existing counts.
