# Editor Adoption Assurance Dogfood Receipts

Date: 2026-05-19

Lane: 3, Editor / LSP UX

Work item: `dogfood/lane3-editor-adoption-assurance-receipts`

Branch: `dogfood/editor-adoption-assurance-receipts`

## Scope

This receipt records external-style editor adoption proof for the Lane 3
Adoption Assurance stack. It proves that the editor path can explain setup,
compatibility, root, artifact, receipt, and first-pr packet states without
making Lane 3 a producer of analyzer truth, PR/CI summaries, policy decisions,
source edits, generated tests, provider calls, or mutation execution.

The checked adoption loop is:

```text
open repo
-> Diagnose Setup
-> Show Status
-> inspect one diagnostic
-> copy bounded repair packet
-> verify
-> receipt
-> refresh
-> inspect first-pr packet
```

## External-Style Cases

| Case | Fixture / Smoke Surface | Editor State Observed | Safe Next Action |
| --- | --- | --- | --- |
| Small Rust crate with no prior artifacts | `fixtures/editor_adoption_assurance/setup_ok` | Setup compatible; first-pr packet missing; receipt missing. | Run saved-workspace analysis, verify, receipt, and first-pr before PR review. |
| Rust workspace with tests/examples | VS Code e2e boundary-gap workspace | Extension activates, server resolves, diagnostics and first-pr repair copy actions stay bounded. | Inspect one diagnostic, copy repair/verify/receipt payloads, then refresh. |
| Clean or no-action workspace | `fixtures/editor_adoption_assurance/setup_ok` plus no first-pr packet | No repair claim is made before artifacts exist. | Continue review or generate current artifacts after a relevant change. |
| Wrong-root artifact | `fixtures/editor_adoption_assurance/wrong_root_artifact` | Wrong-root state fails closed; repair actions are suppressed. | Regenerate artifacts from the active workspace root. |
| Stale receipt | `fixtures/editor_adoption_assurance/stale_receipt` | Receipt is stale; movement is not claimed as current proof. | Rerun verify and receipt from current artifacts. |
| First-pr packet ready | `fixtures/editor_adoption_assurance/first_pr_packet_ready` and VS Code e2e | Top repairable gap is visible; packet copy, repair, verify, and receipt actions are bounded. | Inspect or copy the packet only after identity matches the diagnostic. |
| First-pr packet mismatch | `fixtures/editor_adoption_assurance/first_pr_packet_mismatch` | Gap mismatch fails closed; packet repair actions are suppressed. | Regenerate the packet after current repair/receipt artifacts exist. |
| Preview disabled or unavailable | `fixtures/editor_adoption_assurance/preview_adapter_unavailable` | Preview adapter unavailable is visible and advisory; no preview repair claim is promoted. | Use a compatible preview-enabled binary or remove the preview language from config. |

## Artifact Paths

The dogfood evidence is fixture-backed and editor-smoke-backed:

```text
fixtures/editor_adoption_assurance/*/expected/setup-diagnosis.md
fixtures/editor_adoption_assurance/*/expected/vscode-status.json
fixtures/editor_adoption_assurance/*/expected/lsp-diagnostics.json
fixtures/editor_adoption_assurance/*/expected/lsp-code-actions.json
fixtures/editor_adoption_assurance/*/expected/first-pr-status.json
fixtures/editor_adoption_assurance/*/expected/receipt-status.json
target/ripr/reports/lsp-cockpit.json
target/ripr/reports/lsp-cockpit.md
```

The VS Code smoke also writes and removes temporary workspace artifacts while
testing the packaged command path:

```text
ripr.toml
target/ripr/reports/first-useful-action.json
target/ripr/agent/agent-receipt.json
target/ripr/first-pr/start-here.json
target/ripr/first-pr/start-here.md
target/ripr/reports/start-here.json
target/ripr/reports/start-here.md
```

## Receipt And First-PR States

| State Family | Covered States |
| --- | --- |
| Receipt | missing, stale, mismatch-safe, movement improved through VS Code smoke. |
| First-pr packet | missing, ready/top repairable, mismatch, wrong-root, malformed, no current repair claim before artifacts exist. |
| Setup/root | compatible, server missing, server version mismatch, no workspace, multi-root, wrong-root. |
| Preview | preview adapter unavailable remains advisory and non-gating. |
| Actions | bounded repair, verify, and receipt copy actions appear only for safe packet and matching diagnostic identity. |

## Validation

```bash
cargo xtask lsp-cockpit-report
npm --prefix editors/vscode run compile
npm --prefix editors/vscode run test:e2e
cargo xtask check-doc-index
cargo xtask markdown-links
cargo xtask check-static-language
cargo xtask check-traceability
cargo xtask check-pr
git diff --check
```

Result: pass at authoring.

## Limits

- Editor behavior remains saved-workspace and read-only.
- The editor consumes existing artifacts; it does not produce first-pr packets,
  PR comments, generated CI summaries, or policy decisions.
- Unsafe setup, root, receipt, and first-pr states suppress repair actions.
- Rust remains the default confidence path.
- Preview evidence remains opt-in, syntax-first, advisory, and static-limit
  bounded.
- No runtime adequacy claim.
- No mutation proof.
- No merge approval.
- No gate authority.
- No source edits or generated tests.
- No provider or model calls.

## Next Work Item

`campaign(lane3): close editor adoption assurance`

Close only after the campaign closeout maps requirements to merged artifacts,
records validation, and confirms no analyzer-truth, policy, PR/CI, release,
source-edit, generated-test, provider, mutation, or UI-sprawl scope landed.
