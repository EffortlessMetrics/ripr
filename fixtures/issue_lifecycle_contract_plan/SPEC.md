# Issue Lifecycle Contract/Plan Pilot Corpus

Spec: RIPR-SPEC-0232

## Given

The issue-lifecycle machinery (#4931, ISSUE-D3) needs a read-only pilot of
the contract/plan decision boundary over two REAL current issue snapshots:
one public output/API/schema or architecture-sensitive issue that requires a
proposal/spec/ADR decision, and one narrow accepted-contract bug that routes
directly to one PR. The contract case runs three distinct fixture roles —
contract author, independent adversary and root — and must preserve open
decisions, record the adversary's inspected scope and findings (or a bounded
`none_found`), and bind a root disposition in the closed RIPR-SPEC-0218
disposition vocabulary plus an accepted/amended/rejected/provisional
contract state. Both cases compile a bounded one-PR or campaign plan with
acceptance coverage, edit cages, semantic conflict resources, proof commands
with denominators, stop conditions and non-goals.

This directory commits the captured corpus (`corpus.json`), a clearly
labeled synthetic mechanics corpus (`controls.json`) carrying the ten
required decision-boundary controls outside the two real rows, a provenance
file recording capture time, base main SHA and per-row category rationale,
and the raw GitHub snapshots (`snapshots/`, one issue/comments/timeline
JSON triple per real row, digest-bound). Every embedded lifecycle attempt
row is an `issue_lifecycle_attempt.v1` row bound by its real `row_digest`,
so the RIPR-SPEC-0218 counting law assesses each row unchanged; the pilot
adds contract-case and planning evidence around that contract instead of a
parallel report type.

## When

`cargo xtask issue-lifecycle-contract-plan-scorecard [--corpus <dir>]`
(default `fixtures/issue_lifecycle_contract_plan`) loads the corpus,
verifies every snapshot digest against the committed snapshot bytes
(fail-closed: a drifted snapshot rejects the row), checks the two-category
shape, and projects the embedded attempt rows through the unchanged
RIPR-SPEC-0218 validator and scorecard builder, so results flow through
#4929 without a parallel contract/plan report. `cargo xtask
check-fixture-contracts` runs the dedicated validator: provenance
completeness, exactly two real rows with the closed category set, snapshot
file presence and digest binding, the decision-boundary law and the
synthetic-only invariant for mechanics control rows. `cargo test -p xtask
issue_lifecycle_contract_plan_pilot` exercises the ten required controls,
including the author-cannot-accept law, the adversary findings/none_found
exclusivity, the shape rejection controls and the behavior-versus-queue and
work-order-versus-authority laws.

## Then

- The two real rows retain their required evidence: the contract case
  (issue #6225) records the source-of-truth identity, the spec-required
  rationale, the draft spec identity, the author role/config/result
  identity (carrying no acceptance state), the independent adversary
  result with inspected scope and two missing failure/limited findings,
  the root role/config/result identity (carrying the acceptance state as
  the acceptance authority), the three preserved open decisions, the root
  disposition
  (`qualified_spec_required`) and the amended contract state; the narrow
  case (issue #6180) records no contract evidence at all.
- The author result never carries an acceptance state; only the root
  disposition records acceptance authority in the closed RIPR-SPEC-0218
  vocabulary.
- Both rows retain complete planning evidence: the shape decision with its
  recorded rejection rationale (a campaign was considered and rejected for
  #6225; one PR covers #6180), work-item identities with dependency edges
  that resolve inside the plan, acceptance rows covered or explicitly
  omitted with reasons, edit cages and semantic conflict resources, proof
  commands with denominators, stop conditions, non-goals, a portfolio
  placement label that writes no tracked selection or current-work file,
  and root corrections (the #6225 draft was amended per the adversary
  findings).
- The ten mechanics controls stay synthetic and outside the two real rows;
  they never enter a real denominator.

## Must Not

- Do not mutate GitHub or repository state: no comments, labels, assignees,
  milestones, claims, branches, merges or issue updates.
- Do not implement either plan, open a PR, or advance any closeout state.
- Do not let the author accept its own contract, upgrade a disposition,
  hide downgrade reasons, or count synthetic mechanics rows in a real
  denominator.
- Do not guess an unresolved decision: an open decision stays verbatim and
  binds a stop condition.
- Do not mint behavior authority from a plan or queue content from a spec:
  draft spec identities stay on the contract evidence and carry the
  `RIPR-SPEC-draft-` behavior-draft prefix.
- Do not reference or mutate a tracked selection or current-work file from
  any planning surface.
- Do not fabricate snapshot or retrieval byte counts: every measured claim
  binds the committed bytes and a drifted or fabricated count fails closed.
