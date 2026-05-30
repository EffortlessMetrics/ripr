# First Successful PR Demo

This demo shows the adopter path that `ripr first-pr` is meant to make obvious:

```text
one PR
-> one start-here packet
-> one repairable stable Rust gap, preview Python gap, or a clear no-action state
-> one repair route
-> one verification command
-> one receipt trail
```

The checked corpus lives in [fixtures/first_successful_pr](../../fixtures/first_successful_pr/README.md).
It is fixture-backed by [RIPR-SPEC-0051](../specs/RIPR-SPEC-0051-first-successful-pr-ux.md)
and composes explicit gap-ledger artifacts. It does not run hidden analysis,
edit source, generate tests, call providers, run mutation testing, or change
gate policy.

## Run The Demo

From the repository root, generate a demo packet for the boundary-gap case:

```bash
ripr first-pr \
  --root . \
  --base origin/main \
  --head HEAD \
  --gap-ledger fixtures/first_successful_pr/boundary-gap/inputs/reports/gap-decision-ledger.json \
  --out-dir target/ripr/demo/boundary-gap
```

Open:

```text
target/ripr/demo/boundary-gap/start-here.md
```

The checked golden version is:

```text
fixtures/first_successful_pr/boundary-gap/expected/start-here.md
```

The case-local demo script is:

```text
fixtures/first_successful_pr/boundary-gap/README.md
```

## Demo Cases

| Case | User story | Start-here result | Repair route | Verification |
| --- | --- | --- | --- | --- |
| `boundary-gap` | Changed Rust behavior is reached by a related test, but the equality boundary is not checked. | Top gap: missing boundary assertion for `amount >= threshold`. | `AddBoundaryAssertion` in `tests/pricing.rs`. | `cargo xtask fixtures boundary_gap` |
| `output-contract-gap` | User-facing output text changed without checked output or golden evidence. | Top gap: missing output contract for `APPLE_M3_AIR_DEVICE_LABELS_TEXT`. | `AddOutputGolden` in the expected output fixture. | `cargo xtask goldens check` |
| `python-preview-gap` | Changed Python behavior is reached by a related pytest context, but the equality boundary is not checked. | Top gap: preview-limited missing boundary assertion for `amount >= threshold`. | `StrengthenExistingTest` in `tests/test_pricing.py`. | `pytest tests/test_pricing.py::test_calculate_discount_smoke` |
| `empty-diff` | The PR has no changed behavior to inspect. | Successful no-action state. | No repair selected. | No-action is advisory, not adequacy proof. |
| `blocked-ledger` | The gap ledger cannot be trusted yet. | Blocked state with a regeneration command. | Refresh the ledger before assigning repair work. | `ripr reports gap-ledger ...` |

## Boundary Gap Story

The boundary-gap packet is the first-run happy path:

```text
Changed behavior:
  amount >= threshold

Why it matters:
  A related Rust test reaches the change, but no equality-boundary assertion
  was found.

Repair:
  Add an exact assertion for amount == threshold.

Verify:
  cargo xtask fixtures boundary_gap
```

The useful output is not the raw finding. It is the bounded work order:
what changed, why it matters, where to repair, and which command verifies
movement.

After the focused proof is added outside RIPR, emit the reviewer receipt:

```bash
cargo xtask fixtures boundary_gap

ripr outcome \
  --before fixtures/boundary_gap/calibration/before-targeted-test.repo-exposure.json \
  --after fixtures/boundary_gap/calibration/after-targeted-test.repo-exposure.json \
  --out target/ripr/demo/boundary-gap/targeted-test-outcome.md
```

The checked receipt is:

```text
fixtures/boundary_gap/calibration/targeted-test-outcome.md
```

That receipt records static before/after movement. It does not claim runtime
mutation confirmation, coverage adequacy, correctness, or merge approval.

## Python Preview Gap Story

The Python preview case proves the same first-run packet can route an explicit
preview gap-ledger record without requiring a Cargo workspace:

```text
Changed behavior:
  if amount >= threshold:

Why it matters:
  A related Python test reaches the change, but no boundary discriminator was
  found.

Repair:
  Strengthen the existing pytest test with an assertion for amount == threshold.

Verify:
  pytest tests/test_pricing.py::test_calculate_discount_smoke
```

The packet stays `preview_limited`, labels the language status as `preview`,
and keeps the static/advisory boundary visible before repair guidance.

## Output Contract Story

The output-contract case covers a different proof surface:

```text
Changed output:
  APPLE_M3_AIR_DEVICE_LABELS_TEXT

Why it matters:
  User-facing output changed without checked output or golden evidence.

Repair:
  Add or update the golden output fixture.

Verify:
  cargo xtask goldens check
```

This keeps output text changes out of generic `static_unknown` repair language.
The packet routes the change to the output proof that should move.

## No-Action And Blocked Are Valid

The demo also pins the non-repair states:

- `empty-diff` is a successful no-action packet. It does not claim runtime,
  coverage, mutation, or correctness adequacy.
- `blocked-ledger` is a useful blocked packet. It names the stale or missing
  evidence and gives the regeneration command instead of assigning repair work.

## What This Shows

This demo shows that the first-run front door can turn existing artifacts into a
user-readable repair path.

It does not claim:

- runtime mutation adequacy;
- coverage adequacy;
- general correctness;
- gate authority;
- preview-language promotion.

Gate decisions remain separate explicit artifacts. `start-here.md` guides the
reviewer; it does not decide pass or fail.

## Related Docs

- [First successful PR workflow](../FIRST_PR_WORKFLOW.md)
- [Quickstart](../QUICKSTART.md)
- [Output schema](../OUTPUT_SCHEMA.md#first-pr-start-here-packet)
- [Support tiers](../status/SUPPORT_TIERS.md)
- [First successful PR fixture corpus](../../fixtures/first_successful_pr/README.md)
