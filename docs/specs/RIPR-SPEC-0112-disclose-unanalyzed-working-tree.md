# RIPR-SPEC-0112: Disclose Unanalyzed Working Tree When Using `--base`

Status: proposed

Owner: product / swarm

Created: 2026-06-15

Linked issues:

- #1291 (check --base silently ignores uncommitted working-tree changes)

Linked PRs:

- None yet

Support-tier impact:

- No tier change. This spec adds advisory disclosure output when `ripr check
  --base <rev>` is invoked and the working tree has uncommitted changes to
  tracked source files. It does not promote any feature to a higher support
  tier, does not change pass/fail authority, and does not alter what the
  analyzer classifies.
- The disclosure is additive output only. Claim boundaries remain governed by
  the canonical ledger in [support tiers](../status/SUPPORT_TIERS.md).
- Empty-result semantics remain unchanged: "No probes found" still means the
  static analyzer found no mutation exposure probes in the analyzed diff. The
  unanalyzed-working-tree case is additionally disclosed so an empty result
  cannot be read as "the uncommitted changes are covered."

Policy impact:

- Register this spec in `policy/doc-artifacts.toml`.
- No new crates, binaries, dependencies, parsers, runtime executors, or LSP
  servers introduced by this spec.

## Problem

When a user runs `ripr check --base <rev>`, the tool diffs committed history
(`<rev>..HEAD`) via `git diff`. Uncommitted working-tree changes to tracked
source files are silently excluded from the analyzed diff.

The critical false-clean case: a developer edits `src/lib.rs` without committing,
then runs `ripr check --base HEAD`. The committed diff is empty (`HEAD..HEAD`),
so the tool outputs:

```
No diff-derived static exposure probes found.
```

Exit code is 0. There are 0 findings. The result looks clean — but the developer's
actual uncommitted edit was never analyzed. This is the cardinal "silence reads as
clean" honesty failure for the `--base` mode.

`ripr check` (no `--base`) correctly analyzes the working tree by passing the
diff through git's staging/unstaged diff, so that path is fine and unchanged.
Only the `--base` path has this gap.

## Behavior

### Trigger conditions

The disclosure fires when ALL of the following are true:

1. The CLI `check` command was invoked.
2. `--base <rev>` was explicitly provided by the user.
3. `--diff <file>` was NOT provided (file-based diff is out-of-scope for
   working-tree disclosure; only the live-repo `--base` path has the gap).
4. The working tree has at least one uncommitted change to a tracked source
   file, as detected by `git status --porcelain` returning non-empty output.

The disclosure fires independent of whether `findings.is_empty()` — an
unanalyzed working tree is worth disclosing whether or not the committed diff
had findings. The false-clean risk is highest when findings are empty (the user
sees "no probes found" and infers clean), but the disclosure is honest in both
cases.

The guidance does NOT fire when:

- `ripr check --diff <file>` was given (file-based diff; not a live worktree).
- `ripr check` was given with no `--base` (analyzes the worktree via the
  default base resolution path — that path is already correct and must not change).
- The worktree is clean (nothing uncommitted) — a clean result is honest.
- `git status --porcelain` cannot be run (fail-closed: no fabricated disclosure).

### Working-tree detection

`working_tree_has_tracked_changes(root: &Path) -> bool` runs
`git -C <root> status --porcelain` and returns `true` if stdout is non-empty.
The function lives in `crates/ripr/src/analysis/diff/load.rs`, alongside the
other git subprocess helpers (`run_git_diff`, `git_symbolic_ref_quiet`,
`git_ref_exists`). Fail-closed: if git cannot be run or returns a non-zero
exit code, the function returns `false` and no disclosure is fabricated.

### Human output

When `unanalyzed_working_tree` is true, the following note is appended:

In the empty-findings branch (after "No diff-derived static exposure probes found."):

```
Note: uncommitted changes to tracked source were not analyzed. `--base` compares
committed history only — commit or stage these changes and re-run, or analyze a
committed branch with `ripr check --base origin/main`.
```

In the non-empty-findings branch (after the all-no-path-disclosure):

```
Note: uncommitted changes to tracked source were not analyzed. `--base` compares
committed history only — commit or stage these changes and re-run, or analyze a
committed branch with `ripr check --base origin/main`.
```

The note does not change the exit code or pass/fail status.

### JSON output (`--json`)

When `unanalyzed_working_tree` is true, an additive top-level field is emitted:

```json
"unanalyzed_working_tree": true
```

Absent when `unanalyzed_working_tree` is false. No schema version bump is
required per the additive field policy in [`docs/OUTPUT_SCHEMA.md`](../OUTPUT_SCHEMA.md).

### Non-claims

- This spec does NOT change the exit code or gate authority.
- An empty result with this disclosure does NOT mean the diff is safe; it
  means the committed history had no probes AND uncommitted changes were excluded.
- This spec does NOT change what the analyzer classifies.
- This spec does NOT analyze the uncommitted changes under `--base` mode.
- This spec does NOT fire for `ripr check` (no `--base`) — that path already
  analyzes the worktree correctly.

## Non-Goals

- Disclosure in SARIF, GitHub, badge, or repo-exposure output formats.
- Auto-staging or auto-analyzing uncommitted changes under `--base`.
- Changing behavior when `--diff <file>` is used.
- Runtime mutation testing, coverage measurement, or correctness claims.
- Changing the `ripr check` (no `--base`) path in any way.

## Acceptance Examples

1. **The false-clean case (the bug)**: `ripr check --base HEAD` with an
   uncommitted `.rs` edit → human output includes the Note; JSON includes
   `"unanalyzed_working_tree": true`.
2. **Clean worktree with `--base`**: `ripr check --base HEAD` with NO
   uncommitted changes → no disclosure; result is genuinely honest.
3. **Committed diff with `--base`**: `ripr check --base HEAD~1` with committed
   changes (real findings) and a CLEAN worktree → no disclosure; result is
   honest.
4. **Worktree mode (no `--base`)**: `ripr check` (no `--base`) → no disclosure;
   this path already analyzes the worktree correctly and MUST NOT change.
5. **File diff mode**: `ripr check --diff change.diff` → no disclosure; file
   diff is not a live worktree query.

## Required Evidence

- `CheckOutput.unanalyzed_working_tree: bool` field (additive, default `false`).
- `working_tree_has_tracked_changes(root)` git subprocess helper.

## Inputs

| Input | Required? | Purpose |
| --- | --- | --- |
| CLI flag `--base` presence | yes | Determines `base_explicitly_provided` signal |
| CLI flag `--diff` absence | yes | Ensures we are in live-worktree mode, not file mode |
| `working_tree_has_tracked_changes(&root)` | yes | Detects uncommitted changes |

## Outputs

| Output | Schema impact | Notes |
| --- | --- | --- |
| Human text `Note:` line | None | Additive; absent when worktree is clean or --diff was used; does not change exit code |
| JSON `"unanalyzed_working_tree": true` | Additive field | Absent when false; no schema version bump |

## Test Mapping

- `crates/ripr/tests/cli_smoke.rs::check_base_head_with_uncommitted_edit_shows_unanalyzed_working_tree_disclosure`
- `crates/ripr/tests/cli_smoke.rs::check_base_head_with_clean_worktree_does_not_show_unanalyzed_working_tree_disclosure`

## Implementation Mapping

| Component | Location |
|---|---|
| `CheckOutput::unanalyzed_working_tree` field | `crates/ripr/src/app.rs` |
| `working_tree_has_tracked_changes` fn | `crates/ripr/src/analysis/diff/load.rs` |
| SPEC-0112 disclosure block | `crates/ripr/src/cli/commands.rs` |
| Human rendering | `crates/ripr/src/output/human.rs` |
| JSON field | `crates/ripr/src/output/json/report.rs` |

## CI Proof

- `RUSTFLAGS="-D warnings" cargo build -p ripr -p xtask` — exit 0 each.
- `cargo test --workspace` — all pass including new smoke tests.
- `cargo clippy --workspace --all-targets -- -D warnings` clean.
- `cargo fmt --check` clean.
- `cargo xtask check-static-language` pass.
- `cargo xtask check-architecture` pass.
- `cargo xtask check-no-panic-family` pass.
- `cargo xtask check-doc-artifacts` pass.
- `cargo xtask check-doc-index` pass.
- `cargo xtask check-spec-format` pass.
- `cargo xtask check-traceability` pass.
- `cargo xtask check-output-contracts` pass.
- `cargo xtask check-support-tiers` pass.
- `cargo xtask check-process-policy` pass.
- Behavioral repro: (a) `ripr check --base HEAD` with uncommitted `.rs` edit
  prints the Note and JSON `unanalyzed_working_tree: true`; (b) `ripr check
  --base HEAD` with clean worktree shows NO disclosure; (c) `ripr check` (no
  `--base`) is byte-identical to before.

## Metrics

- Gate: both smoke tests pass.
- Promote to accepted when behavioral repro confirms the false-clean case is
  closed and the clean-worktree case shows no false disclosure.
