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
