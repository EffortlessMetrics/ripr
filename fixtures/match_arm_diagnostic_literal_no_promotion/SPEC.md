# Fixture: match_arm_diagnostic_literal_no_promotion

Spec: RIPR-SPEC-0108

## Given

Production code routes `&str` keys to result literals. The changed arm is
`"sensor" => "sensor-v2"` (result changed from `sensor-v1`). The only
related test selects the sibling arm and mentions the changed pattern only
as assertion message text:

```rust
assert_eq!(route("focused-test"), "proof", "sensor");
```

A diagnostic string is not an owner-supplied input and cannot select an arm.

## When

```bash
cargo xtask fixtures match_arm_diagnostic_literal_no_promotion
```

or:

```bash
ripr check --root fixtures/match_arm_diagnostic_literal_no_promotion/input --diff fixtures/match_arm_diagnostic_literal_no_promotion/diff.patch --mode fast
```

## Then

The `match_arm` probe for `"sensor" => "sensor-v2",` reports
`weakly_exposed` (NOT `exposed`). The discriminate stage emits
`observation_unverified`: assertion diagnostic/message text never confirms
observation of the changed arm.

## Must Not

- Classify the changed sensor `match_arm` probe as `exposed` when the
  pattern literal appears only in assertion message text (the
  false-exposure family).
- Treat oracle message strings as changed-arm inputs or observations.
