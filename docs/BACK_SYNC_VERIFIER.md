# Back-sync verifier

`cargo xtask back-sync verify` is a read-only, exact-input check for the
post-publication source-to-swarm join. It verifies an already reviewed `K`; it
does not construct a merge, move a ref, change branch policy, publish, or
create release metadata.

```bash
cargo xtask back-sync verify --swarm-before <40-char-swarm-main-sha> --source-release-head <40-char-released-source-sha> --source-release-tag v0.11.0 --join <40-char-K-sha> --tree <40-char-reviewed-tree-sha> --swarm-repo <swarm-checkout> --source-repo <source-checkout> --version 0.11.0 --release-receipt <publication-receipt> --policy-before <policy-before> --policy-exception <temporary-exception> --policy-after <policy-after> --out target/ripr/back-sync
```

Before transport, the declared swarm main must equal `SWARM_BEFORE`; after
transport it must equal `K`. Any other current head fails closed. `K` must have
exactly these ordered parents:

```text
K.parent[0] = SWARM_BEFORE
K.parent[1] = SOURCE_RELEASE_HEAD
```

The verifier checks exact 40-character commit/tree inputs, repository identity,
an actual `refs/tags/<tag>` release tag, parent reachability, reviewed tree
equality, every retained swarm development surface, release/changelog evidence
bound to version/head/K/tree, and a deterministic input/policy manifest. The
before, temporary approved exception, and after policy files are mandatory and
must prove merge commits are disabled before and after. Policy files are read
and hashed only; this command never mutates the temporary merge-policy
exception or branch settings.

The JSON and Markdown files are projections of one receipt:

```text
target/ripr/back-sync/back-sync-verification.json
target/ripr/back-sync/back-sync-verification.md
```

They prove ancestry and transport-boundary identity only. They do not prove
release correctness, artifact adequacy, publication success, or semantic
equivalence. Source publication workflows/settings remain ancestry evidence
and do not become swarm authority.

Focused proof:

```bash
cargo test -p xtask back_sync -- --nocapture
cargo xtask check-command-catalog
cargo xtask check-doc-index
cargo xtask markdown-links
```
