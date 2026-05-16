# Handoff: Campaign 27 Language Adapter Preview Closeout

Date: 2026-05-13
Branch / PR: `campaign-language-adapter-preview-closeout` / pending at authoring
Latest merged PR: #923 `dogfood(language): add preview adapter receipts` (commit `eec094dfe1441e96740dc77fc35390fee810953d`)

## Current Work Item

`campaign/language-adapter-preview-closeout`

Campaign 27 added opt-in preview language adapters without changing the Rust
reference path. TypeScript/JavaScript and Python now feed the same RIPR domain,
output, editor, generated-CI, and dogfood surfaces as preview evidence.

## What Shipped

| Surface | Evidence |
| --- | --- |
| Adapter contract | [RIPR-PROP-0001](../proposals/RIPR-PROP-0001-multi-language-adapter-preview.md), [RIPR-SPEC-0026](../specs/RIPR-SPEC-0026-language-adapter-contract.md), [RIPR-SPEC-0027](../specs/RIPR-SPEC-0027-typescript-preview-static-facts.md), and [RIPR-SPEC-0028](../specs/RIPR-SPEC-0028-python-preview-static-facts.md). |
| Rust reference adapter boundary | `LanguageId`, `LanguageAdapter`, `LanguageFacts`, and the Rust adapter landed behind the existing package, binary, library, and LSP server. |
| Opt-in language config | `[languages] enabled = ["rust"]` remains the default; TypeScript/JavaScript and Python require explicit config. |
| TypeScript/JavaScript preview facts | Owner, test, assertion/oracle, probe, related-test, preview metadata, and static-limit facts are fixture-backed for the current syntax-first scope. |
| Python preview facts | Owner, test, assertion/oracle, probe, related-test, preview metadata, bounded owner kinds, and structured static-limit facts are fixture-backed. |
| Editor projection | [RIPR-PROP-0003](../proposals/RIPR-PROP-0003-editor-preview-routing.md), [RIPR-SPEC-0036](../specs/RIPR-SPEC-0036-editor-preview-routing.md), [RIPR-SPEC-0037](../specs/RIPR-SPEC-0037-editor-preview-static-limit-projection.md), [ADR 0011](../adr/0011-editor-preview-routing-is-projection-only.md), and [Lane 3 tracker](../lanes/LANE_3_EDITOR_LSP.md) record projection-only editor routing. |
| Generated CI grouping | [RIPR-SPEC-0038](../specs/RIPR-SPEC-0038-generated-pr-ci-review-workflow.md) and generated workflow tests keep Rust-only output unchanged while grouping preview language evidence only when configured. |
| Workflow docs | [Language adapter preview workflow](../LANGUAGE_ADAPTER_PREVIEW.md), [Support tiers](../status/SUPPORT_TIERS.md), and the capability matrix explain how to enable, read, and roll back preview evidence. |
| Receipts | [Language adapter preview dogfood receipts](2026-05-13-language-adapter-preview-receipts.md) and `cargo xtask dogfood` cover preview labels, static limits, disabled-language behavior, and no cross-language related-test routing. |

## What Did Not Change

- Rust default analysis, fixtures, and goldens.
- Public package shape: one package, one binary, one library, one LSP server,
  one VS Code extension.
- Default generated CI blocking.
- Gate authority or branch protection.
- Source files under analysis.
- Generated tests.
- Provider calls.
- Mutation execution.
- Runtime typechecker, import graph, virtualenv, npm install, `tsc`, `tsserver`,
  `mypy`, or `pyright` requirements.

## Validation

Closeout validation for this PR:

```bash
cargo xtask check-campaign
cargo xtask goals next
cargo xtask check-doc-index
cargo xtask markdown-links
cargo xtask check-static-language
cargo xtask check-capabilities
cargo xtask check-traceability
cargo xtask check-pr
git diff --check
```

The final dogfood receipt PR also passed:

```bash
cargo test -p xtask dogfood_language_preview_scenarios_cover_projection_boundaries
cargo test -p xtask dogfood_language_preview_run_checks_static_limit_receipt
cargo xtask dogfood
cargo xtask check-pr
git diff --check
```

## Remaining Limits

- Preview evidence remains opt-in and advisory.
- Preview language evidence is not Rust parity.
- Preview language evidence is not eligible for default gate, RIPR Zero,
  baseline-check, or calibrated-confidence authority without a later explicit
  promotion policy.
- Static limits remain part of the finding story; they are not hidden by editor
  or CI projection.

## Artifacts

- `.ripr/goals/archive/2026-05-13-language-adapter-preview.toml`
- `target/ripr/reports/dogfood.json`
- `target/ripr/reports/dogfood.md`
- `target/ripr/dogfood/language-preview/`
- `docs/handoffs/2026-05-13-language-adapter-preview-receipts.md`

## Next Campaign Handoff

The language-preview lane is closed. The next product planning should focus on
market-fit rails and adopter compression rather than more context machinery:
support-tier visibility, first-successful-PR workflow, clean public badge vs.
PR-evidence boundaries, agent repair packet examples, and honest 0.6.0 release
framing.

## What Not To Do

- Do not promote preview-language evidence into default gates.
- Do not claim runtime proof from syntax-first preview evidence.
- Do not make TypeScript/Python analysis run without explicit `[languages]`
  opt-in.
- Do not add editor actions that parse prose instead of structured metadata.
- Do not add runtime typechecker, import graph, package manager, virtualenv, or
  mutation execution requirements to the preview path by default.
