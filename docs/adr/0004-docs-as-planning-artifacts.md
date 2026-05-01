# ADR 0004: Docs as Planning Artifacts

Status: accepted

Date: 2026-05-01

## Context

The project will be built PR by PR across product design, analyzer internals,
editor integration, CI, calibration, and documentation. Chat history is not a
durable planning system.

## Decision

Use in-repo docs as the source of truth for planning and progress:

- roadmap for sequence and release framing
- implementation plan for PR checklists
- specs for user-visible behavior
- ADRs for durable decisions
- metrics for capability and regression tracking
- learnings for repo knowledge
- changelog for notable changes
- spec-test-code traceability for agent reasoning

## Consequences

Positive:

- future agents can continue from repository state
- reviewers can evaluate scope against explicit plans
- behavior, tests, and code stay easier to align

Negative:

- docs require maintenance in every meaningful PR
- stale docs become harmful if not treated as part of the gates
