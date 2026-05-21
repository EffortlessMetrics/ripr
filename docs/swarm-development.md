# Swarm Development

`EffortlessMetrics/ripr-swarm` is the public development landing zone for
trusted same-repo `ripr` pull requests. The release-facing repository remains
`EffortlessMetrics/ripr`.

Use this repository to prove routed CI and high-throughput agent development
before promoting batches back to the source repository.

## Boundaries

- New development PRs target `ripr-swarm` after the swarm lane is enabled.
- Use same-repo branches and pull requests.
- Do not run public fork PRs on self-hosted runners.
- Do not move crates.io, VS Marketplace, Open VSX, GitHub Release, signing, or
  publish secrets into this repository.
- Do not publish releases from this repository.
- Promote reviewed, green batches back to `EffortlessMetrics/ripr`.

## Runner Posture

The first routed lane should be Rust-only:

```text
Ripr Rust Small Result:
  CX53 -> CX43 -> GitHub-hosted
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
Ripr Rust Small on CX53
Ripr Rust Small on CX43
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
- confirm one idle, online runner has labels `CX53` and `em-ci-rust-1.95`;
- confirm one idle, online runner has labels `CX43` and `em-ci-rust-1.95`;
- keep source/release/publish/signing secrets out of `ripr-swarm`.

Prove CX53 primary:

```bash
gh workflow run routed-rust.yml --repo EffortlessMetrics/ripr-swarm --ref main
gh run list --repo EffortlessMetrics/ripr-swarm --workflow routed-rust.yml --limit 1
```

The run must finish with:

```text
Ripr Rust Small Result: success
target: cx53
reason: cx53_idle
cx53: success
cx43: skipped
github: skipped
```

Prove CX43 fallback by making CX53 unavailable or busy while CX43 is online,
idle, and image-ready, then rerun the same workflow command. The run must
finish with:

```text
Ripr Rust Small Result: success
target: cx43
reason: cx43_idle
cx53: skipped
cx43: success
github: skipped
```

If neither self-hosted path can be selected, record the bounded blocker on the
cutover tracker with the current run URL and the result summary:

```text
target: github
reason: runner_api_failed | no_idle_runner | runner_image_unavailable
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

Promotion remains a source-repo pull request:

```bash
git clone git@github.com:EffortlessMetrics/ripr.git ripr-promote
cd ripr-promote
git remote add swarm git@github.com:EffortlessMetrics/ripr-swarm.git
git fetch origin --prune --tags
git fetch swarm --prune --tags
git switch -c promote/swarm-main origin/main
git merge --ff-only swarm/main
git push origin promote/swarm-main
```

Open the resulting source-repo PR as:

```text
promote: sync ripr-swarm main
```

The source repository CI remains the final release and publish proof.
