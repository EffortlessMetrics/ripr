# Handoff: Editor Evidence UX Audit

Date: 2026-05-08
Branch / PR: `docs-editor-evidence-ux-audit` / pending
Latest merged PR: #586 `campaign: activate editor evidence ux`

## Current Work Item

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
| Hover explains next useful test context | Existing hover includes grip class, stage path, missing discriminator, related tests, and next step. | Needs hardening |
| Actions are command-oriented and evidence-aware | LSP and VS Code actions expose packet, brief, assertion, related-test, after-snapshot, verify, receipt, and refresh paths. | Needs conditional audit in behavior PR |
| Context packet exists | `ripr.collectContext` can return an agent seam packet for a selected `seam_id`. | Needs canonical evidence packet |
| VS Code path is tested | Current tests cover registration, handlers, LSP-first context, related-test opening, malformed args, and restart behavior. | Needs live server smoke |
| LSP protocol path is tested | Framed protocol smoke covers server startup and real seam diagnostics for hover/actions after #569. | Needs full loop after context packet |
| Status and freshness exist | First-hour UX added status and action discoverability. | Needs explicit staleness contract |
| Boundaries are explicit | `docs/EDITOR_EVIDENCE_UX.md` repeats no source edits, generated tests, runtime mutation execution, runtime adequacy claims, or policy invention. | Covered |

## Next Work Item

`lsp/evidence-hover-hardening`

Make hover the primary human explanation surface. It should show, when
available, seam class, configured status, evidence path, missing observation,
related test, oracle strength, suggested assertion shape, verify command,
receipt command, and static limits. It should resolve from `diagnostic.data` to
the latest analysis state and fall back cleanly when the evidence is incomplete.

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
