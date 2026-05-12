# RIPR-SPEC-0030: Preview Evidence Policy Boundary

Status: proposed

## Problem

Campaign 27 introduces opt-in TypeScript and Python preview adapters. Those
adapters intentionally feed the same RIPR evidence surfaces as Rust: repo
exposure, PR guidance, evidence ledgers, assistant surfaces, report packets,
front panels, generated CI summaries, gates, baselines, calibration, and RIPR
Zero reports.

That shared surface is useful only if preview evidence cannot accidentally
inherit mature Rust policy meaning. A syntax-first preview finding may be useful
to show, route, acknowledge, or investigate, but it must not become a default
merge gate, RIPR Zero blocker, baseline-check candidate, or mutation-calibrated
confidence signal merely because it appears in a shared report.

This spec defines the policy boundary for evidence where
`language_status = "preview"`.

## Product Contract

Preview evidence is visible advisory evidence until a later explicit promotion
policy says otherwise.

The policy layer must:

- show preview findings with `language`, `language_status`, and static-limit
  metadata when applicable;
- keep stable Rust evidence eligible for existing policy surfaces when otherwise
  qualified;
- exclude preview evidence from default gate eligibility, RIPR Zero blocking
  debt, default policy baselines, and mutation-calibrated confidence;
- allow visible acknowledgements and durable suppressions without hiding the
  underlying preview finding;
- keep recommendation-calibration observations for preview evidence separate
  from gate confidence;
- require a later explicit promotion spec before any preview class can block,
  count against RIPR Zero, or participate in calibrated-gate confidence;
- preserve generated CI's Rust-default advisory posture.

The policy layer must not reinterpret preview evidence as runtime mutation
outcome, test adequacy, or stable language parity.

## Behavior

The boundary applies after analysis has produced language-tagged static
evidence:

```text
RIPR static evidence
-> language metadata
-> preview policy boundary
-> advisory reports, ledgers, readiness, gates, baselines, calibration, RIPR Zero
```

For `language_status = "stable"` or omitted Rust-only evidence, existing policy
specs continue to apply.

For `language_status = "preview"` evidence:

- evidence can be rendered and counted in advisory surfaces;
- policy reports must preserve the preview label;
- missing or opaque preview analysis must use explicit `static_limit_kind`
  values instead of guessing;
- acknowledgement changes only the visible PR-time decision state;
- suppression changes only durable exception visibility and metadata;
- baseline comparison may record preview evidence only in a partition that is
  visibly preview/advisory and not used for default baseline-check blocking;
- calibration may collect review-quality usefulness/noise observations, but
  preview evidence remains ineligible for mutation-calibrated gate confidence;
- RIPR Zero reports may count preview findings as excluded preview debt, not as
  blocking debt under the default scope;
- gate decisions must classify preview candidates as advisory or
  not-applicable unless a later promotion policy is explicitly configured.

## Required Evidence

This boundary is satisfied only when implementation and docs can show:

- every preview-language finding carries `language` and
  `language_status = "preview"` in public JSON surfaces where language metadata
  is available;
- findings with known static limits carry an explicit `static_limit_kind`;
- policy readiness reports show preview findings as visible and report zero
  default gate-eligible, RIPR Zero blocking, and calibrated-confidence preview
  counts;
- gate decisions do not emit preview candidates as `blocking` under default
  policy modes;
- baseline-check and baseline-delta reports do not auto-adopt preview findings
  into the default policy baseline;
- RIPR Zero reports keep preview findings out of default blocking debt and
  record the exclusion visibly;
- recommendation calibration may record preview usefulness/noise separately
  from gate confidence;
- mutation calibration cannot raise confidence for preview evidence without a
  later promotion spec;
- suppressions for preview findings require owner, reason, scope, review date,
  and a visible preview label;
- generated CI remains advisory and Rust-default unless repo configuration and
  a later policy explicitly opt into stricter preview semantics;
- no analyzer changes, LSP/editor behavior changes, generated tests, mutation
  execution, provider calls, default CI blocking, or preview gate promotion are
  introduced by this spec.

## Inputs

The policy boundary consumes existing artifacts. It does not rerun analysis.

| Input | Required? | Purpose |
| --- | --- | --- |
| Language metadata | required for preview policy | Reads `language` and `language_status` from current findings. |
| Static-limit metadata | required when applicable | Explains preview limitations without guessing. |
| Repo configuration | recommended | Confirms preview languages were explicitly enabled. |
| Gate decision input | optional | Lets the boundary classify preview gate eligibility. |
| Baseline or baseline delta input | optional | Lets the boundary keep preview baseline movement visibly separate. |
| Recommendation calibration input | optional | Records preview review-quality observations without gate confidence. |
| Mutation calibration input | optional | Must remain non-promotional for preview evidence by default. |
| Suppression ledger input | optional | Validates durable preview exceptions. |
| RIPR Zero input | optional | Keeps preview findings out of default blocking debt. |

Missing optional inputs must be reported as `unknown`, `not_available`, or a
warning. Missing language metadata for a candidate that appears to be
TypeScript, JavaScript, or Python should be treated as a policy warning or
`config_error` in policy-tightening modes, not as stable Rust evidence.

## Eligibility Matrix

Default policy answers for preview evidence:

| Question | Default answer | Notes |
| --- | --- | --- |
| Can it be shown? | yes | It should remain visible with preview and static-limit labels. |
| Can it be acknowledged? | yes, advisory | Acknowledgement is a visible PR-time state, not promotion. |
| Can it be waived? | yes, advisory | Waiver records must stay separate from durable suppressions. |
| Can it be suppressed? | yes, with metadata | Suppressed preview findings remain visible with owner and reason. |
| Can it be baselined? | advisory partition only | No default baseline-check or auto-adopt-new authority. |
| Can it be used for a gate? | no | Gate decision is advisory or not-applicable by default. |
| Can it be used for calibrated confidence? | no | Review observations can be tracked, but not mutation-calibrated confidence. |
| Can it count against RIPR Zero? | no | It may be reported as excluded preview debt. |
| Can it drive first-useful-action guidance? | yes, labeled | Guidance must preserve preview and static-limit context. |

Stable Rust evidence keeps the existing eligibility rules from
RIPR-SPEC-0014, RIPR-SPEC-0016, RIPR-SPEC-0017, and RIPR-SPEC-0029.

## Surface Rules

Policy surfaces that consume preview evidence must follow these rules:

1. Visibility comes first. Preview findings must not disappear merely because
   they are non-gating.
2. Labels are mandatory. A preview finding without `language_status =
   "preview"` is invalid for policy tightening.
3. Static limits are explicit. Unknown preview analysis should produce
   `static_limit_kind` or an equivalent warning.
4. Suppressions are durable exceptions. They require owner, reason, scope,
   review date, expected visibility, and the preview label.
5. Waivers are PR-time acknowledgements. They do not create suppressions,
   baselines, or gate confidence.
6. Baselines are adoption checkpoints. Preview baseline movement is advisory
   unless a later policy promotes a class.
7. Calibration is separated. Recommendation observations can inform future
   promotion, but imported mutation calibration cannot raise preview gate
   confidence by default.
8. Blocking is explicit. Preview evidence cannot block merely because a repo is
   in `acknowledgeable`, `baseline-check`, or `calibrated-gate` mode.

## Promotion Contract

A future spec may promote a bounded preview evidence class only by naming:

- the language and adapter status being promoted;
- the candidate class and static-limit exclusions;
- the exact policy surfaces gaining eligibility;
- fixture, golden, and dogfood receipts for that class;
- recommendation-calibration thresholds for the same class;
- any imported mutation-calibration join rules, if used;
- baseline identity and shrink-only refresh behavior;
- suppression and waiver behavior;
- generated CI posture and rollback instructions;
- proof that Rust-default behavior and existing gate semantics do not change.

Promotion must be opt-in and reversible. It must not make preview adapters
default-on, change branch protection by default, run mutation tools, generate
tests, or use runtime outcome language as static RIPR evidence.

## Reporting Shape

Reports that summarize preview policy eligibility should use bounded fields
equivalent to:

```json
{
  "preview_evidence_boundary": {
    "state": "healthy",
    "preview_languages": ["typescript", "python"],
    "preview_findings_visible": 3,
    "preview_findings_acknowledgeable": 3,
    "preview_findings_suppressible": 3,
    "preview_findings_baseline_advisory": 3,
    "preview_findings_gate_eligible": 0,
    "preview_findings_ripr_zero_blocking": 0,
    "preview_findings_calibrated_confidence": 0,
    "static_limits_required": true,
    "promotion_policy": null
  }
}
```

The exact field placement may vary by report, but the default zero counts for
gate eligibility, RIPR Zero blocking, and calibrated confidence must remain
explicit whenever preview findings are present in a policy report.

## Non-Goals

- No analyzer behavior changes.
- No LSP or editor behavior changes.
- No PR summary rendering changes.
- No generated tests.
- No mutation execution.
- No provider calls.
- No release or security changes.
- No default CI blocking.
- No automatic baseline adoption.
- No preview-language gate promotion.
- No runtime adequacy or mutation-result claims.

## Acceptance Examples

Rust-only repo:

- No preview findings are present.
- Existing Rust policy behavior is unchanged.
- Gate, baseline, calibration, and RIPR Zero reports follow their existing
  specs.

Mixed repo with TypeScript enabled:

- TypeScript findings carry `language = "typescript"` and
  `language_status = "preview"`.
- Findings are visible in advisory reports and can appear in first-useful-action
  guidance with preview context.
- Gate decisions do not mark TypeScript findings `blocking`.
- RIPR Zero status excludes TypeScript findings from default blocking debt and
  records the exclusion.

Preview finding with static limit:

- The finding carries `static_limit_kind`, such as `dynamic_dispatch` or
  `missing_import_graph`.
- The policy surface keeps the finding visible but does not treat it as
  gate-eligible or calibrated.

Preview suppression:

- A durable suppression includes identity, owner, reason, scope, created date,
  review date, expected visibility, static class, and preview label.
- The suppressed finding remains visible as suppressed with reason.

Promotion attempted by config only:

- Adding `typescript` or `python` to `[languages] enabled` makes preview
  evidence visible.
- It does not make that evidence gate-eligible, RIPR Zero blocking, or
  mutation-calibrated.

## Test Mapping

Follow-up implementation should cover:

- policy readiness fixtures with preview findings present and zero default
  gate/RIPR Zero/calibrated-confidence eligibility;
- gate decision tests showing preview candidates stay advisory or
  not-applicable across all existing gate modes;
- baseline delta tests showing preview findings are not auto-adopted into the
  default policy baseline;
- RIPR Zero tests showing preview debt is excluded from default blocking debt
  and counted separately;
- suppression-ledger tests for preview owner, reason, scope, review date,
  expected visibility, and preview label;
- recommendation-calibration tests that record preview usefulness/noise without
  gate confidence;
- static-language checks that reject runtime or adequacy claims in preview
  policy wording.

## Implementation Mapping

This spec belongs to the focused Lane 2 tracker in
[Policy readiness](../policy/POLICY_READINESS.md).

Implementation should be split into later work items:

- `report/policy-readiness` records the preview boundary health axis;
- `report/waiver-aging` keeps preview waivers visible as acknowledgements;
- `policy/suppression-ledger-health` validates preview suppressions;
- `policy/baseline-refresh-guardrails` prevents preview auto-adoption;
- `policy/exception-ledger-convergence` aligns preview exceptions with other
  ledgers;
- `docs/blocking-readiness-guide` explains when preview evidence keeps policy
  advisory;
- `ci/policy-readiness-advisory-projection` may upload preview boundary
  artifacts without pass/fail authority.

## Metrics

Future reports may emit:

- `preview_policy_findings_visible`
- `preview_policy_findings_acknowledgeable`
- `preview_policy_findings_suppressible`
- `preview_policy_findings_baseline_advisory`
- `preview_policy_findings_gate_eligible`
- `preview_policy_findings_ripr_zero_blocking`
- `preview_policy_findings_calibrated_confidence`
- `preview_policy_missing_language_status`
- `preview_policy_missing_static_limit_kind`
- `preview_policy_promotion_policy_count`
