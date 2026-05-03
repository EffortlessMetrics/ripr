# Deferred Decisions

This file is the canonical home for "v1 simple, revisit later" decisions
in the `ripr` codebase. Each entry records what `ripr` does today, why
the v1 path was chosen, what risk that creates, what would trigger a
revisit, and the likely v2 direction. Entries graduate to closed status
when the v2 work lands or when the underlying motivation is no longer
load-bearing.

This is not a feature backlog — that lives in `docs/IMPLEMENTATION_CAMPAIGNS.md`
and `.ripr/goals/active.toml`. This is the place where the v1 simplifications
that paid for themselves are written down so a future session does not
mistake them for permanent design choices.

## How to read an entry

```text
## deferred/<short-id>

Status:           open | scoping | landing-soon | closed (#PR)
Surface:          analyzer | output | cli | lsp | xtask | docs | policy
Current v1 behavior:
                  one paragraph on what ripr does today.
Why v1 kept it simple:
                  the trade-off that earned its keep at v1 time.
Risk:             what the v1 simplification can let slip through.
Revisit trigger:  the observable signal that says "now is the time."
Likely v2 direction:
                  one paragraph on the expected shape of the v2 fix.
Related PRs / friction:
                  links and #issues.
```

---

## deferred/seam-inventory-test-grip

Status: open
Surface: analyzer / output / lsp

Current v1 behavior:
  Repo-scoped `ripr` (`badge/repo-scope-artifacts`, `analysis::run_repo_analysis`)
  produces findings by seeding probes from every currently-probeable production
  syntax shape (`ProbeShapeFact`) and running them through the existing
  classifier. This is the bounded **Voice A** baseline: an honest count of
  unresolved actionable findings *under what the analyzer can probe today*,
  labeled `scope: "repo"` and disclaimed in `docs/BADGE_POLICY.md` as "not full
  seam inventory and not proof of mutation adequacy."

Why v1 kept it simple:
  Reusing `ProbeShapeFact` + the existing classifier let the badge artifact
  surface exist without inventing a new analyzer model. It unblocked
  `badge/publish-main-endpoint` (the empty-diff-on-main bug) without expanding
  Campaign 4A into a multi-PR analyzer rewrite.

Risk:
  The repo headline (e.g. `ripr 317`) can be read as "this many missing tests"
  or "this many seams need oracles." It is neither. A reader who skips the
  disclaimer might over- or under-trust the number.

Revisit trigger:
  The first concrete user request along the lines of "I want a repo-wide
  burndown of behavior seams I should write tests for," or the first time an
  agent dispatch loop wants to close one finding per PR. Either signal means
  the bounded count is no longer a sufficient interface.

Likely v2 direction:
  First-class **Voice B** model: introduce `RepoSeam` / `SeamKind`
  (predicate boundary, error variant, return value, struct field, side effect,
  match arm, validation, call presence) and classify each seam through
  reach / activate / propagate / observe / discriminate. Feed LSP diagnostics,
  hover evidence, and agent dispatch packets from the same inventory.
  Keep `ProbeShapeFact` as the underlying syntax surface; `RepoSeam` is the
  product-level abstraction over it.

Related PRs / friction:
  - #198 (scope distinction)
  - #202 (issue defining repo-scope-artifacts)
  - #204 (this PR — Voice A baseline)
  - `docs/FRICTION_LOG.md` 2026-05-03: "diff-scoped badge artifacts mistaken
    for repo-scoped baseline"

---

## deferred/repo-badge-cache

Status: open
Surface: xtask / analyzer

Current v1 behavior:
  `cargo xtask repo-badge-artifacts` recomputes everything from scratch each
  run: workspace discovery, file index build, probe generation across all
  production files, classification, badge rendering. On the current ripr
  workspace this is acceptable (single-digit seconds). For larger repos or
  hot LSP loops it would not be.

Why v1 kept it simple:
  Caching adds correctness risk (stale cached facts shipping wrong findings),
  cache-key complexity (workspace + analyzer version + config + intent +
  suppressions hashes), and storage policy. Without a real performance signal
  the v1 path stays end-to-end recomputed.

Risk:
  Editor-mode latency or large-repo dogfood may make the un-cached path
  unworkable, and a hasty cache could ship stale signal under a green badge.

Revisit trigger:
  - LSP diagnostics request takes >100ms p50 on a real workspace, or
  - `cargo xtask repo-badge-artifacts` takes >60s on a contributor's machine,
  - or any consumer asks for "warm" runs.

Likely v2 direction:
  Cache **facts**, not final outputs. Layers (worth caching individually):
  ```
  target/ripr/cache/file-facts/
  target/ripr/cache/owner-index/
  target/ripr/cache/test-facts/
  target/ripr/cache/oracle-facts/
  target/ripr/cache/probe-summaries/
  ```
  Key by: schema version, ripr analyzer version, file path + content hash,
  cfg/features hash, config hash, intent hash, suppressions hash. Never cache
  badge JSON, Shields JSON, final counts, or warnings — those always render
  from current facts and current policy.

Related PRs / friction:
  - `cache/persistent-cache-v1` work item in Campaign 5.

---

## deferred/timezone-aware-suppressions

Status: open
Surface: policy / output

Current v1 behavior:
  `.ripr/suppressions.toml` `expires` field is `YYYY-MM-DD`. Expiry comparison
  uses `today` from `output::suppressions::current_iso_date()` against the
  string `expires` value, both interpreted in whatever local timezone the
  process happens to run in.

Why v1 kept it simple:
  A date-only contract is what reviewers actually write in TOML. Time-of-day
  precision wasn't asked for, and it would force everyone to think about
  timezones.

Risk:
  An expiry of `2026-09-01` could behave differently on a CI machine in UTC
  versus a contributor's machine in PT — possibly off by a day at the boundary.
  Today this is silent; expired entries surface as warnings, but the boundary
  is fuzzy.

Revisit trigger:
  - First reported case where a suppression "expired early" or "didn't expire
    on time" because of timezone drift,
  - or a regulated context that requires UTC-anchored expiry.

Likely v2 direction:
  Pin expiry comparison to UTC explicitly in `current_iso_date`, and document
  it in `docs/BADGE_POLICY.md`. Optionally allow `expires = "2026-09-01T00:00:00Z"`
  as an extended form when needed.

---

## deferred/suppression-intent-precedence

Status: open
Surface: output / policy

Current v1 behavior:
  Test intent and suppressions operate on different selectors: intent matches
  by `(test name, owner, intent)` against the test-efficiency ledger;
  suppressions match by `kind = "test_efficiency"` + `(test, path)`. They are
  composed additively — a test can be both declared-intent (excluded from
  `unsuppressed_test_efficiency_findings`) and suppression-matched
  (`suppressed_test_efficiency_findings`). The current arithmetic treats
  intent and suppression as disjoint.

Why v1 kept it simple:
  Intent and suppression are conceptually distinct (intent = "this is by
  design", suppression = "we know this gap exists, we accept the risk"), so
  v1 keeps them in separate counters. Reviewers can see both.

Risk:
  When a test legitimately matches both an intent declaration and a
  suppression, neither bucket says "the other one also applies." A reviewer
  reading just the suppression entry might think the test still has the
  test-efficiency signal without realizing intent already excluded it.

Revisit trigger:
  First reported confusion in PR review or first time a contributor adds a
  redundant suppression for an already-intent-declared test.

Likely v2 direction:
  Define a precedence rule explicitly in `docs/BADGE_POLICY.md`: intent
  excludes from headline first; suppression applies only to the residual.
  Surface in the JSON as `precedence: "intent"` vs `precedence: "suppression"`
  on a per-test basis.

---

## deferred/diff-relative-badge-thresholds

Status: open
Surface: output

Current v1 behavior:
  Badge color thresholds are absolute: `0` = brightgreen, `1-3` = yellow,
  `4+` = orange. Same thresholds apply to diff and repo scope.

Why v1 kept it simple:
  Absolute thresholds are easy to reason about and match the inbox-zero
  framing. They produce useful signal without policy debate.

Risk:
  Diff-scope badges on large refactor PRs trip orange noisily even when the
  signal-to-finding ratio is normal. Repo-scope `ripr 317` is orange even
  though that's a fresh-baseline number, not a regression.

Revisit trigger:
  Real-world numbers from CI artifacts and public repo-scope endpoints. After
  ~20 PR samples and one published main run, decide whether a diff-relative
  threshold is worth the extra config.

Likely v2 direction:
  Make thresholds tunable via `BadgePolicy { color_thresholds: ... }`, defaulting
  to absolute for backwards compat. Optionally introduce a ratio-based
  threshold: `unresolved / analyzed_findings` exceeds X.

---

## deferred/analyzed-tests-semantics

Status: open
Surface: output

Current v1 behavior:
  `counts.analyzed_tests` is the count of unique `(file, test name, line)`
  tuples observed across the findings' `related_tests` lists in diff scope,
  and the count from the test-efficiency report's `metrics.tests_scanned`
  in `ripr+` paths. The two paths disagree on what "analyzed" means.

Why v1 kept it simple:
  Each path has a sensible local definition. Reconciling required a wider
  schema change than `ripr` v1 wanted.

Risk:
  An LLM consumer reading the JSON sees the same field name with different
  semantics depending on `kind`. The diff-scope `analyzed_tests` is also
  zero-or-tiny when no related tests are detected, even if hundreds of tests
  exist in the repo.

Revisit trigger:
  First downstream tool that tries to compare `analyzed_tests` across `ripr`
  and `ripr+` and gets a confused result.

Likely v2 direction:
  Either rename per kind (`analyzed_related_tests` for diff `ripr`,
  `analyzed_repo_tests` for `ripr+`) or normalize both to "unique tests in
  the workspace" with the per-finding count moved to a separate field.

---

## deferred/render-check-result-api-break

Status: open
Surface: app / public-api

Current v1 behavior:
  `ripr::app::render_check(output: &CheckOutput, format: &OutputFormat) -> Result<String, String>`.
  `OutputFormat` is a public enum; new variants are appended over time
  (`BadgeJson`, `BadgePlusJson`, repo-scope variants in #204). Library
  consumers that exhaustively `match` on `OutputFormat` get a compile error
  on every new variant.

Why v1 kept it simple:
  Adding variants additively was the right move while the API was settling.
  Marking the enum `#[non_exhaustive]` would have signaled instability and
  required `_` arms in tests immediately.

Risk:
  Each new format variant is a SemVer-significant change for downstream Rust
  consumers. There are currently no published downstream consumers, so this
  is theoretical risk.

Revisit trigger:
  - First downstream Rust crate consuming `ripr::app::render_check` that we
    do not control, OR
  - intent to publish 1.0.

Likely v2 direction:
  Mark `OutputFormat` `#[non_exhaustive]` at the next minor bump and document
  in CHANGELOG. Consider exposing a higher-level helper (e.g. `Renderer::for_format`)
  that returns errors for unrecognized formats rather than requiring exhaustive
  matches.

---

## deferred/badge-plus-report-path

Status: open
Surface: cli / output

Current v1 behavior:
  `ripr check --format badge-plus-*` reads `target/ripr/reports/test-efficiency.json`
  resolved relative to `--root`. There is no flag to override the path.

Why v1 kept it simple:
  Convention-over-configuration; `cargo xtask test-efficiency-report` writes
  to that path; the badge-plus path reads from the same path. One implicit
  contract.

Risk:
  CI pipelines that want to materialize the report somewhere else (e.g. a
  cached upload) cannot — they have to copy the file into the conventional
  location. Mentioned in the friction log entry "badge-artifacts diff input
  mismatch" as a possible follow-up.

Revisit trigger:
  First CI pipeline that wants to source the report from a non-default path,
  or first user request for `--test-efficiency-report <PATH>`.

Likely v2 direction:
  Add `--test-efficiency-report PATH` to the CLI that overrides the default;
  the convention path stays as the fallback when the flag is omitted.

Related PRs / friction:
  - `docs/FRICTION_LOG.md` 2026-05-03: "badge-artifacts diff input mismatch"

---

## deferred/xtask-mini-json

Status: open
Surface: xtask

Current v1 behavior:
  `xtask` has no `[dependencies]` block (deliberate dep-free posture). Where
  `xtask` needs to read JSON (badge artifact summaries, dogfood class counts,
  test-oracle reports, receipt status), each call site hand-rolls
  substring-based extraction (`json_number_after`,
  `extract_json_object_usize_map`, `extract_json_string`,
  `extract_json_warnings`). The duplication has reached three or four call
  sites.

Why v1 kept it simple:
  Adding `serde_json` to `xtask` would change the policy posture (no third-party
  deps in repo automation) and complicate `cargo deny` allowlists.

Risk:
  Brittle whitespace and nesting handling. Each new helper adds another place
  that can silently extract the wrong substring on a JSON shape change.

Revisit trigger:
  - Fourth or fifth duplication of substring-extraction logic, OR
  - One real bug caused by the substring helpers (e.g. a nested object
    confused for a top-level key).

Likely v2 direction:
  Either: factor the extraction helpers into one private `xtask::mini_json`
  module with a tested grammar (still no third-party dep), OR — only if a
  bug forces it — introduce `serde_json` to `xtask` with explicit policy
  exception in `policy/dependency_allowlist.txt`.

Related PRs / friction:
  - `docs/FRICTION_LOG.md` 2026-05-03: "xtask dep-free posture vs JSON parsing"

---

## deferred/repo-exposure-baseline

Status: superseded
Surface: analyzer

Status note: This entry was the umbrella concept that became
`deferred/seam-inventory-test-grip` once the bounded Voice A v1 landed in #204.
Kept as a redirect.

Current v1 behavior:
  See `deferred/seam-inventory-test-grip`.

---

## Cross-references

- `docs/BADGE_POLICY.md` — locked vocabulary and what each badge does and does not prove.
- `docs/IMPLEMENTATION_CAMPAIGNS.md` — active campaign work items.
- `.ripr/goals/active.toml` — machine-readable manifest.
- `docs/FRICTION_LOG.md` — raw same-day observations; entries graduate either to a fix, to this register, or to LEARNINGS.
- `docs/LEARNINGS.md` — settled principles.
