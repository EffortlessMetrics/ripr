# Handoff: Value Resolution Audit Delta

Date: 2026-05-22

Branch: `docs-value-resolution-audit-delta-handoff`

Related work item: `report/value-resolution-audit-delta`

## Current State

The Lane 1 Value Resolution Audit Fixes rail has fixture-backed the selected
same-test struct literal field projection sub-shape and recorded the audit
delta. The selected shape was already supported by the current analyzer, so the
report result is intentionally zero movement rather than a hidden improvement
claim.

This handoff preserves the durable audit-delta state for later dogfood and
closeout work. It does not change analyzer behavior, output schemas, PR/CI
rendering, editor behavior, gates, badges, release behavior, generated tests,
source edits, provider calls, or mutation execution.

## Selected Sub-Shape

Supported:

- same-test struct literal fields with concrete literal values;
- owner calls that later pass a field projection such as `case.amount`;
- source-order and same-line column scoping at the owner call;
- mutation or shadowing guards that prevent stale field values from being
  promoted.

Still unsupported and still named as limitations:

- helper-returned structs;
- non-literal fields;
- mutable bindings and mutations before the owner call;
- shadowed, pattern-bound, fixture, or parameterized values;
- cross-file constants;
- macros, generated inputs, and deeper semantic value resolution.

## Audit Snapshot

Current sampled Lane 1 audit:

| Metric | Count |
| --- | ---: |
| Sampled seams / canonical items | 5,000 |
| Raw headline gaps | 1,567 |
| Canonical gap groups | 1,383 |
| Duplicate-looking groups | 130 |
| Missing discriminators | 149 |
| Static limitations | 1,551 |
| Finding-alignment raw signals | 5,630 |
| Finding-alignment canonical items | 5,000 |
| Aligned raw findings | 5,630 |
| Unaligned raw findings | 0 |
| Actionable canonical items | 61 |
| Finding-alignment static limitations | 1,506 |
| Canonical items without repair route | 0 |
| Canonical items without verify command | 0 |

The run is bounded by:

```text
repo_exposure_seam_limit = limit_5000_of_40233
```

This is a bounded work-queue signal, not a full-repo exposure completion claim.

## Predicate Boundary Queue

| Metric | Count |
| --- | ---: |
| Raw signals | 975 |
| Canonical items | 601 |
| Actionable items | 61 |
| Already observed items | 222 |
| Static limitations | 318 |
| Unknown items | 0 |

The dominant named static limitation under `predicate_boundary` remains:

| Static limitation | Count | Repair route |
| --- | ---: | --- |
| `activation_value_unresolved` | 315 | `analysis/value-resolution-audit-fixes` |

## Delta

| Metric | Before | After | Delta |
| --- | ---: | ---: | ---: |
| Static limitations | 1,551 | 1,551 | 0 |
| Finding-alignment static limitations | 1,506 | 1,506 | 0 |
| Missing discriminators | 149 | 149 | 0 |
| `activation_value_unresolved` | 315 | 315 | 0 |

The trend report status is `unknown` because no previous comparable scorecard
or audit snapshot was available. It reports the current
`activation_value_unresolved` value as 315 but cannot claim improvement or
regression.

## What This Proves

- The selected fixture-backed sub-shape is not missing production support in
  the current analyzer.
- Unsupported value flows remain named static limitations rather than user test
  debt.
- The next useful work is proof-oriented dogfood that records material
  value-resolution receipts and non-claims.

## What This Does Not Prove

- no runtime adequacy;
- no mutation proof;
- no coverage adequacy;
- no merge approval;
- no default gate authority;
- no badge semantic change;
- no downstream PR/CI or editor projection change;
- no full-repo exposure completion beyond the sampled 5,000-seam run.

## Validation

Recorded by `report/value-resolution-audit-delta`:

```bash
cargo xtask lane1-evidence-audit
cargo xtask evidence-quality-scorecard
cargo xtask evidence-quality-trend
cargo xtask check-output-contracts
cargo xtask check-doc-index
cargo xtask markdown-links
cargo xtask check-static-language
cargo xtask check-traceability
cargo xtask check-capabilities
cargo xtask check-pr
git diff --check
```

## Next Work

`dogfood/value-resolution-receipts` is the next ready work item. It should
record material value-resolution dogfood receipts with canonical gap identity,
raw finding context, before/after limitation movement or no-movement state,
repair route or limitation state, verify command where applicable, and explicit
static-evidence non-claims.
