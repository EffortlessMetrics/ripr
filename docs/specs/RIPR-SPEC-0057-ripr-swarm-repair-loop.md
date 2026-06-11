# RIPR-SPEC-0057: RIPR Swarm Repair Loop

Status: accepted

Accepted: 2026-05-20. The current implementation mapping below records the
implemented `ripr-swarm plan`, `ripr-swarm attempt --dry-run`, readiness,
swarm-plan, outcome, workflow, fixture, and dogfood surfaces. The accepted
scope remains advisory, dry-run-first, and source-edit-free.

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
- route-quality projection preserves non-success outcomes such as unchanged
  attempts as first-class next actions instead of converting every full
  actionable packet into another blind attempt;
- route-quality backlog rows preserve sampled missing receipt reasons when
  available, so receipt-reliability work can distinguish broad verify timeout
  routes from ordinary absent local receipts;
- timeout-backed missing receipt reasons route to a bounded verify-route
  improvement backlog instead of generic receipt collection, because broad
  verify commands that cannot finish are not safe repair guidance;
- ordinary attempted packets without receipts route to
  `collect_missing_attempt_receipts` with a sample packet/gap/repair-kind when
  one is available, so downstream surfaces cannot claim improvement from verify
  evidence alone;
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
- for predicate-boundary assertion repairs, confidence stronger than
  `static_only`; static-only predicate boundaries require operator judgment
  because the existing movement proof may not observe derived internal
  equality predicates from a focused test invocation.

Before an attempt can claim improvement, typed evidence must show:

- attempted packet identity;
- stable attempt instance identity when a receipt or targeted outcome backs the
  attempt;
- receipt presence or explicit receipt absence;
- evidence movement state joined to the same canonical gap;
- outcome state in `actionable-gap-outcomes`;
- targeted-test movement without a matching receipt remains
  `attempted_no_receipt` and cannot claim improvement, regression, unchanged
  evidence, or resolution;
- outcome rows expose normalized `receipt_command` so downstream ledgers and
  readiness reports do not reinterpret legacy command/path fields;
- outcome and ledger rows preserve typed `verify_result` when source artifacts
  provide it, and do not infer success from a present verify command;
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
allowed_edit_surface[]
confidence_basis
raw_findings[] as supporting evidence
```

The runner may also use projection eligibility and prior outcome state for
ranking and blocking decisions. It must not derive work items from raw findings,
Markdown text, PR annotations, or static class labels.
When the source audit has no safely actionable packets, the swarm plan and
readiness reports may still forward a static-limitation backlog with top
limitation categories and analyzer repair routes. That backlog is advisory
analyzer routing evidence only; it must not become a repair-ready packet or
public actionable count.
The backlog may include `limitation_backlog_packets[]` for analyzer work. Those
packets must include limitation category, repair route, signal count, sample
canonical gap IDs or source samples when available, dominant evidence class, why
the item is not actionable, the analyzer unlock condition, and non-claims. Packet
identity is route- and subroute-grained when `limitation_subroute` is available,
so one limitation category can produce separate analyzer backlog packets for
separate repair routes or expression-shaped repair queues. They remain
non-actionable and must not be placed in `top_ready_packets`.
The bounded packet set must keep samples for each packet-backed top repair
route even when that route's representative packet would otherwise fall below
the highest-volume subroute cutoff. Report-only runtime diagnostics remain in
runtime status/run-limitations instead of appearing as sample-less limitation
routes.
Readiness may project the leading analyzer backlog routes as
`top_limitation_routes[]`. That projection carries `why_not_actionable`, unlock
conditions, non-claims, and sample subroutes, and the top next action must
preserve the same non-actionability reason when routing analyzer work. It is
separate from `repair_route_quality[]`, which is attempt-outcome evidence;
limitation routes remain non-actionable until the full public packet contract is
satisfied.
When readiness consumes older or external backlog packets that omit
presentation-only route fields, it must preserve the non-actionable boundary by
filling standard non-claims, fallback why-not-actionable text, fallback unlock
conditions, and explicit `unknown` evidence class values instead of making those
routes repair-ready.

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
| `verify_command` | Required before a packet can be attempted. Broad repo-exposure snapshot comparisons are not bounded verify commands unless a narrower typed route is also available. |
| `receipt_command` | Required before a packet can be swarm-ready. |
| `raw_findings[]` | Supporting evidence only; never the selection unit. |
| `static_limitations[]` | Must be empty for repair-ready packets or explicitly handled as a blocked state. |
| `must_not_change[]` | Required for any attempt that could otherwise broaden into production-code edits. |
| `allowed_edit_surface[]` | Required bounded workspace-relative file list for delegated edits. Derived entries must resolve to existing workspace files before a packet can be swarm-ready. |
| `confidence_basis` | Required for ranking and blocked-state explanation. |

Missing fields do not make the packet disappear. They move it to a blocked or
report-only state with a named reason.

Receipt paths or receipt hints are not substitutes for `receipt_command`.
Path-only receipt context must remain `missing_receipt_command` and cannot be
ranked swarm-ready because delegated work needs the executable command that
writes or checks the receipt.

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
| `blocked_by_public_projection_exclusion` | The upstream actionable-gap projection marked the packet ineligible for public repair routing, such as suppressed, intentional, or otherwise excluded guidance. | Inspect `projection_exclusion_reasons[]` and repair the upstream actionability classification before attempting the packet. |
| `blocked_by_operator_judgment` | The packet has typed context but current confidence is too weak for a default swarm attempt, such as a static-only predicate boundary. | Inspect manually, add stronger upstream evidence, or route to a human-selected repair. |

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
missing-verify packets, or static-only predicate-boundary assertion packets in
the repair-ready set.

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
Synthetic `not_attempted` rows are current queue placeholders, not durable
repair attempts. They are preserved only while their packet or canonical gap
remains present in the current swarm plan, or when receipt/verification evidence
makes the row audit evidence. Stale synthetic `not_attempted` placeholders must
not inflate route-quality or missing-evidence-field counts after the packet
queue changes.

The attempt ledger must preserve typed route context for each attempt when it
is available: `evidence_class`, `source_file`, `repair_kind`,
`target_test_type`, `assertion_shape`, `verify_command`, `verify_result`, and
`receipt_command`. `attempted_no_receipt` rows should also preserve
`missing_receipt_reason` when the dogfood or outcome source provides one, so
receipt reliability failures can be routed instead of collapsed into counts.
It must summarize latest attempts by `repair_kind` so
repeated unchanged, regressed, no-receipt, missing-verify-result, or unknown
outcomes become analyzer-improvement signals instead of disappearing into
aggregate attempt counts.
Dogfood attempt receipts must not contradict their recorded outcome: movement
receipt state and explicit `evidence_movement` tokens must not claim improved,
unchanged, or regressed evidence that conflicts with the outcome, and
no-receipt attempts must not claim receipt-backed evidence movement.
Repo-local real repair attempts may be imported into the attempt ledger as
advisory dogfood evidence. Imported dogfood attempts affect attempt/outcome and
repair-route-quality summaries, but they do not create public repair packets,
do not make static limitations swarm-ready, and do not change badge, LSP, PR,
or CI authority.

Readiness must preserve durable attempt history separately from current routing
state. It should forward `attempt_history_summary` when the attempt ledger
provides it, and derive the same history summary from durable `attempts[]` for
older ledgers. Current attempt/outcome summary counts, repair-route quality, and
missing-evidence-field counts remain latest-attempt projections. Stored
`summary`, `repair_route_quality[]`, and `top_missing_evidence_fields[]` rows are
summary output; they must not override recomputed latest-attempt state if the
two disagree.
Repair-route quality rows should carry sample packet IDs, sample attempt IDs,
and canonical gap IDs for failing latest attempts when available, and readiness
should point `improve_repair_route_quality` at the derived route-quality
backlog packet while preserving the first failed-attempt sample in
`next_actions[].attempt_id` and the reason text. Route-quality work should
start from a concrete failed attempt without re-queuing that failed repair
packet as swarm-ready work.
Attempt-ledger reports should also project `historical_repair_route_quality[]`,
`historical_language_repair_route_quality[]`, and
`top_historical_failing_repair_routes[]` from durable full attempt history. These
history rows keep older unchanged, regressed, and no-receipt attempts visible
after a later follow-up improves or resolves the same canonical gap. They are
route-learning audit evidence, not current routing state; current readiness
actions still come from latest-attempt repair-route quality and explicit
missing-evidence rows.
Attempt-ledger and readiness reports should also project
`repair_route_quality_backlog[]` from the top failing repair routes. Each row is
an analyzer/report improvement packet with a stable `packet_id`,
`improvement_route`, failure counts, dominant failure reason, sample packet IDs,
sample attempt IDs, canonical gap IDs, an unlock condition, and non-claims. These rows are not
public repair packets, are not swarm-ready work, and must not change badge, PR,
LSP, or CI authority.
The explicit `missing_verify_result` summary count is the closeout counter for
attempted rows whose verification command is known but whose typed pass/fail or
not-run result was not preserved.
Readiness must route `attempted_no_receipt` and `receipt_present` separately:
ordinary no-receipt attempts require collecting the packet receipt, while
timeout-backed no-receipt attempts require an `improve_repair_route_quality`
action against the bounded verify-route backlog before operators retry broad
receipt collection. Receipt-present attempts require joining before/after
evidence movement before route quality can claim improvement, regression, or
unchanged evidence.
When `latest_attempts[]` includes a `receipt_present` sample, readiness should
copy the first sample packet ID, canonical gap ID, and repair kind into the
`join_receipt_evidence_movement` next action.
When `orphaned_receipts[]` includes samples, readiness should copy the first
sample packet ID, canonical gap ID, and repair kind into the
`reconcile_orphaned_receipts` next action.
When `latest_attempts[]` includes `evidence_unchanged` or `evidence_regressed`
samples, readiness should copy the first matching sample packet ID, canonical
gap ID, and repair kind into the matching `inspect_unchanged_attempts` or
`inspect_regressed_attempts` next action.
When `top_missing_evidence_fields[]` includes `attempt_receipt` or
`verify_result` samples, readiness should copy the first ordinary
`attempt_receipt` sample packet ID, canonical gap ID, and repair kind into
`collect_missing_attempt_receipts`, and copy the first `verify_result` sample
into `inspect_missing_verify_results`. If the no-receipt sample is explained by
a timeout-backed route-quality row, readiness should route the sample to
`improve_repair_route_quality` for the bounded verify-route backlog instead of
generic receipt collection.

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

The readiness report must also emit a bounded `next_actions` queue derived
from those same artifacts. It may recommend refreshing missing or malformed
inputs, repairing missing verify or receipt command projections, reconciling
orphaned receipts, inspecting unchanged or regressed attempts, routing static
limitations to the Lane 1 analyzer backlog, improving a noisy repair route
before increasing packet volume, routing operator-judgment packets for manual
selection or stronger upstream evidence, or dry-running a top swarm-ready
packet. These actions are advisory coordination hints; they must not execute
repairs, consume raw findings as work, change badge semantics, or hide blocked
or uncertain evidence.

Readiness must also expose blocked packet states as a route table. Each
reported blocked class must include a count, human-readable reason, next action
kind, repair route, and example packet/canonical gap identity when the source
artifact has one, so `blocked_by_missing_context`,
`blocked_by_static_limitation`, `blocked_by_public_projection_exclusion`, and
`blocked_by_operator_judgment` do not appear only in raw packet JSON. The table
must also route field-level blockers such as `not_actionable_gap_state`,
`missing_verify_command`, `unbounded_verify_command`,
`missing_receipt_command`, `missing_repair_route`,
`missing_related_test_or_observer`, `missing_must_not_change`,
`missing_allowed_edit_surface`, `missing_confidence`, and
`missing_raw_evidence_refs`, plus outcome
blockers such as `attempted_no_receipt`, `missing_verify_result`,
`orphan_receipt`, `unchanged_attempt`, and `regressed_attempt`.
`swarm-plan` must provide non-top-limited packet examples for plan-derived
blocked classes so readiness examples are not dependent on `--top` truncation.
Repo-exposure snapshot comparison commands are not by themselves bounded verify
commands for default swarm delegation; they must be excluded with
`unbounded_verify_command` unless the packet also supplies or can derive a
narrower proof route. The only default derived proof route is a typed Rust test
target: `related_test_or_observer.file` must map to a known workspace package,
`related_test_or_observer.name` must be a safe cargo test filter, and the
resulting command is `cargo test -p <package> <test-filter>`.

Readiness must also expose `top_next_action` as a stable projection of
`next_actions[0]`. Downstream surfaces may show that object directly, but they
must not treat it as an independent ranking source or reinterpret raw findings
to produce their own top action. A `limited_sampled_input` repo-exposure run may
still put the sampled static-limitation/analyzer backlog first when all
coordination inputs are readable and no swarm-ready packet exists; the
`resolve_limited_runtime_status` action must remain visible in `next_actions`
and the report must keep the run marked limited.

Dogfood receipts must include at least one surface-projection alignment case
that starts from a single canonical repair packet and receipt-backed attempt,
then proves the attempt ledger and readiness projection preserve the same
`canonical_gap_id`, `packet_id`, `repair_kind`, verify command, receipt
command/state, outcome, and `top_next_action`. Badge, LSP, PR, and CI remain
thin advisory consumers of that canonical state; the dogfood receipt must not
change their rendering, ranking, or gate authority.

Dogfood receipts must also record multiple real repair-loop attempts from
repo-local PR or handoff evidence. The set must include at least one improved
or resolved case and at least one non-success case such as unchanged evidence
or a named missing receipt. Each case records the packet identity, canonical
gap identity, repair kind, target test or observer shape, verify command,
verify result, receipt command/path/state, before/after gap state, outcome,
must-not-change boundaries, raw evidence references, and an operator note.
These receipts are evidence for repair routing quality, not proof that RIPR
may edit code, run providers, execute mutation testing, or change downstream
surface authority.

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

Given a packet whose `public_projection_eligible` value is false, `ripr-swarm`
must report `blocked_by_public_projection_exclusion` rather than enqueueing it
as repair-ready, even when the packet otherwise has typed target, verify,
receipt, confidence, and boundary fields.
Given a packet with a non-empty `projection_exclusion_reasons[]` array,
`ripr-swarm` must also report `blocked_by_public_projection_exclusion`; explicit
exclusion reasons are authoritative even if a stale producer also set
`public_projection_eligible = true`.

Given a static-only predicate-boundary assertion packet, `ripr-swarm` must
report `blocked_by_operator_judgment` rather than enqueueing it as
repair-ready. The packet remains visible, but a default swarm attempt must wait
for fixture-backed or calibrated evidence, or an explicit operator decision.

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
- static-only predicate-boundary packet requiring operator judgment;
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
- do not rank static-only predicate-boundary packets as swarm-ready without
  stronger evidence;
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
- `crates/ripr/src/output/outcome/mod.rs::tests::targeted_test_outcome_python_preview_fixture_matches_expected_receipts`
  validates the Python first-PR before/after check-output fixture and expected
  `ripr outcome` JSON/Markdown receipts for closed, unchanged, and opened
  canonical gap movement;
- `xtask::tests::first_successful_pr_corpus_reports_outcome_receipt_drift`
  pins first-PR outcome-receipt manifest guardrails for missing paths,
  duplicate receipt IDs, wrong input paths, missing movement, and missing
  review receipts;
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
  repair-ready;
- `xtask::tests::ripr_swarm_plan_ranks_ready_packets_and_blocks_missing_context`
  pins static-limitation backlog forwarding from actionable gaps into swarm
  plan output without making limitations repair-ready;
- `xtask::tests::ripr_swarm_attempt_ledger_preserves_prior_attempts_and_highlights_latest`
  pins durable attempt history and latest-attempt selection;
- `xtask::tests::ripr_swarm_attempt_ledger_drops_stale_synthetic_not_attempted_rows`
  pins cleanup of retired queue placeholders;
- `xtask::tests::ripr_swarm_attempt_ledger_synthesizes_current_plan_not_attempted_rows`
  pins creation of current-plan queue placeholders from swarm-ready packets
  before receipt or outcome evidence exists;
- `xtask::tests::ripr_swarm_attempt_ledger_preserves_current_plan_not_attempted_rows`
  pins carry-forward of current queue placeholders;
- `xtask::tests::ripr_swarm_attempt_ledger_summarizes_repair_route_quality`
  pins typed route context, per-`repair_kind` route-quality metrics, top
  failing routes, and missing evidence fields;
- `xtask::tests::ripr_swarm_attempt_ledger_imports_real_repair_attempts`
  pins advisory import of repo-local dogfood repair attempts into durable
  attempt history and route-quality summaries without creating repair packets;
- `xtask::tests::ripr_swarm_readiness_consumes_attempt_ledger_counts`
  pins readiness consumption of the attempt ledger, durable attempt-history
  summary, and repair-route quality, plus forwarding of the swarm-plan
  static-limitation backlog into readiness output.
- `xtask::tests::ripr_swarm_readiness_recomputes_summary_from_attempts_when_summary_is_stale`
  pins readiness fallback derivation of durable attempt-history summary from
  legacy `attempts[]` while keeping current routing counts on the latest attempt
  per canonical gap.
- `xtask::tests::ripr_swarm_readiness_audits_blocked_state_routes`
  pins blocked-state counts, reasons, next action kinds, repair routes, example
  packet/canonical gap identities, and Markdown/JSON parity for readiness route
  auditing.
- `xtask::tests::lane1_static_limitation_backlog_emits_analyzer_packets`
  pins analyzer backlog packets for named limitations without emitting public
  repair packets.
- `xtask::tests::lane1_static_limitation_backlog_splits_same_category_by_repair_route`
  pins route-grained analyzer backlog packet identity when one limitation
  category has multiple repair routes.
- `xtask::tests::ripr_swarm_readiness_routes_static_limitation_backlog_when_no_ready_packets`
  pins readiness `top_limitation_routes[]`, why-not-actionable projection,
  subroute-preserving top action text, and sample packet routing without making
  limitation backlog packets swarm-ready.
- `xtask::tests::lane1_static_limitation_backlog_keeps_samples_for_each_top_repair_route`
  pins that bounded backlog packets keep inspectable samples for each
  packet-backed top repair route even when a low-count route falls below the
  highest-volume subroute cutoff, while report-only runtime diagnostics stay out
  of the packet-backed projection.
- `xtask::tests::ripr_swarm_readiness_hardens_legacy_limitation_backlog_packets`
  pins readiness fallback non-claims, non-actionability text, unlock conditions,
  and explicit unknown evidence classes for older limitation backlog packets.
- `xtask::tests::dogfood_real_repair_attempt_rejects_movement_contradictions`
  pins that real repair attempt receipts cannot record contradictory movement
  claims or claim evidence movement without a receipt.

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
- `cargo xtask ripr-swarm attempt-ledger` (implemented);
- `target/ripr/reports/swarm-plan.json` (implemented);
- `target/ripr/reports/swarm-plan.md` (implemented);
- `target/ripr/reports/swarm-attempt-ledger.json` (implemented);
- `target/ripr/reports/swarm-attempt-ledger.md` (implemented);
- `target/ripr/reports/swarm-readiness.json` (implemented);
- `target/ripr/reports/swarm-readiness.md` (implemented);
- `docs/RIPR_SWARM_HUMAN_WORKFLOW.md` (implemented);
- existing `actionable-gaps` and `actionable-gap-outcomes` artifacts.

Attempt-ledger and readiness reports must preserve limited upstream runtime
state. A readable-but-limited `actionable-gap-outcomes` artifact must not become
a `full` attempt ledger, and a readable-but-limited attempt ledger must not
become a `full` readiness report. The downstream report keeps the upstream
repair route and input path so the operator can repair the limiting artifact
instead of trusting incomplete attempt/outcome evidence.

Readiness reports also expose a coarse `readiness_state` so thin user surfaces
can answer whether the run is `full`, `limited`, `stale`, or `blocked` without
collapsing the detailed `run_status` category. Missing or malformed required
swarm inputs are `blocked`; `limited_stale_input` is `stale`; other
`limited_*` runtime states remain `limited`; complete runtime state is `full`.

Attempt history must not collapse repeated same-state attempts for the same
canonical gap when distinct attempt-instance evidence is available. Generated
`attempt_id` values include the outcome timestamp, receipt artifact path, or
targeted-test-outcome artifact path when present so repeated unchanged,
regressed, no-receipt, and improved attempts remain visible as durable history.

No provider SDK, mutation executor, generated-test writer, PR/CI renderer,
LSP/editor feature, gate policy, or public badge change belongs to this spec.

## Metrics

Future reports should expose:

- `swarm_ready_packets`;
- `swarm_blocked_packets`;
- `swarm_blocked_by_missing_context_packets`;
- `swarm_blocked_by_static_limitation_packets`;
- `swarm_blocked_by_public_projection_exclusion_packets`;
- `swarm_blocked_by_operator_judgment_packets`;
- `swarm_public_projection_excluded_packets`;
- `swarm_missing_verify_command`;
- `swarm_missing_verify_result`;
- `swarm_missing_receipt_command`;
- `swarm_missing_repair_route`;
- `swarm_missing_must_not_change`;
- `swarm_missing_allowed_edit_surface`;
- `swarm_missing_confidence`;
- `swarm_missing_raw_evidence_refs`;
- `swarm_related_context_missing`;
- `swarm_static_limitation_packets`;
- `swarm_high_confidence_packets`;
- `swarm_attempted_packets`;
- `swarm_attempt_history_attempts_total`;
- `swarm_attempt_history_durable_attempts_total`;
- `swarm_attempt_history_evidence_unchanged`;
- `swarm_attempt_history_attempted_no_receipt`;
- `swarm_attempt_history_expected_unchanged`;
- `swarm_attempted_no_receipt_packets`;
- `swarm_receipt_present_packets`;
- `swarm_verified_improved`;
- `swarm_verified_unchanged`;
- `swarm_verified_regressed`;
- `swarm_failed_to_apply`;
- `swarm_orphaned_receipts`;
- `repair_kind_attempted`;
- `repair_kind_improved`;
- `repair_kind_unchanged`;
- `repair_kind_regressed`;
- `repair_kind_resolved`;
- `repair_kind_missing_verify_result`;
- `repair_kind_failure_count`;
- `repair_kind_dominant_failure_reason`;
- `repair_kind_success_rate`;
- `top_failing_repair_routes`;
- `top_limitation_routes`;
- `top_missing_evidence_fields`.
