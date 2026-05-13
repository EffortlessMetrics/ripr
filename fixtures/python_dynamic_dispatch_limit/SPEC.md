# Fixture: python_dynamic_dispatch_limit

Spec: RIPR-SPEC-0028

## Given

A Python owner changes a dynamic attribute dispatch path with a direct related
pytest assertion.

The fixture workspace enables the Python preview adapter explicitly.

## When

```bash
cargo xtask fixtures python_dynamic_dispatch_limit
```

or:

```bash
ripr check \
  --root fixtures/python_dynamic_dispatch_limit/input \
  --diff fixtures/python_dynamic_dispatch_limit/diff.patch \
  --mode fast
```

## Then

The Python preview finding remains advisory and emits
`static_limit_kind = "dynamic_dispatch"`.

## Must Not

- Resolve the runtime dispatch target.
- Claim Rust-level confidence for the preview evidence.
