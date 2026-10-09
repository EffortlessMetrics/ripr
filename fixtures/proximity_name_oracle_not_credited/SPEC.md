# Fixture: proximity_name_oracle_not_credited

Spec: RIPR-SPEC-0094

Owner: analysis-fixtures

Issue: #4486

## Given

The diff changes the error variant `try_parse` returns for input containing
`@`. `rejects_at_sign` calls `try_parse` directly but only asserts
`is_err()`, which both variants satisfy. `malformedsource_variant_is_distinct`
never calls `try_parse`; it is related only because its name contains the
changed token `MalformedSource`, and it compares the variant with itself.
`try_parse_variant_is_distinct` does the same and is related only because its
name contains the owner's name.
These assertions are intentional analyzed fixture input, governed by the
existing `fixtures/**` source-input policy.

Reverting the change leaves both tests passing.

## When

```bash
cargo xtask fixtures proximity_name_oracle_not_credited
```

The public diff analysis examines the changed return on `src/lib.rs:9`.

## Then

Both findings stay below `exposed`. Reach comes from the direct owner call;
the two name-only tests' exact assertions stay listed as related tests but
cannot supply the credited oracle, because neither test runs the changed
code.

The honesty corpus independently prohibits `exposed` even if a golden is
changed.

## Must Not

- Credit an oracle from a test related only by name when another related
  test supplies reach. Same-file and same-module tests are outside this rule:
  they commonly reach private helpers through the module's entry point.
- Claim sink identity, population accuracy, or runtime adequacy.
