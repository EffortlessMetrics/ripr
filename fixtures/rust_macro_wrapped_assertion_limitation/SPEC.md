# Fixture: rust_macro_wrapped_assertion_limitation

Spec: RIPR-SPEC-0120

## Given

A Rust crate where an integration test directly reaches the changed owner, then
uses a local assertion-like macro that RIPR does not classify as an oracle.

## When

```bash
cargo xtask fixtures rust_macro_wrapped_assertion_limitation
```

or:

```bash
ripr check --root fixtures/rust_macro_wrapped_assertion_limitation/input --diff fixtures/rust_macro_wrapped_assertion_limitation/diff.patch --mode fast
```

## Then

`ripr` should keep the finding reachable-but-undiscriminated and emit
`static_limit_kind: "rust_macro_wrapped_assertion_unresolved"` with limitation
detail naming the assertion macro edge.

## Must Not

- Promote the custom macro assertion as a strong oracle.
- Emit a repair packet for this limitation.
- Claim the assertion macro discriminates the changed behavior.
- Claim the result is clean.
