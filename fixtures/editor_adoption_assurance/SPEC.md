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

The corpus is executable support for RIPR-SPEC-0054's editor authority matrix.
Each required state should have at least one fixture case with expected:

- `vscode-status.json`;
- `setup-diagnosis.md`;
- `lsp-diagnostics.json`;
- `lsp-code-actions.json`;
- `first-pr-status.json`;
- `receipt-status.json`.

The expected files must show both the user-visible state and the action
authority classification: which repair, navigation, verify, receipt, packet,
or refresh/setup actions are present or suppressed.

| Required state | Fixture case | Required action authority |
| --- | --- | --- |
| Setup compatible, artifacts missing | `setup_ok` | Setup/status visible; no first-pr repair claim before artifacts exist. |
| Server missing | `server_missing` | Repair actions suppressed; install/settings guidance visible. |
| Server version mismatch | `server_version_mismatch` | Repair actions depending on unsupported fields suppressed. |
| No workspace | `no_workspace` | Workspace-root actions suppressed; open-workspace guidance visible. |
| Ambiguous multi-root | `multi_root` | Root-scoped actions suppressed; select-root guidance visible. |
| Wrong-root artifact | `wrong_root_artifact` | Repair, first-pr, verify, and receipt authority suppressed. |
| Stale receipt | `stale_receipt` | Receipt movement not projected as current proof; regeneration guidance visible. |
| First-pr packet ready | `first_pr_packet_ready` | Bounded first-pr packet actions available only for matching identity. |
| First-pr packet mismatch | `first_pr_packet_mismatch` | Diagnostic-scoped first-pr actions suppressed. |
| Preview adapter unavailable | `preview_adapter_unavailable` | Preview state visible and advisory; no stable repair authority. |

Future fixture additions should cover these matrix states before code relies on
them:

- stale artifact;
- malformed artifact;
- unsupported schema;
- missing identity;
- preview-enabled advisory evidence;
- no actionable gap;
- receipt mismatch;
- repairable Rust canonical gap.

Missing required coverage is a test gap. A future fixture validator should fail
when a required state has no fixture case or when an expected code-action file
does not show the correct suppression/projection behavior.

## Must Not

Fixtures must not imply source edits, generated tests, provider calls, mutation
execution, runtime adequacy, policy eligibility, gate authority, PR comment
publishing, generated CI summaries, automatic repair, or merge readiness.
