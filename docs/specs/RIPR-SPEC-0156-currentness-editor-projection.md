# RIPR-SPEC-0156: Currentness editor projection and qualification corpus

Status: proposed

Issue: #3282 (parent #3212; builds on #3280/#3281)

## Problem

The C1/C2 slices gave every finding a producer-owned currentness
disposition and gated the counted, gated, annotated, and diagnostic
surfaces on it — but two editor projections still presented base-deleted
evidence at its recorded coordinate without a revision marker, and the
adversarial currentness matrix existed only as in-crate unit tests, with
no end-to-end corpus a downstream consumer could run against.

## Behavior

- LSP code lenses gate on
  `Finding::is_candidate_actionable` (RIPR-SPEC-0152): a base-deleted
  finding's recorded line is the projected new-side coordinate (the
  #3212 incident shape), and pinning an advisory lens there presents
  deleted-side evidence at an impossible candidate position. A
  candidate-current finding keeps its lens. The full-profile diagnostic
  remains historical-context by design and keeps its revision label.
- The currentness matrix corpus pins, end to-end:
  - **deleted function tail** — removed-only probes are `base_deleted`,
    retained as labelled base-side evidence, never a head-actionable
    gap;
  - **reused coordinate with a different expression** — replacement
    pairing seeds only the added expression's `candidate_current`
    probe; identity is expression-addressed, so a deleted expression
    never merges into a same-line different-expression finding;
  - **whole-helper move without same-text evidence** — no resolved
    move is claimed; the records stay `base_deleted` base-side
    evidence;
  - **whole-file delete (mixed cell)** — the deleted file contributes
    no probe, finding, or repair target (disclose-not-probe, disclosed
    as a named non-claim) while the changed production owner in the
    same diff stays `candidate_current` and actionable.

## Required Evidence

- The lens reproduction (base-deleted finding received a lens at its
  projected coordinate on main) and its discriminating fix; the lens
  view identity now mirrors the visible set, so non-actionable-only
  changes do not trigger a spurious refresh.
- The four corpus fixtures with real golden outputs (the
  downstream-consumable export pattern).
- Existing C1/C2 recurrence tests remain green.

## Required guards

- A lens never anchors a non-candidate-actionable finding.
- Replacement pairing never merges identities across different
  expressions.
- Movement resolution requires same-text evidence; absence stays
  `base_deleted`.

## Acceptance Examples

- Accept: a base-deleted probe — no lens, no diagnostic, no
  annotation, no gap; labelled evidence retained in check JSON and
  human-full.
- Accept: a replacement at one line — one `candidate_current` probe
  with the added expression's identity.
- Reject: a resolved-move claim without same-text evidence.

## Test Mapping

`lsp/lens.rs` `currentness_lens_tests`; fixtures
`currentness_matrix_{tail,reuse,move,whole_delete}`.

## Non-Goals

- No re-coordination of recorded locations (deferred with #3280).
- No rename-map retention in the diff parser.
- No downstream release action; the exported fixtures are the
  producer-side evidence for the post-release removal.

## Implementation Mapping

- `lsp/lens.rs` — the eligibility filter.
- `fixtures/currentness_matrix_*` — the corpus.

## Metrics

No new metric; lens counts now reconcile with the diagnostic scope by
construction.
