# ADR 0022: MCP is a bounded projection over shared RIPR authority

- Status: Accepted
- Date: 2026-08-27
- Related: #1599, #3087, #3088, #3094, #3089 (slice B: status, refresh,
  bounded gap lists, and evidence resources)

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

This section records the original pre-SDK slice. Its local dispatch/lifecycle
and dependency decisions are superseded by the SDK migration section below;
the static shared-status and authority boundaries remain current.

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

The local-wire discussion below describes that original slice. The successor
uses SDK-owned dispatch/lifecycle and RIPR-owned bounded framing and IO.

MCP clients gain a standards-shaped discovery and status surface without
getting repair, analysis refresh, source editing, command execution, mutation
execution, remote transport, secrets, or model-provider configuration. The
workspace status is resolved once at process startup and never re-resolved:
it is a static snapshot for the life of the server, not a live view.

The local wire code is intentionally small and fixture-pinned. Protocol growth
beyond this status slice raises the SDK migration trigger rather than expanding
a parallel framework. LSP and MCP remain peers over shared RIPR authority; one
transport must not invoke or reinterpret the other.

## SDK migration (#3088)

The status-only successor uses pinned official `rmcp 3.5.0` with default
features disabled. SDK `ServerHandler` owns dispatch and lifecycle; RIPR's
adapter only maps the existing static WorkspaceStatus to one tool/resource.
The product transport retains persistent bounded input/discard and output
cursor state, while the SDK public codec owns typed wire parsing. No HTTP,
authentication, analysis authority or project-configuration loading is added.

The server feature adds the following locked transitive packages. Their
published manifest declarations fit the repository's Rust 1.95 and
MIT/Apache-2.0 license posture; this is not a vulnerability-audit result.
Cargo generated the lockfile, and dependency-only `cargo fetch --locked`
completed after resolving the new graph.

| Package | Declared license | Declared minimum Rust |
| --- | --- | --- |
| dyn-clone 1.0.20 | MIT OR Apache-2.0 | 1.60 |
| pastey 0.2.3 | MIT OR Apache-2.0 | 1.54 |
| schemars 1.2.1 | MIT | 1.74 |
| schemars_derive 1.2.1 | MIT | 1.74 |
| serde_derive_internals 0.29.1 | MIT OR Apache-2.0 | 1.56 |
| uuid 1.26.0 | Apache-2.0 OR MIT | 1.85.0 |

Compatibility follows the SDK: unsupported or discovery-only initialize
versions negotiate `2025-11-25`; syntax-invalid JSON is ignored, invalid typed
shapes recover with Invalid Request, and unknown error IDs may be omitted.
Readable IDs are never silently replaced: a response/fallback that cannot fit
the 128-KiB output cap terminates with a bounded operational error. The raw
client harness waits for correlated replies before EOF, rather than assuming
prewritten requests remain active after SDK shutdown.

The CLI stdio invocation owns its process session. After the SDK service has
terminated, it preserves the service result and shuts down its owned runtime
without waiting for Tokio's non-cancellable blocking stdin read. This permits
an operational error to reach process exit while the client retains stdin.
It does not join that blocking worker or qualify repeated embedded sessions;
the public argument-taking entry point adds no embedding lifecycle promise.

The SDK maps resource-miss codes by negotiated version and preserves the
adapter's message and data. Both lifecycles now use the bounded message
`unknown resource; available: ripr://workspace/status` and the available URI
data. This replaces the original adapter's legacy-only `Resource not found`
message without maintaining an application version gate or dispatcher.

The pre-migration 21 local controls have explicit dispositions below. This
mapping specifies successor proof; it does not claim that it has run.

| Prior control | Successor owner / disposition |
| --- | --- |
| CLI argument rejection | Existing `mcp::tests::parser_rejects_unknown_and_ambiguous_root_arguments`, unchanged |
| Invalid JSON produces parse error | SDK syntax-ignore policy; typed-shape/recovery transport control replaces manual parse-error assertion |
| Unsupported initialize chooses default | Official client initialize-version control; handshake default corrected to SDK `LATEST_WITH_INITIALIZE` |
| Repeated initialize stability | SDK lifecycle owner; no retained application session state or custom renegotiation gate |
| Malformed discovery metadata | Public stdio discovery negative control through actual SDK |
| Unsupported inline version | SDK metadata/negotiation owner; actual unsupported-version wire control |
| Legacy resource miss | Official-client and public wire rejection controls; SDK maps resource-not-found code by negotiated version |
| Discovery ping with older selected version | SDK lifecycle owner; public discovery ping rejection control |
| Discovery exposes read-only surfaces | Official client exactly-one-tool/resource, status equality and authority-none controls |
| Unknown tool | Official-client and public stdio Method Not Found with same request ID and available-tool data |
| Nonempty status arguments | Official-client and public stdio Invalid Params |
| Current resource miss | Same real-client/wire rejection; SDK performs version-dependent error mapping |
| Initialize after discovery | SDK lifecycle owner; custom lifecycle-switch gate retired |
| Discovery after initialize | SDK lifecycle owner; custom lifecycle-switch gate retired |
| Initialize instructions/tool description | Typed server metadata control and actual official-client discovery |
| Discovery instructions equal initialize | Shared SDK get_info metadata; official client lifecycle controls |
| Coalesced lines and CRLF | Retained FrameReader control over actual transport owner |
| Fragmented message | Retained FrameReader and split public initialize controls |
| Oversize discard/recovery | Retained bounded FrameReader cap/recovery control |
| Over-cap response keeps known ID | Typed bounded writer fallback control |
| Unreadable ID becomes null | SDK typed unknown-ID omission control; no arbitrary Value ID in production |

All three original public stdio controls remain: legacy status/tool-resource
equality, rejection arms, and discovery metadata/ping/unavailable recovery.
Their harness now retains child/output custody until replies arrive. Added
controls cover receive cancellation, partial-write cancellation, giant readable
IDs and the real official SDK clients. The earlier giant-ID and cancellation
behavioral failures remain historical evidence; successor verification must
bind native exits and nonzero selections to its own candidate.

## Slice B (#3089): refresh, bounded gap lists, and evidence resources

The status-only slice above was extended by #3089 without moving any
authority into the transport. The server remains a bounded adapter over
shared RIPR authority:

- `ripr_refresh` runs one bounded static analysis per call through the
  shared `app::check_workspace` authority — the same in-process analysis
  `ripr check` and the language server run. Analysis is read-only static
  evidence production, not execution authority; the adapter still declares
  source-edit, verification-execution, mutation-execution, and
  model-provider authority as none.
- The session keeps one in-memory completed snapshot (content-addressed
  `snapshot:sha256:` identity over the typed `AnalysisOutcome` and the
  canonical item identities). A cancelled or superseded attempt is never
  committed; a failed attempt never replaces the last-known-good snapshot.
- `ripr_list_gaps` serves the snapshot's stored shared diagnostic-budget
  selection (`lsp::diagnostic_budget`); the adapter never re-ranks and
  discloses every omitted identity and reason.
- `ripr_get_gap` and `ripr://gap/{canonical_item_id}` serve one canonical
  item's complete bounded evidence. Readiness is always a hard negative
  (`repair_packet_ready: false`); bounded repair surfaces, CommandSpec
  routes, and repair-attempt resources belong to #3090, and the
  repair-attempt link stays an explicit null.
- Project-local configuration stays detected-not-loaded; refresh runs with
  built-in defaults. No LSP protocol object is parsed and no VS Code
  artifact is read as authority: MCP and LSP remain peers over the same
  producers.

## Slice C (#3090): repair transaction projection

The session-evidence slice was extended by #3090 (RIPR-SPEC-0214) with
repair tools and resources — still without moving any authority into the
transport. The server remains a bounded adapter over shared RIPR authority:

- `ripr_prepare_repair` evaluates the committed producer repair-readiness
  facts (candidate actionability, an established discriminator, and a
  strong directly-related test fix site on a shared edit-cage test-surface
  path) and creates one deterministic, root-bound, in-memory session
  transaction only when every gate is established. An ineligible item
  returns an honest negative document and no attempt is created. The
  adapter does not call `begin_repair_attempt_with_identity`: durable
  attempt creation stays CLI-owned, and MCP reads the durable store
  read-only through the shared repair-attempt authority.
- `ripr_get_repair_attempt` / `ripr://repair-attempt/{attempt_id}` and
  `ripr_get_receipt_status` / `ripr://receipt/{receipt_id}` project session
  transactions and durable manifests/receipts exactly: typed `CommandSpec`
  routes travel only when they validate as typed specs, the human display
  string is explicitly marked as never execution authority, and the
  host-local root path is intentionally not projected (this ADR's hashing
  posture).
- Receipt status is a projection of producer-retained, digest-bound bytes,
  re-validated on every read; the adapter performs no verification, issues
  no receipt, and never upgrades a transaction it did not verify.
- The wire vocabulary gains `attempt_not_found` and `attempt_invalid`; the
  reserved `superseded` code is reachable for session transactions bound to
  a non-current snapshot.
- Authority declarations are unchanged: source-edit, verification
  execution, mutation execution, and model provider remain none; the
  adapter edits nothing, launches nothing, and executes nothing a returned
  route names.
