# Validation Commands

Agents should use the smallest validation set that proves the change.

## Rust

```bash
cargo check --workspace
cargo test --workspace
```

## Workflow policy

```bash
cargo xtask check-workflows
```

Run this for any change to:

- `.github/workflows/**`
- `policy/workflow_allowlist.txt`
- release workflows
- CI/security workflows

## Security-sensitive review

For changes involving secrets, workflows, dependency policy, release scripts, or command execution:

- inspect workflow permissions;
- inspect event triggers;
- inspect fork behavior;
- inspect artifact/log exposure;
- inspect whether secrets can be printed or written to repo files.
