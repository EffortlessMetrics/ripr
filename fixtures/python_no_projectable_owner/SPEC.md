# Fixture: python_no_projectable_owner

Spec: RIPR-SPEC-0028

## Given

A Python module-level constant changes, but the current preview adapter has no
projectable owner for that module-scope expression.

The fixture workspace enables the Python preview adapter explicitly:

```toml
[languages]
enabled = ["rust", "python"]
```

## When

```bash
cargo xtask fixtures python_no_projectable_owner
```

or:

```bash
ripr check \
  --root fixtures/python_no_projectable_owner/input \
  --diff fixtures/python_no_projectable_owner/diff.patch \
  --mode fast
```

## Then

The Python preview adapter emits no finding rather than attaching the changed
module expression to the wrong owner.

## Must Not

- Guess a function owner for a module-level constant.
- Cross-route to tests by text mention alone.
- Add editor routing.
