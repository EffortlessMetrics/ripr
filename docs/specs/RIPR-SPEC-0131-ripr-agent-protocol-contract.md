# RIPR-SPEC-0131: Versioned riprAgent protocol contract

Status: proposed

Owner: product / swarm

Created: 2026-07-19

Parent issue: #1599

Linked spec:

- [RIPR-SPEC-0129](RIPR-SPEC-0129-editor-integration-contract.md) — editor
  integration layers and support matrix.
- [RIPR-SPEC-0069](RIPR-SPEC-0069-lsp-agent-feedback-use-case.md) — bounded
  LSP cockpit behavior.

Linked ADRs:

- None.

Support-tier impact:

- See: [docs/status/SUPPORT_TIERS.md](../status/SUPPORT_TIERS.md).

- None. This contract makes the capability-only `riprAgent` surface
  discoverable and versioned. It does not promote lifecycle, evidence, or
  repair usefulness beyond the existing advisory boundary.

## Problem

RIPR already advertises a fail-closed `experimental.riprAgent` capability and
reserves request and error names. A generic headless client still cannot rely
on a typed wire contract: protocol and DTO schema versions are not distinct,
reserved requests do not have schema entries, and success/error recovery fields
are not bounded by a repository-owned contract.

The capability block is the only initialization authority. This specification
ratifies the machine contract around it without implementing any request
handler. Until a later slice implements a request, its name remains reserved
and absent from `supported_requests`.

## Behavior

### Authority

This spec is the sole authority for the `riprAgent` wire contract: the
exact request methods, protocol and schema versions, request, success,
and error DTOs, the closed error-kind vocabulary, and the
compatibility rules (#1925). Clipboard and open-file client commands
(`ripr.copy*`, `ripr.open*`) are never headless-agent authority; they
belong to the negotiated client-executed command surface governed by
[RIPR-SPEC-0129](RIPR-SPEC-0129-editor-integration-contract.md), which
also owns the three client layers and capability negotiation.
[RIPR-SPEC-0069](RIPR-SPEC-0069-lsp-agent-feedback-use-case.md) owns
the product boundary — the fail-closed evidence rules and the
read-only default edit policy — and neither is restated here.

### Capability discovery

The server advertises the following under `initialize.result.capabilities.experimental.riprAgent`:

```json
{
  "protocol_version": "0.1",
  "schema_version": "0.1",
  "implementation_state": "capability_only",
  "supported_requests": [],
  "reserved_requests": [
    "ripr/workspaceStatus",
    "ripr/refreshAnalysis",
    "ripr/listActionableItems",
    "ripr/getRepairPacket",
    "ripr/getEvidenceContext",
    "ripr/getTopLimitation",
    "ripr/getReceiptStatus"
  ],
  "supported_profiles": [],
  "reserved_profiles": ["actionable", "full"],
  "diagnostic_modes": ["push"],
  "snapshot_handles": false,
  "continuations": false,
  "work_done_progress": false,
  "cancellation": false,
  "source_edit_capability": "none"
}
```

The example is abbreviated: the producer-owned capability also carries
`analysis_status_notification`, `compatibility_commands`, `error_kinds`, and
`claim_boundary`, and the capability schema requires the full field set.

`protocol_version` identifies the wire vocabulary and compatibility rules.
`schema_version` identifies the serialized DTO shape. They are independently
named so an additive DTO change does not silently become a protocol revision.
`reserved_*` values describe names that may be implemented by a later slice;
they are not support claims.

### Request envelope

Reserved requests use a closed request vocabulary. Every request carries the
two explicit versions and a `request` discriminator. The `mode` discriminator
is `read_only` for inspection and `refresh` for the state-changing analysis
request. The following keys are required in every serialized request until a
handler requires a value; each value may be `null` (explicit absence), and a
client must not omit the keys:

- `profile`: `actionable` or `full`, or `null`;
- `snapshot_id`: an opaque retained-snapshot identity, or `null`;
- `continuation_id`: an opaque continuation identity, or `null`.

The request schema rejects unknown request names and unknown fields. Nullable
snapshot/profile/continuation fields remain explicit in serialized envelopes,
so absence is not confused with a fabricated identity. Existing `ripr.collect*`
execute commands remain compatibility surfaces and are not silently
reinterpreted as this protocol.

### Success envelope

Successful responses carry producer-owned identity and honesty fields:

- `protocol_version` and `schema_version`;
- the reserved `request` and response `kind`;
- `status: "ok"`;
- `snapshot_id`, `input_identity`, `profile`, and `budget_identity` as
  distinct opaque values;
- root, configuration, and base identities where applicable;
- `freshness`, `run_status`, and selected/omitted/total counts;
- `complete_evidence_identity` when evidence is complete;
- a nullable `continuation_identity`;
- `allowed_edit_surface` and `must_not_change` read-only boundaries;
- nullable `verify_route` and `receipt_route` routes;
- `limitations` and `non_claims` arrays.

These fields describe a contract and do not fabricate evidence. A capability-only
server does not emit a success response for a reserved request yet.

### Error envelope

Errors use a closed `error.kind` vocabulary and a bounded `error.recovery_route`.
Supported error kinds are:

```text
no_snapshot
analysis_in_flight
stale_snapshot
stale_continuation
workspace_ambiguous
config_invalid
item_not_found
route_static_limitation
unsupported_protocol_version
unsupported_profile
cancelled
superseded
```

An error includes the request and both schema identities, a typed `error`
object, retryability, a bounded recovery route, and an optional retained
snapshot identity. Generic `null`, rendered logs, and unstable internal error
strings are not protocol authority.

## Compatibility and fail-closed rules

- Unknown protocol major versions fail visibly with
  `unsupported_protocol_version`.
- Unknown schema versions fail visibly with `unsupported_schema_version`;
  additive optional fields are only accepted under the documented schema
  version.
- A client must inspect `supported_requests` and `supported_profiles`; it must
  not probe command behavior or infer support from the editor name.
- A reserved request is not supported merely because it appears in
  `reserved_requests`.
- `source_edit_capability = "none"` remains the only claim in this slice.
- Snapshot, input, profile, and budget identities must not be collapsed into
  one token.
- The protocol never adds an LLM, provider, source edit, autonomous repair, or
  alternate capability builder.

## Required Evidence

- `schemas/ripr/ripr-agent-capability.schema.json` defines capability
  discovery.
- `schemas/ripr/ripr-agent-request.schema.json` defines the closed request
  envelope.
- `schemas/ripr/ripr-agent-success.schema.json` defines the bounded success
  envelope.
- `schemas/ripr/ripr-agent-error.schema.json` defines typed failure and
  recovery.
- `fixtures/lsp_agent_protocol/` contains deterministic valid examples and
  negative examples for unsupported versions and unsupported request/profile
  values.
- `crates/ripr/src/lsp/agent_protocol.rs` owns the producer and its unit tests.
- `.ripr/traceability.toml` maps this spec to the DTO code, schemas, and
  fixture/test evidence.

## Acceptance Examples

- A generic client reads `supported_requests: []` and does not probe
  `ripr/getRepairPacket`; the capability-only claim remains honest.
- A future request with protocol `1.0` is rejected as
  `unsupported_protocol_version` before any workspace work is assigned.
- A future success envelope keeps `snapshot_id`, `input_identity`,
  `budget_identity`, and the read-only edit boundary as separate fields.
- A stale-snapshot error exposes `refresh` as its recovery route and never
  offers a source edit.

## Acceptance

- A generic client can determine the exact protocol version and supported
  request/profile sets from `initialize` without probing behavior.
- Every reserved request, profile, and error has one closed vocabulary entry
  and a schema-backed example.
- Protocol and DTO schema versions are explicit and independently validated.
- Unknown major versions and unsupported request/profile values fail visibly.
- Snapshot/input/profile/budget identities are distinct in the success shape.
- Error payloads carry bounded machine fields and a recovery route.
- Existing `ripr.collect*` commands remain unchanged compatibility surfaces.
- `supported_requests` and `supported_profiles` remain empty; no handler is
  implemented or advertised by this slice.
- The spec, schema, fixture, traceability, and capability checks pass.

## Proof

```text
cargo test -p ripr --lib lsp::agent_protocol -- --nocapture
cargo xtask check-output-contracts
cargo xtask check-spec-format
cargo xtask check-spec-numbering
cargo xtask check-traceability
cargo xtask check-capabilities
cargo xtask check-doc-index
cargo fmt --all -- --check
git diff --check
```

Run under the pinned 1.95.0 toolchain (rustfmt 1.9.0) using the worktree's own
build artifacts; a stale cross-worktree binary is not proof of this slice.

## Test Mapping

- Rust vocabulary, version, envelope, and boundary tests live in
  `crates/ripr/src/lsp/agent_protocol.rs`.
- JSON examples and negative controls live in
  `fixtures/lsp_agent_protocol/`.

## Implementation Mapping

- `crates/ripr/src/lsp/agent_protocol.rs` owns the capability producer and
  reserved DTO vocabulary.
- `crates/ripr/src/lsp/capabilities.rs` remains the single initialize
  projection authority.
- `schemas/ripr/ripr-agent-*.schema.json` owns machine-readable envelope
  shapes; no handler consumes them in this slice.

## Metrics

- `ripr_agent_protocol_version_discoverable`;
- `ripr_agent_reserved_vocabulary_closed`;
- `ripr_agent_capability_fail_closed`.

## Non-Goals

- Request handlers or transport changes.
- Snapshot lifecycle, continuation paging, progress, or cancellation.
- Pull diagnostics, budget defaults, or full evidence retrieval.
- WorkspaceEdit, autonomous repair, or source mutation.
- Editor UI changes or a new heavy CI lane.
- Model/provider integration or real-client dogfood.

## Claim boundary

This slice establishes the versioned machine contract and compatibility rules
for a capability-only headless-agent surface. It does not prove lifecycle
behavior, transport performance, complete evidence retrieval, or repair
usefulness.
