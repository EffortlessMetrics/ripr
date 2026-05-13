# Policy Operations Workflow

Use this workflow when a maintainer wants to decide whether RIPR policy can be
tightened without changing policy first.

Policy operations is an advisory review layer:

```text
policy readiness
-> policy operations
-> policy history
-> promotion packet
-> manual review
-> manual config change, if justified
-> post-change history review
```

The reports do not execute a gate. `ripr gate evaluate` remains the pass/fail
authority only when a repository explicitly configures a gate mode.

## 1. Generate Readiness Inputs

Start by producing the artifacts that policy readiness and operations consume.
Use the reports that exist for the repository; missing optional inputs stay
visible as warnings or unknowns.

Common inputs:

- `target/ripr/reports/gate-decision.json`
- `target/ripr/reports/baseline-debt-delta.json`
- `target/ripr/reports/waiver-aging.json`
- `target/ripr/reports/suppression-health.json`
- `target/ripr/reports/recommendation-calibration.json`
- `target/ripr/reports/mutation-calibration.json`, when explicitly supplied

Then run readiness:

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

Read `policy-readiness.md` first. If it reports `config_error`, repair the
unreadable or contradictory input before interpreting stricter policy modes.

## 2. Generate Policy Operations

Policy operations turns readiness plus supporting ledgers into the operator
packet:

```bash
ripr policy operations \
  --policy-readiness target/ripr/reports/policy-readiness.json \
  --waiver-aging target/ripr/reports/waiver-aging.json \
  --suppression-health target/ripr/reports/suppression-health.json \
  --baseline-delta target/ripr/reports/baseline-debt-delta.json \
  --gate-decision target/ripr/reports/gate-decision.json \
  --recommendation-calibration target/ripr/reports/recommendation-calibration.json \
  --mutation-calibration target/ripr/reports/mutation-calibration.json \
  --out target/ripr/reports/policy-operations.json \
  --out-md target/ripr/reports/policy-operations.md
```

Read `policy-operations.md` as the first-screen answer:

| Field | Meaning | Maintainer action |
| --- | --- | --- |
| Current ceiling | The strictest policy posture the current inputs support. | Treat it as a ceiling, not a rollout command. |
| Next safe action | The first repair or review step before stricter policy. | Do this before editing config. |
| Can promote to | Target modes that are at or below the current ceiling. | Generate a promotion packet before changing anything. |
| Cannot promote to | Target modes blocked by readiness or input health. | Read blockers and repair ledgers first. |
| Promotion blockers | Baseline, waiver, suppression, calibration, preview, or input issues that block stricter modes. | Fix the highest-severity blocker first. |
| Input artifacts | Which supplied files were read, missing, or malformed. | Regenerate missing or malformed inputs before trusting the packet. |

Keep baseline, waiver, and suppression buckets separate:

- baseline debt is reviewed historical debt that should stay visible;
- waivers are visible PR-time acknowledgements;
- suppressions are durable policy exceptions with owner, reason, and scope.

## 3. Review Trend With Policy History

Operations is point-in-time. History answers whether policy health is improving
or decaying.

```bash
ripr policy history \
  --current target/ripr/reports/policy-operations.json \
  --history .ripr/policy-history.jsonl \
  --commit HEAD \
  --pr-number 123 \
  --out target/ripr/reports/policy-history.json \
  --out-md target/ripr/reports/policy-history.md
```

The command reads optional history; it does not append to
`.ripr/policy-history.jsonl`.

Use the history report to answer:

- Did readiness improve?
- Did waiver pressure rise?
- Did suppression health regress?
- Did baseline debt shrink?
- Did calibration change the ceiling?
- Did preview evidence remain advisory?

If history is missing, treat trend fields as unknown rather than inventing
movement from one snapshot.

## 4. Generate A Promotion Packet

Before changing policy, generate the packet for the target posture.

```bash
ripr policy promote \
  --to baseline-check \
  --operations target/ripr/reports/policy-operations.json \
  --history target/ripr/reports/policy-history.json \
  --out target/ripr/reports/policy-promotion-baseline-check.json \
  --out-md target/ripr/reports/policy-promotion-baseline-check.md
```

Use these target modes:

```text
visible-only
acknowledgeable
baseline-check
calibrated-gate
```

Read the packet this way:

| Field | Meaning |
| --- | --- |
| `allowed_now` | Whether the target mode is at or below the current operations ceiling. |
| `why_or_why_not` | Reviewable explanation for the decision. |
| `required_repairs` | Work that should happen before promotion. |
| `required_receipts` | Artifacts reviewers should inspect before changing policy. |
| `rollback_path` | How to return to a more advisory posture. |
| `example_config_change` | Manual example only, not an applied change. |

The promotion command does not mutate `ripr.toml`, repository variables,
baselines, suppressions, workflows, branch protection, history ledgers, or gate
mode. A maintainer must make any policy change manually after review.

## 5. Promotion Examples

### Advisory To Visible-Only

Use when the team wants policy decisions visible in reports and summaries
without requiring acknowledgement or baseline membership.

Expected packet:

```text
target_mode: visible-only
allowed_now: true
required_repairs: none, or low-severity input hygiene
```

Manual change, if accepted:

```text
RIPR_GATE_MODE=visible-only
```

Rollback:

```text
RIPR_GATE_MODE=
```

### Visible-Only To Acknowledgeable

Use when policy-eligible stable Rust findings should require a focused test or
visible PR-time acknowledgement.

Before promotion:

- confirm readiness is at least `ready_for_acknowledgeable`;
- confirm `ripr-waive` label handling is documented;
- review waiver aging for repeated acknowledgements;
- keep waivers separate from suppressions.

Manual change, if accepted:

```text
RIPR_GATE_MODE=acknowledgeable
```

### Acknowledgeable To Baseline-Check

Use when reviewed historical debt should stay visible while new eligible stable
Rust debt is governed.

Before promotion:

- review `baseline-debt-delta.md`;
- remove stale or malformed baseline entries;
- confirm the baseline was reviewed, not auto-adopted;
- confirm new policy-eligible debt is not being added to the baseline.

Manual change, if accepted:

```text
RIPR_GATE_MODE=baseline-check
RIPR_GATE_BASELINE=.ripr/gate-baseline.json
```

Rollback:

```text
RIPR_GATE_MODE=acknowledgeable
```

### Baseline-Check To Calibrated-Gate

Use only for a narrow stable Rust class when same-class calibration supports
blocking.

Before promotion:

- confirm readiness is `ready_for_calibrated_gate`;
- confirm recommendation calibration covers the candidate class;
- treat mutation calibration as supplied runtime evidence only when explicitly
  present and joined to the same class;
- confirm baseline, waiver, and suppression health are clean enough for a
  reversible gate.

Manual change, if accepted:

```text
RIPR_GATE_MODE=calibrated-gate
```

Rollback:

```text
RIPR_GATE_MODE=baseline-check
```

## 6. Preview Evidence Promotion Packet

Preview TypeScript and Python evidence stays visible and advisory by default.
It is not gate-eligible, not RIPR Zero blocking debt, and not calibrated
confidence unless a later explicit policy promotes a narrow language/class.

Generate a preview packet when a maintainer wants to understand what evidence
would be required:

```bash
ripr policy preview-promote \
  --language typescript \
  --class boundary_gap \
  --out target/ripr/reports/preview-promotion-typescript-boundary-gap.json \
  --out-md target/ripr/reports/preview-promotion-typescript-boundary-gap.md
```

The default result is blocked:

```text
allowed_now: false
reason: preview promotion evidence not supplied
```

When explicit evidence receipts are available, supply them without changing
the default boundary:

```bash
ripr policy preview-promote \
  --language typescript \
  --class boundary_gap \
  --evidence target/ripr/reports/preview-promotion-evidence.json \
  --out target/ripr/reports/preview-promotion-typescript-boundary-gap.json \
  --out-md target/ripr/reports/preview-promotion-typescript-boundary-gap.md
```

Required receipts include:

- fixture corpus coverage;
- static-limit exclusions;
- false-positive review;
- recommendation calibration;
- baseline behavior;
- waiver and suppression behavior;
- rollback path;
- generated CI posture.

Mutation calibration is optional evidence; it is not inferred from stable Rust
calibration.

## 7. Manual Config Review

Do not treat any policy report as a config writer.

Before changing policy:

- inspect `policy-operations.md`;
- inspect the target `policy-promotion-*.md` packet;
- confirm `allowed_now` and `why_or_why_not`;
- confirm required repairs are complete or intentionally deferred;
- confirm required receipts are attached to the PR or issue;
- confirm rollback is documented;
- make the config or repository-variable change manually in a separate,
  reviewable step.

Use a separate PR when the config change is operationally risky or when the
baseline/suppression ledger changes at the same time.

## 8. Post-Change Monitoring

After a manual policy change, regenerate:

```bash
ripr policy readiness ...
ripr policy operations ...
ripr policy history ...
```

Watch for:

- current ceiling falling below the configured mode;
- waiver pressure rising;
- stale suppressions;
- baseline debt growing or failing to shrink;
- preview evidence crossing the advisory boundary;
- calibration no longer covering the candidate class.

If the configured mode is above the current ceiling, roll back to the last
supported mode and keep the receipts.

## Hard Boundaries

This workflow does not authorize:

- analyzer truth changes;
- evidence identity rewrites;
- recommendation ranking changes;
- LSP/editor behavior changes;
- PR/CI front-panel redesign;
- generated tests;
- provider calls;
- mutation execution;
- default CI blocking;
- automatic config mutation;
- automatic baseline adoption;
- automatic suppression creation;
- workflow or branch-protection mutation;
- preview-language promotion;
- runtime-proof claims from static evidence.
