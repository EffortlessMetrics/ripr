# RIPR-SPEC-0004: Test Efficiency and Vacuity Signals

Status: planned

## Problem

`ripr` can now explain whether changed behavior appears exposed to a meaningful
test discriminator. The same facts can also reveal tests that appear to add low
or duplicate discriminator value.

This should not become a judgement that tests are bad. A smoke test, broad
integration test, or opaque fixture can be intentionally useful. The product
needs an advisory lens that reports evidence and risk shape.

## Behavior

`ripr` should provide a test-efficiency report that answers:

```text
Which tests appear low-discriminator, smoke-only, broad, opaque, circular, or
duplicative for the behavior they appear to reach?
```

The report should use conservative classifications:

- `strong_discriminator`
- `useful_but_broad`
- `smoke_only`
- `likely_vacuous`
- `possibly_circular`
- `duplicative`
- `opaque`

Each advisory finding should include reasons such as:

- no assertion detected
- smoke oracle only
- broad error oracle for exact error behavior
- missing boundary activation value
- assertion does not match the visible flow sink
- duplicate activation and oracle shape
- expected value appears computed from the production path
- fixture, helper, macro, or integration boundary is opaque

## Required Evidence

Each report item should include enough evidence for a reviewer to decide whether
the signal matters:

- test name, file, and line
- owners or functions the test appears to reach
- oracle kind and strength
- observed values
- related flow sinks when known
- missing discriminator overlap when known
- duplicate group, if any
- recommended next step, if any

## Non-Goals

This spec does not require:

- blocking CI
- deleting tests
- global suite scoring
- runtime duration analysis
- real mutation execution
- repository-specific test intent config in the first report

## Acceptance Examples

Smoke-only test:

```text
Given a test that calls production code and only unwraps the result, when ripr
builds the test-efficiency report, then it classifies the test as smoke_only and
explains that no meaningful discriminator was detected.
```

Broad error oracle:

```text
Given a changed exact error variant and a related test that only asserts
is_err(), when ripr builds the test-efficiency report, then it classifies the
oracle as useful_but_broad or smoke_only and recommends an exact variant
assertion.
```

Duplicate discriminator:

```text
Given two tests that reach the same owner with the same activation values and
the same oracle shape, when ripr builds the test-efficiency report, then it
groups them as duplicative without recommending deletion.
```

## Invariants

- Signals are advisory.
- Report language should not call tests bad.
- Static output uses only the conservative static-exposure vocabulary from the
  language policy.
- Opaque is a valid result when static evidence cannot see through helpers,
  fixtures, macros, or integration boundaries.

## Test Mapping

Current and planned tests:

- existing oracle-strength and activation-value BDD tests
- existing `cargo xtask test-oracle-report`
- planned unit tests for test-efficiency classification reasons
- planned fixture coverage for smoke-only, broad-error, duplicate-boundary, and
  opaque-helper cases

## Implementation Mapping

Planned modules:

- `analysis` test, oracle, owner, value, and flow facts
- `xtask` report command
- future `output` rendering if the report becomes a product output surface

## Metrics

- strong discriminator count
- useful broad oracle count
- smoke-only count
- likely vacuous count
- possible circularity count
- duplicate discriminator count
- opaque test count
- unique activation values per owner
