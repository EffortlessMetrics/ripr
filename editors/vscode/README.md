# ripr: Rust Test-Oracle Gaps

[![VS Marketplace Installs (manual)](https://img.shields.io/badge/VS%20Marketplace-2%20installs-0078D4)](https://marketplace.visualstudio.com/items?itemName=EffortlessMetrics.ripr)
[![Open VSX Downloads](https://img.shields.io/open-vsx/dt/EffortlessMetrics/ripr?label=Open%20VSX%20downloads)](https://open-vsx.org/extension/EffortlessMetrics/ripr)

<!-- VS Marketplace install count is manually maintained. Last checked: 2026-05-07 after the 0.4.0 publish from the public listing. Refresh from publisher metrics when updating this manual count. Do not use live VS Marketplace Shields routes. -->

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

1. Check the `ripr` status bar item for server, workspace, analysis, stale,
   failed, or no-actionable-seam state.
2. Use the Problems panel to find actionable saved-workspace seam diagnostics.
3. Hover a diagnostic to see why RIPR flagged it.
4. Copy the targeted test brief or agent command chain.
5. Open the best related test when RIPR finds an imitation target.
6. Add one focused test.
7. Verify with the copied command chain or the CI artifact packet.

Unsaved-buffer overlays are not enabled by default.

## What ripr Does

`ripr` scans Rust code for mutation-shaped static seams and reports whether
tests appear to contain the discriminators needed to expose the changed
behavior. It uses conservative static-exposure language and is meant to guide
the next useful test, not to prove test adequacy.

It does not run mutation testing, report killed/survived, or prove test
adequacy. Use real mutation testing, such as `cargo-mutants`, for ready-mode
confirmation.

## Settings

- `ripr.server.path`: explicit path to the `ripr` executable. Empty by default.
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
- `ripr: Copy Suggested Assertion`
- `ripr: Copy Targeted Test Brief`
- `ripr: Copy Agent Packet Command`
- `ripr: Copy Agent Brief Command`
- `ripr: Copy After Snapshot Command`
- `ripr: Copy Agent Verify Command`
- `ripr: Copy Agent Receipt Command`
- `ripr: Open Related Test`
- `ripr: Open Settings`

## Preview Limitations

The `0.4.x` extension uses a universal VSIX and downloads native server
binaries from matching GitHub Releases when available. It does not auto-install
Rust tooling, run mutation tests, make automatic edits, or analyze unsaved
buffer overlays by default. Bundled platform-specific VSIXs are planned after
the downloader path is proven.
