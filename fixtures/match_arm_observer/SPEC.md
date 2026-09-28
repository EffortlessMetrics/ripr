# Fixture: match_arm_observer

Spec: RIPR-SPEC-0093

## Given

Production code adds a `Status::Idle => 0` arm to a `match s` expression:

```rust
pub enum Status { Active, Idle }

pub fn classify(s: Status) -> i32 {
    match s {
        Status::Active => 1,
        Status::Idle => 0,   // ← new arm in diff
    }
}
```

The test directly exercises the probed arm and references its variant token in
the assertion:

```rust
#[test]
fn idle_arm_returns_zero() {
    assert_eq!(classify(Status::Idle), 0);
}
```

The identifier `Idle` appears in both the probe expression
(`Status::Idle => 0,`) and the assertion text, so `token_match` fires.

## When

```bash
cargo xtask fixtures match_arm_observer
```

or:

```bash
ripr check --root fixtures/match_arm_observer/input --diff fixtures/match_arm_observer/diff.patch --mode fast
```

## Then

`ripr` must emit `exposed` (confidence 1.0) for the `Status::Idle => 0` arm
probe. The `token_match` confirms the assertion references this arm's variant,
so `arm_observation_unverified` must NOT be set.

This fixture is the **anti-over-correction proof** for
[RIPR-SPEC-0093](../../docs/specs/RIPR-SPEC-0093-match-arm-blind-reach-downgrade.md):
a real arm-observer with `token_match` must not be downgraded.

## Must Not

- Downgrade a confirmed arm-observer to `weakly_exposed`.
- Emit `arm_observation_unverified` when the test assertion references the arm's pattern token.
- Use mutation-runtime outcome vocabulary.
