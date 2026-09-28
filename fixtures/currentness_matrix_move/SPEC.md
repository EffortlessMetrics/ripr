# Fixture: currentness_matrix_move

Spec: RIPR-SPEC-0156

## Given

A helper function that calls the production owner is deleted entirely, with no same-text added line anywhere in the file (movement evidence absent).

## When

```bash
cargo xtask fixtures currentness_matrix_move
```

or:

```bash
ripr check --root fixtures/currentness_matrix_move/input --diff fixtures/NAME/diff.patch --mode fast
```

## Then

The removed-only probes are `base_deleted`: without same-text movement evidence the producer never claims a resolved move, and the records stay non-actionable base-side evidence.

## Must Not

- Emit a head-actionable gap, repair target, or diagnostic for any base-deleted record.
- Merge a deleted expression's identity with a different added expression at the same coordinate.
- Drop the production owner's ordinary classification for candidate-current changes.
- Use mutation-runtime outcome vocabulary reserved for real mutation execution.
