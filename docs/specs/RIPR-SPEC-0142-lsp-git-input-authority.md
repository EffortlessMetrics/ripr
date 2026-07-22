# RIPR-SPEC-0142: LSP Git Input Authority — One Resolution per Refresh

Status: accepted

Owner: product / swarm

Created: 2026-07-22

Linked issues:

- [#2000](https://github.com/EffortlessMetrics/ripr-swarm/issues/2000) -
  resolve Git input authority once per refresh and reuse it across analysis
  and snapshot identity.
- [#2261](https://github.com/EffortlessMetrics/ripr-swarm/issues/2261) -
  amendment: the `loader_default` state carries the resolved default-base
  SHA via an `analysis::diff` export.
- [#1919](https://github.com/EffortlessMetrics/ripr-swarm/issues/1919) -
  repeated `rev-parse` of the same base ref per refresh (parent).
- [#1968](https://github.com/EffortlessMetrics/ripr-swarm/issues/1968) -
  saved-content refresh dedup/input identity (parent); the resolved record
  feeds this dedup.
- [#1960](https://github.com/EffortlessMetrics/ripr-swarm/issues/1960) -
  reliability convergence program (parent).

Support-tier impact:

- No tier change. This spec changes when and how often the LSP server
  resolves Git inputs for a refresh. It does not change any classification,
  finding set, ExposureClass, probe family, confidence score,
  `repair_packet_ready` authority, output schema version, or pass/fail
  behavior. The `ripr/analysisStatus` input-authority view gains one
  additive derived label, `git_input_resolution`; CLI output contracts are
  untouched.
- Claim boundaries remain governed by the canonical ledger in
  [support tiers](../status/SUPPORT_TIERS.md).

Policy impact:

- No new crates, binaries, dependencies, parsers, runtime executors, or LSP
  servers introduced by this spec. No new production process-spawn site:
  resolution runs through the existing allowlisted `analysis::diff` adapter
  (`resolve_base_commit`; the #2261 amendment adds
  `resolve_default_base_commit` in the same adapter). No Git fetch, pull, or
  network access is introduced or implied.

## Problem

Before this spec, every LSP refresh request — including requests the
scheduler would immediately deduplicate — resolved the requested base ref
with its own `git rev-parse` probe inside input-identity construction
(`input_identity.rs` calling `analysis::resolve_base_commit`). On Windows,
where a process spawn costs 50-100ms, a save storm paid one redundant probe
per request. There was no typed record of what was resolved: the identity,
the dedup decision, the snapshot, and the status projection each derived
their view independently, and a resolution failure had no single typed
representation consumers could agree on.

## Behavior

### Typed request-local Git input record

One accepted refresh request resolves its load-bearing Git inputs exactly
once and carries the resulting typed record (`ResolvedGitInputs` in
`crates/ripr/src/lsp/git_inputs.rs`):

```text
root:           effective workspace root the resolution ran against
requested_base: base ref from repository config or session options
                (None selects the diff loader's default-base authority)
resolved_base:  requested base resolved to an exact commit, or the loader's
                default base resolved to an exact commit when no base was
                requested (#2261 amendment; None when nothing resolves —
                never fabricated)
resolution:     resolved | loader_default | unresolved
resolution probe: git rev-parse --verify --quiet <base>^{commit}; the
                loader_default state probes through the loader's
                default-base candidate search (symbolic-ref + rev-parse)
resolver version: lsp-git-inputs-v1
```

Rules:

- The record is resolved once per accepted refresh request by the refresh
  scheduler and is carried on the `RefreshRequest`. The input identity, the
  dedup decision, the committed snapshot (via its input identity), and the
  status projection all consume this one record. No consumer re-runs an
  equivalent resolution.
- The record is request-local with a session cache bounded to the
  in-flight refresh episode. It is not a global mutable cache.
- Reuse: a non-explicit request that arrives while an episode is in flight
  (an active or pending request exists) and whose root, requested base, and
  saved workspace revision all match the episode record reuses that record
  instead of spawning a redundant probe. This is the per-save `rev-parse`
  saving from #1919.
- Invalidation: the episode record is dropped when the scheduler returns to
  idle, on `invalidate_input` (root transitions and configuration changes
  already route here), on stop/cancel, and whenever the requested base,
  root, or saved revision differs. An explicit refresh
  (`ripr.refreshDiagnostics`) always resolves against the live repository.
  Because every post-episode request resolves fresh, base-ref movement,
  rebases, and fetches are observed by the first post-episode request; a
  moved base changes the resolved commit, which changes the input identity,
  which defeats dedup against the completed refresh.
- HEAD movement without base-ref movement changes the analyzed diff, and
  the analysis always runs `git diff <base>...HEAD` against the live
  repository on any accepted refresh, so no stale result can publish; an
  explicit refresh bypasses dedup entirely and therefore always observes a
  moved HEAD.
- Fail-closed: a requested base that does not resolve records
  `resolution: unresolved` with no resolved commit. The analysis run
  reports the named base failure through the unchanged `load_diff` error
  surface; the scheduler neither retries nor synthesizes a conflicting
  error, and the identity records no fabricated SHA, so a later successful
  resolution invalidates dedup.
- `loader_default` is honest: when no base is requested, the diff loader's
  default-base candidate order (`analysis::diff`, RIPR-SPEC-0084) applies
  inside the analysis run. Amendment (#2261): the record now resolves that
  default base once per refresh through the `analysis::diff` export
  `resolve_default_base_commit` — the same candidate order and probe shape
  the loader itself uses — and carries the resolved commit, so default-base
  workspaces dedup on the same commit authority as the explicit-base path.
  A workspace with no resolvable default base fails closed with no resolved
  commit; the analysis run reports the named default-base failure through
  the unchanged `load_diff` error surface, and no SHA is fabricated.
- Dirty tracked-worktree state does not participate in the record: the LSP
  diff path analyzes committed history (`base...HEAD`), and the
  RIPR-SPEC-0112 uncommitted-changes disclosure remains a CLI-only surface
  with identical semantics. Uncommitted edits never alter the resolved
  base identity.
- The record identifies analysis inputs only. It does not require a PR to
  be rebased because the base branch advanced, and it feeds no merge
  policy.

### Shared consumption and visibility

- `LspAnalysisInputIdentity::from_refresh_inputs_with_git` consumes the
  record's resolved base verbatim; for a requested base the identity is
  byte-identical to the pre-change resolution path for the same inputs. The
  #2261 amendment intentionally changes the default-base identity by adding
  the resolved default-base commit — the parity obligation for
  default-base workspaces is that the record-consuming and compatibility
  constructors stay byte-identical to each other.
- The refresh-start phase-boundary log names the record
  (`git_input_resolution`, `requested_base`, `resolved_base`), and the
  workspace-status input authority projects the derived
  `git_input_resolution` label (`resolved | loader_default | unresolved`)
  from the identity's own fields — additive, bounded, no new identity
  state.
- Scheduler telemetry counts fresh Git input resolutions
  (`git_input_resolutions`), so the subprocess saving is measurable per
  session. Concurrent queued requests carry their own records and can never
  mix Git input state.

## Required Evidence

- `ResolvedGitInputs`, `GitInputResolution`, and the single resolution site
  in `crates/ripr/src/lsp/git_inputs.rs`.
- The combined default-base export `resolve_default_base_commit` in
  `crates/ripr/src/analysis/diff/load.rs` (#2261 amendment).
- The record-consuming identity constructor
  `LspAnalysisInputIdentity::from_refresh_inputs_with_git` and the derived
  `git_input_resolution` status label in
  `crates/ripr/src/lsp/input_identity.rs`.
- The episode cache, reuse/invalidation policy, per-request record, and
  `git_input_resolutions` telemetry in
  `crates/ripr/src/lsp/refresh_scheduler.rs`.
- The refresh-start record log in `crates/ripr/src/lsp/backend.rs`.
- The framed duplex end-to-end projection test and the
  `run_lsp_scope_git`-based fixtures in `crates/ripr/src/lsp/tests.rs`,
  with the fake client answering every server-originated request it
  receives.

## Non-Goals

- HEAD/merge-base commit identity fields beyond what refresh consumers use
  today; the analyzed diff already binds `base...HEAD` at analysis time.
- Git fetch, merge-compatibility assessment, diff-cache redesign, debounce
  implementation, or repository-watcher expansion.
- Any change to analysis results, CLI behavior, or the RIPR-SPEC-0112
  dirty-worktree disclosure semantics.
- A global or ambient Git cache, time-based reuse, or any hidden network
  access.

## Acceptance Examples

### One resolution per accepted refresh

```text
Given a workspace with baseRef configured,
when a refresh request is accepted,
then the scheduler resolves the requested base exactly once,
and the request, the input identity, and the committed snapshot all carry
  the same resolved commit,
and git_input_resolutions increments by exactly one.
```

### Equivalent in-episode request reuses the record

```text
Given an in-flight refresh episode for root R, base B, saved revision N,
when a non-explicit request for the same (R, B, N) arrives,
then it reuses the episode record with no new probe,
and it deduplicates against the active or completed matching refresh.
```

### Base ref moves

```text
Given a completed refresh that resolved base B to commit X,
when base B advances to commit Y,
then the first post-episode request resolves fresh, observes Y,
  does not deduplicate against the stale identity, and starts new work.
```

### Explicit refresh always resolves fresh

```text
Given any episode state,
when ripr.refreshDiagnostics runs,
then the Git inputs resolve against the live repository,
  bypassing both dedup and episode reuse.
```

### Invalid or missing base ref

```text
Given baseRef names a ref that does not resolve,
when a refresh is accepted,
then the record carries resolution: unresolved with no fabricated SHA,
and the analysis reports the named base failure through the unchanged
  load_diff error surface,
and no consumer retries or emits a conflicting error.
```

### Default-base workspace carries the resolved default-base SHA (#2261)

```text
Given no baseRef is configured,
when a refresh is accepted,
then the record carries resolution: loader_default and the commit the
  loader's default-base candidate order resolves, resolved exactly once,
and the input identity is byte-identical to the legacy resolution path,
and a moved default base changes the resolved commit, defeats dedup, and
  starts new work,
and a workspace with no resolvable default base carries no fabricated SHA.
```

### Dirty tracked worktree

```text
Given uncommitted edits to tracked files,
when the Git inputs resolve,
then the resolved record is identical to the clean-tree record,
and the CLI dirty-worktree disclosure semantics are unchanged.
```

## Test Mapping

- `crates/ripr/src/lsp/git_inputs.rs::tests::resolution_contract_names_the_probe_and_version`
  — resolution vocabulary, probe shape, and resolver version.
- `crates/ripr/src/lsp/git_inputs.rs::tests::requested_base_resolves_once_to_the_same_commit_the_analysis_layer_reports`
  — the record resolves to the same commit `analysis::resolve_base_commit`
  reports.
- `crates/ripr/src/lsp/git_inputs.rs::tests::missing_ref_fails_closed_as_unresolved`
  — fail-closed unresolved state.
- `crates/ripr/src/analysis/diff/load.rs::tests::resolve_default_base_commit_returns_the_ref_and_its_commit`
  — the #2261 combined export returns the loader's default base and its
  exact commit, and fails closed when nothing resolves.
- `crates/ripr/src/lsp/git_inputs.rs::tests::unrequested_base_resolves_the_loader_default_commit`
  — the loader-default record carries the default-base commit (#2261).
- `crates/ripr/src/lsp/git_inputs.rs::tests::unrequested_base_fails_closed_when_no_default_base_resolves`
  — no fabricated SHA when no default base resolves (#2261).
- `crates/ripr/src/lsp/git_inputs.rs::tests::governs_binds_record_to_root_and_requested_base`
  — reuse binding to root and requested base.
- `crates/ripr/src/lsp/git_inputs.rs::tests::dirty_tracked_worktree_does_not_change_the_resolved_inputs`
  — dirty-worktree invariance.
- `crates/ripr/src/lsp/input_identity.rs::tests::record_built_identity_matches_legacy_resolution_byte_for_byte`
  — byte-identical identity versus the pre-change resolution path.
- `crates/ripr/src/lsp/input_identity.rs::tests::default_base_identity_matches_legacy_resolution_byte_for_byte`
  — byte-identical default-base identity carrying the loader's resolved
  default-base commit (#2261).
- `crates/ripr/src/lsp/input_identity.rs::tests::status_payload_projects_loader_default_and_unresolved_labels`
  — derived status label vocabulary.
- `crates/ripr/src/lsp/refresh_scheduler.rs::tests::accepted_refresh_resolves_git_inputs_once_and_shares_one_record`
  — one resolution per accepted refresh, shared with the identity.
- `crates/ripr/src/lsp/refresh_scheduler.rs::tests::default_base_refresh_resolves_once_and_carries_the_loader_default_sha`
  — one resolution per default-base accepted refresh, carrying the
  default-base commit (#2261).
- `crates/ripr/src/lsp/refresh_scheduler.rs::tests::default_base_dedup_shares_one_resolution_and_a_moved_default_base_invalidates`
  — same-commit default-base refreshes share one resolution; a moved
  default base forces a fresh resolution and defeats dedup (#2261).
- `crates/ripr/src/lsp/refresh_scheduler.rs::tests::post_episode_request_resolves_fresh_and_observes_base_ref_movement`
  — base-ref movement invalidation.
- `crates/ripr/src/lsp/refresh_scheduler.rs::tests::explicit_refresh_always_resolves_against_the_live_repository`
  — explicit refresh bypasses reuse.
- `crates/ripr/src/lsp/refresh_scheduler.rs::tests::base_configuration_change_is_not_governed_by_the_episode_record`
  — base configuration change invalidation and per-request record isolation.
- `crates/ripr/src/lsp/refresh_scheduler.rs::tests::invalidate_input_drops_the_episode_git_record`
  — input invalidation drops the episode record.
- `crates/ripr/src/lsp/refresh_scheduler.rs::tests::unresolved_requested_base_is_typed_once_and_shared_with_identity`
  — typed unresolved state with no fabricated SHA and the unchanged
  analysis error surface.
- `crates/ripr/src/lsp/tests.rs::framed_lsp_refresh_resolves_git_inputs_once_and_projects_the_record`
  — framed duplex end-to-end projection.

## Implementation Mapping

- `crates/ripr/src/analysis/diff/load.rs` — `resolve_default_base_commit`,
  the combined default-base + commit export consumed by the LSP record
  (#2261 amendment).
- `crates/ripr/src/lsp/git_inputs.rs` — the typed record, resolution
  states, the single resolution site, and the reuse-binding check.
- `crates/ripr/src/lsp/input_identity.rs` —
  `from_refresh_inputs_with_git` and the derived `git_input_resolution`
  status label.
- `crates/ripr/src/lsp/refresh_scheduler.rs` — the episode cache,
  reuse/invalidation policy, the per-request record on `RefreshRequest`,
  and `git_input_resolutions` telemetry.
- `crates/ripr/src/lsp/backend.rs` — the refresh-start record log.

## Metrics

- Unit and framed tests listed above pass under `cargo test -p ripr lsp::`.
- Scheduler telemetry `git_input_resolutions` records fresh resolutions per
  session; deduplicated in-episode requests and episode reuses do not
  increment it, which is the subprocess saving from #1919.
- `cargo xtask goldens check` remains clean: the LSP Git input authority
  touches no CLI golden.
- `cargo xtask check-static-language` and
  `cargo xtask check-output-contracts` remain clean: conservative vocabulary
  and no CLI output-contract change.
