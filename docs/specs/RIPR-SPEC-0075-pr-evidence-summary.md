# RIPR-SPEC-0075: PR Evidence Summary

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

- None yet

Linked PRs:

- None yet

Support-tier impact:

- None. This spec adds `pr-evidence-summary.json` and
  `pr-evidence-summary.md` as new advisory report artifacts written by
  `cargo xtask ripr-pr-summary`. No existing contract is modified.
  No language, surface, or evidence class is promoted to a stronger
  support tier.
- Claim boundaries for all referenced surfaces are governed by the
  canonical ledger in [support tiers](../status/SUPPORT_TIERS.md);
  nothing here promotes a tier.

Policy impact:

- Register this spec in `policy/doc-artifacts.toml`.
- No new crates, binaries, dependencies, parsers, runtime executors,
  or LSP servers.

## Problem

`cargo xtask ripr-pr-summary` writes `target/ripr/reports/pr-summary.md`
today. That summary is human-readable but not machine-readable: a
reviewer or downstream agent must parse Markdown text to extract gap
counts, run status, or the top repair action. This blocks automated
triage, badge generation, and agent-loop routing.

The fix is an evidence summary JSON/MD pair that surfaces the same
information in a stable, versioned schema, pulling from the artifacts
that already exist in `target/ripr/reports/`.

## Behavior

`cargo xtask ripr-pr-summary` writes two new sibling files each run:

- `target/ripr/reports/pr-evidence-summary.json` — versioned JSON, schema `0.1`.
- `target/ripr/reports/pr-evidence-summary.md` — human-readable companion panel.

Both files are written after the legacy `pr-summary.md`. Failure to
load any input artifact is fail-closed: the field is set to
`"not_available"` or `null` (never a fake zero). No artifact load
failure causes the command to exit non-zero.

### `--baseline <path>` flag

An optional `--baseline <before.json>` flag allows the caller to
supply a previous `pr-evidence-summary.json` snapshot. When supplied,
the delta fields (`new_actionable`, `resolved`, `regressed`) are
computed as simple before/after counts. When omitted, those fields
are `null` with an explanatory `gap_delta_note`. This is the
**honest-baseline rule**: delta fields must never be faked as zeros
when no baseline is available.

### Input artifacts

| Artifact | Field sourced |
| --- | --- |
| `target/ripr/reports/diff-report.json` | `run_status`, `changed_surfaces`, `local_reproduction_commands` base/head |
| `target/ripr/reports/repo-exposure.json` | `run_status` (fallback), `limitations[]` |
| `target/ripr/reports/gap-decision-ledger.json` | `gaps.total_actionable`, `gaps.total_static_limitation`, `missing_receipts`, `receipt_status.receipts_present`, `receipt_status.missing_receipts` |
| `target/ripr/reports/start-here.json` | `top_repair` (when `selected.state == "top_gap"`), `top_repair_state`, `local_reproduction_commands` verify command |
| `--baseline <path>` | gap delta computation (optional) |

### Static-language boundary

All vocabulary values in the JSON output use static-analysis terms
only: `exposed`, `weakly_exposed`, `reachable_unrevealed`,
`no_static_path`, `infection_unknown`, `propagation_unknown`,
`static_unknown`. The forbidden runtime-mutation words (`killed`,
`survived`, `untested`, `proven`, `adequate`) are not used.

`run_status` values: `complete`, `seam_limit_applied`,
`diff_complete_full_repo_limited`, `unknown`.

## JSON Shape

Schema version `0.1`. Stable field order.

```jsonc
{
  "schema_version": "0.1",
  "kind": "pr_evidence_summary",
  "tool": "ripr",
  "run_status": "diff_complete_full_repo_limited",
  "changed_surfaces": 3,
  "gaps": {
    "total_actionable": 2,
    "total_static_limitation": 1,
    "new_actionable": null,
    "resolved": null,
    "regressed": null,
    "gap_delta_note": "no baseline snapshot provided; pass --baseline <before.json> for delta counts"
  },
  "limitations": [
    {
      "category": "repo_seam_limit_applied",
      "repair_route": "Set RIPR_REPO_EXPOSURE_SEAM_LIMIT=0 to analyze all seams."
    }
  ],
  "missing_receipts": 2,
  "receipt_status": {
    "receipts_present": 1,
    "missing_receipts": 2,
    "orphan_receipts": "not_available",
    "stale_receipts": "not_available",
    "gap_mismatch_receipts": "not_available",
    "verify_failed_receipts": "not_available"
  },
  "top_repair": {
    "canonical_gap_id": "gap:src/lib.rs:error_path:c1a03250",
    "language": "rust",
    "repair_kind": "AddTest",
    "target": "src/lib.rs",
    "verify_command": "cargo test -p ripr error_path",
    "receipt_command": "cargo xtask receipts",
    "receipt_state": "receipt_missing"
  },
  "top_limitation": {
    "category": "repo_seam_limit_applied",
    "repair_route": "Set RIPR_REPO_EXPOSURE_SEAM_LIMIT=0 to analyze all seams.",
    "why_not_actionable": "Seam inventory was capped; not all seams were analyzed in this run."
  },
  "local_reproduction_commands": [
    "ripr check --base origin/main",
    "ripr first-pr --root . --base origin/main --head HEAD",
    "cargo test -p ripr error_path"
  ]
}
```

When `top_repair` is absent (no actionable gap), `top_repair_state`
appears instead of `top_repair`:

```jsonc
{
  ...
  "top_repair": null,
  "top_repair_state": "no_actionable_gap",
  ...
}
```

When there are no limitations, `top_limitation` is omitted entirely.

### Field semantics

| Field | Type | Source | Notes |
| --- | --- | --- | --- |
| `schema_version` | string | static | Always `"0.1"`. |
| `kind` | string | static | Always `"pr_evidence_summary"`. |
| `tool` | string | static | Always `"ripr"`. |
| `run_status` | string | diff-report then repo-exposure | `"unknown"` when both missing. |
| `changed_surfaces` | u64 or `"not_available"` | diff-report `summary.changed_files` | `"not_available"` when artifact missing. |
| `gaps.total_actionable` | u64 or `"not_available"` | gap-ledger `summary.repairable_total` | |
| `gaps.total_static_limitation` | u64 or `"not_available"` | gap-ledger `summary.static_limitation_total` | |
| `gaps.new_actionable` | u64 or null | computed from baseline | null without `--baseline`. |
| `gaps.resolved` | u64 or null | computed from baseline | null without `--baseline`. |
| `gaps.regressed` | u64 or null | computed from baseline | null without `--baseline`; always 0 when baseline present (not yet tracked). |
| `gaps.gap_delta_note` | string or absent | advisory | Present when delta fields are null. |
| `limitations[]` | array | repo-exposure `limitations[]` | Empty when no limitations. |
| `missing_receipts` | u64 or `"not_available"` | gap-ledger `summary.repairable_total - receipt_improved_total` | Proxy; exact field preferred when present. |
| `receipt_status` | object | gap-decision-ledger summary | Six-count receipt status breakdown. See below. |
| `receipt_status.receipts_present` | u64 or `"not_available"` | `receipt_improved_total + receipt_unchanged_after_attempt_total` | Records carrying any receipt evidence. |
| `receipt_status.missing_receipts` | u64 or `"not_available"` | mirrors top-level `missing_receipts` | Convenience mirror. |
| `receipt_status.orphan_receipts` | `"not_available"` | not yet derivable | Unlock: add receipts/ dir sweep to ledger. |
| `receipt_status.stale_receipts` | `"not_available"` | not yet derivable | Unlock: wire the real staleness signal (`swarm_ingest.staleness_status`) into the gap-ledger build. Emitting 0 today would be a fake zero — no producer exists (#1130). |
| `receipt_status.gap_mismatch_receipts` | `"not_available"` | not yet derivable | Unlock: read each receipt's own `canonical_gap_id` and compare. |
| `receipt_status.verify_failed_receipts` | `"not_available"` | not yet derivable | Unlock: wire the real verify signal (`swarm_ingest.verify.passed/failed`) into the gap-ledger build. Emitting 0 today would be a fake zero — no producer exists (#1130). |
| `top_repair` | object or null | start-here `selected` when `state == "top_gap"` | null when no actionable gap. |
| `top_repair_state` | string or absent | start-here `selected.state` | Present only when `top_repair` is null. |
| `top_limitation` | object or absent | first entry in `limitations[]` | Omitted when limitations empty. |
| `local_reproduction_commands` | string[] | diff-report base/head + start-here verify_command | Always present; at least two commands. |

## Required Evidence

- Unit tests in `xtask/src/reports/pr_evidence_summary/json.rs`:
  - `missing_all_artifacts_yields_unknown_run_status`
  - `present_top_gap_populates_top_repair`
  - `gap_ledger_counts_are_surfaced`
  - `repo_exposure_limitations_are_aggregated`
  - `receipt_status_missing_all_artifacts_is_not_available`
  - `receipt_status_json_not_derivable_fields_are_not_available_not_zero`
  - `receipt_status_receipts_present_derived_from_ledger`
  - `claimed_repair_with_no_receipt_shows_in_missing_receipts`
  - `receipt_status_json_derived_fields_are_integers`
  - `receipt_status_deferred_fields_stay_not_available_even_with_ledger_summary` (#1130 honesty guard)
- Unit test in `crates/ripr/src/output/gap_decision_ledger.rs` (#1130 honesty guard):
  - `ledger_summary_does_not_emit_fabricated_receipt_state_counts`
- Integration tests in `xtask/src/reports/pr_evidence_summary.rs`:
  - `parse_accepts_baseline_path`
  - `parse_rejects_baseline_without_path`
  - `evidence_summary_pair_missing_all_shows_explicit_states`
  - `evidence_summary_pair_present_top_gap_is_copyable`

## Non-Goals

- No mutation-testing integration.
- `pr-evidence-summary.json` is advisory only; it does not gate CI.
- No schema version bump to existing artifacts.
- `top_repair` does not synthesize test code or call LLM providers.
- No full mutation engine, coverage dashboard, proof system, or generic
  test generator.
- `--baseline` computation is intentionally simple (before/after counts);
  no semantic diffing of individual gap ids.

## Acceptance Examples

**Example 1** — nominal run with no baseline:

```
cargo xtask ripr-pr-summary
```

Writes `target/ripr/reports/pr-evidence-summary.json` with
`"schema_version": "0.1"`, `"kind": "pr_evidence_summary"`,
`"tool": "ripr"`, `"run_status"` derived from available artifacts,
and delta fields as `null` with `gap_delta_note`.

**Example 2** — with baseline for delta counts:

```
cargo xtask ripr-pr-summary --baseline before.json
```

`gaps.new_actionable`, `gaps.resolved`, and `gaps.regressed` are
integer values computed from the before/after snapshots.

## Test Mapping

- `xtask/src/reports/pr_evidence_summary/json.rs::tests::missing_all_artifacts_yields_unknown_run_status`
- `xtask/src/reports/pr_evidence_summary/json.rs::tests::present_top_gap_populates_top_repair`
- `xtask/src/reports/pr_evidence_summary/json.rs::tests::gap_ledger_counts_are_surfaced`
- `xtask/src/reports/pr_evidence_summary/json.rs::tests::repo_exposure_limitations_are_aggregated`
- `xtask/src/reports/pr_evidence_summary::tests::parse_accepts_baseline_path`
- `xtask/src/reports/pr_evidence_summary::tests::parse_rejects_baseline_without_path`
- `xtask/src/reports/pr_evidence_summary::tests::evidence_summary_pair_missing_all_shows_explicit_states`
- `xtask/src/reports/pr_evidence_summary::tests::evidence_summary_pair_present_top_gap_is_copyable`

## Implementation Mapping

- `xtask/src/reports/pr_evidence_summary.rs` — top-level module: `SummaryOptions`, `parse_options`, `write_evidence_summary_pair`, `render_evidence_summary_md`.
- `xtask/src/reports/pr_evidence_summary/json.rs` — `build_pr_evidence_summary`, `render_pr_evidence_summary_json`.
- `xtask/src/reports/pr_evidence_summary/model.rs` — `PrEvidenceSummaryJson`, `GapCounts`, `LimitationEntry`, `TopRepair`, `TopLimitation`, `U64OrNotAvailable`, `NullableU64`.
- `xtask/src/reports/pr_evidence_summary/io.rs` — `load_json` (reused unchanged).
- `xtask/src/reports/pr_evidence_summary/util.rs` — `value_path` helper (added).
- `docs/OUTPUT_SCHEMA.md` — `## PR Evidence Summary` section.
- `docs/specs/RIPR-SPEC-0075-pr-evidence-summary.md` — this file.

## Metrics

- `pr_evidence_summary_json_schema_version_present` — advisory: `schema_version == "0.1"` in each produced file.
- `unit_test_pass_rate` for `xtask::reports::pr_evidence_summary` (existing CI gate).
