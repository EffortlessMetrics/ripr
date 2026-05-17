# RIPR-SPEC-0031: Lane 1 Evidence Quality Audit

Status: proposed

## Problem

Lane 1 has a stable `evidence_record` spine. The next risk is evidence quality:
RIPR can still overcount equivalent gaps, rank weak related tests too highly,
leave candidate values sparse, explain oracle semantics unevenly, or report
uncalibrated classes without enough visibility.

Before changing analyzer heuristics, maintainers need a repo-local audit that
measures those gaps from the existing repo exposure artifact.

## Product Contract

`cargo xtask lane1-evidence-audit` is an advisory repo-local report over
`seams[].evidence_record` data generated from repo exposure.

The command:

- generates repo exposure through the existing `ripr check --mode instant
  --format repo-exposure-json` path;
- streams `seams[].evidence_record` from the generated repo exposure JSON so
  the audit does not need to retain the full repo-exposure artifact in memory;
- writes deterministic JSON and Markdown reports under `target/ripr/reports`;
- summarizes evidence quality without changing classifications;
- does not alter gates, PR/CI projection, editor behavior, schemas outside this
  report, source files, baselines, or generated workflows;
- does not run mutation execution.

`cargo xtask evidence-quality-audit` is a compatibility alias for the same
repo-local report.

## Behavior

```text
cargo xtask lane1-evidence-audit
```

The command writes:

```text
target/ripr/reports/lane1-evidence-audit.json
target/ripr/reports/lane1-evidence-audit.md
```

It exits successfully after both artifacts are written. If repo exposure
generation exits non-zero after writing a complete repo-exposure JSON document
with a top-level `seams` array, the audit may continue from that captured
artifact with a warning. If the captured artifact is missing, malformed, or does
not contain `seams`, the command returns an actionable error and does not claim
an audit exists.

## JSON Contract

The JSON shape is defined in
[OUTPUT_SCHEMA.md](../OUTPUT_SCHEMA.md#lane-1-evidence-quality-audit). It
includes:

- `schema_version`;
- `tool`;
- `report`;
- `scope`;
- `status`;
- `inputs`;
- `summary`;
- `finding_alignment`;
- `canonical_gap_groups`;
- `duplicate_looking_groups`;
- `missing_discriminator_classes`;
- `static_limitations`;
- `oracle_semantics_distribution`;
- `related_test_ranking`;
- `movement_availability`;
- `calibration_availability`;
- `evidence_record_field_health`;
- `top_files_by_unresolved_evidence_debt`.

The report is additive and repo-local. It is not a replacement for
`repo-exposure.json`, `evidence-health.json`, or calibration reports.

## Markdown Contract

The Markdown sibling prints the same audit areas in bounded tables:

- summary;
- finding alignment;
- largest canonical gap groups;
- duplicate-looking groups;
- missing discriminator classes;
- static limitations;
- oracle semantics;
- related-test ranking;
- movement availability;
- calibration availability;
- evidence record field health;
- top files by unresolved evidence debt.

High-cardinality count maps remain complete in JSON and are capped in Markdown.

## Required Evidence

The audit must summarize:

- raw headline gaps;
- finding-alignment raw signals, canonical items, actionability states, and
  raw-to-canonical counts derived from `evidence_record.canonical_item`;
- finding-alignment coverage by evidence class, unaligned raw finding examples,
  same-line duplicate groups, static-unknown items without named limitations,
  and canonical items missing repair or verification guidance;
- canonical gap groups;
- largest canonical groups;
- duplicate-looking groups;
- missing discriminator classes;
- static limitations by reason, stage, normalized category, and repair route;
- oracle semantics distribution;
- related-test ranking confidence;
- movement availability fields;
- calibration availability;
- calibrated versus uncalibrated records;
- `evidence_record` missing, nullable, or empty fields;
- top files by unresolved evidence debt.

## Acceptance Examples

Given two headline seams with the same canonical gap ID, the audit reports one
canonical group and lists that group as duplicate-looking.

Given evidence records that carry `canonical_item`, the audit reports a
`finding_alignment.summary` so the scorecard can count canonical items,
actionable items, observed items, static limitations, calibration support, and
raw-to-canonical alignment without requiring a separate top-level projection.

Given evidence records with and without `canonical_item`, the audit reports
`finding_alignment.coverage` so maintainers can see which evidence classes are
aligned, which raw findings remain unaligned, whether duplicate raw findings
share a file and line, and whether canonical items lack repair routes,
verification commands, or named static-limitation categories.

Given a static-unknown or limitation-shaped canonical item, a limitation is
named only when it carries a non-generic category and repair route. Generic
`static_unknown` or `unknown` categories remain counted under
`static_unknown_without_named_limitation` so unknowns stay visible as analyzer
work instead of becoming vague user test debt.

Given a headline seam with no canonical gap ID, the audit counts it under
`headline_without_canonical_gap_id`.

Given missing discriminators, static limitations, low-confidence top related
tests, or no related tests, the audit increments the matching distributions and
file-debt rows. Static limitations are grouped by normalized analyzer category
and repair route without treating those categories as user-actionable test
gaps.

Given records with `calibration.availability = "not_imported"`, the audit counts
them as uncalibrated. Imported availability counts as calibrated scope for this
audit report only; it does not change static classifications.

## Test Mapping

- `xtask::tests::lane1_evidence_audit_counts_quality_gaps_from_evidence_record`
  pins JSON counts for canonical groups, duplicate groups, missing
  discriminators, static limitation categories, ranking confidence,
  calibration, derived finding-alignment summary, alignment coverage, and
  field health.
- `xtask::tests::lane1_evidence_audit_reports_alignment_coverage_holes` pins
  unaligned raw finding examples and same-line duplicate grouping.
- `xtask::tests::lane1_evidence_audit_rejects_generic_static_unknown_limitation_category`
  pins that generic `static_unknown` does not satisfy the named-limitation
  requirement.
- `xtask::tests::lane1_evidence_audit_markdown_names_required_sections` pins
  Markdown section coverage.
- `xtask::tests::lane1_evidence_audit_rejects_repo_exposure_without_seams` pins
  malformed input handling.
- `xtask::tests::lane1_repo_exposure_file_completion_check_requires_seams_and_closing_brace`
  pins captured repo-exposure fallback acceptance after a non-zero generator
  exit.
- `xtask::tests::xtask_command_parse_preserves_compatibility_aliases` pins the
  `evidence-quality-audit` alias.
- `xtask::tests::report_commands_dispatch_through_report_facades` keeps the
  xtask report facade routed.

## Implementation Mapping

- `xtask/src/command.rs` exposes `lane1-evidence-audit` and the
  `evidence-quality-audit` alias.
- `xtask/src/dispatch.rs`, `xtask/src/reports/mod.rs`, and
  `xtask/src/reports/repo.rs` route the report facade.
- `xtask/src/main.rs` generates repo exposure, builds the audit, renders JSON
  and Markdown, and writes the artifacts.
- `xtask/src/run.rs` provides the stdout-to-file command runner used to stream
  the generated repo-exposure input without adding process-spawn logic to the
  report implementation.
- `docs/OUTPUT_SCHEMA.md` documents the report shape.
- `docs/lanes/LANE_1_EVIDENCE_ACCURACY.md` records this as the audit-first
  Lane 1 slice.

## Metrics

The audit feeds these Lane 1 metrics:

- `lane1_evidence_audit_raw_headline_gaps`;
- `lane1_evidence_audit_canonical_gap_groups`;
- `lane1_evidence_audit_duplicate_looking_groups`;
- `lane1_evidence_audit_missing_discriminators`;
- `lane1_evidence_audit_static_limitations`;
- `lane1_evidence_audit_uncalibrated_records`.
- `finding_alignment_raw_signals_total`;
- `finding_alignment_canonical_items_total`;
- `finding_alignment_actionable_items_total`;
- `finding_alignment_static_limitation_total`.
- `finding_alignment_coverage_by_class`;
- `finding_alignment_unaligned_raw_findings_by_class`;
- `finding_alignment_static_unknown_without_named_limitation`;
- `finding_alignment_canonical_items_without_repair_route`;
- `finding_alignment_canonical_items_without_verify_command`.

## Non-Goals

- No analyzer behavior changes.
- No gate or policy decision.
- No PR or CI projection.
- No LSP, editor, provider, release, dependency, or platform work.
- No mutation execution.
- No generated tests or source edits.
- No evidence-health field folding in this slice.

## Validation

The implementation is pinned by:

- focused xtask unit tests;
- `cargo xtask lane1-evidence-audit`;
- `cargo xtask check-output-contracts`;
- `cargo xtask check-static-language`;
- `cargo xtask check-traceability`;
- `cargo xtask check-capabilities`;
- `cargo xtask check-pr`.
