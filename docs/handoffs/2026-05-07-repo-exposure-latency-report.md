# Handoff: Repo Exposure Latency Report

Date: 2026-05-07
Branch / PR: `repo-exposure-latency-report`

## Current Work Item

`cache/repo-exposure-latency-report`

This work item adds bounded latency visibility for the repo-exposure command
path. It does not change analyzer classifications, repo-exposure JSON/Markdown,
LSP behavior, SARIF, badges, public API, VS Code behavior, or runtime
calibration behavior.

## What Changed

| Surface | Change |
| --- | --- |
| Analyzer trace | `inventory_classified_seams_at_with_config` emits opt-in stderr trace lines when `RIPR_REPO_EXPOSURE_LATENCY_TRACE=1` is set. |
| Xtask command | `cargo xtask repo-exposure-latency-report` builds the debug `ripr` binary, runs repo-exposure formats with a timeout, and writes `target/ripr/reports/repo-exposure-latency.{json,md}`. |
| Timeout behavior | The report records `timeout` and skips Markdown if JSON times out, so the diagnostic command returns a bounded report instead of hanging indefinitely. |
| Docs and manifests | Campaign 9 now points next at `cache/repo-exposure-warm-path-reuse`. |

## Local Evidence

A local smoke run with `RIPR_REPO_EXPOSURE_LATENCY_TIMEOUT_MS=2000` and a
default-timeout run both produced a `warn` report:

- `repo-exposure-json` timed out after about 2 seconds.
- The default 30-second run also timed out in `repo-exposure-json`.
- `collect_workspace_state` completed quickly.
- `cache_load` reported `miss`.
- `repo-exposure-md` was skipped after the JSON timeout.

That evidence narrows the next investigation to cold compute and warm-path
reuse, not workspace-state collection.

## Next Work Item

`cache/repo-exposure-warm-path-reuse`

Use the latency report to reduce warm-path recomputation. Keep rendered outputs
uncached and preserve analyzer outputs, output schemas, LSP, SARIF, badges,
agent packets, and public API behavior.

## What Not To Do

- Do not cache rendered JSON, Markdown, diagnostics, hover text, SARIF, badges,
  or agent packets.
- Do not add new LSP product features during cache work.
- Do not change static classifications to make latency reports look better.
- Do not hide timeouts by treating missing measurements as zero.
