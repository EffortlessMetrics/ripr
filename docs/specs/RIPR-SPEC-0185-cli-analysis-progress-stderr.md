# RIPR-SPEC-0185: CLI analysis progress on stderr

Status: proposed

Owner: product / cli

Created: 2026-09-29

Linked proposal:

Linked ADRs:

Linked plan:

Linked issues:

- #4810 — CLI stderr stages and bounded heartbeats (this slice)
- #4957 — time-based heartbeat cadence so long walks never go silent
- #2608 — parent shared progress contract (not closed here)
- #4829 — live producer-owned progress events (PR A; this slice consumes that
  contract and does not replace it)
- #4193 — stale draft producer; superseded as the live PR A writer by #4829
- #4811 — LSP work-done mapping (forbidden here)

Linked PRs:

Support-tier impact:

- None. Stderr progress is advisory visibility. It does not change findings,
  gates, support-tier labels, required CI, or release scope.
  [docs/status/SUPPORT_TIERS.md](../status/SUPPORT_TIERS.md)

Policy impact:

- None. No new env var, config key, workflow, or exception ledger.

## Problem

`ripr check` can stay silent on stderr for a long analysis while stdout is
reserved for findings. Users and CI cannot tell active work from a hang
without scraping logs. Copying LSP progress into the CLI would invent a
second stage vocabulary.

## Behavior

1. The check application pipeline emits optional producer-owned
   `AnalysisProgressEvent` records at actual boundaries: `loading_input`,
   `analyzing`, `building_output`, and one terminal `completed`,
   `cancelled`, or `failed`. Counters stay `None` until the producer has an
   honest denominator. Elapsed time is observational and is not analysis
   identity.
2. The CLI projects those events onto stderr only:
   `ripr progress: <stage> [<scope>]` and, while a stage stays active,
   `ripr progress: <stage> still active after <elapsed class>`.
3. Non-TTY / CI lines are newline-delimited and contain no ANSI or carriage
   returns. A TTY may reuse one line, pads overwrites to the longest prior
   progress line, and stays silent below a 250ms minimum-duration threshold.
   A TTY stage that remains active past that threshold becomes visible even
   if the producer event arrived during the silence window.
4. Heartbeats repeat only the current producer stage, start after 2s, and are
   time-throttled to at most one line every 8 seconds of stage activity for as
   long as the stage stays active (#4957). There is no per-run count ceiling on
   the STANDARD policy, so a minutes-long repo walk never goes silent; line
   growth stays one line per `heartbeat_every` of stage time, and policies
   that need a hard count stop can still set one.
5. JSON, SARIF, GitHub annotations, and other machine stdout formats remain
   byte-identical whether progress is emitted or `--quiet` is set. Machine
   formats do not implicitly disable stderr progress.
6. `--quiet` suppresses every non-error progress record and heartbeat.
   Command errors still go to stderr.
7. Unknown totals never render as a percentage or ETA. Absolute paths,
   source text, and environment values never enter progress lines.
8. A sink or rendering failure cannot turn successful analysis into failure.
   The CLI holds producer `completed` until artifact writing and stdout
   succeed; a later command failure projects `failed` and never `completed`.
9. Removing the producer sink makes the CLI discriminator lose its stage
   evidence. This slice does not add analyzer stages, LSP mapping, or a
   speed claim.

## Required Evidence

- A sample `ripr check --format json` parses stdout while stderr carries
  producer stage tokens and no control sequences.
- The same JSON or SARIF stdout bytes are emitted with `--quiet`; quiet
  stderr has no `ripr progress:` lines.
- A missing diff projects `failed` and never `completed`.
- `ripr check --help` documents stderr, `--quiet`, unknown totals, and the
  no-speed-claim boundary, and does not emit progress.
- Unit tests reject path-like, percent, and ETA progress lines; throttle
  heartbeats and show no 10s window of a minutes-long stage stays silent;
  isolate a broken writer; and show that omitting the producer
  sink leaves the observer empty.

## Non-Goals

- LSP `workDoneProgress` mapping (#4811)
- Analyzer optimization or latency envelopes (#1578 / #1604)
- Closing parent #2608
- A new UI crate, env var, or config-file progress authority
- Per-file or per-probe event flood
- Invented percentages, ETAs, or a second stage vocabulary

## Acceptance Examples

1. `ripr check --root <sample> --diff <sample.diff> --format json` writes
   findings JSON on stdout and `ripr progress: loading_input [diff]` then
   `analyzing` then `completed` on stderr.
2. Adding `--quiet` keeps stdout bytes identical and drops progress from
   stderr.
3. `--format sarif` and `--format github` with and without `--quiet` produce
   identical stdout.
4. `--diff` pointing at a missing file exits non-zero, projects
   `ripr progress: failed [diff]`, and never `completed`.
5. A TTY run that finishes under 250ms emits no stage spray.
6. A blocked analyzing stage keeps emitting heartbeats at most one per 8
   seconds of stage time; no 10s window of a minutes-long stage is silent.
   The STANDARD policy places no per-run count ceiling, so the former
   16-line cap is gone (#4957).

## Test Mapping

- `crates/ripr/src/app/progress.rs::tests::check_progress_reports_real_boundaries_without_invented_totals`
- `crates/ripr/src/app/progress.rs::tests::missing_diff_fails_after_loading_input_without_false_completion`
- `crates/ripr/src/app/progress.rs::tests::progress_sink_panic_cannot_change_analysis_result`
- `crates/ripr/src/app/progress.rs::tests::no_sink_path_stays_available`
- `crates/ripr/src/app/progress.rs::tests::cancelled_token_through_check_emits_cancelled_not_completed`
- `crates/ripr/src/cli/progress.rs::tests::non_tty_stage_lines_are_newline_delimited_and_path_free`
- `crates/ripr/src/cli/progress.rs::tests::unknown_totals_never_render_as_percentage_or_eta`
- `crates/ripr/src/cli/progress.rs::tests::tty_short_run_does_not_flash_completed`
- `crates/ripr/src/cli/progress.rs::tests::tty_short_failure_still_projects_failed`
- `crates/ripr/src/cli/progress.rs::tests::tty_and_non_tty_share_stage_tokens`
- `crates/ripr/src/cli/progress.rs::tests::failure_terminal_never_emits_completed`
- `crates/ripr/src/cli/progress.rs::tests::cancelled_terminal_never_emits_completed`
- `crates/ripr/src/cli/progress.rs::tests::projection_tokens_cover_the_closed_producer_vocabulary`
- `crates/ripr/src/cli/progress.rs::tests::rendering_failure_is_isolated`
- `crates/ripr/src/cli/progress.rs::tests::heartbeat_is_throttled_and_bounded`
- `crates/ripr/src/cli/progress.rs::tests::standard_heartbeat_spans_long_repo_walks_without_a_ten_second_gap`
- `crates/ripr/src/cli/progress.rs::tests::tty_suppressed_stage_becomes_visible_once_min_visible_elapses`
- `crates/ripr/src/cli/progress.rs::tests::tty_overwrite_clears_a_longer_previous_line`
- `crates/ripr/src/cli/progress.rs::tests::tty_overwrite_pads_to_the_longest_prior_line`
- `crates/ripr/src/cli/progress.rs::tests::held_completed_waits_for_command_commit`
- `crates/ripr/src/cli/progress.rs::tests::drop_without_commit_converts_held_completed_to_failed`
- `crates/ripr/src/cli/progress.rs::tests::unsafe_constructed_lines_are_rejected`
- `crates/ripr/src/cli/help.rs::tests::check_help_mentions_repo_badge_formats_and_examples`
- `crates/ripr/tests/cli_progress.rs::check_json_stdout_parses_while_progress_stays_on_stderr`
- `crates/ripr/tests/cli_progress.rs::check_quiet_keeps_json_stdout_byte_identical_and_drops_progress`
- `crates/ripr/tests/cli_progress.rs::check_sarif_stdout_is_unchanged_by_progress`
- `crates/ripr/tests/cli_progress.rs::check_github_stdout_is_unchanged_by_progress`
- `crates/ripr/tests/cli_progress.rs::check_markdown_stdout_is_unchanged_by_progress`
- `crates/ripr/tests/cli_progress.rs::check_worktree_projects_worktree_scope_on_stderr`
- `crates/ripr/tests/cli_progress.rs::check_progress_failure_emits_failed_not_completed`
- `crates/ripr/tests/cli_progress.rs::check_quiet_failure_keeps_errors_and_drops_progress`
- `crates/ripr/tests/cli_progress.rs::check_unwritable_artifact_projects_failed_not_completed`
- `crates/ripr/tests/cli_progress.rs::check_help_does_not_spray_progress`

## Implementation Mapping

- `crates/ripr/src/app/progress.rs` — closed event/sink contract
- `crates/ripr/src/app/check.rs` — producer boundaries on the check path
- `crates/ripr/src/cli/progress.rs` — stderr projection, heartbeat, TTY policy
- `crates/ripr/src/cli/commands/check.rs` — `--quiet` and sink wiring
- `crates/ripr/src/cli/help/core.rs` — user-facing contract

## Metrics

- `cli_progress_stage_events` — producer stages observed by the CLI sink
- `cli_progress_heartbeats` — throttled heartbeat lines emitted
