# Fixture: python_field_assignment_shape

Spec: RIPR-SPEC-0028

## Given

A Python class method changes an attribute assignment, and a pytest test calls
the method and checks the resulting field value.

The fixture workspace enables the Python preview adapter explicitly:

```toml
[languages]
enabled = ["rust", "python"]
```

## When

```bash
cargo xtask fixtures python_field_assignment_shape
```

or:

```bash
ripr check \
  --root fixtures/python_field_assignment_shape/input \
  --diff fixtures/python_field_assignment_shape/diff.patch \
  --mode fast
```

## Then

The Python preview adapter:

- finds the `Invoice.mark_paid` method owner,
- classifies the changed line as a `field_construction` / `value` probe,
- records the related exact-value pytest assertion,
- emits Python preview metadata and `owner_kind = "method"`.

## Must Not

- Execute pytest.
- Resolve descriptor or dataclass runtime semantics.
- Change editor or CI routing.
