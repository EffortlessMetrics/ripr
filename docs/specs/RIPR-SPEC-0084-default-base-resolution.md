# RIPR-SPEC-0084: Default Base Resolution

Status: proposed

Owner: product / swarm

Created: 2026-06-12

Linked proposal:

- None yet

Linked ADRs:

- None yet

Linked plan:

- None yet

Linked issues:

- #1144 — bare `ripr check` errors raw when origin/main is absent
- #4319 — `--base` beside `--diff` silently asserted on explain/context, and
  `--diff -` at an attached prompt looked like a silent hang

Linked PRs:

- None yet

Support-tier impact:

- No tier change. This spec fixes a fail-open raw git error into a
  fail-closed named actionable message, and adds smart default-base
  resolution so `ripr check` works on `master`-default, no-remote, and
  fork repos without user intervention. It does not promote any feature
  to a higher support tier, does not change pass/fail authority, and does
  not alter what the analyzer classifies.
- The resolution is transparent when a base is found (analysis runs as
  before). The named message fires only when nothing resolves — it does
  NOT claim a clean or empty analysis result. Claim boundaries remain
  governed by the canonical ledger in [support tiers](../status/SUPPORT_TIERS.md).

Policy impact:

- Register this spec in `policy/doc-artifacts.toml`.
- Update `policy/process_allowlist.txt` to reflect the new `Command::new`
  surface in `load.rs` (three production helpers + test-module setup).
- No new crates, binaries, dependencies, parsers, runtime executors, or
  LSP servers introduced by this spec.

## Problem

`ripr check` with no `--base` flag defaults to `origin/main...HEAD` as the
git diff range. In repos where `origin/main` does not exist — a fresh `git
init`, a `master`-default repo, a repo with no remote, or a fork whose
default branch differs — bare `ripr check` fails with a raw git error:

```
ripr: git diff failed: fatal: ambiguous argument 'origin/main...HEAD': unknown revision or path in the working tree.
Use '--' to separate paths from revisions...
```

This is a first-use blocker: a new user's very first `ripr check` on a
repo that does not have `origin/main` gets a confusing low-level git error
instead of guidance. `ripr doctor` recommends `ripr check --base
origin/main`, which has the same hardcoded assumption.

This extends the #1111 honesty theme (raw errors that mislead) to the
default-base-resolution case. Scoped separately from RIPR-SPEC-0083 (which
handles the empty-result-no-scope case; this handles the unresolvable-base
case).

## Behavior

### Scope of the fix

The smart resolution applies ONLY when the caller did NOT provide an
explicit `--base` flag (i.e. `base: None` at the `load_diff` call site).

When `--base X` is explicitly given and X is unresolvable, the existing git
error is kept — it names the ref the user chose and is actionable. Silent
substitution of an explicit ref would produce wrong findings without warning,
which is worse than a clear error.

The same resolution covers every command that diffs committed history
(#3952): `ripr diff`, `ripr first-pr` and `ripr pr-evidence` resolve an
omitted `--base` through `resolve_effective_base` rather than defaulting to
`origin/main`. Where nothing resolves, `first-pr` and `pr-evidence` fail
with the named message below and record no base; `first-pr` still writes its
own recovery packet when the root is missing or is not a Git work tree.

### Default-base resolution order

When base is `None`, the following candidates are tried in order. Each
candidate is verified with `git rev-parse --verify --quiet <ref>` before
use so a candidate that does not genuinely exist is never selected.

1. `git symbolic-ref --quiet refs/remotes/origin/HEAD` — the remote's own
   default-branch pointer (e.g. `refs/remotes/origin/master`). Convert to
   tracking-ref form (strip `refs/remotes/` prefix). Works for
   `master`-default, renamed, and fork repos without user configuration.
2. `origin/main` — explicit remote-tracking fallback.
3. `origin/master` — explicit remote-tracking fallback.
4. `main` — local branch fallback (no remote scenario).
5. `master` — local branch fallback (no remote scenario).

The first candidate that passes verification is used as the diff base.

### Fail-closed named message

When NONE of the above candidates resolves (e.g. a repo with no commits,
no branches, or a detached single commit with no base), `ripr check`
returns a named, actionable `Err` rather than a raw git error or a silent
empty result. The message is:

```
could not resolve a default base (no origin/main, origin/master, or local main/master found). Pass `--base <ref>` to diff against a specific ref, or run `ripr check --root . --format repo-exposure-md` for a full-repo scan.
```

This message explicitly says the analysis did not run (unlike "No probes
found", which means the analysis ran and found nothing). It names the
problem and the two remediation paths.

### Honesty bar

- A resolved base MUST genuinely exist (`git rev-parse --verify` must
  succeed). The helper never substitutes a base that does not exist.
- The named fail-closed message MUST NOT claim a clean or empty analysis
  — it says "could not resolve a base" (analysis did not run).
- An explicit bad `--base X` keeps a clear git error (user chose that ref).
  Auto-resolution does NOT fire for explicit inputs.

### Diff-input contracts (#4319)

- On the fresh-run path, `ripr explain` and `ripr context` reject an
  explicit `--base` combined with `--diff` at parse time, in either flag
  order and before any pipeline run: the loader gives `--diff` precedence
  and never validated `--base` beside it, so both flags silently analyzed
  one input while appearing to assert the other. Beside `--from`, both
  flags remain scope assertions verified against the recording
  (RIPR-SPEC-0140) and do NOT conflict.
- A `--diff -` command run with an attached terminal prints a one-line
  stderr disclosure before the loader blocks on stdin, so the documented
  `git diff origin/main | ripr check --diff -` right half alone no longer
  looks like a silent hang. The disclosure is owned by the cli adapter
  (`check`, `explain`, `context`), gated on `IsTerminal`; the analysis
  loader stays silent so library callers of the public API never receive
  CLI-branded stderr text, and piped, redirected, or captured stdin stays
  byte-identical. A shared emitter test injects the terminal state and
  captures the actual emission callback, pinning exactly one note for
  terminal stdin and none for piped stdin or file sources. This observes
  the production decision and emission path without assuming a PTY is
  available. The real terminal-detection/stderr adapter still requires a
  terminal spot-check; piped silence and successful JSON analysis are
  pinned end-to-end by a subprocess test. Its stdin writer, child completion
  and output drains share a 30-second deadline and the shared `OwnedProcess`
  termination/reap authority. Forced early exit and stalled-child controls
  exercise bounded write-error and timeout returns without unbounded thread
  joins. Those controls alone do not independently witness fixture startup
  or OS-level reaping; explicit owning cleanup is separately source-reviewed.

### Non-claims

- This spec does NOT auto-run the suggested scope or pick an arbitrary ref.
- This spec does NOT change the analysis result when `origin/main` already
  exists (the normal existing path is unaffected).
- This spec does NOT change the exit code semantics for the explicit-base
  error path.
- This spec does NOT change what the analyzer classifies.

## Required Evidence

- `git symbolic-ref --quiet refs/remotes/origin/HEAD` output for
  candidate 1.
- `git rev-parse --verify --quiet <ref>` exit code for each candidate.
- Integration tests with real temp git repos for each resolution path and
  the fail-closed path.

## Non-Goals

- Auto-fetching missing remote refs before resolving.
- Resolving non-standard branch names beyond the five candidates above.
- Changing behavior when `--base` is explicitly provided.
- Runtime mutation testing, coverage measurement, or correctness claims.

## Inputs

| Input | Required? | Purpose |
| --- | --- | --- |
| `base: Option<&str>` in `load_diff` | yes | Determines whether auto-resolution fires |
| `git symbolic-ref --quiet refs/remotes/origin/HEAD` | conditional | Remote default-branch pointer (candidate 1) |
| `git rev-parse --verify --quiet <ref>` | yes per candidate | Verifies each candidate before use |

## Outputs

| Output | Notes |
| --- | --- |
| Resolved base ref string | Used as the diff range base; transparent to the caller |
| Named actionable error | Returned when nothing resolves; says "could not resolve" not "found nothing" |

## Acceptance Examples

1. **`master`-default remote (origin/HEAD → master)**: `git init` +
   commit + `git update-ref refs/remotes/origin/master HEAD` +
   `git symbolic-ref refs/remotes/origin/HEAD refs/remotes/origin/master`
   → bare `ripr check` resolves `origin/master` (no raw git error).
2. **No remote, local `main` branch**: `git init` + commit, no remote refs
   → bare `ripr check` resolves local `main` (no raw git error).
3. **No commits, no remote, no branches**: `git init` only, no commit →
   bare `ripr check` returns the named actionable message ("could not
   resolve a default base"), NOT a raw `git diff failed` error.
4. **Explicit bad `--base` kept as-is**: `ripr check --base
   nonexistent-branch` → clear git error naming `nonexistent-branch`, NOT
   the auto-resolve named message (explicit ref path is untouched).
5. **Normal `origin/main` repo (unchanged path)**: existing repos with
   `origin/main` continue to work identically.

## Test Mapping

- `crates/ripr/src/analysis/diff/load.rs::tests::resolve_default_base_uses_origin_master_when_symbolic_ref_points_there`
- `crates/ripr/src/analysis/diff/load.rs::tests::resolve_default_base_uses_local_main_when_no_remote`
- `crates/ripr/src/analysis/diff/load.rs::tests::resolve_default_base_returns_named_error_when_nothing_resolves`
- `crates/ripr/src/analysis/diff/load.rs::tests::explicit_base_is_used_as_is_without_resolution`
- `crates/ripr/src/analysis/diff/load.rs::tests::load_diff_from_file_returns_content`
- `crates/ripr/tests/cli_smoke.rs::history_commands_resolve_the_default_base_without_origin`
- `crates/ripr/tests/cli_smoke.rs::history_commands_without_a_resolvable_default_base_fail_named`
- `crates/ripr/tests/cli_smoke.rs::first_pr_check_missing_packet_recovers_without_a_resolvable_base`
- `crates/ripr/tests/cli_smoke.rs::first_pr_check_recovery_write_resolves_the_default_base`
- `crates/ripr/src/cli/parse.rs::tests::base_and_diff_conflict_error_names_the_command_and_both_flags`
- `crates/ripr/src/cli/parse.rs::tests::attached_terminal_stdin_note_fires_only_for_a_terminal`
- `crates/ripr/src/cli/parse.rs::tests::terminal_stdin_disclosure_emits_once_only_for_a_terminal_diff_source`
- `crates/ripr/src/cli/commands.rs::tests::explain_rejects_base_and_diff_together_at_parse_time`
- `crates/ripr/src/cli/commands.rs::tests::explain_keeps_base_and_diff_as_from_artifact_assertions`
- `crates/ripr/src/cli/commands/context.rs::tests::context_rejects_base_and_diff_together_at_parse_time`
- `crates/ripr/src/cli/commands/context.rs::tests::context_keeps_base_and_diff_as_from_artifact_assertions`
- `crates/ripr/tests/cli_smoke.rs::check_diff_stdin_from_a_pipe_stays_silent_about_terminal_disclosure`
- `crates/ripr/tests/cli_smoke.rs::stdin_probe_stalled_child_returns_bounded_timeout`
- `crates/ripr/tests/cli_smoke.rs::stdin_probe_early_exit_returns_write_error`

## Implementation Mapping

- `crates/ripr/src/analysis/diff/load.rs` — `resolve_default_base`,
  `git_symbolic_ref_quiet`, `git_ref_exists` helpers; modified `load_diff`
  to call `resolve_default_base` when `base` is `None`.
- `policy/process_allowlist.txt` — updated `Command::new` count for
  `load.rs` to cover the three production helpers plus test-module setup.
- `crates/ripr/src/cli/parse.rs` (#4319) —
  `attached_terminal_stdin_note` pure decision, the verbatim note const,
  and the single `disclose_attached_terminal_stdin_read` emission site;
  `base_with_diff_conflict_error` parse-time conflict phrasing.
- `crates/ripr/src/cli/commands/check.rs`, `crates/ripr/src/cli/commands.rs`
  (`explain`), `crates/ripr/src/cli/commands/context.rs` (#4319) — one thin
  disclosure call each before dispatching a run that accepted `--diff -`,
  and the parse-time `--base`+`--diff` conflict gate on the fresh path.

## CI Proof

- `RUSTFLAGS="-D warnings" cargo build -p ripr -p xtask` — exit 0 each.
- `cargo test -p ripr -p xtask` — all pass including the four resolution tests.
- `cargo clippy -p ripr -p xtask --all-targets -- -D warnings` clean.
- `cargo fmt --check` clean.
- `cargo xtask check-static-language` pass.
- `cargo xtask check-architecture` pass.
- `cargo xtask check-no-panic-family` pass.
- `cargo xtask check-process-policy` pass.
- `cargo xtask check-doc-artifacts` pass.
- `cargo xtask check-doc-index` pass.
- `cargo xtask check-spec-format` pass.
- `cargo xtask check-traceability` pass.
- `cargo xtask check-output-contracts` pass.
- `cargo xtask check-support-tiers` pass.
- Behavioral repro: `git init` + commit + `refs/remotes/origin/master`
  (no origin/main) → bare `ripr check` resolves `origin/master` (no raw
  error); `git init` + commit, no remote → resolves local `main`; `git
  init` only (no commit) → named actionable message (not raw git error).

## Metrics

- Gate: all four resolution and fail-closed acceptance tests pass.
- Promote to accepted when a new-user onboarding scenario on a
  `master`-default or no-remote repo confirms bare `ripr check` no longer
  surfaces a raw git error.
