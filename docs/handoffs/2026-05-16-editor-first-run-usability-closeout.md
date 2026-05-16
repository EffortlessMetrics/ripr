# Handoff: Editor First-Run Usability Closeout

Date: 2026-05-16

Branch / PR: `lane3-first-run-usability-closeout` / pending at authoring

Latest merged PR: #1038 `dogfood(lane3): record first-run repair receipts`
(commit `e8088c37cc68b375f4dd35bb3ac27187fd6cce0f`)

## Current Work Item

`campaign(lane3): close editor first-run usability`

Editor First-Run and Repair Usability made the existing saved-workspace editor
cockpit self-orienting for a first-time or low-context user:

```text
install/open
-> diagnose setup
-> understand no-output state
-> inspect one Rust gap
-> open related test or copy first repair packet
-> verify
-> emit receipt
-> refresh
```

The campaign stayed read-only and projection-only. It did not add analyzer
truth, policy or gate decisions, PR comments, generated CI summaries, source
edits, generated tests, provider calls, mutation execution, default blocking,
CodeLens, inlays, semantic tokens, inline patches, unsaved-buffer overlays,
binary installation, or config mutation from the editor.

## Prompt-To-Artifact Audit

| Requirement | Artifact | Validation |
| --- | --- | --- |
| Source-of-truth stack exists | #1012 added `RIPR-PROP-0008`, `RIPR-SPEC-0049`, `RIPR-SPEC-0050`, ADR 0013, the implementation plan, lane tracker state, indexes, and traceability. | `cargo xtask check-doc-index`, `cargo xtask markdown-links`, `cargo xtask check-doc-roles`, `cargo xtask check-traceability` |
| Setup diagnosis status model exists | #1017 models server, workspace, config, language, artifact, freshness, and next safe action state. | VS Code status tests and `cargo xtask check-pr` |
| `ripr: Diagnose Setup` exists | #1023 exposes a read-only command-palette setup report for server, binary, workspace, config, language, artifacts, freshness, and next step. | `npm --prefix editors/vscode run compile`, `npm --prefix editors/vscode run test:e2e` |
| First-run and no-output states are smoke-tested | #1026 covers no workspace, server unavailable, server available, missing config, Rust default, preview disabled, adapter unavailable, stale evidence, no actionable gap, and actionable gap. | VS Code e2e and `cargo test -p ripr lsp --lib` |
| Receipt state is visible in Show Status | #1028 consumes existing receipt artifacts and reports found, missing, stale, gap mismatch, movement improved, and movement unchanged states without producing receipts. | LSP tests and `cargo xtask lsp-cockpit-report` |
| First repair packet action is bounded | #1030 adds `Copy first repair packet` only when gap identity, repair route, verify command, and receipt command are valid. | LSP action tests, VS Code copy tests, and `cargo xtask check-static-language` |
| First-run usability fixtures are pinned | #1033 adds `fixtures/editor_first_run_usability` for setup, server missing, config missing, language disabled, adapter unavailable, artifact missing, artifact stale, receipt improved, and receipt unchanged states. | `cargo xtask check-fixture-contracts`, `cargo xtask check-pr`, Codecov patch |
| First-run-to-first-receipt guide exists | #1037 adds `docs/EDITOR_FIRST_RUN_TO_FIRST_RECEIPT.md` and links it from editor docs and indexes. | `cargo xtask check-doc-index`, `cargo xtask markdown-links`, `cargo xtask check-static-language` |
| Dogfood receipts record the loop | #1038 records setup diagnosis, safe partial pilot packets, first actionable Rust gap fixture, verify artifact, receipt artifact, refresh/cockpit check, and no-action state. | `cargo run -p ripr -- doctor --root .`, `cargo run -p ripr -- pilot ...`, `cargo xtask lsp-cockpit-report`, `cargo xtask dogfood` |

## PR Chain

- #1012 `docs(lane3): open editor first-run usability stack`
- #1017 `vscode: add setup diagnosis status model`
- #1023 `vscode: add Diagnose Setup command`
- #1026 `test(vscode): smoke first-run and no-output states`
- #1028 `lsp: link receipts into Show Status`
- #1030 `lsp: add first-repair action packet`
- #1033 `fixtures(editor): add first-run usability fixtures`
- #1037 `docs(editor): write first-run-to-first-receipt guide`
- #1038 `dogfood(lane3): record first-run repair receipts`
- `campaign(lane3): close editor first-run usability`

## Closeout Validation

Closeout validation for this PR:

```bash
cargo xtask goals next
cargo xtask lsp-cockpit-report
cargo test -p ripr lsp --lib
cargo test -p ripr lsp::tests --lib
npm --prefix editors/vscode ci
npm --prefix editors/vscode run compile
npm --prefix editors/vscode run test:e2e
cargo xtask check-output-contracts
cargo xtask check-static-language
cargo xtask check-traceability
cargo xtask check-capabilities
cargo xtask check-doc-index
cargo xtask markdown-links
cargo xtask check-doc-roles
cargo xtask check-pr
git diff --check
```

Result: pass before PR creation on 2026-05-16.

The final dogfood receipt PR also passed:

```bash
cargo run -p ripr -- doctor --root .
cargo run -p ripr -- pilot --root . --out target/ripr/pilot
cargo run -p ripr -- pilot --root . --out target/ripr/pilot --mode draft --max-seams 5 --timeout-ms 120000
cargo xtask lsp-cockpit-report
cargo xtask dogfood
cargo xtask check-doc-index
cargo xtask markdown-links
cargo xtask check-traceability
cargo xtask check-static-language
cargo xtask check-doc-roles
cargo xtask check-pr
git diff --check
```

## Remaining Limits

- Rust remains the stable default editor path.
- TypeScript, JavaScript, and Python remain opt-in preview evidence.
- Preview evidence remains syntax-first, advisory, and static-limit bounded.
- Static evidence does not imply runtime adequacy.
- Receipts record static artifact relationships, not mutation proof.
- Missing, stale, wrong-root, malformed, disabled, unavailable, and unsupported
  states fail closed.
- `ripr pilot` can still produce a safe partial packet on large workspaces when
  the analysis budget is too small; that is a first-run recovery state, not a
  repair action.
- Policy, gates, PR/CI summaries, PR comments, receipts, and analyzer truth
  remain owned by their lanes.

## Artifacts

- `docs/EDITOR_FIRST_RUN_TO_FIRST_RECEIPT.md`
- `docs/handoffs/2026-05-16-lane3-first-run-repair-receipts.md`
- `fixtures/editor_first_run_usability/`
- `target/ripr/pilot/pilot-summary.json`
- `target/ripr/pilot/pilot-summary.md`
- `target/ripr/reports/dogfood.json`
- `target/ripr/reports/dogfood.md`

## Next Work Item

No behavior-bearing Lane 3 work item is selected after this closeout. Lane 3
returns to maintenance and should review editor-impacting upstream changes
without opening speculative editor furniture.

Future first-run/install polish, CodeLens, inlay hints, semantic tokens,
inline patch application, automatic source edits, generated tests, provider
calls, mutation execution, policy editing, PR comment publishing, or
unsaved-buffer overlays require a new editor campaign.

## What Not To Do

- Do not reopen first-run usability to add extra UI surfaces.
- Do not make setup diagnosis install binaries or mutate config.
- Do not make the editor produce receipts.
- Do not infer receipt state by parsing prose.
- Do not expose repair actions for stale, wrong-root, malformed, disabled,
  unavailable, unsupported, or missing evidence except refresh/status.
- Do not promote preview evidence to Rust-level confidence.
- Do not add source edits, generated tests, provider calls, mutation execution,
  policy decisions, gate behavior, PR comments, generated CI composition,
  CodeLens, inlays, semantic tokens, inline patches, or unsaved-buffer overlays
  to Lane 3.
