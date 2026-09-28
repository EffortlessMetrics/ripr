# First Useful Action Dogfood Receipts

Date: 2026-05-09

Campaign: 22, First Useful Action

Work item: `dogfood/first-useful-action-receipts`

## Scope

This receipt records repo-local first-action routes that are checked by
`cargo xtask dogfood`. The checks read committed
`first-useful-action.{json,md}` artifacts and verify their status, action kind,
audience, selected-seam presence, static movement, Markdown status/action copy,
and static-evidence limit.

## Checked Routes

| Case | Status | Action | Purpose |
| --- | --- | --- | --- |
| `actionable` | `actionable` | `write_focused_test` | PR-local weak seam routes to one focused test. |
| `baseline-only` | `baseline_only` | `acknowledge_baseline` | Historical baseline debt stays visible without becoming PR-local test work. |
| `stale` | `stale` | `refresh_evidence` | Stale evidence routes to refresh before action. |
| `missing-required-artifact` | `missing_required_artifact` | `generate_missing_artifact` | Missing assistant proof routes to artifact generation. |
| `unchanged-after-attempt` | `unchanged_after_attempt` | `revise_focused_test` | Unchanged movement routes back to revising the focused test. |
| `no-actionable-seam` | `no_actionable_seam` | `no_action` | Clean state remains explicit instead of silent. |

## Validation

```bash
cargo xtask dogfood
```

Result: pass.

The dogfood report writes advisory receipts to:

```text
target/ripr/reports/dogfood.json
target/ripr/reports/dogfood.md
```

## Limits

- Static evidence only.
- No hidden analysis rerun.
- No source edits or generated tests.
- No provider calls.
- No mutation execution.
- No policy or gate semantic changes.
- No default CI blocking.
