# RIPR-SPEC-0180: LSP saved-edit sequence harness

Status: proposed

Owner: product / lsp

Created: 2026-09-29

Linked proposal:

Linked ADRs:

Linked plan:

Linked issues:

- #1578 (interactive latency, work, memory, and output envelopes; 2026-09-28
  execution packet)

Linked PRs:

Support-tier impact:

- None. This is rolling/conditional maintainer evidence. It does not change
  support-tier labels, required CI, release scope, or permission to execute
  project tests.
  [docs/status/SUPPORT_TIERS.md](../status/SUPPORT_TIERS.md)

Policy impact:

- Adds unpublished `cargo xtask lsp-performance-report` (`ci_enforced=false`).
- Existing rust tests absorb the deterministic sequence; no new always-on
  full-workspace CI job.

## Problem

RIPR-SPEC-0105 made the editor path usable by deferring the full seam
inventory, but the interactive loop still lacked a repeatable saved-edit
sequence that could tell a stale cached answer from a fast honest refresh.
Historical 2s/10s/30s figures were treated as if they were operating
envelopes. A fast elapsed time could hide a redundant full rescan or a
duplicate diagnostic publication, and a warm cache hit could satisfy a speed
story without matching complete-scope semantic output.

## Behavior

1. Finite saved-workspace sequence. One exact development/installed binary is
   driven through cold start, unchanged save, unchanged explicit refresh, one
   production expression edit, one related-test edit, one unrelated-test
   edit, a harmless rename, a config/base/features change, rapid superseding
   saves/cancellation, corrupt cache/store failure, and explicit full
   refresh.
2. Honesty over speed. A stale cached answer cannot satisfy a speed target.
   Complete-scope semantic parity is checked independently of elapsed time. A
   fast elapsed time cannot hide a redundant full rescan or duplicate
   diagnostic publication for an unchanged identity.
3. Envelope class. The historical 2-second warm-save p95, 10-second cold
   small-project, and 30-second warm-PR figures remain `proposal`. They are
   not `gating` or `achieved`. Missing memory samples stay `not_measured`.
4. Reuse landed owners. Scheduler telemetry (`analyses_started`,
   `requests_coalesced`, `completed_but_superseded`) comes from #1575.
   Published versus suppressed diagnostic bytes come from #1565/#1566.
   Cache-load vocabulary is `hit` / `miss` / `corrupt_ignored` from
   #3837/#3795. Interactive runs disclose `seams_deferred` (RIPR-SPEC-0105);
   explicit full refresh is labelled `full` and must not reuse
   `seams_deferred`.
5. Report. `cargo xtask lsp-performance-report` runs the sequence harness,
   overlays source SHA, keeps `binary_digest` as `not_measured` because the
   sequence is a `ripr --lib` harness rather than an executed
   `target/debug/ripr`, and writes
   `target/ripr/reports/lsp-performance.{json,md}` with schema
   `ripr-lsp-saved-edit-sequence-v1`. The first delivery records an explicit
   `no_change` optimization verdict; #3796 semantic reuse and #1702 trial
   execution remain out of scope.

## Required Evidence

- Cheat oracles reject: a stale+fast cache hit, a slow stale miss, a fast redundant full rescan,
  duplicate diagnostic publication on an unchanged identity, a related-test
  edit that does not invalidate evidence, an unrelated-test edit that
  manufactures an actionable finding or claims no work was needed, a
  mislabelled interactive full refresh, a proposed latency treated as a gate,
  a hidden workspace binary, superseded work presented as current, a corrupt
  cache reported as a hit, and a config-change warm hit.
- The live fixture driver exercises production `workspace_diagnostics_with_config`,
  `RefreshScheduler` telemetry, and `RepoSeamFactCache` corrupt-ignored
  fallback, then the same evaluator accepts the honest receipt.
- The report JSON names schema, identity, proposed envelopes as `proposal`,
  per-step work counts, and the claim boundary that latency figures remain
  proposals.

## Non-Goals

- No semantic-reuse / bounded-diff optimization (#3796).
- No trial execution (#1702).
- No reopening #3795 from its old problem statement.
- No new always-on full-workspace CI job or required latency gate.
- No support-tier or release-scope auto-expand.
- No per-keystroke analysis; this remains the saved-workspace loop.

## Acceptance Examples

- Given an unchanged saved identity, when the sequence records a full rescan
  or republishes diagnostic bytes, then evaluation fails even if elapsed time
  is 1 ms.
- Given a stale semantic digest presented with a cache hit and a fast elapsed
  time, when the sequence is evaluated, then it fails
  `stale_cache_satisfied_speed_target`.
- Given the historical 2s/10s/30s envelopes marked `gating` or `achieved`,
  when the sequence is evaluated, then it fails
  `proposed_latency_treated_as_gate`.
- Given an explicit full refresh, when `run_status` is `seams_deferred` or
  `semantic_scope` is `interactive`, then evaluation fails.

## Test Mapping

- `crates/ripr/src/lsp/saved_edit_sequence.rs`: cheat-oracle unit tests and
  `saved_edit_sequence_fixture_harness`.
- `xtask/src/reports/lsp_performance.rs`: identity overlay, proposed-envelope
  rejection, and markdown honesty.

## Implementation Mapping

- `crates/ripr/src/lsp/saved_edit_sequence.rs`: sequence types, evaluator,
  and live fixture driver.
- `xtask/src/reports/lsp_performance.rs`: report command.
- `xtask/src/command.rs`: catalog entry (`ci_enforced=false`).
- `docs/OUTPUT_SCHEMA.md`: `ripr-lsp-saved-edit-sequence-v1`.

## Metrics

- `lsp_saved_edit_sequence_honesty`: an unchanged identity cannot restart
  analysis, republish diagnostics, or full-rescan; a stale cache cannot
  satisfy a speed target; proposed 2s/10s/30s envelopes remain `proposal`.
