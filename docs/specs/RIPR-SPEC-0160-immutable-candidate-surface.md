# RIPR-SPEC-0160: Immutable Git candidate CLI and output surface

Status: proposed

Issue: #3278 (parent #3237; builds on #3276 / #3277)

## Problem

The R2 object producer executes the typed immutable subject, but only
through internal library seams: no CLI flag names the input, and the
primary check JSON cannot prove *which* base/candidate trees were
analyzed. A consumer given `--candidate-tree T` output cannot
distinguish "analyzed T" from "analyzed HEAD by fallback" without
parsing command construction.

## Behavior

- CLI: `ripr check --candidate-tree <TREE>` binds the typed subject
  (#3276) with `--candidate-base <BASE>` (default: the repository's
  empty tree). Both flags are scope providers. `--diff` and `--base`
  conflicts fail at binding with the R1 named errors before any
  analysis; malformed OIDs/treeishes fail construction.
- Execution routes through the R2 producer exactly as the internal
  path does: identities resolve from Git objects, the diff is derived
  tree-to-tree, and the materialized candidate root is analyzed. The
  worktree and index are never consulted.
- Output: `analysis_outcome.outcome.identity` gains an additive
  `git_candidate_subject` object — `subject_kind` (`tree_to_tree`),
  `base_tree`, `candidate_tree`, and `diff_identity`
  (`sha256:<hex>` of the derived unified diff) — populated **only**
  when a subject ran, bound directly to the producer's resolved state
  (never inferred from argv). Ordinary runs emit `null`; no
  `schema_version` bump.
- Human output names the subject under `--verbose` (the resolved
  identity disclosure); the JSON is the machine surface.
- Fail-closed: an invalid, missing, or unsupported subject is a named
  `ExecutionFailed` error and can never render as clean zero findings.

## Required Evidence

- End-to-end CLI reproduction: a two-commit fixture repository; the
  JSON identity block carries the exact resolved base/candidate trees
  and a sha256 diff identity; dirtying the worktree (edit + staged
  blob) changes neither the identities nor the findings.
- CLI fail-closed controls: `--diff` conflict, `--base` conflict, and
  a malformed OID all fail naming the subject boundary.
- The app-layer identity projection test (`subject_kind`, both trees,
  `sha256:` diff identity).
- Golden blast radius: every fixture whose check JSON carries the
  outcome identity block gains exactly the additive
  `"git_candidate_subject": null` line (207 fixtures, all
  `formatting_only`); no classification, evidence-order, or missing
  drift.

## Required guards

- The identity fields bind to the producer's resolved state only;
  argv or config never populate them.
- A subject run in worktree or repo mode fails closed (#3277 review
  blocker, preserved).
- The diff identity is the SHA-256 **digest** of the derived diff, not
  the diff text (bounded identity fields).

## Acceptance Examples

- Accept: `--candidate-tree <T> --candidate-base <B>` → findings plus
  an identity block whose `candidate_tree` equals `T`'s tree OID.
- Reject: `--candidate-tree` with `--diff` or `--base`; a malformed
  OID; a missing tree.

## Test Mapping

`cli/commands/check.rs` `candidate_tree_tests`; the R2
`git_candidate_execution` tests; the schema doc entry in
`docs/OUTPUT_SCHEMA.md`.

## Non-Goals

- No LSP/staged-index behavior, Git hooks, #3212/#3213 semantic
  changes, stable shell-string promise, or release repin.
- No adversarial isolation/parity qualification (R4 / #3279).

## Implementation Mapping

- `cli/commands/check.rs` — the flags and the binding call.
- `analysis/pipeline.rs` — the resolved identity on the internal
  options clone.
- `analysis_outcome.rs` — the `GitCandidateSubjectIdentity` model.

## Metrics

No new metric; subject runs are countable by the presence of the
identity object.
