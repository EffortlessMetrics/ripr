# Editor Gap Cockpit Implementation Plan

Status: closed

Owner: Lane 3 - Editor / LSP UX

Linked proposal: `RIPR-PROP-0007`

Linked spec: `RIPR-SPEC-0047`

Linked ADR: `ADR-0012`

## Current State

Campaign 27 Language Adapter Preview is closed. Rust remains the stable
saved-workspace editor path, and TypeScript, JavaScript, and Python preview
routing exists behind opt-in language configuration with preview labels and
static limits.

Editor Gap Cockpit is closed. It made the editor consume existing RIPR evidence
artifacts and project one safe local next action. It was not a language-routing
campaign, analyzer campaign, policy campaign, PR/CI campaign, provider
campaign, mutation campaign, or source-edit campaign.

Closeout handoff:
`docs/handoffs/2026-05-15-editor-gap-cockpit-closeout.md`.

Merged PR chain:

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

## Hard Boundaries

- saved-workspace first;
- projection-only;
- read-only artifact consumption;
- Rust default unchanged;
- preview languages opt-in and visibly bounded;
- static limits before action language;
- wrong-root, stale, malformed, disabled, unavailable, unsupported, and
  missing-identity states fail closed;
- no analyzer facts created in Lane 3;
- no policy, gate, badge, baseline, waiver, or default-blocking changes;
- no PR comment publishing;
- no generated CI summary composition;
- no generated tests;
- no source edits or inline patches;
- no provider or model calls;
- no runtime mutation execution;
- no CodeLens, inlay hints, semantic tokens, or unsaved-buffer overlays.

## Source-of-Truth Stack

- Proposal: why the editor gap cockpit exists and who benefits.
- Spec: what editor gap projection must do.
- ADR: durable read-only projection architecture decision.
- Plan: PR sequence, acceptance, proof commands, and rollback.
- Active manifest: current machine-readable execution state only.
- Lane tracker: Lane 3 ownership, readiness, and cross-lane boundaries.
- Closeout: what landed, what passed, and what remains.

## Work Item 1: docs(lane3): open editor gap cockpit source-of-truth stack

### Goal

Define the Editor Gap Cockpit campaign without changing editor behavior.

### Production Delta

Add the proposal, spec, ADR, implementation plan, lane tracker state, indexes,
and traceability entries for the campaign.

### Non-Goals

- No LSP or VS Code behavior changes.
- No new diagnostics, hover, status, or actions.
- No output schema changes.
- No active manifest status changes.

### Acceptance

- The repo states where to put why, what, architecture decisions, PR sequence,
  current execution state, lane ownership, and closeout proof.
- Lane 3 is described as a read-only projection consumer.
- The docs state that Lane 3 does not create policy, gate decisions, analyzer
  facts, PR comments, generated tests, source edits, provider calls, or
  mutation runs.

### Proof Commands

```bash
cargo xtask check-doc-index
cargo xtask markdown-links
cargo xtask check-static-language
cargo xtask check-doc-roles
cargo xtask check-pr
git diff --check
```

### Rollback

Revert the docs-only stack. No runtime behavior changes should remain.

## Work Item 2: test(lsp): pin post-campaign editor contract

### Goal

Re-pin editor behavior after Campaign 27 before adding gap projection.

### Production Delta

Add tests and fixture expectations only.

### Non-Goals

- No new UI.
- No artifact validation behavior.
- No action model changes.

### Acceptance

- Rust default diagnostics, hover, actions, and status remain unchanged.
- Preview-language diagnostic metadata remains visible and bounded.
- Disabled-language no-diagnostic status is pinned.
- Stale status, wrong-root fail-closed behavior, related-test path safety, and
  static-limit hover ordering are pinned.

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

Revert the tests and fixtures. No production behavior should be affected.

## Work Item 3: lsp(gap): add read-only gap artifact validation

### Goal

Validate gap-oriented artifacts in the LSP layer without rendering them yet.

### Production Delta

Add parsing and validation for supported gap artifacts using root, schema,
identity, freshness, language, path, static-limit, and command-payload checks.

### Non-Goals

- No new diagnostics.
- No hover/status/action rendering.
- No analyzer or output schema changes.

### Acceptance

- Supported artifacts validate when root, identity, language, freshness, path,
  and command payloads are safe.
- Wrong-root, missing identity, unsupported schema, stale artifact, disabled
  language, out-of-workspace related test, and malformed command payload fail
  closed.
- Validation stores enough state for later status, hover, and action projection
  without parsing prose for action semantics.

### Proof Commands

```bash
cargo test -p ripr lsp --lib
cargo xtask check-output-contracts
cargo xtask check-static-language
cargo xtask check-pr
git diff --check
```

### Rollback

Revert validation code and tests. Existing editor projection should continue
to use current artifacts.

## Work Item 4: lsp(gap): project gap state in Show Status

### Goal

Make `ripr: Show Status` answer what the user can safely do next.

### Production Delta

Render validated gap state, preview status, disabled/unavailable states, stale
states, wrong-root states, and refresh guidance in status output.

### Non-Goals

- No new diagnostics.
- No new code actions.
- No source edits, generated tests, providers, mutation, or policy behavior.

### Acceptance

- Status can report actionable gap, preview evidence, no actionable seam, stale
  evidence, unavailable preview language, disabled language, and wrong-root
  report states.
- Status includes workspace/server/language context when available.
- Stale or invalid artifacts do not project repair instructions.

### Proof Commands

```bash
cargo test -p ripr lsp --lib
cargo test -p ripr lsp::tests --lib
cargo xtask lsp-cockpit-report
cargo xtask check-pr
git diff --check
```

### Rollback

Revert status projection changes. Existing Show Status behavior should remain
as the fallback.

## Work Item 5: lsp(gap): enrich hover with repair route

### Goal

Show the local repair route when a diagnostic maps to a validated gap or
evidence record.

### Production Delta

Add hover sections for gap state, why it matters, related test or repair
target, verify command, receipt command, and limits/non-claims.

### Non-Goals

- No action model change.
- No generated repair text from providers.
- No runtime adequacy or policy eligibility claims.

### Acceptance

- Hover order is language/status, static limits, gap state, why, repair target,
  verify command, receipt command, limits.
- Preview static limits appear before suggested action language.
- Missing or invalid gap artifacts suppress gap-specific hover sections.

### Proof Commands

```bash
cargo test -p ripr lsp --lib
cargo xtask lsp-cockpit-report
cargo xtask check-static-language
cargo xtask check-pr
git diff --check
```

### Rollback

Revert hover changes. Existing hover evidence should remain intact.

## Work Item 6: lsp(gap): add bounded repair packet actions

### Goal

Expose copy/open actions only when validated artifacts support them.

### Production Delta

Add conditional actions for opening related tests, copying repair packets,
copying verify commands, copying receipt commands, copying static-limit notes,
and refreshing evidence.

### Non-Goals

- No preview-only action model.
- No generated tests.
- No source edits.
- No PR comment publishing.

### Acceptance

- No empty action payloads.
- No path escapes.
- No stale artifact actions except refresh.
- Wrong-root, disabled, malformed, and unsupported artifacts suppress actions.
- Preview languages use the same cockpit action model with explicit labels.

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

Revert action changes. Existing saved-workspace actions should remain.

## Work Item 7: fixtures(editor): add gap cockpit workflow fixtures

### Goal

Pin the editor gap cockpit contract with explicit workflow fixtures.

### Production Delta

Add `fixtures/editor_gap_cockpit/*` cases and expected LSP/status/action
artifacts.

### Non-Goals

- No new LSP behavior beyond fixture-backed expectations.
- No analyzer changes.

### Acceptance

- Fixtures cover Rust actionable, TypeScript preview static limit, Python
  preview static limit, disabled language, wrong root, stale artifact, and no
  actionable gap.
- Expected artifacts include diagnostics, hover, code actions, VS Code status,
  and gap projection JSON.

### Proof Commands

```bash
cargo xtask fixtures
cargo xtask goldens check
cargo xtask lsp-cockpit-report
cargo xtask check-fixture-contracts
cargo xtask check-pr
git diff --check
```

### Rollback

Revert fixture additions and expected artifacts.

## Work Item 8: test(vscode): smoke editor gap cockpit

### Goal

Prove the real extension path.

### Production Delta

Add live VS Code e2e coverage for the gap cockpit.

### Non-Goals

- No new editor feature beyond the already implemented projection contract.
- No publishing, packaging, or marketplace behavior.

### Acceptance

- Extension activates and server resolves.
- Rust default works.
- Preview-enabled workspace works.
- Disabled preview produces no diagnostics and clear status.
- Hover renders gap state and static limits.
- Code actions are bounded.
- Related test opens only for safe paths.
- Wrong-root and stale artifacts fail closed.

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

Revert e2e test additions. Existing VS Code smoke should continue to pass.

## Work Item 9: docs(editor): document gap cockpit workflow

### Goal

Explain the local repair loop for users.

### Production Delta

Add or update editor docs with the gap cockpit workflow and no-output states.

### Non-Goals

- No behavior changes.
- No support-tier change.

### Acceptance

- Docs cover open workspace, read status, inspect diagnostic, hover evidence,
  open related test or copy repair packet, write one focused test, run verify,
  emit receipt, and refresh.
- Docs explain Rust stable/default, preview-language opt-in, static limits,
  advisory preview boundary, no-output, stale, disabled, unavailable, no source
  edits, no generated tests, no mutation execution, and no provider calls.

### Proof Commands

```bash
cargo xtask check-doc-index
cargo xtask markdown-links
cargo xtask check-static-language
cargo xtask check-doc-roles
cargo xtask check-pr
git diff --check
```

### Rollback

Revert documentation changes.

## Work Item 10: dogfood(lane3): record editor gap cockpit receipts

### Goal

Record proof that the loop works against real artifacts.

### Production Delta

Add dogfood receipts or handoff notes for representative cockpit cases.

### Non-Goals

- No new behavior.
- No release or publishing work.

### Acceptance

- Receipts cover Rust actionable gap, TypeScript preview static limit, Python
  preview static limit, disabled language, wrong-root artifact, stale artifact,
  and no actionable gap.
- Receipts include commands run and observed artifact paths.

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

Revert dogfood receipt docs. No behavior should change.

## Work Item 11: campaign(lane3): close editor gap cockpit

### Goal

Close the campaign only when behavior, proof, docs, and boundaries are current.

### Production Delta

Add a closeout handoff and final tracker updates.

### Non-Goals

- No late behavior changes in closeout.
- No new campaign scope.

### Acceptance

- Post-Campaign 27 editor contract is pinned.
- Gap artifacts are validated read-only.
- Show Status projects gap state.
- Hover projects repair route.
- Actions remain bounded and fail closed.
- Fixtures cover Rust, preview, stale, wrong-root, disabled, and no-action
  cases.
- VS Code e2e proves the real path.
- Docs explain the workflow.
- No analyzer, policy, gate, PR-comment, source-edit, generated-test, provider,
  mutation, default-blocking, CodeLens, inlay, semantic-token, inline patch, or
  unsaved-overlay work landed.

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

Revert closeout-only docs if proof is incomplete. Do not close the campaign
until the required behavior and evidence have landed.
