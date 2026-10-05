# B6 edit-cage, production path

Executable oracle: `crates/ripr/tests/agentic_bench_cage.rs`.

The suite drives `ripr agent repair --phase before/after` through the
worktree-built `ripr` binary (the B1 pattern). `crates/ripr/src/edit_cage`
stays `pub(crate)`; the bench does not widen that API and does not
reimplement production matching, digest, or capture semantics.

## Fixture

`input/` is a minimal Cargo package with one real analysis seam
(`discounted_total` in `src/lib.rs`, tests in `tests/pricing.rs` that miss
the equality boundary). The harness copies it into a temp Git repository
and ignores `/target/` before running repair.

The bench `manifest.json` names the intended edit surface for the xtask
runner (`ripr-agentic-bench-manifest-v1`). Production cage policy still
comes from the repair packet `agent repair` derives; the two surfaces are
aligned on `tests/` vs `src/` for this fixture.

## Cases

1. **Out-of-surface edit.** `--phase before`, then edit `src/lib.rs` (and
   the selected test so the pair is not a no-movement refusal), then
   `--phase after`. Production must fail closed: attempt `after.verdict.status`
   is `violated`, the CLI exits 2 with the post-verify `not receipt-ready`
   refusal (not exit-3 `repair_after_refusal`), and neither the compatibility
   receipt nor an attempt-local copy is written.
2. **Stale snapshot / reread.** An in-surface finish writes a receipt.
   `ripr agent receipt --attempt` against the unchanged tree is the
   production reread. After the tree moves, that same receipt command must
   refuse (`after verdict binding is tampered or stale`) instead of replaying
   the old compliant verdict. Re-running `--phase after` must refuse as
   already finished. Production has no `superseded` cage token; those
   refusal strings are the expected-value oracle for the original B6
   supersession ledger.
3. **Over-budget capture.** Plant a sparse file one byte over production
   `MAX_CAPTURE_FILE_BYTES` (16 MiB) *before* `--phase before`, so both
   snapshots see the over-budget path as capture `Other` rather than a new
   out-of-surface edit. `--phase after` must record `incomparable`, exit 2,
   and write no receipt. The numeric bound is an expected-value pin of the
   production constant; the test does not reimplement capture.
4. **Invalid packet / attempt manifest.** After `--phase before`, corrupt
   the retained packet or `attempt.json`. `--phase after` must refuse with
   exit 2 before any receipt (compatibility file or attempt-local copy).
   Bench-manifest schema rejection stays with `cargo xtask agentic-bench`
   (the production owner); this suite does not copy that validator.

## Run

```sh
cargo test -p ripr --test agentic_bench_cage
cargo xtask agentic-bench
```

Claim boundary: named-fixture static repair-cage behavior through the
built CLI only. This does not claim runtime mutation behavior, coverage
adequacy, or that a `superseded` verdict exists on the production cage.
