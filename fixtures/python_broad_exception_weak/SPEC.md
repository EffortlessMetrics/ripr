# Fixture: python_broad_exception_weak

Spec: RIPR-SPEC-0028

## Given

A related pytest test reaches a changed Python error path with
`pytest.raises(Exception)`.

## When

```bash
cargo xtask fixtures python_broad_exception_weak
```

## Then

The Python preview adapter records weak `broad_error` evidence and keeps the
finding advisory.

## Must Not

- Treat broad exception assertions as exact error variants.
- Run pytest.
- Add editor routing or policy behavior.
