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
- `RIPR-SPEC-0053`: Start-here surface convergence

Linked ADRs:

- `ADR-0016`: Editor adoption assurance is read-only diagnosis
- `ADR-0015`: Start-here surfaces use canonical gap records
- `ADR-0014`: Editor first-pr projection is read-only
- `ADR-0013`: Editor setup diagnostics are read-only

Linked issues:

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

Lane 3 already built the saved-workspace editor cockpit, first-run setup
diagnosis, first repair loop, and first-pr bridge. A user can inspect a gap,
copy a bounded repair packet, verify movement, emit a receipt, refresh, and
inspect the first-pr packet.

The next problem is adoption assurance. A new user can still lose trust before
the first repair if the editor cannot clearly answer:

```text
Did the extension start?
Which server binary is active?
Is the server compatible with this extension?
Which workspace root is active?
Why are no diagnostics visible?
Is the first-pr packet current and safe to open?
Is the receipt current, stale, mismatched, or missing?
What is the one safe next action?
```

This proposal opens a narrow Lane 3 campaign to make those first-use states
explicit and fixture-backed without adding new editor furniture or changing the
evidence model.

## Users and surfaces

- New VS Code users installing or opening RIPR for the first time.
- Developers in multi-root, nested, or path-with-spaces workspaces.
- Users with stale, wrong-root, malformed, or missing artifacts.
- Preview-language users who need disabled or unavailable adapter states to be
  explicit.
- Coding agents that need a bounded first repair packet and a stop condition.
- Release operators proving that the editor path supports install-to-first-PR
  adoption without hidden automation.

## Success criteria

- `ripr: Diagnose Setup` and `ripr: Show Status` explain server compatibility,
  workspace root, config, language, artifact, first-pr packet, and receipt
  state.
- Single-root, multi-root, no-workspace, nested-workspace, wrong-root, stale,
  malformed, disabled, and unavailable states are fixture-backed or e2e-backed
  where practical.
- Repair, first-pr, and receipt actions appear only when typed fields and
  validated paths make them safe.
- Preview-language findings remain opt-in, advisory, syntax-first, and
  static-limit bounded.
- The editor points to existing start-here artifacts; it does not create them.
- Dogfood receipts record external-style adoption paths and failure states.
- Closeout maps requirements to artifacts, commands, and remaining limitations.

## Proposed shape

Open a focused Lane 3 campaign:

1. Add this source-of-truth stack and lane tracker/index/traceability updates.
2. Pin the post-first-pr-bridge editor baseline.
3. Add extension/server compatibility diagnosis.
4. Harden workspace-root and multi-root diagnosis.
5. Add adoption-assurance fixtures for success and fail-closed states.
6. Smoke the real VS Code path.
7. Write the install-to-first-pr editor guide.
8. Record external-style editor adoption receipts.
9. Close the campaign with prompt-to-artifact proof.

## Alternatives considered

| Alternative | Why we are not picking it |
| --- | --- |
| Treat the existing first-run guide as enough. | The guide is useful, but users still need live editor state for server compatibility, root selection, and artifact safety. |
| Add CodeLens, inlays, or inline repair UI. | The problem is orientation and safety, not more editor furniture. |
| Generate missing first-pr packets from the editor. | Lane 3 consumes existing artifacts. Producing first-pr packets belongs to CLI/report surfaces. |
| Auto-install or replace the server binary. | Release/platform owns install rails. This campaign diagnoses compatibility and points to safe next actions. |
| Collapse wrong-root, stale, and missing states into no diagnostics. | No-output ambiguity is the adoption failure this campaign is meant to remove. |

## Risks

- Compatibility diagnosis could become an installer. Mitigation: keep it
  read-only and show next safe commands instead of mutating binaries or config.
- Multi-root handling could accidentally expose repair actions for an ambiguous
  root. Mitigation: ambiguous roots fail closed and suppress repair packets.
- Adoption assurance could reopen analyzer or PR/CI scope. Mitigation: this
  campaign consumes typed artifacts and keeps PR/CI production outside Lane 3.
- Preview unavailable states could sound like stable support. Mitigation:
  preview status and static limits remain visible before action language.

## Non-goals

- No analyzer truth changes.
- No first-pr packet producer changes.
- No generated CI summaries or PR comments.
- No policy, gate, baseline, suppression, or default-blocking behavior.
- No source edits or generated tests.
- No provider/model calls.
- No mutation execution or runtime adequacy claims.
- No auto-install, binary download, config mutation, or hidden server
  replacement.
- No CodeLens, inlays, semantic tokens, inline patches, or unsaved-buffer
  overlays.

## Exit criteria

This proposal can move to `accepted` when the #1245-#1253 issue burn-down is
closed or explicitly superseded, VS Code e2e and `lsp-cockpit-report` prove the
adoption path, docs explain install-to-first-pr recovery states, dogfood
receipts cover external-style use, and closeout records what the editor proves
and what it does not claim.
