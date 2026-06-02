# RIPR-SPEC-0061: Lane 1 Canonical Actionability Contract

Status: proposed

Linked specs:

- [RIPR-SPEC-0021: Evidence Record](RIPR-SPEC-0021-evidence-record.md)
- [RIPR-SPEC-0031: Lane 1 Evidence Quality Audit](RIPR-SPEC-0031-lane1-evidence-quality-audit.md)
- [RIPR-SPEC-0056: Public Actionable Projection](RIPR-SPEC-0056-public-actionable-projection.md)
- [RIPR-SPEC-0057: RIPR Swarm Repair Loop](RIPR-SPEC-0057-ripr-swarm-repair-loop.md)
- [RIPR-SPEC-0059: Actionable Surface Translation](RIPR-SPEC-0059-actionable-surface-translation.md)

## Problem

Lane 1 exists to make RIPR evidence safe enough to drive repair work without
pretending static analysis knows more than it does. This spec defines the shared
actionability rule used by evidence records, Lane 1 reports, actionable-gap
packets, swarm planning, badge inputs, editor status, PR summaries, and CI
advisory gates.

The product unit is not a raw finding. The product unit is a canonical evidence
item with a stable identity and an explicit actionability state.

## Behavior

### Core Rule

RIPR may call a canonical evidence item actionable only when it can provide a
safe, bounded repair route.

If typed evidence cannot support that repair route, the item must be a named
non-actionable state. It must not be counted as an actionable gap, projected as a
public repair packet, ranked as swarm-ready, or used as a blocking CI candidate.

### Evidence Flow

Lane 1 producers and consumers use this flow:

```text
raw static signals
  -> canonical evidence items
    -> actionability classification
      -> repair packet
        -> verify command
          -> receipt command
            -> attempt/outcome evidence
```

Raw findings remain supporting evidence. They do not create independent
user-facing work unless they collapse into a canonical item that satisfies this
contract.

### Vocabulary

`raw_finding`

One analyzer signal from the static engine. It may include file, line,
expression, class, and source span data. Raw findings are not user-facing repair
work.

`canonical_item`

The deduplicated evidence item surfaced to users and agents. It carries a stable
identity, source evidence references, state, actionability classification, and
optional repair packet.

`canonical_gap_id`

Stable identity for a canonical behavioral gap. It must survive line movement,
report generation, badge inputs, LSP status, PR comments, CI advisory gates,
repair packets, receipts, ledgers, and trend reports.

`gap_state`

The canonical item's current state. Required values for Lane 1 projection are:

- `actionable`
- `static_limitation`
- `advisory`
- `internal_only`
- `already_observed`
- `unknown`

`repair_packet`

The bounded work unit handed to a human or agent. It is advisory repair intent,
not authorization to edit arbitrary code.

`static_limitation`

Named analyzer limitation. It explains why RIPR cannot safely project a repair
packet and gives the analyzer or human route for improving the evidence.

### Actionable Item Requirements

A canonical item may use `gap_state = "actionable"` only when all fields below
are present and internally consistent:

- `canonical_gap_id`
- `gap_state = "actionable"`
- `repair_kind`
- `target_test_shape` or equivalent typed target observer shape
- `verify_command`
- `receipt_command` or receipt path that can be turned into a receipt command
- `confidence`
- `must_not_change[]`
- `allowed_edit_surface[]`
- `raw_evidence_refs[]`

`raw_evidence_refs[]` is evidence lineage, not a placeholder slot. Each public
or swarm-ready item must include at least one structured reference with:

- an anchor field: `file`, `path`, or `source_file`;
- an identity field: `kind`, `source_id`, `evidence_record_ref`, or
  `canonical_gap_id`.

Empty objects, prose strings, raw duplicate counts, or placeholder values do not
satisfy the evidence reference requirement. Downstream surfaces must treat those
as missing evidence context and suppress the item from public repair queues.

The item must also have one of:

- a related test or observer that can safely be extended;
- a typed target test location;
- a typed target observer shape that does not require production-code edits.

The item must not depend only on prose, raw line count, raw same-line duplicate
signals, or an unresolved static limitation.

### Repair Packet Requirements

Every public or swarm-ready repair packet must include:

- `packet_id`
- `canonical_gap_id`
- `repair_kind`
- `target_test_shape`
- `related_test_or_observer`
- `verify_command`
- `receipt_command`
- `confidence`
- `must_not_change[]`
- `allowed_edit_surface[]`
- `raw_evidence_refs[]`

The packet form uses the same structured `raw_evidence_refs[]` rule as the
canonical item. A packet with only placeholder refs is not swarm-ready even when
it carries `repair_kind`, `verify_command`, and `receipt_command`.

`receipt_command` must be an executable command, not only a receipt path or
receipt hint. A path-only value stays non-ready with
`missing_receipt_command`.

`must_not_change[]` is required because packets are intended to bound repair
attempts. `allowed_edit_surface[]` is required because delegated packet attempts
need an explicit workspace-relative edit cage. A packet without at least one
allowed file that resolves to an existing workspace file is not
public-projection eligible or swarm-ready. Typical
`must_not_change[]` entries include:

When canonical items do not already carry `related_test_or_observer`, the
structured `recommendation.recommended_test` target may supply that packet
field and the derived `allowed_edit_surface[]`, but only when the target file
exists in the workspace. Implementations must not parse free-form recommendation
prose or create guessed root-level test paths to infer this edit cage.

- do not edit production code;
- do not broaden the assertion beyond the named observer shape;
- do not change public API or policy behavior;
- do not add suppression, waiver, or baseline entries as the repair.

### Non-Actionable Requirements

When a canonical item cannot satisfy the actionable requirements, it must use a
non-actionable state instead of weakening the packet contract.

For `gap_state = "static_limitation"`, the item must include:

- `category`
- `why_not_actionable`
- `repair_route`
- `raw_evidence_refs[]`

For `gap_state = "advisory"`, the item must include:

- advisory reason;
- why it is not safe as a bounded repair route;
- what evidence would be needed to promote it.

For `gap_state = "internal_only"`, the item must include:

- why it is analyzer or inventory pressure rather than user repair work;
- which downstream surfaces must suppress it from public action queues.

For `gap_state = "already_observed"`, the item must include:

- the observer or related evidence that already covers the gap;
- why no repair packet should be emitted.

For `gap_state = "unknown"`, the item must include:

- the missing typed evidence;
- a route for analyzer improvement or manual investigation.

### Unsafe Actionability Examples

Boundary operands such as:

```rust
if idx >= offset
```

must not become `repair_kind = "add_boundary_assertion"` with an exact candidate
value when the operands are local, iterator-derived, or computed and the
analyzer cannot safely resolve activation values.

That case must become:

```text
gap_state = static_limitation
category = activation_boundary_input_unresolved
repair_route = add analyzer support for iterator, member-access, local, or computed operand resolution
```

Iterator-derived operands should route to
`analysis/iterator-boundary-operand-resolution`; member-access operands should
route to `analysis/local-member-boundary-operand-resolution`; local or computed
operands should route to
`analysis/local-computed-boundary-operand-resolution`. All remain static
limitations until the analyzer can safely name activation values.

The same rule applies to any class where RIPR cannot safely name the activation
value, observer shape, verify command, or receipt command.

### Consumer Rules

Badge, LSP, PR, CI, scorecard, readiness, and swarm consumers must not derive
actionability from raw findings or prose. They must consume typed canonical
state.

Consumers must fail closed when:

- `canonical_gap_id` is missing for a claimed actionable item;
- `verify_command` is missing;
- `receipt_command` is missing;
- `must_not_change[]` is missing;
- `allowed_edit_surface[]` is missing;
- the item has static limitations that are not explicitly handled;
- the run is stale or not downstream-consumable;
- the item is raw-only, seam-inventory-only, or preview-only.

Consumers may still display non-actionable items as analyzer improvement work,
but they must not rank them as next user repair work.

## Required Evidence

A producer or consumer claiming conformance to this contract must provide:

- schema or spec text naming the canonical fields it emits or consumes;
- tests or fixtures for actionable items with identity, verify, receipt,
  confidence, must-not-change, and raw evidence references;
- tests or fixtures for non-actionable static limitations;
- fail-closed tests for missing identity, repair route, verify command, receipt
  command, or must-not-change boundaries;
- docs naming the non-goals for generated tests, autonomous edits, mutation
  execution, provider integration, and default blocking gates.

## Output Compatibility

This spec is additive over existing output contracts. Existing `actionability`
fields remain advisory labels until the producer can satisfy the stricter
canonical packet contract. Producers may expose both the older advisory label
and the stricter `gap_state`/packet fields while consumers migrate.

If a producer cannot tell whether an older artifact satisfies this contract, it
must treat the artifact as non-actionable or projection-excluded.

## Non-Goals

This spec does not authorize:

- generated tests;
- autonomous edits;
- provider integration;
- mutation execution;
- public badge semantic changes;
- CI blocking by default;
- production-code edits as a repair unless a future spec explicitly allows them.

## Acceptance Examples

- Actionable items require canonical identity, repair kind, target shape, verify
  command, receipt command, confidence, must-not-change boundaries, and raw
  evidence references.
- Non-actionable items name a state, limitation or advisory reason, why they are
  not actionable, and a repair route for analyzer or human improvement.
- Same-line duplicate raw findings collapse into canonical items before any
  user-facing count.
- Downstream surfaces consume canonical actionability state, not raw finding
  counts.
- Static limitations reduce misleading actionable counts instead of becoming
  vague repair packets.

## Test Mapping

Existing proof is split across linked specs and fixtures:

- evidence-record canonical item fields and static limitations;
- Lane 1 evidence-quality audit counts and actionable-gap packet projection;
- public actionable projection eligibility and exclusion reasons;
- swarm plan blocking for missing verify, missing receipt, and static
  limitation packets;
- editor actionable gap queue fail-closed validation.

Implemented false-actionable proof covers unresolved activation boundary
operands:

- `crates/ripr/src/analysis/test_grip_evidence.rs::tests::given_boundary_owner_call_when_input_operand_is_iterator_local_then_activation_is_static_limitation`
  keeps `idx >= offset` style iterator-local operands as named limitations and
  suppresses exact candidate values;
- `crates/ripr/src/analysis/test_grip_evidence.rs::tests::given_boundary_owner_call_when_input_operand_is_computed_local_then_activation_stays_static_limitation`
  keeps computed locals as named limitations and suppresses exact candidate
  values;
- `crates/ripr/src/analysis/test_grip_evidence.rs::tests::given_boundary_seam_when_parameter_operands_are_called_with_equal_strings_then_missing_discriminator_is_cleared`
  keeps parameter-vs-parameter equality boundaries from becoming repair
  packets when a related test already calls the owner with equal concrete
  operands;
- `crates/ripr/src/output/evidence_record.rs::tests::evidence_record_keeps_unresolved_boundary_operands_as_named_limitation`
  maps iterator-derived unresolved boundary operands to
  `analysis/iterator-boundary-operand-resolution`;
- `crates/ripr/src/output/evidence_record.rs::tests::evidence_record_routes_computed_boundary_operands_to_local_computed_limitation`
  maps local/computed unresolved boundary operands to
  `analysis/local-computed-boundary-operand-resolution`;
- `crates/ripr/src/output/evidence_record.rs::tests::evidence_record_routes_member_boundary_operands_to_local_member_limitation`
  maps member-access unresolved boundary operands to
  `analysis/local-member-boundary-operand-resolution`;
- `crates/ripr/src/output/evidence_record.rs::tests::evidence_record_carries_identity_path_guidance_and_calibration_placeholder`
  preserves the normal parameter/literal boundary path as actionable with
  `repair_kind = add_boundary_assertion`.

## Implementation Mapping

Current and planned producers:

- `crates/ripr/src/output/evidence_record.rs` emits evidence-record canonical
  item fields.
- `cargo xtask lane1-evidence-audit` and `cargo xtask actionable-gaps` emit
  Lane 1 canonical counts and repair packets.
- `cargo xtask ripr-swarm plan` consumes actionable packets and blocks missing
  verify, receipt, static-limitation, must-not-change, and allowed-edit-surface
  cases.
- LSP actionable gap queue validation consumes `actionable-gaps` artifacts and
  suppresses unsafe packets.

Planned implementation work must tighten false-actionable analyzer routes before
public surfaces or gates strengthen their semantics.

## Metrics

Relevant metrics:

- `finding_alignment_actionable_items_total`
- `finding_alignment_static_limitation_total`
- `finding_alignment_canonical_items_without_repair_route`
- `finding_alignment_canonical_items_without_verify_command`
- `lane1_actionable_gap_packets`
- `swarm_ready_packets`
- `swarm_missing_verify_command`
- `swarm_missing_receipt_command`
- `swarm_static_limitation_packets`
