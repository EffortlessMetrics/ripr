# Actionable Surface Translation Implementation Plan

Status: active

Owner: cross-surface / repo-infra

Created: 2026-05-23

Plan ID: RIPR-PLAN-0059

Linked proposal:

- [RIPR-PROP-0016: Actionable Surface Translation](../../docs/proposals/RIPR-PROP-0016-actionable-surface-translation.md)

Linked specs:

- [RIPR-SPEC-0059: Actionable Surface Translation](../../docs/specs/RIPR-SPEC-0059-actionable-surface-translation.md)

Linked ADRs:

- None.

Active goal:

- [`.ripr/goals/active.toml`](../../.ripr/goals/active.toml)

Support-tier impact:

- None for the activation slice. Later behavior PRs must state whether a public
  claim changes and keep existing advisory/static boundaries unless a dedicated
  support-tier PR changes them.

Policy impact:

- Register this plan, its proposal, and its spec in
  [`policy/doc-artifacts.toml`](../../policy/doc-artifacts.toml).

Required evidence:

- `cargo xtask check-doc-artifacts`
- `cargo xtask check-goals`
- `cargo xtask goals next`
- `cargo xtask pr-body --work-item docs/actionable-surface-translation-stack`
- `cargo xtask repo-contract-report`
- `cargo xtask check-doc-index`
- `cargo xtask markdown-links`
- `cargo xtask check-static-language`
- `cargo xtask check-doc-roles`
- `cargo xtask check-traceability`
- `cargo xtask check-capabilities`
- `cargo xtask check-support-tiers`
- `cargo xtask check-pr`
- `git diff --check`

Non-goals:

- No analyzer behavior changes.
- No actionable-gap producer or schema changes.
- No output-schema change in the activation slice.
- No default CI blocking or gate behavior changes.
- No PR comment publishing changes.
- No release, publish, signing, or marketplace work.
- No source edits or generated tests.
- No provider or model calls.
- No mutation execution.
- No runtime, coverage, mutation, policy, gate, or merge-readiness claim.

Claim boundary:

- This plan makes the actionable surface translation queue discoverable and
  checkable. It does not prove surface behavior until the relevant behavior PRs
  land with focused fixtures, snapshots, tests, receipts, or output contracts.

Rollback:

- Revert this plan, its proposal, active-goal wiring, ledger entries, indexes,
  and traceability outputs. No runtime behavior changes are involved.

## Current state

`RIPR-SPEC-0059` already defines the cross-surface translation contract for
badge, PR evidence, editor status, swarm dry-run, and outcome/trend surfaces.
The missing piece is execution wiring: proposal rationale, implementation plan,
active manifest, artifact ledger entries, and campaign map entries.

This plan opens that execution rail without changing behavior. The first
behavior-bearing slice should be the badge presentation pass because the public
badge headline is the highest-risk place for users to misunderstand the
actionable count.

## Work items

### Work item: docs/actionable-surface-translation-stack

Status: done

Linked proposal:

- RIPR-PROP-0016

Linked spec:

- RIPR-SPEC-0059

Linked ADR:

- n/a

Blocks:

- `badge/actionable-basis-presentation`

Blocked by:

- n/a

Branch:

- `docs-activate-actionable-surface-translation`

Issue:

- n/a

PR:

- #309

#### Goal

Open the Actionable Surface Translation campaign as the next active
source-of-truth rail without changing any user-facing behavior.

#### Production delta

- Add `RIPR-PROP-0016`.
- Add this implementation plan.
- Register proposal, spec, and plan in `policy/doc-artifacts.toml`.
- Link `RIPR-SPEC-0059` to its proposal and plan.
- Update campaign, implementation-plan, plan-index, proposal-index,
  traceability, and active-goal surfaces.

#### Non-goals

- No badge, PR, editor, swarm, outcome, analyzer, schema, gate, CI, release,
  source-edit, generated-test, provider, or mutation behavior changes.

#### Acceptance

- The active goal selects `actionable-surface-translation`.
- This source-of-truth work item is done and the first behavior work item is
  ready.
- The proposal, spec, and plan resolve through the document artifact ledger.
- `RIPR-SPEC-0059` remains behavior-proposed and does not claim implementation
  proof before the surface PRs land.

#### Proof commands

```bash
cargo xtask check-doc-artifacts
cargo xtask check-goals
cargo xtask goals next
cargo xtask pr-body --work-item docs/actionable-surface-translation-stack
cargo xtask repo-contract-report
cargo xtask check-doc-index
cargo xtask markdown-links
cargo xtask check-static-language
cargo xtask check-doc-roles
cargo xtask check-traceability
cargo xtask check-capabilities
cargo xtask check-support-tiers
cargo xtask check-pr
git diff --check
```

#### Support-tier impact

- None.

#### Policy impact

- `policy/doc-artifacts.toml` gains proposal, spec, and plan registrations for
  this campaign.

#### Claim boundary

- This slice proves only source-of-truth activation and next-work selection.
  It does not prove surface translation behavior.

#### Rollback

- Revert the proposal, plan, active-goal, ledger, index, campaign, and
  traceability edits.

### Work item: badge/actionable-basis-presentation

Status: done

Linked proposal:

- RIPR-PROP-0016

Linked spec:

- RIPR-SPEC-0059

Linked ADR:

- n/a

Blocks:

- `pr/actionable-delta-front-panel`

Blocked by:

- `docs/actionable-surface-translation-stack`

Branch:

- `badge-actionable-basis-presentation`

Issue:

- n/a

PR:

- #314

#### Goal

Make badge-adjacent public copy and badge-basis output explain the headline
count as unresolved actionable static repair gaps using
`canonical_actionable_gap` as the public basis.

#### Production delta

- Update only badge/badge-basis presentation and focused proof needed for the
  first-screen meaning.
- Keep seam-native inventory and raw findings labeled as supporting or internal
  diagnostics.

#### Non-goals

- No badge endpoint refresh unless explicitly scoped.
- No count-basis change away from `canonical_actionable_gap`.
- No support-tier promotion or gate/default-blocking change.

#### Acceptance

- Public badge/badge-basis wording names unresolved actionable static repair
  gaps at point of use.
- `ripr+` copy explains additional items only when they project into the same
  repair/verify/receipt model.
- Existing badge semantics and advisory boundaries remain intact.

#### Proof commands

```bash
cargo xtask badge-basis
cargo xtask check-badge-diff-policy
cargo xtask check-output-contracts
cargo xtask check-static-language
cargo xtask check-pr
git diff --check
```

#### Rollback

- Revert the badge/badge-basis presentation and focused proof edits.

### Work item: pr/actionable-delta-front-panel

Status: done

Linked proposal:

- RIPR-PROP-0016

Linked spec:

- RIPR-SPEC-0059

Linked ADR:

- n/a

Blocks:

- `editor/repair-first-status-hierarchy`

Blocked by:

- n/a

Branch:

- `pr-actionable-delta-front-panel`

Issue:

- n/a

PR:

- this PR

#### Goal

Make PR evidence front panels lead with actionable delta and one top next
repair packet before raw finding totals or seam inventory.

#### Production delta

- Update PR evidence/front-panel presentation and fixtures for first-screen
  actionable delta.

#### Non-goals

- No PR comment publishing.
- No default gate or CI blocking changes.
- No analyzer truth changes.

#### Acceptance

- A reviewer can identify repo actionable count, PR-local actionable count,
  new/resolved gaps, receipt state, blocked/static-limited counts, and one top
  next repair packet from the front panel.

#### Proof commands

```bash
cargo xtask pr-summary
cargo xtask check-output-contracts
cargo xtask check-pr
git diff --check
```

#### Rollback

- Revert PR/front-panel presentation and fixture edits.

### Work item: editor/repair-first-status-hierarchy

Status: done

Linked proposal:

- RIPR-PROP-0016

Linked spec:

- RIPR-SPEC-0059

Linked ADR:

- n/a

Blocks:

- `swarm/dry-run-copy-ready-packet`

Blocked by:

- n/a

Branch:

- `editor-repair-first-status-hierarchy`

Issue:

- n/a

PR:

- #316

#### Goal

Make editor Show Status lead with immediate repair clarity: one safe action or
one precise fail-closed reason.

#### Production delta

- Update editor status ordering and focused fixture/smoke proof for the
  actionable queue first screen.

#### Non-goals

- No new editor furniture such as CodeLens, inlays, semantic tokens, inline
  patches, or unsaved-buffer overlays.
- No analyzer, PR/CI, policy, gate, provider, source-edit, generated-test, or
  mutation behavior.

#### Acceptance

- Show Status names workspace/current-file actionable state, top repair item,
  related test or target, verify command, receipt state, and next safe command
  when fail-closed.

#### Proof commands

```bash
cargo test -p ripr lsp --lib
npm --prefix editors/vscode run compile
npm --prefix editors/vscode run test:e2e
cargo xtask lsp-cockpit-report
cargo xtask check-output-contracts
cargo xtask check-pr
git diff --check
```

#### Rollback

- Revert editor status presentation and fixture/smoke edits.

### Work item: swarm/dry-run-copy-ready-packet

Status: ready

Linked proposal:

- RIPR-PROP-0016

Linked spec:

- RIPR-SPEC-0059

Linked ADR:

- n/a

Blocks:

- `outcome/movement-front-section`

Blocked by:

- n/a

Branch:

- `swarm-dry-run-copy-ready-packet`

Issue:

- n/a

PR:

- n/a

#### Goal

Make `ripr-swarm attempt --dry-run` emit a compact copy-ready operator/agent
packet section.

#### Production delta

- Update dry-run formatting and focused fixtures for task, allowed files,
  do-not-change boundaries, repair target, verify command, receipt command,
  stop conditions, and required return format.

#### Non-goals

- No provider calls.
- No source edits.
- No generated tests.
- No mutation execution.
- No claim that dry-run verifies movement.

#### Acceptance

- A human or external agent operator can copy the packet without reconstructing
  missing scope, verify, receipt, or stop-condition boundaries.

#### Proof commands

```bash
cargo test -p xtask ripr_swarm_attempt --bin xtask
cargo xtask check-output-contracts
cargo xtask check-pr
git diff --check
```

#### Rollback

- Revert dry-run formatting and fixture edits.

### Work item: outcome/movement-front-section

Status: blocked

Linked proposal:

- RIPR-PROP-0016

Linked spec:

- RIPR-SPEC-0059

Linked ADR:

- n/a

Blocks:

- `campaign/actionable-surface-translation-closeout`

Blocked by:

- `swarm/dry-run-copy-ready-packet`

Branch:

- `outcome-movement-front-section`

Issue:

- n/a

PR:

- n/a

#### Goal

Make outcome/trend reports lead with receipt-linked movement since the previous
refresh.

#### Production delta

- Update outcome/trend front sections and fixtures for total actionable count,
  delta, resolved, improved, unchanged after attempt, orphaned/missing
  receipts, and top blocked reason.

#### Non-goals

- No runtime adequacy or mutation proof claim.
- No gate or merge-readiness claim.

#### Acceptance

- Movement reads as receipt-linked delta, not just static queue size.

#### Proof commands

```bash
cargo xtask actionable-gap-outcomes
cargo xtask check-output-contracts
cargo xtask check-pr
git diff --check
```

#### Rollback

- Revert outcome/trend presentation and fixture edits.

### Work item: campaign/actionable-surface-translation-closeout

Status: blocked

Linked proposal:

- RIPR-PROP-0016

Linked spec:

- RIPR-SPEC-0059

Linked ADR:

- n/a

Blocks:

- n/a

Blocked by:

- `outcome/movement-front-section`

Branch:

- `campaign-actionable-surface-translation-closeout`

Issue:

- n/a

PR:

- n/a

#### Goal

Close the campaign only after each covered surface proves the first-screen
translation required by `RIPR-SPEC-0059`.

#### Production delta

- Add a closeout handoff and update statuses only after proof exists.

#### Non-goals

- No new behavior in the closeout PR.
- No support-tier promotion unless a dedicated claim PR has already landed.

#### Acceptance

- Closeout maps each requirement to artifact, validation command, status,
  remaining limits, and next recommended work.

#### Proof commands

```bash
cargo xtask check-doc-artifacts
cargo xtask check-goals
cargo xtask check-doc-index
cargo xtask markdown-links
cargo xtask check-static-language
cargo xtask check-doc-roles
cargo xtask check-traceability
cargo xtask check-capabilities
cargo xtask check-support-tiers
cargo xtask check-pr
git diff --check
```

#### Rollback

- Revert closeout and status updates. Leave already-landed behavior slices in
  place unless the closeout PR changed them.

## Closeout criteria

- Proposal status moves to accepted only after behavior proof lands.
- `RIPR-SPEC-0059` remains linked to focused tests, fixtures, output contracts,
  and docs for all covered surfaces.
- The active manifest is closed with either a successor or
  `no_current_goal = true`.
- The closeout records what users may believe and what remains advisory,
  preview-limited, static-only, or unsupported.
