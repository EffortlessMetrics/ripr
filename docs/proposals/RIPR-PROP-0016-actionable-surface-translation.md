# RIPR-PROP-0016: Actionable Surface Translation

Status: accepted

Owner: cross-surface / repo-infra

Created: 2026-05-23

Target campaign: Actionable Surface Translation

Linked specs:

- `RIPR-SPEC-0059`: Actionable surface translation

Linked ADRs:

- None. Add an ADR only if a later slice centralizes a durable presentation
  authority decision or changes surface ownership.

Linked work items:

- `docs/actionable-surface-translation-stack`
- `badge/actionable-basis-presentation`
- `pr/actionable-delta-front-panel`
- `editor/repair-first-status-hierarchy`
- `swarm/dry-run-copy-ready-packet`
- `outcome/movement-front-section`
- `campaign/actionable-surface-translation-closeout`

Support-tier impact:

- None for the proposal and activation slice. Later surface PRs must state
  whether a user-facing claim changes and keep advisory/static boundaries
  intact unless a separate support-tier PR promotes a claim.

Policy impact:

- Register the proposal, spec, and implementation plan in
  `policy/doc-artifacts.toml`.

## Problem

RIPR now has the right unit for repair-oriented surfaces: actionable canonical
gaps with repair routes, verify commands, receipt commands, and bounded packet
context. The user-facing surfaces still do not all translate that unit in the
same way.

That creates a product risk. A badge, PR summary, editor status, swarm dry-run,
and outcome report can each be technically correct while forcing the user to
reconstruct what the number or packet means.

The product loop should be simpler:

```text
one actionable gap
-> one repair route
-> one verification command
-> one receipt-backed outcome
```

## Users and surfaces

Users:

- developers choosing what to fix next;
- reviewers reading PR evidence;
- coding-agent operators copying bounded packets;
- maintainers checking whether the actionable queue moved;
- platform owners deciding whether the advisory loop is useful enough to adopt.

Surfaces:

- badge and badge-basis reports;
- generated PR evidence and front panels;
- editor Show Status and repair actions;
- `ripr-swarm attempt --dry-run` packets;
- actionable-gap outcome and trend reports;
- docs, fixtures, output contracts, metrics, and closeout receipts.

## Success criteria

- Badge-adjacent copy names the public count as unresolved actionable static
  repair gaps, with seam-native inventory below the primary basis.
- PR evidence leads with actionable delta and one top next repair packet before
  raw finding or inventory totals.
- Editor status leads with the safe next repair action or the precise
  fail-closed reason.
- Swarm dry-run output includes a compact copy-ready operator/agent packet with
  verify, receipt, allowed-scope, and stop-condition sections.
- Outcome and trend reports lead with receipt-linked movement instead of queue
  size alone.
- All covered surfaces preserve advisory/static boundaries and avoid runtime,
  coverage, mutation, policy, gate, or merge-readiness claims.

## Proposed shape

Use `RIPR-SPEC-0059` as the cross-surface behavior contract. Each behavior PR
touches one surface, keeps typed fields as the action source, adds or updates
the surface's focused proof, and leaves raw counts and diagnostics as supporting
context below the repair-first section.

The campaign starts with source-of-truth activation only, then proceeds through
badge, PR evidence, editor, swarm dry-run, outcome/trend, and closeout slices.

## Alternatives considered

| Alternative | Why we are not picking it |
| --- | --- |
| Let every surface define its own copy. | This has already produced too much interpretation burden and inconsistent first-screen questions. |
| Start with a shared renderer abstraction. | The first risk is product translation, not code duplication. Shared code can follow once at least two surfaces prove the same contract. |
| Promote badge or gate claims now. | Translation polish does not prove runtime adequacy, coverage adequacy, mutation confirmation, or policy eligibility. |
| Make the editor or swarm choose a new top gap. | Ranking and actionability remain upstream artifact responsibilities. Covered surfaces translate existing typed evidence. |

## Behavior specs to create or update

- `RIPR-SPEC-0059`: Actionable surface translation.

## Architecture decisions needed

No ADR is needed for this activation. Add one only if a later PR changes
surface authority, centralizes a new presentation owner, or grants a surface a
decision role it does not have today.

## Implementation campaign shape

Each slice follows the [scoped PR contract](../SCOPED_PR_CONTRACT.md).

1. `docs/actionable-surface-translation-stack`
2. `badge/actionable-basis-presentation`
3. `pr/actionable-delta-front-panel`
4. `editor/repair-first-status-hierarchy`
5. `swarm/dry-run-copy-ready-packet`
6. `outcome/movement-front-section`
7. `campaign/actionable-surface-translation-closeout`

## Evidence plan

- Source-of-truth activation passes document artifact, goals, doc index,
  traceability, support-tier, and PR-shape checks.
- Badge work updates badge-basis proof and badge wording fixtures or snapshots.
- PR evidence work updates PR summary/front-panel fixtures and output contracts.
- Editor work updates LSP or VS Code fixture/smoke proof for first-screen
  hierarchy without changing analyzer truth.
- Swarm work updates dry-run packet fixture proof and human workflow docs.
- Outcome work updates actionable-gap outcome/trend fixture proof.
- Closeout records proof commands, claim boundary, remaining limits, and next
  recommended work.

## Risks

- Surfaces may lead with raw counts again. Mitigation: each surface PR must
  prove first-screen ordering.
- Presentation polish may imply stronger authority. Mitigation: repeat
  advisory/static non-claims and keep gate authority with explicit gate
  artifacts.
- Copy-ready packets may look like autonomous repair. Mitigation: dry-run and
  editor packets must state that RIPR does not edit source, generate tests, call
  providers, run mutation, or claim movement without receipts.
- A shared vocabulary could hide preview limits. Mitigation: preview and static
  limits remain visible before action language where relevant.

## Non-goals

- No analyzer truth changes.
- No actionable-gap producer or schema changes.
- No output-schema change in the activation PR.
- No public badge semantic change in the activation PR.
- No default CI blocking or gate behavior changes.
- No PR comment publishing changes.
- No source edits or generated tests.
- No provider or model calls.
- No mutation execution.
- No release, publish, signing, or marketplace work.
- No runtime adequacy, coverage adequacy, proof-of-correctness, policy
  eligibility, gate pass/fail, or merge-readiness claim.

## Exit criteria

This proposal can move to `accepted` when:

- the implementation plan is done and closeout proof exists;
- badge, PR, editor, swarm, and outcome/trend surfaces each lead with the
  actionable unit required by `RIPR-SPEC-0059`;
- focused fixtures, snapshots, tests, or dogfood receipts prove the touched
  surfaces;
- support-tier and public claim boundaries remain unchanged or are updated by a
  dedicated claim PR;
- remaining limits and next recommended work are recorded in the closeout.
