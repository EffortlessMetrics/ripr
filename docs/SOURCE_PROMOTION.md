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

## Preconstruction reviewed-tree validation

Before constructing or publishing a direct two-parent join, validate the exact
reviewed tree with the source-parent-selected governance implementation:

```bash
cargo xtask source-promotion validate-resolved-tree \
  --source-parent <exact-source-parent> \
  --swarm-parent <exact-frozen-W7> \
  --reviewed-tree <exact-reviewed-tree> \
  --preflight <exact-preflight.json> \
  --preflight-sha256 <exact-preflight-digest> \
  --resolution-manifest <exact-resolution.json> \
  --resolution-sha256 <exact-resolution-digest> \
  --out target/ripr/source-promotion/resolved-tree
```

The command requires exact lowercase object identities, canonical non-symlink
sidecars inside the source checkout, and matching sidecar digests. It records
the running source-checkout SHA and executable digest, materializes the reviewed
tree only through an unreferenced disposable object/worktree, runs the ordered
source-owned governance catalog with bounded process-tree termination and
bounded logs, then removes the temporary worktree. A missing, failed,
`not_run`, or unavailable command, source/checker mismatch, sidecar movement,
ref movement, dirty materialization, or cleanup residue produces `rejected`.

The canonical JSON and Markdown receipts use schema
`ripr.source_promotion_resolved_tree_validation.v1`. They omit observed
wall-clock duration so identical semantic states remain byte-stable; each
command instead records the fixed timeout bound. The receipt proves only the
named repository-governance commands on one exact reviewed tree. It does not
construct J, qualify product/editor behavior, or authorize merge or
publication.

Parent-comparative semantic policy decisions remain reviewer-owned inputs. For
0.11.0, #1572 produces the exact network-ledger reconciliation and #1478 binds
that receipt into the complete resolution manifest before this command runs the
source-trusted final-tree checks.

## Source-owned admission, construction, and candidate-ref controller

Issue #1609 adds four typed control-plane subcommands under
`cargo xtask source-promotion`:

1. `write-trusted-builder-receipt` records the exact clean source/workflow SHA,
   Rust toolchain, `Cargo.lock`, isolated locked target directory, and running
   executable digest.
2. `admit-resolved-tree` binds that producer to the exact validated-tree
   packet, preflight and resolution bytes, and command-catalog and
   network-policy integration receipts. The integration-index bytes must match
   the caller-bound lowercase `--integration-index-sha256` before JSON parsing
   and again during the final identity snapshot.
3. `construct-exact-join` consumes the admitted packet and a terminal
   qualification receipt, rechecks the same caller-bound integration-index
   digest, and requires the qualification bytes to match the caller-bound
   `--qualification-receipt-sha256` to create one deterministic, unreferenced,
   direct two-parent commit object without moving a ref.
4. `publish-candidate-ref` creates the construction-bound local candidate ref
   and publishes it only to the bound source repository behind an exact
   old-or-absent lease.

The typed integration receipts must identify the admitted `SOURCE_PARENT` as
`producer_source_sha` and the trusted-builder executable as
`producer_executable_sha256`; matching schemas and status strings alone are not
producer authority. Admission and construction final snapshots reread every
indexed packet member and typed integration receipt; unchanged index bytes do
not authorize changed member bytes.

The local #1609 controller validates exact content identity and internal
consistency only. Its producer fields and digests do not independently prove
producer provenance or reviewer acceptance. #1610 owns trusted transport from
fixed producer repository, commit/ref, path, and digest authority; #1478 owns
reviewer acceptance of integration evidence, and #1507 owns qualification-lane
execution and evidence. No local receipt grants merge, publication, or release
authority.

The tree-qualification receipt has this exact ordered denominator:

```text
editor_package_linux
editor_package_windows
rust_product
source_governance
source_survivors
trusted_product_journeys
untrusted_workspace_contract
w7_product
```

Every lane must be terminal `passed` with a lowercase 64-character evidence
SHA-256. Missing, extra, reordered, renamed, failed, or evidence-free lanes
reject. The qualification also binds the admission packet and receipt,
resolved-tree validation receipt, and admitted network-policy receipt.
Construction compares the complete qualification receipt bytes with the
caller-bound digest before parsing and again during the final preconstruction
reread; a substituted receipt rejects with zero commit-tree attempts.

Controller receipts use numeric `commit_tree_attempts`, `local_ref_attempts`,
`remote_push_attempts`, and `merge_command_attempts`. Admission performs none
of those operations. Construction may attempt `commit-tree` once only after
admission and qualification, and never attempts a ref, push, or merge command.
Publication never constructs a commit or attempts a merge command; a local-ref
rollback is a second local-ref attempt and remains visible in the receipt.

The `--out` packet destination is exclusively reserved before any commit-tree,
local-ref, or remote-push operation. The reservation creates and syncs the
contents of `control-attempt.json`; a missing `packet-index.json` means final state is
unknown and must be reconciled before retry. The journal binds the protected
commit/tree and packet identities, refs, fixed remotes, expected state, and
maximum operation counters needed for that
reconciliation. Completed packets retain the
attempt journal and publish the complete index last. An existing output path or
unsafe/non-directory parent fails closed without overwriting the earlier packet
and without advancing a Git-mutation attempt counter. Outputs beneath either
Git administration directory or a consumed packet/indexed-sidecar root reject
before any output path is created, so a receipt cannot corrupt its own input.
Malformed-command rejection paths protect every supplied known input, and the
comparison resolves filesystem aliases before testing containment.
This ordering detects process-visible interruption; it does not claim
power-loss durability for directory entries on every supported filesystem.

Construction performs its complete live reread of refs, tree and sidecar
digests, indexed packet members, typed integration receipts, and qualification
bytes immediately before `commit-tree`. A changed or unreadable value stops
with zero commit-tree attempts.

Construction and publication require `--source-main-ref refs/heads/main`;
caller-selected aliases never stand in for source authority. Publication
requires `--target-ref` to equal the construction receipt's
`candidate_ref`, requires exact matching old-or-absent state locally and
remotely, and uses
`--force-with-lease=<target-ref>:<expected-old-or-empty>`. Fetch and push URLs
must both equal `https://github.com/EffortlessMetrics/ripr.git`; the protected
W7 ref is reread from
`https://github.com/EffortlessMetrics/ripr-swarm.git`.

Before local mutation, after creating the local candidate ref but before push,
and after the push attempt, the controller rereads local and remote source,
local and remote W7, the complete indexed construction packet, the exact join
object, and source fetch/push URLs. A stale pre-push reread attempts to roll the
local ref back to its exact expected old-or-absent state without pushing. If
the guarded push process fails, or the remote is observed not to have published
the exact join, the controller likewise attempts to restore only the local
candidate ref behind an exact-state guard and records whether that rollback
succeeded. This local-only rollback also applies when a failed push is followed
by an observed join: the remote ref is never rolled back, the join remains
unattributed, and the receipt records `publication_state_unknown`.
An unavailable final remote observation immediately rolls back only the local
candidate ref behind an exact-state guard, then records every mandatory
post-push authority reread before returning `publication_state_unknown`;
remote state remains unknown and is never rolled back.

Publication receipts state what was observed:

The receipt keeps `push_process_succeeded` separate from
`target_ref_updated`; an exit-zero no-op records true and false respectively.
Exit-zero malformed or unparseable porcelain records true process success and
null target-update attribution, remains fail closed, and cannot publish.
The `atomic_push` and `expected_state_guard_passed` fields describe that
guarded operation, not the later publication status: both are true for an
attributed target update, null after an attempted push without attribution,
and false when no push was attempted. If another actor moves the remote before
the final reread, the receipt can consequently be `rejected` while retaining
true operation facts for the controller's attributed update.

- `published` means the guarded push's machine-readable status reported an
  actual update of the exact target ref, the remote candidate ref was observed
  at the exact join, and every post-push authority reread remained valid;
- `published_but_invalidated` means the exact join reached the remote candidate
  ref but a bound source, W7, packet, object, or URL identity invalidated during
  publication, or the post-push local candidate-ref observation was
  unavailable;
- `publication_state_unknown` means the final remote state could not be read,
  or it equals the join without an actual target-update attribution; an
  exit-zero up-to-date/no-op push is not publication attribution; and
- `rejected` means the controller observed no authoritative publication.

Only `published` is a successful candidate-ref transport result, and even it is
not source integration or release authority. The other states are stop states;
do not retry blindly or infer rollback of an observed remote mutation.

The #1609 controller never emits or executes a merge command. Every controller
receipt keeps `merge_command: null` and `merge_command_attempts: 0`. Issue #1508
owns the later authoritative promotion candidate and merge-command decision
after accepted-tree and qualification evidence exist.

## Source Promotion Admission workflow

`.github/workflows/source-promotion-admission.yml` is the permanent,
source-owned transport for resolved-tree admission. It supports
`workflow_call` and `workflow_dispatch`, uses only `contents: read`, and keeps
all mutable checkouts, isolated repositories, logs, and packets under the
runner-owned temporary root. It does not use `pull_request_target`, a
privileged environment, write permissions, identity tokens, release secrets,
or caller-selected commands and runners.

The workflow has three closed execution profiles:

- `live` consumes exact externally produced source-promotion evidence;
- `positive_synthetic` exercises terminal admission through a deterministic
  source-owned fixture; and
- `j5_negative` exercises the retained combined-tree network-policy
  under-description and must finish with a complete rejected packet.

The operation mode is `admit_only` or `constructor_dry_run`. Live inputs bind
the full source repository/parent/controller identity, the closed
`source-owned-xtask@<workflow-source-SHA>` trusted-checker identity, swarm
repository, protected W7 ref and peeled commit, reviewed tree, and every
sidecar by producer repository, immutable commit/ref, mode-100644 path, and
lowercase SHA-256.
Artifact names, floating branches, abbreviated SHAs, candidate-checkout paths,
and caller-supplied success booleans are not authority. Synthetic profiles use
source-owned fixture identities; they are not general path or command inputs.
The checker identity must equal the workflow/controller SHA. The workflow
produces the trusted-builder packet internally from that exact identity, the
pinned toolchain, lockfile, isolated external target, and executable digest.

The workflow runs the production `source-promotion run-admission-workflow`
harness and preserves its exit status. It finalizes and uploads every available
member of the immutable admission packet with `if: always()`, downloads that
artifact into a fresh runner-owned path, and independently verifies the indexed
bytes and exact requested identities before enforcing admission. Only then can
`finalize-admission-workflow` perform admit-only normalization or the optional
constructor dry-run and produce the final normalized
`workflow-disposition.json`, `workflow-disposition.md`, and
`packet-index.json`; the available final packet is uploaded separately with
`if: always()`. `enforce-admission-workflow --expected-status admitted` makes
missing, malformed, unsupported, rejected, `not_run`, `unavailable`, non-zero,
stale, or contradictory evidence terminal red.

The normalized schema is
`ripr.source_promotion_admission_workflow.v1`; its only statuses are `admitted`
and `rejected`. The indexed packet schema is
`ripr.source_promotion_admission_workflow_packet.v1`. An `admit_only` result
requires zero constructor, local-ref, remote-push, merge-command, and
release/publication attempts. A `constructor_dry_run` result is reachable only
after terminal admission and may create at most one unreferenced object in the
isolated synthetic repository. It still cannot move a ref, push, emit a merge
command, or invoke publication/release behavior.

The PR that adds or changes this workflow can establish only exact-head static
and contract proof. Trusted hosted behavior requires a later dispatch of the
committed default-branch workflow using the exact merged source SHA, workflow
blob SHA, controller/schema versions, fixture identities, and dispatch inputs.
A terminal-green admission packet is necessary transport evidence, not
product/editor qualification, J6 publication, merge authority, or release
authority.

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
publication channels, or secrets.

A workflow path may appear in more than one reviewed resolution category because
the manifest is keyed by `kind:key`. A promotion PR may change that workflow
only when the authenticated immutable resolution manifest contains at least one
row for the exact path and **every** row for that path has disposition
`swarm_blob` or `integrated`. Missing rows, any `source_blob`, mixed
source/non-source authority, and unknown or other dispositions fail closed.
Duplicate rows for the same `kind:key` remain invalid under the resolution
verifier. This reviewed-resolution rule is the only workflow-import authority;
there is no hardcoded workflow-name exception.

After the protected merge, manually dispatch the same workflow from `main` with
the exact `control_commit`, `J`, the original source parent, the trusted
verifier source parent, and the merged source-main SHA. The dispatch lane
requires `source_parent == source_main` from the immutable sidecar before it
builds the trusted verifier, fetches the same fixed-path sidecar, records the
exact control commit in its normalized post-merge workflow receipt, passes the existing exact-J
arguments and `--main-head` to the trusted verifier, and fails when an
equivalent flattened tree is present without the exact join object remaining
reachable.
