# Fixture: currentness_matrix_tail

Spec: RIPR-SPEC-0156

## Given

A production owner with a boundary predicate and a same-file helper whose tail (a removed-only predicate and its branch body) is deleted by the diff while the production owner is unchanged.

## When

```bash
cargo xtask fixtures currentness_matrix_tail
```

or:

```bash
ripr check --root fixtures/currentness_matrix_tail/input --diff fixtures/NAME/diff.patch --mode fast
```

## Then

Every removed-only probe is `base_deleted`: retained as labelled base-side evidence at its projected coordinate, never a head-actionable gap, repair target, or diagnostic. The #3212 incident shape (a deleted expression whose candidate file no longer contains it) stays non-blocking on every surface.

## Must Not

- Emit a head-actionable gap, repair target, or diagnostic for any base-deleted record.
- Merge a deleted expression's identity with a different added expression at the same coordinate.
- Drop the production owner's ordinary classification for candidate-current changes.
- Use mutation-runtime outcome vocabulary reserved for real mutation execution.
