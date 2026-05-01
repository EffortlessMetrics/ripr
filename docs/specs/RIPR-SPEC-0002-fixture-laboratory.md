# RIPR-SPEC-0002: Fixture Laboratory

Status: planned

## Problem

Analyzer improvements are hard to trust without stable examples that show the
expected finding shape, output shape, and unknown behavior. Without fixtures,
precision changes can drift silently.

## Behavior

The repository should contain a fixture laboratory where each capability is
represented by source code, tests, a diff, and expected outputs.

Each fixture should make one behavior question obvious:

```text
Given this changed behavior and these tests, what static exposure evidence
should ripr report?
```

## Required Fixture Shape

Each fixture should include:

- source files
- test files
- `diff.patch`
- expected JSON
- expected human output
- expected context packet when applicable
- expected LSP diagnostic shape when applicable

## Initial Fixture Set

- `boundary_gap`
- `weak_error_oracle`
- `field_not_asserted`
- `side_effect_unobserved`
- `smoke_assertion_only`
- `no_static_path`
- `opaque_fixture`
- `workspace_cross_crate`
- `duplicate_symbols`
- `stacked_test_attrs`
- `nested_src_tests_layout`
- `macro_unknown`
- `snapshot_oracle`
- `mock_effect`

## Invariants

- Static output never says `killed` or `survived`.
- Unknowns carry stop reasons.
- Weak oracle evidence does not become strong without explicit support.
- Finding order is deterministic.
- Context packets are parseable.

## Test Mapping

Planned tests:

- golden JSON fixture tests
- golden human fixture tests
- context-packet fixture tests
- invariant tests for static language and unknown stop reasons

## Implementation Mapping

Planned modules:

- `analysis` fixture runner helpers
- `output::json`
- `output::human`
- `app` command orchestration
