# RIPR-SPEC-0091: Pilot Artifact Size Bound

Status: proposed

Owner: product / swarm

Created: 2026-06-13

Linked proposal:

- None yet

Linked ADRs:

- None yet

Linked plan:

- None yet

Linked issues:

- #1170 — `ripr pilot` writes ~400 MB artifacts for a one-file change

Linked PRs:

- None yet

Support-tier impact:

- No tier change. This spec adds a size bound to the two large pilot artifacts
  (`repo-exposure.json`, `agent-seam-packets.json`). It does not change the
  analysis logic, pass/fail authority, or what the analyzer classifies.
- The bound is additive: when applied it inserts a `limitations[]` disclosure
  into both artifacts naming the cap, the controlling env var, and a repair
  route. No new fields are added to the `check.json` shape. No schema version
  bump. Claim boundaries and tier governance remain governed by the canonical
  ledger in [support tiers](../status/SUPPORT_TIERS.md).

Policy impact:

- Register this spec in `policy/doc-artifacts.toml`.
- No new crates, binaries, dependencies, parsers, runtime executors, or LSP
  servers introduced by this spec.

## Problem

`ripr pilot` scans every seam in the entire workspace and writes the full
classified seam list to two large JSON artifacts:

- `repo-exposure.json` — one entry per seam with reach/activate/propagate/
  observe/discriminate evidence and observed values.
- `agent-seam-packets.json` — one packet per seam with full evidence records.

For large Rust workspaces (tens of thousands of seams) these files balloon to
hundreds of megabytes per run. A one-file change that triggers a deep-mode
full-repo scan has been observed to write ~205 MB + ~189 MB ≈ 394 MB in a
single pilot invocation. At this size the artifacts are impractical to open,
diff, store in CI, or pass to downstream tools.

The existing `RIPR_REPO_EXPOSURE_SEAM_LIMIT` (default 10,000) bounds the
repo-exposure inventory pass but the pilot command discarded that `SeamLimitInfo`
result and then passed `None` to `render_agent_seam_packets_json`, leaving
`agent-seam-packets.json` entirely unbounded. The root fix requires:

1. A tighter default budget for both pilot artifacts.
2. Honest fail-closed disclosure in both artifacts when the budget is applied.

## Behavior

### Budget cap

A new constant `DEFAULT_PILOT_SEAM_BUDGET = 2_000` is introduced in
`analysis::seam_inventory`. After the workspace inventory completes, any
classified seam list longer than the budget is truncated to the budget length
before rendering. The truncation preserves the highest-ranked seams (the front
of the sorted slice as produced by the existing inventory).

### Environment variable control

`RIPR_PILOT_SEAM_BUDGET` controls the budget:

- Unset → use `DEFAULT_PILOT_SEAM_BUDGET` (2,000). `SeamLimitSource::Default`.
- Set to a positive integer N → use N. `SeamLimitSource::Configured`.
- Set to `0` → no budget (opt-out). Both artifacts receive all seams.

The budget is distinct from `RIPR_REPO_EXPOSURE_SEAM_LIMIT` (which bounds the
inventory pass). When both caps apply the pilot budget fires on the already-
capped slice and the tighter cap's `SeamLimitInfo` is disclosed.

### Disclosure

When the budget is applied, both `repo-exposure.json` and
`agent-seam-packets.json` include:

```json
"run_status": "seam_limit_applied",
"limitations": [
  {
    "category": "pilot_seam_budget_applied",
    "seams_analyzed": <N>,
    "seams_total": <M>,
    "limit_source": "default" | "configured",
    "control": "RIPR_PILOT_SEAM_BUDGET",
    "repair_route": "Set RIPR_PILOT_SEAM_BUDGET=<M> (or 0 to disable) to include all seams."
                  | "Set RIPR_PILOT_SEAM_BUDGET=0 to disable the budget and include all seams."
  }
]
```

When the budget is not applied, `"run_status": "complete"` is emitted with no
`limitations` array (mirroring the existing `repo-exposure.json` contract from
RIPR-SPEC-0074).

The `pilot-summary.json` and `pilot-summary.md` artifacts are NOT modified.
They already reflect the top-N seams from the pilot summary logic, which is
governed separately.

### Non-claims

- This spec does NOT change the exit code or gate authority.
- The budget is a presentation bound, not an analysis bound. The inventory
  still classifies all seams; only the artifact output is capped.
- This spec does NOT imply that the uncapped seams are unimportant. The repair
  route in the disclosure tells users how to recover the full output.

## Non-Goals

- Capping `pilot-summary.json` or `pilot-summary.md`.
- Capping `check.json` or `human.txt` (`ripr check` output).
- Changing `RIPR_REPO_EXPOSURE_SEAM_LIMIT` semantics.
- Streaming output or incremental artifact writes.
- Per-seam evidence field truncation (observed_values, missing_discriminators,
  evidence_record).

## Required Evidence

- `SeamLimitInfo { analyzed: usize, total: usize, source: SeamLimitSource }`
  already exists in `analysis::seam_inventory`.
- `render_repo_exposure_json(classified, limit_info)` already accepts
  `Option<&SeamLimitInfo>` and emits `run_status` / `limitations[]` per
  RIPR-SPEC-0074.
- `render_agent_seam_packets_json(classified, limit_info)` — NEW parameter
  mirrors the repo-exposure renderer pattern.

## Inputs

| Input | Required? | Purpose |
| --- | --- | --- |
| `RIPR_PILOT_SEAM_BUDGET` env var | no | Override the default 2,000 seam budget |
| `classified: &[ClassifiedSeam]` (post-inventory) | yes | Slice to cap before rendering |
| `SeamLimitInfo` from inventory pass | no | Carry forward if the inventory cap fired first |

## Outputs

| Output | Schema impact | Notes |
| --- | --- | --- |
| `repo-exposure.json` `run_status` | Additive | `complete` or `seam_limit_applied` |
| `repo-exposure.json` `limitations[]` | Additive | Present only when budget applied |
| `agent-seam-packets.json` `run_status` | NEW field | `complete` or `seam_limit_applied` |
| `agent-seam-packets.json` `limitations[]` | NEW field | Present only when budget applied |
| `pilot-summary.json` | None | Unchanged |
| `check.json` | None | Unchanged |

## Acceptance Examples

1. **Default budget**: workspace with 5,000 seams — both pilot artifacts
   contain 2,000 seams, `run_status: "seam_limit_applied"`, `limitations[0].seams_analyzed: 2000`,
   `limitations[0].seams_total: 5000`, `limit_source: "default"`.
2. **Opt-out**: `RIPR_PILOT_SEAM_BUDGET=0` — both artifacts contain all seams,
   `run_status: "complete"`, no `limitations` array.
3. **Custom budget**: `RIPR_PILOT_SEAM_BUDGET=500` with 800 seams — both
   artifacts contain 500 seams, `limit_source: "configured"`.
4. **Small workspace**: workspace with 100 seams, default budget 2,000 — both
   artifacts contain all 100 seams, `run_status: "complete"`.
5. **Budget via help**: `ripr pilot --help` output names `RIPR_PILOT_SEAM_BUDGET`
   with its default, opt-out value, and disclosure explanation.

## Test Mapping

- `crates/ripr/src/analysis/seam_inventory.rs::tests::pilot_seam_budget_default_constant_is_smaller_than_repo_exposure_cap`
- `crates/ripr/src/analysis/seam_inventory.rs::tests::pilot_seam_budget_env_zero_parses_as_unbounded`
- `crates/ripr/src/analysis/seam_inventory.rs::tests::apply_pilot_seam_budget_inner_truncates_when_above_limit`
- `crates/ripr/src/analysis/seam_inventory.rs::tests::apply_pilot_seam_budget_inner_returns_none_when_at_or_below_limit`
- `crates/ripr/src/output/agent_seam_packets.rs::tests::no_limit_info_emits_run_status_complete`
- `crates/ripr/src/output/agent_seam_packets.rs::tests::limit_info_emits_run_status_seam_limit_applied_and_disclosure`
- `crates/ripr/src/output/agent_seam_packets.rs::tests::limit_info_configured_source_emits_configured_repair_route`

## Implementation Mapping

- `crates/ripr/src/analysis/seam_inventory.rs` — adds `PILOT_SEAM_BUDGET_ENV`,
  `DEFAULT_PILOT_SEAM_BUDGET`, `apply_pilot_seam_budget`, `apply_pilot_seam_budget_inner`,
  and `pilot_seam_budget` helper.
- `crates/ripr/src/output/agent_seam_packets.rs` — adds `limit_info: Option<&SeamLimitInfo>`
  parameter to `render_agent_seam_packets_json`; emits `run_status` and `limitations[]`
  mirroring the repo-exposure renderer pattern.
- `crates/ripr/src/cli/commands/pilot.rs` — threads `SeamLimitInfo` from the
  inventory result through `apply_pilot_seam_budget` and passes `limit_info.as_ref()`
  to both renderers.
- `crates/ripr/src/cli/help/core.rs` — adds `RIPR_PILOT_SEAM_BUDGET` documentation
  to `PILOT_HELP`.

## Metrics

- Gate: all 7 new tests pass (4 in `seam_inventory.rs`, 3 in `agent_seam_packets.rs`).
- Promote to accepted when a large-workspace pilot run confirms artifact sizes are
  bounded by the 2,000-seam default.
