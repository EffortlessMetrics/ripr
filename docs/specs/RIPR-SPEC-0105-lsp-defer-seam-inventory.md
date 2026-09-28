# RIPR-SPEC-0105: LSP Seam-Inventory Deferral (Interactive-Path Performance)

Status: accepted

Owner: product / swarm

Created: 2026-06-14

Linked issues:

- #1245

Linked PRs:

- None yet

Support-tier impact:

- Performance + disclosure change for the LSP cockpit: the expensive full-repo
  seam inventory (336s cold / 31s warm on this repo) is deferred off the
  interactive `did_open`/`did_save` path. Diff-scoped findings (Pass 1, 33ms–11s)
  are always produced on open/save. The seam inventory runs only on the explicit
  `ripr.refreshDiagnostics` command.
- Deferred runs are disclosed via a new `run_status` value `"seams_deferred"`.
  The limited policy (severity downgrade WARNING→INFORMATION, gap-diagnostic
  suppression) applies to `seams_deferred` snapshots. Nothing partial is ever
  presented as authoritative.
- No classification change. No `schema_version` bump (additive enum value).
  No version bump.
  Claim boundaries and tier labels remain governed by the canonical ledger in
  [support tiers](../status/SUPPORT_TIERS.md).

Policy impact:

- One new field `seams_deferred: bool` on `AnalysisSnapshot` in
  `crates/ripr/src/lsp/state.rs`.
- `workspace_diagnostics_with_config` gains a `defer_seam_inventory: bool`
  parameter; the interactive path passes `true`, the explicit refresh path
  passes `false`.
- `snapshot_run_status` and `workspace_status_run_status` gain a
  `"seams_deferred"` branch.
- No new public API symbols. No schema_version bump.
- Register this spec in `policy/doc-artifacts.toml`, `.ripr/traceability.toml`,
  and `docs/specs/README.md`.

## Problem

On `did_open`/`did_save` the LSP runs TWO passes sequentially:

1. `check_workspace_with_config` (diff-scoped against `origin/main`/`baseRef`) —
   FAST: ~33ms–11s, complete findings. This is the analysis users care about.
2. `inventory_classified_seams_at_with_config` (full-repo seam inventory) — walks
   ALL production Rust files: **336s cold / 31s warm** on this repo. This is
   the seam evidence panel in the cockpit.

Because `spawn_blocking` is not cancellable, and the two passes run sequentially,
the first `publishDiagnostics` is not emitted until both passes complete. On cold
start this exceeds 120s — the cockpit is dead-on-arrival. The LSP client timeout
typically fires, the user sees "server not responding", and ripr is effectively
unusable as a live editor aid.

The existing `repo_exposure_seam_limit` / pilot budget bounds OUTPUT size after the
full walk completes; it does not reduce latency.

## Fix

**One production delta: defer the seam inventory off the interactive path.**

### Pass-1-only default on interactive open/save

`did_open` / `did_save` / `did_close` call `refresh_diagnostics(defer_seam_inventory: true)`.
The `workspace_diagnostics_with_config` function skips the
`inventory_classified_seams_at_with_config` block when `defer_seam_inventory` is
`true`. Diff-scoped findings are still produced — they are complete and fast.
The snapshot records `seams_deferred: true`.

### Honest disclosure

`snapshot_run_status` returns `"seams_deferred"` when `defer_seam_inventory` is
`true` and no other limitation (stale/cache_limited/limited) applies.
`workspace_status_run_status` also checks `snapshot.seams_deferred`.

The existing limited policy (`is_full_run = run_status == "full"`) applies:

- Finding WARNINGs downgrade to INFORMATION (not authoritative without full evidence).
- Gap-record diagnostics are suppressed (not emitted when not full).
- `collect_workspace_status` returns `run_status: "seams_deferred"` with the
  `refresh_command: "ripr.refreshDiagnostics"` affordance so the cockpit can
  show "run refresh for full seam evidence."

### Explicit refresh on demand

The `ripr.refreshDiagnostics` (`REFRESH_COMMAND`) handler calls
`refresh_diagnostics(defer_seam_inventory: false)`. The full seam inventory runs
and the snapshot transitions to `full` (or `limited`/`stale`/`cache_limited` per
existing rules) with seam diagnostics present.

Deferral changes WHEN the seam inventory runs, not WHETHER it runs.

## Honesty Invariants

- A deferred run is NEVER presented as `full`. It is labeled `seams_deferred`.
- Absence of seam analysis is a DISCLOSED deferral with a refresh affordance, never
  a fabricated "0 seams".
- Diff-scoped findings on open are COMPLETE: `findings=0` means the diff genuinely
  has no exposure (an honest empty, not an aborted run).
- No classification / probe / grip-class logic change.
- No static-language terms: no `killed`, `survived`, `untested`, `proven`, `adequate`.
- `seams_deferred` is additive within the existing `full`/`limited`/`stale`/`cache_limited`
  family; no schema_version bump needed.

## Behavior

When `refresh_diagnostics(defer_seam_inventory: bool)` is called:

- If `defer_seam_inventory = true` (interactive path): `workspace_diagnostics_with_config`
  runs Pass 1 only (`check_workspace_with_config`). The `inventory_classified_seams_at_with_config`
  block is skipped. The resulting `AnalysisSnapshot` has:
  - `seams_deferred = true`
  - `classified_seams = []` (no seam walk ran)
  - Complete diff-scoped findings (`findings.len()` may be 0 — honest empty, not aborted)
  - `run_status` = `"seams_deferred"` (from `snapshot_run_status` and `workspace_status_run_status`)
  - Severity downgrade policy applies: finding WARNINGs → INFORMATION
  - Gap-record diagnostics suppressed (not emitted when `!is_full_run`)

- If `defer_seam_inventory = false` (explicit refresh path): both passes run.
  The resulting snapshot has `seams_deferred = false`, `classified_seams` populated
  as before, and `run_status` = `"full"` (or `"limited"`/`"stale"`/`"cache_limited"` per
  existing rules).

`collect_workspace_status` surfaces `run_status: "seams_deferred"` and
`refresh_command: "ripr.refreshDiagnostics"` so the cockpit can present an actionable
"run refresh for full seam evidence" affordance.

## Non-Goals

- No incremental/cancellable analysis yet (deferred to a follow-up). The seam
  inventory walk is still all-or-nothing when explicitly requested.
- No cap on the inventory walk itself (the seam limit / pilot budget already bounds
  output size; latency reduction is solely via deferral).
- No classification change. No new exposure class.
- No version bump / crates.io publish.
- No change to LSP initialization or `seamDiagnostics` configuration option.

## Required Evidence

Tests in `crates/ripr/src/lsp/tests.rs` (behavioral, unit-level):

1. `spec_0105_deferred_snapshot_has_no_seams_and_run_status_seams_deferred` —
   A snapshot with `seams_deferred = true` has zero `classified_seams` and
   `run_status = "seams_deferred"`. Must NOT be `"full"`. The `refresh_command`
   affordance must be present.

2. `spec_0105_collect_workspace_status_reports_seams_deferred_not_full` —
   After a default open (deferred), `collect_workspace_status` returns
   `run_status: "seams_deferred"` (not "full"), `refresh_command` is set, and
   `diagnostics.findings` > 0 (diff findings are complete, seam_diagnostics = 0).

3. `spec_0105_non_deferred_snapshot_has_run_status_not_seams_deferred` —
   A snapshot with `seams_deferred = false` reports `run_status: "full"`
   (when no other limits apply). Must NOT be `"seams_deferred"`.

4. `spec_0105_seams_deferred_run_status_value_is_not_full` —
   Unit gate: `snapshot_run_status_for_test(&[], &[], true)` → `"seams_deferred"`.
   `snapshot_run_status_for_test(&[], &[], false)` → `"full"`.

## Controls

### Control 1: Deferred snapshot has no seams and run_status "seams_deferred"

A `WorkspaceDiagnostics` with `snapshot.seams_deferred = true` must:
- Have zero `classified_seams`.
- Surface `run_status: "seams_deferred"` from `collect_workspace_status`.
- NOT surface `run_status: "full"`.

Test: `spec_0105_deferred_snapshot_has_no_seams_and_run_status_seams_deferred`

### Control 2: collect_workspace_status reports seams_deferred with refresh affordance

After a default open (deferred), `collect_workspace_status` returns:
- `run_status: "seams_deferred"` (not "full").
- `refresh_command: "ripr.refreshDiagnostics"`.
- `diagnostics.findings` > 0 (diff findings are complete, not aborted).
- `diagnostics.seam_diagnostics == 0`.

Test: `spec_0105_collect_workspace_status_reports_seams_deferred_not_full`

### Control 3: Explicit refresh produces a non-deferred snapshot

A snapshot built with `seams_deferred = false` must report `run_status: "full"`
(when no other limits apply) — confirming the explicit refresh path transitions
away from `seams_deferred`.

Test: `spec_0105_non_deferred_snapshot_has_run_status_not_seams_deferred`

### Control 4: snapshot_run_status returns "seams_deferred" iff deferred

`snapshot_run_status(&[], &[], true)` → `"seams_deferred"`.
`snapshot_run_status(&[], &[], false)` → `"full"`.

Test: `spec_0105_seams_deferred_run_status_value_is_not_full`

## Acceptance

- All 4 controls pass in CI.
- `cargo fmt --check` clean under rustfmt 1.9.0 (toolchain 1.95.0).
- `RUSTFLAGS="-D warnings" cargo build -p ripr -p xtask` clean.
- `cargo clippy -p ripr --all-targets -- -D warnings` clean.
- `cargo xtask check-static-language` passes (no forbidden terms in new output).
- `cargo xtask check-output-contracts` passes.
- `cargo xtask check-traceability` passes.
- `cargo xtask goldens check` passes (no golden drift from this change).

## Test Mapping

| Test | Fixture |
|---|---|
| `spec_0105_deferred_snapshot_has_no_seams_and_run_status_seams_deferred` | Control 1: deferred → no seams, run_status=seams_deferred |
| `spec_0105_collect_workspace_status_reports_seams_deferred_not_full` | Control 2: workspace_status → seams_deferred + refresh affordance + findings present |
| `spec_0105_non_deferred_snapshot_has_run_status_not_seams_deferred` | Control 3: full refresh → run_status=full (not seams_deferred) |
| `spec_0105_seams_deferred_run_status_value_is_not_full` | Control 4: snapshot_run_status unit gate |

## Acceptance Examples

### Before (broken — interactive open blocks 336s then shows seam diagnostics)

```
did_open → workspace_diagnostics_with_config runs BOTH passes
  Pass 1: check_workspace_with_config → ~33ms–11s   (fast, complete findings)
  Pass 2: inventory_classified_seams_at_with_config → ~336s cold
publishDiagnostics emitted after 336s — cockpit times out
```

### After (correct — interactive open fast, seams on demand)

```
did_open → workspace_diagnostics_with_config(defer_seam_inventory=true)
  Pass 1 only: check_workspace_with_config → ~33ms–11s   (complete findings)
  Pass 2 SKIPPED (seams_deferred=true)
publishDiagnostics emitted in ~11s

collect_workspace_status → run_status: "seams_deferred", refresh_command: "ripr.refreshDiagnostics"

ripr.refreshDiagnostics → workspace_diagnostics_with_config(defer_seam_inventory=false)
  Pass 1: check_workspace_with_config
  Pass 2: inventory_classified_seams_at_with_config → seam diagnostics present
publishDiagnostics with seam evidence
collect_workspace_status → run_status: "full" (or "limited"/"stale" per existing rules)
```

## Implementation Mapping

| Behavior | Code location |
|---|---|
| `seams_deferred: bool` field on `AnalysisSnapshot` | `crates/ripr/src/lsp/state.rs` |
| `defer_seam_inventory: bool` parameter on `workspace_diagnostics_with_config` | `crates/ripr/src/lsp/diagnostics.rs` |
| `snapshot_run_status` → `"seams_deferred"` branch | `crates/ripr/src/lsp/diagnostics.rs` |
| `workspace_status_run_status` → `"seams_deferred"` branch | `crates/ripr/src/lsp/backend.rs` |
| Interactive path: `did_open`/`did_save`/`did_close` pass `defer_seam_inventory=true` | `crates/ripr/src/lsp/backend.rs` |
| Explicit refresh: `REFRESH_COMMAND` handler passes `defer_seam_inventory=false` | `crates/ripr/src/lsp/backend.rs` |
| `snapshot_run_status_for_test` test-only export | `crates/ripr/src/lsp/diagnostics.rs` |
| Spec registration | `policy/doc-artifacts.toml`, `docs/specs/README.md` |
| Traceability | `.ripr/traceability.toml` |

## Metrics

- `lsp_seam_deferral_honesty`: a deferred snapshot (interactive open) never reports
  `run_status: "full"` and always reports `run_status: "seams_deferred"` with the
  `refresh_command` affordance. An explicit refresh snapshot reports `run_status: "full"`
  (or limited/stale/cache_limited per existing rules) — never `"seams_deferred"`.
