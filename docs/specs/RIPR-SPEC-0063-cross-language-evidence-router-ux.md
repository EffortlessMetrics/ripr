# RIPR-SPEC-0063: Cross-Language Evidence Router UX

Status: proposed

Linked specs:

- [RIPR-SPEC-0026: Language adapter contract](RIPR-SPEC-0026-language-adapter-contract.md)
- [RIPR-SPEC-0027: TypeScript preview static facts](RIPR-SPEC-0027-typescript-preview-static-facts.md)
- [RIPR-SPEC-0030: Preview evidence policy boundary](RIPR-SPEC-0030-preview-evidence-policy-boundary.md)
- [RIPR-SPEC-0056: Public actionable projection](RIPR-SPEC-0056-public-actionable-projection.md)
- [RIPR-SPEC-0058: RIPR swarm external agent handoff](RIPR-SPEC-0058-ripr-swarm-external-agent-handoff.md)
- [RIPR-SPEC-0061: Lane 1 canonical actionability contract](RIPR-SPEC-0061-lane1-canonical-actionability-contract.md)
- [RIPR-SPEC-0062: Cross-language oracle graph](RIPR-SPEC-0062-cross-language-oracle-graph.md)

## Problem

RIPR now has a calibrated Bun Blob / ArrayBuffer cross-language oracle graph,
but the user experience is still a collection of graph rows, runbook text, and
dogfood receipts. That is enough for maintainers who watched the lane happen,
but it is not yet a repeatable mixed TypeScript plus Rust operating loop.

The next product problem is narrower than generic TypeScript support:

```text
This Rust or FFI seam changed.
Does the TypeScript integration layer discriminate the changed boundary?
If not, what branch, bridge leg, proof mode, or placement is missing?
```

A good answer must be readable by a Bun operator, precise enough for Claude Code
or another coding agent, and portable enough for other configured TypeScript
plus Rust projects. It must also preserve the current preview boundary: no
runtime execution, no generated tests, no source edits, no public repair packet,
no gate, no badge, and no support-tier promotion from cross-language preview
evidence alone.

## Behavior

### Operator Summary

When a configured Bun UB preview run has cross-language route-quality evidence,
RIPR should expose a compact first-screen summary before the operator has to
inspect raw preview-card JSON.

The summary must answer:

- which calibrated profiles were evaluated;
- which profiles are complete advisory witnesses;
- which profiles are missing a discriminator;
- which profiles are token mentions rather than observers;
- which profiles are blocked by `bridge_unknown`;
- which profiles are named static limitations;
- whether any row is public-repair-packet ready.

The summary must keep the current command surface honest. A Bun operator uses
`ripr.toml` to configure the preview profile, then runs `ripr doctor` and
`ripr check`. This spec does not add or require a `ripr check --profile` flag.

### Cross-Language States

The operator and agent surfaces must use the existing configured states from
RIPR-SPEC-0062 rather than inventing stronger claims:

| State | User meaning |
| --- | --- |
| `rust_ungripped_ts_discriminated` | The configured bridge and TypeScript evidence discriminate the Rust seam. |
| `rust_ungripped_ts_missing_discriminator` | The TypeScript route exists, but a required boundary branch is missing. |
| `ts_mention_not_observer` | A token appears, but it is not a stable observer. |
| `bridge_unknown` | TypeScript evidence exists, but the binding or FFI edge is not credited. |
| named static limitation | The case is visible but not safely actionable. |

No state may use `untested`, `adequate`, `proven`, `killed`, or `survived`.
Complete configured witnesses may say `no_missing_bridge_discriminator`; they
must not say generic `no_new_test_needed`.

### Agent Advisory Packet

For configured preview rows, RIPR should be able to emit a bounded advisory
packet that an external coding agent can consume without re-inferring the seam.

The packet must include these fields when evidence exists:

```text
cross_language_state
rust_file
rust_owner
rust_boundary
ts_test_file
missing_discriminators
suggested_shape
bridge_confidence
missing_graph_legs
proof_mode
authority_boundary
repair_packet_ready
must_not_change
stop_condition
raw_evidence_refs
```

The packet is not a public repair packet unless the later canonical
actionability contract supplies all required public packet fields. Until then,
`repair_packet_ready` remains `false`.

If the state is `bridge_unknown`, the packet must tell the agent to inspect or
add bridge evidence. It must not tell the agent to edit tests.

If the state is `rust_ungripped_ts_missing_discriminator` and typed placement
evidence exists, the packet may suggest an advisory TypeScript file and
discriminator shape. For the configured Bun Blob route, the placement is:

```text
test/js/web/fetch/blob.test.ts
```

### Proof Mode Projection

Stable-byte review needs a proof-mode hint so operators and agents do not
mistake a static witness route for runtime proof. RIPR should project one
advisory proof mode per configured row:

| Proof mode | Meaning |
| --- | --- |
| `observable_red_green` | A real Bun behavior can be shown red on the changed system and green after the fix. |
| `mutation_plus_miri` | The risk is not directly observable and needs mutation, Miri, or a model witness. |
| `helper_gated` | The route is real, but an upstream helper or primitive must land first. |
| `bridge_unknown` | The TypeScript route cannot be credited until the binding or FFI edge is known. |
| `static_limitation` | The row is visible but not safe to turn into a test-edit task. |

The proof mode is advisory metadata. It must not run Bun, Jest, Vitest, Miri, a
mutation engine, or any provider.

### Profile Intake

New Bun stable-byte profiles, and later non-Bun TypeScript plus Rust profiles,
must start from a fixture or corpus contract before analyzer behavior changes.

A profile is not eligible for operator or agent projection until it records:

- Rust seam file, owner, boundary, and source line or span;
- external API or test surface;
- boundary discriminator vocabulary;
- expected oracle strength;
- configured bridge requirement;
- expected state for complete, missing, mention-only, and bridge-unknown or
  named-limitation rows when those rows apply;
- authority boundary and `repair_packet_ready = false`;
- dogfood or manual-verdict receipt before any support claim expands.

Manifest-only profile PRs may pin expected behavior without adding analyzer
logic. They must not create repair packets or placement suggestions unless the
graph has typed placement evidence.

### Bridge Inventory

Before RIPR grows a full Bun binding graph, it should expose a configured bridge
inventory:

```text
Configured:
  Blob ArrayBuffer -> Blob::from_js_without_defer_gc
  copy_to_unshared -> copy_to_unshared
  MarkdownObject -> MarkdownObject::to_string

Missing or future:
  node:fs scalar write sink
  Bun.write sink
  S3 stable-byte sink
```

The inventory is report-only. It must not infer reachability, emit repair
packets, or promote support status.

## Required Evidence

Implementations of this spec must provide:

- a 0.8.1 patch proof packet that records the current Bun Blob / ArrayBuffer
  preview behavior, validation commands, preview boundary, and non-claims;
- a compact Bun UB preview summary built from existing route-quality,
  calibration, and dogfood data, with no new analyzer behavior in that slice;
- JSON and human output contract tests for the agent advisory packet;
- fixture or corpus rows proving `bridge_unknown` emits an inspect-bridge packet
  rather than a test-edit packet;
- output tests proving missing discriminator rows may suggest TypeScript
  placement only when placement evidence exists;
- proof-mode fixture or output tests for `observable_red_green`,
  `mutation_plus_miri`, `helper_gated`, `bridge_unknown`, and
  `static_limitation`;
- manifest-only profile rows for node:fs scalar write and Bun.write before
  analyzer behavior changes; both rows are pinned before bridge inventory or
  analyzer behavior changes;
- a configured bridge inventory report with current configured bridges and
  explicitly missing future surfaces;
- live Bun dogfood receipts with manual verdicts for at least one credited
  witness, one missing discriminator, one mention-only rejection, one
  bridge-unknown row, and one named static limitation;
- documentation showing the copy-pasteable `ripr.toml`, `ripr doctor`, and
  `ripr check` loop;
- a post-0.8.1 support decision that either keeps the claim preview/advisory or
  lists the extra evidence required for any promotion.

## Non-Goals

This spec does not authorize:

- stable TypeScript or JavaScript support;
- a full Bun binding graph;
- TypeScript compiler, `tsserver`, Bun, Jest, Vitest, Miri, or mutation
  execution;
- provider calls;
- generated tests;
- autonomous source edits;
- source release, publish, tag, signing, marketplace, or install-doc work;
- default CI gates;
- public badge, baseline, RIPR Zero, or support-tier contribution;
- public repair packets from preview cross-language evidence;
- generic UB proof or runtime behavior proof;
- generic cross-language support for every mixed-language repository;
- a `ripr check --profile` command-line flag.

## Acceptance Examples

Compact Bun UB preview summary:

```text
Bun UB preview

Calibrated routes:
  Blob SAB/RAB: rust_ungripped_ts_discriminated
  copy_to_unshared: rust_ungripped_ts_discriminated
  MarkdownObject: rust_ungripped_ts_discriminated
  FFI panic boundary: public_reachable_panic_boundary_unrevealed

Current states:
  rust_ungripped_ts_discriminated: 3
  rust_ungripped_ts_missing_discriminator: 0
  bridge_unknown: 0
  ts_mention_not_observer: 0
  named_static_limitation: 2

Authority:
  preview/advisory only
  repair_packet_ready: false
```

Missing discriminator agent packet:

```json
{
  "cross_language_state": "rust_ungripped_ts_missing_discriminator",
  "rust_file": "src/jsc/Blob.rs",
  "rust_owner": "Blob::from_js_without_defer_gc",
  "rust_boundary": "array_buffer.shared || array_buffer.resizable",
  "ts_test_file": "test/js/web/fetch/blob.test.ts",
  "missing_discriminators": ["resizable_array_buffer"],
  "suggested_shape": "new ArrayBuffer(..., { maxByteLength: ... }) through Blob/view with a stable-byte assertion",
  "bridge_confidence": "configured",
  "missing_graph_legs": [],
  "proof_mode": {
    "mode": "observable_red_green",
    "reason": "The missing TypeScript discriminator belongs in an existing bridged stable-byte observer route; future proof should be a system-Bun red/patched-green witness after the discriminator is added.",
    "authority_boundary": "preview_advisory_only",
    "runtime_execution": false,
    "mutation_execution": false,
    "miri_execution": false,
    "proof_claim": false
  },
  "authority_boundary": "preview_advisory_only",
  "repair_packet_ready": false,
  "must_not_change": [
    "Rust production behavior",
    "public API",
    "test framework shape"
  ],
  "stop_condition": "do not edit tests if bridge evidence is removed"
}
```

Bridge-unknown packet:

```json
{
  "cross_language_state": "bridge_unknown",
  "missing_graph_legs": ["binding_or_ffi_edge"],
  "proof_mode": {
    "mode": "bridge_unknown",
    "reason": "The binding or FFI edge is missing, so TypeScript evidence cannot be credited to the Rust seam.",
    "authority_boundary": "preview_advisory_only",
    "runtime_execution": false,
    "mutation_execution": false,
    "miri_execution": false,
    "proof_claim": false
  },
  "next_action": "inspect or add bridge evidence before crediting TypeScript evidence",
  "ts_test_file": null,
  "repair_packet_ready": false
}
```

Manifest-only new profile:

```text
Profile: bun_node_fs_scalar_write
Status: manifest_only
Rust seam: unresolved until fixture audit
TS witness path: test/js/node/fs/fs.test.ts
Proof mode: observable_red_green
State: named_static_limitation
Missing graph legs: binding_or_ffi_edge:node_fs_scalar_write, external_oracle:stable_byte_scalar_write
Projection: not eligible for analyzer credit or repair packets until the bridge and oracle rows are audited
```

Helper-gated manifest profile:

```text
Profile: bun_write_helper_gated
Status: manifest_only
Rust seam: unresolved until fixture audit
TS witness path: test/js/bun/write.test.ts
Proof mode: helper_gated
State: named_static_limitation
Missing graph legs: binding_or_ffi_edge:bun_write_sink, helper:bun_write_fixture_helper, external_oracle:stable_byte_write
Suggested test file: not_applicable
Projection: not eligible for analyzer credit, placement, or repair packets until the helper, bridge, and oracle are audited
```

## Test Mapping

Implemented and planned tests:

- `xtask/src/main.rs::tests::bun_ub_preview_summary_reports_route_quality`
- `xtask/src/main.rs::tests::bun_ub_preview_summary_rejects_public_repair_packets`
- `crates/ripr/src/output/typescript_preview_card.rs::tests::bun_cross_language_agent_packet_projects_missing_discriminator`
- `crates/ripr/src/output/typescript_preview_card.rs::tests::bun_cross_language_agent_packet_projects_bridge_unknown_stop_condition`
- `crates/ripr/src/output/typescript_preview_card.rs::tests::stable_byte_proof_mode_is_advisory`
- `xtask/src/main.rs::tests::cross_language_profile_intake_requires_manifest_rows`
- `xtask/src/main.rs::tests::cross_language_profile_intake_requires_helper_gated_rows`
- `xtask/src/main.rs::tests::configured_bridge_inventory_reports_missing_future_surfaces`
- `xtask/src/main.rs::tests::live_bun_stable_byte_dogfood_receipts_are_checked`

Existing related tests are owned by RIPR-SPEC-0062 and remain the proof for the
current bounded graph behavior. The Bun preview summary, advisory packet,
stable-byte proof-mode rows, and node:fs scalar-write manifest-only intake row
are now implemented; the Bun.write helper-gated manifest-only intake row is
also implemented. Bridge inventory and live dogfood expansion remain planned
until their slices land.

## Implementation Mapping

Implementation surfaces:

- `crates/ripr/src/output/typescript_preview_card.rs` for packet projection and
  proof-mode rendering;
- `crates/ripr/src/output/evidence_record.rs` for shared output fields when the
  packet needs evidence-record projection;
- `xtask/src/reports/bun.rs` for Bun UB preview summary, bridge inventory, and
  dogfood receipt aggregation;
- `xtask/src/main.rs`, `xtask/src/command.rs`, and `xtask/src/dispatch.rs` for
  report command routing if the existing report command surface expands;
- `fixtures/cross-language-oracle-graph-corpus` for profile intake rows;
- `fixtures/bun-ub-cross-language-dogfood` for live review receipts;
- `docs/BUN_UB_TYPESCRIPT_PREVIEW_RUNBOOK.md` for operator workflow updates.

## Metrics

Track:

- calibrated cross-language routes evaluated;
- complete advisory witnesses;
- missing discriminator limitations;
- mention-only rejections;
- bridge-unknown limitations;
- named static limitations;
- advisory TypeScript placements emitted;
- bridge inventory configured and missing profiles;
- proof-mode distribution;
- agent packets emitted with `repair_packet_ready = false`;
- public repair packets emitted from cross-language preview evidence, which must
  remain zero until a later accepted contract changes that boundary.
