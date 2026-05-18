# Adoption Integration Cleanup Implementation Plan

Status: open

Owner: repo operations and first-run adoption surfaces

Linked issues:

- [#1177](https://github.com/EffortlessMetrics/ripr/issues/1177)
- [#1182](https://github.com/EffortlessMetrics/ripr/issues/1182)
- [#1183](https://github.com/EffortlessMetrics/ripr/issues/1183)
- [#1184](https://github.com/EffortlessMetrics/ripr/issues/1184)
- [#1185](https://github.com/EffortlessMetrics/ripr/issues/1185)
- [#1148](https://github.com/EffortlessMetrics/ripr/issues/1148)
- [#1150](https://github.com/EffortlessMetrics/ripr/issues/1150)
- [#1151](https://github.com/EffortlessMetrics/ripr/issues/1151)
- [#1178](https://github.com/EffortlessMetrics/ripr/issues/1178)
- [#1179](https://github.com/EffortlessMetrics/ripr/issues/1179)
- [#1180](https://github.com/EffortlessMetrics/ripr/issues/1180)
- [#1154](https://github.com/EffortlessMetrics/ripr/issues/1154)
- [#1156](https://github.com/EffortlessMetrics/ripr/issues/1156)
- [#1157](https://github.com/EffortlessMetrics/ripr/issues/1157)

## Goal

Make the existing first-run adoption cockpit obvious, packaged, and safe by
disposing of duplicate PR clusters, converging recovery commands, resolving
stale integration worktrees, and keeping start-here as the first outside-user
front door.

## Current State

The first-run and editor first-pr surfaces already exist:

```text
ripr first-pr
target/ripr/reports/start-here.{json,md}
target/ripr/first-pr/start-here.{json,md}
ripr: Start Current Repair
ripr: Show Status
generated advisory CI summary
badge endpoint contracts
PR evidence contracts
```

The remaining work is not a new proposal/spec. It is cleanup and convergence:
reduce queue pressure, keep one owner per surface, and finish the highest-value
integrations without creating parallel truth.

## Work Item 1: repo-ops: drain duplicate PR clusters

Issue: [#1177](https://github.com/EffortlessMetrics/ripr/issues/1177)

### Production Delta

No product behavior. Use repo-ops reports and PR review to classify duplicate
or stale open PRs.

### Evidence Delta

Generate and inspect:

```bash
rtk cargo xtask pr-triage-report
rtk cargo xtask gh-pr-status --pr <number>
```

### Acceptance

Each open PR cluster has a disposition: merge candidate, close duplicate,
superseded, needs review, needs rebase, or owner decision.

### Non-Goals

- no mass merge;
- no branch deletion;
- no analyzer changes.

## Work Item 2: test: consolidate extraction coverage PR cluster

Issue: [#1182](https://github.com/EffortlessMetrics/ripr/issues/1182)

### Production Delta

Choose a keeper for overlapping extraction/oracle coverage PRs and port only
unique missing assertions.

### Acceptance

One keeper path exists for #1162, #1164, #1165, #1172, #1173, #1175, and
#1176; duplicates are closed or explicitly ported.

## Work Item 3: devex: disposition first-run helper PR cluster

Issue: [#1183](https://github.com/EffortlessMetrics/ripr/issues/1183)

### Production Delta

Keep first-run helper PRs non-overlapping and aligned with the recovery command
path.

### Acceptance

#1166, #1167, #1171, and #1174 have keeper, repair, or close decisions.

## Work Item 4: refactor: disposition output helper PR cluster

Issue: [#1184](https://github.com/EffortlessMetrics/ripr/issues/1184)

### Production Delta

Select small behavior-preserving output/helper refactor keepers and avoid
overlapping broad merges.

### Acceptance

#1120 through #1127 have keeper, port, close, or review decisions backed by
output-contract evidence.

## Work Item 5: repo-ops: disposition older Claude queue

Issue: [#1185](https://github.com/EffortlessMetrics/ripr/issues/1185)

### Production Delta

Convert #1072 through #1083 from ambiguous backlog into review-now, port,
close-stale, or blocked decisions.

### Acceptance

Draft refactors are not merged from draft state, and unique still-useful tests
are ported only after overlap review.

## Work Item 6: docs/product: keep start-here as the surface owner

Issues: [#1148](https://github.com/EffortlessMetrics/ripr/issues/1148),
[#1150](https://github.com/EffortlessMetrics/ripr/issues/1150),
[#1151](https://github.com/EffortlessMetrics/ripr/issues/1151)

### Production Delta

Tighten docs and command language so `start-here.md` is the first-run front
door and deeper reports stay below it.

### Acceptance

The README, Quickstart, First PR workflow, CI docs, and output schema agree on:

```text
start-here first
top repairable gap or no-action
repair route
verify command
artifact links
gate authority boundary
```

## Work Item 7: devex: converge first-run recovery commands

Issue: [#1178](https://github.com/EffortlessMetrics/ripr/issues/1178)

### Production Delta

Align `ripr doctor`, worktree doctor, cockpit, pr-ready, and first-pr recovery
copy so a new user or agent knows the next command.

### Acceptance

Expected missing, wrong-root, stale, malformed, empty, and blocked states all
produce one clear next action and a regeneration command.

## Work Item 8: repo-ops: disposition stale integration worktrees

Issue: [#1179](https://github.com/EffortlessMetrics/ripr/issues/1179)

### Production Delta

No product behavior. Audit local and remote stale integration branches against
current `origin/main`.

### Acceptance

Each worktree is classified as keep, port, PR, close, or remove, and cleanup
only removes confirmed temporary worktrees or target artifacts.

## Work Item 9: repo-ops: reconcile learning residue

Issue: [#1180](https://github.com/EffortlessMetrics/ripr/issues/1180)

### Production Delta

No product behavior. Reconcile local `docs/LEARNINGS.md` and locked learning
worktrees after already-merged learning PRs.

### Acceptance

No stale local learning diff can delete newer `origin/main` entries, and any
unique remaining learning text is PR'd before cleanup.

## Work Item 10: output: standardize no-output states

Issue: [#1154](https://github.com/EffortlessMetrics/ripr/issues/1154)

### Production Delta

Make empty, missing, stale, malformed, wrong-root, and blocked first-run states
return useful packets rather than confusing failure.

### Acceptance

No-action remains a successful state with reason and next command.

## Work Item 11: dogfood: record external-style receipts

Issue: [#1156](https://github.com/EffortlessMetrics/ripr/issues/1156)

### Production Delta

Dogfood the first outside-user loop from start-here to repair, verify, and
receipt.

### Acceptance

Receipts show whether the top gap led to a focused test or output proof and
whether movement improved, resolved, stayed unchanged, or was blocked.

## Work Item 12: campaign: close start-here convergence

Issue: [#1157](https://github.com/EffortlessMetrics/ripr/issues/1157)

### Production Delta

Closeout docs only.

### Acceptance

All start-here convergence issues are closed or explicitly deferred, and the
handoff records remaining limits and non-goals.

## Validation

Docs-only rails should run:

```bash
rtk cargo xtask check-doc-index
rtk cargo xtask markdown-links
rtk cargo xtask check-static-language
rtk cargo xtask check-doc-roles
rtk cargo xtask check-traceability
rtk cargo xtask check-pr
rtk git diff --check
```

Behavior PRs must add the narrower command set required by their touched
surface.
