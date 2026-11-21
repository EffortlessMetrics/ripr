# Fixture: guarded_result_match_conditional_failure

Spec: RIPR-SPEC-0175

Issue: #3709

## Given

A producer whose changed line constructs the exact error variant
`Err(ParseError::InvalidData)`. Three related tests guard the owner's result
with guarded Result matches whose Err arms pin that exact variant but whose
bodies can return normally: a `panic!` nested behind an unrelated
condition, an `.unwrap()` on an unrelated value, and a `panic!` nested in a
closure.

## When

```bash
cargo xtask fixtures guarded_result_match_conditional_failure
```

or:

```bash
ripr check --root fixtures/guarded_result_match_conditional_failure/input --diff fixtures/guarded_result_match_conditional_failure/diff.patch --mode fast
```

## Then

No guarded Result match oracle is emitted: the bounded depth-0 terminal
grammar accepts only UNCONDITIONALLY diverging statements (or the
body-predicate failure form), so every shape here stays unrecognized and
the probes never read `exposed`.

## Must Not

- Credit a strong guarded-result oracle from a `panic!` that fires only
  when an unrelated condition holds.
- Credit `.unwrap()` or `.expect()` statements as the Err arm's failure
  action: the unwrapped value may be unrelated to the matched error.
- Credit failure markers nested inside closure or `if` blocks (depth > 0).
