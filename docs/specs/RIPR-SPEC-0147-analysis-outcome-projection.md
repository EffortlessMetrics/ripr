# RIPR-SPEC-0147: Typed analysis-outcome projection

Status: proposed

Issue: #2829

Related work: #2827, #2828

## Problem

Every diff-scoped analysis result must preserve the producer's typed
completeness and limitation facts. Empty findings and zero probes are not
enough to establish that the input was fully analyzable.

## Behavior

The diff pipeline consumes `AnalysisLimitation` records from the parser and
language adapters without reconstructing them from warning text. It publishes
one `AnalysisOutcome` containing:

- a closed outcome kind (`complete_*`, `partial_with_limitations`,
  `unsupported_input`, `no_*`, or `analysis_failed`);
- changed, candidate, probe, and finding counts;
- typed limitations with producer stage, affected items, and recovery route;
- a portable input identity and the static-analysis claim boundary.

Malformed non-empty diff input is a typed `malformed_diff` limitation at the
diff-parse stage and produces `unsupported_input`; it is never represented as
complete zero scope. Disabled preview-language files and configured generated
source skips are typed limitations as well, while a producer-declared partial
fact packet remains advisory rather than complete.

Human output must name incomplete or unsupported analysis before any empty
finding message and must state that zero findings is not a clean result when a
limitation exists. JSON/status output must expose the same DTO and derive
`analysis_complete` from its closed kind; it must not maintain an independent
completeness flag.

## PR boundary

PR A of #2829 owns parser-to-pipeline, human, JSON, and status projection plus
complete-versus-limited zero-result parity. SARIF, badge, gate, generated-CI,
LSP, agent, and review projections are the serial PR-B follow-up and remain
unclaimed until they consume this DTO.

## Non-Goals

The outcome describes static analysis completeness only. It does not claim
runtime execution, test adequacy, mutation results, correctness, or merge
readiness. Unsupported combined/conflicted input remains unsupported; the
projection does not analyze those regions.

## Acceptance Examples

- combined hunk and conflict-marker parser limitations survive into the
  pipeline outcome;
- ordinary zero-result input has a complete non-limitation kind distinct from
  unsupported or partial input;
- human and JSON output carry the same limitation kind and recovery route;
- the outcome input identity is stable for identical diff bytes and does not
  contain an absolute checkout path;
- PR-B surfaces remain explicitly unclaimed until their own parity fixtures
  land.

Current PR-A proof is anchored by
`analysis::pipeline::tests::diff_pipeline_projects_parser_limitation_and_distinguishes_complete_zero`
and
`output::json::tests::typed_incomplete_outcome_matches_human_and_json_projection`.

## Required Evidence

- parser and language-adapter limitations reach one typed pipeline outcome;
- complete zero-result and incomplete zero-result fixtures remain distinct;
- human and JSON projections disclose the same typed kind and recovery route;
- stable outcome identity excludes absolute checkout paths;
- PR-B projections remain explicitly unclaimed.

## Test Mapping

The focused pipeline and JSON/human parity tests above cover limitation
propagation, complete-versus-incomplete zero-result behavior, and shared
projection facts. The fixture goldens under `fixtures/` cover the additive
JSON, human, and changelog output projection across the existing language and
edge-case corpus; `cargo xtask goldens check` is the drift gate.

## Implementation Mapping

The producer contract is implemented in
`crates/ripr/src/analysis_outcome.rs`; parser-to-pipeline projection is in
`crates/ripr/src/analysis/` and `crates/ripr/src/analysis/pipeline.rs`.
Human rendering is in `crates/ripr/src/output/human.rs`, while JSON/status
projection is in `crates/ripr/src/output/json/` and the related output
builders. `docs/OUTPUT_SCHEMA.md` records the wire shape.

## Metrics

The typed outcome exposes changed-file, changed-line, candidate-line, probe,
finding, limitation, and semantic-digest fields. `analysis_complete` is a
derived projection of the closed outcome kind; no independent completeness
metric is introduced by PR A.
