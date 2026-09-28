# Fixture: format_only_diff

Spec: RIPR-SPEC-0002

## Given

A Rust diff only adds whitespace in `discounted_total`. The predicate and
returned values are unchanged.

## When

```bash
cargo xtask fixtures format_only_diff
```

or:

```bash
ripr check --root fixtures/format_only_diff/input --diff fixtures/format_only_diff/diff.patch --mode fast
```

## Then

`ripr` should not produce a scary behavior-exposure finding for whitespace-only
syntax drift.

## Must Not

- Use mutation-runtime outcome vocabulary reserved for real mutation execution.
- Claim that a behavior discriminator is missing for a whitespace-only diff.
- Infer a changed predicate, error variant, field, or effect.
