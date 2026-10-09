# RIPR-SPEC-0188: Windows trailing-component root disclosure

Status: proposed

Owner: product / cli

Created: 2026-09-30

Linked proposal:

- None yet

Linked ADRs:

- None yet

Linked plan:

- None yet

Linked issues:

- #4951 — Windows trailing dot/space roots conflate refusal causes;
  `--write-artifact` silently strips the suffix
- #4958 — Windows interior trailing-dot/space components rebind silently;
  disclosure covers only the final component

Linked PRs:

- #4956 — final-component refusal family and rebind note (#4951)
- #4961 — interior-component coverage of the same disclosures (#4958)

Support-tier impact:

- None. Windows-only stderr disclosures at command startup; no support-tier
  change, no analysis classification change, and no JSON schema change. Claim
  boundaries remain governed by the canonical ledger in
  [support tiers](../status/SUPPORT_TIERS.md).

Policy impact:

- Register this spec in `policy/doc-artifacts.toml`.
- Add a `[[behavior]]` row for this spec to `.ripr/traceability.toml` mapping
  the `commands_context.rs` test family.
- Stderr-only message family; no output-contract or schema surface changes.

## Problem

Win32 path normalization strips trailing dots and spaces from every component
of a path, so a root typed as `x.` addresses a different existing entry — the
sibling `x` — than the one the user typed. Before this family landed, both
shapes were dishonest in opposite directions (#4951, #4958):

- A rebound root that existed as a directory passed validation silently:
  `ripr check --root <T>/x.` analyzed `<T>/x` and attributed the findings to a
  path the user never typed.
- A rebound root whose normalized entry was not a directory, or did not exist,
  failed with the generic `is not a directory` refusal, conflating "the typed
  name resolved to a different existing entry" with "nothing is there".

The rebind case is the worse half: silently substituting the analyzed subject
is a wrong actionable signal, which is worse than a missed advisory finding.

## Behavior

On Windows, command-root validation (`ensure_command_root`) detects typed
components carrying trailing dots or spaces over the whole path — final and
interior components; prefix, root, `.` and `..` components carry no strippable
name. Detection is syntactic and platform-scoped: off Windows nothing is
detected and no Windows-specific disclosure fires. Three disclosure shapes:

1. **Rebind note (root exists as a directory).** The command is accepted and
   proceeds, and stderr names the resolved on-disk path the run will address
   (`<command> root <typed> rebinds to <resolved> ... (Windows strips trailing
   dots and spaces from path components); the run addresses the rebound
   path.`). The rebind is disclosed, never silent.
2. **Resolves-to-non-directory refusal.** Metadata resolved under the typed
   name, so it addresses a different existing entry that is not a directory:
   the refusal names that resolution — `<typed> resolves to <resolved>, which
   is not a directory on this platform (Windows strips trailing dots and
   spaces); pass a path without them` — instead of the generic refusal.
3. **NotFound refusal naming the normalized form.** Metadata is NotFound, so
   the normalized name genuinely does not exist: the refusal says the typed
   spelling cannot be addressed as typed and names the normalization. When
   only the final component strips, it names the single normalized name; when
   an interior component strips, it names every stripped component and its
   normalized form in typed order (`"x." normalizes to "x"`). NotFound-only:
   any other metadata error kind (permissions, ...) leaves existence unknown,
   so it keeps the generic refusal instead of claiming absence.

Boundary: a root with no stripped component keeps the generic behavior on
every platform — plain names are accepted as typed or refused generically,
with no Windows text. The `--write-artifact` trailing-component refusal is a
sibling consumer of the same detection/normalization authority owned in
`check.rs`; it is not owned by this spec.

## Non-Goals

- No path rewriting or silent coercion of user input: the only case that
  proceeds past validation is the disclosed rebind, and it proceeds to the
  addressable path it names.
- No change off Windows, and no change for plain names on Windows.
- No JSON, schema, badge, gate, or output-contract change (stderr-only).
- The `--write-artifact` name refusal is a separate surface, not owned here.

## Required Evidence

- Windows unit tests (`#[cfg(windows)]`) over real temp directories covering
  each disclosure shape for final and interior stripped components, including
  the interior+final combined NotFound ordering.
- Portable controls on every platform: plain names keep generic acceptance and
  the generic refusal (no Windows text); detection rejects plain names, `.`,
  `..`, and drive roots.
- A native Windows CLI repro (PR #4961 proof): typed `x./child` over an
  existing `<T>/x/child` prints the rebind disclosure and proceeds; typed
  `x./missing` gets the unaddressable-as-typed refusal naming the normalized
  form.

## Inputs

| Input | Required? | Purpose |
| --- | --- | --- |
| Typed command root path | yes | Validated per component for trailing dots/spaces |
| `std::fs::metadata` outcome | yes | Selects rebind / resolves-to / NotFound / generic branch |
| Platform | yes | Disclosures are Windows-only |

## Outputs

| Output | Notes |
| --- | --- |
| Stderr rebind note | Root accepted; names the resolved path actually addressed |
| Named refusal | Exit failure; names the resolution or the normalized form; repair: pass a path without trailing dots/spaces |

## Acceptance Examples

1. `check --root <T>/x.` where `<T>/x` exists as a directory → accepted,
   stderr rebind note names `<T>\x`, and the run analyzes `<T>\x`.
2. `check --root <T>/f.` where `<T>/f` exists as a file → refusal names
   `<T>\f` and the resolves-to-non-directory condition.
3. `check --root <T>/x.` where nothing exists under `<T>/x` → NotFound
   refusal names `"x"` as the normalization.
4. `check --root <T>/x./missing` where `<T>/x` exists but has no `missing` →
   refusal names `"x." normalizes to "x"` (interior stripped component).
5. `check --root <T>/x./y.` where neither exists → refusal names both
   normalizations in typed order.
6. Off Windows, or any name without trailing dots or spaces → generic
   acceptance or generic refusal, with no Windows text.

## Test Mapping

The family in `crates/ripr/src/cli/commands_context.rs` (ten `#[cfg(windows)]`
shape tests and five portable controls):

- `crates/ripr/src/cli/commands_context.rs::tests::windows_rebound_root_is_accepted_and_disclosed`
- `crates/ripr/src/cli/commands_context.rs::tests::windows_root_resolving_to_an_existing_file_names_the_condition`
- `crates/ripr/src/cli/commands_context.rs::tests::windows_unaddressable_root_names_the_normalized_name`
- `crates/ripr/src/cli/commands_context.rs::tests::windows_metadata_not_found_failure_names_the_normalized_name`
- `crates/ripr/src/cli/commands_context.rs::tests::windows_interior_rebound_root_is_accepted_and_disclosed`
- `crates/ripr/src/cli/commands_context.rs::tests::windows_interior_root_resolving_to_a_file_names_the_condition`
- `crates/ripr/src/cli/commands_context.rs::tests::windows_interior_root_missing_leaf_names_the_stripped_component`
- `crates/ripr/src/cli/commands_context.rs::tests::windows_interior_and_final_stripped_names_both_components`
- `crates/ripr/src/cli/commands_context.rs::tests::stripped_component_detection_matches_final_and_interior_components`
- `crates/ripr/src/cli/commands_context.rs::tests::normalized_path_names_the_written_spelling`
- `crates/ripr/src/cli/commands_context.rs::tests::plain_directory_root_is_accepted`
- `crates/ripr/src/cli/commands_context.rs::tests::plain_missing_root_keeps_the_generic_refusal`
- `crates/ripr/src/cli/commands_context.rs::tests::plain_existing_file_keeps_the_generic_refusal`
- `crates/ripr/src/cli/commands_context.rs::tests::plain_interior_layout_keeps_the_generic_behavior`
- `crates/ripr/src/cli/commands_context.rs::tests::stripped_component_detection_rejects_plain_names_and_directory_references`

## Implementation Mapping

- `crates/ripr/src/cli/commands_context.rs` — `ensure_command_root` (branch
  selection), `windows_stripped_components` and `is_windows_stripped`
  (detection authority), `windows_root_rebind_note` (shape 1),
  `windows_normalized_path` (normalized spelling), and `resolved_display`
  (resolved path rendered in a user-retypable form).
- `crates/ripr/src/cli/commands/check.rs`,
  `crates/ripr/src/cli/commands/agent.rs`, `crates/ripr/src/cli/rerun.rs`, and
  `crates/ripr/src/cli/commands/swarm/{ingest,queue,queue_live}.rs` — command
  entry points validating their roots through `ensure_command_root`
  (`check`, the `agent` subcommands, `rerun`, `swarm ingest`, `swarm queue`).

## Metrics

- windows_root_disclosure_shapes
