# Testing

Tests should prove product behavior, not implementation trivia. For analyzer and
output work, use BDD-shaped names and fixtures that make the behavior question
plain:

```text
given_changed_boundary_when_equal_value_is_missing_then_reports_weak_exposure
```

Behavior changes should have a three-way match:

```text
spec -> test -> code
```

See [Spec-test-code traceability](SPEC_TEST_CODE.md) for the expected mapping.
See [Test taxonomy](TEST_TAXONOMY.md) for required proof levels by change type.

Run everything:

```bash
cargo xtask shape
cargo xtask fix-pr
cargo xtask pr-summary
cargo xtask precommit
cargo xtask check-pr
cargo xtask fixtures
cargo xtask goldens check
cargo xtask golden-drift
cargo fmt --check
cargo check --workspace --all-targets
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo doc --workspace --no-deps
cargo xtask ci-fast
cargo xtask ci-full
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
cargo xtask check-pr-shape
cargo xtask check-generated
cargo xtask check-generated-clean
cargo xtask check-dependencies
cargo xtask check-process-policy
cargo xtask check-network-policy
```

Package check:

```bash
cargo package -p ripr --list
cargo publish -p ripr --dry-run
```

The current test suite covers:

- unified diff parsing
- Rust test/assertion extraction
- JSON escaping
- simple end-to-end diff analysis
- CLI smoke behavior

## Error-Handling Bar

The target rule is:

```text
No panic, unwrap, expect, todo, or unimplemented in production or tests.
```

New tests should return `Result` when setup can fail and should use explicit
assertions. Existing panic-family usage is tracked engineering debt and should
be paid down in scoped PRs rather than copied into new tests.

## VS Code Extension Tests

The VS Code extension smoke tests run inside a real VS Code instance through
`@vscode/test-electron`:

```bash
cd editors/vscode
npm ci
npm run test:e2e
```

The test suite:

- opens a fixture Rust workspace (`test-fixtures/workspace/Cargo.toml`)
- activates the extension
- asserts commands are registered (`ripr.restartServer`, `ripr.showOutput`,
  `ripr.copyContext`, `ripr.copySuggestedAssertion`,
  `ripr.copyTargetedTestBrief`, `ripr.copyAgentPacketCommand`,
  `ripr.copyAgentBriefCommand`, `ripr.copyAfterSnapshotCommand`,
  `ripr.copyAgentVerifyCommand`, `ripr.copyAgentReceiptCommand`,
  `ripr.openRelatedTest`, `ripr.openSettings`)
- verifies the defaults-first editor check mode is `draft`
- verifies `copyContext` completes without crash when no editor is active
- verifies `copyContext` accepts a structured target with `finding_id` and
  `probe_id` without crashing
- verifies `copyContext` asks LSP `ripr.collectContext` first for `seam_id`
  targets and falls back to the CLI when LSP returns no packet or errors
- verifies suggested-assertion and targeted-test-brief commands copy valid
  payloads and ignore malformed arguments without throwing
- verifies agent-loop command copying writes command text and the contributed
  command handlers ignore malformed arguments without throwing
- verifies LSP agent-loop command payloads stay workspace-relative across
  platform-shaped roots and fail closed for stale seam diagnostics
- verifies the live real-server boundary-gap path publishes a seam diagnostic,
  renders hover evidence, exposes seam actions, copies seam packet and verify
  command payloads, and opens the best related test
- verifies explicit editor status states for disabled config, missing
  workspace, unavailable server, queued/running/complete/no-seam/failed
  refreshes, and stale dirty Rust buffers that stay stale until save or close
- verifies an existing workspace-matched `first-useful-action.json` is projected
  through status bar and `ripr: Show Status` without overriding stale evidence
  or accepting reports for another workspace
- verifies `openRelatedTest` opens URI/line targets and ignores malformed
  arguments without throwing
- verifies `restartServer` is callable even when server resolution fails

CI runs the suite headless with `xvfb-run -a npm run test:e2e`. The test
runner stores downloaded VS Code archives under
`target/ripr/vscode-test-cache` so generated editor-host state stays with the
rest of the repo-local build output.

## Golden Output

When changing user-visible output, update or add golden coverage for:

- human output
- JSON output
- context packets
- LSP diagnostic shape, when applicable

Golden updates must preserve the static language boundary: draft static output
does not use mutation-runtime terms such as `killed` or `survived`.

## CI Test Results

CI uploads Rust test results to Codecov Test Analytics from JUnit XML generated
by `cargo nextest`:

```bash
cargo nextest run --workspace --all-features --profile ci
```

Doc tests remain a separate Cargo test step:

```bash
cargo test --workspace --doc
```

The JUnit output path is configured in `.config/nextest.toml` at
`target/nextest/ci/junit.xml`.
