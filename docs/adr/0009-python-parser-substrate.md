# ADR 0009: Python Parser Substrate

Status: proposed

Date: 2026-05-12

## Context

Campaign 27 (Language Adapter Preview) introduces a `LanguageAdapter`
seam inside the existing `crates/ripr` package and ships preview
adapters for TypeScript and Python alongside the Rust reference
adapter. The Python preview contract is pinned by
[RIPR-SPEC-0028](../specs/RIPR-SPEC-0028-python-preview-static-facts.md):
syntax-first owner, test, assertion, related-test, and probe extraction
from `*.py` files, with explicit static-limit reporting for things the
syntax-first adapter cannot classify.

The adapter must not depend on `mypy`, `pyright`, a runtime test
runner, or an import graph. Syntax facts are the contract; semantic
enrichment is explicitly out of scope. Within that constraint, the
adapter still needs a real parser — regex-driven token recognition
cannot robustly handle decorators, `async def`, `match`/`case`, f-strings,
type-parameter syntax (PEP 695), or the broader Python grammar surface.

The parser decision should be explicit before the dependency-allowlist
update and adapter implementation slices land so future PRs do not mix
parser choice with adapter shape, output metadata, or generated CI
behavior — the same separation
[ADR 0008](0008-typescript-parser-substrate.md) used for the TypeScript
adapter.

The choice also locks the workspace dependency surface (cargo-deny and
`policy/dependency_allowlist.txt` apply). Mature Rust-native options
that exist today include `rustpython-parser`, `ruff_python_parser`, and
`tree-sitter-python`. Each has different ownership, AST shape, compile
footprint, and tooling-vs-runtime design intent.

This ADR records the parser pick. It does not add the dependency, write
adapter code, or change any fact-extraction behavior — those land in
the next slices of Campaign 27.

## Decision

Use **`ruff_python_parser`** as the Python syntax substrate for the
Python preview adapter, following the same pattern as
[ADR 0006](0006-rust-syntax-substrate.md) (Rust syntax substrate) and
[ADR 0008](0008-typescript-parser-substrate.md) (TypeScript syntax
substrate): a small, Rust-native, static-tooling-focused parser
quarantined behind the adapter implementation.

Parser-specific types stay inside the adapter implementation. The rest
of the product keeps consuming the language-neutral fact shells from
RIPR-SPEC-0026/0028 and the existing domain DTOs:

- `LanguageFacts`, `OwnerFact`, `OwnerKind`, `TestFact`, `AssertionFact`,
  `RelatedTest`, `StaticLimitKind` (per RIPR-SPEC-0026)
- existing `Probe`, `Finding`, `OracleKind`, `OracleStrength`,
  `FlowSinkFact` from `crate::domain`

The adapter must:

- handle `*.py` and (when the adapter later opts in) Jupyter Python
  cells via `*.ipynb` extraction routed through this parser;
- emit explicit `StaticLimitKind` values when syntax-first analysis
  cannot classify (no silent coercion to `no_static_path`);
- not invoke a Python type checker, import resolver, build graph, or
  runtime tooling by default;
- isolate `ruff_python_*` types behind the `PythonAdapter` impl so the
  trait surface stays language-neutral.

`ruff_python_parser` joins `ra_ap_syntax` and `oxc_parser` as an
approved crate decision in `policy/dependency_allowlist.txt`; the
actual Cargo dependency is added in the next scoped PR.

## Consequences

Positive:

- Python syntax parsing stays in-process and Rust-native, matching the
  `RustAdapter` (`ra_ap_syntax`) and `TypeScriptAdapter` (`oxc_parser`)
  patterns.
- Compile footprint is small relative to alternatives —
  `ruff_python_parser` is designed for static analyzers (Ruff itself
  uses it for lints and AST queries) rather than a full Python compiler
  pipeline.
- The Ruff project maintains an active AST contract with grammar
  upgrades tracked alongside CPython releases (PEP 695 type params,
  `match`/`case`, f-strings, etc.).
- Decorator, `async def`, `class`, comprehension, and pattern-matching
  syntax are supported out of the box.
- Parser-specific API churn is quarantined behind `PythonAdapter`
  (matches the RustAdapter / TypeScriptAdapter pattern).
- Future Python typing-aware analysis (if ever needed) can sit *on top*
  of the same AST without re-litigating the parser choice.

Negative:

- `ripr` adds a production parser dependency, expanding the cargo-deny
  audit surface.
- `ruff_python_parser` is published from the Ruff repository where
  Ruff itself is the primary consumer. We accept that the API may
  evolve faster than a parser whose stability contract is its product —
  the adapter quarantines the dependency to a single module to absorb
  churn.
- Adapter code must translate parser nodes into repository-owned DTOs
  instead of leaking `ruff_python_*` types across module seams.
- Syntax-backed extraction still cannot answer semantic questions
  requiring Python type inference (e.g., resolving a duck-typed
  attribute call). Those cases must emit
  `StaticLimitKind::missing_import_graph` /
  `StaticLimitKind::dynamic_dispatch` rather than guessing.

## Alternatives Considered

- **`rustpython-parser`**. The parser layer of the
  [RustPython](https://github.com/RustPython/RustPython) project (a
  Python implementation in Rust). Mature, follows the CPython AST
  shape, and has a stable published API. Rejected for the same reason
  ADR 0008 rejected `swc_ecma_parser` for TypeScript: its design intent
  is to support a full language implementation (compiler / runtime),
  which is broader than the syntax-first analysis our adapter needs.
  The audit and compile surface is larger than necessary. If
  `ruff_python_parser` later proves unsuitable (e.g., API churn that
  breaks the adapter repeatedly, project pivot), `rustpython-parser` is
  the natural fallback and this ADR should be superseded.
- **`tree-sitter-python`**. Portable, language-neutral parser model
  used by Neovim, Helix, and GitHub's code-search. Rejected for the
  same reason ADR 0008 rejected `tree-sitter-typescript`: its CST /
  byte-range model differs from the AST-shaped extraction the spec
  implies, and porting the Rust and TypeScript adapter shapes (owner /
  test / assertion / probe family classification) to tree-sitter
  queries would diverge from the established adapter pattern without
  a benefit that matches the divergence.
- **Roll a syntax-aware regex extractor**. Rejected because
  RIPR-SPEC-0028 requires extracting owners, tests, assertions, probe
  families, and static limits across decorators, `async def`,
  comprehensions, `match`/`case`, parametrized tests, and f-strings.
  A regex/heuristic extractor cannot meet that bar without becoming a
  fragile parser of its own.
- **Defer parser choice until Python adapter implementation**. Rejected
  because the dependency-allowlist gate requires a documented rationale
  before adding the crate, and review burden is lower when the choice
  is recorded separately from the adapter code — matching how Campaign
  27 split [ADR 0008](0008-typescript-parser-substrate.md) and the
  TypeScript adapter scaffold (PR #759).

## Revisit Criteria

This ADR should be revisited if any of these change:

- `ruff_python_parser` stops being published from the Ruff repository,
  pivots its API in a way that breaks adapter use, or stops keeping pace
  with CPython grammar releases.
- A new Rust-native Python parser (a stable spin-out from Ruff, a new
  `tree-sitter-python` API generation, a focused syntax-only crate, or
  another option) makes a measurable correctness or compile-footprint
  difference.
- The Python adapter discovers a class of seams that requires Python
  type information (not just syntax), at which point the static-limit
  reporting boundary itself needs revision via a follow-up ADR or
  proposal.
- A future Campaign 27 work item adds a typed Python pass (e.g., for
  duck-typed call resolution), which would suggest a parser whose AST
  carries the additional facts natively.

## Related Specs and Campaigns

- [RIPR-SPEC-0026: Language adapter contract](../specs/RIPR-SPEC-0026-language-adapter-contract.md)
- [RIPR-SPEC-0028: Python preview static facts](../specs/RIPR-SPEC-0028-python-preview-static-facts.md)
- [RIPR-PROP-0001: Multi-Language Adapter Preview](../proposals/RIPR-PROP-0001-multi-language-adapter-preview.md)
- [ADR 0008: TypeScript Parser Substrate](0008-typescript-parser-substrate.md)
- Campaign 27: Language Adapter Preview (active in
  [`.ripr/goals/active.toml`](../../.ripr/goals/active.toml) and the
  [implementation campaigns ledger](../IMPLEMENTATION_CAMPAIGNS.md)).

The Python adapter scaffold work item is tracked separately. This ADR
only records the parser pick.
