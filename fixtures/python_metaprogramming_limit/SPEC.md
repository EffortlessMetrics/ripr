# Fixture: python_metaprogramming_limit

Spec: RIPR-SPEC-0028

## Given

A Python production function changes a runtime type-construction expression.

The fixture workspace enables the Python preview adapter explicitly:

```toml
[languages]
enabled = ["rust", "python"]
```

## When

```bash
cargo xtask fixtures python_metaprogramming_limit
```

or:

```bash
ripr check \
  --root fixtures/python_metaprogramming_limit/input \
  --diff fixtures/python_metaprogramming_limit/diff.patch \
  --mode fast
```

## Then

The Python preview adapter:

- finds the `build_model` function owner,
- records exact-value related-test evidence,
- emits `static_limit_kind = "metaprogramming"`,
- keeps the finding preview/advisory.

## Must Not

- Infer runtime-created class behavior.
- Execute pytest.
- Treat metaprogramming as ordinary return-value semantics.
