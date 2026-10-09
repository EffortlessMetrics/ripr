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

The arm reads `exposed`. `bridge_fortnight` remains related by file proximity,
but its own exact assertion calls a parser-established transparent wrapper.
The wrapper forwards its sole plain enum parameter unchanged to the owner's
whole tail match, and the assertion selects the changed enum variant through
a verified parent-module binding. That same admitted oracle therefore supplies
owner identity, changed-arm observation, and strong discrimination. Possible
wrapper reach alone supplies no such identity. This is the positive control for
`match_arm_proximity_confirmation_not_credited` and the wrapper binding refusal
fixtures.

## Must Not

- Promote an oracle through possible wrapper reach without its own verified
  callable, enum-variant, and eager assertion binding.
- Claim runtime adequacy.
