# Fixture: python_async_owner

Spec: RIPR-SPEC-0028

## Given

A Python `async def` production owner changes a return value, and an async
pytest-style test calls that owner with an exact-value assertion.

The fixture workspace enables the Python preview adapter explicitly:

```toml
[languages]
enabled = ["rust", "python"]
```

## When

```bash
cargo xtask fixtures python_async_owner
```

or:

```bash
ripr check \
  --root fixtures/python_async_owner/input \
  --diff fixtures/python_async_owner/diff.patch \
  --mode fast
```

## Then

The Python preview adapter:

- finds the async function owner,
- keeps the finding labeled as Python preview evidence,
- relates the async test by syntactic owner call,
- records the exact-value assertion.

## Must Not

- Execute pytest or an event loop.
- Treat async syntax as runtime proof.
- Add editor routing.
