# RIPR-SPEC-0001: Static Exposure Loop

Status: accepted

## Problem

Developers and coding agents can change Rust behavior while leaving tests that
execute nearby code but do not discriminate the changed behavior. Coverage does
not identify that oracle gap, and real mutation testing is often too expensive
for live draft feedback.

## Behavior

Given a Rust/Cargo workspace and a diff, `ripr` identifies changed Rust behavior,
creates mutation-shaped probes, and reports whether current tests appear to
contain a discriminator that would notice if that behavior were wrong.

The loop is:

```text
changed behavior
-> static probe
-> related tests
-> RIPR evidence
-> missing or weak discriminator
-> recommended targeted test intent
```

## Required Evidence

Each finding should carry:

- changed behavior
- probe family
- RIPR stage evidence
- related tests, if any
- oracle evidence, if any
- observed activation values, if statically visible
- local flow sink family when the changed behavior reaches an observable
  returned value, error variant, output field, match result, event/outbound
  call, state write, persistence write, log message, configuration change, or
  generic call effect
- missing discriminator
- recommended next step
- stop reason for unknowns

## Inputs

- Rust/Cargo workspace root
- Git base or explicit unified diff
- analysis mode
- optional repository configuration

## Outputs

- human findings
- versioned JSON findings
- GitHub annotations
- LSP diagnostics and hover content when used through the editor
- agent context packet for a selected finding

## Classifications

Static findings may use only these exposure classes:

- `exposed`
- `weakly_exposed`
- `reachable_unrevealed`
- `no_static_path`
- `infection_unknown`
- `propagation_unknown`
- `static_unknown`

## Non-Goals

This spec does not require:

- running mutants
- proving adequacy
- generating complete tests
- whole-workspace semantic proof
- coverage reporting

## Acceptance Examples

Boundary example:

```rust
if amount >= discount_threshold {
    apply_discount(...)
}
```

If existing tests use `50` and `10_000`, and only assert
`quote.total > Money::zero()`, `ripr` should report weak exposure and name the
missing equality-boundary value and exact assertion shape.

Error example:

```rust
Err(Error::InvalidCurrency)
```

If a related test only checks `result.is_err()`, `ripr` should distinguish that
from an exact variant assertion.

Side-effect example:

```rust
events.publish(DiscountApplied { amount })
```

`ripr` should report the propagation sink as an event or outbound call rather
than a generic call effect. Similar syntax-first labels should be available for
state writes, persistence writes, log messages, and configuration changes. When
the sink family is not statically obvious, `ripr` should keep the older
`call_effect` fallback or report propagation as unknown with a stop reason.

## Test Mapping

Fixture coverage:

- `fixtures/boundary_gap` (baseline)
- `fixtures/weak_error_oracle` (baseline)
- `fixtures/smoke_assertion_only`
- `fixtures/no_static_path`
- unit coverage for local flow sink families: predicate-to-return,
  predicate-to-error, match-arm result, output field, event/outbound call,
  state write, persistence write, log message, configuration change, and
  unknown propagation fallback

## Implementation Mapping

Current and planned modules:

- `analysis`: diff loading, file facts, probe generation, classification
- `domain`: probes, RIPR evidence, oracle strength, exposure class
- `output`: human, JSON, GitHub, future SARIF rendering
- `lsp`: diagnostics, hover, actions

## Metrics

- fixture pass rate
- unknowns with stop reasons
- oracle kind recognition rate
- flow sink identification rate
- activation value extraction rate
- static runtime by mode
