# RIPR-SPEC-0152: Exact source-promotion verifier

Status: proposed

## Problem

An exact source-promotion join must be checked against reviewed preflight and
resolution inputs. A squash, repair descendant, or substituted history must
not be accepted as the declared join.

## Behavior

The read-only `cargo xtask source-promotion verify` command consumes one exact
`ripr.source_promotion_preflight.v1` receipt and one deterministic
`ripr.source_promotion_resolution.v1` manifest. It accepts only exact
40-character lowercase commit identities for `--join-head`, `--source-main`,
and optional `--main-head`; branch names, `HEAD`, and abbreviations fail closed.

The manifest binds the preflight byte digest, exact parents and merge base,
reviewed join tree, and one non-empty disposition/rationale for every preflight
conflict, source-survivor candidate, and swarm-exclusion candidate. Missing,
duplicate, extra, or out-of-inventory rows are rejected; semantic rulings
remain with the reviewer.

Verification proves the requested J has exactly ordered parents
`SOURCE_PARENT` then `SWARM_PARENT`, both parents are ancestors, selected swarm
commits remain reachable through parent 2, ancestry counts and ordered digests
match preflight, and J's tree equals the reviewed tree. It separately proves
that J's effective `ripr` crate version, `Cargo.lock` `ripr` package version,
VS Code package version, and both npm lock-root version fields match
`SOURCE_PARENT`, while `CHANGELOG.md` remains byte-identical to
`SOURCE_PARENT`. Dependency, feature, package-layout, script, and lock-graph
changes are not release metadata drift when those governed identities remain
unchanged. Optional merged source main must reach J.

Automatic `preview_tree` is never accepted as the reviewed tree. The command
never constructs a join or mutates refs, the index, worktree, remotes,
branches, tags, releases, credentials, publication channels, or K back-sync
state.

## Required Evidence

The verifier emits deterministic `ripr.source_promotion_verification.v2` JSON
and Markdown receipts containing exact identities, ordered parents, tree and
ancestry digests, governed release-version checks, source-authoritative
changelog-byte checks, invalidation rules, non-claims, and structured failure
reasons. Git object probes use an explicit repository root and disable
replacement refs.

## Non-Goals

No join construction, semantic conflict ruling, product correctness, artifact
adequacy, release readiness, publication, or K back-sync verification.

## Acceptance Examples

- A valid two-parent J with matching reviewed tree, governed versions,
  source-authoritative changelog bytes, ranges, and resolution inventory emits
  `verified` JSON and Markdown receipts.
- Dependency, feature, package-layout, script, and lock-graph changes pass when
  every governed release-version identity remains equal to `SOURCE_PARENT`.
- Squash, rebase, cherry-pick, reversed-parent, substituted-parent,
  tree-equivalent, preview-tree substitution, release-version or
  source-authoritative changelog drift, and appended repair heads are rejected
  with deterministic failure reasons.
- Omitting `--main-head` records reachability as `not_run`; an unrelated
  equivalent-tree main rejects the receipt.

## Test Mapping

Executable proof lives in
`xtask/src/command.rs::tests::source_promotion_verify_cli_entrypoint` and
`xtask/src/reports/source_promotion_verify.rs::tests`, including exact identity,
canonical manifest and inventory, range/tree/parent adversaries, replacement
refs, dependency/layout preservation, each governed version-field mutation,
npm lock-root disagreement, changelog mutation, caller-state snapshots,
structured rejection, and valid end-to-end receipt tests. The mapping is
maintained in `.ripr/traceability.toml`.

## Implementation Mapping

The command is registered through `xtask/src/command.rs` and
`xtask/src/reports/mod.rs`, dispatched by
`xtask/src/dispatch.rs`, and implemented in
`xtask/src/reports/source_promotion_verify.rs`. Outputs are written to
`target/ripr/source-promotion/source-promotion-verification.json` and `.md`.

## Metrics

The governed local metric is `unit_test_pass_rate`; receipt denominators and
ordered digests are retained as verification evidence, not product-quality or
mutation-runtime metrics.
