# RIPR-SPEC-0086: Python Tier A External-Repo Eval Sweep

Status: proposed

Owner: language-adapter / swarm

Linked proposal:

- None. This is a standalone evidence-tooling contract; it adds no product
  library behavior and no public API. It anchors the eval-sweep-driven Python
  reliability campaign tracked in `.ripr/goals/python-repair-routing.toml`.

Linked ADRs:

- [ADR 0009](../adr/0009-python-parser-substrate.md) (Python parser substrate;
  the sweep measures the current `rustpython-parser`-backed lane).

Linked plan:

- None.

Linked issues:

- [release(py): Python usable-tier readiness checklist](https://github.com/EffortlessMetrics/ripr-swarm/issues/1160)

Linked PRs:

- (this PR)

## Problem

The in-repo Python dogfood corpus is saturated: every metric passes on a curated
set authored by the same people who wrote the analyzer. That confirms the repair
loop is internally consistent, but it says nothing about how `ripr check` behaves
on Python repositories we did not write. The release-readiness question —
**does the analyzer stay crash-free, parse-robust, and gap-ID-stable on external
code?** — has no measured answer.

This spec defines a **Tier A external-repo eval sweep**: a report-only `xtask`
command that runs `ripr check` over a pinned manifest of real external Python
repositories and records only machine-checkable robustness facts. Tier A is a
stability floor; it deliberately does **not** judge actionability or usefulness
(that is Tier B, a later spec).

## Behavior

### One production delta

Add `cargo xtask eval-sweep`: a report-only command. It introduces no change to
`crates/ripr` (the analyzer library) — it is automation that exercises the
existing `ripr check` surface and aggregates results.

### Inputs

- A pinned manifest (`fixtures/python-eval-sweep/manifest.json`): a versioned
  envelope listing external repos as `{ id, url (https), sha, license, shape,
  synthetic_diff?, why }`.
- A synthetic Python diff per repo (or a shared fallback diff). The diff is read
  from a file; the external repo working tree is never mutated.

### Run algorithm (per manifest entry)

1. Resolve the repo checkout. When `--clone` is passed, clone at the pinned
   `sha` into a `target/`-local checkout dir via the existing
   `run::run_with_envs` helper; otherwise expect a pre-placed checkout and record
   `skipped_missing_checkout` if absent (never fails on absence).
2. Run `ripr check --root <checkout> --diff <synthetic-diff> --format json`
   under a wall-clock timeout via `run::capture_output_with_timeout`.
3. Classify the outcome from exit code and JSON:
   - `crash` — process abort or `ripr: <err>` failure exit;
   - `timed_out` — exceeded the timeout;
   - `parse_failure` — JSON parsed but the file degraded to a named static-unknown
     limitation (graceful, **not** a crash);
   - `ok` — exit 0 with well-formed JSON.
4. Collect the set of `canonical_gap_id` values and the run-1 runtime.
5. Re-run steps 2–4 once; gap-ID stability is `set(run1) == set(run2)`.

### Metrics

Across non-skipped entries: `crash_rate`, `parse_failure_rate`, `timed_out_count`,
runtime min/median/max/total, and `gap_id_stability_rate`. A `gate_status` of
`pass` requires `crash_rate == 0` and `gap_id_stability_rate == 1.0`; otherwise
`review`, with a recorded reason. Empty `repos_run` guards division (rates default
to `0.0` crash / `1.0` stability).

### Policy boundary (load-bearing)

- `--clone` is **opt-in and off the default CI path.** No `.github/workflows`
  step clones or fetches. The default command runs against pre-placed checkouts
  only, so the gated `check-pr` path never touches the network.
- All subprocess work routes through the already-allowlisted `xtask/src/run.rs`
  helpers, so no `process_allowlist.txt` or `network_allowlist.txt` change is
  required.

## Required Evidence

- This spec, registered in `policy/doc-artifacts.toml` and `docs/specs/README.md`.
- A `[[behavior]]` entry in `.ripr/traceability.toml` mapping this spec to the
  unit tests and the manifest fixture.
- `fixtures/python-eval-sweep/{SPEC.md, manifest.json, synthetic-diff.diff}`.
- Unit tests in `eval_sweep.rs`: manifest load/validate (rejects duplicate ids,
  non-https url, empty repos); outcome classifier over sample JSON; gap-ID set
  comparison flags an injected instability; metrics arithmetic with empty-set
  guards; deterministic JSON/markdown report rendering.
- A golden of the rendered report from a fixed in-memory run vector.

## Non-Goals

- No actionability, usefulness, or false-actionable judgement (that is Tier B).
- No mutation execution, provider calls, generated tests, or production-code edits.
- No network access on the default CI path; external clone is opt-in only.
- No change to `crates/ripr` analyzer behavior or public API.
- No support-tier claim; this produces evidence, not a promotion.

## Acceptance Examples

### A passing sweep

```text
repos_run = 12, crash_rate = 0.0, parse_failure_rate = 0.08,
gap_id_stability_rate = 1.0  ->  gate_status = "pass"
```

### A sweep that fails closed to review

```text
repos_run = 12, crash_rate = 0.08 (1 repo aborted)  ->  gate_status = "review",
reason = "1/12 repos crashed; investigate before promotion"
```

### Missing checkout without --clone

```text
repo skipped, outcome = "skipped_missing_checkout"  ->  excluded from rates,
never fails the command
```

## Test Mapping

- `eval_sweep::manifest_load_rejects_invalid` -> manifest validation contract.
- `eval_sweep::classifier_maps_outcomes` -> outcome classification contract.
- `eval_sweep::gap_id_instability_detected` -> gap-ID stability contract.
- `eval_sweep::metrics_guard_empty_run_set` -> metrics arithmetic contract.
- `eval_sweep::report_render_golden` -> deterministic report rendering.

## Implementation Mapping

| Concern | Code |
| --- | --- |
| Command logic (arg parse, manifest load, run orchestration, classify, metrics, render) | `xtask/src/reports/eval_sweep.rs` |
| Subcommand registration | `xtask/src/command.rs`, `xtask/src/dispatch.rs`, `xtask/src/reports/mod.rs` |
| Subprocess helpers (clone, `ripr check`) | `xtask/src/run.rs` (`run_with_envs`, `capture_output_with_timeout`) |
| Pinned manifest + synthetic diff | `fixtures/python-eval-sweep/manifest.json`, `fixtures/python-eval-sweep/synthetic-diff.diff` |
| Rendered report | `target/ripr/reports/eval-sweep.{json,md}` |

## Metrics

| Metric | Meaning |
| --- | --- |
| `repos_run` | external repos analyzed (excludes skipped/clone-failed) |
| `crash_rate` | fraction of `repos_run` that aborted or failed-exit |
| `parse_failure_rate` | fraction degrading to a named static-unknown limitation |
| `timed_out_count` | repos exceeding the wall-clock timeout |
| `runtime_ms_median` | median run-1 `ripr check` wall-clock per repo |
| `gap_id_stability_rate` | fraction with identical canonical gap-ID sets across a re-run |
| `gate_status` | `pass` iff `crash_rate == 0` and `gap_id_stability_rate == 1.0`, else `review` |

