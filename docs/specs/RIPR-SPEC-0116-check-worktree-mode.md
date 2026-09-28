# RIPR-SPEC-0116: `check --worktree` Mode

Status: accepted

Owner: product / swarm

Created: 2026-06-18

Linked issues:

- [#1296](https://github.com/EffortlessMetrics/ripr-swarm/issues/1296)
- [#3183](https://github.com/EffortlessMetrics/ripr-swarm/issues/3183) - LSP
  saved-workspace adoption of the canonical worktree diff authority.

Linked PRs:

- [#1325](https://github.com/EffortlessMetrics/ripr-swarm/pull/1325) -
  implemented explicit `ripr check --base <rev> --worktree` tracked-worktree
  diff mode and dirty `doctor` guidance.

Support-tier impact:

- No tier change. `docs/status/SUPPORT_TIERS.md` remains unchanged; this adds
  an explicit CLI input mode for draft analysis and does not change
  classifications, repair-packet authority, schema version, support tiers, or
  release claims.
- The #3183 amendment reuses that accepted input mode for LSP saved-workspace
  refreshes. It changes the editor's tracked-diff source, not its support tier,
  classification language, repair authority, or unsaved-buffer policy.
- Existing committed-history modes stay compatible. `--base <rev>` without
  `--worktree` still compares committed history and keeps the
  `unanalyzed_working_tree` disclosure from RIPR-SPEC-0112.

Policy impact:

- Register this spec in `policy/doc-artifacts.toml`.
- No new dependencies, crates, binaries, workflow permissions, or network/process
  surfaces beyond the existing local `git diff` adapter.

## Problem

RIPR is a draft-time evidence tool, but the obvious first-run path still has an
awkward gap for uncommitted work. `ripr check --base HEAD` compares committed
history and therefore excludes staged/unstaged tracked edits. RIPR-SPEC-0112
made that exclusion visible, but it still tells a new user to commit or stage
before RIPR can analyze their actual draft.

The desired first-run behavior is explicit and honest:

```text
ripr check --base HEAD --worktree
```

That command should analyze the live tracked working tree against `HEAD`, so an
empty result means the working tree really has no tracked diff against the base.

## Behavior

### CLI contract

`ripr check` accepts a new `--worktree` flag.

When `--worktree` is present:

- the diff source is `git diff <base>` instead of `git diff <base>...HEAD`;
- staged and unstaged tracked edits are included;
- committed changes since `<base>` are also included;
- untracked files remain out of scope until staged or supplied through
  `--diff`;
- `--diff <file>` is rejected because a file diff and a live worktree diff are
  mutually exclusive scope sources;
- `--worktree` counts as an explicit analysis scope, so no-scope disclosure does
  not fire.

When `--worktree` is absent:

- `--base <rev>` remains committed-history mode and may emit
  `unanalyzed_working_tree`;
- `--diff <file>` remains file-based mode;
- default-base resolution is unchanged.

### Doctor guidance

When `ripr doctor --root <repo>` sees staged or unstaged tracked changes, it
recommends:

```text
ripr check --base HEAD --worktree
```

and names the boundary that untracked files remain out of scope until staged or
provided via `--diff`.

### LSP saved-workspace contract

The LSP refresh path consumes the same tracked-worktree diff source as
`check --worktree`. Both interactive save refreshes and explicit full refreshes
therefore include staged and unstaged tracked edits without requiring a commit.
The refresh scope still controls only the seam inventory: interactive refreshes
defer it and disclose `seams_deferred`, while explicit refreshes run it.

Document lifecycle and diff scope remain separate authorities. Unsaved buffers
stay quarantined and never enter analysis. Untracked files remain outside the
worktree diff until staged or supplied through an explicit diff, and the LSP
workspace-status limits note names that boundary so zero findings cannot imply
that untracked source was analyzed.

## Non-Goals

- Auto-staging, reading untracked files, or inventing a diff for untracked
  files.
- Changing default `ripr check` semantics.
- Changing `--base` committed-history semantics.
- Changing `--diff` file semantics.
- Adding or renaming output fields.
- Promoting any finding based only on worktree scope.

## Acceptance Examples

1. **Dirty tracked worktree**: a tracked Rust source file is edited after
   `HEAD`. `ripr check --base HEAD --worktree --json` emits findings for the
   changed source and does not emit `unanalyzed_working_tree`.
2. **Clean worktree**: `ripr check --base HEAD --worktree --json` emits no
   findings and no scope/unanalyzed-worktree disclosure.
3. **Committed-history compatibility**: `ripr check --base HEAD --json` with a
   dirty tracked worktree still emits `unanalyzed_working_tree: true`.
4. **File diff compatibility**: `ripr check --diff change.patch` keeps existing
   behavior; `ripr check --diff change.patch --worktree` returns an error.
5. **Doctor**: dirty tracked-worktree guidance names
   `ripr check --base HEAD --worktree`.
6. **LSP saved edit**: with an empty `HEAD...HEAD` committed diff and a tracked
   saved source edit in `git diff HEAD`, an interactive LSP diagnostic refresh
   emits diff-scoped findings while keeping the seam inventory deferred.
7. **LSP explicit refresh parity**: an explicit full refresh consumes the same
   worktree diff and differs only by running the full seam inventory.

## Required Evidence

- The CLI carries `--worktree` as an explicit internal analysis mode without
  changing the public `CheckInput` or `AnalysisOptions` struct shape.
- `analysis::diff::load_worktree_diff` uses `git diff <base>` and default-base
  resolution when no explicit base is supplied.
- CLI parser accepts `--worktree`, rejects `--diff` plus `--worktree`, and keeps
  existing `--base` / `--diff` behavior unchanged.
- Doctor dirty tracked-worktree guidance recommends the worktree command and
  untracked-only files do not trigger the tracked-edit recommendation.
- CLI smoke tests cover dirty worktree, clean worktree, and doctor guidance.
- LSP interactive and explicit refreshes share the worktree diff producer;
  refresh scope continues to decide only whether seam inventory is deferred.
- LSP workspace status names the untracked-file boundary, while document
  quarantine continues to exclude unsaved buffers from current evidence.

## Test Mapping

- `crates/ripr/tests/cli_smoke.rs::check_worktree_base_head_analyzes_uncommitted_tracked_edit`
  - dirty tracked edit produces findings and no unanalyzed-worktree disclosure.
- `crates/ripr/tests/cli_smoke.rs::check_worktree_base_head_clean_worktree_has_no_scope_or_unanalyzed_disclosure`
  - clean worktree produces no findings and no scope/unanalyzed-worktree
  disclosure.
- `crates/ripr/tests/cli_smoke.rs::doctor_recommends_worktree_check_on_dirty_worktree`
  - dirty doctor guidance names the new command.
- `crates/ripr/src/cli/commands.rs::tests::check_rejects_diff_file_plus_worktree_mode`
  - `--diff` and `--worktree` remain mutually exclusive.
- `crates/ripr/src/analysis/diff/load.rs::tests::tracked_change_detector_ignores_untracked_only_files`
  - untracked-only files do not trigger tracked-worktree guidance.
- `crates/ripr/src/analysis/diff/load.rs::tests::tracked_change_detector_detects_tracked_edit`
  - staged or unstaged tracked edits still trigger tracked-worktree guidance.
- `crates/ripr/src/analysis/diff/load.rs::tests::tracked_change_detector_ignores_parent_repo_changes_outside_root`
  - parent-repo tracked edits outside the requested root do not trigger
    tracked-worktree guidance for nested roots.
- Existing RIPR-SPEC-0112 tests continue to prove committed-history compatibility.
- `crates/ripr/src/lsp/tests.rs::lsp_saved_worktree_refresh_analyzes_uncommitted_tracked_edit`
  - proves the committed diff is empty, the tracked worktree diff contains the
    saved source file, interactive seams remain deferred, and the real LSP
    diagnostic producer emits the source diagnostic.
- `crates/ripr/src/lsp/tests.rs::framed_code_lens_refresh_follows_semantic_lens_view_changes`
  - proves the framed explicit-refresh consumer retains a worktree-derived
    semantic lens view across a repeated full refresh.
- `crates/ripr/src/lsp/tests.rs::workspace_diagnostics_include_saved_tracked_edits_and_exclude_untracked_files`
  - proves deferred LSP analysis emits diff-scoped findings for both unstaged
    and staged tracked edits without passing through full seam inventory, and
    proves a separate untracked-only workspace emits no findings or diagnostic
    batch for that source.
- `crates/ripr/src/lsp/tests.rs::framed_lsp_saved_workspace_session_serves_saved_state_across_dirty_save`
  - proves the real framed `didSave` path publishes the RIPR diagnostic on the
    changed saved source line before the later explicit full refresh, while the
    dirty pre-save document remains quarantined.

## Implementation Mapping

| Component | Location |
|---|---|
| CLI flag parse and doctor guidance | `crates/ripr/src/cli/commands.rs` |
| User help | `crates/ripr/src/cli/help/core.rs` |
| App-internal worktree check path | `crates/ripr/src/app/check.rs` |
| Analysis worktree pipeline | `crates/ripr/src/analysis/mod.rs` |
| Diff source selection | `crates/ripr/src/analysis/pipeline.rs` |
| Worktree diff loader | `crates/ripr/src/analysis/diff/load.rs` |
| LSP saved-workspace consumer | `crates/ripr/src/lsp/diagnostics.rs` |
| LSP untracked-scope disclosure | `crates/ripr/src/lsp/backend.rs` |

## CI Proof

- `cargo test -p ripr --test cli_smoke worktree`
- `cargo test -p ripr --test cli_smoke doctor_recommends_worktree_check_on_dirty_worktree`
- `cargo test -p ripr --lib check_rejects_diff_file_plus_worktree_mode`
- `cargo test -p ripr --lib tracked_change_detector`
- `cargo test -p ripr --lib lsp::tests::lsp_saved_worktree_refresh_analyzes_uncommitted_tracked_edit -- --exact`
- `cargo test -p ripr --lib lsp::tests::framed_code_lens_refresh_follows_semantic_lens_view_changes -- --exact`
- `cargo test -p ripr --lib lsp::tests::workspace_diagnostics_include_saved_tracked_edits_and_exclude_untracked_files -- --exact --nocapture --test-threads=1`
- `cargo test -p ripr --lib lsp::tests::framed_lsp_saved_workspace_session_serves_saved_state_across_dirty_save -- --exact --nocapture --test-threads=1`
- Each fully qualified exact LSP selector above must report `running 1 test`;
  a zero-test success is not acceptance evidence.
- `cargo test -p ripr`
- `cargo fmt --check`
- `cargo check --workspace --all-targets`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo xtask check-spec-format`
- `cargo xtask check-doc-artifacts`
- `cargo xtask check-static-language`
- `cargo xtask check-traceability`
- `cargo xtask check-output-contracts`

## Metrics

- Gate: worktree-mode CLI smoke tests pass.
- Gate: tracked-change detector unit tests pass, including untracked-only false
  recommendation protection and parent-repo dirty-state isolation.
- Gate: the LSP saved-worktree fixture has an empty committed diff, a nonempty
  tracked worktree diff, deferred seams, and a nonempty source diagnostic.
- Promote to accepted when the dirty tracked edit, clean worktree,
  committed-history compatibility, and `--diff`/`--worktree` rejection examples
  all pass in the PR proof set.
