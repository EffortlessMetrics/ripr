# Fixture: python_parametrize_preview

Spec: RIPR-SPEC-0028

## Given

A pytest `test_*` function uses `@pytest.mark.parametrize` while calling a
changed Python owner.

## When

```bash
cargo xtask fixtures python_parametrize_preview
```

## Then

The Python preview adapter preserves the parametrize decorator as syntactic
preview metadata and emits preview-labeled related-test evidence.

## Must Not

- Expand parametrized cases dynamically.
- Treat parametrization as runtime adequacy.
