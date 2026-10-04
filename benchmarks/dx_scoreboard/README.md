# Developer-experience scoreboards

`cargo xtask dx-scoreboard` measures whether ripr is something a developer
would happily reach for, using numbers a developer feels rather than internal
counts. Each metric belongs to one board, carries a target bar, and has a
regression margin. The report lands at
`target/ripr/reports/dx-scoreboard.{json,md}`.

The definitions live in [`scoreboards.toml`](scoreboards.toml). Changing a
target, margin, or corpus pin is a reviewed edit to that file.

## Boards

| Board | What a developer feels | Metrics |
|---|---|---|
| `speed` | How long until ripr says something useful, and whether the edit-check loop stays interactive | cold `ripr pilot` time and peak memory, warm `ripr check` time and peak memory, per corpus repository |
| `ci` | What it costs to adopt ripr in CI | lines in the workflow `ripr init --ci github` writes, whether compiling ripr is its only install route, install time (pending) |
| `trust` | Whether ripr is ever confidently wrong | commands that exit 0 on a missing repository, self-contradicting findings, false-verdict, false-actionable and abstention rates on the hand-checked verdict corpus (read from `fixtures/rust-verdict-corpus/expected/report.json`, which lands with #5318 together with its drift check; not measured until then), mutation spot-check agreement on discriminator and gap claims plus join coverage (ingested), judged-panel false actionable rate |
| `paste` | Whether a printed command works when pasted | printed commands that break or run injected code under a hostile repository path, printed commands that drop the repository root |
| `first_run` | The new-developer journey from install to first useful result | time to first useful result, walk seconds per crate, failed steps, steps over budget, friction events, `*_unknown` verdicts (all ingested) |
| `agent` | Whether an agent using only ripr's help closes a real test gap quickly and without being misled | fix success, mutants caught, ripr commands and tool steps to fix, false weak findings left after the fix, stale re-checks, white-box tests written only for ripr (all ingested from the agent-as-user harness) |

The rollup counts metrics that meet their target, fall below it, are not
measured, have a failed instrument, or regressed. A per-repository view groups
every per-repo sample, so a repository we intend to integrate with reads as
its own row set.

## Corpus

The corpus is a fixed list of real repositories pinned to exact commits. Each
pin is a commit whose own change touches production Rust, so the warm
`ripr check --base HEAD~1` measures a real small edit. Entries marked
`heavy = true` (ripr-swarm itself) run only with `--include-heavy`.
Checkouts live under `target/ripr/dx-scoreboard/corpus/`; cloning is opt-in:

```bash
cargo xtask dx-scoreboard --clone                  # all boards, light corpus
cargo xtask dx-scoreboard --clone --include-heavy  # what the nightly runs
cargo xtask dx-scoreboard --boards ci,paste        # seconds, no corpus
```

`--ripr-bin <path>` measures an existing binary; otherwise the command builds
release ripr first, because developers run release builds.

## How each instrument works

- **Cold pilot.** Fresh `RIPR_CACHE_DIR`, no `target/ripr` in the checkout,
  then the exact invocation the generated CI workflow runs:
  `ripr pilot --root <repo> --out <dir> --mode ready --max-seams 5`. The
  sample counts only when pilot exits 0 and `pilot-summary.json` says
  `complete`; otherwise it is `incomplete` and cannot meet its target.
- **Warm check.** `ripr check --root <repo> --base HEAD~1 --format json` twice
  on the cache pilot left behind; the second run is measured.
- **Peak memory.** `VmHWM` from `/proc/<pid>/status`, sampled in the same
  10 ms loop that enforces the deadline. Linux only; a lower bound.
- **Workflow size.** `ripr init --root <tiny crate> --ci github`, then the
  line count of `.github/workflows/ripr.yml`. The source-build flag is 1 only
  when `cargo install ripr` is the workflow's sole route; a fallback behind a
  prebuilt release download does not count.
- **False clean on bad input.** Twelve commands pointed at a repository that
  does not exist, run from an unrelated directory. Each should exit nonzero.
- **Self-contradictions.** Rules over the pilot `repo-exposure.json` and the
  warm check JSON:
  - `R1` a seam says reach is `no` while it lists related tests;
  - `R2` a finding is `no_static_path` while it lists related tests;
  - `R3` a finding's evidence says related tests were found while it lists
    none.
- **Paste safety.** A two-commit fixture crate at a path containing a space,
  `$(touch PWNED)`, an apostrophe and double quotes. `check`, `check
  --format human-full`, `pilot`, `first-pr` and `doctor` run against it from
  another directory. Every printed `ripr <command> … --option …` line (plain
  lines, backtick spans, bash fences; PowerShell forms skipped) is replayed
  under bash with `ripr` replaced by an argv recorder. A line is **unsafe** if
  bash rejects it, the canary file appears, or the root reaches ripr split or
  rewritten. It is **unbound** if it runs but neither passes the root nor
  changes into it.

## Ingesting other harnesses

The scripted first-run journey and the mutation spot-check measure their
metrics elsewhere and hand them over with `--ingest <file>`:

```json
{
  "schema_version": "ripr-dx-scoreboard-input-v1",
  "source": "first-run",
  "evidence": "link or path to the run",
  "metrics": [
    {"id": "first_run.friction_events", "value": 3},
    {"id": "first_run.time_to_first_useful_result_s", "value": 412, "repo": "serde", "completed": true}
  ]
}
```

`source` must match the metric's `ingest:<source>` in `scoreboards.toml`, so
an ingest file cannot overwrite a number this command measures. `completed:
false` records a journey that never reached a useful result.

Two native receipts are also accepted as-is:

- `first_run.v1` from the scripted first-run harness: time to first useful
  result (install step plus steps through the first successful `check`; not
  emitted when no install step was timed), friction events, and `*_unknown`
  verdicts.
- `first_run_row.v1` JSON Lines (one row per case, step and metric) from the
  first-run walk. Its gates map onto scoreboard metrics: nonzero `exit` rows
  are failed steps; `secs`, `stdout_lines` and `workflow_lines` rows over
  their own `budget` are over-budget steps; `friction_count` sums to friction
  events and any rise regresses; a missing `exit` row counts as a failed step;
  walk seconds per crate regress above 1.5x the same crate's baseline plus
  0.5 s (applied to each crate's total, not to every step). Malformed rows
  reject the file instead of counting as zero.
  `verdict` rows feed the `*_unknown` count, which is `review_on_change`: a
  changed verdict list is printed under "For review" and never fails the gate.
- `ripr-mutation-spot-check-v1` from the mutation spot-check: the agreement
  rate of the `claims_discriminator` and `claims_no_discriminator` families,
  and join coverage as `seam_precise` pairings over all mutants. These compare
  static claims with real mutation outcomes on a sample; they are evidence
  about ripr's calibration, not a mutation result for the corpus.

## Targets versus the regression gate

A **target** is the bar a developer would feel. Missing it is a gap to close
and never fails the command.

The **gate** (`--gate --baseline <earlier dx-scoreboard.json>`) fails when a
metric, or any repository's own sample of it, is worse than the baseline by
more than `max(regression_pct% of baseline, regression_floor)` (their sum for
`regression_additive` metrics), when a run stops completing, or when an
instrument breaks. Metrics the baseline measured that a run cannot compare,
such as ingested metrics without a receipt, are listed as not compared. Wall-time and memory metrics compare only against a baseline from the
same runner class (`local-linux-x86_64-4cpu`, `github-hosted-linux-x86_64-4cpu`,
or `RIPR_DX_RUNNER_CLASS`). Counts and line totals compare across runners.

`metrics/dx-scoreboard/baseline.json` is the committed baseline. To move it,
commit a newer report after reviewing why the numbers changed. A baseline
recorded on one runner class leaves speed metrics uncompared on another; the
report says so instead of passing them.

The nightly `.github/workflows/dx-scoreboard.yml` runs the full corpus with
the gate and uploads the report as an artifact.

## Claim boundary

Numbers hold for the recorded revision, binary, runner class, and pinned
corpus. Targets are proposed bars, not product guarantees. Verdict metrics are
static and do not claim runtime mutation outcomes.
