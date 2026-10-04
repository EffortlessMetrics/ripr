# RIPR-SPEC-0200: Blind journey prompt, intervention and transcript receipt contract

Status: proposed

Owner: test-infra

Created: 2026-10-02

Linked issues:

- #4603 (this slice)
- #4600 (parent blind installed-agent acceptance)
- #3797 (blind journey authority)
- #4510 (shared candidate/process harness; admission authority, referenced not copied)
- #4508 (deterministic journey inputs; public journey fields, no answer key copied)
- #4604 (named execution consumer; must consume this exact merged contract identity)

Support-tier impact:

- None. The validator is an offline typed projection over committed fixture
  packets; it launches no candidate, edits nothing, calls no provider and
  collects no telemetry. [docs/status/SUPPORT_TIERS.md](../status/SUPPORT_TIERS.md)

Policy impact:

- None. No new process, network or file-policy surface; the report reads the
  committed corpus and receipt and renders to stdout/`target/ripr/reports`
  only.

## Problem

#4600 requires an uninformed operator to use only the installed product,
public docs/help and ordinary source inspection. That claim needs a reusable
machine contract before the candidate run begins, otherwise the final
evaluator can define "hidden help", omit inconvenient interventions, or
strengthen a partial transcript after seeing the result. The generic #4510
packet knows candidate identity, commands, processes, artifacts and cleanup;
it does not know what the operator was initially told, which information was
prohibited, which actions were product-supported versus private assistance,
whether the prompt leaked the answer key, or whether a transcript is complete
enough for independent audit.

## Behavior

`cargo xtask blind-journey-contract` (#4603) validates the committed
manifest-only corpus `fixtures/blind_journey_contract/corpus.json` through one
typed, versioned contract and validator in `xtask/src/blind_journey.rs`:

- `BlindJourneyPromptV1` retains the candidate/package/binary identity
  reference, the target repository/base/head/tree/root identity, the generic
  operator goal, the public docs/help inputs made available, ordinary
  permissions, explicit prohibited hints, the exact prompt bytes and digest,
  the retained named-reviewer verdict, and the producer-recorded mechanical
  contamination result.
- `BlindJourneyAnswerKeyV1` is stored separately from the operator input and
  retains eligible canonical items, quiet/already-covered neighbors, the
  expected safe target/edit cage, the discriminator family, known product
  limitations and forbidden edits, plus its canonical digest.
- `BlindJourneyEventV1` captures observable activity only (product command
  invocations, followed outputs, public documentation lookups, ordinary
  target-source reads, operator questions and product-option selections,
  file edits, project verification, static analysis, receipt execution,
  process cleanup, and harness/evaluator interventions). Every event binds
  sequence/predecessor, an exact subject, input/output digests and whether it
  was operator visible at that point. Private chain-of-thought is never
  required or stored.
- `BlindJourneyInterventionV1` is one closed taxonomy of ten classes. The
  first four (`product_supported`, `ordinary_source_inspection`,
  `public_documentation_lookup`,
  `operator_choice_within_product_options`) may appear in a positive run;
  `instrument_only_not_operator_visible` may record watchdog/process/
  filesystem observation without changing the operator path or product state;
  the remaining five (`hidden_operator_knowledge`, `manual_artifact_plumbing`,
  `private_harness_hint`, `workspace_binary_substitution`,
  `unsafe_or_unbounded_action`) make the positive blind claim false. Every
  non-product action carries one classification, an actor, an exact
  reason and, where applicable, exact bytes/subject. Free-form "assisted a
  little" is invalid.
- `BlindJourneyReceiptV1` binds the exact reviewed prompt and answer-key
  digests, the recorded selection and edit, the event transcript, the
  separate evidence axes (candidate/currentness, selection correctness,
  edit-cage verdict, project verification status, static movement, receipt
  status, operator assistance state, and external runtime/mutation evidence
  when separately supplied) and one terminal result from the closed thirteen
  value vocabulary. Only `passed_blind_journey` is the positive row; negative
  controls retain their own expected result and no aggregate percentage
  converts them into success.

Two contamination judgments stay mechanically distinct. A closed structured
scan over the exact prompt bytes rejects issue/PR or internal campaign
references, selected fixture families, preselected gap/seam/item ids, target
files/tests/functions, command sequences beyond public entrypoint permission,
non-public artifact paths, private expected results or workarounds, internal
source references and prior transcript content. A retained named-reviewer
verdict over the exact prompt and separately stored answer key addresses
semantic answer leakage. Keyword absence is not evidence of semantic
blindness; a supplied reviewer label never overrides a mechanical finding;
and until a current accepted reviewer verdict exists, the validator refuses a
positive blind result.

The validator fails closed: hidden hints, manual plumbing, wrong binaries and
unsafe or wrong edits cannot produce a positive receipt; a quiet-neighbor
selection fails the answer-key comparison; a passing verification or static
axis cannot hide operator assistance; an honest limitation stays an accepted
non-positive receipt with the complete transcript and exact recovery
non-claim; missing predecessors, reordered traces, altered prompt bindings,
changed answer keys and human/machine disagreement reject; a receipt review
that diverges from the retained prompt review, a recorded selection that
disagrees with the recorded selection event, a passed verification axis
without an execution event, an unclassified process-control event, a
malformed event digest, and a forbidden edit recorded only in the transcript
all reject; equivalent concrete root spellings share one portable semantic
identity while concrete root evidence remains retained; and an unsupported
future schema rejects instead of aggregating to a clean pass. The prompt
digest, the mechanical scan and the secrecy projection all bind the same
operator-visible prompt surface — the goal, restated inputs, permissions,
hints and exact prompt bytes — so a producer cannot restate one reviewed
field after review. Answer-key bytes remain evaluator-only: the
operator-visible projection is scanned for exact absence of answer-key
contents and references at the event boundary; operator-originated events
(selections, edits, ordinary reads, questions) stay outside the secrecy
projection because an honest transcript records what the operator actually
did, which may legitimately name a quiet neighbor or a forbidden path.
Timestamps, durations, PIDs and absolute temporary-root spelling stay
telemetry.

JSON and Markdown derive from one evaluated DTO, so prose cannot strengthen
machine state. The committed decision receipt
`metrics/blind-journey-contract/contract-receipt.json` ratifies the fixture
scope and is rejected when it drifts from the contract schema versions, no
longer binds the assessed corpus scenario count, carries no exact
limitations, or claims more than the fixture evidence supports.

## Required Evidence

- `cargo xtask blind-journey-contract` rebuilds the assessment in CI, checks
  the committed corpus against the live validator, validates the decision
  receipt, and writes `target/ripr/reports/blind-journey-contract.{md,json}`.
- `cargo test -p xtask blind_journey` covers the validator units: the
  contamination categories, the intervention taxonomy, the digest bindings,
  the event-chain ordering controls, the secrecy projection, the portable
  identity and the terminal result consistency.
- `cargo test -p xtask blind_journey_contract` covers the gate: committed
  corpus drift, drifted contract receipts, receipts without limitations and
  projection determinism.
- `cargo xtask check-fixture-contracts` runs the corpus through the same
  validator on every policy pass.

## Non-Goals

- No candidate execution, operator/model evaluation, prompt optimization,
  model scoring or private chain-of-thought collection.
- No product command or repair implementation.
- No candidate admission, selection, publication, tag, release or source
  merge; #4510 stays the admission authority and #4604 the execution consumer.
- No claim that any candidate passes the journey.

## Acceptance Examples

1. A clean generic prompt validates `passed_blind_journey`; appending an
   issue number, target test, gap id, artifact path or expected command to
   the prompt bytes rejects each category independently.
2. A packet with a classified private hint after a failed command records
   `hidden_operator_assistance` even when verification passed and static
   movement improved.
3. A packet that manually copies a before/verify artifact records
   `manual_artifact_plumbing`; a planted binary records
   `candidate_identity_failure`; neither can produce a positive receipt.
4. A packet whose operator selects the quiet neighbor records
   `wrong_or_stale_subject`; selecting either of two eligible items keeps the
   receipt positive.
5. A reordered trace, a missing predecessor, altered prompt bytes or a
   changed answer key rejects with the exact digest or chain reason.
6. Two packets identical except for concrete root spelling share
   one portable semantic identity and both validate.
7. A hand-edited decision receipt that drifts from the contract schema
   versions, drops its limitations or blanks a limitation entry, or stops
   binding the assessed corpus scenario count fails the report gate.
8. A packet whose transcript records a forbidden edit that the selected edit
   does not name still rejects as `unsafe_or_wrong_edit`; the answer-key
   comparison binds to every recorded file edit, not only the selected one.

## Test Mapping

- `xtask/src/blind_journey.rs` unit tests: contract units, digest bindings,
  contamination, taxonomy, ordering, secrecy, portable identity and terminal
  consistency.
- `xtask/src/reports/blind_journey.rs` tests: committed corpus and receipt
  gate, drift rejections, projection determinism.
- `cargo xtask blind-journey-contract` is itself a CI-executable proof of the
  live assessment and the receipt agreement.

## Implementation Mapping

| Surface | Responsibility |
| --- | --- |
| `xtask/src/blind_journey.rs` | typed contract DTOs, mechanical scan, validator, portable identity, corpus loader, unit tests |
| `xtask/src/reports/blind_journey.rs` | corpus assessment, decision-receipt gate, JSON/Markdown projections, report tests |
| `fixtures/blind_journey_contract/` | committed manifest-only scenario corpus + SPEC |
| `metrics/blind-journey-contract/contract-receipt.json` | versioned ratification receipt with limitations |
| `xtask/src/command.rs` + `dispatch.rs` | `blind-journey-contract` registration |

## Metrics

- `blind_journey_contract_scenarios`
- `blind_journey_contract_positive_scenarios`
- `blind_journey_contract_expectation_failures`
- `blind_journey_contract_receipt_status`
