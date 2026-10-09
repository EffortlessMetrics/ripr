# Issue Lifecycle Attempts Fixture Corpus

Spec: RIPR-SPEC-0218

## Given

The issue-lifecycle family (parent #1650, base attempt authority #4925 /
RIPR-SPEC-0213) needs one versioned issue-lifecycle extension contract
before any real issue is counted. The base orchestration attempt receipts
bind context, execution, verification and cleanup facts; the lifecycle
extension must reference those facts through the base attempt identity
instead of copying them into a second attempt store, and must decide whether
a claimed issue disposition — from intake qualification through burn-down to
closeout — is supported by its own retained evidence.

This manifest-only corpus commits typed lifecycle rows
(`issue_lifecycle_attempt.v1`) and the expected counting outcome for every
mechanics scenario. `cargo xtask issue-lifecycle-scorecard` and
`cargo xtask check-fixture-contracts` run each row through the real
counting-law validator in `xtask/src/issue_lifecycle_attempt.rs`; a
hand-edited expectation cannot make a wrong row count because the validator
decides independently.

## When

An offline validator run loads `corpus.json`, recomputes every row digest
binding over the canonical retained surface, and assesses each row against
the closed disposition vocabulary (fifteen values from `needs_evidence` to
`not_run`) and the counting law: required identities, base-attempt
references with the closed shared-fact families (`context`, `execution`,
`verification`, `cleanup`), malformed zero-byte intake evidence, the intake
evidence gate (open questions cap every claimed disposition at
`needs_evidence`), spec-required decisions blocking `qualified_one_pr`
qualification, unsuppressed unchanged progress updates blocking
completion-shaped dispositions, and the merge-versus-closeout gate (a claimed
`completed` without a completed closeout state or current-head verification
downgrades to `merged_pending_closeout`, uncovered or contradicted burn-down
rows downgrade to `partially_landed`, and a stale current-main receipt
downgrades to `stale`). Downgrades are deterministic and every reason is
retained; no claimed disposition is ever upgraded.

The scorecard then deduplicates by `observation_key` (one lifecycle observed
through several roles or imports remains one lifecycle; conflicting
observations reject each other), separates synthetic mechanics rows from
real denominators, and projects one `IssueLifecycleScorecardV1` DTO to JSON
and Markdown. The corpus identity binds the sorted row digests, so reordered
inputs preserve it.

## Then

- The one-PR narrow bug qualifies directly as `qualified_one_pr` with no new
  spec and no downgrade reasons.
- The public-contract issue stays `qualified_spec_required`: the proposal,
  spec draft, independent challenge and amendment identities are retained
  and the acceptance identity is still absent.
- The architecture ambiguity stops at `root_decision_required`: no root
  disposition is recorded and no contract is accepted.
- The duplicate and already-satisfied cases stay first-class counted rows;
  neither counts as implementation success.
- The overlapping open PR blocks pickup: the collision event and the
  overlapping PR identity stay retained and the row stays `blocked`.
- The needs-evidence case keeps its exact open question and stays
  `needs_evidence`.
- The claimed completed umbrella with uncovered acceptance rows and a merge
  identity downgrades to `partially_landed` with the reason retained.
- The verification-failed merge candidate stays `verification_failed` with
  the contradicted burn-down row retained.
- The claimed completed merge without current-head verification downgrades
  to `merged_pending_closeout`: merge is not closeout.
- The stale main/verification receipt downgrades the claimed completed row
  to `stale`.
- The unchanged progress update with equal before/after digests stays
  counted because it was suppressed.
- The closed-not-planned row stays first-class and claims no implementation
  success.
- The zero-byte intake evidence row rejects and enters no denominator.
- Every row in this corpus is synthetic mechanics evidence: the projected
  scorecard carries zero real lifecycles and an honest `not_measured`
  implementation success rate — the honest empty report exists before any
  real issue is counted.

## Must Not

- Do not run triage, select work, mutate an issue, accept a spec, execute a
  PR or close an issue.
- Do not count any synthetic mechanics row in a real-use denominator.
- Do not upgrade a claimed disposition or hide downgrade reasons.
- Do not let a merge identity imply a completed closeout.
- Do not count duplicate, already-satisfied, needs-evidence, blocked or
  closed-not-planned rows as implementation success.
- Do not copy base orchestration attempt facts into this row; reference them
  through `base_attempts` instead.
- Do not let Markdown or any prose projection strengthen machine state.
