# RIPR-SPEC-0022: Assistant Loop Health Report

Status: proposed

## Problem

Campaign 21 made one assistant-directed test loop reviewable as an advisory
`test-oracle-assistant-proof.{json,md}` packet. Campaign 22 then made one next
action visible from the wider RIPR evidence set.

Those surfaces answer local review questions:

```text
What did this assistant loop target?
What is the next useful action?
```

They do not yet answer the operating question:

```text
Are assistant-directed test loops complete, stuck, missing receipts, or moving
static evidence over time?
```

Assistant loop health summarizes one or more existing proof reports into an
advisory operating dashboard for maintainers and coding agents. It should make
missing proof inputs, unchanged static movement, recurring warnings, and repair
queues visible without adding another policy engine or another analyzer pass.

## Product Contract

The assistant loop health report is a read-only aggregate over explicit
`test-oracle-assistant-proof` JSON artifacts. It does not create proof reports,
rerun hidden analysis, inspect source files, edit source, generate tests, call a
provider, run mutation testing, change recommendation ranking, change gate
policy, change LSP/editor behavior, or change default CI blocking.

The report must:

- count proof packets by completeness state;
- summarize static movement as `improved`, `unchanged`, `regressed`, or
  `unknown`;
- preserve recurring warnings and missing-input details;
- produce a bounded repair queue for maintainers and coding agents;
- support one proof file now and multiple proof files later;
- keep optional gate, coverage, ledger, and first-action context separate from
  pass/fail authority;
- emit JSON for tools and Markdown for CI summaries and human review;
- keep static evidence vocabulary conservative and advisory.

## Behavior

The canonical behavior is:

```text
explicit proof report paths
-> parse and classify proof completeness
-> normalize static movement buckets
-> group missing inputs and warnings
-> build bounded repair queue
-> advisory JSON and Markdown
```

The producer must read explicit input paths. It must not search hidden state or
rerun analysis to fill missing proof data.

## Inputs

The first public command surface is:

```text
ripr assistant-loop health \
  --proof target/ripr/reports/test-oracle-assistant-proof.json \
  --out target/ripr/reports/assistant-loop-health.json \
  --out-md target/ripr/reports/assistant-loop-health.md
```

`--proof` is repeatable. The first implementation may validate one proof path
first, but the JSON contract must model `inputs.proofs` as a list so later
campaign slices can summarize multiple PR, CI, or dogfood proof packets without
schema churn.

Required command inputs:

- at least one explicit `--proof` path;
- output path for JSON;
- output path for Markdown when Markdown is requested.

Optional proof-context fields, copied only when already present in a proof
report:

- selected seam identity, path, line, kind, static class, and missing
  discriminator;
- recommendation placement, related test, suggested test, and verify command;
- handoff artifact and agent command;
- receipt artifact and before/after movement;
- PR ledger, optional gate decision, optional coverage/grip frontier, and
  first-action report paths;
- warning strings and static limits.

Missing optional proof-context fields become warnings or `null` values. Missing
fields must not be treated as acknowledgement, suppression, success, runtime
confirmation, or policy state.

## Required Evidence

The health report can only summarize evidence already present in supplied proof
artifacts. A useful proof item should provide:

- the explicit proof artifact path and compatible schema version;
- selected seam identity or a clear missing-required-input warning;
- selected seam path and line when known;
- recommendation or handoff context that explains what work was attempted;
- receipt or static movement context when before/after evidence was available;
- proof warnings for missing, stale, ambiguous, malformed, unsupported, or
  summary-only inputs;
- advisory limits copied from the proof report.

The health report may also preserve optional proof context:

- PR ledger path;
- gate decision path;
- coverage/grip frontier path;
- first-useful-action path;
- related test, suggested test, missing discriminator, verify command, and
  receipt command.

Missing optional context remains visible as `null`, `unknown`, or a bounded
warning. Missing required proof evidence produces a `missing_required_input`
proof item and repair guidance. The health report must not synthesize missing
evidence from the workspace.

## Health Status Vocabulary

Top-level `status` must be one of:

- `advisory`: at least one proof path was read and summarized.
- `incomplete`: no proof path could be read or every supplied proof was
  malformed or incompatible.

Each proof item uses `proof_state`:

- `complete`: the proof has a selected seam, recommendation or handoff context,
  static movement or receipt context, and no missing-required proof warning.
- `partial`: the proof is parseable and tied to a seam, but optional context or
  non-fatal artifacts are missing.
- `missing_required_input`: the proof file is unreadable, malformed,
  incompatible, or its own proof status says required proof inputs are missing.

`movement_state` must be one of:

- `improved`: the proof reports improved or resolved static movement.
- `unchanged`: the proof reports unchanged static movement after an attempt.
- `regressed`: the proof reports weaker static movement after an attempt.
- `unknown`: the proof does not include enough static movement data.

`warning_kind` must be one of:

- `missing_required_input`;
- `missing_optional_input`;
- `stale_input`;
- `malformed_input`;
- `incompatible_schema`;
- `summary_only_guidance`;
- `unchanged_movement`;
- `regressed_movement`;
- `missing_receipt`;
- `missing_handoff`;
- `unknown_movement`;
- `static_limit`;
- `other`.

`repair_kind` must be one of:

- `regenerate_proof`;
- `regenerate_missing_artifact`;
- `rerun_verify_and_receipt`;
- `refresh_before_after_evidence`;
- `inspect_unchanged_attempt`;
- `inspect_regression`;
- `inspect_summary_only_guidance`;
- `attach_receipt`;
- `no_repair`.

The report must not compute an opaque health score. The counts, warnings, and
repair queue are the health signal.

## JSON Shape

The JSON report uses schema version `0.1`:

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
- `status` uses the bounded top-level health status vocabulary.
- `inputs.proofs` records explicit proof paths in deterministic order.
- `summary.*` counts proof states, movement buckets, warnings, and repair queue
  entries. Counts must be derived from `proofs`, `warning_summary`, and
  `repair_queue`; they must not be hand-authored scores.
- `proofs[].source_artifact` is the explicit source proof path.
- `proofs[].proof_state` uses the bounded per-proof vocabulary.
- `proofs[].movement_state` normalizes existing proof movement to the bounded
  health vocabulary. A proof movement of `resolved` counts as `improved`.
- `proofs[].seam.*`, `recommendation.*`, `handoff.*`, `receipt.*`, and
  `movement.*` are copied from existing proof fields when available. The health
  report must not mint seam identities or rerank recommendations.
- `optional_context.*` records optional artifact paths when the proof names
  them. Missing optional paths are `null` plus warnings when useful.
- `warnings[].kind` and `repair_queue[].repair_kind` use bounded vocabularies.
- `limits` must preserve the static-evidence, no-provider-call, no-source-edit,
  no-generated-test, no-runtime-mutation-execution, and advisory-default
  boundaries.

## Repair Queue Shape

Repair queue entries must be bounded and mechanical:

```json
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
```

Repair entries may tell a maintainer or external coding agent to regenerate a
missing artifact, rerun verify/receipt, refresh before/after evidence, inspect
unchanged movement, inspect regressed movement, or inspect summary-only
guidance. They must not tell an agent to inspect the whole repository freely or
generate tests automatically.

## Markdown Shape

The Markdown sibling should fit in a generated CI job summary or reviewer
handoff:

```md
# RIPR Assistant Loop Health

Status: advisory

Proof packets:
- complete: 8
- partial: 2
- missing required inputs: 1
- missing optional inputs: 3

Evidence movement:
- improved: 5
- unchanged: 4
- regressed: 0
- unknown: 1

Top warnings:
- missing_optional_input: 3
- unchanged_movement: 4

Next repair queue:
- `inspect_unchanged_attempt` - src/pricing.rs:88 - unchanged movement;
  inspect whether the focused test observes the missing discriminator.
- `rerun_verify_and_receipt` - src/auth.rs:42 - missing receipt; rerun
  verify and receipt.

Limits:
- Static RIPR evidence only.
- Does not run mutation testing.
- Does not call providers.
- Does not edit source or generate tests.
- Gate evaluator remains pass/fail authority.
```

When no proof input can be read, Markdown should show the repair before empty
counts:

```md
# RIPR Assistant Loop Health

Status: incomplete

Next: provide at least one `test-oracle-assistant-proof.json` artifact.
```

## Acceptance Examples

- One complete proof with improved or resolved static movement returns
  `status = "advisory"`, `summary.complete = 1`, and
  `summary.improved = 1`.
- One proof with an unchanged receipt returns `movement_state = "unchanged"` and
  a repair queue item with `repair_kind = "inspect_unchanged_attempt"`.
- One proof with regressed movement returns `movement_state = "regressed"` and
  a repair queue item with `repair_kind = "inspect_regression"`.
- A proof missing a required receipt, selected seam, or before/after evidence
  returns `proof_state = "missing_required_input"` and a bounded repair item.
- Missing optional gate, coverage, ledger, or first-action context increments
  `missing_optional_input` and warning counts without making the report
  incomplete.
- Summary-only guidance remains visible with `warning_kind =
  "summary_only_guidance"` and a repair item only when no better proof context
  is available.
- Multiple proof paths are counted deterministically in input order and grouped
  by warning kind without producing an opaque score.

## Test Mapping

Follow-up tests and fixtures should cover:

- complete proof with improved static movement;
- complete proof whose source movement was `resolved` and is counted as
  `improved`;
- complete proof with unchanged static movement and a repair queue entry;
- complete proof with regressed static movement and a repair queue entry;
- partial proof missing optional gate, coverage, ledger, or first-action
  context;
- missing required receipt, selected seam, before/after evidence, or malformed
  proof input;
- summary-only guidance preserved as a warning;
- warning-heavy proof grouping;
- multiple proof inputs with deterministic counts and ordering;
- JSON and Markdown rendering;
- generated-CI projection that keeps the report advisory.

## Implementation Mapping

Follow-up implementation belongs to Campaign 23:

- `fixtures/assistant-loop-health-corpus` pins the complete, partial,
  missing-input, unchanged, regressed, warning-heavy, and multi-proof corpus.
- `report/assistant-loop-health` adds the read-only
  `ripr assistant-loop health` producer over explicit proof inputs.
- `ci/assistant-loop-health-artifacts` uploads and summarizes health artifacts
  only when proof artifacts exist.
- `docs/assistant-loop-health-workflow` explains proof report versus health
  report, complete and partial loops, missing-input repair, unchanged movement,
  generated-CI summary use, and advisory limits.
- `campaign/assistant-loop-health-closeout` records the prompt-to-artifact
  audit, validation, and non-goals.

No follow-up surface may change analyzer behavior, recommendation ranking, gate
policy semantics, LSP/editor behavior, source files, generated tests, provider
calls, mutation execution, public crate shape, or default CI blocking as part of
this campaign.

## Metrics

The report should make these counts available to later metrics surfaces:

- `assistant_loop_health_reports`;
- `assistant_loop_health_complete`;
- `assistant_loop_health_partial`;
- `assistant_loop_health_missing_required_input`;
- `assistant_loop_health_missing_optional_input`;
- `assistant_loop_health_static_improved`;
- `assistant_loop_health_static_unchanged`;
- `assistant_loop_health_static_regressed`;
- `assistant_loop_health_unknown_movement`;
- `assistant_loop_health_repair_queue_items`;
- `assistant_loop_health_warnings`.

## Non-Goals

- No analyzer behavior changes.
- No recommendation ranking changes.
- No gate policy semantic changes.
- No LSP or editor behavior changes.
- No generated workflow changes in this spec PR.
- No default CI blocking.
- No source edits.
- No generated tests.
- No provider or API calls.
- No mutation execution.
- No implicit artifact discovery or hidden analysis reruns.
- No opaque health score.
- No runtime correctness or sufficiency claims.

## Validation

The spec PR should run:

```bash
cargo xtask check-spec-format
cargo xtask check-doc-index
cargo xtask markdown-links
cargo xtask check-static-language
cargo xtask check-traceability
cargo xtask check-capabilities
cargo xtask check-campaign
cargo xtask goals next
cargo xtask check-pr
git diff --check
```
