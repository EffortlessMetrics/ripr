# ADR 0022: MCP is a bounded projection over shared RIPR authority

- Status: Accepted
- Date: 2026-08-27
- Related: #1599, #3087, #3088, #3094

## Context

RIPR already has CLI, LSP, report, and private agent-protocol surfaces. A public
Model Context Protocol server must not become another place that discovers
roots differently, interprets evidence, chooses repairs, or acquires execution
or edit authority. It also has to interoperate with clients on the legacy
`initialize` lifecycle and clients using the 2026-07-28 `server/discover`
lifecycle.

The official Rust MCP SDK is the long-term transport target. Adding it in the
first slice without regenerating and reviewing `Cargo.lock`, running the SDK
conformance suite, and preserving RIPR's transport-neutral authority boundary
would make dependency adoption the dominant change rather than the protocol
slice.

## Decision

`ripr mcp --stdio` is a newline-delimited JSON-RPC adapter over a shared,
transport-neutral workspace-status producer. Binary startup selects this
protocol lane before the general human-oriented CLI dispatcher so no generic
help, rendering, or diagnostic path can write to protocol stdout.

The first slice:

- supports legacy `initialize` / `notifications/initialized` sessions and the
  2026-07-28 `server/discover` lifecycle;
- exposes one read-only tool, `ripr_workspace_status`, and one equivalent
  resource, `ripr://workspace/status`;
- validates and canonicalizes the selected repository root, then emits only a
  hashed host-local root identity rather than an absolute path;
- detects `ripr.toml` but does not load project-local configuration through the
  transport;
- declares no source-edit, verification-execution, mutation-execution, or model
  provider authority;
- bounds request and response bytes and keeps stdout protocol-only; and
- keeps MCP method dispatch, framing, and lifecycle state in the adapter while
  root, trust, configuration, and authority facts remain outside it.

The adapter uses the repository's existing Tokio and Serde dependencies for
this narrow slice. The official Rust SDK replaces the local wire adapter when a
single reviewed change can regenerate the lockfile, run SDK conformance, retain
the same shared status producer, and demonstrate that no product semantics
moved into the transport.

## Consequences

MCP clients gain a live, standards-shaped discovery and status surface without
getting repair, analysis refresh, source editing, command execution, mutation
execution, remote transport, secrets, or model-provider configuration.

The local wire code is intentionally small and fixture-pinned. Protocol growth
beyond this status slice raises the SDK migration trigger rather than expanding
a parallel framework. LSP and MCP remain peers over shared RIPR authority; one
transport must not invoke or reinterpret the other.
