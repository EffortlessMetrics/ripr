# RIPR-SPEC-0114: Rust Transitive-Reach Limitation

Status: proposed

Owner: product / swarm

Created: 2026-06-15

Linked issues:

- dtolnay/semver dogfood: `pub(crate) matches_greater` reached only via `VersionReq::matches`
  (pub → pub(crate) helper chain) — ripr returned bare `no_static_path` with empty stop_reasons,
  misleading new users into thinking tests are absent.

Linked PRs:

- P3 slice-a (this PR)

Support-tier impact:

- No tier change. This spec adds an additive optional JSON field (`static_limit_kind`) and a
  named stop-reason (`transitive_reach_unresolved`) to `no_static_path` findings where a bounded
  BFS walk detects a plausible transitive path. Classification stays `no_static_path` (fail-closed);
  no promotion to weakly_exposed or higher.
- The `static_limit_kind` field is already defined in the schema; `rust_transitive_reach_unresolved`
  is a new enum value (additive). No `schema_version` bump is required.
- Claim boundaries remain governed by the canonical ledger in
  [support tiers](../status/SUPPORT_TIERS.md).

Policy impact:

- Register this spec in `policy/doc-artifacts.toml`.
- Register `RustTransitiveReachUnresolved` in `crates/ripr/src/domain/language.rs`.
- Register `TransitiveReachUnresolved` in `crates/ripr/src/domain/probe.rs`.
- No new crates, binaries, dependencies, parsers, runtime executors, or LSP servers introduced.

## Problem

When ripr classifies a Rust finding as `no_static_path`, it means the direct-call classifier
found no test that statically calls the changed owner by name. But many real-world production
functions are `pub(crate)` helpers only reachable through a public façade. A test calls the
façade (`VersionReq::matches`), which calls the helper (`matches_greater`), which is the changed
owner — but ripr's direct-call classifier only checks one hop. The result is bare `no_static_path`
with empty `stop_reasons`, which looks the same as "genuinely untested."

This slice names the limitation honestly:

> ripr saw a test reaching public API that may call toward this change through a transitive path
> it does not fully trace (pub to pub(crate) helper chains, macros, or generics). This is not a
> coverage assessment — ripr cannot confirm or deny that the change is observed.

## Behavior

### Trigger condition

After the direct-call classifier returns `ExposureClass::NoStaticPath` with empty `related_tests`,
the Rust adapter runs a bounded BFS walk over the lexical call facts in the RustIndex:

1. Collect all tests from the index (unit + integration).
2. For each test's `CallFact` that is NOT a macro invocation and NOT a direct call to the owner:
   find the corresponding production `FunctionFact` in-crate by name.
3. BFS from that production function through `FunctionFact.calls`, depth ≤ 3:
   - Stop at macro invocations (`name!`).
   - Stop when callee name not found in-crate (external / unresolved).
   - Stop when depth > 3.
4. If the owner's name is reached within depth ≤ 3: a candidate path was found.

### When found

Set `Finding.static_limit_kind = Some(StaticLimitKind::RustTransitiveReachUnresolved)`.
Push `StopReason::TransitiveReachUnresolved` to `Finding.stop_reasons`.
Push the honest limitation message to `Finding.evidence`.

The classification **stays `no_static_path`**. This is a named limitation, not a reach promotion.

### When NOT found

Leave the finding exactly as today (bare `no_static_path`, no `static_limit_kind`).
Genuinely-untested stays genuinely-untested.

### Fail-closed boundaries

- NEVER change the classification from `no_static_path`. No promotion of any kind.
- Walk is NAME matching over LEXICAL call facts only — no AST resolution, no visibility tracking.
- Depth is bounded at MAX 3 hops; any callee beyond depth 3 is not walked.
- Macro invocations (`name!`) stop the walk immediately at that branch.
- Callees not found in the production function set stop that branch (fail-closed on external deps).
- The message uses "may" language: "may call toward this change through a path ripr does not fully trace."

### Wire format

| Field | Value when fires | Value when not fires |
|-------|------------------|----------------------|
| `classification` | `no_static_path` (unchanged) | `no_static_path` (unchanged) |
| `static_limit_kind` | `"rust_transitive_reach_unresolved"` | omitted / null |
| `stop_reasons` | includes `"transitive_reach_unresolved"` | unchanged |
| `evidence` | includes the limitation message | unchanged |

## Non-Goals

- Promoting `no_static_path` to `weakly_exposed` or higher — that is the next P3 slice.
- Tracing macro invocations or generic dispatch — explicit fail-closed boundary.
- Tracking visibility (pub vs pub(crate)) — not available in lexical call facts.
- Depth > 3 paths — explicit fail-closed boundary.

## Acceptance Examples

1. **Positive (limitation fires)**: `pub(crate) fn inner()` changed; integration test calls
   `outer()` which calls `inner()`. Direct-call classifier finds no test for `inner`. BFS finds
   `test → outer → inner`. Result: `no_static_path` + `static_limit_kind: rust_transitive_reach_unresolved`.
   Fixture: `fixtures/rust_transitive_reach_positive/`.

2. **Negative (limitation does NOT fire)**: `apply_fee` changed; no tests at all. BFS finds no
   candidate path. Result: bare `no_static_path`, no `static_limit_kind`. Classification unchanged.
   Fixture: `fixtures/rust_transitive_reach_negative/`.

3. **Fail-closed on macros**: test calls `my_macro!()` which calls the owner — BFS stops at macro.
   No limitation fires (no candidate path found through macros). Classification stays bare `no_static_path`.

4. **Depth boundary**: test → fn_a → fn_b → fn_c → owner (4 hops) — exceeds depth 3. No candidate path.
   Classification stays bare `no_static_path`.

5. **Existing tests found directly**: direct-call classifier finds a test for the owner (related_tests
   non-empty). BFS does NOT run. Finding classified normally (infection_unknown or higher).

## Required Evidence

- `StaticLimitKind::RustTransitiveReachUnresolved` added to `domain/language.rs`.
- `StopReason::TransitiveReachUnresolved` added to `domain/probe.rs`.
- `analysis/classify/transitive_reach.rs` module with BFS walk, bounded at depth 3, fail-closed on
  macros/externals.
- Wired in `analysis/language/rust.rs` `analyze_diff` and `analyze_repo` post-classify guards.
- Positive fixture: `fixtures/rust_transitive_reach_positive/` — limitation fires.
- Negative fixture: `fixtures/rust_transitive_reach_negative/` — limitation does NOT fire.
- Unit tests covering: positive path, no path, depth-3 boundary (found), depth-4 (not found),
  macro skip, external callee stop, honest message language.

## Test Mapping

- `crates/ripr/src/analysis/classify/transitive_reach.rs::tests::given_test_calls_outer_which_calls_owner_then_transitive_candidate_found`
  — positive path found
- `crates/ripr/src/analysis/classify/transitive_reach.rs::tests::given_no_path_to_owner_then_no_transitive_candidate`
  — no path, limitation does not fire
- `crates/ripr/src/analysis/classify/transitive_reach.rs::tests::given_path_at_depth_3_then_transitive_candidate_found`
  — depth 3 boundary: still finds
- `crates/ripr/src/analysis/classify/transitive_reach.rs::tests::given_path_depth_4_then_no_transitive_candidate`
  — depth 4 boundary: does NOT find (fail-closed)
- `crates/ripr/src/analysis/classify/transitive_reach.rs::tests::given_macro_call_in_test_calls_then_macro_is_skipped`
  — macro invocations skipped
- `crates/ripr/src/analysis/classify/transitive_reach.rs::tests::given_callee_not_in_crate_then_walk_stops_fail_closed`
  — external callees stop walk
- `crates/ripr/src/analysis/classify/transitive_reach.rs::tests::wire_message_contains_honest_may_language_and_no_coverage_claim`
  — message uses "may", no coverage claim

## Implementation Mapping

| Component | Location |
|---|---|
| `StaticLimitKind::RustTransitiveReachUnresolved` | `crates/ripr/src/domain/language.rs` |
| `StopReason::TransitiveReachUnresolved` | `crates/ripr/src/domain/probe.rs` |
| BFS walk module | `crates/ripr/src/analysis/classify/transitive_reach.rs` |
| Module export | `crates/ripr/src/analysis/classify/mod.rs` |
| Wiring (diff mode) | `crates/ripr/src/analysis/language/rust.rs::analyze_diff` |
| Wiring (repo mode) | `crates/ripr/src/analysis/language/rust.rs::analyze_repo` |
| JSON renderer | already handles `static_limit_kind` generically (no change) |
| Human renderer | already handles `stop_reasons` generically (no change) |
| Positive fixture | `fixtures/rust_transitive_reach_positive/` |
| Negative fixture | `fixtures/rust_transitive_reach_negative/` |

## CI Proof

- `RUSTFLAGS="-D warnings" cargo build -p ripr -p xtask` — exit 0.
- `cargo test --workspace` — all pass including new BFS unit tests.
- `cargo clippy --workspace --all-targets -- -D warnings` clean.
- `cargo fmt --check` clean.
- `cargo xtask check-static-language` pass (limitation uses "no_static_path", not forbidden vocabulary).
- `cargo xtask check-architecture` pass.
- `cargo xtask check-no-panic-family` pass.
- `cargo xtask check-allow-attributes` pass.
- `cargo xtask check-doc-artifacts` pass.
- `cargo xtask check-doc-index` pass.
- `cargo xtask check-spec-format` pass.
- `cargo xtask check-traceability` pass.
- `cargo xtask goldens check` clean (no existing no_static_path classifications changed).
- `cargo xtask fixtures` clean (both new fixtures pass).

## Metrics

- Gate: 0 golden drift on existing fixtures; 2 new fixtures passing.
- Behavioral repro: positive fixture shows `static_limit_kind: "rust_transitive_reach_unresolved"`,
  classification stays `no_static_path`. Negative fixture shows bare `no_static_path`, no `static_limit_kind`.