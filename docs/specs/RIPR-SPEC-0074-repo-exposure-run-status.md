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
  ledger in [support tiers](../status/SUPPORT_TIERS.md); nothing here
  promotes a tier.

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

### Slice A (Honest run-status field)

`repo-exposure-json` carries `run_status` at all times:

- **Complete run** (`RIPR_REPO_EXPOSURE_SEAM_LIMIT` unset, or set but
  `total <= limit`): `"run_status": "complete"`. No `limitations` key.
- **Bounded run** (`RIPR_REPO_EXPOSURE_SEAM_LIMIT` truncated the
  inventory): `"run_status": "seam_limit_applied"` plus a
  `limitations[]` block.

The default behavior — env var unset — was unchanged by Slice A and
emits `run_status: "complete"`.

### Slice B (Default seam cap + cache honesty)

Full-repo `repo-exposure-json` runs with the env var unset now apply
a built-in default cap of `DEFAULT_REPO_EXPOSURE_SEAM_LIMIT = 10_000`
seams. This bounds the pathological 41-minute full-repo run.

- **Default-capped run** (env var unset, `total > 10_000`): emits
  `"run_status": "seam_limit_applied"` with `"limit_source": "default"`.
  The repair route advises setting `RIPR_REPO_EXPOSURE_SEAM_LIMIT=0`
  to remove the cap.
- **Configured-capped run** (env var set, `total > limit`): emits
  `"run_status": "seam_limit_applied"` with `"limit_source": "configured"`.
  The repair route advises removing or raising the env var.
- **Complete run** (env var unset, `total <= 10_000`; or env var set,
  `total <= limit`; or env var set to `0` for unlimited): emits
  `"run_status": "complete"`. No `limitations` key.

#### Cache honesty contract

A truncated cached result must never later report `run_status: "complete"`.
This is enforced by:

1. **Seam-limit key in the cache key**: `RepoSeamCacheKey` includes a
   `seam_limit_key` field (`"limit_10000"`, `"limit_N"`, or
   `"unlimited"`). Capped and unbounded runs write to separate cache
   files and never share a hit.
2. **`seam_limit_info` stored in the cache envelope**: `CacheEnvelope`
   and `ShardedCacheManifest` carry `seam_limit_info:
   Option<CachedSeamLimitInfo>`. On a warm hit, the stored
   `limit_info` is returned alongside the seams so the renderer
   reproduces the original `run_status`.
3. **`#[serde(default)]` backward compat**: Old cache entries without
   `seam_limit_info` deserialize as `None` — correct because pre-Slice-B
   entries were always complete runs (no default cap existed).

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

Bounded run (default cap, `limit_source: "default"`):

```json
{
  "schema_version": "0.3",
  "scope": "repo",
  "run_status": "seam_limit_applied",
  "limitations": [
    {
      "category": "repo_seam_limit_applied",
      "seams_analyzed": 10000,
      "seams_total": 42000,
      "limit_source": "default",
      "control": "RIPR_REPO_EXPOSURE_SEAM_LIMIT",
      "repair_route": "Set RIPR_REPO_EXPOSURE_SEAM_LIMIT=0 to analyze all seams, or use `ripr check --diff` to scope the run."
    }
  ],
  "metrics": { ... }
}
```

Bounded run (env-configured cap, `limit_source: "configured"`):

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
      "limit_source": "configured",
      "control": "RIPR_REPO_EXPOSURE_SEAM_LIMIT",
      "repair_route": "Remove or raise RIPR_REPO_EXPOSURE_SEAM_LIMIT to analyze more seams, or use `ripr check --diff`."
    }
  ],
  "metrics": { ... }
}
```

`run_status` is inserted after `scope` and before `metrics`.
`limitations[]` appears after `run_status` only when bounded.
`limit_source` is added by Slice B as an additive field within `0.3`.

### Static-language boundary

`run_status` and `category` values use static vocabulary only:
`complete`, `seam_limit_applied`, `repo_seam_limit_applied`. The
forbidden runtime-mutation words (`killed`, `survived`, `untested`,
`proven`, `adequate`) are not used.

### Internal representation

**Slice A**: `SeamLimitInfo { analyzed: usize, total: usize }` is
returned from `apply_repo_exposure_seam_limit` (previously `void`)
and threaded through `inventory_classified_seams_from_state_with_config`
and `inventory_classified_seams_at_with_config` as
`Option<SeamLimitInfo>`.

**Slice B extensions**:
- `SeamLimitInfo` gains a `source: SeamLimitSource` field.
- `SeamLimitSource` is an enum with variants `Default` and `Configured`
  and an `as_str()` method returning `"default"` / `"configured"`.
- `DEFAULT_REPO_EXPOSURE_SEAM_LIMIT = 10_000` constant (always-on).
- `repo_exposure_seam_limit()` always returns `Some(...)` (default cap)
  unless the env var is set to `"0"` for opt-out unlimited mode.
- `RepoSeamCacheKey` gains `seam_limit_key: String` so capped/unbounded
  runs never share a cache file.
- `CacheEnvelope` and `ShardedCacheManifest` gain
  `seam_limit_info: Option<CachedSeamLimitInfo>` with `#[serde(default)]`
  for backward compat. Warm cache hits return the stored limit_info so
  the renderer reproduces the original `run_status` faithfully.

All callers that do not route to `repo-exposure-json` ignore the
second tuple element via `let (classified, _) = ...`.

## Required Evidence

**Slice A**:

- Unit tests in `crates/ripr/src/output/repo_exposure.rs`:
  `json_carries_run_status_complete_when_no_limit_applied` and
  `json_carries_run_status_and_limitations_when_limit_applied`.
- Smoke test in `crates/ripr/tests/cli_smoke.rs`:
  `check_repo_exposure_json_run_status_seam_limit_applied_and_complete`
  — sets `RIPR_REPO_EXPOSURE_SEAM_LIMIT=1` against a two-seam
  workspace, asserts bounded output; runs without env var, asserts
  complete output.

**Slice B**:

- Unit tests in `crates/ripr/src/analysis/seam_inventory.rs`:
  - `apply_repo_exposure_seam_limit_default_applies_when_above_cap`
  - `apply_repo_exposure_seam_limit_default_does_not_truncate_when_at_or_below_cap`
  - `apply_repo_exposure_seam_limit_configured_source_when_env_set`
  - `apply_repo_exposure_seam_limit_opt_out_none_returns_none_for_any_size`
  - `apply_repo_exposure_seam_limit_default_source_below_cap_returns_none`
  - `apply_repo_exposure_seam_limit_below_default_cap_is_complete`
- Unit tests in `crates/ripr/src/analysis/seam_cache.rs`:
  - `cache_envelope_with_limit_info_round_trips`
  - `cache_envelope_missing_limit_info_field_deserializes_as_none`
  - `complete_run_stores_none_limit_info_in_envelope`
  - `capped_cold_then_warm_cache_hit_returns_same_limit_info`
- Smoke tests in `crates/ripr/tests/cli_smoke.rs`:
  - `check_repo_exposure_json_limit_source_configured_when_env_set`
    — asserts `limit_source="configured"` when env var is set.
  - `check_repo_exposure_json_cache_roundtrip_preserves_seam_limit_applied`
    — runs twice with env=1; asserts both runs report
    `seam_limit_applied` (critical cache-honesty test).

**Both slices**:

- `cargo xtask check-output-contracts` green.
- `cargo xtask goldens check` — no golden drift (goldens loop does
  not exercise `repo-exposure-json` output).

## Non-Goals

- No schema version bump; `0.3` additive additions are explicitly
  allowed per `docs/OUTPUT_SCHEMA.md`.
- `run_status` is not added to `repo-exposure-summary-json`,
  `repo-sarif`, `agent-seam-packets-json`, or any other format.
- The md renderer (`render_repo_exposure_md`) does not carry
  `limit_info`; the advisory note is omitted.
- No mutation-testing integration.
- No new flags or configuration surface.
- Slice B does not add a CLI flag to override the default cap; the env
  var `RIPR_REPO_EXPOSURE_SEAM_LIMIT=0` is the opt-out path.
- No full mutation engine, coverage dashboard, proof system, or generic
  test generator.

## Acceptance Examples

**Example 1** — env-configured bounded run (Slice A + B):

```
RIPR_REPO_EXPOSURE_SEAM_LIMIT=1 ripr check --root . --format repo-exposure-json
```

Output contains `"run_status": "seam_limit_applied"`, `"limit_source": "configured"`,
and `"limitations": [{"category": "repo_seam_limit_applied", ...}]`.

**Example 2** — complete run (small repo, within default cap):

```
ripr check --root . --format repo-exposure-json
```

Output contains `"run_status": "complete"` and no `"limitations"` key.

**Example 3** — default-capped run (large repo, Slice B):

```
ripr check --root . --format repo-exposure-json
# (repo has > 10,000 seams; env var is unset)
```

Output contains `"run_status": "seam_limit_applied"`, `"limit_source": "default"`,
and repair_route instructing `RIPR_REPO_EXPOSURE_SEAM_LIMIT=0`.

**Example 4** — opt-out unlimited run (Slice B):

```
RIPR_REPO_EXPOSURE_SEAM_LIMIT=0 ripr check --root . --format repo-exposure-json
```

Output contains `"run_status": "complete"` — the default cap is disabled.

## Test Mapping

**Slice A**:

- `crates/ripr/src/output/repo_exposure.rs::tests::json_carries_run_status_complete_when_no_limit_applied`
- `crates/ripr/src/output/repo_exposure.rs::tests::json_carries_run_status_and_limitations_when_limit_applied`
- `crates/ripr/src/output/repo_exposure.rs::tests::json_carries_schema_version_scope_and_metrics`
- `crates/ripr/tests/cli_smoke.rs::check_repo_exposure_json_run_status_seam_limit_applied_and_complete`

**Slice B**:

- `crates/ripr/src/analysis/seam_inventory.rs::tests::apply_repo_exposure_seam_limit_default_applies_when_above_cap`
- `crates/ripr/src/analysis/seam_inventory.rs::tests::apply_repo_exposure_seam_limit_default_does_not_truncate_when_at_or_below_cap`
- `crates/ripr/src/analysis/seam_inventory.rs::tests::apply_repo_exposure_seam_limit_configured_source_when_env_set`
- `crates/ripr/src/analysis/seam_inventory.rs::tests::apply_repo_exposure_seam_limit_opt_out_none_returns_none_for_any_size`
- `crates/ripr/src/analysis/seam_inventory.rs::tests::apply_repo_exposure_seam_limit_default_source_below_cap_returns_none`
- `crates/ripr/src/analysis/seam_inventory.rs::tests::apply_repo_exposure_seam_limit_below_default_cap_is_complete`
- `crates/ripr/src/analysis/seam_cache.rs::tests::cache_envelope_with_limit_info_round_trips`
- `crates/ripr/src/analysis/seam_cache.rs::tests::cache_envelope_missing_limit_info_field_deserializes_as_none`
- `crates/ripr/src/analysis/seam_cache.rs::tests::complete_run_stores_none_limit_info_in_envelope`
- `crates/ripr/src/analysis/seam_cache.rs::tests::capped_cold_then_warm_cache_hit_returns_same_limit_info`
- `crates/ripr/tests/cli_smoke.rs::check_repo_exposure_json_limit_source_configured_when_env_set`
- `crates/ripr/tests/cli_smoke.rs::check_repo_exposure_json_cache_roundtrip_preserves_seam_limit_applied`

## Implementation Mapping

**Slice A**:

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

**Slice B**:

- `crates/ripr/src/analysis/seam_inventory.rs` — `SeamLimitSource` enum,
  `DEFAULT_REPO_EXPOSURE_SEAM_LIMIT` const, `SeamLimitInfo.source` field,
  `repo_exposure_seam_limit()` always returns `Some(...)` (default cap),
  `apply_repo_exposure_seam_limit_inner` inner fn,
  `apply_repo_exposure_seam_limit_for_test` cfg(test) helper.
- `crates/ripr/src/analysis/seam_cache.rs` — `CachedSeamLimitInfo` struct,
  `seam_limit_key` field on `RepoSeamCacheKey`, `seam_limit_info` field on
  `CacheEnvelope` and `ShardedCacheManifest` (both `#[serde(default)]`),
  updated load/store signatures to pass limit_info.
- `crates/ripr/src/analysis/mod.rs` — re-exports `DEFAULT_REPO_EXPOSURE_SEAM_LIMIT`
  and `SeamLimitSource`.
- `crates/ripr/src/output/repo_exposure.rs` — renders `limit_source` field,
  source-aware `repair_route`.
- `crates/ripr/src/cli/commands.rs` — doctor updated to name the default cap const.
- `docs/OUTPUT_SCHEMA.md` — `limit_source` field documented in limitations[].
- `docs/specs/RIPR-SPEC-0074-repo-exposure-run-status.md` — this file.

## Metrics

- `run_status_complete_rate` — proportion of repo-exposure runs
  carrying `run_status=complete` (tracked informally via operator
  CI logs; no automated metric in this slice).
- `unit_test_pass_rate` for `output::repo_exposure` (existing CI gate).
