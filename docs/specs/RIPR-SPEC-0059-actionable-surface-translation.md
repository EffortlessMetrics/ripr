# RIPR-SPEC-0059: Actionable Surface Translation

Status: accepted

Linked proposal: [RIPR-PROP-0016](../proposals/RIPR-PROP-0016-actionable-surface-translation.md)

Linked plan: [RIPR-PLAN-0059](../../plans/actionable-surface-translation/implementation-plan.md)

## Problem

RIPR public projection now uses a more correct unit (`canonical_actionable_gap`),
but multiple user-facing surfaces still require too much prior context to act
safely.

A headline count like `184` is now plausible as repo repair queue size, but a
user still has to ask:

- what this number counts;
- what changed in the current PR;
- what to do in the current file;
- what packet is safe to hand to an external agent;
- what confirms that a repair attempt changed outcomes.

Without a strict translation contract between badge, PR, editor, swarm, and
outcomes surfaces, RIPR risks drifting back to report-volume presentation
instead of repair-loop guidance.

## Goal

Define a cross-surface translation contract so every first-screen surface answers
one top-level user question using the same actionable unit and explicit
non-claims.

## Non-Goals

- Changing badge count basis away from `canonical_actionable_gap`.
- Making new blocking CI gates by default.
- Expanding static findings vocabulary beyond existing conservative language.
- Turning editor or swarm surfaces into mutation proof or merge-readiness proof.

## Behavior

User-facing and agent-facing start-here surfaces translate existing RIPR
evidence into one actionable first-screen question. Each surface keeps raw
findings, seam-native inventory, and static pressure gauges below the primary
repair unit unless the surface is explicitly labeled as internal diagnostics.

## Canonical User Questions

Each surface must lead with one question it answers first:

| Surface | First question | Primary unit |
| --- | --- | --- |
| Badge | How much actionable work remains in this repo? | unresolved actionable canonical gaps |
| PR evidence | What changed in this PR's repair queue? | actionable gap delta |
| Editor status | What should I do now in this workspace/file? | top safe actionable gap or fail-closed reason |
| Swarm attempt dry-run | What can an agent safely attempt right now? | one bounded repair packet |
| Outcome/trend report | What proved movement since last refresh? | receipt-linked outcome delta |

## Shared Unit and Terms

All surfaces in this spec must use:

- actionable canonical gap identity;
- repair route;
- related test or repair target;
- verify command;
- receipt command or state;
- advisory/static/preview boundaries;
- explicit non-claims.

Internal seam inventory and raw findings may be present only as supporting,
clearly labeled context below primary actionable sections.

## Surface Contracts

### 1) Badge and Badge-Basis Translation

Badge-adjacent user-visible copy must define the headline count at point of use:

```text
unresolved actionable static repair gaps
```

`ripr+` copy must state that additional test-efficiency items only count when
projected into the same repair/verify/receipt model.

Badge-basis output should include at least:

- public count;
- basis (`canonical_actionable_gap`);
- `ripr+` delta over base actionable count;
- separation from seam-native inventory;
- top exclusion reasons;
- last generated timestamp;
- trend versus previous refresh.

### 2) PR Evidence First-Screen

PR summary front panel should lead with actionable delta, not only repo
inventory:

- repo actionable queue count;
- PR-local actionable count;
- new actionable gaps;
- resolved actionable gaps;
- attempted-but-unchanged repairs;
- receipt missing/orphaned counts;
- blocked/static-limited counts;
- one top next repair packet with verify and receipt commands;
- non-claims.

Raw finding totals, seam inventory, or other diagnostics are secondary and
should appear below the actionable delta block.

### 3) Editor Show Status First-Screen

`Show Status` should lead with immediate repair clarity:

- workspace actionable count;
- current-file actionable/blocked/no-action counts;
- top repair item;
- related test/target;
- verify command;
- receipt command or state;
- next safe command when fail-closed.

If no safe action is available, the first-screen state must name the precise
blocking reason (for example stale artifact, wrong root, missing verify,
missing receipt path, unsupported schema) and a regeneration or setup command.

### 4) Swarm Attempt Packet Usability

`cargo xtask ripr-swarm attempt --packet <id> --dry-run` should emit a compact
copy-ready operator/agent packet section with:

- task;
- allowed files;
- do-not-change boundaries;
- repair target;
- verify command;
- receipt command;
- stop conditions;
- required return format.

This packet remains deny-by-default and must align with external-agent handoff
boundaries.

### 5) Outcome/Trend Movement

Outcome/trend front sections should answer motion since last refresh using
receipt-linked states:

- total actionable count;
- delta since prior refresh;
- resolved;
- improved;
- unchanged after attempt;
- orphaned/missing receipts;
- top blocked reason.

## Required Evidence

Implementation PRs that change a covered surface must provide typed evidence
for the surface they touch:

- the primary rendered unit is an actionable canonical gap, actionable gap
  delta, bounded repair packet, or receipt-linked outcome delta;
- badge and badge-basis surfaces name `canonical_actionable_gap` as the public
  basis and keep seam-native inventory labeled as internal diagnostics;
- PR evidence front panels name PR-local actionable deltas and do not lead with
  raw finding counts;
- editor status names the safe next action or a precise fail-closed reason;
- swarm dry-run packets include verify command, receipt command, allowed files,
  and do-not-change boundaries;
- outcome/trend reports join movement to receipt-backed states;
- every surface repeats the relevant advisory, static, preview, and non-runtime
  proof boundary.

Dogfood receipts must include a badge/LSP/PR/CI projection-alignment corpus.
The corpus starts from one canonical repair state and proves each surface keeps
the same `canonical_gap_id`, `packet_id`, `repair_kind`, `verify_command`,
`receipt_command`, and top next action. Each case must state that it consumes
canonical state, does not reinterpret raw findings, does not headline raw
finding totals, remains advisory by default, and can show limited or stale
state instead of an incorrect count.

For non-full runtime states, surface examples must fail closed: the first
screen names the limited or stale state, routes the next action to
`resolve_limited_runtime_status`, and must not headline an actionable count.
Limited-state projections use `projection_basis = canonical_runtime_status`,
name a limitation category and runtime repair command, and carry no
`canonical_gap_id`, `packet_id`, `repair_kind`, `verify_command`, or
`receipt_command`. Those packet fields belong only to full runs where a safe
actionable repair packet is ready. The limitation category and runtime repair
command must match the named runtime state; for example,
`limited_large_cache_skip` routes to
`cargo xtask cache report && cargo xtask cache gc --dry-run`, while
`limited_incomplete_input` and `limited_sampled_input` route to
`cargo xtask lane1-evidence-audit`.
For full runtime states with a ready packet, the top next action may be
`attempt_ready_packet`. Full runtime states with degraded route-quality or
non-success outcomes may instead route `improve_repair_route_quality`,
`inspect_unchanged_attempts`, or another canonical repair-loop next action.
Full runtime states with no ready public packet may project
`canonical_limitation_backlog` and route `route_static_limitation_backlog` as
an analyzer-backlog next action. That projection is advisory, preserves
non-actionable status, and must not be counted as a public repair packet or
actionable gap. Limited or stale runtime states must fail closed before this
projection: they use `canonical_runtime_status`, route
`resolve_limited_runtime_status`, and do not carry static-limitation backlog
packet identity or analyzer-backlog commands as complete current evidence.
Full actionable projection examples name a `source_alignment_case` from the
repair-loop surface projection corpus and must match that source case's
canonical gap, packet, repair kind, verify command, receipt command, and top
next action. User-facing surfaces stay thin consumers of the canonical repair
source instead of maintaining independent packet truth.

## Acceptance Examples

Given the public badge headline, a new reader can describe the count as
unresolved actionable static repair gaps without reading another spec.

Given a PR summary, a reviewer can identify the next repair action from the
front panel without reconstructing raw findings.

Given editor status for a workspace, a developer sees either one safe repair
action or one explicit fail-closed reason on the first screen.

Given a swarm-ready packet, an operator can copy one dry-run packet into an
external agent without reconstructing missing boundaries.

Given an outcome or trend report, movement can be read as receipt-linked delta,
not just static queue size.

## Test Mapping

Validation should use existing guardrails plus focused evidence checks:

- `cargo xtask pr-summary`
- `cargo xtask check-pr`
- `cargo xtask lane1-evidence-audit`
- `cargo xtask actionable-gap-outcomes`
- `cargo xtask badge-basis`
- editor and swarm focused fixture/smoke checks for first-screen rendering
- `xtask::tests::dogfood_user_surface_projection_alignment_reports_contract_drift`
  pins fail-closed user-surface behavior for limited or stale runtime states so
  the corpus cannot route stale evidence to an attempt-ready packet or headline
  an actionable count.
- `xtask::tests::dogfood_user_surface_projection_alignment_accepts_limited_runtime_status`
  pins limited runtime-state projection without packet identity.
- `xtask::tests::dogfood_user_surface_projection_alignment_rejects_wrong_runtime_repair_route`
  pins limited runtime-state categories and repair commands to their named
  status.
- `xtask::tests::dogfood_user_surface_projection_alignment_rejects_limited_static_limitation_backlog_projection`
  pins that limited or stale runtime rows route to `canonical_runtime_status`
  and cannot reuse static-limitation backlog packet identity or analyzer
  backlog next actions as complete current evidence.
- `xtask::tests::dogfood_surface_projection_alignment_covers_route_quality_non_success`
  requires a non-success route-quality source case for downstream surfaces.
- `xtask::tests::dogfood_surface_projection_alignment_covers_missing_receipt_route_quality`
  requires an attempted-no-receipt route-quality source case before downstream
  surfaces can project missing receipt state.
- `xtask::tests::dogfood_user_surface_projection_alignment_matches_route_quality_non_success_source`
  pins full user-surface rows to degraded route-quality source state without
  converting it into `attempt_ready_packet`.
- `xtask::tests::dogfood_user_surface_projection_alignment_matches_missing_receipt_source`
  pins user-surface missing receipt state to `collect_missing_attempt_receipts`
  instead of claiming improvement or another packet attempt.
- `xtask::tests::dogfood_user_surface_projection_alignment_covers_route_quality_non_success_all_surfaces`
  requires badge, LSP, PR comment, and CI examples to consume the degraded
  route-quality source state as the same canonical next action.
- `xtask::tests::dogfood_user_surface_projection_alignment_matches_surface_projection_source`
  pins full user-surface projection to the canonical repair-loop source case.

Future implementation PRs should add or update fixtures for the surface they
change. Docs-only PRs satisfy this spec by passing spec-format and doc-index
checks.

## Risks and Mitigations

- **Risk:** surfaces reintroduce raw findings as headline metrics.
  - **Mitigation:** enforce first-screen section ordering and non-claim checks in
    report writers and generated checks.
- **Risk:** same number appears with inconsistent labels across surfaces.
  - **Mitigation:** centralize shared phrasing constants for badge and front
    panels.
- **Risk:** packet verbosity blocks adoption.
  - **Mitigation:** define compact top blocks and push diagnostics below fold.

## Implementation Mapping

Existing source-of-truth surfaces that this spec coordinates:

- public badge projection and badge-basis policy from `RIPR-SPEC-0056`;
- editor actionable gap queue projection from `RIPR-SPEC-0055`;
- swarm repair-loop planning and dry-run behavior from `RIPR-SPEC-0057`;
- external-agent handoff boundaries from `RIPR-SPEC-0058`;
- outcome joins from `cargo xtask actionable-gap-outcomes`;
- generated start-here and PR evidence surfaces already documented in
  `docs/OUTPUT_SCHEMA.md`.

This spec does not add a command, schema, renderer, editor surface, CI gate, or
badge generator by itself. Those changes require separate scoped PRs with
fixtures and output-contract updates.

## Metrics

Covered surfaces should be able to report or derive:

- repo actionable queue count;
- PR-local actionable gap count;
- new and resolved actionable gaps;
- blocked/static-limited counts;
- receipt missing and orphaned counts;
- attempted-but-unchanged repairs;
- swarm-ready packet count;
- outcome deltas for improved, unchanged, regressed, and resolved states;
- excluded raw, seam-native, preview-only, or static-limited diagnostics by
  named reason.

## Rollout Plan

1. Badge presentation pass (badge-adjacent meaning + basis link visibility).
2. PR summary first-screen actionable-delta contract.
3. Editor first-screen repair-first hierarchy polish.
4. Swarm dry-run packet copy-ready formatting alignment.
5. Outcome/trend movement front section.

Each step should remain a scoped PR with spec -> tests/fixtures -> code ->
output contract -> metric traceability.
