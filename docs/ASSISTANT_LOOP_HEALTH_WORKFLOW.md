# Assistant Loop Health Workflow

Use this workflow when assistant-directed test work already produces one or
more `test-oracle-assistant-proof` reports and you need to know whether the
loop is complete, stuck, missing inputs, or moving static evidence.

Assistant loop health is a read-only operating dashboard:

```text
test-oracle assistant proof reports
-> assistant-loop-health.{json,md}
-> proof completeness, movement, warnings, and repair queue
-> focused follow-up for humans or external coding agents
```

It does not rerun hidden analysis, inspect source files to fill missing fields,
edit source, generate tests, call providers, run mutation testing, change
recommendation ranking, change gate policy, or make CI blocking by default.

## Proof Report Versus Health Report

The proof report answers one-loop review questions:

```text
What seam did this assistant loop target?
What was missing?
Did the before/after static evidence move?
Where is the receipt?
```

The health report answers operating questions across one or more proof reports:

```text
How many proof packets are complete?
Which inputs are missing?
How often is static evidence unchanged or regressed?
Which warnings recur?
Which packets should a maintainer or coding agent repair next?
```

Use proof reports to review one loop. Use health reports to improve the loop
system.

## Generate The Report

Generated GitHub CI writes health artifacts when the assistant proof report
already exists:

```text
target/ripr/reports/test-oracle-assistant-proof.json
```

It writes:

```text
target/ripr/reports/assistant-loop-health.json
target/ripr/reports/assistant-loop-health.md
```

To run the same summary locally:

```bash
ripr assistant-loop health \
  --root . \
  --proof target/ripr/reports/test-oracle-assistant-proof.json \
  --out target/ripr/reports/assistant-loop-health.json \
  --out-md target/ripr/reports/assistant-loop-health.md
```

`--proof` can be repeated when you want one health report over multiple proof
packets:

```bash
ripr assistant-loop health \
  --root . \
  --proof target/ripr/reports/test-oracle-assistant-proof.json \
  --proof target/ripr/reports/older-test-oracle-assistant-proof.json \
  --out target/ripr/reports/assistant-loop-health.json \
  --out-md target/ripr/reports/assistant-loop-health.md
```

Only pass explicit proof paths. Do not use the health report as a reason to
search the workspace or infer missing proof data.

## Start In GitHub

In generated CI, start with the `Assistant loop health` section of the RIPR
advisory summary when it exists. The first screen shows:

- total proof packets;
- complete, partial, missing-required, and missing-optional counts;
- improved, unchanged, regressed, and unknown static movement counts;
- warning totals and top warning kinds;
- repair queue count and first repair kind;
- the artifact paths for the full JSON and Markdown report;
- the advisory boundary.

If the section is absent, no proof report existed for that run. Read the
assistant proof report workflow first and rebuild the proof packet before
expecting health output.

## Read Completeness

Completeness describes whether the supplied proof packet can support review of
the assistant loop.

| State | Meaning | Usual action |
| --- | --- | --- |
| `complete` | Required proof evidence is present: selected seam, handoff or recommendation context, and receipt or static movement context. | Review movement and warnings; no proof repair is usually needed. |
| `partial` | The proof is parseable and tied to a seam, but optional or non-fatal context is missing. | Decide whether the missing context matters before handing it to an agent. |
| `missing_required_input` | The proof is unreadable, malformed, incompatible, or missing required loop evidence. | Regenerate the named proof input or missing artifact before trusting the loop. |

Do not treat `partial` as failure or `complete` as adequacy. These states only
describe the proof packet.

## Read Static Movement

Movement is copied from proof data. The health report does not run tests or
mutation tools.

| State | Meaning | Usual action |
| --- | --- | --- |
| `improved` | The proof reports stronger static evidence, including resolved movement. | Keep the proof and receipt; consider baseline burn-down if the finding was known debt. |
| `unchanged` | Static evidence did not move after an assistant-directed attempt. | Inspect whether the test observes the missing discriminator, whether artifacts are stale, or whether static limits apply. |
| `regressed` | Static evidence weakened according to the proof. | Treat this as a review concern and inspect the attempted change. |
| `unknown` | The proof does not include enough before/after evidence to classify movement. | Rebuild before/after evidence, verify, and receipt artifacts. |

Use conservative static vocabulary. The health report cannot say a mutation was
killed, survived, or that a test is adequate.

## Repair Missing Inputs

Warnings and repair queue entries are meant to route bounded follow-up work.
They are not scores.

Common repairs:

| Repair kind | When it appears | Next action |
| --- | --- | --- |
| `regenerate_proof` | The proof file is unreadable, malformed, or incompatible. | Recreate `test-oracle-assistant-proof.{json,md}` from explicit inputs. |
| `regenerate_missing_artifact` | The proof names a required artifact that is missing. | Generate or upload that artifact, then rerun health. |
| `rerun_verify_and_receipt` | The loop lacks a verify or receipt artifact. | Run the verify and receipt commands from the proof or first-action packet. |
| `refresh_before_after_evidence` | Before/after evidence is missing or stale. | Regenerate the before and after static evidence snapshots. |
| `inspect_unchanged_attempt` | Movement stayed unchanged after an attempt. | Check whether the test asserts the missing discriminator and whether the analyzer can observe it. |
| `inspect_regression` | Movement regressed. | Review the attempted test or evidence context before accepting the change. |
| `inspect_summary_only_guidance` | PR guidance was intentionally summary-only. | Keep the recommendation in summary form; do not force misleading inline placement. |
| `attach_receipt` | A receipt is missing but the rest of the loop is readable. | Attach or regenerate the receipt so reviewers can inspect movement. |

A repair queue entry should name a concrete artifact, seam, reason, and command
when available. It should not ask an agent to inspect the whole repository or
generate tests automatically.

## Use With First Useful Action

First Useful Action chooses the next user-facing move from the wider RIPR
artifact set. Assistant loop health measures whether assistant-loop proof
packets are healthy.

Use them together this way:

1. Read `first-useful-action.md` to decide the next focused action.
2. Use `test-oracle-assistant-proof.md` to review the single assistant loop.
3. Use `assistant-loop-health.md` to see whether proof packets are complete,
   stuck, or producing unchanged movement.
4. Assign repair queue entries to a maintainer or external coding agent when
   proof packets are missing receipts, stale inputs, or unchanged movement.

If the first action and health report disagree, prefer the more specific repair
state. For example, an `unchanged` health item should be repaired before asking
an agent to repeat the same focused test.

## CI And Gate Boundary

Generated CI uploads and summarizes assistant loop health as advisory evidence
only. The report does not own pass/fail behavior.

Gate authority remains:

```text
target/ripr/reports/gate-decision.json
target/ripr/reports/gate-decision.md
```

If a PR fails, inspect the configured gate decision. If the health report has
missing inputs or unchanged movement, use it to repair the assistant loop, not
to infer a policy result.

## Handoff For Coding Agents

A good coding-agent handoff from a health report is narrow:

```text
Use the first repair queue entry from assistant-loop-health.md.
Repair that proof packet only.
Regenerate the named missing artifact, rerun verify/receipt, or inspect the
unchanged attempt as directed.
Do not call providers from RIPR, generate tests automatically, rerun hidden
analysis, or change gate policy.
Return the updated proof and health artifacts.
```

Include:

- source proof artifact;
- selected seam ID, file, and line when present;
- warning kind;
- repair kind;
- next command;
- expected result;
- advisory limits.

## Maintainer Checklist

Before using a health report to route follow-up work, verify:

- the report input paths are the proof packets you meant to summarize;
- `status` is `advisory`, or all `incomplete` warnings are understood;
- missing-required counts are zero before treating proof packets as reviewable;
- unchanged and regressed movement items have repair queue entries;
- repeated warnings are real workflow issues, not stale artifacts from an old
  CI run;
- pass/fail conclusions come from `ripr gate evaluate`, not from health;
- runtime mutation vocabulary appears only in imported calibration elsewhere;
- generated CI remains advisory unless an explicit gate mode is configured.

## Related Docs

- [Test-oracle assistant proof report](TEST_ORACLE_ASSISTANT_PROOF_REPORT.md)
  explains the per-loop proof packet that feeds health reports.
- [First useful action workflow](FIRST_USEFUL_ACTION_WORKFLOW.md) explains the
  first-screen action route that may use proof and health context.
- [PR evidence ledger workflow](PR_EVIDENCE_LEDGER_WORKFLOW.md) explains PR
  movement, waivers, baseline burn-down, repair receipts, and coverage/grip
  frontier context.
- [CI strategy](CI.md#generated-github-workflow) describes generated workflow
  projection into GitHub summaries and artifacts.
- [Output schema](OUTPUT_SCHEMA.md#assistant-loop-health-report) defines the
  JSON and Markdown contract.
- [RIPR-SPEC-0022](specs/RIPR-SPEC-0022-assistant-loop-health-report.md)
  records the assistant-loop-health report contract and non-goals.
