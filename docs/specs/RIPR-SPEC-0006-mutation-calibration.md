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
- summarize static/runtime agreement in advisory buckets;
- label imported static/runtime agreement as confidence context without changing
  static classifications;
- preserve samples of runtime gap signals without static gaps;
- preserve samples of static gap seams without runtime gap signals;
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
- confidence label, one of:
  - `supports_static_gap`
  - `contradicts_static_gap`
  - `supports_static_clean`
  - `contradicts_static_clean`
  - `no_runtime_data`

Ambiguous file/line matches should keep the runtime record and list all static
candidate seams without assigning the runtime outcome to any single seam. These
rows should carry `ambiguous_runtime_join` so consumers know not to raise
confidence from that runtime record.

Unmatched runtime mutants should preserve their location, mutation operator,
runtime outcome, duration, and test command when available.

Runtime gap signals that cannot be joined to a static seam should carry
`runtime_only_signal`; they are calibration context only and must not create a
static gap.

The agreement summary should count:

- static gap seams with a matched runtime gap signal;
- static gap seams without a matched runtime gap signal;
- runtime gap signals without a matching static gap;
- static-clean seams with runtime-clean labels;
- inconclusive runtime labels that should not be counted as agreement.

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
when ripr calibrate cargo-mutants runs,
then the report emits one matched row with join_method = seam_id.
```

### Runtime mutant matches by file and line

```text
Given a repo exposure seam at src/pricing.rs:42,
and imported cargo-mutants JSON has no seam_id but has file = src/pricing.rs
and line = 42,
when ripr calibrate cargo-mutants runs,
then the report emits one matched row with join_method = file_line.
```

### Unmatched runtime mutant remains visible

```text
Given imported runtime data for src/other.rs:99,
and no static seam matches that seam_id or file/line,
when ripr calibrate cargo-mutants runs,
then the report lists the runtime mutant under unmatched_mutants.
```

### Ambiguous file and line match stays unassigned

```text
Given two repo exposure seams at src/pricing.rs:42,
and imported cargo-mutants JSON has no seam_id but has file = src/pricing.rs
and line = 42,
when ripr calibrate cargo-mutants runs,
then the report lists the runtime mutant under ambiguous_file_line_matches
and does not pick the first seam as a definitive match.
```

### Agreement summary stays advisory

```text
Given matched runtime data with static gaps, static-clean seams, runtime gap
signals, runtime-clean labels, and runtime-inconclusive labels,
when ripr calibrate cargo-mutants runs,
then the report emits agreement counts, precision notes, static-only finding
samples, and missed-runtime-signal samples without changing static seam classes.
```

### Confidence labels stay advisory

```text
Given static gap seams, static-clean seams, runtime gap labels, runtime-clean
labels, ambiguous file/line joins, unmatched runtime gap signals, and seams with
no usable runtime signal,
when ripr calibrate cargo-mutants runs,
then matched/sample rows include static/runtime confidence labels and those
labels do not change static seam classes or gate behavior.
```

## Test Mapping

Current tests:

- `crates/ripr/src/output/mutation_calibration.rs::tests::mutation_calibration_summarizes_static_runtime_agreement`
- `crates/ripr/src/output/mutation_calibration.rs::tests::mutation_calibration_joins_by_seam_id_then_file_line_and_keeps_ambiguous`
- `crates/ripr/src/output/mutation_calibration.rs::tests::mutation_calibration_parses_repo_exposure_and_cargo_mutants_json`
- `crates/ripr/src/output/mutation_calibration.rs::tests::mutation_calibration_merges_mutants_and_outcomes_by_id`
- `crates/ripr/src/output/mutation_calibration.rs::tests::mutation_calibration_reports_are_advisory_and_structured`
- `crates/ripr/src/cli/commands.rs::tests::calibrate_parses_required_inputs_format_and_out`
- `crates/ripr/src/cli/commands.rs::tests::calibrate_command_writes_json_file`
- `crates/ripr/tests/cli_smoke.rs::calibrate_cargo_mutants_prints_markdown_by_default`
- `crates/ripr/tests/cli_smoke.rs::calibrate_cargo_mutants_writes_json_when_requested`
- `crates/ripr/tests/cli_smoke.rs::calibration_runtime_fixture_matches_checked_reports`
- `xtask/src/main.rs::mutation_calibration_args_parse_root_and_input_paths`
- `xtask/src/main.rs::mutation_calibration_imports_static_seams_and_runtime_outcomes`
- `xtask/src/main.rs::mutation_calibration_merges_mutants_and_outcomes_by_mutant_id`
- `xtask/src/main.rs::mutation_calibration_imports_span_based_mutant_locations`
- `xtask/src/main.rs::mutation_calibration_directory_input_combines_outcomes_and_mutants`
- `xtask/src/main.rs::mutation_calibration_joins_by_seam_id_then_file_line`
- `xtask/src/main.rs::mutation_calibration_summarizes_static_runtime_agreement`
- `xtask/src/main.rs::mutation_calibration_reports_ambiguous_file_line_without_selecting_first`
- `xtask/src/main.rs::mutation_calibration_uses_same_static_without_runtime_sample_limit_for_json_and_markdown`
- `xtask/src/main.rs::mutation_calibration_reports_are_advisory_and_structured`

Checked fixture-backed samples:

- `fixtures/boundary_gap/calibration/runtime-fixtures-v1/` covers the main
  static/runtime agreement buckets, ambiguous file/line joins, unmatched runtime
  mutants, static seams without runtime data, and both `seam_id` and
  unambiguous `file_line` joins.
- `fixtures/boundary_gap/calibration/runtime-fixtures-v2/` covers checked
  observer-class runtime imports for side-effect observers, mock expectations,
  snapshot oracles, and opaque dispatch. The sample maps runtime outcomes to
  existing static seams where possible, keeps ambiguous file/line opaque
  dispatch joins ambiguous, and keeps a runtime-only signal out of static gap
  creation.

Planned tests:

- end-to-end smoke around a real cargo-mutants output artifact when runtime cost
  is acceptable.

## Implementation Mapping

Current implementation:

- `crates/ripr/src/cli/commands.rs` implements
  `ripr calibrate cargo-mutants`.
- `crates/ripr/src/output/mutation_calibration.rs` parses repo exposure JSON
  and imported cargo-mutants JSON.
- `ripr calibrate cargo-mutants` accepts either a JSON file path or a
  cargo-mutants output directory containing `outcomes.json` or `mutants.json`.
- `ripr calibrate cargo-mutants` renders Markdown by default and supports
  `--format json` plus `--out`.
- `xtask/src/main.rs` keeps repo-local `cargo xtask mutation-calibration`
  automation for generated reports under `target/ripr/reports/`.

The public command is an installed-binary adoption surface. It remains
advisory and does not make calibration a public library API.

## Metrics

- `static_seams_total`
- `mutants_total`
- `matched_total`
- `ambiguous_file_line_total`
- `unmatched_mutants_total`
- `static_without_runtime_total`
- `static_gap_and_runtime_signal`
- `static_gap_without_runtime_signal`
- `runtime_signal_without_static_gap`
- `static_clean_and_runtime_clean`
- `runtime_inconclusive`
- static/runtime confidence labels
- runtime outcome counts
- join method counts
