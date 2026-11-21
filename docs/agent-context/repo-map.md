# Repo Map

This file gives agents a fast orientation before review or repair work.

For the typed graph that connects proposals, specs, ADRs, campaigns, the
active goal manifest, work items, surfaces, traceability, reports, and
handoffs, see the [repo context system](CONTEXT_SYSTEM.md). The
human-facing layering doctrine lives in
[REPO_TRACKING_MODEL.md](../REPO_TRACKING_MODEL.md).

## Primary surfaces

- Rust workspace: core implementation and server-side logic.
- VS Code extension: editor integration and activation behavior.
- GitHub Actions: CI, release, security checks, and workflow policy.
- `policy/workflow_allowlist.txt`: required workflow budget policy.
- `docs/agent-context/droid-rollout.md`: Factory Droid rollout checklist.

## Important policy files

- `policy/workflow_allowlist.txt`
  - Every `.github/workflows/*.yml` file must have an entry.
  - Shell `run:` blocks must fit the approved non-empty line budget.
  - If a workflow is added or materially changed, review this file.
- `docs/agent-context/droid-rollout.md`
  - Checklist for copying the Factory Droid setup to pilot repositories.
  - Captures required secrets, BYOK invariants, smoke tests, and rollout non-goals.

## Agent-sensitive surfaces

Treat these as higher risk:

- `.github/workflows/**`
- `policy/**`
- release and packaging scripts
- VS Code extension activation/configuration
- LSP protocol handling
- filesystem/path handling
- process execution
- secret or token handling
