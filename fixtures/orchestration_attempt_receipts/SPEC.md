# Orchestration Attempt Receipts Fixture Corpus

Spec: RIPR-SPEC-0213

## Given

The orchestration dogfood family (parent #1639, root contract #4925) needs one
versioned, agent-neutral attempt receipt contract before any real attempt is
counted. The generic pilot receipts (#1707/#1796) record that one journey
happened; they do not decide whether an attempt row may enter a denominator,
whether a claimed completion survives its own evidence, or whether several
observations of one work item were laundered into several attempts.

This manifest-only corpus commits typed attempt rows
(`orchestration_attempt.v1`) and the expected counting outcome for every
mechanics scenario. `cargo xtask orchestration-scorecard` and
`cargo xtask check-fixture-contracts` run each row through the real
counting-law validator in `xtask/src/orchestration_attempt.rs`; a hand-edited
expectation cannot make a wrong row count because the validator decides
independently.

## When

An offline validator run loads `corpus.json`, recomputes every row digest
binding over the canonical retained surface, and assesses each row against the
closed strategy and disposition vocabulary and the counting law: required
identities, malformed and over-budget evidence, the verified_fact independent
receipt gate, forbidden-path boundary status, unresolved contradictions,
rejected claims, stale verification bindings, zero-subject command
denominators, missing synthesis, missing required overflow, and cleanup
residue. Downgrades are deterministic and every reason is retained; no
claimed disposition is ever upgraded.

The scorecard then deduplicates by `observation_key` (one work item observed
through several roles remains one attempt; conflicting observations reject
each other), separates synthetic mechanics rows from real denominators, and
projects one `OrchestrationScorecardV1` DTO to JSON and Markdown. The corpus
identity binds the sorted portable identities, so reordered inputs and
equivalent worktree root spellings preserve it.

## Then

- The single-agent-preferred narrow task counts as `completed` under the
  `single_agent` strategy; single-agent preference stays a first-class,
  visible decision.
- The read-only fan-out with bounded overflow counts as `completed`: the
  retrieved required overflow is retained and the omitted non-required
  overflow is disclosed, not hidden.
- The adversary contradiction blocks the dependent result: the claimed
  completion downgrades to `contradicted` and the reason is retained.
- The builder claim rejected by the verifier downgrades the claimed
  completion to `verification_failed`.
- The parallel writers rejected for a semantic conflict across disjoint files
  downgrade to `contradicted`.
- The stale portfolio/base/result identity downgrades the claimed completion
  to `stale`.
- The root-worktree contamination through a forbidden path downgrades the
  claimed completion to `boundary_violation`.
- The malformed over-budget result rejects and enters no denominator.
- The interrupted claim with cleanup residue stays a counted, visible
  `partial` row with the residue retained.
- The duplicate observations of one attempt share one portable identity and
  deduplicate to one attempt with two observations.
- The equivalent-root rows share one portable identity across different
  retained worktree spellings.
- Every row in this corpus is synthetic mechanics evidence: the projected
  scorecard carries zero real attempts and an honest `not_measured`
  completion rate.

## Must Not

- Do not run an agent, select work, compare strategies, or ratify fan-out
  defaults.
- Do not count any synthetic mechanics row in a real-use denominator.
- Do not upgrade a claimed disposition or hide downgrade reasons.
- Do not treat a passing zero-subject command as verification.
- Do not let a builder claim become `verified_fact` without a matching
  independent receipt.
- Do not let Markdown or any prose projection strengthen machine state.
