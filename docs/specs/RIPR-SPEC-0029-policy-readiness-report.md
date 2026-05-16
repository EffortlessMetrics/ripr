# RIPR-SPEC-0029: Policy Readiness Report

Status: proposed

## Problem

RIPR already has distinct advisory and policy surfaces: PR guidance,
recommendation calibration, optional calibrated gates, reviewed baselines,
baseline debt deltas, RIPR Zero status, PR evidence ledgers, suppressions, and
imported mutation calibration. These surfaces are useful separately, but a
maintainer still has to inspect several artifacts to answer one operational
question:

```text
Which policy mode is safe for this repo right now?
```

Without a readiness layer, teams can either stay advisory longer than needed or
tighten policy before the evidence, baseline, waiver, suppression, or
calibration ledgers are mature enough. That erodes trust: old debt can look like
new debt, preview-language evidence can look calibrated, waivers can look like
durable exceptions, and optional gates can feel like noisy gateware.

## Product Contract

The policy readiness report is a read-only advisory report over explicit
existing artifacts. It recommends the strictest safe policy posture for the
current repository state without changing any policy decision itself.

The report must:

- read only the paths supplied on the command line;
- preserve baselines, waivers, suppressions, recommendation calibration,
  mutation calibration, and preview-language evidence as separate health axes;
- recommend one policy posture using bounded vocabulary;
- explain which missing, stale, invalid, or immature evidence prevents a
  stricter posture;
- make preview-language evidence visible but non-gating unless a later explicit
  policy promotes it;
- write deterministic JSON and Markdown;
- never block CI by itself.

The report must not rerun analysis, post comments, edit source, mutate
baselines, create suppressions, change gate decisions, run mutation testing,
call providers, generate tests, upload artifacts, or change generated workflow
defaults.

## Behavior

The readiness producer joins existing report artifacts into a control-room
inspection:

```text
gate decision
baseline debt delta
recommendation calibration
mutation calibration
waiver aging
suppression health
preview evidence metadata
-> policy readiness report
```

The report answers:

```text
Should this repo stay advisory, use visible-only, require acknowledgement,
enable baseline-check, or narrowly enable calibrated-gate?
```

It is not a new gate. It can say that `baseline-check` is ready, but the only
pass/fail authority remains an explicitly configured gate decision. It can say
that calibration is strong enough for a narrow gate, but it cannot turn
calibration into runtime proof.

## Required Evidence

The report contract is proven only when implementation can show:

- a `policy-readiness.json` report with schema version `0.1`;
- a Markdown sibling suitable for job summaries and report packets;
- explicit input recording for every supplied artifact path;
- readiness status derivation for `advisory_only`,
  `ready_for_visible_only`, `ready_for_acknowledgeable`,
  `ready_for_baseline_check`, `ready_for_calibrated_gate`, `not_ready`, and
  `config_error`;
- separate health axes for blocking readiness, baseline health, waiver health,
  suppression health, calibration health, and preview evidence boundary;
- missing optional inputs recorded as warnings or unknowns, not as passing
  evidence;
- malformed required inputs recorded as `config_error` with repair guidance;
- preview-language findings recorded as visible/advisory and excluded from gate
  eligibility, RIPR Zero blocking debt, and calibrated confidence unless a
  later explicit policy promotes them;
- no source edits, generated tests, mutation execution, provider calls,
  baseline mutation, suppression mutation, comment posting, or default CI
  blocking.

## Inputs

Input artifacts:

| Input | Required? | Purpose |
| --- | --- | --- |
| Gate decision JSON | recommended | Current policy decisions, configured mode, candidate counts, acknowledgements, suppressions, calibration use, and blocking state. |
| Baseline debt delta JSON | recommended for baseline modes | Existing, resolved, new, acknowledged, suppressed, stale, invalid, and missing-input debt movement. |
| Recommendation calibration JSON | recommended for calibrated modes | Usefulness, noise, placement, suppression correctness, target quality, and static movement health. |
| Mutation calibration JSON | optional | Imported runtime calibration evidence, used only when it matches the same static candidate class and identity. |
| Waiver aging JSON | recommended for acknowledgeable modes | Repeated PR-time visible acknowledgements, age, repeated seam/file patterns, and repair or suppression candidates. |
| Suppression health JSON | recommended for any policy tightening | Durable suppression metadata health, stale review windows, selector validity, and preview-label correctness. |
| Repo config summary | optional | Explicit policy mode defaults, acknowledgement labels, baseline path, suppression path, and language enablement. |
| Prior readiness JSON | optional | Trend context for whether readiness is improving or degrading. |

Missing recommended or optional inputs must produce warnings or `unknown`
health. Missing inputs must not be treated as passing evidence. Malformed
required inputs for a requested output should produce `config_error` with a
repair hint.

## Command Surface

The planned command is:

```text
ripr policy readiness \
  --gate-decision target/ripr/reports/gate-decision.json \
  --baseline-delta target/ripr/reports/baseline-debt-delta.json \
  --recommendation-calibration target/ripr/reports/recommendation-calibration.json \
  --mutation-calibration target/ripr/reports/mutation-calibration.json \
  --waiver-aging target/ripr/reports/waiver-aging.json \
  --suppression-health target/ripr/reports/suppression-health.json \
  --out target/ripr/reports/policy-readiness.json \
  --out-md target/ripr/reports/policy-readiness.md
```

The exact CLI nesting may be adjusted during implementation, but the public
behavior must preserve explicit inputs, read-only execution, advisory output,
and no gate authority.

## Readiness Status

Top-level `status` is one of:

- `advisory_only` - policy should remain advisory; evidence is useful to show,
  but the repo is not ready for stricter policy.
- `ready_for_visible_only` - the repo can safely render policy decisions
  without pass/fail authority.
- `ready_for_acknowledgeable` - the repo can require visible acknowledgement
  for narrow policy-eligible evidence while keeping escape hatches auditable.
- `ready_for_baseline_check` - the repo has reviewed baseline health and can
  govern new policy-eligible debt against a baseline.
- `ready_for_calibrated_gate` - the repo has narrow, calibrated, stable evidence
  that can support an explicit calibrated gate.
- `not_ready` - required evidence for the requested policy posture is missing,
  stale, invalid, or too noisy.
- `config_error` - supplied inputs or requested options are malformed or
  internally inconsistent.

`recommended_mode` uses the deployable mode vocabulary. Values other than
`advisory-only` must match the existing gate mode strings accepted by
`RIPR_GATE_MODE` and the gate CLI. `advisory-only` is a readiness sentinel that
means keep generated CI advisory and leave gate mode unset.

- `advisory-only`
- `visible-only`
- `acknowledgeable`
- `baseline-check`
- `calibrated-gate`

`status` explains readiness. `recommended_mode` is the policy posture a
maintainer can choose next.

## Policy Axes

The report evaluates these independent axes:

| Axis | Healthy when | Blocks stricter recommendation when |
| --- | --- | --- |
| `blocking_readiness` | Current gate decisions are visible, scoped, and explicit. | Gate input is missing, blocked by config error, or using unsupported mode semantics. |
| `baseline_health` | Baseline delta is present, reviewed, shrink-only compatible, and stale debt is visible. | Baseline is missing for baseline modes, auto-adopts new debt, hides old debt, or has high invalid/stale counts. |
| `waiver_health` | Waivers are visible acknowledgements with label, identity, owner/source, age, and repeat counts. | Waivers are missing, hidden, repeatedly used without owner/source, or treated as durable suppressions. |
| `suppression_health` | Suppressions have identity, owner, reason, scope, dates, expected visibility, and static class. | Suppressions are overbroad, stale, missing owner/reason, unknown selectors, or preview suppressions lack preview labels. |
| `calibration_health` | Recommendation calibration is useful, low-noise, and tied to the same candidate class; mutation calibration is imported and joined unambiguously when used. | Calibration is missing for calibrated modes, noisy, stale, ambiguous, runtime-only, or unmatched to current static candidates. |
| `preview_evidence_boundary` | Preview findings are labeled and visible as advisory, with explicit static limits. | Preview findings would affect gates, RIPR Zero blocking debt, or calibrated confidence without explicit promotion. |

Each axis writes:

- `state`: `healthy`, `warning`, `not_ready`, `missing`, or `config_error`;
- `evidence`: short facts copied from source artifacts;
- `warnings`: bounded warning records;
- `next_action`: one concrete repair or rollout action.

## Readiness Rules

The report recommends the strictest mode whose prerequisites are satisfied.

`advisory_only` is always safe when inputs are readable enough to explain the
current state. It is recommended when any stronger mode lacks evidence.

`visible_only` requires:

- gate decision or equivalent policy-state input is readable;
- findings remain visible even when suppressed, acknowledged, or baseline-known;
- preview-language evidence is labeled or absent;
- no input claims runtime proof or mutation adequacy.

`acknowledgeable` additionally requires:

- acknowledgement labels are explicit;
- waiver aging is available or clearly not applicable;
- waivers are visible PR-time acknowledgements, not durable suppressions;
- suppression health has no unowned or reasonless durable exceptions for the
  candidate class being tightened.

`baseline_check` additionally requires:

- baseline debt delta is available;
- the baseline is reviewed or missing metadata is explicitly bounded;
- shrink-only refresh is available for resolved debt;
- new policy-eligible debt is distinguishable from existing debt;
- no CI auto-adopts new baseline entries.

`calibrated_gate` additionally requires:

- recommendation calibration supports the same candidate class with low noise;
- optional mutation calibration is imported, not executed, and joined
  unambiguously when it raises confidence;
- candidates are stable Rust evidence or explicitly promoted preview evidence;
- policy-eligible classes are narrow and documented;
- blocking remains explicit, narrow, and reversible.

Preview TypeScript and Python evidence must keep the recommendation at or below
`visible-only` for that evidence unless a later explicit policy says otherwise.
Preview evidence can coexist with a stricter Rust-only recommendation only when
the report records that preview candidates are excluded from blocking, RIPR Zero
blocking debt, and calibrated confidence.

## Outputs

The report writes:

```text
target/ripr/reports/policy-readiness.json
target/ripr/reports/policy-readiness.md
```

The JSON report uses schema version `0.1`:

```json
{
  "schema_version": "0.1",
  "tool": "ripr",
  "kind": "policy_readiness",
  "status": "ready_for_baseline_check",
  "recommended_mode": "baseline-check",
  "root": ".",
  "generated_at": "2026-05-12T00:00:00Z",
  "inputs": {
    "gate_decision": "target/ripr/reports/gate-decision.json",
    "baseline_delta": "target/ripr/reports/baseline-debt-delta.json",
    "recommendation_calibration": "target/ripr/reports/recommendation-calibration.json",
    "mutation_calibration": null,
    "waiver_aging": "target/ripr/reports/waiver-aging.json",
    "suppression_health": "target/ripr/reports/suppression-health.json",
    "repo_config": null,
    "previous_readiness": null
  },
  "summary": {
    "blocking_ready": false,
    "visible_only_ready": true,
    "acknowledgeable_ready": true,
    "baseline_check_ready": true,
    "calibrated_gate_ready": false,
    "preview_candidates": 3,
    "preview_candidates_gate_eligible": 0,
    "warnings": 2
  },
  "blocking_readiness": {
    "state": "healthy",
    "current_gate_mode": "visible-only",
    "candidate_classes": ["weakly_gripped", "reachable_unrevealed"],
    "blocking_candidates": 0,
    "next_action": "Keep generated CI advisory unless RIPR_GATE_MODE is explicitly configured."
  },
  "baseline_health": {
    "state": "healthy",
    "existing_debt_visible": 40,
    "resolved": 7,
    "new_policy_eligible": 1,
    "stale": 2,
    "invalid": 0,
    "auto_adopt_new": false,
    "next_action": "Use baseline-check only with the reviewed baseline path supplied."
  },
  "waiver_health": {
    "state": "warning",
    "waivers": 5,
    "repeated_seams": 1,
    "oldest_age_days": 21,
    "next_action": "Review the repeated seam before requiring acknowledgement."
  },
  "suppression_health": {
    "state": "healthy",
    "suppressions": 4,
    "missing_owner": 0,
    "missing_reason": 0,
    "stale": 0,
    "overbroad": 0,
    "unknown_selector": 0,
    "preview_without_label": 0,
    "next_action": "Keep suppressions visible with owner and reason."
  },
  "calibration_health": {
    "state": "not_ready",
    "recommendation_useful_rate": 0.72,
    "false_annotation_rate": 0.03,
    "mutation_calibration": "missing",
    "same_class_support": false,
    "next_action": "Collect same-class recommendation calibration before calibrated-gate."
  },
  "preview_evidence_boundary": {
    "state": "healthy",
    "preview_languages": ["typescript", "python"],
    "preview_findings_visible": 3,
    "preview_findings_gate_eligible": 0,
    "preview_findings_ripr_zero_blocking": 0,
    "preview_findings_calibrated_confidence": 0,
    "static_limits_required": true,
    "next_action": "Keep preview evidence advisory until an explicit promotion policy exists."
  },
  "unknowns": [
    {
      "kind": "missing_optional_input",
      "message": "No mutation calibration input was supplied.",
      "source_artifact": null
    }
  ],
  "warnings": [
    {
      "kind": "waiver_aging",
      "message": "One seam has repeated waivers.",
      "source_artifact": "target/ripr/reports/waiver-aging.json"
    }
  ],
  "next_policy_action": "Enable baseline-check for stable Rust evidence only; keep preview-language evidence visible/advisory.",
  "limits_note": "Read-only advisory readiness over explicit artifacts; gate-decision remains the only pass/fail authority when configured."
}
```

Field contract:

- `schema_version` - currently `"0.1"`.
- `tool` - always `"ripr"`.
- `kind` - always `"policy_readiness"`.
- `status` - one readiness status from this spec.
- `recommended_mode` - one deployable mode from this spec.
- `root` - workspace root used for path display.
- `generated_at` - generation timestamp when available.
- `inputs` - paths supplied to the command, or `null` when omitted.
- `summary.*_ready` - booleans for whether each stricter posture is currently
  safe.
- `summary.preview_candidates_gate_eligible` - must be zero unless a later
  explicit policy promotes preview evidence.
- `blocking_readiness` - health over current gate decision semantics and
  blocking scope.
- `baseline_health` - health over reviewed baseline and baseline-delta
  movement. `auto_adopt_new` must be `false` for `baseline_check` readiness.
- `waiver_health` - PR-time acknowledgement health. Waivers do not hide
  evidence and do not become suppressions.
- `suppression_health` - durable exception ledger health. Suppressed findings
  remain visible with reason.
- `calibration_health` - recommendation calibration and optional imported
  mutation calibration health. Runtime-only or ambiguous mutation data must not
  raise readiness.
- `preview_evidence_boundary` - preview-language policy state. Preview evidence
  is visible and advisory by default.
- `unknowns[]` - missing or unknown context that does not necessarily block the
  recommended mode.
- `warnings[]` - bounded warning records with `kind`, `message`, and optional
  `source_artifact`.
- `next_policy_action` - one concise action for maintainers.
- `limits_note` - static/advisory/pass-fail boundary.

The Markdown sibling should fit in generated CI summaries and report packets:

```text
# Policy Readiness

Status: ready_for_baseline_check
Recommended mode: baseline-check

| Axis | State | Next action |
| --- | --- | --- |
| Blocking readiness | healthy | Keep generated CI advisory unless RIPR_GATE_MODE is explicit. |
| Baseline health | healthy | Use reviewed baseline path. |
| Waiver health | warning | Review one repeated seam. |
| Suppression health | healthy | Keep suppressions visible with reason. |
| Calibration health | not_ready | Collect same-class calibration before calibrated-gate. |
| Preview evidence boundary | healthy | Keep preview evidence advisory. |

Next policy action:
Enable baseline-check for stable Rust evidence only; keep preview-language
evidence visible/advisory.

Limits:
This report is read-only advisory readiness. Gate decision remains the only
pass/fail authority when explicitly configured.
```

## Non-Goals

- No analyzer behavior changes.
- No recommendation ranking changes.
- No PR summary rendering changes.
- No LSP or editor behavior changes.
- No generated tests.
- No provider calls.
- No mutation execution.
- No source edits.
- No posting comments.
- No baseline mutation.
- No suppression creation or deletion.
- No generated workflow default changes.
- No default CI blocking.
- No preview-language gate promotion.
- No runtime proof or adequacy claims.

## Acceptance Examples

Advisory-only repo with missing baseline:

- Gate decision and PR guidance are visible.
- Baseline delta is missing.
- Recommendation calibration is missing or too sparse.
- Report status is `advisory_only`.
- `next_policy_action` tells maintainers to create or review a baseline before
  enabling baseline-check.

Ready for acknowledgement:

- Gate decision input is readable.
- Waiver aging exists and records visible acknowledgements.
- Suppressions have owner and reason.
- Baseline delta is missing.
- Report status is `ready_for_acknowledgeable`.
- `baseline_health.state` is `missing` or `not_ready`.

Ready for baseline-check:

- Baseline delta distinguishes existing debt, resolved debt, and new
  policy-eligible debt.
- Shrink-only refresh exists and no auto-adopt-new path is present.
- Suppressions are reasoned.
- Waiver aging has no unbounded repeated waiver signal.
- Report status is `ready_for_baseline_check`.

Not ready for calibrated-gate:

- Recommendation calibration is missing, noisy, or not tied to the same class.
- Mutation calibration is missing, ambiguous, or runtime-only.
- Preview findings dominate the current evidence.
- Report may recommend `visible-only`, `acknowledgeable`, or `baseline-check`,
  but not `calibrated-gate`.

Preview evidence present:

- TypeScript or Python findings are counted in
  `preview_evidence_boundary.preview_findings_visible`.
- They remain at zero for gate eligibility, RIPR Zero blocking debt, and
  calibrated confidence.
- The report may recommend stricter Rust-only policy only when that exclusion
  is explicit in the readiness output.

## Test Mapping

Follow-up implementation should include:

- CLI option parsing tests for `ripr policy readiness`.
- JSON/Markdown fixture cases for advisory-only, visible-only,
  acknowledgeable, baseline-check, calibrated-gate-ready, config-error, and
  preview-evidence-present states.
- Unit tests for readiness mode derivation.
- Unit tests proving missing optional inputs produce warnings instead of
  passing evidence.
- Unit tests proving malformed required inputs produce `config_error`.
- Fixture tests proving preview-language findings do not become gate-eligible,
  RIPR Zero blocking debt, or calibrated confidence by default.
- Generated-CI tests only if a later PR projects readiness artifacts into CI;
  those tests must prove advisory-only behavior.

## Implementation Mapping

This spec is the contract for the focused Lane 2 tracker:

- `spec/policy-readiness-report` defines this report contract.
- `report/policy-readiness` implements the read-only JSON/Markdown producer.
- `report/waiver-aging` supplies the waiver-health input.
- `policy/suppression-ledger-health` supplies the suppression-health input and
  keeps warning/config-error suppression ledgers from qualifying
  acknowledgeable readiness.
- `policy/baseline-refresh-guardrails` defines the no-auto-adopt-new baseline
  rule used by `baseline_health`.
- `policy/exception-ledger-convergence` aligns exception ledger semantics used
  by suppression and baseline health.
- `docs/blocking-readiness-guide` translates readiness output into operator
  rollout guidance.
- `ci/policy-readiness-advisory-projection` may surface artifacts in generated
  CI without pass/fail authority.

No implementation PR may change analyzer truth, recommendation ranking, gate
semantics, LSP/editor behavior, provider behavior, mutation execution, source
files, generated tests, branch protection, or default CI blocking to satisfy
this spec.

## Metrics

The report makes these metrics available to later capability and trend
surfaces:

- `policy_readiness_reports`
- `policy_readiness_recommended_mode`
- `policy_readiness_blocking_ready`
- `policy_readiness_baseline_health_state`
- `policy_readiness_waiver_health_state`
- `policy_readiness_suppression_health_state`
- `policy_readiness_calibration_health_state`
- `policy_readiness_preview_findings_visible`
- `policy_readiness_preview_findings_gate_eligible`
- `policy_readiness_preview_findings_ripr_zero_blocking`
- `policy_readiness_preview_findings_calibrated_confidence`
- `policy_readiness_warning_count`
- `policy_readiness_unknown_count`
