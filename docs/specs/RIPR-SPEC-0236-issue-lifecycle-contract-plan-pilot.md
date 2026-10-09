# RIPR-SPEC-0236: Issue lifecycle contract/plan decision-boundary pilot over two real issue snapshots

Status: proposed

Owner: test-infra

Created: 2026-10-04

Linked issues:

- #4931 (this slice: the read-only contract/plan decision-boundary pilot)
- #1650 (parent/controller: governed issue-to-closeout acceptance)
- #4929 / RIPR-SPEC-0218 (the frozen attempt contract this pilot consumes;
  referenced, never duplicated)
- #4930 / RIPR-SPEC-0234 (the sibling intake pilot; this pilot extends the
  same machinery and reuses its snapshot/retrieval DTOs)
- #1645 (contract author/challenge authority) / #1646 (work-item planning
  authority)

Support-tier impact:

- None.
- The pilot is an offline typed projection over committed, captured issue
  snapshots; it launches no agent, selects no work, edits nothing, calls no
  provider and reads no live GitHub state at validation time.

Policy impact:

- None. No new process, network or file-policy surface; the report reads
  the committed corpus and renders to stdout/`target/ripr/reports` only.

## Problem

RIPR-SPEC-0234 exercised intake over six real issue snapshots; #4931 now
asks where the boundary between contract-required work and direct one-PR
work actually lies. Before any live contract drafting, challenge or
planning runs against real issues, the machinery needs one honest exercise
over exactly two REAL current issue snapshots: can a public
output/API/schema or architecture-sensitive issue with a genuinely
unresolved decision traverse author, independent adversary and root roles —
with open decisions preserved and a root disposition recorded in the frozen
RIPR-SPEC-0218 vocabulary — while a narrow accepted-contract bug routes
directly to one PR with zero contract bureaucracy? And can both cases
compile a bounded one-PR or campaign plan whose acceptance coverage, edit
cages, conflict resources, proof commands, stop conditions and non-goals a
cold-start root can reconstruct from the committed artifacts alone?

## Behavior

`cargo xtask issue-lifecycle-contract-plan-scorecard [--corpus <dir>]`
(default `fixtures/issue_lifecycle_contract_plan`) loads the committed
contract/plan corpus (`issue_lifecycle_contract_plan_corpus.v1`: exactly
two real rows over the closed category set `contract_required |
narrow_accepted_contract_bug`), the clearly labeled synthetic mechanics
control corpus, and the provenance file (capture timestamp, base main SHA,
per-row category justification). It verifies every snapshot digest against
the committed GitHub snapshot bytes fail-closed, assesses every row's
decision-boundary law, assesses every embedded `issue_lifecycle_attempt.v1`
row through the unchanged RIPR-SPEC-0218 counting law, and projects the two
real attempts through the unchanged `IssueLifecycleScorecardV1` builder,
writing the standard issue-lifecycle scorecard reports — results flow
through #4929 without a parallel contract/plan report. Any shape drift,
digest drift, provenance mismatch or counting-law rejection fails the gate.

The contract-required row retains the full contract-case evidence:
source-of-truth identity, spec-required rationale, draft spec identity
(`RIPR-SPEC-draft-` behavior-draft prefix), author role/config/result
identity, independent adversary result with inspected scope and concrete
missing failure/limited findings or a bounded `none_found`, preserved open
decisions, the root disposition in the closed fifteen-value RIPR-SPEC-0218
vocabulary, and the accepted/amended/rejected/provisional contract state.
Both rows retain the planning evidence: one-PR versus campaign decision
with the recorded rejection rationale, work-item identities with dependency
edges that resolve inside the plan, acceptance rows covered or explicitly
omitted with reasons, edit cages and semantic conflict resources, proof
commands with denominators, stop conditions, non-goals, portfolio placement
without a tracked selection/current-work mutation, and root corrections and
re-splits.

Decision-boundary laws enforced by `assess_contract_plan_row` and the
fixture-contract gate:

- Role separation: the author result never carries an acceptance state; a
  contract can only be accepted, amended, rejected or left provisional by
  the root disposition, and the root disposition must equal the assessed
  RIPR-SPEC-0218 disposition of the embedded attempt row.
- Adversary honesty: findings and bounded `none_found` are mutually
  exclusive, and the inspected scope is always recorded.
- Unresolved decisions block implementation: open decisions stay verbatim,
  bind a stop condition, and cap every disposition below
  `qualified_one_pr`; acceptance evidence cannot outrun the root
  disposition.
- Narrow routing: a narrow accepted-contract bug carries no contract
  evidence, no contract artifacts, `spec_required: false` and a one-PR
  shape.
- Shape honesty: a campaign is rejected when one vertical slice covers
  acceptance (rationale recorded), and a one-PR plan is rejected when its
  acceptance rows cannot be covered coherently; the decision and its
  rejection rationale stay on the row.
- Specs contain behavior, not execution queues: a draft identity must carry
  the `RIPR-SPEC-draft-` behavior-draft prefix; queue-shaped identities
  fail closed.
- Plans contain work order, not new behavior authority: no planning surface
  may mint a draft spec identity; draft specs live on the contract evidence
  only.
- No tracked selection or current-work file is referenced or mutated by any
  planning surface; portfolio placement is a label.
- Byte law: every retrieval-step byte claim binds the committed snapshot
  bytes; a fabricated or stale count fails closed.

## Required Evidence

- The committed corpus `fixtures/issue_lifecycle_contract_plan/` with the
  two real rows (issues #6225 and #6180 at capture base main
  `c23c6ba24eb719f1ad3990e6d921a6888b69607e`), the synthetic mechanics
  control corpus covering the ten required decision-boundary controls,
  provenance, and the immutable issue/comments/timeline snapshots.
- `cargo test -p xtask issue_lifecycle_contract_plan_pilot` coverage of the
  ten required controls.
- `cargo xtask check-fixture-contracts` gate coverage: shape, provenance,
  snapshot digest bindings, decision-boundary law, synthetic-only controls.

## Non-Goals

No spec draft lands as a numbered spec, no work-item plan executes, no
claim, source edit, PR, issue update, closeout, broad corpus, decision
correctness beyond the two captured rows, contract-author tooling, live
adversary automation, portfolio architecture or policy ratification. This
contract does not establish that generated contracts or plans are generally
correct, and it does not run implementation on either issue.

## Acceptance Examples

- The contract case (#6225) carries complete author/adversary/root evidence
  with three open decisions preserved, two adversary findings (a missing
  failure state and a limited state), a root disposition of
  `qualified_spec_required` and an amended contract state; the embedded
  attempt stays `qualified_spec_required` through the unchanged counting
  law.
- The narrow case (#6180) routes directly to one PR: no contract evidence,
  no contract artifacts, `qualified_one_pr`, all three acceptance rows
  covered, nothing omitted.
- The shape controls record a rejected campaign when one slice suffices and
  a rejected one-PR plan when acceptance cannot be covered coherently.
- Mutations fail closed: an author carrying acceptance, a queue-shaped
  draft identity, a plan minting behavior authority, a selection-file
  reference, a fabricated retrieval byte count, a downgraded root
  disposition and an extra provenance row each reject the gate.
- A cold-start reload from the committed corpus alone reconstructs
  byte-identical contract/plan projections with no chat and no live read.

## Test Mapping

- `xtask/src/issue_lifecycle_contract_plan.rs::tests` — the
  `issue_lifecycle_contract_plan_pilot` filter: two-category corpus shape;
  contract case author/adversary/root separation; narrow case direct
  one-PR routing; adversary findings/none_found exclusivity;
  author-cannot-accept; unresolved-decision blocks implementation;
  campaign/one-PR rejection controls; specs-behavior/plans-work-order
  laws; no tracked selection file; cold-start reconstruction; fabricated
  retrieval bytes fail closed; root disposition downgrade rejected; extra
  provenance row rejected.
- `xtask/src/fixture_contracts/general_validators.rs` —
  `validate_issue_lifecycle_contract_plan_fixture_corpus` gate coverage
  over the committed corpus, controls, provenance and snapshot bindings.

## Implementation Mapping

- `xtask/src/issue_lifecycle_contract_plan.rs` — contract/plan DTO family,
  corpus and control loaders, snapshot digest verification,
  decision-boundary law, packet projection, scorecard command.
- `fixtures/issue_lifecycle_contract_plan/` — the real corpus, control
  corpus, provenance, snapshots and manifest.
- `xtask/src/fixture_contracts/general_validators.rs`,
  `xtask/src/fixture_contracts/mod.rs` — the fixture-contract gate.
- `xtask/src/command.rs`, `xtask/src/dispatch.rs`, `xtask/src/main.rs` —
  command registration.

## CI Proof

```bash
cargo test -p xtask issue_lifecycle_contract_plan_pilot -- --nocapture
cargo test -p xtask issue_lifecycle
cargo xtask issue-lifecycle-contract-plan-scorecard
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

- `issue_lifecycle_contract_plan_real_rows`
- `issue_lifecycle_contract_plan_control_rows`
- `issue_lifecycle_contract_plan_snapshot_bindings`

## Failure Modes

- A row with a drifted or hand-edited snapshot digest rejects; it cannot
  enter any denominator.
- A corpus missing one of the two categories, or carrying a third real
  row, fails the gate.
- A contract root disposition stronger than the assessed attempt
  disposition fails the gate; no claimed disposition is ever upgraded.
- An author result carrying acceptance, a findings-plus-none_found
  adversary, a queue-shaped draft identity, a plan minting behavior
  authority, a dangling dependency edge, an acceptance omission without a
  reason, or a selection-file reference fails the gate.
- A mechanics control row that is not synthetic, or a synthetic row among
  the two real rows, fails the gate.
- A retrieval byte claim that disagrees with the committed snapshot bytes
  fails the gate; a fabricated count fails closed.
