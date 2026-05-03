# Changelog

All notable repository-level changes are tracked here.

This project uses a human-readable changelog. Versioned release notes summarize
user-visible behavior, compatibility notes, and migration guidance. Internal
planning, ADR, and spec changes are called out when they affect how future PRs
are scoped or reviewed.

## Unreleased

### Added

- Added internal local flow sink facts for changed expressions, including
  return values, error variants, struct fields, call effects, and match-arm
  results.
- Added activation evidence facts for observed test values and missing
  discriminator values, including boundary equality gaps and exact error
  variant gaps tied to local flow sinks.
- Added evidence-first human and JSON finding output that promotes changed
  behavior evidence paths, local flow sinks, observed values, missing
  discriminators, oracle kind/strength, and suggested next actions.
- Added negative and metamorphic fixture coverage for whitespace/comment/import
  noise, unrelated token mentions, strong boundary/error oracles, and equivalent
  assertion/test-layout variants.
- Passed VS Code `ripr.check.mode` and `ripr.baseRef` settings into LSP
  workspace diagnostics.
- Stored the latest LSP analysis snapshot alongside diagnostics so future
  hover, code-action, and context paths can resolve findings without rerunning
  analysis.
- Scoped LSP diagnostic ranges to the probe source column and expression width
  instead of marking a fixed line prefix.
- Added a framed LSP protocol smoke test for initialize, didOpen, refresh,
  hover, codeAction, shutdown, and exit over the tower server.

## 0.3.0 - 2026-05-02

### Added

- Added the syntax-backed analyzer foundation: `FileFacts`,
  `RustSyntaxAdapter`, parser-backed test/oracle extraction, stable owner
  symbols, and parser-backed predicate, return, error, field, match,
  side-effect, and call-change probes.
- Added the Evidence Quality foundation: unknown findings now carry explicit
  stop reasons, and oracle kind/strength is probe-relative for exact values,
  exact error variants, broad errors, snapshots, mock expectations, relational
  checks, smoke-only checks, and unknown oracles.
- Added fixture, golden, report, metrics, traceability, dogfood, test-oracle,
  report-index, receipt, golden-drift, critic, local-context, allow-attribute,
  supply-chain, and workflow-runtime automation for reviewable PR evidence.
- Added `tower-lsp-server` as the LSP framework and moved the sidecar to typed
  async handlers.
- Added LSP state and evidence surfaces: workspace-root selection from
  initialization, stale diagnostic clearing, refresh failure logging, document
  state tracking, saved-workspace refresh semantics, serialized refresh
  generations, stable diagnostic metadata, related test information,
  diagnostic-targeted context actions, and diagnostic hover details.
- Added CI and release hardening: coverage workflow, cargo-deny supply-chain
  checks, GitHub Dependency Review, Dependabot configuration, Node 24 workflow
  action/tooling updates, and Open VSX publishing through `OVSX_PAT`.

### Changed

- Reworked the README as a problem-first front door and moved detailed operating
  guidance into docs.
- Upgraded the Rust baseline to 1.93 and added high-signal Rust/Clippy lint
  gates.
- Split larger internal modules for CLI, domain, JSON output, and LSP sidecar
  responsibilities without changing the one-package public surface.

### Fixed

- Hardened unified diff parsing against multi-hunk, multi-file, malformed, and
  fuzz-like inputs.
- Expanded output, CLI, classifier, app mode, snapshot oracle, workspace
  selection, rustdoc, and LSP unit coverage.
- Improved golden snapshot drift diagnostics and normalized golden text
  comparison around trailing newlines.

## 0.2.0 - 2026-05-01

- First self-provisioning editor distribution path.
- Added `ripr lsp --stdio` and `ripr lsp --version`.
- Added VS Code/Open VSX server resolution:
  `ripr.server.path` -> bundled -> cached download -> verified first-run
  download -> PATH -> actionable error.
- Added GitHub Release server archives and a SHA-256 manifest used by the
  extension downloader.
- Published the universal VSIX and Open VSX extension.

## 0.1.0 - 2026-05-01

- First publishable alpha of `ripr`: static RIPR exposure analysis for
  Rust/Cargo workspaces.
