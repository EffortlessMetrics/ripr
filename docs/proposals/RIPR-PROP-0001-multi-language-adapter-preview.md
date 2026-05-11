# RIPR-PROP-0001: Multi-Language Adapter Preview

Status: proposed

Owner: ripr maintainers

Created: 2026-05-11

Target campaign: Campaign 27, Language Adapter Preview

Linked specs:

- [RIPR-SPEC-0026: Language adapter contract](../specs/RIPR-SPEC-0026-language-adapter-contract.md)
- [RIPR-SPEC-0027: TypeScript preview static facts](../specs/RIPR-SPEC-0027-typescript-preview-static-facts.md)
- [RIPR-SPEC-0028: Python preview static facts](../specs/RIPR-SPEC-0028-python-preview-static-facts.md)

Linked ADRs:

- ADR for the language-adapter boundary (deferred to the adapter-boundary
  PR if the decision needs durable record).

Linked work items:

- See Campaign 27 in
  [Implementation campaigns](../IMPLEMENTATION_CAMPAIGNS.md).

## Problem

`ripr` answers one draft-time question for Rust pull requests today, end to
end. The analyzer, output schema, LSP, agent surfaces, generated CI, gates,
baselines, ledgers, proof, front panel, packet index, and inline-comment
publisher are all single-language by accident: every fact-extraction and
test-discovery module assumes Rust source, Rust tests, and the Rust
workspace layout.

That assumption is now the biggest adoption gap. Teams that already trust
the saved-workspace editor loop and the advisory PR review front panel on
Rust ask whether the same evidence surface can run against the TypeScript
or Python parts of their repos — without RIPR forking into separate tools,
separate schemas, or separate UX.

The assumption is also a quiet drag on contributor work. Several internal
modules already look like single-language reductions of a more general
fact-extraction interface, and the output, CLI, LSP, agent, and Lane 4
report producers are already language-neutral by accident. Without an
explicit adapter boundary, the cost of a second language grows each
campaign.

## Users and surfaces

- mixed-language repository maintainers who want one evidence surface
- developers using the VS Code extension in TypeScript or Python projects
- reviewers reading the advisory PR review front panel on mixed-language
  PRs
- coding agents (Codex, Kiro, Claude Code, Cursor, generic) consuming
  agent seam packets across languages
- ripr contributors who would otherwise touch single-language assumptions
  on every PR

## Success criteria

- Rust analyzer behavior, fixtures, goldens, capability statuses, and
  release semantics are unchanged.
- An internal `LanguageAdapter` boundary exists inside
  `crates/ripr/src/analysis/`; Rust is the reference adapter.
- Existing reports gain only additive optional `language` and
  `language_status` fields. No schema version bump. No per-language
  schema fork.
- Repo configuration adds `[languages] enabled = ["rust"]` as the default,
  with explicit opt-in to enable preview adapters.
- TypeScript and Python preview adapters emit syntax-first owner, test,
  assertion, related-test, and probe facts, plus explicit static-limit
  reporting where syntax-first analysis cannot classify.
- VS Code extension language selectors cover TypeScript, JavaScript, and
  Python only after preview adapters are enabled; saved-workspace defaults
  for Rust stay identical.
- Generated GitHub CI stays Rust-default and advisory; language grouping
  appears only when `[languages]` declares more than Rust.
- One published crate, one binary, one library, one LSP server, one VS
  Code extension across all languages.
- `cargo xtask dogfood` carries checked TypeScript and Python preview
  receipts.

## Proposed shape

Inside the existing package, add a language-neutral analysis seam:

```text
diff
-> language router (file path + repo config)
-> per-language LanguageAdapter
   -> source facts (owners, tests, assertions)
   -> probes (changed owners x changed lines)
   -> classification (existing exposure classes)
-> domain RIPR evidence (language-neutral)
-> existing output, LSP, agent, CI, gate, ledger, proof, front-panel,
   packet-index, and review-comments surfaces
```

The router is pure path classification plus repo configuration. The adapter
does the language work. The domain stays language-neutral so every existing
report can carry preview-language findings without a schema fork.

Rust remains the reference adapter and the only adapter that may be
`stable` or higher under the existing capability vocabulary. TypeScript and
Python adapters are `alpha` and explicitly `preview`.

## Alternatives considered

| Alternative | Why we are not picking it |
| --- | --- |
| Split the workspace into `ripr-core`, `ripr-cli`, `ripr-lsp`, per-language crates. | Premature; there is no external consumer that justifies the API surface cost, and the existing one-package shape is enforced by policy for good reason. |
| Add language-specific CLI subcommands and per-language output schemas. | Forks the user experience and the report packet, multiplying reviewer load and complicating the editor extension. |
| Depend on `tsc` and `mypy`/`pyright` by default. | Adds a heavy runtime dependency and a long install path for marginal gains in the preview stage; syntax-first facts already produce useful signal. |
| Run mutation testing per language. | Out of scope. `ripr` is a static oracle-gap analyzer; mutation execution belongs to other tools. |
| Ship TypeScript-only or Python-only first and add the other later. | Either order forces the analyzer into language-specific shape before the boundary is proven; introducing the adapter contract before per-language facts is cheaper than refactoring after. |

## Behavior specs to create or update

- `RIPR-SPEC-0026`: Language adapter contract (boundary, language router,
  output metadata, repo config opt-in, preview posture).
- `RIPR-SPEC-0027`: TypeScript preview static facts (owners, tests,
  assertions, related tests, probes, static limits, fixture coverage).
- `RIPR-SPEC-0028`: Python preview static facts (owners, tests,
  assertions, related tests, probes, static limits, fixture coverage).

## Architecture decisions needed

- ADR for the language-adapter boundary may be opened as part of the
  `analysis/language-adapter-boundary` work item if the decision needs
  durable record beyond the spec; otherwise omit.
- ADR is not needed for individual preview-adapter implementations.

## Implementation campaign shape

Campaign 27, Language Adapter Preview, in
[Implementation campaigns](../IMPLEMENTATION_CAMPAIGNS.md):

1. `spec/language-adapter-preview-contract` (this work, RIPR-SPEC-0026/27/28).
2. `analysis/language-adapter-boundary`.
3. `analysis/rust-adapter-behind-boundary` (no Rust regression).
4. `output/language-metadata`.
5. `analysis/typescript-preview-adapter`.
6. `analysis/python-preview-adapter`.
7. `lsp/editor-language-routing`.
8. `ci/language-aware-grouping`.
9. `docs/language-adapter-preview-workflow`.
10. `dogfood/language-adapter-preview-receipts`.
11. `campaign/language-adapter-preview-closeout`.

Each slice follows the [scoped PR contract](../SCOPED_PR_CONTRACT.md).

## Evidence plan

- Rust fixture and golden regression suite must stay unchanged across the
  campaign.
- Additive optional `language` and `language_status` fields roundtrip
  through JSON serialization and appear only when populated.
- TypeScript and Python preview fixtures pin syntax-first owners, tests,
  assertions, related tests, probes, and at least one fixture per
  language for explicit static-limit reporting (dynamic dispatch,
  decorator indirection, missing import graph).
- Generated CI fixtures prove Rust-default behavior is unchanged and
  prove advisory grouping appears only when `[languages]` declares more
  than Rust.
- LSP protocol smoke covers at least one TypeScript and one Python seam.
- VS Code extension e2e covers opening a TypeScript and a Python file
  when preview adapters are enabled.
- `cargo xtask dogfood` carries TypeScript and Python preview receipts.
- Capability matrix gains `Language adapter boundary` (alpha) plus
  `TypeScript preview static facts` (alpha, preview) and
  `Python preview static facts` (alpha, preview); Rust capability rows do
  not regress.

## Risks

- Preview adapters might be read as production-ready. Mitigation: every
  surface labels preview adapters explicitly; capability matrix labels
  them `alpha`; generated CI summaries name the preview label; docs
  describe static limits.
- Schema fork pressure. Mitigation: the contract forbids per-language
  schemas; new fields are additive and optional.
- Workspace split pressure. Mitigation: policy already enforces a
  single package; the campaign adds no new crate or binary.
- Editor extension scope creep. Mitigation: language selectors are
  additive; saved-workspace defaults for Rust stay identical.
- Dependency creep (typecheckers, runtime tools). Mitigation: preview
  adapters are syntax-first by contract; `policy/dependency_allowlist.txt`
  remains the gate.
- Adoption confusion. Mitigation: the workflow doc explains preview
  posture and rollback; `[languages]` opt-in keeps preview disabled by
  default.

## Non-goals

- No runtime mutation execution in any language.
- No default-on preview adapters.
- No new published crate, binary, LSP server, or editor extension.
- No typechecker, build-graph, or runtime tool dependency by default.
- No generated tests.
- No automatic source edits.
- No provider or API calls from the adapter layer.
- No mixed-language schema fork.
- No parity or adequacy claims for preview languages.
- No change to inline-comment defaults, gate authority, or branch
  protection defaults.

## Exit criteria

This proposal moves to `accepted` when:

- RIPR-SPEC-0026, RIPR-SPEC-0027, and RIPR-SPEC-0028 are merged with the
  contract sections that match this proposal's shape.
- Campaign 27 ships with the work-item chain above, fixtures, dogfood
  receipts, capability rows, and the closeout handoff under
  `docs/handoffs/`.
- Rust analyzer behavior is unchanged across the entire campaign.

If, after the adapter boundary lands, TypeScript or Python preview
adapters cannot meet the static-limit reporting bar without depending on
external runtime tooling, the affected per-language spec moves to
`superseded` and a follow-up proposal records the new design.
