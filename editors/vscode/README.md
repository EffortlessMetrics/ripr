# ripr: Static Mutation Exposure

**Find potential test gaps while you work.**

ripr highlights changed behavior that appears weakly checked by nearby tests.
Hover a finding to read the evidence, open a related test, or copy a focused
brief for a coding agent. You write the test; ripr helps identify what it
should check.

The extension analyzes the saved workspace. Its findings are static and
advisory, not runtime mutation results.

[VS Code Marketplace](https://marketplace.visualstudio.com/items?itemName=EffortlessMetrics.ripr) ·
[Open VSX](https://open-vsx.org/extension/EffortlessMetrics/ripr) ·
[User guide](https://github.com/EffortlessMetrics/ripr/blob/main/docs/EDITOR_EXTENSION.md)

## Install and First Run

Install `EffortlessMetrics.ripr` and open a Rust/Cargo workspace. The released
extension normally downloads and caches its matching native server, so you do
not need to build that server with `cargo install ripr`. Workspace analysis and
test execution still need their own tooling on the workspace host.

Open Problems and hover a diagnostic labeled `ripr`. Inspect the changed
behavior, the missing case or assertion, and the related test. Use the available
code actions to open that test or copy a brief, then make the test edit in your
editor.

If no diagnostics appear, run `ripr: Show Status`. It shows the selected workspace,
server, analysis state, and next action. `ripr: Show Output` provides the logs.
No diagnostics can mean no finding, limited analysis, or a setup problem; read
the status before drawing a conclusion.

Save files before refreshing analysis. Unsaved-buffer overlays are not enabled
by default.

## Requirements

The server runs on the host containing the workspace and needs the repository
and its analysis tooling there. The extension does not install that tooling.
For offline use, a pinned binary, or a source-built server, set `ripr.server.path`.
See [Server provisioning](https://github.com/EffortlessMetrics/ripr/blob/main/docs/SERVER_PROVISIONING.md).

This checkout documents **0.11 development**. The latest GitHub release is
[0.10.0](https://github.com/EffortlessMetrics/ripr/releases/tag/v0.10.0);
its [extension README](https://github.com/EffortlessMetrics/ripr/blob/v0.10.0/editors/vscode/README.md)
describes that release. A development extension needs a compatible local server
until matching release assets are available. Do not assume an unreleased server
can be downloaded.

## What ripr Does

A test can execute the changed code without checking the behavior that changed.
For example, changing `>` to `>=` needs an equality case with an assertion on
the result. Another test far above the threshold would not distinguish it.

ripr reads changed code and related tests for evidence like this. It explains
what it found and where static analysis stopped. It does not run mutants,
generate tests, or prove test adequacy. Use real mutation testing for
execution-backed confirmation.

The [static exposure model](https://github.com/EffortlessMetrics/ripr/blob/main/docs/STATIC_EXPOSURE_MODEL.md)
and [terminology](https://github.com/EffortlessMetrics/ripr/blob/main/docs/TERMINOLOGY.md)
explain the classifications used in detailed reports.

## Repair a gap

In the development extension, `ripr: Start Current Repair` prepares a supported
repair for the current selection. Read the packet's allowed test files,
proposed assertion, verification route, and stop conditions before editing or
delegating work. A diagnostic alone is not authorization or a complete repair
route.

Use the copied continuation commands to finish the attempt. The before/after
receipt records static evidence movement; retain actual test results separately.
Do not substitute a probe ID from `ripr check` for the repository-scoped seam ID
required by the repair command.

Follow the [editor repair walkthrough](https://github.com/EffortlessMetrics/ripr-swarm/blob/main/docs/EDITOR_FIRST_RUN_TO_FIRST_RECEIPT.md)
and [repair recovery](https://github.com/EffortlessMetrics/ripr-swarm/blob/main/docs/REPAIR_ATTEMPT.md)
for the complete sequence. The [PR handoff guide](https://github.com/EffortlessMetrics/ripr-swarm/blob/main/docs/EDITOR_FIRST_PR_BRIDGE_WORKFLOW.md)
explains how to use an existing receipt in a PR packet.

## Settings

The following describes the development extension. Most workspaces can start
with the defaults.

| Setting | Purpose | Default |
| --- | --- | --- |
| `ripr.enabled` | Enable saved-workspace feedback. | `true` |
| `ripr.server.path` | Use an explicit server executable. | Empty |
| `ripr.server.args` | Arguments passed to the server. | `["lsp", "--stdio"]` |
| `ripr.server.autoDownload` | Download a matching server when needed. | `true` |
| `ripr.server.version` | Pin a server version. | Empty; match the extension |
| `ripr.server.downloadBaseUrl` | Use an internal download mirror. | Built-in release location |
| `ripr.check.mode` | Choose editor analysis mode. | `draft` |
| `ripr.baseRef` | Select the Git comparison base. | `origin/main` |
| `ripr.includeUnchangedTests` | Include unchanged tests as evidence. | `true` |
| `ripr.seamDiagnostics` | Include repository-scoped diagnostics. | `true` |
| `ripr.diagnosticProfile` | Show actionable routes or full audit output. | `actionable` |
| `ripr.gitTimeoutMs` | Bound each Git invocation. | `30000` |
| `ripr.refreshDeadlineMs` | Bound one refresh attempt. | `600000` |
| `ripr.trace.server` | Configure language-server tracing. | See extension settings |

## Commands

Start with `ripr: Show Status`, `ripr: Show Output`, and
`ripr: Refresh Diagnostics`. Finding-specific code actions appear on diagnostics.
The development extension also exposes the following commands:

<details>
<summary>Command reference</summary>

- `ripr: Restart Server`
- `ripr: Select Workspace Root`
- `ripr: Diagnose Setup`
- `ripr: Start Current Repair`
- `ripr: Copy Current Repair Packet`
- `ripr: Copy Repo Gap Map`
- `ripr: First PR - Open Packet`
- `ripr: First PR - Copy Summary`
- `ripr: First PR - Copy Repair Packet`
- `ripr: Copy Repair Packet at Cursor`
- `ripr: First PR - Copy Verify Command`
- `ripr: First PR - Copy Receipt Command`
- `ripr: First PR - Copy Regeneration Guidance`
- `ripr: Inspect Test Gap - Copy Context`
- `ripr: Write Targeted Test - Copy Suggested Assertion`
- `ripr: Write Targeted Test - Copy Brief`
- `ripr: Agent Handoff - Copy Packet Command`
- `ripr: Agent Handoff - Copy Brief Command`
- `ripr: Verify After Test - Copy After Snapshot Command`
- `ripr: Verify After Test - Copy Verify Command`
- `ripr: Review Result - Copy Receipt Command`
- `ripr: Write Targeted Test - Open Best Related Test`
- `ripr: Open Settings`
- `ripr: Copy Top Repair Packet`
- `ripr: Copy Verify Command`
- `ripr: Copy Receipt Command (Top Repair Packet)`
- `ripr: Open Report`
- `ripr: Show Top Limitation`
- `ripr: Show Receipt Status`
- `ripr: Copy Receipt Command`
- `ripr: Open Attempt Ledger`
- `ripr: Show Route Quality`

</details>

Some repair commands appear in the editor context menu for Rust and preview
languages. Actions that need a diagnostic payload remain code-action-only.
Default shortcuts are `Ctrl+Alt+R` for Show Status, `Ctrl+Alt+P` for Copy Top
Repair Packet, and `Ctrl+Alt+Shift+P` for Copy Repair Packet at Cursor; use `Cmd`
instead of `Ctrl` on macOS. All are configurable.

## Preview Limitations

The development extension uses a universal VSIX and a matching native server.
It does not auto-install Rust tooling, run mutation tests, apply edits, or enable
unsaved-buffer analysis by default. Rust repair requires a complete supported
route. Other languages have separate preview conditions; see
[Support tiers](https://github.com/EffortlessMetrics/ripr/blob/main/docs/status/SUPPORT_TIERS.md).

## Remote / Web support

For Remote-SSH, containers, and WSL, install the extension on the remote
workspace host. The server runs where the workspace lives; a local-only install
does not analyze the remote workspace.

Browser-only VS Code (`vscode.dev` or `github.dev`) is not supported. The extension
requires a native server process and does not activate in virtual workspaces.

## Coexisting with rust-analyzer

rust-analyzer provides language and compiler feedback. ripr examines whether
related tests appear to check the behavior changed in your diff. Both can appear
in Problems; filter by the `ripr` source label to see only ripr findings.

ripr does not suppress, rewrite, or merge rust-analyzer diagnostics. Disabling
one tool does not disable the other. There is no client-side de-duplication
setting; report confusing overlaps with a small example.
