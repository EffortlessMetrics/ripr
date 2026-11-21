# Fixture: boundary_gap_reordered_tests

Spec: RIPR-SPEC-0002

## Given

This is an ordering variant of `boundary_gap`: the related tests are written in
the opposite source order.

## When

```bash
cargo xtask fixtures boundary_gap_reordered_tests
```

or:

```bash
ripr check --root fixtures/boundary_gap_reordered_tests/input --diff fixtures/boundary_gap_reordered_tests/diff.patch --mode fast
```

## Then

`ripr` should keep deterministic related-test and missing-discriminator evidence.

## Must Not

- Use mutation-runtime outcome vocabulary reserved for real mutation execution.
- Let test declaration order change the exposure class.
- Hide the missing equality-boundary discriminator.
