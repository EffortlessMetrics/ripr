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
complete-versus-limited zero-result parity. PR-B1 owns the additive SARIF
run-level disclosure and diff-scoped native/Shields badge downgrade. PR-B2
owns gate and PR-evidence-summary consumption: an
`analysis_complete: false` envelope is never a clean gate input and is
retained, with its outcome kind and recovery limitations, in the summary.
The B2 ownership includes `crates/ripr/src/output/gate/input.rs`,
`crates/ripr/src/output/gate/tests.rs`,
`crates/ripr/src/output/gap_decision_ledger.rs`,
`crates/ripr/src/app/pr_summary/`, and the compatibility forwarding surface
under `xtask/src/reports/pr_evidence_summary/`. Generated-CI, LSP, agent, and
review projections remain serial follow-up work and are unclaimed until they
consume this DTO.

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
- gate and PR-evidence-summary consumers fail closed or disclose the same
  incomplete typed outcome rather than deriving a clean result from zero
  candidates;
- gate-decision ledger JSON and Markdown preserve and disclose the same typed
  incomplete outcome when a check-output or rendered records artifact is
  supplied;
- the outcome input identity is stable for identical diff bytes and does not
  contain an absolute checkout path;
- SARIF and diff-scoped badge output disclose an incomplete zero-result run;
  repo-scoped badges retain `analysis_complete = null` and
  `analysis_outcome = null` because no diff denominator exists.
- Gate and PR-evidence-summary consumers are covered by PR-B2; generated-CI,
  LSP, agent, and review surfaces remain explicitly unclaimed until their own
  parity fixtures land.

Current PR-A proof is anchored by
`analysis::pipeline::tests::diff_pipeline_projects_parser_limitation_and_distinguishes_complete_zero`
and
`output::json::tests::typed_incomplete_outcome_matches_human_and_json_projection`.
PR-B1 adds
`output::sarif::tests::sarif_discloses_incomplete_zero_finding_outcome_at_run_level`
and
`output::badge::tests::incomplete_zero_finding_diff_is_not_a_green_badge`.

## Required Evidence

- parser and language-adapter limitations reach one typed pipeline outcome;
- complete zero-result and incomplete zero-result fixtures remain distinct;
- human and JSON projections disclose the same typed kind and recovery route;
- stable outcome identity excludes absolute checkout paths;
- SARIF run-level and diff-scoped badge projections disclose the typed outcome
  and fail closed for incomplete zero-result input.
- Gate and PR-evidence-summary projections are covered by PR-B2; generated-CI,
  LSP, agent, and review projections remain explicitly unclaimed.

## Test Mapping

The focused pipeline and JSON/human parity tests above cover limitation
propagation, complete-versus-incomplete zero-result behavior, and shared
 projection facts. PR-B1's focused SARIF and badge fixtures cover the
incomplete zero-result downgrade and run-level disclosure. PR-B2's gate and
PR-summary typed-envelope fixtures cover fail-closed consumption and summary
preservation across `crates/ripr/src/output/gate/`,
`crates/ripr/src/app/pr_summary/`, and the xtask compatibility route. The
gate-decision-ledger round-trip fixture covers preservation in both JSON and
Markdown when a rendered ledger is consumed as records. The
fixture goldens under `fixtures/` cover the additive JSON, human, and
changelog output projection across the existing language and edge-case corpus;
`cargo xtask goldens check` is the drift gate.

## Implementation Mapping

The producer contract is implemented in
`crates/ripr/src/analysis_outcome.rs`; parser-to-pipeline projection is in
`crates/ripr/src/analysis/` and `crates/ripr/src/analysis/pipeline.rs`.
Human rendering is in `crates/ripr/src/output/human.rs`, while JSON/status
projection is in `crates/ripr/src/output/json/` and the related output
builders. PR-B1 projects the DTO in `crates/ripr/src/output/sarif.rs` and
`crates/ripr/src/output/badge/`. PR-B2 consumes it in
`crates/ripr/src/output/gate/input.rs`,
`crates/ripr/src/output/gap_decision_ledger.rs`, and the shared
`crates/ripr/src/app/pr_summary/` owner; xtask delegates through that API.
`docs/OUTPUT_SCHEMA.md` records the wire shape.

## Metrics

The typed outcome exposes changed-file, changed-line, candidate-line, probe,
finding, limitation, and semantic-digest fields. `analysis_complete` is a
derived projection of the closed outcome kind; no independent completeness
metric is introduced by PR A or PR-B2.
