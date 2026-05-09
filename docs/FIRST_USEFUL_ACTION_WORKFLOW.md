# First Useful Action Workflow

Use this workflow when RIPR has already produced PR, editor, ledger, proof, or
receipt artifacts and you need one bounded next action instead of another
artifact to inspect.

The first useful action is a read-only routing surface:

```text
existing RIPR artifacts
-> first useful action
-> focused test, refresh, missing artifact, acknowledgement, or no action
-> verify static movement
-> receipt
```

It does not rerun hidden analysis, edit source, generate tests, call providers,
run mutation testing, change gate policy, or make CI blocking by default.

## Start In GitHub

In generated CI, start with the `First Useful Action` job-summary section when
it exists. It is meant to answer:

- what action should happen next;
- which seam or artifact drove that action;
- why this action is first;
- where to work;
- which command verifies movement;
- which receipt proves the result;
- which warnings or fallback states limit the recommendation.

The full artifacts are:

```text
target/ripr/reports/first-useful-action.json
target/ripr/reports/first-useful-action.md
```

The GitHub summary is the first-screen view. The Markdown artifact is the
deeper reviewer view. The JSON artifact is for tools and coding agents.

If no first-action inputs exist, generated CI logs that no inputs were
available and leaves pass/fail behavior unchanged.

## Start In The Editor

In VS Code, `ripr: Show Status` and the status bar can project an existing:

```text
target/ripr/reports/first-useful-action.json
```

That projection is read-only. The editor does not invoke `ripr first-action`,
create a new report, add diagnostics, edit source, generate tests, call
providers, run mutation testing, or treat unsaved-buffer evidence as fresh.

If the report is missing, malformed, stale, or suppressed by dirty Rust buffers,
use the normal editor status and refresh workflow before treating the action as
current evidence.

## Generate Or Refresh The Report

Regenerate the report only from explicit existing inputs. Omit optional inputs
that are not available rather than inventing placeholders.

```bash
ripr first-action \
  --root . \
  --pr-guidance target/ripr/review/comments.json \
  --assistant-proof target/ripr/reports/test-oracle-assistant-proof.json \
  --ledger target/ripr/reports/pr-evidence-ledger.json \
  --baseline-delta target/ripr/reports/baseline-debt-delta.json \
  --receipt target/ripr/reports/agent-receipt.json \
  --gate-decision target/ripr/reports/gate-decision.json \
  --coverage-frontier target/ripr/reports/coverage-grip-frontier.json \
  --editor-context target/ripr/workflow/evidence-context.json \
  --out target/ripr/reports/first-useful-action.json \
  --out-md target/ripr/reports/first-useful-action.md
```

Use the command fields in the generated report for follow-up work. Do not
hand-roll a broader agent task when the report already names one seam, one
target, and one verify path.

## Read The Status

| Status | Meaning | Next action |
| --- | --- | --- |
| `actionable` | RIPR found a bounded focused-test action. | Add the focused test or hand the packet to an agent, then verify movement. |
| `stale` | A supporting artifact is older than the current evidence boundary. | Refresh the named artifact before acting. |
| `missing_required_artifact` | The first action cannot be trusted because required evidence is absent. | Generate the missing artifact named in the report. |
| `baseline_only` | The visible issue is known baseline debt rather than a PR-local action. | Keep it visible and use the baseline burn-down workflow. |
| `acknowledged` | A PR-time acknowledgement is present. | Keep the finding visible; do not treat the label as suppression. |
| `waived` | A waiver label acknowledged the candidate for this PR. | Review whether a focused test or durable policy exception should replace repeated waivers. |
| `suppressed` | Durable policy says the finding is an exception. | Inspect owner/reason metadata if the exception looks stale. |
| `no_actionable_seam` | No bounded seam action is available from current inputs. | Do not invent one; inspect warnings and deeper artifacts if needed. |
| `already_improved` | A prior attempt already moved static evidence. | Preserve the receipt and consider shrink-only baseline refresh if applicable. |
| `unchanged_after_attempt` | A focused attempt exists but static evidence did not move. | Recheck the test target, artifact freshness, and analyzer limits before retrying. |

These statuses are advisory routing states. Gate decisions, when explicitly
configured, remain the pass/fail authority.

## Act On The Action

For a developer, the preferred action is one focused test:

- target the selected seam;
- assert the missing discriminator or observation;
- imitate the related test when supplied;
- avoid unrelated refactors;
- run the report's verify command;
- emit or update the receipt.

For a reviewer, use the first action to ask for one concrete change:

```text
Please add the focused test for this seam, run the verify command, and attach
the receipt. Do not broaden the test pass beyond the selected discriminator.
```

For a coding agent, pass the bounded packet rather than a broad instruction:

```text
Use this seam, missing discriminator, related test, suggested focused test,
verify command, and receipt command. Add one focused test and return the
receipt.
```

If the report recommends refreshing evidence or generating a missing artifact,
do that before assigning a focused-test task.

## Verify Movement

Run the verify command from the first-action report or from the linked assistant
packet. The command should compare before and after static evidence and write a
machine-readable result such as:

```text
target/ripr/agent/agent-verify.json
```

Read movement conservatively:

| Movement | Meaning |
| --- | --- |
| `improved` | Static evidence strengthened for the selected seam. |
| `resolved` | The selected visible gap no longer appears under current evidence. |
| `unchanged` | Static evidence did not move; inspect the test, inputs, or analyzer limits. |
| `regressed` | Static evidence got weaker. Treat this as review evidence, not runtime mutation proof. |
| `unknown` | Required before or after evidence is missing or not comparable. |

Static movement is not runtime adequacy. Do not use mutation-runtime outcome
words unless imported runtime calibration is explicitly present.

## Emit The Receipt

Run the receipt command from the report or handoff packet after verification.
The receipt should preserve:

- selected seam identity;
- before and after evidence artifacts;
- static movement;
- warnings and stale inputs;
- next action or residual limit.

When no receipt exists, do not infer improvement from a test diff alone.

## Interpret Fallbacks

Fallback states are product signals, not noise to hide.

- `summary-only` placement means bad inline placement was avoided.
- stale inputs mean refresh before acting.
- missing artifacts mean the report cannot prove the loop yet.
- acknowledgements and waivers keep findings visible.
- suppressions are durable policy exceptions and should carry reason/owner
  metadata in their source policy.

Do not turn fallback states into default blocking. They are evidence for review
and for later dogfood receipts.

## CI And Gate Boundary

Generated CI surfaces the first useful action as advisory summary and artifact
content. The first-action report does not own pass/fail behavior.

Gate authority stays with the explicit gate decision:

```text
target/ripr/reports/gate-decision.json
target/ripr/reports/gate-decision.md
```

If `RIPR_GATE_MODE` is unset, generated CI remains advisory. If a gate mode is
configured, use the gate report to decide whether the PR is advisory,
acknowledged, blocked, or a configuration error. Use the first useful action to
repair or explain the decision.

## Related Docs

- [PR review guidance](PR_REVIEW_GUIDANCE.md) explains changed-line-safe PR
  recommendations, summary-only fallback, and inline-comment opt-in boundaries.
- [Test-oracle assistant workflow](TEST_ORACLE_ASSISTANT_WORKFLOW.md) explains
  the focused-test, verify, and receipt loop that first-action reports may
  route to.
- [Test-oracle assistant proof report](TEST_ORACLE_ASSISTANT_PROOF_REPORT.md)
  explains the proof artifact that can feed the first useful action.
- [Assistant loop health workflow](ASSISTANT_LOOP_HEALTH_WORKFLOW.md) explains
  how proof packets are summarized into completeness, movement, warning, and
  repair queue health.
- [PR evidence ledger workflow](PR_EVIDENCE_LEDGER_WORKFLOW.md) explains PR
  movement, waivers, baseline burn-down, repair receipts, and coverage/grip
  frontier signals.
- [Editor evidence workflow](EDITOR_EVIDENCE_WORKFLOW.md) explains the
  saved-workspace editor path and read-only first-action status projection.
- [Output schema](OUTPUT_SCHEMA.md) defines the
  `first-useful-action.{json,md}` contract.
- [RIPR-SPEC-0020](specs/RIPR-SPEC-0020-first-useful-action-report.md)
  defines the first useful action report contract.
