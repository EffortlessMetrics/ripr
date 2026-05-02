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
cargo xtask check-traceability
cargo xtask check-capabilities
cargo xtask check-workspace-shape
cargo xtask check-architecture
cargo xtask check-public-api
cargo xtask check-output-contracts
cargo xtask check-doc-index
cargo xtask check-readme-state
cargo xtask markdown-links
cargo xtask check-campaign
cargo xtask check-pr-shape
cargo xtask check-generated
cargo xtask check-dependencies
cargo xtask check-process-policy
cargo xtask check-network-policy
cargo package -p ripr --list
cargo publish -p ripr --dry-run
```

Local shaping commands are intentionally separate from CI because they mutate
the worktree:

```bash
cargo xtask shape
cargo xtask fix-pr
cargo xtask pr-summary
cargo xtask precommit
cargo xtask check-pr
cargo xtask fixtures
cargo xtask goldens check
cargo xtask test-oracle-report
cargo xtask dogfood
cargo xtask reports index
cargo xtask receipts
cargo xtask receipts check
```

They are safe to run before checks. `shape` runs `cargo fmt`, sorts allowlists,
ensures `target/ripr/reports`, and writes a local report. `fix-pr` currently
runs `shape`, refreshes `pr-summary`, and writes a local fix-pr report.
`pr-summary` writes `target/ripr/reports/pr-summary.md` from git diff/status.
`precommit` is the cheap non-mutating local guardrail. `check-pr` is the
review-ready local gate and intentionally does not run package or publish
dry-run checks. `fixtures` and `goldens check` validate the current fixture and
expected-output scaffolding without accepting output drift. `test-oracle-report`
writes an advisory baseline for the strength of `ripr`'s own Rust test oracles.
`dogfood` writes a non-blocking `ripr`-on-`ripr` report from stable fixture
diffs. `reports index` writes a reviewer front door for generated reports.
`receipts` writes machine-readable gate evidence under `target/ripr/receipts`,
and `receipts check` validates the receipt set.

The fuller automation model is documented in [PR automation](PR_AUTOMATION.md).
Deterministic shaping should happen locally; CI should verify the committed
tree and upload reports when available.

Codex Goals runs should treat CI artifacts as campaign receipts. A campaign can
advance through multiple work items, but each scoped PR should leave the same
shape/check/report artifacts that CI uploads for human review.

Current policy checks write Markdown reports to `target/ripr/reports` when they
run. The Rust workflow generates `target/ripr/reports/index.md`, writes it to
the GitHub Actions job summary when present, and uploads the report and receipt
directories as the `ripr-pr-reports` artifact.

Local policy checks can also be run directly:

```bash
cargo xtask check-static-language
cargo xtask check-no-panic-family
cargo xtask check-file-policy
cargo xtask check-executable-files
cargo xtask check-workflows
cargo xtask check-spec-format
cargo xtask check-fixture-contracts
cargo xtask check-traceability
cargo xtask check-capabilities
cargo xtask check-workspace-shape
cargo xtask check-architecture
cargo xtask check-public-api
cargo xtask check-output-contracts
cargo xtask check-doc-index
cargo xtask check-readme-state
cargo xtask markdown-links
cargo xtask check-campaign
cargo xtask check-pr-shape
cargo xtask check-generated
cargo xtask check-dependencies
cargo xtask check-process-policy
cargo xtask check-network-policy
```

Fixture and golden scaffolding checks can be run directly with:

```bash
cargo xtask fixtures
cargo xtask goldens check
cargo xtask test-oracle-report
cargo xtask dogfood
cargo xtask reports index
cargo xtask receipts
cargo xtask receipts check
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
- decide whether CI should call `check-pr` directly or keep the current
  explicit workflow steps
- wire fixture and golden checks into CI once the fixture lab has executable
  behavior fixtures
- add markdown/link checks for docs-heavy PRs
- add README capability snapshot consistency checks
- add README state and Markdown link checks
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
