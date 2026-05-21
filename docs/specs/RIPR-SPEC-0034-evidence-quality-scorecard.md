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

When the audit contains `finding_alignment.summary` derived from
`evidence_record.canonical_item`, the scorecard uses those values for
raw-to-canonical, actionability, observed, static-limitation, and calibration
counts. It reports finding alignment as unavailable only when the audit lacks
both that canonical-item-derived summary and a compatible top-level projection
summary.

The scorecard must lead with actionable canonical gaps when finding-alignment
counts are available. Raw finding counts remain diagnostic context, while
canonical items are the countable evidence unit and actionable canonical gaps
are the user-facing repair count. This headline does not redefine public badges
or gate policy.

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
- audit-derived actionable canonical gap top lists for classes, files, repair
  kinds, missing discriminator kinds, static limitation reasons on actionable
  gap records, and guidance-unknown classes;
- audit-derived actionable-gap packet public-projection readiness counts,
  including eligible packets, excluded packets, and exclusion reasons;
- explicit unknowns when the audit or evidence-health input reports bounded
  run limitations, so zero or missing counts from limited artifacts are not
  treated as complete repo truth;
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
- counts for raw findings/signals, canonical items, actionable items,
  already-observed items, static limitations, and raw-to-canonical ratio when
  canonical alignment data is present in the audit;
- top actionable canonical gap lists copied from the Lane 1 audit so the
  scorecard shows the live shape of user work without reconstructing it from
  raw findings;
- actionable-gap packet public-projection readiness copied from the Lane 1
  audit so badge-readiness work can trend packet completeness without changing
  public badge behavior;
- a headline object that names
  `finding_alignment_actionable_unresolved_canonical_gaps` as the primary
  scorecard count while preserving raw signal, canonical item, no-action,
  limitation, unknown, and raw-to-canonical context;
- top risks ordered by expected Lane 1 product impact, not raw count alone;
- class-scoped calibration coverage that distinguishes static-only,
  fixture-backed, imported-runtime-calibrated, and uncalibrated classes;
- before and after deltas when a comparable prior artifact is available;
- explicit unknowns for missing input artifacts, bounded run limitations,
  missing calibration, ambiguous runtime joins, opaque helpers, and unsupported
  oracle shapes.
- trend rows over current and previous scorecard or audit summary metrics when
  a previous artifact exists, including duplicate-looking groups, static
  limitations, low or opaque related-test choices, oracle unknown counts,
  uncalibrated records, calibrated records, missing evidence records, and
  actionable-gap packet public-projection readiness;
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
fails before a complete audit artifact exists, the command must write a bounded
diagnostic scorecard with
`unknowns[].kind = "evidence_quality_scorecard_audit_regeneration_failed"` and
a matching audit `run_limitations[]` category. That limited scorecard must not
claim complete repo truth, public badge readiness, or user test debt. If the
audit or evidence-health artifact exists but carries `run_limitations[]`, the
scorecard must surface that as an unknown and must not let the limited
artifact's zero or partial counts masquerade as complete repo truth.

## Outputs

The JSON output includes:

- `schema_version`;
- `tool`;
- `generated_at`;
- `scope`;
- `inputs`;
- `headline`;
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
- `actionable_gap_top_lists`;
- `actionable_gap_packet_public_projection`;
- `evidence_class_work_queue`;
- `recommended_repairs`;
- `recent_audit_deltas`;
- `unknowns`.

The Markdown output includes bounded sections for the same areas:

- summary;
- maturity by class;
- actionable canonical gap top lists;
- actionable-gap packet public-projection readiness;
- evidence-class work queue, including dominant static limitation category and
  repair route for static-dominated rows;
- top evidence-quality risks;
- recommended Lane 1 repairs;
- duplicate-looking and canonical group signals;
- static limitations and missing discriminators;
- static limitation categories and repair routes;
- related-test and oracle distributions;
- movement and calibration coverage;
- runtime confidence coverage by evidence class;
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

Given a Lane 1 audit with runtime confidence rows by evidence class, the
scorecard carries them under `calibration_coverage.by_evidence_class` and prints
the same bounded table in Markdown. Rows distinguish calibrated-supported,
fixture-backed, static-only, unknown-confidence, uncalibrated, actionable, and
limitation items without changing public badge semantics.

Given a high-confidence related test, the scorecard distinguishes it from a
low-confidence lexical-only match and preserves `related_tests_total` as
supporting context rather than a primary confidence claim.

Given an opaque helper or unsupported oracle shape, the scorecard records the
limitation category and recommended Lane 1 repair route instead of converting
the limitation into a user test gap.

Given no previous audit snapshot, the scorecard marks recent deltas
unavailable and still emits current maturity, risk, and repair sections.

Given a Lane 1 audit with `evidence_record.canonical_item` data, the scorecard
reports finding-alignment counts from the audit summary instead of treating
alignment as unavailable merely because no separate top-level
`finding_alignment.items[]` projection was present.

Given a Lane 1 audit with raw signals, canonical items, and actionable canonical
gaps, the scorecard headline uses actionable canonical gaps as the primary
count and keeps raw signals as diagnostic context.

Given a Lane 1 audit with actionable gap top lists, the scorecard carries those
lists forward unchanged so agents and maintainers can see the dominant evidence
classes, files, repair kinds, missing discriminator kinds, static limitation
reasons on actionable gap records, and guidance-unknown classes without
inferring from raw findings.

Given a Lane 1 audit with actionable-gap packet public-projection readiness,
the scorecard carries the eligible/excluded packet counts and projection
exclusion reasons forward. The trend report tracks eligible packets as
higher-is-better and excluded packets as lower-is-better. Neither report
changes public badge semantics.

Given a Lane 1 audit with an evidence-class work queue, the scorecard carries
that queue forward so operators can choose the next repair class from live
actionable/static-limitation/unknown/unaligned/duplicate signals instead of a
static roadmap guess. This queue does not change public badge semantics.

Given a Lane 1 audit or evidence-health artifact with `run_limitations[]`, the
scorecard adds `lane1_evidence_audit_limited` or `evidence_health_limited` to
`unknowns` so downstream users can see that the report is bounded diagnostic
evidence rather than complete repo truth.

Given a failed attempt to regenerate a missing Lane 1 audit, the scorecard
emits a bounded diagnostic scorecard and adds
`evidence_quality_scorecard_audit_regeneration_failed` to `unknowns` instead of
silently dropping the report.

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
- `xtask::tests::evidence_quality_scorecard_uses_audit_canonical_item_alignment_summary`
  pins the fallback from audit-derived `canonical_item` alignment summary and
  runtime confidence coverage by evidence class.
- `xtask::tests::evidence_quality_scorecard_carries_actionable_gap_top_lists_from_audit`
  pins scorecard propagation of the audit-derived actionable gap top lists.
- `xtask::tests::evidence_quality_scorecard_carries_actionable_packet_projection_readiness`
  pins scorecard propagation of packet-level public-projection readiness.
- `xtask::tests::evidence_quality_scorecard_surfaces_limited_inputs_as_unknowns`
  pins scorecard unknowns for bounded audit and evidence-health input
  limitations.
- `xtask::tests::evidence_quality_scorecard_names_audit_regeneration_failure`
  pins the bounded diagnostic scorecard for failed missing-audit regeneration.
- `xtask::tests::evidence_quality_scorecard_headline_prefers_actionable_canonical_gaps`
  pins the scorecard headline counting model.
- `xtask::tests::evidence_quality_trend_reports_no_history_explicitly` pins the
  no-history state.
- `xtask::tests::evidence_quality_trend_distinguishes_improvement_regression_and_unchanged`
  pins metric direction semantics.
- `xtask::tests::evidence_quality_trend_reports_static_limitation_category_deltas`
  pins normalized static-limitation category deltas.
- `xtask::tests::evidence_quality_trend_reports_finding_alignment_presentation_text_deltas`
  pins finding-alignment and packet-readiness metric deltas.

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
- `lane1_evidence_scorecard_runtime_confidence_by_class`;
- `lane1_evidence_scorecard_calibrated_classes`;
- `lane1_evidence_scorecard_uncalibrated_classes`;
- `lane1_evidence_scorecard_recent_delta_available`.
- `lane1_evidence_scorecard_limited_input_unknowns`.
- `lane1_evidence_scorecard_actionable_packet_projection_eligible`;
- `lane1_evidence_scorecard_actionable_packet_projection_excluded`.

The trend report feeds these Lane 1 metrics:

- `lane1_evidence_trend_compared_metrics`;
- `lane1_evidence_trend_improved_metrics`;
- `lane1_evidence_trend_regressed_metrics`;
- `lane1_evidence_trend_unchanged_metrics`;
- `lane1_evidence_trend_unknown_metrics`;
- `lane1_evidence_trend_no_history`;
- `lane1_evidence_trend_static_limitation_category_rows`.
- `lane1_evidence_trend_actionable_packet_projection_eligible`;
- `lane1_evidence_trend_actionable_packet_projection_excluded`.

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
