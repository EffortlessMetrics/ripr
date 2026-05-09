# Test-Oracle Assistant Proof Report

Use this report when PR guidance, editor or agent handoff, before/after static
evidence, and a receipt already exist and you want one reviewable record of the
loop.

The proof report is a read-only join over existing artifacts:

```text
PR guidance
-> editor or agent handoff
-> before static evidence
-> after static evidence
-> receipt
-> PR evidence ledger
-> optional gate or coverage/grip context
-> test-oracle assistant proof
```

It writes:

```text
target/ripr/reports/test-oracle-assistant-proof.json
target/ripr/reports/test-oracle-assistant-proof.md
```

The report does not make tests better by itself. It tells a reviewer,
maintainer, or coding agent whether the current evidence packet is complete
enough to trust as a focused-test receipt.

## Generate The Report

Generated GitHub CI writes the report only when the required artifact chain is
already present:

```text
target/ripr/review/comments.json
target/ripr/workflow/agent-brief.json
target/ripr/workflow/before.repo-exposure.json
target/ripr/workflow/after.repo-exposure.json
target/ripr/reports/agent-receipt.json
target/ripr/reports/pr-evidence-ledger.json
```

When any required input is absent, generated CI skips this projection. It does
not print a placeholder, block the PR, rerun hidden analysis, or infer success
from partial artifacts.

To run the same join locally:

```bash
ripr assistant-loop proof \
  --pr-guidance target/ripr/review/comments.json \
  --agent-packet target/ripr/workflow/agent-brief.json \
  --before target/ripr/workflow/before.repo-exposure.json \
  --after target/ripr/workflow/after.repo-exposure.json \
  --receipt target/ripr/reports/agent-receipt.json \
  --ledger target/ripr/reports/pr-evidence-ledger.json \
  --coverage-frontier target/ripr/reports/coverage-grip-frontier.json \
  --gate-decision target/ripr/reports/gate-decision.json \
  --out target/ripr/reports/test-oracle-assistant-proof.json \
  --out-md target/ripr/reports/test-oracle-assistant-proof.md
```

`--coverage-frontier` and `--gate-decision` are optional. Leave them out when
those reports were not produced.

## Read The First Screen

Start with the GitHub job summary or
`target/ripr/reports/test-oracle-assistant-proof.md`. A reviewer should not
need to open every source JSON file to answer:

```text
Which changed behavior was selected?
What discriminator or observation looked weak or missing?
What focused test was suggested?
Where is the handoff packet?
Did static evidence move after the test?
Where is the receipt?
Which optional CI reports were joined?
```

Read the main Markdown sections this way:

| Section | Meaning | Usual action |
| --- | --- | --- |
| `Status` | `advisory` means the joined record is complete enough to read; `incomplete` means required evidence is missing or not comparable. | Inspect warnings before using the report as a receipt. |
| `Top focused test` | The selected seam, owner, missing discriminator, suggested test, related test, assertion shape, and verify command copied from `evidence_record` when present, with legacy guidance or handoff fields as fallback. | Give this bounded task to a human or coding agent. |
| `Movement` | Static before/after evidence for the selected seam. | Use it as review evidence, not runtime confirmation. |
| `Projection` | PR ledger, coverage/grip frontier, and gate-decision paths when supplied. | Follow links for adoption, coverage, or gate context. |
| `Warnings` | Missing, invalid, stale-looking, summary-only, or unsupported inputs. | Treat warnings as part of the receipt, not as noise. |
| `Limits` | Boundaries that keep the loop advisory and static. | Do not promote stronger claims than the report makes. |

## Interpret Static Movement

`evidence_movement.state` is static RIPR movement only.
When before/after repo-exposure seams include `evidence_record`, movement class
comparison uses `evidence_record.grip_class`; older `grip_class` fields remain
fallback for legacy artifacts.

| State | Meaning | Usual action |
| --- | --- | --- |
| `improved` | The selected seam has stronger static evidence after the focused test. | Keep the receipt and consider shrink-only baseline cleanup when relevant. |
| `resolved` | The selected visible gap no longer appears under current static evidence. | Review the test and remove resolved baseline debt after review. |
| `unchanged` | Static evidence did not move. | Inspect whether the test missed the discriminator, the artifacts are stale, or the analyzer cannot see the signal. |
| `regressed` | Static evidence is weaker after the change. | Treat this as a review concern; do not hide it behind waiver or baseline language. |
| `unknown` | Required before/after evidence is missing or cannot be compared. | Rebuild the artifact chain before claiming movement. |

Do not use runtime mutation vocabulary for this state. Without imported runtime
calibration, the report cannot say that a mutant was killed or survived.

## Read Warnings

Warnings are evidence. They explain why a proof record may be incomplete or
weaker than the first-screen card suggests.

Common warning classes:

- required input missing or unreadable;
- optional input supplied but invalid;
- selected seam missing from one artifact;
- before and after evidence not comparable;
- recommendation is summary-only because changed-line placement was unsafe;
- receipt or ledger did not include the selected seam;
- coverage/grip frontier or gate decision was not supplied.

Use warnings to decide the next action:

```text
missing required input -> rerun or upload the missing report
summary-only guidance -> keep recommendation in the summary, do not force an inline annotation
unchanged movement -> inspect the focused test and static limits
invalid optional input -> repair the optional report, but do not treat the proof as blocking
```

## Use CI Projection

Generated CI appends the proof report to the RIPR advisory summary only when
the report exists. It also uploads both proof artifacts through the normal
`ripr-reports` artifact packet.

The proof report remains evidence only:

- it does not post PR comments;
- it does not emit new check annotations;
- it does not edit source;
- it does not generate tests;
- it does not mutate baselines;
- it does not call an external provider;
- it does not run mutation testing;
- it does not make CI blocking.

If a run fails, look for a configured gate decision. `ripr gate evaluate`
remains the pass/fail authority when `RIPR_GATE_MODE` is set.

## Handoff For Coding Agents

The proof report is the compact task packet for an external coding agent. A
good agent handoff from the report is:

```text
Use the selected seam from test-oracle-assistant-proof.md.
Write one focused test for the missing discriminator.
Imitate the related test when present.
Do not edit production code unless the PR scope already requires it.
Run the verify command.
Return the updated receipt and proof report.
```

The handoff should include:

- seam ID, file, and line;
- missing discriminator;
- suggested focused test;
- related test;
- agent command;
- verify command;
- receipt path;
- warnings that affect confidence.

If the report lacks a suggested test, related test, or verify command, keep the
task in human review until the source artifacts are repaired.

## Maintainer Checklist

Before treating a proof report as a useful receipt, verify:

- `status` is `advisory`, or all `incomplete` warnings are understood;
- the selected seam is the same seam discussed in PR guidance or editor
  diagnostics;
- the missing discriminator is visible in the Markdown summary;
- the suggested focused test is specific enough to act on;
- the verify command points at the same before/after artifacts named in the
  report;
- the receipt path exists and records static movement;
- optional gate and coverage/grip paths are present only when those reports
  were actually generated;
- pass/fail conclusions come from `ripr gate evaluate`, not from the proof
  report;
- runtime mutation terms appear only when imported runtime calibration is
  explicitly supplied elsewhere.

## Related Docs

- [Test-oracle assistant workflow](TEST_ORACLE_ASSISTANT_WORKFLOW.md) explains
  the end-to-end PR/editor-to-receipt loop.
- [PR evidence ledger workflow](PR_EVIDENCE_LEDGER_WORKFLOW.md) explains how
  PR movement, waivers, baseline burn-down, repair receipts, and coverage/grip
  frontier data fit together.
- [First useful action workflow](FIRST_USEFUL_ACTION_WORKFLOW.md) explains how
  proof, ledger, receipt, gate, and editor context can collapse into one
  advisory next action.
- [CI strategy](CI.md#generated-github-workflow) describes generated workflow
  projection into GitHub summaries and artifacts.
- [Output schema](OUTPUT_SCHEMA.md#test-oracle-assistant-loop) defines the
  JSON and Markdown contract.
- [RIPR-SPEC-0019](specs/RIPR-SPEC-0019-test-oracle-assistant-loop.md) records
  the proof-loop product contract and non-goals.
