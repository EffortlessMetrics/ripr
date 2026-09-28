# Handoff: test-oracle assistant dogfood receipt

Date: 2026-05-09
Branch / PR: dogfood-test-oracle-assistant-receipt / pending at authoring
Latest merged PR: #624 `fixtures: pin test-oracle assistant loop`

## Current work item

`dogfood/test-oracle-assistant-receipt`

This receipt traces the canonical boundary-gap seam through the current
Campaign 20 surfaces without editing source, generating tests, calling an
external provider, running mutation tests, or changing CI defaults.

## Seam traced

| Field | Value |
| --- | --- |
| Seam ID | `67fc764ba37d77bd` |
| Kind | `predicate_boundary` |
| Location | `fixtures/boundary_gap/input/src/lib.rs:2` |
| Missing discriminator | `discount_threshold (equality boundary)` |
| Suggested focused test | `discounted_total_boundary_discriminator` in `tests/pricing.rs` |
| Related test | `tests/pricing.rs::below_threshold_has_no_discount` |

## Artifact chain

| Stage | Artifact |
| --- | --- |
| PR guidance | `fixtures/boundary_gap/expected/test-oracle-assistant-loop/canonical/pr-guidance.json` |
| Editor/agent handoff | `fixtures/boundary_gap/expected/editor-agent-loop/agent-brief.json` |
| Before evidence | `fixtures/boundary_gap/calibration/before-targeted-test.repo-exposure.json` |
| After evidence | `fixtures/boundary_gap/calibration/after-targeted-test.repo-exposure.json` |
| Receipt | `fixtures/boundary_gap/expected/editor-agent-loop/agent-receipt.json` |
| PR ledger projection | `fixtures/boundary_gap/expected/test-oracle-assistant-loop/canonical/pr-evidence-ledger.json` |
| Proof packet | `fixtures/boundary_gap/expected/test-oracle-assistant-loop/canonical/test-oracle-assistant-proof.{json,md}` |

## Evidence movement

The checked receipt preserves current static analyzer behavior:

```text
weakly_gripped -> weakly_gripped
state: unchanged
```

The after evidence still matters: the targeted-test outcome records a new
observed value, `100`, for the seam. The current static classifier does not yet
promote that to `strongly_gripped`. That is intentional dogfood evidence, not a
failure of this receipt.

## PR / CI projection

The canonical ledger projection reports:

```text
new_policy_eligible: 0
baseline_resolved: 0
acknowledged: 0
suppressed: 0
blocking_candidates: 0
visible_unresolved: 1
```

Gate authority remains separate:

```text
gate decision when explicitly configured
```

No generated CI default changes are part of this receipt.

## Coverage / grip frontier

Command:

```bash
cargo run -p ripr -- coverage-grip frontier --ledger fixtures/boundary_gap/expected/test-oracle-assistant-loop/canonical/pr-evidence-ledger.json --out target/ripr/reports/test-oracle-assistant-coverage-grip-frontier.json --out-md target/ripr/reports/test-oracle-assistant-coverage-grip-frontier.md
```

Observed summary:

```text
Status: advisory
Coverage status: not_available
RIPR source: pr_evidence_ledger
Visible unresolved gaps: 1
Warnings: coverage input not supplied; coverage axis is not available
```

The report keeps coverage and behavioral grip as separate axes. It does not
claim coverage adequacy or runtime mutation outcomes.

## Verification run

```text
cargo xtask dogfood
status: pass

boundary_gap: 1 finding, 1 stop-reason field, no errors
weak_error_oracle: 3 findings, 3 stop-reason fields, no errors
gate adoption receipts: visible-only, acknowledged, baseline-existing,
baseline-new, calibrated-gate, and missing-baseline cases matched expected
statuses and exit behavior
```

```text
cargo xtask test-oracle-report
status: warn

strong: 639
medium: 4
weak: 310
smoke: 46
BDD-shaped names: 103 / 999
```

`test-oracle-report` is advisory and intentionally reports existing test-oracle
debt rather than failing this work item.

## Limits

- Static RIPR evidence only.
- No source edits.
- No generated tests.
- No provider calls.
- No mutation execution.
- No CI blocking by default.

## Next work item

`docs/test-oracle-assistant-workflow`

Document the user workflow from PR recommendation to editor/agent handoff, one
focused test, verification, receipt, and advisory CI/ledger projection while
preserving static-evidence limits.

