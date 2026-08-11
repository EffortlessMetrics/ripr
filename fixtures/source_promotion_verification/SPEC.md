# Fixture: source_promotion_verification

Spec: RIPR-SPEC-0149

## Given

The source-promotion verifier receives a source repository, a declared
`refs/ripr/*` swarm ref, the exact join commit `J`, and a preflight manifest.
The manifest records the merge base, ordered parents, reviewed tree, canonical
v1 range, resolution inventory, and governed metadata. The fixture harness
uses disposable repositories so the caller's refs, index, worktree, and
remotes can be snapshotted before verification.

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
- governed release-version or source-authoritative changelog changes and merged-main equivalents that do not reach J;
- floating, abbreviated, or uppercase identity arguments.

The successful case must emit both the JSON and Markdown v1 receipts. Rejected
cases must emit a structured rejection receipt when an output directory can be
determined. The executable witnesses are the focused `source_promotion_verify`
tests in `xtask/src/reports/source_promotion_verify.rs`.

## Then

The valid direct join is accepted only when its merge base is unique, its
ordered parents and canonical v1 range match the manifest, its tree, release-version identity, and source-authoritative changelog bytes are unchanged, and its resolution rows are complete and in
inventory. Rebase, cherry-pick, substituted-parent, squash/tree-equivalent,
preview-substitution, ambiguous-merge-base, and post-main histories are
rejected with reason-bearing checks. Receipt booleans are derived from those
verification results, and caller state is unchanged after every run.

## Must Not

- Treat a tree-equivalent squash or cherry-pick as the declared history join.
- Accept a floating ref, abbreviated object id, substituted parent, or
  ambiguous merge base.
- Claim that main reachability ran after the verifier's terminal decision.
- Mutate process-wide CWD or silently rewrite the caller's refs, index,
  worktree, or remotes.
- Infer semantic conflict correctness, product correctness, release readiness,
  publication, or K back-sync behavior from this graph/tree identity proof.
