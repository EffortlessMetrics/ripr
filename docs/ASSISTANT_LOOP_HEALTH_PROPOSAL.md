# Assistant Loop Health Proposal

Campaign 21 made one assistant-directed test loop reviewable with
`test-oracle-assistant-proof.{json,md}`. The next health lane should measure
whether those proof packets are complete, useful, and moving static evidence
over time.

This proposal has been promoted into Campaign 23 after Campaign 22 closed First
Useful Action. The machine-readable campaign is `.ripr/goals/active.toml`; this
document remains the design brief for the campaign. The report contract is
[RIPR-SPEC-0022](specs/RIPR-SPEC-0022-assistant-loop-health-report.md).

## Goal

Produce an advisory assistant-loop health report from existing proof artifacts:

```text
target/ripr/reports/assistant-loop-health.json
target/ripr/reports/assistant-loop-health.md
```

The report should answer:

```text
How many assistant proof packets exist?
How many are complete or partial?
Which required or optional inputs are missing?
Which selected seams improved, stayed unchanged, regressed, or remain unknown?
Which warnings recur?
Which proof packets should a maintainer or coding agent repair next?
```

The health report is an operating dashboard. It is not a gate, ranking model,
LLM judge, runtime proof, or test generator.

## Inputs

The first implementation should read one or more existing proof reports:

```bash
ripr assistant-loop health \
  --proof target/ripr/reports/test-oracle-assistant-proof.json \
  --out target/ripr/reports/assistant-loop-health.json \
  --out-md target/ripr/reports/assistant-loop-health.md
```

Repeatable `--proof` inputs should be supported or explicitly reserved in the
spec so the health report can summarize multiple PR or loop artifacts later.

The command must read existing artifacts only. It must not rerun analysis, infer
missing proof fields from the workspace, call providers, run mutation testing,
edit source, or generate tests.

## Output Shape

The JSON contract should include at least:

```json
{
  "schema_version": "0.1",
  "tool": "ripr",
  "kind": "assistant_loop_health",
  "status": "advisory",
  "inputs": {
    "proofs": ["target/ripr/reports/test-oracle-assistant-proof.json"]
  },
  "summary": {
    "proofs": 1,
    "complete": 1,
    "partial": 0,
    "missing_required_input": 0,
    "missing_optional_input": 0,
    "improved": 1,
    "unchanged": 0,
    "regressed": 0,
    "unknown_movement": 0
  },
  "warnings": [],
  "repair_queue": [],
  "limits": [
    "advisory static evidence only",
    "gate evaluator remains pass/fail authority",
    "no provider calls",
    "no mutation execution"
  ]
}
```

The Markdown should fit a GitHub job summary:

```text
Assistant Loop Health: advisory

Proof packets:
- complete: 8
- partial: 2
- missing required inputs: 1
- missing optional inputs: 3

Evidence movement:
- improved: 5
- unchanged: 4
- regressed: 0
- unknown: 1

Top warnings:
- missing gate-decision input: 3
- unchanged selected seam: 4

Next repair queue:
- src/pricing.rs:88 - unchanged movement; add focused assertion
- src/auth.rs:42 - missing receipt; rerun verify and receipt
```

## Health Buckets

The report should use explicit buckets rather than an opaque health score:

| Bucket | Meaning |
| --- | --- |
| `complete` | Required proof inputs and receipt state are present. |
| `partial` | Proof exists but optional context or non-fatal inputs are missing. |
| `missing_required_input` | The proof cannot establish the loop chain because a required input is absent. |
| `missing_optional_input` | A useful but non-required context artifact is absent. |
| `improved` | Static after-evidence moved in the intended direction according to the proof. |
| `unchanged` | Static evidence did not move after the focused attempt. |
| `regressed` | Static after-evidence weakened according to the proof. |
| `unknown_movement` | The proof cannot classify before/after movement. |

## Repair Queue

The repair queue should remain mechanical and bounded. It should route work such
as:

- missing receipt: rerun `ripr agent verify` and `ripr agent receipt`;
- missing required proof input: regenerate the named artifact;
- unchanged movement: inspect whether the added test observes the missing
  discriminator;
- stale before/after evidence: regenerate the before/after snapshots;
- warning-heavy proof: inspect the warning and source artifact path.

It must not tell an agent to inspect the whole repository freely or generate
tests automatically.

## Proposed Campaign Stack

The active campaign uses this PR stack:

| Work item | Purpose |
| --- | --- |
| `spec/assistant-loop-health-report` | Done: RIPR-SPEC-0022 defines JSON/Markdown contract, inputs, statuses, buckets, warnings, repair queue, and advisory limits. |
| `fixtures/assistant-loop-health-corpus` | Next: pin complete, partial, missing-input, unchanged, regressed, warning-heavy, and multi-proof cases. |
| `report/assistant-loop-health` | Add the read-only CLI report producer over explicit proof inputs. |
| `ci/assistant-loop-health-artifacts` | Upload and summarize health artifacts only when proof artifacts exist. |
| `docs/assistant-loop-health-workflow` | Explain proof report vs health report, repair routing, and advisory limits. |
| `campaign/assistant-loop-health-closeout` | Close after spec, fixtures, producer, CI projection, docs, and validation prove the lane. |

## Non-goals

- No analyzer behavior changes.
- No recommendation ranking changes.
- No gate policy changes.
- No LSP or editor behavior changes.
- No mutation execution.
- No provider or API calls.
- No source edits.
- No generated tests.
- No default CI blocking.
- No adequacy or proof-of-correctness claims.

## Relationship To First Useful Action

First Useful Action answers:

```text
What should this developer, reviewer, or coding agent do next?
```

Assistant Loop Health answers:

```text
Are assistant-directed test loops complete, stuck, missing receipts, or moving
static evidence over time?
```

The health report waited until First Useful Action settled the first-screen
routing contract. That avoids adding another raw report before users have one
clear next action.
