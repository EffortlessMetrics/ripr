# Fixture: python_error_path_shape

Spec: RIPR-SPEC-0028

## Given

A Python production function changes a raised error-path statement.

The fixture workspace enables the Python preview adapter explicitly.

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

The Python preview adapter classifies the changed line as
`probe.family = "error_path"` with `probe.delta = "control"`.

## Must Not

- Claim exact runtime error adequacy from syntax-first evidence.
- Run pytest or inspect exception payloads.
