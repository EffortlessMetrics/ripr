# Fixture: source-role-corpus

Spec: RIPR-SPEC-0153 (producer-owned source-role authority; #3534)

## Given

Case directories under `cases/<name>/`, each a minimal crate slice with
source and manifest inputs covering one point of the source-role matrix:
ordinary production source, an explicit `[[test]]` target, a
naming-lookalike file (`src/price_test.rs`) that is not a test target,
and plain/conjunct/negated `cfg(test)` module variants.

## When

```bash
cargo test -p ripr --lib source_role_corpus
```

builds each case through `facts::build_index` and asserts the
producer-owned facts: executable-test membership, per-function
`FunctionSourceRole`, and the layout classification of each file
through `workspace::classify_with`.

## Then

- `production_lib`: one production subject, no executable tests;
- `explicit_test_target`: the `#[test]` function joins executable
  tests, the production function stays a production subject, and
  layout classification splits `src/` from `tests/`;
- `naming_lookalike`: a plain function in a test-named file stays a
  production subject (the name alone cannot establish role);
- `cfg_variants`: plain, conjunct, and negated cfg forms all reach
  executable tests.

Consumer-side role re-derivation is gated separately by
`cargo xtask check-rust-source-role-authority` (see the repository
README in this directory for the mutation-discrimination mapping).
