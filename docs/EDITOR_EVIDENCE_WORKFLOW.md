# Editor Evidence Workflow

Use this workflow when you want RIPR's saved-workspace editor evidence to guide
one focused test. The editor path keeps the loop local:

```text
diagnostic -> evidence hover -> related test or context packet
-> one focused test -> after snapshot -> verify -> receipt -> refresh
```

RIPR does not edit source, generate tests, run mutation testing from the
editor, or make runtime adequacy claims. It projects static evidence and exact
commands so a human or external agent can do the work deliberately.

## 1. Install And Start

Install `EffortlessMetrics.ripr` from VS Code Marketplace or Open VSX, then
open a Rust workspace with a `Cargo.toml`.

Normal editor installs do not require `cargo install ripr`. The extension
resolves the server from `ripr.server.path`, bundled or cached assets, a
verified GitHub Release download, or `ripr` on `PATH`.

Check the `ripr` status bar item first. For details, run:

```text
ripr: Show Status
```

Useful states include disabled config, unresolved workspace, server
unavailable, analysis queued, analysis running, analysis complete, stale,
failed, and no actionable seam. If the status is stale, save or close the Rust
buffer and refresh before treating diagnostics as current saved-workspace
evidence.

If `target/ripr/reports/first-useful-action.json` already exists, the same
status surface includes the first useful action from that report: the selected
seam, missing discriminator, target or related test, verify and receipt
commands, warnings, fallback, and advisory limits. The editor projection is a
read-only view over that artifact. It does not run hidden analysis, create a
new report, edit source, generate tests, call providers, run mutation testing,
or decide gate policy.

## 2. Read The Diagnostic

Open the Problems panel or inspect the editor underline for RIPR diagnostics.
Seam diagnostics carry stable identity in `diagnostic.data`; hover and code
actions use that data instead of reconstructing identity from message text.

Prefer seam diagnostics that name a missing discriminator or weak observation.
They are the most direct path to one focused test.

## 3. Hover For Evidence

Hover the diagnostic before taking an action. When the current analysis snapshot
contains matching seam evidence, the hover explains:

- seam class and configured severity;
- reach, activation, propagation, observation, and discrimination state;
- missing discriminator or missing observation;
- related test and oracle shape when available;
- suggested test or assertion shape;
- packet, brief, after-snapshot, verify, and receipt commands;
- static evidence limits.

If the hover says the evidence is stale or incomplete, refresh analysis before
copying work commands.

## 4. Open Or Copy Related Test Context

Use the seam code actions that match the available evidence:

- `Write targeted test: open best related test` opens the closest test to
  imitate when RIPR has a related-test location.
- `Write targeted test: copy suggested assertion` copies the assertion shape
  when RIPR has one.
- `Write targeted test: copy brief` copies a focused plain-language test brief
  when the seam has related-test or assertion context.
- `Inspect seam: copy packet` copies the full seam packet.

These actions are conditional. If an action is not shown, the current snapshot
does not have enough evidence for that specific handoff.

## 5. Copy A Bounded Agent Handoff

For external agents or clipboard-driven workflows, use:

- `Agent handoff: copy packet command`;
- `Agent handoff: copy brief command`;
- `ripr.collectEvidenceContext` through the existing copy-context path.

The evidence context packet is bounded JSON. It includes seam identity,
file/range, evidence path, missing discriminator, related test, suggested test
shape, shared agent-loop commands, and static limits. It does not call a model
provider and does not authorize broad edits.

When handing it to an agent, keep the task narrow:

```text
Write one focused test for this seam.
Do not edit production code.
Use the related test or assertion shape when provided.
Run the verify command.
Return the receipt.
Stop.
```

## 6. Write One Focused Test

Write the test outside RIPR. Target the missing observation named by the hover
or packet, and imitate the related test's style when one is available.

Keep the change narrow:

- one seam;
- one missing discriminator or observation;
- one focused assertion shape;
- no unrelated refactors;
- no production edits unless the review task explicitly changes.

It is acceptable if static evidence does not move. The verify and receipt steps
make that visible without turning it into a runtime claim.

## 7. Capture The After Snapshot

After saving the test, use the editor action:

```text
Verify after test: copy after-snapshot command
```

Paste the copied command in a terminal rooted at the open workspace. The command
uses the editor's configured mode and writes the after snapshot to the shared
agent-loop artifact path.

Do not hand-edit the command unless you intentionally changed the workspace
root, mode, or artifact path.

## 8. Verify Static Movement

Use:

```text
Verify after test: copy verify command
```

Paste the copied `ripr agent verify` command after the after snapshot exists.
The verify report compares before and after static artifacts. It does not run
tests or mutation testing.

## 9. Emit The Receipt

Use:

```text
Review result: copy receipt command
```

Paste the copied `ripr agent receipt` command to write the seam receipt. The
receipt records static before/after artifact identity, selected seam, class
movement, and next-action guidance. It is the reviewer-facing trail for what
changed in RIPR evidence.

## 10. Refresh The Editor

Save the Rust buffer, then use:

```text
Refresh analysis: rerun saved-workspace check
```

or run `ripr: Restart Server` if the server state is unavailable or failed.
The refreshed diagnostics should either move, disappear, or explain why static
evidence did not change.

## Limits

The editor workflow deliberately stays narrow:

- RIPR does not generate tests.
- RIPR does not edit source.
- RIPR does not run mutation testing from the editor.
- RIPR does not make runtime adequacy claims.
- Policy state comes from existing artifacts, not editor invention.
- Unsaved-buffer overlays are not part of this saved-workspace loop.
- CodeLens, inlay hints, semantic tokens, and inline patches are out of scope
  for this workflow.

For server configuration and command inventory, see
[Editor extension](EDITOR_EXTENSION.md). For the read-only first-action report
that can appear in status, see
[First useful action workflow](FIRST_USEFUL_ACTION_WORKFLOW.md). For external
agent operation, see [LLM operator guide](LLM_OPERATOR_GUIDE.md).
