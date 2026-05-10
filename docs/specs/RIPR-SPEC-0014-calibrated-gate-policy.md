# RIPR-SPEC-0014: Calibrated Gate Policy

Status: proposed

## Problem

RIPR now exposes PR-time test-oracle evidence through editor actions, agent
briefs and packets, before/after verification, receipts, generated CI
summaries, check annotations, SARIF, badges, and recommendation calibration.
Those surfaces answer the visibility question:

```text
Which changed seam appears weakly gripped, and what focused test would help?
```

They do not yet answer the policy question:

```text
Given this PR's existing static evidence, configured policy, labels,
receipts, and optional imported calibration, should the result remain advisory,
be recorded as acknowledged, or block under an explicit gate mode?
```

Visibility and enforcement must stay separate. A reviewer should be able to see
a high-value test-oracle gap without automatically making that recommendation
a merge gate. A gate should exist only when a repository explicitly configures
one, and it should fail only for narrow, high-confidence, new gaps.

The policy layer also needs a strict runtime boundary. Static RIPR evidence can
say a changed seam appears weakly exercised or lacks a discriminator. Imported
runtime mutation calibration can only adjust confidence when a pre-existing
calibration artifact joins runtime evidence to the same static seam. The gate
must not run mutation testing, infer runtime outcomes, or use runtime outcome
language as static proof.

## Product Contract

The calibrated gate is optional policy over existing RIPR artifacts. It does
not replace PR guidance, SARIF, badges, LSP diagnostics, agent packets,
receipts, operator cockpit reports, or calibration reports.

The contract is:

- generated workflows stay advisory and non-blocking by default;
- blocking requires an explicit gate mode;
- acknowledgement labels produce visible `acknowledged` decisions, not silent
  success;
- configured-off and suppressed evidence cannot block and must remain visible
  in decision metadata when present in inputs;
- imported runtime mutation calibration is confidence evidence only; the gate
  never runs a mutation tool;
- static decisions use RIPR static evidence vocabulary and never claim runtime
  adequacy;
- every gate run writes deterministic JSON and Markdown before returning a
  non-zero exit code for a blocking decision.

## Behavior

The evaluator is a read-only command:

```text
ripr gate evaluate --root . \
  --repo-exposure target/ripr/reports/repo-exposure.json \
  --pr-guidance target/ripr/review/comments.json \
  --sarif-policy target/ripr/reports/sarif-policy.json \
  --labels-json target/ci/labels.json \
  --agent-verify target/ripr/workflow/agent-verify.json \
  --agent-receipt target/ripr/reports/agent-receipt.json \
  --recommendation-calibration target/ripr/reports/recommendation-calibration.json \
  --mutation-calibration target/ripr/reports/mutation-calibration.json \
  --mode visible-only \
  --out target/ripr/reports/gate-decision.json \
  --out-md target/ripr/reports/gate-decision.md
```

The command writes:

```text
target/ripr/reports/gate-decision.json
target/ripr/reports/gate-decision.md
```

The evaluator must not:

- post GitHub comments;
- edit source files;
- generate tests;
- upload SARIF;
- mutate GitHub state;
- run cargo-mutants or any mutation engine;
- rerun analysis unless a later explicit refresh mode is specified;
- change generated workflow defaults.

Generated workflows may later run the evaluator only when explicitly
configured. The default generated workflow remains advisory and non-blocking.

## Inputs

The evaluator may read these existing artifacts:

- repo exposure JSON;
- PR guidance JSON from `ripr review-comments`;
- SARIF policy JSON;
- repository config and suppressions;
- CI labels JSON;
- agent verify JSON;
- agent receipt JSON;
- recommendation calibration JSON;
- optional imported mutation calibration JSON;
- optional baseline artifacts for baseline-aware modes.

Missing optional inputs must remain visible as warnings, omitted input fields,
or unknown confidence. They must not invent evidence. Missing required inputs
for a configured mode should produce `config_error` with a repair command in
the Markdown output.

The first implementation should treat `repo-exposure` and `pr-guidance` as the
primary current evidence inputs. SARIF policy, receipts, recommendation
calibration, mutation calibration, and baseline artifacts refine the decision
only when present.

## Gate Modes

The policy vocabulary is:

- `visible-only` - writes a decision report and exits successfully. This is the
  default generated-workflow posture.
- `acknowledgeable` - records policy-eligible gaps as acknowledged when a
  configured acknowledgement label applies; otherwise it can fail.
- `baseline-check` - can fail only on policy-eligible gaps that are new or
  materially stronger than an explicitly supplied baseline.
- `calibrated-gate` - can fail only on new, policy-eligible, high-confidence
  gaps after recommendation calibration and optional mutation calibration
  evidence are applied.

Every mode writes the same JSON and Markdown surfaces. Failing modes still
preserve all evidence so reviewers can see what blocked, what was
acknowledged, what stayed advisory, and why.

`visible-only` must never fail because of RIPR evidence. It may still return
`config_error` for malformed command-line arguments, unreadable required files,
or invalid policy values.

## Labels And Acknowledgement

`ripr-waive` is the default acknowledgement label. It means:

- the top-level decision may become `acknowledged`;
- the matching gaps remain visible in JSON and Markdown;
- the report names the label that changed the decision;
- the label does not modify suppressions, baselines, SARIF, badges, or source
  files;
- the label does not hide the PR guidance recommendation.

Future acknowledgement labels may be configurable, but every configured label
must be explicit in the decision report.

`ripr-waive` does not override `visible-only` into blocking behavior. `full-ci`
or an equivalent future exhaustive label may opt into stricter modes only when
the generated workflow explicitly maps that label to a gate mode.

## Candidate Selection

A policy-eligible candidate must satisfy all of these conditions:

1. The candidate comes from current PR guidance, repo exposure, or an equivalent
   changed-seam static evidence surface.
2. The candidate is visible under configured severity and suppression policy.
3. The candidate is tied to a changed seam, changed owner function, or accepted
   PR-guidance changed-line fallback.
4. The candidate class is `weakly_gripped`, `ungripped`, or
   `reachable_unrevealed`.
5. The candidate includes concrete focused-test guidance such as a missing
   discriminator, assertion shape, candidate value, related test, or
   recommended test target.
6. No nearby focused test changed in the same PR.

Unknown-stage and opaque classes may remain visible as advisory decisions, but
they must not block in the initial policy.

Configured-off candidates cannot block. Suppressed candidates cannot block.
Candidates with ambiguous placement or missing required evidence should remain
advisory unless a future spec explicitly promotes that case.

## Baseline Comparison

`baseline-check` and `calibrated-gate` may compare current candidates against a
baseline artifact. The baseline must be explicit. The evaluator must record the
baseline path and type it used.

A candidate is new when no baseline record has the same stable identity with
the same or stronger policy significance.

Stable identity comparison order:

1. `canonical_gap_id` when supplied directly, through
   `identity.canonical_gap_id`, or through
   `evidence_record.canonical_gap_id`;
2. `seam_id` when present;
3. PR guidance item ID or dedupe key;
4. finding ID or probe ID;
5. normalized repo-relative path, line, and source class.

Canonical gap identity is semantic evidence identity, not a new policy mode.
When present it lets reviewed baseline debt survive line movement and ordinary
refactors without making generated workflows blocking by default.

If the baseline is required but missing, unreadable, malformed, or produced by
an incompatible schema, the evaluator returns `config_error`. It must not guess
a baseline from the current artifact.

## Calibration Boundary

Recommendation calibration can adjust policy confidence when it names the same
PR guidance item, seam, target, suppression behavior, or static movement. It is
review-quality evidence: useful, noisy, wrong-line, already-covered,
wrong-target, summary-only-correct, suppressed-correctly, or unknown.

Mutation calibration can adjust policy confidence only when an existing
calibration report directly joins a runtime record to the same static seam.
The preferred join is `seam_id`. A fallback file/line join is usable only when
the calibration report records it as unambiguous. Ambiguous file/line matches,
unmatched runtime records, and runtime-only signal sections must not raise gate
confidence.

Calibration effects:

- static gap plus matching recommendation-calibration support can keep or raise
  confidence;
- static gap plus matching recommendation-calibration noise can keep the
  candidate advisory;
- static gap plus unambiguous mutation-calibration support can raise
  confidence;
- static gap plus unambiguous mutation-calibration non-gap evidence can lower
  confidence or keep the candidate advisory;
- runtime signal without a matching static candidate is calibration evidence,
  not a gate failure.

Gate summaries should describe runtime inputs as imported calibration evidence.
Runtime outcome labels stay inside calibration fields and must not become static
RIPR conclusions.

## Decisions

Top-level decision values are:

- `pass` - a non-advisory mode evaluated successfully and found no visible
  advisory, acknowledged, blocking, suppressed, or unknown-confidence decision.
- `advisory` - evidence is visible but not blocking.
- `acknowledged` - at least one policy-eligible candidate was acknowledged by a
  configured label and no candidate blocked.
- `blocked` - at least one policy-eligible candidate blocks under the explicit
  mode.
- `config_error` - required input or policy configuration is invalid.

Per-candidate decision values are:

- `blocking`
- `acknowledged`
- `advisory`
- `suppressed`
- `not_applicable`

Top-level decision derivation order:

1. `config_error` when required inputs or policy are invalid.
2. `blocked` when any candidate decision is `blocking`.
3. `acknowledged` when no candidate blocks and at least one candidate is
   `acknowledged`.
4. `advisory` when mode is `visible-only`, any candidate is advisory, any
   candidate has unknown confidence, or any optional input limitation matters.
5. `pass` when a non-advisory mode evaluated successfully and nothing remained
   visible, acknowledged, blocking, suppressed, or unknown-confidence.

The evaluator may return a non-zero exit code only for `blocked` or
`config_error`.

## JSON Shape

The gate decision JSON uses schema version `0.1`:

```json
{
  "schema_version": "0.1",
  "tool": "ripr",
  "status": "acknowledged",
  "mode": "acknowledgeable",
  "root": ".",
  "inputs": {
    "repo_exposure": "target/ripr/reports/repo-exposure.json",
    "pr_guidance": "target/ripr/review/comments.json",
    "sarif_policy": "target/ripr/reports/sarif-policy.json",
    "labels_json": "target/ci/labels.json",
    "labels": ["ripr-waive"],
    "agent_verify": "target/ripr/workflow/agent-verify.json",
    "agent_receipt": "target/ripr/reports/agent-receipt.json",
    "recommendation_calibration": "target/ripr/reports/recommendation-calibration.json",
    "mutation_calibration": null,
    "baseline": null
  },
  "policy": {
    "mode": "acknowledgeable",
    "threshold": "high_confidence_new_gap",
    "acknowledgement_labels": ["ripr-waive"],
    "default_workflow_posture": "advisory"
  },
  "summary": {
    "evaluated": 2,
    "blocking": 0,
    "acknowledged": 1,
    "advisory": 1,
    "suppressed": 0,
    "not_applicable": 0,
    "unknown_confidence": 0
  },
  "decisions": [
    {
      "id": "ripr-gate-67fc764ba37d77bd",
      "source": "pr_guidance",
      "decision": "acknowledged",
      "gate_reason": "policy-eligible gap acknowledged by ripr-waive",
      "seam_id": "67fc764ba37d77bd",
      "source_id": "ripr-review-67fc764ba37d77bd",
      "static_class": "weakly_gripped",
      "severity": "warning",
      "placement": {
        "path": "src/pricing.rs",
        "line": 88
      },
      "policy": {
        "mode": "acknowledgeable",
        "threshold": "high_confidence_new_gap",
        "acknowledgement_label": "ripr-waive",
        "baseline_identity": null
      },
      "evidence": {
        "missing_discriminator": "amount == discount_threshold",
        "assertion_shape": "Assert the returned discount behavior directly.",
        "candidate_values": ["amount == discount_threshold"],
        "recommended_test": "tests/pricing.rs::discounted_total_boundary",
        "nearby_test_changed": false,
        "suppressed": false,
        "configured_off": false,
        "recommendation_calibration": {
          "available": true,
          "outcome": "useful",
          "confidence_effect": "supports_static_gap"
        },
        "mutation_calibration": {
          "available": false,
          "confidence_effect": "not_used"
        }
      }
    }
  ],
  "warnings": [],
  "limits_note": "Optional policy over static RIPR evidence; advisory by default; runtime mutation calibration is used only when supplied."
}
```

## Field Contract

- `schema_version` - currently `"0.1"`.
- `status` - one of `pass`, `advisory`, `acknowledged`, `blocked`, or
  `config_error`.
- `mode` - one of `visible-only`, `acknowledgeable`, `baseline-check`, or
  `calibrated-gate`.
- `inputs` - normalized paths and labels used by the evaluator. Optional inputs
  should appear as `null` or produce a warning when they are absent.
- `policy.mode` - effective gate mode after config and CLI precedence.
- `policy.threshold` - initially `high_confidence_new_gap`.
- `policy.acknowledgement_labels` - configured labels that can turn a blocking
  candidate into a visible acknowledged decision.
- `policy.default_workflow_posture` - must remain `advisory` for generated
  workflows unless a later explicit configuration changes it.
- `summary.evaluated` - candidate count considered after parsing inputs.
- `summary.blocking` - count of candidate decisions that make the gate fail.
- `summary.acknowledged` - count of candidate decisions made non-failing by an
  acknowledgement label.
- `summary.advisory` - count of visible non-blocking decisions.
- `summary.suppressed` - count of suppressed or configured-hidden candidates
  preserved in the gate report.
- `summary.not_applicable` - count of parsed records that are outside the
  configured policy scope.
- `summary.unknown_confidence` - count of candidates that could not satisfy
  high-confidence requirements.
- `decisions[].source` - source artifact family such as `pr_guidance`,
  `repo_exposure`, `sarif_policy`, or `agent_receipt`.
- `decisions[].decision` - one of `blocking`, `acknowledged`, `advisory`,
  `suppressed`, or `not_applicable`.
- `decisions[].gate_reason` - short policy explanation for human summaries.
- `decisions[].static_class` - source static class copied without rewriting
  seam-grip classes into finding classes.
- `decisions[].severity` - configured severity from the source surface.
- `decisions[].policy` - mode, threshold, acknowledgement, and baseline facts
  that affected the candidate.
- `decisions[].evidence` - static evidence and optional calibration confidence
  effects used for the candidate.
- `warnings[]` - missing optional inputs, unsupported labels, ambiguous
  calibration, baseline limitations, or schema limitations.
- `limits_note` - static/runtime and advisory-default boundary text.

## Markdown Shape

The Markdown report should be compact enough for a job summary:

```text
# RIPR Gate Decision

Decision: acknowledged
Mode: acknowledgeable
Evaluated: 2
Blocking: 0
Acknowledged: 1
Advisory: 1

## Acknowledged

- src/pricing.rs:88 weakly_gripped
  Reason: policy-eligible gap acknowledged by ripr-waive
  Evidence: missing discriminator amount == discount_threshold
  Action: keep the acknowledgement visible or add the focused test and rerun
  `ripr agent verify`.

## Limits

Optional policy over static RIPR evidence. Generated workflows are advisory by
default. Runtime mutation calibration is imported confidence evidence only.
```

Markdown must not hide acknowledged decisions. If the evaluator exits with a
blocking code, the Markdown still needs enough detail for the next agent or
reviewer to resolve the state.

## CI Projection

Generated workflows should not run the gate by default. A future generated
workflow may opt in with explicit configuration, for example:

```yaml
env:
  RIPR_GATE_MODE: acknowledgeable
```

When configured, the workflow should:

- run existing advisory evidence producers first;
- run the gate evaluator after PR guidance and repo exposure exist;
- append the gate decision summary to `$GITHUB_STEP_SUMMARY`;
- upload `target/ripr/reports/gate-decision.json`;
- upload `target/ripr/reports/gate-decision.md`;
- fail only when the evaluator returns a blocking decision in an explicit
  blocking mode.

The workflow must preserve existing PR guidance, SARIF, badge, agent, cockpit,
and calibration artifacts even when the gate blocks.

## Required Evidence

Implementation must add:

- a spec-pinned gate decision schema in `docs/OUTPUT_SCHEMA.md`;
- a read-only evaluator that writes JSON and Markdown;
- tests for every gate mode;
- tests for acknowledgement labels and visible acknowledged decisions;
- tests for suppression and configured-off behavior;
- tests for missing optional inputs and required-input config errors;
- fixture cases for advisory, acknowledged, baseline-check,
  fail-on-new-high-confidence-gap, suppression, missing-input, and calibration
  agreement or disagreement;
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
- change SARIF, badge, PR guidance, recommendation calibration, mutation
  calibration, or agent receipt schemas without an explicit compatibility
  note;
- split the public crate surface.

## Acceptance Examples

- With no gate mode configured, generated CI remains advisory even when PR
  guidance contains policy-eligible gaps.
- In `acknowledgeable`, a policy-eligible gap without `ripr-waive` can produce
  a blocking decision and still writes JSON/Markdown evidence.
- In `acknowledgeable`, the same gap with `ripr-waive` produces an
  acknowledged decision that remains visible in summaries.
- In `baseline-check`, a candidate already present in the baseline with the
  same or stronger policy significance does not block.
- In `calibrated-gate`, a changed-line `weakly_gripped` seam with missing
  discriminator, no nearby test change, and supporting calibration can block
  when no acknowledgement applies.
- A configured-off or suppressed seam does not block and is counted as
  suppressed or not applicable.
- Ambiguous mutation calibration does not raise confidence enough to block.
- Missing optional mutation calibration does not invent runtime confidence.

## Test Mapping

Initial implementation should add tests for:

- gate mode parsing and default `visible-only` behavior;
- decision JSON and Markdown rendering;
- acknowledgement label handling;
- baseline comparison behavior;
- high-confidence candidate filtering;
- configured severity and suppression behavior;
- missing and malformed input reports;
- recommendation calibration agreement and disagreement;
- mutation calibration agreement, disagreement, and ambiguous join handling;
- generated workflow opt-in wiring.

## Implementation Mapping

The implementation should map this spec to:

- a CLI adapter for `ripr gate evaluate`;
- an app/use-case module that reads existing repo exposure, PR guidance, SARIF
  policy, suppressions, labels, receipts, recommendation calibration, and
  optional mutation calibration reports;
- an output module for gate decision JSON and Markdown;
- fixture expectations under the boundary-gap corpus;
- generated workflow opt-in wiring only after the pure evaluator is
  fixture-backed.

## Metrics

- `gate_decisions_evaluated`
- `gate_decisions_blocking`
- `gate_decisions_acknowledged`
- `gate_decisions_advisory`
- `gate_decisions_suppressed`
- `gate_decisions_unknown_confidence`
- `gate_config_errors`
