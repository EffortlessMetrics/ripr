# Learnings

This log captures repo knowledge that should survive individual PRs and chat
sessions. It is intentionally short and actionable.

## 2026-05-01: Product Contract

`ripr` answers a narrow question:

```text
For the behavior changed in this diff, do the current tests appear to contain
a discriminator that would notice if that behavior were wrong?
```

This should remain the filter for roadmap, architecture, and output decisions.

## 2026-05-01: Static Language

Static findings should use conservative exposure language:

- `exposed`
- `weakly_exposed`
- `reachable_unrevealed`
- `no_static_path`
- `infection_unknown`
- `propagation_unknown`
- `static_unknown`

Do not use mutation-runtime outcome language such as `killed` or `survived`
unless explicit real mutation data is being reported in a calibration context.

## 2026-05-01: Architecture Shape

Keep one published package until there is a real external contract:

```text
Package: ripr
Binary: ripr
Library: ripr
Automation: xtask, unpublished
```

Internal modules remain the seam:

- `domain`
- `app`
- `analysis`
- `output`
- `cli`
- `lsp`

## 2026-05-01: Current Bottleneck

Distribution and product framing are in place for alpha. The next bottleneck is
analyzer truth:

```text
line-oriented scanner
-> facts
-> parser-backed syntax
-> owner symbols
-> oracle facts
-> flow facts
-> activation values
```

## 2026-05-01: Extension Path

The normal editor path must not require `cargo install ripr`. The extension
should resolve the server in this order:

```text
ripr.server.path
bundled server binary
downloaded cached server binary
verified first-run download
ripr on PATH
actionable error
```

## 2026-05-02: Runtime State Is Not Repo State

Durable repo knowledge belongs in reviewed docs, campaign manifests,
capability metadata, traceability, specs, and fixtures. Runtime/session state
belongs under generated artifact directories such as reports, receipts, or
learning output.

Do not commit local checkout notes, machine-specific paths, chat transcript
artifacts, or one-run command transcripts as repository state.

## 2026-05-01: Engineering Debt to Track

The repository currently contains `unwrap`/`expect` usage in code and tests.
That conflicts with the target engineering bar. Do not normalize this pattern in
new work. Pay it down in a scoped PR with explicit fallible handling and tests.

Observed inventory during PR 0:

```text
1 production expect() call site:
  crates/ripr/src/lsp.rs

13 test unwrap() call sites:
  crates/ripr/tests/cli_smoke.rs
  crates/ripr/src/analysis/mod.rs
  crates/ripr/src/lsp.rs

4 string-pattern matches in rust_index.rs intentionally detect unwrap/expect in
analyzed user code and are not panic-family call sites.
```
