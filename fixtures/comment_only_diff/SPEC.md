# Fixture: comment_only_diff

Spec: RIPR-SPEC-0002

## Given

A Rust diff only adds a comment above `discounted_total`. Executable syntax is
unchanged.

## When

```bash
cargo xtask fixtures comment_only_diff
```

or:

```bash
ripr check --root fixtures/comment_only_diff/input --diff fixtures/comment_only_diff/diff.patch --mode fast
```

## Then

`ripr` should not report changed behavior for a comment-only diff.

## Must Not

- Use mutation-runtime outcome vocabulary reserved for real mutation execution.
- Treat the comment text as a predicate, return value, or oracle.
- Recommend a targeted test for non-behavioral text.
