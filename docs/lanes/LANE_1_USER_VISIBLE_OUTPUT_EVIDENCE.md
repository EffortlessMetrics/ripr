# Lane 1: User-Visible Output Evidence

Status: open

Opened: 2026-05-13

## Goal

Make changed presentation, help, report, table, and label text evidence
actionable without overclaiming. Lane 1 should be able to say:

```text
This changed text is user-visible, internal-only, or visibility-unknown.
This observer shape exists or is missing.
This declaration and literal belong to one canonical evidence item.
This is the action, no-action state, or static limitation.
This is the fixture or scorecard proof behind that claim.
```

The target screenshot class is a changed string constant such as:

```rust
pub const APPLE_M3_AIR_DEVICE_LABELS_TEXT: &str =
    "apple-m3-air-cpu-neon = M3 MacBook Air Apple CPU/NEON lane";
```

RIPR should not turn that into two unrelated raw actions. It should expose one
presentation-text evidence item with visibility, observer, actionability,
canonical grouping, and limitation context.

## Boundary

Lane 1 owns the evidence truth:

- evidence class and additive `evidence_record` fields;
- visibility and observer evidence;
- static limitation categories and repair routes;
- canonical grouping of raw text seams into one evidence item;
- actionability labels for downstream consumers;
- benchmark, scorecard, trend, capability, and traceability proof.

Downstream PR/CI and editor lanes own rendering. This tracker does not change
annotations, hovers, gates, generated workflows, default blocking, generated
tests, source edits, provider calls, or mutation execution.

Do not update `.ripr/goals/active.toml` unless the repo-wide operator sequence
explicitly makes this Lane 1 tracker active.

## Source-Of-Truth Stack

- proposal: [RIPR-PROP-0005: User-Visible Output Evidence](../proposals/RIPR-PROP-0005-user-visible-output-evidence.md)
  explains why this evidence class exists;
- spec: [RIPR-SPEC-0043: Presentation Text Evidence](../specs/RIPR-SPEC-0043-presentation-text-evidence.md)
  defines behavior;
- alignment spec: [RIPR-SPEC-0045: Finding-To-Gap Alignment](../specs/RIPR-SPEC-0045-finding-to-gap-alignment.md)
  defines how raw findings become canonical evidence items;
- ADR: [ADR 0010: Fixture-First Evidence Confidence](../adr/0010-fixture-first-evidence-confidence.md)
  remains the maturity rule;
- lane tracker: this file records sequencing, state, validation, and non-goals;
- capability matrix: `docs/CAPABILITY_MATRIX.md` and
  `metrics/capabilities.toml` record class-scoped maturity when behavior lands;
- traceability: `.ripr/traceability.toml` links specs, fixtures, tests, code,
  outputs, and metrics as implementation slices land;
- closeout: `docs/handoffs/` will record what improved, what remains unknown,
  and which presentation-text repair should happen next.

## Current State

- Evidence Spine Stabilization is complete.
- Evidence Accuracy Evaluation is closed.
- Evidence Quality Leadership is closed in documented scope.
- The scorecard, benchmark corpus, static limitation taxonomy,
  oracle-semantics audit fix, runtime-fixtures-v3, evidence-quality trend, and
  class-scoped closeout proof are in place.
- Presentation text is not yet a first-class evidence class in implemented
  analyzer behavior.
- PR #900 started a screenshot-derived benchmark case for the changed
  `APPLE_M3_AIR_DEVICE_LABELS_TEXT` constant. Treat it as the benchmark slice
  after the proposal and spec foundation land.
- The next contract step is finding-to-gap alignment so raw line-local findings
  roll up into one canonical item with state, actionability, reason, repair,
  proof, and supporting raw evidence.

## Planned Slices

| Slice | Intent | Status |
| --- | --- | --- |
| `docs/proposal-user-visible-output-evidence` | Open the proposal and lane tracker for presentation/help/report/table text evidence. | merged in #904 |
| `docs/spec-presentation-text-evidence` | Define visibility, observer, actionability, canonical grouping, static limitation, and must-not-claim behavior. | merged in #909 |
| `docs/spec-finding-to-gap-alignment` | Define how raw findings roll up into canonical evidence items with state, actionability, repair, confidence, and supporting raw evidence. | merged in #927 |
| `fixtures/finding-alignment-benchmark` | Add raw-to-canonical benchmark cases for grouping, no-action, already-observed, static limitation, and actionable states. | current |
| `fixtures/presentation-text-evidence-benchmark` | Add benchmark cases for user-visible observed/unobserved text, internal-only labels, visibility unknowns, declaration/literal grouping, and unrelated strings. | merged in #900 |
| `analysis/presentation-text-evidence-fields` | Add additive evidence-record fields for presentation text visibility, observer, actionability, source kind, and grouping. | planned |
| `analysis/presentation-text-visibility` | Conservatively classify obvious output sinks and keep opaque or indirect routes as limitations. | planned |
| `analysis/presentation-text-canonical-grouping` | Group a constant declaration and its string literal into one canonical evidence item without colliding different constants. | planned |
| `analysis/presentation-text-actionability` | Classify snapshot/help-output/report-test action, already-observed output, internal no-action, and limitation states. | planned |
| `report/presentation-text-scorecard-trend-fields` | Add scorecard and trend counts for presentation-text evidence quality. | planned |
| `docs/presentation-text-consumer-handoff` | Hand downstream lanes the evidence contract without changing rendering in this lane. | planned |

## Required States

| State | Meaning | User action |
| --- | --- | --- |
| `presentation_text_user_visible_unobserved` | Text flows to observable user output, but no observer is found. | Add or update a help-output, snapshot, report, table, or golden-output test. |
| `presentation_text_user_visible_observed` | Text flows to output and an observer checks it. | No new RIPR action. |
| `presentation_text_internal_only` | Text appears internal, proof-lane, or config-only with no user-facing sink. | No action. |
| `presentation_text_visibility_unknown` | RIPR cannot trace whether the text is user-visible. | Static limitation; inspect output flow. |
| `presentation_text_observer_unknown` | Visibility is likely, but observer evidence is not available. | Static limitation or low-confidence action, depending on spec scope. |
| `presentation_text_duplicate_lines_grouped` | Declaration and literal raw seams describe one text change. | One canonical action, not duplicate raw notices. |

## Validation Gates

Docs and planning slices should run:

```bash
cargo xtask check-doc-index
cargo xtask markdown-links
cargo xtask check-static-language
cargo xtask check-traceability
cargo xtask check-capabilities
cargo xtask check-pr
git diff --check
```

Spec slices should also run:

```bash
cargo xtask check-spec-format
```

Benchmark slices should also run:

```bash
cargo test -p xtask evidence_quality_benchmark
cargo xtask check-fixture-contracts
cargo xtask check-output-contracts
```

Analyzer and report slices should run focused tests named by
`RIPR-SPEC-0043` plus:

```bash
cargo xtask evidence-quality-scorecard
cargo xtask evidence-quality-trend
cargo xtask check-output-contracts
cargo xtask check-traceability
cargo xtask check-capabilities
cargo xtask check-pr
git diff --check
```

## Must-Not-Claim Guards

- Do not create static gaps from text alone.
- Do not recommend mutation testing as the first action for presentation text.
- Do not treat internal labels as user test debt.
- Do not produce duplicate actions for a declaration and literal that represent
  one text constant.
- Do not overclaim user visibility through opaque helpers, macros, dynamic
  dispatch, or unsupported cross-file formatting.
- Do not redefine RIPR scores.

## Non-Goals

- No PR or CI rendering changes in Lane 1.
- No inline PR comment publishing.
- No LSP or editor polish.
- No gate-policy changes or default blocking.
- No generated tests.
- No automatic source edits.
- No provider or model calls.
- No mutation execution.
- No release, packaging, platform, dependency, or MSRV cleanup.
- No broad analyzer stability claim.

## Open PR Table

| Slice | PR | Status | Notes |
| --- | --- | --- | --- |
| `docs/proposal-user-visible-output-evidence` | #904 | merged | Added RIPR-PROP-0005 and this tracker; no behavior changes. |
| `docs/spec-presentation-text-evidence` | #909 | merged | Added RIPR-SPEC-0043; no analyzer, projection, gate, generated-test, provider, mutation-execution, or score-definition changes. |
| `docs/spec-finding-to-gap-alignment` | #927 | merged | Defines raw finding to canonical evidence item alignment before behavior changes. |
| `fixtures/finding-alignment-benchmark` | pending | current | Adds manifest-only raw-to-canonical presentation-text alignment cases; no analyzer behavior changes. |
| `fixtures/presentation-text-evidence-benchmark` | #900 | merged | Added the screenshot-derived constant benchmark after the proposal/spec foundation. |

## Closeout Conditions

This lane can close after:

- `RIPR-SPEC-0043` is merged;
- `RIPR-SPEC-0045` is merged;
- raw findings can align to canonical evidence items with explicit state,
  actionability, reason, repair, confidence, and supporting raw evidence;
- benchmark cases exist for observed, unobserved, internal-only,
  visibility-unknown, observer-unknown, declaration/literal grouping, and
  unrelated-string guard cases;
- additive evidence fields are documented or implemented;
- fixture-backed analyzer behavior classifies the required states without
  turning text alone into user test debt;
- scorecard or trend output reports presentation-text evidence quality;
- downstream consumer handoff documents the contract without changing
  projection behavior;
- capability and traceability updates are class-scoped and proof-backed;
- a closeout handoff records proof, remaining unknowns, and the next repair.
