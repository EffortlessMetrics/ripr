# ripr: Rust Test-Oracle Gaps

[![VS Marketplace Installs (manual)](https://img.shields.io/badge/VS%20Marketplace-2%20installs-0078D4)](https://marketplace.visualstudio.com/items?itemName=EffortlessMetrics.ripr)
[![Open VSX Downloads](https://img.shields.io/open-vsx/dt/EffortlessMetrics/ripr?label=Open%20VSX%20downloads)](https://open-vsx.org/extension/EffortlessMetrics/ripr)

<!-- VS Marketplace install count is manually maintained. Last checked: 2026-05-07 after the 0.4.0 publish; 0.5.0 publish completed 2026-05-10 (VS Marketplace + Open VSX). Refresh the count and date from publisher metrics whenever you check; do not use live VS Marketplace Shields routes. -->

Preview VS Code/Open VSX extension for `ripr`, a static Rust analysis tool that
finds weak or missing test oracles and guides the next targeted test.

The extension starts `ripr lsp --stdio`, surfaces saved-workspace diagnostics,
and helps a human or coding agent move from static evidence to one focused
test.

## Requirements

The extension can download and cache the matching `ripr` server binary from
GitHub Releases on first activation. Manual installation is still supported for
offline, pinned, or enterprise-controlled environments.

## Install and First Run

Install `EffortlessMetrics.ripr` from VS Code Marketplace or Open VSX. The
extension should resolve its server automatically, so `cargo install ripr` is a
fallback rather than a required first step.

After opening a Rust/Cargo workspace:

1. Check the `ripr` status bar item for server, workspace, analysis,
   first-useful-action, stale, failed, or no-actionable-seam state. The status
   bar projects an existing workspace-matched
   `target/ripr/reports/first-useful-action.json` report when one is present,
   without rerunning analysis.
2. Use the Problems panel to find actionable saved-workspace seam diagnostics.
3. Hover a diagnostic to see why RIPR flagged it. The seam hover names the
   missing discriminator, related test, suggested test shape, verify and
   receipt commands, and projects an existing first-useful-action match for
   the same seam.
4. Use the intent-titled code actions to copy the targeted test brief, the
   suggested assertion, or the agent handoff command chain.
5. Open the best related test when RIPR finds an imitation target.
6. Add one focused test.
7. Verify with the copied command chain or the CI artifact packet.

Unsaved-buffer overlays are not enabled by default.

For the full editor loop from diagnostic to receipt, see
[`docs/EDITOR_EVIDENCE_WORKFLOW.md`](../../docs/EDITOR_EVIDENCE_WORKFLOW.md).

## What ripr Does

`ripr` scans Rust code for mutation-shaped static seams and reports whether
tests appear to contain the discriminators needed to expose the changed
behavior. It uses conservative static-exposure language and is meant to guide
the next useful test, not to prove test adequacy.

The 0.5.x extension surfaces saved-workspace seam diagnostics, evidence-aware
hovers, intent-titled code actions for inspecting the seam / writing the
targeted test / copying the agent handoff / verifying after the test /
reviewing the receipt / refreshing analysis, an LSP `collectEvidenceContext`
seam handoff packet, and a first-useful-action projection in the status bar
and seam hover when a workspace-matched report already exists.

It does not run mutation testing, report killed/survived, or prove test
adequacy. Use real mutation testing, such as `cargo-mutants`, for ready-mode
confirmation.

## Settings

- `ripr.server.path`: explicit path to the `ripr` executable. Empty by default.
- `ripr.enabled`: enables saved-workspace diagnostics, hovers, status, and code
  actions. Defaults to `true`.
- `ripr.server.args`: arguments used to start the language server. Defaults to
  `["lsp", "--stdio"]`.
- `ripr.server.autoDownload`: download a matching server when needed. Defaults
  to `true`.
- `ripr.server.version`: pinned server version. Empty means match the extension
  version.
- `ripr.server.downloadBaseUrl`: override the manifest location for internal
  mirrors.
- `ripr.check.mode`: preferred editor check mode. Defaults to `draft`.
- `ripr.baseRef`: Git base ref used by context commands. Defaults to
  `origin/main`.
- `ripr.trace.server`: language-server trace setting.

## Commands

- `ripr: Restart Server`
- `ripr: Show Status`
- `ripr: Show Output`
- `ripr: Copy Finding Context`
- `ripr: Write Targeted Test - Copy Suggested Assertion`
- `ripr: Write Targeted Test - Copy Brief`
- `ripr: Agent Handoff - Copy Packet Command`
- `ripr: Agent Handoff - Copy Brief Command`
- `ripr: Verify After Test - Copy After Snapshot Command`
- `ripr: Verify After Test - Copy Verify Command`
- `ripr: Review Result - Copy Receipt Command`
- `ripr: Write Targeted Test - Open Best Related Test`
- `ripr: Open Settings`

## Preview Limitations

The `0.5.x` extension uses a universal VSIX and downloads native server
binaries from matching GitHub Releases when available. It does not auto-install
Rust tooling, run mutation tests, make automatic edits, or analyze unsaved
buffer overlays by default. Bundled platform-specific VSIXs are planned after
the downloader path is proven.
