# Fixture: unrelated_test_mentions_token

Spec: RIPR-SPEC-0002

## Given

Production code changes a discount boundary, but the only test mentions the word
`token` while calling a different helper. The test text should not be mistaken
for a related discriminator.

## When

```bash
cargo xtask fixtures unrelated_test_mentions_token
```

or:

```bash
ripr check --root fixtures/unrelated_test_mentions_token/input --diff fixtures/unrelated_test_mentions_token/diff.patch --mode fast
```

## Then

`ripr` should avoid treating unrelated test text as evidence for the changed
discount behavior.

## Must Not

- Use mutation-runtime outcome vocabulary reserved for real mutation execution.
- Claim the unrelated test discriminates the changed discount predicate.
- Hide stop reasons if the finding is unknown.
