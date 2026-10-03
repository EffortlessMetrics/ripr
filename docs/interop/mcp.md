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
| Resource (`application/json`) | `ripr://workspace/status` |
| Resource template | `ripr://snapshot/{snapshot_id}` |
| Resource template | `ripr://gap/{canonical_item_id}` |

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
producer did not establish. Readiness is always `repair_packet_ready: false`
with its reason — this evidence never authorizes an edit — and the repair
boundary is `none_declared`, prepared by the repair slice (#3090); the
repair-attempt link is an explicit `null`. A missing field stays a typed
state; MCP never fills it from prose.

The `ripr://snapshot/{snapshot_id}` resource returns bounded snapshot
evidence: the snapshot identity, the typed `AnalysisOutcome`, the full
canonical item index (identities and locations, not evidence), and the stored
bounded-selection summary.

## Typed failures

The evidence tools fail closed with structured content (`isError: true` and a
`failure` block carrying `code`, bounded `detail`, `recovery`, and small
structured `data`), never with a partial document. The shared vocabulary:

```text
workspace_unavailable  analysis_failed     unsupported_profile
no_snapshot            analysis_in_flight  stale_snapshot
item_not_found         result_too_large
config_invalid         workspace_ambiguous static_limitation
cancelled              superseded
```

The first seven are reachable in this slice; the rest are reserved for the
slices that own those states (they are named now so the wire contract stays
stable). Tool argument shape violations use standard Invalid Params; unknown
tools use Method Not Found with the available names in `error.data`.

## What it does not do

It does not edit source, execute verify commands or mutation testing, prepare
or create a repair transaction (that is #3090's slice; readiness here is
always a hard negative), load project-local configuration or providers, embed
a model, or offer a remote transport. It does not watch the worktree: the
snapshot is current as of its completed `ripr_refresh`, so refresh again
after edits. The session is in-memory; restarting the server drops the
snapshot unless a new refresh commits one.

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
