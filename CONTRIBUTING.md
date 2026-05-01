# Contributing

`ripr` is built PR by PR. Each PR should be small enough to review, but complete
enough to leave the repository in a better state than it found it.

## Product Contract

Before changing code, check the product question:

```text
For the behavior changed in this diff, do the current tests appear to contain
a discriminator that would notice if that behavior were wrong?
```

Changes that do not improve the precision, speed, usability, calibration, or
maintainability of that answer should be deferred.

## PR Shape

Prefer one PR per capability step from [Implementation plan](docs/IMPLEMENTATION_PLAN.md).

Each PR should include:

- scoped implementation or documentation changes
- tests or documented verification
- relevant docs updates
- changelog entry when behavior, workflow, or public docs change
- traceability from spec to tests to code for behavior changes

## Review Checklist

Before requesting review:

- [ ] Scope matches one roadmap or implementation-plan item.
- [ ] New behavior has a spec entry or updates an existing spec.
- [ ] Tests use BDD-shaped names or fixture names that explain the behavior.
- [ ] Output changes update golden expectations and schema docs.
- [ ] Static output avoids mutation-runtime outcome language.
- [ ] Unknowns include stop reasons where applicable.
- [ ] No new `panic`, `unwrap`, `expect`, `todo`, or `unimplemented` in
      production or test code.
- [ ] CI-relevant docs or workflows were updated when gates changed.

## Required Rust Gates

```bash
cargo fmt --check
cargo check --workspace --all-targets
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo doc --workspace --no-deps
cargo package -p ripr --list
cargo publish -p ripr --dry-run
```

When the worktree is intentionally dirty during local review, Cargo packaging
commands may require `--allow-dirty`. A branch is not ready to merge until the
plain commands pass on a committed tree.

## Required Extension Gates

For changes under `editors/vscode`:

```bash
cd editors/vscode
npm ci
npm run compile
npm run package
```

## Documentation

Use the documentation map in [Documentation system](docs/DOCUMENTATION.md).

For behavior changes, update:

- [Specs](docs/specs/README.md)
- [Spec-test-code traceability](docs/SPEC_TEST_CODE.md)
- [Testing](docs/TESTING.md)
- output or config reference docs when public shapes change

For decisions, add or update an ADR.

For repo knowledge, update [Learnings](docs/LEARNINGS.md).
