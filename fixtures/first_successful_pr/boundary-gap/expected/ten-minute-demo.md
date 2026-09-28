# First PR Boundary Gap Ten-Minute Demo

This checked story is the canonical first-useful-PR loop for a Rust boundary
gap:

```text
before -> ripr first-pr -> top gap -> focused external proof -> ripr outcome -> receipt
```

It stitches together existing fixture artifacts instead of running hidden
analysis, editing source, generating tests, calling providers, running mutation,
or changing CI/gate behavior.

## 1. Start From The Existing PR Evidence

The boundary-gap input is a gap decision ledger:

```text
fixtures/first_successful_pr/boundary-gap/inputs/reports/gap-decision-ledger.json
```

It describes a changed Rust predicate:

```text
amount >= threshold
```

The related test reaches the changed code, but the first-run packet does not
find an equality-boundary assertion for `amount == threshold`.

## 2. Run The Front Door

```bash
ripr first-pr --root . --base origin/main --head HEAD --gap-ledger fixtures/first_successful_pr/boundary-gap/inputs/reports/gap-decision-ledger.json --out-dir target/ripr/demo/boundary-gap
```

Open:

```text
target/ripr/demo/boundary-gap/start-here.md
```

The checked expected output is:

```text
fixtures/first_successful_pr/boundary-gap/expected/start-here.md
```

## 3. Read The Top Repairable Gap

The one-screen recommendation should identify:

```text
Top repairable gap:
  missing boundary assertion

Changed behavior:
  amount >= threshold

Missing discriminator:
  assert_eq!(discount(100, 100), 90)

Focused proof intent:
  Add one focused assertion in tests/pricing.rs for the threshold equality case.

Verify:
  cargo xtask fixtures boundary_gap
```

This is a bounded repair route. It is not a proof that the program is correct.

## 4. Add The Focused Proof Outside RIPR

RIPR does not edit source or generate tests. The external repair is one focused
assertion in the related test:

```rust
assert_eq!(discount(100, 100), 90);
```

The boundary-gap calibration snapshots model this before/after state:

```text
fixtures/boundary_gap/calibration/before-targeted-test.repo-exposure.json
fixtures/boundary_gap/calibration/after-targeted-test.repo-exposure.json
```

Run the verifier named by the first-pr packet:

```bash
cargo xtask fixtures boundary_gap
```

## 5. Emit The Reviewer Receipt

```bash
ripr outcome --before fixtures/boundary_gap/calibration/before-targeted-test.repo-exposure.json --after fixtures/boundary_gap/calibration/after-targeted-test.repo-exposure.json --format md --out target/ripr/receipts/gap-pr-pricing-threshold-boundary.md
```

The checked reviewer-native receipt is:

```text
fixtures/boundary_gap/calibration/targeted-test-outcome.md
```

The receipt should tell the reviewer:

```text
What RIPR flagged before:
  weakly_gripped boundary evidence

Focused proof observed:
  new observed value: 100

Static movement:
  the seam stayed in the same static class while rendered evidence changed

Reviewer should not believe:
  RIPR edited source, generated tests, ran mutation, approved the PR, decided a
  gate, or proved coverage completeness
```

## 6. Keep The Boundary Honest

This demo proves the product path:

```text
one changed behavior
-> one missing discriminator
-> one focused proof intent
-> one verify command
-> one reviewer-readable receipt
```

It remains static advisory evidence. It is not runtime confirmation, mutation
confirmation, coverage adequacy, merge approval, gate authority, or a claim that
RIPR made the repair.
