# RIPR-SPEC-0033: Match-Arm Canonical Gap Discriminators

Status: proposed

## Problem

The Lane 1 evidence-quality audit found a dogfood overgrouping pattern where
multiple match-arm seams in the same owner shared a generic `=>` or `match`
discriminator. That made distinct missing behavioral evidence look like one
canonical gap and hid which match arm needed targeted attention.

Lane 1 needs parser-backed match-arm evidence to carry enough syntax to
distinguish different arms while preserving line-movement-tolerant canonical
identity for the same arm.

## Behavior

Rust parser-backed probe shape facts for match syntax must use normalized,
discriminating text:

- match expressions use the match head, such as `match key`;
- match arms use the arm pattern plus fat arrow, such as `"kind" =>`;
- whitespace is normalized to single spaces.

The source location remains anchored to the existing match token or fat-arrow
token range. The discriminating text changes the missing-discriminator input to
canonical gap identity; it does not change source mapping, output schemas, or
public command surfaces.

Canonical gap identity must continue to group the same owner, seam kind, flow
sink, missing discriminator, and assertion shape. Different match-arm
discriminators in the same owner must not share a canonical gap identity.

## Required Evidence

The implementation must prove:

- parser-backed match probe facts include `match <scrutinee>` and `<arm> =>`
  text instead of generic `match` or `=>`;
- canonical gap identity differs for different match-arm discriminators in the
  same owner;
- canonical gap identity remains stable for the same match-arm discriminator
  across line movement;
- the Lane 1 evidence-quality fixture corpus pins the original suppressions
  dogfood case after the split;
- `cargo xtask lane1-evidence-audit` shows the generic match-arm duplicate
  group removed from the top duplicate-looking groups.

## Non-Goals

- No new analyzer family.
- No new public output schema.
- No gate, policy, PR, CI, or evidence-health behavior change.
- No related-test ranking rewrite.
- No oracle semantics expansion.
- No LSP, editor, provider, release, dependency, or platform work.
- No mutation execution.

## Acceptance Examples

Given `match amount { 0 => ..., _ => ... }`, parser-backed probe facts include
`match amount`, `0 =>`, and `_ =>`, and do not include a bare `=>` fact.

Given two headline match-arm seams in `parser::parse_format`, one for
`OutputFormat::Json =>` and one for `OutputFormat::Md =>`, their canonical gap
IDs are different.

Given the same `OutputFormat::Json =>` match-arm seam before and after line
movement, its canonical gap ID stays the same while the raw seam ID can change.

Given the suppressions dogfood seam `205829e99ffbd3ca`, the evidence-quality
fixture records the concrete `"kind" =>` discriminator and canonical group
size `1`; it must not regress to a generic `=>` group.

## Test Mapping

- `crates/ripr/src/analysis/rust_index.rs::tests::parser_adapter_extracts_probe_shapes_from_syntax`
  pins parser-backed match expression and match-arm probe text.
- `crates/ripr/src/analysis/canonical_gap.rs::tests::canonical_gap_id_distinguishes_different_match_arm_discriminators`
  pins identity separation for different match-arm discriminators.
- `crates/ripr/src/analysis/canonical_gap.rs::tests::canonical_gap_id_groups_same_match_arm_across_line_movement`
  pins line-movement tolerance for the same match-arm discriminator.
- `xtask::tests::lane1_evidence_quality_failure_corpus_is_valid` validates the
  updated suppressions fixture case.

## Implementation Mapping

- `crates/ripr/src/analysis/syntax/ra.rs` emits normalized match-head and
  match-arm probe text while preserving existing token ranges.
- `crates/ripr/src/analysis/rust_index.rs` pins parser-backed probe shape
  extraction.
- `crates/ripr/src/analysis/canonical_gap.rs` pins match-arm canonical gap
  identity behavior.
- `fixtures/boundary_gap/expected/evidence-quality-failures/corpus.json` pins
  the audit-derived suppressions dogfood case after overgrouping reduction.
- `docs/lanes/LANE_1_EVIDENCE_ACCURACY.md` records the audit impact.

## Metrics

This slice is tracked by:

- `lane1_evidence_audit_duplicate_looking_groups`;
- `lane1_evidence_audit_canonical_gap_groups`;
- `lane1_evidence_audit_static_limitations`.

## Validation

The implementation is pinned by:

- focused parser and canonical-gap unit tests;
- `cargo xtask lane1-evidence-audit`;
- `cargo xtask check-fixture-contracts`;
- `cargo xtask check-static-language`;
- `cargo xtask check-spec-format`;
- `cargo xtask check-traceability`;
- `cargo xtask check-capabilities`;
- `cargo xtask check-pr`.
