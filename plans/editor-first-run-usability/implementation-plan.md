# Editor First-Run Usability Implementation Plan

Status: proposed

Owner: Lane 3 - Editor / LSP UX

Linked proposal: `RIPR-PROP-0008`

Linked specs: `RIPR-SPEC-0048`, `RIPR-SPEC-0049`

Linked ADR: `ADR-0013`

## Current State

The Editor Gap Cockpit is closed. Rust remains the stable saved-workspace
editor path, and TypeScript, JavaScript, and Python preview routing remains
opt-in, advisory, and static-limit bounded.

The next Lane 3 campaign is not new language routing or new analyzer truth. It
is first-run and repair usability: make the existing cockpit self-orienting so
a user can get from install/open to one focused Rust gap repair receipt without
already knowing RIPR internals.

## Hard Boundaries

- projection-only;
- saved-workspace first;
- read-only artifact consumption;
- typed fields over prose;
- Rust default unchanged;
- preview evidence visibly bounded;
- static limits before action language;
- stale, wrong-root, malformed, disabled, unavailable, unsupported, and
  missing states fail closed;
- no analyzer facts created in Lane 3;
- no policy, gate, badge, baseline, waiver, or default-blocking changes;
- no PR comment publishing;
- no generated CI summary composition;
- no generated tests;
- no source edits, inline patches, or automatic repair application;
- no provider or model calls;
- no runtime mutation execution;
- no binary installation or config mutation from the editor;
- no CodeLens, inlay hints, semantic tokens, or unsaved-buffer overlays.

## Source-of-Truth Stack

- Proposal: why first-run usability exists and who benefits.
- Specs: setup/no-output status and first repair loop contracts.
- ADR: durable read-only setup diagnosis architecture decision.
- Plan: PR sequence, acceptance, validation commands, and rollback.
- Active manifest: current machine-readable execution state only.
- Lane tracker: Lane 3 ownership, readiness, and cross-lane boundaries.
- Closeout: what landed, what passed, and what remains.

## Work Item 1: docs(lane3): open editor first-run usability stack

### Goal

Define the Editor First-Run and Repair Usability campaign without changing
editor behavior.

### Production Delta

Add the proposal, specs, ADR, implementation plan, lane tracker state, indexes,
and traceability entries for the campaign.

### Non-Goals

- No LSP or VS Code behavior changes.
- No new diagnostics, hover, status, or actions.
- No output schema changes.
- No active manifest status changes.

### Acceptance

- The repo states where to put why, setup-status behavior, first-repair
  behavior, architecture decisions, PR sequence, current execution state, lane
  ownership, and closeout validation.
- Lane 3 is described as a read-only projection consumer.
- The docs state that Lane 3 does not create analyzer facts, policy decisions,
  gate decisions, PR comments, generated tests, source edits, provider calls,
  mutation runs, binary installs, or config mutations.

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

## Work Item 2: vscode: add setup diagnosis status model

### Goal

Represent setup, server, workspace, language, artifact, freshness, and next
safe action state in a stable model.

### Production Delta

Add a status model for `server_path`, `server_version`, `server_started`,
`workspace_root`, `config_path`, `enabled_languages`, `available_languages`,
`artifact_paths`, stale state, and `next_safe_action`.

### Non-Goals

- No new diagnostics.
- No repair actions from setup state alone.
- No binary installation or config mutation.

### Acceptance

- Show Status can explain installed, started, configured, available, disabled,
  unavailable, missing, and stale states.
- Missing or ambiguous setup state does not invent analyzer conclusions.
- Rust default remains unchanged.

### Proof Commands

```bash
npm --prefix editors/vscode run compile
cargo test -p ripr lsp --lib
cargo xtask check-pr
git diff --check
```

### Rollback

Revert the status model and tests. Existing Show Status behavior should remain
as the fallback.

## Work Item 3: vscode: add ripr Diagnose Setup command

### Goal

Expose a command-palette setup diagnosis report for first-run and no-output
states.

### Production Delta

Add `ripr: Diagnose Setup` to render a readable report naming server, binary,
workspace, config, languages, artifacts, freshness, and next step.

### Non-Goals

- No install or repair workflow.
- No hidden analysis rerun.
- No source edits or config writes.

### Acceptance

- The command renders useful setup status when diagnostics are absent.
- Server missing, workspace missing, config missing, disabled language,
  unavailable adapter, artifact missing, and stale evidence states are
  distinct.
- The command stays read-only.

### Proof Commands

```bash
npm --prefix editors/vscode run compile
npm --prefix editors/vscode run test:e2e
cargo xtask check-pr
git diff --check
```

### Rollback

Revert the command registration and rendering. Existing commands should remain
unchanged.

## Work Item 4: test(vscode): smoke first-run and no-output states

### Goal

Prove the real extension path explains itself when no diagnostics appear.

### Production Delta

Add VS Code e2e coverage for first-run and no-output states.

### Non-Goals

- No production behavior beyond test support if it is not already present.
- No new UI furniture.

### Acceptance

- E2e covers no workspace, server unavailable, server available, missing
  config, Rust-only default, preview language disabled, preview adapter
  unavailable, stale evidence, no actionable gap, and actionable gap.
- The tests assert clear status or setup diagnosis text, not just absence of
  diagnostics.

### Proof Commands

```bash
npm --prefix editors/vscode run compile
npm --prefix editors/vscode run test:e2e
cargo test -p ripr lsp --lib
cargo xtask check-pr
git diff --check
```

### Rollback

Revert the test additions and any test-only fixtures. Production behavior
should remain unchanged.

## Work Item 5: lsp: link receipts into Show Status

### Goal

Make Show Status tell whether the current gap has existing receipt evidence.

### Production Delta

Consume existing receipt artifacts and render receipt found, missing, stale,
gap mismatch, movement improved, and movement unchanged states.

### Non-Goals

- No new receipt producer.
- No receipt mutation.
- No policy or gate decision.

### Acceptance

- Status can tell whether the current gap has receipt evidence.
- Stale, wrong-root, malformed, or gap-mismatched receipts fail closed.
- Receipt status does not claim runtime adequacy or gate eligibility.

### Proof Commands

```bash
cargo test -p ripr lsp --lib
cargo test -p ripr lsp::tests --lib
cargo xtask lsp-cockpit-report
cargo xtask check-pr
git diff --check
```

### Rollback

Revert receipt status projection. Existing Show Status behavior should remain
as the fallback.

## Work Item 6: lsp: add first-repair action packet

### Goal

Expose a bounded `Copy first repair packet` action when evidence supports it.

### Production Delta

Add a copy action that includes gap identity, language/status, static limit,
related test, suggested action, verify command, receipt command, limits, and
non-claims.

### Non-Goals

- No generated tests.
- No source edits.
- No provider or model calls.
- No preview-only action model.

### Acceptance

- The action appears only when gap identity, repair route, verify command, and
  receipt command are valid.
- Stale, wrong-root, disabled, malformed, or missing-command artifacts
  suppress the action.
- Preview packets keep preview status and static limits before action
  language.

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

Revert the action and tests. Existing repair packet and command-copy actions
should remain unchanged.

## Work Item 7: fixtures(editor): add first-run usability fixtures

### Goal

Pin first-run, no-output, and receipt-status states as editor fixtures.

### Production Delta

Add fixture cases:

- `setup_ok`;
- `server_missing`;
- `config_missing`;
- `language_disabled`;
- `adapter_unavailable`;
- `artifact_missing`;
- `artifact_stale`;
- `receipt_improved`;
- `receipt_unchanged`.

Expected artifacts:

- `vscode-status.json`;
- `setup-diagnosis.md`;
- `lsp-code-actions.json`;
- `receipt-status.json`.

### Non-Goals

- No new behavior without corresponding tests.
- No broad fixture corpus beyond first-run usability.

### Acceptance

- Fixtures distinguish no-output states.
- Receipt fixtures pin improved and unchanged movement without overclaiming.
- Wrong-root, stale, disabled, and unavailable states fail closed.

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

## Work Item 8: docs(editor): write first-run-to-first-receipt guide

### Goal

Document the operator path from install/open to first Rust repair receipt.

### Production Delta

Add or update editor docs for:

```text
Install RIPR.
Open workspace.
Run Diagnose Setup.
Read status.
Inspect diagnostic.
Open related test.
Write one focused test.
Run verify.
Emit receipt.
Refresh.
```

### Non-Goals

- No behavior change.
- No policy, PR/CI, release, or analyzer docs expansion outside the first-run
  path.

### Acceptance

- Docs explain Rust stable/default, preview opt-in, static limits, stale,
  disabled, unavailable, wrong-root, no-action, receipt found/missing/stale/
  mismatch/improved/unchanged, and non-claims.
- Docs state no source edits, generated tests, mutation execution, provider
  calls, gate decisions, PR comments, or runtime adequacy claims.

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

## Work Item 9: dogfood(lane3): record first-run repair receipts

### Goal

Prove the first-run repair loop with actual artifacts.

### Production Delta

Record dogfood receipts for setup diagnosis, first actionable Rust gap, verify
movement, receipt emitted, refresh state, and no-action state after receipt
evidence is present.

### Non-Goals

- No behavior change.
- No new receipt producer.
- No generated tests.

### Acceptance

- Dogfood records command lines, artifact paths, setup status, hover/action
  result, verify result, receipt path, refresh state, and known limitations.
- The receipts record movement without claiming runtime adequacy or policy gate
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

Revert dogfood receipt docs/artifacts. Production behavior should remain
unchanged.

## Work Item 10: campaign(lane3): close editor first-run usability

### Goal

Close the campaign only after the setup diagnosis and first repair loop are
implemented, validated, documented, and dogfooded.

### Production Delta

Add closeout handoff, update lane tracker and campaign state, and record final
validation evidence.

### Non-Goals

- No new behavior in closeout.
- No reopening editor gap cockpit or language routing.

### Acceptance

- Setup diagnosis exists.
- Show Status explains no-output states.
- VS Code e2e covers first-run states.
- Receipt status is visible.
- First repair packet is bounded.
- Docs explain the flow.
- Dogfood receipt validates it.
- No analyzer, policy, PR/CI, source-edit, generated-test, provider, mutation,
  gate, CodeLens, inlay, semantic-token, inline-patch, or unsaved-overlay
  scope landed.

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

Revert closeout docs and restore the campaign state to the previous active
work item. Do not revert already-merged behavior PRs unless a separate rollback
is explicitly opened.
