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
- `lsp`: experimental `tower-lsp-server` sidecar entrypoint.

## Analysis scope

The analysis engine selects Rust files according to mode before building its
syntax-first index:

- `instant`: changed Rust files only.
- `draft` / `fast`: packages touched by the diff.
- `deep` / `ready`: all Rust files in the workspace.

This keeps live feedback narrow while preserving an explicit path to wider
manual or CI scans.

## Design rules

- Static objects are `Probe`s, not mutants.
- Static output never says `killed` or `survived`.
- Unknowns are first-class outcomes.
- Findings must carry evidence and a recommended next step.
- The first release stays syntax-first. Semantic enrichment comes later.
- Behavior changes should preserve a spec-test-code trail so future humans and
  agents can recover intent from repository artifacts.
- Implementation modules should keep a single product responsibility: parsing,
  fact extraction, probe generation, classification, orchestration, or
  rendering.

## Mechanical Guards

Run:

```bash
cargo xtask check-workspace-shape
cargo xtask check-architecture
cargo xtask check-public-api
```

These checks use policy metadata in `policy/workspace_shape.txt`,
`policy/architecture.txt`, and `policy/public_api.txt` to preserve the
one-package public surface and internal module boundaries.

See also:

- [Charter](CHARTER.md)
- [Static exposure model](STATIC_EXPOSURE_MODEL.md)
- [Output schema](OUTPUT_SCHEMA.md)
- [Engineering rules](ENGINEERING.md)
- [Spec-test-code traceability](SPEC_TEST_CODE.md)
