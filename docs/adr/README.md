# Architecture Decision Records

ADRs record decisions that should not be rediscovered or re-litigated in every
PR. They should be short, dated, and focused on consequences.

## Index

| ADR | Status | Decision |
| --- | --- | --- |
| [0001](0001-one-published-package.md) | accepted | Keep one published package with internal module seams. |
| [0002](0002-static-exposure-language.md) | accepted | Use conservative static exposure language. |
| [0003](0003-fixtures-before-analyzer-rewrites.md) | accepted | Build fixture lab before parser and flow rewrites. |
| [0004](0004-docs-as-planning-artifacts.md) | accepted | Track PR-by-PR implementation through docs, specs, and metrics. |
| [0005](0005-scoped-evidence-heavy-prs.md) | accepted | Scope PRs by production risk, not line count. |
| [0006](0006-rust-syntax-substrate.md) | accepted | Use `ra_ap_syntax` behind the syntax adapter for Campaign 2. |
| [0007](0007-lsp-server-framework.md) | accepted | Use `tower-lsp-server` for the LSP sidecar. |
| [0008](0008-typescript-parser-substrate.md) | proposed | Use `oxc_parser` for the TypeScript preview adapter. |
| [0009](0009-python-parser-substrate.md) | proposed | Use `rustpython-parser` for the Python preview adapter. |
| [0010](0010-fixture-first-evidence-confidence.md) | accepted | Keep Lane 1 evidence confidence fixture-first and class-scoped. |
| [0011](0011-editor-preview-routing-is-projection-only.md) | proposed | Keep editor preview routing projection-only. |
| [0012](0012-editor-gap-projection-is-read-only.md) | proposed | Keep editor gap projection read-only. |
