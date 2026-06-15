# Python Tier A Eval Sweep Fixtures

Contract: [RIPR-SPEC-0086](../../docs/specs/RIPR-SPEC-0086-python-eval-sweep.md).

These inputs drive `cargo xtask eval-sweep` — a report-only Tier A robustness
sweep that runs `ripr check` over real external Python repositories and records
only machine-checkable facts (crash rate, parse-failure rate, runtime, gap-ID
stability across a re-run). Tier A is a stability floor; it does not judge
actionability or usefulness.

## Files

- `manifest.json` — pinned external repos. Each entry: `id` (unique), `url`
  (https), `sha` (pinned commit), `license` (permissive: MIT / BSD / Apache-2.0),
  `shape` (one of `pytest_library`, `unittest_library`, `click_typer`,
  `fastapi_web`, `flask_web`), optional per-repo `synthetic_diff`, and a `why`
  note. A top-level `synthetic_diff` is the fallback when a repo omits its own.
- `synthetic-diff.diff` — the shared fallback synthetic diff (matches no real
  file; exercises the no-crash path for repos without a per-repo diff).
- `diffs/<id>.diff` — per-repo diffs that touch a **real** source file so the
  analyzer produces a real canonical gap (the meaningful gap-ID-stability signal).
  Each must match the repo at its pinned `sha`. The external repo working tree is
  never mutated; the diff is read from the file via `ripr check --diff`.

## Boundaries

- Cloning is opt-in via `cargo xtask eval-sweep --clone` and never runs on the
  default CI path. The default command runs against pre-placed checkouts and
  records a missing checkout as `skipped_missing_checkout` rather than failing.
- Repos here are analysis inputs only; their licenses are reviewed at pin time
  and are not governed by `ripr`'s own crate supply-chain gate.
- Commit SHAs are pinned; updating a SHA is a reviewed manifest change.
