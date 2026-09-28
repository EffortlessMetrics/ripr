# Fixture: python_pytest_raises

Spec: RIPR-SPEC-0028

## Given

A Python production function changes an error-path predicate, and a pytest test
calls the changed owner inside `pytest.raises(...)`.

The fixture workspace enables the Python preview adapter explicitly:

```toml
[languages]
enabled = ["rust", "python"]
```

## When

```bash
cargo xtask fixtures python_pytest_raises
```

or:

```bash
ripr check \
  --root fixtures/python_pytest_raises/input \
  --diff fixtures/python_pytest_raises/diff.patch \
  --mode fast
```

## Then

The Python preview adapter:

- finds the `normalize_quantity` function owner,
- finds the pytest `test_rejects_zero_quantity` related test,
- records the `pytest.raises(...)` context manager as `broad_error` / `weak`,
- emits Python preview metadata,
- keeps the finding `weakly_exposed` because broad error evidence is not an
  exact-value discriminator.

## Must Not

- Execute pytest.
- Resolve imports or exception inheritance.
- Treat broad error evidence as gate authority.
