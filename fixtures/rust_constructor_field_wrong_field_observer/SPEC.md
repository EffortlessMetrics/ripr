# Fixture: rust_constructor_field_wrong_field_observer

Spec: RIPR-SPEC-0005

## Given

A same-crate test reaches the constructor through `lower_ast` but asserts the
sibling `name` field rather than the changed `storage` field.

## When

```bash
cargo xtask fixtures rust_constructor_field_wrong_field_observer
```

## Then

RIPR must not promote the `storage` field-construction seam to strong exposure.
The test reaches the constructor, but its oracle does not discriminate the
changed field.

## Must Not

- Treat a strong assertion on `name` as an assertion on `storage`.
- Emit a repair-ready or clean result for the changed field.
- Claim runtime mutation adequacy.
