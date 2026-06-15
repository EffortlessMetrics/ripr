# Handoff: Honesty Hardening + TypeScript Must-Use Campaign Closeout

Date: 2026-06-15
Branch / PR: `docs/honesty-campaign-closeout` / pending at authoring
Latest landed spec chain: `RIPR-SPEC-0103` … `RIPR-SPEC-0110`

## Current Work Item

`campaign/honesty-and-must-use`

This campaign moved RIPR from "produces interesting evidence" to "produces
**honest, actionable, self-enforcing** evidence across the fleet," and from a
TypeScript *preview* lane to one that is **must-use for mainstream TypeScript**.
It did not bump the crate version, publish, or change the published `0.9.0`
support tiers; the release decision (likely `0.10.0`) is deliberately parked for
an explicit human call.

## What landed (by theme)

### 1. The fake-`exposed` class — fixed fleet-wide, then made a standing gate
The central adversary this campaign was a single recurring shape: **evidence
credited to a seam it does not actually observe → a false `exposed` /
`strongly_gripped` / `strong_oracle_observed` claim.** It recurred across
analysis stages, across languages, and was even baked into a golden. Each
instance was closed fail-closed, and then the *class* was turned into a standing
invariant:

```text
identity-not-token (Python)              #1242 / #1244 / #1247 / #1253
oracle-kind-vs-seam-family (TS)          RIPR-SPEC-0104  #1248
wrong-kind exemplar nomination (Rust)    RIPR-SPEC-0103  #1243
unwrap_err variant binding (Rust)        RIPR-SPEC-0106  #1252
error_path needs a variant oracle (Rust) RIPR-SPEC-0107  #1254
SideEffect observation guard (TS)        RIPR-SPEC-0098  #1236
stage confidence caps the headline       RIPR-SPEC-0109  #1219-D #1258
--------------------------------------------------------------------
THE CAPSTONE: evidence-promotion honesty meta-gate
                                         RIPR-SPEC-0108  #1257
```

`cargo xtask check-evidence-promotion-honesty` (RIPR-SPEC-0108) pins a
cross-language adversarial corpus (`fixtures/evidence-promotion-honesty-corpus/
corpus.json`). It reads each charter fixture's pinned golden and asserts the
class **independent of the golden** — so it catches a *dishonest re-bless* that
plain `goldens check` would accept (because `goldens check` only asserts
`binary == golden`). Every future fake-`exposed` fix registers a `cases[]` entry.
The invariant + corpus + gate are shared; the per-language matchers are not
unified (different taxonomies, different edge policies).

### 2. TypeScript: preview → must-use for mainstream
```text
single-hop re-export test discovery       #1231
package.json test-runner detection        #1233
.toThrow('payload') exact-error oracle     RIPR-SPEC-0097 #1234
tsconfig compilerOptions.paths aliases     RIPR-SPEC-0099 #1237 (opt-in, fail-closed)
named import-alias owner-call credit       RIPR-SPEC-0102 #1241
runner-disclosure honesty                  RIPR-SPEC-0101 #1240
missing_actionability_fields self-fix      #1251
advisory related-test codeLens             RIPR-SPEC-0100 #1238
```
End-to-end dogfood verdict: **must-use for mainstream single-package and simple
monorepo TS, no over-claims found.** TypeScript remains preview/advisory tier;
preview can be actionable when the full repair-packet contract holds, but it is
delegatable, not gate authority. Remaining gap: namespace-import owner calls — a
documented honesty-safe under-emit, not a release blocker.

### 3. LSP cockpit: performance is part of honesty
The LSP first-open ran the full-repo seam inventory (~336 s vs the CLI's ~14 s
diff pass) — dead-on-arrival, so agents would act on no state. `RIPR-SPEC-0105`
(#1250) defers the seam inventory off the interactive path and **discloses** it
(`run_status: "seams_deferred"`, a `limited`-family value); the full pass runs on
the explicit `ripr.refresh` command. A fast path may be partial only if the
status says so.

### 4. Rust usefulness (#1168, fully closed)
Error-return seams with dedicated `unwrap_err` tests that pin the exact variant
were falsely `weakly_gripped`. Slice 1 (#1243) kind-gated the exemplar; slice 2
(#1252) recognizes the `unwrap_err`+`assert_eq!(err, Variant)` oracle and binds
it to the matching seam variant — a sibling variant or a generic `is_err()` stays
`weakly_gripped` (fail-closed).

### 5. The receipt → outcome → route-quality loop (#1123)
Found ~90% already built and honest. The one missing link — a real producer for
receipt staleness/orphan detection — landed as `RIPR-SPEC-0110` (#1261):
`ripr receipt check --ledger` cross-references the receipt's `canonical_gap_id`
against the live gap set (`receipt_ok` / `orphan_receipt` / `receipt_gap_mismatch`,
non-zero on a real problem; **`not_available` when no ledger — never `receipt_ok`**).

## Durable learnings (detail in `docs/LEARNINGS.md`)

- **The fake-`exposed` is one cross-language class, now a standing gate, not
  whack-a-mole.** When you fix the same shape three times, enforce the property.
- **Goldens can encode dishonesty.** A re-bless is a semantic act; the meta-gate
  pins the expectation independent of the golden.
- **Verify the artifact — and the harness.** Gates / tests / builder self-reports
  are weak oracles; so is your own harness. A wrong binary path or a non-pinned
  rustfmt manufactures false *negatives*. Run the absolute worktree binary + the
  pinned toolchain; inject a unique marker to confirm your edits are in the binary.
- **Fail-closed is directional.** Under-emit before over-emit; `not_available`
  beats fake-zero; an unprovable stage caps confidence and cannot read as certain.
- **Honesty is tested at the surface.** Internal dishonesty becomes visible when
  projected (a finding contradicting its own evidence); usability is where honesty
  is tested; performance is part of honesty.
- **Dogfood is the highest-signal tool.** Every adversarial dogfood found real
  bugs the gates missed and re-grounded the roadmap. A closed-corpus "0
  false-`exposed`" measures the corpus, not the analyzer — construction, not
  sampling, finds silent over-credit.
- **Operating model:** scout (read-only) → adversarial spec → build → verify
  myself → gate. The scout-before-build repeatedly resolved uncertainty cheaply
  and caught spec holes before they shipped.

## What this campaign did NOT change

No crate version bump, no publish, no release-workflow change. TypeScript /
JavaScript / Python remain preview/advisory tiers. No full TypeScript
typechecker, no cross-language oracle graph, no autonomous edits, no generated
tests, no mutation execution, no default gate/badge authority for preview
evidence. The one published artifact (`ripr 0.9.0`) is untouched.

## What remains (next frontier)

- **Release decision** — the unreleased delta since `0.9.0` (TS actionable
  packets, cockpit performance, the fleet-wide honesty gate) is more than a
  patch; likely `0.10.0`. Human call. The version bump auto-publishes.
- **Small honesty residuals** — `#1255` (`missing_discriminators` lists a
  variant a real test pins), `#1146` (unify the per-surface fail-closed
  vocabulary), the Python owner-name/receiver-identity closure (in-progress).
- **Operational hardening** — `#1181` (make `source-of-truth` a required check),
  `#1058` (runner/disk-guard host-class sizing).
- **Code health** — `#1147` (module-size advisory; `xtask/src/main.rs` is large).
- **TS namespace imports** — documented under-emit; do only if dogfood demands.

## Boundaries for the next agent

Keep the published `0.9.x` crate honest. No version bump / publish / release
without an explicit human call. Every new fake-`exposed` fix registers a
`corpus.json` entry. Verify the rendered artifact, not the report. Do not stack
on another campaign's unmerged registry/docs surfaces.
