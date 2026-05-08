# Implementation Plan

This is the working PR checklist for building `ripr` incrementally. It is more
operational than the [roadmap](ROADMAP.md): each entry should become a scoped PR
with clear artifacts, tests, documentation updates, and gates.

The checklist is grouped into
[implementation campaigns](IMPLEMENTATION_CAMPAIGNS.md). A Codex Goal may work
through multiple work items in a campaign, but each work item should follow the
[scoped PR contract](SCOPED_PR_CONTRACT.md).

## Campaign Map

| Campaign | Objective | Work items |
| --- | --- | --- |
| Agentic DevEx Foundation | Make the repo safe for Codex Goals and human review. | `policy/architecture-guard`, `output/output-contract-check`, `docs/codex-goals-campaigns`, `fixtures/runner-comparison-v1`, `fixtures/first-two-goldens`, `testing/test-oracle-report`, `dogfood/static-self-check` |
| Syntax-Backed Analyzer Foundation | Move the analyzer from lexical facts to syntax-backed facts. | `analysis/file-facts-model`, `analysis/syntax-adapter-mvp`, `design/rust-syntax-substrate`, `analysis/ast-test-oracle-extraction`, `analysis/ast-probe-ownership`, `analysis/ast-probe-generation` |
| Evidence Quality | Improve oracle strength, local flow, activation values, output evidence, and stop reasons. | `output/unknown-stop-reason-invariant`, `analysis/oracle-strength-v2`, `analysis/local-delta-flow-v1`, `analysis/activation-value-modeling-v1`, `output/evidence-first-output`, `fixtures/negative-metamorphic-baseline` |
| Test Efficiency and Vacuity Signals (4A) | Make low-discriminator, smoke-only, broad-oracle, opaque, circular, and duplicate test signals visible as advisory evidence; ship `ripr` and `ripr+` badge artifacts. | `test-efficiency/test-fact-ledger`, `test-efficiency/vacuous-signal-v1`, `test-efficiency/duplicate-discriminator-v1`, `test-efficiency/report-and-metrics`, `badge/ripr-count-v1`, `badge/ripr-plus-count-v1`, `badge/repo-scope-artifacts`, `badge/publish-main-endpoint` |
| Repo Seam Inventory and Test Grip (4B) | Inventory behavior seams, classify test-grip per seam, and turn actionable gaps into editor diagnostics and agent-ready packets. | `spec/repo-seam-inventory`, `analysis/repo-seam-model-v1`, `analysis/repo-seam-inventory-v1`, `analysis/test-grip-evidence-v1`, `analysis/repo-ripr-classification-v1`, `output/repo-exposure-report-v1`, `lsp/repo-seam-diagnostics-v1`, `lsp/seam-evidence-hover-v1`, `context/agent-seam-packets-v1`, `docs/agent-dispatch-workflow-v1` |
| Seam Evidence Usability and Precision (5A) | Make repo seam evidence fast, precise, and directly actionable for developers and coding agents. | Complete: #255, #310, #313, #314, #315, #316, #327, and `campaign/seam-evidence-usability-closeout`. |
| Operationalization (5B) | Govern analyzer behavior with repository config, integrate SARIF/CI policy modes, and remap badges onto seam-native counts. | Complete: `config/ripr-config-v1`, `ci/sarif-ci-policy`, `badge/seam-native-count-mapping`, and `campaign/operationalization-closeout`. |
| Module SRP Refactoring (6) | Refactor internal modules under `crates/ripr/src/` so each module has one product responsibility, without splitting the package. | Complete: #347, the Campaign 6 refactor chain through #405, and `campaign/modularization-closeout`. |
| Defaults-First Operator Adoption (7) | Make a clean install useful through conservative defaults, one operator cockpit, CI artifacts, editor install docs, examples, and install/release proof. | Complete: #409 through #417 plus `campaign/defaults-first-closeout`. |
| Runtime Calibration Fixture Expansion (8) | Expand supplied-runtime calibration fixtures without making RIPR run mutation tests. | Complete: #420 plus `campaign/runtime-calibration-closeout`. |
| Hot Sidecar Latency Proof (9) | Measure current cache and saved-workspace editor refresh behavior before changing warm-path reuse. | Complete: latency reporting, warm-path reuse, bounded `ripr pilot`, first-screen clarity, evidence progress tracing, hot-path indexes, and `campaign/hot-sidecar-latency-closeout`. |
| Editor Agent Integration (10) | Make the saved-workspace editor loop and the agent CLI loop line up from diagnostic to evidence, packet/brief, focused test, verify, receipt, cockpit, CI, and install proof. | Complete: `campaign/editor-agent-integration-closeout`. |
| LLM Work Loop (11) | Make the completed editor-agent loop stateful, deterministic, and useful to LLM agents under review pressure. | Complete: status, command templates, workflow manifests, receipt provenance, next-action guidance, reviewer summary, fixture matrix, CI work packets, operator guide, and `campaign/llm-work-loop-closeout`. |
| First-Hour UX (12) | Make new LSP-first and CI-first users successful without learning RIPR's internal report topology. | Active: `spec/pr-test-guidance-annotations` is the first ready contract item before editor or CI behavior changes. |

The active machine-readable campaign is `.ripr/goals/active.toml`. Campaigns 1
through 8 are complete. Campaign 6 closed after the internal module SRP chain
landed through #405 while preserving the saved-workspace LSP cockpit contract,
output schemas, public API, SARIF, and badge behavior. Campaign 7 closed after
the defaults-first CLI, editor, CI, fixture, release, and report surfaces were
verified; the closeout audit lives at
`docs/handoffs/2026-05-07-campaign-7-closeout.md`. Campaign 8 added the checked
`fixtures/boundary_gap/calibration/runtime-fixtures-v1/` sample for the main
static/runtime agreement buckets and closed with runtime calibration still
confined to supplied-data reports. Campaign 9 measured the cache/editor proof
surfaces, added bounded latency reporting, reused warm-path facts below rendered
outputs, bounded `ripr pilot`, improved first-screen pilot clarity, added
evidence progress tracing, and closed after hot-path evidence indexes made the
default latency report pass on cache hits. Campaign 10 closed after aligning
the saved-workspace editor and agent CLI loop through diagnostics, evidence,
packet/brief commands, focused-test receipts, cockpit status, generated CI
artifacts, and release-readiness proof. Campaign 11 closed after adding a
read-only `ripr agent status` lens over existing agent-loop artifacts,
centralized command templates for CLI, LSP, cockpit, generated CI, docs, and
fixtures, source-edit-free workflow manifests, provenance-backed receipts,
bounded next-action guidance, review summaries, a fixture matrix, generated CI
work-loop packet uploads, and the LLM operator guide. Campaign 12 is active as
the First-Hour UX lane after the LLM work-loop control plane: it keeps the CLI
as the shared engine while making the extension and generated CI workflow feel
obvious from their own first screens.

## PR 0: `planning-and-tracking-docs`

Purpose: put the plan, engineering rules, metrics, ADRs, specs, changelog, and
traceability conventions in the repository before analyzer rewrites begin.

Deliverables:

- [x] Update `docs/ROADMAP.md` with the release sequence and PR queue.
- [x] Add an implementation checklist that future PRs can update.
- [x] Add ADR scaffolding and initial ADRs for product-shaping decisions.
- [x] Add spec scaffolding for behavior contracts.
- [x] Add metrics definitions for capability and regression tracking.
- [x] Add learnings and repo-knowledge log.
- [x] Add spec-test-code traceability rules.
- [x] Update the README doc index and metric summary.
- [x] Add a root changelog.
- [x] Add PR review checklist guidance.
- [x] Add contributor workflow guidance.
- [x] Add CI strategy guidance.
- [x] Add dogfooding guidance.
- [x] Add ADR and spec templates.
- [x] Add changelog policy guidance.
- [x] Add scoped evidence-heavy PR doctrine.
- [x] Add first executable policy checks for static language and panic-family
      debt.

Acceptance:

- [x] A contributor can identify the next PR from docs alone.
- [x] A contributor can identify which spec, tests, and code modules belong
      together for a feature.
- [x] The docs state that production and test code should avoid `panic`,
      `unwrap`, and `expect`, and that existing uses are tracked debt.
- [x] The docs preserve the product contract and conservative static language.
- [x] PRs are scoped by production risk rather than line count.

## PR 1: `verify-one-click-extension-install`

Purpose: verify the normal VS Code extension path without requiring users to
install `ripr` separately.

Deliverables:

- [ ] Manual install verification matrix for VS Marketplace and Open VSX.
- [ ] Fresh-profile check with no `ripr` on `PATH`.
- [ ] Server auto-download and checksum verification evidence.
- [ ] Output-channel log checklist for mode, base, config, server path, and
      download source.
- [ ] Clear-error scenarios for disabled auto-download, missing manifest,
      unsupported platform, and checksum mismatch.

Tests and gates:

- [ ] `cd editors/vscode && npm ci`
- [ ] `cd editors/vscode && npm run compile`
- [ ] `cd editors/vscode && npm run package`

## PR 1A: `xtask-policy-checks`

Purpose: expand the initial policy checks into a broader local and CI quality
rail.

Deliverables:

- [ ] Move static language and panic-family checks into CI.
- [ ] Add markdown local link check.
- [ ] Add doc index check for README, docs, specs, and ADRs.
- [ ] Add traceability manifest validation.
- [ ] Add capability matrix validation.
- [ ] Add PR-scope check for production delta and evidence delta.

Acceptance:

- [ ] `cargo xtask ci-fast` runs the core policy checks.
- [ ] Existing debt is allowlisted with counts, and new debt fails the check.
- [ ] Docs explain how to remove allowlist entries as debt is paid down.

## PR 1B: `rust-first-file-policy`

Purpose: keep repo implementation and automation Rust-first by denying
unapproved non-Rust programming files, checked-in executable scripts, and
workflow shell sprawl.

Deliverables:

- [ ] Add Rust-first file policy docs.
- [ ] Add non-Rust allowlist with owner, kind, and reason.
- [ ] Add workflow shell-budget allowlist.
- [ ] Add `cargo xtask check-file-policy`.
- [ ] Add `cargo xtask check-executable-files`.
- [ ] Add `cargo xtask check-workflows`.
- [ ] Wire checks into `cargo xtask ci-fast`.
- [ ] Wire checks into CI.

Acceptance:

- [ ] Rust is documented as the default implementation and automation language.
- [ ] Existing VS Code, workflow, docs, fixture, asset, and config surfaces are
      explicitly allowlisted.
- [ ] New shell, Python, JavaScript, TypeScript, or other programming files
      outside approved surfaces fail the file policy check.
- [ ] Checked-in executable bits fail unless allowlisted.
- [ ] Long workflow run blocks fail unless allowlisted.

Future policy PRs:

- [x] generated-file policy
- [x] dependency-surface policy
- [x] process-spawn policy
- [x] network policy
- [ ] workspace-shape policy
- [ ] architecture import guard
- [ ] public API guard

## PR 1C: `spec-fixture-contracts`

Purpose: make specs and fixtures agent-readable and mechanically checkable
before fixture and golden output work expands.

Deliverables:

- [ ] Add spec format reference.
- [ ] Add test taxonomy reference.
- [ ] Add fixture contract README.
- [ ] Update existing specs to the checked format.
- [ ] Add `cargo xtask check-spec-format`.
- [ ] Add `cargo xtask check-fixture-contracts`.
- [ ] Wire checks into `cargo xtask ci-fast`.
- [ ] Wire checks into CI.

Acceptance:

- [ ] Every `docs/specs/RIPR-SPEC-*.md` has required sections and a valid
      status.
- [ ] Spec filename IDs match title IDs.
- [ ] Future fixture directories must include `SPEC.md`, `diff.patch`, and
      `expected/check.json`.
- [ ] Fixture `SPEC.md` files must include Given/When/Then/Must Not sections.

## PR 1D: `automation-guardrails`

Purpose: finish the first Rust-first policy family by making generated files,
dependency surfaces, process spawning, and network behavior explicit.

Deliverables:

- [ ] Add generated-file allowlist and `cargo xtask check-generated`.
- [ ] Add dependency-surface allowlist and `cargo xtask check-dependencies`.
- [ ] Add process-spawn allowlist and `cargo xtask check-process-policy`.
- [ ] Add network allowlist and `cargo xtask check-network-policy`.
- [ ] Wire checks into `cargo xtask ci-fast`.
- [ ] Wire checks into CI.
- [ ] Update the file policy, CI docs, contributor docs, and PR template.

Acceptance:

- [ ] Tracked generated lockfiles and future fixture goldens require explicit
      allowlist entries.
- [ ] New dependency manager files fail unless they belong to approved Cargo,
      VS Code, or fixture surfaces.
- [ ] New process spawning fails unless allowlisted with a reason.
- [ ] New network behavior fails unless allowlisted with a reason.

## PR 1E: `shape-fix-pr`

Purpose: add the first mutating PR-shaping commands without changing existing
policy semantics.

Deliverables:

- [ ] Add `cargo xtask shape`.
- [ ] Add `cargo xtask fix-pr`.
- [ ] Run `cargo fmt` through `shape`.
- [ ] Sort `.ripr/*.txt` and `policy/*.txt` allowlists through `shape`.
- [ ] Ensure `target/ripr/reports` exists.
- [ ] Write `target/ripr/reports/shape.md`.
- [ ] Write `target/ripr/reports/fix-pr.md`.
- [ ] Document safe mutations and repair guidance.

Acceptance:

- [ ] `cargo xtask shape` passes.
- [ ] `cargo xtask fix-pr` passes.
- [ ] `cargo xtask ci-fast` passes after shaping.
- [ ] Shaping does not add policy exceptions or bless output drift.

## PR 1F: `pr-summary`

Purpose: generate a reviewer packet before human review without mutating source
files.

Deliverables:

- [ ] Add `cargo xtask pr-summary`.
- [ ] Read changed paths from git diff and git status.
- [ ] Write `target/ripr/reports/pr-summary.md`.
- [ ] Classify production delta and evidence/support delta.
- [ ] Classify detected surfaces, public contracts, and policy exceptions.
- [ ] Suggest reviewer focus files.
- [ ] Update `cargo xtask fix-pr` to refresh the PR summary after shaping.

Acceptance:

- [ ] `cargo xtask pr-summary` passes.
- [ ] `target/ripr/reports/pr-summary.md` exists after the command.
- [ ] `cargo xtask fix-pr` refreshes shape, PR summary, and fix-pr reports.

## PR 1G: `automation-path-docs`

Purpose: document the fix/check/guide operating model and the Codex Goals
campaign handoff so automation and analyzer implementation work share the same
review contract.

Deliverables:

- [x] Add a PR automation operating model.
- [x] Document deterministic shaping, non-mutating checks, and repair briefs.
- [x] Document the scoped PR contract.
- [x] Record the automation cutoff that made Campaign 1 safe to leave setup
      mode.
- [x] Link the new docs from the roadmap, documentation map, agent workflow,
      contributor docs, and README.

Acceptance:

- [x] A contributor can identify which cleanup should be automated and which
      changes require explicit judgment.
- [x] A coding agent can identify the next automation PRs without confusing
      them with product campaign work.
- [x] A coding agent can use a standard task template for the analyzer queue.

## PR 1H: `check-pr-precommit`

Purpose: add obvious local gates for cheap pre-commit checks and review
readiness checks.

Deliverables:

- [x] Add `cargo xtask precommit`.
- [x] Add `cargo xtask check-pr`.
- [x] Keep `precommit` cheap and non-mutating.
- [x] Make `check-pr` run the review-ready command set that exists today.
- [x] Update CI, contributor, and agent docs.

Acceptance:

- [x] `cargo xtask precommit` passes on main.
- [x] `cargo xtask check-pr` passes on main.
- [x] `check-pr` does not run release packaging unless the repo later adds a
      path-aware release lane.

## PR 1I: `guided-check-reports`

Purpose: make existing policy checks emit repair briefs instead of only command
failure text.

Deliverables:

- [x] Add a shared report model or helper for Markdown check reports.
- [x] Upgrade static-language, panic-family, file-policy, executable-file,
      workflow, spec-format, fixture-contract, generated, dependency, process,
      and network checks to write reports under `target/ripr/reports`.
- [x] Classify failures as auto-fixable, author decision, reviewer decision, or
      policy exception.
- [x] Include exact rerun commands and exception templates where useful.

Acceptance:

- [x] Each upgraded check writes a useful report on failure.
- [x] Successful checks either write a pass report or are summarized by
      `pr-summary`.
- [x] Report generation does not hide the non-zero exit status of failed checks.

## PR 1J: `ci-report-artifacts`

Purpose: make CI upload review artifacts even when a check fails.

Deliverables:

- [x] Run `cargo xtask pr-summary` where possible in CI.
- [x] Defer metrics report generation until `cargo xtask metrics` exists.
- [x] Upload `target/ripr/reports` with an always step.
- [x] Document report artifact names and expected contents.

Acceptance:

- [x] CI artifacts include the PR summary and any check reports that were
      generated before failure.
- [x] CI remains non-mutating.

## PR 1K: `fixture-golden-scaffolding`

Purpose: add the command surface for fixture execution and golden comparison
before analyzer internals change.

Deliverables:

- [x] Add `cargo xtask fixtures`.
- [x] Add `cargo xtask fixtures <name>`.
- [x] Add `cargo xtask goldens check`.
- [x] Add `cargo xtask goldens bless <name> --reason "..."`.
- [x] Document the fixture and golden directory conventions.

Acceptance:

- [x] Fixture commands pass with a clear "no fixtures found" message if no
      executable fixtures exist yet.
- [x] Existing fixture contract checks still pass.
- [x] Golden blessing requires an explicit reason.

## PR 1L: `traceability-spec-id-checks`

Purpose: make spec IDs and behavior manifest entries checkable.

Deliverables:

- [x] Harden `.ripr/traceability.toml`.
- [x] Add `cargo xtask check-spec-ids`.
- [x] Add `cargo xtask check-behavior-manifest`.
- [ ] Add warning-only drift checks for analysis, output, docs, fixture, and
      metric changes.

Acceptance:

- [x] Accepted specs point to real docs and at least one test or fixture unless
      explicitly planned.
- [x] Fixture specs reference valid spec IDs.
- [ ] Missing expected evidence appears in the PR summary.

## PR 1M: `capability-metrics-report`

Purpose: make capability progress and automation debt visible.

Deliverables:

- [x] Add or harden a machine-readable capability source.
- [x] Add `cargo xtask metrics`.
- [x] Add `cargo xtask check-capabilities`.
- [x] Write `target/ripr/reports/metrics.md` or `metrics.json`.
- [ ] Keep the README capability snapshot aligned with the capability source.

Acceptance:

- [x] Capability statuses have valid values and required fields.
- [x] Stable or calibrated statuses require the evidence defined by policy.
- [x] Metrics reports are generated without changing product behavior.

## PR 1N: `architecture-guard`

Purpose: protect internal seams while keeping one published package.

Deliverables:

- [x] Add `cargo xtask check-workspace-shape`.
- [x] Add `cargo xtask check-architecture`.
- [x] Add `cargo xtask check-public-api` or document why it is deferred.
- [x] Add policy metadata for allowed workspace packages and module-boundary
      rules.

Acceptance:

- [x] New workspace packages require an explicit approved policy entry.
- [x] Domain and analysis layers cannot accidentally depend on adapters.
- [x] CLI, LSP, and output layers do not own exposure classification.

## PR 1O: `readme-state-and-link-checks`

Purpose: make README state and Markdown links part of the checked trust packet.

Deliverables:

- [x] Add `cargo xtask check-readme-state`.
- [x] Add `cargo xtask markdown-links`.
- [x] Check README front-door sections and headline capability snapshot shape.
- [x] Check README/capability matrix checkpoint drift against
      `metrics/capabilities.toml`.
- [x] Check repo-local Markdown links in tracked `.md` files.
- [x] Wire the checks into `precommit` and `ci-fast`.
- [x] Update CI and PR automation docs.

Acceptance:

- [x] Deleted or renamed docs fail before review when still linked.
- [x] README remains linked to active campaign, metrics, capability, and
      automation docs.
- [x] `cargo xtask check-readme-state` and `cargo xtask markdown-links` pass on
      main.

## PR 1P: `campaign-manifest-check`

Purpose: make the active Codex Goals campaign queue mechanically checkable and
reportable.

Deliverables:

- [x] Add `cargo xtask check-campaign`.
- [x] Add `cargo xtask check-goals` as an alias.
- [x] Add `cargo xtask goals status`.
- [x] Add `cargo xtask goals next`.
- [x] Validate `.ripr/goals/active.toml` against
      `docs/IMPLEMENTATION_CAMPAIGNS.md`.
- [x] Validate work item IDs, statuses, branch fields, acceptance claims,
      stackability, merge boundaries, blocked dependencies, and command names.
- [x] Wire the manifest check into `precommit` and `ci-fast`.

Acceptance:

- [x] `cargo xtask check-campaign` passes on main.
- [x] `cargo xtask goals status` writes `target/ripr/reports/goals.md`.
- [x] `cargo xtask goals next` writes `target/ripr/reports/goals-next.md`.

## PR 1Q: `fixtures-runner-comparison-v1`

Purpose: make fixture and golden commands execute the current product and
compare actual output against checked-in expected output.

Deliverables:

- [x] `cargo xtask fixtures` runs all fixtures when fixture directories exist.
- [x] `cargo xtask fixtures <name>` runs one fixture.
- [x] Actual JSON and human outputs are written under
      `target/ripr/fixtures/<name>/`.
- [x] `cargo xtask goldens check` compares actual `check.json` and optional
      `human.txt` outputs against `fixtures/<name>/expected/`.
- [x] `cargo xtask goldens bless <name> --reason "..."` requires a reason,
      updates `expected/check.json` and `expected/human.txt`, and appends the
      fixture changelog.

Acceptance:

- [x] Fixture commands still pass with a clear report when no fixture
      directories exist.
- [x] Golden checks fail on drift without mutating expected outputs.
- [x] Golden blessing remains explicit and does not run from `shape` or
      `fix-pr`.

## PR 2: `fixture-laboratory`

Purpose: build the regression control bench before changing analyzer internals.

Deliverables:

- [x] `fixtures/boundary_gap`
- [x] `fixtures/weak_error_oracle`
- [ ] `fixtures/field_not_asserted`
- [ ] `fixtures/side_effect_unobserved`
- [ ] `fixtures/smoke_assertion_only`
- [ ] `fixtures/no_static_path`
- [ ] `fixtures/opaque_fixture`
- [ ] `fixtures/workspace_cross_crate`
- [ ] `fixtures/duplicate_symbols`
- [ ] `fixtures/stacked_test_attrs`
- [ ] `fixtures/nested_src_tests_layout`
- [ ] `fixtures/macro_unknown`
- [ ] `fixtures/snapshot_oracle`
- [ ] `fixtures/mock_effect`

Each fixture should include:

- [x] source and tests
- [x] `diff.patch`
- [x] expected JSON output
- [x] expected human output
- [ ] expected context packet
- [ ] expected LSP diagnostic shape when relevant

Invariants:

- [ ] Static output never says `killed` or `survived`.
- [ ] Unknowns include stop reasons.
- [ ] Weak or smoke oracle evidence does not silently become strong.
- [ ] Finding order is deterministic.
- [ ] Context packets are parseable.

## PR 2A: `testing-test-oracle-report`

Purpose: measure `ripr`'s own test oracle strength as analyzer work expands.

Deliverables:

- [x] `cargo xtask test-oracle-report` writes
      `target/ripr/reports/test-oracles.md`.
- [x] `cargo xtask test-oracle-report` writes
      `target/ripr/reports/test-oracles.json`.
- [x] `cargo xtask check-test-oracles` aliases the same advisory report.
- [x] The report classifies detected Rust tests as strong, medium, weak, or
      smoke.
- [x] Existing weak or smoke debt is advisory and non-blocking.

Acceptance:

- [x] `cargo xtask test-oracle-report`
- [x] `cargo xtask check-test-oracles`
- [x] `cargo xtask metrics`
- [x] `cargo xtask check-pr`

## PR 2B: `dogfood-static-self-check`

Purpose: add a focused non-blocking `ripr`-on-`ripr` report.

Deliverables:

- [x] `cargo xtask dogfood` runs stable fixture diffs through
      `ripr check --mode fast`.
- [x] Actual dogfood JSON and human outputs are written under
      `target/ripr/dogfood/<fixture>/`.
- [x] `target/ripr/reports/dogfood.md` summarizes findings, exposure classes,
      runtime, and errors.
- [x] `target/ripr/reports/dogfood.json` provides the same advisory summary for
      future machine readers.
- [x] Dogfood is advisory and non-blocking.

Acceptance:

- [x] `cargo xtask dogfood`
- [x] `cargo xtask check-pr`

## PR 3: `file-facts-model`

Purpose: introduce an internal fact model while preserving current scanner
behavior.

Deliverables:

- [x] `FileFacts`
- [x] `FunctionFact`
- [x] `TestFact`
- [x] `OracleFact`
- [x] `CallFact`
- [x] `ReturnFact`
- [ ] `StructConstructionFact`
- [ ] `EnumConstructionFact`
- [x] `LiteralFact`
- [ ] `BuilderChainFact`
- [ ] `EffectFact`

Acceptance:

- [x] Existing sample findings are unchanged.
- [x] Analysis consumes facts rather than ad hoc scanner structures.
- [x] Scanner behavior remains available as the fallback.

## PR 4: `syntax-adapter-mvp`

Purpose: create the parser boundary before relying on parser-specific details.

Deliverables:

- [x] `RustSyntaxAdapter` trait or equivalent boundary.
- [x] Lexical adapter `summarize_file` implementation.
- [x] Changed range to syntax-node mapping.
- [x] No public API commitment to a parser crate.
- [x] Parser substrate decision recorded in
      [ADR 0006](adr/0006-rust-syntax-substrate.md).
- [x] Parser-backed `summarize_file` implementation.

Acceptance:

- [x] Existing outputs remain stable or intentionally updated with fixture
      evidence.
- [ ] Parser errors produce `static_unknown` or structured diagnostics, not
      panics.

## PR 5: `ast-test-oracle-extraction`

Purpose: extract tests and oracles from syntax nodes instead of line substrings.

Deliverables:

- [x] `#[test]` function extraction.
- [x] Stacked attribute preservation.
- [x] Multi-line assertion macro extraction.
- [x] `assert!`, `assert_eq!`, `assert_ne!`, `assert_matches!`, and `matches!`
      handling.
- [x] `unwrap` and `expect` smoke-oracle handling.

Acceptance:

- [x] Fixture output remains deterministic.
- [x] Line scanning is fallback only.

## PR 6: `ast-probe-ownership`

Purpose: attach probes to stable owner symbols.

Deliverables:

- [x] Diff hunk to changed text range.
- [x] Changed range to syntax-backed owner node.
- [x] Syntax node to enclosing function, method, or module.
- [x] Stable `SymbolId`.

Acceptance:

- [x] Duplicate function names across modules or crates do not cross-link tests.
- [x] Probe IDs remain stable enough for `explain` and `context`.

## PR 7: `ast-probe-generation`

Purpose: generate probes from syntax kind and ownership facts.

Deliverables:

- [x] Predicate boundary probes.
- [x] Return value probes.
- [x] Error path probes.
- [x] Field construction probes.
- [x] Side-effect or call-change probes.
- [x] `static_unknown` fallback with reason.

Acceptance:

- [x] Multi-line predicate changes produce one useful probe.
- [x] Tail-expression return changes produce return probes.
- [x] `Err(Error::X)` changes produce error-path probes.

## PR 8: `oracle-strength-v2`

Purpose: make oracle kind and strength explicit and probe-relative.

Deliverables:

- [x] Exact value oracle.
- [x] Exact error variant oracle.
- [x] Broad error oracle.
- [ ] Whole-object equality oracle.
- [x] Snapshot oracle.
- [x] Mock expectation oracle.
- [x] Relational check oracle.
- [ ] Shape-only oracle.
- [x] Smoke-only oracle.
- [x] Unknown oracle kind.

Acceptance:

- [x] `is_err()` differs from exact error variant assertions.
- [x] `unwrap()` differs from exact return assertions.
- [x] JSON and human output keep the stable schema while rendering
  probe-relative oracle strength.

## PR 9: `local-delta-flow-v1`

Purpose: explain what changed behavior appears to flow to.

Deliverables:

- [x] Changed expression to `let` binding flow.
- [x] Binding to return flow.
- [x] Binding to struct field flow.
- [x] Changed expression to `Ok` or `Err` flow.
- [x] Predicate branch to return or field construction flow.
- [x] Changed call to effect boundary candidate.

Acceptance:

- [x] Findings can name at least one sink when locally visible.
- [x] `propagation_unknown` includes a concrete stop reason.

## PR 10: `activation-value-modeling-v1`

Purpose: detect whether tests appear to activate the changed behavior.

Deliverables:

- [x] Numeric and string literal value facts.
- [x] Function argument value facts.
- [x] Builder-chain value facts.
- [x] Table-row value facts.
- [x] Enum variant value facts.
- [x] Boundary equality discriminator facts.

Acceptance:

- [x] Boundary findings include detected values.
- [x] Boundary findings include missing equality value.
- [x] Opaque fixtures produce `infection_unknown`, not false confidence.

## PR 11: `evidence-first-output`

Purpose: make CLI output the reference explanation.

Deliverables:

- [x] Changed behavior section.
- [x] RIPR stage evidence section.
- [x] Related tests section.
- [x] Oracle evidence section.
- [x] Missing discriminator section.
- [x] Next step section.
- [x] Stop reason section for unknowns.

Acceptance:

- [x] Golden human and JSON output cover current Campaign 3 fixtures.
- [x] Static language remains conservative.
- [ ] Negative and metamorphic fixtures cover noise-only and syntax-variant
      cases.

## PR 12: `lsp-evidence-hover-actions`

Purpose: make editor diagnostics specific and actionable.

Deliverables:

- [ ] Diagnostic data with finding and probe IDs.
- [ ] Stable diagnostic codes.
- [ ] Hover evidence for exact finding.
- [ ] Copy context packet code action.
- [ ] Open related tests code action.
- [ ] Run deep check command.
- [ ] Output-channel lifecycle logs.

Acceptance:

- [ ] `didChange` refreshes diagnostics after debounce.
- [ ] Code action copies the context for the selected finding.

## PR 13: `agent-context-v2`

Purpose: turn `ripr context` into a test-writing brief.

Deliverables:

- [ ] Recommended test location.
- [ ] Related existing tests.
- [ ] Fixture or builder hints.
- [ ] Missing input values.
- [ ] Missing oracle shape.
- [ ] Suggested assertion shapes.
- [ ] Confidence and stop reasons.

Acceptance:

- [ ] Context packet is golden-tested.
- [ ] CLI and LSP use the same packet shape.

## PR 14: `ripr-config-v1`

Purpose: let repositories teach `ripr` topology and oracle conventions.

Deliverables:

- [ ] Workspace-root config discovery.
- [ ] Missing config accepted.
- [ ] Useful invalid-config errors.
- [ ] Test topology override.
- [ ] Custom oracle macro config.
- [ ] Snapshot, mock, and external-boundary config.

Acceptance:

- [ ] Config changes oracle classification only through explicit rules.

## PR 15: `suppression-v1`

Purpose: support honest noise control without hiding the model.

Deliverables:

- [ ] Inline suppression comment form.
- [ ] Config suppression form.
- [ ] Required reason.
- [ ] Optional expiry.
- [ ] `--show-suppressed`.

Acceptance:

- [ ] Suppressed findings remain visible when requested.
- [ ] Suppression rate can be measured.

## PR 16: `sarif-ci-policy`

Purpose: support PR workflows without making default CI noisy.

Deliverables:

- [x] SARIF output.
- [x] Markdown summary.
- [x] JSON artifact guidance.
- [x] Advisory mode.
- [x] Opt-in failure modes.
- [x] Baseline-aware mode.

Acceptance:

- [x] SARIF validates.
- [x] SARIF results point to static evidence locations.
- [x] Blocking policy is opt-in.

## PR 17: `cargo-mutants-calibration-scaffold`

Purpose: compare static predictions with real mutation results.

Deliverables:

- [x] Import cargo-mutants output through `cargo xtask mutation-calibration`
  and public `ripr calibrate cargo-mutants`.
- [x] Match static seam evidence to runtime records by `seam_id` first and
  unambiguous normalized file/line second; report ambiguous file/line
  candidates separately.
- [x] Emit advisory static class vs runtime outcome reports at
  `target/ripr/reports/mutation-calibration.{json,md}`.
- [x] Keep mutation-runtime language out of static findings; runtime vocabulary
  is confined to calibration/runtime reports.

Acceptance:

- [x] Runtime mutation vocabulary appears only in explicit calibration data and
  static-language checks remain clean.

## PR 18: `persistent-cache-v1`

Purpose: cache stable facts after the fact model is worth caching.

Deliverables:

- [ ] File-hash invalidation.
- [ ] Warm `FileFacts` reuse.
- [ ] LSP reuse of test and oracle facts.
- [ ] Graceful stale-cache recovery.

Acceptance:

- [ ] Warm run avoids reparsing unchanged files.

## Required Gates

Rust PRs must run:

```bash
cargo fmt --check
cargo check --workspace --all-targets
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo doc --workspace --no-deps
cargo package -p ripr --list
cargo publish -p ripr --dry-run
```

Extension PRs must run:

```bash
cd editors/vscode
npm ci
npm run compile
npm run package
```
