# Fixture: python_unsupported_syntax_limit

Spec: RIPR-SPEC-0028

## Given

A Python production function changes a lambda-return shape that the preview
adapter does not model precisely yet.

The fixture workspace enables the Python preview adapter explicitly:

```toml
[languages]
enabled = ["rust", "python"]
```

## When

```bash
cargo xtask fixtures python_unsupported_syntax_limit
```

or:

```bash
ripr check \
  --root fixtures/python_unsupported_syntax_limit/input \
  --diff fixtures/python_unsupported_syntax_limit/diff.patch \
  --mode fast
```

## Then

The Python preview adapter:

- finds the `make_mapper` function owner,
- records exact-value related-test evidence,
- emits `static_limit_kind = "unsupported_syntax"`,
- keeps the finding preview/advisory.

## Must Not

- Treat lambda capture semantics as resolved.
- Execute pytest.
- Hide the unsupported syntax boundary.
