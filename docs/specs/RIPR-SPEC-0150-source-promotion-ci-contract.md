# RIPR-SPEC-0150: Source-promotion CI contract

Status: proposed

## Problem

The exact-J verifier is useful only when the source PR invokes it against the
live PR head and makes the history-preserving merge method unambiguous. A valid
join somewhere in ancestry, a stale PR body, or a flattened equivalent tree
must not produce a promotion success.

## Behavior

The `Source Promotion Contract` workflow runs only for a pull request whose body
contains `<!-- source-promotion: true -->` and exactly one
`<!-- source-promotion-control: <40 lowercase hex> -->` marker. It checks out
the exact PR head with full history, fetches the named control commit from the
fixed `EffortlessMetrics/ripr` source repository, and consumes only these fixed
paths from that immutable commit:

- `docs/release/source-promotion/contract-inputs.json`;
- `docs/release/source-promotion/preflight.json`; and
- `docs/release/source-promotion/resolution-manifest.json`.

The control manifest uses schema `ripr.source_promotion_ci_inputs.v2` and binds
exact `source_main`, `join_head`, fixed paths, and the SHA-256 digest of each
receipt. `join_head` must equal the live PR head and `source_main` must equal
the live PR base. The control files are external to J's tree: the workflow
rejects candidate-provided paths and does not require a tracked input file in
the promotion PR. This avoids a circular fixed-point contract in which a file
containing J also changes `J^{tree}`.

The workflow runs `cargo xtask source-promotion verify` and emits one
normalized `ripr.source_promotion_contract.v2` receipt plus the verifier
JSON/Markdown receipts. Every receipt is bound to the PR head, control commit,
and input digests and is uploaded under a SHA-containing artifact name.

Before invoking the verifier, the workflow resolves the declared `swarm_ref`
with `git ls-remote --refs` against the fixed public
`EffortlessMetrics/ripr-swarm` remote and requires exactly one result whose SHA
equals `SWARM_PARENT`. It also reads fixed public ruleset `20661783` and
requires target `tag`, active enforcement, the singleton
`refs/tags/ripr-release-*` pattern, and both active `update` and `deletion`
rules. These checks are external provenance, not receipt-controlled
self-consistency.

The workflow summary repeats the ordered parent graph, candidate/source SHAs,
merge base, tree, receipt digests, preflight conflict/survivor/version state,
and the only supported copy-safe command:

```bash
gh pr merge <PR> --repo EffortlessMetrics/ripr \
  --merge \
  --match-head-commit <JOIN_SHA>
```

The summary explicitly requires Create a merge commit and forbids squash and
rebase. The command is printed only; the workflow never executes it. A manual
workflow-dispatch lane reruns the verifier with `--main-head` and requires the
exact J object to remain reachable from merged source `main`.

## Required Evidence

- unrelated PRs skip without a success claim;
- a stale or substituted `join_head`, control commit, receipt digest, body
  command SHA, fixed receipt path, or source parent fails closed;
- missing, abbreviated, uppercase, unreachable, wrong-repository, symlinked,
  directory, and placeholder control inputs fail closed;
- a valid two-parent join passes the verifier;
- a single-parent or equivalent-tree flattened history fails post-merge;
- uploaded receipts retain exact heads, ordered parents, input digests, checks,
  failure reasons, and claim boundaries.
- the verifier is built from the trusted source-parent SHA in an isolated target
  and invoked against the candidate checkout; the candidate cannot supply it.

## Non-Goals

## Acceptance Examples

- A marked promotion PR with one exact source-control marker, canonical
  mode-100644 sidecar files, matching digests, and a numeric,
  repository-bound `gh pr merge` command passes the PR-head verifier lane.
- Missing, duplicate, abbreviated, uppercase, unreachable, wrong-repository,
  symlinked, directory, or placeholder sidecar inputs, candidate-tree input
  paths, and flattened ancestry are rejected.
- Duplicate or mixed merge strategies are rejected, and the sole command must use `--merge`.
- An unrelated PR skips the promotion lane honestly, while manual `--main-head` verification proves the post-merge ancestry contract.

No automatic merge, publication, release, tag, signing, secret use, settings,
branch-protection mutation, ref mutation, or product-correctness claim.

## Test Mapping

Executable proof lives in
`xtask/src/command.rs::tests::source_promotion_workflow_is_exact_head_and_read_only`,
`source_promotion_workflow_rejects_symlink_and_path_escape_inputs`,
`source_promotion_workflow_rejects_placeholder_and_wrong_repo_commands`,
`source_promotion_workflow_disables_checkout_credentials_before_code`, and
`source_promotion_workflow_refutes_crlf_rewrite_thread`. The exact-J graph and
flattened-history controls remain covered by the verifier tests mapped in
`.ripr/traceability.toml`.

## Implementation Mapping

- Workflow: `.github/workflows/source-promotion-contract.yml`
- Operator contract: `docs/SOURCE_PROMOTION.md`
- Exact verifier: `xtask/src/reports/source_promotion_verify.rs`
- External control sidecar: the fixed source-repository commit named by the
  PR-body `source-promotion-control` marker; its files never enter J.

## Metrics

The workflow receipt denominator is the number of promotion-specific runs; a
skipped unrelated PR is not a passing promotion result. GitHub check state and
receipt status remain distinct evidence axes.
