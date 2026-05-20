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
