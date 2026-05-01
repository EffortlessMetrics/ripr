# ADR 0003: Fixtures Before Analyzer Rewrites

Status: accepted

Date: 2026-05-01

## Context

The analyzer needs parser-backed syntax facts, better probe ownership, richer
oracle extraction, local flow, and activation modeling. These changes can easily
alter output in ways that look plausible but are not actually better.

## Decision

Build a fixture laboratory and golden output tests before major analyzer
rewrites.

The fixture lab should cover:

- boundary gaps
- weak error oracles
- unasserted fields
- unobserved side effects
- smoke-only assertions
- missing static paths
- opaque fixtures
- cross-crate workspaces
- duplicate symbols
- stacked test attributes
- macro unknowns
- snapshot oracles
- mock effects

## Consequences

Positive:

- analyzer changes have objective review targets
- output drift becomes visible
- agents can map specs to tests to code

Negative:

- early fixture work delays parser implementation
- fixture maintenance becomes part of every output-changing PR
