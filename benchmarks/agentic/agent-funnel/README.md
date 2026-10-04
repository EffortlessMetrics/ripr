# Benchmark B1: agent-funnel E2E

Executable producer-consumer benchmark for the `ripr first-action` agent
funnel. The harness runs the exact printed commands byte for byte from a
foreign working directory (launch directory != workspace root), the way a
consuming agent pastes them.

## Fixture

`input/` is a minimal Cargo package with one real analysis seam
(`discounted_total` boundary in `src/lib.rs`, weak tests in
`tests/pricing.rs`). The harness copies it into a temp dir and commits it
to a real Git repository, so every snapshot carries a concrete identity.

## Procedure

1. Produce `before.repo-exposure.json` on the base commit, advance HEAD.
2. Run `ripr first-action` from the foreign launch directory.
3. Execute the printed `after_snapshot`, `analysis_outcome`, `verify`, and
   `receipt` commands literally through bash (script file, not `bash -c`).
4. Complete the receipt step with the status-prescribed
   `ripr agent receipt --out target/ripr/reports/agent-receipt.json`.

## Oracle (all byte-oriented, on-disk, cross-process)

- `target/ripr/workflow/agent-verify.json` exists.
- `target/ripr/workflow/analysis-outcome.json` exists beside it.
- The receipt's `provenance.{verify,before,after}_artifact.sha256`
  commitments equal the sha256 of the exact bytes now on disk.
- The receipt carries the fixture seam and the funnel-written outcome with
  `analysis_outcome_status: complete`.
- `ripr agent status` re-reads the persisted chain: all five artifacts
  `present`, no `stale_artifact` warnings.

## Anti-gaming twins

- **stdout-only twin**: the printed verify redirect is mutated away, so the
  verify document goes to stdout only. The command still exits 0, but the
  oracle must FAIL (no `agent-verify.json` on disk). Exit-status grading
  would pass; the byte oracle does not.
- **stale-bytes twin**: a fabricated `agent-verify.json` is planted before
  the funnel runs. The funnel must overwrite it (raw byte inequality) and
  the receipt must bind the fresh bytes, never the decoy digest.

## Receipt

The happy-path test writes
`target/ripr/reports/agent-funnel-benchmark.json`
(`ripr-agent-funnel-benchmark-v1`: revision, runner class, analyzer
version, named checks, claim boundary), mirroring the
`targeted-rerun-benchmark` receipt pattern.

## Run

```sh
cargo test -p ripr --test agentic_bench_agent
```

Needs a usable POSIX `bash` (Git Bash on Windows). Absence skips locally
and fails under `GITHUB_ACTIONS`, like `agent_command_journey`.

Claim boundary: named-fixture static funnel behavior only; this does not
claim runtime mutation behavior, coverage adequacy, or universal latency.
