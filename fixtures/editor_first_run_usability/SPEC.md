# Fixture Corpus: editor_first_run_usability

Spec: RIPR-SPEC-0049
Spec: RIPR-SPEC-0050

## Given

A first-run or low-context editor user needs to understand setup, no-output,
receipt, and first repair states without knowing RIPR internals.

## When

VS Code renders `ripr: Show Status`, `ripr: Diagnose Setup`, receipt status,
or first repair actions from saved-workspace artifacts.

## Then

Each fixture pins one visible state with:

- `vscode-status.json`;
- `setup-diagnosis.md`;
- `lsp-code-actions.json`;
- `receipt-status.json`.

## Must Not

Fixtures must not imply source edits, generated tests, provider calls, mutation
execution, runtime adequacy, gate eligibility, PR comments, or hidden analysis
reruns from the editor.
