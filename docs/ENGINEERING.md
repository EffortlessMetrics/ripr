# Engineering Rules

This document records the engineering bar for `ripr` implementation PRs.

## Product-First Scope

Every implementation PR should be traceable to the product contract:

```text
For the behavior changed in this diff, do the current tests appear to contain
a discriminator that would notice if that behavior were wrong?
```

Avoid broad infrastructure unless it makes that answer more precise, faster, or
more actionable.

## Architecture

Keep one published package:

```text
Package: ripr
Binary: ripr
Library: ripr
Automation: xtask, unpublished
```

Use internal seams:

- `domain`: exposure concepts, probes, RIPR evidence, oracle strength, classes
- `app`: use-case orchestration and public library API
- `analysis`: diff, indexing, facts, probes, classification
- `output`: human, JSON, GitHub, future SARIF
- `cli`: command adapter
- `lsp`: editor protocol adapter

Do not split crates until an external contract makes the boundary real.

## SRP and Modularity

Prefer small modules whose names describe the problem concept they own.

Good ownership examples:

- diff parsing owns hunks and changed ranges
- syntax adapter owns parser integration
- fact extraction owns syntax-to-fact conversion
- probe generation owns changed-fact-to-probe conversion
- classifier owns fact/probe-to-finding decisions
- output adapters own rendering only

Avoid modules that mix parsing, analysis, classification, and rendering.

## Error Handling

Target rule:

```text
No panic, unwrap, expect, todo, or unimplemented in production or tests.
```

Use typed errors or `Result<_, String>` where the existing codebase has not yet
introduced a richer error type. Tests should return `Result` and use explicit
assertions instead of unwrap-style failure.

Exceptions require a narrow comment explaining why failure is impossible and why
the exception is better than propagating an error. The preferred long-term count
is zero exceptions.

## Modern Rust

Use Rust 2024 and the workspace minimum Rust version. Prefer standard library
types and clear ownership before adding dependencies. Keep `unsafe_code =
"forbid"`.

When adding types, encode domain states directly instead of passing loosely
typed strings through the analyzer. Keep fallible boundaries explicit with
`Result`, and keep rendering concerns out of domain and analysis types.

## Testing Style

Use BDD-shaped test names and fixtures:

```text
given_changed_boundary_when_only_smoke_oracle_exists_then_reports_weak_exposure
```

Each behavior should have:

- a spec entry
- one or more tests that cite the spec ID or fixture name
- implementation code in the matching module
- golden output when user-visible output changes

## Documentation

Use Diataxis deliberately:

- tutorials: teach first successful use
- how-to guides: solve concrete tasks
- reference: define commands, schemas, config, and enums
- explanation: record model, architecture, ADRs, and tradeoffs

The README should stay problem-first and should surface the most important
metrics, current capability state, and next-step docs.

## Dogfooding

When `ripr` can analyze a behavior shape that exists in its own codebase, add a
fixture or smoke command that uses this repository as an example. Dogfooding
should produce focused evidence and tests, not broad self-analysis dashboards.

## Output Language

Static output may use:

- `exposed`
- `weakly_exposed`
- `reachable_unrevealed`
- `no_static_path`
- `infection_unknown`
- `propagation_unknown`
- `static_unknown`

Static output must not claim:

- `killed`
- `survived`
- `untested`
- `proven`
- `adequate`

Real mutation data can be reported only in explicit calibration output.
