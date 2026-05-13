# RIPR-PROP-0005: User-Visible Output Evidence

Status: proposed

Owner: ripr maintainers

Created: 2026-05-13

Target campaign: Lane 1 User-Visible Output Evidence

Linked specs:

- `RIPR-SPEC-0043`: Presentation text evidence (planned)

Linked ADRs:

- ADR 0010: Fixture-first evidence confidence

Linked work items:

- To be recorded in the Lane 1 User-Visible Output Evidence tracker.

## Problem

Lane 1 has closed Evidence Quality Leadership in documented scope. RIPR now
has a stable `evidence_record`, scorecard, benchmark corpus, audit-driven
repairs, runtime-fixtures-v3, trend reporting, and class-scoped capability
proof.

That quality loop exposes the next evidence-class gap: changed presentation
text can still appear to users as raw line-local evidence. A changed string
constant may produce one notice for the declaration line and another for the
literal line, with generic `exposed` or `static_unknown` guidance. Reviewers
then have to infer whether the right response is to add a help-output snapshot,
update a golden report, confirm the label is internal-only, inspect a static
limitation, or take no action.

For user-visible text, the question is not "is this string interesting by
itself?" The useful question is whether the changed text reaches a visible
output sink, whether an observer already checks that output, and what confidence
boundary blocks a stronger claim. Lane 1 should encode that truth once so PR,
CI, editor, and agent surfaces can consume it without inventing their own
presentation-text confidence model.

## Users and surfaces

- Reviewers deciding whether changed help, label, report, or table text needs
  test attention.
- Maintainers of CLI, generated help, report Markdown, rendered tables, and
  golden-output surfaces.
- Developers reading CLI evidence, repo reports, PR annotations, or editor
  hovers.
- Coding agents consuming evidence records, first-useful-action packets, and
  repair routes.
- Downstream lanes that render Lane 1 evidence into PR/CI and editor surfaces.

This proposal primarily touches Lane 1 evidence truth: `evidence_record`
fields, benchmark fixtures, static limitation categories, canonical grouping,
related-test evidence for observers, scorecard/trend fields, and downstream
consumer contracts. Projection copy and annotation rendering belong to the
downstream surface lanes.

## Success criteria

- Changed presentation/help/report/table text is represented as a distinct
  evidence class.
- Declaration and string-literal lines for the same text constant group into
  one canonical evidence item.
- Evidence records classify visibility as `user_visible`, `internal_only`, or
  `unknown`.
- Visible text records identify observer shape when statically available:
  snapshot, CLI help output, report render, table render, or golden output.
- Actionability distinguishes add or update snapshot/help-output/report tests,
  already-observed output, no-action internal labels, and static limitations.
- Visibility and observer unknowns are normalized static limitation categories
  with repair routes.
- Text alone does not create user test debt.
- Mutation execution or runtime mutation testing is not the first recommended
  action for presentation text.
- Scorecard and trend outputs can show whether presentation-text evidence is
  improving over time.

## Proposed shape

Extend the Lane 1 evidence-quality loop to a new evidence class:

```text
proposal -> spec -> benchmark -> additive evidence fields ->
visibility and observer detection -> canonical grouping -> actionability ->
scorecard/trend fields -> downstream handoff
```

The core output should let consumers distinguish:

- user-visible text with no observer: add or update a snapshot, help-output,
  report, table, or golden-output test;
- user-visible text with an observer: no new RIPR action;
- internal-only text: no user test action;
- visibility unknown: static limitation with a route to trace output flow;
- observer unknown: static limitation or low-confidence action, depending on
  the visibility evidence;
- declaration and literal raw lines: one canonical item, not two action items.

Behavior contracts belong in `RIPR-SPEC-0043` and later output-contract specs
if the schema grows. This proposal owns the motivation, boundaries, and campaign
shape.

## Alternatives considered

| Alternative | Why we are not picking it |
| --- | --- |
| Keep raw line-local annotations. | Technically correct evidence still leaves reviewers guessing between snapshot, no action, or limitation. |
| Suppress all changed strings. | Some presentation text is user-visible behavior and should have observer evidence. |
| Always recommend snapshot tests for strings. | Internal labels and unknown visibility should not become user test debt. |
| Always recommend mutation testing for `static_unknown` strings. | Presentation text usually needs output/snapshot observer evidence first; runtime mutation escalation is too broad. |
| Let PR/CI rendering infer presentation text behavior. | Downstream surfaces should consume Lane 1 evidence truth instead of creating parallel confidence models. |
| Infer user visibility through opaque helpers or macros. | Unknown is acceptable and should remain a static limitation until fixture-backed support exists. |

## Behavior specs to create or update

- `RIPR-SPEC-0043`: Presentation text evidence.
- `RIPR-SPEC-0035`: Evidence quality benchmark corpus, when the
  presentation-text benchmark cases land.
- `RIPR-SPEC-0034`: Evidence quality scorecard, if scorecard/trend fields are
  added for this class.
- `RIPR-SPEC-0021`: Evidence record, if additive presentation-text fields are
  promoted into the public evidence-record contract.

## Architecture decisions needed

No new ADR is planned for this campaign. ADR 0010 already requires
fixture-first, class-scoped evidence confidence. Add an ADR only if the
presentation-text work changes the evidence model's maturity policy or creates
a durable architecture rule not covered by ADR 0010.

## Implementation campaign shape

Each slice follows the [scoped PR contract](../SCOPED_PR_CONTRACT.md).

1. `docs/proposal-user-visible-output-evidence`
2. `docs/spec-presentation-text-evidence`
3. `fixtures/presentation-text-evidence-benchmark`
4. `analysis/presentation-text-evidence-fields`
5. `analysis/presentation-text-visibility`
6. `analysis/presentation-text-canonical-grouping`
7. `analysis/presentation-text-actionability`
8. `report/presentation-text-scorecard-trend-fields`
9. `docs/presentation-text-consumer-handoff`

The existing benchmark PR for the screenshot-derived constant should be treated
as the benchmark slice after the proposal and spec foundation land.

## Evidence plan

- Proposal and tracker state define the campaign boundary and non-goals.
- `RIPR-SPEC-0043` defines visibility, observer, actionability, canonical
  grouping, static limitation, and must-not-claim behavior.
- Benchmark fixtures include positive, negative, line-grouping, internal-only,
  visibility-unknown, observer-unknown, and unrelated-string cases.
- Analyzer changes are preceded by fixture cases and must-not-claim guards.
- Evidence-record changes are additive and backward compatible.
- Scorecard and trend fields count presentation-text total, visible, observed,
  unobserved, internal-only, visibility-unknown, observer-unknown, duplicate
  groups, and actionable snapshot/help-output cases.
- Capability updates remain class-scoped and proof-backed.
- Traceability links specs, fixtures, tests, code, output contracts, metrics,
  and closeout artifacts as each behavior lands.

## Risks

- String changes can look actionable even when they are internal labels.
  Mitigation: text alone does not create user test debt.
- Visibility heuristics can overclaim through helpers, macros, or indirect
  formatting. Mitigation: unsupported routes stay static limitations.
- Grouping declaration and literal lines can hide distinct changes. Mitigation:
  group only same constant declaration and literal evidence, and keep group size
  visible.
- Snapshot recommendations can become generic test generation. Mitigation: RIPR
  recommends observer shape and related test candidates only; it does not
  generate tests or edit source.
- Downstream surfaces can treat low-confidence presentation-text evidence as
  policy. Mitigation: no gate, default-blocking, PR/CI rendering, or editor
  behavior changes in Lane 1.

## Non-goals

- No PR or CI rendering changes in Lane 1.
- No inline PR comment publishing.
- No LSP or editor polish.
- No gate-policy changes or default blocking.
- No generated tests.
- No automatic source edits.
- No provider or model calls.
- No mutation execution.
- No score redefinition.
- No broad analyzer stability claim.

## Exit criteria

This proposal can move to `accepted` when:

- `RIPR-SPEC-0043` is merged;
- the presentation-text benchmark corpus exists with must-not-claim guards;
- additive evidence fields are documented or implemented for visibility,
  observer shape, actionability, source kind, static limitation, and canonical
  grouping;
- fixture-backed analyzer behavior classifies at least user-visible observed,
  user-visible unobserved, internal-only, and visibility-unknown cases;
- scorecard or trend output reports presentation-text evidence quality;
- downstream consumer handoff documents the evidence contract without changing
  PR/CI or editor rendering;
- capability and traceability updates are class-scoped and proof-backed;
- a closeout handoff records what improved, what remains unknown, and which
  presentation-text repair should happen next.
