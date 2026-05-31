# Large-Repo Scan Guardrails

Date: 2026-05-31

Issues: #588, #593

## Summary

This repo is large enough that repo-scoped RIPR scans must be treated as
build-heavy operations. The cache implementation issue (#588) made the compact
repo seam cache limit configurable with
`RIPR_COMPACT_REPO_SEAM_CACHE_MAX_SEAMS`, but the local workflow issue (#593)
still needs durable guardrails so agents and CI do not repeat expensive scans or
produce multi-GB artifacts by default.

The relevant observed failure mode was:

```text
ripr: compact repo seam cache store ignored (skipped_large_entry_seams_135812_limit_100000)
```

That warning meant the repo exceeded the previous default compact-cache store
limit. The configurable limit gives an explicit opt-in path for machines with
enough disk and time budget; it does not make broad repo scans cheap or safe to
duplicate.

## Local Rules

- Use `repo-badge-json`, generated receipts, or an explicit gap ledger for
  ordinary summary counts. Do not use full `repo-exposure-json` as the normal
  badge, receipt, top-file, or packet-queue input.
- Treat no-ledger repo-wide badge refreshes as build-heavy until a bounded
  canonical summary path supersedes them.
- Run at most one build-heavy repo-scoped RIPR scan at a time in local agent
  batches. Parallel agents should consume the generated receipt or ledger
  instead of starting another scan.
- Raise `RIPR_COMPACT_REPO_SEAM_CACHE_MAX_SEAMS` only for the command that needs
  the cache write, and only after checking disk headroom.
- Keep temporary large outputs under `target/ripr/` when possible. Delete
  ad-hoc files such as `repo-exposure*.json` or `exposure.json` after
  inspection.

## Operator Checks

Before an intentional build-heavy full repo refresh:

```powershell
rtk powershell -Command 'Get-PSDrive -Name H'
```

Then run the narrow command once, with the cache limit scoped to that process
when needed:

```powershell
rtk powershell -Command '$env:RIPR_COMPACT_REPO_SEAM_CACHE_MAX_SEAMS = "200000"; rtk cargo xtask badge-basis; Remove-Item Env:\RIPR_COMPACT_REPO_SEAM_CACHE_MAX_SEAMS'
```

If a full exposure dump is explicitly requested for debugging, write it under
`target/ripr/`, inspect it, and remove it before handing off:

```powershell
rtk powershell -Command 'rtk cargo run -p ripr -- check --root . --format repo-exposure-json > target/ripr/reports/repo-exposure.json; Remove-Item -LiteralPath target/ripr/reports/repo-exposure.json'
```

Ordinary badge and receipt workflows should not need that file.

## Remaining Boundary

#593 is the local workflow guardrail closeout. It does not replace #589/#594:
those still cover bounded exposure output and routing ordinary workflows away
from full exposure dumps once the product surface exists.
