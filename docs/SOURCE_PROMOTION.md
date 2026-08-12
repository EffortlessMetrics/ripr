# Source Promotion

This runbook promotes reviewed development history from
`EffortlessMetrics/ripr-swarm` into the release-authority repository
`EffortlessMetrics/ripr`.

The goal is not to copy a final tree. The goal is to preserve the reviewed swarm
commits, preserve source-only release and security work, and leave an auditable
Git graph.

## Non-negotiable history contract

```text
never squash
never rebase
never cherry-pick or reconstruct the swarm range
never merge a source-promotion PR with GitHub's squash option
```

A source-promotion PR must be merged with **Create a merge commit**. The
copy-safe CLI equivalent is:

```bash
gh pr merge <PR> --repo EffortlessMetrics/ripr \
  --merge \
  --match-head-commit <PROMOTION_HEAD_SHA>
```

If a promotion PR is squashed accidentally, stop the release. Do not tag or
publish. Revert the flattened merge if necessary and repeat the promotion with
preserved ancestry.

## Inputs

Freeze these values before creating the promotion branch:

```bash
SOURCE_PARENT=<exact EffortlessMetrics/ripr main SHA>
SWARM_CANDIDATE=<exact frozen EffortlessMetrics/ripr-swarm SHA>
VERSION=<requested release version>
PREVIOUS_SWARM_PROMOTION=<last swarm SHA promoted to source>
```

Also record:

- the controlling source and swarm issues;
- the latest required swarm check receipts;
- included and deferred swarm work;
- source-only commits and paths that must survive;
- swarm-only workflows or settings that must not become source authority.

`SWARM_REF` in the preflight receipt is not a floating branch or a local-only
namespace. It must be the exact fully-qualified protected candidate tag
`refs/tags/ripr-release-${VERSION}-${SWARM_CANDIDATE}` from the active swarm tag
ruleset, and it must resolve in the actual `EffortlessMetrics/ripr-swarm`
remote to `SWARM_CANDIDATE`. The source promotion contract also verifies fixed
public ruleset `20661783` has the exact `refs/tags/ripr-release-*` singleton
pattern with active update and deletion protections. Missing, wrong-target,
missing-ruleset, or mismatched-ruleset inputs fail closed.
The governed candidate tag is expected to be lightweight: its direct ref must
resolve to the candidate commit, rather than relying on an annotated tag object.

Do not use a floating `swarm/main` ref as the reviewed candidate.

## Choose the promotion mode

### Fast-forward promotion

Use fast-forward mode only when source is already an ancestor of the frozen
swarm candidate and there is no source-only divergence to preserve:

```bash
git merge-base --is-ancestor "$SOURCE_PARENT" "$SWARM_CANDIDATE"
```

A zero exit status makes fast-forward technically available. The release owner
must still confirm that no source-only release, security, workflow, metadata, or
history commit would be lost.

### Two-parent history-preserving join

Use a two-parent join whenever source and swarm have diverged. This is the
required mode for the `0.11.0` release train.

The join commit must have exactly these ordered parents:

```text
parent 1: exact source main at branch creation
parent 2: exact frozen swarm candidate
```

The source parent remains first so first-parent release history stays readable.
The swarm candidate remains second so every reviewed swarm commit stays
reachable.

## Preflight

Use a fresh clone or disposable worktree:

```bash
git clone git@github.com:EffortlessMetrics/ripr.git ripr-promote
cd ripr-promote
git remote add swarm git@github.com:EffortlessMetrics/ripr-swarm.git
git fetch origin --prune --tags
git fetch swarm --prune --tags

git cat-file -e "$SOURCE_PARENT^{commit}"
git cat-file -e "$SWARM_CANDIDATE^{commit}"
git merge-base "$SOURCE_PARENT" "$SWARM_CANDIDATE"
git rev-list --count "$PREVIOUS_SWARM_PROMOTION..$SWARM_CANDIDATE"
```

Confirm the candidate is still reachable from swarm `main`:

```bash
git merge-base --is-ancestor "$SWARM_CANDIDATE" swarm/main
```

Record the merge base, included commit count, expected conflicts, source
survivors, and swarm exclusions before creating the join.

## Create the promotion branch

```bash
git switch -c "promote/${VERSION}-swarm" "$SOURCE_PARENT"
```

### Fast-forward mode

```bash
git merge --ff-only "$SWARM_CANDIDATE"
```

### Two-parent join mode

```bash
git merge --no-ff --no-commit "$SWARM_CANDIDATE"
```

Resolve only the conflicts identified by the preflight. Preserve source release
and publish authority, source metadata and history, and any named source-only
analyzer fixes. Exclude or deliberately resolve swarm-only automation that does
not belong in the release-authority repository.

Do not bump crate or extension versions and do not add the new release section
to `CHANGELOG.md` in this PR. Release metadata is a separate review obligation.

Commit the join:

```bash
git commit -m "promote: join frozen ripr-swarm candidate for ${VERSION}"
PROMOTION_HEAD_SHA="$(git rev-parse HEAD)"
```

## Verify the branch history

For two-parent mode:

```bash
set -- $(git show -s --format='%P' "$PROMOTION_HEAD_SHA")
test "$#" -eq 2
test "$1" = "$SOURCE_PARENT"
test "$2" = "$SWARM_CANDIDATE"

git merge-base --is-ancestor "$SOURCE_PARENT" "$PROMOTION_HEAD_SHA"
git merge-base --is-ancestor "$SWARM_CANDIDATE" "$PROMOTION_HEAD_SHA"
```

Also verify:

- all named source-only survivor commits and paths remain present;
- every swarm exclusion has an explicit resolution;
- crate and extension versions remain at the pre-metadata value;
- `CHANGELOG.md` has no new release section;
- generated and golden changes are understood;
- the full source proof suite passes.

## Open the PR

Push the exact reviewed head:

```bash
git push --set-upstream origin "promote/${VERSION}-swarm"
```

Open the source-promotion template:

```text
https://github.com/EffortlessMetrics/ripr/compare/main...promote/<VERSION>-swarm?expand=1&template=source-promotion.md
```

The PR body must record:

- source parent, swarm candidate, merge base, and promotion head;
- ordered parent proof;
- included swarm range and count;
- conflict resolutions;
- source-only survivors;
- swarm-only exclusions;
- version/changelog no-change proof;
- current checks and artifacts;
- post-merge verification command.

Review the integration boundary. The swarm commits were reviewed on the
development trunk; the source PR review should focus on identity, conflicts,
survivors, exclusions, and current source proof rather than treating the entire
swarm range as one opaque new patch.

## Merge

Before merging, fetch the current PR head and confirm it still equals the
reviewed promotion head. Then use merge commit mode only:

```bash
gh pr merge <PR> --repo EffortlessMetrics/ripr \
  --merge \
  --match-head-commit "$PROMOTION_HEAD_SHA"
```

Do not use `--squash` or `--rebase`.

## Post-merge verification

```bash
git fetch origin --prune
git merge-base --is-ancestor "$PROMOTION_HEAD_SHA" origin/main
git merge-base --is-ancestor "$SWARM_CANDIDATE" origin/main
git show -s --format='join %H%nparents %P' "$PROMOTION_HEAD_SHA"
```

The release packet must retain the promotion head and both ordered join parents.
Release readiness must fail when the join is not reachable from source `main`,
even if the flattened tree contents happen to match.

## Recovery

### Source main moved before the promotion PR opened

Re-run preflight from the new exact source parent. Do not silently rebase the
join commit and reuse old receipts.

### The promotion branch has unexpected conflicts

Stop and update the preflight/conflict policy. Do not resolve new conflicts ad
hoc inside an already-reviewed plan.

### The PR was squashed or rebased

Stop the release and do not tag. The reviewed history was flattened. Revert or
otherwise neutralize the incorrect integration, then repeat the source promotion
with a preserved join and fresh current-head proof.

### A source-only survivor is missing

Treat that as a failed integration, not a documentation discrepancy. Repair the
join before the metadata/version PR starts.

## Claim boundary

A successful source promotion proves that reviewed swarm history and required
source-only work coexist on source `main` with auditable ancestry. It does not
prove release metadata, downstream compatibility, or publication.

## Repeatable exact-J verification

Retain the final `ripr.source_promotion_preflight.v1` receipt as immutable
input, then author and review one deterministic
`ripr.source_promotion_resolution.v1` manifest. The manifest binds the
receipt's byte digest, exact parents and merge base, reviewed final tree, and
one disposition with rationale and evidence reference for every conflict,
source-survivor candidate, and swarm-exclusion candidate.

Before merging the direct join, run:

```bash
cargo xtask source-promotion verify \
  --preflight <exact-preflight.json> \
  --resolution-manifest <exact-resolution.json> \
  --join-head <exact-40-character-J-SHA> \
  --source-main <exact-40-character-held-source-main-SHA> \
  --out target/ripr/source-promotion
```

Merge with a merge commit only, guarded by the expected exact head. Do not
squash, rebase, cherry-pick, or append repair commits to J. After the protected
merge, rerun the command with `--main-head
<exact-40-character-merged-source-main-SHA>` to prove exact J reaches merged
source main. Keep the separate 0.11.0 metadata/version lane out of J.

This rule applies to the intentional repository-sync commits J (source
promotion) and the later K (ancestry-preserving back-sync). Ordinary swarm
feature PRs may be squash-merged within `ripr-swarm`; those PR commits are
already part of the selected parent-2 history and must remain reachable through
J rather than being reconstructed as source commits.

The verifier is read-only and does not construct J, resolve conflicts, publish,
or perform the later ancestry-preserving K back-sync; K verification is a
separate follow-up contract.

## Source Promotion Contract workflow

Promotion PRs opt into the source-side contract check with this exact body
marker:

```text
<!-- source-promotion: true -->
```

The promotion PR must name one immutable source-repository control commit in
its body. The workflow requires exactly one lowercase `source-promotion-control`
marker; it is one HTML comment with a full lowercase SHA:

```text
<!-- source-promotion-control: <exact-control-commit-SHA> -->
```

That control commit is a durable sidecar, not an ancestor or child of J and
not a commit in J's tree. It is fetched only from the fixed source repository
`https://github.com/EffortlessMetrics/ripr.git`. It must contain these tracked
regular files at these fixed paths:

- `docs/release/source-promotion/contract-inputs.json`
- `docs/release/source-promotion/preflight.json`
- `docs/release/source-promotion/resolution-manifest.json`

The sidecar's `contract-inputs.json` has this shape:

```json
{
  "schema": "ripr.source_promotion_ci_inputs.v2",
  "source_main": "<exact-40-character-source-parent-SHA>",
  "join_head": "<exact-40-character-J-SHA>",
  "preflight": "docs/release/source-promotion/preflight.json",
  "resolution_manifest": "docs/release/source-promotion/resolution-manifest.json",
  "preflight_sha256": "<64-character-lowercase-SHA-256>",
  "resolution_manifest_sha256": "<64-character-lowercase-SHA-256>"
}
```

The `Source Promotion Contract` workflow checks out the exact PR head with
full history, fetches the sidecar control commit from source origin, verifies
that each fixed path is a mode-100644 regular file, rejects a control commit
that is an ancestor or descendant of J, extracts the files by object id,
verifies their digests, and requires both `source_main` and
`join_head` to match the live base/head. It then runs the read-only verifier
and uploads JSON/Markdown receipts under an artifact name containing the
PR-head SHA. No candidate-provided path is read, and no input file is required
in J's tree; this avoids the impossible fixed-point construction where an
input file contains `J` while also contributing bytes to `J^{tree}`.

The PR body must also contain the live-head guarded merge command and these
warnings:

```text
Use Create a merge commit.
Do not use Squash and merge.
Do not use Rebase and merge.
```

PRs without the marker skip this job; a skipped job is not a promotion pass.
The workflow has read-only contents permission and never executes the printed
merge command or changes refs, settings, branch protection, tags, releases,
publication channels, or secrets. Promotion branches must not change any
workflow other than this permanent contract workflow.

After the protected merge, manually dispatch the same workflow from `main` with
the exact `control_commit`, `J`, the original source parent, the trusted
verifier source parent, and the merged source-main SHA. The dispatch lane
requires `source_parent == source_main` from the immutable sidecar before it
builds the trusted verifier, fetches the same fixed-path sidecar, records the
control commit in its normalized workflow receipt, passes the existing exact-J
arguments and `--main-head` to the trusted verifier, and fails when an
equivalent flattened tree is present without the exact join object remaining
reachable.
