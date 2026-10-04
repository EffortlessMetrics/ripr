# MCP workspace session server

`ripr mcp` serves RIPR's bounded, read-only workspace session over the
[Model Context Protocol](https://modelcontextprotocol.io/): newline-delimited
JSON-RPC on stdin and stdout. It tells an MCP client which repository root
RIPR would use, what RIPR is and is not allowed to do there, and — after an
explicit `ripr_refresh` — the committed analysis snapshot: a deterministically
bounded working set of canonical items and one complete evidence document per
item. It never edits source, runs tests or mutation, executes verification
commands, or prepares a repair.

The design boundary is [ADR 0022](../adr/0022-mcp-is-a-bounded-projection.md).

## Launch

```bash
ripr mcp --stdio [--root PATH]
```

`--stdio` is the default and only transport. With `--root`, RIPR uses that
exact directory. Without it, RIPR starts at the current directory and walks up
to the nearest directory containing `.git`, falling back to the nearest one
with a project file such as `Cargo.toml`, `package.json`, or `pyproject.toml`.
A directory containing only `.git` (a directory, or a gitfile as in worktrees
and submodules) is a valid repository root: `.git` itself counts as the
repository marker (#3927). Clients often start servers outside the repository,
so pass an absolute `--root` when yours does.

A generic MCP client entry:

```json
{
  "mcpServers": {
    "ripr": {
      "command": "ripr",
      "args": ["mcp", "--stdio", "--root", "/path/to/repo"]
    }
  }
}
```

## Progressive flow

```text
ripr_workspace_status   → root, authority, session facts (no evidence)
ripr_refresh            → one bounded static analysis, commits the snapshot
ripr_list_gaps          → deterministic bounded working set (summaries only)
ripr_get_gap            → one item's complete bounded evidence
ripr_prepare_repair     → readiness-gated in-memory repair transaction
ripr_get_repair_attempt → one session or durable attempt, read-only
ripr_get_receipt_status → one attempt's receipt state, read-only
ripr_get_repair_card    → one item's bounded repair card, read-only
```

Status and list calls never return the full evidence graph; read one item at
a time through the tool or the resource.

## What it exposes

| Surface | Name |
| --- | --- |
| Tool (no arguments) | `ripr_workspace_status` |
| Tool (no arguments) | `ripr_refresh` |
| Tool (`snapshot_id?`) | `ripr_list_gaps` |
| Tool (`gap_id`, `snapshot_id?`) | `ripr_get_gap` |
| Tool (`gap_id`, `snapshot_id?`) | `ripr_prepare_repair` |
| Tool (`attempt_id`) | `ripr_get_repair_attempt` |
| Tool (`receipt_id`) | `ripr_get_receipt_status` |
| Tool (`gap_id`, `snapshot_id?`) | `ripr_get_repair_card` |
| Resource (`application/json`) | `ripr://workspace/status` |
| Resource template | `ripr://snapshot/{snapshot_id}` |
| Resource template | `ripr://gap/{canonical_item_id}` |
| Resource template | `ripr://repair-attempt/{attempt_id}` |
| Resource template | `ripr://receipt/{receipt_id}` |
| Resource template | `ripr://repair-card/{canonical_item_id}` |

`ripr_workspace_status` and `ripr://workspace/status` return the same JSON
document, schema `ripr-mcp-workspace-status-v1`. It wraps:

- the `ripr-workspace-status-v1` workspace block (resolved once at startup,
  never re-resolved): root validation state, discovery source, repository
  markers, a root `error_code` when unavailable, and a hashed host-local
  `identity` — the absolute path is never returned; configuration presence
  (`ripr.toml` is detected, not loaded); `trust` and `authority` facts with
  source edit, verification execution, mutation execution, and model provider
  all `none`;
- a `ripr-mcp-session-v1` session block: the current desired input (workspace
  diff against the default branch, draft mode, built-in defaults), the current
  attempt state (`no_snapshot`, `in_flight`, `completed`, `failed`), the last
  completed snapshot identity, last-known-good state, freshness relative to
  the last completed refresh (`current_at_last_refresh`, or
  `stale_after_failed_attempt` once a later attempt fails),
  the committed snapshot's typed `AnalysisOutcome`, and profile and
  support facts. Complete-zero and incomplete-zero outcomes stay distinct
  through the typed outcome; a partial outcome keeps its typed limitations;
- the transport, tool, resource, resource-template, and byte bounds under
  `mcp`.

`ripr_refresh` runs one bounded static analysis of the workspace diff through
the same shared check authority as `ripr check` and the language server, then
commits the completed snapshot into the session. The call blocks until the
attempt reaches a terminal state and reports it: `completed` with the new
snapshot identity (a `snapshot:sha256:` digest over the typed outcome, the
canonical item identities, and each item's evidence digest — equivalent
roots at equivalent inputs share one portable identity, while the concrete
root evidence stays a separate
host-local hash), `failed` with a typed failure code and bounded detail (the
last-known-good snapshot is kept), `in_flight`, or `workspace_unavailable`.
An attempt runs to a terminal state; cancelling the MCP request never rolls
an attempt back or manufactures a snapshot, and a cancelled or superseded
attempt is never committed. Project-local `ripr.toml` stays
detected-not-loaded: refresh runs with built-in defaults.

`ripr_list_gaps` returns the snapshot's deterministic bounded working set:
`total` / `eligible` / `selected` / `omitted` counts, selected and complete
serialized bytes, every omitted identity with its reason, the
snapshot/profile/budget identity and selection basis, and one small summary
per selected item. Selection is the shared CLI/LSP diagnostic-budget
authority over the snapshot's canonical items; eligibility is the producer's
candidate-actionability predicate captured at projection time, so
non-actionable items appear as disclosed profile-filtered omissions. The
adapter never re-ranks,
never truncates silently, and infers no business risk. Overflow is disclosed
with reasons and the continuation route (`ripr_get_gap`). Pass `snapshot_id`
to bind the read to a specific snapshot: a mismatched identity fails closed
with `stale_snapshot` and the current identity.

`ripr_get_gap` (and the equivalent resource `ripr://gap/{canonical_item_id}`)
returns one canonical item's complete bounded evidence bound to its snapshot
identity: identity and location, the changed behavior (expression,
before/after, delta kind, probe family), causal attribution (canonical gap
owner, behavior kind, probe kind, normalized discriminator), discriminator
availability and the producer's observed/missing evidence, related tests with
oracle kind and strength, and typed limitation states for anything the
producer did not establish. The readiness block reports the committed
producer repair-readiness facts (candidate actionability, an established
discriminator, and a strong directly-related test fix site on a test
surface) with the typed first-failing-gate reason when not ready — this
evidence never authorizes an edit by itself — and the repair boundary is
`none_declared` until `ripr_prepare_repair` binds a session transaction;
then the repair-attempt link names that transaction instead of staying an
explicit `null`. A missing field stays a typed state; MCP never fills it
from prose.

`ripr_prepare_repair` (`gap_id`, optional `snapshot_id`) evaluates those
readiness facts for one canonical item and, only when every gate is
established, creates — or replays — one bounded in-memory repair transaction
bound to the current snapshot, the item, and the root identity. The packet
carries a deterministic `repair-attempt-` identity, the fix site (test file,
line, oracle), an `allowed_edit_surface` limited to that test file, the
edit-cage `must_not_change` statements, stop conditions, the before-evidence
identity, an empty `command_routes` list with the typed limitation (concrete
typed `CommandSpec` routes are published only by the durable CLI before
phase), the shared non-claims, and the resource links. An ineligible item
returns `repair_packet_ready: false` with the typed ineligibility reason and
`attempt: null` — no attempt is created and no field is guessed.

`ripr_get_repair_attempt` (and `ripr://repair-attempt/{attempt_id}`) reads
one transaction by identity: session transactions answer first; otherwise
the durable attempt store of this workspace root is inventoried through the
shared repair-attempt authority and a valid manifest projects its state,
artifact digest bindings, after-phase bindings, and typed `CommandSpec`
routes when the retained packet carries valid ones — each projected exactly,
with the human display string marked as never execution authority. The
host-local root path is intentionally not projected.

`ripr_get_receipt_status` (and `ripr://receipt/{receipt_id}`) projects the
current receipt state for one attempt identity (receipt ids are
attempt-bound) onto the vocabulary `awaiting_edit`, `after_pending`,
`verification_pending`, `improved`, `closed`, `unchanged`, `regressed`,
`limited`, `stale`, `invalid`. Session transactions report `awaiting_edit`
with an explicit `null` receipt; a finished durable attempt with a
digest-bound terminal receipt projects the receipt document with its exact
byte bindings and the movement-derived status. RIPR performs no verification
and issues no receipt: the external client owns the edit, the verification
execution, and the receipt under its own authority.

The `ripr://snapshot/{snapshot_id}` resource returns bounded snapshot
evidence: the snapshot identity, the typed `AnalysisOutcome`, the full
canonical item index (identities and locations, not evidence), and the stored
bounded-selection summary.

`ripr_get_repair_card` (and the equivalent resource
`ripr://repair-card/{canonical_item_id}`) projects the bounded repair card
for one canonical item: the same versioned `repair_card.v1` document `ripr
agent card` and the standard language server project, assembled by the shared
application authority from the committed snapshot — the adapter never
re-derives readiness, identity, currentness, or route state. The analyzed
repository head and the diff-scoped classified seam inventory (each retained
seam owner-discriminated bound to a snapshot item, with its evidence-scope
dirty-state currentness probe) are bound when `ripr_refresh` commits the
snapshot — the adapter never launches git per read — so the card binds the
analyzed head and the commit-time currentness of its snapshot, and edits
after the refresh are visible only after the next one. The durable attempt
store is plain filesystem state and is re-read live at card-read time, so the
card's attempt block matches `ripr agent status` at that moment; in-memory
session transactions never ride a card. The next-action display binds the
portable root `.` and is presentation only, never execution authority.

## Typed failures

The evidence tools fail closed with structured content (`isError: true` and a
`failure` block carrying `code`, bounded `detail`, `recovery`, and small
structured `data`), never with a partial document. The shared vocabulary:

```text
workspace_unavailable  analysis_failed     unsupported_profile
no_snapshot            analysis_in_flight  stale_snapshot
item_not_found         result_too_large    attempt_not_found
config_invalid         workspace_ambiguous static_limitation
cancelled              superseded          attempt_invalid
seam_not_found         policy_omitted      witness_unavailable
identity_unnameable    budget_overflow
```

Before the first successful refresh the evidence tools fail with
`no_snapshot` and the repair-attempt / receipt reads fail with
`attempt_not_found`; `superseded` is reachable for a session transaction
bound to a snapshot that is no longer current; `attempt_invalid` reports a
durable manifest that fails canonical validation; `seam_not_found`,
`identity_unnameable`, and `budget_overflow` are reachable on the repair-card
read (an item no seam owner-discriminated binds, an unnameable portable
workspace identity, a card over its detail budget); `policy_omitted` and
`witness_unavailable` stay named for wire stability even though this adapter
cannot reach them; the rest stay reserved for the slices that own those
states (they are named now so the wire contract
stays stable). Tool argument shape violations use standard Invalid Params;
unknown tools use Method Not Found with the available names in `error.data`.

## What it does not do

It does not edit source, execute verify commands or mutation testing, run
verification or issue receipts, or create durable repair attempts (durable
attempt creation stays CLI-owned; the adapter's session transactions are
in-memory and its durable-store reads are read-only). A prepared transaction
never authorizes an edit, a command, or a merge by itself — the external
client's approval and sandbox policy remains authoritative. It does not
load project-local configuration or providers, embed a model, or offer a
remote transport. It does not watch the worktree: the snapshot is current as
of its completed `ripr_refresh`, so refresh again after edits. The session
is in-memory; restarting the server drops the snapshot and every session
transaction unless a new refresh commits one.

An invalid root does not stop the server. Status reports
`workspace_state: "unavailable"` with a `root.error_code`, and the tool result
adds a second text content item that names the cause and the recovery
(restart with `--root <repository>`). An unknown static resource name is
rejected with the one valid static URI in the message and in
`error.data.available`; the snapshot and gap templates reject unknown or
stale identities with their typed codes in `error.data`. A client that
negotiated an older protocol version gets resource-not-found (`-32002`) for a
resource miss; current clients get Invalid Params (`-32602`). The SDK maps
the code, while the adapter's bounded message
`unknown resource; available: ripr://workspace/status` and
`error.data.available` name the same valid URI in both lifecycles.
The instructions (returned by both `initialize` and `server/discover`) and the
tool descriptions state positively what each tool returns, its bounds, and
what it does not do. Protocol errors keep standard JSON-RPC codes. The pinned
official Rust SDK owns negotiation, dispatch, correlation and cancellation.
Syntax-invalid JSON is ignored; well-formed messages with invalid typed
shapes receive Invalid Request and the transport can read the next frame.
Unknown request IDs are omitted in SDK error responses; readable IDs remain
correlated. Messages are capped at 256 KiB and responses, including their
delimiter, at 128 KiB; the bound is enforced on the final serialized
envelope — the document is measured again after the tool or resource wrapper
adds its text and structured-content representations — so an over-bound
response fails closed with `result_too_large` before the wire cap is
reached. If even a
correlated fallback cannot fit its readable ID, the service terminates with
the bounded stderr reason `MCP output limit`, without substituting an ID.
Partial reads and writes retain their state across cancellation. EOF closes
the SDK service, so scripted clients must await replies before closing stdin.
Stdout carries only protocol messages; operational errors go to stderr, and
invalid command-line arguments exit with status 2.

Supported protocol versions are `2024-11-05`, `2025-03-26`, `2025-06-18`,
`2025-11-25`, and `2026-07-28`. A client can open with `initialize`, where an
unsupported or discovery-only requested version is answered with
`2025-11-25`, or with `server/discover`, where every request carries
`io.modelcontextprotocol/protocolVersion` and
`io.modelcontextprotocol/clientCapabilities` in `params._meta` and an
unsupported version is refused.
