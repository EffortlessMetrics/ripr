# Fixture: guarded_result_match_owner_observation

Spec: RIPR-SPEC-0175

## Given

A production owner `expect_response` returning
`Result<Response, Box<dyn Error>>` (the reduced #13162 consumer shape), and
one related test that guards the direct helper result with an explicit
Result match: the Ok arm asserts exact response fields, the Err arm pins
the exact error variant via `matches!(error.downcast_ref::<ParseError>(),
Some(ParseError::InvalidData { .. }))` and panics otherwise.

## When

```bash
cargo xtask fixtures guarded_result_match_owner_observation
```

or:

```bash
ripr check --root fixtures/guarded_result_match_owner_observation/input --diff fixtures/guarded_result_match_owner_observation/diff.patch --mode fast
```

## Then

The producer-owned seam (`error_path`/`return_value` probes of
`expect_response`) credits the `guarded_result_match` oracle bound to the
changed owner without any changed-line token overlap; the probe whose
propagation witness completes reads `exposed`, with the related-test
evidence naming `match expect_response(..) { Ok(..) => .., Err(..) =>
Some(ParseError::InvalidData { .. }) }`. The file-fact cache generation
bump keeps warm pre-extension caches out of the run.

## Must Not

- Credit any oracle from a bare `?`, a terminal `Ok` alone, nearby tokens,
  or diagnostic strings.
- Flatten a downcast-only type pin to `exposed` (that shape stays below
  `exposed`; see `guarded_result_match_fail_closed` and the scanner tests).
- Use mutation-runtime outcome vocabulary reserved for real mutation
  execution.
- Re-introduce the accidental whole-match capture through the
  mock-expectation name sniff; the dedicated scanner owns the statement.
