# RIPR-SPEC-0013: Calibrated Gate Policy

Status: proposed

## Problem

RIPR now exposes PR-time test-oracle gap evidence through CI summaries, check
annotations, editor actions, agent packets, and before/after receipts. That
visibility is intentionally advisory. The next product layer is an optional
gate that can answer a separate policy question:

```text
Given this PR's static evidence, configured policy, acknowledgement labels,
and optional imported runtime calibration, should this run stay advisory,
count as acknowledged, or fail?
```

This decision must not be implicit in PR guidance. Review visibility and CI
blocking are different responsibilities. A reviewer should be able to see a
high-value test-oracle gap without that automatically becoming a required
merge gate.

The gate also needs a strict evidence boundary. Static RIPR evidence can say a
changed seam appears weakly exercised or lacks a discriminator. Runtime
mutation outcomes can raise or lower confidence only when an already-produced
calibration report is supplied. The gate must not run mutation testing, infer
runtime outcomes, or use runtime-outcome wording in static-only decisions.

## Product Contract

The calibrated gate is optional policy over existing RIPR artifacts. It does
not replace the PR guidance, SARIF, badge, LSP, agent, or calibration surfaces.

The contract is:

- visibility remains advisory by default;
- blocking requires an explicit gate mode;
- acknowledgement labels produce visible acknowledged outcomes, not silent
  success;
- suppression and configured-off evidence remain visible in decision metadata;
- imported runtime calibration is confidence evidence, never a command the gate
  runs;
- static decisions use RIPR static evidence vocabulary only.

## Behavior

The planned gate evaluator is a read-only command. It consumes existing
artifacts and writes a gate decision report:

```text
ripr gate --root . \
  --review-comments target/ripr/review/comments.json \
  --repo-exposure target/ripr/pilot/repo-exposure.json \
  --policy ripr.toml \
  --labels-json target/ripr/review/labels.json \
  --out target/ripr/review/gate-decision.json
```

The command writes:

```text
target/ripr/review/gate-decision.json
target/ripr/review/gate-decision.md
```

It must not post comments, edit files, generate tests, upload SARIF, mutate
GitHub state, run cargo-mutants, or change generated workflow defaults.

Generated workflows may later run the evaluator only when explicitly configured
with a gate mode. The default generated workflow remains advisory and
non-blocking.

## Gate Modes

The gate supports these planned modes:

- `advisory` - always exits successfully and records what the stricter policy
  would have considered.
- `baseline-check` - fails only when configured baseline comparisons detect new
  policy-eligible gaps.
- `fail-on-new-high-confidence-gap` - fails on new, policy-eligible,
  high-confidence gaps when no acknowledgement label applies.
- `acknowledgeable-soft-gate` - fails on policy-eligible gaps unless an
  acknowledgement label such as `ripr-waive` is present; acknowledged runs stay
  visible as acknowledged.

Every mode must write a decision report. Failing mode must still preserve the
same JSON and Markdown evidence so reviewers can understand the decision.

## Inputs

The evaluator may read:

- PR guidance from `ripr review-comments`;
- repo exposure or pilot output;
- current and optional baseline SARIF policy reports;
- configured severity and suppressions from `ripr.toml`;
- labels supplied by CI as JSON;
- optional imported mutation calibration reports;
- optional before/after receipt or agent verify artifacts.

Missing optional inputs must degrade to advisory or unknown confidence, not
invent evidence. Missing required inputs should produce a configuration error
with a repair command.

## Decision Rules

A blocking candidate must satisfy all of these conditions:

1. The gate mode is not `advisory`.
2. The candidate comes from current PR guidance or an equivalent changed-seam
   static evidence surface.
3. The candidate is visible under configured severity and suppression policy.
4. The candidate is new relative to the selected baseline when a baseline is
   required.
5. No nearby focused test changed in the same PR.
6. No acknowledgement label applies, unless the mode is only reporting
   acknowledged state.
7. The candidate has high static confidence, or imported calibration supports
   treating it as high confidence.

High static confidence is intentionally narrow. Initial policy-eligible gaps
should be limited to changed-line `weakly_gripped`, `ungripped`, or
`reachable_unrevealed` seams with concrete focused-test guidance, such as a
missing discriminator, assertion shape, related test, or candidate value.

Unknown-stage and opaque cases may be reported, but should not block by default
until a later policy explicitly promotes them.

## Acknowledgement Labels

`ripr-waive` is the default acknowledgement label. Acknowledgement means:

- the gate decision status may be `acknowledged`;
- summaries must show which gaps were acknowledged;
- acknowledged gaps remain in JSON/Markdown outputs;
- acknowledgement does not modify suppressions, baselines, or source files;
- acknowledgement does not hide the recommendation from PR guidance.

Future labels may be added by configuration, but they must be explicit and
visible in the decision report.

## Runtime Calibration Boundary

The gate may read an existing mutation calibration report. It must not run
mutation testing or shell out to a mutation tool.

Calibration can affect confidence only when the report directly joins a runtime
record to the same static seam by `seam_id` or another unambiguous join already
accepted by the calibration importer.

Calibration effects:

- static gap plus matching runtime signal can raise confidence;
- static gap plus clean matching runtime signal can lower confidence or keep
  the candidate advisory;
- runtime signal without a matching static gap remains calibration evidence,
  not a static gate failure;
- ambiguous joins must remain advisory.

Runtime outcome labels belong in calibration report fields. Gate summaries
should describe them as imported runtime calibration evidence, not as static
RIPR conclusions.

## JSON Shape

The planned gate decision JSON uses schema version `0.1`:

```json
{
  "schema_version": "0.1",
  "tool": "ripr",
  "status": "acknowledged",
  "mode": "acknowledgeable-soft-gate",
  "root": ".",
  "inputs": {
    "review_comments": "target/ripr/review/comments.json",
    "repo_exposure": "target/ripr/pilot/repo-exposure.json",
    "policy": "ripr.toml",
    "labels": ["ripr-waive"],
    "mutation_calibration": null
  },
  "summary": {
    "evaluated": 2,
    "blocking": 0,
    "acknowledged": 1,
    "advisory": 1,
    "suppressed": 0,
    "unknown_confidence": 0
  },
  "decisions": [
    {
      "id": "ripr-gate-67fc764ba37d77bd",
      "seam_id": "67fc764ba37d77bd",
      "source": "review_comments",
      "decision": "acknowledged",
      "gate_reason": "policy-eligible gap acknowledged by ripr-waive",
      "static_class": "weakly_gripped",
      "severity": "warning",
      "placement": {
        "path": "src/pricing.rs",
        "line": 88
      },
      "policy": {
        "mode": "acknowledgeable-soft-gate",
        "acknowledgement_label": "ripr-waive",
        "threshold": "high_confidence_new_gap"
      },
      "evidence": {
        "missing_discriminator": "amount == discount_threshold",
        "suggested_test": "Add an equality-boundary test.",
        "related_test": "tests/pricing.rs::applies_discount_above_threshold",
        "nearby_test_changed": false,
        "suppressed": false,
        "calibration": {
          "available": false,
          "confidence_effect": "not_used"
        }
      }
    }
  ],
  "warnings": [],
  "limits_note": "Optional policy over static RIPR evidence; advisory by default; runtime calibration is used only when supplied."
}
```

## Field Contract

- `schema_version` - currently `"0.1"`.
- `status` - one of `pass`, `advisory`, `acknowledged`, `fail`, or
  `config_error`.
- `mode` - one of the configured gate modes.
- `inputs` - paths and labels used by the decision. Missing optional inputs
  should be visible.
- `summary.evaluated` - number of candidate recommendations considered.
- `summary.blocking` - number of decisions that made the gate fail.
- `summary.acknowledged` - number of decisions made non-failing by an
  acknowledgement label.
- `summary.advisory` - number of non-blocking visible decisions.
- `summary.suppressed` - number of candidates hidden by configured policy.
- `summary.unknown_confidence` - number of candidates that could not satisfy
  confidence requirements.
- `decisions[].decision` - `blocking`, `acknowledged`, `advisory`,
  `suppressed`, or `not_applicable`.
- `decisions[].gate_reason` - short policy explanation.
- `decisions[].static_class` - static seam or finding class from RIPR output.
- `decisions[].policy` - mode, threshold, and acknowledgement fields that
  affected the result.
- `decisions[].evidence` - static evidence, nearby-test state, suppression
  state, and optional calibration confidence effect.
- `warnings[]` - missing inputs, unsupported labels, ambiguous calibration, or
  baseline limitations.
- `limits_note` - static/runtime and advisory-default boundary text.

## CI Projection

Generated workflows should not run the gate by default. A future generated
workflow may opt in with an explicit setting such as:

```yaml
env:
  RIPR_GATE_MODE: acknowledgeable-soft-gate
```

When configured, the workflow should:

- run existing advisory evidence producers first;
- run the gate evaluator after PR guidance exists;
- append the gate decision summary to `$GITHUB_STEP_SUMMARY`;
- upload gate decision JSON and Markdown;
- fail only when the evaluator exits with a blocking decision in an explicit
  blocking mode.

The workflow must preserve the existing PR guidance, SARIF, badge, and artifact
uploads even when the gate fails.

## Required Evidence

The implementation campaign must add:

- a spec-pinned gate decision schema;
- a read-only evaluator with JSON and Markdown output;
- tests for every gate mode;
- tests for acknowledgement labels and visible acknowledged decisions;
- tests for suppression and configured-off behavior;
- tests for missing optional inputs;
- fixtures for baseline, fail-on-new-high-confidence-gap, acknowledged,
  advisory, suppressed, and calibration agreement/disagreement cases;
- generated workflow tests proving advisory defaults remain unchanged;
- docs explaining visibility versus gating and static/runtime boundaries.

## Non-Goals

Calibrated gates must not:

- change default generated workflow posture from advisory to blocking;
- fail on every visible RIPR recommendation;
- run mutation testing;
- infer runtime outcomes from static evidence;
- hide acknowledged or waived gaps from summaries;
- post inline comments;
- edit source files;
- generate tests;
- change SARIF, badge, PR guidance, or calibration schemas without an explicit
  compatibility note;
- split the public crate surface.

## Acceptance Examples

- With no gate mode configured, generated CI remains advisory even when PR
  guidance contains policy-eligible gaps.
- In `acknowledgeable-soft-gate`, a policy-eligible gap without `ripr-waive`
  produces a failing decision and still writes JSON/Markdown evidence.
- In `acknowledgeable-soft-gate`, the same gap with `ripr-waive` produces an
  acknowledged decision that remains visible in summaries.
- In `fail-on-new-high-confidence-gap`, a changed-line `weakly_gripped` seam
  with missing discriminator and no nearby test change can block when no
  acknowledgement applies.
- A configured-off or suppressed seam does not block and is counted as
  suppressed or not applicable.
- A runtime calibration report with an ambiguous join does not raise confidence
  enough to block.
- A runtime calibration report is optional; missing calibration does not invent
  runtime confidence.

## Test Mapping

Initial implementation should add tests for:

- gate mode parsing and default advisory behavior;
- decision JSON and Markdown rendering;
- acknowledgement label handling;
- baseline comparison behavior;
- high-confidence candidate filtering;
- configured severity and suppression behavior;
- missing and malformed input reports;
- calibration agreement, disagreement, and ambiguous join handling;
- generated workflow opt-in wiring.

## Implementation Mapping

The first implementation should map this spec to:

- a CLI adapter for `ripr gate`;
- an app/use-case module that reads existing PR guidance, repo exposure, SARIF
  policy, suppressions, labels, and optional calibration reports;
- an output module for gate decision JSON/Markdown;
- fixture expectations under the boundary-gap corpus;
- generated workflow opt-in wiring only after the pure evaluator is
  fixture-backed.

## Metrics

- `gate_decisions`
- `gate_blocking`
- `gate_acknowledged`
- `gate_advisory`
- `gate_suppressed`
- `gate_unknown_confidence`
