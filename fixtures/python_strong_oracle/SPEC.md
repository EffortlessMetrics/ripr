# Fixture: python_strong_oracle

Spec: RIPR-SPEC-0028

## Given

A Python production function changes a predicate boundary, and a pytest test
calls the changed owner with an exact-value assertion.

The fixture workspace enables the Python preview adapter explicitly:

```toml
[languages]
enabled = ["rust", "python"]
```

## When

```bash
cargo xtask fixtures python_strong_oracle
```

or:

```bash
ripr check \
  --root fixtures/python_strong_oracle/input \
  --diff fixtures/python_strong_oracle/diff.patch \
  --mode fast
```

## Then

The Python preview adapter:

- finds the `apply_discount` function owner,
- finds the pytest `test_apply_discount_boundary` related test,
- records the `assert ... == ...` oracle as `exact_value` / `strong`,
- emits `language = "python"`, `language_status = "preview"`, and
  `owner_kind = "function"`,
- classifies the changed predicate line as `exposed` because the related test
  has a strong syntax-first oracle.

## Must Not

- Run pytest or any Python runtime.
- Require an import graph, `mypy`, `pyright`, or a virtualenv.
- Claim runtime mutation-test vocabulary.
