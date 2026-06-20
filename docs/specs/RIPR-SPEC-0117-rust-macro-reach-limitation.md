# RIPR-SPEC-0117: Rust Macro-Reach Limitation

Status: accepted

Owner: product / swarm

Created: 2026-06-18

Linked issues:

- [#1292](https://github.com/EffortlessMetrics/ripr-swarm/issues/1292)

Linked PRs:

- This PR

Support-tier impact:

- No tier change. `docs/status/SUPPORT_TIERS.md` remains unchanged. This spec
  adds one additive `static_limit_kind` value, `rust_macro_reach_unresolved`,
  and one additive `stop_reasons` value, `macro_reach_unresolved`.
- Classification stays `no_static_path`. This is a named limitation, not a
  coverage claim, test relation, repair packet, release-readiness claim, or
  macro-expansion engine.
- No `schema_version` bump is required because `static_limit_kind` and
  `stop_reasons` already exist as additive optional finding metadata.

Policy impact:

- Register this spec in `policy/doc-artifacts.toml`.
- Register the new stop reason in `policy/output_contracts.txt`.
- Update `docs/OUTPUT_SCHEMA.md` and `.ripr/traceability.toml`.
- No new crates, dependencies, binaries, workflow permissions, process
  execution, runtime macro expansion, rustc driver integration, or LSP surface.

## Problem

RIPR-SPEC-0114 names bounded Rust helper-call chains when a test calls an entry
point that lexically reaches the changed owner. It intentionally stops at
macros. In real Rust repos, tests and public entry points often route through
`macro_rules!` helpers. Today that can leave a first-run user with bare
`no_static_path`, even though RIPR saw a plausible entry path and the unresolved
edge is specifically a macro boundary.

The honest first-run behavior is to name that boundary without pretending the
macro was expanded:

```text
no_static_path + static_limit_kind: rust_macro_reach_unresolved
```

## Behavior

### Trigger condition

After direct related-test classification returns `no_static_path` with no
related tests, and after the RIPR-SPEC-0114 bounded transitive witness check
does not find a lexical path, the Rust adapter may emit the macro-reach
limitation when all of these are true:

1. A test calls a non-owner Rust entry symbol, or directly invokes a macro.
2. A bounded BFS from that entry symbol reaches a production function that
   invokes a macro, or the test directly invokes the macro.
3. A same-repo `macro_rules!` definition with that macro name is visible in the
   indexed source.
4. That macro definition lexically mentions the changed owner name as an
   identifier.

### When found

Set `Finding.static_limit_kind = Some(StaticLimitKind::RustMacroReachUnresolved)`.
Push `StopReason::MacroReachUnresolved` to `Finding.stop_reasons`.
Push an honest limitation message and a concrete witness pointer into
`Finding.evidence`.

The witness pointer names:

- the witnessing test file and line;
- the entry symbol the test called;
- the macro invocation site;
- the macro name.

The pointer uses candidate language:

```text
The macro path may lead here. Inspect it to judge whether this change is observed.
```

### When not found

Leave the finding exactly as before. Do not emit the limitation merely because a
test file contains an unrelated macro, an external macro is invoked, or a macro
definition does not mention the changed owner name.

### Fail-closed boundaries

- Never change classification from `no_static_path`.
- Never add the witness to `related_tests`.
- Never claim the test reaches, covers, tests, or exercises the changed owner.
- Only same-repo `macro_rules!` definitions are considered.
- The macro definition scan is lexical owner-name matching only; it is not macro
  expansion, type resolution, hygiene, trait dispatch, visibility analysis, or
  rustc integration.
- If the macro definition is absent, ambiguous, generated, external, or does not
  lexically mention the owner, do not emit this limitation.

## Wire Format

| Field | Value when fires | Value when not fires |
|---|---|---|
| `classification` | `no_static_path` | unchanged |
| `static_limit_kind` | `rust_macro_reach_unresolved` | omitted / unchanged |
| `stop_reasons` | includes `macro_reach_unresolved` | unchanged |
| `related_tests` | unchanged (witness is not added) | unchanged |
| `evidence` | limitation prose plus concrete macro witness pointer | unchanged |

## Non-Goals

- Full macro expansion.
- Promoting to `weakly_exposed`, `reachable_unrevealed`, or `exposed`.
- Emitting a repair packet.
- Inferring macro hygiene, generated items, trait dispatch, or visibility.
- Solving all macro-heavy Rust reachability cases.

## Acceptance Examples

1. **Macro-boundary limitation fires**: an integration test calls `outer()`;
   `outer()` invokes `call_inner!`; same-repo `macro_rules! call_inner` mentions
   changed owner `inner`. Result: `no_static_path` plus
   `static_limit_kind: rust_macro_reach_unresolved`.
2. **No owner mention**: `outer()` invokes `call_other!`, but the macro
   definition does not mention changed owner `inner`. Result: unchanged bare
   `no_static_path`.
3. **External macro**: `outer()` invokes a macro whose definition is not in the
   indexed repo source. Result: unchanged bare `no_static_path`.
4. **Lexical transitive path exists**: the RIPR-SPEC-0114 witness fires first.
   Result remains a transitive/public-API limitation, not macro-reach. For
   integration-test witnesses, RIPR-SPEC-0118 refines the kind to
   `rust_integration_public_api_path_unresolved`; non-integration witnesses
   keep `rust_transitive_reach_unresolved`.

## Required Evidence

- `StaticLimitKind::RustMacroReachUnresolved` in
  `crates/ripr/src/domain/language.rs`.
- `StopReason::MacroReachUnresolved` in `crates/ripr/src/domain/probe.rs`.
- Macro-boundary witness logic in
  `crates/ripr/src/analysis/classify/transitive_reach.rs`.
- Rust adapter wiring in `crates/ripr/src/analysis/language/rust.rs` for diff
  and repo modes.
- Pure fixture: `fixtures/rust_macro_reach_limitation/`.
- Honesty corpus member in
  `fixtures/evidence-promotion-honesty-corpus/corpus.json` asserting
  `must_emit_limitation`, `expected_limit_kind: rust_macro_reach_unresolved`,
  `must_disclose_limitation_detail`, and `must_remain_non_promoted`.

## Test Mapping

- `crates/ripr/src/analysis/classify/transitive_reach.rs::tests::given_entry_path_stops_at_owner_macro_then_macro_witness_is_captured`
- `crates/ripr/src/analysis/classify/transitive_reach.rs::tests::given_macro_definition_does_not_name_owner_then_macro_witness_is_none`
- `crates/ripr/src/analysis/classify/transitive_reach.rs::tests::given_test_invokes_owner_macro_directly_then_macro_witness_is_captured`
- `crates/ripr/src/analysis/classify/transitive_reach.rs::tests::macro_witness_pointer_uses_may_language_and_no_coverage_claim`
- `crates/ripr/src/analysis/classify/transitive_reach.rs::tests::macro_reach_limitation_detail_names_edges_route_and_non_claim`
- `crates/ripr/src/output/human.rs::tests::human_output_surfaces_static_limitation_detail`
- `fixtures/rust_macro_reach_limitation/expected/check.json`
- `cargo xtask check-evidence-promotion-honesty`

## Implementation Mapping

| Component | Location |
|---|---|
| Static limit enum | `crates/ripr/src/domain/language.rs` |
| Stop reason enum | `crates/ripr/src/domain/probe.rs` |
| Macro witness producer | `crates/ripr/src/analysis/classify/transitive_reach.rs` |
| Classifier export | `crates/ripr/src/analysis/classify/mod.rs` |
| Diff/repo-mode wiring | `crates/ripr/src/analysis/language/rust.rs` |
| Human witness and limitation-detail projection | `crates/ripr/src/output/human/sections.rs` |
| Output contract docs | `docs/OUTPUT_SCHEMA.md` |
| Pure fixture | `fixtures/rust_macro_reach_limitation/` |
| Honesty corpus case | `fixtures/evidence-promotion-honesty-corpus/corpus.json` |

## CI Proof

- `cargo test -p ripr analysis::classify::transitive_reach`
- `cargo test -p ripr analysis::language::rust`
- `cargo xtask fixtures rust_macro_reach_limitation`
- `cargo xtask check-evidence-promotion-honesty`
- `cargo xtask check-output-contracts`
- `cargo xtask check-static-language`
- `cargo xtask check-spec-format`
- `cargo xtask check-spec-numbering`
- `cargo xtask check-traceability`
- `cargo fmt --check`

## Metrics

- Golden fixture pass: `fixtures/rust_macro_reach_limitation`.
- Corpus invariant: `rust_macro_reach_named_limitation` emits
  `rust_macro_reach_unresolved`, discloses a witness and limitation detail, and remains
  non-promoted at `no_static_path`.
- Existing transitive-reach fixture behavior remains unchanged.
