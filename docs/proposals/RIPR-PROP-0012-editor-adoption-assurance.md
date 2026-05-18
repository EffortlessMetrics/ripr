# RIPR-PROP-0012: Editor Adoption Assurance

Status: proposed

Owner: Lane 3 - Editor / LSP UX

Created: 2026-05-18

Target campaign: Editor Adoption Assurance

Linked specs:

- `RIPR-SPEC-0054`: Editor adoption assurance
- `RIPR-SPEC-0052`: Editor first-pr packet projection
- `RIPR-SPEC-0050`: Editor first repair loop
- `RIPR-SPEC-0049`: Editor setup status

Linked ADRs:

- `ADR-0016`: Editor adoption assurance remains read-only
- `ADR-0014`: Editor first-pr projection is read-only
- `ADR-0013`: Editor setup diagnostics are read-only

Linked work items:

- #1245 `docs(lane3): open editor adoption assurance stack`
- #1246 `test(lsp): pin editor adoption baseline`
- #1247 `vscode: add extension/server compatibility diagnosis`
- #1248 `vscode: harden workspace-root and multi-root diagnosis`
- #1249 `fixtures(editor): add adoption-assurance fixture corpus`
- #1250 `test(vscode): smoke editor adoption assurance path`
- #1251 `docs(editor): write install-to-first-pr editor guide`
- #1252 `dogfood(lane3): record external-style editor adoption receipts`
- #1253 `campaign(lane3): close editor adoption assurance`

## Problem

Lane 3 has closed the editor cockpit, preview routing, gap cockpit, first-run
repair loop, receipt projection, and first-pr bridge. The editor can now carry
a user from a diagnostic to a repair packet, verify command, receipt command,
refresh, and first-pr packet.

The remaining adoption risk is first-use trust. A new user can still lose the
thread before the first repair if the editor cannot explain:

```text
which extension and server are active
whether their protocol and artifact schemas are compatible
which workspace root is active
whether multi-root state is safe or ambiguous
why artifacts are missing, stale, wrong-root, malformed, or mismatched
whether receipt and first-pr packet state belongs to the current root and gap
what action is safe next
```

This campaign makes the existing editor cockpit adoption-grade without making
the editor a report producer, installer, source editor, policy engine, or
runtime proof system.

## Users and surfaces

- First-time VS Code users opening RIPR in a new or unfamiliar repo.
- Rust users following the stable repair loop.
- Preview-language users who need disabled, unavailable, and static-limited
  evidence to remain visibly bounded.
- Coding agents that need a narrow local work packet with stop conditions.
- Maintainers reviewing setup, root, and artifact mismatch failures.

Primary surfaces:

- `ripr: Diagnose Setup`;
- `ripr: Show Status`;
- diagnostic hover and bounded code actions;
- VS Code server resolution and workspace-root handling;
- first-pr packet and receipt state projection;
- editor fixtures, VS Code smoke, docs, and dogfood receipts.

## Success criteria

- Current Rust/default editor cockpit behavior is pinned before new behavior
  lands.
- Diagnose Setup and Show Status explain extension version, resolved server
  path, server version, expected feature/protocol support, supported artifact
  schema versions, and the next safe action for incompatible state.
- The editor always names the active workspace root when one is available.
- No-workspace, multi-root, nested-root, wrong-root, path-with-spaces, stale,
  malformed, unsupported-schema, receipt-mismatch, and first-pr-mismatch
  states fail closed.
- Repair actions appear only when root, freshness, schema, identity, paths, and
  command payloads are safe.
- Preview evidence remains opt-in, syntax-first, advisory, static-limit
  labeled, and never gate-eligible or Rust-level by implication.
- Fixtures, VS Code smoke, docs, and external-style dogfood receipts prove the
  success and fail-closed states.

## Proposed shape

Open a Lane 3 campaign named Editor Adoption Assurance. It is a narrow
extension of the closed first-run and first-pr bridge work:

1. define this source-of-truth stack and issue burn-down map;
2. pin the closed Lane 3 contract;
3. add extension/server compatibility diagnosis;
4. harden active workspace and multi-root diagnosis;
5. fixture-pin adoption success and fail-closed states;
6. smoke the real VS Code adoption path;
7. write the install-to-first-pr editor guide;
8. record external-style dogfood receipts;
9. close with proof and remaining limits.

## Alternatives considered

| Alternative | Why we are not picking it |
| --- | --- |
| Keep relying on the first-run and first-pr bridge closeouts. | Those prove the cockpit works, but they do not explicitly harden version compatibility and multi-root first-use failure states. |
| Add auto-install or binary repair in VS Code. | Installation and release are separate surfaces; Lane 3 should explain missing or incompatible binaries without mutating user systems. |
| Let each root decide independently in multi-root workspaces. | A repair packet from the wrong root is worse than no packet. Ambiguous root state must fail closed first. |
| Use diagnostics for setup failures. | Setup state is not analyzer truth; status and diagnosis should explain it without inventing code diagnostics. |
| Add CodeLens, inlays, semantic tokens, or inline patches. | The bottleneck is compatibility/root confidence, not more editor furniture. |

## Behavior specs to create or update

- Add `RIPR-SPEC-0054`: Editor adoption assurance.
- Continue to reference `RIPR-SPEC-0049`, `RIPR-SPEC-0050`, and
  `RIPR-SPEC-0052` for setup, repair, and first-pr projection behavior.

## Architecture decisions needed

- Add `ADR-0016` to record that adoption assurance remains read-only and
  projection-only.

## Implementation campaign shape

Each slice follows the [scoped PR contract](../SCOPED_PR_CONTRACT.md).

1. `docs/lane3-editor-adoption-assurance-stack`
2. `test/lsp-editor-adoption-baseline`
3. `vscode/extension-server-compatibility-diagnosis`
4. `vscode/workspace-root-multi-root-diagnosis`
5. `fixtures/editor-adoption-assurance`
6. `test/vscode-editor-adoption-assurance`
7. `docs/editor-install-to-first-pr`
8. `dogfood/lane3-editor-adoption-receipts`
9. `campaign/lane3-editor-adoption-assurance-closeout`

## Evidence plan

- Source-of-truth docs identify the proposal, spec, ADR, plan, lane tracker
  state, issue burn-down, and traceability entry.
- LSP tests pin the closed baseline and fail-closed action gating.
- VS Code tests cover server compatibility, workspace root, multi-root, and
  artifact mismatch state.
- Fixtures pin setup ok, server missing, version mismatch, no workspace,
  multi-root, wrong-root artifact, stale receipt, first-pr packet ready,
  first-pr packet mismatch, and preview adapter unavailable.
- User docs explain install/open/status/diagnostic/repair/verify/receipt/
  refresh/first-pr as one path with non-claims.
- Dogfood receipts record external-style repo states.

## Risks

- Compatibility diagnosis could become an installer. Mitigation: ADR-0016 keeps
  it explanatory and read-only.
- Multi-root handling could surface the wrong repair packet. Mitigation:
  ambiguous roots fail closed until a safe active root is selected.
- Status text could imply PR readiness or gate authority. Mitigation:
  first-pr packet and receipt state stay advisory unless an explicit gate
  artifact owns authority.
- Preview evidence could look mature. Mitigation: preview status and static
  limits stay visible before action language.

## Non-goals

- No analyzer behavior changes.
- No policy, gate, default-blocking, badge, baseline, waiver, or suppression
  changes.
- No PR comment publishing or generated CI summary composition.
- No release publishing, binary installation, binary download, or config
  mutation.
- No source edits, inline patches, or automatic repair application.
- No generated tests.
- No provider or model calls.
- No runtime mutation execution.
- No runtime adequacy, mutation proof, policy eligibility, or merge-readiness
  claim.
- No CodeLens, inlay hints, semantic tokens, inline patches, or
  unsaved-buffer overlays.
- No preview-language promotion to Rust-level confidence.

## Exit criteria

This proposal can move to `accepted` when:

- the closed Lane 3 contract remains pinned;
- compatibility diagnosis is implemented and tested;
- workspace-root and multi-root diagnosis has fixture and e2e evidence;
- adoption fixtures cover success and fail-closed states;
- VS Code e2e proves the install-to-first-pr path;
- docs explain the workflow and recovery states;
- external-style dogfood receipts prove the path;
- no analyzer, policy, PR/CI, release, source-edit, generated-test, provider,
  mutation, gate, or UI-sprawl scope landed.
