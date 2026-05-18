# Editor Adoption Assurance Implementation Plan

Status: proposed

Owner: Lane 3 - Editor / LSP UX

Linked proposal: `RIPR-PROP-0012`

Linked spec: `RIPR-SPEC-0054`

Linked ADR: `ADR-0016`

## Current state

Editor Gap Cockpit, Editor First-Run and Repair Usability, and Editor First-PR
Bridge are closed. The editor can project diagnostics, hovers, Show Status,
setup diagnosis, bounded repair packets, receipt state, and first-pr packet
state from existing artifacts.

The remaining Lane 3 adoption gap is assurance: make install/open/setup/root
and first-pr/receipt states explicit enough that a new user can complete the
first successful PR path without learning the internal report graph.

## Hard boundaries

- saved-workspace first;
- read-only projection over existing artifacts and extension/server metadata;
- typed fields over prose;
- Rust default unchanged;
- preview evidence visibly bounded;
- static limits before action language;
- stale, wrong-root, malformed, missing, unsupported, disabled, unavailable,
  ambiguous, path-unsafe, and command-unsafe states fail closed;
- no analyzer truth changes;
- no first-pr packet producer changes;
- no generated CI summary composition or PR comment publishing;
- no policy/gate/default-blocking behavior;
- no source edits, generated tests, provider/model calls, mutation execution,
  CodeLens, inlays, semantic tokens, inline patches, or unsaved-buffer
  overlays;
- no auto-install, binary download, config mutation, or hidden server
  replacement.

## GitHub issue burn-down

| Issue | Work item | Status |
| --- | --- | --- |
| #1245 | `docs(lane3): open editor adoption assurance stack` | active |
| #1246 | `test(lsp): pin editor adoption baseline` | planned |
| #1247 | `vscode: add extension/server compatibility diagnosis` | planned |
| #1248 | `vscode: harden workspace-root and multi-root diagnosis` | planned |
| #1249 | `fixtures(editor): add adoption-assurance fixture corpus` | planned |
| #1250 | `test(vscode): smoke editor adoption assurance path` | planned |
| #1251 | `docs(editor): write install-to-first-pr editor guide` | planned |
| #1252 | `dogfood(lane3): record external-style editor adoption receipts` | planned |
| #1253 | `campaign(lane3): close editor adoption assurance` | planned |

## Work item 1: docs(lane3): open editor adoption assurance stack

### Goal

Define the campaign without changing editor behavior.

### Production delta

Add proposal, spec, ADR, implementation plan, indexes, lane tracker state, and
traceability entries.

### Non-goals

- No LSP or VS Code behavior changes.
- No analyzer, PR/CI, policy, release, or schema changes.
- No new fixtures or e2e tests in this docs-only slice.

### Acceptance

- The repo states why editor adoption assurance exists.
- The spec defines setup, compatibility, root, artifact, first-pr packet, and
  receipt states.
- The ADR records the read-only diagnosis boundary.
- The plan maps #1245-#1253 to PR-sized work.
- Lane 3 tracker and indexes point to the source-of-truth stack.

### Proof commands

```bash
cargo xtask check-doc-index
cargo xtask markdown-links
cargo xtask check-static-language
cargo xtask check-doc-roles
cargo xtask check-traceability
cargo xtask check-pr
git diff --check
```

### Rollback

Revert the docs stack and leave #1245-#1253 open for a revised plan.

## Work item 2: test(lsp): pin editor adoption baseline

### Goal

Pin the closed Lane 3 editor contract before adoption-assurance behavior
changes.

### Production delta

Add or harden tests around Diagnose Setup, Show Status, first-pr packet state,
receipt state, Rust default diagnostics/hover/actions, preview static-limit
ordering, wrong-root/stale/malformed fail-closed behavior, and first-repair or
first-pr action gating.

### Non-goals

- No new UI.
- No behavior change unless a test exposes real contract drift.

### Acceptance

- Current editor behavior is documented by tests.
- Fail-closed states suppress unsafe repair actions.
- Rust defaults and preview static-limit ordering remain pinned.

### Proof commands

```bash
cargo test -p ripr lsp --lib
cargo test -p ripr lsp::tests --lib
cargo xtask lsp-cockpit-report
npm --prefix editors/vscode run compile
npm --prefix editors/vscode run test:e2e
cargo xtask check-pr
git diff --check
```

## Work item 3: vscode: add extension/server compatibility diagnosis

### Goal

Make Diagnose Setup and Show Status explain whether the extension and resolved
server are compatible.

### Production delta

Surface extension version, resolved server path, server version, expected
protocol/features, supported artifact schema versions, unsupported schema
state, and next safe action.

### Non-goals

- No auto-install.
- No binary download.
- No config mutation.
- No hidden server replacement.

### Acceptance

- Compatible, missing, mismatched, and unsupported states are explicit.
- Repair actions that depend on compatibility are suppressed when compatibility
  is unknown or unsafe.

### Proof commands

```bash
npm --prefix editors/vscode run compile
npm --prefix editors/vscode run test:e2e
cargo test -p ripr lsp --lib
cargo xtask lsp-cockpit-report
cargo xtask check-pr
git diff --check
```

## Work item 4: vscode: harden workspace-root and multi-root diagnosis

### Goal

Make workspace selection and root mismatch states explicit.

### Production delta

Cover single-root, multi-root, no workspace, nested workspace, wrong-root
artifacts, paths with spaces, Windows path normalization, and supported
WSL-like path mismatch cases where testable.

### Non-goals

- No new analyzer root-discovery behavior.
- No repair packet projection from ambiguous roots.

### Acceptance

- Active root is named.
- Ambiguous or wrong-root states fail closed.
- No repair packet is available from an ambiguous root state.

### Proof commands

```bash
npm --prefix editors/vscode run compile
npm --prefix editors/vscode run test:e2e
cargo test -p ripr lsp --lib
cargo xtask lsp-cockpit-report
cargo xtask check-pr
git diff --check
```

## Work item 5: fixtures(editor): add adoption-assurance fixture corpus

### Goal

Pin setup and use failure modes as editor fixtures.

### Production delta

Add fixture coverage for setup OK, server missing, server version mismatch, no
workspace, multi-root, wrong-root artifact, stale receipt, first-pr packet
ready, first-pr packet mismatch, and preview adapter unavailable.

### Non-goals

- No analyzer novelty.
- No new editor UI surface.
- No gate, runtime proof, merge readiness, policy eligibility, or automatic
  repair claims.

### Acceptance

- Fixture artifacts pin setup diagnosis, status JSON, code actions, first-pr
  status, and receipt status.
- Unsafe states suppress repair and first-pr actions except refresh or
  diagnosis guidance.

### Proof commands

```bash
cargo xtask fixtures
cargo xtask goldens check
cargo xtask check-fixture-contracts
cargo xtask lsp-cockpit-report
cargo xtask check-pr
git diff --check
```

## Work item 6: test(vscode): smoke editor adoption assurance path

### Goal

Prove the real extension path for adoption assurance.

### Production delta

Smoke extension activation, server resolution, Diagnose Setup compatibility
state, Show Status root/artifact state, first-pr packet state, receipt state,
safe repair action gating, and wrong-root/stale/malformed suppression.

### Non-goals

- No new editor furniture.
- No analyzer, policy, PR/CI, source-edit, generated-test, provider, or
  mutation behavior.

### Acceptance

- The packaged extension path reports the same states as the fixture and LSP
  contract.
- Unsafe states fail closed in the live extension path.

### Proof commands

```bash
npm --prefix editors/vscode run compile
npm --prefix editors/vscode run test:e2e
cargo test -p ripr lsp --lib
cargo test -p ripr lsp::tests --lib
cargo xtask lsp-cockpit-report
cargo xtask check-pr
git diff --check
```

## Work item 7: docs(editor): write install-to-first-pr editor guide

### Goal

Document the user path from install/open to first-pr packet inspection.

### Production delta

Write or update the guide for Diagnose Setup, Show Status, diagnostic
inspection, related-test opening or repair packet copying, focused test,
verify, receipt, refresh, first-pr packet inspection, and PR handoff.

### Non-goals

- No release execution.
- No merge approval, gate decision, runtime proof, mutation proof, policy
  eligibility, or automatic repair claims.

### Acceptance

- Recovery states are documented for server missing, version mismatch, no
  workspace, wrong root, missing/stale artifacts, preview adapter unavailable,
  first-pr packet missing/malformed/mismatched, and receipt
  missing/stale/mismatched.

### Proof commands

```bash
cargo xtask check-doc-index
cargo xtask markdown-links
cargo xtask check-static-language
cargo xtask check-pr
git diff --check
```

## Work item 8: dogfood(lane3): record external-style editor adoption receipts

### Goal

Record proof that the editor adoption path works against external-style repo
states.

### Production delta

Record commands run, editor states observed, artifact paths, receipt state,
first-pr packet state, known limitations, and advisory boundaries for small
Rust crate, Rust workspace, no-action workspace, wrong-root artifact, stale
receipt, first-pr packet ready, first-pr packet mismatch, and preview
disabled/unavailable cases.

### Non-goals

- No behavior change.
- No new analyzer or PR/CI producer behavior.

### Acceptance

- Receipts prove success and fail-closed states.
- Known limitations and non-claims are explicit.

### Proof commands

```bash
cargo xtask lsp-cockpit-report
npm --prefix editors/vscode run compile
npm --prefix editors/vscode run test:e2e
cargo xtask check-pr
git diff --check
```

## Work item 9: campaign(lane3): close editor adoption assurance

### Goal

Close only after the adoption-assurance chain is proved.

### Production delta

Add closeout proof that maps requirements to artifacts, validation commands,
remaining limits, and future work.

### Non-goals

- No reopening editor gap cockpit, first-run usability, or first-pr bridge.
- No release/tag/publish action.

### Acceptance

- Current Lane 3 contract is pinned.
- Compatibility diagnosis exists.
- Workspace-root and multi-root diagnosis are fixture/e2e backed.
- Adoption fixtures cover success and fail-closed states.
- VS Code e2e covers the real path.
- Docs explain install-to-first-pr.
- Dogfood receipts record external-style use.
- No scope creep landed.

### Proof commands

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
