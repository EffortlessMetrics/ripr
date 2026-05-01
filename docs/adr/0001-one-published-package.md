# ADR 0001: One Published Package

Status: accepted

Date: 2026-05-01

## Context

`ripr` needs clear architecture without prematurely creating public contracts.
The current product has a CLI, library API, output renderers, analysis engine,
and experimental LSP sidecar.

## Decision

Keep one published package:

```text
Package: ripr
Binary: ripr
Library: ripr
Automation: xtask, unpublished
```

Use internal module seams:

- `domain`
- `app`
- `analysis`
- `output`
- `cli`
- `lsp`

Do not split into `ripr-core`, `ripr-cli`, `ripr-lsp`, `ripr-engine`, or
`ripr-schema` until a real external contract exists.

## Consequences

Positive:

- simpler publishing and versioning
- easier refactoring while the model is still evolving
- fewer artificial boundaries for agents to reason about

Negative:

- internal boundaries require discipline rather than crate-level enforcement
- future extraction may require cleanup if external consumers appear
