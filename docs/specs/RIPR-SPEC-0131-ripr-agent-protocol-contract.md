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
ratifies the machine contract around it. #1603 implemented
`ripr/listActionableItems`; every other reserved name remains reserved and
absent from `supported_requests` until a later slice implements it.

## Behavior

### Capability discovery

The server advertises the following under `initialize.result.capabilities.experimental.riprAgent`:

```json
{
  "protocol_version": "0.1",
  "schema_version": "0.2",
  "implementation_state": "implemented",
  "supported_requests": ["ripr/listActionableItems"],
  "reserved_requests": [
    "ripr/workspaceStatus",
    "ripr/refreshAnalysis",
    "ripr/listActionableItems",
    "ripr/getRepairPacket",
    "ripr/getEvidenceContext",
    "ripr/getTopLimitation",
    "ripr/getReceiptStatus"
  ],
  "supported_profiles": ["actionable"],
  "reserved_profiles": ["actionable", "full"],
  "diagnostic_modes": ["push"],
  "snapshot_handles": true,
  "continuations": false,
  "work_done_progress": false,
  "cancellation": true,
  "source_edit_capability": "none"
}
```

The example is abbreviated: the producer-owned capability also carries
`analysis_status_notification`, `compatibility_commands`, `error_kinds`, and
`claim_boundary`, and the capability schema requires the full field set. Since
#1603, `implementation_state` is `implemented` with exactly the handler set
above; a reserved name is still not a support claim.

`protocol_version` identifies the wire vocabulary and compatibility rules.
`schema_version` identifies the serialized DTO shape. They are independently
named so an additive DTO change does not silently become a protocol revision.
`reserved_*` values describe names that may be implemented by a later slice;
they are not support claims.

### Schema and protocol versioning

- `schema_version` moved additively from `0.1` to `0.2` (#1617 slice 4): the
  success envelope gained the route-readiness and typed-command-spec fields
  described below. The major stays `0`.
- Schema-version parsing is major-gated: any minor under major `0` parses, so
  a `0.1`-versioned client keeps parsing the version identity of a `0.2`
  envelope. A client that enforces the closed DTO field set must adopt the
  `0.2` shape; a client that ignores unknown fields is unaffected.
- `protocol_version` stays `0.1`: the request vocabulary, compatibility rules,
  and recovery routes did not change, only the serialized DTO shape did. An
  additive DTO change must not be published as a protocol revision.
- `protocol_version` and `schema_version` move independently. A future
  wire-vocabulary change bumps `protocol_version`; a future DTO-shape change
  bumps `schema_version`.

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
- nullable `verify_route_readiness` and `receipt_route_readiness` values from
  a closed four-value vocabulary (#1617 slice 4, schema 0.2);
- nullable `verify_command_spec` and `receipt_command_spec` objects carrying a
  producer-owned typed CommandSpec in full when one exists;
- `limitations` and `non_claims` arrays.

These fields describe a contract and do not fabricate evidence. A capability-only
server does not emit a success response for a reserved request yet.

The success envelope documented here is the contract target for implemented
requests. The currently live `ripr/listActionableItems` handler still returns
its first-slice abbreviated object (no version identity and no route fields);
migrating that handler onto the full envelope is a tracked follow-up under
#1617 and must not be read as already conforming to this section.

### Route readiness vocabulary (#1617 slice 4)

`verify_route_readiness` and `receipt_route_readiness` use one closed
vocabulary with exactly four values:

```text
typed_direct          a producer-owned typed CommandSpec with
                      execution_mode `direct` is present in the paired
                      command-spec field
typed_shell_required  a producer-owned typed CommandSpec with
                      execution_mode `shell_required` is present
manual                the route is described by a validated producer-owned
                      CommandSpec with execution_mode `manual`: the route is
                      not automatically executable, but consumers should keep
                      the spec as route context instead of discarding it
                      (declared to keep the vocabulary closed; no producer
                      emits it today)
legacy_string_only    only the legacy display string exists; the paired
                      command-spec field is null
```

Rules:

- Readiness is `null` exactly when the paired legacy route string is `null`.
- `verify_route` and `receipt_route` remain legacy display/compatibility
  strings. They are never reinterpreted as typed routes, and a typed
  CommandSpec is never synthesized from them.
- A command spec present in `verify_command_spec` must carry role `verify`
  and satisfy the CommandSpec validation contract; `receipt_command_spec`
  must carry role `receipt`. The readiness value must equal the value derived
  from the spec's `execution_mode`. Slot↔spec role matching is beyond
  cross-field JSON Schema and is enforced by the Rust resolver in
  `crates/ripr/src/lsp/agent_protocol.rs`.
- A payload that declares schema 0.2 or later must carry all four new fields
  explicitly; omission is a decoding failure. Only a true schema-0.1 payload
  may omit them, and its readiness is then derived truthfully (`legacy_string_only`
  for present route strings, `null` for null routes). Declaring a readiness
  that contradicts the route and spec the envelope carries is a contract
  violation, not a tolerance.

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
  version. Schema minors under major `0` are additive (see
  "Schema and protocol versioning"); a `0.2` envelope carries the route
  readiness and command-spec fields, and a reader deriving them from a
  `0.1`-shaped payload must derive them truthfully, not fabricate them.
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
- Route readiness uses exactly the closed four-value vocabulary; readiness is
  `null` exactly when the paired route string is `null`, and a typed spec is
  never synthesized from a legacy display string (#1617 slice 4).
- `supported_requests` and `supported_profiles` name exactly the implemented
  handlers and profiles; reserved names are not support claims.
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
- Route-slot coverage (#1617 slice 4) lives in the same module:
  `schema_minor_bump_stays_additive_within_major_zero` (additive minor bump,
  protocol version unchanged), `route_readiness_vocabulary_is_closed` (the
  four-value closed vocabulary), `typed_command_specs_carry_typed_readiness`
  (typed spec serializes with matching readiness and round-trips),
  `legacy_route_strings_stay_legacy_string_only` (legacy display strings stay
  `legacy_string_only` with null specs), `null_route_carries_null_readiness_and_null_spec`
  (readiness is null exactly when the route is null),
  `schema_0_1_payload_without_route_fields_still_decodes` (a true
  `schema_version: "0.1"` payload without the new fields derives readiness
  truthfully), `schema_0_2_payload_omitting_route_field_is_rejected` (a 0.2
  payload omitting any of the four fields, or carrying an explicit null
  readiness beside a route, fails with a named error),
  `readiness_must_agree_with_route_and_spec` (contradictory readiness, spec
  without a route, and role-mismatched specs fail closed with pinned error
  messages), and
  `command_spec_wire_shape_matches_the_domain_type` (the published
  CommandSpec schema shape pins the serde wire names, including the
  `working_directory`/`environment`/`network`/`human_display` renames), and
  `command_spec_decoding_is_closed_to_the_published_shape` (spec decoding
  rejects serde legacy aliases and unknown fields the published schema does
  not declare).
- `capability_output_matches_the_published_schema` validates the live
  `server_capability()` bytes and the committed capability fixture against
  `schemas/ripr/ripr-agent-capability.schema.json` (structural const/enum/
  required-key assertions; no JSON-Schema validator crate is in this crate's
  dependency set).
- JSON examples and negative controls live in
  `fixtures/lsp_agent_protocol/`.

## Implementation Mapping

- `crates/ripr/src/lsp/agent_protocol.rs` owns the capability producer and
  reserved DTO vocabulary. Route-slot readiness is resolved in one place,
  `resolve_route_readiness`, so every surface shares the same fail-closed
  coherence rules (#1617 slice 4).
- `crates/ripr/src/lsp/capabilities.rs` remains the single initialize
  projection authority.
- `schemas/ripr/ripr-agent-*.schema.json` owns machine-readable envelope
  shapes; no handler consumes them in this slice. The success schema reuses
  the published CommandSpec definition from
  `schemas/ripr/repair-assurance.schema.json` (`$defs.command_spec`) rather
  than forking a second CommandSpec shape.

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
