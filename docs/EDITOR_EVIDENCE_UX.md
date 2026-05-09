# Editor Evidence UX

Editor Evidence UX owns the saved-workspace editor loop:

```text
diagnostic -> evidence hover -> related test -> context packet
-> one focused test -> verify -> receipt -> refresh
```

This is an editor contract, not a new analyzer lane. The editor projects RIPR's
existing evidence spine at the point of work so a developer can see the weak
seam, understand the missing observation, open or copy the relevant test
context, run the verify command after a focused test, and produce a receipt.

The campaign does not add automatic edits, generated tests, runtime mutation
execution, runtime adequacy claims, unsaved-buffer overlays, CodeLens, inlay
hints, semantic tokens, policy invention, or generated workflow changes.

## Contract

| Surface | Contract | Must not do |
| --- | --- | --- |
| Diagnostic | Carry stable seam identity in `diagnostic.data`, including `seam_id`, class, kind, owner, sink, and evidence states when available. | Reconstruct identity from message text or present stale evidence as fresh. |
| Hover | Resolve `diagnostic.data` against the latest analysis state and explain class, evidence path, missing observation, related test, suggested assertion shape, verify command, receipt command, and static limits when available. | Claim runtime adequacy or invent evidence when the current snapshot is incomplete. |
| Code actions | Show user-intent actions only when the supporting evidence or command context exists. | Offer empty payloads, unsupported actions, source edits, generated tests, or fix-all commands. |
| Context packet | Return one bounded packet for human, clipboard, or external-agent handoff with seam identity, file range, evidence, related test, suggested assertion, agent brief command, verify command, receipt command, and limits. | Couple RIPR to an LLM provider or turn the packet into a free-form reviewer. |
| VS Code extension | Resolve the server, publish diagnostics, surface hover/actions, copy command payloads, open related tests, and report restart or bad-server-path failures clearly. | Require `cargo install ripr` as the normal install path or hide server resolution failures. |
| LSP cockpit | Regress diagnostics, hover, code actions, command payloads, and VS Code command coverage from committed fixtures. | Replace real editor smoke coverage or rerun broad analysis as a report join. |
| Status | Distinguish unavailable, unresolved workspace, disabled config, queued, running, complete, stale, failed, and no-actionable-seam states. | Let stale diagnostics look like fresh evidence. |

## Current Surface Audit

| Surface | Current evidence | Gap for this campaign |
| --- | --- | --- |
| Diagnostics | `crates/ripr/src/lsp/diagnostics.rs` emits saved-workspace seam diagnostics with stable `ripr-seam-{class}` codes and `data.seam_id`; `fixtures/boundary_gap/expected/lsp-diagnostics.json` pins the boundary-gap seam identity. | Keep diagnostic identity stable and make later hover/action work depend on `diagnostic.data`, not message text. |
| Hover | `crates/ripr/src/lsp/hover.rs` renders from `ClassifiedSeam` through the latest analysis state, including grip class, RIPR stage path, missing discriminator, related tests, suggested test shape, handoff commands, verify and receipt commands, static limits, and next step. | Keep the structure pinned while later work tightens actions, context packets, protocol proof, VS Code smoke, and staleness. |
| Actions | `crates/ripr/src/lsp/actions.rs` and `crates/ripr/src/lsp/backend.rs` expose seam-aware actions for inspecting packets, targeted-test briefs, suggested assertions, related tests, agent-loop commands, verify, receipt, and refresh; targeted-test brief, suggested assertion, and related-test actions are conditional on supporting evidence; stale seam diagnostics fail closed to refresh-only; `fixtures/boundary_gap/expected/lsp-code-actions.json` pins the action payload. | Add the canonical evidence-context packet command and extend protocol/VS Code proof around the action path. |
| Context collection | Existing `ripr.collectContext` returns an agent seam packet for a known `seam_id`; `ripr.collectEvidenceContext` returns a bounded editor handoff packet with seam identity, evidence path, related test, suggested test, shared command templates, and static limits; VS Code asks LSP before CLI fallback for the existing packet path. | Extend protocol/VS Code proof around the canonical packet command. |
| VS Code proof | Extension tests cover command registration, copy handlers, LSP-first seam context, related-test opening, malformed arguments, restart behavior, and a live real-server boundary-gap path from seam diagnostic through hover, actions, copy packet/verify payloads, and related-test opening. | Keep extension proof current as regression coverage. |
| Protocol proof | Framed LSP smoke covers server startup, saved-workspace refresh, a real seam diagnostic, hover, code actions, `ripr.collectEvidenceContext`, and shutdown. | Keep protocol proof current as regression coverage. |
| Cockpit proof | `cargo xtask lsp-cockpit-report` reads committed LSP diagnostics/actions and VS Code command coverage; #569 made packet, brief, after-snapshot, verify, and receipt command actions explicit. | Keep cockpit as regression proof for command/action drift. |
| Status and staleness | The extension status bar and `ripr: Show Status` path name disabled config, missing workspace, server unavailable, queued, running, complete, no-actionable-seam, stale, and failed states. Dirty Rust buffers keep the editor in stale status until save or close, so a completed saved-workspace refresh is not presented as current unsaved evidence. | Maintain status semantics when future editor work is explicitly opened. |
| Workflow docs | `docs/EDITOR_EVIDENCE_WORKFLOW.md` gives the user-facing path from install and status through diagnostic, hover, related test, context packet, one focused test, after snapshot, verify, receipt, and refresh. | Maintain docs as the saved-workspace editor path changes. |

## Required Fields

Diagnostic-backed hover and actions should use stable data fields when present:

- `seam_id`
- file and range
- class or grip state
- seam kind
- owner
- flow sink
- configured severity or disabled state
- evidence path states
- missing discriminator or missing observation
- related test location and confidence
- suggested assertion shape
- command artifact paths

When a field is missing, the editor should fall back to a narrower explanation
or omit the dependent action. It should not synthesize a stronger claim.

## Action Visibility Rules

| Action | Show when |
| --- | --- |
| Inspect seam evidence | Diagnostic has stable seam metadata. |
| Open related test | Related-test location exists. |
| Copy focused test brief | Related-test or assertion context exists. |
| Copy suggested assertion | Suggested assertion shape exists. |
| Copy agent packet | Stable seam ID exists. |
| Copy after-snapshot command | Workspace root and artifact path context exist. |
| Copy verify command | Before and after snapshot paths are known. |
| Copy receipt command | Verify artifact and seam ID are known. |
| Refresh RIPR analysis | Always safe when the server is running. |

Malformed command arguments should fail closed with clear feedback. They should
not panic, write source, or silently copy unusable payloads.

## Static Evidence Boundary

Editor Evidence UX is intentionally conservative:

- RIPR reports static evidence and targeted test intent.
- RIPR can import runtime mutation evidence from explicit artifacts elsewhere,
  but the editor loop does not run mutation testing.
- A receipt records the relationship between static before/after artifacts. It
  is not a runtime proof.
- Suggested assertions are work-order guidance, not generated tests.
- Policy state may be displayed from existing artifacts later, but the editor
  should not invent policy or make generated CI blocking.

## Closeout

Editor Evidence UX is closed as a parallel Lane 3 campaign. The closeout audit
is recorded in
[Editor Evidence UX closeout](handoffs/2026-05-09-editor-evidence-ux-closeout.md).

Landed slices:

- hover hardening;
- evidence-aware action hardening;
- `ripr.collectEvidenceContext`;
- framed LSP protocol smoke;
- live VS Code editor evidence smoke;
- editor status and staleness;
- user-facing editor evidence workflow docs.

No queued Editor Evidence UX work item remains. Future editor work should be
opened as an explicit new campaign when usage shows a concrete need.

## Validation

Closeout and maintenance changes use the campaign and documentation gates:

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

Behavior PRs add the relevant LSP, VS Code, cockpit, output-contract, and
fixture checks listed in the active campaign manifest.
