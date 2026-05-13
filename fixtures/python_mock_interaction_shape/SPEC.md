# Fixture: python_mock_interaction_shape

Spec: RIPR-SPEC-0028

## Given

A Python production helper changes a syntactic `MagicMock(...)` initializer,
and a pytest test calls the owner and checks the mock interaction state.

The fixture workspace enables the Python preview adapter explicitly:

```toml
[languages]
enabled = ["rust", "python"]
```

## When

```bash
cargo xtask fixtures python_mock_interaction_shape
```

or:

```bash
ripr check \
  --root fixtures/python_mock_interaction_shape/input \
  --diff fixtures/python_mock_interaction_shape/diff.patch \
  --mode fast
```

## Then

The Python preview adapter:

- finds the `recording_callback` function owner,
- classifies the changed `MagicMock(...)` assignment as a `side_effect` /
  `effect` probe,
- records the related `assert_not_called()` mock expectation,
- emits Python preview metadata.

## Must Not

- Inspect mock runtime state.
- Resolve imports semantically.
- Treat mock expectation evidence as a strong discriminator.
