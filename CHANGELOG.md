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
- Added PR automation docs to pin down the remaining shape/check/guide
  automation path before deeper analyzer implementation.
- Added Codex Goals, implementation campaign, and scoped PR contract docs to
  distinguish multi-PR campaign execution from PR-sized work items.
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
- Added output contract registry checks for schema versions and public enum
  strings.
- Added docs index checks for spec, ADR, README, and documentation front-door
  links.
- Added advisory PR-shape warnings for likely missing evidence in changed-file
  sets.
- Added README state and repo-local Markdown link checks to keep documentation
  references and capability checkpoints aligned before review.
- Added Codex Goals campaign manifest checks plus `goals status` and
  `goals next` reports for `.ripr/goals/active.toml`.
- Added fixture/golden runner comparison so `cargo xtask fixtures` and
  `cargo xtask goldens check` execute `ripr` and compare actual outputs against
  expected fixture goldens.
- Added the first behavior fixtures, `boundary_gap` and `weak_error_oracle`,
  with checked JSON and human expected outputs.
- Added advisory `cargo xtask test-oracle-report` and `check-test-oracles`
  commands for measuring `ripr`'s own Rust test oracle strength.
- Added advisory `cargo xtask dogfood` reports that run `ripr` against stable
  fixture diffs and record current self-check output.
- Completed the Agentic DevEx Foundation campaign and activated the
  Syntax-Backed Analyzer Foundation campaign with `analysis/file-facts-model`
  as the next ready work item.
- Added the internal `FileFacts` model and related function, test, oracle,
  call, return, and literal facts while preserving current analyzer output.
- Added the `RustSyntaxAdapter` boundary with a lexical adapter and changed-line
  node facts while preserving fixture and golden output.
- Added ADR 0006 to select `ra_ap_syntax` as the Campaign 2 Rust parser
  substrate while keeping parser-specific types behind the syntax adapter.
- Added parser-backed test and oracle extraction through `RustSyntaxAdapter`,
  including stacked test attributes, multi-line assertion macros, and
  unwrap/expect smoke-oracle facts without fixture output drift.
- Added module- and impl-qualified parser-backed owner symbols so changed-line
  probes attach to stable function and method owners without fixture output
  drift.
- Added parser-backed probe shape facts so current predicate, return, error,
  field, match, side-effect, and call probes can be generated from syntax facts
  with lexical fallback and stable fixture output.
- Completed the Syntax-Backed Analyzer Foundation campaign and activated the
  Evidence Quality campaign with unknown stop reasons and oracle strength as
  the next ready work items.
- Added `cargo xtask reports index` to write a reviewer front door for generated
  report artifacts and surface it in CI job summaries.
- Added `cargo xtask receipts` and `cargo xtask receipts check` for
  machine-readable gate receipts under `target/ripr/receipts`.
- Reworked the README as a problem-first front door that links deeper roadmap,
  automation, capability, and contributor docs instead of carrying the full map.
- Added `cargo xtask golden-drift` and wired `goldens check` to write semantic
  drift summaries for fixture expected-output review.
- Added `cargo xtask check-local-context` to reject committed local machine
  paths, session-state artifacts, and chat/runtime references before review.
- Added high-signal workspace Clippy denies for `dbg_macro`, `todo`, and
  `unimplemented`, plus `cargo xtask check-allow-attributes` to prevent
  bypassing guarded lint families without reviewed allowlist entries.
- Added static/manual VS Marketplace badges and live Open VSX badges to the
  extension README, plus release-doc maintenance steps for refreshing the
  manual Marketplace install count.
- Switched Open VSX release workflow and docs to the `OVSX_PAT` repository
  secret and added manual workflow switches for registry-specific extension
  publishing.
- Added root README status/package/marketplace badges, reset manual VS
  Marketplace install badges to the first-launch `0 installs` seed, and added
  an advisory Codecov coverage workflow.
- Added cargo-deny supply-chain policy, a security workflow with GitHub
  Dependency Review, and `cargo xtask check-supply-chain`.
- Moved GitHub workflow actions to Node-24-backed majors where available,
  moved extension build and publish workflows to Node 24, updated extension
  Node typings, and added workflow runtime policy checks with a documented
  Dependency Review action exception.
- Added the unknown stop-reason invariant so unknown findings surface explicit
  stop reasons across domain, JSON/context, GitHub annotation, and human output.
- Added probe-relative oracle kind and strength classification so exact error
  variants, exact values, broad errors, relational checks, snapshots, mock
  expectations, smoke-only checks, and unknown oracles are ranked by the changed
  probe family.
- Adopted `tower-lsp-server` for the experimental LSP sidecar, replacing
  hand-rolled JSON-RPC framing with typed async LSP handlers while preserving
  current diagnostics, hover, and code action behavior.
- Added `MIT-0` to the cargo-deny license allowlist for the `tower-lsp-server`
  dependency graph.
