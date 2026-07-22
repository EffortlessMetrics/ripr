# RIPR-SPEC-0135: Assurance State Vocabulary

Status: accepted

Owner: product / swarm

Created: 2026-07-22

Linked ADRs:

- [ADR 0021](../adr/0021-static-and-executed-assurance-axes.md)

Linked issues:

- [#1978](https://github.com/EffortlessMetrics/ripr-swarm/issues/1978) - define
  the assurance vocabulary before adding command execution.
- [#1979](https://github.com/EffortlessMetrics/ripr-swarm/issues/1979) - future
  explicit command execution and provenance-bound result slice.
- [#1941](https://github.com/EffortlessMetrics/ripr-swarm/issues/1941) - repair
  verification and receipt trust boundary.

Support-tier impact:

- See: [support tiers](../status/SUPPORT_TIERS.md).
- No support-tier promotion. This contract separates static movement, command
  execution, receipt issuance, and external runtime mutation confirmation.
- Existing CLI, LSP, report, and receipt surfaces remain advisory and static
  unless a later implementation slice emits the corresponding typed state.

## Problem

RIPR already compares static before/after artifacts and records caller-supplied
verification command text, but the words `verify` and `receipt` can be read as
proof that a command ran. A static movement comparison and an executed test
command answer different questions. A receipt must preserve both answers
without collapsing them into one Boolean or promoting runtime mutation claims.

## Behavior

`RepairAssuranceV1` is a schema-versioned, producer-owned envelope with
independent axes:

```text
static movement
  static_movement_evaluated
  → improved | unchanged | regressed | not_comparable

verification command
  verification_command_available
  → verification_executed_pass
  → verification_executed_fail
  → verification_not_run | verification_unavailable
  → verification_cancelled_or_timed_out

receipt
  receipt_issued | receipt_rejected | receipt_not_requested

runtime mutation
  runtime_mutation_confirmation_external
```

The axes are independent. Static improvement does not imply a command ran. A
passing command does not imply static movement. A receipt can be issued for a
validated static comparison without a command result, but it must disclose
that its assurance is static-only. A runtime mutation result is always
external to RIPR's ordinary static contract.

### Static movement

`static_movement_evaluated` is emitted only after the producer validates a
comparable before/after artifact pair under
[RIPR-SPEC-0134](RIPR-SPEC-0134-repair-artifact-provenance.md). Its result is
one of `improved`, `unchanged`, `regressed`, or `not_comparable`. The record
preserves the canonical gap/seam identity, artifact identities, repository
root, revision/currentness, and analysis input identity.

The static result is not a test execution result and must not be rendered as
`verified`, `killed`, `survived`, `adequate`, or `proven`.

### Command specification and execution

`CommandSpecV1` is a typed description, not permission to execute. It records:

- program and ordered arguments;
- root-relative working directory;
- timeout and cancellation policy;
- network and environment policy;
- expected result parser;
- cost class; and
- bounded human display text.

Only a later explicitly governed execution slice may turn
`verification_command_available` into an execution state. No packet, receipt
request, JSON field, or editor action automatically runs a command.

An execution result records the declared command identity, root and revision
before/after the run, process disposition, exit status, bounded stdout/stderr
commitments, an immutable command-spec digest, and currentness.
`verification_executed_pass` means only that the declared command completed
with the producer-defined passing disposition. It does not mean that the
static gap closed or that the repository is correct. A future runner must
reject a result whose command-spec digest, root, or revision does not match the
declared execution context.

### Receipt issuance

`receipt_issued` requires validated static movement and a coherent assurance
record. A static-only receipt uses `verification_not_run` or
`verification_unavailable` and carries the explicit non-claim
`static_only_assurance`. `receipt_rejected` is fail-closed for malformed,
fabricated, mismatched, stale, or otherwise unverifiable inputs.

`receipt_not_requested` is an explicit absence of a receipt decision; it is
not a passing state.

### Runtime mutation boundary

`runtime_mutation_confirmation_external` is a classification of externally
provided runtime mutation evidence. RIPR may record that boundary in a future
consumer contract, but does not produce `killed` or `survived` from this
vocabulary and does not import those terms into normal static output.

## Compatibility and migration

The current `ripr agent verify` command remains a compatibility surface for
static artifact movement and must document its static-only meaning. The current
`agent receipt` output remains advisory until the later receipt implementation
binds all consumed artifacts, repository state, configuration, command
execution, and observed result.

Future output may add an assurance envelope alongside existing fields. Existing
static fields remain readable; consumers must use the typed assurance states
when present and must not infer execution from `verify_command`,
`commands_run`, `verify_result`, or receipt presence alone.

## Required fixture coverage

`fixtures/assurance_vocabulary/assurance/corpus.json` pins positive and
negative controls for:

- static improvement with no command run;
- unchanged static movement with a passing command;
- improved static movement with a failed command;
- unavailable verification;
- cancellation or timeout;
- malformed or unsupported `CommandSpecV1`;
- wrong repository root;
- fabricated result input; and
- externally supplied runtime mutation classification.

The corpus is contract evidence only. It does not execute commands, invoke a
provider, run mutation testing, or promote any state to a gate.

## Acceptance Examples

### Static-only improvement

An improved comparable static pair with `verification_not_run` may produce a
`receipt_issued` record only when its non-claims include static-only assurance.
It must not render `verified` or a runtime mutation outcome.

### Execution disagreement

An improved static pair with `verification_executed_fail` preserves both
states. Neither state overwrites the other, and the receipt cannot call the
repair runtime-verified.

### Invalid currentness

A wrong-root, stale, malformed, fabricated, or timed-out input is rejected or
disclosed as unavailable. It cannot become a passing current receipt.

## Required Evidence

- `schemas/ripr/repair-assurance.schema.json` closes the V1 envelope,
  `CommandSpecV1`, and execution-result vocabulary.
- `fixtures/assurance_vocabulary/assurance/corpus.json` contains positive and
  negative controls for every required assurance boundary.
- The regular `assurance_vocabulary` analyzer fixture proves this design-only
  corpus does not alter the existing no-finding fixture path.
- The spec, ADR, slice, indexes, traceability, and policy ledger remain
  consistent.

## Non-Goals

- implementing command execution;
- changing the current `agent verify` or `agent receipt` producer;
- adding signatures, remote attestation, or a policy gate;
- claiming test adequacy, correctness, merge approval, or runtime mutation;
- adding a second analyzer or provider/orchestrator authority; and
- promoting any language support tier.

## Claim boundary

This slice makes assurance vocabulary and migration boundaries durable. It
does not make command execution, receipt provenance, gate blocking, runtime
mutation confirmation, or proof authority available.

## Test Mapping

- `fixtures/assurance_vocabulary/SPEC.md` describes the fixture-level
  non-claims and static-only control.
- `fixtures/assurance_vocabulary/assurance/corpus.json` is the adversarial
  assurance corpus consumed by the future schema/runner implementation.
- The existing `cargo xtask fixtures assurance_vocabulary` path proves the
  regular fixture remains a zero-finding control.

## Implementation Mapping

- `schemas/ripr/repair-assurance.schema.json` owns the design-only machine
  contract.
- `docs/OUTPUT_SCHEMA.md` owns compatibility wording for current static
  `agent verify` and `agent receipt` surfaces.
- A future command runner and receipt issuer under #1979/#1941 must implement
  these states; this slice intentionally adds no production handler.

## Metrics

- `repair_assurance_vocabulary_closed`;
- `repair_assurance_static_only_disclosure`.

## Proof

```text
cargo xtask check-spec-format
cargo xtask check-spec-numbering
cargo xtask check-doc-index
cargo xtask check-doc-artifacts
cargo xtask check-fixture-contracts
cargo xtask check-output-contracts
cargo xtask check-command-catalog
git diff --check
```
