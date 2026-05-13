# Fixture: python_class_method_owner

Spec: RIPR-SPEC-0028

## Given

A Python `@classmethod` changes a boundary inside a class owner and a test
calls the class method.

## When

```bash
cargo xtask fixtures python_class_method_owner
```

## Then

The Python preview adapter emits a finding for `Pricing.from_amount` with
`owner_kind: class_method` and preserves the decorator syntax as evidence.

## Must Not

- Resolve decorator behavior semantically.
- Route evidence through the editor.
