# RIPR-SPEC-0205: Blind journey execution consumer

Status: proposed

Owner: test-infra

Created: 2026-10-02

Linked issues:

- #4604 (this slice)
- #4603 (blind journey contract; this consumer must consume that exact
  merged contract identity)
- #4600 (parent blind installed-agent acceptance)
- #3797 (blind journey authority)
- #4510 (shared candidate/process harness; admission authority, referenced not
  copied)
- #4516 / #4518 / #4519 (literal installed Rust/Python/TypeScript journeys
  this executor makes scriptable without pre-implementing them)

Support-tier impact:

- None. The executor is an offline typed projection over committed scripted
  journeys; it launches no candidate, edits nothing, calls no provider and
  collects no telemetry. [docs/status/SUPPORT_TIERS.md](../status/SUPPORT_TIERS.md)

Policy impact:

- None. No new process, network or file-policy surface; the report reads the
  committed corpus and receipt and renders to stdout/`target/ripr/reports`
  only.

## Problem

RIPR-SPEC-0200 (#4603) ratified the blind-journey contract shapes, the
mechanical contamination scan and the fixture scope, and its decision receipt
explicitly defers two execution duties: per-kind event input/output digest
presence, and the fail-closed producer path a real run must stamp and validate
through. Without a named deterministic consumer, the #4516/#4518/#4519
literal journeys and the final #4600 blind run have no executable seam: each
evaluator would hand-assemble receipts, restate digests, or claim terminal
results after seeing the machine state — exactly the no-rescue and
evidence-axis laws #4600 forbids.

## Behavior

`cargo xtask blind-journey-execute` (#4604) runs the committed scripted
corpus `fixtures/blind_journey_execute/corpus.json` through one deterministic
executor in `xtask/src/blind_journey_execute.rs`:

- `BlindJourneyJourneyV1` (`blind_journey_journey.v1`) scripts one journey:
  the candidate identity reference, the reviewed operator-visible prompt
  surface (goal, public inputs, ordinary permissions, prohibited hints and
  exact prompt bytes), the retained reviewer verdict with digest bindings,
  the separately stored `BlindJourneyAnswerKeyV1`, the ordered
  `BlindJourneyActionV1` list, the axis observations, and the limitations and
  non-claims. Digests are always empty on the wire; stamping binds them.
- The executor assigns every sequence number and event predecessor itself, so
  a script cannot reorder or gap the transcript. It enforces the per-kind
  digest presence rule the RIPR-SPEC-0200 receipt deferred: `file_edit`,
  `project_verification_execution`, `static_analysis_execution` and
  `receipt_execution` actions must carry both input and output bytes, and
  `product_command_invocation` must carry its argv input bytes. The executor
  digests the recorded bytes itself; a precomputed digest is never accepted.
- The observation-binding rule keeps every axis honest: a declared
  verification exit without a project-verification execution, a declared
  static movement without a static analysis execution, and a declared receipt
  state without a receipt execution refuse the journey, as does an execution
  without its recorded observation. Each execution kind owns one canonical
  output record — `exit:<n>` for project verification, `static:<movement>`
  for static analysis, `receipt:<state>` for receipt inspection — and the
  last execution of the kind governs the axis: a declared observation that
  disagrees with the recorded canonical output, a non-canonical output record,
  or a receipt execution recording `receipt:not-applicable` refuses the
  journey, so no script can declare an exit the transcript did not record.
  Candidate currentness remains a declared admission observation (#4510 in
  real runs) that the executor records without strengthening.
- A retained accepted review must arrive with both digest bindings present.
  Blank bindings would let stamping bind today's prompt or answer-key content
  to a previously accepted verdict, so an accepted review without exact
  bindings refuses with `review_binding_missing` before any receipt exists;
  a present divergent binding keeps refusing at stamping.
- Intervention classification is closed at construction: harness and
  process-control actions must carry one taxonomy class, every classified
  action names its actor, every disqualifying action names its exact basis,
  and an instrument-only observation must not be operator visible.
- The terminal result is derived, never claimed, in one fixed precedence:
  assistance disqualifiers in event order; the answer-key comparison
  (selection against the eligible list and quiet neighbors, then the edit
  cage against the expected cage and forbidden edits); an explicit operator
  question without any selection
  (`product_discoverability_failure`); a failed project verification
  (`verification_failure_visible`); an absent project verification
  (`verification_not_run_visible`); any non-positive-grade evidence axis
  (`honest_limitation`); and only then `passed_blind_journey`. Honest
  terminals require exact non-empty limitations and non-claims before any
  receipt exists.
- Every emitted packet passes through `stamp_blind_journey_packet` and must
  then be accepted by the live `assess_blind_journey_packet`; a contaminated
  prompt, a missing or stale review, a divergent review binding or any other
  validator rejection refuses the run with the validator's own reasons. The
  executor therefore cannot emit a receipt the contract would reject, and a
  local modification to the producer or validator invalidates the run rather
  than becoming an execution convenience.

JSON and Markdown derive from one evaluated DTO. The committed decision
receipt `metrics/blind-journey-execute/executor-receipt.json` ratifies the
fixture scope and is rejected when it drifts from the consumer schema
versions or the consumer claim boundary, no longer binds the assessed corpus
scenario count, carries no exact limitations, or claims more than the
scripted evidence supports.

## Required Evidence

- The required native committed-corpus test regenerates the JSON/Markdown
  assessment; the existing always-uploaded required-test artifact retains those
  exact two report paths alongside JUnit and run context. Acceptance requires
  that actual test to pass on the recorded checkout, plus matching current corpus
  counts and input digests. Report file existence alone is not fresh execution
  proof, and the scripted assessment does not qualify an installed candidate.
- `cargo xtask blind-journey-execute` rebuilds the assessment in CI, checks
  the committed corpus against the live executor, validates the decision
  receipt, and writes `target/ripr/reports/blind-journey-execute.{md,json}`.
- `cargo test -p xtask blind_journey_execute` covers the executor units:
  digest presence per kind, observation binding, derived terminals for every
  disqualifier and honest shape, stamping refusals and the
  validator-accepted emission loop.
- `cargo xtask check-fixture-contracts` runs the corpus through the same
  executor on every policy pass.
- `cargo test -p xtask blind_journey` (RIPR-SPEC-0200) stays the contract
  authority; this consumer adds no contract field.

## Non-Goals

- No candidate execution, operator/model evaluation, prompt optimization,
  model scoring or private chain-of-thought collection; the real candidate
  run stays with #4604's residual execution lane once #4510/#1609 start
  conditions are terminal.
- No product command or repair implementation; #4516/#4518/#4519 stay the
  journey owners and supply real fixtures and observations later.
- No candidate admission, selection, publication, tag, release or source
  merge; #4510 stays the admission authority.
- No claim that any candidate passes the journey, and no strengthening of
  the RIPR-SPEC-0200 contract corpus, which stays digest-optional at its own
  schema version.

## Acceptance Examples

1. A clean scripted journey emits one stamped receipt that the live
   RIPR-SPEC-0200 validator accepts with `passed_blind_journey`.
2. A `file_edit` action without output bytes, a verification execution
   without input bytes, or a command invocation without argv input bytes
   refuses with `digest_presence_violation` before any receipt exists.
3. A declared verification exit without a verification execution refuses
   with `observation_unbound`; so does a declared static movement without a
   static analysis execution. A declared exit that contradicts the recorded
   canonical output (declared `exit 0` over recorded `exit:1`), a
   non-canonical output record, or a receipt execution recording
   `receipt:not-applicable` refuses with `observation_unbound` as well.
4. A classified private hint after a fully passing journey derives
   `hidden_operator_assistance`; the receipt stays accepted and non-positive.
5. A quiet-neighbor selection derives `wrong_or_stale_subject`; a forbidden
   edit derives `unsafe_or_wrong_edit`; a failing verification exit derives
   `verification_failure_visible`; and an honest limitation without exact
   limitations refuses instead of emitting a blank receipt.
6. A mechanically contaminated prompt, a positive journey without an
   accepted review, an accepted review whose digest bindings are blank, or a
   stale review binding refuses with the executor's, the stamper's or the
   validator's own reason.
7. A hand-edited executor decision receipt that drifts from the consumer
   schema versions, drops its limitations or stops binding the assessed
   corpus scenario count fails the report gate.

## Test Mapping

- `xtask/src/blind_journey_execute.rs` unit tests: digest presence,
  observation binding, terminal derivation, honest-limitation exactness,
  stamping refusals, instrument-only visibility and the accepted emission
  loop.
- `xtask/src/reports/blind_journey_execute.rs` tests: committed corpus and
  receipt gate, drift rejections, projection determinism and required
  scenario coverage.
- `cargo xtask blind-journey-execute` is itself a CI-executable proof of the
  live executor and the receipt agreement.

## Implementation Mapping

| Surface | Responsibility |
| --- | --- |
| `xtask/src/blind_journey_execute.rs` | journey/action/observation DTOs, event builder with per-kind digest presence, axis derivation and observation binding, terminal derivation, executor loop, unit tests |
| `xtask/src/reports/blind_journey_execute.rs` | corpus assessment, executor decision-receipt gate, JSON/Markdown projections, report tests |
| `fixtures/blind_journey_execute/` | committed scripted journey corpus + SPEC |
| `metrics/blind-journey-execute/executor-receipt.json` | versioned ratification receipt with limitations |
| `xtask/src/command.rs` + `dispatch.rs` | `blind-journey-execute` registration |

## Metrics

- `blind_journey_execute_scenarios`
- `blind_journey_execute_emitted_scenarios`
- `blind_journey_execute_positive_scenarios`
- `blind_journey_execute_expectation_failures`
- `blind_journey_execute_receipt_status`
