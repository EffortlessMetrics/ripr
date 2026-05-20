# Editor Actionable Gap Queue Implementation Plan

Status: active

Owner: Lane 3 - Editor / LSP UX

Linked proposal: `RIPR-PROP-0013`

Linked specs: `RIPR-SPEC-0055`

Linked ADR: `ADR-0017`

## Current State

Editor Gap Cockpit, Editor First-Run and Repair Usability, Editor First-PR
Bridge, Editor Adoption Assurance, and preview-language routing are closed.
The editor can diagnose setup, project saved-workspace gaps, copy a bounded
repair packet, show receipt state, refresh, and inspect an existing first-pr
packet.

Lane 1 now emits `target/ripr/reports/actionable-gaps.{json,md}` as the typed
source artifact for the current repair queue. Lane 3 should project that
artifact after validating it; it should not rederive the queue, produce the
artifact, or parse Markdown for action semantics.

No behavior PR should start until this source-of-truth stack lands.

## Hard Boundaries

- saved-workspace first;
- read-only projection over existing artifacts;
- typed fields over prose;
- Rust default unchanged;
- preview evidence visibly bounded;
- static limits before action language;
- fail closed on stale, wrong-root, malformed, missing, unsupported,
  unavailable, disabled, unsafe, receipt-mismatched, first-pr-mismatched, and
  actionable-packet-mismatched states;
- no analyzer truth;
- no `actionable-gaps` producer or schema changes;
- no policy, gate, badge, baseline, waiver, suppression, or default-blocking
  behavior;
- no PR comment publishing or generated CI summary composition;
- no release publishing, binary download, binary install, or config mutation;
- no source edits, generated tests, provider/model calls, mutation execution,
  CodeLens, inlays, semantic tokens, inline patches, or unsaved overlays.

## GitHub Issue Burn-Down

| Issue | Work item | Status | Evidence / remaining gap |
| --- | --- | --- | --- |
| #1298 | `docs(lane3): open editor actionable gap queue stack` | closed | Source-of-truth proposal, spec, ADR, plan, indexes, lane tracker, traceability, and capability wiring landed before behavior work. |
| #1299 | `test(lsp): pin post-adoption editor contract` | closed | Post-adoption Rust LSP projection, hover, actions, first-pr, receipt, and fail-closed behavior were pinned before queue validation. |
| #1300 | `lsp(queue): validate actionable gap packet artifacts` | active | Add read-only validation for `target/ripr/reports/actionable-gaps.json`. |
| #1301 | `lsp(queue): project repair queue in Show Status` | planned | Show bounded queue summary and no-action/fail-closed states. |
| #1302 | `lsp(queue): add Copy Current Repair Packet` | planned | Copy one bounded packet only for validated actionable gaps. |
| #1303 | `lsp(queue): add Copy Repo Gap Map` | planned | Copy read-only orientation without gate/runtime/policy claims. |
| #1304 | `fixtures(editor): add actionable gap queue corpus` | planned | Add success and fail-closed fixture cases. |
| #1305 | `test(vscode): smoke actionable gap queue` | planned | Prove the packaged extension path. |
| #1306 | `docs(editor): document actionable gap queue` | planned | Explain workflow, recovery states, and non-claims. |
| #1307 | `dogfood(lane3): record actionable gap queue receipts` | planned | Record queue, packet, receipt, no-action, and fail-closed proof. |
| #1308 | `campaign(lane3): close editor actionable gap queue` | planned | Close after behavior, fixtures, e2e, docs, dogfood, and validation land. |

## Work Item 1: docs(lane3): open editor actionable gap queue stack

### Goal

Define the campaign and GitHub issue burn-down without changing behavior.

### Production Delta

Add proposal, spec, ADR, plan, lane tracker state, indexes, documentation
links, traceability entries, and capability planning rows.

### Evidence Delta

Source-of-truth docs name the consumed artifact, required typed fields,
fail-closed matrix, status shape, command contracts, fixture corpus, VS Code
smoke expectations, dogfood proof, and non-claims.

### Non-Goals

- No LSP or VS Code behavior changes.
- No analyzer changes.
- No `actionable-gaps` producer or output schema changes.
- No PR/CI, policy, gate, release, source-edit, generated-test, provider, or
  mutation behavior.

### Acceptance

- The repo states why the queue exists, what behavior it will own, what
  architecture boundary applies, and what PR sequence follows.
- The issue burn-down is linked from the durable plan.
- Traceability points only at docs for this source-of-truth PR.
- The Lane 3 tracker says no behavior PR should start until this stack lands.

### Proof Commands

```bash
cargo xtask check-spec-format
cargo xtask check-doc-index
cargo xtask markdown-links
cargo xtask check-static-language
cargo xtask check-doc-roles
cargo xtask check-traceability
cargo xtask check-capabilities
cargo xtask check-pr
git diff --check
```

### Rollback

Revert the docs stack and close the newly created GitHub issues as not
planned.

### Stop Conditions

- Required queue safety would require a new upstream `actionable-gaps` field in
  this docs-only PR.
- Behavior, schema, PR/CI, policy, gate, source-edit, generated-test, provider,
  mutation, or release changes become necessary.

## Work Item 2: test(lsp): pin post-adoption editor contract

### Goal

Pin the closed Lane 3 contract before queue projection behavior changes.

### Production Delta

Add or harden tests for Diagnose Setup, Show Status, Rust diagnostics, preview
labels, static-limit ordering, first repair packet, first-pr packet projection,
receipt projection, wrong-root fail-closed behavior, stale fail-closed
behavior, direct repair no-active-editor guard, and multi-root fail-closed
behavior.

### Evidence Delta

Tests prove current behavior before queue validation and projection land.

### Non-Goals

- No queue validation yet.
- No new actions.
- No behavior change unless tests reveal real drift.

### Acceptance

- Rust default behavior remains unchanged.
- Preview evidence remains bounded.
- Existing unsafe states still fail closed.

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

### Stop Conditions

- Tests require queue behavior before the validation seam exists.
- Tests reveal behavior drift that needs a separate bugfix PR.

## Work Item 3: lsp(queue): validate actionable gap packet artifacts

### Goal

Create the safe input seam for `target/ripr/reports/actionable-gaps.json`.

### Production Delta

Validate schema support, workspace root, canonical gap id, language, repair
route, verify command, workspace-local paths, static-limit kind, confidence
basis, and artifact freshness.

### Evidence Delta

Tests cover valid, missing, malformed, unsupported-schema, wrong-root,
missing-identity, unsafe-path, unsafe-command, and stale-packet states.

### Non-Goals

- No rendering.
- No new VS Code command.
- No upstream schema change.

### Acceptance

- Valid packets become safe internal inputs.
- Unsafe packets fail closed with structured reasons.
- No actionability is inferred from prose.

### Proof Commands

```bash
cargo test -p ripr lsp --lib
cargo test -p ripr lsp::tests --lib
cargo xtask lsp-cockpit-report
cargo xtask check-output-contracts
cargo xtask check-pr
git diff --check
```

### Rollback

Revert validator code and tests. Existing editor behavior remains.

### Stop Conditions

- Required action safety depends on a missing upstream typed field.
- Validation requires parsing Markdown.

## Work Item 4: lsp(queue): project repair queue in Show Status

### Goal

Make Show Status answer what is safe to work on next.

### Production Delta

Show a bounded queue summary with actionable count, top repair, related test,
verify command, receipt state, report-only count, static-limit-only count, and
next safe action when a validated packet exists.

### Evidence Delta

Tests cover top repair, no-action, report-only, static-limit-only, stale,
wrong-root, malformed, receipt improved, and receipt unchanged states.

### Non-Goals

- No new copy command.
- No gate, runtime, policy, or merge-readiness claim.
- No independent queue re-ranking.

### Acceptance

- Show Status uses upstream queue order.
- No-action and fail-closed states are legible.
- Unsafe states suppress repair actions and offer safe guidance.

### Proof Commands

```bash
cargo test -p ripr lsp --lib
cargo test -p ripr lsp::tests --lib
cargo xtask lsp-cockpit-report
cargo xtask check-pr
git diff --check
```

### Rollback

Revert status projection code and tests.

### Stop Conditions

- Status copy would imply gate status, runtime proof, mutation proof, policy
  eligibility, or merge readiness.

## Work Item 5: lsp(queue): add Copy Current Repair Packet

### Goal

Give humans and coding agents one bounded local repair packet.

### Production Delta

Add `ripr: Copy Current Repair Packet` when a validated actionable gap has a
canonical id, repair route, safe verify command, fresh packet, safe paths, and
safe command payloads.

### Evidence Delta

Tests pin packet sections: Task, Context, Repair, Verification, Receipt, Stop
conditions, and Do not do.

### Non-Goals

- No source edits.
- No generated tests.
- No provider/model calls.
- No automatic repair.

### Acceptance

- The action appears only for validated actionable gaps.
- Report-only, static-limit-only, preview-unavailable, stale, wrong-root,
  malformed, unsupported, missing-verify, unsafe-command, unsafe-path, and
  identity-mismatched states suppress the action.

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

Revert command/action wiring and tests.

### Stop Conditions

- The packet would need to infer missing repair guidance from prose.

## Work Item 6: lsp(queue): add Copy Repo Gap Map

### Goal

Provide read-only orientation over the current queue.

### Production Delta

Add `ripr: Copy Repo Gap Map` with top actionable gaps, report-only and
static-limit groups, receipt states, preview boundaries, refresh/regeneration
commands, and non-claims.

### Evidence Delta

Tests pin orientation copy and forbid gate/runtime/mutation/policy language.

### Non-Goals

- No repair packet for report-only entries.
- No gate or merge-readiness language.
- No dashboard UI.

### Acceptance

- The repo map is available only from safe validated state or safe
  read-only partial state.
- The map never implies gate pass/fail, runtime proof, mutation proof, policy
  eligibility, or merge readiness.

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

Revert command/action wiring and tests.

### Stop Conditions

- The repo map starts acting as a policy/gate surface.

## Work Item 7: fixtures(editor): add actionable gap queue corpus

### Goal

Pin queue success and fail-closed states as fixtures.

### Production Delta

Add fixture cases:

- `setup_ok`;
- `top_gap_ready`;
- `multiple_gaps_bounded`;
- `no_actionable_gap`;
- `report_only_static_limit`;
- `stale_actionable_packet`;
- `wrong_root_packet`;
- `malformed_packet`;
- `receipt_improved`;
- `receipt_unchanged`.

Expected artifacts should include `vscode-status.json`,
`lsp-code-actions.json`, `current-repair-packet.md`, `repo-gap-map.md`, and
`receipt-status.json` where applicable.

### Evidence Delta

Fixtures pin positive and negative queue projection behavior.

### Non-Goals

- No analyzer novelty.
- No generated-test or source-edit behavior.

### Acceptance

- Every success and fail-closed state is pinned.
- No fixture implies gate pass, runtime proof, merge readiness, policy
  eligibility, mutation proof, or automatic repair.

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

### Stop Conditions

- Fixtures require behavior that has not landed yet.

## Work Item 8: test(vscode): smoke actionable gap queue

### Goal

Prove the packaged extension path for queue projection.

### Production Delta

Cover extension activation, server resolution, Show Status queue summary, Copy
Current Repair Packet, Copy Repo Gap Map, wrong-root suppression, stale
suppression, malformed suppression, receipt improved/unchanged state, Rust
default preservation, and preview boundary visibility.

### Evidence Delta

VS Code e2e proves the real extension path.

### Non-Goals

- No source edits.
- No generated tests.
- No provider/model calls.
- No mutation execution.

### Acceptance

- Copy Current Repair Packet works only for safe packets.
- Copy Repo Gap Map remains read-only orientation.
- Wrong-root, stale, and malformed states suppress repair packets.

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

### Stop Conditions

- Smoke requires hidden analysis reruns or artifact mutation from the editor.

## Work Item 9: docs(editor): document actionable gap queue

### Goal

Document the local queue workflow.

### Production Delta

Add or update user docs for:

```text
Diagnose Setup
-> Show Status
-> read current repair queue
-> Copy Current Repair Packet
-> open related test
-> verify
-> receipt
-> refresh
-> move to next gap or no-action
```

### Evidence Delta

Docs explain `actionable-gaps`, current repair packet, repo gap map, receipt
movement, no-action, static-limit-only, preview, and what the editor does not
do.

### Non-Goals

- No behavior change.
- No PR/CI publishing docs beyond handoff pointers.

### Acceptance

- Docs state non-claims: not merge approval, gate decision, runtime proof,
  mutation proof, policy eligibility, or automatic repair.
- Docs include missing, stale, wrong-root, malformed, unsupported, disabled,
  unavailable, receipt mismatch, and first-pr mismatch recovery.

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

### Stop Conditions

- Docs would need to claim behavior that has not landed.

## Work Item 10: dogfood(lane3): record actionable gap queue receipts

### Goal

Record proof across actionable, no-action, and fail-closed states.

### Production Delta

Dogfood:

- top actionable Rust gap;
- multiple actionable gaps;
- no actionable gap;
- static-limit-only queue;
- receipt improved;
- receipt unchanged;
- wrong-root packet;
- stale packet;
- preview advisory packet.

### Evidence Delta

Receipts record commands run, artifacts inspected, editor state, packet copied,
receipt state, and known limitations.

### Non-Goals

- No behavior change.
- No release/publish behavior.

### Acceptance

- Receipts prove the queue path without claiming runtime adequacy, gate
  success, policy eligibility, or merge readiness.

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

### Stop Conditions

- Dogfood exposes a behavior bug that needs a separate fix before closeout.

## Work Item 11: campaign(lane3): close editor actionable gap queue

### Goal

Close only after validation, projection, commands, fixtures, e2e, docs, and
dogfood proof land.

### Production Delta

Add closeout handoff, update lane tracker and campaign state, accept the
proposal/spec when appropriate, and record final validation evidence.

### Evidence Delta

Closeout maps requirement to artifact, validation command, status, remaining
future work, and non-claims.

### Non-Goals

- No new behavior in closeout.
- No reopening Adoption Assurance, First-PR Bridge, First-Run Usability, Gap
  Cockpit, or preview routing.

### Acceptance

- Source-of-truth stack exists.
- Post-adoption contract remains green.
- Actionable-gaps packet validation exists.
- Show Status projects queue state.
- Copy Current Repair Packet exists and is bounded.
- Copy Repo Gap Map exists and is read-only.
- Fixtures cover success and fail-closed states.
- VS Code e2e proves the real path.
- Docs explain the workflow.
- Dogfood receipts prove use.
- No analyzer, schema-producer, policy, PR/CI, release, source-edit,
  generated-test, provider, mutation, gate, CodeLens, inlay, semantic-token,
  inline-patch, or unsaved-buffer scope landed.

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

### Stop Conditions

- Required proof is missing.
- The slice accidentally includes analyzer, schema-producer, PR/CI, policy,
  gate, release, source-edit, generated-test, provider, mutation, or UI-sprawl
  behavior.
