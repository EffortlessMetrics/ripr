# Fixture: python_class_method_owner

Spec: RIPR-SPEC-0028

## Given

A Python `@classmethod` owner changes a return value, and a pytest test calls the
class method.

The fixture workspace enables the Python preview adapter explicitly:

```toml
[languages]
enabled = ["rust", "python"]
```

## When

```bash
cargo xtask fixtures python_class_method_owner
```

or:

```bash
ripr check \
  --root fixtures/python_class_method_owner/input \
  --diff fixtures/python_class_method_owner/diff.patch \
  --mode fast
```

## Then

The Python preview adapter:

- finds the classmethod owner,
- emits `owner_kind = "class_method"`,
- relates the pytest test by syntactic classmethod call,
- keeps the evidence labeled as Python preview.

## Must Not

- Resolve class construction semantics.
- Treat `@classmethod` as decorator indirection.
- Add editor routing.
