# Fixture: match_arm_shared_result_no_promotion

Spec: RIPR-SPEC-0108

## Given

Production code routes `&str` keys to result literals. The changed arm is
`"sensor" => "sensor-v2"` (result changed from `sensor-v1`). A sibling arm
`"focused-test" => "sensor-v2"` returns the same result literal. The only
related test selects the sibling arm:

```rust
assert_eq!(route("focused-test"), "sensor-v2");
```

This test never supplies the changed arm's pattern input (`"sensor"`) to the
owner, so it cannot discriminate the sensor result change.

## When

```bash
cargo xtask fixtures match_arm_shared_result_no_promotion
```

or:

```bash
ripr check --root fixtures/match_arm_shared_result_no_promotion/input --diff fixtures/match_arm_shared_result_no_promotion/diff.patch --mode fast
```

## Then

The `match_arm` probe for `"sensor" => "sensor-v2",` reports
`weakly_exposed` (NOT `exposed`). The discriminate stage emits
`observation_unverified`: a sibling call returning the same result literal
does not select the changed arm — only the pattern side establishes literal
arm identity, and only an owner-supplied pattern input confirms it.

## Must Not

- Classify the changed sensor `match_arm` probe as `exposed` when no test
  supplies `"sensor"` through the owner call (the false-exposure family).
- Treat a result-side literal overlap as observing the changed arm.
