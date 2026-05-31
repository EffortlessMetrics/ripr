# Lane 1 RIPR+ Burndown Map

Issue: [#591](https://github.com/EffortlessMetrics/ripr-swarm/issues/591)
Date: 2026-05-30

This tracker is the repo-local map for making repo-wide RIPR/RIPR+
enforcement meaningful. It is not a repair PR queue by itself, and it does not
claim badge movement without receipt movement.

## Current Measurement Contract

- Raw seam inventory is not debt. `repo-seams-json` is an inventory surface.
- Repo-wide unresolved RIPR+ debt is sourced from `repo-badge-json` on the
  canonical actionable basis, using `counts.unsuppressed_exposure_gaps`.
- The RIPR+ receipt contract remains:
  `unresolved`, `top_files`, `suppressed`, and `head`.
- `top_files` must stay bounded. Do not run full `repo-exposure-json` just to
  populate it.
- Badge endpoint JSON must be generated, not hand-edited.

## Evidence Snapshot

| Evidence | State |
| --- | --- |
| Gauge basis fix | #586 is closed by PR #668, which added `cargo xtask ripr-plus` and sources the receipt from `repo-badge-json`. |
| Current committed public endpoint | `badges/ripr.json` reports `191`; `badges/ripr-plus.json` reports `191`. These are endpoint snapshots, not proof of a fresh recompute in this PR. |
| Fresh no-ledger repo badge recompute | `cargo xtask badge-basis` timed out after 90 seconds while generating `repo-badge-json`; no public badge endpoint was refreshed. |
| Latest sampled packet report | `target/ripr/reports/actionable-gaps.json` is `limited_sampled_input`, `repo-exposure-json:limit_5000_of_46406`, and `downstream_consumable: false`; it emitted zero repair packets and must not be treated as the full repo queue. |
| 2026-05-29 issue snapshot | The filing note recorded about `120,408` raw active seams, `135,812` total seams, and about `2,722` canonical actionable gaps. Treat those as issue-snapshot context until a fresh bounded canonical report supersedes them. |
| RIPR+ badge input | `cargo xtask test-efficiency-report` writes `target/ripr/reports/test-efficiency.json`, and the repo badge endpoint path runs that producer before repo `ripr+` badge rendering. #590/#592 are now closeout/docs issues; no-ledger endpoint refresh is still blocked by the #588/#593 scan timeout. |
| Compact cache limit | #588 is closed by PR #689. `RIPR_COMPACT_REPO_SEAM_CACHE_MAX_SEAMS` can raise the compact repo seam cache store limit for an intentional full refresh, but that opt-in does not remove the #593 need to avoid duplicate build-heavy scans. |

## Large-Repo Scan Guardrails

Use these rules until the lane records a fresh downstream-consumable queue:

- Prefer existing generated receipts, committed endpoint snapshots, or an
  explicit `--gap-ledger` for summary counts.
- Use `cargo xtask repo-exposure-summary-report` when local planning needs a
  bounded repo exposure summary. It writes
  `target/ripr/reports/repo-exposure-summary.json`.
- Do not use full `repo-exposure-json` for ordinary badge, receipt, top-file, or
  packet-queue paths.
- Treat fresh no-ledger `repo-badge-json` and `rtk cargo xtask badge-basis`
  refreshes as build-heavy full-refresh exceptions, not ordinary summary
  paths. Run only one such scan at a time, then share the generated receipt or
  ledger with parallel agents.
- Use `RIPR_COMPACT_REPO_SEAM_CACHE_MAX_SEAMS=200000` only as a scoped opt-in
  for a deliberate large-repo refresh on a machine with enough disk headroom.
- Keep large temporary JSON under `target/ripr/` when possible, and remove
  ad-hoc files such as `repo-exposure*.json` or `exposure.json` after
  inspection.
- Record whether a report is full repo, sampled, fixture-ledger, or stale
  issue-snapshot data before using it to rank packet work.

## Current Queue Classification

Use this classification before opening packet PRs:

| Class | Route |
| --- | --- |
| Focused test repair | Add or tighten one discriminating assertion for a concrete packet with verify and receipt movement. |
| Analyzer false positive | Add the smallest fixture proving the analyzer classification is wrong, then fix classification logic. |
| Legitimate test intent | Record owner, reason, and review condition only when the test is intentionally smoke, opaque, duplicate, performance, integration, or documentation-oriented. |
| Suppression or ledger decision | Use only for accepted-risk or policy-backed cases; never suppress to make a number drop. |
| Blocked or missing context | Do not rank as swarm-ready without a verify command, receipt command, allowed edit surface, and typed evidence refs. |
| Parser-owned | Do not pin suspicious parser behavior in this lane; route it to parser correctness first. |
| Coverage-owned | Do not open generic coverage PRs as RIPR repairs. |
| Static limitation backlog | The latest sampled report names analyzer backlog routes such as owner-call tracing, assertion-target affinity, related-test affinity, local operand resolution, and value-resolution audit fixes. These are not repair-ready packet PRs until stronger evidence makes them actionable. |

## Merge Queue

These are the next small PRs after #586/#668. They should remain independent and
reviewable.

| Order | Issue | PR title | Purpose |
| ---: | --- | --- | --- |
| 1 | #591 | `docs(ripr): publish canonical burndown baseline and first packet map` | Establish this map and prevent broad repairs against the old raw seam count. |
| 2 | #587 | `fix(badges): make badge-plus missing test-efficiency input actionable` | Stop silent or impossible `ripr+ unavailable` behavior when the auxiliary input is missing. |
| 3 | #590, #592 | `docs(ripr): close test-efficiency badge path tracking` | Record that the local producer and badge wiring already exist; leave large-repo no-ledger refresh to #588/#593. |
| 4 | #588, #593 | `perf(cache): make compact repo seam cache limit configurable` | Remove the 100k seam cache ceiling as a repeated full-scan blocker for large repos. |
| 5 | #593 | `docs(ripr): add large-repo scan guardrails` | Keep agents and CI from repeating expensive repo scans or treating sampled reports as full-repo proof. |
| 6 | #589 | `feat(output): add bounded repo exposure summary output` | Provide bounded summary data instead of multi-GB `repo-exposure-json` output for ordinary workflows. |
| 7 | #594 | `devex(ripr): route local workflows away from full exposure dumps` | Enforce local guardrails so default workflows use bounded badge or summary inputs. |

Actual packet burndown starts only after the map and deterministic inputs are in
place and a downstream-consumable canonical queue exists.

## First Packet Families

When the queue is ready, burn down one packet or tight family per PR in this
order:

1. `no_assertion_detected` / likely vacuous proof - add one focused assertion.
2. `smoke_oracle_only` - replace a smoke-only proof with a discriminator.
3. `broad_oracle` - narrow the oracle to an exact field, diagnostic, event, or
   object member.
4. `duplicate_activation_and_oracle_shape` - distinguish duplicate activation
   or declare intentional duplication with owner and reason.
5. Analyzer false-positive fixture - prove the analyzer is wrong and fix the
   classification without suppressing real gaps.

Each packet PR must include a focused test command, a RIPR receipt command, an
allowed edit surface, must-not-change constraints, and before/after receipt
movement. If a packet no longer moves the canonical receipt after #586, close or
supersede it instead of forcing it through.

## Commands Recorded For This Map

```bash
rtk cargo xtask badge-basis
```

Result: failed after the default 90 second `repo-badge-json` generation timeout.
This confirms #593 is still a practical blocker for fresh full-repo badge-basis
audits on this machine.

For an intentional full refresh after #588/#689, scope the cache override to the
single command and check disk headroom first:

```bash
RIPR_COMPACT_REPO_SEAM_CACHE_MAX_SEAMS=200000 rtk cargo xtask badge-basis
```

```bash
rtk git diff --check
```

Run this before the PR is opened.

## What Not To Do

- Do not start test-repair PRs against the `120,408` raw seam count.
- Do not use full `repo-exposure-json` as the normal top-file or badge input.
- Do not hand-edit `badges/ripr.json` or `badges/ripr-plus.json`.
- Do not suppress, declare intent, or ledger away findings just to reduce a
  counter.
- Do not claim the sampled `actionable-gaps.json` report is a full repo queue.
