# Friction Log

This log captures things that wasted time, surprised us, or felt "not
quite right" during day-to-day work — raw, fresh, and **not yet
distilled**. It is intentionally append-only and low-friction to write
to.

## How this differs from `docs/LEARNINGS.md`

| | Friction Log | Learnings |
|---|---|---|
| Cadence | Live, per-incident, written same day | Periodic, after a pattern is clear |
| Shape | Raw observation + suggested fix or status | Distilled insight that should shape future decisions |
| Lifecycle | Items graduate into Learnings, into a fix, or stay as known-friction | Settled |
| Bar to add | Low — "this surprised me, log it" | High — "this is now a settled principle" |
| Reader | Anyone iterating on the same surface tomorrow | Anyone making architecture / roadmap calls |

When a friction-log entry has been resolved by a code change, mark it
**resolved** with a PR/commit reference. When several entries point at
the same root cause, distill the pattern into a Learnings entry and
mark the friction-log entries **graduated**.

## Format

Each entry is a date-grouped bullet:

```markdown
## YYYY-MM-DD

- **<short tag>** — what happened. Why it was friction. Suggested fix or
  current status. **Status:** open | resolved (#PR) | graduated (LEARNINGS#section).
```

## 2026-05-03

- **badge-artifacts diff input mismatch** — issue #194 originally specced
  rendering CI badge artifacts against the sample fixture
  (`crates/ripr/examples/sample/example.diff` + `--root crates/ripr/examples/sample/src`),
  matching the `cargo xtask dogfood` pattern for determinism. While
  reading `crates/ripr/src/app.rs:201`, found that
  `ripr_plus_summary_from_disk` resolves
  `target/ripr/reports/test-efficiency.json` relative to `--root`.
  The sample fixture has no such report and its tests are different from
  the outer repo's; mixing them would have produced an incoherent badge
  (exposure side from one codebase, test-efficiency side from another).
  Corrected mid-flight to `--root .` + per-PR diff captured via
  `git diff origin/main...HEAD`. **Status:** resolved in #194 PR.
  **Possible follow-up:** badge-plus could grow an explicit
  `--test-efficiency-report <path>` flag so the auxiliary input is
  not implicit-by-root, removing the mismatch class entirely.
- **briefing off in-memory schema instead of reading source** — the
  haiku brief for `cargo xtask badge-artifacts` described the badge
  JSON shape from memory: `{"value": ..., "components": {...}}`. The
  actual schema in `crates/ripr/src/output/badge.rs` uses
  `"message"` (string) for the headline and `"counts"` + `"reason_counts"`
  (two separate objects) for the breakdown — there is no `"value"` and
  no `"components"`. Tests passed because the haiku built test fixtures
  matching the brief, not the real output. Caught only at the
  integration smoke (`cargo xtask badge-artifacts` actually run against
  the repo) — the resulting `ripr-badges.md` showed `value: 0` for the
  ripr+ badge that actually had `message: "11"`. **Status:** resolved
  in #194 PR. **Lesson:** when briefing a subagent on a schema, paste
  the live JSON output (or the source-of-truth code path) into the
  brief; do not paraphrase. Cost a full agent loop + re-implementation.
- **diff-scoped badge artifacts mistaken for repo-scoped baseline** —
  the dogfood preflight for `badge/publish-main-endpoint` ran
  `cargo xtask badge-artifacts` on freshly-pulled `main` and got
  `ripr 0 brightgreen`. I initially read that as "the repo is clean
  → safe to publish," but the task runs `git diff origin/main...HEAD`
  which is empty on `main` itself. The result is mechanically `0`
  exposure findings, not a meaningful repo baseline. Using that as a
  public README badge would publish `ripr 0 brightgreen` regardless of
  the repo's actual exposure profile — an empty signal dressed as a
  pass. Caught at the dogfood-classification step before any public
  badge URL was wired. **Status:** resolved across #198 (documents
  `scope: diff` vs `scope: repo` in `docs/BADGE_POLICY.md` and adds
  `badge/repo-scope-artifacts` as a separate work item blocking
  `badge/publish-main-endpoint`) and #204 (implements
  `cargo xtask repo-badge-artifacts`, the four `repo-badge-*` CLI
  formats, `analysis::run_repo_analysis`, schema 0.2 `scope` field;
  the bounded Voice A baseline). First honest repo-scoped headline:
  `ripr 317` against the current ripr workspace, vs the misleading
  `ripr 0` from the empty diff. **Lesson:** before publishing any
  `ripr` artifact as a public signal, run it on `main` itself and
  verify the number is *informative* — a mechanically-derivable
  constant (like a no-diff ripr count) is not.
- **two-voice operating brief** — the ChatGPT operating packet that
  framed Campaign 4A's repo-scope work contained two voices: Voice A
  (finish the bounded probe-shape baseline that was already in flight
  on `badge/repo-scope-artifacts`) and Voice B (reframe the entire
  product as a first-class seam-inventory + test-grip model with new
  `RepoSeam`/`SeamKind` types feeding LSP diagnostics and agent
  dispatch packets). Voice A was a single-PR finish; Voice B was a
  multi-campaign reshape. Step 0 caught the contradiction and surfaced
  it explicitly rather than picking silently. The user then chose
  Option C (bounded Voice A now via #204, seam inventory deferred to
  a future campaign and recorded in `docs/DEFERRED.md` as
  `deferred/seam-inventory-test-grip`). **Status:** open as a process
  observation. **Lesson:** when an operating brief contains
  contradictory directives, name the contradiction in the Step 0
  report and force the choice — silent picks lock the user into a
  path they didn't actually choose.
- **clippy gates can redden existing committed code** — `cargo xtask
  check-pr` on `badge/repo-scope-artifacts` failed `clippy::manual_strip`
  on an existing test in `crates/ripr/src/output/badge.rs:880-885`
  added by `6d845df` (the prior thread's checkpoint commit). The test
  was already on origin; the lint hadn't fired during the prior
  thread's dev loop because `-D warnings` only runs through the full
  xtask check. Fix in #204 (`ff42d71`): rewrote with `strip_prefix('"')?`
  rather than adding `#[allow(clippy::manual_strip)]`, since the
  idiomatic form is shorter and clearer anyway. **Status:** resolved
  (#204 `ff42d71`). **Lesson:** when a clippy gate fails on
  pre-existing code, fix the underlying expression — don't allow-attribute
  past it. The fact that prior CI accepted the code does not mean the
  code is correct; it means the prior CI did not run clippy with
  `-D warnings` in the path that landed on `main`.
- **identical mechanical fix replicated across simultaneous PRs** —
  `cargo xtask shape` re-sorted `policy/non_rust_allowlist.txt` (the
  `codecov.yml` row from #197 landed out of order) on both
  `badge/repo-scope-artifacts` (PR #204) and `docs/deferred-decisions`
  (PR #205) because both run `xtask shape` as part of the implementation
  packet, and both branch off main pre-#204. Whichever merges first
  cleans it up; the second PR sees the file already sorted and the
  diff falls out at merge time. Not actually a bug, but worth knowing
  when two scoped PRs run concurrently. **Status:** open as a process
  observation. **Lesson:** mechanical sort fixes from `xtask shape`
  duplicate cleanly across simultaneous PRs branched off the same
  base; trust the merge resolution rather than trying to coordinate
  which PR "owns" the sort.
- **`xtask` dep-free posture vs JSON parsing** — `badge-artifacts`
  needs to read the four badge JSONs to build the Markdown summary,
  but xtask has no `[dependencies]` block (deliberate). Implementation
  hand-rolled substring-based JSON extraction. Works, but is brittle:
  whitespace tolerance and array/object nesting are now duplicated in
  three places (`json_number_after`, `dogfood_class_counts`, the new
  `extract_json_*` helpers). **Status:** open. **Possible fix:** factor
  the substring-extraction helpers into one private module within
  xtask, OR introduce a tiny vendored serde-free reader (`mini_json`)
  if a fourth duplication appears.
- **codecov.yml informational field not in docs** — drafting the codecov
  config for PR1 (`coverage/codecov-config-v1`), the handoff packet
  included `informational: true` fields on coverage statuses. Web check
  against https://docs.codecov.com/docs/codecovyml-reference found
  `informational` is not a documented field; only `target`, `threshold`,
  `base`, `branches`, `if_ci_failed`, `only_pulls`, `flags`, and
  `paths` are mentioned. Simplified to the fallback safe config (no
  named path statuses, no undocumented fields). **Status:** resolved in
  PR1 by using documented fields only.
