# Adoption Integration Cleanup Rails

Status: closed as historical cleanup rail.

This plan is the cleanup rail for making the first-run adoption cockpit obvious
without adding new analyzer truth or duplicate product surfaces.

It connects already-shipped first-run, start-here, editor, badge, PR evidence,
and repo-ops surfaces to the GitHub issues that should finish or dispose of
the remaining integration work.

Current disposition:

- The issue-backed source-repo cleanup rail has been reconciled.
- Source issues #1177, #1182, #1183, #1184, #1185, #1148, #1150, #1151,
  #1178, #1179, and #1180 were already closed.
- Source issues #1154, #1156, and #1157 are closed as completed by the
  `ripr-swarm` Start-Here Surface Convergence PR chain and closeout.
- This rail is not the active execution manifest and must not be selected as
  the next campaign without a fresh repo-owned activation.
- `.ripr/goals/active.toml` remains the machine-readable current goal source.

## Scope

This rail owns:

- start-here surface convergence issue burn-down;
- first-run recovery command convergence;
- duplicate PR queue disposition;
- stale local integration worktree disposition;
- landed learning residue cleanup;
- explicit handoff from cleanup rails back to product work.

This rail does not own:

- analyzer evidence changes;
- output schema changes except where a linked issue explicitly owns them;
- default CI blocking;
- badge claim changes;
- preview-language promotion;
- source edits, generated tests, provider calls, or mutation execution.

## Source Of Truth

| Question | Source |
| --- | --- |
| What first-run behavior must do? | `RIPR-SPEC-0051` and `docs/FIRST_PR_WORKFLOW.md` |
| What editor first-pr projection may show? | `RIPR-SPEC-0052` and `docs/EDITOR_FIRST_PR_BRIDGE_WORKFLOW.md` |
| What public badges may claim? | `docs/BADGE_POLICY.md` and `docs/verification/` |
| What generated evidence may be committed? | `docs/GENERATED_EVIDENCE.md` |
| How should PRs be shaped? | `docs/SCOPED_PR_CONTRACT.md` and `docs/PR_AUTOMATION.md` |
| What queue state is current? | `cargo xtask pr-triage-report` |
| What local state is safe to clean? | `cargo xtask worktree doctor` plus explicit operator disposition |

## Issue Map

| Order | Issue | Purpose | Done when |
| ---: | --- | --- | --- |
| 1 | [#1177](https://github.com/EffortlessMetrics/ripr/issues/1177) | Drain duplicate PR clusters. | Closed in source. |
| 2 | [#1182](https://github.com/EffortlessMetrics/ripr/issues/1182) | Consolidate extraction coverage PRs. | Closed in source. |
| 3 | [#1183](https://github.com/EffortlessMetrics/ripr/issues/1183) | Disposition first-run DevEx helper PRs. | Closed in source. |
| 4 | [#1184](https://github.com/EffortlessMetrics/ripr/issues/1184) | Disposition output helper refactor PRs. | Closed in source. |
| 5 | [#1185](https://github.com/EffortlessMetrics/ripr/issues/1185) | Disposition older Claude queue. | Closed in source. |
| 6 | [#1148](https://github.com/EffortlessMetrics/ripr/issues/1148) | Keep the start-here convergence stack as the source-of-truth umbrella. | Closed in source. |
| 7 | [#1150](https://github.com/EffortlessMetrics/ripr/issues/1150) | Align PR and CI first screens on the canonical repair unit. | Closed in source. |
| 8 | [#1151](https://github.com/EffortlessMetrics/ripr/issues/1151) | Converge public command language for start-here. | Closed in source. |
| 9 | [#1178](https://github.com/EffortlessMetrics/ripr/issues/1178) | Converge first-run recovery commands. | Closed in source. |
| 10 | [#1179](https://github.com/EffortlessMetrics/ripr/issues/1179) | Disposition stale integration worktrees. | Closed in source. |
| 11 | [#1180](https://github.com/EffortlessMetrics/ripr/issues/1180) | Reconcile landed learning residue. | Closed in source. |
| 12 | [#1154](https://github.com/EffortlessMetrics/ripr/issues/1154) | Standardize no-output and fail-closed states. | Closed as completed by `ripr-swarm` Start-Here Surface Convergence #220 and closeout. |
| 13 | [#1156](https://github.com/EffortlessMetrics/ripr/issues/1156) | Record external-style start-here receipts. | Closed as completed by `ripr-swarm` #223 receipts and closeout. |
| 14 | [#1157](https://github.com/EffortlessMetrics/ripr/issues/1157) | Close the start-here convergence campaign. | Closed as completed by the `ripr-swarm` Start-Here Surface Convergence closeout. |

## Operating Rules

- Review before merge. Do not bulk-merge generated PR clusters.
- Close duplicate PRs only after naming the keeper or superseding artifact.
- Keep public badge endpoint work repo-scoped. Do not mix it with PR-local
  repair evidence.
- Keep editor work projection-only unless a linked Lane 3 issue reopens
  behavior.
- Keep start-here summaries advisory. Gate decisions remain the only configured
  pass/fail authority.
- Use clean worktrees for PR-bound cleanup when the root checkout is dirty.
- Remove temporary worktrees and external Cargo target directories after PRs
  merge.

## Standard Commands

```bash
rtk cargo xtask pr-triage-report
rtk cargo xtask worktree doctor
rtk cargo xtask pr-ready
rtk cargo xtask check-doc-index
rtk cargo xtask markdown-links
rtk cargo xtask check-static-language
rtk cargo xtask check-traceability
rtk cargo xtask check-pr
rtk git diff --check
```
