# Reviewed JOIN_TREE acceptance evidence (ripr#1478)

Immutable reviewer-owned evidence for one complete resolution of the exact
post-validator source/W7 pair. This directory is **evidence only**. It
constructs no join, moves no ref, and authorizes no release.

## Exact accepted identities

```text
SOURCE_PARENT   = ad291d1bc936d00847d9712d2adf9ea56ca19533
SWARM_PARENT    = 83217e97ec6847db41d757f57279a8b1ca433fe6
SWARM_REF       = refs/tags/ripr-release-0.11.0-83217e97ec6847db41d757f57279a8b1ca433fe6
MERGE_BASE      = 36909460db013ed3a3238ee8b2fc3ccda1135c15
JOIN_TREE       = ab7f81af954acdbe8486f9cfac1c88338ef279cd
P1_SHA256       = 0be27b0aa83ad67ee89118c4463542864ed36654bb92c6c7c44fa88aefaf5f85
MANIFEST_SHA256 = 21356565db166226a2503af250dd526ac610128c427c46f010d6d92f9d2dba6c
```

`P1_SHA256` is the SHA-256 of `source-promotion-preflight.json`, the finalized
preflight that binds `dry_merge.reviewed_resolved_tree = JOIN_TREE` with
`reviewed_resolved_tree_verified = true`. It was produced by the frozen W7
preflight producer checked out at `SWARM_PARENT`, from the same automatic
inventory as the accepted `ripr-swarm#3312` P0 packet (21 conflicts, 131 source
survivors, 56 swarm-authority candidates).

## Files

| File | What it is |
| --- | --- |
| `source-promotion-preflight.json` / `.md` | finalized P1 preflight bound to `JOIN_TREE` |
| `resolution-manifest.json` | the complete 208-row reviewed resolution (`ripr.source_promotion_resolution.v1`) |
| `resolved-tree-validation.json` / `.md` | terminal-green source-trusted validator receipt |
| `packet-index.json` | validator packet index |
| `commands/` | stdout/stderr of every required governance command, as executed against the materialized tree |
| `SHA256SUMS` | digests of every file above |

## Reproduction

```bash
# 1. Materialize the reviewed tree and confirm its identity.
git cat-file -t ab7f81af954acdbe8486f9cfac1c88338ef279cd   # tree

# 2. Regenerate P1 with the frozen W7 producer checked out at SWARM_PARENT.
cargo xtask source-promotion preflight \
  --source-parent ad291d1bc936d00847d9712d2adf9ea56ca19533 \
  --swarm-parent 83217e97ec6847db41d757f57279a8b1ca433fe6 \
  --swarm-ref refs/tags/ripr-release-0.11.0-83217e97ec6847db41d757f57279a8b1ca433fe6 \
  --source-repo <ripr checkout at SOURCE_PARENT> \
  --swarm-repo <ripr-swarm checkout> \
  --version 0.11.0 \
  --resolved-tree ab7f81af954acdbe8486f9cfac1c88338ef279cd \
  --out <dir>

# 3. Re-run the source-trusted validator from a ripr checkout whose HEAD is
#    exactly SOURCE_PARENT.
cargo xtask source-promotion validate-resolved-tree \
  --source-parent ad291d1bc936d00847d9712d2adf9ea56ca19533 \
  --swarm-parent 83217e97ec6847db41d757f57279a8b1ca433fe6 \
  --reviewed-tree ab7f81af954acdbe8486f9cfac1c88338ef279cd \
  --preflight <dir>/source-promotion-preflight.json \
  --preflight-sha256 0be27b0aa83ad67ee89118c4463542864ed36654bb92c6c7c44fa88aefaf5f85 \
  --resolution-manifest resolution-manifest.json \
  --resolution-sha256 21356565db166226a2503af250dd526ac610128c427c46f010d6d92f9d2dba6c \
  --out <dir>/validation
```

The validator materializes `JOIN_TREE` in a disposable operating-system
temporary checkout, runs every required governance command there with the
**source-parent** build of `xtask`, and observes that no ref, branch, tag, or
worktree of the source checkout changed.

## Resolution summary

208 rows, exactly one per required `kind:key` identity:

| kind | disposition | rows |
| --- | --- | ---: |
| `conflict` | `integrated` | 18 |
| `conflict` | `source_blob` | 1 |
| `conflict` | `swarm_blob` | 2 |
| `source_survivor` | `integrated` | 34 |
| `source_survivor` | `source_blob` | 93 |
| `source_survivor` | `swarm_blob` | 4 |
| `swarm_exclusion` | `excluded` | 40 |
| `swarm_exclusion` | `integrated` | 2 |
| `swarm_exclusion` | `swarm_blob` | 14 |

Every disposition is *derived from the accepted tree bytes*, not asserted: each
row compares the `JOIN_TREE` blob against the exact source-parent and W7 blobs.

`policy/network_allowlist.txt` is integrated from ripr#1572's reconciled ledger
(receipt SHA-256 `23edfaf268a5c271603bf142a067fc54b37fd37330f55a927c450eae4023708c`),
carrying one reviewer-applied identifier correction described below.

## Reviewer decisions worth naming

1. **Independently allocated spec identifiers.** Both repositories had allocated
   `RIPR-SPEC-0112` and `RIPR-SPEC-0149` to different specs. `check-traceability`
   and `check-doc-artifacts` both reject duplicate identifiers, so the collision
   cannot be carried. The side with fewer consumers moves: source
   `RIPR-SPEC-0112` (bounded subprocess adapter boundary) becomes
   `RIPR-SPEC-0151`, and W7 `RIPR-SPEC-0149` (back-sync verifier) becomes
   `RIPR-SPEC-0152`.

2. **Public-API gate format divergence.** The source-parent gate records
   crate-root `pub mod` / `pub use` declarations; W7's gate (#3052) records the
   transitive module-level surface. Both are bidirectional against
   `policy/public_api.txt`, so no single file in that format can satisfy both,
   and the source-trusted validator necessarily runs the source-parent build.
   The accepted tree keeps **both** gates: `policy/public_api.txt` holds the
   crate-root ledger the source validator checks, `policy/public_api_surface.txt`
   holds W7's transitive recording, and the tree's own `check-public-api`
   enforces both. Nothing is dropped and the combined gate is stricter than
   either parent's.

3. **`routed-rust.yml` hosted-fallback conditional.** The source gate requires
   the literal `if: needs.route.outputs.router_target == 'github'`; W7 folded
   that term into a multi-line tempfail-fallback condition. The accepted file is
   W7's, with the `rust-github` job condition rewritten into the distributed
   form. `&&` binds tighter than `||` in GitHub expressions, so the condition is
   semantically identical, and it satisfies both gates.

4. **Retired `check-campaign`.** W7 retired the `.ripr/goals/` scheduler
   (#1701), so the source-side `source-of-truth.yml` step invoking
   `cargo xtask check-campaign` would fail in the combined tree. W7's removal of
   that step is the combined-tree truth, and the source-owned regression that
   asserted the retired command is excluded with it.

5. **Clean-merge damage.** `xtask/Cargo.toml` gained a duplicate `toml`
   dependency key and `xtask/src/command.rs` gained a duplicate
   `SourcePromotion` variant and parse arm from the automatic merge. Both are
   repaired; neither was a reviewer decision.

6. **Golden drift on a source-only fixture.** `cargo xtask goldens check` and
   `cargo xtask fixtures` are not among the validator's thirteen required
   commands, so the first accepted tree carried a stale golden. Fixture
   `source_promotion_verification` is source-authored and its expected output was
   recorded against the source-parent analyzer; the accepted tree runs the frozen
   W7 analyzer, where RIPR-SPEC-0147 parser-shape canonicalisation emits one probe
   per changed statement rather than per changed line and RIPR-SPEC-0122 makes
   default human output a bounded triage view. The drift was proved causally
   attributable to the W7 analyzer rather than to the integrated
   bounded-subprocess arm by substituting W7's
   `crates/ripr/src/analysis/probes/diff.rs` verbatim and reproducing the identical
   drift. The golden is re-blessed to the analyzer the tree actually contains, with
   a retained blessing CHANGELOG citing both specs.

## Non-claims

This evidence proves one exact reviewed tree passes the named source-governed
repository contracts. It does not construct `J`, qualify product, editor,
package, or server surfaces, move source or swarm authority, change release
metadata, or authorize publication. Product and editor qualification is ripr#1507;
direct two-parent construction is ripr#1508; ancestry-preserving transport is
ripr#1465.
