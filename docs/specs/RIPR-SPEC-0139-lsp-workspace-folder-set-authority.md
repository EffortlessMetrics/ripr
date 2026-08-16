# RIPR-SPEC-0139: LSP Workspace-Folder Set Authority

Status: accepted

Owner: product / swarm

Created: 2026-07-22

Linked issues:

- [#2036](https://github.com/EffortlessMetrics/ripr-swarm/issues/2036) - make
  workspace-folder transitions delta-driven and epoch-safe.
- [#1577](https://github.com/EffortlessMetrics/ripr-swarm/issues/1577) -
  workspace-root and input-authority lifecycle (parent).
- [#1873](https://github.com/EffortlessMetrics/ripr-swarm/issues/1873) - real
  multi-root transition matrix (consumes this model).

Support-tier impact:

- No tier change. This spec changes how the LSP server tracks workspace
  folders and derives root transitions. It does not change any
  classification, finding set, ExposureClass, probe family, confidence score,
  `repair_packet_ready` authority, output schema version, or pass/fail
  behavior.
- Claim boundaries remain governed by the canonical ledger in
  [support tiers](../status/SUPPORT_TIERS.md).

Policy impact:

- No new crates, binaries, dependencies, parsers, runtime executors, or LSP
  servers introduced by this spec.

## Problem

The previous `workspace/didChangeWorkspaceFolders` handler discarded the
notification's `added`/`removed` delta and asked the client for the complete
current folder list, applying the answer unconditionally. Two events could
race: event A's query could return a list reflecting event B (or pre-B), and
the stale answer overwrote the current root authority. There was also no
single stored workspace-folder-set identity connecting standard LSP events,
root selection, and the #1873 acceptance matrix, and every notification
bumped the root epoch even when nothing changed, invalidating in-flight
refreshes for no reason.

## Behavior

### Stored folder-set identity

The server retains one `WorkspaceFolderSet` per session, populated at
`initialize` from `InitializeParams.workspaceFolders` or the documented
`rootUri` fallback (the same inputs the root resolution consumes). The set
tracks:

```text
ordered canonical entries (client URI spelling + normalized lexical path)
folder-set epoch
last applied event identity (a session-local sequence number)
```

Entry identity is lexical normalization only — never symlink
canonicalization — matching the existing root-identity invariant. Entries
are deduplicated and sorted by normalized path, so an equivalent folder set
in any order canonicalizes byte-identical and produces no epoch bump and no
transition. The selection reason derives from the entry count: zero entries
is `none`, one entry is `single`, more than one is `ambiguous` — the server
never falls back to the first folder. The authority state itself remains the
existing `WorkspaceRootAuthority` (`selected_single_root`,
`workspace_ambiguous`, `root_unavailable`, `root_removed`, `root_changed`).

### Delta application

`DidChangeWorkspaceFoldersParams.event.added/removed` is applied to the
stored set; the delta is never discarded. Every entry is validated before
any mutation, so a rejected event leaves the stored set untouched:

- a non-file URI is rejected (`invalid_file_uri`);
- adding a folder that is already stored, or adding the same folder twice in
  one event, is rejected (`duplicate_addition`);
- a folder appearing in both `added` and `removed` of one event is rejected
  (`contradictory_event`);
- removing a folder that is not stored (including a duplicate removal) is
  rejected (`unknown_removal`).

A rejection surfaces a typed bounded status through the existing
`WorkspaceRootAuthority::unavailable` detail plus the
`root_authority_block_reason` surface (`workspace_root_unavailable`); there
is no silent fallback. The root resolution still validates that selected or
candidate roots are absolute, accessible directories before the authority
changes.

### Authority derivation and no-op transitions

After an accepted event, the authority derives from the stored set: zero
entries yields `root_removed` (carrying the previous effective root), one
entry yields `selected_single_root`, more than one yields
`workspace_ambiguous`. A derived authority that is byte-identical to the
current one — state, effective root, candidate roots, and detail — is not a
transition: the root epoch is not bumped, analysis input is not
invalidated, and no status is published.

### Epoch-bound reconciliation

If the client answers `workspace/workspaceFolders`, the handler treats the
complete list as a separately versioned confirmation step, never as a
substitute for the delta. The folder-set epoch, the last applied event
identity, and the root epoch are captured under one lock acquisition before
the query; after the answer arrives they are re-checked under one lock, and
a mismatch drops the round-trip untouched — a newer event owns the
authority. Entry identity is the normalized path only; the stored URI
spelling is display-only, so a path-equivalent answer with a byte-different
URI spelling is a no-op. When the accepted delta changed the set, the
answer is consistency-checked against the stored set (same path set) and
never installed: a consistent answer merely confirms the set, and a
lagging, contradictory answer — for example the pre-delta list — is
dropped without mutating, so the delta-derived state stands. When the
delta did not change the set, the answer is the drift-correction path and
may replace the stored set canonically; an unparseable answer there
surfaces the typed bounded rejection. A client that cannot answer the
query keeps the delta-derived state. The derived authority is applied under
the root transition guard bound to the folder-set epoch it was derived
from, so a stale derivation can never overwrite a newer folder set.

### Transition coalescing

Root transitions keep coalescing with configuration/manifest reload under
the existing desired-input authority: the configuration pull epoch, the
deferred-pull scheduling discipline (scheduled after the transition guard is
released), and the refresh epoch guards are unchanged. Removing the active
root keeps clearing diagnostics, snapshot, and health state, and repair
authority stays quarantined behind the typed `workspace_root_removed` block
reason. Adding the first valid folder after a no-workspace state produces
exactly one transition to `selected_single_root`.

## Required Evidence

- `WorkspaceFolderSet` with canonical entries, folder-set epoch, last
  applied event identity, and validate-before-mutate delta application in
  `crates/ripr/src/lsp/state.rs`.
- Initialize-side folder-set retention in
  `crates/ripr/src/lsp/capabilities.rs`
  (`workspace_folder_set_from_initialize_params`) alongside the unchanged
  root resolution.
- Delta-driven `did_change_workspace_folders` with typed rejections, the
  epoch-bound reconciliation round-trip, and the byte-identical no-bump
  guard in `crates/ripr/src/lsp/backend.rs`.
- The `workspace_folder_transitions` framed duplex test suite in
  `crates/ripr/src/lsp/tests.rs` covering the #2036 fixture matrix, with the
  fake client answering every server-originated request.

## Non-Goals

- Simultaneous per-root servers or analyzing multiple roots at once.
- Analyzer evidence changes of any kind.
- Extension-side restart/session behavior (#2015) and the real two-root
  client fixture (#1873); those consume this model.
- Symlink canonicalization of root identity and any output schema version
  bump.
- Status payload shape changes; root state disclosure reuses the existing
  `root_state` / `candidate_roots` / `root_detail` fields.

## Acceptance Examples

### Delta applied under one stored authority

```text
Given a session initialized with workspace folder A,
when an event adds folder B,
then the stored set is {A, B},
and the authority becomes workspace_ambiguous with candidates [A, B],
and no first-folder fallback selects A.
```

### Stale reconciliation response dropped

```text
Given a session with stored set {A},
when event A adds B and its reconciliation query is still in flight,
and event B removes B and is applied from its delta,
when the stale A-era reconciliation answer claims the list is only [B],
then the answer is dropped,
and the authority remains selected_single_root at A.
```

### Lagging contradictory answer never undoes an accepted delta

```text
Given a session with stored set {A},
when an event adds B and the reconciliation answer is the lagging
pre-delta list [A],
then the answer is dropped without mutating the stored set,
and the authority becomes the delta-derived workspace_ambiguous {A, B}.
```

### Equivalent set in different order is a no-op

```text
Given an ambiguous session initialized with folders [B, A],
when an empty delta arrives and the reconciliation answer lists [A, B],
then the stored set is byte-identical,
and there is no epoch bump, no transition, and no status publish.
```

### Contradictory event rejected typed

```text
Given a session with stored set {A},
when an event lists folder C in both added and removed,
then the event is rejected with kind contradictory_event,
and the authority discloses root_unavailable with a bounded detail,
and the stored set still holds exactly {A}.
```

### First folder after none

```text
Given a session initialized with an explicit empty folder list,
when an event adds folder A and the reconciliation confirms [A],
then exactly one transition selects A as selected_single_root,
and no duplicate analysis runs.
```

## Test Mapping

- `crates/ripr/src/lsp/state.rs::tests::workspace_folder_set_canonicalizes_order_and_duplicates`
  — canonical ordering, dedup, and selection reasons.
- `crates/ripr/src/lsp/state.rs::tests::workspace_folder_set_rejects_invalid_file_uri`
  — non-file URI rejection.
- `crates/ripr/src/lsp/state.rs::tests::workspace_folder_set_apply_event_validates_before_mutating`
  — duplicate, contradictory, and unknown-removal rejections leave the set
  untouched.
- `crates/ripr/src/lsp/state.rs::tests::workspace_folder_set_apply_event_tracks_epoch_and_event_identity`
  — folder-set epoch and last applied event identity bookkeeping, including
  the no-op empty delta and the equivalent reconciliation.
- `crates/ripr/src/lsp/state.rs::tests::workspace_folder_set_reconciliation_with_equivalent_uri_spelling_is_noop`
  — path-identity reconciliation: a byte-different but path-equivalent URI
  spelling is a no-op; consistency checks are path-based.
- `crates/ripr/src/lsp/tests.rs::workspace_folder_transitions_first_folder_after_none_starts_single_fresh_transition`
  — fixture 1: one fresh transition, no duplicate analysis.
- `crates/ripr/src/lsp/tests.rs::workspace_folder_transitions_second_folder_becomes_ambiguous_without_fallback`
  — fixture 2: ambiguity, never first-folder.
- `crates/ripr/src/lsp/tests.rs::workspace_folder_transitions_ambiguous_resolves_to_remaining_folder_on_removal`
  — fixture 3.
- `crates/ripr/src/lsp/tests.rs::workspace_folder_transitions_direct_switch_lands_on_root_changed`
  — fixture 4.
- `crates/ripr/src/lsp/tests.rs::workspace_folder_transitions_remove_active_root_quarantines_repair_authority`
  — fixture 5: removal quarantines projections and repair authority.
- `crates/ripr/src/lsp/tests.rs::workspace_folder_transitions_non_active_folder_removal_keeps_ambiguous_selection`
  — fixture 6.
- `crates/ripr/src/lsp/tests.rs::workspace_folder_transitions_duplicate_and_contradictory_events_rejected_typed`
  — fixture 7.
- `crates/ripr/src/lsp/tests.rs::workspace_folder_transitions_invalid_file_uri_event_rejected_typed`
  — fixture 8.
- `crates/ripr/src/lsp/tests.rs::workspace_folder_transitions_stale_reconciliation_response_is_dropped`
  — fixtures 9 and 10: delayed and stale async completion cannot overwrite
  the newer epoch.
- `crates/ripr/src/lsp/tests.rs::workspace_folder_transitions_lagging_contradictory_reconciliation_is_dropped`
  — a lagging pre-delta reconciliation answer never undoes an accepted
  delta; a consistent answer confirms it.
- `crates/ripr/src/lsp/tests.rs::workspace_folder_transitions_equivalent_set_different_order_is_noop`
  — fixture 11.
- `crates/ripr/src/lsp/tests.rs::workspace_folder_transitions_shutdown_during_inflight_reconciliation_stops_cleanly`
  — fixture 12.
- Existing pins `framed_code_lens_refresh_follows_semantic_lens_view_changes`,
  `framed_lsp_deferred_configuration_pull_runs_after_root_transition_guard_release`,
  `framed_lsp_root_switch_repulls_scoped_to_new_root`,
  `framed_lsp_direct_root_switch_repulls_on_reselection`, and
  `stale_refresh_does_not_rollback_after_root_authority_transition` —
  behavior preservation for the reconciled transition path, the deferred
  pull, the root-switch re-pull, and the stale-refresh epoch guards.

## Implementation Mapping

- `crates/ripr/src/lsp/state.rs` — `WorkspaceFolderSet`,
  `WorkspaceFolderEntry`, `WorkspaceFolderSelection`,
  `WorkspaceFolderEventRejection` with typed kinds, validate-before-mutate
  `apply_event`, canonical `replace_from_folder_list`.
- `crates/ripr/src/lsp/capabilities.rs` —
  `workspace_folder_set_from_initialize_params`.
- `crates/ripr/src/lsp/backend.rs` — stored set on `Backend`, delta-driven
  `did_change_workspace_folders`, `reject_workspace_folder_update`,
  `apply_workspace_folder_set_authority` (folder-set-epoch-bound
  application), and the byte-identical no-bump guard in
  `apply_workspace_root_authority_locked`.

## Metrics

- Unit and integration tests listed above pass under
  `cargo test -p ripr workspace_folder_transitions`.
- `cargo xtask goldens check` remains clean: LSP transitions touch no CLI
  goldens.
- `cargo xtask check-static-language` clean: rejection and status strings
  use conservative language only.
