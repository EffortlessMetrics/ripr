# Fixture: assertion_form_parity_assert_msg

Spec: RIPR-SPEC-0154

## Given

A production owner `discounted_total` with a boundary predicate, and one
related test that checks the exact boundary value using the message-carrying `assert!(actual == expected, ...)`
harness form inside an ordinary integration test.

## When

```bash
cargo xtask fixtures assertion_form_parity_assert_msg
```

or:

```bash
ripr check --root fixtures/assertion_form_parity_assert_msg/input --diff fixtures/assertion_form_parity_assert_msg/diff.patch --mode fast
```

## Then

The credited oracle for the boundary seam is identical to the equivalent
`assert!`-form twin fixture (`assertion_form_parity_assert_msg`): same
oracle kind and strength, same classification, same gap accounting.
Equivalent harness assertion forms never change the production gap
denominator (#3284).

## Must Not

- Treat the Err-return guard or its assert! twin as an exact-value oracle stronger than the other form.
- Seed a production probe from the test file or the Result plumbing.
- Credit a broad or wrong-target assertion through either form.
- Use mutation-runtime outcome vocabulary reserved for real mutation execution.
