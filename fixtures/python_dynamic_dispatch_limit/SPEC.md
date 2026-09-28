# Fixture: python_dynamic_dispatch_limit

Spec: RIPR-SPEC-0028

## Given

A Python production function changes a dynamic dispatch call through
`getattr(...)`, and a pytest test calls the owner with an exact-value
assertion.

The fixture workspace enables the Python preview adapter explicitly:

```toml
[languages]
enabled = ["rust", "python"]
```

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

The Python preview adapter:

- finds the `call_named` function owner,
- records exact-value related-test evidence,
- emits `static_limit_kind = "dynamic_dispatch"`,
- keeps the finding preview/advisory.

## Must Not

- Resolve the dynamic callee.
- Execute pytest.
- Downgrade strong related-test evidence because the static limit is visible.
