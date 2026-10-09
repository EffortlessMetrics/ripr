# RIPR-SPEC-0206: Batched candidate-tree materialization

Status: proposed

Owner: product-analysis

Linked issues: #5015 (builds on #3237 / #3277 / #3548)

## Problem

`git_candidate_execution::materialize` extracted a candidate tree with one
sequential `git cat-file` subprocess per file. For an N-file tree that is N
sequential spawns (tens of ms each on Windows), no progress or phase
disclosure, and — because every per-blob call carried the full
per-invocation git deadline — a worst case of N × git_timeout for a slow or
hanging git. Each call also buffered up to 512 MiB in memory per blob.

## Behavior

- Materialization uses ONE streaming `git cat-file --batch` process for the
  whole tree (two git spawns total with the `ls-tree` listing), regardless
  of tree size. The materialization phase is O(1) in git processes.
- The session is a lockstep request/response protocol: one object ID is
  queued per request, the response header is parsed, and the announced
  bytes stream to the destination file in bounded chunks — a large blob can
  never deadlock against an unread pipe. The stdout reader feeds a bounded
  queue, so a slow destination stalls git through the pipe (backpressure)
  instead of accumulating chunks in memory.
- A single named overall deadline (the same value the per-blob calls each
  used before, enforced once) bounds the entire materialization phase —
  both git processes: the clock starts at materialization entry, `ls-tree`
  runs inside the remaining budget, and the batch session receives only
  what is left. Every blocking read recomputes its wait allowance from the
  absolute deadline, so a stream dribbling fragments just under each
  individual wait cannot outlive the budget. Deadline expiry classifies as
  the shared named `git_invocation_timeout` error with the same repair
  route as every other git invocation; cooperative cancellation is checked
  per request and per chunk. One residual limitation, unchanged from the
  per-blob path and disclosed in code: a single stalled OS-level file
  write is outside the deadline's reach.
- Fail-closed behavior is preserved exactly: unsupported entry modes,
  non-UTF-8 paths, traversal attempts, malformed batch framing, truncated
  streams, and missing objects are named errors; any failure fails the
  whole subject (the temp-root guard removes the partial tree), so no
  partial tree is ever presented as complete.
- Bounded totals: the materialized byte count is capped at
  `MAX_ARCHIVE_BYTES` (512 MiB) for the whole tree (previously a per-blob
  cap), and the `ls-tree` listing keeps its existing per-invocation cap.
- Blob bytes remain byte-identical to per-blob `git cat-file` output: the
  batch protocol returns raw blob bytes, preserving the #3548
  no-textconv identity argument against `git archive`.

## Non-Goals

- The identity-resolution and diff-derivation spawns are unchanged (already
  one call each).
- Wall-clock measurement: spawn count N+1 → 2 is structural, but the
  magnitude of the wall-clock improvement stays a measurement-pending
  `design_question` (the issue's own classification) until measured on a
  genuinely large candidate; no unmeasured speedup is asserted in tracked
  files.
- The unrelated per-blob `cat-file` consumers (`committed_source`,
  `python_repair_verification`) are untouched.
- Real mutation testing, coverage dashboards, and any evidence-promotion
  vocabulary remain out of scope per the product contract.

## Required Evidence

- Real-repository session round trip: one process answers many blob
  requests byte-identically to the raw `git show` oracle; a nonexistent
  object reports missing without desynchronizing the stream.
- Stream parser unit tests: chunk-boundary reassembly of headers, content
  with interior newlines, and framing; truncated-stream fail-closed;
  spent-budget timeout classification.
- Zero-budget session control: a blocking wait classifies as the named
  `git_invocation_timeout` with the raw prefix intact.
- Scale fixture: a 300-file nested tree plus a >1 MiB binary blob and an
  empty file materialize byte-identically to the `git show` oracle through
  the batched path.
- Existing byte-parity, attribute-conversion, long-path, worktree-isolation,
  and pipeline tests keep passing over the batched path.

## Acceptance Examples

- A candidate tree with 5,000 files materializes through two git processes
  (`ls-tree` + one `cat-file --batch`) instead of 5,001, under one
  `git_timeout` deadline for the whole phase.
- A tree whose materialized bytes exceed 512 MiB fails closed with a named
  total-limit error and the temp root is removed.
- A hung `git cat-file --batch` is terminated at the single overall
  deadline and surfaces the named `git_invocation_timeout` repair route.

## Test Mapping

- `crates/ripr/src/git.rs::tests::cat_file_batch_stream_reads_across_chunk_boundaries`
- `crates/ripr/src/git.rs::tests::cat_file_batch_stream_fails_closed_on_truncated_blob`
- `crates/ripr/src/git.rs::tests::cat_file_batch_stream_times_out_when_budget_is_spent`
- `crates/ripr/src/git.rs::tests::cat_file_batch_stream_enforces_the_absolute_deadline`
- `crates/ripr/src/git.rs::tests::cat_file_batch_stream_rejects_ready_bytes_past_the_deadline`
- `crates/ripr/src/git.rs::tests::cat_file_batch_session_round_trips_blobs_and_reports_missing`
- `crates/ripr/src/git.rs::tests::cat_file_batch_session_enforces_the_overall_budget`
- `crates/ripr/src/analysis/git_candidate_execution.rs::tests::batched_materialization_preserves_bytes_at_scale`
- Existing `git_candidate_execution.rs` byte-parity, attribute, long-path,
  worktree-isolation, and pipeline tests (unchanged, exercising the new
  path)

## Implementation Mapping

- `crates/ripr/src/git.rs` — `CatFileBatch` session, `CatFileBatchStream`
  parser (absolute-deadline waits), bounded backpressure queue,
  `git_invocation_timeout_message` (extracted from `poll_child`),
  `spawn_cat_file_batch_chunk_reader`; every stream failure aborts the
  owned process tree.
- `crates/ripr/src/analysis/git_candidate_execution.rs` — `materialize`
  entry validation up front, one budget clock for both git processes,
  batched streaming loop, whole-tree byte cap.

## Metrics

- Spawn count for the materialization phase: N+1 → 2 (structural).
- Cold materialization wall-clock before/after on a multi-thousand-file
  tree: measurement pending (no toolchain in the authoring environment);
  to be captured by a future dogfood receipt per the issue's acceptance
  sketch.

## Failure Modes

- Missing object: named `git cat-file blob <oid> is missing` error; the
  whole subject fails closed.
- Deadline expiry mid-materialization: named `git_invocation_timeout`; the
  partial tree is removed by the temp-root guard.
- Corrupt or truncated batch stream: named framing/truncation error; the
  owned process tree is terminated.
