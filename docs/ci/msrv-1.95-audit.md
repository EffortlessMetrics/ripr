# Rust 1.95 Compatibility Audit

Date: 2026-05-09

Rollout item: PR 01, `policy(msrv): audit Rust 1.95 compatibility`

## Result

Status: pass

`ripr` builds, tests, and passes the current lint profile under Rust 1.95. The
audit found no blocker to the follow-up MSRV bump PR.

This document is a compatibility receipt only. It does not change:

- `workspace.package.rust-version`
- `rust-toolchain.toml`
- `policy/clippy-lints.toml`
- active lint levels
- CI workflow behavior
- no-panic policy files

## Commands

Run from the repository root on Windows:

```bash
cargo +1.95 check --workspace --all-targets
cargo +1.95 test --workspace
cargo +1.95 clippy --workspace --all-targets -- -D warnings
```

Observed result:

| Command | Result |
| --- | --- |
| `cargo +1.95 check --workspace --all-targets` | pass |
| `cargo +1.95 test --workspace` | pass |
| `cargo +1.95 clippy --workspace --all-targets -- -D warnings` | pass |

`cargo +1.95 test --workspace` still prints expected stderr from tests that
exercise invalid command-line flags and missing `Cargo.toml` error handling.
Those are intentional test fixtures; the workspace test result is pass.

## Findings

- Rust 1.95 can compile the workspace with the current source and dependency
  graph.
- The current test suite passes under Rust 1.95.
- The current Clippy lint profile passes under Rust 1.95 with warnings denied.
- Planned Rust 1.94/1.95 lints remain planned; this audit does not activate
  them.

## Next Step

Proceed to PR 02:

```text
policy(msrv): move ripr to Rust 1.95
```

That PR should update the declared MSRV and toolchain policy. It should not
also promote the planned Clippy lints; PR 03 owns that ratchet.
