# RIPR Blocking Readiness

Use this guide when deciding whether an optional RIPR gate should stay
advisory, require a visible acknowledgement, or block CI.

RIPR starts as review evidence. Blocking is an adoption choice over measured
local behavior, not a default. Generated workflows keep `RIPR_GATE_MODE` unset
unless a repository explicitly configures a mode.

## Decision Ladder

| Stage | Mode | Use when | What can fail |
| --- | --- | --- | --- |
| Observe | unset | The repository is collecting PR guidance, SARIF, badges, and artifacts for the first time. | Nothing from RIPR. |
| Explain | `visible-only` | Reviewers need to see gate decisions and counts without enforcement. | Nothing from RIPR. |
| Acknowledge | `acknowledgeable` | The team wants policy-eligible gaps to require either a focused test or a visible `ripr-waive` label. | Unacknowledged policy-eligible gaps. |
| Compare | `baseline-check` | The team has reviewed existing debt and wants only new baseline misses to block. | New policy-eligible gaps not in the baseline. |
| Enforce | `calibrated-gate` | Local recommendation calibration supports the same narrow candidate class and the baseline is maintained. | New calibrated policy-eligible gaps. |

Move one stage at a time. Do not jump from advisory evidence to calibrated
blocking just because the evaluator exists.

## Stay Advisory

Keep `RIPR_GATE_MODE` unset or use `visible-only` when any of these are true:

- reviewers have not inspected several `gate-decision.md` reports;
- PR guidance frequently points at the wrong line or needs summary-only
  fallback;
- recommendation calibration is missing, mostly `unknown`, or noisy for the
  candidate class you would block on;
- the repository has not reviewed a baseline for existing findings;
- the team has not agreed how to use `ripr-waive`;
- failures would not include a focused test shape and verify command;
- the workflow is still being rolled out to a new repository.

Advisory mode is still useful. It should show the top recommendation, gate
counts, labels, baseline state, calibration availability, and artifact paths in
the job summary.

## Require Acknowledgement

Use `acknowledgeable` when the team is ready to make policy-eligible findings
visible in review without turning every finding into a hard stop.

Before enabling it:

- create the `ripr-waive` label;
- make sure `target/ci/labels.json` is captured in CI;
- confirm the job summary shows acknowledged findings, not silent success;
- document when a reviewer should add the label instead of adding a focused
  test;
- keep waivers PR-local and separate from `.ripr/suppressions.toml`.

An acknowledged decision is still evidence. The finding should remain in
`gate-decision.json`, `gate-decision.md`, and the job summary with the label
that changed the decision.

## Use Baseline Check

Use `baseline-check` after the repository has reviewed current gate output and
created a small checked-in baseline for historical debt.

Use [Baseline ledger workflow](BASELINE_LEDGER_WORKFLOW.md) for the concrete
`ripr baseline create`, `ripr baseline diff`, and shrink-only
`ripr baseline update --remove-resolved` commands.

Before enabling it:

- generate a candidate baseline from reviewed `gate-decision.json` output;
- remove malformed, suppressed, configured-off, or soon-to-fix entries;
- commit the baseline path configured by `RIPR_GATE_BASELINE`;
- run `visible-only` or `baseline-check` on the baseline PR;
- confirm baseline hits remain visible and non-blocking;
- confirm a new unbaselined candidate blocks in a controlled trial run.

Refresh the baseline by shrinking it when focused tests remove old findings.
Do not add new PR-time findings to the baseline just to make a run pass.

## Enable Calibrated Blocking

Use `calibrated-gate` only when all of these are true:

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
