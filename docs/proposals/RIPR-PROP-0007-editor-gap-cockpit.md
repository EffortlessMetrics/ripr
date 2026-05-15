# RIPR-PROP-0007: Editor Gap Cockpit

Status: accepted

Owner: Lane 3 - Editor / LSP UX

Created: 2026-05-14

Target campaign: Editor gap cockpit

Linked specs:

- `RIPR-SPEC-0047`: Editor gap projection
- `RIPR-SPEC-0046`: Gap decision ledger
- `RIPR-SPEC-0020`: First useful action report
- `RIPR-SPEC-0021`: Evidence record
- `RIPR-SPEC-0036`: Editor preview routing
- `RIPR-SPEC-0037`: Editor preview static-limit projection

Linked ADRs:

- `ADR-0012`: Editor gap projection is read-only

Linked work items:

- To be recorded when the editor gap cockpit campaign manifest opens.

## Problem

RIPR now has richer evidence records, gap records, first-useful-action reports,
repair routes, receipts, and preview-language static limits. Those artifacts
are useful only when a developer can turn them into one safe local action while
the code is open.

The editor already projects the saved-workspace evidence loop for Rust and
opt-in preview languages. The next Lane 3 problem is not more language routing
or more editor furniture. It is making the editor consume the same gap identity
and repair-route artifacts that reports, agents, and receipts use:

```text
diagnostic
-> hover evidence
-> gap state / preview limit / repair route
-> related test or repair packet
-> one focused test
-> verify
-> receipt
-> refresh
```

Without a cockpit contract, editor surfaces can drift back to raw diagnostic
text, infer actionability from prose, or over-trust preview evidence. That
would make local repairs noisier and make LLM handoffs less bounded.

## Users and surfaces

- Developers using VS Code to repair one test-intent gap while a PR is moving.
- Rust users who need the existing stable saved-workspace cockpit to stay
  unchanged by default.
- TypeScript, JavaScript, and Python preview users who need advisory evidence
  to remain visibly bounded by preview status and static limits.
- Coding agents that need a narrow, evidence-backed repair packet and receipt
  instruction.
- Maintainers who need Lane 3 to consume, not invent, analyzer facts, policy
  state, PR comments, or generated CI summaries.

Primary surfaces:

- LSP diagnostics and `diagnostic.data` identity;
- hover evidence;
- `ripr: Show Status`;
- code actions for related tests, repair packets, verify commands, receipt
  commands, static-limit notes, and refresh;
- editor gap workflow fixtures;
- live VS Code smoke tests;
- user-facing editor workflow docs.

## Success criteria

- Editor surfaces resolve through one identity path:
  `diagnostic.data -> canonical_gap_id / seam_id / finding_id -> evidence_record
  or gap_record -> repair route -> verify command -> receipt`.
- Rust diagnostics, hover, actions, status, stale handling, wrong-root handling,
  and saved-workspace defaults remain unchanged except for additive safe
  projection when matching gap artifacts exist.
- Preview findings continue to show language, `language_status = "preview"`,
  static limits, syntax-first/advisory boundaries, and no runtime adequacy
  claim.
- Static limits appear before suggested action language.
- `ripr: Show Status` explains whether a gap is actionable, already observed,
  preview-limited, stale, disabled, unavailable, wrong-root, or no-action.
- Code actions are emitted only when the current artifact has enough typed
  evidence for a bounded action.
- Wrong-root, stale, malformed, disabled-language, unavailable-language,
  unsupported-schema, missing-identity, out-of-workspace-path, and malformed
  command states fail closed.
- The editor consumes existing artifacts and does not create analyzer facts,
  policy decisions, gate decisions, PR comments, generated tests, provider
  calls, mutation runs, or source edits.
- Fixtures, `lsp-cockpit-report`, and live VS Code e2e prove the path before
  closeout.

## Proposed shape

Open a Lane 3 campaign that makes the editor a read-only gap projection
consumer. The campaign should add validation and projection in stages:

1. pin the post-Campaign 27 editor contract;
2. validate gap-oriented artifacts without rendering them;
3. project gap state in `ripr: Show Status`;
4. enrich hover with the repair route;
5. expose bounded repair packet actions;
6. fixture-pin Rust, preview, disabled, wrong-root, stale, and no-action cases;
7. prove the real VS Code path;
8. document the workflow and close with receipts.

The editor should prefer typed fields from gap records, evidence records,
first-useful-action reports, repair cards, receipts, and preview metadata.
It should render stable text as evidence when no structured field exists, but
must not parse prose to decide action semantics.

## Alternatives considered

| Alternative | Why we are not picking it |
| --- | --- |
| Keep the existing editor loop unchanged. | The current cockpit is useful, but richer gap records and repair routes would remain less actionable at the point of coding. |
| Let each editor surface read raw finding text independently. | This recreates projection drift and encourages actionability inference from prose. |
| Generate repair tests from the editor. | RIPR supplies test intent and bounded packets; generated tests and source edits are out of Lane 3 scope. |
| Run fresh analysis from VS Code before showing actions. | The cockpit is saved-workspace first and must not become a hidden analyzer path. |
| Publish PR comments from the editor. | PR comment composition and publishing belong to PR/CI lanes, not Lane 3. |
| Add CodeLens, inlays, semantic tokens, or unsaved-buffer overlays now. | Those are separate editor campaigns with different failure models and fixtures. |
| Treat preview gap packets as Rust parity. | Preview evidence is syntax-first, advisory, opt-in, and static-limit bounded. |

## Behavior specs to create or update

- Add `RIPR-SPEC-0047`: Editor gap projection.
- Reference `RIPR-SPEC-0046` as the gap decision source.
- Reference `RIPR-SPEC-0020` for first-useful-action report inputs.
- Reference `RIPR-SPEC-0036` and `RIPR-SPEC-0037` for preview routing and
  static-limit boundaries.

## Architecture decisions needed

- Add `ADR-0012` to record that editor gap projection is read-only,
  saved-workspace first, and projection-only.

## Implementation campaign shape

Each slice follows the [scoped PR contract](../SCOPED_PR_CONTRACT.md).

1. `docs/lane3-editor-gap-cockpit-stack`
2. `test/lsp-post-campaign-editor-contract`
3. `lsp/gap-artifact-validation`
4. `lsp/gap-status-projection`
5. `lsp/gap-hover-repair-route`
6. `lsp/gap-bounded-repair-actions`
7. `fixtures/editor-gap-cockpit-workflows`
8. `test/vscode-editor-gap-cockpit`
9. `docs/editor-gap-cockpit-workflow`
10. `dogfood/lane3-editor-gap-cockpit-receipts`
11. `campaign/lane3-editor-gap-cockpit-closeout`

## Evidence plan

- Source-of-truth docs identify why, behavior contract, architecture decision,
  sequencing, proof commands, and hard boundaries.
- LSP tests pin Rust default behavior, preview metadata, disabled-language
  status, stale status, wrong-root fail-closed behavior, related-test path
  safety, and static-limit hover ordering.
- Artifact validation tests prove unsupported schema, wrong-root, stale,
  malformed, disabled-language, missing-identity, out-of-workspace path, and
  malformed command states fail closed.
- Editor fixtures pin actionable Rust, preview static-limit, disabled-language,
  wrong-root, stale-artifact, and no-actionable-gap workflows.
- VS Code e2e proves activation, server resolution, hover, status, code action
  bounds, related-test opening, stale handling, and wrong-root handling.
- `lsp-cockpit-report` records the contract for diagnostics, hover, actions,
  status, packet payloads, static limits, and receipts.
- User docs explain stable Rust, opt-in preview evidence, no-output states,
  stale evidence, disabled/unavailable languages, and the read-only boundary.

## Risks

- Editor projection could drift from the gap ledger. Mitigation: prefer typed
  gap identity and repair-route fields, and fail closed on missing identity.
- Preview evidence could look mature. Mitigation: preview status and static
  limits render before action language.
- Actions could become unsafe for stale or wrong-root artifacts. Mitigation:
  suppress all actions except refresh when freshness or root checks fail.
- Related-test opening could escape the workspace. Mitigation: require
  workspace-local, current-language paths.
- The editor could become a policy or PR-comment surface. Mitigation: ADR-0012
  keeps policy, gates, PR comments, and generated CI outside Lane 3.
- Agent packets could become over-broad. Mitigation: packets carry one gap,
  one repair route, explicit limits, verify command, receipt command, and a
  stop condition.

## Non-goals

- No analyzer behavior changes.
- No evidence schema invention in Lane 3.
- No policy, gate, default-blocking, badge, baseline, or waiver changes.
- No PR comment publishing or generated CI summary composition.
- No generated tests.
- No source edits or inline patches.
- No provider or model calls.
- No runtime mutation execution.
- No CodeLens, inlay hints, semantic tokens, or unsaved-buffer overlays.
- No preview-language promotion to Rust-level confidence.

## Exit criteria

This proposal can move to `accepted` when:

- post-Campaign 27 editor behavior is pinned;
- gap artifacts are validated read-only;
- `ripr: Show Status` projects gap state and safe next action;
- hover projects preview/static-limit boundaries, gap state, repair route,
  verify command, and receipt command;
- code actions remain bounded and fail closed;
- fixtures cover Rust, preview, disabled, wrong-root, stale, and no-action
  cases;
- live VS Code e2e and `lsp-cockpit-report` prove the path;
- docs explain the workflow and no-output states;
- dogfood receipts record the full loop;
- no analyzer, policy, PR-comment, source-edit, generated-test, provider,
  mutation, gate, or UI-sprawl work landed as part of the campaign.
