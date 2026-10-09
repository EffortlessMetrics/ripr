# RIPR-SPEC-0183: Executed-control obligation and result contract

Status: proposed

Owner: product / swarm

Created: 2026-09-29

Linked issues:

- [#4641](https://github.com/EffortlessMetrics/ripr-swarm/issues/4641) — EC01 contract and schema authority
- [#4200](https://github.com/EffortlessMetrics/ripr-swarm/issues/4200) — parent: executed-control acceptance has no verification
- [#4644](https://github.com/EffortlessMetrics/ripr-swarm/issues/4644) — later closeout enforcement (out of this spec)
- [#4646](https://github.com/EffortlessMetrics/ripr-swarm/issues/4646) — later current-cycle audit (out of this spec)
- [#3858](https://github.com/EffortlessMetrics/ripr-swarm/issues/3858) / [#4063](https://github.com/EffortlessMetrics/ripr-swarm/issues/4063) — documentation fixture of not-established execution

Support-tier impact:

- None. This spec adds a closed obligation/result vocabulary, validator, and
  bounded projection. It does not promote a language, editor surface, gate, or
  public support claim.
- Claim boundaries remain governed by
  [support tiers](../status/SUPPORT_TIERS.md).

Policy impact:

- Register this spec in `policy/doc-artifacts.toml` and
  `.ripr/traceability.toml`.
- Publish `schemas/ripr/executed-control.schema.json` and register it in the
  verification-contract inventory.
- No new crates, binaries, CLI commands, workflow policy, network allowlist
  rows, process spawners, or support-tier changes.

## Problem

Issue acceptance often names an executed discriminating control — run the
removal variant, retain the red output, prove the fixture fails against a
deliberately wrong implementation. That requirement currently lives in prose.
A passing ordinary test, a review argument, or a statement that a fixture is
structurally discriminating can be mistaken for execution. #3858 required the
eager / removed-guard experiment; PR #4063 merged and the issue closed without
a retained execution artifact. The repository needs one machine-readable
contract so later closeout enforcement can consume obligations without parsing
issue prose.

## Behavior

`executed_control_obligation.v1` and `executed_control_result.v1` are the closed
vocabulary. A packet binds them for set-level validation. Human Markdown and
machine JSON derive from the same packet object.

An obligation records:

```text
obligation_id
owning claim
exact control class
intended wrong implementation or removed guard
required execution subject (command/instrument, named wrong implementation, head)
expected discriminating outcome
acceptable evidence forms
permitted substitute, if any
requiredness
invalidators
```

A result records:

```text
obligation_id
source / candidate / head identity
command or instrument identity
artifact identity
observed outcome
offered evidence kind
state = passed | failed | not_run | not_proven | substituted | instrument_failure
limitation / reason
obligation digest
```

Laws:

- `passed` requires evidence that the named wrong implementation was actually
  exercised and rejected for the intended reason.
- An ordinary positive test, review prose, or structural-discrimination claim
  cannot satisfy an executed-control obligation.
- `not_run`, `not_proven`, `substituted`, and `instrument_failure` remain
  explicit. Only a substitute declared on the obligation can satisfy in place
  of a pass.
- Evidence binds exact source/head, command or instrument, and retained
  artifact identity. A stale digest, other-head result, or foreign
  `source_identity` cannot pass. `passed` requires an artifact-backed form
  listed on the obligation; a named substitute cannot satisfy with
  `not_executed` / ordinary-test / review / structural outcomes. Conflicting
  passed and failed results for the same head and command are rejected.
- Volatile timestamps, machine paths, and log ordering are not semantic
  identity. Serialization canonicalizes obligation and result order.
- #3858 / #4063 is retained as a documentation fixture whose execution state
  is `not_proven`. It must not be rewritten as `passed`.

The contract is reusable by later PR-closeout enforcement. This spec does not
inspect live GitHub, enforce merge eligibility, or audit historical issues.

## Required Evidence

- `schemas/ripr/executed-control.schema.json` closes the V1 wire shape.
- `crates/ripr/src/domain/executed_control.rs` owns validation and semantic
  identity.
- `crates/ripr/src/output/executed_control.rs` owns deterministic JSON and
  bounded Markdown projections of the same packet.
- `fixtures/executed-control-contract/corpus.json` covers the required
  positive, negative, determinism, and #3858 documentation cases.

## Non-Goals

- general mutation-testing platforms
- automatic extraction of arbitrary prose requirements
- retroactive claim that #3858 was run
- workflow policy, merge-gate, or GitHub mutation
- live issue/PR query or historical audit (#4646)
- PR-closeout enforcement (#4644)
- release or publication action
- product analyzer behavior change

## Acceptance Examples

### Fail-before / pass-after removal control

A required `removed_guard` obligation with a failed result on the unrepaired
head and a `passed` result on the repaired head, each with retained artifact
identity and executed-control evidence, validates and satisfies.

### Ordinary positive test offered as the control

A `passed` result whose offered evidence is `ordinary_positive_test` is
rejected. The obligation is not satisfied.

### #3858 documentation fixture

The obligation for the eager file-count removal control has a `not_proven`
result, no retained artifact, and an explicit limitation that PR #4063 did
not record execution. Projection renders `not_proven` and `does_not_satisfy`.
It does not render `passed`.

## Test Mapping

- Fixture: `fixtures/executed-control-contract/`
- Unit: `crates/ripr/src/domain/executed_control.rs::tests`
- Projection: `crates/ripr/src/output/executed_control.rs::tests`
- Golden/output contract: `fixtures/executed-control-contract/expected/`
- Proof command: `cargo test -p ripr --lib executed_control`

## Implementation Mapping

- `crates/ripr/src/domain/executed_control.rs` — obligation/result/packet types
  and fail-closed validation
- `crates/ripr/src/output/executed_control.rs` — JSON and Markdown projection
- `schemas/ripr/executed-control.schema.json` — published wire contract
- `.allow/spec-system/slices/executed-control-obligation.v1.toml` — PR-local
  claim boundary for #4641
- `docs/OUTPUT_SCHEMA.md` — published schema row and field documentation

## Metrics

- `executed_control_obligation_vocabulary_closed`
- `executed_control_pass_requires_executed_evidence`
- `executed_control_projections_deterministic`
