# Fixture: opaque_fixture_builder

Spec: RIPR-SPEC-0009

## Given

Production code changes the discount predicate from:

```rust
amount > 100
```

to:

```rust
amount >= 100
```

The related test reaches the changed owner through an exact assertion, but the
input values come from a fixture/builder helper instead of literals in the test
body.

## When

```bash
cargo xtask fixtures opaque_fixture_builder
```

or:

```bash
ripr check --root fixtures/opaque_fixture_builder/input --diff fixtures/opaque_fixture_builder/diff.patch --mode fast
```

## Then

`ripr` keeps the seam visible and explains that activation/infection is unknown.
The output should include the `fixture_opaque` stop reason and recommend either
adding a targeted boundary test or teaching `ripr` about the fixture/builder.

## Must Not

- Use runtime mutation outcome vocabulary.
- Treat the fixture/builder path as exact boundary coverage.
- Hide the recommendation to add a targeted boundary test or configure the
  fixture/builder policy.
