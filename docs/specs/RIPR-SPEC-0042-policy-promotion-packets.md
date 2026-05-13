# RIPR-SPEC-0042: Policy Promotion Packets

Status: proposed

## Problem

Policy operations can say which modes are safe now and which modes are blocked.
That is enough for situational awareness, but a maintainer still needs a
reviewable packet before changing policy posture.

The packet should answer:

```text
Can this repo move to the target mode now?
Why or why not?
What repairs are required first?
What receipts should reviewers inspect?
How would the change be rolled back?
What config change would be reviewed manually?
Which inputs support the decision?
What remains out of scope?
```

Without a promotion packet, maintainers can jump from a general operations
report to a config change without recording the evidence boundary, rollback
path, and non-goals for the target mode.

## Behavior

The policy promotion packet is a read-only advisory report over explicit policy
operations and optional policy history inputs. It targets one configured mode:

```text
visible-only
acknowledgeable
baseline-check
calibrated-gate
```

The command must never mutate `ripr.toml`, baselines, suppressions, workflows,
branch protection, generated CI defaults, source files, or history ledgers. It
must not execute a gate, post comments, run analysis, generate tests, call
providers, run mutation testing, or promote preview-language evidence.

Promotion means "allowed to review a manual config change", not "automatically
enable this mode". `ripr gate evaluate` remains the only pass/fail authority
when a gate mode is explicitly configured.

## Required Evidence

The report contract is satisfied only when implementation can show:

- `target/ripr/reports/policy-promotion-visible-only.json` and `.md`;
- `target/ripr/reports/policy-promotion-acknowledgeable.json` and `.md`;
- `target/ripr/reports/policy-promotion-baseline-check.json` and `.md`;
- `target/ripr/reports/policy-promotion-calibrated-gate.json` and `.md`;
- `target_mode`, `allowed_now`, `why_or_why_not`, `required_repairs`,
  `required_receipts`, `rollback_path`, `example_config_change`,
  `input_artifacts`, `warnings`, `unknowns`, `non_goals`, and `limits_note`
  fields;
- target-mode checks that respect the current policy operations ceiling and
  blockers;
- calibrated-gate language that restricts any manual promotion to eligible
  stable Rust classes with same-class calibration;
- preview-language evidence remains advisory unless a later preview promotion
  packet explicitly handles it;
- no config, baseline, suppression, workflow, branch-protection, generated CI,
  history, source, or preview eligibility mutation.

## Inputs

The planned command is:

```text
ripr policy promote \
  --to baseline-check \
  --operations target/ripr/reports/policy-operations.json \
  --history target/ripr/reports/policy-history.json \
  --out target/ripr/reports/policy-promotion-baseline-check.json \
  --out-md target/ripr/reports/policy-promotion-baseline-check.md
```

Input artifacts:

| Input | Required? | Purpose |
| --- | --- | --- |
| Policy operations JSON | required | Current ceiling, safe/not-safe promotion modes, promotion blockers, grouped actions, warnings, unknowns, and input artifact status. |
| Policy history JSON | optional | Trend context and rollback confidence. Missing history is an unknown, not passing evidence. |
| Target mode | required | One of `visible-only`, `acknowledgeable`, `baseline-check`, or `calibrated-gate`. |

## Outputs

The JSON report uses schema version `0.1`:

```json
{
  "schema_version": "0.1",
  "tool": "ripr",
  "kind": "policy_promotion_packet",
  "root": ".",
  "generated_at": "unix_ms:1778277000000",
  "target_mode": "baseline-check",
  "allowed_now": false,
  "why_or_why_not": "Baseline contains stale entries and suppression health has warnings.",
  "required_repairs": [
    "Run shrink-only baseline review and remove resolved entries.",
    "Repair suppression-health warnings before tightening policy."
  ],
  "required_receipts": [
    "policy-operations.json showing baseline-check in safe_to_promote_to",
    "policy-history.json showing baseline debt is not being normalized",
    "baseline-debt-delta.json showing reviewed shrink-only movement",
    "suppression-health.json showing durable exception metadata is healthy"
  ],
  "rollback_path": [
    "Revert the manual gate-mode config change.",
    "Return to visible-only or acknowledgeable policy mode.",
    "Keep policy operations and history artifacts for audit."
  ],
  "example_config_change": {
    "file": "ripr.toml",
    "change": "Set the reviewed policy gate mode to baseline-check.",
    "manual_only": true
  },
  "input_artifacts": [
    {
      "kind": "policy_operations",
      "path": "target/ripr/reports/policy-operations.json",
      "status": "read"
    },
    {
      "kind": "policy_history",
      "path": "target/ripr/reports/policy-history.json",
      "status": "read"
    }
  ],
  "warnings": [],
  "unknowns": [],
  "non_goals": [
    "No automatic config mutation.",
    "No automatic baseline adoption.",
    "No suppression creation.",
    "No default CI blocking.",
    "No preview-language promotion."
  ],
  "limits_note": "Read-only advisory promotion packet. It supports manual review only and never mutates policy configuration or gate authority."
}
```

Field contract:

- `schema_version` - currently `"0.1"`.
- `tool` - always `"ripr"`.
- `kind` - always `"policy_promotion_packet"`.
- `target_mode` - requested target mode.
- `allowed_now` - true only when policy operations lists the target mode in
  `safe_to_promote_to`.
- `why_or_why_not` - concise explanation from policy operations safe/not-safe
  entries and blockers.
- `required_repairs[]` - blocker repair actions required before manual
  promotion review. Empty only when `allowed_now = true`.
- `required_receipts[]` - artifacts reviewers should inspect before accepting
  a manual config change.
- `rollback_path[]` - explicit steps to return to a less strict posture.
- `example_config_change` - manual review guidance only. The command must not
  write this change.
- `input_artifacts[]` - per-input status. Status values are `read`, `omitted`,
  `missing`, `malformed`, and `not_applicable`.
- `warnings[]` - malformed supplied inputs, unsupported history shape, or
  target-mode limitations.
- `unknowns[]` - missing optional history or unavailable supporting context.
- `non_goals[]` - hard boundaries repeated in the packet.
- `limits_note` - read-only/manual-review/no-mutation boundary.

Markdown should fit in generated CI summaries and report packets:

```text
# RIPR Policy Promotion Packet

Target mode: baseline-check
Allowed now: no
Why: Baseline contains stale entries and suppression health has warnings.

## Required Repairs

- Run shrink-only baseline review and remove resolved entries.
- Repair suppression-health warnings before tightening policy.

## Required Receipts

- policy-operations.json showing baseline-check in safe_to_promote_to
- policy-history.json showing baseline debt is not being normalized

## Rollback

- Revert the manual gate-mode config change.
- Return to visible-only or acknowledgeable policy mode.

## Example Config Change

Manual review only. This command does not edit ripr.toml.
```

## Target Mode Rules

The packet uses current policy operations as the source of truth:

| Target mode | Allowed when |
| --- | --- |
| `visible-only` | Present in `safe_to_promote_to`. |
| `acknowledgeable` | Present in `safe_to_promote_to` and waiver/suppression inputs are healthy enough for the operations report. |
| `baseline-check` | Present in `safe_to_promote_to`, baseline movement is reviewed, stale/invalid baseline entries are absent, suppressions are healthy, and preview evidence remains non-gating. |
| `calibrated-gate` | Present in `safe_to_promote_to`, baseline-check requirements hold, and same-class stable Rust calibration supports the target class. |

If the target is absent from `safe_to_promote_to`, the packet must set
`allowed_now = false` and list blockers from policy operations. Missing
optional history should add an unknown but must not by itself make a blocked
target look allowed.

## Non-Goals

- No config mutation.
- No baseline mutation or adoption.
- No suppression creation, deletion, or auto-expiry.
- No workflow, branch-protection, or generated CI mutation.
- No history append.
- No gate decision.
- No default CI blocking.
- No analyzer behavior changes.
- No evidence identity rewrites.
- No LSP or editor behavior changes.
- No generated tests.
- No provider calls.
- No mutation execution.
- No preview-language gate promotion.
- No static evidence claim that runtime behavior was confirmed.

## Acceptance Examples

Visible-only allowed:

- Policy operations lists `visible-only` in `safe_to_promote_to`.
- `allowed_now = true`.
- Required repairs are empty.
- Required receipts include policy operations.
- Example config change is manual-only.

Acknowledgeable blocked:

- Policy operations lists `acknowledgeable` in `not_safe_to_promote_to`.
- Waiver aging or suppression health blockers are present.
- `allowed_now = false`.
- Required repairs include waiver or suppression actions.

Baseline-check blocked:

- Baseline stale, invalid, missing, or auto-adopt blockers are present.
- `allowed_now = false`.
- Required repairs include shrink-only baseline review.
- Required receipts include baseline delta and suppression health.

Calibrated-gate allowed:

- Policy operations lists `calibrated-gate` in `safe_to_promote_to`.
- Required receipts include recommendation calibration and optional imported
  mutation calibration when supplied.
- The packet says calibrated-gate is only for eligible stable Rust classes.

Missing history:

- History input is omitted.
- `unknowns[]` records missing history.
- The packet can still be allowed if policy operations allows the target, but
  required receipts should say history is recommended for trend review.

## Test Mapping

Follow-up implementation should include:

- CLI option parsing tests for `ripr policy promote`.
- JSON/Markdown tests for visible-only allowed, acknowledgeable blocked,
  baseline-check blocked, calibrated-gate allowed, missing history, malformed
  operations input, and unknown target mode.
- Unit tests for safe/not-safe target lookup.
- Unit tests showing required repairs come from policy operations blockers.
- Unit tests showing example config changes are rendered but never written.
- Unit tests showing preview-language promotion is not included.

## Implementation Mapping

This spec belongs to the focused Lane 2 tracker in
[Policy operations](../policy/POLICY_OPERATIONS.md).

Implementation should be split into later work items:

- `spec/policy-promotion-packets` defines this report contract.
- `policy/promotion-packet-report` implements the read-only JSON/Markdown
  producer.
- `spec/preview-evidence-promotion-packet` defines future preview promotion
  packets without changing default preview eligibility.

No implementation PR may change analyzer truth, evidence identity, gate
semantics, LSP/editor behavior, provider behavior, mutation execution, source
files, generated tests, branch protection, generated CI defaults, config,
baselines, suppressions, history, or preview-language eligibility to satisfy
this spec.

## Metrics

The report makes these metrics available to later capability and trend
surfaces:

- `policy_promotion_packets`
- `policy_promotion_allowed`
- `policy_promotion_blocked`
- `policy_promotion_required_repairs`
- `policy_promotion_required_receipts`
- `policy_promotion_missing_history`
- `policy_promotion_warning_count`
- `policy_promotion_unknown_count`
