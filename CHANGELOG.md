# Changelog

All notable repository-level changes are tracked here.

This project uses a human-readable changelog. Versioned release notes summarize
user-visible behavior, compatibility notes, and migration guidance. Internal
planning, ADR, and spec changes are called out when they affect how future PRs
are scoped or reviewed.

## Unreleased

### Added

- Closed Campaign 4B (Repo Seam Inventory and Test Grip) and made repo
  seam evidence first-class: `RepoSeam` / `SeamId` / `SeamKind` /
  `RequiredDiscriminator` /
  `ExpectedSink` / `SeamGripClass` data model with deterministic 16-char
  FNV-1a seam IDs (#229); production-file seam inventory walker writing
  `target/ripr/reports/repo-seams.{json,md}` (#235); `TestGripEvidence`
  + `RelatedTestGrip` attaching reach/activate/propagate/observe/
  discriminate evidence per seam (#236); seam classification mapping
  evidence to one of 11 spec classes with explicit headline-eligibility
  table (#237); repo exposure report at
  `target/ripr/reports/repo-exposure.{json,md}` with per-class metric
  buckets (#239); agent seam packets at
  `target/ripr/reports/agent-seam-packets.json` carrying
  `write_targeted_test` work orders for headline-eligible seams and
  `inspect_static_limitation` for opaque seams (#240); LSP seam
  diagnostics with stable `ripr-seam-{class}` codes behind
  `seamDiagnostics: true` opt-in (#241); seam-native LSP hover that
  looks up `ClassifiedSeam` via `data.seam_id` and renders the RIPR
  evidence path (#242); and `docs/AGENT_DISPATCH_WORKFLOW.md`
  documenting the practical loop (#248). Static output keeps the
  audit vocabulary; runtime mutation testing remains a separate
  confirmation step.
- Started Campaign 5 (Adoption and Calibration). `cache/repo-seam-facts-v1`
  and `calibration/cargo-mutants-v1` carry forward from Campaign 4B as
  ready items; `config/ripr-config-v1` and `ci/sarif-ci-policy` remain
  blocked on the cache and config respectively.
- Reframed Campaign 5 as Campaign 5A (Seam Evidence Usability and Precision)
  to focus the queue on four product axes — fast (cache), precise
  (related-test-precision-v1, value-extraction-v2, oracle-shape-v2),
  actionable (agent-seam-packets-v2, lsp/seam-code-actions-v1), and
  calibrated (cargo-mutants-v1). Operationalization items
  (`config/ripr-config-v1`, `ci/sarif-ci-policy`, future
  `badge/seam-native-count-mapping`) move to Campaign 5B and stay
  blocked behind 5A's cache and oracle-shape work. Cache
  serialization policy: never bincode; postcard if binary; fact
  layers only.
- Renamed durable Campaign 5A wording from "Voice B" to "seam
  evidence" across manifest, docs, README, and rendered report
  Markdown; marked `cache/repo-seam-facts-v1` done after #255 merged.
  State-only PR; no analyzer behavior, cache behavior, or output
  schema changes. The manifest campaign id is now
  `seam-evidence-usability-and-precision`.
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
- Closed Campaign 3 and added the advisory Test Efficiency and Vacuity Signals
  lane for per-test evidence ledgers, likely-vacuity signals, and duplicate
  discriminator reports.
- Added `cargo xtask test-efficiency-report`, an advisory per-test evidence
  ledger that reports apparent owner calls, oracle kind/strength, observed
  literal values, and static limitations.
- Extended the test-efficiency report with advisory reason counts for
  smoke-only, broad-oracle, disconnected, opaque, circular, and likely-vacuous
  signals.
- Passed VS Code `ripr.check.mode` and `ripr.baseRef` settings into LSP
  workspace diagnostics.
- Stored the latest LSP analysis snapshot alongside diagnostics so future
  hover, code-action, and context paths can resolve findings without rerunning
  analysis.
- Scoped LSP diagnostic ranges to the probe source column and expression width
  instead of marking a fixed line prefix.
- Added a framed LSP protocol smoke test for initialize, didOpen, refresh,
  hover, codeAction, shutdown, and exit over the tower server.
- Added `cargo xtask mutation-calibration`, an advisory cargo-mutants import
  scaffold that joins runtime mutation records to static seam evidence by
  `seam_id` or unambiguous normalized file/line and writes
  `target/ripr/reports/mutation-calibration.{json,md}`. Span-based generated
  mutant locations are imported, and ambiguous file/line candidates remain
  unassigned. Runtime mutation vocabulary stays confined to calibration/runtime
  reports.
- Closed Campaign 5A (Seam Evidence Usability and Precision) after the cache,
  related-test precision, value extraction, oracle-shape, agent packet, LSP code
  action, and cargo-mutants calibration chain landed (#255, #310, #313, #314,
  #315, #316, #327). The active manifest now moves to Campaign 5B
  Operationalization with `config/ripr-config-v1` as the next ready item and
  SARIF / seam-native badge policy blocked behind config.
- Added repo-root `ripr.toml` configuration for Campaign 5B. Config can set
  analysis mode, oracle policy for snapshots/mocks/broad errors, finding and
  seam severity mapping, suppressions path, related-test report caps, and LSP
  seam-diagnostic defaults. Missing config preserves existing defaults, unknown
  keys fail loudly, and explicit CLI flags or LSP initialization options still
  win. SARIF and seam-native badge remapping remain out of scope for this PR.
- Added `ripr doctor` visibility for repository config. Doctor now reports
  whether `ripr.toml` was loaded, which effective defaults are active, and
  malformed config errors without printing config source text.
- Added RIPR-SPEC-0008 to define the Campaign 5B SARIF and CI policy contract:
  stable Finding and seam rule IDs, configured severity mapping, suppression
  visibility, advisory defaults, and opt-in baseline policy modes.

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
