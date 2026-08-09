# RIPR-SPEC-0145: Historical 0.11 candidate execution-surface scope

Status: accepted

## Problem

PR #2396 merged the `ripr agent verify-execute` surface into development
`main`, while the accepted 0.11 authority says that 0.11 does not execute
verification commands or issue `RepairReceiptV2`. Candidate selection must make
that boundary explicit without rewriting development history.

This specification is retained for audit of the former C/T authority. It is
not the active 0.11.0 publication rule; the live-head decision is recorded in
`docs/release-candidates/0.11.0-live-head-selection.json`.

## Behavior

The historical candidate-only scope report replays a captured Outcome A decision, verifies
that the named #2396 commit and its complete changed-path inventory are
available, checks that the candidate parent has not drifted, and records the
execution paths excluded from the candidate. It keeps #2332 open and never
constructs or mutates a candidate tree.

## Input contract

`fixtures/release_scope/accepted-outcome-a.json` is a schema `0.1`
`release_execution_scope` input. It names the candidate parent ref and SHA, the
execution commit, strictly dependent commits, exact execution-only paths,
candidate-excluded paths, preserved provenance/static-assurance paths, the
accepted non-claim, and the open state of #2332.

## Output contract

`cargo xtask release-scope --input <scope.json>` writes one normalized JSON
report and a Markdown projection to `target/ripr/reports/release-scope.{json,md}`.
The report is `ready` only when the current Git observations agree with the
captured decision; stale parents, missing commits, path drift, partial
exclusions, or a closed #2332 produce `reconcile_required` and a non-zero exit.

## Required Evidence

- the exact #2396 commit is present and its changed paths match the captured
  execution-only inventory;
- candidate exclusion covers every execution-only path;
- preserved provenance/static-assurance paths exist at the candidate parent and
  are outside the exclusion;
- the candidate parent ref resolves to the captured SHA;
- #2332 remains open for undelivered execution acceptance;
- the report explicitly says that no candidate tree was constructed.

## Non-Goals

- no candidate-tree construction, branch mutation, history rewrite, merge, or
  publication;
- no verification-command execution or receipt issuance;
- no candidate qualification or package/source handoff;
- no closure or mutation of #2332, #2379, #1609, or related GitHub state.

## Acceptance Examples

### Complete Outcome A input is ready

```text
Given the current candidate parent and the complete #2396 path inventory,
when release-scope replays Outcome A,
then the report is ready and excludes every execution-only path while retaining
the accepted 0.11 non-claim.
```

### Partial exclusion fails closed

```text
Given one #2396 path is missing from candidate_excluded_paths,
when the report is normalized,
then status is reconcile_required and candidate qualification is not claimed.
```

### Candidate-parent drift fails closed

```text
Given origin/main no longer resolves to the captured candidate_parent_sha,
when the report is normalized,
then status is reconcile_required and the stale decision is not ready.
```

## Test Mapping

- `xtask/src/reports/release_scope.rs::tests::accepted_scope_normalizes_captured_git_observation`
  — complete Outcome A input normalizes a captured commit and full path
  inventory; the runnable command performs the live Git verification.
- `xtask/src/reports/release_scope.rs::tests::a_missing_excluded_path_fails_closed`
  — partial candidate exclusion is rejected.
- `xtask/src/reports/release_scope.rs::tests::a_changed_commit_path_fails_closed`
  — fabricated execution paths cannot pass reconciliation.
- `xtask/src/reports/release_scope.rs::tests::closed_execution_issue_fails_closed`
  — closing #2332 cannot turn an incomplete execution surface into an accepted
  scope.
- `xtask/src/reports/release_scope.rs::tests::duplicate_strictly_dependent_commits_fail_closed`
  — duplicate dependent-commit inventory is rejected instead of being silently
  normalized away.
- `xtask/src/reports/release_scope.rs::tests::report_json_has_an_explicit_authority_boundary`
  — the report does not claim candidate construction or qualification.

## Implementation Mapping

- `xtask/src/reports/release_scope.rs` owns input validation, Git observation,
  decision normalization, and JSON/Markdown projections.
- `xtask/src/command.rs`, `xtask/src/dispatch.rs`, and
  `xtask/src/reports/mod.rs` expose the report-only command.
- `xtask/src/fixture_contracts/mod.rs` validates the fixture corpus shape.
- `fixtures/release_scope/accepted-outcome-a.json` carries the captured
  candidate-only decision.
- `plans/release-control-0-11/implementation-plan.md` sequences this decision
  before the denominator and exact-candidate bundle slices.

## Metrics

- `status` counts ready versus reconciliation-required scope decisions;
- `checks.execution_paths_match_commit` records exact path-denominator proof;
- `checks.exclusion_is_complete` records candidate-only exclusion completeness;
- `candidate_tree_delta.candidate_tree_constructed` remains false in this
  report-only slice.

## Proof

```text
cargo test -p xtask release_scope -- --nocapture
cargo xtask release-scope --input fixtures/release_scope/accepted-outcome-a.json
cargo xtask check-fixture-contracts
cargo xtask check-output-contracts
cargo xtask check-traceability
```

## Claim boundary

This spec proves only that one captured Outcome A scope decision is complete,
current, and internally consistent against Git. It does not prove that a
candidate tree has been built, that the candidate qualifies, that execution is
safe, or that a release is approved.
