# Calibrated Gate Policy

Calibrated gates are optional policy over existing RIPR evidence. They answer a
review question that PR guidance and recommendation calibration do not answer
by themselves:

```text
Given the current PR evidence, configured mode, labels, suppressions, baseline,
and calibration inputs, should this finding stay advisory, be acknowledged, or
block CI?
```

The default answer is still advisory. Generated workflows do not run the gate
unless `RIPR_GATE_MODE` is explicitly configured, and RIPR never turns a static
recommendation into a merge gate just because a weak seam was found.

## What The Gate Consumes

`ripr gate evaluate` is read-only. It consumes existing artifacts and writes a
decision report:

```bash
ripr gate evaluate \
  --root . \
  --repo-exposure target/ripr/reports/repo-exposure.json \
  --pr-guidance target/ripr/review/comments.json \
  --sarif-policy target/ripr/reports/sarif-policy.json \
  --labels-json target/ci/labels.json \
  --agent-verify target/ripr/workflow/agent-verify.json \
  --agent-receipt target/ripr/reports/agent-receipt.json \
  --recommendation-calibration target/ripr/reports/recommendation-calibration.json \
  --mutation-calibration target/ripr/reports/mutation-calibration.json \
  --baseline target/ripr/reports/gate-baseline.json \
  --mode visible-only \
  --out target/ripr/reports/gate-decision.json \
  --out-md target/ripr/reports/gate-decision.md
```

Only `--pr-guidance`, `--mode`, `--out`, and `--out-md` are required for the
smallest local run. Other inputs refine the decision when available:

| Input | Effect |
| --- | --- |
| PR guidance | Supplies changed-seam recommendations and placement metadata. |
| Repo exposure | Adds static seam context when the report is available. |
| SARIF policy | Adds configured code-scanning policy context when the SARIF policy report exists. |
| Labels JSON | Lets acknowledgement labels such as `ripr-waive` change a blocking candidate into an acknowledged decision. |
| Agent verify and receipt | Connect the policy decision to the focused-test verification loop when present. |
| Recommendation calibration | Records whether this class of recommendation was useful, noisy, correctly placed, suppressed correctly, or unknown. |
| Mutation calibration | Imported confidence evidence only. RIPR does not run mutation testing in the gate. |
| Baseline | Lets baseline-aware modes distinguish existing debt from new policy-eligible gaps. |

Missing optional inputs remain visible as warnings, unknown confidence, or
omitted input fields. Missing required inputs for the selected mode produce
`config_error` with a repair-oriented Markdown report.

## Modes

The gate mode is the policy boundary:

| Mode | Blocks on RIPR evidence? | Use when |
| --- | --- | --- |
| unset | no | Generated workflow default. No gate decision is evaluated. |
| `visible-only` | no | You want a decision report without enforcement. |
| `acknowledgeable` | yes, unless acknowledged | You want policy-eligible findings to require a visible waiver label such as `ripr-waive`. |
| `baseline-check` | yes, for new baseline misses | You have an explicit baseline and want to avoid failing on old known gaps. |
| `calibrated-gate` | yes, narrowly | You have baseline and calibration inputs and want only new, high-confidence, policy-eligible gaps to block. |

Every mode writes the same JSON and Markdown surfaces. Blocking modes return a
non-zero exit only after writing `target/ripr/reports/gate-decision.json` and
`target/ripr/reports/gate-decision.md`.

## Waivers And Suppressions

`ripr-waive` is the default acknowledgement label. It is not a silent skip. In
`acknowledgeable` mode, a matching label produces an `acknowledged` decision:

```text
Decision: acknowledged
Label: ripr-waive
Finding: still visible
Reason: policy-eligible gap acknowledged by ripr-waive
```

Use waivers when the recommendation should stay in the review record but should
not block this PR. The gate report still names the seam, missing discriminator,
suggested test shape, and policy reason when that data exists.

Suppressions are different. A suppressed or configured-off candidate cannot
block, but it should remain visible in decision metadata when it appears in the
inputs. Suppression means the repository policy intentionally hid or downgraded
that candidate; acknowledgement means a visible PR-time candidate was accepted
for this review.

The sample reviewer workflow for applying, auditing, and removing
`ripr-waive` lives in [CI strategy](CI.md#waiver-and-label-workflows). Use that
flow when moving a repository from `visible-only` to `acknowledgeable`; keep
waivers as PR labels and suppressions as durable repository policy.

## CI Behavior

`ripr init --ci github` generates gate wiring but keeps it disabled:

```yaml
env:
  RIPR_UPLOAD_SARIF: "true"
  RIPR_GATE_MODE: ${{ vars.RIPR_GATE_MODE || '' }}
  RIPR_GATE_BASELINE: ${{ vars.RIPR_GATE_BASELINE || '' }}
```

Leave `RIPR_GATE_MODE` unset for the default advisory workflow. To opt in, set
repository variables such as:

```text
RIPR_GATE_MODE=visible-only
RIPR_GATE_BASELINE=.ripr/gate-baseline.json
```

Copyable generated-CI adoption examples live in
[CI strategy](CI.md#gate-adoption-examples). Start there when enabling
`visible-only`, `acknowledgeable`, `baseline-check`, or `calibrated-gate` on an
external repository; do not fork the generated workflow just to change gate
mode.

The baseline create, diff, and shrink-only update workflow lives in
[Baseline ledger workflow](BASELINE_LEDGER_WORKFLOW.md). Use that workflow
before moving from visible decisions to `baseline-check`.

When `RIPR_GATE_MODE` is set, the generated workflow:

1. runs the existing advisory evidence producers first;
2. captures pull-request labels to `target/ci/labels.json`;
3. runs `ripr gate evaluate` after PR guidance exists;
4. runs `ripr baseline diff` when `RIPR_GATE_BASELINE` and
   `gate-decision.json` are present;
5. appends an at-a-glance gate summary plus `gate-decision.md` to the GitHub
   job summary;
6. summarizes `baseline-debt-delta.{json,md}` when present;
7. uploads gate and baseline delta artifacts with the `ripr-reports` artifact
   packet;
8. fails only when the explicit mode produces `blocked` or `config_error`.

The generated workflow currently passes the PR guidance, repo exposure when
available, SARIF policy when available, PR labels, agent verify, agent receipt,
recommendation calibration when available, mutation calibration when available,
and optional baseline. It also keeps SARIF upload steps behind `always()` so
explicit gate failures still preserve code-scanning and artifact evidence when
the files exist.

## Reading The Decision

Top-level statuses are:

| Status | Meaning |
| --- | --- |
| `pass` | A configured non-advisory mode found no visible candidate decisions. |
| `advisory` | Findings are visible but non-blocking. |
| `acknowledged` | At least one policy-eligible candidate was acknowledged by a configured label and none blocked. |
| `blocked` | At least one policy-eligible candidate blocks under the explicit mode. |
| `config_error` | Required input or policy configuration is invalid. |

Candidate decisions are `blocking`, `acknowledged`, `advisory`, `suppressed`,
or `not_applicable`. Review the per-candidate `gate_reason` before changing a
policy setting; it is the shortest explanation of why CI did or did not fail.

## Calibration Boundary

Recommendation calibration measures review quality: useful, noisy, wrong line,
already covered, wrong target, summary-only correct, suppressed correctly, or
unknown. A gate may use those labels as confidence evidence when they match the
same recommendation, seam, target, suppression behavior, or static movement.

Mutation calibration is different. It is imported runtime evidence from a
separate artifact. The gate may use it only when it unambiguously joins to the
same static seam. Ambiguous joins, unmatched runtime records, and runtime-only
signals must not raise gate confidence.

The gate never:

- runs mutation testing;
- claims runtime test strength;
- generates tests;
- edits source files;
- posts comments;
- uploads SARIF;
- hides acknowledged decisions.

Static vocabulary stays static. Gate reports can say a seam is
`weakly_gripped`, `ungripped`, `reachable_unrevealed`, suppressed,
acknowledged, advisory, or blocking under policy. They must not say the test
suite has runtime-backed strength or that RIPR observed runtime mutation
behavior.

## Rollout Path

Use the gate in stages:

1. Run PR guidance and recommendation calibration until the top recommendation
   is useful enough to govern.
2. Set `RIPR_GATE_MODE=visible-only` and inspect `gate-decision.md` for several
   PRs.
3. Try `acknowledgeable` when the team wants visible waiver records.
4. Add an explicit baseline before `baseline-check`.
5. Use `calibrated-gate` only after recommendation calibration and any imported
   mutation calibration consistently support the same narrow candidate class.

Rollback is simple: unset `RIPR_GATE_MODE`. The advisory PR guidance, SARIF,
badge, agent, cockpit, and artifact packet surfaces continue to run.

Use [RIPR blocking readiness](BLOCKING_READINESS.md) before promoting a mode.
It lists the conditions for staying advisory, requiring acknowledgement, using
baseline-check, and enabling calibrated blocking without changing generated
workflow defaults by accident.

Baseline creation and refresh guidance lives in
[CI strategy](CI.md#gate-baseline-workflow). Use that flow before enabling
`baseline-check` or `calibrated-gate`: the baseline is a visible debt ledger for
existing policy-eligible gaps, not a suppression list and not a way to hide new
findings.

## Fixture Matrix

The checked gate cases live under:

```text
fixtures/boundary_gap/expected/calibrated-gate/
```

They pin:

- visible-only advisory output;
- acknowledged `ripr-waive` output;
- baseline-check existing-gap behavior;
- calibrated high-confidence new-gap blocking;
- summary and suppressed candidates;
- missing-input `config_error`;
- recommendation and mutation calibration disagreement.

Use those fixtures when changing policy behavior. The guide is the operating
model; the fixture matrix is the regression contract.
