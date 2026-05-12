# RIPR-PROP-0002: Lane 1 Evidence Quality Leadership

Status: proposed

Owner: ripr maintainers

Created: 2026-05-12

Target campaign: Lane 1 Evidence Quality Leadership

Linked specs:

- `RIPR-SPEC-0034`: Evidence quality scorecard (planned)
- `RIPR-SPEC-0035`: Evidence quality benchmark corpus (planned)
- `RIPR-SPEC-0036`: Static/runtime confidence expansion (planned)

Linked ADRs:

- Fixture-first evidence confidence ADR if the maturity rule needs a durable
  architecture record.

Linked work items:

- To be recorded in the Lane 1 Evidence Quality Leadership tracker.

## Problem

Lane 1 has moved past evidence-spine stabilization and the first evidence
accuracy evaluation. `seams[].evidence_record`, canonical gap identity,
evidence movement, baseline and gate identity, assistant proof, RIPR Zero
routes, related-test ranking, oracle semantics, local flow, activation
modeling, runtime-confidence labels, the repo-local audit, audit-derived
fixtures, the first duplicate-gap analyzer fix, runtime-fixtures-v2, and
evidence-health audit fields are all wired or stable in documented scope.

That plumbing is necessary, but it is not enough for leading usefulness. Users
do not only need RIPR to emit reusable evidence. They need RIPR to explain why
that evidence should be trusted, which evidence class it belongs to, what
fixture or calibration supports it, what remains unknown, which analyzer
limitation is blocking stronger confidence, and which repair would most improve
the product.

Without an explicit leadership loop, Lane 1 can drift into raw count chasing or
broad heuristic rewrites. Stable evidence records would then make weak evidence
more portable without making it more accurate. This proposal keeps Lane 1
audit-driven: measure evidence quality, fixture-pin high-value failures, make
targeted analyzer or calibration improvements, compare before and after deltas,
and promote capability only inside the documented proof scope.

## Users and surfaces

- maintainers deciding which Lane 1 repair should happen next
- reviewers judging whether a report is actionable or noisy
- developers reading CLI evidence, report artifacts, or follow-up guidance
- coding agents consuming seam packets, first-useful-action context, and repair
  routes
- downstream lanes that project Lane 1 truth into PR panels, editor hovers,
  gates, baselines, ledgers, and handoffs

The proposal primarily touches repo-local reports, fixtures, specs, capability
evidence, and analyzer/calibration internals. Downstream surfaces should
consume the resulting evidence truth; they should not invent parallel quality
claims.

## Success criteria

- An evidence quality scorecard exists and summarizes maturity, risk, and next
  repair slices from `evidence_record` data.
- A benchmark corpus pins positive cases, negative guards, movement cases,
  equivalent-code cases, known static limitations, and calibration cases.
- Top evidence risks are categorized by evidence class instead of inferred from
  raw gap counts alone.
- Audit-driven analyzer or calibration fixes show before and after deltas.
- Capability promotions are class-scoped and identify the fixture or runtime
  evidence that supports the claim.
- Calibration expands only with checked imported runtime outcomes and never
  changes static vocabulary into mutation-test vocabulary.
- Unknowns and static limitations stay visible instead of being converted into
  stronger confidence by default.

## Proposed shape

Use the existing Lane 1 source-of-truth stack:

```text
audit -> fixture -> targeted analyzer/calibration fix -> audit delta -> capability update
```

The next Lane 1 loop should produce a scorecard that can say:

- which evidence classes are strong in documented scope;
- which classes are shallow, duplicated, underconfident, overconfident, or
  uncalibrated;
- what fixture or calibration class supports each maturity claim;
- which static limitation or analyzer gap blocks stronger confidence;
- which Lane 1 repair is expected to improve product usefulness next.

Behavior contracts belong in specs. The lane tracker should keep the
implementation sequence and non-goals. Capability rows should make maturity
claims only for fixture-backed or calibrated classes. Closeouts should record
what changed, what proof ran, and what remains unknown.

## Alternatives considered

| Alternative | Why we are not picking it |
| --- | --- |
| Keep extending projection surfaces now that `evidence_record` is stable. | Downstream surfaces already have a shared spine. The next risk is evidence quality, not another rendering path. |
| Rewrite broad analyzer heuristics from intuition. | Lane 1 should use audit data and fixtures first so improvements are targeted and reviewable. |
| Treat raw gap-count reduction as the primary success signal. | A lower count can hide overgrouping, underreporting, or weaker explanations. The scorecard must preserve class-specific quality and unknowns. |
| Promote analyzer confidence globally. | RIPR evidence maturity is class-scoped. Stable or calibrated claims require documented fixture or runtime evidence for that class. |
| Change gates or PR/CI policy based on the scorecard. | The scorecard is an evidence-quality operating loop, not a policy decision or default-blocking surface. |
| Add generated tests or source edits as the improvement path. | RIPR should identify missing discriminators and evidence limits; it should not generate or apply test/source changes in this lane. |

## Behavior specs to create or update

- `RIPR-SPEC-0034`: Evidence quality scorecard.
- `RIPR-SPEC-0035`: Evidence quality benchmark corpus.
- `RIPR-SPEC-0036`: Static/runtime confidence expansion.
- Existing Lane 1 audit, fixture, match-arm, capability, and traceability docs
  may be referenced by those specs but should not absorb the proposal's
  motivation.

## Architecture decisions needed

An ADR is useful if maintainers want the fixture-first maturity rule to be
durable architecture policy:

```text
No evidence class moves to stable without fixture-backed documented scope.
No evidence class moves to calibrated without checked imported runtime outcomes.
Analyzer confidence upgrades are preceded by audit findings and fixtures.
```

No ADR is needed for the scorecard report itself unless it changes the evidence
model or maturity policy.

## Implementation campaign shape

1. `docs/proposal-lane-1-evidence-quality-leadership`
2. `docs/spec-evidence-quality-scorecard`
3. `docs/spec-evidence-quality-benchmark-corpus`
4. `docs/adr-fixture-first-evidence-confidence`
5. `docs/lane-1-evidence-quality-leadership-tracker`
6. `report/evidence-quality-scorecard`
7. `fixtures/evidence-quality-benchmark-corpus`
8. `analysis/related-test-ranking-audit-fixes`
9. `analysis/oracle-semantics-audit-fixes`
10. `analysis/static-limitation-taxonomy`
11. `calibration/runtime-fixtures-v3`
12. `report/evidence-quality-trend`
13. `campaign/evidence-quality-leadership-closeout`

Each slice follows the [scoped PR contract](../SCOPED_PR_CONTRACT.md). One
semantic docs artifact should land per docs PR, with index updates allowed.

## Evidence plan

- Scorecard artifacts summarize evidence maturity by class, canonical groups,
  duplicate-looking groups, static limitation categories, missing discriminator
  classes, related-test confidence, oracle semantics, movement availability,
  calibration coverage, recommended repair slices, and recent audit deltas.
- Benchmark fixtures include positive cases, negative cases, line-movement
  cases, equivalent-code cases, must-not-claim guards, calibration cases, known
  static limitations, and before/after audit expectations.
- Audit deltas show whether targeted analyzer work improved the class it
  claimed to improve without hiding unknowns.
- Runtime calibration fixtures identify imported outcomes, ambiguous joins,
  runtime-only signals, and class-scoped confidence labels.
- Capability matrix and `metrics/capabilities.toml` updates name the exact
  fixture or calibration class behind any promotion.
- Traceability links the scorecard, benchmark, calibration, tests, code, output
  contracts, metrics, and closeout artifacts.
- Handoffs record the landed proof, residual risks, and future work by evidence
  class.

## Risks

- Raw count chasing could make the evidence look better while reducing
  precision. Mitigation: require class-specific scorecard fields and
  must-not-claim guards.
- Heuristic expansion could overfit dogfood cases. Mitigation: add negative
  fixtures, line-movement cases, equivalent-code cases, and known limitation
  cases before analyzer changes.
- Calibration language could imply mutation-test proof. Mitigation: keep RIPR's
  static vocabulary and describe imported runtime outcomes as calibration
  evidence, not killed or survived claims.
- Downstream lanes could reinterpret scorecard findings as policy. Mitigation:
  keep gates, PR/CI projection, and default blocking out of this proposal.
- Proposal, spec, tracker, and capability docs could duplicate each other.
  Mitigation: keep motivation in this proposal, behavior in specs, sequence in
  the lane tracker, maturity in capabilities, and proof in traceability and
  closeouts.

## Non-goals

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

## Exit criteria

This proposal can move to `accepted` when:

- the evidence quality scorecard spec and implementation are merged;
- the benchmark corpus spec and fixtures are merged;
- at least two audit-driven analyzer or calibration improvements land with
  before and after evidence;
- runtime-fixtures-v3 or equivalent checked calibration expansion lands without
  gate or static-vocabulary changes;
- capability updates are class-scoped and proof-backed;
- traceability links the proposal, specs, fixtures, tests, code, metrics, and
  closeout;
- a Lane 1 evidence quality leadership closeout records what landed, what
  improved, and what remains unknown.

If the scorecard shows that another evidence class has higher product value
than the planned ranking, oracle, limitation, or calibration work, the lane
tracker should change the implementation order and keep this proposal focused
on the audit-fixture-delta-capability loop.
