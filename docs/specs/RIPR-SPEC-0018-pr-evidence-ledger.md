# RIPR-SPEC-0018: PR Evidence Ledger

Status: proposed

## Problem

Campaign 18 made RIPR Zero status visible from the current baseline, current
debt delta, gate decision, recommendation evidence, and repair routes. That
answers the current-state question:

```text
Where is this repository relative to RIPR 0 right now?
```

The remaining Lane 4 adoption gap is history. Maintainers need to know whether
pull requests are shrinking reviewed baseline debt, introducing new
policy-eligible gaps, accumulating waivers, preserving suppressions as durable
policy exceptions, routing repair packets, and improving behavioral grip even
when line coverage stays flat.

Without a PR evidence ledger, RIPR can show a useful PR summary, but teams
cannot easily answer:

```text
Did this PR make behavioral test grip better or worse?
Which waivers are aging?
Which baseline entries were resolved by focused tests?
Which repair receipts prove movement?
Did coverage move, or did only behavioral grip improve?
```

## Product Contract

The PR evidence ledger is an advisory record over existing RIPR artifacts. It
does not create evidence, rerun the analyzer, post comments, mutate baselines,
or make policy decisions. It records the outcome of one PR and, when a history
input is supplied, summarizes movement over prior PR records.

The ledger must:

- preserve per-PR movement for new policy-eligible gaps, resolved baseline
  debt, acknowledgements, suppressions, blocking candidates, and gate mode;
- keep waivers visible as PR-time acknowledgement records, not hidden success;
- keep suppressions visible as durable policy exceptions, not waivers or
  baseline debt;
- link repair routes and receipts when existing artifacts provide them;
- optionally show coverage/grip frontier signals as separate execution and
  behavioral-grip axes;
- remain advisory by default;
- leave pass/fail authority with `ripr gate evaluate`;
- avoid analyzer identity rewrites, recommendation ranking changes, gate policy
  semantic changes, generated tests, LSP behavior changes, default CI blocking,
  or runtime mutation claims from static evidence.

## RIPR 0 Boundary

RIPR 0 means:

```text
No visible unresolved behavioral test-grip gaps remain under configured scope
and policy.
```

The PR evidence ledger may report movement toward or away from RIPR 0. It must
not redefine RIPR 0 as:

- perfect tests;
- 100 percent coverage;
- no suppressions;
- no unknowns;
- no static limitations;
- runtime mutation adequacy.

Runtime mutation vocabulary such as `killed` or `survived` may appear only when
an imported runtime calibration artifact explicitly supplies that vocabulary.

## Behavior

The PR evidence ledger reads existing artifacts and writes one advisory PR
record. It should:

- copy movement counts from baseline debt delta, RIPR Zero status, and gate
  decision artifacts instead of recomputing analyzer semantics;
- preserve gate mode and gate decision as evidence while leaving pass/fail
  authority with `ripr gate evaluate`;
- preserve waivers, suppressions, baseline records, stale records, invalid
  records, and missing-input records as separate visible categories;
- link repair receipts and top repair routes only when existing artifacts
  provide them;
- emit coverage/grip frontier fields only when coverage input is supplied;
- read prior ledger history only when an explicit history input is supplied;
- write warnings for missing recommended inputs rather than treating them as
  success;
- keep generated CI advisory unless a separate gate decision has already
  blocked.

## Command Surface

The command surface is:

```text
ripr pr-ledger record \
  --pr-number 123 \
  --head <sha> \
  --base <sha> \
  --gate target/ripr/reports/gate-decision.json \
  --baseline-delta target/ripr/reports/baseline-debt-delta.json \
  --zero-status target/ripr/reports/ripr-zero-status.json \
  --pr-guidance target/ripr/review/comments.json \
  --recommendation-calibration target/ripr/reports/recommendation-calibration.json \
  --agent-receipt target/ripr/reports/agent-receipt.json \
  --coverage target/ripr/reports/coverage-summary.json \
  --history .ripr/pr-evidence-ledger.jsonl \
  --out target/ripr/reports/pr-evidence-ledger.json \
  --out-md target/ripr/reports/pr-evidence-ledger.md
```

Required inputs:

- PR identity: number or equivalent local identifier, base revision, and head
  revision;
- at least one current RIPR evidence source: gate decision, baseline debt
  delta, RIPR Zero status, or PR guidance.

Recommended inputs:

- gate decision JSON;
- baseline debt delta JSON;
- RIPR Zero status JSON;
- PR guidance JSON;
- recommendation calibration JSON.

Optional inputs:

- targeted-test outcome JSON;
- agent receipt JSON;
- imported mutation calibration JSON;
- coverage summary JSON;
- previous PR evidence ledger JSON or append-only JSONL history;
- captured PR labels JSON.

Missing recommended or optional inputs must produce explicit `unknown`,
`not_available`, or warning fields. Missing inputs must not be treated as pass,
resolved, suppressed, acknowledged, or hidden success.

## Append-Only History

The primary report is a per-PR record written to
`target/ripr/reports/pr-evidence-ledger.json` and
`target/ripr/reports/pr-evidence-ledger.md`.

When a history path is supplied, it is a JSON Lines ledger with one immutable
record per PR observation:

```text
.ripr/pr-evidence-ledger.jsonl
```

Append-only means:

- a new run may append a new record;
- a new run must not rewrite prior records;
- regenerated CI artifacts may contain a derived summary, but generated CI must
  not auto-commit ledger history;
- correcting a historical record requires an explicit future repair command or
  a reviewed manual edit.

The first implementation may read history without appending. If appending is
implemented, it must require an explicit flag and preserve prior records.

## JSON Shape

The JSON report uses schema version `0.1`:

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
    "source": "ripr_zero_status",
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

- `status` is `advisory` unless required PR identity or all evidence sources
  are missing, in which case the report may use `incomplete`.
- `movement.*` is copied or derived from existing gate, baseline delta, and
  RIPR Zero status artifacts. It must not recompute analyzer semantics.
- `gate.pass_fail_authority` must name `ripr gate evaluate` whenever a gate
  decision is present.
- `waivers[]` records PR-time visible acknowledgements. Waivers do not hide
  findings and do not become suppressions. `waivers[].canonical_gap_id` is
  copied from source artifacts when available and remains `null` otherwise.
- `suppressions[]` records durable policy exceptions. Suppressions are not
  waivers and are not baseline debt. `suppressions[].canonical_gap_id` is
  copied from gate or baseline-delta evidence when available.
- `repair_receipts[]` records supplied outcome or agent receipt evidence.
  `repair_receipts[].static_movement` uses the same object shape as review
  guidance outcome receipts, including `state`, `source`, and `artifact`; it
  must not infer receipt success from a missing artifact.
  `repair_receipts[].canonical_gap_id` is copied from receipt or
  recommendation provenance when supplied.
- `coverage_grip_frontier.status` is `available`, `not_available`, or
  `unsupported`.
- `coverage_grip_frontier.*` must keep coverage movement separate from RIPR
  evidence movement. Coverage movement is execution evidence, not test
  adequacy.
- `top_repair_route` is copied from existing PR guidance, RIPR Zero status,
  gate decision, agent packet, or receipt artifacts. Missing fields are `null`
  plus warnings, not invented. `top_repair_route.canonical_gap_id` is copied
  from the selected source artifact when available.
- `history.*` is present only when a prior ledger history or previous ledger
  summary is supplied.

## Markdown Shape

The Markdown sibling should fit in generated CI job summaries:

```text
# RIPR PR Evidence Ledger

Status: advisory
Gate: baseline-check / acknowledged

| Measure | Count |
| --- | ---: |
| New policy-eligible gaps | 1 |
| Existing baseline gaps still present | 40 |
| Baseline gaps resolved | 3 |
| Acknowledged gaps | 1 |
| Suppressed gaps | 0 |
| Blocking candidates | 0 |
| Visible unresolved gaps | 41 |

Top focused test to add:
- src/pricing.rs:88 weakly_gripped
  Missing: amount == discount_threshold
  Suggested test: Add an equality-boundary assertion.
  Verify: ripr agent verify --root . --before ... --after ... --json

Coverage / grip frontier:
- Coverage delta: 0.0 percent
- RIPR unresolved delta: -3
- Interpretation: behavioral grip improved without line-coverage movement.

Receipts:
- Agent receipt: target/ripr/reports/agent-receipt.json
- Full ledger: target/ripr/reports/pr-evidence-ledger.json

Limits:
The PR evidence ledger is advisory history. Gate decisions remain the pass/fail
authority.
```

The first-screen question is:

```text
Did this PR make behavioral test grip better or worse?
```

## Coverage / Grip Frontier

Coverage is optional execution evidence. Behavioral grip is static RIPR
evidence about whether tests appear to discriminate the changed behavior. The
ledger may show both axes when coverage input exists, but it must never collapse
them into one score.

The frontier may use these quadrants when both inputs are available:

- `covered_with_ripr_gap`;
- `covered_without_ripr_gap`;
- `uncovered_with_ripr_gap`;
- `uncovered_without_ripr_gap`.

Allowed interpretations:

- coverage stayed flat while RIPR evidence improved;
- coverage improved while RIPR evidence stayed unresolved;
- both improved;
- both regressed or became unknown;
- coverage input is not available.

Forbidden interpretations:

- coverage proves test adequacy;
- RIPR replaces coverage;
- static RIPR evidence proves runtime mutation outcomes;
- flat coverage means no behavioral improvement occurred.

## Waiver, Suppression, and Baseline Semantics

The ledger must preserve these distinctions:

- A waiver is a PR-time visible acknowledgement, usually from `ripr-waive`.
  It keeps the finding visible and records that the PR accepted it.
- A suppression is a durable repository policy exception with reason and owner.
  It stays visible as suppressed policy state and is not a PR waiver.
- A baseline entry is reviewed historical debt. It is neither a waiver nor a
  suppression. It can be still present, resolved, stale, invalid, or missing
  current input.

The ledger must not merge these categories to make the PR look cleaner.

## Required Evidence

The report must include:

- PR identity and revision identity;
- explicit input paths and missing-input status;
- new policy-eligible count;
- existing baseline still-present count;
- baseline resolved count;
- acknowledged count;
- suppressed count;
- blocking candidate count;
- visible unresolved count when RIPR Zero status is available;
- gate mode and decision when a gate decision is available;
- waiver records with label, decision identity, age when known, and visibility;
- suppression records with source, owner, reason when known, and visibility;
- repair receipt records when outcome or agent receipt inputs exist;
- top repair route with canonical gap ID when supplied, seam identity, file,
  line, missing discriminator, suggested test, related test, verify command,
  and agent command when those fields are available;
- coverage/grip frontier status and optional movement values;
- history source and movement summary when history is supplied;
- limits text that keeps the report advisory and names the gate evaluator as
  pass/fail authority.

The report must not hide acknowledged, suppressed, stale, invalid, or
missing-input entries.

## Acceptance Examples

Given a PR with no new policy-eligible gaps and three resolved baseline entries,
the ledger records `new_policy_eligible = 0` and `baseline_resolved = 3`.

Given a PR with a `ripr-waive` label, the ledger records a waiver entry and
keeps the finding visible as acknowledged.

Given a suppressed finding, the ledger records it separately from waivers and
baseline debt.

Given source artifacts that supply `canonical_gap_id`, the ledger carries that
identity through waiver records, suppression records, repair receipts, and the
top repair route without changing the existing `seam_id` fields.

Given an agent receipt showing static movement improved after one focused test,
the ledger records a repair receipt and links the receipt path.

Given coverage delta is zero and RIPR unresolved delta is negative, the ledger
may say behavioral grip improved without line-coverage movement.

Given no coverage input, the ledger sets `coverage_grip_frontier.status` to
`not_available` and avoids coverage claims.

Given no history input, the ledger omits waiver age and long-term burn-down
totals or marks them `unknown`.

## Test Mapping

The implementation adds tests for:

- CLI parsing for `ripr pr-ledger record`;
- missing PR identity producing an incomplete report;
- movement counts copied from baseline debt delta and RIPR Zero status;
- gate mode and pass/fail authority copied from gate decision;
- waiver records staying visible and separate from suppressions;
- suppression records staying visible and separate from baselines;
- repair receipt records from agent receipt or targeted-test outcome inputs;
- top repair route copied from existing artifacts without inventing missing
  fields;
- coverage/grip frontier unavailable, available, flat-coverage/improved-grip,
  and coverage-regressed cases;
- CLI parsing and JSON/Markdown rendering for `ripr coverage-grip frontier`;
- history input producing waiver age, baseline burn-down, and trend summaries;
- append-only history refusing to rewrite prior records when append mode is
  implemented;
- JSON and Markdown report shape;
- generated CI fixture behavior proving the ledger stays advisory and gate
  decision remains the pass/fail authority.

## Implementation Mapping

Expected follow-up surfaces:

- `report/pr-evidence-ledger` writes `pr-evidence-ledger.json` and
  `pr-evidence-ledger.md` from existing artifacts.
- `ci/pr-evidence-ledger-summary` runs `ripr pr-ledger record` in generated CI
  on pull requests when PR guidance exists, uploads `pr-evidence-ledger.{json,md}`,
  and appends a PR movement card while keeping advisory defaults and gate
  pass/fail authority.
- `report/coverage-grip-frontier` adds `ripr coverage-grip frontier`, a
  read-only advisory JSON/Markdown report that keeps execution coverage and
  RIPR behavioral grip movement visible as separate axes without making
  coverage blocking.
- `docs/pr-evidence-ledger-workflow` adds
  `docs/PR_EVIDENCE_LEDGER_WORKFLOW.md`, explaining how teams read ledgers,
  waiver aging, baseline burn-down, repair receipts, coverage/grip frontier
  signals, and progress toward RIPR 0.

## Metrics

The report should feed these adoption metrics:

- `pr_evidence_ledger_records`;
- `pr_evidence_ledger_new_policy_eligible`;
- `pr_evidence_ledger_baseline_still_present`;
- `pr_evidence_ledger_baseline_resolved`;
- `pr_evidence_ledger_acknowledged`;
- `pr_evidence_ledger_suppressed`;
- `pr_evidence_ledger_blocking_candidates`;
- `pr_evidence_ledger_repair_receipts`;
- `pr_evidence_ledger_waiver_age_max_days`;
- `pr_evidence_ledger_coverage_grip_frontier_available`;
- `pr_evidence_ledger_flat_coverage_grip_improved`;

## Non-Goals

- No analyzer behavior changes.
- No analyzer identity rewrites.
- No recommendation ranking changes.
- No gate policy semantic changes.
- No CI blocking by default.
- No generated workflow changes in this spec PR.
- No baseline mutation.
- No automatic adoption of new current debt.
- No automatic PR comment posting.
- No source edits.
- No generated tests.
- No LSP or editor changes.
- No mutation execution.
- No runtime adequacy claims from static evidence.
- No opaque quality score.
- No public crate split.

## Validation

The implementation should be pinned by:

- output contract tests for `pr-evidence-ledger.json` and
  `pr-evidence-ledger.md`;
- fixture cases for new debt, resolved baseline debt, waiver, suppression,
  missing inputs, repair receipt, unavailable coverage, flat-coverage improved
  grip, and history trend summaries;
- generated CI fixture tests proving advisory defaults and gate authority are
  preserved;
- `cargo xtask check-output-contracts`;
- `cargo xtask check-static-language`;
- `cargo xtask check-traceability`;
- `cargo xtask check-capabilities`;
- `cargo xtask check-pr`.
