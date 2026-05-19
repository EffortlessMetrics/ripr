# Fixture Corpus: editor_adoption_assurance

Spec: RIPR-SPEC-0054

## Given

A first-use editor user needs setup, compatibility, workspace-root, receipt,
and first-pr packet state before trusting any repair action.

## When

VS Code renders `ripr: Diagnose Setup`, `ripr: Show Status`, diagnostics, and
bounded code actions from already-written saved-workspace artifacts.

## Then

Each case pins one adoption-assurance state with:

- `vscode-status.json`;
- `setup-diagnosis.md`;
- `lsp-diagnostics.json`;
- `lsp-code-actions.json`;
- `first-pr-status.json`;
- `receipt-status.json`.

The corpus covers setup-ready, missing server, server version mismatch,
no-workspace, ambiguous multi-root, wrong-root artifact, stale receipt,
first-pr packet ready, first-pr packet mismatch, and preview-adapter
unavailable states.

## Must Not

Fixtures must not imply source edits, generated tests, provider calls, mutation
execution, runtime adequacy, policy eligibility, gate authority, PR comment
publishing, generated CI summaries, automatic repair, or merge readiness.
