# RIPR-SPEC-0048: Config And Policy Constant Evidence

Status: proposed

## Problem

RIPR can already align presentation-text changes into canonical items, but
changed constants outside that scope can still look like user-facing behavior
when they are only internal configuration, policy metadata, fixture labels, or
proof-lane names.

Config and policy constants need their own evidence class because the same
syntax shape can mean different things:

- a report-visible setting label may need a golden or render assertion;
- a schema key shown to users may need an output observer;
- an internal allowlist label may require no user action;
- a value passed through opaque lookup or generated formatting may be a static
  limitation, not user test debt.

Lane 1 needs a conservative contract that keeps raw findings available while
mapping constants into canonical evidence items with state, actionability,
repair route, and must-not-claim boundaries. Downstream lanes should consume
those canonical items instead of inferring actionability from `exposed` or
`static_unknown`.

## Behavior

RIPR should model changed configuration and policy constants as the
`config_or_policy_constant` evidence class when a diff changes a constant,
static item, enum-like value, or string/number literal that appears to drive
configuration, policy selection, report/schema labels, allowlists, thresholds,
or feature/default metadata.

The class is advisory and confidence-scoped. A changed constant alone must not
create user test debt. RIPR may create an actionable config/policy gap only
when it can identify a supported user-observable behavior path and no observer
or discriminator for that behavior exists.

### Constant Role

Each config/policy record should classify the constant role as:

| Value | Meaning |
| --- | --- |
| `rendered_config_label` | The constant labels config or settings output visible to users. |
| `rendered_policy_label` | The constant labels policy/report output visible to users. |
| `schema_field_label` | The constant names a schema or field that appears in rendered output. |
| `behavior_selector` | The constant selects behavior, validation, routing, or thresholds. |
| `internal_policy_metadata` | The constant is used only for internal policy, proof, fixture, or allowlist metadata. |
| `unknown` | RIPR cannot safely classify the role. |

Role classification must be based on supported local evidence, not names alone.
Names can be weak hints for fixture-backed cases, but they do not prove
visibility, observation, or user debt.

### Visibility

Each config/policy record must classify visibility as:

| Value | Meaning |
| --- | --- |
| `user_visible` | The constant flows to supported user-visible output or behavior. |
| `internal_only` | The constant appears confined to internal metadata or non-rendered policy state. |
| `unknown` | RIPR cannot statically trace the constant to or away from user-visible behavior. |

Supported user-visible paths are conservative and fixture-backed:

- rendered config or settings output;
- rendered report, markdown, or table output;
- schema or field labels shown in output;
- validation or routing behavior with an observable discriminator;
- snapshot, golden, or renderer tests that observe the output or selected
  behavior.

Unsupported paths remain static limitations:

- cross-file indirect formatting without a clear sink;
- macro-generated or generated config output;
- dynamic dispatch or registry lookup;
- opaque helper or table lookup;
- generated schema emission without fixture-backed tracing.

### Observer Or Discriminator Shape

Visible records should identify the strongest observer or discriminator shape
available:

| Value | Meaning |
| --- | --- |
| `snapshot` | A snapshot test observes rendered output containing the constant. |
| `golden` | A golden fixture observes rendered output containing the constant. |
| `report_render` | A report-render test observes the label or value. |
| `schema_render` | A schema/render test observes the field or label. |
| `config_output` | A config/settings output test observes the rendered value. |
| `validation_behavior` | A behavior test observes the validation or routing selected by the constant. |
| `none` | A visible path exists and no observer/discriminator is found. |
| `unknown` | Visibility or topology prevents reliable observer discovery. |

Observer discovery must not overclaim from lexical mention alone. A test that
mentions a constant name without observing output or behavior is not enough to
mark the canonical item observed.

### Gap State And Actionability

Config/policy records map to the shared alignment vocabulary:

| State | Actionability | Meaning |
| --- | --- | --- |
| `actionable` | `add_output_observer` | A visible label/value lacks snapshot, golden, report, schema, or config-output observation. |
| `actionable` | `add_behavior_discriminator` | A visible behavior selector lacks a test discriminator for the selected behavior. |
| `already_observed` | `already_observed` | A supported observer or discriminator already covers the changed output or behavior. |
| `internal_only` | `no_action_internal` | The constant is internal metadata in documented scope. |
| `static_limitation` | `inspect_config_flow` | RIPR cannot trace visibility, output, or behavior topology. |
| `unknown` | `unknown` | The class does not yet provide a sharper state. |

Static limitations are analyzer backlog, not user test debt. Actionable
config/policy items must name the repair kind, target test type or assertion
shape, and verify command when known. Supported actionable items also carry the
normalized top-level `repair_route` object so downstream surfaces consume one
canonical repair contract instead of inferring actionability from raw static
classes or class-specific fields.

### Canonical Grouping

RIPR should group raw findings that describe the same changed constant into one
canonical evidence item:

```text
canonical_gap_id: config_or_policy_constant::<stable-owner>
raw_group_size: <number of contributing raw findings>
group_reason: declaration_and_literal_same_text_constant | same_config_policy_constant | constant_owner_identity
```

Supported grouping includes:

- constant declaration plus assigned literal;
- same-line duplicate raw findings for one constant;
- adjacent declaration/value findings for the same owner;
- line movement where the semantic owner and evidence class remain stable.

Grouping must not merge different constants, different roles, different
behavior selectors, or different evidence classes merely because they share a
literal value or nearby source lines.

### Static Limitation Categories

Config/policy unknowns should use normalized static limitation categories:

| Category | Repair route |
| --- | --- |
| `config_policy_flow_unknown` | Trace the constant to an output, schema, validation, or behavior sink. |
| `config_policy_observer_unknown` | Trace the visible path to snapshot, golden, report, schema, config-output, or behavior tests. |
| `config_policy_internal_only` | Confirm the constant is internal metadata and should not be projected as user debt. |
| `macro_generated_config_output` | Add fixture-backed support for generated config/schema/report output. |
| `opaque_config_lookup` | Add analyzer support or fixtures for the lookup/helper shape. |
| `dynamic_config_dispatch` | Add runtime fixture or static support for the dispatch path before stronger claims. |

These categories are Lane 1 maintenance signals. They should not be rendered as
user failures unless a supported evidence class later turns them into
actionable canonical gaps.

### Unsupported-Flow Expansion Criteria

Unsupported-flow work must be selected from audit data, then narrowed by
fixture coverage before analyzer support changes. The next selected expansion
target is `opaque_config_lookup`.

| Flow | Current state | Expansion rule |
| --- | --- | --- |
| `opaque_config_lookup` | Selected next. | May move out of limitation only for a fixture-backed lookup shape that identifies the changed constant, the lookup owner, a supported output/schema/validation sink, and the observer or missing observer for that sink. |
| generated config or schema output | Limitation. | Remains `macro_generated_config_output` or `config_policy_flow_unknown` until generated output has a fixture-backed source-to-render path and a stable observer shape. |
| macro output | Limitation. | Remains `macro_generated_config_output` until macro expansion or a fixture-backed macro wrapper gives a stable constant owner and sink without reading generated prose as authority. |
| dynamic dispatch or registry lookup | Limitation. | Remains `dynamic_config_dispatch` unless the dispatch target is statically resolved by a fixture-backed registry shape and the selected sink is supported. |
| unsupported cross-file flow | Limitation. | Remains `config_policy_flow_unknown` unless the cross-file edge, sink owner, and observer candidate are fixture-backed and stable across file movement. |

The first implementation slice after this spec should target only the
`opaque_config_lookup` row. Generated output, macro output, dynamic dispatch,
and unsupported cross-file flow must stay named static limitations unless a
later spec or fixture PR selects one of those rows explicitly.

Before analyzer changes, the fixture and benchmark PR must add:

- a positive `opaque_config_lookup` case with a directly supported lookup shape;
- a must-not-claim `opaque_config_lookup` case where helper or registry naming
  alone is insufficient;
- limitation cases for generated config/schema output, macro output, dynamic
  dispatch, and unsupported cross-file flow;
- an internal-policy/config metadata guard proving metadata alone remains
  `internal_only` / `no_action_internal`;
- expected audit signals for `opaque_config_lookup`,
  `macro_generated_config_output`, `dynamic_config_dispatch`, and
  `config_policy_flow_unknown`.

The analyzer implementation PR must record before/after audit or scorecard
movement:

- `opaque_config_lookup` decreases only for the supported fixture-backed lookup
  shape;
- `config_policy_static_limitations` decreases only when the selected case moves
  to `already_observed`, `actionable`, or `internal_only`;
- `config_policy_repair_route_coverage` and
  `config_policy_verify_command_coverage` do not regress;
- generated, macro, dynamic-dispatch, and unsupported cross-file examples remain
  named limitations with repair routes;
- internal metadata does not become user test debt.

If those deltas cannot be shown, the implementation must keep the cases in
`static_limitation` and report the missing proof instead of widening the
supported scope.

## Required Evidence

An implemented config/policy evidence item must carry enough evidence for
downstream consumers to render one action, no-action, or limitation state:

- `evidence_class = "config_or_policy_constant"`;
- stable owner or constant name when available;
- constant role;
- source kind, such as `const_decl`, `static_item`, `enum_variant`,
  `schema_field`, or `literal`;
- visibility classification and reason;
- observer or discriminator shape when available;
- related test or observer candidate when known;
- `gap_state` and actionability;
- repair kind, target test type, assertion/output observer shape, and verify
  command when actionable and known;
- canonical identity, group size, primary anchor, and grouping reason;
- raw findings or raw spans that contributed to the item;
- named static limitation category and repair route for unknowns;
- confidence basis: `fixture_backed`, `calibrated`, `static_only`, or
  `unknown`;
- must-not-claim guard coverage in fixtures or benchmark cases.

The additive `evidence_record` shape must remain compatible with existing
consumers. If an implementation PR adds public fields, it must update
RIPR-SPEC-0021 and output schema docs in the same slice.

## Inputs

Config/policy evidence may use:

- diff hunks and changed Rust syntax facts;
- constants, statics, enum-like values, and literal spans;
- local references and owner facts;
- supported report/config/schema renderer facts;
- supported validation or routing behavior facts;
- existing related-test and oracle facts;
- existing snapshot, golden, report, schema, or config-output fixtures;
- benchmark metadata for supported internal-only and limitation cases.

Unsupported or ambiguous inputs must produce `unknown` visibility, observer, or
role states instead of stronger claims.

## Outputs

The expected evidence-record subset for an internal-only policy constant is:

```json
{
  "evidence_class": "config_or_policy_constant",
  "canonical_gap_id": "config_or_policy_constant::INTERNAL_POLICY_LABEL",
  "canonical_gap_group_size": 2,
  "canonical_gap_reason": "declaration_and_literal_same_text_constant",
  "gap_state": "internal_only",
  "actionability": "no_action_internal",
  "config_policy": {
    "constant": "INTERNAL_POLICY_LABEL",
    "role": "internal_policy_metadata",
    "source_kind": "const_decl",
    "visibility": "internal_only",
    "observer": "none",
    "repair_kind": "no_action",
    "target_test_type": "none",
    "suggested_assertion": "No user-facing assertion is recommended for this internal policy constant."
  },
  "raw_findings": [
    {
      "line": 12,
      "kind": "exposed"
    },
    {
      "line": 13,
      "kind": "static_unknown"
    }
  ]
}
```

The expected evidence-record subset for a visible unobserved config label is:

```json
{
  "evidence_class": "config_or_policy_constant",
  "canonical_gap_id": "config_or_policy_constant::REPORT_POLICY_LABEL",
  "gap_state": "actionable",
  "actionability": "add_output_observer",
  "recommended_repair": "Add or update a report, schema, config-output, snapshot, or golden observer for REPORT_POLICY_LABEL.",
  "config_policy": {
    "constant": "REPORT_POLICY_LABEL",
    "role": "rendered_policy_label",
    "visibility": "user_visible",
    "observer": "none",
    "repair_kind": "output_observer",
    "target_test_type": "report_render_or_golden"
  }
}
```

When visibility is unknown, the record should use
`config_policy_flow_unknown` or a more specific limitation category and must
not create a headline user test gap.

## Non-Goals

- No analyzer behavior changes in this spec PR.
- No fixture implementation in this spec PR.
- No PR or CI annotation rendering changes.
- No inline PR comment publishing.
- No LSP or editor polish.
- No gate-policy changes or default blocking.
- No generated tests.
- No automatic source edits.
- No provider or model calls.
- No mutation execution.
- No score redefinition or public badge redefinition.
- No broad analyzer stability claim.
- No overclaiming user visibility from constant names, helper names, macros,
  registries, generated output, or lexical test mentions.

## Acceptance Examples

Internal allowlist or policy metadata:

- Given a changed constant used only by an internal allowlist, proof-lane map,
  policy metadata table, or fixture label, RIPR emits one canonical
  `config_or_policy_constant` item.
- `gap_state = "internal_only"`.
- Actionability is `no_action_internal`.
- RIPR preserves raw findings as support and does not create user test debt.

Rendered report/config label without observer:

- Given a changed policy/config label that flows to a supported report, schema,
  config-output, table, snapshot, or golden output path with no observer, RIPR
  emits one actionable canonical item.
- Actionability is `add_output_observer`.
- The recommended repair names the most specific observer class known.

Rendered label already observed:

- Given a changed policy/config label and a supported golden, snapshot,
  report-render, schema-render, or config-output test observing it, RIPR emits
  an `already_observed` item.
- No new RIPR repair action is recommended.

Behavior selector without discriminator:

- Given a changed threshold, routing constant, or validation selector that
  reaches user-visible behavior but no related test observes the selected
  behavior, RIPR emits one actionable canonical item.
- Actionability is `add_behavior_discriminator`.
- The repair names the missing discriminator or target assertion shape when
  known.

Opaque config lookup:

- Given a changed constant whose flow passes through an unsupported helper,
  registry, macro, generated output, dynamic dispatch, or untraceable cross-file
  formatting, RIPR emits a static limitation.
- The limitation category is `config_policy_flow_unknown`,
  `opaque_config_lookup`, `macro_generated_config_output`, or
  `dynamic_config_dispatch`.
- RIPR does not infer output visibility or recommend mutation execution as the
  first action.

Unrelated constant:

- Given a changed constant that has no supported config, policy, output,
  schema, validation, or behavior-selector shape, RIPR does not create a
  config/policy user action from the constant alone.

## Test Mapping

Follow-up implementation should include:

- benchmark cases for internal policy metadata and allowlist constants;
- benchmark cases for visible report/config/schema labels without observers;
- benchmark cases for visible labels with snapshot, golden, report, schema, or
  config-output observers;
- benchmark cases for behavior selectors with and without discriminators;
- benchmark cases for opaque helpers, generated output, dynamic dispatch, and
  untraceable cross-file formatting limitations;
- grouping tests for declaration plus literal, same-line duplicates,
  line-movement identity, and non-collision across owners;
- evidence-record contract tests for role, visibility, observer,
  actionability, grouping, primary anchor, raw findings, static limitations,
  repair routes, and verify command coverage;
- scorecard and audit tests for `config_or_policy_constant` alignment coverage;
- must-not-claim tests proving policy metadata alone is not user test debt;
- dogfood receipts that record internal metadata, rendered label, behavior
  selector, observed schema or behavior, and flow-unknown examples from real
  RIPR PR work.

This spec PR does not add analyzer behavior, public output fields, or fixture
data.

## Implementation Mapping

Planned Lane 1 slices:

- `docs/spec-config-policy-constant-evidence` defines this contract.
- `fixtures/config-policy-constant-alignment-benchmark` pins positive,
  negative, limitation, and must-not-claim cases before analyzer changes.
- `analysis/config-policy-constant-fields` adds additive evidence-record fields
  only after fixtures define the expected shape.
- `analysis/config-policy-constant-grouping` groups raw findings for the same
  constant without merging unrelated owners or classes.
- `analysis/config-policy-constant-visibility` classifies supported
  user-visible, internal-only, already-observed, and static-limitation states.
- `analysis/config-policy-constant-actionability` adds concrete repair kinds,
  target test types, assertion/output observer shapes, top-level repair routes,
  and verify commands.
- `report/config-policy-alignment-quality` carries class-specific counts into
  audit, scorecard, and trend reports.
- `dogfood/finding-alignment-examples` records checked config/policy examples
  under `fixtures/finding-alignment-dogfood/` without changing downstream
  rendering, gates, scores, source edits, generated tests, providers, or
  mutation execution.

Implementation must preserve the Lane 1 boundary: evidence truth changes are in
scope, projection rendering changes are not.

## Metrics

Config/policy scorecard and trend work should expose:

- `config_policy_constant_total`;
- `config_policy_user_visible`;
- `config_policy_observed`;
- `config_policy_unobserved`;
- `config_policy_internal_only`;
- `config_policy_flow_unknown`;
- `config_policy_observer_unknown`;
- `config_policy_duplicate_groups`;
- `config_policy_actionable_output_observer`;
- `config_policy_actionable_behavior_discriminator`;
- `config_policy_no_action`;
- `config_policy_static_limitations`;
- `config_policy_repair_route_coverage`;
- `config_policy_verify_command_coverage`.
