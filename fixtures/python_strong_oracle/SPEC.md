# Fixture: python_strong_oracle

Spec: RIPR-SPEC-0028

## Given

A Python function changes a boundary predicate, and a related pytest test calls
the owner with an exact-value assertion.

## When

```bash
cargo xtask fixtures python_strong_oracle
```

## Then

The Python preview adapter emits `language = "python"`,
`language_status = "preview"`, `owner_kind = "function"`, and strong
`exact_value` oracle evidence.

## Must Not

- Run Python.
- Claim parity with Rust evidence.
- Add editor routing or policy behavior.
