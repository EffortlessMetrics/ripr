# Handoff: Finding Alignment Consumer Contract v2

Date: 2026-05-16
Branch / PR: `lane1-finding-alignment-contract-v2` / #1042
Latest prerequisite PR: #1034 `dogfood: record finding alignment examples`
(commit `613c640f`)

## Current work item

`docs/canonical-gap-action-consumer-handoff`

Lane 1 now has fixture-backed finding alignment beyond presentation text:
presentation-text evidence, config/policy constant evidence, repair routes,
verify-command coverage, scorecard headline counts, and dogfood examples all
flow through the canonical item model. This handoff is the v2 downstream
consumer contract for PR/CI, editor, report, and agent lanes.

It does not change rendering, gates, scores, generated tests, provider calls,
source edits, or mutation execution.

## Consumer invariant

Downstream surfaces must keep these layers separate:

```text
Raw findings are analyzer evidence.
Canonical evidence items are the countable unit.
Actionable canonical gaps are the user-facing problem.
```

Raw findings remain available for line, expression, probe kind, static class,
audit, and debugging context. They are supporting evidence, not the default
headline count and not independent user action items.

## Authoritative projections

For `ripr check --json`, render canonical items from:

```text
finding_alignment.items[]
```

For repo exposure and seam-native consumers, render the matching canonical
item from:

```text
seams[].evidence_record.canonical_item
```

Consumers may show raw `findings[]`, `raw_findings[]`, or `raw_spans[]` as
expandable supporting detail after the canonical item.

## Fields to prefer

Consumers that need one rendered item should prefer these fields:

| Field | Use |
| --- | --- |
| `canonical_gap_id` | Stable grouping and dedupe identity. |
| `evidence_class` | Class such as `presentation_text` or `config_or_policy_constant`. |
| `gap_state` | Lane 1 evidence state: `actionable`, `already_observed`, `internal_only`, `static_limitation`, or `unknown`. |
| `actionability` | Class-scoped action label such as `add_output_observer`, `add_behavior_discriminator`, `no_action`, or `inspect_flow`. |
| `why` | Short explanation for the evidence state. |
| `recommended_repair` | User repair, no-action explanation, or limitation repair route. |
| `repair_route` | Normalized repair contract with `repair_kind`, `target_test_type`, and `suggested_assertion` when actionable. |
| `related_test` or `related_observer` | Best known repair location or observer. |
| `verify_command` | Verification route when known. |
| `primary_anchor` | Preferred annotation placement when a surface needs one line. |
| `raw_findings[]` or `raw_spans[]` | Supporting line-local evidence only. |
| `static_limitations[]` | Named analyzer limitation categories and repair routes. |
| `confidence` | Fixture-backed, calibrated, static-only, or unknown basis. |
| `presentation_text` | Presentation-text visibility, observer kind, repair kind, and target test type. |
| `config_policy` | Config/policy role, visibility, observer kind, repair kind, and limitation category. |

## Evidence states

Render `gap_state` as Lane 1 evidence truth:

| `gap_state` | Meaning | Consumer posture |
| --- | --- | --- |
| `actionable` | A concrete user repair is known. | Render one repair route and verification route when present. |
| `already_observed` | Existing evidence observes the behavior or output. | Render no-new-action language. |
| `internal_only` | The item is internal metadata or no-action behavior in documented scope. | Render no-action language. |
| `static_limitation` | RIPR cannot make the stronger claim safely. | Render the named limitation and repair route; do not call it user test debt. |
| `unknown` | The evidence state is not yet classifiable. | Render uncertainty and the next inspection route if present. |

Policy and adoption lanes may overlay `baseline_known`, `acknowledged`,
`suppressed`, `waived`, `blocked`, `resolved`, or `reintroduced`. Those are not
Lane 1 evidence states and must not replace `gap_state`.

## Counting guidance

Use canonical counts for user work and raw counts for analyzer diagnostics:

| Count | Meaning |
| --- | --- |
| `raw_findings_total` or `raw_signals_total` | Diagnostic analyzer signal volume. |
| `canonical_items_total` | Countable aligned evidence units. |
| `actionable_canonical_gaps` | User-facing repair work. |
| `already_observed_items` | No-new-action evidence items. |
| `internal_no_action_items` | No-action internal items. |
| `static_limitation_items` | Named analyzer limitations, not user debt. |
| `unknown_items` | Explicit uncertainty. |
| `raw_to_canonical_ratio` | Diagnostic dedupe/alignment signal, not a score. |
| `repair_route_coverage` | Share of actionable items with concrete repair routes. |
| `verify_command_coverage` | Share of actionable items with verification routes. |

Do not summarize a PR as having one user problem per raw finding.

## Class guidance

For `presentation_text`, consumers should render:

- `actionable` plus `add_output_observer` as an output-observer repair:
  add or update a help-output, snapshot, report-render, table-render, or
  golden-output test.
- `already_observed` as no new RIPR action.
- `internal_only` as no test debt.
- `static_limitation` as a named visibility or observer limitation, such as
  `presentation_text_visibility_unknown`.

For `config_or_policy_constant`, consumers should render:

- internal policy metadata as no action.
- rendered config/report labels without observers as output-observer repairs.
- behavior selectors without discriminators as behavior-discriminator repairs.
- cross-file, opaque, dynamic, or unsupported flows as named static
  limitations, such as `config_policy_flow_unknown` or
  `config_policy_observer_unknown`.

## Primary anchor and raw spans

When a surface needs one inline placement, use `primary_anchor` when present.
Attach every contributing raw span as supporting evidence. Same-line and
adjacent-line raw findings may map to one canonical item; consumers should not
emit duplicate action comments for each raw span.

If a raw finding has no canonical item, render it only in diagnostic or
developer-detail surfaces unless a later Lane 1 contract gives it a
`gap_state`, `actionability`, and repair route.

## What not to infer

- Do not infer actionability from raw `exposed`, `weakly_exposed`, or
  `static_unknown` classes.
- Do not treat every raw finding as a separate user problem.
- Do not treat `static_limitation` as user test debt.
- Do not treat internal policy constants as user-visible behavior.
- Do not infer user visibility from string text alone.
- Do not recommend mutation testing as the first repair for output or
  config/policy text.
- Do not infer baseline, waiver, suppression, acknowledgement, or blocking
  authority from Lane 1 evidence fields alone.
- Do not hide raw findings; attach them as supporting evidence after the
  canonical item.

## Verification run

The prerequisite dogfood PR (#1034) was validated with:

```bash
cargo test -p xtask finding_alignment --bin xtask
cargo xtask dogfood
cargo xtask check-traceability
cargo xtask check-doc-index
cargo xtask markdown-links
cargo xtask check-static-language
cargo xtask check-no-panic-family
cargo xtask check-pr
git diff --check
```

This handoff PR should remain docs-only and rerun the documentation,
traceability, static-language, and PR checks before merge.

## Artifacts

- `docs/specs/RIPR-SPEC-0045-finding-to-gap-alignment.md`
- `docs/specs/RIPR-SPEC-0043-presentation-text-evidence.md`
- `docs/specs/RIPR-SPEC-0048-config-policy-constant-evidence.md`
- `docs/OUTPUT_SCHEMA.md`
- `docs/DOGFOODING.md`
- `fixtures/finding-alignment-dogfood/SPEC.md`
- `fixtures/finding-alignment-dogfood/corpus.json`
- `docs/handoffs/2026-05-14-presentation-text-consumer-handoff.md`

## Recommended next action

Downstream lanes can opt into canonical item rendering using this contract.
Lane 1 should continue class-by-class alignment only when the scorecard,
coverage audit, or dogfood receipts identify a concrete unaligned evidence
class.

## What not to do

- Do not change PR/CI rendering in this Lane 1 handoff PR.
- Do not change LSP/editor behavior.
- Do not change gate policy, default blocking, public scores, generated tests,
  provider calls, source edits, or mutation execution.
- Do not reopen presentation text; it is the first supported class, not the
  whole alignment model.
