# Fixture: python_call_argument_shape

Spec: RIPR-SPEC-0028

## Given

A Python production function changes a call argument, and a pytest test calls
the owner and checks the mock call shape.

The fixture workspace enables the Python preview adapter explicitly:

```toml
[languages]
enabled = ["rust", "python"]
```

## When

```bash
cargo xtask fixtures python_call_argument_shape
```

or:

```bash
ripr check \
  --root fixtures/python_call_argument_shape/input \
  --diff fixtures/python_call_argument_shape/diff.patch \
  --mode fast
```

## Then

The Python preview adapter:

- finds the `send_receipt` function owner,
- classifies the changed line as a `side_effect` / `effect` probe,
- records the related mock expectation,
- emits Python preview metadata.

## Must Not

- Execute pytest.
- Inspect mock framework runtime state.
- Add generated tests or source edits.
