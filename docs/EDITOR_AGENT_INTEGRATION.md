# Editor Agent Integration

Campaign 10 owns the path from a saved-workspace editor diagnostic to a
review-ready agent receipt:

```text
diagnostic -> evidence -> packet/brief -> focused test -> after snapshot
-> agent verify -> agent receipt -> cockpit -> CI artifacts -> install proof
```

This is an integration contract, not a new analyzer lane. The campaign connects
surfaces that already exist and then pins the loop with fixtures, cockpit joins,
CI artifacts, docs, and install proof. It does not add automatic edits,
generated tests, CodeLens, inlay hints, semantic tokens, unsaved-buffer
overlays, public crates, or a runtime execution engine.

## Release-Surface Gate

Campaign 10 briefly moved to a broader `release-surface-0-4` lane. The useful
release-readiness requirements from that work stay in this campaign as a gate:
the editor-agent loop must be shown through the installed CLI, packaged VSIX,
package dry-run, known-limits docs, and non-blocking CI artifacts before
closeout.

That gate does not replace the active product lane. If the product goal changes
from editor-agent integration to release hardening, open a separate campaign or
rewrite the active manifest explicitly.

## Contract Table

| Stage | Current surface | Gap to close |
| --- | --- | --- |
| Diagnostic | LSP seam diagnostic | Must reference the same seam identity used by agent packet, brief, and receipt |
| Evidence | Hover plus LSP cockpit | Must match agent packet vocabulary |
| Packet | `ripr agent packet --seam-id` | Editor copy command is pinned; fixture and cockpit joins must reuse it |
| Brief | `ripr agent brief` | Editor copy command is pinned; fixture and cockpit joins must reuse it |
| Test write | Human or agent writes focused test | No generated tests and no automatic edits |
| After snapshot | `ripr check --format repo-exposure-json > after.json` | Editor copy command is pinned; cockpit visibility is still pending |
| Verify | `ripr agent verify` | Editor copy command is pinned; cockpit now reports artifact presence and movement counts |
| Receipt | `ripr agent receipt` | Editor copy command is pinned; cockpit now reports artifact presence and receipt summary |
| Cockpit | `cargo xtask operator-cockpit` | Joins before/after snapshots, agent verify JSON, agent receipt JSON, and missing-input commands |
| CI | Generated workflow artifacts | Uploads the full non-blocking editor-agent artifact set |
| Fixture | Boundary-gap `expected/editor-agent-loop/` | Pins the canonical editor-agent loop fixture |
| Install | `cargo install` plus VSIX proof | Needs installed binary plus packaged extension loop proof |

## Existing Surfaces

| Surface | Current command or action | Current role |
| --- | --- | --- |
| LSP seam diagnostic | Saved-workspace LSP diagnostics with seam IDs | Starts the editor path from the same seam identity used by repo exposure and agent packet output |
| Hover evidence | LSP hover on seam diagnostics | Shows evidence, related tests, missing discriminator text, and next-step wording without changing files |
| LSP cockpit | `cargo xtask lsp-cockpit-report` | Checks fixture-pinned diagnostics, hovers, code actions, and VS Code command registration |
| Copy seam packet | `ripr.copyContext` / `ripr agent packet --root . --seam-id <id> --json` | Copies or emits the selected seam packet |
| Copy targeted test brief | `ripr.copyTargetedTestBrief` / `ripr agent brief --root . --diff <patch> --json` | Copies or emits a focused test brief for an agent working set |
| Open related test | `ripr.openRelatedTest` | Opens the strongest related test without editing it |
| Suggested assertion | `ripr.copySuggestedAssertion` | Copies assertion text when the packet has a concrete assertion shape |
| Refresh analysis | `ripr.refresh` | Refreshes saved-workspace diagnostics and latency status |
| After snapshot | `ripr check --root . --mode ready --format repo-exposure-json > target/ripr/workflow/after.repo-exposure.json` | Captures the post-test static exposure state |
| Agent verify | `ripr agent verify --root . --before target/ripr/workflow/before.repo-exposure.json --after target/ripr/workflow/after.repo-exposure.json --json > target/ripr/workflow/agent-verify.json` | Compares before and after repo-exposure snapshots |
| Agent receipt | `ripr agent receipt --root . --verify-json target/ripr/workflow/agent-verify.json --seam-id <id> --json --out target/ripr/reports/agent-receipt.json` | Narrows verify output to one seam for review handoff |
| Operator cockpit | `cargo xtask operator-cockpit` | Joins existing repo-local reports into `target/ripr/reports/operator-cockpit.{json,md}` |
| CI artifacts | Generated GitHub workflow | Uploads advisory repo artifacts and optional SARIF or badge outputs |
| Install proof | `cargo install --path crates/ripr --locked --force --root target/ripr/install-smoke` and `npm --prefix editors/vscode run package` | Shows the installed binary and packaged extension can run the loop |

## Missing Work

| Work item | Missing integration |
| --- | --- |
| `lsp/agent-loop-copy-commands` | Done: seam diagnostics now expose command-oriented editor actions for agent packet, brief, after snapshot, verify, and receipt command text |
| `operator/verify-receipt-status` | Done: cockpit joins before snapshot, after snapshot, agent verify JSON, agent receipt JSON, movement counts, and next commands |
| `fixtures/editor-agent-loop` | Done: boundary-gap pins LSP diagnostics/actions, agent brief, agent packet, agent verify, agent receipt, and operator cockpit output in one canonical fixture |
| `ci/editor-agent-artifacts` | Done: generated workflow uploads pilot summary, repo exposure, agent packet, agent brief, agent verify, agent receipt, targeted-test outcome, optional operator cockpit, SARIF when enabled, and badge JSON as visible artifacts |
| `docs/full-evidence-loop` | Next: make the first-hour path explicit and state that `ripr init` is optional policy materialization, not activation |
| `release/editor-agent-readiness-proof` | Run the editor-agent loop through installed CLI, packaged VSIX, package dry-run, and known-limits checks before closeout |

## Fixture Boundary

The boundary-gap `expected/editor-agent-loop/` fixture is the load-bearing
regression rail for this lane. Later PRs that change the diagnostic-to-receipt
loop must explain any fixture drift and keep LSP diagnostics/actions, agent
brief, agent packet, agent verify, agent receipt, and operator cockpit output
aligned.

## Validation Packet

Docs-only audit PRs use the campaign and contract checks:

```bash
cargo xtask check-campaign
cargo xtask goals next
cargo xtask check-doc-index
cargo xtask check-traceability
cargo xtask check-capabilities
cargo xtask check-static-language
cargo xtask check-output-contracts
cargo xtask check-pr
git diff --check
```

Behavior PRs add the relevant editor, cockpit, fixture, CI, or install proof on
top of those gates. If a PR touches VS Code surfaces, run:

```bash
npm --prefix editors/vscode run compile
npm --prefix editors/vscode run test:e2e
```
