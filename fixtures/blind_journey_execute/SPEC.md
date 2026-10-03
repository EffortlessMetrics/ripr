# Blind Journey Execute Fixture Corpus

Spec: RIPR-SPEC-0205

## Given

The 0.11 blind installed-agent journey (#4600, authority #3797) has an
accepted machine contract (RIPR-SPEC-0200, #4603): typed prompt, answer-key,
event, intervention and receipt shapes, a mechanical contamination scan, a
retained reviewer verdict and one validator. What it deliberately does not
own is execution: the RIPR-SPEC-0200 decision receipt defers per-kind event
digest presence to the #4604 execution slice, and no packet field may turn a
scripted or observed journey into a receipt without the fail-closed producer
path.

This corpus commits scripted journeys (`blind_journey_journey.v1`: the
reviewed operator-visible prompt surface, the separately stored answer key,
ordered observable actions with exact bytes, and axis observations) plus the
expected executor outcome for every scenario. `cargo xtask blind-journey-execute`
and `cargo xtask check-fixture-contracts` run each journey through the
deterministic executor in `xtask/src/blind_journey_execute.rs`; the executor
stamps every emitted packet through the RIPR-SPEC-0200 producer, so a
hand-edited expectation cannot make a wrong journey emit.

The `installed_rust_*` scenarios are the retained literal installed Rust
repair journey of #4516 (fixture authority RIPR-SPEC-0207,
`fixtures/blind_journey_installed_rust/`): the positive row and its ten
failure controls script only public docs/help and literal product-emitted
commands over the bounded tier-boundary Cargo fixture, recorded from a
foreign launch directory against the selected root. They exist alongside the
generic executor scenarios and obey the same derived-terminal law.

## When

An offline executor run loads `corpus.json`, builds the event chain (sequence
and predecessor are assigned by the executor, never by the script), digests
the recorded per-kind input/output bytes, classifies every non-product action
through the closed ten-class intervention taxonomy, derives every evidence
axis under the observation-binding rule, derives the terminal result in one
fixed precedence, requires exact limitations and non-claims for honest
terminals, stamps the packet and then requires the live RIPR-SPEC-0200
validator to accept the derived receipt. The per-kind presence rule deferred
by the contract is enforced here: `file_edit`,
`project_verification_execution`, `static_analysis_execution` and
`receipt_execution` events must carry both input and output bytes, and
`product_command_invocation` events must carry their argv input bytes. A
declared verification exit without a verification execution, or a declared
static or receipt movement without the matching execution, refuses the
journey. Each execution kind also owns one canonical output record:
`exit:<n>` for project verification, `static:<movement>` for static analysis
and `receipt:<state>` for receipt inspection, with the last execution of the
kind governing the axis. A declared observation that disagrees with the
recorded canonical output, a non-canonical output record, or a receipt
execution recording `receipt:not-applicable` refuses the journey, so no
script can declare an exit the transcript did not record. A retained accepted
review must arrive with both digest bindings present: blank bindings would let
stamping bind changed prompt or answer-key content to an old accepted verdict.

## Then

- A clean scripted journey validates `passed_blind_journey`; selecting the
  second eligible item or appending an instrument-only watchdog observation
  stays positive.
- A recorded failing verification exit derives
  `verification_failure_visible`; an absent verification derives
  `verification_not_run_visible`; an explicit operator question without any
  selection derives `product_discoverability_failure`; and non-positive-grade
  axes with exact limitations and non-claims derive `honest_limitation`.
- A classified private hint or hidden knowledge derives
  `hidden_operator_assistance` even when every other axis passes; manual
  plumbing, workspace binary substitution and an unsafe action each derive
  their own terminal result; a quiet-neighbor selection derives
  `wrong_or_stale_subject` and a forbidden edit derives `unsafe_or_wrong_edit`.
- A journey whose events miss a required per-kind digest, whose observations
  are unbound or contradict the recorded canonical execution outputs, whose
  prompt is mechanically contaminated, whose accepted review is unbound or
  stale, or whose honest terminal lacks exact limitations refuses with the
  exact violated rule; no receipt exists for those journeys.

## Must Not

- Do not run a candidate, launch a process, or decide a release verdict.
- Do not accept a precomputed event digest: the executor digests recorded
  bytes itself.
- Do not let a script claim a terminal result, an axis value or a review
  binding: every receipt field is stamped or derived.
- Do not expose answer-key material in any operator-visible projection.
- Do not count any scripted executor success as installed usefulness, blind
  qualification, candidate selection or parent acceptance.
