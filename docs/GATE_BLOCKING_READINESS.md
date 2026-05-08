# Gate Blocking Readiness

Calibrated gates are optional policy over existing RIPR evidence. This guide
helps a team decide when to stay advisory, when to require visible
acknowledgement, and when calibrated blocking is mature enough to use.

The default generated workflow stays advisory. Do not enable blocking because a
repo has a large raw RIPR count. Start with visibility, learn the evidence,
checkpoint historical debt, and only then choose a stricter mode for new
policy-eligible gaps.

## Readiness Ladder

| Stage | Mode | Blocks on RIPR evidence? | Use when |
| --- | --- | --- | --- |
| First visibility | unset | no | The repo is adopting RIPR or wants advisory artifacts only. |
| Decision visibility | `visible-only` | no | Reviewers need `gate-decision.{json,md}` and CI summaries without enforcement. |
| Visible acknowledgement | `acknowledgeable` | yes, unless acknowledged | Reviewers can act on policy-eligible gaps and the team wants a PR-time waiver record. |
| New-debt control | `baseline-check` | yes, for new baseline misses | Existing debt has been reviewed into a baseline and new policy-eligible gaps should not slip in silently. |
| Calibrated blocking | `calibrated-gate` | yes, narrowly | Baseline, recommendation calibration, optional imported runtime calibration, and local receipts all support the same narrow candidate class. |

Rollback is simple: unset `RIPR_GATE_MODE`. Advisory PR guidance, SARIF when
enabled, badges, pilot packets, dogfood receipts, and uploaded artifacts keep
running.

## Stay Advisory

Keep `RIPR_GATE_MODE` unset, or use `visible-only`, when any of these are true:

- reviewers are still learning how to read `gate-decision.md`;
- there is no reviewed baseline for existing debt;
- the repo has not dogfooded gate adoption receipts with `cargo xtask dogfood`;
- candidate findings are mostly unknown-confidence or static-limitation cases;
- recommendations do not yet name concrete missing discriminators or test
  targets;
- the team has no documented owner for waiver review or baseline refreshes;
- label capture into `target/ci/labels.json` has not been verified;
- branch protection requirements would change before the team has seen several
  representative PRs.

Advisory does not mean low value. It is the right state while a team is learning
whether the evidence is actionable enough to govern. A useful advisory PR still
uploads:

```text
target/ripr/review/comments.json
target/ripr/review/comments.md
target/ripr/reports/gate-decision.json
target/ripr/reports/gate-decision.md
target/ripr/reports/dogfood.md
target/ripr/reports/dogfood.json
```

## Require Acknowledgement

Use `RIPR_GATE_MODE=acknowledgeable` when the team is ready to make a reviewer
choose between a focused test and a visible PR-time acknowledgement.

Readiness checklist:

- `visible-only` has run on representative PRs without confusing reviewers;
- `ripr-waive` exists as a repository label and its meaning is documented;
- the workflow captures labels into `target/ci/labels.json`;
- reviewers know that `ripr-waive` is not a suppression and not a hidden skip;
- the gate summary keeps acknowledged candidates visible;
- the team has a rule for removing `ripr-waive` when a focused test is added;
- the expected finding class has concrete guidance, such as a missing
  discriminator, candidate value, assertion shape, or related test.

Do not use acknowledgement mode when the normal response will be "apply the
label to every PR." That is a signal to stay advisory, improve evidence, or
create a reviewed baseline.

Expected acknowledged state:

```text
Decision: acknowledged
Mode: acknowledgeable
Blocking: 0
Acknowledged: 1
Label: ripr-waive
```

## Use Baseline-Check For New Debt

Use `RIPR_GATE_MODE=baseline-check` after existing policy-eligible debt has
been reviewed into a checked-in baseline. This mode is the normal adoption step
for a repo with a large initial RIPR score.

Ready baseline-check adoption has:

- a baseline file at the same path configured by `RIPR_GATE_BASELINE`;
- a baseline PR explaining adoption date, source artifacts, configured scope,
  and refresh owner;
- existing baseline entries still visible in gate artifacts;
- a reviewed rule that baseline refreshes remove resolved identities by
  default and do not add new debt without explicit review;
- a path for one-PR exceptions through `ripr-waive`, not through baseline churn;
- `missing-baseline-config` behavior understood as a configuration error.

Expected baseline-check behavior:

| Candidate state | Decision |
| --- | --- |
| Existing baseline identity | visible and non-blocking |
| New policy-eligible identity | blocking unless acknowledged by the chosen policy |
| Missing or invalid baseline | `config_error` |
| Suppressed or configured-off candidate | visible as suppressed or not applicable when present |

Do not enable `baseline-check` from an unreviewed copy of every report entry.
The baseline is a debt ledger, not a dump of everything the analyzer can see.

## Enable Calibrated Blocking

Use `RIPR_GATE_MODE=calibrated-gate` only after the repository has evidence that
the blocked class is narrow, actionable, and locally trusted.

Minimum readiness:

- `baseline-check` behavior is already understood by maintainers;
- the configured baseline distinguishes historical debt from new candidates;
- `cargo xtask dogfood` passes and writes the gate adoption receipts;
- PR guidance produces concrete missing discriminators or assertion guidance;
- recommendation calibration supports the same candidate class, or records why
  the class should remain advisory;
- imported mutation calibration, when supplied, joins unambiguously to the same
  static seam before it raises confidence;
- ambiguous calibration, missing calibration, runtime-only signals, and
  unknown-confidence candidates stay advisory;
- the team has a documented rollback path: unset `RIPR_GATE_MODE`;
- reviewers can see the blocking reason and artifact paths in the first-screen
  CI summary.

Good calibrated-gate candidates are new, policy-eligible, changed-production
gaps with concrete guidance and supporting calibration. Poor candidates are
summary-only placements, broad unknown-confidence groups, static limitations,
configured-off classes, and findings whose only support is an ambiguous runtime
join.

The calibrated gate may produce a non-zero exit, but only after writing the
decision artifacts:

```text
target/ripr/reports/gate-decision.json
target/ripr/reports/gate-decision.md
```

Those artifacts must remain uploaded even when the explicit gate fails.

## Decision Checklist

Before moving to a stricter mode, record answers to these questions in the PR
or rollout issue:

| Question | Ready answer |
| --- | --- |
| What exact mode is being enabled? | `visible-only`, `acknowledgeable`, `baseline-check`, or `calibrated-gate`. |
| What will block? | Only the documented policy-eligible candidate class for that mode. |
| What will not block? | Existing baseline debt, suppressed candidates, configured-off candidates, missing or ambiguous calibration, and visible-only evidence. |
| How does a reviewer resolve it? | Add a focused test, add `ripr-waive` when allowed, refresh a reviewed baseline, or roll back the mode. |
| What artifact records the decision? | `gate-decision.{json,md}` plus PR guidance, labels JSON, baseline, and calibration inputs when present. |
| What is the rollback? | Unset `RIPR_GATE_MODE`; do not edit the generated workflow for routine rollback. |

## What Not To Do

- Do not turn on a blocking mode for a whole raw RIPR score.
- Do not make generated workflows blocking by default.
- Do not treat `ripr-waive` as a suppression.
- Do not add new baseline entries just to make one PR pass.
- Do not treat missing or ambiguous calibration as high confidence.
- Do not run mutation testing from the gate.
- Do not hide acknowledged, suppressed, baseline-known, or summary-only
  decisions from artifacts.
- Do not add branch protection requirements before the emitted check names and
  rollback path are documented.

## Related Guides

- [Calibrated gate policy](CALIBRATED_GATE_POLICY.md) defines the evaluator,
  modes, inputs, decision vocabulary, and static/runtime boundary.
- [CI strategy](CI.md#gate-adoption-examples) has copyable generated-workflow
  adoption examples.
- [Waiver and label workflows](CI.md#waiver-and-label-workflows) documents
  `ripr-waive`.
- [Gate baseline workflow](CI.md#gate-baseline-workflow) documents baseline
  creation, review, and refresh.
- [Dogfooding](DOGFOODING.md#gate-adoption-receipts) documents repo-local gate
  adoption receipts.
