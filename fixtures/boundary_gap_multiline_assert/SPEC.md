# Fixture: boundary_gap_multiline_assert

Spec: RIPR-SPEC-0002

## Given

This is a syntax variant of `boundary_gap`: the same boundary values are tested,
but the assertions are written across multiple lines.

## When

```bash
cargo xtask fixtures boundary_gap_multiline_assert
```

or:

```bash
ripr check --root fixtures/boundary_gap_multiline_assert/input --diff fixtures/boundary_gap_multiline_assert/diff.patch --mode fast
```

## Then

`ripr` should preserve the same static exposure intent as `boundary_gap` and
still name the missing equality-boundary discriminator.

## Must Not

- Use mutation-runtime outcome vocabulary reserved for real mutation execution.
- Lose related-test evidence only because an assertion spans multiple lines.
- Hide the missing equality-boundary discriminator.
