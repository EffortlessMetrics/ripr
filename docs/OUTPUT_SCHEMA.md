# Output Schema

`ripr` emits stable JSON for tools, CI systems, editor integrations, and coding
agents.

The current schema version is:

```text
0.1
```

Schema changes that remove fields, rename fields, or change field meanings
should bump `schema_version`.

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
  "suggested_next_action": "Add boundary tests with exact assertions."
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

Reserved `flow_sink` values:

- `return_value`
- `error_variant`
- `struct_field`
- `call_effect`
- `match_arm`
- `unknown`

These labels are internal analysis terms in schema `0.1`. They are documented
now so future evidence-first output can expose them without inventing new
contract language.

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
