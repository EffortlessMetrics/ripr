# ADR 0007: LSP Server Framework

Status: accepted

Date: 2026-05-02

## Context

`ripr lsp --stdio` started as a small experimental sidecar with hand-rolled
JSON-RPC and LSP framing. It manually parsed `Content-Length`, matched methods
with string searches, extracted JSON-RPC IDs by slicing raw JSON, and formatted
responses and diagnostics with string builders.

That was acceptable for the first smoke path, but Campaign 4 is expected to add
editor and agent features such as evidence diagnostics, hover details, context
commands, document state, refresh behavior, and eventually background analysis.
Protocol plumbing is not product work, and keeping custom JSON-RPC transport in
place would make those features harder to review.

## Decision

Use `tower-lsp-server` for the LSP sidecar.

Do not use the original `tower-lsp`, vendor it, or migrate through a temporary
synchronous `lsp-server` loop.

Keep the framework dependency in the published `ripr` package for now. The
project still has one public package and internal module seams; there is not yet
a separate external contract that justifies a `ripr-lsp` crate.

## Consequences

Positive:

- LSP requests and notifications are handled by typed async methods.
- Protocol framing and JSON-RPC dispatch come from a maintained LSP framework.
- `ripr lsp --stdio` can grow into background diagnostics, hover, code actions,
  and command handling without another transport migration.
- Analyzer work can remain synchronous behind adapter functions and
  `spawn_blocking` instead of forcing the analysis layer to become async.

Negative:

- The CLI now depends on Tokio and the Tower LSP stack.
- The sidecar has a larger dependency graph than the initial hand-rolled smoke
  loop.
- LSP behavior still needs focused fixture and editor smoke coverage as the
  evidence fields mature.
