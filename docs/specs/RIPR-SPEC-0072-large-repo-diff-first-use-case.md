# RIPR-SPEC-0072: Large-Repo Diff-First Use Case

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

- [#1031](https://github.com/EffortlessMetrics/ripr-swarm/issues/1031)
  — make diff-first the productive default path on large repos

Linked PRs:

- [#928](https://github.com/EffortlessMetrics/ripr-swarm/pull/928)
  `cache: surface large seam-cache skips` (merged)
- [#933](https://github.com/EffortlessMetrics/ripr-swarm/pull/933)
  `cache: shard large repo seam cache entries` (merged)
- [#934](https://github.com/EffortlessMetrics/ripr-swarm/pull/934)
  `report: summarize sharded cache sets` (merged)
- [#935](https://github.com/EffortlessMetrics/ripr-swarm/pull/935)
  `analysis: add diff-scoped review fast path` (merged)

Support-tier impact:

- None. This spec writes the use-case contract over existing
  large-repo mechanisms; it promotes no language, surface, or
  evidence class to a stronger support tier.
- Claim boundaries for this surface are governed by the canonical ledger in [support tiers](../status/SUPPORT_TIERS.md); nothing here promotes a tier.

Policy impact:

- Register this spec in `policy/doc-artifacts.toml`.
- No new crates, binaries, dependencies, parsers, runtime executors,
  or LSP servers.

## Problem

Large repositories make full-repo analysis the wrong first answer.
Measured reality on this repository (2026-06-06): a full-repo
evidence audit observes 53,531 seams and needs roughly 41 minutes of
compute; the default sampled path caps audit input at 5,000 seams.
Three full-repo audit runs on that date all ended in explicit limited
states (`limited_sampled_input` or `limited_timeout`), each carrying
a limitation category and a repair route. The fail-closed contract
held: no run presented partial evidence as full.

The remaining gap is productivity, not honesty. A user on a large
repo should get the diff-scoped answer first — changed seams, their
exposure classes, and their missing discriminators — and only then
decide whether full-repo context is worth its cost. Today the
mechanisms exist (the diff report, the diff-scoped review fast path
from #935, sharded seam caching from #933, cache visibility from #934
and #928, summarized in the Lane 1 large-repo runtime completeness
closeout, `docs/handoffs/2026-06-03-lane1-large-repo-runtime-`
`completeness-closeout.md`), but there is no single contract that
makes diff-first the productive default path. Issue #1031 tracks that
gap.

## Behavior

The user question this use case answers:

```text
Can I get useful ripr output on a large repo without waiting
for full-repo analysis?
```

### Surfaces in scope

- `ripr diff` report (`crates/ripr/src/output/diff_report.rs`):
  top-level `run_status = "diff_complete_full_repo_limited"`, a
  `runtime_status.diff` phase (`state = "diff_complete"`,
  `phase = "changed_surface_diff"`, changed file and changed seam
  counts, `downstream_consumable = true`) and a
  `runtime_status.full_repo_context` phase
  (`state = "full_repo_limited"`,
  `limitation_category = "full_repo_context_not_run"`,
  `downstream_consumable = false`, and `repair_route =
  "ripr check --format repo-exposure-summary-json"`).
- `ripr review-comments` diff-first mode (#935,
  `crates/ripr/src/output/review_comments.rs`):
  `analysis_scope.scope = "diff_scoped_changed_files"`,
  `run_status = "limited_diff_scope"`,
  `basis = "changed_production_files_plus_immediate_callers"`,
  `limitation = "review_comments_diff_scope_only"`, `repair_route =
  "analysis/diff-scoped-large-repo-review-fast-path"`,
  `downstream_consumable = true`, plus the scoped counts
  (`production_files_considered`, `classified_seams_considered`,
  `total_rust_files`, `total_production_files`).
- Seam-cache sharding and visibility: classified seam-cache entries
  above `RIPR_REPO_SEAM_CACHE_LIMIT` (default 20,000) are written as
  bounded shard files (#933); `cargo xtask cache report` summarizes
  shard families, shard counts, bytes, largest shard sets, and orphan
  or incomplete shard sets (#934); a large-entry store that cannot
  shard surfaces a named limited state with `observed_seams` and
  `cache_limit` fields and a configuration route (#928).

The `downstream_consumable = true` carried by the diff phase and by
`limited_diff_scope` review-comments output is a deliberate, named
exception to the Lane 1 doctrine that limited runs carry
`downstream_consumable = false`: diff-scoped output is consumable for
its named scope only, never as repo-level totals (see Non-claims).
The Lane 1 completeness states (`limited_timeout`,
`limited_runner_failure`, `limited_large_cache_skip`,
`limited_incomplete_input`, `limited_sampled_input`,
`limited_stale_input`) keep `downstream_consumable = false`, matching
the `docs/OUTPUT_SCHEMA.md` contract.

### Required behavior

1. Changed seams are analyzed and rendered before any full-repo
   phase runs. The diff phase is independently consumable.
2. A large cache skip is explicit: the limited state names the
   observed seam count and the configured limit, never a bare skip.
3. The observed seam count is emitted on every scoped or sampled
   surface, alongside the total when the total is known (for
   example the historical `repo-exposure-json:limit_5000_of_39685`
   input string recorded in `docs/OUTPUT_SCHEMA.md`; seam totals are
   per-snapshot and drift as the repo grows, so this example does
   not contradict the 53,531-seam measurement in Problem).
4. The configured cache limit (`RIPR_REPO_SEAM_CACHE_LIMIT` value in
   effect) is emitted whenever it bounds the run.
5. Every limited state carries a `repair_route` that names the
   command or configuration that lifts the limitation.
6. Partial output never looks full: a diff-scoped or sampled run
   must not render `run_status = "full"` or omit its limitation.

### Closed vocabulary: limited states this surface may emit

| State | Where it appears |
| --- | --- |
| `diff_complete_full_repo_limited` | diff report `run_status` and `runtime_status.state` |
| `diff_complete/full_repo_limited` | diff report `receipt.outcome_hint` (compound form of the same state) |
| `diff_complete` | diff report `runtime_status.diff.state` |
| `full_repo_limited` | diff report `full_repo_context.state` |
| `full_repo_context_not_run` | diff report `limitation_category` |
| `limited_diff_scope` | review-comments `analysis_scope.run_status` |
| `review_comments_diff_scope_only` | review-comments `limitation` |
| `limited_large_cache_skip` | Lane 1 audit `run_status` |
| `limited_sampled_input` | Lane 1 audit `run_status` |
| `limited_timeout` | Lane 1 audit `run_status` |
| `limited_runner_failure` | Lane 1 audit `run_status` |
| `limited_incomplete_input` | Lane 1 audit `run_status` |
| `limited_stale_input` | Lane 1 audit `run_status` |

This vocabulary is closed. A new limited state requires an update to
`docs/OUTPUT_SCHEMA.md` and this spec before any surface emits it.

This list covers the diff report, review-comments, and Lane 1 audit
surfaces named above; it composes with the downstream export
vocabulary in RIPR-SPEC-0070 rather than replacing it. Non-limited
run-status values (for example the review-comments `scoped` value,
currently constructed only in test helpers) are intentionally
outside this table because they name scope, not partiality.

### Required and forbidden wording

- Required: "diff complete; full-repo context limited" —
  Forbidden: "analysis complete" for a diff-scoped run.
- Required: "observed N of M seams" (or "observed N seams; total
  unknown") — Forbidden: a bare count that reads as repo truth.
- Required: "cache store limited at `RIPR_REPO_SEAM_CACHE_LIMIT` = L;
  observed S seams" — Forbidden: any silent cache truncation.
- Required: a named repair route on every limited state —
  Forbidden: a limited state with no next step.

### Non-claims

- Diff-scoped output is not a full repo-exposure scan and must not be
  consumed as repo-level debt totals.
- Sampled counts are work-queue evidence, not full-repo debt totals.
- This contract does not claim that any specific large repository run
  is fast; it claims the scoped result arrives first and partiality
  is named.

## Non-Goals

- No analyzer behavior changes in this spec; docs and contract only.
- No background daemon, watch mode, or incremental index service.
- No change to cache key semantics or shard file format.
- No default blocking CI behavior and no badge semantic switch.
- No claim that diff-first output demonstrates full test adequacy in
  the runtime sense; static language stays within the conservative
  vocabulary the static-language gate enforces.

## Required Evidence

Existing evidence this contract builds on:

- Diff report status tests in
  `crates/ripr/src/output/diff_report.rs` (the
  `diff_complete_full_repo_limited` preservation test).
- Review-comments scope tests in `crates/ripr/src/cli/commands.rs`
  asserting `limited_diff_scope`, the basis string, and the
  limitation route.
- Seam-cache shard and limit tests in
  `crates/ripr/src/analysis/seam_cache.rs`.
- The `run_status` / `runtime_status` contract and the
  `lane1_repo_exposure_cache_store_skipped_large_entry`
  compatibility limitation in `docs/OUTPUT_SCHEMA.md`.

Fail-closed reject list — states the surface must refuse to render
as success:

- Diff-scoped or sampled output presented as full-repo truth
  (`run_status = "full"` while the full-repo phase did not run).
- Scoped or sampled output that omits the observed seam count, or
  omits the total when the total is known.
- Silent cache truncation: a classified seam-cache store above the
  configured `RIPR_REPO_SEAM_CACHE_LIMIT` that neither writes shard
  files nor emits the named limited state with `observed_seams` and
  `cache_limit` (the classified-limit behavior cross-referenced as
  `lane1_repo_exposure_cache_store_skipped_large_entry`).
- A limited state emitted without a `repair_route`.
- A limited full-repo phase whose `downstream_consumable` field is
  absent or implied true.
- A new limited-state string outside the closed vocabulary above.

## Acceptance Examples

### Diff report on a large repo

A user runs the diff surface against a large repository. The JSON
carries `run_status = "diff_complete_full_repo_limited"`; the diff
phase reports its changed file and changed seam counts with
`downstream_consumable = true`; the full-repo context phase reports
`full_repo_limited` / `full_repo_context_not_run` with
`downstream_consumable = false` and the repo-exposure repair route.
The human rendering opens with
`RIPR diff status: diff_complete_full_repo_limited`.

### Review comments without full-repo wait

Default `ripr review-comments` on a large repo classifies changed
production files plus bounded immediate callers and reports
`analysis_scope.run_status = "limited_diff_scope"` with the basis,
the scoped counts against the repo totals, the named limitation, and
the fast-path repair route. It never claims full-repo truth.

### Large cache store

A run whose classified seam set exceeds the configured limit writes
bounded shard files; `cargo xtask cache report` shows the shard
family, shard count, and bytes. If a store cannot shard, the run
emits the named limited state with `observed_seams`, `cache_limit`,
and a route through `cargo xtask cache report` plus
`RIPR_REPO_SEAM_CACHE_LIMIT` configuration.

### Sampled full-repo audit

A default Lane 1 audit on this repository records
`limited_sampled_input` with input such as
`repo-exposure-json:limit_5000_of_39685` (a historical snapshot
recorded in `docs/OUTPUT_SCHEMA.md`; the observed total tracks the
live repo and was 53,531 on 2026-06-06), contributes the limitation
to the headline summary, and names the unsampled route. The sampled
counts read as work-queue evidence, not repo debt totals.

## Test Mapping

- None yet. This spec is a use-case contract over mechanisms that
  already carry focused tests (diff report status, review-comments
  scope, seam-cache shard and limit behavior). Traceability entries
  are added when the diff-first default-path implementation slices
  land under issue #1031.

## Implementation Mapping

- docs/specs/RIPR-SPEC-0072-large-repo-diff-first-use-case.md —
  this document.
- plans/use-case-specs/implementation-plan.md (planned) — the
  "large-repo diff-first mode" slice: make the diff-scoped path the
  productive default entry on large repos, wire first-run guidance
  toward it, and keep full-repo context an explicit opt-in with its
  cost named up front.
- Issue #1031 sequences the implementation deltas; each slice names
  the section of this spec it satisfies.

## Metrics

- Time-to-first-changed-seam-result on a large repository, compared
  against the measured ~41-minute full-repo compute baseline.
- Fraction of large-repo runs that deliver consumable diff-scoped
  output before any full-repo phase starts (target: all).
- Fraction of limited states carrying a repair route (target: 100%;
  the 2026-06-06 baseline already met this on three of three runs).
- Count of emitted limited-state strings outside the closed
  vocabulary (target: zero).
- Promotion rule: move this spec to `accepted` when the diff-first
  default-path slice from the linked plan lands, issue #1031 closes
  with measured evidence, and fixtures cover every limited state in
  the closed vocabulary.

## Failure Modes

- A surface renders a scoped run as full — caught by the reject
  list plus `cargo xtask check-output-contracts`.
- A cache store exceeds the limit and neither shards nor names the
  limited state — a contract defect; the seam-cache tests and the
  cache report must surface it.
- A new limited-state string ships without a schema and spec update
  — a closed-vocabulary violation tracked as a defect against this
  spec.
- The full-repo phase silently runs by default on a large repo and
  absorbs the productivity win — the diff-first ordering requirement
  makes this a named regression, not a tuning choice.
