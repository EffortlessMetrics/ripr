# Editor Evidence UX

Campaign 17 owns the saved-workspace editor loop:

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
| Hover | `crates/ripr/src/lsp/hover.rs` renders from `ClassifiedSeam` through the latest analysis state, including grip class, RIPR stage path, missing discriminator, related tests, and next step. | Harden structure and fallback behavior so missing discriminator, related test, assertion shape, verify, receipt, and limits are consistently visible when present. |
| Actions | `crates/ripr/src/lsp/actions.rs` and `crates/ripr/src/lsp/backend.rs` expose seam-aware actions for inspecting packets, targeted-test briefs, suggested assertions, related tests, agent-loop commands, verify, receipt, and refresh; `fixtures/boundary_gap/expected/lsp-code-actions.json` pins the action payload. | Tighten conditional visibility and malformed-argument failure so every shown action has usable evidence or command context. |
| Context collection | Existing `ripr.collectContext` returns an agent seam packet for a known `seam_id`; VS Code asks LSP before CLI fallback. | Add a canonical evidence-context packet shape for editor handoff parity across human and external-agent use. |
| VS Code proof | Extension tests cover command registration, copy handlers, LSP-first seam context, related-test opening, malformed arguments, and restart behavior. | Add live extension smoke that uses the real server path to reach diagnostics, hover, actions, command payloads, related-test opening, restart, and bad-server-path status. |
| Protocol proof | Framed LSP smoke covers server startup and now uses a real seam diagnostic for hover and code action coverage. | Extend it to the full editor loop, including the canonical context packet command once that command exists. |
| Cockpit proof | `cargo xtask lsp-cockpit-report` reads committed LSP diagnostics/actions and VS Code command coverage; #569 made packet, brief, after-snapshot, verify, and receipt command actions explicit. | Keep cockpit as regression proof while behavior PRs pin the richer hover/action/status contracts. |
| Status and staleness | Campaign 12 added first-run status and intent-titled actions; extension docs describe saved-workspace analysis status. | Make freshness and failure states explicit enough that stale seam evidence is never presented as current. |

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

## Next Work

The audit keeps the first behavior-bearing slice narrow:

1. `lsp/evidence-hover-hardening`
2. `lsp/evidence-aware-actions`
3. `lsp/context-packet-command`
4. `test/lsp-protocol-smoke`
5. `test/vscode-extension-smoke`
6. `lsp/editor-status-and-staleness`
7. `docs/editor-evidence-workflow`
8. `campaign/editor-evidence-ux-closeout`

Start with hover. It is the first human explanation surface and should make the
next useful test action clear before any new editor affordance is added.

## Validation

Docs-only audit changes use the campaign and documentation gates:

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
