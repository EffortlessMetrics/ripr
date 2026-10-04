# RIPR-SPEC-0220: VS Code shared repair-attempt status and active-attempt resolution

Status: proposed

Owner: editor

Created: 2026-10-04

Linked proposal:

- None yet

Linked ADRs:

- None yet

Linked plan:

- None yet

Linked issues:

- #4643 — VS Code projects the shared RepairAttempt state and resolves the
  active attempt (this spec implements its acceptance contract)
- #2928 — parent editor shell (consumer; not absorbed)
- #4798 / RIPR-SPEC-0217 — the `agent_attempt_status` DTO this spec consumes
  as the semantic authority (no editor-side state machine)
- #2927 — shared durable `RepairAttempt` authority

Linked PRs:

- None yet

Support-tier impact:

- None. This spec adds a read-only editor presentation over an existing CLI
  DTO. It does not promote a language, editor surface, gate, or public
  support claim. Claim boundaries remain governed by
  [support tiers](../status/SUPPORT_TIERS.md).

Policy impact:

- Register this spec in `policy/doc-artifacts.toml` and
  `.ripr/traceability.toml`.
- No new crates, binaries, dependencies, network allowlist rows, process
  spawners, LSP/MCP write tools, or support-tier changes. The editor runs the
  already-resolved ripr executable read-only with the existing `runRipr`
  runtime seam.

## Problem

RIPR-SPEC-0217 made `ripr agent status --attempt <id>` the typed,
fail-closed, attempt-first discovery/resume surface, but the packaged editor
could not show any of it: the only attempt-adjacent surface was the
human-prose attempt ledger opened by a cockpit command. Without a typed
adapter, an editor integration would re-derive attempt state from clipboard
text, output-channel prose, Problems entries, or artifact mtimes — a second,
editor-specific state machine that drifts from the CLI authority and can
silently strengthen a stale or corrupt attempt into a green check mark.

## Behavior

The VS Code extension consumes the shared DTO; it invents no state.

- `src/attemptStatus.ts` is the only parser of `agent_attempt_status` and
  attempt-inventory documents. It binds the envelope
  (`schema_version: "0.1"`, `kind: "agent_attempt_status"` for the selected
  document; no `kind` for the inventory), requires a non-empty
  `attempt.attempt_id`, and refuses — returns `undefined`, never a softened
  projection — any `status_class` outside the ten RIPR-SPEC-0217 classes or
  any `currentness` outside `current`/`historical`/`unknown`.
- `ripr: Show Repair Attempt Status` resolves the workspace root, runs
  `ripr agent status --root <root> --json` through the existing
  `runRipr(command, args, cwd)` seam, parses rows with the adapter, applies
  the deterministic active-attempt resolution law, then reads the selected
  attempt with `ripr agent status --root <root> --attempt <id> --json` and
  presents the result.
- The resolution law: an explicit selection always wins — including an
  optional explicit attempt id argument forwarded from the command
  registration; an id absent from the inventory stays an exact query — the
  CLI reports the typed `corrupt_or_unavailable` document for it, never a
  substituted attempt; a remembered selection is honored only while it is
  still one of the inventory rows; exactly one row selects that row; several
  rows with no valid selection require an explicit quick pick, and
  dismissing the pick presents nothing and hides any previous attempt
  presentation. Newest, first folder, first row, most-recently-modified,
  and same-seam heuristics do not exist in the editor layer.
- A returned status document binds to the requested attempt identity: a
  response naming any other attempt id is refused as mismatched, never
  re-labelled. A server stop during either CLI read abandons the render so
  a dead session cannot repaint the status bar.
- The per-root explicit selection persists in `workspaceState`
  (`ripr.activeAttemptSelection.v1`) across extension restart and
  deactivation — including an explicit id that names no current inventory
  row, once its document reads back with the matching identity.
  In-memory presentation is discarded when the server session
  stops; a stale remembered id is deleted, never resurrected.
- Presentation maps each class to exactly one tone:
  `finished_current` → pass; `awaiting_edit`/`prepared` → info;
  `finished_historical`/`stale`/`incomparable`/`limited`/
  `legacy_compatibility_only` → warning; `failed`/
  `corrupt_or_unavailable` → error. A read that fails validation or CLI
  execution renders `ripr: attempt status unavailable` with a warning tone —
  never a friendly state. Tooltip and notification lines derive only from
  DTO fields (attempt id, seam, operational state, status class,
  currentness, moved-HEAD note, unreadable reason, the exact next/recovery
  action, first limitation).
- Untrusted workspaces never read repair authority: the command refuses
  before any CLI invocation, matching the extension's existing
  server-resolution trust gate.
- The command is read-only end to end: it never starts, finishes, restarts,
  rewrites, or deletes an attempt and never executes a returned route.

## Required Evidence

- `npm --prefix editors/vscode test` (extension-host suite) passes with the
  new parity, resolution-law, and command suites executing nonzero tests.
- Parity fixtures under `editors/vscode/test-fixtures/attempt-status/` cover
  all ten status classes plus a multi-attempt inventory; removing or
  bypassing `src/attemptStatus.ts` turns the suite red (compile failure or
  fixture rejection), which is the mechanical enforcement that the editor
  cannot drift onto its own vocabulary.
- Suite cases pin: per-class tone mapping (never stronger), fail-closed
  rejection of a strengthened class (`passed`) and foreign envelopes,
  no-attempt/one-attempt/several-attempts distinctness, stale remembered
  selection discard, explicit-selection precedence, unknown explicit id kept
  as an exact query, quick-pick dismissal presenting nothing,
  per-root isolation of remembered selections (a selection stored for one
  root never suppresses the picker in another), persistence of a remembered
  selection across a fresh controller reading the same `workspaceState`
  (the restart/deactivation lifecycle, since `workspaceState` is the only
  store and survives both), and the untrusted-workspace refusal with zero
  CLI invocations.
- `cargo xtask check-spec-format`, `check-spec-numbering`,
  `check-doc-artifacts`, `check-traceability`, `check-doc-index`, and
  `check-file-policy` pass on the candidate.
- `npm --prefix editors/vscode run compile` succeeds (hosted lane).

## Non-Goals

- No Start Repair, Continue, or Finish behavior (#2928 owns that journey).
- No palette/onboarding reorganization (#2928).
- No LSP protocol change: the LSP stays read-only and the editor reads the
  CLI DTO directly through the existing binary-resolution seam.
- No editor-side attempt lifecycle, state machine, repair vocabulary, or
  receipt re-derivation.
- No automatic source/test edit, verification execution, or mutation
  execution.
- No support-tier, release, marketplace, or publication action.

## Acceptance Examples

1. A workspace with exactly one awaiting attempt shows
   `ripr: attempt awaiting edit` with the recorded after command in the
   tooltip, and the inventory-plus-selected reads bind the workspace root.
2. A workspace with two current awaiting attempts shows a quick pick listing
   both exact ids; dismissing it changes nothing and presenting resumes only
   after an explicit pick, which is then remembered across an extension
   restart.
3. After HEAD moves, the same attempt shows
   `ripr: attempt finished (historical)` with a warning tone and the
   moved-HEAD note — never a pass.
4. A tampered attempt document whose `status_class` reads `passed` is
   rejected as unavailable rather than projected.
5. In an untrusted workspace the command refuses with an explanation and
   `runRipr` is never invoked.

## Test Mapping

- `editors/vscode/test/suite/attempt_status.test.ts` — suite "Agent attempt
  status adapter parity (#4643)"
- `editors/vscode/test/suite/attempt_status.test.ts` — suite "Active attempt
  resolution law (#4643)"
- `editors/vscode/test/suite/attempt_status.test.ts` — suite "Show Repair
  Attempt Status command (#4643)", including the remembered-selection
  second run that models post-restart persistence (a fresh controller reads
  the same `workspaceState`) and the cross-root isolation case.

Fixtures: `editors/vscode/test-fixtures/attempt-status/` (one
`status-<class>.json` per pinned class plus `inventory.json`).

## Implementation Mapping

- `editors/vscode/src/attemptStatus.ts` — the typed adapter, the resolution
  law, and the class→tone presentation mapping (pure, no VS Code API)
- `editors/vscode/src/client.ts` — `showAttemptStatus`, the durable
  per-root selection in `workspaceState`, the second status-bar item
  rendering, and the stop-path cleanup
- `editors/vscode/src/extension.ts` — command registration and the attempt
  status-bar item lifecycle
- `editors/vscode/package.json` — `ripr.showAttemptStatus` command and
  activation event (palette-only; no menu/keybinding changes)

## CI Proof

- Hosted extension lane: `npm --prefix editors/vscode test` and
  `npm --prefix editors/vscode run compile`
- Focused grep: `RIPR_TEST_GREP="attempt status|Active attempt|Show Repair Attempt"`
- `cargo xtask check-traceability`, `check-doc-artifacts`, `check-doc-index`,
  `check-spec-format`, `check-spec-numbering`

## Metrics

- None new. This spec projects an existing CLI DTO; it adds no counters and
  claims no measurement.
