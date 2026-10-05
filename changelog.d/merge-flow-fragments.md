<!-- section: Docs -->
- Changelog entries now go in one fragment file per PR under `changelog.d/`
  instead of `CHANGELOG.md`, folded at the release cut, so concurrent PRs stop
  conflicting on the `Unreleased` section. `docs/MERGE_FLOW.md` records the
  swarm merge order, conflict, and post-merge compile-check rules (#5342).
