# RIPR-PROP-0008: Editor First-Run Usability

Status: proposed

Owner: Lane 3 - Editor / LSP UX

Created: 2026-05-15

Target campaign: Editor first-run and repair usability

Linked specs:

- `RIPR-SPEC-0048`: Editor setup status
- `RIPR-SPEC-0049`: Editor first repair loop
- `RIPR-SPEC-0047`: Editor gap projection

Linked ADRs:

- `ADR-0013`: Editor setup diagnostics are read-only
- `ADR-0012`: Editor gap projection is read-only

Linked work items:

- To be recorded when the editor first-run usability campaign manifest opens.

## Problem

The Editor Gap Cockpit is built and closed. The editor can project RIPR
evidence, gap state, preview boundaries, related tests, repair packets, verify
commands, receipts, and refresh actions from existing saved-workspace
artifacts. That makes the cockpit technically useful, but a new user can still
get stuck before the first successful repair loop.

The most common first-run failure is uncertainty:

```text
Did the extension start?
Which ripr binary is it using?
Which workspace root is active?
Why do I see no diagnostics?
Is the evidence stale, missing, wrong-root, disabled, or preview-limited?
What is the first safe action?
What command records movement?
Where did the receipt go?
```

Lane 3 needs a focused usability campaign that makes the existing cockpit
self-orienting. The editor should explain setup, no-output states, first safe
repair actions, verification, receipt status, and limits without becoming a
new analyzer, installer, policy engine, PR publisher, source editor, provider
client, generated-test surface, or mutation runner.

## Users and surfaces

- First-time VS Code users who need to know whether RIPR is installed,
  started, configured, and reading the right workspace.
- Rust users following the first successful gap repair loop.
- TypeScript, JavaScript, and Python preview users who need disabled,
  unavailable, stale, and static-limit states to remain explicit.
- Coding agents that need one bounded repair packet with verify and receipt
  instructions.
- Maintainers who need first-run UX improvements to stay read-only and
  projection-only.

Primary surfaces:

- `ripr: Show Status`;
- a future `ripr: Diagnose Setup` command;
- LSP hover and code-action copy payloads;
- setup, no-output, and receipt-status fixtures;
- live VS Code smoke tests;
- first-run-to-first-receipt user docs;
- dogfood receipts for the first repair loop.

## Success criteria

- `ripr: Show Status` can explain server path, server version, server start
  state, workspace root, config path, enabled languages, available languages,
  artifact paths, freshness, and the next safe action when known.
- A dedicated setup diagnosis path can distinguish no workspace, server
  missing, server available, config missing, language disabled, adapter
  unavailable, artifact missing, artifact stale, no actionable gap, and
  actionable gap states.
- The first Rust repair route is visible from the editor:
  diagnostic -> hover -> related test or repair packet -> verify command ->
  receipt command -> refresh.
- Receipt status is visible when existing receipt artifacts exist:
  receipt found, receipt missing, receipt stale, receipt gap mismatch,
  movement improved, and movement unchanged.
- Stale, wrong-root, malformed, disabled, unavailable, and unsupported states
  fail closed and suppress repair actions except refresh or setup guidance.
- Preview findings remain opt-in, advisory, visibly labeled, and
  static-limit bounded; static limits appear before suggested action language.
- No editor surface claims runtime adequacy, policy eligibility, gate outcome,
  mutation confirmation, or Rust-level confidence for preview evidence.
- The campaign ships with fixtures, VS Code e2e coverage, docs, dogfood
  receipts, and closeout validation.

## Proposed shape

Open a Lane 3 campaign named Editor First-Run and Repair Usability. The work
should not add new analyzer facts or new policy surfaces. It should make the
existing editor cockpit explain itself and guide a user through one focused
Rust repair receipt:

1. define the source-of-truth docs stack;
2. add a setup diagnosis status model;
3. add a `ripr: Diagnose Setup` command;
4. smoke first-run and no-output states through VS Code;
5. link existing receipt artifacts into Show Status;
6. add a bounded "Copy first repair packet" action;
7. fixture-pin first-run usability states;
8. write the first-run-to-first-receipt guide;
9. record dogfood receipts;
10. close the campaign with validation evidence.

The editor should prefer typed fields and existing artifact identity over
prose. It may explain status, missing inputs, and next safe commands, but it
must not run hidden analysis, mutate configuration, install binaries, write
receipts, edit source, generate tests, call providers, run mutation testing,
publish PR comments, or decide gates.

## Alternatives considered

| Alternative | Why we are not picking it |
| --- | --- |
| Leave first-run diagnosis to docs only. | Users most often need the answer inside VS Code when diagnostics are absent. |
| Add more diagnostics for setup failures. | Setup state is not analyzer truth; it belongs in status and diagnosis surfaces unless an existing artifact supports a diagnostic. |
| Auto-run analysis from the extension when artifacts are missing. | That creates a hidden analyzer path and weakens saved-workspace reproducibility. |
| Install or repair the `ripr` binary from the editor. | Binary acquisition and publishing are release/package concerns; setup diagnosis can explain what is missing without mutating the system. |
| Generate a test from the repair packet. | RIPR supplies bounded test intent, verify commands, and receipt commands; source edits and generated tests are out of scope. |
| Publish the receipt or PR comment from VS Code. | PR/CI publishing belongs outside Lane 3. The editor may copy commands and show receipt status from existing artifacts. |
| Add CodeLens, inlays, semantic tokens, or unsaved-buffer overlays. | Those are separate editor campaigns with different contracts and failure modes. |

## Behavior specs to create or update

- Add `RIPR-SPEC-0048`: Editor setup status.
- Add `RIPR-SPEC-0049`: Editor first repair loop.
- Continue to reference `RIPR-SPEC-0047` for existing editor gap projection.

## Architecture decisions needed

- Add `ADR-0013` to record that setup diagnosis and first-run repair guidance
  are read-only projections over server state and existing artifacts.

## Implementation campaign shape

Each slice follows the [scoped PR contract](../SCOPED_PR_CONTRACT.md).

1. `docs/lane3-editor-first-run-usability-stack`
2. `vscode/setup-diagnosis-status-model`
3. `vscode/diagnose-setup-command`
4. `test/vscode-first-run-no-output-states`
5. `lsp/receipt-status-in-show-status`
6. `lsp/first-repair-action-packet`
7. `fixtures/editor-first-run-usability`
8. `docs/editor-first-run-to-first-receipt`
9. `dogfood/lane3-first-run-repair-receipts`
10. `campaign/lane3-editor-first-run-usability-closeout`

## Evidence plan

- Source-of-truth docs identify the proposal, setup-status spec, first-repair
  loop spec, read-only ADR, implementation plan, lane tracker state, and
  traceability entries.
- VS Code tests validate setup and no-output states for no workspace, server
  unavailable, server available, missing config, Rust-only default, disabled
  preview language, unavailable adapter, stale evidence, no actionable gap,
  and actionable gap.
- LSP tests validate that receipt status consumes only existing artifacts and fails
  closed for stale, wrong-root, malformed, or gap-mismatched receipts.
- Fixtures pin `setup_ok`, `server_missing`, `config_missing`,
  `language_disabled`, `adapter_unavailable`, `artifact_missing`,
  `artifact_stale`, `receipt_improved`, and `receipt_unchanged`.
- User docs describe install/open/status/diagnostic/repair/verify/receipt/
  refresh as one first successful Rust repair path.
- Dogfood receipts validate the path with actual artifacts.

## Risks

- Setup diagnosis could become an installer or repair wizard. Mitigation:
  ADR-0013 keeps the surface read-only and explanatory.
- No-output states could invent analyzer conclusions. Mitigation: status names
  known setup/artifact states and suppresses diagnostics when evidence is
  missing.
- Receipt status could imply movement for the wrong gap. Mitigation: receipts
  must match workspace root and gap identity before stronger status appears.
- Preview evidence could look mature. Mitigation: preview status and static
  limits stay before action language.
- Agent packets could broaden scope. Mitigation: the first repair packet
  names one gap, one language/status boundary, one related test or repair
  target, verify command, receipt command, limits, and a stop condition.
- The campaign could drift into CodeLens, inlays, generated tests, providers,
  mutation, gates, or PR comments. Mitigation: these remain explicit
  non-goals.

## Non-goals

- No analyzer behavior changes.
- No evidence schema invention in the editor.
- No output-contract changes unless a later scoped behavior PR explicitly owns
  them.
- No policy, gate, default-blocking, badge, baseline, waiver, or suppression
  changes.
- No PR comment publishing or generated CI summary composition.
- No generated tests.
- No source edits, inline patches, or automatic repair application.
- No provider or model calls.
- No runtime mutation execution.
- No binary installation or config mutation from the editor.
- No CodeLens, inlay hints, semantic tokens, or unsaved-buffer overlays.
- No preview-language promotion to Rust-level confidence.

## Exit criteria

This proposal can move to `accepted` when:

- setup diagnosis exists and is validated in VS Code;
- Show Status explains no-output states and first safe actions;
- receipt status is visible from existing artifacts and fails closed;
- the first repair packet is bounded and typed;
- fixtures cover setup, no-output, stale, disabled, unavailable, receipt
  improved, and receipt unchanged states;
- docs explain the first-run-to-first-receipt workflow;
- dogfood receipts validate one successful Rust gap repair path;
- no analyzer, policy, PR/CI publishing, source-edit, generated-test,
  provider, mutation, gate, or UI-sprawl scope landed.
