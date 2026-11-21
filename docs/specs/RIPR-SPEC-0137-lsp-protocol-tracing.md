# RIPR-SPEC-0137: LSP Protocol Tracing

Status: accepted

Owner: product / swarm

Created: 2026-07-22

Linked issues:

- [#2035](https://github.com/EffortlessMetrics/ripr-swarm/issues/2035) -
  implement redacted `$/setTrace` / `$/logTrace` protocol tracing.

Support-tier impact:

- No tier change. This spec adds one session-local observability lifecycle.
  It does not change any classification, finding set, ExposureClass, probe
  family, confidence score, `repair_packet_ready` authority, output schema
  version, or pass/fail behavior. Traces are observability, never a semantic
  or evidence authority.
- Claim boundaries remain governed by the canonical ledger in
  [support tiers](../status/SUPPORT_TIERS.md).

Policy impact:

- No new crates, binaries, dependencies, parsers, runtime executors, or LSP
  servers introduced by this spec.

## Problem

The LSP server exposes ordinary `window/logMessage` logs and a custom
`ripr/analysisStatus` notification, but no governed implementation of the
standard LSP trace lifecycle (`$/setTrace` with `off | messages | verbose`,
and `$/logTrace`). Generic clients get weaker diagnostics, and ad hoc
debugging could tempt future code to log full request/response payloads
containing source, paths, configuration, repair packets, or command
arguments.

## Behavior

### Trace state

One session-local value: `off | messages | verbose`, default `off`. The
standard `InitializeParams.trace` value is honored at `initialize`; an
omitted value leaves `off`, and an unknown value fails typed
initialize-param parsing per the library contract.

tower-lsp-server has no native `$/setTrace` handler and silently drops
unregistered notifications, so `$/setTrace` is registered through
`LspServiceBuilder::custom_method` with untyped (`LSPAny`) params: a typed
params parse failure would be dropped silently, while manual validation
makes the rejection observable. `$/setTrace` updates the state immediately
and never triggers analysis, refresh, configuration reload, or source
access. An unknown or malformed value is rejected without crashing the
session: the current state is kept, and the rejection is observable through
`$/logTrace` (naming the rejection class `unknown_value`) whenever tracing
is enabled. The client-provided value is never reflected verbatim.

Trace state is volatile: it never enters snapshot, input-identity,
diagnostic, action, continuation, command, status, or receipt state, and it
is never read from refresh or identity paths. A trace toggle must not
advance the workspace revision, reschedule analysis, or change any status
payload field (including the #2031 `configuration_pull` disclosure).

### Redacted trace output

Emission is structurally redacted by construction — there is no payload
scrubber because params never enter the trace as free text:

- `messages`: direction (`<-` inbound / `->` outbound), method name, and
  message class (`request` / `notification` / `response` / `error`).
- `verbose`: adds bounded numeric metadata only — `params_bytes=<n>` for
  inbound messages, `outcome=ok response_bytes=<n>` or
  `outcome=error code=<n>` for outbound results. Params are serialized only
  to count bytes; the serialized form is dropped immediately.

Always omitted: source text and unsaved document content, document URIs and
paths, diagnostic/hover/packet prose, configuration values and environment,
command arguments and display strings, URLs, tokens, credentials, and
arbitrary client-provided JSON.

### `$/logTrace` behavior

- The standard method name `$/logTrace` (no space) is used; emission goes
  through the typed `notification::LogTrace` params (`message`, optional
  `verbose`).
- Trace emission is fire-and-forget and non-fatal: it cannot block analysis
  or shutdown, and the transport suppresses it while the session is not
  initialized (so the `initialize` handshake itself is never traced).
- Trace lifecycle notifications (`$/setTrace`, `$/logTrace`) are never
  themselves traced, so the trace channel cannot recurse.
- The hook point is per-handler emission at the `LanguageServer` method
  dispatch (a `trace_inbound` call at the top of each trait method and a
  `trace_response` call wrapping each request result), not a tower `Layer`:
  it is explicit, testable, and keeps the diff bounded.

## Required Evidence

- `trace: Mutex<TraceValue>` session state on `Backend`, default `off`,
  beside `pull_diagnostics`; `initialize` honors `InitializeParams.trace`.
- `Backend::set_trace` custom-method handler with manual validation and
  observable rejection; registered in the shared `build_service`
  constructor used by both `serve_streams` and the framed duplex tests.
- `trace_inbound` / `trace_response` / `emit_log_trace` helpers emitting
  only the structural fields above.
- Framed duplex tests: off emits nothing; messages emits
  method/direction/class only; verbose adds bounded numeric metadata;
  runtime transitions round-trip; a source-text canary and document URIs
  never appear in any `$/logTrace`; an unknown value is rejected observably
  with the session alive and the state unchanged; `initialize` with
  `"trace": "verbose"` enables tracing without any `$/setTrace`.
- In-process tests: state transitions and rejection classes; a trace toggle
  leaves the workspace-status payload (input-identity and
  `configuration_pull` disclosure) and the workspace revision untouched.

## Non-Goals

- A payload scrubber or free-text trace content of any kind.
- Tracing outbound server notifications (publishDiagnostics, analysisStatus,
  progress); only inbound messages and outbound request results are traced.
- Editor extension changes; the client's trace channel is out of scope.
- A closed per-message-family trace disposition manifest (#1995's surface
  manifest has not landed; this spec's emission contract covers every
  current handler uniformly instead).
- Trace sinks other than the standard `$/logTrace` notification.

## Acceptance Examples

### Off emits nothing

```text
Given a session with no trace selection,
when the client sends didOpen and hover,
then no $/logTrace notification is emitted.
```

### messages traces method, direction, and class only

```text
Given trace = messages,
when the client sends didOpen with source text and a hover request,
then $/logTrace entries name textDocument/didOpen and textDocument/hover
with direction and message class,
and no entry carries verbose detail, source text, or a document URI.
```

### verbose adds bounded numeric metadata

```text
Given trace = verbose,
when the client sends a hover request,
then the inbound trace adds params_bytes=<n>
and the response trace adds outcome=ok response_bytes=<n>,
and still no source text, path, or URI appears.
```

### Unknown values are rejected observably without crashing

```text
Given trace = verbose,
when the client sends $/setTrace with value "everything",
then the state stays verbose,
a $/logTrace names the rejection class unknown_value,
and the next request is still answered.
```

### Trace toggles never touch analysis state

```text
Given a running session,
when the client toggles trace off -> messages -> verbose -> off,
then the workspace revision does not advance,
and the workspace status payload is byte-identical across the toggle.
```

## Test Mapping

- `crates/ripr/src/lsp/tests.rs::lsp_trace_set_trace_updates_state_and_rejects_unknown_values`
  — state round trip plus unknown/malformed-value rejection classes.
- `crates/ripr/src/lsp/tests.rs::lsp_trace_initialize_honors_client_trace_value`
  — `InitializeParams.trace` honored; omitted means `off`.
- `crates/ripr/src/lsp/tests.rs::lsp_trace_toggle_leaves_status_identity_and_revision_untouched`
  — a trace toggle changes neither the workspace status payload (including
  input-identity and `configuration_pull` fields) nor the workspace
  revision.
- `crates/ripr/src/lsp/tests.rs::framed_lsp_trace_lifecycle_and_redaction`
  — framed duplex lifecycle: off emits nothing; messages emits
  method/direction/class only; verbose adds bounded numeric metadata;
  source-text canary and `file://` URIs never appear; unknown value is
  rejected observably with the session alive and the state unchanged.
- `crates/ripr/src/lsp/tests.rs::framed_lsp_trace_initialize_trace_param_enables_tracing`
  — `"trace": "verbose"` at initialize enables tracing without any
  `$/setTrace`.
- `crates/ripr/src/lsp/tests.rs::serve_stdio_call_presence_observer` —
  the `$/setTrace` custom-method registration is pinned in the shared
  service constructor.

## Implementation Mapping

- `crates/ripr/src/lsp/backend.rs` — `trace` session state,
  `Backend::set_trace`, `trace_inbound` / `trace_response` /
  `emit_log_trace`, per-handler emission at every `LanguageServer` trait
  method, `initialize` trace honoring.
- `crates/ripr/src/lsp.rs` — shared `build_service` constructor registering
  `$/setTrace` as a custom method, used by `serve_streams` and the framed
  tests.

## Metrics

- Unit and integration tests listed above pass under `cargo test -p ripr`.
- `cargo xtask goldens check` remains clean: the change is LSP-only and
  additive; it touches no CLI goldens.
- `cargo xtask check-static-language` clean: trace and disclosure strings
  use conservative language only.
