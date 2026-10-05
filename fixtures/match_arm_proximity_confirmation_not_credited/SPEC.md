# Fixture: match_arm_proximity_confirmation_not_credited

Spec: RIPR-SPEC-0094

Owner: analysis-fixtures

Issue: #6297

## Given

The diff changes the value of the `Unit::Fortnight =>` arm in `seconds`.
`seconds_total` calls `seconds` for a week and a fortnight and asserts the
exact sum, but its assertion never names the arm's variant or value.
`from_str_fortnight` never calls `seconds`; it shares the file and asserts
`matches!(Unit::from_str("fortnight"), Ok(Unit::Fortnight))`, which names the
arm's variant.
These assertions are intentional analyzed fixture input, governed by the
existing `fixtures/**` source-input policy.

## When

```bash
cargo xtask fixtures match_arm_proximity_confirmation_not_credited
```

The public diff analysis examines the changed arm on `src/lib.rs:23`.

## Then

The arm stays `weakly_exposed` with `observation_unverified`. Reach comes from
`seconds_total`'s direct call; the same-file test still credits strength but
cannot confirm the arm, because the variant it names is shared by every
function that handles `Unit`. The discriminator summary says that a test which
only shares the file cannot confirm the arm, rather than claiming no assertion
names it.

Before #6297 this case read `exposed`, and rewriting only
`from_str_fortnight`'s assertion moved it to `weakly_exposed`.

The honesty corpus independently prohibits `exposed` even if a golden is
changed.

## Must Not

- Confirm a match arm from a same-file or same-module test that names the
  arm's variant while another related test reaches the owner.
- Remove same-file credit when no related test reaches the owner.
- Claim runtime adequacy.
