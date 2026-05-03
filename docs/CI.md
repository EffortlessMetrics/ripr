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
cargo xtask check-allow-attributes
cargo xtask check-local-context
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

The CI workflow also has an explicit MSRV job that pins Rust `1.93.1` and runs:

```bash
cargo check --workspace --all-targets
```

The main Rust job stays on `stable` so routine CI also proves the current stable
toolchain, while the MSRV job proves the declared workspace baseline.

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
cargo xtask golden-drift
cargo xtask test-oracle-report
cargo xtask dogfood
cargo xtask critic
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
expected-output scaffolding without accepting output drift. `golden-drift`
writes advisory Markdown and JSON summaries of semantic expected-output drift
for reviewers. `test-oracle-report` writes an advisory baseline for the strength
of `ripr`'s own Rust test oracles. `dogfood` writes a non-blocking
`ripr`-on-`ripr` report from stable fixture diffs. `critic` writes an advisory
adversarial review packet from the current diff, reports, and receipts.
`reports index` writes a reviewer front door for generated reports.
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
cargo xtask check-allow-attributes
cargo xtask check-local-context
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
cargo xtask check-supply-chain
cargo xtask check-process-policy
cargo xtask check-network-policy
```

Fixture and golden scaffolding checks can be run directly with:

```bash
cargo xtask fixtures
cargo xtask goldens check
cargo xtask golden-drift
cargo xtask test-oracle-report
cargo xtask dogfood
cargo xtask critic
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

The VS Code extension build and extension publish workflows use Node 24. This
is separate from the VS Code extension-host compatibility declared in
`editors/vscode/package.json`.

The coverage workflow currently runs:

```bash
cargo llvm-cov clean --workspace
cargo llvm-cov --workspace --all-features --lcov --output-path lcov.info
```

It uploads `lcov.info` as the `rust-lcov` GitHub Actions artifact and uploads
the same file to Codecov with the `rust` flag and `rust-workspace` upload name.

Codecov uses the repository `CODECOV_TOKEN` secret. Codecov upload failures are
blocking for trusted coverage runs: pushes and same-repository pull requests.
Fork pull requests still generate `lcov.info` and upload the `rust-lcov`
GitHub Actions artifact, but skip the Codecov upload because repository secrets
are unavailable to those runs.

Codecov project and patch status checks are not yet branch-protection gates.
After the emitted status names and baseline are stable, a later scoped PR can
ratchet Codecov status requirements and branch protection separately.

The security workflow currently runs:

```bash
cargo deny check advisories licenses bans sources
```

It uses `deny.toml` to enforce RustSec advisories, license policy, banned
crates, and approved dependency sources. Duplicate dependency findings are
warnings while the `ra_ap_syntax` dependency graph is being baselined.

Pull requests also run GitHub Dependency Review for high-severity vulnerability
alerts and denied license families. Dependency Graph is enabled for the
repository, so Dependency Review is a blocking security gate.

## GitHub Actions Runtime Policy

GitHub-hosted action majors should use Node-24-backed releases where official
releases exist. `cargo xtask check-workflows` rejects old action refs such as
`actions/checkout@v4`, `actions/setup-node@v4`, artifact v4 actions, and
`codecov/codecov-action@v4`.

`actions/dependency-review-action@v4` is temporarily allowlisted in
`policy/workflow_action_runtime_allowlist.txt` because the official Dependency
Review action still declares a Node 20 runtime and no Node-24-backed major is
available. Keep Dependency Review enabled until a supported replacement exists.

The same cargo-deny check can be run locally with:

```bash
cargo xtask check-supply-chain
```

Dependabot is configured in `.github/dependabot.yml` for Cargo dependencies,
the VS Code extension npm package, and GitHub Actions. Routine version-update
PRs are limited to minor and patch updates. Major updates should be deliberate,
scoped PRs because they often change toolchain, release, or runtime behavior.
Dependabot PRs are not auto-merged; they must pass the normal CI, coverage,
security, and `xtask` checks before merge.

GitHub-hosted security settings are tracked in
[Repository settings](REPO_SETTINGS.md). Dependency Graph, Dependabot alerts,
Dependabot security updates, secret scanning, push protection, and private
vulnerability reporting are settings, not workflow files. Keep that document
updated when repository settings change.

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
- add markdown/link checks for docs-heavy PRs
- add README capability snapshot consistency checks
- add README state and Markdown link checks
- ratchet Codecov project and patch status requirements after the first stable
  coverage baseline
- decide when duplicate dependency findings should become blocking after the
  cargo-deny baseline is stable
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
