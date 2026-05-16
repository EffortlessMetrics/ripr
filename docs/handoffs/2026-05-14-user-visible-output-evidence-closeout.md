# Handoff: Lane 1 User-Visible Output Evidence Closeout

Date: 2026-05-14
Branch / PR: `lane1-user-visible-output-closeout` / #966
Latest merged PR: #959 `docs: hand off presentation text consumer contract` (commit `595a9933`)

## Current work item

`campaign/user-visible-output-evidence-closeout`

Lane 1 User-Visible Output Evidence is complete in the documented presentation
text scope. The lane turned raw line-local findings for changed
presentation/help/report/table text into canonical evidence items that
distinguish actionable output-observer gaps, already-observed output,
internal-only no-action labels, and named static limitations.

The PR chain:

- #904 opened RIPR-PROP-0005 and the Lane 1 tracker.
- #909 added RIPR-SPEC-0043 for presentation text evidence.
- #927 added RIPR-SPEC-0045 for finding-to-gap alignment.
- #900 added the screenshot-derived presentation text benchmark.
- #931 added finding-alignment benchmark cases.
- #935 added additive `raw_findings[]`, `canonical_item`, and nullable
  `presentation_text` fields to `evidence_record`.
- #943 grouped supported declaration plus adjacent literal findings into one
  canonical `finding_alignment.items[]` item.
- #947 classified fixture-backed visibility and observer states for supported
  help, report, internal, and visibility-unknown text.
- #951 added concrete repair kind, target test type, and suggested assertion
  fields for presentation text canonical items.
- #957 added scorecard and trend counts for finding alignment and
  presentation text evidence quality.
- #959 handed the canonical evidence item contract to downstream lanes.
- #966 closed the focused Lane 1 tracker and added the final observer-unknown
  benchmark guard.

## Evidence state

Lane 1 now has a clean counting split for this evidence class:

```text
Raw findings are supporting evidence.
Canonical evidence items are the countable unit.
Actionable canonical gaps are the user-facing problem.
```

Implemented proof includes:

- `finding_alignment.items[]` in `ripr check --json` for supported
  presentation-like Rust `&str` constants;
- `seams[].evidence_record.canonical_item` and `raw_findings[]` as additive
  repo-exposure evidence-record fields;
- fixture-backed grouping for declaration plus adjacent literal findings;
- line-movement identity and non-collision guards;
- visible unobserved output classified as actionable output-observer repair;
- visible observed output classified as already observed;
- internal labels classified as no action;
- visibility-unknown and observer-unknown states classified as static
  limitations, not user test debt;
- concrete presentation-text repair fields: `repair_kind`,
  `target_test_type`, and `suggested_assertion`;
- scorecard and trend metrics for raw signals, canonical items,
  raw-to-canonical ratio, duplicate groups, actionable items, no-action items,
  static limitations, visibility unknowns, observed text, and output-observer
  repairs.

The lane did not change PR/CI rendering, LSP/editor behavior, gates, scores,
generated tests, provider calls, source editing, default blocking, or mutation
execution.

## Remaining unknowns

- Presentation-text support is conservative and Rust-focused in documented
  scope. Unsupported macros, dynamic dispatch, opaque helpers, and untraceable
  cross-file formatting remain limitations.
- Observer discovery is limited to supported snapshot, help-output, report,
  table, and golden-output shapes. Other observer topologies stay
  `presentation_text_observer_unknown`.
- Downstream PR/CI and editor surfaces still need separate lane work to render
  canonical items instead of raw findings.
- Baseline, acknowledgement, suppression, waiver, blocking, resolved, and
  reintroduced states remain policy overlays outside Lane 1 evidence truth.

## Next work item

Do not reopen this tracker for downstream projection work. The next work should
open in the owning lane when a PR/CI, editor, agent, or policy surface is ready
to consume canonical evidence items directly.

Future Lane 1 work should require a new measured evidence class or a concrete
scorecard/regression signal. Candidate Lane 1 repairs include:

- broader presentation-text output tracing for currently unsupported macro or
  helper shapes;
- observer extraction for unsupported snapshot/help/report/table/golden
  topologies;
- additional non-Rust presentation-text evidence once preview-language policy
  explicitly promotes that scope.

## Open decisions

- None for this closeout.

## Current blockers

- None.

## Verification run

The preceding report and handoff slices were validated with:

```text
rtk cargo fmt --check
rtk cargo test -p xtask finding_alignment_presentation_text
rtk cargo test -p xtask evidence_quality_scorecard
rtk cargo test -p xtask evidence_quality_trend
rtk cargo xtask check-output-contracts
rtk cargo xtask check-traceability
rtk cargo xtask check-capabilities
rtk cargo xtask check-static-language
rtk cargo xtask check-pr
rtk git diff --check
```

This closeout PR should run:

```text
rtk cargo test -p xtask evidence_quality_benchmark
rtk cargo xtask check-fixture-contracts
rtk cargo xtask check-output-contracts
rtk cargo xtask check-doc-index
rtk cargo xtask markdown-links
rtk cargo xtask check-static-language
rtk cargo xtask check-traceability
rtk cargo xtask check-capabilities
rtk cargo xtask check-pr
rtk git diff --check
```

## Artifacts

- [Lane 1 User-Visible Output Evidence tracker](../lanes/LANE_1_USER_VISIBLE_OUTPUT_EVIDENCE.md)
- [RIPR-PROP-0005](../proposals/RIPR-PROP-0005-user-visible-output-evidence.md)
- [RIPR-SPEC-0043](../specs/RIPR-SPEC-0043-presentation-text-evidence.md)
- [RIPR-SPEC-0045](../specs/RIPR-SPEC-0045-finding-to-gap-alignment.md)
- [Presentation text consumer handoff](2026-05-14-presentation-text-consumer-handoff.md)
- `fixtures/evidence-quality-benchmark/corpus.json`
- `target/ripr/reports/evidence-quality-scorecard.{json,md}`
- `target/ripr/reports/evidence-quality-trend.{json,md}`

## Recommended next action

Use the canonical evidence item contract in the next downstream projection lane
that is ready to stop rendering raw findings as independent user work.

## What not to do

- Do not change PR/CI rendering in Lane 1.
- Do not change LSP/editor behavior.
- Do not change gate policy, default blocking, scores, schemas, generated
  tests, provider calls, source edits, or mutation execution.
- Do not treat text alone as user test debt.
- Do not infer actionability from raw `exposed` or `static_unknown` labels.
- Do not reopen generic Evidence Quality Leadership work.
