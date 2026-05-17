# Handoff: Lane 1 Shippable Finding Alignment Closeout

Date: 2026-05-17
Branch / PR: `codex/evidence-alignment-audit` / #1117
Latest merged PR: #1109 `analysis: add predicate boundary repair routes`
(commit `fb7a1c13`)

## Current work item

Lane 1 Evidence Accuracy is complete for the 0.6.x shippable finding-alignment
scope. The lane keeps raw analyzer findings available for traceability and
debugging, but the countable and user-facing unit is now the canonical evidence
item:

```text
raw findings -> canonical evidence item -> actionable gap / no-action state / named limitation
```

The invariant is:

```text
Raw findings are evidence.
Canonical items are the countable unit.
Actionable canonical gaps are user work.
```

This closeout does not change analyzer behavior, public rendering, LSP/editor
behavior, PR comments, gates, badges, baselines, suppressions, generated tests,
provider calls, source edits, default blocking, or mutation execution.

## What shipped

- #1093 hardened the Lane 1 coverage audit so actionable canonical items must
  carry structured `repair_route` data and generic `static_unknown` limitation
  categories are counted as alignment gaps.
- #1102 bounded the Lane 1 audit producer and made timeout failures report
  phase context instead of becoming invisible hangs.
- #1106 surfaced Lane 1 alignment reliability gaps in the evidence-quality
  scorecard and trend reports.
- #1109 added structured predicate-boundary repair routes so actionable
  predicate-boundary canonical items no longer rely on prose-only repair text.

The result is a shippable 0.6.x evidence contract: raw findings remain
supporting evidence, canonical items carry the Lane 1 evidence state, and
actionable canonical gaps carry concrete repair and verification guidance.

## Audit snapshot

The clean `origin/main` audit snapshot generated
`target/ripr/reports/lane1-evidence-audit.{json,md}` with these current counts:

| Metric | Count |
| --- | ---: |
| Raw alignment signals | 47,181 |
| Canonical alignment items | 38,027 |
| Actionable canonical gaps | 149 |
| Already observed items | 10,551 |
| Static limitation items | 27,327 |
| Unaligned raw findings | 0 |
| Static unknown without named limitation | 0 |
| Canonical items without repair route | 0 |
| Canonical items without verify command | 0 |
| Raw-to-canonical ratio | 1.24 |

This is the release-readable evidence statement:

```text
47,181 raw signals -> 38,027 canonical items -> 149 actionable gaps,
with zero actionable canonical items missing repair routes or verify commands.
```

The actionable items are currently predicate-boundary repair work. Other
evidence classes are either already observed or named static limitations in
the repo-local audit snapshot.

## Downstream contract

Downstream surfaces must consume canonical items first:

- `finding_alignment.items[]` for `ripr check --json` class-specific
  alignment output.
- `seams[].evidence_record.canonical_item` for repo-exposure and seam-native
  consumers.

They may show `raw_findings[]` as supporting detail, but must not infer user
work from raw `exposed`, `weakly_exposed`, `reachable_unrevealed`, or
`static_unknown` labels alone.

Actionable user-facing work requires:

- `gap_state = "actionable"`;
- class-scoped `actionability`;
- structured `repair_route`;
- `recommended_repair`;
- `verify_command` when feasible.

No-action and limitation states are first-class:

- `already_observed` means no new RIPR repair action.
- `internal_only` means no user test action in documented scope.
- `static_limitation` means named analyzer backlog or inspection work, not user
  test debt.
- `unknown` means the evidence is explicit uncertainty, not a repair card.

Policy overlays such as baseline-known, suppressed, waived, blocked, resolved,
or reintroduced remain owned by policy lanes. They must not replace the Lane 1
`gap_state`.

## Release truth

0.6.x release copy can claim:

- raw analyzer signals are rolled up into canonical evidence items;
- actionable canonical gaps carry structured repair routes and verification
  commands;
- generic static-unknown states are named limitations before they reach
  user-facing evidence accounting;
- the scorecard starts with actionable canonical gaps and keeps raw counts as
  diagnostic context.

0.6.x release copy must not claim:

- runtime mutation adequacy;
- coverage adequacy;
- general correctness;
- default CI blocking;
- preview-language gate authority;
- automatic source edits or generated tests.

## Verification run

Latest local proof for this closeout should include:

```text
rtk cargo test -p xtask lane1_evidence_audit --bin xtask
rtk cargo test -p xtask evidence_quality_scorecard --bin xtask
rtk cargo xtask check-doc-index
rtk cargo xtask markdown-links
rtk cargo xtask check-static-language
rtk cargo xtask check-traceability
rtk cargo xtask check-capabilities
rtk cargo xtask check-pr
rtk git diff --check
```

Release operators making evidence-alignment claims should also run:

```text
rtk cargo xtask lane1-evidence-audit
rtk cargo xtask evidence-quality-scorecard
```

and confirm the audit coverage section reports:

```text
Static unknown without named limitation: 0
Canonical items without repair route: 0
Canonical items without verify command: 0
```

## Artifacts

- [RIPR-SPEC-0031 Lane 1 evidence quality audit](../specs/RIPR-SPEC-0031-lane1-evidence-quality-audit.md)
- [RIPR-SPEC-0045 finding-to-gap alignment](../specs/RIPR-SPEC-0045-finding-to-gap-alignment.md)
- [RIPR-SPEC-0048 config/policy constant evidence](../specs/RIPR-SPEC-0048-config-policy-constant-evidence.md)
- [Finding Alignment Consumer Contract v2](2026-05-16-finding-alignment-consumer-contract-v2.md)
- [Capability matrix](../CAPABILITY_MATRIX.md)
- [Output schema](../OUTPUT_SCHEMA.md)
- `target/ripr/reports/lane1-evidence-audit.{json,md}`
- `target/ripr/reports/evidence-quality-scorecard.{json,md}`

## Remaining limits

- Explicit `primary_anchor` and `raw_spans[]` fields remain a documented
  projection contract, not a completed public field on every evidence record.
- Config/policy constant alignment is heuristic and supported-sink scoped;
  opaque lookup, generated config, macros, and unsupported cross-file flows
  stay named limitations until fixture-first analyzer work expands them.
- Runtime calibration is imported when supplied; this lane does not run
  mutation testing or promote static evidence to runtime proof.
- Scorecard and audit counters are advisory evidence-quality reports. They do
  not redefine gates, badges, baselines, suppressions, or default blocking.

## Recommended next action

Keep Lane 1 finding alignment in maintenance. Open a new Lane 1 slice only when
`cargo xtask lane1-evidence-audit`, `cargo xtask evidence-quality-scorecard`, a
dogfood receipt, or a downstream consumer identifies a concrete uncovered
evidence class.

## What not to do

- Do not reopen the finding-to-gap model without a measured coverage gap.
- Do not route raw findings directly into user-facing work.
- Do not treat named static limitations as user test debt.
- Do not change gate, badge, PR, CI, LSP, or policy authority from this lane.
- Do not promote TypeScript or Python preview evidence from this Rust-focused
  closeout.
