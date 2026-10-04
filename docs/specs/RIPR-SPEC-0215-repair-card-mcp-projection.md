# RIPR-SPEC-0215: MCP repair card projection

Status: proposed

Owner: mcp

Created: 2026-10-05

Linked issues:

- #4668 (this slice: `ripr_get_repair_card` and the
  `ripr://repair-card/{canonical_item_id}` resource; closes the MCP half of
  the RepairCard transport parity issue)
- #3087 (parent standard MCP server epic)
- #3088 / RIPR-SPEC-0211 (transport/discovery/session-evidence slices; this
  slice keeps their SDK-owned dispatch, bounded framing, and snapshot
  freshness contract)
- #4667 / RIPR-SPEC-0194 (the `ripr agent card` CLI handoff; this slice
  projects the same `RepairCardV1` through the same
  `app::repair_card_handoff` assembly authority)
- RIPR-SPEC-0198 (the standard-LSP repair card projection; this slice is its
  MCP peer — same card, same producers, different transport binding)
- RIPR-SPEC-0214 (sibling repair-transaction slice; the card's attempt block
  reads the same durable store the attempt/receipt tools expose, and session
  transactions stay reachable only through those tools)
- ADR 0022 (bounded read-only MCP adapter; this slice adds no execution
  authority)

Support-tier impact:

- None. The slice projects producer facts the CLI and LSP already compute;
  it adds no language support claim and runs no new analysis.
  [docs/status/SUPPORT_TIERS.md](../status/SUPPORT_TIERS.md)

Policy impact:

- None. No new output format, gate, badge, or report file is added; the card
  document is a wire payload bounded by the existing MCP byte caps. The wire
  failure vocabulary gains the five `AgentCardRefusalKind` spellings
  (`seam_not_found`, `policy_omitted`, `witness_unavailable`,
  `identity_unnameable`, `budget_overflow`); `policy_omitted` and
  `witness_unavailable` stay named for wire stability even though this slice
  cannot reach them.

## Problem

An agent that identified one actionable canonical item through the session
evidence and repair-transaction slices still had to leave the protocol to
assemble the bounded repair card `ripr agent card` and the standard language
server project: no MCP route named the seam-bound card facts (changed
behavior, exact blocker, fix instruction, edit cage, attempt state, typed
next action) for one item of the committed snapshot. Slice B deliberately
left card projection out of the gap evidence document so this slice could
bind the card's producer facts at snapshot commit time without a breaking
change.

## Behavior

`ripr mcp --stdio` gains one tool and one resource template, both read-only
and without execution authority (ADR 0022):

- Tool `ripr_get_repair_card` (inputs `gap_id`, optional `snapshot_id`) and
  resource template `ripr://repair-card/{canonical_item_id}`, routing
  canonical item ids exactly like `ripr://gap/{canonical_item_id}`.
- The tool and the template project the same versioned `repair_card.v1`
  document the CLI `ripr agent card` handoff (#4667) and the standard-LSP
  projection (RIPR-SPEC-0198) consume, assembled by the shared pure
  [`app::repair_card_handoff::assemble_repair_card`] authority from
  producer-bound facts. The MCP layer never re-derives readiness, identity,
  currentness, or route state and never reconstructs a card from weaker
  evidence; the card's `repair_card_id` therefore matches the CLI/LSP card
  for the same facts.
- Two transport facts are snapshot-bound instead of request-bound, because
  ADR 0022 forbids the adapter from launching git per read: the analyzed
  repository head (`git rev-parse HEAD`) and the diff-scoped classified seam
  inventory, each with its evidence-scope dirty-state currentness probe
  (`Current` / `AcceptedDirtyDraft`), are bound when `ripr_refresh` commits
  the snapshot — the adapter's single bounded analysis attempt. A seam is
  retained only when it owner-discriminated binds a snapshot canonical item:
  the seam's readiness canonical gap id must name the item and the finding's
  producer-recorded gap owner must be the seam's owner, so a gap id shared
  with a sibling owner never credits this seam with another seam's item.
  Any binding failure fails the whole refresh attempt with
  `analysis_failed`: the snapshot commits complete — items, findings, head,
  and card seams — or not at all, so the card surface can never silently
  vanish under a served snapshot identity.
- The durable repair-attempt store is plain filesystem state, not analysis:
  it is re-read live at card-read time through the same
  [`latest_attempt_for_seam`] inventory the CLI card producer runs, so the
  card's attempt block matches `ripr agent status` at this moment. An
  unreadable store fails the read closed with `analysis_failed` rather than
  projecting a wrongly empty attempt state. In-memory session transactions
  never ride a card; they stay reachable through `ripr_prepare_repair` /
  `ripr_get_repair_attempt`.
- The next-action command binds the portable root `.`: the host-local
  checkout path is intentionally not projected (ADR 0022), and the display
  string is presentation only, never execution authority.
- The wire document is versioned `ripr-mcp-repair-card-v1` and carries the
  verbatim card, the item identity, the currentness basis (the analyzed head
  plus the commit-time probe), the claim boundary, limitations, and links to
  the snapshot, gap, repair-attempt, and receipt resources.
- Fail-closed states: before the first successful refresh, and for
  snapshots committed before this slice existed (no bound card producers),
  the read fails with `no_snapshot`; during an attempt it reports
  `analysis_in_flight`; a mismatched `snapshot_id` fails with
  `stale_snapshot`; an unknown item fails with `item_not_found`; an item no
  seam owner-discriminated binds fails with `seam_not_found`; an unnameable
  portable workspace identity fails with `identity_unnameable`; a card over
  the detail budget fails with `budget_overflow`; a document over the
  response bound fails with `result_too_large`; an unavailable workspace
  root (the durable store cannot be read) fails with
  `workspace_unavailable`; producer operational failures stay
  `analysis_failed`. Refusal kinds travel under their CLI-pinned wire
  spellings via `AgentCardRefusalKind::as_str()`, so one machine state names
  one refusal on every transport. Unknown tool arguments are rejected at
  the dispatch edge with `INVALID_PARAMS`.
- The card binds the *analyzed* head and the commit-time currentness of its
  snapshot; post-refresh edits stay invisible until the next refresh — the
  session's existing freshness contract. Unlike the standard-LSP projection
  (RIPR-SPEC-0198), no live-head binding window exists: the MCP adapter
  never re-resolves git state per read, and the document says so.

## Required Evidence

- Unit controls over the projection: strict resource-template id parsing;
  fail-closed reads for every missing producer fact (no snapshot,
  producer-less legacy snapshot, unbound item, unknown item, stale
  snapshot, in-flight attempt); the witness-bound card projects the shared
  handoff card verbatim (same `repair_card.v1` schema, seam subject, finding
  identity, analyzed head, nine detail-reference families, null next action
  on a gated route); refusal kinds map onto their pinned wire spellings with
  detail and recovery; an owner-mismatched finding never credits the seam
  with another owner's item.
- Wire controls: the SDK session discovers all eight tools and five
  templates; the stdio fail-closed control pins the typed pre-refresh
  `no_snapshot` failure for the card tool; the inline-version recovery
  control expects the eight-tool surface.

## Non-Goals

- No source edit, test authoring, process launch, or verification or
  mutation command execution; the external client's approval and sandbox
  policy remains authoritative for every command a returned route names.
- No git per read: the analyzed head and currentness are commit-time facts;
  the adapter never re-resolves repository state between refreshes.
- No durable attempt creation or mutation; the card reads the durable store
  through the shared read-only inventory.
- No session-transaction projection on the card; in-memory repairs stay
  behind `ripr_prepare_repair` / `ripr_get_repair_attempt`.
- No project-local policy or configuration loading; the inventory and the
  evidence facts both run with built-in defaults
  (`detected_not_loaded`).
- No card re-assembly, readiness re-derivation, or evidence upgrade in the
  adapter; the shared assembly authority is the only card builder.
- No custom LSP request expansion; MCP and LSP remain peers over shared
  producers.
- No support-tier promotion.

## Acceptance Examples

1. A stock MCP client refreshes, lists gaps, and calls
   `ripr_get_repair_card` for one item whose seam owner-discriminated binds
   it: the response carries the verbatim `repair_card.v1` card with the
   seam subject, the analyzed repository head, the commit-time currentness,
   the fix instruction and edit cage, the live durable attempt state when
   one exists, and links to the snapshot, gap, repair-attempt, and receipt
   resources; the same document answers a
   `ripr://repair-card/{canonical_item_id}` resource read.
2. The same client refreshes a workspace whose evidence changed after the
   snapshot: the card still binds the analyzed head and the commit-time
   currentness of its snapshot, and the document's limitations name that
   contract instead of silently rebinding to live state.
3. A card read for an item whose finding names a different gap owner fails
   closed with `seam_not_found`; a card read before any refresh, or against
   a snapshot committed before this slice, fails closed with `no_snapshot`.
4. A refresh whose seam-inventory or currentness binding fails fails the
   whole attempt with `analysis_failed` and commits no partial snapshot.

## Test Mapping

- `crates/ripr/src/mcp/repair_card.rs::tests` — resource id parsing,
  fail-closed producer facts, the witness-bound shared card projection,
  refusal wire spellings, and the owner-discrimination binding rule.
- `crates/ripr/src/mcp/protocol.rs::tests` + `server_tests.rs` —
  descriptors, output schema, instructions, status surface lists, failure
  vocabulary, dispatch-edge rejections, and pre-refresh typed failures.
- `crates/ripr/tests/mcp_sdk.rs`, `crates/ripr/tests/mcp_stdio.rs` —
  hosted wire interop controls.

## Implementation Mapping

| Surface | Responsibility |
| --- | --- |
| `crates/ripr/src/mcp/repair_card.rs` | card document projection over the committed snapshot, snapshot commit-time producer binding (head, seam inventory, currentness probes), owner-discriminated seam-to-item binding, refusal wire-spelling mapping |
| `crates/ripr/src/mcp/workspace.rs` | snapshot `findings` and `card_producers` fields; `bind_snapshot_card_producers` inside the one bounded refresh attempt |
| `crates/ripr/src/mcp/protocol.rs` | tool/resource-template descriptors, output schema, instructions, status surface lists, failure vocabulary extension |
| `crates/ripr/src/mcp/server.rs` | dispatch arm and resource read for the card tool/template |
| `docs/interop/mcp.md`, `docs/adr/0022` (Slice D note) | operator-facing surface and authority boundary record |
| `docs/specs/README.md` + `.ripr/traceability.toml` | spec registration and test traceability |

## Metrics

- `mcp_tools` (8) and `mcp_resource_templates` (5) on the slice surface
- typed pre-refresh failure control on the wire for the card tool (target:
  `no_snapshot`)
- zero cards assembled outside the shared `assemble_repair_card` authority
  (negative invariant)
