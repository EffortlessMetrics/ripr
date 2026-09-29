# MCP workspace status server

`ripr mcp` serves a bounded, read-only RIPR workspace status over the
[Model Context Protocol](https://modelcontextprotocol.io/): newline-delimited
JSON-RPC on stdin and stdout. It tells an MCP client which repository root RIPR
would use and what RIPR is and is not allowed to do there. It does not analyze
code, name gaps, or repair anything; use `ripr check` and `ripr agent repair`
for that.

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

## What it exposes

| Surface | Name |
| --- | --- |
| Tool (no arguments) | `ripr_workspace_status` |
| Resource (`application/json`) | `ripr://workspace/status` |

Both return the same JSON document, schema `ripr-mcp-workspace-status-v1`,
which wraps a `ripr-workspace-status-v1` workspace block. The status is
resolved once at process startup and held for the life of the server — it is
a static snapshot, not a live view, and nothing re-resolves it:

- `workspace_state`: `ready` or `unavailable`;
- `root`: validation `state`, `source` (`explicit`, `current_directory`,
  `ancestor_discovery`, or `unavailable`), detected repository markers, an
  `error_code` when unavailable (for example `root_missing`), and a hashed
  host-local `identity`. The absolute path is never returned;
- `configuration.project_config_state`: whether a `ripr.toml` was detected.
  It is detected, not loaded;
- `trust` and `authority`: `read_only_status` access, with source edit,
  verification execution, mutation execution, and model provider all `none`;
- `claim_boundary` and `limitations`, as plain text. The `ripr.toml`
  limitation appears only when a `ripr.toml` was detected;
- the transport, tool, resource, and byte bounds under `mcp`.

Supported protocol versions are `2024-11-05`, `2025-03-26`, `2025-06-18`,
`2025-11-25`, and `2026-07-28`. A client can open with `initialize`, where an
unsupported or discovery-only requested version is answered with `2025-11-25`, or with
`server/discover`, where every request carries
`io.modelcontextprotocol/protocolVersion` and
`io.modelcontextprotocol/clientCapabilities` in `params._meta` and an
unsupported version is refused.

## What it does not do

It does not run analysis or refresh evidence, edit source, execute verify
commands or mutation testing, load project-local configuration or providers,
embed a model, or offer a remote transport.

An invalid root does not stop the server. Status reports
`workspace_state: "unavailable"` with a `root.error_code`, and the tool result
adds a second text content item that names the cause and the recovery
(restart with `--root <repository>`). An unknown tool or resource name is
rejected with the one valid name in the message and in `error.data.available`.
A client that negotiated an older protocol version gets resource-not-found
(`-32002`) for an unknown resource; current clients get Invalid Params
(`-32602`). The SDK maps the code, while the adapter's bounded message
`unknown resource; available: ripr://workspace/status` and
`error.data.available` name the same valid URI in both lifecycles.
The instructions (returned by both `initialize` and `server/discover`) and the
tool description say that this server
does not analyze the diff and name the CLI route that does
(`ripr check --format json`, `ripr pilot --root .`); naming a route executes
nothing. Protocol errors keep
standard JSON-RPC codes. The pinned official Rust SDK owns negotiation,
dispatch, correlation and cancellation. Syntax-invalid JSON is ignored;
well-formed messages with invalid typed shapes receive Invalid Request and
the transport can read the next frame. Unknown request IDs are omitted in SDK
error responses; readable IDs remain correlated. Messages are capped at
256 KiB and responses, including their delimiter, at 128 KiB. If even a
correlated fallback cannot fit its readable ID, the service terminates with
the bounded stderr reason `MCP output limit`, without substituting an ID.
Partial reads and writes retain their state across cancellation. EOF closes
the SDK service, so scripted clients must await replies before closing stdin.
Stdout carries only protocol messages; operational errors go to stderr,
and invalid command-line arguments exit with status 2.
