# RIPR-SPEC-0065: Evidence-to-Repair Use-Case Roadmap

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

- [#1040: adopt machine-checked spec lifecycle dashboard](https://github.com/EffortlessMetrics/ripr-swarm/issues/1040)
- [#1041: align downstream consumer contract with unsafe-review requirements](https://github.com/EffortlessMetrics/ripr-swarm/issues/1041)

Linked PRs:

- None yet

Support-tier impact:

- None. This spec defines user-facing use-case contracts over existing
  mechanisms; it promotes no language, surface, or evidence class to a
  stronger support tier.
- Claim boundaries for this surface are governed by the canonical ledger in [support tiers](../status/SUPPORT_TIERS.md); nothing here promotes a tier.

Policy impact:

- Register this spec in `policy/doc-artifacts.toml`.
- No new crates, binaries, dependencies, parsers, runtime executors, or
  LSP servers.
- No new files, config formats, or artifact types beyond the spec set
  itself (RIPR-SPEC-0065 through RIPR-SPEC-0073) and one implementation
  plan.

## Problem

`ripr` has strong mechanism and a weak product spine. Reports, repair
packets, runtime status, readiness roll-ups, preview cards, limitation
routes, receipts, the TypeScript/Bun preview, and cross-language
evidence routing all exist — but there is no product-facing map that
says who uses each surface, where they see it, what good output looks
like, what must never happen, and which implementation specs make that
true.

Without that map, implementation work drifts toward isolated analyzer
slices while the actual missing piece is user affordance. The same gap
makes overclaiming possible: a surface that has no written contract for
its fail-closed states can silently present partial evidence as
complete.

The product thesis this roadmap fixes in writing:

```text
ripr's product is not raw findings.
ripr's product is evidence-to-repair routing.
```

The core user promise:

```text
ripr tells a maintainer or agent the next safe repair,
how to verify it,
how to receipt it,
and when it cannot safely recommend action.
```

## Behavior

This spec owns the use-case map. Each use case gets its own spec; this
roadmap is the start-here index and the cut line.

### Use-case map

| Use case | User question | Spec |
| --- | --- | --- |
| UC1 repo badge | Is this repo clean, actionable, limited, or stale? | RIPR-SPEC-0066 (planned) |
| UC2 PR gate | Did this PR add evidence debt or regress repair evidence? | RIPR-SPEC-0067 (planned) |
| UC3 PR review cards | Where is the issue and what do I do next? | RIPR-SPEC-0068 (planned) |
| UC4 LSP / agent feedback | What is the first safe bounded action here? | RIPR-SPEC-0069 (planned) |
| UC5 downstream consumers | Can unsafe-review / ub-review consume ripr evidence without reinterpreting raw findings? | RIPR-SPEC-0070 (planned) |
| UC6 TypeScript/Bun preview | Can TS/Bun evidence be visible without fake cross-language certainty? | RIPR-SPEC-0071 (planned) |
| UC7 large repo / diff-first | Can a large repo get useful output without waiting for full-repo analysis? | RIPR-SPEC-0072 (planned) |
| UC8 receipts and outcomes | Did the attempted repair actually improve evidence? | RIPR-SPEC-0073 (planned) |

The "(planned)" markers are point-in-time. When a child spec registers
in `docs/specs/README.md` and `policy/doc-artifacts.toml`, the same PR
updates that row's marker so this index never lags the registry; the
promotion rule treats a stale marker as unmet evidence.

### Shared doctrine all use-case specs inherit

- Every surface is a projection of canonical actionability
  (RIPR-SPEC-0061) plus runtime completeness. Surfaces must not create
  alternate analyzer truth or re-derive state from raw findings.
- Fail closed: a surface that cannot satisfy its full contract shows a
  named limitation with a repair route, never a degraded answer that
  looks complete.
- Advisory versus blocking is explicit per surface. Nothing becomes
  blocking by default in this lane.
- Limited, sampled, and partial runs carry `run_status`, a
  `limitation_category`, and a `repair_route`; no surface may present
  them as full runs. The Lane 1 completeness states (`limited_timeout`,
  `limited_runner_failure`, `limited_large_cache_skip`,
  `limited_incomplete_input`, `limited_sampled_input`,
  `limited_stale_input`) carry `downstream_consumable = false`.
  Deliberate exception: diff-scoped output (`limited_diff_scope` and
  the diff phase of the diff report) is `downstream_consumable = true`
  for its named scope only — never for repo totals — matching the
  existing `docs/OUTPUT_SCHEMA.md` contract and RIPR-SPEC-0072.
- Preview evidence (TypeScript/Bun, cross-language) stays advisory and
  never emits public repair packets unless the complete
  actionability/edit/verify/receipt contract is satisfied.
- Each use-case spec states its non-claims in the same breath as its
  claims, including required wording for clean/no-action states so an
  empty result never reads as "all clear".

### Cut line

```text
mechanisms exist;
this lane makes them usable, connected, and non-misleading.
```

This lane adds no analyzer behavior, no mutation execution, no provider
integration, no generated tests, and no autonomous edits. It writes the
contracts that existing mechanisms must satisfy and sequences the
implementation deltas that close the gaps.

### Relationship to existing adoption work

RIPR-SPEC-0009 (defaults-first adoption) covers install-time defaults
and first-contact ergonomics, including standing per-surface default
contracts (its Surface Defaults table covers badges, LSP, and
SARIF/CI). This roadmap does not replace it; it covers the operating
surfaces a user or agent touches after adoption. Where the two overlap
(first-hour experience), this roadmap imposes a forward obligation:
use-case specs must link to RIPR-SPEC-0009 wherever they change a
RIPR-SPEC-0009 surface default (at minimum RIPR-SPEC-0066,
RIPR-SPEC-0067, and RIPR-SPEC-0069). This obligation is enforced
through the Required Evidence checklist below, so the promotion rule
cannot fire while an overlapping child spec omits the link.

## Non-Goals

- No analyzer behavior changes in this spec set. Docs and contracts
  only; implementation follows via the linked plan.
- No full TypeScript semantic analysis, no full Bun binding graph, no
  generated tests, no autonomous repair, no provider integration.
- No default blocking CI or badge semantic switch.
- No claim that any surface demonstrates full test adequacy in the
  runtime sense; static evidence language stays within the conservative
  vocabulary the static-language gate enforces.
- No parallel product board: active goals route through the linked
  implementation plan, not a second tracker.

## Required Evidence

- This roadmap registered in `docs/specs/README.md` and
  `policy/doc-artifacts.toml`.
- Each use-case spec (0066–0073) registered the same way before its
  implementation work starts.
- Each use-case spec carries, at minimum: the user question, the
  surface/artifact, required fields, source of truth, fail-closed
  behavior, advisory-versus-blocking posture, non-claims, and proof
  commands that exist in the current xtask surface.
- A verifier-style reject list per use-case spec: the enumerated states
  the surface must refuse to render as success.
- Each use-case spec that changes a RIPR-SPEC-0009 surface default (at
  minimum RIPR-SPEC-0066, RIPR-SPEC-0067, and RIPR-SPEC-0069) links to
  RIPR-SPEC-0009 and names the default it changes.

## Acceptance Examples

- A contributor asks "what does the badge mean when it says limited?"
  and RIPR-SPEC-0066 answers it without reading analyzer source.
- An agent integrating via LSP finds in RIPR-SPEC-0069 the exact fields
  a first-useful-action packet carries and the exact states in which no
  action is offered.
- unsafe-review engineers consume RIPR-SPEC-0070 plus the existing
  consumable shapes (the versioned check JSON, evidence records, and
  `docs/OUTPUT_SCHEMA.md`) and never need to parse raw finding
  internals; there is no separate export-schema artifact.
- A Codex planning pass picks its next slice from the use-case
  implementation plan instead of inventing an analyzer-first PR list.

## Test Mapping

- None yet. This roadmap is docs-only; the use-case specs add planned
  test mappings as their implementation deltas land. Traceability
  entries are added only when behavior and tests exist.

## Implementation Mapping

- docs/specs/RIPR-SPEC-0065-evidence-to-repair-use-case-roadmap.md —
  this document.
- Planned spec PRs, in order: 0066/0067 (badge + PR gate), 0068/0069
  (review cards + LSP), 0070/0071 (downstream + TypeScript/Bun),
  0072/0073 (large repo + receipts/outcomes).
- plans/use-case-specs/implementation-plan.md (planned) — sequences the
  implementation deltas after the spec set lands: review file:line,
  badge projection, PR gate advisory behavior, LSP agent packet,
  TypeScript adapter, large-repo diff-first mode, receipt outcome
  quality, route metrics.
- `.ripr/goals/active.toml` routes through the plan after it lands
  (separate PR; see the plan).

## Metrics

- Spec coverage: 9 of 9 specs in the set registered (this roadmap plus
  the eight use-case specs 0066–0073). Throughout this document
  "use-case spec" means one of the eight children; the roadmap itself
  is not a use-case spec.
- Plan linkage: every implementation slice in the use-case plan names
  the spec section it satisfies.
- Promotion rule: move this roadmap to `accepted` when all eight
  use-case specs are registered with the Required Evidence checklist
  satisfied (including the RIPR-SPEC-0009 linkage obligation), the
  implementation plan exists, and active goals route through it.
- Drift guard: a use-case surface shipping behavior not described by
  its spec is a defect in this roadmap's process, tracked via the spec
  lifecycle dashboard proposal (issue #1040).

## Failure Modes

- A use-case spec lands without registry entries — caught by
  `check-spec-numbering` / `check-doc-artifacts`.
- A surface renders partial evidence as complete — the owning use-case
  spec's reject list plus output-contract checks make this a named
  defect rather than a style choice.
- Spec set stalls half-landed — the promotion rule above keeps this
  roadmap `proposed` until the full spine exists, which keeps the gap
  visible in the specs index.
