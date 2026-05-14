# RIPR-SPEC-0046: Gap Decision Ledger

Status: proposed

## Problem

RIPR now separates many kinds of truth: evidence records, finding-to-gap
alignment, policy ledgers, baselines, waivers, suppressions, gate decisions,
PR review front panels, report packet indexes, inline comment plans, editor
diagnostics, agent packets, and receipts. Those surfaces are useful, but they
can still project raw evidence differently.

The missing contract is the decision layer that says which aligned evidence is
a real, repairable RIPR gap, which policy state applies, where the gap may
surface, and what proves movement. Without that layer, projections can
incorrectly turn raw `exposed`, `static_unknown`, confidence, preview language,
or raw report count fields into user instructions, gate failures, badge counts,
or editor diagnostics.

## Behavior

RIPR should produce a typed gap decision ledger that composes existing explicit
artifacts into `GapRecord` entries. The ledger is the shared source for
projectable Rust gaps. It does not create analyzer truth and does not replace
raw evidence records.

The intended flow is:

```text
raw evidence
-> finding-to-gap alignment
-> GapRecord
-> PR comment / CI summary / gate / badge / LSP / agent packet / receipt
```

Projection surfaces should consume `GapRecord` fields when deciding whether to
interrupt a reviewer, render an inline comment, count a badge target, evaluate
an optional gate, show an editor diagnostic, or create an agent repair packet.
They may display source evidence as context, but they must not infer
projectability from raw findings alone.

### Ledger Inputs

The ledger may read only explicit artifacts supplied by existing RIPR commands
or user configuration:

- aligned canonical evidence items from `RIPR-SPEC-0045`;
- evidence records from `RIPR-SPEC-0021`;
- PR guidance and first useful action reports;
- baseline debt delta and RIPR Zero status reports;
- PR evidence ledger records;
- waiver, acknowledgement, suppression, and policy ledgers;
- optional gate decision artifacts;
- optional receipt and before/after movement artifacts;
- optional presentation-text evidence from `RIPR-SPEC-0043`;
- optional preview-language evidence, which remains advisory and ineligible
  for default gate, RIPR Zero, and public badge authority.

The ledger must not rerun hidden analysis, execute mutation tests, run tests,
edit source, generate tests, call providers, publish comments, mutate
baselines, or change gate policy.

### GapRecord

Each ledger item should contain a stable, additive shape:

```rust
GapRecord {
    gap_id,
    canonical_gap_id,
    kind,
    language,
    language_status,
    scope,
    evidence_class,
    gap_state,
    policy_state,
    repairability,
    repair_route,
    anchor,
    evidence_ids,
    projection_eligibility,
    verification_commands,
    receipt,
    movement,
    authority_boundary,
}
```

The exact Rust type and JSON version are implementation details, but the public
ledger output must preserve these concepts with stable names or documented
compatibility aliases.

### Kind

Gap kinds name the repair problem, not the raw classifier label. The initial
Rust-focused vocabulary should include:

| Kind | Meaning |
| --- | --- |
| `MissingBoundaryAssertion` | A related test reaches the owner but lacks the boundary discriminator. |
| `MissingErrorDiscriminator` | A related test checks broad error shape but not the exact error discriminator. |
| `MissingValueAssertion` | A related test reaches the owner but lacks the changed return or field value discriminator. |
| `MissingSideEffectObserver` | The changed behavior affects an effect or call that lacks an observer. |
| `MissingOutputContract` | User-visible output or presentation text lacks a supported output observer. |
| `StaticLimitation` | Named static limitation blocks a repairable user gap decision. |
| `NoActionAlreadyObserved` | Supported evidence says the changed behavior is already observed. |
| `NoActionInternal` | Supported evidence says the change is internal-only in documented scope. |
| `Unknown` | The evidence is insufficient to choose a sharper gap kind. |

Raw static labels such as `exposed`, `weakly_exposed`, `reachable_unrevealed`,
`no_static_path`, `infection_unknown`, `propagation_unknown`, and
`static_unknown` may contribute evidence. They are not gap kinds by themselves.

### Scope

`scope` must distinguish where the decision applies:

| Scope | Meaning |
| --- | --- |
| `pr_local` | The gap is tied to changed behavior in the current diff. |
| `repo_scoped` | The gap exists in repo-level evidence independent of one PR. |
| `baseline_debt` | The gap is known existing debt under a reviewed baseline. |
| `artifact_missing` | The ledger cannot decide because an expected artifact is missing, stale, or malformed. |

Scope is separate from policy state. A repo-scoped gap can be waived or
suppressed; a PR-local gap can be baseline-known.

### Policy State

`policy_state` describes overlays supplied by policy artifacts:

| State | Meaning |
| --- | --- |
| `new` | New relative to supplied baseline/policy context. |
| `baseline_known` | Known baseline debt. |
| `waived` | Explicitly waived by a waiver artifact. |
| `suppressed` | Suppressed by policy. |
| `acknowledged` | Acknowledged but not resolved. |
| `resolved` | Previously present gap is no longer present. |
| `reintroduced` | Previously resolved gap appears again. |
| `blocked` | Configured gate or policy says action is required. |
| `not_policy_targeted` | Evidence exists, but current policy does not target it. |
| `unknown` | Policy state cannot be determined from supplied artifacts. |

Policy state must not replace evidence state. A suppressed actionable gap
remains actionable evidence with a suppression overlay.

### Repairability

`repairability` must be explicit:

| Repairability | Meaning |
| --- | --- |
| `repairable` | RIPR has a bounded repair route and verification command. |
| `needs_human_design` | RIPR can identify the risk but not a bounded mechanical repair route. |
| `analyzer_limitation` | The useful next action is to improve evidence extraction, not write a product test. |
| `no_action` | Current evidence says no user repair is needed. |
| `unknown` | RIPR cannot determine repairability from supplied artifacts. |

A gate, PR comment, or agent repair packet must not treat `unknown`,
`analyzer_limitation`, or `no_action` as a user repairable test gap.

### Repair Route

Repair routes must be typed enough for humans and agents:

- `route_kind`, such as `AddBoundaryAssertion`, `AddErrorVariantAssertion`,
  `AddValueAssertion`, `AddSideEffectObserver`, `AddOutputGolden`,
  `InspectStaticLimit`, or `NoAction`;
- target file and optional line when available;
- related test or suggested test file when available;
- assertion or observer shape when known;
- changed behavior text or discriminator when safe to include;
- one or more verification commands;
- stop conditions for agent repair when the evidence is insufficient.

Repair routes are guidance. RIPR still must not generate tests or edit source.

### Anchor And Dedupe

Projectable gaps need stable anchors:

- `canonical_gap_id` or equivalent semantic ID;
- changed file and line or source span when available;
- owner or symbol when available;
- dedupe fingerprint for PR comments and editor diagnostics;
- source evidence IDs for traceability.

Line movement should not create duplicate comments when the semantic gap is
unchanged. Different discriminators, owners, or evidence classes must not
collide.

### Projection Eligibility

`projection_eligibility` must state where a gap may surface and why:

| Projection | Eligibility rule |
| --- | --- |
| `ci_summary` | May summarize actionable, no-action, missing-artifact, and policy states with advisory language. |
| `report_packet` | May include all ledger records as navigable evidence. |
| `pr_comment` | Requires PR-local scope, stable anchor, dedupe fingerprint, repairable route, verification command, and non-preview Rust evidence. |
| `lsp_diagnostic` | Requires local file scope and structured payload; static limitations are informational unless a repair route exists. |
| `agent_packet` | Requires a bounded repair or inspection route plus stop conditions and verification command. |
| `gate_candidate` | Requires the safe gate predicate below. |
| `ripr_zero_count` | Requires repo-scoped, unresolved, unsuppressed, unwaived, policy-targeted Rust gap state. |
| `ripr_plus_count` | Requires broader unresolved Rust advisory exposure gap state. |

Eligibility should include both boolean outcome and explanation, for example:

```json
{
  "pr_comment": {
    "eligible": false,
    "reason": "static_limitation_only"
  }
}
```

### Safe Gate Predicate

Optional gates may only treat a gap as blocking when every condition is true:

- language is Rust;
- language status is stable/reference, not preview;
- scope is `pr_local`;
- policy state is `new` or explicit blocking candidate;
- gap is unresolved;
- gap is not suppressed, waived, acknowledged-only, or baseline-known;
- gap is not preview-language evidence;
- gap is not static-unknown-only;
- repairability is `repairable`;
- repair route is present;
- verification command is present;
- configured policy target enables that gap kind.

Gate decisions remain separate artifacts. The ledger may identify gate
candidates, but the generated summary, PR comment, LSP diagnostic, badge, and
packet index do not become pass/fail authority.

### Badge Targets

Public badges are repo-scoped trust markers, not PR-local evidence.

`ripr 0` should mean:

```text
zero unresolved, unsuppressed, unwaived, repo-scoped,
policy-targeted Rust gaps
```

`ripr+` should mean:

```text
zero broader unresolved Rust advisory exposure gaps
```

Neither badge target means runtime mutation adequacy, coverage adequacy, PR
adequacy, or general correctness.

### Presentation And Output Text

Presentation/output text gaps should consume `RIPR-SPEC-0043` evidence. A
`MissingOutputContract` decision is allowed only when supported visibility and
observer evidence show a user-visible output contract lacks a supported
observer. The repair route should be output-proof oriented, for example
`AddOutputGolden`, `AddHelpOutputSnapshot`, or `AddReportRenderGolden`.

Visibility-unknown presentation text remains a static limitation or inspection
route. It must not render as generic `static_unknown` user instruction or
mutation-testing escalation.

### Receipts And Movement

`receipt` and `movement` fields must tie before/after proof to a selected
`GapRecord`. Receipt existence alone is insufficient; the record should state
whether movement is:

- `improved`;
- `unchanged_after_attempt`;
- `resolved`;
- `worsened`;
- `missing_receipt`;
- `not_applicable`.

Verification commands should regenerate the evidence needed to reassess the
same gap decision.

## Required Evidence

An implemented gap decision ledger must provide:

- a versioned JSON and Markdown report or equivalent public artifact;
- stable gap IDs and canonical gap IDs when supplied;
- source evidence IDs and artifact paths;
- language and language-status fields;
- scope, evidence class, gap state, policy state, and repairability;
- typed repair routes and verification commands for repairable gaps;
- anchor and dedupe fields for PR comments and editor diagnostics;
- projection eligibility with explanations;
- safe gate candidate calculation without creating gate authority;
- repo-scoped `ripr 0` and `ripr+` target inputs;
- receipt and movement state when supplied;
- missing, stale, or malformed artifact warnings that do not become clean,
  waived, suppressed, improved, or runtime-confirmed states.

The ledger must preserve advisory/static language and must not claim runtime
mutation proof unless imported runtime calibration artifacts explicitly provide
that context.

## Non-Goals

- No analyzer classification or ranking changes in this spec.
- No mutation execution.
- No runtime test execution.
- No generated tests.
- No automatic source edits.
- No provider or model calls.
- No default CI blocking change.
- No branch protection change.
- No inline comment publishing by the ledger command.
- No LSP/editor behavior change in the ledger command.
- No preview-language promotion into gates, RIPR Zero, or public badge truth.
- No hiding or deleting raw findings.
- No score or confidence threshold gate.
- No replacement of `RIPR-SPEC-0045`; the ledger consumes aligned evidence.

## Acceptance Examples

Repairable PR-local boundary gap:

- Given a Rust PR-local changed predicate and a related test that reaches the
  owner but lacks the equality-boundary assertion, the ledger emits one
  `GapRecord`.
- `kind = "MissingBoundaryAssertion"`.
- `scope = "pr_local"`.
- `repairability = "repairable"`.
- `repair_route.route_kind = "AddBoundaryAssertion"`.
- `projection_eligibility.pr_comment.eligible = true` when anchor, dedupe, and
  verification command are present.
- `projection_eligibility.gate_candidate.eligible = true` only when configured
  policy targets the gap and all safe gate predicate conditions hold.

Static-unknown-only evidence:

- Given raw evidence with only `static_unknown` and no bounded repair route,
  the ledger preserves the source evidence.
- It emits `repairability = "analyzer_limitation"` or `unknown`.
- It is not PR-comment eligible.
- It is not gate-candidate eligible.
- The recommended next action is inspection or analyzer repair, not a generic
  user test instruction.

Baseline-known gap:

- Given a repairable Rust gap that matches reviewed baseline debt, the ledger
  preserves `gap_state = "actionable"` and sets
  `policy_state = "baseline_known"`.
- It may appear in reports and repo debt summaries.
- It is not a new PR-local gate candidate.

Waived or suppressed gap:

- Given an otherwise repairable gap with a waiver or suppression artifact, the
  ledger carries the waiver or suppression state.
- It does not count toward `ripr 0`.
- It does not become a blocking gate candidate.
- Projection surfaces may show the state without hiding the underlying
  evidence.

Presentation/output text gap:

- Given supported presentation-text evidence showing user-visible output with
  no output observer, the ledger emits `kind = "MissingOutputContract"`.
- The repair route is `AddOutputGolden`, `AddHelpOutputSnapshot`, or the most
  specific supported observer route.
- PR comments and agent packets show output-proof language, not
  `static_unknown` instructions.

Preview-language evidence:

- Given TypeScript or Python preview evidence, the ledger may include the
  advisory record with preview labels.
- It is not eligible for `ripr 0`, `ripr+`, or default gate authority.
- Generated CI and editor surfaces preserve preview/advisory labels.

RIPR Zero target:

- Given repo-scoped Rust gaps after applying waiver, suppression, and policy
  targets, the `ripr 0` input count includes only unresolved, unsuppressed,
  unwaived, policy-targeted Rust gaps.
- It does not count PR-local preview evidence or missing optional artifacts.

Receipt movement:

- Given a before/after receipt for a selected gap, the ledger ties the receipt
  path to the same `gap_id`.
- If the missing discriminator is now observed, movement is `improved` or
  `resolved`.
- If the same gap remains, movement is `unchanged_after_attempt` with the next
  repair route still visible.

Missing artifacts:

- Given a missing first useful action, baseline delta, receipt, or policy input
  needed for a decision, the ledger emits a warning and a regeneration command
  when known.
- Missing input is not treated as clean, waived, suppressed, improved, or
  runtime-confirmed.

## Test Mapping

Follow-up implementation should include:

- fixture corpus for repairable Rust boundary, error, value, side-effect, and
  output-contract gaps;
- fixture cases for baseline-known, waived, suppressed, acknowledged, resolved,
  reintroduced, missing-artifact, preview-language, and static-limitation
  states;
- JSON and Markdown golden outputs for the ledger;
- unit tests for safe gate predicate truth table;
- unit tests for `ripr 0` and `ripr+` target input counts;
- PR comment projection tests proving repair-card eligibility and dedupe;
- LSP/agent packet tests proving shared `gap_id`, repair route, and stop
  conditions;
- receipt tests proving improved, unchanged-after-attempt, resolved, and
  missing-receipt movement;
- traceability tests proving source artifacts link to gap records.

This spec PR does not add production code or public output fields.

## Implementation Mapping

Planned slices:

- `docs/spec-gap-decision-ledger` defines this behavior contract.
- `fixtures/gap-decision-ledger-corpus` pins the record vocabulary and edge
  cases.
- `output/gap-record-model-and-ledger` adds the internal model and public
  JSON/Markdown producer.
- `report/first-useful-action-gap-record` routes first useful action through
  gap IDs and repair routes. (done)
- `report/pr-evidence-ledger-gap-record` routes PR evidence ledger movement
  through the same gap IDs. (done)
- `policy/ripr-zero-and-badges-gap-targets` makes repo badge targets consume
  policy-backed gap records.
- `policy/gate-repairable-gap-predicate` makes optional gates consume the safe
  predicate without changing default blocking.
- `comments/repair-card-projection` makes inline comments render only eligible
  repair cards.
- `editor/gap-work-packet-projection` makes editor and agent packets consume
  the same gap decisions.
- `analysis/presentation-output-contract-gap-route` connects
  `RIPR-SPEC-0043` evidence to `MissingOutputContract` decisions.

Expected implementation files, once the code slice starts:

- `crates/ripr/src/output/gap_decision_ledger.rs`;
- `crates/ripr/src/output/mod.rs`;
- `crates/ripr/src/cli/commands.rs`;
- `docs/OUTPUT_SCHEMA.md`;
- `fixtures/gap-decision-ledger/`;
- `.ripr/traceability.toml`;
- `metrics/capabilities.toml`.

## Metrics

The ledger should expose or feed these metrics when implemented:

- `gap_decision_records_total`;
- `gap_decision_repairable_total`;
- `gap_decision_static_limitation_total`;
- `gap_decision_no_action_total`;
- `gap_decision_missing_artifact_total`;
- `gap_decision_projection_pr_comment_eligible`;
- `gap_decision_projection_gate_candidate`;
- `gap_decision_projection_agent_packet_eligible`;
- `gap_decision_ripr_zero_target_count`;
- `gap_decision_ripr_plus_target_count`;
- `gap_decision_preview_ineligible_total`;
- `gap_decision_receipt_improved_total`;
- `gap_decision_receipt_unchanged_after_attempt_total`;
- `gap_decision_missing_output_contract_total`.
