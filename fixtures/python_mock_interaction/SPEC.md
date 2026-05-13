# Fixture: python_mock_interaction

Spec: RIPR-SPEC-0028

## Given

A Python helper changes a syntactic mock interaction call.

The fixture workspace enables the Python preview adapter explicitly.

## When

```bash
cargo xtask fixtures python_mock_interaction
```

or:

```bash
ripr check \
  --root fixtures/python_mock_interaction/input \
  --diff fixtures/python_mock_interaction/diff.patch \
  --mode fast
```

## Then

The Python preview adapter classifies the changed mock interaction as a
`side_effect` probe with `effect` delta and keeps mock assertions as
interaction oracles.

## Must Not

- Treat mock interaction evidence as exact value evidence.
- Introspect the mock object at runtime.
