# RIPR Blocking Readiness

Use this guide when deciding whether an optional RIPR gate should stay
advisory, require a visible acknowledgement, or block CI.

RIPR starts as review evidence. Blocking is an adoption choice over measured
local behavior, not a default. Generated workflows keep `RIPR_GATE_MODE` unset
unless a repository explicitly configures a mode.

Read the policy-readiness report before promoting a mode:

```bash
ripr policy readiness \
  --gate-decision target/ripr/reports/gate-decision.json \
  --baseline-delta target/ripr/reports/baseline-debt-delta.json \
  --recommendation-calibration target/ripr/reports/recommendation-calibration.json \
  --mutation-calibration target/ripr/reports/mutation-calibration.json \
  --waiver-aging target/ripr/reports/waiver-aging.json \
  --suppression-health target/ripr/reports/suppression-health.json \
  --out target/ripr/reports/policy-readiness.json \
  --out-md target/ripr/reports/policy-readiness.md
```

The report is advisory. It can recommend a policy mode, but `ripr gate
evaluate` remains the only pass/fail authority when a repository explicitly
configures a gate.

## Decision Ladder

| Stage | Mode | Use when | What can fail |
| --- | --- | --- | --- |
| Observe | unset | The repository is collecting PR guidance, SARIF, badges, and artifacts for the first time. | Nothing from RIPR. |
| Explain | `visible-only` | Policy readiness is at least `ready_for_visible_only`, or the repository is intentionally collecting visible evidence before readiness inputs are wired. | Nothing from RIPR. |
| Acknowledge | `acknowledgeable` | Policy readiness is `ready_for_acknowledgeable` or better and the team wants policy-eligible gaps to require either a focused test or a visible `ripr-waive` label. | Unacknowledged policy-eligible stable Rust gaps. |
| Compare | `baseline-check` | Policy readiness is `ready_for_baseline_check` or better, the baseline is reviewed, and old debt should stay visible without blocking. | New policy-eligible stable Rust gaps not in the baseline. |
| Enforce | `calibrated-gate` | Policy readiness is `ready_for_calibrated_gate` and local calibration supports the same narrow stable Rust candidate class. | New calibrated policy-eligible stable Rust gaps. |

Move one stage at a time. Do not jump from advisory evidence to calibrated
blocking just because the evaluator exists.

## Readiness Ceiling

Treat the policy-readiness status as the ceiling for stricter gate posture:

| Policy readiness status | Maximum safe posture | Operator response |
| --- | --- | --- |
| `config_error` | unset | Fix unreadable or contradictory policy inputs before interpreting the report. |
| `not_ready` | unset or `visible-only` | Keep evidence visible and repair the unhealthy axis first. |
| `advisory_only` | unset or `visible-only` | Collect reports and reviewer experience without requiring acknowledgement. |
| `ready_for_visible_only` | `visible-only` | Surface decisions and counts, but do not require labels or baseline membership. |
| `ready_for_acknowledgeable` | `acknowledgeable` | Require focused repair or visible PR acknowledgement for eligible stable Rust gaps. |
| `ready_for_baseline_check` | `baseline-check` | Block only new eligible stable Rust debt outside the reviewed baseline. |
| `ready_for_calibrated_gate` | `calibrated-gate` | Block only the narrow eligible class covered by same-class recommendation calibration. |

This is a ceiling, not an automatic rollout. A maintainer can always stay more
advisory than the report recommends.

Use the readiness axes this way:

| Axis | Promotion signal | Stay advisory when |
| --- | --- | --- |
| `baseline_health` | Baseline exists, parses, joins current evidence, and uses shrink-only refresh. | Baseline is missing, stale, malformed, or would need new debt adoption to pass. |
| `waiver_health` | Waivers are visible PR-time acknowledgements with manageable aging. | Repeated waivers are high enough that a focused test, baseline, or suppression review should happen first. |
| `suppression_health` | Suppressions have owner, reason, scope, review date, visibility, static class, and language status. | Suppressions are missing owner/reason, stale, overbroad, unknown, or preview without preview label. |
| `calibration_health` | Recommendation calibration covers the same candidate class; mutation calibration, if present, joins unambiguously. | Calibration is missing, noisy, ambiguous, or for a different class. |
| `preview_evidence_boundary` | Preview evidence is labeled, visible, and counted as advisory only. | Preview-language findings dominate the decision or lack preview/static-limit labels. |

## Stay Advisory

Keep `RIPR_GATE_MODE` unset or use `visible-only` when any of these are true:

- reviewers have not inspected several `gate-decision.md` reports;
- PR guidance frequently points at the wrong line or needs summary-only
  fallback;
- recommendation calibration is missing, mostly `unknown`, or noisy for the
  candidate class you would block on;
- the repository has not reviewed a baseline for existing findings;
- the team has not agreed how to use `ripr-waive`;
- waiver aging is high enough that repeated acknowledgement needs review;
- durable suppressions are missing owner, reason, scope, or review state;
- preview-language findings dominate the report or lack preview labels;
- imported mutation calibration is unavailable, ambiguous, or not joined to
  the same candidate class;
- failures would not include a focused test shape and verify command;
- the workflow is still being rolled out to a new repository.

Advisory mode is still useful. It should show the top recommendation, gate
counts, labels, baseline state, calibration availability, and artifact paths in
the job summary.

## Require Acknowledgement

Use `acknowledgeable` when the team is ready to make policy-eligible findings
visible in review without turning every finding into a hard stop.

Before enabling it:

- confirm policy readiness is at least `ready_for_acknowledgeable`;
- create the `ripr-waive` label;
- make sure `target/ci/labels.json` is captured in CI;
- confirm the job summary shows acknowledged findings, not silent success;
- document when a reviewer should add the label instead of adding a focused
  test;
- keep waivers PR-local and separate from `.ripr/suppressions.toml`.

An acknowledged decision is still evidence. The finding should remain in
`gate-decision.json`, `gate-decision.md`, and the job summary with the label
that changed the decision.

Preview-language findings can be acknowledged as advisory review context, but
acknowledgement does not promote them into gate eligibility.

## Use Baseline Check

Use `baseline-check` after the repository has reviewed current gate output and
created a small checked-in baseline for historical debt.

Use [Baseline ledger workflow](BASELINE_LEDGER_WORKFLOW.md) for the concrete
`ripr baseline create`, `ripr baseline diff`, and shrink-only
`ripr baseline update --remove-resolved` commands.

Before enabling it:

- confirm policy readiness is at least `ready_for_baseline_check`;
- generate a candidate baseline from reviewed `gate-decision.json` output;
- remove malformed, suppressed, configured-off, or soon-to-fix entries;
- commit the baseline path configured by `RIPR_GATE_BASELINE`;
- run `visible-only` or `baseline-check` on the baseline PR;
- confirm baseline hits remain visible and non-blocking;
- confirm a new unbaselined candidate blocks in a controlled trial run.

Refresh the baseline by shrinking it when focused tests remove old findings.
Do not add new PR-time findings to the baseline just to make a run pass.

Preview-language evidence may appear in an advisory baseline partition, but it
must not participate in default `baseline-check` blocking unless a later
explicit policy promotes that exact class.

## Enable Calibrated Blocking

Use `calibrated-gate` only when all of these are true:

- policy readiness is `ready_for_calibrated_gate`;
- `baseline-check` already behaves predictably;
- recommendation calibration is available for the relevant candidate class;
- calibration outcomes are usually `useful`, correctly placed, and not noisy;
- missing calibration stays visible as unknown confidence instead of becoming
  a block;
- any imported mutation calibration joins unambiguously to the same seam;
- blocked summaries explain the missing discriminator, suggested focused test,
  acknowledgement path, baseline state, and verification command;
- artifact uploads still happen before the job fails.

This mode should remain narrow. It should block only new, calibrated,
policy-eligible gaps under the configured scope.

Preview TypeScript and Python evidence is not mutation-calibrated confidence
by default. A future promotion must name the language, candidate class,
static-limit exclusions, calibration threshold, baseline behavior, suppression
behavior, generated CI posture, and rollback path before preview evidence can
be used for calibrated blocking.

## Preview Evidence Boundary

Preview-language evidence follows a separate policy boundary:

| Question | Default answer |
| --- | --- |
| Can it be shown? | Yes, with `language_status = "preview"` and static-limit labels. |
| Can it be acknowledged or waived? | Yes, as visible advisory review state. |
| Can it be suppressed? | Yes, with owner, reason, scope, review date, expected visibility, static class, and preview label. |
| Can it be baselined? | Advisory partition only. |
| Can it be used for a gate? | No, unless later explicit policy promotes the exact class. |
| Can it count against RIPR 0? | No, unless later explicit policy promotes the exact class. |
| Can it provide calibrated confidence? | No, unless later explicit policy promotes the exact class. |

Generated CI should keep preview evidence visible in artifacts and summaries,
but preview evidence must not become a required check, comment-posting trigger,
RIPR 0 blocker, or default gate input by accident.

## Repair Expectations

A blocking RIPR summary should let a reviewer or follow-up agent act without
opening raw JSON first. It should include:

- policy mode and status;
- changed seam and static class;
- missing discriminator;
- suggested focused test shape;
- best related test when available;
- baseline and acknowledgement state;
- recommendation and mutation calibration availability;
- preview-language boundary state when preview findings are present;
- verify command or artifact path;
- `ripr-waive` path when acknowledgement is acceptable.

If those fields are missing or confusing, treat the mode as not ready to block.

## Rollback

Rollback is configuration-only:

```text
RIPR_GATE_MODE=
RIPR_GATE_BASELINE=
```

Unset the mode to return to advisory PR guidance, SARIF, badges, agent
artifacts, and report uploads. Keep the receipts; they explain why blocking was
paused and what evidence should improve before trying again.

## Repo Dogfood

This repository records gate-adoption receipts with:

```bash
cargo xtask dogfood
```

The report shows visible-only, acknowledged, baseline-aware, and
calibrated-gate decisions from checked RIPR evidence while preserving
`default_generated_ci_blocking = false`. Use it as a local adoption receipt,
not as a reason for default generated workflows to block.
