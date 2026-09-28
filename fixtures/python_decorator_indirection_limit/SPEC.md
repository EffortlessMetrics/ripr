# Fixture: python_decorator_indirection_limit

Spec: RIPR-SPEC-0028

## Given

A Python production owner is decorated, and the changed line is otherwise a
plain return-value shape.

The fixture workspace enables the Python preview adapter explicitly:

```toml
[languages]
enabled = ["rust", "python"]
```

## When

```bash
cargo xtask fixtures python_decorator_indirection_limit
```

or:

```bash
ripr check \
  --root fixtures/python_decorator_indirection_limit/input \
  --diff fixtures/python_decorator_indirection_limit/diff.patch \
  --mode fast
```

## Then

The Python preview adapter:

- finds the `fetch_total` function owner,
- records the owner decorator as syntactic context,
- emits `static_limit_kind = "decorator_indirection"`,
- keeps the finding preview/advisory.

## Must Not

- Resolve decorator runtime behavior.
- Execute pytest.
- Treat decorator wrapping as hidden analyzer truth.
