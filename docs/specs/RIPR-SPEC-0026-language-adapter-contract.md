# RIPR-SPEC-0026: Language Adapter Contract

Status: proposed

## Problem

`ripr` is single-language by accident. The analyzer, output schema, LSP,
agent, generated CI, gate, baseline, ledger, proof, front-panel,
packet-index, and inline-comment surfaces all assume Rust source files and
Rust tests, even though the domain types, output renderers, and Lane 4
report producers are already language-neutral.

Mixed-language repositories want the same evidence surface across Rust,
TypeScript/JavaScript, and Python. Adding a second language without an
explicit boundary would multiply single-language assumptions across every
internal module and would tempt RIPR into per-language schema forks,
per-language CLIs, or a workspace split.

This spec defines the language-neutral adapter boundary, the language
router, the additive optional output metadata, the repo-config opt-in, and
the preview posture. Per-language behavior contracts live in separate
specs:

- [RIPR-SPEC-0027: TypeScript preview static facts](RIPR-SPEC-0027-typescript-preview-static-facts.md)
- [RIPR-SPEC-0028: Python preview static facts](RIPR-SPEC-0028-python-preview-static-facts.md)

The proposal context is in
[RIPR-PROP-0001: Multi-Language Adapter Preview](../proposals/RIPR-PROP-0001-multi-language-adapter-preview.md).

## Behavior

The canonical flow is:

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

The router is pure path classification plus repo configuration. Adapters
do the language work. The domain stays language-neutral so every existing
report can carry preview-language findings without a schema fork.

Rust remains the reference adapter and the only adapter that may be
`stable` or higher under the existing capability vocabulary. TypeScript
and Python adapters are `alpha` and explicitly `preview` until per-language
specs and fixtures justify a higher status.

## Inputs

| Input | Required? | Purpose |
| --- | --- | --- |
| Workspace root | yes | Same root the existing CLI/LSP receives. |
| Diff (paths + spans) | yes | Same diff inputs as the Rust path. |
| Source file content | yes | Read through the existing file abstraction. |
| Repo configuration | yes | `ripr.toml` declares which languages are enabled. |
| Cargo feature set | yes | Determines which preview adapters are available in the current binary. |
| Language router decision | yes | Maps each changed file to at most one adapter. |
| Existing report inputs | optional | Lane 4 producers continue to consume their existing artifacts; language metadata is additive. |

The boundary does not consume a runtime typechecker, build graph from a
language toolchain, mutation runner output, provider context, or live
editor buffers (the saved-workspace boundary is unchanged).

## Outputs

The adapter boundary contributes additive optional fields to existing
output schemas. No schema version is incremented; no per-language schema
fork is permitted.

Additive optional fields:

- `language`: one of `rust`, `typescript`, `python`. Omitted when unknown.
- `language_status`: `stable` or `preview`. Omitted when `rust`.
- `owner_kind`: bounded vocabulary (`function`, `method`, `class_method`,
  `arrow_function`, `component`, `module_function`); omitted when unknown.
- `static_limit_kind`: bounded vocabulary describing why a probe could not
  be classified (`dynamic_dispatch`, `metaprogramming`,
  `missing_import_graph`, `decorator_indirection`, `mocked_module`,
  `unsupported_syntax`).

Reports gaining these fields:

- `check.json` finding entries
- repo-exposure seam entries
- agent seam packets
- PR guidance comments
- PR evidence ledger entries
- assistant-loop proof entries
- first-useful-action entries
- pr-review front-panel routes
- report packet index entries (group label only)

Generated CI summaries may group advisory output by language when
`[languages]` declares more than Rust. Default summary output remains
Rust-only when only Rust is enabled.

## Language Router

Routing rules:

- `*.rs` → Rust adapter (always).
- `*.ts`, `*.tsx`, `*.js`, `*.jsx` → TypeScript adapter (preview, opt-in).
- `*.py` → Python adapter (preview, opt-in).
- Files matched by no adapter pass through unchanged (no probes, not an
  error).

Repo configuration adds a `[languages]` section to `ripr.toml`:

```toml
[languages]
enabled = ["rust"]
# Preview adapters are opt-in. Adding "typescript" or "python" enables them.
```

The default `enabled` value is `["rust"]`. Preview adapters do not run
when absent from `enabled`. A `--languages rust,typescript` CLI flag
overrides config when needed.

Runtime opt-in is not the same as build-time availability. The published
default build may include preview adapter support, but Rust-only binaries
are allowed. If `languages.enabled` names a preview language that was not
compiled into the current binary, config validation must fail closed with
an actionable message naming the missing Cargo feature, such as
`lang-typescript` or `lang-python`. The editor and other projection
surfaces must treat that as unavailable adapter state, not as a reason to
invent diagnostics.

## Required Evidence

The contract is supported only when the implementation can show:

- `LanguageId`, `LanguageAdapter`, `LanguageFacts`, `OwnerFact`,
  `TestFact`, `AssertionFact`, `RelatedTest`, `FlowSink`, `Probe`, and
  `StaticLimit` are part of the language-neutral domain or analysis
  layer.
- Rust fact extraction sits behind `RustAdapter` with no observable
  fixture, golden, or output schema change.
- Existing Rust fixtures pass `cargo xtask fixtures` and
  `cargo xtask goldens check` with no diff.
- Additive `language` and `language_status` fields appear only when
  populated and roundtrip through JSON serialization.
- The language router has fixture coverage for `.rs`, `.ts`, `.tsx`,
  `.js`, `.jsx`, `.py`, unmatched extensions, and excluded paths.
- `ripr.toml` parses `[languages] enabled` and rejects unsupported
  values with a clear error.
- `ripr.toml` rejects languages whose adapter Cargo feature is unavailable
  in the current binary with a clear error naming the missing feature.
- Generated CI fixtures cover Rust-default behavior (unchanged) and
  language-grouped advisory summaries when more than Rust is enabled.
- `policy/architecture.txt`, `policy/workspace_shape.txt`,
  `policy/public_api.txt`, and `policy/non_rust_allowlist.txt` remain
  satisfied; no new crate, binary, LSP server, or editor extension is
  added.
- Output schema, traceability, capability matrix, and campaign entries
  point to the new behavior.

## Non-Goals

- No runtime mutation execution.
- No default-on preview adapters.
- No requirement that every distributed binary ships every preview parser
  dependency.
- No new published crate, binary, LSP server, or editor extension.
- No typechecker, build-graph, or runtime tool dependency by default.
- No generated tests.
- No automatic source edits.
- No provider or API calls from the adapter layer.
- No per-language schema fork.
- No parity or adequacy claims for preview languages.
- No reinterpretation of existing exposure vocabulary; preview adapters
  use the same conservative classes and static-limit vocabulary as Rust.
- No change to inline-comment defaults, gate authority, branch protection,
  or `pull_request_target` defaults.

## Acceptance Examples

Rust-only repo, default config:

- Existing Rust fixtures unchanged; `language` and `language_status`
  omitted from all outputs; no preview adapter runs; no CI behavior
  change.

Mixed Rust + TypeScript repo, `[languages] enabled = ["rust", "typescript"]`:

- Rust findings emit `language = "rust"` and continue to drive the front
  panel and gate authority.
- TypeScript findings emit `language = "typescript"`,
  `language_status = "preview"`, and are visible across repo exposure,
  agent packets, PR guidance, evidence ledger, assistant proof, front
  panel, and report packet index.
- Default generated CI groups advisory summaries by language without
  changing pass/fail authority.

Preview disable:

- Removing `typescript` from `[languages] enabled` removes preview
  findings from every report without leaving stale artifacts.

## Test Mapping

Follow-up fixtures and tests cover:

- Rust fixture/golden regression suite (must remain unchanged).
- Additive optional field roundtrip tests for `language`,
  `language_status`, `owner_kind`, and `static_limit_kind`.
- Language router fixtures for `.rs`, `.ts`, `.tsx`, `.js`, `.jsx`,
  `.py`, unmatched extensions, and excluded paths.
- Repo configuration parsing of `[languages] enabled` including
  unsupported values.
- Generated CI fixtures for Rust-default and language-grouped advisory
  summaries.

Per-language tests belong to RIPR-SPEC-0027 and RIPR-SPEC-0028.

## Implementation Mapping

Follow-up implementation belongs to Campaign 27 in
[Implementation campaigns](../IMPLEMENTATION_CAMPAIGNS.md):

- `spec/language-adapter-preview-contract` (this spec set: RIPR-SPEC-0026
  plus RIPR-SPEC-0027 and RIPR-SPEC-0028).
- `analysis/language-adapter-boundary` introduces `LanguageId`,
  `LanguageAdapter`, `LanguageFacts`, and the router behind the existing
  Rust pipeline.
- `analysis/rust-adapter-behind-boundary` moves Rust fact extraction
  behind the adapter trait without changing observable behavior,
  fixtures, or goldens.
- `output/language-metadata` adds the additive optional fields across
  existing reports and updates `docs/OUTPUT_SCHEMA.md`.
- `analysis/typescript-preview-adapter` implements RIPR-SPEC-0027.
- `analysis/python-preview-adapter` implements RIPR-SPEC-0028.
- `lsp/editor-language-routing` extends the VS Code extension language
  selectors without changing Rust saved-workspace defaults.
- `ci/language-aware-grouping` updates generated CI advisory summaries.
- `docs/language-adapter-preview-workflow` documents enabling preview
  adapters, mixed-language reports, preview labels, and static-limit
  guidance.
- `dogfood/language-adapter-preview-receipts` adds checked TypeScript and
  Python preview receipts.
- `campaign/language-adapter-preview-closeout` records the closeout
  audit, preview boundary, and any decisions deferred to a later
  campaign.

No follow-up may change Rust analyzer behavior, recommendation ranking,
gate semantics, LSP/editor behavior for Rust seams, provider behavior,
source files, generated tests, mutation execution, branch protection,
`pull_request_target` defaults, or default CI blocking.

## Metrics

The contract makes these counts available to later metrics surfaces:

- `language_adapter_router_routed_files`
- `language_adapter_router_unmatched_files`
- `language_adapter_languages_enabled`
- `language_adapter_rust_findings`
- `language_adapter_preview_findings_visible`
- `language_adapter_preview_findings_suppressed`
- `language_adapter_static_limit_dynamic_dispatch`
- `language_adapter_static_limit_decorator_indirection`
- `language_adapter_static_limit_missing_import_graph`
- `language_adapter_static_limit_metaprogramming`
- `language_adapter_static_limit_mocked_module`
- `language_adapter_static_limit_unsupported_syntax`
