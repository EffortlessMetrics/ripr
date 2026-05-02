# Implementation Plan

This is the working PR checklist for building `ripr` incrementally. It is more
operational than the [roadmap](ROADMAP.md): each entry should become a scoped PR
with clear artifacts, tests, documentation updates, and gates.

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

## PR 2: `fixture-laboratory`

Purpose: build the regression control bench before changing analyzer internals.

Deliverables:

- [ ] `fixtures/boundary_gap`
- [ ] `fixtures/weak_error_oracle`
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

- [ ] source and tests
- [ ] `diff.patch`
- [ ] expected JSON output
- [ ] expected human output
- [ ] expected context packet
- [ ] expected LSP diagnostic shape when relevant

Invariants:

- [ ] Static output never says `killed` or `survived`.
- [ ] Unknowns include stop reasons.
- [ ] Weak or smoke oracle evidence does not silently become strong.
- [ ] Finding order is deterministic.
- [ ] Context packets are parseable.

## PR 3: `file-facts-model`

Purpose: introduce an internal fact model while preserving current scanner
behavior.

Deliverables:

- [ ] `FileFacts`
- [ ] `FunctionFact`
- [ ] `TestFact`
- [ ] `OracleFact`
- [ ] `CallFact`
- [ ] `ReturnFact`
- [ ] `StructConstructionFact`
- [ ] `EnumConstructionFact`
- [ ] `LiteralFact`
- [ ] `BuilderChainFact`
- [ ] `EffectFact`

Acceptance:

- [ ] Existing sample findings are unchanged.
- [ ] Analysis consumes facts rather than ad hoc scanner structures.
- [ ] Scanner behavior remains available as the fallback.

## PR 4: `syntax-adapter-mvp`

Purpose: create the parser boundary before relying on parser-specific details.

Deliverables:

- [ ] `RustSyntaxAdapter` trait or equivalent boundary.
- [ ] Parser-backed `summarize_file` implementation.
- [ ] Changed range to syntax-node mapping.
- [ ] No public API commitment to a parser crate.

Acceptance:

- [ ] Existing outputs remain stable or intentionally updated with fixture
      evidence.
- [ ] Parser errors produce `static_unknown` or structured diagnostics, not
      panics.

## PR 5: `ast-test-oracle-extraction`

Purpose: extract tests and oracles from syntax nodes instead of line substrings.

Deliverables:

- [ ] `#[test]` function extraction.
- [ ] Stacked attribute preservation.
- [ ] Multi-line assertion macro extraction.
- [ ] `assert!`, `assert_eq!`, `assert_ne!`, `assert_matches!`, and `matches!`
      handling.
- [ ] `unwrap` and `expect` smoke-oracle handling.

Acceptance:

- [ ] Fixture output remains deterministic.
- [ ] Line scanning is fallback only.

## PR 6: `ast-probe-ownership`

Purpose: attach probes to stable owner symbols.

Deliverables:

- [ ] Diff hunk to changed text range.
- [ ] Changed range to syntax node.
- [ ] Syntax node to enclosing function, method, or module.
- [ ] Stable `SymbolId`.

Acceptance:

- [ ] Duplicate function names across modules or crates do not cross-link tests.
- [ ] Probe IDs remain stable enough for `explain` and `context`.

## PR 7: `ast-probe-generation`

Purpose: generate probes from syntax kind and ownership facts.

Deliverables:

- [ ] Predicate boundary probes.
- [ ] Return value probes.
- [ ] Error path probes.
- [ ] Field construction probes.
- [ ] Side-effect or call-change probes.
- [ ] `static_unknown` fallback with reason.

Acceptance:

- [ ] Multi-line predicate changes produce one useful probe.
- [ ] Tail-expression return changes produce return probes.
- [ ] `Err(Error::X)` changes produce error-path probes.

## PR 8: `oracle-strength-v2`

Purpose: make oracle kind and strength explicit and probe-relative.

Deliverables:

- [ ] Exact value oracle.
- [ ] Exact error variant oracle.
- [ ] Whole-object equality oracle.
- [ ] Snapshot oracle.
- [ ] Mock expectation oracle.
- [ ] Relational check oracle.
- [ ] Shape-only oracle.
- [ ] Smoke-only oracle.
- [ ] Unknown oracle with stop reason where applicable.

Acceptance:

- [ ] `is_err()` differs from exact error variant assertions.
- [ ] `unwrap()` differs from exact return assertions.
- [ ] JSON and context include oracle kind and strength.

## PR 9: `local-delta-flow-v1`

Purpose: explain what changed behavior appears to flow to.

Deliverables:

- [ ] Changed expression to `let` binding flow.
- [ ] Binding to return flow.
- [ ] Binding to struct field flow.
- [ ] Changed expression to `Ok` or `Err` flow.
- [ ] Predicate branch to return or field construction flow.
- [ ] Changed call to effect boundary candidate.

Acceptance:

- [ ] Findings can name at least one sink when locally visible.
- [ ] `propagation_unknown` includes a concrete stop reason.

## PR 10: `activation-value-modeling-v1`

Purpose: detect whether tests appear to activate the changed behavior.

Deliverables:

- [ ] Numeric and string literal value facts.
- [ ] Function argument value facts.
- [ ] Builder-chain value facts.
- [ ] Table-row value facts.
- [ ] Enum variant value facts.
- [ ] Boundary neighbor suggestions.

Acceptance:

- [ ] Boundary findings include detected values.
- [ ] Boundary findings include missing equality value.
- [ ] Opaque fixtures produce `infection_unknown`, not false confidence.

## PR 11: `evidence-first-output`

Purpose: make CLI output the reference explanation.

Deliverables:

- [ ] Changed behavior section.
- [ ] RIPR stage evidence section.
- [ ] Related tests section.
- [ ] Oracle evidence section.
- [ ] Missing discriminator section.
- [ ] Next step section.
- [ ] Stop reason section for unknowns.

Acceptance:

- [ ] Golden human output covers each finding class.
- [ ] Static language remains conservative.

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

- [ ] SARIF output.
- [ ] Markdown summary.
- [ ] JSON artifact guidance.
- [ ] Advisory mode.
- [ ] Opt-in failure modes.
- [ ] Baseline-aware mode.

Acceptance:

- [ ] SARIF validates.
- [ ] GitHub annotations point to changed lines.
- [ ] Blocking policy is opt-in.

## PR 17: `cargo-mutants-calibration-scaffold`

Purpose: compare static predictions with real mutation results.

Deliverables:

- [ ] Import cargo-mutants output.
- [ ] Match obvious static probe to mutant result.
- [ ] Emit static class vs real outcome report.
- [ ] Keep mutation-runtime language out of static findings.

Acceptance:

- [ ] `killed` and `survived` appear only in explicit calibration data.

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
