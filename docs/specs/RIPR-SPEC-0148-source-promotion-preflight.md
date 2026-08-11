# RIPR-SPEC-0148: Source-Promotion Preflight Receipt

Status: accepted

Owner: release control / swarm operations

Created: 2026-08-09

Linked issues:

- #1492 — compile a source-promotion preflight receipt.
- #1478 — consume the exact parent pair before constructing the join.

Support-tier impact:

- No product or [support-tier](../status/SUPPORT_TIERS.md) change. This is a
  maintainer-facing, read-only control-plane report.

Policy impact:

- No source integration, version, workflow, credential, publication, or
  secret change.

## Problem

The source release process must preserve the complete squashed-PR history in
the swarm parent while proving that source and swarm inputs are the exact
transaction-boundary repositories and commits. Hand-built merge audits are
easy to repeat incorrectly, confuse all-reachable with first-parent counts,
and can accidentally mutate an authoritative checkout during conflict
inspection.

## Behavior

`cargo xtask source-promotion preflight` consumes complete source and swarm
parent SHAs plus explicit local repository roots. It verifies origin identity,
exact commit identity, the held source main, and swarm-parent reachability. A
disposable repository fetches both exact objects, computes the merge base and
separately named all-reachable/first-parent counts, inventories changed paths,
and runs `git merge-tree --write-tree --name-only -z` for machine-readable
conflict-path evidence. This requires Git 2.38 or newer; older or malformed
Git versions fail closed before the merge probe.

JSON and Markdown are projections of one deterministic receipt. The ordered
all-reachable SHA digest uses:

```text
git rev-list --topo-order --reverse MERGE_BASE..PARENT
UTF-8 SHA lines joined with LF, then SHA-256
```

The ordered first-parent SHA digest uses:

```text
git rev-list --first-parent --reverse MERGE_BASE..PARENT
UTF-8 SHA lines joined with LF, then SHA-256
```

The receipt records source-survivor candidates, a set-differenced inventory of
paths changed only on the swarm side, and a non-dispositive inventory of
swarm-authority resolution candidates, exact-parent version/changelog
observations (including Cargo.lock and npm lock roots; missing changelog
evidence remains unknown), invalidation rules, and
next actions. `preview_tree` is automatic merge-tree output only. A separate
optional reviewed resolved-tree SHA is recorded and verified in the supplied
repository object store; absent that input, finalization is visibly missing.
It does not create a join or modify either authoritative checkout.

Canonical receipt identity is the exact UTF-8 file written by the producer,
including pretty-print whitespace and its trailing LF. The source verifier
hashes those bytes before parsing JSON; a reserialized equivalent is not the
same preflight input. The corresponding exact-J consumer contract is
[RIPR-SPEC-0152](RIPR-SPEC-0152-source-promotion-verifier.md).

## Required Evidence

- complete parent SHAs resolve exactly in their named repositories;
- required protected candidate tag uses
  `refs/tags/ripr-release-<version>-<SWARM_PARENT>` and resolves in the
  supplied swarm repository to exactly SWARM_PARENT; the local verifier ref
  `refs/ripr/release-<version>-<SWARM_PARENT>` is a separate release-control
  value and is not accepted as the preflight input;
- source parent equals the declared current source main;
- swarm parent is an ancestor of the declared swarm main;
- origin remotes identify the declared repositories;
- merge base, both denominator variants, and ordered digest recipe are present;
- disposable merge diagnostics and machine-readable conflict paths are present;
- automatic preview-tree output is distinct from an optional reviewed
  resolved-tree input;
- JSON and Markdown are deterministic projections with no temporary path or
  capture timestamp;
- exact-parent version observations include Cargo.lock ripr and npm lock root;
- invalidation rules name changes to the source parent, swarm parent, declared
  main, immutable ref resolution, identity, ancestry, digest, conflict, and
  tree.

## Non-Goals

- constructing or committing the history-preserving join;
- changing versions or changelog metadata;
- qualifying artifacts or authorizing publication;
- tagging, publishing, signing, marketplace mutation, or back-sync;
- treating a clean textual merge as proof that semantic overlap is absent.

## Acceptance Examples

- Diverged source/swarm repositories with a shared base report
  `two_parent_join` and preserve each first-parent denominator.
- A shared-path edit reports the `git merge-tree` conflict without changing
  either checkout.
- An abbreviated SHA, wrong origin, stale source main, or candidate outside
  swarm main fails with an actionable error.

## Test Mapping

- `xtask/src/reports/source_promotion.rs` unit tests cover SHA validation,
  digest order, strict remote identity (including suffix-trick rejection),
  authority-path classification, fixture shape, and disposable conflicting and
  clean repository pairs, exact-parent version reads, and reviewed resolved-tree
  verification for an unreachable `git write-tree` object. They also cover
  source-promotion fixture linkage, missing changelog unknown-state handling,
  location-independent identity serialization, exclusive disposable-directory
  creation, and rejection of a non-ancestor swarm main.
- `fixtures/source_promotion/diverged-conflict.json` pins the discriminating
  divergent/conflict expectation.

## Implementation Mapping

- `xtask/src/reports/source_promotion.rs`
- `xtask/src/command.rs`
- `xtask/src/dispatch.rs`
- `docs/SOURCE_PROMOTION_PREFLIGHT.md`

## Metrics

No product metric is emitted. Receipt fields provide the source-promotion
denominator, digest, conflict, and identity evidence needed by the release
operator.
