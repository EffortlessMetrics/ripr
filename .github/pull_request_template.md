## Summary

-

## Scope

-

## Spec-Test-Code Traceability

- Spec:
- Tests:
- Code:
- Golden outputs:

## Static Language Check

- [ ] Static output avoids `killed`, `survived`, `untested`, `proven`, and `adequate`.
- [ ] Unknowns include stop reasons where applicable.

## Engineering Check

- [ ] No new `panic`, `unwrap`, `expect`, `todo`, or `unimplemented` in production code.
- [ ] No new `panic`, `unwrap`, `expect`, `todo`, or `unimplemented` in tests.
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

Extension changes:

- [ ] `cd editors/vscode && npm ci`
- [ ] `cd editors/vscode && npm run compile`
- [ ] `cd editors/vscode && npm run package`
