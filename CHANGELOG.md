# Changelog

All notable repository-level changes are tracked here.

This project uses a human-readable changelog. Versioned release notes summarize
user-visible behavior, compatibility notes, and migration guidance. Internal
planning, ADR, and spec changes are called out when they affect how future PRs
are scoped or reviewed.

## Unreleased

### 0.5.1 — quality rollout planned

The 0.5.1 patch release carries policy hardening, CI economics, DevEx improvements, and
targeted Rust 1.95 API cleanup on top of the 1.95 MSRV foundation that landed in 0.5.0.
No new user-visible features. No public API changes. MSRV remains 1.95.

Planned scope:

- No-panic allowlist exact-identity hardening (path + family + selector + snippet + count).
- Clippy ledger and checker alignment for all active and planned lints.
- Companion file-policy ledgers for generated, executable, dependency, workflow, process, and
  network surfaces.
- Evidence lane tuning: mutation stays targeted/nightly/release, not a default PR tax.
- Targeted Rust 1.95 API cleanup in evidence and report builders.
- Release readiness workflow and 0.5.1 dry-run proof.
- Public product-copy cleanup: VS Code marketplace title restored to
  `ripr: Static Mutation Exposure`, plain-language first-hour copy in
  `README.md`, `editors/vscode/README.md`, `docs/QUICKSTART.md`, and
  `docs/EDITOR_EXTENSION.md`. Internal vocabulary (seams, discriminators,
  status IDs, schemas, commands) is unchanged.
- Added [docs/TERMINOLOGY.md](docs/TERMINOLOGY.md): plain-language ↔ internal
  vocabulary bridge linked from `README.md`, `docs/QUICKSTART.md`,
  `docs/EDITOR_EXTENSION.md`, and `editors/vscode/README.md`. No schema, JSON,
  status ID, or command renames.
- Generated CI advisory job summary now uses reviewer-friendly section
  headings: `PR review front panel` → `PR review summary`, `First useful
  action` → `Recommended next test`, `Report packet index` → `Uploaded review
  artifacts`, `Assistant loop health` → `Agent proof status`. The matching
  `… at a glance` subsection headings move with each section, and fallback
  "X was not generated" messages stay aligned. Artifact filenames, JSON
  fields, command names, status IDs, workflow step `name:` values, and
  schemas are unchanged.
- `ripr --help` and every `ripr <subcommand> --help` now lead with an
  action-oriented one-liner before the `Usage:` block (e.g.,
  `ripr pilot --help` opens with "Find the top test gap in this repo and
  write a packet you can act on."). The canonical `Usage: ripr <cmd>` syntax
  and all options remain in place. Command names, subcommand names, JSON
  fields, artifact filenames, schemas, and CLI behavior are unchanged.
- VS Code command palette title for `ripr.copyContext` renamed from
  `ripr: Copy Finding Context` to `ripr: Inspect Test Gap - Copy Context` so
  it groups alongside the existing workflow-step categories ("Write Targeted
  Test - …", "Agent Handoff - …", "Verify After Test - …", "Review Result - …").
  Other command palette titles are already action-oriented and stay
  unchanged. Command IDs, settings IDs, LSP requests, JSON fields, schemas,
  status IDs, report names, artifact paths, and behavior are unchanged.
- Added [docs/RELEASE_COPY_CHECKLIST.md](docs/RELEASE_COPY_CHECKLIST.md):
  the reusable public-surface checklist captured from the v0.5.0 release.
  Covers GitHub Release body vs. process narrative, marketplace metadata,
  install truth, README badge freshness disclosure, public vocabulary,
  VSIX rebuild before publish, and dependent-channel asset verification.
  Linked from [docs/RELEASE.md](docs/RELEASE.md),
  [docs/RELEASE_MARKETPLACE.md](docs/RELEASE_MARKETPLACE.md), and the root
  README docs table. No publish workflow, schema, JSON, or behavior change.
- Added `cargo xtask check-product-copy`: a lightweight guard that scans
  the public surfaces named in the release copy checklist and flags
  unbridged use of internal vocabulary (`test oracle`, `discriminator`,
  `seam-native`, `grip`, `evidence spine`, `canonical gap`,
  `no-actionable-seam`, `front panel`, `report packet`). A file is
  bridged if it links to `docs/TERMINOLOGY.md`. Specs, output schema,
  fixtures, metrics, implementation campaigns, and the CHANGELOG are
  allowlisted internal surfaces and are not scanned. The current
  baseline is clean; the `product_copy_baseline_is_clean` unit test
  catches regressions. The guard is not wired into `cargo xtask
  check-pr` yet — promote it to a gate after a release cycle confirms
  it stays low-noise. `crates/ripr/README.md` gains the bridge link so
  the published crates.io README is also covered.
- Opened the Lane 1 Evidence Accuracy Evaluation tracker after the v0.1
  evidence spine stabilized. The tracker names PR #697 as the final consumer
  closeout, keeps `.ripr/goals/active.toml` unchanged, and routes next work
  through a repo-local evidence-quality audit before analyzer or calibration
  changes.
- Closed the Lane 2 Policy Readiness and Preview Evidence Governance tracker.
  The closeout leaves Campaign 27 active while documenting the policy-readiness
  report, preview evidence boundary, waiver-aging report, suppression-health
  report, shrink-only baseline refresh guardrails, exception-ledger alignment,
  blocking readiness guide, and advisory generated CI projection. Preview
  TypeScript/Python evidence remains visible and advisory by default, with no
  gate eligibility, RIPR Zero blocking debt, calibrated-confidence authority,
  automatic baseline adoption, generated tests, mutation execution, or default
  CI blocking without later explicit promotion policy.
- Added `cargo xtask lane1-evidence-audit` with
  `cargo xtask evidence-quality-audit` as an alias. The repo-local report writes
  `target/ripr/reports/lane1-evidence-audit.{json,md}` from generated
  repo-exposure `evidence_record` data and summarizes headline gaps, canonical
  groups, duplicate-looking groups, missing discriminators, static limitations,
  oracle semantics, related-test ranking, movement availability, calibration
  availability, field health, and top files by unresolved evidence debt without
  changing analyzer behavior or gate policy.
- Added the Lane 1 evidence-quality failure fixture corpus, pinning the first
  audit-derived `evidence_record` subsets for duplicate canonical groups,
  equality-boundary misses, activation static limitations, mock-expectation
  observer semantics, and no-runtime-data calibration gaps before analyzer
  tuning begins.
- Reduced the first audit-pinned Lane 1 canonical overcount by emitting
  parser-backed match-arm discriminators such as `"kind" =>` instead of generic
  `=>` / `match` text. The repo-local audit now splits the suppressions
  match-arm case to group size `1` and reduces duplicate-looking groups from
  `1287` to `926` without changing gates, schemas, or public command surfaces.
- Folded durable Lane 1 audit fields into `ripr evidence-health`: canonical
  gap group totals, largest groups, duplicate-looking groups, actionability
  classes, static limitation distributions, evidence-record calibration
  coverage, movement availability, and top evidence-quality risks. The report
  remains advisory and does not change analyzer classifications, gate policy,
  CI behavior, mutation execution, or score definitions.
- Added checked `runtime-fixtures-v2` calibration reports for the Lane 1
  side-effect observer, mock expectation, snapshot oracle, and opaque dispatch
  runtime classes. The fixture maps imported outcomes to existing static seams
  where possible, keeps an ambiguous opaque dispatch file-line signal
  ambiguous, and keeps a runtime-only signal from creating a static gap. No
  CI mutation execution, gate behavior, schema, or score definition changes.
- Closed the Lane 1 Evidence Accuracy Evaluation campaign in documented scope.
  The closeout records the audit, fixture corpus, first analyzer improvement,
  evidence-health dashboard fields, runtime-fixtures-v2 calibration expansion,
  and future evidence-class boundary without changing `.ripr/goals/active.toml`.

See `docs/ci/rust-1.95-quality-rollout.md` for the full PR ladder and acceptance gates.

## 0.5.0 - 2026-05-10

`ripr` 0.5.0 is the review-surface release. It moves RIPR from a collection
of static evidence reports into a coordinated advisory workflow for
developers, reviewers, CI, editors, and coding agents. The core boundary is
unchanged: RIPR does not run mutation testing, call LLM providers, generate
tests, edit source, or make default CI blocking decisions.

### Highlights

- Lane 1 evidence spine is stable in scope: a seam-native `evidence_record`
  projection with canonical gap identity threads through agent packets, repo
  exposure, gate evaluation, baseline diff, RIPR Zero status, and assistant
  proof so headline gap classes group by behavior instead of by raw line.
- Campaigns 17 through 26 turn the read-only static-evidence loop into one
  reviewer-first surface: RIPR Zero adoption, PR evidence ledger, RIPR Zero
  reporting, test-oracle assistant proof and report producer, first useful
  action, assistant-loop health, PR review front panel, report packet index,
  and the optional PR inline comment publisher.
- Editor Evidence UX matches the agent loop: saved-workspace seam
  diagnostics, hovers, and intent-titled code actions; first-useful-action
  hover and status projection; LSP `ripr.collectEvidenceContext`; and a
  status-bar / `ripr: Show Status` surface that keeps stale buffers visible.

### Evidence spine and identity

- Added the shared `RIPR-SPEC-0021` `evidence_record` projection for
  seam-native evidence, giving Lane 1 an identity, evidence path, observed
  values, missing discriminators, related tests, recommendation, calibration
  placeholder, and static limits while preserving existing repo-exposure
  fields.
- Added generated canonical gap identity so headline-eligible raw seam gaps
  group by owner, seam kind, flow sink, missing discriminator, and assertion
  shape; line numbers remain locators but no longer act as durable identity.
- Routed baseline ledgers, PR evidence ledgers, RIPR Zero status repair
  routes, agent seam packets, targeted-test outcome, agent verify movement,
  and test-oracle assistant proof through the shared evidence spine.
- Routed calibrated gate baseline comparison through canonical evidence
  identity so reviewed baseline debt matches across line movement before
  falling back to legacy seam, source, and path/line/static-class identities.
- Promoted the Lane 1 `evidence_record` capability to stable within its
  documented v0.1 scope and added a dedicated Lane 1 evidence-spine tracker.
- Stabilized related-test ranking v2 (relation confidence, reason, oracle
  strength, activation overlap, file/name/line tie-breakers), oracle
  semantics v3 (structured `oracle_semantics` explanations on related
  tests), syntax-first side-effect propagation (event, state-write,
  persistence, log, config-change, call-effect, generic-call sinks), and
  fixture-backed activation/value modeling.
- Added advisory static/runtime confidence labels to mutation calibration
  rows so runtime data can support, contradict, remain ambiguous, or stay
  unavailable for a static claim without changing static classifications.
- Added the evidence-record contract corpus pinning representative v0.1
  shapes for predicate, error, exact-value, broad-error, field,
  whole-object, snapshot, side-effect, opaque static-limitation,
  canonical-gap, and calibration-placeholder cases.

### RIPR Zero adoption

- Added `ripr baseline create --from <gate-decision.json>` writing reviewed
  gate baselines from existing gate-decision evidence with `--dry-run`,
  `--force`, and skip-on-malformed semantics.
- Added `ripr baseline diff` writing advisory baseline-debt-delta JSON and
  Markdown over still-present, resolved, new policy-eligible, acknowledged,
  suppressed, stale, invalid, and missing-input identities without making
  gate decisions or rewriting baselines.
- Added `ripr baseline update --remove-resolved`, shrink-only refresh that
  preserves malformed or ambiguous records and refuses to auto-adopt new
  current debt.
- Added baseline metadata support: owner, reason, created, review-after,
  source fields preserved across baseline create / diff / shrink-only update
  without breaking Campaign 17 baseline files.
- Added `ripr zero status`, a read-only advisory report joining baseline
  debt deltas, reviewed baseline metadata, optional gate decisions, PR
  guidance, and recommendation calibration into repo-level RIPR Zero
  progress.
- Added generated CI baseline-debt-delta artifacts and RIPR Zero summary
  wiring guarded on `RIPR_GATE_BASELINE` without changing advisory defaults.
- Added `docs/BASELINE_LEDGER_WORKFLOW.md` and
  `docs/RIPR_ZERO_REPORTING_WORKFLOW.md`, framing RIPR 0 as configured-scope
  burn-down and documenting waiver / baseline / suppression boundaries.

### Agent and reviewer workflow

- Added `ripr agent start`, `ripr agent status`, `ripr agent verify`, and
  `ripr agent receipt` LLM work-loop commands: a source-edit-free workflow
  packet, read-only loop status, before/after comparison, and a
  provenance-backed receipt with bounded next-action guidance.
- Added `ripr assistant-loop proof` and `ripr assistant-loop health`, the
  test-oracle assistant proof report and the multi-proof health summary
  with proof completeness, missing inputs, static movement, recurring
  warnings, and bounded repair queues.
- Added `ripr first-action`, a read-only advisory report producer that
  writes `first-useful-action.{json,md}` from explicit PR guidance,
  assistant proof, PR evidence ledger, baseline delta, receipt, optional
  gate, optional coverage/grip frontier, and editor context inputs.
- Added `ripr pr-ledger record`, the per-PR evidence ledger joining PR
  guidance, gate decisions, baseline debt deltas, RIPR Zero status,
  recommendation calibration, agent receipts, optional coverage, and
  optional history.
- Added `ripr review-comments`, the bounded PR test guidance JSON and
  Markdown producer; `ripr coverage-grip frontier`, an advisory report that
  keeps coverage movement and behavioral grip movement visible as separate
  axes; `ripr pr-review front-panel`, the composed reviewer front panel
  over existing front-panel inputs; and `ripr reports index`, the reviewer
  packet index over explicit artifact directories.
- Added `ripr gate evaluate`, a read-only optional policy evaluator writing
  `gate-decision.{json,md}` from existing PR guidance, labels, baselines,
  and calibration without posting comments, editing source, or running
  mutation tests.
- Added `ripr pr-comments plan`, a read-only advisory publish plan from
  explicit PR guidance and optional existing comment metadata, plus
  generated CI wiring for `RIPR_COMMENT_MODE = off|plan|inline` that posts
  or updates only safe same-repository changed-line operations from the
  plan, capped, deduped, and default off.
- Added `cargo xtask recommendation-calibration`, the advisory
  PR-recommendation usefulness report (placement, suppression correctness,
  target-file correctness, before/after static movement).
- Generated GitHub CI now surfaces PR guidance, gate decisions, baseline
  debt deltas, RIPR Zero status, assistant proof, assistant-loop health,
  first useful action, PR review front panel, and the report packet index
  only when their explicit inputs already exist; defaults remain advisory.

### Editor evidence UX

- Hardened saved-workspace seam diagnostics, evidence hovers, intent-titled
  code actions (inspect seam, write targeted test, copy agent handoff,
  verify after test, review receipt, refresh analysis), and
  `ripr.collectEvidenceContext` handoff packet.
- Added the VS Code status bar item and `ripr: Show Status` command
  covering server, workspace, analysis, stale, failed, no-actionable-seam,
  and first-useful-action states; stale Rust buffers keep stale status
  visible until save or close.
- Added first-useful-action projection in VS Code status and in the LSP
  seam hover from existing workspace-matched reports without adding
  diagnostics, editing source, generating tests, or making gate decisions.
- Hardened LSP command payload contracts and first-action status edges so
  saved-workspace command smoke and saved-workspace status output stay
  pinned across analysis-queued, analysis-running, stale-buffer,
  missing-input, and no-actionable-seam transitions.
- Added the `fixtures/editor_lsp_workflow` canonical Lane 3 fixture and
  extended VS Code e2e + framed LSP smoke coverage.
- Added `docs/EDITOR_EVIDENCE_WORKFLOW.md`, the saved-workspace editor
  guide from install and status through diagnostic, hover, related test,
  context packet, focused test, after snapshot, verify, receipt, and
  refresh with explicit static-evidence limits.

### CI, policy, and release hygiene

- Raised MSRV to Rust 1.95: workspace `rust-version`, pinned toolchain
  (`rust-toolchain.toml` -> `1.95.0`), CI MSRV job toolchain and cache keys,
  release-readiness preconditions, and doc/README references are aligned
  with the 0.5.0 / Rust 1.95 release line.
- Promoted clean Rust 1.94 / 1.95 Clippy ratchets into the active workspace
  lint table (`same_length_and_capacity`, `manual_ilog2`,
  `needless_type_cast`, `decimal_bitwise_operands`, `manual_checked_ops`,
  `manual_take`, `duration_suboptimal_units`, `unnecessary_trailing_comma`,
  plus `unsafe_op_in_unsafe_fn`, `undocumented_unsafe_blocks`,
  `multiple_unsafe_ops_per_block`, `repr_packed_without_abi`,
  `match_result_ok`); unsupported or config-dependent lints remain
  explicitly deferred with blockers in `policy/clippy-lints.toml`.
- Strengthened `cargo xtask check-no-panic-family` drift reporting (allowed
  / advisory-drift / stale / unallowed / warning sections) and added
  `--propose`, a review-only allowlist migration helper.
- Made `policy/no-panic-allowlist.toml` the canonical schema 0.3 no-panic
  allowlist with governed ids, owners, and expiry dates.
- Documented the CI verification economics policy (required, advisory,
  on-demand / release postures; LEM budget bands; label effects; artifact
  families; cheaper-signal-first rules; CI actuals; and rollback
  expectations) and added non-enforcing CI policy ledgers for LEM bands,
  target lane IDs, risk packs, artifact families, labels, and rollout
  exceptions.
- Prepared the 0.5.0 release surface: crate, VSIX, generated CI workflow
  artifacts, server archives and manifest, release-readiness flow, and the
  related-release docs.

### Release recovery (v0.5.0)

The initial `v0.5.0` tag push exposed a Windows-only bug in the new Rust
xtask release-server-archive path (PR #557 had moved the legacy
PowerShell-driven packaging into xtask, but the Windows zip branch relied
on `pwsh -Command` binding trailing positional args to `$args`, which
PowerShell only does for `-File`). The Linux and macOS targets succeeded;
the Windows target failed with a null `-Path` in `Compress-Archive`, and
the `manifest` job correctly skipped.

Recovery was fix-forward (#718): the zip branch was rewritten to use the
Rust `zip` crate (deflate-only, `default-features = false`), the Zlib
license used by the transitive `zlib-rs` dependency was added to
`deny.toml`, a `create_zip_archive_writes_flat_package_contents` test
exercises the new path on every platform including Windows, and
`release-server-binaries.yml` was rerun via `workflow_dispatch` from
`main` with `version=0.5.0`. The `v0.5.0` tag was kept at the release-prep
commit; the server archives, per-target `.sha256` files, `checksums.txt`,
and `ripr-server-manifest-v0.5.0.json` were attached to the existing
GitHub Release. The marketplace VSIX publish and crates.io publish run
separately once asset verification completes.

### Compatibility

- Raised the declared workspace MSRV and pinned repository toolchain from
  Rust 1.93 to Rust 1.95. CI MSRV jobs, release-readiness preconditions,
  README/AGENTS/CLAUDE MSRV references, and the active Clippy ratchet table
  are aligned with the 0.5.0 / Rust 1.95 release line; deferred Clippy
  promotions remain tracked in `policy/clippy-lints.toml`.

### Boundaries (unchanged)

- No LLM provider integration, no generated tests, no automatic edits, no
  runtime mutation execution, and no default CI blocking. RIPR remains a
  static, advisory evidence layer; calibrated gate, inline-comment publisher,
  and runtime calibration remain explicit opt-ins.

### Added

- Extended `cargo xtask dogfood` with checked report-packet index receipts for
  complete, sparse advisory, missing-front-panel, blocked-gate, missing-proof,
  missing-receipts, and coverage/grip-present packet cases, plus a handoff
  receipt documenting the validation boundary.
- Closed Campaign 25, Report Packet Index, with a prompt-to-artifact audit,
  validation plan, advisory boundary, and future-lane boundary in
  `docs/handoffs/2026-05-10-campaign-25-closeout.md`.
- Opened Campaign 26, PR Inline Comment Publisher, with
  `spec/pr-inline-comment-publisher-contract` as the first ready work item so
  optional durable PR comments can be planned, capped, deduped, and kept
  explicit opt-in before any GitHub posting behavior changes.
- Added `RIPR-SPEC-0025` for the PR inline comment publisher, pinning the
  read-only publish-plan schema, comment modes, permission boundary,
  summary-only exclusion, cap and dedupe behavior, and generated-CI default-off
  posture before producer or workflow changes.
- Added the PR inline comment publisher fixture corpus for publishable
  changed-line comments, summary-only exclusion, cap overflow, dedupe/upsert,
  stale-existing cleanup, fork or no-token blockers, and missing review-comments
  input before the publish-plan producer changes.
- Added read-only `ripr pr-comments plan` support, emitting advisory
  `comment-publish-plan.{json,md}` artifacts from explicit PR guidance and
  optional existing-comment metadata without posting to GitHub or changing gate
  authority.
- Added generated GitHub CI wiring for the optional PR inline comment publisher:
  `RIPR_COMMENT_MODE` defaults to `off`, `plan` mode uploads and summarizes the
  publish plan, and `inline` mode posts or updates only safe same-repository
  changed-line operations from that plan.
- Added the PR inline comment publisher workflow guide, documenting `off`,
  `plan`, and `inline` rollout, publish-plan review, fork and permission
  behavior, review-thread noise controls, dedupe/upsert, rollback, and the
  advisory gate boundary.
- Extended `cargo xtask dogfood` with checked PR inline comment publisher
  receipts for publishable, summary-only, capped, dedupe/upsert, stale-existing,
  fork or no-token, and missing-input publish plans without posting real PR
  comments.
- Closed Campaign 26, PR Inline Comment Publisher, with a prompt-to-artifact
  audit, validation plan, advisory/default-off boundary, and future-lane
  boundary in `docs/handoffs/2026-05-10-campaign-26-closeout.md`.
- Added the report-packet index fixture corpus under
  `fixtures/boundary_gap/expected/report-packet-index/`, pinning complete,
  sparse advisory, missing-front-panel, blocked-gate, missing-proof,
  missing-receipt, and coverage/grip-present packet states plus an
  `xtask check-fixture-contracts` guard before the producer changes.
- Added `fixtures/editor_lsp_workflow` as the canonical Lane 3 editor/LSP
  workflow fixture, pinning the saved-workspace diagnostic, hover, code action,
  first-useful-action status, stale-refresh guidance, and LSP cockpit surfaces
  without adding analyzer behavior or editor automation.
- Added RIPR-SPEC-0021 and the additive repo-exposure
  `seams[].evidence_record` projection, giving Lane 1 a seam-native evidence
  spine with identity, evidence path, observed values, missing discriminators,
  related tests, recommendation/actionability, calibration placeholder, and
  static limitations while preserving existing repo-exposure fields.
- Added generated canonical gap identity to evidence records so
  headline-eligible raw seam gaps group by owner, seam kind, flow sink, missing
  discriminator, and assertion shape while keeping line numbers as locators.
- Added advisory static/runtime confidence labels to mutation calibration
  JSON/Markdown rows so imported runtime data can support, contradict, remain
  ambiguous, or stay unavailable for a static gap/clean claim without changing
  static classifications, gate behavior, or mutation execution.
- Opened Campaign 23, Assistant Loop Health, with
  `spec/assistant-loop-health-report` as the first ready work item so existing
  assistant proof reports can be summarized into advisory health, missing-input,
  static-movement, warning, and repair-queue surfaces without changing analyzer,
  ranking, gate, editor, provider, mutation, generated-test, source-edit, or
  default-blocking behavior.
- Added RIPR-SPEC-0022 for the planned assistant-loop-health report, defining
  explicit proof inputs, complete/partial/missing proof states, static movement
  buckets, warning kinds, bounded repair queue entries, future multi-proof
  behavior, and advisory limits before fixtures or implementation.
- Routed zero-surface consumers through the shared evidence spine:
  agent seam packets now include additive `packets[].evidence_record`, and
  RIPR Zero status repair routes prefer supplied `evidence_record` guidance
  while preserving legacy top-level fallback fields and advisory boundaries.
- Added the assistant-loop-health fixture corpus under
  `fixtures/boundary_gap/expected/assistant-loop-health/`, pinning
  complete-improved, partial-missing-optional, missing-required-input,
  unchanged, regressed, warning-heavy, and multi-proof report states before the
  producer implementation.
- Routed targeted-test outcome and agent verify movement through the shared
  evidence spine: before/after comparison now prefers `seams[].evidence_record`
  stage, observed-value, missing-discriminator, oracle-strength, and
  related-test movement while preserving legacy repo-exposure fallback fields
  and existing advisory buckets.
- Routed test-oracle assistant proof through the shared evidence spine:
  selected seam identity, owner/location, missing discriminator, static limits,
  related test, assertion shape, verification command, and before/after classes
  now prefer supplied `evidence_record` fields while preserving legacy proof
  fallbacks and advisory boundaries.
- Added `ripr assistant-loop health`, a read-only advisory producer that writes
  `assistant-loop-health.{json,md}` from explicit proof artifacts, summarizes
  proof completeness, missing inputs, static movement, recurring warnings, and
  bounded repair queues, and leaves gate policy, analyzer behavior, provider
  calls, mutation execution, generated tests, source edits, and default CI
  blocking unchanged.
- Routed baseline and PR ledger identities through canonical gap identity:
  baseline create, diff, and shrink-only update now preserve supplied
  `canonical_gap_id` and match it before legacy selectors, while PR evidence
  ledger waiver, suppression, receipt, and top repair route records carry the
  same identity when supplied.
- Routed calibrated gate baseline comparison through canonical evidence
  identity so `ripr gate evaluate` can preserve supplied `canonical_gap_id`
  values and match reviewed baseline debt across line movement before falling
  back to legacy seam, source, and path/line/static-class identities.
- Added an evidence-record contract corpus that pins representative
  `evidence_record` v0.1 shapes for predicate, error, exact-value, broad-error,
  field, whole-object, snapshot, side-effect, opaque static-limitation,
  canonical-gap, and calibration-placeholder cases, with xtask validation for
  required cases, required fields, and schema-version drift.
- Stabilized related-test ranking v2 so capped related-test arrays preserve the
  full `related_tests_total` while ordering by relation confidence, relation
  reason, oracle strength, activation-value overlap, and stable file/name/line
  tie-breakers.
- Stabilized oracle-semantics v3 by adding structured
  `evidence_record.related_tests[].oracle_semantics` explanations that name what
  an oracle observes, what discriminator remains missing, and which assertion
  upgrade applies for broad, smoke-only, unknown, snapshot, and exact oracle
  shapes.
- Deepened local delta flow so syntax-first side-effect propagation now
  distinguishes event or outbound calls, state writes, persistence writes, log
  messages, configuration changes, and generic call-effect fallback sinks while
  preserving advisory static evidence semantics.
- Promoted activation/value modeling to stable within fixture-backed
  syntax-first scope, covering visible equality boundaries, exact error
  variants, direct literals, let bindings, same-file constants, table rows,
  rstest cases, builder or fixture overrides, enum variants, and one-level
  Option/Result constructor values while keeping unsupported value sources as
  explicit limitations.
- Promoted imported static/runtime calibration labels to calibrated for checked
  runtime-fixture classes, covering agreement, disagreement, runtime-only,
  ambiguous-join, unmatched, no-runtime-data, and seam-id/file-line join cases
  without running mutation tests or changing static classifications.
- Surfaced `assistant-loop-health.{json,md}` in generated GitHub CI when
  `test-oracle-assistant-proof.json` exists, uploads the reports with the
  normal `ripr-reports` packet, and appends a compact advisory health summary
  without changing pass/fail authority.
- Added `docs/ASSISTANT_LOOP_HEALTH_WORKFLOW.md`, explaining proof report versus
  health report, generated-CI summary use, complete/partial/missing states,
  static movement interpretation, repair routing, coding-agent handoff, and
  advisory limits for assistant-loop-health reports.
- Closed Campaign 23, Assistant Loop Health, with a prompt-to-artifact audit,
  validation plan, advisory boundary, and future-lane boundary in
  `docs/handoffs/2026-05-09-campaign-23-closeout.md`.
- Opened Campaign 24, PR Review Front Panel, with
  `spec/pr-review-front-panel-report` as the first ready work item so existing
  PR guidance, first useful action, assistant proof, assistant-loop health, PR
  ledger, baseline, gate, receipt, calibration, and coverage/grip artifacts can
  be composed into one advisory generated-CI first screen.
- Added RIPR-SPEC-0023 for the planned PR review front-panel report, including
  explicit input artifacts, bounded first-screen states, artifact groups,
  generated-CI projection limits, advisory boundaries, and the next
  fixture-first work item.
- Added the PR review front-panel fixture corpus for advisory-only,
  actionable, summary-only, acknowledged, suppressed, baseline-resolved,
  blocked, missing-proof, and coverage-flat-grip-improved cases, with an xtask
  guard to keep the producer fixture-first.
- Added `ripr pr-review front-panel`, a read-only advisory producer that writes
  `pr-review-front-panel.{json,md}` from explicit existing RIPR artifacts
  without rerunning analysis or changing gate authority.
- Updated generated GitHub CI to run `ripr pr-review front-panel` only when
  explicit input artifacts exist, upload `pr-review-front-panel.{json,md}` with
  the report packet, and append the advisory PR review front panel to the job
  summary while preserving `ripr gate evaluate` as pass/fail authority.
- Added `docs/PR_REVIEW_FRONT_PANEL_WORKFLOW.md`, documenting how reviewers,
  developers, maintainers, and coding agents read the front panel, follow
  repair routes, inspect receipts, and preserve the advisory gate boundary.
- Added dogfood validation for PR review front-panel receipts covering
  actionable, acknowledged, suppressed, baseline-resolved, blocked,
  missing-proof, no-actionable, and coverage-flat-grip-improved reviewer states
  without changing generated-CI blocking defaults.
- Closed Campaign 24, PR Review Front Panel, with a prompt-to-artifact audit,
  validation plan, advisory boundary, and future-lane boundary in
  `docs/handoffs/2026-05-10-campaign-24-closeout.md`.
- Opened Campaign 25, Report Packet Index, with
  `spec/report-packet-index-contract` as the first ready work item so the
  uploaded `ripr-reports` packet can become a reviewer-first index over
  explicit existing artifacts without changing analyzer, gate, editor,
  provider, mutation, source-edit, generated-test, inline-comment, or
  default-blocking behavior.
- Added RIPR-SPEC-0024 for the Report Packet Index contract and advanced
  Campaign 25 to the fixture-corpus slice so packet states are pinned before
  changing the index producer.
- Added `ripr reports index`, a read-only advisory producer that writes
  `target/ripr/reports/index.{json,md}` from explicit artifact directories,
  grouping reviewer-first packet surfaces while preserving gate-decision
  authority and avoiding hidden analysis reruns.
- Updated generated GitHub CI to run `ripr reports index` only when indexed
  artifacts exist, upload `index.{json,md}`, and append a compact packet-index
  section to the advisory summary without changing gate authority.
- Added `docs/REPORT_PACKET_INDEX_WORKFLOW.md`, documenting how reviewers,
  maintainers, developers, and coding agents use the grouped packet index,
  regenerate missing surfaces, and preserve the advisory gate boundary.
- Strengthened `cargo xtask check-no-panic-family` drift reporting with
  structured allowed, advisory-drift, stale, unallowed, and warning sections,
  plus exact selector-cardinality checks for ambiguous or duplicate no-panic
  allowlist entries.
- Added `cargo xtask check-no-panic-family --propose`, a review-only no-panic
  allowlist migration helper that writes Markdown/TOML selector proposals
  without rewriting policy files.
- Opened Campaign 22, First Useful Action, with
  `spec/first-useful-action-report` as the first ready work item so existing
  editor, PR guidance, ledger, proof, receipt, optional gate, coverage/grip,
  and staleness evidence can be compressed into one advisory next test action
  before adding another raw artifact surface.
- Added RIPR-SPEC-0020, defining the first-useful-action report contract,
  bounded status and action vocabularies, deterministic routing priorities,
  planned JSON/Markdown schema, traceability, and capability metrics before
  adding the producer, fixtures, CI projection, or editor projection.
- Added the Assistant Loop Health proposal, including the planned advisory
  `assistant-loop-health` JSON/Markdown surface, health buckets, repair queue,
  PR stack, and non-goals before promoting it into the active Campaign 23
  manifest.
- Added the first-useful-action routing corpus under
  `fixtures/boundary_gap/expected/first-useful-action/`, pinning actionable,
  stale, missing-required-artifact, baseline-only, acknowledged, waived,
  suppressed, no-actionable-seam, already-improved, and
  unchanged-after-attempt JSON/Markdown expectations before adding the report
  producer.
- Added `ripr first-action`, a read-only advisory report producer that writes
  `first-useful-action.{json,md}` from explicit PR guidance, assistant proof,
  PR evidence ledger, baseline delta, receipt, optional gate, optional
  coverage/grip frontier, and editor context inputs without hidden analysis,
  source edits, generated tests, provider calls, mutation execution, or default
  CI blocking.
- Generated GitHub CI now renders `ripr first-action` when explicit report
  inputs already exist, uploads `first-useful-action.{json,md}` with the normal
  report artifact packet, and appends a compact advisory first-action summary
  without changing gate authority or default blocking.
- VS Code status and `ripr: Show Status` now project an existing
  `target/ripr/reports/first-useful-action.json` report, including the selected
  action, seam location, missing discriminator, verify/receipt commands,
  warnings, fallback, and advisory limits without running hidden analysis,
  adding diagnostics, editing source, generating tests, or changing gate
  authority.
- Hardened the VS Code first-useful-action projection so reports from a
  different workspace root are ignored and stale saved-workspace evidence stays
  visible instead of being hidden behind the action report.
- Added `docs/FIRST_USEFUL_ACTION_WORKFLOW.md`, documenting how developers,
  reviewers, and coding agents read first-action reports from GitHub or the
  editor, act on the selected action, verify static movement, emit receipts,
  and interpret fallback states without treating the report as gate authority.
- Extended `cargo xtask dogfood` with checked first-useful-action receipts for
  actionable, baseline-only, stale, missing-required-artifact,
  unchanged-after-attempt, and no-actionable-seam routes while preserving
  advisory static-evidence limits and default non-blocking CI behavior.
- Closed Campaign 22 with
  `docs/handoffs/2026-05-09-campaign-22-closeout.md`, recording the
  first-useful-action prompt-to-artifact audit, validation plan, and boundary
  that future health, analyzer, policy, or editor lanes need explicit follow-up
  campaigns.
- Added `docs/ci/msrv-1.95-audit.md`, recording that `ripr` passes
  `cargo +1.95 check --workspace --all-targets`,
  `cargo +1.95 test --workspace`, and
  `cargo +1.95 clippy --workspace --all-targets -- -D warnings` before the
  follow-up MSRV bump.
- Opened Campaign 20, Test-Oracle Assistant Proof, with
  `spec/test-oracle-assistant-loop` as the first ready work item so the
  already-built PR guidance, editor/agent handoff, verification, receipts,
  ledgers, and advisory CI projection can be exercised as one end-to-end
  test-oracle assistant loop without changing analyzer, policy, editor, or CI
  defaults.
- Added RIPR-SPEC-0019, defining the end-to-end test-oracle assistant proof
  contract from changed Rust behavior through static evidence, PR/editor
  guidance, focused-test handoff, after-evidence verification, receipt, and
  advisory PR/CI projection while leaving analyzer semantics, recommendation
  ranking, gate policy, editor behavior, and default CI behavior unchanged.
- Added the canonical Campaign 20 replay corpus under
  `fixtures/boundary_gap/expected/test-oracle-assistant-loop/canonical/`,
  pinning one boundary-gap seam across PR guidance, editor/agent handoff,
  before/after static evidence, a receipt, and PR ledger projection without
  adding analyzer, policy, editor, or CI behavior.
- Added the Campaign 20 dogfood receipt, tracing the canonical boundary-gap
  seam through PR guidance, editor/agent handoff, verification commands,
  after-evidence, receipt, PR ledger projection, and coverage/grip frontier
  availability while preserving advisory static-evidence limits.
- Added `docs/TEST_ORACLE_ASSISTANT_WORKFLOW.md`, documenting the Campaign 20
  user path from PR recommendation or editor diagnostic through bounded
  handoff, one focused test, after evidence, receipt, and advisory CI/ledger
  projection without source edits, generated tests, provider calls, mutation
  execution, or default CI blocking.
- Closed Campaign 20 with `docs/handoffs/2026-05-09-campaign-20-closeout.md`,
  recording the prompt-to-artifact audit, proof commands, and follow-up
  boundaries for future proof report, PR/CI polish, analyzer, and editor work.
- Opened Campaign 21, Test-Oracle Assistant Report Producer, with
  `report/test-oracle-assistant-proof` as the first ready work item so the
  Campaign 20 proof loop can become advisory `test-oracle-assistant-proof`
  JSON/Markdown artifacts from explicit existing inputs.
- Added `ripr assistant-loop proof`, a read-only advisory report producer that
  writes `test-oracle-assistant-proof.{json,md}` from explicit PR guidance,
  agent packet, before/after evidence, receipt, PR ledger, optional gate, and
  optional coverage/grip frontier inputs without rerunning analysis, editing
  source, generating tests, calling providers, running mutation testing, or
  changing default CI blocking.
- Generated GitHub CI now surfaces `test-oracle-assistant-proof.{json,md}` as
  advisory summary and artifact content only when the required PR guidance,
  agent brief, before/after evidence, agent receipt, and PR evidence ledger
  inputs already exist.
- Added `docs/TEST_ORACLE_ASSISTANT_PROOF_REPORT.md`, a reader-facing guide for
  proof report status, warnings, static movement, optional CI projection,
  coding-agent handoff, and advisory limits.
- Closed Campaign 21 with `docs/handoffs/2026-05-09-campaign-21-closeout.md`,
  recording the proof-report producer, generated-CI projection, user docs,
  validation, next-work boundary, and advisory non-goals.
- Opened Campaign 19, PR Evidence Ledger, with
  `spec/pr-evidence-ledger-surface` as the first ready work item so per-PR
  RIPR evidence can become an adoption ledger for movement history, waiver
  aging, baseline burn-down, repair receipts, and coverage/grip frontier
  signals without changing advisory defaults.
- Added RIPR-SPEC-0018, defining the PR evidence ledger contract for per-PR
  behavioral grip movement, waiver aging, baseline burn-down, repair receipts,
  optional coverage/grip frontier signals, and advisory-only CI projection
  without changing analyzer identity, gate policy, or default blocking.
- Added `ripr pr-ledger record`, a read-only advisory JSON/Markdown report that
  joins existing PR guidance, gate decisions, baseline debt deltas, RIPR Zero
  status, recommendation calibration, agent receipts, optional coverage, and
  optional history into per-PR evidence ledger records without changing gate
  authority or CI blocking defaults.
- Added generated GitHub CI projection for PR evidence ledgers: pull-request
  runs now render and upload `pr-evidence-ledger.{json,md}` when PR guidance is
  present, append a PR movement card to the job summary, and keep gate decisions
  as the only pass/fail authority.
- Added `ripr coverage-grip frontier`, an advisory JSON/Markdown report that
  keeps coverage movement and RIPR behavioral grip movement visible as separate
  axes without treating coverage as test adequacy.
- Added `docs/PR_EVIDENCE_LEDGER_WORKFLOW.md`, explaining how teams read PR
  evidence ledgers for waiver aging, baseline burn-down, repair receipts,
  coverage/grip frontier signals, and movement toward RIPR 0 without learning
  internal report topology.
- Closed Campaign 19, PR Evidence Ledger, after the spec, producer, generated
  CI projection, coverage/grip frontier report, user workflow docs, and
  closeout receipt landed while generated CI stayed advisory by default.
- Opened Campaign 18, RIPR Zero Reporting, with
  `spec/ripr-zero-reporting-surface` as the first ready work item so reviewed
  baselines and debt deltas can become repo-level RIPR 0 status, stale-debt,
  trend, and top-repair-area reporting without changing advisory defaults.
- Added RIPR-SPEC-0017, defining the RIPR Zero status report contract for
  repo-level status, baseline metadata health, stale warnings, trend summaries,
  top debt areas, and advisory repair routing without changing analyzer
  identity, gate policy, or default CI blocking.
- Added additive baseline review metadata support: new baseline ledgers record
  owner/reason/created/review-after/source fields, baseline delta reports
  preserve that metadata on baseline-derived items, and shrink-only updates keep
  existing metadata while remaining compatible with Campaign 17 baseline files.
- Added `ripr zero status`, a read-only advisory JSON/Markdown report that
  joins baseline debt deltas, reviewed baseline metadata, optional gate
  decisions, PR guidance, and recommendation calibration into repo-level RIPR
  Zero progress without changing gate authority or CI blocking defaults.
- Added generated-CI RIPR Zero summary wiring: when baseline debt delta exists,
  the workflow writes/uploads `ripr-zero-status.{json,md}` and appends a
  first-screen RIPR Zero summary without changing advisory defaults or gate
  pass/fail authority.
- Added `docs/RIPR_ZERO_REPORTING_WORKFLOW.md`, a user workflow for reading
  RIPR Zero status, aging and refreshing reviewed baselines, routing repair
  packets, and interpreting movement without treating RIPR 0 as perfect tests
  or 100 percent coverage.
- Closed Campaign 18, RIPR Zero Reporting, after the reporting spec, baseline
  metadata preservation, status report, generated-CI summary, and user workflow
  docs made RIPR 0 progress visible without changing advisory defaults.
- Added `ripr baseline create --from <gate-decision.json> --out .ripr/gate-baseline.json`,
  which writes reviewed gate baseline ledgers from existing gate-decision
  evidence, skips suppressed or malformed decisions, supports `--dry-run`, and
  refuses to overwrite without `--force`.
- Added LSP `ripr.collectEvidenceContext`, a saved-workspace seam handoff
  packet with seam identity, evidence path, missing discriminator, related
  test, suggested test, shared agent-loop commands, and static limits for
  editor or external-agent use.
- Added `ripr baseline diff --baseline <gate-baseline.json> --current <gate-decision.json>`,
  which writes advisory baseline-debt-delta JSON/Markdown showing
  still-present, resolved, new policy-eligible, acknowledged, suppressed,
  stale, invalid, and missing-input identities without making gate decisions or
  rewriting baselines.
- Added `ripr baseline update --remove-resolved`, which shrink-only refreshes a
  reviewed gate baseline ledger by removing resolved entries while preserving
  malformed or ambiguous records for review and refusing to auto-adopt new
  current debt.
- Added generated CI baseline debt delta artifacts and summary output: when a
  repository sets `RIPR_GATE_BASELINE` and gate evaluation writes
  `gate-decision.json`, the workflow runs `ripr baseline diff`, uploads
  `baseline-debt-delta.{json,md}`, and summarizes debt movement without making
  the delta report the pass/fail authority.
- Added `docs/BASELINE_LEDGER_WORKFLOW.md`, a command-by-command adoption guide
  for reviewed baseline creation, baseline debt deltas, `baseline-check`,
  shrink-only refresh, new debt review, waiver versus baseline versus
  suppression boundaries, and the path toward RIPR 0.
- Closed Campaign 17, RIPR Zero Adoption, after the baseline debt delta spec,
  baseline create/diff/update commands, generated CI delta artifacts, and
  baseline ledger workflow docs made historical behavioral test debt governable
  without changing advisory defaults.
- Extended framed LSP protocol smoke coverage through a real seam diagnostic,
  hover, code actions, `ripr.collectEvidenceContext`, and shutdown.
- Extended VS Code e2e smoke coverage so the real boundary-gap server path
  reaches a seam diagnostic, hover, seam actions, copied packet and verify
  payloads, and related-test opening.
- Added `ripr evidence-health` and `cargo xtask evidence-health`, which write
  advisory Lane 1 analyzer-health JSON/Markdown reports summarizing grip
  classes, stage states, missing discriminators, observed value contexts,
  related-test confidence, oracle evidence, top static limitations, and
  optional imported calibration availability without changing analyzer
  behavior.
- Added a VS Code status bar item and `ripr: Show Status` command for first-run
  server resolution, workspace detection, saved-workspace analysis
  disabled, queued/running/complete/stale/failed, server-unavailable, and
  no-actionable-seam states. Dirty Rust buffers now keep stale status visible
  until save or close so saved-workspace evidence is not presented as current
  for unsaved text.
- Added `docs/EDITOR_EVIDENCE_WORKFLOW.md`, a user-facing saved-workspace editor
  guide from install and status through diagnostic, hover, related test, context
  packet, one focused test, after snapshot, verify, receipt, and refresh with
  explicit static-evidence limits.
- Closed Editor Evidence UX with a prompt-to-artifact audit covering seam
  diagnostics, evidence hover, related-test actions, context packets, VS Code
  smoke, status/staleness, workflow docs, and the no-source-edit/no-runtime
  boundary.
- Added `ripr agent start --root . --seam-id <id> --out target/ripr/workflow`
  to write a source-edit-free workflow packet with `workflow.json`,
  `commands.md`, and `agent-brief.json` for one selected seam. The packet
  names artifact paths, shared commands, missing inputs, and explicit no-edit,
  no-generated-test, no-LLM-call boundaries.
- Added `ripr agent status --root . --json`, a read-only LLM work-loop status
  report that checks existing agent artifacts, recovers a seam id when
  possible, emits missing-step commands, and warns on stale-looking verify or
  receipt artifacts without rerunning analysis.
- Added `cargo xtask check-ci-lane-whitelist`, a structural advisory checker
  for the CI lane, risk-pack, budget, artifact-family, and rollout-exception
  ledgers.
- Added `ripr agent start --root . --seam-id <id> --out target/ripr/workflow`,
  which writes source-edit-free `agent-workflow.json` and `agent-workflow.md`
  checklists from the shared LLM work-loop command templates.
- Queued Campaign 12, First-Hour UX, as the post-LLM-work-loop lane for making
  the VS Code extension and generated CI workflow useful from their first
  screens without requiring users to learn RIPR's internal report topology.
- Opened Campaign 12, First-Hour UX, with `spec/pr-test-guidance-annotations`
  as the first ready contract item before editor or CI behavior changes.
- Added RIPR-SPEC-0012 for advisory PR test guidance annotations, defining the
  `ripr review-comments` JSON contract, changed-line placement rules,
  check-annotation default, opt-in inline review comments, and bounded LLM
  handoff guidance.
- Added `ripr review-comments --root . --base <sha> --head <sha> --out target/ripr/review/comments.json`
  to write bounded advisory PR guidance JSON plus Markdown without posting to
  GitHub, generating tests, editing source, running mutation testing, or making
  CI blocking.
- Added generated CI execution of `ripr review-comments` on pull requests so
  `target/ripr/review/comments.json` is written before the existing advisory
  summary and non-blocking check-annotation consumers run.
- Added PR guidance fixture outputs under
  `fixtures/boundary_gap/expected/pr-guidance` for exact-line,
  owner-function-line, same-file-line, summary-only, capped, configured-off,
  and changed-test-skip cases.
- Added `docs/PR_REVIEW_GUIDANCE.md` to document `ripr review-comments`,
  generated CI check annotations, summary-only fallback, pinned fixture cases,
  and the inline-comment opt-in boundary.
- Added the Campaign 13 closeout handoff after PR guidance renderer, generated
  CI consumption, placement fixtures, and user-facing docs aligned.
- Opened Campaign 14, Recommendation Calibration, with
  `spec/recommendation-calibration-report` as the first ready item so
  recommendation quality is measured before ranking or policy work.
- Queued Campaign 15, Calibrated Gate Policy, as a later optional-policy lane
  after recommendation calibration, preserving advisory defaults and keeping
  static evidence separate from runtime mutation vocabulary.
- Added RIPR-SPEC-0013 for recommendation calibration reports, defining the
  planned input artifacts, JSON/Markdown shape, usefulness metrics, false
  annotation tracking, summary-only correctness, suppression correctness,
  target-file correctness, latency fields, advisory posture, and non-goals.
- Added pinned review guidance outcome receipt examples for useful, noisy,
  wrong-line, already-covered, wrong-target, summary-only-correct, and
  suppressed-correctly recommendation calibration feedback.
- Added `cargo xtask recommendation-calibration`, which reads existing PR
  guidance, calibration expectations, optional outcome receipts, targeted-test
  outcome, and agent receipt artifacts, then writes advisory
  `recommendation-calibration.{json,md}` without telemetry, source edits,
  generated tests, runtime execution, or CI blocking.
- Added checked recommendation calibration report outputs under
  `fixtures/boundary_gap/expected/recommendation-calibration/`.
- Added `docs/RECOMMENDATION_CALIBRATION.md` to document how to run and read
  recommendation calibration reports, local outcome receipts, placement
  quality, suppression correctness, static movement buckets, and advisory
  limits.
- Added the Campaign 14 closeout handoff after recommendation calibration was
  specified, fixture-pinned, receipt-backed, reported, documented, and kept
  advisory-first for later ranking or policy work.
- Added RIPR-SPEC-0014 for calibrated gate policy, defining optional
  visible-only, acknowledgeable, baseline-check, and calibrated-gate modes plus
  the planned gate decision JSON/Markdown contract.
- Added `ripr gate evaluate`, a read-only optional policy evaluator that writes
  `gate-decision.{json,md}` from existing PR guidance, labels, baselines, and
  calibration inputs without posting comments, editing source, running mutation
  tests, uploading SARIF, or changing generated workflow defaults.
- Added `docs/CALIBRATED_GATE_POLICY.md` to document optional gate modes,
  waiver labels, generated CI behavior, calibration evidence, rollout stages,
  fixture cases, and static/runtime vocabulary boundaries.
- Added the Campaign 15 closeout handoff after optional calibrated gates were
  specified, implemented as read-only evaluation, fixture-pinned, optionally
  wired into generated CI, documented, and kept advisory by default.
- Opened Campaign 16, Gate Adoption UX, with `docs/gate-adoption-examples` as
  the first ready item before waiver workflows, baseline guidance, CI summary
  polish, dogfood receipts, and blocking-readiness docs.
- Added copyable generated-CI gate adoption examples for default advisory
  posture, `visible-only`, `acknowledgeable`, `baseline-check`, and
  `calibrated-gate` repository-variable settings.
- Queued Editor Evidence UX as a separate Lane 3 campaign proposal after Gate
  Adoption UX, preserving `gate-adoption-ux` as the active manifest while
  documenting the saved-workspace LSP loop from diagnostic to hover, related
  test, context packet, verify, and receipt.
- Added gate waiver workflow docs for `ripr-waive`, including label setup,
  acknowledgeable-mode review steps, audit artifacts, and the boundary between
  PR-time waivers, durable suppressions, and baselines.
- Added gate baseline workflow docs for creating, reviewing, and refreshing
  `.ripr/gate-baseline.json` as a visible historical-debt ledger rather than a
  suppression file, with RIPR 0 framed as a configured-scope burn-down target
  and `baseline-check` behavior documented for reviewed historical debt.
- Polished the generated CI gate summary so reviewers can see mode, status,
  decision counts, active and acknowledgement labels, applied waiver, baseline
  input, calibration inputs/effects, blocking reason, and gate artifact paths
  before opening JSON.
- Added checked repo-local gate adoption dogfood receipts to
  `cargo xtask dogfood`, covering `visible-only`, acknowledged waiver,
  baseline-existing, baseline-new, repair-oriented missing-baseline, and
  explicit calibrated-gate decisions from checked evidence while preserving
  non-blocking generated CI defaults.
- Added `docs/BLOCKING_READINESS.md` to explain when teams should stay
  advisory, require acknowledgement, use `baseline-check`, or enable
  `calibrated-gate` after local evidence is mature.
- Added the Campaign 16 closeout handoff after gate adoption examples, waiver
  workflows, baseline guidance, generated-CI gate summary polish, dogfood
  receipts, and blocking-readiness guidance were complete while generated CI
  stayed advisory by default.
- Opened Campaign 17, RIPR Zero Adoption, with
  `spec/baseline-debt-delta-report` as the first ready item before baseline
  create, diff, shrink-only update, and generated CI debt-delta artifacts.
- Added `docs/EDITOR_EVIDENCE_UX.md` and the Editor Evidence UX audit handoff
  to define the queued saved-workspace editor contract before behavior changes.
- Hardened seam evidence hover so saved-workspace seam diagnostics now show
  related test locations, suggested test shape, packet and brief handoff
  commands, verify and receipt commands, and static-evidence limits from the
  same classified seam state.
- Tightened seam code-action visibility so the focused test brief action is
  offered only when a related test or suggested assertion context exists, while
  packet, agent handoff, verify, receipt, and refresh commands remain
  available for stable seam diagnostics.
- Added RIPR-SPEC-0016 for the baseline debt delta report, defining the planned
  JSON/Markdown contract, identity matching order, debt movement buckets,
  advisory boundary, and future `ripr baseline create`, `diff`, and
  shrink-only `update --remove-resolved` command surfaces.
- Added a generated GitHub workflow advisory summary that combines the pilot
  recommendation, agent review packet, artifact paths, SARIF and badge status,
  known limits, and PR guidance annotation counts before artifact
  download.
- Added a generated workflow smoke fixture test that pins artifact paths,
  top-seam extraction, agent artifact generation, non-blocking posture,
  optional SARIF gates, badge output, advisory summary sections, and PR
  guidance annotation hooks.
- Added an LLM work-loop fixture matrix that pins happy, unchanged, regressed,
  missing-artifact, stale-artifact, configured-off, path-with-spaces, and
  Windows-separator review states.
- Added generated CI LLM work-loop packets: `ripr init --ci github` now uploads
  workflow manifest, commands Markdown, agent status JSON/Markdown, review
  summary JSON/Markdown, receipt, and operator cockpit artifacts as advisory
  evidence.
- Added `docs/LLM_OPERATOR_GUIDE.md`, a source-edit-free guide for humans and
  external LLM tools using RIPR status, workflow packet, verify, receipt, and
  reviewer-summary artifacts.
- Closed Campaign 11 after status, command templates, workflow manifests,
  provenance-backed receipts, bounded next-action guidance, reviewer summaries,
  fixtures, generated CI packets, and the LLM operator guide aligned around a
  source-edit-free static work loop.
- Extended the LSP seam evidence hover to project first-useful-action when an
  existing workspace-matched report selects the same seam, so the editor hover
  carries the same advisory next-action, target test, verify command, and
  receipt command surfaces as the status bar without rerunning analysis.

### Changed

- Promoted the Lane 1 `evidence_record` capability to stable within its
  documented v0.1 scope and added a dedicated Lane 1 evidence-spine tracker so
  future evidence work stays separate from active PR/CI, editor, policy, and
  release campaigns.
- Promoted local delta flow to stable within its fixture-backed syntax-first
  scope for visible return, error, field, match, and side-effect sink families
  while keeping unsupported propagation as explicit static limitations.
- Moved the declared workspace MSRV, pinned toolchain, `clippy.toml`, and
  `policy/clippy-lints.toml` MSRV ledger to Rust 1.95 after the compatibility
  audit passed; planned Clippy lint promotion remains deferred to the next
  rollout PR.
- Promoted the clean Rust 1.94/1.95 planned Clippy lints into the active
  workspace lint policy and retained unsupported or config-dependent lints with
  explicit blockers.
- Made `policy/no-panic-allowlist.toml` the canonical schema 0.3 no-panic
  allowlist, with governed ids, owners, expiry dates, and checker support while
  leaving `.ripr/no-panic-allowlist.toml` as a legacy compatibility mirror.
- Advanced Campaign 13, PR Review Guidance, after adding the read-only
  `ripr review-comments` producer and generated CI producer step;
  placement/suppression fixtures and PR guidance docs completed the lane before
  closeout.
- Closed Campaign 13 after PR guidance became produced, consumed by generated
  CI, fixture-pinned, documented, and still advisory/non-blocking by default.
- Advanced the active product lane to Campaign 14 so recommendation
  usefulness, placement, suppression/noise behavior, and before/after static
  movement can be calibrated before optional gates.
- Advanced Campaign 14 to `fixtures/pr-guidance-calibration-corpus` and
  `review-feedback/outcome-receipts` after pinning the recommendation
  calibration report contract.
- Advanced Campaign 14 to `report/recommendation-precision` after pinning
  outcome receipt fixtures for local recommendation feedback.
- Advanced Campaign 14 to `docs/calibration-workflow` after adding the
  advisory recommendation precision report command and checked outputs.
- Advanced Campaign 14 to `campaign/recommendation-calibration-closeout` after
  documenting the recommendation calibration workflow.
- Opened Campaign 15, Calibrated Gate Policy, with
  `spec/calibrated-gate-policy` as the next ready contract item.
- Advanced Campaign 15 to `gate/policy-evaluator` after pinning the calibrated
  gate policy contract.
- Advanced Campaign 15 to `fixtures/calibrated-gate-cases` after adding the
  read-only gate decision producer.
- Advanced Campaign 15 to `ci/generated-gate-wiring` after pinning calibrated
  gate decision fixtures for advisory, acknowledged, baseline-check,
  high-confidence blocking, suppression, missing-input, and calibration
  disagreement cases.
- Wired generated GitHub workflows to run `ripr gate evaluate` only when
  `RIPR_GATE_MODE` is explicitly configured, upload gate-decision artifacts,
  and keep default generated workflows advisory.
- Advanced Campaign 15 to `docs/calibrated-gate-policy` after optional
  generated CI gate wiring landed without changing default workflow blocking.
- Advanced Campaign 15 to `campaign/calibrated-gate-closeout` after documenting
  calibrated gates as optional policy over existing static evidence.
- Closed Campaign 15 after the optional calibrated gate layer was specified,
  implemented as a read-only evaluator, fixture-pinned, wired into generated CI
  only behind explicit configuration, documented, and kept advisory by default.
- Clarified agent merge ownership and replaced the old campaign-field guard
  with stale merge-boundary language detection.
- Pinned RIPR-SPEC-0012 as the PR test guidance annotation contract and
  advanced Campaign 12 to `vscode/first-run-status` as the next ready UX item.
- Advanced Campaign 12 to `vscode/action-discoverability` after pinning the
  extension first-run status surface.
- Grouped seam diagnostic code-action and VS Code command titles around user
  intent: inspect the seam, write the targeted test, copy agent handoff
  commands, verify after the test, review the receipt, and refresh analysis.
  Command IDs and payloads remain stable.
- Advanced Campaign 12 to `ci/pr-summary-surface` after pinning editor action
  discoverability.
- Advanced Campaign 12 to `ci/generated-workflow-smoke-fixture` after wiring
  the generated workflow advisory summary and PR guidance annotation hook.
- Advanced Campaign 12 to `docs/ux-by-user-type` after pinning the generated
  workflow smoke fixture.
- Reworked README and Quickstart first-hour docs around VS Code, CI, CLI,
  agent/reviewer, troubleshooting, and known-limit paths, and advanced Campaign
  12 to closeout.
- Closed Campaign 12 after the editor first-run status path, intent-titled
  actions, generated CI advisory summary, workflow smoke fixture, and
  user-type first-hour docs aligned the VS Code, CI, CLI, and agent/reviewer
  entry paths.
- Aligned public package and extension front-door metadata around Rust
  test-oracle gaps, targeted tests, and static RIPR evidence instead of
  internal mutation-exposure wording.
- Centralized LLM work-loop command templates for agent status next commands,
  agent brief follow-up commands, pilot follow-up commands, LSP copy-action
  payloads, generated CI artifact paths, and operator cockpit missing-input
  commands without changing the emitted command text.
- `ripr agent status --root .` now prints Markdown by default; `--json` keeps
  the machine-readable Agent Status schema.
- Documented the CI verification economics policy: required, advisory, and
  on-demand/release postures; LEM budget bands; label effects; artifact
  families; cheaper-signal-first rules; CI actuals; and rollback expectations.
  The PR template now asks CI-affecting PRs to record cost, affected workflows,
  branch-protection impact, cheaper signals considered, artifact families, and
  rollback path.
- Documented the completed `0.4.0` post-publish verification for crates.io,
  GitHub Release server assets, VS Marketplace, Open VSX, and installed
  editor-agent loop smoke checks.
- Added non-enforcing CI policy ledgers for LEM budget bands, target lane IDs,
  risk packs, artifact families, labels, and rollout exceptions. These seed
  files document the future PR planning surface without changing workflow
  behavior.
- Tightened LSP command payload contracts and first-useful-action status edges
  so saved-workspace command smoke and saved-workspace status output stay
  pinned across analysis-queued, analysis-running, stale-buffer, missing-input,
  and no-actionable-seam transitions.
- Hardened the VS Code saved-workspace status output and command smoke
  fixtures to keep the status bar item, `ripr: Show Status` text, and
  intent-titled action payloads stable across the report-projection,
  stale-buffer, and disabled-by-setting paths without changing user-visible
  behavior.

### Release prep

- Bumped crate, extension, lockfile, agent-receipt fixture, doc, and xtask
  release-readiness test-fixture references from 0.4.0 to 0.5.0; aligned the
  CI MSRV job toolchain pin and cache keys to Rust 1.95.0; refreshed the
  workspace and crate README MSRV badges, Distribution capability rows, and
  Rust-requirement statements; advanced version refs in `docs/RELEASE.md`,
  `docs/RELEASE_BINARIES.md`, `docs/RELEASE_MARKETPLACE.md`,
  `docs/SERVER_PROVISIONING.md`, `docs/EDITOR_EXTENSION.md`, `docs/OPENVSX.md`,
  `docs/specs/RIPR-SPEC-0011-llm-work-loop.md`, `docs/OUTPUT_SCHEMA.md`, and
  the VS Code extension changelog and resolver fallback. The 0.4.0 release
  receipts and historical rollout records are preserved unchanged.

## 0.4.0 - 2026-05-07

This release aligns RIPR's editor and CI evidence loop: saved-workspace
diagnostics, hover evidence, targeted briefs, agent command handoff, focused
test verification, receipts, cockpit artifacts, and non-blocking CI output now
tell the same conservative static-exposure story.

### Added

- Added `ripr pilot`, a zero-config first-run command that writes
  `target/ripr/pilot/repo-exposure.{json,md}`,
  `target/ripr/pilot/agent-seam-packets.json`, and
  `target/ripr/pilot/pilot-summary.{json,md}` while printing the top
  actionable seam and after-test commands.
- Added `ripr outcome` to compare before/after `repo-exposure-json` snapshots
  from the installed binary, printing an advisory targeted-test receipt by
  default with `--format json` and `--out` for tool/file output.
- Added `ripr calibrate cargo-mutants` to import already-produced
  cargo-mutants JSON from the installed binary, join it to a
  `repo-exposure-json` snapshot, and render advisory Markdown/JSON calibration
  output without running mutation testing.
- Added `ripr init --ci github` to generate a non-blocking GitHub Actions
  workflow that runs `ripr pilot`, uploads pilot/report artifacts, writes repo
  badge JSON, and keeps SARIF rendering/upload optional through
  `RIPR_UPLOAD_SARIF`.
- Added `ripr init` as an optional command that materializes built-in defaults
  into a repo-local `ripr.toml` so teams can commit, review, version, and tune
  policy; it does not unlock basic usefulness, and missing `ripr.toml` remains
  the normal first-run state. Includes `--dry-run` for previewing and `--force`
  for explicit overwrite.
- Added RIPR-SPEC-0009 to define defaults-first adoption behavior for `init`,
  `pilot`, `outcome`, calibration import, editor, SARIF, badge, and config
  work.
- Added focused defaults-first guardrails that pin the generated `ripr.toml`
  against `ripr.toml.example` and test default repo discovery exclusions for
  generated, policy-only, fixture-only, and package-manager directories.
- Added `cargo xtask operator-cockpit-report`, which writes
  `target/ripr/reports/operator-cockpit.{json,md}` by joining repo exposure,
  LSP cockpit, SARIF policy, badge status, targeted-test outcome, and optional
  mutation calibration artifacts into one next-action report.
- Added `docs/INSTALLATION_VERIFICATION.md` to pin the defaults-first release
  proof for local package install, public `cargo install`, GitHub Release server
  archives, VSIX packaging, and known limits.
- Added the initial JSON-only `ripr agent brief` command, which ranks
  working-set seams from existing repo exposure evidence and points agents to
  packet references, candidate discriminators, assertion shape, and static
  before/after verification commands.
- Added `ripr agent verify` and `ripr agent receipt` so agent workflows can
  compare before/after repo exposure snapshots and emit a focused review
  receipt for one seam.
- Added saved-workspace LSP/VS Code code actions that copy the agent loop
  command chain for a seam diagnostic: agent packet, agent brief, after
  snapshot, agent verify, and agent receipt.
- Added `cargo xtask release-readiness --version <version>`, which writes
  `target/ripr/reports/release-readiness.{json,md}` and checks the 0.4 CLI,
  agent verify/receipt, LSP cockpit, advisory CI, latency, install, VSIX, and
  known-limit surfaces from repo artifacts.
- Added operator cockpit status for the editor-agent loop artifacts:
  before/after snapshots, `agent verify`, `agent receipt`, movement counts,
  and missing-input commands aligned with the editor copy-command chain.
- Added a canonical boundary-gap editor-agent loop fixture that pins agent
  packet, agent brief, agent verify, agent receipt, and operator cockpit output
  against the LSP diagnostic/action seam identity.
- Expanded the generated `ripr init --ci github` workflow to upload the
  editor-agent loop artifacts: pilot output, agent packet, agent brief, agent
  verify, agent receipt, targeted-test outcome, optional operator cockpit,
  SARIF, and badge JSON.

### Changed

- Centered the first-hour installed-user docs on the full editor-agent evidence
  loop: `ripr pilot`, targeted brief, focused test, after snapshot,
  `ripr outcome`, `ripr agent verify`, `ripr agent receipt`, editor actions,
  generated CI artifacts, and the explicit `ripr init` policy-materialization
  boundary.
- Documented and test-pinned the LSP agent-loop copy-command payload contract:
  commands stay workspace-relative, preserve seam metadata, and fail closed for
  stale seam diagnostics.
- Restored Campaign 10 to `editor-agent-integration` after the brief
  release-surface pivot, carrying release readiness as a later gate and moving
  the lane from LSP command copy actions to operator cockpit verify/receipt
  status.
- Closed Campaign 10 after aligning editor, agent, cockpit, CI, fixture, docs,
  and release-readiness surfaces without adding analyzer families, runtime
  mutation execution, CI blocking, public crate splits, automatic edits, or
  speculative editor features.
- Routed `ripr agent brief` file and diff working sets through existing
  related-test evidence so edits to known related tests rank their seams before
  repo fallback, and added the related-test-confidence tie-breaker from
  RIPR-SPEC-0010.
- Added an advisory `ripr agent brief` warning when visible seams are omitted
  by the default or requested brief cap.
- Routed `ripr agent brief --diff` and `--base` changed lines through existing
  owner-function facts so same-owner seams can rank as
  `changed_owner_function` before broad file fallback.
- Normalized agent seam packet file paths to use stable `/` separators in
  checked JSON, including related-test and recommended-test paths on Windows.
- Made `ripr pilot` budget-aware with a default 30 second analysis timeout,
  `--timeout-ms` for explicit runs, and a versioned `pilot-summary.json` schema
  update that records complete versus partial timeout status.
- Prepared the `0.3.1` release line as the first defaults-first public install
  target. `0.3.0` remains published but predates `ripr pilot` and
  `ripr outcome`.
- Documented and test-pinned the defaults-first config profile, including
  missing-config/generated-config equivalence, repo-mode production exclusions,
  badge/report defaults, and fast/normal/deep operator mode vocabulary; Campaign
  7 now moves from `defaults/config-init` to the operator cockpit and editor
  install polish work items.
- Aligned the VS Code extension's default `ripr.check.mode` with the
  defaults-first posture by switching it to `draft` and exposing the full LSP
  mode enum.
- Ignored generated `.vscode-test` editor-host artifacts in repo file scans so
  local extension smoke runs do not pollute Rust policy gates.
- Split `xtask` command parsing, help/catalog, and execution dispatch into
  focused modules while preserving existing `cargo xtask` command behavior.
- Routed xtask policy checks through focused `xtask/src/policy/` checker modules
  while preserving existing `cargo xtask check-*` policy command behavior.
- Routed xtask report commands through focused `xtask/src/reports/` modules
  while preserving existing report command behavior.
- Closed Campaign 6 after the internal module SRP refactor chain landed through
  #405, confirmed stale forks #250, #253, and #352 are closed unmerged, and
  moved the active manifest to Campaign 7 defaults-first operator adoption.
- Completed the Campaign 7 `defaults/config-init` baseline and advanced the
  active manifest to `reports/operator-cockpit` as the next ready work item.
- Completed the Campaign 7 `reports/operator-cockpit` surface and advanced the
  active manifest to `ci/github-action-entrypoint` as the next ready work item.
- Completed the Campaign 7 `ci/github-action-entrypoint` surface and advanced
  the active manifest to `editor/install-polish` as the next ready work item.
- Completed the Campaign 7 `editor/install-polish` surface and advanced the
  active manifest to `fixtures/example-corpus` as the next ready work item.
- Aligned built-in defaults with the `ripr init` profile for LSP seam
  diagnostics: missing config now uses the same bounded saved-workspace default
  as the generated policy file, while explicit LSP options or `ripr.toml` can
  still disable seam diagnostics.
- Tightened RIPR-SPEC-0009 so missing `ripr.toml` means useful built-in
  defaults, while `ripr init` records repo policy instead of unlocking basic
  CLI or editor usefulness.
- Added a boundary-gap runtime calibration sample so the targeted-test case
  study can demonstrate a static-gap/runtime-clean join without running
  mutation testing.
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
- Added SARIF output formats for Campaign 5B. `ripr check --format sarif`
  renders diff-scoped Finding SARIF and `--format repo-sarif` renders
  repo-scoped seam SARIF with configured severity, visible suppression metadata,
  stable rule IDs, and stable fingerprints.
- Added `cargo xtask sarif-policy` for opt-in SARIF baseline checks. The
  command compares current SARIF to a baseline using stable rule IDs and
  fingerprints, ignores suppressed results, writes
  `target/ripr/reports/sarif-policy.{json,md}`, and only exits non-zero for
  new warning-level results when `--mode fail-on-new-warning` is requested.
- Remapped public repo badges onto seam-native counts for Campaign 5B.
  Repo-scoped `ripr` and `ripr+` badges now count configured-visible
  headline-eligible `SeamGripClass` values, while diff-scoped badge artifacts
  remain legacy finding-exposure summaries for PRs. Native badge JSON is now
  schema `0.3` with `basis` and `counts.analyzed_seams`; the checked-in
  Shields endpoint artifacts in `badges/` were refreshed together.
- Closed Campaign 5B (Operationalization) after repository config, SARIF/CI
  policy, and seam-native badge count mapping landed (#331, #333, #336, #338,
  #342). The active manifest now moves to Campaign 6 with a draft-stack audit
  before structural refactors resume.
- Audited the Campaign 6 modularization draft stack against current `main` and
  recorded the canonical rebase path before structural refactors resume. The
  first ready item is the #244 summary/sort extraction; #249 stays in the
  sequence after the workspace split, while #250 is parked for close or rewrite
  after the facts/syntax/build-index path stabilizes.
- Started the Campaign 6 refactor stack by extracting summary/sort helpers,
  pipeline orchestration, diff load/model/parse modules, workspace
  classify/discover/select modules, and probe classify/config/diff/repo modules
  without output, schema, or public API drift.
- Moved neutral Rust analysis fact DTOs into `analysis/facts/model.rs` for the
  Campaign 6 facts model extraction while leaving syntax adapters, builders,
  extraction, and query logic in place. The next ready seam is syntax adapter
  type extraction.
- Moved syntax adapter traits and shared syntax facts into
  `analysis/syntax/adapter.rs` while keeping builders, parser-backed extraction,
  lexical fallback, and query logic in `analysis/rust_index.rs`. The next ready
  seam is build-index extraction.
- Moved Rust index construction into `analysis/facts/build.rs` while keeping
  parser-backed extraction, lexical fallback, and query helpers in
  `analysis/rust_index.rs`. The next ready seam is parser-backed RA syntax
  extraction.
- Moved parser-backed RA syntax adapter implementation into
  `analysis/syntax/ra.rs` while keeping lexical fallback and Rust index query
  helpers behavior-stable. The next ready seam is lexical syntax fallback
  extraction.
- Moved the lexical syntax fallback implementation into
  `analysis/syntax/lexical.rs` while keeping `analysis/rust_index.rs` as the
  compatibility facade for query and extractor helpers. The next ready seam is
  fact extraction helper modularization.
- Moved call, return, literal, oracle, and text extraction helpers plus
  probe-shape constants into `analysis/extract/*`, with `analysis/rust_index.rs`
  still re-exporting the compatibility helper surface. The next ready seam is
  probe family metadata extraction.
- Moved probe-family mapping, changed-line family heuristics, and delta metadata
  into `analysis/probes/family.rs` while preserving probe generation behavior.
  The next ready seam is probe expectation helper extraction.
- Moved probe expected-sink and required-oracle helpers into
  `analysis/probes/expectations.rs` while preserving probe generation behavior.
  The next ready seam is probe ID helper extraction.
- Moved probe ID construction and path sanitization helpers into
  `analysis/probes/ids.rs` while preserving diff and repo probe ID formats.
  The next ready seam is lexical probe fallback extraction.
- Moved lexical changed-line probe fallback helpers into
  `analysis/probes/lexical.rs` while preserving probe generation behavior.
  The next ready seam is diff/repo probe seeding split.
- Reconciled the Campaign 6 probe seeding manifest after confirming diff and
  repo probe seeding already lives in `analysis/probes/diff.rs` and
  `analysis/probes/repo.rs`. The next ready seam is classification context
  extraction.
- Added a private `analysis/classify/context.rs` `ProbeContext` carrier for
  the classifier's probe, owner, and related-test inputs, setting up later
  RIPR stage module extraction without changing classification behavior. The
  next ready seam is related-test discovery extraction.
- Moved related-test discovery into `analysis/classify/related_tests.rs` while
  preserving classification behavior. The next ready seam is reach evidence
  extraction.
- Moved reach evidence into `analysis/classify/reach.rs` while preserving
  classification behavior. The next ready seam is flow and propagation
  extraction.
- Moved local flow and propagation evidence into `analysis/classify/flow.rs`
  while preserving classification behavior. The next ready seam is activation
  evidence extraction.
- Moved activation evidence, observed-value extraction, and missing
  discriminator helpers into `analysis/classify/activation.rs` while preserving
  classification behavior. The next ready seam is remaining classifier stage
  extraction.
- Moved remaining classifier stage and decision helpers into
  `analysis/classify/{infection,reveal,decision}.rs` while preserving
  classification behavior. The next ready seam is app use-case splitting.
- Split check, explain, and context use-case orchestration into focused `app`
  modules while preserving public API and output behavior. The next ready seam
  is output format extraction.
- Moved `OutputFormat` into `output/format.rs` while preserving the
  `app::OutputFormat` public path and output behavior. The next ready seam is
  render dispatch extraction.
- Moved `render_check` dispatch into `output/render.rs` while preserving the
  `app::render_check` public facade and output behavior. The next ready seam is
  CLI command model extraction.
- Added a focused private `cli/command.rs` `CliCommand` enum for top-level CLI
  command shape while preserving CLI parsing and dispatch behavior. The next
  ready seam is parsed-command extraction.
- Updated CLI parsing so `cli::parse` returns the typed `CliCommand` shape
  before dispatch, while preserving command argument behavior. The next ready
  seam is CLI execution extraction.
- Moved CLI command execution dispatch into `cli/execute.rs` while preserving
  parsed argument and handler behavior. The next ready seam is context packet
  DTO extraction.
- Added the domain-owned `ContextPacket` DTO shape in `domain/context_packet.rs`
  without changing context packet JSON rendering. The next ready seam is wiring
  JSON context rendering through the DTO.
- Updated JSON context packet rendering to build from the domain `ContextPacket`
  DTO while preserving the emitted packet schema. The next ready seam is LSP
  context packet usage.
- Updated LSP context packet lookup to build finding packets through the domain
  `ContextPacket` DTO while preserving the emitted packet schema. The next
  ready seam is doc-hidden internal modules.
- Marked compatibility module exports as doc-hidden so generated Rust docs point
  new integrations at crate-root re-exports. The optional private-internals seam
  remains blocked behind an explicit breaking public API decision.
- Added `cargo xtask targeted-test-outcome` as an advisory receipt for comparing
  before/after `repo-exposure-json` artifacts. The report writes
  `target/ripr/reports/targeted-test-outcome.{json,md}`, matches seams by
  `seam_id`, summarizes grip-class movement, and keeps runtime mutation
  confirmation as a separate calibration step.
- Added `docs/TARGETED_TEST_WORKFLOW.md` to join repo exposure snapshots, LSP
  seam actions, targeted-test receipts, SARIF policy, badge artifacts, and
  mutation calibration into one operator loop for adding a focused test.
- Updated `ripr check --help` to list the repo seam, repo exposure, repo SARIF,
  and agent seam packet formats used by the targeted-test workflow.
- Extended `cargo xtask mutation-calibration` with advisory static/runtime
  agreement buckets, precision notes, static-only finding samples, and runtime
  gap signals that did not line up with a static gap.
- Added `fixtures/CALIBRATION_CORPUS.md` as a controlled-scenario index for
  targeted-test receipts, static/runtime calibration, SARIF, badges, and LSP
  alignment checks without changing fixture execution.
- Documented a copyable, non-blocking GitHub Actions recipe for rendering RIPR
  SARIF and uploading it to GitHub code scanning.
- Updated targeted-test outcome Markdown to show unchanged seams and their
  evidence deltas, so a receipt can show static evidence movement even when the
  grip class does not change.
- Added a boundary-gap targeted-test case study showing one focused test, the
  before/after receipt, and the current static evidence gap when the class stays
  `weakly_gripped`.

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
