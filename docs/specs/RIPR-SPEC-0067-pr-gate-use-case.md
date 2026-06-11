# RIPR-SPEC-0067: PR Gate Use Case

Status: proposed

Owner: product / swarm

Created: 2026-06-06

Linked proposal:

- None yet

Linked ADRs:

- None yet

Linked plan:

- plans/use-case-specs/implementation-plan.md (planned)

Linked issues:

- None yet

Linked PRs:

- None yet

Support-tier impact:

- None. This spec writes the user-facing contract over the existing
  optional gate (`docs/CALIBRATED_GATE_POLICY.md`,
  `docs/BLOCKING_READINESS.md`). It promotes no language, surface, or
  evidence class; preview TypeScript/Bun and Perl evidence stays
  outside gate eligibility per the preview evidence boundary.
- Claim boundaries for this surface are governed by the canonical ledger in [support tiers](../status/SUPPORT_TIERS.md); nothing here promotes a tier.

Policy impact:

- Register this spec in `policy/doc-artifacts.toml`.
- No new crates, binaries, dependencies, parsers, runtime executors,
  or files beyond the spec itself. The contract binds the existing
  `ripr gate evaluate` / `ripr baseline diff` / `ripr policy
  readiness` surfaces and their generated-CI wiring.

## Problem

This is use case UC2 of the evidence-to-repair roadmap
(RIPR-SPEC-0065). A reviewer or maintainer looking at a PR check asks
one question:

```text
Did this PR introduce new evidence debt
or regress existing repair evidence?
```

The mechanism exists: `ripr gate evaluate` consumes PR guidance
(`target/ripr/review/comments.json`) plus optional repo exposure,
SARIF policy, labels, agent verify/receipt, calibration, and baseline
inputs, and writes `target/ripr/reports/gate-decision.{json,md}`;
`ripr baseline diff` writes
`target/ripr/reports/baseline-debt-delta.{json,md}`. Modes, decision
statuses, waivers, readiness ceilings, and rollback are documented in
`docs/CALIBRATED_GATE_POLICY.md` and `docs/BLOCKING_READINESS.md`.

What is missing is the use-case contract: which deltas the gate may
reason over, which signals it must never block on, what a decision
report must contain before a human can act on it, and the wording
rules that keep an advisory or empty result from reading as a
completeness claim. Without that contract, the gate can drift toward
blocking on raw-finding churn or static limitations — exactly the
overclaiming RIPR-SPEC-0065 forbids.

## Behavior

The user should be able to answer:

```text
Did this PR add new canonical evidence debt,
resolve or regress existing repair evidence,
and if the check failed — why, and how do I reproduce it locally?
```

### Posture: advisory by default

Generated workflows leave `RIPR_GATE_MODE` unset; no gate decision is
evaluated and nothing from RIPR fails CI. Blocking happens only when a
repository explicitly configures a mode AND the policy-readiness
ceiling allows that mode. Rollback is configuration-only: unset
`RIPR_GATE_MODE` (and `RIPR_GATE_BASELINE` when present).

### Modes (closed vocabulary)

`RIPR_GATE_MODE` is one of:

| Mode | Blocks on RIPR evidence? |
| --- | --- |
| unset | No. Default. No gate decision is evaluated. |
| `visible-only` | No. Decision report without enforcement. |
| `acknowledgeable` | Yes, unless acknowledged via the `ripr-waive` label. |
| `baseline-check` | Yes, for new policy-eligible gaps outside the reviewed baseline. |
| `calibrated-gate` | Yes, narrowly: new, calibrated, policy-eligible gaps only. |

### Decision statuses (closed vocabulary)

Top-level decision status is one of `pass`, `advisory`,
`acknowledged`, `blocked`, `config_error`. Per-candidate decisions are
one of `blocking`, `acknowledged`, `advisory`, `suppressed`,
`not_applicable`. No other values are permitted; new values require a
spec revision and an output-contract update.

### Readiness ceiling

The policy-readiness status caps the strictest permitted mode:

| Policy readiness status | Maximum gate posture |
| --- | --- |
| `config_error` | unset |
| `not_ready` | unset or `visible-only` |
| `advisory_only` | unset or `visible-only` |
| `ready_for_visible_only` | `visible-only` |
| `ready_for_acknowledgeable` | `acknowledgeable` |
| `ready_for_baseline_check` | `baseline-check` |
| `ready_for_calibrated_gate` | `calibrated-gate` |

The ceiling is a ceiling, not an automatic rollout; a maintainer may
always stay more advisory.

### What the gate reasons over

Gate reasons are expressed over these deltas and states only:

- changed-surface canonical gaps (gaps on surfaces this PR touched);
- new actionable canonical gaps introduced by the PR;
- resolved gaps (evidence debt this PR retired);
- regressed gaps (previously resolved evidence that this PR
  weakened or removed);
- missing receipts (a claimed repair without its receipt artifact);
- limited or stale input state (`run_status` from the Lane 1
  completeness contract), which degrades the decision rather than
  silently shrinking it.

### What the gate MUST NOT block on

- raw finding count churn (finding totals are not canonical debt);
- static limitations themselves — a named limitation is a visibility
  state, not a defect introduced by the PR;
- preview-only TypeScript/Bun evidence (or any preview-language
  evidence) — visible, waivable, advisory only;
- a sampled or partial run presented as full — limited input may cap
  the decision at advisory/`config_error` semantics but must never
  manufacture a block from incomplete counts.

### Inputs and outputs

Inputs: `target/ripr/review/comments.json` (required), plus optional
`repo-exposure.json`, `sarif-policy.json`, `target/ci/labels.json`,
`agent-verify.json`, `agent-receipt.json`,
recommendation/mutation calibration reports, and the gate baseline.
Missing optional inputs stay visible as warnings or unknown
confidence; missing required inputs for the selected mode produce
`config_error` with a repair-oriented Markdown report.

Outputs: `target/ripr/reports/gate-decision.json` and
`gate-decision.md`; when a baseline is configured,
`target/ripr/reports/baseline-debt-delta.json` and
`baseline-debt-delta.md`. Blocking modes return non-zero only after
writing both gate-decision surfaces.

### Required output fields

A decision report a user can act on must carry every field below.
Only `decision` and the per-candidate `gate_reason` are pinned by the
accepted contract today (`docs/CALIBRATED_GATE_POLICY.md`); the rows
marked "planned addition" are the target contract delivered by the PR
gate advisory-behavior slice (see Implementation Mapping).

| Field | Meaning | Status |
| --- | --- | --- |
| `decision` | One of the closed decision statuses above. | existing |
| `reason` | The shortest explanation of why the status holds (per-candidate `gate_reason`). | existing |
| changed surfaces | Which PR-touched surfaces the candidates live on. | planned addition |
| canonical gap deltas | New, resolved, and regressed canonical gap counts and identities. | planned addition |
| receipt deltas | Receipts expected, present, and missing for claimed repairs. | planned addition |
| runtime status | `run_status` / `runtime_status` of the inputs, so limited or stale evidence is visible in the decision. | planned addition |
| local reproduction command | The `ripr gate evaluate ...` invocation that reproduces the decision locally. | planned addition |

### Required versus forbidden wording for clean and empty states

Required wording shape for a passing or empty result:

```text
pass: no visible policy-eligible candidates under mode <mode>
  over <N> changed surfaces (run_status: full)
```

An empty diff or no-action PR produces a schema-valid advisory packet
that says no candidates were evaluated — it never reads as a claim
that the repo is clean, that oracle coverage is complete, or that
evidence debt is zero.

Forbidden wording:

- presenting `advisory` or `acknowledged` as `pass`;
- presenting a `limited_*` or stale-input run as a full-run `pass`;
- runtime-strength claims: the report may say a seam is
  `weakly_gripped`, `ungripped`, or `reachable_unrevealed` under
  policy, but must not say the suite has runtime-backed strength or
  that RIPR observed runtime mutation behavior;
- hiding acknowledged decisions: an acknowledged candidate stays
  visible with the label that changed the decision.

## Non-Goals

Explicit non-claims — the gate does not and must not:

- run mutation testing, claim runtime test strength, or claim
  complete oracle coverage in any mode;
- generate tests, edit source files, post comments, or upload SARIF
  (those are separate surfaces);
- become blocking by default — generated CI keeps `RIPR_GATE_MODE`
  unset and rollout is a repository-variable change, never a forked
  workflow;
- promote preview-language evidence into gate eligibility, baselines
  (beyond an advisory partition), or calibrated confidence;
- use the baseline as a suppression list or a place to park new
  PR-time findings to make a run pass (shrink-only refresh);
- introduce alternate analyzer truth: candidates derive from
  canonical actionability plus runtime completeness, not re-derived
  raw findings.

## Required Evidence

- This spec registered in `docs/specs/README.md` and
  `policy/doc-artifacts.toml`.
- The PR gate advisory-behavior implementation slice (see
  Implementation Mapping) cites this spec and pins each
  reason-class and reject-list entry with fixtures, extending the
  existing checked matrix under
  `fixtures/boundary_gap/expected/calibrated-gate/` (visible-only,
  acknowledged `ripr-waive`, baseline-check existing-gap, calibrated
  new-gap blocking, suppressed candidates, missing-input
  `config_error`, calibration disagreement).
- Proof commands that exist today: `ripr gate evaluate`,
  `ripr baseline diff`, `ripr policy readiness`,
  `cargo xtask dogfood` (gate-adoption receipts with
  `default_generated_ci_blocking = false`), plus the docs-only gates
  (`cargo xtask check-doc-artifacts`, `check-doc-index`,
  `check-static-language`, `markdown-links`).

### Verifier reject-list

A verifier for this surface must REFUSE to render the following as a
passing or blocking success state:

- a `blocked` decision whose only basis is raw finding count churn;
- a `blocked` decision whose only basis is a static limitation or a
  named analyzer boundary;
- a `blocked` decision derived from preview-only TypeScript/Bun (or
  other preview-language) evidence;
- a `pass` rendered from a sampled, partial, limited, or stale input
  presented as full (`run_status` not `full` and not surfaced);
- any blocking decision in a mode above the current policy-readiness
  ceiling;
- a blocking exit before `gate-decision.json` and `gate-decision.md`
  are written;
- a decision report missing any required output field above
  (decision, reason, changed surfaces, canonical gap deltas, receipt
  deltas, runtime status, local reproduction command);
- an acknowledged or suppressed candidate silently dropped from the
  report.

Each rejection resolves to the fail-closed state: `config_error` with
a repair-oriented report for malformed inputs, or advisory visibility
for evidence that is real but not policy-eligible — never a
manufactured block and never a silent pass.

## Acceptance Examples

### Advisory default (mode unset)

- `RIPR_GATE_MODE` unset. No gate decision is evaluated; advisory PR
  guidance, SARIF, badges, and artifacts run as usual. Nothing from
  RIPR can fail CI.

### Visible-only decision

- Mode `visible-only`; PR guidance shows two changed-surface
  candidates. Decision `advisory`; both candidates visible with
  `gate_reason`; exit zero; reviewer reads `gate-decision.md` from
  the job summary.

### Acknowledged waiver

- Mode `acknowledgeable`; one policy-eligible candidate; PR carries
  the `ripr-waive` label captured in `target/ci/labels.json`.
- Decision `acknowledged`; the candidate remains visible with the
  label, seam, missing discriminator, and suggested test shape.

### New debt beyond baseline

- Mode `baseline-check`; baseline holds historical debt; the PR adds
  one new policy-eligible canonical gap not in the baseline.
- Decision `blocked`; `baseline-debt-delta.{json,md}` names the new
  gap; the report carries the focused test shape, acknowledgement
  path, baseline state, and the local `ripr gate evaluate ...`
  reproduction command; artifacts upload before the job fails.

### Resolved and regressed evidence

- The PR retires one canonical gap (resolved) and weakens a receipt
  for another (regressed). The deltas appear in canonical gap deltas
  and receipt deltas regardless of mode; whether `regressed` blocks
  depends on the configured mode and readiness ceiling.

### Limited input

- Repo-exposure input carries `run_status = "limited_timeout"`.
- The decision surfaces the runtime status, treats the limited counts
  as not downstream-consumable, and does not synthesize a block (or a
  full-run `pass`) from them.

### Misconfiguration

- Mode `baseline-check` with a missing or malformed baseline.
- Decision `config_error` with a repair-oriented Markdown report; the
  fix is configuration, not a waiver.

## Test Mapping

- None yet. Test mappings land with the PR gate advisory-behavior
  implementation slice; traceability entries are added only when
  behavior and tests exist.

## Implementation Mapping

- docs/specs/RIPR-SPEC-0067-pr-gate-use-case.md — this document.
- plans/use-case-specs/implementation-plan.md (planned) — the "PR
  gate advisory behavior" slice: ensure the decision report carries
  all required output fields, express gate reasons as canonical gap /
  receipt deltas over changed surfaces, surface input `run_status` in
  the decision, and pin the reject-list with fixtures.
- Existing surfaces this contract binds: `ripr gate evaluate`,
  `ripr baseline diff`, `ripr policy readiness`,
  `docs/CALIBRATED_GATE_POLICY.md`, `docs/BLOCKING_READINESS.md`,
  the generated-CI `RIPR_GATE_MODE` / `RIPR_GATE_BASELINE` wiring,
  and `fixtures/boundary_gap/expected/calibrated-gate/`.

## Metrics

- Blocking precision: blocked decisions trace to a new, regressed, or
  receipt-missing canonical delta on a changed surface — zero blocks
  from reject-list signals, measured by the fixture matrix.
- Actionability of failure: 100% of blocking reports contain every
  required output field, including the local reproduction command.
- Posture integrity: zero observed decisions above the readiness
  ceiling; `default_generated_ci_blocking` stays `false` in dogfood
  receipts.
- Mode adoption health: waiver aging and suppression health stay
  within the readiness axes before any mode promotion.
- Promotion rule: move this spec from `proposed` to `accepted` when
  the implementation satisfies every entry of the verifier
  reject-list with fixture-backed proof and the proof commands above
  exist and pass.

## Failure Modes

- Required input missing for the selected mode — `config_error` with
  a repair-oriented report; never a guessed decision.
- Labels not captured in CI — acknowledgement cannot apply; the
  candidate stays a visible blocking/advisory candidate rather than a
  silent pass.
- Baseline drift or new-debt adoption into the baseline — forbidden;
  refresh is shrink-only and the delta report keeps new debt visible.
- Limited or stale evidence inputs — visible in runtime status;
  capped at advisory semantics; never presented as a full-run `pass`
  and never the sole basis of a block.
- Mode configured above the readiness ceiling — treat as
  not-ready-to-block; the readiness report is the named repair route.
- Blocking exit without written reports — contract violation; the
  decision surfaces must exist before any non-zero exit.
- Rollback needed — unset `RIPR_GATE_MODE` / `RIPR_GATE_BASELINE`;
  advisory guidance, SARIF, badges, and artifact uploads continue.
