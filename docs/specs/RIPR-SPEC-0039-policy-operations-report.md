# RIPR-SPEC-0039: Policy Operations Report

Status: proposed

## Problem

Policy readiness tells maintainers which posture is safe for a repository right
now. That answers the first Lane 2 question, but a maintainer still needs a
single operating packet that explains what to do next:

```text
What is the current safe policy ceiling?
Which stricter modes are safe now?
Which stricter modes are blocked?
What repairs or receipts would justify promotion?
What preview-language evidence must remain advisory?
```

Without that packet, teams can read readiness reports, waiver-aging reports,
suppression-health reports, baseline deltas, gate decisions, and calibration
reports separately and still miss the policy posture. The risk is not merely
noise. Old debt can be normalized into new baselines, waivers can start acting
like durable exceptions, suppressions can hide findings, preview evidence can
inherit stable Rust policy meaning, and calibrated gates can be enabled before
same-class evidence supports them.

## Behavior

The policy operations report is a read-only advisory report over explicit
existing artifacts. It composes readiness, waiver, suppression, baseline, gate,
calibration, and preview-boundary evidence into a single operator packet.

The report answers:

```text
current safe policy ceiling
recommended next safe action
safe-to-promote modes
not-safe-to-promote modes
promotion blockers
baseline actions
waiver actions
suppression actions
calibration actions
preview-boundary actions
warnings
unknowns
input artifact status
```

The report must not decide pass/fail. It must not mutate `ripr.toml`,
baselines, suppressions, waiver ledgers, workflows, branch protection, generated
CI defaults, or source files. It must not rerun analysis, run mutation tools,
call providers, create tests, post comments, or promote preview-language
evidence.

The report consumes only paths supplied on the command line:

```text
policy readiness
waiver aging
suppression health
baseline debt delta
gate decision
recommendation calibration
mutation calibration when explicitly supplied
preview evidence boundary
-> policy operations report
```

Missing recommended inputs become warnings or unknowns. Malformed required
inputs become `config_error` posture for any target mode that depends on them.
Missing inputs must not be treated as passing evidence.

## Required Evidence

The report contract is satisfied only when implementation can show:

- `target/ripr/reports/policy-operations.json` with schema version `0.1`;
- `target/ripr/reports/policy-operations.md`;
- explicit input artifact records for every supplied or omitted input;
- one `current_policy_ceiling` value using policy-readiness vocabulary;
- one `recommended_next_action`;
- `safe_to_promote_to[]` and `not_safe_to_promote_to[]` entries for target
  modes;
- promotion blockers with source artifact references and repair actions;
- separate baseline, waiver, suppression, calibration, and preview-boundary
  action lists;
- warnings and unknowns for missing, stale, malformed, or not-applicable inputs;
- preview-language evidence reported as visible/advisory and excluded from gate
  eligibility, RIPR Zero blocking debt, and calibrated confidence unless a
  later explicit promotion policy is supplied;
- no analyzer behavior changes, output identity rewrites, LSP/editor changes,
  generated tests, provider calls, mutation execution, default CI blocking,
  automatic config mutation, automatic baseline adoption, automatic suppression
  creation, or preview-language promotion.

## Inputs

The planned command is:

```text
ripr policy operations \
  --policy-readiness target/ripr/reports/policy-readiness.json \
  --waiver-aging target/ripr/reports/waiver-aging.json \
  --suppression-health target/ripr/reports/suppression-health.json \
  --baseline-delta target/ripr/reports/baseline-debt-delta.json \
  --gate-decision target/ripr/reports/gate-decision.json \
  --recommendation-calibration target/ripr/reports/recommendation-calibration.json \
  --mutation-calibration target/ripr/reports/mutation-calibration.json \
  --out target/ripr/reports/policy-operations.json \
  --out-md target/ripr/reports/policy-operations.md
```

Input artifacts:

| Input | Required? | Purpose |
| --- | --- | --- |
| Policy readiness JSON | required | Current policy ceiling, axis health, preview boundary, and next policy action. |
| Waiver aging JSON | recommended | Repeated PR-time acknowledgements, age, owner/source gaps, and repair routes. |
| Suppression health JSON | recommended | Durable exception health, owner/reason/scope/review metadata, stale state, and preview labels. |
| Baseline debt delta JSON | recommended for baseline modes | Existing, resolved, new policy-eligible, stale, invalid, and missing-input debt movement. |
| Gate decision JSON | recommended | Configured mode, policy candidates, acknowledgements, suppressions, baseline state, calibration use, and blocking state. |
| Recommendation calibration JSON | recommended for calibrated modes | Same-class recommendation usefulness, placement, noise, and static movement quality. |
| Mutation calibration JSON | optional | Imported runtime calibration evidence, used only as explicit confidence evidence when same-class joins are unambiguous. |
| Preview boundary report | optional | Explicit preview-language advisory boundary when not already present in policy readiness. |

The command may accept fewer inputs, but the report must then name the missing
inputs and limit its safe promotion posture accordingly.

## Outputs

The JSON report uses schema version `0.1`:

```json
{
  "schema_version": "0.1",
  "tool": "ripr",
  "kind": "policy_operations",
  "root": ".",
  "generated_at": "unix_ms:1778277000000",
  "current_policy_ceiling": "ready_for_acknowledgeable",
  "recommended_next_action": "Run shrink-only baseline review and remove resolved entries.",
  "safe_to_promote_to": [
    {
      "mode": "visible-only",
      "allowed_now": true,
      "reason": "Policy readiness and supplied inputs allow visible-only advisory display.",
      "source_artifacts": [
        "target/ripr/reports/policy-readiness.json",
        "target/ripr/reports/gate-decision.json"
      ]
    },
    {
      "mode": "acknowledgeable",
      "allowed_now": true,
      "reason": "Waivers are visible PR-time acknowledgements and suppression health is readable.",
      "source_artifacts": [
        "target/ripr/reports/policy-readiness.json",
        "target/ripr/reports/waiver-aging.json",
        "target/ripr/reports/suppression-health.json"
      ]
    }
  ],
  "not_safe_to_promote_to": [
    {
      "mode": "baseline-check",
      "allowed_now": false,
      "reason": "Current policy ceiling ready_for_acknowledgeable does not allow baseline-check. Baseline contains 1 stale entries.",
      "blockers": [
        "current_ceiling_below_baseline_check",
        "baseline_stale_entries"
      ],
      "source_artifacts": [
        "target/ripr/reports/baseline-debt-delta.json"
      ]
    }
  ],
  "promotion_blockers": [
    {
      "kind": "baseline_stale_entries",
      "severity": "warning",
      "message": "Baseline contains 1 stale entries.",
      "target_modes": ["baseline-check", "calibrated-gate"],
      "source_artifact": "target/ripr/reports/baseline-debt-delta.json",
      "repair_action": "Run shrink-only baseline review and remove resolved entries."
    }
  ],
  "baseline_actions": [
    "Review stale baseline entries.",
    "Use shrink-only refresh for resolved debt."
  ],
  "waiver_actions": [
    "Review repeated PR-time acknowledgements before requiring acknowledgement.",
    "Keep waivers visible and do not convert them to suppressions automatically."
  ],
  "suppression_actions": [
    "Keep durable suppressions visible with owner, reason, scope, and review metadata."
  ],
  "calibration_actions": [
    "Collect same-class recommendation calibration before calibrated-gate.",
    "Optional mutation calibration was not supplied; keep runtime confirmation separate from static evidence."
  ],
  "preview_boundary_actions": [
    "Keep typescript preview evidence visible/advisory and excluded from gate eligibility, RIPR Zero blocking debt, and calibrated confidence."
  ],
  "warnings": [
    {
      "kind": "missing_optional_input",
      "message": "No mutation calibration input was supplied.",
      "source_artifact": null
    }
  ],
  "unknowns": [
    {
      "kind": "preview_boundary_not_supplied",
      "message": "Preview boundary details came only from policy readiness when available.",
      "source_artifact": "target/ripr/reports/policy-readiness.json"
    }
  ],
  "input_artifacts": [
    {
      "kind": "policy_readiness",
      "path": "target/ripr/reports/policy-readiness.json",
      "status": "read"
    },
    {
      "kind": "preview_boundary",
      "path": null,
      "status": "omitted"
    }
  ],
  "limits_note": "Read-only advisory policy operations report over explicit existing artifacts. Promotion requires separate manual review and configuration changes; this report never mutates config, baselines, suppressions, workflows, CI defaults, or preview-language eligibility."
}
```

Field contract:

- `schema_version` - currently `"0.1"`.
- `tool` - always `"ripr"`.
- `kind` - always `"policy_operations"`.
- `current_policy_ceiling` - copied or derived from policy readiness.
  Supported values are `advisory_only`, `ready_for_visible_only`,
  `ready_for_acknowledgeable`, `ready_for_baseline_check`,
  `ready_for_calibrated_gate`, `not_ready`, and `config_error`.
- `recommended_next_action` - the first repair or operator action needed before
  stricter policy review.
- `safe_to_promote_to[]` - target modes currently allowed by the ceiling and
  readable dependent inputs.
- `not_safe_to_promote_to[]` - target modes blocked by ceiling, baseline,
  waiver, suppression, calibration, preview-boundary, or input health.
- `promotion_blockers[]` - normalized blocker records with severity, target
  modes, source artifact, and repair action.
- `baseline_actions[]`, `waiver_actions[]`, `suppression_actions[]`,
  `calibration_actions[]`, and `preview_boundary_actions[]` - operator actions
  grouped by policy surface.
- `warnings[]` - malformed supplied inputs or optional evidence gaps.
- `unknowns[]` - missing or unknowable context that limits confidence.
- `input_artifacts[]` - one record for every operations input. Status values
  are `read`, `omitted`, `missing`, `malformed`, and `not_applicable`.
- `limits_note` - static advisory boundary and no-mutation policy text.

The Markdown sibling should fit in generated CI summaries and report packets:

```text
# RIPR Policy Operations

Current ceiling: ready_for_acknowledgeable
Next safe action: Run shrink-only baseline review and remove resolved entries.

Can promote to:
- visible-only
- acknowledgeable

Cannot promote to:
- baseline-check: current ceiling is below baseline-check
- calibrated-gate: same-class calibration is insufficient

Top blockers:
- baseline_stale_entries: Run shrink-only baseline review and remove resolved entries.

Baseline actions:
- Review stale baseline entries.

Waiver actions:
- Review repeated PR-time acknowledgements before requiring acknowledgement.

Suppression actions:
- Keep durable suppressions visible with owner, reason, scope, and review metadata.

Calibration actions:
- Collect same-class recommendation calibration before calibrated-gate.

Preview evidence boundary:
- Keep preview evidence visible/advisory and excluded from gate eligibility.

Input artifacts:
- policy_readiness: read target/ripr/reports/policy-readiness.json

Limits:
Read-only advisory policy operations report. Promotion requires manual review.
```

Markdown must show current ceiling, next safe action, can-promote and
cannot-promote sections, top blockers, grouped actions, warnings, unknowns,
input artifact status, and limits. It must not make a gate decision or promote
preview-language evidence.

## Promotion Rules

The operations report uses the current policy ceiling as the first bound:

| Current ceiling | Safe target modes |
| --- | --- |
| `advisory_only` | none |
| `ready_for_visible_only` | `visible-only` |
| `ready_for_acknowledgeable` | `visible-only`, `acknowledgeable` |
| `ready_for_baseline_check` | `visible-only`, `acknowledgeable`, `baseline-check` |
| `ready_for_calibrated_gate` | `visible-only`, `acknowledgeable`, `baseline-check`, `calibrated-gate` for eligible stable Rust classes |
| `not_ready` | none |
| `config_error` | none |

The ceiling is necessary but not sufficient. A target can still be blocked by
missing inputs, malformed inputs, stale baselines, waiver pressure, suppression
health warnings, insufficient same-class calibration, or preview evidence that
has not been explicitly promoted.

Preview TypeScript and Python evidence must remain visible/advisory and count
as not gate-eligible, not RIPR Zero blocking debt, and not calibrated
confidence unless a later preview promotion packet explicitly changes that
policy.

## Non-Goals

- No analyzer behavior changes.
- No evidence identity rewrites.
- No recommendation ranking changes.
- No LSP or editor behavior changes.
- No PR/CI front-panel redesign.
- No generated tests.
- No provider calls.
- No mutation execution.
- No source edits.
- No posting comments.
- No baseline mutation.
- No suppression creation or deletion.
- No generated workflow default changes.
- No branch-protection changes.
- No default CI blocking.
- No automatic config mutation.
- No automatic baseline adoption.
- No automatic suppression creation.
- No preview-language gate promotion.
- No runtime-proof or adequacy claims from static evidence.

## Acceptance Examples

Advisory-only repo:

- Policy readiness reports `advisory_only`.
- Baseline, waiver, suppression, or calibration inputs may be missing.
- `safe_to_promote_to[]` is empty.
- `not_safe_to_promote_to[]` names visible-only and stricter targets with
  blockers.
- `recommended_next_action` tells maintainers which evidence to collect first.

Ready for acknowledgement:

- Policy readiness reports `ready_for_acknowledgeable`.
- Waiver aging is readable and keeps waivers as visible PR-time
  acknowledgements.
- Suppression health is readable.
- `safe_to_promote_to[]` includes `visible-only` and `acknowledgeable`.
- `baseline-check` is blocked until baseline health is reviewed.

Ready for baseline-check:

- Policy readiness reports `ready_for_baseline_check`.
- Baseline delta distinguishes existing debt, resolved debt, and new
  policy-eligible debt.
- No auto-adopt-new baseline path is treated as safe.
- `safe_to_promote_to[]` includes `baseline-check`.

Not ready for calibrated-gate:

- Recommendation calibration is missing, noisy, or not tied to the same class.
- Mutation calibration is missing, ambiguous, or runtime-only.
- `calibrated-gate` is listed under `not_safe_to_promote_to[]`.
- `calibration_actions[]` names the same-class receipts to collect.

Preview evidence present:

- TypeScript or Python preview evidence is visible in the input artifacts.
- `preview_boundary_actions[]` says preview evidence remains advisory.
- No preview finding becomes gate-eligible, RIPR Zero blocking debt, or
  calibrated confidence.
- The report may still allow a stricter Rust-only target only when preview
  exclusion is explicit.

Malformed required input:

- A malformed policy readiness input produces `current_policy_ceiling =
  "config_error"`.
- All stricter targets are blocked.
- `warnings[]` and `input_artifacts[]` name the malformed input and repair
  route.

## Test Mapping

Follow-up implementation should include:

- CLI option parsing tests for `ripr policy operations`.
- JSON and Markdown fixture cases for advisory-only, visible-only,
  acknowledgeable, baseline-check, calibrated-gate-ready, missing inputs,
  malformed required inputs, and preview-evidence-present states.
- Unit tests for promotion mode derivation from `current_policy_ceiling`.
- Unit tests proving missing optional inputs create warnings or unknowns instead
  of passing evidence.
- Unit tests proving malformed required inputs block dependent targets.
- Unit tests proving preview evidence remains advisory unless an explicit later
  promotion policy is supplied.
- Output-contract checks for the JSON kind, schema version, status vocabulary,
  and input artifact status vocabulary.

## Implementation Mapping

This spec is the contract for the focused Lane 2 tracker:

- `spec/policy-operations-report` defines this report contract.
- `policy/operations-report` implements the read-only JSON and Markdown
  producer.
- `policy-readiness` supplies the policy ceiling and axis health.
- `report/waiver-aging` supplies PR-time acknowledgement pressure.
- `policy/suppression-ledger-health` supplies durable exception health.
- `policy/baseline-refresh-guardrails` supplies shrink-only baseline repair
  boundaries.
- `policy/exception-ledger-convergence` aligns baseline, waiver, and
  suppression semantics.
- `spec/policy-history-ledger` later adds trend context over policy operations.
- `spec/policy-promotion-packets` later turns a target mode into a dedicated
  manual-review packet.

No implementation PR may change analyzer truth, recommendation ranking, gate
semantics, LSP/editor behavior, provider behavior, mutation execution, source
files, generated tests, branch protection, preview eligibility, or default CI
blocking to satisfy this spec.

## Metrics

The report makes these metrics available to later capability and trend
surfaces:

- `policy_operations_reports`
- `policy_operations_current_ceiling`
- `policy_operations_recommended_next_action`
- `policy_operations_safe_target_count`
- `policy_operations_blocked_target_count`
- `policy_operations_promotion_blocker_count`
- `policy_operations_baseline_action_count`
- `policy_operations_waiver_action_count`
- `policy_operations_suppression_action_count`
- `policy_operations_calibration_action_count`
- `policy_operations_preview_boundary_action_count`
- `policy_operations_warning_count`
- `policy_operations_unknown_count`
- `policy_operations_input_read_count`
- `policy_operations_input_missing_count`
- `policy_operations_input_malformed_count`
