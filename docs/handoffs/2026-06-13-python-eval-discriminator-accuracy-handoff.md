# Python eval + discriminator-accuracy session handoff

**Date:** 2026-06-13
**Active tracker:** [#1160](https://github.com/EffortlessMetrics/ripr-swarm/issues/1160) (Python usable-tier readiness) and `.ripr/goals/python-repair-routing.toml` (Campaign 30)
**Watchpoint:** EffortlessMetrics/ripr#1430 (Ruff parser substrate — not a release gate)
**Source:** continuation context for resuming the Python release-readiness campaign after the external eval-sweep and the discriminator over-credit fix.

This packet is the connective tissue a future thread needs to resume. The durable
records hold the rest: `#1160`, the goals tracker, `docs/LEARNINGS.md`, and the
specs. Do not restate them — start here.

---

## Where we are

The Tier A external eval engine is built and the analyzer's core accuracy
invariant is fixed. Landed this session:

- foundation: `.gitattributes` (line-ending hygiene), ADR 0009 ↔ #1430 cross-link
- engine: eval-sweep harness (`RIPR-SPEC-0086`), `not_run` honesty gate, runtime
  fidelity (built-binary invocation), per-repo real diffs
- accuracy: **discriminator sink-alignment fix (#1172)** — the highest-value change
- docs: learnings encoded across `STATIC_EXPOSURE_MODEL.md`, `RIPR-SPEC-0028`,
  `RIPR-SPEC-0086`, `LEARNINGS.md`, `AGENTS.md`; README front-door contract
  adopted and `check-readme-state` retargeted to enforce it

`ripr` is validated on four external repos (tenacity, click, six, attrs): real
stable gaps, honest no-gaps, and fail-closed on unsupported shapes. **No error
*rate* is measured yet** — that is the next phase.

## Start here (next three PRs, in order)

1. **Output the alignment** (the next work item). Surface `changed_sink`,
   `observed_sink`, `oracle_alignment`, and `alignment_reason` on findings so a
   user can see why `ripr` was not fooled by a strong-but-orthogonal oracle. The
   classifier already computes the alignment (`strong_oracle_observes_owner` in
   `analysis/language/python.rs`); this exposes it in the output contract.
2. **Eval-sweep classification distribution.** Make `cargo xtask eval-sweep`
   record the exposed / weakly_exposed / no_static_path / limitation + confidence
   distribution, not just gap-ID stability. This turns the sweep into a `ripr+`
   measurement instead of a robustness floor.
3. **Tier B judged-diff panel.** The release gate: measured **false-actionable**
   and **false-`exposed`** rates, on a corpus that includes should-stay-quiet
   cases (direct-boundary tests that must read `exposed`), not only should-gap
   boundary flips.

## Open threads

- `__call__`-via-local-instance over-downgrade (#1172 known limitation):
  `stop = X(); assert stop(s) is True` over-downgrades because resolving `stop`'s
  type needs local-variable tracking beyond syntax-first analysis. Conservative
  but real.
- `wip/response-cli-assertion-20260612`: an unfinished `cli_assertion` /
  `response_assertion` feature, parked during the line-ending recovery. Decide
  finish-or-discard; it is not on the critical path.
- The corpus is four repos and only "should-gap" diffs. Scale it and add
  should-stay-quiet cases before claiming any error rate.

## Gotchas

- **Two repos:** work lands in `ripr-swarm` (dev trunk) and promotes to `ripr`
  (release/distribution authority). The README is the public-`ripr` front door.
- **Auto-merge is off.** Merge manually after CI; stacked PRs go `BEHIND` and
  need `gh pr update-branch` plus a full Rust-matrix re-run. `codecov/patch` is
  advisory (informational).
- **Re-run the sweep:** `cargo xtask eval-sweep --clone` (checkouts persist at
  `target/ripr/eval-sweep/checkouts/`; per-repo diffs in
  `fixtures/python-eval-sweep/diffs/`). The default no-clone run reports
  `not_run`, never a vacuous `pass`. Runtime is real analysis time (built binary).
- **Gating quirks:** `check-static-language` bans `proven` in all tracked prose;
  `check-readme-state` now enforces the front-door contract; `check-campaign`
  needs the exact work-item schema and the "not the active Codex Goals manifest"
  phrase on one line; `fixtures/python-eval-sweep` needs the manifest-only
  exemption in `xtask/src/main.rs`.
- **Policy gates are advisory at merge time.** Branch protection on `main`
  requires only `Ripr Rust Small Result`; the whole `source-of-truth` job
  (`check-support-tiers`, `check-static-language`, `check-doc-index`,
  `check-campaign`, …) runs but does **not** block the merge button. A red
  `source-of-truth` once rode into `main` (RIPR-SPEC-0088 without a
  `SUPPORT_TIERS.md` reference) and broke `check-support-tiers` for every
  later PR until a one-line fix. **Read `source-of-truth` yourself and refuse
  to merge on red** even though GitHub allows it. Also: a PR can fail a gate it
  did not touch when `main` advanced underneath it — reproduce against
  `origin/main`, and if `main` is already broken, fix it in a tiny unblock PR
  before rebasing dependent work. Tracked for a real fix in issue #1181.

## The invariant to protect

`exposed` requires the strong oracle to observe the changed sink — not merely
reach the owner. Reach plus a strong oracle is the coverage mistake. See
`docs/STATIC_EXPOSURE_MODEL.md` (Discrimination vs Coverage), `RIPR-SPEC-0028`
revealability, and the `strong_oracle_observes_owner` tests.
