# RIPR-SPEC-0221: Agent surface benchmark (CLI/MCP/LSP latency, actionability, determinism)

Status: proposed

Owner: xtask

Created: 2026-10-03

Linked issues:

- #5257 (this slice: `cargo xtask bench-agent-surfaces`)
- #1578 (closed; the in-process `lsp-performance-report` this benchmark
  complements with real-process, cross-surface numbers)
- #5213 (CPU time / peak memory are unreportable; this slice measures
  wall-clock only)
- #5200 (the in-house benchmark precedent for corpus generation, env
  pinning, `run_status`, and claim-limit conventions)

The receipt feeds the METRICS.md capability rows "Static runtime by mode"
and "LSP diagnostic refresh latency", which previously had no repeatable
cross-surface measurement.

Support-tier impact:

- None. This is an advisory xtask measurement slice; it adds no language
  support claim, no product behavior change, and no new gate.
  [docs/status/SUPPORT_TIERS.md](../status/SUPPORT_TIERS.md)

Policy impact:

- None. It writes only generated reports under
  `target/ripr/reports/bench-agent-surfaces.{json,md}` and regenerable
  corpora under `target/ripr/bench-agent-surfaces/` (both gitignored). No
  new file-policy exception, no new dependency, no network access.

## Problem

An agent consumer of ripr calls `ripr check --format json`, speaks MCP to
`ripr mcp --stdio`, or consumes LSP diagnostics from `ripr lsp --stdio`.
Four things must be true and are not currently measured across those
surfaces:

1. it gets an answer fast enough to stay in its loop (cold and warm, at
   corpus scales it will actually meet);
2. the MCP/LSP round-trips it is documented to make stay responsive and
   bounded;
3. the findings it receives carry actionable material — targeted test
   intent, evidence paths, repair readiness, and disclosed unknowns —
   countable from the versioned JSON rather than prose;
4. repeated runs of the same binary on the same input do not silently
   change the answer.

Latency evidence is scattered: `repo-exposure-latency-report` covers one
repo-exposure path, `lsp-performance-report` runs an in-process saved-edit
harness and explicitly leaves the latency envelopes as proposals. No
receipt ties the three live surfaces to one identity-bound measurement.

## Behavior

One new xtask command, `cargo xtask bench-agent-surfaces`, writes
`target/ripr/reports/bench-agent-surfaces.{json,md}` (schema
`bench-agent-surfaces-v1`) with five metrics over three pinned corpora:

- **M1 — CLI end-to-end analysis latency.** Wall time around
  spawn→exit of `<ripr> check --format json` per corpus: cold samples on
  a fresh `RIPR_CACHE_DIR` scratch each, warm samples on a primed pinned
  cache, execution order interleaved and recorded. Per sample: exit
  status, stdout/stderr bytes, finding count. Reported per corpus ×
  population: n, min, p50, p95, max, and the warm/cold p50 speedup.
- **M2 — MCP stdio round-trip latency.** Newline-delimited JSON-RPC
  sessions (`initialize`, `notifications/initialized`, then
  `tools/call ripr_workspace_status`, `ripr_refresh` twice — first is
  server-side cold, second warm — `ripr_list_gaps`, and `ripr_get_gap`
  for the first listed gap id), replies drained on a reader thread so a
  blocked server cannot deadlock the harness. Reported per op × corpus:
  n, p50, p95, response bytes.
- **M3 — LSP first-diagnostics latency.** `Content-Length` framed
  sessions: initialize (corpus root, typed initializationOptions),
  initialized, then `textDocument/didOpen` for one pinned document; the
  sample is didOpen-written → first `textDocument/publishDiagnostics`
  for that URI, with the published diagnostics count recorded so a
  zero-diagnostic fast answer is visible as such. Cold sessions use a
  fresh cache scratch; warm sessions share one primed cache. M3 is
  measured on tiny and mid, where the interactive first-publish contract
  completes within bounded time. For the repo corpus the first publish
  is bounded by a workspace-scale analysis that does not complete within
  any reasonable per-sample ceiling on large checkouts, so repo M3
  normally ends as the M3 per-sample timeout — a distinct, named outcome
  recorded in the receipt as `recorded_limitation`, never silently
  dropped and never read as an empty-population gate failure (that gate
  stays reserved for corpora whose population can exist; a run whose
  repo sessions do observe a first publish replaces the limitation with
  measured populations). The authoring session's observations, kept here
  rather than in the portable receipt: a traced cold session held
  `analysis_outcome` null for 720 s, the same scope's cold CLI check
  took 572 s, and warm-cache sessions produced no document publish
  within 600 s.
- **M4 — output actionability.** Pooled from the M1 envelopes per
  corpus: findings total, the seven static-class histogram,
  actionable-intent fraction (missing discriminator or typed next
  action), evidence-path fraction, related-test-evidence fraction,
  unknown-disclosure fraction (unknown is a valid result; this is a
  disclosure direction, not a "fewer unknowns" direction),
  `analysis_outcome` completeness and typed limitation counts, repair
  readiness only where the contract actually emits it (a named absence
  otherwise), and `finding_alignment.summary` recorded as alignment
  evidence or `alignment_absent`.
- **M5 — determinism.** Raw stdout byte equality of repeated
  `check --format json` runs on the content-fixed corpora (tiny, mid):
  2 cold runs (independent fresh caches) and 5 warm runs (shared primed
  cache) per corpus. On mismatch, canonical comparison after dropping
  an explicit allowlist of disclosed-volatile fields (initially empty);
  a difference surviving normalization is a determinism failure.

Corpora are content-pinned, offline, and digest-recorded in the receipt
identity:

- **tiny** — derived from the checked-in
  `crates/ripr/examples/sample` bytes; its diff is rewritten to
  corpus-relative paths and a patch round-trip (derived before-state +
  rewritten diff == checked-in after bytes) is required before the
  corpus is usable.
- **mid** — generated deterministically (no clock, no rng): 20 packages
  × 25 Rust files with seeded predicate-boundary, error-path and
  return-value behaviors and a written strong/weak/absent oracle mix.
- **repo** — the checkout the bench runs in, diff-scoped against
  `git merge-base HEAD origin/main` (falling back to `HEAD~1` when the
  merge-base equals HEAD); the resolved base is asserted against the
  envelope `base` field (#3940). The repo corpus reports whatever the
  checkout diff holds: tiny/mid are constructed to carry findings, so a
  zero-subject run there is a gate violation, while a zero-findings
  repo run is a recorded disclosure (`recorded disclosure: the checkout
  diff holds no findings`), never folded into a numerator and never a
  violation. Two further repo-scale structural outcomes are recorded by
  name instead of failing the run, because on a large checkout the
  population itself can be the finding: (1) when the checkout's diff
  exceeds `RIPR_MAX_DIFF_CHANGED_RUST_LINES`, ripr fails closed with the
  typed `diff_scope_oversized` limited artifact before probe expansion
  (docs/OUTPUT_SCHEMA.md); every repo M1 sample classified as that
  typed refusal is recorded as `limited_diff_scope_oversized` with the
  analyzer's own reason, and a fully refused corpus emits
  `recorded_limitation` instead of an empty-population violation —
  a mixed corpus with any valid sample still gates as usual;
  (2) when the M3 first publish is bounded by a workspace-scale
  analysis that does not complete within any reasonable per-sample
  ceiling, repo M3 ends as the named M3-timeout limitation (see the M3
  bullet for the authoring session's observations). Downstream M2
  `no_snapshot` failures that follow a
  refresh attempt carrying a typed failure are named chained absences
  citing that refusal. Cold repo-scale samples also legitimately exceed
  the default 120 s per-sample timeout on large workspaces; the
  `RIPR_BENCH_AGENT_SURFACES_TIMEOUT_MS` environment override exists
  for that scale and the recorded run names the value it used.

The derived/generated corpora carry two pinned-identity commits — the
before-state as the base commit and the after-state as HEAD, worktree
clean — so every surface measures the same behavior change: the CLI
through `--diff`, the MCP server through its committed-diff analysis
against the resolved default base (HEAD~1), and the LSP through
baseRef `HEAD~1`. Every child run
gets `RIPR_CACHE_DIR` pinned explicitly and
`RIPR_REPO_EXPOSURE_SEAM_LIMIT` pinned to the product default, so caller
environment cannot silently change the measured quantity.

The receipt records source revision, binary path and SHA-256, host
class, toolchain, timestamp, and per-corpus digests. `--compare <path>`
against a stored prior receipt flags any M1/M2/M3 p50 worsening more
than 25% and an M1 warm/cold speedup below 1.0 as `regressed`. Status
semantics: `pass`, `regressed` (compare-only, exits non-zero), `warn`
(timed-out sample; exits zero), `fail` (any hard validity gate; exits
non-zero). Hard validity gates: non-zero child exit where success is
expected, unparseable check envelope, zero findings on an expected-
findings corpus, MCP `isError`/oversize on an expected-success op, LSP
session failure, corpus digest mismatch, determinism failure, and an
empty population. Absolute millisecond floors are not gated; host-
dependent numbers stay descriptive. The command is advisory and never
joins `precommit` or CI.

## Required Evidence

- A real end-to-end run on a clean checkout writing both report files
  with populated M1–M5 samples for every corpus whose population can
  exist (no zero-subject populations there); repo M1/M3 are bounded by
  the structural outcomes already specified above (the typed
  diff-scope refusal and the M3 per-sample timeout) and satisfy this
  clause as named `recorded_limitation` entries, not as silent gaps.
- A `--compare` negative control: a prior receipt with artificially
  lowered p50s (so the real child looks more than 25% slower) is flagged
  `regressed` with the named metric.
- A determinism negative control: an injected volatile field fails
  `determinism_outcome` with the empty check-envelope allowlist and
  passes as disclosed only when the field is explicitly allowlisted.
- Validity-gate evidence: forced child failure and forced zero-finding
  classifications produce named gate violations and non-zero exit.

## Non-Goals

- Not mutation testing; no mutants are built or run and all vocabulary
  stays static (`exposed` … `static_unknown`).
- Not a coverage dashboard; no coverage percentages or raw-volume
  counters. M4 measures disclosure and intent presence.
- No analyzer, output-schema, MCP, or LSP behavior changes; problems the
  measurements reveal get their own issues.
- No network access and no external repo clones (that stays
  `eval-sweep --clone` territory).
- No CPU-time or peak-memory instrumentation (#5213 owns that gap).
- No new gate in default CI, no `precommit` membership, no
  release/readiness claim changes, no new dependencies or crates.

## Acceptance Examples

- `cargo xtask bench-agent-surfaces` exits 0 with status `pass` on a
  clean checkout, writes `bench-agent-surfaces.{json,md}`, and the
  printed table has M1/M2/M3 rows per corpus and population plus M4/M5
  sections.
- `cargo xtask bench-agent-surfaces --compare <prior.json>` where the
  prior receipt's p50s are artificially halved exits non-zero with
  status `regressed` naming the metric.
- A determinism unit test injects a volatile field: the outcome is a
  failure under the empty allowlist and a disclosed pass naming the
  field once allowlisted.
- Forcing a child failure or a zero-finding envelope produces a named
  `non_zero_child_exit` / `zero_findings` violation and non-zero exit.

## Test Mapping

- `xtask/src/reports/bench_agent_surfaces.rs` unit tests: option parsing
  and run-count defaults; nearest-rank percentiles; determinism outcome
  (stable / injected-volatile failure / undisclosed-allowlist failure /
  disclosed pass naming the field); mid corpus generation determinism
  and single-line mutation; mid diff round-trip through the patch
  applier; oracle-mix shape; sample-diff rewrite and round-trip against
  the checked-in bytes; M4 aggregation over a synthetic envelope
  (fractions, alignment absence, repair-readiness named absence);
  status ordering; `--compare` threshold and ratio boundaries; M1
  sample classification over the validity gates (with a real exiting
  child for the nonzero-exit witness); file-URI form; gap-id extraction
  and named absence.

## Implementation Mapping

- `xtask/src/reports/bench_agent_surfaces.rs` — the benchmark module
  (corpus preparation, the five metrics, gates, receipt, markdown).
- `xtask/src/reports/mod.rs`, `xtask/src/command.rs`,
  `xtask/src/dispatch.rs` — registration, help, and the command
  mutability catalog (`report_only`, never CI-enforced).
- Reuses only `crate::run` subprocess helpers, `crate::write_report`,
  `ripr_debug_binary()`, `crate::blind_journey::sha256_hex`, and
  `ripr::process_owner::OwnedProcess` for owned-subprocess containment
  (kill-on-drop server sessions). Wire shapes follow
  `crates/ripr/tests/mcp_stdio.rs` and
  `crates/ripr/tests/lsp_lifecycle.rs`.

## Metrics

- M1 CLI cold/warm p50/p95 per corpus and the warm/cold p50 speedup
  (METRICS.md "Static runtime by mode").
- M2 MCP op p50/p95 per corpus with response bytes.
- M3 LSP didOpen→first publishDiagnostics p50/p95 per corpus with the
  published diagnostics count (METRICS.md "LSP diagnostic refresh
  latency").
- M4 actionability fractions per corpus as specified under Behavior.
- M5 byte-stability per population with any disclosed-volatile fields
  named.
