# RIPR-PROP-0004: PR / CI Review Cockpit

Status: proposed

Owner: ripr maintainers

Created: 2026-05-13

Target campaign: Lane 4 PR / CI Review Cockpit

Linked specs:

- [RIPR-SPEC-0023: PR Review Front Panel Report](../specs/RIPR-SPEC-0023-pr-review-front-panel-report.md)
- [RIPR-SPEC-0024: Report Packet Index](../specs/RIPR-SPEC-0024-report-packet-index.md)
- Future generated PR CI review workflow spec using the next available spec ID.

Linked ADRs:

- None required for the proposal. Add an ADR only if a later slice changes a
  durable workflow architecture decision.

Linked work items:

- [Lane 4 implementation plan](../../plans/lane4-pr-ci-review-cockpit/implementation-plan.md)
- [Lane 4 tracker](../lanes/LANE_4_PR_CI_REVIEW.md)

## Problem

RIPR already produces useful PR-time artifacts: PR guidance, first useful
action, assistant proof and health, baseline debt movement, RIPR Zero status,
PR evidence ledger, optional gate decisions, receipts, PR review front panel,
report packet index, and generated CI summaries.

The product gap is not that those artifacts are absent. The gap is that a busy
reviewer or coding agent still has to learn the artifact topology before
answering the first-minute questions:

```text
What changed?
What test gap matters most?
Is this PR-local risk, old baseline debt, waived, suppressed, blocked,
already improved, or missing evidence?
What should happen next?
Which artifact supports that next step?
Which command regenerates a missing surface?
What, if anything, has configured pass/fail authority?
```

At high PR volume, the limiting factor is review compression. Lane 4 should
make generated PR and CI output feel like a cockpit: start here, read the top
issue, see policy state, run the next command, inspect the receipt, and know
which artifact carries authority.

The lane must keep the product boundary clear. Lane 4 composes explicit RIPR
artifacts; it does not create analyzer truth.

## Users and surfaces

- Reviewers reading generated GitHub job summaries and uploaded artifacts.
- Maintainers deciding whether a PR needs repair, waiver review, baseline
  movement, or gate attention.
- Coding agents consuming PR packets and repair commands.
- Developers using generated CI to understand the most useful next test action.
- Repo operators maintaining generated workflow defaults, artifact upload
  shape, and gate authority boundaries.

Touched surfaces:

- generated GitHub CI;
- PR review front panel;
- report packet index;
- generated job summary;
- uploaded artifact packet;
- repair and agent handoff commands;
- receipt and movement summaries;
- gate authority pointers;
- optional language-aware advisory grouping.

## Success criteria

- PR review front panel is the first-screen PR story when its inputs exist.
- Report packet index is the uploaded packet's start-here map.
- Generated job summaries use reviewer language before schema or file topology.
- Missing expected artifacts remain visible with regeneration commands when
  known.
- Gate authority remains explicit and separate: `gate-decision.{json,md}` is
  the configured pass/fail authority when gate mode is enabled.
- Generated summaries, front panels, packet indexes, and language groups remain
  advisory projections by default.
- Language-aware grouping appears only when `[languages]` declares more than
  Rust.
- Rust-default generated CI behavior remains unchanged.
- Preview-language evidence is labeled preview and advisory.
- The lane has durable why, behavior, sequencing, execution-state, policy,
  evidence, and closeout artifacts without making one document do every job.

## Proposed shape

Lane 4 should compose the PR/CI story in this order:

```text
PR diff
-> advisory generated workflow
-> PR review front panel
-> report packet index
-> optional language-aware grouping
-> repair or agent handoff
-> receipt and movement state
-> explicit gate authority boundary
```

The source-of-truth stack stays layered:

| Artifact | Job |
| --- | --- |
| Proposal | why the lane exists, user value, alternatives, success criteria |
| Spec | behavior contracts, required evidence, acceptance examples |
| ADR | durable architecture decisions only |
| Plan | PR-sized sequencing and proof commands |
| Active manifest | machine-readable agent execution state |
| Policy ledger | CI, gate, exception, and allowlist authority |
| Closeout | what shipped, what did not change, validation, remaining work |
| Capability map | public claim to evidence linkage |

Current `main` already ships the front-panel and packet-index producers and
generated-CI projections from Campaigns 24 and 25. This proposal does not ask
future agents to duplicate them. Remaining work should normalize role
boundaries, add the generated workflow behavior contract, map current generated
CI cockpit gaps, add any missing lane-local execution manifest, improve
advisory doc-role validation, and later wire language-aware grouping only when
preview-language evidence is ready.

## Alternatives considered

| Alternative | Why we are not picking it |
| --- | --- |
| Keep the current pile of artifacts and rely on docs to explain them. | The artifact stack is correct but costly to consume. Reviewers and agents need a first-screen story, not a directory lesson. |
| Make the job summary itself the gate. | Summaries are projections and can be incomplete. Gate authority belongs to explicit gate-decision artifacts. |
| Rerun hidden analysis inside the front panel or packet index. | That would make a projection surface create evidence and would make stale or missing inputs hard to trust. |
| Fold Lane 4 into analyzer, editor, or policy work. | Lane 4 consumes those surfaces but does not own their truth, editor routing, or authority semantics. |
| Duplicate existing front-panel and packet-index producers under a new cockpit command. | Current `main` already ships those producers. Duplicating them would create competing public surfaces without new behavior value. |
| Turn preview-language grouping on by default. | Rust-default generated CI must stay unchanged; preview evidence needs explicit opt-in and visible labeling. |

## Behavior specs to create or update

- Update [RIPR-SPEC-0023](../specs/RIPR-SPEC-0023-pr-review-front-panel-report.md)
  with explicit role metadata and a short role section.
- Update [RIPR-SPEC-0024](../specs/RIPR-SPEC-0024-report-packet-index.md)
  with explicit role metadata and a short role section.
- Add a generated PR CI review workflow spec using the next available
  `RIPR-SPEC-00NN` identifier. Current `main` already uses
  `RIPR-SPEC-0032` through `RIPR-SPEC-0037`, so this lane must not reuse
  those IDs.

## Architecture decisions needed

No ADR is needed for this proposal. The source-of-truth layering is a docs and
operator model, and the current Lane 4 surfaces remain read-only projections.

Add an ADR only if a later implementation slice changes a durable architecture
choice, such as introducing a new workflow composition boundary or changing the
public generated workflow contract in a way that outlives the spec.

## Implementation campaign shape

The lane sequence is maintained in the
[Lane 4 implementation plan](../../plans/lane4-pr-ci-review-cockpit/implementation-plan.md).
The current shape is:

1. `docs/lane4-source-of-truth` - done by the source-of-truth scaffolding PR.
2. `docs/lane4-proposal` - this proposal.
3. `docs/lane4-spec-role-alignment`.
4. `docs/generated-pr-ci-review-workflow-spec`.
5. `plans/report-packet-index`.
6. `plans/pr-review-front-panel`.
7. `goals/lane4-active-manifest`.
8. `xtask/check-doc-roles-lane4`.
9. `audit/pr-review-front-panel-current-state`.
10. `audit/report-packet-index-current-state`.
11. `docs/generated-ci-cockpit-gap-map`.
12. `ci/language-aware-grouping` after preview-language readiness.
13. `dogfood/lane4-cockpit-gap-receipts`.
14. `docs/lane4-closeout`.

Each slice follows the [scoped PR contract](../SCOPED_PR_CONTRACT.md).

## Evidence plan

- Proposal, lane tracker, implementation plan, and proposal index links must
  stay in sync.
- RIPR-SPEC-0023 and RIPR-SPEC-0024 role metadata should link to this proposal
  and the Lane 4 plan.
- The generated-CI workflow spec should name command surfaces, artifact uploads,
  job-summary sections, missing-artifact behavior, language grouping rules, and
  gate authority boundaries.
- Generated workflow tests should preserve Rust-default behavior while adding
  any new configured behavior.
- Fixture and golden changes should be added only when behavior or output
  contracts change.
- Dogfood receipts should cover remaining cockpit gaps without duplicating
  Campaign 24 and Campaign 25 receipts.
- Capability and traceability surfaces should update only when a public claim
  or spec-test-code linkage changes.
- Closeout should record shipped surfaces, validation commands, remaining work,
  known limits, and next-lane handoff.

## Feedback loop

The cockpit should be read through reviewer and agent questions, not through
artifact count. Each follow-up slice should ask:

- Can a reviewer find the top issue in under a minute?
- Can an agent copy the next command without guessing paths?
- Is missing, stale, preview, waived, suppressed, baseline, blocked, or
  improved evidence labeled?
- Is pass/fail authority still attached only to the configured gate decision?
- Does the packet index tell the reader which artifact to open first?

When review feedback shows the plan is out of date, update the plan before
starting implementation. The previous source-of-truth PR found one such issue:
front-panel and packet-index producers are already shipped and must be treated
as baseline dependencies rather than future TODO work.

## Risks

- Duplicating shipped surfaces. Mitigation: the plan starts from current
  `main` and records front-panel and packet-index producers as baseline.
- Authority drift. Mitigation: every projection names gate decisions as
  authority when configured and keeps summaries advisory.
- Scope creep into analyzer, editor, policy, or inline-comment lanes.
  Mitigation: lane tracker and spec non-goals keep those boundaries explicit.
- Preview evidence overstatement. Mitigation: language grouping is opt-in and
  preview-language evidence remains visibly advisory.
- Doc-role drift. Mitigation: proposal, spec, ADR, plan, manifest, policy, and
  closeout artifacts each keep one job.
- Spec-number collision. Mitigation: use the next available spec number from
  current `main`, not stale packet numbering.

## Non-goals

- No analyzer behavior changes.
- No recommendation ranking changes.
- No mutation execution.
- No source edits.
- No generated tests.
- No provider or API calls.
- No LSP or editor behavior changes.
- No gate semantic changes.
- No branch-protection changes.
- No default CI blocking changes.
- No inline comment publishing changes.
- No duplicate front-panel or packet-index producers.
- No hidden artifact discovery or hidden analysis reruns.
- No runtime correctness, sufficiency, or complete-test-suite claims.

## Exit criteria

This proposal can move to `accepted` when:

- RIPR-SPEC-0023 and RIPR-SPEC-0024 clearly state their behavior-contract role.
- A generated PR CI review workflow spec exists and is linked from the plan.
- The Lane 4 plan and any lane manifest reflect current shipped surfaces and
  remaining gaps.
- Any doc-role checker extension remains advisory and has low-noise repair
  output.
- Generated CI cockpit changes preserve Rust-default behavior, advisory
  defaults, and gate-decision authority.
- Language-aware grouping is either complete after preview-language readiness
  or explicitly left as remaining work.
- Dogfood receipts and closeout docs record what shipped, what did not change,
  validation, known limits, and next-lane handoff.
