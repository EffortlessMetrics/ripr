# Fixture: python_no_projectable_owner

Spec: RIPR-SPEC-0028

## Given

A Python diff changes a comment in a file with no projectable owner.

## When

```bash
cargo xtask fixtures python_no_projectable_owner
```

## Then

The Python preview adapter counts the changed Python file but emits no
finding instead of inventing an owner.

## Must Not

- Attach the comment to a nearby or synthetic owner.
- Emit preview diagnostics for unowned comments.
