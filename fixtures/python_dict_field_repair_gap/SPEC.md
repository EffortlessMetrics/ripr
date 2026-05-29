# Fixture: python_dict_field_repair_gap

Spec: RIPR-SPEC-0028

## Given

A Python function changes a returned dict field, and a pytest test calls the
function but only checks that some payload exists.

The fixture workspace enables the Python preview adapter explicitly:

```toml
[languages]
enabled = ["rust", "python"]
```

## When

```bash
cargo xtask fixtures python_dict_field_repair_gap
```

or:

```bash
ripr check \
  --root fixtures/python_dict_field_repair_gap/input \
  --diff fixtures/python_dict_field_repair_gap/diff.patch \
  --mode fast
```

## Then

The Python preview adapter:

- finds the `invoice_payload` function owner,
- classifies the changed returned dict as a `field_construction` / `value`
  probe,
- keeps the broad pytest assertion weak,
- emits a field/object missing discriminator for `status == "paid"`.

## Must Not

- Execute pytest.
- Infer dataclass or serializer runtime semantics.
- Emit a full repair card, verify command, or receipt command.
