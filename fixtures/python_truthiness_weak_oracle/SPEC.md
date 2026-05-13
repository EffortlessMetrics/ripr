# Fixture: python_truthiness_weak_oracle

Spec: RIPR-SPEC-0028

## Given

A related pytest test reaches the changed Python owner with a bare truthiness
assertion.

## When

```bash
cargo xtask fixtures python_truthiness_weak_oracle
```

## Then

The Python preview adapter keeps the finding `weakly_exposed` and records a
`smoke_only` oracle.

## Must Not

- Upgrade truthiness to exact-value evidence.
- Run Python.
- Add editor routing or policy behavior.
