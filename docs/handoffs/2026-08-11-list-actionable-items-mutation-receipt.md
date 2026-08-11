# `ripr/listActionableItems` mutation receipt (#3032)

Date: 2026-08-11
Repository: `EffortlessMetrics/ripr-swarm`
Inspected ref: `origin/main` at `c8e3e18345ab34291981978963c7b1b5e13988fc`
Classification: `verified_current`

## Scope

Issue #3032's residual acceptance row: a retained removal/mutation receipt
proving the tests landed by #3031 (`6fc4d5eb`) fail when one handler branch or
capability field of the `ripr/listActionableItems` slice (#3012, squash
`d3fafbcb`) is weakened. Each mutation below makes behavior wrong while
keeping the crate compilable; each was applied alone, measured, and reverted
before the next ran.

## Baseline (unmutated `origin/main`)

```text
cargo test -p ripr --lib lsp::backend::list_actionable_items_tests
  test result: ok. 4 passed; 0 failed
cargo test -p ripr --lib lsp::agent_protocol
  test result: ok. 11 passed; 0 failed
```

## Mutation A — handler error branch weakened

Change: in `Backend::ripr_list_actionable_items`
(`crates/ripr/src/lsp/backend.rs`), the no-snapshot branch's
`"kind": "no_snapshot"` replaced with `"kind": "analysis_in_flight"`.

Expected: the no-snapshot path test fails; the other three handler tests
still pass (branch-discriminating, not suite-wide).

Actual:

```text
cargo test -p ripr --lib lsp::backend::list_actionable_items_tests  (exit 101)
  list_actionable_items_without_snapshot_fails_closed ... FAILED
  list_actionable_items_analysis_in_flight_fails_closed ... ok
  list_actionable_items_unavailable_delivery_fails_closed ... ok
  list_actionable_items_applied_returns_bounded_envelope ... ok
  test result: FAILED. 3 passed; 1 failed
```

Reverted with `git checkout -- crates/ripr/src/lsp/backend.rs`.

## Mutation B — success envelope weakened

Change: the `"must_not_change": [...]` field removed from the applied-outcome
envelope in `Backend::ripr_list_actionable_items`.

Expected: the applied-path test fails; the three error-path tests still pass.

Actual:

```text
cargo test -p ripr --lib lsp::backend::list_actionable_items_tests  (exit 101)
  list_actionable_items_applied_returns_bounded_envelope ... FAILED
  list_actionable_items_without_snapshot_fails_closed ... ok
  list_actionable_items_analysis_in_flight_fails_closed ... ok
  list_actionable_items_unavailable_delivery_fails_closed ... ok
  test result: FAILED. 3 passed; 1 failed
```

Reverted with `git checkout -- crates/ripr/src/lsp/backend.rs`.

## Mutation C — capability field weakened

Change: in `RiprAgentCapability::v0_1_implemented()`
(`crates/ripr/src/lsp/agent_protocol.rs`), `supported_requests` changed from
`vec![RiprAgentRequest::ListActionableItems]` to `vec![]`.

Expected: the fail-closed capability equality test fails; the other
agent-protocol tests still pass.

Actual:

```text
cargo test -p ripr --lib lsp::agent_protocol  (exit 101)
  capability_advertises_only_implemented_handlers ... FAILED
  (all 10 other lsp::agent_protocol tests) ... ok
  test result: FAILED. 10 passed; 1 failed
```

Reverted with `git checkout -- crates/ripr/src/lsp/agent_protocol.rs`. After
all three reversions `git status --porcelain` is empty.

## Verdict

All three mutations were caught by exactly the pinned test for the weakened
branch or field, with no collateral failures. The #3031 tests discriminate
handler branches and the capability contract as #3032 requires. This receipt
satisfies #3032's residual acceptance row (removal/mutation experiment). It
does not satisfy #3080's separate, stronger removal experiment over typed
envelopes and immutable snapshots, which remains owned by #3080.

## Companion evidence: `MutexGuard::clone` claim refuted

The #3012 post-merge review raised a possible `MutexGuard::clone` type defect
in the landed snapshot extraction:

```rust
let latest = self.latest_analysis.lock().ok();
let snapshot = match latest {
    Some(guard) => guard.clone(),
    None => None,
};
```

Refuted, not accepted by narration:

1. The exact landed shape, with the production field type
   `Mutex<Option<Arc<AnalysisSnapshot>>>`, was extracted into a standalone
   probe and compiled with the pinned toolchain
   (`rustc 1.95.0 (59807616e 2026-04-14)`, `--edition 2024 -D warnings`):
   it compiles with no warnings and runs. `MutexGuard` has no `Clone` impl, so
   `guard.clone()` resolves through `Deref` to
   `Option::<Arc<AnalysisSnapshot>>::clone`; the probe asserts the result via
   `Arc::ptr_eq`, confirming a cheap `Arc` refcount bump, not a guard clone or
   a deep clone.
2. `d3fafbcb` is an ancestor of `origin/main` and main builds and passes CI —
   the landed code demonstrably compiled.

The claim was a valid review lead but is not a defect. Current main has since
simplified the extraction to
`.and_then(|guard| (*guard).clone())` (#3031), which makes the deref explicit;
that is a clarity improvement, not a bug fix.
