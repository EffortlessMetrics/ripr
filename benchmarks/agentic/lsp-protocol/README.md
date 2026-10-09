# B4: LSP agent-protocol benchmark

Drives the real `ripr lsp --stdio` binary over JSON-RPC with
`Content-Length` framing and pins the agent-protocol wire contract:

- `initialize` advertises the `riprAgent` experimental block
  (`protocol_version` 0.1, `schema_version` 0.2) and the
  `executeCommandProvider` collect surface;
- `ripr/listActionableItems` is the one live `riprAgent` request;
  reserved requests stay rejected;
- lifecycle codes: pre-init `-32002`, duplicate `initialize` `-32600`,
  bad collect arguments `-32602`, malformed frames `-32700`;
- an unsupported client `riprAgent` major is rejected fail-closed:
  `initialize` still succeeds and the server keeps advertising major 0;
- every step lands in a bounded receipt, and malformed frames never
  hang the server.

## Layout

- `script.json`: the wire script the harness executes (initialize
  params, version matrix, bad-argument cases, journey arguments,
  malformed frames, byte bounds).
- Harness: `crates/ripr/tests/agentic_bench_lsp.rs`.
- Receipt (git-ignored test evidence):
  `target/ripr/reports/agentic-bench-lsp-receipt.json`.

Sources: `crates/ripr/src/lsp/agent_protocol.rs` (version gate,
request vocabulary), `crates/ripr/src/lsp/backend.rs` (collect
commands), `crates/ripr/tests/lsp_lifecycle.rs` (framing oracle).
