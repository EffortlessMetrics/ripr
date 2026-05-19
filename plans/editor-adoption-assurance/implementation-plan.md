# Editor Adoption Assurance Implementation Plan

Status: active

Owner: Lane 3 - Editor / LSP UX

Linked proposal: `RIPR-PROP-0012`

Linked specs: `RIPR-SPEC-0054`, `RIPR-SPEC-0052`, `RIPR-SPEC-0050`, `RIPR-SPEC-0049`

Linked ADR: `ADR-0016`

## Current State

Editor Gap Cockpit, Editor First-Run and Repair Usability, Editor First-PR
Bridge, and preview-language routing are closed. The editor can diagnose
setup, project saved-workspace gaps, copy a first repair packet, show receipt
state, refresh, and inspect an existing first-pr packet.

The next Lane 3 gap is adoption assurance: compatibility, active root,
multi-root, and mismatch states should be explicit and fail closed before a
first outside user or coding agent receives a repair packet.

## Hard Boundaries

- saved-workspace first;
- read-only projection over existing artifacts;
- typed fields over prose;
- Rust default unchanged;
- preview evidence visibly bounded;
- fail closed on stale, wrong-root, malformed, missing, unsupported,
  unavailable, unsafe, receipt-mismatched, and first-pr-mismatched states;
- no analyzer truth;
- no policy, gate, badge, baseline, waiver, suppression, or default-blocking
  behavior;
- no PR comment publishing or generated CI summary composition;
- no release publishing, binary download, binary install, or config mutation;
- no source edits, generated tests, provider/model calls, mutation execution,
  CodeLens, inlays, semantic tokens, inline patches, or unsaved overlays.

## GitHub Issue Burn-Down

Issue reconciliation on 2026-05-18 found that the implementation work for
#1247 and #1248 already landed on `main`, but the GitHub issues remained open
because those PRs did not carry closing references. Close #1247 from #1262.
Close #1248 from #1267, #1270, #1272, and #1274. Close #1249 from the
dedicated `fixtures/editor_adoption_assurance` corpus. Close #1250 from the
dedicated VS Code adoption-assurance smoke. Close #1251 from the install-to-
first-pr editor guide. Close #1252 from the external-style dogfood receipts.
Keep #1253 open until the closeout artifact lands.

| Issue | Work item | Status | Evidence / remaining gap |
| --- | --- | --- | --- |
| #1245 | `docs(lane3): open editor adoption assurance stack` | done | Source-of-truth stack landed. |
| #1246 | `test(lsp): pin editor adoption baseline` | done | Baseline editor contract landed. |
| #1247 | `vscode: add extension/server compatibility diagnosis` | done; close issue | #1262 added extension/server compatibility diagnosis, version/schema status, unsupported-schema handling, and VS Code coverage. |
| #1248 | `vscode: harden workspace-root and multi-root diagnosis` | done; close issue | #1267 added root/multi-root diagnosis; #1270 added direct repair-command root guards; #1272 added selected-root projection guards for first-pr copy actions and LSP subscriptions; #1274 added no-active-editor fail-closed guards for direct repair payloads. |
| #1249 | `fixtures(editor): add adoption-assurance fixture corpus` | done; close issue | `fixtures/editor_adoption_assurance` pins setup-ready, server-missing, server-version-mismatch, no-workspace, multi-root, wrong-root, stale-receipt, first-pr-ready, first-pr-mismatch, and preview-adapter-unavailable states. |
| #1250 | `test(vscode): smoke editor adoption assurance path` | done; close issue | VS Code smoke proves setup/status commands, bounded first-pr repair packets, verify/receipt copy actions, and wrong-root/malformed suppression without running hidden analysis. |
| #1251 | `docs(editor): write install-to-first-pr editor guide` | done; close issue | `docs/EDITOR_INSTALL_TO_FIRST_PR.md` stitches install/open, setup diagnosis, Show Status, one repair, verify, receipt, refresh, first-pr packet, recovery states, and non-claims into one adoption path. |
| #1252 | `dogfood(lane3): record external-style editor adoption receipts` | done; close issue | `docs/handoffs/2026-05-19-editor-adoption-assurance-receipts.md` records external-style setup, root, receipt, first-pr, preview-unavailable, and fail-closed evidence. |
| #1253 | `campaign(lane3): close editor adoption assurance` | open; blocked | Blocked by #1249 through #1252. |

## Work Item 1: docs(lane3): open editor adoption assurance stack

### Goal

Define the campaign and GitHub issue burn-down without changing behavior.

### Production Delta

Add proposal, spec, ADR, plan, lane tracker state, indexes, documentation
links, and traceability entries.

### Non-Goals

- No LSP or VS Code behavior changes.
- No output schema changes.
- No active manifest status change.

### Acceptance

- The repo states why adoption assurance exists, what behavior it will own,
  what architecture boundary applies, and what PR sequence follows.
- The issue burn-down is linked from the durable plan.
- Compatibility, workspace-root, multi-root, receipt mismatch, first-pr
  mismatch, and preview boundaries are explicit.

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

Revert the docs stack and close the newly created GitHub issues as not planned.

## Work Item 2: test(lsp): pin editor adoption baseline

### Goal

Pin the closed Lane 3 contract before adoption-assurance behavior changes.

### Production Delta

Add or harden tests for Diagnose Setup, Show Status, first-pr packet state,
receipt state, Rust default diagnostics/hover/actions, preview static-limit
ordering, wrong-root/stale/malformed fail-closed behavior, and first-repair /
first-pr action gating.

### Non-Goals

- No new UI.
- No compatibility diagnosis behavior yet.
- No workspace-root behavior change unless tests reveal existing drift.

### Acceptance

- Current Rust/default editor cockpit behavior remains unchanged.
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

## Work Item 3: vscode: add extension/server compatibility diagnosis

### Goal

Make setup diagnosis explain extension/server compatibility instead of merely
binary presence.

### Production Delta

Show extension version, resolved server path, server version, expected version
or feature support, supported artifact schemas, unsupported schema state, and
the next safe action.

### Non-Goals

- No auto-install.
- No binary download.
- No hidden server replacement.
- No config mutation.

### Acceptance

- Compatible server state appears as safe.
- Version, feature, or unsupported-schema mismatches fail closed for dependent
  repair actions.
- Status remains explanatory and does not claim runtime proof, policy
  eligibility, gate status, or merge readiness.

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

Revert compatibility diagnosis code and tests. Existing setup status remains.

## Work Item 4: vscode: harden workspace-root and multi-root diagnosis

### Goal

Make workspace selection and root mismatch states boring and safe.

### Production Delta

Cover single-root, multi-root, no-workspace, nested-root, wrong-root artifact,
path-with-spaces, Windows path normalization, and supported WSL-like mismatch
cases when testable.

### Non-Goals

- No cross-root repair packet routing.
- No automatic root switching.
- No source edits.

### Acceptance

- The active root is always named when available.
- Ambiguous multi-root state fails closed.
- Wrong-root artifacts suppress repair actions and explain expected versus
  observed root when known.

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

Revert root diagnosis code and tests.

## Work Item 5: fixtures(editor): add adoption-assurance fixture corpus

### Goal

Pin first-use success and fail-closed states as fixtures.

### Production Delta

Add fixture cases:

- `setup_ok`;
- `server_missing`;
- `server_version_mismatch`;
- `no_workspace`;
- `multi_root`;
- `wrong_root_artifact`;
- `stale_receipt`;
- `first_pr_packet_ready`;
- `first_pr_packet_mismatch`;
- `preview_adapter_unavailable`.

Expected artifacts should include setup diagnosis, status JSON, code actions,
first-pr status, and receipt status where applicable.

### Non-Goals

- No analyzer novelty.
- No behavior without corresponding tests.

### Acceptance

- Every success and fail-closed state is pinned.
- No fixture implies gate pass, runtime proof, merge readiness, policy
  eligibility, or automatic repair.

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

Revert fixture corpus and generated expectations.

## Work Item 6: test(vscode): smoke editor adoption assurance path

### Goal

Prove the packaged extension path for adoption assurance.

### Production Delta

Cover extension activation, server resolution, Diagnose Setup compatibility
state, Show Status root/artifact state, first-pr packet state, receipt state,
safe repair actions, and wrong-root/stale/malformed suppression.

### Non-Goals

- No new editor furniture.
- No PR/CI producer behavior.
- No source edits or generated tests.

### Acceptance

- The real extension reports compatibility and root state.
- Repair actions appear only when safe.
- Unsafe states explain and suppress actions.

### Proof Commands

```bash
npm --prefix editors/vscode run compile
npm --prefix editors/vscode run test:e2e
cargo test -p ripr lsp --lib
cargo test -p ripr lsp::tests --lib
cargo xtask lsp-cockpit-report
cargo xtask check-pr
git diff --check
```

### Rollback

Revert smoke tests and test-only harness changes.

## Work Item 7: docs(editor): write install-to-first-pr editor guide

### Goal

Document the user path from install/open to first-pr packet inspection.

### Production Delta

Add or update user docs for:

```text
Install/open RIPR
-> Diagnose Setup
-> Show Status
-> inspect diagnostic
-> open related test or copy repair packet
-> write one focused test
-> verify
-> receipt
-> refresh
-> inspect first-pr packet
-> open PR
```

### Non-Goals

- No behavior change.
- No PR/CI publishing docs beyond handoff pointers.

### Acceptance

- Docs cover server missing, version mismatch, no workspace, wrong root,
  missing artifacts, stale artifacts, preview adapter unavailable, first-pr
  packet missing/malformed/mismatched, and receipt missing/stale/mismatched.
- Docs state non-claims: not merge approval, gate decision, runtime proof,
  mutation proof, policy eligibility, or automatic repair.

### Proof Commands

```bash
cargo xtask check-doc-index
cargo xtask markdown-links
cargo xtask check-static-language
cargo xtask check-pr
git diff --check
```

### Rollback

Revert docs-only change.

## Work Item 8: dogfood(lane3): record external-style editor adoption receipts

### Goal

Record proof across normal repo shapes and fail-closed states.

### Production Delta

Dogfood:

- small Rust crate with no prior artifacts;
- Rust workspace with tests/examples;
- clean/no-action workspace;
- wrong-root artifact;
- stale receipt;
- first-pr packet ready;
- first-pr packet mismatch;
- preview disabled or unavailable.

### Non-Goals

- No behavior change.
- No release/publish behavior.

### Acceptance

- Receipts record commands, editor states, artifact paths, receipt state,
  first-pr packet state, known limitations, and advisory boundaries.
- Receipts do not claim runtime adequacy, gate success, or merge readiness.

### Proof Commands

```bash
cargo xtask lsp-cockpit-report
npm --prefix editors/vscode run compile
npm --prefix editors/vscode run test:e2e
cargo xtask check-pr
git diff --check
```

### Rollback

Revert dogfood docs/artifacts.

## Work Item 9: campaign(lane3): close editor adoption assurance

### Goal

Close only after validation, compatibility diagnosis, root diagnosis,
fixtures, e2e, docs, and dogfood proof land.

### Production Delta

Add closeout handoff, update lane tracker and campaign state, and record final
validation evidence.

### Non-Goals

- No new behavior in closeout.
- No reopening editor gap cockpit, first-run usability, or first-pr bridge.

### Acceptance

- Current Lane 3 contract is pinned.
- Compatibility diagnosis exists.
- Workspace-root and multi-root diagnosis has fixture/e2e evidence.
- Adoption fixtures cover success and fail-closed states.
- VS Code e2e proves the real path.
- Docs explain install-to-first-pr.
- Dogfood receipts prove external-style use.
- No analyzer, policy, PR/CI, release, source-edit, generated-test, provider,
  mutation, CodeLens, inlay, semantic-token, inline-patch, or unsaved-buffer
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

Revert closeout docs and restore campaign state to the previous active work
item. Do not revert already-merged behavior PRs without a separate rollback.
