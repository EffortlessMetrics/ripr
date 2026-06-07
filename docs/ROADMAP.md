# Roadmap

This roadmap is the product plan for moving `ripr` from a published alpha to a
live static exposure analyzer that developers and agents can rely on during a
pull request.

The goal state is this loop:

```text
Developer changes Rust behavior
-> ripr detects the changed behavior
-> ripr identifies the missing or weak discriminator
-> editor shows a precise diagnostic
-> hover explains the evidence path
-> code action emits agent-ready test intent
-> human or agent adds a targeted test
-> finding closes or downgrades
-> real mutation confirms later when the PR is ready
```

`ripr` stays focused on static oracle-gap analysis for diff-derived mutation
probes. It does not become a full mutation runner, a coverage dashboard, a proof
system, a second rust-analyzer, or a generic test generator.

## End Goal

`ripr`'s end goal is to be a repo-native, proof-carrying static exposure
assistant.

That makes the repository itself a self-aligning product system: each PR moves
one bounded capability, proves what changed, states what users may believe, and
leaves the next slice easier to choose from repo artifacts.

For users, `ripr` should make changed behavior test gaps obvious and
repairable:

```text
one changed behavior
-> one missing discriminator
-> one focused proof
-> one verification command
-> one receipt
```

For maintainers and agents, `ripr` should carry its own operating truth:

```text
proposal/spec/ADR/campaign/active goal/support tier/policy/evidence/closeout
-> linked and checked PR by PR
```

A PR is aligned when it moves one bounded capability, proves the movement,
states the claim boundary, and leaves the next slice discoverable from the
repo. The near-term product target is that a Rust PR author can use RIPR from
CLI, CI, VS Code, or agent handoff and get the same top repairable gap, the
same focused test intent, the same verification command, and the same receipt
boundary.

The support-tier system remains the public claim boundary. Improvements only
change what users may believe when the supporting spec, fixture, golden,
dogfood receipt, or validation command is strong enough. `ripr` should never
imply full test adequacy, runtime mutation confirmation, coverage adequacy,
merge approval, or proof of correctness from static evidence alone.

## Multi-Lane Safe Swarm Autonomy

The swarm roadmap is intentionally **parallel-lane**, not a single serialized
phase chain. Multiple lanes may move at the same time, but all lanes must
converge through shared claim boundaries, evidence-level semantics, authority
boundaries, and durable receipts.

Roadmap eligibility is not execution authority. A lane starts only when current
repo state selects it: an open PR, an accepted proposal/spec/plan work item, an
active goal item, or an issue that links to those artifacts. If
`cargo xtask goals next` says all unfinished items are blocked, this roadmap
does not create a hidden ready item.

The operating rule is:

```text
parallel work is encouraged; claim escalation is not
```

In practice, this means one lane may improve packet routing while another lane
improves docs or CI/report truth, but no lane may silently convert:

- visible -> ready
- static-only -> safe-to-edit
- review unavailable -> review passed
- advisory timeout -> all failures advisory

### Shared Gates

All lanes must pass the same global gates:

1. **Claim boundary gate**: every output states what it does not prove.
2. **Evidence-level gate**: every packet/report states its evidence level and
   what action that level allows.
3. **Authority gate**: each action is explicit about swarm authority, source
   authority, operator authority, CI authority, or docs-only authority.
4. **Receipt gate**: meaningful movement records a durable receipt artifact.
5. **Material-change gate**: no refresh-only churn PRs when only timestamps or
   regenerated equivalent status changed.

### Parallel Lane Model

| Lane | Owns | Produces | Must not do |
| --- | --- | --- | --- |
| A. Control Plane / CI Truth | Workflow truth, advisory semantics, runner proof disposition, report consistency | Control-plane status + workflow/runner receipts | Hide unknown failures as advisory |
| B. Judgment Routing | `blocked_by_operator_judgment` packet routing + decision ledger | Judgment packets + decision receipts | Mark packets ready by visibility alone |
| C. Evidence Ladder / Readiness | Evidence levels, readiness names, allowed actions | Evidence ladder + readiness contract | Use ambiguous "ready" without readiness target |
| D. Analyzer Evidence Movement | Narrow class-by-class evidence movement with fixture backing | Evidence audits + fixture-backed readiness deltas | Overclaim runtime/mutation adequacy |
| E. Source Promotion | Bounded swarm-proof to source-proof promotion path | Promotion candidates + source template + closeout receipts | Backdoor unauthorized source edits |
| F. Operator UX / Product Surface | First-useful "what now?" guidance and recovery docs | Roadmap/operator guidance artifacts | Create status-only docs churn |
| G. Bounded Attempts (gated) | Attempt contracts and receipt model for dry-run/test-only work | Attempt schemas + attempt outcome receipts | Run production source edits by default |

Lanes **A-F may proceed in parallel** only through PR-sized slices selected by
the active goal, issue ledger, implementation plan, or source-of-truth stack.
Lane **G is design-eligible** under the same selection rule, but execution
remains gated on control-plane truth, judgment routing, evidence ladder
readiness semantics, and explicit authority checks.

### Shared Evidence Vocabulary

New lane artifacts that introduce or change readiness, routing, or handoff
behavior should carry an explicit evidence level:

- `E0`: raw static signal (report only)
- `E1`: canonical actionable gap (surface in reports)
- `E2`: repair packet (dry-run routing / judgment packet)
- `E3`: strong static packet (bounded swarm dry-run eligible)
- `E4`: operator-selected packet (test-only candidate)
- `E5`: receipt-backed improvement (source-promotion candidate)
- `E6`: source-merged proof (durable advancement)
- `E7`: runtime/mutation confirmation (calibration only)

Readiness labels should always be concrete, for example
`ready_for_report`, `ready_for_judgment`, `ready_for_dry_run`,
`ready_for_test_only_attempt`, and `ready_for_source_promotion`.
These labels are descriptive; they do not grant source-promotion, dry-run, or
merge authority without the linked proof and authority boundary.

## Current Position

The current alpha has the product shape in place:

- one published package: `ripr`
- one CLI binary: `ripr`
- one shared analysis engine
- human, JSON, and GitHub output
- an experimental LSP sidecar
- extension-managed server provisioning
- analysis modes that change indexing scope
- repo seam inventory, test-grip evidence, agent seam packets, LSP seam code
  actions, cached seam fact layers, and advisory cargo-mutants calibration

Mode scope is intentionally cost-aware:

| Mode | Current scope |
| --- | --- |
| `instant` | Changed Rust files only. |
| `draft` | Rust files in packages touched by the diff. |
| `fast` | Package-local scope for now. |
| `deep` | Whole workspace. |
| `ready` | Whole workspace static preflight before separate mutation confirmation. |

The main bottleneck is now proving the hot sidecar path stays responsive without
serving stale evidence. Campaign 5A closed the cache, precision, actionability,
and calibration loop for repo seam evidence. Campaign 5B landed repository
configuration, SARIF/CI policy, and seam-native badge count mapping. Campaign 6
then completed the internal module SRP refactor chain through #405 without
changing output schemas, public API, SARIF, badge, or saved-workspace LSP
behavior. Campaign 7 then pinned the defaults/config baseline, added the
operator cockpit report, made the generated GitHub Action upload pilot/report
artifacts with optional SARIF rendering/upload, documented and verified the
existing VS Code install path and command coverage, and added the public example
corpus for the defaults-first operator path. Install and release-path proof is
now complete for the crate install path, public GitHub Release server assets,
and VSIX package path.
Campaign 7 is closed; the closeout audit demonstrates the installed
boundary-gap seam packet, outcome receipt, and optional calibration loop.
Campaign 8 is closed; the runtime calibration fixture expansion now has a
checked supplied-data sample for the main static/runtime agreement buckets, and
runtime mutation vocabulary remains confined to explicit calibration reports.
Campaign 9 is closed; bounded repo-exposure latency reporting now records cache
collection, cache load hit/miss/corrupt state, file-fact cache reuse, cold
compute, cache store, and total phase timing without changing repo-exposure
outputs. File-fact warm reuse improved the measured index-build subphase, and
the repo evidence hot path now uses indexed related-test candidates, lazy
value-resolution facts, and owned classification handoff. Current proof shows a
bounded cold run can fill the classified-seam cache, after which the default
30-second repo-exposure latency report passes on JSON and Markdown cache hits.
The budget-aware pilot path has first-screen Markdown and terminal copy that
states the inspected seam, why it matters, the focused test to write, and the
before/after commands. Campaign 10 closed after aligning the saved-workspace
diagnostic, evidence, packet/brief, focused-test, after-snapshot, verify,
receipt, cockpit, CI, and install loop. Its release-readiness gate proves the
installed CLI, boundary-gap `pilot`, `outcome`, `agent verify`,
`agent receipt`, latency, LSP cockpit, advisory workflow, VSIX path, and
known-limit surfaces. Campaign 11 closed the LLM work loop: `ripr agent status`
is a read-only lens over existing artifacts, loop command templates are
centralized for the current CLI/LSP/cockpit/CI surfaces, workflow manifests,
provenance-backed receipts, next-action guidance, review summaries, fixtures,
and generated CI packet uploads are pinned, and the LLM operator guide documents
the source-edit-free human and external-agent path. Campaign 12 closed the
First-Hour UX lane: after the LLM work-loop command and artifact state
stabilized, the current product risk was making the VS Code extension and
generated GitHub workflow explain the top recommendation without requiring
users to inspect CLI reports first. The PR guidance annotation contract is
pinned, the editor now has
a first-run status path for server/workspace/analysis state, and seam diagnostic
actions are titled around inspect, targeted-test, agent-handoff, verify, review,
and refresh intent, and the generated GitHub workflow now writes a
reviewer-oriented advisory summary before artifact download. The generated
workflow smoke fixture pins the CI first screen, artifact packet, optional
SARIF gates, badge output, and PR guidance annotation hook. Campaign 12
closed after the first-hour docs routed users by VS Code, CI, CLI, and
agent/reviewer path. Campaign 13 closed PR Review Guidance:
`ripr review-comments` now writes the already-specified
`target/ripr/review/comments.json` report, generated CI runs that producer
before the existing non-blocking summary and changed-line check-annotation
consumers, guidance placement and suppression cases are fixture-pinned, and
[PR review guidance](PR_REVIEW_GUIDANCE.md) documents the command, CI behavior,
summary-only fallback, inline-comment opt-in boundary, and static-evidence
limits. Campaign 14 is closed as Recommendation Calibration: RIPR-SPEC-0013
pins the recommendation calibration report contract for measuring whether
completed recommendation surfaces are actionable, correctly placed, properly
suppressed or capped, and correlated with before/after static movement without
telemetry, generated tests, runtime mutation execution, or default CI blocking.
The PR-shaped calibration corpus, local outcome receipts, and advisory
`cargo xtask recommendation-calibration` report are now checked, and
[Recommendation calibration](RECOMMENDATION_CALIBRATION.md) documents how to
read metrics, receipts, placement quality, suppression correctness, static
movement buckets, and advisory limits. The closeout handoff records the PR
chain and deferred policy boundary. Campaign 15 is closed as Calibrated Gate
Policy: RIPR-SPEC-0014 pins optional policy gates after measured signal
quality, `ripr gate evaluate` writes the read-only decision report,
`fixtures/calibrated-gate-cases` pins the decision matrix, and generated
GitHub workflows run gate evaluation only when `RIPR_GATE_MODE` is explicitly
configured. [Calibrated gate policy](CALIBRATED_GATE_POLICY.md) documents the
operating model. Gates remain explicit, advisory by default, and separate from
runtime mutation vocabulary. The [Campaign 15
closeout](handoffs/2026-05-08-campaign-15-closeout.md) records the final proof
and defers the next product campaign to an explicit decision. Campaign 16 is
closed as Gate Adoption UX: it made explicit gate adoption safe through
copyable generated-CI examples, visible waiver workflows, baseline creation and
refresh guidance, first-screen gate summaries, repo-local dogfood receipts, and
guidance for when blocking is earned by local evidence. The generated-CI
examples for default advisory posture, `visible-only`, `acknowledgeable`,
`baseline-check`, and `calibrated-gate` are documented; the `ripr-waive`
reviewer workflow keeps acknowledgements visible and separate from
suppressions; baseline guidance frames existing findings as visible historical
debt that teams can shrink toward RIPR 0 under configured scope; generated CI
summaries show gate mode, status, labels, waiver, baseline, calibration,
blocking reason, and artifact paths at a glance; and checked repo-local dogfood
receipts cover visible-only, acknowledged, baseline-existing, baseline-new,
missing-baseline, and explicit calibrated-gate decisions while preserving
non-blocking generated CI defaults. [RIPR blocking
readiness](BLOCKING_READINESS.md) explains when to stay advisory, require
acknowledgement, use baseline-check, or enable calibrated blocking. The
[Campaign 16 closeout](handoffs/2026-05-08-campaign-16-closeout.md) records
the adoption proof and next-work boundary. Campaign 17 closed RIPR Zero
Adoption: RIPR-SPEC-0016 now defines the baseline debt delta contract, `ripr
baseline create` can write `.ripr/gate-baseline.json` ledgers from existing
gate-decision evidence, `ripr baseline diff` can report baseline debt
movement, and `ripr baseline update --remove-resolved` can shrink reviewed
baselines without adopting new current debt. Generated CI now uploads and
summarizes baseline debt delta artifacts when a baseline and gate decision are
present. [Baseline ledger workflow](BASELINE_LEDGER_WORKFLOW.md) documents the
user path from advisory adoption to reviewed baseline creation,
`baseline-check`, shrink-only refresh, new debt review, and RIPR 0 under
configured scope. The [Campaign 17
closeout](handoffs/2026-05-09-campaign-17-closeout.md) records the PR chain,
proof commands, and next-work boundary.
Campaign 18 closed as RIPR Zero Reporting. It turns reviewed baselines,
baseline debt deltas, gate decisions, and recommendation evidence into
repo-level RIPR 0 status, baseline age and ownership, stale-debt warnings,
trend summaries, and top repair areas while keeping generated CI advisory by
default. RIPR-SPEC-0017 now pins that reporting contract; baseline metadata,
the status report, generated-CI projection, and the
[RIPR Zero reporting workflow](RIPR_ZERO_REPORTING_WORKFLOW.md) are in place.
The [Campaign 18 closeout](handoffs/2026-05-09-campaign-18-closeout.md)
records the PR chain, proof commands, and next-work boundary.
Editor Evidence UX closed as a separate Lane 3 campaign; its contract audit is
recorded in [Editor Evidence UX](EDITOR_EVIDENCE_UX.md), the saved-workspace
editor loop is documented in the
[editor evidence workflow](EDITOR_EVIDENCE_WORKFLOW.md), and the
[Editor Evidence UX closeout](handoffs/2026-05-09-editor-evidence-ux-closeout.md)
records the prompt-to-artifact audit. Future editor work should be opened as a
new explicit campaign.
Campaign 19 closed as PR Evidence Ledger. It turned RIPR Zero progress from a
current-status surface into an append-only PR adoption history: new
policy-eligible gaps, baseline debt resolved, acknowledgements, suppressions,
gate mode, repair receipts, waiver aging, and optional coverage/grip frontier
signals. RIPR-SPEC-0018 pins the contract, `ripr pr-ledger record` writes the
read-only JSON/Markdown producer, generated CI writes and summarizes
`pr-evidence-ledger.{json,md}` as advisory PR movement evidence, and `ripr
coverage-grip frontier` reports coverage delta and RIPR movement as separate
advisory axes. `docs/PR_EVIDENCE_LEDGER_WORKFLOW.md` documents how teams use
ledger cards for waiver aging, baseline burn-down, repair receipts,
coverage/grip frontier signals, and movement toward RIPR 0. Campaign 20 closed
as Test-Oracle Assistant Proof. RIPR-SPEC-0019 defines the full PR-time proof
loop from changed Rust behavior through static evidence, PR/editor guidance,
focused-test handoff, verification, receipt, and advisory CI/ledger projection
without changing analyzer, policy, editor, or default CI behavior. The
canonical replay corpus pins one boundary-gap seam across recommendation,
handoff, receipt, and ledger projection. A repo-local dogfood receipt traces
seam `67fc764ba37d77bd` through PR guidance, editor/agent handoff,
before/after evidence, receipt, PR ledger projection, and coverage/grip
frontier availability. `docs/TEST_ORACLE_ASSISTANT_WORKFLOW.md` explains the
user-facing PR/editor-to-receipt workflow and static evidence limits. The
[Campaign 20 closeout](handoffs/2026-05-09-campaign-20-closeout.md) records
the prompt-to-artifact audit and the boundary that future proof report
producers, PR/CI polish, analyzer improvements, and editor UX work need an
explicit follow-up campaign. Campaign 21 closed as Test-Oracle Assistant
Report Producer, and `ripr assistant-loop proof` now turns the proved loop into
advisory `test-oracle-assistant-proof.{json,md}` artifacts from explicit
existing inputs without changing analyzer, ranking, gate, editor, provider,
mutation, or default CI behavior. Generated GitHub CI now projects that report
only when the required artifact chain already exists.
`docs/TEST_ORACLE_ASSISTANT_PROOF_REPORT.md` now explains how to read the
report, warnings, static movement, optional CI projection, and limits. Campaign
21 closed with
`docs/handoffs/2026-05-09-campaign-21-closeout.md`. Campaign 22 closed as
First Useful Action. Its product goal was to compress editor, PR, ledger,
proof, receipt, optional gate, coverage/grip, and staleness evidence into one
advisory next test action before adding another raw surface. RIPR-SPEC-0020
defines that report contract, the routing corpus pins actionable and fallback
cases, and `ripr first-action` writes the read-only report from explicit
artifacts. Generated CI surfaces that report as advisory summary and artifact
content. VS Code status and `ripr: Show Status` project an existing
first-action report without rerunning analysis. The
[first useful action workflow](FIRST_USEFUL_ACTION_WORKFLOW.md) documents how
developers, reviewers, and agents read the report, act on the action, verify
movement, emit receipts, and interpret fallback states. `cargo xtask dogfood`
checks repo-local first-action receipts for the documented actionable,
baseline-only, stale, missing-input, unchanged-after-attempt, and no-actionable
routes. The [Campaign 22 closeout](handoffs/2026-05-09-campaign-22-closeout.md)
records the prompt-to-artifact audit and next-lane boundary.

[Assistant Loop Health](ASSISTANT_LOOP_HEALTH_PROPOSAL.md) closed as Campaign
23 after First Useful Action. It summarizes proof completeness, missing inputs,
static evidence movement, recurring warnings, and repair queues across one or
more `test-oracle-assistant-proof` reports without changing analyzer behavior,
ranking, gate semantics, editor behavior, mutation execution, provider calls,
source files, generated tests, or default CI blocking.
[RIPR-SPEC-0022](specs/RIPR-SPEC-0022-assistant-loop-health-report.md)
defines the contract, and
`fixtures/boundary_gap/expected/assistant-loop-health/` pins the complete,
partial, missing-input, unchanged, regressed, warning-heavy, and multi-proof
corpus; `ripr assistant-loop health` writes the advisory report over explicit
proof inputs; generated GitHub CI uploads and summarizes assistant-loop-health
artifacts when proof artifacts exist; the
[assistant loop health workflow](ASSISTANT_LOOP_HEALTH_WORKFLOW.md) explains
maintainer and agent use; and the
[Campaign 23 closeout](handoffs/2026-05-09-campaign-23-closeout.md) records
the final audit and future-lane boundary.

[PR Review Front Panel](PR_REVIEW_FRONT_PANEL_PROPOSAL.md) is now closed as
Campaign 24 after Assistant Loop Health. RIPR-SPEC-0023 now defines the PR
review front-panel report contract over existing PR guidance, first useful
action, assistant proof, assistant-loop health, PR evidence ledger, baseline
delta, gate decision, receipts, calibration, and optional coverage/grip frontier
artifacts, and the fixture corpus pins the first set of reviewer states before
producer work. `ripr pr-review front-panel` now produces the advisory
JSON/Markdown report from explicit existing artifacts, and generated GitHub CI
now uploads and summarizes `pr-review-front-panel.{json,md}` only when input
artifacts exist. The campaign keeps Lane 4's projection boundary: consume
explicit artifacts, do not change analyzer behavior, recommendation ranking,
gate semantics, editor behavior, mutation execution, provider calls, source
files, generated tests, inline-comment defaults, or default CI blocking. The
[PR review front panel workflow](PR_REVIEW_FRONT_PANEL_WORKFLOW.md) now
documents reviewer, maintainer, developer, and coding-agent use of the
first-screen panel. The dogfood report now validates checked front-panel
receipts for actionable, acknowledged, suppressed, baseline-resolved, blocked,
missing-proof, no-actionable, and coverage-flat-grip-improved states. The
[Campaign 24 closeout](handoffs/2026-05-10-campaign-24-closeout.md) records the
PR chain, validation plan, advisory boundary, and future-lane boundary.

[Report Packet Index](REPORT_PACKET_INDEX_PROPOSAL.md) closed as Campaign 25
after PR Review Front Panel. The campaign made
`target/ripr/reports/index.{json,md}` the reviewer front door for the uploaded
`ripr-reports` packet, grouping existing reports, receipts, gates, calibration,
SARIF, badges, and repair artifacts by reviewer use. The boundary stays Lane 4:
consume explicit artifacts, do not change analyzer behavior, recommendation
ranking, gate semantics, editor behavior, mutation execution, provider calls,
source files, generated tests, inline-comment defaults, or default CI blocking.
The spec contract is now
[RIPR-SPEC-0024: Report Packet Index](specs/RIPR-SPEC-0024-report-packet-index.md);
the fixture corpus pins the representative packet states, `ripr reports index`
now writes the read-only index JSON/Markdown artifacts, generated GitHub CI
projects those artifacts when indexed inputs exist, and the
[report packet index workflow](REPORT_PACKET_INDEX_WORKFLOW.md) now documents
reviewer, maintainer, developer, and coding-agent use of the grouped packet
map. `cargo xtask dogfood` now validates checked report-packet index receipts
for complete, sparse, missing-front-panel, blocked-gate, missing-proof,
missing-receipts, and coverage/grip-present cases. The
[Campaign 25 closeout](handoffs/2026-05-10-campaign-25-closeout.md) records
the final audit and future-lane boundary.

[PR Inline Comment Publisher](PR_INLINE_COMMENT_PUBLISHER_PROPOSAL.md) is now
open as Campaign 26 after Report Packet Index. The campaign should make
optional durable PR comments safe by first producing a read-only publish plan
from existing `ripr review-comments` artifacts, then posting only when an
explicit workflow mode and safe permissions are configured.
[RIPR-SPEC-0025](specs/RIPR-SPEC-0025-pr-inline-comment-publisher.md) now pins
the publish-plan schema and permission boundary. The boundary stays Lane 4:
consume explicit artifacts, do not change analyzer behavior, recommendation
ranking, gate semantics, editor behavior, mutation execution, provider calls,
source files, generated tests, branch protection, `pull_request_target`
defaults, inline-comment defaults, or default CI blocking. The fixture corpus
under `fixtures/boundary_gap/expected/pr-inline-comment-publisher/` now pins
the publisher plan cases before producer behavior changes, and
`ripr pr-comments plan` now emits the read-only JSON/Markdown publish plan
without posting to GitHub or changing gate authority. Generated GitHub CI now
keeps `RIPR_COMMENT_MODE=off` by default, uploads/summarizes the plan in opt-in
modes, and posts or updates only safe inline operations when `inline` mode is
explicitly configured. `docs/PR_INLINE_COMMENT_PUBLISHER_WORKFLOW.md` now
documents opt-in modes, publish-plan review, forks, permissions, noise controls,
dedupe/upsert behavior, rollback, and the advisory gate boundary. `cargo xtask
dogfood` now checks repo-local receipts for publishable, summary-only, capped,
dedupe/upsert, stale-existing, fork or no-token, and missing-input publish plans
without posting real comments. Campaign 26 is closed by
`docs/handoffs/2026-05-10-campaign-26-closeout.md`.

[Multi-Language Adapter Preview](proposals/RIPR-PROP-0001-multi-language-adapter-preview.md)
is closed as Campaign 27 after PR Inline Comment Publisher. The campaign
introduced a language-neutral analysis adapter boundary inside the existing
`crates/ripr` package, kept Rust as the reference adapter, and added
syntax-first TypeScript and Python preview adapters that feed the same RIPR
domain, output, LSP, agent, and Lane 4 review surfaces. The boundary stays
explicit: one published package, one binary, one library, one LSP server, one
editor extension; preview adapters are syntax-first, opt-in, and labeled
`preview` in every public surface; Rust analyzer behavior, recommendation
ranking, gate semantics, LSP/editor behavior for Rust seams, mutation
execution, provider behavior, source files, generated tests, branch protection,
`pull_request_target` defaults, and default CI blocking did not change.
[RIPR-SPEC-0026](specs/RIPR-SPEC-0026-language-adapter-contract.md) pins
the language-neutral adapter contract, additive optional `language` and
`language_status` output fields, the `[languages]` repo-config opt-in, and
explicit preview static-limit vocabulary.
[RIPR-SPEC-0027](specs/RIPR-SPEC-0027-typescript-preview-static-facts.md)
pins the TypeScript preview static-fact contract, and
[RIPR-SPEC-0028](specs/RIPR-SPEC-0028-python-preview-static-facts.md) pins
the Python preview static-fact contract. The closeout audit lives at
[Campaign 27 Language Adapter Preview closeout](handoffs/2026-05-13-campaign-27-closeout.md).

[Policy Readiness and Preview Evidence Governance](policy/POLICY_READINESS.md)
is closed as a focused Lane 2 tracker alongside Campaign 27. It does not
replace the active campaign manifest. It defines what stable Rust evidence and
preview-language evidence are allowed to mean for acknowledgement, baseline,
waiver, suppression, calibration, RIPR Zero, and gates. Preview TypeScript and
Python findings remain visible and advisory by default; they do not become
gate-eligible or RIPR Zero blocking debt without a later explicit policy
promotion. Generated CI may surface waiver-aging, suppression-health, and
policy-readiness artifacts, but those remain advisory projections with no
default blocking or comment posting. The closeout audit is
[Policy Readiness closeout](handoffs/2026-05-12-policy-readiness-closeout.md).

[Policy Operations and Promotion Readiness](policy/POLICY_OPERATIONS.md) is
closed as the next focused Lane 2 tracker. It turns readiness into advisory
operator packets: current safe policy ceiling, next safe action, blockers to
stricter modes, read-only promotion packets, preview-promotion packets, and
history/trend surfaces. RIPR-SPEC-0039 defines the first policy operations
report contract, `ripr policy operations` now writes that packet, and
RIPR-SPEC-0041 defines the policy history trend contract, RIPR-SPEC-0042
defines manual-review promotion packets, and RIPR-SPEC-0044 defines
default-blocked preview evidence promotion packets. `ripr policy history` now
writes the read-only trend packet over policy operations plus optional history
JSONL, and `ripr policy promote --to ...` now writes read-only promotion
packets from policy operations plus optional policy history. `ripr policy
preview-promote` now writes default-blocked preview evidence promotion packets
for TypeScript and Python classes. [Policy operations workflow](POLICY_OPERATIONS_WORKFLOW.md)
documents how maintainers review the packets before manual config changes.
Generated CI now renders, uploads, indexes, and summarizes operations, history,
promotion, and configured preview-promotion packets as advisory-only artifacts
with no new gate authority. The closeout audit is
[Policy Operations closeout](handoffs/2026-05-13-policy-operations-closeout.md).
This tracker preserves the same boundaries: no analyzer truth changes, no
editor changes, no generated tests, no mutation execution, no default CI
blocking, no automatic config or baseline mutation, and no preview evidence
promotion.

## Current Operating Sequence: 0.9.0 Release and Evidence-To-Repair Routing

This section is the bridge between the closed campaign history above and the
next product era. It records where the repo is, the end state the next era
targets, and the only sanctioned path between them, so a maintainer or agent
can resume this work from repository artifacts instead of chat history.

### Where the repo is

- The evidence-to-repair use-case spec spine is landed:
  [RIPR-SPEC-0065](specs/RIPR-SPEC-0065-evidence-to-repair-use-case-roadmap.md)
  owns the use-case map, and child specs RIPR-SPEC-0066 through
  RIPR-SPEC-0073 pin repo badge, PR gate, PR review cards, LSP/agent
  feedback, downstream review consumers, TypeScript/Bun evidence, large-repo
  diff-first operation, and receipts/outcomes/route quality.
- [RIPR-PLAN-0061](../plans/use-case-specs/implementation-plan.md) sequences
  the implementation work items for that spine. Active goals route through
  that plan; this roadmap section does not create ready work items.
- The post-freeze merge wave has been drained: the valid PRs deferred at
  freeze time were merged or closed with exact supersession citations.
- The [0.9.0 swarm freeze note](handoffs/2026-06-05-0.9.0-swarm-freeze.md)
  records the original candidate SHA and the standing back-sync drift. Its
  deferred-PR list is stale by design: the merge wave landed the valid
  deferred work after the note was written, so the candidate SHA must be
  re-pinned before release.

### Target end state

RIPR becomes the evidence-to-repair router for real repositories. The
product shape is two queues:

```text
1. Repair queue
   Actionable packets safe to delegate, verify, receipt, and measure.

2. Limitation backlog
   Non-actionable analyzer work with named limits, samples, unlock
   conditions, and non-claims.
```

One trust rule controls every surface (CLI, PR gate, review comments, LSP,
badge input, CI summary, downstream consumers, previews, large-repo mode,
receipts, route-quality reporting):

```text
Actionable means safe to delegate.
Everything else fails closed into a named limitation, blocked state,
or advisory preview.
```

RIPR-SPEC-0065 owns the canonical definition of done for this end state;
this roadmap defers to it rather than restating the field contracts.

### Phase P: proof-aware repo validation

Before the release phase, the repo gains the same routing discipline the
product ships: changed surfaces select proof packs; proof packs select
local preflight commands and CI lanes; unknown surfaces route
conservatively; release proof is never routed away. The operating model is
[Proof routing](PROOF_ROUTING.md), and it lands in slices: operating model,
`policy/proof-packs.toml` manifest, `cargo xtask proof route` report,
`cargo xtask proof preflight`, PR summary integration, a CI dry-run
artifact that demonstrates routing safety before any lane is skipped, then
low-risk docs/spec routing, report/schema routing, and release-proof
protection. CI stops being first discovery; measured 2026-06-07, docs-only
PRs were paying full heavy-lane proof that their changed surfaces could
not fail.

### Phase R: publish 0.9.0 from source

| Order | Step | Acceptance |
| ---: | --- | --- |
| R1 | Re-pin the freeze candidate to reconciled swarm `main` and refresh the freeze note. | New candidate SHA recorded; deferred-PR list replaced with actual merge dispositions; no valid PR deferred by a cut line. |
| R2 | Refresh the source release PR (`ripr` #1424) onto the new candidate. | Source PR carries current swarm `main`; version and changelog claims match merged behavior; `cargo publish -p ripr --dry-run --locked` passes; source release authority unchanged. |
| R3 | Tag and publish `0.9.0` from source. | `v0.9.0` tag exists; crate published; `cargo install ripr --version 0.9.0 --locked` smoke passes; the GitHub release uses the reviewed changelog. |
| R4 | Back-sync source `main` into swarm `main` with a merge commit. | Version and changelog parity restored (swarm `crates/ripr/Cargo.toml` still carries `0.7.0`; the `0.8.0` bump was source-side only); ahead/behind accounting works for future releases. This step is mandatory, not optional cleanup. |
| R5 | Post-release cleanup sweep. | Stale worktrees, branches, stashes, `target/ripr` cache growth, generated reports, temp files, orphaned local processes, and rescue leftovers are removed; trunk is ready for implementation work. |

### Phase I: implement from RIPR-PLAN-0061

After R5, implementation resumes only through the active goal manifest
routed at [RIPR-PLAN-0061](../plans/use-case-specs/implementation-plan.md),
in the plan's own sequence: review file-and-line navigation, badge
projection of canonical actionability and runtime status, advisory PR gate
evidence delta, the first useful LSP action, downstream consumer export,
the bounded TypeScript adapter slices, cross-language fail-closed routing,
large-repo diff-first operation, receipts and attempt-ledger hardening,
route-quality metrics, and surface alignment.

Each slice keeps the scoped PR contract: one production delta, one evidence
package, explicit claim boundary, explicit non-goals.

### Operating rules for this sequence

- Work selection flows through `.ripr/goals/active.toml`; roadmap
  eligibility is not execution authority.
- PRs close only as merged, superseded with an exact replacement citation
  (PR, commit, or spec), or invalid with a cited reason. No vague deferral.
- Actionability is never invented: raw findings are diagnostic material,
  not product truth, and wrong-language test suggestions are forbidden.
- Unknown cross-language oracle visibility stays a named limitation, never
  a repair packet.
- No provider integration, autonomous edits, mutation execution, broad
  refactors, or badge/CI semantic switches unless a spec-backed PR
  requires them.

## Strategic Sequence

The load-bearing path is:

```text
quality rails
-> traceability
-> capability metrics
-> fixture/golden tooling
-> dogfooding checks
-> install verification
-> fixture lab
-> file facts
-> syntax facts
-> probe ownership
-> probe generation
-> local flow facts
-> oracle facts
-> activation/value facts
-> evidence findings
-> LSP evidence loop
-> agent context
-> repo seam inventory and test grip
-> seam fact cache
-> related-test, value, and oracle-shape precision
-> seam-native LSP actions
-> runtime calibration import
-> repository config
-> SARIF and CI policy
-> seam-native badge counts
-> operationalization closeout
-> Campaign 6 stack audit
-> Campaign 6 modularization closeout
-> defaults-first operator adoption
-> runtime calibration fixture expansion
-> Campaign 8 closeout
-> hot-sidecar latency proof
-> editor-agent integration
-> editor-agent release readiness proof
-> LLM work loop
-> first-hour UX
-> PR review guidance
-> recommendation calibration
-> calibrated gate policy
-> gate adoption UX
-> RIPR Zero adoption
-> RIPR Zero reporting
-> editor evidence UX
-> PR evidence ledger
-> test-oracle assistant proof
-> test-oracle assistant report producer
-> first useful action
-> assistant loop health
-> PR review front panel
-> report packet index
-> PR inline comment publisher
-> language adapter preview
-> policy readiness and preview evidence governance
-> policy operations and promotion readiness
```

The analyzer path is:

```text
fixture lab
-> file facts
-> syntax facts
-> probe ownership
-> probe generation
-> local flow facts
-> oracle facts
-> activation/value facts
-> evidence findings
-> LSP evidence loop
-> agent context
-> repository config
-> calibration
-> cache
```

Do not skip ahead to MIR, Charon, a hard HIR dependency, SQLite-first storage,
large dashboards, broad LSP features, or more probe families before the current
probe families are grounded in better facts.

## Quality Rail Sequence

Before large analyzer work, add the repo machinery that makes future PRs easy to
write and review:

| Order | PR | Purpose |
| ---: | --- | --- |
| Q1 | `engineering-doctrine-rails` | Scope PRs by production risk, separate production and evidence deltas, add issue templates, capability matrix, traceability seed, and first policy checks. |
| Q2 | `rust-first-file-policy` | Deny unapproved non-Rust programming files, executable bits, and workflow shell sprawl through allowlisted policy checks. |
| Q3 | `spec-fixture-contracts` | Require agent-readable spec sections and BDD fixture contracts before fixture/golden work expands. |
| Q4 | `automation-guardrails` | Require allowlisted generated files, dependency surfaces, process spawning, and network behavior. |
| Q5 | `shape-fix-pr` | Add safe local PR normalization and report writing through `cargo xtask shape` and `cargo xtask fix-pr`. |
| Q6 | `pr-summary` | Generate a reviewer packet from changed paths, policy exceptions, and suggested focus areas. |
| Q7 | `check-pr-precommit` | Add obvious local gates for cheap pre-commit checks and review readiness checks. |
| Q8 | `guided-check-reports` | Make policy checks emit repair briefs with fix kind, why-it-matters text, commands, and exception templates. |
| Q9 | `ci-report-artifacts` | Upload `target/ripr/reports` from CI so failed runs still produce reviewer guidance. |
| Q10 | `fixture-golden-scaffolding` | Add fixture/golden conventions plus scaffold/check/bless commands. |
| Q11 | `traceability-spec-id-checks` | Validate behavior manifests, spec IDs, fixture links, and drift warnings. |
| Q12 | `capability-metrics-report` | Generate capability and quality metrics artifacts from fixtures and traceability. |
| Q13 | `architecture-boundary-check` | Add workspace-shape, public API, and module-boundary checks that preserve one crate with strong internal seams. |
| Q14 | `dogfood-report` | Add focused `ripr`-on-`ripr` reports as CI artifacts without blocking by default. |

These PRs should remain narrow production changes. Most of their size may be
evidence, docs, templates, allowlists, or generated scaffolding. That is
intentional: future analyzer PRs should start with a clear spec, fixture,
golden-output path, metric, and mechanical check.

These rails are also agent-context infrastructure. Long-running agent work
should not depend on a single chat transcript. Roadmap items, specs,
traceability, capability status, metrics, ADRs, and learnings are the durable
handoff surface that lets an agent resume, subset the next slice, and finish one
reviewable PR without guessing.

The operating model for those rails is documented in
[PR automation](PR_AUTOMATION.md): deterministic cleanup is shaped locally,
non-negotiable rules are checked, and judgment-required issues produce repair
briefs. Codex Goals campaign work is documented in
[Codex Goals](CODEX_GOALS.md), [Implementation campaigns](IMPLEMENTATION_CAMPAIGNS.md),
and [Scoped PR contract](SCOPED_PR_CONTRACT.md).

## Operating-System Cutoff

Do not wait for perfect automation before analyzer work. The cutoff is enough
repo machinery that a future analyzer PR is pushed toward one production delta,
one evidence package, and an actionable PR summary.

Required before deeper analyzer mode:

- PR summary and reviewer packet
- `precommit` and `check-pr` command surface
- guided reports for existing policy checks
- CI report artifacts
- fixture and golden command scaffolding
- behavior manifest and spec ID checks
- capability metrics report
- architecture and workspace-shape guard

Nice later, not blocking:

- test-oracle quality report
- full docs-as-tests suite
- auto-labeling
- learning-required triggers
- public API compatibility checker
- local hook polish

## PR Queue

| Order | PR | Purpose | Release target |
| ---: | --- | --- | --- |
| 0 | `planning-and-tracking-docs` | Put the product plan, metrics, and contribution rules in-repo. | `0.2.x` |
| 1 | `engineering-doctrine-rails` | Make scoped evidence-heavy PRs mechanical with templates, traceability, capability status, and first policy checks. | `0.2.x` |
| 2 | `rust-first-file-policy` | Keep implementation and automation Rust-first by allowlisting non-Rust programming surfaces and checking workflow shell budgets. | `0.2.x` |
| 3 | `spec-fixture-contracts` | Make specs and fixtures mechanically checkable with required sections and BDD fixture contracts. | `0.2.x` |
| 4 | `automation-guardrails` | Require allowlisted generated files, dependency surfaces, process spawning, and network behavior. | `0.2.x` |
| 5 | `shape-fix-pr` | Add safe local PR normalization and report writing through `cargo xtask shape` and `cargo xtask fix-pr`. | `0.2.x` |
| 6 | `pr-summary` | Generate a reviewer packet from changed paths, policy exceptions, and suggested focus areas. | `0.2.x` |
| 7 | `automation-path-docs` | Document the fix/check/guide model and Codex Goals campaign handoff before the remaining rails. | `0.2.x` |
| 8 | `check-pr-precommit` | Add `cargo xtask precommit` and `cargo xtask check-pr` as the obvious local gates. | `0.2.x` |
| 9 | `guided-check-reports` | Make existing policy checks write actionable Markdown repair briefs. | `0.2.x` |
| 10 | `ci-report-artifacts` | Upload generated PR reports from CI. | `0.2.x` |
| 11 | `verify-one-click-extension-install` | Verify the normal editor install path without requiring `cargo install ripr`. | `0.2.x` |
| 12 | `fixture-golden-scaffolding` | Add fixture/golden structure, scaffold command, check command, and bless command. | `0.3.0` |
| 13 | `traceability-spec-id-checks` | Validate spec IDs, behavior manifests, fixture links, and drift warnings. | `0.3.0` |
| 14 | `fixture-laboratory` | Create golden fixtures and invariants before changing the analyzer. | `0.3.0` |
| 15 | `capability-metrics-report` | Generate capability, quality, engineering, and latency metrics artifacts. | `0.3.0` |
| 16 | `architecture-boundary-check` | Enforce internal module boundaries while keeping one published crate. | `0.3.x` |
| 17 | `dogfood-report` | Emit focused `ripr`-on-`ripr` reports as non-blocking artifacts. | `0.3.x` |
| 18 | `file-facts-model` | Introduce a fact model while preserving current scanner behavior. | `0.3.0` |
| 19 | `syntax-adapter-mvp` | Add a parser adapter boundary and syntax-backed file facts. | `0.3.0` |
| 20 | `ast-test-oracle-extraction` | Extract tests and assertions from syntax nodes. | `0.3.0` |
| 21 | `ast-probe-ownership` | Map diff spans to changed syntax nodes and stable owner symbols. | `0.3.0` |
| 22 | `ast-probe-generation` | Generate predicate, return, error, field, and call probes from syntax. | `0.3.0` |
| 23 | `oracle-strength-v2` | Distinguish exact, weak, smoke, snapshot, mock, and unknown oracles. | `0.4.0` |
| 24 | `local-delta-flow-v1` | Name return, field, error, and effect sinks for changed behavior. | `0.4.0` |
| 25 | `activation-value-modeling-v1` | Detect observed values and missing boundary or variant inputs. | `0.4.0` |
| 26 | `evidence-first-output` | Make CLI output the reference explanation for each finding. | `0.4.0` |
| 27 | `test-efficiency-test-fact-ledger` | Emit advisory per-test ledgers from existing owner, oracle, value, and evidence facts. | `0.4.x` |
| 28 | `test-efficiency-vacuous-signal-v1` | Classify likely vacuous, smoke-only, broad-oracle, opaque, and circular test signals with evidence. | `0.4.x` |
| 29 | `test-efficiency-duplicate-discriminator-v1` | Group tests with duplicate owner, activation, oracle, and sink evidence. | `0.4.x` |
| 30 | `lsp-evidence-hover-actions` | Add finding-specific diagnostics, hover evidence, and code actions. | `0.5.0` |
| 31 | `agent-context-v2` | Emit a compact test-writing brief from CLI and LSP. | `0.5.0` |
| 32 | `ripr-config-v1` | Add topology, oracle, snapshot, mock, severity, suppression, and seam-diagnostic config. | `0.6.0` |
| 33 | `suppression-v1` | Add reasoned, visible suppressions with optional expiry. | `0.6.0` |
| 34 | `sarif-ci-policy` | Add SARIF and opt-in CI policy modes. | `0.6.0` |
| 35 | `seam-native-count-mapping` | Remap `ripr` and `ripr+` badge artifacts onto seam-native unresolved gap counts. | `0.6.x` |
| done | `repo-seam-facts-cache` | Cache seam fact layers after the fact model became stable enough. | Campaign 5A |
| done | `cargo-mutants-calibration-scaffold` | Import real mutation results for offline calibration. | Campaign 5A |
| 36 | `language-adapter-preview` | Introduce a language-neutral adapter boundary, keep Rust as the reference adapter, and add TypeScript and Python preview adapters that feed the existing RIPR domain, output, LSP, agent, and Lane 4 surfaces without changing Rust behavior or default CI blocking. | `0.9.0` |
| 37 | `policy-readiness-preview-evidence-governance` | Define when stable Rust and preview-language evidence can be shown, acknowledged, suppressed, baselined, calibrated, counted toward RIPR Zero, or gated without changing advisory defaults. | Lane 2 |

## Release Frames

### `0.3.0` - Evidence Foundation

Ship:

- fixture and golden scaffolding
- fixture laboratory
- capability metrics report
- stable output DTOs
- file facts
- parser adapter MVP
- AST-backed test and oracle extraction
- AST-backed probe ownership and generation

Success condition:

```text
Existing sample findings come from fact objects instead of line-substring guesses.
```

### `0.4.0` - Editor Agent Integration

Ship:

- LSP copy commands for agent packet, brief, after-snapshot, verify, and receipt
- operator cockpit status for before/after snapshots, agent verify JSON, agent
  receipt JSON, movement counts, and missing-input next commands
- one canonical editor-agent loop fixture that pins diagnostics, actions, agent
  brief, agent packet, verify, receipt, and cockpit output
- generated CI artifacts for the non-blocking editor-agent loop
- first-hour docs centered on `ripr pilot`, one focused test, after snapshot,
  `ripr outcome`, `ripr agent verify`, `ripr agent receipt`, editor, CI, and
  known limits
- release-readiness proof for installed CLI, packaged VSIX, package dry-run, and
  known-limits surfaces after the loop is pinned

Success condition:

```text
saved-workspace diagnostic -> packet/brief -> focused test -> after snapshot -> agent verify -> agent receipt -> cockpit/CI artifact
```

### `0.5.0` - Live Editor Loop

Ship:

- finding-specific diagnostics
- evidence hovers
- copy-context action
- open-related-tests action
- deep-check command
- agent context v2

Success condition:

```text
A developer can hover a diagnostic, understand the gap, and copy a test intent.
```

### `0.6.0` - Repository Adaptation

Ship:

- `ripr.toml` v1
- custom oracle macros
- snapshot, mock, and effect config
- reasoned suppressions
- SARIF and opt-in CI policy modes

Success condition:

```text
Real repositories can teach ripr their testing idioms without hiding findings.
```

### `0.7.0` - Calibration

Campaign 5A shipped the advisory scaffold:

- `cargo-mutants` import
- static-vs-real mutation reports

Future calibration work can add:

- family-specific precision measurements
- explicit calibration fixtures or bounded runtime artifacts

Success condition:

```text
ripr can compare static exposure classes with real mutation results when explicit mutation data is present.
```

### `0.9.0` - Language Adapter Preview

The shipped 0.9.0 content is recorded in the
[0.9.0 swarm freeze note](handoffs/2026-06-05-0.9.0-swarm-freeze.md); the
freeze candidate is re-pinned during Phase R of the
[current operating sequence](#current-operating-sequence-090-release-and-evidence-to-repair-routing).

Ship:

- a `LanguageAdapter` boundary inside `crates/ripr/src/analysis/` with Rust
  as the reference adapter
- additive optional `language` and `language_status` output fields across
  existing reports without schema forks
- a TypeScript/JavaScript preview adapter with syntax-first owner, test,
  assertion, related-test, and probe extraction plus explicit static-limit
  reporting
- a Python preview adapter with syntax-first owner, test, assertion,
  related-test, and probe extraction plus explicit static-limit reporting
- repo configuration through `[languages] enabled = ["rust", ...]`, default
  Rust-only
- VS Code extension language selectors covering TypeScript, JavaScript, and
  Python once preview adapters are enabled, without changing saved-workspace
  defaults
- generated CI advisory grouping by language when more than Rust is
  enabled, with Rust-default behavior unchanged
- fixture and dogfood coverage for the preview adapters

Success condition:

```text
A TypeScript or Python PR produces the same kind of advisory answer as
Rust: what behavior changed, whether related tests appear to reach it,
whether an assertion appears to discriminate it, what static limits apply,
and what focused test action to take next, all without changing Rust
behavior, recommendation ranking, gate semantics, LSP/editor behavior for
Rust seams, mutation execution, provider behavior, source files, generated
tests, or default CI blocking.
```

### `0.8.0` - Hot Sidecar

Campaign 5A shipped the first seam fact cache. Future hot-sidecar work can add:

- incremental in-memory store
- file-hash invalidation
- warm fact reuse
- persisted cache when needed

Success condition:

```text
Common editor edits reclassify without rescanning the full workspace.
```

## Canonical Acceptance Scenario

Use one case as the product-in-miniature:

```rust
if amount >= discount_threshold {
    apply_discount(...)
}
```

Existing tests:

```rust
#[test]
fn premium_customer_gets_discount() {
    let quote = price(10_000);
    assert!(quote.total > Money::zero());
}

#[test]
fn small_customer_gets_no_discount() {
    let quote = price(50);
    assert_eq!(quote.discount_applied, false);
}
```

Expected diagnostic:

```text
Changed boundary has no detected equality-boundary test.
```

Expected hover:

```text
Static exposure: weakly_exposed

Changed:
  amount >= discount_threshold

Evidence:
  premium_customer_gets_discount reaches price
  detected amount values: 50, 10_000
  assertion: assert!(quote.total > Money::zero())
  oracle strength: weak

Missing:
  amount == discount_threshold
  exact assertion on discount_amount
  exact assertion on total
```

Expected context packet:

```json
{
  "task": "write_targeted_test",
  "gap": "boundary_gap",
  "arrange": "build input where amount == discount_threshold",
  "act": "call price",
  "assert": [
    "discount_applied == true",
    "discount_amount == expected_discount",
    "total == expected_total"
  ]
}
```

Expected close condition:

```text
When a related test covers amount == discount_threshold and checks the changed
outputs with exact assertions, the finding disappears or downgrades.
```

## Validation Gate Before Deeper Semantics

Before investing in HIR, MIR, Charon, persistent storage, or wider probe
families, the product should satisfy this suite:

| Area | Required evidence |
| --- | --- |
| Distribution | Marketplace and Open VSX install paths work without requiring `cargo install ripr`. |
| Distribution | Verified server download starts `ripr lsp --stdio`. |
| Analyzer | Duplicate function names do not cross-link tests. |
| Analyzer | Stacked test attributes are detected. |
| Analyzer | Multi-line assertions are extracted. |
| Analyzer | Boundary probes report missing equality values. |
| Analyzer | Error-path probes distinguish broad error checks from exact variant checks. |
| Analyzer | Return-value probes distinguish exact assertions from smoke assertions. |
| Analyzer | Local flow names at least one sink or reports a stop reason. |
| Output | Static output never uses mutation-runtime outcome language. |
| Output | Unknowns carry stop reasons. |
| LSP | Diagnostics include finding and probe metadata. |
| LSP | Hover shows evidence path and missing discriminator. |
| LSP | Code action copies the exact context packet. |
| Agent | Context packet includes related tests, missing values, and suggested assertion shape. |
| Calibration | Imported mutation results are shown only as explicit calibration data. |

## Documentation Tracking

Every significant PR should update the matching docs:

- product behavior: [Static exposure model](STATIC_EXPOSURE_MODEL.md)
- JSON or context shape: [Output schema](OUTPUT_SCHEMA.md)
- architecture or module seams: [Architecture](ARCHITECTURE.md)
- test strategy or gates: [Testing](TESTING.md)
- roadmap status or sequencing: this file
- decisions that should not be re-litigated: [ADR directory](adr/)
- contributor learnings: [Learnings](LEARNINGS.md)
