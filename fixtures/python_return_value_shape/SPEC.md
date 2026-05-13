# Fixture: python_return_value_shape

Spec: RIPR-SPEC-0028

## Given

A Python production function changes a returned value, and a pytest test calls
the owner with an exact-value assertion.

The fixture workspace enables the Python preview adapter explicitly:

```toml
[languages]
enabled = ["rust", "python"]
```

## When

```bash
cargo xtask fixtures python_return_value_shape
```

or:

```bash
ripr check \
  --root fixtures/python_return_value_shape/input \
  --diff fixtures/python_return_value_shape/diff.patch \
  --mode fast
```

## Then

The Python preview adapter:

- finds the `final_price` function owner,
- classifies the changed line as a `return_value` / `value` probe,
- records the related exact-value pytest assertion,
- emits Python preview metadata.

## Must Not

- Execute pytest.
- Infer runtime value flow beyond syntax-first probe shape.
- Change Rust default behavior.
