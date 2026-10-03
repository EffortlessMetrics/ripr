# RIPR-SPEC-0214: MCP repair transaction projection — prepare, attempt reads, and receipt status

Status: proposed

Owner: mcp

Created: 2026-10-03

Linked issues:

- #3090 (this slice: `ripr_prepare_repair`, `ripr_get_repair_attempt`,
  `ripr_get_receipt_status`, and the repair-attempt / receipt resources)
- #3087 (parent standard MCP server epic)
- #3088 (transport/discovery/read-only boundary slice; this slice keeps its
  SDK-owned dispatch and bounded framing)
- #3089 / RIPR-SPEC-0211 (sibling session-evidence slice; this slice
  supersedes its hard `repair_packet_ready: false` readiness pin with the
  committed producer repair-readiness evaluation and owns binding the
  repair boundary and repair-attempt link)
- #4668 (RepairCard MCP projection consumer; the seam list this slice
  exposes lets that work proceed)
- ADR 0022 (bounded read-only MCP adapter; this slice adds no execution
  authority)

Support-tier impact:

- None. The slice projects producer facts the CLI and LSP already compute;
  it adds no language support claim and runs no new analysis.
  [docs/status/SUPPORT_TIERS.md](../status/SUPPORT_TIERS.md)

Policy impact:

- None. No new output format, gate, badge, or report file is added; the
  repair documents are wire payloads bounded by the existing MCP byte caps.
  The wire failure vocabulary gains two typed codes (`attempt_not_found`,
  `attempt_invalid`); the reserved `superseded` code becomes reachable for
  session transactions bound to a non-current snapshot.

## Problem

An agent that found one actionable gap through the session-evidence slice
(RIPR-SPEC-0211) still had to leave the protocol to assemble a bounded
repair packet: no route named the committed readiness facts, no transaction
identity bound the evidence to one snapshot and item, and the durable
CLI-owned repair attempts and receipts were invisible to the adapter. Slice
B deliberately pinned readiness as a hard negative and left the
repair-attempt link an explicit null so this slice could extend the
documents without a breaking change.

## Behavior

`ripr mcp --stdio` gains three tools and two resource templates, all
read-only and without execution authority (ADR 0022):

- Tools `ripr_prepare_repair` (inputs `gap_id`, optional `snapshot_id`),
  `ripr_get_repair_attempt` (input `attempt_id`), and
  `ripr_get_receipt_status` (input `receipt_id`); resource templates
  `ripr://repair-attempt/{attempt_id}` and `ripr://receipt/{receipt_id}`.
  Tool and template names follow the registry established in #3088.
- `ripr_prepare_repair` evaluates the committed producer repair-readiness
  facts on one canonical item of the current completed snapshot. The
  evaluation is a fail-closed conjunction computed at snapshot commit time:
  candidate actionability (`#3281` predicate), an established discriminator
  (non-empty normalized discriminator, no producer-named missing
  discriminator), and an established fix site (a strong-oracle,
  high-confidence directly-related test on a shared edit-cage test-surface
  path). The first failing gate is the typed reason (`not_candidate_actionable`,
  `missing_discriminator`, `fix_site_not_established`,
  `fix_site_not_test_surface`).
- When every gate is established, the tool creates one session repair
  transaction with a deterministic, root-bound `repair-attempt-` identity
  (the first 24 hex digits of a SHA-256 over schema version, snapshot id,
  canonical id, and root identity), an `awaiting_edit` state, the fix-site
  bounded packet (allowed edit surface limited to the test file,
  `must_not_change` edit-cage statements, stop conditions, before-evidence
  identity, an empty `command_routes` list with the typed limitation that
  concrete `CommandSpec` routes are published only by the durable CLI
  before phase), the shared non-claims, and resource links. Repeating the
  call for the same current snapshot, item, and root returns the identical
  document and never creates a second transaction. The transaction is
  in-memory like the snapshot: a restart drops it.
- When a gate is not established, the tool returns an
  `isError: false` document with `repair_packet_ready: false`, the typed
  `ineligibility.reason`, and `attempt: null` — never a failure envelope,
  never a created attempt, never a guessed field.
- `ripr_get_repair_attempt` and the `ripr://repair-attempt/{attempt_id}`
  resource read one transaction by identity: session transactions answer
  first; otherwise the durable attempt store of this session's root is
  inventoried through the shared repair-attempt authority and a valid
  manifest projects its state, repository head, seam identity, artifact
  digest bindings, follow-up command display string, typed `CommandSpec`
  routes when the retained packet carries valid ones (each projected
  exactly, with the human display marked as never execution authority and
  `shell_required` / `manual` modes visibly non-direct), limitations,
  non-claims, and after-phase bindings. The host-local root path is
  intentionally not projected (ADR 0022 hashing posture).
- `ripr_get_receipt_status` and the `ripr://receipt/{receipt_id}` resource
  project the current receipt state for one attempt identity (receipt ids
  are attempt-bound). The status vocabulary is `awaiting_edit`,
  `after_pending`, `verification_pending`, `improved`, `closed`,
  `unchanged`, `regressed`, `limited`, `stale`, `invalid`; durable manifest
  states and the shared receipt-lifecycle vocabulary map onto it, and a
  finished attempt with a digest-bound terminal receipt projects the
  receipt document with its exact byte bindings. Session transactions
  report `awaiting_edit` with an explicit null receipt: RIPR performs no
  verification and issues no receipt, so the external client owns the edit,
  the verification execution, and the receipt under its own authority.
- Fail-closed states: before the first successful refresh,
  `ripr_prepare_repair` fails with `no_snapshot`; the two reads route
  through the durable store and fail with `attempt_not_found`. During an
  attempt the tools report `analysis_in_flight`. An unknown item fails with
  `item_not_found`; an unknown attempt or receipt identity fails with
  `attempt_not_found`; a durable manifest that fails canonical validation
  fails with `attempt_invalid`; a session transaction bound to a snapshot
  that is no longer current fails with the reserved `superseded` state and
  the current snapshot identity. A document over the response bound fails
  with `result_too_large`. Unknown tool arguments are rejected at the
  dispatch edge with `INVALID_PARAMS`.
- `ripr_get_gap`'s readiness block now reports the committed producer
  repair-readiness facts, and a live session transaction binds the item's
  `repair_attempt` link (removing the reservation note); without a
  transaction the link stays an explicit null.

## Required Evidence

- Unit controls over session transactions: the complete ready route
  creates exactly one replay-resistant attempt; missing producer facts
  produce the honest negative document and no attempt; stale and unknown
  items fail closed; attempt identity is root-bound; a refresh that changes
  the evidence supersedes the prior transaction; session receipt status
  stays `awaiting_edit`; unknown and canonically invalid durable attempts
  fail closed in a temporary store.
- Vocabulary and projection controls: the receipt-status mapping over the
  shared receipt-lifecycle states; `CommandSpec` projection round-trips
  direct, shell-required, and manual modes without granting display
  authority, and a removal experiment shows a spaced display argument
  cannot be reconstructed from the display string.
- Wire controls: the SDK session discovers all seven tools and four
  templates; the stdio fail-closed control pins the typed pre-refresh
  failures for the new tools; the inline-version recovery control expects
  the seven-tool surface.

## Non-Goals

- No source edit, test authoring, process launch, or verification or
  mutation command execution; the external client's approval and sandbox
  policy remains authoritative for every command a returned route names.
- No durable attempt creation: `begin_repair_attempt_with_identity` stays
  CLI-owned (`ripr agent repair --phase before`); MCP reads the durable
  store read-only and never writes it.
- No command-route reconstruction from display prose; a route that does
  not validate as a typed `CommandSpec` stays unprojected with an honest
  limitation.
- No receipt issuance; receipt state binds producer-retained bytes and is
  re-validated on every read, never joined by mtime or latest-file
  convention.
- No seam-pipeline target-admission rerun; the fix site is the strongest
  producer test grip, not an admitted seam target.
- No custom LSP request expansion; MCP and LSP remain peers over shared
  producers.
- No provider-specific configuration; project-local `ripr.toml` stays
  detected-not-loaded.
- No support-tier promotion.

## Acceptance Examples

1. A stock MCP client refreshes, lists gaps, and calls
   `ripr_prepare_repair` for one ready item twice: both calls return the
   identical bounded packet with one `repair-attempt-` identity, an
   `awaiting_edit` state, the test-file-only allowed edit surface, empty
   `command_routes` with the typed limitation, and the resource links; the
   item's `ripr_get_gap` document now binds the same repair-attempt link.
2. The same client prepares an ineligible item (missing discriminator) and
   receives `repair_packet_ready: false`, the typed ineligibility reason,
   and `attempt: null` — with no attempt discoverable afterwards.
3. `ripr_get_repair_attempt` for a durable CLI attempt of the same root
   projects the manifest state, artifact digest bindings, and typed
   `CommandSpec` routes with `display_is_execution_authority: false`; a
   garbage manifest fails closed with `attempt_invalid`.
4. `ripr_get_receipt_status` for a finished durable attempt with a retained
   terminal receipt projects the movement-derived status
   (`improved`/`unchanged`/`closed`/…) with the receipt's exact byte
   bindings; a session transaction reports `awaiting_edit` with an explicit
   null receipt.
5. Before any refresh, `ripr_prepare_repair` fails closed with
   `no_snapshot` and the two reads fail closed with `attempt_not_found`; a
   refresh that changes the evidence fails an old transaction with the
   reserved `superseded` state and the current snapshot identity.

## Test Mapping

- `crates/ripr/src/mcp/repair.rs::tests` — session transaction route,
  ineligibility, fail-closed states, root-bound identity, supersession,
  receipt status, durable-store negatives, vocabulary mapping, and
  `CommandSpec` projection experiments.
- `crates/ripr/src/mcp/gaps.rs::tests` — the committed producer
  repair-readiness evaluation and its fail-closed gates.
- `crates/ripr/src/mcp/protocol.rs::tests` + `server_tests.rs` —
  descriptors, vocabulary, instructions, dispatch-edge rejections, and
  pre-refresh typed failures.
- `crates/ripr/tests/mcp_sdk.rs`, `crates/ripr/tests/mcp_stdio.rs` —
  hosted wire interop controls.

## Implementation Mapping

| Surface | Responsibility |
| --- | --- |
| `crates/ripr/src/mcp/repair.rs` | readiness-gated session transactions, deterministic root-bound attempt identity, durable attempt/receipt projection, `CommandSpec` document projection, receipt-status vocabulary mapping |
| `crates/ripr/src/mcp/gaps.rs` | producer repair-readiness evaluation at snapshot commit time; readiness block and live repair-attempt link in the evidence document |
| `crates/ripr/src/mcp/workspace.rs` | session repairs map (in-memory, restart-drops), shared `AttemptFailure::with_data`, link injection in `get_gap` |
| `crates/ripr/src/mcp/protocol.rs` | tool/resource-template descriptors, output schemas, instructions, status surface lists, failure vocabulary |
| `crates/ripr/src/mcp/server.rs` | dispatch arms and resource reads for the three tools and two templates |
| `docs/interop/mcp.md`, `docs/adr/0022` (Slice C note) | operator-facing surface and authority boundary record |
| `docs/specs/README.md` + `.ripr/traceability.toml` | spec registration and test traceability |

## Metrics

- `mcp_tools` (7) and `mcp_resource_templates` (4) on the slice surface
- typed pre-refresh failure controls on the wire for the three new tools
  (target: prepare → `no_snapshot`; attempt/receipt reads →
  `attempt_not_found`)
- zero created attempts for ineligible items (negative invariant)
