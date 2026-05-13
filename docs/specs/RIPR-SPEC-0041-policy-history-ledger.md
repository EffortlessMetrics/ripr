# RIPR-SPEC-0041: Policy History Ledger

Status: proposed

## Problem

Policy readiness and policy operations are point-in-time reports. They tell a
maintainer what posture is safe now, but they do not answer whether policy
health is improving or silently decaying over time.

Lane 2 needs an advisory history surface that can show trends without becoming
a dashboard, telemetry stream, or gate:

```text
Did the safe policy ceiling improve?
Did baseline debt shrink instead of absorb new debt?
Did waiver pressure rise?
Did suppressions age or lose metadata?
Did calibration evidence improve enough to change the ceiling?
Did preview-language evidence remain advisory?
```

Without a history packet, maintainers can repeatedly run readiness and
operations reports and still miss policy drift. Baseline-known debt can look
normal forever, PR-time waivers can become permanent practice, stale
suppressions can stay hidden in plain sight, and preview-language evidence can
appear more mature than it is.

## Behavior

The policy history report is a read-only advisory trend report over an explicit
current policy operations record and an optional append-only history input. It
must not execute a gate, collect telemetry, require a durable history file,
mutate config, baselines, suppressions, workflows, branch protection, generated
CI defaults, source files, or preview-language eligibility.

The report answers:

```text
current policy ceiling
recommended mode movement
baseline health movement
waiver pressure movement
suppression health movement
calibration health movement
preview boundary movement
new policy-eligible count movement
waiver count movement
stale suppression count movement
baseline still-present movement
baseline resolved movement
warnings
unknowns
input artifact status
```

The command consumes only explicit inputs. The current operations report is the
primary input. The history input, when supplied, is append-only JSONL maintained
outside this command. Missing history is allowed and yields a single-snapshot
report with unknown trend fields.

The command may read:

```text
policy operations report
optional policy history JSONL
optional commit and PR metadata supplied by flags
-> policy history report
```

It must not append to `.ripr/policy-history.jsonl`. It may include an example
append record so a maintainer or later explicit writer can review what would be
recorded.

## Required Evidence

The report contract is satisfied only when implementation can show:

- `target/ripr/reports/policy-history.json` with schema version `0.1`;
- `target/ripr/reports/policy-history.md`;
- optional input support for `.ripr/policy-history.jsonl`;
- explicit input artifact records for current operations and history inputs;
- a current snapshot derived from `policy-operations.json`;
- zero or more prior snapshots parsed from supplied history JSONL;
- trend fields for ceiling, baseline, waiver, suppression, calibration, and
  preview-boundary movement;
- warnings and unknowns for missing history, malformed history lines, missing
  current operations, missing commit or PR metadata, or unsupported snapshot
  shapes;
- no gate decision, telemetry collection, dashboard generation, config
  mutation, baseline mutation, suppression mutation, workflow mutation, default
  CI blocking, preview-language promotion, analyzer behavior change, provider
  calls, generated tests, or mutation execution.

## Inputs

The planned command is:

```text
ripr policy history \
  --current target/ripr/reports/policy-operations.json \
  --history .ripr/policy-history.jsonl \
  --commit HEAD \
  --pr-number 123 \
  --out target/ripr/reports/policy-history.json \
  --out-md target/ripr/reports/policy-history.md
```

Input artifacts:

| Input | Required? | Purpose |
| --- | --- | --- |
| Current policy operations JSON | required | Current ceiling, safe and blocked promotion modes, blockers, grouped actions, warnings, unknowns, and input artifact status. |
| Policy history JSONL | optional | Prior policy snapshots for trend comparison. Missing history produces a single-snapshot report. |
| Commit | optional | Human-supplied revision identity for the current snapshot. Missing commit is an unknown, not a failure. |
| PR number | optional | Human-supplied PR identity for the current snapshot. Missing PR number is an unknown, not a failure. |

The optional durable history input path is:

```text
.ripr/policy-history.jsonl
```

That file is append-only by convention but not required by this spec. The
history command reads it when supplied and never writes it.

## Outputs

The JSON report uses schema version `0.1`:

```json
{
  "schema_version": "0.1",
  "tool": "ripr",
  "kind": "policy_history",
  "root": ".",
  "generated_at": "unix_ms:1778277000000",
  "current": {
    "commit": "HEAD",
    "pr_number": "123",
    "generated_at": "unix_ms:1778277000000",
    "recommended_mode": "acknowledgeable",
    "current_policy_ceiling": "ready_for_acknowledgeable",
    "baseline_health": "warning",
    "waiver_health": "advisory",
    "suppression_health": "healthy",
    "calibration_health": "not_ready",
    "preview_boundary_state": "healthy",
    "new_policy_eligible_count": 1,
    "waiver_count": 2,
    "stale_suppression_count": 0,
    "baseline_still_present": 4,
    "baseline_resolved": 1
  },
  "history_summary": {
    "entries": 3,
    "oldest_generated_at": "unix_ms:1778190600000",
    "newest_generated_at": "unix_ms:1778277000000",
    "readiness_improved": true,
    "waiver_pressure_increased": false,
    "suppression_health_regressed": false,
    "baseline_shrank": true,
    "preview_remained_advisory": true,
    "calibration_changed_ceiling": false
  },
  "trend": {
    "ceiling": {
      "previous": "ready_for_visible_only",
      "current": "ready_for_acknowledgeable",
      "direction": "improved"
    },
    "waiver_count": {
      "previous": 3,
      "current": 2,
      "direction": "improved"
    },
    "stale_suppression_count": {
      "previous": 0,
      "current": 0,
      "direction": "unchanged"
    },
    "baseline_still_present": {
      "previous": 5,
      "current": 4,
      "direction": "improved"
    },
    "baseline_resolved": {
      "previous": 0,
      "current": 1,
      "direction": "improved"
    },
    "preview_boundary_state": {
      "previous": "healthy",
      "current": "healthy",
      "direction": "unchanged"
    },
    "calibration_health": {
      "previous": "not_ready",
      "current": "not_ready",
      "direction": "unchanged"
    }
  },
  "example_append_record": {
    "commit": "HEAD",
    "pr_number": "123",
    "generated_at": "unix_ms:1778277000000",
    "current_policy_ceiling": "ready_for_acknowledgeable",
    "recommended_mode": "acknowledgeable"
  },
  "warnings": [],
  "unknowns": [
    {
      "kind": "history_not_supplied",
      "message": "No policy history JSONL was supplied; trend is limited to the current snapshot.",
      "source_artifact": null
    }
  ],
  "input_artifacts": [
    {
      "kind": "policy_operations",
      "path": "target/ripr/reports/policy-operations.json",
      "status": "read"
    },
    {
      "kind": "policy_history_jsonl",
      "path": ".ripr/policy-history.jsonl",
      "status": "missing"
    }
  ],
  "limits_note": "Read-only advisory policy history report. It reads explicit history inputs and never appends, mutates policy, or changes gate authority."
}
```

Field contract:

- `schema_version` - currently `"0.1"`.
- `tool` - always `"ripr"`.
- `kind` - always `"policy_history"`.
- `current` - normalized current snapshot. It is derived from
  `policy-operations.json` plus optional commit and PR metadata.
- `current.commit` - optional human-supplied commit identity.
- `current.pr_number` - optional human-supplied PR identity.
- `current.recommended_mode` - derived from the highest safe promotion mode or
  current policy operations ceiling.
- `current.current_policy_ceiling` - copied from policy operations.
- `current.baseline_health`, `waiver_health`, `suppression_health`,
  `calibration_health`, and `preview_boundary_state` - normalized health
  states derived from policy operations actions, blockers, and input artifacts.
- `current.new_policy_eligible_count` - current count of new policy-eligible
  debt when available, otherwise zero with an unknown.
- `current.waiver_count` - current count of PR-time waiver signals when
  available, otherwise zero with an unknown.
- `current.stale_suppression_count` - current count of stale suppressions when
  available, otherwise zero with an unknown.
- `current.baseline_still_present` - current baseline-known debt still present
  when available, otherwise zero with an unknown.
- `current.baseline_resolved` - current resolved baseline-known debt when
  available, otherwise zero with an unknown.
- `history_summary.entries` - count of prior plus current snapshots included in
  the trend.
- `history_summary.readiness_improved` - true only when the current ceiling
  ranks higher than the previous comparable snapshot.
- `history_summary.waiver_pressure_increased` - true when waiver count rises.
- `history_summary.suppression_health_regressed` - true when stale or malformed
  suppression signals rise.
- `history_summary.baseline_shrank` - true when still-present baseline debt
  falls or resolved baseline debt rises without adopt-new behavior.
- `history_summary.preview_remained_advisory` - true only when preview evidence
  stayed non-gating across comparable snapshots.
- `history_summary.calibration_changed_ceiling` - true when calibration health
  improvement is the reason the ceiling changed.
- `trend.*.direction` - `improved`, `regressed`, `unchanged`, or `unknown`.
- `example_append_record` - the current snapshot in appendable JSONL shape. It
  is advisory output only and must not be written automatically.
- `warnings[]` - malformed supplied history lines, malformed current input, or
  unsupported historical shapes.
- `unknowns[]` - missing optional history, commit, PR number, or unavailable
  metric fields.
- `input_artifacts[]` - per-input status. Status values are `read`, `omitted`,
  `missing`, `malformed`, and `not_applicable`.
- `limits_note` - read-only/no-telemetry/no-mutation/no-gate boundary.

The Markdown sibling should fit in generated CI summaries and report packets:

```text
# RIPR Policy History

Current ceiling: ready_for_acknowledgeable
Recommended mode: acknowledgeable
History entries: 3

## Trend

- Readiness: improved
- Waiver pressure: unchanged
- Suppression health: unchanged
- Baseline debt: improved
- Preview boundary: unchanged, still advisory
- Calibration ceiling effect: no

## Current Snapshot

- commit: HEAD
- PR: 123
- new policy-eligible: 1
- waivers: 2
- stale suppressions: 0
- baseline still present: 4
- baseline resolved: 1

## Input Artifacts

- policy-operations: read
- policy-history-jsonl: missing

## Append Record

The command may show the JSONL record to append manually, but it must not write
history automatically.
```

## Non-Goals

- No gate decisions.
- No required history file.
- No automatic history append.
- No telemetry collection.
- No dashboard.
- No CI pass/fail authority.
- No config mutation.
- No baseline mutation or adoption.
- No suppression creation, deletion, or auto-expiry.
- No workflow or branch-protection changes.
- No generated workflow default changes.
- No default CI blocking.
- No analyzer behavior changes.
- No evidence identity rewrites.
- No LSP or editor behavior changes.
- No generated tests.
- No provider calls.
- No mutation execution.
- No preview-language gate promotion.
- No runtime-confirmation or coverage-sufficiency claims from static evidence.

## Acceptance Examples

Single snapshot:

- Current policy operations is supplied.
- No history JSONL is supplied.
- The report emits current snapshot fields.
- Trend directions are `unknown`.
- `history_not_supplied` appears in `unknowns`.
- No history file is written.

Readiness improved:

- Previous comparable snapshot has
  `current_policy_ceiling = "ready_for_visible_only"`.
- Current snapshot has
  `current_policy_ceiling = "ready_for_acknowledgeable"`.
- `history_summary.readiness_improved = true`.
- `trend.ceiling.direction = "improved"`.

Waiver pressure rose:

- Previous waiver count is lower than current waiver count.
- `history_summary.waiver_pressure_increased = true`.
- `trend.waiver_count.direction = "regressed"`.
- The report recommends waiver review before stricter policy.

Baseline shrank:

- Current `baseline_still_present` is lower than the previous comparable
  snapshot or `baseline_resolved` increased.
- No auto-adopt behavior is present.
- `history_summary.baseline_shrank = true`.
- The report preserves shrink-only language.

Preview remained advisory:

- Preview evidence is present in current or historical snapshots.
- Gate eligibility, RIPR Zero blocking debt, and calibrated confidence for
  preview evidence remain excluded.
- `history_summary.preview_remained_advisory = true`.

Malformed history:

- A supplied JSONL line is malformed.
- The report keeps usable lines, records the malformed line as a warning, and
  marks the history input as `malformed` if no useful history can be read.
- The command still does not write or repair the history file.

## Test Mapping

Follow-up implementation should include:

- CLI option parsing tests for `ripr policy history`.
- JSON/Markdown tests for single-snapshot, readiness-improved,
  waiver-pressure-regressed, baseline-shrank, preview-still-advisory, missing
  history, and malformed-history cases.
- Unit tests for ceiling rank comparison.
- Unit tests for count trend direction.
- Unit tests proving missing history is an unknown, not a failure.
- Unit tests proving malformed history is a warning/config problem and never
  causes automatic repair.
- Unit tests proving no writer path appends to `.ripr/policy-history.jsonl`.

## Implementation Mapping

This spec belongs to the focused Lane 2 tracker in
[Policy operations](../policy/POLICY_OPERATIONS.md).

Implementation should be split into later work items:

- `spec/policy-history-ledger` defines this report contract.
- `policy/history-report` implements the read-only JSON/Markdown producer.
- `spec/policy-promotion-packets` defines target-mode promotion packets after
  history trend exists.
- `policy/promotion-packet-report` implements promotion packets without config,
  baseline, suppression, workflow, or CI mutation.
- `spec/preview-evidence-promotion-packet` defines future preview promotion
  packets without changing default preview eligibility.

No implementation PR may change analyzer truth, evidence identity, gate
semantics, LSP/editor behavior, provider behavior, mutation execution, source
files, generated tests, branch protection, default CI blocking, automatic
history append, or preview-language eligibility to satisfy this spec.

## Metrics

The report makes these metrics available to later capability and trend
surfaces:

- `policy_history_reports`
- `policy_history_entries`
- `policy_history_readiness_improved`
- `policy_history_waiver_pressure_increased`
- `policy_history_suppression_health_regressed`
- `policy_history_baseline_shrank`
- `policy_history_preview_remained_advisory`
- `policy_history_calibration_changed_ceiling`
- `policy_history_warning_count`
- `policy_history_unknown_count`
