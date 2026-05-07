# Handoff: Repo Exposure Warm-Path Reuse

Date: 2026-05-07
Branch / PR: `repo-exposure-warm-path-reuse`

## Current Work Item

`cache/repo-exposure-warm-path-reuse`

This work item adds fact-layer reuse below the classified-seam cache. It does
not cache rendered JSON, Markdown, diagnostics, hover text, SARIF, badges, or
agent packets, and it does not change analyzer classifications or public output
schemas.

## What Changed

| Surface | Change |
| --- | --- |
| File facts | `FileFacts` and related fact DTOs are serializable so parser/file facts can be cached as internal fact layers. |
| Cache | Added `target/ripr/cache/repo-file-facts/0.1`, keyed by analyzer version, file path, and file content hash. |
| Repo exposure cold compute | After a classified-seam cache miss, repo exposure now builds its index from the already-collected workspace bytes and reuses cached file facts when inputs are unchanged. |
| Latency trace | `repo-exposure-latency-report` now records a `file_fact_cache` trace phase with hit/miss/corrupt/store-error counters. |

## Local Evidence

Two consecutive local `cargo xtask repo-exposure-latency-report` runs on this
workspace showed the file-fact warm path working:

- First run: `file_fact_cache` reported `hits_0_misses_134_corrupt_0_store_errors_0` at about 3065 ms.
- Second run: `file_fact_cache` reported `hits_134_misses_0_corrupt_0_store_errors_0` at about 328 ms.

The full repo-exposure command still timed out later in cold classification, so
the next product step should make `ripr pilot` bounded and explicit under
timeout rather than broaden analyzer scope.

## Next Work Item

`pilot/budget-aware`

Make `ripr pilot` bounded and clear when analysis is partial or times out.
Preserve analyzer outputs and public schemas unless an intentional versioned
schema change is made.

## What Not To Do

- Do not cache rendered repo-exposure JSON or Markdown.
- Do not cache LSP diagnostics, hover text, SARIF, badges, or agent packets.
- Do not change static classifications to make latency reports look better.
- Do not hide partial or timed-out analysis as a complete pilot result.
