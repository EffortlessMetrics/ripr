# RIPR-SPEC-0149: Exact source-promotion verifier

Status: proposed

The read-only `cargo xtask source-promotion verify` command consumes one exact
`ripr.source_promotion_preflight.v1` receipt and one deterministic
`ripr.source_promotion_resolution.v1` manifest. It accepts only exact
40-character lowercase commit identities for `--join-head`, `--source-main`,
and optional `--main-head`; branch names, `HEAD`, and abbreviations fail closed.

The manifest binds the preflight byte digest, exact parents and merge base,
reviewed join tree, and one non-empty disposition/rationale for every preflight
conflict, source-survivor candidate, and swarm-exclusion candidate. Each row
carries a non-empty disposition, rationale, and evidence reference. Missing,
duplicate, extra, or out-of-inventory rows are rejected; semantic rulings remain
with the reviewer.

Verification proves that J is the requested commit with exactly ordered parents
`SOURCE_PARENT` then `SWARM_PARENT`, both parents are ancestors, every selected
swarm commit remains reachable through parent 2, ancestry counts and ordered
digests match preflight, J's tree equals the reviewed tree (never automatic
`preview_tree`), governed metadata surfaces are byte-identical to the source
parent, and optional merged source main reaches exact J. A descendant repair,
squash, rebase, cherry-pick, reversed-parent, substituted-parent, or
tree-equivalent reconstruction therefore fails.

The JSON and Markdown receipts contain exact identities, ordered parents,
tree/digests, checks, deterministic invalidation rules, and non-claims without
timestamps or local paths. The command never constructs a join or mutates refs,
the index, worktree, remotes, branches, tags, releases, credentials,
publication channels, or K back-sync state.
