# Handoff: Lane 1 Evidence-To-Repair Closeout Audit

Date: 2026-05-27

Branch: `docs-lane1-evidence-to-repair-closeout-audit`

Audit ID: `LANE1-AUDIT-2026-05-27-EVIDENCE-TO-REPAIR`

Linked specs:

- [RIPR-SPEC-0057](../specs/RIPR-SPEC-0057-ripr-swarm-repair-loop.md)
- [RIPR-SPEC-0059](../specs/RIPR-SPEC-0059-actionable-surface-translation.md)
- [RIPR-SPEC-0061](../specs/RIPR-SPEC-0061-lane1-canonical-actionability-contract.md)

## Current State

Lane 1 has the evidence-to-repair spine, but this audit does not close the full
lane goal. The fresh repo run is honest and bounded:

```text
run_status: limited_incomplete_input
phase: repo_exposure_generation
limitation_category: lane1_repo_exposure_sampled
input: repo-exposure-json:limit_5000_of_40873
downstream_consumable: false
```

The current artifacts are therefore useful for selecting the next analyzer
slice, not for claiming a complete repo-level actionable repair queue.

The important product result is that the limitation is explicit. The reports do
not emit all-zero success as if the run were full, do not silently consume stale
input, and do not ask badge, LSP, PR, or CI surfaces to reinterpret raw
findings.

## Fresh Proof Commands

These commands were run locally on 2026-05-27 from the `ripr-swarm` workspace
root after `main` was synced to `a0d85035`.

| Command | Result | Evidence |
| --- | --- | --- |
| `cargo xtask lane1-evidence-audit` | pass | Wrote `target/ripr/reports/lane1-evidence-audit.json`; repo exposure sampled 5,000 of 40,873 seams in 70,604 ms with `limited_incomplete_input`. |
| `cargo xtask actionable-gaps` | not a current command | Current command catalog has no top-level `actionable-gaps`; current equivalents are `ripr-swarm plan`, `actionable-gap-outcomes`, and downstream repair-loop reports. |
| `cargo xtask ripr-swarm plan --top 10` | pass | Read `target/ripr/reports/actionable-gaps.json`; emitted `0` packets and propagated `limited_incomplete_input`. |
| `cargo xtask actionable-gap-outcomes` | pass | Emitted `0` outcomes and `0` orphaned receipts for the current empty packet queue. |
| `cargo xtask ripr-swarm attempt-ledger` | pass | Emitted 6 ledger attempts, all `not_attempted`, with `0` improved, `0` unchanged, `0` regressed, `0` resolved, and `0` orphaned receipts. |
| `cargo xtask ripr-swarm readiness` | pass | Emitted `top_next_action.kind = no_ready_action`; `swarm_ready_packets = 0`; `downstream_consumable = false`. |
| `cargo xtask evidence-quality-scorecard` | pass | Ranked next analyzer work by named static limitations and preserved `run_status = limited_incomplete_input`. |
| `cargo xtask evidence-quality-trend` | pass | Reported no previous scorecard history and kept `current_scorecard_limited` visible. |
| `cargo xtask dogfood` | pass with advisory report status `warn` | Real repair receipts show 5 attempts: 2 improved, 1 resolved, 1 unchanged, 1 attempted without receipt, 0 regressed. The generated CI cockpit dogfood case still records a warning about expected repair-command count. |
| `cargo xtask cache report` | pass | Cache scope was only `target/ripr/cache`; total size was 136.03 MB across 270 `repo-file-facts` files. |
| `cargo xtask cache gc --dry-run --max-size-gb 20 --ttl-days 14` | pass | Selected 0 files for deletion; projected cache size remained 136.03 MB. |

## Evidence Snapshot

| Field | Current value |
| --- | ---: |
| Raw signals in sampled audit | 5,272 |
| Canonical items in sampled audit | 5,000 |
| Already observed canonical items | 3,381 |
| Static limitations | 1,664 |
| Actionable gaps | 0 |
| Public projection eligible packets | 0 |
| Swarm-ready packets | 0 |
| Attempt-ledger attempts | 6 |
| Attempt-ledger improved | 0 |
| Attempt-ledger unchanged | 0 |
| Attempt-ledger regressed | 0 |
| Attempt-ledger resolved | 0 |
| Dogfood real attempts | 5 |
| Dogfood real improved | 2 |
| Dogfood real unchanged | 1 |
| Dogfood real regressed | 0 |
| Dogfood real resolved | 1 |
| Dogfood real attempted without receipt | 1 |

## Completion Audit

| Lane 1 requirement | Current evidence | Status |
| --- | --- | --- |
| Runtime completeness is explicit | `lane1-evidence-audit`, scorecard, trend, swarm plan, attempt ledger, and readiness all carry `run_status = limited_incomplete_input` with `downstream_consumable = false`. | Proven for this limited run. |
| Canonical evidence is the countable unit | Sampled audit reports 5,000 canonical items, 5,272 raw signals, 0 unaligned raw findings, and a raw-to-canonical ratio of 1.0544. | Proven for the sampled input. |
| Actionability is strict | Current sampled audit emits 0 actionable gaps, 0 packets, 0 missing repair routes, and 0 missing verify commands instead of manufacturing unsafe work. | Proven for the sampled input. |
| Named limitations replace speculative actionability | Static limitations are named, including `activation_owner_call_unresolved`, `activation_boundary_input_unresolved`, `activation_value_unresolved`, and `lane1_repo_exposure_sampled`, each with repair routes. | Proven for the sampled input. |
| Repair packets are bounded and receiptable | Fixture and dogfood receipts prove packet contracts, verify commands, receipt commands, and `must_not_change` boundaries; the fresh sampled queue has no packets. | Partially proven; no current repo packet is ready to attempt. |
| Attempt outcomes stay visible | Dogfood receipts include improved, resolved, unchanged, and attempted-without-receipt cases; current ledger keeps 6 not-attempted entries visible. | Proven for dogfood and current empty queue. |
| Repair-route quality feeds analyzer work | Readiness and attempt-ledger expose repair-route quality, top missing evidence field `repair_kind`, and dogfood route outcomes. | Proven as report behavior; needs more live packet volume. |
| Badge/LSP/PR/CI consume canonical state | `cargo xtask dogfood` checks the user-surface projection corpus for badge, LSP, PR comment, and CI examples over one canonical packet. | Proven as projection fixture, not behavior change. |
| Full lane closeout | A full run with `run_status = full`, non-stale input, current top repair, receipts, and outcome movement is required. | Not achieved. |

## Top Analyzer Backlog From The Fresh Audit

The next work should improve analyzer trust before increasing public packet
volume.

| Priority | Slice | Fresh signal |
| ---: | --- | --- |
| 1 | `analysis/related-test-ranking-audit-fixes` | `activation_owner_call_unresolved` has 1,132 total static limitations, including 663 in `call_presence`; scorecard ranks this as the top work queue. |
| 2 | `analysis/local-iterator-boundary-operand-resolution` | `activation_boundary_input_unresolved` has 291 static limitations; these remain named limitations instead of unsafe boundary assertions. |
| 3 | `analysis/value-resolution-audit-fixes` | `activation_value_unresolved` has 136 static limitations. |
| 4 | `report/evidence-quality-trend` | Trend has no previous scorecard history, so movement is `unknown` instead of improved or regressed. |

## What Users May Believe

- RIPR currently has a typed evidence-to-repair spine across canonical items,
  actionability, swarm planning, attempt ledgers, outcomes, route quality, and
  downstream projection fixtures.
- The fresh repo audit did not find a current sampled actionable repair queue.
- The fresh repo audit did find named analyzer limitations that should drive
  the next Lane 1 PR.
- Limited input remains visible to downstream reports and is not consumable as
  a complete badge, LSP, PR, or CI truth source.

## What Users Must Not Infer

- Do not infer a full Lane 1 closeout.
- Do not infer repo-level `0 actionable` from the sampled limited run.
- Do not infer mutation proof, runtime adequacy, coverage adequacy, merge
  readiness, policy eligibility, or default CI blocking.
- Do not treat raw findings as user work.
- Do not treat named static limitations as user test debt.
- Do not change badge endpoint semantics, LSP behavior, PR comment publishing,
  CI gate behavior, provider integration, generated tests, autonomous edits, or
  mutation execution from this audit.

## Next Recommended PR

Open a fixture-first analyzer PR for:

```text
analysis: reduce activation_owner_call_unresolved limitations
```

The first acceptance target should come from the current scorecard queue:

```text
evidence_class: call_presence
category: activation_owner_call_unresolved
repair_route: analysis/related-test-ranking-audit-fixes
fresh sampled count: 663
```

Keep the actionability rule intact. If related-test ranking cannot safely prove
owner-call activation, keep the item as a named limitation with a sharper
repair route rather than projecting it as an actionable packet.

## Artifacts

- `docs/handoffs/2026-05-27-lane1-evidence-to-repair-closeout-audit.md`
- `target/ripr/reports/lane1-evidence-audit.json`
- `target/ripr/reports/swarm-plan.json`
- `target/ripr/reports/actionable-gap-outcomes.json`
- `target/ripr/reports/swarm-attempt-ledger.json`
- `target/ripr/reports/swarm-readiness.json`
- `target/ripr/reports/evidence-quality-scorecard.json`
- `target/ripr/reports/evidence-quality-trend.json`
- `target/ripr/reports/dogfood.json`
