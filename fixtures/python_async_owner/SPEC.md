# Fixture: python_async_owner

Spec: RIPR-SPEC-0028

## Given

An `async def` Python owner changes a return expression and an async pytest
test calls the owner.

## When

```bash
cargo xtask fixtures python_async_owner
```

## Then

The Python preview adapter recognises the async function as a `function`
owner and emits preview-labeled related-test evidence.

## Must Not

- Run an async runtime.
- Treat awaited assertions as calibrated runtime evidence.
