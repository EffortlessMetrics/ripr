# Assistant Loop Health Workflow

Use this workflow after RIPR has already produced one or more
`test-oracle-assistant-proof` reports and you need to know whether assistant
directed test loops are complete, stuck, missing receipts, or moving static
evidence.

Assistant loop health is a read-only operating view:

```text
test-oracle assistant proof reports
-> assistant loop health
-> complete, partial, missing-input, movement, warning, and repair queues
-> targeted repair, refresh, receipt, or no repair
```

It does not rerun hidden analysis, inspect source to fill missing fields, edit
source, generate tests, call providers, run mutation testing, change
recommendation ranking, change gate policy, change editor behavior, or make CI
blocking by default.

## Proof Report Versus Health Report

Use the proof report for one focused loop:

```text
changed behavior -> guidance -> handoff -> focused attempt -> before/after
evidence -> receipt -> test-oracle-assistant-proof
```

Use the health report to inspect one or more proof reports:

```text
proof packets -> completeness and movement summary -> recurring warnings ->
bounded repair queue
```

The distinction matters:

| Report | Question | Usual reader |
| --- | --- | --- |
| `test-oracle-assistant-proof` | Did this selected seam have a focused handoff, attempt, receipt, and static movement? | Reviewer, developer, coding agent |
| `assistant-loop-health` | Are proof packets complete, partial, missing inputs, unchanged, regressed, or ready for repair? | Maintainer, reviewer, coding-agent operator |

The health report consumes proof artifacts. It must not invent a proof packet or
promote a missing receipt into success.

## Start In GitHub

Generated CI runs the health producer only when this proof artifact already
exists:

```text
target/ripr/reports/test-oracle-assistant-proof.json
```

When the input exists, generated CI writes and uploads:

```text
target/ripr/reports/assistant-loop-health.json
target/ripr/reports/assistant-loop-health.md
```

It also appends a compact advisory health summary to the GitHub job summary.
Use that summary as the first-screen view for:

- proof packet counts;
- complete, partial, and missing-required states;
- improved, unchanged, regressed, and unknown movement;
- top warning kinds;
- the bounded repair queue;
- advisory limits.

If no proof input exists, generated CI skips this projection and leaves
pass/fail behavior unchanged.

## Generate Or Refresh Locally

Run the health producer from explicit proof paths only:

```bash
ripr assistant-loop health \
  --proof target/ripr/reports/test-oracle-assistant-proof.json \
  --out target/ripr/reports/assistant-loop-health.json \
  --out-md target/ripr/reports/assistant-loop-health.md
```

`--proof` is repeatable. Multiple proof paths are summarized in deterministic
input order.

Do not search the workspace for missing context. Regenerate the source proof
artifact first when a required proof input is absent.

## Read Proof Completeness

Read `proof_state` before acting on the repair queue.

| State | Meaning | Next action |
| --- | --- | --- |
| `complete` | The proof is parseable and includes the selected seam, recommendation or handoff context, receipt or movement context, and no missing-required proof warning. | Use the movement and warnings to decide whether the focused loop is done. |
| `partial` | The proof is parseable and tied to a seam, but optional context or non-fatal artifacts are missing. | Repair optional context when it matters; do not block by default. |
| `missing_required_input` | The proof is unreadable, malformed, incompatible, or missing required proof-chain evidence. | Regenerate or repair the named source artifact before trusting the loop. |

Complete does not mean runtime adequacy. It only means the static proof packet
has enough explicit inputs for review.

## Read Static Movement

Movement is copied from the proof report. It is static RIPR evidence, not
runtime mutation confirmation.

| Movement | Meaning | Next action |
| --- | --- | --- |
| `improved` | Static evidence strengthened or the source proof reported resolved movement. | Preserve the receipt and consider baseline shrink-only cleanup when relevant. |
| `unchanged` | A focused attempt exists, but static evidence did not move. | Inspect whether the test observed the missing discriminator, whether artifacts are stale, or whether static limits apply. |
| `regressed` | Static evidence weakened after the attempt. | Inspect the attempted repair and source artifacts; do not hide this behind a waiver. |
| `unknown` | Required before/after evidence is absent or not comparable. | Rebuild the proof chain before claiming movement. |

For unchanged movement, do not ask for broad test generation. The useful repair
is bounded: inspect the selected seam, the suggested discriminator, the related
test, and the verify command that produced the receipt.

## Repair Missing Inputs

The repair queue is mechanical. It routes maintainers or external coding agents
to the smallest next repair step.

| Repair kind | Use when | Typical action |
| --- | --- | --- |
| `regenerate_proof` | The proof file is missing, unreadable, malformed, or incompatible. | Rerun `ripr assistant-loop proof` from explicit source artifacts. |
| `regenerate_missing_artifact` | A named source artifact is absent. | Recreate the missing PR guidance, handoff, before/after evidence, receipt, ledger, gate, or coverage/grip artifact. |
| `rerun_verify_and_receipt` | The focused attempt exists but the receipt is missing. | Rerun verify, then emit `ripr agent receipt`. |
| `refresh_before_after_evidence` | Before/after inputs are stale or incomparable. | Rebuild the named before and after static evidence artifacts. |
| `inspect_unchanged_attempt` | The proof is complete enough to read but movement is unchanged. | Check whether the focused test observes the missing discriminator. |
| `inspect_regression` | Static movement regressed. | Inspect the attempted change and source artifacts before proceeding. |
| `inspect_summary_only_guidance` | Guidance could not be safely placed inline. | Keep the finding in summary view; do not force a bad changed-line annotation. |
| `attach_receipt` | A result exists outside the packet but is not attached. | Attach the receipt path so reviewers can inspect movement. |
| `no_repair` | There is no bounded repair available from current inputs. | Do not invent a broader task; inspect source warnings if needed. |

Repair entries may include an `agent_command` or `next_command`. Pass that
bounded command to a coding agent instead of asking it to inspect the repository
freely.

## Use The Generated-CI Summary

The generated-CI summary is the first-screen health view. It should be enough
to answer:

- how many proof packets were inspected;
- which packets are complete, partial, or missing required input;
- whether static movement improved, stayed unchanged, regressed, or is unknown;
- which warning kinds recur;
- what repair command or artifact should be handled next.

Open `assistant-loop-health.md` for reviewer detail and
`assistant-loop-health.json` for tools. Open the source
`test-oracle-assistant-proof` report when you need the full selected seam,
handoff, before/after, receipt, ledger, gate, or coverage/grip context.

## Advisory And Gate Boundary

Assistant loop health is evidence only.

It does not:

- make CI fail;
- post inline PR comments;
- mutate baselines;
- create suppressions or waivers;
- change gate mode;
- decide pass/fail state.

Configured gate authority remains:

```text
target/ripr/reports/gate-decision.json
target/ripr/reports/gate-decision.md
```

If `RIPR_GATE_MODE` is unset, generated CI stays advisory. If a gate mode is
configured, use the gate report for advisory, acknowledged, blocked, or
configuration-error state. Use assistant loop health to repair the proof packet
or explain why a focused loop is incomplete.

## Maintainer Checklist

Before treating a health report as useful operating evidence, verify:

- every proof path in `inputs.proofs` is an explicit artifact you intended to
  summarize;
- `summary.*` counts match the proof items, warnings, and repair queue;
- `missing_required_input` entries name the source artifact to regenerate;
- `partial` entries are not silently treated as success;
- unchanged and regressed movement stay visible;
- repair queue entries are bounded to one seam, artifact, or command;
- optional gate, ledger, coverage/grip, and first-action context remain
  optional;
- pass/fail conclusions come from `ripr gate evaluate`, not from health.

## Related Docs

- [Test-oracle assistant workflow](TEST_ORACLE_ASSISTANT_WORKFLOW.md) explains
  the end-to-end PR/editor-to-receipt loop.
- [Test-oracle assistant proof report](TEST_ORACLE_ASSISTANT_PROOF_REPORT.md)
  explains the source proof packet consumed by health.
- [First useful action workflow](FIRST_USEFUL_ACTION_WORKFLOW.md) explains how
  existing artifacts collapse into one advisory next action.
- [Output schema](OUTPUT_SCHEMA.md#assistant-loop-health-report) defines the
  `assistant-loop-health.{json,md}` contract.
- [RIPR-SPEC-0022](specs/RIPR-SPEC-0022-assistant-loop-health-report.md)
  defines the assistant loop health report contract and non-goals.
