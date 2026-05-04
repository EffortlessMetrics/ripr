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

## Repo Seam Inventory

`ripr check --root . --format repo-seams-json` emits the Voice B repo seam
inventory introduced by RIPR-SPEC-0005. The artifact lands at
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
- `scope` — always `"repo"` for this artifact. Distinguishes Voice B repo
  inventory from Voice A diff-scoped findings.
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

Voice B v1 inventories every probeable production syntax shape and does not
yet classify test grip; `analysis/repo-ripr-classification-v1` adds
`SeamGripClass` and the headline-eligibility table per RIPR-SPEC-0005. Static
output continues to forbid runtime-mutation outcome words.

The Markdown sibling (`repo-seams.md`, generated alongside the JSON) is
human-readable but follows the same contract for `kind`, `owner`, and
`expected_sink` strings.

## Repo Exposure Report

`ripr check --root . --format repo-exposure-json` emits the Voice B
classified seam inventory introduced by `analysis/repo-ripr-classification-v1`.
The artifact lands at `target/ripr/reports/repo-exposure.json` when generated
via `cargo xtask repo-exposure-report`.

```json
{
  "schema_version": "0.1",
  "scope": "repo",
  "metrics": {
    "seams_total": 12882,
    "headline_eligible": 12352,
    "strongly_gripped": 530,
    "weakly_gripped": 8102,
    "ungripped": 45,
    "reachable_unrevealed": 104,
    "activation_unknown": 4101,
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
          "evidence_summary": "exact value assertion"
        }
      ],
      "observed_values": ["50", "10000"],
      "missing_discriminators": [
        {
          "value": "discount_threshold (equality boundary)",
          "reason": "observed values do not include the equality-boundary case for this predicate"
        }
      ]
    }
  ]
}
```

Field contract:

- `schema_version` — currently `"0.1"`. Bumping requires updating this
  section, the renderer (`crates/ripr/src/output/repo_exposure.rs`), and
  any downstream consumers in lockstep.
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
- `seams[].observed_values` — literal scalar values seen in owner-call
  arguments across related tests. Bare identifiers and helper-derived
  values are intentionally excluded.
- `seams[].missing_discriminators` — per-rule hypothesis strings (e.g.,
  the equality-boundary case for predicate seams). Empty when no rule
  fires.

The Markdown sibling (`repo-exposure.md`) prints a metrics table plus
the top headline-eligible seams (capped at 50). Both formats are
generated together by `cargo xtask repo-exposure-report`.

Voice B reports static test grip evidence. Runtime confirmation via
`cargo-mutants` is a separate calibration step (`calibration/cargo-mutants-v1`).
Static-language constraints from RIPR-SPEC-0005 still apply: the report
never uses runtime-mutation outcome words.

## Agent Seam Packets

`ripr check --root . --format agent-seam-packets-json` emits per-seam
agent work orders for every headline-eligible classified seam. The
artifact lands at `target/ripr/reports/agent-seam-packets.json` when
generated via `cargo xtask agent-seam-packets`.

```json
{
  "schema_version": "0.2",
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
      "missing_oracle_shape": "exact returned value assertion at the equality boundary",
      "related_existing_tests": [
        {
          "name": "below_threshold_has_no_discount",
          "file": "tests/pricing.rs",
          "line": 12,
          "oracle_kind": "exact_value",
          "oracle_strength": "strong",
          "evidence_summary": "exact value assertion"
        }
      ],
      "suggested_assertions": [
        "assert_eq!(discounted_total(/* discount_threshold (equality boundary) */), /* expected */)"
      ],
      "runtime_confirmation": "optional cargo-mutants confirmation; ripr reports static evidence only"
    }
  ]
}
```

Field contract:

- `schema_version` — currently `"0.2"`. Distinct from the repo-exposure
  report's `"0.1"` because the packet is a separate contract aimed at
  coding agents rather than reviewers. Bumping requires updating this
  section, the renderer (`crates/ripr/src/output/agent_seam_packets.rs`),
  and any downstream consumers in lockstep.
- `scope` — always `"repo"`.
- `packets_total` — number of actionable packets emitted. Equals the
  count of headline-eligible seams plus opaque seams (which emit
  `inspect_static_limitation`). Strongly-gripped, intentional, and
  suppressed seams produce no packet.
- `packets[].task` — `"write_targeted_test"` for headline-eligible
  seams; `"inspect_static_limitation"` for opaque seams. Future
  versions may add tasks like `"strengthen_oracle"` or
  `"add_match_arm_observer"`.
- `packets[].current_grip` — one of the `SeamGripClass` strings the
  packet is emitted for (`weakly_gripped`, `ungripped`,
  `reachable_unrevealed`, the four `*_unknown` classes, or
  `opaque`).
- `packets[].headline_eligible` — boolean. `true` for the
  headline-eligible classes, `false` for `opaque`. Lets agents
  prioritize without re-deriving the headline mapping.
- `packets[].evidence` — per-stage `StageState` strings.
- `packets[].observed_values` — literal scalars seen in owner-call
  arguments across related tests.
- `packets[].missing_discriminators` — array of `{value, reason}`
  records mirroring the analyzer's `MissingDiscriminatorFact` shape.
  For predicate-boundary seams, includes a fallback entry naming the
  equality boundary even when no analyzer hypothesis fired.
- `packets[].missing_oracle_shape` — guidance string keyed by
  `SeamKind` and `ExpectedSink`. Examples:
  - `predicate_boundary` → "exact returned value assertion at the
    equality boundary"
  - `error_variant` → "exact error-variant assertion (matches! /
    assert_matches!)"
  - `side_effect` → "mock expectation, event/state observer, or
    persistence assertion (...)"
- `packets[].related_existing_tests` — capped at
  `MAX_RELATED_TESTS_PER_PACKET` (currently 8). Carries test name,
  file, line, oracle kind, oracle strength, and a short
  `evidence_summary` describing the oracle shape (e.g., "exact value
  assertion", "is_err / broad-error assertion").
- `packets[].suggested_assertions` — best-effort assertion templates
  the agent fills in. Placeholders are intentional; ripr never invents
  expected values. Example for predicate boundary:
  `"assert_eq!(discounted_total(/* discount_threshold (equality boundary) */), /* expected */)"`.
- `packets[].runtime_confirmation` — boilerplate string reminding the
  agent that `ripr` is preflight static evidence and runtime
  mutation confirmation (e.g., `cargo-mutants`) is a separate
  calibration step.

The packet is the agent's work order: it names the seam, the missing
discriminator, the oracle shape, and an assertion template — but never
generates the test itself. Composition with a coding agent closes the
loop.

## LSP Seam Diagnostics

When `seamDiagnostics: true` is passed in `initializationOptions`, the
LSP server publishes a `Diagnostic` for every actionable
`ClassifiedSeam` alongside the existing diff-scoped `Finding`
diagnostics. The flag is **off by default** because the underlying
classification walks the entire production tree (~12 k seams on the
ripr repo) and would add multi-second latency to every editor
refresh; `cache/repo-seam-facts-v1` will lift the default once the
classification is cached.

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

The diagnostic range is currently a **full-line placeholder**: seams
do not yet carry a column, so the range spans `(line, 0)` →
`(line, MAX_DIAGNOSTIC_RANGE_WIDTH)`. Editors render this as a
single-line squiggle that always covers the seam regardless of
indentation. A future PR can derive the real column from the source
file via the (now reserved) `_root` parameter on
`diagnostic_for_classified_seam`.

This PR adds diagnostics only — no code actions yet. The pre-4B
`Finding`/`AnalysisSnapshot` hover continues to work for
diff-scoped diagnostics; seam diagnostics live alongside it.

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
