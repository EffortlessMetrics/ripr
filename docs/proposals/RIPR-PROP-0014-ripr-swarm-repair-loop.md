# RIPR-PROP-0014: RIPR Swarm Repair Loop

Status: proposed

Owner: Lane 1 - Evidence Accuracy / ripr-swarm

Created: 2026-05-20

Target campaign: RIPR Swarm Repair Loop

Linked specs:

- `RIPR-SPEC-0057`: RIPR swarm repair loop

Linked ADRs:

- None yet. Add an ADR only if provider integration, source-edit authority, or
  autonomous merge behavior is proposed later.

Linked work items:

- #25 `docs(spec): define ripr-swarm repair loop`
- Follow-up: `report: rank actionable packets for swarm`
- Follow-up: `fixtures: add swarm-plan packet corpus`
- Follow-up: `report: add actionable gap outcome report`
- Follow-up: `xtask: add swarm dry-run`
- Follow-up: `docs: add ripr-swarm human workflow`
- Follow-up: `dogfood: close real actionable packets`

## Problem

Lane 1 now emits actionable canonical gap packets with canonical identity,
repair kind, verify commands, receipt commands, raw findings as supporting
evidence, and must-not-change boundaries. That is enough structure to guide
humans and external agents, but it is not yet a safe repair loop.

Without a swarm contract, the next automation step can regress into the old
failure mode:

```text
raw finding appears
agent guesses repair
test happens to pass
evidence movement is unknown
```

The repair loop needs a bounded middle layer:

```text
actionable canonical packet
-> ranked repair candidate
-> one bounded attempt
-> verify command
-> receipt command
-> evidence movement outcome
```

The product problem is coordination, not detection. `ripr-swarm` should help
choose and track scoped repair attempts without consuming raw findings,
inventing repairs, editing production code by default, calling providers, or
claiming movement without receipt-backed evidence.

## Users and surfaces

Users:

- maintainers triaging the top actionable canonical gaps;
- humans applying one bounded test, assertion, snapshot, or observer repair;
- coding-agent operators handing a safe packet to an external agent;
- reviewers checking whether an attempted repair improved evidence;
- Lane 1 maintainers measuring packet readiness and outcome quality.

Surfaces:

- `target/ripr/reports/actionable-gaps.json`;
- `target/ripr/reports/actionable-gaps.md`;
- `target/ripr/reports/swarm-plan.json`;
- `target/ripr/reports/swarm-plan.md`;
- `target/ripr/reports/actionable-gap-outcomes.json`;
- `target/ripr/reports/actionable-gap-outcomes.md`;
- dry-run xtask commands;
- docs, fixtures, scorecard/trend readiness metrics, and dogfood receipts.

This proposal does not create a new PR/CI, editor/LSP, provider, gate, badge,
or mutation-execution surface.

## Success criteria

- `ripr-swarm` consumes actionable canonical gap packets, not raw findings.
- Top repair selection is based on typed packet fields, including repair route,
  verify command, receipt command, confidence basis, related test or observer,
  must-not-change boundaries, evidence class maturity, expected canonical gap
  delta, and absence of blocking static limitations.
- Packets without verify commands, receipt commands, or safe typed repair
  context are reported as blocked rather than repair-ready.
- Static-limitation packets are named and blocked; they are not ranked as user
  repair work.
- Dry-run planning and attempt commands do not edit files, call providers, run
  mutation testing, generate tests, update PR/CI rendering, change editor/LSP
  behavior, change gates, or change public badges.
- Each attempted repair can produce or explain the absence of a receipt.
- Outcome reports distinguish not attempted, attempted without receipt, receipt
  present, evidence improved, unchanged, regressed, resolved, and unknown.
- Failed, unchanged, and regressed attempts remain visible.
- Dogfood closes 3-5 real packets with before packet, repair, receipt, evidence
  movement, and delta records before any badge or autonomous-agent readiness
  claim.

## Proposed shape

Build `ripr-swarm` as a repo-local, dry-run-first repair coordination loop over
existing actionable packet artifacts:

1. define the behavior spec and proposal rails;
2. rank actionable packets into swarm-ready and blocked groups;
3. fixture-pin high-confidence and blocked packet cases;
4. join receipts and evidence movement into outcome reports;
5. add dry-run commands for planning and one-packet attempt context;
6. document the human workflow;
7. dogfood a few real repairs and record receipts;
8. only then define external agent handoff and readiness metrics.

The first useful artifact should answer:

```text
Which top packets are safest to attempt now?
Which packets are blocked, and why?
Which packets are missing verify or receipt paths?
```

## Alternatives considered

| Alternative | Why we are not picking it |
| --- | --- |
| Agent reads raw findings. | Raw findings are supporting evidence, not work. This would bypass canonical identity, actionability, confidence, and static-limit truth. |
| Agent writes arbitrary tests. | Arbitrary test generation ignores repair kind, related test, assertion shape, and must-not-change boundaries. |
| Agent edits production code by default. | The default repair loop is about missing observers and discriminators. Production-code edits require explicit operator authority outside this proposal. |
| Treat a passing test as success. | Passing tests do not prove RIPR evidence improved. Receipt-backed evidence movement is the success signal. |
| Hide failed attempts and retry. | Silent retry loops make evidence less trustworthy. Failed, unchanged, and regressed attempts must stay visible. |
| Start with provider integration. | Provider calls are not needed to prove the packet, receipt, and outcome contracts. External agent handoff comes after dry-run and human workflow proof. |
| Change public badges now. | Badge semantics require dogfood evidence that actionable canonical gap packets lead to safe, verifiable repairs. |

## Behavior specs to create or update

- `RIPR-SPEC-0057`: RIPR swarm repair loop.

Future implementation specs may split packet ranking, dry-run attempts,
outcome joins, and external agent handoff if those contracts grow beyond
`RIPR-SPEC-0057`.

## Architecture decisions needed

No ADR is needed for the docs and dry-run planning phases. Add an ADR before
any future work grants provider integration, source-edit authority, autonomous
retry loops, autonomous merge behavior, or default production-code edits.

## Implementation campaign shape

Each slice follows the [scoped PR contract](../SCOPED_PR_CONTRACT.md).

1. `docs/ripr-swarm-repair-loop-spec`
2. `docs/ripr-swarm-repair-loop-proposal`
3. `report/ripr-swarm-plan`
4. `fixtures/swarm-plan-packet-corpus`
5. `report/actionable-gap-outcomes`
6. `analysis/join-receipts-to-canonical-gaps`
7. `xtask/ripr-swarm-plan-dry-run`
8. `xtask/ripr-swarm-attempt-dry-run`
9. `docs/ripr-swarm-human-workflow`
10. `dogfood/ripr-swarm-real-packet-repairs`
11. `docs/external-agent-handoff`
12. `report/ripr-swarm-readiness-score`

## Evidence plan

- Specs and proposals define the source-of-truth boundaries before code.
- Output schema docs pin `swarm-plan` and `actionable-gap-outcomes` artifacts
  when the reports land.
- Fixtures cover high-confidence boundary assertion, exact error variant,
  output observer, blocked static limitation, missing verify command, missing
  receipt command, must-not-change boundary, and receipt outcome cases.
- Unit tests cover ranking readiness, blocked reasons, dry-run attempt output,
  packet validation failures, receipt/outcome joins, multiple attempts per
  canonical gap, and static-limitation blocking.
- Scorecard or readiness reports track swarm-ready packets, blocked packets,
  missing verify commands, missing receipt commands, static-limitation packets,
  high-confidence packets, attempted packets, improved attempts, unchanged
  attempts, regressed attempts, and failed-to-apply attempts.
- Dogfood receipts prove real repairs before downstream rendering or badge
  readiness changes.

## Risks

- Ranking could become a second actionability engine. Mitigation: ranking is
  advisory and starts only from actionable canonical packets.
- Missing typed fields could tempt Markdown parsing. Mitigation: Markdown is
  human explanation only; missing fields create blocked states.
- Static limitations could become user test debt. Mitigation: limitation rows
  are blocked with named reasons and repair routes.
- A dry-run command could look like an autonomous repair. Mitigation: command
  names, docs, and output state that dry-run does not edit files, run tests,
  call providers, generate tests, create receipts, or claim movement.
- A provider handoff could bypass review. Mitigation: external agent handoff is
  a later contract and requires operator review, receipt, and evidence movement.
- Badge pressure could overtake evidence proof. Mitigation: badge-readiness is
  blocked until dogfood shows real packet burn-down with receipts.

## Non-goals

- No raw-finding work queue.
- No provider or model integration.
- No generated tests.
- No mutation execution.
- No autonomous source edits.
- No production-code edits by default.
- No PR/CI rendering.
- No LSP/editor projection.
- No gate policy.
- No public badge change.
- No automatic merge behavior.
- No retry loop without operator bounds.
- No claim that a passing test equals evidence improvement.

## Exit criteria

This proposal can move to `accepted` when:

- `RIPR-SPEC-0057` is in the spec index and traceability manifest;
- `swarm-plan.{json,md}` ranks top ready and blocked packets from typed
  actionable packet fields;
- packet corpus fixtures pin ready, blocked, static-limit, missing verify,
  missing receipt, must-not-change, and outcome cases;
- actionable-gap outcome reporting joins receipts to canonical gaps;
- dry-run plan and attempt commands exist and do not edit files by default;
- docs explain the human repair workflow;
- dogfood records 3-5 real packet repairs with receipt and evidence movement;
- readiness metrics show swarm-ready, blocked, missing verify, missing receipt,
  static limitation, improved, unchanged, regressed, and failed attempt counts;
- no provider, generated-test, mutation, source-edit, PR/CI, editor/LSP, gate,
  public badge, or autonomous merge scope landed.
