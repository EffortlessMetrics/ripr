# Fixture: match_arm_type_token_blind

Spec: RIPR-SPEC-0094

## Given

Production code adds a `Mode::Frozen => -1` arm to a match expression:

```rust
pub enum Mode { Warm, Frozen }

pub fn classify(mode: Mode) -> i32 {
    match mode {
        Mode::Warm => 1,
        Mode::Frozen => -1,   // ← value changed from 0 to -1
    }
}
```

The only test exercises the `Warm` arm and carries an exact-value assertion:

```rust
#[test]
fn warm_arm_returns_one() {
    assert_eq!(classify(Mode::Warm), 1);
}
```

The assertion text `assert_eq!(classify(Mode::Warm), 1)` contains `Mode` (the
enum qualifier) but NOT `Frozen` (the variant specific to the changed arm).
With variant-scoped `token_match`, the shared qualifier `Mode` must NOT clear
`observation_unverified` for the `Mode::Frozen` probe. This is the Part B
regression lock for the type-blind token hole fixed in RIPR-SPEC-0094.

## When

```bash
cargo xtask fixtures match_arm_type_token_blind
```

or:

```bash
ripr check --root fixtures/match_arm_type_token_blind/input \
           --diff fixtures/match_arm_type_token_blind/diff.patch --mode fast
```

## Then

`ripr` must emit `weakly_exposed` (not `exposed`) for the `Mode::Frozen => -1`
arm probe. The discriminate stage must be `weak` with reason
`observation_unverified`, because the shared qualifier `Mode` in the sibling
assertion (`Mode::Warm`) does NOT confirm observation of the `Frozen` arm.
Only the `Frozen` variant token would confirm it.

This fixture is the **Part B regression lock** for
[RIPR-SPEC-0094](../../docs/specs/RIPR-SPEC-0094-observation-unverified-guard-generalization.md):
before the variant-scoped fix, `Mode` in `Mode::Warm` spuriously cleared
`observation_unverified` for the `Mode::Frozen` probe, producing
`exposed`/confidence-1.00.

## Must Not

- Report `exposed` for a `Frozen` arm when the only assertion references `Warm`.
- Allow the shared enum qualifier (`Mode`) to confirm a sibling-arm observation.
- Use mutation-runtime outcome vocabulary.
