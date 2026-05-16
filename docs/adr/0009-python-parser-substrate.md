# ADR 0009: Python Parser Substrate

Status: proposed (superseding its own original decision recorded
in commit `d70f1802`).

Date: 2026-05-12

## Correction Note

The first version of this ADR picked `ruff_python_parser`. That pick
turned out to rest on a failed assumption about availability:
`ruff_python_parser` is a workspace crate inside the
[astral-sh/ruff](https://github.com/astral-sh/ruff) monorepo and is
marked `publish = false`, so it is not available on crates.io. The
only ways to depend on it are a git dependency (rejected by our
production dependency policy) or a third-party vendored fork
(supply-chain and maintenance risk we will not take). The original
ADR even named `rustpython-parser` as the documented natural fallback
under Revisit Criteria; we are exercising that fallback here.

The architectural envelope is unchanged: a small, Rust-native,
syntax-first parser quarantined behind `PythonAdapter`, with no
Python runtime, type checker, build graph, or git dependency. Only
the substrate name changes.

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
`policy/dependency_allowlist.txt` apply). Rust-native parser options
that exist today and are usable as a normal Cargo dependency include
`rustpython-parser` and `tree-sitter-python`. Astral's
`ruff_python_parser` would otherwise be a natural fit on technical
grounds but is intentionally unpublished from the `astral-sh/ruff`
workspace (`publish = false`); see the Alternatives Considered
section for details on why we treat that as a hard exclusion rather
than a soft trade-off.

This ADR records the parser pick. It does not add the dependency, write
adapter code, or change any fact-extraction behavior — those land in
the next slices of Campaign 27.

## Decision

Use **`rustpython-parser`** (published on crates.io) as the Python
syntax substrate for the Python preview adapter, following the same
pattern as [ADR 0006](0006-rust-syntax-substrate.md) (Rust syntax
substrate) and [ADR 0008](0008-typescript-parser-substrate.md)
(TypeScript syntax substrate): a small, Rust-native, syntax-first
parser quarantined behind the adapter implementation.

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
- isolate `rustpython_parser` / `rustpython-ast` types behind the
  `PythonAdapter` impl so the trait surface stays language-neutral.

`rustpython-parser` joins `ra_ap_syntax` and `oxc_parser` as an
approved crate decision in `policy/dependency_allowlist.txt`; the
actual Cargo dependency is added in the next scoped PR.

## Consequences

Positive:

- Python syntax parsing stays in-process and Rust-native, matching the
  `RustAdapter` (`ra_ap_syntax`) and `TypeScriptAdapter` (`oxc_parser`)
  patterns.
- `rustpython-parser` is published on crates.io, so the workspace can
  declare a normal Cargo dependency without git URLs or vendored
  forks.
- AST shape follows the CPython AST closely, which keeps fact
  extraction recognisable to anyone who has used Python's own `ast`
  module as a reference.
- Decorator, `async def`, `class`, comprehension, `match`/`case`, and
  f-string syntax are supported by the released parser.
- Parser-specific API churn is quarantined behind `PythonAdapter`
  (matches the RustAdapter / TypeScriptAdapter pattern).
- Future Python typing-aware analysis (if ever needed) can sit *on top*
  of the same AST without re-litigating the parser choice.

Negative:

- `ripr` adds a production parser dependency, expanding the cargo-deny
  audit surface.
- `rustpython-parser` is the parser layer of the RustPython compiler
  project, so it carries a small amount of "compiler-shaped" weight
  relative to a parser whose only product is static analysis. We
  accept that in exchange for crates.io availability and a stable
  published API surface.
- Adapter code must translate parser nodes into repository-owned DTOs
  instead of leaking `rustpython_*` types across module seams.
- Syntax-backed extraction still cannot answer semantic questions
  requiring Python type inference (e.g., resolving a duck-typed
  attribute call). Those cases must emit
  `StaticLimitKind::missing_import_graph` /
  `StaticLimitKind::dynamic_dispatch` rather than guessing.

## Alternatives Considered

- **`ruff_python_parser`** (the original pick this ADR superseded).
  Designed for static analyzers, Rust-native, actively maintained
  inside [astral-sh/ruff](https://github.com/astral-sh/ruff). Rejected
  because the crate is marked `publish = false` in the Ruff workspace
  and is not available on crates.io. Depending on it would require
  either a git dependency (rejected by our production dependency
  policy and `policy/dependency_allowlist.txt`) or a third-party
  vendored fork (`littrs-ruff-python-parser`,
  `rustpython-ruff_python_parser`), which carries supply-chain and
  maintenance risk we will not take. If Astral later publishes the
  parser to crates.io under stable versioning, this ADR should be
  revisited.
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
- **Invoke Python at build/test time (subprocess into `python -m ast`)**.
  Rejected because RIPR-SPEC-0028 requires no Python runtime
  dependency: `ripr` is a single Rust binary that must work on hosts
  without a Python install. A subprocess parser also breaks
  reproducibility — the adapter's facts would depend on whichever
  Python interpreter happens to be on PATH.
- **`littrs-ruff-python-parser` / `rustpython-ruff_python_parser`**
  (third-party re-publications of Ruff's parser). Rejected because
  neither is maintained by Astral. Picking either ties the adapter to
  a single contributor's pace and willingness to track upstream Ruff
  changes, which is exactly the supply-chain risk the Ruff workspace
  itself would solve if published canonically. If a canonical
  publication appears, this ADR should be revisited.
- **Defer parser choice until Python adapter implementation**. Rejected
  because the dependency-allowlist gate requires a documented rationale
  before adding the crate, and review burden is lower when the choice
  is recorded separately from the adapter code — matching how Campaign
  27 split [ADR 0008](0008-typescript-parser-substrate.md) and the
  TypeScript adapter scaffold (PR #759).

## Revisit Criteria

This ADR should be revisited if any of these change:

- `rustpython-parser` stops releasing, breaks its published AST
  contract without a clear migration path, or stops keeping pace with
  CPython grammar releases.
- Astral publishes `ruff_python_parser` to crates.io under stable
  versioning, eliminating the availability gap that drove this
  correction. At that point the lighter static-tooling-focused parser
  becomes available as a normal Cargo dependency and the original ADR
  rationale (Ruff parser → static-tool fit) regains its weight.
- A new Rust-native Python parser (a focused syntax-only crate spun
  out from Ruff, a new `tree-sitter-python` API generation, or another
  option) makes a measurable correctness or compile-footprint
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
