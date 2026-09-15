# Active-goal authority migration audit

Status: Phase 1 framework; #1697 remains blocked

## Scope

This plan inventories and classifies singleton active-goal consumers before
#1701 changes authority. It does not move files, choose a campaign, or grant a
writer permission.

## Reviewed slices

1. Inventory tracked readers, documentation, policy, fixtures, workflows, and
   captured issue contracts. Unknown paths remain `not_proven` and block the
   migration.
2. Freeze per-consumer target classification, owner, dependent issue,
   compatibility period, fields, authority effect, and proof.
3. Prove historical references remain readable, while hidden readers and
   legacy ready status cannot authorize current work.
4. #1701 may migrate only after the occurrence and issue-snapshot framework
   blockers are burned down and #1697 is independently closed.

## Phase 2 work packets

1. Replace broad live rules with exact `(path, anchor, marker_kind,
   normalized_marker_hash)` rows. New occurrences in known files must block;
   hidden readers under covered `docs/` and `xtask/` paths prove this. Tracked
   in #1715.
2. Replace issue ranges with individual captured snapshots for #1631-#1639,
   #1643-#1650, #1692, #1697, and #1701. Record body hash, `updated_at`,
   classification, evidence, and explicit `not_proven` freshness. Tracked in
   #1716.
3. Review every remaining blocker on the exact head. Only zero blockers may
   satisfy #1697. Phase 1 cannot unblock #1701.

### #1715 occurrence batches

1. Add the portable occurrence identity and schema `0.2` report projection
   while retaining the Phase 1 consumer view. This batch stays fail-closed.
2. Replace parser, selector, command, and checker families with exact reviewed
   occurrence rows.
3. Replace live documentation and agent-guidance families with exact reviewed
   occurrence rows; retain historical rows without authority.
4. Replace workflow, fixture, policy, metric, and report families with exact
   reviewed occurrence rows. Finish only with zero unknown live occurrences
   and covered-path insertion controls.

## Migration proof

- Before each #1701 slice, run `cargo xtask active-goal-authority-audit` and
  inspect both generated projections.
- Treat `migration_ready = false`, an unclassified discovery, or a
  contradiction as a stop condition.
- Verify each changed consumer with the positive and negative proof named by
  its inventory row.
- Preserve live GitHub and local ownership as unavailable unless supplied by a
  separate captured-input portfolio contract.

## Rollback

Revert the authority-migration slice while retaining this audit, durable
campaign records, and historical evidence. A compatibility reader may be
restored only as explicitly non-authoritative. Rollback must not restore global
selection or mutation authority to legacy status, branch, lane, or work-item
fields.

## Non-goals

- No authority migration in this prerequisite.
- No live portfolio, issue priority, claim, worktree, writer, merge, or release
  action.
- No replacement for cargo-allow structural authority.
