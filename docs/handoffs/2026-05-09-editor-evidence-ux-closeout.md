# Handoff: Editor Evidence UX Closeout

Date: 2026-05-09
Branch / PR: `campaign-editor-evidence-ux-closeout` / pending
Latest merged PR: #604 `docs: document editor evidence workflow` (commit `46e2165`)

## Current Work Item

`campaign/editor-evidence-ux-closeout`

Editor Evidence UX made the saved-workspace LSP path usable as a local
test-intent cockpit:

```text
diagnostic
-> evidence hover
-> related test or context packet
-> one focused test
-> after snapshot
-> verify
-> receipt
-> refresh
```

This was a parallel Lane 3 closeout over existing editor surfaces. It did not
change analyzer behavior, policy or gate behavior, generated CI behavior,
SARIF, badges, output schemas beyond the already-landed LSP context packet,
public crate shape, runtime mutation execution, automatic source edits,
generated tests, unsaved-buffer overlays, CodeLens, inlay hints, or semantic
tokens.

## Prompt-To-Artifact Audit

| Requirement | Evidence |
| --- | --- |
| Diagnostic carries stable seam identity | `crates/ripr/src/lsp/diagnostics.rs` emits seam diagnostics with `data.seam_id`; `fixtures/boundary_gap/expected/lsp-diagnostics.json` pins the boundary-gap seam identity; LSP tests cover seam lookup through diagnostic data and unknown seam-id fail-closed behavior. |
| Hover makes missing observation legible | `crates/ripr/src/lsp/hover.rs` renders `Why this diagnostic?`, evidence path, missing discriminator, related test, suggested test shape, handoff/verify/receipt commands, and static limits from `ClassifiedSeam`; LSP tests assert missing discriminator, suggested test shape, handoff commands, and static-limit language. |
| Hover does not parse diagnostic message text | The hover path resolves `diagnostic.data` against the latest analysis snapshot; LSP tests prefer seam-id lookup over overlapping finding diagnostics and return no seam when `data.seam_id` is unknown. |
| Related-test action points to an imitation target | `crates/ripr/src/lsp/actions.rs` selects the strongest related test first, then highest confidence; LSP tests cover strongest-test and confidence fallback selection; VS Code e2e opens the related test from the real boundary-gap server path. |
| Small assertion or test shape is exposed | `crates/ripr/src/lsp/hover.rs` shows `Suggested test shape`; `crates/ripr/src/lsp/actions.rs` exposes `Write targeted test: copy suggested assertion` and `Write targeted test: copy brief` only when supporting evidence exists; tests cover omission when evidence is missing and keeping the brief when related-test context exists. |
| Bounded context packet exists | `ripr.collectEvidenceContext` in `crates/ripr/src/lsp/backend.rs` returns schema `0.1` with seam identity, file range, evidence path, missing discriminator, related test, suggested test, shared agent-loop commands, and static limits; `docs/OUTPUT_SCHEMA.md` documents the packet; framed LSP smoke executes it against a real published seam diagnostic. |
| Handoff, verify, and receipt commands are exact and shared | LSP actions and hover use shared agent-loop command templates for packet, brief, after snapshot, verify, and receipt commands; tests pin command titles and payloads; `fixtures/boundary_gap/expected/lsp-code-actions.json` and editor-agent-loop fixtures pin public payloads. |
| Code actions fail closed | `crates/ripr/src/lsp/actions.rs` omits unsupported targeted-test, assertion, and related-test actions; stale seam diagnostics expose refresh-only behavior; malformed VS Code command arguments are covered in extension tests. |
| Status and staleness are visible | `editors/vscode/src/client.ts` models disabled, workspace unresolved, server unavailable, queued, running, complete, no-actionable-seam, stale, and failed states; dirty Rust buffers keep stale status visible until save or close; VS Code tests cover disabled config, missing workspace, missing server, queued/running/complete/no-seam/failed, and dirty-buffer stale behavior. |
| Protocol path is framed | `crates/ripr/src/lsp/tests.rs` drives initialize, saved-workspace refresh, a real boundary-gap seam diagnostic, hover, code actions, `ripr.collectEvidenceContext`, and shutdown without relying on VS Code. |
| Real VS Code path is covered | `editors/vscode/test/suite/extension.test.ts` activates the extension, resolves the real server, waits for a boundary-gap seam diagnostic, verifies hover and code actions, copies seam packet and verify payloads, opens the related test, covers malformed arguments, and checks restart/status behavior. |
| User workflow is documented | `docs/EDITOR_EVIDENCE_WORKFLOW.md` walks install/status, diagnostic, hover, related test, context packet, one focused test, after snapshot, verify, receipt, and refresh with explicit static-evidence limits; `docs/EDITOR_EXTENSION.md`, `docs/QUICKSTART.md`, and `editors/vscode/README.md` link to it. |
| Non-goals stayed intact | The editor docs and tests preserve no source edits, no generated tests, no runtime mutation execution, no runtime adequacy claims, no policy invention, no unsaved-buffer overlays, no CodeLens, no inlay hints, no semantic tokens, and no LLM provider coupling. |

## PR Chain

- #589 `docs: audit editor evidence contract`
- #592 `lsp: harden evidence-rich hover`
- #593 `lsp: tighten evidence-aware actions`
- #595 `lsp: add evidence context packet`
- #596 `test: harden framed LSP protocol smoke`
- #598 `test: smoke vscode editor evidence loop`
- #602 `lsp: clarify editor status and staleness`
- #604 `docs: document editor evidence workflow`
- `campaign/editor-evidence-ux-closeout`

## Verification Run

Closeout proof before opening this PR:

```bash
cargo test -p ripr lsp --lib
cargo test -p ripr lsp::tests --lib
cargo xtask lsp-cockpit-report
npm --prefix editors/vscode run compile
npm --prefix editors/vscode run test:e2e
cargo xtask check-output-contracts
cargo xtask check-static-language
cargo xtask check-traceability
cargo xtask check-capabilities
cargo xtask check-doc-index
cargo xtask markdown-links
cargo xtask check-campaign
cargo xtask goals next
cargo xtask check-pr
git diff --check
```

## Next Work Item

No Lane 3 work item remains in the queued Editor Evidence UX campaign. Choose
the next editor campaign explicitly if product usage shows a concrete need.

Likely later editor work remains outside this closeout:

- unsaved-buffer overlays;
- CodeLens;
- inlay hints;
- semantic tokens;
- inline patch application;
- automatic test generation;
- runtime mutation execution;
- policy editing from the editor.

## What Not To Do

- Do not reopen Editor Evidence UX to add speculative editor surfaces.
- Do not add automatic source edits or generated tests.
- Do not run mutation testing from the editor path.
- Do not claim runtime adequacy from static LSP evidence.
- Do not invent policy or gate state inside the editor.
- Do not make generated CI or gate behavior part of Lane 3 closeout.
- Do not split `ripr` into new public crates for editor internals.
