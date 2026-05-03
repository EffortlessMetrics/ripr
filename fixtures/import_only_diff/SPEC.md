# Fixture: import_only_diff

Spec: RIPR-SPEC-0002

## Given

A Rust diff only adds an import. The production function body is unchanged.

## When

```bash
cargo xtask fixtures import_only_diff
```

or:

```bash
ripr check --root fixtures/import_only_diff/input --diff fixtures/import_only_diff/diff.patch --mode fast
```

## Then

`ripr` should not report changed behavior for an import-only diff.

## Must Not

- Use mutation-runtime outcome vocabulary reserved for real mutation execution.
- Treat the import as a changed predicate, return value, or side effect.
- Recommend adding a test for an import-only change.
