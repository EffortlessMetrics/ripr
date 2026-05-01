# ADR 0002: Static Exposure Language

Status: accepted

Date: 2026-05-01

## Context

`ripr` uses mutation-testing concepts but does not run mutants during normal
static analysis. Overstating static findings would mislead users and make the
tool harder to trust.

## Decision

Static findings use only conservative exposure language:

- `exposed`
- `weakly_exposed`
- `reachable_unrevealed`
- `no_static_path`
- `infection_unknown`
- `propagation_unknown`
- `static_unknown`

Static findings must not claim:

- `killed`
- `survived`
- `untested`
- `proven`
- `adequate`

Real mutation outcomes can appear only in explicit calibration output where real
mutation data is present.

## Consequences

Positive:

- preserves trust in draft-mode findings
- keeps the product distinct from mutation runners
- makes unknowns honest and actionable

Negative:

- output is less dramatic than traditional mutation testing language
- users may need calibration docs to understand where real mutation fits
