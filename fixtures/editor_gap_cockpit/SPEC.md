# Fixture Corpus: editor_gap_cockpit

Spec: RIPR-SPEC-0047

## Given

The editor gap cockpit consumes already-written RIPR artifacts: diagnostics,
gap records, repair routes, static-limit metadata, status state, and command
payloads.

## When

```bash
cargo xtask lsp-cockpit-report
cargo xtask check-fixture-contracts
```

## Then

The nested cases pin the editor projection contract for actionable Rust gaps,
preview static-limit gaps, disabled language state, wrong-root reports, stale
artifacts, and no-action gaps. Each case records diagnostics, hover markdown,
code actions, VS Code status, and a compact gap-projection summary.

## Must Not

- Run hidden analysis from the editor.
- Edit source or generate tests.
- Call providers or run mutation testing.
- Treat preview-language evidence as Rust-level confidence.
- Project stale, wrong-root, disabled, or no-action artifacts as repairable.
