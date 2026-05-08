# RIPR-SPEC-0016: Baseline Debt Delta

Status: accepted

## Problem

Campaign 15 added an optional read-only gate evaluator, and Campaign 16 made
gate adoption visible and safer for teams. A repository can now see advisory
findings, acknowledgement labels, suppressions, baseline-aware decisions, and
explicit calibrated gate outcomes.

The next adoption gap is operational. Teams should not have to hand-author a
baseline ledger or inspect raw gate-decision JSON to answer:

```text
Which reviewed baseline gaps are still present?
Which reviewed gaps disappeared?
Which current gaps are new policy-eligible debt?
Which current gaps were acknowledged or suppressed?
Which baseline records are stale, malformed, or missing current inputs?
```

Without a deterministic debt delta report, RIPR can tell a PR what policy would
do, but it cannot yet make historical behavioral-grip debt easy to shrink.

## Product Contract

The baseline debt delta report is advisory movement evidence over existing gate
decisions and reviewed baseline records. It does not make policy decisions.
`ripr gate evaluate` remains the pass/fail authority for configured gate modes.

The contract is:

- baselines are reviewed historical-debt ledgers, not suppressions;
- the report compares current evidence to an explicit checked-in baseline;
- new debt is visible and never auto-adopted;
- resolved baseline entries are visible so teams can shrink the ledger;
- acknowledged and suppressed current findings remain visible in separate
  buckets;
- malformed or stale baseline records are repair-oriented warnings, not hidden
  success;
- generated CI may upload and summarize the report, but the report itself does
  not fail CI;
- the report never changes analyzer identity, gate policy semantics, LSP
  behavior, generated workflow defaults, or source files.

## Behavior

The command surface is:

```text
ripr baseline diff \
  --baseline .ripr/gate-baseline.json \
  --current target/ripr/reports/gate-decision.json \
  --out target/ripr/reports/baseline-debt-delta.json \
  --out-md target/ripr/reports/baseline-debt-delta.md
```

The command writes:

```text
target/ripr/reports/baseline-debt-delta.json
target/ripr/reports/baseline-debt-delta.md
```

The report producer must not:

- create or update baselines;
- post GitHub comments;
- edit source files;
- generate tests;
- run mutation testing;
- rerun analysis unless a later explicit refresh mode is specified;
- change generated workflow defaults;
- turn advisory movement evidence into a gate failure.

## Inputs

Required inputs:

- `.ripr/gate-baseline.json` or an equivalent explicit baseline path;
- `target/ripr/reports/gate-decision.json` or an equivalent current gate
  decision path.

Optional inputs:

- PR guidance JSON such as `target/ripr/review/comments.json`;
- agent receipt JSON such as `target/ripr/reports/agent-receipt.json`;
- targeted-test outcome JSON;
- recommendation calibration JSON;
- labels JSON captured by generated CI;
- repository config and suppressions when already available to the caller.

Missing optional inputs must appear as `missing_current_input`, `unknown`, or a
warning. Missing required inputs should produce an incomplete report with repair
guidance. The report must not synthesize current evidence from the baseline.

## Baseline Record Contract

Baseline records are stable identities plus review metadata. The first baseline
ledger shape is expected to be:

```json
{
  "schema_version": "0.1",
  "tool": "ripr",
  "kind": "gate_baseline",
  "created_at": "2026-05-08T00:00:00Z",
  "source_report": "target/ripr/reports/gate-decision.json",
  "mode": "baseline-check",
  "reviewed": false,
  "entries": [
    {
      "identity": {
        "seam_id": "67fc764ba37d77bd",
        "source_id": "ripr-review-67fc764ba37d77bd",
        "id": "ripr-gate-67fc764ba37d77bd",
        "dedupe_key": "ripr:67fc764ba37d77bd:src/pricing.rs:88",
        "fallback": "src/pricing.rs:88:weakly_gripped"
      },
      "path": "src/pricing.rs",
      "line": 88,
      "static_class": "weakly_gripped",
      "decision": "advisory",
      "review": {
        "reviewed": false,
        "reason": "initial adoption baseline"
      }
    }
  ]
}
```

`ripr baseline create` writes this ledger shape from existing gate-decision
JSON. It includes `advisory`, `acknowledged`, and `blocking` decisions; skips
`suppressed`, configured-off, `not_applicable`, and malformed decisions; writes
`reviewed = false` with an initial adoption review reason; refuses to overwrite
without `--force`; and supports `--dry-run` for review without writing. The
baseline is a historical-debt ledger and does not suppress or silence current
findings by itself.

## Identity Matching

The report compares baseline records to current gate decisions using stable
identity fields in this order:

1. `seam_id` when present in both records;
2. `source_id` when present in both records;
3. gate decision `id` when present in both records;
4. `dedupe_key` when present in both records;
5. stable fallback made from normalized repo-relative path, line, and static
   class.

The fallback must be deterministic and documented in the JSON. It is less
stable than `seam_id` and should produce a warning when it is the selected match
method.

If more than one current record matches one baseline identity, the report must
mark the baseline entry `stale_baseline_entry` or `invalid_baseline_entry`
instead of selecting a current record arbitrarily.

## Delta Buckets

Every parsed baseline or current decision should land in exactly one primary
bucket:

- `still_present` - the baseline identity is present in current evidence.
- `resolved` - the baseline identity is absent from current evidence and no
  replacement identity was matched.
- `new_policy_eligible` - a current policy-eligible decision is not in the
  baseline.
- `acknowledged` - a current decision is visible and acknowledged by label or
  policy.
- `suppressed` - a current decision is suppressed or configured off and remains
  visible in the report.
- `stale_baseline_entry` - a baseline record can be parsed but no longer joins
  cleanly because its identity is ambiguous, obsolete, or incompatible.
- `invalid_baseline_entry` - a baseline record is malformed or missing required
  identity fields.
- `missing_current_input` - the report cannot classify current movement because
  a required current artifact is absent or unreadable.

Secondary flags may record `matched_by`, `repair`, `warnings`, and optional
static movement when receipts are supplied.

## JSON Shape

The JSON report uses schema version `0.1`:

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
        "seam_id": "67fc764ba37d77bd",
        "source_id": "ripr-review-67fc764ba37d77bd",
        "id": "ripr-gate-67fc764ba37d77bd",
        "dedupe_key": null,
        "fallback": "src/pricing.rs:88:weakly_gripped",
        "matched_by": "seam_id"
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
      }
    }
  ],
  "warnings": [],
  "limits_note": "Advisory baseline debt movement over static RIPR gate evidence; pass/fail remains owned by ripr gate evaluate."
}
```

## Markdown Shape

The Markdown sibling should fit in generated CI job summaries:

```text
# RIPR Baseline Debt Delta

Status: advisory
Baseline: .ripr/gate-baseline.json

| Bucket | Count |
| --- | ---: |
| Still present | 40 |
| Resolved | 7 |
| New policy-eligible | 2 |
| Acknowledged | 1 |
| Suppressed | 0 |
| Stale baseline entry | 1 |
| Invalid baseline entry | 0 |
| Missing current input | 0 |

Top new policy-eligible gaps:
- src/pricing.rs:88 weakly_gripped
  Missing: amount == discount_threshold
  Action: add a focused boundary test or acknowledge visibly.
```

The Markdown must distinguish:

- baseline debt from suppression;
- resolved baseline entries from new current debt;
- acknowledged current decisions from hidden success;
- missing or invalid inputs from passing policy.

## Command Sequence

Campaign 17 introduces these command surfaces in order:

```text
ripr baseline create \
  --from target/ripr/reports/gate-decision.json \
  --out .ripr/gate-baseline.json

ripr baseline diff \
  --baseline .ripr/gate-baseline.json \
  --current target/ripr/reports/gate-decision.json \
  --out target/ripr/reports/baseline-debt-delta.json \
  --out-md target/ripr/reports/baseline-debt-delta.md

ripr baseline update \
  --baseline .ripr/gate-baseline.json \
  --current target/ripr/reports/gate-decision.json \
  --remove-resolved \
  --out .ripr/gate-baseline.json
```

`baseline update --remove-resolved` is shrink-only. It removes reviewed baseline
identities that are absent from current gate-decision evidence, preserves
malformed or ambiguous entries for manual review, and never adopts new current
debt. Adopting new debt must require a later explicit flag and reviewed reason
if it is implemented. Generated CI must not rewrite baselines.

## Required Evidence

The report must include:

- explicit baseline and current gate-decision input paths;
- baseline entry counts for parsed, valid, stale, and invalid records;
- current decision counts for every delta bucket;
- one primary bucket for every parsed baseline or current decision record;
- identity fields used for comparison: `seam_id`, `source_id`, `id`,
  `dedupe_key`, and deterministic fallback;
- `matched_by` for every joined baseline/current pair;
- repair-oriented warnings for fallback matches, ambiguous matches, invalid
  baseline records, missing required inputs, and unsupported schema versions;
- focused repair context when supplied by existing gate, PR guidance, agent, or
  outcome artifacts;
- an advisory limits note that names the gate evaluator as the pass/fail owner.

## Acceptance Examples

Given a baseline with one advisory weakly gripped seam and current evidence with
the same `seam_id`, the report counts one `still_present` item and records
`matched_by = "seam_id"`.

Given a baseline with one reviewed seam that is absent from current evidence,
the report counts one `resolved` item and does not add a new baseline record.

Given current evidence with one policy-eligible gate decision that is absent
from the baseline, the report counts one `new_policy_eligible` item and carries
the missing discriminator and repair command when available.

Given a current decision acknowledged by `ripr-waive`, the report counts it in
`acknowledged` and keeps it visible rather than treating it as silent success.

Given a malformed baseline entry with no stable identity field, the report
counts it in `invalid_baseline_entry` and emits repair guidance.

Given a missing current gate-decision input, the report produces
`missing_current_input` instead of treating the baseline as resolved.

## Test Mapping

The implementation adds tests for:

- CLI parsing for `ripr baseline diff`;
- CLI parsing for `ripr baseline update --remove-resolved`;
- baseline JSON parsing and invalid-entry reporting;
- identity matching by `seam_id`, `source_id`, `id`, `dedupe_key`, and
  fallback;
- ambiguous fallback matches becoming stale or invalid rather than arbitrary;
- JSON and Markdown report shape;
- shrink-only baseline update preserving malformed or ambiguous entries and
  ignoring new current debt;
- fixture cases for still-present, resolved, new policy-eligible,
  acknowledged, suppressed, stale, invalid, and missing-current-input buckets.

## Implementation Mapping

The expected implementation surface is:

- `crates/ripr/src/cli/command.rs`, `commands.rs`, `execute.rs`, and `help.rs`
  for the `ripr baseline` command group;
- `crates/ripr/src/output/baseline_delta.rs` for JSON/Markdown rendering;
- `crates/ripr/src/output/baseline_update.rs` for shrink-only baseline
  refreshes;
- `docs/OUTPUT_SCHEMA.md` for the public contract;
- fixtures under `fixtures/boundary_gap/expected/baseline-debt-delta/`;
- generated CI wiring only after the command and fixture contract are pinned.

## Metrics

The report should feed these adoption metrics:

- `baseline_debt_delta_entries`;
- `baseline_debt_delta_still_present`;
- `baseline_debt_delta_resolved`;
- `baseline_debt_delta_new_policy_eligible`;
- `baseline_debt_delta_acknowledged`;
- `baseline_debt_delta_suppressed`;
- `baseline_debt_delta_stale_entries`;
- `baseline_debt_delta_invalid_entries`;
- `baseline_debt_delta_missing_current_input`.

## Non-Goals

- No automatic adoption of new current debt.
- No analyzer behavior changes.
- No gate policy semantics changes.
- No generated workflow behavior changes.
- No LSP or editor changes.
- No default CI blocking.
- No automatic baseline rewrites.
- No generated tests or source edits.
- No mutation execution.
- No opaque quality score.
- No runtime adequacy claim from static evidence.

## Validation

The implementation should be pinned by:

- output contract tests for JSON and Markdown shape;
- baseline identity matching tests for `seam_id`, `source_id`, `id`,
  `dedupe_key`, and fallback;
- fixture cases for still-present, resolved, new policy-eligible,
  acknowledged, suppressed, stale, invalid, and missing-current-input buckets;
- `cargo xtask check-output-contracts`;
- `cargo xtask check-static-language`;
- `cargo xtask check-traceability`;
- `cargo xtask check-capabilities`;
- `cargo xtask check-pr`.
