# Handoff: Hot Sidecar Latency Audit

Date: 2026-05-07
Branch / PR: `hot-sidecar-latency-audit`
Latest merged PR: #421 `campaign: close runtime calibration fixtures`

## Current Work Item

`cache/current-latency-audit`

The audit measured current cache and editor proof surfaces before changing any
cache semantics. No analyzer, output schema, LSP, SARIF, badge, calibration,
VS Code, or public API behavior changes are part of this work item.

## Observations

| Surface | Observation |
| --- | --- |
| Seam cache unit tests | `cargo test -p ripr analysis::seam_cache --lib` passed and exercised miss, warm hit, invalidation, corrupt-entry, and store-failure behavior. |
| Seam inventory integration tests | `cargo test -p ripr analysis::seam_inventory --lib` passed and exercised cache-backed inventory/count behavior. |
| LSP tests | `cargo test -p ripr lsp` and `cargo test -p ripr lsp::tests` passed. |
| LSP cockpit | `cargo xtask lsp-cockpit-report` generated `target/ripr/reports/lsp-cockpit.{json,md}` with status `pass`, one boundary-gap seam diagnostic, all seam actions present, and no uncovered contributed VS Code commands. |
| Operator cockpit | `cargo xtask operator-cockpit` generated `target/ripr/reports/operator-cockpit.{json,md}` quickly and correctly reported missing required report inputs when only LSP cockpit and optional calibration reports were present. |
| Repo exposure command | `cargo xtask repo-exposure-report` did not finish within a 20-minute local timeout on this workspace. The spawned `ripr.exe` process was stopped after the timeout. |

The local sub-second timings for the test and cockpit commands were warm-build
smoke measurements, not benchmarks. They are useful only as evidence that the
existing proof commands are cheap enough to keep in the Campaign 9 gate.

## Finding

The current repo seam cache has good unit and integration coverage, and the LSP
cockpit proof remains cheap and green. The unresolved product risk is the
full-repo exposure command: a direct local run exceeded a 20-minute timeout, so
the next PR should add bounded latency visibility before attempting cache
optimization.

## Next Work Item

`cache/repo-exposure-latency-report`

Add bounded repo-exposure latency instrumentation or reporting so cache
hit/miss state and major phase timing can be observed. The output should be a
diagnostic/reporting surface for maintainers, not a change to analyzer results.

## What Not To Do

- Do not rewrite the cache before measuring phase timing and cache hit/miss
  state.
- Do not cache rendered JSON, Markdown, diagnostics, hover text, or agent
  packets.
- Do not add new LSP product features while doing latency work.
- Do not change static classifications, SARIF, badges, output schemas, or public
  API as part of latency instrumentation.
