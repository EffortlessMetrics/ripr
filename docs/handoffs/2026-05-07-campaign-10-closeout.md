# Handoff: Campaign 10 Closeout

Date: 2026-05-07
Branch / PR: `campaign-editor-agent-integration-closeout` / pending
Latest merged PR: #491 `release: prove editor-agent readiness`

## Current Work Item

`campaign/editor-agent-integration-closeout`

Campaign 10 made the saved-workspace editor loop and agent CLI loop line up as
one conservative evidence path:

```text
diagnostic -> evidence -> packet or brief -> focused test -> after snapshot
-> outcome -> agent verify -> agent receipt -> cockpit -> CI artifacts
```

It did not add analyzer families, LSP feature expansion, unsaved-buffer
overlays, automatic edits, runtime mutation execution, CI blocking by default,
public crate splits, or SARIF/badge schema churn.

## Prompt-To-Artifact Audit

| Requirement | Evidence |
| --- | --- |
| Editor copy actions expose the agent loop | `lsp/agent-loop-copy-commands` added packet, brief, after-snapshot, verify, and receipt command copy actions, pinned by boundary-gap LSP action fixtures. |
| Cockpit joins verify and receipt state | `operator/verify-receipt-status` made `operator-cockpit` report before/after snapshots, agent verify JSON, agent receipt JSON, movement counts, and missing-input next commands without rerunning analysis inside the join. |
| Fixture pins the full loop | `fixtures/editor-agent-loop` pins LSP diagnostics/actions, agent packet, agent brief, agent verify, agent receipt, and operator cockpit output for the boundary-gap scenario. |
| Generated CI carries artifacts | `ci/editor-agent-artifacts` uploads pilot output, repo exposure, agent packet, agent brief, agent verify, agent receipt, targeted-test outcome, optional operator cockpit, SARIF when enabled, and badge JSON. |
| Installed-user docs lead with the loop | `docs/full-evidence-loop` centers quickstart and installed-user docs on `ripr pilot`, targeted brief, focused test, after snapshot, `ripr outcome`, `ripr agent verify`, and `ripr agent receipt`. |
| Release readiness proves the installed surface | `release/editor-agent-readiness-proof` makes `cargo xtask release-readiness --version 0.4.0` check installed command surface, boundary-gap pilot/outcome/verify/receipt fixtures, repo-exposure latency, LSP cockpit, advisory workflow defaults, VSIX path, and known-limit docs. |

## PR Chain

- #466 `docs: audit editor-agent integration contract`
- #470 `lsp: add agent loop copy commands`
- #480 `xtask: report agent verification cockpit status`
- `fixtures: pin editor-agent loop`
- `ci: upload editor-agent artifacts`
- `docs: document full evidence loop`
- #491 `release: prove editor-agent readiness`
- `campaign/editor-agent-integration-closeout`

## Verification Run

Closeout proof before opening this PR:

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

The merged readiness proof also ran:

```bash
cargo xtask release-readiness --version 0.4.0
cargo package -p ripr --list
cargo publish -p ripr --dry-run
npm --prefix editors/vscode run package
cargo test --workspace
```

`release-readiness --version 0.4.0` records `warn` only because the crate
version is still `0.3.1`; package-list and publish-dry-run remain explicit
release-prep gates for the version-bump branch.

## Next Work Item

No ready work item remains in `.ripr/goals/active.toml`.

Choose the next product campaign explicitly from the roadmap before starting
new implementation work.

## What Not To Do

- Do not reopen Campaign 10 just to add new editor conveniences.
- Do not add unsaved-buffer overlays, CodeLens, inlay hints, semantic tokens,
  automatic test edits, or generated tests under this closeout.
- Do not turn `ripr agent verify` or `ripr agent receipt` into runtime mutation
  execution.
- Do not make CI blocking default.
- Do not split the public package or broaden SARIF/badge schemas as part of
  editor-agent closeout.
