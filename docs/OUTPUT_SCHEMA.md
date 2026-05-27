# Output Schema

`ripr` emits stable JSON for tools, CI systems, editor integrations, and coding
agents.

The current schema version is:

```text
0.1
```

Schema changes that remove fields, rename fields, or change field meanings
should bump `schema_version`.

Repository config in `ripr.toml` does not add a new field to the `check`
schema. It can change the rendered `mode` and configured `severity` values,
because those fields already describe the effective analysis mode and reporting
policy for the current run. See [Configuration](CONFIGURATION.md).

SARIF output is governed by
[RIPR-SPEC-0008](specs/RIPR-SPEC-0008-sarif-ci-policy.md). SARIF uses the
standard SARIF `version: "2.1.0"` envelope rather than `schema_version: "0.1"`.
Adding SARIF must not change the existing human, JSON, GitHub annotation,
badge, LSP, or context schemas.

## Check Output

`ripr check --json` emits:

```json
{
  "schema_version": "0.1",
  "tool": "ripr",
  "mode": "draft",
  "root": ".",
  "base": "origin/main",
  "summary": {
    "changed_rust_files": 1,
    "probes": 1,
    "findings": 1,
    "exposed": 0,
    "weakly_exposed": 1,
    "reachable_unrevealed": 0,
    "no_static_path": 0,
    "infection_unknown": 0,
    "propagation_unknown": 0,
    "static_unknown": 0
  },
  "findings": []
}
```

When supported raw findings align to a canonical evidence item, `ripr check
--json` also emits an additive `finding_alignment` section. The section is
omitted when no supported alignment item is present, so existing consumers can
continue to read `findings` directly. Raw findings remain unchanged and are
repeated only as supporting evidence for the canonical item.

```json
{
  "finding_alignment": {
    "scope": "supported_classes",
    "supported_evidence_classes": ["presentation_text", "config_or_policy_constant"],
    "summary": {
      "raw_signals": 2,
      "canonical_items": 1,
      "aligned_raw_findings": 2,
      "unaligned_raw_findings": 0,
      "raw_to_canonical_ratio": 2.0,
      "duplicate_groups_total": 1,
      "actionable_gaps": 0,
      "already_observed": 0,
      "internal_no_action": 0,
      "static_limitations": 1,
      "unknown": 0,
      "calibrated_supported": 0,
      "uncalibrated": 1,
      "repair_route_coverage": 0,
      "actionable_items_without_repair_route": 0,
      "verify_command_coverage": 0,
      "actionable_items_without_verify_command": 0,
      "presentation_text_total": 1,
      "presentation_text_user_visible": 0,
      "presentation_text_observed": 0,
      "presentation_text_unobserved": 0,
      "presentation_text_internal_only": 0,
      "presentation_text_visibility_unknown": 1,
      "presentation_text_observer_unknown": 1,
      "presentation_text_duplicate_groups": 1,
      "presentation_text_actionable_snapshot": 0,
      "presentation_text_actionable_output_repairs": 0,
      "presentation_text_no_action": 0,
      "presentation_text_static_limitations": 1,
      "config_policy_constant_total": 0,
      "config_policy_user_visible": 0,
      "config_policy_observed": 0,
      "config_policy_unobserved": 0,
      "config_policy_internal_only": 0,
      "config_policy_flow_unknown": 0,
      "config_policy_observer_unknown": 0,
      "config_policy_duplicate_groups": 0,
      "config_policy_actionable_output_observer": 0,
      "config_policy_actionable_behavior_discriminator": 0,
      "config_policy_no_action": 0,
      "config_policy_static_limitations": 0,
      "config_policy_repair_route_coverage": 0,
      "config_policy_verify_command_coverage": 0
    },
    "items": [
      {
        "canonical_gap_id": "presentation_text::APPLE_M3_AIR_DEVICE_LABELS_TEXT",
        "canonical_item_kind": "limitation",
        "evidence_class": "presentation_text",
        "gap_state": "static_limitation",
        "actionability": "inspect_visibility",
        "raw_group_size": 2,
        "group_reason": "declaration_and_literal_same_text_constant",
        "primary_anchor": {
          "file": "src/device_labels.rs",
          "line": 46,
          "kind": "exposed",
          "source_id": "probe:src_device_labels_rs:46:decl",
          "reason": "declaration_line_for_grouped_constant"
        },
        "raw_spans": [
          {
            "file": "src/device_labels.rs",
            "start_line": 46,
            "end_line": 46,
            "kind": "exposed",
            "source_id": "probe:src_device_labels_rs:46:decl"
          },
          {
            "file": "src/device_labels.rs",
            "start_line": 47,
            "end_line": 47,
            "kind": "static_unknown",
            "source_id": "probe:src_device_labels_rs:47:literal"
          }
        ],
        "why": "Changed presentation text could not be traced to or away from a user-visible output sink.",
        "recommended_repair": "Trace the string constant to a rendered output path or confirm it is internal-only.",
        "repair_route": null,
        "related_test": null,
        "verify_command": "cargo xtask evidence-quality-scorecard",
        "static_limitations": [
          {
            "category": "presentation_text_visibility_unknown",
            "repair_route": "trace_string_constant_to_output_or_snapshot_test",
            "user_actionability": "unknown_until_visibility_known"
          }
        ],
        "confidence": {
          "basis": "fixture_backed",
          "notes": [
            "Visibility-unknown presentation text is benchmark-pinned; no user test debt is claimed without an output sink."
          ]
        },
        "raw_findings": [
          {
            "file": "src/device_labels.rs",
            "line": 46,
            "kind": "exposed",
            "expression": "pub const APPLE_M3_AIR_DEVICE_LABELS_TEXT: &str =",
            "probe_kind": "field_construction",
            "source_id": "probe:src_device_labels_rs:46:decl",
            "evidence_record_ref": "probe:src_device_labels_rs:46:decl"
          },
          {
            "file": "src/device_labels.rs",
            "line": 47,
            "kind": "static_unknown",
            "expression": "\"apple-m3-air-cpu-neon = M3 MacBook Air Apple CPU/NEON lane\";",
            "probe_kind": "static_unknown",
            "source_id": "probe:src_device_labels_rs:47:literal",
            "evidence_record_ref": "probe:src_device_labels_rs:47:literal"
          }
        ],
        "presentation_text": {
          "constant_name": "APPLE_M3_AIR_DEVICE_LABELS_TEXT",
          "text_literal": "apple-m3-air-cpu-neon = M3 MacBook Air Apple CPU/NEON lane",
          "visibility": "unknown",
          "observer": "unknown",
          "actionability": "static_limitation_visibility_unknown",
          "source_kind": "const_decl",
          "canonical_group_reason": "declaration_and_literal_same_text_constant",
          "recommended_observer": "unknown",
          "repair_kind": "inspect_visibility",
          "target_test_type": "unknown",
          "suggested_assertion": "Trace the constant to a supported output sink before adding or updating tests."
        },
        "config_policy": null
      }
    ]
  }
}
```

Field contract:

- `finding_alignment.scope` - currently `supported_classes`; Lane 1 reports
  only evidence classes whose grouping behavior is fixture-backed.
- `finding_alignment.supported_evidence_classes` - evidence classes included
  in this projection. The current fixture-backed classes are
  `presentation_text` and `config_or_policy_constant`.
- `finding_alignment.summary.raw_signals` - total raw findings emitted in the
  check output.
- `finding_alignment.summary.canonical_items` - supported canonical evidence
  items after alignment.
- `finding_alignment.summary.aligned_raw_findings` and
  `unaligned_raw_findings` - how many raw findings were attached to supported
  canonical items versus left as ordinary raw findings.
- `finding_alignment.summary.raw_to_canonical_ratio` - raw signal count divided
  by supported canonical item count; this is diagnostic evidence, not a score.
- `finding_alignment.summary.duplicate_groups_total` - canonical items with
  more than one supporting raw finding.
- `finding_alignment.summary.actionable_gaps`,
  `already_observed`, `internal_no_action`, `static_limitations`, and
  `unknown` - Lane 1 evidence states. Policy states such as baseline, waiver,
  acknowledgement, suppression, or reintroduction remain outside this section.
- `finding_alignment.summary.calibrated_supported` and `uncalibrated` -
  confidence-basis counts for canonical items. Current presentation-text items
  are fixture-backed static evidence unless a later checked runtime calibration
  class supplies calibrated support.
- `finding_alignment.summary.repair_route_coverage` - count of actionable
  canonical items that carry a concrete top-level `repair_route` with
  `repair_kind`, `target_test_type`, and `suggested_assertion`.
- `finding_alignment.summary.actionable_items_without_repair_route` - count of
  actionable canonical items that are missing a concrete top-level repair
  route. Supported fixture-backed classes should keep this at zero; no-action
  and static-limitation items are not counted as missing user repair routes.
- `finding_alignment.summary.verify_command_coverage` - count of actionable
  canonical items that carry a concrete `verify_command`.
- `finding_alignment.summary.actionable_items_without_verify_command` - count
  of actionable canonical items missing a concrete verification route. Supported
  fixture-backed classes should keep this at zero; no-action and
  static-limitation items are not counted as missing user verification commands.
- `finding_alignment.summary.presentation_text_*` - presentation-text class
  counts for visibility, observer status, duplicate grouping, no-action states,
  static limitations, and output-observer repairs.
- `finding_alignment.summary.config_policy_*` - config/policy constant class
  counts for visibility, observer or discriminator status, duplicate grouping,
  no-action states, static limitations, output-observer repairs,
  behavior-discriminator repairs, repair-route coverage, and verify-command
  coverage. Config/policy repair-route coverage uses the same normalized
  top-level structured `repair_route` contract as the overall summary; prose
  `recommended_repair` or class-local repair metadata alone does not count.
  Config/policy verify-command coverage uses the same concrete-command rule as
  the overall summary; empty, `unknown`, or `verify_command_unknown` values do
  not count as covered.
- `finding_alignment.items[]` - canonical evidence items. Downstream surfaces
  should prefer these items as the user-facing unit and show raw findings as
  supporting evidence.
- `finding_alignment.items[].canonical_gap_id` - stable class-scoped grouping
  key. For presentation text constants this is currently
  `presentation_text::<CONSTANT_NAME>`, so line movement does not change the
  identity.
- `finding_alignment.items[].gap_state` - one of `actionable`,
  `already_observed`, `internal_only`, `static_limitation`, or `unknown`.
- `finding_alignment.items[].actionability` - class-scoped action label such
  as `inspect_visibility`, `add_output_observer`, `already_observed`, or
  `no_action`. Presentation text does not produce user repair work from text
  alone.
- `finding_alignment.items[].primary_anchor` - nullable preferred placement
  hint for downstream surfaces that need one inline location. Supported
  declaration-backed items point at the declaration or owner line and include
  the source ID plus a placement reason.
- `finding_alignment.items[].raw_spans[]` - source-span summary for every raw
  finding attached to the canonical item. These spans preserve line-local
  evidence for expansion/detail views; they do not become separate user
  actions.
- `finding_alignment.items[].repair_route` - nullable normalized repair route
  copied from class-specific evidence. It is required for
  `gap_state = "actionable"` in supported classes and is `null` for
  already-observed, internal-only, and static-limitation items.
- `finding_alignment.items[].static_limitations[]` - analyzer limitation
  categories and repair routes. `presentation_text_visibility_unknown` means
  RIPR could not safely trace the text to or away from a user-visible output
  sink.
- `finding_alignment.items[].presentation_text` - class-specific visibility,
  observer, actionability, source-kind, grouping reason, recommended observer,
  repair kind, target test type, and suggested assertion context. Implemented
  fixture-backed states include visibility unknown, user-visible unobserved
  help/report text, user-visible observed report text, and internal-only
  labels.
- `finding_alignment.items[].config_policy` - class-specific constant role,
  source-kind, visibility, observer/discriminator, actionability, repair kind,
  target test type, and suggested assertion context. Implemented
  fixture-backed states include internal-only policy metadata, visible
  unobserved report/config labels, observed schema labels, cross-file flow
  unknown limitations, and opaque lookup limitations.

## Finding

A finding contains:

```json
{
  "id": "probe:src_lib.rs:88:predicate",
  "classification": "weakly_exposed",
  "severity": "warning",
  "confidence": 0.92,
  "probe": {
    "id": "probe:src_lib.rs:88:predicate",
    "family": "predicate",
    "delta": "control",
    "file": "src/lib.rs",
    "line": 88,
    "expression": "if amount >= discount_threshold {"
  },
  "ripr": {
    "reach": {
      "state": "yes",
      "confidence": "medium",
      "summary": "Related tests appear to reach price: premium_customer_gets_discount"
    },
    "infect": {
      "state": "weak",
      "confidence": "medium",
      "summary": "Tests have literals, but no detected value matches changed boundary"
    },
    "propagate": {
      "state": "yes",
      "confidence": "medium",
      "summary": "Changed behavior can propagate through a return boundary"
    },
    "observe": {
      "state": "yes",
      "confidence": "medium",
      "summary": "A related test observes a value near the changed behavior"
    },
    "discriminate": {
      "state": "weak",
      "confidence": "high",
      "summary": "Only weak or smoke oracle found"
    }
  },
  "evidence_path": [
    "reach yes: Related tests appear to reach price: premium_customer_gets_discount",
    "propagation yes: Changed behavior appears to influence returned value: amount - discount",
    "related test tests/pricing.rs:12 premium_customer_gets_discount uses strong exact value oracle: assert_eq!(total, 90)",
    "observed function argument value amount = 100 at line 12",
    "missing discriminator amount == discount_threshold: No related test call uses the boundary value"
  ],
  "flow_sinks": [
    {
      "kind": "return_value",
      "text": "amount - discount",
      "line": 89
    }
  ],
  "evidence": [],
  "missing": [],
  "activation": {
    "observed_values": [
      {
        "line": 12,
        "text": "assert_eq!(discounted_total(50, 100), 50);",
        "value": "amount = 50",
        "context": "function_argument"
      }
    ],
    "missing_discriminators": [
      {
        "value": "amount == discount_threshold",
        "reason": "No related test call uses amount equal to discount_threshold",
        "flow_sink": {
          "kind": "return_value",
          "text": "amount - 10",
          "line": 89
        }
      }
    ]
  },
  "observed_values": [
    {
      "line": 12,
      "text": "assert_eq!(discounted_total(50, 100), 50);",
      "value": "amount = 50",
      "context": "function_argument"
    }
  ],
  "missing_discriminators": [
    {
      "value": "amount == discount_threshold",
      "reason": "No related test call uses amount equal to discount_threshold",
      "flow_sink": {
        "kind": "return_value",
        "text": "amount - 10",
        "line": 89
      }
    }
  ],
  "related_tests": [
    {
      "name": "premium_customer_gets_discount",
      "file": "tests/pricing.rs",
      "line": 12,
      "oracle_strength": "strong",
      "oracle_kind": "exact_value",
      "oracle": "assert_eq!(total, 90)"
    }
  ],
  "stop_reasons": [],
  "oracle_kind": "exact_value",
  "oracle_strength": "strong",
  "recommended_next_step": "Add boundary tests with exact assertions.",
  "suggested_next_action": "Add boundary tests with exact assertions.",
  "language": "rust"
}
```

The evidence-first fields are additive in schema `0.1`:

- `evidence_path` is an ordered, human-readable summary of reachability,
  infection, propagation, observation, discrimination, local flow, related test
  oracles, observed values, and missing discriminator evidence.
- `flow_sinks`, `observed_values`, and `missing_discriminators` promote the
  nested activation evidence for consumers that want direct finding-level
  access.
- `oracle_kind` and `oracle_strength` summarize the strongest related oracle
  currently visible to the finding.
- `suggested_next_action` mirrors `recommended_next_step` for action-oriented
  integrations.
- `language` is the per-finding source language reported by the language
  adapter that produced it (see [RIPR-SPEC-0026](specs/RIPR-SPEC-0026-language-adapter-contract.md)).
  Values are `rust`, `typescript`, or `python`. Omitted when no adapter
  populated it. Rust findings always carry `language: "rust"`; TypeScript
  and Python preview adapters land in later Campaign 27 work items.
- `language_status` is the per-finding adapter status. Values are `stable`
  or `preview`. **Omitted for Rust** per RIPR-SPEC-0026; preview adapters
  (TypeScript, Python) will set `preview` when they land.
- `owner_kind` is an additive optional per-finding syntactic owner
  discriminator. It is omitted when no preview adapter populated a bounded
  owner. Values are `function`, `method`, `class_method`, `arrow_function`,
  `component`, or `module_function`.
- `static_limit_kind` is an additive optional per-finding static limitation
  discriminator. It is omitted when no structured static limit is known. Values
  are `dynamic_dispatch`, `metaprogramming`, `missing_import_graph`,
  `decorator_indirection`, `mocked_module`, or `unsupported_syntax`.

## Enums

`classification` values:

- `exposed`
- `weakly_exposed`
- `reachable_unrevealed`
- `no_static_path`
- `infection_unknown`
- `propagation_unknown`
- `static_unknown`

`severity` values:

- `info`
- `warning`
- `note`

`family` values:

- `predicate`
- `return_value`
- `error_path`
- `call_deletion`
- `field_construction`
- `side_effect`
- `match_arm`
- `static_unknown`

`delta` values:

- `value`
- `control`
- `effect`
- `unknown`

`static_limit_kind` values:

- `dynamic_dispatch`
- `metaprogramming`
- `missing_import_graph`
- `decorator_indirection`
- `mocked_module`
- `unsupported_syntax`

Reserved `flow_sink` values:

- `return_value`
- `error_variant`
- `struct_field`
- `event_call`
- `state_write`
- `persistence`
- `log_message`
- `config_change`
- `call_effect`
- `match_arm`
- `unknown`

These labels are internal analysis terms in schema `0.1`. The side-effect
families are additive refinements of the older generic `call_effect` sink:
event or outbound calls, state writes, persistence writes, log messages, and
configuration changes are named when syntax-first analysis can identify them,
while `call_effect` remains the fallback for other observable calls.

`state` values:

- `yes`
- `weak`
- `no`
- `unknown`
- `opaque`
- `not_applicable`

`confidence` values inside RIPR stages:

- `high`
- `medium`
- `low`
- `unknown`

`oracle_strength` values:

- `strong`
- `medium`
- `weak`
- `smoke`
- `none`
- `unknown`

`oracle_kind` values:

- `exact_value`
- `exact_error_variant`
- `whole_object_equality`
- `snapshot`
- `relational_check`
- `broad_error`
- `smoke_only`
- `mock_expectation`
- `unknown`

`value_context` values:

- `function_argument`
- `assertion_argument`
- `builder_method`
- `table_row`
- `enum_variant`
- `return_value`
- `unknown`

`stop_reason` values:

- `max_depth_reached`
- `external_crate_boundary`
- `dynamic_dispatch_unresolved`
- `proc_macro_opaque`
- `fixture_opaque`
- `feature_unknown`
- `async_boundary_opaque`
- `no_changed_rust_line`
- `infection_evidence_unknown`
- `propagation_evidence_unknown`
- `static_probe_unknown`

## Badge Output

Badge-native JSON is a separate output contract from `ripr check --json`.
It is consumed by CI artifacts, public Shields endpoint generation, and
badge policy tooling. The Shields projection is always exactly four fields;
the native shape carries the stable metadata consumers need to understand
scope and count basis.

Formats:

```bash
ripr check --format badge-json
ripr check --format badge-plus-json
ripr check --format repo-badge-json
ripr check --format repo-badge-plus-json
ripr check --format repo-badge-json --gap-ledger target/ripr/reports/gap-decision-ledger.json
```

Native schema `0.5`:

```json
{
  "schema_version": "0.5",
  "kind": "ripr",
  "scope": "repo",
  "basis": "canonical_actionable_gap",
  "label": "ripr",
  "message": "0",
  "status": "pass",
  "color": "brightgreen",
  "counts": {
    "unsuppressed_exposure_gaps": 0,
    "unsuppressed_test_efficiency_findings": 0,
    "intentional_test_efficiency_findings": 0,
    "suppressed_exposure_gaps": 0,
    "suppressed_test_efficiency_findings": 0,
    "unknowns": 0,
    "unknowns_test_efficiency": 0,
    "analyzed_findings": 0,
    "analyzed_seams": 120,
    "analyzed_gap_records": 0,
    "analyzed_tests": 0
  },
  "reason_counts": {
    "no_assertion_detected": 0,
    "smoke_oracle_only": 0,
    "relational_oracle": 0,
    "broad_oracle": 0,
    "assertion_may_not_match_detected_owner": 0,
    "opaque_helper_or_fixture_boundary": 0,
    "no_activation_literal_detected": 0,
    "expected_value_computed_from_detected_owner_path": 0,
    "duplicate_activation_and_oracle_shape": 0
  },
  "policy": {
    "include_unknowns": false,
    "fail_on_nonzero": false,
    "test_intent_path": ".ripr/test_intent.toml",
    "suppressions_path": ".ripr/suppressions.toml"
  },
  "warnings": []
}
```

Field contract:

- `schema_version` — currently `"0.5"`. `0.2` added `scope`; `0.3` adds
  `basis` and `counts.analyzed_seams`; `0.4` adds
  `basis = "gap_decision_ledger"` and `counts.analyzed_gap_records`;
  `0.5` adds `basis = "canonical_actionable_gap"` for public repair-item
  projection.
- `kind` — `"ripr"` or `"ripr_plus"`.
- `scope` — `"diff"` for PR/diff artifacts, `"repo"` for public repo
  baseline artifacts.
- `basis` — `"finding_exposure"` for legacy Finding/ExposureClass count
  artifacts, `"canonical_actionable_gap"` for public repo repair-item badge
  projection, `"seam_native"` for internal RepoSeam/SeamGripClass inventory
  artifacts, or `"gap_decision_ledger"` when repo badge formats are explicitly
  rendered from supplied GapRecord projection targets. Diff-scoped badge
  formats currently use `finding_exposure`; repo-scoped public badge formats
  use `canonical_actionable_gap` unless `--gap-ledger` is supplied.
- `message` — the headline count rendered as a string for Shields
  compatibility. It is a count, never a denominator or coverage fraction.
- `counts.unsuppressed_exposure_gaps` — diff scope: unsuppressed
  `weakly_exposed`, `reachable_unrevealed`, and `no_static_path` Findings;
  repo public scope: unresolved actionable canonical repair items; seam-native
  inventory scope: configured-visible headline-eligible seam classes.
- `counts.unknowns` — diff scope: static unknown Finding classes; seam-native
  inventory scope: configured-visible `opaque` seams. Canonical-actionable
  public badge projection does not count unknown-only or limitation-only states
  in the headline.
- `counts.analyzed_findings` — number of Findings considered by the
  finding-exposure basis; `0` for canonical-actionable and seam-native repo
  badges.
- `counts.analyzed_seams` — number of classified seams considered by the
  canonical-actionable or seam-native repo basis; `0` for finding-exposure diff
  badges.
- `counts.analyzed_gap_records` — number of GapRecord entries considered by
  the gap-decision-ledger basis, or canonical repair groups considered by the
  canonical-actionable basis; `0` for finding-exposure and seam-native badges.
- `warnings` — advisory suppressions/config warnings that remain visible in
  native JSON. The Shields projection never includes warnings.

Shields projection:

```json
{
  "schemaVersion": 1,
  "label": "ripr",
  "message": "0",
  "color": "brightgreen"
}
```

The Shields projection drops native-only fields including `schema_version`,
`kind`, `scope`, `basis`, `status`, `counts`, `reason_counts`, `policy`, and
`warnings`.

### Badge-Basis Audit Report

`cargo xtask badge-basis` writes an advisory audit report at:

```text
target/ripr/reports/badge-basis.json
target/ripr/reports/badge-basis.md
```

This report decomposes committed public endpoint values and proves whether the
public badge basis matches RIPR-SPEC-0056. It does not edit `badges/*.json`.

Required JSON shape:

```json
{
  "schema_version": "0.1",
  "status": "pass",
  "mode": "advisory",
  "current_public_endpoints": [
    {
      "path": "badges/ripr.json",
      "label": "ripr",
      "message": "179",
      "color": "orange"
    }
  ],
  "current_repo_badges": [
    {
      "kind": "ripr",
      "scope": "repo",
      "basis": "canonical_actionable_gap",
      "message": "179",
      "counts": {}
    }
  ],
  "seam_native": {
    "status": "pass",
    "source": "ripr check --root . --format repo-exposure-md",
    "counts_by_class": {}
  },
  "test_efficiency": {
    "status": "pass",
    "source": "target/ripr/reports/test-efficiency.json",
    "counts_by_class": {}
  },
  "canonical_actionable_gap": {
    "status": "available",
    "source": "repo-badge-artifacts",
    "ripr_count": 179,
    "ripr_plus_count": 179
  },
  "supporting_signals": {
    "raw_alignment_signals": { "status": "not_in_current_badge_generator" },
    "canonical_evidence_items": { "status": "not_in_current_badge_generator" },
    "static_limitations": { "status": "available" },
    "suppressed_or_intentional_items": { "status": "available_from_badge_counts" },
    "no_action_items": { "status": "requires_gap_decision_ledger" }
  },
  "recommended_public_projection": {
    "basis": "canonical_actionable_gap",
    "rule": "README/store badges should count unresolved actionable static repair gaps using canonical_actionable_gap; ripr+ adds only items projected into the same repair, verify, and receipt model; seam-native inventory stays supporting/internal."
  },
  "warnings": [],
  "non_claims": []
}
```

Field contract:

- `current_public_endpoints` mirrors committed Shields endpoint JSON.
- `current_repo_badges` records the native badge basis used to derive public
  counts.
- `canonical_actionable_gap` records the public repair projection counts: the
  unresolved actionable static repair gaps used for README/store badge
  headlines.
- `seam_native` records internal inventory status and per-class counts when
  collected with `--include-seam-classes`.
- `test_efficiency` records class counts but does not move the public `ripr+`
  headline unless those items are projected into the repair / verify / receipt
  model.
- `supporting_signals` names supporting or excluded evidence rather than
  silently dropping it.
- `recommended_public_projection.basis` must be
  `canonical_actionable_gap` for public repair badges, and
  `recommended_public_projection.rule` must keep seam-native inventory
  supporting/internal rather than a public headline counter.

## SARIF Output

Campaign 5B SARIF formats:

```bash
ripr check --format sarif
ripr check --format repo-sarif
```

`sarif` is the diff-scoped Finding SARIF surface. `repo-sarif` is the
repo-scoped classified seam SARIF surface. Both use SARIF 2.1.0:

```json
{
  "$schema": "https://json.schemastore.org/sarif-2.1.0.json",
  "version": "2.1.0",
  "runs": [
    {
      "tool": {
        "driver": {
          "name": "ripr",
          "rules": []
        }
      },
      "results": []
    }
  ]
}
```

Rule IDs are stable public integration strings.

Finding rule IDs:

- `ripr.finding.exposed`
- `ripr.finding.weakly_exposed`
- `ripr.finding.reachable_unrevealed`
- `ripr.finding.no_static_path`
- `ripr.finding.infection_unknown`
- `ripr.finding.propagation_unknown`
- `ripr.finding.static_unknown`

Seam rule IDs:

- `ripr.seam.strongly_gripped`
- `ripr.seam.weakly_gripped`
- `ripr.seam.ungripped`
- `ripr.seam.reachable_unrevealed`
- `ripr.seam.activation_unknown`
- `ripr.seam.propagation_unknown`
- `ripr.seam.observation_unknown`
- `ripr.seam.discrimination_unknown`
- `ripr.seam.opaque`
- `ripr.seam.intentional`
- `ripr.seam.suppressed`

Configured severity maps into SARIF as:

| `ripr.toml` severity | SARIF result behavior |
| --- | --- |
| `warning` | emit `level: "warning"` |
| `info` | emit `level: "note"` |
| `note` | emit `level: "note"` |
| `off` | omit the result |

SARIF v1 does not emit `level: "error"`. CI blocking is a separate opt-in
policy decision, not a property of the static SARIF renderer.

Every result carries:

- `ruleId`;
- `level`;
- a primary physical location when file and line are known;
- `partialFingerprints.riprFingerprintV1`;
- `properties.kind` (`finding` or `seam`);
- stable IDs (`finding_id`, `probe_id`, or `seam_id`) when available;
- class metadata (`classification`, `probe_family`, `grip_class`, or
  `seam_kind`) when available.

Suppressed exposure-gap Findings remain visible with SARIF suppression metadata
when their configured severity is visible. Results whose configured severity is
`off` are omitted. See RIPR-SPEC-0008 for the full suppression and baseline
policy contract.

`cargo xtask sarif-policy` compares current SARIF against an optional baseline
and writes:

```text
target/ripr/reports/sarif-policy.json
target/ripr/reports/sarif-policy.md
```

The JSON report is repo automation output with schema version `"0.1"`:

```json
{
  "schema_version": "0.1",
  "tool": "ripr",
  "status": "new_results",
  "mode": "baseline-check",
  "threshold": "warning",
  "current": {
    "path": "target/ripr/reports/ripr-seams.sarif.json",
    "results_total": 12,
    "compared_results": 3
  },
  "baseline": {
    "path": ".ripr/sarif-baseline.json",
    "missing": false,
    "results_total": 10,
    "compared_results": 2
  },
  "new_results_total": 1,
  "new_results": [
    {
      "rule_id": "ripr.seam.weakly_gripped",
      "level": "warning",
      "fingerprint": "ripr.seam.weakly_gripped|abc123|src/lib.rs|42",
      "uri": "src/lib.rs",
      "line": 42,
      "message": "weakly_gripped seam grip for predicate_boundary"
    }
  ]
}
```

Policy reports are advisory unless `--mode fail-on-new-warning` is used.

## Context Packet

`ripr context --json` emits compact test intent for agents:

```json
{
  "version": "1.0",
  "tool": "ripr",
  "probe": {
    "id": "probe:src_lib.rs:88:predicate",
    "family": "predicate",
    "delta": "control",
    "file": "src/lib.rs",
    "line": 88,
    "changed_expression": "if amount >= discount_threshold {"
  },
  "ripr": {
    "reach": "yes",
    "infect": "weak",
    "propagate": "yes",
    "observe": "yes",
    "discriminate": "weak"
  },
  "related_tests": [],
  "observed_values": [],
  "missing_discriminators": [],
  "missing": [],
  "stop_reasons": [],
  "recommended_next_step": "Add below, equal, and above threshold tests."
}
```

The context packet is intentionally smaller than check output. It is optimized
for coding agents and editor commands.

## Repo Seam Inventory

`ripr check --root . --format repo-seams-json` emits the repo seam inventory
introduced by RIPR-SPEC-0005. The artifact lands at
`target/ripr/reports/repo-seams.json` when generated via
`cargo xtask repo-seam-inventory`.

```json
{
  "schema_version": "0.1",
  "scope": "repo",
  "seams": [
    {
      "seam_id": "f3c9e4d21a0b7c88",
      "kind": "predicate_boundary",
      "file": "src/pricing.rs",
      "line": 88,
      "owner": "src/pricing.rs::discounted_total",
      "expression": "amount >= discount_threshold",
      "required_discriminator": {
        "kind": "boundary_value",
        "description": "amount >= discount_threshold"
      },
      "expected_sink": {
        "kind": "return_value"
      }
    }
  ]
}
```

Field contract:

- `schema_version` — currently `"0.1"`. Bumping requires updating this section,
  the renderer (`crates/ripr/src/output/repo_seams.rs`), and any downstream
  consumers in lockstep.
- `scope` — always `"repo"` for this artifact. Distinguishes the repo seam
  inventory from diff-scoped findings.
- `seam_id` — 16-char lowercase hex. FNV-1a 64-bit hash of
  `file | owner | kind | byte_offset` (null-byte separators). Stable across
  runs and file walk reorderings.
- `kind` — one of `predicate_boundary`, `error_variant`, `return_value`,
  `field_construction`, `side_effect`, `match_arm`, `call_presence`. The spec
  also reserves `validation_branch` for a future detection PR.
- `file` — repo-root-relative Unix-separator path (no leading `./`).
- `line` — 1-based start line for human display only. Not part of the seam ID
  hash; `byte_offset` is the canonical position field internally.
- `owner` — fully-qualified module/symbol path of the enclosing function.
  Backslashes from native paths are normalized to forward slashes before
  hashing. Test functions (e.g., `#[test] fn` inside `#[cfg(test)] mod tests`)
  are excluded.
- `expression` — verbatim source-code text at the seam origin. Surfaced for
  human review; not part of the seam ID hash.
- `required_discriminator.kind` — `boundary_value`, `error_variant`,
  `return_value`, `field_value`, `effect`, `match_arm_taken`, or `call_site`.
- `required_discriminator.description` — human-readable summary of what a test
  must observe to grip the seam.
- `expected_sink.kind` — `return_value`, `output_field`, `error_channel`, or
  `side_effect`. The spec's `unknown` sink will return when an undetermined
  kind is detected.

The repo seam inventory v1 captures every probeable production syntax shape
and does not yet classify test grip. When the repository root is analyzed,
repository automation and fixture data (`xtask/`, top-level `fixtures/`) are
excluded so repo-scoped public signals represent the published `ripr` package
surface; passing an individual fixture workspace as `--root` still analyzes
that fixture normally. `analysis/repo-ripr-classification-v1` adds
`SeamGripClass` and the headline-eligibility table per RIPR-SPEC-0005.
Static output continues to forbid runtime-mutation outcome words.

The Markdown sibling (`repo-seams.md`, generated alongside the JSON) is
human-readable but follows the same contract for `kind`, `owner`, and
`expected_sink` strings.

## Repo Exposure Report

`ripr check --root . --format repo-exposure-json` emits the classified seam
inventory introduced by `analysis/repo-ripr-classification-v1`. The artifact
lands at `target/ripr/reports/repo-exposure.json` when generated via
`cargo xtask repo-exposure-report`.

```json
{
  "schema_version": "0.3",
  "scope": "repo",
  "metrics": {
    "seams_total": 9355,
    "headline_eligible": 6114,
    "strongly_gripped": 3241,
    "weakly_gripped": 1756,
    "ungripped": 0,
    "reachable_unrevealed": 2,
    "activation_unknown": 4356,
    "propagation_unknown": 0,
    "observation_unknown": 0,
    "discrimination_unknown": 0,
    "opaque": 0,
    "intentional": 0,
    "suppressed": 0
  },
  "seams": [
    {
      "seam_id": "f3c9e4d21a0b7c88",
      "kind": "predicate_boundary",
      "file": "src/pricing.rs",
      "line": 88,
      "owner": "src/pricing.rs::discounted_total",
      "expression": "amount >= discount_threshold",
      "grip_class": "weakly_gripped",
      "headline_eligible": true,
      "evidence": {
        "reach": "yes",
        "activate": "yes",
        "propagate": "yes",
        "observe": "yes",
        "discriminate": "weak"
      },
      "related_tests_total": 47,
      "related_tests": [
        {
          "name": "below_threshold_has_no_discount",
          "file": "tests/pricing_tests.rs",
          "line": 12,
          "oracle_kind": "exact_value",
          "oracle_strength": "strong",
          "evidence_summary": "exact value assertion",
          "relation_reason": "direct_owner_call",
          "relation_confidence": "high"
        }
      ],
      "observed_values": ["50", "10000"],
      "missing_discriminators": [
        {
          "value": "input that hits the boundary: amount >= discount_threshold",
          "reason": "predicate uses an equality-bearing operator; tests should exercise the boundary case"
        },
        {
          "value": "discount_threshold (equality boundary)",
          "reason": "observed values do not include the equality-boundary case for this predicate"
        }
      ],
      "evidence_record": {
        "schema_version": "0.1",
        "seam_id": "f3c9e4d21a0b7c88",
        "canonical_gap_id": "gap:67fc764ba37d77bd",
        "canonical_gap_group_size": 1,
        "canonical_gap_reason": "same owner, seam kind, flow sink, missing discriminator, and assertion shape",
        "raw_findings": [
          {
            "file": "src/pricing.rs",
            "line": 88,
            "kind": "weakly_gripped",
            "expression": "amount >= discount_threshold",
            "probe_kind": "predicate_boundary",
            "source_id": "f3c9e4d21a0b7c88",
            "evidence_record_ref": "f3c9e4d21a0b7c88"
          }
        ],
        "canonical_item": {
          "canonical_gap_id": "gap:67fc764ba37d77bd",
          "raw_group_size": 1,
          "canonical_item_kind": "gap",
          "evidence_class": "predicate_boundary",
          "gap_state": "actionable",
          "actionability": "extend_related_test",
          "group_reason": "same owner, seam kind, flow sink, missing discriminator, and assertion shape",
          "primary_anchor": {
            "file": "src/pricing.rs",
            "line": 88,
            "kind": "weakly_gripped",
            "source_id": "f3c9e4d21a0b7c88",
            "reason": "canonical_group_primary_raw_finding"
          },
          "raw_spans": [
            {
              "file": "src/pricing.rs",
              "start_line": 88,
              "end_line": 88,
              "kind": "weakly_gripped",
              "source_id": "f3c9e4d21a0b7c88"
            }
          ],
          "why": "extend the nearest related test with the missing discriminator",
          "recommended_repair": "Add or strengthen `assert_eq!(discounted_total(/* boundary input where amount >= discount_threshold */), /* expected */)` for `input that hits the boundary: amount >= discount_threshold` in `tests/pricing_tests.rs` as `discounted_total_boundary_discriminator`.",
          "repair_route": {
            "repair_kind": "add_boundary_assertion",
            "target_test_type": "boundary_discriminator",
            "suggested_assertion": "assert_eq!(discounted_total(/* boundary input where amount >= discount_threshold */), /* expected */)"
          },
          "related_test": {
            "name": "below_threshold_has_no_discount",
            "file": "tests/pricing_tests.rs",
            "line": 12,
            "reason": "direct_owner_call"
          },
          "verify_command": "ripr agent verify --root . --before target/ripr/pilot/repo-exposure.json --after target/ripr/pilot/after.repo-exposure.json --json",
          "confidence": {
            "basis": "static_only",
            "notes": ["no imported runtime calibration data"]
          }
        },
        "owner": "src/pricing.rs::discounted_total",
        "location": {
          "file": "src/pricing.rs",
          "line": 88
        },
        "seam_kind": "predicate_boundary",
        "grip_class": "weakly_gripped",
        "headline_eligible": true,
        "evidence_path": {
          "reach": {
            "state": "yes",
            "confidence": "medium",
            "summary": "owner is reached"
          },
          "activate": {
            "state": "yes",
            "confidence": "medium",
            "summary": "boundary values were observed"
          },
          "propagate": {
            "state": "yes",
            "confidence": "medium",
            "summary": "changed value flows to return value"
          },
          "observe": {
            "state": "yes",
            "confidence": "medium",
            "summary": "related test observes returned value"
          },
          "discriminate": {
            "state": "weak",
            "confidence": "medium",
            "summary": "equality discriminator is missing"
          }
        },
        "observed_values": [
          {
            "value": "50",
            "line": 12,
            "text": "discounted_total(50, 100)",
            "context": "function_argument"
          }
        ],
        "missing_discriminators": [
          {
            "value": "discount_threshold (equality boundary)",
            "reason": "observed values do not include the equality-boundary case for this predicate",
            "flow_sink": null
          }
        ],
        "related_tests_total": 47,
        "related_tests": [
          {
            "name": "below_threshold_has_no_discount",
            "file": "tests/pricing_tests.rs",
            "line": 12,
            "oracle_kind": "exact_value",
            "oracle_strength": "strong",
            "evidence_summary": "exact value assertion",
            "oracle_semantics": {
              "observes": "the exact value or value pattern asserted by the test",
              "missing": "no obvious value-shape discriminator gap under static scope",
              "upgrade_suggestion": null
            },
            "relation_reason": "direct_owner_call",
            "relation_confidence": "high"
          }
        ],
        "recommendation": {
          "action": "write_targeted_test",
          "reason": "extend the nearest related test with the missing discriminator",
          "recommended_test": {
            "name": "discounted_total_boundary_discriminator",
            "file": "tests/pricing_tests.rs",
            "reason": "place the new targeted test next to the nearest strong related test"
          },
          "nearest_test_to_imitate": {
            "name": "below_threshold_has_no_discount",
            "file": "tests/pricing_tests.rs",
            "line": 12,
            "oracle_kind": "exact_value",
            "oracle_strength": "strong",
            "evidence_summary": "exact value assertion",
            "oracle_semantics": {
              "observes": "the exact value or value pattern asserted by the test",
              "missing": "no obvious value-shape discriminator gap under static scope",
              "upgrade_suggestion": null
            },
            "relation_reason": "direct_owner_call",
            "relation_confidence": "high"
          },
          "candidate_values": [
            {
              "value": "discount_threshold (equality boundary)",
              "reason": "observed values do not include the equality-boundary case for this predicate"
            }
          ],
          "assertion_shape": {
            "kind": "exact_return_value",
            "example": "assert_eq!(actual, expected)"
          },
          "verify_command": "ripr agent verify --root . --before target/ripr/pilot/repo-exposure.json --after target/ripr/pilot/after.repo-exposure.json --json"
        },
        "actionability": {
          "class": "actionable_related_test_extension",
          "reason": "extend the nearest related test with the missing discriminator",
          "has_concrete_guidance": true,
          "signals": {
            "missing_discriminator": true,
            "candidate_value": true,
            "assertion_shape": true,
            "related_test": true,
            "recommended_test_target": true,
            "verification_command": true
          }
        },
        "calibration": {
          "availability": "not_imported",
          "confidence": "unknown",
          "agreement": "no_runtime_data"
        },
        "static_limitations": [],
        "presentation_text": null
      }
    }
  ]
}
```

Field contract:

- `schema_version` — currently `"0.3"`. Bumping requires updating this
  section, the renderer (`crates/ripr/src/output/repo_exposure.rs`), and
  any downstream consumers in lockstep. `0.1` → `0.2`: per-related-test
  entries gained `relation_reason` and `relation_confidence` fields
  (`analysis/related-test-precision-v1`). `0.2` -> `0.3`: seams gained
  the additive `evidence_record` projection (`RIPR-SPEC-0021`) while
  preserving existing top-level seam fields. `relation_reason` is an
  additive string enum within `0.3`; `helper_owner_call` extends the
  existing relation taxonomy without changing the field shape.
- `scope` — always `"repo"`.
- `metrics` — totals plus a per-`SeamGripClass` count bucket. Keys mirror
  `SeamGripClass::as_str()`. The renderer emits all 11 buckets even when
  zero so consumers can plot stable bar charts.
- `metrics.headline_eligible` — count of seams whose `grip_class`
  satisfies `SeamGripClass::is_headline_eligible()` per RIPR-SPEC-0005.
- `seams[].grip_class` — one of the 11 `SeamGripClass` strings:
  `strongly_gripped`, `weakly_gripped`, `ungripped`, `reachable_unrevealed`,
  `activation_unknown`, `propagation_unknown`, `observation_unknown`,
  `discrimination_unknown`, `opaque`, `intentional`, `suppressed`.
- `seams[].evidence` — per-stage `StageState` strings: `yes`, `weak`,
  `no`, `unknown`, `opaque`, `not_applicable`.
- `seams[].related_tests_total` — number of related tests the analyzer
  matched. The `related_tests` array is **capped** for artifact size; see
  `MAX_RELATED_TESTS_PER_SEAM_JSON` in the renderer (currently 8). The
  total field always carries the unbounded count.
- `seams[].related_tests[].relation_reason` — single highest-priority
  reason this test is related to the seam. One of:
  `direct_owner_call`, `helper_owner_call`, `assertion_target_affinity`,
  `same_test_file`, `same_module`, `owner_named_test`,
  `import_path_affinity`, `fixture_owner_affinity`. Detection lives in
  `crates/ripr/src/analysis/test_grip_evidence.rs`.
  `helper_owner_call` is limited to a one-hop same-file helper that directly
  calls the owner. The helper either carries the specific owner token in its
  name or is a direct delegating wrapper that calls exactly one specific local
  owner. Generic owner names, skipped-owner wrappers, and two-hop wrapper chains
  remain non-activating static limitations.
- `seams[].related_tests[].relation_confidence` — `high`, `medium`,
  `low`, or `opaque`. Mapping from reason: `direct_owner_call`,
  `helper_owner_call`, and `assertion_target_affinity` → `high`;
  `same_test_file`,
  `same_module`, `owner_named_test`, `import_path_affinity` →
  `medium`; `fixture_owner_affinity` → `low`. Independent of
  `oracle_strength`: a `low` relation can still carry a strong oracle.
- The `related_tests` array is **ranked** by
  `(confidence, reason_priority, oracle_strength, activation_overlap, file,
  name, line)` so the highest-confidence tests appear first, then the nearest
  strong imitation target wins within otherwise equivalent relationships.
  `activation_overlap` is a static tie-breaker from already observed call
  values, such as a predicate-boundary equality call. `related_tests_total` is
  unaffected by ranking.
- `seams[].observed_values` — literal scalar values seen in owner-call
  arguments across related tests. Bare identifiers and helper-derived
  values are intentionally excluded.
- `seams[].missing_discriminators` — per-rule hypothesis strings (e.g.,
  the equality-boundary case for predicate seams). Empty when no rule
  fires.
- `seams[].evidence_record` - additive Lane 1 evidence spine for the seam.
  It is schema versioned independently from repo exposure and currently uses
  `schema_version: "0.1"`.
- `seams[].evidence_record.canonical_gap_id` - generated canonical
  behavioral gap identity for headline-eligible gap classes, or `null` for
  strong, opaque, intentional, and suppressed seams. Line numbers remain
  locators, not durable canonical identity.
- `seams[].evidence_record.canonical_gap_group_size` - number of raw seams
  in this repo-exposure snapshot that share the same canonical gap identity,
  or `null` when no canonical gap identity is assigned.
- `seams[].evidence_record.canonical_gap_reason` - grouping reason for the
  canonical identity, or `null` when no canonical gap identity is assigned.
- `seams[].evidence_record.raw_findings` - supporting raw analyzer signals
  that contributed to this record. The current seam-native projection emits
  one raw finding per seam; later class-specific grouping may attach multiple
  raw findings to one canonical item.
- `seams[].evidence_record.canonical_item` - additive finding-alignment
  projection with `gap_state`, class-scoped `actionability`, `why`,
  `recommended_repair`, nullable structured `repair_route`, `related_test`,
  `verify_command`, nullable `receipt_command`, `confidence`, raw group size,
  nullable `primary_anchor`, and `raw_spans`. Actionable canonical items carry
  `repair_route.repair_kind`, `target_test_type`, and `suggested_assertion`;
  no-action, observed, limitation, and unknown items keep `repair_route: null`.
  Actionable items also carry a safe agent receipt command when the canonical
  repair/verify loop is available, so public-projection readiness can be
  assessed from canonical evidence rather than raw findings. Downstream
  surfaces should render this canonical item before treating raw findings as
  separate work.
- `seams[].evidence_record.canonical_item.primary_anchor` - preferred
  placement hint for downstream surfaces when the canonical item has a safe
  source location. It is `null` only when RIPR cannot safely name a placement.
- `seams[].evidence_record.canonical_item.raw_spans[]` - source-span summary
  for every raw finding contributing to the canonical item. These spans are
  supporting evidence, not independent user-facing actions.
- `seams[].evidence_record.canonical_item.static_limitations[]` - canonical
  item-local copy of named analyzer limitations for static-limitation and
  unknown states. Downstream canonical-item consumers may use these category
  and repair-route rows to explain why an item is not actionable, but must not
  treat them as user test debt.
- `seams[].evidence_record.evidence_path` - typed reach, activate,
  propagate, observe, and discriminate stages. Each stage carries `state`,
  `confidence`, and `summary`.
- `seams[].evidence_record.observed_values`,
  `missing_discriminators`, and `related_tests` - structured copies of
  existing seam evidence, including related-test relation fields. The
  nested `related_tests` array is capped like the top-level array and keeps
  `related_tests_total`.
- `seams[].evidence_record.related_tests[].oracle_semantics` - structured
  oracle-shape explanation with `observes`, `missing`, and nullable
  `upgrade_suggestion`. Weak, broad, smoke-only, and unknown oracle shapes
  name the behavior they observe, the discriminator they fail to observe, and
  the assertion upgrade RIPR recommends for this seam kind.
- `seams[].evidence_record.recommendation` - bounded test-intent guidance
  derived from existing evidence: recommended test target, nearest test to
  imitate, candidate values, assertion shape, and verification command when
  the seam has concrete guidance.
- `seams[].evidence_record.actionability` - advisory classification plus
  boolean signals showing which pieces of guidance are present. It does not
  change gate or baseline policy.
- `seams[].evidence_record.calibration` - placeholder static/runtime
  confidence context. `no_runtime_data` means no imported runtime
  calibration was supplied; it does not imply runtime confirmation.
- `seams[].evidence_record.static_limitations` - unknown or opaque static
  evidence stages that should be treated as analyzer limitations rather than
  focused-test instructions. Each entry carries the original `stage`, `state`,
  and `reason` plus a normalized `category` and `repair_route` so Lane 1 can
  group analyzer limits without treating them as user test gaps.
  Predicate boundaries whose activation operand is local, iterator-derived, or
  computed use category `activation_boundary_input_unresolved` with repair
  route `analysis/local-iterator-boundary-operand-resolution`; they must not
  emit exact boundary candidate values or public repair packets.
- `seams[].evidence_record.presentation_text` - reserved presentation-text
  evidence-class projection. It is `null` until a fixture-backed presentation
  text slice classifies visibility, observer shape, and output actionability.

The fixture contract corpus at
`fixtures/boundary_gap/expected/evidence-record-contract/corpus.json` pins
representative `evidence_record` v0.1 records for predicate boundaries, exact
error variants, strong exact-value evidence, broad error assertions, field and
whole-object oracles, snapshot evidence, side-effect observers, opaque static
limitations, generated canonical gap identity, and the current
`no_runtime_data` calibration placeholder. Unit and repo-exposure tests pin the
additive `raw_findings`, `canonical_item`, and `presentation_text` alignment
fields before later presentation-text grouping changes. `cargo xtask check-fixture-contracts`
validates the required case matrix and field shape; `cargo xtask
check-output-contracts` validates the `evidence_record` schema version in code,
docs, and the corpus.

The Markdown sibling (`repo-exposure.md`) prints a metrics table plus
the top headline-eligible seams (capped at 50). Both formats are
generated together by `cargo xtask repo-exposure-report`.

This report shows static test-grip evidence for repo seams. Runtime
confirmation via `cargo-mutants` is a separate calibration step
(`calibration/cargo-mutants-v1`). Static-language constraints from
RIPR-SPEC-0005 still apply: the report never uses runtime-mutation
outcome words.

## Evidence Health Report

`ripr evidence-health --root .` summarizes Lane 1 analyzer evidence health
without changing analyzer behavior. The same report lands at
`target/ripr/reports/evidence-health.json` and
`target/ripr/reports/evidence-health.md` when generated through
`cargo xtask evidence-health`.

The xtask facade bounds both the preflight `cargo build -p ripr` phase and the
live `ripr evidence-health` subprocess with `RIPR_EVIDENCE_HEALTH_TIMEOUT_MS`
(default 4 minutes). If either phase times out, exits before a complete report
is available, or the xtask runner cannot start, capture, poll, or read the
child process, xtask discards stale or partial outputs and writes warning JSON
and Markdown with `status = "warn"`, phase context such as
`evidence_health_build` or `evidence_health_generation`, and a named
`evidence_health_timeout`, `evidence_health_incomplete`, or
`evidence_health_runner_error` `run_limitations[]` entry. Runner/capture errors
use `inputs.generation.status = "runner_error"` and the limitation category
`evidence_health_runner_error`. The default is intentionally below common
5-minute validation shells so pathological live runs can write bounded warning
artifacts instead of being killed before `evidence-health.json` / `.md` exist.
while pathological runs still produce bounded diagnostics before abnormal
termination can drop the artifact. During
generation, xtask enables repo-exposure latency tracing so timeout artifacts can
include analyzer phase breadcrumbs when available. Limited artifacts expose those
breadcrumbs as bounded `latency_trace_events_total` and `latency_trace_tail`
fields on both `inputs.generation` and `run_limitations[]`, so operators can see
which repo-exposure phase was active without scraping stderr. That limited
artifact is diagnostic only; it does not claim user test debt from missing health
counts.

```json
{
  "schema_version": "0.2",
  "tool": "ripr",
  "scope": "repo",
  "status": "advisory",
  "inputs": {
    "root": ".",
    "mutation_calibration": "target/ripr/reports/mutation-calibration.json"
  },
  "metrics": {
    "seams_total": 9355,
    "headline_eligible_total": 6114,
    "weakly_gripped_total": 1756,
    "ungripped_total": 0,
    "grip_class_counts": {
      "strongly_gripped": 3241,
      "weakly_gripped": 1756,
      "ungripped": 0,
      "reachable_unrevealed": 2,
      "activation_unknown": 4356,
      "propagation_unknown": 0,
      "observation_unknown": 0,
      "discrimination_unknown": 0,
      "opaque": 0,
      "intentional": 0,
      "suppressed": 0
    },
    "stage_state_counts": {
      "reach": {
        "yes": 4999,
        "weak": 0,
        "no": 0,
        "unknown": 4356,
        "opaque": 0,
        "not_applicable": 0
      }
    },
    "unknown_stage_counts": {
      "reach": 4356,
      "activate": 4356,
      "propagate": 0,
      "observe": 0,
      "discriminate": 0
    },
    "unknown_stop_reason_counts": {
      "activation_unknown": 4356,
      "propagation_unknown": 0,
      "observation_unknown": 0,
      "discrimination_unknown": 0,
      "opaque": 0
    },
    "missing_discriminators_total": 1756,
    "seams_with_missing_discriminators": 1756,
    "missing_discriminator_counts": [
      {"label": "amount == threshold", "count": 4}
    ],
    "observed_values_total": 740,
    "seams_with_observed_values": 310,
    "observed_value_context_counts": {
      "function_argument": 600,
      "assertion_argument": 40,
      "builder_method": 30,
      "table_row": 50,
      "enum_variant": 12,
      "return_value": 8,
      "unknown": 0
    },
    "related_tests_total": 2200,
    "seams_with_related_tests": 1720,
    "related_test_confidence_counts": {
      "high": 910,
      "medium": 1060,
      "low": 220,
      "opaque": 10
    },
    "oracle_strength_counts": {
      "strong": 800,
      "medium": 410,
      "weak": 600,
      "smoke": 300,
      "none": 80,
      "unknown": 10
    },
    "oracle_kind_counts": {
      "exact_value": 700,
      "exact_error_variant": 60,
      "whole_object_equality": 40,
      "snapshot": 120,
      "relational_check": 180,
      "broad_error": 250,
      "smoke_only": 300,
      "mock_expectation": 40,
      "unknown": 10
    },
    "opaque_oracle_count": 10
  },
  "evidence_quality": {
    "canonical_gap_groups_total": 4800,
    "duplicate_looking_groups_total": 240,
    "largest_canonical_groups": [
      {
        "canonical_gap_id": "gap:37d49d135d41fb52",
        "count": 18,
        "reported_group_size": 18,
        "owner": "crates/ripr/src/output/first_useful_action.rs::selected_from_editor_context",
        "seam_kind": "call_presence",
        "flow_sink": "n/a",
        "missing_discriminator": "n/a",
        "assertion_shape": "n/a",
        "example_seam_id": "f013a5a5798ec6c5",
        "example_file": "crates/ripr/src/output/first_useful_action.rs"
      }
    ],
    "actionability_class_counts": {
      "actionable_related_test_extension": 1200,
      "static_limitation": 3600
    },
    "static_limitation_stage_counts": {
      "activate": 3600
    },
    "static_limitation_reason_counts": [
      {
        "label": "No concrete activation values observed for seam `Vec::new()`",
        "count": 255
      }
    ],
    "static_limitation_category_counts": {
      "activation_value_unresolved": 255
    },
    "calibration_availability_counts": {
      "not_imported": 9355
    },
    "movement_availability": {
      "records_with_seam_id": 9355,
      "records_with_canonical_gap_id": 6114,
      "records_with_complete_evidence_path": 9355,
      "records_with_recommendation": 9355,
      "records_with_verify_command": 1756
    },
    "top_evidence_quality_risks": [
      {
        "kind": "static_limitations",
        "count": 3600,
        "summary": "Evidence records still contain static limitations."
      }
    ]
  },
  "calibration": {
    "status": "loaded",
    "source": "target/ripr/reports/mutation-calibration.json",
    "matched_total": 18,
    "static_without_runtime_total": 120,
    "runtime_without_static_total": 2,
    "ambiguous_file_line_total": 1,
    "unmatched_runtime_total": 2
  },
  "top_static_limitations": [
    {
      "kind": "missing_discriminator",
      "count": 1756,
      "summary": "At least one discriminator remains missing for the seam.",
      "example_seam_id": "f3c9e4d21a0b7c88"
    }
  ]
}
```

Field contract:

- `schema_version` - currently `"0.2"`. `0.2` changes free-form
  `missing_discriminator_counts` and `static_limitation_reason_counts` from
  JSON objects to `{label, count}` rows so downstream consumers do not treat
  analyzer evidence strings as stable field names.
- `scope` - always `"repo"`.
- `status` - `"advisory"` for complete analyzer-health reports and `"warn"`
  for bounded xtask fallback artifacts. This report is an analyzer-health view,
  not a gate decision.
- `inputs.root` - the analyzed workspace root as supplied to the command.
- `inputs.mutation_calibration` - optional imported calibration report path;
  `null` when not provided.
- `inputs.generation` - present on bounded xtask fallback artifacts. It records
  `phase` (`evidence_health_build` or `evidence_health_generation`), bounded
  command, `status` (`fail`, `timeout`, `pass_incomplete`, or `runner_error`),
  timeout/duration, exit code when available, output byte counts, optional
  `failure_reason`, bounded stdout/stderr excerpts,
  `latency_trace_events_total`, and `latency_trace_tail` repo-exposure phase
  diagnostics when available. Complete `ripr evidence-health` reports omit this
  wrapper field and keep the normal analyzer-health payload, so the current
  contract does not emit an `"ok"` generation status.
- `metrics.grip_class_counts` - all `SeamGripClass` buckets, including zero
  counts.
- `metrics.stage_state_counts` - per-stage `StageState` buckets for `reach`,
  `activate`, `propagate`, `observe`, and `discriminate`.
- `metrics.unknown_stage_counts` - unknown or opaque counts by evidence stage.
- `metrics.unknown_stop_reason_counts` - counts of unknown/opaque
  `SeamGripClass` buckets. This is intentionally repo-seam terminology; diff
  finding stop-reason strings are not reinterpreted here.
- `metrics.missing_discriminator_counts` - aggregate `{label, count}` rows for
  missing discriminator value text. It is row-shaped because the labels are
  analyzer evidence strings and can collide in case-insensitive JSON consumers.
- `metrics.observed_value_context_counts` - aggregate counts keyed by
  `ValueContext::as_str()`.
- `metrics.related_test_confidence_counts` - `high`, `medium`, `low`, and
  `opaque` related-test confidence buckets.
- `metrics.oracle_strength_counts` and `metrics.oracle_kind_counts` - aggregate
  oracle evidence observed on related tests.
- `evidence_quality` - audit-style fields derived from
  `seams[].evidence_record` and canonical gap identity, without changing
  classifications or policy.
- `evidence_quality.canonical_gap_groups_total` - number of distinct canonical
  gap IDs among headline-eligible evidence records.
- `evidence_quality.duplicate_looking_groups_total` - number of canonical gap
  groups with more than one raw seam.
- `evidence_quality.largest_canonical_groups` - top canonical gap groups by raw
  seam count, capped to 10 rows, including the canonical ID, reported group
  size, owner, seam kind, flow sink, discriminator, assertion shape, and
  example seam/file.
- `evidence_quality.actionability_class_counts` - counts keyed by
  `evidence_record.actionability.class`.
- `evidence_quality.static_limitation_stage_counts` and
  `static_limitation_reason_counts` - distributions from
  `evidence_record.static_limitations`. Reason counts are `{label, count}` rows
  because reasons are free-form evidence strings.
- `evidence_quality.static_limitation_category_counts` - normalized limitation
  categories such as `activation_value_unresolved`,
  `activation_owner_call_unresolved`, `opaque_helper_call`,
  `cross_file_constant_unresolved`, `dynamic_dispatch`,
  `unsupported_mock_shape`, `snapshot_field_unknown`, and
  `side_effect_sink_unknown`.
- `evidence_quality.calibration_availability_counts` - counts keyed by
  `evidence_record.calibration.availability`. These are placeholder coverage
  labels from the static record and do not imply runtime execution.
- `evidence_quality.movement_availability` - counts of records carrying seam
  IDs, canonical gap IDs, complete evidence paths, recommendations, and verify
  commands for movement-aware downstream reports.
- `evidence_quality.top_evidence_quality_risks` - largest advisory risk buckets
  for follow-up Lane 1 work. They are measurements, not gate decisions.
- `calibration` - availability counts from an already-produced mutation
  calibration report when one is supplied. The evidence-health command does not
  run mutation testing, infer thresholds, or change static classification.
- `top_static_limitations` - the largest static evidence gaps by count, capped
  to 10 rows and carrying one example seam ID for inspection.
- `run_limitations` - present on bounded xtask fallback artifacts. Timeout,
  incomplete, and runner-error rows name `evidence_health_timeout`,
  `evidence_health_incomplete`, or `evidence_health_runner_error`, the
  `evidence_health_build` or `evidence_health_generation` phase,
  timeout/duration/output byte diagnostics, bounded stdout/stderr excerpts,
  optional `failure_reason`, bounded `latency_trace_events_total` and
  `latency_trace_tail` repo-exposure phase diagnostics when available, and a
  repair route for inspecting runtime, stdout/stderr, or increasing
  `RIPR_EVIDENCE_HEALTH_TIMEOUT_MS` on slower machines. If the child exits
  successfully but the expected JSON/Markdown artifacts are missing or
  incomplete, the fallback uses
  `inputs.generation.status = "pass_incomplete"` and overwrites stale prior
artifacts. If the build or generation runner cannot start, capture, poll, or
read the child process, the fallback uses
`inputs.generation.status = "runner_error"`,
`run_limitations[].category = "evidence_health_runner_error"`, and still
overwrites stale prior artifacts.

The Markdown sibling prints the same summary, grip-class, top missing
discriminator, oracle-strength, related-test confidence, evidence-quality,
largest canonical group, actionability, static limitation distribution,
evidence-record calibration coverage, calibration, top evidence-quality risk,
and top limitation sections for humans. High-cardinality
missing-discriminator and static-limitation reason details remain complete in
JSON and are capped in Markdown. Static-language constraints still apply:
runtime-specific labels stay confined to the optional imported calibration
availability section.

## Lane 1 Runtime Status

Lane 1 repair-control reports keep the existing advisory `status` field and add
a separate `run_status` field for completeness:

```text
full
limited_timeout
limited_runner_failure
limited_large_cache_skip
limited_incomplete_input
limited_stale_input
```

Reports that emit this contract also include `runtime_status`:

```json
{
  "run_status": "limited_timeout",
  "runtime_status": {
    "state": "limited_timeout",
    "phase": "repo_exposure_generation",
    "duration_ms": 120000,
    "limit_ms": 120000,
    "input_kind": "repo-exposure-json",
    "input_path": null,
    "limitation_category": "lane1_repo_exposure_timeout",
    "repair_route": "inspect repo-exposure latency trace",
    "downstream_consumable": false
  }
}
```

`run_status = "full"` means the report did not observe a completeness-affecting
runtime limitation. Limited states must name the phase, duration or limit when
available, an input kind or path, a limitation category, a repair route, and
whether downstream consumers may safely use the counts. `status = "advisory"`
still means the artifact does not change gate policy or public badge semantics.
Downstream surfaces must read `run_status` before treating Lane 1 counts as
complete.

## Lane 1 Evidence Quality Audit

`cargo xtask lane1-evidence-audit` writes a repo-local audit over generated
`ripr check --mode instant --format repo-exposure-json`
`seams[].evidence_record` data:

```text
target/ripr/reports/lane1-evidence-audit.json
target/ripr/reports/lane1-evidence-audit.md
target/ripr/reports/actionable-gaps.json
target/ripr/reports/actionable-gaps.md
```

`cargo xtask evidence-quality-audit` is an alias. The report is advisory and
does not change analyzer behavior, gate policy, PR/CI projection, editor UX, or
runtime execution.

```json
{
  "schema_version": "0.1",
  "tool": "ripr",
  "report": "lane1-evidence-audit",
  "scope": "repo",
  "status": "advisory",
  "run_status": "full",
  "runtime_status": {
    "state": "full",
    "phase": null,
    "duration_ms": null,
    "limit_ms": null,
    "input_kind": null,
    "input_path": null,
    "limitation_category": null,
    "repair_route": null,
    "downstream_consumable": true
  },
  "inputs": {
    "root": ".",
    "source": "repo-exposure-json",
    "repo_exposure_mode": "instant",
    "repo_exposure_schema_version": "0.3",
    "repo_exposure_generation": {
      "command": "target/debug/ripr check --root . --mode instant --format repo-exposure-json",
      "timeout_ms": 120000,
      "status": "pass",
      "failure_reason": null,
      "duration_ms": 42000,
      "exit_code": 0,
      "stdout_bytes": 1048576,
      "stderr_bytes": 4096,
      "latency_trace_events_total": 18,
      "latency_trace_tail": [
        {
          "phase": "evidence_for_seams_progress",
          "status": "processed_5000_of_38124",
          "duration_ms": 41000
        }
      ]
    }
  },
  "run_limitations": [],
  "summary": {
    "seams_total": 9355,
    "raw_headline_gaps": 6114,
    "evidence_records_total": 9355,
    "evidence_records_missing": 0,
    "canonical_gap_groups_total": 4800,
    "duplicate_looking_groups_total": 240,
    "headline_without_canonical_gap_id": 12,
    "missing_discriminators_total": 1756,
    "static_limitations_total": 4356,
    "related_tests_total": 2200,
    "seams_without_related_tests": 310,
    "low_or_opaque_top_related_tests": 48,
    "calibrated_records": 0,
    "uncalibrated_records": 9355
  },
  "finding_alignment": {
    "source": "evidence_record.canonical_item",
    "summary": {
      "raw_findings": 9355,
      "raw_signals": 9355,
      "canonical_items": 9355,
      "aligned_raw_findings": 9355,
      "unaligned_raw_findings": 0,
      "raw_to_canonical_ratio": 1.0,
      "duplicate_groups_total": 0,
      "actionable_gaps": 1756,
      "already_observed": 3200,
      "internal_no_action": 20,
      "static_limitations": 4356,
      "unknown": 23,
      "calibrated_supported": 0,
      "uncalibrated": 9355,
      "presentation_text_total": 0,
      "presentation_text_visibility_unknown": 0,
      "finding_alignment_raw_signals_total": 9355,
      "finding_alignment_canonical_items_total": 9355,
      "finding_alignment_actionable_items_total": 1756,
      "finding_alignment_static_limitation_total": 4356
    },
    "coverage": {
      "alignment_coverage_by_class": [
        {
          "evidence_class": "predicate_boundary",
          "raw_findings": 4200,
          "canonical_items": 3900,
          "aligned_raw_findings": 4100,
          "unaligned_raw_findings": 100,
          "actionable_items": 900,
          "already_observed_items": 1700,
          "internal_no_action_items": 20,
          "static_limitation_items": 1200,
          "unknown_items": 80
        }
      ],
      "unaligned_raw_findings_by_class": {
        "config_or_policy_constant": 12
      },
      "top_unaligned_examples": [
        {
          "evidence_class": "config_or_policy_constant",
          "file": "src/policy.rs",
          "line": 10,
          "kind": "static_unknown",
          "expression": "pub const POLICY_LABEL: &str = \"internal\";",
          "reason": "missing canonical_item"
        }
      ],
      "same_line_duplicate_groups": [
        {
          "file": "src/policy.rs",
          "line": 10,
          "raw_findings": 2,
          "evidence_classes": ["config_or_policy_constant"],
          "kinds": ["exposed", "static_unknown"],
          "example_expression": "pub const POLICY_LABEL: &str ="
        }
      ],
      "static_unknown_without_named_limitation": 0,
      "canonical_items_without_repair_route": 0,
      "canonical_items_without_verify_command": 120
    },
    "actionable_gap_top_lists": {
      "top_actionable_gap_classes": [
        {"label": "predicate_boundary", "count": 900}
      ],
      "top_actionable_files": [
        {"label": "src/pricing.rs", "count": 42}
      ],
      "top_repair_kinds": [
        {"label": "add_boundary_assertion", "count": 810}
      ],
      "top_missing_discriminator_kinds": [
        {"label": "return_value", "count": 720}
      ],
      "top_static_limitation_reasons": [
        {"label": "opaque helper value", "count": 1200}
      ],
      "top_verify_command_unknowns": [
        {"label": "predicate_boundary", "count": 120}
      ],
      "top_repair_route_unknowns": []
    },
    "actionable_gap_packets": [
      {
        "canonical_gap_id": "gap:abc",
        "evidence_class": "predicate_boundary",
        "gap_state": "actionable",
        "actionability": "extend_related_test",
        "source_file": "src/pricing.rs",
        "primary_anchor": {"file": "src/pricing.rs", "line": 42},
        "repair_kind": "add_boundary_assertion",
        "target_test_type": "boundary_discriminator",
        "assertion_shape": "assert_eq!(price(/* boundary input where amount == threshold */), expected)",
        "repair_route": {
          "repair_kind": "add_boundary_assertion",
          "target_test_type": "boundary_discriminator",
          "assertion_shape": "assert_eq!(price(/* boundary input where amount == threshold */), expected)"
        },
        "target_test_shape": "boundary_discriminator: assert_eq!(price(/* boundary input where amount == threshold */), expected)",
        "recommended_repair": "Add or strengthen `assert_eq!(price(/* boundary input where amount == threshold */), expected)` for `input that hits the boundary: amount == threshold` in `tests/pricing.rs` as `price_boundary_discriminator`.",
        "why": "Related tests reach the seam but miss equality at the threshold.",
        "related_test_or_observer": {
          "file": "tests/pricing.rs",
          "name": "below_threshold_has_no_discount",
          "line": 10
        },
        "candidate_value_or_observer": "input that hits the boundary: amount == threshold",
        "verify_command": "cargo xtask evidence-quality-scorecard",
        "repair_route_source": "canonical_item.repair_route",
        "verify_command_source": "canonical_item.verify_command",
        "receipt_command": null,
        "receipt_command_or_path": null,
        "receipt_source": "missing",
        "public_projection_eligible": false,
        "projection_exclusion_reasons": ["missing_receipt_path"],
        "raw_evidence_refs": [
          {"file": "src/pricing.rs", "line": 42, "kind": "weakly_exposed"}
        ],
        "raw_findings": [
          {"file": "src/pricing.rs", "line": 42, "kind": "weakly_exposed"}
        ],
        "raw_findings_supporting_only": true,
        "static_limitations": [],
        "confidence": {"basis": "static_only"},
        "confidence_basis": "static_only",
        "must_not_change": [
          "Do not infer actionability from raw static class."
        ]
      }
    ],
    "actionable_gap_packet_public_projection": {
      "scope": "emitted_actionable_gap_packets",
      "public_projection_eligible_packets": 0,
      "public_projection_excluded_packets": 1,
      "projection_exclusion_reasons": [
        {"label": "missing_receipt_path", "count": 1}
      ]
    },
    "runtime_confidence_by_class": [
      {
        "evidence_class": "predicate_boundary",
        "canonical_items": 900,
        "calibrated_supported": 0,
        "fixture_backed": 0,
        "static_only": 900,
        "unknown_confidence": 0,
        "uncalibrated": 900,
        "actionable_items": 120,
        "static_limitation_items": 40
      }
    ]
  },
  "canonical_gap_groups": {
    "total": 4800,
    "largest": [
      {
        "key": "canonical:gap:abc",
        "canonical_gap_id": "gap:abc",
        "count": 8,
        "reported_group_size": 8,
        "owner": "pricing::discount",
        "seam_kind": "predicate_boundary",
        "flow_sink": "return_value",
        "missing_discriminator": "amount == threshold",
        "assertion_shape": "exact_value",
        "example_seam_id": "f3c9e4d21a0b7c88",
        "example_file": "src/pricing.rs"
      }
    ]
  },
  "duplicate_looking_groups": [],
  "missing_discriminator_classes": {
    "by_reason": [
      {"label": "boundary value not observed", "count": 900}
    ],
    "by_flow_sink": {
      "return_value": 870
    },
    "by_value": [
      {"label": "amount == threshold", "count": 4}
    ]
  },
  "static_limitations": {
    "by_reason": [
      {"label": "static evidence is opaque or unknown for this seam", "count": 1200}
    ],
    "by_stage": {
      "activate": 800
    },
    "by_category": {
      "activation_static_unknown": 800
    },
    "repair_routes": {
      "analysis/static-limitation-taxonomy": 800
    }
  },
  "oracle_semantics_distribution": {
    "by_semantics": [
      {
        "label": "observes=exact return value; missing=boundary equality; upgrade=add equality boundary",
        "count": 42
      }
    ],
    "oracle_kind_counts": {
      "exact_value": 700
    },
    "oracle_strength_counts": {
      "strong": 800
    }
  },
  "related_test_ranking": {
    "all_confidence_counts": {
      "high": 910,
      "medium": 1060,
      "low": 220,
      "opaque": 10
    },
    "top_confidence_counts": {
      "high": 600,
      "medium": 900,
      "low": 40,
      "opaque": 8
    },
    "top_relation_reason_counts": {
      "direct_owner_call": 600
    },
    "seams_without_related_tests": 310,
    "low_or_opaque_top_related_tests": 48
  },
  "movement_availability": {
    "records_with_seam_id": 9355,
    "records_with_canonical_gap_id": 4800,
    "records_with_complete_evidence_path": 9355,
    "records_with_recommendation": 9355,
    "records_with_verify_command": 1756
  },
  "calibration_availability": {
    "availability_counts": {
      "not_imported": 9355
    },
    "confidence_counts": {
      "unknown": 9355
    },
    "agreement_counts": {
      "no_runtime_data": 9355
    },
    "calibrated_records": 0,
    "uncalibrated_records": 9355,
    "runtime_confidence_by_class": [
      {
        "evidence_class": "predicate_boundary",
        "canonical_items": 900,
        "calibrated_supported": 0,
        "fixture_backed": 0,
        "static_only": 900,
        "unknown_confidence": 0,
        "uncalibrated": 900,
        "actionable_items": 42,
        "static_limitation_items": 0
      }
    ]
  },
  "evidence_record_field_health": [
    {
      "field": "canonical_gap_id",
      "present": 4800,
      "missing": 0,
      "null": 4555,
      "empty": 0
    }
  ],
  "top_files_by_unresolved_evidence_debt": [
    {
      "file": "src/pricing.rs",
      "debt_score": 42,
      "headline_gaps": 10,
      "missing_discriminators": 10,
      "static_limitations": 5,
      "unknown_stage_records": 12,
      "no_related_tests": 3,
      "low_or_opaque_top_related_tests": 2,
      "missing_evidence_records": 0
    }
  ]
}
```

Field contract:

- `schema_version` - currently `"0.1"`.
- `tool` - always `"ripr"`.
- `report` - always `"lane1-evidence-audit"`.
- `scope` - always `"repo"`.
- `status` - always `"advisory"`.
- `run_status` - Lane 1 completeness state. Values are `full`,
  `limited_timeout`, `limited_runner_failure`, `limited_large_cache_skip`,
  `limited_incomplete_input`, or `limited_stale_input`.
- `runtime_status` - structured completeness context matching `run_status`.
  Limited states name the phase, input kind or path, limitation category, repair
  route, timing fields when available, and `downstream_consumable`.
- `inputs.root` - analyzed root for the generated repo exposure snapshot.
- `inputs.source` - always `"repo-exposure-json"`.
- `inputs.repo_exposure_mode` - currently `"instant"`; this keeps the
  repo-local audit bounded while preserving the existing repo-exposure
  `evidence_record` contract.
- `inputs.repo_exposure_generation.latency_trace_tail` - includes the
  preserved `repo_exposure_seam_limit` trace row when the default sampled
  repo-exposure input path is used. `cargo xtask lane1-evidence-audit` samples
  5,000 seams by default via the internal `RIPR_REPO_EXPOSURE_SEAM_LIMIT`
  handoff; operators can set `RIPR_LANE1_EVIDENCE_AUDIT_SAMPLE_SEAMS=0` for an
  unsampled full-repo attempt, or a positive integer to change the sample size.
- `inputs.repo_exposure_schema_version` - schema version read from the generated
  repo exposure JSON, or `null` if absent.
- `inputs.repo_exposure_generation` - bounded diagnostics for the live
  repo-exposure subprocess, including timeout, status, nullable
  `failure_reason`, duration, output byte counts, and the last captured latency
  trace events. These diagnostics explain long or pathological audit input
  generation without changing classifications, gate policy, or score semantics.
- `run_limitations` - bounded report-level limitations. A timed-out
  repo-exposure subprocess produces a warning audit artifact with a
  `lane1_repo_exposure_timeout` row, phase/input context, timeout/duration
  diagnostics, the latency trace tail, and a repair route. A subprocess that
  exits before writing complete repo-exposure JSON, including a nominally
  successful exit with an empty or malformed output file, produces
  `lane1_repo_exposure_incomplete` with the same bounded diagnostics. Counts in
  such limited artifacts are not complete repo truth and downstream reports must
  surface the limitation instead of treating zeros as absence of gaps. A runner
  or capture failure before repo exposure can be started or read produces
  `lane1_repo_exposure_runner_error` with `failure_reason`, command, timeout,
  duration, phase/input context, and a repair route. If the existing
  `target/ripr/cache` footprint exceeds the Lane 1 cache budget before
  repo-exposure generation starts, the audit writes
  `lane1_repo_exposure_large_cache_preflight_skip` with `run_status =
  "limited_large_cache_skip"`, `downstream_consumable = false`, and a repair
  route through `cargo xtask cache report` and `cargo xtask cache gc --dry-run`.
  A completed audit may also report
  `lane1_repo_exposure_cache_store_skipped_large_entry` when the live
  repo-exposure run emitted complete evidence but skipped a full classified
  seam cache store because the entry exceeded the bounded cache-store limit.
  The default sampled repo-exposure path records
  `lane1_repo_exposure_sampled` with input such as
  `repo-exposure-json:limit_5000_of_39685`; sampled counts are useful work-queue
  evidence, not full-repo debt totals.
  Run-limitation rows also carry `run_status`, `input_kind`, `input_path`,
  `limit_ms`, and `downstream_consumable` so consumers do not need to infer
  completeness from category strings.
  Named `run_limitations[]` entries also contribute to
  `summary.static_limitations_total` and `static_limitations.by_category`, so a
  limited audit cannot look like a clean zero-limitation run in headline
  summaries.
- `summary.raw_headline_gaps` - count of seams that are headline-eligible in
  the record or top-level repo exposure row.
- `finding_alignment.source` - source used for audit-local alignment counts;
  currently `evidence_record.canonical_item`.
- `finding_alignment.summary.raw_signals` and
  `finding_alignment.summary.finding_alignment_raw_signals_total` - raw
  finding/supporting-signal count derived from each evidence record's
  `raw_findings[]` or `canonical_item.raw_group_size`.
- `finding_alignment.summary.canonical_items` and
  `finding_alignment.summary.finding_alignment_canonical_items_total` - count
  of evidence records carrying a canonical item.
- `finding_alignment.summary.actionable_gaps`,
  `already_observed`, `internal_no_action`, `static_limitations`, `unknown`,
  `calibrated_supported`, and `uncalibrated` - audit-local rollups of
  `canonical_item.canonical_item_kind`, `gap_state`, `actionability`, and
  `confidence.basis`.
- `finding_alignment.summary.presentation_text_*` - presentation-text
  class-specific counts when those canonical items are present. These remain
  zero when the instant repo-exposure artifact has no presentation-text
  canonical items.
- `finding_alignment.coverage.alignment_coverage_by_class` - per-class raw
  finding, canonical item, state, and aligned/unaligned counts. The grain is
  `evidence_class`, using `canonical_item.evidence_class` when available and a
  conservative seam/raw-finding fallback otherwise. Rows also carry
  `static_limitation_categories` and `static_limitation_repair_routes` maps so
  static-dominated classes keep their named analyzer limitation and repair
  route instead of collapsing to a generic `static_unknown` bucket.
- `finding_alignment.coverage.unaligned_raw_findings_by_class` - raw finding
  counts by class for evidence records that do not carry `canonical_item`.
- `finding_alignment.coverage.top_unaligned_examples` - bounded examples of
  raw findings that did not align to a canonical item, for fixture-first
  follow-up selection. Each example includes `evidence_class`, `file`, `line`,
  `kind`, `expression`, and `reason` so the next fixture can be selected from
  typed raw-finding context instead of Markdown prose.
- `finding_alignment.coverage.same_line_duplicate_groups` - bounded raw
  finding groups sharing one file and line so maintainers can spot remaining
  duplicate user-action risks. Each group includes `file`, `line`,
  `raw_findings`, `evidence_classes`, `kinds`, and `example_expression`.
- `finding_alignment.coverage.evidence_class_work_queue` - ranked evidence
  classes that still need Lane 1 work, derived from alignment coverage rows.
  Rows include `work_score`, `dominant_signal`, raw/canonical/actionable/
  limitation/unknown/unaligned/duplicate counts, dominant static limitation
  category/count/repair route when present, and `next_repair`. When static
  limitations dominate a class, `next_repair` is the dominant named limitation
  repair route. This is the audit-local "choose the next class from live
  output" queue.
- `finding_alignment.coverage.static_unknown_without_named_limitation` -
  count of static-unknown or limitation-shaped canonical items without a named
  static limitation category plus repair route. Generic `static_unknown` or
  `unknown` categories do not satisfy the named-limitation requirement.
- `finding_alignment.coverage.canonical_items_without_repair_route` and
  `canonical_items_without_verify_command` - coverage counts for canonical
  items missing repair or verification guidance.
- `finding_alignment.actionable_gap_top_lists` - bounded top counts derived
  from canonical items, not raw findings. Each row is `{label, count}` sorted
  by descending count and then label. The section reports actionable gap
  classes, files, repair kinds, missing discriminator kinds, static limitation
  reasons on actionable gap records, verify-command unknowns by class, and
  repair-route unknowns by class so maintainers can choose the next
  fixture-backed repair slice from live evidence.
- `finding_alignment.actionable_gap_packets` - bounded top actionable
  canonical gap packets derived from `evidence_record.canonical_item`. Packets
  are agent-safe work items: they carry stable identity, evidence class, repair
  kind, `target_test_shape`, related test or observer, verification command,
  receipt command, raw evidence references as supporting evidence, confidence
  basis, and conservative `must_not_change` boundaries. They do not create user
  work from raw static class alone.
- `finding_alignment.actionable_gap_packet_public_projection` - packet-level
  badge-readiness diagnostics for the emitted packet set. It counts
  public-projection eligible packets, excluded packets, and stable
  `projection_exclusion_reasons` rows such as `missing_receipt_path`,
  `missing_related_test_or_observer`, `missing_confidence`,
  `missing_must_not_change`, `missing_raw_evidence_refs`, and
  `static_limitation_present`. This is advisory report evidence only and does
  not change public badge endpoint semantics.
- `finding_alignment.runtime_confidence_by_class` - runtime confidence coverage
  rows at the canonical evidence-class grain. Each row reports canonical item
  count, calibrated-supported, fixture-backed, static-only, unknown-confidence,
  uncalibrated, actionable, and static-limitation counts so maintainers can see
  which classes still need runtime support before badge-readiness work.
- `canonical_gap_groups.total` - number of distinct canonical gap IDs among
  headline records.
- `canonical_gap_groups.largest` - top canonical groups by observed count,
  capped for review.
- `duplicate_looking_groups` - canonical or fallback groups with observed count
  greater than one, or a reported group size greater than one.
- `missing_discriminator_classes` - complete `{label, count}` rows by reason
  and value plus count maps by flow sink.
- `static_limitations` - complete `{label, count}` rows by limitation reason
  plus count maps by evidence stage, normalized category, and suggested repair
  route.
- `oracle_semantics_distribution` - complete `{label, count}` rows for rendered
  related-test oracle semantics plus oracle kind and strength counts.
  Free-form text counts are not object keys because discriminator, limitation,
  and oracle text can differ only by case, and Windows/PowerShell JSON
  consumers treat object keys case-insensitively.
- `related_test_ranking` - confidence and relation-reason counts for all
  rendered related tests and for the top related test per seam.
- `movement_availability` - counts of records carrying the identity and
  recommendation fields needed by before/after evidence movement.
- `calibration_availability` - counts of imported calibration placeholder
  fields from `evidence_record`; `runtime_confidence_by_class` breaks canonical
  items down by `canonical_item.evidence_class`, confidence basis,
  actionability, and limitation state so badge-readiness work can see which
  classes remain static-only or unknown. This report does not import or execute
  calibration itself.
- `evidence_record_field_health` - per-field present, missing, null, and empty
  counts for key `evidence_record` contract fields.
- `top_files_by_unresolved_evidence_debt` - top files by an audit-local debt
  score that combines headline gaps, missing discriminators, static
  limitations, unknown stages, no related tests, low/opaque top related tests,
  and missing evidence records.

The Markdown sibling prints the same audit areas in bounded tables. JSON keeps
the complete count maps.

## Actionable Gap Packets

`cargo xtask lane1-evidence-audit` also writes a bounded packet projection for
humans and agents:

```text
target/ripr/reports/actionable-gaps.json
target/ripr/reports/actionable-gaps.md
```

The packet artifact is advisory and derives from the Lane 1 audit's
`evidence_record.canonical_item` projection. It does not change public badge
semantics, PR/CI rendering, gate policy, provider calls, generated tests, or
mutation execution.

```json
{
  "schema_version": "0.1",
  "tool": "ripr",
  "report": "actionable-gaps",
  "scope": "repo",
  "status": "advisory",
  "run_status": "full",
  "runtime_status": {
    "state": "full",
    "phase": null,
    "duration_ms": null,
    "limit_ms": null,
    "input_kind": null,
    "input_path": null,
    "limitation_category": null,
    "repair_route": null,
    "downstream_consumable": true
  },
  "source_report": "target/ripr/reports/lane1-evidence-audit.json",
  "source": "evidence_record.canonical_item",
  "packet_limit": 25,
  "summary": {
    "raw_signals": 47515,
    "canonical_items": 38445,
    "actionable_gaps": 162,
    "already_observed": 12006,
    "internal_no_action": 0,
    "static_limitations": 26277,
    "packets_emitted": 25,
    "public_projection_eligible_packets": 25,
    "public_projection_excluded_packets": 0,
    "projection_exclusion_reasons": [],
    "raw_to_canonical_ratio": 1.24,
    "repair_route_unknowns": 0,
    "verify_command_unknowns": 0
  },
  "run_limitations": [],
  "packets": [
    {
      "canonical_gap_id": "gap:abc",
      "evidence_class": "predicate_boundary",
      "gap_state": "actionable",
      "actionability": "extend_related_test",
      "source_file": "src/pricing.rs",
      "primary_anchor": {"file": "src/pricing.rs", "line": 42},
      "repair_kind": "add_boundary_assertion",
      "target_test_type": "boundary_discriminator",
      "assertion_shape": "assert_eq!(price(/* boundary input where amount == threshold */), expected)",
      "repair_route": {
        "repair_kind": "add_boundary_assertion",
        "target_test_type": "boundary_discriminator",
        "assertion_shape": "assert_eq!(price(/* boundary input where amount == threshold */), expected)"
      },
      "recommended_repair": "Add or strengthen `assert_eq!(price(/* boundary input where amount == threshold */), expected)` for `input that hits the boundary: amount == threshold` in `tests/pricing.rs` as `price_boundary_discriminator`.",
      "why": "Related tests reach the seam but miss equality at the threshold.",
      "related_test_or_observer": {
        "file": "tests/pricing.rs",
        "name": "below_threshold_has_no_discount",
        "line": 10
      },
      "candidate_value_or_observer": "input that hits the boundary: amount == threshold",
      "missing_discriminators": [
        {
          "value": "discount_threshold (equality boundary)",
          "reason": "observed values do not include the equality-boundary case for this predicate"
        }
      ],
      "verify_command": "cargo xtask evidence-quality-scorecard",
      "repair_route_source": "canonical_item.repair_route",
      "verify_command_source": "canonical_item.verify_command",
      "receipt_command_or_path": "ripr agent receipt --root . --verify-json target/ripr/workflow/agent-verify.json --seam-id probe:src_pricing_rs:42:predicate_boundary --json --out target/ripr/reports/agent-receipt.json",
      "receipt_source": "canonical_item.receipt_command",
      "public_projection_eligible": true,
      "projection_exclusion_reasons": [],
      "raw_findings": [
        {"file": "src/pricing.rs", "line": 42, "kind": "weakly_exposed"}
      ],
      "raw_findings_supporting_only": true,
      "static_limitations": [],
      "confidence_basis": "static_only",
      "must_not_change": [
        "Do not infer actionability from raw static class."
      ]
    }
  ],
  "must_not_infer": [
    "raw findings are supporting evidence, not user work",
    "do not infer actionability from raw static class",
    "do not treat named static limitations as user test debt",
    "do not claim mutation execution or runtime proof from this packet"
  ]
}
```

The packet grain is one canonical actionable item. `raw_findings[]` is included
only to preserve supporting evidence and line context; downstream consumers must
not fan it back out into separate user-facing work.
`missing_discriminators[]` carries the exact unresolved discriminator facts from
the evidence record so agents do not have to infer the boundary or assertion
target from a broader candidate-value hint.
`public_projection_eligible` is an audit-only badge-readiness decision for the
emitted packet. It is true only when the packet has public-projection
prerequisites such as canonical repair and verify fields plus a receipt command
or path; otherwise the stable `projection_exclusion_reasons[]` values explain
why an otherwise useful agent packet is not yet a public badge item. This does
not change committed badge endpoint semantics.

## RIPR Swarm Plan

`cargo xtask ripr-swarm plan --top <n>` ranks existing actionable canonical gap
packets for a bounded, dry-run repair loop:

```text
target/ripr/reports/swarm-plan.json
target/ripr/reports/swarm-plan.md
```

The command reads `target/ripr/reports/actionable-gaps.json` by default, or the
path supplied by `--actionable-gaps`. It is report-only. It does not edit files,
run tests, call providers, generate tests, create receipts, run mutation
testing, change PR/CI rendering, change editor/LSP behavior, change gates, or
change public badges.

For compatibility with current actionable-gap packets, input may carry either
`receipt_command_or_path` or `receipt_command`. The swarm plan normalizes the
ranked packet output to `receipt_command`. A packet is not swarm-ready unless it
also carries a structured `repair_route` object and a typed workspace-relative
repair target in `related_test_or_observer` or `candidate_value_or_observer`.

If the actionable-gaps input is missing or malformed, the command still writes
a bounded blocked report with the input path, input state, and limitation text.
It does not silently drop the plan or infer work from stale Markdown.

```json
{
  "schema_version": "0.1",
  "tool": "ripr",
  "report": "swarm-plan",
  "scope": "repo",
  "status": "advisory",
  "input": {
    "actionable_gaps": "target/ripr/reports/actionable-gaps.json",
    "state": "read",
    "limitation": null
  },
  "source": "actionable-gaps.packets",
  "source_summary": {
    "raw_signals": 47515,
    "canonical_items": 38445,
    "actionable_gaps": 162,
    "packets_emitted": 25
  },
  "top_limit": 10,
  "summary": {
    "packets_total": 25,
    "swarm_ready_packets": 10,
    "blocked_packets": 15,
    "missing_verify_command": 0,
    "missing_receipt_command": 0,
    "missing_repair_route": 0,
    "missing_must_not_change": 0,
    "related_context_missing": 3,
    "static_limitation_packets": 2,
    "high_confidence_packets": 4
  },
  "top_ready_packets": [
    {
      "packet_id": "gap:abc",
      "canonical_gap_id": "gap:abc",
      "evidence_class": "predicate_boundary",
      "source_file": "src/pricing.rs",
      "repair_kind": "add_boundary_assertion",
      "target_test_type": "boundary_discriminator",
      "assertion_shape": "assert_eq!(price(/* boundary */), expected)",
      "confidence_basis": "fixture_backed",
      "swarm_state": "queued",
      "score": 110,
      "expected_canonical_gap_delta": 1,
      "readiness_reasons": [
        "repair_route_present",
        "verify_command_present",
        "receipt_command_present",
        "related_test_or_observer_present",
        "must_not_change_present",
        "public_projection_eligible",
        "no_static_limitation",
        "confidence_basis_fixture_backed"
      ],
      "blocked_reasons": [],
      "missing_context": [],
      "verify_command": "cargo xtask evidence-quality-scorecard",
      "receipt_command": "cargo xtask receipts check",
      "related_test_or_observer_available": true,
      "must_not_change_count": 1,
      "raw_findings_count": 2,
      "raw_findings_supporting_only": true,
      "static_limitations_count": 0,
      "public_projection_eligible": true
    }
  ],
  "top_blocked_packets": [],
  "top_missing_verify_or_receipt": [],
  "must_not_infer": [
    "do not consume raw findings as swarm work",
    "do not rank static limitations as repair-ready",
    "do not rank static-only predicate-boundary packets as swarm-ready without stronger evidence",
    "do not rank packets without receipt_command as swarm-ready",
    "do not rank packets without verify_command as high confidence",
    "do not edit files, call providers, generate tests, run mutation testing, or create receipts from this plan"
  ]
}
```

`swarm_state = queued` means the packet is ready for a bounded dry-run repair
attempt. Queued packets require a structured `repair_route` object and a typed
workspace-relative `related_test_or_observer` or `candidate_value_or_observer`
target. Candidate prose, `repair_route_source` hints, and top-level
repair-shape strings remain supporting context only; they do not authorize file
edits or swarm-ready ranking by themselves. Packets missing required typed
context use `blocked_by_missing_context`. Packets with static limitations use
`blocked_by_static_limitation`. Static-only predicate-boundary assertion packets
use `blocked_by_operator_judgment`; they remain visible but are not default
swarm-ready until upstream evidence is fixture-backed, calibrated, or explicitly
operator-selected. Ranking is advisory and never redefines actionability; it
starts from the canonical packet state already emitted by Lane 1.

## RIPR Swarm Attempt Dry Run

`cargo xtask ripr-swarm attempt --packet <id> --dry-run` reads
`target/ripr/reports/actionable-gaps.json` by default, or the path supplied by
`--actionable-gaps`, and prints bounded packet context to stdout. The packet id
can be a `packet_id`, `canonical_gap_id`, or compatible unprefixed canonical
identifier. The command does not edit files, run tests, call providers,
generate tests, create receipts, run mutation testing, merge code, or change
public badge semantics.

The dry-run output includes:

```text
copy-ready operator packet
task
allowed files
do-not-change boundaries
repair target
verify command
receipt command
stop conditions
required return format
canonical_gap_id
evidence_class
source_file
swarm_state
confidence_basis
repair_kind
repair_route
target_test_type
assertion_or_observer_shape
related_test_or_observer
expected_evidence_movement
verify_command
receipt_command_or_path
must_not_change boundaries
raw_findings_count
static_limitations_count
```

`queued` packets show the expected canonical-gap delta if receipt-backed
evidence movement resolves or improves the item. Blocked packets remain visible
with their `blocked_by_missing_context` or `blocked_by_static_limitation` state
or `blocked_by_operator_judgment` state and are not promoted to repair-ready
work.

## Actionable Gap Outcomes

`cargo xtask actionable-gap-outcomes` joins actionable-gap packets with optional
agent receipt and targeted-test outcome artifacts:

```text
target/ripr/reports/actionable-gap-outcomes.json
target/ripr/reports/actionable-gap-outcomes.md
```

The report is advisory. It does not run repairs, generate tests, execute
mutation testing, change PR/CI rendering, or change public badge semantics.

```json
{
  "schema_version": "0.1",
  "tool": "ripr",
  "report": "actionable-gap-outcomes",
  "scope": "repo",
  "status": "advisory",
  "source": "actionable-gaps plus optional receipt and targeted-test outcome artifacts",
  "inputs": {
    "actionable_gaps": "target/ripr/reports/actionable-gaps.json",
    "agent_receipt": "target/ripr/reports/agent-receipt.json",
    "targeted_test_outcome": "target/ripr/reports/targeted-test-outcome.json"
  },
  "summary": {
    "packets_total": 25,
    "outcomes_total": 25,
    "not_attempted": 22,
    "attempted_no_receipt": 0,
    "receipt_present": 0,
    "evidence_improved": 1,
    "evidence_unchanged": 1,
    "evidence_regressed": 0,
    "resolved": 1,
    "unknown": 0,
    "receipts_present": 1,
    "receipts_missing_after_input": 24,
    "orphaned_receipts": 1
  },
  "movement_front": {
    "current_actionable_count": 25,
    "receipt_linked_actionable_delta": -1,
    "resolved": 1,
    "improved": 1,
    "unchanged_after_attempt": 1,
    "missing_receipts": 24,
    "orphaned_receipts": 1,
    "top_blocked_reason": "missing_receipts"
  },
  "outcomes": [
    {
      "canonical_gap_id": "gap:abc",
      "evidence_class": "predicate_boundary",
      "repair_kind": "add_boundary_assertion",
      "source_file": "src/pricing.rs",
      "verify_command": "ripr agent verify --root . --before before.json --after after.json --json",
      "receipt_command_or_path": "ripr agent receipt --root . --verify-json target/ripr/workflow/agent-verify.json --seam-id abc --json --out target/ripr/reports/agent-receipt.json",
      "receipt_state": "receipt_movement_improved",
      "outcome_state": "evidence_improved",
      "seam_id": "abc",
      "before": "weakly_gripped",
      "after": "strongly_gripped",
      "movement_source": "agent_receipt",
      "movement_direction": "improved",
      "evidence_delta": [
        "missing discriminator no longer reported: threshold equality"
      ],
      "reason": "Matched agent receipt artifact."
    }
  ],
  "orphaned_receipts": [
    {
      "receipt_id": "receipt:old-gap",
      "seam_id": "old-gap",
      "source_file": "src/old.rs",
      "line": 7,
      "movement_direction": "improved",
      "reason": "Receipt artifact did not match any current actionable canonical gap packet."
    }
  ],
  "must_not_infer": [
    "outcome reports join existing artifacts; they do not execute repairs",
    "raw findings remain supporting evidence, not user work",
    "targeted-test outcomes are static evidence movement, not mutation proof",
    "missing receipts do not imply a repair failed",
    "orphaned receipts do not create new actionable gaps"
  ]
}
```

`outcome_state` uses the bounded Lane 1 lifecycle states
`not_attempted`, `attempted_no_receipt`, `receipt_present`,
`evidence_improved`, `evidence_unchanged`, `evidence_regressed`, `resolved`,
and `unknown`. Raw findings do not determine outcome state; the join is based
on canonical packet identity, seam identity, or the packet primary anchor.
`receipt_state` uses the canonical receipt lifecycle vocabulary:
`receipt_missing`, `receipt_found`, `receipt_stale`,
`receipt_gap_mismatch`, `receipt_movement_improved`,
`receipt_movement_unchanged`, or `receipt_not_applicable`.
`movement_front` is the first-screen outcome summary. It reports the current
actionable packet count, receipt-linked actionable delta, resolved/improved
movement, unchanged attempts, missing/orphaned receipts, and the top follow-up
blocker. The delta is receipt-linked static movement only: it is not mutation
confirmation, runtime adequacy, policy eligibility, gate passage, or merge
readiness.
`orphaned_receipts[]` preserves receipt artifacts that do not match any current
packet so attempt history remains visible without creating new actionable gaps.

## RIPR Swarm Attempt Ledger

`cargo xtask ripr-swarm attempt-ledger` joins the swarm plan and
actionable-gap outcome report into durable attempt history:

```text
target/ripr/reports/swarm-attempt-ledger.json
target/ripr/reports/swarm-attempt-ledger.md
```

The command reads `target/ripr/reports/swarm-plan.json`,
`target/ripr/reports/actionable-gap-outcomes.json`, and any existing
`target/ripr/reports/swarm-attempt-ledger.json` by default. It preserves prior
attempt entries by `attempt_id`, adds the current outcome join, and highlights
the latest attempt per `canonical_gap_id`. It does not execute repairs, edit
files, run tests, create receipts, call providers, run mutation testing, change
PR/CI rendering, change editor/LSP behavior, change gates, or change public
badges.

```json
{
  "schema_version": "0.1",
  "tool": "ripr",
  "report": "swarm-attempt-ledger",
  "scope": "repo",
  "status": "advisory",
  "run_status": "full",
  "runtime_status": {
    "state": "full",
    "phase": null,
    "duration_ms": null,
    "limit_ms": null,
    "input_kind": null,
    "input_path": null,
    "limitation_category": null,
    "repair_route": null,
    "downstream_consumable": true
  },
  "generated_at": "unix_ms:1778240100000",
  "inputs": {
    "swarm_plan": {
      "path": "target/ripr/reports/swarm-plan.json",
      "state": "read",
      "limitation": null
    },
    "actionable_gap_outcomes": {
      "path": "target/ripr/reports/actionable-gap-outcomes.json",
      "state": "read",
      "limitation": null
    },
    "prior_ledger": {
      "path": "target/ripr/reports/swarm-attempt-ledger.json",
      "state": "read",
      "limitation": null
    }
  },
  "summary": {
    "attempts_total": 4,
    "canonical_gaps_total": 3,
    "not_attempted": 1,
    "attempted_no_receipt": 0,
    "receipt_present": 0,
    "evidence_improved": 2,
    "evidence_unchanged": 1,
    "evidence_regressed": 0,
    "resolved": 0,
    "unknown": 0,
    "orphaned_receipts": 1
  },
  "attempts": [
    {
      "packet_id": "packet-boundary-001",
      "canonical_gap_id": "gap:abc",
      "attempt_id": "attempt:gap-abc:evidence-improved:receipt-movement-improved:agent-receipt:abc",
      "actor_kind": "agent",
      "receipt_path": "target/ripr/reports/agent-receipt.json",
      "verify_command": "cargo test -p ripr boundary_gap",
      "receipt_command": "cargo xtask receipts write --packet packet-boundary-001",
      "before_gap_state": "weakly_gripped",
      "after_gap_state": "strongly_gripped",
      "outcome": "evidence_improved",
      "timestamp": "unix_ms:1778240100000",
      "receipt_state": "receipt_movement_improved",
      "movement_source": "agent_receipt",
      "reason": "Matched agent receipt artifact."
    }
  ],
  "latest_attempts": [
    {
      "packet_id": "packet-boundary-001",
      "canonical_gap_id": "gap:abc",
      "attempt_id": "attempt:gap-abc:evidence-improved:receipt-movement-improved:agent-receipt:abc",
      "actor_kind": "agent",
      "receipt_path": "target/ripr/reports/agent-receipt.json",
      "verify_command": "cargo test -p ripr boundary_gap",
      "receipt_command": "cargo xtask receipts write --packet packet-boundary-001",
      "before_gap_state": "weakly_gripped",
      "after_gap_state": "strongly_gripped",
      "outcome": "evidence_improved",
      "timestamp": "unix_ms:1778240100000",
      "receipt_state": "receipt_movement_improved",
      "movement_source": "agent_receipt",
      "reason": "Matched agent receipt artifact."
    }
  ],
  "orphaned_receipts": [
    {
      "receipt_id": "receipt:old-gap",
      "seam_id": "old-gap",
      "reason": "Receipt artifact did not match any current actionable canonical gap packet."
    }
  ],
  "must_not_infer": [
    "attempt ledgers preserve existing artifact joins; they do not execute repairs",
    "not_attempted means no matching attempt artifact was supplied, not that repair failed",
    "receipt_present without movement is not evidence improvement",
    "orphaned receipts do not create new actionable gaps",
    "ledger counts do not change public badge semantics or CI gate mode"
  ]
}
```

`attempts[]` is durable history. `latest_attempts[]` is the current routing
view, one entry per canonical gap, and is the source readiness uses for
attempt/improved/unchanged/regressed/resolved counts. The ledger preserves
`not_attempted`, `attempted_no_receipt`, `receipt_present`,
`evidence_improved`, `evidence_unchanged`, `evidence_regressed`, `resolved`,
and `unknown` outcomes. Missing outcome inputs make the ledger
`limited_incomplete_input`; missing swarm-plan input is consumable but
explicitly limited because packet ids may be less complete.

## RIPR Swarm Readiness

`cargo xtask ripr-swarm readiness` rolls up the existing swarm plan,
actionable-gap outcomes, and swarm attempt ledger into a repo-level
repair-coordination readiness report:

```text
target/ripr/reports/swarm-readiness.json
target/ripr/reports/swarm-readiness.md
```

The command reads `target/ripr/reports/swarm-plan.json` and
`target/ripr/reports/actionable-gap-outcomes.json` and
`target/ripr/reports/swarm-attempt-ledger.json` by default, or the paths
provided by `--swarm-plan`, `--actionable-gap-outcomes`, and
`--attempt-ledger`. It is report-only. It does not execute repairs, edit files,
run tests, call providers, generate tests, create receipts, run mutation
testing, change PR/CI rendering, change editor/LSP behavior, change gates, or
change public badges.

If `swarm-plan.json` is missing or malformed, the report is `blocked` with a
bounded input limitation. If `actionable-gap-outcomes.json` or
`swarm-attempt-ledger.json` is missing, the report records the limitation and
routes the operator to regenerate the missing artifact; missing outcomes or
ledger inputs do not imply failed attempts.

Swarm plan and readiness reports include `run_status` and `runtime_status`.
Readiness preserves a limited swarm-plan input, and reports missing or malformed
required plan input as `limited_incomplete_input` instead of turning absent
packets into a clean zero-ready state.

```json
{
  "schema_version": "0.1",
  "tool": "ripr",
  "report": "swarm-readiness",
  "scope": "repo",
  "status": "advisory",
  "run_status": "full",
  "runtime_status": {
    "state": "full",
    "phase": null,
    "duration_ms": null,
    "limit_ms": null,
    "input_kind": null,
    "input_path": null,
    "limitation_category": null,
    "repair_route": null,
    "downstream_consumable": true
  },
  "inputs": {
    "swarm_plan": {
      "path": "target/ripr/reports/swarm-plan.json",
      "state": "read",
      "limitation": null
    },
    "actionable_gap_outcomes": {
      "path": "target/ripr/reports/actionable-gap-outcomes.json",
      "state": "read",
      "limitation": null
    },
    "attempt_ledger": {
      "path": "target/ripr/reports/swarm-attempt-ledger.json",
      "state": "read",
      "limitation": null
    }
  },
  "summary": {
    "actionable_gaps_total": 162,
    "public_projection_eligible_packets": 25,
    "swarm_ready_packets": 10,
    "blocked_packets": 15,
    "missing_verify_command": 0,
    "missing_receipt_command": 0,
    "static_limitation_packets": 2,
    "high_confidence_packets": 4,
    "attempted_packets": 3,
    "improved_packets": 2,
    "unchanged_packets": 1,
    "regressed_packets": 0,
    "resolved_packets": 1,
    "orphaned_receipts": 0
  },
  "next_actions": [
    {
      "kind": "inspect_unchanged_attempts",
      "packet_id": null,
      "canonical_gap_id": null,
      "evidence_class": null,
      "repair_kind": null,
      "command": "cargo xtask actionable-gap-outcomes",
      "reason": "1 attempted packet(s) left evidence unchanged; refine the repair route before retrying"
    },
    {
      "kind": "route_static_limitations",
      "packet_id": null,
      "canonical_gap_id": null,
      "evidence_class": null,
      "repair_kind": null,
      "command": "cargo xtask lane1-evidence-audit",
      "reason": "2 packet(s) are blocked by static limitations; route them to the Lane 1 analyzer backlog, not repair execution"
    },
    {
      "kind": "route_operator_judgment_packets",
      "packet_id": "gap:static-only-boundary",
      "canonical_gap_id": "gap:static-only-boundary",
      "evidence_class": "predicate_boundary",
      "repair_kind": "add_boundary_assertion",
      "command": "cargo xtask ripr-swarm plan --top 10",
      "reason": "1 top blocked packet(s) require operator judgment; improve upstream evidence confidence or choose a manual repair outside the default swarm-ready queue"
    },
    {
      "kind": "attempt_ready_packet",
      "packet_id": "packet-boundary-001",
      "canonical_gap_id": "gap:boundary",
      "evidence_class": "predicate_boundary",
      "repair_kind": "add_boundary_assertion",
      "command": "cargo xtask ripr-swarm attempt --packet packet-boundary-001 --dry-run",
      "reason": "packet is queued with repair, verify, receipt, and no static limitation"
    }
  ],
  "must_not_infer": [
    "readiness reports summarize existing swarm artifacts; they do not execute repairs",
    "raw findings remain supporting evidence, not swarm work",
    "missing outcome artifacts mean no outcome join is available, not that attempts failed",
    "readiness counts do not change public badge semantics",
    "static limitations and blocked packets are not repair-ready work"
  ]
}
```

The readiness report is the management dashboard for repair coordination. It
summarizes whether actionable packets have enough typed context to be
swarm-ready, whether attempts have been recorded, and whether receipt-backed
outcomes improved, stayed unchanged, regressed, or resolved. It does not make
badge-readiness claims by itself. `next_actions` is a bounded advisory queue
derived from the same plan and outcome artifacts. It can point operators to a
ready dry-run packet, missing verify/receipt source fields, orphaned receipts,
unchanged or regressed attempts, static-limitation backlog work, or
operator-judgment packets that are visible but not default swarm-ready. It does
not execute the action or consume raw findings as work.


## Evidence Quality Scorecard

`cargo xtask evidence-quality-scorecard` writes a repo-local Lane 1 scorecard
over existing evidence-quality artifacts:

```text
target/ripr/reports/evidence-quality-scorecard.json
target/ripr/reports/evidence-quality-scorecard.md
```

The command reads `target/ripr/reports/lane1-evidence-audit.json`, regenerating
the audit first only when that required input is absent. It also reads
`target/ripr/reports/evidence-health.json` and the previous scorecard artifact
when they are already available. The report is advisory and does not change
analyzer behavior, gate policy, PR/CI projection, editor output, source files,
generated tests, provider calls, or runtime execution.

If the scorecard cannot regenerate a missing Lane 1 audit, it still writes a
bounded diagnostic scorecard instead of silently dropping the report. That
limited scorecard carries `unknowns[].kind =
"evidence_quality_scorecard_audit_regeneration_failed"` and an audit
`run_limitations[]` entry with the same category. Counts in that artifact are
diagnostic only and must not be treated as complete repo truth or user test
debt.

Scorecard JSON includes `run_status` and `runtime_status`. It preserves a
limited current audit or limited evidence-health input instead of converting
partial counts into a clean scorecard headline. A completed audit that only
  skipped a large cache store reports `limited_large_cache_skip` with
  `downstream_consumable = true`; an audit skipped before generation because
  the existing cache footprint exceeded the Lane 1 budget reports
  `limited_large_cache_skip` with `downstream_consumable = false`; timeout,
  runner failure, sampled, incomplete, or audit-regeneration states are not
  complete repo truth.

When a Lane 1 audit carries named `run_limitations[]`, the scorecard treats the
matching `static_limitations.by_category` rows as static limitations even if an
older or partial audit summary did not increment `summary.static_limitations_total`.
This keeps limited artifacts visible in the headline static-limitation count
instead of presenting a misleading zero.

```json
{
  "schema_version": "0.1",
  "tool": "ripr",
  "report": "evidence-quality-scorecard",
  "generated_at": "unix_ms:1778620000000",
  "run_status": "full",
  "runtime_status": {
    "state": "full",
    "phase": null,
    "duration_ms": null,
    "limit_ms": null,
    "input_kind": null,
    "input_path": null,
    "limitation_category": null,
    "repair_route": null,
    "downstream_consumable": true
  },
  "scope": {
    "kind": "repo",
    "root": "."
  },
  "inputs": {
    "lane1_evidence_audit": {
      "path": "target/ripr/reports/lane1-evidence-audit.json",
      "status": "loaded",
      "schema_version": "0.1",
      "sha256": "0123456789abcdef",
      "note": "required Lane 1 evidence-quality audit input"
    },
    "evidence_health": {
      "path": "target/ripr/reports/evidence-health.json",
      "status": "missing",
      "schema_version": null,
      "sha256": null,
      "note": "optional durable evidence-health audit fields"
    }
  },
  "headline": {
    "primary_metric": "finding_alignment_actionable_unresolved_canonical_gaps",
    "primary_count": 0,
    "counting_model": "actionable_canonical_gaps",
    "raw_signals": 2,
    "canonical_items": 1,
    "already_observed": 0,
    "internal_no_action": 0,
    "static_limitations": 4356,
    "unknown": 0,
    "raw_to_canonical_ratio": 2.0,
    "note": "Raw findings are diagnostic; actionable canonical gaps are the user-facing repair count."
  },
  "summary": {
    "raw_headline_gaps": 6114,
    "canonical_gap_groups_total": 4800,
    "duplicate_looking_groups_total": 240,
    "missing_discriminators_total": 1756,
    "static_limitations_total": 4356,
    "related_tests_total": 2200,
    "low_or_opaque_top_related_tests": 48,
    "calibrated_records": 0,
    "uncalibrated_records": 9355,
    "evidence_records_total": 9355,
    "evidence_records_missing": 0,
    "top_repair_count": 5,
    "recent_delta_available": false,
    "finding_alignment_raw_findings_total": 2,
    "finding_alignment_raw_signals_total": 2,
    "finding_alignment_canonical_items_total": 1,
    "finding_alignment_aligned_raw_findings_total": 2,
    "finding_alignment_unaligned_raw_findings_total": 0,
    "finding_alignment_raw_to_canonical_ratio": 2.0,
    "finding_alignment_duplicate_groups_total": 1,
    "finding_alignment_actionable_items_total": 0,
    "finding_alignment_actionable_unresolved_canonical_gaps": 0,
    "finding_alignment_already_observed_total": 0,
    "finding_alignment_internal_only_total": 0,
    "finding_alignment_internal_no_action_total": 0,
    "finding_alignment_static_limitation_total": 1,
    "finding_alignment_unknown_total": 0,
    "finding_alignment_calibrated_supported_total": 0,
    "finding_alignment_uncalibrated_total": 1,
    "finding_alignment_visibility_unknown_total": 1,
    "finding_alignment_presentation_text_actionable_total": 0,
    "finding_alignment_static_unknown_without_named_limitation": 0,
    "finding_alignment_canonical_items_without_repair_route": 0,
    "finding_alignment_canonical_items_without_verify_command": 0,
    "finding_alignment_actionable_gap_packet_public_projection_eligible_packets": 25,
    "finding_alignment_actionable_gap_packet_public_projection_excluded_packets": 0,
    "presentation_text_total": 1,
    "presentation_text_user_visible": 0,
    "presentation_text_observed": 0,
    "presentation_text_unobserved": 0,
    "presentation_text_internal_only": 0,
    "presentation_text_visibility_unknown": 1,
    "presentation_text_observer_unknown": 1,
    "presentation_text_duplicate_groups": 1,
    "presentation_text_actionable_snapshot": 0,
    "presentation_text_no_action": 0,
    "presentation_text_static_limitations": 1
  },
  "maturity_by_class": [
    {
      "class": "related_test_ranking",
      "status": "static_only",
      "proof_source": "RIPR-SPEC-0029, Lane 1 audit related-test confidence distribution",
      "known_limits": "Top related-test choices include low-confidence or opaque rankings.",
      "recommended_next_repair": "analysis/related-test-ranking-audit-fixes"
    }
  ],
  "canonical_gap_groups": {
    "total": 4800,
    "largest": []
  },
  "duplicate_looking_groups": [],
  "static_limitation_categories": {
    "by_reason": [],
    "by_stage": {},
    "by_category": {},
    "repair_routes": {}
  },
  "missing_discriminator_classes": {
    "by_reason": [],
    "by_flow_sink": {},
    "by_value": []
  },
  "related_test_confidence": {
    "all_confidence_counts": {},
    "top_confidence_counts": {}
  },
  "oracle_semantics_distribution": {
    "by_semantics": [],
    "oracle_kind_counts": {},
    "oracle_strength_counts": {}
  },
  "movement_availability": {
    "records_with_canonical_gap_id": 4800
  },
  "calibration_coverage": {
    "availability_counts": {
      "not_imported": 9355
    },
    "confidence_counts": {
      "unknown": 9355
    },
    "agreement_counts": {
      "no_runtime_data": 9355
    },
    "calibrated_records": 0,
    "uncalibrated_records": 9355,
    "runtime_scope": "uncalibrated",
    "by_evidence_class": [
      {
        "evidence_class": "predicate_boundary",
        "canonical_items": 900,
        "calibrated_supported": 0,
        "fixture_backed": 0,
        "static_only": 900,
        "unknown_confidence": 0,
        "uncalibrated": 900,
        "actionable_items": 42,
        "static_limitation_items": 0
      }
    ]
  },
  "actionable_gap_top_lists": {
    "top_actionable_gap_classes": [
      {"label": "predicate_boundary", "count": 900}
    ],
    "top_actionable_files": [
      {"label": "src/pricing.rs", "count": 42}
    ],
    "top_repair_kinds": [
      {"label": "add_boundary_assertion", "count": 810}
    ],
    "top_missing_discriminator_kinds": [
      {"label": "return_value", "count": 720}
    ],
    "top_static_limitation_reasons": [
      {"label": "opaque helper value", "count": 1200}
    ],
    "top_verify_command_unknowns": [
      {"label": "predicate_boundary", "count": 120}
    ],
    "top_repair_route_unknowns": []
  },
  "actionable_gap_packet_public_projection": {
    "scope": "emitted_actionable_gap_packets",
    "public_projection_eligible_packets": 25,
    "public_projection_excluded_packets": 0,
    "projection_exclusion_reasons": []
  },
  "recommended_repairs": [
    {
      "slice": "analysis/related-test-ranking-audit-fixes",
      "priority": 100,
      "evidence_class": "related_test_ranking",
      "risk_kind": "low_or_opaque_top_related_tests",
      "signal_count": 48,
      "why": "Top related-test choices include low-confidence or opaque evidence.",
      "expected_impact": "Improve first-useful-action task quality and agent packet reliability without changing gate behavior."
    }
  ],
  "recent_audit_deltas": {
    "available": false,
    "source": null,
    "reason": "no previous scorecard artifact was available",
    "deltas": []
  },
  "unknowns": [
    {
      "kind": "recent_delta_unavailable",
      "summary": "No previous scorecard artifact was available for before/after delta reporting.",
      "next_repair": "report/evidence-quality-trend"
    }
  ]
}
```

Field contract:

- `schema_version` - currently `"0.1"`.
- `tool` - always `"ripr"`.
- `report` - always `"evidence-quality-scorecard"`.
- `generated_at` - generation timestamp in `unix_ms:<millis>` form.
- `scope.kind` - always `"repo"`.
- `inputs.*` - input artifact identity with path, load status, optional schema
  version, optional SHA-256, and a short note. Missing optional artifacts are
  reported instead of treated as failures.
- `headline` - additive scorecard lead numbers for the finding-alignment
  counting model plus the audit-wide static-limitation headline.
  `primary_metric` is
  `finding_alignment_actionable_unresolved_canonical_gaps`, `primary_count` is
  the actionable canonical gap count, and raw signals remain diagnostic context
  alongside canonical item, already-observed, no-action, unknown, and
  raw-to-canonical counts. `static_limitations` mirrors the scorecard summary's
  audit-wide `static_limitations_total`, including named run limitations that
  are carried into the static-limitation taxonomy. This does not redefine public
  badges or gate policy.
- `summary` - headline scorecard counts copied from the current Lane 1 audit
  plus scorecard-local repair, delta availability, finding-alignment, and
  presentation-text counts. Finding-alignment counts preserve raw signals,
  canonical item totals, raw-to-canonical ratio, evidence states, confidence
  basis, and class-scoped presentation-text actionability without redefining
  RIPR scores.
- `maturity_by_class` - class-scoped maturity rows. Status values are
  `fixture_backed`, `static_only`, `imported_runtime_calibrated`, or
  `uncalibrated`; these are scorecard maturity labels, not RIPR exposure
  classifications.
- `canonical_gap_groups`, `duplicate_looking_groups`,
  `static_limitation_categories`, `missing_discriminator_classes`,
  `related_test_confidence`, `oracle_semantics_distribution`, and
  `movement_availability` - current audit sections carried forward so the
  scorecard remains traceable to `lane1-evidence-audit.json`. Static
  limitation categories and repair routes are advisory Lane 1 analyzer-work
  buckets; they are not user-actionable test-gap labels.
- `calibration_coverage` - class-scoped calibration availability from
  `evidence_record.calibration`; `by_evidence_class` carries the audit's
  runtime-confidence rows for canonical items so maintainers can see calibrated
  support, fixture-backed static confidence, static-only evidence, unknown
  confidence, actionable items, and limitation items by class. It does not run
  mutation testing.
- `actionable_gap_top_lists` - the audit-derived
  `finding_alignment.actionable_gap_top_lists` section carried forward for the
  scorecard. It shows the dominant actionable classes, files, repair kinds,
  missing discriminator kinds, static limitation reasons on actionable gap
  records, and guidance-unknown classes so the scorecard explains the shape of
  user work before any badge or downstream rendering change.
- `actionable_gap_packet_public_projection` - the audit-derived
  `finding_alignment.actionable_gap_packet_public_projection` readiness section
  carried forward for scorecard and trend use. It counts emitted actionable-gap
  packets that are internally ready for future public projection and lists
  exclusion reasons such as missing receipt paths. This is advisory
  badge-readiness evidence only; it does not switch public badges or PR/CI
  rendering.
- `evidence_class_work_queue` - the audit-derived
  `finding_alignment.coverage.evidence_class_work_queue` section carried
  forward so the scorecard names the next evidence classes to burn down from
  live output rather than static roadmap guesses. Static-dominated rows retain
  the dominant named limitation category and repair route, matching the audit
  queue.
- `recommended_repairs` - bounded Lane 1 repair slices. The scorecard promotes
  the audit-derived `evidence_class_work_queue` rows first so the next repair
  class comes from live evidence-class counts rather than static roadmap
  guesses; remaining generic risks are ordered by product risk priority and
  signal count. These are advisory next steps, not policy decisions.
- `recent_audit_deltas` - before/after summary deltas when a previous
  scorecard artifact is available; otherwise an explicit unavailable reason.
- `unknowns` - unavailable inputs and evidence-quality unknowns that should
  stay visible until a fixture, analyzer, or calibration slice addresses them.
  A scorecard generated after failed missing-audit regeneration includes
  `evidence_quality_scorecard_audit_regeneration_failed` and the generic
  `lane1_evidence_audit_limited` unknown so downstream consumers can explain
  the bounded diagnostic state. Non-completeness audit limitations, such as
  skipped full-cache storage after a complete repo-exposure run, remain visible
  on the audit artifact but do not mark scorecard counts as partial.

The Markdown sibling prints bounded sections for summary, finding-alignment and
presentation-text quality, actionable canonical gap top lists, actionable-gap
packet public-projection readiness, evidence-class work queue, maturity by
class, top evidence-quality risks, recommended repairs, duplicate/canonical group signals, static
limitations, missing discriminators, related-test and oracle distributions,
movement and calibration coverage, recent deltas, and unknowns.

## Evidence Quality Trend

`cargo xtask evidence-quality-trend` writes a repo-local Lane 1 trend report
over existing scorecard or audit snapshots:

```text
target/ripr/reports/evidence-quality-trend.json
target/ripr/reports/evidence-quality-trend.md
```

By default the command reads the current
`target/ripr/reports/evidence-quality-scorecard.json`, regenerating the
scorecard first only when that required input is absent. It compares against
`target/ripr/reports/evidence-quality-scorecard.previous.json` or
`target/ripr/reports/lane1-evidence-audit.previous.json` when one exists.
Operators may also pass `--current <path>` and `--previous <path>`. Missing
history is reported explicitly as `unknown`; the command does not change
analyzer behavior, gate policy, PR/CI projection, editor output, source files,
generated tests, provider calls, score definitions, or runtime execution.

If an explicit `--previous <path>` artifact is missing or malformed, the command
still writes bounded trend JSON/Markdown with `summary.status = "unknown"`,
`inputs.previous_artifact.status = "missing"` or `"malformed"`, and the named
`evidence_quality_trend_previous_artifact_unavailable` unknown instead of
exiting before producing trend evidence. Metric rows may still carry current
values, but movement and badge-readiness deltas remain unknown.

Trend JSON includes `run_status` and `runtime_status`. A limited current
scorecard is preserved as `limited_incomplete_input`; an explicit missing or
malformed previous artifact also produces a limited trend state. Missing
implicit history remains an unknown trend, not a gate or badge claim.

```json
{
  "schema_version": "0.1",
  "tool": "ripr",
  "report": "evidence-quality-trend",
  "generated_at": "unix_ms:1778620000000",
  "run_status": "full",
  "runtime_status": {
    "state": "full",
    "phase": null,
    "duration_ms": null,
    "limit_ms": null,
    "input_kind": null,
    "input_path": null,
    "limitation_category": null,
    "repair_route": null,
    "downstream_consumable": true
  },
  "scope": {
    "kind": "repo",
    "root": "."
  },
  "inputs": {
    "current_scorecard": {
      "path": "target/ripr/reports/evidence-quality-scorecard.json",
      "status": "loaded",
      "schema_version": "0.1",
      "sha256": "0123456789abcdef",
      "note": "current evidence-quality scorecard"
    },
    "previous_artifact": {
      "path": "target/ripr/reports/evidence-quality-scorecard.previous.json",
      "status": "missing",
      "schema_version": null,
      "sha256": null,
      "note": "optional previous scorecard or audit snapshot unavailable; movement is diagnostic only"
    }
  },
  "summary": {
    "status": "unknown",
    "compared_metrics": 0,
    "improved_metrics": 0,
    "regressed_metrics": 0,
    "unchanged_metrics": 0,
    "unknown_metrics": 27,
    "no_history": true
  },
  "movement_front": {
    "current_actionable_count": 926,
    "actionable_delta_since_prior_refresh": null,
    "resolved": null,
    "improved": null,
    "unchanged_after_attempt": null,
    "missing_receipts": null,
    "orphaned_receipts": null,
    "top_blocked_reason": "trend_history_unavailable",
    "receipt_linked_movement_source": "unavailable_in_evidence_quality_trend",
    "next_receipt_linked_command": "cargo xtask actionable-gap-outcomes"
  },
  "metric_trends": [
    {
      "metric": "finding_alignment_actionable_unresolved_canonical_gaps",
      "label": "Actionable canonical gaps",
      "before": null,
      "after": 926,
      "delta": null,
      "direction": "unknown",
      "interpretation": "No comparable previous value was available."
    }
  ],
  "static_limitation_category_trends": [],
  "runtime_confidence_static_only_class_trends": [
    {
      "metric": "runtime_confidence_static_only_class:call_presence",
      "label": "call_presence",
      "before": null,
      "after": 2567,
      "delta": null,
      "direction": "unknown",
      "interpretation": "No comparable previous value was available."
    }
  ],
  "unknowns": [
    {
      "kind": "trend_history_unavailable",
      "summary": "No previous scorecard or audit snapshot was available, so the report cannot claim improvement or regression.",
      "next_repair": "report/evidence-quality-trend"
    },
    {
      "kind": "evidence_quality_trend_previous_artifact_unavailable",
      "summary": "Evidence-quality trend could not load the requested previous artifact. No movement or badge-readiness delta claim is made from this limited trend.",
      "next_repair": "report/evidence-quality-trend"
    }
  ]
}
```

Field contract:

- `schema_version` - currently `"0.1"`.
- `tool` - always `"ripr"`.
- `report` - always `"evidence-quality-trend"`.
- `inputs.current_scorecard` - current scorecard artifact identity. The
  command creates the default scorecard first if it is missing.
- `inputs.previous_artifact` - optional previous scorecard or audit snapshot.
  Missing history is an explicit unknown, not a failure. Missing or malformed
  explicit previous paths are bounded unavailable-input states.
- `summary.status` - `improvement`, `regression`, `mixed`, `unchanged`, or
  `unknown`.
- `movement_front` - the first-screen movement panel. In
  `evidence-quality-trend`, `current_actionable_count` and
  `actionable_delta_since_prior_refresh` come from the scorecard trend for
  `finding_alignment_actionable_unresolved_canonical_gaps`. Receipt-linked
  `resolved`, `improved`, `unchanged_after_attempt`, `missing_receipts`, and
  `orphaned_receipts` are `null` because this report does not read receipt
  artifacts; operators should run `cargo xtask actionable-gap-outcomes` for
  receipt-linked movement. This field does not imply runtime adequacy, mutation
  proof, policy eligibility, gate passage, or merge readiness.
- `metric_trends[]` - comparable Lane 1 evidence-quality metrics with
  nullable `before`, `after`, and `delta` values plus a direction. Lower counts
  are better for debt and uncertainty metrics; higher counts are better for
  calibrated records, calibrated-supported canonical items, already-observed
  items, and internal no-action items. The first trend row is the actionable
  canonical gap count, matching the scorecard headline and keeping raw findings
  diagnostic rather than user work. Finding-alignment and presentation-text
  metrics also track raw-to-canonical quality, duplicate groups, actionability,
  static limitations, visibility unknowns, no-action/observed outcomes, and
  actionable-gap packet public-projection readiness. If the current scorecard
  carries limited input unknowns such as `lane1_evidence_audit_limited`,
  `evidence_health_limited`, or
  `evidence_quality_scorecard_audit_regeneration_failed`, metric rows remain
  present for diagnostics but their direction is `unknown` and `delta` is null.
- `static_limitation_category_trends[]` - bounded category-level deltas for
  normalized static limitation classes. Current limited scorecards also force
  these category trend directions to `unknown`.
- `runtime_confidence_static_only_class_trends[]` - bounded evidence-class
  deltas derived from `calibration_coverage.by_evidence_class[].static_only`.
  Rows make the top static-only canonical evidence classes visible so runtime
  confidence work can pick calibrated fixture expansion targets. They remain
  advisory trend evidence and do not imply mutation execution or gate authority.
  Current limited scorecards also force these class trend directions to
  `unknown`.
- `unknowns[]` - missing history or missing current metric fields that must
  stay visible until later audit or scorecard inputs exist. A
  `current_scorecard_limited` unknown means the current scorecard is itself a
  bounded diagnostic artifact, so the trend must not claim improvement or
  regression from its counts. Missing or malformed explicit previous artifacts
  are reported as `evidence_quality_trend_previous_artifact_unavailable`.

The Markdown sibling starts with a movement front section, then prints bounded
sections for summary, metric trends, static limitation category trends, runtime
confidence static-only class trends, and unknowns.

## Repo Exposure Latency Report

`cargo xtask repo-exposure-latency-report` writes a maintainer diagnostic
report to:

```text
target/ripr/reports/repo-exposure-latency.json
target/ripr/reports/repo-exposure-latency.md
```

This report is intentionally separate from `repo-exposure.json` and
`repo-exposure.md`. It can time-box the repo-exposure command path and capture
phase timing without changing analyzer classifications or public report
schemas.

```json
{
  "schema_version": "0.1",
  "tool": "ripr",
  "report": "repo-exposure-latency",
  "status": "warn",
  "timeout_ms": 30000,
  "binary": "target/debug/ripr.exe",
  "runs": [
    {
      "format": "repo-exposure-json",
      "status": "timeout",
      "duration_ms": 30082,
      "exit_code": 1,
      "stdout_bytes": 0,
      "stderr_bytes": 152,
      "trace": [
        {
          "phase": "collect_workspace_state",
          "status": "ok",
          "duration_ms": 15
        },
        {
          "phase": "cache_load",
          "status": "miss",
          "duration_ms": 0
        },
        {
          "phase": "file_fact_cache",
          "status": "hits_134_misses_0_corrupt_0_store_errors_0",
          "duration_ms": 328
        }
      ]
    }
  ]
}
```

Field contract:

- `schema_version` - currently `"0.1"` for the diagnostic report.
- `status` - `pass` when every attempted format completes successfully, `warn`
  when a format times out or a later format is skipped after timeout, and
  `fail` when a format exits unsuccessfully before timeout.
- `timeout_ms` - timeout budget per repo-exposure format. Override with
  `RIPR_REPO_EXPOSURE_LATENCY_TIMEOUT_MS`.
- `runs[].format` - `repo-exposure-json` or `repo-exposure-md`.
- `runs[].status` - `pass`, `fail`, `timeout`, or
  `skipped_after_json_timeout`.
- `runs[].trace` - analyzer trace lines captured from stderr when
  `RIPR_REPO_EXPOSURE_LATENCY_TRACE=1` is set by the xtask command. Phases
  currently include `collect_workspace_state`, `cache_load`,
  `file_fact_cache`, `apply_oracle_policy`, `inventory_seams`,
  `evidence_for_seams`, `classify_seams`, `cold_compute`, `cache_store`, and
  `total`; cache load statuses include `hit`, `miss`, and `corrupt_ignored`.
  The `file_fact_cache` status is a compact counter label such as
  `hits_134_misses_0_corrupt_0_store_errors_0`; it describes parser/file-fact
  cache reuse only, not rendered output caching.

## Targeted-Test Outcome Report

`ripr outcome --before <repo-exposure-json> --after <repo-exposure-json>`
compares two repo exposure snapshots and prints Markdown by default. Use
`--format json` for the machine-readable shape, or `--out <path>` to write the
rendered receipt to disk.

```text
ripr outcome --before before.json --after after.json
ripr outcome --before before.json --after after.json --format json
ripr outcome --before before.json --after after.json --out target/ripr/outcome/targeted-test-outcome.md
```

The report is an advisory receipt for the targeted-test loop. It does not run
analysis, mutation testing, SARIF policy, or badge generation; it only compares
the two supplied `repo-exposure-json` artifacts.

JSON shape:

```json
{
  "schema_version": "0.1",
  "tool": "ripr",
  "status": "advisory",
  "inputs": {
    "before": "target/ripr/before.json",
    "after": "target/ripr/after.json"
  },
  "before": {
    "seams_total": 15,
    "strongly_gripped": 3,
    "weakly_gripped": 9,
    "ungripped": 3
  },
  "after": {
    "seams_total": 15,
    "strongly_gripped": 5,
    "weakly_gripped": 7,
    "ungripped": 3
  },
  "summary": {
    "moved": 2,
    "unchanged": 12,
    "regressed": 0,
    "new": 0,
    "removed": 1
  },
  "moved": [
    {
      "seam_id": "67fc764ba37d77bd",
      "seam_kind": "predicate_boundary",
      "file": "src/pricing.rs",
      "line": 88,
      "before": "weakly_gripped",
      "after": "strongly_gripped",
      "direction": "improved",
      "evidence_delta": [
        "grip class moved from weakly_gripped to strongly_gripped",
        "discriminate evidence moved from missing to yes",
        "missing discriminator no longer reported: discount_threshold (equality boundary)",
        "stronger related oracle visible: weak -> strong"
      ],
      "evidence_source": "evidence_record",
      "reach_delta": null,
      "activate_delta": null,
      "propagate_delta": null,
      "observe_delta": null,
      "discriminate_delta": {
        "before_state": "missing",
        "after_state": "yes",
        "before_confidence": "high",
        "after_confidence": "high",
        "before_summary": "equality boundary not asserted",
        "after_summary": "equality boundary asserted"
      },
      "observed_values_added": ["discount_threshold"],
      "observed_values_removed": [],
      "missing_discriminators_resolved": [
        "discount_threshold (equality boundary)"
      ],
      "missing_discriminators_reopened": [],
      "oracle_strength_delta": "weak -> strong",
      "related_test_delta": 1,
      "no_movement_reason": null
    }
  ],
  "unchanged": [],
  "regressed": [],
  "new": [],
  "removed": [],
  "review_receipt": {
    "what_changed": [
      "Compared before snapshot target/ripr/before.json with after snapshot target/ripr/after.json.",
      "Static seam movement: 2 moved, 12 unchanged, 0 regressed, 0 new, 1 removed."
    ],
    "ripr_flagged_before": [
      "weakly_gripped before predicate_boundary at src/pricing.rs:88."
    ],
    "focused_proof_added": [
      "predicate_boundary at src/pricing.rs:88 shows static evidence movement for focused proof outside RIPR: missing discriminator no longer reported: discount_threshold (equality boundary); new observed value: discount_threshold."
    ],
    "movement_after_verification": [
      "2 improved, 0 changed without ranking higher, 0 regressed, 12 unchanged.",
      "predicate_boundary at src/pricing.rs:88 moved weakly_gripped -> strongly_gripped (improved)."
    ],
    "remaining_weak_or_unknown": [
      "predicate_boundary remains weakly_gripped at src/checkout.rs:41."
    ],
    "reviewer_should_inspect": [
      "Open the compared artifacts: target/ripr/before.json and target/ripr/after.json.",
      "Inspect the focused test or output proof corresponding to each listed evidence delta.",
      "Review remaining weak, unknown, new, or regressed seams before treating the repair loop as complete."
    ],
    "reviewer_may_believe": [
      "RIPR compared only the listed static snapshots: target/ripr/before.json and target/ripr/after.json.",
      "The listed focused-proof signals are static evidence visible after a test or output proof changed outside RIPR.",
      "The movement and remaining-weak sections define the static claim boundary for this receipt."
    ],
    "reviewer_should_not_believe": [
      "Runtime mutation result.",
      "Coverage adequacy.",
      "General correctness.",
      "Merge approval.",
      "That RIPR edited source or generated tests."
    ]
  }
}
```

Field contract:

- `schema_version` — currently `"0.1"`.
- `status` — always `"advisory"`; this report is a receipt, not a CI policy.
- `inputs.before` / `inputs.after` — normalized paths to the compared
  `repo-exposure-json` artifacts.
- `before` / `after` — grip-class counts computed from the supplied seams. The
  report emits `seams_total` plus every known `SeamGripClass` bucket, even when
  a bucket is zero.
- `summary` — movement bucket counts. `moved` means the seam matched by
  `seam_id` changed grip class without ranking lower; `regressed` means the
  after class ranked lower than the before class; `unchanged` means the class
  stayed the same; `new` and `removed` cover seam IDs present in only one input.
- `moved[]` / `unchanged[]` / `regressed[]` — matched seams with before/after
  grip classes, a direction string, and evidence-delta hints. When
  `seams[].evidence_record` is present, the comparison prefers that shared
  evidence spine; otherwise it falls back to legacy repo-exposure seam fields.
- `evidence_delta[]` — advisory hints such as missing discriminators no longer
  reported, new observed values, or stronger related oracles. These hints are
  based on the rendered static artifact and do not claim runtime confirmation.
- `evidence_source` — `evidence_record`, `legacy_fields`, or a mixed transition
  label when before and after snapshots differ in available evidence source.
- `reach_delta`, `activate_delta`, `propagate_delta`, `observe_delta`, and
  `discriminate_delta` — `null` when the stage is unchanged or unavailable, or
  an object with before/after state, confidence, and summary copied from the
  evidence record.
- `observed_values_added` / `observed_values_removed` — value-level activation
  evidence movement derived from the evidence record when available.
- `missing_discriminators_resolved` /
  `missing_discriminators_reopened` — discriminator-level movement derived from
  the evidence record when available.
- `oracle_strength_delta` — `null` when unchanged, otherwise a compact
  `before -> after` static oracle-strength movement label.
- `related_test_delta` — count movement for related-test evidence.
- `no_movement_reason` — explicit static reason for unchanged seams with no
  rendered evidence movement.
- `new[]` / `removed[]` — seam identity and grip class for seam IDs present in
  only one input.
- `review_receipt` — an additive reviewer packet derived from the same
  before/after movement data. It answers what changed, what RIPR flagged before
  the focused repair attempt, which static proof signals moved, what still
  remains weak or unknown, which bounded static claims reviewers may make, and
  what reviewers should inspect or avoid inferring. It does not add gate
  authority or runtime evidence beyond the compared snapshots.

The Markdown surface prints the same summary, highlights moved, unchanged,
regressed, new, and removed seams for human review, and includes a "Review
Receipt" section with the same reviewer-native fields. Unchanged seams can
still carry evidence-delta hints, such as a new observed value, so reviewers can
see when a targeted test improved rendered evidence without changing the grip
class.

## Agent Verify

`ripr agent verify --root <workspace> --before <repo-exposure-json> --after
<repo-exposure-json> --json` compares two saved static repo-exposure snapshots
under the workspace root and emits a compact agent-focused JSON summary. It
reuses the targeted-test outcome comparison engine, but names the buckets for
the active agent loop:

```text
ripr agent verify --root . --before target/ripr/workflow/before.repo-exposure.json --after target/ripr/workflow/after.repo-exposure.json --json
```

The command does not run analysis, mutation testing, SARIF policy, badge
generation, LSP refresh, or cache warm-up. It only compares the supplied
`repo-exposure-json` artifacts after validating they resolve under `--root`.

JSON shape:

```json
{
  "schema_version": "0.1",
  "tool": "ripr",
  "status": "advisory",
  "inputs": {
    "before": "target/ripr/workflow/before.repo-exposure.json",
    "after": "target/ripr/workflow/after.repo-exposure.json"
  },
  "summary": {
    "improved": 1,
    "changed": 0,
    "regressed": 0,
    "unchanged": 0,
    "new": 0,
    "resolved": 0
  },
  "changed_seams": [
    {
      "seam_id": "67fc764ba37d77bd",
      "seam_kind": "predicate_boundary",
      "file": "src/pricing.rs",
      "line": 88,
      "before": "weakly_gripped",
      "after": "strongly_gripped",
      "change": "improved",
      "evidence_delta": [
        "grip class moved from weakly_gripped to strongly_gripped",
        "missing discriminator no longer reported: discount_threshold (equality boundary)"
      ],
      "evidence_source": "evidence_record",
      "reach_delta": null,
      "activate_delta": null,
      "propagate_delta": null,
      "observe_delta": null,
      "discriminate_delta": {
        "before_state": "missing",
        "after_state": "yes",
        "before_confidence": "high",
        "after_confidence": "high",
        "before_summary": "equality boundary not asserted",
        "after_summary": "equality boundary asserted"
      },
      "observed_values_added": ["discount_threshold"],
      "observed_values_removed": [],
      "missing_discriminators_resolved": [
        "discount_threshold (equality boundary)"
      ],
      "missing_discriminators_reopened": [],
      "oracle_strength_delta": "weak -> strong",
      "related_test_delta": 1,
      "no_movement_reason": null
    }
  ],
  "unchanged_seams": [],
  "new_gaps": [],
  "resolved_gaps": []
}
```

Field contract:

- `schema_version` - currently `"0.1"`.
- `status` - always `"advisory"`; this is an agent verification hint, not a CI
  policy.
- `summary.improved` - matched seams whose after `SeamGripClass` ranks higher
  than before.
- `summary.changed` - matched seams whose class changed without ranking higher
  or lower.
- `summary.regressed` - matched seams whose after class ranks lower than
  before.
- `summary.unchanged` - matched seams whose class stayed the same.
- `summary.new` - seam IDs present only in the after snapshot.
- `summary.resolved` - seam IDs absent from the after snapshot. This is
  advisory; it can mean a gap was fixed, or that the seam disappeared because
  the code changed.
- `changed_seams[]` - improved, same-rank changed, and regressed matched seams.
- `unchanged_seams[]` - matched seams whose class stayed the same. These can
  still carry `evidence_delta` hints when rendered evidence improved without
  changing class.
- `changed_seams[]` / `unchanged_seams[]` carry the same additive
  evidence-record movement fields as `ripr outcome`: stage deltas,
  observed-value movement, missing-discriminator movement, oracle strength
  movement, related-test count movement, and `no_movement_reason`.
- `new_gaps[]` / `resolved_gaps[]` - seam identity and static class for seam IDs
  present in only one snapshot.

## Agent Receipt

`ripr agent receipt --root <workspace> --verify-json <agent-verify-json>
--seam-id <id> --json` narrows a saved `ripr agent verify` artifact to one
seam and adds optional handoff metadata for review:

```text
ripr agent receipt --root . --verify-json target/ripr/workflow/agent-verify.json --seam-id 67fc764ba37d77bd --json
ripr agent receipt --root . --verify-json target/ripr/workflow/agent-verify.json --seam-id 67fc764ba37d77bd --test discounted_total_boundary_discriminator --command "cargo test discounted_total_boundary_discriminator" --json --out target/ripr/reports/agent-receipt.json
```

The command does not run analysis, mutation testing, SARIF policy, badge
generation, LSP refresh, or cache warm-up. It reads the supplied `agent verify`
JSON after validating that path resolves under `--root`, then reads and hashes
the `inputs.before` and `inputs.after` snapshot artifacts named by the verify
JSON after validating those paths also resolve under `--root`.

JSON shape:

```json
{
  "schema_version": "0.3",
  "tool": "ripr",
  "status": "advisory",
  "inputs": {
    "agent_verify_json": "target/ripr/workflow/agent-verify.json",
    "before": "target/ripr/workflow/before.repo-exposure.json",
    "after": "target/ripr/workflow/after.repo-exposure.json"
  },
  "provenance": {
    "ripr_version": "0.7.0",
    "repo_root": ".",
    "config_fingerprint": "fnv1a64:4c94a2f6cfaa5c21",
    "command_template_version": "0.1",
    "generated_at": "unix_ms:1778179200000",
    "workflow_artifact": null,
    "before_artifact": {
      "path": "target/ripr/workflow/before.repo-exposure.json",
      "sha256": "sha256:..."
    },
    "after_artifact": {
      "path": "target/ripr/workflow/after.repo-exposure.json",
      "sha256": "sha256:..."
    },
    "verify_artifact": {
      "path": "target/ripr/workflow/agent-verify.json",
      "sha256": "sha256:..."
    },
    "seam_id": "67fc764ba37d77bd",
    "before_class": "weakly_gripped",
    "after_class": "strongly_gripped",
    "movement": "improved",
    "limits": {
      "static_artifact_relationship": true,
      "runtime_mutation_execution": false,
      "runtime_adequacy_claim": false
    }
  },
  "seam": {
    "seam_id": "67fc764ba37d77bd",
    "seam_kind": "predicate_boundary",
    "file": "src/pricing.rs",
    "line": 88,
    "before": "weakly_gripped",
    "after": "strongly_gripped",
    "grip_class": null,
    "change": "improved",
    "evidence_delta": [
      "missing discriminator no longer reported: discount_threshold (equality boundary)"
    ]
  },
  "test_changed": "discounted_total_boundary_discriminator",
  "verification": {
    "commands_run": ["cargo test discounted_total_boundary_discriminator"]
  },
  "summary": {
    "receipt_state": "receipt_movement_improved",
    "remaining_gap": "No remaining static gap is named by this receipt; inspect the current seam packet if review needs final assertion detail.",
    "next_recommendation": "Keep the focused test and attach this receipt with the agent verify JSON.",
    "next_action": {
      "kind": "improved",
      "summary": "Static grip improved.",
      "recommended_action": "Keep the focused test and include this receipt in review.",
      "safe_to_merge": false
    }
  }
}
```

Field contract:

- `schema_version` - currently `"0.3"`. Version `0.2` added receipt
  provenance fields; version `0.3` adds structured next-action guidance while
  preserving the selected-seam and handoff fields from `0.1`.
- `status` - always `"advisory"`; this is a handoff receipt, not a CI policy.
- `inputs.agent_verify_json` - the verify JSON path supplied to the command.
- `inputs.before` / `inputs.after` - snapshot paths copied from the verify JSON.
- `provenance` - identity for the static artifacts behind the receipt. It is
  produced without rerunning analysis.
- `provenance.ripr_version` - the `ripr` binary version that rendered the
  receipt.
- `provenance.repo_root` - the `--root` argument normalized to forward slashes
  for reporting.
- `provenance.config_fingerprint` - stable fingerprint of `ripr.toml` when that
  file exists under the root, or `null` when no config file is present. The
  receipt reads the file text only; it does not rerun analysis.
- `provenance.command_template_version` - version of the internal agent-loop
  command templates that produced the workflow command strings.
- `provenance.generated_at` - local render timestamp as `unix_ms:<millis>`.
- `provenance.workflow_artifact` - reserved workflow manifest artifact identity
  when a future receipt command is tied to a specific manifest. It is currently
  `null`.
- `provenance.before_artifact` / `provenance.after_artifact` /
  `provenance.verify_artifact` - path and SHA-256 hash for the static before,
  after, and verify artifacts used by the receipt.
- `provenance.seam_id` - selected seam identity copied from the receipt seam.
- `provenance.before_class` / `provenance.after_class` - static grip classes
  before and after for matched seams. For one-sided gaps, the absent side is
  `null`.
- `provenance.movement` - selected verify movement bucket such as `improved`,
  `changed`, `regressed`, `unchanged`, `new`, or `resolved`.
- `provenance.limits` - explicit static boundary flags. Receipts prove only the
  relationship between static before/after artifacts; they do not run mutation
  testing or claim runtime adequacy.
- `seam` - the selected seam from `changed_seams`, `unchanged_seams`,
  `new_gaps`, or `resolved_gaps`.
- `seam.before` / `seam.after` - before/after grip class for matched seams, or
  `null` for one-sided new/resolved gaps.
- `seam.grip_class` - one-sided grip class for `new` or `resolved` gaps, or
  `null` for matched seams.
- `test_changed` - optional focused test name supplied by the caller.
- `verification.commands_run` - optional commands supplied by the caller. The
  receipt records them; it does not run them.
- `summary.remaining_gap` / `summary.next_recommendation` - static advisory
  guidance derived from the verify bucket. It does not claim runtime
  confirmation.
- `summary.receipt_state` - canonical receipt lifecycle state for the selected
  receipt. It is one of `receipt_missing`, `receipt_found`, `receipt_stale`,
  `receipt_gap_mismatch`, `receipt_movement_improved`,
  `receipt_movement_unchanged`, or `receipt_not_applicable`.
- `summary.next_action` - structured static guidance for agents and reviewers.
  `kind` is `improved`, `changed`, `regressed`, `unchanged`, `new_gap`,
  `resolved`, or `unknown`; `summary` is a short static movement statement;
  `recommended_action` is the bounded next step; and `safe_to_merge` is always
  `false` because the static receipt is review evidence, not a merge policy.

## PR Test Guidance

RIPR-SPEC-0012 defines the pinned contract for the
`ripr review-comments` report that projects existing seam evidence into
advisory pull-request guidance:

```text
ripr review-comments \
  --root . \
  --base <sha> \
  --head <sha> \
  --out target/ripr/review/comments.json

ripr review-comments \
  --root . \
  --base <sha> \
  --head <sha> \
  --gap-ledger target/ripr/reports/gap-decision-ledger.json \
  --out target/ripr/review/comments.json
```

The command is a pure renderer. The default path joins existing static seam
evidence with the changed-line diff. When `--gap-ledger` is supplied, the
command does not rerun analysis; it renders changed-line repair cards only from
explicit `GapRecord` entries with `projection_eligibility.pr_comment.eligible =
true`, PR-local scope, a stable anchor, a dedupe fingerprint, a repair route,
and verification commands. It does not post to GitHub, run mutation testing,
refresh LSP state, edit source files, or generate tests. CI can use the JSON to
write a job summary and emit check annotations by default. Inline PR review
comments require a custom explicit opt-in publisher.

JSON shape:

```json
{
  "schema_version": "0.1",
  "tool": "ripr",
  "status": "advisory",
  "root": ".",
  "base": "origin/main",
  "head": "HEAD",
  "mode": "draft",
  "inputs": {
    "gap_ledger": "target/ripr/reports/gap-decision-ledger.json"
  },
  "limits": {
    "max_inline_comments": 3,
    "max_summary_items": 10
  },
  "summary": {
    "comments": 2,
    "summary_only": 1,
    "suppressed": 1,
    "unchanged_tests": true
  },
  "comments": [
    {
      "id": "ripr-review-67fc764ba37d77bd",
      "seam_id": "67fc764ba37d77bd",
      "dedupe_key": "ripr:67fc764ba37d77bd:src/pricing.rs:88",
      "placement": {
        "path": "src/pricing.rs",
        "line": 88,
        "side": "RIGHT",
        "mode": "exact_seam_line"
      },
      "kind": "predicate_boundary",
      "grip_class": "weakly_gripped",
      "severity": "warning",
      "reason": "Related tests reach and observe the owner but miss the equality boundary.",
      "missing_discriminator": "amount == discount_threshold",
      "suggested_test": {
        "intent": "Add an equality-boundary test.",
        "candidate_values": ["amount == discount_threshold"],
        "assertion_shape": "Assert the returned discount behavior directly.",
        "assertion_kind": "exact_value",
        "recommended_file": "tests/pricing.rs",
        "recommended_name": "discounted_total_boundary",
        "near_test": "applies_discount_above_threshold"
      },
      "llm_guidance": {
        "prompt": "Write one focused Rust test for the missing equality boundary. Place it near tests/pricing.rs::applies_discount_above_threshold. Do not change production code. Preserve existing fixture style. Verify with ripr agent verify.",
        "command": "ripr agent brief --root . --seam-id 67fc764ba37d77bd --json > target/ripr/workflow/agent-brief.json",
        "verify_command": "ripr agent verify --root . --before target/ripr/workflow/before.repo-exposure.json --after target/ripr/workflow/after.repo-exposure.json --json"
      },
      "repair_card": {
        "gap_kind": "MissingBoundaryAssertion",
        "changed_behavior": "amount == discount_threshold",
        "why_this_matters": "Changed behavior `amount == discount_threshold` has a repairable MissingBoundaryAssertion gap.",
        "repair": "Assert the returned discount behavior directly.",
        "evidence_ids": ["evidence:pricing-threshold-reached"],
        "verification_commands": ["cargo xtask fixtures boundary_gap"],
        "verify_command": "cargo xtask fixtures boundary_gap",
        "source_artifact": "target/ripr/reports/gap-decision-ledger.json",
        "authority_boundary": "gate_decision_artifact_only"
      }
    }
  ],
  "summary_only": [],
  "suppressed": [],
  "warnings": [],
  "limits_note": "Advisory static evidence only; no automatic edits, generated tests, runtime mutation execution, or CI blocking."
}
```

Field contract:

- `schema_version` - currently `"0.1"`.
- `tool` - always `"ripr"`.
- `status` - always `"advisory"`; this report is review guidance, not a CI
  policy.
- `root`, `base`, `head`, and `mode` - the workspace root, compared revisions,
  and RIPR analysis mode used to render the report.
- `inputs.gap_ledger` - optional explicit gap decision ledger used for
  repair-card projection. It is present only when `--gap-ledger` is supplied.
- `limits.max_inline_comments` - default cap for changed-line annotations.
- `limits.max_summary_items` - default cap for total recommendations.
- `summary.comments` - count of guidance items with safe changed-line
  placement.
- `summary.summary_only` - count of recommendations without safe changed-line
  placement.
- `summary.suppressed` - count hidden by configured severity, suppression,
  caps, or missing guidance.
- `summary.unchanged_tests` - `true` when selected recommendations did not have
  a nearby test change in the pull request.
- `comments[]` - line-placeable advisory recommendations. These are the only
  items eligible for check annotations or inline review comments.
- `comments[].id` - stable report-local ID derived from the seam when possible.
- `comments[].seam_id` - static seam identifier from the existing exposure or
  agent packet evidence.
- `comments[].dedupe_key` - stable key based on seam ID, path, and seam line.
- `comments[].placement` - GitHub-compatible changed-line placement. Items
  without safe placement belong in `summary_only[]`.
- `comments[].placement.mode` - `"exact_seam_line"`,
  `"owner_function_changed_line"`, or `"same_file_changed_line"`. The renderer
  must prefer summary-only guidance over misleading line placement.
- `comments[].kind` - seam kind from the existing static evidence.
- `comments[].grip_class` - seam grip class from the existing static evidence.
- `comments[].severity` - configured report severity for the recommendation.
- `comments[].reason` - short static-evidence explanation for why a focused
  test would be useful.
- `comments[].missing_discriminator` - missing value, branch, variant, or
  observation when available.
- `comments[].suggested_test` - bounded test intent, candidate values,
  assertion shape, recommended test file, and related test to imitate when
  available.
- `comments[].llm_guidance` - bounded handoff command and prompt for one
  focused test. It is not a request for free-form diff review.
- `comments[].repair_card` - optional GapRecord-backed repair card. When
  present, inline publish planning should use this field for the human/LLM
  comment body instead of raw static classes. It carries gap kind, changed
  behavior when available, why the gap matters, the bounded repair route,
  evidence IDs, verification commands, source artifact, and authority boundary.
- `summary_only[]` - same recommendation shape without `placement`. CI should
  show these in the Markdown/job summary but must not invent a changed-line
  annotation for them.
- `suppressed[]` - bounded records for recommendations hidden by caps or
  nearby test changes.
- `warnings[]` - selection warnings from the agent brief selection path.
- `limits_note` - static-evidence boundary text for downstream summaries.

Default CI projection runs `ripr review-comments` on pull requests, writes
summary items to the job summary, and emits check annotations only for changed
lines. Inline PR review comments remain opt-in through `RIPR_COMMENT_MODE`; the
generated workflow can emit a publish plan or publish safe same-repository
changed-line comments only when explicitly configured, capped, and deduped. See
[PR review guidance](PR_REVIEW_GUIDANCE.md) for the command, generated CI
behavior, placement-safe review flow, and inline-comment boundary.

## PR Inline Comment Publish Plan

RIPR-SPEC-0025 defines the optional inline comment publisher contract. The
first surface is a read-only publish plan over existing PR guidance:

```text
target/ripr/review/comment-publish-plan.json
target/ripr/review/comment-publish-plan.md
```

The plan consumes `target/ripr/review/comments.json`, optional existing RIPR
comment metadata, explicit comment mode, and permission context. The producer
does not post comments, call GitHub, rerun analysis, edit source files,
generate tests, run mutation testing, or change gate authority. Generated CI
keeps inline comments disabled by default, writes the plan in opt-in `plan` or
`inline` mode, and publishes only safe create/update operations in explicit
`inline` mode.

Producer:

```bash
ripr pr-comments plan \
  --pr-guidance target/ripr/review/comments.json \
  --existing-comments target/ripr/review/existing-comments.json \
  --mode plan \
  --out target/ripr/review/comment-publish-plan.json \
  --out-md target/ripr/review/comment-publish-plan.md
```

JSON shape:

```json
{
  "schema_version": "0.1",
  "tool": "ripr",
  "kind": "pr_inline_comment_publish_plan",
  "status": "advisory",
  "root": ".",
  "generated_at": "2026-05-10T12:00:00Z",
  "mode": "plan",
  "inputs": {
    "pr_guidance": "target/ripr/review/comments.json",
    "existing_comments": "target/ripr/review/existing-comments.json",
    "pull_request": 123,
    "event_name": "pull_request",
    "head_repo": "EffortlessMetrics/ripr",
    "base_repo": "EffortlessMetrics/ripr"
  },
  "limits": {
    "max_inline_comments": 3,
    "advisory": true,
    "comments_default": "off"
  },
  "summary": {
    "guidance_comments": 4,
    "summary_only": 1,
    "suppressed": 1,
    "publishable": 3,
    "planned_create": 2,
    "planned_update": 1,
    "planned_keep": 0,
    "planned_delete": 0,
    "skipped": 2,
    "blocked": 0,
    "safe_to_publish": false
  },
  "operations": [
    {
      "operation": "create",
      "safe_to_publish": false,
      "dry_run": true,
      "source_collection": "comments",
      "source_id": "ripr-review-67fc764ba37d77bd",
      "dedupe_key": "ripr:67fc764ba37d77bd:src/pricing.rs:88",
      "placement": {
        "path": "src/pricing.rs",
        "line": 88,
        "side": "RIGHT",
        "mode": "exact_seam_line"
      },
      "body": "RIPR advisory: static evidence says this seam misses `amount == discount_threshold`. Add one focused boundary assertion and verify with `ripr agent verify`.",
      "existing_comment_id": null,
      "skip_reason": null,
      "blocked_reason": null
    }
  ],
  "skipped": [
    {
      "source_collection": "summary_only",
      "source_id": "ripr-review-summary-1",
      "dedupe_key": "ripr:summary:67fc764ba37d77bd",
      "skip_reason": "summary_only",
      "message": "Summary-only guidance is visible in comments.md but is not eligible for inline publishing."
    }
  ],
  "blocked": [],
  "warnings": [],
  "limits_note": "Advisory inline-comment publish plan only; default workflows do not post comments, summary-only guidance is never published inline, and gate decisions remain separate."
}
```

Field contract:

- `schema_version` is `0.1` until the plan shape changes.
- `kind` is always `pr_inline_comment_publish_plan`.
- `status` is `advisory`; this report is not gate authority.
- `mode` is `off`, `plan`, or `inline`; generated CI defaults to `off`.
- `inputs.pr_guidance` records the explicit `review-comments` source.
- `inputs.existing_comments` records optional existing-comment metadata when
  supplied.
- `inputs.event_name`, `inputs.head_repo`, and `inputs.base_repo` preserve the
  permission context used for plan decisions.
- `limits.max_inline_comments` defaults to three.
- `limits.comments_default` is `off`.
- `summary.guidance_comments`, `summary.summary_only`, and
  `summary.suppressed` mirror the input collections when available.
- `summary.publishable` counts only `comments[]` items eligible under placement,
  cap, permission, and dedupe rules.
- `summary.safe_to_publish` is `true` only when mode is `inline` and every
  planned publishing operation satisfies the permission boundary.
- `operations[]` records publishable operations sourced from `comments[]` and
  stale existing-comment cleanup candidates.
- `operations[].operation` is `create`, `update`, `keep`, or `delete`.
- `operations[].source_collection` must be `comments` for publishable
  operations.
- `operations[].dedupe_key` is required for `create`, `update`, `keep`, and
  `delete`.
- `operations[].placement` is copied from the source `comments[]` item; the
  plan must not invent placement.
- `operations[].body` preserves advisory static-evidence language and must not
  claim runtime mutation results.
- `skipped[]` records capped, summary-only, suppressed, disabled, and
  already-current items.
- `skip_reason` is `mode_off`, `summary_only`, `suppressed`, `cap_reached`,
  `unchanged_tests`, `not_publishable`, or `already_current`.
- `blocked[]` records hard safety blockers such as missing permissions,
  untrusted forks, missing PR context, unsafe events, missing dedupe keys, or
  malformed inputs.
- `blocked_reason` is `missing_pr_guidance`, `malformed_pr_guidance`,
  `missing_pull_request`, `missing_token`, `missing_write_permission`,
  `fork_untrusted`, `unsafe_event`, `missing_dedupe_key`,
  `missing_changed_line_placement`, `unsupported_mode`, or `unknown`.
- `warnings[]` records malformed optional inputs, stale existing comments,
  unsupported optional metadata, and other non-authoritative context.
- `limits_note` preserves the default-off, advisory, no-summary-only-inline,
  and separate-gate-authority boundaries.

Existing-comment metadata may be supplied to plan upsert behavior:

```json
{
  "schema_version": "0.1",
  "tool": "ripr",
  "kind": "pr_inline_comment_existing_comments",
  "comments": [
    {
      "comment_id": 987654321,
      "dedupe_key": "ripr:67fc764ba37d77bd:src/pricing.rs:88",
      "path": "src/pricing.rs",
      "line": 88,
      "side": "RIGHT",
      "body_hash": "sha256:...",
      "outdated": false
    }
  ]
}
```

Generated CI may upload and summarize the publish plan only when explicit
configuration requests `plan` or `inline` mode. The default remains job summary,
check annotations, and uploaded artifacts without durable PR comments.

## Recommendation Calibration Report

RIPR-SPEC-0013 defines the recommendation calibration report contract.
The report measures whether existing PR guidance was useful, correctly placed,
properly suppressed or capped, aimed at the expected test target, and
correlated with later static evidence movement.

The repo-local report command is:

```text
cargo xtask recommendation-calibration \
  --root . \
  --pr-guidance target/ripr/review/comments.json \
  --calibration-expectations fixtures/boundary_gap/expected/recommendation-calibration/expectations.json \
  --outcome-receipts fixtures/boundary_gap/expected/recommendation-calibration/outcome-receipts \
  --agent-receipt target/ripr/reports/agent-receipt.json \
  --targeted-test-outcome target/ripr/outcome/targeted-test-outcome.json \
  --out target/ripr/reports/recommendation-calibration.json
```

The report writes:

```text
target/ripr/reports/recommendation-calibration.json
target/ripr/reports/recommendation-calibration.md
```

See [Recommendation calibration](RECOMMENDATION_CALIBRATION.md) for the
operator workflow and metric interpretation guide.

This is an advisory calibration surface. It does not call LLM providers, edit
source files, generate tests, run mutation testing, post comments, or make CI
blocking by default.

JSON shape:

```json
{
  "schema_version": "0.1",
  "tool": "ripr",
  "status": "advisory",
  "root": ".",
  "inputs": {
    "pr_guidance": ["target/ripr/review/comments.json"],
    "agent_receipt": "target/ripr/reports/agent-receipt.json",
    "targeted_test_outcome": "target/ripr/outcome/targeted-test-outcome.json",
    "calibration_expectations": "fixtures/boundary_gap/expected/recommendation-calibration/expectations.json",
    "outcome_receipts": [
      "fixtures/boundary_gap/expected/recommendation-calibration/outcome-receipts/useful.json"
    ]
  },
  "summary": {
    "recommendations_evaluated": 4,
    "top_recommendation_outcome": "useful",
    "useful": 2,
    "noisy": 1,
    "false_annotations": 1,
    "summary_only_correct": 1,
    "suppressed_correctly": 1,
    "target_file_correct": 2,
    "static_improved": 1,
    "static_unchanged": 1,
    "static_regressed": 0,
    "unknown": 1
  },
  "latency": {
    "guidance_generated_unix_ms": 1778240000000,
    "annotation_emitted_unix_ms": 1778240001200,
    "outcome_recorded_unix_ms": 1778240100000,
    "annotation_latency_ms": 1200,
    "outcome_latency_ms": 100000
  },
  "recommendations": [
    {
      "id": "ripr-review-67fc764ba37d77bd",
      "seam_id": "67fc764ba37d77bd",
      "rank": 1,
      "source": "comments",
      "source_artifact": "target/ripr/review/comments.json",
      "source_case": "useful_exact_line_boundary",
      "placement": {
        "path": "src/pricing.rs",
        "line": 88,
        "mode": "exact_seam_line",
        "quality": "correct"
      },
      "grip_class": "weakly_gripped",
      "severity": "warning",
      "missing_discriminator": "amount == discount_threshold",
      "suggested_test": {
        "recommended_file": "tests/pricing.rs",
        "near_test": "applies_discount_above_threshold",
        "target_quality": "correct",
        "expected_file": "tests/pricing.rs"
      },
      "calibration": {
        "outcome": "useful",
        "source": "outcome_receipt:fixture",
        "reason": "expected equality-boundary recommendation on the changed seam"
      },
      "static_movement": {
        "state": "improved",
        "source": "outcome_receipt",
        "before_class": null,
        "after_class": null
      }
    }
  ],
  "suppressed": [
    {
      "id": "ripr-review-capped-1",
      "seam_id": "67fc764ba37d77bd",
      "source_artifact": "target/ripr/review/comments.json",
      "source_case": "configured_off_boundary",
      "reason": "cap_reached",
      "quality": "suppressed_correctly",
      "calibration": {
        "outcome": "suppressed_correctly",
        "source": "fixture_expectation",
        "reason": "expected cap suppression"
      }
    }
  ],
  "warnings": [],
  "limits_note": "Advisory recommendation-quality evidence only; no telemetry, generated tests, source edits, runtime execution, or CI blocking."
}
```

Field contract:

- `schema_version` - currently `"0.1"`.
- `tool` - always `"ripr"`.
- `status` - `advisory` when at least one recommendation can be evaluated and
  `incomplete` when required inputs are missing. Malformed required inputs
  return an actionable command error instead of a successful report.
- `root` - workspace root used to resolve artifact paths.
- `inputs` - paths and receipt lists considered by the report. Missing optional
  inputs should be visible as `null`, empty arrays, or warnings.
- `summary.recommendations_evaluated` - count of visible and suppressed
  recommendations considered.
- `summary.top_recommendation_outcome` - outcome label for the highest-ranked
  recommendation, or `unknown`.
- `summary.useful`, `summary.noisy`, `summary.false_annotations`,
  `summary.summary_only_correct`, `summary.suppressed_correctly`,
  `summary.target_file_correct`, `summary.static_improved`,
  `summary.static_unchanged`, `summary.static_regressed`, and
  `summary.unknown` - aggregate quality and static-movement counts.
- `latency.*_unix_ms` - optional timestamps from artifacts or CI-provided
  metadata. Values are `null` when timestamps are unavailable.
- `latency.annotation_latency_ms` - elapsed time from guidance generation to
  annotation emission when both timestamps are available.
- `latency.outcome_latency_ms` - elapsed time from guidance generation to the
  first matching outcome or receipt timestamp when available.
- `recommendations[]` - calibrated records for visible PR guidance items.
- `recommendations[].rank` - ranking from the source guidance.
- `recommendations[].placement.quality` - `correct`, `wrong_line`,
  `summary_only_expected`, `not_placeable`, or `unknown`.
  `summary_only_expected` is a placement-quality value only; the matching
  review-quality outcome remains `summary_only_correct`.
- `recommendations[].suggested_test.target_quality` - `correct`,
  `wrong_target`, `not_applicable`, or `unknown`.
- `recommendations[].calibration.outcome` - `useful`, `noisy`, `wrong_line`,
  `already_covered`, `wrong_target`, `summary_only_correct`,
  `suppressed_correctly`, or `unknown`.
- `recommendations[].calibration.source` - `fixture_expectation` or
  `outcome_receipt:<source>`.
- `recommendations[].static_movement.state` - `improved`, `unchanged`,
  `regressed`, `resolved`, `new_gap`, `missing_after_snapshot`, or `unknown`.
- `suppressed[]` - recommendations hidden by caps, suppression, configured-off
  severity, generated/migration exclusion, or nearby-test change.
- `suppressed[].reason` - stable reason code: `cap_reached`, `suppression`,
  `severity_off`, `nearby_test_changed`, `generated_or_migration`, or
  `unknown`.
- `suppressed[].quality` - `suppressed_correctly`, `over_suppressed`, or
  `unknown`.
- `warnings[]` - missing inputs, unsupported expectation fields, stale
  artifacts, or latency values that could not be derived.
- `limits_note` - advisory boundary text for summaries and generated CI.

## Calibrated Gate Decision

RIPR-SPEC-0014 defines the optional calibrated gate policy contract. The gate
decision report is read-only policy over existing repo exposure, PR guidance,
GapRecord decisions, SARIF policy, labels, receipts, recommendation
calibration, and optional imported mutation calibration artifacts.

The evaluator is:

```text
ripr gate evaluate \
  --root . \
  --repo-exposure target/ripr/reports/repo-exposure.json \
  --pr-guidance target/ripr/review/comments.json \
  --gap-ledger target/ripr/reports/gap-decision-ledger.json \
  --sarif-policy target/ripr/reports/sarif-policy.json \
  --labels-json target/ci/labels.json \
  --agent-verify target/ripr/workflow/agent-verify.json \
  --agent-receipt target/ripr/reports/agent-receipt.json \
  --recommendation-calibration target/ripr/reports/recommendation-calibration.json \
  --mutation-calibration target/ripr/reports/mutation-calibration.json \
  --mode visible-only \
  --out target/ripr/reports/gate-decision.json \
  --out-md target/ripr/reports/gate-decision.md
```

The report writes:

```text
target/ripr/reports/gate-decision.json
target/ripr/reports/gate-decision.md
```

This is an optional policy surface. Generated workflows remain advisory and
non-blocking by default. When `--gap-ledger` is supplied, gate candidates come
from explicit GapRecord `projection_eligibility.gate_candidate` records that
satisfy the safe gate predicate; PR guidance remains the legacy/fallback input.
The evaluator writes JSON and Markdown before returning a non-zero exit for
`blocked` or `config_error` decisions. It must not post comments, edit source
files, generate tests, run mutation testing, upload SARIF, mutate GitHub state,
or hide acknowledged decisions.

Generated CI runs this evaluator only when `RIPR_GATE_MODE` is explicitly set.
The default generated workflow leaves `RIPR_GATE_MODE` empty, captures pull
request labels to `target/ci/labels.json`, and uploads any gate-decision files
with the regular RIPR artifact packet. When `gate-decision.json` and
`RIPR_GATE_BASELINE` are available, generated CI also runs
`ripr baseline diff` as a non-blocking movement report and uploads
`baseline-debt-delta.{json,md}` with the same artifact packet. The job summary
renders an at-a-glance projection of mode, status, decision counts, labels,
waiver, baseline, calibration, blocking reason, debt movement counts, and
artifact paths before appending the full Markdown decision report.
`visible-only` remains advisory; blocking modes are opt-in.

See [Calibrated gate policy](CALIBRATED_GATE_POLICY.md) for the operating
model, rollout path, waiver behavior, and static/runtime vocabulary boundary.

JSON shape:

```json
{
  "schema_version": "0.1",
  "tool": "ripr",
  "status": "acknowledged",
  "mode": "acknowledgeable",
  "root": ".",
  "inputs": {
    "repo_exposure": "target/ripr/reports/repo-exposure.json",
    "pr_guidance": "target/ripr/review/comments.json",
    "gap_ledger": "target/ripr/reports/gap-decision-ledger.json",
    "sarif_policy": "target/ripr/reports/sarif-policy.json",
    "labels_json": "target/ci/labels.json",
    "labels": ["ripr-waive"],
    "agent_verify": "target/ripr/workflow/agent-verify.json",
    "agent_receipt": "target/ripr/reports/agent-receipt.json",
    "recommendation_calibration": "target/ripr/reports/recommendation-calibration.json",
    "mutation_calibration": null,
    "baseline": null
  },
  "policy": {
    "mode": "acknowledgeable",
    "threshold": "high_confidence_new_gap",
    "acknowledgement_labels": ["ripr-waive"],
    "default_workflow_posture": "advisory"
  },
  "summary": {
    "evaluated": 2,
    "blocking": 0,
    "acknowledged": 1,
    "advisory": 1,
    "suppressed": 0,
    "not_applicable": 0,
    "unknown_confidence": 0
  },
  "decisions": [
    {
      "id": "ripr-gate-67fc764ba37d77bd",
      "source": "gap_decision_ledger",
      "decision": "acknowledged",
      "gate_reason": "policy-eligible gap acknowledged by ripr-waive",
      "gap_id": "gap:pricing",
      "gap_kind": "MissingBoundaryAssertion",
      "seam_id": "67fc764ba37d77bd",
      "source_id": "ripr-review-67fc764ba37d77bd",
      "static_class": "weakly_gripped",
      "severity": "warning",
      "placement": {
        "path": "src/pricing.rs",
        "line": 88
      },
      "policy": {
        "mode": "acknowledgeable",
        "threshold": "high_confidence_new_gap",
        "acknowledgement_label": "ripr-waive",
        "baseline_identity": null
      },
      "evidence": {
        "missing_discriminator": "amount == discount_threshold",
        "assertion_shape": "Assert the returned discount behavior directly.",
        "candidate_values": ["amount == discount_threshold"],
        "recommended_test": "tests/pricing.rs::discounted_total_boundary",
        "repair_route": {
          "route_kind": "AddBoundaryAssertion",
          "target_file": "tests/pricing.rs",
          "related_test": "tests/pricing.rs::discounted_total_boundary",
          "assertion_shape": "Assert the returned discount behavior directly.",
          "changed_behavior": "amount == discount_threshold"
        },
        "verification_commands": ["cargo xtask fixtures boundary_gap"],
        "nearby_test_changed": false,
        "suppressed": false,
        "configured_off": false,
        "recommendation_calibration": {
          "available": true,
          "outcome": "useful",
          "confidence_effect": "supports_static_gap"
        },
        "mutation_calibration": {
          "available": false,
          "confidence_effect": "not_used"
        }
      }
    }
  ],
  "warnings": [],
  "limits_note": "Optional policy over static RIPR evidence; advisory by default; runtime mutation calibration is used only when supplied."
}
```

Field contract:

- `schema_version` - currently `"0.1"`.
- `status` - one of `pass`, `advisory`, `acknowledged`, `blocked`, or
  `config_error`.
- `mode` - one of `visible-only`, `acknowledgeable`, `baseline-check`, or
  `calibrated-gate`.
- `inputs` - normalized paths and labels used by the evaluator. Optional inputs
  should appear as `null` or produce a warning when they are absent.
- `inputs.gap_ledger` - optional explicit gap decision ledger input. When
  supplied, gate candidates come from GapRecord gate-candidate projection
  targets instead of raw PR guidance candidates.
- `policy.mode` - effective gate mode after config and CLI precedence.
- `policy.threshold` - initially `high_confidence_new_gap`.
- `policy.acknowledgement_labels` - configured labels that can turn a blocking
  candidate into a visible acknowledged decision.
- `policy.default_workflow_posture` - must remain `advisory` for generated
  workflows unless a later explicit configuration changes it.
- `summary.evaluated` - candidate count considered after parsing inputs.
- `summary.blocking` - count of candidate decisions that make the gate fail.
- `summary.acknowledged` - count of candidate decisions made non-failing by an
  acknowledgement label.
- `summary.advisory` - count of visible non-blocking decisions.
- `summary.suppressed` - count of suppressed or configured-hidden candidates
  preserved in the gate report.
- `summary.not_applicable` - count of parsed records that are outside the
  configured policy scope.
- `summary.unknown_confidence` - count of candidates that could not satisfy
  high-confidence requirements.
- `decisions[].source` - source artifact family such as `pr_guidance`,
  `gap_decision_ledger`, `repo_exposure`, `sarif_policy`, or `agent_receipt`.
- `decisions[].gap_id` and `decisions[].gap_kind` - optional GapRecord identity
  and repair-problem kind copied when the decision came from a gap ledger.
- `decisions[].canonical_gap_id` - optional semantic Lane 1 gap identity copied
  from the source candidate when supplied directly, through
  `identity.canonical_gap_id`, or through `evidence_record.canonical_gap_id`.
  The field is omitted when unavailable.
- `decisions[].decision` - one of `blocking`, `acknowledged`, `advisory`,
  `suppressed`, or `not_applicable`.
- `decisions[].gate_reason` - short policy explanation for human summaries.
- `decisions[].static_class` - source static class copied without rewriting
  seam-grip classes into finding classes.
- `decisions[].severity` - configured severity from the source surface.
- `decisions[].policy` - mode, threshold, acknowledgement, and baseline facts
  that affected the candidate.
- `decisions[].policy.baseline_identity` - identity used for baseline
  comparison. Matching prefers `canonical_gap_id` before legacy seam, source,
  ID, dedupe-key, and path/line/static-class fallback identities.
- `decisions[].evidence` - static evidence and optional calibration confidence
  effects used for the candidate.
- `decisions[].evidence.repair_route` and `verification_commands` - optional
  GapRecord repair route and verification commands. These fields are present
  when the gate decision is driven by a repairable gap ledger record.
- `warnings[]` - missing optional inputs, unsupported labels, ambiguous
  calibration, baseline limitations, or schema limitations.
- `limits_note` - static/runtime and advisory-default boundary text.

Markdown should fit in a job summary. It should name the top-level decision,
mode, counts, blocking or acknowledged seams, repair action, and limits. It
must not hide acknowledged decisions. If the evaluator returns a blocking exit
code, the Markdown still needs enough evidence for the next agent or reviewer
to resolve the state.

## Gate Baseline Ledger

`ripr baseline create` writes the first executable Campaign 17 baseline ledger.
It turns an existing gate-decision JSON report into a stable historical-debt
baseline that can be reviewed, checked in, diffed against current evidence, and
shrink-only refreshed after resolved debt is reviewed.

Command:

```text
ripr baseline create \
  --from target/ripr/reports/gate-decision.json \
  --out .ripr/gate-baseline.json
```

Options:

- `--from` - required gate-decision JSON from `ripr gate evaluate`.
- `--out` - baseline path. Defaults to `.ripr/gate-baseline.json`.
- `--dry-run` - print the baseline JSON without writing.
- `--force` - overwrite an existing baseline path.

The command includes `advisory`, `acknowledged`, and `blocking` gate decisions.
It skips `suppressed`, configured-off, `not_applicable`, and malformed
decisions. The command refuses to overwrite an existing baseline unless
`--force` is supplied. It does not run analysis, change gate policy, edit
source, generate tests, run mutation testing, or make CI blocking by default.

JSON shape:

```json
{
  "schema_version": "0.1",
  "tool": "ripr",
  "kind": "gate_baseline",
  "created_at": "unix_ms:1778277000000",
  "source_report": "target/ripr/reports/gate-decision.json",
  "mode": "baseline-check",
  "reviewed": false,
  "summary": {
    "entries": 1,
    "included": 1,
    "skipped": {
      "suppressed": 0,
      "not_applicable": 0,
      "malformed": 0,
      "other": 0
    }
  },
  "entries": [
    {
      "identity": {
        "canonical_gap_id": "pricing::discount::threshold_equality",
        "seam_id": "67fc764ba37d77bd",
        "source_id": "ripr-review-67fc764ba37d77bd",
        "id": "ripr-gate-67fc764ba37d77bd",
        "dedupe_key": null,
        "fallback": "src/pricing.rs:88:weakly_gripped"
      },
      "path": "src/pricing.rs",
      "line": 88,
      "static_class": "weakly_gripped",
      "decision": "advisory",
      "severity": "warning",
      "source": "pr_guidance",
      "gate_reason": "visible-only mode records evidence without blocking",
      "evidence": {
        "missing_discriminator": "amount == discount_threshold",
        "assertion_shape": "Assert returned discount behavior directly.",
        "recommended_test": "tests/pricing.rs::applies_discount_above_threshold"
      },
      "review": {
        "reviewed": false,
        "owner": null,
        "reason": "initial adoption baseline",
        "created_at": "unix_ms:1778277000000",
        "review_after": null,
        "source": "target/ripr/reports/gate-decision.json"
      }
    }
  ],
  "warnings": [],
  "limits_note": "Reviewed baseline debt ledger over static RIPR gate evidence; baselines are not suppressions and do not change gate policy by themselves."
}
```

The gate evaluator accepts this `entries[]` ledger shape in addition to the
older lightweight `decisions[]` baseline fixture shape, so newly created
baselines can be used by `ripr gate evaluate --baseline`.

The `entries[].review` object is additive review metadata for later RIPR Zero
reporting. New ledgers include `owner`, `created_at`, `review_after`, and
`source` fields alongside the original `reviewed` and `reason` fields. Older
Campaign 17 ledgers that only contain `reviewed` and `reason`, or no entry
review object at all, remain valid inputs for baseline diff and shrink-only
update.

## Gate Baseline Update

`ripr baseline update --remove-resolved` refreshes a reviewed baseline ledger in
shrink-only mode. It removes reviewed baseline entries that are absent from the
current gate-decision evidence and never adopts new current debt.

Command:

```text
ripr baseline update \
  --baseline .ripr/gate-baseline.json \
  --current target/ripr/reports/gate-decision.json \
  --remove-resolved \
  --out .ripr/gate-baseline.json
```

Options:

- `--baseline` - reviewed baseline ledger to refresh.
- `--current` - current gate-decision JSON from `ripr gate evaluate`.
- `--remove-resolved` - required shrink-only mode.
- `--out` - updated baseline path. Defaults to `--baseline`.

The update command hard-fails if either input is missing or unsupported. It
preserves malformed entries without stable identity and ambiguous matches for
manual review. New current gate decisions are counted as ignored and are not
inserted into the baseline. Generated CI must not use this command to rewrite
checked-in baselines automatically.

The output is the same `kind = "gate_baseline"` ledger with updated `entries`
and `summary` counts plus an additive update receipt:

```json
{
  "schema_version": "0.1",
  "tool": "ripr",
  "kind": "gate_baseline",
  "summary": {
    "entries": 40,
    "included": 39,
    "skipped": {
      "malformed": 1
    }
  },
  "entries": [],
  "update": {
    "remove_resolved": true,
    "current_gate_decision": "target/ripr/reports/gate-decision.json",
    "removed_resolved": 7,
    "ignored_new_current": 2
  },
  "warnings": [
    "preserved malformed baseline entry without stable identity during shrink-only update"
  ],
  "limits_note": "Shrink-only baseline refresh over static RIPR gate evidence; update removes resolved reviewed debt and never adopts new current debt."
}
```

## Baseline Debt Delta Report

RIPR-SPEC-0016 defines the baseline debt delta report. The report compares an
explicit reviewed baseline ledger with current gate-decision evidence so teams
can see existing, resolved, new, acknowledged, suppressed, stale, invalid, and
missing-input behavioral-grip debt without making the report a gate.

Command:

```text
ripr baseline diff \
  --baseline .ripr/gate-baseline.json \
  --current target/ripr/reports/gate-decision.json \
  --out target/ripr/reports/baseline-debt-delta.json \
  --out-md target/ripr/reports/baseline-debt-delta.md
```

The report writes:

```text
target/ripr/reports/baseline-debt-delta.json
target/ripr/reports/baseline-debt-delta.md
```

This report is advisory movement evidence. `ripr gate evaluate` remains the
pass/fail authority for configured gate modes. Generated CI uploads and
summarizes the delta report when `RIPR_GATE_BASELINE` is set and
`gate-decision.json` exists, but the report itself must not fail CI, rewrite a
baseline, post comments, edit source, generate tests, rerun analysis, or run
mutation testing.

JSON shape:

```json
{
  "schema_version": "0.1",
  "tool": "ripr",
  "kind": "baseline_debt_delta",
  "status": "advisory",
  "root": ".",
  "inputs": {
    "baseline": ".ripr/gate-baseline.json",
    "current_gate_decision": "target/ripr/reports/gate-decision.json",
    "pr_guidance": null,
    "agent_receipt": null
  },
  "baseline": {
    "path": ".ripr/gate-baseline.json",
    "schema_version": "0.1",
    "entries": 47,
    "valid": 46,
    "stale": 1,
    "invalid": 0
  },
  "delta": {
    "still_present": 40,
    "resolved": 7,
    "new_policy_eligible": 2,
    "acknowledged": 1,
    "suppressed": 0,
    "stale_baseline_entry": 1,
    "invalid_baseline_entry": 0,
    "missing_current_input": 0
  },
  "items": [
    {
      "bucket": "new_policy_eligible",
      "identity": {
        "canonical_gap_id": "pricing::discount::threshold_equality",
        "seam_id": "67fc764ba37d77bd",
        "source_id": "ripr-review-67fc764ba37d77bd",
        "id": "ripr-gate-67fc764ba37d77bd",
        "dedupe_key": null,
        "fallback": "src/pricing.rs:88:weakly_gripped",
        "matched_by": "canonical_gap_id"
      },
      "path": "src/pricing.rs",
      "line": 88,
      "static_class": "weakly_gripped",
      "decision": "blocking",
      "reason": "Current policy-eligible gap is not present in the reviewed baseline.",
      "missing_discriminator": "amount == discount_threshold",
      "suggested_test": {
        "recommended_test": "tests/pricing.rs::applies_discount_above_threshold",
        "assertion_shape": "Assert returned discount behavior directly."
      },
      "repair": {
        "action": "add_focused_test_or_acknowledge",
        "verify_command": "ripr agent verify --root . --before target/ripr/pilot/repo-exposure.json --after target/ripr/pilot/after.repo-exposure.json --json"
      },
      "review": null
    }
  ],
  "warnings": [],
  "limits_note": "Advisory baseline debt movement over static RIPR gate evidence; pass/fail remains owned by ripr gate evaluate."
}
```

Field contract:

- `schema_version` - currently `"0.1"`.
- `status` - currently `advisory`. The report does not return `blocked`;
  blocking belongs to the gate evaluator.
- `inputs.baseline` - explicit baseline ledger path. Missing or unreadable
  required baseline input produces a repair-oriented warning and a
  `missing_current_input` item instead of treating the baseline as resolved.
- `inputs.current_gate_decision` - current gate-decision JSON path.
- `inputs.pr_guidance`, `inputs.agent_receipt`, and other optional inputs -
  used only to enrich repair context when supplied.
- `baseline.entries` - parsed baseline entry count.
- `baseline.valid`, `baseline.stale`, and `baseline.invalid` - baseline record
  health counts after current comparison.
- `delta.still_present` - baseline identities present in current evidence.
- `delta.resolved` - baseline identities absent from current evidence.
- `delta.new_policy_eligible` - current policy-eligible identities absent from
  the baseline.
- `delta.acknowledged` - current visible findings acknowledged by label or
  policy.
- `delta.suppressed` - current findings hidden by suppression or configured-off
  severity while remaining visible in the delta report.
- `delta.stale_baseline_entry` - baseline records that parse but cannot join
  cleanly because the identity is ambiguous, obsolete, or incompatible.
- `delta.invalid_baseline_entry` - malformed baseline records or records
  missing required identity fields.
- `delta.missing_current_input` - records whose movement cannot be classified
  because required current artifacts are missing or unreadable.
- `items[].bucket` - one primary bucket from the `delta` object.
- `items[].identity` - stable identity fields. Matching order is
  `canonical_gap_id`, `seam_id`, `source_id`, `id`, `dedupe_key`, then
  normalized path, line, and static class fallback. `canonical_gap_id` may be
  supplied directly, through `identity.canonical_gap_id`, or through
  `evidence_record.canonical_gap_id`; when absent it remains `null` for
  backward-compatible ledgers.
- `items[].identity.matched_by` - the identity selector that joined baseline
  and current records.
- `items[].repair` - focused repair context from the current gate decision and
  built-in baseline debt movement actions.
- `items[].review` - optional reviewed baseline metadata copied from the
  baseline ledger when the item is baseline-derived. It preserves
  `reviewed`, `owner`, `reason`, `created_at`, `review_after`, and `source`
  when present. Current-only items use `null`; older Campaign 17 entries with
  only `reviewed` and `reason` remain valid and render missing metadata fields
  as `null`.
- `warnings[]` - malformed baseline entries, ambiguous matches, fallback
  matches, missing optional inputs, or unsupported schema versions.
- `limits_note` - advisory boundary text for generated CI summaries.

Markdown should fit in a generated CI job summary. It should include the
baseline path, status, bucket counts, top new policy-eligible gaps, top resolved
baseline entries, warnings, and the advisory boundary. It must distinguish
baseline debt from suppressions and acknowledged current findings from hidden
success.

## RIPR Zero Status Report

RIPR-SPEC-0017 defines the RIPR Zero status report. `ripr zero status` joins
existing baseline ledgers, baseline debt deltas, gate decisions, PR guidance,
gap decision ledgers, and optional calibration or receipt artifacts so teams
can see repo-level movement toward RIPR 0 without changing analyzer identity,
gate policy, or advisory defaults.

Command:

```text
ripr zero status \
  --baseline .ripr/gate-baseline.json \
  --delta target/ripr/reports/baseline-debt-delta.json \
  --gap-ledger target/ripr/reports/gap-decision-ledger.json \
  --gate target/ripr/reports/gate-decision.json \
  --pr-guidance target/ripr/review/comments.json \
  --recommendation-calibration target/ripr/reports/recommendation-calibration.json \
  --out target/ripr/reports/ripr-zero-status.json \
  --out-md target/ripr/reports/ripr-zero-status.md
```

The report writes:

```text
target/ripr/reports/ripr-zero-status.json
target/ripr/reports/ripr-zero-status.md
```

This report is advisory progress evidence. `ripr gate evaluate` remains the
pass/fail authority for configured gate modes. Generated CI uploads and
summarizes `ripr-zero-status.{json,md}` when `baseline-debt-delta.json` exists,
but the report itself must not fail CI, rewrite baselines, post comments, edit
source, generate tests, rerun analysis, call an LLM, or run mutation testing.

JSON shape:

```json
{
  "schema_version": "0.1",
  "tool": "ripr",
  "kind": "ripr_zero_status",
  "status": "advisory",
  "root": ".",
  "generated_at": "2026-05-08T00:00:00Z",
  "inputs": {
    "baseline": ".ripr/gate-baseline.json",
    "baseline_debt_delta": "target/ripr/reports/baseline-debt-delta.json",
    "gap_decision_ledger": "target/ripr/reports/gap-decision-ledger.json",
    "gate_decision": "target/ripr/reports/gate-decision.json",
    "pr_guidance": "target/ripr/review/comments.json",
    "recommendation_calibration": null,
    "previous_status": null
  },
  "ripr_zero": {
    "state": "not_yet",
    "target_source": "gap_decision_ledger",
    "visible_unresolved": 43,
    "new_policy_eligible": 1,
    "blocking_candidates": 0,
    "acknowledged": 1,
    "suppressed": 0,
    "limits_note": "RIPR 0 means no visible unresolved behavioral test-grip gaps under configured scope and policy; it is not a coverage or runtime adequacy claim."
  },
  "baseline": {
    "path": ".ripr/gate-baseline.json",
    "entries": 47,
    "still_present": 40,
    "resolved": 7,
    "age_days": 31,
    "metadata": {
      "current": 38,
      "stale": 4,
      "missing_metadata": 5,
      "unknown": 0
    }
  },
  "debt_delta": {
    "still_present": 40,
    "resolved": 7,
    "new": 2,
    "new_policy_eligible": 1,
    "acknowledged": 1,
    "suppressed": 0,
    "stale": 4,
    "invalid": 0,
    "missing_input": 0
  },
  "trend": {
    "source": "not_available",
    "window": null,
    "visible_unresolved_delta": null,
    "resolved_delta": null,
    "new_policy_eligible_delta": null
  },
  "top_debt_areas": [
    {
      "rank": 1,
      "area": "src/pricing.rs",
      "visible_unresolved": 8,
      "new_policy_eligible": 1,
      "stale_baseline_entries": 2,
      "top_static_class": "weakly_gripped"
    }
  ],
  "repair_routes": [
    {
      "rank": 1,
      "source": "baseline_debt_delta",
      "gap_id": null,
      "canonical_gap_id": null,
      "seam_id": "67fc764ba37d77bd",
      "path": "src/pricing.rs",
      "line": 88,
      "missing_discriminator": "amount == discount_threshold",
      "suggested_test": "Add an equality-boundary assertion.",
      "related_test": "tests/pricing.rs::applies_discount_above_threshold",
      "verify_command": "ripr agent verify --root . --before target/ripr/pilot/repo-exposure.json --after target/ripr/pilot/after.repo-exposure.json --json",
      "agent_command": "ripr agent start --root . --seam-id 67fc764ba37d77bd --out target/ripr/workflow",
      "static_limitations": []
    }
  ],
  "warnings": [
    "5 baseline entries are missing review metadata"
  ],
  "limits_note": "Read-only advisory RIPR Zero status over existing static RIPR artifacts; gate-decision remains the pass/fail authority."
}
```

Field contract:

- `schema_version` - currently `"0.1"`.
- `status` - `advisory` for complete reports and `incomplete` when required
  inputs are missing or unsupported.
- `ripr_zero.state` - one of `achieved`, `not_yet`, or `unknown`.
- `ripr_zero.target_source` - `gap_decision_ledger` when explicit GapRecord
  RIPR Zero targets were supplied, otherwise `baseline_debt_delta`.
- `ripr_zero.visible_unresolved` - visible unresolved behavioral test-grip gaps
  under the supplied baseline and gate scope, or explicit
  `projection_eligibility.ripr_zero_count` GapRecord targets when a gap ledger
  is supplied.
- `ripr_zero.new_policy_eligible` - current policy-eligible gaps that are not
  covered by the reviewed baseline.
- `ripr_zero.blocking_candidates` - current gate candidates that would block
  under the configured gate mode. The status report surfaces this count but does
  not make the blocking decision.
- `ripr_zero.acknowledged` - visible current findings acknowledged by label or
  policy.
- `ripr_zero.suppressed` - current findings hidden by suppression or
  configured-off severity while remaining visible in the status report.
- `baseline.metadata.current`, `stale`, `missing_metadata`, and `unknown` -
  baseline review metadata health counts. Missing metadata must not hide the
  entry.
- `debt_delta.*` - baseline movement buckets copied from the baseline debt
  delta report so summaries can show old debt, new debt, resolved debt,
  acknowledgements, suppressions, stale entries, invalid entries, and missing
  inputs without reinterpreting gate policy.
- `trend.source` - `previous_status`, `ledger`, or `not_available`.
- `top_debt_areas[]` - capped groups by stable repo-relative path or configured
  area name. Grouping is a reporting surface, not an analyzer identity rewrite.
- `repair_routes[]` - capped focused repair candidates copied from existing PR
  guidance, gate decisions, baseline debt delta, agent packets, or receipts.
  When a baseline debt delta item supplies `evidence_record`, RIPR Zero status
  prefers the record's location, grip class, missing discriminator, related
  test, assertion shape, verification command, and static limitations while
  preserving legacy top-level fields as fallback. The report must not invent
  missing commands or generated tests.
- `warnings[]` - stale baseline metadata, missing inputs, unsupported schemas,
  ambiguous identities, and trend gaps.
- `limits_note` - advisory boundary text for generated CI summaries.

Markdown should fit in a generated CI job summary. It should show RIPR 0 state,
visible unresolved gaps, existing baseline gaps still present, resolved baseline
gaps, new policy-eligible gaps, acknowledged gaps, suppressed gaps, stale
metadata, the top repair route, warnings, and the advisory boundary. It must
say that RIPR 0 is not perfect tests, 100 percent coverage, or runtime mutation
adequacy.

## Policy Readiness Report

RIPR-SPEC-0029 defines the policy readiness report. `ripr policy readiness`
joins explicit existing policy artifacts so maintainers can see the strictest
safe policy posture for the current repository without executing a gate or
changing any policy decision.

Command:

```text
ripr policy readiness \
  --gate-decision target/ripr/reports/gate-decision.json \
  --baseline-delta target/ripr/reports/baseline-debt-delta.json \
  --recommendation-calibration target/ripr/reports/recommendation-calibration.json \
  --mutation-calibration target/ripr/reports/mutation-calibration.json \
  --waiver-aging target/ripr/reports/waiver-aging.json \
  --suppression-health target/ripr/reports/suppression-health.json \
  --out target/ripr/reports/policy-readiness.json \
  --out-md target/ripr/reports/policy-readiness.md
```

The report writes:

```text
target/ripr/reports/policy-readiness.json
target/ripr/reports/policy-readiness.md
```

This report is advisory readiness evidence. `ripr gate evaluate` remains the
only pass/fail authority when an explicit gate mode is configured. The command
does not run analysis, mutate baselines or suppressions, post comments, edit
source, generate tests, run mutation testing, change gate policy, or make CI
blocking.

JSON shape:

```json
{
  "schema_version": "0.1",
  "tool": "ripr",
  "kind": "policy_readiness",
  "status": "ready_for_baseline_check",
  "recommended_mode": "baseline-check",
  "root": ".",
  "generated_at": "unix_ms:1778277000000",
  "inputs": {
    "gate_decision": "target/ripr/reports/gate-decision.json",
    "baseline_delta": "target/ripr/reports/baseline-debt-delta.json",
    "recommendation_calibration": null,
    "mutation_calibration": null,
    "waiver_aging": null,
    "suppression_health": null,
    "repo_config": null,
    "previous_readiness": null
  },
  "summary": {
    "blocking_ready": true,
    "visible_only_ready": true,
    "acknowledgeable_ready": false,
    "baseline_check_ready": true,
    "calibrated_gate_ready": false,
    "preview_candidates": 1,
    "preview_candidates_gate_eligible": 0,
    "warnings": 0,
    "unknowns": 6
  },
  "blocking_readiness": {
    "state": "healthy",
    "evidence": [
      "gate_status=advisory",
      "current_gate_mode=visible-only",
      "blocking_candidates=0"
    ],
    "warnings": [],
    "next_action": "Keep generated CI advisory unless RIPR_GATE_MODE is explicitly configured."
  },
  "baseline_health": {
    "state": "healthy",
    "evidence": ["new_policy_eligible=0", "auto_adopt_new=false"],
    "warnings": [],
    "next_action": "Use baseline-check only with the reviewed baseline path supplied."
  },
  "waiver_health": {
    "state": "missing",
    "evidence": [],
    "warnings": [],
    "next_action": "Add waiver-aging input before requiring acknowledgement."
  },
  "suppression_health": {
    "state": "missing",
    "evidence": [],
    "warnings": [],
    "next_action": "Add suppression-health input before tightening policy."
  },
  "calibration_health": {
    "state": "not_ready",
    "evidence": [],
    "warnings": [],
    "next_action": "Collect same-class recommendation calibration before calibrated-gate."
  },
  "preview_evidence_boundary": {
    "state": "healthy",
    "preview_languages": ["typescript"],
    "preview_findings_visible": 1,
    "preview_findings_acknowledgeable": 1,
    "preview_findings_suppressible": 1,
    "preview_findings_baseline_advisory": 1,
    "preview_findings_gate_eligible": 0,
    "preview_findings_ripr_zero_blocking": 0,
    "preview_findings_calibrated_confidence": 0,
    "missing_language_status": 0,
    "static_limits_seen": 1,
    "static_limits_required": true,
    "promotion_policy": null,
    "warnings": [],
    "next_action": "Keep preview evidence advisory until an explicit promotion policy exists."
  },
  "unknowns": [
    {
      "kind": "missing_input",
      "message": "recommendation_calibration input not supplied.",
      "source_artifact": null
    },
    {
      "kind": "missing_input",
      "message": "mutation_calibration input not supplied.",
      "source_artifact": null
    },
    {
      "kind": "missing_input",
      "message": "waiver_aging input not supplied.",
      "source_artifact": null
    },
    {
      "kind": "missing_input",
      "message": "suppression_health input not supplied.",
      "source_artifact": null
    },
    {
      "kind": "missing_input",
      "message": "repo_config input not supplied.",
      "source_artifact": null
    },
    {
      "kind": "missing_input",
      "message": "previous_readiness input not supplied.",
      "source_artifact": null
    }
  ],
  "warnings": [],
  "next_policy_action": "Enable baseline-check for stable Rust evidence only; keep preview evidence advisory.",
  "limits_note": "Read-only advisory readiness over explicit artifacts; gate-decision remains the only pass/fail authority when configured.",
  "preview_limits_note": "Preview-language evidence is visible and advisory by default; it is not gate-eligible, RIPR Zero blocking debt, or calibrated confidence without explicit promotion."
}
```

Field contract:

- `status` - one of `advisory_only`, `ready_for_visible_only`,
  `ready_for_acknowledgeable`, `ready_for_baseline_check`,
  `ready_for_calibrated_gate`, or `config_error`.
- `recommended_mode` - `advisory-only`, `visible-only`, `acknowledgeable`,
  `baseline-check`, or `calibrated-gate`. Values other than `advisory-only`
  match gate mode strings.
- `inputs` - supplied artifact paths, or `null` when omitted.
- `summary.*_ready` - boolean projection of which policy modes currently have
  enough readable input evidence.
- `blocking_readiness`, `baseline_health`, `waiver_health`,
  `suppression_health`, and `calibration_health` - independent health axes with
  `state`, evidence facts, warnings, and a next action.
- `suppression_health.evidence[]` - includes the supplied
  `suppression_health_status`, suppression count, missing owner/reason counts,
  stale count, overbroad scope count, unknown selector count, preview label gap
  count, warning count, and config-error count. `warning` or `config_error`
  status prevents acknowledgeable readiness.
- `preview_evidence_boundary` - RIPR-SPEC-0030 projection. Preview findings
  remain visible while default gate eligibility, RIPR Zero blocking, and
  calibrated-confidence counts remain zero until explicit promotion. Missing
  preview labels keep the readiness recommendation advisory until repaired.
- `unknowns[]` - missing recommended or optional inputs. Missing inputs are not
  treated as passing evidence.
- `warnings[]` - malformed supplied inputs, preview metadata gaps, or policy
  readiness limitations.
- `limits_note` and `preview_limits_note` - advisory, pass/fail, and preview
  policy boundary text.

Markdown should fit in a job summary. It should show the status, recommended
mode, each health axis, preview zero-count boundary, unknowns, warnings, next
policy action, and limits. It must not claim runtime mutation outcomes or make
the report a gate.

## Policy Operations Report

RIPR-SPEC-0039 defines the policy operations report. `ripr policy operations`
composes explicit policy artifacts into one read-only operator
packet that names the current safe ceiling, next safe action, safe and blocked
promotion modes, blockers, action lists, warnings, unknowns, and input health.

Command:

```text
ripr policy operations \
  --policy-readiness target/ripr/reports/policy-readiness.json \
  --waiver-aging target/ripr/reports/waiver-aging.json \
  --suppression-health target/ripr/reports/suppression-health.json \
  --baseline-delta target/ripr/reports/baseline-debt-delta.json \
  --gate-decision target/ripr/reports/gate-decision.json \
  --recommendation-calibration target/ripr/reports/recommendation-calibration.json \
  --mutation-calibration target/ripr/reports/mutation-calibration.json \
  --out target/ripr/reports/policy-operations.json \
  --out-md target/ripr/reports/policy-operations.md
```

The report writes:

```text
target/ripr/reports/policy-operations.json
target/ripr/reports/policy-operations.md
```

This report is advisory policy operations evidence. It does not execute a gate,
mutate config, baselines, suppressions, workflows, branch protection, generated
CI defaults, or source files, promote preview-language evidence, run analysis,
generate tests, call providers, post comments, or run mutation testing.

JSON shape:

```json
{
  "schema_version": "0.1",
  "tool": "ripr",
  "kind": "policy_operations",
  "root": ".",
  "generated_at": "unix_ms:1778277000000",
  "current_policy_ceiling": "ready_for_acknowledgeable",
  "recommended_next_action": "Run shrink-only baseline review and remove resolved entries.",
  "safe_to_promote_to": [
    {
      "mode": "visible-only",
      "allowed_now": true,
      "reason": "Policy readiness and supplied inputs allow visible-only advisory display.",
      "source_artifacts": [
        "target/ripr/reports/policy-readiness.json",
        "target/ripr/reports/gate-decision.json"
      ]
    },
    {
      "mode": "acknowledgeable",
      "allowed_now": true,
      "reason": "Waivers are visible PR-time acknowledgements and suppression health is readable.",
      "source_artifacts": [
        "target/ripr/reports/policy-readiness.json",
        "target/ripr/reports/waiver-aging.json",
        "target/ripr/reports/suppression-health.json"
      ]
    }
  ],
  "not_safe_to_promote_to": [
    {
      "mode": "baseline-check",
      "allowed_now": false,
      "reason": "Current policy ceiling ready_for_acknowledgeable does not allow baseline-check. Baseline contains 1 stale entries.",
      "blockers": [
        "current_ceiling_below_baseline_check",
        "baseline_stale_entries"
      ],
      "source_artifacts": [
        "target/ripr/reports/baseline-debt-delta.json"
      ]
    }
  ],
  "promotion_blockers": [
    {
      "kind": "baseline_stale_entries",
      "severity": "warning",
      "message": "Baseline contains 1 stale entries.",
      "target_modes": ["baseline-check", "calibrated-gate"],
      "source_artifact": "target/ripr/reports/baseline-debt-delta.json",
      "repair_action": "Run shrink-only baseline review and remove resolved entries."
    }
  ],
  "baseline_actions": [
    "Review stale baseline entries.",
    "Use shrink-only refresh for resolved debt."
  ],
  "waiver_actions": [
    "Review repeated PR-time acknowledgements before requiring acknowledgement.",
    "Keep waivers visible and do not convert them to suppressions automatically."
  ],
  "suppression_actions": [
    "Keep durable suppressions visible with owner, reason, scope, and review metadata."
  ],
  "calibration_actions": [
    "Collect same-class recommendation calibration before calibrated-gate.",
    "Optional mutation calibration was not supplied; keep runtime confirmation separate from static evidence."
  ],
  "preview_boundary_actions": [
    "Keep typescript preview evidence visible/advisory and excluded from gate eligibility, RIPR Zero blocking debt, and calibrated confidence."
  ],
  "warnings": [
    {
      "kind": "missing_optional_input",
      "message": "No mutation calibration input was supplied.",
      "source_artifact": null
    }
  ],
  "unknowns": [
    {
      "kind": "preview_boundary_not_supplied",
      "message": "Preview boundary details came only from policy readiness when available.",
      "source_artifact": "target/ripr/reports/policy-readiness.json"
    }
  ],
  "input_artifacts": [
    {
      "kind": "policy_readiness",
      "path": "target/ripr/reports/policy-readiness.json",
      "status": "read"
    },
    {
      "kind": "preview_boundary",
      "path": null,
      "status": "omitted"
    }
  ],
  "limits_note": "Read-only advisory policy operations report over explicit existing artifacts. Promotion requires separate manual review and configuration changes; this report never mutates config, baselines, suppressions, workflows, CI defaults, or preview-language eligibility."
}
```

Field contract:

- `schema_version` - currently `"0.1"`.
- `tool` - always `"ripr"`.
- `kind` - always `"policy_operations"`.
- `current_policy_ceiling` - copied or derived from policy readiness.
  Supported values are `advisory_only`, `ready_for_visible_only`,
  `ready_for_acknowledgeable`, `ready_for_baseline_check`,
  `ready_for_calibrated_gate`, `not_ready`, and `config_error`.
- `recommended_next_action` - the first repair or operator action needed before
  stricter policy review.
- `safe_to_promote_to[]` - target modes currently allowed by the ceiling and
  readable dependent inputs.
- `not_safe_to_promote_to[]` - target modes blocked by ceiling, baseline,
  waiver, suppression, calibration, preview-boundary, or input health.
- `promotion_blockers[]` - normalized blocker records with severity, target
  modes, source artifact, and repair action.
- `baseline_actions[]`, `waiver_actions[]`, `suppression_actions[]`,
  `calibration_actions[]`, and `preview_boundary_actions[]` - operator actions
  grouped by policy surface.
- `warnings[]` - malformed supplied inputs or optional evidence gaps.
- `unknowns[]` - missing or unknowable context that limits confidence.
- `input_artifacts[]` - one record for every operations input. Status values
  are `read`, `omitted`, `missing`, `malformed`, and `not_applicable`.
- `limits_note` - static advisory boundary and no-mutation policy text.

Markdown should fit in a job summary. It should show current ceiling, next safe
action, can-promote and cannot-promote sections, top blockers, grouped actions,
warnings, unknowns, input artifact status, and limits. It must not make a gate
decision or promote preview-language evidence.

## Policy History Report

RIPR-SPEC-0041 defines the policy history report. `ripr policy history` reads a
current `policy-operations.json` report plus an optional append-only history
JSONL input and writes a read-only advisory trend packet. The report shows
whether readiness, waiver pressure, suppression health, baseline movement,
preview-boundary state, and calibration health improved, regressed, stayed
unchanged, or are unknown.

Command:

```text
ripr policy history \
  --current target/ripr/reports/policy-operations.json \
  --history .ripr/policy-history.jsonl \
  --commit HEAD \
  --pr-number 123 \
  --out target/ripr/reports/policy-history.json \
  --out-md target/ripr/reports/policy-history.md
```

The report writes:

```text
target/ripr/reports/policy-history.json
target/ripr/reports/policy-history.md
```

This report is advisory policy trend evidence. It does not append to
`.ripr/policy-history.jsonl`, execute gates, collect telemetry, mutate config,
baselines, suppressions, workflows, branch protection, generated CI defaults,
or source files, promote preview-language evidence, run analysis, generate
tests, call providers, post comments, or run mutation testing.

JSON shape:

```json
{
  "schema_version": "0.1",
  "tool": "ripr",
  "kind": "policy_history",
  "root": ".",
  "generated_at": "unix_ms:1778277000000",
  "current": {
    "commit": "HEAD",
    "pr_number": "123",
    "generated_at": "unix_ms:1778277000000",
    "recommended_mode": "acknowledgeable",
    "current_policy_ceiling": "ready_for_acknowledgeable",
    "baseline_health": "warning",
    "waiver_health": "advisory",
    "suppression_health": "healthy",
    "calibration_health": "not_ready",
    "preview_boundary_state": "healthy",
    "new_policy_eligible_count": 1,
    "waiver_count": 2,
    "stale_suppression_count": 0,
    "baseline_still_present": 4,
    "baseline_resolved": 1
  },
  "history_summary": {
    "entries": 3,
    "oldest_generated_at": "unix_ms:1778190600000",
    "newest_generated_at": "unix_ms:1778277000000",
    "readiness_improved": true,
    "waiver_pressure_increased": false,
    "suppression_health_regressed": false,
    "baseline_shrank": true,
    "preview_remained_advisory": true,
    "calibration_changed_ceiling": false
  },
  "trend": {
    "ceiling": {
      "previous": "ready_for_visible_only",
      "current": "ready_for_acknowledgeable",
      "direction": "improved"
    },
    "waiver_count": {
      "previous": 3,
      "current": 2,
      "direction": "improved"
    },
    "stale_suppression_count": {
      "previous": 0,
      "current": 0,
      "direction": "unchanged"
    },
    "baseline_still_present": {
      "previous": 5,
      "current": 4,
      "direction": "improved"
    },
    "baseline_resolved": {
      "previous": 0,
      "current": 1,
      "direction": "improved"
    },
    "preview_boundary_state": {
      "previous": "healthy",
      "current": "healthy",
      "direction": "unchanged"
    },
    "calibration_health": {
      "previous": "not_ready",
      "current": "not_ready",
      "direction": "unchanged"
    }
  },
  "example_append_record": {
    "commit": "HEAD",
    "pr_number": "123",
    "generated_at": "unix_ms:1778277000000",
    "current_policy_ceiling": "ready_for_acknowledgeable",
    "recommended_mode": "acknowledgeable"
  },
  "warnings": [],
  "unknowns": [
    {
      "kind": "history_not_supplied",
      "message": "No policy history JSONL was supplied; trend is limited to the current snapshot.",
      "source_artifact": null
    }
  ],
  "input_artifacts": [
    {
      "kind": "policy_operations",
      "path": "target/ripr/reports/policy-operations.json",
      "status": "read"
    },
    {
      "kind": "policy_history_jsonl",
      "path": ".ripr/policy-history.jsonl",
      "status": "missing"
    }
  ],
  "limits_note": "Read-only advisory policy history report. It reads explicit history inputs and never appends, mutates policy, or changes gate authority."
}
```

Field contract:

- `schema_version` - currently `"0.1"`.
- `tool` - always `"ripr"`.
- `kind` - always `"policy_history"`.
- `current` - normalized snapshot derived from `policy-operations.json` plus
  optional commit and PR metadata.
- `current.recommended_mode` - derived from the highest safe promotion mode or
  current policy operations ceiling.
- `current.current_policy_ceiling` - copied from policy operations.
- `current.*_health` fields - normalized policy surface states derived from
  operations actions, blockers, and input artifacts.
- `current.new_policy_eligible_count`, `waiver_count`,
  `stale_suppression_count`, `baseline_still_present`, and
  `baseline_resolved` - current movement counters when available, otherwise
  zero with an unknown.
- `history_summary.entries` - count of prior plus current snapshots included in
  the trend.
- `history_summary.readiness_improved` - true only when the current ceiling
  ranks higher than the previous comparable snapshot.
- `history_summary.waiver_pressure_increased` - true when waiver count rises.
- `history_summary.suppression_health_regressed` - true when stale or malformed
  suppression signals rise.
- `history_summary.baseline_shrank` - true when still-present baseline debt
  falls or resolved baseline debt rises without adopt-new behavior.
- `history_summary.preview_remained_advisory` - true only when preview evidence
  stayed non-gating across comparable snapshots.
- `history_summary.calibration_changed_ceiling` - true when calibration health
  improvement is the reason the ceiling changed.
- `trend.*.direction` - `improved`, `regressed`, `unchanged`, or `unknown`.
- `example_append_record` - the current snapshot in appendable JSONL shape. It
  is advisory output only and must not be written automatically.
- `warnings[]` - malformed supplied history lines, malformed current input, or
  unsupported historical shapes.
- `unknowns[]` - missing optional history, commit, PR number, or unavailable
  metric fields.
- `input_artifacts[]` - per-input status. Status values are `read`, `omitted`,
  `missing`, `malformed`, and `not_applicable`.
- `limits_note` - read-only/no-telemetry/no-mutation/no-gate boundary.

Markdown should fit in generated CI summaries and report packets. It should
show the current ceiling, recommended mode, history entry count, trend summary,
current snapshot counters, input artifact status, an optional manual append
record, warnings, unknowns, and limits. It must not append history
automatically, make a gate decision, or promote preview-language evidence.

## Policy Promotion Packet

RIPR-SPEC-0042 defines the policy promotion packet. `ripr policy promote`
reads a current `policy-operations.json` report plus optional
`policy-history.json` and writes a read-only manual-review packet for one target
mode.

Command:

```text
ripr policy promote \
  --to baseline-check \
  --operations target/ripr/reports/policy-operations.json \
  --history target/ripr/reports/policy-history.json \
  --out target/ripr/reports/policy-promotion-baseline-check.json \
  --out-md target/ripr/reports/policy-promotion-baseline-check.md
```

The report writes:

```text
target/ripr/reports/policy-promotion-visible-only.json
target/ripr/reports/policy-promotion-visible-only.md
target/ripr/reports/policy-promotion-acknowledgeable.json
target/ripr/reports/policy-promotion-acknowledgeable.md
target/ripr/reports/policy-promotion-baseline-check.json
target/ripr/reports/policy-promotion-baseline-check.md
target/ripr/reports/policy-promotion-calibrated-gate.json
target/ripr/reports/policy-promotion-calibrated-gate.md
```

This report is advisory policy review evidence. It does not mutate `ripr.toml`,
baselines, suppressions, workflows, branch protection, generated CI defaults,
source files, history ledgers, or preview-language eligibility. It does not
execute gates, post comments, run analysis, generate tests, call providers, run
mutation testing, or make CI blocking by default.

JSON shape:

```json
{
  "schema_version": "0.1",
  "tool": "ripr",
  "kind": "policy_promotion_packet",
  "root": ".",
  "generated_at": "unix_ms:1778277000000",
  "target_mode": "baseline-check",
  "allowed_now": false,
  "why_or_why_not": "Baseline contains stale entries and suppression health has warnings.",
  "required_repairs": [
    "Run shrink-only baseline review and remove resolved entries.",
    "Repair suppression-health warnings before tightening policy."
  ],
  "required_receipts": [
    "policy-operations.json showing baseline-check in safe_to_promote_to",
    "policy-history.json showing baseline debt is not being normalized",
    "baseline-debt-delta.json showing reviewed shrink-only movement",
    "suppression-health.json showing durable exception metadata is healthy"
  ],
  "rollback_path": [
    "Revert the manual gate-mode config change.",
    "Return to visible-only or acknowledgeable policy mode.",
    "Keep policy operations and history artifacts for audit."
  ],
  "example_config_change": {
    "file": "ripr.toml",
    "change": "Set the reviewed policy gate mode to baseline-check.",
    "manual_only": true
  },
  "input_artifacts": [
    {
      "kind": "policy_operations",
      "path": "target/ripr/reports/policy-operations.json",
      "status": "read"
    },
    {
      "kind": "policy_history",
      "path": "target/ripr/reports/policy-history.json",
      "status": "read"
    }
  ],
  "warnings": [],
  "unknowns": [],
  "non_goals": [
    "No automatic config mutation.",
    "No automatic baseline adoption.",
    "No suppression creation.",
    "No default CI blocking.",
    "No preview-language promotion."
  ],
  "limits_note": "Read-only advisory promotion packet. It supports manual review only and never mutates policy configuration or gate authority."
}
```

Field contract:

- `schema_version` - currently `"0.1"`.
- `tool` - always `"ripr"`.
- `kind` - always `"policy_promotion_packet"`.
- `target_mode` - one of `visible-only`, `acknowledgeable`,
  `baseline-check`, or `calibrated-gate`.
- `allowed_now` - true only when policy operations lists the target in
  `safe_to_promote_to`.
- `why_or_why_not` - explanation from the operations safe/not-safe entry and
  blockers.
- `required_repairs[]` - blocker repair actions required before manual
  promotion review.
- `required_receipts[]` - artifacts reviewers should inspect before accepting
  a manual config change.
- `rollback_path[]` - explicit steps to return to a less strict posture.
- `example_config_change` - manual review guidance only. The command must not
  write this change.
- `input_artifacts[]` - per-input status.
- `warnings[]` - malformed supplied inputs, unsupported history shape, or
  target-mode limitations.
- `unknowns[]` - missing optional history or unavailable supporting context.
- `non_goals[]` - hard boundaries repeated in the packet.
- `limits_note` - read-only/manual-review/no-mutation boundary.

Markdown should fit in generated CI summaries and report packets. It should
show the target mode, allowed status, why/why not explanation, required
repairs, required receipts, rollback path, manual-only config example, input
artifact status, warnings, unknowns, non-goals, and limits. It must not mutate
policy configuration or promote preview-language evidence.

## Preview Evidence Promotion Packet

RIPR-SPEC-0044 defines the preview evidence promotion packet. The
`ripr policy preview-promote` command writes a read-only advisory packet for a
preview language and evidence class. The default result is blocked:
`allowed_now = false` with reason `preview promotion evidence not supplied`.
The maintainer-facing proof checklist is
[Preview promotion criteria](policy/PREVIEW_PROMOTION_CRITERIA.md).

Command:

```text
ripr policy preview-promote \
  --language typescript \
  --class boundary_gap \
  --evidence target/ripr/reports/preview-promotion-evidence.json \
  --out target/ripr/reports/preview-promotion-typescript-boundary-gap.json \
  --out-md target/ripr/reports/preview-promotion-typescript-boundary-gap.md
```

The report writes:

```text
target/ripr/reports/preview-promotion-<language>-<class>.json
target/ripr/reports/preview-promotion-<language>-<class>.md
```

This report is advisory policy review evidence. It does not mutate `ripr.toml`,
baselines, suppressions, workflows, branch protection, generated CI defaults,
source files, history ledgers, gate configuration, RIPR Zero membership,
calibrated confidence, or preview-language eligibility. It does not execute
gates, post comments, run analysis, generate tests, call providers, run
mutation testing, or make CI blocking by default.

JSON shape:

```json
{
  "schema_version": "0.1",
  "tool": "ripr",
  "kind": "preview_evidence_promotion_packet",
  "root": ".",
  "generated_at": "unix_ms:1778277000000",
  "language": "typescript",
  "language_status": "preview",
  "candidate_class": "boundary_gap",
  "target_status": "policy_eligible",
  "allowed_now": false,
  "reason": "preview promotion evidence not supplied",
  "required_evidence": [
    {
      "kind": "fixture_corpus_coverage",
      "required": true,
      "description": "Representative fixtures cover the candidate class and known static limits."
    },
    {
      "kind": "static_limit_exclusions",
      "required": true,
      "description": "Known static parser, language-adapter, and static-limit taxonomy limits are covered, excluded, or labeled."
    },
    {
      "kind": "false_positive_review",
      "required": true,
      "description": "Maintainer-reviewed false-positive sample is documented for this language and class."
    },
    {
      "kind": "recommendation_calibration",
      "required": true,
      "description": "Same-class recommendation calibration supports policy eligibility."
    },
    {
      "kind": "dogfood_receipts",
      "required": true,
      "description": "External-style dogfood receipts exercise the candidate language and class through the start-here repair loop."
    },
    {
      "kind": "related_test_accuracy_review",
      "required": true,
      "description": "Maintainer-reviewed related-test samples show the candidate language does not route repair packets to wrong tests."
    },
    {
      "kind": "false_repair_packet_review",
      "required": true,
      "description": "Maintainer-reviewed sample confirms preview repair packets do not overstate or invent safe repairs."
    },
    {
      "kind": "surface_consistency_review",
      "required": true,
      "description": "Editor, CLI, generated CI, PR evidence, receipts, and docs show the same preview/advisory boundary."
    },
    {
      "kind": "policy_signoff",
      "required": true,
      "description": "Policy owner explicitly signs off that the narrow language/class may be reviewed for stronger status."
    },
    {
      "kind": "mutation_calibration",
      "required": false,
      "description": "Optional runtime calibration exists for this language and class without being inferred from Rust."
    },
    {
      "kind": "baseline_behavior",
      "required": true,
      "description": "Baseline handling keeps preview debt visible and does not auto-adopt new preview findings."
    },
    {
      "kind": "waiver_suppression_behavior",
      "required": true,
      "description": "Waivers and suppressions preserve owner, reason, scope, and preview status."
    },
    {
      "kind": "rollback_path",
      "required": true,
      "description": "Manual rollback to advisory preview status is documented."
    },
    {
      "kind": "generated_ci_posture",
      "required": true,
      "description": "Generated CI remains advisory and non-blocking unless a later explicit gate mode is configured."
    }
  ],
  "supplied_evidence": [],
  "missing_evidence": [
    "fixture_corpus_coverage",
    "static_limit_exclusions",
    "false_positive_review",
    "recommendation_calibration",
    "dogfood_receipts",
    "related_test_accuracy_review",
    "false_repair_packet_review",
    "surface_consistency_review",
    "policy_signoff",
    "baseline_behavior",
    "waiver_suppression_behavior",
    "rollback_path",
    "generated_ci_posture"
  ],
  "required_repairs": [
    "Supply explicit preview promotion evidence before policy eligibility review."
  ],
  "required_receipts": [
    "preview-promotion-typescript-boundary-gap.json",
    "preview-boundary report showing advisory language status",
    "fixture corpus coverage receipt for TypeScript boundary_gap",
    "static-limit exclusions receipt for TypeScript boundary_gap",
    "false-positive review receipt for TypeScript boundary_gap",
    "recommendation-calibration receipt for TypeScript boundary_gap",
    "dogfood receipt for TypeScript boundary_gap",
    "related-test accuracy review receipt for TypeScript boundary_gap",
    "false repair packet review receipt for TypeScript boundary_gap",
    "surface consistency receipt for TypeScript boundary_gap",
    "policy signoff receipt for TypeScript boundary_gap",
    "baseline behavior receipt for TypeScript boundary_gap",
    "waiver/suppression behavior receipt for TypeScript boundary_gap",
    "rollback path receipt for TypeScript boundary_gap",
    "generated CI posture receipt for TypeScript boundary_gap"
  ],
  "rollback_path": [
    "Keep TypeScript boundary_gap evidence advisory.",
    "Remove any manual preview promotion config if one was reviewed later.",
    "Regenerate policy operations and preview promotion packets after rollback."
  ],
  "generated_ci_posture": {
    "may_upload_artifact": true,
    "may_summarize_artifact": true,
    "may_fail_check": false,
    "may_post_comment": false,
    "may_mutate_config": false
  },
  "input_artifacts": [],
  "warnings": [],
  "unknowns": [],
  "non_goals": [
    "No actual promotion.",
    "No gate eligibility change.",
    "No RIPR Zero inclusion.",
    "No calibrated confidence.",
    "No CI blocking."
  ],
  "limits_note": "Read-only advisory preview promotion packet. Preview evidence remains visible and non-gating until a later explicit promotion policy is reviewed."
}
```

Field contract:

- `schema_version` - currently `"0.1"`.
- `tool` - always `"ripr"`.
- `kind` - always `"preview_evidence_promotion_packet"`.
- `language` - requested preview language.
- `language_status` - current status, initially `"preview"`.
- `candidate_class` - requested evidence class.
- `target_status` - requested future policy status. The packet may describe it
  but must not apply it.
- `allowed_now` - false unless every required evidence item is supplied and a
  later implementation explicitly recognizes those receipts.
- `reason` - concise explanation for the decision.
- `required_evidence[]` - full evidence checklist for preview promotion.
- `supplied_evidence[]` - evidence receipts accepted by the packet.
- `missing_evidence[]` - required evidence still absent.
- `required_repairs[]` - concrete work before a maintainer can review
  promotion.
- `required_receipts[]` - artifacts reviewers should inspect before promotion.
- `rollback_path[]` - explicit return path to advisory preview status.
- `generated_ci_posture` - advisory CI permissions and hard denials.
- `input_artifacts[]` - optional explicit evidence input status.
- `warnings[]` - malformed supplied inputs or target-language limitations.
- `unknowns[]` - unavailable context that must stay visible.
- `non_goals[]` - hard boundaries repeated in the packet.
- `limits_note` - read-only/manual-review/no-promotion boundary.

Markdown should fit in generated CI summaries and report packets. It should
show language, class, current status, target status, allowed status, reason,
supplied and missing evidence, required repairs, required receipts, rollback
path, generated CI posture, input artifact status, warnings, unknowns,
non-goals, and limits. It must not promote preview evidence or mutate policy.

## Suppression Health Report

`ripr policy suppression-health` summarizes the durable suppression manifest
without applying suppressions or changing policy. It exists so teams can audit
whether durable exceptions have enough metadata before stricter policy modes
depend on them.

Command:

```text
ripr policy suppression-health \
  --root . \
  --manifest .ripr/suppressions.toml \
  --out target/ripr/reports/suppression-health.json \
  --out-md target/ripr/reports/suppression-health.md
```

The report writes:

```text
target/ripr/reports/suppression-health.json
target/ripr/reports/suppression-health.md
```

This report is advisory policy evidence. It does not run analysis, mutate
baselines or suppressions, post comments, edit source, generate tests, run
mutation testing, change gate policy, or make CI blocking.

JSON shape:

```json
{
  "schema_version": "0.1",
  "tool": "ripr",
  "kind": "suppression_health",
  "status": "warning",
  "root": ".",
  "generated_at": "unix_ms:1778277000000",
  "inputs": {
    "manifest": ".ripr/suppressions.toml"
  },
  "summary": {
    "suppressions": 2,
    "healthy": 1,
    "missing_owner": 0,
    "missing_reason": 0,
    "missing_scope": 1,
    "missing_created_at": 0,
    "missing_last_seen": 0,
    "missing_review_by_or_expires": 0,
    "missing_expected_visibility": 0,
    "missing_static_class": 0,
    "stale": 0,
    "overbroad_scope": 1,
    "unknown_selector": 0,
    "preview_without_preview_label": 1,
    "warnings": 3,
    "config_errors": 0
  },
  "records": [
    {
      "identity": "probe:src/pricing.rs:88:predicate",
      "kind": "exposure_gap",
      "owner": "billing",
      "reason": "accepted durable policy exception",
      "scope": "seam:pricing::threshold",
      "created_at": "2026-01-01",
      "last_seen": "2026-05-01",
      "expires": null,
      "review_by": "2026-12-01",
      "expected_visibility": "suppressed_visible",
      "static_class": "weakly_exposed",
      "language": "rust",
      "language_status": null,
      "health": "healthy",
      "still_visible": true,
      "source": ".ripr/suppressions.toml:4",
      "findings": []
    }
  ],
  "findings": [
    {
      "kind": "preview_without_preview_label",
      "severity": "warning",
      "message": "preview-language suppression is missing language_status = \"preview\"",
      "source": ".ripr/suppressions.toml:18"
    }
  ],
  "warnings": [],
  "limits_note": "Read-only advisory suppression-health report over the durable suppression manifest; suppressions remain visible and the report never creates, deletes, applies, or gates on suppressions."
}
```

Field contract:

- `status` - `no_suppressions` when the manifest is missing or empty,
  `healthy` when all parsed records have complete policy metadata, `warning`
  when valid records need review, or `config_error` when the manifest is
  malformed.
- `summary.missing_owner` and `summary.missing_reason` - parser-level
  structural errors. Owner and reason remain required for every durable
  suppression.
- `summary.stale` - entries whose `expires` or `review_by` date is before the
  report date.
- `summary.overbroad_scope` - entries whose scope is explicitly broad, or
  test-efficiency suppressions that omit `path`.
- `summary.unknown_selector` - unsupported kinds, missing required selectors,
  blank selectors, or duplicate selectors.
- `summary.preview_without_preview_label` - preview-language suppressions that
  omit `language_status = "preview"`.
- `records[].still_visible` - always `true`; suppression health never hides
  suppressed findings.
- `findings[]` - normalized findings with `kind`, `severity`, `message`, and
  optional source.
- `limits_note` - advisory/read-only boundary text.

Markdown should fit in a job summary. It should show the status, each durable
suppression identity, owner, review date, findings, and the advisory boundary.

## Waiver Aging Report

`ripr policy waiver-aging` summarizes visible PR-time waivers from the current
PR evidence ledger and optional prior ledger history. It exists so repeated
waiver remains a visible signal for repair or explicit policy review without
becoming a failure, a suppression, or a hidden exception.

Command:

```text
ripr policy waiver-aging \
  --ledger target/ripr/reports/pr-evidence-ledger.json \
  --history .ripr/pr-evidence-ledger.jsonl \
  --out target/ripr/reports/waiver-aging.json \
  --out-md target/ripr/reports/waiver-aging.md
```

The report writes:

```text
target/ripr/reports/waiver-aging.json
target/ripr/reports/waiver-aging.md
```

This report is advisory policy evidence. It does not run analysis, mutate
baselines or suppressions, post comments, edit source, generate tests, run
mutation testing, change gate policy, or make CI blocking.

JSON shape:

```json
{
  "schema_version": "0.1",
  "tool": "ripr",
  "kind": "waiver_aging",
  "status": "advisory",
  "root": ".",
  "generated_at": "unix_ms:1778277000000",
  "inputs": {
    "ledger": "target/ripr/reports/pr-evidence-ledger.json",
    "history": ".ripr/pr-evidence-ledger.jsonl"
  },
  "summary": {
    "waiver_count": 3,
    "identity_count": 1,
    "repeated_seam_count": 1,
    "repeated_file_count": 1,
    "max_age_prs": 3,
    "max_age_days": 40,
    "focused_test_candidates": 1,
    "durable_suppression_candidates": 1,
    "warnings": 0
  },
  "records": [
    {
      "identity": "pricing::discount::threshold_equality",
      "canonical_gap_id": "pricing::discount::threshold_equality",
      "seam_id": "67fc764ba37d77bd",
      "file": "src/pricing.rs",
      "owner": null,
      "waiver_count": 3,
      "first_seen": "pr#123",
      "last_seen": "pr#125",
      "age_prs": 3,
      "age_days": 40,
      "same_seam_waived_repeatedly": true,
      "same_file_waived_repeatedly": true,
      "candidate_for_focused_test": true,
      "candidate_for_durable_suppression": true,
      "reasons": ["accepted for this PR"],
      "labels": ["ripr-waive"],
      "still_visible": true,
      "source_records": [
        ".ripr/pr-evidence-ledger.jsonl:1",
        ".ripr/pr-evidence-ledger.jsonl:2",
        "target/ripr/reports/pr-evidence-ledger.json"
      ]
    }
  ],
  "warnings": [],
  "limits_note": "Read-only advisory waiver-aging report over existing PR evidence ledgers; repeated waiver is a signal, not a failure or durable suppression."
}
```

Field contract:

- `status` - `advisory`, `no_waivers`, `incomplete`, or `config_error`.
- `inputs` - supplied current PR ledger and JSONL history paths, or `null` when
  omitted.
- `summary.waiver_count` - visible waiver observations across supplied ledgers.
- `summary.identity_count` - distinct canonical gap, seam, or waiver identities.
- `summary.repeated_*` - repeated-waiver signals. These are not failures.
- `records[].identity` - canonical gap id when available, else seam id,
  decision id, or a source-local fallback.
- `records[].file` and `records[].owner` - copied from source ledgers when
  available; missing values stay `null`.
- `records[].candidate_for_focused_test` - advisory signal for repeated or aged
  waiver that should usually become a focused test.
- `records[].candidate_for_durable_suppression` - advisory signal for policy
  review only; it does not create or imply a suppression.
- `records[].still_visible` - waivers remain visible acknowledgements.
- `warnings[]` - malformed supplied inputs, invalid JSONL lines, or missing
  optional history.
- `limits_note` - advisory boundary text.

Markdown should fit in a job summary. It should show waiver identities, counts,
age, candidate signals, warnings, and the policy boundary that repeated waiver
is a visible signal rather than pass/fail authority.

## PR Evidence Ledger

RIPR-SPEC-0018 defines the PR evidence ledger. `ripr pr-ledger record` records
per-PR behavioral grip movement from existing RIPR artifacts so teams can track
new policy-eligible gaps, resolved baseline debt, visible acknowledgements,
suppressions, repair receipts, and optional coverage/grip frontier signals
without changing analyzer identity, gate policy, or advisory defaults.

Command:

```text
ripr pr-ledger record \
  --pr-number 123 \
  --head <sha> \
  --base <sha> \
  --gate target/ripr/reports/gate-decision.json \
  --baseline-delta target/ripr/reports/baseline-debt-delta.json \
  --zero-status target/ripr/reports/ripr-zero-status.json \
  --pr-guidance target/ripr/review/comments.json \
  --gap-ledger target/ripr/reports/gap-decision-ledger.json \
  --recommendation-calibration target/ripr/reports/recommendation-calibration.json \
  --agent-receipt target/ripr/reports/agent-receipt.json \
  --coverage target/ripr/reports/coverage-summary.json \
  --history .ripr/pr-evidence-ledger.jsonl \
  --out target/ripr/reports/pr-evidence-ledger.json \
  --out-md target/ripr/reports/pr-evidence-ledger.md
```

The report writes:

```text
target/ripr/reports/pr-evidence-ledger.json
target/ripr/reports/pr-evidence-ledger.md
```

This report is advisory history. `ripr gate evaluate` remains the pass/fail
authority for configured gate modes. Generated GitHub CI runs
`ripr pr-ledger record` on pull requests when `target/ripr/review/comments.json`
exists, adds optional gate, baseline delta, RIPR Zero, recommendation
calibration, agent receipt, coverage, label, and history inputs when present,
and may add a gap decision ledger input when an explicit ledger artifact exists.
It uploads `pr-evidence-ledger.{json,md}` with the normal report packet and
appends a PR movement card to the job summary. The report itself must not fail
CI, rewrite baselines, post comments, edit source, generate tests, rerun
analysis, call an LLM, or run mutation testing.

JSON shape:

```json
{
  "schema_version": "0.1",
  "tool": "ripr",
  "kind": "pr_evidence_ledger",
  "status": "advisory",
  "root": ".",
  "generated_at": "2026-05-09T00:00:00Z",
  "pr": {
    "number": 123,
    "base": "53ea9a205f569a5ca636ba0a7451c6aca8b5ad2e",
    "head": "984d5222a058fbceecfb9b230baef65c47c52820",
    "labels": ["ripr-waive"]
  },
  "inputs": {
    "gate_decision": "target/ripr/reports/gate-decision.json",
    "baseline_debt_delta": "target/ripr/reports/baseline-debt-delta.json",
    "ripr_zero_status": "target/ripr/reports/ripr-zero-status.json",
    "pr_guidance": "target/ripr/review/comments.json",
    "gap_decision_ledger": "target/ripr/reports/gap-decision-ledger.json",
    "recommendation_calibration": "target/ripr/reports/recommendation-calibration.json",
    "agent_receipt": "target/ripr/reports/agent-receipt.json",
    "coverage": "target/ripr/reports/coverage-summary.json",
    "history": ".ripr/pr-evidence-ledger.jsonl"
  },
  "movement": {
    "new_policy_eligible": 1,
    "baseline_still_present": 40,
    "baseline_resolved": 3,
    "acknowledged": 1,
    "suppressed": 0,
    "blocking_candidates": 0,
    "visible_unresolved": 41,
    "ripr_zero_state": "not_yet"
  },
  "gate": {
    "mode": "baseline-check",
    "decision": "acknowledged",
    "pass_fail_authority": "ripr gate evaluate",
    "acknowledgement_label": "ripr-waive"
  },
  "waivers": [
    {
      "label": "ripr-waive",
      "canonical_gap_id": "pricing::discount::threshold_equality",
      "decision_id": "ripr-gate-67fc764ba37d77bd",
      "seam_id": "67fc764ba37d77bd",
      "age_prs": 1,
      "age_days": 0,
      "reason": "accepted for this PR",
      "still_visible": true
    }
  ],
  "suppressions": [
    {
      "canonical_gap_id": "pricing::discount::threshold_equality",
      "decision_id": "ripr-gate-suppressed",
      "seam_id": "suppressed",
      "source": ".ripr/suppressions.toml",
      "owner": "test-platform",
      "reason": "accepted durable policy exception",
      "still_visible": true
    }
  ],
  "repair_receipts": [
    {
      "source": "agent_receipt",
      "canonical_gap_id": "pricing::discount::threshold_equality",
      "seam_id": "67fc764ba37d77bd",
      "receipt_state": "receipt_movement_improved",
      "static_movement": {
        "state": "improved",
        "source": "agent_receipt",
        "artifact": "target/ripr/reports/agent-receipt.json"
      },
      "focused_test": "tests/pricing.rs::threshold_exact_boundary",
      "receipt": "target/ripr/reports/agent-receipt.json"
    }
  ],
  "coverage_grip_frontier": {
    "status": "available",
    "coverage_delta_percent": 0.0,
    "ripr_visible_unresolved_delta": -3,
    "interpretation": "behavioral grip improved without line-coverage movement",
    "quadrants": {
      "covered_with_ripr_gap": 2,
      "covered_without_ripr_gap": 12,
      "uncovered_with_ripr_gap": 1,
      "uncovered_without_ripr_gap": 0
    }
  },
  "top_repair_route": {
    "source": "gap_decision_ledger",
    "gap_id": "gap:pr:pricing:threshold-boundary",
    "canonical_gap_id": "pricing::discount::threshold_equality",
    "seam_id": "67fc764ba37d77bd",
    "path": "src/pricing.rs",
    "line": 88,
    "missing_discriminator": "amount == discount_threshold",
    "suggested_test": "Add an equality-boundary assertion.",
    "related_test": "tests/pricing.rs::applies_discount_above_threshold",
    "verify_command": "ripr agent verify --root . --before target/ripr/pilot/repo-exposure.json --after target/ripr/pilot/after.repo-exposure.json --json",
    "agent_command": "ripr agent start --root . --seam-id 67fc764ba37d77bd --out target/ripr/workflow"
  },
  "history": {
    "source": ".ripr/pr-evidence-ledger.jsonl",
    "records": 42,
    "waiver_age_max_days": 14,
    "baseline_resolved_total": 45,
    "new_policy_eligible_total": 3,
    "trend": "improving"
  },
  "warnings": [
    "coverage input is optional and does not determine pass/fail"
  ],
  "limits_note": "Read-only advisory PR evidence ledger over existing static RIPR artifacts; gate-decision remains the pass/fail authority."
}
```

Field contract:

- `schema_version` - currently `"0.1"`.
- `status` - `advisory` for complete reports and `incomplete` when PR identity
  or all evidence sources are missing.
- `movement.*` - copied or derived from existing gate, baseline delta, and RIPR
  Zero status artifacts. The ledger must not recompute analyzer semantics.
- `gate.pass_fail_authority` - names `ripr gate evaluate` whenever a gate
  decision is present.
- `waivers[]` - PR-time visible acknowledgement records. Waivers do not hide
  findings and do not become suppressions. `waivers[].canonical_gap_id` is
  copied from source artifacts when available and remains `null` otherwise.
- `suppressions[]` - durable policy exceptions. Suppressions are not waivers
  and are not baseline debt. `suppressions[].canonical_gap_id` is copied from
  gate or baseline-delta evidence when available.
- `repair_receipts[]` - supplied outcome or agent receipt evidence.
  `repair_receipts[].receipt_state` carries the canonical receipt lifecycle
  label: `receipt_missing`, `receipt_found`, `receipt_stale`,
  `receipt_gap_mismatch`, `receipt_movement_improved`,
  `receipt_movement_unchanged`, or `receipt_not_applicable`.
  `repair_receipts[].static_movement` uses the same object shape as review
  guidance outcome receipts, including `state`, `source`, and `artifact`; the
  ledger must not infer receipt success from a missing artifact.
  `repair_receipts[].canonical_gap_id` is copied from receipts or
  recommendation provenance when supplied.
- `coverage_grip_frontier.status` - `available`, `not_available`, or
  `unsupported`.
- `coverage_grip_frontier.*` - keeps coverage movement separate from RIPR
  evidence movement. Coverage movement is execution evidence, not test
  adequacy.
- `top_repair_route` - copied from an explicit gap decision ledger when it
  supplies a repairable, stable Rust, PR-local gap record with a verification
  command. If no such gap record is present, the ledger falls back to existing
  PR guidance, RIPR Zero status, gate decision, agent packet, or receipt
  artifacts. Missing fields are `null` plus warnings, not invented.
  `top_repair_route.gap_id` and `top_repair_route.canonical_gap_id` are copied
  from the selected source artifact when available.
- `history.*` - present only when prior ledger history or previous ledger
  summary is supplied.
- `warnings[]` - missing inputs, unavailable coverage, unsupported schemas,
  ambiguous identities, and trend gaps.
- `limits_note` - advisory boundary text for generated CI summaries.

Markdown should fit in a generated CI job summary. It should show new
policy-eligible gaps, existing baseline gaps still present, baseline gaps
resolved, acknowledged gaps, suppressed gaps, blocking candidates, visible
unresolved gaps, the top focused test to add, receipt paths, coverage/grip
frontier status, and the advisory boundary. It must say that the PR evidence
ledger is advisory history and that gate decisions remain the pass/fail
authority.

See [PR evidence ledger workflow](PR_EVIDENCE_LEDGER_WORKFLOW.md) for how
teams read ledger records as waiver aging, baseline burn-down, repair receipts,
coverage/grip frontier signals, and movement toward RIPR 0.

## Coverage / Grip Frontier Report

`ripr coverage-grip frontier` writes an advisory report that keeps execution
coverage movement and static RIPR behavioral grip movement as separate axes. It
can consume a coverage summary plus any existing PR evidence ledger, baseline
debt delta, or RIPR Zero status report.

Command:

```text
ripr coverage-grip frontier \
  --coverage target/ripr/reports/coverage-summary.json \
  --ledger target/ripr/reports/pr-evidence-ledger.json \
  --baseline-delta target/ripr/reports/baseline-debt-delta.json \
  --zero-status target/ripr/reports/ripr-zero-status.json \
  --out target/ripr/reports/coverage-grip-frontier.json \
  --out-md target/ripr/reports/coverage-grip-frontier.md
```

The report writes:

```text
target/ripr/reports/coverage-grip-frontier.json
target/ripr/reports/coverage-grip-frontier.md
```

Coverage is optional. Without coverage input, the report still preserves RIPR
movement and marks the coverage axis `not_available`. The report must not
treat coverage as test adequacy, run mutation testing, change gate policy, post
comments, edit source, generate tests, call an LLM, or make CI blocking by
default.

JSON shape:

```json
{
  "schema_version": "0.1",
  "tool": "ripr",
  "kind": "coverage_grip_frontier",
  "status": "advisory",
  "root": ".",
  "generated_at": "2026-05-09T00:00:00Z",
  "inputs": {
    "coverage": "target/ripr/reports/coverage-summary.json",
    "pr_evidence_ledger": "target/ripr/reports/pr-evidence-ledger.json",
    "baseline_debt_delta": "target/ripr/reports/baseline-debt-delta.json",
    "ripr_zero_status": "target/ripr/reports/ripr-zero-status.json"
  },
  "coverage": {
    "status": "available",
    "delta_percent": "0.0",
    "source": "target/ripr/reports/coverage-summary.json"
  },
  "ripr": {
    "source": "pr_evidence_ledger",
    "new_policy_eligible": 1,
    "baseline_resolved": 3,
    "baseline_still_present": 2,
    "acknowledged": 1,
    "suppressed": 0,
    "blocking_candidates": 0,
    "visible_unresolved": 4,
    "visible_unresolved_delta": -3
  },
  "quadrants": {
    "covered_with_ripr_gap": 4,
    "covered_without_ripr_gap": 20,
    "uncovered_with_ripr_gap": 2,
    "uncovered_without_ripr_gap": 8
  },
  "interpretation": "behavioral grip improved without line-coverage movement",
  "warnings": [],
  "limits_note": "Coverage is execution evidence; RIPR is static behavioral grip evidence. This report is advisory and does not claim test adequacy or runtime mutation outcomes."
}
```

Markdown shape:

```md
# RIPR Coverage / Grip Frontier

Status: advisory

Coverage axis:
- Status: available
- Delta percent: 0.0

RIPR axis:
- Source: pr_evidence_ledger
- New policy-eligible gaps: 1
- Baseline gaps resolved: 3
- Visible unresolved gaps: 4
- Visible unresolved delta: -3

Interpretation:
- behavioral grip improved without line-coverage movement
```

The report may use these coverage inputs when present:

- `coverage_delta_percent`;
- `coverage.delta_percent`;
- `summary.coverage_delta_percent`;
- `ripr_visible_unresolved_delta`;
- `ripr.visible_unresolved_delta`;
- `quadrants.covered_with_ripr_gap`;
- `quadrants.covered_without_ripr_gap`;
- `quadrants.uncovered_with_ripr_gap`;
- `quadrants.uncovered_without_ripr_gap`.

## Test-Oracle Assistant Loop

RIPR-SPEC-0019 defines the end-to-end test-oracle assistant loop. `ripr
assistant-loop proof` writes an advisory proof report that joins existing PR
guidance, editor or agent handoff packets, before/after static evidence,
receipts, PR evidence ledgers, and optional gate or coverage/grip frontier
reports without changing analyzer identity, recommendation ranking, gate
policy, editor behavior, or CI defaults.

When an input artifact supplies the shared Lane 1 `evidence_record`, the proof
report prefers that record for selected seam identity, owner/location, grip
class, missing discriminator, assertion shape, related test, static limits, and
before/after movement classes. Legacy PR guidance, agent packet, receipt, and
repo-exposure fields remain fallback for older artifacts.

Command shape:

```text
ripr assistant-loop proof \
  --pr-guidance target/ripr/review/comments.json \
  --agent-packet target/ripr/workflow/agent-brief.json \
  --before target/ripr/pilot/repo-exposure.json \
  --after target/ripr/pilot/after.repo-exposure.json \
  --receipt target/ripr/reports/agent-receipt.json \
  --ledger target/ripr/reports/pr-evidence-ledger.json \
  --coverage-frontier target/ripr/reports/coverage-grip-frontier.json \
  --gate-decision target/ripr/reports/gate-decision.json \
  --out target/ripr/reports/test-oracle-assistant-proof.json \
  --out-md target/ripr/reports/test-oracle-assistant-proof.md
```

The report writes:

```text
target/ripr/reports/test-oracle-assistant-proof.json
target/ripr/reports/test-oracle-assistant-proof.md
```

Generated GitHub CI writes the same artifacts when the required PR guidance,
editor/agent brief, before/after static evidence, agent receipt, and PR
evidence ledger inputs already exist. The generated workflow treats the report
as advisory summary/artifact content only; it does not make the proof report a
pass/fail authority, post comments, mutate the baseline, rerun hidden analysis,
or print a placeholder when the required inputs are missing.

The report is advisory and read-only. It must not fail CI, post comments, edit
source, generate tests, call an LLM provider, run mutation testing, or claim
runtime confirmation from static evidence.

JSON shape:

```json
{
  "schema_version": "0.1",
  "tool": "ripr",
  "kind": "test_oracle_assistant_loop",
  "status": "advisory",
  "root": ".",
  "inputs": {
    "pr_guidance": "target/ripr/review/comments.json",
    "agent_packet": "target/ripr/workflow/agent-brief.json",
    "before": "target/ripr/pilot/repo-exposure.json",
    "after": "target/ripr/pilot/after.repo-exposure.json",
    "receipt": "target/ripr/reports/agent-receipt.json",
    "ledger": "target/ripr/reports/pr-evidence-ledger.json",
    "coverage_frontier": "target/ripr/reports/coverage-grip-frontier.json"
  },
  "seam": {
    "seam_id": "67fc764ba37d77bd",
    "canonical_gap_id": null,
    "owner": "pricing::discounted_total",
    "seam_kind": "predicate_boundary",
    "path": "src/pricing.rs",
    "line": 88,
    "grip_class": "weakly_gripped",
    "missing_discriminator": "amount == discount_threshold",
    "evidence_source": "evidence_record",
    "static_limitations": []
  },
  "recommendation": {
    "source": "evidence_record",
    "placement": "changed_line",
    "summary_only_reason": null,
    "suggested_test": "Add an equality-boundary assertion.",
    "related_test": "tests/pricing.rs::applies_discount_above_threshold",
    "assertion_shape": "assert_eq!(discounted_total(100, 100), 90)",
    "verify_command": "ripr agent verify --root . --before target/ripr/pilot/repo-exposure.json --after target/ripr/pilot/after.repo-exposure.json --json"
  },
  "handoff": {
    "source": "agent_packet",
    "artifact": "target/ripr/workflow/agent-brief.json",
    "agent_command": "ripr agent start --root . --seam-id 67fc764ba37d77bd --out target/ripr/workflow",
    "external_provider": false
  },
  "evidence_movement": {
    "state": "improved",
    "before_class": "weakly_gripped",
    "after_class": "strongly_gripped",
    "source": "agent_receipt",
    "artifact": "target/ripr/reports/agent-receipt.json"
  },
  "ci_projection": {
    "ledger": "target/ripr/reports/pr-evidence-ledger.json",
    "coverage_frontier": "target/ripr/reports/coverage-grip-frontier.json",
    "gate_decision": null,
    "pass_fail_authority": "gate decision when explicitly configured"
  },
  "warnings": [],
  "limits": {
    "advisory": true,
    "source_edits": false,
    "generated_tests": false,
    "external_service": false,
    "runtime_mutation_execution": false,
    "ci_blocking_default": false
  }
}
```

Field contract:

- `status` is `advisory` for complete proof records and `incomplete` when the
  selected seam or required before/after evidence is missing.
- `inputs.*` records explicit input paths. Missing optional inputs are `null`;
  missing or invalid supplied inputs produce a warning.
- `seam.*` is copied from existing RIPR evidence or guidance. When
  `evidence_record` is present in the agent packet or matching repo-exposure
  seam, `seam.evidence_source` is `evidence_record`, and the proof prefers the
  record's seam identity, canonical gap ID, owner, location, grip class,
  missing discriminator, and static limits. Otherwise
  `seam.evidence_source` is `legacy_fields`. The report must not recompute
  analyzer identity.
- `recommendation.placement` is `changed_line`, `summary_only`, or `unknown`.
  Summary-only guidance must remain visible.
- `recommendation.assertion_shape`, `recommendation.related_test`, and
  `recommendation.verify_command` prefer `evidence_record.recommendation`
  fields when available and otherwise use the legacy agent packet or PR
  guidance fields.
- `handoff.external_provider` is always `false`; RIPR emits packets but does
  not call a provider.
- `evidence_movement.state` is `improved`, `resolved`, `unchanged`,
  `regressed`, or `unknown`. It is static RIPR movement, not runtime mutation
  confirmation. Without a receipt, before/after class comparison prefers the
  matching repo-exposure `evidence_record.grip_class` and falls back to legacy
  seam `grip_class`.
- `ci_projection.pass_fail_authority` keeps proof records separate from
  optional gate decisions.
- `limits.*` preserves the no-edit, no-generated-test, no-provider-call,
  no-runtime-mutation-execution, and advisory-default boundaries.

Markdown should fit in a PR summary, generated CI job summary, or dogfood
receipt. It should show the selected seam, missing discriminator, suggested
focused test, related test, verify command, before/after static movement,
receipt path, ledger path, optional coverage/grip frontier path, assertion
shape, owner, and static limits.

See [Test-oracle assistant proof report](TEST_ORACLE_ASSISTANT_PROOF_REPORT.md)
for how reviewers, maintainers, and coding agents should read the report,
warnings, optional CI projection, and advisory limits.

## First Useful Action Report

RIPR-SPEC-0020 defines the first useful action report. `ripr first-action`
writes an advisory JSON and Markdown report that compresses existing
editor, PR guidance, gap decision ledger, PR evidence ledger, baseline,
assistant proof, receipt, optional gate, optional coverage/grip, and staleness
evidence into one next test action or one fallback reason. The report is
read-only and must not rerun hidden analysis, edit source, generate tests, call
a provider, run mutation testing, invent policy, or change default CI blocking.

See [First useful action workflow](FIRST_USEFUL_ACTION_WORKFLOW.md) for how
developers, reviewers, and coding agents read the report, act on the selected
action, verify static movement, emit receipts, and interpret fallback states.

The producer lives in `crates/ripr/src/output/first_useful_action.rs`; the
fixture corpus under `fixtures/boundary_gap/expected/first-useful-action/`
pins every bounded status plus expected JSON and Markdown routes.

Command shape:

```text
ripr first-action \
  --root . \
  --pr-guidance target/ripr/review/comments.json \
  --assistant-proof target/ripr/reports/test-oracle-assistant-proof.json \
  --gap-ledger target/ripr/reports/gap-decision-ledger.json \
  --ledger target/ripr/reports/pr-evidence-ledger.json \
  --baseline-delta target/ripr/reports/baseline-debt-delta.json \
  --receipt target/ripr/reports/agent-receipt.json \
  --gate-decision target/ripr/reports/gate-decision.json \
  --coverage-frontier target/ripr/reports/coverage-grip-frontier.json \
  --editor-context target/ripr/workflow/evidence-context.json \
  --out target/ripr/reports/first-useful-action.json \
  --out-md target/ripr/reports/first-useful-action.md
```

The report writes:

```text
target/ripr/reports/first-useful-action.json
target/ripr/reports/first-useful-action.md
```

JSON shape:

```json
{
  "schema_version": "0.1",
  "tool": "ripr",
  "kind": "first_useful_action",
  "status": "actionable",
  "audience": "developer",
  "action_kind": "write_focused_test",
  "root": ".",
  "generated_at": "2026-05-09T12:00:00Z",
  "inputs": {
    "pr_guidance": "target/ripr/review/comments.json",
    "assistant_proof": "target/ripr/reports/test-oracle-assistant-proof.json",
    "gap_ledger": "target/ripr/reports/gap-decision-ledger.json",
    "ledger": "target/ripr/reports/pr-evidence-ledger.json",
    "baseline_delta": "target/ripr/reports/baseline-debt-delta.json",
    "receipt": "target/ripr/reports/agent-receipt.json",
    "gate_decision": "target/ripr/reports/gate-decision.json",
    "coverage_frontier": "target/ripr/reports/coverage-grip-frontier.json",
    "editor_context": "target/ripr/workflow/evidence-context.json"
  },
  "selected": {
    "source": "assistant_proof",
    "source_artifact": "target/ripr/reports/test-oracle-assistant-proof.json",
    "seam_id": "67fc764ba37d77bd",
    "seam_kind": "predicate_boundary",
    "path": "src/pricing.rs",
    "line": 88,
    "classification": "weakly_exposed",
    "missing_discriminator": "amount == discount_threshold",
    "gap_id": "gap:pr:pricing:threshold-boundary",
    "canonical_gap_id": "gap:rust:pricing:discount:threshold-boundary",
    "repair_route": "AddBoundaryAssertion"
  },
  "title": "Add equality-boundary discriminator test",
  "why": "Changed predicate boundary is weakly exposed and lacks an equality-boundary discriminator.",
  "why_first": [
    "The seam is PR-local.",
    "The assistant proof report links guidance, handoff, before/after evidence, and receipt inputs.",
    "No waiver, acknowledgement, or suppression applies."
  ],
  "target": {
    "file": "tests/pricing.rs",
    "related_test": "below_threshold_has_no_discount",
    "suggested_test_name": "discounted_total_boundary_discriminator",
    "suggested_assertion": "Assert the exact returned discount at the equality boundary."
  },
  "commands": {
    "context_packet": "ripr agent packet --root . --seam-id 67fc764ba37d77bd --json",
    "after_snapshot": "ripr check --root . --mode draft --format repo-exposure-json > target/ripr/workflow/after.repo-exposure.json",
    "verify": "ripr agent verify --root . --before target/ripr/workflow/before.repo-exposure.json --after target/ripr/workflow/after.repo-exposure.json --json",
    "receipt": "ripr agent receipt --root . --verify-json target/ripr/workflow/agent-verify.json --seam-id 67fc764ba37d77bd --json"
  },
  "evidence": {
    "pr_guidance": "target/ripr/review/comments.json",
    "assistant_proof": "target/ripr/reports/test-oracle-assistant-proof.json",
    "gap_ledger": "target/ripr/reports/gap-decision-ledger.json",
    "receipt": "target/ripr/reports/agent-receipt.json",
    "ledger": "target/ripr/reports/pr-evidence-ledger.json",
    "static_movement": "unknown"
  },
  "fallback": null,
  "warnings": [],
  "limits": [
    "Static evidence only.",
    "Does not prove runtime adequacy.",
    "Does not run mutation testing.",
    "Does not edit source or generate tests.",
    "Does not make CI blocking by default."
  ]
}
```

Field contract:

- `schema_version` is `0.1` until the report shape changes.
- `kind` is always `first_useful_action`.
- `status` is one of `actionable`, `stale`,
  `missing_required_artifact`, `baseline_only`, `acknowledged`, `waived`,
  `suppressed`, `no_actionable_seam`, `already_improved`, or
  `unchanged_after_attempt`.
- `action_kind` is one of `write_focused_test`, `refresh_evidence`,
  `generate_missing_artifact`, `acknowledge_baseline`, `inspect_proof_report`,
  `revise_focused_test`, or `no_action`.
- `audience` is `developer`, `reviewer`, or `agent`.
- `inputs.*` records explicit input paths. Missing optional inputs are `null`
  or omitted for additive unsupplied fields; missing or invalid supplied inputs
  produce warnings and an appropriate fallback status.
- `selected.*` is copied from existing RIPR artifacts. The report must not
  mint a new seam identity or rerank findings with a provider.
- `selected.gap_id`, `selected.canonical_gap_id`, and
  `selected.repair_route` are present when an explicit gap decision ledger
  drives the first action.
- `why_first` records deterministic routing reasons. It must not be an opaque
  score.
- `target.*` records the recommended test file, related test, suggested test
  name, and assertion shape when supplied by existing artifacts.
- `commands.*` records copyable commands from existing command templates or
  supplied artifacts. Missing commands become `null` and warnings.
- `evidence.*` records supporting artifact paths and static movement when
  supplied. Static movement is not runtime mutation confirmation.
- `fallback` records the reason for non-actionable statuses and the next safe
  command when available.
- `limits` preserves static-evidence, no-edit, no-generated-test,
  no-provider-call, no-runtime-mutation-execution, and advisory-default
  boundaries.

Markdown should fit in a PR summary, generated CI job summary, or editor status
detail. It should show status, audience, action kind, top action, deterministic
why-first reasons, target file, related test, suggested test name, verification
command, receipt command, supporting artifact paths, warnings, fallback reason
when present, and static limits.

Generated CI runs `ripr first-action` only when one or more explicit upstream
RIPR artifacts already exist, uploads `first-useful-action.{json,md}` with the
normal report packet, and appends a compact at-a-glance summary. The projection
is advisory: `ripr gate evaluate` remains the only configured pass/fail
authority, and the first-action report must not edit source, generate tests,
call a provider, run mutation testing, rerun hidden analysis, or change default
blocking.

The VS Code extension may also read an existing
`target/ripr/reports/first-useful-action.json` and project the selected action
through the status bar and `ripr: Show Status`. That editor projection is not a
schema producer: it does not run `ripr first-action`, add diagnostics, edit
source, generate tests, call providers, run mutation testing, or make gate
decisions.

## Assistant Loop Health Report

RIPR-SPEC-0022 defines the assistant-loop-health report contract.
`ripr assistant-loop health` reads one or more explicit
`test-oracle-assistant-proof.json` paths and writes advisory JSON and Markdown
that summarize proof completeness, missing inputs, static movement, warnings,
and bounded repair queues. The report is read-only and must not rerun hidden
analysis, inspect source to infer missing fields, edit source, generate tests,
call providers, run mutation testing, change recommendation ranking, change gate
policy, or change default CI blocking.

Command shape:

```text
ripr assistant-loop health \
  --proof target/ripr/reports/test-oracle-assistant-proof.json \
  --out target/ripr/reports/assistant-loop-health.json \
  --out-md target/ripr/reports/assistant-loop-health.md
```

The report writes:

```text
target/ripr/reports/assistant-loop-health.json
target/ripr/reports/assistant-loop-health.md
```

Fixture corpus:

```text
fixtures/boundary_gap/expected/assistant-loop-health/
```

The corpus pins complete-improved, partial-missing-optional,
missing-required-input, unchanged, regressed, warning-heavy, and multi-proof
health reports plus representative proof inputs for producer tests.

JSON shape:

```json
{
  "schema_version": "0.1",
  "tool": "ripr",
  "kind": "assistant_loop_health",
  "status": "advisory",
  "root": ".",
  "generated_at": "2026-05-09T12:00:00Z",
  "inputs": {
    "proofs": [
      "target/ripr/reports/test-oracle-assistant-proof.json"
    ]
  },
  "summary": {
    "proofs": 1,
    "complete": 0,
    "partial": 1,
    "missing_required_input": 0,
    "missing_optional_input": 1,
    "improved": 1,
    "unchanged": 0,
    "regressed": 0,
    "unknown_movement": 0,
    "warnings": 2,
    "repair_queue": 1
  },
  "proofs": [
    {
      "id": "proof-67fc764ba37d77bd",
      "source_artifact": "target/ripr/reports/test-oracle-assistant-proof.json",
      "proof_state": "partial",
      "movement_state": "improved",
      "seam": {
        "seam_id": "67fc764ba37d77bd",
        "seam_kind": "predicate_boundary",
        "path": "src/pricing.rs",
        "line": 88,
        "grip_class": "weakly_gripped",
        "missing_discriminator": "amount == discount_threshold"
      },
      "recommendation": {
        "placement": "changed_line",
        "related_test": "tests/pricing.rs::applies_discount_above_threshold",
        "suggested_test": "Add an equality-boundary assertion.",
        "verify_command": "ripr agent verify --root . --before target/ripr/pilot/repo-exposure.json --after target/ripr/pilot/after.repo-exposure.json --json"
      },
      "handoff": {
        "artifact": "target/ripr/workflow/agent-brief.json",
        "agent_command": "ripr agent start --root . --seam-id 67fc764ba37d77bd --out target/ripr/workflow"
      },
      "receipt": {
        "artifact": null,
        "status": "missing"
      },
      "movement": {
        "before_class": "weakly_gripped",
        "after_class": "strongly_gripped",
        "source": "agent_receipt"
      },
      "optional_context": {
        "ledger": "target/ripr/reports/pr-evidence-ledger.json",
        "gate_decision": null,
        "coverage_frontier": "target/ripr/reports/coverage-grip-frontier.json",
        "first_useful_action": "target/ripr/reports/first-useful-action.json"
      },
      "warnings": [
        {
          "kind": "missing_optional_input",
          "message": "No gate decision input was supplied.",
          "source_artifact": null
        },
        {
          "kind": "missing_receipt",
          "message": "No receipt was supplied for the repair attempt.",
          "source_artifact": "target/ripr/reports/test-oracle-assistant-proof.json"
        }
      ]
    }
  ],
  "warning_summary": [
    {
      "kind": "missing_optional_input",
      "count": 1,
      "examples": [
        "No gate decision input was supplied."
      ]
    },
    {
      "kind": "missing_receipt",
      "count": 1,
      "examples": [
        "No receipt was supplied for the repair attempt."
      ]
    }
  ],
  "repair_queue": [
    {
      "repair_kind": "rerun_verify_and_receipt",
      "source_artifact": "target/ripr/reports/test-oracle-assistant-proof.json",
      "seam_id": "67fc764ba37d77bd",
      "path": "src/pricing.rs",
      "line": 88,
      "reason": "Proof packet is missing an agent receipt.",
      "next_command": "ripr agent receipt --root . --verify-json target/ripr/workflow/agent-verify.json --seam-id 67fc764ba37d77bd --json",
      "expected_result": "Attach a receipt so reviewers can inspect static before/after movement."
    }
  ],
  "limits": [
    "Static RIPR evidence only.",
    "Does not provide runtime confirmation.",
    "Does not run mutation testing.",
    "Does not call providers.",
    "Does not edit source or generate tests.",
    "Does not change default CI blocking.",
    "Gate evaluator remains pass/fail authority."
  ]
}
```

Field contract:

- `schema_version` is `0.1` until the report shape changes.
- `kind` is always `assistant_loop_health`.
- `status` is `advisory` when at least one proof path was read and summarized,
  or `incomplete` when no proof path could be read.
- `inputs.proofs` records explicit proof paths in deterministic order.
- `summary.*` counts proof states, movement buckets, warnings, and repair queue
  entries. Counts are derived from report items; they are not an opaque score.
- `proofs[].proof_state` is `complete`, `partial`, or
  `missing_required_input`.
- `proofs[].movement_state` is `improved`, `unchanged`, `regressed`, or
  `unknown`. A source proof movement of `resolved` is counted as `improved`.
- `proofs[].seam`, `recommendation`, `handoff`, `receipt`, and `movement`
  fields are copied from existing proof artifacts when present. The health
  report must not mint seam identities or rerank recommendations.
- `optional_context.*` records optional artifact paths when the proof names
  them. Missing optional paths are `null` and may also appear as warnings.
- `warnings[].kind` is one of `missing_required_input`,
  `missing_optional_input`, `stale_input`, `malformed_input`,
  `incompatible_schema`, `summary_only_guidance`, `unchanged_movement`,
  `regressed_movement`, `missing_receipt`, `missing_handoff`,
  `unknown_movement`, `static_limit`, or `other`.
- `repair_queue[].repair_kind` is one of `regenerate_proof`,
  `regenerate_missing_artifact`, `rerun_verify_and_receipt`,
  `refresh_before_after_evidence`, `inspect_unchanged_attempt`,
  `inspect_regression`, `inspect_summary_only_guidance`, `attach_receipt`, or
  `no_repair`.
- `limits` preserves static-evidence, no-edit, no-generated-test,
  no-provider-call, no-runtime-mutation-execution, and advisory-default
  boundaries.

Markdown should fit in a generated CI job summary or reviewer handoff. It
should show status, complete/partial/missing proof counts, movement counts, top
warning kinds, bounded repair queue entries that include `repair_kind`, and
advisory limits. For example, a repair row should begin with
`rerun_verify_and_receipt` before the file and reason. If no proof input can be
read, Markdown should show `Status: incomplete` and put the repair instruction
before empty counts.

Generated CI runs `ripr assistant-loop health` only when proof artifacts exist,
uploads `assistant-loop-health.{json,md}` with the normal report packet, and
appends a compact summary. The projection is advisory: `ripr gate evaluate`
remains the only configured pass/fail authority.

See [Assistant loop health workflow](ASSISTANT_LOOP_HEALTH_WORKFLOW.md) for how
maintainers and coding agents read complete versus partial proof packets,
missing-input repairs, unchanged movement, generated-CI summaries, and advisory
limits.

## PR Review Front Panel Report

RIPR-SPEC-0023 defines the PR review front-panel report contract. The
`ripr pr-review front-panel` producer reads explicit existing RIPR artifacts and
writes advisory JSON and Markdown that summarize the PR's top test-oracle issue,
policy state, baseline movement, repair route, receipt state, optional
calibration, optional coverage/grip context, and artifact groups. The report is
read-only and does not rerun hidden analysis, inspect source to infer missing
fields, edit source, generate tests, call providers, run mutation testing,
change recommendation ranking, change gate policy, publish inline comments, or
change default CI blocking.

Command shape:

```text
ripr pr-review front-panel \
  --root . \
  --pr-guidance target/ripr/review/comments.json \
  --first-action target/ripr/reports/first-useful-action.json \
  --assistant-proof target/ripr/reports/test-oracle-assistant-proof.json \
  --assistant-health target/ripr/reports/assistant-loop-health.json \
  --ledger target/ripr/reports/pr-evidence-ledger.json \
  --baseline-delta target/ripr/reports/baseline-debt-delta.json \
  --zero-status target/ripr/reports/ripr-zero-status.json \
  --gate-decision target/ripr/reports/gate-decision.json \
  --recommendation-calibration target/ripr/reports/recommendation-calibration.json \
  --mutation-calibration target/ripr/reports/mutation-calibration.json \
  --coverage-frontier target/ripr/reports/coverage-grip-frontier.json \
  --receipt target/ripr/reports/agent-receipt.json \
  --out target/ripr/reports/pr-review-front-panel.json \
  --out-md target/ripr/reports/pr-review-front-panel.md
```

The report writes:

```text
target/ripr/reports/pr-review-front-panel.json
target/ripr/reports/pr-review-front-panel.md
```

JSON shape:

```json
{
  "schema_version": "0.1",
  "tool": "ripr",
  "kind": "pr_review_front_panel",
  "status": "advisory",
  "root": ".",
  "generated_at": "2026-05-09T12:00:00Z",
  "inputs": {
    "pr_guidance": "target/ripr/review/comments.json",
    "first_action": "target/ripr/reports/first-useful-action.json",
    "assistant_proof": "target/ripr/reports/test-oracle-assistant-proof.json",
    "assistant_health": "target/ripr/reports/assistant-loop-health.json",
    "ledger": "target/ripr/reports/pr-evidence-ledger.json",
    "baseline_delta": "target/ripr/reports/baseline-debt-delta.json",
    "zero_status": "target/ripr/reports/ripr-zero-status.json",
    "gate_decision": "target/ripr/reports/gate-decision.json",
    "recommendation_calibration": "target/ripr/reports/recommendation-calibration.json",
    "mutation_calibration": null,
    "coverage_frontier": "target/ripr/reports/coverage-grip-frontier.json",
    "receipt": "target/ripr/reports/agent-receipt.json"
  },
  "summary": {
    "status": "advisory",
    "headline": "Add equality-boundary discriminator test.",
    "top_issue_state": "actionable",
    "policy_state": "new_policy_eligible",
    "placement": "changed_line",
    "movement_state": "unknown",
    "coverage_grip_state": "not_available",
    "blocking_candidates": 0,
    "acknowledged": 0,
    "suppressed": 0,
    "new_policy_eligible": 1,
    "baseline_still_present": 42,
    "baseline_resolved": 3
  },
  "top_issue": {
    "source": "first_useful_action",
    "source_artifact": "target/ripr/reports/first-useful-action.json",
    "seam_id": "67fc764ba37d77bd",
    "canonical_gap_id": "gap-67fc764ba37d77bd",
    "path": "src/pricing.rs",
    "line": 88,
    "classification": "weakly_exposed",
    "current_evidence_strength": "Static evidence found related test context, but the current check is weak because the discriminator is missing.",
    "missing_discriminator": "amount == discount_threshold",
    "focused_proof_intent": "Add an equality-boundary assertion.",
    "related_test": "tests/pricing.rs::applies_discount_above_threshold",
    "suggested_test": "Add an equality-boundary assertion.",
    "verify_command": "ripr agent verify --root . --before target/ripr/workflow/before.repo-exposure.json --after target/ripr/workflow/after.repo-exposure.json --json",
    "receipt_command": "ripr agent receipt --root . --verify-json target/ripr/workflow/agent-verify.json --seam-id 67fc764ba37d77bd --json",
    "static_evidence_boundary": "static advisory evidence only; not runtime proof, coverage adequacy, mutation confirmation, gate approval, or merge approval.",
    "agent_command": "ripr agent start --root . --seam-id 67fc764ba37d77bd --out target/ripr/workflow",
    "receipt": {
      "artifact": "target/ripr/reports/agent-receipt.json",
      "status": "present"
    }
  },
  "movement": {
    "state": "unknown",
    "before_class": null,
    "after_class": null,
    "source_artifact": null
  },
  "debt_delta": {
    "new_policy_eligible": 1,
    "baseline_still_present": 42,
    "baseline_resolved": 3,
    "acknowledged": 0,
    "waived": 0,
    "suppressed": 0,
    "blocking_candidates": 0
  },
  "policy": {
    "mode": "visible-only",
    "decision": "advisory",
    "authority_artifact": "target/ripr/reports/gate-decision.json",
    "acknowledgement_label": "ripr-waive"
  },
  "calibration": {
    "recommendation": "unknown",
    "mutation": "not_available",
    "source_artifacts": [
      "target/ripr/reports/recommendation-calibration.json"
    ]
  },
  "coverage_grip": {
    "state": "not_available",
    "coverage_delta": null,
    "grip_delta": null,
    "source_artifact": "target/ripr/reports/coverage-grip-frontier.json"
  },
  "artifacts": [
    {
      "group": "start_here",
      "label": "PR review front panel",
      "path": "target/ripr/reports/pr-review-front-panel.md",
      "available": true,
      "required": true
    },
    {
      "group": "repair",
      "label": "Assistant proof",
      "path": "target/ripr/reports/test-oracle-assistant-proof.md",
      "available": true,
      "required": false
    },
    {
      "group": "policy",
      "label": "Gate decision",
      "path": "target/ripr/reports/gate-decision.md",
      "available": true,
      "required": false
    }
  ],
  "warnings": [],
  "limits": [
    "Static RIPR evidence only.",
    "Does not provide runtime confirmation.",
    "Does not run mutation testing.",
    "Does not call providers.",
    "Does not edit source or generate tests.",
    "Does not publish inline comments.",
    "Does not change default CI blocking.",
    "Gate evaluator remains pass/fail authority."
  ]
}
```

Field contract:

- `schema_version` remains `0.1`; additive top-issue projection fields preserve
  existing consumers and carry the same first-screen vocabulary used by
  `first-pr`.
- `kind` is always `pr_review_front_panel`.
- `status` is `advisory`, `pass`, `acknowledged`, `blocked`,
  `config_error`, or `incomplete`.
- `summary.top_issue_state` is `actionable`, `summary_only`,
  `baseline_only`, `already_improved`, `unchanged_after_attempt`,
  `no_actionable_seam`, `missing_required_input`, or `stale_input`.
- `summary.policy_state` is `none`, `new_policy_eligible`, `baseline`,
  `acknowledged`, `waived`, `suppressed`, `blocking`, or `config_error`.
- `summary.placement` is `changed_line`, `summary_only`, or
  `not_available`.
- `summary.movement_state` is `improved`, `resolved`, `unchanged`,
  `regressed`, `unknown`, or `not_available`.
- `summary.coverage_grip_state` is `not_available`,
  `flat_coverage_grip_improved`, `coverage_and_grip_improved`,
  `coverage_improved_grip_unchanged`, `coverage_regressed`, or `unknown`.
- `inputs.*` records explicit input paths. Missing optional paths are `null`;
  missing recommended paths produce warnings or fallback states.
- `summary.*` carries first-screen counts and states derived from supplied
  artifacts. It must not be an opaque score.
- `top_issue.*` is copied from existing RIPR artifacts. The front panel must
  not mint seam identities, rerank recommendations with a model, or infer
  missing source facts from code.
- `top_issue.current_evidence_strength`,
  `top_issue.missing_discriminator`, `top_issue.focused_proof_intent`,
  `top_issue.verify_command`, `top_issue.receipt_command`, and
  `top_issue.static_evidence_boundary` are the typed one-screen repair
  vocabulary. They mirror the CLI first screen when the supplied artifacts
  carry the field. For `first_useful_action` inputs, current evidence strength
  must come from `selected.current_evidence_strength`. Legacy PR guidance,
  gate, baseline, and assistant-health inputs may normalize existing typed
  class/status fields, but must not infer the value from Markdown prose or code
  inspection.
- `movement.*` preserves before/after static movement when supplied. It is not
  runtime mutation confirmation.
- `debt_delta.*` carries PR-local movement from baseline, RIPR Zero, gate, or
  ledger inputs when available.
- `policy.authority_artifact` records the gate decision path when supplied.
  Gate decision remains the only configured pass/fail authority.
- `calibration.*` and `coverage_grip.*` are advisory context. They must not
  become adequacy or blocking claims.
- `artifacts[].group` is `start_here`, `repair`, `evidence`, `policy`,
  `calibration`, or `generated_ci`.
- `artifacts[]` groups known artifacts by reviewer use. Missing artifacts stay
  visible with `available = false`.
- `warnings[].kind` is one of `missing_required_input`,
  `missing_optional_input`, `stale_input`, `malformed_input`,
  `incompatible_schema`, `summary_only_guidance`, `missing_receipt`,
  `missing_handoff`, `missing_gate_decision`, `missing_calibration`,
  `static_limit`, `config_error`, or `other`.
- `limits` preserves static-evidence, no-edit, no-generated-test,
  no-provider-call, no-runtime-mutation-execution, no-inline-comment, and
  advisory-default boundaries.

Markdown should fit in a generated GitHub job summary. It should show status,
top issue, current evidence strength, missing discriminator, focused proof
intent, suggested focused test, related test, baseline and PR movement, policy
state, repair commands, receipt command/state, artifact groups, and advisory
limits. For fallback states, Markdown should put the safe next step before
lower-priority detail. For example, missing required inputs should say to
regenerate the missing PR guidance or first-useful-action artifact before
acting on the panel.

Generated CI runs the producer only when configured input artifacts exist,
uploads `pr-review-front-panel.{json,md}` with the normal report packet, and
appends the Markdown plus compact at-a-glance fields to the job summary. The
projection is advisory: `ripr gate evaluate` remains the only configured
pass/fail authority. See
[PR review front panel workflow](PR_REVIEW_FRONT_PANEL_WORKFLOW.md) for the
reviewer, developer, maintainer, and coding-agent workflow over the generated
panel.

## Report Packet Index

RIPR-SPEC-0024 defines the report packet index contract. The index is the
reviewer front door for the uploaded `ripr-reports` packet. It groups explicit
existing artifacts by reviewer use, identifies the recommended start-here
artifact, preserves missing or warning surfaces, and names commands that
regenerate missing expected artifacts when the command is known.

The report is advisory and read-only. It does not rerun hidden analysis, inspect
source to infer missing fields, edit source, generate tests, call providers, run
mutation testing, change recommendation ranking, change gate policy, publish
inline comments, or change default CI blocking. `gate-decision.{json,md}`
remains the only configured pass/fail authority.

Command shape:

```text
ripr reports index \
  --root . \
  --reports-dir target/ripr/reports \
  --review-dir target/ripr/review \
  --receipts-dir target/ripr/receipts \
  --workflow-dir target/ripr/workflow \
  --agent-dir target/ripr/agent \
  --pilot-dir target/ripr/pilot \
  --ci-dir target/ci \
  --out target/ripr/reports/index.json \
  --out-md target/ripr/reports/index.md
```

Repo-local automation may keep `cargo xtask reports index` as a wrapper.
Generated GitHub CI uses the public `ripr reports index` command when indexed
artifacts exist and projects the resulting index into the advisory summary.

The report writes:

```text
target/ripr/reports/index.json
target/ripr/reports/index.md
```

JSON shape:

```json
{
  "schema_version": "0.1",
  "tool": "ripr",
  "kind": "report_packet_index",
  "status": "warn",
  "root": ".",
  "generated_at": "2026-05-10T12:00:00Z",
  "inputs": {
    "reports_dir": "target/ripr/reports",
    "review_dir": "target/ripr/review",
    "receipts_dir": "target/ripr/receipts",
    "workflow_dir": "target/ripr/workflow",
    "agent_dir": "target/ripr/agent",
    "pilot_dir": "target/ripr/pilot",
    "ci_dir": "target/ci"
  },
  "summary": {
    "entries": 18,
    "available": 14,
    "missing_expected": 4,
    "warnings": 3,
    "failures": 0,
    "start_here": "target/ripr/reports/start-here.md",
    "gate_authority": "target/ripr/reports/gate-decision.md",
    "advisory": true
  },
  "groups": [
    {
      "group": "start_here",
      "label": "Start here",
      "summary": "Reviewer-first PR story.",
      "entries": [
        {
          "id": "first_pr_start_here",
          "label": "First PR start here",
          "kind": "markdown",
          "path": "target/ripr/reports/start-here.md",
          "json_path": "target/ripr/reports/start-here.json",
          "status": "available",
          "available": true,
          "required": true,
          "authority": false,
          "description": "Canonical first-screen repair packet.",
          "next_command": null
        },
        {
          "id": "pr_review_front_panel",
          "label": "PR review front panel",
          "kind": "markdown",
          "path": "target/ripr/reports/pr-review-front-panel.md",
          "json_path": "target/ripr/reports/pr-review-front-panel.json",
          "status": "available",
          "available": true,
          "required": true,
          "authority": false,
          "description": "First-screen PR review story.",
          "next_command": null
        }
      ]
    }
  ],
  "repo_ops_packets": [
    {
      "id": "gh_pr_status",
      "label": "PR merge readiness",
      "status": "warn",
      "next_command": "cargo xtask gh-pr-status --pr <number>",
      "description": "Summarizes one PR's merge state, checks, reviews, and safe next action.",
      "artifacts": [
        {
          "path": "target/ripr/reports/gh-pr-status.md",
          "status": "warn",
          "available": true
        },
        {
          "path": "target/ripr/reports/gh-pr-status.json",
          "status": "warn",
          "available": true
        }
      ]
    }
  ],
  "lane1_readiness": {
    "status": "warn",
    "missing_artifacts": 2,
    "warning_artifacts": 0,
    "failing_artifacts": 0,
    "packets": [
      {
        "id": "lane1_evidence_audit",
        "label": "Lane 1 evidence audit",
        "status": "missing",
        "next_command": "cargo xtask lane1-evidence-audit",
        "description": "Produces raw-to-canonical/actionability counts and actionable-gap packet inputs.",
        "artifacts": [
          {
            "path": "target/ripr/reports/lane1-evidence-audit.json",
            "status": "missing",
            "available": false
          },
          {
            "path": "target/ripr/reports/lane1-evidence-audit.md",
            "status": "missing",
            "available": false
          }
        ]
      }
    ]
  },
  "missing_expected": [
    {
      "id": "assistant_loop_health",
      "label": "Assistant loop health",
      "group": "repair_agent_handoff",
      "path": "target/ripr/reports/assistant-loop-health.md",
      "required": false,
      "reason": "input_not_available",
      "next_command": "ripr assistant-loop health --proof target/ripr/reports/test-oracle-assistant-proof.json --out target/ripr/reports/assistant-loop-health.json --out-md target/ripr/reports/assistant-loop-health.md"
    }
  ],
  "warnings": [
    {
      "kind": "missing_expected",
      "message": "Assistant loop health was not generated because no proof input was present.",
      "source_artifact": null
    }
  ],
  "limits": [
    "Advisory report-packet index only.",
    "Does not rerun analysis.",
    "Does not edit source or generate tests.",
    "Does not call providers.",
    "Does not run mutation testing.",
    "Does not publish inline comments.",
    "Does not change default CI blocking.",
    "Gate decision remains pass/fail authority when configured."
  ]
}
```

Field contract:

- `schema_version` is `0.1` until the report shape changes.
- `kind` is always `report_packet_index`.
- `status` is `pass`, `warn`, `fail`, or `incomplete`. This is packet-health
  context only, not gate authority.
- `inputs.*` records explicit directories and paths that the producer was
  allowed to inspect.
- `summary.start_here` names the first artifact to show reviewers. Prefer
  `pr-review-front-panel.md` when available.
- `summary.gate_authority` records `gate-decision.md` when supplied. The index
  itself never becomes gate authority.
- `groups[].group` is `start_here`, `pr_review_story`,
  `repair_agent_handoff`, `evidence_movement`, `policy_gates`, `calibration`,
  `validation_receipts`, `sarif_badges`, or `local_context`.
- `groups[].entries[]` records artifact id, label, kind, path, optional JSON
  sibling, status, availability, requiredness, authority, description, and
  next command.
- `repo_ops_packets[]` is the repo-local operating packet index used by
  `cargo xtask reports index`. It records command mutability, the repo
  cockpit, worktree doctor, PR-ready, PR triage, per-PR merge readiness,
  generated-clean, badge diff policy, command catalog coverage, critic,
  receipts, suggested-fixes, and `check-pr` artifacts with status, known output
  paths, and regeneration commands. It is advisory front-door metadata only and
  never becomes gate authority.
- `lane1_readiness` is the Lane 1 evidence packet index used by
  `cargo xtask reports index`. It records whether evidence-health, Lane 1
  evidence-audit/actionable-gap, evidence-quality scorecard, evidence-quality
  trend, and badge-basis packets are present and healthy. Missing, warning, or
  failing Lane 1 readiness artifacts add advisory next commands, but do not
  create gate authority, badge authority, runtime mutation proof, or coverage
  adequacy claims.

Report packet index field contract:

- `entries[].status` is `available`, `missing`, `pass`, `warn`, `fail`,
  `actionable`, `blocked`, `acknowledged`, `suppressed`, `stale`, `incomplete`,
  `unreadable`, or `not_applicable`.
- `lane1_readiness.status` is `present`, `warn`, or `fail`.
- `lane1_readiness.missing_artifacts`,
  `lane1_readiness.warning_artifacts`, and
  `lane1_readiness.failing_artifacts` are counts over the packet artifacts.
- `lane1_readiness.packets[]` uses the same packet shape as
  `repo_ops_packets[]`: id, label, status, next command, description, and
  artifact availability.
- `missing_expected[].reason` is `not_generated`, `input_not_available`,
  `configured_off`, `missing_required_input`, `stale_upstream`, or `unknown`.
- `missing_expected[]` keeps absent expected surfaces visible with a bounded
  reason and, when known, a command to regenerate the missing surface.
- `warnings[]` carries malformed, stale, unreadable, missing-input, and
  incomplete packet context without converting it to waiver, suppression,
  improvement, runtime confirmation, or pass/fail authority.
- `limits` preserves read-only, explicit-input, no-source-edit,
  no-generated-test, no-provider-call, no-runtime-mutation-execution,
  no-inline-comment, and advisory-default boundaries.

`target/ripr/reports/pr-triage.json` is the open-board hygiene packet emitted by
`cargo xtask pr-triage-report`:

```json
{
  "schema_version": "0.1",
  "mode": "advisory",
  "status": "warn",
  "open_prs": [],
  "queue_disposition": [
    {
      "pr_number": 819,
      "disposition": "needs_owner_decision",
      "reason": "duplicate or stale work needs canonical owner selection",
      "recommended_action": "Choose the canonical branch, refresh the stale draft, or close superseded variants."
    }
  ],
  "findings": [],
  "recommended_actions": []
}
```

Field contract:

- `queue_disposition[].disposition` is advisory. It may be
  `merge_candidate`, `needs_rebase`, `needs_review`, `close_duplicate`,
  `superseded`, `needs_fresh_validation`, `needs_owner_decision`, or
  `do_not_touch_wrong_lane`.
- `queue_disposition[]` never closes, updates, merges, comments on, or mutates
  PRs. It translates triage findings into operator next-action vocabulary.
- `findings[]` remains the detailed evidence; `recommended_actions[]` keeps the
  grouped repair guidance for agents and maintainers.

`target/ripr/reports/pr-ready.json` is the local PR readiness cockpit emitted by
`cargo xtask pr-ready`:

```json
{
  "schema_version": "0.1",
  "mode": "advisory",
  "status": "actionable",
  "next_action": "review the attention items, then run cargo xtask check-pr for full gate receipts",
  "steps": [
    {
      "id": "worktree_doctor",
      "command": "cargo xtask worktree doctor",
      "status": "pass",
      "required": true,
      "report": "target/ripr/reports/worktree-doctor.md",
      "summary": "completed"
    }
  ],
  "safe_repairs": ["run cargo xtask fix-pr"],
  "generated_only": ["target/ripr/**"],
  "judgment_required": ["golden blessing"],
  "next_commands": ["cargo xtask check-pr"]
}
```

Field contract:

- `status` is `pass`, `actionable`, or `fail`. `fail` means a required local
  hygiene step failed; `actionable` means a non-blocking packet needs attention.
- `steps[].required` records whether a failed step makes `pr-ready` exit
  nonzero.
- `safe_repairs[]` lists deterministic repair paths; it must not include badge
  value edits, golden blessing, baselines, suppressions, dependency exceptions,
  schema version changes, or policy authority changes.

`target/ripr/reports/cockpit.json` is the repo-level maintainer cockpit emitted
by `cargo xtask cockpit`:

```json
{
  "schema_version": "0.1",
  "mode": "advisory",
  "status": "actionable",
  "next_action": "review the action queue, then run cargo xtask pr-ready or cargo xtask check-pr for the active PR",
  "action_queue": ["review stale, duplicate, behind, policy-sensitive, or generated-artifact PRs"],
  "steps": [
    {
      "id": "pr_triage",
      "command": "cargo xtask pr-triage-report",
      "status": "needs_attention",
      "required": false,
      "report": "target/ripr/reports/pr-triage.md",
      "summary": "report status: warn; see target/ripr/reports/pr-triage.md"
    }
  ],
  "safe_repairs": ["run cargo xtask fix-pr"],
  "generated_only": ["target/ripr/**"],
  "judgment_required": ["branch protection"],
  "next_commands": ["cargo xtask pr-ready", "cargo xtask check-pr"]
}
```

Field contract:

- `status` is `pass`, `actionable`, or `fail`. `fail` means a required
  repo-ops rail failed; `actionable` means the cockpit found advisory queue,
  source-of-truth, generated-evidence, or command-catalog attention items.
- `action_queue[]` is the maintainer-facing next-work queue derived from the
  composed repo-ops packet statuses. It is advisory and must not close PRs,
  update branches, edit badges, or mutate policy.
- `steps[]` records each composed repo-ops command, whether it is required for
  cockpit success, and the report path to inspect.
- `safe_repairs[]`, `generated_only[]`, and `judgment_required[]` preserve the
  generated-evidence discipline boundary: deterministic cleanup is allowed,
  while badge refreshes, goldens, suppressions, baselines, dependency
  exceptions, branch protection, and policy authority remain human decisions.

Markdown should fit in a generated GitHub job summary and uploaded report
packet. It should show status, start-here artifact, gate authority, packet
summary counts, grouped artifact links, missing expected artifacts with next
commands, and advisory limits. When no useful packet map can be rendered,
Markdown should show `Status: incomplete` and put the regeneration instruction
before empty groups.

Generated GitHub CI may run the index producer after individual report
producers and before artifact upload, upload `index.{json,md}` with the normal
`ripr-reports` artifact, and append a compact index section to the job summary.
The projection is advisory. Missing optional index entries must not fail CI
unless the explicit gate decision already failed or reported `config_error`.
See [Report packet index workflow](REPORT_PACKET_INDEX_WORKFLOW.md) for
reviewer, maintainer, developer, and coding-agent use of the generated packet
map.

## First PR Start Here Packet

`ripr first-pr` writes the first successful PR front-door packet from explicit
existing RIPR artifacts. `cargo xtask first-pr` remains a repo-local wrapper
over the same public command. The packet selects one top repairable
PR-local Rust gap when the gap decision ledger supplies one, or emits a bounded
no-action or blocked recovery state. It does not rerun hidden analysis, edit
source, generate tests, call providers, run mutation testing, change gate
policy, or change CI blocking.

Command shape:

```text
ripr first-pr \
  --root . \
  --base origin/main \
  --head HEAD \
  --gap-ledger target/ripr/reports/gap-decision-ledger.json \
  --first-action target/ripr/reports/first-useful-action.json \
  --review-comments target/ripr/review/comments.json \
  --agent-packet target/ripr/agent/gap-packet.md \
  --gate-decision target/ripr/reports/gate-decision.json \
  --receipts-dir target/ripr/receipts \
  --out-dir target/ripr/reports
```

The command writes:

```text
target/ripr/reports/start-here.json
target/ripr/reports/start-here.md
```

JSON shape:

```json
{
  "schema_version": "0.1",
  "tool": "ripr",
  "kind": "first_pr_start_here",
  "status": "blocked",
  "posture": "advisory",
  "root": ".",
  "selected": {
    "state": "stale_artifact",
    "output_state": "stale_evidence",
    "message": "The gap decision ledger is stale; refresh the first-run evidence before assigning repair work.",
    "next_command": "ripr reports gap-ledger --repo-exposure target/ripr/reports/repo-exposure.json --out target/ripr/reports/gap-decision-ledger.json --out-md target/ripr/reports/gap-decision-ledger.md"
  },
  "preflight": {
    "status": "needs_attention",
    "mode": "write",
    "root": ".",
    "base": "origin/main",
    "head": "HEAD",
    "next_command": "git fetch origin main; then rerun `ripr first-pr --root . --base origin/main --head HEAD`.",
    "checks": [
      {
        "id": "git_base",
        "label": "Git base",
        "status": "needs_attention",
        "message": "Could not resolve `origin/main` to a commit.",
        "path": null,
        "next_command": "git fetch origin main; then rerun `ripr first-pr --root . --base origin/main --head HEAD`."
      }
    ]
  },
  "commands": {
    "regenerate_gap_ledger": "ripr reports gap-ledger --repo-exposure target/ripr/reports/repo-exposure.json --out target/ripr/reports/gap-decision-ledger.json --out-md target/ripr/reports/gap-decision-ledger.md",
    "next": "ripr reports gap-ledger --repo-exposure target/ripr/reports/repo-exposure.json --out target/ripr/reports/gap-decision-ledger.json --out-md target/ripr/reports/gap-decision-ledger.md"
  },
  "artifacts": [
    {
      "id": "gap_ledger",
      "label": "Gap decision ledger",
      "path": "target/ripr/reports/gap-decision-ledger.json",
      "status": "present",
      "regeneration_command": "ripr reports gap-ledger --repo-exposure target/ripr/reports/repo-exposure.json --out target/ripr/reports/gap-decision-ledger.json --out-md target/ripr/reports/gap-decision-ledger.md"
    }
  ],
  "authority": {
    "status": "advisory",
    "gate_decision": "target/ripr/reports/gate-decision.json",
    "boundary": "Pass/fail authority remains with explicit gate-decision artifacts when configured; this first-run packet does not gate."
  },
  "warnings": [
    "The gap decision ledger is stale; refresh the first-run evidence before assigning repair work."
  ],
  "limits": [
    "Composes explicit RIPR artifacts only.",
    "Does not run hidden analysis.",
    "Does not edit source or generate tests.",
    "Does not run mutation testing.",
    "Does not change CI blocking or gate policy."
  ]
}
```

Field contract:

- `schema_version` is `0.1` until the packet shape changes.
- `kind` is always `first_pr_start_here`.
- `status` is `actionable`, `blocked`, or `no_action`. It is reviewer context
  only, not gate authority.
- `posture` is always `advisory`.
- `selected.state` is `top_gap` for a selected repairable gap,
  `missing_artifact`, `malformed_artifact`, `stale_artifact`, `wrong_root`
  (including selected-root preflight failures), `blocked_artifact`, or `timeout`
  for blocked recovery states, and
  `empty_diff` or `no_action` for no-action states.
- `selected.output_state` is the canonical no-output/fail-closed state for
  machine consumers. It distinguishes `actionable_gap`, `clean`,
  `no_actionable_gap`, `missing_artifacts`, `stale_evidence`, `wrong_root`,
  `language_disabled`, `adapter_unavailable`, `preview_disabled`,
  `preview_limited`, `malformed_artifact`, `timeout_partial`,
  `server_unavailable`, `unsupported_schema`, `unsafe_path`, and
  `unsafe_command` where the packet has that data.
- `top_gap` requires `status = "actionable"`.
- `selected.canonical_gap_id` and `selected.gap_id` identify the repair unit
  when a top gap is selected. Generated CI and report indexes should prefer the
  canonical gap id when present.
- `selected.language` and `selected.language_status` keep Rust stable evidence
  distinct from preview evidence when a top gap is selected.
- `selected.current_evidence_strength`,
  `selected.missing_discriminator`, and `selected.focused_proof_intent`
  provide the one-screen recommendation contract. They are derived from typed
  gap kind and repair-route fields; consumers must not infer them from
  Markdown prose.
- `selected.why` explains why the selected gap matters in reviewer language.
  It is supporting explanation for the same typed repair unit, not a separate
  authority source.
- `selected.static_evidence_boundary` repeats the static/advisory non-claim
  boundary in the selected top-gap object so one-screen consumers do not need
  to infer it from Markdown or higher-level packet authority text.
- `selected.repair.route`, `selected.repair.target_file`,
  `selected.repair.related_test`, and `selected.repair.suggested_assertion`
  describe the bounded repair route when present.
- `selected.static_limit_kind` and `selected.static_limit_detail` are optional;
  surfaces must show them before suggested action language when they are
  present.
- `selected.verify_command`, `selected.receipt_command`,
  `selected.receipt_path`, `selected.receipt_command_source`, and
  `selected.receipt_state` are the static movement proof path. When the source
  gap ledger omits a receipt command, `ripr first-pr` may provide a deterministic
  `ripr outcome` command under the configured receipts directory. A missing
  receipt is not failure, merge approval, mutation proof, or runtime adequacy.
  `selected.receipt_state` uses the canonical receipt lifecycle vocabulary:
  `receipt_missing`, `receipt_found`, `receipt_stale`,
  `receipt_gap_mismatch`, `receipt_movement_improved`,
  `receipt_movement_unchanged`, or `receipt_not_applicable`.
- `missing_artifact`, `malformed_artifact`, `stale_artifact`, `wrong_root`,
  `blocked_artifact`, and `timeout` require `status = "blocked"` and a
  bounded next command when one is known.
- `blocked_artifact` may also represent setup preflight failures such as a
  missing git worktree, missing base ref, missing head ref, or invalid diff
  range.
- `empty_diff` and `no_action` require `status = "no_action"` and must not
  produce a repair interruption.
- `preflight` is present for the public `ripr first-pr` command path. It
  records read-only front-door checks for root, Git worktree, base/head refs,
  diff presence, Cargo workspace, `ripr.toml` defaulting, output directory, and
  write/check mode. It does not create analyzer facts and does not become gate
  authority.
- `preflight.status` is `ready` when the command can proceed without setup
  attention, or `needs_attention` when a setup check has a recovery/no-action
  note. A `needs_attention` preflight can still accompany an explicit
  artifact-backed packet; typed artifact states still decide repair
  selection.
- `preflight.checks[].status` is one of `ok`, `needs_attention`, `no_action`,
  `defaulted`, or `will_create`. Checks with `next_command` provide the next
  safe setup or recovery command; they must not imply mutation, coverage,
  runtime proof, merge approval, or gate pass/fail.
- `commands.regenerate_gap_ledger` is always present so missing, stale,
  wrong-root, malformed, and timeout states can point to a known refresh path.
- `artifacts[]` records artifact id, label, path, `present` or `missing`
  status, and optional regeneration command.
- `authority.boundary` preserves the gate boundary. This packet never becomes
  pass/fail authority.
- `warnings[]` carries blocked-state context without converting it to waiver,
  suppression, improvement, clean, or gate-passing state.
- `limits` preserves explicit-input, no-source-edit, no-generated-test,
  no-provider-call, no-runtime-mutation-execution, and advisory-default
  boundaries.

Markdown should fit in a PR summary, local handoff, or generated CI summary. It
should show the selected top gap, no-action state, or blocked recovery state
first. For a top gap, the first screen must include changed behavior, why it
matters, current evidence strength, missing discriminator, focused proof intent,
verify command, receipt command or path, and the static advisory boundary
before deeper artifact links.
`empty_diff` must render as a no-action state, not a blocked repair.

### Review Guidance Outcome Receipt

Review guidance outcome receipts are optional repo-local inputs to the
recommendation calibration report. They record reviewer, fixture, agent, or CI
artifact feedback for one PR guidance item without sending telemetry, calling an
external service, editing source, generating tests, running mutation testing, or
changing CI blocking behavior.

Receipt files may live anywhere a repo chooses. The boundary-gap calibration
corpus pins examples under:

```text
fixtures/boundary_gap/expected/recommendation-calibration/outcome-receipts/
```

JSON shape:

```json
{
  "schema_version": "0.1",
  "tool": "ripr",
  "kind": "review_guidance_outcome_receipt",
  "status": "advisory",
  "spec": "RIPR-SPEC-0013",
  "receipt_id": "review-outcome-useful-exact-line-boundary",
  "case_id": "useful_exact_line_boundary",
  "root": ".",
  "guidance": {
    "artifact": "fixtures/boundary_gap/expected/pr-guidance/exact-line/comments.json",
    "collection": "comments",
    "id": "ripr-review-8f7fa8644fd12280",
    "seam_id": "8f7fa8644fd12280",
    "dedupe_key": "ripr:8f7fa8644fd12280:src/pricing.rs:88"
  },
  "outcome": {
    "label": "useful",
    "source": "fixture",
    "reason": "The recommendation points at the changed seam line and names the expected boundary discriminator and test target."
  },
  "placement": {
    "path": "src/pricing.rs",
    "line": 88,
    "mode": "exact_seam_line",
    "quality": "correct"
  },
  "suggested_test": {
    "target_quality": "correct",
    "expected_file": "tests/pricing.rs",
    "actual_file": "tests/pricing.rs",
    "near_test": "applies_discount_above_threshold"
  },
  "suppression": {
    "reason": null,
    "quality": "not_applicable"
  },
  "static_movement": {
    "state": "improved",
    "source": "targeted_test_outcome",
    "artifact": "fixtures/boundary_gap/calibration/targeted-test-outcome.json"
  },
  "latency": {
    "guidance_generated_unix_ms": null,
    "outcome_recorded_unix_ms": null,
    "outcome_latency_ms": null
  },
  "limits": {
    "telemetry": false,
    "external_service": false,
    "source_edits": false,
    "generated_tests": false,
    "runtime_mutation_execution": false,
    "ci_blocking": false
  },
  "limits_note": "Advisory review-guidance outcome receipt only; no telemetry, generated tests, source edits, mutation execution, or CI blocking."
}
```

Field contract:

- `schema_version` - currently `"0.1"`.
- `tool` - always `"ripr"`.
- `kind` - always `review_guidance_outcome_receipt`.
- `status` - `advisory`; receipts are feedback artifacts, not policy gates.
- `spec` - `RIPR-SPEC-0013`.
- `receipt_id` - stable local identifier for this receipt.
- `case_id` - optional calibration-corpus case identifier when the receipt
  came from a fixture expectation.
- `root` - workspace root used to resolve artifact paths.
- `guidance` - the source PR guidance artifact, collection, item id,
  `seam_id`, and dedupe key when available.
- `outcome.label` - one of `useful`, `noisy`, `wrong_line`,
  `already_covered`, `wrong_target`, `summary_only_correct`,
  `suppressed_correctly`, or `unknown`.
- `outcome.source` - local source class such as `fixture`, `reviewer`,
  `agent`, `ci_artifact`, or `unknown`.
- `outcome.reason` - concise local rationale for the outcome label.
- `placement.quality` - `correct`, `wrong_line`,
  `summary_only_expected`, `not_placeable`, or `unknown`.
- `suggested_test.target_quality` - `correct`, `wrong_target`,
  `not_applicable`, or `unknown`.
- `suppression.reason` - `cap_reached`, `suppression`, `severity_off`,
  `nearby_test_changed`, `generated_or_migration`, `none`, or `unknown`.
- `suppression.quality` - `suppressed_correctly`, `over_suppressed`,
  `not_applicable`, or `unknown`.
- `static_movement.state` - `improved`, `unchanged`, `regressed`, `resolved`,
  `new_gap`, `missing_after_snapshot`, or `unknown`.
- `latency.*` - optional timestamps and elapsed time. Values are `null` when
  not available.
- `limits` - explicit false values for telemetry, external services, source
  edits, generated tests, runtime mutation execution, and CI blocking.
- `limits_note` - static/advisory boundary text for downstream summaries.

## Agent Status

`ripr agent status --root <workspace>` reads already-written agent-loop
artifacts and reports which step is missing next. Markdown is the default for
human review packets; add `--json` for the machine-readable contract:

```text
ripr agent status --root .
ripr agent status --root . --json
```

The command does not run analysis, mutation testing, SARIF policy, badge
generation, LSP refresh, or cache warm-up. It only inspects fixed artifact
paths under the supplied workspace root:

```text
target/ripr/workflow/before.repo-exposure.json
target/ripr/workflow/after.repo-exposure.json
target/ripr/workflow/agent-brief.json
target/ripr/workflow/agent-packet.json
target/ripr/workflow/agent-verify.json
target/ripr/reports/agent-receipt.json
```

JSON shape:

```json
{
  "schema_version": "0.1",
  "tool": "ripr",
  "status": "incomplete",
  "root": ".",
  "seam": {
    "seam_id": "67fc764ba37d77bd",
    "source": "agent_receipt"
  },
  "artifacts": [
    {
      "name": "before_snapshot",
      "label": "before snapshot",
      "path": "target/ripr/workflow/before.repo-exposure.json",
      "required": true,
      "state": "present",
      "bytes": 12000,
      "modified_unix_ms": 1778179200000
    }
  ],
  "missing_commands": [
    {
      "step": "agent_packet",
      "artifact": "target/ripr/workflow/agent-packet.json",
      "reason": "agent packet artifact is missing",
      "command": "ripr agent packet --root . --seam-id 67fc764ba37d77bd --json > target/ripr/workflow/agent-packet.json"
    }
  ],
  "next_command": {
    "step": "agent_packet",
    "artifact": "target/ripr/workflow/agent-packet.json",
    "reason": "agent packet artifact is missing",
    "command": "ripr agent packet --root . --seam-id 67fc764ba37d77bd --json > target/ripr/workflow/agent-packet.json"
  },
  "warnings": []
}
```

Field contract:

- `schema_version` - currently `"0.1"`.
- `status` - `"complete"` when every required artifact is present and there
  are no warnings; `"warning"` when every artifact is present but a
  stale-looking condition exists; `"incomplete"` when any required artifact is
  missing.
- `root` - the `--root` argument normalized to forward slashes for reporting.
- `seam` - recovered seam identity when available. The current recovery order
  is receipt, verify, packet, then brief. It is `null` when no existing
  artifact names a seam.
- `artifacts[]` - one entry for each required fixed artifact. `bytes` and
  `modified_unix_ms` are `null` when the artifact is missing or the filesystem
  does not expose the timestamp.
- `missing_commands[]` - one command for each missing artifact in workflow
  order: before snapshot, packet, brief, after snapshot, verify, receipt. If no
  seam can be recovered, packet, brief, and receipt commands use `<seam-id>`.
- `next_command` - the first entry from `missing_commands`, or `null` when no
  required artifact is missing.
- `warnings[]` - stale-looking or unreadable-artifact hints. Timestamp warnings
  are emitted when `agent verify` is older than a before/after snapshot or
  `agent receipt` is older than `agent verify`. Hash mismatch warnings remain a
  later reviewer-summary/status enhancement now that receipt provenance records
  artifact SHA-256 values.

Markdown output contains the same status, recovered seam, artifact table, next
command, warnings, and static-only limits. Generated CI writes it to
`target/ripr/workflow/agent-status.md` next to
`target/ripr/workflow/agent-status.json`.

## Agent Review Summary

`ripr agent review-summary --root <workspace>` reads already-written agent-loop
artifacts and emits a compact Markdown packet for PR review. Add `--json` for
the machine-readable contract:

```text
ripr agent review-summary --root .
ripr agent review-summary --root . --json
```

The command does not run analysis, mutation testing, SARIF policy, badge
generation, LSP refresh, cache warm-up, source edits, or test generation. It
joins only existing artifacts:

- `ripr agent status` computed from the current artifact tree;
- `target/ripr/workflow/workflow.json` when present;
- `target/ripr/reports/agent-receipt.json`;
- `target/ripr/reports/operator-cockpit.json` when present;
- `target/ripr/reports/repo-exposure.json` when present;
- `target/ripr/reports/lsp-cockpit.json` when present;
- local file presence for CI-published work-loop artifacts.

The JSON schema is version `0.1`:

```json
{
  "schema_version": "0.1",
  "tool": "ripr",
  "status": "ready",
  "root": ".",
  "target_seam": {
    "seam_id": "67fc764ba37d77bd",
    "source": "agent_receipt",
    "file": "src/lib.rs",
    "line": 42,
    "seam_kind": "predicate_boundary"
  },
  "static_movement": {
    "state": "improved",
    "before_class": "weakly_gripped",
    "after_class": "strongly_gripped",
    "grip_class": "strongly_gripped",
    "evidence_artifact": "target/ripr/reports/agent-receipt.json",
    "verify_artifact": "target/ripr/workflow/agent-verify.json",
    "summary": "Static movement is improved (weakly_gripped -> strongly_gripped).",
    "next_action": {
      "kind": "improved",
      "summary": "Static grip improved.",
      "recommended_action": "Keep the focused test and include this receipt in review."
    }
  },
  "next_command": null,
  "surfaces": [
    {
      "name": "agent_status",
      "label": "Agent status",
      "path": "target/ripr/workflow/agent-status.json",
      "state": "computed",
      "status": "complete",
      "required": true,
      "summary": "6 required artifacts present, 0 missing, 0 warnings."
    }
  ],
  "ci_artifacts": [
    {
      "name": "agent_status",
      "path": "target/ripr/workflow/agent-status.json",
      "state": "present"
    },
    {
      "name": "agent_status_markdown",
      "path": "target/ripr/workflow/agent-status.md",
      "state": "present"
    },
    {
      "name": "agent_review_summary",
      "path": "target/ripr/workflow/agent-review-summary.json",
      "state": "missing"
    },
    {
      "name": "agent_review_summary_markdown",
      "path": "target/ripr/workflow/agent-review-summary.md",
      "state": "missing"
    }
  ],
  "reviewer_summary": {
    "headline": "Review packet is ready for seam 67fc764ba37d77bd.",
    "what_changed": "Static movement is improved (weakly_gripped -> strongly_gripped).",
    "evidence": "Review target/ripr/reports/agent-receipt.json with target/ripr/workflow/agent-verify.json.",
    "remaining": "Keep the focused test and include this receipt in review.",
    "reviewer_should_inspect": [
      "target/ripr/reports/agent-receipt.json",
      "target/ripr/workflow/agent-verify.json"
    ]
  },
  "limits": {
    "static_artifact_relationship": true,
    "runtime_mutation_execution": false,
    "automatic_edits": false,
    "generated_tests": false
  }
}
```

Field notes:

- `status` is `ready` when a receipt is present and required loop artifacts do
  not report warnings; `warning` when a receipt exists but local artifact state
  looks stale or malformed; `incomplete` when the receipt is missing.
- `target_seam` is recovered from receipt first, then workflow, then agent
  status.
- `static_movement` is copied from the receipt and remains a static
  before/after artifact relationship.
- `surfaces[]` reports each joined surface as `computed`, `present`, `missing`,
  `optional_missing`, or `invalid_json`.
- `ci_artifacts[]` is local file presence for artifacts that generated CI can
  upload later; it does not query GitHub Actions.
- `reviewer_summary` is intentionally compact enough for PR comments and LLM
  context windows.

The Markdown output contains the same target seam, movement, evidence artifact,
next command when one is missing, reviewer inspection list, and static limits.

## Agent Workflow Manifest

`ripr agent start --root <workspace> --seam-id <id> --out <dir>` writes a
source-edit-free workflow packet for one visible seam:

```text
ripr agent start --root . --seam-id 67fc764ba37d77bd --out target/ripr/workflow
```

Outputs:

```text
target/ripr/workflow/workflow.json
target/ripr/workflow/commands.md
target/ripr/workflow/agent-brief.json
```

The command selects the requested seam with the same policy as
`ripr agent brief --seam-id`, writes a focused brief, then renders a workflow
manifest that names artifact paths and shared command templates for the static
before snapshot, agent packet, agent brief, after snapshot, verify, and
receipt steps. It does not edit source files, generate tests, call LLM APIs,
run mutation testing, refresh LSP state, or configure CI blocking.

JSON shape:

```json
{
  "schema_version": "0.1",
  "tool": "ripr",
  "status": "ready",
  "root": ".",
  "mode": "draft",
  "out_dir": "target/ripr/workflow",
  "seam": {
    "seam_id": "67fc764ba37d77bd",
    "file": "src/pricing.rs",
    "line": 88,
    "seam_kind": "predicate_boundary",
    "grip_class": "weakly_gripped",
    "why": "caller requested seam_id 67fc764ba37d77bd",
    "missing_discriminator": "amount == discount_threshold",
    "assertion_shape": "assert_eq!(...)",
    "recommended_test_file": "tests/pricing.rs",
    "recommended_test_name": "discount_threshold_equality_boundary_is_asserted",
    "related_test_to_imitate": "applies_discount_above_threshold"
  },
  "outputs": {
    "workflow_manifest": "target/ripr/workflow/workflow.json",
    "commands_markdown": "target/ripr/workflow/commands.md",
    "agent_brief": "target/ripr/workflow/agent-brief.json"
  },
  "artifacts": [
    {
      "name": "before_snapshot",
      "label": "before snapshot",
      "path": "target/ripr/workflow/before.repo-exposure.json",
      "required": true,
      "state": "missing"
    }
  ],
  "commands": [
    {
      "step": "before_snapshot",
      "artifact": "target/ripr/workflow/before.repo-exposure.json",
      "purpose": "Capture static seam evidence before editing tests.",
      "command": "ripr check --root . --mode draft --format repo-exposure-json > target/ripr/workflow/before.repo-exposure.json"
    }
  ],
  "missing_inputs": [
    {
      "step": "before_snapshot",
      "artifact": "target/ripr/workflow/before.repo-exposure.json",
      "purpose": "Capture static seam evidence before editing tests.",
      "command": "ripr check --root . --mode draft --format repo-exposure-json > target/ripr/workflow/before.repo-exposure.json"
    }
  ],
  "next_command": {
    "step": "before_snapshot",
    "artifact": "target/ripr/workflow/before.repo-exposure.json",
    "purpose": "Capture static seam evidence before editing tests.",
    "command": "ripr check --root . --mode draft --format repo-exposure-json > target/ripr/workflow/before.repo-exposure.json"
  },
  "boundaries": {
    "source_edits": false,
    "generated_tests": false,
    "runtime_mutation_execution": false,
    "llm_api_calls": false,
    "ci_blocking": false
  }
}
```

Field contract:

- `schema_version` - currently `"0.1"`.
- `status` - currently `"ready"` when the manifest was written.
- `root`, `mode`, and `out_dir` - the selected workspace root, effective
  analysis mode, and workflow output directory.
- `seam` - the selected seam fields copied from the generated agent brief.
- `outputs` - the three files written by `agent start`.
- `artifacts[]` - required downstream workflow inputs and outputs, marked
  `present` or `missing` at manifest creation time.
- `commands[]` - deterministic command templates for regenerating the
  workflow, capturing snapshots, rendering packet and brief artifacts,
  comparing before/after evidence, and writing a receipt.
- `missing_inputs[]` - the commands whose artifacts are currently missing.
- `next_command` - the first missing-input command, or `null` when all
  downstream artifacts are present.
- `boundaries` - explicit false-valued guardrails for source edits, generated
  tests, runtime mutation execution, LLM API calls, and CI blocking.

## Release Readiness Report

`cargo xtask release-readiness --version <version>` writes a Campaign 10
release-surface report to:

```text
target/ripr/reports/release-readiness.json
target/ripr/reports/release-readiness.md
```

The report checks repo artifacts and safe local commands for the 0.4
first-hour loop. It path-installs the local binary, verifies the public command
surface, runs the boundary-gap `ripr pilot`, `ripr outcome`, and
`ripr agent verify` snapshots, writes a focused `ripr agent receipt`, refreshes
repo-exposure latency and LSP cockpit reports, checks the advisory GitHub
workflow dry-run, and confirms VSIX and known-limit docs. It does not run
mutation testing, enable CI blocking, change analyzer classifications, or
expand LSP behavior.

JSON shape:

```json
{
  "schema_version": "0.1",
  "tool": "ripr",
  "report": "release-readiness",
  "version": "0.7.0",
  "status": "warn",
  "checks": [
    {
      "id": "installed-command-surface",
      "status": "pass",
      "required": true,
      "command": "target/ripr/release-readiness/install/bin/ripr --help",
      "summary": "installed binary exposes the public release-loop commands",
      "artifacts": [
        "target/ripr/release-readiness/install/bin/ripr"
      ],
      "details": []
    },
    {
      "id": "publish-dry-run",
      "status": "not_run",
      "required": false,
      "command": "cargo publish -p ripr --dry-run",
      "summary": "requested release version does not match the crate version yet",
      "artifacts": [],
      "details": [
        "requested version: 0.4.0; crates/ripr version: 0.3.1"
      ]
    }
  ],
  "next_commands": [
    "cargo publish -p ripr --dry-run"
  ]
}
```

Field contract:

- `schema_version` - currently `"0.1"`.
- `status` - `pass` when all checks pass, `warn` when any check is `warn` or
  `not_run` and no required check failed, and `fail` when a required check
  failed.
- `version` - requested release version from `--version`.
- `checks[].id` - stable check identifier such as `package-list`,
  `publish-dry-run`, `path-install`, `installed-command-surface`,
  `pilot-boundary-fixture`, `outcome-boundary-fixture`,
  `agent-verify-boundary-fixture`, `agent-receipt-boundary-fixture`,
  `repo-exposure-latency`, `lsp-cockpit`, `github-workflow-defaults`,
  `vsix-packaging-path`, or `known-limits-docs`.
- `checks[].status` - `pass`, `warn`, `fail`, or `not_run`.
- `checks[].required` - `true` for checks that must pass in the normal local
  readiness run. Release-only package and publish dry-run checks can be
  `not_run` and non-required until the version bump and clean release-prep tree
  make them safe to execute.
- `checks[].command` - command or dry-run surface that produced the signal.
- `checks[].summary` - short human-readable status.
- `checks[].artifacts` - generated or inspected artifacts for the check.
- `checks[].details` - optional bounded command output, missing fields, or
  skip reasons.
- `next_commands[]` - follow-up commands for non-passing checks, or the
  release-readiness command itself when everything passed.

The Markdown sibling prints the same check table, per-check details, artifacts,
and next commands for release review.

## Operator Cockpit Report

`cargo xtask operator-cockpit` joins existing repo-local report artifacts into
one next-action cockpit:

```text
target/ripr/reports/operator-cockpit.json
target/ripr/reports/operator-cockpit.md
```

The command reads current artifacts under `target/ripr/reports/`; it does not
rerun analysis, generate tests, mutate source files, or change static
classifications. Missing inputs are reported with the command that should
generate them. `cargo xtask operator-cockpit-report` remains an alias for
existing repo automation.

JSON shape:

```json
{
  "schema_version": "0.1",
  "tool": "ripr",
  "status": "warn",
  "inputs": [
    {
      "name": "repo exposure",
      "path": "target/ripr/reports/repo-exposure.json",
      "state": "present",
      "status": "present",
      "command": "cargo xtask repo-exposure-report",
      "required": true,
      "summary": "2 seams; 1 weakly_gripped, 0 ungripped, 0 reachable_unrevealed."
    },
    {
      "name": "LSP cockpit",
      "path": "target/ripr/reports/lsp-cockpit.json",
      "state": "present",
      "status": "pass",
      "command": "cargo xtask lsp-cockpit-report",
      "required": true,
      "summary": "1 fixture reports; 0 uncovered contributed commands."
    },
    {
      "name": "before snapshot",
      "path": "target/ripr/pilot/repo-exposure.json",
      "state": "present",
      "status": "present",
      "command": "ripr pilot --out target/ripr/pilot",
      "required": true,
      "summary": "2 seams; 1 weakly_gripped, 0 ungripped, 0 reachable_unrevealed."
    },
    {
      "name": "after snapshot",
      "path": "target/ripr/pilot/after.repo-exposure.json",
      "state": "present",
      "status": "present",
      "command": "ripr check --root . --mode draft --format repo-exposure-json > target/ripr/pilot/after.repo-exposure.json",
      "required": true,
      "summary": "2 seams; 0 weakly_gripped, 0 ungripped, 0 reachable_unrevealed."
    },
    {
      "name": "agent verify",
      "path": "target/ripr/agent/agent-verify.json",
      "state": "present",
      "status": "advisory",
      "command": "ripr agent verify --root . --before target/ripr/pilot/repo-exposure.json --after target/ripr/pilot/after.repo-exposure.json --json > target/ripr/agent/agent-verify.json",
      "required": true,
      "summary": "1 improved, 0 changed, 0 regressed, 1 unchanged seams."
    },
    {
      "name": "agent receipt",
      "path": "target/ripr/agent/agent-receipt.json",
      "state": "present",
      "status": "advisory",
      "command": "ripr agent receipt --root . --verify-json target/ripr/agent/agent-verify.json --seam-id <seam-id> --json --out target/ripr/agent/agent-receipt.json",
      "required": true,
      "summary": "Receipt for seam 67fc764ba37d77bd: improved; before weakly_gripped, after strongly_gripped. No remaining static gap is named by this receipt."
    },
    {
      "name": "SARIF policy",
      "path": "target/ripr/reports/sarif-policy.json",
      "state": "missing",
      "status": "missing",
      "command": "cargo xtask sarif-policy --current target/ripr/workflow/current.repo-sarif.json",
      "required": true,
      "summary": "Report has not been generated yet."
    },
    {
      "name": "badge status",
      "path": "target/ripr/reports/repo-ripr-badge.json",
      "state": "present",
      "status": "present",
      "command": "cargo xtask repo-badge-artifacts",
      "required": true,
      "summary": "Badge headline status is available."
    },
    {
      "name": "targeted-test outcome",
      "path": "target/ripr/reports/targeted-test-outcome.json",
      "state": "missing",
      "status": "missing",
      "command": "ripr outcome --before target/ripr/pilot/repo-exposure.json --after target/ripr/pilot/after.repo-exposure.json --format json --out target/ripr/reports/targeted-test-outcome.json",
      "required": true,
      "summary": "Report has not been generated yet."
    },
    {
      "name": "actionable gap outcomes",
      "path": "target/ripr/reports/actionable-gap-outcomes.json",
      "state": "missing",
      "status": "missing",
      "command": "cargo xtask actionable-gap-outcomes",
      "required": true,
      "summary": "Actionable packet outcome join has not been generated yet."
    },
    {
      "name": "mutation calibration",
      "path": "target/ripr/reports/mutation-calibration.json",
      "state": "optional_missing",
      "status": "optional",
      "command": "cargo xtask mutation-calibration . --mutants-json target/mutants/outcomes.json --repo-exposure-json target/ripr/reports/repo-exposure.json",
      "required": false,
      "summary": "Optional calibration report has not been generated."
    }
  ],
  "top_weak_seams": [
    {
      "seam_id": "67fc764ba37d77bd",
      "seam_kind": "predicate_boundary",
      "file": "src/lib.rs",
      "line": 42,
      "owner": "src/lib.rs::discounted_total",
      "expression": "amount >= discount_threshold",
      "grip_class": "weakly_gripped",
      "why_it_matters": "observed values do not include the equality-boundary case for this predicate",
      "suggested_next_targeted_test": "Add a focused predicate_boundary test for `src/lib.rs::discounted_total` that exercises `discount_threshold (equality boundary)` and asserts the observable result.",
      "best_related_test": {
        "name": "below_threshold_has_no_discount",
        "file": "tests/pricing.rs",
        "line": 12,
        "oracle_strength": "strong"
      }
    }
  ],
  "surface_alignment": [
    {
      "surface": "LSP cockpit",
      "state": "present",
      "status": "pass",
      "agreement": "editor_contract_green",
      "signal": "1 LSP fixture reports; 0 uncovered contributed VS Code commands.",
      "command": "cargo xtask lsp-cockpit-report"
    },
    {
      "surface": "agent verify",
      "state": "present",
      "status": "advisory",
      "agreement": "agent_verify_counts_available",
      "signal": "1 improved, 0 changed, 0 regressed, 1 unchanged seams.",
      "command": "ripr agent verify --root . --before target/ripr/pilot/repo-exposure.json --after target/ripr/pilot/after.repo-exposure.json --json > target/ripr/agent/agent-verify.json"
    },
    {
      "surface": "agent receipt",
      "state": "present",
      "status": "advisory",
      "agreement": "agent_receipt_available",
      "signal": "Receipt for seam 67fc764ba37d77bd: improved; before weakly_gripped, after strongly_gripped. No remaining static gap is named by this receipt.",
      "command": "ripr agent receipt --root . --verify-json target/ripr/agent/agent-verify.json --seam-id <seam-id> --json --out target/ripr/agent/agent-receipt.json"
    }
  ],
  "next_commands": [
    {
      "command": "ripr pilot --out target/ripr/pilot",
      "reason": "Open the top actionable seam packet and write one focused targeted test."
    }
  ]
}
```

Field contract:

- `schema_version` - currently `"0.1"`.
- top-level `status` - `"pass"` when all required inputs are present and no top
  weak seams require operator attention; `"warn"` when required inputs are
  missing, stale/unreadable, LSP cockpit status needs review, or actionable
  weak seams are visible.
- `inputs[]` - report inventory for repo exposure, LSP cockpit, before
  snapshot, after snapshot, agent verify, agent receipt, SARIF policy, badge
  status, targeted-test outcome, and optional mutation calibration.
  `state` is `present`, `missing`, `optional_missing`, `unreadable`, or
  `invalid_json`.
- `inputs[].required` - `true` for reports expected in the normal operator
  cockpit loop and `false` for optional mutation calibration.
- `inputs[].status` - when an artifact is present, this is copied from the
  artifact's top-level `status` field. If the source JSON has no `status`, the
  cockpit uses `"present"`. Missing required inputs use `"missing"`; missing
  optional inputs use `"optional"`; unreadable or invalid JSON inputs use
  `"warn"`. Source-specific values such as `"pass"`, `"warn"`,
  `"new_results"`, and `"advisory_missing_baseline"` are preserved.
- `top_weak_seams[]` - up to five headline-eligible repo exposure seams with
  operator-attention classes: `weakly_gripped`, `ungripped`,
  `reachable_unrevealed`, `activation_unknown`, `propagation_unknown`,
  `observation_unknown`, or `discrimination_unknown`.
- `surface_alignment[]` - per-surface status and an `agreement` string that
  states whether LSP, before/after snapshots, agent verify, agent receipt,
  SARIF, badge, targeted outcome, and calibration artifacts are available and
  aligned with the operator loop.
- `next_commands[]` - ordered commands to generate missing reports, inspect the
  top seam packet, capture the after snapshot, run agent verify, write an agent
  receipt, and write the before/after targeted-test receipt.

The Markdown sibling prints:

- `Top Weak Seams`, with each seam's ID, class, file, line, kind, why it
  matters, suggested next targeted test, and best related test when present.
- `Surface Alignment`, a table with `Surface`, `State`, `Status`, `Agreement`,
  and `Signal` columns for LSP, before/after snapshots, agent verify, agent
  receipt, SARIF, badge, targeted outcome, and calibration surfaces.
- `Inputs`, a table with `Report`, `Required`, `State`, and `Path` columns for
  every input artifact.
- `Next Commands`, an ordered list of commands to refresh missing reports,
  inspect the top seam packet, capture the after snapshot, run agent verify,
  write the agent receipt, and write the before/after targeted-test receipt.

## Agent Seam Packets

`ripr check --root . --format agent-seam-packets-json` emits per-seam
agent work orders for every headline-eligible classified seam. The
artifact lands at `target/ripr/reports/agent-seam-packets.json` when
generated via `cargo xtask agent-seam-packets`.

`ripr agent packet --root . --seam-id <id> --json` emits the same
`agent-seam-packets-json` envelope filtered to one visible seam. It does not
dump the full repo packet set. Missing seam IDs, non-actionable seam classes,
and seams whose configured severity is `off` return an actionable error.

`ripr agent packet --root . --gap-ledger <path> --gap-id <id> --json` emits
the same packet envelope from one explicit `GapRecord`, matched by `gap_id` or
`canonical_gap_id`. This mode does not rerun analysis and does not infer
projectability from raw classifications. The selected record must have
`projection_eligibility.agent_packet.eligible = true`, a `repair_route`, and
`verification_commands`. Records that are already observed, waived,
suppressed, preview-gating ineligible, or otherwise not agent-packet eligible
return an actionable error instead of a repair packet.

```json
{
  "schema_version": "0.3",
  "scope": "repo",
  "packets_total": 12565,
  "packets": [
    {
      "task": "write_targeted_test",
      "seam_id": "f3c9e4d21a0b7c88",
      "owner": "src/pricing.rs::discounted_total",
      "seam_kind": "predicate_boundary",
      "file": "src/pricing.rs",
      "line": 88,
      "changed_expression": "amount >= discount_threshold",
      "current_grip": "weakly_gripped",
      "headline_eligible": true,
      "recommended_test": {
        "name": "discounted_total_boundary_discriminator",
        "file": "tests/pricing.rs",
        "reason": "place the new targeted test next to the nearest strong related test"
      },
      "nearest_strong_test_to_imitate": {
        "name": "below_threshold_has_no_discount",
        "file": "tests/pricing.rs",
        "line": 12,
        "oracle_kind": "exact_value",
        "oracle_strength": "strong",
        "relation_reason": "direct_owner_call",
        "relation_confidence": "high",
        "reason": "nearest strong related test by ranked evidence"
      },
      "evidence": {
        "reach": "yes",
        "activate": "yes",
        "propagate": "yes",
        "observe": "yes",
        "discriminate": "weak"
      },
      "observed_values": ["50", "10000"],
      "missing_discriminators": [
        {
          "value": "discount_threshold (equality boundary)",
          "reason": "observed values do not include the equality-boundary case for this predicate"
        }
      ],
      "candidate_values": [
        {
          "value": "discount_threshold (equality boundary)",
          "reason": "observed values do not include the equality-boundary case for this predicate"
        }
      ],
      "missing_oracle_shape": "exact returned value assertion at the equality boundary",
      "assertion_shape": {
        "kind": "exact_return_value",
        "example": "assert_eq!(discounted_total(/* discount_threshold (equality boundary) */), /* expected */)"
      },
      "related_existing_tests": [
        {
          "name": "below_threshold_has_no_discount",
          "file": "tests/pricing.rs",
          "line": 12,
          "oracle_kind": "exact_value",
          "oracle_strength": "strong",
          "evidence_summary": "exact value assertion",
          "relation_reason": "direct_owner_call",
          "relation_confidence": "high"
        }
      ],
      "patterns_to_imitate": [
        {
          "name": "below_threshold_has_no_discount",
          "file": "tests/pricing.rs",
          "line": 12,
          "oracle_kind": "exact_value",
          "oracle_strength": "strong",
          "relation_reason": "direct_owner_call",
          "relation_confidence": "high",
          "reason": "strong exact_value oracle with high relation"
        }
      ],
      "patterns_to_avoid": [
        {
          "pattern": "adding another test with only already-observed values",
          "reason": "candidate values should include the missing discriminator"
        }
      ],
      "suggested_assertions": [
        "assert_eq!(discounted_total(/* discount_threshold (equality boundary) */), /* expected */)"
      ],
      "confidence": "high",
      "evidence_record": {
        "schema_version": "0.1",
        "seam_id": "f3c9e4d21a0b7c88",
        "canonical_gap_id": "gap:67fc764ba37d77bd",
        "canonical_gap_group_size": 1,
        "canonical_gap_reason": "same owner, seam kind, flow sink, missing discriminator, and assertion shape",
        "owner": "src/pricing.rs::discounted_total",
        "location": {
          "file": "src/pricing.rs",
          "line": 88
        },
        "seam_kind": "predicate_boundary",
        "grip_class": "weakly_gripped",
        "headline_eligible": true,
        "evidence_path": {
          "reach": {
            "state": "yes",
            "confidence": "high",
            "summary": "related test calls owner"
          },
          "activate": {
            "state": "yes",
            "confidence": "high",
            "summary": "related test supplies boundary-adjacent values"
          },
          "propagate": {
            "state": "yes",
            "confidence": "medium",
            "summary": "predicate flows to return value"
          },
          "observe": {
            "state": "yes",
            "confidence": "medium",
            "summary": "related test asserts returned value"
          },
          "discriminate": {
            "state": "weak",
            "confidence": "high",
            "summary": "equality boundary is not observed"
          }
        },
        "observed_values": [
          {
            "value": "50",
            "line": 12,
            "text": "discounted_total(50, 100)",
            "context": "function_argument"
          }
        ],
        "missing_discriminators": [
          {
            "value": "discount_threshold (equality boundary)",
            "reason": "observed values do not include the equality-boundary case for this predicate",
            "flow_sink": null
          }
        ],
        "related_tests_total": 1,
        "related_tests": [
          {
            "name": "below_threshold_has_no_discount",
            "file": "tests/pricing.rs",
            "line": 12,
            "oracle_kind": "exact_value",
            "oracle_strength": "strong",
            "evidence_summary": "exact value assertion",
            "relation_reason": "direct_owner_call",
            "relation_confidence": "high"
          }
        ],
        "recommendation": {
          "action": "write_targeted_test",
          "reason": "extend the nearest related test with the missing discriminator",
          "recommended_test": {
            "name": "discounted_total_boundary_discriminator",
            "file": "tests/pricing.rs",
            "reason": "place the new targeted test next to the nearest strong related test"
          },
          "nearest_test_to_imitate": null,
          "candidate_values": [
            {
              "value": "discount_threshold (equality boundary)",
              "reason": "observed values do not include the equality-boundary case for this predicate"
            }
          ],
          "assertion_shape": {
            "kind": "exact_return_value",
            "example": "assert_eq!(discounted_total(/* discount_threshold (equality boundary) */), /* expected */)"
          },
          "verify_command": "ripr agent verify --root . --before target/ripr/pilot/repo-exposure.json --after target/ripr/pilot/after.repo-exposure.json --json"
        },
        "actionability": {
          "class": "actionable_related_test_extension",
          "reason": "extend the nearest related test with the missing discriminator",
          "has_concrete_guidance": true,
          "signals": {
            "missing_discriminator": true,
            "candidate_value": true,
            "assertion_shape": true,
            "related_test": true,
            "recommended_test_target": true,
            "verification_command": true
          }
        },
        "calibration": {
          "availability": "not_imported",
          "confidence": "unknown",
          "agreement": "no_runtime_data"
        },
        "static_limitations": []
      },
      "runtime_confirmation": "optional cargo-mutants confirmation; ripr reports static evidence only",
      "static_evidence_boundary": "static advisory evidence only; not runtime proof, coverage adequacy, mutation confirmation, gate approval, or merge approval."
    }
  ]
}
```

Field contract:

- `schema_version` — currently `"0.3"`. Distinct from the repo-exposure
  report's `"0.2"` because the packet is a separate contract aimed at
  coding agents rather than reviewers. Bumping requires updating this
  section, the renderer (`crates/ripr/src/output/agent_seam_packets.rs`),
  and any downstream consumers in lockstep. `0.2` → `0.3`:
  `related_existing_tests[]` entries gained `relation_reason` and
  `relation_confidence` fields, and the array is now ranked
  highest-confidence first (`analysis/related-test-precision-v1`);
  `context/agent-seam-packets-v2` added `recommended_test`,
  `nearest_strong_test_to_imitate`, `candidate_values`,
  `assertion_shape`, `patterns_to_imitate`, `patterns_to_avoid`, and
  packet `confidence` without changing the version again because the
  in-flight `0.3` contract had not yet closed.
  Reason and confidence vocabularies are documented in the
  `repo-exposure.json` field contract above.
- `scope` — always `"repo"`, including the one-seam `ripr agent packet`
  expansion. The one-seam command is a filtered view of the repo packet
  contract, not a second packet schema.
- `source` - optional. Present as `"gap_decision_ledger"` when the packet was
  rendered from explicit `GapRecord` input rather than live seam analysis.
- `inputs.gap_ledger` - optional. Present with gap-ledger packet mode so
  reviewers and agents can trace the packet back to the artifact that owned
  projection eligibility.
- `packets_total` — number of actionable packets emitted. Equals the
  count of headline-eligible seams plus opaque seams (which emit
  `inspect_static_limitation`). Strongly-gripped, intentional, and
  suppressed seams produce no packet.
- `packets[].task` — `"write_targeted_test"` for headline-eligible
  seams; `"inspect_static_limitation"` for opaque seams. Future
  versions may add tasks like `"strengthen_oracle"` or
  `"add_match_arm_observer"`. Gap-ledger packet mode also uses
  `"write_targeted_test"` for repairable assertion routes,
  `"inspect_static_limitation"` for explicit inspection routes, and
  `"add_output_golden"` for `MissingOutputContract` records whose repair
  route is `AddOutputGolden`.
- `packets[].gap_id`, `packets[].canonical_gap_id`, `packets[].gap_kind`,
  `packets[].language`, `packets[].language_status`,
  `packets[].policy_state`, `packets[].gap_state`,
  `packets[].repairability` - optional GapRecord identity and policy fields.
  Present only when `source = "gap_decision_ledger"`.
- `packets[].current_evidence_strength` - optional GapRecord first-screen
  evidence summary in the form `<evidence_class> / <gap_state>`. Present only
  when `source = "gap_decision_ledger"` and mirrored in the copyable packet so
  agent work orders carry the same current-evidence vocabulary as `first-pr`.
- `packets[].static_evidence_boundary` - optional static/advisory non-claim
  string. Gap-ledger packet mode uses the same boundary text as `first-pr` so
  coding agents do not infer runtime mutation proof, coverage adequacy, gate
  approval, or merge approval from a repair packet.
- `packets[].anchor` - optional GapRecord anchor with `file`, `line`, `owner`,
  and `dedupe_fingerprint` when supplied by the ledger.
- `packets[].repair_route` - optional full GapRecord repair route. Present
  for gap-ledger packet mode and mirrors the source ledger instead of
  reconstructing repair intent from rendered prose.
- `packets[].verification_commands` and `packets[].verify_command` - optional
  GapRecord verification commands. `verify_command` is the first command and
  is provided for existing single-command consumers.
- `packets[].stop_conditions` - optional agent stop conditions. Gap-ledger
  packet mode uses the route's `stop_conditions` when supplied and otherwise
  adds bounded operational stop conditions so agents know when to stop instead
  of inventing a fix.
- `packets[].repair_card` - optional GapRecord-backed repair card carrying
  repair text, route, source artifact, verification commands, and the
  current evidence strength, static evidence boundary, and authority boundary.
  It is the same repair vocabulary used by PR comment projection.
- `packets[].llm_guidance.copyable_packet` - optional GapRecord-backed
  pasteable repair packet for coding agents. It carries `task`, `context`,
  `repair`, `verification`, `receipt`, `stop_conditions`, `do_not_do`,
  `authority_boundary`, static evidence boundary context, and a Markdown
  sibling with the same sections. It is additive and derives only from the
  selected `GapRecord`; it does not rerun analysis, edit source, generate
  tests, call providers, change gate authority, or infer repairability from
  prose.
- `packets[].current_grip` — one of the `SeamGripClass` strings the
  packet is emitted for (`weakly_gripped`, `ungripped`,
  `reachable_unrevealed`, the four `*_unknown` classes, or
  `opaque`).
- `packets[].headline_eligible` — boolean. `true` for the
  headline-eligible classes, `false` for `opaque`. Lets agents
  prioritize without re-deriving the headline mapping.
- `packets[].recommended_test` — suggested test placement. `name` is a
  deterministic snake-case test name derived from the seam owner and
  kind. `file` uses the nearest strong related test when present,
  falls back to the highest-confidence related test, and otherwise
  infers a conventional `tests/*_tests.rs` path from the production
  seam file. `reason` explains that choice.
- `packets[].nearest_strong_test_to_imitate` — first ranked related
  test with `oracle_strength: "strong"`, or `null` when no strong
  related test is visible. This is an imitation target, not a
  requirement to clone that test.
- `packets[].evidence` — per-stage `StageState` strings.
- `packets[].observed_values` — literal scalars seen in owner-call
  arguments across related tests.
- `packets[].missing_discriminators` — array of `{value, reason}`
  records mirroring the analyzer's `MissingDiscriminatorFact` shape.
  For predicate-boundary seams, includes a fallback entry naming the
  equality boundary even when no analyzer hypothesis fired.
- `packets[].candidate_values` — array of `{value, reason}` records
  naming input values or trigger shapes the new test should exercise.
  It is seeded from `missing_discriminators`; if no missing
  discriminator exists, it falls back to the seam's required
  discriminator.
- `packets[].missing_oracle_shape` — guidance string keyed by
  `SeamKind` and `ExpectedSink`. Examples:
  - `predicate_boundary` → "exact returned value assertion at the
    equality boundary"
  - `error_variant` → "exact error-variant assertion (matches! /
    assert_matches!)"
  - `side_effect` → "mock expectation, event/state observer, or
    persistence assertion (...)"
- `packets[].assertion_shape` — structured assertion guidance with a
  stable `kind` (`exact_return_value`, `exact_error_variant`,
  `field_equality`, `side_effect_observer`, `match_result`, or
  `call_expectation`) plus a fill-in example. Placeholders are
  intentional; ripr does not invent expected values.
- `packets[].related_existing_tests` — capped at
  `MAX_RELATED_TESTS_PER_PACKET` (currently 8). Carries test name,
  file, line, oracle kind, oracle strength, and a short
  `evidence_summary` describing the oracle shape (e.g., "exact value
  assertion", "is_err / broad-error assertion").
- `packets[].patterns_to_imitate` — ranked related tests with strong
  or medium oracle strength. Each entry carries the same test identity
  and oracle/relation fields as `nearest_strong_test_to_imitate`, plus
  a reason.
- `packets[].patterns_to_avoid` — advisory patterns that would keep
  the packet low-discriminator, such as copying broad/smoke-only
  related tests or adding another test with only already-observed
  values. Each entry has `{pattern, reason}`.
- `packets[].suggested_assertions` — best-effort assertion templates
  the agent fills in. Placeholders are intentional; ripr never invents
  expected values. Example for predicate boundary:
  `"assert_eq!(discounted_total(/* discount_threshold (equality boundary) */), /* expected */)"`.
- `packets[].confidence` — `high`, `medium`, `low`, or `unknown`
  confidence in the packet recommendation. It is derived from related
  test ranking and visible missing-discriminator evidence.
- `packets[].evidence_record` - additive Lane 1 evidence spine for the selected
  seam, using the same schema version `0.1` documented under repo exposure.
  Agent packets keep their existing top-level work-order fields for
  compatibility, while this nested record gives downstream consumers the shared
  identity, evidence path, recommendation, actionability, calibration
  placeholder, and static limitations without reassembling them.
- `packets[].runtime_confirmation` — boilerplate string reminding the
  agent that `ripr` is preflight static evidence and runtime
  mutation confirmation (e.g., `cargo-mutants`) is a separate
  calibration step.
- `packets[].static_evidence_boundary` - typed non-claim boundary for coding
  agents and editor surfaces. Consumers should copy this field rather than
  infer runtime, coverage, correctness, gate, or merge claims from prose.

The packet is the agent's work order: it names the seam, the missing
discriminator, the oracle shape, and an assertion template — but never
generates the test itself. Composition with a coding agent closes the
loop.

## Agent Working-Set Brief

`ripr agent brief --json` is an agent-active routing surface governed by
[RIPR-SPEC-0010](specs/RIPR-SPEC-0010-agent-working-set-brief.md). It emits a
small working-set summary that selects the top seams relevant to the files,
lines, diff, base ref, or explicit seam ID an agent is touching.

Command forms:

```bash
ripr agent brief --root . --diff change.diff --json
ripr agent brief --root . --base main --json
ripr agent brief --root . --files src/pricing.rs --json
ripr agent brief --root . --seam-id f3c9e4d21a0b7c88 --json
```

The JSON shape uses schema `0.1`:

```json
{
  "schema_version": "0.1",
  "tool": "ripr",
  "scope": "working_set",
  "root": ".",
  "mode": "draft",
  "config": {
    "state": "loaded",
    "path": "ripr.toml",
    "fingerprint": "fnv1a64:4c94a2f6cfaa5c21"
  },
  "working_set": {
    "source": "diff",
    "files": ["src/pricing.rs"],
    "changed_lines": [
      {
        "file": "src/pricing.rs",
        "line": 88
      }
    ],
    "base": "main",
    "diff": "change.diff",
    "seam_id": null
  },
  "limits": {
    "requested": 3,
    "returned": 1,
    "default": 3,
    "hard_cap": 10
  },
  "top_seams": [
    {
      "seam_id": "f3c9e4d21a0b7c88",
      "owner": "src/pricing.rs::discounted_total",
      "seam_kind": "predicate_boundary",
      "file": "src/pricing.rs",
      "line": 88,
      "expression": "amount >= discount_threshold",
      "grip_class": "weakly_gripped",
      "severity": "warning",
      "headline_eligible": true,
      "why_now": {
        "reason": "changed_line_intersects_seam",
        "confidence": "high",
        "evidence": "changed line 88 intersects the seam origin"
      },
      "evidence": {
        "reach": "yes",
        "activate": "yes",
        "propagate": "yes",
        "observe": "yes",
        "discriminate": "weak"
      },
      "recommended_test": {
        "name": "discounted_total_boundary_discriminator",
        "file": "tests/pricing.rs",
        "reason": "place the new targeted test next to the nearest strong related test"
      },
      "nearest_strong_test_to_imitate": {
        "name": "below_threshold_has_no_discount",
        "file": "tests/pricing.rs",
        "line": 12,
        "oracle_kind": "exact_value",
        "oracle_strength": "strong",
        "relation_reason": "direct_owner_call",
        "relation_confidence": "high"
      },
      "candidate_values": [
        {
          "value": "discount_threshold (equality boundary)",
          "reason": "observed values do not include the equality-boundary case"
        }
      ],
      "missing_discriminators": [
        {
          "value": "discount_threshold (equality boundary)",
          "reason": "observed values do not include the equality-boundary case"
        }
      ],
      "assertion_shape": {
        "kind": "exact_return_value",
        "example": "assert_eq!(discounted_total(/* discount_threshold (equality boundary) */), /* expected */)"
      },
      "packet_ref": {
        "format": "agent-seam-packets-json",
        "seam_id": "f3c9e4d21a0b7c88"
      },
      "verification": {
        "before_snapshot_command": "ripr check --root . --mode draft --format repo-exposure-json > target/ripr/workflow/before.repo-exposure.json",
        "after_snapshot_command": "ripr check --root . --mode draft --format repo-exposure-json > target/ripr/workflow/after.repo-exposure.json",
        "verify_command": "ripr agent verify --root . --before target/ripr/workflow/before.repo-exposure.json --after target/ripr/workflow/after.repo-exposure.json --json",
        "suggested_test_command": "cargo test discounted_total_boundary_discriminator"
      }
    }
  ],
  "next": {
    "inspect_packet": "ripr check --root . --mode draft --format agent-seam-packets-json > target/ripr/workflow/agent-seam-packets.json",
    "verify_after_edit": "ripr agent verify --root . --before target/ripr/workflow/before.repo-exposure.json --after target/ripr/workflow/after.repo-exposure.json --json"
  },
  "warnings": []
}
```

Field contract:

- `scope` — always `"working_set"`.
- `working_set.source` — `"diff"`, `"base"`, `"files"`, or `"seam_id"`.
- `limits.default` — always `3`.
- `limits.hard_cap` — always `10`.
- `top_seams[]` — ranked seam summaries, intentionally smaller than full agent
  seam packets.
- `top_seams[].why_now.reason` — one of
  `changed_line_intersects_seam`, `changed_owner_function`,
  `changed_test_for_related_seam`, `changed_assertion_near_related_test`,
  `same_file_seam`, `explicit_seam_id`, or `repo_actionable_fallback`.
- `top_seams[].packet_ref` — pointer to the full agent seam packet.
- `top_seams[].verification` — before/after static evidence commands and an
  optional focused test command.

Static examples use abbreviated JSON fragments to show routing behavior.

Diff-scoped touched seam:

```json
{
  "working_set": {
    "source": "diff",
    "files": ["src/pricing.rs"],
    "changed_lines": [{ "file": "src/pricing.rs", "line": 88 }],
    "diff": "change.diff"
  },
  "limits": { "requested": 3, "returned": 1, "default": 3, "hard_cap": 10 },
  "top_seams": [
    {
      "seam_id": "f3c9e4d21a0b7c88",
      "file": "src/pricing.rs",
      "line": 88,
      "grip_class": "weakly_gripped",
      "why_now": {
        "reason": "changed_line_intersects_seam",
        "confidence": "high"
      },
      "missing_discriminators": [
        { "value": "discount_threshold (equality boundary)" }
      ],
      "packet_ref": {
        "format": "agent-seam-packets-json",
        "seam_id": "f3c9e4d21a0b7c88"
      },
      "verification": {
        "after_snapshot_command": "ripr check --root . --mode draft --format repo-exposure-json > target/ripr/workflow/after.repo-exposure.json"
      }
    }
  ],
  "warnings": []
}
```

File-scoped capped brief:

```json
{
  "working_set": {
    "source": "files",
    "files": ["src/pricing.rs"],
    "changed_lines": []
  },
  "limits": { "requested": 3, "returned": 3, "default": 3, "hard_cap": 10 },
  "top_seams": [
    {
      "seam_id": "f3c9e4d21a0b7c88",
      "why_now": { "reason": "same_file_seam", "confidence": "medium" }
    },
    {
      "seam_id": "a4c733e1d9ef0220",
      "why_now": { "reason": "same_file_seam", "confidence": "medium" }
    },
    {
      "seam_id": "c2f1b5d0a8ee9b41",
      "why_now": { "reason": "same_file_seam", "confidence": "medium" }
    }
  ],
  "warnings": ["7 additional visible seams were omitted by the brief cap"]
}
```

Seam-ID lookup:

```json
{
  "working_set": {
    "source": "seam_id",
    "files": ["src/pricing.rs"],
    "seam_id": "f3c9e4d21a0b7c88"
  },
  "limits": { "requested": 1, "returned": 1, "default": 3, "hard_cap": 10 },
  "top_seams": [
    {
      "seam_id": "f3c9e4d21a0b7c88",
      "why_now": { "reason": "explicit_seam_id", "confidence": "high" },
      "packet_ref": {
        "format": "agent-seam-packets-json",
        "seam_id": "f3c9e4d21a0b7c88"
      }
    }
  ],
  "warnings": []
}
```

Configured-off or suppressed seams:

```json
{
  "working_set": {
    "source": "files",
    "files": ["src/pricing.rs"],
    "changed_lines": []
  },
  "limits": { "requested": 3, "returned": 0, "default": 3, "hard_cap": 10 },
  "top_seams": [],
  "warnings": [
    "1 matching seam was hidden because configured severity is off",
    "1 matching seam was hidden by a reasoned suppression"
  ]
}
```

The working-set brief must not write files, generate tests, change cache or LSP
refresh behavior, or emit runtime mutation claims.

## Pilot Summary

`ripr pilot` writes a first-run operator packet under `target/ripr/pilot/` by
default. It reuses the repo-exposure and agent seam packet renderers, then adds
a small summary that ranks the next actionable seams.

Pilot files:

```text
target/ripr/pilot/repo-exposure.json
target/ripr/pilot/repo-exposure.md
target/ripr/pilot/agent-seam-packets.json
target/ripr/pilot/pilot-summary.json
target/ripr/pilot/pilot-summary.md
```

`pilot-summary.json` uses schema `0.2`:

```json
{
  "schema_version": "0.2",
  "tool": "ripr",
  "scope": "repo",
  "status": "complete",
  "root": ".",
  "mode": "draft",
  "config": {
    "state": "missing",
    "path": null
  },
  "outputs": {
    "repo_exposure_json": "target/ripr/pilot/repo-exposure.json",
    "repo_exposure_md": "target/ripr/pilot/repo-exposure.md",
    "agent_seam_packets_json": "target/ripr/pilot/agent-seam-packets.json",
    "pilot_summary_json": "target/ripr/pilot/pilot-summary.json",
    "pilot_summary_md": "target/ripr/pilot/pilot-summary.md"
  },
  "max_seams": 5,
  "timeout_ms": 30000,
  "outputs_written": [
    "repo_exposure_json",
    "repo_exposure_md",
    "agent_seam_packets_json",
    "pilot_summary_json",
    "pilot_summary_md"
  ],
  "actionable_seams_total": 1,
  "top_actionable_seams": [
    {
      "seam_id": "67fc764ba37d77bd",
      "file": "src/lib.rs",
      "line": 2,
      "kind": "predicate_boundary",
      "owner": "src/lib.rs::discounted_total",
      "grip_class": "weakly_gripped",
      "why": "missing discriminator: discount_threshold (equality boundary)",
      "missing_discriminator": {
        "value": "discount_threshold (equality boundary)",
        "reason": "observed values do not include the equality-boundary case"
      },
      "related_test_present": true,
      "suggested_assertion_present": true,
      "targeted_test_brief": "Target seam:\n- src/lib.rs:2\n..."
    }
  ],
  "next": {
    "inspect_packet": "target/ripr/pilot/agent-seam-packets.json",
    "after_snapshot_command": "ripr check --root . --mode draft --format repo-exposure-json > target/ripr/pilot/after.repo-exposure.json",
    "outcome_command": "ripr outcome --before target/ripr/pilot/repo-exposure.json --after target/ripr/pilot/after.repo-exposure.json"
  }
}
```

If analysis exceeds the pilot budget, `pilot-summary.json` is still written with
`status: "partial"` and no ranked seams:

```json
{
  "schema_version": "0.2",
  "tool": "ripr",
  "scope": "repo",
  "status": "partial",
  "reason": "timeout",
  "timeout_ms": 30000,
  "completed_phases": [],
  "root": ".",
  "mode": "draft",
  "config": {
    "state": "missing",
    "path": null
  },
  "outputs": {
    "repo_exposure_json": "target/ripr/pilot/repo-exposure.json",
    "repo_exposure_md": "target/ripr/pilot/repo-exposure.md",
    "agent_seam_packets_json": "target/ripr/pilot/agent-seam-packets.json",
    "pilot_summary_json": "target/ripr/pilot/pilot-summary.json",
    "pilot_summary_md": "target/ripr/pilot/pilot-summary.md"
  },
  "outputs_written": [
    "pilot_summary_json",
    "pilot_summary_md"
  ],
  "max_seams": 5,
  "actionable_seams_total": null,
  "top_actionable_seams": [],
  "next": {
    "retry_command": "ripr pilot --root . --out target/ripr/pilot --mode draft --max-seams 5 --timeout-ms 120000"
  }
}
```

Field contract:

- `schema_version` — currently `"0.2"`.
- `scope` — always `"repo"`.
- `status` — `"complete"` when repo exposure and agent seam packet artifacts
  were written, or `"partial"` when the command stopped at a diagnostic summary.
- `reason` — present for partial summaries; currently `"timeout"`.
- `timeout_ms` — explicit pilot analysis budget. The default is `30000`.
- `completed_phases` — present for partial summaries. It is empty until pilot
  owns more detailed phase instrumentation.
- `root` — analyzed workspace root as supplied to `ripr pilot`.
- `mode` — effective analysis mode after explicit CLI flags and repo config are
  applied.
- `config.state` — `"loaded"` when `ripr.toml` was loaded, otherwise
  `"missing"`. Missing config is healthy and means built-in conservative
  defaults were used.
- `outputs` — paths to the generated pilot packet files.
- `outputs_written` — names of output files actually written. Partial timeout
  summaries write only `pilot_summary_json` and `pilot_summary_md`.
- `max_seams` — cap requested by `--max-seams`.
- `actionable_seams_total` — number of seams considered actionable by the pilot
  ranking policy, or `null` for partial summaries where analysis did not finish.
- `top_actionable_seams[]` — ranked seams using class order
  `weakly_gripped`, `ungripped`, `reachable_unrevealed`, unknown-stage classes,
  then `opaque`, with evidence tie-breakers for missing discriminator, related
  test, suggested assertion, and stable location.
- `top_actionable_seams[].targeted_test_brief` — human-readable work order
  derived from the same fields as the agent seam packet. Placeholders are
  intentional; RIPR does not invent expected values.
- `next` — advisory follow-up commands. Complete summaries include the public
  `ripr outcome` before/after receipt command. Partial summaries include a
  retry command with a larger explicit timeout.

The Markdown sibling prints the same summary, puts the top recommendation first,
and includes the inspected seam, why it matters, the focused test to write, the
top seam's targeted test brief, and the before/after commands for complete
runs. It remains advisory. On timeout, the Markdown sibling records the partial
state and the retry command instead of pretending the packet is complete.

## LSP Seam Diagnostics

The LSP server publishes a `Diagnostic` for every actionable
`ClassifiedSeam` alongside the existing diff-scoped `Finding` diagnostics
under the built-in saved-workspace default. Clients or repo policy can pass
`seamDiagnostics: false` to disable seam diagnostics for a session.

Diagnostic shape:

```jsonc
{
  "range": { "start": { "line": 87, "character": 0 }, "end": { "line": 87, "character": 28 } },
  "severity": 2, // 1=Error, 2=Warning, 3=Information, 4=Hint
  "code": "ripr-seam-weakly-gripped",
  "source": "ripr",
  "message": "Weakly gripped behavioral seam (predicate_boundary): amount >= discount_threshold",
  "data": {
    "schema_version": "0.1",
    "seam_id": "f3c9e4d21a0b7c88",
    "seam_kind": "predicate_boundary",
    "grip_class": "weakly_gripped",
    "headline_eligible": true,
    "owner": "src/pricing.rs::discounted_total",
    "expected_sink": "return_value",
    "evidence": {
      "reach": "yes",
      "activate": "yes",
      "propagate": "yes",
      "observe": "yes",
      "discriminate": "weak"
    }
  }
}
```

When `target/ripr/reports/gap-decision-ledger.json` exists, the LSP server also
publishes advisory diagnostics for records whose
`projection_eligibility.lsp_diagnostic.eligible` flag is true and whose anchor
contains a local file and line. These diagnostics are sourced from the explicit
GapRecord; they do not infer projectability from raw findings and do not make
gate or badge decisions.

GapRecord diagnostic shape:

```jsonc
{
  "range": { "start": { "line": 41, "character": 0 }, "end": { "line": 41, "character": 120 } },
  "severity": 2,
  "code": "ripr-gap-MissingBoundaryAssertion",
  "source": "ripr",
  "message": "ripr gap: MissingBoundaryAssertion; repair route: AddBoundaryAssertion; changed behavior: amount >= threshold; suggested check: assert_eq!(...)",
  "data": {
    "schema_version": "0.1",
    "source": "gap_decision_ledger",
    "gap_ledger": "target/ripr/reports/gap-decision-ledger.json",
    "gap_id": "gap:pr:pricing:threshold-boundary",
    "canonical_gap_id": "gap:rust:pricing:threshold-boundary",
    "gap_kind": "MissingBoundaryAssertion",
    "language": "rust",
    "language_status": "stable",
    "scope": "pr_local",
    "evidence_class": "boundary_assertion",
    "gap_state": "actionable",
    "policy_state": "new",
    "repairability": "repairable",
    "repair_route": { "route_kind": "AddBoundaryAssertion" },
    "anchor": {
      "file": "src/pricing.rs",
      "line": 42,
      "owner": "pricing::discounted_total",
      "dedupe_fingerprint": "gap:rust:pricing:threshold-boundary"
    },
    "evidence_ids": ["evidence:pricing:threshold-boundary"],
    "verification_commands": ["cargo xtask fixtures boundary_gap"]
  }
}
```

Validated gap diagnostics can drive bounded repair actions such as `Inspect
gap: copy repair packet`, `Verify after test: copy verify command`, and `Review
result: copy receipt command` when the current artifact supplies safe payloads.
The repair-packet action calls `ripr.collectContext` with `gap_id` and
`gap_ledger`. The command returns the same GapRecord-backed agent packet
produced by `ripr agent packet --gap-ledger ... --gap-id ...`; it does not
rerun analysis, edit source, generate tests, call a provider, run mutation
testing, or parse the diagnostic message. Stale, missing, disabled, or
unvalidated gap artifacts fail closed to refresh-only actions.

Per-class severity:

| `SeamGripClass`            | Severity      | Diagnostic? |
|----------------------------|---------------|-------------|
| `weakly_gripped`           | `Warning`     | yes         |
| `ungripped`                | `Warning`     | yes         |
| `reachable_unrevealed`     | `Warning`     | yes         |
| `activation_unknown`       | `Information` | yes         |
| `propagation_unknown`      | `Information` | yes         |
| `observation_unknown`      | `Information` | yes         |
| `discrimination_unknown`   | `Information` | yes         |
| `opaque`                   | `Information` | yes         |
| `strongly_gripped`         | —             | **no**      |
| `intentional`              | —             | **no**      |
| `suppressed`               | —             | **no**      |

Diagnostic codes are stable: `ripr-seam-{class}` with `_` replaced by
`-` in the class name. Editors can filter or theme by code without
parsing severity. The `data` field carries `seam_id` so seam-evidence
hover (`lsp/seam-evidence-hover-v1`) can look up the same record from
`inventory_classified_seams_at`.

Seam evidence hover renders from that same `ClassifiedSeam` record, not from
diagnostic message text. For actionable seams it includes the grip class,
RIPR evidence path, observed values, missing discriminator, related test
location, suggested test shape, packet and brief handoff command strings,
verify and receipt command strings, static evidence limits, and per-kind next
step when those fields are available. The hover does not run mutation testing,
edit source, generate tests, or claim runtime adequacy.

The diagnostic range is currently a **full-line placeholder**: seams
do not yet carry a column, so the range spans `(line, 0)` →
`(line, MAX_DIAGNOSTIC_RANGE_WIDTH)`. Editors render this as a
single-line squiggle that always covers the seam regardless of
indentation. A future PR can derive the real column from the source
file via the (now reserved) `_root` parameter on
`diagnostic_for_classified_seam`.

Seam diagnostics also drive editor code actions:

- `Inspect Test Gap - Copy Context` calls `ripr.collectContext` with `seam_id` and
  copies the selected agent seam packet JSON.
- `Write targeted test: copy brief` copies a plain-language work order derived
  from the same seam packet guidance.
- `Agent handoff: copy packet command` and `Agent handoff: copy brief command`
  copy the selected seam's agent packet and brief commands.
- `Verify after test: copy after-snapshot command` and
  `Verify after test: copy verify command` copy the static after-snapshot and
  verify commands.
- `Review result: copy receipt command` copies the selected seam's receipt
  command.
- `Write targeted test: copy suggested assertion` appears only when the agent
  seam packet assertion shape contains a concrete assertion example.
- `Write targeted test: open best related test` appears only when ranked
  related-test evidence has a visible file/line.
- `Refresh Analysis - Saved Workspace Check` remains available for every
  request.

These actions do not edit files, generate tests, or add CodeLens
surface. The pre-4B `Finding`/`AnalysisSnapshot` hover and context
actions continue to work for diff-scoped diagnostics; seam diagnostics
live alongside them.

Seam diagnostics can also drive `executeCommand` `ripr.collectEvidenceContext`.
The command accepts a `seam_id` argument and returns one bounded editor handoff
packet derived from the latest saved-workspace `ClassifiedSeam` plus the shared
agent-loop command templates. It does not run analysis, edit source, generate
tests, call an LLM provider, or run mutation testing. Unknown seams return no
packet.

JSON shape:

```json
{
  "schema_version": "0.1",
  "tool": "ripr",
  "root": ".",
  "mode": "draft",
  "seam_id": "67fc764ba37d77bd",
  "file": "src/pricing.rs",
  "range": {
    "start": 88,
    "end": 88
  },
  "class": "weakly_gripped",
  "seam_kind": "predicate_boundary",
  "owner": "pricing::discounted_total",
  "expression": "amount >= discount_threshold",
  "required_discriminator": "boundary_value",
  "expected_sink": "return_value",
  "evidence_path": {
    "reach": "present",
    "activate": "present",
    "propagate": "present",
    "observe": "present",
    "discriminate": "weak"
  },
  "evidence_summaries": {
    "reach": "related test calls owner",
    "activate": "test reaches branch",
    "propagate": "return value sink",
    "observe": "exact assertion",
    "discriminate": "boundary value missing"
  },
  "missing_discriminator": "discount_threshold (equality boundary)",
  "missing_discriminators": [
    {
      "value": "discount_threshold (equality boundary)",
      "reason": "observed values skip equality boundary"
    }
  ],
  "related_test": "tests/pricing.rs::below_threshold_has_no_discount",
  "related_test_location": {
    "file": "tests/pricing.rs",
    "line": 12,
    "test_name": "below_threshold_has_no_discount",
    "oracle_kind": "exact_value",
    "oracle_strength": "strong"
  },
  "suggested_assertion": "assert_eq!(...)",
  "suggested_test": {
    "file": "tests/pricing.rs",
    "name": "discounted_total_boundary_value",
    "candidate_value": "discount_threshold (equality boundary)",
    "assertion_shape": "Assert the exact returned value."
  },
  "agent_packet_command": "ripr agent packet --root . --seam-id 67fc764ba37d77bd --json > target/ripr/agent/agent-packet.json",
  "agent_brief_command": "ripr agent brief --root . --seam-id 67fc764ba37d77bd --json > target/ripr/agent/agent-brief.json",
  "after_snapshot_command": "ripr check --root . --mode draft --format repo-exposure-json > target/ripr/pilot/after.repo-exposure.json",
  "verify_command": "ripr agent verify --root . --before target/ripr/pilot/repo-exposure.json --after target/ripr/pilot/after.repo-exposure.json --json > target/ripr/agent/agent-verify.json",
  "receipt_command": "ripr agent receipt --root . --verify-json target/ripr/agent/agent-verify.json --seam-id 67fc764ba37d77bd --json --out target/ripr/agent/agent-receipt.json",
  "limits_note": "Static evidence only; no runtime mutation execution."
}
```

## Dogfood Report

`cargo xtask dogfood` writes:

```text
target/ripr/reports/dogfood.json
target/ripr/reports/dogfood.md
target/ripr/dogfood/<fixture>/check.json
target/ripr/dogfood/<fixture>/human.txt
target/ripr/dogfood/gate-adoption/<case>/gate-decision.json
target/ripr/dogfood/gate-adoption/<case>/gate-decision.md
fixtures/boundary_gap/expected/gate-adoption/<case>/gate-decision.json
fixtures/boundary_gap/expected/gate-adoption/<case>/gate-decision.md
fixtures/boundary_gap/expected/first-useful-action/<case>/first-useful-action.json
fixtures/boundary_gap/expected/first-useful-action/<case>/first-useful-action.md
fixtures/first_successful_pr/<case>/expected/start-here.json
fixtures/first_successful_pr/<case>/expected/start-here.md
fixtures/boundary_gap/expected/pr-review-front-panel/<case>/pr-review-front-panel.json
fixtures/boundary_gap/expected/pr-review-front-panel/<case>/pr-review-front-panel.md
fixtures/boundary_gap/expected/report-packet-index/<case>/index.json
fixtures/boundary_gap/expected/report-packet-index/<case>/index.md
fixtures/finding-alignment-dogfood/corpus.json
```

The report is advisory. It runs `ripr check --mode fast` against stable fixture
diffs and runs `ripr gate evaluate` against checked boundary-gap PR guidance
and calibration evidence for the explicit gate adoption modes. It also checks
repo-local first useful action receipts for the documented first-action routes.
It records first successful PR receipts and first-run adoption counters for the
checked `start-here.{json,md}` corpus.
It checks repo-local PR review front-panel receipts for the documented Campaign
24 reviewer routes. It checks repo-local report-packet index receipts for the
documented Campaign 25 packet-index routes. It records
`default_ci_blocking: false`; generated CI still leaves `RIPR_GATE_MODE` unset
unless the repository configures it. It also validates the generated GitHub
workflow cockpit receipt for `Start here` guidance, known regeneration
commands, artifact upload, advisory default posture, and gate-authority
boundaries. The generated adoption receipts are compared with
`fixtures/boundary_gap/expected/gate-adoption/`. The checked first-action
receipts are read from `fixtures/boundary_gap/expected/first-useful-action/`.
The checked first successful PR receipts are read from
`fixtures/first_successful_pr/`.
The checked front-panel receipts are read from
`fixtures/boundary_gap/expected/pr-review-front-panel/`. The checked
report-packet index receipts are read from
`fixtures/boundary_gap/expected/report-packet-index/`. The checked finding
alignment receipts are read from `fixtures/finding-alignment-dogfood/` and
record real RIPR PR examples where raw findings remain supporting evidence,
canonical items are the countable unit, canonical gap identities and raw
finding summaries are explicit, actionable items have repair and verification
routes, static limitations name analyzer repair routes, and runtime-confidence
static-only class examples stay calibration work rather than user test debt.
The calibrated-gate dogfood case expects a non-zero evaluator exit only for the
explicit blocking mode and treats that as healthy when the written decision
report has the expected `blocked` status and count.

JSON shape:

```json
{
  "schema_version": "0.1",
  "status": "pass",
  "advisory": true,
  "runs": [
    {
      "name": "boundary_gap",
      "root": "fixtures/boundary_gap/input",
      "diff": "fixtures/boundary_gap/diff.patch",
      "actual_dir": "target/ripr/dogfood/boundary_gap",
      "duration_ms": 123,
      "findings": 1,
      "stop_reason_mentions": 1,
      "class_counts": {
        "exposed": 0,
        "weakly_exposed": 1,
        "reachable_unrevealed": 0,
        "no_static_path": 0,
        "infection_unknown": 0,
        "propagation_unknown": 0,
        "static_unknown": 0
      },
      "errors": []
    }
  ],
  "first_useful_action": {
    "default_ci_blocking": false,
    "receipt_dir": "fixtures/boundary_gap/expected/first-useful-action",
    "cases": [
      {
        "name": "actionable",
        "expected_dir": "fixtures/boundary_gap/expected/first-useful-action/actionable",
        "json_path": "fixtures/boundary_gap/expected/first-useful-action/actionable/first-useful-action.json",
        "markdown_path": "fixtures/boundary_gap/expected/first-useful-action/actionable/first-useful-action.md",
        "status": "actionable",
        "action_kind": "write_focused_test",
        "audience": "developer",
        "selected": true,
        "static_movement": "unknown",
        "expected_status": "actionable",
        "expected_action_kind": "write_focused_test",
        "expected_audience": "developer",
        "expected_selected": true,
        "expected_static_movement": "unknown",
        "errors": []
      }
    ]
  },
  "first_successful_pr": {
    "default_ci_blocking": false,
    "receipt_dir": "fixtures/first_successful_pr",
    "metrics": {
      "first_run_packets_total": 4,
      "first_run_top_gap_selected_total": 2,
      "first_run_no_action_total": 1,
      "first_run_blocked_total": 1,
      "first_run_missing_artifact_total": 0,
      "first_run_stale_artifact_total": 0,
      "first_run_wrong_root_total": 0,
      "first_run_malformed_artifact_total": 0,
      "first_run_timeout_total": 0
    },
    "cases": [
      {
        "name": "boundary-gap",
        "expected_dir": "fixtures/first_successful_pr/boundary-gap/expected",
        "json_path": "fixtures/first_successful_pr/boundary-gap/expected/start-here.json",
        "markdown_path": "fixtures/first_successful_pr/boundary-gap/expected/start-here.md",
        "status": "actionable",
        "state": "top_gap",
        "top_gap_kind": "MissingBoundaryAssertion",
        "verify_command": "cargo xtask fixtures boundary_gap",
        "next_command": null,
        "expected_status": "actionable",
        "expected_state": "top_gap",
        "description": "A repairable Rust boundary gap becomes the top first-run repair.",
        "errors": []
      }
    ]
  },
  "pr_review_front_panel": {
    "default_ci_blocking": false,
    "receipt_dir": "fixtures/boundary_gap/expected/pr-review-front-panel",
    "cases": [
      {
        "name": "actionable",
        "json_path": "fixtures/boundary_gap/expected/pr-review-front-panel/actionable/pr-review-front-panel.json",
        "markdown_path": "fixtures/boundary_gap/expected/pr-review-front-panel/actionable/pr-review-front-panel.md",
        "status": "advisory",
        "top_issue_state": "actionable",
        "policy_state": "new_policy_eligible",
        "placement": "changed_line",
        "movement_state": "unknown",
        "coverage_grip_state": "not_available",
        "new_policy_eligible": 1,
        "baseline_resolved": 0,
        "blocking_candidates": 0,
        "warnings": 0,
        "expected_status": "advisory",
        "expected_top_issue_state": "actionable",
        "expected_policy_state": "new_policy_eligible",
        "expected_placement": "changed_line",
        "expected_movement_state": "unknown",
        "expected_coverage_grip_state": "not_available",
        "expected_new_policy_eligible": 1,
        "expected_baseline_resolved": 0,
        "expected_blocking_candidates": 0,
        "expected_warnings": 0,
        "reason": "The front panel should show the top focused-test action first when a PR-local seam is line-placeable.",
        "errors": []
      }
    ]
  },
  "report_packet_index": {
    "default_ci_blocking": false,
    "receipt_dir": "fixtures/boundary_gap/expected/report-packet-index",
    "cases": [
      {
        "name": "complete_packet",
        "actual_dir": "fixtures/boundary_gap/expected/report-packet-index/complete-packet",
        "json_path": "fixtures/boundary_gap/expected/report-packet-index/complete-packet/index.json",
        "markdown_path": "fixtures/boundary_gap/expected/report-packet-index/complete-packet/index.md",
        "expected_report": "fixtures/boundary_gap/expected/report-packet-index/complete-packet/index.json",
        "expected_markdown": "fixtures/boundary_gap/expected/report-packet-index/complete-packet/index.md",
        "status": "pass",
        "missing_expected": 0,
        "warnings": 0,
        "failures": 0,
        "start_here_available": true,
        "gate_authority_present": true,
        "groups": [
          "start_here",
          "pr_review_story",
          "repair_agent_handoff",
          "evidence_movement",
          "policy_gates",
          "calibration",
          "validation_receipts",
          "sarif_badges"
        ],
        "expected_status": "pass",
        "expected_missing_expected": 0,
        "expected_warnings": 0,
        "expected_failures": 0,
        "expected_start_here_available": true,
        "expected_gate_authority_present": true,
        "expected_required_groups": [
          "start_here",
          "pr_review_story",
          "repair_agent_handoff",
          "evidence_movement",
          "policy_gates",
          "calibration",
          "validation_receipts",
          "sarif_badges"
        ],
        "reason": "A complete packet should tell reviewers to start at the PR front panel while preserving gate-decision authority.",
        "errors": []
      }
    ]
  },
  "generated_ci_cockpit": {
    "default_ci_blocking": false,
    "default_inline_comments": "off",
    "language_grouping": "deferred",
    "cases": [
      {
        "name": "generated-pr-ci-review-workflow",
        "command": "cargo run --quiet -p ripr -- init --ci github --dry-run",
        "duration_ms": 123,
        "start_here": true,
        "repair_commands": 4,
        "expected_repair_commands": 4,
        "gate_authority_boundary": true,
        "default_advisory": true,
        "artifact_upload": true,
        "language_grouping_status": "deferred",
        "errors": []
      }
    ]
  },
  "finding_alignment": {
    "default_ci_blocking": false,
    "receipt_dir": "fixtures/finding-alignment-dogfood",
    "cases": [
      {
        "name": "config_policy_rendered_label_unobserved",
        "source_pr": "EffortlessMetrics/ripr#1016",
        "canonical_gap_id": "gap:config_policy_rendered_label_unobserved",
        "evidence_class": "config_or_policy_constant",
        "raw_findings_total": 2,
        "canonical_items_total": 1,
        "raw_finding_summary": "A rendered policy-label declaration and literal align into one canonical output-observer item.",
        "gap_state": "actionable",
        "actionability": "add_output_observer",
        "user_outcome": "actionable_gap",
        "repair_kind": "output_observer",
        "target_test_type": "report_render_or_golden",
        "verify_command": "cargo xtask evidence-quality-scorecard",
        "static_limitation_category": null,
        "static_limitation_repair_route": null,
        "raw_findings_supporting_only": true,
        "recommended_repair": "Add or update a report-render, config-output, snapshot, or golden observer for the rendered policy label.",
        "before_after_context": "Before structured repair-route checks, prose or class-local metadata could look sufficient; after the burn-down this actionable item requires a concrete repair and verify command.",
        "must_not_claim": [
          "Do not count declaration and literal findings as separate user actions.",
          "Do not infer actionability from raw static class.",
          "Do not recommend mutation testing before output-observer work."
        ],
        "reason": "A rendered config or policy label with no supported observer should become one actionable output-observer item.",
        "errors": []
      }
    ]
  },
  "gate_adoption": {
    "default_ci_blocking": false,
    "receipt_dir": "target/ripr/dogfood/gate-adoption",
    "cases": [
      {
        "name": "visible-only-advisory",
        "mode": "visible-only",
        "actual_dir": "target/ripr/dogfood/gate-adoption/visible-only-advisory",
        "json_path": "target/ripr/dogfood/gate-adoption/visible-only-advisory/gate-decision.json",
        "markdown_path": "target/ripr/dogfood/gate-adoption/visible-only-advisory/gate-decision.md",
        "duration_ms": 123,
        "status": "advisory",
        "blocking": 0,
        "acknowledged": 0,
        "advisory": 1,
        "expected_status": "advisory",
        "expected_blocking": 0,
        "expected_acknowledged": 0,
        "expected_advisory": 1,
        "exit_success": true,
        "expected_exit_success": true,
        "errors": []
      }
    ]
  }
}
```

The checked first-action receipt cases are:

| Case | Expected status | Purpose |
| --- | --- | --- |
| `actionable` | `actionable` | Records the focused-test action for a PR-local weak seam. |
| `baseline-only` | `baseline_only` | Keeps historical baseline debt visible without turning it into PR-local test work. |
| `stale` | `stale` | Requires evidence refresh before selecting a focused action. |
| `missing-required-artifact` | `missing_required_artifact` | Routes agents to generate the missing assistant proof input. |
| `unchanged-after-attempt` | `unchanged_after_attempt` | Routes back to revising the focused test when static movement is unchanged. |
| `no-actionable-seam` | `no_actionable_seam` | Records the clean advisory state instead of silence. |

The checked PR review front-panel receipt cases are:

| Case | Expected top issue state | Purpose |
| --- | --- | --- |
| `actionable` | `actionable` | Shows the focused-test repair route for a PR-local weak seam. |
| `acknowledged` | `actionable` | Keeps `ripr-waive` visible as acknowledgement, not hidden success. |
| `suppressed` | `baseline_only` | Keeps durable suppression policy visible in PR review. |
| `baseline_resolved` | `already_improved` | Shows reviewed baseline debt resolved by the PR. |
| `blocked` | `actionable` | Shows a configured gate block while preserving the repair route and gate authority. |
| `missing_proof` | `missing_required_input` | Routes reviewers to regenerate the missing assistant proof artifact. |
| `advisory_only` | `no_actionable_seam` | Records a no-action advisory state instead of silence. |
| `coverage_flat_grip_improved` | `already_improved` | Shows static grip improvement while coverage stays flat. |

The checked report-packet index receipt cases are:

| Case | Expected status | Purpose |
| --- | --- | --- |
| `complete_packet` | `pass` | Shows the complete reviewer-first packet, including start-here and gate-authority links. |
| `sparse_advisory` | `warn` | Keeps sparse adoption advisory while showing missing optional surfaces. |
| `missing_front_panel` | `warn` | Makes a missing first-screen front panel visible instead of forcing artifact archaeology. |
| `blocked_gate` | `fail` | Preserves a configured blocked gate state while naming gate decision as authority. |
| `missing_assistant_proof` | `warn` | Routes users to regenerate missing assistant proof instead of hiding the gap. |
| `missing_receipts` | `warn` | Shows missing validation receipts and their regeneration commands. |
| `coverage_grip_present` | `pass` | Keeps coverage/grip context findable as calibration context, not runtime confirmation. |

The checked generated-CI cockpit receipt cases are:

| Case | Expected result | Purpose |
| --- | --- | --- |
| `generated-pr-ci-review-workflow` | `pass` | Validates the generated workflow summary starts with `Start here`, includes known regeneration commands for missing cockpit surfaces, uploads report artifacts, stays advisory by default, and keeps gate-decision authority separate. |

The checked adoption cases are:

| Case | Expected status | Purpose |
| --- | --- | --- |
| `visible-only-advisory` | `advisory` | Records policy evidence without blocking. |
| `acknowledged-waiver` | `acknowledged` | Shows `ripr-waive` as a visible acknowledgement. |
| `baseline-check-existing` | `advisory` | Shows a baseline-known identity as visible and non-blocking. |
| `baseline-check-new-gap` | `blocked` | Shows a new policy-eligible identity blocking in explicit baseline-check mode. |
| `calibrated-high-confidence-new-gap` | `blocked` | Shows explicit calibrated-gate behavior for a new supported candidate. |
| `missing-baseline-config` | `config_error` | Shows missing required baseline input as a repair-oriented configuration error. |

## LSP Cockpit Report

`cargo xtask lsp-cockpit-report` writes:

```text
target/ripr/reports/lsp-cockpit.json
target/ripr/reports/lsp-cockpit.md
```

The report is an advisory dogfood artifact for the editor loop. It reads the
committed LSP fixture expectations, plus the VS Code e2e smoke test file, and
summarizes the editor surface without opening VS Code.
For seam fixtures, `status` downgrades from `pass` when the editor-agent loop
command actions for packet, brief, after-snapshot, verify, or receipt are
missing from the pinned action payloads.

JSON shape:

```json
{
  "schema_version": "0.1",
  "tool": "ripr",
  "status": "pass",
  "fixtures": [
    {
      "fixture": "boundary_gap",
      "diagnostics_path": "fixtures/boundary_gap/expected/lsp-diagnostics.json",
      "code_actions_path": "fixtures/boundary_gap/expected/lsp-code-actions.json",
      "diagnostics": {
        "total": 1,
        "seams": 1,
        "findings": 0,
        "seam_ids": ["67fc764ba37d77bd"],
        "grip_classes": ["weakly_gripped"]
      },
      "actions": {
        "titles": [
          "Inspect Test Gap - Copy Context",
          "Write targeted test: copy brief",
          "Agent handoff: copy packet command",
          "Agent handoff: copy brief command",
          "Verify after test: copy after-snapshot command",
          "Verify after test: copy verify command",
          "Review result: copy receipt command",
          "Write targeted test: copy suggested assertion",
          "Write targeted test: open best related test",
          "Refresh Analysis - Saved Workspace Check"
        ],
        "commands": [
          "ripr.copyContext",
          "ripr.copyTargetedTestBrief",
          "ripr.copyAgentPacketCommand",
          "ripr.copyAgentBriefCommand",
          "ripr.copyAfterSnapshotCommand",
          "ripr.copyAgentVerifyCommand",
          "ripr.copyAgentReceiptCommand",
          "ripr.copySuggestedAssertion",
          "ripr.openRelatedTest",
          "ripr.refresh"
        ],
        "argument_fields": [
          "after_snapshot",
          "agent_brief_json",
          "agent_packet_json",
          "agent_receipt_json",
          "agent_verify_json",
          "assertion",
          "before_snapshot",
          "brief",
          "command",
          "diagnostic_range",
          "label",
          "line",
          "mode",
          "owner",
          "root",
          "seam_file",
          "seam_id",
          "seam_kind",
          "severity",
          "target_artifact",
          "test_name",
          "uri"
        ]
      },
      "context": {
        "seam_packet_available": true,
        "targeted_test_brief_available": true,
        "agent_packet_command_available": true,
        "agent_brief_command_available": true,
        "after_snapshot_command_available": true,
        "agent_verify_command_available": true,
        "agent_receipt_command_available": true,
        "assertion_available": true,
        "related_test_available": true,
        "refresh_available": true
      }
    }
  ],
  "vscode_e2e": {
    "test_file": "editors/vscode/test/suite/extension.test.ts",
    "contributed_commands": [
      "ripr.copyAfterSnapshotCommand",
      "ripr.copyAgentBriefCommand",
      "ripr.copyAgentPacketCommand",
      "ripr.copyAgentReceiptCommand",
      "ripr.copyAgentVerifyCommand",
      "ripr.copyContext",
      "ripr.copySuggestedAssertion",
      "ripr.copyTargetedTestBrief",
      "ripr.openRelatedTest",
      "ripr.openSettings",
      "ripr.restartServer",
      "ripr.showOutput",
      "ripr.showStatus"
    ],
    "covered_commands": [
      "ripr.collectContext",
      "ripr.copyAfterSnapshotCommand",
      "ripr.copyAgentBriefCommand",
      "ripr.copyAgentPacketCommand",
      "ripr.copyAgentReceiptCommand",
      "ripr.copyAgentVerifyCommand",
      "ripr.copyContext",
      "ripr.copySuggestedAssertion",
      "ripr.copyTargetedTestBrief",
      "ripr.openRelatedTest",
      "ripr.openSettings",
      "ripr.restartServer",
      "ripr.showOutput",
      "ripr.showStatus"
    ],
    "covered_contributed_commands": [
      "ripr.copyAfterSnapshotCommand",
      "ripr.copyAgentBriefCommand",
      "ripr.copyAgentPacketCommand",
      "ripr.copyAgentReceiptCommand",
      "ripr.copyAgentVerifyCommand",
      "ripr.copyContext",
      "ripr.copySuggestedAssertion",
      "ripr.copyTargetedTestBrief",
      "ripr.openRelatedTest",
      "ripr.openSettings",
      "ripr.restartServer",
      "ripr.showOutput",
      "ripr.showStatus"
    ],
    "uncovered_contributed_commands": []
  }
}
```

`status` is `pass` when at least one fixture pins LSP diagnostics/actions and
all contributed VS Code commands are represented in the e2e command coverage
scan. It is `warn` when no LSP fixture expectations are present or a contributed
command is not represented in the e2e command scan. The report is not a schema
for LSP protocol messages; those remain pinned by fixture expectations and LSP
unit tests.

## Gap Decision Ledger

`ripr reports gap-ledger --records <path>` renders a read-only advisory ledger
from explicit `GapRecord` input. The input may be a `records` array,
`gap_records` array, raw record array, or the `fixtures/gap-decision-ledger`
corpus shape where each case contains `expected_gap_record`.

`ripr reports gap-ledger --repo-exposure <path>` derives conservative
repo-scoped Rust `GapRecord` entries from existing
`seams[].evidence_record.canonical_item` data in a repo-exposure report. This
does not rerun analysis or make PR-local gate/comment claims; derived records
are repo-scoped projection inputs for reports, badges, LSP diagnostics, and
agent packets when the evidence record already supplies a repair route and
verification command.

`ripr reports gap-ledger --check-output <path>` derives PR-local
presentation/output contract gap records from an existing check JSON
`finding_alignment.items[]` section. Supported visible output text without a
checked observer becomes `MissingOutputContract` with
`repair_route.route_kind = "AddOutputGolden"` and
`verification_commands = ["cargo xtask goldens check"]`. Visibility-unknown
presentation text remains a static limitation and does not become a generic
`static_unknown` repair instruction.

The command writes JSON to `target/ripr/reports/gap-decision-ledger.json` and
Markdown to `target/ripr/reports/gap-decision-ledger.md` by default. It does
not rerun analysis, infer analyzer truth, publish comments, edit source,
generate tests, call providers, run mutation testing, change gate policy, or
make CI blocking by default.

JSON shape:

```jsonc
{
  "schema_version": "0.1",
  "tool": "ripr",
  "kind": "gap_decision_ledger",
  "status": "advisory",
  "root": ".",
  "generated_at": "unix_ms:1778710000000",
  "inputs": {
    "source_kind": "records",
    "records": "fixtures/gap-decision-ledger/corpus.json"
  },
  "summary": {
    "records_total": 18,
    "repairable_total": 9,
    "static_limitation_total": 1,
    "no_action_total": 5,
    "missing_artifact_total": 1,
    "projection_pr_comment_eligible": 5,
    "projection_gate_candidate": 1,
    "projection_agent_packet_eligible": 10,
    "ripr_zero_target_count": 1,
    "ripr_plus_target_count": 2,
    "preview_ineligible_total": 1,
    "receipt_improved_total": 1,
    "receipt_unchanged_after_attempt_total": 1,
    "missing_output_contract_total": 1
  },
  "records": [
    {
      "gap_id": "gap:pr:pricing:threshold-boundary",
      "canonical_gap_id": "gap:rust:pricing:discount:threshold-boundary",
      "kind": "MissingBoundaryAssertion",
      "language": "rust",
      "language_status": "stable",
      "scope": "pr_local",
      "evidence_class": "predicate_boundary",
      "gap_state": "actionable",
      "policy_state": "new",
      "repairability": "repairable",
      "repair_route": {
        "route_kind": "AddBoundaryAssertion",
        "target_file": "tests/pricing.rs",
        "assertion_shape": "assert_eq!(discount(100, 100), 90)"
      },
      "anchor": {
        "file": "src/pricing.rs",
        "line": 42,
        "owner": "pricing::discount",
        "dedupe_fingerprint": "gap:rust:pricing:discount:threshold-boundary"
      },
      "evidence_ids": [
        "evidence:pricing-threshold-reached"
      ],
      "projection_eligibility": {
        "pr_comment": {
          "eligible": true,
          "reason": "stable_anchor_and_repair_route"
        },
        "gate_candidate": {
          "eligible": true,
          "reason": "safe_gate_predicate_satisfied"
        }
      },
      "verification_commands": [
        "cargo xtask fixtures boundary_gap"
      ],
      "safe_gate_predicate": {
        "policy_target_enabled": true,
        "suppressed": false,
        "waived": false,
        "acknowledged_only": false,
        "baseline_known": false,
        "preview_language": false,
        "static_unknown_only": false
      },
      "receipt": {
        "state": "missing_receipt",
        "movement": "missing_receipt"
      },
      "authority_boundary": "gate_decision_artifact_only"
    }
  ],
  "warnings": [],
  "limits": [
    "Advisory static gap decisions only.",
    "Gate-decision artifacts remain the only configured pass/fail authority."
  ]
}
```

`status` is `advisory` when records parse cleanly, `advisory_with_warnings`
when records are present but violate projection-safety checks, and `blocked`
when no records can be read. The summary counts are projection inputs only;
they are not gate authority.

## Mutation Calibration Reports

`ripr calibrate cargo-mutants --mutants-json <path> --repo-exposure-json <repo-exposure-json>`
joins an existing repo exposure report with imported cargo-mutants JSON/output
and prints Markdown by default:

```bash
ripr calibrate cargo-mutants \
  --mutants-json target/mutants/outcomes.json \
  --repo-exposure-json target/ripr/pilot/after.repo-exposure.json
```

Use `--format json` for the JSON shape below, and `--out <path>` to write the
rendered report to a file. Repo-local automation can still write
`target/ripr/reports/mutation-calibration.{json,md}` through
`cargo xtask mutation-calibration`.

`<path>` may point directly at a JSON file or at a cargo-mutants output
directory. When given a directory, the command reads and combines
`outcomes.json` and `mutants.json` when both are present, preserving runtime
outcomes and generated mutant locations for matching.

This is an advisory runtime calibration report, not a static finding surface.
Runtime outcome labels come from the supplied mutation output and are kept under
the `runtime` side of each match. Static reports continue using the audit
vocabulary (`test grip`, `missing discriminator`, `static evidence`, `runtime
confirmation`).

JSON shape:

```jsonc
{
  "schema_version": "0.1",
  "scope": "repo",
  "status": "advisory",
  "metrics": {
    "static_seams_total": 120,
    "mutants_total": 8,
    "matched_total": 6,
    "ambiguous_file_line_total": 1,
    "unmatched_mutants_total": 1,
    "static_without_runtime_total": 113,
    "runtime_outcome_counts": {
      "caught": 5,
      "timeout": 3
    },
    "join_method_counts": {
      "seam_id": 4,
      "file_line": 2
    }
  },
  "agreement": {
    "static_gap_and_runtime_signal": 18,
    "static_gap_without_runtime_signal": 4,
    "runtime_signal_without_static_gap": 3,
    "static_clean_and_runtime_clean": 41,
    "runtime_inconclusive": 2
  },
  "precision_notes": [
    "runtime gap signals are imported runtime labels such as missed, survived, not_caught, or uncaught"
  ],
  "missed_runtime_signals": [
    {
      "runtime": {
        "mutant_id": "m9",
        "seam_id": "f3c9e4d21a0b7c88",
        "file": "src/pricing.rs",
        "line": 88,
        "mutation_operator": "replace >= with >",
        "runtime_outcome": "missed",
        "duration": "123",
        "test_command": "cargo test pricing"
      },
      "static": {
        "seam_id": "f3c9e4d21a0b7c88",
        "seam_kind": "predicate_boundary",
        "file": "src/pricing.rs",
        "line": 88,
        "seam_grip_class": "strongly_gripped",
        "oracle_kind": "exact_value",
        "oracle_strength": "strong",
        "observed_values": ["50", "10000"],
        "missing_discriminators": []
      },
      "confidence_label": "contradicts_static_clean",
      "reason": "runtime gap signal joined to a static-clean seam"
    }
  ],
  "static_only_findings": [
    {
      "static": {
        "seam_id": "a1b2c3d4e5f60718",
        "seam_kind": "return_value",
        "file": "src/pricing.rs",
        "line": 90,
        "seam_grip_class": "weakly_gripped",
        "oracle_kind": "smoke",
        "oracle_strength": "smoke",
        "observed_values": [],
        "missing_discriminators": ["exact returned value assertion"]
      },
      "confidence_label": "contradicts_static_gap",
      "reason": "static gap seam matched runtime data without a runtime gap signal"
    }
  ],
  "matches": [
    {
      "join_method": "seam_id",
      "static": {
        "seam_id": "f3c9e4d21a0b7c88",
        "seam_kind": "predicate_boundary",
        "file": "src/pricing.rs",
        "line": 88,
        "seam_grip_class": "weakly_gripped",
        "oracle_kind": "exact_value",
        "oracle_strength": "strong",
        "observed_values": ["50", "10000"],
        "missing_discriminators": ["amount == discount_threshold (equality boundary)"]
      },
      "runtime": {
        "mutant_id": "m1",
        "seam_id": "f3c9e4d21a0b7c88",
        "file": "src/pricing.rs",
        "line": 88,
        "mutation_operator": "replace >= with >",
        "runtime_outcome": "caught",
        "duration": "123",
        "test_command": "cargo test pricing"
      },
      "confidence_label": "contradicts_static_gap"
    }
  ],
  "ambiguous_file_line_matches": [
    {
      "runtime": {
        "mutant_id": "m7",
        "seam_id": null,
        "file": "src/pricing.rs",
        "line": 88,
        "mutation_operator": "replace >= with >",
        "runtime_outcome": "caught",
        "duration": "99",
        "test_command": "cargo test pricing"
      },
      "confidence_label": "ambiguous_runtime_join",
      "candidates": [
        {
          "seam_id": "f3c9e4d21a0b7c88",
          "seam_kind": "predicate_boundary",
          "file": "src/pricing.rs",
          "line": 88,
          "seam_grip_class": "weakly_gripped",
          "oracle_kind": "exact_value",
          "oracle_strength": "strong",
          "observed_values": ["50", "10000"],
          "missing_discriminators": [
            "amount == discount_threshold (equality boundary)"
          ]
        },
        {
          "seam_id": "a1b2c3d4e5f60718",
          "seam_kind": "return_value",
          "file": "src/pricing.rs",
          "line": 88,
          "seam_grip_class": "ungripped",
          "oracle_kind": "unknown",
          "oracle_strength": "unknown",
          "observed_values": [],
          "missing_discriminators": []
        }
      ]
    }
  ],
  "unmatched_mutants": [],
  "static_without_runtime_sample": []
}
```

Field contract:

- `schema_version` — currently `"0.1"`.
- `status` — always `"advisory"`; this report does not block CI by default.
- `metrics.static_seams_total` — count of seams imported from
  `repo-exposure.json`.
- `metrics.mutants_total` — count of runtime mutation records imported from the
  supplied JSON.
- `metrics.matched_total` — runtime records joined to a static seam.
- `metrics.ambiguous_file_line_total` — runtime records whose normalized
  file/line matched multiple static seams and were therefore not assigned to a
  single seam.
- `metrics.unmatched_mutants_total` — runtime records that could not be joined
  by `seam_id` or file/line.
- `metrics.static_without_runtime_total` — static seams with no definitive or
  ambiguous runtime record in this import.
- `metrics.runtime_outcome_counts` — counts keyed by normalized runtime outcome
  label from the imported data.
- `metrics.join_method_counts` — counts for `seam_id` and `file_line` joins.
- `agreement.static_gap_and_runtime_signal` — static gap seams that also have at
  least one matched runtime gap signal in this import.
- `agreement.static_gap_without_runtime_signal` — static gap seams with no
  matched runtime gap signal in this import. This includes seams with only
  runtime-clean labels, only runtime-inconclusive labels, or no matched runtime
  record.
- `agreement.runtime_signal_without_static_gap` — runtime gap signals joined to
  static-clean seams, plus unmatched runtime gap signals.
- `agreement.static_clean_and_runtime_clean` — static-clean seams with matched
  runtime-clean labels and no matched runtime gap signal.
- `agreement.runtime_inconclusive` — matched or ambiguous runtime records whose
  imported labels are neither runtime gap signals nor runtime-clean signals.
- `precision_notes[]` — short notes explaining the report's advisory agreement
  mapping. The report treats imported labels such as `missed`, `survived`,
  `not_caught`, and `uncaught` as runtime gap signals, and labels such as
  `caught` and `timeout` as runtime-clean signals.
- `missed_runtime_signals[]` — capped sample of runtime gap signals that did
  not correspond to a static gap. `static` is `null` when the runtime record did
  not join to a seam.
- `missed_runtime_signals[].confidence_label` — `contradicts_static_clean` when
  a runtime gap signal joined to a static-clean seam, or `runtime_only_signal`
  when a runtime gap signal did not join to any static seam. This is advisory
  calibration context only and does not create a static gap.
- `static_only_findings[]` — capped sample of static gap seams without a
  matched runtime gap signal.
- `static_only_findings[].confidence_label` — `contradicts_static_gap` when a
  static gap joined only to runtime-clean labels, or `no_runtime_data` when no
  usable runtime signal was available for the static gap in this import.
- `matches[].join_method` — `seam_id` when the runtime record carries a matching
  seam/probe ID; otherwise `file_line` when normalized path and line match.
- `matches[].static` — static seam evidence copied from `repo-exposure.json`:
  seam identity, class, strongest visible oracle kind/strength, observed values,
  and missing discriminators.
- `matches[].runtime` — imported runtime mutation record: mutation ID when
  available, seam/probe ID when available, location, operator, outcome, duration,
  and test command.
- `matches[].confidence_label` — per-match static/runtime confidence label:
  `supports_static_gap`, `contradicts_static_gap`, `supports_static_clean`,
  `contradicts_static_clean`, or `no_runtime_data`. Runtime-inconclusive labels
  map to `no_runtime_data` because they provide no usable support or
  contradiction for the static claim.
- `ambiguous_file_line_matches[]` — runtime records that matched multiple
  static seams by normalized file/line. These records are intentionally not
  assigned to `matches[]` without a stronger seam/probe ID.
- `ambiguous_file_line_matches[].confidence_label` — always
  `ambiguous_runtime_join`; ambiguous joins do not raise or lower confidence for
  any candidate seam.
- `unmatched_mutants[]` — runtime records that did not match a static seam.
- `static_without_runtime_sample[]` — capped sample of static seams with no
  definitive or ambiguous runtime data in this import. Use
  `static_without_runtime_total` for the full count.

## No-Panic Allowlist Proposal Reports

`cargo xtask check-no-panic-family --propose` writes review-only migration
artifacts:

```text
target/ripr/reports/no-panic-allowlist-proposals.md
target/ripr/reports/no-panic-allowlist-proposals.toml
```

These reports are policy-maintenance aids, not product JSON contracts. They are
generated from current panic-family findings and the canonical
`policy/no-panic-allowlist.toml` ledger. They may propose semantic selectors
for legacy line/column entries, current semantic entries, or narrower selectors
for ambiguous entries, but they never rewrite the allowlist.

The Markdown report records each candidate's current finding, confidence,
whether it replaces a v0.1 entry, old coordinates when available, new
`last_seen` values, preserved reason text, suggested selector, and review
warnings. The TOML report uses schema `0.3` with `status = "proposal"` and is a
copy aid only; reviewers must supply real ids, owners, expiries, and rationale
before adopting any proposal.

## Stability Rules

Output contract values are registered in `policy/output_contracts.txt`.

Run:

```bash
cargo xtask check-output-contracts
```

Additive fields are allowed within the same schema version.

Do not remove fields, rename fields, or change enum meanings without bumping the
schema version.

Do not emit mutation-runtime terms such as `killed` or `survived` in static JSON.
