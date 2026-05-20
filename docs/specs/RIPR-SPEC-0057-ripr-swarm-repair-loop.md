# RIPR-SPEC-0057: RIPR Swarm Repair Loop

Status: proposed

## Problem

Lane 1 now emits actionable canonical gap packets with repair, verify, receipt,
and must-not-change fields. That makes the next safe automation boundary a
bounded repair-execution loop over those packets.

The unsafe version of this idea would let an agent read raw findings, invent a
repair, edit production code, retry silently, or claim success from a passing
test alone. That would turn RIPR back into unbounded detector output with an
automation wrapper.

`ripr-swarm` exists to preserve the Lane 1 invariant:

```text
Raw findings are evidence.
Canonical evidence items are the countable unit.
Actionable canonical gaps are user work.
```

The swarm consumes the user-work unit. It does not consume raw analyzer signals.

## Behavior

`ripr-swarm` coordinates bounded repair attempts from actionable canonical gap
packets. It reads typed packet fields, selects a safe top slice, presents a
dry-run attempt context, and records whether receipt-backed evidence movement
improved, stayed unchanged, regressed, resolved, or remained unknown.

The behavior is intentionally narrow:

- packet selection starts from `actionable-gaps.json`;
- actionability comes from the canonical item, not raw findings;
- attempts are one packet at a time;
- a verify command and receipt command are required for a swarm-ready packet;
- outcome reporting keeps missing receipts, unchanged repairs, and regressions
  visible;
- default operation is dry-run and human-reviewable.

## Scope

`ripr-swarm` is a repo-local repair-coordination runner. It coordinates one
bounded repair attempt at a time from actionable canonical gap packets.

It may:

- read `target/ripr/reports/actionable-gaps.json`;
- rank or select packets using typed packet fields;
- show a dry-run repair packet;
- require a receipt command before an attempt is considered complete;
- join receipts and evidence movement through outcome reports;
- leave failed or unchanged attempts visible.

It must not:

- consume raw findings as work items;
- infer actionability from raw static class, prose, Markdown, or annotation
  text;
- invent repairs outside packet boundaries;
- edit production code by default;
- generate tests without explicit operator action;
- call providers or model APIs;
- run mutation testing;
- change PR/CI rendering, LSP/editor behavior, gates, public badges, or policy
  state;
- silently retry until success;
- hide failed attempts.

## Required Evidence

Before a packet can be ranked as swarm-ready, typed evidence must show:

- canonical gap identity;
- unresolved actionable gap state;
- evidence class;
- structured repair route;
- repair kind;
- target test, assertion, observer, or safe typed fallback when applicable;
- verify command;
- receipt command;
- must-not-change boundaries;
- confidence basis;
- absence of blocking static limitations.

Before an attempt can claim improvement, typed evidence must show:

- attempted packet identity;
- receipt presence or explicit receipt absence;
- evidence movement state joined to the same canonical gap;
- outcome state in `actionable-gap-outcomes`;
- no production-code edit claim unless explicitly operator-authorized outside
  the default swarm contract.

## Consumed Artifacts

The primary input is:

```text
target/ripr/reports/actionable-gaps.json
```

Optional inputs may include:

```text
target/ripr/reports/actionable-gap-outcomes.json
target/ripr/agent/agent-receipt.json
target/ripr/workflow/agent-receipt.json
target/ripr/reports/targeted-test-outcome.json
target/ripr/reports/evidence-quality-scorecard.json
target/ripr/reports/evidence-quality-trend.json
```

The swarm may read these artifacts only as typed JSON contracts. Markdown is
human explanation and must not decide actionability.

## Actionable Packet Input Contract

`actionable-gaps.json` is the only repair-queue input. A swarm runner consumes
canonical actionable packets from that artifact and treats raw findings only as
supporting evidence.

Each consumed packet must expose the typed fields needed to route one bounded
repair attempt:

```text
canonical_gap_id
evidence_class
gap_state
repair_kind
repair_route
target_test_type or target_assertion_shape
related_test_or_observer
verify_command
receipt_command
static_limitations[]
must_not_change[]
confidence_basis
raw_findings[] as supporting evidence
```

The runner may also use projection eligibility and prior outcome state for
ranking and blocking decisions. It must not derive work items from raw findings,
Markdown text, PR annotations, or static class labels.

## Required Packet Fields

A packet is swarm-ready only when typed fields provide a closed repair loop.

| Field | Requirement |
| --- | --- |
| `canonical_gap_id` | Required stable identity for selection, receipt join, and outcome join. |
| `evidence_class` | Required class for maturity, risk, and fixture-backed handling. |
| `gap_state` | Must indicate an unresolved actionable gap. |
| `repair_kind` | Required repair family such as boundary assertion, exact error variant, output observer, side-effect observer, or visibility inspection. |
| `repair_route` | Required structured route. Prose-only repair guidance is not enough. |
| `target_test_type` or `target_assertion_shape` | Required when applicable to bound the edit target. |
| `related_test_or_observer` | Preferred. Missing context lowers readiness unless the packet has a safe typed fallback. |
| `verify_command` | Required before a packet can be attempted. |
| `receipt_command` | Required before a packet can be swarm-ready. |
| `raw_findings[]` | Supporting evidence only; never the selection unit. |
| `static_limitations[]` | Must be empty for repair-ready packets or explicitly handled as a blocked state. |
| `must_not_change[]` | Required for any attempt that could otherwise broaden into production-code edits. |
| `confidence_basis` | Required for ranking and blocked-state explanation. |

Missing fields do not make the packet disappear. They move it to a blocked or
report-only state with a named reason.

## State Model

Each packet has one swarm state.

| State | Meaning | Allowed next step |
| --- | --- | --- |
| `queued` | Packet is valid but not assigned. | Select, dry run, or skip. |
| `assigned` | A human or agent has accepted the packet. | Attempt or release assignment. |
| `attempted` | A repair was attempted, but verification or receipt is not complete. | Run verify and receipt commands. |
| `receipt_present` | A receipt exists for the attempt, but evidence movement has not been joined or is still unknown. | Join evidence movement or inspect missing movement context. |
| `verified_improved` | Receipt and evidence movement indicate improvement. | Record outcome and refresh audit. |
| `verified_unchanged` | Receipt exists but evidence did not improve. | Keep attempt visible and inspect repair fit. |
| `verified_regressed` | Receipt exists and evidence worsened. | Stop, revert or inspect manually. |
| `resolved` | Receipt-backed evidence movement shows the canonical gap is no longer actionable. | Record resolution and refresh audit/scorecard. |
| `failed_to_apply` | The repair could not be applied within packet boundaries. | Record failure and keep packet visible. |
| `blocked_by_missing_context` | Required packet fields, related test, verify command, receipt command, or safe path context are missing. | Regenerate upstream artifacts or improve Lane 1 packet fields. |
| `blocked_by_static_limitation` | The packet or class carries a named static limitation that prevents a safe repair attempt. | Close analyzer limitation or inspect manually. |

State transitions are receipt-backed when an attempt reaches verification. A
passing test without a receipt does not become `verified_improved`.

## Ranking Contract

Swarm ranking is advisory. It chooses safer packets first; it does not redefine
actionability.

Ranking may consider:

- `repair_route` present;
- `verify_command` present;
- `receipt_command` present;
- confidence basis;
- related test or observer availability;
- must-not-change boundaries;
- evidence class maturity;
- expected canonical gap delta;
- lack of static limitations;
- prior outcome state.

Ranking must not place static-limitation-only items, missing-receipt packets,
or missing-verify packets in the high-confidence repair-ready set.

## Attempt Contract

An attempt starts from exactly one packet.

The attempt context must include:

- canonical gap id;
- evidence class;
- repair kind;
- repair route;
- target test, assertion, or observer shape when known;
- related test or observer when known;
- verify command;
- receipt command;
- must-not-change boundaries;
- raw findings as supporting evidence;
- static limitations and confidence basis.

The default attempt mode is dry-run. A non-dry-run attempt requires explicit
operator action.

When a repair changes files, the attempt should prefer tests, snapshots,
goldens, or output observers identified by the packet. Production-code edits
are outside the default swarm contract and require an explicit operator override.

## Receipt and Outcome Contract

Every attempt must produce or explain the absence of a receipt.

Outcome reporting uses:

```text
target/ripr/reports/actionable-gap-outcomes.json
target/ripr/reports/actionable-gap-outcomes.md
```

Outcome states are:

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

The swarm maps these outcome states into swarm attempt states:

| Outcome | Swarm interpretation |
| --- | --- |
| `not_attempted` | `queued` unless assignment metadata says otherwise. |
| `attempted_no_receipt` | `attempted`; do not claim improvement. |
| `receipt_present` | `receipt_present`; movement still unknown. |
| `evidence_improved` | `verified_improved`. |
| `evidence_unchanged` | `verified_unchanged`. |
| `evidence_regressed` | `verified_regressed`. |
| `resolved` | `resolved`. |
| `unknown` | `attempted` or blocked, depending on missing context. |

The same canonical gap may have multiple attempts. The latest attempt may be
highlighted, but previous failed, unchanged, or regressed attempts remain
visible. A receipt that does not match any current canonical gap packet is
reported as an orphaned receipt; it remains audit evidence and does not create
a new actionable gap.

## Dry-Run Commands

The first implementation surface should be dry-run only:

```bash
cargo xtask ripr-swarm plan --top 10
cargo xtask ripr-swarm attempt --packet <id> --dry-run
```

`plan` reads actionable packets and writes:

```text
target/ripr/reports/swarm-plan.json
target/ripr/reports/swarm-plan.md
```

`readiness` reads the swarm plan plus actionable-gap outcomes and writes:

```text
target/ripr/reports/swarm-readiness.json
target/ripr/reports/swarm-readiness.md
```

`attempt --dry-run` prints the bounded packet context and the commands a human
or external agent would run. It does not edit files, run tests, call providers,
or create receipts.

## Non-Goals

This spec does not add:

- provider integration;
- mutation execution;
- generated-test execution or writing;
- autonomous source edits;
- production-code edits by default;
- PR/CI rendering;
- LSP/editor projection;
- gate policy;
- public badge changes;
- automatic merge behavior;
- retry loops without operator bounds.

## Non-Claims

`ripr-swarm` does not claim:

- runtime adequacy;
- mutation kill/survival;
- gate pass/fail authority;
- public badge readiness by itself;
- editor or PR rendering behavior;
- autonomous merge readiness;
- that a test edit improved evidence without receipt-backed movement.

Unknowns remain visible. Static limitations remain named analyzer gaps with
repair routes, not user test debt.

## Acceptance Examples

Given a high-confidence boundary assertion packet with `verify_command`,
`receipt_command`, and must-not-change boundaries, `ripr-swarm plan` may rank it
as swarm-ready and `ripr-swarm attempt --dry-run` may render the bounded repair
context without editing files.

Given an exact error variant packet without `receipt_command`, `ripr-swarm plan`
must not rank it as swarm-ready. It should report
`blocked_by_missing_context` with a stable missing-receipt reason.

Given a packet whose only support is `raw_findings[]`, `ripr-swarm` must not
create a repair attempt. Raw findings remain supporting evidence only.

Given a static-limitation packet such as an opaque helper or unsupported
observer topology, `ripr-swarm` must report `blocked_by_static_limitation`
rather than enqueueing it as repair-ready.

Given a receipt-backed attempt with evidence movement `evidence_unchanged`,
`ripr-swarm` must keep the failed attempt visible and must not mark the gap as
resolved.

Given a receipt-backed attempt with evidence movement `evidence_regressed`,
`ripr-swarm` must stop and expose the regressed state for human review.

Given a receipt artifact whose seam id or anchor does not match any current
actionable packet, `actionable-gap-outcomes` must report it as an orphaned
receipt rather than silently dropping it or creating a new repair packet.

## Fixture Expectations

`fixtures/swarm-plan-packet-corpus` pins the first packet-ranking corpus with:

- high-confidence boundary assertion packet;
- exact error variant packet;
- output observer packet;
- blocked static limitation packet;
- missing verify command packet;
- missing receipt command packet;
- must-not-change boundary packet.

`fixtures/actionable-gap-outcomes-corpus` pins outcome reporting with:

- not attempted packet;
- receipt present without movement;
- evidence improved;
- evidence unchanged;
- evidence regressed;
- resolved;
- attempted without a matching receipt.
- orphaned receipt reporting.

Must-not-claim guards:

- do not rank static limitation as repair-ready;
- do not rank packet without `receipt_command` as swarm-ready;
- do not rank packet without `verify_command` as high confidence;
- do not create a repair attempt from raw findings alone;
- do not hide unchanged or regressed attempts;
- do not create a new actionable gap from an orphaned receipt;
- do not imply production-code edits are allowed by default.

## Test Mapping

Current implementation coverage:

- `xtask::tests::ripr_swarm_plan_ranks_ready_packets_and_blocks_missing_context`
  pins ready, missing-context, missing verify, missing receipt, and
  static-limitation blocking behavior for `swarm-plan`;
- `xtask::tests::ripr_swarm_plan_packet_corpus_matches_expected_states`
  validates `fixtures/swarm-plan-packet-corpus/corpus.json` against the same
  planner used by the report command;
- `xtask::tests::actionable_gap_outcomes_fixture_corpus_matches_expected_states`
  validates `fixtures/actionable-gap-outcomes-corpus/corpus.json` against the
  same outcome joiner used by the report command;
- `xtask::tests::actionable_gap_outcomes_fixture_corpus_reports_contract_drift`
  pins missing, malformed, and mismatched outcome-corpus guardrails;
- `xtask::tests::ripr_swarm_command_parses_plan_args` pins the
  `cargo xtask ripr-swarm plan --top <n>` command shape.
- `xtask::tests::ripr_swarm_command_parses_attempt_dry_run_args` pins the
  `cargo xtask ripr-swarm attempt --packet <id> --dry-run` command shape;
- `xtask::tests::ripr_swarm_attempt_requires_packet_and_dry_run` pins that the
  attempt command stays dry-run-only and requires a packet id;
- `xtask::tests::ripr_swarm_attempt_dry_run_renders_bounded_packet_context`
  pins the dry-run context for a queued repair packet, including repair route,
  related observer, verify command, receipt command, expected movement, and
  must-not-change boundaries;
- `xtask::tests::ripr_swarm_attempt_dry_run_reports_blocked_packet_context`
  pins that blocked/static-limitation packets stay visible without becoming
  repair-ready.

Follow-up implementation PRs should add tests for:

- packet validation failure modes;
- receipt and outcome joins;
- multiple attempts per canonical gap;
- richer must-not-change boundary rendering.

## Implementation Mapping

This spec is the behavior contract for future repo-local automation. Expected
implementation surfaces are:

- `cargo xtask ripr-swarm plan --top <n>` (implemented);
- `cargo xtask ripr-swarm attempt --packet <id> --dry-run` (implemented);
- `cargo xtask ripr-swarm readiness` (implemented);
- `target/ripr/reports/swarm-plan.json` (implemented);
- `target/ripr/reports/swarm-plan.md` (implemented);
- `target/ripr/reports/swarm-readiness.json` (implemented);
- `target/ripr/reports/swarm-readiness.md` (implemented);
- `docs/RIPR_SWARM_HUMAN_WORKFLOW.md` (implemented);
- existing `actionable-gaps` and `actionable-gap-outcomes` artifacts.

No provider SDK, mutation executor, generated-test writer, PR/CI renderer,
LSP/editor feature, gate policy, or public badge change belongs to this spec.

## Metrics

Future reports should expose:

- `swarm_ready_packets`;
- `swarm_blocked_packets`;
- `swarm_missing_verify_command`;
- `swarm_missing_receipt_command`;
- `swarm_static_limitation_packets`;
- `swarm_high_confidence_packets`;
- `swarm_attempted_packets`;
- `swarm_verified_improved`;
- `swarm_verified_unchanged`;
- `swarm_verified_regressed`;
- `swarm_failed_to_apply`;
- `swarm_orphaned_receipts`.
