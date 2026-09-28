# ADR 0010: Fixture-First Evidence Confidence

Status: accepted

Date: 2026-05-13

## Context

Lane 1 now has a stable shared evidence spine and a closed evidence accuracy
evaluation pass. The next phase asks RIPR to explain evidence quality by class:
what is strong, what is shallow, what is duplicated, what is uncalibrated, and
which repair is most likely to improve the product.

That creates a maturity risk. Once evidence quality is summarized in audits,
scorecards, health reports, capability rows, and agent packets, future work can
accidentally turn a local fixture win into a global analyzer claim. RIPR's
static vocabulary is intentionally conservative, and runtime calibration is
imported proof about checked classes rather than a license to claim mutation
outcomes.

The repo already has ADR 0003 for fixtures before analyzer rewrites. Lane 1
needs the narrower maturity rule for evidence confidence: fixture and runtime
proof must scope the claim, not merely support the implementation.

## Decision

Lane 1 evidence confidence is class-scoped and fixture-first.

Rules:

- No evidence class moves to `stable` without fixture-backed documented scope.
- No evidence class moves to `calibrated` without checked imported runtime
  outcomes for that class.
- Analyzer confidence upgrades must be preceded by audit findings and fixtures
  that include positive cases and must-not-claim guards.
- Runtime-only signal does not create a static gap.
- Ambiguous runtime joins stay ambiguous until a checked fixture proves a
  supported join.
- Static reports keep RIPR vocabulary: `exposed`, `weakly_exposed`,
  `reachable_unrevealed`, `no_static_path`, `infection_unknown`,
  `propagation_unknown`, and `static_unknown`.
- Capability rows, scorecards, health reports, and closeouts must name the
  fixture or runtime evidence behind any maturity claim.

This ADR does not require every docs-only planning PR to add fixtures. It
requires analyzer, calibration, and capability-confidence upgrades to be
fixture-backed before the maturity claim is made.

## Consequences

Positive:

- prevents broad "analyzer stable" claims
- keeps capability promotions honest and class-scoped
- makes scorecard maturity rows reviewable
- keeps unknowns and static limitations visible
- gives agents a durable rule for rejecting uncalibrated confidence upgrades
- preserves RIPR's static language boundary while still allowing runtime
  calibration evidence

Negative:

- some useful heuristics wait for fixture work before promotion
- capability updates need more precise proof language
- reports must carry more unknown and limitation detail instead of collapsing
  everything into a single confidence label

## Alternatives Considered

- Promote analyzer confidence globally after a successful audit-driven fix.
  Rejected: one fix can improve a class without proving unrelated evidence
  classes.
- Use raw count reduction as the promotion bar. Rejected: counts can improve
  through underreporting, overgrouping, or lost detail.
- Let runtime calibration upgrade static vocabulary broadly. Rejected: imported
  runtime outcomes calibrate checked classes; they do not turn static evidence
  into mutation-test results.
- Treat this as a lane tracker convention only. Rejected: the fixture-first
  confidence rule affects capability maturity, scorecards, calibration, and
  closeouts, so it needs a durable architecture decision.
