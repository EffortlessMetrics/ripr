# RIPR-SPEC-0195: Repair-attempt store identity and resolver

Status: proposed

Owner: product-swarm

Created: 2026-09-29

Linked proposal:

- None yet

Linked ADRs:

- None yet

Linked plan:

- None yet

Linked issues:

- #4797 — explicit RepairAttempt store identity and resolver
- #2927 — parent repair-attempt transaction
- #4798 — discovery/resume UX (later; not absorbed)
- #4799 — crash/retry matrix (later; not absorbed)
- #1613 — `help --json` / workflow catalog (later; not absorbed)

Linked PRs:

- #4844 — explicit RepairAttempt store identity and resolver

Support-tier impact:

- None. This spec makes durable attempt storage explicit and root-bound. It
  does not promote a language, editor surface, gate, or public support claim.
- Claim boundaries remain governed by
  [support tiers](../status/SUPPORT_TIERS.md).

Policy impact:

- Register this spec in `policy/doc-artifacts.toml` and
  `.ripr/traceability.toml`.
- No new crates, binaries, dependencies, network allowlist rows, process
  spawners, LSP/MCP write tools, or support-tier changes.

## Problem

Durable `RepairAttempt` manifests and retained artifacts live at a hardcoded
repository-local directory. Before, status, and after each join that path
independently. An operator cannot name a non-default store, and nothing proves
that an attempt ID from one location cannot resolve through another by
coincidence, CWD, newest-mtime search, or a silent fallback.

## Behavior

One typed resolver owns every supported attempt-store location.

The runtime ref carries repository/root identity, a portable repository-relative
locator, the concrete resolved path, location class
`default_repository | explicit_repository`, schema generation, filesystem
identity when the store is present, currentness, and limitations. Concrete path
spelling is for access and diagnostics; absolute checkout spelling, temporary
aliases, and presentation quoting are not canonical identity.

Resolution law:

- The repository-local default `target/ripr/repair-attempts` stays
  behavior-compatible. Default-store and legacy manifests omit the optional
  `store` field so ordinary before → after bytes stay compatible.
- An explicit store is accepted only as typed CLI/application input
  (`--store PATH` on `ripr agent repair` and `ripr agent status`) and is
  resolved once against the selected `--root`, never against process CWD
  independently of that root.
- Before, status, after, and verify consume the same resolver. No command
  independently guesses a directory, searches parent directories, sibling
  worktrees, user homes, temporary roots, or a newest-mtime folder.
- An attempt ID from one store cannot resolve through another store.
- Repository, root, store, and attempt identities remain separate.
- Relative paths are root-contained. Traversal, absolute-outside-root,
  symlink/junction escape, in-tree symlink or case-fold aliases, drive-relative
  ambiguity, and unsupported UNC spelling fail closed.
- A missing explicit store does not fall back to the default. A missing default
  store may be created on prepare, or reported missing on open.
- Prepare canonicalizes the deepest existing ancestor before creating a missing
  store, so a child under an escaping or in-tree alias parent is refused
  without external mutation.
- Nested `store.schema_version` must be `0.1`; a different generation is not
  another store identity.
- Recovery and status follow-up commands for an explicit store repeat `--store`
  / the portable locator. Status projections do not hardcode the default
  directory for an explicit-store attempt.
- An explicit store outside `target/ripr` is included in the edit cage's
  expected operational writes before baseline capture. Stores already under
  `target/ripr` stay covered by that subtree.
- Equivalent supported spellings of the default locator collapse to one
  default identity. Equivalent supported spellings of one explicit locator
  collapse to that explicit identity.
- Store identity does not prove an attempt is current, correct, or useful.

Python trust/verify and edit-cage consumers share this resolver. There is no
Python-specific or editor-specific attempt store.

## Required Evidence

- Default-store manifests omit `store` on the wire; explicit-store manifests
  retain `schema_version`, `location_class`, and the portable locator.
- Before, status, and after accept the same `--store` value and resolve it
  against `--root`.
- Discriminating tests cover default compatibility, explicit prepare/reopen,
  foreign CWD, two-repository isolation, two-store isolation, missing explicit
  non-fallback, traversal and absolute-outside-root, symlink/junction escape,
  in-tree alias and case-fold spelling, spaces and non-ASCII, Windows
  backslash identity, UNC and drive-relative refusal, file-not-directory,
  repository-root refusal, and empty locators.
- Help documents `--store` on `agent repair` and `agent status`; the parser
  accepts the same flag. `help --json` / catalog ownership stays #1613.

## Non-Goals

- Complete attempt-list or status UX (#4798)
- Crash/retry matrix (#4799)
- New attempt state, receipt family, database, global registry, or watcher
- Automatic source/test edit or verification execution
- Migration of historical attempts between stores
- `help --json` / workflow catalog change (#1613)
- Support-tier, release, publication, credential, or milestone action

## Acceptance Examples

1. Ordinary `ripr agent repair --phase before` then `--phase after` without
   `--store` writes and resumes under `target/ripr/repair-attempts/` and omits
   `store` from the manifest JSON.
2. `--store target/ripr/alt-attempts` on before and after from a new process
   shares one explicit identity; default status does not list that attempt.
3. Launching from a foreign CWD with `--root` still creates the store under
   that root.
4. The same attempt ID under two repositories, or two explicit stores under
   one repository, cannot cross-resolve.
5. `../escape`, an absolute path outside the root, a symlink/junction out of
   the root, UNC, and drive-relative spelling all fail closed.
6. A missing `--store` directory on after or status names the miss and does
   not open the default store.

## Test Mapping

- `crates/ripr/src/app/repair_attempt/store.rs::tests::default_store_is_the_repository_local_directory`
- `crates/ripr/src/app/repair_attempt/store.rs::tests::equivalent_default_spellings_share_one_identity`
- `crates/ripr/src/app/repair_attempt/store.rs::tests::explicit_store_prepares_and_reopens_without_falling_back`
- `crates/ripr/src/app/repair_attempt/store.rs::tests::missing_explicit_store_does_not_fall_back_to_default`
- `crates/ripr/src/app/repair_attempt/store.rs::tests::two_explicit_stores_stay_isolated`
- `crates/ripr/src/app/repair_attempt/store.rs::tests::two_repositories_cannot_cross_resolve_the_same_locator`
- `crates/ripr/src/app/repair_attempt/store.rs::tests::foreign_cwd_still_resolves_against_the_selected_root`
- `crates/ripr/src/app/repair_attempt/store.rs::tests::traversal_and_absolute_outside_root_reject`
- `crates/ripr/src/app/repair_attempt/store.rs::tests::spaces_and_non_ascii_round_trip`
- `crates/ripr/src/app/repair_attempt/store.rs::tests::windows_backslash_locator_is_one_identity`
- `crates/ripr/src/app/repair_attempt/store.rs::tests::unc_and_drive_relative_locators_fail_closed`
- `crates/ripr/src/app/repair_attempt/store.rs::tests::empty_explicit_locator_is_rejected`
- `crates/ripr/src/app/repair_attempt/store.rs::tests::symlink_escape_rejects`
- `crates/ripr/src/app/repair_attempt/store.rs::tests::default_manifest_identity_is_omitted`
- `crates/ripr/src/app/repair_attempt/store.rs::tests::a_file_is_not_a_store_directory`
- `crates/ripr/src/app/repair_attempt/store.rs::tests::the_repository_root_is_not_a_store`
- `crates/ripr/src/app/repair_attempt/store.rs::tests::case_fold_spelling_does_not_silently_share_identity`
- `crates/ripr/src/app/repair_attempt/store.rs::tests::in_tree_symlink_alias_rejects`
- `crates/ripr/src/app/repair_attempt/store.rs::tests::missing_child_under_escaping_symlink_parent_is_refused_without_external_mutation`
- `crates/ripr/src/app/repair_attempt/store.rs::tests::missing_child_under_in_tree_symlink_parent_is_refused`
- `crates/ripr/src/app/repair_attempt/store.rs::tests::nested_store_schema_version_mismatch_is_refused`
- `crates/ripr/src/app/repair_attempt/mod.rs::tests::explicit_store_before_and_after_share_identity_and_stay_isolated`
- `crates/ripr/src/app/repair_attempt/mod.rs::tests::cage_policy_includes_explicit_store_outside_target_ripr`
- `crates/ripr/src/app/repair_attempt/mod.rs::tests::diverged_head_recovery_repeats_explicit_store_on_follow_up_commands`
- `crates/ripr/src/app/repair_attempt/mod.rs::tests::after_phase_not_awaiting_error_names_explicit_store`
- `crates/ripr/src/app/agent_status.rs::tests::agent_status_reads_only_the_selected_store`
- `crates/ripr/src/cli/commands/agent.rs::tests::repair_after_cage_recovery_repeats_explicit_store`
- `crates/ripr/src/cli/agent.rs::tests::agent_repair_parses_explicit_store_and_rejects_empty`
- `crates/ripr/src/cli/agent.rs::tests::agent_status_parses_explicit_store_and_rejects_empty`

## Implementation Mapping

- `crates/ripr/src/app/repair_attempt/store.rs` — locator, identity, access,
  containment, and the one resolver
- `crates/ripr/src/app/repair_attempt/mod.rs` — manifest `store`, begin/open,
  inventory, load, finish, retain, and restore through that resolver
- `crates/ripr/src/app/agent_status.rs` — status inventory through the same
  resolver
- `crates/ripr/src/cli/agent.rs` — `--store` on repair and status
- `crates/ripr/src/cli/help/agent.rs` — documented `--store`
- `schemas/ripr/repair-attempt.schema.json` — optional `store` identity

## CI Proof

- Focused `cargo test -p ripr --lib repair_attempt`
- Focused `cargo test -p ripr --lib agent_status`
- `cargo xtask precommit` on the candidate

## Metrics

- `repair_attempt_store_default_resolutions` — default-store resolutions
- `repair_attempt_store_explicit_resolutions` — explicit-store resolutions
- `repair_attempt_store_containment_refusals` — containment/alias/UNC refusals

These metrics are advisory counts of resolver outcomes. They do not prove an
attempt is current, correct, or useful, and they are not a support-tier or
gate claim.
