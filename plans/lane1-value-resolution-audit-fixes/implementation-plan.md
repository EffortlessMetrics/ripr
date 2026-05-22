# Lane 1 Value Resolution Audit Fixes Implementation Plan

Status: active
Owner: Lane 1
Linked proposal: n/a
Linked specs:
- `docs/specs/RIPR-SPEC-0001-static-exposure-loop.md`
- `docs/specs/RIPR-SPEC-0021-evidence-record.md`
- `docs/specs/RIPR-SPEC-0045-finding-to-gap-alignment.md`
Linked ADRs: n/a
Active goal: `.ripr/goals/active.toml`
GitHub issue: [ripr-swarm #285](https://github.com/EffortlessMetrics/ripr-swarm/issues/285)

## Current State

The Lane 1 Finding Alignment Burn-Down is closed. Its closeout says future
Lane 1 work must be selected from fresh audit, scorecard, dogfood receipt,
downstream consumer issue, or explicit spec-backed campaign evidence.

The current evidence-quality scorecard names two high-value repair routes:

- `analysis/related-test-ranking-audit-fixes` for
  `call_presence` / `activation_owner_call_unresolved`;
- `analysis/value-resolution-audit-fixes` for
  `predicate_boundary` / `activation_value_unresolved`.

This campaign selects the value-resolution route because it is a continuation
of the top remaining named limitation bucket recorded by the closeout and it
directly improves predicate-boundary repairability, where the current
actionable canonical gaps already concentrate.

This rail must not treat a named static limitation as user test debt. It should
move a limitation only when fixture-backed analyzer evidence can safely name a
concrete activation value. Unsupported flows remain named limitations.

## Work Item: `docs/lane1-value-resolution-audit-fixes-stack`

Status: done
Blocks:
- `fixtures/value-resolution-audit-corpus`
Blocked by: n/a

### Goal

Open the issue-backed Lane 1 value-resolution audit-fixes rail and leave the
first behavior-bearing work item ready.

### Production Delta

Docs and goal-manifest wiring only:

- `.ripr/goals/active.toml`
- this implementation plan
- Lane 1 tracker and docs indexes
- implementation campaign and campaign map entries

### Evidence Delta

The selected successor now points at:

- the closed Lane 1 finding-alignment closeout;
- the current evidence-quality scorecard;
- ripr-swarm issue #285.

### Non-Goals

- no analyzer behavior;
- no output schema or public badge semantics;
- no PR/CI/editor/gate/release behavior;
- no generated tests;
- no source edits;
- no provider/model calls;
- no mutation execution.

### Acceptance

- Active manifest names `lane1-value-resolution-audit-fixes`.
- The campaign is listed in `docs/IMPLEMENTATION_CAMPAIGNS.md`.
- The plan and lane tracker are indexed.
- `fixtures/value-resolution-audit-corpus` is the only ready behavior work item.

### Proof Commands

```bash
cargo xtask goals status
cargo xtask goals next
cargo xtask check-goals
cargo xtask check-doc-index
cargo xtask markdown-links
cargo xtask check-static-language
cargo xtask check-pr
git diff --check
```

### Rollback

Restore `.ripr/goals/active.toml` to the closed no-current-goal state and
remove the new plan/tracker/index references. Leave issue #285 open only if a
new successor selection PR will replace this one.

### Notes

This work item is marked done by the activation PR itself. It does not mean the
value-resolution behavior is implemented.

## Work Item: `fixtures/value-resolution-audit-corpus`

Status: done
Blocks:
- `analysis/value-resolution-supported-subshape`
Blocked by: n/a

### Goal

Pin the first selected value-resolution sub-shape before analyzer behavior
changes.

### Production Delta

Add or update fixture/benchmark cases only. The PR should identify one narrow
supported shape from audit evidence and cover:

- positive cases where concrete activation values are statically visible;
- negative guards where values are helper-built, cross-file, generated,
  macro-expanded, shadowed, pattern-bound, non-literal, or otherwise opaque;
- expected canonical item, gap state, actionability, static limitation, repair
  route, and verify-command behavior.

### Evidence Delta

The fixture corpus makes the selected value-resolution shape reviewable before
classification behavior changes.

### Non-Goals

- no analyzer behavior change;
- no broad semantic value analysis;
- no promotion of unsupported flows;
- no user-facing output claim change.

### Acceptance

- Positive and negative cases are fixture-backed.
- Unsupported value flows remain named static limitations.
- Predicate-boundary actionability still requires concrete activation values.

### Proof Commands

```bash
cargo test -p xtask evidence_quality_benchmark
cargo xtask check-fixture-contracts
cargo xtask check-output-contracts
cargo xtask check-pr
git diff --check
```

### Rollback

Remove the added fixture cases and revert any expected-output references. No
production behavior should need rollback in this slice.

### Notes

The fixture PR should name the selected sub-shape explicitly in its PR body and
leave unsupported shapes as future work.

## Work Item: `analysis/value-resolution-supported-subshape`

Status: done
Blocks:
- `report/value-resolution-audit-delta`
Blocked by:
- `fixtures/value-resolution-audit-corpus`

### Goal

Move only the fixture-backed supported value-resolution sub-shape out of
`activation_value_unresolved`.

### Production Delta

No production analyzer delta was needed in this slice: the current
value-resolution path already supports the fixture-backed same-test struct
literal projection shape with source-order and mutation guards. Unsupported
value flows remain named limitations and RIPR still does not infer concrete
activation values from presentation text or opaque helper behavior.

### Evidence Delta

Focused value-resolution tests, the Lane 1 audit, and the evidence-quality
scorecard confirm the existing implementation satisfies the pinned fixtures.
The follow-up audit-delta slice records whether the sampled scorecard moved or
whether the selected sub-shape was already accounted for.

### Non-Goals

- no cross-file/HIR/deep semantic value solver;
- no generated tests;
- no source edits;
- no PR/CI/editor/gate/badge behavior changes;
- no mutation execution.

### Acceptance

- Supported fixtures are not reported as `activation_value_unresolved`.
- Negative guards stay limited.
- Raw findings remain supporting evidence.
- Canonical items remain the countable evidence unit.

### Proof Commands

```bash
cargo test -p ripr value_resolution --lib
cargo xtask lane1-evidence-audit
cargo xtask evidence-quality-scorecard
cargo xtask check-output-contracts
cargo xtask check-pr
git diff --check
```

### Rollback

Revert the analyzer support and expected-output changes together. The fixtures
should remain only if they still express the limitation as expected behavior.

### Notes

This was dispositioned as a proof/status PR because the fixture-backed
sub-shape was already implemented before the fixture corpus landed. Do not
broaden the slice after seeing nearby unsupported shapes. Open a follow-up if
the audit reveals a second safe sub-shape.

## Work Item: `report/value-resolution-audit-delta`

Status: ready
Blocks:
- `dogfood/value-resolution-receipts`
Blocked by:
- `analysis/value-resolution-supported-subshape`

### Goal

Record the evidence movement caused by the supported sub-shape.

### Production Delta

Report or documentation evidence only. Refresh generated reports only when the
repo pattern calls for checked-in outputs; otherwise record commands and
numbers in a handoff.

### Evidence Delta

The before/after audit, scorecard, and trend should make the moved limitation
count and remaining unsupported shapes visible.

### Non-Goals

- no score redefinition;
- no public badge semantic change;
- no downstream projection changes.

### Acceptance

- The delta names the selected sub-shape.
- Remaining `activation_value_unresolved` classes remain visible.
- The report does not imply runtime, mutation, or coverage adequacy.

### Proof Commands

```bash
cargo xtask lane1-evidence-audit
cargo xtask evidence-quality-scorecard
cargo xtask evidence-quality-trend
cargo xtask check-output-contracts
cargo xtask check-pr
git diff --check
```

### Rollback

Remove the delta note or refreshed report references. Do not revert analyzer
work from this reporting PR.

### Notes

This slice should not be used to hide a small or zero movement result. If the
movement is zero, record why and decide whether to stop or choose a different
sub-shape.

## Work Item: `dogfood/value-resolution-receipts`

Status: blocked
Blocks:
- `campaign/value-resolution-audit-closeout`
Blocked by:
- `report/value-resolution-audit-delta`

### Goal

Record material dogfood receipts for the value-resolution movement.

### Production Delta

Evidence only. Add handoff or dogfood receipt artifacts that show the selected
case, command path, before/after state, remaining limitations, and non-claims.

### Evidence Delta

Receipts connect the analyzer movement to canonical gap identity and downstream
consumer expectations.

### Non-Goals

- no behavior change;
- no new public claim;
- no generated tests;
- no provider/model calls.

### Acceptance

- Receipts include canonical gap id, raw finding context, before/after movement,
  repair route or limitation state, verify command where applicable, and
  remaining unsupported shapes.

### Proof Commands

```bash
cargo xtask dogfood
cargo xtask check-doc-index
cargo xtask markdown-links
cargo xtask check-static-language
cargo xtask check-pr
git diff --check
```

### Rollback

Remove the receipt artifacts and index links. Leave behavior and fixtures in
place.

### Notes

Dogfood receipts should preserve that static movement is not mutation proof.

## Work Item: `campaign/value-resolution-audit-closeout`

Status: blocked
Blocks: n/a
Blocked by:
- `dogfood/value-resolution-receipts`

### Goal

Close the rail with proof, remaining limitations, and the next evidence-class
selection rule.

### Production Delta

Closeout and manifest updates only.

### Evidence Delta

The closeout should record:

- selected sub-shape;
- validation commands;
- moved counts;
- no-move or warning states if any;
- downstream consumer impact;
- remaining `activation_value_unresolved` and other static limitations;
- next recommended Lane 1 route.

### Non-Goals

- no analyzer changes;
- no downstream projection changes;
- no gate or release behavior.

### Acceptance

- Active manifest is closed with either `successor` or `no_current_goal = true`.
- Closeout links all proof artifacts.
- The next ready work is discoverable or intentionally absent.

### Proof Commands

```bash
cargo xtask goals status
cargo xtask goals next
cargo xtask check-doc-index
cargo xtask markdown-links
cargo xtask check-static-language
cargo xtask check-pr
git diff --check
```

### Rollback

Reopen the active manifest to the last incomplete work item and remove the
closeout handoff if the closeout landed prematurely.

### Notes

Do not keep adding value-resolution changes under this rail after closeout
without a fresh issue-backed successor.
