# ripr operator cockpit

Status: warn

## Top Weak Seams

- `67fc764ba37d77bd` `weakly_gripped` src/lib.rs:2 `predicate_boundary`
  - why: observed values do not include the equality-boundary case for this predicate
  - next targeted test: Add a focused predicate_boundary test for `src/lib.rs::discounted_total` that exercises `discount_threshold (equality boundary)` and asserts the observable result.
  - best related test: `below_threshold_has_no_discount` tests/pricing.rs:4 (strong)

## Surface Alignment

| Surface | State | Status | Agreement | Signal |
| --- | --- | --- | --- | --- |
| repo exposure | present | present | actionable_seams_visible | 1 seams; 1 weakly_gripped, 0 ungripped, 0 reachable_unrevealed. |
| LSP cockpit | present | pass | editor_contract_green | 1 LSP fixture reports; 0 uncovered contributed VS Code commands. |
| before snapshot | present | present | before_snapshot_available | 1 seams; 1 weakly_gripped, 0 ungripped, 0 reachable_unrevealed. |
| after snapshot | present | present | after_snapshot_available | 1 seams; 1 weakly_gripped, 0 ungripped, 0 reachable_unrevealed. |
| agent verify | present | advisory | agent_verify_counts_available | 0 improved, 0 changed, 0 regressed, 1 unchanged seams. |
| agent receipt | present | advisory | agent_receipt_available | Receipt for seam 67fc764ba37d77bd: unchanged; before weakly_gripped, after weakly_gripped. Static grip class did not move. |
| SARIF policy | missing | missing | not_available | Report has not been generated yet. |
| badge status | missing | missing | not_available | Report has not been generated yet. |
| targeted-test outcome | present | advisory | targeted_outcome_artifact_present | 0 moved, 0 regressed, 1 unchanged seams. |
| mutation calibration | optional_missing | optional | not_available | Optional calibration report has not been generated. |

## Inputs

| Report | Required | State | Path |
| --- | --- | --- | --- |
| repo exposure | true | present | `target/ripr/reports/repo-exposure.json` |
| LSP cockpit | true | present | `target/ripr/reports/lsp-cockpit.json` |
| before snapshot | true | present | `target/ripr/pilot/repo-exposure.json` |
| after snapshot | true | present | `target/ripr/pilot/after.repo-exposure.json` |
| agent verify | true | present | `target/ripr/agent/agent-verify.json` |
| agent receipt | true | present | `target/ripr/agent/agent-receipt.json` |
| SARIF policy | true | missing | `target/ripr/reports/sarif-policy.json` |
| badge status | true | missing | `target/ripr/reports/repo-ripr-badge.json` |
| targeted-test outcome | true | present | `target/ripr/reports/targeted-test-outcome.json` |
| mutation calibration | false | optional_missing | `target/ripr/reports/mutation-calibration.json` |

## Next Commands

- `cargo xtask sarif-policy --current target/ripr/workflow/current.repo-sarif.json`
  - Generate the missing SARIF policy input.
- `cargo xtask repo-badge-artifacts`
  - Generate the missing badge status input.
- `ripr pilot --out target/ripr/pilot`
  - Open the top actionable seam packet and write one focused targeted test.
- `ripr check --root . --mode draft --format repo-exposure-json > target/ripr/pilot/after.repo-exposure.json`
  - After adding the targeted test, capture the after repo-exposure snapshot.
- `ripr agent verify --root . --before target/ripr/pilot/repo-exposure.json --after target/ripr/pilot/after.repo-exposure.json --json > target/ripr/agent/agent-verify.json`
  - Compare the before and after static evidence snapshots for the agent loop.
- `ripr agent receipt --root . --verify-json target/ripr/agent/agent-verify.json --seam-id 67fc764ba37d77bd --json --out target/ripr/agent/agent-receipt.json`
  - Write a focused receipt for the top seam after agent verify completes.
- `ripr outcome --before target/ripr/pilot/repo-exposure.json --after target/ripr/pilot/after.repo-exposure.json --format json --out target/ripr/reports/targeted-test-outcome.json`
  - Compare the before and after static evidence snapshots.

This cockpit joins existing reports. It does not rerun analysis, mutate tests, or change static classifications.
