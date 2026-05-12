# ADR 0008: TypeScript Parser Substrate

Status: proposed

Date: 2026-05-11

## Context

Campaign 27 (Language Adapter Preview) introduces a `LanguageAdapter` seam
inside the existing `crates/ripr` package and ships preview adapters for
TypeScript and Python alongside the Rust reference adapter. The TypeScript
preview contract is pinned by
[RIPR-SPEC-0027](../specs/RIPR-SPEC-0027-typescript-preview-static-facts.md):
syntax-first owner, test, assertion, related-test, and probe extraction
from `.ts`, `.tsx`, `.js`, and `.jsx` files, with explicit static-limit
reporting for things the syntax-first adapter cannot classify.

The adapter must not depend on `tsc`, the TypeScript language server, a
type checker, a build graph, or a runtime test runner. Syntax facts are
the contract; semantic enrichment is explicitly out of scope. Within that
constraint, the adapter still needs a real parser — regex-driven token
recognition cannot robustly handle template literals, JSX/TSX, decorators,
nested arrow functions, computed property keys, or the broader TypeScript
syntax surface.

The parser decision should be explicit before the dependency-allowlist
update and adapter implementation slices land so future PRs do not mix
parser choice with adapter shape, output metadata, or generated CI
behavior.

The choice also locks the workspace dependency surface (cargo-deny and
`policy/dependency_allowlist.txt` apply). Mature ecosystem options that
exist today include `swc_ecma_parser`, `oxc_parser`, `biome_js_parser`,
and `tree-sitter-typescript`. Each has different ownership, AST shape,
compile footprint, and tooling-vs-bundler design intent.

This ADR records the parser pick. It does not add the dependency, write
adapter code, or change any fact-extraction behavior — those land in the
next two slices of Campaign 27.

## Decision

Use **`oxc_parser`** as the TypeScript/JavaScript syntax substrate for
the TypeScript preview adapter, following the same pattern as
[ADR 0006](0006-rust-syntax-substrate.md) (Rust syntax substrate).

Parser-specific types stay inside the adapter implementation. The rest
of the product keeps consuming the language-neutral fact shells from
RIPR-SPEC-0026/0027 and the existing domain DTOs:

- `LanguageFacts`, `OwnerFact`, `OwnerKind`, `TestFact`, `AssertionFact`,
  `RelatedTest`, `StaticLimitKind` (per RIPR-SPEC-0026)
- existing `Probe`, `Finding`, `OracleKind`, `OracleStrength`,
  `FlowSinkFact` from `crate::domain`

The adapter must:

- handle `.ts`, `.tsx`, `.js`, `.jsx`;
- emit explicit `StaticLimitKind` values when syntax-first analysis
  cannot classify (no silent coercion to `no_static_path`);
- not invoke a TypeScript type checker, build graph, or runtime tooling
  by default;
- isolate `oxc_*` types behind the `TypeScriptAdapter` impl so the trait
  surface stays language-neutral.

`oxc_parser` joins `ra_ap_syntax` as an approved crate decision in
`policy/dependency_allowlist.txt`; the actual Cargo dependency is added
in the next scoped PR.

## Consequences

Positive:

- TypeScript syntax parsing stays in-process and Rust-native.
- Compile footprint is small relative to alternatives — `oxc_parser` is
  designed for tooling (linters, AST analyzers) rather than transform
  pipelines.
- The parser ecosystem (oxc-project) maintains a stable AST contract and
  treats correctness as a first-class concern.
- JSX, TSX, and modern TypeScript syntax are supported out of the box.
- Parser-specific API churn is quarantined behind `TypeScriptAdapter`
  (RustAdapter's pattern with `ra_ap_syntax`).
- Future Python and other-language adapters can pick their own
  substrate without re-litigating this one.

Negative:

- `ripr` adds a production parser dependency, expanding the
  cargo-deny audit surface.
- `oxc_parser` is younger than `swc_ecma_parser` and may have a steeper
  ecosystem maturity curve. We accept that in exchange for the lighter
  footprint and tooling fit.
- Adapter code must translate parser nodes into repository-owned DTOs
  instead of leaking `oxc_*` types across module seams.
- Syntax-backed extraction still cannot answer semantic questions
  requiring TypeScript type inference (e.g., resolving an
  imported-but-unanalyzed symbol's call signature). Those cases must
  emit `StaticLimitKind::missing_import_graph` rather than guessing.

## Alternatives Considered

- **`swc_ecma_parser`**. The most mature Rust-native TypeScript parser,
  used in Next.js, Deno, and many production tooling pipelines. Rejected
  because its design intent is the full SWC transform pipeline (parser +
  visitor + codegen + Source Map machinery), which pulls a much larger
  compile and audit surface than the adapter needs. We do not transform
  source; we only read it.
- **`biome_js_parser`**. The parser from the Biome (formerly Rome)
  toolchain. Comparable in correctness to `oxc_parser` and similarly
  designed for static tooling. Rejected because Biome's parser is split
  across several crates (`biome_rowan`, `biome_js_syntax`,
  `biome_js_parser`, plus shared infrastructure), which is more surface
  than the adapter needs for syntax-first fact extraction. `oxc_parser`
  is more focused for our use case. If `oxc_parser` later proves
  unsuitable (e.g., AST stability concerns, project abandonment), Biome
  is the natural fallback and this ADR should be superseded.
- **`tree-sitter-typescript`**. Portable, language-neutral parser model
  used by Neovim, Helix, and GitHub's code-search. Rejected because its
  CST/byte-range model differs from the AST-shaped extraction the spec
  implies, and porting the Rust adapter's owner / test / assertion
  patterns to tree-sitter queries would diverge from `RustAdapter`'s
  shape unnecessarily.
- **Roll a syntax-aware regex extractor**. Rejected because
  RIPR-SPEC-0027 requires extracting owners, tests, and assertions
  across JSX/TSX, async functions, decorators, template literals, and
  nested arrow consts. A regex/heuristic extractor cannot meet that
  bar without becoming a fragile parser of its own.
- **Defer parser choice until TypeScript adapter implementation**.
  Rejected because the dependency-allowlist gate requires a documented
  rationale before adding the crate, and review burden is lower when
  the choice is recorded separately from the adapter code.

## Revisit Criteria

This ADR should be revisited if any of these change:

- `oxc_parser` stops releasing or breaks AST stability without a clear
  migration path.
- A new Rust-native parser (e.g., `biome_js_parser` consolidation, a new
  `tree-sitter-typescript` API generation, or another option) makes a
  measurable correctness or compile-footprint difference.
- The TypeScript adapter discovers a class of seams that requires
  TypeScript type information (not just syntax), at which point the
  static-limit reporting boundary itself needs revision via a follow-up
  ADR or proposal.
- A future Campaign 27 work item adds a SWC-based pass (e.g., for
  source-map consumption), which would suggest unifying on `swc_*`.

## Related Specs and Campaigns

- [RIPR-SPEC-0026: Language adapter contract](../specs/RIPR-SPEC-0026-language-adapter-contract.md)
- [RIPR-SPEC-0027: TypeScript preview static facts](../specs/RIPR-SPEC-0027-typescript-preview-static-facts.md)
- [RIPR-PROP-0001: Multi-Language Adapter Preview](../proposals/RIPR-PROP-0001-multi-language-adapter-preview.md)
- Campaign 27: Language Adapter Preview (active in
  [`.ripr/goals/active.toml`](../../.ripr/goals/active.toml) and the
  [implementation campaigns ledger](../IMPLEMENTATION_CAMPAIGNS.md)).

A separate ADR will pin the Python parser choice when the
`analysis/python-preview-adapter` work item opens.
