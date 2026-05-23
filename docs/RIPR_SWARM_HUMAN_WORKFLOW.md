# RIPR Swarm Human Workflow

Use this workflow when a human or external coding agent needs to attempt one
bounded repair from RIPR's actionable canonical gap packets.

The loop is:

```text
actionable canonical packet
-> dry-run attempt context
-> manual or agent repair attempt
-> verify command
-> receipt command
-> actionable-gap outcome report
-> scorecard or trend delta
```

This is a repair-coordination workflow, not an autonomous code-writing system.
It consumes `actionable-gaps.json`, not raw findings. Raw findings remain
supporting evidence only.

## Inputs

Generate or refresh Lane 1 evidence first:

```bash
rtk cargo xtask lane1-evidence-audit
rtk cargo xtask evidence-health
rtk cargo xtask evidence-quality-scorecard
rtk cargo xtask evidence-quality-trend
rtk cargo xtask ripr-swarm plan --top 10
rtk cargo xtask actionable-gap-outcomes
rtk cargo xtask ripr-swarm readiness
```

Run these Cargo-backed report commands sequentially in one worktree. They share
the Cargo target directory and `target/ripr/reports`; overlapping runs can cause
lock contention or stale report reads. If parallel validation is necessary,
isolate both `CARGO_TARGET_DIR` and report output paths.

The workflow reads these artifacts:

| Artifact | Purpose |
| --- | --- |
| `target/ripr/reports/actionable-gaps.json` | Canonical actionable packet source. |
| `target/ripr/reports/actionable-gaps.md` | Human-readable packet list. |
| `target/ripr/reports/swarm-plan.json` | Ranked ready and blocked packet plan. |
| `target/ripr/reports/swarm-plan.md` | Human-readable top-N repair plan. |
| `target/ripr/reports/actionable-gap-outcomes.json` | Receipt and evidence-movement join. |
| `target/ripr/reports/actionable-gap-outcomes.md` | Human-readable attempt outcomes. |

Do not use raw `raw_findings[]` as a work queue. They exist to explain and
audit the canonical packet.

## 1. Pick One Packet

Open `target/ripr/reports/swarm-plan.md` and choose one packet from the
swarm-ready list.

Prefer packets with:

- `swarm_state = queued`;
- a concrete `repair_kind`;
- confidence stronger than `static_only` for predicate-boundary assertion
  packets;
- a related test or observer;
- `verify_command`;
- `receipt_command` or `receipt_command_or_path`;
- non-empty `must_not_change`;
- no static limitations.

Do not attempt packets marked:

- `blocked_by_missing_context`;
- `blocked_by_static_limitation`;
- `blocked_by_operator_judgment`;
- missing verify command;
- missing receipt command;
- missing must-not-change boundaries.

Blocked packets are still useful evidence. They are not repair-ready work.

## 2. Print The Dry-Run Context

Run the attempt dry-run for the selected packet:

```bash
rtk cargo xtask ripr-swarm attempt --packet <packet_id> --dry-run
```

If you are using a non-default packet file:

```bash
rtk cargo xtask ripr-swarm attempt \
  --packet <packet_id> \
  --dry-run \
  --actionable-gaps target/ripr/reports/actionable-gaps.json
```

The dry-run prints:

- canonical gap id;
- evidence class;
- swarm state;
- confidence basis;
- repair kind;
- repair route;
- target test type;
- assertion or observer shape;
- related test or observer;
- expected evidence movement;
- verify command;
- receipt command or path;
- must-not-change boundaries;
- raw finding and static limitation counts.

The dry-run does not edit files, run tests, call providers, generate tests,
create receipts, run mutation testing, merge code, change gates, or change
public badge semantics.

## 3. Apply One Bounded Repair

Make the smallest test or observer change that matches the packet.

Common repair kinds:

| Repair kind | Human action |
| --- | --- |
| `add_boundary_assertion` | Add or strengthen one equality-boundary assertion in the related test. |
| `add_exact_error_variant` | Replace broad `is_err` style checks with an exact error variant assertion. |
| `add_output_observer` | Add or update a help-output, report, snapshot, or golden observer. |
| `add_snapshot_or_golden` | Add or update the named snapshot or golden output. |
| `add_side_effect_observer` | Add an assertion for the named event, state, mock, or effect. |
| `inspect_visibility` | Inspect the named path and either add observer proof or leave a limitation. |

Stay inside `must_not_change`. The default is no production-code edit. If a
repair requires production-code changes, stop and split that into a separate
reviewed work item.

## 4. Using An External Agent Safely

Give the agent the dry-run output and the matching packet only.
The full handoff contract is defined by
[`RIPR-SPEC-0058`](specs/RIPR-SPEC-0058-ripr-swarm-external-agent-handoff.md).

The agent may:

- edit the target test or observer named by the packet;
- add one narrow fixture or assertion when the packet asks for it;
- run the packet's verify command;
- run the packet's receipt command;
- return a patch plus receipt path.

The agent must not:

- consume raw findings as separate tasks;
- invent repairs outside `repair_kind` and `repair_route`;
- edit production code by default;
- ignore `must_not_change`;
- call providers from repo automation;
- generate broad tests without operator approval;
- retry indefinitely without an operator decision;
- claim success from a passing test without receipt-backed evidence movement.

## 5. Verify The Attempt

Run the packet's verify command exactly, unless the operator deliberately
chooses a narrower local equivalent and records that choice in the PR.

Examples:

```bash
rtk cargo test -p ripr <related_test_name>
rtk cargo xtask evidence-quality-scorecard
rtk cargo xtask lane1-evidence-audit
```

A passing verify command means the repository accepted the attempted change. It
does not by itself prove the canonical gap improved.

## 6. Emit The Receipt

Run the packet's receipt command after verification:

```bash
<receipt_command>
```

If the packet provides `receipt_command_or_path`, follow that field. If the
receipt cannot be produced, keep the attempt visible as `attempted_no_receipt`
instead of retrying silently.

## 7. Read Outcomes

Refresh the outcome report:

```bash
rtk cargo xtask actionable-gap-outcomes
```

Read:

```text
target/ripr/reports/actionable-gap-outcomes.json
target/ripr/reports/actionable-gap-outcomes.md
```

Outcome states:

| State | Meaning |
| --- | --- |
| `not_attempted` | No receipt or evidence-movement record is joined yet. |
| `attempted_no_receipt` | A repair was attempted but no receipt was produced. |
| `receipt_present` | Receipt exists, but evidence movement is not classified as improved, unchanged, or regressed. |
| `evidence_improved` | Receipt-backed evidence moved in the expected direction. |
| `evidence_unchanged` | The attempt did not improve the canonical item. |
| `evidence_regressed` | Evidence moved backward or created a worse state. |
| `resolved` | The canonical actionable gap no longer appears as unresolved work. |
| `unknown` | The report cannot classify the attempt safely. |

Failed, unchanged, regressed, and orphaned receipts remain visible. They are
part of the evidence trail.

## 8. Record The PR Evidence

The PR should say:

- packet id and canonical gap id;
- repair kind;
- target test or observer changed;
- verify command and result;
- receipt command and receipt path;
- outcome state;
- scorecard or trend delta when available;
- any blocked or unknown state that remains.

Do not say the gap is fixed unless the outcome report shows `resolved` or
receipt-backed `evidence_improved` for the relevant canonical item.

## Stop Conditions

Stop and ask for operator review when:

- the packet is blocked;
- the repair needs production-code changes;
- `must_not_change` conflicts with the needed edit;
- verify command is missing or fails for reasons unrelated to the repair;
- receipt command is missing or cannot be produced;
- evidence is unchanged after a plausible repair;
- evidence regresses;
- the packet points through a static limitation.

The useful end state is a small, reviewable attempt:

```text
one packet
one bounded repair
one verify command
one receipt
one outcome state
```
