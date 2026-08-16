# RIPR-SPEC-0150: Source-promotion CI contract

Status: proposed

## Problem

The exact-J verifier is useful only when the source PR invokes it against the
live PR head and makes the history-preserving merge method unambiguous. A valid
join somewhere in ancestry, a stale PR body, or a flattened equivalent tree
must not produce a promotion success.

## Behavior

The `Source Promotion Contract` workflow runs only for a pull request whose body
contains `<!-- source-promotion: true -->` and exactly one lowercase
`source-promotion-control` marker in the form
`<!-- source-promotion-control: <40 lowercase hex> -->`. It checks out
the exact PR head with full history, fetches the named control commit from the
fixed `EffortlessMetrics/ripr` source repository, and consumes only these fixed
paths from that immutable commit:

- `docs/release/source-promotion/contract-inputs.json`;
- `docs/release/source-promotion/preflight.json`; and
- `docs/release/source-promotion/resolution-manifest.json`.

The control manifest uses schema `ripr.source_promotion_ci_inputs.v2` and binds
exact `source_main`, `join_head`, fixed paths, and the SHA-256 digest of each
receipt. `join_head` must equal the live PR head and `source_main` must equal
the live PR base. The control commit must not be an ancestor or descendant of
J. On manual dispatch, `source_parent` must equal the sidecar's `source_main`
before the verifier is built. The control files are external to J's tree: the workflow
rejects candidate-provided paths and does not require a tracked input file in
the promotion PR. This avoids a circular fixed-point contract in which a file
containing J also changes `J^{tree}`.

The workflow's `validate_tracked_regular_file` helper performs the mode-100644
and canonical-path checks in the fixed source repository before any sidecar
bytes are copied into runner-temporary files. It is called for all three fixed
sidecars and must never be pointed at a candidate-checkout `INPUTS_PATH`.

The resolution schema is keyed by `kind:key`, so one workflow path may
legitimately have rows in several reviewed categories. For every workflow path
changed by the promotion PR, the authenticated immutable resolution manifest is
the sole import authority: at least one row must have that exact key and every
row with that key must have disposition `swarm_blob` or `integrated`. Missing
rows, any `source_blob`, mixed source/non-source authority, and unknown or other
dispositions fail closed. Duplicate rows for one `kind:key` remain invalid
under the resolution verifier. The workflow must not substitute a hardcoded
workflow-name exception for reviewed resolution authority.

The workflow runs `cargo xtask source-promotion verify` and emits one
normalized `ripr.source_promotion_contract.v2` receipt plus the verifier
JSON/Markdown receipts. PR and post-merge receipt directories live under the
runner-owned temporary directory rather than the candidate checkout. Every
receipt is bound to the PR head, control commit, and input digests and is
uploaded under a SHA-containing artifact name. The normalized PR receipt also
preserves the trusted `source_parent` identity emitted by input validation.

After the PR receipt upload, an always-run terminal enforcement step reads the
normalized contract and succeeds only when its schema is
`ripr.source_promotion_contract.v2`, its status is `verified`, validation is
`passed`, the verifier receipt is `present`, and the verifier exit code is zero.
Rejected evidence is therefore retained before the hosted job fails; a missing,
malformed, rejected, candidate-supplied, or non-zero-verifier receipt cannot
produce a green `Source Promotion Contract` check.

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
- a stale or substituted `join_head`, control commit, source parent, receipt digest, body
  command SHA, or fixed receipt path fails closed;
- missing, abbreviated, uppercase, unreachable, wrong-repository, symlinked,
  directory, and placeholder control inputs fail closed;
- a dispatch whose trusted `source_parent` differs from the immutable sidecar's
  `source_main` fails before the verifier is built;
- one or more `swarm_blob`/`integrated` rows for distinct resolution kinds are
  accepted when every row for the workflow key authorizes non-source movement;
- missing, source-only, or mixed allowed/source dispositions for one workflow
  key are rejected;
- duplicate rows for the same `kind:key` remain rejected by the resolution
  verifier;
- a valid two-parent join passes the verifier;
- a single-parent or equivalent-tree flattened history fails post-merge;
- uploaded receipts retain exact heads, ordered parents, input digests, checks,
  failure reasons, and claim boundaries;
- the retained verified `ripr.source_promotion_verification.v2` shape passes the
  balanced normalizer predicate while malformed shapes fail closed;
- a rejected normalized contract is uploaded and then fails the hosted job,
  while a verified, passed, present, zero-exit contract passes terminal
  enforcement;
- candidate-checkout files cannot substitute for runner-owned verifier or
  normalized-contract receipts.
- The uploaded post-merge contract receipt uses schema
  `ripr.source_promotion_post_merge_contract.v1` and retains the exact
  `control_commit` alongside J, source, trusted-parent, and merged-main SHAs.
- the verifier is built from the trusted source-parent SHA in an isolated target
  and invoked against the candidate checkout; the candidate cannot supply it.

## Non-Goals

## Acceptance Examples

- A marked promotion PR with one exact source-control marker, canonical
  mode-100644 sidecar files, matching digests, and a numeric,
  repository-bound `gh pr merge` command passes the PR-head verifier lane.
- A changed workflow with one reviewed `swarm_blob` row is eligible for import.
  If the same workflow also appears in another reviewed category, that second
  row may be `integrated` or `swarm_blob`; any `source_blob` row makes the
  workflow authority mixed and rejects the promotion.
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
`source_promotion_workflow_disables_checkout_credentials_before_code`,
`source_promotion_workflow_binds_trusted_source_parent`,
`source_promotion_workflow_refutes_crlf_rewrite_thread`, and the integration
contract tests in `xtask/tests/source_promotion_workflow_contract.rs`, including
missing/source-only, mixed allowed/source, one-or-more unanimously allowed
workflow-disposition cases, the balanced verifier-receipt predicate, and
runner-owned upload-before-enforcement ordering. The exact-J graph, `kind:key`
completeness, duplicate
`kind:key`, and flattened-history controls remain covered by the verifier tests
mapped in `.ripr/traceability.toml`.

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
