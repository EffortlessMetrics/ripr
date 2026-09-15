# RIPR-SPEC-0121: perl-lsp-swarm CI scratch-GC orphan-reaper path mismatch

Status: proposed

> Spec-only: the fix lands in the `perl-lsp-swarm` repository (unreachable
> from this session), not in `ripr-swarm`. Status is `proposed` until the fix
> is applied in `perl-lsp-swarm`; there is no `ripr` test or code to map. An
> agent or maintainer with access to `perl-lsp-swarm` applies it there.

Owner: product / swarm

Created: 2026-06-26

Linked issues:

- Campaign 31 Perl repair-routing lane (#1379): downstream CI disk pressure
  traced to orphaned per-run scratch dirs in the `perl-lsp-swarm` repo's
  self-hosted runner lane.

Linked PRs:

- None in this repo. This spec documents a fix for the **`perl-lsp-swarm`**
  repository, which is not reachable from this session. An agent or maintainer
  with access to `perl-lsp-swarm` applies it there.

Support-tier impact:

- None. This is a CI hygiene fix in a sibling repository, not a `ripr` product
  behavior, output-contract, or classification change. See
  `docs/status/SUPPORT_TIERS.md`.

Policy impact:

- Register this spec in `policy/doc-artifacts.toml` (done).

## Problem

The `perl-lsp-swarm` `ripr.yml` workflow creates per-run scratch dirs:

```text
CARGO_TARGET_DIR=/mnt/ci-scratch/perl-lsp-swarm/ripr-${run_id}-${attempt}
TMPDIR=/mnt/ci-scratch/tmp/ripr-${run_id}-${attempt}
```

But the orphan reaper searches the **parent** at depth 1:

```bash
find /mnt/ci-scratch -maxdepth 1 -name 'ripr-*' -mmin +120 -exec rm -rf {} +
```

That pattern does **not** match either real directory, because both nest one
level below `/mnt/ci-scratch`:

```text
/mnt/ci-scratch/perl-lsp-swarm/ripr-...
/mnt/ci-scratch/tmp/ripr-...
```

Final cleanup (`find /tmpdir /target -mindepth 1 -delete` + `rmdir`) only runs
at the end of a **successful** job. When a job is killed, OOM-evicted,
cancelled, or times out before its cleanup step, the per-run Cargo target tree
(a multi-GB Rust build) is left behind, and the preflight reaper never reaps
it. The disk pressure accumulates run over run and blocks subsequent runs on
the self-hosted lane.

## Behavior

After the fix, the preflight reaper in `perl-lsp-swarm`'s `ripr.yml` (both
self-hosted lanes) sweeps the real nested roots, mirroring the pattern
`ripr-swarm` already uses in `.github/workflows/scratch-gc.yml`:

```bash
# ripr-swarm reference (scratch-gc.yml): sweeps INSIDE each subdir at -maxdepth 1.
find /mnt/ci-scratch/cargo-home /mnt/ci-scratch/target /mnt/ci-scratch/tmp \
  -mindepth 1 -maxdepth 1 -mmin +45 -exec rm -rf {} + 2>/dev/null || true
```

Applied to `perl-lsp-swarm`'s two roots:

```bash
for root in /mnt/ci-scratch/perl-lsp-swarm /mnt/ci-scratch/tmp; do
  mkdir -p "$root"
  find "$root" -maxdepth 1 -type d -name 'ripr-*' -mmin +120 \
    -print -exec rm -rf {} + 2>/dev/null || true
done
```

- `mkdir -p` first so `find` does not fail on a missing dir.
- `-print` surfaces reaped paths in the step log (operability).
- The shared `sccache` cap (60G / purge-at-70G on CX53) is **intentional**
  cache pressure, not the leak — do not treat sccache as the primary cause.
- Keep the existing per-run final cleanup unchanged.

## Required Evidence

This is a spec-only deliverable for a sibling repository. The evidence
required to close it is produced **in `perl-lsp-swarm`**, not here:

- A killed/OOM/cancelled `perl-lsp-swarm` run leaves no
  `/mnt/ci-scratch/{perl-lsp-swarm,tmp}/ripr-*` dir older than the reaper's
  `-mmin` window after the next preflight fires.
- The preflight step log shows the reaped paths.
- `/mnt/ci-scratch` free space stays above the disk-guard threshold across
  consecutive cancellations.

No `ripr` test, golden, or metric is evidence for this spec.

## Non-Goals

- No change to `ripr-swarm`'s CI (its `scratch-gc.yml` is already correct and
  is the reference pattern cited above).
- No change to the `ripr` product, schema, output contract, or classification.
- Host-level disk guards (#1058) are a separate layer; this spec is the
  GitHub-Actions-level sweep only.
- No sccache policy change (intentional cache pressure).

## Acceptance Examples

```text
Given a perl-lsp-swarm run cancelled before its cleanup step,
  leaving /mnt/ci-scratch/perl-lsp-swarm/ripr-<id>-1 (older than 120 min)
When the next run's preflight fires,
Then the stale dir is reaped and its path appears in the step log,
  and /mnt/ci-scratch free space is above the disk-guard threshold.
```

```text
Given a perl-lsp-swarm run cancelled before its cleanup step,
  leaving /mnt/ci-scratch/tmp/ripr-<id>-1 (older than 120 min)
When the next run's preflight fires,
Then the stale TMPDIR is reaped,
  and no stale ripr-* dir remains under either nested root.
```

## Test Mapping

None in this repo. This spec documents a CI workflow fix for `perl-lsp-swarm`;
there is no `ripr` production or test code to map. Any test belongs in the
`perl-lsp-swarm` repository (e.g., a workflow-level check or a documented
manual verification that a cancelled run's scratch dir is reaped by the next
preflight).

## Implementation Mapping

None in this repo. The implementation is a one-block edit to
`perl-lsp-swarm`'s `.github/workflows/ripr.yml` preflight step (both
self-hosted lanes), as shown in the `## Behavior` section. There is no
`ripr-swarm` file change required — `ripr-swarm`'s `scratch-gc.yml` is the
reference implementation.

## Metrics

No `ripr` metrics are affected or produced. The operability signal lives in
the `perl-lsp-swarm` CI step log (reaped-path count, `du` summary, `df`
free-space), mirroring the `du -sh` / `df -h` reporting block already present
in `ripr-swarm`'s `scratch-gc.yml`.
