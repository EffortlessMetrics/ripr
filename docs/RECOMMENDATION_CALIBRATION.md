# Recommendation Calibration

Recommendation calibration measures whether RIPR's PR-time test guidance was
worth reviewer attention. It is the trust layer between advisory visibility and
any later gate policy.

The calibration question is:

```text
Did this PR guidance point to a useful focused test request, place it on the
right review surface, avoid noise when suppression or summary-only fallback was
appropriate, and correlate with better static evidence after one focused test?
```

Calibration does not run analysis by itself. It joins existing artifacts and
reports counts and buckets. It does not edit source files, generate tests, run
mutation testing, post review comments, send telemetry, call external services,
or make CI blocking.

## Run The Report

The default boundary-gap calibration corpus can be checked with:

```bash
cargo xtask recommendation-calibration
```

The command writes:

```text
target/ripr/reports/recommendation-calibration.json
target/ripr/reports/recommendation-calibration.md
```

For an explicit run, pass the PR guidance, expectations, optional outcome
receipts, and optional static movement inputs:

```bash
cargo xtask recommendation-calibration \
  --root . \
  --pr-guidance target/ripr/review/comments.json \
  --calibration-expectations fixtures/boundary_gap/expected/recommendation-calibration/expectations.json \
  --outcome-receipts fixtures/boundary_gap/expected/recommendation-calibration/outcome-receipts \
  --targeted-test-outcome fixtures/boundary_gap/calibration/targeted-test-outcome.json \
  --out target/ripr/reports/recommendation-calibration.json
```

The Markdown report is written next to the JSON output with an `.md` extension.

## Inputs

`recommendation-calibration` reads existing artifacts:

| Input | Purpose |
| --- | --- |
| PR guidance JSON | The `ripr review-comments` recommendations being calibrated. |
| Calibration expectations | PR-shaped fixture expectations for outcome, placement, target, suppression, and movement. |
| Outcome receipts | Optional local feedback from fixtures, reviewers, agents, or CI artifacts. |
| Targeted-test outcome | Optional before/after static movement after one focused test. |
| Agent receipt | Optional selected-seam static movement and receipt context. |

Missing optional inputs do not imply success or failure. The report uses
`unknown`, `not_applicable`, or incomplete warnings instead of inventing
feedback.

## Metrics

The report intentionally uses counts and buckets, not an opaque score.

| Metric | Meaning |
| --- | --- |
| `recommendations_evaluated` | Visible recommendations plus suppressed candidates included in the calibration corpus. |
| `top_recommendation_outcome` | Outcome label for the first visible recommendation. |
| `false_annotations` | Visible recommendations labeled `noisy`, `wrong_line`, `already_covered`, or `wrong_target`. |
| `summary_only_correct` | Recommendations that correctly avoided an unsafe changed-line annotation. |
| `suppressed_correctly` | Hidden candidates that match an expected suppression, cap, configured-off severity, generated/migration exclusion, or nearby-test change. |
| `target_file_correct` | Visible recommendations whose suggested test target matches the expected target when one applies. |
| `static_improved` | Recommendations or receipts associated with improved static grip after a focused test. |
| `static_unchanged` | Recommendations associated with unchanged static grip. This is evidence to inspect, not proof the test was bad. |
| `static_regressed` | Recommendations associated with worse static grip and requiring review before policy use. |
| `unknown` | Recommendations without enough local evidence to classify. |

Use `top_recommendation_outcome` to judge the first ranked recommendation. Use
false annotation, summary-only, suppression, and target-file counts to decide
where the recommendation placement rules need hardening.

## Outcome Receipts

Review guidance outcome receipts are optional local JSON files. They record how
one recommendation performed after review without telemetry or external
services.

Pinned receipt labels are:

| Label | Meaning |
| --- | --- |
| `useful` | The recommendation was worth acting on. |
| `noisy` | The recommendation added review noise. |
| `wrong_line` | The advice was plausible but attached to the wrong changed line. |
| `already_covered` | The PR already included the focused test or equivalent evidence. |
| `wrong_target` | The suggested test target was not the expected target. |
| `summary_only_correct` | The recommendation correctly stayed out of inline comments. |
| `suppressed_correctly` | A hidden candidate was intentionally suppressed or capped. |

Missing receipts leave the recommendation outcome as `unknown` unless fixture
expectations or static movement provide bounded evidence.

## Placement Quality

Placement quality answers whether the review surface was appropriate:

| Quality | Meaning |
| --- | --- |
| `correct` | The recommendation landed on the expected changed seam line or safe fallback. |
| `wrong_line` | The recommendation was line-placeable but attached to the wrong changed line. |
| `summary_only_expected` | Summary-only was the correct surface because no safe changed-line target existed. |
| `suppressed_correctly` | The recommendation should not have been visible. |
| `unknown` | The corpus or receipt did not provide enough evidence. |

Wrong line placement is high-risk because a bad annotation line can reduce
reviewer trust faster than a missing recommendation.

## Suppression Correctness

Suppression is correct when RIPR hides or caps a recommendation for a bounded
reason:

- `cap_reached`
- `suppression`
- `severity_off`
- `nearby_test_changed`
- `generated_or_migration`
- `unknown`

A suppressed candidate is useful evidence only when the reason matches the
fixture expectation or local outcome receipt. Suppression should reduce noise;
it should not hide a PR-local test request that a reviewer would expect to see.

## Static Movement

Static movement compares before/after RIPR evidence after a focused test:

| State | How to read it |
| --- | --- |
| `improved` | Static grip improved. Keep the receipt with the PR evidence. |
| `unchanged` | Static grip did not move. Inspect whether the test target, discriminator, or assertion was too broad. |
| `regressed` | Static grip got worse. Revisit the change before using it as policy evidence. |
| `resolved` | The seam disappeared. Confirm that disappearance was intentional. |
| `new_gap` | A new gap appeared. Generate fresh PR guidance or an agent packet. |
| `missing_after_snapshot` | The after snapshot is missing; run the after command before interpreting movement. |
| `unknown` | No local artifact supplied movement evidence. |

`unchanged` does not prove the test is bad. It means the static evidence did
not improve enough for RIPR to observe it. Runtime mutation evidence, when
available, is imported through calibration artifacts; RIPR does not run mutation
testing in this workflow.

## Reviewer Use

Use the report to decide what to improve next:

1. Check the top recommendation outcome.
2. Inspect false annotations before changing policy.
3. Treat summary-only correctness as a positive result when line placement was
   unsafe.
4. Check target-file correctness before sending LLMs to write tests.
5. Compare static movement with the receipt trail.
6. Feed repeated noisy or wrong-line cases into ranking hardening, not gates.

Recommendation calibration is advisory. A future policy gate may consume these
counts, but only after a repo has enough measured signal to choose explicit
thresholds.

## Fixture Corpus

The checked calibration corpus lives under:

```text
fixtures/boundary_gap/expected/recommendation-calibration/
```

Key files:

| Artifact | Purpose |
| --- | --- |
| `expectations.json` | PR-shaped expectations for useful, noisy, wrong-line, already-covered, summary-only, suppression, generated/migration, macro-heavy, trait/generic, and async/error-boundary cases. |
| `outcome-receipts/` | Optional local outcome receipt examples. |
| `recommendation-calibration.json` | Checked JSON report output. |
| `recommendation-calibration.md` | Checked Markdown report output. |

See [Calibration corpus index](../fixtures/CALIBRATION_CORPUS.md) for the
broader fixture catalog.

## Limits

Recommendation calibration remains static, local, and advisory:

- It does not establish runtime test adequacy.
- It does not run mutation testing.
- It does not generate or edit tests.
- It does not post comments.
- It does not block CI by default.
- It does not send feedback to an external service.
- It does not create an opaque score.

Use it to measure recommendation quality, tune ranking, and decide whether a
future policy lane has enough evidence to define optional gates.
