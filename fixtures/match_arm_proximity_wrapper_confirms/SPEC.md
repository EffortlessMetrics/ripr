# Fixture: match_arm_proximity_wrapper_confirms

Spec: RIPR-SPEC-0094

Owner: analysis-fixtures

Issue: #6297

## Given

The diff changes the value of the `Unit::Fortnight =>` arm in `seconds`.
`seconds_total` calls `seconds` and asserts an exact sum that never names the
arm. `bridge_fortnight` shares the file and asserts
`seconds_bridge(Unit::Fortnight) == 1_209_600`; `seconds_bridge` is a public
wrapper that calls `seconds`. `from_str_fortnight` calls only `Unit::from_str`.
These assertions are intentional analyzed fixture input, governed by the
existing `fixtures/**` source-input policy.

## When

```bash
cargo xtask fixtures match_arm_proximity_wrapper_confirms
```

## Then

The arm reads `exposed`. `bridge_fortnight` is related only by sharing the
file, but it calls a function that reaches `seconds`, so it may run the arm
and its `Unit::Fortnight` assertion still confirms it. This is the control for
`match_arm_proximity_confirmation_not_credited`.

## Must Not

- Withhold confirmation from a same-file test that calls a function with a
  name path to the owner.
- Claim runtime adequacy.
