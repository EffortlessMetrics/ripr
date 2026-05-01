# Roadmap

## 0.1

- Small-dependency publishable crate.
- Syntax-first diff probe generation.
- Test and oracle indexing.
- Static RIPR classification.
- Human, JSON, GitHub output.
- Basic LSP sidecar.

## 0.1.1

- Replace manual CLI parsing with `clap` derive.
- Convert JSON output rendering to serde output DTOs.
- Add more fixture coverage for duplicate names, stacked attributes, and nested path layouts.

## 0.2

- `ripr.toml` config loading.
- SARIF output.
- Better workspace topology detection.
- Basic persistent cache.
- Richer LSP diagnostics and code actions.

## 0.3

- rust-analyzer/HIR enrichment.
- SQLite index and hot bitsets.
- Per-test reachability cache.
- cargo-mutants calibration import.

## 0.4+

- Deep mode with MIR/Charon-style summaries.
- Proc-macro and feature-set awareness.
- Learned oracle priors.
- Test skeleton generation.
