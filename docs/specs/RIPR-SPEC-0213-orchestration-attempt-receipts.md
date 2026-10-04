# RIPR-SPEC-0213: Orchestration attempt receipts contract and scorecard projection

Status: proposed

Owner: test-infra

Created: 2026-10-03

Linked issues:

- #4925 (this slice: the attempt receipt contract, counting law, mechanics
  fixtures and scorecard projection)
- #1639 (parent/controller: governed orchestration acceptance)
- #1792 (closed oversized slice; this spec replaces its schema, fixture,
  denominator and report portions)
- #1633 (task/result/verification contracts; referenced, not duplicated)
- #1636 (synthesis vocabulary; referenced, not duplicated)
- #1707/#1796 (pilot receipts; referenced, not duplicated)
- #4929/#4930/#4931/#4932/#4933 (issue-lifecycle family; this contract is the
  root they extend)

Support-tier impact:

- None. The validator is an offline typed projection over committed fixture
  rows; it launches no agent, selects no work, edits nothing, calls no
  provider and collects no telemetry.
  [docs/status/SUPPORT_TIERS.md](../status/SUPPORT_TIERS.md)

Policy impact:

- None. No new process, network or file-policy surface; the report reads the
  committed corpus and renders to stdout/`target/ripr/reports` only.

## Problem

The orchestration dogfood family (parent #1639) cannot count a single attempt
honestly: there is no versioned, agent-neutral row that binds a work item's
identity to its strategy, disposition, denominators, verification and cleanup
evidence, and no deterministic projection that decides whether a claimed
completion survives its own evidence. Without that contract, later slices
would either count attempts that lack exact identity or cleanup, or silently
drop blocked, contradicted and single-agent-preferred rows from view. The
issue-lifecycle family (#4929 onward) additionally needs one shared attempt
authority to reference for context, execution, verification and cleanup
facts, so those facts are not copied into a second attempt store.

## Behavior

`cargo xtask orchestration-scorecard [--captured <fixture-corpus.json>]`
loads one committed corpus of typed attempt rows (`orchestration_attempt.v1`),
runs every row through the fail-closed counting-law validator, evaluates every
committed fixture expectation against the live outcome, and projects one
`OrchestrationScorecardV1` DTO to `target/ripr/reports/orchestration-scorecard.{json,md}`.
Both projections derive from the same assessed rows; Markdown can never
strengthen machine state. The gate fails closed when a scenario's live outcome
drifts from its committed expectation, when a required scenario is missing,
or when the corpus is unparseable.

The closed vocabulary:

- `AttemptStrategyV1`: `single_agent`, `read_only_fanout`,
  `scouts_plus_adversary`, `builder_plus_verifier`,
  `validated_parallel_writers`.
- `AttemptDispositionV1`: `completed`, `partial`, `blocked`, `contradicted`,
  `verification_failed`, `stale`, `boundary_violation`, `instrument_failure`,
  `not_run`. Only `completed` is the positive terminal.
- `AttemptComparisonV1`: `matched`, `near_matched`, `non_comparative`,
  `incomparable`.

Each real row retains at least repository/source/selected-work/portfolio/base/head
identity — the opaque `source` names where the selected work came from and
stays distinct from `selected_work`, so identical work from different sources
remains distinct — task family and accepted contract, agent client and role
configuration, planned and actual waves, packet/result/synthesis/overflow
identities and bytes, claims with worktree/edit cage/resources, command
denominators, independent verification with its base/head/result binding,
contradictions and rejected claims, changed paths and boundary status,
PR/review/CI/merge state where applicable, cleanup and residue, limitations
and non-claims, and a producer-recorded `row_digest` over the canonical
retained surface. A blank required identity — attempt, work, task family,
contract, client, observation key, evidence identity, verification id or
claim id — rejects the row outright; evidence without an exact identity is
malformed, never countable. Volatile durations, PIDs, scratch roots and API
request IDs
stay outside the contract entirely: they are host-local telemetry, never
portable semantic identity. The one retained host-local spelling is each
claim's `worktree_root`; it binds the row digest but never the portable
identity, so equivalent roots at equivalent inputs share one portable
identity.

The counting law, enforced by `assess_orchestration_attempt` and the
scorecard builder:

- Synthetic mechanics rows (`synthetic: true`) are assessed by the same law
  but counted separately; they never enter real-use denominators.
- One work item observed through several roles remains one attempt: rows
  deduplicate by `observation_key`, the representative is the smallest
  attempt id, and observations that disagree on portable identity, counted
  flag or disposition reject each other — in either input order — so a
  malformed duplicate cannot launder into a second attempt or hide a
  digest-rejected row.
- Blocked, contradicted, stale, malformed, over-budget, verification-failed
  and single-agent-preferred rows remain visible: counted rows carry their
  disposition and rejected rows stay listed with their exact reasons.
- A passing command with zero subjects is not verification.
- A builder claim cannot become `verified_fact` without a matching
  independent receipt (`independent_receipt` with a `matched` comparison).
- Missing required overflow makes the row incomplete.
- Absent trustworthy data stays `not_measured`: with zero real attempts the
  completion rate projects `not_measured`, never a fabricated zero or
  hundred percent.
- A claimed `completed` disposition is deterministically downgraded — never
  upgraded — when the evidence does not support it: forbidden-path changes
  force `boundary_violation`, unresolved contradictions force `contradicted`,
  missing independent verification blocks, a non-`matched` comparison, failed
  or zero-subject verification, or non-independent receipts force
  `verification_failed`, stale verification bindings force `stale`, missing
  synthesis or missing required overflow blocks, rejected claims force
  `verification_failed`, and cleanup residue downgrades to `partial`. The
  first established downgrade wins: once a row is `boundary_violation` or
  `contradicted`, no later verification signal rewrites it, and every
  blocking condition is retained in the row reasons.

The scorecard's `corpus_identity` binds the sorted portable identities of
every row, so reordered inputs and equivalent roots preserve it while any
evidence change yields a new identity.

## Required Evidence

- The committed mechanics corpus
  `fixtures/orchestration_attempt_receipts/corpus.json` covering all thirteen
  required scenarios: single-agent-preferred narrow task; read-only fan-out
  with bounded overflow; adversary contradiction blocking a dependent result;
  builder claim rejected by the verifier; parallel writers rejected for a
  semantic conflict despite disjoint files; stale portfolio/base/result
  identity; root-worktree contamination through a forbidden path;
  malformed/over-budget result; interrupted claim with cleanup residue;
  duplicate observations of one attempt (two scenarios); and equivalent roots
  preserving the portable digest (two scenarios).
- `cargo test -p xtask orchestration_attempt` unit coverage over the
  counting law, digest bindings and scorecard projection.
- `cargo xtask orchestration-scorecard` JSON and Markdown reports derived
  from one DTO, including the honest empty report (zero real attempts,
  `not_measured` rate) that the synthetic-only fixture corpus projects.

## Non-Goals

No agent spawning, task selection, source edit, PR, merge, GitHub mutation,
provider comparison, strategy ratification, fan-out default ratification,
support-tier or release action. This contract does not run subagents,
execute a real PR or establish that orchestration is useful, faster, safer or
preferable on any real task. The issue-lifecycle children (#4929 onward) are
designed-for but not implemented here.

Requirement-level v2 blocks and PR-local implementation slices belong in
their respective authorities; do not duplicate their normative prose here.
Acceptance of this document does not imply implementation, evidence, or
support.

## Acceptance Examples

- A complete single-agent row with matching identities, a matched
  independent receipt, retrieved required overflow, clean cleanup and only
  within-cage paths counts as `completed` under `single_agent`.
- The same row with a zero-subject passing verification command downgrades to
  `verification_failed`; with the verification bound to an older base it
  downgrades to `stale`; with cleanup residue it downgrades to `partial`;
  with a forbidden path change it downgrades to `boundary_violation`; with an
  unresolved contradiction it downgrades to `contradicted`; and with a
  rejected builder claim it downgrades to `verification_failed` — each with
  the exact reason retained.
- The same row with its verification comparison set to `near_matched`
  downgrades to `verification_failed`; a forbidden path combined with a
  failed verification command keeps `boundary_violation` with both reasons
  retained, because the first established downgrade wins.
- Two byte-identical observations of one attempt project as one attempt with
  two observations; two rows differing only in worktree root spelling share
  one portable identity.
- A corpus of only synthetic fixture rows projects `real_attempts: 0` and a
  `not_measured` completion rate.

## Test Mapping

- `xtask/src/orchestration_attempt.rs::tests` — row validation: counting law,
  digest binding, verified_fact receipt gate, vocabulary, fixture loader.
- `xtask/src/reports/orchestration.rs::tests` — scorecard projection: honest
  empty rate, synthetic separation, deduplication, conflicting-observation
  rejection, corpus identity stability, deterministic JSON/Markdown.
- `xtask/src/fixture_contracts/general_validators.rs` —
  `validate_orchestration_attempt_receipts_fixture_corpus` gate coverage over
  the committed corpus.

## Implementation Mapping

- `xtask/src/orchestration_attempt.rs` — DTO family, digests, validator.
- `xtask/src/reports/orchestration.rs` — scorecard DTO, projections, command.
- `fixtures/orchestration_attempt_receipts/` — mechanics corpus and manifest.
- `xtask/src/command.rs`, `xtask/src/dispatch.rs` — command registration.

## CI Proof

```bash
cargo test -p xtask orchestration_attempt
cargo xtask orchestration-scorecard --captured fixtures/orchestration_attempt_receipts/corpus.json
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

- `orchestration_fixture_scenarios`
- `orchestration_scorecard_synthetic_attempts`
- `orchestration_scorecard_rejected_rows`
- `orchestration_scorecard_real_attempts`
- `orchestration_scorecard_completion_rate_state`

## Failure Modes

- A row with a drifted or hand-edited `row_digest` rejects; it cannot enter
  any denominator.
- A corpus missing a required scenario fails the gate.
- A committed expectation that disagrees with the live validator outcome
  fails the report and the fixture-contract gate.
- A corpus with zero real attempts must never render a numeric success rate;
  `not_measured` is the only honest state.
