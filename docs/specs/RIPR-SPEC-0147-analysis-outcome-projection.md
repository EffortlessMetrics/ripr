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
under `xtask/src/reports/pr_evidence_summary/`. PR-C owns LSP consumption: the
committed editor snapshot, analysis status, workspace status, and
top-limitation command retain the typed outcome and derive
`limited_incomplete_input` without treating zero findings as complete.
Generated-CI and agent projections are covered by serial follow-up slices.
This slice owns the generated-CI handoff: `ripr-pr` preserves the producer's
canonical check JSON at `target/ripr/pr/check.json`, removes a stale copy
before each run, and pull-request workflows pass that artifact to
review-comments. The workflow does not infer completeness from the PR-evidence
summary packet. The first agent parity slice is the generated workflow and
`agent review-summary` handoff described below. The receipt parity slice now
consumes the same artifact through a shared validator, copies the typed outcome
and semantic digest, and emits an explicit missing or invalid state instead of
a clean-looking legacy receipt. Agent packets copy the typed producer envelope
when diff-scoped and explicitly mark repo-only and gap-ledger packets as not
applicable; their seam-budget `run_status` is not a diff-completeness
authority.
Review-comments is the bounded review consumer described below.

The review-comments projection is the next bounded consumer. When invoked with
`--check-output PATH`, it consumes the producer-generated check JSON and copies
the typed outcome into an `analysis_outcome` envelope. It does not rerun the
producer or infer completeness from review-comment counts. Incomplete and
unsupported outcomes set the review report status to `incomplete`, retain every
limitation and recovery route, and must not become a clean or zero-finding
claim. Without `--check-output`, the existing limited-diff scope remains a
legacy scope-only report with no fabricated outcome. Gap-ledger-only rendering
also remains scope-only because it has no diff-analysis denominator. An
explicit artifact must be the canonical producer envelope, including its
schema version, tool, mode, root, base, summary, findings, and typed outcome;
the declared completeness must agree with the closed outcome kind. The xtask
wrapper still uses the legacy empty packet only when no producer artifact was
requested.

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
- Gate and PR-evidence-summary consumers are covered by PR-B2.
- LSP analysis status and workspace/top-limitation status disclose the same
  typed outcome; incomplete input is never reported as `run_status = "full"`.
- Generated-CI preserves the canonical check artifact and forwards it to the
  review-comments projection; the generated agent workflow also publishes a
  named diff-scoped analysis-outcome artifact for `agent review-summary`.
  Agent receipts consume that artifact without rerunning analysis and preserve
  complete, incomplete, missing, malformed, stale, and identity-mismatched
  states. Agent packets copy the typed producer envelope when diff-scoped and
  explicitly mark repo-only and gap-ledger packets as not applicable; their
  seam-budget `run_status` is not a diff-completeness authority.

Review-comments complete, partial, and unsupported producer-backed fixtures
preserve the typed kind, completeness, limitation, and recovery route in JSON
and Markdown. Missing or malformed producer output is an error rather than a
silent legacy fallback; a complete zero-finding outcome remains advisory and
an incomplete outcome is explicitly not clean. Generated-CI is covered by the
canonical check artifact and workflow forwarding described above. The agent
review-summary and receipt slices require the producer artifact, preserve the
typed envelope and semantic identity, and never fabricate a replacement
outcome. Agent packet fixtures cover complete zero, incomplete/unsupported,
and repo-only not-applicable projections without conflating `run_status` with
diff completeness.

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
- Gate and PR-evidence-summary projections are covered by PR-B2.
- LSP status, workspace status, top limitation, and diagnostic severity retain
  the typed outcome and suppress full-run repair authority for incomplete
  input.
- Generated-CI canonical-artifact publication and workflow forwarding are
  covered by this slice through `xtask/src/reports/pr_evidence.rs`,
  `crates/ripr/src/cli/commands/init.rs`, and the generated-workflow fixtures
  `commands::tests::init_generated_github_workflow_matches_smoke_fixture` and
  `commands::tests::init_generated_github_workflow_is_advisory`. Direct
  producer coverage includes
  `pr_evidence::tests::run_ripr_check_uses_fake_binary_success_output`,
  `pr_evidence::tests::run_ripr_check_reports_fake_binary_failure`,
  `pr_evidence::tests::run_ripr_check_reports_fake_binary_timeout`,
  `pr_evidence::tests::write_pr_evidence_writes_error_packet_when_check_fails`,
  `pr_evidence::tests::write_and_check_packet_in_git_repo`, and
  `pr_evidence::tests::stale_check_artifact_is_removed_before_revision_setup_failure`;
  generated agent workflow and review-summary parity are covered by their named
  artifact fixtures; agent packet fixtures cover the producer envelope and
  explicit repo-only non-applicability.

## Test Mapping

The focused pipeline and JSON/human parity tests above cover limitation
propagation, complete-versus-incomplete zero-result behavior, and shared
projection facts. PR-B1's focused SARIF and badge fixtures cover the
incomplete zero-result downgrade and run-level disclosure. PR-B2's gate and
PR-summary typed-envelope fixtures cover fail-closed consumption and summary
preservation across `crates/ripr/src/output/gate/`,
`crates/ripr/src/app/pr_summary/`, and the xtask compatibility route. The
gate-decision-ledger round-trip fixture covers preservation in both JSON and
Markdown when a rendered ledger is consumed as records. PR-C's
LSP fixtures cover complete-versus-unsupported zero, status/top-limitation
parity, and precedence over stale and partial states. The fixture goldens
under `fixtures/` cover the additive JSON, human, and changelog output
projection across the existing language and edge-case corpus; `cargo xtask
goldens check` is the drift gate.

Review-comments parity is covered by
`output::review_comments::tests::review_comments_projects_typed_outcome_without_strengthening_incomplete_input`
and
`output::review_comments::tests::review_comments_projects_complete_zero_outcome_as_advisory`.
The CLI `--check-output` loader and mutual-exclusion parser tests cover the
fail-closed artifact boundary; the review-comments schema and xtask forwarding
surface carry the same envelope without changing the legacy gap-ledger route.

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
PR-C projects the DTO through `crates/ripr/src/lsp/state.rs`,
`crates/ripr/src/lsp/diagnostics.rs`, and `crates/ripr/src/lsp/backend.rs`.
`docs/OUTPUT_SCHEMA.md` records the wire shape. Generated-CI preserves the
raw producer artifact through `xtask/src/reports/pr_evidence.rs`, forwards it
from the CI workflows, and emits the same handoff from the generated workflow
in `crates/ripr/src/cli/commands/init.rs`. The producer's success, failure,
timeout, packet-validation, and
stale-artifact setup tests are mapped directly in the traceability ledger.
The generated agent workflow records the same diff-scoped producer envelope at
`target/ripr/workflow/analysis-outcome.json`, and `agent_review_summary`
consumes it as typed data. Diff-backed agent packets receive the typed outcome
from `CheckOutput`; repo-only and gap-ledger packet routes disclose that no
diff denominator applies, without rerunning analysis.
Review-comments consumes the DTO in `crates/ripr/src/cli/commands.rs` and
projects it through `crates/ripr/src/output/review_comments.rs`; the wire
contract is defined in `schemas/ripr/review-comments.schema.json` and the
xtask wrapper forwards `--check-output`.

## Metrics

The typed outcome exposes changed-file, changed-line, candidate-line, probe,
finding, limitation, and semantic-digest fields. `analysis_complete` is a
derived projection of the closed outcome kind; no independent completeness
metric is introduced by PR A, PR-B2, or PR-C.
