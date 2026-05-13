# Fixture: python_isinstance_oracle

Spec: RIPR-SPEC-0028

## Given

A related pytest test reaches the changed Python owner with
`assert isinstance(...)`.

## When

```bash
cargo xtask fixtures python_isinstance_oracle
```

## Then

The Python preview adapter records a weak `relational_check` oracle rather than
exact-value evidence.

## Must Not

- Treat broad type checks as exact value checks.
- Run Python.
- Add editor routing or policy behavior.
