# RIPR-SPEC-0211: MCP workspace session — status, refresh, bounded gap lists, and evidence resources

Status: proposed

Owner: mcp

Created: 2026-10-06

Linked issues:

- #3089 (this slice: status, refresh, bounded gap list, and evidence
  resources)
- #3087 (parent standard MCP server epic)
- #3088 (transport/discovery/read-only boundary slice; this slice keeps its
  SDK-owned dispatch and bounded framing)
- #3090 (sibling repair slice; this slice leaves the repair-attempt and
  receipt resources as explicit nulls reserved for it)
- #1602 (shared completed-analysis lifecycle and currentness authority;
  consumed through `app::check_workspace`)
- #1582 (bounded selection and overflow authority; consumed through
  `lsp::diagnostic_budget`)
- #1895 (shared fix-instruction meaning; not projected in this slice)

Support-tier impact:

- None. The MCP adapter runs the same read-only static analysis the CLI and
  LSP run in-process; it adds no language support claim. Preview-language
  findings keep their existing support posture through the shared producer.
  [docs/status/SUPPORT_TIERS.md](../status/SUPPORT_TIERS.md)

Policy impact:

- None. No new output format, gate, badge, or report file is added; the MCP
  documents are wire payloads bounded by the existing MCP byte caps.

## Problem

Agents need RIPR-specific discovery and evidence through standard MCP
tools/resources, not custom LSP requests or direct report-file archaeology.
The status-only MCP slice (#3088) deliberately answered "is the root usable"
and nothing more; an agent that wants the current bounded evidence working
set had to leave the protocol and reconstruct state from CLI output.

## Behavior

`ripr mcp --stdio` exposes one read-only workspace session over the pinned
official SDK transport:

- Tools `ripr_workspace_status`, `ripr_refresh`, `ripr_list_gaps`,
  `ripr_get_gap`; static resource `ripr://workspace/status`; resource
  templates `ripr://snapshot/{snapshot_id}` and
  `ripr://gap/{canonical_item_id}`. Tool names and templates follow the
  registry established in #3088's slice.
- `ripr_workspace_status` returns the startup workspace block (root
  discovery, configuration presence, trust, authority — all unchanged from
  #3088) plus a `ripr-mcp-session-v1` session block: current desired input
  (workspace diff against the default branch, draft mode, built-in
  defaults), current attempt state (`no_snapshot`, `in_flight`,
  `completed`, `failed`), last completed snapshot identity,
  last-known-good, freshness relative to the last completed refresh
  (`current_at_last_refresh`, or `stale_after_failed_attempt` once a later
  attempt fails, leaving the retained snapshot unverified for that attempt),
  the committed
  snapshot's typed `AnalysisOutcome`, and profile/support facts. No
  evidence detail is returned.
- `ripr_refresh` runs one bounded static analysis through the shared
  `app::check_workspace` authority (identical in-process analysis to
  `ripr check` and the LSP) and commits the completed snapshot. The call
  blocks to a terminal state and reports it. An attempt runs to a terminal
  state: cancelling the MCP request never rolls an attempt back or
  manufactures a snapshot, and a cancelled or superseded attempt is never
  committed. A failed attempt keeps the last-known-good snapshot.
  Closed incomplete outcomes (`unsupported_input`, `analysis_failed`) stay
  typed failures and never become snapshots; `partial_with_limitations`
  commits with its typed limitations so complete-zero and incomplete-zero
  remain distinct.
- The snapshot identity is `snapshot:sha256:` over the typed outcome digest,
  the sorted canonical item identities, and each item's evidence digest:
  equivalent roots at equivalent inputs share one portable identity, while
  the concrete root evidence stays the separate host-local root hash. An
  evidence change therefore yields a new snapshot identity instead of
  serving altered bytes under the old one.
- `ripr_list_gaps` serves the snapshot's stored shared-budget selection
  (`lsp::diagnostic_budget` with its default budget): total/eligible/
  selected/omitted counts, selected and complete serialized bytes, every
  omitted identity with its reason, the snapshot/profile/budget identity
  and selection basis, and one small summary per selected item. Eligibility
  is the producer's candidate-actionability predicate captured at
  projection time, so non-actionable items surface as disclosed
  profile-filtered omissions rather than MCP inventing its own filter. The
  adapter never re-runs ranking, never truncates silently, and infers no
  business risk. Overflow is disclosed with reasons and the
  `ripr_get_gap` continuation route.
- `ripr_get_gap` and `ripr://gap/{canonical_item_id}` return one canonical
  item's complete bounded evidence bound to its snapshot identity:
  identity/location, changed behavior (expression, before/after, delta
  kind, probe family), causal attribution (canonical gap owner, behavior
  kind, probe kind, normalized discriminator), discriminator availability,
  related tests with oracle kind/strength, typed limitations for producer
  fields that are not established, and resource links. This slice pinned
  readiness as a hard `repair_packet_ready: false` negative; RIPR-SPEC-0214
  (slice C, #3090) owns the committed producer repair-readiness evaluation
  that replaced the pin, and it also owns binding the repair boundary and
  the repair-attempt link when a session transaction exists — until then
  the boundary stays `none_declared` and the link stays an explicit null. A
  missing field stays a typed state; MCP never fills it
  from prose. `ripr://snapshot/{snapshot_id}` returns bounded snapshot
  evidence: identity, typed outcome, the full item index, and the stored
  selection summary.
- Reads may bind an explicit `snapshot_id`; a mismatch fails closed with
  `stale_snapshot` and the current identity. Reads before the first
  successful refresh fail closed with `no_snapshot`; reads during an
  attempt report `analysis_in_flight`; unknown item ids fail closed with
  `item_not_found`; a document that cannot fit the 128-KiB response bound
  fails closed with `result_too_large`. Reserved vocabulary
  (`config_invalid`, `workspace_ambiguous`, `static_limitation`,
  `cancelled`, `superseded`) is named on the wire for the slices that own
  those states.
- The read-only boundary (ADR 0022) holds for the whole surface: no source
  edit, no verification or mutation execution, no project-local
  configuration loading, no model provider, and no repair-ready claim
  anywhere. MCP does not call the LSP server, parse LSP diagnostics, or
  read VS Code artifacts as authority.

## Required Evidence

- `cargo test -p ripr --lib mcp::workspace` — session state machine,
  snapshot identity portability, stale/no-snapshot/in-flight fail-closed
  reads, complete-zero vs incomplete-zero distinctness, last-known-good
  retention across a failed refresh, bounded failure detail.
- `cargo test -p ripr --lib mcp::gaps` — canonical identity projection
  (producer gap id preferred, finding id fallback), the readiness block this
  slice pinned (the readiness evaluation itself is owned with
  RIPR-SPEC-0214), strict resource-URI parsing.
- `cargo test -p ripr --lib mcp` — descriptor contracts, positive
  LLM-facing tool descriptions, resource-template discovery, dispatch-edge
  argument rejection.
- `crates/ripr/tests/mcp_sdk.rs` — the pinned official SDK client
  discovers the slice-B tools (four at that slice; the surface is seven
  tools and four templates after RIPR-SPEC-0214), the static resource, and
  the resource templates across the `initialize` and `server/discover`
  lifecycles, and the status tool/resource project the same session
  document.
- `crates/ripr/tests/mcp_stdio.rs` — raw-wire controls: tool/resource
  equality, rejection arms, and the new fail-closed control proving a
  stock-shaped client receives typed `no_snapshot` failures (and the
  resource-miss mapping) before the first refresh.
- `cargo test -p ripr --lib lsp::diagnostic_budget` — the shared budget
  authority this slice consumes stays green.

## Non-Goals

- No repair-attempt creation, receipt status, or CommandSpec projection
  tools; #3090 owns them, and this slice's `ripr_get_gap` leaves their
  links as explicit nulls so #3090 can extend the document without a
  breaking change.
- No source edit or command execution. This slice pinned readiness as a
  hard negative; RIPR-SPEC-0214 (slice C, #3090) owns the committed
  producer repair-readiness evaluation that replaced that pin.
- No custom LSP request expansion; MCP and LSP remain peers over shared
  producers.
- No provider-specific configuration; project-local `ripr.toml` stays
  detected-not-loaded.
- No support-tier promotion.
- No durable snapshot persistence; the session is in-memory and a server
  restart drops it (a fresh `ripr_refresh` rebuilds it).

## Acceptance Examples

1. A stock MCP client calls `ripr_workspace_status` (root + authority +
   session facts), `ripr_refresh` (one bounded analysis, terminal-state
   report), `ripr_list_gaps` (bounded working set with disclosed
   omissions), and `ripr_get_gap` (one complete item), or reads
   `ripr://workspace/status`, `ripr://snapshot/{snapshot_id}`, and
   `ripr://gap/{canonical_item_id}` — no report-file, clipboard, log, or
   LSP-protocol archaeology involved.
2. Before any refresh, list/get/snapshot reads fail closed with typed
   `no_snapshot`; during an attempt they report `analysis_in_flight`; a
   named snapshot that is not current fails with `stale_snapshot` naming
   the current identity.
3. A refresh whose outcome is `unsupported_input` or `analysis_failed`
   never commits a snapshot; the previous last-known-good snapshot remains
   readable; a `partial_with_limitations` outcome commits with its typed
   limitations and stays distinct from `complete_no_findings`.
4. Two equivalent roots at equivalent inputs produce the same
   `snapshot:sha256:` identity, while each server's status document keeps
   its own host-local root hash.
5. `ripr_get_gap` for any item reports the committed producer
   repair-readiness facts (the evaluation is owned by RIPR-SPEC-0214, slice
   C #3090, which superseded this slice's hard `repair_packet_ready: false`
   pin), a `none_declared` repair boundary, and an explicit-null
   repair-attempt link until a session transaction binds one; missing
   producer fields remain typed states.

## Test Mapping

- `crates/ripr/src/mcp/workspace.rs::tests` — session lifecycle, typed
  failures, identity portability, boundedness, last-known-good retention.
- `crates/ripr/src/mcp/gaps.rs::tests` — canonical item projection and the
  readiness block this slice pinned (the readiness evaluation itself is
  owned with RIPR-SPEC-0214).
- `crates/ripr/src/mcp/protocol.rs::tests` + `server_tests.rs` —
  descriptor and dispatch contracts.
- `crates/ripr/tests/mcp_sdk.rs`, `crates/ripr/tests/mcp_stdio.rs` —
  hosted wire interop controls.

## Implementation Mapping

| Surface | Responsibility |
| --- | --- |
| `crates/ripr/src/mcp/workspace.rs` | session state machine, snapshot binding, typed failures, bounded documents, `run_check` bridge to `app::check_workspace` |
| `crates/ripr/src/mcp/gaps.rs` | canonical item projection, budget items, resource-URI parsing |
| `crates/ripr/src/mcp/protocol.rs` | tool/resource descriptors, schemas, instructions, status document |
| `crates/ripr/src/mcp/server.rs` | SDK adapter dispatch, refresh single-flight, resource reads |
| `crates/ripr/src/mcp/transport.rs` + `workspace_status.rs` | retained canonical root for in-process refresh (never serialized) |
| `docs/interop/mcp.md`, `docs/adr/0022` (Slice B note) | operator-facing surface and authority boundary record |
| `docs/specs/README.md` + `.ripr/traceability.toml` | spec registration and test traceability |

## Metrics

- `mcp_tools` (4) and `mcp_resource_templates` (2) on the slice surface
- typed pre-refresh failure controls on the wire (target: 3 — list, get,
  snapshot resource)
- zero committed snapshots for `unsupported_input` / `analysis_failed`
  outcomes (negative invariant)
