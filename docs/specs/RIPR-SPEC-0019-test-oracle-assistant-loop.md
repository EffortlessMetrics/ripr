# RIPR-SPEC-0019: Test-Oracle Assistant Loop

Status: proposed

## Problem

RIPR now has the pieces needed for PR-time behavioral test guidance:

- static evidence for changed Rust behavior;
- bounded PR guidance and changed-line-safe annotations;
- saved-workspace editor diagnostics, hover, and code actions;
- agent handoff packets and verification commands;
- recommendation calibration and local outcome receipts;
- optional calibrated gates, baselines, RIPR Zero status, PR evidence ledgers,
  and coverage/grip frontier reports.

Those pieces are useful individually, but the product promise depends on one
review loop that a developer, reviewer, or coding agent can follow without
downloading unrelated artifacts or learning internal report topology.

The loop should answer:

```text
For this changed behavior, what focused test would most improve static RIPR
evidence, how do I hand that task to a human or agent, and which receipt shows
the before/after movement?
```

## Product Contract

The test-oracle assistant loop is an evidence-preserving workflow over existing
RIPR artifacts. It does not create new analyzer semantics, rank findings with a
new policy model, edit source, generate tests, call an LLM provider, run
mutation testing, post PR comments, or make CI blocking by default.

The loop must:

- start from changed Rust behavior and the static RIPR evidence already emitted
  for that behavior;
- keep static findings in static vocabulary;
- show the top recommendation, missing discriminator, related test, focused
  test shape, and verify command;
- preserve whether guidance was placed on a changed line or fell back to
  summary-only guidance;
- expose a bounded handoff packet for humans or external agents;
- compare before and after static evidence only when both inputs exist;
- record local receipts without telemetry or external service calls;
- project the result into advisory PR/CI summaries, ledgers, and artifacts;
- keep optional gate decisions separate from proof-loop evidence;
- keep imported runtime mutation data explicit when present and absent when not
  supplied.

## Behavior

The canonical loop is:

```text
changed Rust behavior
-> static RIPR evidence
-> PR or editor recommendation
-> focused-test handoff packet
-> human or external agent adds one focused test
-> after static evidence
-> verification and receipt
-> advisory PR/CI projection
```

Each stage has an owner:

| Stage | Owner | Required artifact |
| --- | --- | --- |
| Changed behavior | user / PR diff | diff, base/head revisions, or fixture diff |
| Static evidence | RIPR analyzer | check, repo exposure, pilot, or PR guidance JSON |
| Recommendation | PR guidance or editor evidence | changed-line comment or summary-only entry |
| Handoff | editor action or `ripr agent start` | agent brief, workflow packet, or evidence context |
| Focused test | human or external agent | source change outside RIPR automation |
| After evidence | RIPR analyzer | after repo exposure or pilot output |
| Receipt | RIPR receipt command | targeted-test outcome or agent receipt |
| PR/CI projection | generated CI / reports | summary, ledger, gate decision, frontier report |

## Canonical Proof Case

The first proof case is the product-in-miniature boundary example from the
roadmap:

```rust
if amount >= discount_threshold {
    apply_discount(...)
}
```

Existing tests reach the pricing behavior but do not discriminate the equality
boundary. The expected recommendation is:

```text
Add a focused test where amount == discount_threshold and assert the exact
discount_applied, discount_amount, and total outputs.
```

The proof case should connect these facts:

- changed seam: predicate boundary at the pricing branch;
- missing discriminator: `amount == discount_threshold`;
- related test: an existing above-threshold or below-threshold pricing test;
- focused test shape: equality-boundary arrange/act/assert;
- before evidence: weak or missing discrimination for the boundary;
- after evidence: improved or resolved static movement when the focused test is
  present;
- receipt: local artifact naming the seam, before/after inputs, movement, and
  limits;
- PR/CI projection: advisory summary and ledger movement that point to the
  receipt.

## Required Evidence

The proof loop must preserve these inputs and outputs when present:

- PR identity, base revision, head revision, or fixture identity;
- selected seam identity, file, line, class, missing discriminator, and
  related-test context;
- PR guidance or editor evidence that names the recommendation and placement
  state;
- handoff packet path and command payload for the selected seam;
- before static evidence artifact;
- after static evidence artifact;
- verification command;
- outcome or agent receipt artifact;
- PR evidence ledger path when available;
- coverage/grip frontier path when available;
- gate decision path when explicitly configured;
- warnings for missing, stale, ambiguous, unsupported, or summary-only inputs;
- limits text preserving static-evidence and advisory-default boundaries.

Missing optional artifacts must be reported as `unknown`, `not_available`, or a
warning. Missing artifacts must not be treated as success, acknowledgement,
suppression, or runtime confirmation.

## Proof Report Contract

A future producer may write:

```text
target/ripr/reports/test-oracle-assistant-proof.json
target/ripr/reports/test-oracle-assistant-proof.md
```

The report is advisory and read-only. It joins existing artifacts; it does not
rerun analysis unless a caller explicitly ran the underlying commands first.

Command shape is intentionally deferred to the implementation PR, but it should
accept explicit input paths rather than searching hidden state:

```text
ripr assistant-loop proof \
  --pr-guidance target/ripr/review/comments.json \
  --agent-packet target/ripr/workflow/agent-brief.json \
  --before target/ripr/pilot/repo-exposure.json \
  --after target/ripr/pilot/after.repo-exposure.json \
  --receipt target/ripr/reports/agent-receipt.json \
  --ledger target/ripr/reports/pr-evidence-ledger.json \
  --coverage-frontier target/ripr/reports/coverage-grip-frontier.json \
  --out target/ripr/reports/test-oracle-assistant-proof.json \
  --out-md target/ripr/reports/test-oracle-assistant-proof.md
```

The JSON report uses schema version `0.1`:

```json
{
  "schema_version": "0.1",
  "tool": "ripr",
  "kind": "test_oracle_assistant_loop",
  "status": "advisory",
  "root": ".",
  "inputs": {
    "pr_guidance": "target/ripr/review/comments.json",
    "agent_packet": "target/ripr/workflow/agent-brief.json",
    "before": "target/ripr/pilot/repo-exposure.json",
    "after": "target/ripr/pilot/after.repo-exposure.json",
    "receipt": "target/ripr/reports/agent-receipt.json",
    "ledger": "target/ripr/reports/pr-evidence-ledger.json",
    "coverage_frontier": "target/ripr/reports/coverage-grip-frontier.json"
  },
  "seam": {
    "seam_id": "67fc764ba37d77bd",
    "seam_kind": "predicate_boundary",
    "path": "src/pricing.rs",
    "line": 88,
    "grip_class": "weakly_gripped",
    "missing_discriminator": "amount == discount_threshold"
  },
  "recommendation": {
    "source": "pr_guidance",
    "placement": "changed_line",
    "summary_only_reason": null,
    "suggested_test": "Add an equality-boundary assertion.",
    "related_test": "tests/pricing.rs::applies_discount_above_threshold",
    "verify_command": "ripr agent verify --root . --before target/ripr/pilot/repo-exposure.json --after target/ripr/pilot/after.repo-exposure.json --json"
  },
  "handoff": {
    "source": "agent_packet",
    "artifact": "target/ripr/workflow/agent-brief.json",
    "agent_command": "ripr agent start --root . --seam-id 67fc764ba37d77bd --out target/ripr/workflow",
    "external_provider": false
  },
  "evidence_movement": {
    "state": "improved",
    "before_class": "weakly_gripped",
    "after_class": "strongly_gripped",
    "source": "agent_receipt",
    "artifact": "target/ripr/reports/agent-receipt.json"
  },
  "ci_projection": {
    "ledger": "target/ripr/reports/pr-evidence-ledger.json",
    "coverage_frontier": "target/ripr/reports/coverage-grip-frontier.json",
    "gate_decision": null,
    "pass_fail_authority": "gate decision when explicitly configured"
  },
  "warnings": [],
  "limits": {
    "advisory": true,
    "source_edits": false,
    "generated_tests": false,
    "external_service": false,
    "runtime_mutation_execution": false,
    "ci_blocking_default": false
  }
}
```

Field contract:

- `status` is `advisory` for complete proof records and `incomplete` when the
  selected seam or required before/after evidence is missing.
- `inputs.*` records explicit paths. Missing optional inputs are `null` plus a
  warning.
- `seam.*` is copied from existing RIPR evidence or guidance. It must not be
  recomputed with new identity semantics.
- `recommendation.placement` is `changed_line`, `summary_only`, or `unknown`.
  Summary-only guidance must remain visible.
- `handoff.external_provider` is always `false`; RIPR emits packets but does
  not call a provider.
- `evidence_movement.state` is `improved`, `resolved`, `unchanged`,
  `regressed`, or `unknown`. It is static RIPR movement, not runtime mutation
  confirmation.
- `ci_projection.pass_fail_authority` must keep proof records separate from
  optional gate decisions.
- `limits.*` must preserve the no-edit, no-generated-test, no-provider-call,
  no-runtime-mutation-execution, and advisory-default boundaries.

## Markdown Shape

The Markdown sibling should fit in a PR summary, generated CI job summary, or
dogfood receipt:

```md
# RIPR Test-Oracle Assistant Loop

Status: advisory

Top focused test:
- Seam: src/pricing.rs:88
- Missing discriminator: amount == discount_threshold
- Suggested test: Add an equality-boundary assertion.
- Related test: tests/pricing.rs::applies_discount_above_threshold
- Verify: ripr agent verify --root . --before ... --after ... --json

Movement:
- Before: weakly_gripped
- After: strongly_gripped
- State: improved
- Receipt: target/ripr/reports/agent-receipt.json

Projection:
- PR ledger: target/ripr/reports/pr-evidence-ledger.json
- Coverage/grip frontier: target/ripr/reports/coverage-grip-frontier.json
- Gate: not configured

Limits:
- Static RIPR evidence only.
- Advisory by default.
- No source edits, generated tests, provider calls, or mutation execution.
```

## Acceptance Examples

Given a changed predicate boundary with a missing equality discriminator, the
loop record names the seam, missing discriminator, related test, focused test
shape, and verify command.

Given a recommendation that cannot be safely placed on a changed line, the loop
record keeps the recommendation visible as summary-only guidance and names the
reason when available.

Given before and after static evidence for the same seam, the loop record
reports static movement as `improved`, `resolved`, `unchanged`, `regressed`, or
`unknown`.

Given a `ripr-waive` label or gate acknowledgement, the loop record may link the
gate or ledger artifact, but it must not treat acknowledgement as suppression
or hidden success.

Given imported runtime mutation calibration, the loop may link that artifact as
explicit calibration evidence. Without imported runtime calibration, the loop
must not use runtime mutation vocabulary.

## Test Mapping

The implementation should add tests or fixtures for:

- selecting the canonical boundary-gap seam from PR guidance;
- preserving changed-line and summary-only placement states;
- copying related-test, suggested-test, missing-discriminator, verify-command,
  and agent-command fields from existing artifacts;
- reporting missing optional inputs as warnings or unknowns;
- joining before and after static evidence by seam identity without changing
  analyzer identity semantics;
- rendering `improved`, `resolved`, `unchanged`, `regressed`, and `unknown`
  static movement states;
- linking receipt, PR evidence ledger, coverage/grip frontier, and optional
  gate-decision artifacts without making them pass/fail authority;
- proving the JSON and Markdown shapes for
  `test-oracle-assistant-proof.{json,md}`;
- keeping generated tests, source edits, provider calls, runtime mutation
  execution, and default CI blocking disabled.

## Implementation Mapping

Expected follow-up surfaces:

- `fixtures/canonical-review-loop` adds the canonical before/after fixture or
  replay corpus and expected proof artifacts.
- `dogfood/test-oracle-assistant-receipt` records a repo-local proof receipt
  that traces one seam through guidance, handoff, verification, receipt, ledger,
  and coverage/grip frontier availability.
- `docs/test-oracle-assistant-workflow` documents the user workflow from PR
  summary or editor diagnostic through one focused test and receipt.
- `campaign/test-oracle-assistant-proof-closeout` records the prompt-to-artifact
  audit and remaining advisory limits.

No follow-up surface may change analyzer identity, recommendation ranking, gate
policy semantics, LSP/editor behavior, generated workflow defaults, or public
crate shape as part of this campaign.

## Metrics

The loop should feed these future metrics:

- `test_oracle_assistant_loop_reports`;
- `test_oracle_assistant_loop_verified_receipts`;
- `test_oracle_assistant_loop_unknown_inputs`;
- `test_oracle_assistant_loop_summary_only`;
- `test_oracle_assistant_loop_static_improved`;
- `test_oracle_assistant_loop_static_resolved`;
- `test_oracle_assistant_loop_static_regressed`.

## Non-Goals

- No analyzer behavior changes.
- No analyzer identity rewrites.
- No recommendation ranking changes.
- No gate policy semantic changes.
- No CI blocking by default.
- No generated workflow changes in this spec PR.
- No automatic PR comment posting.
- No source edits.
- No generated tests.
- No LSP or editor behavior changes.
- No LLM provider calls.
- No mutation execution.
- No runtime adequacy claims from static evidence.
- No coverage adequacy claims.
- No public crate split.
