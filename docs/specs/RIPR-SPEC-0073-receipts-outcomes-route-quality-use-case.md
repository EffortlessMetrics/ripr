# RIPR-SPEC-0073: Receipts, Outcomes, and Route Quality Use Case

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

- None. This spec writes the use-case contract over existing receipt,
  outcome, and ledger mechanisms; it promotes no language, surface,
  or evidence class to a stronger support tier.
- Claim boundaries for this surface are governed by the canonical ledger in [support tiers](../status/SUPPORT_TIERS.md); nothing here promotes a tier.

Policy impact:

- Register this spec in `policy/doc-artifacts.toml`.
- No new crates, binaries, dependencies, parsers, runtime executors,
  or LSP servers.

## Problem

`ripr` already records what happened after a repair attempt: agent
receipts snapshot a seam before and after a focused change, the
targeted-test outcome report buckets seams into moved, unchanged,
regressed, new, and removed with per-stage deltas, swarm ingest
classifies external agent results, and the gap decision ledger counts
receipt movement. What is missing is the product contract that turns
those records into a learning loop: a closed outcome vocabulary every
surface shares, and route-quality metrics that say which repair kinds
and limitation routes actually move evidence.

This is the difference between a report generator and a learning
loop. A report generator emits findings and forgets. A learning loop
attributes every attempt to an outcome, refuses to count claims
without receipts, and accumulates which routes earn their keep —
so the next packet routes better than the last.

The standing doctrine this spec writes down: external agent claims
are never trusted without receipt plus verify evidence. Swarm ingest
hard-codes `safety.trusted_success = false` and
`safety.requires_human_review = true` on every ingested result.

## Behavior

The user question this use case answers:

```text
Did the attempted repair actually improve evidence?
```

### Source artifacts (existing)

- Agent receipt (`crates/ripr/src/output/agent_receipt.rs`,
  `AGENT_RECEIPT_SCHEMA_VERSION = "0.3"`): per-seam before/after
  snapshots (`seam.before`, `seam.after`, `grip_class`, `change`,
  `evidence_delta`), sha256 artifact provenance,
  `verification.commands_run`, and a summary carrying
  `receipt_state`, `remaining_gap`, `next_recommendation`, and
  `next_action {kind, summary, recommended_action, safe_to_merge}`.
- Receipt lifecycle (`crates/ripr/src/output/receipt_lifecycle.rs`):
  the states `receipt_missing`, `receipt_found`, `receipt_stale`,
  `receipt_gap_mismatch`, `receipt_movement_improved`,
  `receipt_movement_unchanged`, and `receipt_not_applicable`.
- Targeted test outcome (`crates/ripr/src/output/outcome/`,
  `TARGETED_TEST_OUTCOME_SCHEMA_VERSION = "0.1"`): seams bucketed
  into `moved[]`, `unchanged[]`, `regressed[]`, `new[]`, and
  `removed[]`, each movement carrying stage deltas across `reach`,
  `activate`, `propagate`, `observe`, and `discriminate`.
- Swarm ingest (`crates/ripr/src/output/swarm_ingest.rs`,
  `SWARM_INGEST_SCHEMA_VERSION = "0.1"`): `attempt_outcome`,
  classification `gap_id` / `canonical_gap_id`, evidence
  `agent_status` and `edited_files[]`, `verify {present, status,
  exit_code, passed, failed}`, `receipt {present, path, movement}`,
  and safety `forbidden_edit_flagged`, `requires_human_review`,
  and `trusted_success` (always `false`).
- Gap decision ledger
  (`crates/ripr/src/output/gap_decision_ledger.rs`): `GapRecord`
  receipt fields (`receipt_command`, `receipt {state, movement,
  path}`) and the summary counts `receipt_improved_total` and
  `receipt_unchanged_after_attempt_total`.
- Evidence quality roll-ups: `cargo xtask
  evidence-quality-scorecard` and `cargo xtask
  evidence-quality-trend`, writing
  `target/ripr/reports/evidence-quality-scorecard.{json,md}` and
  `target/ripr/reports/evidence-quality-trend.{json,md}`.

### Closed outcome vocabulary

Every surface that reports an attempt outcome draws from this
vocabulary and no other:

| Outcome | Source state today | Status |
| --- | --- | --- |
| `not_attempted` | ledger-derived: gap with no attempt record | planned addition |
| `attempted_no_receipt` | swarm ingest `attempt_outcome` | existing |
| `receipt_present` | swarm ingest `attempt_outcome` | existing |
| `evidence_improved` | swarm ingest; `receipt_movement_improved` | existing |
| `evidence_unchanged` | swarm ingest; `receipt_movement_unchanged` | existing |
| `evidence_regressed` | swarm ingest `attempt_outcome` | existing |
| `resolved` | swarm ingest `attempt_outcome` | existing |
| `unknown` | swarm ingest `attempt_outcome` | existing |
| `orphan_receipt` | receipt on disk with no matching current gap | planned addition |
| `receipt_stale` | `receipt_stale` lifecycle state | planned addition |
| `receipt_gap_mismatch` | `receipt_gap_mismatch` lifecycle state | planned addition |

`receipt_missing`, `receipt_found`, and `receipt_not_applicable`
remain the receipt-presence axis underneath this vocabulary; they
feed it but are not attempt outcomes themselves. The four planned
additions (`not_attempted`, `orphan_receipt`, `receipt_stale`,
`receipt_gap_mismatch`) require schema and fixture updates before
any surface emits them as attempt outcomes. The `receipt_stale` and
`receipt_gap_mismatch` strings already exist as lifecycle states in
`crates/ripr/src/output/receipt_lifecycle.rs`, but no surface emits
them as an attempt outcome today — swarm ingest classifies a stale
packet as outcome `unknown` — so promoting them to attempt outcomes
is the same gated work as the other planned rows. (`not_attempted`
keeps the outcome string already used by the `cargo xtask
actionable-gap-outcomes` rows of RIPR-SPEC-0031 and the swarm
attempt ledger of RIPR-SPEC-0057.)

Attempt-outcome strings must never be routed through
`normalize_receipt_lifecycle_state`
(`crates/ripr/src/output/receipt_lifecycle.rs`); that normalizer
serves the receipt-presence axis only. It rewrites the legacy input
aliases `stale_receipt` / `gap_mismatch` into `receipt_stale` /
`receipt_gap_mismatch` — which is why this vocabulary uses the
canonical `receipt_*` forms rather than the aliases — and it
rewrites `not_attempted` into `receipt_not_applicable`, so applying
it to an attempt outcome would silently produce a string outside
this closed vocabulary.

### Outcome resolution rules

1. A forbidden-file edit forces `unknown` with
   `forbidden_edit_flagged = true`, regardless of claimed success.
2. Missing or failing verify evidence caps the outcome at
   `receipt_present` / `attempted_no_receipt`; it never reaches
   `evidence_improved` or `resolved`.
3. Verify passed plus receipt movement decides the rest: improved
   maps to `evidence_improved`, unchanged to `evidence_unchanged`,
   regressed to `evidence_regressed`, closed to `resolved`.
4. An unrecognized movement string resolves to the presence-based
   outcome, never to an improvement.
5. Receipt movement is valid only with before/after snapshot
   provenance (the receipt's sha256 artifact fields). Planned
   enforcement: rules 1–4 match current `classify_swarm_result`
   behavior, but swarm-ingest classification today reads
   `receipt.provenance.movement` without validating snapshot
   provenance; the agent-receipt artifact carries sha256 provenance
   by construction, and the ingest-side validation is a named
   implementation slice in the linked plan.

### Required and forbidden wording

- Required: "evidence improved" — Forbidden: "repair confirmed"
  for any attempt without receipt plus verify evidence.
- Required: "verify passed; receipt movement unchanged" —
  Forbidden: "attempt succeeded" when evidence did not move.
- Required: "attempt outcome unknown" — Forbidden: omitting the
  outcome when classification cannot resolve it.
- Required: "agent-claimed, not verified" for unverified claims —
  Forbidden: any rendering of `trusted_success = true`.

### Route-quality metrics (planned)

These metrics extend the evidence-quality scorecard and trend
artifacts (`evidence-quality-scorecard.{json,md}` and
`evidence-quality-trend.{json,md}`); they are named here so the
schema work lands against a contract:

- `repair_kind_attempted` — attempts per repair kind.
- `repair_kind_improved` — attempts per kind ending
  `evidence_improved` or `resolved`.
- `repair_kind_unchanged` — attempts per kind ending
  `evidence_unchanged`.
- `repair_kind_regressed` — attempts per kind ending
  `evidence_regressed`.
- `repair_kind_success_rate` — improved over attempted, per kind,
  receipt-backed only.
- `limitation_repair_route_sharpened` — limitation repair routes
  (the `repair_route` field attached to a named limitation) whose
  follow-up produced a narrower or better-named limitation.
- `limitation_repair_route_promoted` — limitation repair routes
  whose follow-up produced actionable evidence.
- `top_failing_routes` — routes with the highest unchanged plus
  regressed share.
- `top_missing_fields` — the actionability fields most often
  missing across attempts.

### Non-claims

- An `evidence_improved` outcome is a static-evidence movement claim
  only; it is not a runtime mutation result and not a correctness
  guarantee for the production change.
- `resolved` means the gap left the after snapshot; it does not claim
  the test suite is complete.
- Route-quality rates rank routing choices; they do not grade agents
  or authors.

## Non-Goals

- No analyzer behavior changes in this spec; docs and contract only.
- No runtime mutation execution and no test generation.
- No provider integration and no autonomous retry loop.
- No trust elevation for external agents: `trusted_success` stays
  hard-coded `false` in swarm ingest.
- No second tracker: outcome accumulation lives in the ledger and
  scorecard artifacts, not a new database.

## Required Evidence

Existing evidence this contract builds on:

- Agent receipt rendering and guidance tests in
  `crates/ripr/src/output/agent_receipt.rs`.
- Receipt lifecycle normalization tests in
  `crates/ripr/src/output/receipt_lifecycle.rs`.
- Outcome bucketing and stage-delta tests in
  `crates/ripr/src/output/outcome/`.
- Swarm ingest classification tests in
  `crates/ripr/src/output/swarm_ingest.rs`, including the
  forbidden-edit and `trusted_success = false` assertions.
- Ledger receipt summary tests in
  `crates/ripr/src/output/gap_decision_ledger.rs`
  (`receipt_improved_total`, `receipt_unchanged_after_attempt_total`).

Fail-closed reject list — states the surfaces must refuse to render
as success:

- An agent-claimed success without verify evidence rendered as
  anything other than an uncertain presence-based outcome.
- A receipt-absent attempt counted toward `evidence_improved`,
  `resolved`, or any route-quality success numerator.
- Receipt movement emitted without before/after snapshot provenance
  (planned enforcement; see outcome resolution rule 5 — swarm-ingest
  classification does not yet validate snapshot provenance).
- An unrecognized movement string mapped to an improvement outcome.
- Any rendering of `trusted_success = true`.
- A forbidden-file edit that does not flag
  `forbidden_edit_flagged = true` and force `unknown`.
- Route-quality counts derived from outcomes outside the closed
  vocabulary or lacking receipt provenance.
- An attempt outcome string outside the closed vocabulary above.

## Acceptance Examples

### Improved attempt

An agent edits a test file inside the allowed surface, runs the
packet verify command (recorded in `verification.commands_run`), and
produces a receipt whose seam moved to a higher grip class. Swarm
ingest classifies `attempt_outcome = "evidence_improved"`; the ledger
increments `receipt_improved_total`; the scorecard's planned
`repair_kind_improved` counts the repair kind. `safe_to_merge` stays
`false` pending human review.

### Verified but unchanged

Verify passes but the receipt reports
`receipt_movement_unchanged`. The outcome is `evidence_unchanged`,
the ledger increments `receipt_unchanged_after_attempt_total`, and
the next action is to strengthen the named discriminator — not to
celebrate a passing command.

### Claimed success, no receipt

An external agent reports success with no receipt and no verify
output. The outcome is `attempted_no_receipt`, the next action asks
for the verify run and a before/after receipt, and nothing in the
ledger or scorecard moves toward improvement.

### Forbidden edit

The result names an edited file on the packet's forbidden list.
Classification is `edited_forbidden_file` with
`attempt_outcome = "unknown"`, `forbidden_edit_flagged = true`, and
`requires_human_review = true`; the attempt is excluded from every
route-quality success count.

### Orphan receipt (planned)

A receipt exists on disk but no current gap matches its seam. The
planned `orphan_receipt` outcome surfaces it for cleanup instead of
silently dropping it or matching it to the wrong gap.

## Test Mapping

- None yet. This spec is a use-case contract over mechanisms that
  already carry focused tests (agent receipt, receipt lifecycle,
  targeted test outcome, swarm ingest, gap decision ledger).
  Traceability entries are added when the outcome-vocabulary and
  route-metrics implementation slices land.

## Implementation Mapping

- docs/specs/RIPR-SPEC-0073-receipts-outcomes-route-quality-use-case.md
  — this document.
- plans/use-case-specs/implementation-plan.md (planned) — the
  "receipt outcome quality" slice: emit the closed outcome
  vocabulary end to end (swarm ingest → gap decision ledger →
  scorecard), including the planned `not_attempted`,
  `orphan_receipt`, `receipt_stale`, and `receipt_gap_mismatch`
  attempt-outcome states.
- plans/use-case-specs/implementation-plan.md (planned) — the
  "route metrics" slice: extend the evidence-quality scorecard and
  trend schemas with the route-quality metrics named above, fed only
  by receipt-backed outcomes.

## Metrics

- Outcome coverage: fraction of recorded attempts carrying an
  outcome from the closed vocabulary (target: 100%).
- Receipt backing: fraction of improvement-counted outcomes with
  receipt plus verify provenance (target: 100% by construction).
- Ledger movement counts: `receipt_improved_total` and
  `receipt_unchanged_after_attempt_total` trend over time via the
  evidence-quality trend artifact.
- Planned route-quality metrics (scorecard/trend extension):
  `repair_kind_attempted`, `repair_kind_improved`,
  `repair_kind_unchanged`, `repair_kind_regressed`,
  `repair_kind_success_rate`, `limitation_repair_route_sharpened`,
  `limitation_repair_route_promoted`, `top_failing_routes`,
  `top_missing_fields`.
- Promotion rule: move this spec to `accepted` when the closed
  outcome vocabulary is emitted end to end across ingest, ledger,
  and scorecard, the route-quality metrics land in the scorecard and
  trend schemas, and fixtures cover every outcome in the closed
  vocabulary including the fail-closed reject cases.

## Failure Modes

- A surface invents an outcome string — a closed-vocabulary
  violation caught by output-contract checks and fixtures.
- An unverified claim leaks into an improvement count — the reject
  list makes this a named defect, and `trusted_success = false`
  keeps the doctrine machine-visible.
- Receipts accumulate without gap matches — the planned
  `orphan_receipt` state keeps them visible instead of silently
  skewing route metrics.
- Route metrics degenerate into agent scoreboards — the non-claims
  section bounds interpretation to routing quality; misuse is a
  process defect against this spec.
