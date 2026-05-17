# Editor First-PR Bridge Implementation Plan

Status: proposed

Owner: Lane 3 - Editor / LSP UX

Linked proposal: `RIPR-PROP-0010`

Linked specs: `RIPR-SPEC-0052`, `RIPR-SPEC-0051`, `RIPR-SPEC-0050`, `RIPR-SPEC-0049`

Linked ADR: `ADR-0014`

## Current State

Editor Gap Cockpit and Editor First-Run and Repair Usability are closed. The
editor can diagnose setup, explain no-output states, project one repairable
gap, copy a bounded first repair packet, show receipt state, and refresh.

The first successful PR path also exists outside the editor. `cargo xtask
first-pr` writes start-here packets from explicit artifacts:

```text
target/ripr/reports/start-here.{json,md}
target/ripr/first-pr/start-here.{json,md}
```

The next Lane 3 slice bridges those surfaces. It makes the editor explain
whether the first-pr packet exists, is safe, and matches the current workspace
or gap without producing PR/CI artifacts or deciding PR readiness.

## Hard Boundaries

- saved-workspace first;
- read-only projection over existing artifacts;
- typed fields over prose;
- Rust default unchanged;
- preview evidence visibly bounded;
- static limits before action language;
- stale, wrong-root, malformed, missing, unsupported, disabled, unavailable,
  path-unsafe, and command-unsafe states fail closed;
- no analyzer facts created in Lane 3;
- no first-pr packet producer changes;
- no generated CI summary composition;
- no PR comment publishing;
- no policy, gate, badge, baseline, waiver, suppression, or default-blocking
  changes;
- no source edits, inline patches, or automatic repair application;
- no generated tests;
- no provider or model calls;
- no runtime mutation execution;
- no runtime adequacy, mutation proof, policy eligibility, or merge-readiness
  claim;
- no binary installation or config mutation from the editor;
- no CodeLens, inlay hints, semantic tokens, or unsaved-buffer overlays.

## Source-of-Truth Stack

- Proposal: why the editor needs a first-pr bridge.
- Spec: first-pr packet projection contract and fail-closed states.
- ADR: read-only first-pr projection architecture decision.
- Plan: PR sequence, acceptance, validation commands, and rollback.
- Active manifest: current machine-readable execution state only.
- Lane tracker: Lane 3 ownership, readiness, and cross-lane boundaries.
- Closeout: what landed, what passed, and what remains.

## Work Item 1: docs(lane3): open editor first-pr bridge stack

### Goal

Define the Editor First-PR Bridge campaign without changing editor behavior.

### Production Delta

Add proposal, spec, ADR, implementation plan, lane tracker state, indexes, and
traceability entries for the campaign.

### Non-Goals

- No LSP or VS Code behavior changes.
- No new diagnostics, hover, status, or actions.
- No first-pr producer changes.
- No active manifest status changes.

### Acceptance

- The repo states where to put why, first-pr packet behavior, architecture
  decisions, PR sequence, current execution state, lane ownership, and closeout
  validation.
- Lane 3 is described as a read-only first-pr packet consumer.
- The docs state that Lane 3 does not create analyzer facts, first-pr packets,
  policy decisions, gate decisions, PR comments, generated CI summaries,
  generated tests, source edits, provider calls, mutation runs, CodeLens,
  inlays, semantic tokens, inline patches, or unsaved-buffer overlays.

### Proof Commands

```bash
cargo xtask check-spec-format
cargo xtask check-doc-index
cargo xtask markdown-links
cargo xtask check-static-language
cargo xtask check-doc-roles
cargo xtask check-traceability
cargo xtask check-pr
git diff --check
```

### Rollback

Revert the docs-only stack. No runtime behavior changes should remain.

## Work Item 2: test(lsp): pin post-first-run editor contract

### Goal

Pin the current editor contract before adding first-pr packet projection.

### Production Delta

Add or harden tests for Diagnose Setup, Show Status, receipt status, first
repair packet action, Rust default diagnostics/hover/actions, preview
static-limit ordering, and wrong-root/stale fail-closed behavior.

### Non-Goals

- No new UI.
- No first-pr packet behavior yet.
- No analyzer or output producer changes.

### Acceptance

- Current first-run and first-repair behavior is unchanged.
- Existing fail-closed behavior remains pinned.
- Preview static limits still appear before action language.

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

Revert the contract tests. Existing behavior should remain unchanged.

## Work Item 3: lsp(first-pr): validate first-pr packet artifacts

### Goal

Add read-only validation for first-pr packet JSON and Markdown artifacts
without rendering them yet.

### Production Delta

Validate `target/ripr/first-pr/start-here.{json,md}` and
`target/ripr/reports/start-here.{json,md}` for schema, root, freshness, state,
identity, path safety, command safety, language availability, and preview
boundaries.

### Non-Goals

- No first-pr packet producer.
- No Show Status projection yet.
- No open/copy actions yet.

### Acceptance

- Supported packets validate.
- Missing, stale, wrong-root, malformed, gap-mismatched, path-unsafe, and
  command-unsafe packets fail closed.
- Markdown is never parsed for action semantics.

### Proof Commands

```bash
cargo test -p ripr lsp --lib
cargo test -p ripr lsp::tests --lib
cargo xtask check-static-language
cargo xtask check-pr
git diff --check
```

### Rollback

Revert validation code and tests.

## Work Item 4: lsp(first-pr): project first-pr state in Show Status

### Goal

Make `ripr: Show Status` and setup diagnosis explain first-pr packet state.

### Production Delta

Project missing, found, stale, wrong-root, malformed, no-action, and top
repairable gap states from validated first-pr packets.

### Non-Goals

- No PR readiness or gate decision.
- No PR comments or CI summary.
- No new diagnostics.

### Acceptance

- Status explains the first-pr packet state and next safe action.
- Unsafe states fail closed and suppress first-pr repair claims.
- Advisory and preview boundaries remain explicit.

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

Revert status projection. Existing Show Status behavior remains as fallback.

## Work Item 5: lsp(first-pr): add bounded first-pr packet actions

### Goal

Expose open/copy actions only when a validated first-pr packet supports them.

### Production Delta

Add actions for opening the Markdown packet, copying a first-pr summary,
copying a first-pr repair packet, copying verify and receipt commands, and
refresh/regeneration guidance.

### Non-Goals

- No generated tests.
- No source edits.
- No provider calls.
- No preview-only action model.

### Acceptance

- Actions appear only for workspace-local, current, command-safe packets.
- Current diagnostic identity must match packet gap identity for
  diagnostic-scoped repair actions.
- Stale, wrong-root, malformed, missing, gap-mismatched, path-unsafe, and
  command-unsafe packets suppress open/copy repair actions.

### Proof Commands

```bash
cargo test -p ripr lsp --lib
cargo test -p ripr lsp::tests --lib
cargo xtask lsp-cockpit-report
cargo xtask check-static-language
cargo xtask check-pr
git diff --check
```

### Rollback

Revert first-pr actions. Existing repair packet and command-copy actions remain.

## Work Item 6: fixtures(editor): add first-pr bridge fixtures

### Goal

Pin success and fail-closed first-pr bridge states as editor fixtures.

### Production Delta

Add fixture cases:

- `setup_ok`;
- `packet_missing`;
- `packet_found_repairable`;
- `packet_no_action`;
- `packet_stale`;
- `packet_wrong_root`;
- `packet_malformed`;
- `receipt_improved_packet_ready`;
- `receipt_unchanged_packet_ready`.

Expected artifacts:

- `vscode-status.json`;
- `lsp-code-actions.json`;
- `first-pr-status.json`;
- `setup-diagnosis.md`.

### Non-Goals

- No broad first successful PR corpus changes.
- No behavior without corresponding tests.

### Acceptance

- Fixtures distinguish first-pr packet states.
- Unsafe states fail closed.
- Receipt-improved and receipt-unchanged states do not overclaim PR readiness.

### Proof Commands

```bash
cargo xtask fixtures
cargo xtask goldens check
cargo xtask check-fixture-contracts
cargo xtask lsp-cockpit-report
cargo xtask check-pr
git diff --check
```

### Rollback

Revert the fixture corpus and generated expectations.

## Work Item 7: test(vscode): smoke first-pr bridge

### Goal

Prove the real extension path for first-pr packet projection.

### Production Delta

Add VS Code e2e coverage for Diagnose Setup, Show Status, open packet, copy
packet, and fail-closed states.

### Non-Goals

- No new editor furniture.
- No PR publisher.
- No first-pr producer.

### Acceptance

- Extension activates and server resolves.
- Diagnose Setup reports first-pr packet state.
- Show Status reports first-pr packet state.
- Open/copy actions work only for safe paths and payloads.
- Wrong-root, stale, malformed, missing, and unsafe states fail closed.

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

Revert the smoke tests and any test-only harness changes.

## Work Item 8: docs(editor): document first successful PR bridge

### Goal

Document the editor flow from local repair to first-pr packet inspection.

### Production Delta

Update user docs so the end-to-end flow is:

```text
Diagnose Setup
-> Show Status
-> inspect diagnostic
-> copy first repair packet
-> write one focused test
-> verify
-> receipt
-> refresh
-> inspect first-pr packet
-> open PR
```

### Non-Goals

- No behavior change.
- No PR/CI publishing docs beyond the handoff pointer.

### Acceptance

- Docs explain what the first-pr packet means and does not mean.
- Docs cover missing, stale, wrong-root, malformed, no-action, and
  top-repairable-gap states.
- Docs state no merge approval, gate decision, runtime proof, mutation proof,
  or policy eligibility claim.

### Proof Commands

```bash
cargo xtask check-doc-index
cargo xtask markdown-links
cargo xtask check-static-language
cargo xtask check-pr
git diff --check
```

### Rollback

Revert the docs-only change.

## Work Item 9: dogfood(lane3): record first-pr bridge receipts

### Goal

Record proof across success and fail-closed first-pr bridge states.

### Production Delta

Add dogfood receipts for packet missing, top repairable gap, no-action,
malformed artifact, stale packet, wrong-root packet, receipt improved, and
receipt unchanged.

### Non-Goals

- No behavior change.
- No first-pr packet producer changes.

### Acceptance

- Dogfood records commands, artifact paths, status output, action fixture
  results, known limitations, and advisory boundaries.
- Receipts prove bridge behavior without claiming runtime adequacy or gate
  success.

### Proof Commands

```bash
cargo xtask lsp-cockpit-report
cargo test -p ripr lsp --lib
cargo test -p ripr lsp::tests --lib
npm --prefix editors/vscode run compile
npm --prefix editors/vscode run test:e2e
cargo xtask check-pr
git diff --check
```

### Rollback

Revert dogfood receipt docs/artifacts. Production behavior remains unchanged.

## Work Item 10: campaign(lane3): close editor first-pr bridge

### Goal

Close the campaign only after validation, status projection, bounded actions,
fixtures, e2e, docs, and dogfood proof land.

### Production Delta

Add closeout handoff, update lane tracker and campaign state, and record final
validation evidence.

### Non-Goals

- No new behavior in closeout.
- No reopening editor gap cockpit or first-run usability.

### Acceptance

- Current first-run contract remains green.
- First-pr packet validation exists.
- Show Status and Diagnose Setup project first-pr state.
- Bounded actions exist.
- Fixtures cover success and fail-closed states.
- VS Code e2e proves the real path.
- Docs explain the workflow.
- Dogfood receipts prove it.
- No analyzer, policy, PR-comment, CI-summary, source-edit, generated-test,
  provider, mutation, gate, CodeLens, inlay, semantic-token, inline-patch, or
  unsaved-overlay scope landed.

### Proof Commands

```bash
cargo xtask goals next
cargo xtask lsp-cockpit-report
cargo test -p ripr lsp --lib
cargo test -p ripr lsp::tests --lib
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

### Rollback

Revert closeout docs and restore the campaign state to the previous active work
item. Do not revert already-merged behavior PRs unless a separate rollback is
explicitly opened.
