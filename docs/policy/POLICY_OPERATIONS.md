# Policy Operations and Promotion Readiness

GitHub tracker: pending

This is the focused Lane 2 tracker for policy operations after
[Policy readiness](POLICY_READINESS.md). It is not the global active campaign
manifest. `.ripr/goals/active.toml` remains on Campaign 27: Language Adapter
Preview. This tracker records the policy-operations work that should follow
without changing analyzer behavior, editor behavior, generated tests, mutation
execution, default CI blocking, config files, baselines, suppressions, or
preview-language gate eligibility.

## Mission

Make RIPR policy adoption operational.

Policy readiness defines what evidence is allowed to mean. Policy operations
should tell maintainers what policy posture is safe now, what blocks stricter
modes, what evidence would justify promotion, what changed over time, and why
preview-language evidence remains advisory until explicitly promoted.

The product of this tracker is policy trust. It is not more findings, editor
UX, PR-summary polish, generated tests, or default gate behavior.

## Objective

```text
Make RIPR policy adoption operational. The policy layer should not only say what
evidence means; it should tell maintainers what policy posture is safe now, what
blocks stricter modes, what changed over time, and what explicit promotion
packet would be required before baseline-check, calibrated-gate, or
preview-language evidence promotion. All outputs are read-only and advisory
unless an existing explicit gate mode is configured. No analyzer behavior,
editor behavior, generated tests, mutation execution, default CI blocking,
automatic baseline adoption, automatic config mutation, or preview evidence
promotion may occur in this campaign.
```

## Inputs

Policy operations consumes existing Lane 2 and policy artifacts:

- policy readiness;
- waiver aging;
- suppression health;
- baseline debt delta;
- gate decision;
- recommendation calibration;
- mutation calibration when explicitly supplied;
- preview evidence boundary metadata.

The tracker must not invent hidden analysis. Missing inputs should become
warnings or unknowns in read-only packets.

## End State

A maintainer should be able to generate advisory packets that answer:

- What is the current safe policy ceiling?
- What is the next safe policy action?
- Which target modes are safe now?
- Which target modes are blocked, and why?
- Which baseline, waiver, suppression, calibration, or preview-boundary repairs
  come first?
- What changed over time?
- What exact promotion packet would justify a manual policy change?
- Why does preview TypeScript or Python evidence remain visible but non-gating?

Example target shape:

```text
Current safe ceiling: ready_for_acknowledgeable
Target mode: baseline-check
Allowed now: no

Why not:
- baseline has stale entries
- suppression health has warnings
- waiver aging shows repeated PR-time acknowledgements
- preview evidence is present but not promoted
- calibration is insufficient for this class

Next safe action:
- shrink baseline
- review stale suppressions
- keep preview evidence advisory
- collect same-class calibration receipts
```

## Work Items

Spec numbers are assigned by current `main`, not by older local transcripts.
At tracker opening, `RIPR-SPEC-0034` through `RIPR-SPEC-0037` are already used
by other lanes, so the policy operations spec PRs should take the next
available spec IDs.

| Order | Work item | Purpose | Default status |
| ---: | --- | --- | --- |
| 1 | `campaign/policy-operations-tracker` | Open this focused tracker, manifest, roadmap, and plan references without behavior changes. | done |
| 2 | `spec/policy-operations-report` | Define the read-only policy operations report contract. | done |
| 3 | `policy/operations-report` | Implement `ripr policy operations` over explicit existing artifacts. | done |
| 4 | `spec/policy-history-ledger` | Define the read-only policy history report and optional append-only input. | planned |
| 5 | `policy/history-report` | Implement `ripr policy history` as advisory trend reporting. | planned |
| 6 | `spec/policy-promotion-packets` | Define read-only promotion packets for stricter configured modes. | planned |
| 7 | `policy/promotion-packet-report` | Implement `ripr policy promote --to ...` without mutating config. | planned |
| 8 | `spec/preview-evidence-promotion-packet` | Define the future preview-language promotion packet contract. | planned |
| 9 | `policy/preview-promotion-packet-report` | Implement `ripr policy preview-promote` with default `allowed_now = false`. | planned |
| 10 | `docs/policy-operator-workflow` | Document the maintainer workflow from readiness through promotion review. | planned |
| 11 | `ci/policy-operations-advisory-projection` | Surface policy operations artifacts in generated CI without pass/fail authority. | planned |
| 12 | `campaign/policy-operations-closeout` | Close after operations, history, promotion, preview-promotion, workflow, CI projection, capability, metrics, traceability, and handoff surfaces exist. | planned |

## Planned Report Surface

The policy operations report is defined by
[RIPR-SPEC-0039](../specs/RIPR-SPEC-0039-policy-operations-report.md). The
implementation is read-only and explicit about input artifacts:

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

Expected output paths:

- `target/ripr/reports/policy-operations.json`;
- `target/ripr/reports/policy-operations.md`;
- `target/ripr/reports/policy-history.json`;
- `target/ripr/reports/policy-history.md`;
- `target/ripr/reports/policy-promotion-*.json`;
- `target/ripr/reports/policy-promotion-*.md`;
- `target/ripr/reports/preview-promotion-*.json`;
- `target/ripr/reports/preview-promotion-*.md`.

## Promotion Ceiling

Promotion packets should use policy operations as the ceiling:

| Current ceiling | Safe target modes |
| --- | --- |
| `advisory_only` | none |
| `ready_for_visible_only` | `visible-only` |
| `ready_for_acknowledgeable` | `visible-only`, `acknowledgeable` |
| `ready_for_baseline_check` | `visible-only`, `acknowledgeable`, `baseline-check` |
| `ready_for_calibrated_gate` | `visible-only`, `acknowledgeable`, `baseline-check`, `calibrated-gate` for eligible stable Rust classes |

Preview-language evidence remains advisory unless a later preview promotion
packet justifies a narrow explicit promotion.

## Boundaries

This tracker does not authorize:

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
- preview-language gate promotion;
- runtime-proof claims from static evidence.

## Source-Of-Truth Roles

- Policy trackers explain why.
- Specs define externally meaningful behavior.
- Implementation plans sequence PR-sized work.
- `.ripr/goals` manifests encode execution state.
- Policy ledgers hold exceptions and receipts.
- Traceability, capability, and output-schema files prove surfaces when the
  corresponding spec or behavior exists.
- Handoffs record what shipped and what not to do.

Do not collapse these roles into one document. In this tracker-opening PR,
traceability and capability entries were intentionally deferred until the first
policy operations spec existed, because the repo verifiers require those entries
to point at real spec files. RIPR-SPEC-0039 now supplies that first behavior
contract.

## Validation

Tracker-opening validation:

```bash
cargo xtask check-doc-index
cargo xtask markdown-links
cargo xtask check-static-language
cargo xtask check-campaign
cargo xtask check-pr
git diff --check
```
