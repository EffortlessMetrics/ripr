## Summary

-

## Scope

-

## Scope Classification

This PR is scoped by:

- [ ] One production behavior
- [ ] One public contract
- [ ] One architectural seam
- [ ] Docs/spec/test-only evidence package
- [ ] Other:

Production delta:

-

Evidence/support delta:

-

Single acceptance criterion:

-

Non-goals:

-

## Spec-Test-Code Traceability

- Spec:
- Tests:
- Code:
- Golden outputs:
- Metrics:
- ADR/learning:

## Static Language Check

- [ ] Static output avoids `killed`, `survived`, `untested`, `proven`, and `adequate`.
- [ ] Unknowns include stop reasons where applicable.

## Engineering Check

- [ ] No new `panic`, `unwrap`, `expect`, `todo`, or `unimplemented` in production code.
- [ ] No new `panic`, `unwrap`, `expect`, `todo`, or `unimplemented` in tests.
- [ ] New non-Rust programming files are allowlisted with owner, surface, and reason.
- [ ] Errors are reported with actionable context.
- [ ] Public JSON/schema changes are documented.

## Gates

- [ ] `cargo fmt --check`
- [ ] `cargo check --workspace --all-targets`
- [ ] `cargo test --workspace`
- [ ] `cargo clippy --workspace --all-targets -- -D warnings`
- [ ] `cargo doc --workspace --no-deps`
- [ ] `cargo package -p ripr --list`
- [ ] `cargo publish -p ripr --dry-run`
- [ ] `cargo xtask check-static-language`
- [ ] `cargo xtask check-no-panic-family`
- [ ] `cargo xtask check-file-policy`
- [ ] `cargo xtask check-executable-files`
- [ ] `cargo xtask check-workflows`
- [ ] `cargo xtask check-spec-format`
- [ ] `cargo xtask check-fixture-contracts`

Extension changes:

- [ ] `cd editors/vscode && npm ci`
- [ ] `cd editors/vscode && npm run compile`
- [ ] `cd editors/vscode && npm run package`
