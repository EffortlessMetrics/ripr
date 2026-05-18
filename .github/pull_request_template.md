## Summary

-

## Scope

-

## Source-of-truth links

Proposal:
Spec:
ADR:
Plan item:
Active goal:
Support/status impact:
Policy impact:

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

## CI Economics

Complete this section when the PR changes workflows, policy gates, branch
protection expectations, CI artifacts, report uploads, release checks, or the
cost/posture of existing lanes. Use `n/a` for ordinary PRs that do not affect
CI behavior.

- LEM impact:
- Workflows touched:
- Branch protection impact:
- Failure mode caught:
- Cheaper signal considered:
- Required lanes affected:
- Advisory lanes affected:
- On-demand/release lanes affected:
- Labels that alter behavior:
- Artifact families affected:
- Rollback path:

If rollback requires branch-protection or workflow-file changes, say so here
and split the PR unless the workflow change itself is the narrow reviewed
surface. If a CI-breaking rollback would need emergency procedures because
normal CI cannot validate the revert, document that explicitly.

## Engineering Check

- [ ] No new `panic`, `unwrap`, `expect`, `todo`, or `unimplemented` in production code.
- [ ] No new `panic`, `unwrap`, `expect`, `todo`, or `unimplemented` in tests.
- [ ] New non-Rust programming files are allowlisted with owner, surface, and reason.
- [ ] New generated, dependency, process-spawn, or network surfaces are allowlisted with owner and reason.
- [ ] Errors are reported with actionable context.
- [ ] Public JSON/schema changes are documented.

## Proof

```bash
# commands run
```

Results:

-

Claim boundary:

-

Rollback:

-

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
- [ ] `cargo xtask check-allow-attributes`
- [ ] `cargo xtask check-local-context`
- [ ] `cargo xtask check-file-policy`
- [ ] `cargo xtask check-executable-files`
- [ ] `cargo xtask check-workflows`
- [ ] `cargo xtask check-spec-format`
- [ ] `cargo xtask check-spec-numbering`
- [ ] `cargo xtask check-fixture-contracts`
- [ ] `cargo xtask check-generated`
- [ ] `cargo xtask check-badge-diff-policy`
- [ ] `cargo xtask check-generated-clean`
- [ ] `cargo xtask check-dependencies`
- [ ] `cargo xtask check-process-policy`
- [ ] `cargo xtask check-network-policy`

Extension changes:

- [ ] `cd editors/vscode && npm ci`
- [ ] `cd editors/vscode && npm run compile`
- [ ] `cd editors/vscode && npm run package`
