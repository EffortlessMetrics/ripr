# Handoff: Presentation Text Consumer Contract

Date: 2026-05-14
Branch / PR: `lane1-presentation-text-consumer-handoff` / #959
Latest merged PR: #959 `docs: hand off presentation text consumer contract` (commit `595a9933`)

## Current work item

`docs/presentation-text-consumer-handoff`

Lane 1 now emits fixture-backed presentation-text finding alignment for
supported `ripr check --json` cases and reports those counts through the
evidence-quality scorecard and trend reports. PR #957 landed the preceding
report slice. This handoff tells downstream PR/CI, editor, agent, and report
lanes which fields are authoritative and which fields are supporting evidence.

## Consumer contract

Downstream surfaces should render canonical evidence items before raw findings.
For check output, the authoritative projection is:

```text
finding_alignment.items[]
```

For repo-exposure and seam-native surfaces, the matching authoritative evidence
projection is:

```text
seams[].evidence_record.canonical_item
```

Raw `findings[]` and `raw_findings[]` remain visible as supporting evidence for
line, expression, probe kind, and static class context. They are not the
default user-facing action unit.

## Fields to prefer

Consumers that need one thing to render should read:

| Field | Use |
| --- | --- |
| `canonical_gap_id` | Stable grouping key and dedupe identity. |
| `evidence_class` | Evidence class such as `presentation_text`. |
| `gap_state` | Lane 1 evidence state: `actionable`, `already_observed`, `internal_only`, `static_limitation`, or `unknown`. |
| `actionability` | Class-scoped action label such as `add_output_observer`, `already_observed`, `no_action`, or `inspect_visibility`. |
| `why` | Short explanation for the state. |
| `recommended_repair` | User repair, no-action explanation, or limitation repair route. |
| `related_test` | Candidate observer or repair location when known. |
| `verify_command` | Verification command when known. |
| `static_limitations[]` | Analyzer limitation category and repair route. |
| `confidence` | Fixture-backed, calibrated, static-only, or unknown basis. |
| `presentation_text` | Presentation-text visibility, observer, repair kind, target test type, and suggested assertion context. |

## Rendering guidance

For `gap_state = "actionable"` and
`presentation_text.actionability = "add_output_observer"`, render one output
observer repair, for example:

```text
Changed user-visible help/report text. No supported output observer was found.
Add or update a help-output, snapshot, report-render, table-render, or golden-output test.
```

For `gap_state = "already_observed"`, render a no-new-action state:

```text
Changed rendered output text. A supported output observer already checks it.
No new RIPR action is needed.
```

For `gap_state = "internal_only"`, render internal/no-action language:

```text
Changed internal label text. RIPR found no supported user-visible output path.
No test debt is claimed.
```

For `gap_state = "static_limitation"`, render the named limitation and repair
route, not a user test gap:

```text
Changed presentation text, but RIPR cannot trace visibility through the current static model.
Static limitation: presentation_text_visibility_unknown.
Inspect the output path or confirm the label is internal-only.
```

## Counting guidance

Use raw counts for analyzer debugging and canonical counts for user work:

| Count | Meaning |
| --- | --- |
| `raw_signals` | All analyzer findings before alignment. |
| `canonical_items` | Supported grouped evidence items after alignment. |
| `actionable_gaps` | Canonical items with concrete user repairs. |
| `already_observed` | Canonical items already gripped by a supported observer. |
| `internal_no_action` | Canonical items that should not become user test debt. |
| `static_limitations` | Analyzer backlog, not user test debt. |
| `calibrated_supported` | Canonical items with checked imported runtime support. |
| `uncalibrated` | Static-only canonical evidence in documented scope. |

User-facing summaries should not use raw signal count as the number of things
to fix.

## Policy overlay boundary

Lane 1 owns evidence state. Policy and adoption lanes may overlay:

```text
baseline_known
acknowledged
suppressed
waived
blocked
resolved
reintroduced
```

Those overlays must not replace `gap_state`. For example, a baseline-known
actionable item is still Lane 1 `gap_state = "actionable"` with a policy
overlay.

## What not to infer

- Do not infer actionability from raw `exposed` or `static_unknown` labels.
- Do not treat every raw finding as a separate user action.
- Do not treat static limitations as user test debt.
- Do not infer presentation-text visibility from string text alone.
- Do not recommend mutation testing as the first presentation-text repair.
- Do not infer baseline, waiver, suppression, or blocking authority from Lane 1
  evidence fields alone.
- Do not hide raw findings; attach them as evidence details.

## Verification run

The preceding report slice (#957) was validated with:

```bash
cargo fmt --check
cargo test -p xtask finding_alignment_presentation_text
cargo test -p xtask evidence_quality_scorecard
cargo test -p xtask evidence_quality_trend
cargo xtask check-output-contracts
cargo xtask check-traceability
cargo xtask check-capabilities
cargo xtask check-static-language
cargo xtask check-pr
git diff --check
```

The handoff PR (#959) was docs-only and validated the documentation,
capability, traceability, and PR gates before merge.

## Artifacts

- `docs/specs/RIPR-SPEC-0045-finding-to-gap-alignment.md`
- `docs/specs/RIPR-SPEC-0043-presentation-text-evidence.md`
- `docs/OUTPUT_SCHEMA.md`
- `docs/lanes/LANE_1_USER_VISIBLE_OUTPUT_EVIDENCE.md`
- `docs/CAPABILITY_MATRIX.md`
- `metrics/capabilities.toml`
- `.ripr/traceability.toml`

## Recommended next action

Review the Lane 1 User-Visible Output Evidence closeout conditions and open a
focused closeout PR if the lane owner wants to close this tracker.

## What not to do

- Do not change PR/CI rendering in this Lane 1 handoff PR.
- Do not change LSP/editor behavior.
- Do not change gate policy, default blocking, scores, schemas, generated tests,
  provider calls, or mutation execution.
- Do not reopen generic Evidence Quality Leadership work; this lane is a
  focused presentation-text evidence-class expansion.
