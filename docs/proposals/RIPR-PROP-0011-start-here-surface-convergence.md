# RIPR-PROP-0011: Start-Here Surface Convergence

Status: proposed

Owner: Cross-lane; Lane 4 / PR-CI and CLI surfaces lead, Lane 3 consumes
editor-ready state, Lane 2 owns policy meaning

Created: 2026-05-17

Target campaign: Start-Here Surface Convergence

Linked specs:

- `RIPR-SPEC-0053`: Start-here surface convergence
- `RIPR-SPEC-0051`: First successful PR UX
- `RIPR-SPEC-0052`: Editor first-pr packet projection
- `RIPR-SPEC-0046`: Gap decision ledger
- `RIPR-SPEC-0044`: Preview evidence promotion packet

Linked ADRs:

- `ADR-0015`: Start-here surfaces use canonical gap records

Linked issues:

- #1148 `docs(product): open start-here surface convergence stack`
- #1150 `report: align PR/CI first screen on canonical repair unit`
- #1151 `cli: converge start-here command language`
- #1152 `receipt: standardize receipt lifecycle state`
- #1154 `output: standardize no-output and fail-closed states`
- #1155 `policy(language): define preview promotion proof criteria`
- #1156 `dogfood: record external-style start-here receipts`
- #1157 `campaign: close start-here surface convergence`

## Problem

RIPR now has the core first successful repair loop:

```text
diagnose setup
-> inspect one gap
-> copy repair packet
-> write one focused test
-> verify
-> receipt
-> refresh
-> inspect first-pr packet
```

The remaining product gap is convergence. Editor, CLI, generated CI, PR
evidence, report packet indexes, receipts, preview-language reports, and
release/install docs can each be correct while still forcing users to interpret
different report shapes.

The next improvement is to make every major surface answer the same first
question:

```text
What is the one safest next action, what proves movement, and what remains
limited or advisory?
```

## Users and surfaces

- First-time users running RIPR from a clean install or a normal PR.
- Reviewers reading generated CI summaries and uploaded report packets.
- CLI users moving through `doctor`, `first-pr`, `pr-ready`, and cockpit
  reports.
- Coding agents that need one bounded repair packet and a stop condition.
- Preview-language evaluators who need explicit promotion criteria before
  stronger claims.
- Release operators validating install, VS Code server resolution, and first
  receipt paths.

## Success criteria

- PR/CI, CLI, editor, docs, and receipts lead with the same canonical unit:
  gap identity, repair route, related test, verify command, receipt command,
  limits, and non-claims.
- Raw findings remain supporting evidence, not the primary user action.
- No-output and fail-closed states use consistent names across non-editor
  surfaces.
- Receipt lifecycle state is visible as found, missing, stale, gap-mismatched,
  improved, unchanged, or not applicable.
- Preview-language promotion criteria are explicit and do not silently promote
  TypeScript, JavaScript, or Python because routing exists.
- External-style dogfood proves the path on normal repo shapes, not just
  fixture-only happy paths.
- Release/install guidance points users to the same start-here workflow and
  recovery states.

## Proposed shape

Open a small cross-surface campaign. Each work item should be a scoped PR or
closed as a duplicate of an existing PR if current code already satisfies it.

1. Define this docs stack and GitHub issue burn-down map.
2. Align PR/CI first-screen output on the canonical repair unit.
3. Align CLI front-door commands on the same start-here language.
4. Standardize receipt lifecycle state across surfaces.
5. Standardize no-output and fail-closed states outside the editor.
6. Publish preview-language promotion criteria and required proof.
7. Record external-style dogfood receipts for the converged path.
8. Close the campaign with prompt-to-artifact proof.

## Alternatives considered

| Alternative | Why we are not picking it |
| --- | --- |
| Keep adding isolated reports. | That increases internal power while making first-use harder. |
| Make the editor the authority for all start-here state. | Lane 3 is a projection layer. PR/CI, CLI, policy, and release rails need their own consistent surfaces. |
| Promote preview languages because routing works. | Routing is not maturity. Promotion needs evidence quality, dogfood, related-test safety, static-limit coverage, and policy signoff. |
| Lead with raw finding counts. | Users need a repair route and proof command first; counts are supporting evidence. |

## Risks

- The campaign could become too broad. Mitigation: each GitHub issue owns one
  surface or state family, and each implementation PR must preserve the
  spec-test-code-output chain.
- Start-here language could accidentally imply gate or merge authority.
  Mitigation: non-claims stay visible unless a separate gate artifact owns the
  decision.
- Preview-language criteria could be read as promotion. Mitigation: the issue
  defines proof criteria only; promotion remains a later policy-owned packet.
- Surface convergence could tempt prose parsing. Mitigation: ADR-0015 requires
  typed fields and canonical gap records for action safety.

## Non-goals

- No analyzer behavior changes in the docs/issue stack.
- No output schema changes without a follow-up spec-test-code PR.
- No PR comment publishing changes.
- No generated CI blocking or gate/default behavior changes.
- No policy promotion for preview languages.
- No source edits, generated tests, provider/model calls, or mutation
  execution.
- No editor furniture such as CodeLens, inlays, semantic tokens, inline
  patches, or unsaved-buffer overlays.

## Exit criteria

This proposal can move to `accepted` when the GitHub issue burn-down is closed,
the converged start-here path is documented, dogfood receipts prove the path,
and closeout records which surfaces now lead with the same canonical repair
unit and which future work remains.
