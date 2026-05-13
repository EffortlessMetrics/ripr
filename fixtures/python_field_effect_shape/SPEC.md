# Fixture: python_field_effect_shape

Spec: RIPR-SPEC-0028

## Given

A Python class method changes an attribute write.

The fixture workspace enables the Python preview adapter explicitly.

## When

```bash
cargo xtask fixtures python_field_effect_shape
```

or:

```bash
ripr check \
  --root fixtures/python_field_effect_shape/input \
  --diff fixtures/python_field_effect_shape/diff.patch \
  --mode fast
```

## Then

The Python preview adapter classifies the changed line as
`probe.family = "field_construction"` with `probe.delta = "value"` and
keeps the method owner kind visible.

## Must Not

- Treat attribute writes as generic predicates.
- Infer object state beyond the local syntax.
