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
  refresh guidance, and LSP cockpit coverage.

## Current Open PRs

There are no behavior-bearing Lane 3 PRs open.

When opening future Lane 3 PRs, list them here until they merge or close:

| PR | Slice | State | Notes |
| --- | --- | --- | --- |
| none | - | - | - |

## Upcoming Dependency

Campaign 27 Language Adapter Preview has one expected Lane 3 slice:
`lsp/editor-language-routing`.

That slice is blocked until both TypeScript and Python preview adapters exist.
Lane 3 should review upstream analyzer, config, and output work only as a
consumer of future editor projection inputs. Rust saved-workspace editor
behavior must stay unchanged while this dependency is prepared.

Readiness boundary:

- preview adapters must be opt-in;
- preview evidence must be labeled preview;
- static limits must be visible;
- saved-workspace analysis must route through the adapter layer only when the
  selected language is enabled;
- Rust defaults, gate semantics, generated CI, and Rust LSP/editor behavior must
  not change.

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
