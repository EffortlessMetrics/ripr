# RIPR-SPEC-0193: RepairCard detail references and overflow disclosure

Status: proposed

Owner: product / agent

Created: 2026-10-05

Linked issues:

- #4666 (this slice)
- #3166 (parent: repair packet card profile)
- #4663 (RepairCardV1; landed as PR #4979)
- #4667 (CLI/LSP/MCP projection wiring)
- #4669 (measured budget ratification)

Support-tier impact:

- None. This slice extends an in-memory DTO and its pure builder; no command
  accepts or executes anything new.
  [docs/status/SUPPORT_TIERS.md](../status/SUPPORT_TIERS.md)

Policy impact:

- None. No new process, network, or file-policy surface. The budgeting engine
  measures serialized bytes in memory; no renderer or transport is added in
  this slice.

## Problem

`RepairCardV1` (RIPR-SPEC-0192) is the compact work order, but a compact card
that embeds every load-bearing evidence family is not compact for long: the
full fix instruction, witness/stage evidence, related-test candidates,
limitation detail, the canonical packet, attempt status, the focused-proof
receipt, static-movement detail and optional mutation calibration all grow
independently of the card. Silently truncating any of them would discard
exactly the evidence a reviewer needs; embedding all of them would make the
default card unbounded. #4666 keeps the default card finite while preserving
stable, explicit routes to every omitted load-bearing evidence family, with
measured accounting so the omission is auditable, not implicit.

## Behavior

`crates/ripr/src/domain/repair_card.rs` defines the additive, versioned
surface: `REPAIR_CARD_BUDGET_VERSION` (`repair-card-budget-v1`),
`RepairCardBudget` with its provisional-but-versioned defaults
(`DEFAULT_REPAIR_CARD_MAX_DETAIL_ITEMS = 16`,
`DEFAULT_REPAIR_CARD_MAX_SERIALIZED_BYTES = 64 KiB`,
`DEFAULT_REPAIR_CARD_MAX_INLINE_DETAIL_BYTES = 4 KiB`; #4669 ratifies the
numbers against measured agent use), the nine-variant
`RepairCardDetailFamily` vocabulary, the producer-reported
`RepairCardDetailState` vocabulary (`current`, `stale`, `malformed`,
`wrong_root`, `missing`, `unavailable`), `RepairCardOmissionClass`
(`not_embeddable`, `authority_owned_detail`, `unavailable`), the typed
`RepairCardDetailRef`, and the measured `RepairCardDetailSummary`.
`RepairCardV1` gains three `#[serde(default)]` fields
(`detail_references`, `detail_summary`, `complete_evidence_digest`) and keeps
`REPAIR_CARD_SCHEMA_VERSION = "repair_card.v1"` per the additive-change rule.

`crates/ripr/src/repair_card_budget.rs` owns the budgeting engine at the
crate root (domain must not know JSON rendering; the engine measures the
normalized serialized representation, so it follows the `repair_card_digest.rs`
split). The builder applies it after projecting the semantic card and before
minting `repair_card_id`:

- Every producer-supplied detail family becomes exactly one reference, in
  deterministic family-sorted order (input order never leaks). Duplicate
  families fail closed.
- Reference content never enters the wire card: only the family's content
  digest (sha256 over the normalized serialized bytes), the measured omitted
  byte count, the producer-reported state, the omission class, and a stable
  portable route — or, for unavailable families, no route plus the exact
  producer-owned unavailable reason — ride on the card.
- The canonical packet family is `not_embeddable` in every budget: the
  complete canonical packet stays retrievable only through its explicit
  detail route.
- The card's own compact prose fields (`changed_behavior`,
  `exact_blocker`, `assertion_goal_detail`, `candidate_value`, `limitations`,
  `stop_conditions`, `allowed_files`, `forbidden_files`) are fail-closed
  against the inline budget: an oversized compact field returns `Err` naming
  the field instead of being silently truncated or serialized unbounded; the
  full content belongs behind its owning family's reference.
- Unavailable families record an exact reason and claim no content; missing
  families keep their stable route with zero omitted bytes; stale, malformed
  and wrong-root states are projected verbatim and never upgraded or repaired
  silently. Root-specific route spellings (absolute paths, drive-letter
  roots) are refused at build time so equivalent roots keep one identity.
- Accounting is measured, never estimated: `selected_bytes` is the
  normalized serialized size of the final wire card (taken with the summary's
  self-reference zeroed and the finalized `repair_card_id` in place, so the
  measurement is deterministic and the byte bound covers the real id),
  `omitted_bytes` is the sum of the routed families' normalized bytes, and
  `complete_bytes` is their sum. `complete_evidence_digest` is a sha256 over
  the semantic card id joined with the sorted `family=detail_digest` pairs:
  the identity of the complete evidence, binding the card's repair facts to
  its routed content.
- Item and byte bounds are enforced on the result: a card that would exceed
  either fails closed.

Authority:

1. Budgeting changes presentation and detail only. It cannot change
   canonical identity, readiness, target selection or actionability: the
   semantic digest covers each reference's family, state and content digest,
   and never its route spelling, ordinal, byte counts, omission class, or the
   summary fields. Changing budget numbers alone remints nothing.
2. Equivalent roots and unchanged facts produce the same semantic card and
   reference identities: routes are portable producer-owned spellings outside
   the identity surface, and source ordering does not leak into references.
3. Every omitted load-bearing detail class has a stable retrieval reference
   or an exact unavailable reason; nothing is silently dropped and no
   completeness is claimed for content the card does not carry.
4. A single oversized detail (for example multi-megabyte witness evidence)
   cannot force unbounded wire serialization and cannot disappear: the card
   carries its digest, measured bytes, and route in a bounded reference while
   `complete_bytes` still accounts for the full content.
5. Referenced evidence states are projected facts. Stale stays stale,
   unavailable stays unavailable, and none of them strengthens or weakens the
   card's readiness flip, route exposure, or instruction state.

`build_repair_card` and the budgeting engine project existing authority
only. They perform no IO, no analysis, no edit, no execution and no network
access, and they never strengthen any readiness, command, attempt, or
evidence claim.

## Required Evidence

- Projection fixtures: all nine families project to exactly one reference
  each, in deterministic family-sorted order with dense ordinals; the
  canonical packet is `not_embeddable`; an unavailable family carries an
  exact reason and no route.
- Accounting fixtures: per-reference and summary byte counts match the actual
  normalized serialized representations; `complete_bytes = selected +
  omitted`; an oversized single detail keeps the card inside the byte bound
  while its full bytes stay accounted.
- Identity fixtures: source order and route spelling changes preserve
  reference and complete-evidence identities; budget-number changes remint
  nothing; a detail content move remints both the reference digest and the
  semantic card id.
- State fixtures: stale, malformed, wrong-root and missing references stay
  visibly so; budgeting never moves the readiness flip, the instruction
  state, or route exposure.
- Fail-closed fixtures: duplicate families, unavailable without an exact
  reason, routed states without a portable route or without content, missing
  families claiming content, root-specific route spellings, oversized compact
  fields, and item-bound violations all return `Err` rather than producing a
  partial or stronger card.

## Non-Goals

- No CLI, LSP, VS Code or MCP adoption in this slice (#4667 wires the first
  producer-owned detail sources).
- No arbitrary pagination framework or new public custom protocol.
- No analyzer, RepairAttempt or CommandSpec changes; the referenced
  authorities are consumed, never modified.
- No measured ratification of the budget numbers (#4669); the defaults are
  provisional but versioned.

## Acceptance Examples

1. A ready card projecting all nine families stays within the default item
   and byte bounds and carries nine typed references with measured
   accounting, a complete-evidence digest, and its minted semantic id.
2. A multi-megabyte witness payload never enters the wire card: the witness
   reference holds its content digest and measured bytes, and
   `complete_bytes` includes the full payload size.
3. A producer reporting mutation calibration as unavailable gets an exact
   reason on the card; no route is invented.
4. A compact `assertion_goal_detail` over the inline budget fails closed
   naming the field; the card is neither truncated nor enlarged.
5. Reordering the producer's sources or spelling a route for an equivalent
   root remints nothing; changing one family's content remints the card id.

## Test Mapping

- `crates/ripr/src/repair_card_budget.rs` unit tests cover the deterministic
  projection and ordering, measured accounting, oversized-detail boundedness,
  identity stability across order/route/budget changes, the visible
  stale/malformed/wrong-root/missing states, the no-stronger-state
  invariant, and every fail-closed validation.
- `crates/ripr/src/domain/repair_card.rs` unit tests cover budget validation
  and the empty summary default.
- `crates/ripr/src/app/repair_card.rs` unit tests cover builder integration:
  typed references on a ready card, identity remint on detail content moves,
  and stale detail evidence leaving readiness and route exposure untouched.
- `crates/ripr/src/repair_card_digest.rs` unit tests cover the digest scope:
  reference family/state/content digest are load-bearing; route spelling,
  ordinal, byte counts, omission class, and budget accounting are not.

## Implementation Mapping

| Surface | Responsibility |
| --- | --- |
| `crates/ripr/src/domain/repair_card.rs` | budget/version constants, `RepairCardBudget`, detail family/state/class vocabulary, `RepairCardDetailRef`, `RepairCardDetailSummary`, additive card fields, tests |
| `crates/ripr/src/repair_card_budget.rs` | normalization, measured accounting, reference projection, complete-evidence digest, fail-closed validation, tests |
| `crates/ripr/src/repair_card_digest.rs` | scoped detail identity in the semantic digest, digest-scope tests |
| `crates/ripr/src/app/repair_card.rs` | `RepairCardInput` detail sources + budget, builder integration, integration tests |
| `policy/public_api.txt` | public API allowlist entries for the new domain surface |

## Metrics

- `repair_card_schema_version`
- `repair_card_budget_version`
- `repair_card_violations`
