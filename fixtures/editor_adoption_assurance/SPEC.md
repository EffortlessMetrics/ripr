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
- `receipt-status.json`;
- explicit action-authority classification in fixture metadata.

## Required State Coverage

Each required state must map to at least one fixture case that includes all
pinned outputs above plus expected action authority.

| Required state | Expected authority summary |
| --- | --- |
| `setup_ok` | stable Rust repair path allowed when identity/freshness checks pass |
| `server_missing` | fail closed; setup guidance only |
| `version_mismatch` | fail closed for incompatible fields; guidance allowed |
| `no_workspace` | fail closed; setup/root guidance only |
| `multi_root_ambiguous` | fail closed for root-scoped actions |
| `wrong_root` | fail closed for repair/open/copy actions |
| `stale_artifact` | fail closed for repair actions; refresh guidance |
| `malformed_artifact` | fail closed; regeneration guidance |
| `unsupported_schema` | fail closed for unsupported fields |
| `missing_identity` | fail closed for packet/receipt actions |
| `first_pr_ready` | bounded first-pr projection allowed |
| `first_pr_mismatch` | fail closed for first-pr dependent actions |
| `receipt_stale` | fail closed for receipt-derived authority |
| `receipt_mismatch` | fail closed for receipt-derived authority |
| `preview_adapter_unavailable` | no preview repair claim; explain unavailable |
| `preview_advisory` | advisory projection only; no stable repair authority |
| `no_action` | no repair packet; inspect/setup guidance only |
| `repairable_rust_gap` | repair packet/test/verify/receipt actions allowed |

The corpus covers setup-ready, missing server, server version mismatch,
no-workspace, ambiguous multi-root, wrong-root artifact, stale receipt,
first-pr packet ready, first-pr packet mismatch, and preview-adapter
unavailable states. Remaining required states must be tracked to closure in
fixture expansion and validator enforcement.

## Must Not

Fixtures must not imply source edits, generated tests, provider calls, mutation
execution, runtime adequacy, policy eligibility, gate authority, PR comment
publishing, generated CI summaries, automatic repair, or merge readiness.
