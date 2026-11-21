# Fixture: match_arm_blind

Spec: RIPR-SPEC-0093

## Given

Production code adds a `None => 0` arm to an existing `match x` expression:

```rust
pub fn reason(x: Option<i32>) -> i32 {
    match x {
        Some(v) => v + 1,
        None => 0,   // ← new arm in diff
    }
}
```

The only test exercises the `Some` arm and carries a single `assert_eq!`:

```rust
#[test]
fn some_arm_returns_incremented_value() {
    assert_eq!(reason(Some(5)), 6);
}
```

The assertion is exact-value (`OracleKind::ExactValue`) which fires
`oracle_matches_family(MatchArm, _)` — but the assertion text contains no token
from the `None => 0` arm's expression (`None` is filtered as a common keyword,
`0` has no alpha content). There is no `token_match` to the probed arm.

## When

```bash
cargo xtask fixtures match_arm_blind
```

or:

```bash
ripr check --root fixtures/match_arm_blind/input --diff fixtures/match_arm_blind/diff.patch --mode fast
```

## Then

`ripr` must emit `weakly_exposed` (not `exposed`) for the `None => 0` arm probe.
The discriminate stage must be `weak` with reason
`arm_observation_unverified`, and confidence must be less than 1.0.

This fixture is the **before/after proof** for the fix introduced in
[RIPR-SPEC-0093](../../docs/specs/RIPR-SPEC-0093-match-arm-blind-reach-downgrade.md):
without the fix this probe was reported `exposed / confidence 1.0` — a
false-actionable because no test exercises the `None` arm.

## Must Not

- Use mutation-runtime outcome vocabulary.
- Report this arm as `exposed` when the only related test observes a different arm.
- Report `no_static_path` or `reachable_unrevealed` (the test IS reachable and
  DOES have an oracle — the oracle is just not confirmed for THIS arm).
