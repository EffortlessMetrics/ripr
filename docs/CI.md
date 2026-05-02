# CI Strategy

CI should protect correctness without making ordinary contribution slow or
noisy. Default CI is advisory for static exposure findings until calibration and
configuration are mature enough to support opt-in failure policies.

## Current Workflows

The Rust workflow currently runs:

```bash
cargo fmt --check
cargo check --workspace --all-targets
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo xtask check-static-language
cargo xtask check-no-panic-family
cargo xtask check-file-policy
cargo xtask check-executable-files
cargo xtask check-workflows
cargo xtask check-spec-format
cargo xtask check-fixture-contracts
cargo xtask check-generated
cargo xtask check-dependencies
cargo xtask check-process-policy
cargo xtask check-network-policy
cargo package -p ripr --list
cargo publish -p ripr --dry-run
```

Local policy checks can also be run directly:

```bash
cargo xtask check-static-language
cargo xtask check-no-panic-family
cargo xtask check-file-policy
cargo xtask check-executable-files
cargo xtask check-workflows
cargo xtask check-spec-format
cargo xtask check-fixture-contracts
cargo xtask check-generated
cargo xtask check-dependencies
cargo xtask check-process-policy
cargo xtask check-network-policy
```

The VS Code workflow currently runs:

```bash
cd editors/vscode
npm ci
npm run compile
npm run package
```

Release workflows handle extension publishing and server binary releases.

## Principles

- Fast gates first: formatting, check, clippy, and tests should fail early.
- Packaging gates matter: crates.io packaging catches missing files and metadata
  drift.
- Extension gates stay separate: Node setup should not slow Rust-only PRs.
- Policy gates should be mechanical and allowlisted while existing debt is paid
  down.
- Rust-first file policy keeps repo automation in `xtask` instead of ad hoc
  scripts.
- Blocking `ripr` findings are opt-in until SARIF policy, baselines, and
  calibration exist.
- CI changes require documentation updates.

## Future Improvements

Planned CI work:

- cache Cargo and npm dependencies without hiding stale-lockfile failures
- add fixture-golden tests once the fixture lab exists
- add markdown/link checks for docs-heavy PRs
- add traceability and capability-matrix checks
- add workspace-shape, architecture, public API, docs-index, and PR-summary
  checks
- add SARIF validation when SARIF output exists
- add opt-in policy modes:
  - advisory
  - warn-only
  - fail-on-no-static-path
  - fail-on-high-confidence-gap
  - top-N-only
  - baseline-aware

## Merge Criteria

A branch is ready to merge when:

- required gates for touched areas pass on a committed tree
- docs and changelog are updated for user-visible changes
- static output language rules are preserved
- spec-test-code traceability is present for behavior changes

Local `--allow-dirty` packaging checks are useful during review but are not a
substitute for plain package and publish dry-run checks on the final committed
branch.
