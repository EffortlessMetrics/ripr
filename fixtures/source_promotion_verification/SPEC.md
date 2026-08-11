# Fixture: source_promotion_verification

Spec: RIPR-SPEC-0149

## Given

The source-promotion verifier receives a source repository, a declared
`refs/ripr/*` swarm ref, the exact join commit `J`, and a preflight manifest.
The manifest records the merge base, ordered parents, reviewed tree, canonical
`ripr.source_promotion_preflight.v1` range, resolution inventory, and governed
release-metadata identity. The fixture harness uses disposable repositories so
the caller's refs, index, worktree, and remotes can be snapshotted before
verification.

The governed source comparison consists of:

- J's effective `ripr` crate version against `SOURCE_PARENT`;
- J's `Cargo.lock` `ripr` package version against `SOURCE_PARENT`;
- J's VS Code package version against `SOURCE_PARENT`;
- J's npm lock root and `packages[""]` versions against each other and
  `SOURCE_PARENT`; and
- J's `CHANGELOG.md` bytes against `SOURCE_PARENT`.

## When

Run the verifier against a direct two-parent join and against adversarial
histories that are superficially similar but do not preserve the declared
identity:

- equivalent-tree squash and appended ordinary or merge repair commits;
- reversed or substituted parents and rebased/cherry-picked ranges;
- ancestry count or ordered-digest drift;
- reviewed-tree mismatch and automatic preview-tree substitution;
- stale/tampered preflight bytes;
- incomplete, duplicate, extra, or out-of-inventory resolution rows;
- each governed release-version field changing independently;
- npm lock-root disagreement;
- source-authoritative changelog changes with governed versions unchanged;
- merged-main equivalents that do not reach J; and
- floating, abbreviated, or uppercase identity arguments.

The successful case must emit both the JSON and Markdown v2 receipts. Rejected
cases must emit a structured rejection receipt when an output directory can be
determined. The executable witnesses are the focused `source_promotion_verify`
tests in `xtask/src/reports/source_promotion_verify.rs`.

## Then

The valid direct join is accepted only when its merge base is unique, its
ordered parents and canonical `ripr.source_promotion_preflight.v1` range match
the manifest, and J's tree equals the reviewed tree. J's governed versions must
match `SOURCE_PARENT`, its npm lock-root fields must agree, and its
`CHANGELOG.md` bytes must match `SOURCE_PARENT`. Dependency, feature,
package-layout, script, and lock-graph changes remain permitted when those
governed identities are unchanged. Resolution rows must be complete and in
inventory.

Rebase, cherry-pick, substituted-parent, squash/tree-equivalent,
preview-substitution, ambiguous-merge-base, release-version drift,
source-authoritative changelog drift, and post-main histories are rejected with
reason-bearing checks. Receipt booleans are derived from those verification
results, and caller state is unchanged after every run.

## Must Not

- Treat a tree-equivalent squash or cherry-pick as the declared history join.
- Accept a floating ref, abbreviated object id, substituted parent, or
  ambiguous merge base.
- Freeze complete manifests or lockfiles when only governed release identities
  must remain source-equal.
- Claim that main reachability ran after the verifier's terminal decision.
- Mutate process-wide CWD or silently rewrite the caller's refs, index,
  worktree, or remotes.
- Infer semantic conflict correctness, product correctness, release readiness,
  publication, or K back-sync behavior from this graph/tree identity proof.
