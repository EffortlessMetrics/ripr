# Proof-routing dry-run evidence (0.9.0 candidate)

**Date:** 2026-06-10
**Issue:** #1078 (P16 evidence collection)
**Informs:** #1080 (P17 docs/spec routing), #1079 (P18 report/schema routing)
**Source artifact:** generated from `cargo xtask proof route` over five representative merged PRs; raw working copy at `target/ripr/reports/p16-proof-route-evidence.md`.

This packet answers the P16 question: across representative PR categories, what would proof routing have skipped, and would any skip have dropped a lane that carried real signal? Routing is **advisory-only** today (`proof_route.rs` hardcodes `routing_state = "advisory-report-only"`), so CI ran every lane and the skips are **counterfactual inferences**, not observed enforcement.

---

## Explicit conclusions

| Question | Answer |
|----------|--------|
| **P17 docs/spec routing safe to implement?** | **Yes — docs/markdown-only category only.** The single docs-only sample (#1063) is unambiguously safe; no other category clears. |
| **P18 report/schema routing safe to implement?** | **Not yet.** xtask/report/schema (#1067) and release-like (#1069) read **ambiguous** (static-language + coverage skips have observable relevance). Requires the manifest fixes below first, then re-evidence. |
| **Manifest/router fixes required before any code routing** | (1) **Uncovered lanes:** `rust`, `msrv`, `coverage`, `policy` are owned by no proof pack, so routing marks them skipped on every PR — they must be mapped onto the code packs or pinned unconditional before P18. (2) **Keep `static-language` unconditional** — it showed real relevance on static-output PRs. (3) **output-contracts lane gap** (#1079): the pack requires `goldens check` + `fixtures` but its lane does not run them. |
| **Release proof remained protected?** | **Yes.** `release-package` (`never_routed = true`) never appeared in `skipped_lanes` on any PR; `release_proof_required = true` fired correctly on every release-surface PR (#1067, #1069, #1072). |
| **Unknown surfaces remained conservative?** | **Partially — and this is why mixed/unknown is unsafe.** #1072 (touching `policy/proof-packs.toml`, an unmatched surface) correctly forced 7 required lanes incl. release proof, but the routed verdict still skips `rust`/`msrv`/`policy` because those lanes are unowned (see uncovered-lanes finding). Unknown-surface handling is not yet conservative enough for enforcement outside docs-only. |

**Bottom line:** enable the docs/markdown-only skip in P17; do **not** enable any code/report/schema skip until the uncovered-lane and output-contracts manifest gaps are closed and re-evidenced.

---

## The uncovered-lanes finding (key P18 prerequisite)

The proof-pack manifest's `ci_lane` values cover only: `docs`, `static-language`, `output-contracts`, `traceability`, `routed-rust-small` (×2 packs), `vscode-e2e`, `release-readiness-proof`. The heavy/important CI lanes **`rust`, `msrv`, `coverage`, and `policy` are owned by no pack at all**, so `proof route` reports them `skipped (no_matched_surface)` on *every* PR — including pure Rust-code PRs (this is precisely why #1056 routed to require only `static-language`).

Consequence for sequencing:
- **Docs-only (P17) is unaffected:** a markdown PR legitimately needs none of `rust`/`msrv`/`coverage`/`policy`, so "skipping" them is correct.
- **Any code/report routing (P18) is blocked** until those lanes are mapped onto the code packs (`analysis-fixture` / `xtask-report`) or pinned unconditional — otherwise enforcement would skip the real build/test/coverage lane on published Rust.

---

## Sample

| PR | Commit | Category | Changed files | Release proof required | Required lanes | Skipped lanes |
|----|--------|----------|---------------|------------------------|----------------|---------------|
| #1063 | 6affb336 | docs/markdown-only | 6 | false | `docs` | 25 |
| #1056 | 5751cd8f | normal Rust code | 5 | false | `static-language` | 24 |
| #1067 | f3b7a9fe | xtask/report/schema | 11 | true | `docs`, `routed-rust-small`, `release-readiness-proof` | 20 |
| #1069 | 99eed4b7 | release-like | 7 | true | `docs`, `routed-rust-small`, `release-readiness-proof` | 20 |
| #1072 | f932cb1f | mixed/unknown-surface | 7 | true | `docs`, `routed-rust-small`, `static-language`, `output-contracts`, `traceability`, `vscode-e2e`, `release-readiness-proof` | 18 |

Sample strategy: all five are real merged PRs after #1070 (where P16 artifacts begin). No synthetic PRs were created. The `release-like` slot (#1069) touches `CHANGELOG.md` + xtask; no PR in this window touched `Cargo.toml`/`Cargo.lock`, so true version-bump release evidence should be confirmed against the source release PR (#1424) during Phase R rather than asserted here.

---

## Routed-vs-actual per PR

### #1063 — docs/markdown-only — **SAFE**
Three markdown files (DOCUMENTATION.md, PROOF_ROUTING.md, ROADMAP.md); no Rust/policy/fixture/editor surface. `release_proof_required=false`. CI: `routed-rust-small` on CX43 SUCCESS, rust-tests-junit SUCCESS, rust-coverage SUCCESS, source-of-truth SUCCESS, cargo-deny SUCCESS, codecov/patch SUCCESS; `rust`/`msrv`/`vscode` independently skipped by CI. No failures. Every routed skip is a lane with no plausible failure mode on pure markdown.

### #1056 — normal Rust code — **UNSAFE**
Published-crate refactor (commands.rs, commands/pilot.rs). Routed to require only `static-language`, skipping `rust`/`routed-rust-small`/`coverage`. **codecov/patch FAILED**, confirming the coverage lane had real signal. Skipping build+test on a published Rust code-motion PR is a material gap.

### #1067 — xtask/report/schema — **AMBIGUOUS**
Introduces proof_route.rs (new static output) + CHANGELOG.md. Routed skips `static-language` (the gate validating static output) on the very PR adding static-output paths, and skips `coverage` where **codecov/patch FAILED**. xtask is unpublished (lowers rust-skip risk), but both skips have observable relevance → ambiguous.

### #1069 — release-like — **AMBIGUOUS**
Adds proof_route.rs rendering to pr-summary (437 new lines) + CHANGELOG.md. Same pattern as #1067: `static-language` and `coverage` skipped where codecov/patch FAILED and new static text was added. `release_proof_required=true` and `release-readiness-proof` required are correct, but the two skips have real relevance → ambiguous.

### #1072 — mixed/unknown-surface — **UNSAFE**
Modifies policy/proof-packs.toml + adds xtask validation (proof_preflight.rs, expanded proof_route.rs). Unknown surface correctly forced 7 required lanes incl. release proof, but still skips `rust`/`msrv`/`policy`. **cargo-deny and source-of-truth were CANCELLED** (in-flight, not passively skipped) on a PR hardening release-proof invariants — required signal did not complete. Unsafe.

---

## Caveats

1. **Advisory-only routing:** skips are counterfactual inferences; no skip was actually enforced.
2. **Single sample per category:** a "safe" verdict could be accidentally safe. Docs-only should accrue more samples before broad confidence, though one clean sample plus the structural argument (markdown matches only `docs-spec`, `release_proof_required=false`) is adequate to *enable* the docs-only skip.
3. **`routed-rust-small` multi-platform gap:** maps to several jobs (CX43/CPX42/CX53/GitHub-Hosted); typically only CX43 ran, so even the required lane lacked full multi-platform compile coverage.
4. **CANCELLED ≠ SKIPPED:** droid-review was cancelled on most PRs; cargo-deny/source-of-truth cancelled on #1072. Cancellation is terminated in-flight work, not a safe planned skip.
5. **codecov/patch blocking status undocumented:** it FAILED on #1056/#1067/#1069; if non-blocking it does not gate merge but still represents signal a skipped coverage lane would suppress.
6. **`release-readiness-proof` job name:** appears in required_lanes but no CI job of that exact name was observed — confirm the mapping (or that it is enforced) before relying on it for release routing.
7. **release-package `never_routed` held** across all five PRs — the core invariant is intact.

---

## P17 acceptance derived from this evidence

- Skip heavy lanes **only** when every changed file matches `docs-spec` (zero unmatched), no other pack matched, and `release_proof_required=false`.
- Keep running on docs PRs: spec-format, spec-numbering, doc-index, markdown-links, static-language, **traceability/capabilities** (the `docs-spec` `ci_lane="docs"` omits these — they must not be silently dropped), and the proof-route artifact.
- Unknown/mixed/release surfaces continue to route full proof. Mechanism must keep every branch-protection **required** check reporting success on a skipped docs-only PR (no merge block) while guaranteeing non-docs PRs run full proof.
