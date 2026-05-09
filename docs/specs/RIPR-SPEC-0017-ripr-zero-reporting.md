# RIPR-SPEC-0017: RIPR Zero Reporting

Status: proposed

## Problem

Campaign 17 made reviewed baselines executable: teams can create a baseline,
compare current gate evidence to that baseline, surface debt deltas in generated
CI, and shrink the ledger without adopting new current debt.

The remaining adoption gap is the repo-level story. A reviewer can see a PR
delta, but maintainers still need one report that answers:

```text
Are we moving toward RIPR 0?
How old is the reviewed baseline?
Which baseline entries are stale or missing metadata?
Which files or areas contain the most visible unresolved grip debt?
Which focused repair packet should a human or agent take next?
```

Without that reporting layer, baseline debt stays mechanically governable but
hard to operate over time.

## RIPR 0 Definition

RIPR 0 means:

```text
No visible unresolved behavioral test-grip gaps remain under configured scope
and policy.
```

It does not mean:

- perfect tests;
- 100 percent coverage;
- no suppressions;
- no static limitations;
- no unknowns;
- runtime mutation adequacy.

RIPR Zero reporting is adoption governance over static RIPR evidence. Runtime
mutation data may be linked only when an explicit imported calibration artifact
is supplied, and any runtime vocabulary must stay scoped to that imported
calibration context.

## Product Contract

The RIPR Zero status report is a read-only advisory report over existing
artifacts:

- reviewed gate baseline ledger;
- baseline debt delta report;
- gate decision report;
- PR guidance and recommendation calibration when present;
- targeted-test outcome or agent receipt when present;
- optional prior RIPR Zero status or history artifacts when present.

The report must:

- make baseline age, owner/reason metadata, stale review windows, and missing
  metadata visible;
- distinguish baseline debt, new policy-eligible debt, acknowledgements,
  suppressions, stale entries, invalid entries, and missing inputs;
- show repo-level movement toward or away from RIPR 0;
- group top debt areas for maintainer planning;
- route top repair candidates to existing focused-test and agent handoff
  artifacts;
- remain advisory by default;
- leave pass/fail authority with `ripr gate evaluate`;
- avoid analyzer identity rewrites, recommendation ranking changes, gate policy
  semantic changes, source edits, generated tests, LSP behavior changes, and
  default CI blocking.

## Behavior

The report reads existing artifacts and writes a repo-level RIPR Zero status
summary. It should:

- classify the current repo state as `achieved`, `not_yet`, or `unknown`;
- preserve baseline debt, new policy-eligible debt, acknowledgements,
  suppressions, stale entries, invalid entries, and missing inputs as separate
  visible facts;
- summarize baseline review metadata health without requiring Campaign 17
  baselines to already contain every Campaign 18 metadata field;
- produce top debt areas as reporting groups only;
- produce top repair routes only from existing guidance, gate, baseline delta,
  agent, receipt, or calibration artifacts;
- surface trends only when a prior status, PR evidence ledger, or history input
  is explicitly supplied;
- emit warnings for missing or stale inputs instead of treating them as passing
  or resolved;
- keep generated CI advisory unless a separate gate decision has already
  blocked.

## Command Surface

The planned command surface is:

```text
ripr zero status \
  --baseline .ripr/gate-baseline.json \
  --delta target/ripr/reports/baseline-debt-delta.json \
  --gate target/ripr/reports/gate-decision.json \
  --pr-guidance target/ripr/review/comments.json \
  --recommendation-calibration target/ripr/reports/recommendation-calibration.json \
  --out target/ripr/reports/ripr-zero-status.json \
  --out-md target/ripr/reports/ripr-zero-status.md
```

Required inputs:

- baseline debt delta JSON.

Recommended inputs:

- reviewed gate baseline ledger;
- gate decision JSON.

Optional inputs:

- PR guidance JSON;
- recommendation calibration JSON;
- targeted-test outcome JSON;
- agent receipt JSON;
- imported mutation calibration JSON;
- prior RIPR Zero status JSON;
- local trend or PR evidence ledger when that exists.

Missing recommended or optional inputs must produce explicit `unknown`,
`not_available`, or warning fields. Missing inputs must not be treated as
passing, resolved, or hidden success.

## Baseline Metadata

RIPR Zero reporting expects Campaign 18 baseline metadata to be additive and
backward-compatible with Campaign 17 ledgers. A baseline entry may carry:

```json
{
  "review": {
    "reviewed": true,
    "owner": "test-platform",
    "reason": "initial adoption baseline",
    "created_at": "2026-05-08T00:00:00Z",
    "review_after": "2026-08-08T00:00:00Z",
    "source": "target/ripr/reports/gate-decision.json"
  }
}
```

When metadata is absent, the report must keep the entry visible and mark the
metadata state as `missing`, `partial`, or `unknown`. Missing metadata is a
review signal, not a suppression and not a reason to drop the entry.

Campaign 18 metadata support is intentionally additive: baseline create writes
the full object for new ledgers, baseline diff reports any present metadata on
baseline-derived delta items, and shrink-only update preserves existing entry
objects while removing resolved debt. Older Campaign 17 ledgers with partial or
absent review metadata remain valid inputs.

Baseline review status values:

- `current` - metadata exists and the review window has not expired.
- `stale` - `review_after` is in the past or the configured age threshold is
  exceeded.
- `missing_metadata` - owner, reason, created_at, or review_after is absent.
- `unknown` - the report cannot parse enough metadata to classify the entry.

## JSON Shape

The JSON report uses schema version `0.1`:

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
    "gate_decision": "target/ripr/reports/gate-decision.json",
    "pr_guidance": "target/ripr/review/comments.json",
    "recommendation_calibration": null,
    "previous_status": null
  },
  "ripr_zero": {
    "state": "not_yet",
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
      "seam_id": "67fc764ba37d77bd",
      "path": "src/pricing.rs",
      "line": 88,
      "missing_discriminator": "amount == discount_threshold",
      "suggested_test": "Add an equality-boundary assertion.",
      "related_test": "tests/pricing.rs::applies_discount_above_threshold",
      "verify_command": "ripr agent verify --root . --before target/ripr/pilot/repo-exposure.json --after target/ripr/pilot/after.repo-exposure.json --json",
      "agent_command": "ripr agent start --root . --seam-id 67fc764ba37d77bd --out target/ripr/workflow"
    }
  ],
  "warnings": [
    "5 baseline entries are missing review metadata"
  ],
  "limits_note": "Read-only advisory RIPR Zero status over existing static RIPR artifacts; gate-decision remains the pass/fail authority."
}
```

Field contract:

- `status` is `advisory` unless required inputs are missing or invalid, in
  which case the report may use `incomplete`.
- `ripr_zero.state` is one of `achieved`, `not_yet`, or `unknown`.
- `ripr_zero.visible_unresolved` counts visible unresolved gaps under the
  supplied baseline and gate scope.
- `baseline.metadata.*` counts reviewed metadata health. Missing metadata does
  not suppress or hide the underlying baseline entry.
- `debt_delta.*` mirrors baseline debt movement buckets so PR summaries can
  show old debt, new debt, resolved debt, acknowledgements, suppressions, stale
  entries, invalid entries, and missing inputs without reinterpreting gate
  policy.
- `trend.source` is `previous_status`, `ledger`, or `not_available`.
- `top_debt_areas[]` groups visible unresolved entries by stable repo-relative
  path or configured area name. It is a reporting grouping, not an analyzer
  identity rewrite.
- `repair_routes[]` is a capped list of focused repair candidates copied from
  existing PR guidance, gate decisions, baseline debt delta, agent packets, or
  receipts. The report must not generate tests or call an LLM.
- `warnings[]` records stale baseline metadata, missing inputs, unsupported
  schemas, ambiguous identities, and trend gaps.

## Markdown Shape

The Markdown sibling should fit in generated CI job summaries:

```text
# RIPR Zero Status

Status: advisory
RIPR 0: not yet

| Measure | Count |
| --- | ---: |
| Visible unresolved gaps | 43 |
| Existing baseline gaps still present | 40 |
| Baseline gaps resolved | 7 |
| New policy-eligible gaps | 1 |
| Acknowledged gaps | 1 |
| Suppressed gaps | 0 |
| Stale baseline entries | 4 |
| Missing metadata entries | 5 |

Top repair route:
- src/pricing.rs:88 weakly_gripped
  Missing: amount == discount_threshold
  Suggested test: Add an equality-boundary assertion.
  Verify: ripr agent verify --root . --before ... --after ... --json

Limits:
RIPR 0 is not perfect tests, 100 percent coverage, or runtime mutation
adequacy. Gate decisions remain the pass/fail authority.
```

The Markdown must make the first-screen answer clear:

```text
Did this PR or repo state move behavioral test grip toward RIPR 0?
```

## Trend Semantics

Trend reporting is optional in the first implementation. When a prior status,
PR evidence ledger, or history file is not supplied, trend fields must be
`null` or `not_available` and the Markdown must say that trend evidence is not
available.

When trend input is supplied, the report may show:

- visible unresolved delta;
- resolved baseline delta;
- new policy-eligible delta;
- acknowledged delta;
- suppressed delta;
- stale baseline metadata delta;
- top area movement.

Trend reporting must not infer causality from static evidence alone. A movement
summary can say evidence improved, stayed visible, or became newly visible, but
it must not claim runtime adequacy.

## Repair Routing

Repair routes are bounded handoffs. They should prefer existing artifacts in
this order:

1. PR guidance item with changed-line-safe recommendation;
2. baseline debt delta item with current focused repair context;
3. gate decision item with blocking or acknowledged candidate details;
4. agent seam packet or agent workflow command for the same seam;
5. targeted-test outcome or receipt when present.

Repair routes must include source provenance. If a suggested test, related
test, verify command, or agent command is missing, the field should be `null`
with a warning rather than invented.

## Acceptance Examples

Given a baseline debt delta with zero visible unresolved gaps, no
new policy-eligible gaps, no stale metadata, and no missing inputs, the report
sets `ripr_zero.state = "achieved"`.

Given a baseline debt delta with existing baseline gaps still present, the
report sets `ripr_zero.state = "not_yet"` and counts them as visible unresolved
baseline debt.

Given a current gate decision with `ripr-waive`, the report counts the finding
as `acknowledged` and keeps the repair route visible.

Given a suppression, the report counts it separately as `suppressed` and does
not treat it as baseline debt or a waiver.

Given baseline entries with expired `review_after` dates, the report emits stale
metadata warnings without dropping those entries.

Given missing recommendation calibration, the report sets calibration fields to
`unknown` or `not_available`; it must not upgrade confidence.

## Required Evidence

The report must include:

- explicit input paths and missing-input status;
- RIPR 0 state and visible unresolved count;
- baseline entry count, still-present count, resolved count, age, and metadata
  health counts;
- baseline debt delta buckets for still-present, resolved, new,
  new policy-eligible, acknowledged, suppressed, stale, invalid, and missing
  input records;
- trend source and nullable trend deltas;
- top debt areas with stable repo-relative grouping;
- top repair routes with seam identity, file, line, missing discriminator,
  suggested test, related test, verify command, agent command, and source
  provenance when those fields are available;
- warnings for stale metadata, missing metadata, missing inputs, unsupported
  schemas, ambiguous identities, and unavailable trends;
- limits text that states RIPR 0 is not perfect tests, 100 percent coverage, or
  runtime mutation adequacy.

The report must not hide acknowledged, suppressed, stale, invalid, or
missing-input entries.

## Test Mapping

The implementation adds tests for:

- CLI parsing for `ripr zero status`;
- missing required baseline debt delta input producing an incomplete report;
- Campaign 17 baseline ledgers without Campaign 18 metadata remaining
  compatible and visible;
- metadata classification for current, stale, missing, and unknown entries;
- RIPR 0 achieved, not yet, and unknown state calculation;
- top debt area grouping by repo-relative path or configured area name;
- repair-route selection from PR guidance, baseline debt delta, gate decision,
  and agent artifacts without inventing missing fields;
- trend fields remaining `not_available` when no history input is supplied;
- JSON and Markdown report shape;
- generated CI fixture behavior proving the report is advisory and gate
  decision remains the pass/fail authority.

## Implementation Mapping

Expected follow-up surfaces:

- `baseline/metadata-v2` extends baseline ledger parsing and rendering with
  optional owner, reason, created_at, review_after, and source metadata.
- `report/ripr-zero-status` writes `ripr-zero-status.json` and
  `ripr-zero-status.md` from existing artifacts.
- `ci/ripr-zero-summary` uploads and summarizes the status report in generated
  CI while keeping advisory defaults and gate pass/fail authority.
- `docs/ripr-zero-reporting-workflow` explains how teams read status, stale
  metadata, top repair areas, and progress toward RIPR 0.

## Metrics

The report should feed these adoption metrics:

- `ripr_zero_status_reports`;
- `ripr_zero_visible_unresolved`;
- `ripr_zero_new_policy_eligible`;
- `ripr_zero_baseline_still_present`;
- `ripr_zero_baseline_resolved`;
- `ripr_zero_acknowledged`;
- `ripr_zero_suppressed`;
- `ripr_zero_stale_baseline_entries`;
- `ripr_zero_missing_metadata_entries`;
- `ripr_zero_top_repair_routes`;

## Non-Goals

- No analyzer behavior changes.
- No analyzer identity rewrites.
- No recommendation ranking changes.
- No gate policy semantic changes.
- No CI blocking by default.
- No generated workflow changes in this spec PR.
- No baseline mutation.
- No automatic adoption of new current debt.
- No source edits.
- No generated tests.
- No LSP or editor changes.
- No mutation execution.
- No runtime adequacy claims.
- No opaque quality score.

## Validation

The implementation should be pinned by:

- output contract tests for `ripr-zero-status.json` and
  `ripr-zero-status.md`;
- baseline metadata compatibility tests for Campaign 17 ledgers;
- stale metadata tests for `review_after` and missing owner/reason fields;
- fixture cases for RIPR 0 achieved, not yet, stale metadata, missing inputs,
  acknowledged findings, suppressed findings, and missing calibration;
- generated CI fixture tests proving advisory defaults and gate authority are
  preserved;
- `cargo xtask check-output-contracts`;
- `cargo xtask check-static-language`;
- `cargo xtask check-traceability`;
- `cargo xtask check-capabilities`;
- `cargo xtask check-pr`.
