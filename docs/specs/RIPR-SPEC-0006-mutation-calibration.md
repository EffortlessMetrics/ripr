# RIPR-SPEC-0006: Mutation Calibration Reports

Status: proposed

## Problem

`ripr` gives fast static seam evidence. Real mutation execution can later confirm
or correct those static predictions, but that runtime evidence currently has no
standard place to land.

Without a calibration report, agents and maintainers cannot compare
`SeamGripClass` predictions with cargo-mutants outcomes in a repeatable way, and
runtime mutation vocabulary can leak into static reports where it would overclaim
what `ripr` has proven.

## Behavior

`ripr` should provide an advisory calibration report that joins static seam
evidence to imported cargo-mutants JSON/output.

The report should:

- read current repo seam exposure evidence;
- import runtime mutation records from a supplied JSON file or `mutants.out`
  directory;
- combine `mutants.out/outcomes.json` with `mutants.out/mutants.json` when both
  are available;
- import span-based cargo-mutants locations when generated mutant records carry
  a `span` object instead of a flat `line` field;
- join records by `seam_id` when present;
- fall back to normalized file + line matching when `seam_id` is absent;
- report file/line matches as ambiguous when multiple static seams share the
  same normalized file and line;
- report unmatched runtime mutants separately;
- keep static seam fields and runtime mutation fields separate;
- write `target/ripr/reports/mutation-calibration.json`;
- write `target/ripr/reports/mutation-calibration.md`;
- stay advisory and non-blocking by default.

Runtime mutation outcome words are allowed only in this calibration/runtime
report family. Static check, exposure, badge, context, and editor reports must
continue using the audit vocabulary.

## Required Evidence

Each matched calibration row should carry:

- `seam_id`
- `seam_kind`
- `seam_grip_class`
- oracle kind and strength
- observed values
- missing discriminators
- mutation operator
- runtime outcome
- duration, when provided by the runtime data
- test command, when provided by the runtime data
- join method (`seam_id` or `file_line`)

Ambiguous file/line matches should keep the runtime record and list all static
candidate seams without assigning the runtime outcome to any single seam.

Unmatched runtime mutants should preserve their location, mutation operator,
runtime outcome, duration, and test command when available.

## Non-Goals

This spec does not require:

- running cargo-mutants;
- blocking CI;
- changing static seam classifications;
- recalibrating classification thresholds automatically;
- SARIF output;
- global suite scoring;
- adding runtime mutation vocabulary to static reports.

## Acceptance Examples

### Runtime mutant matches by seam ID

```text
Given a repo exposure seam with seam_id = abc123,
and imported cargo-mutants JSON has a runtime record with seam_id = abc123,
when cargo xtask mutation-calibration runs,
then the report emits one matched row with join_method = seam_id.
```

### Runtime mutant matches by file and line

```text
Given a repo exposure seam at src/pricing.rs:42,
and imported cargo-mutants JSON has no seam_id but has file = src/pricing.rs
and line = 42,
when cargo xtask mutation-calibration runs,
then the report emits one matched row with join_method = file_line.
```

### Unmatched runtime mutant remains visible

```text
Given imported runtime data for src/other.rs:99,
and no static seam matches that seam_id or file/line,
when cargo xtask mutation-calibration runs,
then the report lists the runtime mutant under unmatched_mutants.
```

### Ambiguous file and line match stays unassigned

```text
Given two repo exposure seams at src/pricing.rs:42,
and imported cargo-mutants JSON has no seam_id but has file = src/pricing.rs
and line = 42,
when cargo xtask mutation-calibration runs,
then the report lists the runtime mutant under ambiguous_file_line_matches
and does not pick the first seam as a definitive match.
```

## Test Mapping

Current tests:

- `xtask/src/main.rs::mutation_calibration_args_parse_root_and_input_paths`
- `xtask/src/main.rs::mutation_calibration_imports_static_seams_and_runtime_outcomes`
- `xtask/src/main.rs::mutation_calibration_merges_mutants_and_outcomes_by_mutant_id`
- `xtask/src/main.rs::mutation_calibration_imports_span_based_mutant_locations`
- `xtask/src/main.rs::mutation_calibration_directory_input_combines_outcomes_and_mutants`
- `xtask/src/main.rs::mutation_calibration_joins_by_seam_id_then_file_line`
- `xtask/src/main.rs::mutation_calibration_reports_ambiguous_file_line_without_selecting_first`
- `xtask/src/main.rs::mutation_calibration_reports_are_advisory_and_structured`

Planned tests:

- fixture-backed calibration samples once a stable cargo-mutants output fixture is
  checked in;
- end-to-end smoke around a real cargo-mutants output artifact when runtime cost
  is acceptable.

## Implementation Mapping

Current implementation:

- `xtask/src/main.rs` implements `cargo xtask mutation-calibration`.
- `xtask/src/main.rs` parses repo exposure JSON and imported cargo-mutants JSON.
- `xtask/src/main.rs` accepts either a JSON file path or a `mutants.out`
  directory containing `outcomes.json` or `mutants.json`.
- `xtask/src/main.rs` renders Markdown and JSON reports under
  `target/ripr/reports/`.

The command intentionally lives in `xtask` first because it is a repo calibration
artifact, not a stable product output surface or public library API yet.

## Metrics

- `static_seams_total`
- `mutants_total`
- `matched_total`
- `ambiguous_file_line_total`
- `unmatched_mutants_total`
- `static_without_runtime_total`
- runtime outcome counts
- join method counts
