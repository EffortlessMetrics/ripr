# PR Evidence Ledger Workflow

Use this workflow after PR guidance, gate decisions, baseline deltas, and RIPR
Zero status are available and the team wants a per-PR adoption record instead
of another one-off summary.

The PR evidence ledger is a read-only advisory layer:

```text
PR guidance
-> optional gate decision
-> optional baseline debt delta
-> optional RIPR Zero status
-> optional repair receipt and coverage summary
-> PR evidence ledger
```

It answers one review question:

```text
Did this PR move behavioral test grip toward or away from RIPR 0?
```

RIPR 0 means no visible unresolved behavioral test-grip gaps remain under the
configured scope and policy. It does not mean perfect tests, 100 percent
coverage, no suppressions, no unknowns, or runtime mutation confirmation.

## Inputs

Generated GitHub workflows run the ledger on pull requests when PR guidance
exists at `target/ripr/review/comments.json`. The command can consume richer
context when those artifacts exist:

```bash
ripr pr-ledger record \
  --pr-number 123 \
  --base origin/main \
  --head HEAD \
  --pr-guidance target/ripr/review/comments.json \
  --gate target/ripr/reports/gate-decision.json \
  --baseline-delta target/ripr/reports/baseline-debt-delta.json \
  --zero-status target/ripr/reports/ripr-zero-status.json \
  --recommendation-calibration target/ripr/reports/recommendation-calibration.json \
  --agent-receipt target/ripr/reports/agent-receipt.json \
  --coverage target/ripr/reports/coverage-summary.json \
  --history .ripr/pr-evidence-ledger.jsonl \
  --out target/ripr/reports/pr-evidence-ledger.json \
  --out-md target/ripr/reports/pr-evidence-ledger.md
```

The report is evidence only. `ripr gate evaluate` remains the pass/fail
authority for configured gate modes.

## Read The First Screen

Start with the GitHub job summary or
`target/ripr/reports/pr-evidence-ledger.md`. A reviewer should not need to
open raw JSON to decide what happened.

Read the main fields this way:

| Field | Meaning | Usual action |
| --- | --- | --- |
| `New policy-eligible gaps` | Current visible gaps that are not covered by the reviewed baseline and are eligible under the explicit gate mode. | Add a focused test, acknowledge visibly for this PR, or let the configured gate block. |
| `Existing baseline gaps still present` | Reviewed historical debt still present in current evidence. | Keep visible and burn down intentionally. |
| `Baseline gaps resolved` | Reviewed baseline identities absent from current evidence. | Remove with shrink-only baseline update after review. |
| `Acknowledged gaps` | PR-time findings accepted with a visible label such as `ripr-waive`. | Keep visible; remove the waiver when the focused test lands. |
| `Suppressed gaps` | Durable policy exceptions or configured-off findings. | Audit separately from waivers and baseline debt. |
| `Blocking candidates` | Candidates the gate decision says can block under the explicit mode. | Follow the repair or acknowledgement path in the summary. |
| `Visible unresolved gaps` | Current visible debt after baseline, acknowledgement, suppression, and gate context are joined. | Use as a PR-local burn-down signal. |
| `Top repair route` | The best bounded focused-test handoff copied from existing artifacts. | Assign it to a human or coding agent. |
| `Coverage/grip frontier` | Optional comparison of execution coverage movement and RIPR behavioral grip movement. | Use it to understand whether grip improved without treating coverage as adequacy. |
| `History trend` | Optional trend from prior ledger records. | Use only when history input was supplied. |

When the ledger status is `incomplete`, inspect `warnings[]` first. Missing PR
identity, missing PR guidance, or unreadable optional inputs should be repaired
before interpreting movement.

## Movement Buckets

The ledger should make a messy repository feel adoptable:

```text
Raw visible gaps: 6114
Known baseline debt: 6114
New policy-eligible gaps: 0
Resolved since baseline: 14
Acknowledged this PR: 1
Blocking: 0
```

Use those buckets separately:

- baseline debt is known before policy and should stay visible;
- new policy-eligible debt is the first thing to stop;
- acknowledgements are PR-time review receipts;
- suppressions are durable policy exceptions with reasons;
- resolved baseline entries are progress toward RIPR 0.

Do not treat baseline, waiver, and suppression as interchangeable controls.

## Waiver Aging

`ripr-waive` is a visible acknowledgement, not a hiding mechanism.

Expected meaning:

```text
Decision: acknowledged
Label: ripr-waive
Finding: still visible
Scope: this PR
```

When history is supplied, the ledger can show waiver age by PR count or days.
Use aging as review pressure:

```text
waiver age grows
-> inspect whether the focused test should land
-> remove the waiver after repair
-> avoid turning repeated waivers into silent policy
```

If the team accepts a finding beyond one PR, use a reviewed baseline or a
reasoned suppression depending on whether the finding should remain part of
the burn-down ledger.

## Baseline Burn-Down

Use `baseline_resolved` and `baseline_still_present` as the adoption ledger.

Good movement:

```text
Existing baseline gaps still present: 42 -> 39
Baseline gaps resolved: 3
New policy-eligible gaps: 0
```

After a focused test moves evidence, refresh the checked-in baseline with the
shrink-only path:

```bash
ripr baseline update \
  --baseline .ripr/gate-baseline.json \
  --current target/ripr/reports/gate-decision.json \
  --remove-resolved \
  --out .ripr/gate-baseline.json
```

Shrink automatically. Grow only intentionally. Do not add new current findings
to the baseline just to make one PR pass.

## Repair Receipts

Use `top_repair_route` and `repair_receipts[]` as the handoff surface.

A useful repair route should include:

- the seam or finding identity;
- the file and line when available;
- the missing discriminator;
- the suggested focused test shape;
- the best related test when available;
- the verify command;
- the agent command when available;
- the receipt path that records evidence movement.

Example reviewer handoff:

```text
Please take the top RIPR repair route:
- missing discriminator: amount == discount_threshold
- suggested test: equality-boundary assertion
- related test: tests/pricing.rs::applies_discount_above_threshold
- verify: ripr agent verify --root . --before ... --after ... --json
```

When a repair receipt is present, read `static_movement.state` as a static
evidence signal only:

| State | Meaning | Usual action |
| --- | --- | --- |
| `improved` | The focused change improved the available static evidence. | Consider removing resolved baseline debt or the PR waiver after review. |
| `unchanged` | The focused change did not move the static signal. | Inspect whether the test missed the discriminator or the source artifact is stale. |
| `regressed` | The current evidence is worse than the before snapshot. | Treat as a review blocker under ordinary code review, even if the ledger itself is advisory. |
| `missing` / `unknown` | No receipt or comparable before/after evidence was available. | Do not infer improvement. |

The ledger must not invent repair success when the receipt is absent.

## Coverage / Grip Frontier

Coverage is execution evidence. RIPR is static behavioral grip evidence. The
ledger keeps those axes separate.

Useful examples:

```text
Coverage delta: +0.0%
RIPR visible unresolved delta: -3
Interpretation: behavioral grip improved without line-coverage movement
```

```text
Coverage delta: +4.2%
New policy-eligible RIPR gaps: 1
Interpretation: more code ran, but a changed behavior still lacks a visible discriminator
```

Do not use coverage movement as proof that tests would catch the changed
behavior. Do not use a flat coverage delta as evidence that a focused test was
useless.

When `ripr coverage-grip frontier` is available, inspect:

- `covered_with_ripr_gap`;
- `covered_without_ripr_gap`;
- `uncovered_with_ripr_gap`;
- `uncovered_without_ripr_gap`;
- `coverage_delta_percent`;
- `ripr_visible_unresolved_delta`;
- `interpretation`;
- `warnings[]`.

If coverage input is absent, the ledger can still describe RIPR movement. If
coverage input is present but unsupported, keep the warning visible and avoid
coverage/grip claims.

## History And Receipts

The ledger may read prior PR ledger history from `.ripr/pr-evidence-ledger.jsonl`
or another configured artifact. History turns one PR card into an adoption
record:

```text
records: 42
waiver_age_max_days: 14
baseline_resolved_total: 45
new_policy_eligible_total: 3
trend: improving
```

Use history to ask:

- Are waivers aging without repair?
- Is baseline debt shrinking?
- Are new policy-eligible gaps recurring in the same area?
- Are repair receipts showing `improved` movement?
- Is coverage moving differently from behavioral grip?

History should not become hidden scoring. It is a review aid over visible
artifacts.

## CI Checklist

Before relying on the ledger in review, make sure the PR summary answers:

- What new policy-eligible debt appeared?
- What baseline debt was resolved?
- What acknowledged or suppressed findings remain visible?
- What is the top focused test to add?
- What command verifies evidence movement?
- Does coverage/grip frontier data exist?
- Is any pass/fail result coming from `ripr gate evaluate`, not the ledger?

If those answers are unclear, improve the summary or artifacts before
promoting stricter gate modes.

## Related Docs

- [Baseline ledger workflow](BASELINE_LEDGER_WORKFLOW.md) explains reviewed
  baselines, baseline deltas, and shrink-only refreshes.
- [RIPR Zero reporting workflow](RIPR_ZERO_REPORTING_WORKFLOW.md) explains
  repo-level status, metadata health, repair routes, and movement toward RIPR
  0.
- [Calibrated gate policy](CALIBRATED_GATE_POLICY.md) defines gate modes and
  acknowledgement behavior.
- [RIPR blocking readiness](BLOCKING_READINESS.md) explains when a repository
  is ready to move from advisory to stricter configured gates.
- [CI strategy](CI.md#generated-github-workflow) describes generated workflow
  projection into GitHub job summaries and artifacts.
- [Output schema](OUTPUT_SCHEMA.md#pr-evidence-ledger) defines the
  `pr-evidence-ledger` JSON and Markdown contracts.
- [Coverage / Grip Frontier Report](OUTPUT_SCHEMA.md#coverage--grip-frontier-report)
  defines the optional frontier report.
- [RIPR-SPEC-0018](specs/RIPR-SPEC-0018-pr-evidence-ledger.md) records the PR
  evidence ledger contract.
