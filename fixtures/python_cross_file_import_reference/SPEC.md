# Fixture: python_cross_file_import_reference

Spec: RIPR-SPEC-0028

## Given

A Python production function changes a return value, and a pytest test imports
the changed owner under an alias before calling that alias.

The fixture workspace enables the Python preview adapter explicitly:

```toml
[languages]
enabled = ["rust", "python"]
```

## When

```bash
cargo xtask fixtures python_cross_file_import_reference
```

or:

```bash
ripr check \
  --root fixtures/python_cross_file_import_reference/input \
  --diff fixtures/python_cross_file_import_reference/diff.patch \
  --mode fast
```

## Then

The Python preview adapter:

- finds the `apply_tax` function owner,
- relates the pytest test by syntax-only import alias call,
- records the exact-value pytest assertion,
- emits Python preview metadata.

## Must Not

- Build or resolve an import graph.
- Execute pytest.
- Treat arbitrary text mentions as related tests.
