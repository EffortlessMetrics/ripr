# Architecture

`ripr` is one published package with strong internal module seams.

```text
CLI / LSP / CI
  -> app
     -> analysis engine
     -> domain
  -> output adapters
```

## Core modules

- `domain`: probe, RIPR evidence, oracle strength, exposure classification.
- `app`: use-case orchestration and public library API.
- `analysis`: diff loading, syntax indexing, probe generation, classification.
- `output`: human, JSON, and GitHub annotation rendering.
- `cli`: command-line entrypoint.
- `lsp`: lightweight sidecar entrypoint.

## Design rules

- Static objects are `Probe`s, not mutants.
- Static output never says `killed` or `survived`.
- Unknowns are first-class outcomes.
- Findings must carry evidence and a recommended next step.
- The first release stays syntax-first. Semantic enrichment comes later.

See also:

- [Charter](CHARTER.md)
- [Static exposure model](STATIC_EXPOSURE_MODEL.md)
- [Output schema](OUTPUT_SCHEMA.md)
