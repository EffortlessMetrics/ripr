# Fixture: rust_missing_discriminator_evidence

Spec: RIPR-SPEC-0125

## Given

Rust production code changes one error variant while the only related test
asserts the broad `is_err()` property. The minimized control has no
producer-owned exact discriminator, and the governed pilot mapping records the
four real repository heads that currently stop at the same limitation.

The fixture is an analyzer control and evidence inventory. It is not a governed
repair attempt and does not authorize source edits in any adopting repository.

## When

```bash
cargo xtask fixtures rust_missing_discriminator_evidence
```

## Then

`ripr` preserves the exact error-variant change, the broad oracle, and the
missing discriminator as static evidence. The pilot mapping keeps each exact
repository/head/evidence reference separate so later producer work can prove
which candidate gained a complete discriminator.

## Must Not

- Claim that `is_err()` observes the exact error variant.
- Infer a test target, assertion, verify command, or receipt command from path
  or line proximity.
- Count a fixture result as a governed real-repository repair attempt.
- Treat the four mapped heads as one candidate or as evidence of an eligible
  route.
- Convert the generic limitation into a resolved producer fact before the
  analyzer supplies one.
