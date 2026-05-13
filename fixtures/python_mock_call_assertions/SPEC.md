# Fixture: python_mock_call_assertions

Spec: RIPR-SPEC-0028

## Given

A related pytest test reaches a changed Python owner and verifies a mock call
with `assert_called_once_with(...)`.

## When

```bash
cargo xtask fixtures python_mock_call_assertions
```

## Then

The Python preview adapter records `mock_expectation` evidence without treating
interaction evidence as exact value observation.

## Must Not

- Inspect mock runtime behavior.
- Run pytest.
- Add editor routing or policy behavior.
