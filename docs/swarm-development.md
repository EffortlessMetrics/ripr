# Swarm Development

`EffortlessMetrics/ripr-swarm` is the public development landing zone for
trusted same-repo `ripr` pull requests. The release-facing repository remains
`EffortlessMetrics/ripr`.

Use this repository to prove routed CI and high-throughput agent development
before promoting batches back to the source repository.

## Boundaries

- New ordinary development PRs target `ripr-swarm`.
- Source `EffortlessMetrics/ripr` receives release, security, and explicit
  swarm-to-source promotion PRs only.
- Use same-repo branches and pull requests.
- Do not run public fork PRs on self-hosted runners.
- Do not move crates.io, VS Marketplace, Open VSX, GitHub Release, signing, or
  publish secrets into this repository.
- Do not publish releases from this repository.
- Promote reviewed, green batches back to `EffortlessMetrics/ripr`.

## Swarm Operator Loop

Use current repo state as the source of truth before starting or reviewing work:

```bash
git fetch origin --prune
git status --short --branch
gh pr list --repo EffortlessMetrics/ripr-swarm --state open
gh pr list --repo EffortlessMetrics/ripr --state open
gh issue list --repo EffortlessMetrics/ripr-swarm --state open --limit 100
```

Treat ordinary development PRs in `EffortlessMetrics/ripr` as source/swarm
drift. Port, redirect, or close them unless they are release, security, or
explicit promotion work.

The retired `.ripr/goals` scheduler is not live execution authority. Do not
continue a closed campaign or infer a successor from chat history. Select work from
repo-owned evidence in this order:

1. open `ripr-swarm` PRs, reviews, and required checks;
2. ordinary source-repo PRs that should be ported or redirected;
3. open issues with explicit ownership and current acceptance criteria;
4. accepted RIPR-SPEC requirements and linked proposals, ADRs, or plans;
5. historical campaign documents only as context, never as current authorization.

After a PR has been selected, consult its PR-local `ImplementationSliceV1`
under `.allow/spec-system/slices/` to bound that PR's change. Slices are scope
evidence, not a task database or live work pointer.

If no aligned work is available, leave the trunk clean. Record new routed-runner
proof on #24 or #34 only when there is fresh evidence; otherwise do not create a
make-work campaign.

Every normal swarm slice should finish the same way:

- open a same-repo PR with one clear purpose;
- wait for `Ripr Rust Small Result` and any touched-surface checks;
- merge only when clean and current;
- remove generated residue, isolated targets, and stale local branches or
  worktrees that are no longer needed.

## Runner Posture

The first routed lane should be Rust-only:

```text
Ripr Rust Small Result:
  CX43 -> CPX42 -> CX53 -> GitHub-hosted
```

Self-hosted jobs are only for trusted same-repo PRs and pushes. Fork or
otherwise untrusted pull requests must route to GitHub-hosted runners or skip
self-hosted implementation jobs.

The routed Rust workflow is `.github/workflows/routed-rust.yml`. It emits one
branch-protection-facing check:

```text
Ripr Rust Small Result
```

Implementation jobs are conditional:

```text
Route Ripr Rust Small
Ripr Rust Small on CX43
Ripr Rust Small on CPX42
Ripr Rust Small on CX53
Ripr Rust Small on GitHub Hosted
```

Do not require implementation jobs directly in branch protection.

Cutover proof should use a same-repo pull request and the normalized
`Ripr Rust Small Result` check. The routed implementation jobs remain routing
details and may be skipped when another target is selected.

The router reads runner state with `EM_RUNNER_READ_TOKEN` when that secret is
available. It selects a self-hosted runner only when the runner is idle and has
the matching host label plus the `em-ci-rust-1.95` runner-image/toolchain
readiness label. If runner state cannot be read, no target runner is idle, or a
runner is available but not image-ready, the workflow falls back to GitHub-hosted with
`router_reason=runner_api_failed`, `router_reason=no_idle_runner`, or
`router_reason=runner_image_unavailable`. Fork PRs route to GitHub-hosted with
`router_reason=fork_or_untrusted_pr`.

The route and protected result summaries report count-only diagnostics for
runner visibility: visible runner count, CX53/CX43 online counts, idle
image-ready counts, and online-but-missing-image counts. The protected result
job also receives those values as environment variables so issue comments can
cite downloaded result logs without relying on the web UI summary. The workflow
must not print runner names, registration tokens, secret values, or full runner
label inventories.

The VS Code lane should remain hosted until a separate Node 24 / VS Code / Xvfb
runner image is proven.

## Self-Hosted Proof Runbook

An org-visible operator should use this runbook to close the remaining
self-hosted cutover proof. Do not expose runner registration tokens, runner
secret values, or signing/publish secrets in issue comments.

Before running proof:

- confirm `ripr-swarm` has access to runner group `em-ci-small`;
- confirm `EM_RUNNER_READ_TOKEN` is available to this repository or the
  workflow can otherwise read org runner state;
- confirm one idle, online runner has labels `CX43` and `em-ci-rust-1.95`;
- confirm one idle, online runner has labels `CPX42` and `em-ci-rust-1.95`;
- confirm one idle, online runner has labels `CX53` and `em-ci-rust-1.95`;
- keep source/release/publish/signing secrets out of `ripr-swarm`.

Prove CX43 primary:

```bash
gh workflow run routed-rust.yml --repo EffortlessMetrics/ripr-swarm --ref main
gh run list --repo EffortlessMetrics/ripr-swarm --workflow routed-rust.yml --limit 1
```

The run must finish with:

```text
Ripr Rust Small Result: success
target: cx43
reason: cx43_idle
cx43: success
cpx42: skipped
cx53: skipped
github: skipped
```

Prove CPX42 or CX53 fallback by making CX43 unavailable or busy while CPX42
or CX53 is online, idle, and image-ready, then rerun the same workflow
command. The run must finish with:

```text
Ripr Rust Small Result: success
target: cpx42
reason: cpx42_idle
cx43: skipped
cpx42: success
cx53: skipped
github: skipped
```

If neither self-hosted path can be selected, record the bounded blocker on the
cutover tracker with the current run URL and the result summary:

```text
target: github
reason: runner_api_failed | no_idle_runner | runner_image_unavailable
runner query: ok | failed | skipped_untrusted_pr
visible runners: <count>
cx53 online: <count>
cx53 idle image-ready: <count>
cx53 online missing image: <count>
cx43 online: <count>
cx43 idle image-ready: <count>
cx43 online missing image: <count>
cx53: skipped
cx43: skipped
github: success
```

Do not add conditional implementation jobs to branch protection while proving
this. The protected gate remains `Ripr Rust Small Result`.

## Machine Cutover

Development machines and orchestrators should clone this repository
side-by-side with any existing `EffortlessMetrics/ripr` checkout:

```bash
git clone git@github.com:EffortlessMetrics/ripr-swarm.git ripr-swarm
```

Do not retarget a dirty source-repo clone in place. Preserve or discard any
local source-repo work first, then recreate it as a same-repo `ripr-swarm` pull
request if it is still normal development work.

Use this operating rule after cutover:

```text
normal development:
  target EffortlessMetrics/ripr-swarm

source repository:
  release PRs
  security PRs
  explicit swarm-to-source promotion PRs
```

Each orchestrator should:

- use a fresh `ripr-swarm` clone;
- create a branch in this repository, not in `EffortlessMetrics/ripr`;
- open same-repo pull requests;
- wait for `Ripr Rust Small Result`;
- keep release, publish, signing, and marketplace secrets out of the swarm repo.

## Promotion Back To Source

Promotion remains a source-repo pull request. The release transaction pins one
exact swarm head before this procedure begins; do not substitute a floating
`swarm/main` ref while building or reviewing the join. Before opening it,
confirm the promotion is carrying reviewed swarm state rather than active
construction:

```bash
gh pr list --repo EffortlessMetrics/ripr --state open
gh pr list --repo EffortlessMetrics/ripr-swarm --state open
gh api repos/EffortlessMetrics/ripr-swarm/branches/main/protection
gh run list --repo EffortlessMetrics/ripr-swarm --workflow routed-rust.yml --branch main --limit 1
```

The operator should see no ordinary source-repo development PRs, a protected
`ripr-swarm/main` branch requiring `Ripr Rust Small Result`, and a recent green
routed Rust result on the selected swarm head. Capture the run's exact SHA,
conclusion, and URL and fail closed before creating `J`:

```bash
ROUTED_RUN="$(gh run list --repo EffortlessMetrics/ripr-swarm \
  --workflow routed-rust.yml --branch main --limit 1 \
  --json headSha,conclusion,url \
  --jq '.[0] | [.headSha, .conclusion, .url] | @tsv')"
IFS=$'\t' read -r ROUTED_HEAD ROUTED_CONCLUSION ROUTED_URL <<<"$ROUTED_RUN"
test "$ROUTED_HEAD" = "$SWARM_PARENT"
test "$ROUTED_CONCLUSION" = "success"
```

Retain `ROUTED_HEAD`, `ROUTED_CONCLUSION`, and `ROUTED_URL` in the promotion
receipt and PR body. Open swarm PRs do not block promotion by themselves, but
each one should be classified as included, deferred, superseded, or not
release-relevant in the promotion PR body.

Record these immutable inputs before creating the source branch:

```text
SOURCE_PARENT  = exact ripr/main commit held for this transaction
SWARM_PARENT   = exact selected ripr-swarm/main commit
SWARM_REF      = immutable ref that points directly to SWARM_PARENT
MERGE_BASE     = exact merge base of SOURCE_PARENT and SWARM_PARENT
JOIN_TREE      = reviewed resolved tree identity from source preflight
```

The source promotion is a history-preserving join, not a tree copy. Create the
promotion branch with the source parent first and the selected swarm parent
second:

```bash
git clone git@github.com:EffortlessMetrics/ripr.git ripr-promote
cd ripr-promote
git remote add swarm git@github.com:EffortlessMetrics/ripr-swarm.git
git fetch origin --prune --tags
git fetch swarm --prune --tags
git fetch swarm "$SWARM_REF:$SWARM_REF"
git cat-file -e "$SOURCE_PARENT^{commit}"
git cat-file -e "$SWARM_PARENT^{commit}"
test "$(git rev-parse "$SWARM_REF^{commit}")" = "$SWARM_PARENT"
test "$(git merge-base "$SOURCE_PARENT" "$SWARM_PARENT")" = "$MERGE_BASE"
git merge-base --is-ancestor "$SWARM_PARENT" swarm/main
VERSION=0.11.0  # use the transaction's release version
git switch -c "promote/${VERSION}-swarm" "$SOURCE_PARENT"
git merge --no-ff --no-commit "$SWARM_PARENT"
# Resolve only the conflicts recorded by the source preflight.
git commit -m "promote: join frozen ripr-swarm candidate for ${VERSION}"
J="$(git rev-parse HEAD)"
git show -s --format='join %H%nparents %P' "$J"
test "$(git show -s --format='%P' "$J" | awk '{print NF}')" -eq 2
test "$(git show -s --format='%P' "$J" | awk '{print $1}')" = "$SOURCE_PARENT"
test "$(git show -s --format='%P' "$J" | awk '{print $2}')" = "$SWARM_PARENT"
git rev-parse "$J^{tree}"
test "$(git rev-parse "$J^{tree}")" = "$JOIN_TREE"
git merge-base --is-ancestor "$SOURCE_PARENT" "$J"
git merge-base --is-ancestor "$SWARM_PARENT" "$J"
git push --set-upstream origin "promote/${VERSION}-swarm"
```

The promotion join `J` must have exactly two ordered parents:

```text
J.parent[0] = SOURCE_PARENT
J.parent[1] = SWARM_PARENT
```

The promotion PR head must itself be `J`; a valid join followed by repair
commits is not the reviewed promotion head. Do not append a repair commit to
make the PR green. If the preflight conflict list or resolved tree changes,
rebuild and re-verify `J` from the same pinned parents.

Never squash, rebase, cherry-pick, or reconstruct the swarm range. The source
promotion PR must be merged with **Create a merge commit** while the head is
still `J`:

```bash
gh pr merge <PR> --repo EffortlessMetrics/ripr \
  --merge \
  --match-head-commit "$J"
```

Do not bump versions or add the new changelog section in the promotion PR.
Those are separate source release-preparation work after `J` reaches source
`main`.

Open the resulting source-repo PR as:

```text
promote: sync ripr-swarm main
```

The PR body should include:

```text
Included swarm range:
  <first promoted commit>..<last promoted commit>
Join:
  parent 1: <SOURCE_PARENT>
  parent 2: <SWARM_PARENT>
  join head: <J>

Swarm proof:
  Ripr Rust Small Result on swarm/main:
    head: <ROUTED_HEAD>
    conclusion: <ROUTED_CONCLUSION>
    run URL: <ROUTED_URL>
  latest routed target/reason: <target>/<reason>

Source proof to run:
  cargo xtask check-pr
  cargo xtask release-readiness --version <current-version>
  cargo package -p ripr --list
  cargo publish -p ripr --dry-run
  npm --prefix editors/vscode ci
  npm --prefix editors/vscode run compile
  npm --prefix editors/vscode run package

Deferred swarm PRs:
  <number/title/disposition>
```

Abort promotion when any of these are true:

- the source repository has an ordinary development PR that should have targeted
  `ripr-swarm`;
- `ripr-swarm/main` is not protected by `Ripr Rust Small Result`;
- the latest routed Rust result on swarm `main` failed or is missing;
- `SOURCE_PARENT`, `SWARM_PARENT`, `SWARM_REF`, `MERGE_BASE`, or `J` is missing
  or changed;
- the promotion head is not the reviewed two-parent join;
- release, publish, signing, marketplace, or GitHub Release secrets would need
  to move into `ripr-swarm`;
- badge endpoint JSON changed outside the generated badge refresh path;
- source CI or release-readiness proof fails.

The source repository CI remains the final release and publish proof. A green
swarm route proves development readiness; it does not replace source release
authority.

## Post-Publication Back-Sync To Swarm

After the source release is publicly verified, source release metadata,
changelog, and publication receipts must become reachable from swarm before
ordinary development resumes. This is a second history-preserving join, not a
squash import and not an ordinary source PR cherry-pick.

Freeze these inputs from the post-publication receipt:

```text
SWARM_BEFORE       = exact ripr-swarm/main head before back-sync
SOURCE_RELEASE_HEAD = exact released ripr/main head
BACK_SYNC_TREE      = reviewed resolved back-sync tree identity
```

Construct `K` from a fresh swarm checkout:

```bash
git clone git@github.com:EffortlessMetrics/ripr-swarm.git ripr-back-sync
cd ripr-back-sync
git remote add source git@github.com:EffortlessMetrics/ripr.git
git fetch origin --prune --tags
git fetch source --prune --tags
git cat-file -e "$SWARM_BEFORE^{commit}"
git cat-file -e "$SOURCE_RELEASE_HEAD^{commit}"
VERSION=0.11.0  # use the published release version
git switch -c "back-sync/${VERSION}" "$SWARM_BEFORE"
git merge --no-ff --no-commit "$SOURCE_RELEASE_HEAD"
# Preserve swarm settings, checks, runner topology, and development tooling.
# Import the released version, changelog, and publication receipts.
git commit -m "sync: back-sync released ${VERSION} from ripr"
K="$(git rev-parse HEAD)"
git show -s --format='join %H%nparents %P' "$K"
test "$(git show -s --format='%P' "$K" | awk '{print NF}')" -eq 2
test "$(git show -s --format='%P' "$K" | awk '{print $1}')" = "$SWARM_BEFORE"
test "$(git show -s --format='%P' "$K" | awk '{print $2}')" = "$SOURCE_RELEASE_HEAD"
git rev-parse "$K^{tree}"
test "$(git rev-parse "$K^{tree}")" = "$BACK_SYNC_TREE"
git merge-base --is-ancestor "$SWARM_BEFORE" "$K"
git merge-base --is-ancestor "$SOURCE_RELEASE_HEAD" "$K"
```

The back-sync join must have exactly these ordered parents:

```text
K.parent[0] = SWARM_BEFORE
K.parent[1] = SOURCE_RELEASE_HEAD
```

The resolved tree must retain swarm repository settings, checks, runner
topology, and development-only authority. Source publication workflows and
settings remain reachable in history but must not become swarm authority. The
tree must import the final release version, changelog, and release receipts,
plus any source-only product correction explicitly required for future swarm
development.

Before transporting `K`, refresh the protected branch and require the expected
head to remain unchanged. The preferred transport is a same-repository PR only
when its merge service provides an atomic expected-base-head guard; otherwise
use the owner-approved guarded ref-update exception below:

```bash
git fetch origin main
test "$(git rev-parse origin/main)" = "$SWARM_BEFORE"
```

For the guarded ref-update exception, obtain owner approval, record the
before-policy state, and temporarily permit this one non-force fast-forward
update. With `back-sync/${VERSION}` based directly on `SWARM_BEFORE`, this push
is the atomic expected-head guard: it rejects if `swarm/main` moved or if `K`
is not a fast-forward.

```bash
git push origin "back-sync/${VERSION}:refs/heads/main"
git fetch origin main
test "$(git rev-parse origin/main)" = "$K"
```

If the same-repository PR route is used instead, merge only the reviewed `K`
with the repository's expected-base-head guard and verify `origin/main == K`
immediately afterward. In either route, restore the normal
merge-commit-disabled policy immediately afterward and record the before/after
policy state, transport route, and exact `K` in the back-sync receipt. Do not
force-push, squash, rebase, or merge unrelated work during this exception.

`J` proves source promotion: source parent first, frozen swarm parent second.
`K` proves release back-sync: swarm parent first, released source parent
second. They are separate proof boundaries and neither substitutes for the
other. Normal swarm development resumes only after `K` is reachable from
`ripr-swarm/main`.
