# Fixture: python_disabled

Spec: RIPR-SPEC-0028

## Given

A Python production owner changes and a pytest test would otherwise be
projectable, but repo configuration leaves Python disabled:

```toml
[languages]
enabled = ["rust"]
```

## When

```bash
cargo xtask fixtures python_disabled
```

or:

```bash
ripr check \
  --root fixtures/python_disabled/input \
  --diff fixtures/python_disabled/diff.patch \
  --mode fast
```

## Then

No Python preview findings are emitted.

## Must Not

- Run Python preview analysis by default.
- Emit Python diagnostics or reports while Python is disabled.
- Add editor routing.
