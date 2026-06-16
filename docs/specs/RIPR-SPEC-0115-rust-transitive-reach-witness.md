# RIPR-SPEC-0115: Rust Transitive-Reach Witness

Status: proposed

Owner: product / swarm

Created: 2026-06-16

Linked issues:

- First-run-trust campaign, P3.1 ("visibility before inference"). RIPR-SPEC-0114 detects a
  bounded transitive path from a test to the changed owner and names the limitation
  (`rust_transitive_reach_unresolved`), but discards *which* test produced the path. A new user
  sees an opaque "a test may reach this through a path ripr does not fully trace" with no pointer
  to the test, so they cannot go check it. This slice names the witness.

Linked PRs:

- P3.1 slice (this PR)

Support-tier impact:

- No tier change. This spec enriches the existing `rust_transitive_reach_unresolved` limitation
  message (an entry already in `Finding.evidence`) with a concrete pointer to the witnessing test
  (file:line) and the entry public-API symbol it called. Classification stays `no_static_path`
  (fail-closed); no promotion; the witness is NOT added to `related_tests`.
- No new JSON field. The witness is rendered into the existing `evidence: Vec<String>` channel and
  the existing `stop_reasons`. No `schema_version` bump.
- Claim boundaries remain governed by the canonical ledger in
  [support tiers](../status/SUPPORT_TIERS.md).

Policy impact:

- Register this spec in `policy/doc-artifacts.toml`.
- No new `StaticLimitKind`, `StopReason`, crate, binary, dependency, parser, runtime executor, or
  LSP server. Reuses `StaticLimitKind::RustTransitiveReachUnresolved` and
  `StopReason::TransitiveReachUnresolved` from RIPR-SPEC-0114.

## Problem

RIPR-SPEC-0114 made `no_static_path` honest when a bounded BFS finds a plausible transitive path
(test → public façade → ... → changed `pub(crate)` owner). But the emitted message is generic:

> ripr saw a test reaching public API that may call toward this change through a transitive path it
> does not fully trace ...

A first-run user reading that has no idea *which* test ripr is talking about, so the limitation is
honest but not actionable-to-read. The user cannot open the test and judge for themselves whether
their change is exercised. The information needed to name the witness — the test that started the
successful walk and the entry symbol it called — is already computed inside the BFS and then thrown
away.

This slice surfaces it: name the witnessing test (file:line) and the entry public-API symbol, so
the limitation points the user at something concrete to inspect.

## Behavior

### Trigger condition

Unchanged from RIPR-SPEC-0114. After the direct-call classifier returns
`ExposureClass::NoStaticPath` with empty `related_tests`, the Rust adapter runs the bounded BFS
(`has_transitive_candidate`). The only change: when a candidate path is found, the walk records the
**witness** instead of returning a bare `true`.

### Witness data

`has_transitive_candidate` is changed to return `Option<TransitiveWitness>` (where `Some` replaces
the old `true` and `None` replaces `false`). `TransitiveWitness` carries, from the `TestFact` and
the first hop that produced the successful walk:

- `test_name: String` — the witnessing test function name.
- `test_file: PathBuf` — the test's source file.
- `test_line: usize` — the test's start line.
- `entry_symbol: String` — the non-macro, non-direct callee the test invoked that began the
  walk reaching the owner (the "public-API entry point").

### Determinism

When more than one test witnesses a path, the witness is selected **deterministically**: candidate
witnesses are ordered by `(test_file, test_line, test_name, entry_symbol)` and the first is chosen.
This keeps goldens stable regardless of index iteration order. If `N > 1` witnesses exist, the
rendered message notes the count ("and N other tests") without enumerating them.

### When found

Unchanged: set `Finding.static_limit_kind = Some(StaticLimitKind::RustTransitiveReachUnresolved)`,
push `StopReason::TransitiveReachUnresolved`, and push the limitation message to `Finding.evidence`.

The `evidence` channel now carries **two parts**: the existing generic
`RUST_TRANSITIVE_REACH_MESSAGE` framing, followed by a concrete witness pointer, e.g.:

> For example, the test `test_uses_outer` (tests/it.rs:12) calls `outer`, an entry point that may
> lead here. Inspect it to judge whether this change is observed.

The witness pointer uses the same "may"/candidate language — it states the test calls the *entry
symbol*, NOT that the test reaches or covers the change.

**Human output:** the human renderer surfaces the witness pointer under a dedicated `Where to look`
section (after `Stop reasons:`), so a first-run CLI user — not only JSON consumers — sees which test
to open. The producer (`analysis::classify::transitive_reach`) and the renderer
(`output::human`) agree on one shared prefix constant
(`domain::TRANSITIVE_REACH_WITNESS_PREFIX`) so the JSON evidence string and the human line stay
single-sourced (reuse, don't fork). No structured JSON field is added; the witness lives in the
existing `evidence` vec (0114's established limitation-prose channel).

### When NOT found

Unchanged: bare `no_static_path`, no `static_limit_kind`, no witness.

### Fail-closed boundaries

- NEVER change classification from `no_static_path`. No promotion.
- The witness is NOT added to `related_tests` — that channel is reserved for direct/verified
  relations. The witness lives only in `evidence` (and the internal stop-reason), clearly labeled
  as a candidate entry point.
- The witness pointer NEVER says the test "reaches", "covers", "tests", or "exercises" the change.
  It says the test calls an entry symbol that "may lead here".
- All RIPR-SPEC-0114 boundaries are inherited unchanged: name-only matching over lexical call
  facts, depth ≤ 3, stop at macros, stop at out-of-crate callees.

### Wire format

| Field | Value when fires | Value when not fires |
|-------|------------------|----------------------|
| `classification` | `no_static_path` (unchanged) | `no_static_path` (unchanged) |
| `static_limit_kind` | `"rust_transitive_reach_unresolved"` | omitted / null |
| `stop_reasons` | includes `"transitive_reach_unresolved"` | unchanged |
| `evidence` | generic framing **+ concrete witness pointer** naming test file:line and entry symbol | unchanged |
| `related_tests` | unchanged (witness NOT added) | unchanged |

## Non-Goals

- Adding a structured machine-readable witness field to JSON — out of scope; this slice is
  evidence-message-only. A structured `transitive_witness` field can be a later additive slice.
- Adding the witness to `related_tests` — explicit fail-closed boundary.
- Listing every witnessing test — only the deterministic first witness is named (count noted).
- Promoting `no_static_path` to any higher class — that remains a separate, deferred slice that
  requires visibility/disambiguation facts ripr does not yet have.
- Tracing through macros, generics, or trait dispatch — inherited RIPR-SPEC-0114 boundaries.

## Acceptance Examples

1. **Positive (witness named)**: `pub(crate) fn inner()` changed; integration test
   `test_uses_outer` in `tests/it.rs` calls `outer()` which calls `inner()`. Result:
   `no_static_path` + `static_limit_kind: rust_transitive_reach_unresolved`, and the evidence
   includes "the test `test_uses_outer` (tests/it.rs:NN) calls `outer`, an entry point that may
   lead here." Fixture: `fixtures/rust_transitive_reach_positive/` (re-blessed: message-only drift).

2. **Negative (no witness)**: no test reaches the owner. Result: bare `no_static_path`, no
   `static_limit_kind`, no witness. Fixture: `fixtures/rust_transitive_reach_negative/` (unchanged).

3. **Deterministic selection**: two tests (`test_a` in `tests/a.rs`, `test_b` in `tests/b.rs`) both
   reach the owner. The named witness is `test_a` (sorts first by file), and the message notes "and
   1 other test".

4. **Honest language**: the witness pointer contains "may lead here" and does NOT contain
   "reaches", "covers", "tests", or "exercises".

5. **Class invariance**: in every case above, `classification` stays `no_static_path`; the witness
   never appears in `related_tests`.

## Required Evidence

- `has_transitive_candidate` changed to return `Option<TransitiveWitness>` in
  `analysis/classify/transitive_reach.rs`; `TransitiveWitness` struct defined there.
- Deterministic witness ordering by `(test_file, test_line, test_name, entry_symbol)`.
- Concrete witness-pointer message builder (reuses "may" language; no coverage claim).
- Wired in `analysis/language/rust.rs` `analyze_diff` and `analyze_repo` (the existing
  post-classify guards now consume the witness to build the evidence string).
- Positive fixture re-blessed (message-only drift; class + static_limit_kind unchanged), recorded
  via `cargo xtask goldens bless rust_transitive_reach_positive --reason <reason>`.
- Negative fixture unchanged.
- Unit tests: witness captured on a positive path; `None` on no path; deterministic selection
  across two witnesses; witness-pointer message contains "may lead here" and none of the forbidden
  coverage verbs; existing RIPR-SPEC-0114 boundary tests (depth, macro, external) still pass with
  the new return type.

## Test Mapping

- `crates/ripr/src/analysis/classify/transitive_reach.rs::tests::given_test_calls_outer_which_calls_owner_then_witness_is_captured`
  — positive path returns `Some(witness)` naming the test and entry symbol
- `crates/ripr/src/analysis/classify/transitive_reach.rs::tests::given_no_path_to_owner_then_witness_is_none`
  — no path returns `None`
- `crates/ripr/src/analysis/classify/transitive_reach.rs::tests::given_two_witnesses_then_first_by_file_line_is_selected`
  — deterministic witness ordering
- `crates/ripr/src/analysis/classify/transitive_reach.rs::tests::witness_pointer_uses_may_language_and_no_coverage_claim`
  — message honesty (contains "may lead here"; excludes reaches/covers/tests/exercises)
- `crates/ripr/src/analysis/classify/transitive_reach.rs::tests::given_path_at_depth_3_then_witness_is_captured`
  — depth-3 boundary still returns a witness
- `crates/ripr/src/analysis/classify/transitive_reach.rs::tests::given_path_depth_4_then_witness_is_none`
  — depth-4 boundary returns `None`
- `crates/ripr/src/analysis/classify/transitive_reach.rs::tests::given_macro_call_in_test_calls_then_witness_is_none`
  — macro entry skipped
- `crates/ripr/src/analysis/classify/transitive_reach.rs::tests::given_callee_not_in_crate_then_witness_is_none`
  — external callee stops walk
- `crates/ripr/src/output/human::tests::human_output_surfaces_transitive_reach_witness_as_where_to_look`
  — human renderer surfaces the witness under `Where to look`
- `crates/ripr/src/output/human::tests::human_output_omits_where_to_look_without_witness`
  — no witness line -> no `Where to look` section (fail-closed)

## Implementation Mapping

| Component | Location |
|---|---|
| `TransitiveWitness` struct + `find_transitive_witness -> Option<TransitiveWitness>` | `crates/ripr/src/analysis/classify/transitive_reach.rs` |
| Deterministic witness selection | `crates/ripr/src/analysis/classify/transitive_reach.rs` |
| Witness-pointer message builder (`transitive_reach_witness_pointer`) | `crates/ripr/src/analysis/classify/transitive_reach.rs` |
| Shared witness prefix constant (`TRANSITIVE_REACH_WITNESS_PREFIX`) | `crates/ripr/src/domain/classification.rs` |
| Module exports | `crates/ripr/src/analysis/classify/mod.rs`, `crates/ripr/src/domain/mod.rs` |
| Wiring (diff mode) | `crates/ripr/src/analysis/language/rust.rs::analyze_diff` |
| Wiring (repo mode) | `crates/ripr/src/analysis/language/rust.rs::analyze_repo` |
| JSON renderer | already handles `static_limit_kind` / `evidence` generically (no change) |
| Human renderer (`Where to look` section) | `crates/ripr/src/output/human/sections.rs` |
| Positive fixture (re-blessed) | `fixtures/rust_transitive_reach_positive/` |
| Negative fixture (unchanged) | `fixtures/rust_transitive_reach_negative/` |

## CI Proof

- `RUSTFLAGS="-D warnings" cargo build -p ripr -p xtask` — exit 0.
- `cargo test --workspace` — all pass including updated BFS unit tests.
- `cargo clippy --workspace --all-targets -- -D warnings` clean.
- `cargo fmt --check` clean.
- `cargo xtask check-static-language` pass (witness pointer uses no forbidden vocabulary).
- `cargo xtask check-architecture` pass.
- `cargo xtask check-no-panic-family` pass.
- `cargo xtask check-allow-attributes` pass.
- `cargo xtask check-doc-artifacts` pass.
- `cargo xtask check-doc-index` pass.
- `cargo xtask check-spec-format` pass.
- `cargo xtask check-traceability` pass.
- `cargo xtask goldens check` clean after the recorded re-bless of the positive fixture.
- `cargo xtask fixtures` clean (both fixtures pass).

## Metrics

- Gate: positive fixture re-blessed with message-only drift (class + static_limit_kind byte-identical
  except the enriched evidence string); negative fixture 0 drift.
- Behavioral repro: positive fixture's human output names the witnessing test file:line and entry
  symbol with "may lead here" language; classification stays `no_static_path`; the witness does not
  appear under `related_tests`.
