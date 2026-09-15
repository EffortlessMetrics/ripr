# Fixture: rust_constructor_field_observation

Spec: RIPR-SPEC-0005

## Given

A same-crate test calls `lower_ast`, which delegates through `lower_body` to
the private `lower_statement` constructor owner. The test asserts the exact
changed field through `statement.storage`.

## When

```bash
cargo xtask fixtures rust_constructor_field_observation
```

## Then

The diff-scoped finding remains conservative because it does not resolve this
same-crate caller chain. Repo-scoped grip evidence links the test through the
bounded same-crate caller graph and credits the exact-field oracle. The
`storage` field-construction seam is strongly gripped in the repo-scoped path.

## Must Not

- Treat the source line or a same-named sibling field as identity evidence.
- Credit a test that only asserts `name` as observing `storage`.
- Claim runtime mutation adequacy.
