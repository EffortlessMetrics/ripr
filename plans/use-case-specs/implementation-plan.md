# Use-Case Spec Layer Implementation Plan

Status: proposed; implementation starts only after the spec set (RIPR-SPEC-0065
through RIPR-SPEC-0073) lands and is registered
Owner: product-swarm
Plan artifact: RIPR-PLAN-0061
Linked proposal: n/a
Linked specs: RIPR-SPEC-0065, RIPR-SPEC-0066, RIPR-SPEC-0067, RIPR-SPEC-0068,
RIPR-SPEC-0069, RIPR-SPEC-0070, RIPR-SPEC-0071, RIPR-SPEC-0072, RIPR-SPEC-0073
Linked ADRs: n/a
Linked issues: #1031 (large-repo diff-first), #1040 (spec lifecycle dashboard),
#1041 (unsafe-review contract alignment)
Active goal: none yet; `.ripr/goals/active.toml` routes through this plan in a
separate PR after the plan lands (per RIPR-SPEC-0065 Implementation Mapping)

## Current State

The use-case spec layer (RIPR-SPEC-0065 roadmap plus the eight use-case specs
0066–0073) writes product contracts over mechanisms that already exist: the
badge model and endpoints pipeline, the optional gate, review guidance and the
inline publish plan, the LSP sidecar and its agent packets, the downstream
check-JSON / evidence-record / gap-ledger shapes, the TypeScript preview
adapter and Bun cross-language grip, the diff report and diff-scoped review
fast path with seam-cache sharding, and the receipt / outcome / swarm-ingest /
ledger loop.

This plan sequences the implementation deltas that make those mechanisms
satisfy their use-case contracts. The order below is maintainer-fixed. Each
work item is one PR-sized production delta plus the complete evidence package
that makes it reviewable (spec citation, fixtures, tests, goldens, docs,
traceability).

The cut line from RIPR-SPEC-0065 holds for every slice:

```text
mechanisms exist;
this lane makes them usable, connected, and non-misleading.
```

## Cross-Cutting Rules

Every work item in this plan inherits these hard boundaries:

- No analyzer behavior beyond the spec contracts: no new reachability,
  infection, or propagation inference; slices change projection, fields,
  fail-closed routing, and selection only, except where a spec names the
  exact analysis delta (work items 5 and 6).
- `gap_state` is the closed six-value vocabulary from RIPR-SPEC-0061 as
  restated by RIPR-SPEC-0070: `actionable`, `static_limitation`, `advisory`,
  `internal_only`, `already_observed`, `unknown`. Every surface uses these
  values and no others; `advisory` items render as advisory context only and
  are never presented as actionable.
- Preview evidence (TypeScript/Bun, Perl, cross-language) stays advisory:
  `language_status = "preview"`, `authority_boundary =
  "preview_advisory_only"`, `repair_packet_ready = false`, and
  `public_repair_packet = false` travel with every projection; no slice
  promotes a support tier, default gate, public badge contribution,
  baseline, or RIPR Zero input.
- Every public packet requires the full field contract: an actionable
  packet missing any of `canonical_gap_id`, `gap_state`, repair shape,
  `verify_command`, `receipt_command`, `allowed_edit_surface`,
  `must_not_change`, confidence, or structured raw evidence refs fails
  closed into a named limitation with `missing_actionability_fields`, never
  a partial packet.
- Fail closed everywhere: limited, sampled, stale, or scoped runs carry
  `run_status`, a limitation category, a `repair_route`, and the correct
  `downstream_consumable` value; no surface renders partial evidence as
  complete, and an empty result is a scope statement, never an all-clear.
- Static output stays inside the conservative vocabulary enforced by
  `cargo xtask check-static-language`; no runtime mutation vocabulary on
  any static surface.
- No mutation execution, no runtime test execution, no generated tests, no
  autonomous source edits, no provider integration, no default blocking CI,
  and no new crates or workspace-shape changes.

## Sequencing

| # | Work item | Spec |
| --- | --- | --- |
| 1 | review/file-line-hardening | RIPR-SPEC-0068 |
| 2 | output/badge-projection | RIPR-SPEC-0066 |
| 3 | gate/pr-gate-advisory-behavior | RIPR-SPEC-0067 |
| 4 | lsp/agent-packet-completeness | RIPR-SPEC-0069 |
| 5 | analysis/typescript-adapter | RIPR-SPEC-0071 |
| 6 | analysis/large-repo-diff-first-mode | RIPR-SPEC-0072 |
| 7 | output/receipt-outcome-quality | RIPR-SPEC-0073 |
| 8 | report/route-quality-metrics | RIPR-SPEC-0073 |

The order is maintainer-fixed. RIPR-SPEC-0070's five named rail gaps are
handled as explicit deferrals plus one queued follow-up slice; see
"Downstream Export Gaps (RIPR-SPEC-0070)" after the work items.

## Work Item 1: review/file-line-hardening

Status: pending
Linked spec: RIPR-SPEC-0068 — "Required card fields" (the hard navigational
rule and the contract-to-implement field list), "Sparse by default" (the
tokenized selection vocabulary and the gap-ledger suppression set), "Scope
honesty", and the verifier reject list
Linked ADR: n/a
Blocks: output/badge-projection
Blocked by: spec set landing

### Goal

Make every review card navigational: a reviewer can jump from every card to a
concrete `file:line`, or is told in a named state why no location resolves.

### Production Delta

Enforce the hard rule in `crates/ripr/src/output/review_comments.rs` and
`crates/ripr/src/output/pr_inline_comment_publish_plan.rs`:

```text
No seam ID without file:line or explicit source_location_unresolved.
```

Close the working-set selection/suppression vocabulary by replacing today's
free-text `summary_reason` strings with the six RIPR-SPEC-0068 machine
tokens, delivering the planned tokens string-for-string:

- `inline_comment_cap_reached` (planned — tokenizes the free-text "inline
  comment cap reached"; the publish-plan skip reason `cap_reached` in
  `pr_inline_comment_publish_plan.rs` and the RIPR-SPEC-0025 metric name
  `pr_inline_comment_cap_reached` name the same condition, and this slice
  collapses all three names into this one token);
- `no_safe_changed_line_placement` (planned — tokenizes today's free text);
- `navigation_only_cross_language_target` (planned — tokenizes today's free
  text);
- `nearby_test_changed` (existing);
- `summary_cap` (existing);
- `missing_verification_command` (existing on the gap-ledger path; the
  working-set path adopts it in this slice).

Enforce the gap-ledger projection path's own closed suppression set
(`gap_record_comment_json`): `not_pr_comment_eligible`,
`not_pr_local_repairable`, `policy_state_not_commentable`, `missing_anchor`,
`missing_dedupe_fingerprint`, `duplicate_dedupe_fingerprint`,
`missing_repair_route`, `missing_verification_command`.

Deliver the RIPR-SPEC-0068 contract-to-implement card fields: a receipt
command on actionable cards (no review card carries one today), `gap_state`
on every card (today only gap-ledger and cross-language limitation cards
carry it; working-set actionable cards carry `grip_class` only), the
structured related-test object `{name, file, line}` (today
`GapRepairRoute.related_test` is a single string), card-level `oracle_kind` /
`oracle_strength` (today carried by agent briefs and seam packets), and an
`analysis_scope` on the gap-ledger guidance artifact
(`render_gap_record_review_comments_json` carries none today). Add the
RIPR-SPEC-0068 reject-list checks to output-contract tests.

### Evidence Package

- Fixtures: an actionable inline card; a summary-only card for each
  working-set closed-vocabulary reason; a suppressed card; a
  `limited_diff_scope` run carrying `run_status`, `basis`, `limitation`, and
  `repair_route`.
- Tests: card-field contract tests in `review_comments.rs` and
  `pr_inline_comment_publish_plan.rs`; token-collapse tests pinning that
  `cap_reached` and `pr_inline_comment_cap_reached` resolve to
  `inline_comment_cap_reached`; reject-list cases (seam without placement or
  unresolved marker; actionable card without verify or receipt command;
  preview card without non-claim fields; `safe_to_publish = false` rendered
  as publishable; empty card set rendered as all clear; a reason outside
  either closed vocabulary).
- Goldens: refreshed review-guidance expectations via the goldens workflow.
- Docs: `docs/OUTPUT_SCHEMA.md` for the added card fields and the
  `analysis_scope` extension; traceability entries for the new tests.

### Non-Goals

No raised inline or summary caps; sparse-by-default is the product position
(`DEFAULT_REVIEW_MAX_INLINE_COMMENTS = 3`,
`DEFAULT_REVIEW_MAX_SUMMARY_ITEMS = 10`). No change to the comment-mode
default (`off`) or the advisory, permission-gated posture of `inline`. No
gate decisions from this surface. No new analyzer behavior.

### Acceptance

- Every rendered card carries placement (`path`, `line`, `mode`) or an
  explicit `source_location` with a summary reason; a card with neither is
  rejected.
- Every actionable card carries verify and receipt commands or is suppressed
  with `missing_verification_command`.
- Every summary-only or suppressed item carries a closed-vocabulary reason
  from its path's set; the three historical names for the inline-cap
  condition collapse into `inline_comment_cap_reached`.
- Every card carries `gap_state` from the six-value vocabulary.
- Diff-scoped guidance carries its `analysis_scope` and never presents
  itself as a full-repo verdict; the gap-ledger guidance artifact gains its
  `analysis_scope`.

### Proof Commands

```bash
cargo test -p ripr review_comments -- --test-threads=1
cargo test -p ripr pr_inline_comment_publish_plan -- --test-threads=1
cargo xtask fixtures
cargo xtask goldens check
cargo xtask check-output-contracts
cargo xtask check-fixture-contracts
cargo xtask check-static-language
cargo xtask check-pr
git diff --check
```

### Rollback

Revert the card-field enforcement, token replacement, reject-list tests,
fixtures, and golden updates. No runtime state needs rollback.

### Claim Boundary

After this slice, review cards may claim to be navigational and
fail-closed. They still claim only conservative static exposure language; an
empty card set remains a scope statement, and no card claims runtime test
strength.

## Work Item 2: output/badge-projection

Status: pending
Linked spec: RIPR-SPEC-0066 — "User-facing badge states", "Required fields",
"State mapping rules (fail closed)", and the verifier reject list
Linked ADR: n/a
Blocks: gate/pr-gate-advisory-behavior
Blocked by: review/file-line-hardening

### Goal

Make the public badge answer "clean, actionable, limited, or stale — and is
the number current and complete?" without ever rendering a degraded input as
a clean count.

### Production Delta

Emit the closed five-message public badge set — `ripr: 0 actionable`,
`ripr: N actionable`, `ripr: limited`, `ripr: stale`, `ripr: unknown` — and
the required sidecar fields (`run_status`, `generated_at`,
`actionable_count`, `limited_reason`, `stale_age`, `source_report`) from the
existing `cargo xtask badges` / `repo-badge-artifacts` pipeline, with the
fail-closed precedence `unknown` over `stale` over `limited` over any count.
Add age-based staleness: evaluate `stale_age` (artifact age relative to its
source) against a configured maximum age, and render `ripr: stale` whenever
the source report or committed endpoint exceeds it. Public badge basis is
restricted to `canonical_actionable_gap` or `gap_decision_ledger`; scope is
restricted to repo. The sidecar field additions are a public contract change
and bump `BADGE_SCHEMA_VERSION` past the current `0.5`.

### Evidence Package

- Fixtures: one per state-mapping row and one per reject-list entry
  (raw-finding basis, diff scope, `limited_*` run, missing or unreadable
  source report, missing `generated_at`, over-age artifact, count alongside
  a degraded state, preview-language counts, sampled run labeled full).
- Tests: badge model and projection tests pinning the precedence order, the
  sidecar field set, and the configured max-age evaluation.
- Goldens / generated artifacts: refreshed badge expectations;
  `cargo xtask badges --check` coverage for the new states.
- Docs: `badges/README.md` vocabulary aligned to the closed message set;
  the `BADGE_SCHEMA_VERSION` bump note; traceability entries.

### Non-Goals

No new badge endpoints, no new render pipeline, no badge-driven blocking, no
preview-language contribution to the public count, no change to the
badge-endpoints workflow's fail-closed skip behavior.

### Acceptance

- The public badge renders exactly one of the five closed messages; every
  reject-list condition resolves to `unknown`, `limited`, or `stale` with a
  named reason in the sidecar — never a silent green.
- A source report or endpoint older than the configured maximum age renders
  `ripr: stale` and does not re-claim the previous count.
- `cargo xtask badges --check` fails on stale committed endpoints and covers
  the `limited` / `stale` / `unknown` renderings.

### Proof Commands

```bash
cargo test -p ripr badge -- --test-threads=1
cargo xtask badges
cargo xtask badges --check
cargo xtask badge-basis
cargo xtask repo-badge-artifacts
cargo xtask check-badge-diff-policy
cargo xtask check-output-contracts
cargo xtask check-static-language
cargo xtask check-pr
git diff --check
```

### Rollback

Revert the projection change, sidecar fields, max-age evaluation, fixtures,
and regenerated badge artifacts; restore the prior `BADGE_SCHEMA_VERSION`.

### Claim Boundary

`ripr: 0 actionable` means exactly: zero unresolved canonical actionable
gaps under a full, current, repo-scoped run. The badge claims no oracle
completeness, no runtime mutation evidence, no coverage-dashboard semantics,
and nothing about preview languages.

## Work Item 3: gate/pr-gate-advisory-behavior

Status: pending
Linked spec: RIPR-SPEC-0067 — "What the gate reasons over", "What the gate
MUST NOT block on", "Required output fields", and the verifier reject list
Linked ADR: n/a
Blocks: lsp/agent-packet-completeness
Blocked by: output/badge-projection

### Goal

Make every gate decision actionable and bounded: a reviewer can read why the
status holds, which canonical deltas drove it, and how to reproduce it
locally — while the gate stays advisory by default.

### Production Delta

Ensure the `ripr gate evaluate` decision report carries every required
output field (decision, reason, changed surfaces, canonical gap deltas,
receipt deltas, runtime status of the inputs, and the local
`ripr gate evaluate ...` reproduction command); express gate reasons as
canonical gap / receipt deltas over changed surfaces; surface input
`run_status` so limited or stale evidence degrades the decision instead of
silently shrinking it.

### Evidence Package

- Fixtures: extend the checked matrix under
  `fixtures/boundary_gap/expected/calibrated-gate/` with the reason classes
  and reject-list entries (block from raw-finding churn, block from a static
  limitation, block from preview evidence, full-run pass from a limited
  input, decision above the readiness ceiling, blocking exit before reports
  are written, missing required output field, dropped acknowledged
  candidate).
- Tests: gate evaluation tests pinning the closed decision-status
  vocabulary, the readiness ceiling, and the required field set.
- Docs: `docs/CALIBRATED_GATE_POLICY.md` / `docs/BLOCKING_READINESS.md`
  cross-references where field names change; traceability entries.
- Dogfood: gate-adoption receipts keep
  `default_generated_ci_blocking = false`.

### Non-Goals

Generated CI keeps `RIPR_GATE_MODE` unset; nothing becomes blocking by
default. No preview-language gate eligibility. No baseline-as-suppression:
refresh stays shrink-only. No comment posting, SARIF upload, or test
generation from this surface.

### Acceptance

- Every blocking decision traces to a new, regressed, or receipt-missing
  canonical delta on a changed surface; zero blocks from reject-list
  signals in the fixture matrix.
- Every blocking report carries the full required field set including the
  local reproduction command, and `gate-decision.{json,md}` exist before
  any non-zero exit.
- A `limited_*` input is visible in runtime status and never produces a
  full-run `pass` or a manufactured block.

### Proof Commands

```bash
cargo test -p ripr gate -- --test-threads=1
cargo xtask fixtures
cargo xtask check-fixture-contracts
cargo xtask check-output-contracts
cargo xtask dogfood
cargo xtask check-static-language
cargo xtask check-pr
git diff --check
```

### Rollback

Revert the decision-report field changes, fixtures, and tests. Operator
rollback for any configured repo stays configuration-only: unset
`RIPR_GATE_MODE` (and `RIPR_GATE_BASELINE`).

### Claim Boundary

A `pass` claims only: no visible policy-eligible candidates under the
configured mode over the named changed surfaces at the reported
`run_status`. It claims nothing about repo cleanliness, oracle
completeness, or runtime test strength.

## Work Item 4: lsp/agent-packet-completeness

Status: pending
Linked spec: RIPR-SPEC-0069 — "What the LSP must expose", "Fail closed", the
closed command vocabulary, and the verifier reject list
Linked ADR: n/a
Blocks: analysis/typescript-adapter
Blocked by: gate/pr-gate-advisory-behavior

### Goal

Make the LSP an agent cockpit: every offered action carries the full bounded
packet, or the surface degrades to a named limitation — never an instruction
without boundaries.

### Production Delta

Enforce, across `crates/ripr/src/lsp/{diagnostics,actions,hover,state}.rs`
and `lsp/gap_artifacts.rs`, that every offered action carries the canonical
RIPR-SPEC-0061 repair packet (the LSP projects that contract; it defines no
packet shape of its own): `packet_id`, `canonical_gap_id`, `repair_kind`,
`target_test_shape`, `related_test_or_observer`, `verify_command`,
`receipt_command`, `confidence`, `must_not_change[]`,
`allowed_edit_surface[]`, and structured `raw_evidence_refs[]` — or degrades
to a named limitation listing `missing_actionability_fields` by name. Stale
or absent snapshots offer `ripr.refresh` only; preview evidence projects as
advisory context with no repair action; gap artifacts that fail validation
are rejected with the named failure and never projected.

First-useful-action stays a read-only projection: the LSP validates
`target/ripr/reports/first-useful-action.json` (`lsp/gap_artifacts.rs`) and
projects it into hover and diagnostics; the status-bar rendering is the VS
Code extension client's surface (`editors/vscode/src/client.ts`), not the
LSP's, and this slice changes neither side's report generation.

### Evidence Package

- Tests: LSP tests (`crates/ripr/src/lsp/tests.rs`, `gap_artifacts.rs`
  validation tests) for the packet-completeness rule, the closed command
  vocabulary, and each reject-list entry (action without edit boundaries,
  actionable packet missing verify or receipt, repair action from preview
  evidence, action against a stale snapshot, invalid gap artifact projected,
  synthesized first-useful-action status, empty diagnostics rendered as
  clean, command outside the vocabulary).
- Fixtures: a complete actionable packet, a named-limitation state, a
  stale-snapshot refresh-only state, a preview-evidence advisory state.
- Docs: traceability entries; editor docs only where command behavior text
  changes (no new commands).

### Non-Goals

No autonomous edits; every command stays read-only or copy-only. No new
commands outside the closed vocabulary. No new report generation from the
first-useful-action projection. No provider integration. No change to the
existing default-on seam diagnostics posture: `enable_seam_diagnostics`
stays default on (`DEFAULT_LSP_SEAM_DIAGNOSTICS = true`) with opt-out via
the `seamDiagnostics` initialization option or repo config, owned by its
existing config contract.

### Acceptance

- 100% of offered actions carry edit surface, must-not-change, verify, and
  receipt, by construction.
- No-action states name a limitation and route instead of rendering nothing.
- Zero repair actions offered against stale snapshots in tests.

### Proof Commands

```bash
cargo test -p ripr lsp -- --test-threads=1
cargo xtask check-output-contracts
cargo xtask check-architecture
cargo xtask check-static-language
cargo xtask check-pr
git diff --check
```

### Rollback

Revert the action-gating change, tests, and fixtures. No editor extension or
protocol changes need rollback.

### Claim Boundary

The LSP claims no analyzer authority of its own: every diagnostic, hover,
action, and packet is a projection of canonical actionability plus runtime
completeness. An empty diagnostic set is a scope statement.

## Work Item 5: analysis/typescript-adapter

Status: pending
Linked spec: RIPR-SPEC-0071 — "Adapter promise", "Supported v1 vocabulary",
"Limitation categories", "Public repair packet contract", and the verifier
reject list
Linked ADR: n/a
Blocks: analysis/large-repo-diff-first-mode
Blocked by: lsp/agent-packet-completeness

### Goal

Make TypeScript and Bun evidence visible under the closed v1 vocabulary
without faking cross-language certainty: detect what is supported, fail
closed into named limitations for everything else.

### Production Delta

Extend the syntax-first TypeScript adapter with the framework-hint extension
(`bun test`, `node:test`, `assert.*`, `t.*` shapes alongside the existing
jest/vitest vocabulary), the six named limitation categories
(`unknown_framework`, `dynamic_helper`, `opaque_matcher`,
`unresolved_bridge`, `missing_verify_command`,
`cross_language_oracle_unresolved`), bounded verify-command inference
(command text only, never executed; ambiguity routes to
`missing_verify_command`), and fail-closed enforcement of the public repair
packet contract: `repair_packet_ready = false` and
`public_repair_packet = false` with missing fields named in
`missing_actionability_fields`, where `raw_evidence_refs[]` must be
structured per RIPR-SPEC-0061 (an anchor field plus an identity field;
placeholder refs do not satisfy the requirement). Field completeness is
necessary but not sufficient: per the post-0.8.1 support decision, public
emission additionally requires a separate accepted promotion contract, so
this slice keeps the public-packet emission count at zero regardless of
field state.

### Evidence Package

- Fixtures: detection of each supported framework hint; each supported
  oracle shape; each limitation category emitting its named category rather
  than an empty or coerced result; bounded verify-command inference per
  framework; a fail-closed fixture per missing packet field; Bun grip
  fixtures for `rust_ungripped_ts_discriminated`,
  `rust_ungripped_ts_missing_discriminator`, `ts_mention_not_observer`,
  `bridge_unknown`, and `rust_ungripped_ts_missing_external_oracle` with
  proof-mode booleans asserted false.
- Tests: adapter fact tests plus the existing Bun calibration tests in
  `xtask/src/main.rs`; card projection stays
  `typescript_preview_card.v1` (no card-shape change).
- Reports: `cargo xtask configured-bridge-inventory` and
  `fixtures/bun-ub-cross-language-dogfood` cited as the calibration base.
- Docs: capability matrix and traceability updates.

### Non-Goals

No `tsc`, `tsserver`, type inference, package graph, bundler, or sourcemap
integration. No execution of any test runner. No new public output surface
beyond the existing card. No default gate, badge, baseline, or RIPR Zero
contribution. No support-tier movement and no public repair packet emission;
both require a separate accepted promotion contract.

### Acceptance

- Each supported framework hint and oracle shape is fixture-pinned; shapes
  outside the closed lists route to named limitation categories, never
  silent coercion.
- The public-packet emission count remains zero — gated on a separate
  accepted promotion contract, not merely on field completeness — and every
  refused packet names its missing fields.
- Grip-state fixtures keep `runtime_execution`, `mutation_execution`,
  `miri_execution`, and `proof_claim` all false.

### Proof Commands

```bash
cargo test -p ripr typescript -- --test-threads=1
cargo test -p xtask cross_language_oracle_graph -- --test-threads=1
cargo test -p xtask typescript_bun_ub_calibration -- --test-threads=1
cargo xtask configured-bridge-inventory
cargo xtask check-fixture-contracts
cargo xtask check-output-contracts
cargo xtask check-capabilities
cargo xtask check-static-language
cargo xtask check-pr
git diff --check
```

### Rollback

Revert the adapter facts, limitation routing, fixtures, and tests. Preview
authority is unchanged in either direction, so no claim rollback is needed.

### Claim Boundary

TypeScript/JavaScript remain opt-in preview. For calibrated Bun stable-byte
cases, ripr provides advisory cross-language evidence distinguishing
TS-discriminated, missing-discriminator, mention-only, bridge-unknown, and
named-limitation states. Proof mode is a planning label, never a statement
that the proof happened.

## Work Item 6: analysis/large-repo-diff-first-mode

Status: pending
Linked spec: RIPR-SPEC-0072 — "Required behavior" items 1–6, the closed
limited-state vocabulary, and the reject list; issue #1031
Linked ADR: n/a
Blocks: output/receipt-outcome-quality
Blocked by: analysis/typescript-adapter

### Goal

Make diff-first the productive default path on large repos: changed seams
render first, full-repo context is an explicit opt-in with its cost named,
and every limited state names its observed count, limit, and repair route.

### Production Delta

Make the diff-scoped path the default productive entry on large repos:
changed seams analyzed and rendered before any full-repo phase; first-run
guidance routes toward the diff surface; full-repo context stays explicit
opt-in with its cost named up front. As part of the same contract, satisfy
RIPR-SPEC-0072 required-behavior items 2–4 end to end: every scoped or
sampled surface emits the observed seam count alongside the total when the
total is known (for example `limit_5000_of_39685`), and every cache-bound
run emits the effective `RIPR_REPO_SEAM_CACHE_LIMIT`. The structured
`observed_seams` / `cache_limit` fields already exist on
`run_limitations[]` rows and are populated by the cache-store skip category
(`lane1_repo_exposure_cache_store_skipped_large_entry`), per RIPR-SPEC-0070;
this slice pins that emission with fixtures and extends the observed-count
emission to any scoped or sampled surface that still lacks it. The residual
RIPR-SPEC-0070 preflight-skip gap (the
`lane1_repo_exposure_large_cache_preflight_skip` category reports cache
footprint in prose only, with null `observed_seams` / `cache_limit`) is not
closed here; it routes to the downstream-export-contract follow-up slice
(see Downstream Export Gaps).

### Evidence Package

- Tests: diff report status preservation
  (`crates/ripr/src/output/diff_report.rs`,
  `diff_complete_full_repo_limited`), review-comments scope tests in
  `crates/ripr/src/cli/commands.rs` (`limited_diff_scope`, basis string,
  limitation route), seam-cache shard and limit tests
  (`crates/ripr/src/analysis/seam_cache.rs`).
- Fixtures: one per closed limited-state vocabulary entry; reject-list
  fixtures (scoped run rendered full, missing observed count, silent cache
  truncation, limited state without `repair_route`, absent
  `downstream_consumable`, out-of-vocabulary limited state).
- Docs: `docs/OUTPUT_SCHEMA.md` update for any observed-count surface
  extension and `run_status` surface changes; traceability entries; issue
  #1031 sequencing notes.
- Reports: `cargo xtask cache report` coverage for shard families and the
  named limited store state.

### Non-Goals

No background daemon, watch mode, or incremental index service. No change
to cache key semantics or shard file format (the cache-persistence contract
stays an explicit RIPR-SPEC-0070 gap; see Downstream Export Gaps). No
report-level diff-first `mode` field in this slice; that is a named
RIPR-SPEC-0070 gap requiring a `docs/OUTPUT_SCHEMA.md` and spec update
before any surface emits it. No default blocking behavior and no badge
semantic change. No speed claim for any specific repository; the claim is
ordering plus named partiality.

### Acceptance

- The diff phase is independently consumable and renders before any
  full-repo phase on large repos.
- Every scoped or sampled surface carries the observed count (and total
  when known); every cache-bound run names the limit in effect.
- Every limited state carries a repair route; zero limited-state strings
  outside the closed vocabulary.

### Proof Commands

```bash
cargo test -p ripr diff_report -- --test-threads=1
cargo test -p ripr seam_cache -- --test-threads=1
cargo test -p ripr review_comments -- --test-threads=1
cargo xtask cache report
cargo xtask fixtures
cargo xtask goldens check
cargo xtask check-output-contracts
cargo xtask check-static-language
cargo xtask check-pr
git diff --check
```

### Rollback

Revert the default-path routing, any observed-count surface extension,
fixtures, tests, and the schema doc update.

### Claim Boundary

Diff-scoped output is not a full repo-exposure scan and must not be consumed
as repo-level debt totals; sampled counts are work-queue evidence. The slice
claims the scoped result arrives first and partiality is named — nothing
about wall-clock speed.

## Work Item 7: output/receipt-outcome-quality

Status: pending
Linked spec: RIPR-SPEC-0073 — "Closed outcome vocabulary", "Outcome
resolution rules", and the reject list
Linked ADR: n/a
Blocks: report/route-quality-metrics
Blocked by: analysis/large-repo-diff-first-mode

### Goal

Turn receipts into a learning loop input: every attempt resolves to exactly
one outcome from the closed vocabulary, and no claim counts as improvement
without receipt plus verify evidence.

### Production Delta

Emit the closed outcome vocabulary end to end — swarm ingest → gap decision
ledger → evidence-quality scorecard — including the four planned additions:
`not_attempted` (gaps with no attempt record), `orphan_receipt` (receipts
with no matching current gap), and the promotion of `receipt_stale` and
`receipt_gap_mismatch` from receipt-lifecycle states to attempt outcomes
(today swarm ingest classifies a stale packet as outcome `unknown`). The
canonical `receipt_stale` / `receipt_gap_mismatch` forms are required —
never the legacy aliases `stale_receipt` / `gap_mismatch` — and attempt
outcomes must never be routed through `normalize_receipt_lifecycle_state`
(`crates/ripr/src/output/receipt_lifecycle.rs`), which serves the
receipt-presence axis only and would silently rewrite `not_attempted` into
`receipt_not_applicable`. The resolution rules are enforced: forbidden edits
force `unknown` with `forbidden_edit_flagged = true`; missing or failing
verify caps the outcome at presence-based states; unrecognized movement
strings never resolve to an improvement; and — the named rule-5 delta —
ingest-side classification validates the receipt's before/after sha256
snapshot provenance before accepting movement, instead of reading
`receipt.provenance.movement` unvalidated.

### Evidence Package

- Tests: swarm ingest classification (including the forbidden-edit and
  `trusted_success = false` assertions and the new snapshot-provenance
  validation), receipt lifecycle normalization (including a guard that
  attempt outcomes never pass through the normalizer), outcome bucketing
  and stage deltas, ledger receipt summary counts
  (`receipt_improved_total`, `receipt_unchanged_after_attempt_total`).
- Fixtures: one per closed-vocabulary outcome including the four planned
  states, plus reject-list cases (unverified claim rendered as anything
  beyond presence, receipt-absent attempt counted as improvement, movement
  without snapshot provenance, any rendering of `trusted_success = true`,
  an outcome string outside the closed vocabulary).
- Docs: schema-version notes for the swarm-ingest / ledger / scorecard
  shapes that gain fields; `docs/OUTPUT_SCHEMA.md` and traceability
  entries.

### Non-Goals

No trust elevation for external agents: `trusted_success` stays hard-coded
false. No autonomous retry loop, no provider integration, no new database
or second tracker; accumulation lives in the ledger and scorecard
artifacts. The receipt-presence axis (`receipt_missing`, `receipt_found`,
`receipt_not_applicable`) stays underneath the vocabulary; its states are
not attempt outcomes.

### Acceptance

- 100% of recorded attempts carry an outcome from the closed vocabulary;
  out-of-vocabulary strings are output-contract failures.
- 100% of improvement-counted outcomes carry receipt plus verify
  provenance, by construction; movement without snapshot provenance never
  counts.
- Orphan receipts surface for cleanup instead of silently skewing counts.

### Proof Commands

```bash
cargo test -p ripr swarm_ingest -- --test-threads=1
cargo test -p ripr receipt_lifecycle -- --test-threads=1
cargo test -p ripr outcome -- --test-threads=1
cargo test -p ripr gap_decision_ledger -- --test-threads=1
cargo xtask check-output-contracts
cargo xtask check-static-language
cargo xtask check-pr
git diff --check
```

### Rollback

Revert the vocabulary additions, resolution-rule enforcement, provenance
validation, fixtures, and schema notes.

### Claim Boundary

`evidence_improved` is a static-evidence movement claim only — not a runtime
mutation result and not a correctness guarantee. `resolved` means the gap
left the after snapshot; it makes no completeness claim about the test
suite.

## Work Item 8: report/route-quality-metrics

Status: pending
Linked spec: RIPR-SPEC-0073 — "Route-quality metrics (planned)"
Linked ADR: n/a
Blocks: n/a
Blocked by: output/receipt-outcome-quality

### Goal

Make routing quality measurable: which repair kinds and limitation routes
actually move evidence, fed only by receipt-backed outcomes.

### Production Delta

Extend the evidence-quality scorecard and trend schemas
(`target/ripr/reports/evidence-quality-scorecard.{json,md}` and
`evidence-quality-trend.{json,md}`) with the named route-quality metrics:
`repair_kind_attempted`, `repair_kind_improved`, `repair_kind_unchanged`,
`repair_kind_regressed`, `repair_kind_success_rate`,
`limitation_repair_route_sharpened`, `limitation_repair_route_promoted`,
`top_failing_routes`, and `top_missing_fields` — computed only from
closed-vocabulary, receipt-backed outcomes landed in work item 7.

### Evidence Package

- Tests: xtask report tests for the new metric fields and for the rule that
  receipt-absent outcomes never enter a success numerator.
- Fixtures: scorecard/trend inputs covering improved, unchanged, regressed,
  and excluded (forbidden-edit, unverified) attempts.
- Docs: report schema notes in `docs/OUTPUT_SCHEMA.md` (or the report
  contract docs the scorecard already uses) and traceability entries.

### Non-Goals

No agent or author scoreboards: rates rank routing choices only. No new
report commands; the existing scorecard and trend commands gain fields. No
analyzer behavior.

### Acceptance

- Every route-quality count derives from closed-vocabulary, receipt-backed
  outcomes; the reject-list fixtures pin the exclusions.
- The trend artifact carries the new metrics across runs.

### Proof Commands

```bash
cargo test -p xtask evidence_quality -- --test-threads=1
cargo xtask evidence-quality-scorecard
cargo xtask evidence-quality-trend
cargo xtask check-output-contracts
cargo xtask check-static-language
cargo xtask check-pr
git diff --check
```

### Rollback

Revert the schema extension, tests, and fixtures; the scorecard and trend
artifacts regenerate without the new fields.

### Claim Boundary

Route-quality rates rank routing choices; they grade neither agents nor
authors, and they carry no runtime claim of any kind.

## Downstream Export Gaps (RIPR-SPEC-0070)

RIPR-SPEC-0070's rail alignment table names five gaps between ripr output
and the unsafe-review requirements rail. Each is either routed to the
queued follow-up slice below or held as an explicit deferral; none is
silently absorbed into the maintainer-fixed sequence:

- Preflight-skip structured counts — residual, narrowed gap. The structured
  `observed_seams` / `cache_limit` fields exist on `run_limitations[]` rows
  and the cache-store skip category
  (`lane1_repo_exposure_cache_store_skipped_large_entry`) populates both;
  the residual is the `lane1_repo_exposure_large_cache_preflight_skip`
  category, which measures cache disk footprint (bytes, files), reports it
  in prose only, and leaves the structured fields null. Closing it (adding
  structured footprint fields to that category) belongs to the
  downstream-export-contract follow-up slice per the RIPR-SPEC-0070
  Implementation Mapping. Until then, consumers read the skip size from
  the prose summary and must not infer a seam count from it.
- Cache persistence keying (file hash, tool version, scan mode) — explicit
  deferral. RIPR-SPEC-0072 forbids cache key semantic changes in this lane,
  and ripr makes no public cache-reuse claim today. The
  downstream-export-contract slice records this gap; opening it requires
  its own scoped slice plus a `docs/OUTPUT_SCHEMA.md` contract update, and
  issue #1041 owns the consumer-side re-confirmation. Downstream docs must
  not assert cache reuse until then.
- Per-seam `source_route` — explicit deferral. No ripr field exists today;
  consumers must not synthesize a route label from grip fields. Deferred
  pending issue #1041 rail alignment.
- Per-seam `stable_byte_family` — explicit deferral. ripr's nearest anchors
  are the configured-route metadata on `bun_cross_language_grip` and the
  configured bridge inventory; no first-class field exists. Deferred
  pending issue #1041 rail alignment; consumers must not synthesize the
  label from grip fields.
- Report-level diff-first `mode` (`mode: diff_first` /
  `changed_seams_first` rail rows) — explicit deferral. The closest current
  encodings are `analysis_scope.run_status = "limited_diff_scope"` and the
  diff report's `run_status = "diff_complete_full_repo_limited"`; a
  check-JSON `analysis_scope` block is a named planned delta of the
  downstream-export-contract slice, and a first-class `mode` field requires
  a `docs/OUTPUT_SCHEMA.md` plus RIPR-SPEC-0072 update before any surface
  emits it.

Downstream-export-contract follow-up slice — queued, not in the
maintainer-fixed sequence above. Selected after work item 8, it delivers
the RIPR-SPEC-0070 Required Evidence package: the fixture-backed ten-facet
canonical-item example, the limited-run example, the per-grip-state Bun
examples, the nine reject-list fixtures, the check-JSON `analysis_scope`
planned delta, the preflight-skip structured-count closure, and the
recorded cache-persistence gap disposition. It gates RIPR-SPEC-0070's
promotion to accepted together with issue #1041 closure and the consumer's
re-confirmation of the remaining named gaps.

## Plan Non-Goals

- No analyzer behavior beyond the spec contracts named per work item.
- No mutation execution, runtime test execution, generated tests,
  autonomous source edits, or provider integration anywhere in this plan.
- No support-tier promotion for TypeScript/JavaScript, Bun routes, Perl, or
  any preview surface; promotion requires a separate accepted promotion
  contract.
- No default blocking CI, no badge semantic switch beyond the fail-closed
  states RIPR-SPEC-0066 specifies, and no gate-mode rollout.
- No new crates, binaries, export formats, daemons, or workspace-shape
  changes; the one-package surface holds.
- No second tracker: sequencing lives here and in
  `.ripr/goals/active.toml` once routed.

## Exit Criteria

- All eight work items are done, each landed as one PR with its spec
  citation, fixtures, tests, goldens, docs, and traceability entries.
- Every verifier reject-list entry in RIPR-SPEC-0066 through
  RIPR-SPEC-0069 and RIPR-SPEC-0071 through RIPR-SPEC-0073 is pinned by a
  fixture or output-contract test, satisfying each spec's promotion rule
  toward `accepted`.
- The five RIPR-SPEC-0070 gap dispositions hold: preflight-skip structured
  counts close via the downstream-export-contract follow-up slice; cache
  persistence keying, per-seam `source_route`, per-seam
  `stable_byte_family`, and the report-level diff-first `mode` remain
  explicit, documented deferrals (or gain their own accepted slices); the
  follow-up slice is queued with issue #1041 as its closure condition.
- `cargo xtask check-pr` passes on every slice; `cargo xtask
  check-static-language`, `check-output-contracts`, `check-traceability`,
  and `check-capabilities` stay green on `main`.
- Preview authority is unchanged end to end: no preview surface emits a
  public repair packet, joins a gate, badge, or baseline, or moves support
  tier as a result of this plan.
- `.ripr/goals/active.toml` routes through this plan while work is active
  and records the closeout when the sequence completes.
