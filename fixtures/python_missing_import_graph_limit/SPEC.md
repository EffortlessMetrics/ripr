# Fixture: python_missing_import_graph_limit

Spec: RIPR-SPEC-0028

## Given

A Python production function changes a call to an imported symbol.

The fixture workspace enables the Python preview adapter explicitly:

```toml
[languages]
enabled = ["rust", "python"]
```

## When

```bash
cargo xtask fixtures python_missing_import_graph_limit
```

or:

```bash
ripr check \
  --root fixtures/python_missing_import_graph_limit/input \
  --diff fixtures/python_missing_import_graph_limit/diff.patch \
  --mode fast
```

## Then

The Python preview adapter:

- finds the `total` function owner,
- records exact-value related-test evidence,
- emits `static_limit_kind = "missing_import_graph"`,
- keeps the finding preview/advisory.

## Must Not

- Resolve imported implementation semantics.
- Read Python package metadata.
- Execute pytest.
