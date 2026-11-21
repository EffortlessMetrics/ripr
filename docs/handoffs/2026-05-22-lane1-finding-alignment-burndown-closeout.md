# Handoff: Lane 1 Finding Alignment Burn-Down Closeout

Date: 2026-05-22

Branch: `campaign-finding-alignment-burndown-closeout`

Current work item: `campaign/finding-alignment-burndown-closeout`

Archived manifest:
`.ripr/goals/archive/2026-05-22-lane1-finding-alignment-burndown.toml`

## Current State

Lane 1 Finding Alignment Burn-Down is closed. The rail kept the evidence model
centered on the same invariant used by downstream PR/CI, editor, report, and
agent surfaces:

```text
Raw findings are analyzer evidence.
Canonical evidence items are the countable unit.
Actionable canonical gaps are the user-facing problem.
```

This closeout records the Lane 1 evidence state after the downstream v2 handoff
refresh landed in swarm #281. It does not change analyzer behavior, PR/CI
rendering, editor behavior, gate policy, badges, generated tests, provider
calls, source edits, baseline or suppression behavior, default blocking, or
mutation execution.

`.ripr/goals/active.toml` now records `status = "closed"` and
`no_current_goal = true`. Future Lane 1 work should be selected from a fresh
audit, scorecard, dogfood receipt, downstream consumer issue, or explicit
spec-backed campaign rather than continued from chat history.

## What Improved

| Area | Closeout state |
| --- | --- |
| Alignment audit | The audit reports aligned classes, unaligned raw findings, duplicate groups, unnamed limitations, missing repair routes, missing verify commands, and raw-to-canonical ratio by evidence class. |
| Named static unknowns | User-facing `static_unknown` canonical items stay named and repair-routed; unknowns remain visible and do not become actionable without fixture-backed evidence. |
| `call_presence` / owner-call activation | The sampled `activation_owner_call_unresolved` bucket gained fixture-backed direct owner-call and same-file one-hop wrapper support while preserving skipped-owner and deeper-wrapper guards. |
| `config_or_policy_constant` | Fixture-backed opaque config report lookup can now become actionable output-observer evidence; unsupported opaque lookup, generated config, macro output, dynamic dispatch, and unsupported cross-file flows remain named limitations. |
| Actionable repair routes | Actionable canonical items require structured top-level `repair_route` data instead of prose-only repair claims. |
| Verify commands | Actionable benchmark records reject missing verify-command sentinels where a concrete command is expected. Missing commands remain explicit and counted. |
| Scorecard and trend lead | Scorecard and trend output lead internally with actionable canonical gaps while preserving raw finding, canonical item, repair-route, verify-command, and capability metric visibility. |
| Runtime confidence | Runtime confidence coverage is reported by canonical evidence class; static-only classes remain calibration work, not user test debt or mutation proof. |
| Dogfood receipts | Finding-alignment dogfood receipts now carry `canonical_gap_id`, `raw_finding_summary`, and `before_after_context` for material #252, #266, and #272 deltas. |
| Downstream contract | The v2 consumer handoff now records the refreshed receipt fields, supported opaque report lookup delta, actionable predicate-boundary lead, and static-only runtime trend boundary. |

## Counts Moved

The rail recorded these movement points from audit and scorecard proof:

| Slice | Before | After | Delta |
| --- | ---: | ---: | ---: |
| Historical total static limitations | 27,677 | 26,339 | -1,338 |
| Historical `activation_value_unresolved` limitations | 27,288 | 25,967 | -1,321 |
| Follow-up total static limitations | 26,277 | 19,106 | -7,171 |
| Follow-up `activation_value_unresolved` limitations | 25,908 | 18,859 | -7,049 |
| Sampled `call_presence` static limitations | 770 | 715 | -55 |
| Sampled `activation_owner_call_unresolved` under `call_presence` | 719 | 663 | -56 |

The live run-reliability proof also recorded a large-repo audit shape after the
streaming fix:

| Metric | Count |
| --- | ---: |
| Raw alignment signals | 47,626 |
| Canonical evidence items | 38,564 |
| Actionable canonical gaps | 162 |
| Named static limitations | 26,250 |
| Top remaining named limitation bucket | `activation_value_unresolved` at 25,881 |

Older 0.6.x release-readable counts remain in the shippable closeout snapshot:
47,181 raw signals, 38,027 canonical items, 149 actionable gaps, and zero
actionable canonical items missing repair routes, verify commands, or named
static limitations.

Final closeout validation also refreshed the bounded sampled audit on
2026-05-22. That sampled report covered 5,000 canonical alignment items, 5,630
raw alignment signals, 61 actionable canonical items, 1,506 alignment static
limitations, and zero unaligned raw findings, unnamed static unknowns, missing
repair routes, or missing verify commands. It explicitly recorded
`repo_exposure_seam_limit = limit_5000_of_40233`, so this is a bounded
selection signal rather than a full-repo exposure completion claim.

## Remaining Limits

- `primary_anchor` and `raw_spans[]` are a projection contract, not a promise
  that every evidence record has universal public placement fields.
- Unsupported opaque lookup, generated config/schema output, macro output,
  dynamic dispatch, and unsupported cross-file config/policy flows remain named
  limitations until selected by a future fixture-first slice.
- Static-only runtime confidence rows remain calibration backlog. They do not
  create user test debt, mutation proof, coverage adequacy, gate authority, or
  public badge semantics.
- Raw findings remain supporting evidence. Downstream surfaces must not
  promote raw `exposed`, `weakly_exposed`, `reachable_unrevealed`, or
  `static_unknown` classes into independent user work.
- Policy overlays such as suppressions, baselines, waivers, blockers, and
  resolved/reintroduced states remain owned outside Lane 1 evidence truth.

## Follow-Up State

- Source #1158, `analysis: make primary_anchor and raw_spans complete for
  canonical items`, is closed and should not be treated as an open blocker for
  this closeout.
- Source #311, value-extraction-v2 planning, remains background only. Reopen
  it as active work only if a fresh audit identifies a concrete value
  extraction class to repair.
- Source #323, cargo-mutants import calibration planning, remains background
  only. Runtime evidence stays imported calibration context unless a future
  campaign explicitly adds an import path; this lane did not add mutation
  execution.
- The next Lane 1 evidence class should be chosen by a fresh
  `lane1-evidence-audit` or `evidence-quality-scorecard` run. Current proof
  points at `activation_value_unresolved` as the largest remaining named
  limitation bucket, with `call_presence` / `activation_owner_call_unresolved`
  as the most recent sampled subqueue.

## Downstream Consumers

These surfaces can consume the refreshed v2 contract:

- PR/CI summaries and report packets should render canonical items first and
  show raw findings as supporting detail.
- Editor and LSP surfaces should project only validated canonical gap records,
  repair routes, verify commands, and receipt state.
- Agent packets should hand off one canonical repair route with raw finding
  context attached, not one task per raw finding.
- Scorecards and trends should lead with actionable canonical gaps while
  preserving raw and canonical diagnostic context.

## Closeout Validation

Closeout validation for this PR:

```bash
cargo xtask goals status
cargo xtask goals next
cargo xtask check-doc-index
cargo xtask markdown-links
cargo xtask check-static-language
cargo xtask check-pr
git diff --check
```

The closed rail also used these proof commands across the implementation
slices:

```bash
cargo xtask lane1-evidence-audit
cargo xtask evidence-quality-scorecard
cargo xtask evidence-quality-trend
cargo xtask dogfood
cargo xtask check-output-contracts
cargo xtask check-fixture-contracts
cargo test -p xtask evidence_quality_benchmark
cargo test -p ripr config_policy --lib
```

## Artifacts

- `.ripr/goals/active.toml`
- `.ripr/goals/archive/2026-05-22-lane1-finding-alignment-burndown.toml`
- `docs/handoffs/2026-05-22-lane1-finding-alignment-burndown-closeout.md`
- `docs/handoffs/2026-05-16-finding-alignment-consumer-contract-v2.md`
- `docs/handoffs/2026-05-17-lane1-shippable-finding-alignment-closeout.md`
- `docs/lanes/LANE_1_FINDING_ALIGNMENT_BURNDOWN.md`
- `plans/lane1-finding-alignment-burndown/implementation-plan.md`

## Next Recommended Goal

No successor campaign is selected. The active manifest intentionally records
`no_current_goal = true`. Select new work from repo-owned state in this order:

1. open pull requests and required checks;
2. `cargo xtask goals next`;
3. `docs/IMPLEMENTATION_CAMPAIGNS.md`;
4. current audits, scorecards, dogfood receipts, and downstream consumer
   issues;
5. accepted proposals, specs, ADRs, and implementation plans.

## What Not To Do

- Do not keep adding Lane 1 changes from this closed rail without a fresh
  selected campaign or concrete regression.
- Do not route raw findings directly into user-facing work.
- Do not treat named static limitations as user test debt.
- Do not use runtime confidence rows as mutation proof.
- Do not change PR/CI rendering, editor behavior, gate policy, badge semantics,
  baseline/suppression behavior, generated tests, provider/model behavior,
  source editing, default blocking, or mutation execution from this closeout.
