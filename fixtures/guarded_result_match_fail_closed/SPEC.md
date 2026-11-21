# Fixture: guarded_result_match_fail_closed

Spec: RIPR-SPEC-0175

## Given

The same production owner shape as
`guarded_result_match_owner_observation`, and a test file whose guarded
Result matches all fail the bounded grammar: a guard over a different
helper (`other_helper`), a variable-binding scrutinee
(`let result = expect_response(..); match result`), a shadowed callee
(`let expect_response = other_helper;`), a message-only error predicate
(`error.to_string().contains("ParseError::InvalidData")`), and a
swallowed-error Err arm (downcast binding without any failure action).

## When

```bash
cargo xtask fixtures guarded_result_match_fail_closed
```

or:

```bash
ripr check --root fixtures/guarded_result_match_fail_closed/input --diff fixtures/guarded_result_match_fail_closed/diff.patch --mode fast
```

## Then

No `guarded_result_match` oracle is emitted for any of these shapes, no
probe of `expect_response` is credited through them, and every finding
stays at `weakly_exposed` or below (the existing weaker meaning of broad
assertions and bare Result plumbing). The wrong-owner match produces no
oracle at all even though its own Err arm pins an exact variant.

## Must Not

- Credit the owner's seam through a wrong-owner, variable-bound, or
  shadowed scrutinee.
- Infer an exact variant from a diagnostic string mention.
- Credit a swallowed or non-terminal failure arm.
- Report the file clean: the tests do reach the owner, so findings remain
  with non-promoting evidence.
