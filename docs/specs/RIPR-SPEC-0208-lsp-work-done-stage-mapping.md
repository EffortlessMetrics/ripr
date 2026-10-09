# RIPR-SPEC-0208: LSP work-done progress consumes shared analysis stages

Status: proposed

Owner: product / lsp

Created: 2026-10-02

Linked issues:

- #4811 — this slice: standard LSP work-done mapping and cross-surface parity
- #2608 — parent shared progress contract (final closeout matrix lands there;
  not closed here)
- #4810 / #4857 — CLI projection of the producer contract this slice consumes
- #4955 — repo-scope producer extension
- #5019 — pilot projection
- #1971 — existing standard `workDoneProgress` lifecycle authority
- #5003 — humanized limited end message (kept honest here)

Linked PRs:

Support-tier impact:

- None. Work-done progress is advisory visibility. It does not change
  findings, gates, support-tier labels, required CI, or release scope.
  [docs/status/SUPPORT_TIERS.md](../status/SUPPORT_TIERS.md)

Policy impact:

- None. No new env var, config key, workflow, custom LSP method, or
  exception ledger.

## Problem

The standard LSP `workDoneProgress` lifecycle (#1971) spoke only scheduler
vocabulary (`queued` / `analyzing workspace` / `publishing diagnostics`):
an editor could not tell which shared analysis stage the accepted request
was actually in, and the stage story could drift from the CLI stderr
projection of the same run. Two surfaces projecting one producer needed one
testable parity contract.

## Behavior

1. The accepted refresh's blocking analysis runs through the shared
   producer entry point (`check_with_progress` family) with an optional
   sink. A synchronous-to-asynchronous bridge (`StageReportBridge`) queues
   producer events; one drain task per attempt forwards them to the
   accepted generation's work-done token as bounded `$/progress` reports.
   The bridge is best-effort: queueing, draining, or transport failure
   cannot change the analysis result, the snapshot, or the lifecycle end.
   The drain is bounded by `STAGE_DRAIN_BUDGET`: a stalled client may delay
   result handling and the next refresh by at most twice that budget (the
   drain task plus the defensive final forward), never block them; missed
   stage reports are progress-only, and the outcome-derived end still
   reports the terminal disposition.
2. `loading_input`, `analyzing`, and `building_output` each map to exactly
   one client-appropriate bounded report
   (`loading input` / `analyzing workspace` / `building output`) on the
   same `ripr-analysis-{generation}` token. Existing tokens, the negotiated
   `window/workDoneProgress` capability, begin/transition wording, and the
   `publishing diagnostics` boundary report remain transport authority.
3. Terminal stages are never reported: the terminal disposition stays
   derived from the attempt outcome (`AnalysisProgressEnd`), so
   cancellation, deadline expiry, supersession, failure, and disclosed
   limited/deferred states end non-successfully (or disclosed-limited) and
   can never publish a fake completed stage. Consecutive duplicate stage
   events collapse to one report, bounding records for one token to
   begin + three stage reports + the publishing boundary + end.
4. Reports are message-only. No percentage is ever attached; unknown
   totals stay unknown and known finite totals do not invent client
   percentages either (the producer currently supplies no trustworthy
   denominator; the projections stay message-only rather than decorative).
   No absolute path, source text, environment value, or run-status tag
   enters a progress message.
5. Clients without `window/workDoneProgress` support see no progress
   traffic and keep unchanged lifecycle/diagnostic behavior; the tracker
   stays the fail-closed no-op it was.
6. Progress remains passive and outside semantic analysis identity: the
   snapshot bytes, `ripr/analysisStatus` payload, and CLI stdout are
   unchanged by emitting or dropping stage reports.

## Cross-surface parity

One normalized producer event trace projects through both real
projections (CLI `CliProgressSink` over a buffer; the LSP tracker over a
recording transport) and retains a parity DTO: producer event identity and
order, CLI semantic stage tokens, LSP semantic stage reports, the
known/unknown denominator posture, the terminal disposition, selected and
total progress records, and the disclosed limitations. The oracle requires:

- the CLI stage token sequence, the LSP report sequence, and the producer
  non-terminal stage sequence agree in identity and order;
- no percentage appears in either projection for unknown totals (or at
  all, under the current vocabulary);
- the terminal dispositions agree semantically: producer `completed` maps
  to success (or disclosed-limited with the limitation recorded, never
  upgraded), `cancelled` to the cancelled family, `failed` to failed;
- removing the shared stage mapping makes the oracle fail even though
  legacy begin/end progress traffic still emits text.

Human strings need not be byte-identical; stage identity, ordering,
denominator honesty, and terminal state may not diverge.

## Required Evidence

- Tracker unit: one bounded report per stage in producer order on the same
  token; consecutive duplicates collapse; terminal, queued, unstarted, and
  unknown generations are silent; capability-absent clients receive no
  stage traffic.
- Parity unit: success, disclosed-limited, cancelled, and failed traces
  project through both real projections with identical stage identity and
  order; mixed known/unknown denominators never render a percentage; the
  removal experiment fails the oracle while legacy begin/end still emits.
- Wire journey: one real refresh over the framed LSP transport against a
  healthy fixture workspace emits `create` → `begin` → ordered stage
  reports (`loading input`, `building output`) within the declared record
  ceiling → exactly one end, all on one token, with no percentages, no
  paths, and a success-family or disclosed-limited terminal message.
- Capability-absent wire journey: the same refresh emits no progress
  traffic at all.

## Non-Goals

- No custom `riprAgent` progress method, MCP progress work, or TUI
  dependency.
- No analyzer optimization, latency envelope, or memory claim.
- No stdout/JSON/SARIF schema change; no release, publication, or
  support-tier operation.
- No heartbeat invention on the LSP surface (the CLI heartbeat is a
  stderr-projection concern; the LSP reports real stage boundaries only).

## Acceptance Examples

1. A capable editor refresh shows `begin` (`analyzing workspace`), then
   `loading input`, `analyzing workspace`, `building output` reports as the
   run crosses each boundary, then `publishing diagnostics`, then exactly
   one end matching the snapshot run status.
2. A `seams_deferred` refresh ends `analysis completed with limited
   evidence` — the limitation is disclosed, never renamed complete, and the
   run-status tag stays machine-only on `ripr/analysisStatus`.
3. A refresh cancelled mid-analysis (superseded by a newer save, client
   cancellation, or the physical deadline) ends `cancelled`,
   `superseded`, or `analysis deadline exceeded`; no stage report ever
   implies completion.
4. An editor without `window/workDoneProgress` receives zero progress
   notifications and unchanged diagnostics.
5. Deleting the stage mapping (or one entry of it) fails the parity oracle
   and the mapping-coverage pin while begin/end traffic still emits.

## Test Mapping

- `crates/ripr/src/lsp/progress.rs::tests::report_stage_emits_one_bounded_report_per_stage_in_producer_order`
- `crates/ripr/src/lsp/progress.rs::tests::report_stage_suppresses_consecutive_duplicate_stages`
- `crates/ripr/src/lsp/progress.rs::tests::report_stage_is_silent_for_terminals_queued_unstarted_and_unknown`
- `crates/ripr/src/lsp/progress.rs::tests::report_stage_is_silent_without_client_capability`
- `crates/ripr/src/lsp/progress_stages.rs::tests::stage_mapping_covers_every_non_terminal_producer_stage`
- `crates/ripr/src/lsp/progress_stages.rs::tests::cli_and_lsp_project_one_success_trace_with_identical_stage_identity`
- `crates/ripr/src/lsp/progress_stages.rs::tests::disclosed_limitation_stays_limited_on_the_lsp_surface`
- `crates/ripr/src/lsp/progress_stages.rs::tests::cancelled_and_failed_traces_stay_non_successful_on_both_surfaces`
- `crates/ripr/src/lsp/progress_stages.rs::tests::known_and_unknown_denominators_never_become_percentages`
- `crates/ripr/src/lsp/progress_stages.rs::tests::removing_the_shared_mapping_breaks_parity_even_with_legacy_progress`
- `crates/ripr/src/lsp/progress_stages.rs::tests::bridge_forwards_events_without_blocking_the_producer`
- `crates/ripr/src/lsp/progress_stages.rs::tests::stalled_progress_sink_cannot_block_the_bounded_stage_drain`
- `crates/ripr/src/lsp/tests.rs::work_done_progress_stage_reports_through_real_refresh_journey`

## Implementation Mapping

- `crates/ripr/src/lsp/progress_stages.rs` — stage mapping, sync/async
  bridge, parity DTO oracle
- `crates/ripr/src/lsp/progress.rs` — `report_stage` on the token registry
  (capability, phase, duplicate, and terminal guards)
- `crates/ripr/src/lsp/backend.rs` — bridge + per-attempt drain task in
  `run_refresh_request` (both bounded by `STAGE_DRAIN_BUDGET`)
- `crates/ripr/src/lsp/diagnostics.rs` — sink-bearing diagnostics entry
- `crates/ripr/src/app/check.rs` — sink-bearing worktree entry point

## Metrics

- `lsp_progress_stage_reports` — producer stage reports forwarded per
  accepted generation
- `lsp_progress_records_per_token` — begin + reports + end per token,
  bounded by the declared ceiling
