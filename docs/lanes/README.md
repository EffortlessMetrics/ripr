# Lane Trackers

Lane trackers record lane-local implementation state. They are not the whole
product board and they do not replace proposals, specs, ADRs, capability
evidence, traceability, or closeout handoffs.

The active Codex Goals manifest at `.ripr/goals/active.toml` is an operator
manifest for the current repo-wide execution sequence. It can point at PR/CI,
editor, policy, release, or cleanup work while a lane tracker still records a
different lane's durable plan. Do not treat the active manifest as the only
source of product truth.

## Document Layers

Use one document for one job:

| Layer | Owns | RIPR storage |
| --- | --- | --- |
| Roadmap | release and product direction | `docs/ROADMAP.md` |
| Proposal / PRD | why the work exists, user value, alternatives, success criteria | `docs/proposals/` |
| Spec | behavior contract, inputs and outputs, required evidence, acceptance examples | `docs/specs/` |
| ADR | durable architecture decisions | `docs/adr/` |
| Lane tracker | lane-local state, active slices, non-goals, sequencing | `docs/lanes/` |
| Implementation plan | repo-wide sequence, campaign summaries, and lane-local PR sequencing | `docs/IMPLEMENTATION_PLAN.md`, `docs/IMPLEMENTATION_CAMPAIGNS.md`, `plans/` |
| Active goals manifest | current Codex/Droid operator sequence | `.ripr/goals/active.toml` |
| Capability matrix | maturity, scope, and proof evidence | `docs/CAPABILITY_MATRIX.md`, `metrics/capabilities.toml` |
| Traceability | spec, fixture, test, code, and metric linkage | `.ripr/traceability.toml` |
| Closeout / handoff | what landed, proof, remaining work, restart context | `docs/handoffs/` |

Do not make every document do every job. A lane tracker may link to the
proposal, spec, ADR, capability, traceability, and closeout records, but it
should not duplicate their full content.

## Lane 1 Source Of Truth

Lane 1 owns evidence truth:

- analyzer accuracy;
- evidence identity;
- `seams[].evidence_record` and related evidence structure;
- canonical gap identity;
- evidence movement;
- related-test ranking;
- oracle semantics;
- local flow and activation/value modeling;
- static limitations;
- user-visible output and presentation-text evidence classes;
- imported static/runtime calibration confidence.

Lane 1 should use these layers:

- Proposal: why evidence quality leadership matters, who benefits, what
  alternatives were rejected, and what success means.
- Spec: what a scorecard, benchmark corpus, calibration expansion, or evidence
  report must do.
- ADR: durable evidence-model or maturity-policy decisions only.
- Lane tracker: current Lane 1 slices, non-goals, validation gates, and closeout
  conditions.
- Capability matrix: class-scoped maturity and proof.
- Traceability: the spec/test/code/fixture/metric chain.
- Closeout: what landed, what proof ran, and what remains unknown.

Lane 1 should not use `.ripr/goals/active.toml` as a substitute for the lane
tracker. Update the active manifest only when the repo-wide operator sequence
explicitly makes Lane 1 active.

## Lane 1 Boundary

Lane 1 may change analyzer truth, evidence structure, identity, movement, and
calibration confidence. Downstream surfaces should consume Lane 1 truth instead
of inventing parallel interpretations.

Lane 1 does not own:

- PR or CI front-panel composition;
- PR inline comment posting;
- LSP or editor polish;
- gate-policy changes or default blocking;
- release or packaging mechanics;
- generated tests;
- source edits;
- provider or model calls;
- runtime mutation execution;
- score redefinition.

If a downstream surface exposes weak evidence, the Lane 1 repair should improve
the source evidence class first and keep projection changes narrow.

## Current Lane 1 Trackers

- [Lane 1 Evidence Spine](LANE_1_EVIDENCE_SPINE.md) records the stable v0.1
  shared evidence spine and consumer closeout.
- [Lane 1 Evidence Accuracy Evaluation](LANE_1_EVIDENCE_ACCURACY.md) records
  the closed audit-first evidence accuracy campaign.
- [Lane 1 Evidence Quality Leadership](LANE_1_EVIDENCE_QUALITY_LEADERSHIP.md)
  records the scorecard, benchmark, calibration, and audit-delta leadership
  loop.
- [Lane 1 User-Visible Output Evidence](LANE_1_USER_VISIBLE_OUTPUT_EVIDENCE.md)
  records the presentation/help/report/table text evidence-class expansion.
- [Lane 1 Finding Alignment Burn-Down](LANE_1_FINDING_ALIGNMENT_BURNDOWN.md)
  records the issue-backed queue for generalizing raw-finding to canonical-item
  alignment across remaining evidence classes.
- [Lane 1 RIPR+ Burndown](LANE_1_RIPR_PLUS_BURNDOWN.md) records the current
  repo-wide RIPR/RIPR+ measurement basis, blockers, merge order, and first
  packet-family queue.
- [Lane 1 Value Resolution Audit Fixes](LANE_1_VALUE_RESOLUTION_AUDIT_FIXES.md)
  records the active issue-backed queue for burning down one fixture-backed
  `predicate_boundary` / `activation_value_unresolved` sub-shape.
- [Lane 1 TypeScript Preview Completion](LANE_1_TYPESCRIPT_PREVIEW_COMPLETION.md)
  records the current-state audit and missing slices for completing
  TypeScript/JavaScript as an opt-in preview repair-guidance surface.

## Lane 4 Source Of Truth

Lane 4 owns PR and CI review composition:

- generated PR workflow summaries and artifact upload shape;
- PR review front panel;
- report packet index;
- repair, agent handoff, and receipt links in PR-time artifacts;
- language-aware advisory grouping when preview languages are configured;
- clear separation between advisory summaries and configured gate authority.

Lane 4 should use these layers:

- Proposal: why the PR/CI review cockpit matters to reviewers, maintainers,
  and coding agents.
- Spec: what generated CI, front-panel, packet-index, repair-command, receipt,
  grouping, and authority-boundary behavior must do.
- ADR: durable workflow or architecture decisions only.
- Lane tracker: current Lane 4 surfaces, non-goals, validation gates, and
  operating rule.
- Plan: PR-sized sequence under `plans/lane4-pr-ci-review-cockpit/`.
- Policy ledgers: gate, workflow, exception, and allowlist authority.
- Closeout: what shipped, what proof ran, and what remains unknown.

Lane 4 should not use generated summaries or indexes as policy authority. Gate
decisions remain the configured pass/fail authority.

## Current Lane 4 Tracker

- [Lane 4 PR / CI Review Cockpit](LANE_4_PR_CI_REVIEW.md) records the PR/CI
  review cockpit source-of-truth model and links to the lane-local plan.
