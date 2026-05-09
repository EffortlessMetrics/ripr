# First Useful Action Workflow

Use this workflow when RIPR has already produced PR, editor, ledger, proof,
receipt, or gate artifacts and you want one next test action instead of a list
of reports to inspect.

`ripr first-action` is a read-only router over existing artifacts. It writes:

```text
target/ripr/reports/first-useful-action.json
target/ripr/reports/first-useful-action.md
```

The report answers:

```text
What should a developer, reviewer, or coding agent do next?
Why is that the first action?
Where should the focused test go?
How should static movement be verified?
Which receipt should be returned?
What fallback state or limit applies?
```

It does not rerun hidden analysis, edit source, generate tests, call a
provider, run mutation testing, invent policy, or change default CI blocking.

## Generate The Report

Generated GitHub CI runs `ripr first-action` when at least one explicit
upstream RIPR artifact exists. It uploads the JSON and Markdown with the normal
`ripr-reports` packet and appends the Markdown to the job summary.

Run the same router locally with whichever artifacts exist:

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

Do not supply paths you do not have. Missing optional inputs are acceptable;
missing required inputs produce an explicit fallback state instead of a
speculative test request.

## Read The First Screen

Start with `target/ripr/reports/first-useful-action.md` or the First Useful
Action section in the GitHub job summary.

Read the first fields in order:

| Field | Meaning | Review action |
| --- | --- | --- |
| `Status` | The bounded route, such as `actionable`, `stale`, or `unchanged_after_attempt`. | Decide whether to write a test, refresh evidence, inspect a receipt, or take no action. |
| `Audience` | The primary consumer: `developer`, `reviewer`, or `agent`. | Use the wording and commands for that handoff. |
| `Action` | The concrete action kind, such as `write_focused_test` or `refresh_evidence`. | Do not broaden the task beyond this route. |
| `Next` | The short first-screen instruction. | Use this as the PR summary or agent handoff headline. |
| `Why First` | Deterministic routing reasons. | Confirm the route came from existing evidence rather than an opaque score. |
| `Where` | Test file, related test, and suggested test name when known. | Keep the new work near the named pattern. |
| `Verify` | Static after-evidence comparison command when known. | Run this after the focused test. |
| `Receipt` | Receipt command or receipt path when known. | Return this to the reviewer. |
| `Fallback` | Reason the report did not emit a test action. | Repair the artifact chain or keep the item visible. |
| `Limits` | Static and advisory boundaries. | Do not turn the report into runtime adequacy or policy authority. |

If a field is absent, treat that absence as part of the route. Do not infer a
target file, receipt, or verification path from memory or chat history.

## Act On Status

Use the status as the boundary for the next step:

| Status | Meaning | Next step |
| --- | --- | --- |
| `actionable` | A fresh PR-local seam has enough existing evidence for one focused test action. | Write one focused test for the missing discriminator, then verify and emit a receipt. |
| `stale` | Available evidence is older than the saved workspace or selected context. | Refresh RIPR evidence before selecting a test action. |
| `missing_required_artifact` | A required upstream proof or evidence artifact is missing. | Generate the named artifact; do not ask an agent to guess from raw files. |
| `baseline_only` | The visible item is reviewed historical debt, not PR-local first-action work. | Keep it visible in baseline or RIPR Zero review, but do not make it the PR's first test request. |
| `acknowledged` | The item has explicit acknowledgement such as `ripr-waive`. | Inspect acknowledgement context; do not hide the item or request duplicate work. |
| `waived` | A waiver is in force for the current PR context. | Keep the waiver visible; no first test action is emitted while it applies. |
| `suppressed` | The seam is suppressed or configured off. | Treat this as policy state, not as improved evidence. |
| `no_actionable_seam` | Fresh inputs do not contain a PR-local actionable seam. | Record the clean state; do not invent a test task. |
| `already_improved` | A receipt records improved or resolved static movement. | Review and keep the receipt instead of asking for another test. |
| `unchanged_after_attempt` | A focused-test attempt exists, but static movement did not improve. | Revise the focused test for the same missing discriminator before moving to another seam. |

These statuses are advisory report routes. They are not pass/fail decisions.
`ripr gate evaluate` remains the only configured gate authority.

## Write One Focused Test

When the route is `actionable`, keep the task narrow:

```text
Write one focused test for the selected seam.
Target the missing discriminator or observation.
Imitate the related test when present.
Do not edit production code unless the PR scope already requires it.
Run the verify command.
Return the receipt.
Stop.
```

Prefer one assertion path that directly observes the behavior named in the
report. Avoid broad refactors, broad fixture rewrites, generated tests, and
new analyzer assumptions.

If the report gives a related test, start there. If it gives only a target file
or assertion shape, keep the test near the closest local pattern and document
the reason in review.

## Verify Static Movement

After the focused test is saved, run the `Verify` command from the report. A
typical command looks like:

```bash
ripr agent verify \
  --root . \
  --before target/ripr/workflow/before.repo-exposure.json \
  --after target/ripr/workflow/after.repo-exposure.json \
  --json
```

The verify step compares static RIPR evidence. It does not run tests, execute
mutation testing, or prove runtime adequacy.

Read movement conservatively:

| Movement | Meaning | Next step |
| --- | --- | --- |
| `improved` | Static evidence got stronger for the selected seam. | Emit the receipt and keep it with the PR. |
| `resolved` | The selected visible gap no longer appears under current evidence. | Emit the receipt and consider shrink-only baseline cleanup when relevant. |
| `unchanged` | Static evidence did not move. | Re-check whether the test observes the missing discriminator and whether artifacts are fresh. |
| `regressed` | Static evidence weakened. | Treat this as review evidence that the change needs inspection. |
| `unknown` | Before or after evidence is missing or not comparable. | Regenerate the artifact chain before claiming movement. |

Do not use runtime mutation vocabulary for these movement states unless a
separate imported calibration artifact explicitly supplies runtime evidence.

## Emit The Receipt

Run the receipt command from the report, for example:

```bash
ripr agent receipt \
  --root . \
  --verify-json target/ripr/workflow/agent-verify.json \
  --seam-id <seam-id> \
  --json
```

The receipt is the durable review trail. It records the selected seam, static
movement, artifact identity, warnings, and next-action guidance.

When the report status is `already_improved`, the receipt is the first action:
include it in review instead of requesting more work. When the status is
`unchanged_after_attempt`, return to the same seam and missing discriminator
instead of broadening the agent task.

## Use The Editor Projection

The VS Code extension can read an existing
`target/ripr/reports/first-useful-action.json` from the workspace and project
it through the status bar and `ripr: Show Status`.

That editor projection is a reader only:

- it does not run `ripr first-action`;
- it does not add diagnostics, CodeLens, or inlay hints;
- it does not analyze unsaved buffers;
- it does not edit source or generate tests;
- it does not call providers or run mutation testing;
- it does not make gate decisions.

If the status surface does not show first-action details, check whether the
report exists in the workspace root and whether the report root matches the
opened workspace.

## Use The CI Projection

Generated CI keeps the first useful action advisory. It may show:

- status, audience, and action kind;
- selected seam and missing discriminator;
- target file, related test, and suggested test name;
- verify and receipt commands;
- fallback state and warnings;
- links to `first-useful-action.json` and `first-useful-action.md`;
- static and advisory limits.

CI should not fail because of the first-action report. If a PR fails, inspect
`target/ripr/reports/gate-decision.md` or the configured gate job. The first
useful action explains what to do next; the gate decision explains pass/fail
policy when a repo explicitly enables one.

## Handoff For Coding Agents

Use the report as a bounded packet for external coding agents:

```text
Use target/ripr/reports/first-useful-action.md.
Follow the Status and Action fields exactly.
If actionable, write one focused test for the selected missing discriminator.
If stale or missing input, regenerate the named artifact instead.
If unchanged_after_attempt, revise the same focused test.
Run the Verify command.
Return the Receipt command output and updated report artifacts.
Do not inspect the whole repo unless the report explicitly routes to human review.
```

The handoff should include:

- selected seam ID, file, and line when present;
- missing discriminator or observation;
- suggested target file, related test, test name, or assertion shape;
- fallback reason when no test action is safe;
- verify command;
- receipt command;
- limits and warnings.

## Relationship To Other Reports

Use the reports for separate jobs:

| Report | Job |
| --- | --- |
| PR guidance | Names changed-line-safe recommendations and summary-only fallbacks. |
| Test-oracle assistant proof | Binds one assistant-directed loop into a reviewable receipt packet. |
| PR evidence ledger | Records PR-local movement, waivers, suppressions, and repair receipts. |
| Baseline debt delta | Explains movement against reviewed historical debt. |
| Gate decision | Owns pass/fail behavior when explicitly configured. |
| Coverage/grip frontier | Compares execution-surface movement with RIPR movement. |
| First useful action | Chooses the next bounded step from the existing evidence. |

Do not make the first-action report a new policy layer. It is a routing layer
that points to the smallest safe next action.

## Limits

- Static RIPR evidence only.
- Advisory by default.
- No hidden analysis rerun.
- No source edits by RIPR.
- No generated tests.
- No provider calls.
- No mutation execution.
- No runtime adequacy claims from static evidence.
- No gate-policy invention.

## Related Docs

- [First useful action spec](specs/RIPR-SPEC-0020-first-useful-action-report.md)
  defines the report contract and status vocabulary.
- [Output schema](OUTPUT_SCHEMA.md#first-useful-action-report) defines the JSON
  and Markdown shape.
- [Test-oracle assistant proof report](TEST_ORACLE_ASSISTANT_PROOF_REPORT.md)
  explains the per-loop receipt binder that first-action routing may consume.
- [Test-oracle assistant workflow](TEST_ORACLE_ASSISTANT_WORKFLOW.md) explains
  the focused-test loop from handoff to receipt.
- [PR evidence ledger workflow](PR_EVIDENCE_LEDGER_WORKFLOW.md) explains how
  PR-local movement, waivers, suppressions, and repair receipts fit together.
- [Baseline ledger workflow](BASELINE_LEDGER_WORKFLOW.md) explains historical
  debt adoption and shrink-only cleanup.
- [Calibrated gate policy](CALIBRATED_GATE_POLICY.md) explains why gate
  decisions stay separate from advisory routing.
- [Editor extension](EDITOR_EXTENSION.md) explains the status and Show Status
  projection.
