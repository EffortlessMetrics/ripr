# Report Packet Index Dogfood Receipts

Date: 2026-05-10

Campaign: 25, Report Packet Index

Work item: `dogfood/report-packet-index-receipts`

## Scope

This receipt records repo-local report-packet index cases that are checked by
`cargo xtask dogfood`. The checks read committed `index.{json,md}` fixture
artifacts and verify status, missing-surface counts, warning/failure counts,
start-here availability, gate-authority availability, required groups,
Markdown status, and advisory report-packet limits.

## Checked Packets

| Case | Status | Purpose |
| --- | --- | --- |
| `complete_packet` | `pass` | Complete reviewer packet with start-here and gate authority. |
| `sparse_advisory` | `warn` | Sparse advisory packet with visible missing optional surfaces. |
| `missing_front_panel` | `warn` | Supporting artifacts exist but the first-screen front panel is missing. |
| `blocked_gate` | `fail` | Configured gate is blocked while the index remains advisory. |
| `missing_assistant_proof` | `warn` | PR guidance exists but assistant proof is missing. |
| `missing_receipts` | `warn` | Review story exists but validation receipts are absent. |
| `coverage_grip_present` | `pass` | Coverage/grip frontier is grouped as calibration context. |

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
