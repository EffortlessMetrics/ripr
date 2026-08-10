# Source-promotion preflight

`cargo xtask source-promotion preflight` creates the repeatable, read-only
receipt consumed by the source release preflight. It preserves the complete
swarm history as the selected parent; it does not create the join.

The disposable merge probe requires Git 2.38 or newer because it uses
`git merge-tree --write-tree --name-only -z`. The command fails closed on an
older or malformed Git version rather than falling back to localized prose.

## Command

Run it from a clean operator checkout with both repositories available locally:

```bash
cargo xtask source-promotion preflight \
  --source-parent "$SOURCE_PARENT" \
  --swarm-parent "$SWARM_PARENT" \
  --swarm-ref "$SWARM_REF" \
  --source-repo ../ripr \
  --swarm-repo ../ripr-swarm \
  --source-main origin/main \
  --swarm-main origin/main \
  --version 0.11.0 \
  --out target/ripr/source-promotion
```

The parent arguments must be complete 40-character commit IDs. The source
parent must equal the declared current source main. The swarm parent must be
reachable from the declared swarm main. Repository roots and Git common
directories must be distinct, and their `origin` URLs must canonically identify
`EffortlessMetrics/ripr` and `EffortlessMetrics/ripr-swarm`; suffix matches and
URL query/path tricks are rejected. Use `--source-remote`/`--swarm-remote` only
for an explicitly reviewed mirror.
The receipt records only the stable verification result for the Git common
directory comparison; it does not serialize an operator's local checkout path.
`SWARM_REF` is required, must use
`refs/ripr/release-<version>-<SWARM_PARENT>`, and must resolve in the swarm
repository to the exact `SWARM_PARENT`; a moved, missing, wrongly named, or
wrong ref fails closed.

The command writes deterministic `source-promotion-preflight.json` and `.md`
files. It records the merge base, separately named all-reachable and
first-parent counts for each parent range, exact-parent version surfaces
(workspace, crate, Cargo.lock ripr package, extension, npm lock root, and
changelog, with missing changelog evidence represented as unknown), and
SHA-256 digests. The
all-reachable digest recipe is:

```text
git rev-list --topo-order --reverse MERGE_BASE..PARENT
UTF-8 SHA lines joined with LF, then SHA-256
```

The ordered first-parent digest recipe is:

```text
git rev-list --first-parent --reverse MERGE_BASE..PARENT
UTF-8 SHA lines joined with LF, then SHA-256
```

It also inventories changed paths, source-survivor candidates, a
set-differenced list of paths changed only on the swarm side, non-dispositive
swarm-authority resolution candidates, and a real
`git merge-tree --write-tree --name-only -z` dry merge with machine-readable
conflict paths. The automatic `preview_tree` is
never a final join tree. An
optional `--resolved-tree <full-tree-sha>` records a separately reviewed
resolved tree after verifying that the object exists in one supplied
repository's common object store; omission remains visibly not finalized. The
dry merge runs in a disposable repository populated by fetching both exact
commits. No branch, index, ref, working tree, version, tag, PR, or publication
state in either authoritative checkout is changed.

## Receipt boundary

The receipt is invalid when either parent, declared main, repository identity,
immutable swarm ref or its resolved SHA, merge base, ancestry count, digest,
conflict list, or resolved tree changes.
Regenerate it if `main` moves before the transaction boundary. A clean dry
merge is not proof that semantic overlap is absent; every textual and semantic
resolution still needs review.

This command does not construct or prove the two-parent join, qualify the
candidate, change versions, authorize publication, publish artifacts, or
perform the source-to-swarm back-sync. Those are separate release boundaries.
