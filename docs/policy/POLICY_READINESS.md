# Policy Readiness and Preview Evidence Governance

GitHub tracker: [#755](https://github.com/EffortlessMetrics/ripr/issues/755)

This is the focused Lane 2 tracker for recommendation trust and policy. It is
not the global active campaign manifest. `.ripr/goals/active.toml` remains on
Campaign 27: Language Adapter Preview. This tracker defines the policy
boundaries Campaign 27 and later policy work must respect.

## Mission

Make RIPR evidence governable.

The policy layer decides what evidence is allowed to mean:

- advisory;
- acknowledged;
- suppressed;
- baseline-known;
- new policy-eligible;
- calibrated;
- blocking;
- stale;
- invalid;
- not applicable.

The goal is boring, auditable policy behavior across stable Rust evidence and
preview language-adapter evidence.

## Why Now

Campaign 27 adds opt-in TypeScript and Python preview adapters plus additive
`language` and `language_status` metadata. Those adapters are syntax-first and
must be labeled `preview` in public surfaces. They must report explicit static
limits instead of guessing.

Lane 2 needs to codify what that means for policy before preview evidence is
treated like mature Rust evidence.

## Evidence Boundary

Rust evidence:

- can participate in current policy surfaces when otherwise qualified;
- can be baselined, acknowledged, waived, suppressed, calibrated, or gated
  under existing explicit policy modes;
- remains governed by the existing static/runtime vocabulary boundary.

Preview TypeScript and Python evidence:

- is visible and advisory by default;
- can appear in reports, summaries, PR review surfaces, and editor surfaces;
- can support first-useful-action or assistant-proof guidance only when the
  preview status is visible;
- is not gate-eligible by default;
- does not count against RIPR Zero by default;
- is not mutation-calibrated confidence unless a later explicit spec promotes
  the same candidate class;
- must carry `language_status = "preview"` and explicit static-limit metadata
  when applicable.

## Hard Rules

- Evidence stays visible.
- Policy decides what evidence means.
- Waivers are visible PR-time acknowledgements.
- Suppressions are durable policy exceptions.
- Baselines are adoption checkpoints, not acceptance forever.
- Preview language evidence is advisory until promoted.
- Blocking is explicit, narrow, and reversible.
- Default generated CI stays non-blocking.
- No hidden mutation or runtime-proof claims.
- No automatic baseline adoption.
- No generated tests.

## Deterministic Questions

A maintainer should be able to ask these questions and get a deterministic
answer:

| Question | Required answer source |
| --- | --- |
| Can this evidence be shown? | Language status, static limits, and report visibility policy. |
| Can it be acknowledged? | Gate mode, waiver label policy, and current candidate class. |
| Can it be suppressed? | Suppression ledger policy with owner, reason, scope, and review date. |
| Can it be baselined? | Reviewed baseline policy and shrink-only refresh rules. |
| Can it be used for a gate? | Explicit gate mode plus policy eligibility and calibration boundary. |
| Can it be used for calibrated confidence? | Recommendation and optional mutation calibration for the same class. |
| Can it be used for RIPR 0? | RIPR Zero policy scope, baseline state, and preview promotion status. |

## Work Items

| Order | Work item | Purpose | Default status |
| ---: | --- | --- | --- |
| 1 | `spec/policy-readiness-report` | Define a read-only report answering which policy mode is safe for the repo right now. | done: [RIPR-SPEC-0029](../specs/RIPR-SPEC-0029-policy-readiness-report.md) |
| 2 | `spec/preview-evidence-policy-boundary` | Specify that preview-language findings are visible/advisory by default and not gate or RIPR Zero eligible without later promotion. | done: [RIPR-SPEC-0030](../specs/RIPR-SPEC-0030-preview-evidence-policy-boundary.md) |
| 3 | `report/policy-readiness` | Implement `ripr policy readiness` over explicit existing artifacts only. | done: `ripr policy readiness` writes `policy-readiness.{json,md}` |
| 4 | `report/waiver-aging` | Report repeated visible waivers as a signal, not as a failure. | done: `ripr policy waiver-aging` writes `waiver-aging.{json,md}` |
| 5 | `policy/suppression-ledger-health` | Require durable suppressions to carry identity, owner, reason, scope, dates, visibility, static class, and preview labels. | done: `ripr policy suppression-health` writes `suppression-health.{json,md}` |
| 6 | `policy/baseline-refresh-guardrails` | Document and enforce shrink-only refresh; no CI auto-adopt-new. | done: shrink-only update and generated-CI no-auto-refresh guardrails |
| 7 | `policy/exception-ledger-convergence` | Align no-panic, Clippy, non-Rust, workflow, suppression, baseline, and waiver semantics. | done: [Policy allowlists](../POLICY_ALLOWLISTS.md#shared-exception-semantics) |
| 8 | `docs/blocking-readiness-guide` | Extend the advisory-to-blocking decision tree for preview evidence and readiness health. | done: [RIPR blocking readiness](../BLOCKING_READINESS.md) |
| 9 | `ci/policy-readiness-advisory-projection` | Surface policy-readiness and waiver-aging artifacts in generated CI without pass/fail authority. | done: generated CI writes, uploads, and summarizes advisory readiness artifacts only |
| 10 | `campaign/policy-readiness-closeout` | Close the tracker only after the readiness, preview, waiver, suppression, baseline, exception, guide, and CI projection surfaces exist. | planned |

## Policy Readiness Report Target

Command:

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

Statuses:

- `advisory_only`;
- `ready_for_visible_only`;
- `ready_for_acknowledgeable`;
- `ready_for_baseline_check`;
- `ready_for_calibrated_gate`;
- `not_ready`;
- `config_error`.

Fields:

- `recommended_mode`;
- `blocking_readiness`;
- `baseline_health`;
- `waiver_health`;
- `suppression_health`;
- `calibration_health`;
- `preview_evidence_boundary`;
- `unknowns`;
- `warnings`;
- `next_policy_action`.

## Baseline Refresh Guardrails

Baseline means known before policy. It does not mean accepted forever.

Allowed:

- `ripr baseline update --remove-resolved` can remove reviewed baseline
  identities that no longer appear in current gate-decision evidence;
- a maintainer can commit that shrink-only baseline update in a reviewed PR.

Not allowed by default:

- adding new current findings to a baseline during refresh;
- treating `new_policy_eligible` as old debt;
- generated CI calling `ripr baseline update`;
- generated CI writing `.ripr/gate-baseline.json` or any configured
  `RIPR_GATE_BASELINE` path;
- generated CI inventing or accepting an `--adopt-new` path.

If a future explicit manual adopt-new command exists, it must require a
reviewed reason and stay outside generated CI.

## Exception Ledger Convergence

Lane 2 keeps policy exceptions distinct while giving each ledger the same
auditable shape: a reviewed reason, a durable identity where available, and
class-specific stale-entry behavior.

The convergence surface is [Policy allowlists](../POLICY_ALLOWLISTS.md), which
now covers:

- no-panic allowlist entries;
- Clippy lint, debt, and source-suppression ledgers;
- non-Rust file policy exceptions;
- workflow run-block and action-runtime allowlists;
- RIPR durable suppressions;
- gate baselines;
- PR waiver records.

The shared rule is that exceptions are not budgets. Baselines checkpoint known
debt, waivers acknowledge one PR, and suppressions record durable policy
exceptions with owner and reason while keeping the underlying evidence visible.

## Blocking Readiness Guide

[RIPR blocking readiness](../BLOCKING_READINESS.md) is the operator decision
tree for moving from advisory evidence toward stricter configured modes. It
uses the policy-readiness status as a ceiling:

- `config_error`, `not_ready`, and `advisory_only` stay advisory;
- `ready_for_visible_only` allows visible evidence without acknowledgement;
- `ready_for_acknowledgeable` allows PR-time acknowledgement for eligible
  stable Rust gaps;
- `ready_for_baseline_check` allows baseline-aware blocking for new eligible
  stable Rust debt;
- `ready_for_calibrated_gate` allows only the narrow eligible class backed by
  same-class recommendation calibration.

The guide keeps preview TypeScript and Python evidence visible and advisory by
default. Preview findings can be acknowledged, waived, suppressed with
metadata, or placed in an advisory baseline partition, but they cannot become
gate-eligible, RIPR Zero blocking debt, or calibrated confidence without a
later explicit promotion policy.

## Generated CI Advisory Projection

`ripr init --ci github` projects Lane 2 readiness into generated CI as uploaded
artifacts and job-summary sections only.

Generated CI may write:

- `target/ripr/reports/waiver-aging.json`;
- `target/ripr/reports/waiver-aging.md`;
- `target/ripr/reports/suppression-health.json`;
- `target/ripr/reports/suppression-health.md`;
- `target/ripr/reports/policy-readiness.json`;
- `target/ripr/reports/policy-readiness.md`.

Those steps are `continue-on-error: true`. They do not post comments, create
required checks, run mutation testing, mutate baselines, change source files, or
decide pass/fail. The only generated workflow step with gate authority remains
an explicitly configured `ripr gate evaluate`.

## Non-Goals

- No analyzer changes.
- No LSP or editor behavior changes.
- No PR summary rendering changes.
- No generated tests.
- No mutation execution.
- No provider calls.
- No release or security changes.
- No default CI blocking.
- No automatic baseline adoption.
- No preview-language gate promotion without explicit later policy.

## Validation

Docs and tracker changes should run:

```bash
cargo xtask check-doc-index
cargo xtask markdown-links
cargo xtask check-static-language
cargo xtask check-traceability
cargo xtask check-capabilities
cargo xtask check-output-contracts
cargo xtask check-pr
git diff --check
```
