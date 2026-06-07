# RIPR-SPEC-0071: TypeScript/Bun Evidence Use Case

Status: proposed

Owner: product / swarm

Created: 2026-06-06

Linked proposal:

- None yet

Linked ADRs:

- None yet

Linked plan:

- plans/use-case-specs/implementation-plan.md (planned)

Linked issues:

- None yet

Linked PRs:

- None yet

Support-tier impact:

- None. TypeScript and JavaScript remain opt-in preview, per the
  post-0.8.1 support decision
  (docs/handoffs/2026-06-05-post-081-typescript-bun-support-decision.md).
  Every surface this spec describes keeps `language_status =
  "preview"` and `authority_boundary = "preview_advisory_only"`, and
  contributes nothing to default gates, public badges, baselines, or
  RIPR Zero. Promotion requires a separate accepted promotion
  contract.
- Claim boundaries for this surface are governed by the canonical ledger in [support tiers](../status/SUPPORT_TIERS.md); nothing here promotes a tier.

Policy impact:

- Register this spec in `policy/doc-artifacts.toml`.
- No new crate, binary, dependency, parser, runtime executor, or LSP
  server is introduced by this spec.

## Problem

Mixed TypeScript/Rust repositories — Bun above all — change Rust and
native seams whose only observers are TypeScript tests. ripr already
has the pieces to make that evidence visible: the syntax-first
TypeScript preview adapter (RIPR-SPEC-0027, accepted), the
`typescript_preview_card.v1` projection, the Bun cross-language grip
with its advisory packet and stable-byte proof mode, and the
configured bridge inventory report
(`cargo xtask configured-bridge-inventory`).

What is missing is the use-case contract: which TypeScript facts the
adapter promises to detect, the closed list of named limitations it
fails into, and the exact field set a public TypeScript repair packet
would require. Without that contract, two failure shapes appear:
preview evidence quietly hardens into cross-language certainty, or
useful TypeScript oracles stay invisible because nothing says they
may be shown.

The post-0.8.1 decision fixed the claim this spec operationalizes.
Its Current Claim, quoted:

```text
This Rust/FFI stable-byte seam changed.
Does configured TypeScript integration evidence discriminate the
boundary?
```

The answer is advisory and calibrated for the currently modeled Bun
stable-byte routes. It can say the TypeScript route is discriminated,
a named discriminator is missing, a token is only a mention, a bridge
is unknown, or a named static limitation blocks actionability.

## Behavior

```text
User question:
Can ripr make TypeScript and Bun evidence visible without
faking cross-language certainty?
```

### Adapter promise

The TypeScript adapter promises, syntax-first and without `tsc`,
`tsserver`, package graphs, or runtime tooling, to:

- detect TypeScript/JavaScript test files and test blocks;
- identify framework hints from the closed supported list below;
- extract basic oracles (exact-value, error-path, smoke, snapshot,
  mock-interaction) per RIPR-SPEC-0027;
- infer bounded verify commands where the runner is obvious from the
  framework hint, emitted as command text only, never executed;
- emit repair packets only when the complete packet contract below is
  satisfied — field completeness is necessary but not sufficient for
  public emission (see the packet contract below);
- fail closed into a named limitation category otherwise.

### Supported v1 vocabulary (closed list)

Framework hints:

- `bun test`
- `vitest`
- `jest`
- `node:test`

Oracle shapes:

- `expect(...).toBe(...)`, `expect(...).toEqual(...)`,
  `expect(...).toThrow(...)`
- `assert.equal(...)`, `assert.strictEqual(...)`,
  `assert.deepStrictEqual(...)`
- `t.equal(...)`, `t.deepEqual(...)`

Anything outside this list is not silently coerced into a supported
shape; it routes to a named limitation. This list extends the
RIPR-SPEC-0027 jest/vitest vocabulary with `bun test`, `node:test`,
`assert.*`, and `t.*` shapes; the extension is part of this use-case
contract and lands through the planned TypeScript adapter slice.

### Limitation categories (closed list)

Each category is a named fail-closed route, never an empty result:

| Category | Existing anchor |
| --- | --- |
| `unknown_framework` | new named category; nearest existing vocab is `static_limit_kind = "unsupported_syntax"` |
| `dynamic_helper` | `static_limit_kind` `dynamic_dispatch` / `mocked_module` family |
| `opaque_matcher` | new named category; nearest existing vocab is `static_limit_kind = "opaque_custom_assertion_helper"` — oracle present but outside the supported shapes; stays weak, no repair shape |
| `unresolved_bridge` | `bridge_unknown` state / `binding_or_ffi_edge` missing graph leg |
| `missing_verify_command` | `missing_actionability_fields: verify_command` |
| `cross_language_oracle_unresolved` | `cross_language_oracle_visibility_unresolved` with repair route `analysis/cross-language-oracle-visibility` |

### Projection surface

The public projection is the existing
`card_version = "typescript_preview_card.v1"` card with
`language_status = "preview"`, `oracle_kind`, `oracle_strength`,
`missing_discriminator`, `suggested_assertion_shape`,
`why_not_actionable`, `repair_route`, `repair_packet_ready`, and
`limits[]`. For configured Bun routes the card carries
`bun_cross_language_grip` with `state`, the nested Rust seam triple
(`rust_seam.file`, `rust_seam.owner`, `rust_seam.boundary`), the
nested TypeScript evidence block (`typescript_evidence.test_file`,
`typescript_evidence.verdict`,
`typescript_evidence.bridge_confidence`,
`typescript_evidence.missing_discriminators[]`),
`limitation_category`, `repair_route`, `missing_graph_legs[]`,
`unlock_condition`, `raw_evidence_refs[]`, `action`,
`suggested_test_file`, ranked `placement`, `authority_boundary`, and
`repair_packet_ready`. The flat names (`rust_file`, `rust_owner`,
`rust_boundary`, `ts_test_file`, `ts_verdict`) are the internal
struct shape; in public card JSON they appear only inside the nested
`advisory_packet` object, per the `typescript_preview_card.v1`
section of `docs/OUTPUT_SCHEMA.md`.

Grip states are the closed set from the post-0.8.1 decision:
`rust_ungripped_ts_discriminated`,
`rust_ungripped_ts_missing_discriminator`, `ts_mention_not_observer`,
`bridge_unknown`, plus the named static-limitation route
`rust_ungripped_ts_missing_external_oracle` — the same five states
the Required Evidence fixtures enumerate. No other grip state ships
without amending this spec.

Every grip projects `TypeScriptBunStableByteProofMode`: `mode`
(`observable_red_green`, `mutation_plus_miri`, `helper_gated`,
`bridge_unknown`, `static_limitation`), a `reason`,
`authority_boundary = "preview_advisory_only"`, and the four
non-claim booleans `runtime_execution = false`,
`mutation_execution = false`, `miri_execution = false`,
`proof_claim = false`. Proof mode is a planning label; it is never a
statement that the proof happened.

The configured bridge inventory report
(`target/ripr/reports/configured-bridge-inventory.{json,md}`) lists
configured, bridge-unknown, manifest-only, and named-limitation
surfaces without analyzer inference; it is the operator's map of
which Bun routes are modeled at all.

### Public repair packet contract

A public TypeScript repair packet inherits the RIPR-SPEC-0061 repair
packet requirements in full; this list restates them and adds the
TypeScript-specific fields (`language` with `language_status`, and
the explicit `gap_state`). It requires every one of:

- `packet_id`
- `canonical_gap_id`
- `language` (with `language_status`)
- `gap_state = "actionable"`
- `repair_kind`
- target shape (target test file/type plus suggested assertion)
- `related_test_or_observer`
- `verify_command`
- `receipt_command`
- `allowed_edit_surface`
- `must_not_change`
- `confidence`
- `raw_evidence_refs[]`, structured per RIPR-SPEC-0061: each
  reference carries an anchor field (`file`, `path`, or
  `source_file`) and an identity field (`kind`, `source_id`,
  `evidence_record_ref`, or `canonical_gap_id`); placeholder refs do
  not satisfy the requirement

Satisfying this field contract is necessary but not sufficient for
public emission. The post-0.8.1 decision lists public repair packets
from TypeScript/Bun preview evidence as a Non-Claim: emitting them
additionally requires a separate accepted promotion contract,
regardless of field completeness. Anything missing fails closed into
one of the named limitation categories above, recorded in
`missing_actionability_fields`, with `repair_packet_ready = false`
and `public_repair_packet = false`. No current TypeScript/Bun surface
satisfies this contract; the Bun advisory packet
(`bun_cross_language_advisory_packet.v1`) exists precisely to carry
the route, stop condition, and must-not-change constraints while
staying non-public.

### Non-claims

Adapted from the post-0.8.1 decision, which is the authoritative
wording; this use case claims none of:

- full TypeScript or JavaScript semantic support, type checking, or
  stable support tier;
- a full Bun binding graph, or generic cross-language support for
  every mixed TypeScript/Rust repository;
- generated tests or source edits;
- runtime claims of any kind unless a verify command actually
  executed and was separately recorded — ripr itself runs no Bun,
  Jest, Vitest, `tsc`, `tsserver`, Miri, or mutation execution;
- support-tier promotion from preview cards, advisory packets, or
  proof-mode labels;
- Bun UB conclusions in either direction.

Required wording: "advisory preview evidence", "named limitation",
"bridge unknown". Forbidden wording for these surfaces: "supported",
"verified at runtime", "UB-free", "memory-safe", "no static path"
when the true state is an unknown bridge.

## Non-Goals

- No `tsc`, `tsserver`, type inference, `package.json` dependency
  graph, bundler, or sourcemap integration (RIPR-SPEC-0027
  boundaries hold).
- No execution of `bun test`, `vitest`, `jest`, or `node:test`;
  framework hints and verify commands are static facts.
- No generated tests, source edits, or provider calls.
- No default gate, badge, baseline, or RIPR Zero contribution.
- No new public output surface beyond the existing
  `typescript_preview_card.v1` projection; the supported-vocabulary
  extension changes adapter facts, not card shape.
- No reinterpretation authority for downstream consumers
  (RIPR-SPEC-0070 owns the downstream contract).

## Required Evidence

- This spec registered in `docs/specs/README.md` and
  `policy/doc-artifacts.toml`.
- Fixtures pinning detection of each supported framework hint
  (`bun test`, `vitest`, `jest`, `node:test`).
- Fixtures pinning each supported oracle shape (`expect` family,
  `assert` family, `t.equal` / `t.deepEqual`).
- Fixtures pinning each limitation category, each emitting the named
  category rather than an empty or coerced result.
- Fixtures pinning bounded verify-command inference per framework,
  with command text only and no execution.
- A fail-closed fixture per missing packet field showing the packet
  contract refuses partial emission.
- Bun grip fixtures for `rust_ungripped_ts_discriminated`,
  `rust_ungripped_ts_missing_discriminator`,
  `ts_mention_not_observer`, `bridge_unknown`, and
  `rust_ungripped_ts_missing_external_oracle`, asserting proof-mode
  booleans stay false.
- The configured bridge inventory and
  `fixtures/bun-ub-cross-language-dogfood` receipts cited as the
  calibration evidence base.

Fail-closed reject list — the adapter and its projections must refuse
to render any of these as supported success:

- a framework outside the closed list presented as a framework hint
  (`unknown_framework` is the only allowed output);
- a helper-wrapped or dynamically dispatched assertion presented as a
  direct oracle (`dynamic_helper`);
- a matcher outside the supported shapes presented as exact-value or
  error-path evidence (`opaque_matcher`);
- TypeScript evidence credited to a Rust seam without a configured or
  derived binding edge (`unresolved_bridge`);
- an actionable state without a verify command
  (`missing_verify_command`);
- a cross-language route with an incomplete observer/oracle graph
  presented as discriminated (`cross_language_oracle_unresolved`);
- any repair packet missing one or more required packet fields;
- any preview card, advisory packet, or proof-mode label presented as
  support-tier evidence or runtime result;
- token mentions presented as observers (`ts_mention_not_observer`
  stays a limitation until callsite and oracle legs are credited).

## Acceptance Examples

### Supported oracle made visible

A changed exported function has a `vitest` test asserting
`expect(applyDiscount(100, 50)).toBe(90)`. The card shows
`oracle_kind = "exact_value"`, a missing boundary discriminator if
the changed predicate lacks one, a suggested assertion shape, and a
bounded verify command such as `npx vitest run <file>` as text. No
repair packet is emitted; `repair_packet_ready = false` with the
missing packet fields named.

### Unknown framework fails closed

A test file uses a bespoke harness with `check(...)` helpers. The
adapter emits `unknown_framework` with no oracle credit and no verify
command; the card explains the route to support (add the framework to
a future supported list) instead of guessing.

### Missing Bun discriminator routed, not promoted

The configured Blob route reports
`rust_ungripped_ts_missing_discriminator` with
`missing_discriminators = ["resizable_array_buffer"]`, placement rank
1 in `test/js/web/fetch/blob.test.ts`, `proof_mode.mode =
"observable_red_green"`, and `proof_claim = false`. The advisory
packet names the suggested shape (`maxByteLength` case with a
stable-byte assertion), the stop condition, and `must_not_change`
including Rust production behavior — and stays
`public_repair_packet = false`.

### Bridge unknown stays bridge unknown

TypeScript discriminators and a stable-byte oracle exist, but no
binding edge is configured. State is `bridge_unknown`, next action is
`inspect_or_add_bridge_evidence`, and the output never reads as
`no_static_path` and never credits the TypeScript tests to the seam.

### Mention is not an observer

A test file mentions a byte length in a comment and reads a value
without asserting it. State is `ts_mention_not_observer`; proof mode
is `static_limitation`; the suggested route requires observer and
bridge evidence before any credit.

## Test Mapping

- None yet.

This spec is docs-only. Implementation slices add traceability
entries when adapter behavior lands; mapping names should follow
`analysis/typescript-adapter-frameworks`,
`analysis/typescript-adapter-limitations`, and
`output/typescript-repair-packet-fail-closed`, alongside the existing
RIPR-SPEC-0027 fixture corpus and the Bun calibration tests in
`xtask/src/main.rs`.

## Implementation Mapping

- docs/specs/RIPR-SPEC-0071-typescript-bun-evidence-use-case.md —
  this document.
- plans/use-case-specs/implementation-plan.md (planned) — the
  "TypeScript adapter" slice: framework-hint extension (`bun test`,
  `node:test`, `assert.*`, `t.*`), the six named limitation
  categories, bounded verify-command inference, and the fail-closed
  packet-contract fixtures, sequenced after the spec set lands.

## Metrics

- Framework-hint detection counts per supported framework, extending
  the `language_adapter_typescript_*` metric family from
  RIPR-SPEC-0027.
- Limitation-category distribution across the six named categories;
  an unexplained empty result counts as a defect, not a metric gap.
- Verify-command inference coverage: supported-framework findings
  with a bounded command versus `missing_verify_command`.
- Repair-packet fail-closed rate: packets refused per missing field;
  the public-packet emission count must remain zero until a separate
  promotion contract is accepted (per the post-0.8.1 decision), not
  merely until the full field contract is satisfiable.
- Bun grip-state distribution
  (`language_adapter_typescript_bun_ub_cross_language_grip_states`)
  with proof-mode booleans audited to remain false.
- Promotion rule: any stronger TypeScript/Bun claim requires a
  separate accepted promotion contract that names the exact support
  tier requested, the additional Bun surfaces beyond the calibrated
  routes, bridge or binding evidence that is no longer manifest-only,
  false-positive and false-negative review evidence across live Bun
  changes, how runtime, mutation, Miri, or red/green evidence is
  represented when required, and why public repair-packet, gate,
  badge, baseline, or support-tier authority is safe for that
  narrower surface. Until such a contract is accepted, this use case
  stays preview/advisory and this spec may move to accepted only on
  the strength of its fixtures, not on any support-tier movement.

## Failure Modes

- Supported-list creep: a new matcher or framework slips in without a
  spec update — the closed lists here are the contract; detection of
  unlisted shapes is a defect even when the guess is right.
- Verify-command overreach: an inferred command for an ambiguous
  runner setup — bounded inference only; ambiguity routes to
  `missing_verify_command`.
- Grip-state coercion: `bridge_unknown` or
  `ts_mention_not_observer` flattened into a weaker generic state —
  each state has its own fixture and its own route.
- Proof-mode misread: a consumer treats `observable_red_green` as a
  completed witness — the four false booleans and
  `proof_claim = false` travel with every projection.
- Packet leakage: an advisory packet serialized where a public repair
  packet is expected — `public_repair_packet = false` is binding and
  fixture-pinned.
- Decision-doc drift: claims here diverge from the post-0.8.1
  handoff — that document's Claim and Non-Claims are authoritative;
  a conflict is resolved in its favor and this spec is corrected.
