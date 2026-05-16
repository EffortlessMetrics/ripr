# RIPR-SPEC-0045: Finding-To-Gap Alignment

Status: proposed

## Problem

RIPR emits raw static findings such as `exposed`, `weakly_exposed`,
`reachable_unrevealed`, `no_static_path`, `infection_unknown`,
`propagation_unknown`, and `static_unknown`. Those findings are useful
evidence fragments, but they are not always user-facing work items.

A single semantic change can produce multiple raw findings. A changed constant
declaration and its assigned string literal can appear as two line-local
notices even when the reviewer needs one answer:

```text
Is this a real test-grip gap, already observed behavior, internal no-action
change, static limitation, or policy/baseline state?
```

Lane 1 needs a shared alignment contract so downstream PR/CI, editor, agent,
and report surfaces consume one canonical evidence item instead of inventing
their own actionability from raw finding labels.

## Behavior

RIPR should preserve raw findings and roll them up into canonical evidence
items. The canonical item is the user-facing grouping unit. Raw findings remain
supporting evidence.

Each canonical evidence item must have:

- one `canonical_gap_id` or equivalent stable item identity;
- an `evidence_class`;
- a `gap_state`;
- `actionability`;
- a primary anchor when the item is eligible for a user-facing annotation,
  diagnostic, or repair packet;
- raw spans or raw findings that explain every contributing analyzer signal;
- a concise `why` explanation;
- a recommended repair or no-action/limitation explanation;
- related-test and verification commands when known;
- static limitation details when the state is limitation-backed;
- a confidence basis that states whether the claim is fixture-backed,
  calibrated, static-only, or unknown;
- the raw findings that contributed to the item.

### Raw Findings

Raw findings remain available for audit, debugging, and line-local support:

| Field | Meaning |
| --- | --- |
| `file` | Source file for the raw finding. |
| `line` | Changed or reported line. |
| `span` | Optional source span when available. |
| `kind` | Static finding label such as `exposed` or `static_unknown`. |
| `expression` | Expression or syntax fragment that produced the finding. |
| `probe_kind` | Probe or seam family when available. |
| `source_id` | Stable seam/probe/source identity when available. |
| `evidence_record_ref` | Link to the existing evidence record when the raw finding came from one. |

Downstream user surfaces may show raw findings as supporting context, but must
not treat each raw finding as an independent action by default.

### Primary Anchor And Raw Spans

Canonical items separate placement from supporting evidence:

| Field | Meaning |
| --- | --- |
| `primary_anchor` | The one preferred file/line/span/symbol target for rendering or repair routing. |
| `raw_spans[]` | Every contributing raw source span, including duplicate, adjacent, or same-line raw findings. |
| `raw_findings[]` | Full raw finding records with static class, expression, probe kind, and evidence references. |

`primary_anchor` is required before another lane may publish a user-facing
inline annotation, editor diagnostic, or repair packet for an aligned item. If
Lane 1 cannot choose a safe primary anchor, the item may still appear in
repo-local reports, but downstream surfaces must treat it as report-only until
an anchor is supplied by a later evidence or policy artifact.

Anchor selection must be deterministic and class-scoped:

- prefer the semantic owner or changed declaration over a supporting literal,
  helper call, or derived expression;
- keep adjacent declaration-plus-literal groups as one item with one primary
  anchor and multiple raw spans;
- keep same-line duplicate raw findings as one item with one primary anchor;
- keep line movement stable through `canonical_gap_id` even when the concrete
  anchor line changes;
- avoid using raw line location alone when different owners, discriminators, or
  evidence classes could collide.

For presentation text constants, the default primary anchor is the constant
declaration line when present. The assigned string literal line remains a raw
span. Literal-only findings may use the literal span as primary anchor only
when the owning constant or rendered-label owner is known.

### Canonical Item Contract

The planned additive evidence-record subset is:

```json
{
  "raw_findings": [
    {
      "file": "src/devices.rs",
      "line": 46,
      "kind": "exposed",
      "expression": "pub const APPLE_M3_AIR_DEVICE_LABELS_TEXT: &str ="
    },
    {
      "file": "src/devices.rs",
      "line": 47,
      "kind": "static_unknown",
      "expression": "\"apple-m3-air-cpu-neon = M3 MacBook Air Apple CPU/NEON lane\""
    }
  ],
  "canonical_gap_id": "presentation_text::APPLE_M3_AIR_DEVICE_LABELS_TEXT",
  "canonical_item_kind": "gap",
  "evidence_class": "presentation_text",
  "gap_state": "static_limitation",
  "actionability": "inspect_visibility",
  "primary_anchor": {
    "file": "src/devices.rs",
    "line": 46,
    "span": null,
    "symbol": "APPLE_M3_AIR_DEVICE_LABELS_TEXT"
  },
  "raw_spans": [
    {
      "file": "src/devices.rs",
      "line": 46,
      "span": null,
      "role": "declaration"
    },
    {
      "file": "src/devices.rs",
      "line": 47,
      "span": null,
      "role": "assigned_literal"
    }
  ],
  "group_reason": "declaration_and_literal_same_text_constant",
  "why": "Changed presentation text could not be traced to or away from a user-visible output sink.",
  "recommended_repair": "Trace the string constant to a rendered output path or confirm it is internal-only.",
  "repair_route": null,
  "related_test": null,
  "verify_command": "cargo xtask evidence-quality-scorecard",
  "static_limitations": [
    {
      "category": "presentation_text_visibility_unknown",
      "repair_route": "trace_string_constant_to_output_or_snapshot_test"
    }
  ],
  "confidence": {
    "basis": "fixture_backed",
    "notes": [
      "Visibility-unknown presentation text is benchmark-pinned as a static limitation, not user test debt."
    ]
  }
}
```

This is additive. Existing evidence-record v0.1 consumers may keep using legacy
fields until they opt into aligned canonical items. The first implementation
adds `raw_findings[]` and `canonical_item` to `seams[].evidence_record` while
leaving the existing `actionability` object intact; the class-scoped alignment
label lives at `canonical_item.actionability`. Explicit `primary_anchor` and
`raw_spans[]` fields are the contract for follow-up projection work that needs
one authoritative placement target. Until those fields are present in a
specific output, consumers must not infer projectable placement from every raw
finding line.

### Counting Model

Raw signal counts are analyzer-debug numbers. They should stay visible in
detailed reports, but they are not the user-facing count of work to do.

Aligned reports should distinguish:

| Count | Meaning |
| --- | --- |
| `raw_signals` | All emitted analyzer findings or signals before alignment. |
| `canonical_items` | Grouped evidence items after dedupe and alignment. |
| `actionable_gaps` | Canonical items with a concrete user repair. |
| `already_observed` | Canonical items where current evidence says the behavior is already observed. |
| `internal_no_action` | Canonical items that are internal-only or otherwise no-action in documented scope. |
| `static_limitations` | Canonical items blocked by named analyzer limitations. These are analyzer backlog, not user test debt. |
| `calibrated_supported` | Canonical items whose static claim has checked imported runtime support in class-scoped calibration. |
| `uncalibrated` | Canonical items that remain static-only confidence in documented scope. |

The user-facing action headline should be based on actionable unresolved
canonical gaps, not raw signal count. Raw signals remain visible as supporting
detail so evidence is not hidden.

Policy/adoption overlays may further split canonical items into baseline-known,
acknowledged, suppressed, waived, blocked, resolved, or reintroduced states,
but those overlays must not redefine the underlying Lane 1 evidence count.

### Gap State

Each canonical item should classify the evidence state with one of these
Lane 1-owned states:

| State | Meaning |
| --- | --- |
| `actionable` | RIPR has enough evidence to recommend a concrete test or assertion repair. |
| `already_observed` | The changed behavior appears observed by a supported test or output observer. |
| `internal_only` | The change appears internal in documented scope and should not become user test debt. |
| `static_limitation` | RIPR cannot make the stronger claim because a named static limitation blocks it. |
| `unknown` | RIPR lacks enough evidence to classify actionability or limitation precisely. |

Policy and adoption lanes may overlay these states:

| Policy state | Owner |
| --- | --- |
| `baseline_known` | Baseline/ledger policy. |
| `acknowledged` | Suppression or waiver policy. |
| `suppressed` | Suppression policy. |
| `resolved` | Baseline or movement comparison. |
| `reintroduced` | Baseline or movement comparison. |

Policy states must not replace Lane 1 evidence state. A suppressed actionable
gap is still an actionable evidence item with a policy overlay.

### Actionability

Actionability is separate from raw finding class and separate from visibility.
The allowed value set is class-scoped. For presentation text, the planned
values are:

| Actionability | Meaning |
| --- | --- |
| `add_output_test` | Add or update a help-output, snapshot, report, table, or golden-output observer. |
| `already_observed` | No new test action is needed because a supported observer exists. |
| `no_action` | No test action is needed in documented scope. |
| `inspect_visibility` | Determine whether the text reaches user-visible output. |
| `static_limitation` | A named limitation blocks stronger actionability. |
| `unknown` | The class does not yet provide a sharper actionability label. |

Other evidence classes may define their own actionability values, but they must
map to the same `gap_state` vocabulary.

### Evidence Class Rules

Each evidence class must define what counts as action, already-observed,
internal/no-action, limitation, and must-not-infer behavior before Lane 1 marks
the class stable.

| Evidence class | Actionable when | Already observed when | Internal/no-action when | Limitation when | Must not infer |
| --- | --- | --- | --- | --- | --- |
| `behavior_boundary` | A reachable owner lacks an exact boundary value or discriminator assertion. | A related test observes the exact boundary value and discriminating result. | The changed boundary is outside user-observable behavior in documented scope. | Activation values, constants, or owner flow cannot be resolved. | Broad reach, smoke assertions, or coverage-like signals are enough. |
| `error_path` | A related test checks broad error shape but misses an exact error variant or payload discriminator. | A related test asserts the exact variant or discriminating payload. | The error path is unreachable or intentionally internal in documented scope. | Helper, macro, or dynamic error construction hides the observed discriminator. | `is_err`, `to_string`, or helper names prove exact error observation. |
| `return_value` | A changed returned or constructed value is reached but not exactly observed. | A related test observes the exact returned value or changed field. | The value is internal-only or intentionally not policy-relevant. | Value origin, builder override, or cross-file constant resolution is unsupported. | A call, unwrap, or broad predicate proves the changed value is asserted. |
| `side_effect` | The changed behavior affects a supported event, state, persistence, mock, log, or outbound call sink without an observer. | A supported observer checks the changed side effect. | The effect is internal-only or non-observable in documented scope. | Sink identity, mock shape, dynamic dispatch, or effect target is unsupported. | A call mention or mock name alone proves side-effect observation. |
| `presentation_text` | User-visible output text lacks a snapshot, help-output, report, table, or golden observer. | A supported observer checks the rendered output text. | The label is internal-only in documented scope. | Visibility or observer topology cannot be traced safely. | Text alone creates user test debt or mutation testing is the first repair. |
| `config_or_policy_constant` | A changed config or policy constant flows to a supported user-observable behavior without a discriminator. | A related test observes the behavior selected by the constant. | The constant only drives internal defaults, proof labels, or non-rendered metadata. | Cross-file formatting, macro expansion, generated config, or opaque lookup hides the behavior path. | Every changed constant is user-visible behavior or test debt. |
| `static_limitation_only` | Never directly user-actionable without a stronger evidence class. | Not applicable. | Not applicable. | Static evidence is blocked by a named analyzer limitation. | Static limitations should be counted as user repair work. |

New classes may start as report-only. A class becomes projectable only after it
has fixture-backed grouping, anchor, actionability, limitation, and
must-not-claim coverage.

### Grouping

Multiple raw findings may map to one canonical item when they describe the same
missing behavioral proof or no-action/limitation state. Grouping must be
deterministic and conservative:

- the canonical ID is the user-facing grouping key;
- raw group size is reported;
- group reason is explicit;
- line movement must not change identity when the semantic owner remains the
  same;
- unrelated owners, missing discriminators, or evidence classes must not
  collide.
- primary anchor movement must not create duplicate user annotations when the
  canonical item identity is unchanged.

Presentation-text declaration and literal findings should group when they
describe the same string constant:

```text
canonical_gap_id: presentation_text::APPLE_M3_AIR_DEVICE_LABELS_TEXT
raw_group_size: 2
group_reason: declaration_and_literal_same_text_constant
```

### Downstream Consumption

Downstream surfaces should render canonical items first. They may show raw
findings as evidence details, but must not infer:

- actionability from a raw `exposed` or `static_unknown` label alone;
- maturity or confidence from a raw finding count;
- user test debt from static limitations;
- visibility from string text alone;
- policy/baseline state from Lane 1 evidence fields alone.

Lane 1 owns evidence truth and static limitations. Policy lanes own baseline,
suppression, acknowledgement, and default blocking authority.

## Required Evidence

An implemented finding-to-gap alignment feature must show:

- raw findings remain available with line, expression, static class, source
  file, and probe/seam identity when available;
- canonical items carry `canonical_gap_id`, `evidence_class`, `gap_state`,
  `actionability`, `why`, `recommended_repair`, `verify_command`,
  `static_limitations`, `confidence`, and `raw_findings`;
- projectable items carry or can derive one `primary_anchor` and retain all
  supporting `raw_spans[]` or `raw_findings[]`;
- each supported evidence class documents actionable, already-observed,
  internal/no-action, limitation, and must-not-infer behavior;
- duplicate raw findings can map to one canonical item with a group reason and
  raw group size;
- static limitations are classified as limitations, not user test debt;
- already-observed and internal-only changes can produce no-action states;
- downstream consumer documentation treats canonical items as authoritative and
  raw findings as supporting evidence;
- fixture and benchmark guards cover supported classes before analyzer
  behavior changes.

## Inputs

Alignment may use:

- existing `seams[].evidence_record` data;
- canonical gap identity;
- raw findings from diff analysis, repo exposure, or annotation-producing
  reports;
- evidence class-specific records such as presentation-text visibility and
  observer data;
- related-test ranking and oracle semantics;
- static limitation category and repair-route data;
- optional calibration confidence labels;
- optional baseline or movement artifacts for policy overlays.

Missing or ambiguous inputs must keep the item in `unknown` or
`static_limitation` state instead of inventing actionability.

## Outputs

Aligned items should appear in the evidence record or a report-specific
projection as an additive object. The minimum output contract is:

```json
{
  "canonical_gap_id": "presentation_text::APPLE_M3_AIR_DEVICE_LABELS_TEXT",
  "evidence_class": "presentation_text",
  "gap_state": "static_limitation",
  "actionability": "inspect_visibility",
  "primary_anchor": {
    "file": "src/devices.rs",
    "line": 46,
    "span": null,
    "symbol": "APPLE_M3_AIR_DEVICE_LABELS_TEXT"
  },
  "raw_spans": [
    {
      "file": "src/devices.rs",
      "line": 46,
      "span": null,
      "role": "declaration"
    },
    {
      "file": "src/devices.rs",
      "line": 47,
      "span": null,
      "role": "assigned_literal"
    }
  ],
  "why": "Visibility is unknown through unsupported output tracing.",
  "recommended_repair": "Trace the constant to an output sink or confirm internal-only use.",
  "related_test": null,
  "verify_command": "cargo xtask evidence-quality-scorecard",
  "static_limitations": [
    {
      "category": "presentation_text_visibility_unknown",
      "repair_route": "trace_string_constant_to_output_or_snapshot_test"
    }
  ],
  "confidence": {
    "basis": "fixture_backed",
    "notes": []
  },
  "raw_findings": [
    {
      "file": "src/devices.rs",
      "line": 46,
      "kind": "exposed"
    },
    {
      "file": "src/devices.rs",
      "line": 47,
      "kind": "static_unknown"
    }
  ]
}
```

JSON fields must be versioned through the owning output contract when they move
from planned docs into implemented public output.

## Non-Goals

- No PR or CI annotation rendering changes.
- No inline PR comment publishing.
- No LSP or editor polish.
- No gate-policy changes or default blocking.
- No score redefinition.
- No generated tests.
- No automatic source edits.
- No provider or model calls.
- No mutation execution.
- No baseline adoption or suppression mutation.
- No hiding or deleting raw findings.
- No broad analyzer stability claim.

## Acceptance Examples

Declaration plus literal grouping:

- Given a changed `pub const APPLE_M3_AIR_DEVICE_LABELS_TEXT: &str =` line and
  its assigned string literal line, RIPR preserves both raw findings.
- RIPR emits one canonical item with
  `canonical_gap_id = "presentation_text::APPLE_M3_AIR_DEVICE_LABELS_TEXT"`.
- `raw_findings` contains both source lines.
- `primary_anchor` points at the declaration line.
- `raw_spans` contains both the declaration and assigned literal line.
- `group_reason = "declaration_and_literal_same_text_constant"`.
- Downstream surfaces have one item to render.

Unrelated constants:

- Given two changed string constants with different owners, RIPR emits separate
  canonical items.
- The canonical IDs do not collide even when the literal text is similar.

Line movement:

- Given the same changed constant after unrelated lines are inserted above it,
  the canonical ID remains stable when the owner and evidence class remain the
  same.

User-visible unobserved presentation text:

- Given text flowing to a supported help, report, table, snapshot, or golden
  output sink with no observer found, the canonical item has
  `gap_state = "actionable"`.
- Actionability is `add_output_observer`.
- The recommended repair names the most specific observer class known.
- The class-specific repair route names `repair_kind`, `target_test_type`, and
  `suggested_assertion` so downstream surfaces do not infer actionability from
  raw static classes.

User-visible observed presentation text:

- Given text flowing to a supported output sink and a supported observer test,
  the canonical item has `gap_state = "already_observed"`.
- No new RIPR repair action is recommended.

Internal-only label:

- Given text that is clearly confined to internal proof, config, or non-rendered
  use in documented scope, the canonical item has
  `gap_state = "internal_only"`.
- Actionability is `no_action`.
- RIPR does not call the change user test debt.

Visibility unknown:

- Given text that may be presentation text but flows through an opaque helper,
  macro-generated output, dynamic dispatch, or unsupported cross-file formatting,
  the canonical item has `gap_state = "static_limitation"`.
- Static limitation category is
  `presentation_text_visibility_unknown` or the most specific supported
  category.
- RIPR does not recommend mutation testing as the first action.

Policy overlay:

- Given an actionable canonical item that is known in the baseline, Lane 1 keeps
  `gap_state = "actionable"`.
- The policy or baseline surface may add `baseline_known` as an overlay without
  changing the underlying evidence state.

## Test Mapping

Follow-up implementation should include:

- benchmark cases for declaration plus literal grouping;
- benchmark cases showing unrelated constants do not group;
- line-movement identity cases;
- user-visible unobserved, user-visible observed, internal-only, and
  visibility-unknown presentation-text cases;
- evidence-record contract tests for raw findings, gap state, actionability,
  group reason, repair route, primary anchor, raw spans, and confidence fields;
- report tests proving raw findings remain supporting evidence;
- consumer-handoff docs that identify canonical items as authoritative;
- scorecard/trend tests for alignment metrics.

This spec PR does not add analyzer behavior or public output fields.

## Implementation Mapping

Planned Lane 1 slices:

- `docs/spec-finding-to-gap-alignment` defines this contract.
- `fixtures/finding-alignment-benchmark` pins raw-to-canonical examples and
  must-not-claim guards.
- `analysis/finding-alignment-evidence-fields` adds additive `raw_findings[]`,
  `canonical_item`, and nullable `presentation_text` evidence-record fields.
- `analysis/finding-alignment-primary-anchors` adds explicit `primary_anchor`
  and `raw_spans[]` fields for projectable canonical items.
- `analysis/presentation-text-canonical-grouping` applies the contract to
  declaration plus literal grouping. The first implemented projection groups
  supported changed `&str` presentation constants and adjacent literal raw
  findings into one `finding_alignment.items[]` canonical limitation item in
  `ripr check --json`, preserving the original `findings[]` array.
- `analysis/presentation-text-visibility-observers` supplies class-specific
  state and actionability. The first implementation distinguishes
  fixture-backed visible unobserved help/report text, visible observed
  golden/snapshot-backed text, internal-only labels, and visibility-unknown
  limitations without treating text alone as user test debt.
- `analysis/finding-alignment-repair-routes` adds concrete repairs, target test
  types, suggested assertion shapes, and verify commands. Actionable supported
  classes also expose the normalized top-level `repair_route` so downstream
  consumers do not parse class-specific fields to determine the user repair.
- `report/finding-alignment-quality` adds scorecard and trend metrics.
- `docs/canonical-gap-action-consumer-handoff` documents downstream
  consumption.

Implementation must preserve the boundary that Lane 1 produces evidence truth
and downstream lanes render it.

## Metrics

Alignment scorecard and trend work should expose:

- `finding_alignment_raw_findings_total`;
- `finding_alignment_raw_signals_total`;
- `finding_alignment_canonical_items_total`;
- `finding_alignment_raw_to_canonical_ratio`;
- `finding_alignment_duplicate_groups_total`;
- `finding_alignment_actionable_items_total`;
- `finding_alignment_actionable_unresolved_canonical_gaps`;
- `finding_alignment_already_observed_total`;
- `finding_alignment_internal_only_total`;
- `finding_alignment_internal_no_action_total`;
- `finding_alignment_static_limitation_total`;
- `finding_alignment_unknown_total`;
- `finding_alignment_calibrated_supported_total`;
- `finding_alignment_uncalibrated_total`;
- `finding_alignment_visibility_unknown_total`;
- `finding_alignment_presentation_text_actionable_total`;
- `finding_alignment_repair_route_coverage`;
- `finding_alignment_actionable_items_without_repair_route`;
- `finding_alignment_verify_command_coverage`;
- `finding_alignment_actionable_items_without_verify_command`.
