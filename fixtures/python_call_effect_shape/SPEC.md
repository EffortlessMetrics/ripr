# Fixture: python_call_effect_shape

Spec: RIPR-SPEC-0028

## Given

A Python production function changes a bare method call statement.

The fixture workspace enables the Python preview adapter explicitly.

## When

```bash
cargo xtask fixtures python_call_effect_shape
```

or:

```bash
ripr check \
  --root fixtures/python_call_effect_shape/input \
  --diff fixtures/python_call_effect_shape/diff.patch \
  --mode fast
```

## Then

The Python preview adapter classifies the changed line as
`probe.family = "side_effect"` with `probe.delta = "effect"`.

## Must Not

- Treat a bare call effect as a return-value probe.
- Inspect the notifier object at runtime.
