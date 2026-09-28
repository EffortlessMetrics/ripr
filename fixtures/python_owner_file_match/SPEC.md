# Fixture: python_owner_file_match

Spec: RIPR-SPEC-0028

## Given

A Python production function changes a return value, and a pytest test in a
different stem calls the changed owner directly.

The fixture workspace enables the Python preview adapter explicitly:

```toml
[languages]
enabled = ["rust", "python"]
```

## When

```bash
cargo xtask fixtures python_owner_file_match
```

or:

```bash
ripr check \
  --root fixtures/python_owner_file_match/input \
  --diff fixtures/python_owner_file_match/diff.patch \
  --mode fast
```

## Then

The Python preview adapter:

- finds the `calculate_total` function owner,
- relates the pytest test by direct syntactic owner call,
- records the exact-value pytest assertion,
- emits Python preview metadata.

## Must Not

- Execute pytest.
- Infer an import graph.
- Depend on same-stem file proximity for this relation.
