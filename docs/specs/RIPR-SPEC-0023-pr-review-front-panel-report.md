# RIPR-SPEC-0023: PR Review Front Panel Report

Status: proposed

Owner: ripr maintainers

Lane: 4

Linked proposal:
[RIPR-PROP-0004: PR / CI Review Cockpit](../proposals/RIPR-PROP-0004-pr-ci-review-cockpit.md)

Linked plan:
[Lane 4 implementation plan](../../plans/lane4-pr-ci-review-cockpit/implementation-plan.md)

Authority: behavior contract for the PR review front panel report

Non-authority: PR order, analyzer truth, gate policy, source edits, generated
tests, editor routing, mutation execution, provider calls, and default CI
blocking

## Role

This spec defines observable behavior and acceptance examples for the Lane 4
PR review front panel surface. It does not define PR order, active execution
state, or policy authority. Implementation sequencing lives in
`plans/lane4-pr-ci-review-cockpit/`; configured pass/fail authority remains
with explicit gate-decision artifacts.

## Problem

RIPR now has many PR-time evidence surfaces:

- PR guidance comments and summary-only recommendations;
- first-useful-action reports;
- test-oracle assistant proof reports;
- assistant-loop health reports;
- PR evidence ledgers;
- baseline debt deltas and RIPR Zero status;
- optional gate decisions;
- optional recommendation, mutation, and coverage/grip calibration reports;
- agent or targeted-test receipts.

Those artifacts are useful, but a reviewer should not need to learn the whole
report topology before deciding what this pull request means. The PR review
front panel composes those explicit artifacts into one advisory first-screen
answer:

```text
Did this PR make behavioral test grip better or worse, what should happen next,
and where is the receipt?
```

## Product Contract

The PR review front panel is a read-only projection over explicit RIPR
artifacts. It does not create evidence, rerun hidden analysis, inspect source
to infer missing fields, edit source, generate tests, call a provider, run
mutation testing, change recommendation ranking, change gate policy, change
LSP/editor behavior, publish inline comments, or change default CI blocking.

The report must:

- show one selected top issue or explain why no safe action is available;
- show the current evidence strength, missing discriminator, focused proof
  intent, suggested focused test, related test, handoff command, verify command,
  receipt command/path, and static movement when present;
- distinguish baseline, new policy-eligible, acknowledged, waived, suppressed,
  blocked, config-error, and advisory states without hiding findings;
- surface line-placeable versus summary-only guidance;
- summarize baseline debt movement and PR ledger movement when supplied;
- keep optional gate decisions separate from report authority;
- keep optional coverage/grip and calibration context advisory;
- group artifacts by reviewer use instead of presenting a file pile;
- emit JSON for tools and Markdown for GitHub job summaries and human review;
- preserve static evidence vocabulary and advisory limits.

## Behavior

The canonical behavior is:

```text
explicit RIPR artifact paths
-> deterministic input status checks
-> selected top issue or fallback state
-> PR movement, policy, repair, proof, and artifact groups
-> advisory JSON and Markdown
```

The producer must read only explicit input paths. Missing, stale, malformed, or
incompatible inputs become warnings or fallback states. They must not be
silently treated as acknowledgement, waiver, suppression, improvement, runtime
confirmation, or pass/fail authority.

## Inputs

The public report producer will write:

```text
target/ripr/reports/pr-review-front-panel.json
target/ripr/reports/pr-review-front-panel.md
```

The command accepts explicit input paths:

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

Recommended inputs:

| Input | Role | Required |
| --- | --- | --- |
| PR guidance comments JSON | Changed-line and summary-only recommendation source | Recommended |
| First-useful-action JSON | Current top action or no-action reason | Recommended |
| Test-oracle assistant proof JSON | Joined recommendation, handoff, before/after evidence, receipt, and optional CI context | Recommended |
| Assistant-loop health JSON | Proof completeness, warnings, and repair queue context | Recommended |
| PR evidence ledger JSON | PR movement, waivers, acknowledgements, suppressions, repair receipts, and coverage/grip movement | Recommended |
| Baseline debt delta JSON | Baseline, resolved, new, acknowledged, suppressed, stale, or invalid debt state | Recommended |
| RIPR Zero status JSON | Repo movement toward RIPR 0 and top repair routes | Optional |
| Gate decision JSON | Optional policy state, never front-panel pass/fail authority | Optional |
| Recommendation calibration JSON | Optional recommendation-quality context | Optional |
| Mutation calibration JSON | Optional imported runtime calibration context | Optional |
| Coverage/grip frontier JSON | Optional coverage/grip context, not adequacy evidence | Optional |
| Agent or targeted-test receipt JSON | Optional static before/after movement receipt | Optional |

Missing recommended inputs should be visible as warnings or fallback states.
Missing optional inputs should be `null`, `not_available`, or warnings when the
absence changes how the front panel should be read.

## Required Evidence

The front panel can only compose evidence already present in supplied artifacts.
A useful panel should provide:

- an explicit report root and generated-at timestamp;
- input artifact paths and compatibility status for each supplied artifact;
- a selected top issue or a bounded fallback state;
- selected seam identity, path, line, static classification, and missing
  discriminator when a top issue exists;
- line-placement state from PR guidance when available;
- current evidence strength, focused proof intent, suggested focused test,
  related test, handoff command, verify command, and receipt command/path when
  available;
- baseline, acknowledgement, waiver, suppression, and gate state from supplied
  policy artifacts when available;
- PR debt movement from supplied ledger, baseline delta, or RIPR Zero reports
  when available;
- before/after static movement and receipt state when supplied;
- optional recommendation, mutation, and coverage/grip calibration context when
  supplied;
- warnings for missing, stale, malformed, incompatible, summary-only, or
  optional inputs that change how a reviewer should read the panel;
- advisory limits copied into both JSON and Markdown.

Missing evidence stays visible as `null`, `unknown`, `not_available`, warnings,
or a fallback state. The front panel must not synthesize missing evidence from
the workspace or from source inspection.

## Status Vocabulary

Top-level `status` must be one of:

- `advisory`: report rendered from available inputs and cannot block by itself.
- `pass`: supplied gate context says no current policy action is needed.
- `acknowledged`: supplied gate or ledger context shows a visible
  acknowledgement.
- `blocked`: supplied explicit gate context says the configured gate blocked.
- `config_error`: supplied explicit gate context reports a configuration error.
- `incomplete`: required front-panel inputs are missing, unreadable,
  malformed, or incompatible.

`top_issue_state` must be one of:

- `actionable`: one PR-local issue has a focused next action.
- `summary_only`: the best guidance is visible but not safely line-placeable.
- `baseline_only`: evidence is existing baseline debt, not new PR-local work.
- `already_improved`: supplied movement or receipts show the selected evidence
  already improved or resolved.
- `unchanged_after_attempt`: a supplied receipt shows unchanged static movement
  after a focused-test attempt.
- `no_actionable_seam`: no current seam has enough actionable evidence.
- `missing_required_input`: required report inputs are missing or invalid.
- `stale_input`: supplied report inputs are stale enough to refresh first.

`policy_state` must be one of:

- `none`;
- `new_policy_eligible`;
- `baseline`;
- `acknowledged`;
- `waived`;
- `suppressed`;
- `blocking`;
- `config_error`.

`placement` must be one of:

- `changed_line`;
- `summary_only`;
- `not_available`.

`movement_state` must be one of:

- `improved`;
- `resolved`;
- `unchanged`;
- `regressed`;
- `unknown`;
- `not_available`.

`coverage_grip_state` must be one of:

- `not_available`;
- `flat_coverage_grip_improved`;
- `coverage_and_grip_improved`;
- `coverage_improved_grip_unchanged`;
- `coverage_regressed`;
- `unknown`.

`warning_kind` must be one of:

- `missing_required_input`;
- `missing_optional_input`;
- `stale_input`;
- `malformed_input`;
- `incompatible_schema`;
- `summary_only_guidance`;
- `missing_receipt`;
- `missing_handoff`;
- `missing_gate_decision`;
- `missing_calibration`;
- `static_limit`;
- `config_error`;
- `other`.

`artifact_group` must be one of:

- `start_here`;
- `repair`;
- `evidence`;
- `policy`;
- `calibration`;
- `generated_ci`.

The report must not compute an opaque score. The status, counts, selected issue,
warnings, and artifact groups are the signal.

## JSON Shape

The JSON report uses schema version `0.1`:

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

- `schema_version` is `0.1` until the report shape changes.
- `kind` is always `pr_review_front_panel`.
- `status`, `summary.top_issue_state`, `summary.policy_state`,
  `summary.placement`, `summary.movement_state`, and
  `summary.coverage_grip_state` use the bounded vocabularies in this spec.
- `inputs.*` records explicit input paths. Missing optional paths are `null`;
  missing recommended paths produce warnings or fallback states.
- `summary.*` carries the first-screen counts and states. Counts must be
  derived from supplied artifacts, not hand-authored scores.
- `top_issue.*` is copied from existing RIPR artifacts. The front panel must
  not mint seam identities, rerank recommendations with a model, or infer
  missing source facts from code.
- `top_issue.current_evidence_strength`,
  `top_issue.missing_discriminator`, `top_issue.focused_proof_intent`,
  `top_issue.verify_command`, `top_issue.receipt_command`, and
  `top_issue.static_evidence_boundary` are the first-screen repair vocabulary.
  They are additive and must come from supplied artifacts. For
  `first_useful_action`, current evidence strength must come from the selected
  typed field. For legacy PR guidance, gate, baseline, and assistant-health
  artifacts, the front panel may normalize existing typed class/status fields,
  but must not derive the value from Markdown prose.
- `movement.*` preserves before/after static movement when supplied. It is not
  runtime mutation confirmation.
- `debt_delta.*` carries PR-local movement from baseline, RIPR Zero, gate, or
  ledger inputs when available.
- `policy.authority_artifact` records the gate decision path when supplied.
  Gate decision remains the only configured pass/fail authority.
- `calibration.*` and `coverage_grip.*` are advisory context. They must not
  become adequacy or blocking claims.
- `artifacts[]` groups known artifacts by reviewer use. Paths may be missing,
  but missing artifacts must remain visible as `available = false`.
- `limits` must preserve the static-evidence, no-edit, no-generated-test,
  no-provider-call, no-runtime-mutation-execution, no-inline-comment, and
  advisory-default boundaries.

## Markdown Shape

The Markdown sibling should fit in a generated GitHub job summary:

```md
# RIPR PR Review

Status: advisory

Top issue:
- File: src/pricing.rs:88
- Class: weakly_exposed
- Current evidence strength: Static evidence found related test context, but the current check is weak because the discriminator is missing.
- Missing discriminator: amount == discount_threshold
- Focused proof intent: Add an equality-boundary assertion.
- Suggested focused test: add an equality-boundary assertion
- Related test: tests/pricing.rs::applies_discount_above_threshold

Movement:
- New policy-eligible gaps: 1
- Baseline gaps still present: 42
- Baseline gaps resolved: 3
- Static movement: unknown
- Coverage/grip: not available

Policy:
- Mode: visible-only
- Decision: advisory
- Gate authority: target/ripr/reports/gate-decision.md

Repair:
- Agent handoff: `ripr agent start --root . --seam-id 67fc764ba37d77bd --out target/ripr/workflow`
- Verify: `ripr agent verify --root . --before target/ripr/workflow/before.repo-exposure.json --after target/ripr/workflow/after.repo-exposure.json --json`
- Receipt: target/ripr/reports/agent-receipt.json

Artifacts:
- Start here: target/ripr/reports/pr-review-front-panel.md
- Repair: target/ripr/reports/test-oracle-assistant-proof.md
- Evidence: target/ripr/review/comments.md
- Policy: target/ripr/reports/gate-decision.md

Limits:
- Static RIPR evidence only.
- Does not run mutation testing.
- Does not edit source or generate tests.
- Gate evaluator remains pass/fail authority.
```

Fallback states should put the safe next step before lower-priority detail:

```md
# RIPR PR Review

Status: incomplete

Next: regenerate the missing first-useful-action or PR guidance artifact before
acting on this panel.
```

## Generated CI Projection

Generated GitHub CI may run the front-panel producer only when the configured
input artifacts exist. It may upload `pr-review-front-panel.{json,md}` with the
normal report packet and append the Markdown to the job summary. That projection
is advisory.

Generated CI must not:

- make the front panel pass/fail authority;
- change `RIPR_GATE_MODE` defaults;
- publish inline PR comments;
- skip uploading lower-level artifacts;
- hide gate, waiver, acknowledgement, suppression, or baseline state;
- infer missing inputs by rerunning hidden analysis;
- fail on missing optional inputs unless the explicit gate decision already
  says `config_error`.

`ripr gate evaluate` and its `gate-decision.{json,md}` output remain the only
configured pass/fail authority.

## Acceptance Examples

- Actionable PR-local weak seam with changed-line guidance returns
  `status = "advisory"`, `top_issue_state = "actionable"`, and
  `placement = "changed_line"`.
- Summary-only guidance returns `top_issue_state = "summary_only"` and keeps
  the recommendation visible without creating an inline placement.
- Baseline-only debt returns `top_issue_state = "baseline_only"` and shows
  still-present and resolved baseline counts without presenting old debt as new
  PR-local work.
- A visible waiver returns `status = "acknowledged"` or
  `policy_state = "waived"` and keeps the finding visible.
- A suppression returns `policy_state = "suppressed"` and keeps the durable
  exception visible.
- A configured gate block returns `status = "blocked"` and points to
  `gate-decision.{json,md}` as the pass/fail authority.
- A receipt with improved or resolved static movement returns
  `movement_state = "improved"` or `movement_state = "resolved"`.
- A receipt with unchanged static movement returns
  `movement_state = "unchanged"` and shows a revise-focused-test repair route
  when supplied by assistant proof or first useful action.
- Missing assistant proof, first useful action, or PR guidance returns
  `status = "incomplete"` only when no safe front-panel story can be composed.
- Flat coverage with improved grip returns
  `coverage_grip_state = "flat_coverage_grip_improved"` without claiming
  coverage adequacy.

## Test Mapping

Follow-up tests and fixtures should cover:

- actionable changed-line guidance;
- summary-only guidance;
- no-actionable-seam fallback;
- baseline-only debt;
- resolved baseline debt;
- acknowledged or waived candidate;
- suppressed candidate;
- configured blocked gate;
- gate `config_error`;
- missing proof, missing first action, or missing PR guidance;
- unchanged-after-attempt receipt;
- improved or resolved static movement receipt;
- flat-coverage and improved-grip frontier context;
- malformed, stale, incompatible, and missing optional inputs;
- artifact grouping in JSON and Markdown;
- generated-CI projection that remains advisory.

## Implementation Mapping

Follow-up implementation belongs to Campaign 24:

- `fixtures/pr-review-front-panel-corpus` pins the advisory, actionable,
  summary-only, acknowledged, suppressed, baseline-resolved, blocked,
  missing-proof, no-actionable, and coverage-flat-grip-improved cases.
- `report/pr-review-front-panel` adds the read-only producer that emits
  `pr-review-front-panel.{json,md}` from explicit existing artifact paths.
- `ci/pr-review-front-panel-summary` uploads and summarizes the panel only
  when required inputs exist and keeps gate decision as pass/fail authority.
- `docs/pr-review-front-panel-workflow` explains reviewer, maintainer,
  developer, and coding-agent use.
- `dogfood/pr-review-front-panel-receipts` records repo-local receipts for the
  representative front-panel states.
- `campaign/pr-review-front-panel-closeout` records the final audit,
  validation, and future-lane boundary.

No follow-up surface may change analyzer behavior, recommendation ranking, gate
policy semantics, LSP/editor behavior, source files, generated tests, provider
calls, mutation execution, public crate shape, inline comment defaults, or
default CI blocking as part of this campaign.

## Metrics

The report should make these counts available to later metrics surfaces:

- `pr_review_front_panel_reports`;
- `pr_review_front_panel_actionable`;
- `pr_review_front_panel_summary_only`;
- `pr_review_front_panel_no_actionable`;
- `pr_review_front_panel_acknowledged`;
- `pr_review_front_panel_waived`;
- `pr_review_front_panel_suppressed`;
- `pr_review_front_panel_blocked`;
- `pr_review_front_panel_config_error`;
- `pr_review_front_panel_baseline_still_present`;
- `pr_review_front_panel_baseline_resolved`;
- `pr_review_front_panel_new_policy_eligible`;
- `pr_review_front_panel_missing_required_input`;
- `pr_review_front_panel_flat_coverage_grip_improved`;
- `pr_review_front_panel_repair_routes`;
- `pr_review_front_panel_artifact_links`.

## Non-Goals

- No analyzer behavior changes.
- No recommendation ranking changes.
- No gate policy semantic changes.
- No LSP or editor behavior changes.
- No generated workflow changes in this spec PR.
- No inline comment publishing.
- No default CI blocking.
- No source edits.
- No generated tests.
- No provider or API calls.
- No mutation execution.
- No implicit artifact discovery or hidden analysis reruns.
- No opaque score.
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
