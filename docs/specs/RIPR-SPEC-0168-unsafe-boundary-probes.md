# RIPR-SPEC-0168: Unsafe execution boundary probes

Status: proposed

Owner:

Created: 2026-08-29

Linked proposal:

Linked ADRs:

Linked plan:

Linked issues: #3536

Linked PRs:

Support-tier impact: none

Policy impact: none

## Problem

Candidate changes inside Rust unsafe execution boundaries (`unsafe fn` bodies
and explicit `unsafe {}` blocks) were analyzed exactly like ordinary safe Rust:
the parser discarded the boundary context, and any lexical workaround would
misclassify comments and string literals. A reviewer could not see from the
static evidence that a changed line sits inside an unsafe execution boundary.

## Behavior

The Rust fact index records parser-backed `unsafe_boundary` probe shapes for
`unsafe fn` bodies and explicit `unsafe {}` blocks. When a changed line lies
inside such a boundary, the diff projection emits one explicit
`static_unknown` probe at the changed line, tagged with the
`unsafe_boundary` probe shape and the `static_unknown` probe family. Ordinary
predicate, call, value, and effect probes are preserved alongside the boundary
context. A line shared between boundary and non-boundary code projects no
unsafe-boundary probe: the boundary's exact AST byte range must overlap the
changed line, and an opening or closing edge line shared with code outside the
boundary disqualifies it. Nested boundaries resolve to the innermost boundary
containing the changed line. The probe family is `static_unknown` by
construction, so the classifier cannot promote reach plus an oracle into
`exposed` for this family.

## Non-Goals

- Proving unsafe preconditions or replacing unsafe-review processes.
- Modeling file-level unsafe declarations (`unsafe trait`, `unsafe impl`,
  `unsafe extern`, `#[unsafe(...)]` attributes).
- Macro expansion or generated-case enumeration.
- Any change to exposure classification, output schema, support tiers, or
  runtime execution claims.

## Required Evidence

- A golden fixture whose diff changes a line strictly inside an `unsafe {}`
  block pins exactly one `static_unknown` unsafe-boundary probe at the changed
  line.
- Parser and classifier unit tests cover extraction, innermost-boundary
  selection, shared-edge-line fail-closed rejection, and comment/string
  false-positive controls.

## Inputs

- A Rust workspace with a diff that changes a line inside an `unsafe {}` block.

## Outputs

- One `static_unknown` probe for the innermost boundary containing each
  changed line, projected at that line with the `unsafe_boundary` probe
  shape.

## Acceptance Examples

- The `unsafe_boundary_probe` fixture: a changed loop body inside an
  `unsafe {}` block produces the boundary probe at the changed line and keeps
  the ordinary value probes for the surrounding function.

## Test Mapping

- `fixtures/unsafe_boundary_probe` (golden corpus).
- `crates/ripr/src/analysis/syntax/ra.rs` — parser extraction tests
  (`ra_adapter_extracts_unsafe_execution_boundaries_without_lexical_false_positives`
  family).
- `crates/ripr/src/analysis/probes/classify.rs` — classifier tests for span
  matching, innermost selection, and edge-line fail-closed guards.

## Implementation Mapping

- `crates/ripr/src/analysis/syntax/ra.rs` — extraction of
  `unsafe_boundary` probe-shape facts (#3516).
- `crates/ripr/src/analysis/probes/classify.rs` — changed-line projection and
  boundary ownership guard.
- `crates/ripr/src/analysis/extract/probe_shapes.rs` — the
  `unsafe_boundary` probe shape constant.

## Metrics

- Golden fixture passes `cargo xtask goldens check`; `unsafe_boundary` appears
  in the fixture corpus denominator.
