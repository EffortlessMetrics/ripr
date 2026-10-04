# RIPR-SPEC-0083: Check No-Scope Disclosure

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

- #1111-adjacent — Silent empty result when `ripr check` is run with no scope

Linked PRs:

- None yet

Support-tier impact:

- No tier change. This spec adds advisory disclosure output when `ripr check`
  is invoked with no analysis scope. It does not promote any feature to a
  higher support tier, does not change pass/fail authority, and does not alter
  what the analyzer classifies.
- The disclosure is additive output only. Claim boundaries remain governed by
  the canonical ledger in [support tiers](../status/SUPPORT_TIERS.md).
- Empty-result semantics remain unchanged: "No probes found" still means the
  static analyzer found no mutation exposure probes. The no-scope case is
  additionally disclosed as "nothing was analyzed."

Policy impact:

- Register this spec in `policy/doc-artifacts.toml`.
- No new crates, binaries, dependencies, parsers, runtime executors, or LSP
  servers introduced by this spec.

## Problem

When a user runs bare `ripr check` (no `--diff` or `--base`), the tool
analyzes nothing but still prints:

```
No diff-derived mutation exposure probes found.
```

Exit code is 0. A new user reads this as "clean." This is the cardinal
"silence reads as clean" sin for the no-scope case. `ripr doctor` recommends
`ripr check --base origin/main`, but `check` itself gives no guidance, so the
most natural first command a new user types produces a falsely-reassuring empty
result.

This extends the #1111 honesty theme (an empty result that misleads) to the
no-scope case.

## Behavior

### Trigger conditions

The disclosure fires when ALL of the following are true:

1. The CLI `check` command was invoked.
2. No explicit analysis scope was provided: none of `--diff` or `--base`
   was given in the CLI arguments. `--mode` is a **speed tier** on the diff
   path, not a scope provider.
3. The result is empty (zero findings).
4. Nothing was actually analyzed: the producer outcome reports zero changed
   files (`analysis_outcome.counts.changed_file_count == 0`). A resolved
   default base that analyzed changed files is a real analyzed-empty result
   and does NOT trigger the disclosure, even though no scope flags were
   typed (#4012).

The guidance does NOT fire when:

- `ripr check --diff <file>` was given and that diff had 0 probes (real result).
- `ripr check --base origin/main` was given and produced 0 probes (real result).
- A bare `ripr check` resolved a default base and analyzed changed files
  with 0 probes (real analyzed-empty result, #4012).

In those cases the existing "No diff-derived mutation exposure probes found."
message is honest and correct. The full-repo scan command
`ripr check --root . --format repo-exposure-md` uses `--format` (scope), not
`--mode` (speed), so it also suppresses the disclosure (it analyzes the whole
repo).

### Scope detection

The CLI `check()` handler tracks a `scope_explicitly_provided` boolean,
initialised to `false`. It is set to `true` when `--diff` or `--base` is
parsed from the argv. `--mode` does NOT set this flag — it is a speed tier on
the diff path, not a scope provider. After running the analysis, if the flag is
still `false`, the result is empty, the format is not repo-scope, AND the
producer outcome reports zero analyzed changed files, `output.no_scope_provided`
is set to `true`. The renderers read this field. The analyzed-file discriminator
(#4012) is authoritative: what was actually analyzed outranks what was typed.

### Human output

When `no_scope_provided` is true, a note is appended after the
"No diff-derived mutation exposure probes found." line. The note names what
was actually established (#4012):

- Established-but-empty range (a base was resolved and compared, e.g. the
  default base): the note names the compared base instead of claiming no
  scope was provided:

```
Note: `<base>...HEAD` contains no changed files, so there was nothing to analyze. The compared base was `<base>`; an empty result here means no behavior changed against it.
```

  The triage "Safe next action" likewise reads `no changed files were
  compared against `<base>`; commit a change and re-run, or add `--worktree` to
  include uncommitted tracked edits` (#5258 qualifies the remedy: the flag
  diffs tracked edits only) — the honest action
  is to change something, not to provide a scope.

- No established base at all: guidance names a placeholder for an existing
  base ref. It must not suggest `origin/main` when that ref may not exist:

```
Note: no analysis scope was provided — `ripr check` is diff-first. Run
`ripr check --base BASE` with BASE set to an existing ref to analyze your changes, or
`ripr check --root . --format repo-exposure-md` for a full-repo scan. An empty result here
does NOT mean your changed behavior is covered.
```

The note is omitted entirely when changed files were analyzed (real
analyzed-empty result). The human note is additionally suppressed while
uncommitted working-tree edits are unanalyzed: the SPEC-0112 working-tree
note owns the guidance there, since its `--base` advice would exclude the
same edit. The note does not change the exit code or pass/fail status.

### JSON output (`--json`)

When `no_scope_provided` is true, an additive `scope_disclosures` array is
emitted after `findings`. It is absent when `no_scope_provided` is false.
No schema version bump is required per the additive field policy in
[`docs/OUTPUT_SCHEMA.md`](../OUTPUT_SCHEMA.md).

No-scope example (no established base):

```json
"scope_disclosures": [
  {
    "scope_status": "no_scope_provided",
    "category": "no_scope_disclosure",
    "why": "no analysis scope provided; ripr check is diff-first; empty result does not mean changed behavior is covered; run ripr check --base BASE with BASE set to an existing ref or ripr check --root . --format repo-exposure-md"
  }
]
```

Established-but-empty range example (#4012): the `why` names the compared
base instead of claiming no scope was provided:

```json
"scope_disclosures": [
  {
    "scope_status": "no_scope_provided",
    "category": "no_scope_disclosure",
    "why": "empty range: <base>...HEAD contains no changed files; nothing was analyzed because nothing changed"
  }
]
```

When changed files were analyzed (real analyzed-empty), `scope_disclosures` is absent.
The `why` string is advisory prose, not an identity or a stable machine key;
consumers use the structured `base` and `scope_status` fields instead.

### Non-claims

- This spec does NOT change the exit code or gate authority.
- An empty result with no-scope disclosure does NOT mean the diff is safe; it
  means nothing was analyzed.
- This spec does NOT change what the analyzer classifies.
- This spec does NOT auto-run the suggested scope for the user.

## Non-Goals

- Disclosure in SARIF, GitHub, badge, or repo-exposure output formats.
- Auto-detecting which base revision to suggest.
- Changing behavior when scope is explicitly provided (even if empty).
- Runtime mutation testing, coverage measurement, or correctness claims.

## Required Evidence

- CLI arg parse result for `--diff` and `--base` flags (scope providers).
  `--mode` is a speed tier and is explicitly NOT a scope provider.
- `CheckOutput.no_scope_provided: bool` field (additive, default `false`).

## Inputs

| Input | Required? | Purpose |
| --- | --- | --- |
| CLI argv `--diff`, `--base` presence | yes | Determines `scope_explicitly_provided` signal; `--mode` is NOT a scope provider |
| Analysis result `findings.is_empty()` | yes | Disclosure only fires for empty results |

## Outputs

| Output | Schema impact | Notes |
| --- | --- | --- |
| Human text `Note:` line | None | Additive; absent when scope was provided; does not change exit code |
| JSON `scope_disclosures[]` | Additive field | Absent when scope was provided; no schema version bump |

## Acceptance Examples

1. **No-scope case (the bug)**: bare `ripr check` with no args → human output
   includes `Note: no analysis scope was provided` and guidance; JSON includes
   `scope_disclosures[0].scope_status == "no_scope_provided"`.
2. **Scope provided, empty**: `ripr check --base origin/main` produces 0 probes
   → existing "No diff-derived mutation exposure probes found." message only;
   NO `Note:` guidance; NO `scope_disclosures` in JSON.
3. **Scope provided via diff**: `ripr check --diff comment.diff` produces 0
   probes → same as case 2; no disclosure.
4. **Speed tier without scope, empty range**: `ripr check --mode fast`
   (no `--diff`/`--base`) on an established-but-empty default range →
   disclosure fires but names the compared base (`<base>...HEAD` contains no
   changed files); it does NOT claim no scope was provided (#4012). `--mode`
   is a speed tier, not a scope provider.
5. **Full-repo scan via format**: `ripr check --root . --format repo-exposure-md`
   → no guidance. `--format repo-exposure-md` triggers a repo-scope analysis;
   scope is real (not empty). This is the correct "full-repo scan" command,
   NOT `--mode fast`.
6. **Bare run that analyzed changes (#4012)**: bare `ripr check` that resolved
   a default base and analyzed changed files with 0 probes → ordinary
   analyzed-empty result: NO `Note:` guidance, NO `missing_scope` triage
   state, NO `scope_disclosures` in JSON.

## Test Mapping

- `crates/ripr/src/output/human.rs::tests::render_emits_no_scope_guidance_when_no_scope_provided_and_empty`
- `crates/ripr/src/output/human.rs::tests::render_omits_no_scope_guidance_when_scope_provided_and_empty`
- `crates/ripr/src/output/human.rs::tests::render_no_scope_guidance_uses_conservative_static_language`
- `crates/ripr/src/output/human.rs::tests::guidance_recommends_format_repo_exposure_md_not_mode_fast`
- `crates/ripr/src/output/json::tests::json_render_emits_scope_disclosures_when_no_scope_provided`
- `crates/ripr/src/output/json::tests::json_render_omits_scope_disclosures_when_scope_provided`
- `crates/ripr/src/output/json::tests::json_guidance_recommends_format_repo_exposure_md_not_mode_fast`
- `crates/ripr/tests/cli_smoke.rs::check_mode_fast_alone_on_empty_range_names_compared_base_smoke` (#4012: empty established range names the base, never claims no scope)
- `crates/ripr/tests/cli_smoke.rs::check_bare_run_on_changed_files_shows_no_scope_disclosure_smoke` (#4012: bare run that analyzed changed files emits no disclosure in any form)
- `crates/ripr/tests/cli_smoke.rs::check_default_base_with_clean_worktree_keeps_no_scope_note_only` (SPEC-0112 intent preserved: clean tree keeps disclosure-only output; wording updated to the #4012 base-naming form)
- `crates/ripr/tests/cli_smoke.rs::check_with_base_scope_does_not_show_no_scope_disclosure_smoke` (unchanged path guard)

## Implementation Mapping

- `crates/ripr/src/app.rs` — `CheckOutput::no_scope_provided` field (additive, default `false`).
- `crates/ripr/src/app/check/output_builder.rs` — sets `no_scope_provided: false` (library API always has scope).
- `crates/ripr/src/cli/commands.rs` — `scope_explicitly_provided` tracking in `check()`; sets `output.no_scope_provided = true` when no scope + empty.
- `crates/ripr/src/output/human.rs` — emits `Note:` guidance in the empty-findings branch when `no_scope_provided`.
- `crates/ripr/src/output/json/report.rs` — emits additive `scope_disclosures[]` when `no_scope_provided`.

## CI Proof

- `RUSTFLAGS="-D warnings" cargo build -p ripr -p xtask` — exit 0 each.
- `cargo test -p ripr -p xtask` — all pass including the disclosure tests.
- `cargo clippy -p ripr -p xtask --all-targets -- -D warnings` clean.
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
- Behavioral repro: (a) `ripr check` (no args) on an empty range prints the
  base-naming `Note:` in human output and `scope_disclosures` in JSON; with no
  established base it prints the legacy `Note: no analysis scope was provided`;
  (b) `ripr check --mode fast` (no diff/base) on an empty range likewise names
  the compared base — `--mode` is a speed tier, not a scope provider; (c)
  `ripr check --diff <file>` with 0 probes shows NO guidance; (d) `ripr check
  --root . --format repo-exposure-md` does a real repo scan (Scope: repo) and
  shows NO guidance; (e) bare `ripr check` that analyzed changed files with 0
  probes shows NO guidance in any form (#4012).

## Metrics

- Gate: all disclosure acceptance tests pass.
- Promote to accepted when a new-user onboarding scenario confirms the empty-
  no-scope result is no longer read as "clean."
