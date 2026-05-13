# Fixture: python_mock_not_called

Spec: RIPR-SPEC-0028

## Given

A related pytest test reaches a changed Python owner and checks
`assert_not_called()`.

## When

```bash
cargo xtask fixtures python_mock_not_called
```

## Then

The Python preview adapter records weak `mock_expectation` evidence.

## Must Not

- Upgrade negative mock interaction evidence to exact value evidence.
- Run pytest.
- Add editor routing or policy behavior.
