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
rtk git fetch origin --prune
rtk git status --short --branch
rtk gh pr list --repo EffortlessMetrics/ripr-swarm --state open
rtk gh pr list --repo EffortlessMetrics/ripr --state open
rtk cargo xtask goals next
```

Treat ordinary development PRs in `EffortlessMetrics/ripr` as source/swarm
drift. Port, redirect, or close them unless they are release, security, or
explicit promotion work.

When `cargo xtask goals next` reports `no_current_goal = true`, do not continue
the closed campaign and do not infer a successor from chat history. Select work
from repo-owned state in this order:

1. open `ripr-swarm` PRs and required checks;
2. ordinary source-repo PRs that should be ported or redirected;
3. `docs/IMPLEMENTATION_CAMPAIGNS.md`;
4. `docs/IMPLEMENTATION_PLAN.md`;
5. accepted proposals, specs, ADRs, and campaign plans;
6. open issues that cite those repo artifacts.

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
- confirm one idle, online runner has labels `CX53` and `em-ci-rust-1.95`;
- confirm one idle, online runner has labels `CX43` and `em-ci-rust-1.95`;
- keep source/release/publish/signing secrets out of `ripr-swarm`.

Prove CX53 primary:

```bash
rtk gh workflow run routed-rust.yml --repo EffortlessMetrics/ripr-swarm --ref main
rtk gh run list --repo EffortlessMetrics/ripr-swarm --workflow routed-rust.yml --limit 1
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
rtk git clone git@github.com:EffortlessMetrics/ripr-swarm.git ripr-swarm
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

Promotion remains a source-repo pull request. Before opening it, confirm the
promotion is carrying reviewed swarm state rather than active construction:

```bash
rtk gh pr list --repo EffortlessMetrics/ripr --state open
rtk gh pr list --repo EffortlessMetrics/ripr-swarm --state open
rtk gh api repos/EffortlessMetrics/ripr-swarm/branches/main/protection
rtk gh run list --repo EffortlessMetrics/ripr-swarm --workflow routed-rust.yml --branch main --limit 1
```

The operator should see no ordinary source-repo development PRs, a protected
`ripr-swarm/main` branch requiring `Ripr Rust Small Result`, and a recent green
routed Rust result on swarm `main`. Open swarm PRs do not block promotion by
themselves, but each one should be classified as included, deferred, superseded,
or not release-relevant in the promotion PR body.

Create the source promotion branch with a fast-forward-only merge:

```bash
rtk git clone git@github.com:EffortlessMetrics/ripr.git ripr-promote
cd ripr-promote
rtk git remote add swarm git@github.com:EffortlessMetrics/ripr-swarm.git
rtk git fetch origin --prune --tags
rtk git fetch swarm --prune --tags
rtk git switch -c promote/swarm-main origin/main
rtk git merge --ff-only swarm/main
rtk git push origin promote/swarm-main
```

If `git merge --ff-only swarm/main` fails, stop. Do not resolve conflicts inside
the promotion branch. First reconcile the source and swarm histories with an
explicit source-sync or owner-approved promotion plan.

Open the resulting source-repo PR as:

```text
promote: sync ripr-swarm main
```

The PR body should include:

```text
Included swarm range:
  <first promoted commit>..<last promoted commit>

Swarm proof:
  Ripr Rust Small Result on swarm/main: <run URL>
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
- the promotion is not a fast-forward from source `main` to swarm `main`;
- release, publish, signing, marketplace, or GitHub Release secrets would need
  to move into `ripr-swarm`;
- badge endpoint JSON changed outside the generated badge refresh path;
- source CI or release-readiness proof fails.

The source repository CI remains the final release and publish proof. A green
swarm route proves development readiness; it does not replace source release
authority.
