# RIPR-SPEC-0129: Editor integration contract and support matrix

Status: proposed

This specification defines the three-layer editor integration contract and
the support matrix that ratifies which client classes ripr supports and
under what conditions. It is the contract authority for #1622 and the
#1574 epic.

## Problem

The ripr LSP server advertises capabilities (diagnostics, hover, code
actions, codeLens, pull diagnostics, the `experimental.riprAgent`
protocol) that different editor clients consume differently. Without a
ratified contract, each client integration is ad-hoc: a generic client
may receive commands it cannot execute, a VS Code client may miss
enhanced features, and the headless-agent protocol may be assumed before
its handlers exist.

This spec defines three client layers, the capabilities each layer
consumes, and the conditions under which ripr advertises each.

## Three-layer client contract

### Layer 1 — Standard LSP baseline

Any off-the-shelf LSP client (Neovim, Helix, Eglot, etc.) that implements
the base LSP specification.

Consumes:
- `textDocument/publishDiagnostics` (push) or `textDocument/diagnostic` (pull)
- `textDocument/hover`
- `textDocument/codeAction` (kind strings are metadata visible to every
  layer: the advertised `quickfix.ripr` / `source.ripr.*` hierarchy; the
  negotiated surface is the command IDs inside the actions, not the kinds)
- `workspace/status` via custom notification

Does NOT consume:
- `experimental.riprAgent` requests (reserved, `supported_requests: []`)
- `experimental.riprEditor` negotiated client commands
- `ripr.copyContext` / `ripr.openRelatedTest` (client-command actions)

Requirement: the server must NOT emit unknown command IDs to a Layer 1
client. The `experimental.riprEditor` capability negotiation (#1628)
filters client-command actions to clients that advertise support
(delivered, #1776): `lsp/actions.rs` emits a client-executed command only
when the negotiated `ClientFeatureProfile` advertised it, and the VS Code
extension's `RIPR_CLIENT_COMMANDS` advertisement covers every `ripr.*`
command the extension registers, so the filter strips nothing from the
enhanced client.

Omit-vs-disabled policy (#1892): a Layer 1 client that advertises
`CodeAction.disabled` support receives a suppressed client-command action
inert instead of omitted — the command and edit are stripped (a disabled
action that still executes is the cardinal-sin flip), the kind is retained
so the `CodeActionContext.only` filter keeps working, `disabled.reason`
carries the human explanation, and `data.disabled_reason` names the
machine reason from a closed vocabulary. A client without disabled support
keeps the fail-closed omission. Server-executed commands stay
unconditional in both forms.

Code action data contract (#1892): every ripr code action carries a
bounded, versioned `CodeAction.data` payload
(`schema_version: "ripr-code-action-data-v1"`) with a deterministic
fingerprinted `action_id` (action class + canonical addressed identity +
command id + action name — never title text), the action class and kind,
the stable snake_case `action_name` machine identity (which keeps
constructors that share one command id on one diagnostic fingerprinting
distinctly), the addressed identity fields (`diagnostic_id`,
`canonical_gap_id`, `gap_id`, `seam_id`, `finding_id` as applicable), the
snapshot `input_identity`, the `required_client_capability` (the
client-command id, or `"server"` for server-executed commands), and
`disabled_reason` only when disabled.
Diagnostic-addressing actions attach the addressed diagnostic in
`CodeAction.diagnostics`. The payload carries no absolute paths, no
fix-instruction summaries, and no retrieval references — those belong to
`codeAction/resolve` (#1751). Disabled reasons are a closed vocabulary in
`lsp/action_contract.rs`; only reasons with a real producer are emitted —
today `stale_snapshot`, `client_capability_missing`,
`verification_route_unavailable`, `receipt_route_unavailable`, and
`preview_or_static_limitation` — while the reserved members
(`stale_document`, `workspace_root_blocked`, `fix_site_unavailable`,
`exact_replacement_unavailable`, `ambiguous_fix_site`,
`outside_allowed_edit_surface`, `configuration_invalid`) stay unemittable
until their named producers land.

### Layer 2 — Enhanced VS Code

The ripr VS Code extension, which advertises `experimental.riprEditor`
with its supported `client_commands[]` at `initialize`.

Consumes everything in Layer 1, plus:
- `experimental.riprEditor` client-command actions (`ripr.copyContext`,
  `ripr.copyAgentPacket`, `ripr.openRelatedTest`, etc.)
- `ripr.collectWorkspaceStatus` / `collectRepairPacket` (legacy execute-command surface)
- `codeAction` kinds: `source.ripr.inspect`, `source.ripr.navigate`,
  `source.ripr.verify`, `source.ripr.refresh`
- Managed server provisioning (#1624)
- Workspace Trust enforcement (#1623)

### Layer 3 — Headless `riprAgent`

A programmatic agent client (Codex, CI tooling) that drives the typed
`experimental.riprAgent` protocol over saved-workspace snapshots.

Consumes everything in Layer 1, plus:
- `ripr/workspaceStatus`, `ripr/refreshAnalysis`
- `ripr/listActionableItems`, `ripr/getRepairPacket`
- `ripr/getEvidenceContext`, `ripr/getTopLimitation`, `ripr/getReceiptStatus`
- Snapshot-bound continuations (#1698)

Does NOT consume:
- Client-command actions (no clipboard, no editor UI)
- `experimental.riprEditor` negotiation

Status: **reserved** — `supported_requests: []` until #1602/#1603 land
real handlers. The capability is advertised as `capability_only`.

## Support matrix

| Dimension | Layer 1 (standard) | Layer 2 (VS Code) | Layer 3 (headless agent) |
|---|---|---|---|
| Transport | stdio | stdio | stdio |
| Diagnostic mode | push or pull | push or pull | pull (preferred) |
| Custom commands | none | `ripr.collect*` | `ripr/<request>` |
| Source-edit capability | none | none (read-only) | none (reserved) |
| Server provisioning | PATH / `cargo install` | managed download (#1624) | binary / Docker |
| Trust enforcement | n/a | `untrustedWorkspaces: limited` (#1623) | n/a |
| Reload | manual restart | `didChangeWatchedFiles` (#1577) | `ripr/refreshAnalysis` |
| Real-repo proof | #1630 (pending) | #1579 (pending) | #1579 (pending) |
| Tier | preview | preview | reserved |

## Acceptance Examples

- A standard LSP client (e.g. Neovim) that does NOT advertise
  `experimental.riprEditor` receives zero `ripr.copyContext` or
  `ripr.openRelatedTest` command IDs in code actions — only server-executed
  commands. The `source.ripr.*` kind strings still appear as kind metadata
  for every client; they carry no negotiation requirement.
- A client that forwards a command the server does not execute — an unknown
  command ID, or a client-registered command such as `ripr.copyContext` —
  receives a stable `InvalidParams` rejection naming the command
  (``unsupported command `<id>`: not a server-executed ripr command``), never
  a silent no-op (#1628).
- A standard LSP client that advertises `CodeAction.disabled` support (but
  no `experimental.riprEditor`) instead receives the otherwise-suppressed
  client-command actions inert (#1892): each carries its kind, the
  addressed diagnostic, a versioned `data` payload naming `disabled_reason:
  "client_capability_missing"`, and no command or edit — so the action is
  visible but cannot execute. Server-executed actions (e.g. Refresh
  Analysis) stay active; only the client-command actions that the omission
  path would have stripped are disabled.
- A VS Code client that advertises `experimental.riprEditor` with
  `client_commands: ["ripr.copyContext"]` receives `source.ripr.inspect`
  code actions containing `ripr.copyContext` commands.
- A headless agent client sees `experimental.riprAgent` advertised with
  `supported_requests: []` and `implementation_state: "capability_only"`
  until #1602/#1603 land real handlers.

## Test Mapping

- `capabilities.rs` tests verify the capability advertisement shape
  (pull diagnostics, code action kinds, riprAgent capability).
- `tests.rs` code-action tests verify the negotiated client-command filter
  (#1776): an unenhanced client receives only server-executed commands, a
  client advertising a subset keeps exactly that subset, and every emitted
  command ID stays within the `executeCommandProvider` set or the
  negotiated `riprEditor.commands` advertisement. The full-profile tests
  parse the advertisement from `editors/vscode/src/client.ts`, so a command
  the extension registers but does not advertise fails the parity tests.
  Further `tests.rs` tests verify `CodeActionContext.only` honoring (#1750)
  and the emitted-kinds ⊆ advertised-kinds parity invariant.
- `action_contract.rs` tests verify the versioned data payload shape, the
  deterministic `action_id`, and the closed disabled-reason vocabulary
  (#1892); `actions.rs` tests verify the fail-closed emit guard; further
  `tests.rs` tests verify the omit-vs-disabled policy both directions, the
  disabled-never-executes invariant across scenarios, the kind retention
  under `context.only`, and the named suppression reasons (`stale_snapshot`,
  `verification_route_unavailable`, `receipt_route_unavailable`,
  `preview_or_static_limitation`).
- `agent_protocol.rs` tests verify the fail-closed `supported_requests: []`
  invariant.

## Implementation Mapping

- `crates/ripr/src/lsp/capabilities.rs` — capability advertisement
- `crates/ripr/src/lsp/agent_protocol.rs` — `experimental.riprAgent`
- `crates/ripr/src/lsp/actions.rs` — code action kind classification and
  the omit-vs-disabled client-command policy (#1776, #1892)
- `crates/ripr/src/lsp/action_contract.rs` — versioned `CodeAction.data`
  payload and the closed disabled-reason vocabulary (#1892)
- `crates/ripr/src/lsp/client_features.rs` — negotiated
  `ClientFeatureProfile` consumed by the code-action command filter (#1776)
- `editors/vscode/src/client.ts` — VS Code client capability advertisement
- `editors/vscode/package.json` — `capabilities.untrustedWorkspaces`

## Metrics

- Layer coverage: count of support-matrix dimensions proven by a real
  client test (target: all 10 per layer before tier promotion).
- Capability drift: zero — `check-spec-format` validates the spec exists
  and is current when capabilities change.

## Behavior

The server advertises capabilities at `initialize` based on what the
client supports:
- Pull diagnostics are advertised only if the client supports
  `textDocument/diagnostic` (detected from `window.workDoneProgress` +
  the pull capability in client capabilities).
- `experimental.riprAgent` is always advertised (fail-closed,
  `supported_requests: []`) so a Layer 3 client can detect the server.
- `experimental.riprEditor` negotiation (#1628, delivered in #1776) filters
  client-command code actions to clients that advertise support.
- Code action kinds are advertised as `quickfix.ripr` and
  `source.ripr.*` (#1750, landed). Every kind the server emits stays within
  the advertised set (parity-pinned by
  `tests.rs::code_action_response_emitted_kinds_stay_within_the_advertised_set`
  against the shared `ADVERTISED_CODE_ACTION_KINDS` constant).
- `textDocument/codeAction` honors `CodeActionContext.only` (#1750): with
  LSP 3.17 hierarchical kind semantics, an action survives when any
  requested kind equals the action's kind or is a dot-segment prefix of it
  (`source` keeps every `source.ripr.*` action; `source.ripr.navigate`
  keeps only that subtree). An absent `only` leaves the response
  unfiltered, and an action with no kind fails closed. The kind filter
  compounds with the negotiated client-command filter.
- Every code action carries the versioned `ripr-code-action-data-v1` data
  payload, and diagnostic-addressing actions attach the addressed
  diagnostic (#1892). The #1776 client-command filter becomes the
  omit-vs-disabled policy: `CodeAction.disabled`-capable clients receive
  suppressed client-command actions inert (command and edit stripped, kind
  retained, `client_capability_missing` named); clients without disabled
  support keep the fail-closed omission. The same disabled form names the
  staleness and limitation suppressions that previously omitted silently:
  `stale_snapshot` (stale gap diagnostic),
  `verification_route_unavailable` / `receipt_route_unavailable` (gap
  record without a safe verify/receipt command), and
  `preview_or_static_limitation` (cross-language unresolved target). The
  backend health/root gate stays omit-only: without a snapshot no
  diagnostic-addressing action can be constructed.

The server does NOT:
- Emit unknown command IDs to a client that has not negotiated them.
- Advertise `source_edit_capability` as anything other than `"none"`.
- Require any specific editor; the standard LSP baseline works with
  any compliant client.

## Required Evidence

- `crates/ripr/src/lsp/capabilities.rs` — capability advertisement code.
- `crates/ripr/src/lsp/agent_protocol.rs` — `experimental.riprAgent`
  fail-closed capability.
- `editors/vscode/package.json` — `capabilities.untrustedWorkspaces`.
- This spec registered in `docs/specs/README.md`.

## Non-Goals

- This spec does not define the `riprAgent` protocol DTOs — that is #1599.
- This spec does not define the diagnostic code catalog — that is #1662.
- This spec does not gate on real-client proof — #1630/#1702 own the
  off-the-shelf-client dogfood.

## Versioning

This spec is versioned as `editor-integration-contract-v1`. Changes to
the layer definitions or support matrix require a spec amendment.
