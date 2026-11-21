# PR Review Front Panel Workflow

Use this workflow when generated CI has produced multiple RIPR artifacts and a
reviewer needs one first-screen PR story instead of artifact archaeology.

The PR review front panel is a read-only projection over explicit existing
artifacts:

```text
PR guidance
-> first useful action
-> assistant proof and assistant-loop health
-> PR ledger, baseline, gate, calibration, coverage/grip, and receipts
-> PR review front panel
```

It writes:

```text
target/ripr/reports/pr-review-front-panel.json
target/ripr/reports/pr-review-front-panel.md
```

It does not rerun hidden analysis, inspect source to infer missing fields, edit
source, generate tests, call providers, run mutation testing, publish inline PR
comments, change analyzer or ranking behavior, change gate policy, or make CI
blocking by default.

## Start In GitHub

In generated CI, start with the `PR review front panel` section in the RIPR
advisory job summary when it exists. It is the first-screen reviewer surface.

The section should answer:

- what issue or no-action state matters most;
- whether the issue is changed-line-placeable or summary-only;
- whether the issue is baseline debt, new policy-eligible debt, acknowledged,
  waived, suppressed, blocked, or only advisory;
- which missing discriminator or static limit explains the result;
- which focused test, related test, agent command, or verify command is next;
- whether static evidence already moved;
- where the receipt and full artifact packet live;
- which warnings limit the report.

The Markdown artifact is the deeper reviewer view. The JSON artifact is for
generated CI, portfolio reports, and coding agents.

If no front-panel inputs exist, generated CI logs that no inputs were available
and leaves pass/fail behavior unchanged.

## Generate Or Refresh The Report

Generate the report only from explicit existing inputs. Omit optional inputs
that are not available rather than inventing placeholders.

```bash
ripr pr-review front-panel \
  --root . \
  --pr-guidance target/ripr/review/comments.json \
  --first-action target/ripr/reports/first-useful-action.json \
  --assistant-proof target/ripr/reports/test-oracle-assistant-proof.json \
  --assistant-health target/ripr/reports/assistant-loop-health.json \
  --ledger target/ripr/reports/pr-evidence-ledger.json \
  --baseline-delta target/ripr/reports/baseline-debt-delta.json \
  --zero-status target/ripr/reports/ripr-zero-status.json \
  --gate-decision target/ripr/reports/gate-decision.json \
  --recommendation-calibration target/ripr/reports/recommendation-calibration.json \
  --mutation-calibration target/ripr/reports/mutation-calibration.json \
  --coverage-frontier target/ripr/reports/coverage-grip-frontier.json \
  --receipt target/ripr/reports/agent-receipt.json \
  --out target/ripr/reports/pr-review-front-panel.json \
  --out-md target/ripr/reports/pr-review-front-panel.md
```

At least one explicit input is required. The producer should not discover
artifacts by scanning the workspace; generated CI decides which paths are
present and passes them deliberately.

## Read The First Screen

The at-a-glance fields are intentionally compact.

| Field | Meaning | Usual action |
| --- | --- | --- |
| `status` | Advisory report state, or a supplied gate decision such as `pass`, `acknowledged`, `blocked`, or `config_error`. | Use it to orient review; pass/fail authority still belongs to the gate decision. |
| `headline` | One human-facing sentence from the best available evidence. | Treat it as the summary, then verify details below. |
| `top_issue_state` | The selected route: `actionable`, `summary_only`, `baseline_only`, `already_improved`, `unchanged_after_attempt`, `no_actionable_seam`, `missing_required_input`, or `stale_input`. | Follow the route-specific repair guidance. |
| `policy_state` | Policy context such as `new_policy_eligible`, `baseline`, `acknowledged`, `waived`, `suppressed`, `blocking`, or `config_error`. | Keep findings visible; do not treat acknowledgement or suppression as missing evidence. |
| `placement` | Whether guidance is changed-line-placeable, summary-only, or unavailable. | Prefer summary-only fallback over a misleading inline location. |
| `movement_state` | Static before/after movement when supplied by proof, receipt, ledger, or baseline artifacts. | Use as static review evidence, not runtime adequacy. |
| `coverage_grip_state` | Optional coverage/grip frontier context. | Use it to explain why coverage may stay flat while grip improves. |

The count fields explain the PR debt story:

- new policy-eligible gaps;
- baseline gaps still present;
- baseline gaps resolved;
- acknowledged or waived gaps;
- suppressions;
- blocking candidates;
- warning count.

Open raw artifacts only when the summary raises a warning, a reviewer needs
source provenance, or a follow-up agent needs JSON.

## Reviewer Workflow

For review, ask for one concrete next step:

```text
Please follow the PR review front panel route: add the focused test or refresh
the named artifact, run the verify command, and keep the receipt with the PR.
```

Use the front panel to avoid broad requests such as "add more tests." It should
name the seam, missing discriminator, suggested focused test, related test,
verify command, and receipt path when those inputs are available.

When the panel says `summary_only`, keep the recommendation in the summary. Do
not force an inline annotation or durable PR comment onto an unsafe line.

When the panel says `baseline_only`, keep the debt visible and use the baseline
burn-down workflow. Do not ask the author to repair unrelated baseline debt in
the current PR unless that is the stated scope.

When the panel says `missing_required_input` or `stale_input`, ask for the
named report to be regenerated before requesting a focused test.

## Developer Workflow

For development, follow the smallest repair route:

- open the selected file and related test when supplied;
- add one focused assertion or observation for the missing discriminator;
- avoid unrelated production or test refactors;
- run the verify command from the front panel or linked handoff packet;
- write or refresh the receipt artifact;
- keep unchanged or regressed movement visible instead of hiding it.

If the report shows `already_improved` or `resolved`, preserve the receipt and
consider shrink-only baseline refresh when the PR also owns baseline cleanup.

If the report shows `unchanged_after_attempt`, inspect whether the test missed
the discriminator, the artifacts are stale, or the current static model cannot
see the new evidence. Do not claim success from a test diff alone.

## Coding Agent Workflow

The front panel is the bounded task packet for an external coding agent. A good
agent handoff is:

```text
Use the top issue from pr-review-front-panel.md.
Target this seam and missing discriminator only.
Imitate the related test when present.
Run the verify command.
Return the receipt and updated front-panel report.
Stop after the focused repair.
```

Include these fields when present:

- seam ID, file, and line;
- missing discriminator;
- suggested focused test;
- related test;
- agent command;
- verify command;
- receipt path;
- warning list.

If those fields are absent, keep the work in human review or regenerate the
missing inputs before assigning an agent.

## Maintainer Workflow

For maintainers, the front panel is a PR-local operating view over debt and
policy state.

Use it to distinguish:

- new policy-eligible debt from old baseline debt;
- visible acknowledgement from durable suppression;
- resolved baseline entries from new debt;
- configured gate decisions from advisory projection;
- static movement from runtime mutation outcomes.

`ripr-waive` remains a visible acknowledgement. It does not hide a finding.
Suppressions are durable policy exceptions and should carry reason and owner
metadata in their source policy.

When a PR resolves baseline debt, keep the receipt and use shrink-only baseline
refresh. Do not adopt new debt as part of a shrink-only refresh.

## Repair Routes

| Route | Meaning | Next action |
| --- | --- | --- |
| `actionable` | A bounded focused-test repair exists. | Add the focused test or hand the packet to an agent, then verify movement. |
| `summary_only` | Guidance exists but changed-line placement is unsafe. | Keep it in the summary; do not force inline placement. |
| `baseline_only` | The issue is known baseline debt. | Keep it visible and use baseline burn-down, not PR-local blocking by default. |
| `already_improved` | Existing receipt or movement evidence shows improvement. | Preserve the receipt; consider shrink-only baseline cleanup. |
| `unchanged_after_attempt` | A repair attempt exists but static evidence did not move. | Inspect test target, artifact freshness, and static limits before retrying. |
| `no_actionable_seam` | No safe current action exists. | Do not invent a repair; inspect warnings and deeper artifacts if needed. |
| `missing_required_input` | A necessary artifact is missing or invalid. | Regenerate the named artifact and rerun the report. |
| `stale_input` | Evidence is too old for the current review boundary. | Refresh the stale artifact first. |
| `blocked` | A supplied explicit gate decision blocked. | Follow the gate report's repair or acknowledgement path. |
| `config_error` | A supplied explicit gate decision reported bad configuration. | Fix the gate configuration or missing baseline input before treating the run as evidence. |

## Inspect Receipts

A receipt should preserve:

- selected seam identity;
- before and after evidence artifacts;
- static movement;
- verify command;
- warning and stale-input state;
- residual next action or limit.

The receipt proves what static evidence moved. It does not prove runtime
adequacy and must not use mutation-runtime vocabulary unless imported runtime
calibration is explicitly present in a separate artifact.

## CI And Gate Boundary

Generated CI surfaces the PR review front panel as advisory summary and
artifact content. The front panel does not own pass/fail behavior.

Gate authority stays with the explicit gate decision:

```text
target/ripr/reports/gate-decision.json
target/ripr/reports/gate-decision.md
```

If `RIPR_GATE_MODE` is unset, generated CI remains advisory. If a gate mode is
configured, use the gate report to decide whether the PR is advisory,
acknowledged, blocked, or a configuration error. Use the front panel to explain
the decision and route the repair.

## Related Docs

- [PR review guidance](PR_REVIEW_GUIDANCE.md) explains changed-line-safe PR
  recommendations, summary-only fallback, and inline-comment opt-in boundaries.
- [First useful action workflow](FIRST_USEFUL_ACTION_WORKFLOW.md) explains the
  one-action routing layer that may feed the front panel.
- [Test-oracle assistant proof report](TEST_ORACLE_ASSISTANT_PROOF_REPORT.md)
  explains the focused-test receipt artifact that may feed the front panel.
- [Assistant loop health workflow](ASSISTANT_LOOP_HEALTH_WORKFLOW.md) explains
  proof completeness, missing-input repair, warning groups, and repair queues.
- [PR evidence ledger workflow](PR_EVIDENCE_LEDGER_WORKFLOW.md) explains PR
  movement, waivers, baseline burn-down, repair receipts, and coverage/grip
  frontier signals.
- [Baseline ledger workflow](BASELINE_LEDGER_WORKFLOW.md) explains reviewed
  baseline debt, shrink-only refresh, and baseline-check adoption.
- [RIPR Zero reporting workflow](RIPR_ZERO_REPORTING_WORKFLOW.md) explains how
  baseline delta and repair routes become progress toward RIPR 0.
- [Calibrated gate policy](CALIBRATED_GATE_POLICY.md) explains configured gate
  modes and why generated CI remains advisory by default.
- [CI strategy](CI.md#generated-github-workflow) describes generated workflow
  projection into GitHub summaries and artifacts.
- [Output schema](OUTPUT_SCHEMA.md#pr-review-front-panel-report) defines the
  JSON and Markdown contract.
- [RIPR-SPEC-0023](specs/RIPR-SPEC-0023-pr-review-front-panel-report.md)
  defines the PR review front-panel report contract.
