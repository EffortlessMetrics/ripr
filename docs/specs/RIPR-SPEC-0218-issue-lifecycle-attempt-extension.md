# RIPR-SPEC-0218: Issue lifecycle attempt extension contract and scorecard projection

Status: proposed

Owner: test-infra

Created: 2026-10-06

Linked issues:

- #4929 (this slice: the issue-lifecycle extension contract, dispositions,
  mechanics fixtures and scorecard projection)
- #1650 (parent/controller: governed issue-to-closeout acceptance)
- #1803 (closed oversized slice; this spec replaces its schema, fixture and
  report portions)
- #4925 (base orchestration attempt receipts; referenced, not duplicated)
- #4930/#4931/#4932/#4933 (issue-lifecycle children; designed-for here,
  implemented there)

Support-tier impact:

- None. The validator is an offline typed projection over committed fixture
  rows; it launches no agent, selects no work, edits nothing, calls no
  provider, reads no live GitHub state and collects no telemetry.
  [docs/status/SUPPORT_TIERS.md](../status/SUPPORT_TIERS.md)

Policy impact:

- None. No new process, network or file-policy surface; the report reads the
  committed corpus and renders to stdout/`target/ripr/reports` only.

## Problem

The issue-lifecycle family (parent #1650) cannot count a single issue
journey honestly: there is no versioned row that binds one issue's
snapshot, intake, contract decision, plan, claim events, burn-down and
closeout evidence to the base orchestration attempts that carried its
execution and verification, and no deterministic projection that decides
whether a claimed disposition — from `qualified_one_pr` to `completed` — is
supported by its own evidence. Without that contract, later slices would
either count issues that lack exact identity or current-head proof, or
silently drop duplicate, already-satisfied, blocked and closed-not-planned
issues from view — and a merged PR could masquerade as a completed closeout.
The #4925 attempt authority already owns context, execution, verification
and cleanup facts; duplicating them into a parallel task store would fork
the authority this family chains on.

## Behavior

`cargo xtask issue-lifecycle-scorecard [--captured <corpus.json>]` (alias
`--corpus`) loads one committed corpus of typed lifecycle rows
(`issue_lifecycle_attempt.v1`), runs every row through the fail-closed
counting-law validator, evaluates every committed fixture expectation
against the live outcome, and projects one `IssueLifecycleScorecardV1` DTO
to `target/ripr/reports/issue-lifecycle-scorecard.{json,md}`. Both
projections derive from the same assessed rows; Markdown can never
strengthen machine state. The gate fails closed when a scenario's live
outcome drifts from its committed expectation, when a required scenario is
missing, or when the corpus is unparseable.

The closed disposition vocabulary (`IssueLifecycleDispositionV1`):
`needs_evidence`, `qualified_one_pr`, `qualified_spec_required`,
`root_decision_required`, `duplicate`, `already_satisfied`, `superseded`,
`blocked`, `partially_landed`, `verification_failed`,
`merged_pending_closeout`, `completed`, `closed_not_planned`, `stale`,
`not_run`. Issue state is a projection over evidence: only `completed` is
the positive terminal and only it counts as implementation success; a label,
comment, merge or confident agent summary alone is never authority.

Each row retains at least: the GitHub issue snapshot identity plus its
comments/labels/assignees/milestone identities; the current main, relevant
PRs, portfolio, selected work and claim identities; the initial information
completeness and issue family; intake evidence references and the exact
missing-evidence questions; the spec-required decision with its rationale
and root disposition; proposal/spec/ADR draft, challenge, amendment and
acceptance identities; plan, work-item, dependency and acceptance-coverage
identities; claim/collision/expiry/takeover events; PR/review/check and
verification identities plus the current main the verification receipt was
bound against; the progress mutation plan with before/after digests and
unchanged suppression; burn-down state with uncovered, contradicted and
deferred rows; closeout state, reason, remaining limitations and the
current-head verification identity; limitations, non-claims and a
producer-recorded `row_digest` over the canonical retained surface.

Shared context, execution, verification and cleanup facts are referenced,
not copied: `base_attempts` binds one or more #4925
`OrchestrationAttemptV1` attempt identities plus the closed shared-fact
families (`context`, `execution`, `verification`, `cleanup`) drawn from each
attempt. The lifecycle retained surface contains no host-local spellings —
no worktree roots, durations, PIDs or scratch paths — so its retained digest
is already portable and equivalent inputs share one identity. A blank
required identity (lifecycle, observation key, base attempt id, shared-fact
family, issue snapshot, current main, portfolio, selected work, decision
rationale, plan id or closeout reason on a completed closeout) rejects the
row outright; evidence without an exact identity or with zero bytes is
malformed, never countable.

The counting law, enforced by `assess_issue_lifecycle_attempt` and the
scorecard builder:

- Synthetic mechanics rows (`synthetic: true`) are assessed by the same law
  but counted separately; they never enter real-use denominators.
- One lifecycle observed through several roles or imports remains one
  lifecycle: rows deduplicate by `observation_key`, the representative is
  the smallest lifecycle id, and observations that disagree on identity,
  counted flag or disposition reject each other — in either input order.
- Open intake questions cap every claimed disposition at `needs_evidence`.
- A `spec_required` decision can never qualify as direct one-PR work: a
  claimed `qualified_one_pr` downgrades to `qualified_spec_required`.
- An unchanged progress update (equal before/after digests) that was not
  suppressed blocks completion-shaped dispositions.
- Merge is not closeout: a claimed `completed` downgrades deterministically
  — never upgrades — when a completed closeout state, a current-head
  verification receipt, a matching current main or a clean burn-down is
  missing: no completed closeout state or no current-head proof becomes
  `merged_pending_closeout` when a merge identity is bound and `blocked`
  otherwise; uncovered or contradicted burn-down rows become
  `partially_landed` when a merge identity is bound and `blocked`
  otherwise; a stale current-main receipt becomes `stale`; a spec-required
  decision without acceptance evidence becomes `blocked`. The first
  established downgrade wins and every blocking condition is retained.
- Absent trustworthy data stays `not_measured`: with zero real lifecycles
  the implementation success rate projects `not_measured`, never a
  fabricated zero or hundred percent — the honest empty report exists
  before any real issue is counted.

The scorecard's `corpus_identity` binds the sorted retained identities of
every row, so reordered inputs preserve it while any evidence change yields
a new identity. Per-disposition totals keep a stable zero row for every one
of the fifteen dispositions, so negative and closed dispositions never
disappear from the projection.

## Required Evidence

- The committed mechanics corpus `fixtures/issue_lifecycle_attempts/corpus.json`
  covering all fourteen required scenarios: one-PR bug with no new spec;
  public-contract issue requiring independent challenge; architecture
  ambiguity returning a root decision; duplicate case; already-satisfied
  case; overlapping open PR blocking pickup; needs-evidence case; partially
  landed umbrella with uncovered rows; verification failed after a merge
  candidate; merged PR pending current-head closeout; stale
  issue/main/PR/receipt identity; unchanged progress update suppressed;
  closed-not-planned without implementation success; and malformed zero-byte
  intake evidence.
- `cargo test -p xtask issue_lifecycle` unit coverage over the
  counting law, digest bindings and scorecard projection.
- `cargo xtask issue-lifecycle-scorecard` JSON and Markdown reports derived
  from one DTO, including the honest empty report (zero real lifecycles,
  `not_measured` rate) that the synthetic-only fixture corpus projects.

## Non-Goals

No live GitHub mutation, issue selection or triage, spec acceptance, source
edit, PR, merge, closeout execution, priority ranking, automation-default
decision, portfolio architecture or policy ratification. This contract does
not run intake, planning, implementation or closeout on a real issue and
does not establish that any lifecycle stage is accurate or useful on real
issues. The children #4930 (intake pilot), #4931 (decision boundary), #4932
(burn-down/closeout pilot) and #4933 (policy ratification) are
designed-for but not implemented here.

Requirement-level v2 blocks and PR-local implementation slices belong in
their respective authorities; do not duplicate their normative prose here.
Acceptance of this document does not imply implementation, evidence, or
support.

## Acceptance Examples

- A one-PR narrow bug row with complete intake evidence, no spec requirement
  and a covered plan counts as `qualified_one_pr` with no downgrade reasons.
- The same shape with open intake questions downgrades to `needs_evidence`;
  with `spec_required: true` a claimed `qualified_one_pr` downgrades to
  `qualified_spec_required`; with an unsuppressed unchanged progress update
  a completion-shaped claim downgrades to `blocked`.
- A claimed `completed` row with a merge identity but no current-head
  verification downgrades to `merged_pending_closeout`; with uncovered
  burn-down rows it downgrades to `partially_landed`; with a verification
  receipt bound to an older main it downgrades to `stale` — each with the
  exact reason retained, and no merge ever projects as a completed closeout.
- Duplicate, already-satisfied, blocked, needs-evidence and
  closed-not-planned rows stay counted and visible, and none of them counts
  as implementation success in the scorecard.
- A corpus of only synthetic fixture rows projects `real_lifecycles: 0` and
  a `not_measured` implementation success rate.

## Test Mapping

- `xtask/src/issue_lifecycle_attempt.rs::tests` — row validation: counting
  law, digest binding, shared-fact vocabulary, intake gate, spec-decision
  gate, unchanged suppression, merge-versus-closeout gates, fixture loader.
- `xtask/src/reports/issue_lifecycle.rs::tests` — scorecard projection:
  honest empty rate, synthetic separation, negative-disposition success
  exclusion, deduplication, conflicting-observation rejection, corpus
  identity stability, deterministic JSON/Markdown.
- `xtask/src/fixture_contracts/general_validators.rs` —
  `validate_issue_lifecycle_attempts_fixture_corpus` gate coverage over the
  committed corpus.

## Implementation Mapping

- `xtask/src/issue_lifecycle_attempt.rs` — DTO family, digests, validator.
- `xtask/src/reports/issue_lifecycle.rs` — scorecard DTO, projections,
  command.
- `fixtures/issue_lifecycle_attempts/` — mechanics corpus and manifest.
- `xtask/src/command.rs`, `xtask/src/dispatch.rs` — command registration.

## CI Proof

```bash
cargo test -p xtask issue_lifecycle
cargo xtask issue-lifecycle-scorecard --captured fixtures/issue_lifecycle_attempts/corpus.json
cargo xtask check-fixture-contracts
cargo xtask check-output-contracts
cargo xtask check-static-language
cargo xtask check-local-context
cargo xtask check-file-policy
cargo xtask check-generated-clean
cargo xtask precommit
git diff --check
```

## Metrics

- `issue_lifecycle_fixture_scenarios`
- `issue_lifecycle_scorecard_synthetic_lifecycles`
- `issue_lifecycle_scorecard_rejected_rows`
- `issue_lifecycle_scorecard_real_lifecycles`
- `issue_lifecycle_scorecard_implementation_success_rate_state`

## Failure Modes

- A row with a drifted or hand-edited `row_digest` rejects; it cannot enter
  any denominator.
- A corpus missing a required scenario fails the gate.
- A committed expectation that disagrees with the live validator outcome
  fails the report and the fixture-contract gate.
- A corpus with zero real lifecycles must never render a numeric success
  rate; `not_measured` is the only honest state.
