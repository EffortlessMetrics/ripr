# ripr Repository Metadata

This directory holds lightweight repository metadata used by humans, agents, and
xtask policy checks.

Files:

- `traceability.toml`: maps specs to tests, code modules, output contracts, and
  metrics; validate it with `cargo xtask check-traceability`.
- `goals/active.toml`: describes the active Codex Goals campaign, work items,
  dependencies, stackability, and acceptance commands.
- `no-panic-allowlist.toml`: tracks reviewed panic-family debt and its bounded
  selectors.
- `allow-attributes.txt`: records reviewed lint-attribute exceptions.
- `positioning-language-allowlist.txt`: records the bounded files allowed to use
  otherwise discouraged positioning language.
- `static-language-allowlist.toml`: records files allowed to mention restricted
  static/runtime terminology because they define or explain the language
  boundary.
- `test_intent.toml`: carries repository-owned test-intent metadata used by the
  validation and review tooling.

Live work selection is **not** stored under `.ripr/`. The former
`.ripr/goals/active.toml` Codex Goals scheduler and its `xtask goals` commands
were retired and deleted. Current execution state comes from GitHub issues,
pull requests, checks, reviews, and the local worktree; proposals, specs, ADRs,
campaign ledgers, plans, traceability, and PR-local implementation slices supply
durable context and scope without selecting a repository-wide current worker or
issue. See [`docs/REPO_TRACKING_MODEL.md`](../docs/REPO_TRACKING_MODEL.md).
The preferred direction is to remove allowlist entries as implementation and
test debt is paid down. New entries should be reviewed as deliberate exceptions.
