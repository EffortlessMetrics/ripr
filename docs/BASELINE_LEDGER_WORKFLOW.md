# Baseline Ledger Workflow

Use this workflow when a repository wants RIPR to behave like an incremental
behavioral-grip debt ledger: show the full current picture, checkpoint
reviewed historical findings, control new policy-eligible gaps, and shrink the
baseline as focused tests move static evidence.

A baseline is not a suppression. It is a reviewed checkpoint that says:

```text
This finding was already visible when policy started.
Keep it visible.
Do not punish every future PR for it.
Remove it when current evidence no longer contains it.
```

## Adoption Path

Move in this order:

```text
observe
-> create a reviewed baseline
-> compare current evidence to baseline debt
-> enable baseline-check
-> add focused tests
-> remove resolved baseline entries
-> move toward RIPR 0 under configured scope
```

`RIPR 0` means no visible unresolved behavioral test-grip gaps remain under the
configured scope and policy. It does not mean the test suite is perfect or that
RIPR has runtime mutation confirmation.

## 1. Start Advisory

Generate the GitHub workflow and leave gate variables unset:

```bash
ripr init --ci github
```

Repository variables:

```text
RIPR_GATE_MODE=
RIPR_GATE_BASELINE=
```

This gives the team advisory summaries, PR guidance, SARIF when enabled,
badges, agent packets, gate artifacts when explicitly enabled later, and
uploaded report artifacts without blocking CI.

For the first baseline pass, set `visible-only` so the workflow writes
`gate-decision.{json,md}` without enforcement:

```text
RIPR_GATE_MODE=visible-only
RIPR_GATE_BASELINE=
```

Review:

- `target/ripr/reports/gate-decision.md`;
- `target/ripr/reports/gate-decision.json`;
- `target/ripr/review/comments.json` when PR guidance was generated;
- the job-summary gate section.

Do not create a baseline from a report nobody has reviewed.

## 2. Create A Candidate Baseline

Use `ripr baseline create` to turn reviewed gate decisions into a candidate
ledger:

```bash
ripr baseline create \
  --from target/ripr/reports/gate-decision.json \
  --out target/ripr/reports/gate-baseline.candidate.json
```

The command writes stable JSON and refuses to overwrite an existing file unless
`--force` is passed. It checkpoints visible advisory, acknowledged, and
blocking decisions. It skips suppressed, configured-off, malformed, and
config-error-only inputs.

Review the candidate before checking it in:

- every entry came from current `gate-decision.json` or PR guidance evidence;
- every entry represents existing debt, not a finding introduced by the
  baseline PR;
- every entry should remain visible in future summaries or artifacts;
- malformed, suppressed, configured-off, or soon-to-fix entries are removed;
- the PR explains the adoption date, configured scope, source artifact, and
  owner.

After review, copy the candidate into the configured baseline path:

```text
.ripr/gate-baseline.json
```

Commit that baseline in its own PR. Keep `RIPR_GATE_MODE=visible-only` on the
first baseline PR unless the team has already validated `baseline-check` in a
controlled trial run.

## 3. Compare Baseline Debt

Use `ripr baseline diff` to compare the checked-in baseline with current gate
evidence:

```bash
ripr baseline diff \
  --baseline .ripr/gate-baseline.json \
  --current target/ripr/reports/gate-decision.json \
  --out target/ripr/reports/baseline-debt-delta.json \
  --out-md target/ripr/reports/baseline-debt-delta.md
```

The delta report is advisory. It explains movement; it does not decide whether
CI passes or fails. `ripr gate evaluate` remains the pass/fail authority.

Read the buckets this way:

| Bucket | Meaning | Usual action |
| --- | --- | --- |
| `still_present` | Reviewed baseline debt still appears in current evidence. | Keep visible or burn down with a focused test. |
| `resolved` | Reviewed baseline debt no longer appears in current evidence. | Remove it with shrink-only update after review. |
| `new_policy_eligible` | Current policy-eligible gap is not in the baseline. | Add a focused test or acknowledge visibly. |
| `acknowledged` | Current gap was accepted for this PR with a label such as `ripr-waive`. | Keep visible in review; do not treat as hidden success. |
| `suppressed` | Current gap is hidden or configured off by repository policy. | Review suppression policy separately from baseline debt. |
| `stale_baseline_entry` | Baseline entry parses but cannot join cleanly to current evidence. | Repair or remove after review. |
| `invalid_baseline_entry` | Baseline entry is malformed or lacks stable identity fields. | Repair the baseline before relying on stricter modes. |
| `missing_current_input` | Current gate evidence was unavailable or unreadable. | Regenerate the missing artifact. |

Generated CI runs the same diff when both are true:

- `RIPR_GATE_BASELINE` is set;
- `target/ripr/reports/gate-decision.json` exists.

It uploads:

```text
target/ripr/reports/baseline-debt-delta.json
target/ripr/reports/baseline-debt-delta.md
```

and adds a `Baseline debt delta` section to the job summary. The generated
workflow must not rewrite `.ripr/gate-baseline.json`.

## 4. Enable Baseline Check

After the baseline PR is reviewed and merged, set:

```text
RIPR_GATE_MODE=baseline-check
RIPR_GATE_BASELINE=.ripr/gate-baseline.json
```

Expected behavior:

```text
Existing baseline identity: visible and non-blocking
New policy-eligible identity: blocking in baseline-check
Acknowledged identity: visible as acknowledged
Suppressed identity: visible as suppressed or not applicable when present
Missing or invalid baseline: config_error
```

Use `baseline-check` only after reviewers understand the job summary and can
find the repair path: missing discriminator, suggested test shape, related test
when available, acknowledgement label, and verification command.

## 5. Review New Debt

When the delta report shows `new_policy_eligible`, do one of three things:

1. Add one focused test that targets the missing observation.
2. Add `ripr-waive` when the team accepts the PR-time exception and wants it
   visible as `acknowledged`.
3. Keep the PR blocked until the finding is repaired or explicitly
   acknowledged.

Do not add a new current finding to the baseline just to make a PR pass. A
future `--adopt-new` path would need an explicit reviewed reason; the current
safe workflow is shrink-only.

## 6. Shrink The Baseline

After focused tests move static evidence, regenerate the gate decision and
delta report. Then remove resolved identities:

```bash
ripr baseline update \
  --baseline .ripr/gate-baseline.json \
  --current target/ripr/reports/gate-decision.json \
  --remove-resolved \
  --out .ripr/gate-baseline.json
```

The update command is shrink-only. It removes reviewed baseline entries that
are absent from current gate-decision evidence, preserves malformed or
ambiguous entries for review, and refuses to adopt new current debt.

Commit shrink-only updates in PRs that explain:

- which baseline identities were removed;
- which focused tests or evidence changes caused the movement;
- whether any new policy-eligible gaps remain;
- whether `ripr-waive` or suppressions were involved.

CI should never auto-write the baseline. Baseline updates are repository
changes that require review.

## Baseline, Waiver, Suppression

Use the three controls separately:

| Control | Scope | Use for | Do not use for |
| --- | --- | --- | --- |
| Baseline | Checked-in debt ledger | Reviewed historical findings that should remain visible while avoiding repeat PR punishment. | Hiding new PR-time findings. |
| `ripr-waive` | One pull request | Visible acknowledgement of a current finding. | Durable acceptance across future PRs. |
| `.ripr/suppressions.toml` | Repository policy | Durable configured-off or accepted debt with owner and reason. | Avoiding baseline review or PR acknowledgement. |

The review record should always answer:

```text
What was already in the baseline?
What is new?
What was resolved?
What was acknowledged?
What was suppressed?
What should be tested next?
```

## RIPR 0 Route

Use baseline deltas to turn raw behavioral-grip debt into a burn-down plan:

```text
Raw current findings
-> reviewed baseline identities
-> still-present baseline debt
-> new policy-eligible gaps
-> focused test packets
-> static movement receipts
-> resolved baseline entries
-> smaller baseline
```

A healthy adoption PR can have a nonzero baseline. A healthy burn-down PR
should reduce `still_present` or increase `resolved` without adding
`new_policy_eligible` debt. Over time, the configured-scope target is:

```text
new_policy_eligible = 0
still_present = 0
invalid_baseline_entry = 0
stale_baseline_entry = 0
```

Suppressed and unknown-confidence cases can remain visible with reasons; they
are not the same as unresolved baseline debt.

## Related Docs

- [CI strategy](CI.md#gate-baseline-workflow) shows the generated workflow and
  repository-variable examples.
- [Calibrated gate policy](CALIBRATED_GATE_POLICY.md) defines gate modes and
  decision vocabulary.
- [RIPR blocking readiness](BLOCKING_READINESS.md) explains when a repository
  is ready to move from advisory to acknowledgement, baseline-check, or
  calibrated blocking.
- [Output schema](OUTPUT_SCHEMA.md#baseline-debt-delta-report) defines the
  baseline ledger, update, and delta JSON/Markdown contracts.
