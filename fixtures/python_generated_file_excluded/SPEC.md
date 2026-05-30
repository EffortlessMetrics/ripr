# Fixture: python_generated_file_excluded

Spec: RIPR-SPEC-0028

## Given

A Python diff changes a detectable generated file (`*_pb2.py`) in a preview
workspace that also contains a pytest test importing that generated module.

The fixture workspace enables the Python preview adapter explicitly:

```toml
[languages]
enabled = ["rust", "python"]
```

## When

```bash
cargo xtask fixtures python_generated_file_excluded
```

or:

```bash
ripr check \
  --root fixtures/python_generated_file_excluded/input \
  --diff fixtures/python_generated_file_excluded/diff.patch \
  --mode fast
```

## Then

The Python preview adapter excludes the generated Python file from diff
analysis, so no Python preview finding, repair card, canonical repair gap, or
swarm packet can be routed from the generated-code edit.

## Must Not

- Treat a generated Python file edit as a repairable source behavior change.
- Emit a Python repair card for `*_pb2.py`.
- Execute pytest, import the generated module, or infer runtime behavior.
