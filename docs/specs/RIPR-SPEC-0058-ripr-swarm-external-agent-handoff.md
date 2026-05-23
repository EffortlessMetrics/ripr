# RIPR-SPEC-0058: RIPR Swarm External Agent Handoff

Status: accepted

Accepted: 2026-05-20 as the external-agent boundary for the implemented 0.7
swarm repair loop. This remains a documentation contract over one bounded
operator-mediated packet; it does not add provider SDKs, autonomous repair,
source-edit authority, or merge authority.

## Problem

`ripr-swarm` can rank actionable canonical gap packets, print a bounded
dry-run attempt context, and join receipts to evidence movement. The next
automation boundary is not a provider SDK. It is a stable handoff contract for
an operator who wants to give exactly one swarm-ready packet to an external
coding agent and review the result.

Without this contract, an agent can drift back to the unsafe workflow:

```text
read raw findings
infer a repair
edit broad files
claim success from a passing test
lose the receipt and evidence movement trail
```

The external-agent handoff keeps the existing Lane 1 invariant intact:

```text
Raw findings are evidence.
Canonical evidence items are the countable unit.
Actionable canonical gaps are user work.
Swarm packets are bounded repair attempts against those gaps.
Receipts and evidence movement decide whether the attempt worked.
```

## Scope

This spec defines the producer and consumer contract for handing one
`ripr-swarm` packet to an external agent.

It covers:

- what the operator may send to the agent;
- what the agent may change;
- what the agent must return;
- how verify and receipt commands close the loop;
- how failed, unchanged, regressed, blocked, or unknown attempts remain visible.

It does not add a provider SDK, autonomous repair runner, autonomous merge
path, production-code edit authority, generated-test writer, retry loop, PR/CI
renderer, LSP/editor surface, gate policy, public badge change, or mutation
execution.

## Behavior

The handoff behavior is a bounded operator-mediated exchange:

```text
swarm-ready packet
-> external-agent request
-> scoped patch attempt
-> verify command
-> receipt command or receipt absence
-> operator review
-> actionable-gap outcome join
```

The external agent receives one packet and returns one attempt result. It does
not select additional work from raw findings, Markdown, PR annotations, or
static class labels. If the packet is blocked, incomplete, or static-limited,
the agent returns a blocked result instead of attempting a repair.

## Input Contract

The operator sends exactly one packet selected from:

```text
target/ripr/reports/actionable-gaps.json
target/ripr/reports/swarm-plan.json
```

The selected packet must already be a canonical actionable packet. The external
agent must not receive raw findings as a work queue.

Required handoff fields are:

| Field | Requirement |
| --- | --- |
| `packet_id` or `canonical_gap_id` | Stable identity for the attempted repair and receipt join. |
| `evidence_class` | Class-specific repair context and limitation boundary. |
| `gap_state` | Must indicate an unresolved actionable gap. |
| `repair_kind` | The only allowed repair family for the attempt. |
| `repair_route` | Structured repair route. Prose-only guidance is not enough. |
| `target_test_type` or `target_assertion_shape` | Required when applicable to bound the test or observer edit. |
| `related_test_or_observer` | Preferred target. Missing context must be called out to the operator. |
| `verify_command` | Command the agent or operator runs after the patch. |
| `receipt_command` or `receipt_command_or_path` | Command or path used to produce the attempt receipt. |
| `must_not_change[]` | Explicit boundaries the agent must preserve. |
| `confidence_basis` | Basis for why the packet is safe enough to attempt. |
| `static_limitations[]` | Must be empty for an unblocked attempt, or the packet stays blocked. |
| `raw_findings[]` | Supporting evidence only. Never split into separate tasks. |

If any required field is missing, the packet is not agent-ready. The operator
should regenerate upstream artifacts or leave the packet in
`blocked_by_missing_context`.

## Required Evidence

Before the operator hands a packet to an external agent, typed evidence must
show:

- the packet is a canonical actionable gap;
- the selected `canonical_gap_id` is stable and present;
- the repair route is structured and names a repair kind;
- verify and receipt commands are present;
- `must_not_change[]` is present;
- static limitations are absent or the packet is treated as blocked;
- raw findings are attached only as supporting evidence.

Before the operator accepts an agent attempt as improved, typed evidence must
show:

- the attempted packet identity;
- patch metadata and files changed;
- verify command status;
- receipt command status or explicit receipt absence;
- evidence movement joined to the same canonical gap;
- no default-scope production-code edit boundary violation.

## Agent Request Packet

An external-agent request should include a bounded packet, not the whole repo
audit. The preferred request shape is:

```json
{
  "schema_version": "0.1",
  "tool": "ripr-swarm",
  "kind": "external_agent_handoff",
  "packet": {
    "packet_id": "gap:parser:missing-input",
    "canonical_gap_id": "gap:parser:missing-input",
    "evidence_class": "error_path",
    "gap_state": "actionable",
    "repair_kind": "add_exact_error_variant",
    "repair_route": {
      "repair_kind": "add_exact_error_variant",
      "target_test_type": "unit_test",
      "assertion_shape": "matches!(..., Err(Error::Missing))"
    },
    "related_test_or_observer": {
      "file": "tests/parser.rs",
      "name": "rejects_missing_input"
    },
    "verify_command": "cargo test -p ripr rejects_missing_input",
    "receipt_command": "ripr agent receipt --root . --verify-json target/ripr/workflow/agent-verify.json --seam-id gap:parser:missing-input --json --out target/ripr/reports/agent-receipt.json",
    "must_not_change": ["production parser code"],
    "confidence_basis": "fixture_backed",
    "static_limitations": [],
    "raw_findings": [
      {"file": "src/parser.rs", "line": 42, "kind": "weakly_exposed"}
    ]
  }
}
```

Markdown may accompany the packet for human explanation. JSON remains the
contract. The agent must not parse Markdown to discover additional tasks.

## Agent Responsibilities

The agent may:

- edit the target test, assertion, snapshot, golden, or observer named by the
  packet;
- add one narrow fixture when the packet repair route asks for it;
- run the packet's verify command;
- run or report the packet's receipt command;
- return patch metadata, verify result, receipt metadata, and blocked reasons.

The agent must:

- obey `must_not_change[]`;
- stay within `repair_kind` and `repair_route`;
- keep raw findings as supporting evidence only;
- stop if the repair requires production-code edits under the default contract;
- stop if the packet has static limitations or missing required fields;
- report failed, unchanged, regressed, blocked, and unknown attempts;
- avoid silent retries and broad rewrites.

The agent must not:

- consume raw findings as separate work items;
- invent repairs outside the packet;
- edit production code by default;
- call provider or model APIs from repo automation;
- generate broad tests without explicit operator approval;
- run mutation testing;
- merge, push, or publish autonomously;
- claim success from a passing test without receipt-backed evidence movement.

## Agent Response Contract

The agent response is an operator-review packet. It is not an automatic merge
request by itself.

Preferred fields:

```text
packet_id
canonical_gap_id
attempt_id
files_changed[]
patch_summary
verify_command
verify_status
receipt_command
receipt_path
outcome_state
blocked_reason
must_not_change_observed
production_code_changed
notes
```

Allowed `verify_status` values:

```text
not_run
passed
failed
timed_out
unknown
```

Allowed `outcome_state` values mirror the outcome report:

```text
not_attempted
attempted_no_receipt
receipt_present
evidence_improved
evidence_unchanged
evidence_regressed
resolved
unknown
```

If `production_code_changed = true`, the default external-agent handoff has
left its safe scope. The operator must split the work into a separately
reviewed production-code change or explicitly authorize that broader path
outside `ripr-swarm`.

## Operator Review Contract

The operator reviews the patch before commit, merge, or follow-up automation.

The review checks:

- one packet was attempted;
- changed files fit the packet and `must_not_change[]`;
- production code was not edited by default;
- verify command was run or the absence is named;
- receipt command was run or the absence is named;
- outcome state is recorded;
- unchanged, regressed, failed, or unknown attempts remain visible.

The operator may accept, edit, reject, or split the patch. The agent response
does not make merge or gate decisions.

## Receipt and Outcome Join

After review, the attempt is measured through existing artifacts:

```text
target/ripr/reports/actionable-gap-outcomes.json
target/ripr/reports/actionable-gap-outcomes.md
```

The join keys are:

```text
canonical_gap_id
packet_id
receipt_id or receipt_path
verify_command
evidence_movement record
```

The same canonical gap may have multiple attempts. The latest attempt may be
highlighted, but prior failed, unchanged, regressed, or orphaned attempts stay
visible.

An orphaned receipt remains evidence. It does not create a new actionable gap.

## Security and Boundary Rules

External-agent handoff is deny-by-default.

- The repo does not pass secrets, tokens, credentials, or private environment
  values in the packet.
- The packet should include only paths and commands already present in local
  RIPR artifacts.
- Commands are copied for operator review; the handoff spec does not execute
  them automatically.
- Network access, provider calls, and model calls are outside this contract.
- If a future provider integration is proposed, it needs its own ADR and
  security review before implementation.

## Non-Goals

This spec does not add:

- provider or model integration;
- autonomous code generation or autonomous source edits;
- autonomous merge, push, release, publish, or CI gate behavior;
- production-code edits by default;
- raw-finding work queues;
- generated tests without explicit operator action;
- mutation execution;
- retry loops without operator bounds;
- PR/CI rendering;
- LSP/editor projection;
- public badge changes.

## Acceptance Examples

Given a swarm-ready boundary assertion packet with a related test, verify
command, receipt command, and `must_not_change = ["production pricing code"]`,
the operator may hand that one packet to an external agent. The agent may edit
the named test, run verification, emit a receipt, and return patch plus receipt
metadata for review.

Given a packet whose only evidence is `raw_findings[]`, the operator must not
create an agent task. The packet is incomplete until canonical actionability,
repair route, verify command, and receipt command are present.

Given a packet with `blocked_by_static_limitation`, the operator must not hand
it off as repair-ready. The next action is analyzer support or manual
inspection, not an agent test-writing task.

Given an agent patch that changes production code while the packet forbids it,
the operator rejects or splits the patch. The attempt outcome is not
`verified_improved` until receipt-backed evidence movement is recorded within
scope.

Given a verify command that passes but no receipt can be produced, the attempt
is `attempted_no_receipt` or `unknown`. It is not resolved.

Given receipt-backed evidence movement `evidence_unchanged`, the attempt stays
visible as unchanged. The operator may choose a new attempt, but the swarm must
not silently retry forever.

## Test Mapping

This is a documentation-only contract. Current adjacent coverage lives in
`RIPR-SPEC-0057`:

- `xtask::tests::ripr_swarm_plan_ranks_ready_packets_and_blocks_missing_context`
  pins that missing verify, missing receipt, and static-limitation packets do
  not become ready work;
- `xtask::tests::ripr_swarm_attempt_dry_run_renders_bounded_packet_context`
  pins the dry-run context an operator may hand to an external agent;
- `xtask::tests::ripr_swarm_attempt_dry_run_reports_blocked_packet_context`
  pins blocked packet visibility;
- `xtask::tests::actionable_gap_outcomes_fixture_corpus_matches_expected_states`
  pins receipt-backed outcome states.

Future implementation PRs that emit external-agent handoff artifacts should add
fixtures for request packet shape, agent response shape, production-code
boundary violations, missing receipts, and unchanged or regressed attempts.

## Implementation Mapping

Implemented prerequisites:

- `cargo xtask ripr-swarm plan --top <n>`;
- `cargo xtask ripr-swarm attempt --packet <id> --dry-run`;
- `target/ripr/reports/swarm-plan.json`;
- `target/ripr/reports/swarm-plan.md`;
- `target/ripr/reports/actionable-gap-outcomes.json`;
- `target/ripr/reports/actionable-gap-outcomes.md`;
- `docs/RIPR_SWARM_HUMAN_WORKFLOW.md`.

This spec is documentation-only. It does not add a command, schema file,
provider adapter, runner, or generated patch surface. Future implementation may
add a report artifact for external-agent handoff packets, but that requires a
separate scoped PR with fixtures and output schema documentation.

## Metrics

Future readiness reporting should be able to count:

- `swarm_agent_ready_packets`;
- `swarm_agent_handoff_packets`;
- `swarm_agent_blocked_packets`;
- `swarm_agent_attempted_packets`;
- `swarm_agent_attempts_without_receipt`;
- `swarm_agent_verified_improved`;
- `swarm_agent_verified_unchanged`;
- `swarm_agent_verified_regressed`;
- `swarm_agent_production_code_boundary_violations`;
- `swarm_agent_orphaned_receipts`.
