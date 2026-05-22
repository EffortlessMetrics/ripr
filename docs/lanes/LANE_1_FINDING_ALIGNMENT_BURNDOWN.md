# Lane 1: Finding Alignment Burn-Down

Status: open planning rail

Opened: 2026-05-17

## Goal

Make RIPR's evidence counts and downstream handoff operate on this invariant:

```text
Raw findings are analyzer evidence.
Canonical evidence items are the countable unit.
Actionable canonical gaps are the user-facing problem.
```

Presentation-text alignment is complete in documented scope. This rail
generalizes the same discipline across remaining evidence classes so raw
signals do not leak upward as independent user work.

## Boundary

Lane 1 owns:

- evidence class identity;
- raw-finding to canonical-item alignment;
- canonical gap identity and primary anchors;
- gap state and actionability;
- named static limitations and repair routes;
- verification command coverage where feasible;
- scorecard, trend, audit, benchmark, capability, and traceability proof.

Downstream PR/CI, editor, policy, and adoption lanes consume this evidence.
They should render canonical items and policy overlays; they should not infer
actionability from raw analyzer classes.

This rail does not update `.ripr/goals/active.toml` unless the repo-wide
operator sequence explicitly makes it active.

## Current Baseline

After #1106 and #1109 on merged main, the repo-local Lane 1 audit reports:

| Metric | Current value |
| --- | ---: |
| Raw alignment signals | 47,181 |
| Canonical alignment items | 38,027 |
| Actionable canonical items | 149 |
| Static unknowns without named limitation | 0 |
| Canonical items without repair route | 0 |
| Canonical items without verify command | 0 |

The zero-count invariants are useful but need to stay true as new evidence
classes land. The large raw-to-canonical shape and static limitation buckets
still need class-by-class burn-down instead of anecdotal screenshot-driven
repairs.

## Source-Of-Truth Stack

- source model: [Lane trackers README](README.md) defines Lane 1 ownership;
- prior Lane 1 quality loop:
  [Lane 1 Evidence Quality Leadership](LANE_1_EVIDENCE_QUALITY_LEADERSHIP.md);
- first aligned evidence class:
  [Lane 1 User-Visible Output Evidence](LANE_1_USER_VISIBLE_OUTPUT_EVIDENCE.md);
- alignment contract:
  [RIPR-SPEC-0045: Finding-To-Gap Alignment](../specs/RIPR-SPEC-0045-finding-to-gap-alignment.md);
- presentation text contract:
  [RIPR-SPEC-0043: Presentation Text Evidence](../specs/RIPR-SPEC-0043-presentation-text-evidence.md);
- config and policy constant contract:
  [RIPR-SPEC-0048: Config And Policy Constant Evidence](../specs/RIPR-SPEC-0048-config-policy-constant-evidence.md);
- downstream v2 consumer contract:
  [Finding Alignment Consumer Contract v2](../handoffs/2026-05-16-finding-alignment-consumer-contract-v2.md);
- shippable finding-alignment closeout:
  [Lane 1 Shippable Finding Alignment Closeout](../handoffs/2026-05-17-lane1-shippable-finding-alignment-closeout.md);
- implementation rail:
  [Lane 1 Finding Alignment Burn-Down plan](../../plans/lane1-finding-alignment-burndown/implementation-plan.md).

## Burn-Down Queue

| Order | Issue | Slice | Intent | Status |
| ---: | --- | --- | --- | --- |
| 1 | [swarm #229](https://github.com/EffortlessMetrics/ripr-swarm/issues/229) / [source #1140](https://github.com/EffortlessMetrics/ripr/issues/1140) | `report/finding-alignment-coverage-audit` | Show aligned, unaligned, duplicate, unnamed-limitation, missing-repair, and missing-verify queues by evidence class. | done |
| 2 | [swarm #233](https://github.com/EffortlessMetrics/ripr-swarm/issues/233) / [source #1141](https://github.com/EffortlessMetrics/ripr/issues/1141) | `analysis/named-static-unknown-invariant` | Keep user-facing static unknown canonical items named and repair-routed. | done |
| 3 | [#1158](https://github.com/EffortlessMetrics/ripr/issues/1158) | `analysis/canonical-primary-anchor-raw-spans` | Make `primary_anchor` and raw span support complete enough that downstream surfaces have one placement hint plus supporting evidence. | merged in #1187 |
| 4 | [swarm #238](https://github.com/EffortlessMetrics/ripr-swarm/issues/238) / [source #1159](https://github.com/EffortlessMetrics/ripr/issues/1159) | `analysis/top-static-limitation-bucket-burndown` | Pick the top named static limitation bucket from #1140 and repair it with fixture-backed before/after evidence. | done via swarm #240; live audit selected `call_presence` / `activation_owner_call_unresolved`, and the fixture-backed direct owner-call plus mock-expectation slice recorded the before/after scorecard delta while preserving the target-affinity guard |
| 5 | [swarm #241](https://github.com/EffortlessMetrics/ripr-swarm/issues/241) / [source #1142](https://github.com/EffortlessMetrics/ripr/issues/1142) | `docs/spec-config-policy-unsupported-flow-expansion` | Refine the criteria for expanding config/policy unsupported-flow support beyond current heuristic supported sinks. | done via swarm #244; `opaque_config_lookup` selected as the next implementation target, with generated, macro, dynamic-dispatch, and unsupported cross-file flows kept as named limitations until separately selected |
| 6 | [swarm #246](https://github.com/EffortlessMetrics/ripr-swarm/issues/246) / [source #1143](https://github.com/EffortlessMetrics/ripr/issues/1143) | `fixtures/config-policy-unsupported-flow-burndown` | Add fixture-backed cases for selected config/policy unsupported flows before analyzer expansion. | done via swarm #249; benchmark limitations now cover `macro_generated_config_output` and `dynamic_config_dispatch` alongside existing cross-file and opaque lookup guards |
| 7 | [swarm #250](https://github.com/EffortlessMetrics/ripr-swarm/issues/250) / [source #1144](https://github.com/EffortlessMetrics/ripr/issues/1144) | `analysis/config-policy-unsupported-flow-support` | Move one selected config/policy unsupported-flow category out of limitation only when fixture-backed. | ready |
| 8 | [#1145](https://github.com/EffortlessMetrics/ripr/issues/1145) | `analysis/actionable-repair-route-completeness` | Preserve the invariant that actionable canonical items always name a concrete repair route. | open |
| 9 | [#1146](https://github.com/EffortlessMetrics/ripr/issues/1146) | `analysis/actionable-verify-command-coverage` | Preserve and expand verification command coverage where feasible. | open |
| 10 | [#1147](https://github.com/EffortlessMetrics/ripr/issues/1147) | `report/actionable-canonical-gaps-scorecard-lead` | Preserve scorecard-leading actionable canonical gaps as new classes land. | actionable top lists merged in #1266; timeout diagnostics merged in #1271; current follow-up emits bounded actionable-gap packet artifacts for agent-safe triage |
| 11 | [#1160](https://github.com/EffortlessMetrics/ripr/issues/1160) | `calibration/runtime-confidence-coverage-audit` | Report calibrated-supported versus static-only canonical items by evidence class. | current follow-up reports runtime confidence coverage by canonical evidence class in the Lane 1 audit and scorecard |
| 12 | [#1149](https://github.com/EffortlessMetrics/ripr/issues/1149) | `dogfood/finding-alignment-examples-refresh` | Refresh real examples after new burn-down deltas instead of duplicating existing dogfood fixtures. | open |
| 13 | [#1153](https://github.com/EffortlessMetrics/ripr/issues/1153) | `docs/canonical-alignment-contract-refresh` | Refresh the downstream handoff only after material burn-down changes. | open |

Related legacy issues stay visible but should not be treated as the only Lane 1
burn-down rail:

- [#311](https://github.com/EffortlessMetrics/ripr/issues/311)
  records older value-extraction-v2 planning. Use it as background when
  #1140 shows value-resolution gaps that need fixture-backed repair.
- [#323](https://github.com/EffortlessMetrics/ripr/issues/323)
  records older cargo-mutants import calibration planning. Runtime evidence
  remains imported calibration context only; this rail does not add mutation
  execution.

Existing config/policy constant evidence is not reopened from scratch:

- [RIPR-SPEC-0048](../specs/RIPR-SPEC-0048-config-policy-constant-evidence.md)
  defines the class.
- `fixtures/evidence-quality-benchmark/corpus.json` and
  `fixtures/finding-alignment-dogfood/corpus.json` already contain
  config/policy cases.
- `finding_alignment` output already reports `config_policy_*` counts.
- #1142 through #1144 are scoped to unsupported-flow expansion beyond current
  supported sinks.

## Operating Rules

- Start from audit data, not screenshots alone.
- Fixture first for every new class or confidence claim.
- Raw findings remain supporting evidence.
- Canonical items are the countable evidence unit.
- Actionable canonical gaps require concrete repair routes.
- Unknowns and static limitations are not user test debt.
- Runtime-only signal does not create a static gap.
- Policy overlays stay separate from Lane 1 evidence state.
- Scorecard-leading changes land internally before public badge semantics move.

## Non-Goals

- No PR or CI rendering changes in Lane 1.
- No inline PR comment publishing.
- No LSP or editor polish.
- No gate-policy changes or default blocking.
- No public badge or score redefinition.
- No generated tests.
- No automatic source edits.
- No provider or model calls.
- No mutation execution.
- No release, packaging, platform, dependency, or MSRV cleanup.

## Validation Gates

Docs and planning slices should run:

```bash
cargo xtask check-doc-index
cargo xtask markdown-links
cargo xtask check-static-language
cargo xtask check-pr
git diff --check
```

Spec slices should also run:

```bash
cargo xtask check-spec-format
```

Fixture slices should also run:

```bash
cargo test -p xtask evidence_quality_benchmark
cargo xtask check-fixture-contracts
cargo xtask check-output-contracts
```

Analyzer, audit, and scorecard slices should run the focused tests named by the
slice plus:

```bash
cargo xtask lane1-evidence-audit
cargo xtask evidence-quality-scorecard
cargo xtask check-output-contracts
cargo xtask check-traceability
cargo xtask check-capabilities
cargo xtask check-pr
git diff --check
```

## Closeout Conditions

This rail can close when:

- #1140 identifies remaining coverage gaps by evidence class;
- user-facing static unknown canonical items stay named and repair-routed;
- primary anchors and raw spans are complete enough for downstream placement
  without raw-finding duplication;
- the top named static limitation bucket from #1140 has a fixture-backed
  repair or a documented reason to remain unsupported;
- config/policy unsupported-flow expansion lands only for a selected
  fixture-backed category;
- actionable canonical items have repair-route coverage and verification
  coverage or explicit verify-command unknowns;
- scorecard output leads internally with actionable canonical gaps while
  keeping raw findings visible as diagnostics;
- runtime confidence coverage is visible by canonical evidence class;
- dogfood examples are refreshed for actual burn-down deltas;
- the downstream contract is refreshed only when material field or guidance
  changes land;
- capability and traceability updates remain class-scoped and proof-backed;
- a closeout handoff records what improved, what remains unknown, and the next
  evidence class to repair.
