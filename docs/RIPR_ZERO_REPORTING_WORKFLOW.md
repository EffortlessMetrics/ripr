# RIPR Zero Reporting Workflow

Use this workflow after a repository has a reviewed baseline ledger and wants
RIPR to show progress toward RIPR 0 without turning the status report into a
new gate.

`ripr zero status` is a read-only reporting layer over existing artifacts:

```text
reviewed baseline
-> baseline debt delta
-> optional gate decision
-> optional PR guidance and recommendation calibration
-> RIPR Zero status
```

The report answers one operating question:

```text
Is this repo or PR moving behavioral test grip toward RIPR 0?
```

RIPR 0 means no visible unresolved behavioral test-grip gaps remain under the
configured scope and policy. It does not mean perfect tests, 100 percent
coverage, no suppressions, no unknowns, or runtime mutation confirmation.

## Inputs

The status command needs a baseline debt delta and can use richer context when
available:

```bash
ripr zero status \
  --baseline .ripr/gate-baseline.json \
  --delta target/ripr/reports/baseline-debt-delta.json \
  --gate target/ripr/reports/gate-decision.json \
  --pr-guidance target/ripr/review/comments.json \
  --recommendation-calibration target/ripr/reports/recommendation-calibration.json \
  --out target/ripr/reports/ripr-zero-status.json \
  --out-md target/ripr/reports/ripr-zero-status.md
```

Generated GitHub workflows run this command when
`target/ripr/reports/baseline-debt-delta.json` exists. That normally requires
an explicit gate mode and a reviewed baseline:

```text
RIPR_GATE_MODE=visible-only | acknowledgeable | baseline-check | calibrated-gate
RIPR_GATE_BASELINE=.ripr/gate-baseline.json
```

The status report is advisory. `ripr gate evaluate` still owns pass/fail
decisions for configured gate modes.

## Read The First Screen

Start with the GitHub job summary or `target/ripr/reports/ripr-zero-status.md`.
The summary should be enough to answer the PR-review question without opening
raw JSON.

Read the key fields this way:

| Field | Meaning | Usual action |
| --- | --- | --- |
| `RIPR 0` / `ripr_zero.state` | `achieved`, `not_yet`, or `unknown` under the supplied scope. | Use `not_yet` as a burn-down signal, not a failed adoption. |
| `visible_unresolved` | Still-visible behavioral grip gaps after baseline, acknowledgement, suppression, and gate context are joined. | Review top debt areas and repair routes. |
| `new_policy_eligible` | Current policy-eligible gaps not covered by the reviewed baseline. | Add a focused test, acknowledge visibly, or keep the gate blocked when configured. |
| `blocking_candidates` | Candidates the gate decision says can block under the explicit mode. | Follow the gate repair or acknowledgement path; do not use the status report as the blocking authority. |
| `acknowledged` | Findings accepted for this PR, usually through `ripr-waive`. | Keep visible in the review record; remove the waiver when the focused test lands. |
| `suppressed` | Findings hidden or configured off by repository policy. | Audit the suppression reason, owner, and review date separately from baseline debt. |
| `baseline.still_present` | Reviewed baseline debt still present in current evidence. | Burn down with focused tests. |
| `baseline.resolved` | Reviewed baseline debt absent from current evidence. | Remove it with shrink-only baseline update after review. |
| `baseline.metadata.stale` | Baseline entries whose review metadata needs attention. | Re-review ownership, reason, or `review_after` before stricter policy. |
| `top_debt_areas` | Reporting groups by stable path or configured area. | Pick the highest-value area for the next repair PR. |
| `repair_routes` | Bounded focused-test handoffs copied from existing evidence. | Give the route to a human or coding agent. |

When `state` is `unknown`, inspect `warnings[]` first. Missing or invalid
inputs should be repaired before interpreting debt movement.

## Age And Refresh Baselines

Baseline metadata is part of the review record. A healthy entry should explain:

- who owns the baseline debt;
- why it was accepted as existing debt;
- when it was created;
- when it should be reviewed again;
- which source artifact created it.

Missing metadata does not hide a finding. It means the baseline is less
reviewable.

Treat stale metadata as a maintenance signal:

```text
stale metadata
-> re-review the entry
-> keep, repair, suppress with reason, or remove when resolved
```

Use shrink-only refreshes after focused tests move evidence:

```bash
ripr baseline update \
  --baseline .ripr/gate-baseline.json \
  --current target/ripr/reports/gate-decision.json \
  --remove-resolved \
  --out .ripr/gate-baseline.json
```

That command removes resolved reviewed entries and refuses to adopt new current
debt. Growing the baseline must be an intentional review decision, not a CI
side effect.

## Route Repair Packets

Use `repair_routes[]` as the handoff surface. A useful route should name:

- the seam or finding identity;
- the file and line when available;
- the missing discriminator;
- the suggested focused test shape;
- the best related test when available;
- the verify command;
- the agent command when available;
- the source artifact that supplied the route.

Example reviewer note:

```text
Please take the top RIPR repair route:
- missing discriminator: amount == discount_threshold
- suggested test: equality-boundary assertion
- related test: tests/pricing.rs::applies_discount_above_threshold
- verify: ripr agent verify --root . --before ... --after ... --json
```

If a route lacks a suggested test, related test, or verify command, use the
warning as a signal to inspect the source artifact. Do not invent a stronger
claim than the report provides.

## Interpret Progress

Use the status report as a behavioral-debt ledger, not a coverage score.

Good movement can look like:

```text
visible_unresolved: 43 -> 40
baseline.resolved: 7 -> 10
new_policy_eligible: 0
coverage delta: 0.0 percent
```

That can be useful because a focused discriminator can improve behavioral grip
without reaching new lines.

Avoid these interpretations:

| Report says | Do not infer |
| --- | --- |
| `RIPR 0: achieved` | The test suite is perfect or the code is bug-free. |
| `visible_unresolved: 0` | Runtime mutation testing would find no issues. |
| `suppressed: 3` | The suppressed findings disappeared. |
| `acknowledged: 1` | The finding was fixed. |
| `trend.source: not_available` | The repo regressed. |

Use trends only when a prior status, ledger, or history artifact is supplied.
Without trend input, the report still describes the current state; it just does
not claim movement over time.

## CI Review Checklist

Before promoting RIPR Zero status into a stronger workflow, make sure reviewers
can answer:

- What is the current RIPR 0 state?
- How many visible unresolved gaps remain?
- Which gaps are new policy-eligible?
- Which baseline gaps were resolved?
- Which entries have stale or missing metadata?
- What is the top repair route?
- What command verifies evidence movement?
- Is any blocking decision coming from `ripr gate evaluate`, not the status
  report?

If these answers are unclear, stay in `visible-only` or `baseline-check` until
the summary and baseline metadata are reviewable.

## Related Docs

- [Baseline ledger workflow](BASELINE_LEDGER_WORKFLOW.md) explains baseline
  creation, diffing, `baseline-check`, and shrink-only refreshes.
- [PR evidence ledger workflow](PR_EVIDENCE_LEDGER_WORKFLOW.md) explains how
  per-PR ledger records track waiver aging, baseline burn-down, repair
  receipts, and coverage/grip frontier signals.
- [Calibrated gate policy](CALIBRATED_GATE_POLICY.md) defines gate modes and
  acknowledgement behavior.
- [RIPR blocking readiness](BLOCKING_READINESS.md) explains when to stay
  advisory or promote an explicit gate mode.
- [Output schema](OUTPUT_SCHEMA.md#ripr-zero-status-report) defines the
  `ripr-zero-status` JSON and Markdown contracts.
- [RIPR-SPEC-0017](specs/RIPR-SPEC-0017-ripr-zero-reporting.md) records the
  reporting contract and acceptance examples.
