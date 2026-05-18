# RIPR-PROP-0010: Editor First-PR Bridge

Status: accepted

Owner: Lane 3 - Editor / LSP UX

Created: 2026-05-17

Target campaign: Editor First-PR Bridge

Linked specs:

- `RIPR-SPEC-0052`: Editor first-pr packet projection
- `RIPR-SPEC-0051`: First successful PR UX
- `RIPR-SPEC-0050`: Editor first repair loop
- `RIPR-SPEC-0049`: Editor setup status

Linked ADRs:

- `ADR-0014`: Editor first-pr projection is read-only
- `ADR-0013`: Editor setup diagnostics are read-only
- `ADR-0012`: Editor gap projection is read-only

Linked work items:

- `docs(lane3): open editor first-pr bridge source-of-truth stack`
- `test(lsp): pin post-first-run editor contract`
- `lsp(first-pr): validate first-pr packet artifacts`
- `lsp(first-pr): project first-pr state in Show Status`
- `lsp(first-pr): add bounded first-pr packet actions`
- `fixtures(editor): add first-pr bridge fixtures`
- `test(vscode): smoke first-pr bridge`
- `docs(editor): document first successful PR bridge`
- `dogfood(lane3): record first-pr bridge receipts`
- `campaign(lane3): close editor first-pr bridge`

## Problem

The Editor Gap Cockpit and Editor First-Run and Repair Usability campaigns are
closed. A user can diagnose setup, inspect a gap, copy a bounded first repair
packet, verify movement, emit a receipt, and refresh editor state. Separately,
`cargo xtask first-pr` writes a first successful PR start-here packet from
explicit artifacts.

The remaining Lane 3 gap is continuity. The editor can guide a local repair,
but it does not yet make the next PR-facing artifact obvious:

```text
diagnose setup
-> inspect one diagnostic
-> repair
-> verify
-> receipt
-> refresh
-> inspect the first-pr packet
```

A low-context user or coding agent should not need to understand RIPR's
internal artifact graph to know whether a first-pr packet exists, whether it is
fresh and root-matched, where it lives, which gap it describes, and what action
is safe next.

## Users and surfaces

- First-time VS Code users moving from one local repair toward first PR review.
- Rust users using the stable saved-workspace editor cockpit.
- Preview-language users who still need syntax-first, static-limit, advisory
  boundaries to remain visible.
- Coding agents that need one bounded gap, verify command, receipt command,
  first-pr packet path, and stop condition.
- Lane 4 and CI surfaces that own PR summaries and comments but benefit when
  the editor points to the same first-pr packet.

Primary Lane 3 surfaces:

- `ripr: Diagnose Setup`;
- `ripr: Show Status`;
- diagnostic hover;
- bounded code actions;
- `lsp-cockpit-report`;
- VS Code e2e smoke;
- editor first-pr bridge fixtures and docs.

## Success criteria

- The editor validates existing `target/ripr/first-pr/start-here.{json,md}`
  and `target/ripr/reports/start-here.{json,md}` artifacts read-only.
- `ripr: Diagnose Setup` and `ripr: Show Status` can explain first-pr packet
  state: missing, found, stale, wrong-root, malformed, no-action, and top
  repairable gap available.
- First-pr packet actions appear only when the packet is validated, current,
  workspace-local, command-safe, and gap-matched to the current diagnostic
  when a diagnostic is involved.
- Stale, wrong-root, malformed, missing, unsupported, disabled, unavailable,
  path-unsafe, or command-unsafe states fail closed and suppress repair
  actions except refresh, setup diagnosis, or first-pr regeneration guidance.
- The editor points to first-pr readiness without publishing PR comments,
  composing generated CI summaries, deciding gates, or claiming merge
  readiness.
- Preview findings remain opt-in, advisory, syntax-first, visibly labeled, and
  static-limit bounded.
- The slice ships with fixture coverage, LSP tests, VS Code smoke, docs,
  dogfood receipts, and closeout proof before it is marked accepted.

## Proposed shape

Open a Lane 3 campaign named Editor First-PR Bridge. It extends the existing
first-run and first-repair cockpit by projecting validated first-pr packet
state in the editor:

1. define this source-of-truth stack;
2. pin the post-first-run editor contract;
3. validate first-pr packet artifacts without rendering them;
4. project first-pr packet state in status and setup diagnosis;
5. expose bounded open/copy actions when typed fields are safe;
6. fixture-pin success and fail-closed states;
7. smoke the real VS Code path;
8. document the flow from local repair to first-pr packet;
9. record dogfood receipts;
10. close the campaign with proof.

Lane 3 consumes existing artifacts. It does not create analyzer truth, first-pr
packets, PR comments, CI summaries, policy decisions, source edits, generated
tests, provider calls, mutation runs, or gates.

## Alternatives considered

| Alternative | Why we are not picking it |
| --- | --- |
| Leave first-pr packet discovery to docs and terminal commands. | The editor is the local repair surface; users need to know the PR handoff artifact exists before leaving the cockpit. |
| Generate the first-pr packet from the editor. | That would make Lane 3 a report producer and blur ownership with Lane 4 / xtask surfaces. |
| Publish PR comments or generated CI summaries from VS Code. | PR/CI publishing is outside Lane 3 and has different safety boundaries. |
| Treat any `start-here.md` path as safe to open. | The editor must validate root, freshness, schema, path safety, and command payloads before stronger actions appear. |
| Add CodeLens, inlays, semantic tokens, or inline patches. | The next bottleneck is continuity over existing artifacts, not more editor furniture. |

## Behavior specs to create or update

- Add `RIPR-SPEC-0052`: Editor first-pr packet projection.
- Continue to reference `RIPR-SPEC-0051` for the first successful PR packet
  contract.
- Continue to reference `RIPR-SPEC-0050` and `RIPR-SPEC-0049` for existing
  editor first-repair and setup status behavior.

## Architecture decisions needed

- Add `ADR-0014` to record that editor first-pr projection is read-only and
  does not make the editor a PR/CI producer.

## Implementation campaign shape

Each slice follows the [scoped PR contract](../SCOPED_PR_CONTRACT.md).

1. `docs/lane3-editor-first-pr-bridge-stack`
2. `test/lsp-post-first-run-editor-contract`
3. `lsp/first-pr-packet-validation`
4. `lsp/first-pr-state-status`
5. `lsp/first-pr-bounded-actions`
6. `fixtures/editor_first_pr_bridge`
7. `test/vscode-first-pr-bridge`
8. `docs/editor-first-pr-bridge`
9. `dogfood/lane3-first-pr-bridge-receipts`
10. `campaign/lane3-editor-first-pr-bridge-closeout`

## Evidence plan

- Source-of-truth docs identify proposal, spec, ADR, plan, lane tracker state,
  and traceability entry.
- LSP tests validate first-pr packet parsing, root matching, freshness,
  diagnostic/gap identity, path safety, command safety, and fail-closed states.
- VS Code e2e tests validate setup/status reporting and bounded actions against
  the packaged extension path.
- Fixtures pin packet missing, found repairable, no-action, stale, wrong-root,
  malformed, receipt-improved, and receipt-unchanged states.
- Docs explain what the first-pr packet means, where it lives, how to
  regenerate it, and what it does not claim.
- Dogfood receipts record the bridge over real artifacts without adding
  behavior.

## Risks

- The bridge could make the editor feel like a PR readiness authority.
  Mitigation: status explains packet state and advisory boundaries; it does not
  decide merge readiness or gates.
- The editor could parse Markdown prose for semantics. Mitigation: prefer JSON
  typed fields and treat Markdown as an openable human artifact only.
- Packet actions could escape the workspace or run unsafe commands.
  Mitigation: validate paths and command payloads before showing actions.
- Preview evidence could look mature when carried into first-pr context.
  Mitigation: language status and static limits stay visible before action
  language.
- The campaign could drift into generated CI, PR comments, CodeLens, inlays,
  source edits, providers, or mutation. Mitigation: ADR-0014 and the spec keep
  the slice read-only and projection-only.

## Non-goals

- No analyzer behavior changes.
- No first-pr packet producer changes.
- No generated CI summary composition.
- No PR comment publishing.
- No policy, gate, default-blocking, badge, baseline, waiver, or suppression
  changes.
- No source edits, inline patches, or automatic repair application.
- No generated tests.
- No provider or model calls.
- No runtime mutation execution.
- No runtime adequacy, mutation proof, or merge-readiness claim.
- No CodeLens, inlay hints, semantic tokens, or unsaved-buffer overlays.
- No preview-language promotion to Rust-level confidence.

## Exit criteria

This proposal can move to `accepted` when:

- first-pr packet validation exists and fails closed;
- Diagnose Setup and Show Status project first-pr packet state;
- bounded first-pr packet actions exist and are absent for unsafe states;
- fixtures cover success, no-action, and fail-closed states;
- VS Code e2e proves the real path;
- docs explain the flow from editor repair to first-pr packet;
- dogfood receipts record proof;
- no analyzer, policy, PR/CI producer, source-edit, generated-test, provider,
  mutation, gate, or UI-sprawl scope landed.
