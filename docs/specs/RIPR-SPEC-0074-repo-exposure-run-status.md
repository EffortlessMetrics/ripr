# RIPR-SPEC-0074: Repo Exposure Run Status

Status: proposed

Owner: product / swarm

Created: 2026-06-11

Linked proposal:

- None yet

Linked ADRs:

- None yet

Linked plan:

- None yet

Linked issues:

- #1031

Linked PRs:

- None yet

Support-tier impact:

- None. This spec adds `run_status` and `limitations[]` to the
  `repo-exposure-json` output as additive fields within schema
  version `0.3`. No language, surface, or evidence class is promoted
  to a stronger support tier.
- Claim boundaries for this surface are governed by the canonical
  ledger in the support-tiers doc; nothing here promotes a tier.

Policy impact:

- Register this spec in `policy/doc-artifacts.toml`.
- No new crates, binaries, dependencies, parsers, runtime executors,
  or LSP servers.

## Problem

When `RIPR_REPO_EXPOSURE_SEAM_LIMIT` truncates the seam inventory
before classification, the `repo-exposure-json` output currently
looks identical to a complete run. A downstream consumer — CI badge,
scorecard, planning dashboard — reads the report and counts findings
without any signal that the run analyzed a bounded subset of seams.
This is a silent completeness failure: a truncated run looks complete.

The fix is fail-closed honesty: the report must always declare whether
the run was complete or bounded.

## Behavior

`repo-exposure-json` carries `run_status` at all times:

- **Complete run** (`RIPR_REPO_EXPOSURE_SEAM_LIMIT` unset, or set but
  `total <= limit`): `"run_status": "complete"`. No `limitations` key.
- **Bounded run** (`RIPR_REPO_EXPOSURE_SEAM_LIMIT` truncated the
  inventory): `"run_status": "seam_limit_applied"` plus a
  `limitations[]` block.

The default behavior — env var unset — is unchanged and emits
`run_status: "complete"`.

### JSON shape (additive within schema version `0.3`)

Complete run:

```json
{
  "schema_version": "0.3",
  "scope": "repo",
  "run_status": "complete",
  "metrics": { ... }
}
```

Bounded run:

```json
{
  "schema_version": "0.3",
  "scope": "repo",
  "run_status": "seam_limit_applied",
  "limitations": [
    {
      "category": "repo_seam_limit_applied",
      "seams_analyzed": 1,
      "seams_total": 42,
      "control": "RIPR_REPO_EXPOSURE_SEAM_LIMIT",
      "repair_route": "Remove RIPR_REPO_EXPOSURE_SEAM_LIMIT or increase it to analyze all seams. For bounded analysis, use `ripr check --diff` to scope the run to changed files."
    }
  ],
  "metrics": { ... }
}
```

`run_status` is inserted after `scope` and before `metrics`.
`limitations[]` appears after `run_status` only when bounded.

### Static-language boundary

`run_status` and `category` values use static vocabulary only:
`complete`, `seam_limit_applied`, `repo_seam_limit_applied`. The
forbidden runtime-mutation words (`killed`, `survived`, `untested`,
`proven`, `adequate`) are not used.

### Internal representation

`SeamLimitInfo { analyzed: usize, total: usize }` is returned from
`apply_repo_exposure_seam_limit` (previously `void`) and threaded
through `inventory_classified_seams_from_state_with_config` and
`inventory_classified_seams_at_with_config` as
`Option<SeamLimitInfo>`. Cache-hit paths return `None` (cache is
only consulted when no limit is active).

All callers that do not route to `repo-exposure-json` ignore the
second tuple element via `let (classified, _) = ...`.

## Required Evidence

- Unit tests in `crates/ripr/src/output/repo_exposure.rs`:
  `json_carries_run_status_complete_when_no_limit_applied` and
  `json_carries_run_status_and_limitations_when_limit_applied`.
- Smoke test in `crates/ripr/tests/cli_smoke.rs`:
  `check_repo_exposure_json_run_status_seam_limit_applied_and_complete`
  — sets `RIPR_REPO_EXPOSURE_SEAM_LIMIT=1` against a two-seam
  workspace, asserts bounded output; runs without env var, asserts
  complete output.
- `cargo xtask check-output-contracts` green.
- `cargo xtask goldens check` — no golden drift (goldens loop does
  not exercise `repo-exposure-json` output).

## Non-Goals

- No schema version bump; `0.3` additive additions are explicitly
  allowed per `docs/OUTPUT_SCHEMA.md`.
- `run_status` is not added to `repo-exposure-summary-json`,
  `repo-sarif`, `agent-seam-packets-json`, or any other format.
- The md renderer (`render_repo_exposure_md`) does not carry
  `limit_info`; the advisory note is omitted in this slice.
- No mutation-testing integration.
- No new flags or configuration surface.

## Acceptance Examples

**Example 1** — bounded run:

```
RIPR_REPO_EXPOSURE_SEAM_LIMIT=1 ripr check --root . --format repo-exposure-json
```

Output contains `"run_status": "seam_limit_applied"` and
`"limitations": [{"category": "repo_seam_limit_applied", ...}]`.

**Example 2** — complete run:

```
ripr check --root . --format repo-exposure-json
```

Output contains `"run_status": "complete"` and no `"limitations"` key.

## Test Mapping

- `crates/ripr/src/output/repo_exposure.rs::tests::json_carries_run_status_complete_when_no_limit_applied`
- `crates/ripr/src/output/repo_exposure.rs::tests::json_carries_run_status_and_limitations_when_limit_applied`
- `crates/ripr/src/output/repo_exposure.rs::tests::json_carries_schema_version_scope_and_metrics`
- `crates/ripr/tests/cli_smoke.rs::check_repo_exposure_json_run_status_seam_limit_applied_and_complete`

## Implementation Mapping

- `crates/ripr/src/analysis/seam_inventory.rs` — `SeamLimitInfo` struct,
  `apply_repo_exposure_seam_limit` returns `Option<SeamLimitInfo>`,
  `inventory_classified_seams_from_state_with_config` and
  `inventory_classified_seams_at_with_config` return
  `Result<(Vec<ClassifiedSeam>, Option<SeamLimitInfo>), String>`.
- `crates/ripr/src/output/repo_exposure.rs` — `render_repo_exposure_json`
  and `write_repo_exposure_json` accept `limit_info: Option<&SeamLimitInfo>`.
- `crates/ripr/src/output/render.rs` — `RepoExposureJson` path
  destructures and passes `limit_info`; other paths ignore it.
- `crates/ripr/src/cli/commands.rs` — fast-path `RepoExposureJson`
  destructures and passes `limit_info`.
- `crates/ripr/src/cli/commands/pilot.rs` — ignores `limit_info`.
- `crates/ripr/src/lsp/diagnostics.rs` — ignores `limit_info`.
- `docs/OUTPUT_SCHEMA.md` — field contract updated.

## Metrics

- `run_status_complete_rate` — proportion of repo-exposure runs
  carrying `run_status=complete` (tracked informally via operator
  CI logs; no automated metric in this slice).
- `unit_test_pass_rate` for `output::repo_exposure` (existing CI gate).
