# Fixture: python_missing_import_graph_limit

Spec: RIPR-SPEC-0028

## Given

A Python related test reaches the changed owner through an import alias.

The fixture workspace enables the Python preview adapter explicitly.

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

The Python preview finding emits `static_limit_kind = "missing_import_graph"`.

## Must Not

- Resolve project imports semantically.
- Treat import-alias syntax as runtime import-graph proof.
