# RIPR-SPEC-0144: Temporary 0.11 Release-Control Lens

Status: accepted

Owner: release control / swarm operations

Created: 2026-07-29

Linked issues:

- [#2766](https://github.com/EffortlessMetrics/ripr-swarm/issues/2766) — bind
  0.11 work selection and merge eligibility to the live #2379 graph.
- [#2379](https://github.com/EffortlessMetrics/ripr-swarm/issues/2379) — exact
  0.11 candidate authority.

Support-tier impact:

- No product support-tier change. This is a maintainer-facing, read-only
  control-plane report and cannot strengthen analyzer, release, or merge claims.
- Reference: [support tiers](../status/SUPPORT_TIERS.md).

Policy impact:

- No release publication, source integration, version, credential, workflow,
  dependency, or secret change.

## Problem

The 0.11 release authority and its writer cutoff are currently expressed in
issue prose, while the live repository continues to receive independent work.
An open-ended session can therefore select a useful PR outside the accepted
release graph without recording whether that PR is required, deferred, or
blocked for the candidate. A green check on such a PR is not a release
disposition.

## Behavior

`cargo xtask release-control --input <snapshot.json>` replays a captured,
schema-versioned snapshot. `cargo xtask release-control --live` collects the
current `origin/main`, open PR inventory, and #2379 state through bounded
read-only adapters. Both paths produce a snapshot containing the observed
`main` SHA, #2379 authority, portfolio and active-claim completeness, and every
open PR. The command sorts PR records by number and derives both JSON and
Markdown from one normalized report.

Every PR must carry one of these dispositions:

- `release_required`;
- `release_optional_pending_decision`;
- `hold_post_release`;
- `blocked_on_named_authority`.

Only a complete, current snapshot can produce `status = ready`, and only a
non-draft `release_required` row can be `merge_eligible = true`. Missing,
stale, contradictory, duplicated, or malformed authority input produces
`status = reconcile_required` and clears merge eligibility for every row.
Missing per-PR disposition defaults visibly to
`blocked_on_named_authority` while retaining the reconciliation reason.

The report is advisory and report-only. It does not close issues, relabel
items, merge or rebase PRs, create or delete branches, select a candidate,
qualify a release, or mutate development `main`.

## Candidate-relative hard-cut boundary

The open-PR inventory and each row's `merge_eligible` value are work-selection
and ownership observations. Neither is a repository-wide candidate-readiness
gate. In particular, the report must not require the open release-PR count to
reach zero before a candidate can be selected.

Candidate readiness is evaluated against a selected development cut `C`, a
selected claim set `S`, candidate-only exclusions `E`, and a reproducible
candidate tree `T = project(C, E)`. The hard-cut predicate is:

```text
candidate_required_claims_pending == 0
```

That predicate requires every selected claim to be landed by `C`, explicitly
excluded from `T`, or explicitly deferred with a truthful release non-claim;
no known unresolved defect may invalidate the selected claims; every commit
through `C` must have a reviewed disposition and candidate-tree state; and the
projection from `C` to `T` must be reproducible. Commits and PRs outside `S`
may remain open or land after `C` without affecting this candidate. They are
relevant only if they disclose a defect that invalidates `T`.

The candidate control vocabulary is
`selected_candidate_claims`, `candidate_required_claims_pending`,
`candidate_claims_landed`, `candidate_claims_excluded`,
`candidate_claims_deferred`, `candidate_defects_unresolved`,
`denominator_decisions_remaining` (the schema-0.1 provisional-cutoff field),
`denominator_decisions_remaining_through_selected_cut`, `candidate_cut_selected`, and
`candidate_ref_created`. An informational `open_release_pr_count` must not be
used as a readiness predicate.

The release-control snapshot may carry an optional `candidate_selection` DTO.
When it is absent, candidate state is `scope_pending`; the ordinary PR lens
remains replayable for disposition work, but it cannot imply candidate
readiness. The DTO is the #2766 authority for the selected claim set:

```text
CandidateSelection
  schema_version
  selected_cut_sha
  selected_claims[]
  candidate_exclusions[]
  known_candidate_defects[]
  denominator_decisions_remaining_through_provisional_cutoff
  denominator_decisions_remaining_through_selected_cut
  projection
  qualification
```

Each selected claim carries `claim_id`, `owner_issue`,
`required_for_candidate`, one resolution (`pending`, `landed`,
`accepted_defer`, `candidate_exclusion`, or `failed`), evidence/commit/artifact
references, `candidate_effect`, an explicit `non_claim` when deferred or
excluded, and `reviewed`. Generic issue references or an `acceptance_owner`
field cannot establish selected-claim satisfaction.

The staged candidate states are fail-closed and ordered:

```text
scope_pending
  → scope_closed
  → hard_cut_eligible
  → candidate_materialized
  → qualification_eligible
```

`scope_closed` requires a non-empty, unique, reviewed claim set with a current
resolution and explicit non-claims for defers/exclusions. `hard_cut_eligible`
also requires zero required claims pending, zero unresolved candidate defects,
zero denominator decisions through `C`, a selected `C`, and a reproducible
projection. `candidate_materialized` additionally requires a candidate tree
whose parent is `C` and matching exclusion/preservation digests.
`qualification_eligible` additionally requires an immutable candidate ref, a
manifest naming the materialized tree, and available qualification instruments.
The immutable ref must use the repository-controlled
`refs/ripr/candidate-<identifier>` namespace; mutable branch refs such as
`refs/heads/main`, blank references, and whitespace-only values are not
qualification evidence. The immutable ref is intentionally not required for
hard-cut eligibility.

## Input contract

The input envelope has `schema_version = "0.1"` and
`kind = "release_control_snapshot"`. Captured input must use
`source.mode = "captured"`; `source.mode = "live"` is admitted only for the
internal `--live` collector, not for an input file. `source` must identify the
current main SHA, open #2379 state, matching authority/main identity, complete
portfolio and claim inventory observations, worktree inventory, current
freshness, and a non-empty graph digest. The live collector records worktree
inventory but deliberately leaves authority-main identity, portfolio
completeness, and active-claim completeness unresolved because its bounded
inputs do not prove those facts; the resulting report is therefore
`reconcile_required` until an approved source supplies them. PR rows carry a
number, title, open state, head SHA, `main` base ref, and explicit
disposition/reason.
An optional `candidate_selection` object carries the #2766 selected-claim
authority and candidate-state inputs described above; its absence is
`scope_pending`, not a successful empty selection.

The fixture corpus in `fixtures/release_control/` is manifest-only and is
validated by `cargo xtask check-fixture-contracts`. It includes a complete
snapshot and a stale/incomplete snapshot that must reconcile.

## Output contract

The command writes `target/ripr/reports/release-control.json` and
`target/ripr/reports/release-control.md`. JSON is schema `0.1` and contains the
normalized source observation, sorted PR rows, `reconciliation_reasons`, a
`status` of `ready` or `reconcile_required`, per-row `merge_eligible`, a
`candidate_state`, `next_action`, and the explicit
`authority_boundary`/`must_not_claim` fields. `candidate_state` is a staged
control signal and never changes the report's non-qualification boundary.
Markdown is a projection of that same normalized DTO; it cannot strengthen a
reconciliation-required state or any per-PR disposition.

## Acceptance

- fixed captured inputs produce byte-stable normalized JSON and Markdown;
- PR input order cannot change the normalized report;
- unrelated work remains visible and non-mergeable;
- live collection inventories current main/open PR/#2379 inputs;
- missing or stale authority never becomes merge eligibility;
- the report preserves an explicit claim boundary and next action;
- the command performs no external state mutation.

## Required Evidence

- `xtask/src/reports/release_control.rs` owns the captured-input schema,
  bounded live collectors, deterministic normalization, shared JSON/Markdown
  projection, and fail-closed merge eligibility.
- `xtask/src/reports/candidate_control.rs` owns the selected-claim DTO,
  candidate-state transitions, and fail-closed false-ready checks.
- `fixtures/release_control/complete.json` proves a complete current snapshot
  with both required and held rows; `fixtures/release_control/reconcile-required.json`
  proves stale authority cannot produce eligibility.
- `xtask/src/command.rs` and `xtask/src/dispatch.rs` expose the report as a
  report-only command, while `xtask/src/fixture_contracts/mod.rs` validates the
  fixture corpus shape.
- `.ripr/traceability.toml` links this specification to the focused tests,
  fixtures, implementation, and report outputs.

## Non-Goals

- no singleton active-goal restoration or automatic backlog priority;
- no candidate denominator, exact-candidate qualification, package proof,
  source handoff, version bump, tag, publication, signing, or marketplace;
- no repository-wide convergence requirement or open-PR-zero gate;
- no issue closure, merge queue, branch operation, or GitHub mutation;
- no replacement for #2379, #1609, #1704, or #1706.

## Acceptance Examples

### Complete captured input is deterministic

```text
Given a current captured snapshot with complete authority and explicit PR
dispositions,
when `cargo xtask release-control --input` replays it,
then the JSON and Markdown reports are normalized in PR-number order and only
non-draft `release_required` rows are merge-eligible.
```

### Stale authority fails closed

```text
Given a snapshot whose authority main SHA or completeness fields are stale,
when the snapshot is normalized,
then the report is `reconcile_required` and every PR remains non-mergeable.
```

### Live collection discloses missing authority

```text
Given the bounded live collector can observe main, open PRs, and #2379,
when portfolio or active-claim authority is not supplied by that collector,
then the report remains `reconcile_required` and names the missing inputs.
```

### An over-bound open-PR inventory fails closed

```text
Given the live open-PR inventory reaches its bounded collection limit,
when the sentinel row shows that more rows exist,
then the inventory is marked incomplete and no row can become merge-eligible.
```

## Test Mapping

- `xtask/src/reports/release_control.rs::tests::complete_snapshot_is_ready_and_only_required_rows_are_merge_eligible`
- `xtask/src/reports/release_control.rs::tests::missing_candidate_selection_is_exposed_as_scope_pending`
  — absent candidate selection is reported as `scope_pending`.
- `xtask/src/reports/release_control.rs::tests::missing_disposition_fails_closed`
  — missing PR authority clears all eligibility.
- `xtask/src/reports/release_control.rs::tests::input_order_does_not_change_normalized_output`
  — JSON and Markdown are stable under input reordering.
- `xtask/src/reports/release_control.rs::tests::stale_authority_cannot_be_merge_eligible`
  — stale source identity cannot produce eligibility.
- `xtask/src/reports/release_control.rs::tests::unsupported_live_mode_cannot_be_merge_eligible`
  — captured replay cannot impersonate the live collector.
- `xtask/src/reports/release_control.rs::tests::collector_error_fails_closed_and_clears_eligibility`
  — collector failures remain visible and non-mergeable.
- `xtask/src/reports/release_control.rs::tests::non_main_base_cannot_be_merge_eligible`
  — PRs targeting a non-release base are rejected.
- `xtask/src/reports/release_control.rs::tests::bounded_live_collector_normalizes_success_and_failure_inputs`
  — live command outputs and bounded failures are normalized explicitly.
- `xtask/src/reports/release_control.rs::tests::live_open_pr_bound_is_disclosed_and_fails_closed`
  — the sentinel row prevents a truncated open-PR inventory from appearing
  complete.

## Implementation Mapping

- `xtask/src/reports/release_control.rs` — snapshot types, live `git`/`gh`
  adapters, validation, disposition normalization, and report renderers.
- `xtask/src/command.rs` — command parsing and report-only command catalog
  entries.
- `xtask/src/dispatch.rs` — dispatch to the release-control report.
- `xtask/src/fixture_contracts/mod.rs` — release-control fixture contract
  validation.
- `fixtures/release_control/` — complete and reconcile-required captured
  inputs plus their fixture specification.
- `docs/OUTPUT_SCHEMA.md` — JSON/Markdown output shape and claim boundary.

## Metrics

- Focused release-control tests cover captured normalization, stale and
  malformed inputs, bounded live collection, and output escaping.
- `cargo xtask check-fixture-contracts` and `cargo xtask check-output-contracts`
  validate the fixture and report contracts.
- The report is advisory and does not publish a readiness, merge, or release
  metric; no product support-tier metric changes in this slice.

## Proof

```text
cargo test -p xtask release_control -- --nocapture
cargo xtask release-control --input fixtures/release_control/complete.json
cargo xtask release-control --live
cargo xtask check-output-contracts
cargo xtask check-fixture-contracts
cargo xtask check-pr
```

## Claim boundary

This spec proves only that a captured input or bounded live observation is
normalized into an explicit, deterministic, fail-closed disposition report. It
does not prove that a PR is correct, that a release candidate is qualified, or
that a merge is approved.
