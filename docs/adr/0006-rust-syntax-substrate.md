# ADR 0006: Rust Syntax Substrate

Status: accepted

Date: 2026-05-02

## Context

Campaign 2 moves `ripr` from lexical scanner facts to syntax-backed facts. The
repo already has a `RustSyntaxAdapter` boundary and a lexical adapter, but the
next analyzer work needs a real Rust parser for test, assertion, and oracle
extraction.

The parser decision should be explicit before implementation starts so future
PRs do not mix parser choice with HIR, MIR, caching, output-schema changes, or
broader analyzer rewrites.

## Decision

Use `ra_ap_syntax` as the Rust syntax substrate for Campaign 2 parser-backed
fact extraction.

Parser-specific types stay inside the syntax adapter implementation. The rest
of the product should keep depending on repository-owned facts and ranges:

- `FileFacts`
- `FunctionFact`
- `TestFact`
- `OracleFact`
- `SyntaxNodeFact`
- `TextRange`
- future `SymbolId`

The first parser-backed work should preserve the current lexical adapter as the
fallback path and keep fixture and golden output stable unless the PR explicitly
documents output drift.

## Consequences

Positive:

- Rust syntax parsing stays in-process and Rust-native.
- Test, assertion, macro, and owner extraction can use a syntax tree without
  starting a rust-analyzer server.
- Parser-specific API churn is quarantined behind `RustSyntaxAdapter`.
- Campaign 2 can improve facts without committing to semantic analysis.

Negative:

- `ripr` adds a production parser dependency when parser-backed extraction is
  implemented.
- Adapter code must translate parser nodes into repository-owned DTOs instead
  of leaking parser types across module seams.
- Syntax-backed extraction still cannot answer semantic questions that require
  type inference or control-flow analysis.

## Alternatives Considered

- Keep the lexical scanner only. This avoids a dependency but cannot robustly
  handle stacked attributes, multi-line assertions, nested modules, or macro
  invocation shapes.
- Use `tree-sitter-rust`. This is a portable parser option, but it adds a
  different syntax model and is a weaker fit for Rust-native syntax facts in
  this phase.
- Start a rust-analyzer server or depend on HIR. This is too much machinery for
  Campaign 2 and would mix parser-backed facts with semantic analysis.
- Use MIR or Charon-style lowering. This belongs to a later calibration or deep
  analysis phase, not the syntax-backed foundation campaign.
