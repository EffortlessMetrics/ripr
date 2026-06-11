# RIPR-SPEC-0070: Downstream Review Consumer Use Case

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

- [#1041](https://github.com/EffortlessMetrics/ripr-swarm/issues/1041)
  (contract alignment with unsafe-review)

Linked PRs:

- None yet

Support-tier impact:

- None. This spec defines how downstream review tools consume evidence
  that ripr already emits. It promotes no language, surface, or evidence
  class. TypeScript/JavaScript and Bun cross-language evidence remain
  opt-in preview with `authority_boundary = "preview_advisory_only"`,
  and Lane 1 limited completeness runs remain
  `downstream_consumable = false` (diff-scoped output is consumable
  only for its named scope, never as repo totals). The canonical
  support-tier boundary is unchanged.
- Claim boundaries for this surface are governed by the canonical ledger in [support tiers](../status/SUPPORT_TIERS.md); nothing here promotes a tier.

Policy impact:

- Register this spec in `policy/doc-artifacts.toml`.
- No new crate, binary, dependency, parser, runtime executor, export
  format, or LSP server is introduced by this spec.

## Problem

unsafe-review and ub-review want ripr evidence as an input to their own
packets. Today the only written guidance is the raw `findings[]` array,
which forces downstream tools to reinterpret probe internals, re-derive
gap state, and guess at completeness. That recreates analyzer truth
outside ripr and makes overclaiming easy: a partial run, a preview
card, or an unresolved bridge can silently become a downstream claim.

ripr already has the canonical layer this use case needs:

- `ripr check --json` emits `schema_version = "0.1"`, a `summary`
  restricted to the static vocabulary (`exposed`, `weakly_exposed`,
  `reachable_unrevealed`, `no_static_path`, `infection_unknown`,
  `propagation_unknown`, `static_unknown`), and an additive
  `finding_alignment` section whose `items[]` carry
  `canonical_gap_id`, `canonical_item_kind` (`limitation` | `gap`),
  `evidence_class`, `gap_state`, `primary_anchor`, `raw_spans[]`,
  `repair_route`, `static_limitations[]`, `confidence`,
  `verify_command`, and `related_test`.
- `seams[].evidence_record` (`EVIDENCE_RECORD_SCHEMA_VERSION = "0.1"`)
  carries `seam_id`, `canonical_gap_id`, `canonical_item`,
  `raw_findings[]`, `grip_class`, `evidence_path`, `actionability`,
  and `calibration`.
- Gap-ledger `GapRecord` carries per-surface `projection_eligibility`,
  `verification_commands[]`, `receipt_command`, and
  `safe_gate_predicate`.

Separately, unsafe-review has published its own requirements rail for
ripr (`docs/dogfood/ripr-bun-diff-first-requirements.md` in
unsafe-review-swarm). Without a spec that maps each rail requirement to
a concrete ripr field or named gap, the two documents drift and the
integration is renegotiated in chat instead of in contracts. Issue
#1041 tracks that alignment; this spec is its ripr-side anchor.

## Behavior

```text
User question:
Can another review tool consume ripr evidence without
reinterpreting raw findings?
```

The contract: downstream tools (unsafe-review, ub-review) consume
canonical items and named limitations. They do not parse raw finding
internals, do not re-derive `gap_state`, and do not upgrade advisory
evidence into stronger claims. Raw findings remain available as
supporting evidence attached to canonical items (`raw_findings[]`,
`raw_spans[]`, `raw_evidence_refs[]`); they are context, not a second
source of truth.

### Required export shape

A downstream-consumable evidence unit must carry all ten of these
facets, each from an existing ripr field:

| Facet | ripr source |
| --- | --- |
| canonical_gap_id | `finding_alignment.items[].canonical_gap_id`, `evidence_record.canonical_gap_id`, `GapRecord.canonical_gap_id` |
| language | `GapRecord.language` plus `language_status` |
| surface | `GapRecord.scope` and per-surface `projection_eligibility` |
| hazard / evidence class | `evidence_class` on the canonical item |
| actionability | `gap_state` plus the class-scoped `actionability` label |
| proof mode | `bun_cross_language_grip.proof_mode` for cross-language items; `confidence.basis` (`fixture_backed` \| `static_only` \| `calibrated` \| `unknown`) elsewhere |
| repair or limitation route | `repair_route {repair_kind, target_test_type, suggested_assertion}` or `static_limitations[] {category, repair_route, user_actionability}` |
| source location | `primary_anchor {file, line, kind, source_id, reason}` plus `raw_spans[]` |
| runtime status | Lane 1 `run_status` plus `runtime_status.downstream_consumable`; diff-scoped guidance carries `analysis_scope.run_status` plus `analysis_scope.downstream_consumable` |
| non-claims | `authority_boundary`, `proof_mode` booleans, the card `limits[]` list |

`gap_state` is a closed vocabulary with the six required values from
RIPR-SPEC-0061: `actionable`, `static_limitation`, `advisory`,
`internal_only`, `already_observed`, `unknown`. `advisory` items are
consumable as advisory context only: per RIPR-SPEC-0061 they carry an
advisory reason, why the item is not safe as a bounded repair route,
and what evidence would promote it; downstream tools must never
render them as actionable.

`canonical_item_kind` is a closed vocabulary: `gap`, `observed`,
`no_action`, `limitation`, `evidence`. `confidence.basis` is a closed
vocabulary: `fixture_backed`, `static_only`, `calibrated`, `unknown`;
no surface emits a `runtime` basis today, and runtime-calibrated
bases stay future work per `docs/OUTPUT_SCHEMA.md`.

`run_status` is closed per surface; the per-surface lists compose and
are not one flat set:

- Lane 1 report-level `run_status`: `full`, `limited_timeout`,
  `limited_runner_failure`, `limited_large_cache_skip`,
  `limited_incomplete_input`, `limited_sampled_input`,
  `limited_stale_input`, paired with
  `runtime_status.downstream_consumable`.
- review-comments `analysis_scope.run_status`: `scoped` or
  `limited_diff_scope`; `limited_diff_scope` carries
  `analysis_scope.downstream_consumable = true` for its named scope
  (with the `review_comments_diff_scope_only` limitation route),
  never for repo totals.
- diff report `run_status`: `diff_complete_full_repo_limited`
  (RIPR-SPEC-0072 owns that surface's vocabulary).

Downstream consumers must treat any value outside the relevant closed
vocabulary as `unknown` and fail closed.

### Rail alignment (unsafe-review requirements rail)

unsafe-review's rail (`ripr-bun-diff-first-requirements.md`) is the
consumer-side statement of this contract. Each rail requirement maps
to a ripr field or a named gap so the two documents cannot drift:

| Rail requirement | ripr field / state |
| --- | --- |
| `schema_version` on machine output | `schema_version` on check JSON (`"0.1"`), on evidence records (`"0.1"`), and on every report shape in `docs/OUTPUT_SCHEMA.md` |
| `status: partial` semantics | Lane 1 completeness states: `run_status = "limited_*"` plus `runtime_status.downstream_consumable = false`; diff-scoped output instead carries `analysis_scope.run_status = "limited_diff_scope"` with `analysis_scope.downstream_consumable = true` for its named scope; ripr never renders a limited run as full |
| repo root + diff as first-class input | `ripr check --diff <file>` against a root accepts the input; the scope label is emitted by `ripr review-comments` (`analysis_scope.run_status = "limited_diff_scope"`) and by the `ripr diff` report (`run_status = "diff_complete_full_repo_limited"`); a check-JSON `analysis_scope` block is a named planned delta in the downstream-export-contract slice |
| changed seams ranked before whole-repo inventory | diff-scoped runs analyze changed files first and label scope explicitly instead of waiting on full-repo caches |
| rail report-level `mode: diff_first` / `changed_seams_first` | not yet a ripr field; named gap — the closest current encodings are `analysis_scope.run_status = "limited_diff_scope"` and the diff report's `run_status = "diff_complete_full_repo_limited"` (see Failure Modes) |
| usable, non-empty partial output | limited runs still emit findings, canonical items, and `run_limitations[]` rows; an empty success body with a limited state is a defect |
| skip metadata: reason, limit, observed count | `run_limitations[]` rows carry first-class `observed_seams`, `cache_limit`, `run_status`, and `downstream_consumable` fields plus `runtime_status.limitation_category`; the rail's motivating example (`skipped_large_entry_seams_411564_limit_20000`) maps to `lane1_repo_exposure_cache_store_skipped_large_entry`, which populates both structured fields; the residual gap is the `lane1_repo_exposure_large_cache_preflight_skip` category, which reports cache footprint (bytes, files) in prose only with null `observed_seams` / `cache_limit` (see Failure Modes) |
| skip remediation as a supported command | `runtime_status.repair_route`, item `verify_command`, and `GapRecord.regeneration_commands[]`; remediation text names real commands, not invented flags |
| cache persistence by hash, version, mode | not yet a public output-contract field; named gap — ripr must not claim cache reuse it cannot show (see Failure Modes) |
| rail per-seam `source_route` | not yet a ripr field; named gap — consumers must not synthesize a route label from grip fields (see Failure Modes) |
| rail per-seam `stable_byte_family` | not yet a first-class ripr field; named gap — the nearest anchors are the configured-route metadata on `bun_cross_language_grip` and the configured bridge inventory; consumers must not synthesize the label (see Failure Modes) |
| mixed-language route context preserved | `bun_cross_language_grip`: `state`, `rust_seam.{file, owner, boundary}`, `typescript_evidence.{test_file, verdict, bridge_confidence, missing_discriminators[]}`, `raw_evidence_refs[]` legs (`rust_seam`, `binding_edge`, `external_callsite`, `external_oracle`); the flat names (`rust_file`, `ts_test_file`, and siblings) appear in public JSON only inside the nested `advisory_packet` object |
| rail `proof_mode` field | `proof_mode.mode`: `observable_red_green`, `mutation_plus_miri`, `helper_gated`, `bridge_unknown`, `static_limitation` |
| rail `oracle_language` / `oracle_path` / `oracle_kind` | card `language`, `bun_cross_language_grip.typescript_evidence.test_file`, card `oracle_kind` and `oracle_strength` |
| rail `coverage_confidence` | `bun_cross_language_grip.typescript_evidence.bridge_confidence` plus canonical-item `confidence.basis` |
| rail `limitation` line | `bun_cross_language_grip.limitation_category`, `why_not_actionable`, and `static_limitations[]` |
| receipts are external evidence only | `proof_mode` booleans `runtime_execution`, `mutation_execution`, `miri_execution`, and `proof_claim` are all `false` for preview evidence; receipts record what was scanned and skipped, never witness or Miri status |
| manual-candidate provenance preserved | downstream-owned fields (`source = manual`, `manual_candidate`, `analyzer_discovered`) stay downstream; ripr contributes `bridge_confidence = "configured_hint"` and `raw_evidence_refs[]` so configured routes are never presented as analyzer discovery |
| acceptance checklist | mirrored in Required Evidence below |
| trust boundary | `authority_boundary = "preview_advisory_only"`, card `limits[]`, and the Non-Goals here |

ripr's `downstream_consumable = false` plus the Lane 1 `run_status =
"limited_*"` completeness states are the ripr-side encoding of the
rail's `status: partial` semantics: the artifact exists, names its
limitation, names a repair route, and refuses downstream credit. The
deliberate exception is diff-scoped output: `limited_diff_scope` (and
the diff report's diff phase) is consumable for its named scope,
never for repo totals.

### Bun stable-byte rule

Stable-byte preview evidence remains advisory unless its proof mode is
satisfied by a separately recorded runtime witness outside ripr.
Consumers must apply these fail-closed routes:

- unresolved cross-language oracle: consume as the named limitation
  `cross_language_oracle_visibility_unresolved` with repair route
  `analysis/cross-language-oracle-visibility`; never as an actionable
  gap.
- unknown bridge: consume as `bridge_unknown`; never coerce to
  `no_static_path` and never credit TypeScript discriminators to the
  Rust seam.
- preview card: never support-tier proof. `repair_packet_ready =
  false` and `public_repair_packet = false` are load-bearing; a
  consumer that drops them has left the contract.

### Required and forbidden wording

Downstream surfaces that render ripr evidence must pair these:

- Required: "partial run; not downstream consumable". Forbidden:
  "complete" or "full" for any `limited_*` run.
- Required: "external evidence only" for receipts. Forbidden:
  "witness execution", "Miri-clean", "site-execution", "UB-free",
  or "memory-safe" from ripr evidence alone.
- Required: "advisory preview evidence" for TypeScript/Bun cards.
  Forbidden: "supported", "stable", or any support-tier wording.
- Required: "bridge unknown" when the binding edge is missing.
  Forbidden: "no static path" for the same state.
- Required: the conservative static vocabulary (`exposed`,
  `weakly_exposed`, `reachable_unrevealed`, `no_static_path`,
  `infection_unknown`, `propagation_unknown`, `static_unknown`).
  Forbidden: runtime mutation vocabulary in static output.

## Non-Goals

- No new export format, crate, schema authority, or sidecar service;
  the contract is the existing check JSON, evidence-record, and
  gap-ledger shapes.
- No raw-finding reinterpretation API; downstream tools that need a
  field missing from canonical items request a contract change here
  instead of parsing internals.
- No runtime claims: no witness execution, no mutation execution, no
  Miri execution, no red/green status from ripr evidence.
- No default blocking policy for any downstream tool.
- No support-tier promotion for TypeScript/JavaScript, Bun routes, or
  any preview surface.
- No guarantee that every rail requirement is already satisfied; the
  named gaps in the rail table (preflight-skip structured counts,
  cache-persistence contract, per-seam `source_route`, per-seam
  `stable_byte_family`, report-level diff-first `mode`) stay visible
  until closed by their own slices.

## Required Evidence

- This spec registered in `docs/specs/README.md` and
  `policy/doc-artifacts.toml`.
- A fixture-backed check JSON example showing a canonical item with
  all ten export facets populated or explicitly null.
- A fixture-backed limited-run example (`run_status = "limited_*"`,
  `downstream_consumable = false`) with non-empty body and a named
  repair route.
- A fixture-backed Bun cross-language example for each grip state:
  `rust_ungripped_ts_discriminated`,
  `rust_ungripped_ts_missing_discriminator`,
  `ts_mention_not_observer`, `bridge_unknown`, and the named static
  limitation route.
- Mirror of the rail acceptance checklist: a two-file diff produces
  changed-seam output before whole-repo cache completion; skip output
  carries reason, limit, observed count, scope, and remediation (the
  observed count is `run_limitations[].observed_seams`, structured on
  the cache-store skip category and a named gap on the preflight-skip
  category); partial artifacts are non-empty with status metadata;
  cross-language oracle fields are present for JS/TS tests mapped to
  Rust seams; cache persistence stays explicit and reproducible (a
  named gap until the cache-persistence contract lands; the rail
  allows that exact schema to change); receipts preserve inventory
  limits and claim no witness status.

Fail-closed reject list — a downstream export must refuse to present
any of these states as consumable success:

- `run_status` is any `limited_*` value and the consumer needs full
  counts (`downstream_consumable = false` is binding).
- `gap_state = "unknown"` or any value outside the closed vocabulary.
- `gap_state = "advisory"` rendered as actionable instead of advisory
  context.
- `canonical_gap_id` missing on an item offered as a canonical unit.
- `verify_command` missing on an item offered as actionable.
- cross-language oracle unresolved
  (`cross_language_oracle_visibility_unresolved`).
- `bridge_unknown` in any form, including
  `bridge_confidence = "unknown"` or a `binding_or_ffi_edge` missing
  graph leg.
- a preview card or advisory packet offered as a public repair packet
  (`repair_packet_ready = false`, `public_repair_packet = false`).
- a receipt offered as witness, Miri, mutation, or proof status
  (`proof_claim = false` is binding).
- an empty artifact body presented as a successful scan.

## Acceptance Examples

### Limited diff-first run consumed safely

unsafe-review runs `ripr review-comments --base <sha> --head <sha>`
over a two-file Bun fork change. ripr returns changed-seam guidance
plus `analysis_scope.run_status = "limited_diff_scope"`,
`analysis_scope.downstream_consumable = true`, and the
`review_comments_diff_scope_only` limitation route. The downstream
packet records the scope label, uses the scoped items for the named
scope only, and does not claim whole-repo inventory. (`ripr check
--diff` accepts the same diff input, but its JSON carries no scope
status today; a check-JSON `analysis_scope` block is a named planned
delta in the downstream-export-contract slice.)

### Missing-discriminator Bun route

A configured Blob route reports
`rust_ungripped_ts_missing_discriminator` with
`missing_discriminators = ["resizable_array_buffer"]`, a ranked
placement in `test/js/web/fetch/blob.test.ts`, and `proof_mode.mode =
"observable_red_green"` with `proof_claim = false`. ub-review renders
the route and the suggested discriminator as advisory context and
emits no claim of runtime exposure.

### Bridge unknown stays a limitation

TypeScript discriminators exist but no binding edge is configured.
ripr reports `bridge_unknown` with the unlock condition naming the
missing edge. The downstream tool consumes it as a named limitation
and follows its `repair_route`; it does not report `no_static_path`
and does not credit the TypeScript tests to the Rust seam.

### Large-cache skip with remediation

A whole-repo request trips the large-cache preflight. ripr emits
`run_status = "limited_large_cache_skip"`, `downstream_consumable =
false`, the limitation category, and a repair route naming a narrower
supported invocation. The downstream tool surfaces the remediation
verbatim instead of inventing flags.

## Test Mapping

- None yet.

This spec is docs-only. Implementation slices add traceability entries
when the downstream export contract behavior and its fixtures land;
mapping names should follow the `output/downstream-export-contract`
prefix.

## Implementation Mapping

- docs/specs/RIPR-SPEC-0070-downstream-review-consumer-use-case.md —
  this document.
- plans/use-case-specs/implementation-plan.md (planned) — the
  "downstream export contract" slice: fixture-backed export examples,
  reject-list fixtures, the preflight-skip structured-count gap, and
  the cache-persistence contract gap, sequenced after the spec set
  lands.

## Metrics

- Export-facet coverage: count of canonical items carrying all ten
  required facets versus items with named nulls.
- Reject-list coverage: count of reject-list states pinned by a
  fixture (target: all nine).
- Rail alignment: count of rail requirements mapped to a ripr field
  versus named gaps (currently five named gaps: preflight-skip
  structured counts, cache-persistence contract, per-seam
  `source_route`, per-seam `stable_byte_family`, report-level
  diff-first `mode`).
- Limited-run honesty: count of artifacts with `run_status =
  "limited_*"` that carry a limitation category and repair route
  (target: all of them).
- Promotion rule: move this spec to accepted only when the downstream
  export contract slice has landed with fixture-backed examples for
  every reject-list state, issue #1041 is closed with unsafe-review
  confirming the field mapping against its rail, and the named gaps
  are either closed or re-confirmed as explicit limitations by the
  consumer. Until then this use case stays proposed and no
  downstream integration may be described as contract-complete.

## Failure Modes

- Rail drift: unsafe-review revises its rail and this mapping table
  goes stale — issue #1041 owns re-alignment; a rail change without a
  spec update here is a named defect.
- Preflight-skip structured-count gap: `run_limitations[]` rows carry
  first-class `observed_seams` and `cache_limit` fields, and the
  cache-store skip category
  (`lane1_repo_exposure_cache_store_skipped_large_entry`) populates
  both next to `downstream_consumable` and
  `runtime_status.limitation_category`. The residual gap is narrow:
  the `lane1_repo_exposure_large_cache_preflight_skip` category
  measures cache disk footprint (bytes, files) rather than entry
  counts, reports that footprint in its prose summary only, and
  leaves `observed_seams` / `cache_limit` null. Until that category
  carries structured footprint fields, consumers must read its skip
  size from the summary and must not infer a seam count from it.
- Cache-persistence gap: the rail wants reuse keyed by file hash, tool
  version, and scan mode as visible contract; ripr makes no such
  public claim yet, and downstream docs must not assert it.
- Unmapped rail fields: per-seam `source_route` and
  `stable_byte_family` and the report-level diff-first `mode` labels
  have no ripr field today; consumers must not synthesize them from
  grip fields or scope labels until their slices land.
- Consumer parses `findings[]` directly: out of contract; the fix is a
  contract change request, not a parser.
- A limited run's counts quoted as repo totals: rejected by the
  reject list; `downstream_consumable = false` is binding.
- Preview card flattened into a downstream finding without its
  `limits[]` and authority fields: the non-claims travel with the
  evidence or the evidence does not travel.
