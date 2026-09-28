# Fixture: python_unrelated_test_mention

Spec: RIPR-SPEC-0028

## Given

A Python production function changes a return value, and an unrelated pytest
test mentions the owner name only as text.

The fixture workspace enables the Python preview adapter explicitly:

```toml
[languages]
enabled = ["rust", "python"]
```

## When

```bash
cargo xtask fixtures python_unrelated_test_mention
```

or:

```bash
ripr check \
  --root fixtures/python_unrelated_test_mention/input \
  --diff fixtures/python_unrelated_test_mention/diff.patch \
  --mode fast
```

## Then

The Python preview adapter:

- finds the `apply_discount` function owner,
- does not relate the text-only mention as a test call,
- emits Python preview metadata with `no_static_path`.

## Must Not

- Treat string literals or prose mentions as related-test evidence.
- Execute pytest.
- Infer an import graph.
