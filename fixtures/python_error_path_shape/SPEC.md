# Fixture: python_error_path_shape

Spec: RIPR-SPEC-0028

## Given

A Python production function changes an error-path raise statement, and a
pytest test calls the owner inside `pytest.raises(...)`.

The fixture workspace enables the Python preview adapter explicitly:

```toml
[languages]
enabled = ["rust", "python"]
```

## When

```bash
cargo xtask fixtures python_error_path_shape
```

or:

```bash
ripr check \
  --root fixtures/python_error_path_shape/input \
  --diff fixtures/python_error_path_shape/diff.patch \
  --mode fast
```

## Then

The Python preview adapter:

- finds the `require_positive` function owner,
- classifies the changed line as an `error_path` / `control` probe,
- records `pytest.raises(...)` as broad error evidence,
- emits Python preview metadata.

## Must Not

- Execute pytest.
- Resolve exception inheritance or runtime imports.
- Treat broad error evidence as a strong discriminator.
