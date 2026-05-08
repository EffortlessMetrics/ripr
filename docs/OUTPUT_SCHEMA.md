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
```

Native schema `0.3`:

```json
{
  "schema_version": "0.3",
  "kind": "ripr",
  "scope": "repo",
  "basis": "seam_native",
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

- `schema_version` — currently `"0.3"`. `0.2` added `scope`; `0.3` adds
  `basis` and `counts.analyzed_seams`.
- `kind` — `"ripr"` or `"ripr_plus"`.
- `scope` — `"diff"` for PR/diff artifacts, `"repo"` for public repo
  baseline artifacts.
- `basis` — `"finding_exposure"` for legacy Finding/ExposureClass count
  artifacts, `"seam_native"` for RepoSeam/SeamGripClass count artifacts.
  Diff-scoped badge formats currently use `finding_exposure`; repo-scoped
  badge formats use `seam_native`.
- `message` — the headline count rendered as a string for Shields
  compatibility. It is a count, never a denominator or coverage fraction.
- `counts.unsuppressed_exposure_gaps` — diff scope: unsuppressed
  `weakly_exposed`, `reachable_unrevealed`, and `no_static_path` Findings;
  repo scope: configured-visible headline-eligible seam classes.
- `counts.unknowns` — diff scope: static unknown Finding classes; repo
  scope: configured-visible `opaque` seams.
- `counts.analyzed_findings` — number of Findings considered by the
  finding-exposure basis; `0` for seam-native repo badges.
- `counts.analyzed_seams` — number of classified seams considered by the
  seam-native basis; `0` for finding-exposure diff badges.
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
          "value": "discount_threshold (equality boundary)",
          "reason": "observed values do not include the equality-boundary case for this predicate"
        }
      ]
    }
  ]
}
```

Field contract:

- `schema_version` — currently `"0.2"`. Bumping requires updating this
  section, the renderer (`crates/ripr/src/output/repo_exposure.rs`), and
  any downstream consumers in lockstep. `0.1` → `0.2`: per-related-test
  entries gained `relation_reason` and `relation_confidence` fields
  (`analysis/related-test-precision-v1`).
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
  `direct_owner_call`, `assertion_target_affinity`, `same_test_file`,
  `same_module`, `owner_named_test`, `import_path_affinity`,
  `fixture_owner_affinity`. Detection lives in
  `crates/ripr/src/analysis/test_grip_evidence.rs`.
- `seams[].related_tests[].relation_confidence` — `high`, `medium`,
  `low`, or `opaque`. Mapping from reason: `direct_owner_call` and
  `assertion_target_affinity` → `high`; `same_test_file`,
  `same_module`, `owner_named_test`, `import_path_affinity` →
  `medium`; `fixture_owner_affinity` → `low`. Independent of
  `oracle_strength`: a `low` relation can still carry a strong oracle.
- The `related_tests` array is **ranked** by
  `(confidence, reason_priority, file, name, line)` so the
  highest-confidence tests appear first. `related_tests_total` is
  unaffected by ranking.
- `seams[].observed_values` — literal scalar values seen in owner-call
  arguments across related tests. Bare identifiers and helper-derived
  values are intentionally excluded.
- `seams[].missing_discriminators` — per-rule hypothesis strings (e.g.,
  the equality-boundary case for predicate seams). Empty when no rule
  fires.

The Markdown sibling (`repo-exposure.md`) prints a metrics table plus
the top headline-eligible seams (capped at 50). Both formats are
generated together by `cargo xtask repo-exposure-report`.

This report shows static test-grip evidence for repo seams. Runtime
confirmation via `cargo-mutants` is a separate calibration step
(`calibration/cargo-mutants-v1`). Static-language constraints from
RIPR-SPEC-0005 still apply: the report never uses runtime-mutation
outcome words.

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
        "missing discriminator no longer reported: discount_threshold (equality boundary)",
        "stronger related oracle visible: weak -> strong"
      ]
    }
  ],
  "unchanged": [],
  "regressed": [],
  "new": [],
  "removed": []
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
  grip classes, a direction string, and evidence-delta hints derived from
  rendered repo-exposure fields.
- `evidence_delta[]` — advisory hints such as missing discriminators no longer
  reported, new observed values, or stronger related oracles. These hints are
  based on the rendered static artifact and do not claim runtime confirmation.
- `new[]` / `removed[]` — seam identity and grip class for seam IDs present in
  only one input.

The Markdown surface prints the same summary and highlights moved, unchanged,
regressed, new, and removed seams for human review. Unchanged seams can still
carry evidence-delta hints, such as a new observed value, so reviewers can see
when a targeted test improved rendered evidence without changing the grip class.

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
      ]
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
    "ripr_version": "0.4.0",
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
ripr review-comments --root . --base <sha> --head <sha> --out target/ripr/review/comments.json
```

The command is a pure renderer. It does not post to GitHub, run mutation
testing, refresh LSP state, edit source files, or generate tests. CI can use
the JSON to write a job summary and emit check annotations by default. Inline
PR review comments require a custom explicit opt-in publisher.

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
- `summary_only[]` - same recommendation shape without `placement`. CI should
  show these in the Markdown/job summary but must not invent a changed-line
  annotation for them.
- `suppressed[]` - bounded records for recommendations hidden by caps or
  nearby test changes.
- `warnings[]` - selection warnings from the agent brief selection path.
- `limits_note` - static-evidence boundary text for downstream summaries.

Default CI projection runs `ripr review-comments` on pull requests, writes
summary items to the job summary, and emits check annotations only for changed
lines. Inline PR review comments remain opt-in; any custom publisher must cap
them to three by default. See [PR review guidance](PR_REVIEW_GUIDANCE.md) for
the command, generated CI behavior, placement-safe review flow, and
inline-comment boundary.

## Calibrated Gate Decision

RIPR-SPEC-0013 defines the planned contract for optional calibrated gate
decisions over existing PR-time evidence. The gate evaluator is explicit policy
over PR guidance, repo exposure, configured severity and suppressions, labels,
optional SARIF policy reports, optional before/after receipts, and optional
imported mutation calibration. It does not run mutation testing, post comments,
edit source files, generate tests, upload SARIF, or change generated workflow
defaults.

Planned command:

```text
ripr gate --root . \
  --review-comments target/ripr/review/comments.json \
  --repo-exposure target/ripr/pilot/repo-exposure.json \
  --policy ripr.toml \
  --labels-json target/ripr/review/labels.json \
  --out target/ripr/review/gate-decision.json
```

Planned outputs:

```text
target/ripr/review/gate-decision.json
target/ripr/review/gate-decision.md
```

JSON shape:

```json
{
  "schema_version": "0.1",
  "tool": "ripr",
  "status": "acknowledged",
  "mode": "acknowledgeable-soft-gate",
  "root": ".",
  "inputs": {
    "review_comments": "target/ripr/review/comments.json",
    "repo_exposure": "target/ripr/pilot/repo-exposure.json",
    "policy": "ripr.toml",
    "labels": ["ripr-waive"],
    "mutation_calibration": null
  },
  "summary": {
    "evaluated": 2,
    "blocking": 0,
    "acknowledged": 1,
    "advisory": 1,
    "suppressed": 0,
    "unknown_confidence": 0
  },
  "decisions": [
    {
      "id": "ripr-gate-67fc764ba37d77bd",
      "seam_id": "67fc764ba37d77bd",
      "source": "review_comments",
      "decision": "acknowledged",
      "gate_reason": "policy-eligible gap acknowledged by ripr-waive",
      "static_class": "weakly_gripped",
      "severity": "warning",
      "placement": {
        "path": "src/pricing.rs",
        "line": 88
      },
      "policy": {
        "mode": "acknowledgeable-soft-gate",
        "acknowledgement_label": "ripr-waive",
        "threshold": "high_confidence_new_gap"
      },
      "evidence": {
        "missing_discriminator": "amount == discount_threshold",
        "suggested_test": "Add an equality-boundary test.",
        "related_test": "tests/pricing.rs::applies_discount_above_threshold",
        "nearby_test_changed": false,
        "suppressed": false,
        "calibration": {
          "available": false,
          "confidence_effect": "not_used"
        }
      }
    }
  ],
  "warnings": [],
  "limits_note": "Optional policy over static RIPR evidence; advisory by default; runtime calibration is used only when supplied."
}
```

Field contract:

- `schema_version` - currently `"0.1"`.
- `status` - one of `pass`, `advisory`, `acknowledged`, `fail`, or
  `config_error`.
- `mode` - one of `advisory`, `baseline-check`,
  `fail-on-new-high-confidence-gap`, or `acknowledgeable-soft-gate`.
- `inputs` - artifact paths and labels used by the decision. Missing optional
  inputs remain visible.
- `summary.evaluated` - candidate recommendations considered.
- `summary.blocking` - decisions that made the gate fail.
- `summary.acknowledged` - decisions made non-failing by an acknowledgement
  label.
- `summary.advisory` - visible non-blocking decisions.
- `summary.suppressed` - candidates hidden by configured policy.
- `summary.unknown_confidence` - candidates that could not satisfy confidence
  requirements.
- `decisions[].decision` - `blocking`, `acknowledged`, `advisory`,
  `suppressed`, or `not_applicable`.
- `decisions[].gate_reason` - short policy explanation.
- `decisions[].static_class` - static seam or finding class from RIPR output.
- `decisions[].policy` - mode, threshold, and acknowledgement fields that
  affected the result.
- `decisions[].evidence` - static evidence, nearby-test state, suppression
  state, and optional calibration confidence effect.
- `warnings[]` - missing inputs, unsupported labels, ambiguous calibration, or
  baseline limitations.
- `limits_note` - static/runtime and advisory-default boundary text.

Generated workflows must not run this gate by default. A future workflow may
opt in with an explicit gate mode, but generated CI remains advisory unless
that setting is present.

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
  "version": "0.4.0",
  "status": "warn",
  "checks": [
    {
      "id": "installed-command-surface",
      "status": "pass",
      "required": true,
      "command": "target/ripr/release-readiness/install/bin/ripr --help",
      "summary": "installed binary exposes the 0.4 public loop commands",
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
      "runtime_confirmation": "optional cargo-mutants confirmation; ripr reports static evidence only"
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
- `packets[].runtime_confirmation` — boilerplate string reminding the
  agent that `ripr` is preflight static evidence and runtime
  mutation confirmation (e.g., `cargo-mutants`) is a separate
  calibration step.

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

Seam diagnostics also drive editor code actions:

- `Inspect seam: copy packet` calls `ripr.collectContext` with `seam_id` and
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
- `Refresh analysis: rerun saved-workspace check` remains available for every
  request.

These actions do not edit files, generate tests, or add CodeLens
surface. The pre-4B `Finding`/`AnalysisSnapshot` hover and context
actions continue to work for diff-scoped diagnostics; seam diagnostics
live alongside them.

## LSP Cockpit Report

`cargo xtask lsp-cockpit-report` writes:

```text
target/ripr/reports/lsp-cockpit.json
target/ripr/reports/lsp-cockpit.md
```

The report is an advisory dogfood artifact for the editor loop. It reads the
committed LSP fixture expectations, plus the VS Code e2e smoke test file, and
summarizes the editor surface without opening VS Code.

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
          "Inspect seam: copy packet",
          "Write targeted test: copy brief",
          "Agent handoff: copy packet command",
          "Agent handoff: copy brief command",
          "Verify after test: copy after-snapshot command",
          "Verify after test: copy verify command",
          "Review result: copy receipt command",
          "Write targeted test: copy suggested assertion",
          "Write targeted test: open best related test",
          "Refresh analysis: rerun saved-workspace check"
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
      }
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
- `static_only_findings[]` — capped sample of static gap seams without a
  matched runtime gap signal.
- `matches[].join_method` — `seam_id` when the runtime record carries a matching
  seam/probe ID; otherwise `file_line` when normalized path and line match.
- `matches[].static` — static seam evidence copied from `repo-exposure.json`:
  seam identity, class, strongest visible oracle kind/strength, observed values,
  and missing discriminators.
- `matches[].runtime` — imported runtime mutation record: mutation ID when
  available, seam/probe ID when available, location, operator, outcome, duration,
  and test command.
- `ambiguous_file_line_matches[]` — runtime records that matched multiple
  static seams by normalized file/line. These records are intentionally not
  assigned to `matches[]` without a stronger seam/probe ID.
- `unmatched_mutants[]` — runtime records that did not match a static seam.
- `static_without_runtime_sample[]` — capped sample of static seams with no
  definitive or ambiguous runtime data in this import. Use
  `static_without_runtime_total` for the full count.

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
