# RIPR-SPEC-0120: Rust Macro-Wrapped Assertion Limitation

Status: accepted

Owner: product / swarm

Created: 2026-06-20

Linked issues:

- First-run-trust / P3 Rust macro limitation taxonomy: name custom
  assertion-like macro observers separately from reach-blocking macro calls.

Linked PRs:

- This PR

Support-tier impact:

- None for support-tier status. This spec adds one additive
  `static_limit_kind` value, `rust_macro_wrapped_assertion_unresolved`.
- Classification stays reachable-but-undiscriminated. The value names an
  unresolved assertion macro, not a reach claim, oracle claim, repair packet, or
  macro expansion proof.
- No `schema_version` bump is required because `static_limit_kind` is already
  additive optional finding metadata.

Policy impact:

- Register this spec in `policy/doc-artifacts.toml`.
- Update `docs/OUTPUT_SCHEMA.md`, `docs/STATIC_LIMITS.md`, and
  `.ripr/traceability.toml`.
- No new crates, dependencies, workflow permissions, runtime executors, macro
  expansion, rustc driver integration, or LSP-specific behavior.

## Problem

RIPR can already recognize direct Rust owner calls in tests. A real repository
may then observe the result through a custom assertion macro such as
`assert_result!(actual, expected)`. RIPR does not expand or classify that macro,
so a plain `reachable_unrevealed` result hides the most useful first-run fact:
there is a related test, but the discriminator is behind an unsupported macro
observer.

## Behavior

After normal Rust classification, if a finding:

- is `reachable_unrevealed`;
- has related Rust tests;
- has no selected recognized oracle; and
- at least one related test body invokes an assertion-like macro that is not in
  RIPR's known assertion macro set,

then set:

```text
static_limit_kind = rust_macro_wrapped_assertion_unresolved
```

The finding also emits witness and structured limitation-detail evidence using
the shared prefixes for:

- last established edge;
- first unresolved edge;
- analyzer route;
- non-claim.

The analyzer route is:

```text
analysis/rust-macro-assertion-oracle
```

## Fail-Closed Boundaries

- Never promote the finding because a custom assertion macro exists.
- Never infer assertion semantics from a generic `assert_*!(...)` name.
- Never emit a repair packet or receipt command for this limitation.
- Never claim the macro expands, reaches, covers, tests, exercises, or
  discriminates the changed behavior.
- Known assertion macros such as `assert_eq!` remain handled by the existing
  oracle extractor, not by this limitation.

## Wire Format

| Field | Value |
|---|---|
| `classification` | `reachable_unrevealed` |
| `static_limit_kind` | `rust_macro_wrapped_assertion_unresolved` |
| `related_tests` | preserved from direct reach classification |
| `static_limitation.analyzer_route` | `analysis/rust-macro-assertion-oracle` |

## Acceptance Examples

1. **Custom assertion macro**: `tests/it.rs` directly calls changed owner
   `inner`, then invokes `assert_result!(result, 7)`. RIPR keeps the finding
   `reachable_unrevealed`, emits
   `static_limit_kind: rust_macro_wrapped_assertion_unresolved`, surfaces the
   assertion macro under "Where to look" and "Limitation detail", and emits no
   repair packet.

   Fixture: `fixtures/rust_macro_wrapped_assertion_limitation/`.

2. **Known assertion macro**: `assert_eq!(result, 7)` remains handled by the
   existing Rust assertion extractor. The new limitation does not fire for
   built-in assertion macros.

## Required Evidence

- `StaticLimitKind::RustMacroWrappedAssertionUnresolved` in
  `crates/ripr/src/domain/language.rs`.
- Rust adapter post-classification selection in
  `crates/ripr/src/analysis/language/rust.rs`.
- Unit guards for custom assertion macros and known assertion macro
  non-matches.
- Pure fixture golden for
  `fixtures/rust_macro_wrapped_assertion_limitation/`.
- Honesty corpus case expecting
  `rust_macro_wrapped_assertion_unresolved`, limitation detail,
  non-promotion, no verify command, no receipt command, and no repair packet.
- Output schema, static-limit documentation, spec index, doc-artifact ledger,
  LSP gap-artifact validation, and traceability entries updated.

## Test Mapping

- `crates/ripr/src/analysis/language/rust.rs::tests::macro_wrapped_assertion_limit_names_reachable_unobserved_assertion_macro`
- `crates/ripr/src/analysis/language/rust.rs::tests::macro_wrapped_assertion_limit_ignores_known_assertion_macros`
- `crates/ripr/src/domain/language.rs::tests::static_limit_kind_wire_strings_are_stable`
- `crates/ripr/src/domain/language.rs::tests::static_limit_kind_describe_is_present_and_distinct`
- `crates/ripr/src/lsp/gap_artifacts.rs::tests::validation_accepts_rust_macro_wrapped_assertion_static_limit_kind`
- `fixtures/rust_macro_wrapped_assertion_limitation/expected/check.json`
- `fixtures/evidence-promotion-honesty-corpus/corpus.json` case
  `rust_macro_wrapped_assertion_named_limitation`
- `cargo xtask check-evidence-promotion-honesty`

## Implementation Mapping

| Component | Location |
|---|---|
| Static limit enum and text | `crates/ripr/src/domain/language.rs` |
| Rust post-classification selection | `crates/ripr/src/analysis/language/rust.rs` |
| LSP gap-artifact known-kind validation | `crates/ripr/src/lsp/gap_artifacts.rs` |
| Output contract docs | `docs/OUTPUT_SCHEMA.md` |
| Static limit docs | `docs/STATIC_LIMITS.md` |
| Pure fixture golden | `fixtures/rust_macro_wrapped_assertion_limitation/` |
| Honesty corpus | `fixtures/evidence-promotion-honesty-corpus/corpus.json` |

## CI Proof

- `cargo test -p ripr macro_wrapped_assertion_limit`
- `cargo test -p ripr static_limit_kind`
- `cargo test -p ripr validation_accepts_rust_macro_wrapped_assertion_static_limit_kind`
- `cargo xtask fixtures rust_macro_wrapped_assertion_limitation`
- `cargo xtask check-evidence-promotion-honesty`
- `cargo xtask goldens check`
- `cargo xtask check-output-contracts`
- `cargo xtask check-static-language`
- `cargo xtask check-spec-format`
- `cargo xtask check-spec-numbering`
- `cargo xtask check-traceability`
- `cargo fmt --check`

## Metrics

- Corpus invariant: custom assertion-macro observers emit
  `rust_macro_wrapped_assertion_unresolved`, disclose witness and limitation
  detail, and remain non-promoted at `reachable_unrevealed`.
- Known Rust assertion macros continue through the existing oracle extractor,
  not this limitation.

## Non-Goals

- General macro expansion.
- Macro-aware promotion.
- Custom assertion macro body inspection.
- Type-directed assertion helper resolution.
- Public API reach improvements.
- External pinned-corpus execution.
