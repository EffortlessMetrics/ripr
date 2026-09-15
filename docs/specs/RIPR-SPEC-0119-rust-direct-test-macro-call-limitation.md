# RIPR-SPEC-0119: Rust Direct Test Macro-Call Limitation

Status: accepted

Owner: product / swarm

Created: 2026-06-20

Linked issues:

- First-run-trust / P3 Rust macro reachability lane: distinguish direct
  test-body macro invocation from a production entry point that later reaches a
  macro boundary.

Linked PRs:

- This PR

Support-tier impact:

- No tier change. This spec adds one additive `static_limit_kind` value,
  `rust_macro_wrapped_test_call_unresolved`.
- Classification stays `no_static_path`. The value names a limitation, not a
  reach claim, oracle claim, repair packet, or macro expansion proof.
- No `schema_version` bump is required because `static_limit_kind` is already
  additive optional finding metadata.
- Claim boundaries remain governed by the canonical ledger in
  [support tiers](../status/SUPPORT_TIERS.md).

Policy impact:

- Register this spec in `policy/doc-artifacts.toml`.
- Update `docs/OUTPUT_SCHEMA.md`, `docs/STATIC_LIMITS.md`, and
  `.ripr/traceability.toml`.
- No new crates, dependencies, workflow permissions, runtime executors, macro
  expansion, rustc driver integration, or LSP-specific behavior.

## Problem

RIPR-SPEC-0117 names unresolved same-repo macro boundaries as
`rust_macro_reach_unresolved`. That label covers two different first-run
questions:

- a test calls a production entry point and the production path stops at a
  macro;
- the test itself directly invokes a macro whose definition mentions the
  changed owner.

The second case should tell the operator that RIPR saw the test-body macro
call, but did not expand it. This is more precise than the generic macro reach
label and keeps the unsupported edge visible.

## Behavior

### Trigger condition

After direct related-test classification returns `no_static_path` with no
related tests, and after the bounded transitive witness check does not find a
lexical path, RIPR-SPEC-0117 may return a macro-reach witness. If that witness
comes from the test body itself, set:

```text
static_limit_kind = rust_macro_wrapped_test_call_unresolved
```

The current witness marks this shape with
`macro_host = "test body"` and an entry symbol such as `call_inner!`.

### When found

The existing RIPR-SPEC-0117 evidence remains unchanged:

- keep `classification = no_static_path`;
- keep `StopReason::MacroReachUnresolved`;
- keep the witness pointer in `evidence`;
- keep structured `static_limitation` detail when the edge fields are present.

Only the static limitation kind becomes more specific for direct test-body
macro calls.

### Fail-closed boundaries

- Never promote the finding.
- Never add the macro witness to `related_tests`.
- Never claim the macro expands, reaches, covers, tests, exercises, or
  discriminates the changed owner.
- Do not infer macro hygiene, visibility, generated items, trait dispatch, or
  type identity.
- If the macro witness is not from the test body, keep the existing
  `rust_macro_reach_unresolved` kind.

## Wire Format

| Field | Direct test-body macro witness | Production-entry macro witness |
|---|---|---|
| `classification` | `no_static_path` | `no_static_path` |
| `static_limit_kind` | `rust_macro_wrapped_test_call_unresolved` | `rust_macro_reach_unresolved` |
| `stop_reasons` | includes `macro_reach_unresolved` | includes `macro_reach_unresolved` |
| `related_tests` | unchanged; witness is not added | unchanged; witness is not added |
| `static_limitation.analyzer_route` | `analysis/rust-macro-aware-reach` | `analysis/rust-macro-aware-reach` |

## Non-Goals

- Macro expansion.
- Macro-aware promotion.
- Assertion-macro oracle extraction.
- Repair packet emission.
- Changing verify or receipt command availability.
- Changing the pinned external checkout model.

## Acceptance Examples

1. **Direct test macro invocation**: `tests/it.rs` invokes `call_inner!`, and
   the same-repo macro definition mentions changed owner `inner`. Result:
   `no_static_path` plus
   `static_limit_kind: rust_macro_wrapped_test_call_unresolved`.
   Fixture: `fixtures/rust_macro_wrapped_test_call_limitation/`.
2. **Production-entry macro boundary**: `tests/it.rs` calls `outer()`, and
   `outer()` invokes `call_inner!`. Result remains `no_static_path` plus
   `static_limit_kind: rust_macro_reach_unresolved`.
   Fixture: `fixtures/rust_macro_reach_limitation/`.

## Required Evidence

- `StaticLimitKind::RustMacroWrappedTestCallUnresolved` in
  `crates/ripr/src/domain/language.rs`.
- Rust adapter selection in `crates/ripr/src/analysis/language/rust.rs`.
- Unit guard proving test-body macro witnesses and production-entry macro
  witnesses select different limitation kinds.
- Pure fixture golden for
  `fixtures/rust_macro_wrapped_test_call_limitation/`.
- Honesty corpus assertions expecting
  `rust_macro_wrapped_test_call_unresolved` for the direct test macro fixture.
- Existing `fixtures/rust_macro_reach_limitation/` remains on
  `rust_macro_reach_unresolved`.
- Output schema, static-limit documentation, spec index, doc-artifact ledger,
  LSP gap-artifact validation, and traceability entries updated.

## Test Mapping

- `crates/ripr/src/analysis/language/rust.rs::tests::macro_reach_limit_kind_names_direct_test_body_macro_path`
- `crates/ripr/src/domain/language.rs::tests::static_limit_kind_wire_strings_are_stable`
- `crates/ripr/src/domain/language.rs::tests::static_limit_kind_describe_is_present_and_distinct`
- `crates/ripr/src/lsp/gap_artifacts.rs::tests::validation_accepts_rust_macro_wrapped_test_call_static_limit_kind`
- `fixtures/rust_macro_wrapped_test_call_limitation/expected/check.json`
- `cargo xtask check-evidence-promotion-honesty`

## Implementation Mapping

| Component | Location |
|---|---|
| Static limit enum and text | `crates/ripr/src/domain/language.rs` |
| Macro-host selection | `crates/ripr/src/analysis/language/rust.rs` |
| Macro witness test-body marker | `crates/ripr/src/analysis/classify/transitive_reach.rs` |
| LSP gap-artifact known-kind validation | `crates/ripr/src/lsp/gap_artifacts.rs` |
| Output contract docs | `docs/OUTPUT_SCHEMA.md` |
| Static limit docs | `docs/STATIC_LIMITS.md` |
| Pure fixture golden | `fixtures/rust_macro_wrapped_test_call_limitation/` |
| Honesty corpus | `fixtures/evidence-promotion-honesty-corpus/corpus.json` |

## CI Proof

- `cargo test -p ripr analysis::language::rust::tests::macro_reach_limit_kind_names_direct_test_body_macro_path`
- `cargo test -p ripr domain::language::tests::static_limit_kind_wire_strings_are_stable`
- `cargo test -p ripr lsp::gap_artifacts::tests::validation_accepts_rust_macro_wrapped_test_call_static_limit_kind`
- `cargo xtask fixtures rust_macro_wrapped_test_call_limitation`
- `cargo xtask fixtures rust_macro_reach_limitation`
- `cargo xtask check-evidence-promotion-honesty`
- `cargo xtask check-output-contracts`
- `cargo xtask check-static-language`
- `cargo xtask check-spec-format`
- `cargo xtask check-spec-numbering`
- `cargo xtask check-traceability`
- `cargo fmt --check`

## Metrics

- Corpus invariant: direct test-body macro witnesses emit
  `rust_macro_wrapped_test_call_unresolved`, disclose witness and limitation
  detail, and remain non-promoted at `no_static_path`.
- Generic production-entry macro witness behavior remains available as
  `rust_macro_reach_unresolved`.
