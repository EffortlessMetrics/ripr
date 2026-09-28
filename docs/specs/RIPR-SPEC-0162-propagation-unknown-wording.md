# RIPR-SPEC-0162: Honest propagation_unknown human wording

Status: proposed

Issue: deferred follow-up from PR #3317's review (binding_value_family_matrix human.txt findings); no dedicated issue.

## Problem

The shared human template contradicted the `propagation_unknown`
classification in two places (`output/human/sections.rs`):

1. The why-hint rendered "the change propagates but the downstream
   effect is unknown statically" — asserting as fact exactly what the
   class says could not be determined (`PropagationUnknown` fires when
   the propagate stage is `Unknown` or `Opaque`; the evidence path says
   "Propagation is not statically obvious").
2. The Weakness heading labeled the class-limitation prose ("No clear
   propagation path from changed behavior to an observable sink") as a
   **Missing discriminator** — a propagation limitation is not a
   discriminator a test could supply; the heading conflated the two
   limitation families.

## Behavior

- The why-hint for `PropagationUnknown` now states the honest boundary:
  "the path from the changed behavior to an observable sink is not
  statically clear."
- The Weakness heading label is content-derived: when the finding's
  class is `PropagationUnknown` or `StaticUnknown` **and** no real
  discriminator is missing (`activation.missing_discriminators` empty),
  the entry renders under `Analyzer limit`; any finding with a real
  missing discriminator keeps the `Missing discriminator` label, as does
  the advisory `exposed` form (`Discriminator (observed, advisory)`).
- No JSON, SARIF, gate, or decision-layer text changes: the fix is
  renderer-owned (the `missing` entry in `analysis/classify/decision.rs`
  is shared and unchanged).

## Required Evidence

- The blast radius measured before the change: 32 fixtures drift, all
  `human.txt`-only, all `formatting_only` (the why-hint line and, where
  the label applies, the Weakness heading) — zero `check.json` drift,
  confirming the change is renderer-only.
- Renderer unit tests pinning: the new hint; the `Analyzer limit` label
  for unknown-class limitation prose; the retained `Missing
  discriminator` label when `missing_discriminators` is non-empty.

## Required guards

- The why-hint never asserts a stage the class marks unknown.
- The `Missing discriminator` label is reserved for findings where a
  discriminator is actually missing.

## Acceptance Examples

- Accept: a `propagation_unknown` finding renders "Why
  propagation_unknown: the path from the changed behavior to an
  observable sink is not statically clear" and the limitation prose
  under `Analyzer limit`.
- Accept: a `propagation_unknown` finding with a real missing
  discriminator keeps the `Missing discriminator` label.
- Reject: any hint asserting "the change propagates" for a class whose
  propagate stage is unknown; any limitation-prose entry labeled
  `Missing discriminator` when none is missing.

## Metrics

No new metric; wording drift is pinned by the 32 re-blessed goldens
and the renderer unit test.

## Test Mapping

`output/human.rs` tests `propagation_unknown_wording_is_honest`;
the 32 re-blessed goldens.

## Non-Goals

- No decision-layer, JSON, or SARIF wording changes; no schema change.

## Implementation Mapping

- `output/human/sections.rs` — `classification_hint` and the header
  label switch.
