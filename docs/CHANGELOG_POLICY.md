# Changelog Policy

The root changelog records repository-level changes that future contributors,
users, and agents need to notice.

## What To Include

Add an entry for:

- user-visible CLI behavior
- JSON, context, GitHub, SARIF, or LSP output changes
- config changes
- release or packaging changes
- documentation-system changes
- architecture or roadmap decisions that affect future PRs
- known compatibility notes

Do not add entries for:

- purely mechanical formatting
- internal refactors with no behavior, API, output, or workflow impact
- dependency bumps with no user-visible effect, unless they affect MSRV,
  packaging, or security

## Sections

Use these headings when relevant:

- `Added`
- `Changed`
- `Deprecated`
- `Removed`
- `Fixed`
- `Security`
- `Docs`

## Static Language

Changelog entries for static analysis behavior should use conservative exposure
language. Do not describe static findings as proving tests, killing mutants, or
showing survivors.
