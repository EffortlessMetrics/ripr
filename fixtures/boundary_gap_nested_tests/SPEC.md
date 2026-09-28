# Fixture: boundary_gap_nested_tests

Spec: RIPR-SPEC-0002

## Given

This is a syntax/topology variant of `boundary_gap`: related tests live in a
nested `#[cfg(test)]` module inside `src/lib.rs`.

## When

```bash
cargo xtask fixtures boundary_gap_nested_tests
```

or:

```bash
ripr check --root fixtures/boundary_gap_nested_tests/input --diff fixtures/boundary_gap_nested_tests/diff.patch --mode fast
```

## Then

`ripr` should keep related-test and missing-boundary evidence despite the nested
test layout.

## Must Not

- Use mutation-runtime outcome vocabulary reserved for real mutation execution.
- Lose related tests only because they are nested under `#[cfg(test)]`.
- Hide the missing equality-boundary discriminator.
