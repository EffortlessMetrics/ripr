# RIPR-SPEC-0080: Route-Quality Standalone Report

Status: proposed

Owner: swarm

Created: 2026-06-12

Linked proposal:

- None yet

Linked ADRs:

- None yet

Linked plan:

- None yet

Linked issues:

- #1132 — Surface repair route-quality as standalone JSON+MD reports
- #1123 — Receipt → outcome → route-quality loop

Linked PRs:

- None yet

Support-tier impact:

- None. This spec adds `route-quality.json` and `route-quality.md` as
  new advisory report artifacts written by `cargo xtask route-quality`.
  No existing contract is modified. No language, surface, or evidence
  class is promoted to a stronger support tier.
- Claim boundaries remain governed by the canonical ledger in
  [support tiers](../status/SUPPORT_TIERS.md); nothing here promotes a tier.

Policy impact:

- Register this spec in `policy/doc-artifacts.toml`.
- No new crates, binaries, dependencies, parsers, runtime executors,
  or LSP servers introduced by this spec.

## Problem

The `swarm-attempt-ledger` report already contains `repair_route_quality`
rows, but they are embedded in a large multi-purpose ledger artifact.
Consumers (agents, operators, CI dashboards) that need only the
route-quality grouping signal must parse the entire ledger to extract
those rows.

This spec extracts the route-quality rows into a dedicated, standalone
`route-quality.{json,md}` pair so that the report can be referenced,
diff-tracked, and consumed independently of the full attempt ledger.

## Behavior

`cargo xtask route-quality [--attempt-ledger <path>]` writes:

- `target/ripr/reports/route-quality.json` — versioned JSON, schema `0.1`.
- `target/ripr/reports/route-quality.md` — human-readable companion.

The default input is `target/ripr/reports/swarm-attempt-ledger.json`.
No new computation is performed: the function reuses
`ripr_swarm_attempt_ledger_entries_from_value`,
`ripr_swarm_attempt_ledger_latest_attempts`,
`ripr_swarm_attempt_ledger_repair_route_quality`, and
`ripr_swarm_repair_route_quality_json` from the existing attempt-ledger
pipeline.

### JSON schema

Top-level fields:

| Field | Type | Description |
| --- | --- | --- |
| `schema_version` | string | `"0.1"` — independent from attempt-ledger schema |
| `tool` | string | `"ripr"` |
| `report` | string | `"route-quality"` |
| `scope` | string | `"repo"` |
| `status` | string | `"advisory"` or `"blocked"` |
| `run_status` | string | inherited runtime status state |
| `runtime_status` | object | lane1 runtime status object |
| `generated_at` | string | `"unix_ms:<N>"` timestamp from caller |
| `metadata` | object | input paths, states, and limitations |
| `repair_route_quality_latest` | array | per-repair-kind rows from latest attempts |
| `repair_route_quality_historical` | array | per-repair-kind rows from all attempts |
| `language_repair_route_quality_latest` | array | per-language per-repair-kind rows (latest) |
| `language_repair_route_quality_historical` | array | per-language per-repair-kind rows (all) |
| `must_not_infer` | array of strings | honesty assertions |

Each row carries the fields produced by `ripr_swarm_repair_route_quality_json`:

| Field | Type | Producer |
| --- | --- | --- |
| `language` | string or null | `RiprSwarmAttemptLedgerEntry.language` |
| `repair_kind` | string | `RiprSwarmAttemptLedgerEntry.repair_kind` |
| `repair_kind_attempted` | integer | `outcome != "not_attempted"` in `ripr_swarm_attempt_ledger_repair_route_quality_grouped` (xtask ~27176) |
| `repair_kind_improved` | integer | `outcome == "evidence_improved"` (xtask ~27188) |
| `repair_kind_unchanged` | integer | `outcome == "evidence_unchanged"` (xtask ~27189) |
| `repair_kind_regressed` | integer | `outcome == "evidence_regressed"` (xtask ~27190) |
| `repair_kind_resolved` | integer | `outcome == "resolved"` (xtask ~27191) |
| `repair_kind_attempted_no_receipt` | integer | `outcome == "attempted_no_receipt"` (xtask ~27186) |
| `repair_kind_receipt_present` | integer | `outcome == "receipt_present"` (xtask ~27187) |
| `repair_kind_missing_verify_result` | integer | `ripr_swarm_attempt_missing_verify_result(attempt)` (xtask ~27302) |
| `repair_kind_expected_unchanged` | integer | `ripr_swarm_attempt_expected_unchanged_negative_capability(attempt)` (xtask ~27182) |
| `repair_kind_unknown` | integer | `outcome == "unknown"` or unrecognized (xtask ~27192) |
| `repair_kind_failure_count` | integer | `ripr_swarm_repair_route_quality_failure_count(row)` (xtask ~29534) |
| `repair_kind_dominant_failure_reason` | string or null | `ripr_swarm_repair_route_quality_dominant_failure_reason(row)` (xtask ~29542) |
| `repair_kind_success_rate` | number or null | `ripr_swarm_repair_route_quality_success_rate(row)` (xtask ~27356): `(improved + resolved + expected_unchanged) / attempted` when `attempted > 0`, else `null` |
| `sample_packet_ids` | array | up to 3 unique packet ids from non-not_attempted outcomes |
| `sample_attempt_ids` | array | up to 3 unique attempt ids |
| `sample_canonical_gap_ids` | array | up to 3 unique canonical gap ids |
| `sample_missing_receipt_reasons` | array | up to 3 unique missing receipt reasons |

### Fail-closed rules

- When `--attempt-ledger` input is missing: `status = "blocked"`,
  `runtime_status.state = "limited_incomplete_input"`, all four row
  arrays are empty `[]`. Never emit zero-filled rows.
- When the ledger is present but no attempts have non-`not_attempted`
  outcomes: all four row arrays are empty `[]`.
- `repair_kind_success_rate` is the JSON number computed above when
  `attempted > 0`, or JSON `null` when `attempted == 0`. The value
  `0.0` is never emitted as a fake rate.
- The following keys are **omitted entirely** from this report:
  - `top_orphan_receipt_sources` — no real producer yet
  - `stale_receipt_count` — no real producer yet
  - `top_limitation_routes` — belongs in the readiness report only

### Cross-validation invariant

The `repair_route_quality_latest` rows in `route-quality.json` must
equal the `repair_route_quality` rows in `swarm-attempt-ledger.json`
in all count fields. They differ only in the addition of
`repair_kind_success_rate` (computed from the same counts, not stored
separately in the ledger). The unit test
`route_quality_cross_validates_with_attempt_ledger_repair_route_quality`
enforces this invariant.

## Non-Goals

- This report is **not a gate** and does not affect CI pass/fail.
- This report does **not rank** repair kinds by quality.
- This report does **not detect orphaned receipts** or stale receipts.
- This report does **not replace** the `swarm-attempt-ledger` report.
- `repair_kind_success_rate` does not weight by receipt presence or
  attempt confidence; it is a simple outcome ratio.
- Does not introduce runtime mutation execution, test runner invocation,
  or any non-static analysis step.

## Required Evidence

- A `swarm-attempt-ledger.json` artifact containing attempt entries
  with `outcome` fields populated from real attempt outcomes.
- No new evidence beyond what the attempt ledger already records.

## Inputs

`cargo xtask route-quality`:

- `--attempt-ledger <path>` (optional; defaults to
  `target/ripr/reports/swarm-attempt-ledger.json`)

## Outputs

- `target/ripr/reports/route-quality.json` — versioned JSON
- `target/ripr/reports/route-quality.md` — human-readable companion

## Acceptance Examples

1. `cargo xtask route-quality` with no attempt ledger present writes
   `route-quality.json` with `status = "blocked"` and all four row
   arrays empty. No zero-filled rows. No orphan/stale keys.
2. `cargo xtask route-quality` with an attempt ledger containing two
   `add_assertion` attempts (one `evidence_improved`, one
   `evidence_unchanged`) produces a single row with `repair_kind_attempted: 2`,
   `repair_kind_improved: 1`, `repair_kind_success_rate: 0.5`.
3. A row with `repair_kind_attempted: 0` carries
   `repair_kind_success_rate: null` (JSON null, not `0.0`).
4. The JSON contains no `top_orphan_receipt_sources`,
   `stale_receipt_count`, or `top_limitation_routes` keys.

## Test Mapping

Unit tests — `xtask/src/main.rs`:

- `tests::route_quality_success_rate_is_null_when_attempted_is_zero`
- `tests::route_quality_success_rate_computed_when_attempted_nonzero`
- `tests::route_quality_empty_input_produces_empty_not_zero_filled_report`
- `tests::route_quality_cross_validates_with_attempt_ledger_repair_route_quality`

## Implementation Mapping

- `xtask/src/main.rs` — `RiprSwarmRouteQualityReport`,
  `ripr_swarm_route_quality_from_ledger_value`,
  `ripr_swarm_route_quality_report_json`,
  `ripr_swarm_route_quality_report_markdown`,
  `ripr_swarm_route_quality_report` (command entry point).
- `xtask/src/command.rs` — `XtaskCommand::RouteQuality` variant +
  `"route-quality"` parse arm + catalog entry.
- `xtask/src/dispatch.rs` — dispatch arm for `RouteQuality`.

All computation is delegated to existing functions:
`ripr_swarm_attempt_ledger_entries_from_value`,
`ripr_swarm_attempt_ledger_latest_attempts`,
`ripr_swarm_attempt_ledger_repair_route_quality`,
`ripr_swarm_repair_route_quality_json`.

## CI Proof

- `cargo xtask check-spec-format` passes on this file.
- `cargo xtask check-doc-artifacts` passes with the new registration.
- `cargo xtask check-doc-index` passes with the README entry.
- `cargo xtask check-traceability` passes.
- `cargo xtask check-static-language` passes.
- `cargo xtask check-no-panic-family` passes.
- `cargo xtask check-command-catalog` passes.
- `cargo xtask check-architecture` passes.
- `cargo test -p xtask tests::route_quality` — 4 pass.
- `RUSTFLAGS="-D warnings" cargo build -p xtask -p ripr` — exit 0.

## Metrics

- Route-quality rows per repair_kind in latest attempts.
- `repair_kind_success_rate` null-rate (fraction of rows where
  `attempted == 0`) as a data-completeness signal.
- Status promoted to `accepted` once all acceptance examples pass and
  real attempt data is flowing through the ledger.
