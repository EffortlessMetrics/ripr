# Fixture: python_boundary_gap

Spec: RIPR-SPEC-0028

## Given

A Python production function changes a predicate from:

```python
amount > threshold
```

to:

```python
amount >= threshold
```

A pytest-style test file calls the changed owner but has no extracted assertion
oracle.

The fixture workspace enables the Python preview adapter explicitly:

```toml
[languages]
enabled = ["rust", "python"]
```

## When

```bash
cargo xtask fixtures python_boundary_gap
```

or:

```bash
ripr check \
  --root fixtures/python_boundary_gap/input \
  --diff fixtures/python_boundary_gap/diff.patch \
  --mode fast
```

## Then

The Python preview adapter:

- finds the `apply_discount` owner in `src/discount.py`,
- finds the pytest-style `test_apply_discount_smoke` test that references
  `apply_discount(`,
- emits `language = "python"`, `language_status = "preview"`, and
  `owner_kind = "function"`,
- classifies the changed predicate line as `weakly_exposed` because a related
  test reaches the owner but no strong assertion oracle is visible.

## Must Not

- Run pytest or any Python runtime.
- Require an import graph, `mypy`, `pyright`, or a virtualenv.
- Claim runtime mutation-test vocabulary.
