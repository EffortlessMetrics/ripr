# ADR 0005: Scoped Evidence-Heavy PRs

Status: accepted

Date: 2026-05-01

## Context

`ripr` is an evidence-first static exposure tool. Its own development process
should prefer narrow production changes with enough evidence for reviewers,
users, and future agents to understand the behavior being changed.

Line count is a weak proxy for review risk. Fixtures, golden outputs, specs,
docs, ADRs, metrics, and traceability can make a PR large while reducing risk.
A small code diff can still be risky if it changes multiple public contracts or
architectural seams without one shared acceptance criterion.

## Decision

PRs are scoped by production behavior, public contract, or architectural seam,
not by total line count.

A PR with a narrow production delta and a large evidence delta is considered
scoped when the evidence supports one shared acceptance criterion.

Every material behavior PR should identify:

- production delta
- evidence delta
- single acceptance criterion
- non-goals
- spec-test-code-output-metric traceability

## Consequences

Positive:

- reviewers evaluate behavior and risk instead of raw diff size
- fixture-heavy and docs-heavy work is encouraged when it clarifies behavior
- output contracts become safer to evolve
- future agents can reason from durable repository artifacts

Negative:

- PRs may look large in GitHub despite narrow production risk
- contributors must maintain docs, specs, goldens, metrics, and traceability
- automation is needed to keep the process lightweight
