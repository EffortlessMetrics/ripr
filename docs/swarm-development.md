# Swarm Development

`EffortlessMetrics/ripr-swarm` is the public development landing zone for
trusted same-repo `ripr` pull requests. The release-facing repository remains
`EffortlessMetrics/ripr`.

Use this repository to prove routed CI and high-throughput agent development
before promoting batches back to the source repository.

For the complete release-boundary lifecycle, use the [live-history release
transaction runbook](RELEASE_TRANSACTION.md). This page retains the swarm
operator loop and repository-side promotion/back-sync details; the runbook is
the canonical place for transaction pins, receipts, expected-head guards, and
publication boundaries.

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

Promotion remains a source-repository pull request. Follow the canonical
[Source Promotion](SOURCE_PROMOTION.md) runbook rather than reconstructing the
procedure from historical handoffs or issue comments.

Freeze one exact swarm candidate first. Open swarm PRs do not block promotion by
themselves, but each must be classified as included, deferred, superseded, or
not release-relevant.

There are two valid promotion modes:

```text
fast_forward
  source parent is already an ancestor of the frozen swarm candidate
  and no source-only divergence must survive

two_parent_join
  source and swarm have diverged
  source parent remains first
  frozen swarm candidate remains second
```

The `0.11.0` release train requires `two_parent_join` because source contains
release and analyzer work that must survive alongside the frozen swarm history.

Non-negotiable history rules:

```text
never squash
never rebase
never cherry-pick or reconstruct the swarm range
merge the source-promotion PR with Create a merge commit
```

Before opening the source PR, record:

- exact source parent, frozen swarm candidate, and merge base;
- included swarm range and check receipts;
- conflicts and their planned resolution;
- source-only commits and paths that must survive;
- swarm-only workflows/settings that must remain excluded or source-owned;
- version and changelog state, which must not change in the source-sync PR.

The source PR review focuses on identity, conflict resolution, survivors,
exclusions, and current source proof. It does not treat already-reviewed swarm
commits as one new opaque patch.

The source repository CI remains the final release and publish proof. A green
swarm route proves development readiness; it does not replace source release
authority.
