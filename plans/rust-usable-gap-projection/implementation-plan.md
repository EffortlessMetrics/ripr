# Rust Usable Gap Projection Implementation Plan

Status: complete

Linked proposal:
[RIPR-PROP-0006](../../docs/proposals/RIPR-PROP-0006-rust-usable-gap-projection.md)

Linked specs:
[RIPR-SPEC-0045](../../docs/specs/RIPR-SPEC-0045-finding-to-gap-alignment.md)
and
[RIPR-SPEC-0046](../../docs/specs/RIPR-SPEC-0046-gap-decision-ledger.md)

Closed manifest:
[.ripr/goals/archive/2026-05-15-rust-usable-gap-projection.toml](../../.ripr/goals/archive/2026-05-15-rust-usable-gap-projection.toml)

Closeout:
[docs/handoffs/2026-05-15-rust-usable-gap-projection-closeout.md](../../docs/handoffs/2026-05-15-rust-usable-gap-projection-closeout.md)

## Objective

Make `ripr`'s Rust user-facing surfaces gap-driven and repair-driven. Raw
static findings remain supporting evidence. User interruptions consume canonical
gap records that name scope, repair route, anchor, projection eligibility,
verification command, and authority boundary.

## End State

- Gap decision ledger is the shared decision layer between raw evidence and
  projection surfaces.
- PR comments, first useful action, PR evidence ledger, RIPR Zero, badges,
  gates, LSP diagnostics, and agent packets can consume explicit `GapRecord`
  inputs.
- Presentation/output text can route to `MissingOutputContract` and
  `AddOutputGolden` when supported evidence identifies output proof as the
  repair route.
- `static_unknown` and preview-language evidence stay report-only or advisory
  unless an explicit repair route and projection eligibility say otherwise.
- Default CI blocking, branch protection, mutation execution, generated tests,
  provider calls, source edits, and preview-language policy promotion did not
  change.

## Work Items

### 1. `docs/finding-to-gap-alignment`

Goal:
Define the contract that raw findings are supporting evidence and canonical
items are the grouping unit for user-facing projection.

Production delta:
Add `RIPR-SPEC-0045`.

Evidence delta:
Spec index, traceability entry, and closeout reference.

Non-goals:
No analyzer behavior, output schema, comments, LSP, gate, badge, or CI change.

Acceptance:
Raw findings cannot directly become independent user actions by default.

Proof commands:

```bash
rtk cargo xtask check-spec-format
rtk cargo xtask check-doc-index
rtk cargo xtask check-traceability
rtk cargo xtask check-pr
```

Rollback:
Remove the spec and traceability entry.

### 2. `docs/rust-usable-gap-projection-proposal`

Goal:
Explain why Rust needs a gap projection layer and which user surfaces it must
align.

Production delta:
Add `RIPR-PROP-0006`.

Evidence delta:
Proposal index and links to `RIPR-SPEC-0045` / `RIPR-SPEC-0046`.

Non-goals:
No behavior, schema, gate, badge, PR comment, LSP, source-edit, generated-test,
provider, or mutation-execution change.

Acceptance:
The proposal names evidence class, gap decision, and projection as separate
layers and keeps preview languages advisory.

Proof commands:

```bash
rtk cargo xtask check-doc-roles
rtk cargo xtask check-doc-index
rtk cargo xtask markdown-links
rtk cargo xtask check-static-language
rtk cargo xtask check-pr
```

Rollback:
Remove the proposal and index entry.

### 3. `docs/gap-decision-ledger-spec`

Goal:
Define the `GapRecord` behavior contract, projection eligibility, safe gate
predicate, badge target semantics, receipt movement, and report-only states.

Production delta:
Add `RIPR-SPEC-0046`.

Evidence delta:
Traceability entry points to planned tests, fixtures, code, output contracts,
metrics, proposal, and support docs.

Non-goals:
No analyzer classification changes, mutation execution, runtime test
execution, generated tests, source edits, provider calls, default CI blocking,
branch protection, inline publishing, or preview-language promotion.

Acceptance:
The spec states that projection surfaces may display evidence context but must
not infer projectability from raw findings alone.

Proof commands:

```bash
rtk cargo xtask check-spec-format
rtk cargo xtask check-doc-index
rtk cargo xtask check-traceability
rtk cargo xtask check-pr
```

Rollback:
Remove the spec, spec index entry, and traceability mapping.

### 4. `fixtures/gap-decision-ledger-corpus`

Goal:
Pin the GapRecord vocabulary and edge cases before projection consumers depend
on it.

Production delta:
Add `fixtures/gap-decision-ledger/corpus.json` and README.

Evidence delta:
Corpus covers repairable gaps, static-limit report-only records,
presentation/output repair, missing artifacts, baseline, waiver, suppression,
preview ineligibility, and receipt movement.

Non-goals:
No analyzer truth, no generated findings, no behavior change.

Acceptance:
`cargo xtask check-fixture-contracts` accepts the manifest-only corpus and
xtask tests validate the corpus contract.

Proof commands:

```bash
rtk cargo test -p xtask gap_decision_ledger
rtk cargo xtask check-fixture-contracts
rtk cargo xtask check-pr
```

Rollback:
Remove the corpus directory and traceability links.

### 5. `output/gap-decision-ledger`

Goal:
Render a public JSON/Markdown ledger from explicit `GapRecord` input.

Production delta:
Add `ripr reports gap-ledger --records`.

Evidence delta:
Output schema docs, unit tests, traceability, and fixture corpus coverage.

Non-goals:
No hidden analysis rerun, gate authority, comment publishing, editor behavior,
source edit, generated test, provider call, or mutation execution.

Acceptance:
The command normalizes explicit gap records, reports warnings for malformed or
unsafe projection states, and keeps authority boundaries advisory.

Proof commands:

```bash
rtk cargo test -p ripr gap_decision_ledger --lib
rtk cargo xtask check-output-contracts
rtk cargo xtask check-pr
```

Rollback:
Remove the command, renderer, output-schema docs, and tests.

### 6. `report/gap-ledger-derived-inputs`

Goal:
Derive conservative gap records from existing explicit artifacts without
creating new analyzer truth.

Production delta:
Add `--repo-exposure` and `--check-output` inputs to `ripr reports gap-ledger`.

Evidence delta:
Tests cover repo-scoped Rust records and presentation/output
`MissingOutputContract` / `AddOutputGolden` records.

Non-goals:
No PR-local gate/comment claims from repo exposure, no mutation execution, no
runtime test execution, no source edits, and no generated tests.

Acceptance:
Repo exposure derivation stays repo-scoped; output derivation uses
presentation-text evidence and routes to output/golden verification only when
the evidence supports that route.

Proof commands:

```bash
rtk cargo test -p ripr reports_gap_ledger --lib
rtk cargo xtask check-output-contracts
rtk cargo xtask check-static-language
rtk cargo xtask check-pr
```

Rollback:
Remove the derived-input modes and focused tests.

### 7. `projection/gap-consumers`

Goal:
Route first-action, PR ledger, RIPR Zero, badges, gates, review comments, agent
packets, and LSP/editor surfaces through explicit gap records.

Production delta:
Add `--gap-ledger` consumers and structured projection checks in the existing
surfaces.

Evidence delta:
Focused tests prove repair-card eligibility, dedupe, safe gate predicate,
repo-scoped badge targets, agent packet validation, LSP diagnostic data, hover,
status, and bounded actions.

Non-goals:
No default CI blocking change, branch-protection change, inline publishing
change, source edit, generated test, provider call, mutation execution, or
preview-language policy promotion.

Acceptance:
Interrupting surfaces require repair route, anchor or local scope when
applicable, verification command, projection eligibility, and explicit authority
boundaries.

Proof commands:

```bash
rtk cargo test -p ripr gap_decision_ledger --lib
rtk cargo test -p ripr gate --lib
rtk cargo test -p ripr review_comments --lib
rtk cargo test -p ripr lsp --lib
rtk cargo xtask check-output-contracts
rtk cargo xtask check-capabilities
rtk cargo xtask check-pr
```

Rollback:
Remove `--gap-ledger` consumer paths and restore prior advisory projections.

### 8. `docs/rust-gap-repair-adoption`

Goal:
Make the shipped loop buyer-readable and support-tiered.

Production delta:
Add `docs/FIRST_PR_WORKFLOW.md` and promote `Rust gap repair loop` to `usable`
in support tiers and capability metadata.

Evidence delta:
Docs link to the gap ledger spec, first useful action, PR guidance, capability
matrix, and support tiers.

Non-goals:
Do not call the loop stable, runtime-confirmed, coverage-backed, or generally
correct.

Acceptance:
New adopters can follow one repairable Rust gap from ledger to repair packet,
focused proof, verification, and receipt.

Proof commands:

```bash
rtk cargo xtask check-doc-index
rtk cargo xtask markdown-links
rtk cargo xtask check-static-language
rtk cargo xtask check-capabilities
rtk cargo xtask check-pr
```

Rollback:
Remove the workflow doc and restore the prior support-tier/capability rows.

### 9. `campaign/rust-usable-gap-projection-closeout`

Goal:
Close the lane with shipped proof, non-changes, remaining limits, and restart
context.

Production delta:
Add the closeout handoff, index it, and mark `RIPR-PROP-0006` accepted.

Evidence delta:
Closeout maps every major prompt requirement to PRs, artifacts, and validation.

Non-goals:
No code or behavior changes in the closeout.

Acceptance:
The closeout records what shipped, what did not change, shipped surfaces,
validation commands, known limits, and future-lane boundaries.

Proof commands:

```bash
rtk cargo xtask check-doc-index
rtk cargo xtask markdown-links
rtk cargo xtask check-static-language
rtk cargo xtask check-doc-roles
rtk cargo xtask check-pr
rtk git diff --check
```

Rollback:
Remove the closeout and restore proposal status.

## Stop Conditions

Stop and write a blocked report instead of broadening a future PR if the work
would require:

- using raw `ExposureClass` or generic confidence as an interrupting projection
  decision;
- parsing prose to drive PR-comment or editor actions;
- changing default CI blocking, branch protection, or gate authority;
- promoting TypeScript, JavaScript, or Python preview evidence into Rust gap
  authority;
- editing source, generating tests, calling providers, or executing mutation
  testing;
- changing public schemas without the owning output contract and tests.
