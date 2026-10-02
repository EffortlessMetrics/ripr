# RIPR-SPEC-0202: Agent card typed refusal envelope

Status: proposed

Owner: product / agent

Created: 2026-10-02

Linked issues:

- #5007 (this slice)
- #3166 (parent: repair packet card profile)
- #4667 (the `ripr agent card` default handoff; RIPR-SPEC-0194)
- #4663 (RepairCardV1; RIPR-SPEC-0192)
- #4666 (bounded detail references; RIPR-SPEC-0193)

Support-tier impact:

- None. The refusal envelope is an advisory, read-only projection of the
  existing handoff failure states; it accepts no edit or execution authority
  and writes nothing. Exit codes distinguish refusal (`3`) from
  could-not-complete (`2`) exactly as the verify/repair precedent does. See
  [docs/status/SUPPORT_TIERS.md](../status/SUPPORT_TIERS.md).

Policy impact:

- None. No new process, network, or file-policy surface. The envelope
  renders to stderr only, and only under `--json`.

## Problem

Every `ripr agent card` failure mode collapsed to exit code `2` plus human
prose on stderr (#5007). The distinct machine states a loop driver must
branch on — seam id not found, policy-omitted seam class, witness analysis
producing no witness, unnameable portable workspace identity, and
builder/route-gate/budget refusal — were only distinguishable by parsing
English sentences the output contracts explicitly leave free to change.
"Not found" and "policy-omitted" need opposite remedies; conflating them
sends an autonomous loop to re-list seams forever, or treats a policy
decision as a transient error. The sibling `verify`, `verify-execute`, and
`repair` commands already emit typed refusal documents at exit code `3`, so
an orchestrator can branch on status alone; the default handoff gave less
machine signal than the commands it fronts for.

## Behavior

Every deliberate named refusal of the `ripr agent card` handoff maps to the
decision exit code `3` and, under `--json`, renders exactly one versioned
typed envelope on stderr:

```json
{
  "schema_version": "0.1",
  "kind": "agent_card_refusal",
  "error": {
    "kind": "seam_not_found",
    "seam_id": "<the seam id the call asked for>",
    "message": "<the exact human prose of the refusal>",
    "remedy_route": "<one typed command to run instead>"
  }
}
```

- The envelope's `schema_version` (`0.1`) is deliberately distinct from the
  success document's `repair_card.v1`, so a consumer dispatching on
  `schema_version` never confuses a refusal with a card.
- `error.kind` is one closed set of five typed kinds, owned by the
  `AgentCardRefusalKind` enum in `crates/ripr/src/domain/repair_card.rs`:
  - `seam_not_found` — the requested seam id names no seam in the current
    inventory. Remedy: re-list seams (`ripr pilot`) or correct the id.
  - `policy_omitted` — the seam's grip class is omitted from agent results
    by the `AgentBriefPolicy` configuration. Remedy: check the
    `agent brief` policy config; the seam is a dead end and re-listing
    cannot fix it.
  - `witness_unavailable` — the witness analysis could not produce this
    seam's witness. Remedy: rerun the analysis or pick another seam.
  - `identity_unnameable` — nothing on this seam names a portable workspace
    identity. Remedy: retrieve the full packet instead.
  - `budget_overflow` — the card builder, route gate, or budget refused to
    mint the card. Remedy: fall back to the canonical packet.
- `error.seam_id` names the seam id the invocation asked for; it is present
  on every kind.
- `error.message` carries the exact human prose, verbatim; it is the
  non-authority rendering and stays free to change. Consumers branch on
  `error.kind`, never on this text.
- `error.remedy_route` names one typed command for the kind's remedy family,
  bound to the same root the failing call used.
- The card's stdout stays empty on every refusal: stdout is the handoff
  artifact stream, the same rule `agent verify` follows (RIPR-SPEC-0134).
  Without `--json`, stderr carries the prose rendering only, byte-identical
  to the pre-typing rendering.
- Operational could-not-complete failures (an unreadable config, a failed
  git head or dirty-state probe, a detail-source serialization failure, an
  unreadable attempt inventory) stay exit code `2` with human prose only.
- A seam with no witness-producing finding still renders its card with the
  `unavailable` instruction and the three unavailable witness families
  (RIPR-SPEC-0194 acceptance); that in-band card state is typed already and
  is not a refusal. No card DTO field, success-path schema, route-gate, or
  packet byte changes.

## Required Evidence

- The `AgentCardRefusalKind` wire set is pinned three ways by
  `cargo xtask check-output-contracts`: the owning enum's variants and
  `as_str` literals in `crates/ripr/src/domain/repair_card.rs`, the
  `agent_card_refusal_kind` rows in `policy/output_contracts.txt`, and the
  `## Enums` list in `docs/OUTPUT_SCHEMA.md` must agree exactly.
- The document kind `agent_card_refusal` is a registered output contract
  row pinned against the CLI adapter producer.
- A CLI smoke test proves the end-to-end refusal: exit code `3`, empty
  stdout, one parseable `agent_card_refusal` envelope on stderr under
  `--json`, and the prose rendering preserved.
- Unit tests pin the envelope shape (schema version, kind, seam id, verbatim
  message, remedy route per kind) and the producer's refusal/operational
  split (kind + prose preserved through the `String` rendering the LSP and
  usability consumers use).

## Inputs

- Any `ripr agent card --root <root> --seam-id <id> [--json]` invocation
  whose handoff cannot complete as a deliberate named refusal.

## Outputs

- Exit code `3` with the `agent_card_refusal` stderr envelope under
  `--json`; exit code `3` with prose-only stderr without it; exit code `2`
  unchanged for operational failures.

## Acceptance Examples

- `ripr agent card --seam-id probe:src_lib.rs:predicate:566edf6b --json`
  exits `3`, prints nothing to stdout, and prints on stderr one envelope
  whose `error.kind` is `seam_not_found`, whose `error.seam_id` repeats the
  asked-for id, whose `error.message` names the finding-ID hint, and whose
  `error.remedy_route` is the `ripr pilot` listing command.
- A seam hidden by policy exits `3` with `error.kind` `policy_omitted` and a
  remedy route naming the `agent brief` surface — never the pilot listing,
  so the two opposite remedies stay distinguishable by `error.kind` alone.
- An unprobed or unprobeable repository state (for example a git failure
  while resolving HEAD) exits `2` with prose only; it never renders the
  envelope.

## Non-Goals

- No typed error envelope for other agent commands in this slice (`agent
  start`, `agent packet`, `agent brief`, `agent receipt`, and the rest keep
  their existing contracts).
- No change to the card DTO shape, the success-path schema, the route gate,
  the packet render, or the LSP/editor projection's fail-closed omission
  behavior.
- No witness-presence requirement change: the no-witness card render stays a
  success path (RIPR-SPEC-0194 acceptance item 5).
- No new exit code; the existing `2`/`3` contract (docs/EXIT_CODES.md) is
  reused, not extended.

## Test Mapping

- `crates/ripr/tests/cli_smoke.rs::agent_card_hands_off_one_seam_as_the_default_repair_card`
  extends the cold-agent refusal journey: exit code `3`, empty stdout, and
  the parseable `agent_card_refusal` envelope on stderr under `--json`.
- `crates/ripr/src/cli/commands/agent_card.rs` unit tests pin the envelope
  shape for all five kinds and the opposite-remedy pairs.
- `crates/ripr/src/app/repair_card_handoff.rs`
  `agent_card_error_split_carries_kind_and_prose` pins the producer split:
  kinds stay attached to the verbatim prose through the `String` rendering.

## Implementation Mapping

| Surface | Responsibility |
| --- | --- |
| `crates/ripr/src/domain/repair_card.rs` | `AgentCardRefusalKind` enum, wire spelling (`as_str`), serde vocabulary |
| `crates/ripr/src/app/repair_card_handoff.rs` | `AgentCardError` refusal/operational split; refusal kinds bound at the producer sites (`witness_unavailable`, `identity_unnameable`, `budget_overflow`); operational probes stay exit `2` |
| `crates/ripr/src/cli/commands/agent_card.rs` | adapter-owned kinds (`seam_not_found`, `policy_omitted`), the `agent_card_refusal` envelope renderer, remedy routes, `CommandError::Decision` mapping |
| `crates/ripr/src/cli/commands/agent.rs` | dispatch: `AgentCommand::Card` carries the typed-refusal exit mapping |
| `policy/output_contracts.txt` + `xtask/src/output_enum_contracts.rs` | registry rows and the governed-enum completeness pin for the five kinds |
| `docs/OUTPUT_SCHEMA.md` + `docs/EXIT_CODES.md` | envelope field contract, version-table row, exit-code rows |

## CI Proof

- `cargo xtask check-output-contracts` (registry, producer, docs, and enum
  agreement), `cargo xtask check-spec-format`, `cargo xtask
  check-spec-numbering`, `cargo xtask check-traceability`.
- The "Ripr Rust Small Result" hosted gate and the full hosted matrix for
  the workspace tests, clippy, and rustfmt.

## Metrics

- None new; the refusal envelope is a projection of existing producer
  states. The card usability measurement (RIPR-SPEC-0196) consumes the
  unchanged assembly path.

## Failure Modes

- A producer error string drifting without a matching kind stays at exit
  code `2` (fail-closed to operational), never silently re-typed; the
  governed-enum check rejects kind-set drift, and the CLI smoke test pins
  the end-to-end envelope.
- An envelope render failure exits `3` with the prose rendering only (the
  refusal decision is already made); the typed kind is never dropped to `2`.
