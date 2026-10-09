# Fixture: split_test_boundary_oracle

Spec: RIPR-SPEC-0186

Owner: analysis-fixtures

Issue: #4828

## Given

The diff changes `gate` from `>` to `>=`. `boundary` calls `gate` at 10 and 9
and asserts nothing. `far` asserts `gate(100) == true`.

Reverting `>=` to `>` leaves both tests passing: `boundary` has no oracle, and
`far` never runs the boundary.

These assertions are intentional analyzed fixture input, governed by the
existing `fixtures/**` source-input policy.

The same-test pairing control (`assert_eq!(gate(10), true)`) is pinned by unit
tests in `classify/boundary_pairing.rs` and by `fixtures/strong_boundary_oracle`.

## When

```bash
cargo xtask fixtures split_test_boundary_oracle
```

The public diff analysis examines the changed predicate on `src/lib.rs`.

## Then

The predicate reads below `exposed`. Infection from `boundary` and the exact
oracle from `far` do not combine: no test both feeds a boundary input and
holds a discriminating oracle on that call. The discriminate summary names
`same_test_pairing_missing`.

The honesty corpus independently prohibits `exposed` even if a golden is
changed.

## Must Not

- Credit helper-call assertions (#4574 / #4715).
- Change proximity-only oracle credit (#4486).
- Change bare-name method relation (#4760).
- Promote any finding's class.
- Claim sink identity, population accuracy, or runtime adequacy.
