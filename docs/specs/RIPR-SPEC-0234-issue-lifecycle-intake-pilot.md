# RIPR-SPEC-0234: Issue lifecycle read-only intake pilot over six real issue snapshots

Status: proposed

Owner: test-infra

Created: 2026-10-04

Linked issues:

- #4930 (this slice: the read-only intake pilot over six real issue snapshots)
- #1650 (parent/controller: governed issue-to-closeout acceptance)
- #4929 / RIPR-SPEC-0218 (the frozen attempt contract this pilot consumes;
  referenced, never duplicated)
- #4931/#4932/#4933 (issue-lifecycle siblings; designed-for here,
  implemented there)

Support-tier impact:

- None.
- The pilot is an offline typed projection over committed, captured
  issue snapshots; it launches no agent, selects no work, edits nothing,
  calls no provider and reads no live GitHub state at validation time.

Policy impact:

- None. No new process, network or file-policy surface; the report reads the
  committed corpus and renders to stdout/`target/ripr/reports` only.

## Problem

RIPR-SPEC-0218 froze the issue-lifecycle attempt contract over synthetic
mechanics rows and explicitly deferred any real issue. Before later slices
decide contracts, plans or claims on live issues, the intake machinery needs
one honest exercise over exactly six REAL current issue snapshots: can the
frozen disposition vocabulary classify heterogeneous live rows — a narrow
accepted-contract bug, a duplicate, an overlapping-PR lane, a needs-evidence
case, a partially landed umbrella and an architecture/authority ambiguity —
with an independent root disposition per row, zero GitHub mutation, and
packet observations (bytes, triager findings, candidates, exact questions,
retrieval steps, limitations) complete enough for a cold-start root to
reconstruct each packet from the committed corpus alone?

## Behavior

`cargo xtask issue-lifecycle-intake-scorecard [--corpus <dir>]` (default
`fixtures/issue_lifecycle_intake`) loads the committed intake corpus
(`issue_lifecycle_intake_corpus.v1`: exactly six real rows over the closed
category set `narrow_accepted_contract_bug | duplicate_or_already_satisfied |
overlapping_open_pr | needs_evidence | partially_landed_umbrella |
root_decision_required`), the clearly labeled synthetic mechanics control
corpus, and the provenance file (capture timestamp, base main SHA, per-row
category justification). It verifies every snapshot digest against the
committed GitHub snapshot bytes fail-closed, assesses every row's packet
law, assesses every embedded `issue_lifecycle_attempt.v1` row through the
unchanged RIPR-SPEC-0218 counting law, and projects the six real attempts
through the unchanged `IssueLifecycleScorecardV1` builder, writing the
standard issue-lifecycle scorecard reports — results flow through #4929
without a parallel intake report. Any shape drift, digest drift, provenance
mismatch or counting-law rejection fails the gate.

Each intake row retains the #4930 required observations: snapshot identity
(issue, comments and timeline digests), current-main context, open
PR/claim candidates, packet selected/omitted/overflow bytes (measured, or
explicitly `not_measured` — a number is never fabricated), triager findings
with evidence references, duplicate/already-satisfied/stale/spec-needed
candidates, the exact missing-evidence questions, the independent root
disposition in the closed fifteen-value RIPR-SPEC-0218 vocabulary, the
artifact-archaeology retrieval steps with byte counts, and the limitations,
false candidates and non-claims.

Intake laws enforced by `assess_intake_row` and the fixture-contract gate:

- The root disposition is independent and never stronger than the attempt
  evidence: it must equal the assessed `IssueLifecycleDispositionV1` of the
  embedded attempt row.
- Packet missing-evidence questions and the attempt intake questions agree
  verbatim; a needs-evidence row keeps its exact questions and guesses no
  implementation work items.
- A duplicate disposition must name its duplicate-of candidate identity and
  evidence; corrected false candidates stay visible in `false_candidates`
  and `corrected_candidates` with the root note retained.
- A row binding an overlapping open PR candidate is blocked: a real
  overlapping PR blocks duplicate pickup.
- Movement law: movement of any identity a row binds (current main, snapshot,
  PR, claim, merge) invalidates the row; unrelated movement does not.
- Selection law: no `active.toml`, label, issue age or title silently
  selects work; the packet projection is a pure function of committed corpus
  fields and never reads the GitHub snapshot body.
- Byte law: every packet byte surface is present on every row as a measured
  number or an explicit `not_measured`; a measured selected count must equal
  the committed snapshot bytes, and a fabricated or stale count fails closed.

## Required Evidence

- The committed corpus `fixtures/issue_lifecycle_intake/` with the six real
  rows (issues #5439, #5440, #5399, #5373, #2928, #5441 at capture base
  main `a88034ea3c3d6baae360d6d374bf547f5f9b4d58`), the synthetic mechanics
  control corpus (false-duplicate correction, overlapping-PR pickup block,
  explicit `not_measured` bytes), provenance, and the immutable
  issue/comments/timeline snapshots (one host-local worktree path in the
  #5399 capture is redacted to `<redacted-local-worktree-path>`; the recorded
  digests bind the committed redacted bytes).
- `cargo test -p xtask issue_lifecycle_intake_pilot` coverage of the ten
  required controls.
- `cargo xtask check-fixture-contracts` gate coverage: shape, provenance,
  snapshot digest bindings, packet law, synthetic-only controls.

## Non-Goals

No spec draft, work-item plan, claim, source edit, PR, issue update,
closeout, broad corpus, triage universality, duplicate-detection accuracy
beyond these six rows, priority ranking, automation-default decision,
portfolio architecture or policy ratification. This contract does not run
planning, implementation or closeout on any issue and does not establish
that intake dispositions are accurate or useful beyond the captured rows.

## Acceptance Examples

- The narrow one-PR bug row (#5439) qualifies as `qualified_one_pr` with no
  contract artifacts: spec bureaucracy is skipped, not deferred.
- The duplicate row (#5440) names #5270 as its duplicate-of identity with
  the shared failing-test evidence and stays a first-class counted row that
  claims no implementation success.
- The overlapping-PR row (#5399) stays `blocked` with the collision event
  and the open PR identity retained; the control corpus shows a real
  overlapping PR blocking duplicate pickup, and a corrected false duplicate
  stays visible with its root note.
- The needs-evidence row (#5373) keeps its exact open questions and no
  guessed implementation; the partially landed umbrella (#2928) retains its
  uncovered burn-down rows and binds the merged child identity without
  advancing closeout; the architecture ambiguity (#5441) stops at
  `root_decision_required` with no contract artifact drafted.
- Relevant portfolio movement (a bound identity changes) invalidates the
  affected row while unrelated movement leaves every row intact, and a
  cold-start reload from the committed corpus alone reconstructs
  byte-identical packets with no chat.

## Test Mapping

- `xtask/src/issue_lifecycle_intake.rs::tests` — the ten intake controls:
  committed six-category corpus shape; false duplicate corrected by root
  review; overlapping PR blocks duplicate pickup; narrow issue skips spec
  bureaucracy; architecture ambiguity stops at `root_decision_required`;
  missing evidence yields exact questions; partial umbrella retains
  uncovered acceptance; duplicate row names evidence and stays first-class;
  relevant movement invalidates, unrelated does not; no `active.toml`,
  label, age or title selects work; cold-start reconstruction; packet bytes
  complete or explicitly `not_measured`.
- `xtask/src/fixture_contracts/general_validators.rs` —
  `validate_issue_lifecycle_intake_fixture_corpus` gate coverage over the
  committed corpus, controls, provenance and snapshot bindings.

## Implementation Mapping

- `xtask/src/issue_lifecycle_intake.rs` — intake DTO family, corpus and
  control loaders, snapshot digest verification, packet law, packet
  projection, scorecard command.
- `fixtures/issue_lifecycle_intake/` — the real corpus, control corpus,
  provenance, snapshots and manifest.
- `xtask/src/fixture_contracts/general_validators.rs`,
  `xtask/src/fixture_contracts/mod.rs` — the fixture-contract gate.
- `xtask/src/command.rs`, `xtask/src/dispatch.rs` — command registration.

## CI Proof

```bash
cargo test -p xtask issue_lifecycle_intake_pilot -- --nocapture
cargo test -p xtask issue_lifecycle
cargo xtask issue-lifecycle-intake-scorecard
cargo xtask check-fixture-contracts
cargo xtask check-output-contracts
cargo xtask check-static-language
cargo xtask check-local-context
cargo xtask check-file-policy
cargo xtask check-spec-numbering
cargo xtask check-traceability
cargo xtask precommit
git diff --check
```

## Metrics

- `issue_lifecycle_intake_real_rows`
- `issue_lifecycle_intake_control_rows`
- `issue_lifecycle_intake_snapshot_bindings`

## Failure Modes

- A row with a drifted or hand-edited snapshot digest rejects; it cannot
  enter any denominator.
- A corpus missing one of the six categories, or carrying a seventh real
  row, fails the gate.
- A root disposition stronger than the assessed attempt disposition fails
  the gate; no claimed disposition is ever upgraded.
- A mechanics control row that is not synthetic, or a synthetic row among
  the six real rows, fails the gate.
- A packet byte surface left implicit fails the gate; `not_measured` is the
  only honest absent measurement.
