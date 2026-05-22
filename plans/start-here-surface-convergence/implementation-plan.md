# Start-Here Surface Convergence Implementation Plan

Status: active

Owner: cross-lane; Lane 4 / PR-CI and CLI surfaces lead, Lane 2 owns policy
meaning, Lane 3 consumes editor-ready state

Linked proposal: `RIPR-PROP-0011`

Linked spec: `RIPR-SPEC-0053`

Linked ADR: `ADR-0015`

## Current State

The editor cockpit, Editor Gap Cockpit, Editor First-Run Usability, and Editor
First-PR Bridge are closed. `cargo xtask first-pr` and `ripr first-pr` can
produce start-here packets. Generated CI, PR evidence, report indexes, support
tiers, preview-language docs, and receipts exist.

The next product gap is convergence: each surface should lead with the same
canonical repair unit, state names, receipt lifecycle, and non-claims.

## Hard Boundaries

- advisory by default;
- typed fields over prose;
- canonical gap and repair route before raw finding count;
- no analyzer behavior changes in docs/issue setup;
- no schema changes without a scoped spec-test-code PR;
- no generated CI blocking or default gate behavior;
- no PR comment publishing changes;
- no preview-language promotion without policy-owned proof;
- no source edits, generated tests, provider/model calls, mutation execution,
  CodeLens, inlays, semantic tokens, inline patches, or unsaved-buffer overlays.

## GitHub Issue Burn-Down

| Issue | Work item | Status |
| --- | --- | --- |
| #201 | `docs(product): open start-here surface convergence stack` | done |
| #202 | `report: align PR/CI first screen on canonical repair unit` | done |
| #203 | `cli: converge start-here command language` | done |
| #204 | `receipt: standardize receipt lifecycle state` | done |
| #205 | `output: standardize no-output and fail-closed states` | done |
| #206 | `policy(language): define preview promotion proof criteria` | done |
| #207 | `dogfood: record external-style start-here receipts` | ready |
| #208 | `campaign: close start-here surface convergence` | blocked |

## Work Item 1: docs(product): open start-here surface convergence stack

### Goal

Define the campaign and create the GitHub issue burn-down map.

### Production Delta

Add proposal, spec, ADR, implementation plan, indexes, documentation index
links, and traceability entries. Open the corresponding GitHub issues with the
`codex` label.

### Non-Goals

- No runtime behavior changes.
- No output schema changes.
- No issue closeout beyond the docs/issue stack itself.

### Acceptance

- The docs state the canonical start-here unit.
- The docs state the issue burn-down sequence.
- The docs preserve advisory, preview, policy, editor, and runtime boundaries.
- GitHub issues exist for each follow-up slice.

### Proof Commands

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

Revert the docs stack and close the newly created GitHub issues as not planned.

## Work Item 2: report: align PR/CI first screen on canonical repair unit

### Goal

Make generated CI and PR evidence lead with one canonical repair unit when one
exists.

### Acceptance

- The first screen names the same gap identity, repair route, verify command,
  receipt command, receipt state, limits, and non-claims as the first-pr packet.
- Raw finding counts appear as supporting evidence.
- Missing, stale, wrong-root, malformed, no-action, and preview-limited states
  remain explicit.

### Proof Commands

```bash
cargo test -p ripr --lib output
cargo test -p xtask first_pr
cargo xtask check-output-contracts
cargo xtask check-pr
git diff --check
```

## Work Item 3: cli: converge start-here command language

### Goal

Make `doctor`, `first-pr`, `pr-ready`, cockpit reports, and related helpers use
the same start-here and next-action vocabulary.

### Acceptance

- CLI surfaces distinguish safe next action, missing artifacts, stale evidence,
  wrong root, malformed artifact, no actionable gap, preview-limited evidence,
  verify command, receipt command, and receipt path.
- Existing command names and output contracts are preserved unless a scoped
  output-contract PR changes them.

### Proof Commands

```bash
cargo test -p ripr --lib cli
cargo test -p ripr --test cli_smoke
cargo xtask check-output-contracts
cargo xtask check-pr
git diff --check
```

## Work Item 4: receipt: standardize receipt lifecycle state

### Goal

Make receipt state consistent across CLI, PR/CI, editor projection, first-pr,
and docs.

### Acceptance

- Surfaces distinguish found, missing, stale, gap mismatch, improved,
  unchanged, and not applicable.
- Receipts remain static movement proof only.
- Stale or mismatched receipts fail closed for repair claims.

### Proof Commands

```bash
cargo test -p ripr --lib receipt
cargo test -p xtask dogfood
cargo xtask check-output-contracts
cargo xtask check-pr
git diff --check
```

## Work Item 5: output: standardize no-output and fail-closed states

### Goal

Stop non-editor surfaces from collapsing distinct "nothing happened" states.

### Acceptance

- CLI and report outputs distinguish clean, no actionable gap, missing
  artifacts, stale evidence, wrong root, disabled language, unavailable
  adapter, preview disabled, preview limited, malformed artifact, timeout
  partial, unsupported schema, unsafe path, and unsafe command where the data is
  available.
- Unsafe states offer regeneration, refresh, setup diagnosis, or manual
  inspection guidance rather than repair claims.

### Proof Commands

```bash
cargo test -p ripr --lib output
cargo test -p ripr --test cli_smoke
cargo xtask check-output-contracts
cargo xtask check-pr
git diff --check
```

## Work Item 6: policy(language): define preview promotion proof criteria

### Goal

Define the proof required before preview-language evidence can claim a stronger
tier than preview.

### Acceptance

- Criteria include fixtures, dogfood, related-test accuracy, static-limit
  taxonomy, false-positive review, false repair packet review, surface
  consistency, and policy signoff.
- TypeScript, JavaScript, and Python remain preview until a policy-owned
  promotion packet closes the criteria.
- Generated CI, editor, and CLI docs keep preview evidence advisory by default.
- JavaScript evidence that shares the TypeScript-family preview adapter remains
  JavaScript preview evidence unless a policy-owned packet explicitly names
  JavaScript or an adapter-family promotion scope.

### Proof Commands

```bash
cargo xtask check-doc-index
cargo xtask markdown-links
cargo xtask check-static-language
cargo xtask check-traceability
cargo xtask check-pr
git diff --check
```

## Work Item 7: dogfood: record external-style start-here receipts

### Goal

Prove the converged path on normal repo shapes and failure states.

### Acceptance

- Receipts cover small Rust crate, Rust workspace, no actionable gap, stale
  artifacts, disabled preview languages, preview-limited evidence, and
  malformed or missing artifacts.
- Dogfood records commands, artifact paths, state labels, known limits, and
  receipt outcomes.

### Proof Commands

```bash
cargo xtask first-pr
cargo xtask dogfood
cargo xtask check-pr
git diff --check
```

## Work Item 8: campaign: close start-here surface convergence

### Goal

Close only after the issue burn-down proves converged start-here behavior.

### Acceptance

- All GitHub issues in this plan are closed or explicitly superseded.
- PR/CI, CLI, receipts, no-output states, preview promotion criteria, and
  dogfood receipts are current.
- Closeout maps requirements to artifacts and validation commands.

### Proof Commands

```bash
cargo xtask check-doc-index
cargo xtask markdown-links
cargo xtask check-static-language
cargo xtask check-doc-roles
cargo xtask check-traceability
cargo xtask check-output-contracts
cargo xtask check-pr
git diff --check
```
