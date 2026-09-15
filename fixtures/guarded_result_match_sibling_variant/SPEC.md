# Fixture: guarded_result_match_sibling_variant

Spec: RIPR-SPEC-0175

Issue: #3709

## Given

A producer whose changed line constructs the exact error variant
`Err(ParseError::InvalidData)`. The only related test guards the owner's
result with a guarded Result match whose Err arm pins the SIBLING variant
`ParseError::UnexpectedEof` on a bare scrutinee.

## When

```bash
cargo xtask fixtures guarded_result_match_sibling_variant
```

or:

```bash
ripr check --root fixtures/guarded_result_match_sibling_variant/input --diff fixtures/guarded_result_match_sibling_variant/diff.patch --mode fast
```

## Then

Both probes on the changed `return Err(ParseError::InvalidData);` line stay
`weakly_exposed` (NOT `exposed`). The guarded oracle binds the owner (bare
scrutinee) but its pin names a sibling variant, so the
`error_construction_variant` confirmation gate fails, the observation stays
`observation_unverified`, and the discriminate stage stays weak.

## Must Not

- Classify the seam `exposed` from a guarded match that pins a sibling
  variant of the same error type (the shared `ParseError` qualifier token
  is not a specificity signal).
- Let the owner-name binding alone confirm observation when the changed
  variant is known and unpinned.
