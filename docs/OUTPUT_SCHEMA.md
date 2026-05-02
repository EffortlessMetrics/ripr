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
  "evidence": [],
  "missing": [],
  "related_tests": [],
  "stop_reasons": [],
  "recommended_next_step": "Add boundary tests with exact assertions."
}
```

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
