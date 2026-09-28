# Fixture: currentness_matrix_reuse

Spec: RIPR-SPEC-0156

## Given

A helper predicate rewritten in place: the diff removes one expression and adds a different expression at the same coordinate (replacement pairing).

## When

```bash
cargo xtask fixtures currentness_matrix_reuse
```

or:

```bash
ripr check --root fixtures/currentness_matrix_reuse/input --diff fixtures/NAME/diff.patch --mode fast
```

## Then

The replacement pairs: the added expression seeds the sole `candidate_current` probe with the added expression's content-addressed identity; the deleted expression contributes no probe and never merges its identity into the added finding (ids are expression-addressed, not line-addressed).

## Must Not

- Emit a head-actionable gap, repair target, or diagnostic for any base-deleted record.
- Merge a deleted expression's identity with a different added expression at the same coordinate.
- Drop the production owner's ordinary classification for candidate-current changes.
- Use mutation-runtime outcome vocabulary reserved for real mutation execution.
