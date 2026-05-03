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
