# RIPR-SPEC-0123: Targeted Rust Rerun

Status: proposed

Owner: product / swarm

Created: 2026-07-10

Linked proposal:

- None yet

Linked ADRs:

- None yet

Linked plan:

- [RIPR-PLAN-0062](../../plans/rust-one-shot-evidence-to-repair.md)

Linked issues:

- #1424

Linked PRs:

- None yet

Support-tier impact:

- No tier promotion. A targeted rerun is static analysis with cache-reuse
  telemetry; it is not runtime mutation evidence, coverage adequacy, or a
  claim that a repair is correct.
- The support-tier ledger in
  [docs/status/SUPPORT_TIERS.md](../status/SUPPORT_TIERS.md) remains the
  authority for public support claims.

Policy impact:

- Register this spec in `policy/doc-artifacts.toml` and
  `.ripr/traceability.toml`.
- The eventual implementation may add a CLI and JSON artifact, but must not
  add a dependency, database, network call, automatic test execution, source
  edit, or default CI authority.

## Problem

After one focused test edit, developers currently rerun a broad analysis path
that reloads the workspace and recomputes far more than the selected repair
needs. The repository already has content-keyed file-fact and workspace seam
caches, but no first-class command that selects one canonical gap or changed
test, reports exactly what was reused, and refuses to serve stale evidence.

## Behavior

The public command will be:

```text
ripr rerun --gap <canonical-gap-id> --gap-ledger <path> [--before <path>] [--root <path>] [--out <path>]
ripr rerun --changed-test <test-path-or-node> [--before <path>] [--root <path>] [--out <path>]
```

Exactly one selector is required. `--gap` resolves the existing
domain-supplied canonical identity from an explicit ledger; it selects every
matching ledger record because one canonical behavioral gap can have several
seam records. It must not derive an ID from rendered location or test text.
Matching records are grouped into stable-deduplicated anchored `file`/`owner`
scopes; each scope is recomputed and the resulting seams are deduplicated by
domain seam identity. `--changed-test` accepts a
repository-relative test file or an unambiguous test node within that file.
An absent, out-of-root, or stale selector is a named limitation, not a broad
silent fallback. An anchorless or stale record in an otherwise usable canonical
group is a named per-scope limitation, not a reason to discard the group's
current evidence.

`--before` is an explicit prior rerun, repo-exposure, or compatible receipt
artifact. When it is absent, the result is `current_state_only` and may not
claim improved, closed, unchanged, or regressed movement. The command must not
discover an ambient cache or report artifact as an implicit before state.

The command writes one JSON and Markdown rerun receipt. It reuses only
content-addressed fact layers whose keys still match. It recomputes the changed
test facts and the affected seam/test edges, then emits the current selected
gap classifications and the same canonical-gap and receipt identities used by
the full pipeline. Rendered artifacts are never cache inputs.

An explicit `--check-parity` opt-in runs the full static seam inventory for the
same root and configuration, then compares the selected canonical gap and seam
identities plus their file, owner, and static classification fields. A mismatch
is named as `full_pipeline_parity_mismatch` and fails the targeted result closed
into a
`limited` state. The full inventory is deliberately opt-in because it can be
expensive; omitting the flag does not imply parity was checked. If the full
inventory is seam-capped, the result is instead limited with
`full_pipeline_parity_incomplete`, because an absent seam may be outside the
analyzed prefix rather than a true mismatch.

### Cache correctness and disclosure

Every result records:

- selector kind and resolved target;
- repository revision or explicit unavailable-revision state;
- cache schema and analyzer version;
- file-fact hits, misses, stores, and corrupt-entry fallbacks;
- selected seam count and affected-test count;
- every invalidation reason;
- whether a cold fallback occurred and why; and
- whether the result is complete, limited, or unavailable.

A content change to any selected test, selected production seam, Cargo manifest
or lockfile, workspace membership or package graph, selected feature
configuration, toolchain or analyzer configuration, configured oracle policy,
test-intent file, suppression file, analyzer/cache schema, or selector ledger
invalidates the affected facts. A corrupt or mismatched cache entry is ignored
and named; it must never be rendered as a hit. A targeted result must not reuse
a whole-workspace classification whose test evidence is stale.

### Identity and receipt continuity

For a resolved `--gap` selector, the result carries that exact
`canonical_gap_id`, its domain seam identity when available, current gap state,
and the ordinary verify and receipt commands. For a changed-test selector, each
returned affected item carries domain-supplied identity or an explicit
unresolved-identity limitation. The renderer must not synthesize identity from
file, line, expression, or test navigation.

With a valid explicit `--before` artifact, the receipt says whether the
selected gap improved, closed, remained unchanged, regressed, or is limited.
Without it, the receipt says `current_state_only`. It is a static result only;
it does not assert runtime mutation behavior or correctness.

### Benchmark receipt

The implementation registers a reproducible dogfood benchmark that compares a
cold full repair pipeline with a warm targeted rerun on the same selected
repair. The receipt records exact commands, commit, hardware or runner class,
input revision, sample count, p50, p95, cache state, and invalidation case.

The proposed acceptance target is both:

- warm targeted p50 no greater than 30 seconds; and
- warm targeted p50 at least five times faster than the registered cold full
  repair pipeline.

The benchmark does not justify a universal latency claim.

## Non-Goals

- Automatic test generation, execution, or consumer source edits.
- Mutation execution by default.
- Persistent database or network-backed cache.
- Reusing rendered JSON, Markdown, diagnostics, or packet strings as facts.
- Silent full-pipeline fallback, default CI blocking changes, or support-tier
  promotion.

## Required Evidence

- CLI parsing rejects zero or multiple selectors and invalid changed-test nodes.
- Fixture-backed tests pin `--gap` identity projection and `--changed-test`
  affected-seam selection.
- Cache tests pin hit, selected-test invalidation, selected-production
  invalidation, manifest/lockfile/package-graph/feature/toolchain/config
  invalidation, policy/intent/suppression invalidation, corrupt-entry fallback,
  and cold-fallback disclosure.
- Contract tests pin required JSON fields, named limitations, identity/receipt
  continuity, and stale-evidence refusal.
- An explicit parity test compares a selected changed-test rerun with the full
  static inventory and fails closed on a selected-seam mismatch.
- A benchmark definition and checked receipt record the cold/warm comparison.
- `cargo xtask check-output-contracts`, fixture/golden/traceability gates, and
  `cargo xtask check-pr` pass.

## Test Mapping

- Targeted selection tests prove a canonical gap resolves only through supplied
  domain identity, groups all matching seam records, deduplicates recomputation
  scopes and returned seams, preserves current scopes beside named stale or
  anchorless records, and fails closed for an absent or stale ledger.
- Changed-test tests prove an exact repository-relative file or node selects
  only its affected seam/test edges.
- File-fact cache tests prove unchanged files hit, changed files miss, and a
  corrupt cache entry becomes a named cold fallback.
- Receipt tests prove unchanged, improved, closed, regressed, and limited
  outcomes preserve canonical identities without rendering a runtime claim;
  no-`--before` runs stay current-state-only.
- CLI contract tests prove selector exclusivity, required result fields, and
  out-of-root or ambiguous target diagnostics.

## Metrics

- `targeted_rerun_warm_p50_ms`
- `targeted_rerun_warm_p95_ms`
- `targeted_rerun_cache_hit_rate`
- selected seam and affected-test counts
- cold-fallback and invalidation-reason counts

## Acceptance Examples

1. A warm `--changed-test tests/pricing.rs::boundary_case` run reuses unchanged
   file facts, recomputes the changed test and affected seam/test edges, and
   records the cache hit/miss counts plus the test-content invalidation reason.
2. A `--gap gap:... --gap-ledger current.json` run preserves that exact canonical
   ID and reports its current static movement and receipt route.
3. A gap selector missing from its supplied ledger emits a named
   `canonical_gap_unresolved` limitation and does not scan unrelated seams as a
   silent substitute.
4. A corrupt cache entry is ignored, a cold recomputation is disclosed, and the
   result remains usable only when the recomputation completes.
5. A benchmark receipt reports cold and warm samples, p50/p95, cache state,
   runner class, revision, and exact commands without claiming those timings
   apply to every repository.
6. A changed-test rerun without `--before` returns current classifications and
   cache telemetry but names `current_state_only` rather than inventing movement
   from a cache entry or an ambient report.

## Implementation Mapping

| Component | Location |
|---|---|
| CLI parsing and help | `crates/ripr/src/cli/` |
| Targeted selection and receipt orchestration | `crates/ripr/src/app/` |
| Content-keyed fact cache | `crates/ripr/src/analysis/seam_cache.rs` |
| File-fact reuse | `crates/ripr/src/analysis/facts/build.rs` |
| Selected seam/test recomputation | `crates/ripr/src/analysis/seam_inventory.rs` |
| JSON and Markdown receipt | `crates/ripr/src/output/` |
| Schema and contract registry | `schemas/ripr/`, `docs/OUTPUT_SCHEMA.md` |
| Reproducible benchmark receipt | `xtask/src/` and `target/ripr/reports/` |

## CI Proof

- Focused targeted-rerun unit and CLI tests.
- `cargo xtask fixtures`
- `cargo xtask goldens check`
- `cargo xtask check-output-contracts`
- `cargo xtask check-fixture-contracts`
- `cargo xtask check-traceability`
- `cargo xtask check-static-language`
- `cargo xtask check-pr`
- `cargo-allow doctor`, `check --mode audit`, and `worklist` with the owned
  spec-system profile.
