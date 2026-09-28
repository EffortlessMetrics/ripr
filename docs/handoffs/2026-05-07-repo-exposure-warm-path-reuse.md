# Handoff: Repo Exposure Warm-Path Reuse

Date: 2026-05-07
Branch / PR: `repo-exposure-warm-path-reuse`

## Current Work Item

`cache/repo-exposure-warm-path-reuse`

This work item adds fact-layer reuse below the rendered-output layer. It does
not cache rendered JSON, Markdown, diagnostics, hover text, SARIF, badges, or
agent packets, and it does not change analyzer classifications or public output
schemas.

## What Changed

| Surface | Change |
| --- | --- |
| File facts | `FileFacts` and related fact DTOs are serializable so parser/file facts can be cached as internal fact layers. |
| Cache | Added `target/ripr/cache/repo-file-facts/0.1`, keyed by analyzer version, file path, and file content hash. |
| Repo exposure cold compute | After a classified-seam cache miss, repo exposure now builds its index from the already-collected workspace bytes and reuses cached file facts when inputs are unchanged. |
| Related-test evidence | Full repo seam evidence now reuses the same precomputed related-test context that the compact badge path already used. |
| Value resolution | Seam-independent value-resolution scans are built once per indexed test and reused while evaluating seams. |
| Latency trace | `repo-exposure-latency-report` now records a `file_fact_cache` trace phase with hit/miss/corrupt/store-error counters and cold sub-phases for discovery, indexing, seam inventory, evidence, and classification. |

## Local Evidence

Two consecutive local `cargo xtask repo-exposure-latency-report` runs on this
workspace showed the file-fact warm path working:

- First run: `file_fact_cache` reported `hits_0_misses_134_corrupt_0_store_errors_0` at about 3065 ms.
- Second run: `file_fact_cache` reported `hits_134_misses_0_corrupt_0_store_errors_0` at about 328 ms.

The full repo-exposure command still timed out later in cold classification, so
the product path moved through a bounded `ripr pilot` timeout surface before
the current first-screen clarity item.

After the related-test and value-resolution reuse was added, a long bounded run
populated the classified-seam cache for the current key. The default 30-second
`cargo xtask repo-exposure-latency-report` then passed on cache hits:

- `repo-exposure-json` completed in about 13 seconds.
- `repo-exposure-md` completed in about 13 seconds.
- The cold run still showed first-run `repo-exposure-json` taking about 175
  seconds locally, with `evidence_for_seams` at about 143 seconds and
  `cache_store` at about 25 seconds.

## Next Work Item

`pilot/first-screen-clarity`

Improve the pilot summary and terminal copy so the first screen explains what
was inspected, why the top seam matters, what focused test to write, and what
command to run next.

## What Not To Do

- Do not cache rendered repo-exposure JSON or Markdown.
- Do not cache LSP diagnostics, hover text, SARIF, badges, or agent packets.
- Do not change static classifications to make latency reports look better.
- Do not hide partial or timed-out analysis as a complete pilot result.
