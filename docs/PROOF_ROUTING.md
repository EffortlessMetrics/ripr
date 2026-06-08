# Proof Routing

This document defines the proof-aware validation operating model: how a
change selects the proof it requires, instead of every change paying for
every proof. It is the repo-side mirror of the product's own routing rule.

```text
right proof
right surface
right cost
right time
```

The product routes repairs: one canonical gap, one bounded edit surface, one
verify command, one receipt. The repo routes proof the same way: one changed
surface, one proof pack, one focused preflight, one CI lane, one receipt.

## Why this exists

Measured on 2026-06-07, during the use-case spec spine merge wave:

- Five docs-only PRs each ran the full heavy Rust lane (workspace
  fmt/check/clippy/test plus the complete xtask gate suite) because the
  only required merge check, `Ripr Rust Small Result`, runs unconditionally
  on every PR with no path awareness.
- Concurrent heavy lanes collided on self-hosted scratch disks
  (issue #1058), tempfailing each other and forcing GC-window re-runs;
  docs-only changes spent roughly an hour of wall clock per merge on proof
  that could not fail for their changed surface.
- Of the gate commands those lanes ran, nineteen need no workspace build at
  all (`fmt --check`, the static-language/spec/policy/file/workflow check
  family, `cargo deny`); only four are build-heavy (`check`, `clippy`,
  `test`, coverage).

CI was acting as first discovery and as an undifferentiated bonfire. Both
are forbidden by this model.

## The operating rules

1. **PR proof is not release proof.** A PR runs the proof packs matched by
   its changed surfaces. Release-surface PRs and release branches always
   run the full release proof; proof routing must never skip release proof.
2. **Proof packs are the routing unit.** A proof pack names the paths it
   covers, the required commands, the advisory commands, the CI lane that
   runs them, what passing demonstrates, and what passing does not
   demonstrate. The pack manifest lives in `policy/proof-packs.toml`
   (planned; see the sequence below).
3. **Unknown surfaces route conservatively.** A changed file that matches
   no pack routes to the full proof, not to the cheapest lane. Routing can
   only narrow proof for surfaces it explicitly understands.
4. **Local preflight before CI.** The expectation is
   `cargo xtask proof preflight` (planned) or the matched packs' required
   commands locally before push. CI confirms proof; it does not discover
   failures first. Using CI as the first execution of a gate you could run
   locally is an anti-pattern.
5. **Advisory stays visible, blocking stays earned.** Routed-away lanes are
   recorded as skipped-with-reason in the proof route report, never
   silently dropped. A lane becomes blocking for a surface only with
   evidence; it stops running for a surface only with evidence.
6. **Receipts over vibes.** Preflight and CI lanes write proof receipts.
   The dry-run artifact phase compares routed proof against actually-run
   proof before any lane is skipped for real.

## What CI must not be used for

```text
first discovery of failures a local gate would have caught
hypothesis testing by repeated push
retry roulette against infrastructure flakes
undifferentiated full proof for surfaces it cannot fail
```

Infrastructure tempfails (for example a disk-guard exit 75) are routed
limitations, not proof failures; they are re-run on infrastructure terms
(issue #1058), and they must not be paid at all by changes whose proof
never needed the heavy lane.

## The delivery sequence

This model lands in slices, each with its own evidence:

```text
1. this operating model            (docs)
2. policy/proof-packs.toml         (manifest + validity check)
3. cargo xtask proof route         (read-only report: changed files ->
                                    packs -> required/advisory/skipped
                                    lanes with reasons and cost)
4. cargo xtask proof preflight     (runs matched required commands,
                                    writes a proof receipt)
5. PR summary integration          (route visible to reviewers)
6. CI dry-run artifact             (all lanes still run; artifact shows
                                    what would have run; divergence is
                                    reviewable evidence)
7. low-risk routing                (docs/specs, handoffs, doc-artifact
                                    policy, markdown-only changes)
8. report/schema pack routing      (output contracts, traceability,
                                    capabilities, focused xtask tests)
9. release-proof protection        (release surfaces pinned to full
                                    proof in the manifest and workflow
                                    contract checks)
```

Nothing skips a real CI lane before step 6 produces comparison evidence,
and step 9's protection is in place before routing touches anything near
the release path.

Slices 1–6 have landed. Slice 6 wires the route into CI as a pure
evidence artifact: on every pull request, each routed-Rust implementation
job (the three self-hosted runners and the GitHub-hosted fallback) runs
`cargo xtask proof route --base "$BASE_SHA" --head "$HEAD_SHA" || true` on
its PR-evidence path, appends `proof-route.md` to the run's step summary,
and uploads `proof-route.{json,md}` with the existing ripr reports. The
step is advisory (`|| true`) and adds no `if:` path filter, lane skip, or
gate — every lane that ran before still runs. The artifact records only
what routing *would* select, so divergence between routed proof and the
proof CI actually ran is reviewable before any real routing (slices 7–9).
The `check-workflows` contract asserts the advisory step is present on all
four PR-evidence paths so the artifact cannot silently regress.

## Initial proof-pack shape

The first manifest should cover these packs. Paths and commands are pinned
in `policy/proof-packs.toml` when it lands; this table states intent.

| Pack | Covers | Required proof core | Build-heavy? |
| --- | --- | --- | --- |
| `docs-spec` | `docs/specs/`, `docs/handoffs/`, markdown-only changes | spec-format, spec-numbering, doc-index, static-language, local-context | no |
| `static-language` | output renderers, user-facing strings | static-language, output contracts | partial |
| `output-contracts` | `crates/ripr/src/output/`, schema docs | output contracts, goldens, fixtures | yes |
| `traceability-capabilities` | `.ripr/traceability.toml`, capability docs | traceability, capabilities | no |
| `xtask-report` | `xtask/src/` report producers | focused xtask tests, report checks | partial |
| `analysis-fixture` | `crates/ripr/src/analysis/`, `fixtures/` | workspace tests, fixtures, goldens | yes |
| `editor-lsp` | `editors/vscode/`, `crates/ripr/src/lsp/` | extension compile/package, LSP smoke | yes |
| `release-package` | versions, changelogs, release workflows | full release proof, never routed away | yes |

A surface in more than one pack runs the union. A surface in no pack runs
the full proof.

## Claim boundary

Proof routing changes which proof runs where. It does not change what any
gate enforces, does not relax branch protection, does not make advisory
lanes blocking or blocking lanes advisory by itself, and does not apply to
release proof at all. Routing decisions are recorded artifacts
(`target/ripr/reports/proof-route.{json,md}`), reviewable like any other
evidence.
