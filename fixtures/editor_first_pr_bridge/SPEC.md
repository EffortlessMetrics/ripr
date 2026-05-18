# Fixture Corpus: editor_first_pr_bridge

Spec: RIPR-SPEC-0052

## Given

The editor first-pr bridge consumes existing setup, receipt, diagnostic, and
`target/ripr/first-pr/start-here` packet artifacts.

## When

VS Code renders `ripr: Diagnose Setup`, `ripr: Show Status`, and bounded
first-pr packet actions from saved-workspace artifacts.

## Then

Each case pins one visible first-pr bridge state with:

- `vscode-status.json`;
- `setup-diagnosis.md`;
- `lsp-diagnostics.json`;
- `lsp-code-actions.json`;
- `first-pr-status.json`.

The corpus covers setup-ready, packet missing, repairable, no-action, stale,
wrong-root, malformed, receipt-improved, and receipt-unchanged states.

## Must Not

Fixtures must not imply source edits, generated tests, provider calls, mutation
execution, runtime adequacy, policy or gate authority, PR comment publishing,
generated CI summaries, or merge readiness.
