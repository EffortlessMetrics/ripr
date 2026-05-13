# Lane 1: Evidence Quality Leadership

Status: open

Opened: 2026-05-13

## Goal

Make RIPR's shared `evidence_record` a measured evidence-quality operating
system. Lane 1 should be able to say:

```text
This is the behavioral evidence.
This is why RIPR believes it.
This is the evidence class.
This is the fixture or calibration proof.
This is what remains unknown.
This is the analyzer limitation category.
This is the next repair most likely to improve evidence quality.
This is the measured before/after delta from the last improvement.
```

The work moves Lane 1 from stable evidence plumbing to evidence quality
leadership: scorecard, benchmark corpus, targeted analyzer or calibration
repair, measured delta, and class-scoped capability proof.

## Boundary

Lane 1 owns evidence truth, identity, movement, calibration confidence, and
analyzer accuracy. Downstream surfaces consume Lane 1 truth; they do not invent
parallel confidence or maturity claims.

This lane may change:

- analyzer truth for fixture-backed evidence classes;
- `evidence_record` quality fields and report summaries;
- canonical gap identity and movement evidence;
- related-test ranking evidence;
- oracle semantics evidence;
- static limitation taxonomy;
- imported static/runtime calibration confidence.

This tracker does not make `.ripr/goals/active.toml` the whole product board.
Do not update `.ripr/goals/active.toml` unless the repo-wide operator sequence
explicitly makes Lane 1 active.

## Source-Of-Truth Stack

Use one document for one job:

- proposal: [RIPR-PROP-0002: Lane 1 Evidence Quality Leadership](../proposals/RIPR-PROP-0002-lane-1-evidence-quality-leadership.md)
  explains why the leadership loop exists;
- spec: [RIPR-SPEC-0034: Evidence Quality Scorecard](../specs/RIPR-SPEC-0034-evidence-quality-scorecard.md)
  defines scorecard behavior;
- spec: [RIPR-SPEC-0035: Evidence Quality Benchmark Corpus](../specs/RIPR-SPEC-0035-evidence-quality-benchmark-corpus.md)
  defines benchmark fixture behavior;
- spec: [RIPR-SPEC-0039: Static/Runtime Confidence Expansion](../specs/RIPR-SPEC-0039-static-runtime-confidence-expansion.md)
  defines runtime-fixtures-v3 confidence labels and runtime/static boundaries;
- ADR: [ADR 0010: Fixture-First Evidence Confidence](../adr/0010-fixture-first-evidence-confidence.md)
  records the maturity rule;
- lane tracker: this document records the PR-sized sequence, current state, and
  non-goals;
- capability matrix: `docs/CAPABILITY_MATRIX.md` and
  `metrics/capabilities.toml` record class-scoped maturity and proof;
- traceability: `.ripr/traceability.toml` links specs, tests, fixtures, code,
  outputs, and metrics;
- closeout: `docs/handoffs/` records what landed, proof, and remaining unknowns.

## Current State

- Evidence Spine Stabilization is complete within documented v0.1 scope.
- Evidence Accuracy Evaluation is closed in documented scope.
- The repo-local Lane 1 evidence-quality audit exists.
- Evidence-health includes durable audit fields.
- The match-arm canonical overgrouping fix landed from audit-driven analyzer
  work.
- Runtime-fixtures-v2 expanded checked calibration classes for side-effect,
  mock, snapshot, and opaque or dynamic dispatch cases.
- The Lane 1 source-of-truth model is documented in [docs/lanes/README.md](README.md).
- RIPR-PROP-0002 explains the Evidence Quality Leadership loop.
- RIPR-SPEC-0034 defines the scorecard behavior contract.
- RIPR-SPEC-0035 defines the benchmark corpus behavior contract.
- ADR 0010 requires fixture-first, class-scoped evidence confidence.
- The first oracle-semantics audit fix landed in #871, keeping clear custom
  assertion helpers strong while leaving opaque helpers unknown and duplicative
  equality assertions weak.

## Planned Slices

| Slice | Intent | Status |
| --- | --- | --- |
| `report/evidence-quality-scorecard` | Generate `target/ripr/reports/evidence-quality-scorecard.{json,md}` or an equivalent extension of the Lane 1 audit with maturity, risk, recommended repairs, and recent deltas. | merged in #850 |
| `fixtures/evidence-quality-benchmark-corpus` | Add the benchmark corpus defined by RIPR-SPEC-0035 with positive cases, negative guards, movement cases, equivalent-code cases, known limitations, and calibration cases. | merged in #851 |
| `analysis/related-test-ranking-audit-fixes` | Fix audit-derived related-test ranking misses only after benchmark cases prove the class. | deferred until the audit shows a ranking miss; current scorecard reports `0` low or opaque top related tests |
| `analysis/oracle-semantics-audit-fixes` | Fix audit-derived oracle-shape misses while keeping unsupported helpers as static limitations. | merged in #871 |
| `analysis/static-limitation-taxonomy` | Normalize limitations into repairable categories and make them visible in scorecard/evidence-health without treating them as user test gaps. | merged in #861 |
| `calibration/runtime-fixtures-v3` | Expand checked runtime fixture classes without creating static gaps from runtime-only signal or running mutation execution in CI. | next substantive planned slice |
| `report/evidence-quality-trend` | Compare current and previous audit or scorecard snapshots to show whether evidence quality is improving. | planned |
| `campaign/evidence-quality-leadership-closeout` | Close after scorecard, benchmark corpus, at least two audit-driven improvements, one calibration expansion, conservative capabilities, and a closeout handoff. | planned |

## Validation Gates

Docs and planning slices should run the narrowest relevant doc gates plus
`cargo xtask check-pr` before merge.

Implementation slices should run the gates named by their specs, plus the
normal review-ready gate:

```bash
cargo xtask check-pr
```

Scorecard implementation should also run:

```bash
cargo xtask evidence-quality-scorecard
cargo xtask check-output-contracts
cargo xtask check-traceability
cargo xtask check-capabilities
```

Benchmark implementation should also run:

```bash
cargo xtask check-fixture-contracts
cargo xtask check-traceability
cargo xtask check-capabilities
```

Analyzer or calibration slices should include focused tests, fixture or runtime
proof, and an audit or scorecard delta.

## Non-Goals

- No PR or CI front-panel work.
- No inline PR comment publishing.
- No LSP or editor polish.
- No gate-policy changes or default blocking.
- No score redefinition.
- No generated tests.
- No automatic source edits.
- No provider or model calls.
- No mutation execution in CI.
- No release, packaging, platform, dependency, or MSRV cleanup.
- No broad "analyzer stable" claim.

## Operating Rule

Follow ADR 0010:

- no evidence class moves to `stable` without fixture-backed documented scope;
- no evidence class moves to `calibrated` without checked imported runtime
  outcomes for that class;
- analyzer confidence upgrades are preceded by audit findings and fixtures;
- unknowns and static limitations remain visible until proof exists.

The implementation order may change if the scorecard shows a different
evidence class has higher product value. Future work should be listed by
evidence class, not by projection surface.

## Open PR Table

| Slice | PR | Status | Notes |
| --- | --- | --- | --- |
| `docs/lane-1-source-of-truth-model` | #843 | merged | Added lane tracker source-of-truth model. |
| `docs/proposal-lane-1-evidence-quality-leadership` | #844 | merged | Added RIPR-PROP-0002. |
| `docs/spec-evidence-quality-scorecard` | #845 | merged | Added RIPR-SPEC-0034. |
| `docs/spec-evidence-quality-benchmark-corpus` | #846 | merged | Added RIPR-SPEC-0035. |
| `docs/adr-fixture-first-evidence-confidence` | #847 | merged | Added ADR 0010. |
| `docs/lane-1-evidence-quality-leadership-tracker` | #848 | merged | Opened this tracker; no behavior changes. |
| `report/evidence-quality-scorecard` | #850 | merged | Added the repo-local scorecard from RIPR-SPEC-0034; no analyzer, gate, PR/CI, LSP, provider, generated-test, or mutation-execution behavior. |
| `fixtures/evidence-quality-benchmark-corpus` | #851 | merged | Added the RIPR-SPEC-0035 manifest-only corpus and validator coverage; no analyzer, gate, PR/CI, LSP, provider, generated-test, or mutation-execution behavior. |
| `analysis/static-limitation-taxonomy` | #861 | merged | Added normalized static-limitation categories and repair routes to evidence records, evidence-health, audit, and scorecard surfaces without changing grip classes, gates, mutation execution, or downstream projection policy. |
| `analysis/oracle-semantics-audit-fixes` | #871 | merged | Tightened custom assertion helper and duplicative equality oracle semantics from audit/benchmark cases without changing gates, PR/CI projection, LSP/editor behavior, generated tests, provider calls, mutation execution, or score definitions. |

## Closeout Conditions

Close this lane only when:

- the scorecard exists and reports maturity, risk, recommended repairs, and
  recent deltas;
- the benchmark corpus exists with required positive, negative, movement,
  equivalent-code, limitation, and calibration cases;
- at least two audit-driven analyzer or calibration improvements land with
  before and after evidence;
- one runtime calibration expansion lands with checked imported outcomes;
- capability updates are class-scoped and proof-backed;
- traceability links specs, fixtures, tests, code, outputs, metrics, and
  closeout artifacts;
- a handoff records what improved, what remains unknown, and which evidence
  class should be repaired next.
