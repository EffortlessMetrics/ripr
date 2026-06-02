# Fixture: python_dynamic_import_limit

Spec: RIPR-SPEC-0028

## Given

A Python production function changes a call through a dynamically imported
module, and a pytest test calls the owner with an exact-value assertion.

The fixture workspace enables the Python preview adapter explicitly:

```toml
[languages]
enabled = ["rust", "python"]
```

## When

```bash
cargo xtask fixtures python_dynamic_import_limit
```

or:

```bash
ripr check \
  --root fixtures/python_dynamic_import_limit/input \
  --diff fixtures/python_dynamic_import_limit/diff.patch \
  --mode fast
```

## Then

The Python preview adapter:

- finds the `run_plugin` function owner,
- records exact-value related-test evidence,
- emits `static_limit_kind = "missing_import_graph"`,
- keeps the finding preview/advisory.

## Must Not

- Resolve dynamic import semantics.
- Execute Python imports or pytest.
- Emit a repair card, canonical repair gap, or agent packet.
