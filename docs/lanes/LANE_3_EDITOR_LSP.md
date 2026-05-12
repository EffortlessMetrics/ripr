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

## Current Open PRs

There are no behavior-bearing Lane 3 PRs open.

When opening future Lane 3 PRs, list them here until they merge or close:

| PR | Slice | State | Notes |
| --- | --- | --- | --- |
| none | - | - | - |

## Upcoming Dependency

Campaign 27 Language Adapter Preview has one expected Lane 3 slice:
`lsp/editor-language-routing` (#772).

That slice is blocked until both TypeScript and Python preview adapters exist.
Lane 3 should review upstream analyzer, config, and output work only as a
consumer of future editor projection inputs. Rust saved-workspace editor
behavior must stay unchanged while this dependency is prepared.

Current dependency state:

- the TypeScript owner+test, assertion-shape, initial probe-shape, and
  mocked-module static-limit sub-slices have landed (#777, #781, #784, #791),
  and #794 marked `analysis/typescript-preview-adapter` done as a first useful
  preview loop;
- `analysis/typescript-editor-readiness` now keeps the editor-impacting
  TypeScript follow-ups explicit: visible preview metadata in human output
  (#779), file-first owner matching (#780), broad `toThrow()` handling (#782),
  awaited `Promise.reject(...)` error-path handling (#785), and
  fixture-per-probe-family evidence (#786) before editor projection can be
  treated as ready;
- assertion-shape extraction landed in #781, with a Lane 3 watchpoint that
  broad `toThrow()` assertions must not be surfaced as exact error-variant
  evidence;
- issue #779 tracks the landed human-output gap where TypeScript JSON carries
  `language_status = "preview"` but the human report does not visibly label the
  finding as preview TypeScript evidence;
- issue #780 tracks the landed owner-matching gap where TypeScript changed
  lines are matched by line range before file identity, which can attach the
  wrong owner and related-test evidence in mixed-file workspaces;
- the Python parser substrate ADR (#770) landed in #794, was corrected to
  `rustpython-parser` in #801, and the Python scaffold landed in #804; the
  Python preview adapter (#771) remains the next Python-side dependency before
  `lsp/editor-language-routing` because owner, test, assertion, probe, and
  static-limit extraction are still incomplete;
- issue #772 now records the current VS Code routing files:
  `editors/vscode/package.json` for activation and
  `editors/vscode/src/client.ts` for `documentSelector` plus
  `isRustFileDocument`;
- issue #771 now records the Python-to-editor handoff contract: Python preview
  artifacts need `language = "python"`, `language_status = "preview"`, and
  projectable static limits before Lane 3 can safely add Python selectors;
- issue #807 tracks the optional structured `static_limit_kind` field. Lane 3
  should prefer that field for future hover/status projection when it exists,
  but must still show explicit static-limit text before suggested action
  language if a preview adapter has not promoted its limit evidence to the
  structured field yet.
- issue #814 records that the policy-readiness scanner already looks for
  `static_limit_kind` even though findings do not emit it yet. Lane 3 should
  treat that as a concrete consumer signal for promoting #807 before any editor
  behavior branches on static-limit kind; until then, hover/status projection
  may only display inspected stable static-limit text.

Before starting `lsp/editor-language-routing`, refresh this audit instead of
inferring readiness from campaign momentum:

- `.ripr/goals/active.toml` must show both
  `analysis/typescript-editor-readiness` and `analysis/python-preview-adapter`
  complete, or `cargo xtask goals next` must list
  `lsp/editor-language-routing` as ready;
- TypeScript and Python preview outputs must visibly carry preview language
  metadata and explicit static limits in artifacts the editor can project;
- if `static_limit_kind` (#807) has landed by then, hover/status should consume
  it; otherwise the routing slice must inspect the preview artifacts and prove
  the text static-limit evidence is stable enough to project;
- if #814 remains open, do not add editor behavior that depends on
  text-parsing static-limit kinds; render the limit text as evidence only, and
  keep action semantics independent of the parsed kind;
- the TypeScript gaps tracked by #779, #780, #782, #785, and #786 must be closed
  or superseded by inspected artifacts with equivalent coverage;
- `editors/vscode/package.json` and `editors/vscode/src/client.ts` should remain
  Rust-only until the routing slice deliberately adds preview selectors behind
  opt-in configuration.

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
  this tracker keeps `analysis/typescript-editor-readiness` as the explicit
  TypeScript-side blocker, treats `analysis/python-preview-adapter` as active
  upstream work, and keeps `lsp/editor-language-routing` blocked until that
  adapter is complete;
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
- after #809 merged the preview-routing path, the saved-workspace Rust editor
  cockpit was rechecked on current `main`: `cargo test -p ripr lsp --lib`
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
  unresolved preview-readiness gaps in #779, #780, #782, #785, and #786, so the
  routing slice remains blocked.
- after #821 merged the static-limit consumer watchpoint, current `main`
  (`58709f7`) rechecked the Rust saved-workspace editor cockpit:
  `cargo xtask goals next` reported no ready work items,
  `cargo xtask lsp-cockpit-report` passed, `cargo test -p ripr lsp --lib`
  passed 123 tests, `cargo test -p ripr lsp::tests --lib` passed 84 tests,
  `npm --prefix editors/vscode run compile` passed, and
  `npm --prefix editors/vscode run test:e2e` passed 30 live VS Code smoke
  tests; the known VS Code runner `path` warning still appears after the
  passing e2e run and exits 0.

Objective audit status from 2026-05-12: not complete, blocked upstream.

| Requirement | Current artifact or command | Audit status |
| --- | --- | --- |
| Rust saved-workspace diagnostics, hover, actions, context packets, status, related-test opening, and copy commands remain stable | `cargo test -p ripr lsp --lib`, `cargo test -p ripr lsp::tests --lib`, `npm --prefix editors/vscode run test:e2e`, `fixtures/editor_lsp_workflow` | Current Rust cockpit checks pass |
| Editor behavior stays saved-workspace only and projection-only | `docs/EDITOR_EVIDENCE_UX.md`, `docs/EDITOR_EVIDENCE_WORKFLOW.md`, `editors/vscode/src/client.ts`, `fixtures/editor_lsp_workflow` | Current tracker evidence covers the saved-workspace path |
| Wrong-root, missing, malformed, and stale reports fail closed | `fixtures/editor_lsp_workflow`, `cargo xtask lsp-cockpit-report`, VS Code e2e status tests | Current cockpit report and e2e smoke cover these states |
| VS Code remains Rust-default until preview routing is selected | `editors/vscode/package.json`, `editors/vscode/src/client.ts` | Current extension activation and selector remain Rust-only |
| TypeScript preview adapter readiness includes editor-projectable preview metadata, static limits, owner matching, oracle precision, and fixture evidence | `.ripr/goals/active.toml`, #779, #780, #782, #785, #786 | Incomplete; first useful TypeScript preview loop is done, but `analysis/typescript-editor-readiness` blocks routing until the open editor-readiness gaps close or are explicitly superseded |
| Python preview adapter exists with editor-projectable preview metadata and static limits | `.ripr/goals/active.toml`, #804 | Partial; scaffold is merged and `analysis/python-preview-adapter` is active, but owner, test, assertion, probe, and static-limit extraction remain incomplete |
| `lsp/editor-language-routing` is ready or selected | `cargo xtask goals next`, `.ripr/goals/active.toml` | Blocked; no ready work items, with `analysis/typescript-editor-readiness` blocked and `analysis/python-preview-adapter` active/incomplete |
| Preview selectors for TypeScript, TSX, JavaScript, JSX, and Python are opt-in and preserve Rust defaults | `editors/vscode/package.json`, `editors/vscode/src/client.ts` | Not started; must wait for both preview adapters |
| Preview diagnostics, hover, status, and actions visibly label preview evidence and static limits | Future `lsp/editor-language-routing` artifacts and editor workflow fixtures | Not started; blocked by adapter outputs |
| No editor hidden analysis reruns, source edits, generated tests, provider calls, mutation execution, gate semantics, default blocking, CodeLens, inlay hints, semantic tokens, or unsaved-buffer overlays | Lane 3 Scope, Non-Goals, and Cross-Lane Rules in this tracker | Current tracker preserves the boundary; future routing must re-audit it |

## Preview Routing Path

Lane 3 stays in maintenance until `lsp/editor-language-routing` is unblocked.
The lane's useful end state is not "more editor UI"; it is the existing Rust
saved-workspace cockpit plus opt-in preview-language projection that makes
syntax-first limits impossible to miss.

User-facing target:

```text
Rust stable cockpit remains boringly reliable
-> TypeScript/Python preview evidence becomes opt-in
-> editor projection makes preview limits obvious
-> users can act without over-trusting syntax-first evidence
```

Planned PR path:

1. `analysis: close TypeScript editor readiness`
   - Not owned by Lane 3, but Lane 3 should review it as a hard dependency.
   - Must resolve or explicitly supersede #779, #780, #782, #785, and #786.
   - No VS Code selector or LSP routing changes belong in this work.
2. `analysis: complete Python preview adapter`
   - Not owned by Lane 3, but Lane 3 should review editor-projectability.
   - Python output must carry `language = "python"`,
     `language_status = "preview"`, owner facts, test facts, assertion facts,
     probe facts, related-test facts, static limits, and fixture/golden
     coverage.
   - No editor selector work belongs in this work.
3. `test(lsp): preserve Rust routing contract`
   - Pin `[languages]` absent, `["rust"]`, `[]`, and invalid-config behavior
     before adding preview selectors.
   - Rust diagnostics, hover, actions, and status must remain unchanged by
     default.
4. `lsp(language): add editor language routing`
   - Extend VS Code activation and selectors for `typescript`,
     `typescriptreact`, `javascript`, `javascriptreact`, and `python`.
   - Route saved-workspace diagnostics only when repo config enables that
     language.
   - Preserve wrong-root, stale, and malformed fail-closed behavior.
5. `lsp(language): surface preview labels and static limits`
   - Show language, preview status, static-limit kind/explanation, and the
     advisory boundary in hover/status before suggested action language.
   - Keep the same cockpit action model; do not invent preview-only action
     semantics.
6. `fixtures: add preview editor workflow fixtures`
   - Add explicit `rust_default`, `typescript_preview`, `python_preview`,
     `mixed_language_opt_in`, and `preview_disabled` editor fixtures.
   - Pin diagnostics, hover, code actions, status, and static-limit artifacts.
7. `test(vscode): smoke preview saved-workspace routing`
   - Prove the packaged extension path for Rust default behavior, opt-in
     TypeScript/Python preview diagnostics, hover preview/static-limit text,
     bounded actions, status, and disabled-preview no-diagnostic behavior.
8. `docs(editor): document preview language workflow`
   - Document Rust as stable/default and TypeScript/Python as opt-in preview.
   - Explain syntax-first evidence, static limits, advisory-only diagnostics,
     and the source-edit-free command loop.
9. `campaign(lane3): close editor preview routing`
   - Close only after Rust defaults are unchanged, preview routing is opt-in and
     fixture-pinned, preview labels/static limits are visible, VS Code e2e and
     `lsp-cockpit-report` prove the path, and docs cover the preview limits.

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

- `.ripr/goals/active.toml` is the active Codex Goals manifest, not the whole
  product board.
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
