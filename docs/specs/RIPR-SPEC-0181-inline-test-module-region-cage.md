# RIPR-SPEC-0181: Inline test-module region cage

Status: proposed

Owner: product-analysis

Created: 2026-09-29

Linked proposal:

- None yet

Linked ADRs:

- None yet

Linked plan:

- None yet

Linked issues:

- #4783 — exact existing inline test-module region authority
- #3162 — parent new-test target capability
- #3163 — file-level RepairAttempt edit cage (does not grant intra-file permission)
- #4784 — InlineUnit producer admission; consumes this cage and is out of this slice

Linked PRs:

- #4837 — inline test-module region cage

Support-tier impact:

- None. This spec adds an internal fail-closed validator. It does not promote
  a language, editor surface, gate, or public support claim.
- Claim boundaries remain governed by
  [support tiers](../status/SUPPORT_TIERS.md).

Policy impact:

- Register this spec in `policy/doc-artifacts.toml` and
  `.ripr/traceability.toml`.
- No new crates, binaries, dependencies, network allowlist rows, process
  spawners, LSP/MCP write tools, or support-tier changes.

## Problem

An inline unit test shares a physical `.rs` file with production code. A
file-level edit cage that allowed that path would turn “add one test inside an
existing test-required module” into authority to change the production owner,
imports, attributes, or adjacent modules. Inline admission needs a structural
region cage first.

## Behavior

RIPR can represent one exact existing inline test-required module as an
internal `InlineTestRegionAuthority` and validate an external before/after
edit against it.

The portable identity excludes absolute checkout spelling and volatile line
numbers. Concrete containment uses the exact source digest, header range, and
body range of the captured bytes. Cfg-test recognition is consumed from
`analysis::facts::cfg_predicates`; this cage does not add a second lexical
detector.

V1 admits only an already-existing inline module whose test-required cfg
authority is exact and unique for the requested module name. The module must
have module-item ancestry rooted at the source file or another inline module;
function-local `mod` items are not authorities. Out-of-line declarations,
generated paths (built-in naming and configured `generated_file_patterns`),
test-layout files, ambiguous duplicate names, symlink or FIFO escapes,
traversals, inner-attribute cfg-basis changes, and stale source digests fail
closed. Capture reads through a no-follow handle of the walked path.

An admitted edit is a pure insertion of test-role `fn` items (optional `use`
companions, including at more than one site) into the named body. Existing
items remain an in-order subsequence. Production bytes, module
declaration/cfg basis, and existing item text must remain unchanged. A
parseable after-file with no new test-role function is `NotARepair`, not a
completed repair.

The contract is reusable by an InlineUnit proposal and RepairAttempt. This
slice does not select a target, generate a test, apply an edit, or flip
actionability.

## Required Evidence

- `crates/ripr/src/edit_cage/inline_test_region/tests.rs` — positive insertion
  plus production, sibling/nested-module, cfg/name/visibility, stale,
  generated (built-in and configured), traversal, symlink, FIFO,
  function-local, inner-attribute, relocated-root, line-movement, non-test
  subject, and file-level-removal controls
- A file-level-only weaker oracle must accept the combined
  production-and-test edit that the region cage rejects

## Non-Goals

- no InlineUnit target producer
- no new inline test module creation
- no whole-test body or expected-value generation
- no automatic edit application
- no Integration-target redesign
- no public schema, CLI, LSP, or support-tier change

## Acceptance Examples

- Adding one `#[test]` function inside `src/lib.rs`'s existing test-required
  `mod tests` is admitted.
- Changing `price` in the same file, or adding a helper beside the module, is
  rejected as a production edit.
- Inserting into a sibling test module not named by the authority is rejected.
- A function-local `#[cfg(test)] mod tests` is not an authority; a file-level
  region stays unique beside that lookalike.
- Inserting `#![cfg(not(test))]` plus a function into the named body is
  rejected as a cfg-basis change.
- Relocating the same bytes to another checkout preserves portable identity.
- Removing the region check makes the production-and-test negative pass the
  weaker file-level oracle.

## Test Mapping

- `crates/ripr/src/edit_cage/inline_test_region/tests.rs::adding_one_test_function_inside_the_named_module_is_admitted`
- `crates/ripr/src/edit_cage/inline_test_region/tests.rs::changing_the_production_owner_in_the_same_file_is_rejected`
- `crates/ripr/src/edit_cage/inline_test_region/tests.rs::adding_a_production_helper_beside_the_test_module_is_rejected`
- `crates/ripr/src/edit_cage/inline_test_region/tests.rs::changing_module_visibility_while_adding_a_test_is_rejected`
- `crates/ripr/src/edit_cage/inline_test_region/tests.rs::inserting_into_a_sibling_test_module_not_named_by_the_authority_is_rejected`
- `crates/ripr/src/edit_cage/inline_test_region/tests.rs::inserting_into_a_nested_test_module_not_named_by_the_authority_is_rejected`
- `crates/ripr/src/edit_cage/inline_test_region/tests.rs::line_movement_after_capture_invalidates_the_old_authority`
- `crates/ripr/src/edit_cage/inline_test_region/tests.rs::macro_body_is_not_an_observed_inline_region`
- `crates/ripr/src/edit_cage/inline_test_region/tests.rs::use_only_insertion_is_not_a_completed_repair`
- `crates/ripr/src/edit_cage/inline_test_region/tests.rs::adding_a_test_and_changing_production_is_the_discriminating_negative`
- `crates/ripr/src/edit_cage/inline_test_region/tests.rs::removing_the_region_check_would_let_the_production_negative_pass`
- `crates/ripr/src/edit_cage/inline_test_region/tests.rs::portable_identity_survives_relocated_roots_while_containment_stays_exact`
- `crates/ripr/src/edit_cage/inline_test_region/tests.rs::symlink_escape_cannot_redirect_the_allowed_region`
- `crates/ripr/src/edit_cage/inline_test_region/tests.rs::function_local_cfg_test_module_is_not_a_region`
- `crates/ripr/src/edit_cage/inline_test_region/tests.rs::file_level_region_stays_unique_beside_a_function_local_lookalike`
- `crates/ripr/src/edit_cage/inline_test_region/tests.rs::inner_cfg_not_test_plus_a_function_is_rejected`
- `crates/ripr/src/edit_cage/inline_test_region/tests.rs::configured_generated_pattern_rejects_a_non_conventional_path`
- `crates/ripr/src/edit_cage/inline_test_region/tests.rs::fifo_at_the_captured_path_is_rejected_without_blocking`

## Implementation Mapping

- `crates/ripr/src/edit_cage/inline_test_region/mod.rs` — authority, capture, portable identity
- `crates/ripr/src/edit_cage/inline_test_region/observe.rs` — parser-backed unique region observation
- `crates/ripr/src/edit_cage/inline_test_region/validate.rs` — before/after containment and insertion law

## Metrics

No new product metric; this slice is an internal fail-closed control. Proof
is `unit_test_pass_rate` on the focused lib tests.
