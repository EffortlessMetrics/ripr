# RIPR-SPEC-0015: Evidence Health Baseline

Status: proposed

## Problem

Lane 1 owns RIPR's analyzer evidence graph: seams, related tests, observed
values, oracle strength, missing discriminators, static limitations, and
optional imported calibration context. Downstream surfaces such as PR guidance,
LSP actions, agent packets, SARIF, badges, and gates are only useful when that
evidence can be inspected as a whole.

Before changing analyzer heuristics, maintainers need a compact health report
that answers:

```text
Which evidence classes exist today, where are the weak or unknown areas, and
how much imported calibration context is available?
```

The report must remain a measurement surface. It must not change analyzer
behavior, make policy decisions, run mutation testing, generate tests, edit
source files, or alter CI defaults.

## Product Contract

`ripr evidence-health` is an advisory report over existing repo seam evidence.
It summarizes analyzer-health counts so Lane 1 work can improve evidence
quality deliberately.

The contract is:

- missing config keeps normal defaults;
- static seam evidence is computed the same way as repo exposure;
- audit-style fields are derived from the shared `evidence_record` projection
  and canonical gap identity;
- the command writes deterministic JSON and Markdown;
- calibration input is optional and read-only;
- imported calibration contributes availability counts only;
- runtime-specific vocabulary remains confined to the calibration availability
  section;
- no analyzer classifications, suppressions, report schemas, gates, SARIF,
  badges, LSP behavior, or generated workflows are changed.

## Behavior

```text
ripr evidence-health \
  --root . \
  --out target/ripr/reports/evidence-health.json \
  --out-md target/ripr/reports/evidence-health.md \
  --mutation-calibration target/ripr/reports/mutation-calibration.json
```

All flags are optional except when callers want non-default paths:

- `--root` defaults to the current directory.
- `--out` defaults to `target/ripr/reports/evidence-health.json`.
- `--out-md` defaults to `target/ripr/reports/evidence-health.md`.
- `--mutation-calibration` accepts an already-produced calibration JSON report.

`cargo xtask evidence-health` is the repo-local automation facade and writes the
same default artifacts. If `target/ripr/reports/mutation-calibration.json`
already exists, the xtask command includes it as optional calibration context.
The facade bounds the live child process with
`RIPR_EVIDENCE_HEALTH_TIMEOUT_MS` (default 20 minutes). This includes both the
preflight `cargo build -p ripr` phase and the `ripr evidence-health` generation
phase. The default matches the Lane 1 audit live-repo budget so normal dogfood
runs can complete useful health counts, while still bounding pathological
inputs before they can silently drop the artifact.
During report generation, xtask enables repo-exposure latency tracing so timeout
or incomplete artifacts can include phase breadcrumbs when the analyzer emits
them.
On timeout or incomplete child-process exit it removes stale or partial
outputs and writes warning JSON/Markdown with phase context such as
`evidence_health_build` or `evidence_health_generation` plus the named
`evidence_health_timeout` or `evidence_health_incomplete` run limitation instead
of waiting forever or pretending missing counts mean no evidence debt.

The command:

- loads repo configuration using normal precedence and defaults;
- inventories classified repo seams using the existing analyzer path;
- aggregates evidence-health counts without mutating classifications;
- optionally reads an already-produced mutation calibration JSON report;
- writes JSON and Markdown artifacts before exiting successfully.

## JSON Contract

The JSON shape is defined in
[OUTPUT_SCHEMA.md](../OUTPUT_SCHEMA.md#evidence-health-report). It includes:

- `schema_version`;
- `scope`;
- `status`;
- `inputs`;
- `metrics`;
- `evidence_quality`;
- `calibration`;
- `top_static_limitations`;
- `run_limitations` on bounded fallback artifacts.

The `metrics` object includes:

- total seams and headline-eligible seams;
- per-`SeamGripClass` counts;
- per-stage `StageState` counts;
- unknown/opaque stage counts;
- unknown/opaque class buckets;
- missing discriminator totals and `{label, count}` value rows;
- observed value totals and value-context counts;
- related-test totals and confidence counts;
- oracle kind and strength counts;
- opaque-oracle counts.

The `evidence_quality` object includes:

- canonical gap group totals and largest groups;
- duplicate-looking canonical group totals;
- actionability class counts from `evidence_record.actionability`;
- static limitation stage distributions and `{label, count}` reason rows from
  `evidence_record.static_limitations`;
- calibration availability counts from `evidence_record.calibration`;
- evidence movement availability counts for seam ID, canonical gap ID, complete
  evidence path, recommendation, and verify command fields;
- top evidence-quality risks.

The `calibration` object includes availability counts from imported calibration
JSON when supplied. It does not infer trust thresholds or change static output.

## Markdown Contract

The Markdown sibling is reviewer-facing and bounded:

- summary counts;
- grip class table;
- top missing discriminator counts, capped for readability;
- oracle strength table;
- related-test confidence table;
- evidence-quality summary;
- largest canonical gap groups;
- actionability distribution;
- static limitation distribution;
- evidence-record calibration coverage;
- top evidence-quality risks;
- calibration availability table;
- top static limitations.

High-cardinality details remain complete in JSON.

## Required Evidence

The report must include:

- grip class counts for every `SeamGripClass` bucket;
- per-stage `StageState` counts for reach, activate, propagate, observe, and
  discriminate;
- unknown/opaque stage and class buckets;
- missing discriminator count rows;
- observed value context counts;
- related-test confidence counts;
- oracle kind and strength counts;
- opaque-oracle counts;
- canonical gap group and duplicate-looking group counts;
- actionability class counts from the shared evidence record;
- static limitation stage counts and reason count rows from the shared evidence
  record;
- evidence-record calibration availability counts;
- evidence movement availability counts;
- top evidence-quality risks;
- top static limitations with an example seam ID;
- optional calibration availability counts when a calibration report is
  supplied.

## Acceptance Examples

Given classified seams with one weakly gripped boundary gap and one ungripped
opaque call seam, the report counts both grip classes, counts the missing
boundary discriminator, counts the observed activation value, and records the
opaque oracle as a top static limitation.

Given two line-moved seams for the same canonical behavioral gap, the report
keeps both raw seams visible while counting one duplicate-looking canonical gap
group and listing that group in the largest canonical groups table.

Given an imported calibration report with matched rows, static-only rows,
runtime rows without static seams, ambiguous file-line joins, and unmatched
runtime rows, the report copies only the availability counts into the
calibration section.

Given no calibration input, the report marks calibration as `not_provided` and
still succeeds.

Given the xtask evidence-health child process times out or exits before a
complete report is available, the command writes bounded warning artifacts with
`status = "warn"`, a `run_limitations[].category = "evidence_health_timeout"` or
`"evidence_health_incomplete"` entry, phase/input context,
timeout/duration/output byte counts, bounded stdout/stderr excerpts, exit
status when available, and a repair route. The limited artifact records
`inputs.generation.status = "timeout"` for timed-out children and `"fail"` for
nonzero or missing status exits. The limited artifact is diagnostic only and
does not claim user test debt from missing health counts.

## Test Mapping

- `crates/ripr/src/output/evidence_health.rs::tests::evidence_health_counts_core_metrics`
  pins core JSON metric counting and evidence-quality audit fields.
- `crates/ripr/src/output/evidence_health.rs::tests::evidence_health_markdown_names_calibration_and_limitations`
  pins Markdown rendering, evidence-quality sections, and calibration
  availability rows.
- `crates/ripr/src/cli/commands.rs::tests::evidence_health_parses_default_and_full_option_surface`
  pins CLI defaults and optional inputs.
- `crates/ripr/src/cli/commands.rs::tests::evidence_health_rejects_unknown_arguments`
  pins argument validation.
- `xtask::tests::evidence_health_timeout_writes_named_limitation_reports`
  pins the bounded xtask timeout fallback, stale-output cleanup, named
  limitation category, and repair route.
- `xtask::tests::evidence_health_build_timeout_writes_named_limitation_reports`
  pins the bounded preflight build fallback, phase diagnostics, stale-output
  cleanup, named limitation category, and repair route.
- `xtask::tests::evidence_health_incomplete_exit_writes_named_limitation_reports`
  and
  `xtask::tests::evidence_health_nonzero_exit_writes_named_limitation_reports`
  pin incomplete-exit fallback artifacts, stale-output cleanup, exit-status
  diagnostics, named limitation category, and repair route.
- `xtask::tests::evidence_health_output_excerpt_is_bounded` pins stdout/stderr
  excerpt bounds for limited warning artifacts.

## Implementation Mapping

- `crates/ripr/src/output/evidence_health.rs` builds and renders the report.
- `crates/ripr/src/cli/commands.rs` parses and runs `ripr evidence-health`.
- `crates/ripr/src/cli/command.rs`, `execute.rs`, and `help.rs` expose the CLI
  command.
- `xtask/src/command.rs`, `dispatch.rs`, `main.rs`, and `reports/repo.rs`
  expose `cargo xtask evidence-health`; `xtask/src/main.rs` also bounds the
  child process and writes timeout or incomplete-exit limitation fallback
  artifacts.
- `docs/OUTPUT_SCHEMA.md` defines the public JSON and Markdown contract.

## Metrics

The evidence-health baseline feeds these Lane 1 metrics:

- `evidence_health_seams_total`;
- `evidence_health_missing_discriminators_total`;
- `evidence_health_observed_values_total`;
- `evidence_health_related_tests_total`;
- `evidence_health_opaque_oracle_count`;
- `evidence_health_calibration_matched_total`;
- `evidence_health_canonical_gap_groups`;
- `evidence_health_duplicate_looking_groups`;
- `evidence_health_records_with_canonical_gap_id`;
- `evidence_health_static_limitation_reasons`;
- `evidence_health_calibration_not_imported`;
- `evidence_health_top_evidence_quality_risks`;
- `evidence_health_timeout_limitations`;
- `evidence_health_incomplete_limitations`.

## Non-Goals

- No analyzer behavior changes.
- No evidence movement comparison.
- No gate or policy decision.
- No CI blocking behavior.
- No LSP, cockpit, PR comment, SARIF, badge, or release workflow changes.
- No mutation execution.
- No generated tests or source edits.

## Validation

The implementation is pinned by:

- output unit tests for metric counting and rendering;
- CLI parsing tests for the command surface;
- `cargo xtask evidence-health`;
- `cargo xtask check-output-contracts`;
- `cargo xtask check-static-language`;
- `cargo xtask check-traceability`;
- `cargo xtask check-capabilities`;
- `cargo xtask check-pr`.
