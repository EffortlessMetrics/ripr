# Generated CI Cockpit Dogfood Receipts

Date: 2026-05-13

Lane: 4, PR / CI Review Cockpit

Work item: `dogfood/lane4-cockpit-gap-receipts`

## Scope

This receipt records the generated GitHub workflow cockpit case checked by
`cargo xtask dogfood`. The check renders the public dry-run workflow with
`ripr init --ci github --dry-run` and verifies the generated summary starts
with reviewer-first guidance, names known regeneration commands for missing
cockpit surfaces, uploads the report packet, keeps generated CI advisory by
default, and preserves the configured gate-decision authority boundary.

The receipt does not duplicate Campaign 24 PR review front-panel cases or
Campaign 25 report-packet index cases. Those fixture-backed receipts remain the
source for missing-proof, blocked-gate, improved, unchanged-after-attempt, and
packet completeness states.

## Checked Case

| Case | Result | Purpose |
| --- | --- | --- |
| `generated-pr-ci-review-workflow` | `pass` | Validates the generated workflow cockpit summary, repair commands, artifact upload, advisory default, and gate-authority boundary. |

## Validation

```bash
cargo test -p xtask dogfood_
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
- No branch-protection changes.
- No inline comment default changes.
- Language-aware grouping remains deferred until preview-language evidence is
  ready or explicitly deferred.
