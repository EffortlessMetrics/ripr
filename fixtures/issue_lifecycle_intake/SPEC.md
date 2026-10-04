# Issue Lifecycle Intake Pilot Corpus

Spec: RIPR-SPEC-0223

## Given

The issue-lifecycle intake machinery (#4930, ISSUE-D2) needs its first
real-corpus exercise before any contract or mutation work: exactly six REAL
current issue snapshots, captured immutably and read-only, classified through
the frozen RIPR-SPEC-0218 disposition vocabulary with an independent root
disposition per row. The six row categories are: a narrow accepted-contract
bug; a duplicate or already-satisfied case; an issue with an overlapping open
PR; a needs-evidence case; a partially landed umbrella; and an
architecture/authority ambiguity requiring a root decision.

This directory commits the captured corpus (`corpus.json`), a clearly
labeled synthetic mechanics corpus (`controls.json`), a provenance file
recording capture time, base main SHA and per-row category rationale, and the
raw GitHub snapshots (`snapshots/`, one issue/comments/timeline JSON triple
per row; one host-local worktree path inside the #5399 capture is redacted to
`<redacted-local-worktree-path>` because durable files must not carry real
machine paths — the corpus digests and retrieval byte counts bind the
committed redacted bytes). Every embedded lifecycle attempt row is an
`issue_lifecycle_attempt.v1` row bound by its real `row_digest`, so the
RIPR-SPEC-0218 counting law assesses each row unchanged; the intake pilot
adds packet observations around that contract instead of a parallel report
type.

## When

`cargo xtask issue-lifecycle-intake-scorecard [--corpus <dir>]` (default
`fixtures/issue_lifecycle_intake`) loads the corpus, verifies every snapshot
digest against the committed snapshot bytes (fail-closed: a drifted snapshot
rejects the row), checks the six-category shape, and projects the embedded
attempt rows through the unchanged RIPR-SPEC-0218 validator and scorecard
builder, so results flow through #4929 without a parallel intake report.
`cargo xtask check-fixture-contracts` runs the dedicated validator:
provenance completeness, exactly six real rows with the closed category set,
snapshot file presence and digest binding, and the synthetic-only invariant
for mechanics control rows. `cargo test -p xtask issue_lifecycle_intake_pilot`
exercises the ten required controls, including the false-duplicate
correction and the relevant-versus-unrelated portfolio movement rules over
the committed control corpus.

## Then

- The six real rows retain their required observations: snapshot identity,
  current-main context, open PR/claim candidates, packet
  selected/omitted/overflow bytes (measured, or explicitly `not_measured`;
  the measured selected count is bound to the committed snapshot bytes and a
  fabricated count fails closed), triager findings,
  duplicate/already-satisfied/stale/spec-needed candidates, exact
  missing-evidence questions, the independent root disposition,
  artifact-archaeology retrieval steps, and limitations/false
  candidates/non-claims.
- Each row's root disposition is expressed in the closed fifteen-value
  RIPR-SPEC-0218 vocabulary and matches its embedded attempt disposition.
- The narrow one-PR row (#5439) carries no contract artifacts: spec
  bureaucracy is skipped, not deferred.
- The duplicate row (#5440) names its duplicate-of identity (#5270) and the
  evidence for it; the false-duplicate correction control demonstrates a
  corrected candidate retained verbatim.
- The overlapping-PR row (#5399) stays `blocked` with the collision event
  and PR identity retained; the control corpus shows a real overlapping PR
  blocking duplicate pickup.
- The needs-evidence row (#5373) retains its exact open questions and stays
  `needs_evidence`; no implementation is guessed.
- The partially landed umbrella (#2928) keeps its uncovered burn-down rows
  and the merged-child identity that does not advance closeout.
- The architecture ambiguity (#5441) stops at `root_decision_required` with
  no contract artifact drafted and no recommendation recorded.
- Mechanics controls stay synthetic and outside the six real rows; they
  never enter a real denominator.

## Must Not

- Do not mutate GitHub or repository state: no comments, labels, assignees,
  milestones, claims, branches, merges or issue updates.
- Do not select work: no `active.toml`, label, issue age or title silently
  selects a row; the packet projection reads only committed corpus fields.
- Do not upgrade a disposition, hide downgrade reasons, or count synthetic
  mechanics rows in a real denominator.
- Do not fabricate packet bytes: every byte count is measured or explicitly
  `not_measured`.
- Do not invent missing evidence: an unanswered question stays an exact
  question.
- Do not let Markdown or any prose projection strengthen machine state.
