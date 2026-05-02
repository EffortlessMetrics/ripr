# Changelog

All notable repository-level changes are tracked here.

This project uses a human-readable changelog. Versioned release notes should
summarize user-visible behavior, compatibility notes, and migration guidance.
Internal planning, ADR, and spec changes should be called out when they affect
how future PRs are scoped or reviewed.

## Unreleased

### Added

- Added in-repo planning and tracking documentation for the staged path from
  published alpha to reliable live static exposure analysis.
- Added roadmap checkpoints for fixture work, syntax facts, probe ownership,
  oracle strength, local flow, activation values, LSP evidence, agent context,
  configuration, calibration, and caching.
- Added ADR, spec, metrics, learnings, and spec-test-code traceability
  scaffolding so future PRs can keep design, tests, and implementation aligned.
- Added contributor workflow, CI strategy, dogfooding, changelog policy, ADR
  template, and spec template docs.
- Added scoped evidence-heavy PR doctrine and first policy-check commands for
  static language and panic-family debt tracking.
- Added Rust-first file policy docs and checks for allowlisted non-Rust
  programming files, executable bits, and workflow shell budgets.
- Added spec format, fixture contract, and test taxonomy docs plus xtask checks
  for spec and fixture evidence shape.
- Added policy checks for generated files, dependency surfaces, process spawning,
  and network behavior.
- Added `cargo xtask shape` and `cargo xtask fix-pr` for safe local PR
  normalization and report generation.
- Added `cargo xtask pr-summary` for local reviewer packet generation from git
  diff and status.
- Added PR automation and goal-mode execution docs to pin down the remaining
  shape/check/guide automation path before deeper analyzer implementation.
- Added `cargo xtask precommit` and `cargo xtask check-pr` for cheap local
  guardrails and review-ready non-release checks.
- Added Markdown pass/fail reports for existing policy checks under
  `target/ripr/reports`.
- Added CI upload of generated PR reports as `ripr-pr-reports` artifacts.
- Added `cargo xtask fixtures` and `cargo xtask goldens check` scaffolding
  commands for validating fixture contracts and expected-output layout.
- Added `cargo xtask check-traceability` with `check-spec-ids` and
  `check-behavior-manifest` aliases for behavior manifest validation.
- Added `metrics/capabilities.toml`, `cargo xtask metrics`, and
  `cargo xtask check-capabilities` for capability status reporting.
- Added workspace-shape, architecture-boundary, and public API policy checks.
