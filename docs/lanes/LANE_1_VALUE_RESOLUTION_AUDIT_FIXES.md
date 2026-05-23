# Lane 1: Value Resolution Audit Fixes

Status: closed

Opened: 2026-05-22

GitHub issue: [ripr-swarm #285](https://github.com/EffortlessMetrics/ripr-swarm/issues/285)

## Goal

Burn down a fixture-backed slice of the
`predicate_boundary` / `activation_value_unresolved` limitation route without
loosening RIPR's static evidence boundary.

The product value is direct: predicate-boundary gaps are the current actionable
repair lead, but they stay useful only when RIPR can name concrete activation
values safely. This rail improves the value-resolution evidence that makes one
focused boundary assertion recommendation trustworthy.

## Selection Evidence

This rail is selected from repo-owned evidence:

- [Lane 1 Finding Alignment Burn-Down closeout](../handoffs/2026-05-22-lane1-finding-alignment-burndown-closeout.md)
  says future Lane 1 work should come from fresh audit, scorecard, dogfood,
  downstream consumer, or spec-backed evidence and records
  `activation_value_unresolved` as the largest remaining named limitation in
  the full-run proof.
- The current `target/ripr/reports/evidence-quality-scorecard.md` recommends
  `analysis/value-resolution-audit-fixes` for `predicate_boundary`, where
  `activation_value_unresolved` is the dominant named limitation for that
  evidence class.
- [ripr-swarm #285](https://github.com/EffortlessMetrics/ripr-swarm/issues/285)
  records the selected queue and proof commands.

The current scorecard also ranks
`analysis/related-test-ranking-audit-fixes` highly for
`activation_owner_call_unresolved`. That remains a valid later Lane 1
successor. This rail chooses the value-resolution path because it preserves the
closeout's full-run limitation signal and improves the predicate-boundary
repair surface already carrying actionable canonical gaps.

## Boundary

Lane 1 owns:

- value-resolution evidence;
- activation facts;
- static limitation naming;
- canonical gap actionability;
- audit, scorecard, trend, fixture, and dogfood proof.

Lane 1 does not own:

- PR or CI rendering;
- PR inline comment publishing;
- LSP or editor polish;
- gate-policy changes or default blocking;
- public badge or score redefinition;
- release or packaging mechanics;
- generated tests;
- source edits;
- provider/model calls;
- mutation execution.

## Source-Of-Truth Stack

- active manifest: `.ripr/goals/active.toml`;
- implementation plan:
  [Lane 1 Value Resolution Audit Fixes plan](../../plans/lane1-value-resolution-audit-fixes/implementation-plan.md);
- finding-alignment contract:
  [RIPR-SPEC-0045](../specs/RIPR-SPEC-0045-finding-to-gap-alignment.md);
- evidence-record contract:
  [RIPR-SPEC-0021](../specs/RIPR-SPEC-0021-evidence-record.md);
- static exposure contract:
  [RIPR-SPEC-0001](../specs/RIPR-SPEC-0001-static-exposure-loop.md);
- prior rail:
  [Lane 1 Finding Alignment Burn-Down](LANE_1_FINDING_ALIGNMENT_BURNDOWN.md);
- closeout that selected the next evidence class:
  [Lane 1 Finding Alignment Burn-Down closeout](../handoffs/2026-05-22-lane1-finding-alignment-burndown-closeout.md).

## Work Queue

| Work item | Status | Intent |
| --- | --- | --- |
| `docs/lane1-value-resolution-audit-fixes-stack` | done | Open the issue-backed rail, active manifest, plan, lane tracker, and index links. |
| `fixtures/value-resolution-audit-corpus` | done | Pin one audit-derived supported value-resolution sub-shape and must-not-claim guards before analyzer behavior. |
| `analysis/value-resolution-supported-subshape` | done | Confirm the fixture-backed supported sub-shape is already handled by the existing value-resolution path while unsupported flows remain named limitations. |
| `report/value-resolution-audit-delta` | done | Recorded zero-movement audit, scorecard, and trend evidence for the already-supported sub-shape; see the [Value Resolution Audit Delta](../handoffs/2026-05-22-value-resolution-audit-delta.md). |
| `dogfood/value-resolution-receipts` | done | Added checked dogfood receipt with canonical gap identity, raw context, zero movement, remaining limitations, and non-claims. |
| `campaign/value-resolution-audit-closeout` | done | Closed the rail with proof, remaining limits, and no selected successor; see the [Lane 1 Value Resolution Audit Fixes Closeout](../handoffs/2026-05-23-lane1-value-resolution-audit-fixes-closeout.md). |

## Audit Delta

The selected supported sub-shape is the same-test struct literal projection with
source-order and mutation guards pinned by the fixture corpus. The follow-up
analysis slice confirmed this shape was already supported before an analyzer
delta was required, so the report result is zero movement rather than a hidden
claim of improvement.

| Signal | Current result |
| --- | ---: |
| Sampled seams | 5000 of 40233 |
| Raw headline gaps | 1567 |
| Canonical gap groups | 1383 |
| Actionable canonical gaps | 61 |
| Missing discriminators | 149 |
| Static limitations | 1551 |
| Predicate-boundary limitations | 318 |
| `activation_value_unresolved` limitations | 315 |

Recent scorecard deltas were unchanged for duplicate-looking groups, missing
discriminators, total static limitations, actionable finding-alignment items,
and repair-route coverage. The trend report has status `unknown` because no
previous comparable snapshot was available and the current scorecard carries
bounded-run diagnostics.

Remaining `activation_value_unresolved` findings stay on
`analysis/value-resolution-audit-fixes`. This report does not prove runtime
behavior, mutation adequacy, coverage adequacy, public badge movement, or any
new product claim.

The durable handoff is
[Value Resolution Audit Delta](../handoffs/2026-05-22-value-resolution-audit-delta.md).

The rail closeout is
[Lane 1 Value Resolution Audit Fixes Closeout](../handoffs/2026-05-23-lane1-value-resolution-audit-fixes-closeout.md).

## Dogfood Receipt

`fixtures/finding-alignment-dogfood/corpus.json` includes
`value_resolution_struct_literal_projection_zero_movement`, a checked receipt
for the selected sub-shape. The receipt records:

- canonical gap id:
  `gap:value_resolution_struct_literal_projection_zero_movement`;
- raw finding context for the same-test struct literal projection;
- zero before/after movement from the already-supported implementation;
- `cargo xtask lane1-evidence-audit` as the verify command;
- no-action outcome for the selected supported sub-shape;
- explicit non-claims for unsupported value flows, runtime proof, mutation
  proof, coverage, badge, and gate authority.

## Operating Rules

- Start from audit and scorecard evidence.
- Fixture first for every supported value-resolution shape.
- Predicate-boundary actionability still requires concrete activation values.
- Unsupported helper-built, cross-file, generated, macro-expanded, shadowed,
  pattern-bound, non-literal, or opaque value flows remain named static
  limitations.
- Raw findings remain supporting evidence.
- Canonical evidence items remain the countable unit.
- Static limitations are analyzer work, not user test debt.
- Runtime-only signal does not create a static gap.

## Non-Goals

- No PR or CI rendering changes.
- No inline PR comment publishing.
- No LSP or editor polish.
- No gate-policy changes or default blocking.
- No public badge or score redefinition.
- No generated tests.
- No automatic source edits.
- No provider or model calls.
- No mutation execution.
- No release, packaging, platform, dependency, or MSRV cleanup.

## Validation Gates

Docs and planning slices should run:

```bash
cargo xtask goals status
cargo xtask goals next
cargo xtask check-goals
cargo xtask check-doc-index
cargo xtask markdown-links
cargo xtask check-static-language
cargo xtask check-pr
git diff --check
```

Fixture slices should also run:

```bash
cargo test -p xtask evidence_quality_benchmark
cargo xtask check-fixture-contracts
cargo xtask check-output-contracts
```

Analyzer, audit, and scorecard slices should run focused tests plus:

```bash
cargo xtask lane1-evidence-audit
cargo xtask evidence-quality-scorecard
cargo xtask evidence-quality-trend
cargo xtask check-output-contracts
cargo xtask check-traceability
cargo xtask check-capabilities
cargo xtask check-pr
git diff --check
```

## Closeout Rule

Close this rail only after:

- the selected supported sub-shape is fixture-backed;
- unsupported value flows remain named limitations;
- analyzer movement, if any, has before/after audit and scorecard proof;
- dogfood receipts record the material movement and non-claims;
- downstream consumers know whether any claim boundary changed;
- `.ripr/goals/active.toml` records the next successor or
  `no_current_goal = true`.
