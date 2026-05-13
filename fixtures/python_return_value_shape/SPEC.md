# Fixture: python_return_value_shape

Spec: RIPR-SPEC-0028

## Given

A Python production function changes a returned value expression.

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

The Python preview adapter classifies the changed line as
`probe.family = "return_value"` with `probe.delta = "value"`.

## Must Not

- Coerce a changed `return ...` line to a predicate probe.
- Run Python tests or inspect runtime values.
