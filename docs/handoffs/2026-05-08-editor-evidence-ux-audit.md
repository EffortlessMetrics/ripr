# Handoff: Editor Evidence UX Audit

Date: 2026-05-08
Branch / PR: `docs-editor-evidence-ux-audit` / #589
Latest merged campaign context: #590 `spec: define baseline debt delta report`
keeps Campaign 17, RIPR Zero Adoption, active while Editor Evidence UX remains
queued.

## Queued Work Item

`campaign/editor-evidence-ux-audit`

The audit converts the queued Lane 3 idea into a single editor evidence
contract. It does not change analyzer behavior, LSP behavior, VS Code behavior,
generated CI, policy gates, output schemas, or public API.

The target loop is:

```text
diagnostic -> evidence hover -> related test -> context packet
-> one focused test -> verify -> receipt -> refresh
```

## Prompt-To-Artifact Checklist

| Requirement | Evidence | Status |
| --- | --- | --- |
| Diagnostic carries stable seam identity | `crates/ripr/src/lsp/diagnostics.rs` emits seam diagnostics with `data.seam_id`; boundary-gap pins the same seam in LSP diagnostics and agent packet fixtures. | Covered |
| Hover reads analysis state, not message text | `crates/ripr/src/lsp/hover.rs` resolves through latest analysis state and renders from `ClassifiedSeam`. | Covered |
| Hover explains next useful test context | Hover includes grip class, stage path, missing discriminator, related test locations, suggested test shape, packet and brief handoff commands, verify and receipt commands, static limits, and next step. | Covered by hover hardening |
| Actions are command-oriented and evidence-aware | LSP and VS Code actions expose packet, brief, assertion, related-test, after-snapshot, verify, receipt, and refresh paths. | Needs conditional audit in behavior PR |
| Context packet exists | `ripr.collectContext` can return an agent seam packet for a selected `seam_id`. | Needs canonical evidence packet |
| VS Code path is tested | Current tests cover registration, handlers, LSP-first context, related-test opening, malformed args, and restart behavior. | Needs live server smoke |
| LSP protocol path is tested | Framed protocol smoke covers server startup and real seam diagnostics for hover/actions after #569. | Needs full loop after context packet |
| Status and freshness exist | First-hour UX added status and action discoverability. | Needs explicit staleness contract |
| Boundaries are explicit | `docs/EDITOR_EVIDENCE_UX.md` repeats no source edits, generated tests, runtime mutation execution, runtime adequacy claims, or policy invention. | Covered |

## Next Lane 3 Work Item

`lsp/evidence-aware-actions`

This is the next Editor Evidence UX behavior item when the lane is explicitly
activated or explicitly run in parallel. It is not the active machine-readable
`goals next` item while Campaign 17 remains active.

Tighten action visibility and payload validation so every shown action has
usable supporting evidence or command context, malformed arguments fail closed,
and no editor action edits source, generates tests, or claims runtime proof.

## Verification Run

Run before merging this audit PR:

```bash
cargo xtask check-campaign
cargo xtask goals next
cargo xtask check-doc-index
cargo xtask markdown-links
cargo xtask check-static-language
cargo xtask check-traceability
cargo xtask check-capabilities
cargo xtask check-pr
git diff --check
```

## What Not To Do

- Do not change analyzer behavior in the audit PR.
- Do not add LSP or VS Code behavior in the audit PR.
- Do not add unsaved-buffer overlays, CodeLens, inlay hints, or semantic tokens.
- Do not generate tests or edit source from editor actions.
- Do not run mutation testing from the editor loop.
- Do not claim runtime adequacy from static evidence.
- Do not move gate adoption, baseline ledger, or generated CI behavior into
  this lane.
