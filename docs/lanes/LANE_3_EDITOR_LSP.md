# Lane 3: Editor / LSP UX

Lane 3 owns RIPR's editor and LSP projection surfaces. Its job is to make the
saved-workspace evidence loop usable at the point of coding:

```text
diagnostic -> hover evidence -> related test or context
-> packet or brief commands -> one focused test -> verify -> receipt -> refresh
```

Lane 3 follows its GitHub issue and PR tracker, this lane tracker, and the
editor/LSP docs. It does not switch to another campaign merely because
`.ripr/goals/active.toml` points elsewhere.

## Scope

Lane 3 owns these surfaces:

- LSP diagnostics and `diagnostic.data` identity;
- hover evidence rendering;
- editor status and `ripr: Show Status`;
- code actions;
- bounded context packets;
- related-test opening;
- copyable packet, brief, after-snapshot, verify, receipt, and refresh commands;
- VS Code extension behavior for server resolution, status, actions, and
  command payloads;
- `cargo xtask lsp-cockpit-report` regression evidence.

Lane 3 consumes existing RIPR artifacts when they are already present. It does
not create PR/CI reports, decide policy, or rerun hidden analysis in the editor.

## Completed Surfaces

The saved-workspace editor cockpit is closed and documented in
[Editor Evidence UX](../EDITOR_EVIDENCE_UX.md),
[Editor evidence workflow](../EDITOR_EVIDENCE_WORKFLOW.md), and the
[Editor Evidence UX closeout](../handoffs/2026-05-09-editor-evidence-ux-closeout.md).

Completed slices:

- diagnostic identity through `diagnostic.data`;
- evidence-rich hover;
- evidence-aware code actions;
- `ripr.collectEvidenceContext`;
- framed LSP protocol smoke;
- live VS Code editor evidence smoke;
- status and staleness handling;
- editor workflow docs;
- first-useful-action status projection, including wrong-root and stale-state
  handling;
- first-useful-action status edge fixtures for malformed, unsupported, missing,
  stale, and fallback report states;
- command payload contracts for packet, brief, after-snapshot, verify, receipt,
  and path-with-spaces handoffs;
- live VS Code saved-workspace smoke executes the real seam copy actions for
  packet, brief, after-snapshot, verify, receipt, suggested assertion, and
  related-test opening;
- evidence hover rendering projects matching first-useful-action reports when
  the existing report is workspace-root and seam-ID matched;
- saved-workspace `ripr: Show Status` tests pin valid first-useful-action
  output, wrong-root fail-closed behavior, stale refresh guidance, and malformed
  or missing report handling;
- `fixtures/editor_lsp_workflow` pins the saved-workspace editor loop across
  diagnostics, hover, code actions, first-useful-action status projection, stale
  refresh guidance, and LSP cockpit coverage;
- Rust language-router contract tests pin that default saved-workspace behavior
  and `[languages] enabled = ["rust"]` produce the same diagnostics, hover, and
  actions, while `[languages] enabled = []` suppresses saved-workspace
  diagnostics and surfaces an explicit `languages off` editor status instead
  of inventing editor behavior, and invalid language config stays config-owned
  while the LSP session falls back to Rust defaults.
- first-run and no-output status projection names the workspace root, resolved
  server source and command, editor selectors, enabled languages from the latest
  server refresh, and the next safe action for disabled, no-workspace,
  unavailable-server, stale, language-off, no-actionable-seam, preview, and
  diagnostic states.

## Current Open PRs

The selected Lane 3 follow-up is Editor Adoption Assurance. The opener is
docs-only and records the source-of-truth stack for #1245-#1253.

When opening future Lane 3 PRs, list them here until they merge or close:

| PR | Slice | State | Notes |
| --- | --- | --- | --- |
| #1245 | `docs(lane3): open editor adoption assurance stack` | active | Docs/source-of-truth opener only; no behavior-bearing LSP or VS Code changes. |

## Campaign 27 Routing State

Campaign 27 Language Adapter Preview closed the Lane 3
`lsp/editor-language-routing` slice in #772.

The slice landed after the Python preview adapter produced fixture-backed
owner, test, assertion/oracle, probe, related-test, and structured static-limit
facts. TypeScript adapter readiness is also complete, so Lane 3 should not
reopen TypeScript or Python editor readiness unless a new editor-facing
regression appears. Lane 3 should review future analyzer, config, and output
work only as a consumer of editor projection inputs. Rust saved-workspace
editor behavior must stay unchanged.

Current dependency state:

- the TypeScript owner+test, assertion-shape, initial probe-shape, and
  mocked-module static-limit sub-slices have landed (#777, #781, #784, #791),
  and #794 marked `analysis/typescript-preview-adapter` done as a first useful
  preview loop;
- `analysis/typescript-editor-readiness` is complete: #779 made preview
  metadata visible in human output, #780 made owner matching file-first before
  line-range matching, #782 kept broad `toThrow()` evidence weak, #785 made
  awaited `Promise.reject(...)` an error-path preview shape, and #786 added
  public fixture/golden coverage for every TypeScript probe family currently
  emitted by the preview adapter;
- assertion-shape extraction landed in #781, with a Lane 3 watchpoint that
  broad `toThrow()` assertions must not be surfaced as exact error-variant
  evidence;
- #832 closed #779, #833 closed #780, #834 closed #782, #835 closed #785, and
  #836 closed #786 without changing VS Code selectors or LSP routing;
- the Python parser substrate ADR (#770) landed in #794, was corrected to
  `rustpython-parser` in #801, and the Python scaffold landed in #804; the
  Python preview adapter then added owner/test facts, assertion/oracle facts,
  probe facts, related-test facts, and structured static-limit facts with
  fixture/golden coverage, so #771 is satisfied for editor-projectable preview
  evidence;
- issue #772 records the VS Code routing files:
  `editors/vscode/package.json` for activation and
  `editors/vscode/src/client.ts` for `documentSelector` plus the routed-file
  stale-buffer guard;
- issue #771 records the Python-to-editor handoff contract: Python preview
  artifacts carry `language = "python"`, `language_status = "preview"`, bounded
  owner facts, related-test facts, and projectable `static_limit_kind` values
  before Lane 3 adds Python selectors;
- #857 closed #807 by adding the optional structured `static_limit_kind` field
  on `Finding` and JSON output. It also closed #814 by proving the
  policy-readiness scanner sees the structured field. Future hover/status
  projection should consume `static_limit_kind` when preview artifacts populate
  it, while still rendering inspected text-only limits as evidence for adapters
  that have not promoted a particular limit kind.

Before starting any future preview-routing follow-up, refresh this audit
instead of inferring readiness from campaign momentum:

- `.ripr/goals/active.toml` must show the relevant upstream analyzer or
  projection work complete, or `cargo xtask goals next` must list the follow-up
  work item as ready;
- TypeScript and Python preview outputs must visibly carry preview language
  metadata and explicit static limits in artifacts the editor can project;
- `static_limit_kind` is available for structured limits after #857; hover and
  status should consume it when present, and the routing slice must still prove
  any text-only static-limit evidence is stable enough to project before it is
  displayed;
- do not add editor behavior that depends on text-parsing static-limit kinds;
  render text-only limits as evidence only, and keep action semantics
  independent of parsed prose;
- the TypeScript gaps tracked by #779, #780, #782, #785, and #786 are closed;
- `editors/vscode/package.json` and `editors/vscode/src/client.ts` now
  deliberately add preview selectors while analysis remains gated by
  `[languages]` through the adapter layer.

Readiness boundary:

- preview adapters must be opt-in;
- preview evidence must be labeled preview;
- static limits must be visible;
- saved-workspace analysis must route through the adapter layer only when the
  selected language is enabled;
- Rust defaults, gate semantics, generated CI, and Rust LSP/editor behavior must
  not change.

Maintenance audit evidence from 2026-05-12:

- after #804 merged, `analysis/typescript-preview-adapter` and the corrected
  `adr/python-parser-substrate` are done, and the Python scaffold is on `main`;
  at that point this tracker kept `analysis/typescript-editor-readiness` as the
  explicit TypeScript-side blocker, treated `analysis/python-preview-adapter` as
  active upstream work, and kept `lsp/editor-language-routing` blocked until
  both dependencies were complete;
- `editors/vscode/package.json` still activates on `onLanguage:rust`, and
  `editors/vscode/src/client.ts` still uses a Rust-only `documentSelector` plus
  `isRustFileDocument` guard;
- `fixtures/editor_lsp_workflow` exists and `cargo xtask lsp-cockpit-report`
  produced a passing cockpit report for the saved-workspace editor loop;
- `cargo test -p ripr lsp --lib` and
  `npm --prefix editors/vscode run compile` passed for the existing Rust editor
  cockpit, and were rerun after #784 landed on `main`;
- `cargo test -p ripr lsp::tests --lib` passed 84 LSP tests, and
  `npm --prefix editors/vscode run test:e2e` passed 30 live VS Code extension
  smoke tests for the saved-workspace Rust editor path;
- after #805 refreshed the Python-adapter blocker state, `cargo test -p ripr
  lsp --lib` passed 123 tests, `cargo xtask lsp-cockpit-report` produced a
  passing saved-workspace cockpit report, `npm --prefix editors/vscode run
  compile` passed, and `npm --prefix editors/vscode run test:e2e` passed 30
  live VS Code extension smoke tests;
- after #809 merged the docs-only preview-routing path, the saved-workspace
  Rust editor cockpit was rechecked on current `main`:
  `cargo test -p ripr lsp --lib`
  passed 123 tests, `cargo xtask lsp-cockpit-report` produced a passing report,
  `npm --prefix editors/vscode run compile` passed, and
  `npm --prefix editors/vscode run test:e2e` passed 30 live VS Code extension
  smoke tests;
- the current #787 merge-result tree changes only the classifier refactor files;
  a detached merge-result check passed `cargo test -p ripr lsp --lib` with 123
  tests and `cargo xtask lsp-cockpit-report`, so that stale refactor branch has
  no current Lane 3 file delta or cockpit regression signal;
- docs-only tracker validation passed with `cargo xtask check-doc-index`,
  `cargo xtask markdown-links`, `cargo xtask check-static-language`,
  `cargo xtask check-pr`, and `git diff --check`.
- later refreshes found #784 merged for #768 probe-shape refinement and #791
  merged for #769 mocked-module static-limit reporting; Lane 3 review captured
  then-unresolved preview-readiness gaps in #779, #780, #782, #785, and #786,
  so the routing slice remained blocked.
- after #821 merged the static-limit consumer watchpoint, current `main`
  (`58709f7`) rechecked the Rust saved-workspace editor cockpit:
  `cargo xtask goals next` reported no ready work items,
  `cargo xtask lsp-cockpit-report` passed, `cargo test -p ripr lsp --lib`
  passed 123 tests, `cargo test -p ripr lsp::tests --lib` passed 84 tests,
  `npm --prefix editors/vscode run compile` passed, and
  `npm --prefix editors/vscode run test:e2e` passed 30 live VS Code smoke
  tests; the known VS Code runner `path` warning still appears after the
  passing e2e run and exits 0.
- after #824 merged the Lane 2 policy-readiness changelog closeout, current
  `main` (`0337f43`) rechecked the saved-workspace cockpit again:
  `cargo xtask lsp-cockpit-report` passed and wrote
  `target/ripr/reports/lsp-cockpit.{md,json}` with diagnostics, code actions,
  context availability, agent packet/brief/after-snapshot/verify/receipt
  command payload fields, related-test opening, refresh, and VS Code command
  coverage; `cargo test -p ripr lsp --lib` passed 123 tests,
  `cargo test -p ripr lsp::tests --lib` passed 84 tests,
  `npm --prefix editors/vscode run compile` passed, and
  `npm --prefix editors/vscode run test:e2e` passed 30 live VS Code smoke
  tests with the same post-success `path` warning exiting 0. `cargo xtask goals
  next` still reported no ready work items, so `lsp/editor-language-routing`
  remained blocked.
- after #826 merged the Lane 3 VS Code command-copy smoke stabilization,
  current `main` (`fe8714f`) keeps the extension Rust-only while the live e2e
  command-copy assertions use a test-only clipboard capture file to avoid
  relying on the VS Code test host clipboard read path. The PR required checks
  passed, `npm --prefix editors/vscode run compile` passed,
  `npm --prefix editors/vscode run test:e2e` passed 30 live VS Code smoke
  tests after the review fix, and `cargo xtask check-pr` passed on merged
  `main`. `cargo xtask goals next` still reports no ready work items, so
  `lsp/editor-language-routing` remains blocked.
- after #827 merged Lane 1 runtime calibration fixtures, current `main`
  (`7d9001f`) was rechecked for Lane 3 impact because the merge touched shared
  fixtures and traceability but no editor/LSP projection files. The Rust
  saved-workspace cockpit stayed green: `cargo xtask lsp-cockpit-report`
  passed, `cargo test -p ripr lsp --lib` passed 123 tests,
  `cargo test -p ripr lsp::tests --lib` passed 84 tests,
  `npm --prefix editors/vscode run compile` passed, and
  `npm --prefix editors/vscode run test:e2e` passed 30 live VS Code smoke
  tests with the known post-success VS Code runner `path` warning exiting 0.
  `cargo xtask goals next` still reports no ready work items, so
  `lsp/editor-language-routing` remains blocked.
- after #836 merged TypeScript probe-family fixture coverage, current `main`
  (`d1fd943`) records `analysis/typescript-editor-readiness` as done in the
  campaign tracker. The TypeScript editor-readiness issues #779, #780, #782,
  #785, and #786 are closed, while Python preview adapter #771 remains open and
  `cargo xtask goals next` still reports no ready work items. Therefore
  `lsp/editor-language-routing` remains blocked by Python, and VS Code/LSP
  selector work must not start yet.
- after #837 synced the TypeScript readiness state, current `main`
  (`a0837f5`) was rechecked for Lane 3 impact. `cargo xtask goals next`
  reported no ready work items; `cargo xtask lsp-cockpit-report` passed;
  `cargo test -p ripr lsp --lib` passed 123 tests;
  `cargo test -p ripr lsp::tests --lib` passed 84 tests;
  `npm --prefix editors/vscode run compile` passed; and
  `npm --prefix editors/vscode run test:e2e` passed 30 live VS Code smoke
  tests with the known post-success VS Code runner `path` warning exiting 0.
  `editors/vscode/package.json` still activates on `onLanguage:rust`, and
  `editors/vscode/src/client.ts` still uses a Rust-only document selector plus
  `isRustFileDocument` guards, so preview routing remains unstarted.
- after #840 corrected the Campaign 27 manifest, current `main` (`d7d1b66`)
  records that `output/language-metadata` emitted only `language` and
  `language_status`; `owner_kind` and `static_limit_kind` remain deferred to
  follow-up metadata work such as #807. Lane 3 added matching #771/#772
  handoff notes so Python preview and editor routing both treat structured
  static-limit metadata as unavailable until it lands. A fresh maintenance pass
  found no stale open editor, LSP, VS Code, or `ripr: Show Status` issues beyond
  #772. `cargo xtask goals next` still reported no ready work items;
  `cargo xtask lsp-cockpit-report` passed; `cargo test -p ripr lsp --lib`
  passed 123 tests; `cargo test -p ripr lsp::tests --lib` passed 84 tests;
  `npm --prefix editors/vscode run compile` passed; and
  `npm --prefix editors/vscode run test:e2e` passed 30 live VS Code smoke
  tests with the known post-success VS Code runner `path` warning exiting 0.
  The extension remains Rust-only until `lsp/editor-language-routing` is ready.
- after #841 recorded the post-840 maintenance audit, current `main`
  (`5c13f2e`) was refreshed against live GitHub and local artifacts. Issue
  #771 remains open for `analysis/python-preview-adapter`, issue #772 remains
  open and blocked for `lsp/editor-language-routing`, and `cargo xtask goals
  next` still reports no ready work items. The open PR queue has no Lane 3
  editor/LSP projection files; #787 was reviewed as a classifier-only refactor
  with no actionable Lane 3 findings, while #788, #789, and #790 remain CLI
  workflow refactors and #819/#820 are docs-learning PRs. Current code still
  keeps `editors/vscode/package.json` on `onLanguage:rust` and
  `editors/vscode/src/client.ts` on a Rust-only `documentSelector` plus
  `isRustFileDocument` guards, with no CodeLens, inlay, semantic-token, or
  preview-language selector registration. The saved-workspace cockpit was
  rechecked with `cargo test -p ripr lsp --lib` (123 tests),
  `cargo xtask lsp-cockpit-report`, `npm --prefix editors/vscode run compile`,
  and `npm --prefix editors/vscode run test:e2e` (30 tests, known
  post-success `path` warning exiting 0). Boundary gates
  `cargo xtask check-output-contracts`, `cargo xtask check-capabilities`,
  `cargo xtask check-traceability`, and `cargo xtask check-pr` also passed;
  `check-pr` left only generated `crates/ripr/examples/sample/target/` build
  output, which was removed, and the worktree returned clean.
- during the next maintenance pass on current `main` (`22f5aa0`), Lane 3 kept
  routing closed and corrected stale current-state wording in
  `docs/CONFIGURATION.md`, `docs/OUTPUT_SCHEMA.md`, `ripr.toml.example`, and
  the generated config source. The corrected docs now distinguish TypeScript
  preview analysis/report output from Python's scaffold-only state until
  `analysis/python-preview-adapter` emits findings. Live GitHub still showed
  #771 open for the Python preview adapter and #772 open for blocked editor
  routing, and the open PR queue had no Lane 3 routing PR. A current code audit
  also confirmed `editors/vscode/package.json` still activates on
  `onLanguage:rust`, `editors/vscode/src/client.ts` still uses a Rust-only
  `documentSelector` and `isRustFileDocument` stale-buffer guards, the text
  change handler only marks saved-workspace state stale, LSP capabilities remain
  text sync, hover, code actions, and execute-command, and no CodeLens, inlay,
  semantic-token, or preview-language selector registration exists.
  Maintenance evidence from this pass: `cargo xtask lsp-cockpit-report` passed,
  `cargo test -p ripr lsp --lib` passed 123 tests,
  `cargo test -p ripr lsp::tests --lib` passed 84 tests,
  `npm --prefix editors/vscode run compile` passed, `npm --prefix
  editors/vscode run test:e2e` passed 30 live VS Code smoke tests with the
  known post-success `path` warning exiting 0, `cargo xtask check-doc-index`
  passed, `cargo xtask markdown-links` passed, `cargo xtask
  check-output-contracts` passed, `cargo xtask check-capabilities` passed,
  `cargo xtask check-traceability` passed, `cargo xtask check-static-language`
  passed, `cargo xtask check-fixture-contracts` passed, `cargo xtask fixtures`
  passed, `cargo xtask goldens check` passed, `cargo xtask check-generated`
  passed, `cargo xtask check-pr-shape` passed, `cargo xtask
  check-workspace-shape` passed, `cargo xtask
  check-file-policy` passed, `cargo xtask check-architecture` passed,
  `cargo xtask check-public-api` passed, `cargo xtask check-dependencies`
  passed, `cargo xtask check-process-policy` passed, `cargo xtask
  check-network-policy` passed, `cargo xtask check-workflows` passed,
  `cargo xtask check-executable-files` passed, `cargo xtask
  check-spec-format` passed, `cargo xtask check-no-panic-family` passed, the
  generated init-config unit test passed, `cargo xtask precommit` passed,
  `cargo xtask check-pr` passed, `cargo xtask dogfood` passed,
  `cargo xtask test-oracle-report` passed, `cargo xtask metrics` passed,
  `cargo package -p ripr --list --allow-dirty`
  passed after the plain package list correctly refused the uncommitted
  `crates/ripr/src/config.rs` diff, the `cargo publish -p ripr --dry-run`
  check passed with `--allow-dirty` and aborted before upload as expected,
  `git diff --check` passed, and `cargo xtask goals next` still reported no
  ready work items.
- after fetching current `origin/main` (`c712350`), Lane 3 rechecked the
  preview-routing source-of-truth stack against newly merged Lane 1 planning
  files. `origin/main` now owns `ADR-0010` and `RIPR-SPEC-0035`, so the Lane 3
  editor-preview docs were renumbered to `ADR-0011`, `RIPR-SPEC-0036`, and
  `RIPR-SPEC-0037` while keeping `RIPR-PROP-0003`. A temporary current-base
  worktree applied the local Lane 3 patch to `origin/main`, resolved only the
  expected index and traceability conflicts by keeping the Lane 1 entries plus
  the Lane 3 entries, and passed `cargo xtask check-doc-index`,
  `cargo xtask markdown-links`, `cargo xtask check-traceability`,
  `cargo xtask check-spec-format`, `cargo xtask check-static-language`,
  `git diff --check`, and `cargo xtask goals next`. Issue #771 remains open,
  issue #772 remains open and blocked, and no VS Code selector, LSP routing,
  preview workflow fixture, CodeLens, inlay, semantic-token, source-edit,
  provider, mutation, policy, or gate behavior was added.
- after fetching current `origin/main` (`23df422`), #849 has merged as a Lane
  4 PR/CI review cockpit docs-only source-of-truth PR. It does not touch
  editor/LSP projection or unblock #772. A disposable current-base worktree
  applied the local Lane 3 docs stack to `23df422`, kept the newly merged Lane
  1 and Lane 4 index entries alongside the Lane 3 entries, and passed
  `cargo xtask check-doc-index`, `cargo xtask markdown-links`,
  `cargo xtask check-traceability`, `cargo xtask check-spec-format`,
  `cargo xtask check-static-language`, `cargo xtask check-doc-roles`,
  `cargo xtask goals next`, and `git diff --check`. Treat #849 as an index
  compatibility concern, not a Lane 3 behavior dependency.
- after fetching current `origin/main` (`ad9c04e`), the Lane 1 evidence
  quality scorecard landed and touched `.ripr/traceability.toml` plus
  `docs/OUTPUT_SCHEMA.md`. The raw dirty diff no longer applies cleanly to
  current base. A docs-stack-only disposable port to `ad9c04e` resolved the
  expected `.ripr/traceability.toml`, ADR/proposal/spec README, and
  documentation-index conflicts by keeping the new Lane 1 scorecard entries
  plus the Lane 3 entries. Since `ad9c04e` adds traceability to generated
  scorecard reports, the port proof first ran `cargo xtask lane1-evidence-audit`
  and `cargo xtask evidence-quality-scorecard`, then passed
  `cargo xtask check-doc-index`, `cargo xtask markdown-links`,
  `cargo xtask check-traceability`, `cargo xtask check-spec-format`,
  `cargo xtask check-static-language`, `cargo xtask check-doc-roles`,
  `cargo xtask goals next`, `cargo xtask check-pr`,
  `cargo xtask lsp-cockpit-report`, and `git diff HEAD --check`. The actual
  main worktree is still behind `origin/main`, so package this stack from a
  current-base port or rebase before opening a PR.
- focused editor proof on that same current-base port also passed:
  `cargo test -p ripr lsp --lib`,
  `cargo test -p ripr lsp::tests --lib`,
  `npm --prefix editors/vscode run compile`, and
  `npm --prefix editors/vscode run test:e2e` with 30 passing tests. The VS Code
  e2e run still printed the known post-success `path` warning while exiting 0.
- after fetching current `origin/main` (`81765dd`), the Lane 1 evidence-quality
  benchmark corpus landed and touched `.ripr/traceability.toml` plus
  `xtask/src/main.rs`. A fresh current-base port kept those Lane 1 fixture
  validators and the Lane 3 `plans/` PR-summary classification together. Live
  GitHub still showed #771 open for `analysis/python-preview-adapter`, #772
  open for blocked `lsp/editor-language-routing`, #807 open for structured
  `static_limit_kind`, and #814 open for the existing policy-readiness consumer
  mismatch. `cargo xtask goals next` still reported no ready work items, so
  preview selector and routing work remains blocked.
- after #857 merged, `Finding.static_limit_kind` and JSON emission are on
  `main`, the TypeScript mocked-module preview limit emits the structured
  `mocked_module` value, and #807/#814 are closed. Live GitHub still shows #771
  open for `analysis/python-preview-adapter` and #772 open for blocked
  `lsp/editor-language-routing`; `cargo xtask goals next` still reports no
  ready work items, so preview selector and routing work remains blocked.
- after #868 refreshed the Campaign 27 Lane 3 routing plan for the post-#857
  `static_limit_kind` state, current `origin/main` (`61ea9c9`) was rechecked
  as a Rust cockpit maintenance smoke. `cargo xtask goals next` still reported
  no ready work items; `cargo xtask lsp-cockpit-report` passed;
  `cargo test -p ripr lsp --lib` passed 123 tests;
  `cargo test -p ripr lsp::tests --lib` passed 84 tests;
  `npm --prefix editors/vscode run compile` passed after restoring local
  `node_modules` with `npm --prefix editors/vscode ci`; and
  `npm --prefix editors/vscode run test:e2e` passed 30 live VS Code smoke
  tests with the known post-success VS Code runner `path` warning exiting 0.
  The open PR queue still had no `editors/vscode`,
  `crates/ripr/src/lsp`, or `fixtures/editor_lsp_workflow` changes, so Lane 3
  remains maintenance-only until Python unblocks `lsp/editor-language-routing`.
- after #865 merged the Lane 2 policy operations spec, current `origin/main`
  (`ad5f200`) was rechecked as a Rust cockpit maintenance proof. The merge
  did not touch editor/LSP projection files. `cargo test -p ripr lsp --lib`
  passed 123 tests, `cargo xtask lsp-cockpit-report` passed,
  `npm --prefix editors/vscode ci` completed in a disposable worktree,
  `npm --prefix editors/vscode run compile` passed, and
  `npm --prefix editors/vscode run test:e2e` passed 30 live VS Code smoke
  tests with the known post-success VS Code runner `path` warning exiting 0.
  `cargo xtask goals next` still reported no ready work items, #771 remained
  open for `analysis/python-preview-adapter`, and #772 remained open and
  blocked for `lsp/editor-language-routing`, so preview selectors and routing
  still must not start.
- after the Python preview corpus completion landed, `lsp/editor-language-routing`
  became the ready Campaign 27 work item. The routing slice adds VS Code
  activation and document selectors for TypeScript, TypeScript React,
  JavaScript, JavaScript React, and Python; preserves Rust saved-workspace
  defaults; routes stale-buffer guards through the same file-language set; and
  carries preview language/status/owner/static-limit metadata through LSP
  diagnostics, hover, and status without adding source edits, generated tests,
  provider calls, mutation execution, gate semantics, default blocking,
  CodeLens, inlay hints, semantic tokens, or unsaved-buffer overlays.

Objective audit status from 2026-05-13: routing is implemented and preview
projection proof is pinned for Campaign 27 closeout. Preview editor workflow
docs and closeout evidence landed without changing Rust saved-workspace
defaults, source edits, generated tests, provider calls, mutation execution,
policy/gate behavior, default blocking, CodeLens, inlay hints, semantic tokens,
or unsaved-buffer overlays.

| Requirement | Current artifact or command | Audit status |
| --- | --- | --- |
| Rust saved-workspace diagnostics, hover, actions, context packets, status, related-test opening, and copy commands remain stable | `cargo test -p ripr lsp --lib`, `cargo test -p ripr lsp::tests --lib`, `npm --prefix editors/vscode run test:e2e`, `fixtures/editor_lsp_workflow` | Current Rust cockpit checks pass |
| Editor behavior stays saved-workspace only and projection-only | `docs/EDITOR_EVIDENCE_UX.md`, `docs/EDITOR_EVIDENCE_WORKFLOW.md`, `editors/vscode/src/client.ts`, `fixtures/editor_lsp_workflow` | Current tracker evidence covers the saved-workspace path |
| Wrong-root, missing, malformed, and stale reports fail closed | `fixtures/editor_lsp_workflow`, `cargo xtask lsp-cockpit-report`, VS Code e2e status tests | Current cockpit report and e2e smoke cover these states |
| VS Code remains Rust-default until preview routing is selected | `editors/vscode/package.json`, `editors/vscode/src/client.ts` | Routing selected; Rust behavior remains the default while preview file selectors are registered |
| TypeScript preview adapter readiness includes editor-projectable preview metadata, static limits, owner matching, oracle precision, and fixture evidence | `.ripr/goals/active.toml`, #779, #780, #782, #785, #786 | Complete for current Campaign 27 routing readiness; #779, #780, #782, #785, and #786 are closed |
| Python preview adapter exists with editor-projectable preview metadata and static limits | `.ripr/goals/active.toml`, #804, Python owner/test/assertion/probe/related-test/static-limit fixtures | Complete for current Campaign 27 routing readiness; preview output is opt-in, labeled, fixture-backed, and uses structured `static_limit_kind` values |
| `lsp/editor-language-routing` is ready or selected | `cargo xtask goals next`, `.ripr/goals/active.toml` | Done; preview selectors landed behind `[languages]` while preserving Rust defaults |
| Preview selectors for TypeScript, TSX, JavaScript, JSX, and Python are opt-in and preserve Rust defaults | `editors/vscode/package.json`, `editors/vscode/src/client.ts` | Implemented in the routing slice; LSP analysis remains gated by `[languages]` |
| Preview diagnostics, hover, status, and actions visibly label preview evidence and static limits | LSP diagnostic data, hover markdown, status refresh logs, existing cockpit actions, `fixtures/python_missing_import_graph_limit`, `fixtures/typescript_mocked_module_limit`, `fixtures/python_disabled`, `cargo xtask lsp-cockpit-report` | Implemented for diagnostic metadata, hover boundary text, status preview/static-limit counts, bounded finding actions, and disabled-preview no-diagnostic behavior; actions keep the existing cockpit model |
| No editor hidden analysis reruns, source edits, generated tests, provider calls, mutation execution, gate semantics, default blocking, CodeLens, inlay hints, semantic tokens, or unsaved-buffer overlays | Lane 3 Scope, Non-Goals, and Cross-Lane Rules in this tracker | Current tracker preserves the boundary; future routing must re-audit it |

## Closed Slice: Editor Gap Cockpit

Campaign 27 closed language routing. The next Lane 3 slice, Editor Gap
Cockpit, is now closed. It made the editor consume existing RIPR evidence
artifacts and project one safe local next action.

Durable sources:

- [RIPR-PROP-0007: Editor Gap Cockpit](../proposals/RIPR-PROP-0007-editor-gap-cockpit.md)
- [RIPR-SPEC-0047: Editor Gap Projection](../specs/RIPR-SPEC-0047-editor-gap-projection.md)
- [ADR-0012: Editor Gap Projection Is Read-Only](../adr/0012-editor-gap-projection-is-read-only.md)
- [Editor gap cockpit implementation plan](../../plans/editor-gap-cockpit/implementation-plan.md)
- [Editor Gap Cockpit closeout](../handoffs/2026-05-15-editor-gap-cockpit-closeout.md)

The target loop is:

```text
diagnostic
-> hover evidence
-> gap state / preview limit / repair route
-> related test or repair packet
-> one focused test
-> verify
-> receipt
-> refresh
```

Lane 3 resolves editor surfaces through one identity path:

```text
diagnostic.data
-> canonical_gap_id / seam_id / finding_id
-> evidence_record or gap_record
-> repair route / related test / verify command
-> receipt
```

Other lanes provide evidence truth, gap records, repair routes, policy state,
PR/CI surfaces, and receipts. Lane 3 consumes those artifacts read-only. It
does not create analyzer facts, decide policy, publish PR comments, compose
generated CI summaries, edit source, generate tests, call providers, run
mutation, or act as a gate authority.

Completed PR path:

1. #967 `docs(lane3): open editor gap cockpit source-of-truth stack`
2. #970 `test(lsp): pin post-campaign editor contract`
3. #973 `lsp(gap): add read-only gap artifact validation`
4. #969 `lsp(gap): project gap records into editor diagnostics`
5. #976 `lsp(gap): project gap state in Show Status`
6. #981 `lsp(gap): enrich hover repair route`
7. #983 `lsp(gap): add bounded repair actions`
8. #985 `fixtures(editor): add gap cockpit workflow fixtures`
9. #993 `test(vscode): smoke editor gap cockpit`
10. #996 `docs(editor): document gap cockpit workflow`
11. #998 `dogfood(lane3): record editor gap cockpit receipts`
12. `campaign(lane3): close editor gap cockpit`

Post-closeout posture: there is no selected behavior-bearing Lane 3 PR open.
Do not reopen this slice for speculative editor furniture. Future first-run,
install, CodeLens, inlay, semantic-token, inline-patch, or unsaved-buffer work
needs a new editor campaign.

## Closed Slice: Editor First-Run and Repair Usability

Editor First-Run and Repair Usability closed after making the existing cockpit
self-orienting for a first-time or low-context user. It did not reopen Campaign
27 language routing or the Editor Gap Cockpit:

```text
install / open workspace
-> diagnose setup
-> understand no-output state
-> inspect one Rust gap
-> open related test or copy first repair packet
-> verify
-> emit receipt
-> refresh
```

Durable sources for the selected docs stack:

- [RIPR-PROP-0008: Editor First-Run Usability](../proposals/RIPR-PROP-0008-editor-first-run-usability.md)
- [RIPR-SPEC-0049: Editor Setup Status](../specs/RIPR-SPEC-0049-editor-setup-status.md)
- [RIPR-SPEC-0050: Editor First Repair Loop](../specs/RIPR-SPEC-0050-editor-first-repair-loop.md)
- [ADR-0013: Editor Setup Diagnostics Are Read-Only](../adr/0013-editor-setup-diagnostics-are-read-only.md)
- [Editor first-run usability implementation plan](../../plans/editor-first-run-usability/implementation-plan.md)
- [Editor First-Run Usability closeout](../handoffs/2026-05-16-editor-first-run-usability-closeout.md)

Completed PR path:

1. #1012 `docs(lane3): open editor first-run usability stack`
2. #1017 `vscode: add setup diagnosis status model`
3. #1023 `vscode: add ripr Diagnose Setup command`
4. #1026 `test(vscode): smoke first-run and no-output states`
5. #1028 `lsp: link receipts into Show Status`
6. #1030 `lsp: add first-repair action packet`
7. #1033 `fixtures(editor): add first-run usability fixtures`
8. #1037 `docs(editor): write first-run-to-first-receipt guide`
9. #1038 `dogfood(lane3): record first-run repair receipts`
10. `campaign(lane3): close editor first-run usability`

This campaign consumes existing server, workspace, config, language, artifact,
gap, command, and receipt state. It may explain setup and no-output states,
show existing receipt status, and expose bounded copy actions when typed fields
support them. It must not install binaries, mutate config, run hidden analysis,
create receipts, edit source, generate tests, call providers, run mutation,
publish PR comments, decide gates, or add CodeLens, inlays, semantic tokens,
inline patches, or unsaved-buffer overlays.

Post-closeout posture: there is no selected behavior-bearing Lane 3 PR open.
Future first-run/install polish, CodeLens, inlay, semantic-token, inline-patch,
source-edit, generated-test, provider, mutation, policy, PR-comment, or
unsaved-buffer work needs a new editor campaign.

## Closed Slice: Editor First-PR Bridge

Editor First-PR Bridge is closed. It carries the user from the closed
first-run repair loop into the existing first successful PR packet without
making the editor a PR/CI producer:

```text
Diagnose Setup
-> Show Status
-> inspect one diagnostic
-> copy first repair packet
-> verify
-> receipt
-> refresh
-> inspect first-pr packet
```

Durable sources for the selected docs stack:

- [RIPR-PROP-0010: Editor First-PR Bridge](../proposals/RIPR-PROP-0010-editor-first-pr-bridge.md)
- [RIPR-SPEC-0052: Editor First-PR Packet Projection](../specs/RIPR-SPEC-0052-editor-first-pr-packet-projection.md)
- [ADR-0014: Editor First-PR Projection Is Read-Only](../adr/0014-editor-first-pr-projection-is-read-only.md)
- [Editor First-PR Bridge implementation plan](../../plans/editor-first-pr-bridge/implementation-plan.md)
- [Editor First-PR Bridge closeout](../handoffs/2026-05-17-editor-first-pr-bridge-closeout.md)

The slice consumes existing first-pr packet artifacts:

```text
target/ripr/reports/start-here.{json,md}
target/ripr/first-pr/start-here.{json,md}
```

Lane 3 may validate and project first-pr packet state in Diagnose Setup, Show
Status, hover, fixtures, VS Code smoke, and bounded open/copy actions. It does
not create first-pr packets, compose generated CI summaries, publish PR
comments, decide policy or gates, claim merge readiness, edit source, generate
tests, call providers, run mutation, or add CodeLens, inlays, semantic tokens,
inline patches, or unsaved-buffer overlays.

Completed PR path:

1. #1098 `docs(lane3): open editor first-pr bridge source-of-truth stack`
2. #1100 `test(lsp): pin post-first-run editor contract`
3. #1103 `lsp(first-pr): validate first-pr packet artifacts`
4. #1104 `lsp(first-pr): project first-pr state in status`
5. #1108 `lsp(first-pr): add bounded packet actions`
6. #1110 `fixtures(editor): add first-pr bridge fixtures`
7. #1113 `test(vscode): smoke first-pr bridge`
8. #1115 `docs(editor): document first-pr bridge workflow`
9. #1116 `dogfood(lane3): record editor first-pr bridge receipts`
10. `campaign(lane3): close editor first-pr bridge`

Hard boundaries:

- saved-workspace first;
- read-only projection over existing artifacts;
- typed fields over prose;
- Rust default unchanged;
- preview evidence visibly bounded;
- static limits before action language;
- stale, wrong-root, malformed, missing, unsupported, disabled, unavailable,
  path-unsafe, and command-unsafe states fail closed;
- no first-pr packet producer changes;
- no PR comment publishing or generated CI summary composition;
- no source edits, generated tests, provider/model calls, mutation execution,
  policy/gate/default-blocking behavior, CodeLens, inlay hints, semantic
  tokens, inline patches, or unsaved-buffer overlays.

Post-closeout posture: there is no selected behavior-bearing Lane 3 PR open.
Future install polish, PR/CI publishing, source edits, generated tests,
provider/model calls, mutation execution, policy/gate changes, CodeLens,
inlay hints, semantic tokens, inline patches, or unsaved-buffer overlays need a
new editor campaign.

## Active Slice: Editor Adoption Assurance

Editor Adoption Assurance is the selected Lane 3 follow-up. It does not reopen
Editor Gap Cockpit, Editor First-Run and Repair Usability, or Editor First-PR
Bridge. It makes the closed editor path adoption-ready by improving diagnosis,
root clarity, fixture proof, e2e proof, docs, and receipts for external-style
first-use states.

Durable sources for the selected docs stack:

- [RIPR-PROP-0012: Editor Adoption Assurance](../proposals/RIPR-PROP-0012-editor-adoption-assurance.md)
- [RIPR-SPEC-0054: Editor Adoption Assurance](../specs/RIPR-SPEC-0054-editor-adoption-assurance.md)
- [ADR-0016: Editor Adoption Assurance Is Read-Only Diagnosis](../adr/0016-editor-adoption-assurance-is-read-only-diagnosis.md)
- [Editor Adoption Assurance implementation plan](../../plans/editor-adoption-assurance/implementation-plan.md)

The target adoption path is:

```text
install / open workspace
-> Diagnose Setup
-> Show Status
-> confirm server, root, language, artifact, first-pr, and receipt state
-> inspect one diagnostic or no-output state
-> take one bounded repair or regeneration action
-> verify / receipt / refresh
-> inspect first-pr packet when safe
```

Planned PR path:

1. #1245 `docs(lane3): open editor adoption assurance stack`
2. #1246 `test(lsp): pin editor adoption baseline`
3. #1247 `vscode: add extension/server compatibility diagnosis`
4. #1248 `vscode: harden workspace-root and multi-root diagnosis`
5. #1249 `fixtures(editor): add adoption-assurance fixture corpus`
6. #1250 `test(vscode): smoke editor adoption assurance path`
7. #1251 `docs(editor): write install-to-first-pr editor guide`
8. #1252 `dogfood(lane3): record external-style editor adoption receipts`
9. #1253 `campaign(lane3): close editor adoption assurance`

Hard boundaries:

- saved-workspace first;
- read-only diagnosis and projection;
- typed fields over prose;
- Rust default unchanged;
- preview evidence visibly bounded;
- static limits before action language;
- stale, wrong-root, malformed, missing, unsupported, disabled, unavailable,
  ambiguous, path-unsafe, and command-unsafe states fail closed;
- no analyzer truth changes;
- no first-pr packet producer changes;
- no generated CI summary composition or PR comment publishing;
- no policy, gate, baseline, suppression, or default-blocking behavior;
- no source edits, generated tests, provider/model calls, mutation execution,
  CodeLens, inlays, semantic tokens, inline patches, or unsaved-buffer
  overlays;
- no auto-install, binary download, config mutation, or hidden server
  replacement.

## Lane 3 Document Model

Lane 3 uses the repo tracking model in layers so future editor work does not
mix why, behavior contracts, architecture decisions, execution state, and
validation in one file:

- proposals in [`docs/proposals/`](../proposals/) explain why an editor slice
  exists, who benefits, alternatives, risks, and success criteria;
- specs in [`docs/specs/`](../specs/) define what the editor behavior must do
  for users, tests, fixtures, and future agents;
- ADRs in [`docs/adr/`](../adr/) record durable editor architecture decisions,
  including projection-only and read-only setup boundaries;
- campaign-specific plans under [`plans/`](../../plans/) define the PR
  sequence, acceptance, validation commands, and rollback notes for the Lane 3
  slice;
- `.ripr/goals/active.toml` records current machine-readable execution state
  only;
- this lane tracker records Lane 3 scope, readiness, blocker state, and
  maintenance evidence;
- closeout handoffs in [`docs/handoffs/`](../handoffs/) record what landed,
  what passed, what remains, and which future editor campaigns are still
  out of scope.

Use the next available proposal, spec, and ADR numbers from the indexes. Do not
reuse occupied IDs when turning the planned editor-preview routing documents
into concrete files.

The editor-preview routing proposal and specs currently use `RIPR-PROP-0003`,
`RIPR-SPEC-0036`, and `RIPR-SPEC-0037` because current `origin/main` already
owns `RIPR-PROP-0002`, `RIPR-SPEC-0034`, `RIPR-SPEC-0035`, and `ADR-0010`
for the Lane 1 evidence quality stack.

Traceability for future editor-preview specs should list the docs outputs that
define the contract and add tests, fixtures, code, and reports only as the
behavior PRs land. Do not point future preview-routing specs at the existing
Rust cockpit tests as if those tests prove a new preview-language behavior.

Traceability for future first-run usability specs follows the same rule:
source-of-truth PRs list the docs outputs that define the contract, while
setup-status tests, receipt-status tests, VS Code smoke tests, fixtures, and
metrics are added by the behavior PRs that implement them.

## Preview Routing Path

Lane 3 returns to maintenance after `lsp/editor-language-routing` closeout. The
lane's useful end state was not "more editor UI"; it is the existing Rust
saved-workspace cockpit plus opt-in preview-language projection that makes
syntax-first limits impossible to miss.

User-facing target:

```text
Rust stable cockpit remains boringly reliable
-> TypeScript/Python preview evidence becomes opt-in
-> editor projection makes preview limits obvious
-> users can act without over-trusting syntax-first evidence
```

Completed Campaign 27 PR path:

1. `docs(lane3): define editor preview routing source-of-truth stack`
   - Records where Lane 3 stores why, behavior contracts, architecture
     decisions, PR sequencing, current execution state, lane readiness, and
     final proof.
   - Does not change behavior, selectors, or `.ripr/goals/active.toml`.
2. `docs(proposal): add Lane 3 editor preview routing proposal`
   - Explains why preview evidence should appear in the existing cockpit
     without looking as mature as Rust evidence.
3. `docs(spec): add editor preview routing contract`
   - Defines selector, opt-in routing, Rust-default preservation, and
     fail-closed behavior.
4. `docs(spec): add preview static-limit projection contract`
   - Defines preview labels and the rule that static limits appear before
     suggested action language.
5. `docs(adr): editor preview routing is projection-only`
   - Records that the editor consumes existing artifacts and is not an
     analyzer, policy engine, generator, provider surface, source editor, or
     mutation runner.
6. `plans(c27): add Lane 3 editor preview routing implementation plan`
   - Defines PR sequence, acceptance, proof commands, and rollback per slice.
7. `analysis: close TypeScript editor readiness`
   - Done for current Campaign 27 routing readiness.
   - Closed #779, #780, #782, #785, and #786 without VS Code selector or LSP
     routing changes.
   - No VS Code selector or LSP routing changes belong in this work.
8. `analysis: complete Python preview adapter`
   - Done for current Campaign 27 routing readiness.
   - Python output carries `language = "python"`,
     `language_status = "preview"`, owner facts, test facts, assertion facts,
     probe facts, related-test facts, structured static limits, and
     fixture/golden coverage.
   - No editor selector work landed in this work.
9. `test(lsp): preserve Rust routing contract`
   - Pin `[languages]` absent, `["rust"]`, `[]`, and invalid-config behavior
     before adding preview selectors.
   - Rust diagnostics, hover, actions, and status must remain unchanged by
     default.
10. `lsp(language): add editor language routing`
   - Extend VS Code activation and selectors for `typescript`,
     `typescriptreact`, `javascript`, `javascriptreact`, and `python`.
   - Route saved-workspace diagnostics only when repo config enables that
     language.
   - Preserve wrong-root, stale, and malformed fail-closed behavior.
11. `lsp(language): surface preview labels and static limits`
   - Show language, preview status, static-limit kind/explanation, and the
     advisory boundary in hover/status before suggested action language.
   - Keep the same cockpit action model; do not invent preview-only action
     semantics.
12. `fixtures: add preview editor workflow fixtures`
   - Add explicit `rust_default`, `typescript_preview`, `python_preview`,
     `mixed_language_opt_in`, and `preview_disabled` editor fixtures.
   - Pin diagnostics, hover, code actions, status, and static-limit artifacts.
   - Current fixture proof pins the same editor projection contract through the
     existing preview fixtures `python_missing_import_graph_limit`,
     `typescript_mocked_module_limit`, and `python_disabled` so
     `lsp-cockpit-report` covers preview diagnostics, hover ordering, bounded
     actions, status, static limits, and disabled-preview no-diagnostic
     behavior before final closeout.
13. `test(vscode): smoke preview saved-workspace routing`
   - Prove the packaged extension path for Rust default behavior, opt-in
     TypeScript/Python preview diagnostics, hover preview/static-limit text,
     bounded actions, status, and disabled-preview no-diagnostic behavior.
14. `docs(editor): document preview language workflow`
   - Document Rust as stable/default and TypeScript/Python as opt-in preview.
   - Explain syntax-first evidence, static limits, advisory-only diagnostics,
     and the source-edit-free command loop.
15. `campaign(lane3): close editor preview routing`
    - Done as part of Campaign 27 closeout after Rust defaults stayed unchanged,
      preview routing remained opt-in and fixture-pinned, preview labels/static
      limits were visible, VS Code e2e and `lsp-cockpit-report` checked the
      path, and docs covered the preview limits.

Hard boundaries for every slice:

- saved-workspace only;
- projection-only;
- Rust default unchanged;
- preview languages opt-in only;
- preview findings labeled preview;
- static limits visible before suggested action language;
- wrong-root, stale, and malformed artifacts fail closed;
- no source edits;
- no generated tests;
- no provider calls;
- no mutation execution;
- no policy, gate, or default-blocking behavior;
- no CodeLens, inlay hint, semantic token, or unsaved-buffer overlay work unless
  a later editor campaign explicitly opens that scope.

## Validation Gates

Docs-only tracker changes should run:

```bash
cargo xtask check-doc-index
cargo xtask markdown-links
cargo xtask check-static-language
cargo xtask check-pr
git diff --check
```

Behavior changes should add the relevant editor checks:

```bash
cargo test -p ripr lsp --lib
cargo test -p ripr lsp::tests --lib
cargo xtask lsp-cockpit-report
npm --prefix editors/vscode run compile
npm --prefix editors/vscode run test:e2e
cargo xtask check-output-contracts
cargo xtask check-static-language
cargo xtask check-traceability
cargo xtask check-capabilities
cargo xtask check-doc-index
cargo xtask markdown-links
cargo xtask check-pr
git diff --check
```

## Cross-Lane Rules

- `.ripr/goals/active.toml` is the current Codex Goals manifest, not the whole
  product board. Its top-level status may be `closed` after campaign closeout
  until a successor campaign is selected.
- Campaign 24 PR Review Front Panel is a PR/CI composition lane. It explicitly
  excludes editor behavior changes.
- Lane 3 may project existing first-action or front-panel artifacts in editor
  status only when that work is selected as editor/LSP scope.
- Lane 3 must not take PR/CI dogfood receipts, campaign closeouts, baseline
  ledgers, gate policy, evidence schema, release, security, or platform work.
- Cross-lane artifacts should be read-only inputs in the editor unless a later
  editor campaign explicitly changes that contract.

## Non-Goals

Lane 3 does not own:

- PR Review Front Panel producer, docs, dogfood, or closeout;
- Campaign 22 or Campaign 24 end-to-end work;
- analyzer behavior;
- evidence-record schema design;
- baseline ledger behavior;
- policy or gate semantics;
- generated CI behavior;
- SARIF or badge output;
- release, packaging, or security workflow mechanics;
- source edits;
- generated tests;
- provider or model calls;
- runtime mutation execution;
- runtime adequacy claims.

Deferred editor features remain out of scope until a new editor campaign opens:

- unsaved-buffer overlays;
- CodeLens;
- inlay hints;
- semantic tokens;
- inline patch application;
- automatic test generation;
- automatic source edits;
- policy or gate editing from the editor.

## Operating Rule

Before taking a Lane 3 task, confirm it touches editor or LSP projection. If it
is about PR/CI summary composition, dogfood receipts, policy, evidence schema,
or campaign closeout outside editor behavior, route it to the owning lane.
