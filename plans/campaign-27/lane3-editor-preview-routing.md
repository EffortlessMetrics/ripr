# Lane 3 Editor Preview Routing Plan

Status: implemented for Campaign 27 routing slice

Blocked by: none

Campaign: Campaign 27, Language Adapter Preview

Lane: Lane 3 - Editor / LSP UX

Linked proposal:

- [RIPR-PROP-0003: Editor Preview Routing](../../docs/proposals/RIPR-PROP-0003-editor-preview-routing.md)

Linked specs:

- [RIPR-SPEC-0036: Editor Preview Routing](../../docs/specs/RIPR-SPEC-0036-editor-preview-routing.md)
- [RIPR-SPEC-0037: Editor Preview Static-Limit Projection](../../docs/specs/RIPR-SPEC-0037-editor-preview-static-limit-projection.md)

`RIPR-PROP-0002`, `RIPR-SPEC-0034`, `RIPR-SPEC-0035`, and `ADR-0010` are
intentionally skipped here because current `origin/main` owns them for the
Lane 1 evidence quality stack.

Linked ADR:

- [ADR-0011: Editor Preview Routing Is Projection-Only](../../docs/adr/0011-editor-preview-routing-is-projection-only.md)

## Current State

Lane 3's saved-workspace Rust editor cockpit is complete and maintenance-only.
TypeScript editor readiness is complete for current Campaign 27 routing
readiness. Python preview adapter work is complete for current Campaign 27
routing readiness, so `lsp/editor-language-routing` has landed as the editor
projection slice.

The routing slice registers VS Code preview selectors for TypeScript,
TypeScript React, JavaScript, JavaScript React, and Python; keeps Rust default
behavior unchanged; routes stale-buffer guards through the supported-language
set; and surfaces preview language/status/static-limit metadata in diagnostic
data, hover, and status. Generated CI grouping remains the next Campaign 27
work item.

## Prompt-To-Artifact Audit

Audit date: 2026-05-13

Objective restated as deliverables:

- keep the existing Rust saved-workspace editor cockpit stable and fail-closed;
- add preview-language editor routing only after explicit Lane 3 LSP/editor
  work is selected;
- make TypeScript-family and Python preview routing opt-in and preserve Rust
  defaults;
- label preview evidence and show static limits before action language;
- prove preview routing through LSP tests, VS Code e2e, editor workflow
  fixtures, and `cargo xtask lsp-cockpit-report`;
- keep the editor projection-only, with no analyzer, policy, provider,
  generator, mutation, gate, or extra UI-surface expansion.

| Requirement | Current artifact or evidence | Coverage status | Missing or next action |
| --- | --- | --- | --- |
| Rust diagnostics, hover, actions, context packets, status, Show Status, related-test opening, and packet/brief/after-snapshot/verify/receipt commands remain stable | [Lane 3 tracker](../../docs/lanes/LANE_3_EDITOR_LSP.md), `fixtures/editor_lsp_workflow`, `cargo test -p ripr lsp --lib`, `cargo test -p ripr lsp::tests --lib`, `npm --prefix editors/vscode run test:e2e`, `cargo xtask lsp-cockpit-report` | Covered for the current Rust cockpit; latest tracker records passing maintenance evidence | Behavior PRs must rerun these checks after any routing change |
| Saved-workspace only and projection-only editor architecture | [ADR-0011](../../docs/adr/0011-editor-preview-routing-is-projection-only.md), [RIPR-SPEC-0036](../../docs/specs/RIPR-SPEC-0036-editor-preview-routing.md), [RIPR-SPEC-0037](../../docs/specs/RIPR-SPEC-0037-editor-preview-static-limit-projection.md) | Covered as a planned architecture and behavior contract | Future implementation diffs must prove they only consume existing artifacts |
| Fail-closed behavior for stale, malformed, missing, unsupported, and wrong-workspace evidence | [Lane 3 tracker](../../docs/lanes/LANE_3_EDITOR_LSP.md), `fixtures/editor_lsp_workflow`, `cargo xtask lsp-cockpit-report` | Covered for the current Rust saved-workspace cockpit | Preview fixtures and VS Code e2e must add matching preview-language cases |
| TypeScript editor readiness is safe for projection | `.ripr/goals/active.toml`, Campaign 27 ledger, closed #779, #780, #782, #785, and #786 | Complete for current Campaign 27 routing readiness | Re-audit only if a new editor-facing TypeScript regression appears |
| Python preview adapter emits editor-projectable preview facts | `.ripr/goals/active.toml`, #771, Campaign 27 ledger | Complete for current Campaign 27 routing readiness | Re-audit only if a new editor-facing Python regression appears |
| `lsp/editor-language-routing` is selected or ready | `.ripr/goals/active.toml`, #772, `cargo xtask goals next` | Selected and implemented | CI grouping is the next Campaign 27 work item |
| TypeScript, TSX, JavaScript, JSX, and Python selectors are opt-in and Rust defaults are preserved | [RIPR-SPEC-0036](../../docs/specs/RIPR-SPEC-0036-editor-preview-routing.md), this plan's routing work item | Implemented in VS Code activation and document selector configuration | Recheck compile/e2e after selector changes |
| Preview diagnostics, hover, status, and actions label preview evidence | [RIPR-SPEC-0036](../../docs/specs/RIPR-SPEC-0036-editor-preview-routing.md), [RIPR-SPEC-0037](../../docs/specs/RIPR-SPEC-0037-editor-preview-static-limit-projection.md) | Implemented for diagnostic metadata, hover boundary text, status refresh counts, and existing cockpit actions | Future fixture work may add richer mixed-language editor workflow packets |
| Static limits appear before suggested action language | [RIPR-SPEC-0037](../../docs/specs/RIPR-SPEC-0037-editor-preview-static-limit-projection.md), #807, #814 | Implemented for structured `static_limit_kind` in hover before RIPR evidence and status counts before action-oriented text | Do not branch action semantics on parsed prose |
| Proof path includes LSP tests, VS Code e2e, editor workflow fixtures, and `lsp-cockpit-report` | Work items and proof commands below | In progress for behavior slice validation | Remaining closeout/docs work can add dedicated preview workflow fixtures if needed |
| No hidden analysis reruns, source edits, generated tests, provider calls, mutation execution, gate/default-blocking behavior, CodeLens, inlay hints, semantic tokens, inline patches, or unsaved-buffer overlays | [ADR-0011](../../docs/adr/0011-editor-preview-routing-is-projection-only.md), hard boundaries in this plan, [Lane 3 tracker](../../docs/lanes/LANE_3_EDITOR_LSP.md) | Covered as contract and tracker boundary | Future implementation review must inspect `editors/vscode/` and `crates/ripr/src/lsp/` diffs for scope drift |
| Source-of-truth stack separates why, behavior, architecture, plan, active state, readiness, and closeout | [RIPR-PROP-0003](../../docs/proposals/RIPR-PROP-0003-editor-preview-routing.md), [RIPR-SPEC-0036](../../docs/specs/RIPR-SPEC-0036-editor-preview-routing.md), [RIPR-SPEC-0037](../../docs/specs/RIPR-SPEC-0037-editor-preview-static-limit-projection.md), [ADR-0011](../../docs/adr/0011-editor-preview-routing-is-projection-only.md), this plan, [Lane 3 tracker](../../docs/lanes/LANE_3_EDITOR_LSP.md), `.ripr/goals/active.toml` | Covered for the docs stack | Closeout handoff remains future work after behavior lands |

Current completion decision: routing behavior is implemented for Campaign 27.
Python preview artifacts and TypeScript readiness are complete, preview editor
selectors and projection metadata have landed, and CI language grouping is the
next unblocked Campaign 27 work item. Dedicated preview editor workflow
fixtures and docs remain available as follow-up evidence slices if the lane
keeps them separate from CI grouping.

Current-base compatibility: this stack was compatible with `origin/main` at
`23df422` after resolving only the expected ADR/proposal/spec/traceability
registries by keeping the existing Lane 1 entries plus the Lane 3 entries.
After `origin/main` advanced to `ad9c04e`, a refresh audit found that the raw
dirty diff no longer applies cleanly as-is. A docs-stack-only disposable port
onto `ad9c04e` resolved the expected `.ripr/traceability.toml`,
ADR/proposal/spec README, and documentation-index conflicts by keeping the new
Lane 1 scorecard entries plus the Lane 3 entries. Because `ad9c04e` adds
traceability to generated scorecard outputs, the port proof first generated
`target/ripr/reports/evidence-quality-scorecard.json` and `.md` with
`cargo xtask lane1-evidence-audit` followed by
`cargo xtask evidence-quality-scorecard`. The port then passed
`cargo xtask check-doc-index`, `cargo xtask markdown-links`,
`cargo xtask check-traceability`, `cargo xtask check-spec-format`,
`cargo xtask check-static-language`, `cargo xtask check-doc-roles`,
`cargo xtask goals next`, `cargo xtask check-pr`,
`cargo xtask lsp-cockpit-report`, and `git diff HEAD --check`. The actual main
worktree still remains behind `origin/main`; package the PR from a current-base
port or rebase before opening it.

Focused editor proof on the same current-base port also passed:
`cargo test -p ripr lsp --lib`, `cargo test -p ripr lsp::tests --lib`,
`npm --prefix editors/vscode run compile`, and
`npm --prefix editors/vscode run test:e2e` with 30 passing tests. The VS Code
run still prints the known post-success `path` warning while exiting 0.

After `origin/main` advanced to `81765dd`, the new base added the Lane 1
evidence-quality benchmark corpus and touched `.ripr/traceability.toml` plus
`xtask/src/main.rs`. A fresh current-base port preserved those Lane 1 fixture
validators while keeping the Lane 3 plan-path classifier in PR summary
automation. The live readiness audit still found #771, #772, #807, and #814
open at that time, and `cargo xtask goals next` still reported no ready work
items. After #857, the structured `static_limit_kind` field is available and
#807/#814 are closed; #771 is complete, and #772 is the current editor routing
implementation slice.

## Completed Source-Of-Truth Stack

#853 landed the docs-only source-of-truth stack:

```text
docs(lane3): define editor preview routing source-of-truth stack
```

Include only the Lane 3 proposal/spec/ADR/plan files, docs indexes,
traceability entries, campaign summaries, Lane 3 tracker updates, and the
small `xtask` reviewer-packet fix needed to make the new `plans/` layer visible
in PR automation.

Landed file group for that PR:

```text
.ripr/traceability.toml
docs/DOCUMENTATION.md
docs/IMPLEMENTATION_CAMPAIGNS.md
docs/IMPLEMENTATION_PLAN.md
docs/PR_AUTOMATION.md
docs/REPO_TRACKING_MODEL.md
docs/adr/0011-editor-preview-routing-is-projection-only.md
docs/adr/README.md
docs/lanes/LANE_3_EDITOR_LSP.md
docs/proposals/RIPR-PROP-0003-editor-preview-routing.md
docs/proposals/README.md
docs/specs/RIPR-SPEC-0036-editor-preview-routing.md
docs/specs/RIPR-SPEC-0037-editor-preview-static-limit-projection.md
docs/specs/README.md
plans/campaign-27/README.md
plans/campaign-27/lane3-editor-preview-routing.md
xtask/src/main.rs
```

Do not include generated config template, runtime config source, or output
schema wording changes in that PR unless the PR title and acceptance explicitly
cover current-state language-configuration clarification. Those files can be a
separate maintenance PR:

```text
docs(config): clarify current preview-language adapter state
```

This split matters because `crates/ripr/src/config.rs` changes are production
source changes even when the diff only updates generated template text. A
`docs(lane3)` PR should not look behavior-bearing in `cargo xtask pr-summary`.

Reviewer packet note: this branch teaches `cargo xtask pr-summary` and the
report index to classify top-level `plans/` files as documentation evidence and
campaign-planning inputs, so the reviewer packet should include the campaign
plan files in this section.

The separate current-state language-configuration clarification remains a
possible future maintenance PR if the config wording needs another refresh:

```text
.ripr/positioning-language-allowlist.txt
crates/ripr/src/config.rs
docs/CONFIGURATION.md
docs/OUTPUT_SCHEMA.md
policy/output_contracts.txt
ripr.toml.example
```

If taking the config-current-state clarification separately:

```bash
rtk git add -- .ripr/positioning-language-allowlist.txt crates/ripr/src/config.rs docs/CONFIGURATION.md docs/OUTPUT_SCHEMA.md policy/output_contracts.txt ripr.toml.example
rtk git diff --cached --name-only
rtk git diff --cached --check
```

## Hard Boundaries

- saved-workspace only;
- projection-only;
- Rust default unchanged;
- preview languages opt-in only;
- preview findings labeled preview;
- static limits visible before suggested action language;
- wrong-root, stale, missing, unsupported, and malformed artifacts fail closed;
- no source edits;
- no generated tests;
- no provider calls;
- no runtime mutation execution;
- no policy, gate, or default-blocking behavior;
- no CodeLens, inlay hints, semantic tokens, inline patches, or unsaved-buffer
  overlays.

## Work Item: lane3-routing-readiness-audit

Status: done

Blocked by: none

### Goal

Confirm TypeScript-family and Python preview outputs are safe to project in the
editor before selector or routing behavior changes.

### Production Delta

None. This is an audit gate over upstream artifacts and active manifest state.

### Non-Goals

- No VS Code selector changes.
- No LSP routing changes.
- No analyzer changes.
- No active manifest edits unless the campaign owner explicitly changes the
  blocker.

### Acceptance

- TypeScript editor readiness remains closed.
- Python emits `language = "python"` and `language_status = "preview"`.
- Python emits owner, test, assertion, probe, and related-test facts.
- Python emits explicit static limits.
- Fixture/golden coverage exists for Python preview facts.
- `cargo xtask goals next` lists `lsp/editor-language-routing` as ready, or
  `.ripr/goals/active.toml` explicitly supersedes the Python blocker.

### Proof Commands

```bash
cargo xtask goals next
cargo test -p ripr --lib analysis::language::typescript
cargo test -p ripr --lib analysis::language::python
cargo xtask fixtures
cargo xtask goldens check
```

### Rollback

No code or behavior should change. If the audit incorrectly marks routing ready,
restore this plan to blocked and update the Lane 3 tracker with the missing
artifact or blocker.

## Work Item: test(lsp): preserve Rust routing contract

Status: done

Blocked by: none

### Goal

Pin current Rust saved-workspace editor behavior before preview selectors land.

### Production Delta

Add LSP tests for language-config states that must not drift when preview
routing is added.

### Non-Goals

- No preview selectors.
- No TypeScript/Python diagnostics.
- No analyzer behavior changes.

### Acceptance

- `[languages]` absent keeps Rust diagnostics, hover, actions, status, packets,
  and commands unchanged.
- `[languages] enabled = ["rust"]` matches the default.
- `[languages] enabled = []` produces no saved-workspace diagnostics and a
  clear languages-off status.
- Invalid language config stays config-owned; the editor does not invent
  preview behavior.

### Proof Commands

```bash
cargo test -p ripr lsp --lib
cargo test -p ripr lsp::tests --lib
cargo xtask lsp-cockpit-report
npm --prefix editors/vscode run compile
npm --prefix editors/vscode run test:e2e
cargo xtask check-pr
git diff --check
```

### Rollback

Revert only the new guard tests if they encode the wrong current behavior.
Do not loosen Rust default behavior to make preview routing easier.

## Work Item: lsp(language): add opt-in editor language routing

Status: done

Blocked by: none

### Goal

Route enabled preview-language saved-workspace findings through the existing
editor cockpit without changing Rust defaults.

### Production Delta

Extend VS Code activation/selector handling and LSP diagnostic routing for
configured TypeScript-family and Python preview languages.

### Non-Goals

- No hover/status static-limit formatting beyond preserving required fields.
- No analyzer changes.
- No source edits, generated tests, providers, mutation execution, gates, or
  preview-only actions.

### Acceptance

- Rust default behavior remains unchanged.
- `typescript`, `typescriptreact`, `javascript`, and `javascriptreact`
  documents route only when `typescript` is enabled.
- `python` documents route only when `python` is enabled.
- Disabled preview languages produce no diagnostics.
- Wrong-root, stale, missing, unsupported, and malformed artifacts fail closed.

### Proof Commands

```bash
cargo test -p ripr lsp --lib
cargo test -p ripr lsp::tests --lib
cargo xtask lsp-cockpit-report
npm --prefix editors/vscode run compile
cargo xtask check-pr
git diff --check
```

### Rollback

Remove preview selector/routing additions and keep the Rust routing-contract
tests. If activation events were added, remove only the preview-language events.

## Work Item: lsp(language): surface preview labels and static limits

Status: done

Blocked by: none

### Goal

Make preview status and static limits impossible to miss in hover and status
before any action language appears.

### Production Delta

Render preview labels, syntax-first evidence, and static-limit kind or stable
text in existing hover/status surfaces. Keep the current cockpit action model.

### Non-Goals

- No preview-only action semantics.
- No behavior branching on parsed static-limit prose.
- No policy or gate implications.

### Acceptance

- Preview findings show language and `language_status = "preview"` where the
  surface can display them.
- Static limits appear before suggested action language.
- `static_limit_kind` is used for stable labels when present.
- Text-only static limits are displayed as evidence only.
- Actions remain bounded to copy/open/refresh commands backed by artifact
  payloads.

### Proof Commands

```bash
cargo test -p ripr lsp --lib
cargo test -p ripr lsp::tests --lib
cargo xtask lsp-cockpit-report
npm --prefix editors/vscode run compile
cargo xtask check-static-language
cargo xtask check-pr
git diff --check
```

### Rollback

Revert preview formatting changes while keeping routing tests. Do not remove
the projection-only ADR or specs.

## Work Item: fixtures: add preview editor workflow fixtures

Status: blocked

Blocked by: `lsp(language): surface preview labels and static limits`

### Goal

Pin the mixed Rust and preview-language editor cockpit behavior in explicit
fixtures.

### Production Delta

Add preview editor workflow fixtures under `fixtures/editor_lsp_workflow/`.

### Non-Goals

- No new analyzer behavior.
- No generated tests.
- No fixture cases that imply policy eligibility or runtime mutation.

### Acceptance

- Rust default fixture remains stable.
- TypeScript preview fixture includes `language_status = "preview"`.
- Python preview fixture includes `language_status = "preview"`.
- Mixed-language opt-in fixture does not cross-route owners or tests.
- Preview-disabled fixture has no preview diagnostics and a clear status.
- Static limits appear in hover/status artifacts.

### Proof Commands

```bash
cargo xtask fixtures
cargo xtask goldens check
cargo xtask lsp-cockpit-report
cargo test -p ripr lsp --lib
cargo xtask check-fixture-contracts
cargo xtask check-pr
git diff --check
```

### Rollback

Remove only the new preview fixture cases and expected artifacts. Keep Rust
fixtures and routing behavior intact unless a fixture exposed a real routing
bug.

## Work Item: test(vscode): smoke preview saved-workspace routing

Status: blocked

Blocked by: `fixtures: add preview editor workflow fixtures`

### Goal

Prove the packaged VS Code extension path handles preview-language routing and
preview static-limit projection.

### Production Delta

Add live VS Code e2e coverage for enabled and disabled preview-language editor
paths.

### Non-Goals

- No new UI surfaces.
- No CodeLens, inlay hints, semantic tokens, inline patches, or unsaved-buffer
  overlays.
- No provider calls or generated tests.

### Acceptance

- Extension activates for Rust default behavior.
- Preview files are ignored when the language is disabled.
- TypeScript-family preview diagnostic appears when enabled.
- Python preview diagnostic appears when enabled.
- Hover labels preview status and static limits.
- Status/Show Status labels preview state and static limits.
- Copy/open/refresh actions remain bounded.
- Wrong-root, stale, missing, unsupported, and malformed artifacts fail closed.

### Proof Commands

```bash
npm --prefix editors/vscode run compile
npm --prefix editors/vscode run test:e2e
cargo test -p ripr lsp --lib
cargo xtask lsp-cockpit-report
cargo xtask check-pr
git diff --check
```

### Rollback

Remove only the preview e2e cases and any test-only fixtures. Keep production
routing if lower-level tests and fixtures still prove it; otherwise roll back
the production routing slice too.

## Work Item: docs(editor): document preview-language workflow

Status: blocked

Blocked by: `test(vscode): smoke preview saved-workspace routing`

### Goal

Document the user workflow for opt-in preview-language editor evidence.

### Production Delta

Update editor workflow docs to explain Rust default behavior, preview opt-in,
syntax-first evidence, static limits, and the no-edit/no-generation boundary.

### Non-Goals

- No behavior changes.
- No new policy or CI semantics.

### Acceptance

- Docs state Rust is stable and default.
- Docs state TypeScript-family and Python evidence is opt-in preview.
- Docs explain syntax-first evidence and expected static limits.
- Docs say diagnostics are advisory.
- Docs say the editor does not edit source, generate tests, run mutation
  testing, call providers, or change gates.
- Docs describe the loop: enable preview language, inspect status, inspect
  diagnostic, hover for limits, copy packet/brief, write one focused test, run
  verify/receipt when valid, refresh.

### Proof Commands

```bash
cargo xtask check-doc-index
cargo xtask markdown-links
cargo xtask check-static-language
cargo xtask check-pr
git diff --check
```

### Rollback

Revert the docs update only. Do not remove the already-validated behavior unless
the docs exposed a behavior contract mismatch.

## Work Item: campaign(lane3): close editor preview routing

Status: blocked

Blocked by: `docs(editor): document preview-language workflow`

### Goal

Close the Lane 3 editor preview routing slice with proof and remaining limits.

### Production Delta

Add a closeout handoff under `docs/handoffs/` and update the Lane 3 tracker if
the slice is complete.

### Non-Goals

- No new behavior.
- No policy, gate, CI, release, or analyzer closeout.

### Acceptance

- Rust defaults unchanged.
- Preview selectors are opt-in.
- TypeScript-family preview routing is fixture-pinned.
- Python preview routing is fixture-pinned.
- Preview labels are visible.
- Static limits are visible before action language.
- VS Code e2e and `lsp-cockpit-report` prove the real path.
- Docs are current.
- No source edits, generated tests, provider calls, mutation execution,
  gate/default-blocking behavior, CodeLens, inlays, semantic tokens, inline
  patches, or unsaved-buffer overlays landed.

### Proof Commands

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

### Rollback

If closeout proof is incomplete, leave the closeout unmerged and restore this
plan or the Lane 3 tracker to the last accurately blocked state. Do not mark
the slice complete on manifest or tracker evidence alone.
