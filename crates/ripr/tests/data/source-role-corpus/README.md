# Rust source-role conformance corpus (#3534)

Retained inputs pinning the producer-owned source-role contract
(RIPR-SPEC-0153) against consumer-side drift. Each `cases/<name>/`
directory holds the source/manifest inputs; the expected producer facts
live as assertions in
`crates/ripr/src/analysis/source_role_corpus.rs`, which loads these
directories verbatim through `facts::build_index`.

## Cases and the failure class each pins

| Case | Matrix point | Historical failure class it discriminates |
| --- | --- | --- |
| `production_lib` | ordinary library production source | executable tests fabricated without attributes; production subject dropped |
| `explicit_test_target` | explicit `[[test]]` target with `#[test]` | executable test not registered; test target seeding production probes |
| `naming_lookalike` | confirmed naming convention vs unconfirmed lookalike (`src/price_test.rs`) | naming convention promoted to role authority |
| `cfg_variants` | plain, nested-conjunct, and negated `cfg(test)` forms | test-first-only or "any token named test" cfg recognition |

## Mutation discrimination

Restoring any of these historical failure classes makes the named check
fail:

- consumer re-derives test-file role from a path
  (`starts_with("tests…")`, `contains("/tests/")`, `ends_with("_test(s).rs")`)
  in production code → `cargo xtask
  check-rust-source-role-authority` (structural deny rules naming
  `rust_index::is_test_file` / `SourceRoleContext::classify_with`);
- an unapproved consumer calls `rust_index::is_test_file` → the same
  gate's consumer inventory;
- producer mutations (attribute/cfg recognition, executable-vs-helper
  collapse, evidence-role promotion) flip the corpus assertions above;
- string-literal cfg matching reintroduced in a producer → the
  `cfg_predicates` authority pins in `analysis/syntax/ra.rs` fail.

## Surface status

Exercised directly by the corpus today: fact normalization (parser),
executable-test membership, layout classification, harness/attribute
role producers (`facts::build_index`).

Documented role-agnostic or covered by dedicated surfaces: LSP
diagnostics (scope partition covered by
`analysis/workspace/source_role.rs` tests), SARIF/badge rendering
(paths render through `display_path`, role-agnostic), gap ledger and
agent packets (consume classified findings; role names ride the typed
facts), cache currentness (covered by
`facts::harness_registry::tests::cached_index_applies_registrations_identically`
and the facts-cache schema version), helper transfer and repair
targeting (covered by `classify` unit fixtures).
