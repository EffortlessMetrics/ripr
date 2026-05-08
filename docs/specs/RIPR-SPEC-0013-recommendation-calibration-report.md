# RIPR-SPEC-0013: Recommendation Calibration Report

Status: proposed

## Problem

RIPR now projects static seam evidence into editor actions, agent packets,
review summaries, generated CI summaries, and PR guidance annotations. That
answers the visibility question:

```text
Which changed seam looks weakly gripped, and what focused test would help?
```

It does not yet answer the product-quality question:

```text
Was that recommendation useful enough to show to a reviewer?
```

Recommendation calibration measures the quality of RIPR's own PR-time
guidance. It stays local to repository artifacts. It does not send telemetry,
call an LLM provider, generate tests, edit source files, run mutation testing,
or turn advisory findings into a CI gate.

## Product Contract

The recommendation calibration report is advisory evidence over existing RIPR
recommendations. It measures whether a recommendation was placed correctly,
suppressed correctly, aimed at a useful test target, and correlated with later
static evidence movement.

The contract is:

- RIPR keeps producing deterministic static guidance first.
- Calibration reads existing guidance, fixture expectations, optional outcome
  receipts, and before/after static movement.
- The report measures recommendation quality without changing recommendation
  ranking, CI blocking, LSP behavior, or public schemas outside this report.
- Calibration labels are review-quality labels, not runtime mutation outcomes.
- Missing feedback remains explicit as `unknown`, not silently interpreted as
  useful or noisy.

## Behavior

The repo-local report producer is:

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

The command writes:

```text
target/ripr/reports/recommendation-calibration.json
target/ripr/reports/recommendation-calibration.md
```

The producer must not rerun analysis unless a later implementation explicitly
adds a safe refresh mode. The first implementation should read existing
artifacts and report missing inputs as incomplete evidence.

## Inputs

The report may read:

- PR guidance JSON from `ripr review-comments`;
- PR guidance Markdown for human reference when present;
- pilot summary JSON;
- agent packet, agent brief, workflow manifest, verify, receipt, or review
  summary artifacts;
- targeted-test outcome JSON;
- repo exposure before/after snapshots;
- configured severity and suppression state when available;
- fixture expectation metadata from the recommendation calibration corpus;
- optional review guidance outcome receipt artifacts;
- file timestamps or CI-provided timestamps for latency fields.

Missing optional inputs must produce `unknown` or `not_observed` fields rather
than inventing feedback. Missing required inputs should produce an incomplete
report with repair guidance, not a successful quality score.

## Outcome Receipts

Review guidance outcome receipts are optional local feedback artifacts. They
record the observed review-quality outcome for one PR guidance item without
depending on telemetry, an external service, model calls, generated tests, source
edits, runtime mutation execution, or CI blocking.

Receipts are inputs to calibration, not a new recommendation source. A missing
receipt leaves the matching recommendation as `unknown` unless fixture
expectations, static movement, or another local artifact supplies a bounded
outcome.

The receipt shape is documented in `docs/OUTPUT_SCHEMA.md` and pinned by the
boundary-gap calibration corpus. It must identify:

- the source guidance artifact, collection, item id, `seam_id`, and dedupe key
  when available;
- one review-quality label: `useful`, `noisy`, `wrong_line`,
  `already_covered`, `wrong_target`, `summary_only_correct`,
  `suppressed_correctly`, or `unknown`;
- placement quality, suggested-test target quality, suppression reason and
  quality, static movement state, and optional latency fields;
- explicit false limits for telemetry, external services, source edits,
  generated tests, runtime mutation execution, and CI blocking.

## Calibration Outcomes

Recommendation calibration uses these review-quality labels:

- `useful` - the recommendation was actionable and aligned with the expected
  seam, test intent, and review surface.
- `noisy` - the recommendation was visible but not useful for the intended
  review context.
- `wrong_line` - line-level placement pointed to the wrong changed line.
- `already_covered` - the PR already changed a nearby focused test or the
  fixture expectation says the seam should not have been recommended.
- `wrong_target` - the recommendation's suggested test file or related test was
  not the expected target.
- `summary_only_correct` - RIPR avoided an unsafe line placement and kept the
  item in summary-only guidance.
- `suppressed_correctly` - suppression, configured-off severity, nearby-test
  change, generated/migration classification, or cap behavior hid the item as
  expected.
- `unknown` - no fixture expectation, receipt, or reviewer outcome exists.

These labels measure recommendation usefulness. They do not mean runtime
mutation results, coverage, or merge readiness.

Calibration outcomes and placement quality are separate axes. Placement quality
describes whether a PR guidance item was safely mapped to a changed line or kept
out of the inline surface. Calibration outcomes describe whether that rendered
recommendation was useful in the expected review context. For example, a
recommendation can have `placement.quality = "summary_only_expected"` and
`calibration.outcome = "summary_only_correct"` when summary-only guidance was
the expected review surface.

## Metrics

The report should expose counts for:

- top recommendation usefulness;
- false annotations;
- summary-only correctness;
- suppression correctness;
- recommended test target correctness;
- before/after static movement;
- unchanged static movement;
- regressed static movement;
- unknown outcome count;
- review-comment or annotation latency when timestamps are available.

Definitions:

- A top recommendation is the first recommendation from PR guidance after
  configured severity, suppression, and cap rules apply.
- A false annotation is a line-level recommendation labeled `wrong_line`,
  `already_covered`, `wrong_target`, or `noisy` by the calibration corpus or an
  outcome receipt.
- Summary-only correctness is the count of recommendations that avoided an
  unsafe changed-line placement when the fixture expectation says summary-only
  was the right surface.
- Suppression correctness is the count of hidden recommendations that match an
  expected suppression, configured-off severity, nearby-test change,
  generated/migration exclusion, or cap condition.
- Target-file correctness compares `suggested_test.recommended_file` and
  related-test fields with fixture or outcome expectations.
- Static movement is imported from existing targeted-test outcome or agent
  receipt artifacts and remains static evidence only.

## JSON Shape

The JSON report uses schema version `0.1`:

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

## Field Contract

- `schema_version` - currently `"0.1"`.
- `tool` - always `"ripr"`.
- `status` - `advisory` when the report has enough inputs to evaluate at least
  one recommendation and `incomplete` when required inputs are missing.
  Malformed required inputs return an actionable command error instead of a
  successful report.
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
  `summary_only_expected` is a placement-quality value only: it means the item
  correctly stayed out of an unsafe line-level placement. The corresponding
  review-quality outcome remains `summary_only_correct`.
- `recommendations[].suggested_test.target_quality` - `correct`,
  `wrong_target`, `not_applicable`, or `unknown`.
- `recommendations[].calibration.outcome` - one of the calibration outcome
  labels defined above.
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
- `limits_note` - static/advisory boundary text for summaries and generated CI.

## Markdown Shape

The Markdown report should be concise enough for generated CI summaries:

```text
# Recommendation Calibration

Status: advisory

Top recommendation:
  src/pricing.rs:88 weakly_gripped useful

Why:
  expected equality-boundary recommendation on the changed seam

Placement:
  exact changed seam line, correct

Suggested test target:
  tests/pricing.rs, correct

Static movement:
  weakly_gripped -> strongly_gripped, static evidence improved

Limits:
  Advisory recommendation-quality evidence only.
```

## Advisory CI Projection

Generated CI may upload the JSON and Markdown reports when they exist. It may
append a compact recommendation-quality section to `$GITHUB_STEP_SUMMARY`.

The generated workflow must not fail because of this report by default. Future
policy gates may consume recommendation calibration, but that belongs to a
separate explicit gate-policy campaign.

## Required Evidence

The implementation campaign adds:

- fixture expectations for useful, noisy, wrong-line, already-covered,
  summary-only-correct, suppressed-correctly, generated/migration,
  macro-heavy, trait/generic, and async/error-boundary cases;
- an optional review guidance outcome receipt shape with pinned examples for
  useful, noisy, wrong-line, already-covered, wrong-target,
  summary-only-correct, and suppressed-correctly labels;
- a report producer that joins PR guidance, expectations, suppression state,
  target placement, latency, and before/after static movement;
- JSON and Markdown output tests;
- generated CI artifact upload and summary tests only in a follow-up after the
  pure report is fixture-backed;
- docs explaining how to read useful, noisy, wrong-line, already-covered,
  wrong-target, summary-only-correct, suppressed-correctly, unchanged,
  improved, regressed, resolved, and unknown outcomes.

## Non-Goals

Recommendation calibration must not:

- call LLM APIs or choose LLM providers;
- generate tests;
- edit source files;
- run mutation testing;
- send telemetry or depend on external services;
- change PR guidance ranking without fixture-backed follow-up work;
- post inline comments;
- make CI blocking by default;
- implement calibrated gates or acknowledgement labels;
- claim runtime confirmation or merge readiness;
- split the public crate surface.

## Acceptance Examples

- A fixture where the top PR guidance item matches the expected changed seam,
  missing discriminator, placement, and test target is counted as `useful`.
- A line-level annotation on a misleading changed line is counted as
  `wrong_line` and as a false annotation.
- A seam with a nearby changed focused test is counted as `already_covered` or
  `suppressed_correctly`, depending on whether it was shown or hidden.
- A recommendation with no safe changed-line placement is counted as
  `summary_only_correct` when the expectation says summary-only was the right
  surface.
- A recommendation whose expected test file differs from
  `suggested_test.recommended_file` is counted as `wrong_target`.
- A receipt showing `weakly_gripped` to `strongly_gripped` movement is counted
  as `static_improved`; this remains static evidence only.
- Missing outcome receipts leave the recommendation outcome as `unknown`
  instead of assuming usefulness.

## Test Mapping

The initial implementation test mapping covers:

- report input parsing and missing-input warnings;
- JSON and Markdown rendering;
- top recommendation usefulness counts;
- false annotation tracking;
- summary-only correctness;
- suppression and cap correctness;
- recommended test target correctness;
- latency derivation with missing timestamp fallback;
- static movement import from agent receipt or targeted-test outcome;
- generated CI summary/artifact behavior remains follow-up work after the
  report producer exists.

## Implementation Mapping

The implementation should map this spec to:

- a repo-local xtask report command for the initial calibration report;
- an xtask report module that joins existing PR guidance, expectation, receipt,
  and static-movement artifacts;
- JSON and Markdown rendering for recommendation calibration;
- fixture expectations under the boundary-gap corpus;
- generated CI upload and summary wiring only after fixture-backed report
  output exists.

## Metrics

- `recommendation_calibration_evaluated`
- `recommendation_calibration_useful`
- `recommendation_calibration_noisy`
- `recommendation_calibration_false_annotations`
- `recommendation_calibration_summary_only_correct`
- `recommendation_calibration_suppressed_correctly`
- `recommendation_calibration_target_file_correct`
- `recommendation_calibration_static_improved`
- `recommendation_calibration_static_unchanged`
- `recommendation_calibration_static_regressed`
- `recommendation_calibration_unknown`
