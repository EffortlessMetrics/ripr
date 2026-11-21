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
could not resolve a default base (no origin/main, origin/master, or local main/master found). Pass `--base <ref>` to diff against a specific ref, or `--root . --mode fast` for a full-repo scan.
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

## Implementation Mapping

- `crates/ripr/src/analysis/diff/load.rs` — `resolve_default_base`,
  `git_symbolic_ref_quiet`, `git_ref_exists` helpers; modified `load_diff`
  to call `resolve_default_base` when `base` is `None`.
- `policy/process_allowlist.txt` — updated `Command::new` count for
  `load.rs` to cover the three production helpers plus test-module setup.

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
