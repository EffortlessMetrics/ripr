# RIPR-SPEC-0039: Static/Runtime Confidence Expansion

Status: proposed

## Problem

Lane 1 already imports static/runtime confidence labels for checked fixture
classes, and runtime-fixtures-v2 expanded side-effect observer, mock
expectation, snapshot, and opaque or dynamic dispatch samples. Evidence Quality
Leadership needs the next calibration expansion to stay class-scoped,
fixture-first, and honest about unknowns.

Without a specific static/runtime confidence expansion contract, runtime data
can be overinterpreted as a new static gap, ambiguous joins can look more
certain than they are, and calibration language can leak into static reports
that should remain conservative.

## Behavior

Runtime-fixtures-v3 defines checked imported runtime calibration samples for
the next Lane 1 evidence classes selected by the scorecard and benchmark
corpus. The calibration path reads imported runtime outcomes and maps them to
existing static seams when a supported join exists. It does not execute
mutation testing, generate tests, edit source files, post PR comments, change
gate behavior, or redefine RIPR scores.

The expansion must support these fixture classes when implemented:

- custom assertion helper outcomes;
- table-driven boundary outcomes;
- builder override outcomes;
- cross-file constant boundary outcomes;
- snapshot field-discriminator outcomes;
- mock expectation mismatch outcomes.

For each imported runtime sample, calibration emits advisory confidence context
using one of these labels:

- `supports_static_gap`;
- `contradicts_static_gap`;
- `supports_static_clean`;
- `contradicts_static_clean`;
- `ambiguous_runtime_join`;
- `runtime_only_signal`;
- `no_runtime_data`.

Runtime calibration may raise or lower confidence context for the joined class,
but it must not change the static seam class, static `evidence_record` grip
class, canonical gap identity, gate policy, or downstream projection authority.

## Required Evidence

Each runtime-fixtures-v3 case must include:

- stable fixture ID;
- evidence class;
- imported runtime sample input;
- expected join method, when a static seam can be joined;
- expected confidence label;
- expected claim;
- `must_not_claim` guards;
- expected calibration row or sample subset;
- static seam reference when the sample maps to an existing static seam;
- ambiguity reason when multiple static seams are possible;
- runtime-only reason when no static seam should be created;
- capability or maturity scope for the class.

Supported join outcomes are:

- direct `seam_id` join;
- unambiguous normalized file and line join;
- ambiguous normalized file and line join;
- unmatched runtime-only signal;
- no runtime data for a static seam.

The report must keep runtime sample fields separate from static seam fields.
Static reports and projection surfaces must keep RIPR vocabulary:

- `exposed`;
- `weakly_exposed`;
- `reachable_unrevealed`;
- `no_static_path`;
- `infection_unknown`;
- `propagation_unknown`;
- `static_unknown`.

Runtime outcome vocabulary remains limited to calibration/runtime report
families. Static reports must not switch to mutation-testing terms or imply
that runtime calibration validates the static analyzer globally.

## Inputs

Runtime-fixtures-v3 consumes checked imported runtime calibration samples with
enough structure to identify:

- fixture ID;
- runtime outcome class;
- file and line, when present;
- optional `seam_id`;
- optional span data;
- optional runtime operator or source of the signal;
- optional test command;
- optional duration;
- expected confidence label;
- expected static seam reference or ambiguity reason.

The input may be represented as a fixture corpus manifest, imported calibration
JSON, or checked report fixture. The implementation must document the concrete
input shape when the corpus lands and must keep it repo-local unless a later
spec promotes a public schema.

## Outputs

The implementation should update the existing calibration report family rather
than create a parallel confidence system. The expected outputs are:

- checked fixture files under a runtime-fixtures-v3 fixture path;
- `target/ripr/reports/mutation-calibration.json`;
- `target/ripr/reports/mutation-calibration.md`;
- scorecard calibration coverage fields that show the v3 class coverage;
- capability and traceability rows that name the fixture-backed or calibrated
  scope.

Calibration output must distinguish:

- matched static seams with runtime context;
- ambiguous runtime joins;
- runtime-only signals;
- static seams with no runtime data;
- class-scoped calibrated fixture coverage;
- unknown or unsupported classes.

## Non-Goals

- No cargo-mutants execution in CI.
- No gate or default-blocking change.
- No static seam-class changes from runtime data.
- No score redefinition.
- No PR or CI front-panel work.
- No LSP or editor polish.
- No generated tests.
- No automatic source edits.
- No provider or model calls.
- No broad analyzer-stable claim.
- No public runtime input schema unless a later spec promotes it.

## Acceptance Examples

Given an imported runtime sample with a matching `seam_id` for a static gap,
the calibration report emits a matched row with
`supports_static_gap` or `contradicts_static_gap` as appropriate. It must not
change the static seam grip class.

Given an imported runtime sample whose file and line match exactly one static
clean seam, the calibration report emits a matched row with
`supports_static_clean` or `contradicts_static_clean` as appropriate. It must
not turn the static-clean seam into a static gap.

Given an imported runtime sample whose file and line match multiple static
seams, the calibration report emits `ambiguous_runtime_join`, lists the
candidate seams, and does not assign the runtime outcome to one candidate.

Given an imported runtime-only signal with no supported static seam join, the
calibration report preserves the runtime signal with `runtime_only_signal`. It
must not create a new static gap or headline finding.

Given a static seam in a supported v3 class with no imported runtime sample,
the calibration report emits `no_runtime_data` and keeps the class
uncalibrated for that seam.

Given runtime-fixtures-v3 covers custom assertion helper outcomes, the
capability row may name only that checked class as calibrated. It must not
promote custom assertion helpers globally.

## Test Mapping

Planned tests:

- `mutation_calibration_runtime_fixtures_v3_custom_helper_outcomes`
  pins joined custom assertion helper outcome labels.
- `mutation_calibration_runtime_fixtures_v3_table_boundary_outcomes`
  pins table-driven boundary outcome labels.
- `mutation_calibration_runtime_fixtures_v3_builder_override_outcomes`
  pins builder override outcome labels.
- `mutation_calibration_runtime_fixtures_v3_cross_file_constant_outcomes`
  pins cross-file constant boundary outcome labels.
- `mutation_calibration_runtime_fixtures_v3_snapshot_field_outcomes`
  pins snapshot field-discriminator outcome labels.
- `mutation_calibration_runtime_fixtures_v3_mock_mismatch_outcomes`
  pins mock expectation mismatch outcome labels.
- `mutation_calibration_runtime_only_signal_stays_nonstatic`
  pins the runtime-only signal rule.
- `mutation_calibration_ambiguous_v3_join_stays_ambiguous`
  pins ambiguous join behavior.

Existing mutation calibration tests from RIPR-SPEC-0006 continue to pin the
base report contract, advisory posture, join methods, and confidence labels.

## Implementation Mapping

Planned implementation:

- runtime-fixtures-v3 fixture inputs under the existing calibration fixture
  hierarchy;
- `crates/ripr/src/output/mutation_calibration.rs` for label assignment and
  report rendering;
- `xtask/src/main.rs` for repo-local `cargo xtask mutation-calibration`
  fixture generation and validation;
- `fixtures/evidence-quality-benchmark/corpus.json` when benchmark cases need
  v3 calibration references;
- `docs/CAPABILITY_MATRIX.md` and `metrics/capabilities.toml` for
  class-scoped capability claims;
- `.ripr/traceability.toml` for spec, tests, fixtures, code, output, and metric
  linkage.

## Metrics

The implementation should feed these metrics:

- `runtime_fixtures_v3_cases`;
- `runtime_fixtures_v3_supported_classes`;
- `runtime_fixtures_v3_matched_static_seams`;
- `runtime_fixtures_v3_ambiguous_joins`;
- `runtime_fixtures_v3_runtime_only_signals`;
- `runtime_fixtures_v3_no_runtime_data`;
- `static_runtime_confidence_supports_static_gap`;
- `static_runtime_confidence_contradicts_static_gap`;
- `static_runtime_confidence_supports_static_clean`;
- `static_runtime_confidence_contradicts_static_clean`;
- `static_runtime_confidence_ambiguous_runtime_join`;
- `static_runtime_confidence_runtime_only_signal`;
- `static_runtime_confidence_no_runtime_data`.

## Validation

The implementation must be pinned by:

- focused calibration tests;
- `cargo xtask mutation-calibration`;
- `cargo xtask evidence-quality-scorecard`;
- `cargo xtask check-fixture-contracts`;
- `cargo xtask check-output-contracts`;
- `cargo xtask check-static-language`;
- `cargo xtask check-spec-format`;
- `cargo xtask check-traceability`;
- `cargo xtask check-capabilities`;
- `cargo xtask check-pr`.
