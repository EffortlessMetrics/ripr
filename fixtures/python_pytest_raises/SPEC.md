# Fixture: python_pytest_raises

Spec: RIPR-SPEC-0028

## Given

A changed Python error path is covered by `pytest.raises(ValueError)`.

## When

```bash
cargo xtask fixtures python_pytest_raises
```

## Then

The Python preview adapter records strong `exact_error_variant` oracle evidence.

## Must Not

- Run pytest.
- Claim runtime mutation-test evidence.
- Add editor routing or policy behavior.
