# RIPR-SPEC-0062: Cross-Language Oracle Graph

Status: accepted

Linked specs:

- [RIPR-SPEC-0026: Language adapter contract](RIPR-SPEC-0026-language-adapter-contract.md)
- [RIPR-SPEC-0027: TypeScript preview static facts](RIPR-SPEC-0027-typescript-preview-static-facts.md)
- [RIPR-SPEC-0030: Preview evidence policy boundary](RIPR-SPEC-0030-preview-evidence-policy-boundary.md)
- [RIPR-SPEC-0044: Preview evidence promotion packet](RIPR-SPEC-0044-preview-evidence-promotion-packet.md)
- [RIPR-SPEC-0056: Public actionable projection](RIPR-SPEC-0056-public-actionable-projection.md)
- [RIPR-SPEC-0057: RIPR swarm repair loop](RIPR-SPEC-0057-ripr-swarm-repair-loop.md)
- [RIPR-SPEC-0058: RIPR swarm external agent handoff](RIPR-SPEC-0058-ripr-swarm-external-agent-handoff.md)
- [RIPR-SPEC-0061: Lane 1 canonical actionability contract](RIPR-SPEC-0061-lane1-canonical-actionability-contract.md)

Linked issues:

- [#908: Cross-language repos report externally covered Rust seams as weakly gripped](https://github.com/EffortlessMetrics/ripr-swarm/issues/908)
- [#910: TS-tested Rust seams reported as ungripped](https://github.com/EffortlessMetrics/ripr-swarm/issues/910)

## Problem

Mixed Rust plus TypeScript repositories can exercise Rust behavior through
binding, FFI, generated host functions, or JavaScript builtins while the visible
test oracle lives in an external test suite. If RIPR sees only the Rust seam, it
can misread externally discriminated behavior as `weakly_gripped` or
`ungripped`, then suggest a Rust test in the wrong crate or language.

RIPR needs an explicit cross-language oracle graph before external test evidence
can affect actionability. The first supported contract is deliberately bounded
to the configured Bun Blob / ArrayBuffer route from #910:

```text
Rust seam
  -> binding or FFI edge
    -> external TypeScript callsite
      -> external TypeScript assertion or observer
        -> advisory witness or named limitation
```

The graph exists to prevent false precision. Complete configured witnesses may
explain why no new test is suggested, but they are not public repair packets.
Incomplete witnesses must become named limitations with samples and unlock
conditions.

## Behavior

### Scope

This spec defines the cross-language oracle graph shape for one configured Bun
Blob route:

- Rust seam file: `src/jsc/Blob.rs`
- Rust owner: `Blob::from_js_without_defer_gc`
- Rust boundary: `array_buffer.shared || array_buffer.resizable`
- External test file: `test/js/web/fetch/blob.test.ts`
- External entrypoints: `new Blob` and `blob.arrayBuffer`

The same vocabulary may be reused by later graph profiles, but this spec does
not claim generic cross-language reachability.

### Required Graph Legs

A cross-language oracle graph is complete only when every leg below has typed
evidence and at least one structured raw evidence reference:

| Leg | Required fields |
| --- | --- |
| Rust seam | `source_file`, `line` or span, `owner`, `boundary`, `seam_id` or `canonical_gap_id` when available |
| Boundary discriminator | each branch or boundary value required by the Rust predicate, including `shared_array_buffer` and `resizable_array_buffer` for the Bun Blob route |
| Binding or FFI edge | configured bridge hint or later generated bridge fact naming how the external surface reaches the Rust owner |
| External callsite | `language`, `language_status`, `source_file`, `line` or span, test name or callsite, and the external entrypoint that reaches the bridge |
| External assertion or oracle | observer kind, oracle strength, asserted value or stable observer shape, and discriminator coverage |
| Authority boundary | `authority_boundary = preview_advisory_only` and `repair_packet_ready = false` until the canonical actionability fields in RIPR-SPEC-0061 are present |

Raw evidence references must be structured enough for downstream reports to
trace the claim back to source. A prose-only note, duplicate count, or
placeholder reference does not satisfy this spec.

### Allowed States

The configured Bun Blob route may produce these states:

| Cross-language state | Canonical state | Meaning | Required route |
| --- | --- | --- | --- |
| `rust_ungripped_ts_discriminated` | `already_observed` or advisory external observation | The configured bridge and TypeScript evidence include both `SharedArrayBuffer` and resizable `ArrayBuffer` discriminators plus a stable Blob byte observer. | No repair packet; keep the witness advisory and manually review the Bun change. |
| `rust_ungripped_ts_missing_discriminator` | `static_limitation` | A configured bridge and observer exist, but at least one required discriminator is absent. | `analysis/cross-language-oracle-visibility` |
| `rust_ungripped_ts_missing_external_oracle` | `static_limitation` | A configured bridge and partial TypeScript Blob observer path exist, but the external callsite or stable-byte oracle edge is incomplete. | `analysis/cross-language-oracle-visibility` |
| `ts_mention_not_observer` | `static_limitation` | TypeScript tokens such as `maxByteLength` appear without a Blob input and stable-byte observer. | `analysis/cross-language-oracle-visibility` |
| `bridge_unknown` | `static_limitation` | External TypeScript discriminators may exist, but no configured or generated bridge ties them to the Rust owner. | `analysis/cross-language-oracle-visibility` |
| `cross_language_target_unresolved` | `static_limitation` | The oracle graph does not identify a safe test placement or observer target. | `analysis/cross-language-test-target-inference` |

`rust_ungripped_ts_discriminated` is not an actionable repair. It can suppress a
wrong "add a Rust test" suggestion for the configured route, but it must not
emit `repair_kind`, `verify_command`, `receipt_command`, `allowed_edit_surface`,
or public projection eligibility unless a later contract supplies the full
repair-packet fields.

### Fail-Closed Rules

RIPR must fail closed when any graph leg is missing:

- missing binding or FFI edge -> `bridge_unknown`;
- missing external callsite or stable-byte oracle on a partial observer path ->
  `rust_ungripped_ts_missing_external_oracle`;
- mention-only external evidence -> `ts_mention_not_observer`;
- missing `shared_array_buffer` or `resizable_array_buffer` discriminator ->
  `rust_ungripped_ts_missing_discriminator`;
- missing source location or raw evidence refs -> static limitation with the
  missing field named;
- missing target placement -> `cross_language_target_unresolved`.

The report must not fall back to `no_static_path`, generic `weakly_gripped`
remediation, a guessed Rust test file, or a TypeScript test target unless the
graph has explicit typed evidence for that route.

### Public Projection

Badge, LSP, PR, CI, readiness, scorecard, and swarm consumers must treat this
graph as advisory or limitation evidence until the canonical public packet
contract is satisfied. A cross-language item remains excluded from public repair
queues when any of these fields is missing:

- `canonical_gap_id`
- `repair_kind`
- `target_test_shape` or typed external observer shape
- `verify_command`
- `receipt_command`
- `allowed_edit_surface[]`
- `must_not_change[]`
- `confidence`
- `raw_evidence_refs[]`

No consumer may turn a complete advisory TypeScript witness into a generated
test, autonomous edit, badge count, blocking CI condition, baseline result, RIPR
Zero claim, or support-tier promotion.

## Required Evidence

Implementations of this spec must provide:

- a fixture or corpus row for each configured Bun Blob state:
  `rust_ungripped_ts_discriminated`,
  `rust_ungripped_ts_missing_discriminator`,
  `rust_ungripped_ts_missing_external_oracle`, `ts_mention_not_observer`, and
  `bridge_unknown`;
- source samples naming the Rust seam, boundary, external TypeScript callsite,
  external assertion or observer, and configured bridge evidence where present;
- raw evidence references for each credited graph leg;
- JSON and Markdown projection that preserve the state, limitation category,
  repair route, missing discriminator list, source locations, and non-claims;
- fail-closed proof that missing graph legs do not create public repair packets,
  suggested Rust tests, verify commands, receipt commands, allowed edit
  surfaces, or public projection eligibility;
- readiness and route-quality proof that cross-language oracle limitations stay
  in limitation backlogs until actionability fields are present.

## Non-Goals

This spec does not authorize:

- generic cross-language reachability for every Rust, TypeScript, JavaScript,
  Python, C, C++, or FFI surface;
- TypeScript, JavaScript, or Bun runtime execution;
- `tsc`, `tsserver`, generated host-function analysis, or package graph
  resolution as a default requirement;
- generated tests;
- autonomous source edits;
- provider integration;
- mutation execution;
- public badge semantic changes;
- CI blocking by default;
- baseline, RIPR Zero, support-tier, or release-publish claims;
- treating preview TypeScript evidence as Rust parity;
- suggesting unrelated Rust test files or external-language test files from
  incomplete graph evidence;
- claiming runtime mutation outcomes, adequacy, sufficiency, or proof language
  such as `killed`, `survived`, `proven`, `adequate`, or `untested`.

## Acceptance Examples

Configured Bun Blob witness with both discriminators:

```text
rust_file = src/jsc/Blob.rs
rust_owner = Blob::from_js_without_defer_gc
rust_boundary = array_buffer.shared || array_buffer.resizable
ts_test_file = test/js/web/fetch/blob.test.ts
observed_ts_facts = shared_array_buffer, resizable_array_buffer,
  view_backed_blob_input, stable_byte_copy_oracle
bridge_confidence = configured_hint
state = rust_ungripped_ts_discriminated
gap_state = already_observed
repair_packet_ready = false
suggested_test_file = not_applicable
```

Expected result: RIPR may display an advisory external observation and must not
suggest a Rust test, create a public packet, or count the item as repair-ready.

Configured bridge with missing resizable discriminator:

```text
observed_ts_facts = shared_array_buffer, view_backed_blob_input,
  stable_byte_copy_oracle
missing_discriminators = resizable_array_buffer
state = rust_ungripped_ts_missing_discriminator
gap_state = static_limitation
category = cross_language_oracle_visibility_unresolved
repair_route = analysis/cross-language-oracle-visibility
repair_packet_ready = false
suggested_test_file = not_applicable
```

Expected result: the item becomes analyzer backlog, not a Rust test suggestion.

Configured bridge with partial external observer and missing stable oracle:

```text
observed_ts_facts = shared_array_buffer, resizable_array_buffer,
  view_backed_blob_input
missing_graph_legs = external_oracle:stable_byte_copy
state = rust_ungripped_ts_missing_external_oracle
gap_state = static_limitation
category = cross_language_oracle_visibility_unresolved
repair_route = analysis/cross-language-oracle-visibility
repair_packet_ready = false
suggested_test_file = not_applicable
```

Expected result: the item names the missing external oracle leg and remains
an analyzer limitation, not a Rust or TypeScript repair packet.

Mention-only TypeScript evidence:

```text
observed_ts_facts = max_byte_length_mention_only
state = ts_mention_not_observer
gap_state = static_limitation
repair_route = analysis/cross-language-oracle-visibility
```

Expected result: RIPR does not credit the token mention as an observer.

Discriminated TypeScript evidence with no bridge:

```text
observed_ts_facts = shared_array_buffer, resizable_array_buffer,
  view_backed_blob_input, stable_byte_copy_oracle
bridge_confidence = unknown
state = bridge_unknown
gap_state = static_limitation
repair_route = analysis/cross-language-oracle-visibility
```

Expected result: RIPR reports the missing bridge route instead of flattening the
case to `no_static_path` or inventing remediation.

## Test Mapping

Current supporting proof:

- `fixtures/cross-language-oracle-graph-corpus/corpus.json`
- `xtask/src/main.rs::tests::cross_language_oracle_graph_corpus_cases_are_checked`
- `xtask/src/main.rs::tests::cross_language_oracle_route_quality_summarizes_corpus_cases`
- `xtask/src/main.rs::tests::bun_ub_calibration_report_summarizes_calibrated_states`
- `xtask/src/main.rs::tests::bun_ub_calibration_command_writes_markdown_and_json`
- `xtask/src/main.rs::tests::ripr_swarm_readiness_rolls_up_plan_and_outcomes`
- `xtask/src/main.rs::tests::evidence_quality_scorecard_summarizes_cross_language_oracle_route_quality`
- `xtask/src/main.rs::tests::cross_language_oracle_graph_rejects_actionability_and_location_holes`
- `crates/ripr/src/analysis/language/typescript.rs::tests::changed_rust_blob_boundary_projects_ts_discriminated_cross_language_grip`
- `crates/ripr/src/analysis/language/typescript.rs::tests::changed_rust_blob_boundary_projects_missing_resizable_cross_language_grip`
- `crates/ripr/src/analysis/language/typescript.rs::tests::changed_rust_blob_boundary_with_unknown_bridge_stays_limitation`
- `crates/ripr/src/output/typescript_preview_card.rs::tests::typescript_preview_card_projects_bun_cross_language_grip`
- `crates/ripr/src/output/typescript_preview_card.rs::tests::typescript_preview_card_projects_bridge_unknown_without_binding_ref`
- `xtask/src/main.rs::tests::typescript_bun_ub_calibration_cases_are_checked`
- `crates/ripr/src/lsp/tests.rs::gap_code_actions_suppress_repair_actions_for_cross_language_target_unresolved`
- `crates/ripr/src/lsp/gap_artifacts.rs::tests::actionable_gaps_report_rejects_cross_language_target_unresolved_packet`

Route-quality proof:

- `report/cross-language-oracle-route-quality` keeps readiness and scorecard
  summaries aligned with complete advisory witnesses, missing discriminator
  limitations, mention-only limitations, unknown bridge limitations, and public
  packet exclusions.

## Implementation Mapping

Current implementation surfaces:

- `crates/ripr/src/analysis/language/typescript.rs` emits configured Bun Blob
  cross-language preview evidence lines with graph-leg raw refs, missing graph
  legs, and unlock conditions for limitation states. Credited configured
  bridge evidence uses the `binding_edge` raw-ref leg; `bridge_unknown` omits
  that raw ref and names missing `binding_or_ffi_edge` instead.
- `crates/ripr/src/output/typescript_preview_card.rs` projects the advisory
  TypeScript preview card, including Bun cross-language limitation category,
  route, graph legs, unlock condition, and raw refs while keeping
  `repair_packet_ready=false`.
- `crates/ripr/src/output/review_comments.rs`, LSP gap artifacts, readiness,
  and scorecard code already suppress repair actions when target placement or
  public packet fields are unresolved.
- `xtask/src/main.rs` projects the SPEC-0062 corpus into
  `cross_language_oracle_route_quality` sections in
  `target/ripr/reports/swarm-readiness.{json,md}` and
  `target/ripr/reports/evidence-quality-scorecard.{json,md}`, including
  complete advisory witnesses, missing discriminator limitations, mention-only
  limitations, unknown bridge limitations, public packet exclusions, missing
  graph legs, unlock conditions, and `repair_packet_ready=false`.
- `cargo xtask bun-ub-calibration` writes
  `target/ripr/reports/bun-ub-calibration.{json,md}` from the Bun Blob
  calibration corpus, deriving observed TS-discriminated, missing
  discriminator, mention-not-observer, and bridge-unknown states without
  creating public repair packets or runtime proof claims.
- `fixtures/typescript-bun-ub-calibration/corpus.json` records the existing
  Bun Blob calibration cases.
- `fixtures/cross-language-oracle-graph-corpus/corpus.json` records the
  SPEC-0062 graph states, source locations, raw refs, limitation routes, target
  exclusions, and non-claims before analyzer behavior changes.
- `docs/CONFIGURATION.md` documents `[profiles.bun_ub]` as advisory operator
  configuration for the existing Bun Blob profile.
- `docs/OUTPUT_SCHEMA.md` documents `preview_actionability` and
  `typescript_preview_card` projection fields.
- `docs/handoffs/2026-06-03-bun-ub-typescript-preview-closeout.md`,
  `docs/handoffs/2026-06-03-lane1-real-repo-trust-readiness-closeout.md`, and
  `docs/handoffs/2026-06-03-lane1-language-aware-placement-navigation-closeout.md`
  record the existing preview, real-repo trust, and placement boundaries that
  this follow-up preserves.

Follow-up implementation must stay within the active
`lane1-cross-language-oracle-graph-readiness` campaign and must not change
source release, publish, signing, marketplace, install-doc, provider,
autonomous-edit, mutation, badge, or default CI behavior.

## Metrics

Current related metrics:

- `language_adapter_typescript_bun_ub_array_buffer_facts`
- `language_adapter_typescript_bun_ub_stable_byte_oracle_facts`
- `language_adapter_typescript_bun_ub_bridge_hints`
- `language_adapter_typescript_bun_ub_cross_language_grip_states`
- `cross_language_oracle_visibility_unresolved_signals`
- `cross_language_oracle_visibility_route_signals`
- `cross_language_target_unresolved_signals`
- `cross_language_test_target_inference_route_signals`
- `cross_language_projection_exclusions`
- `cross_language_oracle_graph_complete_advisory_witnesses`
- `cross_language_oracle_graph_missing_discriminator_limitations`
- `cross_language_oracle_graph_missing_external_oracle_limitations`
- `cross_language_oracle_graph_bridge_unknown_limitations`
- `cross_language_oracle_graph_mention_only_limitations`
- `cross_language_oracle_graph_public_packet_exclusions`
