# RIPR-SPEC-0043: Presentation Text Evidence

Status: proposed

## Problem

RIPR can currently expose changed presentation or help text as raw line-local
evidence. A string constant declaration and the following literal line can look
like two separate notices, and generic `static_unknown` guidance can suggest
heavy escalation when the useful next step is often a help-output snapshot,
golden-output assertion, visibility inspection, or no action.

Presentation text is behavior only when it reaches a user-visible output path.
Lane 1 needs a conservative evidence class that distinguishes user-visible
text, internal-only labels, existing observers, missing observers, and static
visibility limitations without moving that rendering work into PR/CI or editor
lanes.

## Behavior

RIPR should model changed presentation/help/report/table text as the
`presentation_text` evidence class when a diff changes a string constant,
rendered label, help text, report text, display table label, or nearby literal
that can plausibly describe output.

The class is advisory and confidence-scoped. Text alone must not create user
test debt. RIPR may create an actionable presentation-text gap only when it can
identify a user-visible output path or a fixture-backed high-confidence output
sink. Otherwise the record must be internal-only, already observed, or a static
limitation.

### Visibility

Each presentation-text record must classify visibility as:

| Value | Meaning |
| --- | --- |
| `user_visible` | The text flows to a supported output sink. |
| `internal_only` | The text appears confined to internal, proof-lane, config-only, or non-rendered use. |
| `unknown` | RIPR cannot statically trace the text to or away from a user-visible sink. |

Supported user-visible sinks are conservative and fixture-backed:

- CLI help output;
- help or documentation renderer output;
- report Markdown renderer output;
- display table or list renderer output;
- snapshot or golden-output fixture paths.

Unsupported paths remain static limitations:

- opaque helper output;
- macro-generated output;
- dynamic dispatch;
- cross-file indirect formatting without a clear sink;
- generated output paths without fixture-backed tracing.

### Observer Shape

Visible records should identify the strongest observer shape available:

| Value | Meaning |
| --- | --- |
| `snapshot` | A snapshot test or snapshot fixture observes the rendered text. |
| `cli_help_output` | A help-output test observes the rendered text. |
| `report_render` | A report-render test observes Markdown or text output. |
| `table_render` | A table/list renderer test observes the label. |
| `golden` | A golden output fixture observes the rendered text. |
| `none` | RIPR found a visible sink and found no observer. |
| `unknown` | Visibility or test topology prevents reliable observer discovery. |

Observer discovery must not overclaim from lexical mention alone. For
presentation text, snapshot, golden, help-output, and renderer tests are
stronger candidates than same-file helper tests unless direct owner-call or
fixture evidence says otherwise.

### Actionability

Each presentation-text record should classify actionability as:

| Value | Meaning |
| --- | --- |
| `snapshot_or_help_output_test` | User-visible output lacks an observer; add or update a help-output, snapshot, report, table, or golden-output test. |
| `already_observed` | User-visible output is already checked by a supported observer. |
| `no_action_internal` | The text is internal-only in documented scope. |
| `static_limitation_visibility_unknown` | RIPR cannot trace whether the text is user-visible. |
| `static_limitation_observer_unknown` | RIPR can see likely visibility but cannot identify observer coverage. |

The first action for presentation text should be output-path or observer work,
not mutation execution.

### Canonical Grouping

RIPR should group a constant declaration and its string literal into one
canonical presentation-text evidence item when they represent the same changed
constant:

```rust
pub const APPLE_M3_AIR_DEVICE_LABELS_TEXT: &str =
    "apple-m3-air-cpu-neon = M3 MacBook Air Apple CPU/NEON lane";
```

Expected grouping:

```text
canonical_gap_id: presentation_text::APPLE_M3_AIR_DEVICE_LABELS_TEXT
canonical_gap_group_size: 2
canonical_gap_reason: declaration_and_literal_same_text_constant
```

Different constants must not collide. Line movement must not change identity
when the constant name and literal evidence remain the same. Group size should
report the raw seam count so downstream surfaces can show one action while
retaining the raw evidence count.

### Static Limitation Categories

Presentation-text unknowns should use normalized static limitation categories:

| Category | Repair route |
| --- | --- |
| `presentation_text_visibility_unknown` | Trace the text constant to a rendered output path or confirm it is internal-only. |
| `presentation_text_observer_unknown` | Trace the visible sink to snapshot, help-output, report, table, or golden observer tests. |
| `presentation_text_internal_only` | Confirm the label stays internal and should not be projected as user test debt. |

These categories are Lane 1 maintenance signals. They should not be rendered as
user test failures.

## Required Evidence

An implemented presentation-text record must carry enough evidence for
downstream consumers to render one action/no-action/limitation state:

- `evidence_class = "presentation_text"`;
- source kind: `const_decl`, `string_literal`, or `rendered_label`;
- constant or owner name when available;
- changed literal text when safe to include in local reports;
- visibility classification and reason;
- observer shape and related-test candidate when available;
- actionability classification;
- canonical gap identity, group size, and grouping reason when raw seams group;
- static limitation category and repair route for unknowns;
- must-not-claim guard coverage in fixtures or benchmark cases.

The additive `evidence_record` shape should be compatible with existing v0.1
consumers. If the public contract changes, update RIPR-SPEC-0021 and output
schema docs in the implementation PR that introduces those fields.

## Inputs

Presentation-text analysis may use:

- diff hunks and changed Rust syntax facts;
- string constant declarations and literal spans;
- local constant references;
- supported renderer, help-output, report, table, snapshot, and golden-output
  sink facts;
- existing test/oracle facts;
- repository fixture metadata for supported observer shapes;
- configured test topology where it already exists.

Unsupported or ambiguous inputs must produce `unknown` visibility or observer
states instead of stronger claims.

## Outputs

The expected evidence-record subset is:

```json
{
  "evidence_class": "presentation_text",
  "canonical_gap_id": "presentation_text::APPLE_M3_AIR_DEVICE_LABELS_TEXT",
  "canonical_gap_group_size": 2,
  "canonical_gap_reason": "declaration_and_literal_same_text_constant",
  "presentation_text": {
    "source_kind": "const_decl",
    "constant": "APPLE_M3_AIR_DEVICE_LABELS_TEXT",
    "visibility": "unknown",
    "visibility_reason": "no_supported_output_sink_traced",
    "observer": "unknown",
    "actionability": "static_limitation_visibility_unknown",
    "recommended_observer": "unknown"
  },
  "static_limitation_category": "presentation_text_visibility_unknown",
  "static_limitation": {
    "repair_route": "trace_string_constant_to_output_or_snapshot_test",
    "user_actionability": "unknown_until_visibility_known"
  }
}
```

When visibility is `user_visible` and `observer = "none"`, the recommended
observer should name the most specific supported observer class available.

When visibility is `internal_only`, the record should set actionability to
`no_action_internal` and must not create a headline user test gap.

## Non-Goals

- No PR or CI annotation rendering changes.
- No inline PR comment publishing.
- No LSP or editor polish.
- No gate-policy changes or default blocking.
- No generated tests.
- No automatic source edits.
- No provider or model calls.
- No mutation execution.
- No score redefinition.
- No broad analyzer stability claim.
- No overclaiming visibility through opaque helpers, macros, dynamic dispatch,
  or unsupported cross-file formatting.

## Acceptance Examples

Screenshot-derived constant:

- Given the changed `APPLE_M3_AIR_DEVICE_LABELS_TEXT` declaration and literal,
  RIPR emits one presentation-text evidence item.
- It groups declaration and literal raw seams with group size `2`.
- If no supported output sink is traced, visibility is `unknown`.
- The static limitation category is `presentation_text_visibility_unknown`.
- RIPR does not recommend mutation execution as the first action.

User-visible unobserved help text:

- Given a constant flowing into CLI help output with no help-output snapshot,
  visibility is `user_visible`.
- Observer is `none`.
- Actionability is `snapshot_or_help_output_test`.
- Related-test ranking prefers help-output or snapshot tests over lexical-only
  same-file tests.

User-visible observed report label:

- Given a report/table label flowing to a renderer and a golden output fixture
  observing that label, visibility is `user_visible`.
- Observer is `golden` or `report_render`.
- Actionability is `already_observed`.
- No new RIPR action is emitted.

Internal-only proof-lane label:

- Given a label used only in an internal proof/config map with no rendered sink,
  visibility is `internal_only` when the supported local evidence is clear.
- Actionability is `no_action_internal`.
- RIPR does not treat the label as user test debt.

Opaque helper path:

- Given text passed through an unsupported helper or macro-generated renderer,
  visibility stays `unknown`.
- RIPR emits `presentation_text_visibility_unknown` or
  `presentation_text_observer_unknown`.
- RIPR does not infer user visibility from helper names alone.

Unrelated string literal:

- Given a string literal in a local variable, assertion message, or internal
  diagnostic that has no supported presentation-text shape, RIPR does not
  create a presentation-text action from text alone.

## Test Mapping

Follow-up implementation should include:

- benchmark validator coverage for presentation-text must-not-claim guards;
- analyzer tests for string constant detection and non-detection;
- fixture tests for user-visible unobserved help text;
- fixture tests for user-visible observed snapshot/help/report/table/golden
  output;
- fixture tests for internal-only labels;
- fixture tests for visibility-unknown opaque helpers and unsupported macros;
- canonical grouping tests for declaration and literal line movement;
- related-test ranking tests for observer-aware presentation-text candidates;
- scorecard/trend tests for presentation-text quality fields.

The first benchmark slice should pin the screenshot-derived constant before any
analyzer behavior changes.

## Implementation Mapping

Planned Lane 1 slices:

- `fixtures/presentation-text-evidence-benchmark` adds benchmark cases and
  guards.
- `analysis/presentation-text-evidence-fields` adds additive evidence-record
  fields.
- `analysis/presentation-text-canonical-grouping` groups declaration and
  literal raw seams. The initial implementation recognizes supported
  presentation-like `&str` constants, groups the declaration and adjacent
  literal raw findings under `declaration_and_literal_same_text_constant`, and
  emits visibility-unknown limitation context without claiming output-observer
  work.
- `analysis/presentation-text-visibility` classifies supported sinks and
  limitations. The initial implementation recognizes conservative help/report
  file plus constant-name sink patterns, supported snapshot/golden/help/report
  observer tests, and internal proof/policy/config labels while leaving
  unsupported routes as visibility-unknown limitations.
- `analysis/presentation-text-actionability` classifies action/no-action and
  richer observer-aware related-test ranking beyond the initial visibility
  states.
- `report/presentation-text-scorecard-trend-fields` reports quality counts and
  deltas.
- `docs/presentation-text-consumer-handoff` documents the downstream evidence
  contract.

Implementation must preserve the Lane 1 boundary: evidence truth changes are in
scope, projection rendering changes are not.

## Metrics

Presentation-text scorecard and trend work should expose:

- `presentation_text_total`;
- `presentation_text_user_visible`;
- `presentation_text_observed`;
- `presentation_text_unobserved`;
- `presentation_text_internal_only`;
- `presentation_text_visibility_unknown`;
- `presentation_text_observer_unknown`;
- `presentation_text_duplicate_groups`;
- `presentation_text_actionable_snapshot`;
- `presentation_text_no_action`;
- `presentation_text_static_limitations`.
