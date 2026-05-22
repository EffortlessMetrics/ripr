# Boundary Gap First PR Demo

This case is the canonical ten-minute first successful PR story. It uses
checked fixture artifacts to show the loop without asking RIPR to edit source,
generate tests, call a provider, or run mutation testing.

```text
before evidence
-> ripr first-pr
-> top boundary gap
-> focused external proof
-> ripr outcome
-> reviewer receipt
```

## Step 1: Generate The Start-Here Packet

From the repository root:

```bash
ripr first-pr \
  --root fixtures/first_successful_pr/boundary-gap \
  --base origin/main \
  --head HEAD \
  --gap-ledger inputs/reports/gap-decision-ledger.json \
  --out-dir target/ripr/demo/boundary-gap
```

Read:

```text
target/ripr/demo/boundary-gap/start-here.md
```

The checked expected packet is:

```text
fixtures/first_successful_pr/boundary-gap/expected/start-here.md
```

It selects one repairable Rust gap:

```text
Changed behavior:
  amount >= threshold

Missing discriminator:
  Equality-boundary assertion for the changed behavior.

Focused proof intent:
  Add a focused boundary assertion in tests/pricing.rs:
  assert_eq!(discount(100, 100), 90)
```

## Step 2: Add The Focused Proof Outside RIPR

RIPR does not edit source or generate tests. The user, reviewer, or external
coding agent adds the focused proof in the related test. This fixture represents
that external edit with before/after static exposure snapshots:

```text
fixtures/boundary_gap/calibration/before-targeted-test.repo-exposure.json
fixtures/boundary_gap/calibration/after-targeted-test.repo-exposure.json
```

## Step 3: Emit The Outcome Receipt

```bash
ripr outcome \
  --before fixtures/boundary_gap/calibration/before-targeted-test.repo-exposure.json \
  --after fixtures/boundary_gap/calibration/after-targeted-test.repo-exposure.json \
  --out target/ripr/demo/boundary-gap/targeted-test-outcome.md
```

The checked reviewer-native receipt is:

```text
fixtures/boundary_gap/calibration/targeted-test-outcome.md
```

It records that the static evidence moved after the focused external proof and
keeps the claim boundary explicit.

## What Reviewers May Believe

- RIPR selected one bounded boundary assertion gap.
- The suggested proof target and verification command are explicit.
- The outcome receipt records static before/after movement.

## What Reviewers Must Not Infer

- No runtime mutation proof.
- No coverage adequacy.
- No general correctness proof.
- No merge approval.
- No source edit or generated test from RIPR.
