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
  diagnostics instead of inventing editor behavior, and invalid language config
  stays config-owned while the LSP session falls back to Rust defaults.

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
  but TypeScript preview adapter work remains active;
- TypeScript follow-ups still need visible preview metadata in human output
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
- the Python preview adapter (#771) is still absent and blocked by the Python
  parser substrate ADR (#770), so `lsp/editor-language-routing` remains blocked
  even after TypeScript follow-ups land.
- issue #772 now records the current VS Code routing files:
  `editors/vscode/package.json` for activation and
  `editors/vscode/src/client.ts` for `documentSelector` plus
  `isRustFileDocument`;
- issue #771 now records the Python-to-editor handoff contract: Python preview
  artifacts need `language = "python"`, `language_status = "preview"`, and
  projectable static limits before Lane 3 can safely add Python selectors.

Before starting `lsp/editor-language-routing`, refresh this audit instead of
inferring readiness from campaign momentum:

- `.ripr/goals/active.toml` must show both `analysis/typescript-preview-adapter`
  and `analysis/python-preview-adapter` complete, or `cargo xtask goals next`
  must list `lsp/editor-language-routing` as ready;
- TypeScript and Python preview outputs must visibly carry preview language
  metadata and explicit static limits in artifacts the editor can project;
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

- `cargo xtask goals next` reported no ready work items, and
  `.ripr/goals/active.toml` still marks `analysis/typescript-preview-adapter`
  active, `analysis/python-preview-adapter` blocked, and
  `lsp/editor-language-routing` blocked;
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
- docs-only tracker validation passed with `cargo xtask check-doc-index`,
  `cargo xtask markdown-links`, `cargo xtask check-static-language`,
  `cargo xtask check-pr`, and `git diff --check`.
- later refreshes found #784 merged for #768 probe-shape refinement and #791
  merged for #769 mocked-module static-limit reporting; Lane 3 review captured
  unresolved preview-readiness gaps in #779, #780, #782, #785, and #786, so the
  routing slice remains blocked.

Objective audit status from 2026-05-12: not complete, blocked upstream.

| Requirement | Current artifact or command | Audit status |
| --- | --- | --- |
| Rust saved-workspace diagnostics, hover, actions, context packets, status, related-test opening, and copy commands remain stable | `cargo test -p ripr lsp --lib`, `cargo test -p ripr lsp::tests --lib`, `npm --prefix editors/vscode run test:e2e`, `fixtures/editor_lsp_workflow` | Current Rust cockpit checks pass |
| Editor behavior stays saved-workspace only and projection-only | `docs/EDITOR_EVIDENCE_UX.md`, `docs/EDITOR_EVIDENCE_WORKFLOW.md`, `editors/vscode/src/client.ts`, `fixtures/editor_lsp_workflow` | Current tracker evidence covers the saved-workspace path |
| Wrong-root, missing, malformed, and stale reports fail closed | `fixtures/editor_lsp_workflow`, `cargo xtask lsp-cockpit-report`, VS Code e2e status tests | Current cockpit report and e2e smoke cover these states |
| VS Code remains Rust-default until preview routing is selected | `editors/vscode/package.json`, `editors/vscode/src/client.ts` | Current extension activation and selector remain Rust-only |
| TypeScript preview adapter readiness includes editor-projectable preview metadata, static limits, owner matching, oracle precision, and fixture evidence | `.ripr/goals/active.toml`, #779, #780, #782, #785, #786 | Incomplete; #769 static-limit reporting landed in #791, but TypeScript preview adapter work remains active |
| Python preview adapter exists with editor-projectable preview metadata and static limits | `.ripr/goals/active.toml` | Missing; Python preview adapter remains blocked |
| `lsp/editor-language-routing` is ready or selected | `cargo xtask goals next`, `.ripr/goals/active.toml` | Blocked; no ready work items |
| Preview selectors for TypeScript, TSX, JavaScript, JSX, and Python are opt-in and preserve Rust defaults | `editors/vscode/package.json`, `editors/vscode/src/client.ts` | Not started; must wait for both preview adapters |
| Preview diagnostics, hover, status, and actions visibly label preview evidence and static limits | Future `lsp/editor-language-routing` artifacts and editor workflow fixtures | Not started; blocked by adapter outputs |
| No editor hidden analysis reruns, source edits, generated tests, provider calls, mutation execution, gate semantics, default blocking, CodeLens, inlay hints, semantic tokens, or unsaved-buffer overlays | Lane 3 Scope, Non-Goals, and Cross-Lane Rules in this tracker | Current tracker preserves the boundary; future routing must re-audit it |

## Next Slices

Lane 3 is in maintenance for the saved-workspace editor cockpit. Open a new
slice only when it is explicitly selected as editor/LSP projection work.
Likely future slices are incremental hardening, not campaign catch-up:

1. `lsp/editor-language-routing` after TypeScript and Python preview adapters
   land;
2. status, command, hover, or fixture updates required by a selected editor
   behavior change;
3. user-facing editor docs updates when a later behavior change requires them.

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
