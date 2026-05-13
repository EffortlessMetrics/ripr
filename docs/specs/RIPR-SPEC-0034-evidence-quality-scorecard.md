# RIPR-SPEC-0034: Evidence Quality Scorecard

Status: proposed

## Problem

Lane 1 has a stable shared `evidence_record` and a repo-local evidence-quality
audit. The next user problem is prioritization: maintainers can see many audit
signals, but they need a compact scorecard that explains which evidence classes
are strong, which are shallow or risky, what proof supports each maturity
claim, and which Lane 1 repair should happen next.

The scorecard must turn audit data into evidence-quality leadership without
changing RIPR's classifications, gate authority, PR projection, editor output,
or static vocabulary.

## Behavior

`cargo xtask evidence-quality-scorecard` generates an advisory repo-local
scorecard from existing Lane 1 evidence artifacts. The command writes:

```text
target/ripr/reports/evidence-quality-scorecard.json
target/ripr/reports/evidence-quality-scorecard.md
```

The scorecard reads the current Lane 1 evidence-quality audit and durable
evidence-health audit fields when they are available. It may regenerate the
repo-local audit through the existing `cargo xtask lane1-evidence-audit` path
when no current audit artifact exists, but it must not run mutation execution,
edit source files, update baselines, post PR comments, change gate behavior, or
change analyzer classifications.

The scorecard must summarize:

- evidence maturity by class;
- raw headline gaps;
- canonical gap groups;
- largest duplicate-looking groups;
- static limitation categories;
- missing discriminator classes;
- related-test confidence distribution;
- oracle semantics distribution;
- movement availability;
- calibration coverage;
- top recommended Lane 1 repair slices;
- recent audit deltas when a previous scorecard or audit snapshot is
  available.

The Markdown output is a bounded operator report. The JSON output is the
complete machine-readable record for future trend and closeout work.

`cargo xtask evidence-quality-trend` is the follow-on repo-local trend report
for this scorecard contract. It reads the current scorecard, compares it with
an optional previous scorecard or audit snapshot, and writes:

```text
target/ripr/reports/evidence-quality-trend.json
target/ripr/reports/evidence-quality-trend.md
```

Missing previous history must produce an explicit `unknown`/no-history state
instead of claiming improvement. When comparable history exists, the trend
report must distinguish improvement, regression, unchanged, and unknown
metrics. It must not redefine RIPR scores or change analyzer, gate, CI, PR,
editor, source-edit, generated-test, provider, or runtime-execution behavior.

## Required Evidence

Each scorecard must include:

- input artifact identity for the audit and evidence-health data it used;
- generated timestamp and repository scope;
- evidence maturity rows with class name, status, proof source, known limits,
  and recommended next repair;
- counts for raw headline gaps, canonical groups, duplicate-looking groups,
  missing discriminators, static limitations, normalized static-limitation
  categories and repair routes, related-test confidence, oracle semantics,
  movement availability, and calibration availability;
- top risks ordered by expected Lane 1 product impact, not raw count alone;
- class-scoped calibration coverage that distinguishes static-only,
  fixture-backed, imported-runtime-calibrated, and uncalibrated classes;
- before and after deltas when a comparable prior artifact is available;
- explicit unknowns for missing input artifacts, missing calibration, ambiguous
  runtime joins, opaque helpers, and unsupported oracle shapes.
- trend rows over current and previous scorecard or audit summary metrics when
  a previous artifact exists, including duplicate-looking groups, static
  limitations, low or opaque related-test choices, oracle unknown counts,
  uncalibrated records, calibrated records, and missing evidence records;
- an explicit no-history unknown when no previous trend input exists.

The scorecard must not report a class as stable or calibrated unless the row
names the fixture or runtime evidence that supports that scope.

## Inputs

- `target/ripr/reports/lane1-evidence-audit.json`
- `target/ripr/reports/evidence-health.json` when available
- optional previous scorecard or audit snapshot for recent deltas
- `docs/CAPABILITY_MATRIX.md` and `metrics/capabilities.toml` for current
  class-scoped maturity vocabulary
- `.ripr/traceability.toml` for proof links when available

`evidence-quality-trend` additionally accepts optional `--current <path>` and
`--previous <path>` arguments so maintainers can compare checked scorecard or
audit snapshots without inventing a new source of truth.

Missing optional inputs must be reported as unknown or unavailable. Missing
required audit input may be repaired by regenerating the audit; if regeneration
fails, the command exits with an actionable error and must not claim a
scorecard exists.

## Outputs

The JSON output includes:

- `schema_version`;
- `tool`;
- `generated_at`;
- `scope`;
- `inputs`;
- `summary`;
- `maturity_by_class`;
- `canonical_gap_groups`;
- `duplicate_looking_groups`;
- `static_limitation_categories`;
- `missing_discriminator_classes`;
- `related_test_confidence`;
- `oracle_semantics_distribution`;
- `movement_availability`;
- `calibration_coverage`;
- `recommended_repairs`;
- `recent_audit_deltas`;
- `unknowns`.

The Markdown output includes bounded sections for the same areas:

- summary;
- maturity by class;
- top evidence-quality risks;
- recommended Lane 1 repairs;
- duplicate-looking and canonical group signals;
- static limitations and missing discriminators;
- static limitation categories and repair routes;
- related-test and oracle distributions;
- movement and calibration coverage;
- recent deltas;
- unknowns and unavailable inputs.

High-cardinality details remain complete in JSON and capped in Markdown.

The evidence-quality trend JSON includes:

- `schema_version`;
- `tool`;
- `report`;
- `generated_at`;
- `scope`;
- `inputs`;
- `summary`;
- `metric_trends`;
- `static_limitation_category_trends`;
- `unknowns`.

The trend Markdown output includes bounded sections for summary, metric trends,
static limitation category trends, and unknowns.

## Non-Goals

- No analyzer behavior changes.
- No evidence score redefinition.
- No gate or policy decision.
- No PR or CI projection.
- No LSP or editor output.
- No generated tests or automatic source edits.
- No provider or model calls.
- No mutation execution.
- No capability promotion without separate proof-backed capability updates.
- No replacement for `lane1-evidence-audit`, `evidence-health`, repo exposure,
  or calibration reports.

## Acceptance Examples

Given duplicate-looking match-arm gaps, the scorecard groups them, names the
canonical group signal, and shows the audit delta after the analyzer fix. It
must not treat raw count reduction alone as proof that match-arm evidence is
globally stable.

Given no runtime calibration for a class, the scorecard marks the class
`static_only` or `uncalibrated` instead of presenting static evidence as
runtime-calibrated.

Given a high-confidence related test, the scorecard distinguishes it from a
low-confidence lexical-only match and preserves `related_tests_total` as
supporting context rather than a primary confidence claim.

Given an opaque helper or unsupported oracle shape, the scorecard records the
limitation category and recommended Lane 1 repair route instead of converting
the limitation into a user test gap.

Given no previous audit snapshot, the scorecard marks recent deltas
unavailable and still emits current maturity, risk, and repair sections.

Given no previous scorecard or audit snapshot, the trend report marks history
unavailable and emits `unknown` rather than claiming improvement.

Given a previous scorecard with fewer calibrated records and more
duplicate-looking groups, the trend report marks calibrated records and
duplicate-looking groups as improvement.

Given a previous scorecard with fewer static limitations than the current
scorecard, the trend report marks that metric as regression without changing
any gate behavior.

## Test Mapping

- `xtask::tests::evidence_quality_scorecard_renders_required_json_sections`
  pins required JSON sections and unavailable-input handling.
- `xtask::tests::evidence_quality_scorecard_markdown_names_required_sections`
  pins Markdown section coverage.
- `xtask::tests::evidence_quality_scorecard_classifies_maturity_by_proof_scope`
  pins static-only, fixture-backed, calibrated, and uncalibrated class rows.
- `xtask::tests::evidence_quality_scorecard_orders_repairs_by_risk_not_count`
  pins recommended repair ordering when count-only ordering would be wrong.
- `xtask::tests::evidence_quality_scorecard_reports_recent_deltas_when_present`
  pins before and after audit deltas.
- `xtask::tests::evidence_quality_trend_reports_no_history_explicitly` pins the
  no-history state.
- `xtask::tests::evidence_quality_trend_distinguishes_improvement_regression_and_unchanged`
  pins metric direction semantics.
- `xtask::tests::evidence_quality_trend_reports_static_limitation_category_deltas`
  pins normalized static-limitation category deltas.

## Implementation Mapping

- `xtask/src/command.rs` exposes `evidence-quality-scorecard`.
- `xtask/src/dispatch.rs`, `xtask/src/reports/mod.rs`, and
  `xtask/src/reports/repo.rs` route the report facade.
- `xtask/src/main.rs` loads or regenerates the Lane 1 audit, loads optional
  evidence-health and prior scorecard inputs, builds the scorecard, and writes
  JSON and Markdown artifacts.
- `docs/OUTPUT_SCHEMA.md` documents the scorecard JSON shape when the report
  implementation lands, plus the follow-on evidence-quality trend report.
- `docs/lanes/LANE_1_EVIDENCE_QUALITY_LEADERSHIP.md` records the scorecard as
  the first implementation slice and the trend report as the audit-delta slice
  when those tracker updates land.

## Metrics

The scorecard feeds these Lane 1 metrics:

- `lane1_evidence_scorecard_maturity_classes`;
- `lane1_evidence_scorecard_top_risks`;
- `lane1_evidence_scorecard_recommended_repairs`;
- `lane1_evidence_scorecard_static_only_classes`;
- `lane1_evidence_scorecard_calibrated_classes`;
- `lane1_evidence_scorecard_uncalibrated_classes`;
- `lane1_evidence_scorecard_recent_delta_available`.

The trend report feeds these Lane 1 metrics:

- `lane1_evidence_trend_compared_metrics`;
- `lane1_evidence_trend_improved_metrics`;
- `lane1_evidence_trend_regressed_metrics`;
- `lane1_evidence_trend_unchanged_metrics`;
- `lane1_evidence_trend_unknown_metrics`;
- `lane1_evidence_trend_no_history`;
- `lane1_evidence_trend_static_limitation_category_rows`.

## Validation

The implementation must be pinned by:

- focused xtask unit tests;
- `cargo xtask evidence-quality-scorecard`;
- `cargo xtask evidence-quality-trend`;
- `cargo xtask check-output-contracts`;
- `cargo xtask check-static-language`;
- `cargo xtask check-spec-format`;
- `cargo xtask check-traceability`;
- `cargo xtask check-capabilities`;
- `cargo xtask check-pr`.
